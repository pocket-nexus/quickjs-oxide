//! The front end's stack instructions and unlinked function and module records,
//! which the register lowering reads.
pub mod bytecode;
pub mod debug;
pub mod function;
pub(crate) mod instruction;
pub mod module;
