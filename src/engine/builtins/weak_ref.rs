//! `%WeakRef%` / `%FinalizationRegistry%` and pinned QuickJS weak-target semantics.
//!
//! The heap owns the mixed construction-order weak-object pass. This layer
//! supplies ECMAScript validation, realm/prototype selection, branded native
//! dispatch, and the atom ownership boundary for held Symbol values.

use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{AtomIdx, AtomKind};
use crate::engine::object::builtin_properties::{BuiltinEntry, method, to_string_tag};

use crate::engine::builtins::native::{
    FinalizationRegistryNativeKind, NativeFunctionId, WeakRefNativeKind,
};
use crate::engine::heap::{
    ContextId, HeapError, ObjectId, ObjectPayload, RawValue, WeakCollectionKey, WeakRefRealmData,
};
use crate::engine::object::{CallableRef, ObjectRef};
use crate::engine::value::JsValue;
use crate::engine::value::conversion::NativeConversion;
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WeakIntrinsicKind {
    WeakRef,
    FinalizationRegistry,
}

pub(super) mod constructor;

impl Runtime {
    fn weak_intrinsic_mutation_error(error: HeapError) -> RuntimeError {
        match error {
            HeapError::Allocation { .. } => {
                RuntimeError::Engine(Error::new(ErrorKind::JsInternal, "out of memory"))
            }
            error => RuntimeError::Heap(error),
        }
    }

    pub(crate) fn initialize_weak_ref_intrinsics(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        object_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        use FinalizationRegistryNativeKind as FK;
        use NativeFunctionId::{FinalizationRegistry as F, WeakRef as W};
        const WEAK_REF_PROTOTYPE: &[BuiltinEntry] = &[
            method(W(WeakRefNativeKind::Deref), "deref", 0, 0),
            to_string_tag("WeakRef"),
        ];
        const REGISTRY_PROTOTYPE: &[BuiltinEntry] = &[
            method(F(FK::Register), "register", 2, 3),
            method(F(FK::Unregister), "unregister", 1, 1),
            to_string_tag("FinalizationRegistry"),
        ];
        let install = |name, entries, target| -> Result<ObjectRef, RuntimeError> {
            let prototype = self.new_object(Some(object_prototype))?;
            self.define_builtin_entries(&prototype, realm, entries)?;
            let constructor =
                self.new_native_builtin(function_prototype, realm, target, 1, name, 1)?;
            self.define_global_function(global_object, name, &constructor)?;
            self.define_constructor_relationship(&constructor, &prototype)?;
            Ok(prototype)
        };
        let weak_ref_prototype = install(
            "WeakRef",
            WEAK_REF_PROTOTYPE,
            W(WeakRefNativeKind::Constructor),
        )?;
        let finalization_registry_prototype = install(
            "FinalizationRegistry",
            REGISTRY_PROTOTYPE,
            F(FK::Constructor),
        )?;
        self.0.state.borrow_mut().heap.attach_weak_ref_intrinsics(
            realm,
            WeakRefRealmData {
                weak_ref_prototype: weak_ref_prototype.object_id(),
                finalization_registry_prototype: finalization_registry_prototype.object_id(),
            },
        )?;
        Ok(())
    }

    fn weak_ref_realm_data(&self, realm: ContextId) -> Result<WeakRefRealmData, RuntimeError> {
        self.0
            .state
            .borrow()
            .heap
            .context(realm)?
            .weak_ref()
            .ok_or(RuntimeError::Invariant("realm has no WeakRef intrinsics"))
    }

    fn weak_intrinsic_prototype(
        &self,
        realm: ContextId,
        kind: WeakIntrinsicKind,
    ) -> Result<ObjectId, RuntimeError> {
        let intrinsics = self.weak_ref_realm_data(realm)?;
        Ok(match kind {
            WeakIntrinsicKind::WeakRef => intrinsics.weak_ref_prototype,
            WeakIntrinsicKind::FinalizationRegistry => intrinsics.finalization_registry_prototype,
        })
    }

    fn weak_target_key(&self, value: &JsValue) -> Result<Option<WeakCollectionKey>, RuntimeError> {
        match value {
            JsValue::Object(object) => Ok(Some(WeakCollectionKey::Object(*object))),
            JsValue::Symbol(index) => {
                let state = self.0.state.borrow();
                let atom = state.atoms.brand(*index)?;
                Ok((state.atoms.kind(atom)? == AtomKind::Symbol)
                    .then_some(WeakCollectionKey::Symbol(atom)))
            }
            _ => Ok(None),
        }
    }

    fn invalid_weak_target(
        &self,
        realm: ContextId,
        message: &'static str,
    ) -> Result<Completion, RuntimeError> {
        Ok(Completion::Throw(self.new_native_error_jsvalue(
            realm,
            NativeErrorKind::Type,
            message,
        )?))
    }

    fn new_weak_ref_object(
        &self,
        prototype: &ObjectRef,
        target: WeakCollectionKey,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation()?;
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("WeakRef prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state
            .heap
            .allocate_weak_ref_object(shape, Vec::new(), target)
        {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(Self::weak_intrinsic_mutation_error(error));
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        let object = ObjectRef::from_owned_handle(self.clone(), object);
        self.add_to_kept_objects(target)?;
        Ok(object)
    }

    fn new_finalization_registry_object(
        &self,
        prototype: &ObjectRef,
        callback: &CallableRef,
        realm: ContextId,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation()?;
        if !prototype.belongs_to(self) || !callback.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime(
                "FinalizationRegistry constructor input",
            ));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state.heap.allocate_finalization_registry_object(
            shape,
            Vec::new(),
            callback.as_object().object_id(),
            realm,
        ) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(Self::weak_intrinsic_mutation_error(error));
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    pub(crate) fn call_weak_ref_native(
        &self,
        realm: ContextId,
        kind: WeakRefNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        match kind {
            WeakRefNativeKind::Constructor => constructor::finish(
                self,
                realm,
                constructor::WeakConstructorStep::start(
                    self,
                    realm,
                    WeakIntrinsicKind::WeakRef,
                    invocation,
                    arguments,
                )?,
            ),
            WeakRefNativeKind::Deref => {
                let NativeInvocation::Call { this_value } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "WeakRef.prototype.deref received the wrong native invocation",
                    ));
                };
                let JsValue::Object(id) = this_value else {
                    return self.invalid_weak_target(realm, "WeakRef object expected");
                };
                let weak_ref = ObjectRef::from_borrowed_handle(self.clone(), *id)?;
                let target = {
                    let state = self.0.state.borrow();
                    state.heap.weak_ref_target(weak_ref.object_id())
                };
                let target = match target {
                    Ok(target) => target,
                    Err(HeapError::Invariant(_)) => {
                        return self.invalid_weak_target(realm, "WeakRef object expected");
                    }
                    Err(error) => return Err(error.into()),
                };
                let Some(target) = target else {
                    return Ok(Completion::Return(JsValue::Undefined));
                };
                let live = {
                    let state = self.0.state.borrow();
                    match target {
                        WeakCollectionKey::Object(object) => {
                            match state.heap.object_strong_count(object) {
                                Ok(count) => count != 0,
                                Err(HeapError::Stale { .. }) => false,
                                Err(error) => return Err(error.into()),
                            }
                        }
                        WeakCollectionKey::Symbol(atom) => state.atoms.is_live(atom),
                    }
                };
                if !live {
                    return Ok(Completion::Return(JsValue::Undefined));
                }
                self.add_to_kept_objects(target)?;
                let raw = match target {
                    WeakCollectionKey::Object(object) => RawValue::Object(object),
                    // The branded key atom was already validated by the heap
                    // lookup above, so it can be narrowed without re-branding.
                    WeakCollectionKey::Symbol(atom) => {
                        RawValue::Symbol(AtomIdx::from_raw(atom.raw()))
                    }
                };
                Ok(Completion::Return(self.dup_jsvalue(
                    &JsValue::from_raw(raw.clone()).ok_or(RuntimeError::Invariant(
                        "stored collection value is uninitialized",
                    ))?,
                )?))
            }
        }
    }

    pub(in crate::engine::builtins) fn finalization_registry_receiver(
        &self,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "FinalizationRegistry method received the wrong native invocation",
            ));
        };
        let JsValue::Object(id) = this_value else {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "FinalizationRegistry object expected",
            )?));
        };
        let registry = ObjectRef::from_borrowed_handle(self.clone(), *id)?;
        let has_brand = matches!(
            self.0
                .state
                .borrow()
                .heap
                .object(registry.object_id())?
                .payload,
            ObjectPayload::FinalizationRegistry(_)
        );
        if !has_brand {
            return Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                "FinalizationRegistry object expected",
            )?));
        }
        Ok(NativeConversion::Value(registry))
    }

    pub(crate) fn call_finalization_registry_native(
        &self,
        realm: ContextId,
        kind: FinalizationRegistryNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        if kind == FinalizationRegistryNativeKind::Constructor {
            return constructor::finish(
                self,
                realm,
                constructor::WeakConstructorStep::start(
                    self,
                    realm,
                    WeakIntrinsicKind::FinalizationRegistry,
                    invocation,
                    arguments,
                )?,
            );
        }

        let registry = match self.finalization_registry_receiver(realm, invocation)? {
            NativeConversion::Value(registry) => registry,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(value));
            }
        };
        let first = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "FinalizationRegistry first argv was not padded",
        ))?;
        match kind {
            FinalizationRegistryNativeKind::Constructor => {
                unreachable!("FinalizationRegistry constructor returned before receiver validation")
            }
            FinalizationRegistryNativeKind::Register => {
                let Some(target) = self.weak_target_key(first)? else {
                    return self.invalid_weak_target(realm, "invalid target");
                };
                let held_value = arguments.readable.get(1).ok_or(RuntimeError::Invariant(
                    "FinalizationRegistry held value argv was not padded",
                ))?;
                let same_target = {
                    let state = self.0.state.borrow();
                    crate::engine::value::collection_key::same_value(
                        &state.heap,
                        &arguments.readable[0].as_raw(),
                        &held_value.as_raw(),
                    )
                };
                if same_target {
                    return self.invalid_weak_target(realm, "held value cannot be the target");
                }
                let token_value = arguments.readable.get(2).ok_or(RuntimeError::Invariant(
                    "FinalizationRegistry unregister token argv was not padded",
                ))?;
                let unregister_token = if matches!(token_value, JsValue::Undefined) {
                    None
                } else {
                    let Some(token) = self.weak_target_key(token_value)? else {
                        return self.invalid_weak_target(realm, "invalid unregister token");
                    };
                    Some(token)
                };

                let raw_held_value = held_value.as_raw();
                let mut state = self.0.state.borrow_mut();
                let retained_atoms = state.retain_raw_value_atoms([&raw_held_value])?;
                if let Err(error) = state.heap.finalization_registry_register(
                    registry.object_id(),
                    target,
                    raw_held_value,
                    unregister_token,
                ) {
                    state.release_atoms(retained_atoms)?;
                    return Err(Self::weak_intrinsic_mutation_error(error));
                }
                Ok(Completion::Return(JsValue::Undefined))
            }
            FinalizationRegistryNativeKind::Unregister => {
                let Some(token) = self.weak_target_key(first)? else {
                    return self.invalid_weak_target(realm, "invalid unregister token");
                };
                let mut state = self.0.state.borrow_mut();
                let (removed, cleanup) = state
                    .heap
                    .finalization_registry_unregister(registry.object_id(), token)?;
                state.apply_cleanup(cleanup)?;
                Ok(Completion::Return(JsValue::Bool(removed)))
            }
        }
    }
}
