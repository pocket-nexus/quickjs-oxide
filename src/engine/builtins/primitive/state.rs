//! Complete primitive brand and formatting algorithms under an admitted State lease.
use std::cell::Cell;

use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    builtins::{NumericStep, continuation::NativeStep, native::PrimitiveKind},
    heap::{
        ContextId, ObjectPayload, PrimitiveObjectData,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    value::{JsString, JsValue, Value},
    vm::{Completion, call::NativeInvocation},
};

/// Brand selection either retains the exact primitive edge or publishes its
/// own brand Error; it never accepts a propagated callback completion.
pub(crate) enum BrandedPrimitiveStep {
    Value(JsValue),
    CyclePublishedThrow(JsValue),
}

impl RuntimeState {
    pub(crate) fn primitive_this_value_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: PrimitiveKind,
        value: &JsValue,
    ) -> Result<BrandedPrimitiveStep, RuntimeError> {
        if matches!(
            (value, kind),
            (JsValue::Int(_) | JsValue::Float(_), PrimitiveKind::Number)
                | (JsValue::String(_), PrimitiveKind::String)
                | (JsValue::Bool(_), PrimitiveKind::Boolean)
                | (JsValue::Symbol(_), PrimitiveKind::Symbol)
                | (
                    JsValue::BigInt(_) | JsValue::ShortBigInt(_),
                    PrimitiveKind::BigInt
                )
        ) {
            return self.dup_jsvalue(value).map(BrandedPrimitiveStep::Value);
        }
        if let JsValue::Object(id) = value {
            let payload = match &self.heap.object(*id)?.payload {
                ObjectPayload::Primitive(PrimitiveObjectData::Number(v))
                    if kind == PrimitiveKind::Number =>
                {
                    Some(match Value::number(*v) {
                        Value::Int(v) => JsValue::Int(v),
                        Value::Float(v) => JsValue::Float(v),
                        _ => unreachable!(),
                    })
                }
                ObjectPayload::Primitive(PrimitiveObjectData::String(v))
                    if kind == PrimitiveKind::String =>
                {
                    Some(JsValue::String(*v))
                }
                ObjectPayload::Primitive(PrimitiveObjectData::Boolean(v))
                    if kind == PrimitiveKind::Boolean =>
                {
                    Some(JsValue::Bool(*v))
                }
                ObjectPayload::Primitive(PrimitiveObjectData::ShortBigInt(v))
                    if kind == PrimitiveKind::BigInt =>
                {
                    Some(JsValue::ShortBigInt(*v))
                }
                ObjectPayload::Primitive(PrimitiveObjectData::BigInt(v))
                    if kind == PrimitiveKind::BigInt =>
                {
                    Some(JsValue::BigInt(*v))
                }
                ObjectPayload::Primitive(PrimitiveObjectData::Symbol(atom))
                    if kind == PrimitiveKind::Symbol =>
                {
                    Some(JsValue::Symbol(self.atoms.unbrand(*atom)?))
                }
                _ => None,
            };
            if let Some(payload) = payload {
                return self.dup_jsvalue(&payload).map(BrandedPrimitiveStep::Value);
            }
        }
        let message = match kind {
            PrimitiveKind::Number => "not a number",
            PrimitiveKind::String => "not a string",
            PrimitiveKind::Boolean => "not a boolean",
            PrimitiveKind::Symbol => "not a symbol",
            PrimitiveKind::BigInt => "not a BigInt",
        };
        Ok(BrandedPrimitiveStep::CyclePublishedThrow(JsValue::Object(
            self.new_native_error_from_message(
                poisoned,
                realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8(message),
            )?,
        )))
    }

    /// Consume the checked brand edge after producing its exact string/result.
    pub(crate) fn finish_branded_to_string(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: PrimitiveKind,
        value: JsValue,
        radix: u32,
    ) -> Result<NumericStep, RuntimeError> {
        if matches!((kind, &value), (PrimitiveKind::String, JsValue::String(_))) {
            return Ok(NumericStep::Complete(Completion::Return(value)));
        }
        let mut input = OwnedValueGuard::new(self, poisoned, value);
        let (state, input) = input.parts();
        let result = state.format_branded_value(
            poisoned,
            realm,
            kind,
            input.as_ref().expect("brand value"),
            radix,
        );
        if poisoned.get() {
            return result.and(Err(RuntimeError::Poisoned));
        }
        state.release_owned_jsvalue(poisoned, input.take().expect("brand value"))?;
        result
    }

    fn format_branded_value(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: PrimitiveKind,
        value: &JsValue,
        radix: u32,
    ) -> Result<NumericStep, RuntimeError> {
        let text = match (kind, value) {
            (PrimitiveKind::Number, JsValue::Int(_) | JsValue::Float(_)) => {
                let number = value.as_number().ok_or(RuntimeError::Invariant(
                    "Number brand extraction did not return a Number",
                ))?;
                let text =
                    crate::engine::value::number::to_string_radix(number, radix).map_err(|_| {
                        RuntimeError::Invariant(
                            "validated Number radix was rejected by the formatter",
                        )
                    })?;
                JsString::checked_length(0, text.len())?;
                JsString::from_owned_latin1(text.into_bytes())
            }
            (PrimitiveKind::Boolean, JsValue::Bool(value)) => {
                JsString::from_static(if *value { "true" } else { "false" })
            }
            (PrimitiveKind::Symbol, JsValue::Symbol(index)) => {
                let atom = self.atoms.brand(*index)?;
                // Preserve the checked temporary SymbolRef retain, without a Runtime root.
                let owned = self.dup_jsvalue(value)?;
                let mut symbol = OwnedValueGuard::new(self, poisoned, owned);
                let (state, edge) = symbol.parts();
                let text = state.symbol_descriptive_string_atom(atom);
                state.release_owned_jsvalue(
                    poisoned,
                    edge.take().expect("symbol formatter temporary"),
                )?;
                text?
            }
            (PrimitiveKind::BigInt, JsValue::ShortBigInt(value)) => {
                let text = crate::engine::value::bigint::JsBigInt::from(*value)
                    .to_string_radix(radix)
                    .map_err(|_| RuntimeError::Invariant("validated BigInt radix was rejected"))?;
                JsString::from_owned_latin1(text.into_bytes())
            }
            (PrimitiveKind::BigInt, JsValue::BigInt(id)) => {
                let bigint = self.heap.bigint(*id)?;
                if bigint.exceeds_allocation_limit()
                    && (bigint.is_negative() || !radix.is_power_of_two())
                {
                    return Ok(NumericStep::CyclePublished(Completion::Throw(
                        JsValue::Object(self.new_native_error_from_message(
                            poisoned,
                            realm,
                            NativeErrorKind::Range,
                            NativeErrorMessage::from_utf8("BigInt is too large to allocate"),
                        )?),
                    )));
                }
                let text = bigint
                    .to_string_radix(radix)
                    .map_err(|_| RuntimeError::Invariant("validated BigInt radix was rejected"))?;
                JsString::checked_length(0, text.len())?;
                JsString::from_owned_latin1(text.into_bytes())
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "unimplemented primitive toString reached native dispatch",
                ));
            }
        };
        Ok(NumericStep::Complete(Completion::Return(JsValue::String(
            self.heap.allocate_string(text)?,
        ))))
    }

    pub(crate) fn finish_number_format(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        result: Result<String, crate::engine::value::number::NumberFormatError>,
    ) -> Result<NumericStep, RuntimeError> {
        match result {
            Ok(value) => {
                JsString::checked_length(0, value.len())?;
                debug_assert!(value.is_ascii());
                Ok(NumericStep::Complete(Completion::Return(JsValue::String(
                    self.heap
                        .allocate_string(JsString::from_owned_latin1(value.into_bytes()))?,
                ))))
            }
            Err(error) => {
                let message = match error {
                    crate::engine::value::number::NumberFormatError::InvalidDigits => {
                        "invalid number of digits"
                    }
                    crate::engine::value::number::NumberFormatError::InvalidRadix => {
                        "radix must be between 2 and 36"
                    }
                };
                Ok(NumericStep::CyclePublished(Completion::Throw(
                    JsValue::Object(self.new_native_error_from_message(
                        poisoned,
                        realm,
                        NativeErrorKind::Range,
                        NativeErrorMessage::from_utf8(message),
                    )?),
                )))
            }
        }
    }

    pub(crate) fn call_symbol_prototype_description(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<NativeStep, RuntimeError> {
        let NativeInvocation::Getter { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Symbol.prototype.description received the wrong native invocation",
            ));
        };
        let value = match self.primitive_this_value_jsvalue(
            poisoned,
            realm,
            PrimitiveKind::Symbol,
            this_value,
        )? {
            BrandedPrimitiveStep::Value(value) => value,
            BrandedPrimitiveStep::CyclePublishedThrow(value) => {
                return Ok(NativeStep::CyclePublishedComplete(Completion::Throw(value)));
            }
        };
        let mut symbol = OwnedValueGuard::new(self, poisoned, value);
        let (state, edge) = symbol.parts();
        let result = (|| {
            let Some(JsValue::Symbol(index)) = edge.as_ref() else {
                return Err(RuntimeError::Invariant(
                    "Symbol brand extraction did not return a Symbol",
                ));
            };
            let atom = state.atoms.brand(*index)?;
            Ok(NativeStep::Complete(Completion::Return(
                match state.symbol_description_atom(atom)? {
                    Some(value) => JsValue::String(state.heap.allocate_string(value)?),
                    None => JsValue::Undefined,
                },
            )))
        })();
        state.release_owned_jsvalue(poisoned, edge.take().expect("symbol description brand"))?;
        result
    }

    pub(crate) fn call_primitive_prototype_value_of(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: PrimitiveKind,
        invocation: &NativeInvocation,
    ) -> Result<NativeStep, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "primitive valueOf did not receive a generic invocation",
            ));
        };
        Ok(
            match self.primitive_this_value_jsvalue(poisoned, realm, kind, this_value)? {
                BrandedPrimitiveStep::Value(value) => {
                    NativeStep::Complete(Completion::Return(value))
                }
                BrandedPrimitiveStep::CyclePublishedThrow(value) => {
                    NativeStep::CyclePublishedComplete(Completion::Throw(value))
                }
            },
        )
    }
}
