//! Array callback loops retain the captured length and reread each observable property.

use crate::engine::builtins::native::NativeFunctionId;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayFindKind, ArrayIterationKind, ArrayReduceKind},
    heap::ContextId,
    object::{
        CallableRef, DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
        operations::InternalDefineResult,
    },
    value::{JsValue, conversion::NativeConversion},
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
pub(crate) enum CallbackStep {
    Complete(Completion),
    Read { resume: CallbackResume },
    Number { resume: CallbackResume },
    Has { resume: CallbackResume },
    Call { resume: CallbackResume },
    Species { resume: CallbackResume },
    Define { resume: CallbackResume },
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
pub(crate) struct CallbackResume(Box<CallbackResumeState>);
impl std::ops::Deref for CallbackResume {
    type Target = CallbackResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for CallbackResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<CallbackResume>() <= 8);
pub(crate) struct CallbackResumeState {
    runtime: Runtime,
    pending_effect: CallbackStepPending,
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
impl Drop for CallbackResumeState {
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
        if let Some(value) = self.pending_effect.number_value.take() {
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
impl CallbackStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: CallbackKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array callback requires generic invocation",
            ));
        };
        let object =
            match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(this_value)?)? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
        let mut resume = CallbackResume(Box::new(CallbackResumeState {
            runtime: runtime.clone(),
            pending_effect: CallbackStepPending::default(),
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
        }));
        resume.0.original = runtime.dup_jsvalue(this_value)?;
        resume.0.callback_value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("Array callback argv was not padded"),
        )?)?;
        if arguments.actual_arg_count > 1 {
            let second = arguments.readable.get(1).ok_or(RuntimeError::Invariant(
                "Array callback second argument missing",
            ))?;
            resume.0.this_arg = runtime.dup_jsvalue(second)?;
            if matches!(kind, CallbackKind::Reduce(_)) {
                resume.0.accumulator = Some(runtime.dup_jsvalue(second)?);
            }
        }
        let key = runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
        Ok(Self::request_read(resume.0.object.clone(), key, resume))
    }
}

impl CallbackResume {
    fn index(&self) -> u64 {
        match self.0.kind {
            CallbackKind::Reduce(ArrayReduceKind::ReduceRight)
            | CallbackKind::Find(ArrayFindKind::FindLast | ArrayFindKind::FindLastIndex) => {
                self.0.length - self.0.cursor - 1
            }
            _ => self.0.cursor,
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<CallbackStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            Phase::Length => {
                self.0.phase = Phase::Number;
                Ok(CallbackStep::request_number(value, self))
            }
            Phase::Species => {
                if !matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "ArraySpeciesCreate returned a primitive",
                    ));
                }
                runtime.release_jsvalue(std::mem::replace(&mut self.0.result, value))?;
                self.next(runtime)
            }
            Phase::Read => {
                if matches!(self.0.kind, CallbackKind::Reduce(_)) && self.0.accumulator.is_none() {
                    self.0.accumulator = Some(value);
                    self.0.cursor += 1;
                    return self.next(runtime);
                }
                runtime.release_jsvalue(std::mem::replace(&mut self.0.value, value))?;
                self.0.phase = Phase::Callback;
                self.0.pending_effect.call_callable = Some(
                    self.0
                        .callback
                        .as_ref()
                        .ok_or(RuntimeError::Invariant("Array callback missing"))?
                        .clone(),
                );
                self.0.pending_effect.call_receiver =
                    Some(if matches!(self.0.kind, CallbackKind::Reduce(_)) {
                        JsValue::Undefined
                    } else {
                        runtime.dup_jsvalue(&self.0.this_arg)?
                    });
                self.0.pending_effect.call_arguments = Some(Vec::with_capacity(4));
                if matches!(self.0.kind, CallbackKind::Reduce(_)) {
                    let accumulator = self
                        .0
                        .accumulator
                        .take()
                        .ok_or(RuntimeError::Invariant("Array reduce accumulator missing"))?;
                    self.0
                        .pending_effect
                        .call_arguments
                        .as_mut()
                        .unwrap()
                        .push(accumulator);
                }
                let argument = runtime.dup_jsvalue(&self.0.value)?;
                self.0
                    .pending_effect
                    .call_arguments
                    .as_mut()
                    .unwrap()
                    .push(argument);
                let index =
                    crate::engine::value::number::operations::Number::compact(self.index() as f64)
                        .into();
                self.0
                    .pending_effect
                    .call_arguments
                    .as_mut()
                    .unwrap()
                    .push(index);
                let receiver = if matches!(self.0.kind, CallbackKind::Find(_)) {
                    runtime.dup_jsvalue(&self.0.original)?
                } else {
                    JsValue::Object(self.0.object.clone().into_handle())
                };
                self.0
                    .pending_effect
                    .call_arguments
                    .as_mut()
                    .unwrap()
                    .push(receiver);
                Ok(CallbackStep::Call { resume: self })
            }
            Phase::Callback => {
                match self.0.kind {
                    CallbackKind::Reduce(_) => {
                        if let Some(previous) = self.0.accumulator.replace(value) {
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
                                    std::mem::replace(&mut self.0.value, JsValue::Undefined)
                                } else {
                                    crate::engine::value::number::operations::Number::compact(
                                        self.index() as f64,
                                    )
                                    .into()
                                };
                                return Ok(CallbackStep::Complete(Completion::Return(result)));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Every) if !truthy => {
                                return Ok(CallbackStep::Complete(Completion::Return(
                                    JsValue::Bool(false),
                                )));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Some) if truthy => {
                                return Ok(CallbackStep::Complete(Completion::Return(
                                    JsValue::Bool(true),
                                )));
                            }
                            CallbackKind::Iteration(ArrayIterationKind::Filter) if truthy => {
                                let original =
                                    std::mem::replace(&mut self.0.value, JsValue::Undefined);
                                let index = self.0.selected;
                                return self.define(runtime, index, original);
                            }
                            _ => {}
                        }
                    }
                }
                runtime
                    .release_jsvalue(std::mem::replace(&mut self.0.value, JsValue::Undefined))?;
                self.0.cursor += 1;
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

    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Number) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array callback number phase mismatch",
            ));
        }
        self.0.length = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value),
            NativeConversion::Throw(value) => {
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        let callback = match &self.0.callback_value {
            JsValue::Object(id) => runtime.as_callable_object(*id)?,
            _ => None,
        };
        self.0.callback = Some(callback.ok_or_else(|| {
            RuntimeError::Engine(crate::engine::api::Error::new(
                crate::engine::api::ErrorKind::Type,
                "not a function",
            ))
        })?);
        if let CallbackKind::Iteration(kind) = self.0.kind {
            self.0.result = match kind {
                ArrayIterationKind::Every => JsValue::Bool(true),
                ArrayIterationKind::Some => JsValue::Bool(false),
                _ => JsValue::Undefined,
            };
            if matches!(kind, ArrayIterationKind::Map | ArrayIterationKind::Filter) {
                self.0.phase = Phase::Species;
                return Ok(CallbackStep::request_species(
                    self.0.object.clone(),
                    if kind == ArrayIterationKind::Map {
                        self.0.length
                    } else {
                        0
                    },
                    self,
                ));
            }
        }
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<CallbackStep, RuntimeError> {
        if self.0.cursor == self.0.length {
            let result = match self.0.kind {
                CallbackKind::Iteration(_) => {
                    std::mem::replace(&mut self.0.result, JsValue::Undefined)
                }
                CallbackKind::Reduce(_) => match self.0.accumulator.take() {
                    Some(value) => value,
                    None => {
                        return Ok(CallbackStep::Complete(Completion::Throw(
                            runtime.new_native_error_jsvalue(
                                self.0.realm,
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
            return Ok(CallbackStep::Complete(Completion::Return(result)));
        }
        let key = runtime.property_key_for_index(self.index())?;
        if matches!(self.0.kind, CallbackKind::Find(_)) {
            self.0.phase = Phase::Read;
            Ok(CallbackStep::request_read(self.0.object.clone(), key, self))
        } else {
            self.0.phase = Phase::Has;
            Ok(CallbackStep::request_has(self.0.object.clone(), key, self))
        }
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Has) {
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
                return Ok(CallbackStep::Complete(Completion::Throw(value)));
            }
        };
        if !present {
            self.0.cursor += 1;
            return self.next(runtime);
        }
        let key = runtime.property_key_for_index(self.index())?;
        self.0.phase = Phase::Read;
        Ok(CallbackStep::request_read(self.0.object.clone(), key, self))
    }
    fn define(
        mut self,
        runtime: &Runtime,
        index: u64,
        value: JsValue,
    ) -> Result<CallbackStep, RuntimeError> {
        let mut descriptor = OwnedPropertyDescriptor::new(runtime);
        descriptor.value = DescriptorField::Present(value);
        descriptor.writable = DescriptorField::Present(true);
        descriptor.enumerable = DescriptorField::Present(true);
        descriptor.configurable = DescriptorField::Present(true);
        let JsValue::Object(id) = self.0.result else {
            return Err(RuntimeError::Invariant(
                "Array callback result was not an object",
            ));
        };
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), id)?;
        let key = runtime.property_key_for_index(index)?;
        self.0.phase = Phase::Define;
        Ok(CallbackStep::request_define(object, key, descriptor, self))
    }
    pub(crate) fn defined(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<CallbackStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Define) {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array callback define phase mismatch",
            ));
        }
        let filter = matches!(
            self.0.kind,
            CallbackKind::Iteration(ArrayIterationKind::Filter)
        );
        let index = if filter {
            self.0.selected
        } else {
            self.index()
        };
        if let Some(value) =
            runtime.finish_create_indexed_data_property(self.0.realm, index, result)?
        {
            return Ok(CallbackStep::Complete(Completion::Throw(value)));
        }
        if filter {
            self.0.selected += 1;
        }
        runtime.release_jsvalue(std::mem::replace(&mut self.0.value, JsValue::Undefined))?;
        self.0.cursor += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CallbackStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CallbackStep::Complete(result) => return Ok(result),
            CallbackStep::Read { mut resume } => {
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_property_in_realm(realm, &object, &key)?,
                )?
            }
            CallbackStep::Number { mut resume } => {
                let value = resume.take_number_value();
                resume.number(runtime, runtime.native_to_number_jsvalue(realm, value)?)?
            }
            CallbackStep::Has { mut resume } => {
                let object = resume.take_has_object();
                let key = resume.take_has_key();
                resume.boolean(
                    runtime,
                    runtime.internal_has_property(realm, &object, &key)?,
                )?
            }
            CallbackStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = resume.take_call_receiver();
                let arguments = resume.take_call_arguments();
                resume.resume(
                    runtime,
                    runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?,
                )?
            }
            CallbackStep::Species { mut resume } => {
                let source = resume.take_species_source();
                let length = resume.take_species_length();
                resume.resume(
                    runtime,
                    super::species::finish(
                        runtime,
                        realm,
                        super::species::SpeciesStep::start(runtime, realm, &source, length)?,
                    )?,
                )?
            }
            CallbackStep::Define { mut resume } => {
                let object = resume.take_define_object();
                let key = resume.take_define_key();
                let descriptor = resume.take_define_descriptor();
                resume.defined(
                    runtime,
                    runtime.internal_define_owned_property(realm, &object, &key, descriptor)?,
                )?
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unpublished_species_result_and_mapper_survive_wait_then_release() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
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
        let CallbackStep::Read { mut resume } = CallbackStep::start(
            &runtime,
            context.realm,
            CallbackKind::Iteration(ArrayIterationKind::Map),
            &invocation,
            &arguments,
        )
        .unwrap() else {
            panic!("expected length read");
        };
        let _ = resume.take_read_object();
        let _ = resume.take_read_key();

        let NativeInvocation::Call { this_value } = invocation else {
            unreachable!()
        };
        runtime.release_jsvalue(this_value).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
        let CallbackStep::Number { mut resume } = resume
            .resume(&runtime, Completion::Return(JsValue::Int(1)))
            .unwrap()
        else {
            panic!("expected length conversion");
        };
        let _ = resume.take_number_value();

        let CallbackStep::Species { mut resume } = resume
            .number(&runtime, NativeConversion::Value(1.0))
            .unwrap()
        else {
            panic!("expected species");
        };
        let _ = resume.take_species_source();
        let _ = resume.take_species_length();

        let CallbackStep::Has { mut resume } = resume
            .resume(
                &runtime,
                Completion::Return(runtime.into_jsvalue(Value::Object(target)).unwrap()),
            )
            .unwrap()
        else {
            panic!("expected indexed lookup");
        };
        let _ = resume.take_has_object();
        let _ = resume.take_has_key();

        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
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

#[derive(Default)]
struct CallbackStepPending {
    read_object: Option<ObjectRef>,
    read_key: Option<PropertyKey>,
    number_value: Option<JsValue>,
    has_object: Option<ObjectRef>,
    has_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    species_source: Option<ObjectRef>,
    species_length: Option<u64>,
    define_object: Option<ObjectRef>,
    define_key: Option<PropertyKey>,
    define_descriptor: Option<OwnedPropertyDescriptor>,
}
impl CallbackStep {
    pub(crate) fn request_read(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: CallbackResume,
    ) -> Self {
        resume.0.pending_effect.read_object = Some(object);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_number(value: JsValue, mut resume: CallbackResume) -> Self {
        resume.0.pending_effect.number_value = Some(value);
        Self::Number { resume }
    }
    pub(crate) fn request_has(
        object: ObjectRef,
        key: PropertyKey,
        mut resume: CallbackResume,
    ) -> Self {
        resume.0.pending_effect.has_object = Some(object);
        resume.0.pending_effect.has_key = Some(key);
        Self::Has { resume }
    }
    pub(crate) fn request_species(
        source: ObjectRef,
        length: u64,
        mut resume: CallbackResume,
    ) -> Self {
        resume.0.pending_effect.species_source = Some(source);
        resume.0.pending_effect.species_length = Some(length);
        Self::Species { resume }
    }
    pub(crate) fn request_define(
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OwnedPropertyDescriptor,
        mut resume: CallbackResume,
    ) -> Self {
        resume.0.pending_effect.define_object = Some(object);
        resume.0.pending_effect.define_key = Some(key);
        resume.0.pending_effect.define_descriptor = Some(descriptor);
        Self::Define { resume }
    }
}
impl CallbackResume {
    pub(crate) fn take_read_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .read_object
            .take()
            .expect("CallbackStep Read object")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("CallbackStep Read key")
    }
    pub(crate) fn take_number_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .number_value
            .take()
            .expect("CallbackStep Number value")
    }
    pub(crate) fn take_has_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .has_object
            .take()
            .expect("CallbackStep Has object")
    }
    pub(crate) fn take_has_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .has_key
            .take()
            .expect("CallbackStep Has key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("CallbackStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("CallbackStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("CallbackStep Call arguments")
    }
    pub(crate) fn take_species_source(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .species_source
            .take()
            .expect("CallbackStep Species source")
    }
    pub(crate) fn take_species_length(&mut self) -> u64 {
        self.0
            .pending_effect
            .species_length
            .take()
            .expect("CallbackStep Species length")
    }
    pub(crate) fn take_define_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .define_object
            .take()
            .expect("CallbackStep Define object")
    }
    pub(crate) fn take_define_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .define_key
            .take()
            .expect("CallbackStep Define key")
    }
    pub(crate) fn take_define_descriptor(&mut self) -> OwnedPropertyDescriptor {
        self.0
            .pending_effect
            .define_descriptor
            .take()
            .expect("CallbackStep Define descriptor")
    }
}
const _: () = assert!(std::mem::size_of::<CallbackStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<CallbackStep>() <= 64);
