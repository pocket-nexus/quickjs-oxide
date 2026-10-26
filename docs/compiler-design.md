# Compiler design

This page is the design of the compiler that turns JavaScript and a typed dialect of TypeScript into the core's register code. Its two goals are **less memory and more speed**: a smaller peak and resident heap, and fewer retired instructions per unit of work. It fixes the stages and the contract between them, the typed dialect and its guarantee, the boundary between typed and untyped code, the effect facts the optimizer uses, the two optimization levels, the bytecode and VM additions, the image format, the tests, and the order of the work. It builds on [Architecture](architecture.md), which describes the current stages, and on [Planned work](planned.md), whose code images and heap snapshots this design produces and loads.

## 1. Goals and constraints

| Constraint | Consequence |
| --- | --- |
| Memory and speed are the goals | Every stage is judged by its effect on the heap and on retired instructions; the [performance checks](../AGENTS.md#performance-checks) gate every change |
| JavaScript keeps QuickJS's behavior | JS mode is measured against QuickJS with Test262, `scripts/frontend/diff.py` and the position fixtures |
| Typed code needs sound types | The compiler parses, checks and infers its own dialect; the TypeScript checker plays no part |
| `quickjs-oxide-core` forbids `unsafe` | The engine interprets bytecode; the worst outcome of a wrong image is a panic |
| Applications ship as images | The ahead-of-time driver runs on the host; the front end is a component the embedder links or leaves out (section 2) |
| The distributor sees the whole program | Ahead-of-time compilation takes the application and the core image it targets as one module graph |

## 2. Two drivers

The stages of section 3 serve two drivers.

| | Runtime driver | Ahead-of-time driver |
| --- | --- | --- |
| Runs in | the engine: `qjs` with source, `eval`, the `Function` constructors, dynamic `import()` of source | the host or the distributor's build service |
| Accepts | JS mode | JS mode and typed mode |
| Unit of compilation | one script or one module, one function at a time | the module graph of an application plus the interface of its core image |
| Whole-program analysis (stage 5) | skipped | run |
| Optimization | the runtime set of section 7 | level 0 or level 1 of section 7 |
| Output | units published to the `CodeStore` | a code image (`.oxc`) and a debug table (`.oxd`) |

The runtime driver keeps the per-function streaming of the current MIR pipeline, so its peak memory is one function's graph; its compile time counts in every Sweep program and stays within the gates. The ahead-of-time driver holds every function of the graph at once, because stage 5 needs the call graph; its compile time and memory are the host's.

The embedder chooses whether a build contains the runtime driver. The core reaches the front end through its compile function (`set_compiler`) and loads images on its own, so an embedder that depends on `quickjs-oxide-core` alone runs images, and `eval`, the `Function` constructors and dynamic `import()` of source throw `InternalError: no compiler`. An embedder that also links the root crate runs JavaScript source as well.

## 3. Pipeline

```
source files
  │ 1  lexer / parser                     one file at a time
  ▼
AST + scope tree + declaration table
  │ 2  resolve                            one file (runtime) or the module graph (ahead of time)
  ▼
resolution tables: bindings, captures, TDZ, storage
  │ 3  AST → HIR                          mechanical translation
  ▼
HIR, state Resolved
  │ 4  check / infer / elaborate          JS-mode code passes through with every value dynamic
  ▼
HIR, state Checked                         ← the middle end accepts this state only
  │ 5  whole-program analysis             ahead of time only
  ▼
HIR + call graph + effect summaries + reactive schedules
  │ 6  MIR build                          one function at a time
  ▼
MIR: SSA with representations and an effect class per operation
  │ 7  optimization passes                by level
  │ 8  selection pass
  ▼
MIR in one-to-one correspondence with instructions
  │ 9  register allocation + emission
  ▼
register code → CodeStore (runtime) or .oxc + .oxd (ahead of time)
```

| Stage | Owns | Hands on |
| --- | --- | --- |
| 1 Parser | syntax for JS mode, typed mode and TSX; early errors in QuickJS's order; declaration registration; source offsets | the AST, the scope tree, the declaration table |
| 2 Resolve | lexical resolution of value and type names; module linking; captures; TDZ; the storage of each binding (register, closure cell, module slot, global) | the resolution tables |
| 3 AST → HIR | translation, with every decision taken from stages 1 and 2 | HIR in state Resolved |
| 4 Check | the typing rules of section 4; bidirectional local inference | a typed HIR, or type errors |
| 4 Elaborate | property access resolved to a slot or a shape-keyed access; static callees; overload choice; boundary checks at the points of section 5.4; the two entries of every typed function | HIR in state Checked |
| 5 Whole-program analysis | the call graph; devirtualization; reachability (unreachable functions and modules are dropped); effect summaries (6.2); reactive schedules (6.3) | facts recorded on HIR functions and call sites |
| 6 MIR build | SSA; representation types; explicit box and unbox; an effect class on every operation | one MIR function |
| 7 Optimization | the passes of section 7 | optimized MIR |
| 8 Selection | fused instructions, superinstructions, immediate forms | MIR whose operations are instructions |
| 9 Allocation and emission | registers, coalescing, loop rotation, call windows, encoding, the position table, the inline-frame table | register code and its tables |

### 3.1 Parser

The parser reads three syntaxes: JS mode, typed mode (TypeScript syntax under the rules of section 4) and TSX in typed mode. The ahead-of-time driver selects typed mode for `.ts` and `.tsx` modules and JS mode for every other module; the runtime driver reads JS mode. Early errors and their order match QuickJS in JS mode, because a program observes them. The parser records declarations and source offsets and leaves every storage decision to resolve.

### 3.2 Resolve

Resolve binds every value name and every type name to its declaration by lexical scope, links imports to exports, records captures and the TDZ checks each reference needs, and decides each binding's storage. In the ahead-of-time driver it runs in three passes over the module graph: collect every module's declarations and exports, link imports to exports, then resolve references. Property names (`p.x`) belong to the checker, because their meaning depends on types.

### 3.3 HIR

The HIR schema is generated from `contracts/spec/hir.ts`. It carries typed slots for every expression; JS-mode code fills them with `dynamic`. `hir::validate` checks two states: **Resolved** (every name bound, every capture recorded) and **Checked** (every typed slot filled, every boundary check placed, every property access resolved). Stages 5 and later accept Checked HIR only.

### 3.4 Check, infer, elaborate

The checker applies the rules of section 4 with bidirectional inference inside a function; a function exported from a module states its parameter and return types. Elaboration writes out what the source leaves implicit: the slot or shape-keyed access of every property, the static callee of every call the types determine, the boundary checks of section 5, and for every typed function a fast entry and a checked entry (section 5.3).

### 3.5 MIR

A MIR function is an SSA graph whose values carry a representation: a tagged `JsValue`, `i32`, `f64`, a reference to a known layout, or a scalar-replaced object. Box and unbox are explicit operations. Every operation carries an effect class (section 6.1). JS-mode functions build the same graph shape the MIR builder produces today.

### 3.6 Registers

Every VM register holds a valid `JsValue` at every instruction. Typed instructions read their operands without a tag check and write tagged results; an `i32` result is tagged by a constant OR. The collector therefore scans registers as values and needs no stack maps.

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

Typed mode uses TypeScript syntax with these rules; the rules document (section 14, step 6) states each as *TypeScript's rule → this dialect's rule → reason → runtime check*.

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

**Why this design.** Tightening keeps an object's identity, so an erased program and a compiled program share every object and the guarantee holds. It allocates nothing: a value crosses without a copy. It costs one walk the first time a value crosses at a type and one shape comparison every later time. Typed reads never re-check, so the optimizer treats a field read as a load from a known region. Untyped code pays only on objects that typed code has constrained, and only in the store handlers of their shapes. In the ahead-of-time driver, an interface-typed access site that stage 5 proves sees one layout becomes a fixed-slot load.

### 5.3 Functions across the boundary

Every typed function has two entries. The **fast entry** serves `CallDirect` from typed callers whose arguments the checker proved; it checks nothing. The **checked entry** serves every other caller: it checks each argument against its parameter type, tightening as section 5.2 describes, and continues at the fast entry. A typed function value handed to untyped code exposes the checked entry. An untyped function called from typed code is called through the generic call; its result is checked against the static return type.

### 5.4 Where checks happen

A boundary check sits at one of these points: the checked entry of a function, the return from a call to an untyped callee, the point where a `dynamic` value is assigned or passed to a typed binding or parameter, and a checked cast. The optimizer merges a check with an identical dominating check, and moves a check earlier only across operations whose effect class is free of observable effects (section 6.1). A `TypeError` therefore appears at the same point of the observable execution in every compilation of a program.

### 5.5 What optimization leaves observable

| Observable | Mechanism |
| --- | --- |
| frames in `Error.stack` | the inline-frame table: at a throw, the VM expands each inlined call into its frame |
| source positions in `Error.stack` | the position table in the runtime driver and the `.oxd` in the ahead-of-time driver; a device build prints function names with unit and offset, and the [deviation ledger](deviations.md) records this form |
| `Function.prototype.toString` | the `DebugInfoMode` of [Architecture](architecture.md#compilation) |
| the point of a `TypeError` | the check placement of section 5.4 |

## 6. Effect facts

Effect facts are what let the optimizer move, merge and delete operations. They come from the operations themselves and from whole-program inference; the source carries no effect annotations.

### 6.1 Effect classes

Every MIR operation carries an effect class built from these facts, derived from its operands' types and representations:

| Fact | Meaning |
| --- | --- |
| `reads(r)`, `writes(r)` | the regions it reads and writes: a register, a closure cell, a field of a layout, the elements of a pinned kind, or the whole heap |
| `calls_js` | it may run arbitrary JavaScript (a getter, a proxy trap, `valueOf`) |
| `throws` | it may throw |
| `suspends` | it may suspend the function (`await`, `yield`) |
| `allocates` | it may allocate |
| `host` | it calls the embedder |

A dynamic property read is `calls_js + throws + reads(heap) + writes(heap)`. A fixed-slot load from a proven non-null reference is `reads(field)`. Selection keeps each operation's effect class on the instruction it chooses.

### 6.2 Function summaries

The ahead-of-time driver computes, for every function, the union of its operations' facts and the set of parameters that escape. The computation runs bottom-up over the call graph, iterating each strongly connected component to a fixed point; an indirect call whose targets stage 5 cannot enumerate contributes every fact. A call site then carries its callee's summary as its effect class, so a field stays in a register across a call that writes other regions, a call that cannot throw has no exception edge, an argument that does not escape permits scalar replacement, and a call with an unused result and no effects is deleted.

### 6.3 Reactive schedules

The core image's reactive runtime exports `createSignal`, `createMemo` and `createEffect`. Applications import them from the core image's module, so the ahead-of-time driver identifies each call by its binding. At a `createMemo` or `createEffect` call whose argument is a function literal, stage 5 collects the signals the literal reads. When every path through the literal reads the same signals, the call is rewritten to the runtime's static form with that list, and the runtime skips dependency tracking for it; every other call keeps runtime tracking. The rewrite keeps the runtime's update order and batching, so a program draws the same frames either way.

## 7. Optimization levels

| | Runtime set | Level 0 (development) | Level 1 (release) |
| --- | --- | --- | --- |
| Driver | runtime | ahead of time | ahead of time |
| Stage 5 | — | call graph, effect summaries | all of stage 5 |
| Inlining | — | — | by size and call-graph position |
| Local passes | constant folding, branch folding | constant folding, branch folding | constant folding and propagation, dead-code elimination, global value numbering, CFG simplification |
| Loop and memory passes | — | — | loop-invariant code motion, escape analysis and scalar replacement, range analysis, check elimination |
| Selection | fused instructions, immediates | fused instructions, immediates | fused instructions, immediates, superinstructions |
| Variables | as in the source | every source variable live to the end of its scope | as liveness requires |
| Debugging | positions | positions, breakpoint and step hooks | positions, inline-frame table |

A program produces the same output at every level and under its erasure on QuickJS; section 11 tests this.

## 8. Bytecode and VM

| Addition | Purpose |
| --- | --- |
| `AddI32`, `SubI32`, `MulI32` with an overflow target; `AddF64` and the other `f64` arithmetic; typed comparisons and conversions | arithmetic without type dispatch |
| `LoadSlot`, `StoreSlot` | field access at a layout offset without a shape check |
| `CallDirect unit` | a call to a known unit through its fast entry |
| `CallHost id` | a call to an embedder function by index, with arguments as the host interface types them |
| `CheckDesc reg, descriptor` | the boundary check of section 5.2, tightening and pinning included |
| constrained shapes | per-property type constraints on shapes; store, define and delete handlers check them |
| sealed instances | class tag, fixed layout, fixed prototype, non-extensible |
| pinned arrays and dictionaries | an element type in the object header, checked by element stores |
| inline-frame table | stack-trace frames for inlined calls |

The host interface (the embedder's functions, their types and their indices) is a generated declaration that the checker reads and `CallHost` indexes. Asynchronous functions compile to state machines whose frames live on the heap; the embedder resumes ready tasks from its loop, and a task belongs to an owner scope whose disposal cancels it.

Images come from the build service, which signs them; the embedder's installer verifies the signature. The loader runs the publication validator of [Planned work](planned.md#4-code-images-and-heap-snapshots) on every image; the soundness of typed instructions rests on the compiler that produced the image.

## 9. Images

Images follow the format of [Planned work](planned.md#4-code-images-and-heap-snapshots): little-endian, position-independent, every reference a `u32` index, a build fingerprint. This design adds:

- **Execute in place.** Bytecode and constants are aligned and stay unmodified after load, so a loader maps them from read-only storage. The mutable parts (inline caches, interned strings, the link table to the core image) live in RAM.
- **Debug tables apart.** Position tables and the inline-frame table go to the `.oxd`, which stays on the host; a device reports unit and offset, and the host maps them back.
- **Core binding.** An application image records the fingerprint of the core image it was compiled against and refers to the core's functions and layouts by index. The loader accepts the application image when the fingerprints match; on a core update, the build service recompiles the application.

## 10. Runtime layering

```
application images      one runtime per application; discarded whole on exit or reload
core image + snapshot   the framework and system JavaScript, shared, read-only, executed in place
host interface          generated bindings: CallHost indices and their types
engine                  VM, loader, collector
embedder                drawing, layout, input, I/O, the loop
```

The embedder drives each runtime from its loop: once per tick it delivers input, drains the job queue and resumes ready tasks within a budget. The embedder calls into JavaScript only at these points. A reload discards the application's runtime with its heap and its host objects and starts a new runtime from the snapshot.

## 11. Tests

| Subject | Reference | Method |
| --- | --- | --- |
| JS mode | QuickJS | Test262, `scripts/frontend/diff.py`, the position fixtures |
| Typed mode | the erased program on QuickJS | equal output, or `TypeError` in a case of section 4.3 |
| Typing rules | the rules document | at least one accepted and one rejected program per rule |
| Gradual guarantee | the fully annotated program | random programs with a random subset of annotations removed produce the same output, with fewer `TypeError`s at most |
| Check placement | section 5.4 | programs whose side effects precede and follow a failing check, at both levels |
| Soundness | — | randomly generated typed programs are rejected or match their erasure |
| Levels | level 0 | level 1 and the runtime set produce the same output |
| Reactive schedules | runtime tracking | the same frames with the static form and with tracking |
| Images | the program run from source | compile to an image, load, run: same output; a flipped byte at every offset is rejected with `Err` |

## 12. Measurement

Every change to the engine runs the Sweep of [the agent guide](../AGENTS.md#performance-checks). The runtime driver's compile cost counts in every Sweep program, so its passes stay within the instruction gate. The ahead-of-time driver is measured by the programs it compiles: instructions and time of the image against the runtime driver's output, peak and live heap with `bench.py memory`, and image size. Its own compile time and memory are reported beside them.

## 13. Location

| Code | Stage |
| --- | --- |
| `src/engine/ast/parser` | 1 |
| `ast::Storage` | decided in 2 |
| `src/engine/hir/build.rs` | 2 and 3, as two modules |
| `src/engine/hir/schema.rs` (from `contracts/spec/hir.ts`), `hir/validate.rs` | the HIR and its two states |
| new `hir::check` | 4 |
| new `analysis` | 5 |
| `src/engine/mir/build`, `mir/op.rs` | 6, with effect classes |
| new `mir::pass` | 7 |
| `src/engine/mir/lower/alloc.rs` (`Plan`), selection | 8 and 9; `Plan` keeps register decisions |
| `src/engine/mir/compile.rs` | the two drivers |
| `quickjs-oxide-core` bytecode and VM | section 8 |

## 14. Order of the work

Each step has its own acceptance; steps 1, 3 and 4 leave every program's bytecode unchanged.

1. **One front end.** The AST, HIR and MIR pipeline becomes the only front end. Acceptance: the Test262 vector, `diff.py` and the Sweep.
2. **Images.** `encode_image`, `load_image` and `.oxd` with the execute-in-place layout; the core image and the realm snapshot. Acceptance: the image tests of section 11 and the image gates of [Planned work](planned.md).
3. **Resolve.** Stage 2 apart from stage 3; storage decided in resolve. Acceptance: identical bytecode, Sweep at ×1.0000.
4. **Pass framework.** Effect classes on MIR operations; `mir::pass`; `Plan`'s non-register annotations as passes and selection. Acceptance: identical bytecode, Sweep at ×1.0000.
5. **Ahead-of-time driver.** The module graph, stage 5 with effect summaries, and level 1's passes for JS mode. Acceptance: the level tests of section 11; instructions, heap and image size reported per program.
6. **Typed mode.** The rules document; typed parsing; graph-wide resolve of types; check, infer and elaborate with generic instructions. Acceptance: the typed, rule, gradual and soundness tests of section 11.
7. **Typed execution.** Typed MIR, the instructions of section 8, constrained shapes, sealed instances, pinned arrays, two entries per function. Acceptance: the tests of step 6 at both levels; instructions and heap reported per program.
8. **Reactive schedules.** Section 6.3. Acceptance: the reactive test of section 11 and the instructions of one signal update.
9. **TSX.** TSX parsing; static template parts as image constants. Acceptance: the same frames as the existing JSX build of the same application.

