//! Exact typed-key and byte selection shared by own-property materialization.

use super::{
    CanonicalNumericIndex, OrdinaryTypedWord, TypedArrayElementKind,
    ordinary_typed_array_word_in_heap, typed_array_decode, typed_array_snapshot_from_payload,
    typed_array_word_range,
};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, PropertyKeyKind};
use crate::engine::builtins::buffer_access::read_shared_buffer_word;
use crate::engine::heap::ObjectId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::shared_memory::SharedBufferHandle;
use crate::engine::value::{JsString, Value};

/// The String branch historically crosses a Runtime operation before reading
/// its spelling. Existing Runtime callers retain that boundary; admitted State
/// callers use the same spelling algorithm without nesting an operation.
#[derive(Clone, Copy)]
pub(crate) enum TypedOwnKey {
    Immediate(u64),
    String,
    Other,
}

pub(crate) enum TypedOwnProperty {
    Missing,
    Word(TypedOwnWord),
    Shared(SharedTypedOwnWord),
}

pub(crate) struct TypedOwnWord {
    pub(crate) element: TypedArrayElementKind,
    pub(crate) bytes: [u8; 8],
}

impl TypedOwnWord {
    pub(crate) fn into_public_value(self) -> Value {
        typed_array_decode(self.element, self.bytes)
    }
}

/// The actual shared mutex boundary. This owns the backing Arc and copied
/// wrapper bounds, without an ObjectRef or a borrowed runtime-state fact.
pub(crate) struct SharedTypedOwnWord {
    backing: SharedBufferHandle,
    absolute: usize,
    width: usize,
    element: TypedArrayElementKind,
}

impl SharedTypedOwnWord {
    /// Call only after releasing RuntimeState. No JavaScript runs between
    /// selection and this byte-only read, and no property is selected again.
    pub(crate) fn read(self) -> Result<TypedOwnWord, RuntimeError> {
        Ok(TypedOwnWord {
            element: self.element,
            bytes: read_shared_buffer_word(&self.backing, self.absolute, self.width)?,
        })
    }
}

impl RuntimeState {
    pub(crate) fn typed_own_key(&self, atom: Atom) -> Result<TypedOwnKey, RuntimeError> {
        if let Some(index) = atom.immediate_integer() {
            return Ok(TypedOwnKey::Immediate(u64::from(index)));
        }
        Ok(
            if self.atoms.property_key_kind(atom)? == PropertyKeyKind::String {
                TypedOwnKey::String
            } else {
                TypedOwnKey::Other
            },
        )
    }

    /// CanonicalNumericIndexString over an admitted atom. The Runtime adapter
    /// supplies the historical spelling operation before this String branch.
    pub(crate) fn typed_canonical_numeric_index(
        &self,
        atom: Atom,
        key: TypedOwnKey,
    ) -> Result<Option<CanonicalNumericIndex>, RuntimeError> {
        match key {
            TypedOwnKey::Immediate(index) => {
                return Ok(Some(CanonicalNumericIndex::Valid(index)));
            }
            TypedOwnKey::Other => return Ok(None),
            TypedOwnKey::String => {}
        }
        let spelling = self.atoms.to_js_string(atom)?;
        if spelling == JsString::from_static("-0") {
            return Ok(Some(CanonicalNumericIndex::Invalid));
        }
        let number = Value::String(spelling.clone())
            .to_number()
            .map_err(RuntimeError::Engine)?;
        if spelling != Value::number(number).to_js_string()? {
            return Ok(None);
        }
        if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > u64::MAX as f64
        {
            return Ok(Some(CanonicalNumericIndex::Invalid));
        }
        Ok(Some(CanonicalNumericIndex::Valid(number as u64)))
    }

    /// Authenticate the selected view under this State access, then share the
    /// existing ordinary/shared word and range algorithms.
    pub(crate) fn select_typed_own_property(
        &mut self,
        object: ObjectId,
        index: u64,
    ) -> Result<TypedOwnProperty, RuntimeError> {
        let snapshot = typed_array_snapshot_from_payload(&self.heap.object(object)?.payload)
            .ok_or(RuntimeError::Invariant(
                "validated TypedArray lost its class payload",
            ))?;
        match ordinary_typed_array_word_in_heap(&mut self.heap, snapshot, index, None)? {
            OrdinaryTypedWord::Missing => return Ok(TypedOwnProperty::Missing),
            OrdinaryTypedWord::Word(bytes) => {
                return Ok(TypedOwnProperty::Word(TypedOwnWord {
                    element: snapshot.element,
                    bytes,
                }));
            }
            OrdinaryTypedWord::Shared => {}
        }
        // As in snapshot_buffer_access, capture the wrapper state and clone
        // its shared handle before checking the range. Neither operation locks.
        let buffer = self.heap.buffer_state(snapshot.buffer)?;
        let backing = self
            .heap
            .clone_shared_array_buffer_handle(snapshot.buffer)?;
        let Some((absolute, width)) = typed_array_word_range(snapshot, buffer, index)? else {
            return Ok(TypedOwnProperty::Missing);
        };
        Ok(TypedOwnProperty::Shared(SharedTypedOwnWord {
            backing,
            absolute,
            width,
            element: snapshot.element,
        }))
    }
}
