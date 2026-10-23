//! HIR: the bound tree and tables the front end hands the middle end.
//!
//! The schema lives in `contracts/spec/hir.ts`; [`schema`] is generated
//! from it by `tools/hir-schema/gen.ts`.
#[allow(dead_code)]
pub(crate) mod schema;

#[cfg(test)]
mod tests;
