//! Array constructor preserves prototype lookup before numeric length validation and indexed Sets.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        operations::{ArrayLengthConversion, InternalSetResult, PropertyDefineOutcome},
    },
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum ConstructorStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: ConstructorResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: ConstructorResume,
    },
}
pub(crate) struct ConstructorResume {
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
        let resume = ConstructorResume {
            realm,
            new_target: new_target.clone(),
            arguments: arguments.readable[..arguments.actual_arg_count].to_vec(),
            array: None,
            index: 0,
        };
        if matches!(new_target, Value::Undefined) {
            resume.resume(runtime, Completion::Return(Value::Undefined))
        } else {
            Ok(Self::Read {
                receiver: new_target.clone(),
                key: runtime.intern_property_key("prototype")?,
                resume,
            })
        }
    }
}
impl ConstructorResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ConstructorStep, RuntimeError> {
        if self.array.is_some() {
            return Err(RuntimeError::Invariant(
                "Array constructor repeated prototype reply",
            ));
        }
        let prototype = match result {
            Completion::Throw(value) => {
                return Ok(ConstructorStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(Value::Object(object)) => object,
            Completion::Return(_) => {
                let realm = if matches!(self.new_target, Value::Undefined) {
                    self.realm
                } else {
                    match runtime.function_realm_from_value(self.realm, &self.new_target)? {
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
        };
        let array = runtime.new_empty_array_with_prototype(&prototype)?;
        if self.arguments.len() == 1 && matches!(self.arguments[0], Value::Int(_) | Value::Float(_))
        {
            let length = match runtime.array_constructor_length(self.realm, &self.arguments[0])? {
                ArrayLengthConversion::Length(length) => length,
                ArrayLengthConversion::Throw(value) => {
                    return Ok(ConstructorStep::Complete(Completion::Throw(value)));
                }
            };
            let key = runtime.intern_property_key("length")?;
            // Fresh Array own length and an already validated Number cannot invoke JavaScript.
            match runtime.define_own_property_in_realm(
                Some(self.realm),
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
                    return Ok(ConstructorStep::Complete(Completion::Throw(value)));
                }
            }
            return Ok(ConstructorStep::Complete(Completion::Return(
                Value::Object(array),
            )));
        }
        self.array = Some(array);
        self.next(runtime)
    }
    fn next(self, runtime: &Runtime) -> Result<ConstructorStep, RuntimeError> {
        let object = self.array.clone().ok_or(RuntimeError::Invariant(
            "Array constructor allocation missing",
        ))?;
        let Some(value) = self.arguments.get(self.index).cloned() else {
            return Ok(ConstructorStep::Complete(Completion::Return(
                Value::Object(object),
            )));
        };
        let index = u32::try_from(self.index)
            .map_err(|_| RuntimeError::Invariant("native Array argument count exceeded Uint32"))?;
        Ok(ConstructorStep::Set {
            object,
            key: runtime.property_key_for_index(u64::from(index))?,
            value,
            resume: self,
        })
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ConstructorStep, RuntimeError> {
        if self.array.is_none() {
            return Err(RuntimeError::Invariant(
                "Array constructor set before allocation",
            ));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(ConstructorStep::Complete(Completion::Throw(value)));
        }
        self.index += 1;
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
            ConstructorStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            ConstructorStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
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
            new_target: Value::Object(target),
        };
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![Value::Object(argument)],
        };
        let ConstructorStep::Read { resume, .. } =
            ConstructorStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
        else {
            panic!("expected prototype lookup");
        };
        drop(arguments);
        drop(invocation);
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
