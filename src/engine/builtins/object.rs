//! Object constructor and prototype intrinsics.

use crate::engine::api::error::{ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::PropertyKeyKind;
use crate::engine::object::builtin_properties::{BuiltinEntry, BuiltinKey, method};

use crate::engine::builtins::native::{
    NativeFunctionId, ObjectAccessorKind, ObjectExtensibilityKind, ObjectIntegrityKind,
    ObjectKeysKind, ObjectOwnPropertyKeysKind, PrimitiveKind,
};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ContextId, ObjectId, ObjectPayload, PrimitiveObjectData};
use crate::engine::object::operations::{ArrayOwnKey, InternalDefineResult};
use crate::engine::object::{ObjectRef, PropertyKey, SymbolRef};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};
use std::cell::Cell;

pub(super) mod constructor;
pub(crate) mod copy;
pub(super) mod definitions;
pub(super) mod iteration;
pub(super) mod predicate;
pub(super) mod property;
pub(super) mod prototype;
pub(super) mod string;

#[cfg(test)]
mod tests;

pub(crate) enum ObjectIteratorStep {
    Yield(JsValue),
    Done,
    Throw(JsValue),
}

impl Runtime {
    /// QuickJS `JS_ToPrimitive(..., HINT_FORCE_ORDINARY)`: probe the ordinary
    /// conversion methods without consulting `Symbol.toPrimitive`. Date's
    /// standard exotic method delegates here after translating its hint.
    /// QuickJS `js_object_groupBy(..., is_map = 0)`.
    ///
    /// The upstream routine deliberately closes the iterator only after an
    /// abrupt callback, property-key conversion, or element-count check. An
    /// abrupt iterator step or group-Array append takes the ordinary exception
    /// exit instead, so those branches must remain separate here.
    pub(crate) fn call_object_group_by(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

        self.call_object_group_by_with_element_limit(realm, invocation, arguments, MAX_SAFE_INTEGER)
    }

    fn call_object_group_by_with_element_limit(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
        element_limit: u64,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            iteration::finish(
                self,
                realm,
                iteration::IterationStep::start_with_limit(
                    self,
                    realm,
                    iteration::IterationKind::Group,
                    invocation,
                    arguments,
                    element_limit,
                )?,
            )
        })
    }

    /// QuickJS `js_object_fromEntries`.
    ///
    /// Unlike `Object.groupBy`, the pinned implementation allocates its
    /// defining-realm result before touching the iterable and closes an
    /// acquired iterator after every subsequent abrupt completion, including
    /// `next` lookup and iterator-step failures. `JS_IteratorClose(..., TRUE)`
    /// always restores the original pending exception, so close failures are
    /// deliberately ignored by `close_iterator_preserving_throw`.
    pub(crate) fn call_object_from_entries(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            iteration::finish(
                self,
                realm,
                iteration::IterationStep::start(
                    self,
                    realm,
                    iteration::IterationKind::Entries,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    /// QuickJS `js_object_hasOwn`.
    ///
    /// The static method converts its target before its key (the reverse of
    /// `Object.prototype.hasOwnProperty`) and then performs the descriptor-free
    /// `JS_GetOwnPropertyInternal` presence check. The local property kernel
    /// preserves that check without materializing AutoInit payloads.
    pub(crate) fn call_object_has_own(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            predicate::finish(
                self,
                realm,
                predicate::PredicateStep::start(
                    self,
                    realm,
                    predicate::PredicateKind::HasOwn,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn initialize_object_prototype_intrinsics(
        &self,
        realm: ContextId,
        object_prototype: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        use ObjectAccessorKind as A;
        const PROTOTYPE: &[BuiltinEntry] = &[
            method(F::ObjectPrototypeToString, "toString", 0, 0),
            method(F::ObjectPrototypeToLocaleString, "toLocaleString", 0, 0),
            method(F::ObjectPrototypeValueOf, "valueOf", 0, 0),
            method(F::ObjectPrototypeHasOwnProperty, "hasOwnProperty", 1, 1),
            method(F::ObjectPrototypeIsPrototypeOf, "isPrototypeOf", 1, 1),
            method(
                F::ObjectPrototypePropertyIsEnumerable,
                "propertyIsEnumerable",
                1,
                1,
            ),
            BuiltinEntry::Accessor {
                key: BuiltinKey::Name("__proto__"),
                get: Some((F::ObjectPrototypeProtoGetter, "get __proto__")),
                set: Some((F::ObjectPrototypeProtoSetter, "set __proto__")),
            },
            method(
                F::ObjectPrototypeDefineAccessor(A::Getter),
                "__defineGetter__",
                2,
                2,
            ),
            method(
                F::ObjectPrototypeDefineAccessor(A::Setter),
                "__defineSetter__",
                2,
                2,
            ),
            method(
                F::ObjectPrototypeLookupAccessor(A::Getter),
                "__lookupGetter__",
                1,
                1,
            ),
            method(
                F::ObjectPrototypeLookupAccessor(A::Setter),
                "__lookupSetter__",
                1,
                1,
            ),
        ];
        self.define_builtin_entries(object_prototype, realm, PROTOTYPE)
    }

    pub(crate) fn initialize_object_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        object_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use NativeFunctionId as F;
        use ObjectExtensibilityKind as Ext;
        use ObjectIntegrityKind as Integrity;
        use ObjectKeysKind as Keys;
        use ObjectOwnPropertyKeysKind as OwnKeys;
        const CONSTRUCTOR: &[BuiltinEntry] = &[
            method(F::ObjectCreate, "create", 2, 2),
            method(F::ObjectGetPrototypeOf, "getPrototypeOf", 1, 1),
            method(F::ObjectSetPrototypeOf, "setPrototypeOf", 2, 2),
            method(F::ObjectDefineProperty, "defineProperty", 3, 3),
            method(F::ObjectDefineProperties, "defineProperties", 2, 2),
            method(
                F::ObjectGetOwnPropertyKeys(OwnKeys::Names),
                "getOwnPropertyNames",
                1,
                1,
            ),
            method(
                F::ObjectGetOwnPropertyKeys(OwnKeys::Symbols),
                "getOwnPropertySymbols",
                1,
                1,
            ),
            method(F::ObjectGroupBy, "groupBy", 2, 2),
            method(F::ObjectKeys(Keys::Keys), "keys", 1, 1),
            method(F::ObjectKeys(Keys::Values), "values", 1, 1),
            method(F::ObjectKeys(Keys::Entries), "entries", 1, 1),
            method(
                F::ObjectExtensibility(Ext::IsExtensible),
                "isExtensible",
                1,
                1,
            ),
            method(
                F::ObjectExtensibility(Ext::PreventExtensions),
                "preventExtensions",
                1,
                1,
            ),
            method(
                F::ObjectGetOwnPropertyDescriptor,
                "getOwnPropertyDescriptor",
                2,
                2,
            ),
            method(
                F::ObjectGetOwnPropertyDescriptors,
                "getOwnPropertyDescriptors",
                1,
                1,
            ),
            method(F::ObjectIs, "is", 2, 2),
            method(F::ObjectAssign, "assign", 2, 2),
            method(F::ObjectIntegrity(Integrity::Seal), "seal", 1, 1),
            method(F::ObjectIntegrity(Integrity::Freeze), "freeze", 1, 1),
            method(F::ObjectIntegrity(Integrity::IsSealed), "isSealed", 1, 1),
            method(F::ObjectIntegrity(Integrity::IsFrozen), "isFrozen", 1, 1),
            method(F::ObjectFromEntries, "fromEntries", 1, 1),
            method(F::ObjectHasOwn, "hasOwn", 2, 2),
        ];
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            F::ObjectConstructor,
            1,
            "Object",
            1,
        )?;
        self.define_builtin_entries(constructor.as_object(), realm, CONSTRUCTOR)?;
        self.define_global_function(global_object, "Object", &constructor)?;
        self.define_constructor_relationship(&constructor, object_prototype)
    }

    fn object_default_to_string_tag(
        &self,
        realm: ContextId,
        object: &ObjectRef,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        // Pinned QuickJS performs JS_IsArray before its class/callability
        // fallback and before reading @@toStringTag. That unwraps every Proxy
        // layer and makes revocation observable even when the handler would
        // otherwise provide a custom tag.
        let is_array =
            match self.internal_is_array_jsvalue(realm, &JsValue::Object(object.object_id()))? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => return Ok(NativeConversion::Throw(value)),
            };
        let default_tag = if is_array {
            JsString::from_static("Array")
        } else {
            let state = self.0.state.borrow();
            let object_data = state.heap.object(object.object_id())?;
            match &object_data.payload {
                ObjectPayload::NativeFunction { .. }
                | ObjectPayload::BoundFunction { .. }
                | ObjectPayload::BytecodeFunction { .. } => JsString::from_static("Function"),
                ObjectPayload::Proxy(proxy) if proxy.is_callable => {
                    JsString::from_static("Function")
                }
                ObjectPayload::Proxy(_) => JsString::from_static("Object"),
                ObjectPayload::Error => JsString::from_static("Error"),
                ObjectPayload::Primitive(PrimitiveObjectData::Number(_)) => {
                    JsString::from_static("Number")
                }
                ObjectPayload::Primitive(PrimitiveObjectData::String(_)) => {
                    JsString::from_static("String")
                }
                ObjectPayload::Primitive(PrimitiveObjectData::Boolean(_)) => {
                    JsString::from_static("Boolean")
                }
                // QuickJS's built-in class fallback has no Symbol- or
                // BigInt-wrapper case. Their standard tags come exclusively
                // from inherited configurable @@toStringTag properties.
                ObjectPayload::Primitive(
                    PrimitiveObjectData::Symbol(_) | PrimitiveObjectData::BigInt(_) | PrimitiveObjectData::ShortBigInt(_),
                ) => JsString::from_static("Object"),
                ObjectPayload::Array { .. } => JsString::from_static("Array"),
                ObjectPayload::Arguments { .. } => JsString::from_static("Arguments"),
                ObjectPayload::Date(_) => JsString::from_static("Date"),
                ObjectPayload::RegExp(_) => JsString::from_static("RegExp"),
                ObjectPayload::Ordinary
                | ObjectPayload::ArrayBuffer(_)
                | ObjectPayload::SharedArrayBuffer(_)
                | ObjectPayload::DataView(_)
                | ObjectPayload::TypedArray(_)
                | ObjectPayload::RawJson
                | ObjectPayload::Promise(_)
                | ObjectPayload::IteratorHelper(_)
                | ObjectPayload::IteratorWrap(_)
                | ObjectPayload::AsyncFromSyncIterator(_)
                | ObjectPayload::IteratorConcat(_)
                | ObjectPayload::Map { .. }
                | ObjectPayload::MapIterator { .. }
                | ObjectPayload::Set { .. }
                | ObjectPayload::WeakMap { .. }
                | ObjectPayload::WeakSet { .. }
            | ObjectPayload::WeakRef { .. }
            | ObjectPayload::FinalizationRegistry(_)
                | ObjectPayload::SetIterator { .. }
                | ObjectPayload::ForInIterator(_)
                | ObjectPayload::GlobalObject { .. }
                | ObjectPayload::AsyncFunctionState(_)
                // Pinned QuickJS's Object.prototype.toString class switch
                // deliberately excludes JS_CLASS_GENERATOR. The standard
                // "Generator" tag comes from the inherited @@toStringTag;
                // deleting it therefore falls back to "Object".
                | ObjectPayload::Generator { .. }
            | ObjectPayload::AsyncGenerator(_) => JsString::from_static("Object"),
                ObjectPayload::ArrayIterator { .. }
                | ObjectPayload::StringIterator { .. }
                | ObjectPayload::RegExpStringIterator { .. } => JsString::from_static("Object"),
            }
        };
        Ok(NativeConversion::Value(default_tag))
    }

    pub(crate) fn call_object_prototype_to_string(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            string::finish(
                self,
                realm,
                string::ObjectStringStep::start(
                    self,
                    realm,
                    string::ObjectStringKind::Tag,
                    invocation,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_to_locale_string(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            string::finish(
                self,
                realm,
                string::ObjectStringStep::start(
                    self,
                    realm,
                    string::ObjectStringKind::Locale,
                    invocation,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_value_of(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Object.prototype.valueOf did not receive a generic invocation",
            ));
        };
        match this_value {
            value @ JsValue::Object(_) => Ok(Completion::Return(value)),
            JsValue::Undefined | JsValue::Null => {
                Ok(Completion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?))
            }
            value @ JsValue::Bool(_) => {
                let prototype =
                    self.primitive_prototype_for_realm(realm, PrimitiveKind::Boolean)?;
                Ok(Completion::Return(JsValue::Object(
                    self.new_primitive_object_jsvalue(&prototype, PrimitiveKind::Boolean, value)?
                        .into_handle(),
                )))
            }
            value @ (JsValue::Int(_) | JsValue::Float(_)) => {
                let prototype = self.primitive_prototype_for_realm(realm, PrimitiveKind::Number)?;
                Ok(Completion::Return(JsValue::Object(
                    self.new_primitive_object_jsvalue(&prototype, PrimitiveKind::Number, value)?
                        .into_handle(),
                )))
            }
            value @ JsValue::String(_) => {
                let prototype = self.primitive_prototype_for_realm(realm, PrimitiveKind::String)?;
                Ok(Completion::Return(JsValue::Object(
                    self.new_primitive_object_jsvalue(&prototype, PrimitiveKind::String, value)?
                        .into_handle(),
                )))
            }
            value @ (JsValue::BigInt(_) | JsValue::ShortBigInt(_)) => {
                let prototype = self.primitive_prototype_for_realm(realm, PrimitiveKind::BigInt)?;
                Ok(Completion::Return(JsValue::Object(
                    self.new_primitive_object_jsvalue(&prototype, PrimitiveKind::BigInt, value)?
                        .into_handle(),
                )))
            }
            value @ JsValue::Symbol(_) => {
                let prototype = self.primitive_prototype_for_realm(realm, PrimitiveKind::Symbol)?;
                Ok(Completion::Return(JsValue::Object(
                    self.new_primitive_object_jsvalue(&prototype, PrimitiveKind::Symbol, value)?
                        .into_handle(),
                )))
            }
        }
    }

    pub(crate) fn call_object_constructor(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            constructor::finish(
                self,
                realm,
                constructor::ObjectConstructorStep::start(self, realm, invocation, arguments)?,
            )
        })
    }

    pub(crate) fn call_object_create(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object definitions did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        definitions::finish(
            self,
            realm,
            definitions::DefinitionsStep::start(
                self,
                realm,
                definitions::DefinitionsKind::Create,
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_get_prototype_of(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object.getPrototypeOf did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        prototype::finish(
            self,
            realm,
            prototype::BuiltinPrototypeStep::start(
                self,
                realm,
                prototype::BuiltinPrototypeKind::ObjectGet,
                arguments,
            )?,
        )
    }

    fn finish_set_prototype_or_throw(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        result: NativeConversion<bool>,
    ) -> Result<Option<JsValue>, RuntimeError> {
        match result {
            NativeConversion::Value(true) => return Ok(None),
            NativeConversion::Throw(value) => return Ok(Some(value)),
            NativeConversion::Value(false) => {}
        }
        if self.is_proxy_object(object)? {
            return Ok(Some(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "proxy: bad prototype",
            )?));
        }
        let (immutable, extensible) = {
            let state = self.0.state.borrow();
            let object = state.heap.object(object.object_id())?;
            (object.immutable_prototype, object.extensible)
        };
        let message = if immutable {
            "prototype is immutable"
        } else if !extensible {
            "object is not extensible"
        } else {
            "circular prototype chain"
        };
        Ok(Some(self.new_native_error_jsvalue(
            realm,
            NativeErrorKind::Type,
            message,
        )?))
    }

    pub(crate) fn call_object_set_prototype_of(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object.setPrototypeOf did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        prototype::finish(
            self,
            realm,
            prototype::BuiltinPrototypeStep::start(
                self,
                realm,
                prototype::BuiltinPrototypeKind::ObjectSet,
                arguments,
            )?,
        )
    }

    fn property_define_rejection(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<JsValue, RuntimeError> {
        if let ArrayOwnKey::Index(index) = self.array_own_key(object, key)? {
            let (length, writable) = self.array_length_state(object)?;
            if index >= length && !writable {
                let length =
                    self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
                let error =
                    self.native_atom_error(ErrorKind::Type, "'", &length, "' is read-only")?;
                return self.new_native_error_from_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    &error,
                );
            }
        }
        let message = if !self.has_own_property(object, key)? && !self.is_extensible(object)? {
            "object is not extensible"
        } else {
            "property is not configurable"
        };
        self.new_native_error_jsvalue(realm, NativeErrorKind::Type, message)
    }

    fn finish_define_property_or_throw(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<Option<JsValue>, RuntimeError> {
        match result {
            NativeConversion::Value(InternalDefineResult::Defined) => Ok(None),
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => self
                .new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "proxy: defineProperty exception",
                )
                .map(Some),
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(target)) => self
                .property_define_rejection(realm, &target, key)
                .map(Some),
            NativeConversion::Throw(value) => Ok(Some(value)),
        }
    }

    pub(crate) fn call_object_define_property(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object property method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::ObjectDefine,
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_define_properties(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object definitions did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        definitions::finish(
            self,
            realm,
            definitions::DefinitionsStep::start(
                self,
                realm,
                definitions::DefinitionsKind::Define,
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_get_own_property_keys(
        &self,
        realm: ContextId,
        kind: ObjectOwnPropertyKeysKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object enumeration did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::ObjectOwnKeys(kind),
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_keys(
        &self,
        realm: ContextId,
        kind: ObjectKeysKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object enumeration did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::ObjectKeys(kind),
                arguments,
            )?,
        )
    }

    fn define_fresh_object_keys_array_element(
        &self,
        array: &ObjectRef,
        index: u32,
        value: JsValue,
        rejection: &'static str,
    ) -> Result<(), RuntimeError> {
        let outcome = (|| {
            let key = self.property_key_for_index(u64::from(index))?;
            self.define_selected_set_data(array, &key, &value, false)
        })();
        self.release_jsvalue(value)?;
        match outcome? {
            crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => Ok(()),
            crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                Err(RuntimeError::Invariant(rejection))
            }
            crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                self.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(rejection))
            }
        }
    }

    pub(crate) fn call_object_extensibility(
        &self,
        realm: ContextId,
        kind: ObjectExtensibilityKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object extensibility method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        let kind = match kind {
            ObjectExtensibilityKind::IsExtensible => property::PropertyKind::ObjectExtensible,
            ObjectExtensibilityKind::PreventExtensions => property::PropertyKind::ObjectPrevent,
        };
        property::finish(
            self,
            realm,
            property::PropertyStep::start(self, realm, kind, arguments)?,
        )
    }

    /// Allocate the ordinary Object produced by QuickJS `OP_object` in the
    /// executing realm, without routing VM allocation through Context's public
    /// convenience layer.
    pub(crate) fn new_ordinary_object_in_realm(
        &self,
        realm: ContextId,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation()?;
        let object = self
            .0
            .state
            .borrow_mut()
            .new_ordinary_object_in_realm(&self.0.poisoned, realm)?;
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    fn define_fresh_object_descriptor_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: JsValue,
        rejection: &'static str,
    ) -> Result<(), RuntimeError> {
        let outcome = self.define_selected_set_data(object, key, &value, false);
        self.release_jsvalue(value)?;
        match outcome? {
            crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => Ok(()),
            crate::engine::object::operations::PropertyDefineOutcome::Defined(false) => {
                Err(RuntimeError::Invariant(rejection))
            }
            crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                self.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(rejection))
            }
        }
    }

    fn complete_descriptor_to_object(
        &self,
        realm: ContextId,
        descriptor: crate::engine::object::OwnedCompletePropertyDescriptor,
    ) -> Result<ObjectRef, RuntimeError> {
        use crate::engine::object::property::CompletePropertyDescriptor;
        let object = self.new_ordinary_object_in_realm(realm)?;
        // The reply owner keeps all borrowed field handles live while stores retain them.
        let fields = match descriptor.record() {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => [
                (
                    "value",
                    JsValue::from_raw(value.clone())
                        .expect("descriptor contains initialized values"),
                ),
                ("writable", JsValue::Bool(*writable)),
                ("enumerable", JsValue::Bool(*enumerable)),
                ("configurable", JsValue::Bool(*configurable)),
            ],
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => [
                (
                    "get",
                    get.as_ref().map_or(JsValue::Undefined, |value| {
                        JsValue::from_raw(value.clone())
                            .expect("descriptor contains initialized values")
                    }),
                ),
                (
                    "set",
                    set.as_ref().map_or(JsValue::Undefined, |value| {
                        JsValue::from_raw(value.clone())
                            .expect("descriptor contains initialized values")
                    }),
                ),
                ("enumerable", JsValue::Bool(*enumerable)),
                ("configurable", JsValue::Bool(*configurable)),
            ],
        };
        for (name, value) in fields {
            let key = self.intern_property_key(name)?;
            if !match self.define_selected_set_data(&object, &key, &value, false)? {
                crate::engine::object::operations::PropertyDefineOutcome::Defined(defined) => {
                    defined
                }
                crate::engine::object::operations::PropertyDefineOutcome::Throw(value) => {
                    self.release_jsvalue(value)?;
                    false
                }
            } {
                return Err(RuntimeError::Invariant(
                    "fresh property descriptor object rejected a field",
                ));
            }
        }
        Ok(object)
    }

    pub(crate) fn call_object_get_own_property_descriptor(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object property method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::ObjectDescriptor,
                arguments,
            )?,
        )
    }

    pub(crate) fn object_property_key_value(
        &self,
        key: &PropertyKey,
    ) -> Result<Value, RuntimeError> {
        let kind = self.0.state.borrow().atoms.property_key_kind(key.atom())?;
        match kind {
            PropertyKeyKind::String => Ok(Value::String(
                self.0.state.borrow().atoms.to_js_string(key.atom())?,
            )),
            PropertyKeyKind::Symbol => Ok(Value::Symbol(SymbolRef::from_borrowed_atom(
                self.clone(),
                key.atom(),
            )?)),
            PropertyKeyKind::Private => Err(RuntimeError::Invariant(
                "private key escaped into Object.getOwnPropertyDescriptors",
            )),
        }
    }

    pub(crate) fn call_object_get_own_property_descriptors(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object enumeration did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::ObjectDescriptors,
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_is(
        &self,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object.is did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        let left = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant("Object.is lhs argv was not padded"))?;
        let right = arguments
            .readable
            .get(1)
            .ok_or(RuntimeError::Invariant("Object.is rhs argv was not padded"))?;
        let equal = crate::engine::value::collection_key::same_value(
            &self.0.state.borrow().heap,
            &left.as_raw(),
            &right.as_raw(),
        );
        Ok(Completion::Return(JsValue::Bool(equal)))
    }

    pub(crate) fn call_object_assign(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object.assign did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(self, realm, property::PropertyKind::Assign, arguments)?,
        )
    }

    pub(crate) fn call_object_integrity(
        &self,
        realm: ContextId,
        kind: ObjectIntegrityKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Object integrity method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        property::finish(
            self,
            realm,
            property::PropertyStep::start(
                self,
                realm,
                property::PropertyKind::Integrity(kind),
                arguments,
            )?,
        )
    }

    pub(crate) fn call_object_prototype_has_own_property(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            predicate::finish(
                self,
                realm,
                predicate::PredicateStep::start(
                    self,
                    realm,
                    predicate::PredicateKind::PrototypeHasOwn,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_property_is_enumerable(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            predicate::finish(
                self,
                realm,
                predicate::PredicateStep::start(
                    self,
                    realm,
                    predicate::PredicateKind::Enumerable,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_is_prototype_of(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            prototype::finish(
                self,
                realm,
                prototype::BuiltinPrototypeStep::start_invocation(
                    self,
                    realm,
                    prototype::BuiltinPrototypeKind::IsPrototype,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_proto_getter(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            prototype::finish(
                self,
                realm,
                prototype::BuiltinPrototypeStep::start_invocation(
                    self,
                    realm,
                    prototype::BuiltinPrototypeKind::Getter,
                    invocation,
                    &NativeArguments {
                        actual_arg_count: 0,
                        readable: Vec::new(),
                    },
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_proto_setter(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            prototype::finish(
                self,
                realm,
                prototype::BuiltinPrototypeStep::start_invocation(
                    self,
                    realm,
                    prototype::BuiltinPrototypeKind::Setter,
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_define_accessor(
        &self,
        realm: ContextId,
        kind: ObjectAccessorKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            predicate::finish(
                self,
                realm,
                predicate::PredicateStep::start(
                    self,
                    realm,
                    predicate::PredicateKind::Define(kind),
                    invocation,
                    arguments,
                )?,
            )
        })
    }

    pub(crate) fn call_object_prototype_lookup_accessor(
        &self,
        realm: ContextId,
        kind: ObjectAccessorKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            predicate::finish(
                self,
                realm,
                predicate::PredicateStep::start(
                    self,
                    realm,
                    predicate::PredicateKind::Lookup(kind),
                    invocation,
                    arguments,
                )?,
            )
        })
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ObjectIteratorStep>() <= 64);

impl RuntimeState {
    /// The realm owns its Object.prototype and allocation runs no JS, so the
    /// object needs no temporary prototype owner; its shape holds that edge.
    /// The factory still rejects a prototype that could not take another
    /// reference, checked without retaining it (a frozen B0 witness).
    pub(crate) fn new_ordinary_object_in_realm(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
    ) -> Result<ObjectId, RuntimeError> {
        let prototype = self.heap.context(realm)?.object_prototype;
        // The realm record owns its Object.prototype edge.
        self.heap
            .object_strong_fast(prototype)
            .checked_add(1)
            .ok_or(crate::engine::heap::HeapError::Overflow {
                operation: "retaining a heap reference",
            })?;
        self.new_empty_ordinary_object(poisoned, prototype)
    }
}
