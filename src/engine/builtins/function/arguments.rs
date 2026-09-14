//! QuickJS build_arg_list: owned carrier, fixed length and ordered indexed reads.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError},
    object::{ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::Completion,
};

const MAX_APPLY_ARGUMENTS: u64 = 65_534;

pub(crate) enum ArgumentsStep {
    Complete(NativeConversion<Vec<Value>>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ArgumentsResume,
    },
    Number {
        value: Value,
        resume: ArgumentsResume,
    },
}
pub(crate) struct ArgumentsResume {
    realm: ContextId,
    carrier: ObjectRef,
    phase: Phase,
}
enum Phase {
    Length,
    Number,
    Item { length: usize, values: Vec<Value> },
}
impl ArgumentsStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        let Value::Object(carrier) = value else {
            return Ok(Self::Complete(NativeConversion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not a object")?,
            )));
        };
        if let Some(result) = runtime.prepare_fast_array_arguments(realm, &carrier)? {
            return Ok(Self::Complete(result));
        }
        Ok(Self::Read {
            object: carrier.clone(),
            key: runtime.intern_property_key("length")?,
            resume: ArgumentsResume {
                realm,
                carrier,
                phase: Phase::Length,
            },
        })
    }
}
impl ArgumentsResume {
    pub(crate) fn read(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ArgumentsStep::Complete(NativeConversion::Throw(value)));
            }
        };
        match std::mem::replace(&mut self.phase, Phase::Number) {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(ArgumentsStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Item { length, mut values } => {
                values.push(value);
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_call_buffer_moves("apply.indexed", 1);
                self.next(runtime, length, values)
            }
            Phase::Number => Err(RuntimeError::Invariant(
                "argument length received an untyped reply",
            )),
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "argument number reply has no length phase",
            ));
        }
        let number = match result {
            NativeConversion::Value(number) => number,
            NativeConversion::Throw(value) => {
                return Ok(ArgumentsStep::Complete(NativeConversion::Throw(value)));
            }
        };
        let length = Runtime::length_from_number(number);
        if length > MAX_APPLY_ARGUMENTS {
            return Ok(ArgumentsStep::Complete(NativeConversion::Throw(
                runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Range,
                    "too many arguments in function call (only 65534 allowed)",
                )?,
            )));
        }
        let length = usize::try_from(length)
            .map_err(|_| RuntimeError::Invariant("argument-list length does not fit usize"))?;
        if let Some(values) = runtime.fast_array_like_values(&self.carrier, length as u32)? {
            return Ok(ArgumentsStep::Complete(NativeConversion::Value(values)));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| HeapError::Allocation {
                operation: "reserving the argument-list snapshot",
            })?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_call_buffer_capacity(
            "apply.indexed",
            0,
            values.capacity(),
            size_of::<Value>(),
        );
        self.next(runtime, length, values)
    }
    fn next(
        mut self,
        runtime: &Runtime,
        length: usize,
        values: Vec<Value>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        if values.len() == length {
            return Ok(ArgumentsStep::Complete(NativeConversion::Value(values)));
        }
        let key = runtime.intern_property_key(&values.len().to_string())?;
        self.phase = Phase::Item { length, values };
        Ok(ArgumentsStep::Read {
            object: self.carrier.clone(),
            key,
            resume: self,
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ArgumentsStep,
) -> Result<NativeConversion<Vec<Value>>, RuntimeError> {
    loop {
        step = match step {
            ArgumentsStep::Complete(result) => return Ok(result),
            ArgumentsStep::Read {
                object,
                key,
                resume,
            } => resume.read(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ArgumentsStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}
