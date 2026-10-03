//! Running binding ownership and shared-cell lifetime rules.
//!
//! Direct values, private identities, TDZ and captured cells remain distinct.
//! Capturing installs the rooted cell before the direct owner is released;
//! closing roots the detached value before dropping the frame's cell handle.
//! Long-lived suspension storage must encode these owners as managed raw edges.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::AtomIdx;
use crate::engine::code::function::metadata::{ClosureVariable, ClosureVariableKind};
use crate::engine::heap::roots::{VarRefRoot, VarRefView};
use crate::engine::heap::{ObjectId, RawValue, VarRefId};
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;

/// A running frame binding owns exactly one edge for every non-direct variant.
///
/// `Private`, `PrivateCallable` and `Captured` store unbranded handles instead
/// of rooted wrappers: they have no `Drop`, so every move, overwrite and
/// abandonment path must release them explicitly through
/// [`release_frame_binding`].
pub(in crate::engine::vm) enum FrameBinding {
    Direct(JsValue),
    Private(AtomIdx),
    PrivateCallable(ObjectId),
    Uninitialized,
    Captured(VarRefId),
}

// The tag packs into `JsValue`'s spare discriminant values, so the whole
// binding plus its optional slot fit in the direct payload.
const _: () = assert!(std::mem::size_of::<FrameBinding>() == 16);
const _: () = assert!(std::mem::size_of::<Option<FrameBinding>>() == 16);

pub(in crate::engine::vm) const fn is_private_callable_kind(kind: ClosureVariableKind) -> bool {
    matches!(
        kind,
        ClosureVariableKind::PrivateMethod
            | ClosureVariableKind::PrivateGetter
            | ClosureVariableKind::PrivateSetter
            | ClosureVariableKind::PrivateGetterSetter
    )
}

/// Release every owner a frame binding carried. Direct internal values take
/// the deferred-release path; the handle variants release their single edge
/// through the nothrow heap paths.
pub(in crate::engine::vm) fn release_frame_binding(
    runtime: &Runtime,
    binding: FrameBinding,
) -> Result<(), Error> {
    match binding {
        FrameBinding::Direct(value) => runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error),
        FrameBinding::Private(index) => {
            runtime.release_atom_index(index);
            Ok(())
        }
        FrameBinding::PrivateCallable(object) => {
            runtime.release_object_handle(object);
            Ok(())
        }
        FrameBinding::Captured(var_ref) => {
            runtime.release_var_ref_handle(var_ref);
            Ok(())
        }
        FrameBinding::Uninitialized => Ok(()),
    }
}

/// Release a binding while the execution core already owns state access.
/// Captured cells, private atoms and private callables carry the same one-edge
/// obligation as direct values; none needs a temporary public root.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::engine::vm) fn release_frame_binding_in_state(
    state: &mut crate::engine::heap::runtime::RuntimeState,
    binding: FrameBinding,
) -> Result<(), RuntimeError> {
    match binding {
        FrameBinding::Direct(value) => state.release_jsvalue(value),
        FrameBinding::Private(index) => state.release_atom_index(index),
        FrameBinding::PrivateCallable(object) => state.release_object_handle(object),
        FrameBinding::Captured(var_ref) => state.release_var_ref_handle(var_ref),
        FrameBinding::Uninitialized => Ok(()),
    }
}

#[cfg(test)]
mod direct_state_release_tests {
    use super::*;
    use crate::engine::heap::VarRefData;

    #[test]
    fn captured_and_private_bindings_release_their_edges_directly() {
        let runtime = Runtime::new();
        let value_id = runtime.new_object(None).unwrap().into_handle();
        let callable_id = runtime.new_object(None).unwrap().into_handle();
        let symbol = runtime.new_symbol(None).unwrap();
        let atom = symbol.atom();
        let mut state = runtime.0.state.borrow_mut();
        let index = state.atoms.unbrand(atom).unwrap();
        state.atoms.retain_index(index).unwrap();
        let cell = state
            .heap
            .allocate_var_ref_owned(VarRefData::captured(
                RawValue::Object(value_id),
                false,
                false,
                ClosureVariableKind::Normal,
            ))
            .unwrap();
        release_frame_binding_in_state(&mut state, FrameBinding::Captured(cell)).unwrap();
        release_frame_binding_in_state(&mut state, FrameBinding::PrivateCallable(callable_id))
            .unwrap();
        release_frame_binding_in_state(&mut state, FrameBinding::Private(index)).unwrap();
        release_frame_binding_in_state(&mut state, FrameBinding::Direct(JsValue::Int(1))).unwrap();
        release_frame_binding_in_state(&mut state, FrameBinding::Uninitialized).unwrap();
        assert!(state.heap.var_ref(cell).is_err());
        assert!(state.heap.object(value_id).is_err());
        assert!(state.heap.object(callable_id).is_err());
        assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

/// QuickJS keeps access flags on each closure descriptor rather than on the
/// shared VarRef. Its ordinary direct-eval prepass may therefore expose one
/// FunctionName cell through a mutable Normal descriptor. A module import is
/// likewise an immutable lexical view of the exporter's original mutable or
/// immutable ordinary cell, and nested closures/eval relay that immutable
/// view after the original `ModuleImport` source tag is no longer present.
/// Publication authenticates where these view-only metadata differences enter
/// the closure chain.
pub(crate) fn closure_view_matches_cell(
    cell: (bool, bool, ClosureVariableKind),
    descriptor: ClosureVariable,
) -> bool {
    cell == (descriptor.is_lexical, descriptor.is_const, descriptor.kind)
        || (descriptor.is_lexical
            && descriptor.is_const
            && descriptor.kind == ClosureVariableKind::ModuleImportView
            && cell.2 == ClosureVariableKind::Normal)
        || (cell.0 == descriptor.is_lexical
            && !cell.0
            && cell.2 == ClosureVariableKind::FunctionName
            && !descriptor.is_const
            && descriptor.kind == ClosureVariableKind::Normal)
}

#[inline]
pub(in crate::engine::vm) fn read_frame_binding(
    runtime: &Runtime,
    binding: &FrameBinding,
) -> Result<JsValue, Error> {
    match binding {
        FrameBinding::Direct(value) => runtime
            .dup_jsvalue(value)
            .map_err(|error| Error::internal(error.to_string())),
        FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => Err(Error::internal(
            "ordinary local read reached a private-element binding",
        )),
        FrameBinding::Uninitialized => Err(Error::internal(
            "unchecked local read reached an uninitialized lexical binding",
        )),
        FrameBinding::Captured(var_ref) => runtime
            .read_var_ref(&VarRefView::from_frame(runtime, *var_ref))
            .map_err(|error| Error::internal(error.to_string())),
    }
}

/// The caller's frame or closure owns this cell throughout the short read.
#[inline]
pub(in crate::engine::vm) fn try_read_captured_immediate_in_state(
    state: &crate::engine::heap::runtime::RuntimeState,
    id: VarRefId,
) -> Option<JsValue> {
    let cell = state.heap.var_ref(id).ok()?;
    if cell.kind.is_private() {
        return None;
    }
    if !matches!(
        cell.value,
        RawValue::Undefined
            | RawValue::Null
            | RawValue::Bool(_)
            | RawValue::Int(_)
            | RawValue::Float(_)
            | RawValue::ShortBigInt(_)
    ) {
        return None;
    }
    let count = state.heap.var_ref_strong_count(id).ok()?;
    if count == 0 || count >= u32::MAX - 1 {
        return None;
    }
    let value = JsValue::from_raw(cell.value.clone())?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("captured_scalar.read");
    Some(value)
}

/// Publish a freshly created shared cell as both the frame binding and the
/// caller's returned root. The binding and the returned root are independent
/// owners, so the binding retains its own edge before the store.
fn publish_captured_cell(
    runtime: &Runtime,
    binding: &mut FrameBinding,
    root: VarRefRoot,
) -> Result<VarRefRoot, Error> {
    runtime
        .retain_var_ref_handle(root.id())
        .map_err(|error| Error::internal(error.to_string()))?;
    *binding = FrameBinding::Captured(root.id());
    Ok(root)
}

pub(in crate::engine::vm) fn capture_frame_binding(
    runtime: &Runtime,
    binding: &mut FrameBinding,
    descriptor: ClosureVariable,
) -> Result<VarRefRoot, Error> {
    match binding {
        FrameBinding::Direct(_) => {
            if descriptor.kind.is_private() {
                return Err(Error::internal(
                    "private-name capture reached an ordinary frame value",
                ));
            }
            // Move the direct owner into the shared cell; `new_var_ref`
            // consumes its edges and the capture replaces the binding.
            let owned = std::mem::replace(binding, FrameBinding::Uninitialized);
            let FrameBinding::Direct(value) = owned else {
                unreachable!("direct binding authenticated before the move")
            };
            let root = runtime
                .new_var_ref(
                    value,
                    descriptor.is_lexical,
                    descriptor.is_const,
                    descriptor.kind,
                )
                .map_err(|error| Error::internal(error.to_string()))?;
            publish_captured_cell(runtime, binding, root)
        }
        FrameBinding::Private(index) => {
            if descriptor.kind != ClosureVariableKind::PrivateField
                || !descriptor.is_lexical
                || !descriptor.is_const
            {
                return Err(Error::internal(
                    "private-field frame cell used an incompatible closure descriptor",
                ));
            }
            let index = *index;
            let root = runtime
                .new_private_var_ref_from_index(index)
                .map_err(|error| Error::internal(error.to_string()))?;
            let root = publish_captured_cell(runtime, binding, root)?;
            // The shared cell owns its own atom edge now; drop the frame's.
            runtime.release_atom_index(index);
            Ok(root)
        }
        FrameBinding::PrivateCallable(object) => {
            if !is_private_callable_kind(descriptor.kind)
                || !descriptor.is_lexical
                || !descriptor.is_const
            {
                return Err(Error::internal(
                    "private-callable frame cell used an incompatible closure descriptor",
                ));
            }
            let object = *object;
            let root = runtime
                .new_private_callable_var_ref_from_id(object, descriptor.kind)
                .map_err(|error| Error::internal(error.to_string()))?;
            let root = publish_captured_cell(runtime, binding, root)?;
            // The shared cell owns its own object edge now; drop the frame's.
            runtime.release_object_handle(object);
            Ok(root)
        }
        FrameBinding::Uninitialized => {
            let root = runtime
                .new_uninitialized_captured_var_ref(
                    descriptor.is_lexical,
                    descriptor.is_const,
                    descriptor.kind,
                )
                .map_err(|error| Error::internal(error.to_string()))?;
            publish_captured_cell(runtime, binding, root)
        }
        FrameBinding::Captured(var_ref) => reuse_frame_capture(
            runtime,
            &VarRefView::from_frame(runtime, *var_ref),
            descriptor,
        ),
    }
}

/// Reuse a live cell through a publication-authenticated descriptor view.
/// This checks actual cell metadata without redispatching its frame storage.
pub(in crate::engine::vm) fn reuse_frame_capture(
    runtime: &Runtime,
    root: &impl crate::engine::heap::roots::VarRefHandle,
    descriptor: ClosureVariable,
) -> Result<VarRefRoot, Error> {
    runtime
        .validate_var_ref_metadata(root, descriptor)
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(root.to_root()?)
}

pub(in crate::engine::vm) fn close_frame_binding(
    runtime: &Runtime,
    binding: &mut FrameBinding,
    kind: ClosureVariableKind,
) -> Result<(), Error> {
    let FrameBinding::Captured(var_ref) = binding else {
        return Ok(());
    };
    let var_ref = *var_ref;
    let view = VarRefView::from_frame(runtime, var_ref);
    let raw = runtime
        .raw_var_ref_value(&view)
        .map_err(|error| Error::internal(error.to_string()))?;
    let detached = match raw {
        RawValue::Uninitialized => FrameBinding::Uninitialized,
        RawValue::Private(_) if kind == ClosureVariableKind::PrivateField => FrameBinding::Private(
            runtime
                .private_name_index_from_raw_var_ref(&view)
                .map_err(runtime_error_to_vm_error)?,
        ),
        RawValue::Object(_) if is_private_callable_kind(kind) => FrameBinding::PrivateCallable(
            runtime
                .private_callable_id_from_raw_var_ref(&view, kind)
                .map_err(runtime_error_to_vm_error)?,
        ),
        _ if kind.is_private() => {
            return Err(Error::internal(
                "captured private-element cell contains an incompatible value",
            ));
        }
        raw => {
            let value = JsValue::from_raw(raw).ok_or_else(|| {
                Error::internal("captured cell contained an internal value sentinel")
            })?;
            FrameBinding::Direct(
                runtime
                    .dup_jsvalue(&value)
                    .map_err(runtime_error_to_vm_error)?,
            )
        }
    };
    *binding = detached;
    // The detached owner holds its own edge; release the frame's cell edge.
    runtime.release_var_ref_handle(var_ref);
    Ok(())
}

/// Validate a derived constructor's explicit return against its lexical this.
/// No user code runs while reading the binding or materializing these errors.
pub(in crate::engine::vm) fn finish_derived_return(
    runtime: &Runtime,
    caller_realm: crate::engine::heap::ContextId,
    definition: crate::engine::code::function::metadata::VariableDefinition,
    binding: Option<&FrameBinding>,
    value: JsValue,
) -> Result<crate::engine::vm::Completion, Error> {
    use crate::engine::api::error::NativeErrorKind;
    use crate::engine::vm::Completion;
    if !definition.is_lexical
        || definition.is_const
        || definition.kind != ClosureVariableKind::Normal
    {
        return Err(Error::internal(
            "derived return referenced a non-mutable lexical this local",
        ));
    }
    match value {
        value @ JsValue::Object(_) => Ok(Completion::Return(value)),
        JsValue::Undefined => {
            let binding = binding.ok_or_else(|| Error::internal("local index is out of bounds"))?;
            let this_value = match binding {
                FrameBinding::Direct(value) => runtime
                    .dup_jsvalue(value)
                    .map_err(runtime_error_to_vm_error)?,
                FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => {
                    return Err(Error::internal(
                        "derived this local contains a private-element identity",
                    ));
                }
                FrameBinding::Uninitialized => {
                    return runtime
                        .new_native_error_jsvalue(
                            caller_realm,
                            NativeErrorKind::Reference,
                            "this is not initialized",
                        )
                        .map(Completion::Throw)
                        .map_err(runtime_error_to_vm_error);
                }
                FrameBinding::Captured(var_ref) => {
                    let raw = runtime
                        .raw_var_ref_value(&VarRefView::from_frame(runtime, *var_ref))
                        .map_err(runtime_error_to_vm_error)?;
                    if matches!(raw, RawValue::Uninitialized) {
                        return runtime
                            .new_native_error_jsvalue(
                                caller_realm,
                                NativeErrorKind::Reference,
                                "this is not initialized",
                            )
                            .map(Completion::Throw)
                            .map_err(runtime_error_to_vm_error);
                    }
                    let value = JsValue::from_raw(raw).ok_or_else(|| {
                        Error::internal("captured this cell held an internal value sentinel")
                    })?;
                    runtime
                        .dup_jsvalue(&value)
                        .map_err(runtime_error_to_vm_error)?
                }
            };
            if !matches!(this_value, JsValue::Object(_)) {
                return Err(Error::internal(
                    "initialized derived this binding did not contain an Object",
                ));
            }
            Ok(Completion::Return(this_value))
        }
        _ => {
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            runtime
                .new_native_error_jsvalue(
                    caller_realm,
                    NativeErrorKind::Type,
                    "derived class constructor must return an object or undefined",
                )
                .map(Completion::Throw)
                .map_err(runtime_error_to_vm_error)
        }
    }
}

/// Return a direct replacement only for a fresh this slot; captured this is
/// updated in place, and a previously initialized binding is never overwritten.
pub(in crate::engine::vm) fn initialize_derived_binding(
    runtime: &Runtime,
    definition: crate::engine::code::function::metadata::VariableDefinition,
    binding: Option<&FrameBinding>,
    value: JsValue,
) -> Result<Option<FrameBinding>, Error> {
    use crate::engine::api::error::ErrorKind;
    if !definition.is_lexical
        || definition.is_const
        || definition.kind != ClosureVariableKind::Normal
    {
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
        return Err(Error::internal(
            "derived this initialization referenced a non-mutable lexical local",
        ));
    }
    if !matches!(value, JsValue::Object(_)) {
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
        return Err(Error::internal(
            "derived this initialization did not receive an Object",
        ));
    }

    let Some(binding) = binding else {
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
        return Err(Error::internal("local index is out of bounds"));
    };
    let captured = match binding {
        FrameBinding::Uninitialized => None,
        FrameBinding::Captured(var_ref) => Some(*var_ref),
        FrameBinding::Direct(_) | FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => {
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            return Err(Error::new(
                ErrorKind::Reference,
                "'this' can be initialized only once",
            ));
        }
    };
    if let Some(var_ref) = captured {
        let view = VarRefView::from_frame(runtime, var_ref);
        let raw = runtime
            .raw_var_ref_value(&view)
            .map_err(runtime_error_to_vm_error)?;
        if !matches!(raw, RawValue::Uninitialized) {
            runtime
                .release_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            return Err(Error::new(
                ErrorKind::Reference,
                "'this' can be initialized only once",
            ));
        }
        return runtime
            .write_var_ref(&view, value)
            .map(|()| None)
            .map_err(runtime_error_to_vm_error);
    }
    Ok(Some(FrameBinding::Direct(value)))
}

/// Shared diagnostic policy for local, closure and dynamic TDZ reads.
pub(in crate::engine::vm) fn lexical_uninitialized_error(
    runtime: &Runtime,
    name: Option<crate::engine::atom::Atom>,
    name_visible: bool,
) -> Result<Error, Error> {
    use crate::engine::api::error::ErrorKind;
    use crate::engine::object::PropertyKey;
    let Some(name) = name else {
        return Ok(Error::new(
            ErrorKind::Reference,
            "lexical variable is not initialized",
        ));
    };
    if !name_visible {
        return Ok(Error::new(
            ErrorKind::Reference,
            "lexical variable is not initialized",
        ));
    }
    // Compiler-only pseudo names must not leak into observable diagnostics.
    // QuickJS stores this identity as JS_ATOM_this and therefore reports
    // `this`, while this typed compiler uses the unspellable `<this>` name
    // to keep it distinct from authored bindings.
    let hidden_this = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::This)
        .map_err(|error| Error::internal(error.to_string()))?;
    if hidden_this.atom() == name {
        return Ok(Error::new(ErrorKind::Reference, "this is not initialized"));
    }
    let key = PropertyKey::from_borrowed_atom(runtime.clone(), name)
        .map_err(|error| Error::internal(error.to_string()))?;
    runtime
        .native_atom_error(ErrorKind::Reference, "", &key, " is not initialized")
        .map_err(runtime_error_to_vm_error)
}

pub(in crate::engine::vm) fn closure_lexical_uninitialized_error(
    runtime: &Runtime,
    source: crate::engine::code::function::metadata::ClosureSource,
    name: Option<crate::engine::atom::Atom>,
    strip_variable_debug: bool,
) -> Result<Error, Error> {
    use crate::engine::code::function::metadata::ClosureSource;
    let semantic_name = matches!(
        source,
        ClosureSource::GlobalDeclaration
            | ClosureSource::Global
            | ClosureSource::ParentGlobal(_)
            | ClosureSource::ModuleDeclaration
            | ClosureSource::ModuleImport
            | ClosureSource::ModuleImportCollision
            | ClosureSource::ModuleImportMeta
    );
    lexical_uninitialized_error(runtime, name, semantic_name || !strip_variable_debug)
}

pub(in crate::engine::vm) fn lexical_read_only_error(
    runtime: &Runtime,
    name: Option<crate::engine::atom::Atom>,
) -> Result<Error, Error> {
    use crate::engine::api::error::ErrorKind;
    use crate::engine::object::PropertyKey;
    let Some(name) = name else {
        return Ok(Error::new(ErrorKind::Type, "lexical variable is read-only"));
    };
    let key = PropertyKey::from_borrowed_atom(runtime.clone(), name)
        .map_err(|error| Error::internal(error.to_string()))?;
    runtime
        .native_atom_error(ErrorKind::Type, "'", &key, "' is read-only")
        .map_err(runtime_error_to_vm_error)
}

pub(in crate::engine::vm) fn closure_name(
    descriptor: ClosureVariable,
) -> Result<Option<crate::engine::atom::Atom>, Error> {
    use crate::engine::code::function::metadata::ClosureVariableName;
    match descriptor.name {
        ClosureVariableName::Atom(name) => Ok(Some(name)),
        ClosureVariableName::None => Ok(None),
        ClosureVariableName::Constant(_) => Err(Error::internal(
            "published closure descriptor retained an unlinked name constant",
        )),
    }
}

pub(in crate::engine::vm) fn read_checked_closure(
    runtime: &Runtime,
    root: &impl crate::engine::heap::roots::VarRefHandle,
    descriptor: ClosureVariable,
    strip_variable_debug: bool,
) -> Result<JsValue, Error> {
    let raw = runtime
        .raw_var_ref_value(root)
        .map_err(runtime_error_to_vm_error)?;
    if matches!(raw, RawValue::Uninitialized) {
        return Err(closure_lexical_uninitialized_error(
            runtime,
            descriptor.source,
            closure_name(descriptor)?,
            strip_variable_debug,
        )?);
    }
    let value = JsValue::from_raw(raw)
        .ok_or_else(|| Error::internal("captured cell held an internal value sentinel"))?;
    runtime
        .dup_jsvalue(&value)
        .map_err(runtime_error_to_vm_error)
}

pub(in crate::engine::vm) fn write_checked_closure(
    runtime: &Runtime,
    root: &impl crate::engine::heap::roots::VarRefHandle,
    descriptor: ClosureVariable,
    strip_variable_debug: bool,
    value: JsValue,
) -> Result<(), Error> {
    let (uninitialized, is_const) = {
        let state = runtime.0.state.borrow();
        let cell = state
            .heap
            .var_ref(root.id())
            .map_err(|error| Error::internal(error.to_string()))?;
        (matches!(cell.value, RawValue::Uninitialized), cell.is_const)
    };
    if uninitialized {
        let error = closure_lexical_uninitialized_error(
            runtime,
            descriptor.source,
            closure_name(descriptor)?,
            strip_variable_debug,
        )?;
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
        return Err(error);
    }
    if is_const {
        let error = lexical_read_only_error(runtime, closure_name(descriptor)?)?;
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
        return Err(error);
    }
    runtime
        .write_var_ref(root, value)
        .map_err(runtime_error_to_vm_error)
}

/// New local cells take the parent's canonical metadata; existing cells validate
/// the child's authenticated view without changing the cell's identity.
pub(in crate::engine::vm) fn capture_local_binding(
    runtime: &Runtime,
    binding: &mut FrameBinding,
    definition: crate::engine::code::function::metadata::VariableDefinition,
    descriptor: ClosureVariable,
) -> Result<VarRefRoot, Error> {
    if let FrameBinding::Captured(var_ref) = binding {
        reuse_frame_capture(
            runtime,
            &VarRefView::from_frame(runtime, *var_ref),
            descriptor,
        )
    } else {
        capture_frame_binding(
            runtime,
            binding,
            ClosureVariable {
                is_lexical: definition.is_lexical,
                is_const: definition.is_const,
                kind: definition.kind,
                ..descriptor
            },
        )
    }
}

/// Preserve the initial TDZ cell or reset the same cell after an abrupt scope
/// exit marked it reusable; normal iteration must detach it with CloseLocal.
pub(in crate::engine::vm) fn reset_captured_binding(
    runtime: &Runtime,
    root: &impl crate::engine::heap::roots::VarRefHandle,
    reusable: bool,
) -> Result<(), Error> {
    let raw = runtime
        .raw_var_ref_value(root)
        .map_err(runtime_error_to_vm_error)?;
    if matches!(raw, RawValue::Uninitialized) {
        // QuickJS creates direct FunctionBody declaration closures
        // before expanding the body scope's lexical TDZ entries. A
        // child may therefore capture this first uninitialized cell
        // before SetLocalUninitialized reaches it; entering that same
        // initial lifetime is a no-op. A live initialized capture still
        // proves that a later lifetime skipped CloseLocal.
        return Ok(());
    }
    if reusable {
        // QuickJS resets the existing VarRef in place when an abrupt
        // completion skipped CloseLocal. Escaped closures therefore
        // observe the next lifetime initialized at this same scope
        // site, including its next private field/method identity.
        runtime
            .reset_var_ref_uninitialized(root)
            .map_err(runtime_error_to_vm_error)?;
        return Ok(());
    }
    Err(Error::internal(
        "captured local entered a new lexical lifetime before CloseLocal",
    ))
}

/// Initialize a published lexical/with binding while preserving captured cells.
pub(in crate::engine::vm) fn initialize_local_binding(
    runtime: &Runtime,
    kind: ClosureVariableKind,
    binding: &mut FrameBinding,
    value: JsValue,
) -> Result<(), Error> {
    if kind == ClosureVariableKind::WithObject && !matches!(value, JsValue::Object(_)) {
        return Err(Error::internal(
            "with-object initialization did not receive an Object",
        ));
    }
    match binding {
        FrameBinding::Direct(slot) => {
            // Overwrite releases the replaced owner and moves the new one in.
            let previous = std::mem::replace(slot, value);
            runtime
                .release_jsvalue(previous)
                .map_err(runtime_error_to_vm_error)?;
            Ok(())
        }
        FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => Err(Error::internal(
            "ordinary lexical initialization reached a private-element frame cell",
        )),
        FrameBinding::Uninitialized => {
            *binding = FrameBinding::Direct(value);
            Ok(())
        }
        FrameBinding::Captured(var_ref) => runtime
            .write_var_ref(&VarRefView::from_frame(runtime, *var_ref), value)
            .map_err(runtime_error_to_vm_error),
    }
}

pub(in crate::engine::vm) fn initialize_derived_closure(
    runtime: &Runtime,
    root: &impl crate::engine::heap::roots::VarRefHandle,
    descriptor: ClosureVariable,
    value: JsValue,
) -> Result<(), Error> {
    use crate::engine::api::error::ErrorKind;
    if !descriptor.is_lexical
        || descriptor.is_const
        || descriptor.kind != ClosureVariableKind::Normal
    {
        return Err(Error::internal(
            "derived this initialization referenced a non-mutable lexical closure",
        ));
    }
    if !matches!(value, JsValue::Object(_)) {
        return Err(Error::internal(
            "derived this initialization did not receive an Object",
        ));
    }
    let raw = runtime
        .raw_var_ref_value(root)
        .map_err(runtime_error_to_vm_error)?;
    if !matches!(raw, RawValue::Uninitialized) {
        // Pinned QuickJS's captured form (`put_var_ref_check_init`) uses
        // the ordinary uninitialized-binding diagnostic here. This
        // intentionally differs from the owning-local opcode's explicit
        // "initialized only once" message.
        return Err(Error::new(ErrorKind::Reference, "this is not initialized"));
    }
    runtime
        .write_var_ref(root, value)
        .map_err(runtime_error_to_vm_error)
}

/// Preserve the dedicated import-collision authority at both VM consumers.
pub(super) fn validate_module_import_collision(descriptor: ClosureVariable) -> Result<(), Error> {
    if descriptor.source
        != crate::engine::code::function::metadata::ClosureSource::ModuleImportCollision
        || !descriptor.is_lexical
        || !descriptor.is_const
        || !matches!(
            descriptor.kind,
            ClosureVariableKind::Normal | ClosureVariableKind::ModuleImportView
        )
    {
        return Err(Error::internal(
            "module import collision initialization targeted a non-import binding",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod layout_tests;

#[cfg(test)]
mod binding_value_tests {
    use super::*;
    use crate::engine::value::Value;

    #[test]
    fn owned_cell_reads_keep_global_and_captured_function_identity() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(
            context
                .eval("let ownedCellGlobal = function() { return 7; };")
                .unwrap(),
        );
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                let f = ownedCellGlobal;
                function get() { return f; }
                if (get() !== ownedCellGlobal) return false;
                f = function() { return 9; };
                return get()() === 9 && ownedCellGlobal() === 7;
            })()
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn immediate_cell_writes_preserve_assignment_results_and_error_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(()=>{
            let x=1; function read(){return x;} function set(v){return x=v;}
            if(set(2)!==2 || read()!==2)return false;
            if((x=3)!==3 || read()!==3)return false;
            set({answer:4}); if(read().answer!==4)return false;
            set(5); if(read()!==5)return false;
            set(null); if(read()!==null)return false;
            set(undefined); if(read()!==undefined)return false;
            set(true); if(read()!==true)return false;
            function mapped(arg){function read(){return arg;} arg=7; return arguments[0]===7 && read()===7;}
            function strict(arg){'use strict'; function read(){return arg;} arg=7; return arguments[0]===1 && read()===7;}
            let trace='';
            try { (()=>later=2)(); let later=1; } catch(e){if(e instanceof ReferenceError)trace+='tdz';}
            const c=1; try { (()=>c=2)(); } catch(e){if(e instanceof TypeError)trace+='const';}
            return mapped(1) && strict(1) && trace==='tdzconst' && c===1;
        })()"#).unwrap(), Value::Bool(true));
        drop(context.eval("let immediateWriteGlobal=1;").unwrap());
        assert_eq!(context.eval(r#"(()=>{
            immediateWriteGlobal=2; let a=immediateWriteGlobal;
            immediateWriteGlobal={answer:3}; let b=immediateWriteGlobal.answer;
            immediateWriteGlobal=4; let c=immediateWriteGlobal;
            let calls=0,last=0;
            Object.defineProperty(globalThis,'cellSetterProbe',{configurable:true,set(v){calls++;last=v;}});
            cellSetterProbe=8; cellSetterProbe=9; delete globalThis.cellSetterProbe;
            return a===2 && b===3 && c===4 && calls===2 && last===9;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn owned_cell_writes_preserve_global_and_captured_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(context.eval("let profileWriteGlobal=1;").unwrap());
        drop(
            context
                .eval("profileWriteGlobal=2;profileWriteGlobal=3;")
                .unwrap(),
        );
        assert_eq!(
            context
                .eval("(()=>{let x=1;function set(v){x=v;}set(2);set(3);return x;})()")
                .unwrap(),
            Value::Int(3)
        );
        assert_eq!(context.eval("profileWriteGlobal").unwrap(), Value::Int(3));
    }

    #[test]
    fn captured_reads_observe_callbacks_eval_arguments_and_tdz() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(context.eval(r#"(()=>{
            let value=1;
            function read(){return value;}
            function mutate(next){value=next;}
            if(read()!==1)return false;
            mutate(2); if(read()!==2)return false;
            eval('value=3'); if(read()!==3)return false;
            mutate({answer:4}); if(read().answer!==4)return false;
            mutate(-0); if(!Object.is(read(),-0))return false;
            mutate(NaN); if(!Number.isNaN(read()))return false;
            function mapped(arg){function inner(){return arg;} arguments[0]=8; return arg===8 && inner()===8;}
            function strict(arg){'use strict'; function inner(){return arg;} arguments[0]=8; return arg===1 && inner()===1;}
            let tdz=false; try { (()=>later)(); let later=1; } catch(e){tdz=e instanceof ReferenceError;}
            const constant=9; function constantRead(){return constant;}
            return mapped(1) && strict(1) && tdz && constantRead()===9;
        })()"#).unwrap(), Value::Bool(true));
        drop(context.eval("let immediateGlobal=1;").unwrap());
        assert_eq!(context.eval(r#"(()=>{
            let first=immediateGlobal;
            immediateGlobal=2;
            let second=immediateGlobal;
            immediateGlobal={answer:3};
            let third=immediateGlobal.answer;
            let calls=0;
            Object.defineProperty(globalThis,'cellGetterProbe',{configurable:true,get(){calls++;return calls;}});
            let a=cellGetterProbe,b=cellGetterProbe;
            delete globalThis.cellGetterProbe;
            return first===1 && second===2 && third===3 && a===1 && b===2;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn captured_reads_preserve_global_and_closure_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(context.eval("let immediateProfileGlobal=7;").unwrap());
        assert_eq!(
            context.eval("immediateProfileGlobal").unwrap(),
            Value::Int(7)
        );
        assert_eq!(
            context
                .eval("(()=>{let x=2;function get(){return x;}return get()+get();})()")
                .unwrap(),
            Value::Int(4)
        );
    }
}
