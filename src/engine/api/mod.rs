//! Rust embedding boundary: the API over `quickjs-oxide-core` lives in [`core`].
pub mod core;
pub mod error;

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
pub use crate::engine::api::core::compile::use_new_front_end;
/// Formatting and source classification used by embedders.
pub use crate::engine::value::number_to_string;

#[cfg(feature = "test-support")]
pub mod testing;
