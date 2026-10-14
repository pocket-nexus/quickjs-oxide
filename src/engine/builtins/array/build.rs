//! Array.from/of own constructor, iterator and definition replies in source order.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    atom::pinned::PinnedAtom,
    builtins::{
        effect::{self, Continuation, Effect, EffectStep, Reply},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{
        CallableRef, ObjectRef, OwnedPropertyDescriptor, PropertyKey, WellKnownSymbol,
        operations::{InternalDefineResult, InternalSetResult, PropertyDefineOutcome},
    },
    value::{JsValue, Value, conversion::NativeConversion, number::operations::Number},
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
/// The boxed state of one in-flight `Array.from`/`Array.of`.
pub(crate) struct Build {
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
impl Drop for Build {
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
        let mut build = Box::new(Build {
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
        });
        build.constructor = runtime.dup_jsvalue(this_value)?;
        let Mode::Of { values, .. } = &mut build.mode else {
            unreachable!()
        };
        values
            .try_reserve_exact(arguments.actual_arg_count)
            .map_err(|_| RuntimeError::Invariant("Array.of argv allocation failed"))?;
        for value in &arguments.readable[..arguments.actual_arg_count] {
            values.push(runtime.dup_jsvalue(value)?);
        }
        return build.construct(runtime, Some(f64::from(length)), false);
    }
    let items = arguments
        .readable
        .first()
        .ok_or(RuntimeError::Invariant("Array.from argv was not padded"))?;
    let mapfn = if arguments.actual_arg_count > 1
        && !matches!(arguments.readable[1], JsValue::Undefined)
    {
        let Some(callable) = runtime.as_callable_jsvalue(&arguments.readable[1])? else {
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
    let mut build = Box::new(Build {
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
    });
    build.constructor = runtime.dup_jsvalue(this_value)?;
    if arguments.actual_arg_count > 2 {
        build.map_this = runtime.dup_jsvalue(&arguments.readable[2])?;
    }
    build.mode = Mode::Acquire(runtime.dup_jsvalue(items)?);
    effect::request(
        Effect::ReadValue {
            receiver: runtime.dup_jsvalue(items)?,
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)?),
        },
        build,
    )
}
impl Build {
    fn abrupt(
        mut self: Box<Self>,
        _runtime: &Runtime,
        value: JsValue,
    ) -> Result<EffectStep, RuntimeError> {
        let completion = Completion::Throw(value);
        if matches!(self.phase, Phase::Map | Phase::Define)
            && let Mode::Iterable { iterator, .. } = &mut self.mode
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
        self.result
            .as_ref()
            .map(|value| value.try_clone())
            .transpose()?
            .ok_or(RuntimeError::Invariant("Array builder result missing"))
    }
    fn construct(
        mut self: Box<Self>,
        runtime: &Runtime,
        length: Option<f64>,
        initialize_length: bool,
    ) -> Result<EffectStep, RuntimeError> {
        self.phase = Phase::Construct;
        let is_constructor = match &self.constructor {
            JsValue::Object(id) => {
                runtime.is_constructor(&ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)?
            }
            _ => false,
        };
        if is_constructor {
            let constructor = std::mem::replace(&mut self.constructor, JsValue::Undefined);
            let target = match runtime.constructor_from_jsvalue(self.realm, constructor)? {
                NativeConversion::Value(target) => target,
                NativeConversion::Throw(value) => return self.abrupt(runtime, value),
            };
            return effect::request(
                Effect::Construct {
                    target,
                    arguments: length
                        .into_iter()
                        .map(|value| Number::compact(value).into())
                        .collect(),
                },
                self,
            );
        }
        // Numeric length and a new unpublished Array cannot invoke JS. Share
        // the same fallback allocation as ArraySpeciesCreate.
        let result = runtime
            .new_array_with_length(self.realm, if initialize_length { length } else { None })?;
        self.resume(runtime, result)
    }
    fn next(mut self: Box<Self>, runtime: &Runtime) -> Result<EffectStep, RuntimeError> {
        if matches!(self.mode, Mode::Of { .. }) {
            loop {
                let Mode::Of { values, .. } = &mut self.mode else {
                    unreachable!()
                };
                let Some(value) = values.get_mut(self.index as usize) else {
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
                if let Some(error) =
                    runtime.finish_create_indexed_data_property(self.realm, self.index, reply)?
                {
                    return self.abrupt(runtime, error);
                }
                self.index += 1;
            }
        }
        match &mut self.mode {
            Mode::Iterable {
                iterator: Some(iterator),
                next: Some(next),
                ..
            } => {
                let callable = next.try_clone()?;
                let receiver = JsValue::Object(iterator.try_clone()?.into_handle());
                self.phase = Phase::NextCall;
                // Array.from uses ordinary Call, not JS_IteratorNext2's raw
                // cproto fast path; parse its actual object result afterwards.
                effect::request(
                    Effect::Call {
                        callable,
                        receiver,
                        arguments: Vec::new(),
                    },
                    self,
                )
            }
            Mode::ArrayLike { source, length } if self.index < *length => {
                let receiver = JsValue::Object(source.try_clone()?.into_handle());
                self.phase = Phase::Value;
                effect::request(
                    Effect::ReadValue {
                        receiver,
                        key: runtime.property_key_for_index(self.index)?,
                    },
                    self,
                )
            }
            Mode::ArrayLike { .. } => self.set_length(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array builder iteration not initialized",
            )),
        }
    }
    fn set_length(mut self: Box<Self>, runtime: &Runtime) -> Result<EffectStep, RuntimeError> {
        let length = match self.mode {
            Mode::Of { length, .. } => length as u64,
            Mode::ArrayLike { length, .. } => length,
            _ => self.index,
        };
        self.phase = Phase::LengthSet;
        effect::request(
            Effect::Set {
                object: self.result()?,
                key: runtime.pinned_property_key(PinnedAtom::Length)?,
                value: runtime.into_jsvalue(Value::number(length as f64))?,
            },
            self,
        )
    }
    fn map(
        mut self: Box<Self>,
        runtime: &Runtime,
        value: JsValue,
    ) -> Result<EffectStep, RuntimeError> {
        if let Some(callable) = self
            .mapfn
            .as_ref()
            .map(|value| value.try_clone())
            .transpose()?
        {
            self.phase = Phase::Map;
            let receiver = match runtime.dup_jsvalue(&self.map_this) {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = runtime.release_jsvalue(value);
                    return Err(error);
                }
            };
            let index = Number::compact(self.index as f64).into();
            return effect::request(
                Effect::Call {
                    callable,
                    receiver,
                    arguments: vec![value, index],
                },
                self,
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
        let key = runtime.property_key_for_index(self.index)?;
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
    fn define(
        mut self: Box<Self>,
        runtime: &Runtime,
        value: JsValue,
    ) -> Result<EffectStep, RuntimeError> {
        self.phase = Phase::Define;
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
            Ok::<_, RuntimeError>((self.result()?, runtime.property_key_for_index(self.index)?))
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
                descriptor: Box::new(OwnedPropertyDescriptor::data(runtime, value)),
            },
            self,
        )
    }
    fn resume(
        mut self: Box<Self>,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<EffectStep, RuntimeError> {
        if matches!(self.phase, Phase::NextCall) {
            self.phase = Phase::Parse;
            return effect::request(Effect::ParseIterator { result: reply }, self);
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return self.abrupt(runtime, value),
        };
        match self.phase {
            Phase::Value => self.map(runtime, value),
            Phase::Map => self.define(runtime, value),
            Phase::Method => {
                let Mode::Acquire(items) =
                    std::mem::replace(&mut self.mode, Mode::Acquire(JsValue::Undefined))
                else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant("Array.from items missing"));
                };
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    let source = match runtime.native_to_object_jsvalue(self.realm, items)? {
                        NativeConversion::Value(source) => source,
                        NativeConversion::Throw(value) => return self.abrupt(runtime, value),
                    };
                    self.mode = Mode::ArrayLike {
                        source: source.try_clone()?,
                        length: 0,
                    };
                    self.phase = Phase::Length;
                    return effect::request(
                        Effect::ReadValue {
                            receiver: JsValue::Object(source.into_handle()),
                            key: runtime.pinned_property_key(PinnedAtom::Length)?,
                        },
                        self,
                    );
                }
                let callable = runtime.as_callable_jsvalue(&value);
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
                        self.realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    return self.abrupt(runtime, error);
                };
                self.mode = Mode::Iterable {
                    items,
                    method,
                    iterator: None,
                    next: None,
                };
                self.construct(runtime, None, false)
            }
            Phase::Length => {
                self.phase = Phase::Number;
                effect::request(Effect::Number { value }, self)
            }
            Phase::Construct => {
                let JsValue::Object(result) = value else {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "Array result constructor returned a primitive",
                    ));
                };
                self.result = Some(ObjectRef::from_owned_handle(runtime.clone(), result));
                if let Mode::Iterable { items, method, .. } = &self.mode {
                    let receiver = runtime.dup_jsvalue(items)?;
                    let callable = method.try_clone()?;
                    self.phase = Phase::Iterator;
                    return effect::request(
                        Effect::Call {
                            callable,
                            receiver,
                            arguments: Vec::new(),
                        },
                        self,
                    );
                }
                self.next(runtime)
            }
            Phase::Iterator => {
                let JsValue::Object(iterator) = value else {
                    runtime.release_jsvalue(value)?;
                    let error = runtime.new_native_error_jsvalue(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.abrupt(runtime, error);
                };
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), iterator);
                let Mode::Iterable {
                    iterator: target, ..
                } = &mut self.mode
                else {
                    return Err(RuntimeError::Invariant("Array.from iterator mode missing"));
                };
                *target = Some(iterator.try_clone()?);
                self.phase = Phase::NextMethod;
                effect::request(
                    Effect::ReadValue {
                        receiver: JsValue::Object(iterator.into_handle()),
                        key: runtime.pinned_property_key(PinnedAtom::Next)?,
                    },
                    self,
                )
            }
            Phase::NextMethod => {
                let callable = runtime.as_callable_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                let callable = callable?;
                let Some(callable) = callable else {
                    let error = runtime.new_native_error_jsvalue(
                        self.realm,
                        NativeErrorKind::Type,
                        "not a function",
                    )?;
                    return self.abrupt(runtime, error);
                };
                let Mode::Iterable { next, .. } = &mut self.mode else {
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
    fn number(
        mut self: Box<Self>,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<EffectStep, RuntimeError> {
        let number = match reply {
            NativeConversion::Value(number) => number,
            NativeConversion::Throw(value) => return self.abrupt(runtime, value),
        };
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array builder number phase mismatch",
            ));
        }
        let Mode::ArrayLike { length, .. } = &mut self.mode else {
            return Err(RuntimeError::Invariant(
                "Array builder length mode mismatch",
            ));
        };
        *length = Runtime::length_from_number(number);
        let value = *length as f64;
        self.construct(runtime, Some(value), true)
    }
    fn parsed(
        self: Box<Self>,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::Parse) {
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
    fn defined(
        mut self: Box<Self>,
        runtime: &Runtime,
        reply: NativeConversion<InternalDefineResult>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            if let NativeConversion::Throw(value) = reply {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array builder definition phase mismatch",
            ));
        }
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, self.index, reply)?
        {
            return self.abrupt(runtime, value);
        }
        self.index = self.index.checked_add(1).ok_or(RuntimeError::Invariant(
            "Array.from iterator index overflowed u64",
        ))?;
        self.next(runtime)
    }
    fn set(
        self: Box<Self>,
        runtime: &Runtime,
        key: PropertyKey,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::LengthSet) {
            if let NativeConversion::Throw(value) = reply {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array builder length-set phase mismatch",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, reply)? {
            return self.abrupt(runtime, value);
        }
        Ok(EffectStep::Complete(Completion::Return(JsValue::Object(
            self.result()?.into_handle(),
        ))))
    }
}

impl Continuation for Build {
    fn pending(&mut self) -> &mut Option<Effect> {
        &mut self.pending
    }
    fn reply(self: Box<Self>, runtime: &Runtime, reply: Reply) -> Result<EffectStep, RuntimeError> {
        match reply {
            Reply::Completion(completion) => self.resume(runtime, completion),
            Reply::Number(number) => self.number(runtime, number),
            Reply::Parsed(step) => self.parsed(runtime, step),
            Reply::Defined(result) => self.defined(runtime, result),
            Reply::Set { key, result } => self.set(runtime, key, result),
            Reply::Boolean(_) => Err(RuntimeError::Invariant(
                "Array builder received an unexpected reply",
            )),
        }
    }
}
