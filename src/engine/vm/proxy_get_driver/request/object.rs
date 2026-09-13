//! Mechanical adapters for object domain requests.
use super::{
    ArrayLengthStep, DescriptorStep, ProxyBooleanStep, ProxyDefineStep, ProxyGetStep, ProxyOwnStep,
    ProxyPrototypeStep, ProxySetStep, Resume, SetStep, Step, Value, set_completion,
};

impl From<ProxyGetStep> for Step {
    fn from(step: ProxyGetStep) -> Self {
        match step {
            ProxyGetStep::Complete(result) => Self::Complete(result),
            ProxyGetStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Get(resume),
            },
            ProxyGetStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Get(resume),
            },
            ProxyGetStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Get(resume),
            },
        }
    }
}

impl From<ProxyOwnStep> for Step {
    fn from(step: ProxyOwnStep) -> Self {
        match step {
            ProxyOwnStep::Complete(result) => Self::OwnComplete(result),
            ProxyOwnStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Own(resume),
            },
            ProxyOwnStep::Convert { value, resume } => Self::Convert {
                value,
                resume: Resume::Own(resume),
            },
        }
    }
}

impl From<DescriptorStep> for Step {
    fn from(step: DescriptorStep) -> Self {
        match step {
            DescriptorStep::Complete(result) => Self::Converted(result),
            DescriptorStep::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Conversion(resume),
            },
            DescriptorStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Conversion(resume),
            },
        }
    }
}

impl From<ProxyBooleanStep> for Step {
    fn from(step: ProxyBooleanStep) -> Self {
        match step {
            ProxyBooleanStep::Delete {
                object,
                key,
                resume,
            } => Self::Delete {
                object,
                key,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::PreventExtensions { object, resume } => Self::PreventExtensions {
                object,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Complete(result) => Self::BooleanComplete(result),
            ProxyBooleanStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Has {
                object,
                key,
                resume,
            } => Self::Has {
                object,
                key,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Boolean(resume),
            },
            ProxyBooleanStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Boolean(resume),
            },
        }
    }
}

impl From<crate::engine::object::ProxyCallStep> for Step {
    fn from(step: crate::engine::object::ProxyCallStep) -> Self {
        use crate::engine::object::ProxyCallStep;
        match step {
            ProxyCallStep::Complete(result) => Self::Complete(result),
            ProxyCallStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Call(resume),
            },
            ProxyCallStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Call(resume),
            },
        }
    }
}

impl From<SetStep> for Step {
    fn from(step: SetStep) -> Self {
        match step {
            SetStep::Complete(action) => Self::SetComplete(action),
            SetStep::Continue { resume } => Self::SetContinue(resume),
            SetStep::Proxy {
                object,
                key,
                value,
                receiver,
                resume,
            } => Self::SetProxy {
                object,
                key,
                value,
                receiver,
                resume: Resume::OrdinarySet(resume),
            },
            SetStep::Special {
                object,
                key,
                value,
                receiver,
                resume,
            } => Self::SetSpecial {
                object,
                key,
                value,
                receiver,
                resume,
            },
            SetStep::ArrayLength { value, resume, .. } => Self::SetLength { value, resume },
            SetStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::OrdinarySet(resume),
            },
            SetStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::OrdinarySet(resume),
            },
        }
    }
}

impl From<ProxySetStep> for Step {
    fn from(step: ProxySetStep) -> Self {
        match step {
            ProxySetStep::Complete(result) => Self::SetComplete(set_completion(result)),
            ProxySetStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::ProxySet(resume),
            },
            ProxySetStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::ProxySet(resume),
            },
            ProxySetStep::Set {
                object,
                key,
                value,
                receiver,
                resume,
            } => Self::Set {
                object,
                key,
                value,
                receiver,
                resume: Resume::ProxySet(resume),
            },
            ProxySetStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::ProxySet(resume),
            },
        }
    }
}

impl From<ProxyDefineStep> for Step {
    fn from(step: ProxyDefineStep) -> Self {
        match step {
            ProxyDefineStep::Complete(result) => Self::Defined(result),
            ProxyDefineStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Define(resume),
            },
            ProxyDefineStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Define(resume),
            },
            ProxyDefineStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::Define {
                object,
                key,
                descriptor,
                resume: Resume::Define(resume),
            },
            ProxyDefineStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Define(resume),
            },
        }
    }
}

impl From<ArrayLengthStep> for Step {
    fn from(step: ArrayLengthStep) -> Self {
        match step {
            ArrayLengthStep::Complete(result) => Self::LengthComplete(result),
            ArrayLengthStep::Number { value, resume } => Self::Number {
                value,
                resume: Resume::LengthNumber(resume),
            },
        }
    }
}

impl From<ProxyPrototypeStep> for Step {
    fn from(step: ProxyPrototypeStep) -> Self {
        match step {
            ProxyPrototypeStep::Complete(result) => Self::Complete(result),
            ProxyPrototypeStep::Read {
                object,
                key,
                receiver,
                resume,
            } => Self::Read {
                object,
                key,
                receiver,
                resume: Resume::Prototype(resume),
            },
            ProxyPrototypeStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Prototype(resume),
            },
            ProxyPrototypeStep::Get { object, resume } => Self::GetPrototype {
                object,
                resume: Resume::Prototype(resume),
            },
            ProxyPrototypeStep::Set {
                object,
                prototype,
                resume,
            } => Self::SetPrototype {
                object,
                prototype,
                resume: Resume::Prototype(resume),
            },
            ProxyPrototypeStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Prototype(resume),
            },
        }
    }
}

impl From<crate::engine::object::KeysStep> for Step {
    fn from(step: crate::engine::object::KeysStep) -> Self {
        use crate::engine::object::KeysStep;
        match step {
            KeysStep::Complete(result) => Self::KeysComplete(result),
            KeysStep::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Keys(resume),
            },
            KeysStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::Keys(resume),
            },
            KeysStep::Number { value, resume } => Self::Number {
                value,
                resume: Resume::Keys(resume),
            },
            KeysStep::Keys { object, resume } => Self::Keys {
                object,
                resume: Resume::Keys(resume),
            },
            KeysStep::Extensible { object, resume } => Self::Extensible {
                object,
                resume: Resume::Keys(resume),
            },
            KeysStep::Descriptor {
                object,
                key,
                resume,
            } => Self::Descriptor {
                object,
                key,
                resume: Resume::Keys(resume),
            },
        }
    }
}

impl From<crate::engine::object::ProxyConstructStep> for Step {
    fn from(step: crate::engine::object::ProxyConstructStep) -> Self {
        use crate::engine::object::ProxyConstructStep;
        match step {
            ProxyConstructStep::Complete(result) => Self::Complete(result),
            ProxyConstructStep::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::ProxyConstruct(resume),
            },
            ProxyConstructStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::ProxyConstruct(resume),
            },
            ProxyConstructStep::Construct {
                target,
                new_target,
                arguments,
                resume,
            } => Self::Construct {
                target,
                new_target,
                arguments,
                resume: Resume::ProxyConstruct(resume),
            },
        }
    }
}

impl From<crate::engine::object::object_literal::element::LiteralDefinitionStep> for Step {
    fn from(step: crate::engine::object::object_literal::element::LiteralDefinitionStep) -> Self {
        use crate::engine::object::object_literal::element::LiteralDefinitionStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::LiteralDefinition(resume),
            },
            T::Define {
                object,
                key,
                descriptor,
                resume,
            } => Self::DefineOrdinary {
                object,
                key,
                descriptor,
                resume: Resume::LiteralDefinition(resume),
            },
        }
    }
}
