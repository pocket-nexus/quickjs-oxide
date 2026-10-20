//! A pure-Rust rewrite of `QuickJS` aiming at semantic feature parity with the
//! pinned upstream release.
//!
//! The front end compiles source to `QuickJS`-shaped stack bytecode, the
//! register lowering turns it into `quickjs-oxide-core`'s register bytecode, and the
//! embedding API runs it on the core.

/// Interpreter implementation; embedders use [`engine::api`].
///
/// Old root API aliases are deliberately unavailable:
/// ```compile_fail
/// use quickjs_oxide::Runtime;
/// ```
/// Internal engine modules are not embedding interfaces:
/// ```compile_fail
/// use quickjs_oxide::engine::compiler::CompileOptions;
/// ```
pub mod engine;
pub mod source;

/// The version of this quickjs-oxide engine crate.
pub const QUICKJS_OXIDE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The exact upstream release whose observable behavior is the compatibility
/// baseline for this crate.
pub const QUICKJS_COMPAT_VERSION: &str = "2026-06-04";
