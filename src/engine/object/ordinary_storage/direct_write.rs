//! Exchange existing owners under one state access; no pending representation.
use super::*;
use crate::engine::object::append_ic::{AppendKind, AppendMiss, PropertyAppendCacheTable};

/// A store site's append table and execution PC, resolved only on a miss.
pub(crate) type AppendSite<'a> = (&'a PropertyAppendCacheTable, usize);

/// A named store publishes either an existing slot or a new layout. Only the
/// latter can allocate cycle-collectable shapes and needs a publication safe
/// point; ordinary replacements do not poll GC.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldStore {
    Miss,
    Existing,
    LayoutPublished,
}
impl FieldStore {
    #[cfg(test)]
    pub(crate) fn committed(self) -> bool {
        self != Self::Miss
    }
}

impl RuntimeState {
    /// Resolve a static key owned by the current published executable, then
    /// consume the frame's existing value owner through the ordinary selector.
    /// For a missing key, the store site's append fact replaces the prototype
    /// walk and successor selection; a miss teaches the site after publication.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_store_owned_linked_field(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
        appends: Option<AppendSite<'_>>,
    ) -> Result<FieldStore, RuntimeError> {
        let Some(atom) = super::linked_field_atom_in_domain(domain, executable, key) else {
            return Ok(FieldStore::Miss);
        };
        self.try_store_owned_own_data(poisoned, domain, object, atom, input, appends)
    }

    /// A missing ordinary property consumes the same canonical append policy
    /// as descriptors. Setter, exotic and shared-dictionary cases decline with
    /// the input untouched; no continuation is constructed on local success.
    #[inline]
    fn try_store_owned_own_data(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        atom: Atom,
        input: &mut JsValue,
        appends: Option<AppendSite<'_>>,
    ) -> Result<FieldStore, RuntimeError> {
        // A learned writable own data slot needs no own-key selection.
        if let Some((table, pc)) = appends
            && table.has_fact(AppendKind::Existing, pc)
            && self.try_existing_site_store(domain, object, input, table, pc)?
        {
            return Ok(FieldStore::Existing);
        }
        // A learned parent layout proves the key missing on this receiver and
        // the prototype walk's result, before any own-key selection.
        if let Some((table, pc)) = appends
            && table.has_fact(AppendKind::Store, pc)
            && self.try_site_append(poisoned, domain, object, input, table, pc)?
        {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_owned_field_append_site_hit",
            );
            return Ok(FieldStore::LayoutPublished);
        }
        let prototype = match select_set_slot(self, object, atom)? {
            BorrowedSet::Missing(prototype) => prototype,
            BorrowedSet::Data(selected) if selected.flags.writable => {
                if !self
                    .heap
                    .exchange_owned_data_slot(object, selected.index, input)?
                {
                    return Ok(FieldStore::Miss);
                }
                if let Some((table, pc)) = appends {
                    self.learn_existing_site(domain, object, table, pc, selected.index);
                }
                return Ok(FieldStore::Existing);
            }
            BorrowedSet::Data(_) | BorrowedSet::Setter(_) | BorrowedSet::Special(_) => {
                return Ok(FieldStore::Miss);
            }
        };
        self.append_missing_owned_data(poisoned, domain, object, atom, prototype, input, appends)
    }

    /// Out of line so the interpreter loop that inlines the store entry does
    /// not grow; a hit costs one call instead of the own-key selection.
    #[inline(never)]
    fn try_existing_site_store(
        &mut self,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        table: &PropertyAppendCacheTable,
        pc: usize,
    ) -> Result<bool, RuntimeError> {
        let Some(site) = table.site(pc) else {
            return Ok(false);
        };
        // The receiver operand owns `object` throughout the store.
        let Some(slot) = site.existing_slot(&self.heap, domain, self.heap.object_fast(object))
        else {
            site.existing_missed();
            return Ok(false);
        };
        Ok(self
            .heap
            .exchange_owned_data_slot_fast(object, slot, input)?)
    }

    #[cold]
    #[inline(never)]
    fn learn_existing_site(
        &self,
        domain: u64,
        object: ObjectId,
        table: &PropertyAppendCacheTable,
        pc: usize,
        slot: usize,
    ) {
        if let Some(site) = table.site(pc)
            && site.should_learn_existing()
            && let Ok(receiver) = self.heap.object(object)
            && site.learn_existing(&self.heap, domain, receiver, slot)
        {
            table.mark(AppendKind::Existing, pc);
        }
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn append_missing_owned_data(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        atom: Atom,
        prototype: Option<ObjectId>,
        input: &mut JsValue,
        appends: Option<AppendSite<'_>>,
    ) -> Result<FieldStore, RuntimeError> {
        // Observe the epoch before the walk whose result the site may keep.
        let prototype_epoch = self.heap.property_layout_epoch();
        if !matches!(
            set_missing_local(self, object, atom, prototype)?,
            MissingSelection::Define
        ) {
            return Ok(FieldStore::Miss);
        }
        let parent = self.heap.object(object)?.shape;
        let parent_shape = self.heap.shape(parent)?;
        // Indexed keys can be decided by dense Array prototypes, whose
        // elements change without a layout change; they stay uncached.
        let learn = appends.is_some()
            && !parent_shape.is_dictionary()
            && is_ordinary(self.heap.object(object)?)
            && self.atoms.array_index(atom)?.is_none();
        let miss = AppendMiss {
            domain,
            parent,
            parent_revision: parent_shape.layout_revision(),
            has_prototype: prototype.is_some(),
            prototype_epoch,
        };
        if !self.append_selected_missing_slot(
            Some(poisoned),
            object,
            atom,
            crate::engine::object::shape::PropertyFlags::data(true, true, true),
            crate::engine::object::SlotAppendInput::Owned(input),
        )? {
            return Ok(FieldStore::Miss);
        }
        if learn && let Some((table, pc)) = appends {
            table.learn(
                AppendKind::Store,
                pc,
                &self.heap,
                miss,
                self.heap.object(object)?.shape,
            );
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_owned_field_append_in_state",
        );
        Ok(FieldStore::LayoutPublished)
    }

    /// The atom of a computed key that needs no user conversion: a canonical
    /// array index (an Int or integral Float), a String or a Symbol. The flag
    /// says the caller owns an interned String atom and must release it.
    /// Other primitives (negative or fractional numbers, booleans, null,
    /// undefined, BigInt) keep the general key conversion.
    pub(crate) fn primitive_key_atom(
        &mut self,
        key: &JsValue,
    ) -> Result<Option<(Atom, bool)>, RuntimeError> {
        let index = match key {
            JsValue::Int(value) => u32::try_from(*value).ok(),
            JsValue::Float(value)
                if *value >= 0.0 && *value <= f64::from(u32::MAX) && value.fract() == 0.0 =>
            {
                Some(*value as u32)
            }
            JsValue::Symbol(index) => return Ok(Some((self.atoms.brand(*index)?, false))),
            JsValue::String(id) => {
                return Ok(Some((self.intern_property_key_string_id(*id)?, true)));
            }
            _ => None,
        };
        Ok(index
            .and_then(Atom::from_immediate_integer)
            .map(|atom| (atom, false)))
    }

    /// Store an owned value under a resolved computed key without building a
    /// Set state: dense Array elements first, then the ordinary own-data and
    /// append kernel shared with static stores. Every other case (setters,
    /// read-only or non-extensible rejection, exotic receivers) declines with
    /// the input untouched for the general Set.
    pub(crate) fn try_store_owned_atom_key(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        atom: Atom,
        input: &mut JsValue,
    ) -> Result<FieldStore, RuntimeError> {
        if let Some(index) = atom.immediate_integer()
            && (self.try_exchange_dense_value(object, index, input)?
                || self.try_append_dense_value(object, index, input)?)
        {
            return Ok(FieldStore::Existing);
        }
        self.try_store_owned_own_data(poisoned, domain, object, atom, input, None)
    }

    /// CreateDataPropertyOrThrow for a static literal or class-field key.
    /// Definition never consults the prototype chain: a missing key on an
    /// extensible plain ordinary receiver appends, and an existing all-true
    /// data property exchanges its value. Every other case declines with the
    /// input untouched so the definition driver reports or redefines it.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_define_owned_linked_field(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
        appends: Option<AppendSite<'_>>,
    ) -> Result<FieldStore, RuntimeError> {
        let Some(atom) = super::linked_field_atom_in_domain(domain, executable, key) else {
            return Ok(FieldStore::Miss);
        };
        if let Some((table, pc)) = appends
            && table.has_fact(AppendKind::Definition, pc)
            && self.try_site_append(poisoned, domain, object, input, table, pc)?
        {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_owned_field_define_site_hit",
            );
            return Ok(FieldStore::LayoutPublished);
        }
        let data = self.heap.object(object)?;
        if !is_ordinary(data) {
            return Ok(FieldStore::Miss);
        }
        if let Some(selected) = locate(self, object, atom)? {
            if selected.flags != crate::engine::object::shape::PropertyFlags::data(true, true, true)
                || !matches!(data.slots[selected.index], PropertySlot::Data(_))
            {
                return Ok(FieldStore::Miss);
            }
            return Ok(
                if self
                    .heap
                    .exchange_owned_data_slot(object, selected.index, input)?
                {
                    FieldStore::Existing
                } else {
                    FieldStore::Miss
                },
            );
        }
        self.define_missing_owned_data(poisoned, domain, object, atom, input, appends)
    }

    #[inline(never)]
    fn define_missing_owned_data(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        atom: Atom,
        input: &mut JsValue,
        appends: Option<AppendSite<'_>>,
    ) -> Result<FieldStore, RuntimeError> {
        let data = self.heap.object(object)?;
        if !data.extensible {
            return Ok(FieldStore::Miss);
        }
        let parent = data.shape;
        let parent_shape = self.heap.shape(parent)?;
        let learn = appends.is_some() && !parent_shape.is_dictionary();
        let miss = AppendMiss {
            domain,
            parent,
            parent_revision: parent_shape.layout_revision(),
            has_prototype: false,
            prototype_epoch: 0,
        };
        if !self.append_selected_missing_slot(
            Some(poisoned),
            object,
            atom,
            crate::engine::object::shape::PropertyFlags::data(true, true, true),
            crate::engine::object::SlotAppendInput::Owned(input),
        )? {
            return Ok(FieldStore::Miss);
        }
        if learn && let Some((table, pc)) = appends {
            table.learn(
                AppendKind::Definition,
                pc,
                &self.heap,
                miss,
                self.heap.object(object)?.shape,
            );
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_owned_field_define_in_state",
        );
        Ok(FieldStore::LayoutPublished)
    }

    /// The site's recorded parent layout is the receiver's current shape: the
    /// key is missing on this plain ordinary receiver and, for stores, the
    /// prototype chain still selects definition on it. Publish the recorded
    /// successor. Returns false, consuming nothing, when the fact fails.
    #[inline(never)]
    fn try_site_append(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        table: &PropertyAppendCacheTable,
        pc: usize,
    ) -> Result<bool, RuntimeError> {
        let Some(site) = table.site(pc) else {
            return Ok(false);
        };
        let data = self.heap.object_fast(object);
        if !is_ordinary(data) {
            return Ok(false);
        }
        let Some(successor) = site.successor(&self.heap, domain, data) else {
            return Ok(false);
        };
        self.append_cached_owned_slot(poisoned, object, successor, input)?;
        Ok(true)
    }

    /// Consume a site hit. The input's owner moves into the slot only on
    /// publication; a rejected append leaves it and both shapes unchanged.
    /// Store and definition hits each inline it into their miss consumers.
    #[inline(always)]
    fn append_cached_owned_slot(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        object: ObjectId,
        successor: crate::engine::heap::ShapeId,
        input: &mut JsValue,
    ) -> Result<(), RuntimeError> {
        match self
            .heap
            .append_cached_owned_slot(object, successor, input.as_raw())
        {
            Ok(cleanup) => {
                *input = JsValue::Undefined;
                match cleanup {
                    Some(cleanup) => self
                        .apply_cleanup(cleanup)
                        .inspect_err(|_| poisoned.set(true)),
                    None => Ok(()),
                }
            }
            Err(failure) => {
                if failure.published {
                    // Quarantine before cleanup reaches an inconsistent heap.
                    *input = JsValue::Undefined;
                    poisoned.set(true);
                }
                Err(failure.error.into())
            }
        }
    }

    pub(crate) fn try_exchange_dense_value(
        &mut self,
        object: ObjectId,
        index: u32,
        input: &mut JsValue,
    ) -> Result<bool, RuntimeError> {
        Ok(self.heap.exchange_owned_dense_value(object, index, input)?)
    }

    /// Append only the next dense index. The existing shared prototype walk
    /// and length selector establish the complete callback-free Set case.
    pub(crate) fn try_append_dense_value(
        &mut self,
        object: ObjectId,
        index: u32,
        input: &mut JsValue,
    ) -> Result<bool, RuntimeError> {
        let data = self.heap.object(object)?;
        if !data.extensible || !matches!(data.kind, ObjectKind::Array) {
            return Ok(false);
        }
        // The next element after the dense prefix either grows the length
        // (writable length equal to the prefix) or fills a hole below an
        // existing length, as in `new Array(n)` filled in order.
        let fills_hole = match &data.payload {
            ObjectPayload::Array { dense: Some(dense) } if index as usize == dense.len() => {
                crate::engine::object::dense_mutation::dense_hole_below_length(data, index)
            }
            _ => return Ok(false),
        };
        if !fills_hole
            && crate::engine::object::dense_mutation::writable_dense_length(self, data)?
                != Some(index)
        {
            return Ok(false);
        }
        let Some(atom) = Atom::from_immediate_integer(index) else {
            return Ok(false);
        };
        let prototype = self.heap.shape(data.shape)?.prototype();
        if !prototypes_allow_dense_append(self, atom, prototype)? {
            return Ok(false);
        }
        let value = std::mem::replace(input, JsValue::Undefined);
        let appended = if fills_hole {
            self.heap
                .fill_array_dense_hole_owned(object, value.into_raw())
        } else {
            self.heap
                .append_fresh_array_dense_value_owned(object, value.into_raw())
        };
        match appended {
            Ok(()) => Ok(true),
            Err((error, value)) => {
                *input = JsValue::from_raw(value).expect("rejected public array input");
                Err(error.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;

    fn object(value: &JsValue) -> ObjectId {
        let JsValue::Object(id) = value else {
            panic!("object")
        };
        *id
    }

    #[test]
    fn computed_key_stores_match_ordinary_set_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                const out = [];
                const sym = Symbol('s');
                const o = {};
                const keys = ['a', 'b', '3', sym, 7, 2.0];
                for (let round = 0; round < 3; round++)
                    for (const k of keys) o[k] = round;
                out.push(o.a === 2 && o.b === 2 && o[3] === 2 && o[sym] === 2 && o[7] === 2
                    && o[2] === 2 && Object.keys(o).join() === '2,3,7,a,b');
                // Negative and fractional numbers become string keys.
                const n = {};
                n[-1] = 'm'; n[1.5] = 'f'; n[-0] = 'z';
                out.push(n['-1'] === 'm' && n['1.5'] === 'f' && n['0'] === 'z');
                // Integral Float keys reach dense elements.
                const a = [0, 0, 0];
                a[1.0] = 'x'; a[3.0] = 'y'; a['2'] = 'w';
                out.push(a.join() === '0,x,w,y' && a.length === 4);
                // Prototype setters and read-only properties still decide.
                let seen = '';
                const proto = {set k(v) { seen += v; }};
                Object.defineProperty(proto, 'r', {value: 1, writable: false});
                const child = Object.create(proto);
                const kk = 'k', rr = 'r';
                child[kk] = 's'; child[rr] = 2;
                out.push(seen === 's' && !child.hasOwnProperty('k') && child.r === 1);
                let threw = false;
                try { (function() { 'use strict'; child[rr] = 3; })(); }
                catch (e) { threw = e instanceof TypeError; }
                out.push(threw);
                const frozen = Object.freeze({x: 1}), xk = 'x';
                threw = false;
                try { (function() { 'use strict'; frozen[xk] = 2; })(); }
                catch (e) { threw = e instanceof TypeError; }
                out.push(threw && frozen.x === 1);
                // Proxy traps, __proto__, Array length and the global object.
                let trapped = '';
                const p = new Proxy({}, {set(t, k, v) { trapped += k; t[k] = v; return true; }});
                p['q'] = 1;
                const pk = '__proto__', target = {};
                const withProto = {};
                withProto[pk] = target;
                const arr = [1, 2, 3], lk = 'length';
                arr[lk] = 1;
                globalThis['computedGlobal'] = 5;
                out.push(trapped === 'q' && Object.getPrototypeOf(withProto) === target
                    && arr.length === 1 && computedGlobal === 5);
                const result = out.join();
                return result === 'true,true,true,true,true,true,true' || result;
            })()
        "#
                )
                .unwrap(),
            crate::engine::value::Value::Bool(true)
        );
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn computed_string_key_stores_release_their_interned_atoms() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let count = |runtime: &Runtime| runtime.0.state.borrow().atoms.len();
        drop(
            context
                .eval("globalThis.o = {}; globalThis.store = (k) => { o[k] = 1; delete o[k]; };")
                .unwrap(),
        );
        drop(context.eval("store('warm' + 1)").unwrap());
        runtime.run_gc().unwrap();
        let before = count(&runtime);
        drop(
            context
                .eval("for (let i = 0; i < 1000; i++) store('fresh-key-' + i);")
                .unwrap(),
        );
        runtime.run_gc().unwrap();
        assert!(
            count(&runtime) <= before + 2,
            "{} -> {}",
            before,
            count(&runtime)
        );
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn dense_hole_fill_moves_owner_and_keeps_length() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let array = runtime
            .into_jsvalue(context.eval("new Array(3)").unwrap())
            .unwrap();
        let marker = runtime
            .into_jsvalue(context.eval("({marker:1})").unwrap())
            .unwrap();
        let count = |runtime: &Runtime| {
            let state = runtime.0.state.borrow();
            state.heap.object_strong_count(object(&marker)).unwrap()
        };
        let before = count(&runtime);
        let mut input = runtime.dup_jsvalue(&marker).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        // Only the first hole after the dense prefix is filled in place.
        assert!(
            !state
                .try_append_dense_value(object(&array), 1, &mut JsValue::Int(9))
                .unwrap()
        );
        assert!(
            state
                .try_append_dense_value(object(&array), 0, &mut input)
                .unwrap()
        );
        assert!(matches!(input, JsValue::Undefined));
        drop(state);
        // The owner moved in: the dup above is now the element's reference.
        assert_eq!(count(&runtime), before + 1);
        runtime.release_jsvalue(array).unwrap();
        assert_eq!(count(&runtime), before);
        runtime.release_jsvalue(marker).unwrap();
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn dense_hole_fill_matches_ordinary_set_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                const out = [];
                // In-order fill keeps the length and reads back every value.
                const a = new Array(4);
                for (let i = 0; i < 4; i++) a[i] = i * 0.5;
                out.push(a.length === 4 && a.join() === '0,0.5,1,1.5');
                // Out-of-order and past-the-length writes stay correct.
                const b = new Array(4);
                b[2] = 'c'; b[0] = 'a'; b[1] = 'b'; b[5] = 'f';
                out.push(b.length === 6 && b.join() === 'a,b,c,,,f' && !(3 in b));
                // A read-only length still admits holes below it.
                const c = new Array(2);
                Object.defineProperty(c, 'length', {writable: false});
                c[0] = 1; c[1] = 2; c[2] = 3;
                out.push(c.length === 2 && c[0] === 1 && c[1] === 2 && !(2 in c));
                // Non-extensible arrays reject the fill.
                const d = Object.preventExtensions(new Array(2));
                d[0] = 1;
                out.push(!(0 in d));
                // Prototype setters and read-only elements decide the hole.
                let seen = '';
                Object.defineProperty(Array.prototype, 0, {
                    set(v) { seen += v; }, configurable: true,
                });
                const e = new Array(2);
                e[0] = 'x';
                out.push(seen === 'x' && !Object.prototype.hasOwnProperty.call(e, 0));
                delete Array.prototype[0];
                Object.defineProperty(Object.prototype, 0, {value: 'p', writable: false, configurable: true});
                const f = new Array(2);
                f[0] = 'y';
                out.push(f[0] === 'p' && !Object.prototype.hasOwnProperty.call(f, 0));
                delete Object.prototype[0];
                const g = new Array(2);
                g[0] = 'z';
                out.push(g[0] === 'z' && Object.prototype.hasOwnProperty.call(g, 0));
                const result = out.join();
                return result === 'true,true,true,true,true,true,true' || result;
            })()
        "#
                )
                .unwrap(),
            crate::engine::value::Value::Bool(true)
        );
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn owned_append_transfers_heap_and_atom_edges_and_reuses_canonical_shapes() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        for source in [
            "({marker:1})",
            "'owned string'",
            "2n**90n",
            "Symbol('owned')",
        ] {
            let first = runtime.into_jsvalue(context.eval("({})").unwrap()).unwrap();
            let second = runtime.into_jsvalue(context.eval("({})").unwrap()).unwrap();
            let mut input = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let mut state = runtime.0.state.borrow_mut();
            let raw = input.as_raw();
            let before = match raw {
                RawValue::Object(id) => Some(state.heap.object_strong_count(id).unwrap()),
                RawValue::Symbol(id) => {
                    state
                        .atoms
                        .resolve(state.atoms.brand(id).unwrap())
                        .unwrap()
                        .ref_count
                }
                _ => None,
            };
            assert!(matches!(
                state
                    .try_store_owned_own_data(
                        &runtime.0.poisoned,
                        runtime.domain_id(),
                        object(&first),
                        key.atom(),
                        &mut input,
                        None
                    )
                    .unwrap(),
                FieldStore::LayoutPublished,
            ));
            assert!(matches!(input, JsValue::Undefined));
            // This is the publication point consumed by the VM, including GC
            // with the receiver/value protected only by their actual owners.
            runtime.0.gc_pressure.remaining.set(0);
            state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .unwrap();
            assert!(runtime.0.gc_pressure.remaining.get() > 0);
            let first_shape = state.heap.object(object(&first)).unwrap().shape;
            assert!(
                matches!(&state.heap.object(object(&first)).unwrap().slots[0], PropertySlot::Data(value) if JsValue::from_raw(value.clone()) == JsValue::from_raw(raw.clone()))
            );
            match raw {
                RawValue::Object(id) => {
                    assert_eq!(Some(state.heap.object_strong_count(id).unwrap()), before)
                }
                RawValue::Symbol(id) => assert_eq!(
                    state
                        .atoms
                        .resolve(state.atoms.brand(id).unwrap())
                        .unwrap()
                        .ref_count,
                    before
                ),
                _ => {}
            }
            let mut other = JsValue::Null;
            assert!(
                state
                    .try_store_owned_own_data(
                        &runtime.0.poisoned,
                        runtime.domain_id(),
                        object(&second),
                        key.atom(),
                        &mut other,
                        None
                    )
                    .unwrap()
                    .committed()
            );
            assert_eq!(
                state.heap.object(object(&second)).unwrap().shape,
                first_shape
            );
            state
                .release_owned_jsvalue(&runtime.0.poisoned, first)
                .unwrap();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, second)
                .unwrap();
            if let RawValue::Object(id) = raw {
                assert!(state.heap.object(id).is_err());
            }
        }
    }

    #[test]
    fn owned_append_declines_observable_prototypes_before_consuming_input() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        for source in [
            "Object.preventExtensions({})",
            "Object.create({set x(v){throw v}})",
            "Object.create(Object.freeze({x:1}))",
            "Object.create(new Proxy({}, {getOwnPropertyDescriptor(){throw 7}}))",
        ] {
            let receiver = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let mut input = runtime.into_jsvalue(context.eval("({})").unwrap()).unwrap();
            let raw = input.as_raw();
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                !state
                    .try_store_owned_own_data(
                        &runtime.0.poisoned,
                        runtime.domain_id(),
                        object(&receiver),
                        key.atom(),
                        &mut input,
                        None
                    )
                    .unwrap()
                    .committed(),
                "{source}"
            );
            assert_eq!(JsValue::from_raw(input.as_raw()), JsValue::from_raw(raw));
            assert!(
                state
                    .heap
                    .shape(state.heap.object(object(&receiver)).unwrap().shape)
                    .unwrap()
                    .find(crate::engine::atom::AtomIdx::from_raw(key.atom().raw()))
                    .is_none()
            );
            state
                .release_owned_jsvalue(&runtime.0.poisoned, input)
                .unwrap();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, receiver)
                .unwrap();
        }
    }

    #[test]
    fn rejected_owned_successor_restores_the_input_and_shape_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let receiver = runtime
            .into_jsvalue(context.eval("({x:1})").unwrap())
            .unwrap();
        let mut input = runtime
            .into_jsvalue(context.eval("Symbol('rollback')").unwrap())
            .unwrap();
        let raw = input.as_raw();
        let mut state = runtime.0.state.borrow_mut();
        let shape = state.heap.object(object(&receiver)).unwrap().shape;
        let count = state.heap.shape_strong_count(shape).unwrap();
        state.heap.retain_shape(shape).unwrap();
        assert!(
            state
                .append_slot_with_owned_shape_input(
                    None,
                    object(&receiver),
                    shape,
                    crate::engine::object::SlotAppendInput::Owned(&mut input)
                )
                .is_err()
        );
        assert_eq!(JsValue::from_raw(input.as_raw()), JsValue::from_raw(raw));
        assert_eq!(state.heap.shape_strong_count(shape).unwrap(), count);
        assert_eq!(state.heap.object(object(&receiver)).unwrap().slots.len(), 1);
        state
            .release_owned_jsvalue(&runtime.0.poisoned, input)
            .unwrap();
        state
            .release_owned_jsvalue(&runtime.0.poisoned, receiver)
            .unwrap();
    }

    #[test]
    fn published_owned_append_consumes_the_atom_edge_before_poison_cleanup() {
        use crate::engine::heap::RawId;

        let runtime = Runtime::new();
        let receiver = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("published").unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let previous = state.heap.object(receiver.object_id()).unwrap().shape;
        let successor = state
            .append_transition(
                previous,
                crate::engine::object::shape::ShapeEntry {
                    atom: crate::engine::atom::AtomIdx::from_raw(key.atom().raw()),
                    flags: crate::engine::object::shape::PropertyFlags::data(true, true, true),
                },
            )
            .unwrap();
        let atom = state.atoms.new_symbol(Some("transferred")).unwrap();
        let index = state.atoms.unbrand(atom).unwrap();
        let mut input = JsValue::Symbol(index);
        // The publication itself is valid. Retiring the old shape is the
        // failing operation, after the slot acquired this exact atom owner.
        state
            .heap
            .set_strong_count_for_test(RawId::Shape(previous), 0);
        assert!(
            state
                .append_slot_with_owned_shape_input(
                    Some(&runtime.0.poisoned),
                    receiver.object_id(),
                    successor,
                    crate::engine::object::SlotAppendInput::Owned(&mut input),
                )
                .is_err()
        );
        assert!(runtime.is_poisoned());
        assert!(matches!(input, JsValue::Undefined));
        assert!(matches!(
            state.heap.object(receiver.object_id()).unwrap().slots[0],
            PropertySlot::Data(RawValue::Symbol(stored)) if stored == index
        ));
        assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
        // Public roots abandon cleanup in a poisoned runtime; teardown must
        // neither release the transferred atom twice nor attempt recovery.
        drop(state);
        drop(receiver);
        drop(key);
        drop(_unwind);
        drop(runtime);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn constructors_append_in_state_and_keep_getter_and_proxy_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval(r#"(() => {
            function C(v) {this.a=v;this.b=this;this.c=Symbol('field');}
            const value={};const first=new C(value);const second=new C(value);
            if(first.a!==value || first.b!==first || second.b!==second || typeof first.c!=='symbol')return false;
            let calls=0;const proto={set x(v){calls++;if(v!==value)throw 7}};
            const o=Object.create(proto);o.x=value;
            const p=new Proxy({}, {set(t,k,v,r){calls++;return Reflect.set(t,k,v,r)}});p.x=value;
            return calls===2 && !Object.hasOwn(o,'x') && p.x===value;
        })()"#).unwrap(), Value::Bool(true));
        // The second construction may consume its sites' append facts; both
        // paths complete in State.
        let events = profile.snapshot().owned_execution_events;
        let count = |name| events.get(name).copied().unwrap_or(0);
        assert!(
            count("ordinary_owned_field_append_in_state")
                + count("ordinary_owned_field_append_site_hit")
                >= 6
        );
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn vm_field_exchange_preserves_aliases_all_value_kinds_and_throw_recovery() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            let o = {x: {previous:true}};
            let other = {marker:true};
            let values = [undefined,null,false,0,-0,1.5,3n,2n**90n,
                          'new string',Symbol('owner'),other,o];
            for (let v of values) {
                if (!Object.is(o.x = v, v) || !Object.is(o.x,v)) return false;
            }
            o.x = o; o.x = o.x; if (o.x !== o) return false;
            other.x = o; o.x = other; if (o.x.x !== o) return false;
            o.x = null; other.x = null;
            let calls=0;
            let setter={set x(v){calls++;throw v}};
            try {setter.x=other;return false} catch(e){if(e!==other)return false}
            o.x=other; o.x=null;
            const frozen=Object.freeze({x:other});
            try {(function(){'use strict';frozen.x=o})();return false}
            catch(e){if(!(e instanceof TypeError))return false}
            return calls===1 && frozen.x===other;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn heap_field_values_complete_in_the_same_execute_segment() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let _ = context
            .eval("globalThis.exchangeTarget={x:null};globalThis.exchangeInput={};")
            .unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval("for(let i=0;i<16;i++)exchangeTarget.x=exchangeInput;exchangeTarget.x===exchangeInput").unwrap(), Value::Bool(true));
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("ordinary_owned_field_write_in_execute")
                .copied(),
            Some(16)
        );
    }

    #[test]
    fn vm_dense_writes_keep_append_replacement_and_observable_fallbacks() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"(() => {
            const a=[]; const o={};
            const values=[undefined,null,false,-0,1.5,3n,2n**90n,'owner',Symbol('v'),o,a];
            for(let i=0;i<values.length;i++)
                if(!Object.is(a[i]=values[i],values[i]))return false;
            for(let i=0;i<values.length;i++)
                if(!Object.is(a[i]=values[values.length-i-1],values[values.length-i-1]))return false;
            a[0]=a; a[0]=a[0]; if(a[0]!==a)return false;
            a[0]=o; a[0]=null;
            const fixed=[o]; Object.defineProperty(fixed,'length',{writable:false});
            fixed[0]=a; if(fixed[0]!==a)return false;
            try{(function(){'use strict';fixed[1]=o})();return false}
            catch(e){if(!(e instanceof TypeError))return false}
            const sealed=Object.seal([o]); sealed[0]=a;
            if(sealed[0]!==a)return false;
            const frozen=Object.freeze([o]); frozen[0]=a;
            if(frozen[0]!==o)return false;
            let calls=0; const proto={set 0(v){calls++;if(v!==o)throw 7}};
            const special=[]; Object.setPrototypeOf(special,proto); special[0]=o;
            if(calls!==1 || special.length!==0 || Object.hasOwn(special,'0'))return false;
            const hole=[]; hole[2]=o; if(hole.length!==3 || 0 in hole || hole[2]!==o)return false;
            const p=new Proxy(a,{set(t,k,v,r){calls++;return Reflect.set(t,k,v,r)}});
            p[0]=o; if(calls!==2 || a[0]!==o)return false;
            const t=new Float64Array(2); t[0]=2.5; t[1]={valueOf(){calls++;return 3.5}};
            return t[0]===2.5 && t[1]===3.5 && calls===3;
        })()"#).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn heap_dense_append_and_replace_complete_without_a_write_request() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let _ = context
            .eval("globalThis.directDense=[];globalThis.denseInput={};")
            .unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval("for(let i=0;i<16;i++)directDense[i]=denseInput;for(let i=0;i<16;i++)directDense[i]=null;directDense.length").unwrap(), Value::Int(16));
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("dense_array_owned_write_in_execute")
                .copied(),
            Some(32)
        );
    }

    #[test]
    fn exchange_moves_heap_edges_and_retires_old_value_after_commit() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let base = runtime
            .into_jsvalue(context.eval("({x:{old:true}})").unwrap())
            .unwrap();
        let mut input = runtime
            .into_jsvalue(context.eval("({fresh:true})").unwrap())
            .unwrap();
        let fresh = object(&input);
        let key = runtime.intern_property_key("x").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(state.heap.object_strong_count(fresh), Ok(1));
        assert!(
            state
                .try_store_owned_own_data(
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                    object(&base),
                    key.atom(),
                    &mut input,
                    None
                )
                .unwrap()
                .committed()
        );
        let previous = object(&input);
        assert_eq!(state.heap.object_strong_count(fresh), Ok(1));
        assert_eq!(state.heap.object_strong_count(previous), Ok(1));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, input)
            .unwrap();
        assert!(state.heap.object(previous).is_err());
        state
            .release_owned_jsvalue(&runtime.0.poisoned, base)
            .unwrap();
        assert!(state.heap.object(fresh).is_err());
    }

    #[test]
    fn rejected_selection_preserves_both_owners_and_never_invokes_getter() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        for source in [
            "Object.freeze({x:1})",
            "({get x(){throw 1}})",
            "new Proxy({x:1}, {set(){throw 2}})",
        ] {
            let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let mut input = runtime
                .into_jsvalue(context.eval("({fresh:true})").unwrap())
                .unwrap();
            let fresh = object(&input);
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                !state
                    .try_store_owned_own_data(
                        &runtime.0.poisoned,
                        runtime.domain_id(),
                        object(&base),
                        key.atom(),
                        &mut input,
                        None
                    )
                    .unwrap()
                    .committed()
            );
            assert_eq!(object(&input), fresh);
            assert_eq!(state.heap.object_strong_count(fresh), Ok(1));
            state
                .release_owned_jsvalue(&runtime.0.poisoned, input)
                .unwrap();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, base)
                .unwrap();
        }
    }

    #[test]
    fn dense_replace_and_append_move_every_public_value_kind() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        for source in [
            "undefined",
            "null",
            "true",
            "-0",
            "42",
            "1.5",
            "1n",
            "(1n<<100n)",
            "'owned string'",
            "Symbol('owned atom')",
            "({})",
        ] {
            let array = runtime.into_jsvalue(context.eval("[1]").unwrap()).unwrap();
            let mut input = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                state
                    .try_append_dense_value(object(&array), 1, &mut input)
                    .unwrap()
            );
            assert!(matches!(input, JsValue::Undefined));
            let mut replacement = JsValue::Int(7);
            assert!(
                state
                    .try_exchange_dense_value(object(&array), 1, &mut replacement)
                    .unwrap()
            );
            state
                .release_owned_jsvalue(&runtime.0.poisoned, replacement)
                .unwrap();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, array)
                .unwrap();
        }
    }

    #[test]
    fn dense_append_declines_holes_fixed_length_and_observable_prototypes() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        for source in [
            "Object.freeze([])",
            "Object.seal([])",
            "Object.defineProperty([], 'length', {writable:false})",
            "Object.setPrototypeOf([], {set 0(v){throw 1}})",
            "Object.setPrototypeOf([], Object.freeze({0:1}))",
        ] {
            let array = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let mut input = runtime.into_jsvalue(context.eval("({})").unwrap()).unwrap();
            let id = object(&input);
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                !state
                    .try_append_dense_value(object(&array), 0, &mut input)
                    .unwrap()
            );
            assert_eq!(object(&input), id);
            state
                .release_owned_jsvalue(&runtime.0.poisoned, input)
                .unwrap();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, array)
                .unwrap();
        }
        let array = runtime.into_jsvalue(context.eval("[]").unwrap()).unwrap();
        let mut input = JsValue::Int(9);
        let mut state = runtime.0.state.borrow_mut();
        assert!(
            !state
                .try_append_dense_value(object(&array), 2, &mut input)
                .unwrap()
        );
        assert!(matches!(input, JsValue::Int(9)));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, array)
            .unwrap();
    }

    #[test]
    fn constructor_append_sites_follow_prototype_receiver_and_getter_changes() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"(() => {
            function P(x, y) { this.a = x; this.b = y; }
            const made = [];
            for (let i = 0; i < 8; i++) made.push(new P(i, {i}));
            if (!made.every((o, i) => o.a === i && o.b.i === i)) return 1;

            // A setter added to the constructor prototype after the site learned.
            let seen = 0;
            Object.defineProperty(P.prototype, 'b', {set(v) { seen++; }, configurable: true});
            const withSetter = new P(1, 2);
            if (seen !== 1 || Object.hasOwn(withSetter, 'b')) return 2;
            delete P.prototype.b;
            if (!Object.hasOwn(new P(1, 2), 'b')) return 3;

            // A read-only property further up the chain.
            Object.defineProperty(Object.prototype, 'a', {value: 0, writable: false, configurable: true});
            const readOnly = new P(1, 2);
            if (Object.hasOwn(readOnly, 'a') || readOnly.a !== 0) return 4;
            function S(x) { 'use strict'; this.a = x; }
            try { new S(1); return 5; } catch (e) { if (!(e instanceof TypeError)) return 6; }
            delete Object.prototype.a;
            if (new S(7).a !== 7) return 7;

            // Receivers that lost extensibility or already have the key.
            function Q(seal, x) { if (seal === 1) Object.preventExtensions(this);
                                  if (seal === 2) Object.freeze(this);
                                  if (seal === 3) { this.z = 0; delete this.z; }
                                  this.a = x; this.b = x; }
            for (let i = 0; i < 4; i++) new Q(0, i);
            const closed = new Q(1, 5), frozen = new Q(2, 5), dictionary = new Q(3, 5);
            if (Object.hasOwn(closed, 'a') || Object.hasOwn(frozen, 'b')) return 8;
            if (dictionary.a !== 5 || dictionary.b !== 5) return 9;
            function T(x) { 'use strict'; Object.preventExtensions(this); this.a = x; }
            try { new T(1); return 10; } catch (e) { if (!(e instanceof TypeError)) return 11; }

            // A getter evaluated for the stored value changes the receiver
            // shape and the prototype chain between two cached appends.
            let mode = 0;
            const source = { get v() {
                if (mode === 1) self.extra = 1;
                if (mode === 2) Object.setPrototypeOf(self, {set b(v) { seen += 10; }});
                return 3;
            } };
            let self;
            function G() { self = this; this.a = source.v; this.b = source.v; }
            for (let i = 0; i < 4; i++) new G();
            mode = 1; const grown = new G();
            if (Object.keys(grown).join() !== 'extra,a,b') return 12;
            mode = 2; seen = 0; const redirected = new G();
            if (seen !== 10 || Object.hasOwn(redirected, 'b') || redirected.a !== 3) return 13;
            mode = 0; const plain = new G();
            return Object.keys(plain).join() === 'a,b' ? 0 : 14;
        })()"#).unwrap(), Value::Int(0));
        runtime.run_gc().unwrap();
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn constructor_append_sites_keep_values_alive_across_collection() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"
            function N(v) { this.self = this; this.value = v; this.tag = Symbol('t'); }
            globalThis.kept = [];
            for (let i = 0; i < 2000; i++) { const n = new N({i}); if (i % 100 === 0) kept.push(n); }
            kept.length
        "#).unwrap(), Value::Int(20));
        runtime.run_gc().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"
            kept.every((n, k) => n.self === n && n.value.i === k * 100 && typeof n.tag === 'symbol')
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        drop(context.eval("kept = null").unwrap());
        runtime.run_gc().unwrap();
        assert!(!runtime.0.poisoned.get());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn warm_constructor_appends_hit_their_sites() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            function P(x) { this.a = x; this.b = x; }
            let s; for (let i = 0; i < 100; i++) s = new P(i);
            return s.b;
        })()"#
                )
                .unwrap(),
            Value::Int(99)
        );
        let events = profile.snapshot().owned_execution_events;
        let count = |name| events.get(name).copied().unwrap_or(0);
        assert!(count("ordinary_owned_field_append_site_hit") >= 198);
        assert!(count("ordinary_owned_field_append_in_state") <= 2);
    }

    #[test]
    fn literal_and_class_field_definitions_keep_define_semantics() {
        // Expected output was checked against Node.js 24.
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let result = context.eval(r#"(() => {
var out = [];
function lit(i){ return {a:i, b:i, a:i+1}; }
for (var i=0;i<5;i++) { var o = lit(i); out.push(JSON.stringify(o), Object.keys(o).join()); }
var g = {get a(){return 1}, a: 2}; out.push(JSON.stringify(Object.getOwnPropertyDescriptor(g,'a')));
var f = {set a(v){}, a: 3}; out.push(f.a);
Object.defineProperty(Object.prototype, 'q', {set(v){ out.push('setter!') }, configurable:true});
var withQ = {q: 1}; out.push(Object.getOwnPropertyDescriptor(withQ,'q').value);
delete Object.prototype.q;
Object.defineProperty(Object.prototype, 'r', {value: 0, writable:false, configurable:true});
var withR = {r: 5}; out.push(withR.r);
delete Object.prototype.r;
var p = {__proto__: {z:1}, y:2}; out.push(p.z, Object.keys(p).join());
var sym = Symbol('s'); var s = {[sym]: 1, 0: 'zero', 1: 'one', x: 1}; out.push(Object.keys(s).join(), s[sym]);
var log = [];
class Base { constructor(){ return new Proxy({}, {defineProperty(t,k,d){ log.push(k); return Reflect.defineProperty(t,k,d); }}); } }
class D extends Base { f1 = 1; f2 = 2; }
var d1 = new D(); var d2 = new D(); out.push(log.join(), d2.f2);
class Fr { constructor(){ return Object.preventExtensions({}); } }
class E extends Fr { x = 1; }
try { new E(); out.push('no throw'); } catch (e) { out.push(e instanceof TypeError); }
class C { a = 1; b = this.a + 1; }
for (var k=0;k<3;k++) { var c = new C(); out.push(c.a + c.b); }
class Arr extends Array { tag = 'arr'; } var arr = new Arr(); out.push(arr.tag, Array.isArray(arr));
return out.join('|');

        })()"#).unwrap();
        let Value::String(result) = result else {
            panic!("string result")
        };
        assert_eq!(
            result.to_string(),
            r#"{"a":1,"b":0}|a,b|{"a":2,"b":1}|a,b|{"a":3,"b":2}|a,b|{"a":4,"b":3}|a,b|{"a":5,"b":4}|a,b|{"value":2,"writable":true,"enumerable":true,"configurable":true}|3|1|5|1|y|0,1,x|1|f1,f2,f1,f2|2|true|3|3|3|arr|true"#
        );
        runtime.run_gc().unwrap();
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn literal_definitions_keep_values_alive_across_collection() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"
            globalThis.kept = [];
            for (let i = 0; i < 2000; i++) {
                const o = {self: null, value: {i}, tag: Symbol('t')};
                o.self = o;
                if (i % 100 === 0) kept.push(o);
            }
            kept.length
        "#
                )
                .unwrap(),
            Value::Int(20)
        );
        runtime.run_gc().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"
            kept.every((o, k) => o.self === o && o.value.i === k * 100 && typeof o.tag === 'symbol')
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        drop(context.eval("kept = null").unwrap());
        runtime.run_gc().unwrap();
        assert!(!runtime.0.poisoned.get());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn warm_literal_fields_define_in_the_loop_and_hit_their_sites() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            let s; for (let i = 0; i < 100; i++) s = {a: i, b: i};
            return s.b;
        })()"#
                )
                .unwrap(),
            Value::Int(99)
        );
        let events = profile.snapshot().owned_execution_events;
        let count = |name| events.get(name).copied().unwrap_or(0);
        assert!(count("ordinary_owned_field_define_site_hit") >= 198);
        assert!(count("ordinary_owned_field_define_in_state") <= 2);
        assert_eq!(count("public_field_query_fallback"), 0);
    }
}
