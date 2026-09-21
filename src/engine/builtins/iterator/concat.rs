use super::ObjectIteratorStep;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError, IteratorConcatData, IteratorConcatItem, ObjectData, RawValue},
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation, NativeInvokeOutcome},
    },
};

impl Runtime {
    pub(crate) fn call_iterator_concat(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            match finish(
                self,
                realm,
                ConcatStep::start(self, realm, ConcatKind::Create, invocation, arguments)?,
            )? {
                NativeInvokeOutcome::Completion(result) => Ok(result),
                NativeInvokeOutcome::IteratorNextRaw { .. } => {
                    Err(RuntimeError::Invariant("concat creation returned raw next"))
                }
            }
        })
    }

    fn new_iterator_concat(
        &self,
        realm: ContextId,
        inputs: &[(ObjectRef, JsValue)],
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.iterator_realm_data(realm)?.concat_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        let items = inputs
            .iter()
            .map(|(iterable, method)| {
                Some(IteratorConcatItem {
                    iterable: iterable.object_id(),
                    method: method.as_raw(),
                })
            })
            .collect::<Vec<_>>();
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let retained_atoms =
            match state.retain_raw_value_atoms(items.iter().flatten().map(|item| &item.method)) {
                Ok(atoms) => atoms,
                Err(error) => {
                    let cleanup = state.heap.release_shape(shape)?;
                    state.apply_cleanup(cleanup)?;
                    return Err(error);
                }
            };
        let object =
            match state
                .heap
                .allocate_object(ObjectData::iterator_concat(shape, Vec::new(), items))
            {
                Ok(object) => object,
                Err(error) => {
                    state.release_atoms(retained_atoms)?;
                    let cleanup = state.heap.release_shape(shape)?;
                    state.apply_cleanup(cleanup)?;
                    return Err(error.into());
                }
            };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    fn iterator_concat_snapshot(
        &self,
        realm: ContextId,
        concat: &ObjectRef,
    ) -> Result<NativeConversion<IteratorConcatData>, RuntimeError> {
        let snapshot = {
            let state = self.0.state.borrow();
            state.heap.iterator_concat_state(concat.object_id())
        };
        match snapshot {
            Ok(snapshot) => Ok(NativeConversion::Value(snapshot)),
            Err(HeapError::Invariant(_)) => {
                Ok(NativeConversion::Throw(self.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "not an Iterator Concat",
                )?))
            }
            Err(error) => Err(error.into()),
        }
    }

    fn set_iterator_concat_running(
        &self,
        concat: &ObjectRef,
        running: bool,
    ) -> Result<(), RuntimeError> {
        self.0
            .state
            .borrow_mut()
            .heap
            .set_iterator_concat_running(concat.object_id(), running)?;
        Ok(())
    }

    fn set_iterator_concat_iterator(
        &self,
        concat: &ObjectRef,
        iterator: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let cleanup = state
            .heap
            .set_iterator_concat_iterator(concat.object_id(), Some(iterator.object_id()))?;
        state.apply_cleanup(cleanup)
    }

    fn set_iterator_concat_next(
        &self,
        concat: &ObjectRef,
        next: &JsValue,
    ) -> Result<(), RuntimeError> {
        let raw = next.as_raw();
        let mut state = self.0.state.borrow_mut();
        let retained_atoms = state.retain_raw_value_atoms([&raw])?;
        let cleanup = match state.heap.set_iterator_concat_next(concat.object_id(), raw) {
            Ok(cleanup) => cleanup,
            Err(error) => {
                state.release_atoms(retained_atoms)?;
                return Err(error.into());
            }
        };
        state.apply_cleanup(cleanup)?;
        Ok(())
    }

    fn advance_iterator_concat(&self, concat: &ObjectRef) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let cleanup = state.heap.advance_iterator_concat(concat.object_id())?;
        state.apply_cleanup(cleanup)
    }

    fn clear_iterator_concat(&self, concat: &ObjectRef) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let cleanup = state.heap.clear_iterator_concat(concat.object_id())?;
        state.apply_cleanup(cleanup)
    }

    pub(crate) fn call_iterator_concat_next(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        match self.call_iterator_concat_next_raw(realm, invocation)? {
            NativeInvokeOutcome::Completion(completion) => Ok(completion),
            NativeInvokeOutcome::IteratorNextRaw { value, done } => {
                Ok(Completion::Return(self.into_jsvalue(Value::Object(
                    self.new_iterator_result_jsvalue(realm, value, done)?,
                ))?))
            }
        }
    }

    pub(crate) fn call_iterator_concat_next_raw(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<NativeInvokeOutcome, RuntimeError> {
        finish(
            self,
            realm,
            ConcatStep::start(
                self,
                realm,
                ConcatKind::Next,
                &invocation,
                &NativeArguments {
                    actual_arg_count: 0,
                    readable: Vec::new(),
                },
            )?,
        )
    }

    pub(crate) fn call_iterator_concat_return(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            match finish(
                self,
                realm,
                ConcatStep::start(
                    self,
                    realm,
                    ConcatKind::Return,
                    invocation,
                    &NativeArguments {
                        actual_arg_count: 0,
                        readable: Vec::new(),
                    },
                )?,
            )? {
                NativeInvokeOutcome::Completion(result) => Ok(result),
                NativeInvokeOutcome::IteratorNextRaw { .. } => {
                    Err(RuntimeError::Invariant("concat return returned raw next"))
                }
            }
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ConcatKind {
    Create,
    Next,
    Return,
}
pub(crate) enum ConcatStep {
    Complete(NativeInvokeOutcome),
    Read { resume: ConcatResume },
    Call { resume: ConcatResume },
    Next { resume: ConcatResume },
}
struct ConcatGuard {
    runtime: Runtime,
    concat: ObjectRef,
    active: bool,
    clear: bool,
}
impl ConcatGuard {
    fn reset(&mut self) -> Result<(), RuntimeError> {
        self.runtime
            .set_iterator_concat_running(&self.concat, false)?;
        if self.clear {
            self.runtime.clear_iterator_concat(&self.concat)?;
        }
        self.active = false;
        Ok(())
    }
}
impl Drop for ConcatGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = self
                .runtime
                .set_iterator_concat_running(&self.concat, false);
            if self.clear {
                let _ = self.runtime.clear_iterator_concat(&self.concat);
            }
        }
    }
}
pub(crate) struct ConcatResume(Box<ConcatResumeState>);
impl std::ops::Deref for ConcatResume {
    type Target = ConcatResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ConcatResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ConcatResume>() <= 8);
pub(crate) struct ConcatResumeState {
    runtime: Runtime,
    pending_effect: ConcatStepPending,
    realm: ContextId,
    phase: ConcatPhase,
    guard: Option<ConcatGuard>,
}
enum ConcatPhase {
    Input {
        owner: ConcatInputs,
        current: ObjectRef,
    },
    Iterator,
    Method(ObjectRef),
    Next,
    ReturnMethod(ObjectRef),
    ReturnResult,
}
struct ConcatInputs {
    runtime: Runtime,
    remaining: std::vec::IntoIter<JsValue>,
    inputs: Vec<(ObjectRef, JsValue)>,
}
impl Drop for ConcatInputs {
    fn drop(&mut self) {
        for value in self.remaining.by_ref() {
            let _ = self.runtime.release_jsvalue(value);
        }
        for (_, value) in self.inputs.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl Drop for ConcatResumeState {
    fn drop(&mut self) {
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.next_method.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ConcatStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ConcatKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if matches!(kind, ConcatKind::Create) {
            if !matches!(invocation, NativeInvocation::Call { .. }) {
                return Err(RuntimeError::Invariant(
                    "Iterator.concat did not receive a generic invocation",
                ));
            }
            let mut owner = ConcatInputs {
                runtime: runtime.clone(),
                remaining: Vec::new().into_iter(),
                inputs: Vec::with_capacity(arguments.actual_arg_count),
            };
            let mut values = Vec::with_capacity(arguments.actual_arg_count);
            for value in &arguments.readable[..arguments.actual_arg_count] {
                match runtime.dup_jsvalue(value) {
                    Ok(value) => values.push(value),
                    Err(error) => {
                        for value in values {
                            let _ = runtime.release_jsvalue(value);
                        }
                        return Err(error);
                    }
                }
            }
            owner.remaining = values.into_iter();
            return ConcatResume::input(runtime, realm, owner);
        }
        let concat = match runtime.iterator_receiver(realm, invocation)? {
            NativeConversion::Value(concat) => concat,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                    Completion::Throw(value),
                )));
            }
        };
        let snapshot = match runtime.iterator_concat_snapshot(realm, &concat)? {
            NativeConversion::Value(snapshot) => snapshot,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                    Completion::Throw(value),
                )));
            }
        };
        if snapshot.running {
            return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                Completion::Throw(runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "already running",
                )?),
            )));
        }
        if matches!(kind, ConcatKind::Return) && snapshot.iterator.is_none() {
            runtime.clear_iterator_concat(&concat)?;
            return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                Completion::Return(JsValue::Undefined),
            )));
        }
        runtime.set_iterator_concat_running(&concat, true)?;
        let guard = ConcatGuard {
            runtime: runtime.clone(),
            concat,
            active: true,
            clear: false,
        };
        let mut resume = ConcatResume(Box::new(ConcatResumeState {
            runtime: runtime.clone(),
            pending_effect: ConcatStepPending::default(),
            realm,
            phase: ConcatPhase::Next,
            guard: Some(guard),
        }));
        if matches!(kind, ConcatKind::Return) {
            let iterator = ObjectRef::from_borrowed_handle(
                runtime.clone(),
                snapshot
                    .iterator
                    .ok_or(RuntimeError::Invariant("concat return iterator missing"))?,
            )?;
            resume.phase = ConcatPhase::ReturnMethod(iterator.clone());
            return Ok({
                let __pending_field_object = iterator;
                let __pending_field_key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Return)?;
                let __pending_field_resume = resume;
                Self::request_read(
                    __pending_field_object,
                    __pending_field_key,
                    __pending_field_resume,
                )
            });
        }
        resume.advance(runtime)
    }
}
impl ConcatResume {
    fn input(
        runtime: &Runtime,
        realm: ContextId,
        mut owner: ConcatInputs,
    ) -> Result<ConcatStep, RuntimeError> {
        let Some(input) = owner.remaining.next() else {
            return Ok(ConcatStep::Complete(NativeInvokeOutcome::Completion(
                Completion::Return(JsValue::Object(
                    runtime
                        .new_iterator_concat(realm, &owner.inputs)?
                        .into_handle(),
                )),
            )));
        };
        let current = match input {
            JsValue::Object(id) => ObjectRef::from_owned_handle(runtime.clone(), id),
            other => {
                runtime.release_jsvalue(other)?;
                return Ok(ConcatStep::Complete(NativeInvokeOutcome::Completion(
                    Completion::Throw(runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?),
                )));
            }
        };
        let object = current.clone();
        let key = PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator));
        let resume = Self(Box::new(ConcatResumeState {
            runtime: runtime.clone(),
            pending_effect: ConcatStepPending::default(),
            realm,
            guard: None,
            phase: ConcatPhase::Input { owner, current },
        }));
        Ok(ConcatStep::request_read(object, key, resume))
    }
    fn concat(&self) -> Result<&ObjectRef, RuntimeError> {
        self.0
            .guard
            .as_ref()
            .map(|guard| &guard.concat)
            .ok_or(RuntimeError::Invariant(
                "concat resume has no running owner",
            ))
    }
    fn complete(mut self, result: NativeInvokeOutcome) -> Result<ConcatStep, RuntimeError> {
        if let Some(guard) = &mut self.0.guard {
            guard.reset()?;
        }
        Ok(ConcatStep::Complete(result))
    }
    fn advance(mut self, runtime: &Runtime) -> Result<ConcatStep, RuntimeError> {
        let snapshot = {
            runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_concat_state(self.concat()?.object_id())?
        };
        if snapshot.index >= snapshot.items.len() {
            return self.complete(NativeInvokeOutcome::IteratorNextRaw {
                value: JsValue::Undefined,
                done: true,
            });
        }
        if let Some(iterator) = snapshot.iterator {
            return self.method(
                runtime,
                ObjectRef::from_borrowed_handle(runtime.clone(), iterator)?,
            );
        }
        let item = snapshot
            .items
            .get(snapshot.index)
            .and_then(Option::as_ref)
            .ok_or(RuntimeError::Invariant(
                "Iterator Concat current input was already released",
            ))?;
        let iterable = ObjectRef::from_borrowed_handle(runtime.clone(), item.iterable)?;
        let method = runtime.dup_iterator_raw(&item.method)?;
        let callable = runtime.iterator_callable_jsvalue(self.0.realm, &method);
        runtime.release_jsvalue(method)?;
        let callable = match callable? {
            NativeConversion::Value(callable) => callable,
            NativeConversion::Throw(discarded) => {
                runtime.release_jsvalue(discarded)?;
                return Err(RuntimeError::Invariant(
                    "Iterator Concat captured method lost its callable brand",
                ));
            }
        };
        self.0.phase = ConcatPhase::Iterator;
        Ok({
            let __pending_field_callable = callable;
            let __pending_field_receiver = runtime.into_jsvalue(Value::Object(iterable))?;
            let __pending_field_resume = self;
            ConcatStep::request_call(
                __pending_field_callable,
                __pending_field_receiver,
                __pending_field_resume,
            )
        })
    }
    fn method(
        mut self,
        runtime: &Runtime,
        iterator: ObjectRef,
    ) -> Result<ConcatStep, RuntimeError> {
        let snapshot = {
            runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_concat_state(self.concat()?.object_id())?
        };
        if matches!(snapshot.next, RawValue::Undefined) {
            self.0.phase = ConcatPhase::Method(iterator.clone());
            return Ok({
                let __pending_field_object = iterator;
                let __pending_field_key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
                let __pending_field_resume = self;
                ConcatStep::request_read(
                    __pending_field_object,
                    __pending_field_key,
                    __pending_field_resume,
                )
            });
        }
        let method = runtime.dup_iterator_raw(&snapshot.next)?;
        self.0.phase = ConcatPhase::Next;
        Ok({
            let __pending_field_iterator = iterator;
            let __pending_field_method = method;
            let __pending_field_resume = self;
            ConcatStep::request_next(
                __pending_field_iterator,
                __pending_field_method,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ConcatStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return self.complete(NativeInvokeOutcome::Completion(Completion::Throw(value)));
            }
        };
        match std::mem::replace(&mut self.0.phase, ConcatPhase::Next) {
            ConcatPhase::Input { mut owner, current } => {
                let callable = runtime.iterator_callable_jsvalue(self.0.realm, &value);
                match callable {
                    Ok(NativeConversion::Value(_)) => owner.inputs.push((current, value)),
                    Ok(NativeConversion::Throw(error)) => {
                        runtime.release_jsvalue(value)?;
                        return self
                            .complete(NativeInvokeOutcome::Completion(Completion::Throw(error)));
                    }
                    Err(error) => {
                        let _ = runtime.release_jsvalue(value);
                        return Err(error);
                    }
                }
                Self::input(runtime, self.0.realm, owner)
            }
            ConcatPhase::Iterator => {
                let iterator = match value {
                    JsValue::Object(id) => ObjectRef::from_owned_handle(runtime.clone(), id),
                    other => {
                        runtime.release_jsvalue(other)?;
                        let error = runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?;
                        return self
                            .complete(NativeInvokeOutcome::Completion(Completion::Throw(error)));
                    }
                };
                runtime.set_iterator_concat_iterator(self.concat()?, &iterator)?;
                self.method(runtime, iterator)
            }
            ConcatPhase::Method(iterator) => {
                if let Err(error) = runtime.set_iterator_concat_next(self.concat()?, &value) {
                    let _ = runtime.release_jsvalue(value);
                    return Err(error);
                }
                Ok(ConcatStep::request_next(iterator, value, self))
            }
            ConcatPhase::ReturnMethod(iterator) => {
                self.0
                    .guard
                    .as_mut()
                    .ok_or(RuntimeError::Invariant("concat return owner missing"))?
                    .clear = true;
                let callable = runtime.iterator_callable_jsvalue(self.0.realm, &value);
                runtime.release_jsvalue(value)?;
                let callable = match callable? {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(value) => {
                        return self
                            .complete(NativeInvokeOutcome::Completion(Completion::Throw(value)));
                    }
                };
                self.0.phase = ConcatPhase::ReturnResult;
                Ok(ConcatStep::request_call(
                    callable,
                    JsValue::Object(iterator.into_handle()),
                    self,
                ))
            }
            ConcatPhase::ReturnResult => {
                self.complete(NativeInvokeOutcome::Completion(Completion::Return(value)))
            }
            ConcatPhase::Next => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant("concat next received completion"))
            }
        }
    }
    pub(crate) fn next(
        self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<ConcatStep, RuntimeError> {
        if !matches!(self.0.phase, ConcatPhase::Next) {
            return Err(RuntimeError::Invariant("concat next reply phase mismatch"));
        }
        match reply {
            ObjectIteratorStep::Yield(value) => {
                self.complete(NativeInvokeOutcome::IteratorNextRaw { value, done: false })
            }
            ObjectIteratorStep::Throw(value) => {
                self.complete(NativeInvokeOutcome::Completion(Completion::Throw(value)))
            }
            ObjectIteratorStep::Done => {
                runtime.advance_iterator_concat(self.concat()?)?;
                self.advance(runtime)
            }
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConcatStep,
) -> Result<NativeInvokeOutcome, RuntimeError> {
    loop {
        step = match step {
            ConcatStep::Complete(result) => return Ok(result),
            ConcatStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            ConcatStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, Vec::new())?,
                )?
            }
            ConcatStep::Next { mut resume } => {
                let iterator = resume.take_next_iterator();
                let method = resume.take_next_method();
                resume.next(
                    runtime,
                    super::step::finish_next(
                        runtime,
                        realm,
                        super::step::NextStep::start_jsvalue(runtime, realm, iterator, method)?,
                    )?,
                )?
            }
        };
    }
}

#[derive(Default)]
struct ConcatStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
}
impl ConcatStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: ConcatResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        mut resume: ConcatResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        Self::Call { resume }
    }
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: ConcatResume,
    ) -> Self {
        resume.0.pending_effect.next_iterator = Some(iterator);
        resume.0.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
}
impl ConcatResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("ConcatStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("ConcatStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("ConcatStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("ConcatStep Call receiver")
    }
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .next_iterator
            .take()
            .expect("ConcatStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.0
            .pending_effect
            .next_method
            .take()
            .expect("ConcatStep Next method")
    }
}
const _: () = assert!(std::mem::size_of::<ConcatStep>() <= 56);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ConcatStep>() <= 64);
