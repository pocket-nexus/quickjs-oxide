//! Callback-free primitive formatting and key interning use the caller's State.
//! Returned atoms own one edge; VM consumers release or publish that edge.
use crate::engine::{
    api::{Error, ErrorKind, runtime_error::RuntimeError},
    atom::Atom,
    heap::runtime::RuntimeState,
    value::{JsString, JsValue},
};

impl RuntimeState {
    /// Borrow an admitted primitive. Formatting never invokes JavaScript and
    /// never reacquires RuntimeState or constructs a public value root.
    pub(crate) fn primitive_to_js_string(&self, value: &JsValue) -> Result<JsString, Error> {
        Ok(match value {
            JsValue::String(id) => self
                .heap
                .string(*id)
                .map_err(|error| Error::internal(error.to_string()))?
                .clone(),
            JsValue::Undefined => JsString::from_static("undefined"),
            JsValue::Null => JsString::from_static("null"),
            JsValue::Bool(true) => JsString::from_static("true"),
            JsValue::Bool(false) => JsString::from_static("false"),
            JsValue::Int(value) => JsString::from_owned_latin1(value.to_string().into_bytes()),
            JsValue::Float(value) => JsString::from_owned_latin1(
                crate::engine::value::number_to_string(*value).into_bytes(),
            ),
            JsValue::ShortBigInt(value) => {
                JsString::from_owned_latin1(value.to_string().into_bytes())
            }
            JsValue::BigInt(id) => {
                let bigint = self
                    .heap
                    .bigint(*id)
                    .map_err(|error| Error::internal(error.to_string()))?;
                if bigint.exceeds_allocation_limit() {
                    return Err(Error::new(
                        ErrorKind::Range,
                        "BigInt is too large to allocate",
                    ));
                }
                JsString::from_owned_latin1(bigint.to_string().into_bytes())
            }
            JsValue::Symbol(_) => {
                return Err(Error::new(
                    ErrorKind::Type,
                    "cannot convert symbol to string",
                ));
            }
            JsValue::Object(_) => {
                return Err(Error::internal(
                    "object ToPrimitive requires an execution context",
                ));
            }
        })
    }

    /// Return one owned atom without consuming or duplicating the primitive.
    /// Integer encoding follows Number-to-string, including numeric -0. The
    /// input's string or symbol owner remains live until interning is complete.
    pub(crate) fn property_key_atom_from_primitive(
        &mut self,
        value: &JsValue,
    ) -> Result<Atom, RuntimeError> {
        let integer = match value {
            JsValue::Int(value) => u32::try_from(*value).ok(),
            JsValue::Float(value)
                if *value >= 0.0 && *value <= u32::MAX as f64 && value.fract() == 0.0 =>
            {
                Some(*value as u32)
            }
            _ => None,
        };
        if let Some(atom) = integer.and_then(Atom::from_immediate_integer) {
            return Ok(atom);
        }
        match value {
            JsValue::Object(_) => Err(RuntimeError::Invariant(
                "property key conversion received an object",
            )),
            JsValue::Symbol(index) => {
                let atom = self.atoms.brand(*index)?;
                self.atoms.retain(atom)?;
                Ok(atom)
            }
            JsValue::String(id) => self.intern_property_key_string_id(*id),
            value => {
                let text = self.primitive_to_js_string(value)?;
                self.intern_property_key_js_string(&text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{api::Runtime, value::Value};

    #[test]
    fn primitive_keys_use_current_state_and_exact_number_spelling() {
        let runtime = Runtime::new();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        for (value, spelling) in [
            (JsValue::Int(0), "0"),
            (JsValue::Float(-0.0), "0"),
            (JsValue::Float(1.5), "1.5"),
            (JsValue::Int(-1), "-1"),
            (JsValue::Float(1e21), "1e+21"),
            (JsValue::ShortBigInt(-23), "-23"),
            (JsValue::Undefined, "undefined"),
            (JsValue::Null, "null"),
            (JsValue::Bool(true), "true"),
        ] {
            let atom = state.property_key_atom_from_primitive(&value).unwrap();
            assert_eq!(
                state.atoms.to_js_string(atom).unwrap(),
                JsString::from_static(spelling)
            );
            state.atoms.release(atom).unwrap();
        }
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn borrowed_string_key_does_not_duplicate_the_input_edge() {
        let runtime = Runtime::new();
        let text = JsString::try_from_utf16([0xd800, 0x78]).unwrap();
        let value = runtime.into_jsvalue(Value::String(text.clone())).unwrap();
        let JsValue::String(id) = value else {
            panic!("string value")
        };
        let mut state = runtime.0.state.borrow_mut();
        let count = state
            .heap
            .strong_count(crate::engine::heap::RawId::String(id))
            .unwrap();
        let atom = state.property_key_atom_from_primitive(&value).unwrap();
        assert_eq!(
            state
                .heap
                .strong_count(crate::engine::heap::RawId::String(id))
                .unwrap(),
            count
        );
        assert_eq!(state.atoms.to_js_string(atom).unwrap(), text);
        state.atoms.release(atom).unwrap();
        state.release_jsvalue(value).unwrap();
    }

    #[test]
    fn symbol_key_retains_the_atom_and_cleans_directly() {
        let runtime = Runtime::new();
        let symbol = runtime
            .new_symbol(Some(JsString::from_static("key")))
            .unwrap();
        let value = runtime.unroot_value(&Value::Symbol(symbol)).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let atom = state.property_key_atom_from_primitive(&value).unwrap();
        assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
        state.release_jsvalue(value).unwrap();
        assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
        state.atoms.release(atom).unwrap();
        assert!(state.atoms.resolve(atom).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}
