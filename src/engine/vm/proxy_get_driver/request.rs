//! Typed replies connecting property domain states to the owned scheduler.
use super::*;
use crate::engine::object::operations::{
    InternalDefineResult, InternalSetResult, PropertySetAction,
};
use crate::engine::object::{
    ProxyDefineResume, ProxyDefineStep, ProxySetResume, ProxySetStep, SetResume, SetStep,
    set_completion,
};

pub(super) enum Resume {
    OrdinarySet(SetResume),
    ProxySet(ProxySetResume),
    Define(ProxyDefineResume),
    Setter,
    Get(ProxyGetResume),
    Call(crate::engine::object::ProxyCallResume),
    Own(ProxyOwnResume),
    Conversion(DescriptorResume),
    Boolean(ProxyBooleanResume),
}

pub(super) enum Step {
    SetComplete(PropertySetAction),
    SetContinue(SetResume),
    SetDeferred(SetStep),
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: Resume,
    },
    SetProxy {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: Resume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: Resume,
    },
    Defined(NativeConversion<InternalDefineResult>),
    Complete(Completion),
    BooleanComplete(NativeConversion<bool>),
    OwnComplete(NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>),
    Converted(NativeConversion<OrdinaryPropertyDescriptor>),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: Resume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Resume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: Resume,
    },
    Extensible {
        object: ObjectRef,
        resume: Resume,
    },
    Convert {
        value: Value,
        resume: ProxyOwnResume,
    },
}

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
            ProxyOwnStep::Convert { value, resume } => Self::Convert { value, resume },
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
            request @ (SetStep::Special { .. } | SetStep::ArrayLength { .. }) => {
                Self::SetDeferred(request)
            }
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

pub(super) fn set_result(
    action: PropertySetAction,
) -> Result<NativeConversion<InternalSetResult>, crate::engine::api::runtime_error::RuntimeError> {
    Ok(match action {
        PropertySetAction::Complete => NativeConversion::Value(InternalSetResult::Accepted),
        PropertySetAction::Rejected(reason) => {
            NativeConversion::Value(InternalSetResult::Rejected(reason))
        }
        PropertySetAction::RejectedProxyTrap => {
            NativeConversion::Value(InternalSetResult::RejectedProxyTrap)
        }
        PropertySetAction::Throw(value) => NativeConversion::Throw(value),
        PropertySetAction::Call { .. } => {
            return Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Set completed before its setter returned",
            ));
        }
    })
}

impl Resume {
    pub(super) fn set(
        self,
        _runtime: &Runtime,
        action: PropertySetAction,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::OrdinarySet(resume) => resume.forward(action).map(Into::into),
            Self::ProxySet(resume) => resume.set(set_result(action)?).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Set result has no matching continuation",
            )),
        }
    }
    pub(super) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::OrdinarySet(resume) => resume.defined(runtime, result).map(Into::into),
            Self::Define(resume) => resume.defined(result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Define result has no matching continuation",
            )),
        }
    }

    pub(super) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Own(resume) => resume.extensible(result).map(Into::into),
            Self::Boolean(resume) => resume.boolean(runtime, result).map(Into::into),
            Self::Conversion(resume) => resume.has(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "Proxy Get received a boolean reply",
            )),
        }
    }

    pub(super) fn resume(
        self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::ProxySet(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Define(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Setter => Ok(Step::SetComplete(match completion {
                Completion::Return(_) => PropertySetAction::Complete,
                Completion::Throw(value) => PropertySetAction::Throw(value),
            })),
            Self::OrdinarySet(_) => {
                Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                    "ordinary Set received an untyped reply",
                ))
            }
            Self::Get(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Call(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Own(resume) => resume.resume(runtime, completion).map(Into::into),
            Self::Conversion(resume) => resume.read(runtime, completion).map(Into::into),
            Self::Boolean(resume) => resume.resume(runtime, completion).map(Into::into),
        }
    }
    pub(super) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<Step, crate::engine::api::runtime_error::RuntimeError> {
        match self {
            Self::Get(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Own(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Boolean(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::OrdinarySet(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::ProxySet(resume) => resume.descriptor(runtime, result).map(Into::into),
            Self::Define(resume) => resume.descriptor(runtime, result).map(Into::into),
            _ => Err(crate::engine::api::runtime_error::RuntimeError::Invariant(
                "descriptor conversion received an own-property reply",
            )),
        }
    }
}
