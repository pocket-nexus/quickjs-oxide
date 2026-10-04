//! Pinned QuickJS 2026-06-04 `Date.prototype` native handlers.
//!
//! The prototype object itself is deliberately ordinary. Every branded method
//! therefore checks for a genuine `ObjectPayload::Date` receiver instead of
//! accepting the realm's `Date.prototype` object.

use super::calendar::{DateFields, DateInputFields, get_date_fields, set_date_fields};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{DateNativeKind, DateSetFieldKind};
use crate::engine::heap::ContextId;
use crate::engine::object::ObjectRef;
use crate::engine::value::conversion::NativeConversion;

use crate::engine::value::JsValue;
#[cfg(test)]
use crate::engine::value::{JsString, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

pub(crate) mod operation;

fn date_input_fields(fields: &DateFields) -> DateInputFields {
    [
        fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
    ]
}

impl Runtime {
    /// Dispatch every `Date.prototype` callable in the typed Date native
    /// family. Constructor and static selectors are rejected here so the
    /// parent Date module remains the single owner of that routing boundary.
    pub(crate) fn call_date_prototype_native(
        &self,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        if matches!(
            kind,
            DateNativeKind::Constructor
                | DateNativeKind::Now
                | DateNativeKind::Parse
                | DateNativeKind::Utc
        ) {
            return Err(RuntimeError::Invariant(
                "Date static or constructor selector reached Date.prototype dispatch",
            ));
        }

        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Date.prototype native did not receive a generic invocation",
            ));
        };

        match kind {
            DateNativeKind::TimeValue
            | DateNativeKind::String(_)
            | DateNativeKind::TimezoneOffset
            | DateNativeKind::GetField(_) => {
                let _unwind = self.unwind_guard();
                self.0.state.borrow_mut().call_date_readonly_native(
                    &self.0.poisoned,
                    self.0.host_services.as_ref(),
                    realm,
                    kind,
                    invocation,
                )
            }
            DateNativeKind::ToPrimitive => {
                self.call_date_to_primitive(realm, this_value, arguments)
            }
            DateNativeKind::SetTime => self.call_date_set_time(realm, this_value, arguments),
            DateNativeKind::SetField(field) => {
                self.call_date_set_field(realm, this_value, field, arguments)
            }
            DateNativeKind::SetYear => self.call_date_set_year(realm, this_value, arguments),
            DateNativeKind::ToJson => self.call_date_to_json(realm, this_value),
            DateNativeKind::Constructor
            | DateNativeKind::Now
            | DateNativeKind::Parse
            | DateNativeKind::Utc => unreachable!("rejected before invocation adaptation"),
        }
    }

    fn date_this_time_value_jsvalue(
        &self,
        realm: ContextId,
        this_value: &JsValue,
    ) -> Result<NativeConversion<(ObjectRef, f64)>, RuntimeError> {
        // The legacy conversion domains still need a public temporary. Clone
        // its Runtime before the checked receiver retain, as the old adapter did.
        let receiver_runtime = if matches!(this_value, JsValue::Object(_)) {
            let runtime = self.clone();
            self.check_poison()?;
            Some(runtime)
        } else {
            None
        };
        let _unwind = self.unwind_guard();
        let result = self.0.state.borrow_mut().date_this_time_value_jsvalue(
            &self.0.poisoned,
            realm,
            this_value,
        )?;
        Ok(match result {
            NativeConversion::Value((object, value)) => NativeConversion::Value((
                ObjectRef::from_owned_handle(
                    receiver_runtime.expect("a genuine Date has an object receiver"),
                    object,
                ),
                value,
            )),
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
        })
    }

    fn set_date_this_time_value(
        &self,
        object: &ObjectRef,
        value: f64,
    ) -> Result<Completion, RuntimeError> {
        self.0
            .state
            .borrow_mut()
            .heap
            .set_date_value(object.object_id(), value)?;
        Ok(Completion::Return(
            crate::engine::value::number::operations::Number::compact(value).into(),
        ))
    }

    fn call_date_set_time(
        &self,
        realm: ContextId,
        this_value: &JsValue,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operation::finish(self, realm, {
            let invocation = NativeInvocation::Call {
                this_value: self.dup_jsvalue(this_value)?,
            };
            self.dispatch_borrowed_invocation(invocation, |invocation| {
                operation::DatePrototypeStep::start(
                    self,
                    realm,
                    DateNativeKind::SetTime,
                    invocation,
                    arguments,
                )
            })?
        })
    }

    fn call_date_set_field(
        &self,
        realm: ContextId,
        this_value: &JsValue,
        field: DateSetFieldKind,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operation::finish(self, realm, {
            let invocation = NativeInvocation::Call {
                this_value: self.dup_jsvalue(this_value)?,
            };
            self.dispatch_borrowed_invocation(invocation, |invocation| {
                operation::DatePrototypeStep::start(
                    self,
                    realm,
                    DateNativeKind::SetField(field),
                    invocation,
                    arguments,
                )
            })?
        })
    }

    fn finish_date_set_year(
        &self,
        object: &ObjectRef,
        mut year: f64,
    ) -> Result<Completion, RuntimeError> {
        if year.is_finite() {
            year = year.trunc();
            if (0.0..100.0).contains(&year) {
                year += 1900.0;
            }
        }

        let current_value = self.0.state.borrow().heap.date_value(object.object_id())?;
        let mut fields = get_date_fields(current_value, true, true, |instant| {
            self.date_timezone_offset_minutes(instant)
        })
        .ok_or(RuntimeError::Invariant(
            "forced Date decomposition unexpectedly rejected a time value",
        ))?;
        fields[0] = year;
        let new_value = if year.is_finite() {
            let input = date_input_fields(&fields);
            set_date_fields(&input, true, |instant| {
                self.date_timezone_offset_minutes(instant)
            })
        } else {
            f64::NAN
        };
        self.set_date_this_time_value(object, new_value)
    }

    fn call_date_set_year(
        &self,
        realm: ContextId,
        this_value: &JsValue,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operation::finish(self, realm, {
            let invocation = NativeInvocation::Call {
                this_value: self.dup_jsvalue(this_value)?,
            };
            self.dispatch_borrowed_invocation(invocation, |invocation| {
                operation::DatePrototypeStep::start(
                    self,
                    realm,
                    DateNativeKind::SetYear,
                    invocation,
                    arguments,
                )
            })?
        })
    }

    fn call_date_to_primitive(
        &self,
        realm: ContextId,
        this_value: &JsValue,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operation::finish(self, realm, {
            let invocation = NativeInvocation::Call {
                this_value: self.dup_jsvalue(this_value)?,
            };
            self.dispatch_borrowed_invocation(invocation, |invocation| {
                operation::DatePrototypeStep::start(
                    self,
                    realm,
                    DateNativeKind::ToPrimitive,
                    invocation,
                    arguments,
                )
            })?
        })
    }

    fn call_date_to_json(
        &self,
        realm: ContextId,
        this_value: &JsValue,
    ) -> Result<Completion, RuntimeError> {
        let arguments = NativeArguments {
            readable: Vec::new(),
            actual_arg_count: 0,
        };
        operation::finish(self, realm, {
            let invocation = NativeInvocation::Call {
                this_value: self.dup_jsvalue(this_value)?,
            };
            self.dispatch_borrowed_invocation(invocation, |invocation| {
                operation::DatePrototypeStep::start(
                    self,
                    realm,
                    DateNativeKind::ToJson,
                    invocation,
                    &arguments,
                )
            })?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(source: &str) -> Value {
        let runtime = Runtime::new();
        runtime
            .new_context()
            .expect("create context")
            .eval(source)
            .unwrap_or_else(|error| panic!("Date prototype test failed: {error:?}"))
    }

    #[test]
    fn genuine_date_brand_and_utc_formatting_are_observable() {
        assert_eq!(
            eval("new Date(0).toISOString()"),
            Value::String(JsString::from_static("1970-01-01T00:00:00.000Z"))
        );
        assert_eq!(
            eval("(function(){try{return Date.prototype.valueOf()}catch(e){return e.message}})()"),
            Value::String(JsString::from_static("not a Date object"))
        );
        assert_eq!(
            eval("Object.prototype.toString.call(Date.prototype)"),
            Value::String(JsString::from_static("[object Object]"))
        );
    }

    #[test]
    fn only_full_year_setters_recover_an_invalid_date() {
        assert_eq!(
            eval(
                r#"
                (function(){
                    var d=new Date(NaN);
                    var month=d.setUTCMonth(1);
                    var year=d.setUTCFullYear(2000);
                    return (month!==month)+'|'+year+'|'+d.toISOString()
                })()
                "#
            ),
            Value::String(JsString::from_static(
                "true|946684800000|2000-01-01T00:00:00.000Z"
            ))
        );
    }

    #[test]
    fn setters_convert_the_full_field_window_but_ignore_extra_arguments() {
        assert_eq!(
            eval(
                r#"
                (function(){
                    var log='';
                    function V(s,v){this.s=s;this.v=v}
                    V.prototype.valueOf=function(){log+=this.s;return this.v};
                    var d=new Date(0);
                    var result=d.setUTCMinutes(
                        new V('a',NaN),new V('b',1),new V('c',2),new V('x',3));
                    return log+'|'+(result!==result)+'|'+(d.getTime()!==d.getTime())
                })()
                "#
            ),
            Value::String(JsString::from_static("abc|true|true"))
        );
    }

    #[test]
    fn to_primitive_is_forced_ordinary_and_to_json_is_generic() {
        assert_eq!(
            eval(
                r#"
                (function(){
                    var object={};
                    object.valueOf=function(){return 7};
                    object.toString=function(){return 'string-result'};
                    var primitive=Date.prototype[Symbol.toPrimitive];
                    var number=primitive.call(object,'number');
                    var integer=primitive.call(object,'integer');
                    var string=primitive.call(object,'default');
                    var invalid={};
                    invalid.valueOf=function(){return Infinity};
                    invalid.toISOString=function(){return 'unreachable'};
                    var finite={};
                    finite.valueOf=function(){return 1};
                    finite.toISOString=function(){return this===finite?'json-result':'bad-this'};
                    return number+'|'+integer+'|'+string+'|'+
                        Date.prototype.toJSON.call(invalid)+'|'+
                        Date.prototype.toJSON.call(finite)
                })()
                "#
            ),
            Value::String(JsString::from_static("7|7|string-result|null|json-result"))
        );
    }
}
