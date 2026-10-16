//! The new quickjs-oxide core (issue #122): 8-byte NaN-boxed values, typed
//! arenas, a value heap with range allocation and a non-moving mark-sweep
//! collector. See `docs/performance/new-core-phase1.md` and `core/README.md`.

#![forbid(unsafe_code)]

pub mod heap;
pub mod runtime;
pub mod shape;
pub mod value;

pub use heap::atom::{KeyKind, PropertyKey};
pub use heap::gc::{Handle, HeapLimits, RootMark};
pub use heap::object::ObjectKind;
pub use heap::{Heap, HeapError};
pub use runtime::{Intrinsic, Runtime};
pub use shape::PropertyFlags;
pub use value::{
    BigIntId, Kind, ObjectId, ScopeId, ShapeId, StringId, SymbolId, ValidityCellId, Value,
};
