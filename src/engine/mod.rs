//! The front end (AST, HIR, MIR and the register lowering) and the
//! embedding API over `quickjs-oxide-core`.
pub mod api;
pub(crate) mod ast;
pub(crate) mod hir;
pub(crate) mod limits;
pub(crate) mod mir;
pub(crate) mod value;
