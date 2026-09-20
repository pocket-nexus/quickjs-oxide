//! Array constructor preserves prototype lookup before numeric length validation and indexed Sets.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        operations::{ArrayLengthConversion, InternalSetResult, PropertyDefineOutcome},
    },
    value::{JsValue, Value, conversion::NativeConversion},
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
    pending_effect: ConstructorStepPending,
    scheduler_set_key: Option<PropertyKey>,
    realm: ContextId,
    new_target: Value,
    arguments: Vec<Value>,
    array: Option<ObjectRef>,
    index: usize,
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
        let resume = ConstructorResume(Box::new(ConstructorResumeState {
            pending_effect: ConstructorStepPending::default(),
            scheduler_set_key: None,
            realm,
            new_target: runtime.root_value(new_target)?,
            arguments: arguments.readable[..arguments.actual_arg_count]
                .iter()
                .map(|value| runtime.root_value(value))
                .collect::<Result<Vec<_>, _>>()?,
            array: None,
            index: 0,
        }));
        if matches!(new_target, JsValue::Undefined) {
            resume.resume(runtime, Completion::Return(JsValue::Undefined))
        } else {
            Ok(Self::request_read(
                runtime.dup_jsvalue(new_target)?,
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)?,
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
            return Err(RuntimeError::Invariant(
                "Array constructor repeated prototype reply",
            ));
        }
        let prototype = match result {
            Completion::Throw(value) => {
                return Ok(ConstructorStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(value) => match runtime.root_and_release_jsvalue(value)? {
                Value::Object(object) => object,
                _ => {
                    let realm = if matches!(self.0.new_target, Value::Undefined) {
                        self.0.realm
                    } else {
                        match runtime.function_realm_from_value(self.0.realm, &self.0.new_target)? {
                            NativeConversion::Value(realm) => realm,
                            NativeConversion::Throw(value) => {
                                return Ok(ConstructorStep::Complete(Completion::Throw(
                                    runtime.into_jsvalue(value)?,
                                )));
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
            && matches!(self.0.arguments[0], Value::Int(_) | Value::Float(_))
        {
            let length =
                match runtime.array_constructor_length(self.0.realm, &self.0.arguments[0])? {
                    ArrayLengthConversion::Length(length) => length,
                    ArrayLengthConversion::Throw(value) => {
                        return Ok(ConstructorStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
            let key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
            // Fresh Array own length and an already validated Number cannot invoke JavaScript.
            match runtime.define_own_property_in_realm(
                Some(self.0.realm),
                &array,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Runtime::array_length_value(length)),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )? {
                PropertyDefineOutcome::Defined(true) => {}
                PropertyDefineOutcome::Defined(false) => {
                    return Err(RuntimeError::Invariant(
                        "fresh Array rejected its constructor length",
                    ));
                }
                PropertyDefineOutcome::Throw(value) => {
                    return Ok(ConstructorStep::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    )));
                }
            }
            return Ok(ConstructorStep::Complete(Completion::Return(
                runtime.into_jsvalue(Value::Object(array))?,
            )));
        }
        self.0.array = Some(array);
        self.next(runtime)
    }
    fn next(self, runtime: &Runtime) -> Result<ConstructorStep, RuntimeError> {
        let object = self.0.array.clone().ok_or(RuntimeError::Invariant(
            "Array constructor allocation missing",
        ))?;
        let Some(value) = self.0.arguments.get(self.0.index).cloned() else {
            return Ok(ConstructorStep::Complete(Completion::Return(
                runtime.into_jsvalue(Value::Object(object))?,
            )));
        };
        let index = u32::try_from(self.0.index)
            .map_err(|_| RuntimeError::Invariant("native Array argument count exceeded Uint32"))?;
        Ok(ConstructorStep::request_set(
            object,
            runtime.property_key_for_index(u64::from(index))?,
            runtime.into_jsvalue(value)?,
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
            return Err(RuntimeError::Invariant(
                "Array constructor set before allocation",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.0.realm, &key, result)? {
            return Ok(ConstructorStep::Complete(Completion::Throw(
                runtime.into_jsvalue(value)?,
            )));
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
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            ConstructorStep::Set { mut resume } => {
                let object = resume.take_set_object();
                let key = resume.take_set_key();
                let value = runtime.root_and_release_jsvalue(resume.take_set_value())?;
                {
                    let result = runtime.internal_set(
                        realm,
                        &object,
                        &key,
                        value,
                        Value::Object(object.clone()),
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
    #[test]
    fn pending_constructor_owns_arguments_and_new_target_until_abandoned() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let context = runtime.new_context();
        let target = runtime.new_object(None).unwrap();
        let argument = runtime.new_object(None).unwrap();
        let ids = [target.object_id(), argument.object_id()];
        let invocation = NativeInvocation::Construct {
            new_target: runtime
                .unroot_value(&Value::Object(target.clone()))
                .unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![runtime
                .unroot_value(&Value::Object(argument.clone()))
                .unwrap()],
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
