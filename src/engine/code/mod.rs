//! Instructions, compilation drafts and runtime-rooted code.
pub mod bytecode;
pub(crate) mod bytecode_validation;
pub mod debug;
pub mod function;
pub(crate) mod instruction;
pub mod module;
pub mod rooted;

pub(crate) mod bytecode_publish;

pub(crate) mod dynamic_import_policy;

pub(crate) mod runtime;

pub(crate) mod dynamic_source;

pub(crate) mod exec;
pub(crate) mod exec_opcode;
pub(crate) mod initialization;
pub(crate) mod region;

mod executable;
