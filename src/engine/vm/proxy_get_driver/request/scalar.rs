//! Mechanical adapters for scalar domain requests.
use super::JsValue;
use super::{DirectCallTarget, Resume, Step, ToPrimitiveHint};

step_from! {
    crate::engine::builtins::MathStep => Math;
    Number { value } => Number;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::GlobalStep => Global;
    Number { value } => Number;
    String { value } => String;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::NumericStep => Numeric;
    Number { value } => Number;
    Primitive { value } => Primitive { hint: ToPrimitiveHint::Number } @ NumericPrimitive;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::ScalarTextStep => ScalarText;
    Number { value } => Number;
    String { value } => String;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::PrimitiveConstructorStep => PrimitiveConstructor;
    String => String { value: take_string_value };
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Primitive => Primitive { value: take_primitive_value;
        hint: ToPrimitiveHint::Number } @ PrimitiveConstructorValue;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DateConstructorStep => DateConstructor;
    Number => Number { value: take_number_value };
    String => String { value: take_string_value };
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Primitive => Primitive { value: take_primitive_value;
        hint: ToPrimitiveHint::Default } @ DateConstructorPrimitive;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::DatePrototypeStep => DatePrototype;
    Number { value } => Number;
    Primitive { value, hint } => Primitive;
    Read { object, key } => read;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::OrdinaryPrimitive { object, hint } => Self::OrdinaryPrimitive {
            object: Some(object),
            hint: Some(hint),
        },
        T::Call { callable, receiver } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(receiver),
            arguments: Some(Vec::new()),
            resume: Some(Resume::Identity),
        },
    }
}

step_from! {
    crate::engine::builtins::ErrorStep => Error;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    String => String { value: take_string_value };
    Has => Has { object: take_has_object, key: take_has_key };
    Aggregate => Aggregate { iterable: take_aggregate_iterable };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}

step_from! {
    crate::engine::builtins::SumStep => Sum;
    Read => ReadValue { receiver: take_read_receiver, key: take_read_key };
    Call => call(take_call_callable, take_call_receiver);
    Next => IteratorNext { iterator: take_next_iterator, method: take_next_next };
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Close { mut resume } => {
            let iterator = resume.take_close_iterator();
            let completion = resume.take_close_completion();
            Self::IteratorClose {
                iterator: Some(iterator),
                completion: Some(completion),
            }
        }
    }
}

step_from! {
    crate::engine::builtins::AggregateStep => Aggregate;
    Read { receiver, key } => ReadValue;
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
        T::Call {
            callable,
            receiver,
            resume,
        } => Self::Call {
            target: Some(DirectCallTarget::Callable(callable)),
            receiver: Some(receiver),
            arguments: Some(Vec::new()),
            resume: Some(Resume::Aggregate(resume)),
        },
        T::Next {
            iterator,
            next,
            resume,
        } => Self::IteratorNext {
            iterator: Some(iterator),
            method: Some(next),
            resume: Some(Resume::Aggregate(resume)),
        },
        T::Close {
            iterator,
            completion,
        } => Self::IteratorClose {
            iterator: Some(iterator),
            completion: Some(completion),
        },
    }
}
