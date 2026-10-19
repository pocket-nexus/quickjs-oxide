//! The quickjs-oxide core: 8-byte NaN-boxed values, typed arenas, a value heap
//! with range allocation and a non-moving mark-sweep collector; register
//! bytecode and the loop that runs it; the builtins, the host boundary and the
//! embedder's entries. See `core/README.md`.

#![forbid(unsafe_code)]

pub mod builtins;
pub mod bytecode;
mod clock;
pub mod embed;
pub mod heap;
pub mod host;
pub mod runtime;
pub mod shape;
pub mod value;
pub mod vm;

pub use builtins::agent::ScriptRunner;
pub use bytecode::{CodeStore, CodeUnit, CodeUnitId, Insn, Opcode, UnitRef, Units};
pub use embed::{KeyName, PropertyDescriptor};
pub use heap::atom::{KeyKind, PropertyKey};
pub use heap::gc::{Handle, HeapLimits, RootMark};
pub use heap::object::ObjectKind;
pub use heap::report::MemoryReport;
pub use heap::{AtomMode, BytecodeTemplate, Heap, HeapError};
pub use runtime::{Intrinsic, Realm, Runtime, RuntimeOptions, ThrownSummary};
pub use shape::PropertyFlags;
pub use value::{
    BigIntId, BytecodeId, Kind, ObjectId, ScopeId, ShapeId, StringId, SymbolId, ValidityCellId,
    Value,
};
pub use vm::{ErrorKind, Output, Throw};
