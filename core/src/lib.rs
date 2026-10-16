//! The new quickjs-oxide core (issue #122): 8-byte NaN-boxed values, typed
//! arenas, a value heap with range allocation and a non-moving mark-sweep
//! collector. See `docs/performance/new-core-phase1.md` and `core/README.md`.

#![forbid(unsafe_code)]

pub mod value;

pub use value::{
    BigIntId, Kind, ObjectId, ScopeId, ShapeId, StringId, SymbolId, ValidityCellId, Value,
};
