//! A pure-Rust rewrite of `QuickJS` aiming at semantic feature parity with the
//! pinned upstream release.
//!
//! The implementation deliberately follows `QuickJS`'s major runtime boundaries:
//! source is compiled to stack bytecode, bytecode executes inside a context,
//! and contexts share a runtime-owned heap and atom table.

/// Profiling-only statement: `profile!(name(args))` calls
/// `engine::api::profiling::name`, `profile!(let x = Type::start(args))`
/// binds a profiling guard. Both expand to nothing without `profiling`.
#[cfg(feature = "profiling")]
macro_rules! profile {
    ($name:ident($($arg:tt)*)) => {
        $crate::engine::api::profiling::$name($($arg)*)
    };
    (let $var:ident = $ty:ident::$function:ident($($arg:tt)*)) => {
        let $var = $crate::engine::api::profiling::$ty::$function($($arg)*);
    };
}
#[cfg(not(feature = "profiling"))]
macro_rules! profile {
    ($($statement:tt)*) => {};
}

/// Interpreter implementation; embedders use [`engine::api`].
///
/// Old root API aliases are deliberately unavailable:
/// ```compile_fail
/// use quickjs_oxide::Runtime;
/// ```
/// Internal engine modules are not embedding interfaces:
/// ```compile_fail
/// use quickjs_oxide::engine::heap::Heap;
/// ```
/// ```compile_fail
/// use quickjs_oxide::engine::vm::Vm;
/// ```
pub mod engine;
pub mod regexp;
pub mod source;

/// The version of this quickjs-oxide engine crate.
pub const QUICKJS_OXIDE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The exact upstream release whose observable behavior is the compatibility
/// baseline for this crate.
pub const QUICKJS_COMPAT_VERSION: &str = "2026-06-04";
