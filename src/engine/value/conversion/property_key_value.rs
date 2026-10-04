//! The ToPropKey opcode returns a String/Symbol value, rather than an atom.
use crate::engine::{
    api::runtime_error::RuntimeError,
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    value::JsValue,
};
use std::cell::Cell;

impl RuntimeState {
    /// Consume a primitive reply without converting its String/Symbol owner.
    /// Other primitives use the canonical formatter, retire their original
    /// edge, then allocate exactly one String node. This suffix executes no JS.
    pub(crate) fn property_key_primitive(
        &mut self,
        poisoned: &Cell<bool>,
        value: JsValue,
    ) -> Result<JsValue, RuntimeError> {
        if matches!(value, JsValue::String(_) | JsValue::Symbol(_)) {
            return Ok(value);
        }
        let mut input = OwnedValueGuard::new(self, poisoned, value);
        let (state, input) = input.parts();
        let text = state
            .to_js_string_jsvalue(input.as_ref().expect("ToPropKey input owner"))
            .map_err(RuntimeError::Engine);
        // A real destructive retirement failure quarantines before another
        // owner is traversed. A recoverable formatting error still consumes
        // its input; output allocation follows only successful retirement.
        state.release_owned_jsvalue(poisoned, input.take().expect("ToPropKey input owner"))?;
        Ok(JsValue::String(state.heap.allocate_string(text?)?))
    }
}

#[cfg(test)]
mod tests;
