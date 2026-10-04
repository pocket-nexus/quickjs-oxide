//! Exchange existing owners under one state access; no pending representation.
use super::*;

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
    #[inline]
    pub(crate) fn try_store_owned_linked_field(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
    ) -> Result<FieldStore, RuntimeError> {
        let Some(atom) = super::linked_field_atom_in_domain(domain, executable, key) else {
            return Ok(FieldStore::Miss);
        };
        self.try_store_owned_own_data(poisoned, object, atom, input)
    }

    /// A missing ordinary property consumes the same canonical append policy
    /// as descriptors. Setter, exotic and shared-dictionary cases decline with
    /// the input untouched; no continuation is constructed on local success.
    #[inline]
    fn try_store_owned_own_data(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        object: ObjectId,
        atom: Atom,
        input: &mut JsValue,
    ) -> Result<FieldStore, RuntimeError> {
        let prototype = match select_set_slot(self, object, atom)? {
            BorrowedSet::Missing(prototype) => prototype,
            BorrowedSet::Data(selected) if selected.flags.writable => {
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
            BorrowedSet::Data(_) | BorrowedSet::Setter(_) | BorrowedSet::Special(_) => {
                return Ok(FieldStore::Miss);
            }
        };
        self.append_missing_owned_data(poisoned, object, atom, prototype, input)
    }

    #[inline(never)]
    fn append_missing_owned_data(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        object: ObjectId,
        atom: Atom,
        prototype: Option<ObjectId>,
        input: &mut JsValue,
    ) -> Result<FieldStore, RuntimeError> {
        if !matches!(
            set_missing_local(self, object, atom, prototype)?,
            MissingSelection::Define
        ) {
            return Ok(FieldStore::Miss);
        }
        if !self.append_selected_missing_slot(
            Some(poisoned),
            object,
            atom,
            crate::engine::object::shape::PropertyFlags::data(true, true, true),
            crate::engine::object::SlotAppendInput::Owned(input),
        )? {
            return Ok(FieldStore::Miss);
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_owned_field_append_in_state",
        );
        Ok(FieldStore::LayoutPublished)
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
        let Some(length) =
            crate::engine::object::dense_mutation::writable_dense_length(self, data)?
        else {
            return Ok(false);
        };
        if index != length {
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
        match self
            .heap
            .append_fresh_array_dense_value_owned(object, value.into_raw())
        {
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
                        object(&first),
                        key.atom(),
                        &mut input
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
                        object(&second),
                        key.atom(),
                        &mut other
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
                        object(&receiver),
                        key.atom(),
                        &mut input
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
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("ordinary_owned_field_append_in_state")
                .copied()
                .unwrap_or(0)
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
                    object(&base),
                    key.atom(),
                    &mut input
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
                        object(&base),
                        key.atom(),
                        &mut input
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
}
