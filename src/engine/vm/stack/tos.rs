//! Experimental top, owned only by an authenticated frame transaction.
#[cfg(feature = "profiling")]
use super::{Cost, record_owned_storage};
use super::{Error, FrameBinding, FrameWindow, JsValue, SlotStore};

/// The occupied entry owns the logical top; its authenticated backing hole is
/// empty. Ordinary pushes admit only six edge-free scalar kinds. The separate
/// numeric-output admission may move a String or heap BigInt into this same
/// hole; it neither retains the value nor borrows a Runtime. The historical
/// name distinguishes this small cache from the canonical operand storage.
pub(super) struct ScalarTos {
    value: Option<JsValue>,
    hole: usize,
}

impl ScalarTos {
    pub(super) fn new() -> Self {
        Self {
            value: None,
            hole: 0,
        }
    }

    #[cfg(any(test, oxide_owned_tos))]
    pub(super) fn has_owned_numeric_output(&self) -> bool {
        self.value.as_ref().is_some_and(owned_numeric_output)
    }

    /// Pure move for transaction Drop. In particular, do not call profiling
    /// TLS or add assertions here: this is also used while unwinding.
    pub(super) fn restore(&mut self, store: &mut SlotStore) {
        if self.value.is_some() {
            let destination = &mut store.slots[self.hole];
            *destination = self.value.take().map(FrameBinding::Direct);
        }
    }

    #[inline]
    fn debug_validate(&self, store: &SlotStore, window: &FrameWindow) {
        #[cfg(debug_assertions)]
        if let Some(value) = &self.value {
            let admitted = scalar(value);
            #[cfg(any(test, oxide_owned_tos))]
            let admitted = admitted || owned_numeric_output(value);
            debug_assert!(admitted);
            debug_assert!(window.depth > 0 && window.depth <= window.operands().len());
            debug_assert_eq!(self.hole, window.operands().start + window.depth - 1);
            debug_assert!(store.slots[self.hole].is_none());
        }
        #[cfg(not(debug_assertions))]
        let _ = (store, window);
    }

    pub(super) fn canonicalize(
        &mut self,
        store: &mut SlotStore,
        window: &FrameWindow,
        reason: &'static str,
    ) {
        self.debug_validate(store, window);
        if self.value.is_some() {
            self.restore(store);
            event("tos.spill");
            event(reason);
            event("tos.backing_operand_write");
        }
    }

    pub(super) fn peek<'a>(
        &'a self,
        store: &'a SlotStore,
        window: &FrameWindow,
        from_top: usize,
    ) -> Result<&'a JsValue, Error> {
        self.debug_validate(store, window);
        from_top
            .checked_add(1)
            .and_then(|offset| window.depth.checked_sub(offset))
            .ok_or_else(SlotStore::operand_stack_underflow)?;
        if from_top == 0
            && let Some(value) = &self.value
        {
            event("tos.hit");
            return Ok(value);
        }
        event("tos.miss");
        event("tos.backing_operand_read");
        store.peek_current(window, from_top)
    }

    fn install(&mut self, store: &mut SlotStore, index: usize, value: JsValue) -> (bool, bool) {
        let spilled = self.value.is_some();
        self.restore(store);
        if scalar(&value) {
            self.hole = index;
            self.value = Some(value);
            (spilled, true)
        } else {
            store.slots[index] = Some(FrameBinding::Direct(value));
            (spilled, false)
        }
    }
}

pub(super) fn scalar(value: &JsValue) -> bool {
    matches!(
        value,
        JsValue::Undefined
            | JsValue::Null
            | JsValue::Bool(_)
            | JsValue::Int(_)
            | JsValue::Float(_)
            | JsValue::ShortBigInt(_)
    )
}

#[cfg(any(test, oxide_owned_tos))]
fn owned_numeric_output(value: &JsValue) -> bool {
    matches!(value, JsValue::String(_) | JsValue::BigInt(_))
}

#[inline]
fn event(name: &'static str) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(name);
    #[cfg(not(feature = "profiling"))]
    let _ = name;
}

fn record_install((spilled, cached): (bool, bool)) {
    if spilled {
        event("tos.spill");
        event("tos.spill.push_next");
        event("tos.backing_operand_write");
    }
    event(if cached {
        "tos.commit"
    } else {
        "tos.backing_operand_write"
    });
}

impl SlotStore {
    pub(super) fn tos_push(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        value: JsValue,
    ) -> Result<(), Error> {
        tos.debug_validate(self, window);
        let index = self.operand_push_index(window)?;
        let installed = tos.install(self, index, value);
        self.tos_pushed(window);
        record_install(installed);
        Ok(())
    }

    pub(super) fn tos_push_pending(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        value: &mut Option<JsValue>,
    ) -> Result<(), Error> {
        tos.debug_validate(self, window);
        let index = self.operand_push_index(window)?;
        // Preserve the pending contract: even a malformed absent carrier is
        // diagnosed before disturbing a previously cached top.
        let next = value.take().expect("pending operand owner");
        let installed = tos.install(self, index, next);
        self.tos_pushed(window);
        record_install(installed);
        Ok(())
    }

    /// Move an already-owned numeric result only after authenticating its
    /// backing destination. Decline and every error leave the pending owner
    /// and any previously cached top untouched.
    #[cfg(any(test, oxide_owned_tos))]
    pub(super) fn tos_cache_numeric_output(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
        value: &mut Option<JsValue>,
    ) -> Result<bool, Error> {
        if !value.as_ref().is_some_and(owned_numeric_output) {
            return Ok(false);
        }
        tos.debug_validate(self, window);
        let index = self.operand_push_index(window)?;
        let spilled = tos.value.is_some();
        let next = value.take().expect("authenticated owning numeric output");
        tos.restore(self);
        tos.hole = index;
        tos.value = Some(next);
        self.tos_pushed(window);
        record_install((spilled, true));
        event("tos.owned_numeric_output");
        Ok(true)
    }

    fn tos_pushed(&mut self, window: &mut FrameWindow) {
        window.depth += 1;
        #[cfg(feature = "profiling")]
        {
            self.live_slots += 1;
            record_owned_storage(Cost::Move(1));
            self.record_occupancy();
        }
    }

    pub(super) fn tos_pop(
        &mut self,
        window: &mut FrameWindow,
        tos: &mut ScalarTos,
    ) -> Result<JsValue, Error> {
        tos.debug_validate(self, window);
        if tos.value.is_none() {
            event("tos.miss");
            event("tos.backing_operand_read");
            let value = self.pop_current(window)?;
            event("tos.backing_operand_write");
            return Ok(value);
        }
        let depth = window
            .depth
            .checked_sub(1)
            .ok_or_else(Self::operand_stack_underflow)?;
        // The index and empty hole were certified by the successful admission.
        let value = tos.value.take().expect("authenticated cached top");
        window.depth = depth;
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= 1;
            record_owned_storage(Cost::Move(1));
        }
        event("tos.hit");
        Ok(value)
    }
}

mod number;
#[cfg(test)]
mod owned_tests;
mod store;
#[cfg(test)]
mod tests;
