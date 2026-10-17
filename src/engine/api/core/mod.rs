//! The embedding API over the new core (phase 6, section 2): one realm per
//! runtime, values as Rust data or handles, the host as one trait, module
//! loaders with a [`Scope`]. It keeps the current API's shapes wherever the
//! new core gives them their meaning (section 2.7).

pub(crate) mod compile;
mod context;
mod error;
mod module;
mod property;
mod runtime;
mod value;

pub use context::{Context, EvalOptions, FunctionBytecodeRef, ModuleBytecodeRef, QjsHelpers};
pub use error::RuntimeError;
pub use module::{CompiledModule, ModuleLoader, ModuleSource, Scope};
pub use property::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, OrdinaryPropertyDescriptor,
    PropertyKey, WellKnownSymbol,
};
pub use runtime::{
    GcPolicy, GcStats, PendingJobOutcome, PromiseSnapshot, PromiseState, Runtime, SystemHost,
};
pub use value::{CallableRef, ObjectRef, SymbolRef, Value};

pub use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
pub use crate::engine::code::debug::DebugInfoMode;
pub use crate::engine::code::module::{ModuleImportAttribute, ModuleImportAttributes};
pub use crate::engine::compiler::CompileOptions;
pub use crate::engine::value::bigint::{BigIntError, JsBigInt};
pub use crate::engine::value::{JsString, JsStringError};
/// The core's value, which [`Host::promise_rejection`] receives.
pub use oxide_core::Value as CoreValue;
pub use oxide_core::host::{Host, RejectionEvent, Stream};
pub use oxide_core::{HeapLimits, MemoryReport, RuntimeOptions};

#[cfg(test)]
mod tests;
