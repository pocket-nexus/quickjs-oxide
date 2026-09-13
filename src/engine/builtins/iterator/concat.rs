use super::ObjectIteratorStep;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError, IteratorConcatData, IteratorConcatItem, ObjectData, RawValue},
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
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
        match finish(
            self,
            realm,
            ConcatStep::start(self, realm, ConcatKind::Create, &invocation, arguments)?,
        )? {
            NativeInvokeOutcome::Completion(result) => Ok(result),
            NativeInvokeOutcome::IteratorNextRaw { .. } => {
                Err(RuntimeError::Invariant("concat creation returned raw next"))
            }
        }
    }

    fn new_iterator_concat(
        &self,
        realm: ContextId,
        inputs: &[(ObjectRef, Value)],
    ) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.iterator_realm_data(realm)?.concat_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        let items = inputs
            .iter()
            .map(|(iterable, method)| {
                Ok(Some(IteratorConcatItem {
                    iterable: iterable.object_id(),
                    method: self.raw_property_value(method)?,
                }))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;

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
        drop(state);
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
            Err(HeapError::Invariant(_)) => Ok(NativeConversion::Throw(self.new_native_error(
                realm,
                NativeErrorKind::Type,
                "not an Iterator Concat",
            )?)),
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
        next: &Value,
    ) -> Result<(), RuntimeError> {
        let raw = self.raw_property_value(next)?;
        let mut state = self.0.state.borrow_mut();
        let retained_atoms = state.retain_raw_value_atoms([&raw])?;
        let cleanup = match state.heap.set_iterator_concat_next(concat.object_id(), raw) {
            Ok(cleanup) => cleanup,
            Err(error) => {
                state.release_atoms(retained_atoms)?;
                return Err(error.into());
            }
        };
        state.apply_cleanup(cleanup)
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
            NativeInvokeOutcome::IteratorNextRaw { value, done } => Ok(Completion::Return(
                Value::Object(self.new_iterator_result(realm, value, done)?),
            )),
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
        match finish(
            self,
            realm,
            ConcatStep::start(
                self,
                realm,
                ConcatKind::Return,
                &invocation,
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
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ConcatResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: ConcatResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: ConcatResume,
    },
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
pub(crate) struct ConcatResume {
    realm: ContextId,
    phase: ConcatPhase,
    guard: Option<ConcatGuard>,
}
enum ConcatPhase {
    Input {
        remaining: std::vec::IntoIter<Value>,
        inputs: Vec<(ObjectRef, Value)>,
        current: ObjectRef,
    },
    Iterator,
    Method(ObjectRef),
    Next,
    ReturnMethod(ObjectRef),
    ReturnResult,
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
            return ConcatResume::input(
                runtime,
                realm,
                arguments.readable[..arguments.actual_arg_count]
                    .to_vec()
                    .into_iter(),
                Vec::with_capacity(arguments.actual_arg_count),
            );
        }
        let concat = match runtime.iterator_receiver(realm, invocation.clone())? {
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
                Completion::Throw(runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "already running",
                )?),
            )));
        }
        if matches!(kind, ConcatKind::Return) && snapshot.iterator.is_none() {
            runtime.clear_iterator_concat(&concat)?;
            return Ok(Self::Complete(NativeInvokeOutcome::Completion(
                Completion::Return(Value::Undefined),
            )));
        }
        runtime.set_iterator_concat_running(&concat, true)?;
        let guard = ConcatGuard {
            runtime: runtime.clone(),
            concat,
            active: true,
            clear: false,
        };
        let mut resume = ConcatResume {
            realm,
            phase: ConcatPhase::Next,
            guard: Some(guard),
        };
        if matches!(kind, ConcatKind::Return) {
            let iterator = ObjectRef::from_borrowed_handle(
                runtime.clone(),
                snapshot
                    .iterator
                    .ok_or(RuntimeError::Invariant("concat return iterator missing"))?,
            )?;
            resume.phase = ConcatPhase::ReturnMethod(iterator.clone());
            return Ok(Self::Read {
                object: iterator,
                key: runtime.intern_property_key("return")?,
                resume,
            });
        }
        resume.advance(runtime)
    }
}
impl ConcatResume {
    fn input(
        runtime: &Runtime,
        realm: ContextId,
        mut remaining: std::vec::IntoIter<Value>,
        inputs: Vec<(ObjectRef, Value)>,
    ) -> Result<ConcatStep, RuntimeError> {
        let Some(input) = remaining.next() else {
            return Ok(ConcatStep::Complete(NativeInvokeOutcome::Completion(
                Completion::Return(Value::Object(runtime.new_iterator_concat(realm, &inputs)?)),
            )));
        };
        let Value::Object(current) = input else {
            return Ok(ConcatStep::Complete(NativeInvokeOutcome::Completion(
                Completion::Throw(runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "not an object",
                )?),
            )));
        };
        Ok(ConcatStep::Read {
            object: current.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: Self {
                realm,
                guard: None,
                phase: ConcatPhase::Input {
                    remaining,
                    inputs,
                    current,
                },
            },
        })
    }
    fn concat(&self) -> Result<&ObjectRef, RuntimeError> {
        self.guard
            .as_ref()
            .map(|guard| &guard.concat)
            .ok_or(RuntimeError::Invariant(
                "concat resume has no running owner",
            ))
    }
    fn complete(mut self, result: NativeInvokeOutcome) -> Result<ConcatStep, RuntimeError> {
        if let Some(guard) = &mut self.guard {
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
                value: Value::Undefined,
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
        let callable = match runtime
            .iterator_callable_value(self.realm, runtime.root_raw_value(&item.method)?)?
        {
            NativeConversion::Value(callable) => callable,
            NativeConversion::Throw(_) => {
                return Err(RuntimeError::Invariant(
                    "Iterator Concat captured method lost its callable brand",
                ));
            }
        };
        self.phase = ConcatPhase::Iterator;
        Ok(ConcatStep::Call {
            callable,
            receiver: Value::Object(iterable),
            resume: self,
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
            self.phase = ConcatPhase::Method(iterator.clone());
            return Ok(ConcatStep::Read {
                object: iterator,
                key: runtime.intern_property_key("next")?,
                resume: self,
            });
        }
        let method = runtime.root_raw_value(&snapshot.next)?;
        self.phase = ConcatPhase::Next;
        Ok(ConcatStep::Next {
            iterator,
            method,
            resume: self,
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
        match std::mem::replace(&mut self.phase, ConcatPhase::Next) {
            ConcatPhase::Input {
                remaining,
                mut inputs,
                current,
            } => {
                if let NativeConversion::Throw(value) =
                    runtime.iterator_callable_value(self.realm, value.clone())?
                {
                    return self
                        .complete(NativeInvokeOutcome::Completion(Completion::Throw(value)));
                }
                inputs.push((current, value));
                Self::input(runtime, self.realm, remaining, inputs)
            }
            ConcatPhase::Iterator => {
                let Value::Object(iterator) = value else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self
                        .complete(NativeInvokeOutcome::Completion(Completion::Throw(error)));
                };
                runtime.set_iterator_concat_iterator(self.concat()?, &iterator)?;
                self.method(runtime, iterator)
            }
            ConcatPhase::Method(iterator) => {
                runtime.set_iterator_concat_next(self.concat()?, &value)?;
                Ok(ConcatStep::Next {
                    iterator,
                    method: value,
                    resume: self,
                })
            }
            ConcatPhase::ReturnMethod(iterator) => {
                self.guard
                    .as_mut()
                    .ok_or(RuntimeError::Invariant("concat return owner missing"))?
                    .clear = true;
                let callable = match runtime.iterator_callable_value(self.realm, value)? {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(value) => {
                        return self
                            .complete(NativeInvokeOutcome::Completion(Completion::Throw(value)));
                    }
                };
                self.phase = ConcatPhase::ReturnResult;
                Ok(ConcatStep::Call {
                    callable,
                    receiver: Value::Object(iterator),
                    resume: self,
                })
            }
            ConcatPhase::ReturnResult => {
                self.complete(NativeInvokeOutcome::Completion(Completion::Return(value)))
            }
            ConcatPhase::Next => Err(RuntimeError::Invariant("concat next received completion")),
        }
    }
    pub(crate) fn next(
        self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<ConcatStep, RuntimeError> {
        if !matches!(self.phase, ConcatPhase::Next) {
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
            ConcatStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ConcatStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            ConcatStep::Next {
                iterator,
                method,
                resume,
            } => resume.next(
                runtime,
                super::step::finish_next(
                    runtime,
                    realm,
                    super::step::NextStep::start(runtime, realm, iterator, method)?,
                )?,
            )?,
        };
    }
}
