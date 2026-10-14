//! Array callback loops retain the captured length and reread each observable property.

use crate::engine::builtins::native::NativeFunctionId;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    atom::pinned::PinnedAtom,
    builtins::{
        effect::{self, Continuation, Effect, EffectStep, Reply},
        native::{ArrayFindKind, ArrayIterationKind, ArrayReduceKind},
    },
    heap::ContextId,
    object::{
        CallableRef, DescriptorField, ObjectRef, OwnedPropertyDescriptor,
        operations::InternalDefineResult,
    },
    value::{JsValue, conversion::NativeConversion, number::operations::Number},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum CallbackKind {
    Iteration(ArrayIterationKind),
    Reduce(ArrayReduceKind),
    Find(ArrayFindKind),
}
impl CallbackKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypeIteration(kind) => Some(Self::Iteration(kind)),
            NativeFunctionId::ArrayPrototypeReduce(kind) => Some(Self::Reduce(kind)),
            NativeFunctionId::ArrayPrototypeFind(kind) => Some(Self::Find(kind)),
            _ => None,
        }
    }
}
enum Phase {
    Length,
    Number,
    Species,
    Has,
    Read,
    Callback,
    Define,
}
/// The boxed state of one in-flight Array callback loop.
pub(crate) struct Callback {
    runtime: Runtime,
    pending: Option<Effect>,
    realm: ContextId,
    kind: CallbackKind,
    object: ObjectRef,
    original: JsValue,
    callback_value: JsValue,
    callback: Option<CallableRef>,
    this_arg: JsValue,
    accumulator: Option<JsValue>,
    result: JsValue,
    value: JsValue,
    phase: Phase,
    length: u64,
    cursor: u64,
    selected: u64,
}
impl Drop for Callback {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        for value in [
            std::mem::replace(&mut self.original, JsValue::Undefined),
            std::mem::replace(&mut self.callback_value, JsValue::Undefined),
            std::mem::replace(&mut self.this_arg, JsValue::Undefined),
            std::mem::replace(&mut self.result, JsValue::Undefined),
            std::mem::replace(&mut self.value, JsValue::Undefined),
            self.accumulator.take().unwrap_or(JsValue::Undefined),
        ] {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(effect) = self.pending.take() {
            effect.release(&self.runtime);
        }
    }
}
pub(crate) fn start(
    runtime: &Runtime,
    realm: ContextId,
    kind: CallbackKind,
    invocation: &NativeInvocation,
    arguments: &NativeArguments,
) -> Result<EffectStep, RuntimeError> {
    let NativeInvocation::Call { this_value } = invocation else {
        return Err(RuntimeError::Invariant(
            "Array callback requires generic invocation",
        ));
    };
    let object = match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(this_value)?)? {
        NativeConversion::Value(object) => object,
        NativeConversion::Throw(value) => {
            return Ok(EffectStep::Complete(Completion::Throw(value)));
        }
    };
    let mut resume = Box::new(Callback {
        runtime: runtime.clone(),
        pending: None,
        realm,
        kind,
        object,
        original: JsValue::Undefined,
        callback_value: JsValue::Undefined,
        callback: None,
        this_arg: JsValue::Undefined,
        accumulator: None,
        result: JsValue::Undefined,
        value: JsValue::Undefined,
        phase: Phase::Length,
        length: 0,
        cursor: 0,
        selected: 0,
    });
    resume.original = runtime.dup_jsvalue(this_value)?;
    resume.callback_value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
        RuntimeError::Invariant("Array callback argv was not padded"),
    )?)?;
    if arguments.actual_arg_count > 1 {
        let second = arguments.readable.get(1).ok_or(RuntimeError::Invariant(
            "Array callback second argument missing",
        ))?;
        resume.this_arg = runtime.dup_jsvalue(second)?;
        if matches!(kind, CallbackKind::Reduce(_)) {
            resume.accumulator = Some(runtime.dup_jsvalue(second)?);
        }
    }
    let effect = Effect::Read {
        object: resume.object.try_clone()?,
        key: runtime.pinned_property_key(PinnedAtom::Length)?,
    };
    effect::request(effect, resume)
}

impl Callback {
    fn index(&self) -> u64 {
        match self.kind {
            CallbackKind::Reduce(ArrayReduceKind::ReduceRight)
            | CallbackKind::Find(ArrayFindKind::FindLast | ArrayFindKind::FindLastIndex) => {
                self.length - self.cursor - 1
            }
            _ => self.cursor,
        }
    }
    fn resume(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<EffectStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(EffectStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                effect::request(Effect::Number { value }, self)
            }
            Phase::Species => {
                if !matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive",
                    ));
                }
                runtime.release_jsvalue(std::mem::replace(&mut self.result, value))?;
                self.next(runtime)
            }
            Phase::Read => {
                if matches!(self.kind, CallbackKind::Reduce(_)) && self.accumulator.is_none() {
                    self.accumulator = Some(value);
                    self.cursor += 1;
                    return self.next(runtime);
                }
                runtime.release_jsvalue(std::mem::replace(&mut self.value, value))?;
                self.phase = Phase::Callback;
                let reduce = matches!(self.kind, CallbackKind::Reduce(_));
                let callable = self
                    .callback
                    .as_ref()
                    .ok_or(RuntimeError::Invariant("Array callback missing"))?
                    .try_clone()?;
                let receiver = if reduce {
                    JsValue::Undefined
                } else {
                    runtime.dup_jsvalue(&self.this_arg)?
                };
                let mut arguments = Vec::with_capacity(4);
                if reduce {
                    arguments.push(
                        self.accumulator
                            .take()
                            .ok_or(RuntimeError::Invariant("Array reduce accumulator missing"))?,
                    );
                }
                arguments.push(runtime.dup_jsvalue(&self.value)?);
                arguments.push(Number::compact(self.index() as f64).into());
                arguments.push(if matches!(self.kind, CallbackKind::Find(_)) {
                    runtime.dup_jsvalue(&self.original)?
                } else {
                    JsValue::Object(self.object.try_clone()?.into_handle())
                });
                effect::request(
                    Effect::Call {
                        callable,
                        receiver,
                        arguments,
                    },
                    self,
                )
            }
            Phase::Callback => {
                match self.kind {
                    CallbackKind::Reduce(_) => {
                        if let Some(previous) = self.accumulator.replace(value) {
                            runtime.release_jsvalue(previous)?;
                        }
                    }
                    CallbackKind::Iteration(ArrayIterationKind::Map) => {
                        let index = self.index();
                        return self.define(runtime, index, value);
                    }
                    kind => {
                        let truthy = runtime.value_to_boolean_jsvalue(&value);
                        runtime.release_jsvalue(value)?;
                        let truthy = truthy?;
                        match kind {
                            CallbackKind::Find(kind) if truthy => {
                                let result = if matches!(
                                    kind,
                                    ArrayFindKind::Find | ArrayFindKind::FindLast
                                ) {
                                    std::mem::replace(&mut self.value, JsValue::Undefined)
                                } else {
                                    Number::compact(self.index() as f64).into()
                                };
                                return Ok(EffectStep::Complete(Completion::Return(result)));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Every) if !truthy => {
                                return Ok(EffectStep::Complete(Completion::Return(
                                    JsValue::Bool(false),
                                )));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Some) if truthy => {
                                return Ok(EffectStep::Complete(Completion::Return(
                                    JsValue::Bool(true),
                                )));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Filter) if truthy => {
                                let original =
                                    std::mem::replace(&mut self.value, JsValue::Undefined);
                                let index = self.selected;
                                return self.define(runtime, index, original);
                            }
                            _ => {}
                        }
                    }
                }
                runtime.release_jsvalue(std::mem::replace(&mut self.value, JsValue::Undefined))?;
                self.cursor += 1;
                self.next(runtime)
            }
            _ => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "Array callback value reply phase mismatch",
                ))
            }
        }
    }

    fn number(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array callback number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value),
            NativeConversion::Throw(value) => {
                return Ok(EffectStep::Complete(Completion::Throw(value)));
            }
        };
        let callback = match &self.callback_value {
            JsValue::Object(id) => runtime.as_callable_object(*id)?,
            _ => None,
        };
        self.callback = Some(callback.ok_or_else(|| {
            RuntimeError::Engine(crate::engine::api::Error::new(
                crate::engine::api::ErrorKind::Type,
                "not a function",
            ))
        })?);
        if let CallbackKind::Iteration(kind) = self.kind {
            self.result = match kind {
                ArrayIterationKind::Every => JsValue::Bool(true),
                ArrayIterationKind::Some => JsValue::Bool(false),
                _ => JsValue::Undefined,
            };
            if matches!(kind, ArrayIterationKind::Map | ArrayIterationKind::Filter) {
                self.phase = Phase::Species;
                let effect = Effect::Species {
                    source: self.object.try_clone()?,
                    length: if kind == ArrayIterationKind::Map {
                        self.length
                    } else {
                        0
                    },
                };
                return effect::request(effect, self);
            }
        }
        self.next(runtime)
    }
    fn next(mut self: Box<Self>, runtime: &Runtime) -> Result<EffectStep, RuntimeError> {
        if self.cursor == self.length {
            let result = match self.kind {
                CallbackKind::Iteration(_) => {
                    std::mem::replace(&mut self.result, JsValue::Undefined)
                }
                CallbackKind::Reduce(_) => match self.accumulator.take() {
                    Some(value) => value,
                    None => {
                        return Ok(EffectStep::Complete(Completion::Throw(
                            runtime.new_native_error_jsvalue(
                                self.realm,
                                NativeErrorKind::Type,
                                "empty array",
                            )?,
                        )));
                    }
                },
                CallbackKind::Find(ArrayFindKind::Find | ArrayFindKind::FindLast) => {
                    JsValue::Undefined
                }
                CallbackKind::Find(_) => JsValue::Int(-1),
            };
            return Ok(EffectStep::Complete(Completion::Return(result)));
        }
        let object = self.object.try_clone()?;
        let key = runtime.property_key_for_index(self.index())?;
        let effect = if matches!(self.kind, CallbackKind::Find(_)) {
            self.phase = Phase::Read;
            Effect::Read { object, key }
        } else {
            self.phase = Phase::Has;
            Effect::Has { object, key }
        };
        effect::request(effect, self)
    }
    fn boolean(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::Has) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array callback boolean phase mismatch",
            ));
        }
        let present = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(EffectStep::Complete(Completion::Throw(value)));
            }
        };
        if !present {
            self.cursor += 1;
            return self.next(runtime);
        }
        let effect = Effect::Read {
            object: self.object.try_clone()?,
            key: runtime.property_key_for_index(self.index())?,
        };
        self.phase = Phase::Read;
        effect::request(effect, self)
    }
    fn define(
        mut self: Box<Self>,
        runtime: &Runtime,
        index: u64,
        value: JsValue,
    ) -> Result<EffectStep, RuntimeError> {
        let mut descriptor = OwnedPropertyDescriptor::new(runtime);
        descriptor.value = DescriptorField::Present(value);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        let JsValue::Object(id) = self.result else {
            return Err(RuntimeError::Invariant(
                "Array callback result was not an object",
            ));
        };
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), id)?;
        let key = runtime.property_key_for_index(index)?;
        self.phase = Phase::Define;
        let effect = Effect::Define {
            object,
            key,
            descriptor: Box::new(descriptor),
        };
        effect::request(effect, self)
    }
    fn defined(
        mut self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<EffectStep, RuntimeError> {
        if !matches!(self.phase, Phase::Define) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array callback define phase mismatch",
            ));
        }
        let filter = matches!(
            self.kind,
            CallbackKind::Iteration(ArrayIterationKind::Filter)
        );
        let index = if filter { self.selected } else { self.index() };
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.realm, index, result)?
        {
            return Ok(EffectStep::Complete(Completion::Throw(value)));
        }
        if filter {
            self.selected += 1;
        }
        runtime.release_jsvalue(std::mem::replace(&mut self.value, JsValue::Undefined))?;
        self.cursor += 1;
        self.next(runtime)
    }
}
impl Continuation for Callback {
    fn pending(&mut self) -> &mut Option<Effect> {
        &mut self.pending
    }
    fn reply(self: Box<Self>, runtime: &Runtime, reply: Reply) -> Result<EffectStep, RuntimeError> {
        match reply {
            Reply::Completion(completion) => self.resume(runtime, completion),
            Reply::Number(number) => self.number(runtime, number),
            Reply::Boolean(present) => self.boolean(runtime, present),
            Reply::Defined(result) => self.defined(runtime, result),
            _ => Err(RuntimeError::Invariant(
                "Array callback received an unexpected reply",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unpublished_species_result_and_mapper_survive_wait_then_release() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context().expect("create context");
        let mapper = context.eval("(function(value) { return value; })").unwrap();
        let Value::Object(mapper_object) = &mapper else {
            panic!("expected mapper");
        };
        let source = runtime.new_array(context.realm).unwrap();
        let target = runtime.new_object(None).unwrap();
        let ids = [
            source.object_id(),
            target.object_id(),
            mapper_object.object_id(),
        ];
        let invocation = NativeInvocation::Call {
            this_value: runtime.into_jsvalue(Value::Object(source)).unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![runtime.into_jsvalue(mapper).unwrap()],
        };
        let waiting = |step: EffectStep| match step {
            EffectStep::Effect(then) => then,
            _ => panic!("expected an effect"),
        };
        let mut then = waiting(
            start(
                &runtime,
                context.realm,
                CallbackKind::Iteration(ArrayIterationKind::Map),
                &invocation,
                &arguments,
            )
            .unwrap(),
        );
        let Effect::Read { .. } = then.take_pending().unwrap() else {
            panic!("expected length read");
        };

        let NativeInvocation::Call { this_value } = invocation else {
            unreachable!()
        };
        runtime.release_jsvalue(this_value).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
        let reply = Reply::Completion(Completion::Return(JsValue::Int(1)));
        let mut then = waiting(then.reply(&runtime, reply).unwrap());
        let Effect::Number { .. } = then.take_pending().unwrap() else {
            panic!("expected length conversion");
        };

        let reply = Reply::Number(NativeConversion::Value(1.0));
        let mut then = waiting(then.reply(&runtime, reply).unwrap());
        let Effect::Species { .. } = then.take_pending().unwrap() else {
            panic!("expected species");
        };

        let target = runtime.into_jsvalue(Value::Object(target)).unwrap();
        let reply = Reply::Completion(Completion::Return(target));
        let mut then = waiting(then.reply(&runtime, reply).unwrap());
        let Effect::Has { .. } = then.take_pending().unwrap() else {
            panic!("expected indexed lookup");
        };

        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(then);
        runtime.run_gc().unwrap();
        for (i, id) in ids.into_iter().enumerate() {
            assert!(
                runtime.0.state.borrow().heap.object(id).is_err(),
                "id index {i} still live"
            );
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}
