//! Date construction owns converted fields before resolving the result prototype.
use super::{DEFAULT_DATE_FIELDS, MAX_DATE_ARGUMENTS, parsed_date_value};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        date::{
            calendar::{DateInputFields, set_date_fields_checked, time_clip},
            parse::parse_date_string,
        },
        native::DateNativeKind,
    },
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum DateConstructorStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: DateConstructorResume,
    },
    String {
        value: Value,
        resume: DateConstructorResume,
    },
    Number {
        value: Value,
        resume: DateConstructorResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: DateConstructorResume,
    },
}
enum Phase {
    Single,
    Fields,
    Prototype,
    Parse,
}
pub(crate) struct DateConstructorResume {
    realm: ContextId,
    kind: DateNativeKind,
    new_target: Value,
    arguments: std::vec::IntoIter<Value>,
    fields: DateInputFields,
    index: usize,
    value: f64,
    phase: Phase,
}
impl DateConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let new_target = match (kind, invocation) {
            (DateNativeKind::Constructor, NativeInvocation::Construct { new_target }) => {
                new_target.clone()
            }
            (
                DateNativeKind::Now | DateNativeKind::Parse | DateNativeKind::Utc,
                NativeInvocation::Call { .. },
            ) => Value::Undefined,
            _ => {
                return Err(RuntimeError::Invariant(
                    "Date constructor/static invocation mismatch",
                ));
            }
        };
        if kind == DateNativeKind::Now {
            return Ok(Self::Complete(runtime.call_date_now()?));
        }
        if kind == DateNativeKind::Constructor && matches!(new_target, Value::Undefined) {
            return Ok(Self::Complete(runtime.call_date_as_function()?));
        }
        let count = arguments.actual_arg_count.min(MAX_DATE_ARGUMENTS);
        let values = arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Date actual arguments unreadable"))?
            .to_vec();
        let mut resume = DateConstructorResume {
            realm,
            kind,
            new_target,
            arguments: values.into_iter(),
            fields: DEFAULT_DATE_FIELDS,
            index: 0,
            value: f64::NAN,
            phase: Phase::Fields,
        };
        if kind == DateNativeKind::Parse {
            resume.phase = Phase::Parse;
            return Ok(Self::String {
                value: arguments
                    .readable
                    .first()
                    .cloned()
                    .unwrap_or(Value::Undefined),
                resume,
            });
        }
        if arguments.actual_arg_count == 0 {
            if kind == DateNativeKind::Utc {
                return Ok(Self::Complete(Completion::Return(Value::Float(f64::NAN))));
            }
            resume.value = runtime.date_now_millis() as f64;
            return resume.prototype(runtime);
        }
        if kind == DateNativeKind::Constructor && arguments.actual_arg_count == 1 {
            let value = resume
                .arguments
                .next()
                .ok_or(RuntimeError::Invariant("Date sole argument missing"))?;
            if let Some(value) = runtime.genuine_date_value(&value)? {
                resume.value = time_clip(value);
                return resume.prototype(runtime);
            }
            resume.phase = Phase::Single;
            return Ok(Self::Primitive { value, resume });
        }
        resume.fields(runtime)
    }
}
impl DateConstructorResume {
    pub(crate) fn primitive(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Single) {
            return Err(RuntimeError::Invariant("Date primitive phase mismatch"));
        }
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        self.value = if let Value::String(string) = value {
            parsed_date_value(parse_date_string(&string), |instant| {
                runtime.date_timezone_offset_minutes(instant)
            })
        } else {
            match runtime.number_from_primitive(self.realm, &value)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
                }
            }
        };
        self.value = time_clip(self.value);
        self.prototype(runtime)
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Parse) {
            return Err(RuntimeError::Invariant("Date parse phase mismatch"));
        }
        let string = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        Ok(DateConstructorStep::Complete(Completion::Return(
            Value::number(parsed_date_value(parse_date_string(&string), |instant| {
                runtime.date_timezone_offset_minutes(instant)
            })),
        )))
    }
    fn fields(mut self, runtime: &Runtime) -> Result<DateConstructorStep, RuntimeError> {
        if let Some(value) = self.arguments.next() {
            return Ok(DateConstructorStep::Number {
                value,
                resume: self,
            });
        }
        self.value = set_date_fields_checked(
            self.fields,
            self.kind == DateNativeKind::Constructor,
            |instant| runtime.date_timezone_offset_minutes(instant),
        );
        if self.kind == DateNativeKind::Utc {
            return Ok(DateConstructorStep::Complete(Completion::Return(
                Value::number(self.value),
            )));
        }
        self.prototype(runtime)
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Fields) {
            return Err(RuntimeError::Invariant("Date numeric field phase mismatch"));
        }
        self.fields[self.index] = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        self.index += 1;
        self.fields(runtime)
    }
    fn prototype(mut self, runtime: &Runtime) -> Result<DateConstructorStep, RuntimeError> {
        self.phase = Phase::Prototype;
        Ok(DateConstructorStep::Read {
            receiver: self.new_target.clone(),
            key: runtime.intern_property_key("prototype")?,
            resume: self,
        })
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DateConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Prototype) {
            return Err(RuntimeError::Invariant(
                "Date prototype lookup phase mismatch",
            ));
        }
        let prototype = match result {
            Completion::Return(Value::Object(object)) => object,
            Completion::Throw(value) => {
                return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(_) => {
                let realm = match runtime.function_realm_from_value(self.realm, &self.new_target)? {
                    NativeConversion::Value(realm) => realm,
                    NativeConversion::Throw(value) => {
                        return Ok(DateConstructorStep::Complete(Completion::Throw(value)));
                    }
                };
                let prototype = runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .context(realm)?
                    .date_prototype
                    .ok_or(RuntimeError::Invariant("realm has no Date prototype"))?;
                ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
            }
        };
        Ok(DateConstructorStep::Complete(Completion::Return(
            Value::Object(runtime.new_date_object(&prototype, self.value)?),
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: DateConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            DateConstructorStep::Complete(result) => return Ok(result),
            DateConstructorStep::Primitive { value, resume } => resume.primitive(
                runtime,
                runtime.to_primitive(realm, value, crate::engine::vm::ToPrimitiveHint::Default)?,
            )?,
            DateConstructorStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            DateConstructorStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            DateConstructorStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
        };
    }
}
