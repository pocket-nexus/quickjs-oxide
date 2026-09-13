//! Exclusive frame windows in reusable owning storage.
//!
//! A window contains indices, never addresses into the arena. Growing another
//! window therefore cannot invalidate a retained frame identity. Empty slots
//! have no Value owner; TDZ is the distinct FrameBinding::Uninitialized state.

use crate::engine::api::error::Error;
#[cfg(feature = "profiling")]
use crate::engine::api::profiling::{OwnedStorageEvent as Cost, record_owned_storage};
use crate::engine::api::runtime::Runtime;
use crate::engine::code::function::layout::FrameLayout;
use crate::engine::value::Value;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use std::ops::Range;
use std::rc::Rc;

pub(in crate::engine::vm) struct SlotStore {
    slots: Vec<Option<FrameBinding>>,
    owner: Rc<()>,
    next_window: u64,
    windows: Vec<u64>,
    limit: usize,
    #[cfg(feature = "profiling")]
    live_slots: usize,
}

/// Not Clone: releasing a frame consumes its authority over the window.
pub(in crate::engine::vm) struct FrameWindow {
    owner: Rc<()>,
    id: u64,
    whole: Range<usize>,
    original_arguments: Range<usize>,
    parameters: Range<usize>,
    locals: Range<usize>,
    operands: Range<usize>,
    depth: usize,
}

pub(in crate::engine::vm) struct FrameStorage {
    pub original_arguments: Vec<Value>,
    pub parameters: Vec<FrameBinding>,
    pub locals: Vec<FrameBinding>,
    pub operands: Vec<Value>,
}

impl SlotStore {
    pub(in crate::engine::vm) fn new(limit: usize) -> Self {
        Self {
            slots: Vec::new(),
            owner: Rc::new(()),
            next_window: 1,
            windows: Vec::new(),
            limit,
            #[cfg(feature = "profiling")]
            live_slots: 0,
        }
    }

    /// All capacity checks precede ownership installation. Arguments already
    /// contain the writable, padded parameter bindings; the immutable original
    /// snapshot remains a separate range even for strict/non-simple parameters.
    pub(in crate::engine::vm) fn push_frame(
        &mut self,
        layout: &FrameLayout<'_>,
        storage: FrameStorage,
    ) -> Result<FrameWindow, Error> {
        if storage.parameters.len() != layout.argument_slots(storage.original_arguments.len())
            || storage.locals.len() != layout.locals().len()
            || storage.operands.len() > layout.operand_capacity()
        {
            return Err(Error::internal(
                "owned frame storage disagrees with its published layout",
            ));
        }
        let next_window = self
            .next_window
            .checked_add(1)
            .ok_or_else(|| Error::internal("frame window identity exhausted"))?;
        let base = self.slots.len();
        let original_end = base.checked_add(storage.original_arguments.len());
        let parameters_end = original_end.and_then(|n| n.checked_add(storage.parameters.len()));
        let locals_end = parameters_end.and_then(|n| n.checked_add(storage.locals.len()));
        let end = locals_end
            .and_then(|n| n.checked_add(layout.operand_capacity()))
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| Error::internal("execution slot limit exceeded"))?;
        #[cfg(feature = "profiling")]
        let capacity_before = self.slots.capacity();
        self.slots
            .try_reserve(end - base)
            .map_err(|_| Error::internal("execution slot allocation failed"))?;
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::SlotCapacity {
            before: capacity_before,
            after: self.slots.capacity(),
        });
        self.windows
            .try_reserve(1)
            .map_err(|_| Error::internal("execution window allocation failed"))?;
        let original_end = original_end.unwrap();
        let parameters_end = parameters_end.unwrap();
        let locals_end = locals_end.unwrap();
        let depth = storage.operands.len();
        #[cfg(feature = "profiling")]
        let installed = storage.original_arguments.len()
            + storage.parameters.len()
            + storage.locals.len()
            + depth;
        self.slots.extend(
            storage
                .original_arguments
                .into_iter()
                .map(|value| Some(FrameBinding::Direct(value))),
        );
        self.slots.extend(storage.parameters.into_iter().map(Some));
        self.slots.extend(storage.locals.into_iter().map(Some));
        self.slots.extend(
            storage
                .operands
                .into_iter()
                .map(|value| Some(FrameBinding::Direct(value))),
        );
        self.slots.resize_with(end, || None);
        let id = self.next_window;
        self.next_window = next_window;
        self.windows.push(id);
        #[cfg(feature = "profiling")]
        {
            self.live_slots += installed;
            record_owned_storage(Cost::Initialize(end - base));
            record_owned_storage(Cost::Move(installed));
            self.record_occupancy();
        }
        Ok(FrameWindow {
            owner: self.owner.clone(),
            id,
            whole: base..end,
            original_arguments: base..original_end,
            parameters: original_end..parameters_end,
            locals: parameters_end..locals_end,
            operands: locals_end..end,
            depth,
        })
    }

    #[cfg(feature = "profiling")]
    fn record_occupancy(&self) {
        record_owned_storage(Cost::Occupancy {
            reserved: self.slots.len(),
            live: self.live_slots,
        });
    }

    fn check_current(&self, window: &FrameWindow) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.owner, &window.owner)
            || self.windows.last() != Some(&window.id)
            || self.slots.len() != window.whole.end
        {
            return Err(Error::internal(
                "frame window is not the active arena window",
            ));
        }
        Ok(())
    }

    pub(in crate::engine::vm) fn depth(&self, window: &FrameWindow) -> usize {
        window.depth
    }

    pub(in crate::engine::vm) fn peek(
        &self,
        window: &FrameWindow,
        from_top: usize,
    ) -> Result<&Value, Error> {
        self.check_current(window)?;
        let offset = from_top
            .checked_add(1)
            .and_then(|offset| window.depth.checked_sub(offset))
            .ok_or_else(|| Error::internal("owned operand stack underflow"))?;
        match &self.slots[window.operands.start + offset] {
            Some(FrameBinding::Direct(value)) => Ok(value),
            _ => Err(Error::internal("owned operand slot is not a value")),
        }
    }

    /// Move an owned value into an already reserved, empty operand slot.
    pub(in crate::engine::vm) fn push(
        &mut self,
        window: &mut FrameWindow,
        value: Value,
    ) -> Result<(), Error> {
        self.check_current(window)?;
        if window.depth >= window.operands.len() {
            return Err(Error::internal(
                "owned operand stack exceeds verified capacity",
            ));
        }
        let slot = &mut self.slots[window.operands.start + window.depth];
        if slot.is_some() {
            return Err(Error::internal(
                "owned operand push would replace a live value",
            ));
        }
        *slot = Some(FrameBinding::Direct(value));
        window.depth += 1;
        #[cfg(feature = "profiling")]
        {
            self.live_slots += 1;
            record_owned_storage(Cost::Move(1));
            self.record_occupancy();
        }
        Ok(())
    }

    /// Rotate existing owners in place. No retain, release, or allocation occurs.
    pub(in crate::engine::vm) fn rotate_operands(
        &mut self,
        window: &FrameWindow,
        skip_top: usize,
        count: usize,
        left: bool,
    ) -> Result<(), Error> {
        let extent = skip_top
            .checked_add(count)
            .filter(|_| count > 0)
            .ok_or_else(|| Error::internal("invalid owned operand rotation"))?;
        self.peek(window, extent - 1)?;
        let end = window.operands.start + window.depth - skip_top;
        #[cfg(feature = "profiling")]
        if count > 1 {
            record_owned_storage(Cost::Move(count));
        }
        let values = &mut self.slots[end - count..end];
        if left {
            values.rotate_left(1);
        } else {
            values.rotate_right(1);
        }
        Ok(())
    }

    /// Copy once, then install by moving owners inside the reserved window.
    /// All shape/capacity checks and the fallible retain precede mutation.
    pub(in crate::engine::vm) fn insert_copy(
        &mut self,
        window: &mut FrameWindow,
        source_from_top: usize,
        destination_from_top: usize,
    ) -> Result<(), Error> {
        self.peek(window, source_from_top)?;
        if destination_from_top > window.depth || window.depth >= window.operands.len() {
            return Err(Error::internal("owned insertion exceeds verified capacity"));
        }
        let copied = copy_value(self.peek(window, source_from_top)?)?;
        self.push(window, copied)?;
        self.rotate_operands(window, 0, destination_from_top + 1, false)
    }

    /// Retain a sequence into reserved slots. If a retain fails, the committed
    /// prefix stays inside the logical window for the driver's error cleanup.
    /// This error is terminal, never a bridge/retry: no local rollback may drop
    /// roots and unexpectedly drain deferred releases inside the run loop.
    pub(in crate::engine::vm) fn duplicate_operands(
        &mut self,
        window: &mut FrameWindow,
        count: usize,
    ) -> Result<(), Error> {
        let source = count
            .checked_sub(1)
            .ok_or_else(|| Error::internal("empty owned operand duplication"))?;
        self.peek(window, source)?;
        if count > window.operands.len() - window.depth {
            return Err(Error::internal(
                "owned duplication exceeds verified capacity",
            ));
        }
        for _ in 0..count {
            // As depth grows, this fixed offset visits the next original slot.
            let copied = copy_value(self.peek(window, source)?)?;
            self.push(window, copied)?;
        }
        Ok(())
    }

    /// Logical pop removes ownership immediately. No dead value survives above sp.
    pub(in crate::engine::vm) fn pop(&mut self, window: &mut FrameWindow) -> Result<Value, Error> {
        self.peek(window, 0)?;
        window.depth -= 1;
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= 1;
            record_owned_storage(Cost::Move(1));
        }
        let Some(FrameBinding::Direct(value)) =
            self.slots[window.operands.start + window.depth].take()
        else {
            unreachable!("peek authenticated this slot before the move")
        };
        Ok(value)
    }

    /// Replace an authenticated live operand without changing its depth or neighbors.
    pub(in crate::engine::vm) fn replace_operand(
        &mut self,
        window: &FrameWindow,
        from_top: usize,
        value: Value,
    ) -> Result<Value, Error> {
        self.peek(window, from_top)?;
        let index = window.operands.start + window.depth - from_top - 1;
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        let Some(FrameBinding::Direct(previous)) =
            self.slots[index].replace(FrameBinding::Direct(value))
        else {
            unreachable!()
        };
        Ok(previous)
    }

    /// Preflight and release one operand without moving any other owner. A
    /// false result leaves both the value and logical depth untouched.
    pub(in crate::engine::vm) fn release_operand(
        &mut self,
        window: &FrameWindow,
        from_top: usize,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.peek(window, from_top)?;
        let index = window.operands.start + window.depth - from_top - 1;
        let Some(FrameBinding::Direct(value)) = &mut self.slots[index] else {
            unreachable!()
        };
        runtime
            .try_release_slot_value(value)
            .map_err(runtime_error_to_vm_error)
    }

    pub(super) fn binding_counts(&self, window: &FrameWindow) -> Result<(usize, usize), Error> {
        self.check_current(window)?;
        Ok((window.locals.len(), window.parameters.len()))
    }

    pub(in crate::engine::vm) fn local(
        &self,
        window: &FrameWindow,
        index: u16,
    ) -> Result<&FrameBinding, Error> {
        self.check_current(window)?;
        if usize::from(index) >= window.locals.len() {
            return Err(Error::internal("owned local index is out of bounds"));
        }
        self.slots[window.locals.start + usize::from(index)]
            .as_ref()
            .ok_or_else(|| Error::internal("owned local is vacant"))
    }

    /// The displaced owner is returned for explicit release at the caller's
    /// observation boundary. Installing the replacement never drops it implicitly.
    /// Borrow only for a NoJS binding operation; never retain across frame pushes.
    pub(in crate::engine::vm) fn local_mut(
        &mut self,
        window: &FrameWindow,
        index: u16,
    ) -> Result<&mut FrameBinding, Error> {
        self.local(window, index)?;
        Ok(self.slots[window.locals.start + usize::from(index)]
            .as_mut()
            .unwrap())
    }

    pub(in crate::engine::vm) fn parameter_mut(
        &mut self,
        window: &FrameWindow,
        index: u16,
    ) -> Result<&mut FrameBinding, Error> {
        self.parameter(window, index)?;
        Ok(self.slots[window.parameters.start + usize::from(index)]
            .as_mut()
            .unwrap())
    }

    pub(in crate::engine::vm) fn replace_local(
        &mut self,
        window: &FrameWindow,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.local(window, index)?;
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        Ok(self.slots[window.locals.start + usize::from(index)]
            .replace(value)
            .unwrap())
    }

    /// Snapshot the current parameter bindings only across the actual argv
    /// span. Default derived forwarding must not include padded formal slots.
    pub(in crate::engine::vm) fn snapshot_actual_arguments(
        &self,
        window: &FrameWindow,
        runtime: &crate::engine::api::runtime::Runtime,
    ) -> Result<Vec<Value>, Error> {
        self.snapshot_argument_tail(window, runtime, 0)
    }

    pub(in crate::engine::vm) fn actual_argument_count(
        &self,
        window: &FrameWindow,
    ) -> Result<usize, Error> {
        self.check_current(window)?;
        Ok(window.original_arguments.len())
    }

    pub(in crate::engine::vm) fn snapshot_argument_tail(
        &self,
        window: &FrameWindow,
        runtime: &Runtime,
        start: usize,
    ) -> Result<Vec<Value>, Error> {
        self.check_current(window)?;
        let count = window.original_arguments.len();
        if count > window.parameters.len() || start > window.parameters.len() {
            return Err(Error::internal(
                "actual argument count exceeds parameter window",
            ));
        }
        let start = start.min(count);
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(count - start)
            .map_err(|_| Error::internal("argument snapshot allocation failed"))?;
        for index in window.parameters.start + start..window.parameters.start + count {
            let binding = self.slots[index]
                .as_ref()
                .ok_or_else(|| Error::internal("owned parameter is vacant"))?;
            arguments.push(crate::engine::vm::bindings::read_frame_binding(
                runtime, binding,
            )?);
        }
        Ok(arguments)
    }

    pub(in crate::engine::vm) fn parameter(
        &self,
        window: &FrameWindow,
        index: u16,
    ) -> Result<&FrameBinding, Error> {
        self.check_current(window)?;
        if usize::from(index) >= window.parameters.len() {
            return Err(Error::internal("owned parameter index is out of bounds"));
        }
        self.slots[window.parameters.start + usize::from(index)]
            .as_ref()
            .ok_or_else(|| Error::internal("owned parameter is vacant"))
    }

    pub(in crate::engine::vm) fn replace_parameter(
        &mut self,
        window: &FrameWindow,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.parameter(window, index)?;
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        Ok(self.slots[window.parameters.start + usize::from(index)]
            .replace(value)
            .unwrap())
    }

    /// One-way temporary migration handoff. No serialization or extra retain:
    /// the caller receives the very owners which occupied this window.
    pub(in crate::engine::vm) fn take_frame(
        &mut self,
        window: FrameWindow,
    ) -> Result<FrameStorage, Error> {
        self.check_current(&window)?;
        // Reserve every destination before removing any source owner. A failed
        // migration allocation leaves the complete window rooted in this store.
        let mut original_arguments = Vec::new();
        let mut parameters = Vec::new();
        let mut locals = Vec::new();
        let mut operands = Vec::new();
        original_arguments
            .try_reserve_exact(window.original_arguments.len())
            .map_err(|_| Error::internal("original argument handoff allocation failed"))?;
        parameters
            .try_reserve_exact(window.parameters.len())
            .map_err(|_| Error::internal("parameter handoff allocation failed"))?;
        locals
            .try_reserve_exact(window.locals.len())
            .map_err(|_| Error::internal("local handoff allocation failed"))?;
        operands
            .try_reserve_exact(window.depth)
            .map_err(|_| Error::internal("operand handoff allocation failed"))?;
        for index in window.original_arguments.clone() {
            let Some(FrameBinding::Direct(value)) = self.slots[index].take() else {
                return Err(Error::internal("original argument is not an owned value"));
            };
            original_arguments.push(value);
        }
        for index in window.parameters.clone() {
            parameters.push(self.slots[index].take().unwrap());
        }
        for index in window.locals.clone() {
            locals.push(self.slots[index].take().unwrap());
        }
        for index in window.operands.start..window.operands.start + window.depth {
            let Some(FrameBinding::Direct(value)) = self.slots[index].take() else {
                return Err(Error::internal("operand is not an owned value"));
            };
            operands.push(value);
        }
        #[cfg(feature = "profiling")]
        {
            let moved = original_arguments.len() + parameters.len() + locals.len() + operands.len();
            self.live_slots -= moved;
            record_owned_storage(Cost::Move(moved));
        }
        self.slots.truncate(window.whole.start);
        self.windows.pop();
        Ok(FrameStorage {
            original_arguments,
            parameters,
            locals,
            operands,
        })
    }

    /// The driver must keep the completion/pending result rooted before clearing.
    pub(in crate::engine::vm) fn clear_frame(&mut self, window: FrameWindow) -> Result<(), Error> {
        self.check_current(&window)?;
        #[cfg(feature = "profiling")]
        {
            let cleared = self.slots[window.whole.clone()]
                .iter()
                .filter(|slot| slot.is_some())
                .count();
            self.live_slots -= cleared;
            record_owned_storage(Cost::Clear(cleared));
        }
        self.slots.truncate(window.whole.start);
        self.windows.pop();
        Ok(())
    }
}

/// The running stack's copy boundary. Object retain is fallible and neither
/// drains references nor calls JS; primitive Rc copies preserve representation.
/// Releases are separate, so a failed retain cannot repeat a committed release.
pub(in crate::engine::vm) fn copy_value(value: &Value) -> Result<Value, Error> {
    let copied = match value {
        Value::Undefined => Value::Undefined,
        Value::Null => Value::Null,
        Value::Bool(value) => Value::Bool(*value),
        Value::Int(value) => Value::Int(*value),
        Value::Float(value) => Value::Float(*value),
        Value::String(value) => Value::String(value.clone()),
        Value::BigInt(value) => Value::BigInt(value.clone()),
        Value::Object(value) => Value::Object(
            value
                .try_clone()
                .map_err(|error| Error::internal(error.to_string()))?,
        ),
        Value::Symbol(value) => Value::Symbol(
            value
                .try_clone()
                .map_err(|error| Error::internal(error.to_string()))?,
        ),
    };
    #[cfg(feature = "profiling")]
    record_owned_storage(Cost::Copy {
        heap_root: matches!(value, Value::Object(_) | Value::Symbol(_)),
    });
    Ok(copied)
}

#[cfg(feature = "profiling")]
impl Drop for SlotStore {
    fn drop(&mut self) {
        record_owned_storage(Cost::Clear(self.live_slots));
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameStorage, SlotStore};
    use crate::engine::api::Runtime;
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::value::Value;
    use crate::engine::vm::bindings::FrameBinding;

    fn empty_storage() -> FrameStorage {
        FrameStorage {
            original_arguments: Vec::new(),
            parameters: Vec::new(),
            locals: Vec::new(),
            operands: Vec::new(),
        }
    }

    #[test]
    fn parent_window_survives_growth_and_last_result_outlives_clear() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 1;
        let mut slots = SlotStore::new(8192);
        let mut parent = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        slots.push(&mut parent, Value::Object(object)).unwrap();
        let original_capacity = slots.slots.capacity();
        owner.metadata.max_stack = 4096;
        let child = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        assert!(slots.slots.capacity() > original_capacity);
        assert!(
            slots.peek(&parent, 0).is_err(),
            "an inactive parent cannot access the current window"
        );
        slots.clear_frame(child).unwrap();
        let result = slots.pop(&mut parent).unwrap();
        assert!(slots.slots[parent.operands.start].is_none());
        slots.clear_frame(parent).unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        drop(result);
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
        assert!(slots.slots.is_empty());
        assert!(slots.slots.capacity() >= original_capacity);
    }

    #[test]
    fn permutations_move_owners_and_failed_insertion_preserves_the_window() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 6;
        let mut slots = SlotStore::new(6);
        let mut window = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        for value in [
            Value::Int(0),
            Value::Int(1),
            Value::Int(2),
            Value::Object(object),
            Value::Int(4),
        ] {
            slots.push(&mut window, value).unwrap();
        }
        let capacity = slots.slots.capacity();
        slots.rotate_operands(&window, 1, 4, false).unwrap();
        slots.rotate_operands(&window, 0, 4, true).unwrap();
        slots.insert_copy(&mut window, 4, 5).unwrap();
        assert_eq!(slots.slots.capacity(), capacity);
        assert!(slots.insert_copy(&mut window, 0, 0).is_err());
        assert!(slots.rotate_operands(&window, 1, 6, false).is_err());
        assert!(
            slots
                .rotate_operands(&window, usize::MAX, 2, false)
                .is_err()
        );
        assert_eq!(slots.depth(&window), 6);
        let mut values = slots.take_frame(window).unwrap().operands;
        for expected in [0, 4, 2, 1] {
            assert_eq!(values.pop().unwrap(), Value::Int(expected));
        }
        assert!(
            values
                .iter()
                .all(|value| matches!(value, Value::Object(root) if root.object_id() == id))
        );
        drop(values.pop());
        assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        drop(values);
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
        assert!(slots.slots.is_empty());
    }

    #[test]
    fn duplicate_sequence_keeps_order_and_independent_object_owners() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 6;
        let mut slots = SlotStore::new(6);
        let mut window = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        for value in [Value::Int(7), Value::Object(object), Value::Int(9)] {
            slots.push(&mut window, value).unwrap();
        }
        slots.duplicate_operands(&mut window, 3).unwrap();
        assert!(slots.duplicate_operands(&mut window, 3).is_err());
        assert_eq!(slots.depth(&window), 6);
        for _ in 0..2 {
            assert_eq!(slots.pop(&mut window).unwrap(), Value::Int(9));
            assert!(
                matches!(slots.pop(&mut window).unwrap(), Value::Object(root) if root.object_id() == id)
            );
            assert_eq!(slots.pop(&mut window).unwrap(), Value::Int(7));
        }
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
        slots.clear_frame(window).unwrap();
    }

    #[test]
    fn failed_sequence_retain_leaves_committed_prefix_for_driver_cleanup() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 6;
        let mut slots = SlotStore::new(6);
        let mut window = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        let text = Value::String(crate::engine::value::JsString::from_static(
            "retained prefix",
        ));
        for value in [text.clone(), Value::Object(object), Value::Int(9)] {
            slots.push(&mut window, value).unwrap();
        }
        {
            // Rc-backed String copies succeed; the following Object retain
            // must fail because it needs a mutable heap borrow.
            let state = runtime.0.state.borrow();
            assert!(slots.duplicate_operands(&mut window, 3).is_err());
            assert_eq!(slots.depth(&window), 4);
            assert_eq!(slots.peek(&window, 0).unwrap(), &text);
            assert_eq!(slots.peek(&window, 1).unwrap(), &Value::Int(9));
            assert!(state.heap.object(id).is_ok());
            assert!(!runtime.0.deferred_references.has_pending());
        }
        slots.clear_frame(window).unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }

    #[test]
    #[cfg(feature = "profiling")]
    fn cost_collection_distinguishes_reuse_from_initialization_and_live_peaks() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 2;
        let mut slots = SlotStore::new(8);
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut previous = None;
        for _ in 0..2 {
            let mut window = slots
                .push_frame(&owner.frame_layout(), empty_storage())
                .unwrap();
            slots.push(&mut window, Value::Int(1)).unwrap();
            slots.insert_copy(&mut window, 0, 0).unwrap();
            slots.clear_frame(window).unwrap();
            let cost = profile.snapshot().owned_storage;
            assert_eq!(cost.slot_capacity_growths, 1);
            assert_eq!(cost.maximum_reserved_slots, 2);
            assert_eq!(cost.maximum_live_slots, 2);
            if let Some(before) = previous {
                let before: crate::engine::api::profiling::OwnedStorageCost = before;
                assert_eq!(cost.maximum_slot_capacity, before.maximum_slot_capacity);
                assert_eq!(cost.slots_initialized, before.slots_initialized * 2);
                assert_eq!(cost.slot_clears, before.slot_clears * 2);
                assert_eq!(cost.value_copies, before.value_copies * 2);
            }
            previous = Some(cost);
        }
        let before_drop = profile.snapshot();
        drop(slots);
        assert_eq!(
            profile.snapshot(),
            before_drop,
            "cleared arena drop must not count owners twice"
        );
    }

    #[test]
    fn original_arguments_do_not_alias_writable_parameters() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.argument_count = 2;
        let mut slots = SlotStore::new(32);
        let window = slots
            .push_frame(
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: vec![Value::Int(1)],
                    parameters: vec![
                        FrameBinding::Direct(Value::Int(1)),
                        FrameBinding::Direct(Value::Undefined),
                    ],
                    ..empty_storage()
                },
            )
            .unwrap();
        let old = slots
            .replace_parameter(&window, 0, FrameBinding::Direct(Value::Int(2)))
            .unwrap();
        assert!(matches!(old, FrameBinding::Direct(Value::Int(1))));
        let storage = slots.take_frame(window).unwrap();
        assert_eq!(storage.original_arguments, vec![Value::Int(1)]);
        assert!(matches!(
            storage.parameters[0],
            FrameBinding::Direct(Value::Int(2))
        ));
        assert!(matches!(
            storage.parameters[1],
            FrameBinding::Direct(Value::Undefined)
        ));
    }

    #[test]
    fn distinct_arenas_reject_matching_numeric_window_ids() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 1;
        let mut first = SlotStore::new(4);
        let mut second = SlotStore::new(4);
        let mut a = first
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        let mut b = second
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        first.push(&mut a, Value::Int(1)).unwrap();
        second.push(&mut b, Value::Int(2)).unwrap();
        assert!(second.peek(&a, 0).is_err());
        assert!(first.peek(&b, 0).is_err());
        assert_eq!(first.pop(&mut a).unwrap(), Value::Int(1));
        assert_eq!(second.pop(&mut b).unwrap(), Value::Int(2));
    }

    #[test]
    fn failed_capacity_and_shape_checks_do_not_change_live_windows() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 1;
        let mut slots = SlotStore::new(1);
        let mut window = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        slots.push(&mut window, Value::Int(7)).unwrap();
        assert!(slots.push(&mut window, Value::Int(8)).is_err());
        assert!(
            slots
                .push_frame(&owner.frame_layout(), empty_storage())
                .is_err()
        );
        assert!(
            slots
                .push_frame(
                    &owner.frame_layout(),
                    FrameStorage {
                        locals: vec![FrameBinding::Uninitialized],
                        ..empty_storage()
                    }
                )
                .is_err()
        );
        assert!(slots.peek(&window, usize::MAX).is_err());
        assert_eq!(slots.depth(&window), 1);
        assert_eq!(slots.pop(&mut window).unwrap(), Value::Int(7));
        assert!(slots.pop(&mut window).is_err());
        slots.clear_frame(window).unwrap();
        let reused = slots
            .push_frame(&owner.frame_layout(), empty_storage())
            .unwrap();
        assert_eq!(slots.depth(&reused), 0);
        assert!(slots.slots.iter().all(Option::is_none));
    }
}
