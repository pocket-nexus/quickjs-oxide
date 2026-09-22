use super::{
    Error, FrameBinding, FrameWindow, JsValue, ScalarTos, SlotStore, event, record_install, scalar,
};
use crate::engine::value::number::operations::Number;
#[cfg(feature = "profiling")]
use crate::engine::vm::stack::{Cost, record_owned_storage};

impl SlotStore {
    /// Inspect in canonical left/right order, before evaluating a callback or
    /// taking either input. Number copies carry no heap edges.
    fn tos_number_pair(
        &self,
        window: &FrameWindow,
        tos: &ScalarTos,
    ) -> Result<Option<(usize, Number, Number)>, Error> {
        let offset = window
            .depth
            .checked_sub(2)
            .ok_or_else(Self::operand_stack_underflow)?;
        let index = window.operands().start + offset;
        let left = tos.peek(self, window, 1)?;
        let right = tos.peek(self, window, 0)?;
        // Validate both physical locations before any callback can return an
        // owning result. Commit below has no fallible indexing left to do.
        let _ = &self.slots[index..index + 2];
        let (Some(left), Some(right)) = (left.as_number_repr(), right.as_number_repr()) else {
            return Ok(None);
        };
        Ok(Some((index, left, right)))
    }

    pub(in crate::engine::vm::stack) fn tos_binary_number(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        operation: impl FnOnce(Number, Number) -> JsValue,
    ) -> Result<bool, Error> {
        let Some((index, left, right)) = self.tos_number_pair(window, tos)? else {
            return Ok(false);
        };
        let result = operation(left, right);
        // Inputs are proven Numbers; neither their removal nor overwriting the
        // scalar cache releases an edge. Non-scalar output goes straight to
        // backing, once, rather than declining and replaying the callback.
        let cached_input = tos.value.is_some();
        tos.value = None;
        if !cached_input {
            self.slots[index + 1] = None;
        }
        let cached_result = scalar(&result);
        if cached_result {
            self.slots[index] = None;
            tos.hole = index;
            tos.value = Some(result);
        } else {
            self.slots[index] = Some(FrameBinding::Direct(result));
        }
        window.depth -= 1;
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= 1;
            record_owned_storage(Cost::Move(1));
        }
        event("binary_number_in_place");
        event("tos.backing_operand_write");
        if !cached_input {
            event("tos.backing_operand_write");
        }
        if cached_result {
            event("tos.commit");
        }
        Ok(true)
    }

    pub(in crate::engine::vm::stack) fn tos_consume_number_pair(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        operation: impl FnOnce(Number, Number) -> bool,
    ) -> Result<Option<bool>, Error> {
        let Some((index, left, right)) = self.tos_number_pair(window, tos)? else {
            return Ok(None);
        };
        let result = operation(left, right);
        let cached_input = tos.value.is_some();
        tos.value = None;
        self.slots[index] = None;
        if !cached_input {
            self.slots[index + 1] = None;
        }
        window.depth -= 2;
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= 2;
            record_owned_storage(Cost::Clear(2));
        }
        event("number_pair_consumed_in_place");
        event("tos.backing_operand_write");
        if !cached_input {
            event("tos.backing_operand_write");
        }
        Ok(Some(result))
    }

    pub(in crate::engine::vm::stack) fn tos_update_number_local(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        index: u16,
        operation: impl FnOnce(Number) -> (Number, Option<Number>),
    ) -> Result<bool, Error> {
        tos.debug_validate(self, window);
        let FrameBinding::Direct(previous) = self.local_current(window, index)? else {
            return Ok(false);
        };
        let Some(previous) = previous.as_number_repr() else {
            return Ok(false);
        };
        let (replacement, result) = operation(previous);
        let destination = result
            .as_ref()
            .map(|_| self.operand_push_index(window))
            .transpose()?;
        // Authenticate the local place before taking any cached owner.
        let local = window.locals().start + usize::from(index);
        let _ = &self.slots[local];
        self.slots[local] = Some(FrameBinding::Direct(replacement.into()));
        if let (Some(destination), Some(result)) = (destination, result) {
            let installed = tos.install(self, destination, result.into());
            self.tos_pushed(window);
            record_install(installed);
        }
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(1));
        event("number_local_updated_in_place");
        Ok(true)
    }
}
