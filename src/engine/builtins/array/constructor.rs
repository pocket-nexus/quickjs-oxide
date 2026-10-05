//! Array constructor preserves prototype lookup before numeric length validation and indexed Sets.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
        operations::{ArrayLengthConversion, InternalSetResult, PropertyDefineOutcome},
    },
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum ConstructorStep {
    Complete(Completion),
    Read { resume: ConstructorResume },
    Set { resume: ConstructorResume },
}
pub(crate) struct ConstructorResume(Box<ConstructorResumeState>);
impl std::ops::Deref for ConstructorResume {
    type Target = ConstructorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ConstructorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<ConstructorResume>() <= 8);
pub(crate) struct ConstructorResumeState {
    runtime: Runtime,
    pending_effect: ConstructorStepPending,
    scheduler_set_key: Option<PropertyKey>,
    realm: ContextId,
    new_target: JsValue,
    arguments: Vec<JsValue>,
    array: Option<ObjectRef>,
    index: usize,
}
impl Drop for ConstructorResumeState {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.new_target, JsValue::Undefined));
        for value in self.arguments.drain(..) {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.read_receiver.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(value) = self.pending_effect.set_value.take() {
            let _ = self.runtime.release_jsvalue(value);
        }
    }
}
impl ConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array constructor requires constructor-or-function invocation",
            ));
        };
        if let Some(completion) = runtime
            .0
            .state
            .borrow_mut()
            .try_complete_array_constructor(
                &runtime.0.poisoned,
                realm,
                new_target,
                &arguments.readable[..arguments.actual_arg_count],
            )?
        {
            return Ok(Self::Complete(completion));
        }
        let mut resume = ConstructorResume(Box::new(ConstructorResumeState {
            runtime: runtime.clone(),
            pending_effect: ConstructorStepPending::default(),
            scheduler_set_key: None,
            realm,
            new_target: JsValue::Undefined,
            arguments: Vec::new(),
            array: None,
            index: 0,
        }));
        resume.0.new_target = runtime.dup_jsvalue(new_target)?;
        resume
            .0
            .arguments
            .try_reserve_exact(arguments.actual_arg_count)
            .map_err(|_| {
                RuntimeError::Engine(crate::engine::api::Error::internal(
                    "Array constructor arguments allocation failed",
                ))
            })?;
        for value in &arguments.readable[..arguments.actual_arg_count] {
            resume.0.arguments.push(runtime.dup_jsvalue(value)?);
        }
        if matches!(new_target, JsValue::Undefined) {
            resume.resume(runtime, Completion::Return(JsValue::Undefined))
        } else {
            let key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?;
            Ok(Self::request_read(
                runtime.dup_jsvalue(new_target)?,
                key,
                resume,
            ))
        }
    }
}
impl ConstructorResume {
    pub(crate) fn with_scheduler_set_key(mut self, key: PropertyKey) -> Self {
        self.0.scheduler_set_key = Some(key);
        self
    }
    pub(crate) fn take_scheduler_set_key(&mut self) -> PropertyKey {
        self.0.scheduler_set_key.take().expect("waiting Set key")
    }

    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ConstructorStep, RuntimeError> {
        if self.0.array.is_some() {
            let (Completion::Return(value) | Completion::Throw(value)) = result;
            runtime.release_jsvalue(value)?;
            return Err(RuntimeError::Invariant(
                "Array constructor repeated prototype reply",
            ));
        }
        let prototype = match result {
            Completion::Throw(value) => {
                return Ok(ConstructorStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(value) => match value {
                JsValue::Object(object) => ObjectRef::from_owned_handle(runtime.clone(), object),
                value => {
                    runtime.release_jsvalue(value)?;
                    let realm = if matches!(self.0.new_target, JsValue::Undefined) {
                        self.0.realm
                    } else {
                        match runtime
                            .function_realm_from_jsvalue(self.0.realm, &self.0.new_target)?
                        {
                            NativeConversion::Value(realm) => realm,
                            NativeConversion::Throw(value) => {
                                return Ok(ConstructorStep::Complete(Completion::Throw(value)));
                            }
                        }
                    };
                    let prototype = runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .context(realm)?
                        .array_prototype;
                    ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
                }
            },
        };
        let array = runtime.new_empty_array_with_prototype(&prototype)?;
        if self.0.arguments.len() == 1
            && matches!(self.0.arguments[0], JsValue::Int(_) | JsValue::Float(_))
        {
            let length = match match &self.0.arguments[0] {
                JsValue::Int(value) if *value >= 0 => ArrayLengthConversion::Length(*value as u32),
                JsValue::Int(_) => runtime.invalid_array_length(Some(self.0.realm))?,
                JsValue::Float(value) => {
                    runtime.validate_array_length_number(Some(self.0.realm), *value, None)?
                }
                _ => unreachable!(),
            } {
                ArrayLengthConversion::Length(length) => length,
                ArrayLengthConversion::Throw(value) => {
                    return Ok(ConstructorStep::Complete(Completion::Throw(value)));
                }
            };
            let key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
            // Fresh Array own length and an already validated Number cannot invoke JavaScript.
            let mut descriptor = OwnedPropertyDescriptor::new(runtime);
            descriptor.value = DescriptorField::Present(
                crate::engine::value::number::operations::Number::compact(f64::from(length)).into(),
            );
            match runtime.define_owned_property_in_realm(
                Some(self.0.realm),
                &array,
                &key,
                &descriptor,
            )? {
                PropertyDefineOutcome::Defined(true) => {}
                PropertyDefineOutcome::Defined(false) => {
                    return Err(RuntimeError::Invariant(
                        "fresh Array rejected its constructor length",
                    ));
                }
                PropertyDefineOutcome::Throw(value) => {
                    return Ok(ConstructorStep::Complete(Completion::Throw(value)));
                }
            }
            return Ok(ConstructorStep::Complete(Completion::Return(
                JsValue::Object(array.into_handle()),
            )));
        }
        self.0.array = Some(array);
        self.next(runtime)
    }
    fn next(self, runtime: &Runtime) -> Result<ConstructorStep, RuntimeError> {
        let object = self
            .0
            .array
            .as_ref()
            .map(|value| value.try_clone())
            .transpose()?
            .ok_or(RuntimeError::Invariant(
                "Array constructor allocation missing",
            ))?;
        let Some(value) = self.0.arguments.get(self.0.index) else {
            return Ok(ConstructorStep::Complete(Completion::Return(
                JsValue::Object(object.into_handle()),
            )));
        };
        let index = u32::try_from(self.0.index)
            .map_err(|_| RuntimeError::Invariant("native Array argument count exceeded Uint32"))?;
        Ok(ConstructorStep::request_set(
            object,
            runtime.property_key_for_index(u64::from(index))?,
            runtime.dup_jsvalue(value)?,
            self,
        ))
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ConstructorStep, RuntimeError> {
        if self.0.array.is_none() {
            if let NativeConversion::Throw(value) = result {
                let _ = runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "Array constructor set before allocation",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            return Ok(ConstructorStep::Complete(Completion::Throw(value)));
        }
        self.0.index += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ConstructorStep::Complete(result) => return Ok(result),
            ConstructorStep::Read { mut resume } => {
                let receiver = resume.take_read_receiver();
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?,
                )?
            }
            ConstructorStep::Set { mut resume } => {
                let object = resume.take_set_object();
                let key = resume.take_set_key();
                let value = resume.take_set_value();
                {
                    let result = runtime.internal_set_jsvalue(
                        realm,
                        &object,
                        &key,
                        value,
                        JsValue::Object(object.try_clone()?.into_handle()),
                    )?;
                    resume.set(runtime, key, result)?
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::Value;
    #[test]
    fn pending_constructor_owns_arguments_and_new_target_until_abandoned() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let context = runtime.new_context().expect("create context");
        let target = runtime.new_object(None).unwrap();
        let argument = runtime.new_object(None).unwrap();
        let ids = [target.object_id(), argument.object_id()];
        let invocation = NativeInvocation::Construct {
            new_target: runtime
                .unroot_value(&Value::Object(target.try_clone().expect("duplicate root")))
                .unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![
                runtime
                    .unroot_value(&Value::Object(
                        argument.try_clone().expect("duplicate root"),
                    ))
                    .unwrap(),
            ],
        };
        let ConstructorStep::Read { mut resume } =
            ConstructorStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
        else {
            panic!("expected prototype lookup");
        };
        let _ = resume.take_read_key();
        runtime
            .release_jsvalue(resume.take_read_receiver())
            .unwrap();

        let NativeInvocation::Construct { new_target } = invocation else {
            unreachable!()
        };
        runtime.release_jsvalue(new_target).unwrap();
        for value in arguments.readable {
            runtime.release_jsvalue(value).unwrap();
        }
        drop(target);
        drop(argument);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(resume);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

#[derive(Default)]
struct ConstructorStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    set_object: Option<ObjectRef>,
    set_key: Option<PropertyKey>,
    set_value: Option<JsValue>,
}
impl ConstructorStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: ConstructorResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_set(
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        mut resume: ConstructorResume,
    ) -> Self {
        resume.0.pending_effect.set_object = Some(object);
        resume.0.pending_effect.set_key = Some(key);
        resume.0.pending_effect.set_value = Some(value);
        Self::Set { resume }
    }
}
impl ConstructorResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("ConstructorStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("ConstructorStep Read key")
    }
    pub(crate) fn take_set_object(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .set_object
            .take()
            .expect("ConstructorStep Set object")
    }
    pub(crate) fn take_set_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .set_key
            .take()
            .expect("ConstructorStep Set key")
    }
    pub(crate) fn take_set_value(&mut self) -> JsValue {
        self.0
            .pending_effect
            .set_value
            .take()
            .expect("ConstructorStep Set value")
    }
}
const _: () = assert!(std::mem::size_of::<ConstructorStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<ConstructorStep>() <= 64);

impl crate::engine::heap::runtime::RuntimeState {
    /// Execute the constructor's non-callback prefix under one State access.
    /// Declines precede allocation or indexed writes. A prototype getter or
    /// inherited indexed setter is handled by the existing effect consumer.
    pub(crate) fn try_complete_array_constructor(
        &mut self,
        poisoned: &std::cell::Cell<bool>,
        realm: ContextId,
        new_target: &JsValue,
        arguments: &[JsValue],
    ) -> Result<Option<Completion>, RuntimeError> {
        use crate::engine::{
            api::error::{NativeErrorKind, NativeErrorMessage},
            atom::{Atom, AtomIdx, pinned::PinnedAtom},
            heap::runtime::owned_values::OwnedValueGuard,
            heap::{ObjectPayload, PropertySlot, RawValue},
            object::prototypes_allow_dense_append,
        };
        let prototype = match new_target {
            JsValue::Undefined => self.heap.context(realm)?.array_prototype,
            JsValue::Object(target) => {
                let data = self.heap.object(*target)?;
                if matches!(data.payload, ObjectPayload::Proxy { .. }) {
                    return Ok(None);
                }
                let atom = self.pinned_atoms.get(PinnedAtom::Prototype);
                let shape = self.heap.shape(data.shape)?;
                let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
                    return Ok(None);
                };
                let Some(PropertySlot::Data(RawValue::Object(prototype))) =
                    data.slots.get(index as usize)
                else {
                    return Ok(None);
                };
                *prototype
            }
            _ => return Ok(None),
        };
        let numeric_length = match arguments {
            [JsValue::Int(n)] => Some(f64::from(*n)),
            [JsValue::Float(n)] => Some(*n),
            _ => None,
        };
        if numeric_length.is_none() {
            for index in 0..arguments.len() {
                let Some(atom) = u32::try_from(index)
                    .ok()
                    .and_then(Atom::from_immediate_integer)
                else {
                    return Ok(None);
                };
                if !prototypes_allow_dense_append(self, atom, Some(prototype))? {
                    return Ok(None);
                }
            }
        }
        // Preserve the checked temporary prototype owner and its failure order.
        // Both guards reborrow this State; neither carries a Runtime root.
        self.heap.retain_object(prototype)?;
        let mut prototype_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(prototype));
        let (state, _) = prototype_owner.parts();
        let array = state.new_empty_array_with_prototype(poisoned, prototype)?;
        let mut array_owner = OwnedValueGuard::new(state, poisoned, JsValue::Object(array));
        let (state, result_owner) = array_owner.parts();
        if let Some(length) = numeric_length {
            if !(length >= 0.0 && length <= f64::from(u32::MAX) && length.fract() == 0.0) {
                let error = state.new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Range,
                    NativeErrorMessage::from_utf8("invalid array length"),
                )?;
                return Ok(Some(Completion::Throw(JsValue::Object(error))));
            }
            state.replace_property_slot(
                array,
                0,
                PropertySlot::Data(
                    JsValue::from(crate::engine::value::number::operations::Number::compact(
                        f64::from(Runtime::to_uint32_number(length)),
                    ))
                    .into_raw(),
                ),
            )?;
        } else {
            for value in arguments {
                let pushed = state.try_dense_push(array, value)?;
                if pushed.is_none() {
                    return Err(RuntimeError::Invariant(
                        "fresh Array prefix changed under exclusive State access",
                    ));
                }
            }
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "array_constructor_completed_in_state",
        );
        Ok(Some(Completion::Return(
            result_owner.take().expect("Array result owner"),
        )))
    }
}
