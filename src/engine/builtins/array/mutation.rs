//! Array endpoint mutations retain their copy cursor across observable property operations.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayPopKind, ArrayPushKind},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum MutationKind {
    Push(ArrayPushKind),
    Pop(ArrayPopKind),
}
impl MutationKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypePush(kind) => Some(Self::Push(kind)),
            NativeFunctionId::ArrayPrototypePop(kind) => Some(Self::Pop(kind)),
            _ => None,
        }
    }
}
pub(crate) enum MutationStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
    Number {
        value: Value,
        resume: MutationResume,
    },
    Copy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: MutationResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: MutationResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
}
enum Phase {
    Length,
    Number,
    Result,
    Copy,
    Write,
    DeleteLast,
    LengthWrite,
}
pub(crate) struct MutationResume {
    realm: ContextId,
    kind: MutationKind,
    object: ObjectRef,
    arguments: Vec<Value>,
    // Push never uses the Pop result slot. A single immediate argument lives
    // there until completion, avoiding a Vec allocation without moving roots.
    inline_argument: bool,
    phase: Phase,
    length: u64,
    new_length: u64,
    cursor: u64,
    result: Value,
}
impl MutationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array mutation requires generic invocation",
            ));
        };
        let values = &arguments.readable[..arguments.actual_arg_count];
        let inline = match (kind, values) {
            (
                MutationKind::Push(_),
                [
                    value @ (Value::Undefined
                    | Value::Null
                    | Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)),
                ],
            ) => Some(value.clone()),
            _ => None,
        };
        let values = if inline.is_some() {
            #[cfg(all(feature = "profiling", feature = "stack-vm"))]
            crate::engine::api::profiling::record_owned_execution_event(
                "array_mutation_inline_argument",
            );
            Vec::new()
        } else {
            values.to_vec()
        };
        Self::start_arguments(runtime, realm, kind, this_value.clone(), values, inline)
    }
    pub(crate) fn start_values(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        receiver: Value,
        arguments: Vec<Value>,
    ) -> Result<Self, RuntimeError> {
        Self::start_arguments(runtime, realm, kind, receiver, arguments, None)
    }
    fn start_arguments(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        receiver: Value,
        arguments: Vec<Value>,
        inline: Option<Value>,
    ) -> Result<Self, RuntimeError> {
        let object = match runtime.native_to_object(realm, receiver)? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: MutationResume {
                realm,
                kind,
                object,
                arguments,
                inline_argument: inline.is_some(),
                phase: Phase::Length,
                length: 0,
                new_length: 0,
                cursor: 0,
                result: inline.unwrap_or(Value::Undefined),
            },
        })
    }
}
impl MutationResume {
    fn argument_count(&self) -> usize {
        if self.inline_argument {
            1
        } else {
            self.arguments.len()
        }
    }
    fn argument(&self, index: usize) -> Option<&Value> {
        if self.inline_argument {
            (index == 0).then_some(&self.result)
        } else {
            self.arguments.get(index)
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<MutationStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(MutationStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Result => {
                self.result = value;
                self.copy_next(runtime)
            }
            Phase::Copy => self.copied(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array mutation received unexpected value reply",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<MutationStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array mutation number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(number) => Runtime::length_from_number(number),
            NativeConversion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.kind {
            MutationKind::Push(_) => {
                self.new_length = self.length.saturating_add(self.argument_count() as u64);
                if self.new_length > (1_u64 << 53) - 1 {
                    return Ok(MutationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "Array loo long",
                        )?,
                    )));
                }
                self.copy_next(runtime)
            }
            MutationKind::Pop(kind) => {
                self.new_length = self.length.saturating_sub(1);
                if self.length == 0 {
                    return self.write_length(runtime);
                }
                let index = if kind == ArrayPopKind::Shift {
                    0
                } else {
                    self.new_length
                };
                self.phase = Phase::Result;
                Ok(MutationStep::Read {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(index)?,
                    resume: self,
                })
            }
        }
    }
    fn copy_next(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        let (to, from, count, backwards) = match self.kind {
            MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => {
                (self.argument_count() as u64, 0, self.length, true)
            }
            MutationKind::Pop(ArrayPopKind::Shift) => (0, 1, self.new_length, false),
            _ => return self.copied(runtime),
        };
        self.phase = Phase::Copy;
        Ok(MutationStep::Copy {
            object: self.object.clone(),
            to,
            from,
            count,
            backwards,
            resume: self,
        })
    }
    fn copied(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        self.cursor = 0;
        match self.kind {
            MutationKind::Push(_) => self.write_next(runtime),
            MutationKind::Pop(_) => {
                self.phase = Phase::DeleteLast;
                Ok(MutationStep::Delete {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(self.new_length)?,
                    resume: self,
                })
            }
        }
    }
    fn write_next(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        if let Some(value) = self.argument(self.cursor as usize).cloned() {
            let from = match self.kind {
                MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => 0,
                _ => self.length,
            };
            self.phase = Phase::Write;
            return Ok(MutationStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(from + self.cursor)?,
                value,
                resume: self,
            });
        }
        let redundant = matches!(self.kind, MutationKind::Push(ArrayPushKind::Push))
            && self.new_length <= u64::from(u32::MAX)
            && matches!(runtime.array_length_state_if_genuine(&self.object)?, Some((length, true)) if u64::from(length) == self.new_length);
        if redundant {
            self.complete()
        } else {
            self.write_length(runtime)
        }
    }
    fn write_length(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        self.phase = Phase::LengthWrite;
        Ok(MutationStep::Set {
            object: self.object.clone(),
            key: runtime.intern_property_key("length")?,
            value: Value::number(self.new_length as f64),
            resume: self,
        })
    }
    fn complete(self) -> Result<MutationStep, RuntimeError> {
        Ok(MutationStep::Complete(Completion::Return(
            match self.kind {
                MutationKind::Push(_) => Value::number(self.new_length as f64),
                MutationKind::Pop(_) => self.result,
            },
        )))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<MutationStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::DeleteLast => {
                if !value {
                    return Ok(MutationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.write_length(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array mutation boolean phase mismatch",
            )),
        }
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<MutationStep, RuntimeError> {
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(MutationStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            Phase::Write => {
                self.cursor += 1;
                self.write_next(runtime)
            }
            Phase::LengthWrite => self.complete(),
            _ => Err(RuntimeError::Invariant("Array mutation set phase mismatch")),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: MutationStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            MutationStep::Complete(result) => return Ok(result),
            MutationStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            MutationStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            MutationStep::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => resume.resume(
                runtime,
                super::copy::finish(
                    runtime,
                    realm,
                    super::copy::CopyStep::start(
                        runtime, realm, object, to, from, count, backwards,
                    )?,
                )?,
            )?,
            MutationStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
            }
            MutationStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_inline_argument_uses_actual_count_and_retains_immediate_payload() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(runtime.new_array(context.realm).unwrap()),
        };
        #[cfg(all(feature = "profiling", feature = "stack-vm"))]
        let profile = crate::engine::api::profiling::CostProfile::start();
        for value in [
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(-0.0),
        ] {
            let arguments = NativeArguments {
                actual_arg_count: 1,
                readable: vec![value.clone(), Value::Undefined],
            };
            let MutationStep::Read { resume, .. } = MutationStep::start(
                &runtime,
                context.realm,
                MutationKind::Push(ArrayPushKind::Push),
                &invocation,
                &arguments,
            )
            .unwrap() else {
                panic!("expected length read");
            };
            assert!(resume.inline_argument);
            assert_eq!(resume.arguments.capacity(), 0);
            assert_eq!(resume.argument_count(), 1);
            assert!(
                resume
                    .argument(0)
                    .unwrap()
                    .same_quickjs_representation(&value)
            );
            assert!(resume.argument(1).is_none());
        }
        let arguments = NativeArguments {
            actual_arg_count: 0,
            readable: vec![Value::Undefined],
        };
        let MutationStep::Read { resume, .. } = MutationStep::start(
            &runtime,
            context.realm,
            MutationKind::Push(ArrayPushKind::Push),
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("expected length read");
        };
        assert!(!resume.inline_argument);
        assert_eq!(resume.argument_count(), 0);
        #[cfg(all(feature = "profiling", feature = "stack-vm"))]
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("array_mutation_inline_argument")
                .copied()
                .unwrap_or(0),
            5,
        );
    }

    #[test]
    fn push_inline_argument_preserves_observable_mutation_steps() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let value = context.eval(r#"(function () {
            var trace = '', stored;
            var target = {
                get length() { trace += 'g'; return { valueOf: function () { trace += 'n'; return 0; } }; },
                set length(v) { trace += 'l' + v; },
                set 0(v) { trace += 's'; stored = v; }
            };
            if (Array.prototype.push.call(target, -0) !== 1 ||
                trace !== 'gnsl1' || 1 / stored !== -Infinity) return false;
            var a = [2, 3];
            if (a.unshift(1) !== 3 || a.join(',') !== '1,2,3') return false;
            var object = {}, b = [];
            if (b.push(object) !== 1 || b[0] !== object) return false;
            if (b.push(4, 5) !== 3 || b[1] !== 4 || b[2] !== 5) return false;
            var frozen = Object.freeze([]), threw = false;
            try { frozen.push(1); } catch (e) { threw = e instanceof TypeError; }
            return threw && frozen.length === 0;
        })()"#).unwrap();
        assert!(matches!(value, Value::Bool(true)));
    }
}
