//! The front end's primitive values: strings, BigInts and the number
//! conversions the lexer, the constant pool and the API share.
pub mod bigint;

mod primitive;
pub use primitive::*;
