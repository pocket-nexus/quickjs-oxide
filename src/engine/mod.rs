//! The front end, the register lowering and the embedding API over `oxide-core`.
pub mod api;
pub(crate) mod code;
pub(crate) mod compiler;
pub(crate) mod value;

#[cfg(test)]
pub(crate) mod test_support;
