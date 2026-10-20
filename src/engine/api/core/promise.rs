//! Promises created and settled from Rust.

use super::ops::{Js, JsResult};
use super::value::{CallableRef, ObjectRef, Value};

/// A promise and its resolving functions ([`Js::new_promise`]). A host
/// function returns `promise` at once and keeps the capability; when the
/// embedder's event arrives it settles the promise from the top level and
/// drains the job queue.
#[derive(Clone, Debug, PartialEq)]
pub struct PromiseCapability {
    pub promise: ObjectRef,
    pub resolve: CallableRef,
    pub reject: CallableRef,
}

impl PromiseCapability {
    /// Resolves the promise with `value` (a thenable is followed, as the
    /// specification's resolve function does). Settling it twice does
    /// nothing.
    pub fn resolve(&self, js: &mut impl Js, value: Value) -> JsResult<()> {
        js.call(&self.resolve, Value::Undefined, &[value]).map(drop)
    }

    /// Rejects the promise with `reason`.
    pub fn reject(&self, js: &mut impl Js, reason: Value) -> JsResult<()> {
        js.call(&self.reject, Value::Undefined, &[reason]).map(drop)
    }
}
