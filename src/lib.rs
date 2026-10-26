//! A pure-Rust rewrite of `QuickJS` aiming at semantic feature parity with the
//! pinned upstream release.
//!
//! The front end parses source into a syntax tree, binds it into the HIR,
//! builds each function's MIR and lowers it to `quickjs-oxide-core`'s register
//! bytecode; the embedding API runs it on the core.

/// Interpreter implementation; embedders use [`engine::api`].
///
/// Old root API aliases are deliberately unavailable:
/// ```compile_fail
/// use quickjs_oxide::Runtime;
/// ```
/// Internal engine modules are not embedding interfaces:
/// ```compile_fail
/// use quickjs_oxide::engine::mir::compile::LowerError;
/// ```
pub mod engine;
pub mod source;

/// The version of this quickjs-oxide engine crate.
pub const QUICKJS_OXIDE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The exact upstream release whose observable behavior is the compatibility
/// baseline for this crate.
pub const QUICKJS_COMPAT_VERSION: &str = "2026-06-04";
