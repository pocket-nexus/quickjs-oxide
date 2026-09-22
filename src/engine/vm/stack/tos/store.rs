use super::{Error, FrameBinding, FrameWindow, ScalarTos, SlotStore, event};
use crate::engine::vm::stack::{Runtime, StoreMode, copy_value};
#[cfg(feature = "profiling")]
use crate::engine::vm::stack::{Cost, record_owned_storage};

impl SlotStore {
    pub(in crate::engine::vm::stack) fn tos_store_local(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<FrameBinding>, Error> {
        tos.debug_validate(self, window);
        if tos.value.is_none() {
            event("tos.miss");
            return self.store_local_from_top_current(window, runtime, index, mode);
        }
        if usize::from(index) >= window.locals().len() {
            return Err(Error::internal("owned local index is out of bounds"));
        }
        let destination = window.locals().start + usize::from(index);
        self.tos_store_binding(window, tos, runtime, destination, mode, "owned local is vacant")
    }

    pub(in crate::engine::vm::stack) fn tos_store_parameter(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<FrameBinding>, Error> {
        tos.debug_validate(self, window);
        if tos.value.is_none() {
            event("tos.miss");
            return self.store_parameter_from_top_current(window, runtime, index, mode);
        }
        if usize::from(index) >= window.parameters().len() {
            return Err(Error::internal("owned parameter index is out of bounds"));
        }
        let destination = window.parameters().start + usize::from(index);
        self.tos_store_binding(window, tos, runtime, destination, mode, "owned parameter is vacant")
    }

    fn tos_store_binding(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        runtime: &Runtime,
        destination: usize,
        mode: StoreMode,
        vacant: &'static str,
    ) -> Result<Option<FrameBinding>, Error> {
        let destination = self.slots[destination].as_mut().ok_or_else(|| Error::internal(vacant))?;
        if !matches!(destination, FrameBinding::Direct(_)) {
            return Ok(None);
        }
        let depth = window.depth.checked_sub(1).ok_or_else(Self::operand_stack_underflow)?;
        let source = tos.value.as_ref().expect("authenticated scalar top");
        // Keep duplicates the scalar through the same copy boundary as C1.
        // All target/source checks and the copy precede the first mutation.
        let next = match mode {
            StoreMode::Keep => copy_value(runtime, source)?,
            StoreMode::Consume => tos.value.take().expect("authenticated scalar top"),
        };
        let old = std::mem::replace(destination, FrameBinding::Direct(next));
        if matches!(mode, StoreMode::Consume) {
            window.depth = depth;
            #[cfg(feature = "profiling")]
            { self.live_slots -= 1; }
        }
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        event("tos.hit");
        event(match mode {
            StoreMode::Consume => "direct_store.consume",
            StoreMode::Keep => "direct_store.keep",
        });
        Ok(Some(old))
    }
}
