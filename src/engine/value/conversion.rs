pub(crate) mod descriptor;
pub(crate) mod number;
pub(crate) mod primitive;

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::builtins::native::PrimitiveKind;
use crate::engine::heap::ContextId;

use crate::engine::object::{ObjectRef, PropertyKey, WellKnownSymbol};
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::{Completion, ToPrimitiveHint};

impl Runtime {
    /// Completion-aware `ToPropertyKey` used by native Object APIs. Symbols
    /// retain identity; every other value uses string-hint ToPrimitive before
    /// exact UTF-16 key interning.
    #[cfg(test)]
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
    #[cfg(test)]
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

    pub(crate) fn native_to_js_string(
        &self,
        realm: ContextId,
        value: &Value,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        self.native_to_js_string_jsvalue(realm, self.unroot_value(value)?)
    }

    /// Consume an internal ToString input without rebuilding an arena node.
    pub(crate) fn native_to_js_string_jsvalue(
        &self,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        let value = if matches!(value, JsValue::Object(_)) {
            match self.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)? {
                Completion::Return(value) => value,
                Completion::Throw(value) => {
                    return Ok(NativeConversion::Throw(value));
                }
            }
        } else {
            value
        };
        let result = self.string_from_primitive_jsvalue(realm, &value);
        if let Err(error) = self.release_jsvalue(value) {
            if let Ok(NativeConversion::Throw(thrown)) = result {
                let _ = self.release_jsvalue(thrown);
            }
            return Err(error);
        }
        result
    }

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
        let mut step = number::NumberStep::start_jsvalue(self, realm, value)?;
        loop {
            step = match step {
                number::NumberStep::Complete(result) => return Ok(result),
                number::NumberStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?
                }
                number::NumberStep::Call { mut resume } => {
                    let callable = resume.take_call_callable();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    resume.resume(
                        self,
                        self.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                    )?
                }
            };
        }
    }

    /// Borrow a primitive internal value; no public root or arena node is created.
    pub(crate) fn number_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "ToNumber primitive completion received an object",
            ));
        }
        match crate::engine::vm::to_number_jsvalue(self, value) {
            Ok(number) => Ok(NativeConversion::Value(number)),
            Err(error) => {
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                Ok(NativeConversion::Throw(
                    self.new_native_error_from_error_jsvalue(realm, kind, &error)?,
                ))
            }
        }
    }

    /// Primitive-only ToString over the original arena payload.
    pub(crate) fn string_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "ToString primitive completion received an object",
            ));
        }
        match crate::engine::vm::to_js_string_jsvalue(self, value) {
            Ok(value) => Ok(NativeConversion::Value(value.linearize())),
            Err(error) => {
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                Ok(NativeConversion::Throw(
                    self.new_native_error_from_error_jsvalue(realm, kind, &error)?,
                ))
            }
        }
    }

    /// Borrow the stored primitive payload for ToBigInt without re-materializing it.
    pub(crate) fn bigint_from_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<crate::engine::value::bigint::JsBigInt>, RuntimeError> {
        match value {
            JsValue::BigInt(id) => Ok(NativeConversion::Value(
                self.0.state.borrow().heap.bigint(*id)?.clone(),
            )),
            JsValue::Bool(value) => Ok(NativeConversion::Value(
                crate::engine::value::bigint::JsBigInt::from(i64::from(*value)),
            )),
            JsValue::String(id) => {
                let string = self.0.state.borrow().heap.string(*id)?.clone();
                self.native_bigint_from_string(realm, &string)
            }
            JsValue::Object(_) => Err(RuntimeError::Invariant(
                "ToBigInt primitive completion received an object",
            )),
            _ => Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "cannot convert to bigint",
            )?)),
        }
    }

    pub(crate) fn number_constructor_from_primitive(
        &self,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
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
        let units = value.utf16_units().collect::<Vec<_>>();
        let Ok(value) = String::from_utf16(&units) else {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Syntax,
                "invalid bigint literal",
            )?));
        };
        match crate::engine::value::bigint::JsBigInt::parse_js_string(&value) {
            Ok(value) => Ok(NativeConversion::Value(value)),
            Err(crate::engine::value::bigint::BigIntError::InvalidSyntax) => {
                Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Syntax,
                    "invalid bigint literal",
                )?))
            }
            Err(
                crate::engine::value::bigint::BigIntError::BigIntTooLarge
                | crate::engine::value::bigint::BigIntError::AllocationTooLarge,
            ) => Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Range,
                "BigInt is too large to allocate",
            )?)),
            Err(error) => Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Range,
                &error.to_string(),
            )?)),
        }
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
        const MAX_SAFE_INTEGER: i64 = (1_i64 << 53) - 1;
        let value = Self::int64_from_number(number);
        if !(0..=MAX_SAFE_INTEGER).contains(&value) {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Range,
                "invalid array index",
            )?));
        }
        Ok(NativeConversion::Value(
            u64::try_from(value).expect("validated non-negative ToIndex value fits u64"),
        ))
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
        let step = primitive::PrimitiveResume::start(self, realm, self.unroot_value(&value)?, hint);
        self.finish_primitive_steps(realm, step)
    }

    /// Internal-value form of [`Runtime::to_primitive`]: consumes the value.
    pub(crate) fn to_primitive_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step = primitive::PrimitiveResume::start(self, realm, value, hint);
        self.finish_primitive_steps(realm, step)
    }

    /// Internal-value form of [`Runtime::native_to_object`]: consumes the value.
    pub(crate) fn native_to_object_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        use crate::engine::value::JsValue;
        let (kind, value) = match value {
            JsValue::Object(object) => {
                return Ok(NativeConversion::Value(ObjectRef::from_owned_handle(
                    self.clone(),
                    object,
                )));
            }
            JsValue::Undefined | JsValue::Null => {
                return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?));
            }
            value @ JsValue::Bool(_) => (PrimitiveKind::Boolean, value),
            value @ (JsValue::Int(_) | JsValue::Float(_)) => (PrimitiveKind::Number, value),
            value @ JsValue::String(_) => (PrimitiveKind::String, value),
            value @ JsValue::BigInt(_) => (PrimitiveKind::BigInt, value),
            value @ JsValue::Symbol(_) => (PrimitiveKind::Symbol, value),
        };
        let prototype = match self.primitive_prototype_for_realm(realm, kind) {
            Ok(prototype) => prototype,
            Err(error) => {
                self.release_jsvalue(value)?;
                return Err(error);
            }
        };
        Ok(NativeConversion::Value(
            self.new_primitive_object_jsvalue(&prototype, kind, value)?,
        ))
    }
}

pub(crate) enum NativeConversion<T> {
    Value(T),
    Throw(JsValue),
}
