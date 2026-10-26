//! MIR: the middle end's SSA form of one function, built from the HIR and
//! lowered to the core's register code.
#![allow(dead_code)]

pub(crate) mod build;
pub(crate) mod compile;
pub(crate) mod dump;
pub(crate) mod ir;
pub(crate) mod lower;
pub(crate) mod op;
pub(crate) mod verify;

#[cfg(test)]
mod ic_tests;
#[cfg(test)]
pub(crate) mod run_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod window_tests;
