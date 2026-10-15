//! Mechanical adapters for conversion domain requests.
use super::JsValue;
use super::{NumberStep, Resume, Step};

step_from! {
    NumberStep => Number;
    Read => read(take_read_object, take_read_key);
    Call => call(take_call_callable, take_call_receiver, take_call_arguments);
    raw {
        T::Complete(result) => Self::NumberComplete(Some(result)),
    }
}

step_from! {
    crate::engine::value::conversion::primitive::PrimitiveStep => Primitive;
    Get => read(take_get);
    Call => call(take_callable, take_receiver, take_arguments);
    raw {
        T::Complete(result) => Self::Complete(Some(result)),
    }
}
