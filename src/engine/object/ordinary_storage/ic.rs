//! Promote a location-cache hit without draining runtime cleanup or invoking JS.
use super::{LinkedNativeSelection, NamedDataSelection, NamedSelectionMiss};
#[cfg(test)]
use crate::engine::api::runtime::Runtime;
use crate::engine::builtins::native::PrimitiveKind;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectPayload, RawValue};
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
    });
}

#[inline(always)]
fn record_selection_reason(_reason: &'static str) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(_reason);
}

impl RuntimeState {
    /// Return an owned data value directly; only a declined read writes the
    /// small cold outcome. Selection and promotion still happen once under the
    /// same heap borrow for both ordinary and borrowed-receiver consumers.
    /// The execution entry pairs this state with its RuntimeInner domain and
    /// pins the executable's bytecode through the frame's callee owner. This
    /// operation does not consume the base; its owner must remain live until
    /// selection completes, and the returned value owns its retained edge.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn select_linked_data_into(
        &self,
        domain_id: u64,
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
        let Some(atom) = super::linked_field_atom_in_domain(domain_id, executable, key_index)
        else {
            record_selection_reason("property_selection.decline_unlinked_key");
            return None;
        };
        let cache = executable.property_read_ic.site(pc);
        let (receiver, string_receiver) = match base {
            JsValue::Object(object) => (*object, false),
            JsValue::String(id) => {
                let length = self
                    .pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Length);
                if atom == length {
                    let length = self.heap.string_fast(*id).len();
                    record_selection_reason("property_selection.string_length");
                    return Some(
                        i32::try_from(length)
                            .map_or_else(|_| JsValue::Float(length as f64), JsValue::Int),
                    );
                }
                let Ok(index) = self.atoms.array_index(atom) else {
                    return None;
                };
                if index.is_some_and(|index| {
                    usize::try_from(index)
                        .is_ok_and(|index| index < self.heap.string_fast(*id).len())
                }) {
                    // An indexed character creates a string node. Its
                    // allocation and publication remain in the driver.
                    record_selection_reason("property_selection.string_index_allocation");
                    return None;
                }
                let prototype =
                    self.heap.context(executable.realm).ok().and_then(|realm| {
                        realm.primitive_prototypes[PrimitiveKind::String.index()]
                    })?;
                record_selection_reason("property_selection.string_prototype");
                (prototype, true)
            }
            _ => unreachable!("non-string primitive returned before the heap borrow"),
        };
        let Some(cache) = cache else {
            record_selection_reason("property_selection.no_site");
            return if string_receiver {
                None
            } else {
                self.uncached_field_in_state(domain_id, base, atom, keep_receiver, native)
            };
        };
        if let Some(raw) = cache.read(&self.heap, domain_id, executable.realm, receiver) {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("property_selection.cache");
            let result = self.promote_field_in_state(domain_id, raw, keep_receiver, native);
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(if result.is_some() {
                "property_selection.data"
            } else {
                "property_selection.general"
            });
            return result;
        }
        let result = self.select_linked_miss(
            domain_id,
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
        domain_id: u64,
        cache: &PropertyReadCache,
        atom: crate::engine::atom::Atom,
        realm: crate::engine::heap::ContextId,
        receiver: crate::engine::heap::ObjectId,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> NamedDataSelection {
        let selected = cache.miss_selected(
            &self.heap,
            &self.atoms,
            domain_id,
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
                .promote_field_in_state(domain_id, raw, keep_receiver, native)
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

    fn uncached_field_in_state(
        &self,
        domain_id: u64,
        base: &JsValue,
        atom: crate::engine::atom::Atom,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<JsValue> {
        let result = super::field_in_state(self, base, atom, |raw| {
            self.promote_field_in_state(domain_id, raw, keep_receiver, native)
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
        domain_id: u64,
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
                    let object = self.heap.object_fast(*function);
                    match &object.payload {
                        ObjectPayload::NativeFunction { data, .. } => {
                            data.realm.and_then(|realm| {
                                (data.operation().is_some() && self.heap.context(realm).is_ok())
                                    .then_some((*function, *data))
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                self.heap.retain_object_fast(*function);
                *native = selected.map(|(function, data)| LinkedNativeSelection {
                    domain_id,
                    function,
                    data,
                });
                Some(JsValue::Object(*function))
            }
            RawValue::String(id) => {
                self.heap.retain_string_shared(*id).ok()?;
                Some(JsValue::String(*id))
            }
            RawValue::ShortBigInt(value) => Some(JsValue::ShortBigInt(*value)),
            RawValue::BigInt(id) => {
                self.heap.retain_bigint_shared(*id).ok()?;
                Some(JsValue::BigInt(*id))
            }
            RawValue::Symbol(index) => {
                self.atoms.retain_index_shared(*index).ok()?;
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
    /// Only a live cache hit whose stored data value is an immediate number
    /// returns `Some`. Every other kind, and every miss, declines without
    /// warming the site or creating an owner edge. It never records the
    /// owning-hit event because it promotes no owner.
    #[inline]
    pub(crate) fn property_ic_peek_number(
        &self,
        domain_id: u64,
        receiver: ObjectId,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
    ) -> Option<Number> {
        super::linked_field_atom_in_domain(domain_id, executable, key_index)?;
        let cache = executable.property_read_ic.site(pc)?;
        let raw = cache.read(&self.heap, domain_id, executable.realm, receiver)?;
        match raw {
            RawValue::Int(value) => Some(Number::Int(*value)),
            RawValue::Float(value) => Some(Number::Float(*value)),
            _ => None,
        }
    }
}

impl RuntimeState {
    pub(crate) fn try_dense_array_kept_read(&self, base: &JsValue, index: u32) -> Option<JsValue> {
        let JsValue::Object(object) = base else {
            return None;
        };
        let data = self.heap.object(*object).ok()?;
        if !matches!(data.kind, crate::engine::heap::ObjectKind::Array) {
            return None;
        }
        super::immediate_value_jsvalue(data.dense_array_value(index)?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{StateSetProbe, linked_field_atom_in_domain};
    use super::*;
    use crate::engine::code::bytecode::Instruction;
    use crate::engine::value::Value;

    #[allow(clippy::too_many_arguments)]
    fn select(
        state: &RuntimeState,
        domain: u64,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> NamedDataSelection {
        let mut miss = NamedSelectionMiss::ContinueLookup;
        if let Some(value) = state.select_linked_data_into(
            domain,
            base,
            executable,
            pc,
            key,
            keep_receiver,
            native,
            &mut miss,
        ) {
            NamedDataSelection::Data(value)
        } else if let NamedSelectionMiss::Accessor(getter) = miss {
            NamedDataSelection::Accessor(getter)
        } else if matches!(miss, NamedSelectionMiss::CompleteAbsent) {
            NamedDataSelection::CompleteAbsent
        } else {
            NamedDataSelection::ContinueLookup
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        state: &RuntimeState,
        domain: u64,
        base: &JsValue,
        executable: &PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
        keep_receiver: bool,
        native: &mut Option<LinkedNativeSelection>,
    ) -> Option<JsValue> {
        match select(
            state,
            domain,
            base,
            executable,
            pc,
            key,
            keep_receiver,
            native,
        ) {
            NamedDataSelection::Data(value) => Some(value),
            NamedDataSelection::CompleteAbsent => Some(JsValue::Undefined),
            _ => None,
        }
    }

    fn object(value: &JsValue) -> crate::engine::heap::ObjectId {
        let JsValue::Object(object) = value else {
            panic!("object")
        };
        *object
    }

    fn site_for(runtime: &Runtime, source: &str) -> (PublishedFunctionSnapshot, usize, u32) {
        let mut context = runtime.new_context().expect("create context");
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
    fn direct_state_read_keeps_result_after_final_receiver_release() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(context.eval("({x:{marker:42}})").unwrap())
            .unwrap();
        let receiver = object(&base);
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
        let value = state
            .select_linked_data_into(
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                false,
                &mut None,
                &mut NamedSelectionMiss::ContinueLookup,
            )
            .unwrap();
        let result = object(&value);
        state.release_jsvalue(base).unwrap();
        assert!(state.heap.object(receiver).is_err());
        assert_eq!(state.heap.object_strong_count(result), Ok(1));
        state.release_jsvalue(value).unwrap();
        assert!(state.heap.object(result).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn direct_state_selection_preserves_prototypes_and_getter_progress() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let inherited = runtime
            .into_jsvalue(context.eval("Object.create({x:42})").unwrap())
            .unwrap();
        let getter = runtime.into_jsvalue(context.eval("globalThis.directGetterCalls=0;Object.create({get x(){directGetterCalls++;return 7}})").unwrap()).unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            assert_eq!(
                state.select_linked_data_into(
                    runtime.domain_id(),
                    &inherited,
                    &code,
                    pc,
                    key,
                    false,
                    &mut None,
                    &mut NamedSelectionMiss::ContinueLookup
                ),
                Some(JsValue::Int(42))
            );
            let mut miss = NamedSelectionMiss::ContinueLookup;
            assert!(
                state
                    .select_linked_data_into(
                        runtime.domain_id(),
                        &getter,
                        &code,
                        pc,
                        key,
                        false,
                        &mut None,
                        &mut miss
                    )
                    .is_none()
            );
            assert!(matches!(miss, NamedSelectionMiss::Accessor(_)));
            state.release_jsvalue(inherited).unwrap();
            state.release_jsvalue(getter).unwrap();
        }
        assert_eq!(context.eval("directGetterCalls").unwrap(), Value::Int(0));
    }

    #[test]
    fn direct_state_string_length_requires_no_secondary_state_borrow() {
        let runtime = Runtime::new();
        let (code, pc, key) = site_for(&runtime, "(function(o){return o.length})");
        let base = runtime
            .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                "a🙂b",
            )))
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(
            state.select_linked_data_into(
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                false,
                &mut None,
                &mut NamedSelectionMiss::ContinueLookup
            ),
            Some(JsValue::Int(4))
        );
        state.release_jsvalue(base).unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn direct_state_failed_promotion_keeps_receiver_and_slot_owner() {
        use crate::engine::heap::RawId;
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(context.eval("({x:'promotion ownership'})").unwrap())
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let atom = code.property_key_atoms.as_ref().unwrap()[key as usize];
        let slot = super::super::locate(&state, object(&base), atom)
            .unwrap()
            .unwrap();
        let crate::engine::heap::PropertySlot::Data(RawValue::String(string)) = state
            .heap
            .object(object(&base))
            .unwrap()
            .slots
            .get(slot.index)
            .unwrap()
        else {
            panic!("string slot")
        };
        let string = *string;
        let previous = state.heap.strong_count(RawId::String(string)).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::String(string), u32::MAX);
        let result = state.select_linked_data_into(
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            false,
            &mut None,
            &mut NamedSelectionMiss::ContinueLookup,
        );
        let count = state.heap.strong_count(RawId::String(string)).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::String(string), previous);
        assert!(result.is_none());
        assert_eq!(count, u32::MAX);
        assert_eq!(state.heap.object_strong_count(object(&base)), Ok(1));
        state.release_jsvalue(base).unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn direct_state_native_fact_holds_no_runtime_owner_and_rejects_foreign_domain() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(context.eval("({x:Math.min})").unwrap())
            .unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let mut native = None;
        let value = state
            .select_linked_data_into(
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut native,
                &mut NamedSelectionMiss::ContinueLookup,
            )
            .unwrap();
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(
            native
                .take()
                .unwrap()
                .into_parts_jsvalue(&foreign, object(&value))
                .is_none()
        );
        assert!(
            state
                .select_linked_data_into(
                    foreign.domain_id(),
                    &base,
                    &code,
                    pc,
                    key,
                    true,
                    &mut None,
                    &mut NamedSelectionMiss::ContinueLookup
                )
                .is_none()
        );
        state.release_jsvalue(value).unwrap();
        state.release_jsvalue(base).unwrap();
    }

    #[test]
    fn canonical_state_field_and_dense_writes_preserve_rejections() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let atom = linked_field_atom_in_domain(runtime.domain_id(), &code, key).unwrap();
        let index = crate::engine::atom::Atom::from_immediate_integer(0).unwrap();
        let base = runtime
            .into_jsvalue(context.eval("({x:1})").unwrap())
            .unwrap();
        let frozen = runtime
            .into_jsvalue(context.eval("Object.freeze({x:2})").unwrap())
            .unwrap();
        let dense = runtime
            .into_jsvalue(context.eval("[1,2,3]").unwrap())
            .unwrap();
        let frozen_dense = runtime
            .into_jsvalue(context.eval("Object.freeze([5])").unwrap())
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            state
                .ordinary_set_probe_in_state(
                    &runtime.0.poisoned,
                    object(&base),
                    atom,
                    &JsValue::Int(42),
                    true,
                    true
                )
                .unwrap(),
            StateSetProbe::Stored(true)
        ));
        assert!(matches!(
            state
                .ordinary_set_probe_in_state(
                    &runtime.0.poisoned,
                    object(&frozen),
                    atom,
                    &JsValue::Int(42),
                    true,
                    true
                )
                .unwrap(),
            StateSetProbe::Stored(false)
        ));
        assert_eq!(
            state.select_linked_data_into(
                runtime.domain_id(),
                &frozen,
                &code,
                pc,
                key,
                false,
                &mut None,
                &mut NamedSelectionMiss::ContinueLookup
            ),
            Some(JsValue::Int(2))
        );
        assert_eq!(
            state.try_array_immediate_read(&dense, 0),
            Some(JsValue::Int(1))
        );
        assert!(matches!(
            state
                .ordinary_set_probe_in_state(
                    &runtime.0.poisoned,
                    object(&dense),
                    index,
                    &JsValue::Int(9),
                    true,
                    true
                )
                .unwrap(),
            StateSetProbe::Stored(true)
        ));
        assert_eq!(
            state.try_dense_array_kept_read(&dense, 0),
            Some(JsValue::Int(9))
        );
        // These numeric compound-operation kernels remain actual VM consumers.
        state
            .try_add_array_own_number(&dense, 0, Number::Int(1))
            .unwrap();
        state
            .try_replace_array_own_number(&dense, 1, Number::Int(11))
            .unwrap();
        assert!(matches!(
            state.peek_dense_number(&dense, 0),
            Some(Number::Int(10))
        ));
        assert!(
            state
                .try_replace_array_own_number(&frozen_dense, 0, Number::Int(99))
                .is_err()
        );
        assert!(matches!(
            state.peek_dense_number(&frozen_dense, 0),
            Some(Number::Int(5))
        ));
        for owner in [base, frozen, dense, frozen_dense] {
            state.release_jsvalue(owner).unwrap();
        }
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn canonical_state_field_write_preserves_receiver_owner_count() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, _, key) = site(&runtime);
        let atom = linked_field_atom_in_domain(runtime.domain_id(), &code, key).unwrap();
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.scalarBase = {x:1}; scalarBase")
                    .unwrap(),
            )
            .unwrap();
        let owners = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(object(&base))
            .unwrap();
        assert!(matches!(
            runtime
                .0
                .state
                .borrow_mut()
                .ordinary_set_probe_in_state(
                    &runtime.0.poisoned,
                    object(&base),
                    atom,
                    &JsValue::Int(42),
                    true,
                    true
                )
                .unwrap(),
            StateSetProbe::Stored(true)
        ));
        assert_eq!(context.eval("scalarBase.x").unwrap(), Value::Int(42));
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(object(&base))
                .unwrap(),
            owners
        );
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn scalar_field_vm_preserves_assignment_results_and_fallbacks() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"
            (() => {
                let calls = 0;
                const o = {x:1, y:0};
                const p = {set x(v){calls++; o.y = v + 1;}};
                Object.setPrototypeOf(o, p);
                if ((o.x = 3) !== 3 || ++o.x !== 4 || o.x++ !== 4 || o.x !== 5 || calls) return false;
                Object.defineProperty(o, 'x', {get(){return 9}, set(v){calls++; p.x = v}, configurable:true});
                if ((o.x = 4) !== 4 || calls !== 2) return false;
                delete o.x;
                Object.setPrototypeOf(o, null);
                o.x = {marker:1};
                o.x = 7;
                if (o.x !== 7) return false;
                const frozen = Object.freeze({x:1}); frozen.x = 2;
                try { (function(){'use strict'; frozen.x = 3;})(); return false; }
                catch(e) { if (!(e instanceof TypeError)) return false; }
                const proxy = new Proxy(o, {set(t,k,v,r){calls++; return Reflect.set(t,k,v,r)}});
                if ((proxy.x = 8) !== 8 || o.x !== 8 || calls !== 3) return false;
                const a = [1,2,3]; a.length = 1;
                if (a.length !== 1 || a[1] !== undefined) return false;
                ({}).x = 1;
                return frozen.x === 1;
            })()
        "#).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn canonical_set_vm_records_local_completion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let _ = context.eval("globalThis.scalarProfile = {x:0};").unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval("for (let i=0; i<16; i++) scalarProfile.x = i; scalarProfile.x")
                .unwrap(),
            Value::Int(15)
        );
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("core.internal_property_write")
                .copied(),
            Some(16)
        );
    }

    #[test]
    fn initialized_function_slot_completes_locally_after_lazy_decline() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site_for(&runtime, "(function(o){return o.prototype})");
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readFunction=function sample(){};readFunction")
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            select(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None
            ),
            NamedDataSelection::ContinueLookup
        ));
        let _ = context.eval("readFunction.prototype").unwrap();
        let selected = select(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut None,
        );
        let NamedDataSelection::Data(result @ JsValue::Object(_)) = selected else {
            panic!("initialized function slot should yield an owned result")
        };
        runtime.release_jsvalue(result).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn string_prototype_selection_preserves_data_and_getter_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
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
        let NamedDataSelection::Data(value @ JsValue::Object(_)) = select(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut None,
        ) else {
            panic!("String prototype method should be selected locally")
        };
        runtime.release_jsvalue(value).unwrap();
        let _ = context
            .eval("globalThis.readCalls=0;Object.defineProperty(String.prototype,'charAt',{get(){readCalls++;return 17},configurable:true})")
            .unwrap();
        assert!(matches!(
            select(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None
            ),
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
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readBase=Object.create({x:7});readBase")
                    .unwrap(),
            )
            .unwrap();
        let selected = |runtime: &Runtime| match select(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut None,
        ) {
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
            select(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None
            ),
            NamedDataSelection::CompleteAbsent
        ));
        let _ = context.eval("globalThis.readCalls=0;Object.defineProperty(Object.getPrototypeOf(readBase),'x',{get(){readCalls++;return 11}})").unwrap();
        assert!(matches!(
            select(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None
            ),
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
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.readAccessor={get x(){return 3}};readAccessor")
                    .unwrap(),
            )
            .unwrap();
        let selected = |runtime: &Runtime| match select(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut None,
        ) {
            NamedDataSelection::Accessor(getter) => getter,
            _ => panic!("expected selected accessor"),
        };
        let getter = selected(&runtime);
        assert_eq!(selected(&runtime), getter);
        let _ = context
            .eval("Object.defineProperty(readAccessor,'x',{value:7,configurable:true})")
            .unwrap();
        assert!(matches!(
            select(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None
            ),
            NamedDataSelection::Data(JsValue::Int(7))
        ));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn uncached_own_read_retains_every_owner_after_last_base_release() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        // Three distinct shapes put this site into its megamorphic cooldown.
        // Every following value must therefore use the uncached own-slot path.
        for expression in ["({x:1})", "({a:0,x:2})", "({b:0,a:0,x:3})"] {
            let base = runtime
                .into_jsvalue(context.eval(expression).unwrap())
                .unwrap();
            let result = read(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None,
            )
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
            let result = read(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                true,
                &mut None,
            )
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
    fn uncached_own_read_declines_accessors_and_proxies() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
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
                read(
                    &runtime.0.state.borrow(),
                    runtime.domain_id(),
                    &base,
                    &code,
                    pc,
                    key,
                    true,
                    &mut None
                )
                .is_none()
            );
            runtime.release_jsvalue(base).unwrap();
        }
        let base = runtime
            .into_jsvalue(context.eval("({x:{marker:1}})").unwrap())
            .unwrap();
        let result = read(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            false,
            &mut None,
        )
        .unwrap();
        let result_id = object(&result);
        runtime.release_jsvalue(base).unwrap();
        assert!(runtime.0.state.borrow().heap.object(result_id).is_ok());
        runtime.release_jsvalue(result).unwrap();
    }

    #[test]
    fn uncached_native_hint_describes_the_retained_current_value() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let poison = runtime
            .into_jsvalue(context.eval("({get x(){throw 1}})").unwrap())
            .unwrap();
        assert!(
            read(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &poison,
                &code,
                pc,
                key,
                true,
                &mut None
            )
            .is_none()
        );
        runtime.release_jsvalue(poison).unwrap();
        let base = runtime
            .into_jsvalue(context.eval("({x:Math.min})").unwrap())
            .unwrap();
        let mut native = None;
        let result = read(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut native,
        )
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
        let mut context = runtime.new_context().expect("create context");
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
            let actual = read(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                false,
                &mut native,
            )
            .unwrap();
            assert_eq!(
                runtime.root_and_release_jsvalue(actual).unwrap(),
                expected,
                "{expression}"
            );
            drop(context.eval("icHolder.x=99").unwrap());
            let after = read(
                &runtime.0.state.borrow(),
                runtime.domain_id(),
                &base,
                &code,
                pc,
                key,
                false,
                &mut native,
            );
            assert_eq!(
                after.map(|value| runtime.root_and_release_jsvalue(value).unwrap()),
                Some(Value::Int(99))
            );
            assert!(native.is_none());
            runtime.release_jsvalue(base).unwrap();
        }
    }

    #[test]
    fn owned_ic_native_hint_is_bound_to_current_retained_function() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (code, pc, key) = site(&runtime);
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.icNative={x:Math.min};icNative")
                    .unwrap(),
            )
            .unwrap();
        let mut native = None;
        let first = read(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut native,
        )
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
        let second = read(
            &runtime.0.state.borrow(),
            runtime.domain_id(),
            &base,
            &code,
            pc,
            key,
            true,
            &mut native,
        )
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
