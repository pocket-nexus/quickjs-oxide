//! Date prototype operations retain the specified pre-coercion fields or re-read setYear state.
use super::date_input_fields;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        date::calendar::{DateFields, get_date_fields, set_date_fields, time_clip},
        native::{DateNativeKind, DateSetFieldKind},
    },
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{JsString, JsValue, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum DatePrototypeStep {
    Complete(Completion),
    Number {
        value: JsValue,
        resume: DatePrototypeResume,
    },
    Primitive {
        value: JsValue,
        hint: ToPrimitiveHint,
        resume: DatePrototypeResume,
    },
    OrdinaryPrimitive {
        object: ObjectRef,
        hint: ToPrimitiveHint,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: DatePrototypeResume,
    },
    Call {
        callable: CallableRef,
        receiver: JsValue,
    },
}
enum Phase {
    Time,
    Field {
        field: DateSetFieldKind,
        fields: DateFields,
        had_fields: bool,
        all_finite: bool,
    },
    Year,
    JsonPrimitive,
    JsonMethod,
}
pub(crate) struct DatePrototypeResume(Box<DatePrototypeResumeState>);
impl std::ops::Deref for DatePrototypeResume {
    type Target = DatePrototypeResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for DatePrototypeResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<DatePrototypeResume>() <= 8);
pub(crate) struct DatePrototypeResumeState {
    realm: ContextId,
    object: ObjectRef,
    phase: Phase,
    arguments: std::collections::VecDeque<JsValue>,
    reply: JsValue,
    converted: usize,
    actual: usize,
}
impl Drop for DatePrototypeResumeState {
    fn drop(&mut self) {
        for value in self.arguments.drain(..) {
            let _ = self.object.runtime().release_jsvalue(value);
        }
        let _ = self
            .object
            .runtime()
            .release_jsvalue(std::mem::replace(&mut self.reply, JsValue::Undefined));
    }
}
impl DatePrototypeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Date prototype requires generic invocation",
            ));
        };
        if kind == DateNativeKind::ToPrimitive {
            let JsValue::Object(id) = this_value else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?,
                )));
            };
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
            let hint = match arguments.readable.first() {
                Some(JsValue::String(id)) => {
                    let state = runtime.0.state.borrow();
                    let value = state.heap.string(*id)?;
                    if *value == JsString::from_static("number")
                        || *value == JsString::from_static("integer")
                    {
                        Some(ToPrimitiveHint::Number)
                    } else if *value == JsString::from_static("string")
                        || *value == JsString::from_static("default")
                    {
                        Some(ToPrimitiveHint::String)
                    } else {
                        None
                    }
                }
                Some(_) => None,
                None => {
                    return Err(RuntimeError::Invariant(
                        "Date native argument vector was not padded to readable arity",
                    ));
                }
            };
            let Some(hint) = hint else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "invalid hint",
                    )?,
                )));
            };
            return Ok(Self::OrdinaryPrimitive {
                object: object.clone(),
                hint,
            });
        }
        if kind == DateNativeKind::ToJson {
            let object =
                match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(this_value)?)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                };
            return Ok(Self::Primitive {
                value: JsValue::Object(object.clone().into_handle()),
                hint: ToPrimitiveHint::Number,
                resume: DatePrototypeResume(Box::new(DatePrototypeResumeState {
                    realm,
                    object,
                    phase: Phase::JsonPrimitive,
                    arguments: std::collections::VecDeque::new(),
                    reply: JsValue::Undefined,
                    converted: 0,
                    actual: 0,
                })),
            });
        }
        let (object, value) = match runtime.date_this_time_value_jsvalue(realm, this_value)? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
        };
        let (phase, count) = match kind {
            DateNativeKind::SetTime => (Phase::Time, 1),
            DateNativeKind::SetYear => (Phase::Year, 1),
            DateNativeKind::SetField(field) => {
                let first = usize::from(field.first_field());
                let end = usize::from(field.end_field());
                let fields =
                    get_date_fields(value, field.uses_local_time(), first == 0, |instant| {
                        runtime.date_timezone_offset_minutes(instant)
                    });
                let had_fields = fields.is_some();
                (
                    Phase::Field {
                        field,
                        fields: fields.unwrap_or([0.0; 9]),
                        had_fields,
                        all_finite: had_fields,
                    },
                    arguments.actual_arg_count.min(end.saturating_sub(first)),
                )
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "pure Date method reached callback operation",
                ));
            }
        };
        let mut resume = DatePrototypeResume(Box::new(DatePrototypeResumeState {
            realm,
            object: object.clone(),
            phase,
            arguments: std::collections::VecDeque::new(),
            reply: JsValue::Undefined,
            converted: 0,
            actual: arguments.actual_arg_count,
        }));
        resume
            .arguments
            .try_reserve(count)
            .map_err(|_| RuntimeError::Invariant("Date setter argv allocation failed"))?;
        for value in arguments
            .readable
            .get(..count)
            .ok_or(RuntimeError::Invariant("Date setter argv was not padded"))?
        {
            resume.arguments.push_back(runtime.dup_jsvalue(value)?);
        }
        resume.next(runtime)
    }
}
impl DatePrototypeResume {
    fn next(mut self, runtime: &Runtime) -> Result<DatePrototypeStep, RuntimeError> {
        if let Some(value) = self.0.arguments.pop_front() {
            return Ok(DatePrototypeStep::Number {
                value,
                resume: self,
            });
        }
        let Phase::Field {
            field,
            fields,
            had_fields,
            all_finite,
        } = self.0.phase
        else {
            return Err(RuntimeError::Invariant(
                "Date setter numeric result missing",
            ));
        };
        if !had_fields {
            return Ok(DatePrototypeStep::Complete(Completion::Return(
                crate::engine::value::number::operations::Number::compact(f64::NAN).into(),
            )));
        }
        let value = if all_finite && self.0.actual > 0 {
            set_date_fields(
                &date_input_fields(&fields),
                field.uses_local_time(),
                |instant| runtime.date_timezone_offset_minutes(instant),
            )
        } else {
            f64::NAN
        };
        Ok(DatePrototypeStep::Complete(
            runtime.set_date_this_time_value(&self.0.object, value)?,
        ))
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(DatePrototypeStep::Complete(Completion::Throw(value)));
            }
        };
        match &mut self.0.phase {
            Phase::Time => Ok(DatePrototypeStep::Complete(
                runtime.set_date_this_time_value(&self.0.object, time_clip(value))?,
            )),
            Phase::Year => Ok(DatePrototypeStep::Complete(
                runtime.finish_date_set_year(&self.0.object, value)?,
            )),
            Phase::Field {
                field,
                fields,
                all_finite,
                ..
            } => {
                if !value.is_finite() {
                    *all_finite = false;
                }
                fields[usize::from(field.first_field()) + self.0.converted] = value.trunc();
                self.0.converted += 1;
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant("Date setter number phase mismatch")),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<DatePrototypeStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(DatePrototypeStep::Complete(Completion::Throw(value)));
            }
        };
        let previous = std::mem::replace(&mut self.reply, value);
        runtime.release_jsvalue(previous)?;
        match self.0.phase {
            Phase::JsonPrimitive => {
                if matches!(self.reply, JsValue::Float(value) if !value.is_finite()) {
                    return Ok(DatePrototypeStep::Complete(Completion::Return(
                        JsValue::Null,
                    )));
                }
                self.0.phase = Phase::JsonMethod;
                Ok(DatePrototypeStep::Read {
                    object: self.0.object.clone(),
                    key: runtime.pinned_property_key(
                        crate::engine::atom::pinned::PinnedAtom::ToISOString,
                    )?,
                    resume: self,
                })
            }
            Phase::JsonMethod => {
                let callable = match &self.reply {
                    JsValue::Object(id) => runtime.as_callable_object(*id)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(DatePrototypeStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "object needs toISOString method",
                        )?,
                    )));
                };
                Ok(DatePrototypeStep::Call {
                    callable,
                    receiver: JsValue::Object(self.0.object.clone().into_handle()),
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Date prototype value phase mismatch",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: DatePrototypeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            DatePrototypeStep::Complete(result) => return Ok(result),
            DatePrototypeStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
            DatePrototypeStep::Primitive {
                value,
                hint,
                resume,
            } => resume.resume(runtime, runtime.to_primitive_jsvalue(realm, value, hint)?)?,
            DatePrototypeStep::OrdinaryPrimitive { object, hint } => {
                return runtime.ordinary_to_primitive(realm, &object, hint);
            }
            DatePrototypeStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            DatePrototypeStep::Call { callable, receiver } => {
                return runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new());
            }
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<DatePrototypeStep>() <= 64);
