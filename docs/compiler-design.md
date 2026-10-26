# Compiler design

This page is the design of the compiler that turns JavaScript and a typed dialect of TypeScript into the core's register code. Its two goals are **less memory and more speed** in the running program: a smaller peak and resident heap, and fewer retired instructions per unit of work. It fixes the split between compiler and runtime, the stages and the contract between them, the typed dialect and its guarantee, the boundary between typed and untyped code, the effect facts the optimizer uses, the two optimization levels, the bytecode and VM additions, the image format, the tests, and the order of the work. It builds on [Architecture](architecture.md), which describes the current stages, and on [Planned work](planned.md), whose code images and heap snapshots this design produces and loads.

## 1. Goals and constraints

| Constraint | Consequence |
| --- | --- |
| Memory and speed of the running program are the goals | Every stage is judged by its effect on the heap and on retired instructions at run time; the [performance checks](../AGENTS.md#performance-checks) gate every change. The compiler's own memory is reported and carries no gate |
| JavaScript keeps QuickJS's behavior | JS mode is measured against QuickJS with Test262 and the position cases in `tests/frontend/positions`, checked by `scripts/checks/check-outputs.py` |
| Typed code needs sound types | The compiler parses, checks and infers its own dialect; the TypeScript checker plays no part |
| `quickjs-oxide-core` forbids `unsafe` | The engine interprets bytecode; the worst outcome of a wrong image is a panic |
| Compiler and runtime are separate components | The compiler writes images; the runtime accepts images only, through its link step (section 2) |
| The distributor sees the whole program | Compilation takes the application and the core image it targets as one module graph |

## 2. Compiler and runtime

The compiler and the runtime meet at one interface: the image. The compiler takes a module graph and writes a code image (`.oxc`) and a debug table (`.oxd`); a single script is a graph of one module. The runtime takes an image and links it (section 3.7) into its `CodeStore`. Every program reaches the runtime this way:

| Caller | Compiler runs in | Image travels |
| --- | --- | --- |
| a build for distribution | the host or the distributor's build service | as files |
| `qjs` with source, `eval`, the `Function` constructors, dynamic `import()` of source | the same process as the runtime | in memory |

The compiler holds the whole program at once: stage 5 needs the call graph, and every fact one function's code depends on in another function is computed before any MIR is built. The caller chooses the optimization level (section 7); `qjs` uses level 1.

The embedder chooses whether a build contains the compiler. The core reaches it through its compile function (`set_compiler`) and links images on its own, so an embedder that depends on `quickjs-oxide-core` alone runs images, and `eval`, the `Function` constructors and dynamic `import()` of source throw `InternalError: no compiler`. An embedder that also links the root crate runs JavaScript source as well.

## 3. Pipeline

```
source files
  │ 1  lexer / parser                     one file at a time
  ▼
AST + scope tree + declaration table
  │ 2  resolve                            the module graph
  ▼
resolution tables: bindings, captures, closure slots, TDZ, storage
  │ 3  AST → HIR                          translation
  ▼
HIR, state Resolved
  │ 4  check / infer / elaborate          JS-mode code passes through with every value dynamic
  ▼
HIR, state Checked                         ← the middle end accepts this state only
  │ 5  whole-program analysis
  ▼
HIR + call graph + effect summaries + reactive schedules
  │ 6  MIR build                          per function, from HIR facts only
  ▼
MIR: SSA with representations and an effect class per operation
  │ 7  optimization passes                by level
  │ 8  selection pass
  ▼
MIR in one-to-one correspondence with instructions
  │ 9  register allocation + emission
  ▼
image (.oxc) + debug table (.oxd)
  │ 10 link                               in the runtime
  ▼
units in the CodeStore
```

| Stage | Owns | Hands on |
| --- | --- | --- |
| 1 Parser | syntax for JS mode, typed mode and TSX; early errors in QuickJS's order; declaration registration; source offsets | the AST, the scope tree, the declaration table |
| 2 Resolve | lexical resolution of value and type names; module linking; captures and the closure slot of every captured binding; TDZ; the storage of each binding (SSA value, closure cell, module slot, global); the facts one function hands another (inferred names, private brands) | the resolution tables |
| 3 AST → HIR | translation, including the desugaring of classes into constructors and field initializers | HIR in state Resolved |
| 4 Check | the typing rules of section 4; bidirectional local inference | a typed HIR, or type errors |
| 4 Elaborate | property access resolved to a slot or a shape-keyed access; static callees; overload choice; boundary checks at the points of section 5.4; the two entries of every typed function | HIR in state Checked |
| 5 Whole-program analysis | the call graph; devirtualization; reachability (unreachable functions and modules are dropped); effect summaries (6.2); reactive schedules (6.3) | facts recorded on HIR functions and call sites |
| 6 MIR build | SSA; representation types; explicit box and unbox; an effect class on every operation | one MIR function |
| 7 Optimization | the passes of section 7 | optimized MIR |
| 8 Selection | fused instructions, superinstructions, immediate forms | MIR whose operations are instructions |
| 9 Allocation and emission | registers, coalescing, block order, loop rotation, call windows, encoding, the constant tables, the position table, the inline-frame table | the image and the debug table |
| 10 Link | interning the image's constants, numbering check, publication | units in the `CodeStore` |

### 3.1 Parser

The parser reads three syntaxes: JS mode, typed mode (TypeScript syntax under the rules of section 4) and TSX in typed mode. The compiler selects typed mode for `.ts` and `.tsx` modules and JS mode for every other module and for source passed to `eval` and the `Function` constructors. Early errors and their order match QuickJS in JS mode, because a program observes them. The parser records declarations, their syntactic kind and source offsets; storage and every other semantic decision belong to resolve.

### 3.2 Resolve

Resolve binds every value name and every type name to its declaration by lexical scope, links imports to exports, records captures and the TDZ checks each reference needs, assigns every captured binding its closure slot, and decides each binding's storage. It also computes the facts that one function's code needs about another: the name NamedEvaluation gives an anonymous function, and the private brand of a class's methods. It runs in three passes over the module graph: collect every module's declarations and exports, link imports to exports, then resolve references. Property names (`p.x`) belong to the checker, because their meaning depends on types.

### 3.3 HIR

The HIR schema is generated from `contracts/spec/hir.ts`. Every declaration carries its storage and closure slot from resolve, and every expression carries a typed slot; JS-mode code fills the typed slots with `dynamic`. The semantic types of section 4 are the schema's types: `dynamic`, `unknown`, `number` and the other primitives, nominal classes and records, structural object types, arrays, tuples, dictionaries, functions, unions, type parameters and named type declarations. Representations belong to MIR. `hir::validate` checks two states on the compile path: **Resolved** (every name bound, every capture and closure slot recorded, every storage decided) and **Checked** (every typed slot filled, every boundary check placed, every property access resolved). Stages 5 and later accept Checked HIR only.

### 3.4 Check, infer, elaborate

The checker applies the rules of section 4 with bidirectional inference inside a function; a function exported from a module states its parameter and return types. Elaboration writes out what the source leaves implicit: the slot or shape-keyed access of every property, the static callee of every call the types determine, the boundary checks of section 5, and for every typed function a fast entry and a checked entry (section 5.3).

### 3.5 MIR

A MIR function is an SSA graph built from the HIR of one function and the facts resolve and stage 5 recorded; it reads nothing from the MIR of any other function. Every binding that resolve stores as an SSA value is an SSA value, joined by phis; a captured binding lives in its closure cell. Values carry a representation: a tagged `JsValue`, `i32`, `f64`, a reference to a known layout, or a scalar-replaced object. Box and unbox are explicit operations. Every operation carries an effect class (section 6.1). `finally` blocks are ordinary control flow, duplicated or joined by the builder. Block order is a decision of stage 9.

### 3.6 Registers

Every VM register holds a valid `JsValue` at every instruction. Typed instructions read their operands without a tag check and write tagged results. An `i32` result is tagged by a constant OR; an `f64` result passes through `Value::number`, whose NaN test keeps a NaN's bits and maps the NaNs that fall in the boxed range to the canonical NaN. The collector therefore scans registers as values and needs no stack maps.

### 3.7 Link

Stage 9 writes code that refers to the program only through the image's own tables: property keys, strings, BigInts, regular expressions and template sites are indices into its constant tables, and child functions are indices into its unit table. The compiler numbers atoms: the atoms of the core image and its snapshot first, in their snapshot order, then the application's atoms in image order. Link interns the image's constants into the runtime and builds each unit's constant pool in RAM. When the runtime's atom numbering matches the compiler's, which holds for an application loaded into a runtime started from its core image's snapshot, the key immediates are used as written; otherwise link rewrites them in the runtime's copy of the code. Template objects are created on first use. Link then publishes each unit through the publication validator.

## 4. Types

### 4.1 Semantic types and representation types

| | Semantic type | Representation type |
| --- | --- | --- |
| Examples | `number`, `Point`, `string \| null`, `dynamic` | `i32`, `f64`, tagged value, scalar-replaced object |
| Source | annotations and inference | range and escape analysis |
| Layer | HIR | MIR |
| Visible to the program | yes | no |

The language has one numeric type, `number`. MIR represents a `number` as `i32` where range analysis proves it fits, and otherwise as `f64` or a tagged value; an `i32` operation that can overflow carries a branch to the `f64` path in the same function. Integer representation matters most on targets that compute `f64` in software.

### 4.2 The typed dialect

Typed mode uses TypeScript syntax with these rules; the rules document (section 14, step 5) states each as *TypeScript's rule → this dialect's rule → reason → runtime check*.

- `dynamic` takes the place of `any`; `unknown` is the top type and needs a check to use.
- A downcast `as T` and a non-null assertion `!` are checked at run time.
- Mutable arrays are invariant; method parameters are contravariant.
- `class` and `record` declarations are nominal, have a fixed layout and are sealed.
- Interfaces and object type literals are structural.
- A user-defined type guard is accepted when the checker verifies its body; `eval` and `with` are rejected in typed mode.
- Generics are erased: a value of a type parameter is represented as `dynamic` and checked where it flows into a concrete type.
- A module's exported functions state their types.

### 4.3 The guarantee

A program that passes the checker behaves, with its types erased and run on QuickJS, exactly as it behaves compiled, except that the compiled program throws `TypeError` in three cases:

1. **Entry.** A value entering typed code does not have its static type (a boundary check fails, including a tightening that conflicts with an earlier one; section 5.2).
2. **Cast.** A checked `as` or `!` fails.
3. **Violation.** An operation would break a type a value carries: a write of a value outside a constrained property's type, an addition to a sealed instance, a deletion or reconfiguration of a constrained property, or a hole in a pinned array.

This guarantee is also the test method of typed mode (section 11).

## 5. Typed and untyped code together

### 5.1 One IR

JS code and typed code use the same HIR, the same MIR and the same passes. JS code is a program whose values are all `dynamic`. The mode belongs to each operation: a typed function may contain dynamic operations, and inlining mixes both kinds in one function. The passes read effect classes and representations only, so JS code receives every optimization its operations permit; a loop counter that no closure captures, for example, is an `i32` in JS mode too.

### 5.2 Values crossing the boundary

Every object that typed code views at a precise object type carries that type in the VM, and every write to it is checked against it. Typed code therefore reads fields without checks. Two mechanisms give objects their types: **nominal construction** for instances that typed code creates, and **monotonic tightening** for objects that arrive from untyped code. Both record their constraints on the object's shape, so the per-object cost is zero and every object with the same shape and the same constraints shares them.

| Static type | Created by typed code | Arriving as `dynamic` |
| --- | --- | --- |
| primitive, union of primitives, `T \| null` | — | a tag check |
| `class` / `record` | a sealed instance: class tag, fixed layout, fixed prototype, constrained fields | a class-tag check; a plain object never becomes an instance |
| interface, object type literal | a sealed record of an anonymous layout | tightened in place |
| `T[]` | an array pinned to `T` | every element checked once, then pinned to `T` |
| tuple | an array pinned per index, with fixed length | checked per index, then pinned |
| `Record<string, T>`, index signature | an object pinned to `T` for every key | every own property checked once, then pinned |
| function type | a function with two entries | a callable check; each call checks its result |
| union of object types | the member's own construction | tightened to the member its literal discriminant selects |
| type parameter `T` | `dynamic` | checked where it flows into a concrete type |

**Tightening.** When a `dynamic` value flows into an interface type, the VM checks that each data member is an own data property of the member's type (absent for an optional member), that no member is an accessor, and that the object is an ordinary object. It then moves the object to a shape that records the constraints, including those for absent optional members. Nested members of object type are tightened in the same walk; an object already tightened at a compatible type ends the walk at once, and a cycle ends it at its first repeat. Method members are checked for callability at their current resolution along the prototype chain and called through the checked path, so prototypes stay unconstrained. Extra properties stay unconstrained (width subtyping). Tightening an object again adds constraints; two constraints with no common type make the second tightening fail with `TypeError`.

**Pinning.** A pinned array or dictionary records its element type. A write of a value outside that type, a `delete`, or a length change that creates a hole throws `TypeError`; `push`, `pop`, `sort` and a length that shrinks stay valid.

**Why this design.** Tightening keeps an object's identity, so an erased program and a compiled program share every object and the guarantee holds. It allocates nothing: a value crosses without a copy. It costs one walk the first time a value crosses at a type and one shape comparison every later time. Typed reads never re-check, so the optimizer treats a field read as a load from a known region. Untyped code pays only on objects that typed code has constrained, and only in the store handlers of their shapes. An interface-typed access site that stage 5 proves sees one layout becomes a fixed-slot load.

### 5.3 Functions across the boundary

Every typed function has two entries. The **fast entry** serves `CallDirect` from typed callers whose arguments the checker proved; it checks nothing. The **checked entry** serves every other caller: it checks each argument against its parameter type, tightening as section 5.2 describes, and continues at the fast entry. A typed function value handed to untyped code exposes the checked entry. An untyped function called from typed code is called through the generic call; its result is checked against the static return type.

### 5.4 Where checks happen

A boundary check sits at one of these points: the checked entry of a function, the return from a call to an untyped callee, the point where a `dynamic` value is assigned or passed to a typed binding or parameter, and a checked cast. The optimizer merges a check with an identical dominating check, and moves a check earlier only across operations whose effect class is free of observable effects (section 6.1). A `TypeError` therefore appears at the same point of the observable execution in every compilation of a program.

### 5.5 What optimization leaves observable

| Observable | Mechanism |
| --- | --- |
| frames in `Error.stack` | the inline-frame table: at a throw, the VM expands each inlined call into its frame |
| source positions in `Error.stack` | the position table, carried in the image for source run in process and in the `.oxd` for distributed images; a device without the `.oxd` prints function names with unit and offset, and the [deviation ledger](deviations.md) records this form |
| `Function.prototype.toString` | the `DebugInfoMode` of [Architecture](architecture.md#compilation) |
| the point of a `TypeError` | the check placement of section 5.4 |

Positions matter only on operations that can throw or call: an operation whose effect class has neither `throws` nor `calls_js` carries no position.

## 6. Effect facts

Effect facts are what let the optimizer move, merge and delete operations. They come from the operations themselves and from whole-program inference; the source carries no effect annotations.

### 6.1 Effect classes

Every MIR operation carries an effect class built from these facts, derived from its operands' types and representations:

| Fact | Meaning |
| --- | --- |
| `reads(r)`, `writes(r)` | the regions it reads and writes: an SSA value's register, a closure cell, a field of a layout, the elements of a pinned kind, or the whole heap |
| `calls_js` | it may run arbitrary JavaScript (a getter, a proxy trap, `valueOf`) |
| `throws` | it may throw |
| `suspends` | it may suspend the function (`await`, `yield`) |
| `allocates` | it may allocate |
| `host` | it calls the embedder |

A dynamic property read is `calls_js + throws + reads(heap) + writes(heap)`. A fixed-slot load from a proven non-null reference is `reads(field)`. A closure-cell read is `reads(cell)`. Each `Op` has a base class in `mir::op`; the operation's operands refine it. Selection keeps each operation's effect class on the instruction it chooses.

### 6.2 Function summaries

Stage 5 computes, for every function, the union of its operations' facts and the set of parameters that escape. The computation runs bottom-up over the call graph, iterating each strongly connected component to a fixed point; an indirect call whose targets stage 5 cannot enumerate contributes every fact. A call site then carries its callee's summary as its effect class, so a field stays in a register across a call that writes other regions, a call that cannot throw has no exception edge, an argument that does not escape permits scalar replacement, and a call with an unused result and no effects is deleted.

### 6.3 Reactive schedules

The core image's reactive runtime exports `createSignal`, `createMemo` and `createEffect`. Applications import them from the core image's module, so the compiler identifies each call by its binding. At a `createMemo` or `createEffect` call whose argument is a function literal, stage 5 collects the signals the literal reads. When every path through the literal reads the same signals, the call is rewritten to the runtime's static form with that list, and the runtime skips dependency tracking for it; every other call keeps runtime tracking. The rewrite keeps the runtime's update order and batching, so a program draws the same frames either way.

## 7. Optimization levels

| | Level 0 (development) | Level 1 (release) |
| --- | --- | --- |
| Stage 5 | call graph, effect summaries | all of stage 5 |
| Inlining | — | by size and call-graph position |
| Local passes | constant folding, branch folding | constant folding and propagation, dead-code elimination, global value numbering, CFG simplification |
| Loop and memory passes | — | loop-invariant code motion, escape analysis and scalar replacement, range analysis, check elimination |
| Selection | fused instructions, immediates | fused instructions, immediates, superinstructions |
| Bindings | every source binding live to the end of its scope | as liveness requires |
| Debugging | positions, breakpoint and step hooks | positions, inline-frame table |

A program produces the same output at both levels and under its erasure on QuickJS; section 11 tests this.

## 8. Bytecode and VM

| Addition | Purpose |
| --- | --- |
| `AddI32`, `SubI32`, `MulI32` with an overflow target; `AddF64` and the other `f64` arithmetic; typed comparisons and conversions | arithmetic without type dispatch |
| `LoadField`, `StoreField` | field access at a layout offset without a shape check |
| `CallDirect unit` | a call to a known unit through its fast entry |
| `CallHost id` | a call to an embedder function by index, with arguments as the host interface types them |
| `CheckDesc reg, descriptor` | the boundary check of section 5.2, tightening and pinning included |
| constrained shapes | per-property type constraints on shapes; store, define and delete handlers check them |
| sealed instances | class tag, fixed layout, fixed prototype, non-extensible |
| pinned arrays and dictionaries | an element type in the object header, checked by element stores |
| inline-frame table | stack-trace frames for inlined calls |

The opcode space is a `u8` with room for these additions; an instruction that needs three registers and an overflow target takes a new `Format` within the 8-byte instruction. The host interface (the embedder's functions, their types and their indices) is a generated declaration that the checker reads and `CallHost` indexes. Asynchronous functions compile to state machines whose frames live on the heap; the embedder resumes ready tasks from its loop, and a task belongs to an owner scope whose disposal cancels it.

Images come from the build service, which signs them; the embedder's installer verifies the signature. Link runs the publication validator of [Planned work](planned.md#4-code-images-and-heap-snapshots) on every unit; the soundness of typed instructions rests on the compiler that produced the image.

## 9. Images

Images follow the format of [Planned work](planned.md#4-code-images-and-heap-snapshots): little-endian, position-independent, every reference a `u32` index, a build fingerprint. This design fixes:

- **Code as fixed records.** The code section of a unit is an 8-byte-aligned array of 8-byte records: the opcode byte, the `a`, `b` and `c` bytes, and `imm` as a little-endian `u32`. This is the memory layout of the core's `Insn` on a little-endian target. Link copies each unit's records into an `Insn` array after validation. Executing the records in place, through `slice::as_chunks::<8>` and dispatch on the opcode byte, is a change inside the runtime that leaves the format as it is.
- **Constants by value.** Atoms by text, numbers, BigInt digits, regular expressions as source and flags, template sites as their strings; link builds the runtime values (section 3.7).
- **Debug tables apart.** Position tables and the inline-frame table go to the `.oxd`. A distributed image travels without it; a device reports unit and offset, and the host maps them back. An image compiled in process carries its debug tables beside it in memory.
- **Core binding.** An application image records the fingerprint of the core image it was compiled against and refers to the core's functions, layouts and atoms by index. Link accepts the application image when the fingerprints match; on a core update, the build service recompiles the application.

## 10. Runtime layering

```
application images      one runtime per application; discarded whole on exit or reload
core image + snapshot   the framework and system JavaScript, shared, read-only
host interface          generated bindings: CallHost indices and their types
engine                  VM, link, collector
embedder                drawing, layout, input, I/O, the loop
```

The embedder drives each runtime from its loop: once per tick it delivers input, drains the job queue and resumes ready tasks within a budget. The embedder calls into JavaScript only at these points. A reload discards the application's runtime with its heap and its host objects and starts a new runtime from the snapshot.

## 11. Tests

| Subject | Reference | Method |
| --- | --- | --- |
| JS mode | QuickJS | Test262, the position cases (`scripts/checks/check-outputs.py`) |
| Typed mode | the erased program on QuickJS | equal output, or `TypeError` in a case of section 4.3 |
| Typing rules | the rules document | at least one accepted and one rejected program per rule |
| Gradual guarantee | the fully annotated program | random programs with a random subset of annotations removed produce the same output, with fewer `TypeError`s at most |
| Check placement | section 5.4 | programs whose side effects precede and follow a failing check, at both levels |
| Soundness | — | randomly generated typed programs are rejected or match their erasure |
| Levels | level 0 | level 1 produces the same output |
| Reactive schedules | runtime tracking | the same frames with the static form and with tracking |
| Images | the program compiled in process | write the image to bytes, read it back, link, run: same output; a flipped byte at every offset is rejected with `Err` |

Every program run from source passes through an image, so every test above also exercises the image format and link.

## 12. Measurement

Every change to the engine runs the Sweep of [the agent guide](../AGENTS.md#performance-checks). Programs run from source include their compilation, so the Sweep's instruction counts cover the compiler at level 1, image encoding and link. Compilation is also measured apart with `bench.py compile` (the corpus, the probes and the matrix). The running program's peak and live heap are measured with `bench.py memory`; image size is reported per program; the compiler's own memory is reported beside them.

## 13. Location

| Code | Stage |
| --- | --- |
| `src/engine/ast/parser`, `ast/lexer.rs` | 1 |
| new `hir::resolve`, from the resolution parts of `src/engine/hir/build.rs` | 2 |
| `src/engine/hir/build.rs` | 3 |
| `src/engine/hir/schema.rs` (from `contracts/spec/hir.ts`), `hir/validate.rs` | the HIR and its two states |
| new `hir::check` | 4 |
| new `analysis` | 5 |
| `src/engine/mir/build`, `mir/op.rs` | 6, with effect classes |
| new `mir::pass` | 7 |
| new `mir::select`, from the selection parts of `mir/lower/alloc.rs` (`Plan`) and `mir/lower/mod.rs` | 8 |
| `src/engine/mir/lower` | 9; `Plan` keeps register decisions |
| `src/engine/mir/compile.rs` | the compiler's entry: module graph in, image out |
| `quickjs-oxide-core` link and publication | 10 |
| `quickjs-oxide-core` bytecode and VM | section 8 |

## 14. Order of the work

Every step keeps JS mode's behavior. Its acceptance is the Test262 vector, the position cases and the Sweep, plus what the step names; the Sweep's instruction changes are explained per program.

1. **Compiler and runtime apart.** Stage 9 writes the image's tables and records; link in the core; `.oxd`; source run in process goes through an in-memory image. Acceptance: the image tests of section 11.
2. **Resolve.** Stage 2 apart from stage 3; storage, closure slots, inferred names and private brands decided in resolve and carried in HIR; class desugaring in translation; `hir::validate` for state Resolved on the compile path.
3. **SSA bindings and the pass framework.** Bindings as SSA values; `finally` as control flow; block order in stage 9; effect classes with regions on MIR operations; `mir::pass`; `Plan`'s non-register annotations as passes and selection.
4. **Whole-program compilation.** The module graph, stage 5 with effect summaries, and level 1's passes for JS mode. Acceptance: the level tests of section 11; instructions, heap and image size reported per program.
5. **Typed mode.** The HIR's semantic types of section 3.3; the rules document; typed parsing; graph-wide resolve of types; check, infer and elaborate with generic instructions. Acceptance: the typed, rule, gradual and soundness tests of section 11.
6. **Typed execution.** Typed MIR, the instructions of section 8, constrained shapes, sealed instances, pinned arrays, two entries per function. Acceptance: the tests of step 5 at both levels; instructions and heap reported per program.
7. **Core image and snapshot.** The core image compiled once, the realm snapshot, application images bound to them. Acceptance: the image gates of [Planned work](planned.md).
8. **Reactive schedules.** Section 6.3. Acceptance: the reactive test of section 11 and the instructions of one signal update.
9. **TSX.** TSX parsing; static template parts as image constants. Acceptance: the same frames as the existing JSX build of the same application.
