//! Object iterator consumers preserve pinned acquisition and close boundaries.
use super::ObjectIteratorStep;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        iterator::step::{CloseStep, NextStep, finish_close, finish_next},
        native::{ArrayPushKind, NativeFunctionId},
    },
    heap::ContextId,
    object::{
        CallableRef, ObjectRef, OwnedPropertyDescriptor, PropertyKey, WellKnownSymbol,
        operations::InternalDefineResult,
    },
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum IterationKind {
    Entries,
    Group,
    MapGroup,
}
impl IterationKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ObjectFromEntries => Some(Self::Entries),
            NativeFunctionId::ObjectGroupBy => Some(Self::Group),
            NativeFunctionId::Map(crate::engine::builtins::native::MapNativeKind::GroupBy)
            | NativeFunctionId::Set(crate::engine::builtins::native::SetNativeKind::GroupBy) => {
                Some(Self::MapGroup)
            }
            _ => None,
        }
    }
}
pub(crate) enum IterationStep {
    Complete(Completion),
    Read {
        resume: IterationResume,
    },
    Call {
        resume: IterationResume,
    },
    Next {
        resume: IterationResume,
    },
    Key {
        resume: IterationResume,
    },
    Define {
        resume: IterationResume,
    },
    Push {
        resume: IterationResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct IterationResume(Box<IterationResumeState>);
impl std::ops::Deref for IterationResume {
    type Target = IterationResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for IterationResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<IterationResume>() <= 8);
pub(crate) struct IterationResumeState {
    runtime: Runtime,
    pending_effect: IterationStepPending,
    realm: ContextId,
    kind: IterationKind,
    result: Option<ObjectRef>,
    callback: Option<CallableRef>,
    iterator: Option<ObjectRef>,
    next: JsValue,
    held: Option<JsValue>,
    index: u64,
    limit: u64,
    phase: Phase,
}
impl Drop for IterationResumeState {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        let next = std::mem::replace(&mut self.next, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(next);
        if let Some(value) = self.held.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.read_receiver.take() {
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
        if let Some(value) = self.pending_effect.next_method.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.key_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.push_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
enum Phase {
    IteratorMethod,
    Iterator,
    NextMethod,
    Next,
    EntryKey(ObjectRef),
    EntryValue,
    Key,
    Callback,
    Group(PropertyKey),
    GroupDefined(ObjectRef),
    EntryDefined(PropertyKey),
    Push,
}
impl IterationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: IterationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        Self::start_with_limit(
            runtime,
            realm,
            kind,
            invocation,
            arguments,
            (1u64 << 53) - 1,
        )
    }
    pub(super) fn start_with_limit(
        runtime: &Runtime,
        realm: ContextId,
        kind: IterationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
        limit: u64,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant(
                "Object iterator consumer requires a generic invocation",
            ));
        }
        // groupBy validates callback first; fromEntries allocates its result first.
        let callback = if !matches!(kind, IterationKind::Entries) {
            let value = arguments.readable.get(1).ok_or(RuntimeError::Invariant(
                "groupBy callback argv was not padded",
            ))?;
            let value = runtime.root_value(value)?;
            let callback = match value {
                Value::Object(object) => runtime.as_callable(&object)?,
                _ => None,
            };
            let Some(callback) = callback else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "not a function",
                    )?,
                )));
            };
            Some(callback)
        } else {
            None
        };
        let result = if matches!(kind, IterationKind::Entries) {
            Some(runtime.new_ordinary_object_in_realm(realm)?)
        } else {
            None
        };
        let iterable = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Object iterator argv was not padded",
        ))?;
        if matches!(iterable, JsValue::Null | JsValue::Undefined) {
            let base = if matches!(iterable, JsValue::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'Symbol.iterator' of {base}"),
                )?,
            )));
        }
        let resume = IterationResume(Box::new(IterationResumeState {
            runtime: runtime.clone(),
            pending_effect: IterationStepPending::default(),
            realm,
            kind,
            result,
            callback,
            iterator: None,
            next: JsValue::Undefined,
            held: Some(runtime.dup_jsvalue(iterable)?),
            index: 0,
            limit,
            phase: Phase::IteratorMethod,
        }));
        Ok(Self::request_read(
            runtime.dup_jsvalue(iterable)?,
            PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume,
        ))
    }
}
impl IterationResume {
    fn iterator(&self) -> Result<ObjectRef, RuntimeError> {
        self.0
            .iterator
            .clone()
            .ok_or(RuntimeError::Invariant("Object iterator not acquired"))
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.0.result.clone().ok_or(RuntimeError::Invariant(
            "Object iterator result not allocated",
        ))
    }
    fn abrupt(mut self, value: JsValue) -> IterationStep {
        let close = matches!(self.0.kind, IterationKind::Entries)
            || matches!(self.0.phase, Phase::Callback | Phase::Key);
        if close && let Some(iterator) = self.0.iterator.take() {
            IterationStep::Close {
                iterator,
                completion: Completion::Throw(value),
            }
        } else {
            IterationStep::Complete(Completion::Throw(value))
        }
    }
    fn next_step(mut self, runtime: &Runtime) -> Result<IterationStep, RuntimeError> {
        if !matches!(self.0.kind, IterationKind::Entries) && self.0.index >= self.0.limit {
            return Ok(IterationStep::Close {
                iterator: self.iterator()?,
                completion: Completion::Throw(runtime.new_native_error_jsvalue(
                    self.0.realm,
                    NativeErrorKind::Type,
                    "too many elements",
                )?),
            });
        }
        self.0.phase = Phase::Next;
        Ok(IterationStep::request_next(
            self.iterator()?,
            runtime.dup_jsvalue(&self.0.next)?,
            self,
        ))
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<IterationStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Next) {
            return Err(RuntimeError::Invariant(
                "Object iterator step has wrong reply phase",
            ));
        }
        let value = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(self.abrupt(value));
            }
            ObjectIteratorStep::Done => {
                let result = self.result()?;
                return Ok(IterationStep::Complete(Completion::Return(
                    JsValue::Object(result.into_handle()),
                )));
            }
            ObjectIteratorStep::Yield(value) => value,
        };
        match self.0.kind {
            IterationKind::Entries => {
                let JsValue::Object(item) = value else {
                    runtime.release_jsvalue(value)?;
                    let value = runtime.new_native_error_jsvalue(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return Ok(self.abrupt(value));
                };
                let item = ObjectRef::from_owned_handle(runtime.clone(), item);
                self.0.phase = Phase::EntryKey(item.clone());
                Ok(IterationStep::request_read(
                    JsValue::Object(item.into_handle()),
                    runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal1)?,
                    self,
                ))
            }
            IterationKind::Group | IterationKind::MapGroup => {
                self.0.held = Some(value);
                let callable = self
                    .0
                    .callback
                    .clone()
                    .ok_or(RuntimeError::Invariant("groupBy callback missing"))?;
                let receiver =
                    JsValue::Object(runtime.global_object_for_realm(self.0.realm)?.into_handle());
                let argument = match runtime.dup_jsvalue(self.0.held.as_ref().expect("group item"))
                {
                    Ok(value) => value,
                    Err(error) => {
                        runtime.release_jsvalue(receiver)?;
                        return Err(error);
                    }
                };
                let arguments = vec![
                    argument,
                    crate::engine::value::number::operations::Number::compact(self.0.index as f64)
                        .into(),
                ];
                self.0.phase = Phase::Callback;
                Ok(IterationStep::request_call(
                    callable, receiver, arguments, self,
                ))
            }
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<IterationStep, RuntimeError> {
        let value = match reply {
            Completion::Throw(value) => return Ok(self.abrupt(value)),
            Completion::Return(value) => value,
        };
        let phase = std::mem::replace(&mut self.0.phase, Phase::Next);
        match phase {
            Phase::IteratorMethod => {
                let callable = match value {
                    JsValue::Object(id) => {
                        runtime.as_callable(&ObjectRef::from_owned_handle(runtime.clone(), id))?
                    }
                    value => {
                        runtime.release_jsvalue(value)?;
                        None
                    }
                };
                let Some(callable) = callable else {
                    return Ok(IterationStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "value is not iterable",
                        )?,
                    )));
                };
                self.0.phase = Phase::Iterator;
                let iterable = self.0.held.take().expect("iterator source");
                Ok(IterationStep::request_call(
                    callable,
                    iterable,
                    Vec::new(),
                    self,
                ))
            }
            Phase::Iterator => {
                let JsValue::Object(id) = value else {
                    runtime.release_jsvalue(value)?;
                    return Ok(IterationStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), id);
                self.0.iterator = Some(iterator.clone());
                self.0.phase = Phase::NextMethod;
                let key =
                    runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
                Ok(IterationStep::request_read(
                    JsValue::Object(iterator.into_handle()),
                    key,
                    self,
                ))
            }
            Phase::NextMethod => {
                let previous = std::mem::replace(&mut self.0.next, value);
                runtime.release_jsvalue(previous)?;
                match self.0.kind {
                    IterationKind::Group => self.0.result = Some(runtime.new_object(None)?),
                    IterationKind::MapGroup => {
                        self.0.result = Some(runtime.new_map_in_realm(self.0.realm)?)
                    }
                    IterationKind::Entries => {}
                }
                self.next_step(runtime)
            }
            Phase::EntryKey(item) => {
                self.0.held = Some(value);
                self.0.phase = Phase::EntryValue;
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal2)?;
                Ok(IterationStep::request_read(
                    JsValue::Object(item.into_handle()),
                    key,
                    self,
                ))
            }
            Phase::EntryValue => {
                let key = self.0.held.replace(value).expect("entry key");
                self.0.phase = Phase::Key;
                Ok(IterationStep::request_key(key, self))
            }
            Phase::Callback => {
                if matches!(self.0.kind, IterationKind::MapGroup) {
                    let key = Runtime::normalized_map_key(value);
                    let groups = match self.result() {
                        Ok(v) => v,
                        Err(e) => {
                            runtime.release_jsvalue(key)?;
                            return Err(e);
                        }
                    };
                    let found = match runtime.find_map_record(&groups, &key) {
                        Ok(v) => v,
                        Err(e) => {
                            runtime.release_jsvalue(key)?;
                            return Err(e);
                        }
                    };
                    let group = match found {
                        Some((_, raw)) => {
                            let group = match raw {
                                crate::engine::heap::RawValue::Object(id) => {
                                    ObjectRef::from_borrowed_handle(runtime.clone(), id)
                                        .map_err(RuntimeError::from)
                                }
                                _ => Err(RuntimeError::Invariant(
                                    "Map.groupBy result contained a non-Array group",
                                )),
                            };
                            runtime.release_jsvalue(key)?;
                            group?
                        }
                        None => {
                            let group = match runtime.new_array(self.0.realm) {
                                Ok(v) => v,
                                Err(e) => {
                                    runtime.release_jsvalue(key)?;
                                    return Err(e);
                                }
                            };
                            runtime.set_map_record(
                                &groups,
                                key,
                                JsValue::Object(group.clone().into_handle()),
                            )?;
                            group
                        }
                    };
                    self.0.phase = Phase::Push;
                    let item = self.0.held.take().expect("group item");
                    return Ok(IterationStep::request_push(group, item, self));
                }
                self.0.phase = Phase::Key;
                Ok(IterationStep::request_key(value, self))
            }
            Phase::Group(key) => {
                let group = match value {
                    JsValue::Undefined => {
                        let group = runtime.new_array(self.0.realm)?;
                        self.0.phase = Phase::GroupDefined(group.clone());
                        let result = self.result()?;
                        return Ok(IterationStep::request_define(
                            result,
                            key,
                            OwnedPropertyDescriptor::data(
                                runtime,
                                JsValue::Object(group.into_handle()),
                            ),
                            self,
                        ));
                    }
                    JsValue::Object(id) => ObjectRef::from_owned_handle(runtime.clone(), id),
                    value => {
                        runtime.release_jsvalue(value)?;
                        return Err(RuntimeError::Invariant(
                            "Object.groupBy result contained a non-Array group",
                        ));
                    }
                };
                self.0.phase = Phase::Push;
                let item = self.0.held.take().expect("group item");
                Ok(IterationStep::request_push(group, item, self))
            }
            Phase::Push => {
                runtime.release_jsvalue(value)?;
                self.0.index = self.0.index.checked_add(1).ok_or(RuntimeError::Invariant(
                    "Object.groupBy index overflowed Uint64",
                ))?;
                self.next_step(runtime)
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "Object iterator consumer received wrong completion reply",
                ))
            }
        }
    }
    pub(crate) fn key(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<IterationStep, RuntimeError> {
        let value = match reply {
            Completion::Throw(value) => return Ok(self.abrupt(value)),
            Completion::Return(value) => value,
        };
        let key = match runtime.property_key_from_primitive_jsvalue(self.0.realm, value)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(self.abrupt(runtime.into_jsvalue(value)?));
            }
        };
        let Phase::Key = std::mem::replace(&mut self.0.phase, Phase::Next) else {
            return Err(RuntimeError::Invariant(
                "Object iterator key has wrong phase",
            ));
        };
        match self.0.kind {
            IterationKind::Entries => {
                self.0.phase = Phase::EntryDefined(key.clone());
                Ok(IterationStep::request_define(
                    self.result()?,
                    key,
                    OwnedPropertyDescriptor::data(runtime, self.0.held.take().expect("entry item")),
                    self,
                ))
            }
            IterationKind::Group => {
                self.0.phase = Phase::Group(key.clone());
                Ok(IterationStep::request_read(
                    JsValue::Object(self.result()?.into_handle()),
                    key,
                    self,
                ))
            }
            IterationKind::MapGroup => Err(RuntimeError::Invariant(
                "Map.groupBy received a property-key reply",
            )),
        }
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<InternalDefineResult>,
    ) -> Result<IterationStep, RuntimeError> {
        let result = match reply {
            NativeConversion::Throw(value) => {
                return Ok(self.abrupt(runtime.into_jsvalue(value)?));
            }
            NativeConversion::Value(result) => result,
        };
        match std::mem::replace(&mut self.0.phase, Phase::Next) {
            Phase::EntryDefined(key) => {
                if let Some(value) = runtime.finish_define_property_or_throw(
                    self.0.realm,
                    &key,
                    NativeConversion::Value(result),
                )? {
                    return Ok(self.abrupt(runtime.into_jsvalue(value)?));
                }
                self.next_step(runtime)
            }
            Phase::GroupDefined(group) => {
                if !matches!(result, InternalDefineResult::Defined) {
                    return Err(RuntimeError::Invariant(
                        "fresh Object.groupBy result rejected a group property",
                    ));
                }
                self.0.phase = Phase::Push;
                Ok(IterationStep::request_push(
                    group,
                    self.0.held.take().expect("group item"),
                    self,
                ))
            }
            _ => Err(RuntimeError::Invariant(
                "Object iterator definition has wrong phase",
            )),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: IterationStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            IterationStep::Complete(result) => return Ok(result),
            IterationStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
            IterationStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            IterationStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = runtime.root_and_release_jsvalue(resume.take_call_receiver())?;
                let arguments = resume
                    .take_call_arguments()
                    .into_iter()
                    .map(|value| runtime.root_and_release_jsvalue(value))
                    .collect::<Result<Vec<_>, _>>()?;
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
            IterationStep::Next { mut resume } => {
                let iterator = resume.take_next_iterator();
                let method = runtime.root_and_release_jsvalue(resume.take_next_method())?;
                resume.next(
                    runtime,
                    finish_next(
                        runtime,
                        realm,
                        NextStep::start(runtime, realm, iterator, method)?,
                    )?,
                )?
            }
            IterationStep::Key { mut resume } => {
                let value = resume.take_key_value();
                {
                    let result = runtime.to_primitive_jsvalue(
                        realm,
                        value,
                        crate::engine::vm::ToPrimitiveHint::String,
                    )?;
                    resume.key(runtime, result)?
                }
            }
            IterationStep::Define { mut resume } => {
                let object = resume.take_define_object();
                let key = resume.take_define_key();
                let descriptor = resume.take_define_descriptor();
                resume.defined(
                    runtime,
                    runtime.internal_define_owned_property(realm, &object, &key, descriptor)?,
                )?
            }
            IterationStep::Push { mut resume } => {
                let object = resume.take_push_object();
                let value = resume.take_push_value();
                resume.resume(
                    runtime,
                    runtime.call_array_prototype_push(
                        realm,
                        ArrayPushKind::Push,
                        NativeInvocation::Call {
                            this_value: runtime.into_jsvalue(Value::Object(object))?,
                        },
                        &NativeArguments {
                            actual_arg_count: 1,
                            readable: vec![value],
                        },
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
    fn from_entries_unpublished_result_is_owned_until_abandonment() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let iterable = runtime.new_object(None).unwrap();
        let iterable_id = iterable.object_id();
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![runtime.into_jsvalue(Value::Object(iterable)).unwrap()],
        };
        let IterationStep::Read { mut resume } = IterationStep::start(
            &runtime,
            context.realm,
            IterationKind::Entries,
            &NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &arguments,
        )
        .unwrap() else {
            panic!("iterator method read expected")
        };
        runtime
            .release_jsvalue(resume.take_read_receiver())
            .unwrap();
        let _ = resume.take_read_key();

        let result_id = resume.result.as_ref().unwrap().object_id();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
        runtime.run_gc().unwrap();
        for id in [iterable_id, result_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in [iterable_id, result_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
    }
}

#[derive(Default)]
struct IterationStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
    key_value: Option<JsValue>,
    define_object: Option<ObjectRef>,
    define_key: Option<PropertyKey>,
    define_descriptor: Option<OwnedPropertyDescriptor>,
    push_object: Option<ObjectRef>,
    push_value: Option<JsValue>,
}
impl IterationStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: IterationResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: IterationResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: IterationResume,
    ) -> Self {
        resume.0.pending_effect.next_iterator = Some(iterator);
        resume.0.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
    pub(crate) fn request_key(value: JsValue, mut resume: IterationResume) -> Self {
        resume.0.pending_effect.key_value = Some(value);
        Self::Key { resume }
    }
    pub(crate) fn request_define(
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OwnedPropertyDescriptor,
        mut resume: IterationResume,
    ) -> Self {
        resume.0.pending_effect.define_object = Some(object);
        resume.0.pending_effect.define_key = Some(key);
        resume.0.pending_effect.define_descriptor = Some(descriptor);
        Self::Define { resume }
    }
    pub(crate) fn request_push(
        object: ObjectRef,
        value: JsValue,
        mut resume: IterationResume,
    ) -> Self {
        resume.0.pending_effect.push_object = Some(object);
        resume.0.pending_effect.push_value = Some(value);
        Self::Push { resume }
    }
}
impl IterationResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("IterationStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("IterationStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("IterationStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("IterationStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("IterationStep Call arguments")
    }
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .next_iterator
            .take()
            .expect("IterationStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.0
            .pending_effect
            .next_method
            .take()
            .expect("IterationStep Next method")
    }
    pub(crate) fn take_key_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .key_value
            .take()
            .expect("IterationStep Key value")
    }
    pub(crate) fn take_define_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .define_object
            .take()
            .expect("IterationStep Define object")
    }
    pub(crate) fn take_define_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .define_key
            .take()
            .expect("IterationStep Define key")
    }
    pub(crate) fn take_define_descriptor(&mut self) -> OwnedPropertyDescriptor {
        self.0
            .pending_effect
            .define_descriptor
            .take()
            .expect("IterationStep Define descriptor")
    }
    pub(crate) fn take_push_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .push_object
            .take()
            .expect("IterationStep Push object")
    }
    pub(crate) fn take_push_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .push_value
            .take()
            .expect("IterationStep Push value")
    }
}
const _: () = assert!(std::mem::size_of::<IterationStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<IterationStep>() <= 64);
