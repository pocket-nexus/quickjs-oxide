//! `%Set%`, Set Iterator, and the proposal-era Set methods shipped by QuickJS.
//!
//! The heap owns insertion-ordered records and stable tombstones. This module
//! owns the observable constructor, callback, iterator, set-like protocol,
//! realm, and descriptor behavior. The algorithms intentionally follow the
//! pinned QuickJS `JS_AddIntrinsicMapSet` implementation, including its
//! size-dependent branches and mutation-sensitive ordering.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{
    MapNativeKind, NativeFunctionId, SetIteratorKind, SetNativeKind,
};
use crate::engine::heap::{ContextId, HeapError, ObjectData, ObjectPayload, SetRealmData};
use crate::engine::object::{
    AccessorValue, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
    WellKnownSymbol,
};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation, NativeInvokeOutcome};

pub(crate) mod callback;
pub(crate) mod operations;

impl Runtime {
    pub(crate) fn initialize_set_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        object_prototype: &ObjectRef,
        iterator_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let set_prototype = self.new_object(Some(object_prototype))?;
        let set_iterator_prototype = self.new_object(Some(iterator_prototype))?;

        for (kind, name, length, readable) in [
            (SetNativeKind::Add, "add", 1, 1),
            (SetNativeKind::Has, "has", 1, 1),
            (SetNativeKind::Delete, "delete", 1, 1),
            (SetNativeKind::Clear, "clear", 0, 0),
        ] {
            self.define_native_builtin_auto_init(
                &set_prototype,
                realm,
                NativeFunctionId::Set(kind),
                name,
                length,
                readable,
            )?;
        }
        self.define_native_builtin_getter_on(
            &set_prototype,
            function_prototype,
            realm,
            NativeFunctionId::Set(SetNativeKind::Size),
            "size",
            "get size",
        )?;
        for (kind, name, length, readable) in [
            (SetNativeKind::ForEach, "forEach", 1, 2),
            (SetNativeKind::IsDisjointFrom, "isDisjointFrom", 1, 1),
            (SetNativeKind::IsSubsetOf, "isSubsetOf", 1, 1),
            (SetNativeKind::IsSupersetOf, "isSupersetOf", 1, 1),
            (SetNativeKind::Intersection, "intersection", 1, 1),
            (SetNativeKind::Difference, "difference", 1, 1),
            (
                SetNativeKind::SymmetricDifference,
                "symmetricDifference",
                1,
                1,
            ),
            (SetNativeKind::Union, "union", 1, 1),
        ] {
            self.define_native_builtin_auto_init(
                &set_prototype,
                realm,
                NativeFunctionId::Set(kind),
                name,
                length,
                readable,
            )?;
        }

        self.define_native_builtin_auto_init(
            &set_prototype,
            realm,
            NativeFunctionId::Set(SetNativeKind::Iterator(SetIteratorKind::Value)),
            "values",
            0,
            0,
        )?;
        // QuickJS installs both aliases from the exact values-function object,
        // before `entries`; preserving that order is observable in ownKeys.
        let values_key =
            self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Values)?;
        let values = match self.get_property_in_realm(realm, &set_prototype, &values_key)? {
            Completion::Return(JsValue::Object(id)) => {
                ObjectRef::from_owned_handle(self.clone(), id)
            }
            Completion::Return(value) => {
                self.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "Set.prototype.values was not callable during bootstrap",
                ));
            }
            Completion::Throw(value) => {
                self.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "Set.prototype.values initialization threw during bootstrap",
                ));
            }
        };
        let keys_key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Keys)?;
        self.define_set_alias(&set_prototype, &keys_key, &values)?;
        let iterator_key = PropertyKey::from(self.well_known_symbol(WellKnownSymbol::Iterator));
        self.define_set_alias(&set_prototype, &iterator_key, &values)?;
        self.define_native_builtin_auto_init(
            &set_prototype,
            realm,
            NativeFunctionId::Set(SetNativeKind::Iterator(SetIteratorKind::KeyAndValue)),
            "entries",
            0,
            0,
        )?;
        self.define_set_to_string_tag(&set_prototype, "Set")?;

        self.define_native_builtin_auto_init(
            &set_iterator_prototype,
            realm,
            NativeFunctionId::SetIteratorNext,
            "next",
            0,
            0,
        )?;
        self.define_set_to_string_tag(&set_iterator_prototype, "Set Iterator")?;

        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::Set(SetNativeKind::Constructor),
            1,
            "Set",
            0,
        )?;
        self.define_native_builtin_auto_init(
            constructor.as_object(),
            realm,
            NativeFunctionId::Set(SetNativeKind::GroupBy),
            "groupBy",
            2,
            2,
        )?;
        let species_getter = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::Set(SetNativeKind::Species),
            0,
            "get [Symbol.species]",
            0,
        )?;
        let species = PropertyKey::from(self.well_known_symbol(WellKnownSymbol::Species));
        if !self.define_own_property(
            constructor.as_object(),
            &species,
            &OrdinaryPropertyDescriptor {
                get: DescriptorField::Present(AccessorValue::Callable(species_getter)),
                set: DescriptorField::Present(AccessorValue::Undefined),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "Set species definition was rejected",
            ));
        }

        self.define_function_data_property(
            global_object,
            "Set",
            Value::Object(constructor.as_object().clone()),
            true,
            true,
        )?;
        self.define_constructor_relationship(&constructor, &set_prototype)?;
        self.0.state.borrow_mut().heap.attach_set_intrinsics(
            realm,
            SetRealmData {
                prototype: set_prototype.object_id(),
                iterator_prototype: set_iterator_prototype.object_id(),
            },
        )?;
        Ok(())
    }

    fn define_set_alias(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        if !self.define_raw_property(
            object,
            key,
            &crate::engine::object::property::PropertyDescriptor {
                value: Some(crate::engine::heap::RawValue::Object(value.object_id())),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Default::default()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "Set intrinsic alias definition was rejected",
            ));
        }
        Ok(())
    }

    fn define_set_to_string_tag(
        &self,
        object: &ObjectRef,
        value: &'static str,
    ) -> Result<(), RuntimeError> {
        let key = PropertyKey::from(self.well_known_symbol(WellKnownSymbol::ToStringTag));
        if !self.define_own_property(
            object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(JsString::from_static(value))),
                writable: DescriptorField::Present(false),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "Set intrinsic toStringTag definition was rejected",
            ));
        }
        Ok(())
    }

    pub(in crate::engine::builtins) fn set_realm_data(
        &self,
        realm: ContextId,
    ) -> Result<SetRealmData, RuntimeError> {
        self.0
            .state
            .borrow()
            .heap
            .context(realm)?
            .set
            .ok_or(RuntimeError::Invariant("realm has no Set intrinsics"))
    }

    pub(in crate::engine::builtins) fn new_set_object(
        &self,
        prototype: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Set prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state
            .heap
            .allocate_object(ObjectData::set(shape, Vec::new()))
        {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    fn new_set_in_realm(&self, realm: ContextId) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.set_realm_data(realm)?.prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        self.new_set_object(&prototype)
    }

    pub(crate) fn call_set_native(
        &self,
        realm: ContextId,
        kind: SetNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            self.call_set_native_borrowed(realm, kind, invocation, arguments)
        })
    }
    pub(crate) fn call_set_native_borrowed(
        &self,
        realm: ContextId,
        kind: SetNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        match kind {
            SetNativeKind::Constructor => self.call_set_constructor(realm, invocation, arguments),
            SetNativeKind::Species => self.call_set_species(invocation),
            SetNativeKind::GroupBy => {
                self.call_map_native_borrowed(realm, MapNativeKind::GroupBy, invocation, arguments)
            }
            SetNativeKind::Add => self.call_set_add(realm, invocation, arguments),
            SetNativeKind::Has => self.call_set_has(realm, invocation, arguments),
            SetNativeKind::Delete => self.call_set_delete(realm, invocation, arguments),
            SetNativeKind::Clear => self.call_set_clear(realm, invocation),
            SetNativeKind::Size => self.call_set_size(realm, invocation),
            SetNativeKind::ForEach => self.call_set_for_each(realm, invocation, arguments),
            SetNativeKind::IsDisjointFrom => {
                self.call_set_is_disjoint_from(realm, invocation, arguments)
            }
            SetNativeKind::IsSubsetOf => self.call_set_is_subset_of(realm, invocation, arguments),
            SetNativeKind::IsSupersetOf => {
                self.call_set_is_superset_of(realm, invocation, arguments)
            }
            SetNativeKind::Intersection => self.call_set_intersection(realm, invocation, arguments),
            SetNativeKind::Difference => self.call_set_difference(realm, invocation, arguments),
            SetNativeKind::SymmetricDifference => {
                self.call_set_symmetric_difference(realm, invocation, arguments)
            }
            SetNativeKind::Union => self.call_set_union(realm, invocation, arguments),
            SetNativeKind::Iterator(kind) => {
                self.call_set_iterator_factory(realm, invocation, kind)
            }
        }
    }

    fn call_set_species(&self, invocation: &NativeInvocation) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Getter { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Set species did not receive a getter invocation",
            ));
        };
        Ok(Completion::Return(self.dup_jsvalue(this_value)?))
    }

    fn call_set_constructor(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::iterator::collection::finish(
            self,
            realm,
            super::iterator::collection::CollectionStep::start(
                self,
                realm,
                super::iterator::collection::CollectionKind::Set,
                invocation,
                arguments,
            )?,
        )
    }

    fn set_receiver(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        getter: bool,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        match self.set_receiver_id(realm, invocation, getter)? {
            NativeConversion::Value(id) => Ok(NativeConversion::Value(
                ObjectRef::from_borrowed_handle(self.clone(), id)?,
            )),
            NativeConversion::Throw(value) => Ok(NativeConversion::Throw(value)),
        }
    }

    fn set_receiver_id(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        getter: bool,
    ) -> Result<NativeConversion<crate::engine::heap::ObjectId>, RuntimeError> {
        let this_value = match (getter, invocation) {
            (false, NativeInvocation::Call { this_value })
            | (true, NativeInvocation::Getter { this_value }) => this_value,
            _ => {
                return Err(RuntimeError::Invariant(
                    "Set method received the wrong native invocation",
                ));
            }
        };
        let JsValue::Object(id) = this_value else {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "Set object expected",
            )?));
        };
        let is_set = matches!(
            self.0.state.borrow().heap.object(*id)?.payload,
            ObjectPayload::Set { .. }
        );
        if !is_set {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "Set object expected",
            )?));
        }
        Ok(NativeConversion::Value(*id))
    }

    fn normalized_set_key(value: JsValue) -> JsValue {
        match value {
            JsValue::Float(0.0) => JsValue::Int(0),
            value => value,
        }
    }

    fn find_set_record(
        &self,
        set: &ObjectRef,
        key: &JsValue,
    ) -> Result<Option<usize>, RuntimeError> {
        self.find_set_record_id(set.object_id(), key)
    }
    fn find_set_record_id(
        &self,
        set: crate::engine::heap::ObjectId,
        key: &JsValue,
    ) -> Result<Option<usize>, RuntimeError> {
        let raw_key = match key {
            JsValue::Float(0.0) => crate::engine::heap::RawValue::Int(0),
            _ => key.as_raw(),
        };
        Ok(self.0.state.borrow().heap.set_find_record(set, &raw_key)?)
    }

    fn insert_set_record(&self, set: &ObjectRef, key: JsValue) -> Result<bool, RuntimeError> {
        let result = self.insert_set_record_borrowed(set.object_id(), &key);
        let released = self.release_jsvalue(key);
        let result = result?;
        released?;
        Ok(result)
    }
    // The native invocation/argv keeps receiver and key alive until commit.
    fn insert_set_record_borrowed(
        &self,
        set: crate::engine::heap::ObjectId,
        key: &JsValue,
    ) -> Result<bool, RuntimeError> {
        let raw_key = match key {
            JsValue::Float(0.0) => crate::engine::heap::RawValue::Int(0),
            _ => key.as_raw(),
        };
        let mut state = self.0.state.borrow_mut();
        if state.heap.set_find_record(set, &raw_key)?.is_some() {
            return Ok(false);
        }
        let retained = state.retain_raw_value_atoms([&raw_key])?;
        let cleanup = match state.heap.set_insert_record(set, raw_key) {
            Ok(cleanup) => cleanup,
            Err(error) => {
                state.release_atoms(retained)?;
                return Err(error.into());
            }
        };
        state.apply_cleanup(cleanup)?;
        Ok(true)
    }
    fn delete_set_record(&self, set: &ObjectRef, key: JsValue) -> Result<bool, RuntimeError> {
        let result = self.delete_set_record_borrowed(set.object_id(), &key);
        let released = self.release_jsvalue(key);
        let result = result?;
        released?;
        Ok(result)
    }
    fn delete_set_record_borrowed(
        &self,
        set: crate::engine::heap::ObjectId,
        key: &JsValue,
    ) -> Result<bool, RuntimeError> {
        let raw_key = match key {
            JsValue::Float(0.0) => crate::engine::heap::RawValue::Int(0),
            _ => key.as_raw(),
        };
        let mut state = self.0.state.borrow_mut();
        let Some(index) = state.heap.set_find_record(set, &raw_key)? else {
            return Ok(false);
        };
        let cleanup = state.heap.set_delete_record(set, index)?;
        state.apply_cleanup(cleanup)?;
        Ok(true)
    }

    fn set_size_value(&self, set: &ObjectRef) -> Result<usize, RuntimeError> {
        Ok(self.0.state.borrow().heap.set_size(set.object_id())?)
    }

    fn next_live_set_record(
        &self,
        set: &ObjectRef,
        index: &mut usize,
    ) -> Result<Option<(usize, JsValue)>, RuntimeError> {
        let record = self
            .0
            .state
            .borrow()
            .heap
            .set_records(set.object_id())?
            .next_at_or_after(*index)
            .map(|(id, record)| (id, record.key.clone()));
        let Some((record_index, key)) = record else {
            return Ok(None);
        };
        *index = record_index
            .checked_add(1)
            .ok_or(RuntimeError::Invariant("Set record index overflowed"))?;
        Ok(Some((
            record_index,
            self.dup_jsvalue(
                &JsValue::from_raw(key.clone()).ok_or(RuntimeError::Invariant(
                    "stored collection value is uninitialized",
                ))?,
            )?,
        )))
    }

    fn next_live_set_value(
        &self,
        set: &ObjectRef,
        index: &mut usize,
    ) -> Result<Option<JsValue>, RuntimeError> {
        Ok(self
            .next_live_set_record(set, index)?
            .map(|(_, value)| value))
    }

    fn call_set_add(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Set.prototype.add value argv was not padded",
        ))?;
        self.insert_set_record_borrowed(set, value)?;
        Ok(Completion::Return(self.dup_jsvalue(&JsValue::Object(set))?))
    }

    fn call_set_has(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Set.prototype.has value argv was not padded",
        ))?;
        let has = self.find_set_record_id(set, value)?.is_some();
        Ok(Completion::Return(JsValue::Bool(has)))
    }

    fn call_set_delete(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Set.prototype.delete value argv was not padded",
        ))?;
        Ok(Completion::Return(JsValue::Bool(
            self.delete_set_record_borrowed(set, value)?,
        )))
    }

    fn call_set_clear(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };

        let mut state = self.0.state.borrow_mut();
        let cleanup = state.heap.set_clear(set)?;
        state.apply_cleanup(cleanup)?;
        Ok(Completion::Return(JsValue::Undefined))
    }

    fn call_set_size(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver_id(realm, invocation, true)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };

        Ok(Completion::Return(JsValue::Int(
            self.0.state.borrow().heap.set_size(set)? as i32,
        )))
    }

    fn call_set_for_each(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        callback::finish(
            self,
            realm,
            callback::EachStep::start(self, realm, invocation, arguments)?,
        )
    }

    fn new_set_iterator(
        &self,
        realm: ContextId,
        set: &ObjectRef,
        kind: SetIteratorKind,
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.set_realm_data(realm)?.iterator_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let iterator = match state.heap.allocate_object(ObjectData::set_iterator(
            shape,
            Vec::new(),
            set.object_id(),
            kind,
        )) {
            Ok(iterator) => iterator,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), iterator))
    }

    fn call_set_iterator_factory(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        kind: SetIteratorKind,
    ) -> Result<Completion, RuntimeError> {
        let set = match self.set_receiver(realm, invocation, false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        Ok(Completion::Return(self.into_jsvalue(Value::Object(
            self.new_set_iterator(realm, &set, kind)?,
        ))?))
    }

    pub(crate) fn call_set_iterator_next(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        match self.call_set_iterator_next_raw(realm, invocation)? {
            NativeInvokeOutcome::Completion(completion) => Ok(completion),
            NativeInvokeOutcome::IteratorNextRaw { value, done } => {
                Ok(Completion::Return(self.into_jsvalue(Value::Object(
                    self.new_iterator_result_jsvalue(realm, value, done)?,
                ))?))
            }
        }
    }

    pub(crate) fn call_set_iterator_next_raw(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Set Iterator next did not receive an iterator-next invocation",
            ));
        };
        let JsValue::Object(iterator) = this_value else {
            self.release_jsvalue(this_value)?;
            return Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "Set Iterator object expected",
                )?,
            )));
        };
        let iterator = ObjectRef::from_owned_handle(self.clone(), iterator);
        let iterator_id = iterator.object_id();
        let state = self
            .0
            .state
            .borrow_mut()
            .heap
            .begin_set_iterator_next(iterator_id);
        let (set, mut index, kind) = match state {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => {
                return Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                    self.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "Set Iterator object expected",
                    )?,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        let Some(set_id) = set else {
            return Ok(NativeInvokeOutcome::IteratorNextRaw {
                value: JsValue::Undefined,
                done: true,
            });
        };
        let record = self
            .0
            .state
            .borrow()
            .heap
            .set_records(set_id)?
            .next_at_or_after(index)
            .map(|(id, record)| (id, record.key.clone()));
        let Some((record_index, key)) = record else {
            let mut state = self.0.state.borrow_mut();
            let cleanup = state.heap.finish_set_iterator(iterator_id)?;
            state.apply_cleanup(cleanup)?;
            return Ok(NativeInvokeOutcome::IteratorNextRaw {
                value: JsValue::Undefined,
                done: true,
            });
        };
        index = record_index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Set Iterator record index overflowed",
        ))?;
        self.0
            .state
            .borrow_mut()
            .heap
            .set_set_iterator_index(iterator_id, index)?;
        self.0
            .state
            .borrow_mut()
            .heap
            .set_set_iterator_current(iterator_id, record_index)?;
        let value = self.dup_jsvalue(
            &JsValue::from_raw(key).ok_or(RuntimeError::Invariant("Set key is uninitialized"))?,
        )?;
        let value = match kind {
            SetIteratorKind::Value => value,
            SetIteratorKind::KeyAndValue => {
                let key = value;
                let second = match self.dup_jsvalue(&key) {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = self.release_jsvalue(key);
                        return Err(error);
                    }
                };
                JsValue::Object(
                    self.new_array_from_values_jsvalue(realm, vec![key, second])?
                        .into_handle(),
                )
            }
        };
        Ok(NativeInvokeOutcome::IteratorNextRaw { value, done: false })
    }

    fn copy_set_in_realm(
        &self,
        realm: ContextId,
        source: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let copy = self.new_set_in_realm(realm)?;
        let mut index = 0_usize;
        while let Some(value) = self.next_live_set_value(source, &mut index)? {
            self.insert_set_record(&copy, value)?;
        }
        Ok(copy)
    }

    fn call_set_is_disjoint_from(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Disjoint,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_is_subset_of(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Subset,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_is_superset_of(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Superset,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_intersection(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Intersection,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_difference(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Difference,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_symmetric_difference(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::SymmetricDifference,
                invocation,
                arguments,
            )?,
        )
    }

    fn call_set_union(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operations::finish(
            self,
            realm,
            operations::SetStep::start(
                self,
                realm,
                operations::SetOperation::Union,
                invocation,
                arguments,
            )?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_backed_symbol_atoms_return_after_set_mutations() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context
            .eval(
                r#"(function(){
                    var set = new Set();
                    var value = Symbol("set-value");
                    set.add(value);
                    set.add(value);
                    set.delete(value);
                    set.add(Symbol("set-clear-value"));
                    set.clear();
                })"#,
            )
            .unwrap()
        else {
            panic!("Set Symbol ownership probe was not callable");
        };
        let function = runtime.as_callable(&function).unwrap().unwrap();

        drop(
            context
                .call(&function, Value::Undefined, &[])
                .expect("warm Set Symbol ownership probe"),
        );
        let baseline = runtime.test_atom_count();
        for _ in 0..3 {
            drop(
                context
                    .call(&function, Value::Undefined, &[])
                    .expect("repeat Set Symbol ownership probe"),
            );
            assert_eq!(runtime.test_atom_count(), baseline);
        }
    }
}
