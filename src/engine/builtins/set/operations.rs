//! Proposal Set methods retain size-dependent traversal and pinned close policy.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        iterator::step::{NextStep, finish_next},
        native::SetNativeKind,
        object::ObjectIteratorStep,
    },
    heap::{ContextId, ObjectPayload},
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
        frames::{ActiveCollectionRecord, ActiveCollectionRecordGuard},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum SetOperation {
    Disjoint,
    Subset,
    Superset,
    Intersection,
    Difference,
    SymmetricDifference,
    Union,
}
impl SetOperation {
    pub(crate) fn from_native(kind: SetNativeKind) -> Option<Self> {
        Some(match kind {
            SetNativeKind::IsDisjointFrom => Self::Disjoint,
            SetNativeKind::IsSubsetOf => Self::Subset,
            SetNativeKind::IsSupersetOf => Self::Superset,
            SetNativeKind::Intersection => Self::Intersection,
            SetNativeKind::Difference => Self::Difference,
            SetNativeKind::SymmetricDifference => Self::SymmetricDifference,
            SetNativeKind::Union => Self::Union,
            _ => return None,
        })
    }
}
pub(crate) enum SetStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: SetResume,
    },
    Number {
        value: Value,
        resume: SetResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: SetResume,
    },
    Parse {
        result: Completion,
        resume: SetResume,
    },
}
pub(crate) struct SetResume {
    phase: Phase,
    realm: ContextId,
    kind: SetOperation,
    set: ObjectRef,
    target: Value,
    size: i64,
    has: Option<CallableRef>,
    keys: Option<CallableRef>,
    result: Option<ObjectRef>,
    index: usize,
    iterator: Value,
    next: Value,
}
enum Phase {
    Size,
    Number,
    Has,
    Keys,
    Iterator,
    NextMethod,
    NextCall,
    Parse,
    Probe {
        record: Option<ActiveCollectionRecordGuard>,
        value: Value,
    },
    CloseMethod,
    CloseCall,
}
impl SetStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: SetOperation,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let set = match runtime.set_receiver(realm, invocation.clone(), false)? {
            NativeConversion::Value(set) => set,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let target = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Set method operand argv was not padded",
            ))?;
        if matches!(target, Value::Null | Value::Undefined) {
            let base = if matches!(target, Value::Null) {
                "null"
            } else {
                "undefined"
            };
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    &format!("cannot read property 'size' of {base}"),
                )?,
            )));
        }
        let genuine_size = if let Value::Object(object) = &target {
            if !object.belongs_to(runtime) {
                return Err(RuntimeError::WrongRuntime("set-like object"));
            }
            let state = runtime.0.state.borrow();
            if matches!(
                state.heap.object(object.object_id())?.payload,
                ObjectPayload::Set { .. }
            ) {
                Some(state.heap.set_size(object.object_id())?)
            } else {
                None
            }
        } else {
            None
        };
        let mut resume = SetResume {
            realm,
            kind,
            set,
            target,
            size: 0,
            has: None,
            keys: None,
            result: None,
            index: 0,
            iterator: Value::Undefined,
            next: Value::Undefined,
            phase: Phase::Size,
        };
        if let Some(size) = genuine_size {
            resume.size = i64::try_from(size).map_err(|_| {
                RuntimeError::Invariant("genuine Set size exceeded signed 64-bit range")
            })?;
            resume.phase = Phase::Has;
            return resume.read(runtime, "has");
        }
        resume.read(runtime, "size")
    }
}
impl SetResume {
    fn read(self, runtime: &Runtime, name: &str) -> Result<SetStep, RuntimeError> {
        Ok(SetStep::Read {
            receiver: self.target.clone(),
            key: runtime.intern_property_key(name)?,
            resume: self,
        })
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.result
            .clone()
            .ok_or(RuntimeError::Invariant("Set operation result missing"))
    }
    fn complete(self) -> Result<SetStep, RuntimeError> {
        let value = match self.kind {
            SetOperation::Disjoint | SetOperation::Subset | SetOperation::Superset => {
                Value::Bool(true)
            }
            _ => Value::Object(self.result()?),
        };
        Ok(SetStep::Complete(Completion::Return(value)))
    }
    fn selected(mut self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        if matches!(self.kind, SetOperation::Difference) {
            self.result = Some(runtime.copy_set_in_realm(self.realm, &self.set)?);
        }
        let size = i64::try_from(runtime.set_size_value(&self.set)?).unwrap_or(i64::MAX);
        if matches!(self.kind, SetOperation::Subset) && size > self.size
            || matches!(self.kind, SetOperation::Superset) && size < self.size
        {
            return Ok(SetStep::Complete(Completion::Return(Value::Bool(false))));
        }
        let own = match self.kind {
            SetOperation::Subset => true,
            SetOperation::Disjoint | SetOperation::Intersection | SetOperation::Difference => {
                size <= self.size
            }
            _ => false,
        };
        if own {
            if matches!(self.kind, SetOperation::Intersection) {
                self.result = Some(runtime.new_set_in_realm(self.realm)?);
            }
            return self.probe(runtime);
        }
        if matches!(
            self.kind,
            SetOperation::SymmetricDifference | SetOperation::Union
        ) {
            self.has = None;
        }
        self.phase = Phase::Iterator;
        Ok(SetStep::Call {
            callable: self
                .keys
                .clone()
                .ok_or(RuntimeError::Invariant("Set operation keys missing"))?,
            receiver: self.target.clone(),
            arguments: Vec::new(),
            resume: self,
        })
    }
    fn probe(mut self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        let source = if matches!(self.kind, SetOperation::Difference) {
            self.result()?
        } else {
            self.set.clone()
        };
        let Some((record_index, value)) = runtime.next_live_set_record(&source, &mut self.index)?
        else {
            return self.complete();
        };
        let record = runtime.push_active_collection_record(ActiveCollectionRecord::Set {
            object: source.object_id(),
            index: record_index,
        });
        let arguments = vec![value.clone()];
        self.phase = Phase::Probe {
            record: Some(record),
            value,
        };
        Ok(SetStep::Call {
            callable: self
                .has
                .clone()
                .ok_or(RuntimeError::Invariant("Set operation has missing"))?,
            receiver: self.target.clone(),
            arguments,
            resume: self,
        })
    }
    fn next_step(mut self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        let callable = match &self.next {
            Value::Object(object) => runtime.as_callable(object)?,
            _ => None,
        };
        let Some(callable) = callable else {
            return Ok(SetStep::Complete(Completion::Throw(
                runtime.new_native_error(self.realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        self.phase = Phase::NextCall;
        Ok(SetStep::Call {
            callable,
            receiver: self.iterator.clone(),
            arguments: Vec::new(),
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<SetStep, RuntimeError> {
        if matches!(self.phase, Phase::NextCall) {
            self.phase = Phase::Parse;
            return Ok(SetStep::Parse {
                result: reply,
                resume: self,
            });
        }
        if let Phase::Probe { record, .. } = &mut self.phase
            && let Some(record) = record.take()
        {
            record.finish()?;
        }
        if matches!(self.phase, Phase::CloseCall)
            || matches!(self.phase, Phase::CloseMethod) && matches!(reply, Completion::Throw(_))
        {
            return Ok(SetStep::Complete(Completion::Return(Value::Bool(false))));
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(SetStep::Complete(Completion::Throw(value))),
        };
        match std::mem::replace(&mut self.phase, Phase::Parse) {
            Phase::Size => {
                self.phase = Phase::Number;
                Ok(SetStep::Number {
                    value,
                    resume: self,
                })
            }
            phase @ (Phase::Has | Phase::Keys) => {
                let has = matches!(phase, Phase::Has);
                let name = if has { "has" } else { "keys" };
                if matches!(value, Value::Undefined) {
                    return Ok(SetStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            &format!(".{name} is undefined"),
                        )?,
                    )));
                }
                let callable = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(SetStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            &format!(".{name} is not a function"),
                        )?,
                    )));
                };
                if has {
                    self.has = Some(callable);
                    self.phase = Phase::Keys;
                    self.read(runtime, "keys")
                } else {
                    self.keys = Some(callable);
                    self.selected(runtime)
                }
            }
            Phase::Iterator => {
                if matches!(value, Value::Null | Value::Undefined) {
                    let base = if matches!(value, Value::Null) {
                        "null"
                    } else {
                        "undefined"
                    };
                    return Ok(SetStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            &format!("cannot read property 'next' of {base}"),
                        )?,
                    )));
                }
                self.iterator = value.clone();
                self.phase = Phase::NextMethod;
                Ok(SetStep::Read {
                    receiver: value,
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                self.next = value;
                match self.kind {
                    SetOperation::Intersection => {
                        self.result = Some(runtime.new_set_in_realm(self.realm)?)
                    }
                    SetOperation::SymmetricDifference | SetOperation::Union => {
                        self.result = Some(runtime.copy_set_in_realm(self.realm, &self.set)?)
                    }
                    _ => {}
                }
                self.next_step(runtime)
            }
            Phase::Probe { value: item, .. } => {
                let present = runtime.value_to_boolean(&value)?;
                match self.kind {
                    SetOperation::Disjoint if present => {
                        return Ok(SetStep::Complete(Completion::Return(Value::Bool(false))));
                    }
                    SetOperation::Subset if !present => {
                        return Ok(SetStep::Complete(Completion::Return(Value::Bool(false))));
                    }
                    SetOperation::Intersection if present => {
                        runtime.insert_set_record(&self.result()?, item)?;
                    }
                    SetOperation::Difference if present => {
                        runtime.delete_set_record(&self.result()?, &item)?;
                    }
                    _ => {}
                }
                self.probe(runtime)
            }
            Phase::CloseMethod => {
                let callable = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(SetStep::Complete(Completion::Return(Value::Bool(false))));
                };
                self.phase = Phase::CloseCall;
                Ok(SetStep::Call {
                    callable,
                    receiver: self.iterator.clone(),
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            _ => Err(RuntimeError::Invariant(
                "Set operation completion phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<SetStep, RuntimeError> {
        let size = match reply {
            NativeConversion::Value(size) => size,
            NativeConversion::Throw(value) => {
                return Ok(SetStep::Complete(Completion::Throw(value)));
            }
        };
        if size.is_nan() {
            return Ok(SetStep::Complete(Completion::Throw(
                runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    ".size is not a number",
                )?,
            )));
        }
        let size = if size < i64::MIN as f64 {
            i64::MIN
        } else if size >= 2f64.powi(63) {
            i64::MAX
        } else {
            size as i64
        };
        if size < 0 {
            return Ok(SetStep::Complete(Completion::Throw(
                runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Range,
                    ".size must be positive",
                )?,
            )));
        }
        self.size = size;
        self.phase = Phase::Has;
        self.read(runtime, "has")
    }
    pub(crate) fn parsed(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<SetStep, RuntimeError> {
        let value = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(SetStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Done => return self.complete(),
            ObjectIteratorStep::Yield(value) => value,
        };
        let value = Runtime::normalized_set_key(value);
        match self.kind {
            SetOperation::Disjoint | SetOperation::Superset => {
                let present = runtime.find_set_record(&self.set, &value)?.is_some();
                drop(value);
                if present == matches!(self.kind, SetOperation::Disjoint) {
                    self.phase = Phase::CloseMethod;
                    return Ok(SetStep::Read {
                        receiver: self.iterator.clone(),
                        key: runtime.intern_property_key("return")?,
                        resume: self,
                    });
                }
            }
            SetOperation::Intersection => {
                if runtime.find_set_record(&self.set, &value)?.is_some() {
                    runtime.insert_set_record(&self.result()?, value)?;
                }
            }
            SetOperation::Difference => {
                runtime.delete_set_record(&self.result()?, &value)?;
            }
            SetOperation::SymmetricDifference => {
                if runtime.find_set_record(&self.set, &value)?.is_some() {
                    runtime.delete_set_record(&self.result()?, &value)?;
                } else {
                    runtime.insert_set_record(&self.result()?, value)?;
                }
            }
            SetOperation::Union => {
                runtime.insert_set_record(&self.result()?, value)?;
            }
            SetOperation::Subset => {
                return Err(RuntimeError::Invariant("Set subset used foreign iterator"));
            }
        }
        self.next_step(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: SetStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            SetStep::Complete(result) => return Ok(result),
            SetStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            SetStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            SetStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            SetStep::Parse { result, resume } => resume.parsed(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::parse_result(runtime, realm, result)?,
                )?,
            )?,
        };
    }
}
