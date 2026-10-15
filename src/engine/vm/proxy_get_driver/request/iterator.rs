//! Mechanical adapters for iterator domain requests.
use super::JsValue;
use super::{Completion, DirectCallTarget, Resume, Step};

step_from! {
    crate::engine::builtins::IteratorCloseStep => IteratorClose;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            iterator,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Object(iterator.into_handle())),
            arguments: Some(Vec::new()),
            resume: Some(Resume::IteratorClose(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::IteratorNextStep => IteratorNext;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::IteratorNextComplete(Some(result)),
        T::Call {
            callable,
            iterator,
            resume,
        } => Self::IteratorCall {
            callable: Some(callable),
            iterator: Some(iterator),
            resume: Some(resume),
        },
    }
}

step_from! {
    crate::engine::builtins::IteratorConsumeStep => IteratorConsume;
    Read => read(take_read_object, take_read_key);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Call => Call { target: take_call_callable as DirectCallTarget::Callable,
        arguments: take_call_arguments; receiver: JsValue::Undefined };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Close {
            iterator,
            completion,
        } => Self::IteratorClose {
            iterator: Some(iterator),
            completion: Some(completion),
        },
    }
}

step_from! {
    crate::engine::builtins::IteratorHelperStep => IteratorHelper;
    Read => read(take_read_object, take_read_key);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Close => IteratorCloseWithResume { iterator: take_close_iterator,
        completion: take_close_completion };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::IteratorCreateStep => IteratorCreate;
    CloseInvalidCount { iterator } => IteratorCloseWithResume {
        completion: Completion::Throw(JsValue::Undefined) } @ IteratorInvalidCount;
    Read { object, key } => read;
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Close {
            iterator,
            completion,
        } => Self::IteratorClose {
            iterator: Some(iterator),
            completion: Some(completion),
        },
    }
}

step_from! {
    crate::engine::builtins::IteratorFromStep => IteratorFrom;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver);
    Instance => OrdinaryInstance { constructor: take_instance_constructor,
        value: take_instance_value };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::IteratorWrapStep => IteratorWrap;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Parse => ParseIterator { result: take_parse_result };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::IteratorConcatStep => IteratorConcat;
    Read => read(take_read_object, take_read_key);
    Call => call(take_call_callable, take_call_receiver);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    raw {
        T::Complete(result) => Self::NativeRawComplete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::IteratorConstructorStep => IteratorConstructor;
    Prototype { new_target } => ConstructorSource;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::IteratorTagStep => IteratorTag;
    Own => OwnFlag { object: take_own_object, key: take_own_key; enumerable: false };
    Define => Define { object: take_define_object, key: take_define_key,
        descriptor: take_define_descriptor as Into::into };
    Set => set(take_set_object, take_set_key, take_set_value);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::MapCallbackStep => MapCallback;
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::SetEachStep => SetEach;
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::WeakComputedStep => WeakComputed;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            arguments,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(JsValue::Undefined),
            arguments: Some(arguments),
            resume: Some(Resume::WeakComputed(resume)),
        },
    }
}

step_from! {
    crate::engine::builtins::SetOperationStep => SetOperation;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Number => Number { value: take_number_value };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Parse => ParseIterator { result: take_parse_result };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::CollectionStep => Collection;
    Prototype => ConstructorSource { new_target: take_prototype_new_target };
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_method };
    Close => IteratorCloseWithResume { iterator: take_close_iterator,
        completion: take_close_completion };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
