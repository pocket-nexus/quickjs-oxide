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
        CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        WellKnownSymbol, operations::InternalDefineResult,
    },
    value::{Value, conversion::NativeConversion},
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
    #[cfg(feature = "stack-vm")]
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
        receiver: Value,
        key: PropertyKey,
        resume: IterationResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: IterationResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: IterationResume,
    },
    Key {
        value: Value,
        resume: IterationResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: IterationResume,
    },
    Push {
        object: ObjectRef,
        value: Value,
        resume: IterationResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct IterationResume {
    realm: ContextId,
    kind: IterationKind,
    result: Option<ObjectRef>,
    callback: Option<CallableRef>,
    iterator: Option<ObjectRef>,
    next: Value,
    index: u64,
    limit: u64,
    phase: Phase,
}
enum Phase {
    IteratorMethod(Value),
    Iterator,
    NextMethod,
    Next,
    EntryKey(ObjectRef),
    EntryValue(Value),
    Key(Value),
    Callback(Value),
    Group(Value, PropertyKey),
    GroupDefined(Value, ObjectRef),
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
            let callback = match value {
                Value::Object(object) => runtime.as_callable(object)?,
                _ => None,
            };
            let Some(callback) = callback else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
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
        let iterable = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Object iterator argv was not padded",
            ))?;
        if matches!(iterable, Value::Null | Value::Undefined) {
            let base = if matches!(iterable, Value::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'Symbol.iterator' of {base}"),
                )?,
            )));
        }
        Ok(Self::Read {
            receiver: iterable.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: IterationResume {
                realm,
                kind,
                result,
                callback,
                iterator: None,
                next: Value::Undefined,
                index: 0,
                limit,
                phase: Phase::IteratorMethod(iterable),
            },
        })
    }
}
impl IterationResume {
    fn iterator(&self) -> Result<ObjectRef, RuntimeError> {
        self.iterator
            .clone()
            .ok_or(RuntimeError::Invariant("Object iterator not acquired"))
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.result.clone().ok_or(RuntimeError::Invariant(
            "Object iterator result not allocated",
        ))
    }
    fn abrupt(self, value: Value) -> IterationStep {
        let close = matches!(self.kind, IterationKind::Entries)
            || matches!(self.phase, Phase::Callback(_) | Phase::Key(_));
        if close && let Some(iterator) = self.iterator {
            IterationStep::Close {
                iterator,
                completion: Completion::Throw(value),
            }
        } else {
            IterationStep::Complete(Completion::Throw(value))
        }
    }
    fn next_step(mut self, runtime: &Runtime) -> Result<IterationStep, RuntimeError> {
        if !matches!(self.kind, IterationKind::Entries) && self.index >= self.limit {
            return Ok(IterationStep::Close {
                iterator: self.iterator()?,
                completion: Completion::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    "too many elements",
                )?),
            });
        }
        self.phase = Phase::Next;
        Ok(IterationStep::Next {
            iterator: self.iterator()?,
            method: self.next.clone(),
            resume: self,
        })
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<IterationStep, RuntimeError> {
        if !matches!(self.phase, Phase::Next) {
            return Err(RuntimeError::Invariant(
                "Object iterator step has wrong reply phase",
            ));
        }
        let value = match reply {
            ObjectIteratorStep::Throw(value) => return Ok(self.abrupt(value)),
            ObjectIteratorStep::Done => {
                return Ok(IterationStep::Complete(Completion::Return(Value::Object(
                    self.result()?,
                ))));
            }
            ObjectIteratorStep::Yield(value) => value,
        };
        match self.kind {
            IterationKind::Entries => {
                let Value::Object(item) = value else {
                    let value = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return Ok(self.abrupt(value));
                };
                self.phase = Phase::EntryKey(item.clone());
                Ok(IterationStep::Read {
                    receiver: Value::Object(item),
                    key: runtime.intern_property_key("0")?,
                    resume: self,
                })
            }
            IterationKind::Group | IterationKind::MapGroup => {
                let callable = self
                    .callback
                    .clone()
                    .ok_or(RuntimeError::Invariant("groupBy callback missing"))?;
                let arguments = vec![value.clone(), Value::number(self.index as f64)];
                self.phase = Phase::Callback(value);
                Ok(IterationStep::Call {
                    callable,
                    receiver: Value::Object(runtime.global_object_for_realm(self.realm)?),
                    arguments,
                    resume: self,
                })
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
        let phase = std::mem::replace(&mut self.phase, Phase::Next);
        match phase {
            Phase::IteratorMethod(iterable) => {
                let callable = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(IterationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "value is not iterable",
                        )?,
                    )));
                };
                self.phase = Phase::Iterator;
                Ok(IterationStep::Call {
                    callable,
                    receiver: iterable,
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Phase::Iterator => {
                let Value::Object(iterator) = value else {
                    return Ok(IterationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                self.iterator = Some(iterator.clone());
                self.phase = Phase::NextMethod;
                Ok(IterationStep::Read {
                    receiver: Value::Object(iterator),
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                self.next = value;
                match self.kind {
                    IterationKind::Group => self.result = Some(runtime.new_object(None)?),
                    IterationKind::MapGroup => {
                        self.result = Some(runtime.new_map_in_realm(self.realm)?)
                    }
                    IterationKind::Entries => {}
                }
                self.next_step(runtime)
            }
            Phase::EntryKey(item) => {
                self.phase = Phase::EntryValue(value);
                Ok(IterationStep::Read {
                    receiver: Value::Object(item),
                    key: runtime.intern_property_key("1")?,
                    resume: self,
                })
            }
            Phase::EntryValue(key) => {
                self.phase = Phase::Key(value);
                Ok(IterationStep::Key {
                    value: key,
                    resume: self,
                })
            }
            Phase::Callback(item) => {
                if matches!(self.kind, IterationKind::MapGroup) {
                    let key = Runtime::normalized_map_key(value);
                    let groups = self.result()?;
                    let group = match runtime.find_map_record(&groups, &key)? {
                        Some((_, value)) => match runtime.root_raw_value(&value)? {
                            Value::Object(group) => group,
                            _ => {
                                return Err(RuntimeError::Invariant(
                                    "Map.groupBy result contained a non-Array group",
                                ));
                            }
                        },
                        None => {
                            let group = runtime.new_array(self.realm)?;
                            runtime.set_map_record(&groups, key, Value::Object(group.clone()))?;
                            group
                        }
                    };
                    self.phase = Phase::Push;
                    return Ok(IterationStep::Push {
                        object: group,
                        value: item,
                        resume: self,
                    });
                }
                self.phase = Phase::Key(item);
                Ok(IterationStep::Key {
                    value,
                    resume: self,
                })
            }
            Phase::Group(item, key) => {
                let group = match value {
                    Value::Undefined => {
                        let group = runtime.new_array(self.realm)?;
                        self.phase = Phase::GroupDefined(item, group.clone());
                        return Ok(IterationStep::Define {
                            object: self.result()?,
                            key,
                            descriptor: descriptor(Value::Object(group)),
                            resume: self,
                        });
                    }
                    Value::Object(group) => group,
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "Object.groupBy result contained a non-Array group",
                        ));
                    }
                };
                self.phase = Phase::Push;
                Ok(IterationStep::Push {
                    object: group,
                    value: item,
                    resume: self,
                })
            }
            Phase::Push => {
                self.index = self.index.checked_add(1).ok_or(RuntimeError::Invariant(
                    "Object.groupBy index overflowed Uint64",
                ))?;
                self.next_step(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Object iterator consumer received wrong completion reply",
            )),
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
        let key = match runtime.property_key_from_primitive(self.realm, value)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
        };
        let Phase::Key(item) = std::mem::replace(&mut self.phase, Phase::Next) else {
            return Err(RuntimeError::Invariant(
                "Object iterator key has wrong phase",
            ));
        };
        match self.kind {
            IterationKind::Entries => {
                self.phase = Phase::EntryDefined(key.clone());
                Ok(IterationStep::Define {
                    object: self.result()?,
                    key,
                    descriptor: descriptor(item),
                    resume: self,
                })
            }
            IterationKind::Group => {
                self.phase = Phase::Group(item, key.clone());
                Ok(IterationStep::Read {
                    receiver: Value::Object(self.result()?),
                    key,
                    resume: self,
                })
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
            NativeConversion::Throw(value) => return Ok(self.abrupt(value)),
            NativeConversion::Value(result) => result,
        };
        match std::mem::replace(&mut self.phase, Phase::Next) {
            Phase::EntryDefined(key) => {
                if let Some(value) = runtime.finish_define_property_or_throw(
                    self.realm,
                    &key,
                    NativeConversion::Value(result),
                )? {
                    return Ok(self.abrupt(value));
                }
                self.next_step(runtime)
            }
            Phase::GroupDefined(value, group) => {
                if !matches!(result, InternalDefineResult::Defined) {
                    return Err(RuntimeError::Invariant(
                        "fresh Object.groupBy result rejected a group property",
                    ));
                }
                self.phase = Phase::Push;
                Ok(IterationStep::Push {
                    object: group,
                    value,
                    resume: self,
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Object iterator definition has wrong phase",
            )),
        }
    }
}
fn descriptor(value: Value) -> OrdinaryPropertyDescriptor {
    OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(true),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
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
            IterationStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            IterationStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            IterationStep::Next {
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
            IterationStep::Key { value, resume } => {
                let result = runtime.to_primitive(
                    realm,
                    value,
                    crate::engine::vm::ToPrimitiveHint::String,
                )?;
                resume.key(runtime, result)?
            }
            IterationStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            IterationStep::Push {
                object,
                value,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_array_prototype_push(
                    realm,
                    ArrayPushKind::Push,
                    NativeInvocation::Call {
                        this_value: Value::Object(object),
                    },
                    &NativeArguments {
                        actual_arg_count: 1,
                        readable: vec![value],
                    },
                )?,
            )?,
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
            readable: vec![Value::Object(iterable)],
        };
        let IterationStep::Read { resume, .. } = IterationStep::start(
            &runtime,
            context.realm,
            IterationKind::Entries,
            &NativeInvocation::Call {
                this_value: Value::Undefined,
            },
            &arguments,
        )
        .unwrap() else {
            panic!("iterator method read expected")
        };
        let result_id = resume.result.as_ref().unwrap().object_id();
        drop(arguments);
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
