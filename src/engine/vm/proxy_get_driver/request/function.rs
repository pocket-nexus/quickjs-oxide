//! Mechanical adapters for function domain requests.
use super::{JsValue, Resume, Step};

step_from! {
    crate::engine::builtins::ArgumentsStep => Arguments;
    Read { object, key } => read;
    Number => Number { value: take_number_value };
    raw {
        T::Complete(result) => Self::ArgumentsComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::InvokeStep => Invoke;
    Arguments => Arguments { value: take_arguments_value };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Construct(request) => {
            let target = request.target;
            let new_target = request.new_target;
            let arguments = request.arguments;
            Self::Construct {
                target: Some(target),
                new_target: Some(new_target),
                arguments: Some(arguments),
                resume: Some(Resume::Identity),
            }
        }
        T::Call(request) => {
            let target = request.target;
            let receiver = request.receiver;
            let arguments = request.arguments;
            Self::Call {
                target: Some(target),
                receiver: Some(receiver),
                arguments: Some(arguments),
                resume: Some(Resume::Identity),
            }
        }
    }
}

step_from! {
    crate::engine::builtins::InstanceStep => Instance;
    Read => read(take_read_object, take_read_key);
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Prototype => GetPrototype { object: take_prototype_object };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::vm::call::prototype::ProtoSourceStep => ConstructorSource;
    raw {
        T::Complete(result) => Self::ConstructorSourceComplete(Some(result)),
        T::ReadValue { key, mut resume } => Self::ReadValue {
            receiver: Some(resume.take_read_receiver()),
            key: Some(key),
            resume: Some(Resume::ConstructorSource(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::BindStep => Bind;
    Read { object, key } => read;
    Own { object, key } => OwnFlag { enumerable: false };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::FunctionTextStep => FunctionText;
    Read => read(take_read_object, take_read_key);
    String => String { value: take_string_value };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DynamicFunctionStep => DynamicFunction;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    String => String { value: take_string_value };
    Eval => IndirectEval { source: take_eval_source };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::WeakConstructorStep => WeakConstructor;
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
