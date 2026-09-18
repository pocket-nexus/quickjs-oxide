//! Internal execution value for S3-A.
//!
//! The public [`Value`] enum owns `Rc<Runtime>`-backed roots so embedders get
//! drop semantics and cross-runtime safety. Internally the VM needs a compact
//! value with arena-index handles and explicit ownership. [`JsValue`] is that
//! type: a 16-byte enum whose heap variants are `{index, generation}` handles.
//!
//! A0-v introduces the type and its conversion layer only; nothing is wired
//! yet. A1 supplies the String/BigInt arenas that back [`StringId`] and
//! [`BigIntId`], and A2 moves the slot/frame representation onto it.
//!
//! [`Value`]: crate::engine::value::Value
//! [`StringId`]: crate::engine::heap::StringId
//! [`BigIntId`]: crate::engine::heap::BigIntId
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "S3-A0-v foundation; the VM conversion points arrive with A2"
    )
)]

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::AtomIdx;
use crate::engine::heap::{BigIntId, ObjectId, StringId};
use crate::engine::object::{ObjectRef, SymbolRef};
use crate::engine::value::Value;

/// Compact internal value: inline scalars plus handle-only heap references.
///
/// It deliberately does not implement `Copy` or `Drop`. The VM duplicates with
/// [`Runtime::dup_js_value`] and releases with [`Runtime::release_js_value`] at
/// explicit ownership boundaries; releasing a handle that the runtime still
/// owns is the single most direct way to break the arena.
#[derive(Debug, PartialEq)]
pub(crate) enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    BigInt(BigIntId),
    String(StringId),
    Symbol(AtomIdx),
    Object(ObjectId),
}

// A 64-bit NaN-box encoding is A4. A0-A3 stay a Rust enum; the payloads are
// already at most 8 bytes, so the tag adds the same 8 bytes it always does.
const _: () = assert!(std::mem::size_of::<JsValue>() == 16);

impl Runtime {
    /// Root a borrowed public value, duplicating any heap edge.
    ///
    /// The returned [`JsValue`] owns one reference and must be passed to
    /// [`Runtime::release_js_value`] or consumed by [`Runtime::root_js_value`].
    pub(crate) fn unroot_value(&self, value: &Value) -> Result<JsValue, RuntimeError> {
        Ok(match value {
            Value::Undefined => JsValue::Undefined,
            Value::Null => JsValue::Null,
            Value::Bool(value) => JsValue::Bool(*value),
            Value::Int(value) => JsValue::Int(*value),
            Value::Float(value) => JsValue::Float(*value),
            Value::Object(object) => {
                let id = object.object_id();
                self.retain_object_handle(id)?;
                JsValue::Object(id)
            }
            Value::Symbol(symbol) => {
                let atom = symbol.atom();
                self.retain_atom_handle(atom)?;
                JsValue::Symbol(AtomIdx::from(atom))
            }
            // A1 turns these into heap handles; until then a borrowed public
            // String/BigInt has no stable arena identity to hand out.
            Value::String(_) | Value::BigInt(_) => {
                return Err(RuntimeError::Invariant(
                    "String/BigInt heapization arrives in S3-A1",
                ));
            }
        })
    }

    /// Duplicate an owned internal value, retaining any heap edge.
    pub(crate) fn dup_js_value(&self, value: &JsValue) -> Result<JsValue, RuntimeError> {
        Ok(match value {
            JsValue::Object(id) => {
                self.retain_object_handle(*id)?;
                JsValue::Object(*id)
            }
            JsValue::Symbol(atom) => {
                let branded = self.0.state.borrow().atoms.brand_idx(*atom)?;
                self.retain_atom_handle(branded)?;
                JsValue::Symbol(*atom)
            }
            JsValue::String(_) | JsValue::BigInt(_) => {
                return Err(RuntimeError::Invariant(
                    "String/BigInt heapization arrives in S3-A1",
                ));
            }
            JsValue::Undefined => JsValue::Undefined,
            JsValue::Null => JsValue::Null,
            JsValue::Bool(value) => JsValue::Bool(*value),
            JsValue::Int(value) => JsValue::Int(*value),
            JsValue::Float(value) => JsValue::Float(*value),
        })
    }

    /// Release one owned internal value. Scalars are a no-op.
    pub(crate) fn release_js_value(&self, value: JsValue) -> Result<(), RuntimeError> {
        match value {
            JsValue::Object(id) => self.release_object_handle(id),
            JsValue::Symbol(atom) => {
                let branded = self.0.state.borrow().atoms.brand_idx(atom)?;
                self.release_atom_handle(branded);
            }
            JsValue::String(_) | JsValue::BigInt(_) => {
                return Err(RuntimeError::Invariant(
                    "String/BigInt heapization arrives in S3-A1",
                ));
            }
            JsValue::Undefined
            | JsValue::Null
            | JsValue::Bool(_)
            | JsValue::Int(_)
            | JsValue::Float(_) => {}
        }
        Ok(())
    }

    /// Consume an owned internal value into a public root.
    ///
    /// Ownership transfers: this must not retain again, and the returned root's
    /// drop releases the single reference the [`JsValue`] carried.
    pub(crate) fn root_js_value(&self, value: JsValue) -> Result<Value, RuntimeError> {
        Ok(match value {
            JsValue::Undefined => Value::Undefined,
            JsValue::Null => Value::Null,
            JsValue::Bool(value) => Value::Bool(value),
            JsValue::Int(value) => Value::Int(value),
            JsValue::Float(value) => Value::Float(value),
            JsValue::Object(id) => Value::Object(ObjectRef::from_owned_handle(self.clone(), id)),
            JsValue::Symbol(atom) => {
                let branded = self.0.state.borrow().atoms.brand_idx(atom)?;
                Value::Symbol(SymbolRef::from_owned_atom(self.clone(), branded))
            }
            JsValue::String(_) | JsValue::BigInt(_) => {
                return Err(RuntimeError::Invariant(
                    "String/BigInt heapization arrives in S3-A1",
                ));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_round_trip_preserves_representation() {
        let runtime = Runtime::new();
        for value in [
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Int(-7),
            Value::Float(1.5),
        ] {
            let internal = runtime.unroot_value(&value).unwrap();
            let restored = runtime.root_js_value(internal).unwrap();
            assert!(value.same_quickjs_representation(&restored));
        }
    }

    #[test]
    fn object_round_trip_preserves_arena_identity() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let value = context.eval("({ answer: 42 })").unwrap();
        let Value::Object(object) = &value else {
            panic!("expected an object");
        };
        let id = object.object_id();
        let internal = runtime.unroot_value(&value).unwrap();
        assert_eq!(internal, JsValue::Object(id));
        let restored = runtime.root_js_value(internal).unwrap();
        let Value::Object(restored) = &restored else {
            panic!("expected an object root");
        };
        assert_eq!(restored.object_id(), id);
    }

    #[test]
    fn symbol_round_trip_uses_atom_index() {
        use crate::engine::value::JsString;
        let runtime = Runtime::new();
        let symbol = runtime
            .new_symbol(Some(JsString::from_static("tag")))
            .unwrap();
        let atom = AtomIdx::from(symbol.atom());
        let internal = runtime
            .unroot_value(&Value::Symbol(symbol))
            .map_err(|_| "symbol unroot failed")
            .unwrap();
        assert_eq!(internal, JsValue::Symbol(atom));
        let restored = runtime.root_js_value(internal).unwrap();
        assert!(matches!(restored, Value::Symbol(_)));
    }
}
