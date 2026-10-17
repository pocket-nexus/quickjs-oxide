//! The new quickjs-oxide core (issue #122): 8-byte NaN-boxed values, typed
//! arenas, a value heap with range allocation and a non-moving mark-sweep
//! collector (phase 1); register bytecode and the loop that runs it (phase 2). See
//! `docs/performance/new-core-phase1.md`, `new-core-phase2.md` and
//! `core/README.md`.

#![forbid(unsafe_code)]

pub mod builtins;
pub mod bytecode;
pub mod embed;
pub mod heap;
pub mod host;
pub mod runtime;
pub mod shape;
pub mod value;
pub mod vm;

pub use builtins::agent::ScriptRunner;
pub use bytecode::{CodeStore, CodeUnit, CodeUnitId, Insn, Opcode};
pub use embed::{KeyName, PropertyDescriptor};
pub use heap::atom::{KeyKind, PropertyKey};
pub use heap::gc::{Handle, HeapLimits, RootMark};
pub use heap::object::ObjectKind;
pub use heap::report::MemoryReport;
pub use heap::{BytecodeTemplate, Heap, HeapError};
pub use runtime::{Intrinsic, Realm, Runtime, RuntimeOptions, ThrownSummary};
pub use shape::PropertyFlags;
pub use value::{
    BigIntId, BytecodeId, Kind, ObjectId, ScopeId, ShapeId, StringId, SymbolId, ValidityCellId,
    Value,
};
pub use vm::{ErrorKind, Output, Throw};
