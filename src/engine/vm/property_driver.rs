//! Schedule prepared property reads without replaying observable key conversion.
//! Storage selection remains in object; VM owns input transfer and child replies.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execute::FallthroughPc,
    execution::RunningExecution,
    frame::{FrameId, ReturnTarget},
};
use crate::engine::{
    api::{Error, runtime::Runtime},
    code::function::metadata::FunctionKind,
    heap::ContextId,
    object::{OrdinaryRead, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
};

#[derive(Clone, Copy)]
pub(super) enum ReadKey {
    Static(u32),
    Computed { keep_key: bool },
}

/// Only Completed proves that the instruction finished in this same frame.
/// Deferred preserves the existing callback/query protocol even if it happens
/// to return Entered after synchronous work.
pub(super) enum PropertyProgress {
    Completed,
    Deferred(CallStep),
}

/// A static read selected while the frame transaction was still active.
/// The read owns its getter/receiver or result until the driver consumes it.
pub(super) enum SelectedNamedRead {
    Getter(OwnedGetterSelection),
    LookupError(Error),
    Shared(crate::engine::builtins::SharedTypedRead),
    Effect(crate::engine::object::StateReadEffect),
}

// New cold effect selections must fit the existing getter edges plus the
// discriminant. They must not widen every RunningExecution just to transport
// an object whose representation is already known.
const _: () = assert!(
    size_of::<SelectedNamedRead>()
        <= size_of::<OwnedGetterSelection>() + align_of::<OwnedGetterSelection>()
);

impl SelectedNamedRead {
    pub(super) fn release(self, runtime: &Runtime) {
        if runtime.skip_cleanup() {
            return;
        }
        let _unwind = runtime.unwind_guard();
        if self
            .release_in_state(&mut runtime.0.state.borrow_mut(), runtime)
            .is_err()
        {
            runtime.0.poisoned.set(true);
        }
    }

    pub(in crate::engine::vm) fn release_in_state(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        runtime: &Runtime,
    ) -> Result<(), Error> {
        let poisoned = &runtime.0.poisoned;
        match self {
            Self::Effect(effect) => effect
                .release_in_state(state, runtime)
                .map_err(runtime_error_to_vm_error),
            Self::Getter(selected) => selected.release(state, poisoned),
            Self::LookupError(_) | Self::Shared(_) => Ok(()),
        }
    }
}

/// A completed getter choice owns only its callee and receiver edges. Selection
/// does not build a public root or replay lookup at the legacy boundary.
pub(super) struct OwnedGetterSelection {
    getter: JsValue,
    receiver: JsValue,
}

impl OwnedGetterSelection {
    pub(super) fn from_parts(callee: crate::engine::heap::ObjectId, receiver: JsValue) -> Self {
        Self {
            getter: JsValue::Object(callee),
            receiver,
        }
    }
    pub(in crate::engine::vm) fn getter_id(&self) -> crate::engine::heap::ObjectId {
        let JsValue::Object(getter) = self.getter else {
            unreachable!("selected getter owns an object")
        };
        getter
    }

    /// The execution record pins both edges until the installer has finished
    /// every fallible reservation and initialization.
    pub(in crate::engine::vm) fn into_parts(self) -> (crate::engine::heap::ObjectId, JsValue) {
        (self.getter_id(), self.receiver)
    }

    pub(super) fn prepare(
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        receiver: &JsValue,
        getter: crate::engine::heap::ObjectId,
    ) -> Result<Self, Error> {
        let effect = crate::engine::object::StateReadEffect::prepare(
            state,
            poisoned,
            crate::engine::object::ReadBoundary::Getter(getter),
            receiver,
        )
        .map_err(runtime_error_to_vm_error)?;
        let crate::engine::object::StateReadEffect::Getter { callee, receiver } = effect else {
            unreachable!("getter selection produces a getter effect")
        };
        Ok(Self {
            getter: JsValue::Object(callee),
            receiver,
        })
    }

    pub(super) fn release(
        self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<(), Error> {
        state
            .release_owned_jsvalue(poisoned, self.getter)
            .map_err(runtime_error_to_vm_error)?;
        state
            .release_owned_jsvalue(poisoned, self.receiver)
            .map_err(runtime_error_to_vm_error)
    }

    fn into_legacy_read(self, runtime: &Runtime) -> OrdinaryRead {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "core.legacy_boundary.selected_getter_root",
        );
        let JsValue::Object(id) = self.getter else {
            unreachable!("selected getter is an object")
        };
        OrdinaryRead::Call {
            getter: crate::engine::object::CallableRef::from_validated_object(
                crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), id),
            ),
            receiver: self.receiver,
        }
    }
}

struct SelectedReadGuard<'a> {
    runtime: &'a Runtime,
    read: Option<OrdinaryRead>,
}

struct NamedHandoffGuard<'a> {
    runtime: &'a Runtime,
    selected: Option<SelectedNamedRead>,
}

impl Drop for NamedHandoffGuard<'_> {
    fn drop(&mut self) {
        if let Some(selected) = self.selected.take() {
            selected.release(self.runtime);
        }
    }
}

#[cold]
#[inline(never)]
fn publish_named_effect(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
    selected: &mut NamedHandoffGuard<'_>,
) -> Result<PropertyProgress, Error> {
    let depth = {
        let mut state = runtime.0.state.borrow_mut();
        let frame = execution.frames.current_mut(id)?;
        let depth = execution.slots.depth(&frame.window);
        let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
        if !keep_receiver {
            let value = transaction.slots().pop()?;
            state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
        }
        frame.resume_pc = fallthrough.index();
        depth
    };
    let Some(SelectedNamedRead::Effect(effect)) = selected.selected.take() else {
        unreachable!("published named read has an exact effect");
    };
    super::proxy_get_driver::start_property_state_read(runtime, execution, id, effect, None, depth)
        .map(PropertyProgress::Deferred)
}

impl Drop for SelectedReadGuard<'_> {
    fn drop(&mut self) {
        if let Some(read) = self.read.take() {
            read.release(self.runtime);
        }
    }
}

impl PropertyProgress {
    pub(super) fn into_call_step(self) -> CallStep {
        match self {
            Self::Completed => CallStep::Entered,
            Self::Deferred(step) => step,
        }
    }
}

/// Converted inputs stay owned after ToPrimitive's reply, even if lookup next
/// reaches a Proxy or a callable whose domain continuation is still pending.
pub(super) struct ConvertedRead {
    pub base: JsValue,
    pub key: JsValue,
    pub keep_receiver: bool,
    pub keep_key: bool,
}

pub(super) fn throw_error(
    runtime: &Runtime,
    realm: ContextId,
    error: Error,
) -> Result<CallStep, Error> {
    let Some(kind) =
        crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
    else {
        return Err(error);
    };
    Ok(CallStep::Complete(Completion::Throw(
        runtime
            .new_native_error_from_error_jsvalue(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    )))
}

pub(super) fn read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key_kind: ReadKey,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<CallStep, Error> {
    read_progress(runtime, execution, id, key_kind, keep_receiver, fallthrough)
        .map(PropertyProgress::into_call_step)
}

pub(super) fn read_progress(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key_kind: ReadKey,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
) -> Result<PropertyProgress, Error> {
    read_progress_selected(
        runtime,
        execution,
        id,
        key_kind,
        keep_receiver,
        fallthrough,
        None,
    )
}

pub(super) fn read_progress_selected(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key_kind: ReadKey,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
    selected: Option<SelectedNamedRead>,
) -> Result<PropertyProgress, Error> {
    let mut selected = NamedHandoffGuard { runtime, selected };
    if matches!(selected.selected, Some(SelectedNamedRead::Effect(_))) {
        return publish_named_effect(
            runtime,
            execution,
            id,
            keep_receiver,
            fallthrough,
            &mut selected,
        );
    }
    let frame = execution.frames.current_mut(id)?;
    let computed = matches!(key_kind, ReadKey::Computed { .. });
    let realm = frame.executable.realm;
    let next_pc = fallthrough.index();
    let selected_read = match selected.selected.take() {
        Some(SelectedNamedRead::Getter(read)) => Some(read.into_legacy_read(runtime)),
        Some(SelectedNamedRead::Shared(_)) => {
            return Err(Error::internal(
                "shared named read reached the legacy read adapter",
            ));
        }
        Some(SelectedNamedRead::LookupError(error)) => {
            return throw_error(runtime, realm, error).map(PropertyProgress::Deferred);
        }
        Some(SelectedNamedRead::Effect(_)) => {
            unreachable!("exact named effect handled before legacy read")
        }
        None => None,
    };
    let mut selected_read = SelectedReadGuard {
        runtime,
        read: selected_read,
    };
    if let ReadKey::Static(index) = key_kind
        && selected_read.read.is_none()
    {
        use super::stack::LinkedReadCompletion;
        let body = &mut *frame.cold;
        let executable = &*body.executable;
        let depth = execution.slots.depth(&body.window);
        let mut preserved_receiver = None;
        let mut retained_key = None;
        let result = execution.slots.with_linked_own_read_selected(
            &mut body.window,
            runtime,
            executable,
            index,
            None,
            |slots, value| {
                // Lookup has finished and retained the result. Move the base
                // owner into the enclosing driver scope before publication.
                preserved_receiver = Some(slots.pop()?);
                publish_read_result(
                    slots,
                    &mut frame.resume_pc,
                    next_pc,
                    &mut preserved_receiver,
                    &mut retained_key,
                    keep_receiver,
                    value,
                )?;
                Ok(())
            },
        );
        match result {
            Ok(LinkedReadCompletion::Completed) => {
                record_read_completion(depth);
                if !keep_receiver && let Some(value) = preserved_receiver.take() {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                }
                return Ok(PropertyProgress::Completed);
            }
            Ok(LinkedReadCompletion::Pending(read)) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "property_lookup_handoff_selected",
                );
                selected_read.read = Some(read);
            }
            Ok(LinkedReadCompletion::Declined) => {}
            Ok(LinkedReadCompletion::LookupError(error)) => {
                return throw_error(runtime, realm, error).map(PropertyProgress::Deferred);
            }
            Err(error) => {
                runtime
                    .update_active_bytecode_pc(
                        frame.active_frame,
                        super::BytecodePc::new(frame.fault_pc),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                if let Some(value) = retained_key.take() {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                }
                if let Some(value) = preserved_receiver.take() {
                    runtime
                        .release_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?;
                }
                return Err(error);
            }
        }
    }
    let base = execution.slots.peek(&frame.window, usize::from(computed))?;
    if computed && matches!(base, JsValue::Null | JsValue::Undefined) {
        let key = execution.slots.peek(&frame.window, 0)?;
        let message = if matches!(key_kind, ReadKey::Computed { keep_key: true })
            && !matches!(
                key,
                JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_)
            ) {
            "value has no property"
        } else if matches!(base, JsValue::Null) {
            "cannot read property of null"
        } else {
            "cannot read property of undefined"
        };
        return throw_error(
            runtime,
            realm,
            Error::new(crate::engine::api::error::ErrorKind::Type, message),
        )
        .map(PropertyProgress::Deferred);
    }
    let depth = execution.slots.depth(&frame.window);
    enum SelectedKey<'a> {
        Borrowed(&'a PropertyKey),
        Owned(PropertyKey),
    }
    let (key, retained_key) = match key_kind {
        ReadKey::Static(index) => {
            let Some(atom) = frame
                .executable
                .property_key_atoms
                .as_ref()
                .and_then(|atoms| atoms.get(index as usize))
                .copied()
                .filter(|atom| !atom.is_null())
            else {
                return Err(Error::internal("property read has no linked key"));
            };
            let cache = &mut frame.cold.property_keys;
            let key = match cache.entry(index) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                    PropertyKey::from_borrowed_atom(runtime.clone(), atom)
                        .map_err(|error| Error::internal(error.to_string()))?,
                ),
            };
            let key = Some(SelectedKey::Borrowed(&*key));
            (key, None)
        }
        ReadKey::Computed { keep_key } => {
            let value = execution.slots.peek(&frame.window, 0)?;
            if matches!(value, JsValue::Object(_)) {
                return Err(Error::internal(
                    "object property key did not enter its conversion operation",
                ));
            }
            let owned = runtime
                .dup_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?;
            let key = match runtime
                .native_to_property_key_jsvalue(realm, owned)
                .map_err(runtime_error_to_vm_error)?
            {
                NativeConversion::Value(key) => key,
                NativeConversion::Throw(value) => {
                    return Ok(PropertyProgress::Deferred(CallStep::Complete(
                        Completion::Throw(value),
                    )));
                }
            };
            let retained = keep_key
                .then(|| match value {
                    JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_) => runtime
                        .dup_jsvalue(value)
                        .map_err(runtime_error_to_vm_error),
                    value => Ok(super::numeric::allocate_string_jsvalue(
                        runtime,
                        super::numeric::to_js_string_jsvalue(runtime, value)?,
                    )?),
                })
                .transpose()?;
            (Some(SelectedKey::Owned(key)), retained)
        }
    };
    // Lookup borrows the original rooted operand. Only a pending callback
    // needs a second receiver owner; completed reads move this slot directly.
    let read = match selected_read.read.take().map(Ok).unwrap_or_else(|| {
        runtime.prepare_value_property_read_selected_jsvalue(
            realm,
            base,
            key.as_ref()
                .map(|key| match key {
                    SelectedKey::Borrowed(key) => *key,
                    SelectedKey::Owned(key) => key,
                })
                .ok_or(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "fallback read lost its key",
                ))?,
            None,
        )
    }) {
        Ok(read) => read,
        Err(error) => {
            return throw_error(runtime, realm, runtime_error_to_vm_error(error))
                .map(PropertyProgress::Deferred);
        }
    };
    let key = if matches!(read, OrdinaryRead::Special { .. }) {
        key.map(|key| match key {
            SelectedKey::Borrowed(key) => key.try_clone(),
            SelectedKey::Owned(key) => Ok(key),
        })
        .transpose()?
    } else {
        None
    };
    #[cfg(feature = "profiling")]
    if matches!(key_kind, ReadKey::Static(_)) && matches!(read, OrdinaryRead::Complete(_)) {
        use crate::engine::heap::ObjectKind;
        use crate::engine::heap::SlotReleaseReadiness;
        let kind = match base {
            JsValue::Object(_) => "driver_named_complete.object",
            JsValue::String(_) => "driver_named_complete.string",
            JsValue::Null | JsValue::Undefined => "driver_named_complete.nullish",
            _ => "driver_named_complete.primitive",
        };
        crate::engine::api::profiling::record_owned_execution_event(kind);
        if let JsValue::Object(id) = base
            && let Ok(state) = runtime.0.state.try_borrow()
            && let Ok(object) = state.heap.object(*id)
        {
            let kind = match object.kind {
                ObjectKind::Array => "driver_named_complete.kind_array",
                ObjectKind::NativeFunction
                | ObjectKind::BoundFunction
                | ObjectKind::BytecodeFunction => "driver_named_complete.kind_function",
                ObjectKind::Ordinary => "driver_named_complete.kind_ordinary",
                _ => "driver_named_complete.kind_other",
            };
            crate::engine::api::profiling::record_owned_execution_event(kind);
        }
        if keep_receiver {
            crate::engine::api::profiling::record_owned_execution_event(
                "driver_named_complete.keep_receiver",
            );
        }
        if !keep_receiver {
            let readiness = match runtime.slot_value_release_readiness_jsvalue(base) {
                Ok(SlotReleaseReadiness::Ready) => "driver_named_complete.release_ready",
                Ok(SlotReleaseReadiness::QueueCapacity) => {
                    "driver_named_complete.release_queue_capacity"
                }
                Ok(SlotReleaseReadiness::Drain) => "driver_named_complete.release_drain",
                Ok(SlotReleaseReadiness::Deferred) => "driver_named_complete.release_deferred",
                Ok(SlotReleaseReadiness::Borrowed) => "driver_named_complete.release_borrowed",
                Ok(SlotReleaseReadiness::PrimitiveStorage) => {
                    "driver_named_complete.release_primitive_storage"
                }
                Err(_) => "driver_named_complete.release_error",
            };
            crate::engine::api::profiling::record_owned_execution_event(readiness);
        }
    }
    match read {
        OrdinaryRead::Complete(value) => complete_read(
            runtime,
            execution,
            id,
            None,
            retained_key,
            keep_receiver,
            1 + usize::from(computed),
            value.unwrap_or(JsValue::Undefined),
            depth,
            next_pc,
        )
        .map(|()| PropertyProgress::Completed),
        read => {
            let preserved_receiver = runtime
                .dup_jsvalue(base)
                .map_err(runtime_error_to_vm_error)?;
            read_pending(
                runtime,
                execution,
                id,
                preserved_receiver,
                key,
                read,
                retained_key,
                keep_receiver,
                1 + usize::from(computed),
                depth,
            )
            .map(PropertyProgress::Deferred)
        }
    }
}

// The conversion reply already owns this boxed operand bundle; avoid moving it through the driver stack.
#[allow(clippy::boxed_local)]
pub(super) fn read_converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    input: Box<ConvertedRead>,
) -> Result<CallStep, Error> {
    let ConvertedRead {
        base,
        key,
        keep_receiver,
        keep_key,
    } = *input;
    if matches!(key, JsValue::Object(_)) {
        return Err(Error::internal(
            "ToPrimitive returned an object property key",
        ));
    }
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window) + 2;
    // After an object-key conversion, GetArrayEl3 retains String/Symbol, even
    // if ToPrimitive returned an Int. Direct Int keys retain their original tag.
    let retained = if keep_key {
        Some(match &key {
            JsValue::Symbol(_) | JsValue::String(_) => runtime
                .dup_jsvalue(&key)
                .map_err(runtime_error_to_vm_error)?,
            value => super::numeric::allocate_string_jsvalue(
                runtime,
                super::numeric::to_js_string_jsvalue(runtime, value)?,
            )?,
        })
    } else {
        None
    };
    let key = match runtime
        .native_to_property_key_jsvalue(realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
    };
    finish_read(
        runtime,
        execution,
        id,
        base,
        key,
        retained,
        keep_receiver,
        0,
        depth,
    )
    .map(PropertyProgress::into_call_step)
}

#[allow(clippy::too_many_arguments)]
fn finish_read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    base: JsValue,
    key: PropertyKey,
    retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<PropertyProgress, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let read = match runtime.prepare_value_property_read_borrowed_jsvalue(realm, &base, &key) {
        Ok(read) => read,
        Err(error) => {
            return throw_error(runtime, realm, runtime_error_to_vm_error(error))
                .map(PropertyProgress::Deferred);
        }
    };
    read_prepared_progress(
        runtime,
        execution,
        id,
        base,
        key,
        read,
        retained_key,
        keep_receiver,
        consume,
        depth,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read_prepared(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    preserved_receiver: JsValue,
    key: PropertyKey,
    read: OrdinaryRead,
    retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<CallStep, Error> {
    read_prepared_progress(
        runtime,
        execution,
        id,
        preserved_receiver,
        key,
        read,
        retained_key,
        keep_receiver,
        consume,
        depth,
    )
    .map(PropertyProgress::into_call_step)
}

#[allow(clippy::too_many_arguments)]
fn read_prepared_progress(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    preserved_receiver: JsValue,
    key: PropertyKey,
    read: OrdinaryRead,
    retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<PropertyProgress, Error> {
    match read {
        OrdinaryRead::Complete(value) => complete_read_recovering(
            runtime,
            execution,
            id,
            Some(preserved_receiver),
            retained_key,
            keep_receiver,
            consume,
            value.unwrap_or(JsValue::Undefined),
            depth,
        )
        .map(|()| PropertyProgress::Completed),
        read => read_pending(
            runtime,
            execution,
            id,
            preserved_receiver,
            Some(key),
            read,
            retained_key,
            keep_receiver,
            consume,
            depth,
        )
        .map(PropertyProgress::Deferred),
    }
}

// Keep the no-callback path out of the callback dispatcher's large native frame.
#[allow(clippy::too_many_arguments)]
fn complete_read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mut preserved_receiver: Option<JsValue>,
    mut retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    value: JsValue,
    depth: usize,
    next_pc: usize,
) -> Result<(), Error> {
    if consume > 2 || (preserved_receiver.is_none() && consume == 0) {
        return Err(Error::internal("property read consumes too many operands"));
    }
    let mut value = Some(value);
    let frame = execution.frames.current_mut(id)?;
    let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
    let discarded = {
        let mut slots = transaction.slots();
        // Moving the base preserves its owner until after result publication.
        // A scalar key's release cannot free arena storage or drain deferred
        // GC; heap-backed keys take the outside-window release below.
        let immediate_key = preserved_receiver.is_none()
            && (consume == 1
                || matches!(
                    slots.peek(0)?,
                    JsValue::Undefined
                        | JsValue::Null
                        | JsValue::Bool(_)
                        | JsValue::Int(_)
                        | JsValue::Float(_)
                        | JsValue::ShortBigInt(_)
                ));
        let mut discarded = [None, None];
        for destination in discarded.iter_mut().take(consume) {
            *destination = Some(slots.pop()?);
        }
        if preserved_receiver.is_none() {
            preserved_receiver = discarded[consume - 1].take();
        }
        if immediate_key {
            for slot in discarded.iter_mut() {
                if let Some(taken) = slot.take() {
                    runtime
                        .release_jsvalue(taken)
                        .map_err(runtime_error_to_vm_error)?;
                }
            }
            publish_read_result(
                &mut slots,
                &mut frame.resume_pc,
                next_pc,
                &mut preserved_receiver,
                &mut retained_key,
                keep_receiver,
                &mut value,
            )?;
            record_read_completion(depth);
            let _ = slots;
            if !keep_receiver && let Some(receiver) = preserved_receiver.take() {
                runtime
                    .release_jsvalue(receiver)
                    .map_err(runtime_error_to_vm_error)?;
            }
            return Ok(());
        }
        discarded
    };
    // Preserve original pop/release order outside FrameSlots for owning keys
    // and externally prepared reads. The base and normalized key stay owned.
    for slot in discarded.into_iter().flatten() {
        runtime
            .release_jsvalue(slot)
            .map_err(runtime_error_to_vm_error)?;
    }
    let mut slots = transaction.slots();
    publish_read_result(
        &mut slots,
        &mut frame.resume_pc,
        next_pc,
        &mut preserved_receiver,
        &mut retained_key,
        keep_receiver,
        &mut value,
    )?;
    record_read_completion(depth);
    let _ = slots;
    if !keep_receiver && let Some(receiver) = preserved_receiver.take() {
        runtime
            .release_jsvalue(receiver)
            .map_err(runtime_error_to_vm_error)?;
    }
    Ok(())
}

// Converted and super-property replies have not yet transported the decoded
// position. Keep reconstruction explicit at this unmigrated entry point.
#[allow(clippy::too_many_arguments)]
fn complete_read_recovering(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    preserved_receiver: Option<JsValue>,
    retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    value: JsValue,
    depth: usize,
) -> Result<(), Error> {
    // Match complete_read's admission order before touching frame metadata.
    if consume > 2 || (preserved_receiver.is_none() && consume == 0) {
        return Err(Error::internal("property read consumes too many operands"));
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(
        "property_legacy_fallthrough_recovery_decode",
    );
    let next_pc = execution.frames.current_mut(id)?.next_pc()?;
    complete_read(
        runtime,
        execution,
        id,
        preserved_receiver,
        retained_key,
        keep_receiver,
        consume,
        value,
        depth,
        next_pc,
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn publish_read_result(
    slots: &mut super::stack::FrameSlots<'_>,
    resume_pc: &mut usize,
    next_pc: usize,
    preserved_receiver: &mut Option<JsValue>,
    retained_key: &mut Option<JsValue>,
    keep_receiver: bool,
    value: &mut Option<JsValue>,
) -> Result<(), Error> {
    if keep_receiver {
        slots.push_pending(preserved_receiver)?;
    }
    if retained_key.is_some() {
        slots.push_pending(retained_key)?;
    }
    *resume_pc = next_pc;
    slots.push_pending(value)?;
    Ok(())
}

#[inline]
fn record_read_completion(_depth: usize) {
    #[cfg(feature = "profiling")]
    {
        crate::engine::api::profiling::record_owned_instruction(_depth);
        crate::engine::api::profiling::record_owned_execution_event(
            "property_read_completed_directly",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn read_pending(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    preserved_receiver: JsValue,
    key: Option<PropertyKey>,
    read: OrdinaryRead,
    retained_key: Option<JsValue>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let mut request = None;
    let mut ordinary_callback = None;
    let mut proxy = None;
    let mut proxy_callback = None;
    let mut native_callback = None;
    let value = match read {
        OrdinaryRead::Complete(value) => Some(value.unwrap_or(JsValue::Undefined)),
        OrdinaryRead::Call { getter, receiver } => {
            if let Some(call) =
                super::call::ordinary::OrdinaryCall::select_callback(runtime, getter.as_object())
                    .map_err(runtime_error_to_vm_error)?
            {
                if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                    call.executable()
                        .ensure_root(runtime)
                        .map_err(runtime_error_to_vm_error)?;
                    return runtime
                        .bytecode_stack_overflow_completion(
                            realm,
                            call.executable().root().expect("rooted overflow frame"),
                        )
                        .map(CallStep::Complete)
                        .map_err(runtime_error_to_vm_error);
                }
                ordinary_callback = Some((call, receiver));
            } else {
                let super::call::NormalizedCallback {
                    callable,
                    receiver,
                    arguments,
                    classification,
                } = match super::call::normalize_callback(
                    runtime,
                    realm,
                    getter,
                    receiver,
                    Vec::new(),
                )? {
                    NativeConversion::Value(call) => call,
                    NativeConversion::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                };
                let normal = match &classification {
                    CallableExecution::Bytecode { bytecode, .. } => {
                        let state = runtime.0.state.borrow();
                        state
                            .heap
                            .function_bytecode(bytecode.bytecode_id())
                            .map_err(|error| Error::internal(error.to_string()))?
                            .metadata
                            .function_kind
                            == FunctionKind::Normal
                    }
                    _ => false,
                };
                let is_proxy = matches!(classification, CallableExecution::Proxy);
                if let CallableExecution::Bytecode {
                    bytecode,
                    closure_slots,
                } = classification
                    && normal
                {
                    if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                        return runtime
                            .bytecode_stack_overflow_completion(realm, &bytecode)
                            .map(CallStep::Complete)
                            .map_err(runtime_error_to_vm_error);
                    }
                    request = Some(BytecodeCallRequest {
                        callable,
                        receiver,
                        arguments,
                        new_target: JsValue::Undefined,
                        bytecode,
                        closure_slots,
                        caller_realm: realm,
                        return_to: ReturnTarget {
                            value_use: super::frame::ReturnValue::Push,
                            owner: crate::engine::vm::frame::ReturnOwner::Frame(id),
                            tail: false,
                            operation: None,
                        },
                    });
                } else if is_proxy {
                    proxy_callback = Some((callable, receiver, arguments));
                } else {
                    native_callback = Some((callable, receiver, arguments));
                }
            }
            None
        }
        OrdinaryRead::Special {
            object, receiver, ..
        } => {
            // The Proxy protocol owns the remaining lookup stages. Earlier
            // key conversion stays consumed when its callbacks suspend.
            proxy = Some((
                object,
                key.ok_or_else(|| Error::internal("Proxy read lost key"))?,
                receiver,
            ));
            None
        }
    };
    let frame = execution.frames.current_mut(id)?;
    for _ in 0..consume {
        let operand = execution.slots.pop(&mut frame.cold.window)?;
        runtime
            .release_jsvalue(operand)
            .map_err(runtime_error_to_vm_error)?;
    }
    if keep_receiver {
        execution
            .slots
            .push(&mut frame.cold.window, preserved_receiver)?;
    } else {
        runtime
            .release_jsvalue(preserved_receiver)
            .map_err(runtime_error_to_vm_error)?;
    }
    if let Some(key) = retained_key {
        execution.slots.push(&mut frame.cold.window, key)?;
    }
    if let Some((object, key, receiver)) = proxy {
        return super::proxy_get_driver::start(
            runtime, execution, id, object, key, receiver, depth,
        );
    }
    if let Some((callable, receiver, arguments)) = proxy_callback {
        return super::proxy_get_driver::start_call(
            runtime,
            execution,
            id,
            callable.as_object().try_clone()?,
            receiver,
            arguments,
            false,
            depth,
        );
    }
    if let Some((callable, receiver, arguments)) = native_callback {
        return super::proxy_get_driver::start_callback_call(
            runtime, execution, id, callable, receiver, arguments, false, depth,
        );
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(
        "property_legacy_fallthrough_recovery_decode",
    );
    frame.resume_pc = frame.next_pc()?;
    if let Some((call, receiver)) = ordinary_callback {
        let entry = call.prepare_callback(
            runtime,
            &mut execution.call_storage,
            receiver,
            Vec::new(),
            realm,
            ReturnTarget {
                value_use: super::frame::ReturnValue::Push,
                owner: super::frame::ReturnOwner::Frame(id),
                tail: false,
                operation: None,
            },
        )?;
        push_frame(runtime, execution, entry)?;
    } else if let Some(request) = request {
        let entry = request.prepare(runtime, &mut execution.call_storage)?;
        push_frame(runtime, execution, entry)?;
    } else {
        execution.slots.push(
            &mut frame.cold.window,
            value.ok_or_else(|| Error::internal("property result missing"))?,
        )?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(CallStep::Entered)
}

#[cfg(test)]
pub(in crate::engine::vm) mod read_completion_tests {
    use crate::engine::api::{Runtime, Value};

    use super::*;
    use crate::engine::{
        code::exec_opcode::Opcode,
        vm::{
            call::CallableExecution,
            execute::{VmAction, execute_frame},
            execution::ExecutionLimits,
            frame::{ColdFrame, FrameCold, FrameEntry, count_next_pc_calls},
            stack::FrameStorage,
        },
    };

    pub(in crate::engine::vm) fn read_fixture(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        source: &str,
        opcode: Opcode,
    ) -> (RunningExecution, FrameId) {
        let Value::Object(function) = context.eval(source).unwrap() else {
            panic!("fixture must evaluate to a function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("fixture must be bytecode");
        };
        let mut prepared = runtime
            .prepare_bytecode_frame(&callable, Value::Undefined, Value::Undefined, &[], bytecode)
            .unwrap();
        let locals = prepared.locals.len();
        let entry = FrameEntry {
            initialize_bindings: false,
            executable: prepared.executable,
            property_generation: 0,
            iterator_generation: 0,
            caller_realm: context.realm,
            active_frame: prepared.active_frame.token(),
            cold: ColdFrame::new(FrameCold {
                rare: std::cell::OnceCell::new(),
                return_to: None,
                entry_guard: Some(prepared.active_frame.into_internal()),
                function: crate::engine::vm::closure::FrameFunction::new(function, closure_slots)
                    .unwrap()
                    .into(),
                reusable_captured_locals: vec![false; locals],
                input: prepared.input.take().into(),
            }),
            storage: FrameStorage {
                original_arguments: vec![],
                parameters: prepared.arguments,
                locals: prepared.locals,
                operands: vec![],
            },
        };
        let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
        let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(id).unwrap();
        let exec = &frame.executable.exec;
        let published = (0..exec.instruction_len())
            .filter_map(|source| exec.exec_pc(source as u32))
            .map(|pc| (pc, exec.decode(pc).unwrap().opcode))
            .collect::<Vec<_>>();
        frame.resume_pc = published
            .iter()
            .find(|(_, actual)| *actual == opcode)
            .map(|(pc, _)| *pc)
            .unwrap_or_else(|| panic!("fixture missing {opcode:?}: {published:?}"))
            as usize;
        (execution, id)
    }

    #[test]
    fn full_computed_window_keeps_nullish_throw_without_converting_key() {
        for (source, opcode) in [
            ("(function(o,k){return o[k]})", Opcode::GetArrayElDense),
            ("(function(o,k){return o[k]()})", Opcode::GetArrayEl2Dense),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let key = runtime.into_jsvalue(context.eval(
                "globalThis.keyConversions=0; ({[Symbol.toPrimitive](){keyConversions++;throw 99}})"
            ).unwrap()).unwrap();
            let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
            let frame = execution.frames.current_mut(id).unwrap();
            while execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_ok()
            {}
            assert!(execution.slots.pop(&mut frame.window).is_ok());
            assert!(execution.slots.pop(&mut frame.window).is_ok());
            execution
                .slots
                .push(&mut frame.window, JsValue::Null)
                .unwrap();
            execution.slots.push(&mut frame.window, key).unwrap();
            let depth = execution.slots.depth(&frame.window);
            let decoded = frame
                .executable
                .exec
                .decode_published(frame.resume_pc as u32)
                .unwrap();
            let fallthrough = FallthroughPc::from_decoded(decoded);
            let result = read_progress(
                &runtime,
                &mut execution,
                id,
                ReadKey::Computed { keep_key: false },
                opcode == Opcode::GetArrayEl2Dense,
                fallthrough,
            )
            .unwrap();
            let PropertyProgress::Deferred(CallStep::Complete(Completion::Throw(error))) = result
            else {
                panic!("original TypeError must survive a full operand window");
            };
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(execution.slots.depth(&frame.window), depth);
            assert!(matches!(
                execution.slots.peek(&frame.window, 1),
                Ok(JsValue::Null)
            ));
            drop(execution);
            let value = runtime.root_and_release_jsvalue(error).unwrap();
            let Value::Object(error) = value else {
                panic!("Error object");
            };
            assert_eq!(
                context
                    .get_property(&error, &runtime.intern_property_key("name").unwrap())
                    .unwrap(),
                Value::String(crate::engine::value::JsString::from_static("TypeError"))
            );
            assert_eq!(context.eval("keyConversions").unwrap(), Value::Int(0));
            assert!(!runtime.is_poisoned());
        }
    }

    #[test]
    fn getterless_named_read_finishes_in_the_active_frame_scope() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context
            .eval("globalThis.localRead = {}; Object.defineProperty(localRead, 'x', {get: undefined}); localRead")
            .unwrap()
        else {
            panic!("object");
        };
        let _other_owner = object.try_clone().expect("duplicate root");
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert!(execution.selected_named_read.is_none());
    }

    #[test]
    fn selected_named_getter_runs_in_the_current_state_segment() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context
            .eval("globalThis.pendingRead = {get x(){ return 7 }}; pendingRead")
            .unwrap()
        else {
            panic!("object");
        };
        let _other_owner = object.try_clone().expect("duplicate root");
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        #[cfg(feature = "profiling")]
        let _scope = crate::engine::api::profiling::CoreExecutionScope::enter();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert_eq!(execution.pending, Some(JsValue::Int(7)));
        assert!(execution.selected_named_read.is_none());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(
                events.get("named_read.getter_entered_in_state").copied(),
                Some(1)
            );
            assert_eq!(events.get("query.read.acquired").copied().unwrap_or(0), 0);
            assert_eq!(events.get("core.runtime_clone").copied().unwrap_or(0), 0);
        }
    }

    #[test]
    fn bound_named_getter_preserves_its_selected_effect_boundary() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context
            .eval("Object.defineProperty({}, 'x', {get:(function(){return this.x}).bind({x:7})})")
            .unwrap()
        else {
            panic!("object")
        };
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::GetField { .. }
        ));
        assert!(matches!(
            execution.selected_named_read,
            Some(SelectedNamedRead::Getter(_))
        ));
    }

    #[test]
    fn unresolved_named_read_carries_the_selected_proxy_holder() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context
            .eval("globalThis.generalRead = new Proxy({x:7}, {get(t,k,r){return Reflect.get(t,k,r)}}); generalRead")
            .unwrap()
        else {
            panic!("object");
        };
        let _other_owner = object.try_clone().expect("duplicate root");
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::GetField { .. }
        ));
        assert!(matches!(
            execution.selected_named_read,
            Some(SelectedNamedRead::Effect(
                crate::engine::object::StateReadEffect::Get(_)
            ))
        ));
    }

    #[test]
    fn synchronous_proxy_named_read_completes_without_transport() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context
            .eval("({__proto__:new Proxy(new Proxy({x:42},{}),{})})")
            .unwrap()
        else {
            panic!("object")
        };
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        let frame = execution.frames.current_mut(id).unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert_eq!(execution.pending, Some(JsValue::Int(42)));
        assert!(execution.selected_named_read.is_none());
        assert_eq!(owners, std::rc::Rc::strong_count(&runtime.0));
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
        assert!(!runtime.0.deferred_references.has_pending());
        #[cfg(feature = "profiling")]
        for name in [
            "query.read.acquired",
            "get_resume_allocation",
            "method_resume_allocation",
            "core.runtime_clone",
        ] {
            assert_eq!(
                profile
                    .snapshot()
                    .owned_execution_events
                    .get(name)
                    .copied()
                    .unwrap_or(0),
                0,
                "{name}"
            );
        }
    }

    #[test]
    fn selected_accessors_and_general_reads_run_their_effects_once() {
        for (source, expected) in [
            (
                "(function(){let n=0;let o={get x(){return ++n}};function read(o){return o.x}let a=read(o),b=read(o);return a*100+b*10+n})()",
                122,
            ),
            (
                "(function(){let n=0;let o={get x(){return ++n}};function read(o){let r=o;return r.x}let a=read(o),b=read(o);return a*100+b*10+n})()",
                122,
            ),
            (
                "(function(){let n=0;let o={x:4};function read(o){return o.x}let a=read(o);Object.defineProperty(o,'x',{get(){n++;return 7}});let b=read(o);return a*100+b*10+n})()",
                471,
            ),
            (
                "(function(){let n=0;let o=new Proxy({x:7},{get(t,k,r){n++;return Reflect.get(t,k,r)}});function read(o){return o.x}let a=read(o),b=read(o);return a*100+b*10+n})()",
                772,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().expect("create context");
            assert_eq!(context.eval(source).unwrap(), Value::Int(expected));
        }
    }

    #[test]
    fn property_actions_complete_at_carried_boundary_without_recovery() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for (
            source,
            object_source,
            opcode,
            computed,
            expected_receiver,
            expected_key,
            expected_depth,
        ) in [
            (
                "(function(o,k){return o[k]})",
                "({x:7,true:5})",
                Opcode::GetArrayElDense,
                true,
                false,
                false,
                1,
            ),
            (
                "(function(o,k){return o[k]()})",
                "({x:7,true:5})",
                Opcode::GetArrayEl2Dense,
                true,
                true,
                false,
                2,
            ),
            (
                "(function(o,k){return o[k]++})",
                "({x:7,true:5})",
                Opcode::GetArrayEl3Dense,
                true,
                true,
                true,
                3,
            ),
        ] {
            let Value::Object(object) = context.eval(object_source).unwrap() else {
                panic!("object");
            };
            let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
            let frame = execution.frames.current_mut(id).unwrap();
            execution
                .slots
                .push(&mut frame.window, JsValue::Object(object.into_handle()))
                .unwrap();
            if computed {
                execution
                    .slots
                    .push(&mut frame.window, JsValue::Bool(true))
                    .unwrap();
            }
            let action = execute_frame(&runtime, &mut execution, id).unwrap();
            let (key, keep_receiver, fallthrough) = match action {
                VmAction::GetField {
                    index,
                    keep_receiver,
                    fallthrough,
                } => (ReadKey::Static(index), keep_receiver, fallthrough),
                VmAction::GetElement {
                    keep_receiver,
                    keep_key,
                    fallthrough,
                } => (ReadKey::Computed { keep_key }, keep_receiver, fallthrough),
                _ => panic!("{source} produced {action:?} instead of a property action"),
            };
            assert_eq!(keep_receiver, expected_receiver);
            assert_eq!(
                matches!(key, ReadKey::Computed { keep_key: true }),
                expected_key
            );
            let fault = execution.frames.current_mut(id).unwrap().fault_pc;
            let (progress, recovery_calls) = count_next_pc_calls(|| {
                read_progress(
                    &runtime,
                    &mut execution,
                    id,
                    key,
                    keep_receiver,
                    fallthrough,
                )
            });
            assert!(matches!(progress.unwrap(), PropertyProgress::Completed));
            assert_eq!(recovery_calls, 0);
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(
                (frame.fault_pc, frame.resume_pc),
                (fault, fallthrough.index())
            );
            assert_eq!(
                execution.slots.peek(&frame.window, 0).unwrap(),
                &JsValue::Int(if computed { 5 } else { 7 })
            );
            assert_eq!(execution.slots.depth(&frame.window), expected_depth);
        }
    }

    #[test]
    fn retained_receiver_partial_output_keeps_property_publication_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let Value::Object(object) = context.eval("({x:7})").unwrap() else {
            panic!("object");
        };
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x()})",
            Opcode::GetField2Cached,
        );
        loop {
            let frame = execution.frames.current_mut(id).unwrap();
            if execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_err()
            {
                break;
            }
        }
        let frame = execution.frames.current_mut(id).unwrap();
        drop(execution.slots.pop(&mut frame.window).unwrap());
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(object.into_handle()))
            .unwrap();
        let frame = execution.frames.current_mut(id).unwrap();
        let fault = frame.resume_pc;
        let fallthrough = frame
            .executable
            .exec
            .decode_published(fault as u32)
            .unwrap()
            .next_pc as usize;
        let (result, recovery_calls) =
            count_next_pc_calls(|| execute_frame(&runtime, &mut execution, id));
        assert!(result.is_err());
        assert_eq!(recovery_calls, 0);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (fault, fallthrough));
        assert!(matches!(
            execution.slots.peek(&frame.window, 0),
            Ok(JsValue::Object(_))
        ));
    }

    #[test]
    fn resident_general_read_keeps_only_output_owner_and_never_clones_runtime() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        // A real namespace live-cell slot is excluded from ordinary IC
        // selection. Its final receiver owns the cell and the result child.
        let object = runtime.new_module_namespace_object().unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        let child = runtime
            .into_jsvalue(context.eval("({marker:7})").unwrap())
            .unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            let cell = state
                .new_var_ref(
                    &runtime.0.poisoned,
                    child,
                    true,
                    false,
                    crate::engine::code::function::metadata::ClosureVariableKind::Normal,
                )
                .unwrap();
            state
                .store_property_slot(
                    object.object_id(),
                    key.atom(),
                    crate::engine::object::shape::PropertyFlags::data(true, true, false),
                    crate::engine::heap::PropertySlot::VarRef(cell),
                )
                .unwrap();
            state.release_var_ref_handle(cell).unwrap();
        }
        let parent = object.object_id();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
        );
        execution
            .slots
            .push(
                &mut execution.frames.current_mut(id).unwrap().window,
                JsValue::Object(object.into_handle()),
            )
            .unwrap();
        let owners = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        #[cfg(feature = "profiling")]
        let _scope = crate::engine::api::profiling::CoreExecutionScope::enter();
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                crate::engine::vm::execute::execute_frame_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    id
                )
                .unwrap(),
                VmAction::Complete
            ));
            assert!(state.heap.object(parent).is_err());
            let Some(JsValue::Object(value)) = execution.pending.as_ref() else {
                panic!("result owner")
            };
            assert_eq!(state.heap.object_strong_count(*value), Ok(1));
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            assert!(!runtime.0.deferred_references.has_pending());
        }
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("named_read.state_completed").copied(), Some(1));
            assert_eq!(events.get("query.read.acquired").copied().unwrap_or(0), 0);
            assert_eq!(events.get("core.runtime_clone").copied().unwrap_or(0), 0);
        }
        drop(execution);
        assert!(!runtime.is_poisoned());
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn full_named_window_preserves_nullish_exception_and_fault_pc() {
        for opcode in [Opcode::GetFieldCached, Opcode::GetField2Cached] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let source = if opcode == Opcode::GetFieldCached {
                "(function(o){return o.x})"
            } else {
                "(function(o){return o.x()})"
            };
            let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
            let frame = execution.frames.current_mut(id).unwrap();
            while execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_ok()
            {}
            let _ = execution.slots.pop(&mut frame.window).unwrap();
            execution
                .slots
                .push(&mut frame.window, JsValue::Null)
                .unwrap();
            let fault = frame.resume_pc;
            let depth = execution.slots.depth(&frame.window);
            assert!(matches!(
                execute_frame(&runtime, &mut execution, id).unwrap(),
                VmAction::ThrowPrepared
            ));
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(frame.fault_pc, fault);
            assert_eq!(execution.slots.depth(&frame.window), depth);
            assert!(matches!(
                execution.slots.peek(&frame.window, 0).unwrap(),
                JsValue::Null
            ));
            let error = execution.pending.take().expect("prepared exception owner");
            drop(execution);
            let Value::Object(error) = runtime.root_and_release_jsvalue(error).unwrap() else {
                panic!("native Error")
            };
            assert_eq!(
                context
                    .get_property(&error, &runtime.intern_property_key("name").unwrap())
                    .unwrap(),
                Value::String(crate::engine::value::JsString::from_static("TypeError"))
            );
            assert!(!runtime.is_poisoned());
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn computed_read_profiles_carried_completion_and_warm_field_hit_stays_inline() {
        use crate::engine::api::profiling::CostProfile;

        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("function read(o,k){return o[k]}; read({x:7},'x')")
                .unwrap(),
            Value::Int(7)
        );
        let events = profile.snapshot().owned_execution_events;
        assert!(
            events
                .get("property_read_action_exit")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(
            events
                .get("property_read_completed_with_carried_fallthrough")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert_eq!(
            events
                .get("property_legacy_fallthrough_recovery_decode")
                .copied()
                .unwrap_or(0),
            0
        );

        let _ = context
            .eval("var cached={x:9}; function getCached(){return cached.x}; getCached()")
            .unwrap();
        let warm_profile = CostProfile::start();
        assert_eq!(context.eval("getCached()").unwrap(), Value::Int(9));
        let warm_events = warm_profile.snapshot().owned_execution_events;
        assert_eq!(
            warm_events
                .get("property_read_action_exit")
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn direct_and_deferred_reads_keep_originating_source_location() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let direct = "function fail(){\n return null.x;\n}\ntry{fail()}catch(e){e instanceof TypeError && e.stack.includes('at fail (c2-direct.js:2:')}";
        assert_eq!(
            context.eval_with_filename(direct, "c2-direct.js").unwrap(),
            Value::Bool(true)
        );

        let deferred = "let calls=0,marker={};\nfunction fail(){\n return ({get x(){calls++;marker.stack=new Error().stack;throw marker}}).x;\n}\ntry{fail()}catch(e){e===marker && calls===1 && marker.stack.includes('at fail (c2-deferred.js:3:')}";
        assert_eq!(
            context
                .eval_with_filename(deferred, "c2-deferred.js")
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn linked_owning_read_transaction_preserves_method_receiver_and_selected_errors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let log='', marker={}, old;
            let o={tag:42,method(){return this.tag},get x(){log+='g';return marker}};
            old=o.x;
            if(old!==marker||o.method()!==42)return false;
            Object.defineProperty(o,'x',{get(){log+='t';throw marker}});
            try{o.x;return false}catch(e){if(e!==marker)return false}finally{log+='f'}
            let p=new Proxy({x:marker},{get(t,k,r){log+='p';return Reflect.get(t,k,r)}});
            if(p.x!==marker)return false;
            let inherited=Object.create({get x(){log+='h';return this.tag}});inherited.tag=42;
            if(inherited.x!==42)return false;
            return log==='gtfph' && ({x:{tag:42}}).x.tag===42;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn completed_reads_keep_last_receiver_and_result_owners() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                r#"(()=>{
            for(let i=0;i<40;i++) {
                let self=(()=>{let x={n:i};x.self=x;return x})().self;
                let item=[{n:i}][0];
                if(self.self!==self || self.n!==i || item.n!==i)throw 'lost owner';
                if(({n:i,method(){return this.n}})['method']()!==i)throw 'lost receiver';
            }
            return ({child:{tag:42}}).child;
        })()"#,
            )
            .unwrap();
        runtime.run_gc().unwrap();
        let Value::Object(object) = result else {
            panic!("expected surviving result");
        };
        assert_eq!(
            context
                .get_property(&object, &runtime.intern_property_key("tag").unwrap())
                .unwrap(),
            Value::Int(42)
        );
        drop(object);
        drop(context);
        runtime.run_gc().unwrap();
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn completed_and_pending_reads_keep_keys_receivers_and_terminal_typed_indices() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let trace='', key={toString(){trace+='k';return 'x'}};
            try { null[key] } catch(e) { if(e instanceof TypeError)trace+='n'; }
            let o={get x(){trace+='g';return {tag:7}}};
            if(o[key].tag!==7)throw 'getter result';
            let a=[];
            Object.setPrototypeOf(a,{get 0(){trace+=this===a?'h':'!';return 8}});
            if(a[0]!==8)throw 'hole';
            let symbol=Symbol(), b={[symbol]:3,1:4,true:5};
            b[symbol]++;b[1n]++;b[true]++;
            if(b[symbol]!==4 || b[1]!==5 || b[true]!==6)throw 'retained keys';
            let buffer=new ArrayBuffer(4,{maxByteLength:8}), t=new Uint8Array(buffer);
            t[0]=23;
            Object.setPrototypeOf(t,{get 0(){trace+='bad';return 99}});
            if(t[0]!==23 || t['-0']!==undefined)throw 'typed initial';
            buffer.resize(0);
            if(t[0]!==undefined || t[NaN]!==undefined)throw 'typed terminal';
            return trace==='nkgh';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}
