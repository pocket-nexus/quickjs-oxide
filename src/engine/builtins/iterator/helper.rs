//! Lazy helper resumes own their roots and running flag through every child reply.
use super::{
    ObjectIteratorStep,
    step::{CloseStep, NextStep, finish_close, finish_next},
};
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError, IteratorHelperKind, IteratorResumeKind},
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsValue, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum HelperResumeStep {
    Complete(Completion),
    Read { resume: HelperResume },
    Next { resume: HelperResume },
    Call { resume: HelperResume },
    Close { resume: HelperResume },
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
pub(crate) struct HelperResume(Box<HelperResumeState>);
impl std::ops::Deref for HelperResume {
    type Target = HelperResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for HelperResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<HelperResume>() <= 8);
pub(crate) struct HelperResumeState {
    runtime: Runtime,
    pending_effect: HelperResumeStepPending,
    realm: ContextId,
    guard: RunningHelper,
    source: ObjectRef,
    next: JsValue,
    callback: JsValue,
    inner: Option<ObjectRef>,
    kind: IteratorHelperKind,
    mode: IteratorResumeKind,
    count: i64,
    original_count: i64,
    method: JsValue,
    phase: Phase,
    item: Option<JsValue>,
    close_reason: Option<JsValue>,
    reply_value: Option<JsValue>,
}
impl Drop for HelperResumeState {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        for value in [
            self.item.take(),
            self.close_reason.take(),
            self.reply_value.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(Completion::Return(value) | Completion::Throw(value)) =
            self.pending_effect.close_completion.take()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        for value in [&mut self.next, &mut self.callback, &mut self.method] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
        if let Some(value) = self.pending_effect.next_method.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.call_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
}
enum Phase {
    Method,
    OuterNext { dropping: bool },
    Callback,
    MappedMethod(ObjectRef),
    MappedIterator,
    InnerMethod,
    InnerNext,
    CloseOuter,
    CloseTake,
    CloseInner,
}
impl HelperResumeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        mode: IteratorResumeKind,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let helper = match runtime.iterator_receiver(realm, invocation)? {
            NativeConversion::Value(helper) => helper,
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
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
                    runtime.new_native_error_jsvalue(
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
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "cannot invoke a running iterator",
                )?,
            )));
        }
        if state.done {
            return Ok(Self::Complete(Completion::Return(JsValue::Object(
                runtime
                    .new_iterator_result_jsvalue(realm, JsValue::Undefined, true)?
                    .into_handle(),
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
        let inner = state
            .inner
            .map(|inner| ObjectRef::from_borrowed_handle(runtime.clone(), inner))
            .transpose()?;
        let mut resume = HelperResume(Box::new(HelperResumeState {
            runtime: runtime.clone(),
            pending_effect: HelperResumeStepPending::default(),
            realm,
            guard,
            source,
            next: JsValue::Undefined,
            callback: JsValue::Undefined,
            inner,
            kind: state.kind,
            mode,
            count: state.count,
            original_count: state.count,
            method: JsValue::Undefined,
            item: None,
            close_reason: None,
            reply_value: None,
            phase: Phase::Method,
        }));
        resume.next = runtime.dup_iterator_raw(&state.next)?;
        resume.callback = runtime.dup_iterator_raw(&state.callback)?;
        resume.begin(runtime)
    }
}
impl HelperResume {
    fn done(
        mut self,
        runtime: &Runtime,
        value: JsValue,
        done: bool,
    ) -> Result<HelperResumeStep, RuntimeError> {
        self.0.reply_value = Some(value);
        self.0
            .guard
            .finish(done || self.0.mode == IteratorResumeKind::Return)?;
        let value = self.0.reply_value.take().expect("helper result");
        let result = runtime.new_iterator_result_jsvalue(self.0.realm, value, done)?;
        Ok(HelperResumeStep::Complete(Completion::Return(
            JsValue::Object(result.into_handle()),
        )))
    }
    fn fail(
        mut self,
        _runtime: &Runtime,
        value: JsValue,
        close_outer: bool,
    ) -> Result<HelperResumeStep, RuntimeError> {
        if close_outer {
            self.0.phase = Phase::CloseOuter;
            return Ok({
                let __pending_field_iterator = self.0.source.clone();
                let __pending_field_completion = Completion::Throw(value);
                let __pending_field_resume = self;
                HelperResumeStep::request_close(
                    __pending_field_iterator,
                    __pending_field_completion,
                    __pending_field_resume,
                )
            });
        }
        let done = self.0.mode == IteratorResumeKind::Return
            || (self.0.kind == IteratorHelperKind::Take && self.0.original_count == 0);
        self.0.reply_value = Some(value);
        self.0.guard.finish(done)?;
        Ok(HelperResumeStep::Complete(Completion::Throw(
            self.0.reply_value.take().expect("helper error"),
        )))
    }
    fn begin(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        if self.0.kind == IteratorHelperKind::FlatMap && self.0.inner.is_some() {
            return self.inner_method(runtime);
        }
        if self.0.kind == IteratorHelperKind::Take && self.0.count <= 0 {
            self.0.phase = Phase::CloseTake;
            return Ok({
                let __pending_field_iterator = self.0.source.clone();
                let __pending_field_completion = Completion::Return(JsValue::Undefined);
                let __pending_field_resume = self;
                HelperResumeStep::request_close(
                    __pending_field_iterator,
                    __pending_field_completion,
                    __pending_field_resume,
                )
            });
        }
        self.0.phase = Phase::Method;
        if self.0.mode == IteratorResumeKind::Next {
            let method = runtime.dup_jsvalue(&self.0.next)?;
            self.method(runtime, method)
        } else {
            Ok({
                let __pending_field_object = self.0.source.clone();
                let __pending_field_key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Return)?;
                let __pending_field_resume = self;
                HelperResumeStep::request_read(
                    __pending_field_object,
                    __pending_field_key,
                    __pending_field_resume,
                )
            })
        }
    }
    fn method(
        mut self,
        runtime: &Runtime,
        method: JsValue,
    ) -> Result<HelperResumeStep, RuntimeError> {
        runtime.release_jsvalue(std::mem::replace(&mut self.0.method, method))?;
        if self.0.kind == IteratorHelperKind::Take {
            self.0.count -= 1;
            runtime.set_helper_count(&self.0.guard.helper, self.0.count)?;
        }
        self.outer_next(runtime)
    }
    fn outer_next(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        let dropping = self.0.kind == IteratorHelperKind::Drop && self.0.count > 0;
        if dropping {
            self.0.count -= 1;
            runtime.set_helper_count(&self.0.guard.helper, self.0.count)?;
        }
        self.0.phase = Phase::OuterNext { dropping };
        Ok({
            let __pending_field_iterator = self.0.source.clone();
            let __pending_field_method = runtime.dup_jsvalue(&self.0.method)?;
            let __pending_field_resume = self;
            HelperResumeStep::request_next(
                __pending_field_iterator,
                __pending_field_method,
                __pending_field_resume,
            )
        })
    }
    fn inner_method(mut self, runtime: &Runtime) -> Result<HelperResumeStep, RuntimeError> {
        let object = self
            .0
            .inner
            .clone()
            .ok_or(RuntimeError::Invariant("flatMap inner iterator missing"))?;
        self.0.phase = Phase::InnerMethod;
        let key = runtime.intern_property_key(if self.0.mode == IteratorResumeKind::Next {
            "next"
        } else {
            "return"
        })?;
        Ok({
            let __pending_field_object = object;
            let __pending_field_key = key;
            let __pending_field_resume = self;
            HelperResumeStep::request_read(
                __pending_field_object,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
    fn close_inner(mut self, original: Option<JsValue>) -> Result<HelperResumeStep, RuntimeError> {
        self.0.close_reason = original;
        let iterator = self
            .0
            .inner
            .clone()
            .ok_or(RuntimeError::Invariant("flatMap close inner missing"))?;
        // Even with a pending failure, this is a normal close: its exception
        // replaces the inner failure before the outer preserving close.
        self.0.phase = Phase::CloseInner;
        Ok({
            let __pending_field_iterator = iterator;
            let __pending_field_completion = Completion::Return(JsValue::Undefined);
            let __pending_field_resume = self;
            HelperResumeStep::request_close(
                __pending_field_iterator,
                __pending_field_completion,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<HelperResumeStep, RuntimeError> {
        if matches!(self.0.phase, Phase::InnerNext) {
            return match reply {
                ObjectIteratorStep::Yield(value) => self.done(runtime, value, false),
                ObjectIteratorStep::Done => self.close_inner(None),
                ObjectIteratorStep::Throw(value) => self.close_inner(Some(value)),
            };
        }
        let Phase::OuterNext { dropping } = std::mem::replace(&mut self.0.phase, Phase::Method)
        else {
            if let ObjectIteratorStep::Yield(value) | ObjectIteratorStep::Throw(value) = reply {
                runtime.release_jsvalue(value)?;
            }
            return Err(RuntimeError::Invariant(
                "helper iterator reply has wrong phase",
            ));
        };
        let value = match reply {
            ObjectIteratorStep::Throw(value) => {
                return self.fail(runtime, value, false);
            }
            ObjectIteratorStep::Done => return self.done(runtime, JsValue::Undefined, true),
            ObjectIteratorStep::Yield(value) => value,
        };
        if dropping {
            runtime.release_jsvalue(value)?;
            if self.0.mode == IteratorResumeKind::Return {
                return self.done(runtime, JsValue::Undefined, true);
            }
            return self.outer_next(runtime);
        }
        if self.0.mode == IteratorResumeKind::Return
            || matches!(
                self.0.kind,
                IteratorHelperKind::Drop | IteratorHelperKind::Take
            )
        {
            return self.done(runtime, value, false);
        }
        if let Some(previous) = self.0.item.replace(value) {
            runtime.release_jsvalue(previous)?;
        }
        let callable = match runtime.iterator_callable_jsvalue(self.0.realm, &self.0.callback)? {
            NativeConversion::Value(callback) => callback,
            NativeConversion::Throw(discarded) => {
                runtime.release_jsvalue(discarded)?;
                return Err(RuntimeError::Invariant(
                    "Iterator Helper callback lost its callable brand",
                ));
            }
        };
        let index = self.0.count;
        self.0.count = self.0.count.wrapping_add(1);
        runtime.set_helper_count(&self.0.guard.helper, self.0.count)?;
        self.0.phase = Phase::Callback;
        Ok({
            let __pending_field_callable = callable;
            let __pending_field_receiver = JsValue::Undefined;
            let __pending_field_arguments = vec![
                runtime.dup_jsvalue(self.0.item.as_ref().expect("helper callback item"))?,
                JsValue::Float(index as f64),
            ];
            let __pending_field_resume = self;
            HelperResumeStep::request_call(
                __pending_field_callable,
                __pending_field_receiver,
                __pending_field_arguments,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<HelperResumeStep, RuntimeError> {
        let phase = std::mem::replace(&mut self.0.phase, Phase::Method);
        if let Phase::CloseInner = phase {
            let throwing = matches!(reply, Completion::Throw(_));
            let (Completion::Return(value) | Completion::Throw(value)) = reply;
            self.0.reply_value = Some(value);
            runtime.set_helper_inner(&self.0.guard.helper, None)?;
            self.0.inner = None;
            let value = self.0.reply_value.take().expect("inner close result");
            return if let Some(original) = self.0.close_reason.take() {
                let value = if throwing {
                    runtime.release_jsvalue(original)?;
                    value
                } else {
                    runtime.release_jsvalue(value)?;
                    original
                };
                self.fail(runtime, value, true)
            } else {
                runtime.release_jsvalue(value)?;
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
                if matches!(phase, Phase::Callback) {
                    if let Some(item) = self.0.item.take() {
                        runtime.release_jsvalue(item)?;
                    }
                }
                return match phase {
                    Phase::InnerMethod => self.close_inner(Some(value)),
                    Phase::CloseTake | Phase::CloseOuter => self.fail(runtime, value, false),
                    _ => self.fail(runtime, value, true),
                };
            }
        };
        self.0.reply_value = Some(value);
        match phase {
            Phase::Method => {
                let value = self.0.reply_value.take().expect("helper method");
                self.method(runtime, value)
            }
            Phase::Callback => match self.0.kind {
                IteratorHelperKind::Map => {
                    runtime.release_jsvalue(self.0.item.take().expect("map item"))?;
                    let value = self.0.reply_value.take().expect("mapped value");
                    self.done(runtime, value, false)
                }
                IteratorHelperKind::Filter => {
                    let keep = runtime.value_to_boolean_jsvalue(
                        self.0.reply_value.as_ref().expect("filter result"),
                    )?;
                    runtime.release_jsvalue(self.0.reply_value.take().expect("filter result"))?;
                    let item = self.0.item.take().expect("filter item");
                    if keep {
                        self.done(runtime, item, false)
                    } else {
                        runtime.release_jsvalue(item)?;
                        self.outer_next(runtime)
                    }
                }
                IteratorHelperKind::FlatMap => {
                    runtime.release_jsvalue(self.0.item.take().expect("flatMap item"))?;
                    let value = self.0.reply_value.take().expect("mapped value");
                    let JsValue::Object(mapped) = value else {
                        runtime.release_jsvalue(value)?;
                        let error = runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?;
                        return self.fail(runtime, error, true);
                    };
                    let mapped = ObjectRef::from_owned_handle(runtime.clone(), mapped);
                    self.0.phase = Phase::MappedMethod(mapped.clone());
                    let key =
                        PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator));
                    Ok(HelperResumeStep::request_read(mapped, key, self))
                }
                _ => Err(RuntimeError::Invariant(
                    "non-callback helper received callback reply",
                )),
            },
            Phase::MappedMethod(mapped) => {
                if matches!(
                    self.0.reply_value.as_ref(),
                    Some(JsValue::Undefined | JsValue::Null)
                ) {
                    runtime.release_jsvalue(self.0.reply_value.take().expect("mapped method"))?;
                    runtime.set_helper_inner(&self.0.guard.helper, Some(&mapped))?;
                    self.0.inner = Some(mapped);
                    return self.inner_method(runtime);
                }
                let callable = runtime.iterator_callable_jsvalue(
                    self.0.realm,
                    self.0.reply_value.as_ref().expect("mapped method"),
                )?;
                runtime.release_jsvalue(self.0.reply_value.take().expect("mapped method"))?;
                let callable = match callable {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(value) => {
                        return self.fail(runtime, value, true);
                    }
                };
                self.0.phase = Phase::MappedIterator;
                Ok(HelperResumeStep::request_call(
                    callable,
                    JsValue::Object(mapped.into_handle()),
                    Vec::new(),
                    self,
                ))
            }
            Phase::MappedIterator => {
                let value = self.0.reply_value.take().expect("mapped iterator");
                let JsValue::Object(iterator) = value else {
                    runtime.release_jsvalue(value)?;
                    let error = runtime.new_native_error_jsvalue(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.fail(runtime, error, true);
                };
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), iterator);
                runtime.set_helper_inner(&self.0.guard.helper, Some(&iterator))?;
                self.0.inner = Some(iterator);
                self.inner_method(runtime)
            }
            Phase::InnerMethod => {
                if self.0.mode == IteratorResumeKind::Return
                    && matches!(
                        self.0.reply_value.as_ref(),
                        Some(JsValue::Undefined | JsValue::Null)
                    )
                {
                    runtime.release_jsvalue(self.0.reply_value.take().expect("inner method"))?;
                    return self.close_inner(None);
                }
                let iterator = self
                    .0
                    .inner
                    .clone()
                    .ok_or(RuntimeError::Invariant("flatMap inner missing"))?;
                self.0.phase = Phase::InnerNext;
                let method = self.0.reply_value.take().expect("inner method");
                Ok(HelperResumeStep::request_next(iterator, method, self))
            }
            Phase::CloseTake => {
                runtime.release_jsvalue(self.0.reply_value.take().expect("close result"))?;
                self.done(runtime, JsValue::Undefined, true)
            }
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
            HelperResumeStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            HelperResumeStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
            HelperResumeStep::Next { mut resume } => {
                let iterator = resume.take_next_iterator();
                let method = resume.take_next_method();
                resume.next(
                    runtime,
                    finish_next(
                        runtime,
                        realm,
                        NextStep::start_jsvalue(runtime, realm, iterator, method)?,
                    )?,
                )?
            }
            HelperResumeStep::Close { mut resume } => {
                let iterator = resume.take_close_iterator();
                let completion = resume.take_close_completion();
                resume.resume(
                    runtime,
                    finish_close(
                        runtime,
                        realm,
                        CloseStep::start(runtime, realm, iterator, completion)?,
                    )?,
                )?
            }
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
        let callback = runtime.into_jsvalue(callback).unwrap();
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
        runtime.release_jsvalue(next).unwrap();
        runtime.release_jsvalue(callback).unwrap();
        let source_id = source.object_id();
        let helper_id = helper.object_id();
        let invocation = NativeInvocation::Call {
            this_value: runtime.into_jsvalue(Value::Object(helper.clone())).unwrap(),
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
        {
            let NativeInvocation::Call { this_value } = invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(this_value).unwrap();
        }
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
        let invocation = NativeInvocation::Call {
            this_value: runtime.into_jsvalue(Value::Object(helper.clone())).unwrap(),
        };
        let step = HelperResumeStep::start(
            &runtime,
            context.realm,
            IteratorResumeKind::Next,
            &invocation,
        )
        .unwrap();
        {
            let NativeInvocation::Call { this_value } = invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(this_value).unwrap();
        }
        let Completion::Return(value) = finish(&runtime, context.realm, step).unwrap() else {
            panic!("helper result expected")
        };
        let Value::Object(result) = runtime.root_and_release_jsvalue(value).unwrap() else {
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

#[derive(Default)]
struct HelperResumeStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    close_iterator: Option<ObjectRef>,
    close_completion: Option<Completion>,
}
impl HelperResumeStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: HelperResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: HelperResume,
    ) -> Self {
        resume.0.pending_effect.next_iterator = Some(iterator);
        resume.0.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: HelperResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_close(
        iterator: ObjectRef,
        completion: Completion,
        mut resume: HelperResume,
    ) -> Self {
        resume.0.pending_effect.close_iterator = Some(iterator);
        resume.0.pending_effect.close_completion = Some(completion);
        Self::Close { resume }
    }
}
impl HelperResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("HelperResumeStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("HelperResumeStep Read key")
    }
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .next_iterator
            .take()
            .expect("HelperResumeStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.0
            .pending_effect
            .next_method
            .take()
            .expect("HelperResumeStep Next method")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("HelperResumeStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("HelperResumeStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("HelperResumeStep Call arguments")
    }
    pub(crate) fn take_close_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .close_iterator
            .take()
            .expect("HelperResumeStep Close iterator")
    }
    pub(crate) fn take_close_completion(&mut self) -> Completion {
        self.0
            .pending_effect
            .close_completion
            .take()
            .expect("HelperResumeStep Close completion")
    }
}
const _: () = assert!(std::mem::size_of::<HelperResumeStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<HelperResumeStep>() <= 64);
