//! Exchange existing owners under one state access; no pending representation.
use super::*;

fn public_value(value: &RawValue) -> bool {
    !matches!(
        value,
        RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception
    )
}

impl RuntimeState {
    /// Resolve a static key owned by the current published executable, then
    /// consume the frame's existing value owner through the ordinary selector.
    #[inline]
    pub(crate) fn try_exchange_linked_field(
        &mut self,
        domain: u64,
        object: ObjectId,
        input: &mut JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
    ) -> Result<bool, RuntimeError> {
        let Some(atom) = super::linked_field_atom_in_domain(domain, executable, key) else {
            return Ok(false);
        };
        self.try_exchange_own_data(object, atom, input)
    }

    /// The input owner becomes the old slot owner. A miss leaves it untouched.
    /// Physical Set selection is shared with the ordinary semantic algorithm.
    pub(crate) fn try_exchange_own_data(
        &mut self,
        object: ObjectId,
        atom: Atom,
        input: &mut JsValue,
    ) -> Result<bool, RuntimeError> {
        if !public_value(&input.as_raw()) {
            return Ok(false);
        }
        let BorrowedSet::Data(selected) = select_set_slot(self, object, atom)? else {
            return Ok(false);
        };
        if !selected.flags.writable {
            return Ok(false);
        }
        // Both locations already own their edges and atoms. Swapping their
        // representations neither retains nor releases either owner. The
        // consumer retires the old value while its receiver remains rooted.
        let mut raw = input.as_raw();
        let exchanged = self
            .heap
            .exchange_owned_data_slot(object, selected.index, &mut raw)?;
        if exchanged {
            *input = JsValue::from_raw(raw).expect("exchanged public data owner");
        }
        Ok(exchanged)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn try_exchange_dense_value(
        &mut self,
        object: ObjectId,
        index: u32,
        input: &mut JsValue,
    ) -> Result<bool, RuntimeError> {
        if !public_value(&input.as_raw()) {
            return Ok(false);
        }
        let mut raw = input.as_raw();
        let exchanged = self
            .heap
            .exchange_owned_dense_value(object, index, &mut raw)?;
        if exchanged {
            *input = JsValue::from_raw(raw).expect("exchanged public dense owner");
        }
        Ok(exchanged)
    }

    /// Append only the next dense index. The existing shared prototype walk
    /// and length selector establish the complete callback-free Set case.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn try_append_dense_value(
        &mut self,
        object: ObjectId,
        index: u32,
        input: &mut JsValue,
    ) -> Result<bool, RuntimeError> {
        if !public_value(&input.as_raw()) {
            return Ok(false);
        }
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
                .try_exchange_own_data(object(&base), key.atom(), &mut input)
                .unwrap()
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
            "({})",
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
                    .try_exchange_own_data(object(&base), key.atom(), &mut input)
                    .unwrap()
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
