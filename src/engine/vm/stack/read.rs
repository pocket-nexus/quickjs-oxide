//! Concrete property operand publication shared by named and computed reads.
use super::{FrameTransaction, FrameWindow, SlotStore};
use crate::engine::{
    api::Error,
    heap::runtime::{RuntimeState, owned_values::OwnedValueGuard},
    value::JsValue,
    vm::exception::runtime_error_to_vm_error,
};
use std::cell::Cell;

/// The owners remain in the actual callback/Query. This borrowed projection
/// describes only the instruction's original operand prefix.
pub(in crate::engine::vm) struct ReadOperandCommit<'a> {
    pub(in crate::engine::vm) consume: u8,
    pub(in crate::engine::vm) keep_receiver: bool,
    pub(in crate::engine::vm) retained_key: Option<&'a mut Option<JsValue>>,
    pub(in crate::engine::vm) prefix_published: Option<&'a mut bool>,
}
impl ReadOperandCommit<'_> {
    pub(in crate::engine::vm) fn named(keep_receiver: bool) -> Self {
        Self {
            consume: 1,
            keep_receiver,
            retained_key: None,
            prefix_published: None,
        }
    }
}

impl SlotStore {
    pub(super) fn commit_property_read_operands_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        parent: &mut FrameWindow,
        preserved_receiver: &mut Option<JsValue>,
        mut commit: ReadOperandCommit<'_>,
    ) -> Result<(), Error> {
        if commit.consume > 2 || preserved_receiver.is_none() {
            return Err(Error::internal("property callback lost its operand owners"));
        }
        if commit.consume != 0 {
            self.peek_current(parent, usize::from(commit.consume - 1))?;
        }
        // Selected this is already guarded/published. Retire original key then
        // base; a destructive failure poisons before any suffix is traversed.
        for _ in 0..commit.consume {
            let value = self.pop_current(parent).expect("checked property operand");
            state
                .release_owned_jsvalue(poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
        }
        if commit.keep_receiver {
            // Failed publication leaves the owner in its receiving record.
            let mut slots = super::FrameSlots {
                store: self,
                window: parent,
            };
            slots.push_pending(preserved_receiver)?;
        } else {
            state
                .release_owned_jsvalue(
                    poisoned,
                    preserved_receiver.take().expect("preserved receiver"),
                )
                .map_err(runtime_error_to_vm_error)?;
        }
        if let Some(key) = commit.retained_key.as_mut() {
            let mut slots = super::FrameSlots {
                store: self,
                window: parent,
            };
            if key.is_some() {
                slots.push_pending(key)?;
            }
        }
        if let Some(published) = commit.prefix_published {
            *published = true;
        }
        Ok(())
    }
}

/// Data reads keep the actual base owner until after keeper/result publication.
/// Failed output publication retires the output before the base. No FIFO or
/// public root is used while this admitted State lease is held.
#[allow(clippy::too_many_arguments)]
pub(in crate::engine::vm) fn publish_property_read_result(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    transaction: &mut FrameTransaction<'_>,
    resume_pc: &mut usize,
    next_pc: usize,
    preserved_receiver: &mut Option<JsValue>,
    retained_key: &mut Option<JsValue>,
    keep_receiver: bool,
    consume: u8,
    value: JsValue,
) -> Result<(), Error> {
    let mut output = OwnedValueGuard::new(state, poisoned, value);
    let (state, output) = output.parts();
    if consume > 2 || (consume == 0 && preserved_receiver.is_none()) {
        return Err(Error::internal("property read lost its operand owners"));
    }
    let discarded_key = {
        let mut slots = transaction.slots();
        if consume != 0 {
            slots.peek(usize::from(consume - 1))?;
        }
        let key = (consume == 2).then(|| slots.pop().expect("checked property key"));
        if consume != 0 {
            let base = slots.pop().expect("checked property base");
            if preserved_receiver.is_none() {
                *preserved_receiver = Some(base);
            } else {
                // This case is used only by already prepared legacy reads.
                state
                    .release_owned_jsvalue(poisoned, base)
                    .map_err(runtime_error_to_vm_error)?;
            }
        }
        key
    };
    let mut base = OwnedValueGuard::new(
        state,
        poisoned,
        preserved_receiver.take().expect("read base owner"),
    );
    let (state, base) = base.parts();
    if let Some(key) = discarded_key {
        state
            .release_owned_jsvalue(poisoned, key)
            .map_err(runtime_error_to_vm_error)?;
    }
    let result = {
        let mut slots = transaction.slots();
        (|| {
            if keep_receiver {
                slots.push_pending(base)?;
            }
            if retained_key.is_some() {
                slots.push_pending(retained_key)?;
            }
            *resume_pc = next_pc;
            slots.push_pending(output)
        })()
    };
    if !poisoned.get()
        && let Some(value) = output.take()
    {
        state
            .release_owned_jsvalue(poisoned, value)
            .map_err(runtime_error_to_vm_error)?;
    }
    if !poisoned.get()
        && let Some(value) = base.take()
    {
        state
            .release_owned_jsvalue(poisoned, value)
            .map_err(runtime_error_to_vm_error)?;
    }
    result
}
