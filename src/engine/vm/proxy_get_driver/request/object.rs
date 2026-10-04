//! Mechanical adapters for object domain requests.
use super::JsValue;
use super::{
    ArrayLengthStep, DescriptorStep, ProxyBooleanStep, ProxyDefineStep, ProxyGetStep, ProxyOwnStep,
    ProxyPrototypeStep, ProxySetStep, Resume, SetStep, Step, set_completion,
};

impl TryFrom<ProxyGetStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyGetStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxyGetStep::Complete(result) => Self::Complete(Some(result)),
                ProxyGetStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Get(resume)),
                    }
                }
                ProxyGetStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Get(resume)),
                    }
                }
                ProxyGetStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Get(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<ProxyOwnStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyOwnStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxyOwnStep::Complete(result) => Self::OwnComplete(Some(result)),
                ProxyOwnStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Own(resume)),
                    }
                }
                ProxyOwnStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Own(resume)),
                    }
                }
                ProxyOwnStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Own(resume)),
                    }
                }
                ProxyOwnStep::Extensible { mut resume } => {
                    let object = resume.take_extensible_object();
                    Self::Extensible {
                        object: Some(object),
                        resume: Some(Resume::Own(resume)),
                    }
                }
                ProxyOwnStep::Convert { mut resume } => {
                    let value = resume.take_convert_value();
                    Self::Convert {
                        value: Some(value),
                        resume: Some(Resume::Own(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<DescriptorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: DescriptorStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                DescriptorStep::Complete(resume) => Self::Converted(Some(
                    crate::engine::value::conversion::NativeConversion::Value(
                        resume.take_descriptor(),
                    ),
                )),
                DescriptorStep::Throw(value) => Self::Converted(Some(
                    crate::engine::value::conversion::NativeConversion::Throw(value),
                )),
                DescriptorStep::Has { mut resume } => {
                    let object = resume.take_has_object();
                    let key = resume.take_has_key();
                    Self::Has {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Conversion(resume)),
                    }
                }
                DescriptorStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Conversion(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<ProxyBooleanStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyBooleanStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxyBooleanStep::Delete { mut resume } => {
                    let object = resume.take_delete_object();
                    let key = resume.take_delete_key();
                    Self::Delete {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::PreventExtensions { mut resume } => {
                    let object = resume.take_prevent_extensions_object();
                    Self::PreventExtensions {
                        object: Some(object),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::Complete(result) => Self::BooleanComplete(Some(result)),
                ProxyBooleanStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::Has { mut resume } => {
                    let object = resume.take_has_object();
                    let key = resume.take_has_key();
                    Self::Has {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::Extensible { mut resume } => {
                    let object = resume.take_extensible_object();
                    Self::Extensible {
                        object: Some(object),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
                ProxyBooleanStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Boolean(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::object::ProxyCallStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::object::ProxyCallStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::object::ProxyCallStep;
            match step {
                ProxyCallStep::Complete(result) => Self::Complete(Some(result)),
                ProxyCallStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Call(resume)),
                    }
                }
                ProxyCallStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Call(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<SetStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: SetStep) -> Result<Self, Self::Error> {
        Ok(Self::SetProgress(Some(step.into_progress())))
    }
}

impl TryFrom<ProxySetStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxySetStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxySetStep::Complete(result) => Self::SetComplete(Some(set_completion(result))),
                ProxySetStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::ProxySet(resume)),
                    }
                }
                ProxySetStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::ProxySet(resume)),
                    }
                }
                ProxySetStep::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let key = resume.take_set_key();
                    let value = resume.take_set_value();
                    let receiver = resume.take_set_receiver();
                    Self::Set {
                        object: Some(object),
                        key: Some(key),
                        value: Some(value),
                        receiver: Some(receiver),
                        resume: Some(Resume::ProxySet(resume)),
                    }
                }
                ProxySetStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::ProxySet(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<ProxyDefineStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyDefineStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxyDefineStep::Complete(result) => Self::Defined(Some(result)),
                ProxyDefineStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Define(resume)),
                    }
                }
                ProxyDefineStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Define(resume)),
                    }
                }
                ProxyDefineStep::Define { mut resume } => {
                    let object = resume.take_define_object();
                    let key = resume.take_define_key();
                    let descriptor = resume.take_define_descriptor();
                    Self::Define {
                        object: Some(object),
                        key: Some(key),
                        descriptor: Some(descriptor.into()),
                        resume: Some(Resume::Define(resume)),
                    }
                }
                ProxyDefineStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Define(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<ArrayLengthStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ArrayLengthStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ArrayLengthStep::Complete(result) => Self::LengthComplete(Some(result)),
                ArrayLengthStep::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::LengthNumber(resume)),
                },
            }
        })
    }
}

impl TryFrom<ProxyPrototypeStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyPrototypeStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                ProxyPrototypeStep::Complete(result) => Self::Complete(Some(result)),
                ProxyPrototypeStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    let receiver = resume.take_read_receiver();
                    Self::Read {
                        object: Some(object),
                        key: Some(key),
                        receiver: Some(receiver),
                        resume: Some(Resume::Prototype(resume)),
                    }
                }
                ProxyPrototypeStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Prototype(resume)),
                    }
                }
                ProxyPrototypeStep::Get { mut resume } => {
                    let object = resume.take_get_object();
                    Self::GetPrototype {
                        object: Some(object),
                        resume: Some(Resume::Prototype(resume)),
                    }
                }
                ProxyPrototypeStep::Set { mut resume } => {
                    let object = resume.take_set_object();
                    let prototype = resume.take_set_prototype();
                    Self::SetPrototype {
                        object: Some(object),
                        prototype: Some(prototype),
                        resume: Some(Resume::Prototype(resume)),
                    }
                }
                ProxyPrototypeStep::Extensible { mut resume } => {
                    let object = resume.take_extensible_object();
                    Self::Extensible {
                        object: Some(object),
                        resume: Some(Resume::Prototype(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::object::KeysStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::object::KeysStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::object::KeysStep;
            match step {
                KeysStep::Complete(result) => Self::KeysComplete(Some(result)),
                KeysStep::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
                KeysStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
                KeysStep::Number { mut resume } => {
                    let value = resume.take_number_value();
                    Self::Number {
                        value: Some(value),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
                KeysStep::Keys { mut resume } => {
                    let object = resume.take_keys_object();
                    Self::Keys {
                        object: Some(object),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
                KeysStep::Extensible { mut resume } => {
                    let object = resume.take_extensible_object();
                    Self::Extensible {
                        object: Some(object),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
                KeysStep::Descriptor { mut resume } => {
                    let object = resume.take_descriptor_object();
                    let key = resume.take_descriptor_key();
                    Self::Descriptor {
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::Keys(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::object::ProxyConstructStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::object::ProxyConstructStep) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::object::ProxyConstructStep;
            match step {
                ProxyConstructStep::Complete(result) => Self::Complete(Some(result)),
                ProxyConstructStep::Read { mut resume } => {
                    let object = resume.take_read_object();
                    let key = resume.take_read_key();
                    Self::Read {
                        receiver: Some(JsValue::Object(object.try_clone()?.into_handle())),
                        object: Some(object),
                        key: Some(key),
                        resume: Some(Resume::ProxyConstruct(resume)),
                    }
                }
                ProxyConstructStep::Call { mut resume } => {
                    let target = resume.take_call_target();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(target),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::ProxyConstruct(resume)),
                    }
                }
                ProxyConstructStep::Construct { mut resume } => {
                    let target = resume.take_construct_target();
                    let new_target = resume.take_construct_new_target();
                    let arguments = resume.take_construct_arguments();
                    Self::Construct {
                        target: Some(target),
                        new_target: Some(new_target),
                        arguments: Some(arguments),
                        resume: Some(Resume::ProxyConstruct(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::object::object_literal::element::LiteralDefinitionStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::object::object_literal::element::LiteralDefinitionStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::object::object_literal::element::LiteralDefinitionStep as T;
            match step {
                T::Complete(result) => Self::Complete(Some(result)),
                T::Primitive { mut resume } => {
                    let value = resume.take_primitive();
                    Self::Primitive {
                        value: Some(value),
                        hint: Some(crate::engine::vm::ToPrimitiveHint::String),
                        resume: Some(Resume::LiteralDefinition(resume)),
                    }
                }
                T::Define { mut resume } => {
                    let (object, key, descriptor) = resume.take_define();
                    Self::DefineOrdinary {
                        object: Some(object),
                        key: Some(key),
                        descriptor: Some(descriptor.into()),
                        resume: Some(Resume::LiteralDefinition(resume)),
                    }
                }
            }
        })
    }
}
