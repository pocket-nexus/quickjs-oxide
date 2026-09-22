//! Direct operand-to-binding transactions in an authenticated frame window.

#[cfg(feature = "profiling")]
use super::{Cost, record_owned_storage};
use super::{Error, FrameBinding, FrameWindow, Runtime, SlotStore, copy_value};

#[derive(Clone, Copy)]
pub(in crate::engine::vm) enum StoreMode {
    Consume,
    Keep,
}

impl SlotStore {
    #[inline]
    pub(super) fn store_local_from_top_current(
        &mut self,
        window: &mut FrameWindow,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<FrameBinding>, Error> {
        if usize::from(index) >= window.locals().len() {
            return Err(Error::internal("owned local index is out of bounds"));
        }
        let destination = window.locals().start + usize::from(index);
        self.store_binding_from_top_current(
            window,
            runtime,
            destination,
            mode,
            "owned local is vacant",
        )
    }

    #[inline]
    pub(super) fn store_parameter_from_top_current(
        &mut self,
        window: &mut FrameWindow,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<FrameBinding>, Error> {
        if usize::from(index) >= window.parameters().len() {
            return Err(Error::internal("owned parameter index is out of bounds"));
        }
        let destination = window.parameters().start + usize::from(index);
        self.store_binding_from_top_current(
            window,
            runtime,
            destination,
            mode,
            "owned parameter is vacant",
        )
    }

    /// No owner is moved until destination, source and any retain have passed.
    /// A non-direct target declines unchanged; the handler retains authority
    /// over TDZ/const/captured bindings and release-readiness publication.
    /// The displaced owner is always returned for release by that handler.
    #[inline]
    fn store_binding_from_top_current(
        &mut self,
        window: &mut FrameWindow,
        runtime: &Runtime,
        destination: usize,
        mode: StoreMode,
        vacant: &'static str,
    ) -> Result<Option<FrameBinding>, Error> {
        // All parameter/local slots precede the operand region. Keep their
        // disjoint mutable views through preflight and commit, avoiding a
        // second destination lookup after removing the operand owner.
        let (bindings, operands) = self.slots.split_at_mut(window.operands().start);
        let destination = bindings[destination]
            .as_mut()
            .ok_or_else(|| Error::internal(vacant))?;
        if !matches!(destination, FrameBinding::Direct(_)) {
            return Ok(None);
        }
        let top = window
            .depth
            .checked_sub(1)
            .ok_or_else(Self::operand_stack_underflow)?;
        let source = &mut operands[top];
        let Some(FrameBinding::Direct(value)) = source.as_ref() else {
            return Err(Self::operand_slot_not_a_value());
        };
        let next = match mode {
            StoreMode::Keep => FrameBinding::Direct(copy_value(runtime, value)?),
            StoreMode::Consume => {
                // Move the existing binding directly, without a temporary
                // JsValue owner handed from pop() to replace_local().
                source.take().expect("authenticated direct operand")
            }
        };
        let old = std::mem::replace(destination, next);
        if matches!(mode, StoreMode::Consume) {
            window.depth -= 1;
            #[cfg(feature = "profiling")]
            {
                self.live_slots -= 1;
            }
        }
        #[cfg(feature = "profiling")]
        {
            // The incoming binding and displaced owner each move once. A
            // consume has no separate pop-to-temporary owner transfer.
            record_owned_storage(Cost::Move(2));
            crate::engine::api::profiling::record_owned_execution_event(match mode {
                StoreMode::Consume => "direct_store.consume",
                StoreMode::Keep => "direct_store.keep",
            });
        }
        Ok(Some(old))
    }
}

#[cfg(test)]
mod tests;
