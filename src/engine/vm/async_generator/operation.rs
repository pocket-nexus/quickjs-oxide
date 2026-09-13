//! FIFO driving uses one owned continuation per entered async-generator operation.
use super::AsyncGeneratorSettlement;
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::native::{GeneratorResumeKind, NativeFunctionId};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::heap::{
    AsyncGeneratorResumeKind, AsyncGeneratorState, ContextId, InternalCallableData, ObjectPayload,
};
use crate::engine::object::{CallableRef, ObjectRef};
use crate::engine::value::Value;
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
    suspend::{self, EncodedVmActivation, VmRunOutcome},
};
use crate::engine::vm::{
    VmResume, VmSuspendKind,
    suspend::{RootedVmActivation, VmActivationResume},
};

pub(crate) enum AsyncGeneratorStep {
    Complete(Completion),
    Run {
        activation: Box<RootedVmActivation>,
        input: VmActivationResume,
        resume: Box<AsyncGeneratorResume>,
    },
    Resolve {
        value: Value,
        realm: ContextId,
        resume: Box<AsyncGeneratorResume>,
    },
    Call {
        callable: CallableRef,
        value: Value,
        resume: Box<AsyncGeneratorResume>,
    },
}
pub(crate) struct AsyncGeneratorResume {
    runtime: Runtime,
    realm: ContextId,
    generator: Option<ObjectRef>,
    output: Value,
    phase: Phase,
    cleanup: Cleanup,
}
enum Phase {
    Body,
    Await(Box<EncodedVmActivation>),
    CompletedReturn,
    Settled { pump: bool },
}
#[derive(Clone, Copy)]
enum Cleanup {
    None,
    Executing,
    AwaitingReturn,
}
impl Drop for AsyncGeneratorResume {
    fn drop(&mut self) {
        if let Some(generator) = &self.generator {
            match self.cleanup {
                Cleanup::None => {}
                Cleanup::Executing => {
                    let _ = self.runtime.complete_async_generator(generator);
                }
                Cleanup::AwaitingReturn => {
                    let _ = self
                        .runtime
                        .finish_async_generator_completed_return(generator);
                }
            }
        }
    }
}
impl AsyncGeneratorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        target: NativeFunctionId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "AsyncGenerator operation received constructor invocation",
            ));
        };
        let argument = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "AsyncGenerator operation argv was not padded",
            ))?;
        if let NativeFunctionId::AsyncGeneratorPrototypeResume(kind) = target {
            let capability = runtime.new_default_promise_capability(realm)?;
            let promise = Value::Object(capability.promise.clone());
            let generator = match this_value {
                Value::Object(generator)
                    if matches!(
                        runtime
                            .0
                            .state
                            .borrow()
                            .heap
                            .object(generator.object_id())?
                            .payload,
                        ObjectPayload::AsyncGenerator(_)
                    ) =>
                {
                    Some(generator.clone())
                }
                _ => None,
            };
            let resume = Box::new(AsyncGeneratorResume {
                runtime: runtime.clone(),
                realm,
                generator,
                output: promise,
                phase: Phase::Settled { pump: false },
                cleanup: Cleanup::None,
            });
            let Some(generator) = &resume.generator else {
                let reason = runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "not an async generator",
                )?;
                return Ok(Self::Call {
                    callable: capability.reject,
                    value: reason,
                    resume,
                });
            };
            runtime.enqueue_async_generator_request(generator, kind, argument, &capability)?;
            let state = runtime
                .0
                .state
                .borrow()
                .heap
                .async_generator_snapshot(generator.object_id())?
                .state;
            if matches!(
                state,
                AsyncGeneratorState::Executing | AsyncGeneratorState::AwaitingReturn
            ) {
                return Ok(resume.finish());
            }
            return resume.pump();
        }
        let NativeFunctionId::AsyncGeneratorResume(target_kind) = target else {
            return Err(RuntimeError::Invariant("wrong async generator operation"));
        };
        let active = runtime.active_function()?;
        let internal = runtime
            .0
            .state
            .borrow()
            .heap
            .native_internal_callable(active.object_id())?
            .ok_or(RuntimeError::Invariant(
                "AsyncGenerator continuation has no internal state",
            ))?;
        let InternalCallableData::AsyncGeneratorResume { generator, kind } = internal else {
            return Err(RuntimeError::Invariant(
                "AsyncGenerator continuation has the wrong internal state",
            ));
        };
        if kind != target_kind {
            return Err(RuntimeError::Invariant(
                "AsyncGenerator continuation target disagrees with its capture",
            ));
        }
        let generator = ObjectRef::from_borrowed_handle(runtime.clone(), generator)?;
        let snapshot = runtime
            .0
            .state
            .borrow()
            .heap
            .async_generator_snapshot(generator.object_id())?;
        let mut resume = Box::new(AsyncGeneratorResume {
            runtime: runtime.clone(),
            realm,
            generator: Some(generator.clone()),
            output: Value::Undefined,
            phase: Phase::Body,
            cleanup: Cleanup::None,
        });
        match kind {
            AsyncGeneratorResumeKind::AwaitFulfill | AsyncGeneratorResumeKind::AwaitReject => {
                // A synchronous resolver reentry can make a queued reaction stale.
                if snapshot.state != AsyncGeneratorState::Executing {
                    return Ok(resume.finish());
                }
                if snapshot.resume_realm.is_none() {
                    return Err(RuntimeError::Invariant(
                        "AsyncGenerator continuation has no installed awaiting realm",
                    ));
                }
                let activation = snapshot
                    .activation
                    .as_deref()
                    .ok_or(RuntimeError::Invariant(
                        "awaiting AsyncGenerator has no activation",
                    ))?;
                let rooted = suspend::thaw(
                    runtime.clone(),
                    VmSuspendKind::Await,
                    realm,
                    activation,
                    FunctionKind::AsyncGenerator,
                )?;
                resume.detach(AsyncGeneratorState::Executing)?;
                let input = if kind == AsyncGeneratorResumeKind::AwaitFulfill {
                    VmActivationResume::AwaitFulfill(argument)
                } else {
                    VmActivationResume::AwaitReject(argument)
                };
                Ok(Self::Run {
                    activation: Box::new(rooted),
                    input,
                    resume,
                })
            }
            AsyncGeneratorResumeKind::ReturnFulfill | AsyncGeneratorResumeKind::ReturnReject => {
                if snapshot.resume_realm.is_none() {
                    return Err(RuntimeError::Invariant(
                        "AsyncGenerator continuation has no installed awaiting realm",
                    ));
                }
                if snapshot.state != AsyncGeneratorState::AwaitingReturn
                    || snapshot.activation.is_some()
                {
                    return Err(RuntimeError::Invariant(
                        "AsyncGenerator return continuation reached an invalid state",
                    ));
                }
                runtime.finish_async_generator_completed_return(&generator)?;
                let settlement = if kind == AsyncGeneratorResumeKind::ReturnFulfill {
                    AsyncGeneratorSettlement::Resolve {
                        value: argument,
                        done: true,
                    }
                } else {
                    AsyncGeneratorSettlement::Reject(argument)
                };
                // Completed-return reactions service exactly one request.
                resume.settle(settlement, false)
            }
        }
    }
    pub(crate) fn finish(self, runtime: &Runtime) -> Result<Completion, RuntimeError> {
        #[cfg(feature = "stack-vm")]
        {
            let realm = match &self {
                Self::Complete(_) => {
                    return if let Self::Complete(completion) = self {
                        Ok(completion)
                    } else {
                        unreachable!()
                    };
                }
                Self::Run { resume, .. }
                | Self::Resolve { resume, .. }
                | Self::Call { resume, .. } => resume.realm,
            };
            super::super::driver::execute_root(
                runtime.clone(),
                realm,
                super::super::driver::RootOperation::AsyncGenerator(self),
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
                    } => {
                        let realm = resume.realm;
                        resume.resume(runtime.call_internal(
                            realm,
                            &callable,
                            Value::Undefined,
                            &[value],
                        )?)?
                    }
                };
            }
        }
    }
}
impl AsyncGeneratorResume {
    fn generator(&self) -> Result<&ObjectRef, RuntimeError> {
        self.generator.as_ref().ok_or(RuntimeError::Invariant(
            "async generator operation has no generator",
        ))
    }
    fn finish(mut self: Box<Self>) -> AsyncGeneratorStep {
        self.cleanup = Cleanup::None;
        AsyncGeneratorStep::Complete(Completion::Return(std::mem::replace(
            &mut self.output,
            Value::Undefined,
        )))
    }
    fn detach(&mut self, expected: AsyncGeneratorState) -> Result<(), RuntimeError> {
        let id = self.generator()?.object_id();
        let (previous, _activation, cleanup) = self
            .runtime
            .0
            .state
            .borrow_mut()
            .heap
            .begin_async_generator_resume(id)?;
        // The runtime transition has committed; cover even cleanup failure.
        self.cleanup = Cleanup::Executing;
        self.runtime.0.state.borrow_mut().apply_cleanup(cleanup)?;
        if previous != expected {
            return Err(RuntimeError::Invariant(
                "AsyncGenerator activation changed before resume",
            ));
        }
        self.phase = Phase::Body;
        Ok(())
    }
    fn pump(mut self: Box<Self>) -> Result<AsyncGeneratorStep, RuntimeError> {
        loop {
            let generator = self.generator()?.clone();
            let snapshot = self
                .runtime
                .0
                .state
                .borrow()
                .heap
                .async_generator_snapshot(generator.object_id())?;
            let Some(request) = snapshot.queue.front() else {
                return Ok(self.finish());
            };
            let previous = snapshot.state;
            match previous {
                AsyncGeneratorState::AwaitingReturn => return Ok(self.finish()),
                AsyncGeneratorState::SuspendedStart
                    if request.completion != GeneratorResumeKind::Next =>
                {
                    self.runtime.complete_async_generator(&generator)?;
                    continue;
                }
                AsyncGeneratorState::Completed => {
                    return match request.completion {
                        GeneratorResumeKind::Next => self.settle(
                            AsyncGeneratorSettlement::Resolve {
                                value: Value::Undefined,
                                done: true,
                            },
                            false,
                        ),
                        GeneratorResumeKind::Throw => {
                            let request = self
                                .runtime
                                .root_front_async_generator_request(&generator)?;
                            self.settle(AsyncGeneratorSettlement::Reject(request.result), false)
                        }
                        GeneratorResumeKind::Return => {
                            let request = self
                                .runtime
                                .root_front_async_generator_request(&generator)?;
                            self.runtime
                                .0
                                .state
                                .borrow_mut()
                                .heap
                                .begin_async_generator_completed_return(
                                    generator.object_id(),
                                    self.realm,
                                )?;
                            self.cleanup = Cleanup::AwaitingReturn;
                            self.phase = Phase::CompletedReturn;
                            Ok(AsyncGeneratorStep::Resolve {
                                value: request.result,
                                realm: self.realm,
                                resume: self,
                            })
                        }
                    };
                }
                _ => {}
            }
            let kind = match previous {
                AsyncGeneratorState::SuspendedStart => VmSuspendKind::Initial,
                AsyncGeneratorState::SuspendedYield => VmSuspendKind::Yield,
                AsyncGeneratorState::SuspendedYieldStar => VmSuspendKind::AsyncYieldStar,
                AsyncGeneratorState::Executing => VmSuspendKind::Await,
                _ => unreachable!(),
            };
            let activation = snapshot
                .activation
                .as_deref()
                .ok_or(RuntimeError::Invariant(
                    "suspended AsyncGenerator has no activation",
                ))?;
            let rooted = suspend::thaw(
                self.runtime.clone(),
                kind,
                self.realm,
                activation,
                FunctionKind::AsyncGenerator,
            )?;
            let request = self
                .runtime
                .root_front_async_generator_request(&generator)?;
            self.detach(previous)?;
            let input = match previous {
                AsyncGeneratorState::SuspendedStart => VmActivationResume::Initial,
                AsyncGeneratorState::SuspendedYield | AsyncGeneratorState::SuspendedYieldStar => {
                    VmActivationResume::Generator(match request.completion {
                        GeneratorResumeKind::Next => VmResume::Next(request.result),
                        GeneratorResumeKind::Return => VmResume::Return(request.result),
                        GeneratorResumeKind::Throw => VmResume::Throw(request.result),
                    })
                }
                // The still-running outer pump resumes a reentrantly parked await.
                AsyncGeneratorState::Executing => {
                    VmActivationResume::AwaitFulfill(Value::Undefined)
                }
                _ => unreachable!(),
            };
            return Ok(AsyncGeneratorStep::Run {
                activation: Box::new(rooted),
                input,
                resume: self,
            });
        }
    }
    fn settle(
        mut self: Box<Self>,
        settlement: AsyncGeneratorSettlement,
        pump: bool,
    ) -> Result<AsyncGeneratorStep, RuntimeError> {
        let generator = self.generator()?;
        let request = self.runtime.root_front_async_generator_request(generator)?;
        let (callable, value) = match settlement {
            AsyncGeneratorSettlement::Resolve { value, done } => (
                request.resolve,
                Value::Object(self.runtime.new_iterator_result(self.realm, value, done)?),
            ),
            AsyncGeneratorSettlement::Reject(reason) => (request.reject, reason),
        };
        // Allocate and root the result before transferring the queued capability.
        self.runtime
            .remove_front_async_generator_request(generator)?;
        self.phase = Phase::Settled { pump };
        Ok(AsyncGeneratorStep::Call {
            callable,
            value,
            resume: self,
        })
    }
    pub(crate) fn body(
        mut self: Box<Self>,
        outcome: VmRunOutcome,
    ) -> Result<AsyncGeneratorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Body) {
            return Err(RuntimeError::Invariant(
                "async generator body reply has wrong phase",
            ));
        }
        match outcome {
            VmRunOutcome::Complete(completion) => {
                self.runtime.complete_async_generator(self.generator()?)?;
                self.cleanup = Cleanup::None;
                let settlement = match completion {
                    Completion::Return(value) => {
                        AsyncGeneratorSettlement::Resolve { value, done: true }
                    }
                    Completion::Throw(value) => AsyncGeneratorSettlement::Reject(value),
                };
                self.settle(settlement, true)
            }
            VmRunOutcome::Suspend { value, activation } => match activation.kind {
                VmSuspendKind::Yield | VmSuspendKind::AsyncYieldStar => {
                    let state = if activation.kind == VmSuspendKind::Yield {
                        AsyncGeneratorState::SuspendedYield
                    } else {
                        AsyncGeneratorState::SuspendedYieldStar
                    };
                    self.runtime.store_async_generator_suspension(
                        self.generator()?,
                        state,
                        None,
                        &activation,
                    )?;
                    self.cleanup = Cleanup::None;
                    // Keep the encoded owner alive through the raw-edge publication.
                    drop(activation);
                    self.settle(
                        AsyncGeneratorSettlement::Resolve { value, done: false },
                        true,
                    )
                }
                VmSuspendKind::Await => {
                    self.phase = Phase::Await(activation);
                    Ok(AsyncGeneratorStep::Resolve {
                        value,
                        realm: self.realm,
                        resume: self,
                    })
                }
                _ => Err(RuntimeError::Invariant(
                    "AsyncGenerator stopped at an unsupported suspension",
                )),
            },
        }
    }
    pub(crate) fn resume(
        mut self: Box<Self>,
        completion: Completion,
    ) -> Result<AsyncGeneratorStep, RuntimeError> {
        if matches!(self.phase, Phase::Body) {
            return self.body(VmRunOutcome::Complete(completion));
        }
        let phase = std::mem::replace(&mut self.phase, Phase::Body);
        match phase {
            Phase::Body => Err(RuntimeError::Invariant(
                "async generator expected body outcome",
            )),
            Phase::Settled { pump } => {
                if pump {
                    self.pump()
                } else {
                    Ok(self.finish())
                }
            }
            Phase::Await(activation) => {
                let promise = match completion {
                    Completion::Return(Value::Object(promise)) => promise,
                    Completion::Return(_) => {
                        return Err(RuntimeError::Invariant(
                            "intrinsic PromiseResolve returned a non-object",
                        ));
                    }
                    Completion::Throw(reason) => {
                        let rooted = suspend::thaw(
                            self.runtime.clone(),
                            VmSuspendKind::Await,
                            self.realm,
                            &activation.data,
                            FunctionKind::AsyncGenerator,
                        )?;
                        drop(activation);
                        return Ok(AsyncGeneratorStep::Run {
                            activation: Box::new(rooted),
                            input: VmActivationResume::AwaitReject(reason),
                            resume: self,
                        });
                    }
                };
                let generator = self.generator()?;
                let fulfill = self.runtime.new_async_generator_resume_callback(
                    self.realm,
                    generator,
                    AsyncGeneratorResumeKind::AwaitFulfill,
                )?;
                let reject = self.runtime.new_async_generator_resume_callback(
                    self.realm,
                    generator,
                    AsyncGeneratorResumeKind::AwaitReject,
                )?;
                self.runtime.store_async_generator_suspension(
                    generator,
                    AsyncGeneratorState::Executing,
                    Some(self.realm),
                    &activation,
                )?;
                self.runtime.perform_promise_then_without_capability(
                    self.realm, &promise, &fulfill, &reject,
                )?;
                drop(activation);
                Ok(self.finish())
            }
            Phase::CompletedReturn => {
                let promise = match completion {
                    Completion::Return(Value::Object(promise)) => promise,
                    Completion::Return(_) => {
                        return Err(RuntimeError::Invariant(
                            "completed-return PromiseResolve returned a non-object",
                        ));
                    }
                    Completion::Throw(reason) => self
                        .runtime
                        .new_rejected_default_promise(self.realm, reason)?,
                };
                let generator = self.generator()?;
                let fulfill = self.runtime.new_async_generator_resume_callback(
                    self.realm,
                    generator,
                    AsyncGeneratorResumeKind::ReturnFulfill,
                )?;
                let reject = self.runtime.new_async_generator_resume_callback(
                    self.realm,
                    generator,
                    AsyncGeneratorResumeKind::ReturnReject,
                )?;
                self.runtime.perform_promise_then_without_capability(
                    self.realm, &promise, &fulfill, &reject,
                )?;
                Ok(self.finish())
            }
        }
    }
}
