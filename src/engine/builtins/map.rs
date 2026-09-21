//! `%Map%`, Map Iterator, and strong ordered-record semantics.
//!
//! This follows the pinned QuickJS `JS_AddIntrinsicMapSet`/`js_map_*`
//! boundary.  The heap owns the insertion-ordered records and tombstones;
//! this module owns all observable iteration, callback, realm, and descriptor
//! behavior.  `Set` and the weak collections deliberately remain separate
//! follow-up slices rather than weakening the Map brand here.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::builtins::native::{MapIteratorKind, MapNativeKind, NativeFunctionId};
use crate::engine::heap::{
    ContextId, HeapError, MapRealmData, ObjectData, ObjectPayload, RawValue,
};
use crate::engine::object::{
    AccessorValue, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
    WellKnownSymbol,
};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation, NativeInvokeOutcome};
#[cfg(test)]
use crate::engine::vm::frames::ActiveCollectionRecord;

pub(crate) mod callback;

impl Runtime {
    pub(crate) fn initialize_map_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        object_prototype: &ObjectRef,
        iterator_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let map_prototype = self.new_object(Some(object_prototype))?;
        let map_iterator_prototype = self.new_object(Some(iterator_prototype))?;

        for (kind, name, length, readable) in [
            (MapNativeKind::Set, "set", 2, 2),
            (MapNativeKind::Get, "get", 1, 1),
            (MapNativeKind::GetOrInsert, "getOrInsert", 2, 2),
            (
                MapNativeKind::GetOrInsertComputed,
                "getOrInsertComputed",
                2,
                2,
            ),
            (MapNativeKind::Has, "has", 1, 1),
            (MapNativeKind::Delete, "delete", 1, 1),
            (MapNativeKind::Clear, "clear", 0, 0),
        ] {
            self.define_native_builtin_auto_init(
                &map_prototype,
                realm,
                NativeFunctionId::Map(kind),
                name,
                length,
                readable,
            )?;
        }
        self.define_native_builtin_getter_on(
            &map_prototype,
            function_prototype,
            realm,
            NativeFunctionId::Map(MapNativeKind::Size),
            "size",
            "get size",
        )?;
        for (kind, name, length, readable) in [
            (MapNativeKind::ForEach, "forEach", 1, 2),
            (
                MapNativeKind::Iterator(MapIteratorKind::Value),
                "values",
                0,
                0,
            ),
            (MapNativeKind::Iterator(MapIteratorKind::Key), "keys", 0, 0),
            (
                MapNativeKind::Iterator(MapIteratorKind::KeyAndValue),
                "entries",
                0,
                0,
            ),
        ] {
            self.define_native_builtin_auto_init(
                &map_prototype,
                realm,
                NativeFunctionId::Map(kind),
                name,
                length,
                readable,
            )?;
        }

        // QuickJS's alias table preserves the exact entries-function identity.
        let entries_key =
            self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Entries)?;
        let entries = match self.get_property_in_realm(realm, &map_prototype, &entries_key)? {
            Completion::Return(JsValue::Object(id)) => {
                ObjectRef::from_owned_handle(self.clone(), id)
            }
            Completion::Return(value) => {
                self.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "Map.prototype.entries was not callable during bootstrap",
                ));
            }
            Completion::Throw(value) => {
                self.release_jsvalue(value)?;
                return Err(RuntimeError::Invariant(
                    "Map.prototype.entries initialization threw during bootstrap",
                ));
            }
        };
        let iterator_key = PropertyKey::from(self.well_known_symbol(WellKnownSymbol::Iterator));
        if !self.define_raw_property(
            &map_prototype,
            &iterator_key,
            &crate::engine::object::property::PropertyDescriptor {
                value: Some(crate::engine::heap::RawValue::Object(entries.object_id())),
                writable: Some(true),
                enumerable: Some(false),
                configurable: Some(true),
                ..Default::default()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "Map iterator alias definition was rejected",
            ));
        }
        self.define_to_string_tag(&map_prototype, "Map")?;

        self.define_native_builtin_auto_init(
            &map_iterator_prototype,
            realm,
            NativeFunctionId::MapIteratorNext,
            "next",
            0,
            0,
        )?;
        self.define_to_string_tag(&map_iterator_prototype, "Map Iterator")?;

        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::Map(MapNativeKind::Constructor),
            1,
            "Map",
            0,
        )?;
        self.define_native_builtin_auto_init(
            constructor.as_object(),
            realm,
            NativeFunctionId::Map(MapNativeKind::GroupBy),
            "groupBy",
            2,
            2,
        )?;
        let species_getter = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::Map(MapNativeKind::Species),
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
                "Map species definition was rejected",
            ));
        }

        self.define_function_data_property(
            global_object,
            "Map",
            Value::Object(constructor.as_object().clone()),
            true,
            true,
        )?;
        self.define_constructor_relationship(&constructor, &map_prototype)?;
        self.0.state.borrow_mut().heap.attach_map_intrinsics(
            realm,
            MapRealmData {
                prototype: map_prototype.object_id(),
                iterator_prototype: map_iterator_prototype.object_id(),
            },
        )?;
        Ok(())
    }

    fn define_to_string_tag(
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
                "Map intrinsic toStringTag definition was rejected",
            ));
        }
        Ok(())
    }

    pub(in crate::engine::builtins) fn map_realm_data(
        &self,
        realm: ContextId,
    ) -> Result<MapRealmData, RuntimeError> {
        self.0
            .state
            .borrow()
            .heap
            .context(realm)?
            .map
            .ok_or(RuntimeError::Invariant("realm has no Map intrinsics"))
    }

    pub(in crate::engine::builtins) fn new_map_object(
        &self,
        prototype: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Map prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state
            .heap
            .allocate_object(ObjectData::map(shape, Vec::new()))
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

    pub(in crate::engine::builtins) fn new_map_in_realm(
        &self,
        realm: ContextId,
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.map_realm_data(realm)?.prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        self.new_map_object(&prototype)
    }

    pub(crate) fn call_map_native(
        &self,
        realm: ContextId,
        kind: MapNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            self.call_map_native_borrowed(realm, kind, invocation, arguments)
        })
    }
    pub(crate) fn call_map_native_borrowed(
        &self,
        realm: ContextId,
        kind: MapNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        match kind {
            MapNativeKind::Constructor => self.call_map_constructor(realm, invocation, arguments),
            MapNativeKind::Species => self.call_map_species(invocation),
            MapNativeKind::GroupBy => self.call_map_group_by(realm, invocation, arguments),
            MapNativeKind::Set => self.call_map_set(realm, invocation, arguments),
            MapNativeKind::Get => self.call_map_get(realm, invocation, arguments),
            MapNativeKind::GetOrInsert => {
                self.call_map_get_or_insert(realm, invocation, arguments, false)
            }
            MapNativeKind::GetOrInsertComputed => {
                self.call_map_get_or_insert(realm, invocation, arguments, true)
            }
            MapNativeKind::Has => self.call_map_has(realm, invocation, arguments),
            MapNativeKind::Delete => self.call_map_delete(realm, invocation, arguments),
            MapNativeKind::Clear => self.call_map_clear(realm, invocation),
            MapNativeKind::Size => self.call_map_size(realm, invocation),
            MapNativeKind::ForEach => self.call_map_for_each(realm, invocation, arguments),
            MapNativeKind::Iterator(kind) => {
                self.call_map_iterator_factory(realm, invocation, kind)
            }
        }
    }

    fn call_map_species(&self, invocation: &NativeInvocation) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Getter { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Map species did not receive a getter invocation",
            ));
        };
        Ok(Completion::Return(self.dup_jsvalue(this_value)?))
    }

    fn call_map_constructor(
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
                super::iterator::collection::CollectionKind::Map,
                invocation,
                arguments,
            )?,
        )
    }

    fn map_receiver(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        getter: bool,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        match self.map_receiver_id(realm, invocation, getter)? {
            NativeConversion::Value(id) => Ok(NativeConversion::Value(
                ObjectRef::from_borrowed_handle(self.clone(), id)?,
            )),
            NativeConversion::Throw(value) => Ok(NativeConversion::Throw(value)),
        }
    }

    fn map_receiver_id(
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
                    "Map method received the wrong native invocation",
                ));
            }
        };
        let JsValue::Object(id) = this_value else {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "Map object expected",
            )?));
        };
        let is_map = matches!(
            self.0.state.borrow().heap.object(*id)?.payload,
            ObjectPayload::Map { .. }
        );
        if !is_map {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "Map object expected",
            )?));
        }
        Ok(NativeConversion::Value(*id))
    }

    pub(in crate::engine::builtins) fn normalized_map_key(value: JsValue) -> JsValue {
        match value {
            JsValue::Float(0.0) => JsValue::Int(0),
            value => value,
        }
    }

    pub(in crate::engine::builtins) fn find_map_record(
        &self,
        map: &ObjectRef,
        key: &JsValue,
    ) -> Result<Option<(usize, RawValue)>, RuntimeError> {
        self.find_map_record_id(map.object_id(), key)
    }
    fn find_map_record_id(
        &self,
        map: crate::engine::heap::ObjectId,
        key: &JsValue,
    ) -> Result<Option<(usize, RawValue)>, RuntimeError> {
        let raw_key = match key {
            JsValue::Float(0.0) => crate::engine::heap::RawValue::Int(0),
            _ => key.as_raw(),
        };
        let state = self.0.state.borrow();
        let heap = &state.heap;
        let Some(index) = heap.map_find_record(map, &raw_key)? else {
            return Ok(None);
        };
        let value = heap
            .map_records(map)?
            .get(index)
            .expect("indexed Map record exists")
            .value
            .clone();
        Ok(Some((index, value)))
    }

    pub(in crate::engine::builtins) fn set_map_record(
        &self,
        map: &ObjectRef,
        key: JsValue,
        value: JsValue,
    ) -> Result<(), RuntimeError> {
        let result = self.set_map_record_borrowed(map.object_id(), &key, &value);
        let key_release = self.release_jsvalue(key);
        let value_release = self.release_jsvalue(value);
        result?;
        key_release?;
        value_release
    }

    // Native argv owns these edges throughout this non-callback transaction.
    fn set_map_record_borrowed(
        &self,
        map: crate::engine::heap::ObjectId,
        key: &JsValue,
        value: &JsValue,
    ) -> Result<(), RuntimeError> {
        let raw_key = match key {
            JsValue::Float(0.0) => RawValue::Int(0),
            _ => key.as_raw(),
        };
        let raw_value = value.as_raw();
        let mut state = self.0.state.borrow_mut();
        let existing = state.heap.map_find_record(map, &raw_key)?;
        let retained = if existing.is_some() {
            state.retain_raw_value_atoms([&raw_value])?
        } else {
            state.retain_raw_value_atoms([&raw_key, &raw_value])?
        };
        let result = if let Some(index) = existing {
            state.heap.map_replace_record_value(map, index, raw_value)
        } else {
            state.heap.map_insert_record(map, raw_key, raw_value)
        };
        let cleanup = match result {
            Ok(cleanup) => cleanup,
            Err(error) => {
                state.release_atoms(retained)?;
                return Err(error.into());
            }
        };
        state.apply_cleanup(cleanup)
    }

    fn delete_map_record(&self, map: &ObjectRef, key: JsValue) -> Result<bool, RuntimeError> {
        let result = self.delete_map_record_borrowed(map.object_id(), &key);
        let released = self.release_jsvalue(key);
        let result = result?;
        released?;
        Ok(result)
    }
    fn delete_map_record_borrowed(
        &self,
        map: crate::engine::heap::ObjectId,
        key: &JsValue,
    ) -> Result<bool, RuntimeError> {
        let Some((index, _)) = self.find_map_record_id(map, key)? else {
            return Ok(false);
        };
        let mut state = self.0.state.borrow_mut();
        let cleanup = state.heap.map_delete_record(map, index)?;
        state.apply_cleanup(cleanup)?;
        Ok(true)
    }

    fn call_map_set(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let key = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Map.prototype.set key argv was not padded",
        ))?;
        let value = arguments.readable.get(1).ok_or(RuntimeError::Invariant(
            "Map.prototype.set value argv was not padded",
        ))?;
        self.set_map_record_borrowed(map, key, value)?;
        Ok(Completion::Return(self.dup_jsvalue(&JsValue::Object(map))?))
    }

    fn call_map_get(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let key = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Map.prototype.get key argv was not padded",
        ))?;
        let value = match self.find_map_record_id(map, key)? {
            Some((_, value)) => self.dup_jsvalue(
                &JsValue::from_raw(value)
                    .ok_or(RuntimeError::Invariant("Map value is uninitialized"))?,
            )?,
            None => JsValue::Undefined,
        };
        Ok(Completion::Return(value))
    }

    fn call_map_has(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let key = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Map.prototype.has key argv was not padded",
        ))?;
        let has = self.find_map_record_id(map, key)?.is_some();
        Ok(Completion::Return(JsValue::Bool(has)))
    }

    fn call_map_delete(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let key = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Map.prototype.delete key argv was not padded",
        ))?;
        Ok(Completion::Return(JsValue::Bool(
            self.delete_map_record_borrowed(map, key)?,
        )))
    }

    fn call_map_clear(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };

        let mut state = self.0.state.borrow_mut();
        let cleanup = state.heap.map_clear(map)?;
        state.apply_cleanup(cleanup)?;
        Ok(Completion::Return(JsValue::Undefined))
    }

    fn call_map_size(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver_id(realm, invocation, true)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };

        let size = self.0.state.borrow().heap.map_size(map)?;
        Ok(Completion::Return(JsValue::Int(size as i32)))
    }

    fn call_map_get_or_insert(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
        computed: bool,
    ) -> Result<Completion, RuntimeError> {
        callback::finish(
            self,
            realm,
            callback::CallbackStep::start(
                self,
                realm,
                callback::CallbackKind::Insert { computed },
                invocation,
                arguments,
            )?,
        )
    }

    fn call_map_for_each(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        callback::finish(
            self,
            realm,
            callback::CallbackStep::start(
                self,
                realm,
                callback::CallbackKind::Each,
                invocation,
                arguments,
            )?,
        )
    }

    fn new_map_iterator(
        &self,
        realm: ContextId,
        map: &ObjectRef,
        kind: MapIteratorKind,
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.map_realm_data(realm)?.iterator_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let iterator = match state.heap.allocate_object(ObjectData::map_iterator(
            shape,
            Vec::new(),
            map.object_id(),
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

    fn call_map_iterator_factory(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        kind: MapIteratorKind,
    ) -> Result<Completion, RuntimeError> {
        let map = match self.map_receiver(realm, invocation, false)? {
            NativeConversion::Value(map) => map,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        Ok(Completion::Return(self.into_jsvalue(Value::Object(
            self.new_map_iterator(realm, &map, kind)?,
        ))?))
    }

    pub(crate) fn call_map_iterator_next(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        match self.call_map_iterator_next_raw(realm, invocation)? {
            NativeInvokeOutcome::Completion(completion) => Ok(completion),
            NativeInvokeOutcome::IteratorNextRaw { value, done } => {
                Ok(Completion::Return(self.into_jsvalue(Value::Object(
                    self.new_iterator_result_jsvalue(realm, value, done)?,
                ))?))
            }
        }
    }

    pub(crate) fn call_map_iterator_next_raw(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Map Iterator next did not receive an iterator-next invocation",
            ));
        };
        let JsValue::Object(iterator) = this_value else {
            self.release_jsvalue(this_value)?;
            return Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "Map Iterator object expected",
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
            .begin_map_iterator_next(iterator_id);
        let (map, mut index, kind) = match state {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => {
                return Ok(NativeInvokeOutcome::Completion(Completion::Throw(
                    self.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "Map Iterator object expected",
                    )?,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        let Some(map_id) = map else {
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
            .map_records(map_id)?
            .next_at_or_after(index)
            .map(|(id, record)| (id, record.key.clone(), record.value.clone()));
        let Some((record_index, key, value)) = record else {
            let mut state = self.0.state.borrow_mut();
            let cleanup = state.heap.finish_map_iterator(iterator_id)?;
            state.apply_cleanup(cleanup)?;
            return Ok(NativeInvokeOutcome::IteratorNextRaw {
                value: JsValue::Undefined,
                done: true,
            });
        };
        index = record_index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Map Iterator record index overflowed",
        ))?;
        self.0
            .state
            .borrow_mut()
            .heap
            .set_map_iterator_index(iterator_id, index)?;
        self.0
            .state
            .borrow_mut()
            .heap
            .set_map_iterator_current(iterator_id, record_index)?;
        let value = match kind {
            MapIteratorKind::Key => self.dup_jsvalue(
                &JsValue::from_raw(key)
                    .ok_or(RuntimeError::Invariant("Map key is uninitialized"))?,
            )?,
            MapIteratorKind::Value => {
                self.dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                    RuntimeError::Invariant("stored collection value is uninitialized"),
                )?)?
            }
            MapIteratorKind::KeyAndValue => {
                let key = self.dup_jsvalue(
                    &JsValue::from_raw(key)
                        .ok_or(RuntimeError::Invariant("Map key is uninitialized"))?,
                )?;
                let value = (|| {
                    self.dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                        RuntimeError::Invariant("stored collection value is uninitialized"),
                    )?)
                })();
                let value = match value {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = self.release_jsvalue(key);
                        return Err(error);
                    }
                };
                JsValue::Object(
                    self.new_array_from_values_jsvalue(realm, vec![key, value])?
                        .into_handle(),
                )
            }
        };
        Ok(NativeInvokeOutcome::IteratorNextRaw { value, done: false })
    }

    fn call_map_group_by(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        super::object::iteration::finish(
            self,
            realm,
            super::object::iteration::IterationStep::start(
                self,
                realm,
                super::object::iteration::IterationKind::MapGroup,
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
    fn active_collection_record_guard_is_lifo_and_panic_safe() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let map = runtime.new_map_in_realm(context.realm).unwrap();
        let record = ActiveCollectionRecord::Map {
            object: map.object_id(),
            index: 0,
        };

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _active_record = runtime.push_active_collection_record(record);
            panic!("active collection record unwind probe");
        }));
        assert!(unwind.is_err());
        assert!(
            runtime
                .0
                .state
                .borrow()
                .active_collection_records
                .is_empty()
        );

        let outer = runtime.push_active_collection_record(record);
        let inner = runtime.push_active_collection_record(record);
        assert!(matches!(
            outer.finish(),
            Err(RuntimeError::Invariant(
                "active collection record stack was not restored in LIFO order"
            ))
        ));
        drop(inner);
        assert!(
            runtime
                .0
                .state
                .borrow()
                .active_collection_records
                .is_empty()
        );
    }

    #[test]
    fn table_backed_symbol_atoms_return_after_map_mutations() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context
            .eval(
                r#"(function(){
                    var map = new Map();
                    var key = Symbol("map-key");
                    map.set(key, Symbol("map-first-value"));
                    map.set(key, Symbol("map-replacement-value"));
                    map.delete(key);
                    map.set(Symbol("map-clear-key"), Symbol("map-clear-value"));
                    map.clear();
                })"#,
            )
            .unwrap()
        else {
            panic!("Map Symbol ownership probe was not callable");
        };
        let function = runtime.as_callable(&function).unwrap().unwrap();

        drop(
            context
                .call(&function, Value::Undefined, &[])
                .expect("warm Map Symbol ownership probe"),
        );
        let baseline = runtime.test_atom_count();
        for _ in 0..3 {
            drop(
                context
                    .call(&function, Value::Undefined, &[])
                    .expect("repeat Map Symbol ownership probe"),
            );
            assert_eq!(runtime.test_atom_count(), baseline);
        }
    }
}
