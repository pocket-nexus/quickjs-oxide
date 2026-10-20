//! The API's error type.

use std::fmt;

use super::value::Value;
use crate::engine::api::error::Error;
use crate::engine::value::JsStringError;

/// Why an API call failed.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeError {
    /// JavaScript threw this value.
    Exception(Value),
    /// An API call was made from inside a host callback without its
    /// [`Scope`](super::Scope).
    Reentered,
    /// The engine does not have this (`"realms"`).
    Unsupported(&'static str),
    /// The front end rejected the source.
    Compile(Error),
    /// The heap cap was reached.
    OutOfMemory,
    /// The embedder broke one of the API's preconditions.
    Invariant(&'static str),
    /// The engine failed internally.
    Internal(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Exception(_) => f.write_str("JavaScript exception"),
            RuntimeError::Reentered => {
                f.write_str("the runtime was re-entered from a host callback")
            }
            RuntimeError::Unsupported(what) => write!(f, "{what} are not supported"),
            RuntimeError::Compile(error) => write!(f, "{error}"),
            RuntimeError::OutOfMemory => f.write_str("out of memory"),
            RuntimeError::Invariant(what) => write!(f, "invariant violated: {what}"),
            RuntimeError::Internal(message) => write!(f, "internal error: {message}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<Error> for RuntimeError {
    fn from(error: Error) -> Self {
        RuntimeError::Compile(error)
    }
}

impl From<JsStringError> for RuntimeError {
    fn from(error: JsStringError) -> Self {
        match error {
            JsStringError::OutOfMemory => RuntimeError::OutOfMemory,
            other => RuntimeError::Internal(other.to_string()),
        }
    }
}

impl From<oxide_core::HeapError> for RuntimeError {
    fn from(error: oxide_core::HeapError) -> Self {
        match error {
            oxide_core::HeapError::OutOfMemory => RuntimeError::OutOfMemory,
            other => RuntimeError::Internal(format!("{other:?}")),
        }
    }
}
