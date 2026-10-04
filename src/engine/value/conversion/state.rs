//! Primitive conversion replies use the already-admitted State and defining realm.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    heap::{ContextId, runtime::RuntimeState},
    value::{
        JsString, JsValue,
        bigint::{BigIntError, JsBigInt},
        conversion::NativeConversion,
    },
};
use std::cell::Cell;

/// Primitive-only suffix outcomes tag their own freshly allocated diagnostic.
/// A propagated callback throw never enters these factories.
pub(crate) enum NumberPrimitiveStep {
    Value(f64),
    CyclePublishedThrow(JsValue),
}
impl NumberPrimitiveStep {
    pub(crate) fn into_conversion(self) -> NativeConversion<f64> {
        match self {
            Self::Value(value) => NativeConversion::Value(value),
            Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}
pub(crate) enum BigIntPrimitiveStep {
    Value(JsBigInt),
    CyclePublishedThrow(JsValue),
}
impl BigIntPrimitiveStep {
    pub(crate) fn into_conversion(self) -> NativeConversion<JsBigInt> {
        match self {
            Self::Value(value) => NativeConversion::Value(value),
            Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}
pub(crate) enum IndexPrimitiveStep {
    Value(u64),
    CyclePublishedThrow(JsValue),
}
impl IndexPrimitiveStep {
    pub(crate) fn into_conversion(self) -> NativeConversion<u64> {
        match self {
            Self::Value(value) => NativeConversion::Value(value),
            Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}

impl RuntimeState {
    pub(crate) fn object_id_has_call_capability(
        &self,
        object: crate::engine::heap::ObjectId,
    ) -> Result<bool, RuntimeError> {
        Ok(matches!(
            self.heap.object(object)?.payload,
            crate::engine::heap::ObjectPayload::NativeFunction { .. }
                | crate::engine::heap::ObjectPayload::BoundFunction { .. }
                | crate::engine::heap::ObjectPayload::BytecodeFunction { .. }
                | crate::engine::heap::ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                    is_callable: true,
                    ..
                })
        ))
    }

    pub(crate) fn number_from_primitive_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<f64>, RuntimeError> {
        self.number_from_primitive_jsvalue_with_publication(poisoned, realm, value)
            .map(NumberPrimitiveStep::into_conversion)
    }

    pub(crate) fn number_from_primitive_jsvalue_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NumberPrimitiveStep, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "ToNumber primitive completion received an object",
            ));
        }
        match self.to_number_primitive_jsvalue(value) {
            Ok(number) => Ok(NumberPrimitiveStep::Value(number)),
            Err(error) => {
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                let message = error
                    .native_message()
                    .cloned()
                    .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                Ok(NumberPrimitiveStep::CyclePublishedThrow(JsValue::Object(
                    self.new_native_error_from_message(poisoned, realm, kind, message)?,
                )))
            }
        }
    }

    pub(crate) fn bigint_from_primitive_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<JsBigInt>, RuntimeError> {
        self.bigint_from_primitive_jsvalue_with_publication(poisoned, realm, value)
            .map(BigIntPrimitiveStep::into_conversion)
    }

    pub(crate) fn bigint_from_primitive_jsvalue_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<BigIntPrimitiveStep, RuntimeError> {
        match value {
            JsValue::ShortBigInt(value) => Ok(BigIntPrimitiveStep::Value(JsBigInt::from(*value))),
            JsValue::BigInt(id) => Ok(BigIntPrimitiveStep::Value(self.heap.bigint(*id)?.clone())),
            JsValue::Bool(value) => Ok(BigIntPrimitiveStep::Value(JsBigInt::from(i64::from(
                *value,
            )))),
            JsValue::String(id) => {
                let string = self.heap.string(*id)?.clone();
                self.native_bigint_from_string_with_publication(poisoned, realm, &string)
            }
            JsValue::Object(_) => Err(RuntimeError::Invariant(
                "ToBigInt primitive completion received an object",
            )),
            _ => Ok(BigIntPrimitiveStep::CyclePublishedThrow(JsValue::Object(
                self.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("cannot convert to bigint"),
                )?,
            ))),
        }
    }

    pub(crate) fn native_bigint_from_string(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsString,
    ) -> Result<NativeConversion<JsBigInt>, RuntimeError> {
        self.native_bigint_from_string_with_publication(poisoned, realm, value)
            .map(BigIntPrimitiveStep::into_conversion)
    }

    pub(crate) fn native_bigint_from_string_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsString,
    ) -> Result<BigIntPrimitiveStep, RuntimeError> {
        let units = value.utf16_units().collect::<Vec<_>>();
        let parsed = String::from_utf16(&units)
            .map_err(|_| BigIntError::InvalidSyntax)
            .and_then(|value| JsBigInt::parse_js_string(&value));
        let (kind, message) = match parsed {
            Ok(value) => return Ok(BigIntPrimitiveStep::Value(value)),
            Err(BigIntError::InvalidSyntax) => (
                NativeErrorKind::Syntax,
                NativeErrorMessage::from_utf8("invalid bigint literal"),
            ),
            Err(BigIntError::BigIntTooLarge | BigIntError::AllocationTooLarge) => (
                NativeErrorKind::Range,
                NativeErrorMessage::from_utf8("BigInt is too large to allocate"),
            ),
            Err(error) => (
                NativeErrorKind::Range,
                NativeErrorMessage::from_utf8(&error.to_string()),
            ),
        };
        Ok(BigIntPrimitiveStep::CyclePublishedThrow(JsValue::Object(
            self.new_native_error_from_message(poisoned, realm, kind, message)?,
        )))
    }

    pub(crate) fn index_from_number(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        number: f64,
    ) -> Result<NativeConversion<u64>, RuntimeError> {
        self.index_from_number_with_publication(poisoned, realm, number)
            .map(IndexPrimitiveStep::into_conversion)
    }

    pub(crate) fn index_from_number_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        number: f64,
    ) -> Result<IndexPrimitiveStep, RuntimeError> {
        const MAX_SAFE_INTEGER: i64 = (1_i64 << 53) - 1;
        let value = Runtime::int64_from_number(number);
        if !(0..=MAX_SAFE_INTEGER).contains(&value) {
            return Ok(IndexPrimitiveStep::CyclePublishedThrow(JsValue::Object(
                self.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Range,
                    NativeErrorMessage::from_utf8("invalid array index"),
                )?,
            )));
        }
        Ok(IndexPrimitiveStep::Value(
            u64::try_from(value).expect("validated non-negative ToIndex value fits u64"),
        ))
    }
}
