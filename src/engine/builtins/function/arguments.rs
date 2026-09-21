//! QuickJS build_arg_list: owned carrier, fixed length and ordered indexed reads.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError},
    object::{ObjectRef, PropertyKey},
    value::{JsValue, Value, conversion::NativeConversion},
    vm::Completion,
};

const MAX_APPLY_ARGUMENTS: u64 = 65_534;

pub(crate) enum ArgumentsStep {
    Complete(NativeConversion<Vec<JsValue>>),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ArgumentsResume,
    },
    Number {
        resume: ArgumentsResume,
    },
}
pub(crate) struct ArgumentsResume(Box<ArgumentsResumeState>);
impl std::ops::Deref for ArgumentsResume {
    type Target = ArgumentsResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ArgumentsResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ArgumentsResume>() <= 8);
pub(crate) struct ArgumentsResumeState {
    realm: ContextId,
    carrier: ObjectRef,
    number_value: Option<JsValue>,
    phase: Phase,
}
impl Drop for ArgumentsResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.number_value.take() {
            let _ = self.carrier.runtime().release_jsvalue(value);
        }
        if let Phase::Item { values, .. } = &mut self.phase {
            for value in values.drain(..) {
                let _ = self.carrier.runtime().release_jsvalue(value);
            }
        }
    }
}
enum Phase {
    Length,
    Number,
    Item { length: usize, values: Vec<JsValue> },
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
        if let Some(result) = runtime.prepare_fast_array_arguments_jsvalue(realm, &carrier)? {
            return Ok(Self::Complete(result));
        }
        Ok(Self::Read {
            object: carrier.clone(),
            key: runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?,
            resume: ArgumentsResume(Box::new(ArgumentsResumeState {
                realm,
                carrier,
                number_value: None,
                phase: Phase::Length,
            })),
        })
    }
}
impl ArgumentsResume {
    pub(crate) fn take_number_value(&mut self) -> JsValue {
        self.0.number_value.take().expect("arguments number")
    }

    pub(crate) fn read(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ArgumentsStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ArgumentsStep::Complete(NativeConversion::Throw(
                    runtime.root_and_release_jsvalue(value)?,
                )));
            }
        };
        match std::mem::replace(&mut self.0.phase, Phase::Number) {
            Phase::Length => {
                self.0.phase = Phase::Number;
                self.0.number_value = Some(value);
                Ok(ArgumentsStep::Number { resume: self })
            }
            Phase::Item { length, mut values } => {
                values.push(value);
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_call_buffer_moves("apply.indexed", 1);
                self.next(runtime, length, values)
            }
            Phase::Number => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "argument length received an untyped reply",
                ))
            }
        }
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Number) {
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
                    self.0.realm,
                    NativeErrorKind::Range,
                    "too many arguments in function call (only 65534 allowed)",
                )?,
            )));
        }
        let length = usize::try_from(length)
            .map_err(|_| RuntimeError::Invariant("argument-list length does not fit usize"))?;
        if let Some(values) =
            runtime.fast_array_like_values_jsvalue(&self.0.carrier, length as u32)?
        {
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
            size_of::<JsValue>(),
        );
        self.next(runtime, length, values)
    }
    fn next(
        mut self,
        runtime: &Runtime,
        length: usize,
        values: Vec<JsValue>,
    ) -> Result<ArgumentsStep, RuntimeError> {
        if values.len() == length {
            return Ok(ArgumentsStep::Complete(NativeConversion::Value(values)));
        }
        let index = values.len();
        self.0.phase = Phase::Item { length, values };
        let key = runtime.property_key_for_index(index as u64)?;
        Ok(ArgumentsStep::Read {
            object: self.0.carrier.clone(),
            key,
            resume: self,
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ArgumentsStep,
) -> Result<NativeConversion<Vec<JsValue>>, RuntimeError> {
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
            ArgumentsStep::Number { mut resume } => {
                let value = runtime.root_and_release_jsvalue(resume.take_number_value())?;
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ArgumentsStep>() <= 64);
