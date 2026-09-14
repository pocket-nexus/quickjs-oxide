//! Mechanical adapters for string domain requests.
use super::{Resume, Step, Value};

impl From<crate::engine::builtins::StringReplaceStep> for Step {
    fn from(step: crate::engine::builtins::StringReplaceStep) -> Self {
        use crate::engine::builtins::StringReplaceStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::PreparedRead { read, key, resume } => Self::PreparedRead {
                read,
                key,
                resume: Resume::StringReplace(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::StringReplace(resume),
            },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::StringReplace(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target,
                receiver,
                arguments,
                resume: Resume::StringReplace(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpExecStep> for Step {
    fn from(step: crate::engine::builtins::RegExpExecStep) -> Self {
        use crate::engine::builtins::RegExpExecStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::RegExpExec(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::RegExpExec(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::RegExpExec(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpPresentationStep> for Step {
    fn from(step: crate::engine::builtins::RegExpPresentationStep) -> Self {
        use crate::engine::builtins::RegExpPresentationStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpPresentation(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::RegExpPresentation(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpReplaceStep> for Step {
    fn from(step: crate::engine::builtins::RegExpReplaceStep) -> Self {
        use crate::engine::builtins::RegExpReplaceStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::PreparedSet { step, resume } => Self::PreparedSet {
                step,
                resume: Resume::RegExpReplace(resume),
            },
            T::PreparedRead { read, key, resume } => Self::PreparedRead {
                read,
                key,
                resume: Resume::RegExpReplace(resume),
            },
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpReplace(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::RegExpReplace(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::RegExpReplace(resume),
            },
            T::Exec {
                regexp,
                input,
                resume,
            } => Self::RegExpExec {
                regexp,
                input,
                resume: Resume::RegExpReplace(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::RegExpReplace(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::StringTextStep> for Step {
    fn from(step: crate::engine::builtins::StringTextStep) -> Self {
        use crate::engine::builtins::StringTextStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::StringText(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::StringSearchStep> for Step {
    fn from(step: crate::engine::builtins::StringSearchStep) -> Self {
        use crate::engine::builtins::StringSearchStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::StringSearch(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::StringSearch(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::StringSplitStep> for Step {
    fn from(step: crate::engine::builtins::StringSplitStep) -> Self {
        use crate::engine::builtins::StringSplitStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::StringSplit(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::StringSplit(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::StringSplit(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpConstructorStep> for Step {
    fn from(step: crate::engine::builtins::RegExpConstructorStep) -> Self {
        use crate::engine::builtins::RegExpConstructorStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpConstructor(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::RegExpConstructor(resume),
            },
            T::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::RegExpConstructor(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpCompileStep> for Step {
    fn from(step: crate::engine::builtins::RegExpCompileStep) -> Self {
        use crate::engine::builtins::RegExpCompileStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::RegExpCompile(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::StringProtocolStep> for Step {
    fn from(step: crate::engine::builtins::StringProtocolStep) -> Self {
        use crate::engine::builtins::StringProtocolStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::StringProtocol(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::StringProtocol(resume),
            },
            T::Call {
                target,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: target,
                receiver: receiver,
                arguments: arguments,
                resume: Resume::StringProtocol(resume),
            },
            T::Construct {
                constructor,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(
                    constructor.clone(),
                ),
                target: constructor,
                arguments,
                resume: Resume::StringProtocol(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpSearchStep> for Step {
    fn from(step: crate::engine::builtins::RegExpSearchStep) -> Self {
        use crate::engine::builtins::RegExpSearchStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpSearch(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::String,
                resume: Resume::RegExpSearch(resume),
            },
            T::Exec {
                regexp,
                input,
                resume,
            } => Self::RegExpExec {
                regexp,
                input,
                resume: Resume::RegExpSearch(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::RegExpSearch(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpMatchStep> for Step {
    fn from(step: crate::engine::builtins::RegExpMatchStep) -> Self {
        use crate::engine::builtins::RegExpMatchStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpMatch(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::RegExpMatch(resume),
            },
            T::Exec {
                regexp,
                input,
                resume,
            } => Self::RegExpExec {
                regexp,
                input,
                resume: Resume::RegExpMatch(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::RegExpMatch(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpMatchAllStep> for Step {
    fn from(step: crate::engine::builtins::RegExpMatchAllStep) -> Self {
        use crate::engine::builtins::RegExpMatchAllStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpMatchAll(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::RegExpMatchAll(resume),
            },
            T::Species { regexp, resume } => Self::RegExpSpecies {
                regexp,
                resume: Resume::RegExpMatchAll(resume),
            },
            T::Construct {
                constructor,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(
                    constructor.clone(),
                ),
                target: constructor,
                arguments,
                resume: Resume::RegExpMatchAll(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::RegExpMatchAll(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpSplitStep> for Step {
    fn from(step: crate::engine::builtins::RegExpSplitStep) -> Self {
        use crate::engine::builtins::RegExpSplitStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpSplit(resume),
            },
            T::Primitive {
                value,
                hint,
                resume,
            } => Self::Primitive {
                value,
                hint,
                resume: Resume::RegExpSplit(resume),
            },
            T::Species { regexp, resume } => Self::RegExpSpecies {
                regexp,
                resume: Resume::RegExpSplit(resume),
            },
            T::Construct {
                constructor,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(
                    constructor.clone(),
                ),
                target: constructor,
                arguments,
                resume: Resume::RegExpSplit(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key,
                value,
                resume: Resume::RegExpSplit(resume),
            },
            T::Exec {
                regexp,
                input,
                resume,
            } => Self::RegExpExec {
                regexp,
                input,
                resume: Resume::RegExpSplit(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpSpeciesStep> for Step {
    fn from(step: crate::engine::builtins::RegExpSpeciesStep) -> Self {
        use crate::engine::builtins::RegExpSpeciesStep as T;
        match step {
            T::Complete(result) => Self::RegExpSpeciesComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpSpecies(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::RegExpIteratorStep> for Step {
    fn from(step: crate::engine::builtins::RegExpIteratorStep) -> Self {
        use crate::engine::builtins::RegExpIteratorStep as T;
        match step {
            T::Complete(result) => Self::NativeRawComplete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::RegExpIterator(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::RegExpIterator(resume),
            },
            T::Primitive { value, resume } => Self::Primitive {
                value,
                hint: crate::engine::vm::ToPrimitiveHint::Number,
                resume: Resume::RegExpIterator(resume),
            },
            T::Exec {
                regexp,
                input,
                resume,
            } => Self::RegExpExec {
                regexp,
                input,
                resume: Resume::RegExpIterator(resume),
            },
            T::Set {
                object,
                key,
                value,
                resume,
            } => Self::Set {
                receiver: Value::Object(object.clone()),
                object,
                key: key.clone(),
                value,
                resume: Resume::RegExpIteratorSet { key, resume },
            },
        }
    }
}

impl From<crate::engine::builtins::StringFactoryStep> for Step {
    fn from(step: crate::engine::builtins::StringFactoryStep) -> Self {
        use crate::engine::builtins::StringFactoryStep as T;
        match step {
            T::Complete(result) => Self::Complete(result),
            T::Read {
                object,
                key,
                resume,
            } => Self::Read {
                receiver: Value::Object(object.clone()),
                object,
                key,
                resume: Resume::StringFactory(resume),
            },
            T::Number { value, resume } => Self::Number {
                value,
                resume: Resume::StringFactory(resume),
            },
            T::String { value, resume } => Self::String {
                value,
                resume: Resume::StringFactory(resume),
            },
        }
    }
}
