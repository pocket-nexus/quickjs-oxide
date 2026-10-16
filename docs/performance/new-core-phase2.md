# New core, phase 2: register bytecode and main loop

This page is the implementation plan for phase 2 of the new core tracked in issue #122 ("fixed-width instruction structs; register allocation in the compiler back end; register file and frame table; in-loop calls/returns/Base construction; local PC; bool fast paths; immediate variants; compare-jump fusion; smi fast paths"). Like the [phase 1 plan](new-core-phase1.md) it is written for whoever implements the phase: it fixes the decisions that are made, lists the ones that are deliberately open, and gives every milestone an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

Read first: [#122](https://github.com/pocket-nexus/quickjs-oxide/issues/122) for the design and the phase order, [phase 0](new-core-phase0.md) for the comparison base, the dropped language features and the stress-mode specification, [phase 1](new-core-phase1.md) and its [results](new-core-phase1-results.md) for the heap this phase runs on, [`core/README.md`](../../core/README.md) for the rooting rule, [Current execution paths](roadmap.md) for what the current VM does, and [Measurement method](measurement.md).

## 0. Scope

**In scope.** A fixed-width register instruction set in `oxide-core`; a lowering from the compiler's resolved IR to that instruction set, in the root crate; the register file, the frame table and the main loop; calls, returns and `new` inside the loop; scopes for captured variables; exceptions; a `qjs`-compatible driver binary with a `print` host function; the collector tracing the register file; the measurements of section 9.

**Out of scope.** No builtin is ported: the only native is `print`. No inline cache exists: every property access goes through `Heap::lookup_own` and the prototype walk (phase 3). The language surface is the one the micro-workloads and the eight V8 v7 programs need (section 3.4); generators, async functions, classes, destructuring, iterator protocols, modules, `eval`, template literals, regular-expression literals and private names lower to an explicit `Unsupported` error until phase 4. Numeric regions and the execution-only superinstructions of the current core are not carried over (#122); the measurements of section 9 say whether any of them are wanted back.

**What phase 1 learned that shapes this phase.** The [allocation-ledger comparison](new-core-phase1-results.md#m5-micro-workloads) separates the current core's cost per iteration into heap phases and interpreter phases:

| Instructions per iteration | `{a:i,b:i}` | `new P(i)`, two `this.x =` |
| --- | ---: | ---: |
| Current core, heap phases (allocation, field define, reference-count release) | 1,657 | 1,209 |
| Current core, interpreter phases (call and return, loop exit and re-entry, loop, local copies) | 1,032 | 4,279 |
| New core, same heap work (phase 1, collector included) | 415 | 602 |

The heap layer is a 4.0× and 2.0× gain on these probes, not the 6–9× the per-probe ratios suggested; the rest of the gap is interpreter work. 78% of the constructor probe is the call, the loop exit and re-entry, the loop and the local copies. That share is what this phase replaces, so phase 2 is where most of the gap to QuickJS closes or does not. Two consequences are built in below: calls and returns stay inside the loop with no frame materialization (section 4.3), and the `new` sequence reserves the property range from the constructor's static `this.x =` count (section 3.3), because the ledger shows the new core's constructor case losing a third of its gain to the first property growing an empty range.

## 1. Location and the dependency edge

Phase 1 kept `oxide-core` free of any edge to the root crate. Phase 2 adds the edge in one direction only: the root crate `quickjs-oxide` depends on `oxide-core`. The lowering of section 3 is the only code that sees both the compiler's types and the new instruction set, and it lives in the root crate. `oxide-core` keeps `#![forbid(unsafe_code)]` and gains no dependency.

| Module | Crate | Content |
| --- | --- | --- |
| `bytecode` | `oxide-core` | `Insn`, `Opcode`, `CodeUnit`, the handler table, the publication validator, the disassembler |
| `vm` | `oxide-core` | register file, frame table, the main loop, slow paths, exception unwinding, the native ABI |
| `compiler::register` | root crate | the lowering from the resolved `FunctionIr` to `CodeUnit`s, and the tests that compare both cores |
| `src/bin/oxide-run.rs` (or the location the source-layout check admits) | root crate | the driver: compile with the existing front end, lower, run on the new core |

Gates that change in the first milestone, deliberately and in the same pull request: `scripts/checks/test262-host-metadata.py` lists `oxide-core` as a production dependency of `quickjs-oxide` (today its expected set is empty, which is what failed #135's CI until `oxide-core` was admitted as a package); `scripts/checks/check-source-layout.py` admits the new modules. Candidate A of the host-boundary check still holds: providers depend on the engine, never the reverse, and `oxide-core` depends on nothing.

Delivery: one pull request per milestone, stacked in milestone order, each with its acceptance check in the description, gated by `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test -p oxide-core --features gc-stress` and, from M2 on, the Test262 focused and full runs of the current engine, because M2 touches the shared compiler.

## 2. Instruction format

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Insn { pub op: Opcode, pub a: u8, pub b: u8, pub c: u8, pub imm: u32 }

#[repr(u8)]
pub enum Opcode { Mov, LoadInt, LoadConst, LoadUndefined, /* … */ }
```

- 8 bytes, `size_of::<Insn>() == 8` as a compile-time assertion. `op` is the typed enum, not a `u8` to decode: the lowering builds `Insn` values in Rust, so there is no byte parsing, no width prefix and no opcode validation at run time. `decode_published` and its `Result` have no counterpart.
- `a`, `b`, `c` are register operands or small counts; `imm` is a constant-pool index, a jump target (an instruction index, absolute), a `PropertyKey`'s bits, a scope depth and index pair, or an `i32` literal.
- **Registers.** A frame owns a contiguous window of the register file: register 0 is `this`, then the parameters, then the locals, then the operand temporaries (section 3.1). Register operands are 8 bits, so a frame addresses 256 registers directly. A function that needs more keeps its first 256 registers addressable and reaches the rest only through `LoadWide a, imm` and `StoreWide imm, a`, which the lowering inserts around every use; `this`, the parameters and the earliest locals land in the window, so the shuttle costs only the large asm.js-style functions of Mandreel and PdfJS. No prefix instruction, no second encoding.
- **Immediate variants.** `AddI`, `SubI`, `MulI`, `BitAndI`, `ShlI`, `SarI`, `ShrI` and the comparison-jumps `JLtI` and friends take an `i32` in `imm`; the lowering emits them when the stack code pushes `PushI32` straight into the operation.
- **Compare-jump fusion.** `JLt a, b, imm` and the other five relational and four equality forms jump when the comparison holds, with the register-register and register-immediate variants; `JTrue`, `JFalse`, `JUndefinedOrNull` and `Jmp` cover the rest. A comparison whose result is consumed as a value, not a branch, uses the plain `Lt a, b, c` form.
- **Code units.** A `CodeUnit` is `{ code: Box<[Insn]>, handlers: Box<[Handler]>, register_count: u16, parameter_count: u16, flags, children: Box<[CodeUnitId]>, debug }`. Code units live in a `CodeStore` (a `Vec<CodeUnit>`) owned by `Runtime` *next to* the heap, not inside it: the main loop holds `&[Insn]` for the running function across every heap call, which a borrow of the heap's own arena could not allow. Publication appends to the store outside the loop (the driver, and later `new Function` and indirect `eval`), and the store is append-only in phase 2; code units are not collected. The heap's `FunctionBytecode` record (phase 1's reserved seam) holds the `CodeUnitId` and the constant table as a value-heap range, so constants are traced like everything else; a closure's children are reached through their parent's unit.
- **Validation at publication.** A validator runs once per code unit: every register operand is below `register_count`, every target is inside the code, every constant and child index is in range, every handler range nests, every wide index is below `register_count`. The loop then indexes with plain bounds-checked `[]` and nothing else; a validator failure is an internal error, never a JavaScript exception.
- **Disassembler.** `bytecode::disassemble` prints one instruction per line with symbolic registers (`this`, `p0`, `l3`, `t2`) and is used by the lowering tests and by the driver's `--disassemble`.

**Acceptance (M1).** Size assertion; a round-trip test that builds every opcode through the lowering API and disassembles it; validator tests for each rejected malformation; the `CodeStore` and `FunctionBytecode` attach path under `gc-stress`.

## 3. Lowering: from the resolved IR to register code

### 3.1 What the compiler provides

The phase 1 preview (its section 11) assumed a 42-variant expression IR to allocate registers from. The compiler's IR is not that: `FunctionIr` holds a linear `Vec<SpannedIrOp>` whose operations are almost all `IrOp::Bytecode(Instruction)`, the stack instruction itself, around a handful of symbolic operations that `resolution.rs` rewrites in place (`Identifier` and `IdentifierReference` by binding, `EnterScope`, `LeaveScope`, `PushConstant`, `MakeClosure`, `GlobalSet`, `CapturedLexicalSet`, `TemplateCall`, `EvalCall`, `PrivateField`). After `resolve_identifiers` the sequence is QuickJS-shaped stack code (about 200 opcodes) over `GetLocal`/`GetArg`/`GetVarRef` bindings, and `compute_stack_size` (`code/bytecode.rs`) proves that the operand-stack depth at every instruction is static, including across `Catch` and `Gosub` edges.

Phase 2 therefore does not write a new IR. It lowers the resolved stack sequence directly, with the stack depth as the register allocator:

- **Register map.** `this` is register 0; parameter `i` is register `1 + i`; local `j` is register `1 + parameters + j`; operand-stack depth `d` is temporary register `T(d) = 1 + parameters + locals + d`. `register_count` is that base plus the maximum depth `compute_stack_size` reports. Everything is decided at compile time; nothing is renamed at run time.
- **Symbolic stack.** The lowering walks the sequence with a symbolic stack whose entries are `Temp(d)` (the value is in `T(d)`), `Reg(r)` (the value is still in local or parameter register `r`, pushed by `GetLocal`/`GetArg` and not copied) or `Const(k)` (pushed by `PushI32`/`PushConst`, not materialized). A consumer pops its operands and reads them where they are: `GetLocal x; GetLocal y; Add` becomes one `Add T(0), x, y`; `GetLocal i; PushI32 1; Add; PutLocal i` becomes `AddI i, i, 1`. The stack-shuffling opcodes (`Dup`, `Swap`, `Nip`, `Drop`, `Insert2..4`, `Perm3..5`, `Rot4Left`) rewrite the symbolic stack and emit nothing unless an entry must move.
- **Materialization rules**, the two that make this translation correct: (1) before any jump, any jump target, any instruction that may throw inside a handler range, and any `Call`, every symbolic entry is materialized into its canonical `T(d)`, so every control-flow join sees the same layout; (2) a write to local `r` (`PutLocal`, `SetLocal`, `Initialize*`) while `Reg(r)` sits anywhere in the symbolic stack first materializes those entries, so `x + (x = 2)` reads the old `x`. Rule 2 is the one the tests of M2 target first.
- **Retargeting.** When `PutLocal r` follows an instruction whose destination is `T(d)` and that temporary has no other use, the lowering rewrites that instruction's destination to `r` instead of emitting `Mov r, T(d)`. This is the only peephole in phase 2; whether it is worth its complexity is decided by M3's measurements (section 10).
- **Bindings.** `BindingStorage::Argument` and `Local` map to registers. A local captured by an inner function cannot be a register: resolution already records, per inner function, the parent bindings it captures (`closure_variables`, `BindingStorage::External`), and the lowering uses that to mark each binding *captured* or *register*. Captured bindings live in a heap `Scope` object (phase 1's `ObjectKind::Scope`), one per function activation and one per block that declares a captured lexical binding, created at `EnterScope` and dropped at `LeaveScope`; a `for (let …)` body whose bindings are captured gets a fresh scope per iteration, as the specification's per-iteration environment requires. `GetVarRef`/`PutVarRef` on a captured binding become `LoadScope a, imm = (depth, index)` and `StoreScope`; the function's own captured locals are depth 0. Uncaptured bindings in a block need no scope at all, which removes the `SetLocalUninitialized` per block entry that #126 notes.
- **TDZ.** A lexical binding starts as `Value::EMPTY`. `GetLocalCheck` lowers to the load plus `CheckInit a`, which throws `ReferenceError` on `EMPTY`. Phase 2 emits the check wherever the current compiler does (every lexical read, #126 §1); elision where the read is dominated by the initialization is an M3 option, decided by `local_read` and `numeric_local_loop` (section 10).
- **Globals.** `GetVar`/`PutVar`/`GlobalSet` become `GetGlobal a, imm = key`, `SetGlobal`, `DefineGlobal`, resolved by an ordinary property lookup on the global object; the global IC is phase 3.
- **`finally` and `gosub`.** The stack code implements `finally` with `Gosub target` and `Ret`. The register form keeps the shape: `Gosub a, imm` stores the return instruction index in register `a` as an `Int32`, `Ret a` jumps to it. No block duplication.
- **Exceptions.** `Catch target` and `DropCatch`/`NipCatch` become entries in the unit's handler table: `Handler { start, end, target, depth }`, where `depth` is the symbolic-stack depth the handler resumes at; the thrown value lands in `T(depth)`. The loop unwinds by table lookup (section 4.4).

### 3.2 Calls

`Call argc`, `CallMethod argc`, `TailCall`, `Construct argc` and `Return` lower to:

| Register form | Operands | Convention |
| --- | --- | --- |
| `Call a, b, c` | `a` destination, `b` callee register, `c` argc | the callee value is in `b`, `this` in `b + 1`, arguments in `b + 2 …` |
| `CallMethod a, b, c` | same | the receiver was left in `b + 1` by the `GetField2` sequence |
| `New a, b, c` | same | `b + 1` receives the constructed object |
| `Return a` / `ReturnUndefined` | | writes into the caller's `a` |

The materialization before a call (rule 1) already places callee, receiver and arguments in consecutive temporaries, so the callee's window starts at the caller's `b + 1`: its register 0 is the receiver and its parameters are the caller's argument temporaries. No argument is copied. Missing arguments are filled with `undefined` by the callee prologue; extra arguments are left where they are. Tail calls reuse the window. The `arguments` object and rest parameters are phase 4.

### 3.3 Allocation forms

`Object` lowers to `NewObject a, imm = slots`, where `imm` is the number of `DefineField`s that follow in the literal; `ArrayFrom n` to `NewArray a, imm = n` plus `DefineElem`. For `New a, b, c` on a bytecode constructor, the receiver is allocated with the slot count recorded on the callee's code unit: the lowering counts the distinct `this.x = …` property names in the constructor body and stores it as `CodeUnit::this_slots`. That number is the ledger finding of phase 1 turned into an operand; the shape-based constructor cache that refines it is phase 3.

### 3.4 Coverage

Lowered in phase 2: locals, parameters and captured variables; every arithmetic, bitwise, comparison and logical operator; `typeof`, `void`, `delete`, `in`, `instanceof`; `if`, loops, `switch`, labelled `break`/`continue`; function declarations and expressions, arrow functions, closures, `return`, `this`, `new`; property get, set, define and delete by name and by computed key; object and array literals; `try`/`catch`/`finally`, `throw`; globals; `print`. Deferred to phase 4 with an `Unsupported(construct)` error from the lowering: generators, async, `await`, `yield`, classes, `super`, private names, destructuring, spread, `for-in`, `for-of`, iterators, template literals, tagged templates, regular-expression literals, `arguments`, rest parameters, modules, `eval`, `new Function`, `with` (dropped, phase 0). Phase 3 adds whatever the eight V8 v7 programs need from the deferred list, and phase 4 completes it.

**Acceptance (M2).** The symbolic-stack rules under a test corpus: every program in `src/engine/compiler/tests` that stays inside the phase 2 surface, plus hand-written cases for rule 2 (`x + (x = 2)`, `x = x++ + x`), per-iteration scopes (`for (let i…) fns.push(() => i)`), `finally` on every exit path, and nested handler depths. Each case runs on both cores through the driver and the current `qjs` and must print identical output; the register-count of every lowered unit equals `1 + params + locals + max depth`. The current engine's Test262 focused and full runs are unchanged, because only the new consumer of the compiler is added and the one shared change (the captured bit on bindings) is read, not acted on, by the old lowering.

## 4. The VM

### 4.1 State

```rust
pub struct VmStack { registers: Vec<Value>, frames: Vec<Frame>, top: u32 }   // inside Heap, a fixed root
pub struct Frame { unit: CodeUnitId, function: ObjectId, scope: Option<ScopeId>,
                   base: u32, return_pc: u32, return_dst: u8, flags: u8 }    // 24 bytes
```

The register file and the frame table live inside `Heap` beside the `RootStack` and the handles, so a safepoint inside `Heap::alloc_*` marks `registers[..top]` and every frame's function and scope with no extra parameter. That is phase 0 §3's "from phase 2 on the register file and frame headers". Dead temporaries above a frame's live depth may hold stale values; the collector treats them as live (a conservative over-approximation that keeps the loop free of clearing stores), and the sweep's poisoning under `gc-stress` still catches any read of a freed object through them because the object itself is marked only if some live register reaches it.

The register file grows by doubling when a call needs more room, up to a stack limit that turns into `RangeError: Maximum call stack size exceeded`; indices stay valid across growth, which is why the window scheme needs no pointer fix-up.

### 4.2 The loop

```rust
pub fn run(runtime: &mut Runtime, entry: ObjectId, this: Value, args: &[Value]) -> Result<Value, Throw>
```

- `pc`, `base` and `code: &[Insn]` are locals of the loop; `code` is borrowed from `runtime.code` (the `CodeStore`), which is disjoint from `runtime.heap`, so the borrow survives every heap call. On every call and return the loop reloads `code` and `base` from the frame.
- Dispatch is one `match insn.op` on the typed enum. Operands are read as `regs[base + a]`; the only checks on the hot path are the bounds checks safe Rust keeps, which M6 counts in the assembly as phase 1 did for the property hit.
- Fast paths are inline and return `bool`: `Add` with two `Int32` operands adds with overflow check and writes; with two numbers it writes the double; anything else calls `add_slow(&mut heap, a, b) -> Result<Value, Throw>`, which is `#[inline(never)]`. `Result` appears only on slow paths. Integers that fit stay `Int32`, so loop counters and array indices keep the smi path.
- Comparisons fuse with their jumps; a taken jump sets `pc = imm`, nothing else.
- No `profile!`, no cursor, no transaction, no owner records: a value in a register is a `Copy` word and the heap owns everything it points to.

### 4.3 Calls, returns and `new` inside the loop

`Call`: read the callee from `regs[base + b]`; if it is a bytecode function, push a `Frame { base: base + b + 1, return_pc: pc + 1, return_dst: a, … }`, grow the register file if the callee's `register_count` does not fit, fill missing parameters with `undefined`, coerce a sloppy function's `this` from `undefined`/`null` to the global object (one branch on a unit flag), set `code`, `base`, `pc = 0`, continue the loop. `Return a`: copy `regs[base + a]` into the caller's `return_dst`, pop the frame, restore `code`, `base`, `pc = return_pc`, continue. Neither path leaves the loop, allocates, or touches a `Result`. `New` on a bytecode constructor allocates the receiver first (section 3.3, a safepoint, with callee and arguments already in registers and therefore rooted), then proceeds as a call with the `construct` flag; a constructor's `Return` of a non-object yields the receiver. Native callees (only `print` in phase 2) take `fn(&mut Runtime, CallSite { base, argc }) -> Result<Value, Throw>` and read their arguments by index, because a slice of the register file cannot be borrowed while the runtime is mutably held; natives that call back into JavaScript are phase 4's Step/Resume machines and are not designed here.

### 4.4 Exceptions

A slow path returns `Err(Throw(value))`. The loop looks up the current frame's handler table for `pc`; if an entry covers it, `T(depth)` receives the value and `pc = target`; otherwise the frame is popped and the search continues in the caller. The root value is held in the `RootStack` for the duration of the unwind. The entry frame with no handler returns `Err` to the driver. There is no Rust unwinding, no `?` in the loop body, and the frame table is the only state the unwind touches.

### 4.5 Rooting rules for the VM

The phase 1 rule stands: an index held in a Rust local across a call that may allocate must be reachable from a root. In the loop this means: operands are read from registers immediately before use and results are written immediately after; a slow path that allocates receives its inputs as `Value`s that are still in their registers, so they are rooted; a slow path that needs a value *after* an allocation and no longer has it in a register pushes it on the `RootStack`. The stress mode (`--gc-stress=1` in the driver) makes a violation deterministic on the first test that exercises the path.

**Acceptance (M3, M4).** M3: `Mov`, constants, arithmetic, bitwise, comparisons, jumps, `CheckInit`, globals, properties by name and index, object and array literals, `typeof` and the unary operators, with tests per opcode; the arithmetic probes of section 9 recorded. M4: calls, returns, `new`, closures, scopes, `Gosub`/`Ret`, the handler table, the stack limit, `print`; the call and closure probes recorded; the whole suite green under `gc-stress`; a test that a collection during a call with 200 live frames preserves every register.

## 5. The driver

`oxide-run file.js` compiles the file with `compile_unlinked_script_with_filename`, lowers every function in the tree, installs `print` on the global object and runs the script; it exits non-zero with the exception's string on an uncaught throw, and with `unsupported: <construct>` when the lowering declines. Options: `--disassemble` (print every unit and exit), `--gc-stress=N` (in `gc-stress` builds, as phase 0 specifies), `--heap-cap-mib N`. It is the engine binary `scripts/benchmark/fixed.py` runs as `candidate=`, so the fixed probes and the ledger probes compare the two cores with the runner and the method the baseline used. Its stdout is byte-compared against the manifest like any engine.

## 6. Stress mode and CI

From this phase the stress mode covers JavaScript: `cargo test -p oxide-core --features gc-stress` (all VM tests), plus, in the `fast` CI job, the fixed probes at smoke size (`build_fixed_matrix.py --smoke`) and the M2 corpus through `oxide-run --gc-stress=1` built with the feature. Any change under `core/vm`, `core/bytecode`, `core/heap` or `compiler/register` runs them. The V8 v7 programs at one `run()` under stress join in phase 3, when they run at all.

## 7. 32-bit targets and memory

`Insn` is 8 bytes on every target; register-file and code-store indices are `u32`; a `Frame` is 24 bytes. Nothing in the loop holds a pointer into the heap, so the phase 5 snapshot still needs no rewriting. Per-frame cost on a 32-bit target is the same word count as on 64-bit.

## 8. Milestones

| # | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| M1 | `Insn`, `Opcode`, `CodeUnit`, `CodeStore`, handler table, validator, disassembler; the dependency edge and both gate scripts | section 2 | 2–3 days |
| M2 | the lowering: register map, symbolic stack, materialization and retargeting, captured bindings and scopes, TDZ, globals, calls, allocation forms, handlers, `Unsupported` | section 3.4 corpus on both cores; Test262 unchanged on the current engine | 7–10 days |
| M3 | the loop: register file, frames, dispatch, moves, constants, arithmetic with smi and immediate paths, compare-jumps, properties by name and index without IC, literals | per-opcode tests; arithmetic probes recorded | 4–6 days |
| M4 | calls, returns, `new` with `this_slots`, closures, scopes, `Gosub`/`Ret`, unwinding, stack limit, `print`, the native ABI | section 4 tests; call and closure probes recorded; `gc-stress` green | 5–7 days |
| M5 | the driver, `--gc-stress`, `--disassemble`, CI additions of section 6 | fixed probes and corpus run under the runner and under stress in CI | 2–3 days |
| M6 | reference-machine measurement | the tables of section 9 committed under `docs/performance` | 2 days |

Total: about 22–31 working days. M3 and M4 are one pull request if the loop cannot be tested without calls; M1 and M2 are always separate.

## 9. Measurement and exit

Every number is produced as in phase 1: reference machine, CPU pinned, serial, interleaved, raw samples kept, per-iteration slopes from two problem sizes with each engine's empty loop subtracted, `perf stat -e instructions:u`; Callgrind where a phase split is wanted. A driver `probes/new-core/phase2.py` runs both engines over the same JavaScript files. The tables below are the exit record.

**Table A: the fixed probes, both cores and QuickJS**, instructions per iteration (for the QuickJS and current-core columns the whole-process count of [baseline.md](baseline.md) divided by the iteration count, rounded, so startup is included; the slope for the new core):

| Probe | QuickJS | Current core | New core | Measures |
| --- | ---: | ---: | ---: | --- |
| `empty_loop` | 90 | 296 | | dispatch of a compare-jump and an increment |
| `numeric_local_loop` | 167 | 1,010 | | smi arithmetic in registers |
| `local_read`, `local_read_with_setup` | 130 | 833 | | a move between registers |
| `local_move`, `argument_move` | 133, 117 | 680, 458 | | register writes |
| `call0` | 388 | 2,238 | | call and return of an empty function |
| `prop_read`, `prop_write` | 200, 165 | 545, 960 | | the uncached property path (phase 3 adds the IC) |
| `array_read`, `array_write` | 230, 195 | 510, 1,410 | | dense elements |
| `tdz`, `type_error` | | | | `CheckInit`, unwinding |

The `bigint_*` probes are recorded and expected to regress until phase 4 (`ShortBigInt`, phase 1 §2). `numeric_coercion_loop` and `string_bridge` need `ToNumber` on strings and string concatenation, which phase 2 implements only for flat strings; they are recorded if the lowering accepts them.

**Table B: the phase 1 workloads as JavaScript on the new core**, against the ledger totals of the current core (`{a:i,b:i}` 2,689; `new P(i)` 5,488) and the phase 1 Rust numbers (415 and 602). This is the apples-to-apples number the phase 1 results page leaves to phase 2: heap and interpreter together, per probe, on the same JavaScript source. The closure probe (`let j = i; s = () => j`, current core 18,374) and the depth-3 prototype read (current core 687) are included.

**Table C: the loop itself.** (1) The size of `vm::run` from `nm --size-sort` on the release binary against the current core's 62–64 KB `execute_admitted_in_state`; (2) instructions per executed register instruction on `empty_loop` and `numeric_local_loop`, from the slope divided by the instruction count the disassembler shows for the loop body, against the current core's 61–82 per dispatch; (3) the layout experiment of #119 (identical code at several `.text` offsets) over `empty_loop`, `call0` and `numeric_local_loop`, reporting the spread; (4) the bounds-check branches on the `Add` and `JLt` hot paths counted in the assembly, as phase 1 M3 did.

**Exit condition.** Tables A–C and the M2 corpus result committed under `docs/performance`; `gc-stress` green on everything in section 6; the current engine's Test262 unchanged. As in phase 1 no target multiplier is written here: the number is measured first. The go/no-go of #122 is phase 3's, on the eight V8 v7 programs, and these tables are what phase 3's plan starts from. Two things would send the phase back to design rather than forward: a `vm::run` that is not well below the current loop's size, or a `call0` that does not beat the current core by a wide margin, since both say the design has re-grown the structure phase 2 exists to remove.

## 10. Decisions

Made, and not reopened inside phase 2:

- the root crate depends on `oxide-core`; the lowering lives in the root crate; `oxide-core` depends on nothing and stays `forbid(unsafe_code)`;
- `Insn` is 8 bytes with a typed `Opcode`; 8-bit register operands with a 256-register window and `LoadWide`/`StoreWide` shuttles beyond it; no prefix encodings;
- the lowering consumes the resolved stack sequence with the static stack depth as the register allocator; no new IR;
- `this` is register 0; callee windows overlap the caller's argument temporaries; no argument copying;
- code units live in a `CodeStore` beside the heap, append-only, not collected in phase 2; constants are a value-heap range;
- the register file and the frame table are inside `Heap` and are fixed roots; temporaries above the live depth are marked conservatively;
- exceptions are a handler table and an in-loop unwind; `finally` keeps `Gosub`/`Ret` with the return index in a register;
- numeric regions and execution-only superinstructions are not carried over;
- natives are leaf functions in phase 2; the Step/Resume native ABI is phase 4's;
- `New` reserves the receiver's slots from the constructor's static `this.x =` count.

Open, with the milestone that decides them:

- retargeting (section 3.1) kept or dropped: M3, by its effect on `local_move` and the lowering's test surface;
- TDZ-check elision where the initialization dominates: M3, by `local_read` and `numeric_local_loop`;
- the encoding of a compare-jump against a constant that is not an `i32` (a constant-pool index in `c` versus materializing): M3;
- the register-file growth policy and the stack limit value: M4, against the current engine's limit and the 32 MB cap of phase 1 M6;
- whether code units are ever collected: phase 4 (`new Function` churn) or phase 5 (snapshots);
- whether any current-core superinstruction is wanted back: M6, by Table A against the baseline.

## 11. Risks

- **Translation correctness.** The symbolic stack is the one piece of phase 2 with no counterpart in the current code. Mitigation: materialization rules 1 and 2 stated and tested first, the two-core output comparison over the compiler's test corpus, and `gc-stress` on the same corpus.
- **Register pressure.** Functions with more than 255 registers pay the wide shuttle. The lowering reports the count per unit, and M6 lists the Octane functions that exceed the window; if a hot one does, the window layout (which locals land below 256) is revisited in phase 3.
- **Loop size creep.** Every "small" case added inline grows `vm::run`. Table C (1) is a gate on every milestone pull request, not only M6.
- **Shared compiler changes.** The captured-binding bit touches resolution, which the current engine runs. Test262 focused and full gate M2 and every later pull request that touches `compiler/`.
- **The `&mut` structure.** Holding `code` across heap calls depends on `CodeStore` staying outside the heap; a later wish to collect code units must keep that split (an indirection table, not a move into the arena).
- **Residual safe-Rust cost.** Bounds checks on `regs[base + a]` and `code[pc]` are the estimate #122 calls "low tens of percent". Table C (4) measures it; if it dominates `empty_loop`, the debug-asserted `get_unchecked` helper #122 mentions becomes a phase 3 experiment, never a phase 2 change.
- **Per-iteration scopes.** Closures over loop variables are the semantic trap of the scope design; the M2 corpus tests them before any measurement is taken.

## 12. Seam with phase 3 (preview, not phase 2 work)

Phase 3 attaches inline caches to `GetField`, `SetField`, `GetGlobal` and `New` as a side table indexed by instruction index, so no operand is reserved for them now. The validity cells and the IC check of phase 1 M3 (32 instructions in Rust) are what those caches test. `CodeUnit::this_slots` is replaced by the shape-based constructor cache when it exists. Dense-array and typed-array fast paths land in the same loop, which is why its size is gated from the first milestone.

## 13. Implementation record

- **M1** (#136): as section 2, with the departures listed in [`core/README.md`](../../core/README.md#phase-2-departures) (literal compare-jumps carry an `i16`; negated relational jumps; handler entries name their register).
- **M2**: the lowering consumes the [`UnlinkedFunction`](../../src/engine/code/function.rs) tree, the compiler's finished stack code, instead of the resolved `FunctionIr`, so the shared compiler is not changed at all and the current engine's Test262 cannot move. Captured bindings are cells, not block scopes; call windows follow the stack operand order. Both are in [`core/README.md`](../../core/README.md#phase-2-departures). The acceptance corpus runs through the lowering and the publication validator; the two-core output comparison needs the loop and moves to M4, where it becomes the M4 acceptance.
- **M3, M4** (one pull request: the loop cannot be tested without calls, since `print` is one): the loop, calls, returns, `new`, closures, unwinding, the stack limit, `print` and the error constructors, as section 4, with the departures in [`core/README.md`](../../core/README.md#the-loop-m3-m4). The two-core comparison M2 deferred runs here over [`core/tests/corpus`](../../core/tests/corpus) and the fixed probes, with and without stress. Probe slopes, the loop's size and the decisions of section 10 assigned to M3 and M4 are in [the results page](new-core-phase2-results.md).
