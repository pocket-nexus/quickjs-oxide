# Front-end code records

code holds what the front end produces and the register lowering reads:
QuickJS-shaped stack instructions and the unlinked function and module
records that carry them. The lowering (`compiler/register`) turns these records into `oxide-core` code units,
which the core's code store validates and owns.

| File | Content |
| --- | --- |
| [instruction.rs](instruction.rs) | the opcode table: every stack `Instruction` with its operands, stack effect and the facts the front end's passes and the lowering read, declared once in the style of QuickJS's `quickjs-opcode.h` |
| [bytecode.rs](bytecode.rs) | the operand kinds of the instructions (`ArgumentsKind`, `ApplyKind`, `IteratorCallKind`, …), `MAX_LOCAL_SLOTS`, and `compute_stack_size`, QuickJS's maximum operand-stack depth over every reachable path, which reports QuickJS's "stack overflow" past the bound |
| [function.rs](function.rs) | `UnlinkedFunction`: the stack code, the constants (`UnlinkedConstant`: primitives, property-name strings, regular-expression literals, template sites and child functions), the metadata, the argument and local definitions, the closure variables, the eval environments and the debug record (file name, pc-to-source table, source text) |
| [function/metadata.rs](function/metadata.rs) | function, parameter, closure-variable and eval descriptors: `FunctionMetadata`, `FunctionKind`, `ConstructorKind`, `ClosureSource`, `ClosureVariableKind`, `EvalKind` and the rest |
| [module.rs](module.rs) | `UnlinkedModule`: the module's root function beside its request, import, export and star-export tables (QuickJS's `JSModuleDef`), and the import attributes the API's module loader receives |
| [debug.rs](debug.rs) | `DebugInfoMode` (`Full`, `StripSource`, `StripDebug`: what a newly compiled function keeps of its source text and positions) and the pc-to-source table, counted by QuickJS's LF and UTF-8 lead-byte rule |

The stack code's operand-stack depth is static at every instruction; the
lowering uses that depth as its register allocation and recomputes it per
instruction (`compiler/register/analysis.rs`). The lowering turns them into the core's atoms, constant tables and
frozen template objects, and a regular-expression literal into its source
and flags, which the core compiles when the literal is first evaluated.

[Workspace architecture](../../../docs/architecture.md) describes the stages
from source to running code.
