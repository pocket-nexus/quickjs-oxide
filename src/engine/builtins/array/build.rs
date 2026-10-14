//! Array.from/of own constructor, iterator and definition replies in source order.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        effect::{self, Continuation, Effect, EffectStep, Reply},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{
        CallableRef, ObjectRef, OwnedPropertyDescriptor, PropertyKey, WellKnownSymbol,
        operations::{InternalDefineResult, InternalSetResult, PropertyDefineOutcome},
    },
    value::{JsValue, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum BuildKind {
    From,
    Of,
}
pub(crate) struct BuildResume(Box<BuildResumeState>);
impl std::ops::Deref for BuildResume {
    type Target = BuildResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for BuildResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<BuildResume>() <= 8);
pub(crate) struct BuildResumeState {
    runtime: Runtime,
    pending: Option<Effect>,
    realm: ContextId,
    constructor: JsValue,
    result: Option<ObjectRef>,
    mapfn: Option<CallableRef>,
    map_this: JsValue,
    mode: Mode,
    index: u64,
    phase: Phase,
}
enum Mode {
    Acquire(JsValue),
    Iterable {
        items: JsValue,
        method: CallableRef,
        iterator: Option<ObjectRef>,
        next: Option<CallableRef>,
    },
    ArrayLike {
        source: ObjectRef,
        length: u64,
    },
    Of {
        values: Vec<JsValue>,
        length: u32,
    },
}
enum Phase {
    Method,
    Length,
    Number,
    Construct,
    Iterator,
    NextMethod,
    NextCall,
    Parse,
    Value,
    Map,
    Define,
    LengthSet,
}
impl Drop for BuildResumeState {
    fn drop(&mut self) {
        let constructor = std::mem::replace(&mut self.constructor, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(constructor);
        let receiver = std::mem::replace(&mut self.map_this, JsValue::Undefined);
        let _ = self.runtime.release_jsvalue(receiver);
        match &mut self.mode {
            Mode::Acquire(value) | Mode::Iterable { items: value, .. } => {
                let value = std::mem::replace(value, JsValue::Undefined);
                let _ = self.runtime.release_jsvalue(value);
            }
            Mode::Of { values, .. } => {
                for value in values.drain(..) {
                    let _ = self.runtime.release_jsvalue(value);
                }
            }
            Mode::ArrayLike { .. } => {}
        }
        if let Some(effect) = self.pending.take() {
            effect.release(&self.runtime);
        }
    }
}
pub(crate) fn start(
    runtime: &Runtime,
    realm: ContextId,
    kind: BuildKind,
    invocation: &NativeInvocation,
    arguments: &NativeArguments,
) -> Result<EffectStep, RuntimeError> {
    let NativeInvocation::Call { this_value } = invocation else {
        return Err(RuntimeError::Invariant(
            "Array builder did not receive a generic invocation",
        ));
    };
    if matches!(kind, BuildKind::Of) {
        let length = u32::try_from(arguments.actual_arg_count)
            .map_err(|_| RuntimeError::Invariant("Array.of argument count exceeded Uint32"))?;
        let mut resume = BuildResume(Box::new(BuildResumeState {
            runtime: runtime.clone(),
            pending: None,
            realm,
            constructor: JsValue::Undefined,
            result: None,
            mapfn: None,
            map_this: JsValue::Undefined,
            mode: Mode::Of {
                values: Vec::new(),
                length,
            },
            index: 0,
            phase: Phase::Construct,
        }));
        resume.0.constructor = runtime.dup_jsvalue(this_value)?;
        let Mode::Of { values, .. } = &mut resume.0.mode else {
            unreachable!()
        };
        values
            .try_reserve_exact(arguments.actual_arg_count)
            .map_err(|_| RuntimeError::Invariant("Array.of argv allocation failed"))?;
        for value in &arguments.readable[..arguments.actual_arg_count] {
            values.push(runtime.dup_jsvalue(value)?);
        }
        return resume.construct(runtime, Some(f64::from(length)), false);
    }
    let items = arguments
        .readable
        .first()
        .ok_or(RuntimeError::Invariant("Array.from argv was not padded"))?;
    let mapfn = if arguments.actual_arg_count > 1
        && !matches!(arguments.readable[1], JsValue::Undefined)
    {
        let callable = match &arguments.readable[1] {
            JsValue::Object(id) => {
                runtime.as_callable(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)?
            }
            _ => None,
        };
        let Some(callable) = callable else {
            return Ok(EffectStep::Complete(Completion::Throw(
                runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        Some(callable)
    } else {
        None
    };
    if matches!(items, JsValue::Null | JsValue::Undefined) {
        let base = if matches!(items, JsValue::Null) {
            "null"
        } else {
            "undefined"
        };
        return Ok(EffectStep::Complete(Completion::Throw(
            runtime.new_native_error_jsvalue(
                realm,
                NativeErrorKind::Type,
                &format!("cannot read property 'Symbol.iterator' of {base}"),
            )?,
        )));
    }
    let mut resume = BuildResume(Box::new(BuildResumeState {
        runtime: runtime.clone(),
        pending: None,
        realm,
        constructor: JsValue::Undefined,
        result: None,
        mapfn,
        map_this: JsValue::Undefined,
        mode: Mode::Acquire(JsValue::Undefined),
        index: 0,
        phase: Phase::Method,
    }));
    resume.0.constructor = runtime.dup_jsvalue(this_value)?;
    if arguments.actual_arg_count > 2 {
        resume.0.map_this = runtime.dup_jsvalue(&arguments.readable[2])?;
    }
    resume.0.mode = Mode::Acquire(runtime.dup_jsvalue(items)?);
    effect::request(
        Effect::ReadValue {
            receiver: runtime.dup_jsvalue(items)?,
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)?),
        },
        resume.0,
    )
}
impl BuildResume {
    fn abrupt(mut self, _runtime: &Runtime, value: JsValue) -> Result<EffectStep, RuntimeError> {
        let completion = Completion::Throw(value);
        if matches!(self.0.phase, Phase::Map | Phase::Define)
            && let Mode::Iterable { iterator, .. } = &mut self.0.mode
            && let Some(iterator) = iterator.take()
        {
            return Ok(EffectStep::Close {
                iterator,
                completion,
            });
        }
        Ok(EffectStep::Complete(completion))
    }
    fn result(&self) -> Result<ObjectRef, RuntimeError> {
        self.0
            .result
            .as_ref()
            .map(|value| value.try_clone())
            .transpose()?
            .ok_or(RuntimeError::Invariant("Array builder result missing"))
    }
    fn construct(
        mut self,
        runtime: &Runtime,
        length: Option<f64>,
        initialize_length: bool,
    ) -> Result<EffectStep, RuntimeError> {
        self.0.phase = Phase::Construct;
        let is_constructor = match &self.0.constructor {
            JsValue::Object(id) => {
                runtime.is_constructor(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)?
            }
            _ => false,
        };
        if is_constructor {
            let constructor = std::mem::replace(&mut self.0.constructor, JsValue::Undefined);
            let target = match runtime.constructor_from_jsvalue(self.0.realm, constructor)? {
                NativeConversion::Value(target) => target,
                NativeConversion::Throw(value) => return self.abrupt(runtime, value),
            };
            return effect::request(
                Effect::Construct {
                    target,
                    arguments: length
                        .into_iter()
                        .map(|value| {
                            crate::engine::value::number::operations::Number::compact(value).into()
                        })
                        .collect(),
                },
                self.0,
            );
        }
        // Numeric length and a new unpublished Array cannot invoke JS. Share
        // the same fallback allocation as ArraySpeciesCreate.
        let result = runtime
            .new_array_with_length(self.0.realm, if initialize_length { length } else { None })?;
        self.resume(runtime, result)
    }
    fn next(mut self, runtime: &Runtime) -> Result<EffectStep, RuntimeError> {
        if matches!(self.0.mode, Mode::Of { .. }) {
            loop {
                let Mode::Of { values, .. } = &mut self.0.mode else {
                    unreachable!()
                };
                let Some(value) = values.get_mut(self.0.index as usize) else {
                    return self.set_length(runtime);
                };
                let value = std::mem::replace(value, JsValue::Undefined);
                let defined = self.define_array(runtime, &value);
                let reply = match defined {
                    Ok(Some(reply)) => {
                        runtime.release_jsvalue(value)?;
                        reply
                    }
                    Ok(None) => return self.define(runtime, value),
                    Err(error) => {
                        let _ = runtime.release_jsvalue(value);
                        return Err(error);
                    }
                };
                if let Some(error) = runtime.finish_create_indexed_data_property(
                    self.0.realm,
                    self.0.index,
                    reply,
                )? {
                    return self.abrupt(runtime, error);
                }
                self.0.index += 1;
            }
        }
        match &mut self.0.mode {
            Mode::Iterable {
                iterator: Some(iterator),
                next: Some(next),
                ..
            } => {
                let callable = next.try_clone()?;
                let receiver = JsValue::Object(iterator.try_clone()?.into_handle());
                self.0.phase = Phase::NextCall;
                // Array.from uses ordinary Call, not JS_IteratorNext2's raw
                // cproto fast path; parse its actual object result afterwards.
                effect::request(
                    Effect::Call {
                        callable,
                        receiver,
                        arguments: Vec::new(),
                    },
                    self.0,
                )
            }
            Mode::ArrayLike { source, length } if self.0.index < *length => {
                let receiver = JsValue::Object(source.try_clone()?.into_handle());
                self.0.phase = Phase::Value;
                effect::request(
                    Effect::ReadValue {
                        receiver,
                        key: runtime.property_key_for_index(self.0.index)?,
                    },
                    self.0,
                )
            }
            Mode::ArrayLike { .. } => self.set_length(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array builder iteration not initialized",
            )),
        }
    }
    fn set_length(mut self, runtime: &Runtime) -> Result<EffectStep, RuntimeError> {
        let length = match self.0.mode {
            Mode::Of { length, .. } => length as u64,
            Mode::ArrayLike { length, .. } => length,
            _ => self.0.index,
        };
        self.0.phase = Phase::LengthSet;
        effect::request(
            Effect::Set {
                object: self.result()?,
                key: runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?,
                value: runtime.into_jsvalue(Value::number(length as f64))?,
            },
            self.0,
        )
    }
    fn map(mut self, runtime: &Runtime, value: JsValue) -> Result<EffectStep, RuntimeError> {
        if let Some(callable) = self
            .0
            .mapfn
            .as_ref()
            .map(|value| value.try_clone())
            .transpose()?
        {
            self.0.phase = Phase::Map;
            let receiver = match runtime.dup_jsvalue(&self.0.map_this) {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = runtime.release_jsvalue(value);
                    return Err(error);
                }
            };
            let index =
                crate::engine::value::number::operations::Number::compact(self.0.index as f64)
                    .into();
            return effect::request(
                Effect::Call {
                    callable,
                    receiver,
                    arguments: vec![value, index],
                },
                self.0,
            );
        }
        self.define(runtime, value)
    }
    fn define_array(
        &self,
        runtime: &Runtime,
        value: &JsValue,
    ) -> Result<Option<NativeConversion<InternalDefineResult>>, RuntimeError> {
        let result = self.result()?;
        let genuine_array = {
            let state = runtime.0.state.borrow();
            matches!(
                state.heap.object(result.object_id())?.payload,
                crate::engine::heap::ObjectPayload::Array { .. }
            )
        };
        if !genuine_array {
            return Ok(None);
        }
        let key = runtime.property_key_for_index(self.0.index)?;
        Ok(Some(
            match runtime.define_selected_set_data(&result, &key, value, false)? {
                PropertyDefineOutcome::Defined(true) => {
                    NativeConversion::Value(InternalDefineResult::Defined)
                }
                PropertyDefineOutcome::Defined(false) => {
                    NativeConversion::Value(InternalDefineResult::RejectedOrdinary(result))
                }
                PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
            },
        ))
    }
    fn define(mut self, runtime: &Runtime, value: JsValue) -> Result<EffectStep, RuntimeError> {
        self.0.phase = Phase::Define;
        let defined = self.define_array(runtime, &value);
        match defined {
            Ok(Some(reply)) => {
                runtime.release_jsvalue(value)?;
                return self.defined(runtime, reply);
            }
            Err(error) => {
                let _ = runtime.release_jsvalue(value);
                return Err(error);
            }
            Ok(None) => {}
        }
        let target = (|| {
            Ok::<_, RuntimeError>((
                self.result()?,
                runtime.property_key_for_index(self.0.index)?,
            ))
        })();
        let (result, key) = match target {
            Ok(target) => target,
            Err(error) => {
                let _ = runtime.release_jsvalue(value);
                return Err(error);
            }
        };
        effect::request(
            Effect::Define {
                object: result,
                key,
                descriptor: OwnedPropertyDescriptor::data(runtime, value),
            },
            self.0,
        )
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<EffectStep, RuntimeError> {
        if matches!(self.0.phase, Phase::NextCall) {
            self.0.phase = Phase::Parse;
            return effect::request(Effect::ParseIterator { result: reply }, self.0);
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return self.abrupt(runtime, value);
            }
        };
        if matches!(self.0.phase, Phase::Value) {
            return self.map(runtime, value);
        }
        if matches!(self.0.phase, Phase::Map) {
            return self.define(runtime, value);
        }
        match self.0.phase {
            Phase::Method => {
                let Mode::Acquire(items) =
                    std::mem::replace(&mut self.0.mode, Mode::Acquire(JsValue::Undefined))
                else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant("Array.from items missing"));
                };
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    let source = match runtime.native_to_object_jsvalue(self.0.realm, items)? {
                        NativeConversion::Value(source) => source,
                        NativeConversion::Throw(value) => return self.abrupt(runtime, value),
                    };
                    self.0.mode = Mode::ArrayLike {
                        source: source.try_clone()?,
                        length: 0,
                    };
                    self.0.phase = Phase::Length;
                    return effect::request(
                        Effect::ReadValue {
                            receiver: JsValue::Object(source.into_handle()),
                            key: runtime.pinned_property_key(
                                crate::engine::atom::pinned::PinnedAtom::Length,
                            )?,
                        },
                        self.0,
                    );
                }
                let callable = (|| match &value {
                    JsValue::Object(id) => {
                        runtime.as_callable(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)
                    }
                    _ => Ok(None),
                })();
                runtime.release_jsvalue(value)?;
                let callable = match callable {
                    Ok(callable) => callable,
                    Err(error) => {
                        let _ = runtime.release_jsvalue(items);
                        return Err(error);
                    }
                };
                let Some(method) = callable else {
                    runtime.release_jsvalue(items)?;
                    let error = runtime.new_native_error_jsvalue(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    return self.abrupt(runtime, error);
                };
                self.0.mode = Mode::Iterable {
                    items,
                    method,
                    iterator: None,
                    next: None,
                };
                self.construct(runtime, None, false)
            }
            Phase::Length => {
                self.0.phase = Phase::Number;
                effect::request(Effect::Number { value }, self.0)
            }
            Phase::Construct => {
                let JsValue::Object(result) = value else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "Array result constructor returned a primitive",
                    ));
                };
                self.0.result = Some(ObjectRef::from_owned_handle(runtime.clone(), result));
                if let Mode::Iterable { items, method, .. } = &self.0.mode {
                    let receiver = runtime.dup_jsvalue(items)?;
                    let callable = method.try_clone()?;
                    self.0.phase = Phase::Iterator;
                    return effect::request(
                        Effect::Call {
                            callable,
                            receiver,
                            arguments: Vec::new(),
                        },
                        self.0,
                    );
                }
                self.next(runtime)
            }
            Phase::Iterator => {
                let JsValue::Object(iterator) = value else {
                    runtime.release_jsvalue(value)?;
                    let error = runtime.new_native_error_jsvalue(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.abrupt(runtime, error);
                };
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), iterator);
                let Mode::Iterable {
                    iterator: target, ..
                } = &mut self.0.mode
                else {
                    return Err(RuntimeError::Invariant("Array.from iterator mode missing"));
                };
                *target = Some(iterator.try_clone()?);
                self.0.phase = Phase::NextMethod;
                effect::request(
                    Effect::ReadValue {
                        receiver: JsValue::Object(iterator.into_handle()),
                        key: runtime
                            .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?,
                    },
                    self.0,
                )
            }
            Phase::NextMethod => {
                let callable = (|| match &value {
                    JsValue::Object(id) => {
                        runtime.as_callable(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)
                    }
                    _ => Ok(None),
                })();
                runtime.release_jsvalue(value)?;
                let callable = callable?;
                let Some(callable) = callable else {
                    let error = runtime.new_native_error_jsvalue(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "not a function",
                    )?;
                    return self.abrupt(runtime, error);
                };
                let Mode::Iterable { next, .. } = &mut self.0.mode else {
                    return Err(RuntimeError::Invariant("Array.from iterator mode missing"));
                };
                *next = Some(callable);
                self.next(runtime)
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "Array builder completion phase mismatch",
                ))
            }
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<EffectStep, RuntimeError> {
        let number = match reply {
            NativeConversion::Value(number) => number,
            NativeConversion::Throw(value) => return self.abrupt(runtime, value),
        };
        if !matches!(self.0.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array builder number phase mismatch",
            ));
        }
        let Mode::ArrayLike { length, .. } = &mut self.0.mode else {
            return Err(RuntimeError::Invariant(
                "Array builder length mode mismatch",
            ));
        };
        *length = Runtime::length_from_number(number);
        let value = *length as f64;
        self.construct(runtime, Some(value), true)
    }
    pub(crate) fn parsed(
        self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Parse) {
            return Err(RuntimeError::Invariant(
                "Array builder next result phase mismatch",
            ));
        }
        match reply {
            ObjectIteratorStep::Throw(value) => self.abrupt(runtime, value),
            ObjectIteratorStep::Done => self.set_length(runtime),
            ObjectIteratorStep::Yield(value) => self.map(runtime, value),
        }
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<InternalDefineResult>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Define) {
            if let NativeConversion::Throw(value) = reply {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array builder definition phase mismatch",
            ));
        }
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.0.realm, self.0.index, reply)?
        {
            return self.abrupt(runtime, value);
        }
        self.0.index = self.0.index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Array.from iterator index overflowed u64",
        ))?;
        self.next(runtime)
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::LengthSet) {
            if let NativeConversion::Throw(value) = reply {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array builder length-set phase mismatch",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.0.realm, &key, reply)? {
            return self.abrupt(runtime, value);
        }
        Ok(EffectStep::Complete(Completion::Return(JsValue::Object(
            self.result()?.into_handle(),
        ))))
    }
}

impl Continuation for BuildResumeState {
    fn pending(&mut self) -> &mut Option<Effect> {
        &mut self.pending
    }
    fn reply(self: Box<Self>, runtime: &Runtime, reply: Reply) -> Result<EffectStep, RuntimeError> {
        let resume = BuildResume(self);
        match reply {
            Reply::Completion(completion) => resume.resume(runtime, completion),
            Reply::Number(number) => resume.number(runtime, number),
            Reply::Parsed(step) => resume.parsed(runtime, step),
            Reply::Defined(result) => resume.defined(runtime, result),
            Reply::Set { key, result } => resume.set(runtime, key, result),
        }
    }
}
