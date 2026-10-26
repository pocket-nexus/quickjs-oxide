//! HIR: the bound tree and tables the front end hands the middle end.
//!
//! The schema lives in `contracts/spec/hir.ts`; [`schema`] is generated
//! from it by `tools/hir-schema/gen.ts`. [`build`] produces it from the
//! parser's syntax tree, resolving every name; [`validate`] checks it before
//! the middle end trusts it.
#[allow(dead_code)]
pub(crate) mod build;
#[allow(dead_code)]
pub(crate) mod schema;
#[allow(dead_code)]
pub(crate) mod validate;

#[cfg(test)]
mod tests;
