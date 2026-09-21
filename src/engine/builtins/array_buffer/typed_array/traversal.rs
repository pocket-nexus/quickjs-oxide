//! TypedArray callbacks own their traversal and reacquire each live element.

use crate::engine::builtins::native::{NativeFunctionId, TypedArrayNativeKind};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayFindKind, ArrayReduceKind},
    heap::ContextId,
    object::{CallableRef, ObjectRef},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{DirectCallTarget, NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum TypedTraversalKind {
    Find(ArrayFindKind),
    Reduce(ArrayReduceKind),
}
impl TypedTraversalKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::TypedArray(TypedArrayNativeKind::Find(kind)) => Self::Find(kind),
            NativeFunctionId::TypedArray(TypedArrayNativeKind::Reduce(kind)) => Self::Reduce(kind),
            _ => return None,
        })
    }
}
pub(crate) enum TypedTraversalStep {
    Complete(Completion),
    Call { resume: TypedTraversalResume },
}
pub(crate) struct TypedTraversalResume(Box<TypedTraversalResumeState>);
impl std::ops::Deref for TypedTraversalResume {
    type Target = TypedTraversalResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for TypedTraversalResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<TypedTraversalResume>() <= 8);
pub(crate) struct TypedTraversalResumeState {
    pending_effect: TypedTraversalStepPending,
    state: TraversalState,
    phase: TraversalPhase,
}
struct TraversalState {
    realm: ContextId,
    kind: TypedTraversalKind,
    target: ObjectRef,
    callback: CallableRef,
    this_arg: JsValue,
    held_value: JsValue,
    length: u64,
    step: u64,
}
enum TraversalPhase {
    Find { index: u64 },
    Reduce,
}
impl TypedTraversalStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: TypedTraversalKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "TypedArray traversal received a constructor invocation",
            ));
        };
        let target = match runtime.require_typed_array_jsvalue(realm, this_value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let length = match runtime.typed_array_validated_length(realm, &target)? {
            NativeConversion::Value(value) => u64::from(value),
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let callback_value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "TypedArray traversal callback argv was not padded",
        ))?;
        let callback = if let JsValue::Object(id) = callback_value {
            runtime.as_callable_object(*id)?
        } else {
            None
        }
        .ok_or_else(|| {
            RuntimeError::Engine(crate::engine::api::Error::new(
                crate::engine::api::ErrorKind::Type,
                "not a function",
            ))
        })?;
        let mut state = TraversalState {
            realm,
            kind,
            target,
            callback,
            this_arg: JsValue::Undefined,
            held_value: JsValue::Undefined,
            length,
            step: 0,
        };
        if arguments.actual_arg_count > 1 && matches!(kind, TypedTraversalKind::Find(_)) {
            state.this_arg = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                RuntimeError::Invariant("TypedArray traversal second argument was missing"),
            )?)?;
        }
        match kind {
            TypedTraversalKind::Find(_) => state.find(runtime),
            TypedTraversalKind::Reduce(_) => {
                let accumulator = if arguments.actual_arg_count > 1 {
                    runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
                        RuntimeError::Invariant("TypedArray traversal second argument was missing"),
                    )?)?
                } else {
                    if length == 0 {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.new_native_error_jsvalue(
                                realm,
                                NativeErrorKind::Type,
                                "empty array",
                            )?,
                        )));
                    }
                    let index = state.index();
                    state.step += 1;
                    runtime
                        .typed_array_read_index_jsvalue(&state.target, index)?
                        .unwrap_or(JsValue::Undefined)
                };
                state.reduce(runtime, accumulator)
            }
        }
    }
}
impl Drop for TraversalState {
    fn drop(&mut self) {
        let runtime = self.target.runtime();
        let _ = runtime.release_jsvalue(std::mem::replace(&mut self.this_arg, JsValue::Undefined));
        let _ =
            runtime.release_jsvalue(std::mem::replace(&mut self.held_value, JsValue::Undefined));
    }
}
impl TraversalState {
    fn index(&self) -> u64 {
        match self.kind {
            TypedTraversalKind::Find(ArrayFindKind::FindLast | ArrayFindKind::FindLastIndex)
            | TypedTraversalKind::Reduce(ArrayReduceKind::ReduceRight) => {
                self.length - self.step - 1
            }
            _ => self.step,
        }
    }
    fn find(mut self, runtime: &Runtime) -> Result<TypedTraversalStep, RuntimeError> {
        if self.step == self.length {
            let value = match self.kind {
                TypedTraversalKind::Find(ArrayFindKind::Find | ArrayFindKind::FindLast) => {
                    JsValue::Undefined
                }
                TypedTraversalKind::Find(_) => JsValue::Int(-1),
                _ => return Err(RuntimeError::Invariant("TypedArray find lost its selector")),
            };
            return Ok(TypedTraversalStep::Complete(Completion::Return(value)));
        }
        let index = self.index();
        self.step += 1;
        self.held_value = runtime
            .typed_array_read_index_jsvalue(&self.target, index)?
            .unwrap_or(JsValue::Undefined);
        let mut resume = TypedTraversalResume(Box::new(TypedTraversalResumeState {
            pending_effect: TypedTraversalStepPending::new(runtime),
            state: self,
            phase: TraversalPhase::Find { index },
        }));
        if !resume.reserve_arguments(3) {
            return Ok(TypedTraversalStep::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    resume.0.state.realm,
                    NativeErrorKind::Internal,
                    "out of memory",
                )?,
            )));
        }
        resume.0.pending_effect.call_receiver =
            Some(runtime.dup_jsvalue(&resume.0.state.this_arg)?);
        let value = runtime.dup_jsvalue(&resume.0.state.held_value)?;
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(value);
        resume.fill_arguments(index);
        Ok(TypedTraversalStep::Call { resume })
    }
    fn reduce(
        mut self,
        runtime: &Runtime,
        accumulator: JsValue,
    ) -> Result<TypedTraversalStep, RuntimeError> {
        runtime.release_jsvalue(std::mem::replace(&mut self.held_value, accumulator))?;
        if self.step == self.length {
            return Ok(TypedTraversalStep::Complete(Completion::Return(
                std::mem::replace(&mut self.held_value, JsValue::Undefined),
            )));
        }
        let index = self.index();
        self.step += 1;
        let mut resume = TypedTraversalResume(Box::new(TypedTraversalResumeState {
            pending_effect: TypedTraversalStepPending::new(runtime),
            state: self,
            phase: TraversalPhase::Reduce,
        }));
        if !resume.reserve_arguments(4) {
            return Ok(TypedTraversalStep::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    resume.0.state.realm,
                    NativeErrorKind::Internal,
                    "out of memory",
                )?,
            )));
        }
        let accumulator = std::mem::replace(&mut resume.0.state.held_value, JsValue::Undefined);
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(accumulator);
        let value = runtime
            .typed_array_read_index_jsvalue(&resume.0.state.target, index)?
            .unwrap_or(JsValue::Undefined);
        resume
            .0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .push(value);
        resume.0.pending_effect.call_receiver = Some(JsValue::Undefined);
        resume.fill_arguments(index);
        Ok(TypedTraversalStep::Call { resume })
    }
}
impl TypedTraversalResume {
    fn reserve_arguments(&mut self, count: usize) -> bool {
        self.0.pending_effect.call_arguments = Some(Vec::new());
        self.0
            .pending_effect
            .call_arguments
            .as_mut()
            .unwrap()
            .try_reserve_exact(count)
            .is_ok()
    }
    fn fill_arguments(&mut self, index: u64) {
        let arguments = self.0.pending_effect.call_arguments.as_mut().unwrap();
        arguments
            .push(crate::engine::value::number::operations::Number::compact(index as f64).into());
        arguments.push(JsValue::Object(self.0.state.target.clone().into_handle()));
        self.0.pending_effect.call_target =
            Some(DirectCallTarget::Callable(self.0.state.callback.clone()));
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedTraversalStep, RuntimeError> {
        let result = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedTraversalStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            TraversalPhase::Find { index } => {
                let truth = runtime.value_to_boolean_jsvalue(&result);
                runtime.release_jsvalue(result)?;
                if truth? {
                    let found = match self.0.state.kind {
                        TypedTraversalKind::Find(ArrayFindKind::Find | ArrayFindKind::FindLast) => {
                            std::mem::replace(&mut self.0.state.held_value, JsValue::Undefined)
                        }
                        TypedTraversalKind::Find(_) => {
                            crate::engine::value::number::operations::Number::compact(index as f64)
                                .into()
                        }
                        _ => {
                            return Err(RuntimeError::Invariant(
                                "TypedArray find reply lost its selector",
                            ));
                        }
                    };
                    Ok(TypedTraversalStep::Complete(Completion::Return(found)))
                } else {
                    runtime.release_jsvalue(std::mem::replace(
                        &mut self.0.state.held_value,
                        JsValue::Undefined,
                    ))?;
                    self.0.state.find(runtime)
                }
            }
            TraversalPhase::Reduce => self.0.state.reduce(runtime, result),
        }
    }
}

pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedTraversalStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            TypedTraversalStep::Complete(result) => return Ok(result),
            TypedTraversalStep::Call { mut resume } => {
                let target = resume.take_call_target();
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray traversal requested an invalid call target",
                    ));
                };
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
        };
    }
}

struct TypedTraversalStepPending {
    runtime: Runtime,
    call_target: Option<DirectCallTarget>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
}
impl TypedTraversalStepPending {
    fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            call_target: None,
            call_receiver: None,
            call_arguments: None,
        }
    }

    /// Release the internal edges still owned when the request is abandoned
    /// before its step consumed them. Taken fields are empty here.
    fn release_owned(&mut self) {
        if let Some(receiver) = self.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(receiver);
        }
        for argument in self.call_arguments.take().into_iter().flatten() {
            let _ = self.runtime.release_jsvalue(argument);
        }
    }
}
impl Drop for TypedTraversalStepPending {
    fn drop(&mut self) {
        self.release_owned();
    }
}
impl TypedTraversalResume {
    pub(crate) fn take_call_target(&mut self) -> DirectCallTarget {
        self.0
            .pending_effect
            .call_target
            .take()
            .expect("TypedTraversalStep Call target")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("TypedTraversalStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("TypedTraversalStep Call arguments")
    }
}
const _: () = assert!(std::mem::size_of::<TypedTraversalStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<TypedTraversalStep>() <= 64);
