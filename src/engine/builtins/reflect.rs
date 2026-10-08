//! Pinned QuickJS `Reflect` intrinsic algorithms.
//!
//! The implementation follows the 2026-06-04 `js_reflect_funcs` table and
//! intentionally retains QuickJS's validation order where it differs from a
//! tempting shared helper. In particular, `Reflect.construct` validates an
//! explicit `newTarget`, then materializes the argument list, and only then
//! validates the target constructor.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{ObjectId, RawValue};
use std::cell::Cell;

use crate::engine::builtins::native::{NativeFunctionId, ReflectKind};
use crate::engine::heap::{AutoInitProperty, ContextId, HeapError, ObjectPayload, PropertySlot};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{ObjectRef, WellKnownSymbol};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

#[cfg(test)]
mod tests;

const MAX_APPLY_ARGUMENTS: u64 = 65_534;

impl Runtime {
    /// Snapshot QuickJS's fast Array/Arguments storage in numeric-index order.
    ///
    /// Arrays expose their physical dense payload directly. Arguments retain
    /// their distinct mapped/unmapped shape-slot representation and are
    /// reconstructed without observable property lookup. The owning object
    /// stays rooted while raw values are promoted to public roots.
    pub(crate) fn fast_array_like_values_jsvalue(
        &self,
        object: &ObjectRef,
        expected_len: u32,
    ) -> Result<Option<Vec<JsValue>>, RuntimeError> {
        let raw_values = {
            let state = self.0.state.borrow();
            let object_data = state.heap.object(object.object_id())?;
            if let ObjectPayload::Array { dense } = &object_data.payload {
                let Some(dense) = dense else {
                    return Ok(None);
                };
                if dense.len() != expected_len as usize {
                    return Ok(None);
                }
                let mut values = Vec::new();
                values
                    .try_reserve_exact(dense.len())
                    .map_err(|_| HeapError::Allocation {
                        operation: "snapshotting fast Array values",
                    })?;
                values.extend_from_slice(dense);
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_call_buffer_capacity(
                        "arguments.fast_raw",
                        0,
                        values.capacity(),
                        size_of::<crate::engine::heap::RawValue>(),
                    );
                    crate::engine::api::profiling::record_call_buffer_initialized(
                        "arguments.fast_raw",
                        values.len(),
                    );
                }
                values
            } else {
                let (mapped, fast_len) = match &object_data.payload {
                    ObjectPayload::Arguments { mapped, fast_len } => (*mapped, *fast_len),
                    _ => return Ok(None),
                };
                if fast_len != Some(expected_len) {
                    return Ok(None);
                }
                let shape = state.heap.shape(object_data.shape)?;
                let capacity = usize::try_from(expected_len).map_err(|_| {
                    RuntimeError::Invariant("fast argument length does not fit usize")
                })?;
                let mut ordered = vec![None; capacity];
                #[cfg(feature = "profiling")]
                {
                    crate::engine::api::profiling::record_call_buffer_capacity(
                        "arguments.fast_ordering",
                        0,
                        ordered.capacity(),
                        size_of::<Option<crate::engine::heap::RawValue>>(),
                    );
                    crate::engine::api::profiling::record_call_buffer_initialized(
                        "arguments.fast_ordering",
                        ordered.len(),
                    );
                }
                for (entry, slot) in shape.entries().iter().zip(&object_data.slots) {
                    let Some(index) = state.atoms.array_index(state.atoms.brand(entry.atom)?)?
                    else {
                        continue;
                    };
                    if index >= expected_len {
                        continue;
                    }
                    if !entry.flags.writable || !entry.flags.enumerable || !entry.flags.configurable
                    {
                        return Err(RuntimeError::Invariant(
                            "fast argument index is not a C/W/E property",
                        ));
                    }
                    let value = match slot {
                        PropertySlot::VarRef(var_ref) if mapped => {
                            state.heap.var_ref(*var_ref)?.value.clone()
                        }
                        // Extra actual arguments of a mapped object alias no
                        // formal parameter and are plain data.
                        PropertySlot::Data(value) => value.clone(),
                        _ => {
                            return Err(RuntimeError::Invariant(
                                "fast argument index has the wrong storage kind",
                            ));
                        }
                    };
                    let destination = ordered
                        .get_mut(usize::try_from(index).map_err(|_| {
                            RuntimeError::Invariant("fast argument index does not fit usize")
                        })?)
                        .ok_or(RuntimeError::Invariant(
                            "fast argument index escaped its dense prefix",
                        ))?;
                    if destination.replace(value).is_some() {
                        return Err(RuntimeError::Invariant(
                            "fast argument prefix contains a duplicate index",
                        ));
                    }
                }
                ordered
                    .into_iter()
                    .map(|value| {
                        value.ok_or(RuntimeError::Invariant(
                            "fast argument prefix is missing an indexed value",
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        #[cfg(feature = "profiling")]
        {
            // The Arguments ordering collect may reuse its input allocation;
            // observe this raw buffer without inventing a second allocation.
            crate::engine::api::profiling::record_call_buffer_observed(
                "arguments.fast_raw",
                raw_values.capacity(),
                size_of::<crate::engine::heap::RawValue>(),
            );
            crate::engine::api::profiling::record_call_raw_buffer_copies(
                "arguments.fast_raw",
                &raw_values,
            );
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(raw_values.len())
            .map_err(|_| HeapError::Allocation {
                operation: "retaining fast argument snapshot",
            })?;
        for raw in raw_values {
            let borrowed =
                JsValue::from_raw(raw).ok_or(RuntimeError::Invariant("argument storage value"))?;
            match self.dup_jsvalue(&borrowed) {
                Ok(value) => values.push(value),
                Err(error) => {
                    for value in values {
                        let _ = self.release_jsvalue(value);
                    }
                    return Err(error);
                }
            }
        }
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_call_buffer_observed(
                "arguments.fast_internal",
                values.capacity(),
                size_of::<JsValue>(),
            );
            crate::engine::api::profiling::record_call_buffer_js_value_copies(
                "arguments.fast_internal",
                &values,
            );
        }
        Ok(Some(values))
    }

    #[cfg(all(test, feature = "profiling"))]
    pub(crate) fn fast_array_like_values(
        &self,
        object: &ObjectRef,
        expected_len: u32,
    ) -> Result<Option<Vec<Value>>, RuntimeError> {
        let Some(values) = self.fast_array_like_values_jsvalue(object, expected_len)? else {
            return Ok(None);
        };
        let mut values = values.into_iter();
        let mut rooted = Vec::new();
        while let Some(value) = values.next() {
            match self.root_and_release_jsvalue(value) {
                Ok(value) => rooted.push(value),
                Err(error) => {
                    for value in values {
                        let _ = self.release_jsvalue(value);
                    }
                    return Err(error);
                }
            }
        }
        Ok(Some(rooted))
    }

    /// Install the global `Reflect` `JS_OBJECT_DEF` equivalent. The object is
    /// realm-owned and remains lazy until the global slot is first read.
    pub(crate) fn initialize_reflect_intrinsic(
        &self,
        realm: ContextId,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Reflect)?;
        self.store_property_slot(
            global_object,
            &key,
            PropertyFlags::data(true, false, true),
            PropertySlot::auto_init(AutoInitProperty::Reflect { realm }),
        )
    }

    /// QuickJS `build_arg_list`, shared by Function.prototype.apply and the
    /// two Reflect call/construct paths. Nullish exceptions remain a caller
    /// decision: this kernel always requires an object, as upstream does once
    /// it has entered `build_arg_list`.
    /// Classify an Array or intact Arguments carrier without invoking getters.
    /// None leaves the caller free to choose an explicit property-reading protocol.
    pub(crate) fn prepare_fast_array_arguments_jsvalue(
        &self,
        realm: ContextId,
        object: &ObjectRef,
    ) -> Result<Option<NativeConversion<Vec<JsValue>>>, RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        let arguments_length = {
            let state = self.0.state.borrow();
            let data = state.heap.object(object.object_id())?;
            match (data.kind, &data.payload) {
                (crate::engine::heap::ObjectKind::Array, ObjectPayload::Array { .. }) => None,
                (
                    crate::engine::heap::ObjectKind::Arguments,
                    ObjectPayload::Arguments {
                        fast_len: Some(length),
                        ..
                    },
                ) => {
                    // Keep the old property/conversion cleanup checkpoints when
                    // unrelated releases await processing. Otherwise no callback
                    // or release can intervene between this fact and the existing
                    // fast snapshot, while `object` keeps the carrier rooted.
                    if self.0.deferred_references.has_pending()
                        || state.heap.has_pending_zero_cleanup()
                    {
                        return Ok(None);
                    }
                    // The old length protocol owns up to two additional carrier
                    // references (read object and receiver). Reserve headroom for
                    // both and conservatively for every snapshot item aliasing
                    // the carrier, preserving checked overflow/immortal behavior.
                    let headroom = length.saturating_add(2);
                    if state.heap.object_strong_count(object.object_id())?
                        >= u32::MAX.saturating_sub(headroom)
                    {
                        return Ok(None);
                    }
                    let shape = state.heap.shape(data.shape)?;
                    let key = state
                        .pinned_atoms
                        .get(crate::engine::atom::pinned::PinnedAtom::Length);
                    let Some(slot) = shape.find(crate::engine::atom::AtomIdx::from_raw(key.raw()))
                    else {
                        return Ok(None);
                    };
                    if shape.entries()[slot as usize].flags.storage
                        != crate::engine::object::shape::PropertyStorageKind::Data
                    {
                        return Ok(None);
                    }
                    let matches_prefix = match data.slots.get(slot as usize) {
                        Some(PropertySlot::Data(crate::engine::heap::RawValue::Int(value))) => {
                            u32::try_from(*value).ok() == Some(*length)
                        }
                        Some(PropertySlot::Data(crate::engine::heap::RawValue::Float(value))) => {
                            *value == f64::from(*length)
                        }
                        _ => false,
                    };
                    if !matches_prefix {
                        return Ok(None);
                    }
                    Some(*length)
                }
                _ => return Ok(None),
            }
        };
        let length = if let Some(length) = arguments_length {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("arguments.direct_length");
            u64::from(length)
        } else {
            let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
            let Some(crate::engine::object::CompleteOrdinaryPropertyDescriptor::Data {
                value, ..
            }) = self.get_own_property(object, &key)?
            else {
                return Err(RuntimeError::Invariant(
                    "Array argument carrier has no data length",
                ));
            };
            match value {
                Value::Int(value) if value >= 0 => u64::try_from(value).unwrap(),
                Value::Float(value)
                    if value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0 =>
                {
                    value as u64
                }
                _ => {
                    return Err(RuntimeError::Invariant(
                        "Array argument carrier has an invalid length",
                    ));
                }
            }
        };
        if length > MAX_APPLY_ARGUMENTS {
            return Ok(Some(NativeConversion::Throw(
                self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Range,
                    "too many arguments in function call (only 65534 allowed)",
                )?,
            )));
        }
        self.fast_array_like_values_jsvalue(object, length as u32)
            .map(|values| values.map(NativeConversion::Value))
    }

    #[cfg(test)]
    pub(crate) fn prepare_fast_array_arguments(
        &self,
        realm: ContextId,
        object: &ObjectRef,
    ) -> Result<Option<NativeConversion<Vec<Value>>>, RuntimeError> {
        Ok(
            match self.prepare_fast_array_arguments_jsvalue(realm, object)? {
                None => None,
                Some(NativeConversion::Throw(value)) => Some(NativeConversion::Throw(value)),
                Some(NativeConversion::Value(values)) => {
                    let mut rooted = Vec::new();
                    let mut values = values.into_iter();
                    while let Some(value) = values.next() {
                        match self.root_and_release_jsvalue(value) {
                            Ok(value) => rooted.push(value),
                            Err(error) => {
                                for value in values {
                                    let _ = self.release_jsvalue(value);
                                }
                                return Err(error);
                            }
                        }
                    }
                    Some(NativeConversion::Value(rooted))
                }
            },
        )
    }

    pub(crate) fn call_reflect(
        &self,
        realm: ContextId,
        kind: ReflectKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Reflect method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        match kind {
            ReflectKind::Apply => self.call_reflect_apply(realm, arguments),
            ReflectKind::Construct => self.call_reflect_construct(realm, arguments),
            ReflectKind::DefineProperty => self.call_reflect_define_property(realm, arguments),
            ReflectKind::DeleteProperty => self.call_reflect_delete_property(realm, arguments),
            ReflectKind::Get => self.call_reflect_get(realm, arguments),
            ReflectKind::GetOwnPropertyDescriptor => {
                self.call_reflect_get_own_property_descriptor(realm, arguments)
            }
            ReflectKind::GetPrototypeOf => self.call_reflect_get_prototype_of(realm, arguments),
            ReflectKind::Has => self.call_reflect_has(realm, arguments),
            ReflectKind::IsExtensible => self.call_reflect_is_extensible(realm, arguments),
            ReflectKind::OwnKeys => self.call_reflect_own_keys(realm, arguments),
            ReflectKind::PreventExtensions => {
                self.call_reflect_prevent_extensions(realm, arguments)
            }
            ReflectKind::Set => self.call_reflect_set(realm, arguments),
            ReflectKind::SetPrototypeOf => self.call_reflect_set_prototype_of(realm, arguments),
        }
    }

    fn call_reflect_apply(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::function::invoke::finish(
            self,
            realm,
            super::function::invoke::InvokeStep::start(
                self,
                realm,
                super::function::invoke::InvokeKind::ReflectApply,
                &NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                arguments,
            )?,
        )
    }

    fn call_reflect_construct(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::function::invoke::finish(
            self,
            realm,
            super::function::invoke::InvokeStep::start(
                self,
                realm,
                super::function::invoke::InvokeKind::ReflectConstruct,
                &NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                arguments,
            )?,
        )
    }

    fn call_reflect_define_property(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Define, arguments)?,
        )
    }

    fn call_reflect_delete_property(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Delete, arguments)?,
        )
    }

    fn call_reflect_get(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Get, arguments)?,
        )
    }

    fn call_reflect_get_own_property_descriptor(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Descriptor, arguments)?,
        )
    }

    fn call_reflect_get_prototype_of(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::object::prototype::finish(
            self,
            realm,
            super::object::prototype::BuiltinPrototypeStep::start(
                self,
                realm,
                super::object::prototype::BuiltinPrototypeKind::ReflectGet,
                arguments,
            )?,
        )
    }

    fn call_reflect_has(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Has, arguments)?,
        )
    }

    fn call_reflect_is_extensible(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Extensible, arguments)?,
        )
    }

    fn call_reflect_own_keys(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Keys, arguments)?,
        )
    }

    fn call_reflect_prevent_extensions(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Prevent, arguments)?,
        )
    }

    fn call_reflect_set(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        use super::object::property::{self, PropertyKind, PropertyStep};
        property::finish(
            self,
            realm,
            PropertyStep::start(self, realm, PropertyKind::Set, arguments)?,
        )
    }

    fn call_reflect_set_prototype_of(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::object::prototype::finish(
            self,
            realm,
            super::object::prototype::BuiltinPrototypeStep::start(
                self,
                realm,
                super::object::prototype::BuiltinPrototypeKind::ReflectSet,
                arguments,
            )?,
        )
    }
}

#[cfg(test)]
mod argument_preparation_tests {
    use super::*;

    #[test]
    fn array_argument_preflight_keeps_getters_out_of_the_fast_path() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in [
            "({get length(){throw 99}})",
            "Object.defineProperty([1],'0',{get(){throw 99}})",
            "Object.assign(Object.create({get 0(){throw 99}}),{length:1})",
        ] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("expected carrier")
            };
            assert!(
                runtime
                    .prepare_fast_array_arguments(context.realm, &object)
                    .unwrap()
                    .is_none(),
                "{source}"
            );
        }
        let Value::Object(object) = context.eval("[40,2]").unwrap() else {
            panic!("expected array")
        };
        let Some(NativeConversion::Value(values)) = runtime
            .prepare_fast_array_arguments(context.realm, &object)
            .unwrap()
        else {
            panic!("expected snapshot")
        };
        assert_eq!(values, vec![Value::Int(40), Value::Int(2)]);
        let Value::Object(oversized) = context.eval("Array(65535)").unwrap() else {
            panic!("expected array")
        };
        let Some(NativeConversion::Throw(thrown)) = runtime
            .prepare_fast_array_arguments(context.realm, &oversized)
            .unwrap()
        else {
            panic!("expected oversized arguments throw")
        };
        runtime.release_jsvalue(thrown).unwrap();
    }
}

#[cfg(test)]
mod arguments_prefix_tests {
    use super::*;

    fn carrier(context: &mut crate::engine::api::Context, source: &str) -> ObjectRef {
        let Value::Object(object) = context.eval(source).unwrap() else {
            panic!("carrier")
        };
        object
    }
    fn snapshot(runtime: &Runtime, realm: ContextId, object: &ObjectRef) -> Vec<Value> {
        let Some(NativeConversion::Value(values)) = runtime
            .prepare_fast_array_arguments_jsvalue(realm, object)
            .unwrap()
        else {
            panic!("expected direct Arguments snapshot")
        };
        values
            .into_iter()
            .map(|value| runtime.root_and_release_jsvalue(value).unwrap())
            .collect()
    }

    #[test]
    fn arguments_prefix_reads_mapped_unmapped_defaults_duplicates_and_live_aliases() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for (source, expected) in [
            (
                "(function(a,b){a=4;return arguments})(1,2)",
                vec![Value::Int(4), Value::Int(2)],
            ),
            (
                "(function(a,b){'use strict';a=4;return arguments})(1,2)",
                vec![Value::Int(1), Value::Int(2)],
            ),
            (
                "(function(a=8,b=9){a=4;return arguments})(1,2)",
                vec![Value::Int(1), Value::Int(2)],
            ),
            (
                "(function(a,a){a=4;return arguments})(1,2)",
                vec![Value::Int(1), Value::Int(4)],
            ),
            ("(function(){return arguments})()", vec![]),
        ] {
            let object = carrier(&mut context, source);
            assert_eq!(
                snapshot(&runtime, context.realm, &object),
                expected,
                "{source}"
            );
        }
        let object = carrier(
            &mut context,
            "(function(a){globalThis.updatePrefix=v=>a=v;return arguments})(1)",
        );
        assert_eq!(
            snapshot(&runtime, context.realm, &object),
            vec![Value::Int(1)]
        );
        let _ = context.eval("updatePrefix(9)").unwrap();
        assert_eq!(
            snapshot(&runtime, context.realm, &object),
            vec![Value::Int(9)]
        );
    }

    #[test]
    fn arguments_prefix_declines_modified_length_holes_and_proxy_without_effects() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let _ = context.eval("globalThis.prefixHits=0").unwrap();
        for setup in [
            "a.length=1",
            "a.length=1.5",
            "a.length=-1",
            "a.length=NaN",
            "a.length='2'",
            "a.length={valueOf(){prefixHits++;return 2}}",
            "delete a.length",
            "Object.defineProperty(a,'length',{get(){prefixHits++;throw 42}})",
            "delete a[0];Object.setPrototypeOf(a,{get 0(){prefixHits++;throw 42}})",
            "delete a[1]",
            "Object.defineProperty(a,'0',{get(){prefixHits++;throw 42}})",
            "a=new Proxy(a,{get(){prefixHits++;throw 42}})",
        ] {
            let object = carrier(
                &mut context,
                &format!(
                    "(()=>{{let a=(function(){{return arguments}})(1,2);{setup};return a}})()"
                ),
            );
            assert!(
                runtime
                    .prepare_fast_array_arguments_jsvalue(context.realm, &object)
                    .unwrap()
                    .is_none(),
                "{setup}"
            );
        }
        assert_eq!(context.eval("prefixHits").unwrap(), Value::Int(0));
        let object = carrier(&mut context, "(function(){return arguments})(1,2)");
        runtime.set_arguments_fast_len(&object, None).unwrap();
        assert!(
            runtime
                .prepare_fast_array_arguments_jsvalue(context.realm, &object)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn arguments_prefix_serves_apply_reflect_apply_and_construct() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            function sum(a,b) { return this.bias+a*10+b; }
            function C(a,b) { this.value=a*10+b; }
            function mapped(a,b) {
                a=4;
                return sum.apply({bias:100},arguments) === 142 &&
                       Reflect.apply(sum,{bias:200},arguments) === 242 &&
                       Reflect.construct(C,arguments).value === 42;
            }
            function unmapped(a=0,b=0) {
                a=4;
                return sum.apply({bias:100},arguments) === 112 &&
                       Reflect.apply(sum,{bias:200},arguments) === 212 &&
                       Reflect.construct(C,arguments).value === 12;
            }
            return mapped(1,2) && unmapped(1,2);
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn arguments_prefix_fallback_preserves_length_conversion_index_order_and_throw() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(() => {
            let log='',called=0,marker={};
            function target(a,b){called++;this.result=a*10+b;return a*10+b;}
            const consumers=[a=>target.apply({},a),a=>Reflect.apply(target,{},a),a=>Reflect.construct(target,a).result];
            for (const consume of consumers) {
                let a=(function(){return arguments})(1,2);
                Object.defineProperty(a,'length',{get(){log+='L';return {valueOf(){log+='N';return 2}}}});
                Object.defineProperty(a,'0',{get(){log+='0';return 4}});
                Object.defineProperty(a,'1',{get(){log+='1';throw marker}});
                log='';
                try{consume(a);return false}catch(e){if(e!==marker||log!=='LN01'||called!==0)return false}
                a=(function(){return arguments})(1,2);delete a[0];
                Object.setPrototypeOf(a,{get 0(){log+='I';return 4}});log='';
                if(consume(a)!==42||log!=='I')return false;
                a=new Proxy((function(){return arguments})(4,2),{get(t,k,r){log+=String(k)+',';return Reflect.get(t,k,r)}});log='';
                if(consume(a)!==42||log!=='length,0,1,')return false;
                called=0;
                a=(function(){return arguments})(1,2);
                Object.defineProperty(a,'length',{get(){throw marker}});
                try{consume(a);return false}catch(e){if(e!==marker||called!==0)return false}
                a=(function(){return arguments})(1,2);a.length=65535;
                Object.defineProperty(a,'0',{get(){log+='X';throw marker}});log='';
                try{consume(a);return false}catch(e){if(!(e instanceof RangeError)||log!==''||called!==0)return false}
            }
            return true;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn arguments_prefix_snapshot_owns_values_after_carrier_drop() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let object = carrier(&mut context, "(function(a){return arguments})({value:42})");
        let carrier_id = object.object_id();
        let Some(NativeConversion::Value(values)) = runtime
            .prepare_fast_array_arguments_jsvalue(context.realm, &object)
            .unwrap()
        else {
            panic!("snapshot")
        };
        let JsValue::Object(value_id) = values[0] else {
            panic!("value")
        };
        drop(object);
        assert!(runtime.0.state.borrow().heap.object(carrier_id).is_err());
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(value_id),
            Ok(1)
        );
        for value in values {
            runtime.release_jsvalue(value).unwrap();
        }
        assert!(runtime.0.state.borrow().heap.object(value_id).is_err());
    }

    #[test]
    fn arguments_prefix_declines_pending_deferred_cleanup_without_draining() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let object = carrier(&mut context, "(function(){return arguments})(1,2)");
        let pending = runtime.new_object(None).unwrap();
        let pending_id = pending.object_id();
        {
            let _borrow = runtime.0.state.borrow();
            drop(pending);
        }
        assert!(runtime.0.deferred_references.has_pending());
        assert!(
            runtime
                .prepare_fast_array_arguments_jsvalue(context.realm, &object)
                .unwrap()
                .is_none()
        );
        assert!(runtime.0.deferred_references.has_pending());
        assert!(runtime.0.state.borrow().heap.object(pending_id).is_ok());
        runtime.drain_deferred_references().unwrap();
        assert_eq!(
            snapshot(&runtime, context.realm, &object),
            vec![Value::Int(1), Value::Int(2)]
        );
        let other = Runtime::new();
        assert!(matches!(
            other.prepare_fast_array_arguments_jsvalue(context.realm, &object),
            Err(RuntimeError::WrongRuntime("object"))
        ));
    }
}

impl RuntimeState {
    /// Materialize pinned QuickJS's complete `js_reflect_funcs` table in its
    /// defining realm. Each method remains an AutoInit native property.
    pub(crate) fn instantiate_reflect_intrinsic(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
    ) -> Result<ObjectId, RuntimeError> {
        let reflect = self.new_ordinary_object_in_realm(poisoned, realm)?;
        let mut result_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(reflect));
        let (state, result_owner) = result_owner.parts();
        for (kind, name, length) in [
            (ReflectKind::Apply, "apply", 3),
            (ReflectKind::Construct, "construct", 2),
            (ReflectKind::DefineProperty, "defineProperty", 3),
            (ReflectKind::DeleteProperty, "deleteProperty", 2),
            (ReflectKind::Get, "get", 2),
            (
                ReflectKind::GetOwnPropertyDescriptor,
                "getOwnPropertyDescriptor",
                2,
            ),
            (ReflectKind::GetPrototypeOf, "getPrototypeOf", 1),
            (ReflectKind::Has, "has", 2),
            (ReflectKind::IsExtensible, "isExtensible", 1),
            (ReflectKind::OwnKeys, "ownKeys", 1),
            (ReflectKind::PreventExtensions, "preventExtensions", 1),
            (ReflectKind::Set, "set", 3),
            (ReflectKind::SetPrototypeOf, "setPrototypeOf", 2),
        ] {
            state.define_native_builtin_auto_init(
                poisoned,
                reflect,
                realm,
                NativeFunctionId::Reflect(kind),
                name,
                length,
                length,
            )?;
        }

        let to_string_tag = state.well_known_symbols[&WellKnownSymbol::ToStringTag];
        {
            let string = state
                .heap
                .allocate_string(JsString::from_static("Reflect"))?;
            let mut producer = OwnedValueGuard::new(state, poisoned, JsValue::String(string));
            let (state, producer) = producer.parts();
            if !state.define_raw_property_with_poison(
                poisoned,
                reflect,
                to_string_tag,
                &crate::engine::object::property::PropertyDescriptor {
                    value: Some(RawValue::String(string)),
                    writable: Some(false),
                    enumerable: Some(false),
                    configurable: Some(true),
                    ..crate::engine::object::property::PropertyDescriptor::new()
                },
            )? {
                return Err(RuntimeError::Invariant(
                    "Reflect toStringTag definition was rejected",
                ));
            }
            state.release_owned_jsvalue(
                poisoned,
                producer.take().expect("intrinsic tag producer"),
            )?;
        }
        let JsValue::Object(object) = result_owner.take().expect("intrinsic factory result") else {
            unreachable!("intrinsic factory allocated an object")
        };
        Ok(object)
    }
}
