//! Instanceof owns method selection and prototype walking across user callbacks.
use crate::engine::{
    api::{
        Error, ErrorKind, error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError,
    },
    builtins::native::NativeFunctionId,
    heap::{
        ContextId, Heap, HeapError, ObjectId, ObjectKind, ObjectPayload, PropertySlot, RawValue,
    },
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum InstanceStep {
    Complete(Completion),
    Read { resume: InstanceResume },
    Call { resume: InstanceResume },
    Prototype { resume: InstanceResume },
}
pub(crate) struct InstanceResume(Box<InstanceResumeState>);
impl std::ops::Deref for InstanceResume {
    type Target = InstanceResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for InstanceResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<InstanceResume>() <= 8);
pub(crate) struct InstanceResumeState {
    pending_effect: InstanceStepPending,
    realm: ContextId,
    candidate: JsValue,
    target: ObjectRef,
    phase: Phase,
}
enum Phase {
    Method { delegate: bool },
    Result,
    Prototype,
    Walk(ObjectRef),
}
impl Drop for InstanceResumeState {
    fn drop(&mut self) {
        let value = std::mem::replace(&mut self.candidate, JsValue::Undefined);
        let runtime = self.target.runtime();
        let _ = runtime.release_jsvalue(value);
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = runtime.release_jsvalue(value);
            }
        }
    }
}
impl InstanceStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        candidate: JsValue,
        target: ObjectRef,
        intrinsic_budget: bool,
    ) -> Result<Self, RuntimeError> {
        if let Some(found) =
            try_ordinary_instanceof(runtime, &candidate, target.object_id(), intrinsic_budget)
        {
            runtime.release_jsvalue(candidate)?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "instanceof.completed_without_continuation",
            );
            return Ok(Self::Complete(Completion::Return(JsValue::Bool(found))));
        }
        Self::method(runtime, realm, candidate, target, false)
    }
    fn method(
        runtime: &Runtime,
        realm: ContextId,
        candidate: JsValue,
        target: ObjectRef,
        delegate: bool,
    ) -> Result<Self, RuntimeError> {
        Ok({
            let __pending_field_object = target.try_clone()?;
            let __pending_field_key =
                PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::HasInstance)?);
            let __pending_field_resume = InstanceResume(Box::new(InstanceResumeState {
                pending_effect: InstanceStepPending::default(),
                realm,
                candidate,
                target,
                phase: Phase::Method { delegate },
            }));
            Self::request_read(
                __pending_field_object,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn native(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "hasInstance requires generic invocation",
            ));
        };
        let target = match this_value {
            JsValue::Object(id) => {
                let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
                runtime.as_callable(&object)?
            }
            _ => None,
        };
        let Some(target) = target else {
            return Ok(Self::Complete(Completion::Return(JsValue::Bool(false))));
        };
        Self::ordinary(
            runtime,
            realm,
            &target,
            match arguments.readable.first() {
                Some(value) => runtime.dup_jsvalue(value)?,
                None => JsValue::Undefined,
            },
        )
    }
    pub(crate) fn ordinary(
        runtime: &Runtime,
        realm: ContextId,
        target: &CallableRef,
        candidate: JsValue,
    ) -> Result<Self, RuntimeError> {
        let bound = {
            let state = runtime.0.state.borrow();
            match &state.heap.object(target.as_object().object_id())?.payload {
                ObjectPayload::BoundFunction { target, .. } => Some(*target),
                ObjectPayload::NativeFunction { .. }
                | ObjectPayload::BytecodeFunction { .. }
                | ObjectPayload::Proxy(_) => None,
                _ => {
                    return Err(RuntimeError::Invariant(
                        "ordinary instanceof received a non-callable target",
                    ));
                }
            }
        };
        if let Some(bound) = bound {
            let target = ObjectRef::from_borrowed_handle(runtime.clone(), bound)?;
            return Self::method(runtime, realm, candidate, target, true);
        }
        if !matches!(candidate, JsValue::Object(_)) {
            runtime.release_jsvalue(candidate)?;
            return Ok(Self::Complete(Completion::Return(JsValue::Bool(false))));
        }
        Ok({
            let __pending_field_object = target.as_object().try_clone()?;
            let __pending_field_key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
            let __pending_field_resume = InstanceResume(Box::new(InstanceResumeState {
                pending_effect: InstanceStepPending::default(),
                realm,
                candidate,
                target: target.as_object().try_clone()?,
                phase: Phase::Prototype,
            }));
            Self::request_read(
                __pending_field_object,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
}

/// Complete the non-observable part of InstanceofOperator while its inputs
/// remain owned. A miss has selected no getter, called no code, and changed no
/// owner or property; the existing continuation still owns the whole operation.
/// No fact survives this runtime-state borrow.
fn try_ordinary_instanceof(
    runtime: &Runtime,
    candidate: &JsValue,
    target: ObjectId,
    intrinsic_budget: bool,
) -> Option<bool> {
    let state = runtime.0.state.try_borrow().ok()?;
    try_ordinary_instanceof_in_state(runtime, &state, candidate, target, intrinsic_budget)
}

/// The same kernel under the interpreter's own state access, so a hit
/// completes `instanceof` without leaving the instruction loop.
pub(crate) fn try_ordinary_instanceof_in_state(
    runtime: &Runtime,
    state: &crate::engine::heap::runtime::RuntimeState,
    candidate: &JsValue,
    target: ObjectId,
    intrinsic_budget: bool,
) -> Option<bool> {
    if runtime.0.deferred_references.has_pending() {
        return None;
    }
    if state.heap.has_pending_zero_cleanup() {
        return None;
    }
    let heap = &state.heap;
    // Every object below is kept alive by an operand, a slot or a shape edge
    // for the whole borrow, so lookups are trusted; the count thresholds that
    // keep the generic protocol's saturation behavior are unchanged.
    let (target_data, target_strong) = heap.object_and_strong_fast(target);
    if target_strong < 2 {
        return None;
    }
    if !matches!(
        target_data.payload,
        ObjectPayload::BytecodeFunction { .. } | ObjectPayload::NativeFunction { .. }
    ) {
        // Bound functions delegate InstanceofOperator, and a Proxy may observe
        // both @@hasInstance and prototype. Neither uses this ordinary kernel.
        return None;
    }
    let method = state
        .well_known_symbols
        .get(&WellKnownSymbol::HasInstance)?;
    match borrowed_ordinary_data(heap, target, *method)? {
        None | Some(RawValue::Null | RawValue::Undefined) => {}
        Some(RawValue::Object(method)) => {
            if !temporary_roots_fit(heap, *method) {
                return None;
            }
            let ObjectPayload::NativeFunction { data, .. } = &heap.object_fast(*method).payload
            else {
                return None;
            };
            if data.target != NativeFunctionId::FunctionPrototypeHasInstance || data.realm.is_none()
            {
                return None;
            }
            // The ordinary intrinsic used to enter a native continuation.
            // Keep its logical and physical stack admission even though this
            // execution no longer installs that continuation. A nullish or
            // absent method takes OrdinaryHasInstance without this call.
            if !intrinsic_budget || runtime.proxy_method_stack_would_overflow() {
                return None;
            }
            heap.context(data.realm?).ok()?;
        }
        _ => return None,
    }
    let JsValue::Object(candidate) = candidate else {
        // OrdinaryHasInstance answers false for every non-object. Heap
        // primitives acquire temporary argument owners in the generic
        // intrinsic call, so near saturation they keep that path's rules.
        let leaf = match candidate {
            JsValue::String(id) => crate::engine::heap::RawId::String(*id),
            JsValue::BigInt(id) => crate::engine::heap::RawId::BigInt(*id),
            JsValue::Undefined
            | JsValue::Null
            | JsValue::Bool(_)
            | JsValue::Int(_)
            | JsValue::Float(_)
            | JsValue::ShortBigInt(_) => return Some(false),
            JsValue::Symbol(_) | JsValue::Object(_) => return None,
        };
        return (heap.leaf_strong_fast(leaf) < u32::MAX - INSTANCE_PROTOCOL_ROOT_HEADROOM)
            .then_some(false);
    };
    if !temporary_roots_fit(heap, *candidate) {
        return None;
    }
    // Completion consumes both input owners after this borrow. They must
    // remain nonfinal even when candidate and target are the same object.
    if heap.object_strong_fast(*candidate) < if *candidate == target { 3 } else { 2 } {
        return None;
    }
    let key = state
        .pinned_atoms
        .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
    let Some(RawValue::Object(expected)) = borrowed_ordinary_data(heap, target, key)? else {
        // Lazy prototype materialization and the non-object prototype error
        // must retain their original observable execution boundary.
        return None;
    };
    if !temporary_roots_fit(heap, *expected) {
        return None;
    }
    match walk_ordinary_chain(heap, *candidate, *expected).ok()? {
        ChainWalk::Complete(found) => Some(found),
        ChainWalk::Protocol(_) => None,
    }
}

// Bound all owning roles in the callback-free generic protocol, including
// aliases between candidate, target, selected method and expected prototype:
//
//   outer InstanceResume target/candidate                  2
//   pending property-read object/receiver                 2
//   property result and its promoted CallableRef          2
//   native invocation receiver/argument                   2
//   NativeActivation callable                             1
//   InstanceStep::native receiver ObjectRef/CallableRef    2
//   inner InstanceResume target/candidate                 2
//   prototype result                                      1
//   prototype-walk current/result                         2
//                                                        --
//                                                        16
//
// This is an upper bound, not a claim that all roles coexist: ownership moves
// reuse some roots, and property projection covers either named read. Counting
// every role against each identity also covers arbitrary aliases. Frame
// publication borrows its callable root and adds no heap edge. Rejecting near
// saturation preserves the generic MAX-1 -> immortal transition even when
// this kernel omits temporary roots.
const INSTANCE_PROTOCOL_ROOT_HEADROOM: u32 = 2 + 2 + 2 + 2 + 1 + 2 + 2 + 1 + 2;

fn temporary_roots_fit(heap: &Heap, object: ObjectId) -> bool {
    heap.object_strong_fast(object) < u32::MAX - INSTANCE_PROTOCOL_ROOT_HEADROOM
}

/// Restrict Get to storage whose named reads are ordinary parallel slots.
/// The nested Option distinguishes an absent property from a protocol miss.
fn borrowed_ordinary_data(
    heap: &Heap,
    mut object: ObjectId,
    key: crate::engine::atom::Atom,
) -> Option<Option<&RawValue>> {
    for _ in 0..32 {
        let (data, strong) = heap.object_and_strong_fast(object);
        if strong >= u32::MAX - INSTANCE_PROTOCOL_ROOT_HEADROOM {
            return None;
        }
        if !matches!(
            (data.kind, &data.payload),
            (ObjectKind::Ordinary, ObjectPayload::Ordinary)
                | (
                    ObjectKind::BytecodeFunction,
                    ObjectPayload::BytecodeFunction { .. }
                )
                | (
                    ObjectKind::NativeFunction,
                    ObjectPayload::NativeFunction { .. }
                )
        ) {
            return None;
        }
        let shape = heap.shape_fast(data.shape);
        if let Some(index) = shape.find(crate::engine::atom::AtomIdx::from_raw(key.raw())) {
            return match data.slots.get(index as usize)? {
                PropertySlot::Data(value) => Some(Some(value)),
                PropertySlot::Accessor { .. }
                | PropertySlot::AutoInit(_)
                | PropertySlot::VarRef(_) => None,
            };
        }
        let Some(prototype) = shape.prototype() else {
            return Some(None);
        };
        object = prototype;
    }
    None
}
impl InstanceResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<InstanceStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            result @ Completion::Throw(_) => return Ok(InstanceStep::Complete(result)),
        };
        match self.0.phase {
            Phase::Method { delegate } => {
                if matches!(value, JsValue::Null | JsValue::Undefined) {
                    let Some(target) = runtime.as_callable(&self.0.target)? else {
                        return if delegate {
                            Ok(InstanceStep::Complete(Completion::Throw(
                                runtime.new_native_error_jsvalue(
                                    self.0.realm,
                                    NativeErrorKind::Type,
                                    "invalid 'instanceof' right operand",
                                )?,
                            )))
                        } else {
                            Err(RuntimeError::Engine(Error::new(
                                ErrorKind::Type,
                                "invalid 'instanceof' right operand",
                            )))
                        };
                    };
                    return InstanceStep::ordinary(
                        runtime,
                        self.0.realm,
                        &target,
                        std::mem::replace(&mut self.0.candidate, JsValue::Undefined),
                    );
                }
                let callable = runtime.callable_from_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                let callable = match callable {
                    Ok(callable) => callable,
                    Err(RuntimeError::Engine(error))
                        if delegate && error.kind() == ErrorKind::Type =>
                    {
                        return Ok(InstanceStep::Complete(Completion::Throw(
                            runtime.new_native_error_from_error_jsvalue(
                                self.0.realm,
                                NativeErrorKind::Type,
                                &error,
                            )?,
                        )));
                    }
                    Err(error) => return Err(error),
                };
                Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver =
                        JsValue::Object(self.0.target.try_clone()?.into_handle());
                    let __pending_field_arguments =
                        vec![std::mem::replace(&mut self.0.candidate, JsValue::Undefined)];
                    let __pending_field_delegate = delegate;
                    let __pending_field_resume = {
                        let updated_0 = Phase::Result;
                        self.0.phase = updated_0;
                        self
                    };
                    InstanceStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_delegate,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Result => {
                let boolean = runtime.value_to_boolean_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                Ok(InstanceStep::Complete(Completion::Return(JsValue::Bool(
                    boolean?,
                ))))
            }
            Phase::Prototype => {
                let JsValue::Object(prototype) = value else {
                    runtime.release_jsvalue(value)?;
                    return Ok(InstanceStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "operand 'prototype' property is not an object",
                        )?,
                    )));
                };
                let prototype = ObjectRef::from_owned_handle(runtime.clone(), prototype);
                let JsValue::Object(candidate) = &self.0.candidate else {
                    return Err(RuntimeError::Invariant("instanceof lost object candidate"));
                };
                let candidate = *candidate;
                self.0.phase = Phase::Walk(prototype);
                self.walk_ordinary(runtime, candidate)
            }
            Phase::Walk(_) => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "prototype walk received untyped reply",
                ))
            }
        }
    }
    fn walk_ordinary(
        self,
        runtime: &Runtime,
        candidate: ObjectId,
    ) -> Result<InstanceStep, RuntimeError> {
        let Phase::Walk(expected) = &self.0.phase else {
            return Err(RuntimeError::Invariant(
                "instanceof lost expected prototype",
            ));
        };
        let walk = {
            let state = runtime.0.state.borrow();
            // Preserve existing release/drain boundaries when cleanup is pending.
            if runtime.0.deferred_references.has_pending() || state.heap.has_pending_zero_cleanup()
            {
                ChainWalk::Protocol(candidate)
            } else {
                walk_ordinary_chain(&state.heap, candidate, expected.object_id())?
            }
        };
        match walk {
            ChainWalk::Complete(found) => Ok(InstanceStep::Complete(Completion::Return(
                JsValue::Bool(found),
            ))),
            ChainWalk::Protocol(object) => Ok(InstanceStep::request_prototype(
                ObjectRef::from_borrowed_handle(runtime.clone(), object)?,
                self,
            )),
        }
    }
    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<InstanceStep, RuntimeError> {
        let Phase::Walk(expected) = &self.0.phase else {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "instanceof received unexpected prototype",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => InstanceStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(None) => {
                InstanceStep::Complete(Completion::Return(JsValue::Bool(false)))
            }
            NativeConversion::Value(Some(object)) if &object == expected => {
                InstanceStep::Complete(Completion::Return(JsValue::Bool(true)))
            }
            NativeConversion::Value(Some(object)) => {
                // A protocol reply already owns its receiver. Transfer it
                // unchanged, including saturated roots and detached Proxy chains.
                InstanceStep::request_prototype(object, self)
            }
        })
    }
}
#[derive(Debug, PartialEq, Eq)]
enum ChainWalk {
    Complete(bool),
    Protocol(ObjectId),
}

fn walk_ordinary_chain(
    heap: &Heap,
    mut current: ObjectId,
    expected: ObjectId,
) -> Result<ChainWalk, HeapError> {
    // The old entry first retained the candidate. Preserve both checked
    // overflow and the MAX-1 -> immortal transition before omitting that root.
    if heap.object_strong_fast(current) >= u32::MAX - 1 {
        return Ok(ChainWalk::Protocol(current));
    }
    // Only the initial candidate's rooted ancestry is batched. Protocol replies
    // keep their existing owner-transfer path. Bound even internally cyclic
    // chains; no callbacks, mutation or owner release occurs in this borrow.
    for _ in 0..32 {
        let object = heap.object_fast(current);
        if matches!(object.payload, ObjectPayload::Proxy(_)) {
            return Ok(ChainWalk::Protocol(current));
        }
        let Some(prototype) = heap.shape_fast(object.shape).prototype() else {
            return Ok(ChainWalk::Complete(false));
        };
        // get_prototype_of retained its result before comparing identity.
        // Its current receiver also had a temporary root, so a self-edge has
        // two concurrent temporary retains and needs one extra count of room.
        let strong = heap.object_strong_fast(prototype);
        if strong >= u32::MAX - 1 || (prototype == current && strong == u32::MAX - 2) {
            return Ok(ChainWalk::Protocol(current));
        }
        if prototype == expected {
            return Ok(ChainWalk::Complete(true));
        }
        current = prototype;
    }
    Ok(ChainWalk::Protocol(current))
}

pub(crate) fn finish(
    runtime: &Runtime,
    mut realm: ContextId,
    mut step: InstanceStep,
) -> Result<Completion, RuntimeError> {
    // The old consumer retains its bound-chain trampoline and native backtraces.
    let mut frames = Vec::new();
    let result = (|| loop {
        step = match step {
            InstanceStep::Complete(result) => return Ok(result),
            InstanceStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            InstanceStep::Prototype { mut resume } => {
                let object = resume.take_prototype_object();
                resume.prototype(runtime, runtime.internal_get_prototype_of(realm, &object)?)?
            }
            InstanceStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                let delegate = resume.take_call_delegate();
                {
                    let standard = if delegate {
                        runtime.direct_native_callable_metadata(&callable)?
                    } else {
                        None
                    };
                    if let Some((
                        NativeFunctionId::FunctionPrototypeHasInstance,
                        defining_realm,
                        min,
                    )) = standard
                    {
                        frames.push(runtime.push_native_active_frame(
                            callable.as_object().try_clone()?,
                            defining_realm,
                            NativeFunctionId::FunctionPrototypeHasInstance,
                            1,
                            1usize.max(usize::from(min)),
                        )?);
                        realm = defining_realm;
                        let invocation = NativeInvocation::Call {
                            this_value: receiver,
                        };
                        let arguments = NativeArguments {
                            actual_arg_count: 1,
                            readable: arguments,
                        };
                        let result =
                            runtime.dispatch_borrowed_invocation(invocation, |invocation| {
                                InstanceStep::native(runtime, realm, invocation, &arguments)
                            });
                        for value in arguments.readable {
                            runtime.release_jsvalue(value)?;
                        }
                        result?
                    } else {
                        resume.resume(
                            runtime,
                            runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                        )?
                    }
                }
            }
        };
    })();
    let mut frame_error = None;
    while let Some(frame) = frames.pop() {
        if let Err(error) = frame.finish() {
            frame_error.get_or_insert(error);
        }
    }
    frame_error.map_or(result, Err)
}

#[derive(Default)]
struct InstanceStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    call_delegate: Option<bool>,
    prototype_object: Option<ObjectRef>,
}
impl InstanceStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: InstanceResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        delegate: bool,
        mut resume: InstanceResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        resume.0.pending_effect.call_delegate = Some(delegate);
        Self::Call { resume }
    }
    pub(crate) fn request_prototype(object: ObjectRef, mut resume: InstanceResume) -> Self {
        resume.0.pending_effect.prototype_object = Some(object);
        Self::Prototype { resume }
    }
}
impl InstanceResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("InstanceStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("InstanceStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("InstanceStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("InstanceStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("InstanceStep Call arguments")
    }
    pub(crate) fn take_call_delegate(&mut self) -> bool {
        self.0
            .pending_effect
            .call_delegate
            .take()
            .expect("InstanceStep Call delegate")
    }
    pub(crate) fn take_prototype_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .prototype_object
            .take()
            .expect("InstanceStep Prototype object")
    }
}
const _: () = assert!(std::mem::size_of::<InstanceStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<InstanceStep>() <= 64);

#[cfg(test)]
mod tests;
