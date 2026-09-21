//! Fresh ordinary entry; restore/materialized entry stays in stack.rs.
use super::*;
impl SlotStore {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn push_ordinary_frame(
        &mut self,
        runtime: &Runtime,
        layout: &FrameLayout<'_>,
        parent: &mut FrameWindow,
        count: usize,
        method: bool,
        function: &crate::engine::object::ObjectRef,
        function_name: Option<u16>,
        observes_arguments: bool,
    ) -> Result<FrameWindow, Error> {
        self.check_current(parent)?;
        let consumed = count
            .checked_add(1 + usize::from(method))
            .filter(|n| *n <= parent.depth)
            .ok_or_else(|| Error::internal("outgoing call exceeds caller operands"))?;
        let start = parent.operands().start + parent.depth - count;
        let mut keep_originals = observes_arguments;
        for index in start - 1 - usize::from(method)..start + count {
            let Some(FrameBinding::Direct(value)) = &self.slots[index] else {
                return Err(Error::internal("outgoing argument is not a direct owner"));
            };
            if index >= start
                && !matches!(
                    value,
                    JsValue::Undefined
                        | JsValue::Null
                        | JsValue::Bool(_)
                        | JsValue::Int(_)
                        | JsValue::Float(_)
                        | JsValue::ShortBigInt(_)
                )
            {
                // Preserve every original non-scalar owner until frame teardown,
                // even when a writable parameter is replaced or captured.
                keep_originals = true;
            }
        }
        let parameter_count = layout.argument_slots(count);
        let local_count = layout.locals().len();
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
                match copy_value(runtime, value) {
                    Ok(value) => {
                        self.slots[original_end + index] = Some(FrameBinding::Direct(value))
                    }
                    Err(error) => {
                        let _ = self.clear_unpublished(runtime, original_end..original_end + index);
                        return Err(error);
                    }
                }
            }
        }
        for index in original_end + count..parameters_end {
            self.slots[index] = Some(FrameBinding::Direct(JsValue::Undefined));
        }
        for (index, definition) in layout.locals().iter().enumerate() {
            self.slots[parameters_end + index] = Some(
                super::super::call::prepare::initial_local_binding(
                    runtime,
                    definition.is_lexical,
                    function_name == Some(index as u16),
                    function,
                )
                .map_err(runtime_error_to_vm_error)?,
            );
        }
        for index in 0..count {
            self.slots[base + index] = self.slots[start + index].take();
        }
        for index in start - 1 - usize::from(method)..start {
            if let Some(binding) = self.slots[index].take() {
                release_binding(runtime, binding)?;
            }
        }
        parent.depth -= consumed;
        self.active_end = end;
        let id = self.next_window;
        self.next_window = next_window;
        self.windows.push(id);
        #[cfg(feature = "profiling")]
        {
            self.live_slots -= consumed;
            self.live_slots += originals + parameter_count + local_count;
            record_owned_storage(Cost::SlotCapacity {
                before,
                after: self.slots.capacity(),
            });
            record_owned_storage(Cost::NoneInitialization {
                count: self.slots.len() - initialized,
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
                if keep_originals { count } else { 0 },
                roots,
                usize::from(function_name.is_some()),
            );
            crate::engine::api::profiling::record_owned_execution_event(
                "call_bindings_initialized_in_window",
            );
            crate::engine::api::profiling::record_owned_execution_event(
                "call_outgoing_tail_transferred",
            );
            if !keep_originals {
                crate::engine::api::profiling::record_owned_execution_event(
                    "ordinary_scalar_argv_elided",
                );
            }
        }
        Ok(FrameWindow {
            owner: self.owner.clone(),
            id,
            base,
            original_end,
            parameters_end,
            locals_end,
            end,
            depth: 0,
            actual_count: count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    fn storage(values: Vec<JsValue>) -> FrameStorage {
        FrameStorage {
            original_arguments: vec![],
            parameters: vec![],
            locals: vec![],
            operands: values,
        }
    }
    #[test]
    fn scalar_elision_preserves_arity_but_reference_originals_survive_parameter_writes() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let function = runtime.new_object(None).unwrap();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
        executable.metadata.max_stack = 4;
        let mut slots = SlotStore::new(32);
        let mut parent = slots
            .push_frame(
                &runtime,
                &executable.frame_layout(),
                storage(vec![
                    JsValue::Object(function.clone().into_handle()),
                    JsValue::Int(7),
                ]),
            )
            .unwrap();
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                1,
                false,
                &function,
                None,
                false,
            )
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
            .push(&mut parent, JsValue::Object(function.clone().into_handle()))
            .unwrap();
        slots
            .push(&mut parent, JsValue::Object(marker.into_handle()))
            .unwrap();
        let child = slots
            .push_ordinary_frame(
                &runtime,
                &executable.frame_layout(),
                &mut parent,
                1,
                false,
                &function,
                None,
                false,
            )
            .unwrap();
        assert_eq!(child.original_arguments().len(), 1);
        let replaced = slots
            .replace_parameter(&child, 0, FrameBinding::Direct(JsValue::Undefined))
            .unwrap();
        release_binding(&runtime, replaced).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_ok());
        slots.clear_frame(&runtime, child).unwrap();
        assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
        slots.clear_frame(&runtime, parent).unwrap();
    }
    #[test]
    fn ordinary_retain_failure_keeps_the_entire_parent_and_rolls_back_suffix() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
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
                    JsValue::Object(function.clone().into_handle()),
                    JsValue::Object(first.into_handle()),
                    JsValue::Object(stale_handle),
                ]),
            )
            .unwrap();
        let end = slots.active_end;
        assert!(
            slots
                .push_ordinary_frame(
                    &runtime,
                    &executable.frame_layout(),
                    &mut parent,
                    2,
                    false,
                    &function,
                    None,
                    false
                )
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
}
