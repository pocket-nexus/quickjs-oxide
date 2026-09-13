//! Mechanical adapters for native domain requests.
use super::{Completion, Resume, Step, Value};

impl From<crate::engine::builtins::continuation::NativeStep> for Step {
    fn from(step: crate::engine::builtins::continuation::NativeStep) -> Self {
        use crate::engine::builtins::continuation::NativeStep;
        match step {
            NativeStep::Async(step) => step.into(),
            NativeStep::FromSync(step) => step.into(),
            NativeStep::AsyncGenerator(step) => step.into(),
            NativeStep::Promise(step) => step.into(),
            NativeStep::GeneratorResume(step) => step.into(),
            NativeStep::Atomics(step) => step.into(),
            NativeStep::TypedCreate(step) => step.into(),
            NativeStep::BufferSlice(step) => step.into(),
            NativeStep::TypedWith(step) => step.into(),
            NativeStep::Uint8Codec(step) => step.into(),
            NativeStep::TypedSearch(step) => step.into(),
            NativeStep::TypedString(step) => step.into(),
            NativeStep::TypedSlice(step) => step.into(),
            NativeStep::TypedMutation(step) => step.into(),
            NativeStep::StringFactory(step) => step.into(),
            NativeStep::WeakConstructor(step) => step.into(),
            NativeStep::RegExpMatchAll(step) => step.into(),
            NativeStep::RegExpSplit(step) => step.into(),
            NativeStep::RegExpIterator(step) => step.into(),
            NativeStep::ObjectConstructor(step) => step.into(),
            NativeStep::Bind(step) => step.into(),
            NativeStep::FunctionText(step) => step.into(),
            NativeStep::DynamicFunction(step) => step.into(),
            NativeStep::JsonParse(step) => step.into(),
            NativeStep::JsonStringify(step) => step.into(),
            NativeStep::BufferConstructor(step) => step.into(),
            NativeStep::DataViewConstructor(step) => step.into(),
            NativeStep::TypedSet(step) => step.into(),
            NativeStep::RegExpConstructor(step) => step.into(),
            NativeStep::RegExpSearch(step) => step.into(),
            NativeStep::RegExpMatch(step) => step.into(),
            NativeStep::RegExpCompile(step) => step.into(),
            NativeStep::StringProtocol(step) => step.into(),
            NativeStep::GlobalEval(input) => match input {
                Value::String(source) => Self::IndirectEval {
                    source,
                    resume: Resume::Identity,
                },
                input => Self::Complete(Completion::Return(input)),
            },
            NativeStep::JsonRaw { value, resume } => Self::String {
                value,
                resume: Resume::JsonRaw(resume),
            },

            NativeStep::TypedSort(step) => step.into(),
            NativeStep::Math(step) => step.into(),
            NativeStep::Sum(step) => step.into(),
            NativeStep::PrimitiveConstructor(step) => step.into(),
            NativeStep::Global(step) => step.into(),
            NativeStep::Numeric(step) => step.into(),
            NativeStep::ScalarText(step) => step.into(),
            NativeStep::DateConstructor(step) => step.into(),
            NativeStep::DatePrototype(step) => step.into(),
            NativeStep::Error(step) => step.into(),
            NativeStep::MapCallback(step) => step.into(),
            NativeStep::SetEach(step) => step.into(),
            NativeStep::SetOperation(step) => step.into(),
            NativeStep::Collection(step) => step.into(),
            NativeStep::WeakComputed(step) => step.into(),

            NativeStep::ArrayConstructor(step) => step.into(),
            NativeStep::ArraySlice(step) => step.into(),
            NativeStep::IteratorConstructor(step) => step.into(),
            NativeStep::IteratorTag(step) => step.into(),
            NativeStep::TypedTraversal(step) => step.into(),
            NativeStep::TypedIteration(step) => step.into(),
            NativeStep::ArrayConcat(step) => step.into(),
            NativeStep::ArrayFlatten(step) => step.into(),
            NativeStep::StringText(step) => step.into(),
            NativeStep::StringSearch(step) => step.into(),
            NativeStep::StringSplit(step) => step.into(),
            NativeStep::Instance(step) => step.into(),
            NativeStep::IteratorFrom(step) => step.into(),
            NativeStep::IteratorWrap(step) => step.into(),
            NativeStep::IteratorConcat(step) => step.into(),
            NativeStep::ArrayBuild(step) => step.into(),
            NativeStep::ArraySort(step) => step.into(),
            NativeStep::ArrayIndexed(step) => step.into(),
            NativeStep::ArrayReverse(step) => step.into(),
            NativeStep::ArrayString(step) => step.into(),
            NativeStep::RegExpExec(step) => step.into(),
            NativeStep::RegExpPresentation(step) => step.into(),
            NativeStep::RegExpReplace(step) => step.into(),
            NativeStep::IteratorConsume(step) => step.into(),
            NativeStep::IteratorHelper(step) => step.into(),
            NativeStep::IteratorCreate(step) => step.into(),
            NativeStep::ArrayNext(step) => step.into(),
            NativeStep::Raw(result) => Self::NativeRawComplete(result),
            NativeStep::ArrayMutation(step) => step.into(),
            NativeStep::ArrayCallback(step) => step.into(),
            NativeStep::ObjectIteration(step) => step.into(),
            NativeStep::StringReplace(step) => step.into(),
            NativeStep::DataView(step) => step.into(),
            NativeStep::BufferMutation(step) => step.into(),
            NativeStep::Invoke(step) => step.into(),
            NativeStep::Prototype(step) => step.into(),
            NativeStep::Property(step) => step.into(),
            NativeStep::String(step) => step.into(),
            NativeStep::Complete(result) => Self::Complete(result),
            NativeStep::Definitions(step) => step.into(),
            NativeStep::Predicate(step) => step.into(),
        }
    }
}

impl From<crate::engine::vm::generator::GeneratorStep> for Step {
    fn from(step: crate::engine::vm::generator::GeneratorStep) -> Self {
        match step {
            crate::engine::vm::generator::GeneratorStep::Complete(outcome) => {
                Self::NativeRawComplete(outcome)
            }
            crate::engine::vm::generator::GeneratorStep::Run {
                activation,
                input,
                resume,
            } => Self::ResumeFrame {
                activation,
                input,
                resume: Resume::Generator(resume),
            },
        }
    }
}

impl From<crate::engine::builtins::promise::operation::PromiseStep> for Step {
    fn from(step: crate::engine::builtins::promise::operation::PromiseStep) -> Self {
        use crate::engine::builtins::promise::operation::PromiseStep as P;
        match step {
            P::Nested { step, resume } => Self::PromiseOperation {
                step,
                resume: Resume::Promise(resume),
            },
            P::Next {
                iterator,
                method,
                resume,
            } => Self::IteratorNext {
                iterator,
                method,
                resume: Resume::Promise(resume),
            },
            P::Close {
                iterator,
                completion,
                resume,
            } => Self::IteratorCloseWithResume {
                iterator,
                completion,
                resume: Resume::Promise(resume),
            },
            P::Complete(completion) => Self::Complete(completion),
            P::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::Promise(resume),
            },
            P::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: super::DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::Promise(resume),
            },
            P::Construct {
                target,
                arguments,
                resume,
            } => Self::Construct {
                new_target: crate::engine::vm::call::ConstructNewTarget::Validated(target.clone()),
                target,
                arguments,
                resume: Resume::Promise(resume),
            },
            P::Prototype { new_target, resume } => Self::ConstructorSource {
                new_target,
                resume: Resume::Promise(resume),
            },
        }
    }
}

impl From<crate::engine::vm::async_from_sync_iterator::FromSyncStep> for Step {
    fn from(step: crate::engine::vm::async_from_sync_iterator::FromSyncStep) -> Self {
        use crate::engine::vm::async_from_sync_iterator::FromSyncStep as S;
        match step {
            S::Complete(completion) => Self::Complete(completion),
            S::Read {
                receiver,
                key,
                resume,
            } => Self::ReadValue {
                receiver,
                key,
                resume: Resume::FromSync(resume),
            },
            S::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => Self::Call {
                target: super::DirectCallTarget::Callable(callable),
                receiver,
                arguments,
                resume: Resume::FromSync(resume),
            },
            S::Resolve {
                value,
                realm,
                resume,
            } => Self::IntrinsicPromiseResolve {
                value,
                realm,
                resume: Resume::FromSync(resume),
            },
            S::Close {
                iterator,
                completion,
                resume,
            } => Self::IteratorCloseWithResume {
                iterator,
                completion,
                resume: Resume::FromSync(resume),
            },
        }
    }
}
