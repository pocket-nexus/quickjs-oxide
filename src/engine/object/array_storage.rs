//! Batched Array representation and length changes at mutation boundaries.
//!
//! Only genuine Arrays reach this path. It runs no JavaScript: accessor values
//! are never read, and layout publication owns edge movement or retain/release.
//! Length validation, conversion and rollback stay in the property algorithm.

#[cfg(feature = "profiling")]
use crate::engine::api::profiling::record_owned_execution_event;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::{HeapError, ObjectPayload, PropertySlot, Slots};
use crate::engine::object::{ObjectRef, shape::PropertyFlags};

// A recovery attempt is optional work following a successful definition. Bound
// both its scan and temporary storage; this policy can change with workloads.
// This also keeps every recovered index in the immediate-integer atom range.
const MAX_DENSE_RECOVERY_SLOTS: usize = 8192;

impl Runtime {
    /// A newly defined zero can complete a reverse fill. Rebuild dense storage
    /// only at this mutation boundary, after proving that every index is an own
    /// default data property. Reads never pay for this recovery decision.
    pub(super) fn try_recover_dense_array(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        #[cfg(feature = "profiling")]
        record_owned_execution_event("array_storage_dense_recovery_enter");
        let (length, _) = self.array_length_state(object)?;
        let length = length as usize;
        let mut state = self.0.state.borrow_mut();
        let id = object.object_id();
        let (prototype, entries, indexed_slots) = {
            let data = state.heap.object(id)?;
            // Each rejection counter names the first failed predicate in this
            // order, not every property that might also be unsuitable.
            if !matches!(data.payload, ObjectPayload::Array { dense: None }) {
                #[cfg(feature = "profiling")]
                record_owned_execution_event("array_storage_dense_recovery_reject_not_slow_array");
                return Ok(false);
            }
            if length < 2 {
                #[cfg(feature = "profiling")]
                record_owned_execution_event("array_storage_dense_recovery_reject_short_length");
                return Ok(false);
            }
            if data.slots.len() > MAX_DENSE_RECOVERY_SLOTS {
                #[cfg(feature = "profiling")]
                record_owned_execution_event("array_storage_dense_recovery_reject_slot_cap");
                return Ok(false);
            }
            // At least `length` elements plus the mandatory length slot.
            // This rejects huge sparse indices before any allocation/scan.
            if data.slots.len() <= length {
                #[cfg(feature = "profiling")]
                record_owned_execution_event(
                    "array_storage_dense_recovery_reject_insufficient_slots",
                );
                return Ok(false);
            }
            let shape = state.heap.shape(data.shape)?;
            if shape.entries().len() != data.slots.len() {
                return Err(RuntimeError::Invariant(
                    "Array shape and slots differ during recovery",
                ));
            }
            let mut indexed_slots = Vec::new();
            if indexed_slots.try_reserve_exact(length).is_err() {
                #[cfg(feature = "profiling")]
                record_owned_execution_event(
                    "array_storage_dense_recovery_reject_index_map_allocation",
                );
                return Ok(false);
            }
            indexed_slots.resize(length, usize::MAX);
            // A complete payload uses exactly `length` index slots. Any
            // additional slot must stay in the replacement named shape.
            let named_count = data.slots.len() - length;
            let mut entries = Vec::new();
            if entries.try_reserve_exact(named_count).is_err() {
                #[cfg(feature = "profiling")]
                record_owned_execution_event(
                    "array_storage_dense_recovery_reject_named_entries_allocation",
                );
                return Ok(false);
            }
            for slot in shape.ordered_indices() {
                let entry = &shape.entries()[slot];
                // Immediate indices are self-validating. Other atoms still go
                // through the table so large decimal Array indices are not
                // mistaken for named properties.
                let index = if let Some(index) = entry.atom.immediate_integer() {
                    Some(index)
                } else {
                    state.atoms.array_index(state.atoms.brand(entry.atom)?)?
                };
                if let Some(index) = index {
                    let index = index as usize;
                    if index >= length {
                        #[cfg(feature = "profiling")]
                        record_owned_execution_event(
                            "array_storage_dense_recovery_reject_out_of_range_index",
                        );
                        return Ok(false);
                    }
                    if entry.flags != PropertyFlags::data(true, true, true) {
                        #[cfg(feature = "profiling")]
                        record_owned_execution_event(
                            "array_storage_dense_recovery_reject_nondefault_descriptor",
                        );
                        return Ok(false);
                    }
                    if !matches!(data.slots[slot], PropertySlot::Data(_)) {
                        #[cfg(feature = "profiling")]
                        record_owned_execution_event(
                            "array_storage_dense_recovery_reject_nondata_slot",
                        );
                        return Ok(false);
                    }
                    if indexed_slots[index] != usize::MAX {
                        return Err(RuntimeError::Invariant(
                            "duplicate Array index during recovery",
                        ));
                    }
                    indexed_slots[index] = slot;
                } else {
                    // More names than this bound proves an index is missing;
                    // decline before push rather than allocate implicitly.
                    if entries.len() == named_count {
                        #[cfg(feature = "profiling")]
                        record_owned_execution_event(
                            "array_storage_dense_recovery_reject_missing_index",
                        );
                        return Ok(false);
                    }
                    entries.push(*entry);
                }
            }
            if indexed_slots.contains(&usize::MAX) {
                #[cfg(feature = "profiling")]
                record_owned_execution_event("array_storage_dense_recovery_reject_missing_index");
                return Ok(false);
            }
            (shape.prototype(), entries, indexed_slots)
        };
        // This borrow spans proof, shape preparation and publication. No JS,
        // reentry or release of an element can invalidate the selected slots.
        let shape = match state.get_or_create_shape(prototype, &entries) {
            Ok(shape) => shape,
            Err(RuntimeError::Heap(HeapError::Allocation { .. })) => {
                #[cfg(feature = "profiling")]
                record_owned_execution_event(
                    "array_storage_dense_recovery_reject_shape_allocation",
                );
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let moved = state
            .heap
            .recover_array_dense_shape(id, shape, &indexed_slots);
        let shape_cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(shape_cleanup)?;
        let Some(cleanup) = moved? else {
            // The heap API exposes this optional decline only as Ok(None).
            #[cfg(feature = "profiling")]
            record_owned_execution_event("array_storage_dense_recovery_heap_declined");
            return Ok(false);
        };
        state.apply_cleanup(cleanup)?;
        #[cfg(feature = "profiling")]
        record_owned_execution_event("array_storage_dense_recovery");
        Ok(true)
    }

    /// Remove configurable sparse indices in descending-delete semantics with
    /// one layout replacement. Return the highest blocking index, if any.
    ///
    /// Preparation is linear in layout width; publication retains the surviving
    /// slots before releasing the old layout. No per-index layout is published.
    pub(super) fn truncate_sparse_array_indices(
        &self,
        object: &ObjectRef,
        minimum: u32,
    ) -> Result<Option<u32>, RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let object_id = object.object_id();
        let (prototype, entries, slots, blocker) = {
            let data = state.heap.object(object_id)?;
            if !matches!(data.payload, ObjectPayload::Array { .. }) {
                return Err(RuntimeError::Invariant(
                    "sparse Array truncation reached a non-Array object",
                ));
            }
            let shape = state.heap.shape(data.shape)?;
            if shape.entries().len() != data.slots.len() {
                return Err(RuntimeError::Invariant(
                    "Array shape and value slots have different lengths",
                ));
            }
            let indices = shape
                .entries()
                .iter()
                .map(|entry| state.atoms.array_index(state.atoms.brand(entry.atom)?))
                .collect::<Result<Vec<_>, _>>()?;
            // Descending deletion stops at the highest non-configurable index.
            // Everything above it is removed; nothing at or below it is touched.
            let blocker = shape
                .entries()
                .iter()
                .zip(&indices)
                .filter_map(|(entry, index)| {
                    index.filter(|index| *index >= minimum && !entry.flags.configurable)
                })
                .max();
            let remove = |index: Option<u32>| {
                index.is_some_and(|index| {
                    index >= minimum && blocker.is_none_or(|blocked| index > blocked)
                })
            };
            let removed = indices.iter().filter(|&&index| remove(index)).count();
            if removed == 0 {
                return Ok(blocker);
            }
            let survivors = data.slots.len() - removed;
            let mut entries = Vec::with_capacity(survivors);
            let mut slots = Slots::new();
            for slot_index in shape.ordered_indices() {
                let entry = &shape.entries()[slot_index];
                let slot = &data.slots[slot_index];
                let index = indices[slot_index];
                if !remove(index) {
                    entries.push(*entry);
                    slots.push(slot.clone());
                }
            }
            (shape.prototype(), entries, slots, blocker)
        };
        state.replace_layout(object_id, prototype, &entries, slots)?;
        Ok(blocker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::{JsString, Value};

    #[test]
    fn dense_recovery_preserves_dictionary_key_order_and_named_descriptors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(array) = context.eval(r#"(() => {
            const a=[], s=Symbol('key');
            a.before=1; a[3]=3; a.middle=2; a[s]=8; a.after=4;
            delete a.before; a.before=5;
            Object.defineProperty(a, 'hidden', {get(){throw Error('getter called')}, configurable:true});
            a[2]=2; a[1]=1; a[0]=0;
            if (Reflect.ownKeys(a).map(String).join('|') !==
                '0|1|2|3|length|middle|after|before|hidden|Symbol(key)') throw Error('key order');
            if (a.join() !== '0,1,2,3' || a[s] !== 8 || a.before !== 5 ||
                Object.getOwnPropertyDescriptor(a,'hidden').enumerable !== false) throw Error('values');
            return a;
        })()"#).unwrap() else { panic!("array"); };
        assert_eq!(runtime.array_fast_len(&array).unwrap(), Some(4));
    }

    #[test]
    fn dense_recovery_define_property_keeps_readonly_length_and_default_indices() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(array) = context
            .eval(
                r#"(() => {
            const a=[]; a[3]=3; a[2]=2; a[1]=1;
            Object.defineProperty(a,'length',{writable:false});
            Object.defineProperty(a,'0',{value:-0,writable:true,enumerable:true,configurable:true});
            const d=Object.getOwnPropertyDescriptor(a,'0');
            if (!Object.is(d.value,-0) || !d.writable || !d.enumerable || !d.configurable ||
                Object.getOwnPropertyDescriptor(a,'length').writable || a.length !== 4 ||
                Reflect.set(a,'4',4)) throw Error('descriptors');
            a[1]=7;
            if(a[1] !== 7) throw Error('write');
            return a;
        })()"#,
            )
            .unwrap()
        else {
            panic!("array");
        };
        assert_eq!(runtime.array_fast_len(&array).unwrap(), Some(4));
    }

    #[test]
    fn dense_recovery_declines_holes_huge_lengths_and_nondefault_indices() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "const a=[];a[3]=3;a[0]=0;return a",
            "const a=[];a[4294967294]=3;a[0]=0;return a",
            "const a=[];a[2]=2;Object.defineProperty(a,'1',{get(){throw Error('getter called')},configurable:true});a[0]=0;return a",
            "const a=[];a[2]=2;a[1]=1;Object.defineProperty(a,'1',{writable:false});a[0]=0;return a",
            "const a=[];a[2]=2;a[1]=1;Object.defineProperty(a,'1',{enumerable:false});a[0]=0;return a",
            "const a=[];a[2]=2;a[1]=1;Object.defineProperty(a,'1',{configurable:false});a[0]=0;return a",
            "const a=[];a[3]=3;a.a=1;a.b=2;a.c=3;a[0]=0;return a",
        ] {
            let Value::Object(array) = context.eval(&format!("(()=>{{{source}}})()")).unwrap()
            else {
                panic!("array");
            };
            assert_eq!(runtime.array_fast_len(&array).unwrap(), None, "{source}");
        }
    }

    #[test]
    fn dense_recovery_obeys_prototypes_extensibility_and_invalidates_cached_layouts() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            const a=[]; a[2]=2; a[1]=1;
            const log=[], p=Object.create(Array.prototype);
            Object.defineProperty(p,'0',{set(v){log.push(v)}, configurable:true});
            Object.setPrototypeOf(a,p); a[0]=9;
            if (log.join() !== '9' || Object.hasOwn(a,'0')) return false;
            Object.defineProperty(p,'0',{value:4,writable:false,configurable:true});
            if(Reflect.set(a,'0',7) || Object.hasOwn(a,'0')) return false;
            delete p[0]; Object.preventExtensions(a);
            if(Reflect.set(a,'0',7) || Object.hasOwn(a,'0')) return false;
            const b=[]; b[2]=2; b[1]=1; b.named=6;
            const child=Object.create(b);
            function read(){return child[0]}
            function own(){return b[1]+b.named}
            for(let i=0;i<4;i++) { if(read()!==undefined || own()!==7) return false; }
            b[0]=8;
            if(read()!==8 || own()!==7) return false;
            delete b[1];
            if(Object.hasOwn(b,'1') || b.length!==3 || child[0]!==8) return false;
            return true;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn dense_recovery_moves_reference_and_atom_owners_and_collects_self_cycles() {
        let runtime = Runtime::new();
        let baseline_atoms = runtime.test_atom_count();
        {
            let mut context = runtime.new_context();
            let Value::Object(array) = context
                .eval(
                    r#"(() => {
                const a=[], s=Symbol('owned');
                a[5]=a; a[4]={answer:42}; a[3]='retained string';
                a[2]=123456789012345678901234567890n; a[1]=s; a[0]=s;
                return a;
            })()"#,
                )
                .unwrap()
            else {
                panic!("array");
            };
            assert_eq!(runtime.array_fast_len(&array).unwrap(), Some(6));
            runtime.run_gc().unwrap();
            let key = runtime.property_key_for_index(4).unwrap();
            assert!(matches!(
                context.get_property(&array, &key).unwrap(),
                Value::Object(_)
            ));
            let zero = runtime.property_key_for_index(0).unwrap();
            let one = runtime.property_key_for_index(1).unwrap();
            assert_eq!(
                context.get_property(&array, &zero).unwrap(),
                context.get_property(&array, &one).unwrap()
            );
        }
        runtime.run_gc().unwrap();
        assert_eq!(runtime.heap_counts().live, 0);
        assert_eq!(runtime.test_atom_count(), baseline_atoms);
    }

    #[test]
    fn sparse_truncation_preserves_dictionary_insertion_order_after_deletion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            const a=[], s=Symbol('key');
            a.first=1; a[10]=10; a.second=2; a[s]=3; a.third=4;
            delete a.first; a.first=5;
            a.length=3;
            return Reflect.ownKeys(a).map(String).join('|');
        })()"#
                )
                .unwrap(),
            Value::String(JsString::from_static(
                "length|second|third|first|Symbol(key)"
            ))
        );
    }

    #[test]
    fn sparse_truncation_preserves_highest_blocker_named_order_and_accessor_silence() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let result = context.eval(r#"(() => {
        const a = [];
        a.first = 1;
        a[200] = 200;
        Object.defineProperty(a, '120', { get() { throw Error('getter called'); }, configurable: true });
        Object.defineProperty(a, '90', { value: 90, configurable: false });
        Object.defineProperty(a, '40', { value: 40, configurable: false });
        a[10] = 10;
        a.last = 2;
        const ok = Reflect.defineProperty(a, 'length', { value: 5, writable: false });
        return [ok, a.length, Object.getOwnPropertyDescriptor(a, 'length').writable,
            Reflect.ownKeys(a).join(','), a[90], a[40]].join('|');
    })()"#).unwrap();
        assert_eq!(
            result,
            Value::String(JsString::from_static(
                "false|91|false|10,40,90,length,first,last|90|40"
            ))
        );
    }
}
