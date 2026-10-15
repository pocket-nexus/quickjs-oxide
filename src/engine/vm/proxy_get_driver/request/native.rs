//! Mechanical adapters for native domain requests.
use super::{Resume, Step};

step_from! {
    crate::engine::builtins::continuation::NativeStep => JsonRaw;
    JsonRaw { value } => String;
    raw {
        T::ModuleCallback(step) => step.try_into()?,
        #[cfg(feature = "test262-host")]
        T::Test262Agent(step) => step.try_into()?,
        #[cfg(feature = "test262-host")]
        T::EvalScript(step) => step.try_into()?,
        T::Async(step) => step.into(),
        T::FromSync(step) => step.try_into()?,
        T::AsyncGenerator(step) => step.into(),
        T::Promise(step) => step.try_into()?,
        T::GeneratorResume(step) => step.try_into()?,
        T::Atomics(step) => step.try_into()?,
        T::TypedCreate(step) => step.try_into()?,
        T::BufferSlice(step) => step.try_into()?,
        T::TypedWith(step) => step.try_into()?,
        T::Uint8Codec(step) => step.try_into()?,
        T::TypedSearch(step) => step.try_into()?,
        T::TypedString(step) => step.try_into()?,
        T::TypedSlice(step) => step.try_into()?,
        T::TypedMutation(step) => step.try_into()?,
        T::StringFactory(step) => step.try_into()?,
        T::WeakConstructor(step) => step.try_into()?,
        T::RegExpMatchAll(step) => step.try_into()?,
        T::RegExpSplit(step) => step.try_into()?,
        T::RegExpIterator(step) => step.try_into()?,
        T::ObjectConstructor(step) => step.try_into()?,
        T::Bind(step) => step.try_into()?,
        T::FunctionText(step) => step.try_into()?,
        T::DynamicFunction(step) => step.try_into()?,
        T::JsonParse(step) => step.try_into()?,
        T::JsonStringify(step) => step.try_into()?,
        T::BufferConstructor(step) => step.try_into()?,
        T::DataViewConstructor(step) => step.try_into()?,
        T::TypedSet(step) => step.try_into()?,
        T::RegExpConstructor(step) => step.try_into()?,
        T::RegExpSearch(step) => step.try_into()?,
        T::RegExpMatch(step) => step.try_into()?,
        T::RegExpCompile(step) => step.try_into()?,
        T::StringProtocol(step) => step.try_into()?,
        T::GlobalEval(source) => Self::IndirectEval {
            source: Some(source),
            resume: Some(Resume::Identity),
        },
        T::TypedSort(step) => step.try_into()?,
        T::Math(step) => step.try_into()?,
        T::Sum(step) => step.try_into()?,
        T::PrimitiveConstructor(step) => step.try_into()?,
        T::Global(step) => step.try_into()?,
        T::Numeric(step) => step.try_into()?,
        T::ScalarText(step) => step.try_into()?,
        T::DateConstructor(step) => step.try_into()?,
        T::DatePrototype(step) => step.try_into()?,
        T::Error(step) => step.try_into()?,
        T::MapCallback(step) => step.try_into()?,
        T::SetEach(step) => step.try_into()?,
        T::SetOperation(step) => step.try_into()?,
        T::Collection(step) => step.try_into()?,
        T::WeakComputed(step) => step.try_into()?,

        T::ArrayConstructor(step) => step.try_into()?,
        T::ArraySlice(step) => step.try_into()?,
        T::IteratorConstructor(step) => step.try_into()?,
        T::IteratorTag(step) => step.try_into()?,
        T::TypedTraversal(step) => step.try_into()?,
        T::TypedIteration(step) => step.try_into()?,
        T::ArrayConcat(step) => step.try_into()?,
        T::ArrayFlatten(step) => step.try_into()?,
        T::StringText(step) => step.try_into()?,
        T::StringSearch(step) => step.try_into()?,
        T::StringSplit(step) => step.try_into()?,
        T::Instance(step) => step.try_into()?,
        T::IteratorFrom(step) => step.try_into()?,
        T::IteratorWrap(step) => step.try_into()?,
        T::IteratorConcat(step) => step.try_into()?,
        T::ArrayBuild(step) => step.try_into()?,
        T::ArraySort(step) => step.try_into()?,
        T::ArrayIndexed(step) => step.try_into()?,
        T::ArrayReverse(step) => step.try_into()?,
        T::ArrayString(step) => step.try_into()?,
        T::RegExpExec(step) => step.try_into()?,
        T::RegExpPresentation(step) => step.try_into()?,
        T::RegExpReplace(step) => step.try_into()?,
        T::IteratorConsume(step) => step.try_into()?,
        T::IteratorHelper(step) => step.try_into()?,
        T::IteratorCreate(step) => step.try_into()?,
        T::ArrayNext(step) => step.try_into()?,
        T::Raw(result) => Self::NativeRawComplete(Some(result)),
        T::ArrayMutation(step) => step.try_into()?,
        T::ArrayCallback(step) => step.try_into()?,
        T::ObjectIteration(step) => step.try_into()?,
        T::StringReplace(step) => step.try_into()?,
        T::DataView(step) => step.try_into()?,
        T::BufferMutation(step) => step.try_into()?,
        T::Invoke(step) => step.try_into()?,
        T::Prototype(step) => step.try_into()?,
        T::Property(step) => step.try_into()?,
        T::String(step) => step.try_into()?,
        T::Complete(result) => Self::Complete(Some(result)),
        T::Definitions(step) => step.try_into()?,
        T::Predicate(step) => step.try_into()?,
    }
}

step_from! {
    crate::engine::vm::generator::GeneratorStep => Generator;
    Run { activation, input } => ResumeFrame;
    raw {
        T::Complete(outcome) => {
            Self::NativeRawComplete(Some(outcome))
        }
    }
}

step_from! {
    crate::engine::builtins::promise::operation::PromiseStep => Promise;
    Nested => PromiseOperation { step: take_nested_step };
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Close => IteratorCloseWithResume { iterator: take_close_iterator,
        completion: take_close_completion };
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Construct => construct(take_construct_target, take_construct_arguments);
    Prototype => ConstructorSource { new_target: take_prototype_new_target };
    raw {
        T::Complete(completion) => Self::Complete(Some(completion)),
    }
}

step_from! {
    crate::engine::vm::async_from_sync_iterator::FromSyncStep => FromSync;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Resolve => IntrinsicPromiseResolve { value: take_resolve_value, realm: take_resolve_realm };
    Close => IteratorCloseWithResume { iterator: take_close_iterator,
        completion: take_close_completion };
    raw {
        T::Complete(completion) => Self::Complete(Some(completion)),
    }
}

step_from! {
    #[cfg(feature = "test262-host")]
    crate::engine::api::test262_host::operation::EvalScriptStep => EvalScript;
    String { value } => String;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call { callable, receiver } => Self::Call {
            target: Some(crate::engine::vm::call::DirectCallTarget::Callable(
                callable,
            )),
            receiver: Some(receiver),
            arguments: Some(Vec::new()),
            resume: Some(Resume::Identity),
        },
    }
}

step_from! {
    #[cfg(feature = "test262-host")]
    crate::engine::api::test262_agent::operation::AgentStep => Test262Agent;
    String { value } => String;
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
