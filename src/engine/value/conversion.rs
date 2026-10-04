pub(crate) mod descriptor;
pub(crate) mod number;
mod object;
pub(crate) use object::ToObjectOutcome;
pub(crate) mod primitive;
mod state;
mod string;

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::builtins::native::PrimitiveKind;
use crate::engine::heap::ContextId;

use crate::engine::object::{PropertyKey, WellKnownSymbol};
#[cfg(any(test, feature = "test262-host"))]
use crate::engine::value::Value;
use crate::engine::value::{JsString, JsValue};
use crate::engine::vm::{Completion, ToPrimitiveHint};

impl Runtime {
    /// Completion-aware `ToPropertyKey` used by native Object APIs. Symbols
    /// retain identity; every other value uses string-hint ToPrimitive before
    /// exact UTF-16 key interning.
    #[cfg(all(test, feature = "profiling"))]
    pub(crate) fn native_to_property_key(
        &self,
        realm: ContextId,
        value: Value,
    ) -> Result<NativeConversion<PropertyKey>, RuntimeError> {
        let value = if matches!(value, Value::Object(_)) {
            match self.to_primitive(realm, value, ToPrimitiveHint::String)? {
                Completion::Return(value) => self.root_and_release_jsvalue(value)?,
                Completion::Throw(value) => {
                    return Ok(NativeConversion::Throw(value));
                }
            }
        } else {
            value
        };
        self.property_key_from_primitive(realm, value)
    }

    /// Internal-value form of [`Runtime::native_to_property_key`].
    pub(crate) fn native_to_property_key_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
    ) -> Result<NativeConversion<PropertyKey>, RuntimeError> {
        let value = if matches!(value, crate::engine::value::JsValue::Object(_)) {
            match self.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)? {
                Completion::Return(value) => value,
                Completion::Throw(value) => {
                    return Ok(NativeConversion::Throw(value));
                }
            }
        } else {
            value
        };
        self.property_key_from_primitive_jsvalue(realm, value)
    }

    /// Internal-value form of [`Runtime::property_key_from_primitive`].
    ///
    /// Consumes one owned internal value edge and releases it on every path.
    pub(crate) fn property_key_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
    ) -> Result<NativeConversion<PropertyKey>, RuntimeError> {
        let result = (|| {
            use crate::engine::value::JsValue;
            if matches!(value, JsValue::Object(_)) {
                return Err(RuntimeError::Invariant(
                    "property key conversion received an object",
                ));
            }
            if let Some(key) = self.immediate_numeric_property_key_jsvalue(&value) {
                return Ok(NativeConversion::Value(key));
            }
            if let JsValue::Symbol(index) = &value {
                let atom = self.0.state.borrow().atoms.brand(*index)?;
                return Ok(NativeConversion::Value(PropertyKey::from_borrowed_atom(
                    self.clone(),
                    atom,
                )?));
            }
            if let JsValue::String(id) = &value {
                return Ok(NativeConversion::Value(
                    self.intern_property_key_string_id(*id)?,
                ));
            }
            let string = match crate::engine::vm::to_js_string_jsvalue(self, &value) {
                Ok(string) => string,
                Err(error) => {
                    let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                        return Err(RuntimeError::Engine(error));
                    };
                    return Ok(NativeConversion::Throw(
                        self.new_native_error_from_error_jsvalue(realm, kind, &error)?,
                    ));
                }
            };
            Ok(NativeConversion::Value(
                self.intern_property_key_js_string(&string)?,
            ))
        })();
        match (result, self.release_jsvalue(value)) {
            (Ok(conversion), Ok(())) => Ok(conversion),
            (Err(error), _) => Err(error),
            (Ok(NativeConversion::Throw(thrown)), Err(error)) => {
                let _ = self.release_jsvalue(thrown);
                Err(error)
            }
            (Ok(NativeConversion::Value(_)), Err(error)) => Err(error),
        }
    }

    /// Finish ToPropertyKey after the domain continuation has obtained a primitive.
    #[cfg(all(test, feature = "profiling"))]
    pub(crate) fn property_key_from_primitive(
        &self,
        realm: ContextId,
        value: Value,
    ) -> Result<NativeConversion<PropertyKey>, RuntimeError> {
        if matches!(value, Value::Object(_)) {
            return Err(RuntimeError::Invariant(
                "property key conversion received an object",
            ));
        }
        if let Some(key) = self.immediate_numeric_property_key(&value) {
            return Ok(NativeConversion::Value(key));
        }
        if let Value::Symbol(symbol) = value {
            if !symbol.belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("property-key symbol"));
            }
            return Ok(NativeConversion::Value(PropertyKey::from_borrowed_atom(
                self.clone(),
                symbol.atom(),
            )?));
        }
        let string = match value.to_js_string() {
            Ok(string) => string,
            Err(error) => {
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                return Ok(NativeConversion::Throw(
                    self.new_native_error_from_error_jsvalue(realm, kind, &error)?,
                ));
            }
        };
        Ok(NativeConversion::Value(
            self.intern_property_key_js_string(&string)?,
        ))
    }

    /// Port of pinned QuickJS `js_obj_to_desc`. Field probes deliberately use
    /// its C order and inherited HasProperty/Get behavior. The release also
    /// replaces a throw from the `get`/`set` field getter with its own
    /// `invalid getter`/`invalid setter` TypeError, which is preserved here.
    pub(crate) fn native_to_property_descriptor_jsvalue(
        &self,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<crate::engine::object::OwnedPropertyDescriptor>, RuntimeError>
    {
        use descriptor::DescriptorStep;
        let mut step = DescriptorStep::start_jsvalue(self, realm, value)?;
        loop {
            step = match step {
                DescriptorStep::Complete(resume) => {
                    return Ok(NativeConversion::Value(resume.take_descriptor()));
                }
                DescriptorStep::Throw(value) => return Ok(NativeConversion::Throw(value)),
                DescriptorStep::Has { mut resume } => {
                    let object = resume.take_has_object();
                    let key = resume.take_has_key();
                    resume.has(self, self.internal_has_property(realm, &object, &key)?)?
                }
                DescriptorStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    resume.read(
                        self,
                        self.internal_get_jsvalue(realm, &object, &key, receiver)?,
                    )?
                }
            };
        }
    }

    #[cfg(feature = "test262-host")]
    pub(crate) fn native_to_js_string(
        &self,
        realm: ContextId,
        value: &Value,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        self.native_to_js_string_jsvalue(realm, self.unroot_value(value)?)
    }

    /// Consume a complete ToString input through the canonical primitive child.
    pub(crate) fn native_to_js_string_jsvalue(
        &self,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        let completion = if matches!(value, JsValue::Object(_)) {
            self.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)?
        } else {
            Completion::Return(value)
        };
        let _unwind = self.unwind_guard();
        let result =
            self.0
                .state
                .borrow_mut()
                .finish_string_value(&self.0.poisoned, realm, completion);
        if result.is_ok() {
            self.check_poison()?;
        }
        result
    }

    #[cfg(any(test, feature = "test262-host"))]
    pub(crate) fn native_to_number(
        &self,
        realm: ContextId,
        value: &Value,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        self.native_to_number_jsvalue(realm, self.unroot_value(value)?)
    }

    pub(crate) fn native_to_number_jsvalue(
        &self,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        number::NumberStep::start_jsvalue(self, realm, value)?.finish(self, realm)
    }

    /// Borrow a primitive internal value; no public root or arena node is created.
    pub(crate) fn number_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        let _unwind = self.unwind_guard();
        self.0
            .state
            .borrow_mut()
            .number_from_primitive_jsvalue(&self.0.poisoned, realm, value)
    }

    /// Borrow the primitive payload through the shared State conversion.
    pub(crate) fn string_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        let _unwind = self.unwind_guard();
        let result =
            self.0
                .state
                .borrow_mut()
                .string_from_primitive_jsvalue(&self.0.poisoned, realm, value);
        if result.is_ok() {
            self.check_poison()?;
        }
        result
    }

    /// Borrow the stored primitive payload for ToBigInt without re-materializing it.
    pub(crate) fn bigint_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<crate::engine::value::bigint::JsBigInt>, RuntimeError> {
        let _unwind = self.unwind_guard();
        self.0
            .state
            .borrow_mut()
            .bigint_from_primitive_jsvalue(&self.0.poisoned, realm, value)
    }

    pub(crate) fn number_constructor_from_primitive(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        if let JsValue::ShortBigInt(value) = value {
            return Ok(NativeConversion::Value(*value as f64));
        }
        if let JsValue::BigInt(id) = value {
            return Ok(NativeConversion::Value(
                self.0.state.borrow().heap.bigint(*id)?.to_f64(),
            ));
        }
        self.number_from_primitive_jsvalue(realm, value)
    }

    pub(crate) fn native_bigint_from_string(
        &self,
        realm: ContextId,
        value: &JsString,
    ) -> Result<NativeConversion<crate::engine::value::bigint::JsBigInt>, RuntimeError> {
        let _unwind = self.unwind_guard();
        self.0
            .state
            .borrow_mut()
            .native_bigint_from_string(&self.0.poisoned, realm, value)
    }

    pub(crate) fn bigint_constructor_from_primitive(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<crate::engine::value::bigint::JsBigInt>, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "BigInt constructor primitive reply contained an object",
            ));
        }

        match value {
            JsValue::Int(value) => Ok(NativeConversion::Value(
                crate::engine::value::bigint::JsBigInt::from(*value),
            )),
            JsValue::Bool(value) => Ok(NativeConversion::Value(
                crate::engine::value::bigint::JsBigInt::from(i64::from(*value)),
            )),
            JsValue::ShortBigInt(value) => Ok(NativeConversion::Value(
                crate::engine::value::bigint::JsBigInt::from(*value),
            )),
            JsValue::BigInt(id) => Ok(NativeConversion::Value(
                self.0.state.borrow().heap.bigint(*id)?.clone(),
            )),
            JsValue::Float(value) if !value.is_finite() => {
                Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Range,
                    "cannot convert NaN or Infinity to BigInt",
                )?))
            }
            JsValue::Float(value) if value.fract() != 0.0 => {
                Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Range,
                    "cannot convert to BigInt: not an integer",
                )?))
            }
            JsValue::Float(value) => {
                let value = crate::engine::value::bigint::JsBigInt::from_integral_f64(*value)
                    .ok_or(RuntimeError::Invariant(
                        "finite integral f64 could not become a BigInt",
                    ))?;
                Ok(NativeConversion::Value(value))
            }
            JsValue::String(id) => {
                let value = self.0.state.borrow().heap.string(*id)?.clone();
                self.native_bigint_from_string(realm, &value)
            }
            JsValue::Undefined | JsValue::Null | JsValue::Symbol(_) | JsValue::Object(_) => {
                Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to BigInt",
                )?))
            }
        }
    }

    /// Pinned QuickJS `JS_ToIndex`: saturating ToInt64 followed by the
    /// non-negative MAX_SAFE_INTEGER range check.
    pub(crate) fn index_from_number(
        &self,
        realm: ContextId,
        number: f64,
    ) -> Result<NativeConversion<u64>, RuntimeError> {
        let _unwind = self.unwind_guard();
        self.0
            .state
            .borrow_mut()
            .index_from_number(&self.0.poisoned, realm, number)
    }

    pub(crate) fn int64_from_number(number: f64) -> i64 {
        if number.is_nan() {
            0
        } else if number < i64::MIN as f64 {
            i64::MIN
        } else if number >= 2_f64.powi(63) {
            i64::MAX
        } else {
            number as i64
        }
    }

    pub(crate) fn length_from_number(number: f64) -> u64 {
        const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;
        if number.is_nan() || number <= 0.0 {
            0
        } else if number >= MAX_SAFE_INTEGER as f64 {
            MAX_SAFE_INTEGER
        } else {
            number as u64
        }
    }

    #[cfg(test)]
    pub(crate) fn to_primitive(
        &self,
        realm: ContextId,
        value: Value,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step =
            primitive::PrimitiveResume::start(self, realm, self.unroot_value(&value)?, hint)?;
        self.finish_primitive_steps(realm, step)
    }

    /// Internal-value form of [`Runtime::to_primitive`]: consumes the value.
    pub(crate) fn to_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step = primitive::PrimitiveResume::start(self, realm, value, hint)?;
        self.finish_primitive_steps(realm, step)
    }
}

pub(crate) enum NativeConversion<T> {
    Value(T),
    Throw(JsValue),
}
