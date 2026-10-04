//! Mechanical adapters for string domain requests.
use super::{JsValue, Resume, Step};

impl TryFrom<crate::engine::builtins::StringReplaceStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringReplaceStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringReplaceStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::PreparedRead { mut resume } => {
                    let read = resume.take_preparedread_read();
                    let key = resume.take_preparedread_key();
                    Self::PreparedRead {
                        read: Some(read),
                        key: Some(key),
                        resume: Some(Resume::StringReplace(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::StringReplace(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::StringReplace(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpExecStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpExecStep) -> Result<Self, Self::Error> {
        Ok(Self::RegExpExecProgress(Some(step)))
    }
}

impl TryFrom<crate::engine::builtins::RegExpPresentationStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::RegExpPresentationStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpPresentationStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::RegExpPresentation(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                    resume: Some(Resume::RegExpPresentation(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpReplaceStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpReplaceStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpReplaceStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::PreparedSet { mut resume } => {
                    let step = resume.take_preparedset_step();
                    Self::PreparedSet {
                        step: Some(step),
                        resume: Some(Resume::RegExpReplace(resume)),
                    }
                }
                T::PreparedRead { mut resume } => {
                    let read = resume.take_preparedread_read();
                    let key = resume.take_preparedread_key();
                    Self::PreparedRead {
                        read: Some(read),
                        key: Some(key),
                        resume: Some(Resume::RegExpReplace(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    let hint = resume.take_primitive_hint();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(hint),
                        resume: Some(Resume::RegExpReplace(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::RegExpReplace(resume)),
                    }
                }
                T::Exec { mut resume } => {
                    let regexp = resume.take_exec_regexp();
                    let input = resume.take_exec_input();
                    Self::RegExpExec {
                        regexp: Some(regexp),
                        input: Some(input),
                        resume: Some(Resume::RegExpReplace(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::StringTextStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringTextStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringTextStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive {
                    value,
                    hint,
                    resume,
                } => Self::Primitive {
                    value: Some(value),
                    hint: Some(hint),
                    resume: Some(Resume::StringText(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::StringSearchStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringSearchStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringSearchStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::StringSearch(resume)),
                },
                T::Primitive {
                    value,
                    hint,
                    resume,
                } => Self::Primitive {
                    value: Some(value),
                    hint: Some(hint),
                    resume: Some(Resume::StringSearch(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::StringSplitStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringSplitStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringSplitStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::StringSplit(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    let hint = resume.take_primitive_hint();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(hint),
                        resume: Some(Resume::StringSplit(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::StringSplit(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpConstructorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpConstructorStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpConstructorStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::RegExpConstructor(resume)),
                },
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                    resume: Some(Resume::RegExpConstructor(resume)),
                },
                T::Prototype { new_target, resume } => Self::ConstructorSource {
                    new_target: Some(new_target),
                    resume: Some(Resume::RegExpConstructor(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpCompileStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpCompileStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpCompileStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { value, resume } => Self::Primitive {
                    value: Some(value),
                    hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                    resume: Some(Resume::RegExpCompile(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::StringProtocolStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringProtocolStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringProtocolStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::StringProtocol(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::StringProtocol(resume)),
                    }
                }
                T::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::StringProtocol(resume)),
                    }
                }
                T::Construct { mut resume } => {
                    let constructor = resume.take_construct_constructor();
                    let new_target = constructor.try_clone()?;
                    let arguments = resume.take_construct_arguments();
                    Self::Construct {
                        new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                            new_target,
                        )),
                        target: Some(constructor),
                        arguments: Some(arguments),
                        resume: Some(Resume::StringProtocol(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpSearchStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpSearchStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpSearchStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::RegExpSearch(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::RegExpSearch(resume)),
                    }
                }
                T::Exec { mut resume } => {
                    let regexp = resume.take_exec_regexp();
                    let input = resume.take_exec_input();
                    Self::RegExpExec {
                        regexp: Some(regexp),
                        input: Some(input),
                        resume: Some(Resume::RegExpSearch(resume)),
                    }
                }
                T::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = match object.try_clone() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    Self::Set {
                        receiver: Some(JsValue::Object(receiver.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        value: Some(value),
                        resume: Some(Resume::RegExpSearch(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpMatchStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpMatchStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpMatchStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::RegExpMatch(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    let hint = resume.take_primitive_hint();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(hint),
                        resume: Some(Resume::RegExpMatch(resume)),
                    }
                }
                T::Exec { mut resume } => {
                    let regexp = resume.take_exec_regexp();
                    let input = resume.take_exec_input();
                    Self::RegExpExec {
                        regexp: Some(regexp),
                        input: Some(input),
                        resume: Some(Resume::RegExpMatch(resume)),
                    }
                }
                T::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = match object.try_clone() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    Self::Set {
                        receiver: Some(JsValue::Object(receiver.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        value: Some(value),
                        resume: Some(Resume::RegExpMatch(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpMatchAllStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpMatchAllStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpMatchAllStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::RegExpMatchAll(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    let hint = resume.take_primitive_hint();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(hint),
                        resume: Some(Resume::RegExpMatchAll(resume)),
                    }
                }
                T::Species { mut resume } => {
                    let regexp = resume.take_species_regexp();
                    Self::RegExpSpecies {
                        regexp: Some(regexp),
                        resume: Some(Resume::RegExpMatchAll(resume)),
                    }
                }
                T::Construct { mut resume } => {
                    let constructor = resume.take_construct_constructor();
                    let new_target = constructor.try_clone()?;
                    let arguments = resume.take_construct_arguments();
                    Self::Construct {
                        new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                            new_target,
                        )),
                        target: Some(constructor),
                        arguments: Some(arguments),
                        resume: Some(Resume::RegExpMatchAll(resume)),
                    }
                }
                T::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = match object.try_clone() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    Self::Set {
                        receiver: Some(JsValue::Object(receiver.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        value: Some(value),
                        resume: Some(Resume::RegExpMatchAll(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpSplitStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpSplitStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpSplitStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    let hint = resume.take_primitive_hint();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(hint),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
                T::Species { mut resume } => {
                    let regexp = resume.take_species_regexp();
                    Self::RegExpSpecies {
                        regexp: Some(regexp),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
                T::Construct { mut resume } => {
                    let constructor = resume.take_construct_constructor();
                    let new_target = constructor.try_clone()?;
                    let arguments = resume.take_construct_arguments();
                    Self::Construct {
                        new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                            new_target,
                        )),
                        target: Some(constructor),
                        arguments: Some(arguments),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
                T::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = match object.try_clone() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    Self::Set {
                        receiver: Some(JsValue::Object(receiver.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        value: Some(value),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
                T::Exec { mut resume } => {
                    let regexp = resume.take_exec_regexp();
                    let input = resume.take_exec_input();
                    Self::RegExpExec {
                        regexp: Some(regexp),
                        input: Some(input),
                        resume: Some(Resume::RegExpSplit(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpSpeciesStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpSpeciesStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpSpeciesStep as T;
            match step {
                T::Complete(result) => Self::RegExpSpeciesComplete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::RegExpSpecies(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::RegExpIteratorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::RegExpIteratorStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::RegExpIteratorStep as T;
            match step {
                T::Complete(result) => Self::NativeRawComplete(Some(result)),
                T::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::RegExpIterator(resume)),
                    }
                }
                T::String { mut resume } => {
                    let value = resume.take_string_value();
                    Self::String {
                        value: Some(value),
                        resume: Some(Resume::RegExpIterator(resume)),
                    }
                }
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive_value();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::Number),
                        resume: Some(Resume::RegExpIterator(resume)),
                    }
                }
                T::Exec { mut resume } => {
                    let regexp = resume.take_exec_regexp();
                    let input = resume.take_exec_input();
                    Self::RegExpExec {
                        regexp: Some(regexp),
                        input: Some(input),
                        resume: Some(Resume::RegExpIterator(resume)),
                    }
                }
                T::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = match object.try_clone() {
                        Ok(receiver) => receiver,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    let scheduler_key = match key.try_clone() {
                        Ok(key) => key,
                        Err(error) => {
                            let _ = object.runtime().release_jsvalue(value);
                            return Err(error);
                        }
                    };
                    Self::Set {
                        receiver: Some(JsValue::Object(receiver.into_handle())),
                        object: Some(object),
                        key: Some(scheduler_key),
                        value: Some(value),
                        resume: Some(Resume::RegExpIteratorSet {
                            resume: resume.with_scheduler_set_key(key),
                        }),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::StringFactoryStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::builtins::StringFactoryStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::StringFactoryStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Read {
                    object,
                    key,
                    resume,
                } => Self::Read {
                    receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                    object: Some(object),
                    key: Some(key),
                    resume: Some(Resume::StringFactory(resume)),
                },
                T::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::StringFactory(resume)),
                },
                T::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::StringFactory(resume)),
                },
            }
        })
    }
}
