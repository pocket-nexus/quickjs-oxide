//! Mechanical adapters for native domain requests.
use super::{Resume, Step};

impl TryFrom<crate::engine::builtins::continuation::NativeStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::continuation::NativeStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::continuation::NativeStep;
            match step {
                NativeStep::ModuleCallback(step) => step.try_into()?,
                #[cfg(feature = "test262-host")]
                NativeStep::Test262Agent(step) => step.try_into()?,
                #[cfg(feature = "test262-host")]
                NativeStep::EvalScript(step) => step.try_into()?,
                NativeStep::Async(step) => step.into(),
                NativeStep::FromSync(step) => step.try_into()?,
                NativeStep::AsyncGenerator(step) => step.into(),
                NativeStep::Promise(step) => step.try_into()?,
                NativeStep::GeneratorResume(step) => step.try_into()?,
                NativeStep::Atomics(step) => step.try_into()?,
                NativeStep::TypedCreate(step) => step.try_into()?,
                NativeStep::BufferSlice(step) => step.try_into()?,
                NativeStep::TypedWith(step) => step.try_into()?,
                NativeStep::Uint8Codec(step) => step.try_into()?,
                NativeStep::TypedSearch(step) => step.try_into()?,
                NativeStep::TypedString(step) => step.try_into()?,
                NativeStep::TypedSlice(step) => step.try_into()?,
                NativeStep::TypedMutation(step) => step.try_into()?,
                NativeStep::StringFactory(step) => step.try_into()?,
                NativeStep::WeakConstructor(step) => step.try_into()?,
                NativeStep::RegExpMatchAll(step) => step.try_into()?,
                NativeStep::RegExpSplit(step) => step.try_into()?,
                NativeStep::RegExpIterator(step) => step.try_into()?,
                NativeStep::ObjectConstructor(step) => step.try_into()?,
                NativeStep::Bind(step) => step.try_into()?,
                NativeStep::FunctionText(step) => step.try_into()?,
                NativeStep::DynamicFunction(step) => step.try_into()?,
                NativeStep::JsonParse(step) => step.try_into()?,
                NativeStep::JsonStringify(step) => step.try_into()?,
                NativeStep::BufferConstructor(step) => step.try_into()?,
                NativeStep::DataViewConstructor(step) => step.try_into()?,
                NativeStep::TypedSet(step) => step.try_into()?,
                NativeStep::RegExpConstructor(step) => step.try_into()?,
                NativeStep::RegExpSearch(step) => step.try_into()?,
                NativeStep::RegExpMatch(step) => step.try_into()?,
                NativeStep::RegExpCompile(step) => step.try_into()?,
                NativeStep::StringProtocol(step) => step.try_into()?,
                NativeStep::GlobalEval(source) => Self::IndirectEval {
                    source: Some(source),
                    resume: Some(Resume::Identity),
                },
                NativeStep::JsonRaw { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::JsonRaw(resume)),
                },

                NativeStep::TypedSort(step) => step.try_into()?,
                NativeStep::Math(step) => step.try_into()?,
                NativeStep::Sum(step) => step.try_into()?,
                NativeStep::PrimitiveConstructor(step) => step.try_into()?,
                NativeStep::Global(step) => step.try_into()?,
                NativeStep::Numeric(step) => step.try_into()?,
                NativeStep::ScalarText(step) => step.try_into()?,
                NativeStep::DateConstructor(step) => step.try_into()?,
                NativeStep::DatePrototype(step) => step.try_into()?,
                NativeStep::Error(step) => step.try_into()?,
                NativeStep::MapCallback(step) => step.try_into()?,
                NativeStep::SetEach(step) => step.try_into()?,
                NativeStep::SetOperation(step) => step.try_into()?,
                NativeStep::Collection(step) => step.try_into()?,
                NativeStep::WeakComputed(step) => step.try_into()?,

                NativeStep::ArrayConstructor(step) => step.try_into()?,
                NativeStep::ArraySlice(step) => step.try_into()?,
                NativeStep::IteratorConstructor(step) => step.try_into()?,
                NativeStep::IteratorTag(step) => step.try_into()?,
                NativeStep::TypedTraversal(step) => step.try_into()?,
                NativeStep::TypedIteration(step) => step.try_into()?,
                NativeStep::ArrayConcat(step) => step.try_into()?,
                NativeStep::ArrayFlatten(step) => step.try_into()?,
                NativeStep::StringText(step) => step.try_into()?,
                NativeStep::StringSearch(step) => step.try_into()?,
                NativeStep::StringSplit(step) => step.try_into()?,
                NativeStep::Instance(step) => step.try_into()?,
                NativeStep::IteratorFrom(step) => step.try_into()?,
                NativeStep::IteratorWrap(step) => step.try_into()?,
                NativeStep::IteratorConcat(step) => step.try_into()?,
                NativeStep::ArrayBuild(step) => step.try_into()?,
                NativeStep::ArraySort(step) => step.try_into()?,
                NativeStep::ArrayIndexed(step) => step.try_into()?,
                NativeStep::ArrayReverse(step) => step.try_into()?,
                NativeStep::ArrayString(step) => step.try_into()?,
                NativeStep::RegExpExec(step) => step.try_into()?,
                NativeStep::RegExpPresentation(step) => step.try_into()?,
                NativeStep::RegExpReplace(step) => step.try_into()?,
                NativeStep::IteratorConsume(step) => step.try_into()?,
                NativeStep::IteratorHelper(step) => step.try_into()?,
                NativeStep::IteratorCreate(step) => step.try_into()?,
                NativeStep::ArrayNext(step) => step.try_into()?,
                NativeStep::Raw(result) => Self::NativeRawComplete(Some(result)),
                NativeStep::ArrayMutation(step) => step.try_into()?,
                NativeStep::ArrayCallback(step) => step.try_into()?,
                NativeStep::ObjectIteration(step) => step.try_into()?,
                NativeStep::StringReplace(step) => step.try_into()?,
                NativeStep::DataView(step) => step.try_into()?,
                NativeStep::BufferMutation(step) => step.try_into()?,
                NativeStep::Invoke(step) => step.try_into()?,
                NativeStep::Prototype(step) => step.try_into()?,
                NativeStep::Property(step) => step.try_into()?,
                NativeStep::String(step) => step.try_into()?,
                NativeStep::Complete(result) => Self::Complete(Some(result)),
                NativeStep::CyclePublishedComplete(result) => {
                    Self::CyclePublishedComplete(Some(result))
                }
                NativeStep::Definitions(step) => step.try_into()?,
                NativeStep::Predicate(step) => step.try_into()?,
            }
        })
    }
}

impl TryFrom<crate::engine::vm::generator::GeneratorStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(step: crate::engine::vm::generator::GeneratorStep) -> Result<Self, Self::Error> {
        Ok({
            match step {
                crate::engine::vm::generator::GeneratorStep::Complete(outcome) => {
                    Self::NativeRawComplete(Some(outcome))
                }
                crate::engine::vm::generator::GeneratorStep::Run {
                    activation,
                    input,
                    resume,
                } => Self::ResumeFrame {
                    activation: Some(activation),
                    input: Some(input),
                    resume: Some(Resume::Generator(resume)),
                },
            }
        })
    }
}

impl TryFrom<crate::engine::builtins::promise::operation::PromiseStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::builtins::promise::operation::PromiseStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::builtins::promise::operation::PromiseStep as P;
            match step {
                P::Nested { mut resume } => {
                    let step = resume.take_nested_step();
                    Self::PromiseOperation {
                        step: Some(step),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Next { mut resume } => {
                    let iterator = resume.take_next_iterator();
                    let method = resume.take_next_method();
                    Self::IteratorNext {
                        iterator: Some(iterator),
                        method: Some(method),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Close { mut resume } => {
                    let iterator = resume.take_close_iterator();
                    let completion = resume.take_close_completion();
                    Self::IteratorCloseWithResume {
                        iterator: Some(iterator),
                        completion: Some(completion),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Complete(completion) => Self::Complete(Some(completion)),
                P::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Call { mut resume } => {
                    let callable = resume.take_call_callable();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(super::DirectCallTarget::Callable(callable)),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Construct { mut resume } => {
                    let target = resume.take_construct_target();
                    let new_target = target.try_clone()?;
                    let arguments = resume.take_construct_arguments();
                    Self::Construct {
                        new_target: Some(crate::engine::vm::call::ConstructNewTarget::Validated(
                            new_target,
                        )),
                        target: Some(target),
                        arguments: Some(arguments),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
                P::Prototype { mut resume } => {
                    let new_target = resume.take_prototype_new_target();
                    Self::ConstructorSource {
                        new_target: Some(new_target),
                        resume: Some(Resume::Promise(resume)),
                    }
                }
            }
        })
    }
}

impl TryFrom<crate::engine::vm::async_from_sync_iterator::FromSyncStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::vm::async_from_sync_iterator::FromSyncStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::vm::async_from_sync_iterator::FromSyncStep as S;
            match step {
                S::Complete(completion) => Self::Complete(Some(completion)),
                S::Read { mut resume } => {
                    let receiver = resume.take_read_receiver();
                    let key = resume.take_read_key();
                    Self::ReadValue {
                        receiver: Some(receiver),
                        key: Some(key),
                        resume: Some(Resume::FromSync(resume)),
                    }
                }
                S::Call { mut resume } => {
                    let callable = resume.take_call_callable();
                    let receiver = resume.take_call_receiver();
                    let arguments = resume.take_call_arguments();
                    Self::Call {
                        target: Some(super::DirectCallTarget::Callable(callable)),
                        receiver: Some(receiver),
                        arguments: Some(arguments),
                        resume: Some(Resume::FromSync(resume)),
                    }
                }
                S::Resolve { mut resume } => {
                    let value = resume.take_resolve_value();
                    let realm = resume.take_resolve_realm();
                    Self::IntrinsicPromiseResolve {
                        value: Some(value),
                        realm: Some(realm),
                        resume: Some(Resume::FromSync(resume)),
                    }
                }
                S::Close { mut resume } => {
                    let iterator = resume.take_close_iterator();
                    let completion = resume.take_close_completion();
                    Self::IteratorCloseWithResume {
                        iterator: Some(iterator),
                        completion: Some(completion),
                        resume: Some(Resume::FromSync(resume)),
                    }
                }
            }
        })
    }
}

#[cfg(feature = "test262-host")]
impl TryFrom<crate::engine::api::test262_host::operation::EvalScriptStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::api::test262_host::operation::EvalScriptStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::api::test262_host::operation::EvalScriptStep;
            match step {
                EvalScriptStep::Complete(result) => Self::Complete(Some(result)),
                EvalScriptStep::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::EvalScript(resume)),
                },
                EvalScriptStep::Call { callable, receiver } => Self::Call {
                    target: Some(crate::engine::vm::call::DirectCallTarget::Callable(
                        callable,
                    )),
                    receiver: Some(receiver),
                    arguments: Some(Vec::new()),
                    resume: Some(Resume::Identity),
                },
            }
        })
    }
}

#[cfg(feature = "test262-host")]
impl TryFrom<crate::engine::api::test262_agent::operation::AgentStep> for Step {
    type Error = crate::engine::api::RuntimeError;
    fn try_from(
        step: crate::engine::api::test262_agent::operation::AgentStep,
    ) -> Result<Self, Self::Error> {
        Ok({
            use crate::engine::api::test262_agent::operation::AgentStep;
            match step {
                AgentStep::Complete(result) => Self::Complete(Some(result)),
                AgentStep::String { value, resume } => Self::String {
                    value: Some(value),
                    resume: Some(Resume::Test262Agent(resume)),
                },
                AgentStep::Number { value, resume } => Self::Number {
                    value: Some(value),
                    resume: Some(Resume::Test262Agent(resume)),
                },
            }
        })
    }
}
