//! The runtime's single state owner. Borrowing does not create another owner.

use super::RuntimeState;
use std::cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut};

pub(crate) struct StateStorage(Option<RefCell<RuntimeState>>);

impl StateStorage {
    pub(crate) fn new(state: RuntimeState) -> Self {
        Self(Some(RefCell::new(state)))
    }

    fn cell(&self) -> &RefCell<RuntimeState> {
        self.0.as_ref().expect("live runtime owns its state")
    }

    #[inline]
    pub(crate) fn borrow(&self) -> Ref<'_, RuntimeState> {
        #[cfg(feature = "profiling")]
        super::super::super::api::profiling::record_runtime_event(
            "runtime.state.borrow",
            "core.state.borrow",
        );
        self.cell().borrow()
    }

    #[inline]
    pub(crate) fn borrow_mut(&self) -> RefMut<'_, RuntimeState> {
        #[cfg(feature = "profiling")]
        super::super::super::api::profiling::record_runtime_event(
            "runtime.state.borrow_mut",
            "core.state.borrow_mut",
        );
        self.cell().borrow_mut()
    }

    #[inline]
    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, RuntimeState>, BorrowError> {
        #[cfg(feature = "profiling")]
        super::super::super::api::profiling::record_runtime_event(
            "runtime.state.try_borrow",
            "core.state.try_borrow",
        );
        self.cell().try_borrow()
    }

    #[inline]
    pub(crate) fn try_borrow_mut(&self) -> Result<RefMut<'_, RuntimeState>, BorrowMutError> {
        #[cfg(feature = "profiling")]
        super::super::super::api::profiling::record_runtime_event(
            "runtime.state.try_borrow_mut",
            "core.state.try_borrow_mut",
        );
        self.cell().try_borrow_mut()
    }

    pub(crate) fn get_mut(&mut self) -> &mut RuntimeState {
        self.0
            .as_mut()
            .expect("live runtime owns its state")
            .get_mut()
    }
}
