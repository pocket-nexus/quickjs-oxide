//! The new quickjs-oxide core (issue #122): 8-byte NaN-boxed values, typed
//! arenas, a value heap with range allocation and a non-moving mark-sweep
//! collector (phase 1); register bytecode (phase 2). See
//! `docs/performance/new-core-phase1.md`, `new-core-phase2.md` and
//! `core/README.md`.

#![forbid(unsafe_code)]

pub mod bytecode;
pub mod heap;
pub mod runtime;
pub mod shape;
pub mod value;

pub use bytecode::{CodeStore, CodeUnit, CodeUnitId, Insn, Opcode};
pub use heap::atom::{KeyKind, PropertyKey};
pub use heap::gc::{Handle, HeapLimits, RootMark};
pub use heap::object::ObjectKind;
pub use heap::{BytecodeTemplate, Heap, HeapError};
pub use runtime::{Intrinsic, Runtime};
pub use shape::PropertyFlags;
pub use value::{
    BigIntId, BytecodeId, Kind, ObjectId, ScopeId, ShapeId, StringId, SymbolId, ValidityCellId,
    Value,
};
