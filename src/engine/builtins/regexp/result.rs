//! Construction of builtin RegExp match result arrays.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::heap::{ContextId, ObjectData, PropertySlot};
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use std::collections::HashMap;

use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::{JsString, JsValue, Value};
use crate::regexp::{CompiledRegExp, RegExpFlags, RegExpMatch};
use std::rc::Rc;

/// First-occurrence order and the participating value are separate rules for
/// duplicate group names. Keep their resolution here, before heap publication.
#[derive(Default)]
struct NamedCaptures {
    positions: HashMap<Atom, usize>,
    values: Vec<(PropertyKey, usize)>,
}
impl NamedCaptures {
    fn record(&mut self, key: PropertyKey, capture_index: usize, participates: bool) {
        if let Some(&position) = self.positions.get(&key.atom()) {
            if participates {
                self.values[position].1 = capture_index;
            }
        } else {
            self.positions.insert(key.atom(), self.values.len());
            self.values.push((key, capture_index));
        }
    }
}
// A construction transaction owns all producer edges until the result objects
// retain them. Named groups refer to capture positions, never duplicate nodes.
struct RegExpResultOwner {
    runtime: Runtime,
    captures: Vec<JsValue>,
    indices: Vec<JsValue>,
    properties: Vec<JsValue>,
    indices_groups: JsValue,
}
impl Drop for RegExpResultOwner {
    fn drop(&mut self) {
        for value in self
            .captures
            .drain(..)
            .chain(self.indices.drain(..))
            .chain(self.properties.drain(..))
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        let _ = self.runtime.release_jsvalue(std::mem::replace(
            &mut self.indices_groups,
            JsValue::Undefined,
        ));
    }
}

impl Runtime {
    pub(crate) fn build_regexp_result(
        &self,
        realm: ContextId,
        input: JsString,
        input_value: &JsValue,
        program: Rc<CompiledRegExp>,
        matched: RegExpMatch,
    ) -> Result<JsValue, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_result.build");
        let capture_count = matched.captures().len();
        if usize::from(program.capture_count()) != capture_count {
            return Err(RuntimeError::Invariant(
                "compiled RegExp capture count did not align with its match",
            ));
        }
        let group_names = program.group_names();
        if let Some(group_names) = group_names
            && group_names.len() != capture_count.saturating_sub(1)
        {
            return Err(RuntimeError::Invariant(
                "compiled RegExp group names did not align with captures",
            ));
        }

        let has_indices = program.flags().contains(RegExpFlags::HAS_INDICES);
        let mut named = NamedCaptures::default();
        let mut owner = RegExpResultOwner {
            runtime: self.clone(),
            captures: Vec::with_capacity(capture_count),
            indices: Vec::with_capacity(if has_indices { capture_count } else { 0 }),
            properties: Vec::with_capacity(4),
            indices_groups: JsValue::Undefined,
        };
        for (capture_index, range) in matched.captures().iter().enumerate() {
            let capture = match range {
                Some(range) => {
                    self.into_jsvalue(Value::String(input.sub_string(range.start, range.end)))?
                }
                None => JsValue::Undefined,
            };
            owner.captures.push(capture);
            if has_indices {
                let indices = match range {
                    Some(range) => {
                        let start = i32::try_from(range.start).map_err(|_| {
                            RuntimeError::Invariant(
                                "RegExp capture start exceeded signed String range",
                            )
                        })?;
                        let end = i32::try_from(range.end).map_err(|_| {
                            RuntimeError::Invariant(
                                "RegExp capture end exceeded signed String range",
                            )
                        })?;
                        JsValue::Object(
                            self.new_array_from_values_jsvalue(
                                realm,
                                vec![JsValue::Int(start), JsValue::Int(end)],
                            )?
                            .into_handle(),
                        )
                    }
                    None => JsValue::Undefined,
                };
                owner.indices.push(indices);
            }
            if capture_index > 0
                && let Some(Some(group_name)) =
                    group_names.and_then(|names| names.get(capture_index - 1))
            {
                named.record(
                    self.intern_property_key_js_string(group_name)?,
                    capture_index,
                    range.is_some(),
                );
            }
        }
        let complete = matched.capture(0).ok_or(RuntimeError::Invariant(
            "successful RegExp result omitted capture zero",
        ))?;
        owner
            .properties
            .push(JsValue::Int(i32::try_from(complete.start).map_err(
                |_| RuntimeError::Invariant("RegExp match start exceeded signed String range"),
            )?));
        owner.properties.push(self.dup_jsvalue(input_value)?);
        let groups = if group_names.is_some() {
            JsValue::Object(
                self.new_regexp_groups(realm, &named, &owner.captures)?
                    .into_handle(),
            )
        } else {
            JsValue::Undefined
        };
        owner.properties.push(groups);
        if has_indices {
            if group_names.is_some() {
                owner.indices_groups = JsValue::Object(
                    self.new_regexp_groups(realm, &named, &owner.indices)?
                        .into_handle(),
                );
            }
            let indices = self.new_regexp_result_array(
                realm,
                &owner.indices,
                std::slice::from_ref(&owner.indices_groups),
                2,
            )?;
            owner
                .properties
                .push(JsValue::Object(indices.into_handle()));
        }
        let result = self.new_regexp_result_array(
            realm,
            &owner.captures,
            &owner.properties,
            usize::from(has_indices),
        )?;
        Ok(JsValue::Object(result.into_handle()))
    }

    fn new_regexp_groups(
        &self,
        realm: ContextId,
        named: &NamedCaptures,
        captures: &[JsValue],
    ) -> Result<ObjectRef, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("regexp_result.groups_layout");
        let names = named
            .values
            .iter()
            .map(|(key, _)| key.atom())
            .collect::<Vec<_>>();
        let shape = {
            let mut state = self.0.state.borrow_mut();
            if let Some(&shape) = state.heap.context(realm)?.regexp_group_shapes.get(&names) {
                shape
            } else {
                let entries = names
                    .iter()
                    .map(|&atom| ShapeEntry {
                        atom: AtomIdx::from_raw(atom.raw()),
                        flags: PropertyFlags::data(true, true, true),
                    })
                    .collect::<Vec<_>>();
                let shape = state.get_or_create_shape(None, &entries)?;
                let cached = state.heap.cache_regexp_group_shape(realm, names, shape);
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                if let Some(evicted) = cached? {
                    let cleanup = state.heap.release_shape(evicted)?;
                    state.apply_cleanup(cleanup)?;
                }
                shape
            }
        };
        let slots = named
            .values
            .iter()
            .map(|(_, index)| PropertySlot::Data(captures[*index].as_raw()))
            .collect::<Vec<_>>();
        // Allocation retains storage edges while the construction owner remains live.
        let id = {
            let mut state = self.0.state.borrow_mut();
            let atoms = state.retain_slot_atoms(&slots)?;
            match state
                .heap
                .allocate_object(ObjectData::ordinary(shape, slots))
            {
                Ok(id) => id,
                Err(error) => {
                    state.release_atoms(atoms)?;
                    return Err(error.into());
                }
            }
        };
        Ok(ObjectRef::from_owned_handle(self.clone(), id))
    }

    /// Publish the final named layout before adding any dense captures. No
    /// intermediate Array layout or property replacement is constructed.
    fn new_regexp_result_array(
        &self,
        realm: ContextId,
        captures: &[JsValue],
        properties: &[JsValue],
        layout: usize,
    ) -> Result<ObjectRef, RuntimeError> {
        let shape = self
            .regexp_realm_data(realm)?
            .result_shapes
            .ok_or(RuntimeError::Invariant("RegExp result layouts missing"))?[layout];
        let mut slots = Vec::with_capacity(properties.len() + 1);
        slots.push(PropertySlot::Data(crate::engine::heap::RawValue::Int(0)));
        slots.extend(
            properties
                .iter()
                .map(|value| PropertySlot::Data(value.as_raw())),
        );
        // Allocation retains existing payload handles, without arena conversion.
        let id = {
            let mut state = self.0.state.borrow_mut();
            let atoms = state.retain_slot_atoms(&slots)?;
            match state.heap.allocate_object(ObjectData::array(shape, slots)) {
                Ok(id) => id,
                Err(error) => {
                    state.release_atoms(atoms)?;
                    return Err(error.into());
                }
            }
        };
        let result = ObjectRef::from_owned_handle(self.clone(), id);
        for value in captures {
            self.append_fresh_array_value_jsvalue(&result, self.dup_jsvalue(value)?)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_eval_true(source: &str) {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context.eval(source).expect("RegExp result probe threw"),
            Value::Bool(true),
        );
    }

    #[test]
    fn named_capture_publication_reuses_the_numbered_capture_node() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        {
            let mut context = runtime.new_context();
            let Value::Object(result) = context.eval("/(?<name>abc)/d.exec('abc')").unwrap() else {
                panic!("match result")
            };
            let groups_key = runtime.intern_property_key("groups").unwrap();
            let name_key = runtime.intern_property_key("name").unwrap();
            let Value::Object(groups) = context.get_property(&result, &groups_key).unwrap() else {
                panic!("groups")
            };
            let state = runtime.0.state.borrow();
            let crate::engine::heap::ObjectPayload::Array {
                dense: Some(captures),
            } = &state.heap.object(result.object_id()).unwrap().payload
            else {
                panic!("captures")
            };
            let crate::engine::heap::RawValue::String(numbered) = &captures[1] else {
                panic!("capture string")
            };
            let groups = state.heap.object(groups.object_id()).unwrap();
            let slot = state
                .heap
                .shape(groups.shape)
                .unwrap()
                .find(AtomIdx::from_raw(name_key.atom().raw()))
                .unwrap();
            let PropertySlot::Data(crate::engine::heap::RawValue::String(named)) =
                &groups.slots[slot as usize]
            else {
                panic!("named string")
            };
            assert_eq!(numbered, named);
        }
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn named_shape_cache_is_bounded_and_eviction_preserves_results() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
            var saved;
            for (var i = 0; i < 100; i++) {
                var result = new RegExp("(?<g" + i + ">a)", "d").exec("a");
                if (i === 0) saved = result;
                if (result.groups["g" + i] !== "a") throw "group lost";
            }
            saved.groups.g0 === "a" && saved.indices.groups.g0 === saved.indices[1]
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        // Every live realm cache is bounded independently of user-held results.
        // The runtime GC oracle also verifies their shape ownership edges.
    }

    #[test]
    fn named_groups_are_null_prototype_cwe_objects_in_capture_order() {
        assert_eval_true(
            r#"
var result = /(?<first>a)(?<second>b)?/.exec("a");
var groups = result.groups;
var first = Object.getOwnPropertyDescriptor(groups, "first");
var second = Object.getOwnPropertyDescriptor(groups, "second");
Object.getPrototypeOf(groups) === null &&
Object.keys(groups).join(",") === "first,second" &&
groups.first === "a" && groups.second === undefined &&
first.writable === true && first.enumerable === true && first.configurable === true &&
second.writable === true && second.enumerable === true && second.configurable === true
"#,
        );
    }

    #[test]
    fn named_indices_reuse_the_capture_arrays_and_preserve_unmatched_values() {
        assert_eval_true(
            r#"
var result = /(?<first>a)(?<second>b)?/d.exec("a");
var groups = result.indices.groups;
var first = Object.getOwnPropertyDescriptor(groups, "first");
Object.getPrototypeOf(groups) === null &&
Object.keys(groups).join(",") === "first,second" &&
groups.first === result.indices[1] &&
groups.second === result.indices[2] && groups.second === undefined &&
first.writable === true && first.enumerable === true && first.configurable === true
"#,
        );
    }

    #[test]
    fn duplicate_names_keep_first_order_and_the_participating_value() {
        assert_eval_true(
            r#"
var left = /(?:(?<x>a)|(?<x>b))/.exec("a");
var right = /(?:(?<x>a)|(?<x>b))/.exec("b");
left.groups.x === "a" && right.groups.x === "b" &&
Object.keys(left.groups).join(",") === "x" &&
Object.keys(right.groups).join(",") === "x"
"#,
        );
    }

    #[test]
    fn named_groups_force_standard_string_replace_through_get_substitution() {
        assert_eval_true(
            r#"/b/[Symbol.replace]("b", "<$<x>>") === "<$<x>>" && /(?<x>b)/[Symbol.replace]("b", "<$<x>>") === "<b>""#,
        );
    }
    #[test]
    fn global_match_private_output_ignores_prototype_setters_and_keeps_callback_order() {
        assert_eval_true(
            r#"
(function () {
    var calls = 0, trace = "", setterCalls = 0;
    var re = {flags: "g", lastIndex: 0, exec: function () {
        trace += "e";
        if (calls++ === 2) return null;
        return {get 0() { trace += "g"; return {toString: function () {
            trace += "s"; return "a";
        }}; }};
    }};
    Object.defineProperty(Array.prototype, "0", {
        configurable: true, set: function () { setterCalls++; }
    });
    var result;
    try { result = RegExp.prototype[Symbol.match].call(re, "aa"); }
    finally { delete Array.prototype["0"]; }
    var first = Object.getOwnPropertyDescriptor(result, "0");
    return trace === "egsegse" && setterCalls === 0 && result.length === 2 &&
        result[0] === "a" && result[1] === "a" && first.writable &&
        first.enumerable && first.configurable &&
        Object.getPrototypeOf(result) === Array.prototype;
})()
"#,
        );
    }

    #[test]
    fn split_private_output_preserves_capture_identity_and_limit_order() {
        assert_eval_true(
            r#"
(function () {
    var token = {}, trace = "", setterCalls = 0;
    var splitter = {lastIndex: 0, exec: function () {
        trace += "e";
        if (this.lastIndex === 0) return null;
        this.lastIndex = 2;
        return {length: 3, get 1() { trace += "a"; return token; },
            get 2() { trace += "b"; throw "must not read past limit"; }};
    }};
    var re = {flags: "", constructor: {[Symbol.species]: function () {
        trace += "c"; return splitter;
    }}};
    Object.defineProperty(Array.prototype, "1", {
        configurable: true, set: function () { setterCalls++; }
    });
    var result;
    try { result = RegExp.prototype[Symbol.split].call(re, "abc", 2); }
    finally { delete Array.prototype["1"]; }
    var capture = Object.getOwnPropertyDescriptor(result, "1");
    return trace === "ceea" && setterCalls === 0 && result.length === 2 &&
        result[0] === "a" && result[1] === token && capture.writable &&
        capture.enumerable && capture.configurable &&
        Object.getPrototypeOf(result) === Array.prototype;
})()
"#,
        );
    }
}
