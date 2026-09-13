//! Async body, intrinsic resolution and settlement are separate owned requests.
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::heap::{
    AsyncFunctionPhase, AsyncFunctionResumeKind, ContextId, InternalCallableData,
};
use crate::engine::object::{CallableRef, ObjectRef};
use crate::engine::value::Value;
use crate::engine::vm::{
    Completion, VmSuspendKind,
    suspend::{EncodedVmActivation, RootedVmActivation, VmActivationResume, VmRunOutcome},
};

pub(crate) enum AsyncStep {
    Complete(Completion),
    Run {
        activation: Box<RootedVmActivation>,
        input: VmActivationResume,
        resume: Box<AsyncResume>,
    },
    Resolve {
        value: Value,
        realm: ContextId,
        resume: Box<AsyncResume>,
    },
    Call {
        callable: CallableRef,
        value: Value,
        resume: Box<AsyncResume>,
    },
}

pub(crate) struct AsyncResume {
    runtime: Runtime,
    state: ObjectRef,
    output: Value,
    phase: Phase,
    active: bool,
}
enum Phase {
    Body,
    Await(Box<EncodedVmActivation>),
    Settled,
}

impl AsyncResume {
    pub(in crate::engine::vm) fn start(
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Box<Self>, RuntimeError> {
        let capability = runtime.new_default_promise_capability(realm)?;
        let state = runtime.allocate_async_function_state(realm, &capability)?;
        Ok(Box::new(Self {
            runtime: runtime.clone(),
            state,
            output: Value::Object(capability.promise),
            phase: Phase::Body,
            active: true,
        }))
    }
    pub(super) fn resumed(runtime: &Runtime, state: ObjectRef) -> Box<Self> {
        Box::new(Self {
            runtime: runtime.clone(),
            state,
            output: Value::Undefined,
            phase: Phase::Body,
            active: true,
        })
    }
    pub(crate) fn body(
        mut self: Box<Self>,
        outcome: VmRunOutcome,
    ) -> Result<AsyncStep, RuntimeError> {
        if !matches!(self.phase, Phase::Body) {
            return Err(RuntimeError::Invariant(
                "async body replied in the wrong phase",
            ));
        }
        match outcome {
            VmRunOutcome::Complete(completion) => self.settle(completion),
            VmRunOutcome::Suspend { value, activation } => {
                if activation.kind != VmSuspendKind::Await {
                    return Err(RuntimeError::Invariant(
                        "async function stopped at a non-await suspension",
                    ));
                }
                let realm = self
                    .runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .async_function_state_snapshot(self.state.object_id())?
                    .driver_realm;
                self.phase = Phase::Await(activation);
                Ok(AsyncStep::Resolve {
                    value,
                    realm,
                    resume: self,
                })
            }
        }
    }
    fn settle(mut self: Box<Self>, completion: Completion) -> Result<AsyncStep, RuntimeError> {
        let snapshot = self
            .runtime
            .0
            .state
            .borrow()
            .heap
            .async_function_state_snapshot(self.state.object_id())?;
        if snapshot.phase == AsyncFunctionPhase::Completed {
            return Err(RuntimeError::Invariant(
                "async function settled more than once",
            ));
        }
        let (target, value) = match completion {
            Completion::Return(value) => (snapshot.outer_resolve, value),
            Completion::Throw(value) => (snapshot.outer_reject, value),
        };
        let target = ObjectRef::from_borrowed_handle(self.runtime.clone(), target)?;
        let callable = self
            .runtime
            .as_callable(&target)?
            .ok_or(RuntimeError::Invariant(
                "async function outer resolving function is not callable",
            ))?;
        self.runtime.complete_async_function_state(&self.state)?;
        self.active = false;
        self.phase = Phase::Settled;
        Ok(AsyncStep::Call {
            callable,
            value,
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self: Box<Self>,
        completion: Completion,
    ) -> Result<AsyncStep, RuntimeError> {
        match std::mem::replace(&mut self.phase, Phase::Body) {
            Phase::Body => self.body(VmRunOutcome::Complete(completion)),
            Phase::Settled => self.finish(), // Consume either JS completion from the internal resolving pair.
            Phase::Await(activation) => {
                let promise = match completion {
                    Completion::Throw(reason) => return self.settle(Completion::Throw(reason)),
                    Completion::Return(Value::Object(promise)) => promise,
                    Completion::Return(_) => {
                        return Err(RuntimeError::Invariant(
                            "intrinsic PromiseResolve returned a non-object",
                        ));
                    }
                };
                let realm = self
                    .runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .async_function_state_snapshot(self.state.object_id())?
                    .driver_realm;
                let make_resume = |kind| {
                    self.runtime.new_internal_promise_function(
                        realm,
                        NativeFunctionId::AsyncFunctionResume(kind),
                        1,
                        1,
                        InternalCallableData::AsyncFunctionResume {
                            state: self.state.object_id(),
                            kind,
                        },
                    )
                };
                let fulfill = make_resume(AsyncFunctionResumeKind::Fulfill)?;
                let reject = make_resume(AsyncFunctionResumeKind::Reject)?;
                self.runtime
                    .store_async_function_activation(&self.state, &activation)?;
                self.runtime
                    .perform_promise_then_without_capability(realm, &promise, &fulfill, &reject)?;
                self.active = false;
                self.finish()
            }
        }
    }
    fn finish(mut self: Box<Self>) -> Result<AsyncStep, RuntimeError> {
        Ok(AsyncStep::Complete(Completion::Return(std::mem::replace(
            &mut self.output,
            Value::Undefined,
        ))))
    }
}
impl Drop for AsyncResume {
    fn drop(&mut self) {
        if self.active {
            let _ = self.runtime.complete_async_function_state(&self.state);
        }
    }
}
impl AsyncStep {
    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        #[cfg(feature = "stack-vm")]
        {
            crate::engine::vm::execute_root(
                runtime.clone(),
                realm,
                crate::engine::vm::RootOperation::Async(self),
            )
            .map_err(RuntimeError::Engine)
        }
        #[cfg(not(feature = "stack-vm"))]
        {
            let mut step = self;
            loop {
                step = match step {
                    Self::Complete(completion) => return Ok(completion),
                    Self::Run {
                        activation,
                        input,
                        resume,
                    } => resume.body(activation.run(runtime, input)?)?,
                    Self::Resolve {
                        value,
                        realm,
                        resume,
                    } => resume.resume(runtime.promise_resolve_intrinsic(realm, value)?)?,
                    Self::Call {
                        callable,
                        value,
                        resume,
                    } => resume.resume(runtime.call_internal(
                        realm,
                        &callable,
                        Value::Undefined,
                        &[value],
                    )?)?,
                };
            }
        }
    }
}

#[cfg(all(test, feature = "stack-vm", feature = "profiling"))]
mod tests {
    use crate::engine::{
        api::{profiling::CostProfile, runtime::Runtime},
        value::Value,
    };

    #[test]
    fn await_thenable_jobs_and_finally_stay_owned_across_gc() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(context.eval("var asyncResult=0, asyncReads=0, asyncFinally=0; async function f(){try {return 2+await {get then(){asyncReads++;return resolve=>resolve(40)}};} finally {asyncFinally=[1,2].map(x=>x+1)[1]}} f().then(x=>asyncResult=x); asyncResult").unwrap(), Value::Int(0));
        let mut jobs = 0;
        while runtime.is_job_pending() {
            runtime.run_gc().unwrap();
            runtime.execute_pending_job().unwrap();
            jobs += 1;
        }
        assert!(jobs >= 3);
        assert_eq!(
            context
                .eval("asyncResult===42 && asyncReads===1 && asyncFinally===3")
                .unwrap(),
            Value::Bool(true)
        );
        let cost = profile.snapshot();
        assert_eq!(cost.legacy_dispatches, 0, "{cost:?}");
        assert_eq!(cost.owned_bridge_exits, 0, "{cost:?}");
        assert_eq!(cost.owned_sync_call_bridges, 0, "{cost:?}");
    }
}
