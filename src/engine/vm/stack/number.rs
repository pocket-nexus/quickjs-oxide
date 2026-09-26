//! Pure Number transactions inside an authenticated continuous window.
use super::window::{NumberUpdate, NumericDestination};
#[cfg(feature = "profiling")]
use super::{Cost, record_owned_storage};
use super::{Error, FrameBinding, FrameWindow, SlotStore};
use crate::engine::code::fusion::DirectSlot;
use crate::engine::value::number::operations::Number;

fn direct_slot_index(window: &FrameWindow, slot: DirectSlot) -> Option<usize> {
    let (region, index) = match slot {
        DirectSlot::Local(index) => (window.locals(), usize::from(index)),
        DirectSlot::Argument(index) => (window.parameters(), usize::from(index)),
    };
    (index < region.len()).then(|| region.start + index)
}

impl SlotStore {
    /// The virtual peak must fit in the authenticated operand window, and all
    /// slots that canonical execution would temporarily occupy must be empty.
    #[inline]
    pub(super) fn numeric_span_room_current(&self, window: &FrameWindow, extra_peak: u8) -> bool {
        let operands = window.operands();
        let Some(end_depth) = window.depth.checked_add(usize::from(extra_peak)) else {
            return false;
        };
        if end_depth > operands.len() {
            return false;
        }
        self.slots
            .get(operands.start + window.depth..operands.start + end_depth)
            .is_some_and(|slots| slots.iter().all(Option::is_none))
    }

    /// A decline leaves both bindings and operand depth untouched. Once the
    /// checks pass, only direct Number owners are replaced and no operation can
    /// fail or invoke a release path.
    #[inline]
    #[cfg(test)]
    pub(super) fn try_commit_number_current(
        &mut self,
        window: &mut FrameWindow,
        destination: NumericDestination,
        result: Number,
        update: Option<NumberUpdate>,
        extra_peak: u8,
    ) -> bool {
        self.commit_number_current(window, destination, result, update, extra_peak, false)
    }

    /// The span handler just read every overwritten binding as a Number in the
    /// same non-reentrant frame borrow. Retain the slot/capacity transaction,
    /// but do not classify those unchanged inputs a second time.
    pub(super) fn try_commit_proven_number_current(
        &mut self,
        window: &mut FrameWindow,
        destination: NumericDestination,
        result: Number,
        update: Option<NumberUpdate>,
        extra_peak: u8,
    ) -> bool {
        self.commit_number_current(window, destination, result, update, extra_peak, true)
    }

    #[inline]
    fn commit_number_current(
        &mut self,
        window: &mut FrameWindow,
        destination: NumericDestination,
        result: Number,
        update: Option<NumberUpdate>,
        extra_peak: u8,
        inputs_proven: bool,
    ) -> bool {
        if !self.numeric_span_room_current(window, extra_peak) {
            return false;
        }
        if matches!(destination, NumericDestination::Local(_)) && update.is_some() {
            return false;
        }
        let output_index = match destination {
            NumericDestination::Push => {
                let Some(index) = window.operands().start.checked_add(window.depth) else {
                    return false;
                };
                if !matches!(self.slots.get(index), Some(None)) || index >= window.end {
                    return false;
                }
                Some(index)
            }
            NumericDestination::Local(index) => {
                let Some(index) = direct_slot_index(window, DirectSlot::Local(index)) else {
                    return false;
                };
                if !matches!(
                    self.slots.get(index),
                    Some(Some(FrameBinding::Direct(value)))
                        if inputs_proven || value.as_number_repr().is_some()
                ) {
                    return false;
                }
                None
            }
        };
        let update_index = if let Some(update) = update {
            let Some(index) = direct_slot_index(window, update.slot) else {
                return false;
            };
            if !matches!(
                self.slots.get(index),
                Some(Some(FrameBinding::Direct(value)))
                    if inputs_proven || value.as_number_repr().is_some()
            ) {
                return false;
            }
            Some(index)
        } else {
            None
        };

        if let (Some(index), Some(update)) = (update_index, update) {
            self.slots[index] = Some(FrameBinding::Direct(update.value.into()));
            #[cfg(feature = "profiling")]
            record_owned_storage(Cost::Move(1));
        }
        match destination {
            NumericDestination::Push => {
                let index = output_index.expect("checked numeric output slot");
                self.slots[index] = Some(FrameBinding::Direct(result.into()));
                window.depth += 1;
                #[cfg(feature = "profiling")]
                {
                    self.live_slots += 1;
                    record_owned_storage(Cost::Move(1));
                    self.record_occupancy();
                }
            }
            NumericDestination::Local(index) => {
                let index = direct_slot_index(window, DirectSlot::Local(index))
                    .expect("checked numeric local slot");
                self.slots[index] = Some(FrameBinding::Direct(result.into()));
                #[cfg(feature = "profiling")]
                record_owned_storage(Cost::Move(1));
            }
        }
        true
    }

    // Keep fused compare-branch at one call level even when the caller's own
    // inlining decision flips between builds.
    #[inline]
    pub(super) fn consume_number_pair_current(
        &mut self,
        window: &mut FrameWindow,
        operation: impl FnOnce(Number, Number) -> bool,
    ) -> Result<Option<bool>, Error> {
        let offset = window
            .depth
            .checked_sub(2)
            .ok_or_else(Self::operand_stack_underflow)?;
        let index = window.operands().start + offset;
        let [
            Some(FrameBinding::Direct(left)),
            Some(FrameBinding::Direct(right)),
        ] = &self.slots[index..index + 2]
        else {
            return Err(Self::operand_slot_not_a_value());
        };
        let (Some(left), Some(right)) = (left.as_number_repr(), right.as_number_repr()) else {
            return Ok(None);
        };
        let result = operation(left, right);
        // Neither Number owner can trigger release, GC or a callback.
        self.slots[index] = None;
        self.slots[index + 1] = None;
        window.depth -= 2;
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= 2;
            record_owned_storage(Cost::Clear(2));
            crate::engine::api::profiling::record_owned_execution_event(
                "number_pair_consumed_in_place",
            );
        }
        Ok(Some(result))
    }

    /// Non-failing writeback after `immediate_local` proved the target holds a
    /// direct number: the replaced binding is a scalar, so no release,
    /// capacity check or arena access is involved.
    #[inline]
    pub(super) fn store_number_local_current(
        &mut self,
        window: &FrameWindow,
        index: u16,
        value: Number,
    ) {
        self.slots[window.locals().start + usize::from(index)] =
            Some(FrameBinding::Direct(value.into()));
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(1));
    }

    /// Commit an ordinary Put/Set from a Number operand into a direct Number
    /// binding. The caller has just authenticated the destination in this
    /// execution borrow. A non-Number or missing operand declines without
    /// changing either slot, so the canonical instruction handles its error
    /// and ownership rules.
    #[inline]
    pub(super) fn store_proven_number_operand_current(
        &mut self,
        window: &mut FrameWindow,
        destination: DirectSlot,
        keep: bool,
    ) -> bool {
        let destination_index = match destination {
            DirectSlot::Local(index) => window.locals().start + usize::from(index),
            DirectSlot::Argument(index) => window.parameters().start + usize::from(index),
        };
        debug_assert!(matches!(
            self.slots.get(destination_index),
            Some(Some(FrameBinding::Direct(value))) if value.as_number_repr().is_some()
        ));
        let Some(top) = window.depth.checked_sub(1) else {
            return false;
        };
        let operand_index = window.operands().start + top;
        let Some(Some(FrameBinding::Direct(value))) = self.slots.get(operand_index) else {
            return false;
        };
        let Some(value) = value.as_number_repr() else {
            return false;
        };

        // Both overwritten values are inline scalars. This cannot retain,
        // release, allocate, or call back into JavaScript.
        self.slots[destination_index] = Some(FrameBinding::Direct(value.into()));
        if !keep {
            self.slots[operand_index] = None;
            window.depth -= 1;
        }
        #[cfg(feature = "profiling")]
        {
            record_owned_storage(Cost::Move(1));
            if !keep {
                self.live_slots -= 1;
                record_owned_storage(Cost::Clear(1));
            }
        }
        true
    }

    pub(super) fn update_number_local_current(
        &mut self,
        window: &mut FrameWindow,
        index: u16,
        operation: impl FnOnce(Number) -> (Number, Option<Number>),
    ) -> Result<bool, Error> {
        let FrameBinding::Direct(previous) = self.local_current(window, index)? else {
            return Ok(false);
        };
        let Some(previous) = previous.as_number_repr() else {
            return Ok(false);
        };
        let (replacement, result) = operation(previous);
        // Preflight the optional result destination before modifying the local.
        // This also leaves malformed synthetic windows transactional on error.
        let result = if let Some(result) = result {
            if window.depth >= window.operands().len() {
                return Err(Error::internal(
                    "owned operand stack exceeds verified capacity",
                ));
            }
            let destination = window.operands().start + window.depth;
            if self.slots[destination].is_some() {
                return Err(Error::internal(
                    "owned operand push would replace a live value",
                ));
            }
            Some((destination, result))
        } else {
            None
        };
        self.slots[window.locals().start + usize::from(index)] =
            Some(FrameBinding::Direct(replacement.into()));
        if let Some((destination, result)) = result {
            self.slots[destination] = Some(FrameBinding::Direct(result.into()));
            window.depth += 1;
            #[cfg(feature = "profiling")]
            {
                self.live_slots += 1;
                record_owned_storage(Cost::Move(1));
                self.record_occupancy();
            }
        }
        #[cfg(feature = "profiling")]
        {
            record_owned_storage(Cost::Move(1));
            crate::engine::api::profiling::record_owned_execution_event(
                "number_local_updated_in_place",
            );
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::value::JsValue;
    use crate::engine::vm::stack::FrameStorage;
    use std::rc::Rc;

    fn frame(runtime: &Runtime, capacity: u16) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.argument_count = 1;
        owner.metadata.local_count = 1;
        owner.metadata.max_stack = capacity;
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        owner.argument_definitions = Rc::from([definition]);
        owner.local_definitions = Rc::from([definition]);
        let mut slots = SlotStore::new(20);
        let window = slots
            .push_frame(
                runtime,
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: vec![FrameBinding::Direct(JsValue::Int(11))],
                    locals: vec![FrameBinding::Direct(JsValue::Int(7))],
                    operands: Vec::new(),
                },
            )
            .unwrap();
        (slots, window)
    }

    #[test]
    fn direct_slot_read_is_short_borrow_of_direct_local_or_argument() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let run = slots.run_window(&mut window).unwrap();
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
            assert_eq!(
                run.direct_value(DirectSlot::Argument(0)),
                Some(&JsValue::Int(11))
            );
            assert!(run.direct_value(DirectSlot::Local(1)).is_none());
            assert!(run.direct_value(DirectSlot::Argument(1)).is_none());
        }
        slots.slots[window.parameters().start] = Some(FrameBinding::Uninitialized);
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .direct_value(DirectSlot::Argument(0))
                .is_none()
        );
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn ordinary_number_write_preserves_put_set_and_declines_without_mutation() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let mut run = slots.run_window(&mut window).unwrap();
            assert!(!run.store_proven_number_operand(DirectSlot::Local(0), false));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );

            run.push(JsValue::Bool(true)).unwrap();
            assert!(!run.store_proven_number_operand(DirectSlot::Local(0), false));
            assert_eq!(run.peek(0).unwrap(), &JsValue::Bool(true));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
            assert_eq!(run.pop().unwrap(), JsValue::Bool(true));

            let nan_bits = 0x7ff8_0000_0000_0042;
            run.push(JsValue::Float(f64::from_bits(nan_bits))).unwrap();
            assert!(run.store_proven_number_operand(DirectSlot::Local(0), true));
            let JsValue::Float(top) = run.peek(0).unwrap() else {
                panic!("SetLocal lost the Number operand");
            };
            assert_eq!(top.to_bits(), nan_bits);
            let Some(JsValue::Float(local)) = run.direct_value(DirectSlot::Local(0)) else {
                panic!("SetLocal changed the Number representation");
            };
            assert_eq!(local.to_bits(), nan_bits);

            assert!(run.store_proven_number_operand(DirectSlot::Argument(0), false));
            assert!(run.peek(0).is_err());
            let Some(JsValue::Float(argument)) = run.direct_value(DirectSlot::Argument(0)) else {
                panic!("PutArg changed the Number representation");
            };
            assert_eq!(argument.to_bits(), nan_bits);
        }
        assert_eq!(window.depth, 0);
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn numeric_span_checks_full_peak_and_declines_without_partial_commit() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let mut run = slots.run_window(&mut window).unwrap();
            assert!(run.numeric_span_room(2));
            assert!(!run.numeric_span_room(3));
            assert!(run.try_commit_number(
                NumericDestination::Push,
                Number::Int(21),
                Some(NumberUpdate {
                    slot: DirectSlot::Argument(0),
                    value: Number::Int(12),
                }),
                2,
            ));
            assert_eq!(
                run.direct_value(DirectSlot::Argument(0)),
                Some(&JsValue::Int(12))
            );
            assert_eq!(run.peek(0).unwrap(), &JsValue::Int(21));
            assert!(!run.numeric_span_room(2));
            assert!(!run.try_commit_number(
                NumericDestination::Push,
                Number::Int(22),
                Some(NumberUpdate {
                    slot: DirectSlot::Argument(0),
                    value: Number::Int(13),
                }),
                2,
            ));
            assert_eq!(
                run.direct_value(DirectSlot::Argument(0)),
                Some(&JsValue::Int(12))
            );
            assert_eq!(run.peek(0).unwrap(), &JsValue::Int(21));
        }
        assert_eq!(window.depth, 1);
        slots.clear_frame(&runtime, window).unwrap();

        let (mut slots, mut window) = frame(&runtime, 2);
        let inactive = window.operands().start + 1;
        slots.slots[inactive] = Some(FrameBinding::Direct(JsValue::Int(99)));
        {
            let mut run = slots.run_window(&mut window).unwrap();
            assert!(!run.numeric_span_room(2));
            assert!(!run.try_commit_number(
                NumericDestination::Push,
                Number::Int(21),
                Some(NumberUpdate {
                    slot: DirectSlot::Local(0),
                    value: Number::Int(8),
                }),
                2,
            ));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
        }
        assert_eq!(window.depth, 0);
        slots.slots[inactive] = None;
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn numeric_commit_validates_binding_and_destination_before_writing() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        {
            let mut run = slots.run_window(&mut window).unwrap();
            assert!(!run.try_commit_number(
                NumericDestination::Local(0),
                Number::Int(8),
                Some(NumberUpdate {
                    slot: DirectSlot::Argument(0),
                    value: Number::Int(12),
                }),
                0,
            ));
            assert!(!run.try_commit_number(
                NumericDestination::Push,
                Number::Int(8),
                Some(NumberUpdate {
                    slot: DirectSlot::Local(1),
                    value: Number::Int(9),
                }),
                1,
            ));
            assert_eq!(
                run.direct_value(DirectSlot::Local(0)),
                Some(&JsValue::Int(7))
            );
            assert_eq!(
                run.direct_value(DirectSlot::Argument(0)),
                Some(&JsValue::Int(11))
            );
            assert!(run.try_commit_number(
                NumericDestination::Local(0),
                Number::Float(-0.0),
                None,
                0,
            ));
            assert!(
                matches!(run.direct_value(DirectSlot::Local(0)), Some(JsValue::Float(n)) if *n == 0.0 && n.is_sign_negative())
            );
        }
        assert_eq!(window.depth, 0);
        slots.slots[window.locals().start] = Some(FrameBinding::Uninitialized);
        {
            let mut run = slots.run_window(&mut window).unwrap();
            assert!(!run.try_commit_number(NumericDestination::Local(0), Number::Int(9), None, 0,));
            assert!(!run.try_commit_number(
                NumericDestination::Push,
                Number::Int(9),
                Some(NumberUpdate {
                    slot: DirectSlot::Local(0),
                    value: Number::Int(10),
                }),
                1,
            ));
        }
        assert_eq!(window.depth, 0);
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn fused_pair_declines_before_mutation_and_clears_both_number_owners() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 2);
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        slots.push(&mut window, JsValue::Int(3)).unwrap();
        slots
            .push(&mut window, JsValue::Object(object.into_handle()))
            .unwrap();
        assert_eq!(
            slots
                .run_window(&mut window)
                .unwrap()
                .consume_number_pair(|_, _| {
                    panic!("non-number must decline before evaluating operation")
                })
                .unwrap(),
            None
        );
        assert_eq!(window.depth, 2);
        assert!(matches!(slots.peek(&window, 0).unwrap(), JsValue::Object(value) if *value==id));
        runtime
            .release_jsvalue(slots.pop(&mut window).unwrap())
            .unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
        slots.push(&mut window, JsValue::Int(7)).unwrap();
        assert_eq!(
            slots
                .run_window(&mut window)
                .unwrap()
                .consume_number_pair(|left, right| left.float() < right.float())
                .unwrap(),
            Some(true)
        );
        assert_eq!(window.depth, 0);
        assert!(slots.slots[window.operands()].iter().all(Option::is_none));
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .consume_number_pair(|_, _| true)
                .is_err()
        );
        slots.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn fused_local_result_capacity_failure_leaves_binding_and_stack_unchanged() {
        let runtime = Runtime::new();
        let (mut slots, mut window) = frame(&runtime, 1);
        slots.push(&mut window, JsValue::Int(99)).unwrap();
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .update_number_local(0, |previous| (previous.update(true), Some(previous)))
                .is_err()
        );
        assert!(matches!(
            slots.local(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(7))
        ));
        assert_eq!(slots.peek(&window, 0).unwrap(), &JsValue::Int(99));
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .update_number_local(0, |previous| (previous.update(true), None))
                .unwrap()
        );
        assert!(matches!(
            slots.local(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(8))
        ));
        assert_eq!(slots.pop(&mut window).unwrap(), JsValue::Int(99));
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .update_number_local(0, |previous| (previous.update(false), Some(previous)))
                .unwrap()
        );
        assert_eq!(slots.pop(&mut window).unwrap(), JsValue::Int(8));
        assert!(matches!(
            slots.local(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(7))
        ));
        slots
            .replace_local(&window, 0, FrameBinding::Uninitialized)
            .unwrap();
        assert!(
            !slots
                .run_window(&mut window)
                .unwrap()
                .update_number_local(0, |_| panic!("TDZ must not evaluate"))
                .unwrap()
        );
        assert!(
            slots
                .run_window(&mut window)
                .unwrap()
                .update_number_local(1, |_| panic!("invalid local must not evaluate"))
                .is_err()
        );
        slots.clear_frame(&runtime, window).unwrap();
    }
}
