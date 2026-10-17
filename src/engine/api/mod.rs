//! Rust embedding boundary for runtime construction, evaluation and rooted values.
pub use crate::engine::api::context::{Context, EvalOptions};
pub use crate::engine::api::error::{Error, ErrorKind};
pub use crate::engine::api::runtime::Runtime;
pub use crate::engine::api::runtime_error::RuntimeError;

#[cfg(feature = "test262-host")]
pub use crate::engine::api::test262_agent::{Test262AgentError, Test262AgentSession};
pub use crate::engine::builtins::promise::{PromiseRejectionEvent, PromiseSnapshot};
pub use crate::engine::code::debug::DebugInfoMode;
pub use crate::engine::code::rooted::FunctionBytecodeRef;
pub use crate::engine::compiler::CompileOptions;

pub use crate::engine::code::module::{ModuleImportAttribute, ModuleImportAttributes};
pub use crate::engine::heap::{ContextId, GcPolicy, GcStats, HeapCounts, PromiseState};
pub use crate::engine::host::HostServices;
pub use crate::engine::jobs::{PendingJobError, PendingJobOutcome};
pub use crate::engine::modules::{
    ModuleBytecodeRef, ModuleImportMetaProperty, ModuleLoadResult, ModuleLoader, ModuleLoaderError,
    ModuleLoaderRegistration,
};

pub use crate::engine::object::{
    AccessorValue, CallableRef, CompleteOrdinaryPropertyDescriptor, DescriptorField, ObjectRef,
    OrdinaryPropertyDescriptor, PropertyKey, SymbolRef, WellKnownSymbol,
};
pub use crate::engine::value::bigint::{BigIntError, JsBigInt};
pub use crate::engine::value::{JsString, JsStringError, Value};

pub(crate) mod compile;
pub(crate) mod context;

pub(crate) mod runtime_error;

pub(crate) mod runtime;

#[cfg(feature = "profiling")]
pub mod profiling;

#[cfg(feature = "test262-host")]
pub(crate) mod test262_agent;

#[cfg(feature = "test262-host")]
pub(crate) mod test262_host;

pub mod core;
pub mod error;
pub mod new_core;

pub use crate::engine::compiler::lexer::quickjs_detect_module_bytes;

/// Declare that the current thread was spawned with `size` bytes of stack.
///
/// Call this first thing on the thread. The parser reports deeply nested
/// source as a catchable `SyntaxError: stack overflow` before exhausting the
/// native stack; without a declaration it assumes at most one MiB is
/// available to a parse on a thread other than the main thread.
pub fn declare_thread_stack(size: usize) {
    crate::engine::compiler::stack_guard::declare_thread_stack(size);
}
/// Formatting and source classification used by embedders.
pub use crate::engine::value::number_to_string;

#[cfg(feature = "test-support")]
pub mod testing;
