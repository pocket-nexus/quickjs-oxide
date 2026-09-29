//! Promote a location-cache hit without draining runtime cleanup or invoking JS.
use super::{LinkedNativeSelection, NamedDataSelection, NamedSelectionMiss, linked_field_atom};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::native::PrimitiveKind;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectPayload, RawValue, SlotReleaseReadiness};
use crate::engine::object::property_ic::{CacheSelection, PropertyReadCache};
use crate::engine::value::JsValue;
use crate::engine::value::number::operations::Number;

#[inline(always)]
fn record_selection(_result: &NamedDataSelection) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(match _result {
        NamedDataSelection::Data(_) => "property_selection.data",
        NamedDataSelection::CompleteAbsent => "property_selection.absent",
        NamedDataSelection::Accessor(_) => "property_selection.accessor",
        NamedDataSelection::ContinueLookup => "property_selection.general",
        NamedDataSelection::NeedsObservation => "property_selection.observe",
    });
}

#[inline(always)]
fn record_selection_reason(_reason: &'static str) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(_reason);
}

#[inline(always)]
fn record_release_reason(_readiness: SlotReleaseReadiness) {
    #[cfg(feature = "profiling")]
    record_selection_reason(match _readiness {
        SlotReleaseReadiness::Ready => "property_selection.release_ready",
        SlotReleaseReadiness::QueueCapacity => "property_selection.release_queue_capacity",
        SlotReleaseReadiness::Drain => "property_selection.release_drain",
        SlotReleaseReadiness::Deferred => "property_selection.release_deferred",
        SlotReleaseReadiness::Borrowed => "property_selection.release_borrowed",
        SlotReleaseReadiness::PrimitiveStorage => "property_selection.release_primitive_storage",
    });
}

impl Runtime {
    /// A miss only records a location and leaves the canonical read untouched.
    /// Native classification, when requested, describes this retained result;
    /// it never caches a value or outlives the result's ordinary slot owner.
    #[cfg(test)]
    pub(crate) fn try_property_ic_read_owned(
        &self,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Result<Option<JsValue>, RuntimeError> {
        let Some(atom) = linked_field_atom(self, executable, key_index) else {
            return Ok(None);
        };
        let Some(cache) = executable.property_read_ic.site(pc) else {
            return Ok(None);
        };
        if !keep_receiver && self.0.deferred_references.has_pending() {
            return Ok(None);
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return Ok(None);
        };
        if !keep_receiver && state.heap.has_pending_zero_cleanup() {
            return Ok(None);
        }
        let receiver = match base {
            JsValue::Object(object) => *object,
            _ => {
                cache.miss(
                    &state.heap,
                    &state.atoms,
                    self.domain_id(),
                    executable.realm,
                    None,
                    atom,
                );
                return Ok(None);
            }
        };
        // Prove replacement cannot release the last receiver owner BEFORE
        // promoting a result. The proof and retain share this state borrow.
        if !keep_receiver
            && state.heap.slot_object_release_readiness(receiver)? != SlotReleaseReadiness::Ready
        {
            return Ok(None);
        }
        let Some(raw) = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)
        else {
            cache.miss(
                &state.heap,
                &state.atoms,
                self.domain_id(),
                executable.realm,
                Some(receiver),
                atom,
            );
            return Ok(None);
        };
        if matches!(
            raw,
            RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception
        ) {
            return Ok(None);
        }
        let raw = raw.clone();
        let selected = if keep_receiver {
            if let RawValue::Object(function) = &raw {
                state.heap.object(*function).ok().and_then(|object| {
                    let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
                        return None;
                    };
                    let realm = data.realm?;
                    (data.operation().is_some() && state.heap.context(realm).is_ok())
                        .then_some((*function, *data))
                })
            } else {
                None
            }
        } else {
            None
        };
        // Every heap-backed kind retains one new edge; the internal-value
        // conversion below cannot fail after the sentinel exclusion above.
        state.retain_raw_root(raw.clone())?;
        drop(state);
        let value = JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
            "internal value sentinel occupied a cached property slot",
        ))?;
        *native = selected.map(|(function, data)| LinkedNativeSelection {
            runtime: self.clone(),
            function,
            data,
        });
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_ic.hit");
        Ok(Some(value))
    }

    /// Trusted shared-borrow data-property read.
    ///
    /// Covers a cached or freshly selected ordinary data slot, including a
    /// String prototype slot, without ending the heap borrow before promotion.
    /// A declined read claims no owner and leaves special storage to the
    /// canonical driver.
    #[inline]
    pub(crate) fn select_linked_data(
        &self,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> NamedDataSelection {
        let mut miss = NamedSelectionMiss::ContinueLookup;
        if let Some(value) = self.select_linked_data_into(
            base,
            executable,
            pc,
            key_index,
            keep_receiver,
            native,
            &mut miss,
        ) {
            NamedDataSelection::Data(value)
        } else {
            match miss {
                NamedSelectionMiss::CompleteAbsent => NamedDataSelection::CompleteAbsent,
                NamedSelectionMiss::Accessor(getter) => NamedDataSelection::Accessor(getter),
                NamedSelectionMiss::ContinueLookup => NamedDataSelection::ContinueLookup,
                NamedSelectionMiss::NeedsObservation => NamedDataSelection::NeedsObservation,
            }
        }
    }

    /// Return an owned data value directly; only a declined read writes the
    /// small cold outcome. Selection and promotion still happen once under the
    /// same heap borrow for both ordinary and borrowed-receiver consumers.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn select_linked_data_into(
        &self,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
        miss: &mut NamedSelectionMiss,
    ) -> Option<JsValue> {
        // Only object storage and the primitive string length projection can
        // complete under this borrow. Other primitive reads use the existing
        // prototype/conversion driver. They cannot teach this object-location
        // cache anything, so leave its state untouched.
        if !matches!(base, JsValue::Object(_) | JsValue::String(_)) {
            record_selection_reason("property_selection.decline_primitive");
            return None;
        }
        let Some(atom) = linked_field_atom(self, executable, key_index) else {
            record_selection_reason("property_selection.decline_unlinked_key");
            return None;
        };
        let cache = executable.property_read_ic.site(pc);
        if !keep_receiver && self.0.deferred_references.has_pending() {
            record_selection_reason("property_selection.decline_deferred");
            *miss = NamedSelectionMiss::NeedsObservation;
            return None;
        }
        if !keep_receiver
            && matches!(base, JsValue::String(_))
            && !matches!(
                self.slot_value_release_readiness_jsvalue(base),
                Ok(SlotReleaseReadiness::Ready)
            )
        {
            record_selection_reason("property_selection.decline_string_release");
            *miss = NamedSelectionMiss::NeedsObservation;
            return None;
        }
        let Ok(state) = self.0.state.try_borrow() else {
            record_selection_reason("property_selection.decline_heap_borrow");
            *miss = NamedSelectionMiss::NeedsObservation;
            return None;
        };
        if !keep_receiver && state.heap.has_pending_zero_cleanup() {
            record_selection_reason("property_selection.decline_zero_cleanup");
            *miss = NamedSelectionMiss::NeedsObservation;
            return None;
        }
        let (receiver, string_receiver) = match base {
            JsValue::Object(object) => (*object, false),
            JsValue::String(id) => {
                let length = state
                    .pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Length);
                if atom == length {
                    let length = state.heap.string_fast(*id).len();
                    record_selection_reason("property_selection.string_length");
                    return Some(
                        i32::try_from(length)
                            .map_or_else(|_| JsValue::Float(length as f64), JsValue::Int),
                    );
                }
                let Ok(index) = state.atoms.array_index(atom) else {
                    return None;
                };
                if index.is_some_and(|index| {
                    usize::try_from(index)
                        .is_ok_and(|index| index < state.heap.string_fast(*id).len())
                }) {
                    // An indexed character creates a string node. Its
                    // allocation and publication remain in the driver.
                    record_selection_reason("property_selection.string_index_allocation");
                    return None;
                }
                let prototype = state
                    .heap
                    .context(executable.realm)
                    .ok()
                    .and_then(|realm| realm.primitive_prototypes[PrimitiveKind::String.index()])?;
                record_selection_reason("property_selection.string_prototype");
                (prototype, true)
            }
            _ => unreachable!("non-string primitive returned before the heap borrow"),
        };
        if !keep_receiver && !string_receiver && {
            let readiness = state.heap.slot_object_release_readiness_fast(receiver);
            if readiness != SlotReleaseReadiness::Ready {
                record_release_reason(readiness);
                true
            } else {
                false
            }
        } {
            *miss = NamedSelectionMiss::NeedsObservation;
            return None;
        }
        let Some(cache) = cache else {
            record_selection_reason("property_selection.no_site");
            return if string_receiver {
                None
            } else {
                self.uncached_field_in_state(&state, base, atom, keep_receiver, native)
            };
        };
        if let Some(raw) = cache.read(&state.heap, self.domain_id(), executable.realm, receiver) {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("property_selection.cache");
            let result = self.promote_field_in_state(&state, raw, keep_receiver, native);
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(if result.is_some() {
                "property_selection.data"
            } else {
                "property_selection.general"
            });
            return result;
        }
        let result = self.select_linked_miss(
            &state,
            cache,
            atom,
            executable.realm,
            receiver,
            keep_receiver,
            native,
        );
        record_selection(&result);
        match result {
            NamedDataSelection::Data(value) => Some(value),
            NamedDataSelection::CompleteAbsent => {
                *miss = NamedSelectionMiss::CompleteAbsent;
                None
            }
            NamedDataSelection::Accessor(getter) => {
                *miss = NamedSelectionMiss::Accessor(getter);
                None
            }
            NamedDataSelection::ContinueLookup => None,
            NamedDataSelection::NeedsObservation => {
                *miss = NamedSelectionMiss::NeedsObservation;
                None
            }
        }
    }

    /// Cache adaptation and the cold ordinary walk stay outside the warm
    /// location-hit path. The selected borrowed value is promoted under this
    /// same state borrow before it can escape.
    #[cold]
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn select_linked_miss(
        &self,
        state: &RuntimeState,
        cache: &PropertyReadCache,
        atom: crate::engine::atom::Atom,
        realm: crate::engine::heap::ContextId,
        receiver: crate::engine::heap::ObjectId,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> NamedDataSelection {
        let selected = cache.miss_selected(
            &state.heap,
            &state.atoms,
            self.domain_id(),
            realm,
            Some(receiver),
            atom,
        );
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "property_selection.cache_miss",
        );
        match selected {
            CacheSelection::Data(raw) => self
                .promote_field_in_state(state, raw, keep_receiver, native)
                .map_or(NamedDataSelection::ContinueLookup, NamedDataSelection::Data),
            CacheSelection::CompleteAbsent => NamedDataSelection::CompleteAbsent,
            CacheSelection::Accessor(Some(getter)) => NamedDataSelection::Accessor(getter),
            CacheSelection::Accessor(None) => NamedDataSelection::Data(JsValue::Undefined),
            CacheSelection::Unresolved => {
                // The cache traversal already inspected the ordinary chain.
                // Unsupported storage belongs to the canonical object driver;
                // another own-slot probe here would repeat selection work.
                record_selection_reason("property_selection.unresolved_after_miss");
                NamedDataSelection::ContinueLookup
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn property_ic_read_fast(
        &self,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<JsValue> {
        match self.select_linked_data(base, executable, pc, key_index, keep_receiver, native) {
            NamedDataSelection::Data(value) => Some(value),
            NamedDataSelection::CompleteAbsent => Some(JsValue::Undefined),
            NamedDataSelection::Accessor(_)
            | NamedDataSelection::ContinueLookup
            | NamedDataSelection::NeedsObservation => None,
        }
    }

    fn uncached_field_in_state(
        &self,
        state: &RuntimeState,
        base: &JsValue,
        atom: crate::engine::atom::Atom,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<JsValue> {
        let result = super::field_in_state(state, base, atom, |raw| {
            self.promote_field_in_state(state, raw, keep_receiver, native)
        });
        #[cfg(feature = "profiling")]
        if result.is_some() {
            crate::engine::api::profiling::record_owned_execution_event(
                "property_ic.uncached_field",
            );
        }
        result
    }

    /// The slot and its receiver stay live for this entire borrow. Retaining
    /// the result cannot drain cleanup or invoke JS, and no fallible step
    /// follows a successful retain. Cache hits and uncached own reads use the
    /// same promotion and native-selection contract.
    fn promote_field_in_state(
        &self,
        state: &RuntimeState,
        raw: &RawValue,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<JsValue> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "property_selection.promotion_attempt",
        );
        match raw {
            RawValue::Object(function) => {
                let selected = if keep_receiver {
                    let object = state.heap.object_fast(*function);
                    match &object.payload {
                        ObjectPayload::NativeFunction { data, .. } => {
                            data.realm.and_then(|realm| {
                                (data.operation().is_some() && state.heap.context(realm).is_ok())
                                    .then_some((*function, *data))
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                state.heap.retain_object_fast(*function);
                *native = selected.map(|(function, data)| LinkedNativeSelection {
                    runtime: self.clone(),
                    function,
                    data,
                });
                Some(JsValue::Object(*function))
            }
            RawValue::String(id) => {
                state.heap.retain_string_shared(*id).ok()?;
                Some(JsValue::String(*id))
            }
            RawValue::ShortBigInt(value) => Some(JsValue::ShortBigInt(*value)),
            RawValue::BigInt(id) => {
                state.heap.retain_bigint_shared(*id).ok()?;
                Some(JsValue::BigInt(*id))
            }
            RawValue::Symbol(index) => {
                state.atoms.retain_index_shared(*index).ok()?;
                Some(JsValue::Symbol(*index))
            }
            RawValue::Undefined => Some(JsValue::Undefined),
            RawValue::Null => Some(JsValue::Null),
            RawValue::Bool(value) => Some(JsValue::Bool(*value)),
            RawValue::Int(value) => Some(JsValue::Int(*value)),
            RawValue::Float(value) => Some(JsValue::Float(*value)),
            RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception => None,
        }
    }

    /// Non-owning immediate projection of the location cache.
    ///
    /// Mirrors the `keep_receiver` admission of `property_ic_read_fast` while
    /// creating no owner edge: only a live cache hit whose stored data value is
    /// an immediate number returns `Some`. Every other kind, and every miss,
    /// declines without warming the site, so a guard failure leaves the IC
    /// exactly as the canonical read would have found it. It never records the
    /// owning-hit event because it promotes no owner.
    #[inline]
    pub(crate) fn property_ic_peek_number(
        &self,
        receiver: ObjectId,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
    ) -> Option<Number> {
        linked_field_atom(self, executable, key_index)?;
        let cache = executable.property_read_ic.site(pc)?;
        let state = self.0.state.try_borrow().ok()?;
        let raw = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)?;
        match raw {
            RawValue::Int(value) => Some(Number::Int(*value)),
            RawValue::Float(value) => Some(Number::Float(*value)),
            _ => None,
        }
    }
}

impl Runtime {
    pub(crate) fn try_dense_array_kept_read(&self, base: &JsValue, index: u32) -> Option<JsValue> {
        let JsValue::Object(object) = base else {
            return None;
        };
        let state = self.0.state.borrow();
        let data = state.heap.object(*object).ok()?;
        if !matches!(data.kind, crate::engine::heap::ObjectKind::Array) {
            return None;
        }
        super::immediate_value_jsvalue(data.dense_array_value(index)?)
    }
}

impl Runtime {
    pub(crate) fn try_dense_array_write_scalar(
        &self,
        base: &JsValue,
        index: u32,
        value: &JsValue,
    ) -> Result<bool, RuntimeError> {
        if !matches!(
            value,
            JsValue::Undefined
                | JsValue::Null
                | JsValue::Bool(_)
                | JsValue::Int(_)
                | JsValue::Float(_)
                | JsValue::ShortBigInt(_)
        ) || self.slot_value_release_readiness_jsvalue(base)? != SlotReleaseReadiness::Ready
        {
            return Ok(false);
        }
        let JsValue::Object(object) = base else {
            return Ok(false);
        };
        let mut state = self.0.state.borrow_mut();
        Ok(state
            .heap
            .try_replace_dense_immediate_value(*object, index, value.as_raw()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::code::bytecode::Instruction;
    use crate::engine::value::Value;

    fn object(value: &JsValue) -> crate::engine::heap::ObjectId {
        let JsValue::Object(object) = value else {
            panic!("object")
        };
        *object
    }

    fn site_for(runtime: &Runtime, source: &str) -> (PublishedFunctionSnapshot, usize, u32) {
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval(source).unwrap())
            .unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode")
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        let (pc, key) = executable
            .exec
            .test_ir()
            .iter()
            .enumerate()
            .find_map(|(pc, op)| match op {
                Instruction::GetField(key) => Some((pc, *key)),
                _ => None,
            })
            .unwrap();
        let pc = executable.exec.exec_pc(pc as u32).unwrap() as usize;
        (executable, pc, key)
    }

    fn site(runtime: &Runtime) -> (PublishedFunctionSnapshot, usize, u32) {
        site_for(runtime, "(function(o){return o.x})")
    }

    #[test]
    fn initialized_function_slot_completes_locally_after_lazy_decline() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site_for(&runtime, "(function(o){return o.prototype})");
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readFunction=function sample(){};readFunction")
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None),
            NamedDataSelection::ContinueLookup
        ));
        let _ = context.eval("readFunction.prototype").unwrap();
        let selected = runtime.select_linked_data(&base, &code, pc, key, true, &mut None);
        let NamedDataSelection::Data(result @ JsValue::Object(_)) = selected else {
            panic!("initialized function slot should yield an owned result")
        };
        runtime.release_jsvalue(result).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn string_prototype_selection_preserves_data_and_getter_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(context.eval("(function(o){return o.charAt})").unwrap())
            .unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode")
        };
        let code = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        let (pc, key) = code
            .exec
            .test_ir()
            .iter()
            .enumerate()
            .find_map(|(pc, op)| match op {
                Instruction::GetField(key) => Some((pc, *key)),
                _ => None,
            })
            .unwrap();
        let pc = code.exec.exec_pc(pc as u32).unwrap() as usize;
        let base = runtime
            .into_jsvalue(context.eval("'sample'").unwrap())
            .unwrap();
        let _ = context.eval("String.prototype.charAt").unwrap();
        let NamedDataSelection::Data(value @ JsValue::Object(_)) =
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None)
        else {
            panic!("String prototype method should be selected locally")
        };
        runtime.release_jsvalue(value).unwrap();
        let _ = context
            .eval("globalThis.readCalls=0;Object.defineProperty(String.prototype,'charAt',{get(){readCalls++;return 17},configurable:true})")
            .unwrap();
        assert!(matches!(
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None),
            NamedDataSelection::Accessor(_)
        ));
        assert_eq!(context.eval("readCalls").unwrap(), Value::Int(0));
        assert_eq!(context.eval("'sample'.charAt").unwrap(), Value::Int(17));
        assert_eq!(context.eval("readCalls").unwrap(), Value::Int(1));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn shared_selection_uses_cold_inherited_data_and_current_warm_value() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readBase=Object.create({x:7});readBase")
                    .unwrap(),
            )
            .unwrap();
        let selected = |runtime: &Runtime| match runtime
            .select_linked_data(&base, &code, pc, key, true, &mut None)
        {
            NamedDataSelection::Data(JsValue::Int(value)) => value,
            _ => panic!("expected selected data"),
        };
        assert_eq!(selected(&runtime), 7);
        let _ = context.eval("Object.getPrototypeOf(readBase).x=9").unwrap();
        assert_eq!(selected(&runtime), 9);
        let _ = context
            .eval("delete Object.getPrototypeOf(readBase).x")
            .unwrap();
        assert!(matches!(
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None),
            NamedDataSelection::CompleteAbsent
        ));
        let _ = context.eval("globalThis.readCalls=0;Object.defineProperty(Object.getPrototypeOf(readBase),'x',{get(){readCalls++;return 11}})").unwrap();
        assert!(matches!(
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None),
            NamedDataSelection::Accessor(_)
        ));
        assert_eq!(context.eval("readCalls").unwrap(), Value::Int(0));
        assert_eq!(context.eval("readBase.x").unwrap(), Value::Int(11));
        assert_eq!(context.eval("readCalls").unwrap(), Value::Int(1));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn cold_and_warm_accessor_selection_tracks_current_shape() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readAccessor={get x(){return 3}};readAccessor")
                    .unwrap(),
            )
            .unwrap();
        let selected = |runtime: &Runtime| match runtime
            .select_linked_data(&base, &code, pc, key, true, &mut None)
        {
            NamedDataSelection::Accessor(getter) => getter,
            _ => panic!("expected selected accessor"),
        };
        let getter = selected(&runtime);
        assert_eq!(selected(&runtime), getter);
        let _ = context
            .eval("Object.defineProperty(readAccessor,'x',{value:7,configurable:true})")
            .unwrap();
        assert!(matches!(
            runtime.select_linked_data(&base, &code, pc, key, true, &mut None),
            NamedDataSelection::Data(JsValue::Int(7))
        ));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn uncached_own_read_retains_every_owner_after_last_base_release() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        // Three distinct shapes put this site into its megamorphic cooldown.
        // Every following value must therefore use the uncached own-slot path.
        for expression in ["({x:1})", "({a:0,x:2})", "({b:0,a:0,x:3})"] {
            let base = runtime
                .into_jsvalue(context.eval(expression).unwrap())
                .unwrap();
            let result = runtime
                .property_ic_read_fast(&base, &code, pc, key, true, &mut None)
                .unwrap();
            runtime.release_jsvalue(result).unwrap();
            runtime.release_jsvalue(base).unwrap();
        }
        for expression in [
            "({nested:7})",
            "'wide λ text'",
            "123456789012345678901234567890n",
            "Symbol.for('uncached-read')",
            "undefined",
            "null",
            "true",
            "1.25",
        ] {
            let base = runtime
                .into_jsvalue(context.eval(&format!("({{x:{expression}}})")).unwrap())
                .unwrap();
            let result = runtime
                .property_ic_read_fast(&base, &code, pc, key, true, &mut None)
                .unwrap_or_else(|| panic!("uncached {expression}"));
            runtime.release_jsvalue(base).unwrap();
            runtime.run_gc().unwrap();
            let result = runtime.root_and_release_jsvalue(result).unwrap();
            if let Value::Object(object) = result {
                assert_eq!(
                    context
                        .get_property(&object, &runtime.intern_property_key("nested").unwrap())
                        .unwrap(),
                    Value::Int(7)
                );
            } else {
                assert_eq!(result, context.eval(expression).unwrap(), "{expression}");
            }
        }
    }

    #[test]
    fn uncached_own_read_declines_accessors_proxies_and_last_owner_consumption() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        for expression in [
            "({get x(){throw 1}})",
            "new Proxy({x:1},{get(){throw 2}})",
            "Object.create({get x(){throw 3}})",
        ] {
            let base = runtime
                .into_jsvalue(context.eval(expression).unwrap())
                .unwrap();
            assert!(
                runtime
                    .property_ic_read_fast(&base, &code, pc, key, true, &mut None)
                    .is_none()
            );
            runtime.release_jsvalue(base).unwrap();
        }
        let base = runtime
            .into_jsvalue(context.eval("({x:{marker:1}})").unwrap())
            .unwrap();
        assert!(
            runtime
                .property_ic_read_fast(&base, &code, pc, key, false, &mut None)
                .is_none()
        );
        let result = runtime
            .property_ic_read_fast(&base, &code, pc, key, true, &mut None)
            .unwrap();
        runtime.release_jsvalue(result).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn uncached_native_hint_describes_the_retained_current_value() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let poison = runtime
            .into_jsvalue(context.eval("({get x(){throw 1}})").unwrap())
            .unwrap();
        assert!(
            runtime
                .property_ic_read_fast(&poison, &code, pc, key, true, &mut None)
                .is_none()
        );
        runtime.release_jsvalue(poison).unwrap();
        let base = runtime
            .into_jsvalue(context.eval("({x:Math.min})").unwrap())
            .unwrap();
        let mut native = None;
        let result = runtime
            .property_ic_read_fast(&base, &code, pc, key, true, &mut native)
            .unwrap();
        assert!(
            native
                .unwrap()
                .into_parts_jsvalue(&runtime, object(&result))
                .is_some()
        );
        runtime.release_jsvalue(base).unwrap();
        runtime.release_jsvalue(result).unwrap();
    }

    #[test]
    fn owned_ic_promotes_every_public_value_and_reads_current_slot() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for expression in [
            "undefined",
            "null",
            "true",
            "123",
            "1.25",
            "'wide λ text'",
            "123456789012345678901234567890n",
            "Symbol('ic')",
            "({nested:7})",
        ] {
            let (code, pc, key) = site(&runtime);
            let base = runtime
                .into_jsvalue(
                    context
                        .eval(&format!(
                            "globalThis.icExpected={expression};globalThis.icHolder={{x:icExpected}};icHolder"
                        ))
                        .unwrap(),
                )
                .unwrap();
            let expected = context.eval("icExpected").unwrap();
            let mut native = None;
            assert!(
                runtime
                    .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                    .unwrap()
                    .is_none()
            );
            let actual = runtime
                .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                .unwrap()
                .unwrap();
            assert_eq!(
                runtime.root_and_release_jsvalue(actual).unwrap(),
                expected,
                "{expression}"
            );
            drop(context.eval("icHolder.x=99").unwrap());
            let after = runtime
                .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                .unwrap();
            assert_eq!(
                after.map(|value| runtime.root_and_release_jsvalue(value).unwrap()),
                Some(Value::Int(99))
            );
            assert!(native.is_none());
            runtime.release_jsvalue(base).unwrap();
        }
    }

    #[test]
    fn owned_ic_guards_borrow_deferred_work_and_final_receiver_before_promotion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(context.eval("({x:{marker:1}})").unwrap())
            .unwrap();
        let mut native = None;
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                .unwrap()
                .is_none()
        );
        // A cache hit must still leave the last receiver owner untouched.
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, false, &mut native)
                .unwrap()
                .is_none()
        );
        let receiver = object(&base);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(receiver)
                .unwrap(),
            1
        );
        {
            let _borrow = runtime.0.state.borrow();
            assert!(
                runtime
                    .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                    .unwrap()
                    .is_none()
            );
        }
        let released = runtime.new_object(None).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        // A kept receiver hit only retains under the exclusive heap borrow;
        // pending unrelated releases cannot mutate its guarded layout.
        let first_hit = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap();
        assert!(matches!(first_hit, Some(JsValue::Object(_))));
        assert!(runtime.0.deferred_references.has_pending());
        runtime.drain_deferred_references().unwrap();
        let second_hit = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap();
        assert!(matches!(second_hit, Some(JsValue::Object(_))));
        if let Some(value) = first_hit {
            runtime.release_jsvalue(value).unwrap();
        }
        if let Some(value) = second_hit {
            runtime.release_jsvalue(value).unwrap();
        }
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn owned_ic_native_hint_is_bound_to_current_retained_function() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.icNative={x:Math.min};icNative")
                    .unwrap(),
            )
            .unwrap();
        let mut native = None;
        assert!(
            runtime
                .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
                .unwrap()
                .is_none()
        );
        let first = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap()
            .unwrap();
        let first_object =
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), object(&first))
                .unwrap();
        let hint = native.take().unwrap();
        drop(context.eval("icNative.x=Math.max").unwrap());
        let data = hint
            .into_parts_jsvalue(first_object.runtime(), first_object.object_id())
            .unwrap();
        assert_eq!(
            data.target,
            crate::engine::builtins::native::NativeFunctionId::MathMinMax(
                crate::engine::builtins::native::MathMinMaxKind::Min
            )
        );
        let second = runtime
            .try_property_ic_read_owned(&base, &code, pc, key, true, &mut native)
            .unwrap()
            .unwrap();
        let hint = native.take().unwrap();
        assert!(
            hint.into_parts_jsvalue(first_object.runtime(), first_object.object_id())
                .is_none()
        );
        assert_ne!(first, second);
        runtime.release_jsvalue(first).unwrap();
        runtime.release_jsvalue(second).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }
}
