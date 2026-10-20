//! The embedding API over `oxide-core`: one realm per runtime, values as
//! Rust data or handles, the host as one trait, host functions with a
//! [`Scope`], module loaders with a [`ModuleScope`].

mod buffer;
pub(crate) mod compile;
mod context;
mod convert;
mod error;
mod function;
mod module;
mod ops;
mod promise;
mod property;
mod runtime;
#[cfg(feature = "test262-host")]
pub mod test262;
mod value;

pub use buffer::{TypedArrayKind, TypedArrayView};
pub use context::{Context, EvalOptions, FunctionBytecodeRef, ModuleBytecodeRef, QjsHelpers};
pub use convert::{FromJs, IntoJs};
pub use error::RuntimeError;
pub use function::{FunctionOptions, HostFunction, Raw, Scope};
pub use module::{CompiledModule, ModuleLoader, ModuleScope, ModuleSource};
pub use ops::{Js, JsResult};
pub use promise::PromiseCapability;
pub use property::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, OrdinaryPropertyDescriptor,
    PropertyKey, WellKnownSymbol,
};
pub use runtime::{
    GcPolicy, GcStats, InterruptHandle, PendingJobOutcome, PromiseSnapshot, PromiseState, Runtime,
    SystemHost,
};
pub use value::{CallableRef, ObjectRef, SymbolRef, Value};

pub use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind};
pub use crate::engine::api::{declare_thread_stack, number_to_string, quickjs_detect_module_bytes};
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
