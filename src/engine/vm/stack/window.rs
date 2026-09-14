//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, Runtime, SlotStore, Value};

pub(in crate::engine::vm) struct RunSlots<'a> {
    pub(super) store: &'a mut SlotStore,
    pub(super) window: &'a mut FrameWindow,
}
impl RunSlots<'_> {
    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) fn depth(&self) -> usize {
        self.window.depth
    }
    pub(in crate::engine::vm) fn peek(&self, from_top: usize) -> Result<&Value, Error> {
        self.store.peek_current(self.window, from_top)
    }

    pub(in crate::engine::vm) fn push(&mut self, value: Value) -> Result<(), Error> {
        self.store.push_current(self.window, value)
    }

    pub(in crate::engine::vm) fn pop(&mut self) -> Result<Value, Error> {
        self.store.pop_current(self.window)
    }

    pub(in crate::engine::vm) fn local(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.local_current(self.window, index)
    }

    pub(in crate::engine::vm) fn parameter(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.parameter_current(self.window, index)
    }

    pub(in crate::engine::vm) fn replace_local(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store.replace_local_current(self.window, index, value)
    }

    pub(in crate::engine::vm) fn replace_parameter(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store
            .replace_parameter_current(self.window, index, value)
    }

    pub(in crate::engine::vm) fn rotate_operands(
        &mut self,
        skip_top: usize,
        count: usize,
        left: bool,
    ) -> Result<(), Error> {
        self.store
            .rotate_operands_current(self.window, skip_top, count, left)
    }

    pub(in crate::engine::vm) fn insert_copy(
        &mut self,
        source_from_top: usize,
        destination_from_top: usize,
    ) -> Result<(), Error> {
        self.store
            .insert_copy_current(self.window, source_from_top, destination_from_top)
    }

    pub(in crate::engine::vm) fn duplicate_operands(&mut self, count: usize) -> Result<(), Error> {
        self.store.duplicate_operands_current(self.window, count)
    }

    pub(in crate::engine::vm) fn release_operand(
        &mut self,
        from_top: usize,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.store
            .release_operand_current(self.window, from_top, runtime)
    }

    pub(in crate::engine::vm) fn binary_number(
        &mut self,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> Value,
    ) -> Result<bool, Error> {
        self.store.binary_number_current(self.window, operation)
    }
}
