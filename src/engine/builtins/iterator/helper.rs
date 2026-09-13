//! Lazy helper resumes own their roots and running flag through every child reply.
use super::{
    ObjectIteratorStep,
    step::{CloseStep, NextStep, finish_close, finish_next},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError, IteratorHelperKind, IteratorResumeKind},
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum HelperResumeStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: HelperResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: HelperResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: HelperResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
        resume: HelperResume,
    },
}
struct RunningHelper {
    runtime: Runtime,
    helper: ObjectRef,
    active: bool,
}
impl RunningHelper {
    fn finish(&mut self, done: bool) -> Result<(), RuntimeError> {
        self.runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_iterator_helper_done_and_running(self.helper.object_id(), done, false)?;
        self.active = false;
        Ok(())
    }
}
impl Drop for RunningHelper {
    fn drop(&mut self) {
        if self.active {
            let _ = self
                .runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_iterator_helper_running(self.helper.object_id(), false);
        }
    }
}
pub(crate) struct HelperResume {
    realm: ContextId,
    guard: RunningHelper,
    source: ObjectRef,
    next: Value,
    callback: Value,
    inner: Option<ObjectRef>,
    kind: IteratorHelperKind,
    mode: IteratorResumeKind,
    count: i64,
    original_count: i64,
    method: Value,
    phase: Phase,
}
enum Phase {
    Method,
    OuterNext { dropping: bool },
    Callback(Value),
    MappedMethod(ObjectRef),
    MappedIterator,
    InnerMethod,
    InnerNext,
    CloseOuter,
    CloseTake,
    CloseInner { original: Option<Value> },
}
impl HelperResumeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        mode: IteratorResumeKind,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let helper = match runtime.iterator_receiver(realm, invocation.clone())? {
            NativeConversion::Value(helper) => helper,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let state_result = {
            runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_helper_state(helper.object_id())
        };
        let state = match state_result {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(
                        realm,
                        NativeErrorKind::Type,
                        "not an Iterator Helper",
                    )?,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        if state.executing {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "cannot invoke a running iterator",
                )?,
            )));
        }
        if state.done {
            return Ok(Self::Complete(Completion::Return(Value::Object(
                runtime.new_iterator_result(realm, Value::Undefined, true)?,
            ))));
        }
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_iterator_helper_running(helper.object_id(), true)?;
        let guard = RunningHelper {
            runtime: runtime.clone(),
            helper,
            active: true,
        };
        let source = ObjectRef::from_borrowed_handle(runtime.clone(), state.source)?;
        let next = runtime.root_raw_value(&state.next)?;
        let callback = runtime.root_raw_value(&state.callback)?;
        let inner = state
            .inner
            .map(|inner| ObjectRef::from_borrowed_handle(runtime.clone(), inner))
            .transpose()?;
        let resume = HelperResume {
            realm,
            guard,
            source,
            next,
            callback,
            inner,
            kind: state.kind,
            mode,
            count: state.count,
            original_count: state.count,
            method: Value::Undefined,
            phase: Phase::Method,
        };
        resume.begin(runtime)
    }
}
impl HelperResume {
    fn done(
        mut self,
        runtime: &Runtime,
        value: Value,
        done: bool,
    ) -> Result<HelperResumeStep, RuntimeError> {
        self.guard
            .finish(done || self.mode == IteratorResumeKind::Return)?;
        Ok(HelperResumeStep::Complete(Completion::Return(
            Value::Object(runtime.new_iterator_result(self.realm, value, done)?),
        )))
    }
    fn fail(
        mut self,
        runtime: &Runtime,
        value: Value,
        close_outer: bool,
    ) -> Result<HelperResumeStep, RuntimeError> {
        if close_outer {
            self.phase = Phase::CloseOuter;
            return Ok(HelperResumeStep::Close {
                iterator: self.source.clone(),
                completion: Completion::Throw(value),
                resume: self,
            });
        }
        let done = self.mode == IteratorResumeKind::Return
            || (self.kind == IteratorHelperKind::Take && self.original_count == 0);
        self.guard.finish(done)?;
        let _ = runtime;
        Ok(HelperResumeStep::Complete(Completion::Throw(value)))
    }
    fn begin(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        if self.kind == IteratorHelperKind::FlatMap && self.inner.is_some() {
            return self.inner_method(runtime);
        }
        if self.kind == IteratorHelperKind::Take && self.count <= 0 {
            self.phase = Phase::CloseTake;
            return Ok(HelperResumeStep::Close {
                iterator: self.source.clone(),
                completion: Completion::Return(Value::Undefined),
                resume: self,
            });
        }
        self.phase = Phase::Method;
        if self.mode == IteratorResumeKind::Next {
            let method = self.next.clone();
            self.method(runtime, method)
        } else {
            Ok(HelperResumeStep::Read {
                object: self.source.clone(),
                key: runtime.intern_property_key("return")?,
                resume: self,
            })
        }
    }
    fn method(
        mut self,
        runtime: &Runtime,
        method: Value,
    ) -> Result<HelperResumeStep, RuntimeError> {
        self.method = method;
        if self.kind == IteratorHelperKind::Take {
            self.count -= 1;
            runtime.set_helper_count(&self.guard.helper, self.count)?;
        }
        self.outer_next(runtime)
    }
    fn outer_next(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        let dropping = self.kind == IteratorHelperKind::Drop && self.count > 0;
        if dropping {
            self.count -= 1;
            runtime.set_helper_count(&self.guard.helper, self.count)?;
        }
        self.phase = Phase::OuterNext { dropping };
        Ok(HelperResumeStep::Next {
            iterator: self.source.clone(),
            method: self.method.clone(),
            resume: self,
        })
    }
    fn inner_method(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        let object = self
            .inner
            .clone()
            .ok_or(RuntimeError::Invariant("flatMap inner iterator missing"))?;
        self.phase = Phase::InnerMethod;
        let key = runtime.intern_property_key(if self.mode == IteratorResumeKind::Next {
            "next"
        } else {
            "return"
        })?;
        Ok(HelperResumeStep::Read {
            object,
            key,
            resume: self,
        })
    }
    fn close_inner(mut self, original: Option<Value>) -> Result<HelperResumeStep, RuntimeError> {
        let iterator = self
            .inner
            .clone()
            .ok_or(RuntimeError::Invariant("flatMap close inner missing"))?;
        // Even with a pending failure, this is a normal close: its exception
        // replaces the inner failure before the outer preserving close.
        self.phase = Phase::CloseInner { original };
        Ok(HelperResumeStep::Close {
            iterator,
            completion: Completion::Return(Value::Undefined),
            resume: self,
        })
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<HelperResumeStep, RuntimeError> {
        if matches!(self.phase, Phase::InnerNext) {
            return match reply {
                ObjectIteratorStep::Yield(value) => self.done(runtime, value, false),
                ObjectIteratorStep::Done => self.close_inner(None),
                ObjectIteratorStep::Throw(value) => self.close_inner(Some(value)),
            };
        }
        let Phase::OuterNext { dropping } = self.phase else {
            return Err(RuntimeError::Invariant(
                "helper iterator reply has wrong phase",
            ));
        };
        let value = match reply {
            ObjectIteratorStep::Throw(value) => return self.fail(runtime, value, false),
            ObjectIteratorStep::Done => return self.done(runtime, Value::Undefined, true),
            ObjectIteratorStep::Yield(value) => value,
        };
        if dropping {
            if self.mode == IteratorResumeKind::Return {
                return self.done(runtime, Value::Undefined, true);
            }
            return self.outer_next(runtime);
        }
        if self.mode == IteratorResumeKind::Return
            || matches!(
                self.kind,
                IteratorHelperKind::Drop | IteratorHelperKind::Take
            )
        {
            return self.done(runtime, value, false);
        }
        let callable = match runtime.iterator_callable_value(self.realm, self.callback.clone())? {
            NativeConversion::Value(callback) => callback,
            NativeConversion::Throw(_) => {
                return Err(RuntimeError::Invariant(
                    "Iterator Helper callback lost its callable brand",
                ));
            }
        };
        let index = self.count;
        self.count = self.count.wrapping_add(1);
        runtime.set_helper_count(&self.guard.helper, self.count)?;
        self.phase = Phase::Callback(value.clone());
        Ok(HelperResumeStep::Call {
            callable,
            receiver: Value::Undefined,
            arguments: vec![value, Value::number(index as f64)],
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<HelperResumeStep, RuntimeError> {
        let phase = std::mem::replace(&mut self.phase, Phase::Method);
        if let Phase::CloseInner { original } = phase {
            runtime.set_helper_inner(&self.guard.helper, None)?;
            self.inner = None;
            return if let Some(original) = original {
                let value = match reply {
                    Completion::Return(_) => original,
                    Completion::Throw(value) => value,
                };
                self.fail(runtime, value, true)
            } else {
                self.begin(runtime)
            };
        }
        self.reply(runtime, reply, phase)
    }

    fn reply(
        mut self,
        runtime: &Runtime,
        reply: Completion,
        phase: Phase,
    ) -> Result<HelperResumeStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return match phase {
                    Phase::InnerMethod => self.close_inner(Some(value)),
                    Phase::CloseTake | Phase::CloseOuter => self.fail(runtime, value, false),
                    _ => self.fail(runtime, value, true),
                };
            }
        };
        match phase {
            Phase::Method => self.method(runtime, value),
            Phase::Callback(item) => match self.kind {
                IteratorHelperKind::Map => self.done(runtime, value, false),
                IteratorHelperKind::Filter => {
                    if runtime.value_to_boolean(&value)? {
                        self.done(runtime, item, false)
                    } else {
                        self.outer_next(runtime)
                    }
                }
                IteratorHelperKind::FlatMap => {
                    let Value::Object(mapped) = value else {
                        let error = runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?;
                        return self.fail(runtime, error, true);
                    };
                    self.phase = Phase::MappedMethod(mapped.clone());
                    Ok(HelperResumeStep::Read {
                        object: mapped,
                        key: PropertyKey::from(
                            runtime.well_known_symbol(WellKnownSymbol::Iterator),
                        ),
                        resume: self,
                    })
                }
                _ => Err(RuntimeError::Invariant(
                    "non-callback helper received callback reply",
                )),
            },
            Phase::MappedMethod(mapped) => {
                if matches!(value, Value::Undefined | Value::Null) {
                    runtime.set_helper_inner(&self.guard.helper, Some(&mapped))?;
                    self.inner = Some(mapped);
                    return self.inner_method(runtime);
                }
                let callable = match runtime.iterator_callable_value(self.realm, value)? {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(value) => return self.fail(runtime, value, true),
                };
                self.phase = Phase::MappedIterator;
                Ok(HelperResumeStep::Call {
                    callable,
                    receiver: Value::Object(mapped),
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Phase::MappedIterator => {
                let Value::Object(iterator) = value else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.fail(runtime, error, true);
                };
                runtime.set_helper_inner(&self.guard.helper, Some(&iterator))?;
                self.inner = Some(iterator);
                self.inner_method(runtime)
            }
            Phase::InnerMethod => {
                if self.mode == IteratorResumeKind::Return
                    && matches!(value, Value::Undefined | Value::Null)
                {
                    return self.close_inner(None);
                }
                self.phase = Phase::InnerNext;
                Ok(HelperResumeStep::Next {
                    iterator: self
                        .inner
                        .clone()
                        .ok_or(RuntimeError::Invariant("flatMap inner missing"))?,
                    method: value,
                    resume: self,
                })
            }
            Phase::CloseTake => self.done(runtime, Value::Undefined, true),
            Phase::CloseOuter => Err(RuntimeError::Invariant(
                "preserving helper close returned normally",
            )),
            _ => Err(RuntimeError::Invariant(
                "helper completion reply has wrong phase",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: HelperResumeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            HelperResumeStep::Complete(result) => return Ok(result),
            HelperResumeStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            HelperResumeStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            HelperResumeStep::Next {
                iterator,
                method,
                resume,
            } => resume.next(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::start(runtime, realm, iterator, method)?,
                )?,
            )?,
            HelperResumeStep::Close {
                iterator,
                completion,
                resume,
            } => resume.resume(
                runtime,
                finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                )?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandoned_helper_request_releases_running_flag_and_owned_roots() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let Value::Object(helper) = context
            .eval("({ next() { return {value: 7, done: false}; } })")
            .unwrap()
        else {
            panic!("source expected")
        };
        let source = helper;
        let callback = context.eval("(function (x) { return x; })").unwrap();
        let next_key = runtime.intern_property_key("next").unwrap();
        let Completion::Return(next) = runtime
            .get_property_in_realm(context.realm, &source, &next_key)
            .unwrap()
        else {
            panic!("next expected")
        };
        let helper = runtime
            .new_iterator_helper(
                context.realm,
                &source,
                &next,
                &callback,
                0,
                IteratorHelperKind::Map,
            )
            .unwrap();
        let source_id = source.object_id();
        let helper_id = helper.object_id();
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(helper.clone()),
        };
        let step = HelperResumeStep::start(
            &runtime,
            context.realm,
            IteratorResumeKind::Next,
            &invocation,
        )
        .unwrap();
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_helper_state(helper_id)
                .unwrap()
                .executing
        );
        drop(source);
        drop(next);
        drop(callback);
        drop(invocation);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(source_id).is_ok());
        drop(step);
        assert!(
            !runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_helper_state(helper_id)
                .unwrap()
                .executing
        );
        let step = HelperResumeStep::start(
            &runtime,
            context.realm,
            IteratorResumeKind::Next,
            &NativeInvocation::Call {
                this_value: Value::Object(helper.clone()),
            },
        )
        .unwrap();
        let Completion::Return(Value::Object(result)) =
            finish(&runtime, context.realm, step).unwrap()
        else {
            panic!("helper result expected")
        };
        drop(result);
        drop(helper);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(source_id).is_err());
        assert!(runtime.0.state.borrow().heap.object(helper_id).is_err());
        drop(next_key);
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
