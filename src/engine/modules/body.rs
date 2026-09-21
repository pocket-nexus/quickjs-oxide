//! A module body and its optional TLA reactions share the owned call protocol.
use super::{ModuleBytecodeRef, ModuleRecordBody};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::{
    native::{ModuleEvaluationKind, NativeFunctionId},
    promise::operation::PromiseStep,
};
use crate::engine::heap::{
    ContextId, InternalCallableData, PromiseState, RawModuleRef, roots::VarRefRoot,
};
use crate::engine::object::{CallableRef, ObjectRef};
use crate::engine::value::JsValue;
use crate::engine::vm::Completion;

pub(crate) enum BodyStep {
    Complete(Completion),
    Call {
        callable: CallableRef,
        resume: Box<BodyResume>,
    },
    Promise {
        step: Box<PromiseStep>,
        resume: Box<BodyResume>,
    },
}
enum Phase {
    Sync,
    Async,
    Attach,
}
pub(crate) struct BodyResume {
    root: ModuleBytecodeRef,
    realm: ContextId,
    phase: Phase,
}
impl BodyStep {
    pub(crate) fn release(self, runtime: &Runtime) {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value)) => {
                let _ = runtime.release_jsvalue(value);
            }
            Self::Call { .. } => {}
            Self::Promise { step, .. } => (*step).release(runtime),
        }
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        module: RawModuleRef,
        asynchronous: bool,
    ) -> Result<Self, RuntimeError> {
        let root = runtime.root_module(module)?;
        if asynchronous {
            return Ok(Self::Call {
                callable: runtime.module_callable(module)?,
                resume: Box::new(BodyResume {
                    root,
                    realm,
                    phase: Phase::Async,
                }),
            });
        }
        let record = runtime.module_record(module)?;
        match &record.body {
            ModuleRecordBody::SourceText { .. } => Ok(Self::Call {
                callable: runtime.module_callable(module)?,
                resume: Box::new(BodyResume {
                    root,
                    realm,
                    phase: Phase::Sync,
                }),
            }),
            ModuleRecordBody::Json { default_value } => {
                let slot = record
                    .instance
                    .as_ref()
                    .and_then(|instance| instance.slots.first())
                    .and_then(|slot| *slot)
                    .ok_or(RuntimeError::Invariant(
                        "linked JSON module has no default live cell",
                    ))?;
                let slot = VarRefRoot::from_borrowed_handle(runtime.clone(), slot)?;
                let default_value = JsValue::from_raw(default_value.clone()).ok_or(
                    RuntimeError::Invariant("JSON module default value is an internal sentinel"),
                )?;
                runtime.write_var_ref(&slot, runtime.dup_jsvalue(&default_value)?)?;
                Ok(Self::Complete(Completion::Return(JsValue::Undefined)))
            }
            ModuleRecordBody::Parsing => Err(RuntimeError::Invariant(
                "module execution reached a parse-in-progress record",
            )),
            ModuleRecordBody::Aborted => Err(RuntimeError::AbortedModule),
        }
    }
}
impl BodyResume {
    pub(crate) fn resume(
        mut self: Box<Self>,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<BodyStep, RuntimeError> {
        match self.phase {
            Phase::Sync => inspect_sync(runtime, completion).map(BodyStep::Complete),
            Phase::Attach => {
                match completion {
                    Completion::Throw(reason) => {
                        // Preserve the ignored abrupt attachment and pending exception.
                        runtime.set_pending_exception_jsvalue(reason)?;
                    }
                    Completion::Return(value) => runtime.release_jsvalue(value)?,
                }
                Ok(BodyStep::Complete(Completion::Return(JsValue::Undefined)))
            }
            Phase::Async => {
                let promise = match completion {
                    Completion::Return(value) => match value {
                        JsValue::Object(promise) => {
                            ObjectRef::from_owned_handle(runtime.clone(), promise)
                        }
                        value => {
                            runtime.release_jsvalue(value)?;
                            return Err(RuntimeError::Invariant(
                                "async module callable did not return a Promise",
                            ));
                        }
                    },
                    Completion::Throw(value) => {
                        runtime.release_jsvalue(value)?;
                        return Err(RuntimeError::Invariant(
                            "async module callable did not return a Promise",
                        ));
                    }
                };
                let module = self.root.raw;
                let make_handler = |kind| {
                    runtime.new_internal_promise_function(
                        self.realm,
                        NativeFunctionId::ModuleEvaluation(kind),
                        1,
                        0,
                        InternalCallableData::ModuleEvaluation { module, kind },
                    )
                };
                let fulfill = make_handler(ModuleEvaluationKind::Fulfill)?;
                let reject = make_handler(ModuleEvaluationKind::Reject)?;
                let step = PromiseStep::module_then(runtime, self.realm, promise, fulfill, reject)?;
                self.phase = Phase::Attach;
                Ok(BodyStep::Promise {
                    step: Box::new(step),
                    resume: self,
                })
            }
        }
    }
}
fn inspect_sync(runtime: &Runtime, completion: Completion) -> Result<Completion, RuntimeError> {
    let promise = match completion {
        Completion::Return(value) => match value {
            JsValue::Object(promise) => ObjectRef::from_owned_handle(runtime.clone(), promise),
            value => {
                runtime.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "async module callable returned a non-Promise",
                ));
            }
        },
        Completion::Throw(value) => {
            runtime.release_jsvalue(value)?;
            return Err(RuntimeError::Invariant(
                "async module callable threw instead of returning a Promise",
            ));
        }
    };
    let snapshot = runtime
        .0
        .state
        .borrow()
        .heap
        .promise_snapshot(promise.object_id())?;
    let result = JsValue::from_raw(snapshot.result.clone()).ok_or(RuntimeError::Invariant(
        "module Promise result is an internal sentinel",
    ))?;
    match snapshot.state {
        PromiseState::Fulfilled => Ok(Completion::Return(runtime.dup_jsvalue(&result)?)),
        PromiseState::Rejected => Ok(Completion::Throw(runtime.dup_jsvalue(&result)?)),
        PromiseState::Pending => Err(RuntimeError::Invariant(
            "synchronous module body retained a pending Promise",
        )),
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<BodyStep>() <= 64);
