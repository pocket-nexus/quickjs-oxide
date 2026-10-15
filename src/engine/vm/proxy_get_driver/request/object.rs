//! Mechanical adapters for object domain requests.
use super::JsValue;
use super::{
    ArrayLengthStep, DescriptorStep, ProxyBooleanStep, ProxyDefineStep, ProxyGetStep, ProxyOwnStep,
    ProxyPrototypeStep, ProxySetStep, Resume, SetStep, Step, set_completion,
};

impl TryFrom<ProxyGetStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: ProxyGetStep) -> Result<Self, Self::Error> {
        Ok(match step {
            ProxyGetStep::Complete(result) => Self::Complete(Some(result)),
            ProxyGetStep::Effect(effect) => Self::StateRead {
                effect: Some(effect),
                atom: None,
                resume: Some(Resume::Identity),
            },
        })
    }
}

step_from! {
    ProxyOwnStep => Own;
    StateRead => state_read(take_state_read);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    Extensible => Extensible { object: take_extensible_object };
    Convert => Convert { value: take_convert_value };
    raw {
        T::Complete(result) => Self::OwnComplete(Some(result)),
    }
}

step_from! {
    DescriptorStep => Conversion;
    Has => Has { object: take_has_object, key: take_has_key };
    Read => Read { object: take_read_object, key: take_read_key, receiver: take_read_receiver };
    raw {
        T::Complete(resume) => Self::Converted(Some(
            crate::engine::value::conversion::NativeConversion::Value(
                resume.take_descriptor(),
            ),
        )),
        T::Throw(value) => Self::Converted(Some(
            crate::engine::value::conversion::NativeConversion::Throw(value),
        )),
    }
}

step_from! {
    ProxyBooleanStep => Boolean;
    Delete => Delete { object: take_delete_object, key: take_delete_key };
    PreventExtensions => PreventExtensions { object: take_prevent_extensions_object };
    StateRead => state_read(take_state_read);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Has => Has { object: take_has_object, key: take_has_key };
    Extensible => Extensible { object: take_extensible_object };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    raw {
        T::Complete(result) => Self::BooleanComplete(Some(result)),
    }
}

step_from! {
    crate::engine::object::ProxyCallStep => Call;
    Read => Read { object: take_read_object, key: take_read_key, receiver: take_read_receiver };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    SetStep => OrdinarySet;
    Proxy => SetProxy { object: take_object, key: take_key, value: take_value,
        receiver: take_receiver };
    Descriptor => Descriptor { object: take_object, key: take_key };
    Define => Define { object: take_object, key: take_key,
        descriptor: take_descriptor as Into::into };
    raw {
        T::Complete(action) => Self::SetComplete(Some(action)),
        T::Continue { resume } => Self::SetContinue(Some(resume)),
        T::Special { mut resume } => {
            let object = resume.take_object();
            let key = resume.take_key();
            let value = resume.take_value();
            let receiver = resume.take_receiver();
            Self::SetSpecial {
                object: Some(object),
                key: Some(key),
                value: Some(value),
                receiver: Some(receiver),
                resume: Some(resume),
            }
        }
        T::ArrayLength { mut resume } => {
            drop(resume.take_object());
            drop(resume.take_key());
            let value = resume.take_value();
            Self::SetLength {
                value: Some(value),
                resume: Some(resume),
            }
        }
    }
}

step_from! {
    ProxySetStep => ProxySet;
    StateRead => state_read(take_state_read);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Set => Set { object: take_set_object, key: take_set_key, value: take_set_value,
        receiver: take_set_receiver };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    raw {
        T::Complete(result) => Self::SetComplete(Some(set_completion(result))),
    }
}

step_from! {
    ProxyDefineStep => Define;
    StateRead => state_read(take_state_read);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    raw {
        T::Complete(result) => Self::Defined(Some(result)),
    }
}

step_from! {
    ArrayLengthStep => LengthNumber;
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::LengthComplete(Some(result)),
    }
}

step_from! {
    ProxyPrototypeStep => Prototype;
    StateRead => state_read(take_state_read);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Get => GetPrototype { object: take_get_object };
    Set => SetPrototype { object: take_set_object, prototype: take_set_prototype };
    Extensible => Extensible { object: take_extensible_object };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::object::KeysStep => Keys;
    StateRead => state_read(take_state_read);
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Number => Number { value: take_number_value };
    Keys => Keys { object: take_keys_object };
    Extensible => Extensible { object: take_extensible_object };
    Descriptor => Descriptor { object: take_descriptor_object, key: take_descriptor_key };
    raw {
        T::Complete(result) => Self::KeysComplete(Some(result)),
    }
}

step_from! {
    crate::engine::object::ProxyConstructStep => ProxyConstruct;
    Read => read(take_read_object, take_read_key);
    Call => Call { target: take_call_target, receiver: take_call_receiver,
        arguments: take_call_arguments };
    Construct => Construct { target: take_construct_target, new_target: take_construct_new_target,
        arguments: take_construct_arguments };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::object::object_literal::element::LiteralDefinitionStep => LiteralDefinition;
    Primitive => Primitive { value: take_primitive;
        hint: crate::engine::vm::ToPrimitiveHint::String };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
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
}
