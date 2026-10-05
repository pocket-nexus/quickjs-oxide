//! Fresh ordinary entry; restore/materialized entry stays in stack.rs.
use super::*;
pub(in crate::engine::vm) struct InstalledOrdinaryFrame {
    pub window: FrameWindow,
    pub function: crate::engine::heap::ObjectId,
    pub input: crate::engine::vm::CallInput,
}

/// Unpublished initialized ranges. Every fallible owner-producing operation has
/// completed; callers immediately move their inputs and publish this window.
#[must_use]
pub(super) struct PreparedOrdinaryWindow {
    pub(super) window: FrameWindow,
    pub(super) start: usize,
    next_window: u64,
    #[cfg(feature = "profiling")]
    before: usize,
    #[cfg(feature = "profiling")]
    initialized: usize,
    #[cfg(feature = "profiling")]
    keep_originals: bool,
    #[cfg(feature = "profiling")]
    roots: usize,
    #[cfg(feature = "profiling")]
    function_name: bool,
}
impl SlotStore {
    #[cfg(test)]
    pub(in crate::engine::vm) fn push_ordinary_frame(
        &mut self,
        runtime: &Runtime,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        checked: CheckedOrdinaryCallOperands,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
    ) -> Result<InstalledOrdinaryFrame, Error> {
        let _unwind = runtime.unwind_guard();
        self.push_ordinary_frame_in_state(
            runtime,
            &mut runtime.0.state.borrow_mut(),
            layout,
            parent,
            checked,
            function,
            observes_arguments,
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn push_ordinary_frame_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        checked: CheckedOrdinaryCallOperands,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
    ) -> Result<InstalledOrdinaryFrame, Error> {
        self.check_current(parent)?;
        if parent.id != checked.window_id || parent.depth != checked.depth {
            return Err(Error::internal(
                "ordinary call operands changed after validation",
            ));
        }
        self.push_current_ordinary_frame_in_state(
            runtime,
            state,
            layout,
            parent,
            checked,
            function,
            observes_arguments,
        )
    }

    /// Called only by a checked legacy adapter or by the current transaction
    /// producer, which holds the exclusive store and caller window together.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_current_ordinary_frame_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        checked: CheckedOrdinaryCallOperands,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
    ) -> Result<InstalledOrdinaryFrame, Error> {
        let count = checked.count;
        let method = checked.method;
        let consumed = count
            .checked_add(1 + usize::from(method))
            .filter(|n| *n <= parent.depth)
            .ok_or_else(|| Error::internal("outgoing call exceeds caller operands"))?;
        let prepared = self.prepare_ordinary_window_in_state(
            runtime,
            state,
            layout,
            parent,
            count,
            function,
            observes_arguments,
        )?;
        let start = prepared.start;
        let base = prepared.window.base;
        for index in 0..count {
            self.slots[base + index] = self.slots[start + index].take();
        }
        // All fallible work has completed. Transfer these roots directly from
        // caller operands; no retain/release round trip or JS re-entry occurs.
        let Some(FrameBinding::Direct(JsValue::Object(callee))) = self.slots[start - 1].take()
        else {
            unreachable!("authenticated ordinary callee is an object")
        };
        let receiver = if method {
            let Some(FrameBinding::Direct(receiver)) = self.slots[start - 2].take() else {
                unreachable!("checked receiver is direct")
            };
            receiver
        } else {
            JsValue::Undefined
        };
        let function = callee;
        let input = crate::engine::vm::CallInput::new(runtime, receiver, JsValue::Undefined, None);
        let window = self.publish_ordinary_window(parent, consumed, prepared);
        Ok(InstalledOrdinaryFrame {
            function,
            input,
            window,
        })
    }

    /// Prepare the unpublished parameter/local suffix while the caller owns
    /// all operands. Ordinary calls and Base constructors share this initializer;
    /// their distinct input owners move only after all fallible work succeeds.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_ordinary_window_in_state(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &FrameLayout<'_>,
        parent: &FrameWindow,
        count: usize,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
    ) -> Result<PreparedOrdinaryWindow, Error> {
        let start = parent.operands().start + parent.depth - count;
        // Only language-visible original arguments need a second owner.
        // WeakRef liveness belongs to the enclosing execution turn.
        let keep_originals = observes_arguments;
        let parameter_count = layout.argument_slots(count);
        let local_count = layout.locals().len();
        let function_name = layout.function_name_local();
        if function_name.is_some_and(|i| usize::from(i) >= local_count) {
            return Err(Error::internal("function-name local is outside the frame"));
        }
        let next_window = self
            .next_window
            .checked_add(1)
            .ok_or_else(|| Error::internal("frame window identity exhausted"))?;
        let base = self.active_end;
        let originals = if keep_originals { count } else { 0 };
        let original_end = base.checked_add(originals);
        let parameters_end = original_end.and_then(|n| n.checked_add(parameter_count));
        let locals_end = parameters_end.and_then(|n| n.checked_add(local_count));
        let end = locals_end
            .and_then(|n| n.checked_add(layout.operand_capacity()))
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| Error::internal("execution slot limit exceeded"))?;
        #[cfg(feature = "profiling")]
        let before = self.slots.capacity();
        self.slots
            .try_reserve(end.saturating_sub(self.slots.len()))
            .map_err(|_| Error::internal("execution slot allocation failed"))?;
        self.windows
            .try_reserve(1)
            .map_err(|_| Error::internal("execution window allocation failed"))?;
        #[cfg(feature = "profiling")]
        let initialized = self.slots.len();
        if end > self.slots.len() {
            self.slots.resize_with(end, || None);
        }
        let original_end = original_end.unwrap();
        let parameters_end = parameters_end.unwrap();
        let locals_end = locals_end.unwrap();
        #[cfg(feature = "profiling")]
        let mut roots = 0;
        if keep_originals {
            // Complete all fallible retains before moving any caller owner.
            for index in 0..count {
                let Some(FrameBinding::Direct(value)) = &self.slots[start + index] else {
                    unreachable!()
                };
                #[cfg(feature = "profiling")]
                {
                    roots += usize::from(matches!(value, JsValue::Object(_) | JsValue::Symbol(_)));
                }
                match state.dup_jsvalue(value) {
                    Ok(copied) => {
                        #[cfg(feature = "profiling")]
                        record_copy(value);
                        self.slots[original_end + index] = Some(FrameBinding::Direct(copied));
                    }
                    Err(error) => {
                        self.clear_unpublished_owned_in_state(
                            state,
                            &runtime.0.poisoned,
                            original_end..original_end + index,
                        )?;
                        return Err(runtime_error_to_vm_error(error));
                    }
                }
            }
        }
        for index in original_end + count..parameters_end {
            self.slots[index] = Some(FrameBinding::Direct(JsValue::Undefined));
        }
        if layout.plain_local_initializers() {
            // Every initial value is edge-free and identical. The immutable
            // published fact removes per-local lexical/name classification and
            // the fallible binding constructor from ordinary call entry.
            self.slots[parameters_end..locals_end]
                .fill_with(|| Some(FrameBinding::Direct(JsValue::Undefined)));
        } else {
            for (index, definition) in layout.locals().iter().enumerate() {
                let binding = match initial_local_binding_in_state(
                    state,
                    definition.is_lexical,
                    function_name == Some(index as u16),
                    function,
                ) {
                    Ok(binding) => binding,
                    Err(error) => {
                        // Parameters and preceding locals were installed only in
                        // the unpublished suffix. The caller still owns every
                        // outgoing operand, including the method receiver.
                        self.clear_unpublished_owned_in_state(
                            state,
                            &runtime.0.poisoned,
                            original_end..parameters_end + index,
                        )?;
                        return Err(runtime_error_to_vm_error(error));
                    }
                };
                self.slots[parameters_end + index] = Some(binding);
            }
        }
        Ok(PreparedOrdinaryWindow {
            start,
            next_window,
            window: FrameWindow {
                owner: self.owner.clone(),
                id: self.next_window,
                base,
                original_end,
                parameters_end,
                locals_end,
                end,
                depth: 0,
                actual_count: count,
            },
            #[cfg(feature = "profiling")]
            before,
            #[cfg(feature = "profiling")]
            initialized,
            #[cfg(feature = "profiling")]
            keep_originals,
            #[cfg(feature = "profiling")]
            roots,
            #[cfg(feature = "profiling")]
            function_name: function_name.is_some(),
        })
    }

    /// Publish a prepared window after its caller edges have moved. There is
    /// no allocation, checked retain, cleanup or callback after that transfer.
    #[inline]
    pub(super) fn publish_ordinary_window(
        &mut self,
        parent: &mut FrameWindow,
        consumed: usize,
        prepared: PreparedOrdinaryWindow,
    ) -> FrameWindow {
        let PreparedOrdinaryWindow {
            window,
            next_window,
            ..
        } = &prepared;
        #[cfg(feature = "profiling")]
        let count = window.actual_count;
        #[cfg(feature = "profiling")]
        let base = window.base;
        let end = window.end;
        #[cfg(feature = "profiling")]
        let originals = window.original_arguments().len();
        #[cfg(feature = "profiling")]
        let parameter_count = window.parameters().len();
        #[cfg(feature = "profiling")]
        let local_count = window.locals().len();
        parent.depth -= consumed;
        self.active_end = end;
        let id = self.next_window;
        self.next_window = *next_window;
        self.windows.push(id);
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= consumed;
            self.live_slots += originals + parameter_count + local_count;
            record_owned_storage(Cost::SlotCapacity {
                before: prepared.before,
                after: self.slots.capacity(),
            });
            record_owned_storage(Cost::NoneInitialization {
                count: self.slots.len() - prepared.initialized,
                high_water: self.slots.len(),
            });
            record_owned_storage(Cost::Clear(consumed - count));
            record_owned_storage(Cost::Initialize(end - base));
            record_owned_storage(Cost::Move(count + parameter_count + local_count));
            self.record_occupancy();
            crate::engine::api::profiling::record_call_preparation(
                parameter_count,
                0,
                local_count,
                0,
                if prepared.keep_originals { count } else { 0 },
                prepared.roots,
                usize::from(prepared.function_name),
            );
            crate::engine::api::profiling::record_owned_execution_event(
                "call_bindings_initialized_in_window",
            );
            crate::engine::api::profiling::record_owned_execution_event(
                "call_outgoing_tail_transferred",
            );
            if !prepared.keep_originals {
                crate::engine::api::profiling::record_owned_execution_event("ordinary_argv_elided");
            }
        }
        prepared.window
    }
    fn clear_unpublished_owned_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        range: std::ops::Range<usize>,
    ) -> Result<(), Error> {
        for index in range {
            if let Some(binding) = self.slots[index].take() {
                if let Err(error) =
                    crate::engine::vm::bindings::release_frame_binding_in_state(state, binding)
                {
                    poisoned.set(true);
                    return Err(runtime_error_to_vm_error(error));
                }
            }
        }
        Ok(())
    }

    /// Clear the actual current window under the executor's state access.
    /// The result owner remains in execution storage throughout retirement.
    pub(in crate::engine::vm) fn clear_frame_owned_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        window: FrameWindow,
    ) -> Result<(), Error> {
        self.check_current(&window)?;
        self.clear_current_frame_owned_in_state(state, poisoned, window)
    }

    /// The exclusive execution producer supplies the actual retiring window.
    /// No detached window/currentness proof is accepted outside this module.
    pub(super) fn clear_current_frame_owned_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        window: FrameWindow,
    ) -> Result<(), Error> {
        #[cfg(feature = "profiling")]
        {
            let cleared = self.slots[window.whole()]
                .iter()
                .filter(|slot| slot.is_some())
                .count();
            self.live_slots -= cleared;
            record_owned_storage(Cost::Clear(cleared));
        }
        self.active_end = window.whole().start;
        self.clear_unpublished_owned_in_state(
            state,
            poisoned,
            window.whole().start..window.operands().start + window.depth,
        )?;
        debug_assert!(self.slots[window.whole()].iter().all(Option::is_none));
        self.windows.pop();
        Ok(())
    }
}

fn initial_local_binding_in_state(
    state: &mut crate::engine::heap::runtime::RuntimeState,
    lexical: bool,
    function_name: bool,
    function: crate::engine::heap::ObjectId,
) -> Result<FrameBinding, crate::engine::api::runtime_error::RuntimeError> {
    if function_name {
        state.heap.retain_object(function)?;
        Ok(FrameBinding::Direct(JsValue::Object(function)))
    } else if lexical {
        Ok(FrameBinding::Uninitialized)
    } else {
        Ok(FrameBinding::Direct(JsValue::Undefined))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;

    fn take_installed_window(
        runtime: &Runtime,
        mut installed: InstalledOrdinaryFrame,
    ) -> FrameWindow {
        let mut state = runtime.0.state.borrow_mut();
        installed.input.release(&mut state).unwrap();
        state.release_object_handle(installed.function).unwrap();
        installed.window
    }

    fn checked_operands(
        slots: &mut SlotStore,
        parent: &mut FrameWindow,
        count: usize,
        method: bool,
    ) -> CheckedOrdinaryCallOperands {
        slots
            .frame_transaction(parent)
            .unwrap()
            .validate_ordinary_call_operands(count, method)
            .unwrap()
    }

    fn storage(values: Vec<JsValue>) -> FrameStorage {
        FrameStorage {
            original_arguments: vec![],
            parameters: vec![],
            locals: vec![],
            operands: values,
        }
    }

    #[test]
    fn ordinary_local_initialization_uses_published_plain_fact_and_preserves_tdz() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 1;
        executable.metadata.local_count = 2;
        executable.local_definitions = std::rc::Rc::from([definition; 2]);
        let mut caller = PublishedFunctionSnapshot::empty_for_test(context.realm);
        caller.metadata.max_stack = 1;
        let mut slots = SlotStore::new(16);
        let mut parent = slots
            .push_frame(
                &runtime,
                &caller.frame_layout(),
                storage(vec![JsValue::Object(
                    function.try_clone().expect("duplicate root").into_handle(),
                )]),
            )
            .unwrap();
        assert!(executable.frame_layout().plain_local_initializers());
        let checked = checked_operands(&mut slots, &mut parent, 0, false);
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .unwrap();
        assert!(matches!(
            slots.local(&child, 0).unwrap(),
            FrameBinding::Direct(JsValue::Undefined)
        ));
        assert!(matches!(
            slots.local(&child, 1).unwrap(),
            FrameBinding::Direct(JsValue::Undefined)
        ));
        slots.clear_frame(&runtime, child).unwrap();

        // A synthetic snapshot may change definitions between test calls. Its
        // test-only layout derives the fact again, so a lexical local cannot
        // accidentally enter the bulk Undefined initialization path.
        executable.local_definitions = std::rc::Rc::from([
            definition,
            VariableDefinition {
                is_lexical: true,
                ..definition
            },
        ]);
        assert!(!executable.frame_layout().plain_local_initializers());
        slots
            .push(
                &mut parent,
                JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
            )
            .unwrap();
        let checked = checked_operands(&mut slots, &mut parent, 0, false);
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .unwrap();
        assert!(matches!(
            slots.local(&child, 0).unwrap(),
            FrameBinding::Direct(JsValue::Undefined)
        ));
        assert!(matches!(
            slots.local(&child, 1).unwrap(),
            FrameBinding::Uninitialized
        ));
        slots.clear_frame(&runtime, child).unwrap();
        slots.clear_frame(&runtime, parent).unwrap();
    }
    #[test]
    fn argument_transfer_preserves_arity_and_releases_overwritten_parameters() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
                    JsValue::Int(7),
                ]),
            )
            .unwrap();
        let checked = checked_operands(&mut slots, &mut parent, 1, false);
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .unwrap();
        assert!(child.original_arguments().is_empty());
        assert_eq!(slots.actual_argument_count(&child).unwrap(), 1);
        assert!(matches!(
            slots.parameter(&child, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(7))
        ));
        assert_eq!(
            slots
                .take_frame(&runtime, child)
                .unwrap()
                .original_arguments,
            vec![JsValue::Undefined]
        );
        let marker = runtime.new_object(None).unwrap();
        let marker_id = marker.object_id();
        slots
            .push(
                &mut parent,
                JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
            )
            .unwrap();
        slots
            .push(&mut parent, JsValue::Object(marker.into_handle()))
            .unwrap();
        let checked = checked_operands(&mut slots, &mut parent, 1, false);
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .unwrap();
        assert!(child.original_arguments().is_empty());
        let replaced = slots
            .replace_parameter(&child, 0, FrameBinding::Direct(JsValue::Undefined))
            .unwrap();
        release_binding(&runtime, replaced).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
        slots.clear_frame(&runtime, child).unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
        slots.clear_frame(&runtime, parent).unwrap();
    }
    #[test]
    fn ordinary_retain_failure_keeps_the_entire_parent_and_rolls_back_suffix() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let first = runtime.new_object(None).unwrap();
        let blocked = runtime.new_object(None).unwrap();
        let stale_handle = blocked.into_handle();
        // Make the top argument's handle stale so its retain fails after the
        // earlier operands commit, exercising suffix rollback.
        runtime
            .release_jsvalue(JsValue::Object(stale_handle))
            .unwrap();
        runtime.run_gc().unwrap();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
                    JsValue::Object(first.into_handle()),
                    JsValue::Object(stale_handle),
                ]),
            )
            .unwrap();
        let end = slots.active_end;
        let checked = checked_operands(&mut slots, &mut parent, 2, false);
        assert!(
            slots
                .push_ordinary_frame(
                    &runtime,
                    &executable.frame_layout(),
                    &mut parent,
                    checked,
                    function.object_id(),
                    true
                )
                .map(|installed| take_installed_window(&runtime, installed))
                .is_err()
        );
        assert_eq!(slots.active_end, end);
        assert_eq!(slots.depth(&parent), 3);
        assert!(slots.slots[end..].iter().all(Option::is_none));
        // Swap the reclaimed top operand for a live binding before clearing so
        // frame teardown never releases a stale handle.
        let stale = slots
            .replace_operand(&parent, 0, JsValue::Undefined)
            .unwrap();
        let _ = stale;
        slots.clear_frame(&runtime, parent).unwrap();
    }

    #[test]
    fn method_receiver_copy_survives_operand_release_until_input_teardown() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let receiver = runtime.new_object(None).unwrap();
        let receiver_id = receiver.object_id();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(receiver.into_handle()),
                    JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
                ]),
            )
            .unwrap();
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(1)
        );
        let checked = checked_operands(&mut slots, &mut parent, 0, true);
        let receiver = copy_value(&runtime, slots.peek(&parent, 1).unwrap()).unwrap();
        let input = crate::engine::vm::protocol::CallInputGuard::new(
            &runtime,
            crate::engine::vm::CallInput::new(&runtime, receiver, JsValue::Undefined, None),
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(2)
        );
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .unwrap();
        assert!(matches!(
            &input.this_value,
            JsValue::Object(id) if *id == receiver_id
        ));
        assert_eq!(slots.depth(&parent), 0);
        assert!(slots.slots[parent.operands().start].is_none());
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(1)
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_ok());
        drop(input);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_err());
        slots.clear_frame(&runtime, child).unwrap();
        slots.clear_frame(&runtime, parent).unwrap();
    }

    #[test]
    fn failed_method_argument_copy_keeps_receiver_and_rolls_back_suffix() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let receiver = runtime.new_object(None).unwrap();
        let receiver_id = receiver.object_id();
        let first = runtime.new_object(None).unwrap();
        let blocked = runtime.new_object(None).unwrap();
        let stale_handle = blocked.into_handle();
        runtime
            .release_jsvalue(JsValue::Object(stale_handle))
            .unwrap();
        runtime.run_gc().unwrap();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(receiver.into_handle()),
                    JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
                    JsValue::Object(first.into_handle()),
                    JsValue::Object(stale_handle),
                ]),
            )
            .unwrap();
        let end = slots.active_end;
        let checked = checked_operands(&mut slots, &mut parent, 2, true);
        let receiver_copy = copy_value(&runtime, slots.peek(&parent, 3).unwrap()).unwrap();
        let input = crate::engine::vm::protocol::CallInputGuard::new(
            &runtime,
            crate::engine::vm::CallInput::new(&runtime, receiver_copy, JsValue::Undefined, None),
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(2)
        );
        assert!(
            slots
                .push_ordinary_frame(
                    &runtime,
                    &executable.frame_layout(),
                    &mut parent,
                    checked,
                    function.object_id(),
                    true,
                )
                .map(|installed| take_installed_window(&runtime, installed))
                .is_err()
        );
        assert_eq!(slots.active_end, end);
        assert_eq!(slots.depth(&parent), 4);
        assert!(slots.slots[end..].iter().all(Option::is_none));
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(2)
        );
        drop(input);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(1)
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_ok());
        let stale = slots
            .replace_operand(&parent, 0, JsValue::Undefined)
            .unwrap();
        let _ = stale;
        slots.clear_frame(&runtime, parent).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_err());
    }

    #[test]
    fn failed_named_local_retain_clears_staged_parameter_and_earlier_local() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let callee = runtime.new_object(None).unwrap();
        let receiver = runtime.new_object(None).unwrap();
        let receiver_id = receiver.object_id();
        let argument = runtime.new_object(None).unwrap();
        let argument_id = argument.object_id();
        // Synthesize an invalid function root only in this test. The separate
        // live callee slot satisfies the operand witness; the named local's
        // retain is the first operation that touches this stale handle.
        let stale_id = runtime.new_object(None).unwrap().into_handle();
        runtime.release_object_handle(stale_id);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(stale_id).is_err());
        let stale_function =
            crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), stale_id);

        let mut caller = PublishedFunctionSnapshot::empty_for_test(context.realm);
        caller.metadata.max_stack = 3;
        let mut callee_layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        callee_layout.metadata.argument_count = 1;
        callee_layout.metadata.local_count = 2;
        callee_layout.metadata.function_name_local = Some(1);
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        callee_layout.local_definitions = std::rc::Rc::from([definition; 2]);
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &caller.frame_layout(),
                storage(vec![
                    JsValue::Object(receiver.into_handle()),
                    JsValue::Object(callee.try_clone().expect("duplicate root").into_handle()),
                    JsValue::Object(argument.into_handle()),
                ]),
            )
            .unwrap();
        let end = slots.active_end;
        let checked = checked_operands(&mut slots, &mut parent, 1, true);
        let error = slots
            .push_ordinary_frame(
                &runtime,
                &callee_layout.frame_layout(),
                &mut parent,
                checked,
                stale_function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .err()
            .expect("the named local must reject a stale function handle");
        // Disarm the synthetic wrapper without releasing a nonexistent edge.
        // Draining first keeps ObjectRef::into_handle on its transfer branch.
        runtime.run_gc().unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.0.state.borrow().heap.has_pending_zero_cleanup());
        let _ = stale_function.into_handle();
        assert!(!error.message().is_empty());
        // The unpublished parameter copy and first local have both been
        // cleaned. The caller still owns its original receiver and argument.
        assert_eq!(slots.active_end, end);
        assert_eq!(slots.depth(&parent), 3);
        assert!(slots.slots[end..].iter().all(Option::is_none));
        assert!(matches!(
            &slots.slots[parent.operands().start],
            Some(FrameBinding::Direct(JsValue::Object(id))) if *id == receiver_id
        ));
        assert!(matches!(
            &slots.slots[parent.operands().start + 2],
            Some(FrameBinding::Direct(JsValue::Object(id))) if *id == argument_id
        ));
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver_id),
            Ok(1)
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(argument_id),
            Ok(1)
        );
        slots.clear_frame(&runtime, parent).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_err());
        assert!(runtime.0.state.borrow().heap.object(argument_id).is_err());
    }

    #[test]
    fn checked_ordinary_operands_reject_depth_change_without_moving_owners() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let marker = runtime.new_object(None).unwrap();
        let marker_id = marker.object_id();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(function.try_clone().expect("duplicate root").into_handle()),
                    JsValue::Object(marker.into_handle()),
                ]),
            )
            .unwrap();
        let checked = checked_operands(&mut slots, &mut parent, 1, false);
        slots.push(&mut parent, JsValue::Int(9)).unwrap();
        let end = slots.active_end;
        let error = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                checked,
                function.object_id(),
                false,
            )
            .map(|installed| take_installed_window(&runtime, installed))
            .err()
            .unwrap();
        assert_eq!(
            error.message(),
            "ordinary call operands changed after validation"
        );
        assert_eq!(slots.active_end, end);
        assert_eq!(slots.depth(&parent), 3);
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_ok());
        slots.clear_frame(&runtime, parent).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
    }

    #[test]
    fn checked_ordinary_operands_preserve_method_domain_error_order() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let function = runtime.new_object(None).unwrap();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 3;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(function.into_handle()),
                    JsValue::Int(7),
                ]),
            )
            .unwrap();
        let argument_index = parent.operands().start + 1;
        slots.slots[argument_index] = Some(FrameBinding::Uninitialized);
        let error = slots
            .frame_transaction(&mut parent)
            .unwrap()
            .validate_ordinary_call_operands(1, true)
            .err()
            .unwrap();
        assert_eq!(error.message(), "owned operand stack underflow");
        assert_eq!(slots.depth(&parent), 2);
        slots.clear_frame(&runtime, parent).unwrap();
    }
}
