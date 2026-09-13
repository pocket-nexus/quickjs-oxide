//! TypedArray callbacks own their traversal and reacquire each live element.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::{NativeFunctionId, TypedArrayNativeKind};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayFindKind, ArrayReduceKind},
    heap::ContextId,
    object::{CallableRef, ObjectRef},
    value::{Value, conversion::NativeConversion},
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
    #[cfg(feature = "stack-vm")]
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
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: TypedTraversalResume,
    },
}
pub(crate) struct TypedTraversalResume {
    state: TraversalState,
    phase: TraversalPhase,
}
struct TraversalState {
    realm: ContextId,
    kind: TypedTraversalKind,
    target: ObjectRef,
    callback: CallableRef,
    this_arg: Value,
    length: u64,
    step: u64,
}
enum TraversalPhase {
    Find { value: Value, index: u64 },
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
        let target = match runtime.require_typed_array(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let length = match runtime.typed_array_validated_length(realm, &target)? {
            NativeConversion::Value(value) => u64::from(value),
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let callback = runtime.callable_from_value(
            arguments
                .readable
                .first()
                .ok_or(RuntimeError::Invariant(
                    "TypedArray traversal callback argv was not padded",
                ))?
                .clone(),
        )?;
        let second = if arguments.actual_arg_count > 1 {
            arguments
                .readable
                .get(1)
                .ok_or(RuntimeError::Invariant(
                    "TypedArray traversal second argument was missing",
                ))?
                .clone()
        } else {
            Value::Undefined
        };
        let this_arg = if matches!(kind, TypedTraversalKind::Find(_)) {
            second.clone()
        } else {
            Value::Undefined
        };
        let mut state = TraversalState {
            realm,
            kind,
            target,
            callback,
            this_arg,
            length,
            step: 0,
        };
        match kind {
            TypedTraversalKind::Find(_) => state.find(runtime),
            TypedTraversalKind::Reduce(_) => {
                let accumulator = if arguments.actual_arg_count > 1 {
                    second
                } else {
                    if length == 0 {
                        return Ok(Self::Complete(Completion::Throw(
                            runtime.new_native_error(
                                realm,
                                NativeErrorKind::Type,
                                "empty array",
                            )?,
                        )));
                    }
                    let index = state.index();
                    state.step += 1;
                    runtime
                        .typed_array_read_index(&state.target, index)?
                        .unwrap_or(Value::Undefined)
                };
                state.reduce(runtime, accumulator)
            }
        }
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
    fn arguments(
        &self,
        runtime: &Runtime,
        count: usize,
    ) -> Result<NativeConversion<Vec<Value>>, RuntimeError> {
        let mut arguments = Vec::new();
        if arguments.try_reserve_exact(count).is_err() {
            return Ok(NativeConversion::Throw(runtime.new_native_error(
                self.realm,
                NativeErrorKind::Internal,
                "out of memory",
            )?));
        }
        Ok(NativeConversion::Value(arguments))
    }
    fn find(mut self, runtime: &Runtime) -> Result<TypedTraversalStep, RuntimeError> {
        if self.step == self.length {
            return Ok(TypedTraversalStep::Complete(Completion::Return(
                match self.kind {
                    TypedTraversalKind::Find(ArrayFindKind::Find | ArrayFindKind::FindLast) => {
                        Value::Undefined
                    }
                    TypedTraversalKind::Find(_) => Value::Int(-1),
                    _ => return Err(RuntimeError::Invariant("TypedArray find lost its selector")),
                },
            )));
        }
        let index = self.index();
        self.step += 1;
        let value = runtime
            .typed_array_read_index(&self.target, index)?
            .unwrap_or(Value::Undefined);
        let mut arguments = match self.arguments(runtime, 3)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedTraversalStep::Complete(Completion::Throw(value)));
            }
        };
        arguments.push(value.clone());
        arguments.push(Value::number(index as f64));
        arguments.push(Value::Object(self.target.clone()));
        Ok(TypedTraversalStep::Call {
            target: DirectCallTarget::Callable(self.callback.clone()),
            receiver: self.this_arg.clone(),
            arguments,
            resume: TypedTraversalResume {
                state: self,
                phase: TraversalPhase::Find { value, index },
            },
        })
    }
    fn reduce(
        mut self,
        runtime: &Runtime,
        accumulator: Value,
    ) -> Result<TypedTraversalStep, RuntimeError> {
        if self.step == self.length {
            return Ok(TypedTraversalStep::Complete(Completion::Return(
                accumulator,
            )));
        }
        let index = self.index();
        self.step += 1;
        let value = runtime
            .typed_array_read_index(&self.target, index)?
            .unwrap_or(Value::Undefined);
        let mut arguments = match self.arguments(runtime, 4)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(TypedTraversalStep::Complete(Completion::Throw(value)));
            }
        };
        arguments.push(accumulator);
        arguments.push(value);
        arguments.push(Value::number(index as f64));
        arguments.push(Value::Object(self.target.clone()));
        Ok(TypedTraversalStep::Call {
            target: DirectCallTarget::Callable(self.callback.clone()),
            receiver: Value::Undefined,
            arguments,
            resume: TypedTraversalResume {
                state: self,
                phase: TraversalPhase::Reduce,
            },
        })
    }
}
impl TypedTraversalResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<TypedTraversalStep, RuntimeError> {
        let result = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedTraversalStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            TraversalPhase::Find { value, index } => {
                if runtime.value_to_boolean(&result)? {
                    Ok(TypedTraversalStep::Complete(Completion::Return(
                        match self.state.kind {
                            TypedTraversalKind::Find(
                                ArrayFindKind::Find | ArrayFindKind::FindLast,
                            ) => value,
                            TypedTraversalKind::Find(_) => Value::number(index as f64),
                            _ => {
                                return Err(RuntimeError::Invariant(
                                    "TypedArray find reply lost its selector",
                                ));
                            }
                        },
                    )))
                } else {
                    self.state.find(runtime)
                }
            }
            TraversalPhase::Reduce => self.state.reduce(runtime, result),
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
            TypedTraversalStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "TypedArray traversal requested an invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
        };
    }
}
