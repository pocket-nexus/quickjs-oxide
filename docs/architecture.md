# Workspace architecture

The engine is three crates. `quickjs-oxide-core` (`core/`) holds the register bytecode, the heap and its collector, the loop that runs the bytecode, the builtins and the runtime that holds them. `quickjs-oxide-regexp` (`regexp/`) is the regular-expression engine and the Unicode tables. The root crate `quickjs-oxide` (`src/`) holds what the core does not: the front end that parses JavaScript into QuickJS-shaped stack code, the register lowering that turns that code into the core's register code, and the embedding API over the core. The command line, the WebAssembly build, the host adapters and the Test262 runner use the engine only through that API. The core's design record, with its rooting rule and its module layout, is [`core/README.md`](../core/README.md).

## Crates

The three engine crates and the workspace members that consume them:

| Crate | Path | Depends on | Content |
| --- | --- | --- | --- |
| `quickjs-oxide-core` | `core/` | `quickjs-oxide-regexp` | values, heap and collector, shapes, register bytecode, the loop, the builtins, the `Host` trait, `Runtime` |
| `quickjs-oxide-regexp` | `regexp/` | nothing | pattern compilation and matching over UTF-16 code units; the checksum-pinned Unicode 17 tables |
| `quickjs-oxide` | `src/` | `quickjs-oxide-core`, `quickjs-oxide-regexp`, `num-bigint`, `num-traits` | the front end, the register lowering, the embedding API |
| `quickjs-oxide-cli` | `apps/cli` | the root crate, `quickjs-oxide-host` | the `qjs` binary |
| `quickjs-oxide-web` | `apps/web` | the root crate, `quickjs-oxide-web-host` | the WebAssembly bindings the site runs |
| `quickjs-oxide-host` | `adapters/native` | the root crate | `SystemHost`: the system clock and the TZif/POSIX time-zone rules |
| `quickjs-oxide-web-host` | `adapters/web` | the root crate | `WebHost`: the browser's clock, time zone and random source |
| `quickjs-oxide-test262` | `conformance/test262` | the root crate | the process-isolated `run-test262` runner |

The core's and the regular-expression crate's libraries are named `oxide_core` and `oxide_regexp` in Rust code. Every crate but the two web packages and the Test262 runner is published to crates.io ([releases](release.md)).

Dependencies point one way: `quickjs-oxide-core` depends only on `quickjs-oxide-regexp`, has `#![forbid(unsafe_code)]` and reaches the platform through its `Host` trait. `num-bigint` and `num-traits` serve the front end's BigInt literals and the API's `JsBigInt`.

## Source owners

The core's modules are listed in the "Layout" table of [`core/README.md`](../core/README.md#layout). The root crate:

```text
src/
  lib.rs           the crate root: `engine`, `source`, the version constants
  source/          exact source bytes, offsets, QuickJS line/column mapping;
                   `unicode` wraps quickjs-oxide-regexp's tables for JsString
  engine/
    compiler/      the front end: lexer, parser, scopes and name resolution,
                   closure capture, stack-code emission, local optimizations
      register/    the lowering: stack code to quickjs-oxide-core register code,
                   published into the core's code store
    code/          the front end's output: stack instructions, the opcode
                   table, stack-size computation, unlinked function and
                   module records, debug positions
    value/         the front end's primitives: JsString, JsBigInt, constant
                   values and the Number-to-string conversion
    hir/           the HIR schema generated from contracts/spec/hir.ts: its
                   types, binary reader and writer, text dump and strip
    api/           the public embedding boundary; `api::core` is the API over
                   quickjs-oxide-core, `api::error` the front end's diagnostics
```

`engine::api` is the only public engine module; `compiler`, `code`, `value` and `hir` are crate-private. `scripts/checks/check-source-layout.py` enforces this layout and the API's re-export boundary.

## Host boundary

The core asks its embedder for one trait, `oxide_core::host::Host`, which the API re-exports as `api::core::Host`:

| Method | Used by | Default |
| --- | --- | --- |
| `write(stream, text)` | `print`, `console.log`, the `qjs` helpers | the process's stdout and stderr |
| `now_ms()` | `Date.now`, `new Date()` | the system clock |
| `local_tz_offset_minutes(utc_ms)` | local-time `Date` methods | UTC |
| `random_seed()` | the initial `Math.random` state | derived from the clock |
| `promise_rejection(event, promise)` | `HostPromiseRejectionTracker` (`Rejected`, `Handled`) | ignored |
| `read_file(path)` | `load` and `read` under `qjs --std` | the process's file system |
| `interrupt()` | the loop's interrupt poll, about every thousand back-edges and calls | never interrupts |

The core's own `SystemHost` uses every default. `adapters/native`'s `SystemHost` takes the time zone from the host's TZif/POSIX rules (`TZ` or `/etc/localtime`, reloaded as QuickJS's `localtime` does); `adapters/web`'s `WebHost` reads the browser's `Date` and `Math.random` and discards output; the page that embeds it shows results itself.

Compilation and module loading are function pointers on the core's services, because the host object lives inside the runtime they borrow. The API installs the front end as the core's compile function (`set_compiler`) and its module shims as the core's `ModuleHost`; an embedder supplies module-loading policy through the API's `ModuleLoader` trait (`normalize`, `load`), whose callbacks receive a `ModuleScope`. A host function, the embedder's Rust closure as a JavaScript function, is a core native closure whose body the API wraps: the core lends it a `NativeCx` while it runs, and the API turns that into the closure's `Scope`. Applications choose input, output, module policy and when the job queue drains. [`src/engine/api/README.md`](../src/engine/api/README.md) describes the API's types.

## Compilation

A source passes four stages:

1. **Front end** (`src/engine/compiler`). The parser emits `IrOp`s into a recursive `FunctionIr` tree; identifier resolution runs child-first once every scope is known, as QuickJS's `resolve_scope_var` does; then `lowering` emits QuickJS-shaped stack `Instruction`s, `optimize` applies bounded local rewrites, and `compute_stack_size` (`code/bytecode.rs`) computes each function's maximum operand-stack depth and enforces QuickJS's stack bound. The result is an `UnlinkedFunction` tree (or an `UnlinkedModule` with its request, import and export tables) whose operand-stack depth is static at every instruction.
2. **Register lowering** (`src/engine/compiler/register`). `analysis` records the stack depth, the active catch regions and the join points at every reachable instruction. The lowering then walks each function with a symbolic stack: register 0 is `this`, then the parameters, the locals and one temporary per stack depth, so the stack depth is the register allocator. Captured bindings become cells. A function whose locals or operand stack pass the 256-register window keeps the rest in the slots of a spill scope, reached through scratch registers at the top of the window. A `+` chain that starts with a string constant and continues with constants becomes one `ConcatN` over the operands' temporaries. The lowering builds each function as an `oxide_core::bytecode::CodeUnit` with the core's `CodeBuilder`, which numbers the inline-cache sites.
3. **Publication** (`quickjs-oxide-core`). `Compiler::publish` runs the core's validator over each unit once, children first, and assigns its inline-cache entries. A unit published through `Runtime::compiler()` (the program, its modules, and by default whatever the embedder compiles) goes into the `CodeStore` and lives as long as the runtime; code compiled at run time (`eval`, `new Function`, `$262.evalScript`) and code the embedder compiles through `Runtime::transient_compiler()` (the API's `transient` option) is published into the heap, which frees it once no function, frame or job reaches it ([The heap](heap.md#code-compiled-at-run-time), [code lifetime](../src/engine/api/README.md#code-lifetime)). The loop runs a published unit's instructions as they are.
4. **Instantiation**. `instantiate` allocates one `FunctionBytecode` record per unit (its constant table and its children) under one safepoint and returns the script's function object, which the API runs with `Runtime::run_script`. A module's tree is described to the core's module records (environment cells, imports, exports) and linked and evaluated by the core.

Debug positions follow the same path: the front end's pc-to-source table becomes the unit's sparse position table, from which errors take `stack`. `DebugInfoMode` (`Runtime::set_debug_info_mode`) chooses what a newly compiled function keeps: `Full`, `StripSource` (positions without source text) or `StripDebug` (neither). `Runtime::disassemble` (`qjs --disassemble`) prints the register code of a source.

## The loop

`oxide_core::vm` runs register code. A frame owns a window of one register file; a call's window starts on the callee's operands inside the caller's, so no argument is copied. Calls, returns and `new` on bytecode functions stay in one loop (`vm::interp::run_at`), which matches on the typed opcode of 8-byte `Insn`s, keeps `Int32` and double fast paths inline and moves slow paths out of line (`ops`, `property`, `rare_op`). Named property access, globals and constructor sites go through inline caches stored beside the register file. A throw unwinds through each unit's handler table.

Natives are Rust functions that read their receiver and arguments from the register file and call back into JavaScript through the re-entrant `call` and `construct`, which run a nested loop above the caller's frames; such re-entries share a native-stack budget measured from stack addresses. Generators and async functions suspend by copying their register window into a heap record and resume by copying it back. [Execution](performance/execution.md) describes the loop, the cache kinds, the iteration fast path, suspension and the native ABI in depth.

## The heap and the collector

`oxide_core::heap` owns every record. Values are 8-byte NaN-boxed words; objects, shapes, strings, symbols, BigInts, cells, functions, bytecode records and internal records each live in a typed arena with side live and mark bitmaps that allocation reuses lowest index first. Property values, dense elements and scope slots live in the value heap, one `Vec<Value>` allocated in ranges of powers of two and three times powers of two, coalesced at every collection. Shapes form a transition tree with a dictionary mode and prototype validity cells, which the inline caches test.

The collector is a non-moving mark-sweep. Collection happens only inside an allocating heap call (a safepoint) or on an explicit `Runtime::run_gc`; the rooting rule of [`core/README.md`](../core/README.md#the-rooting-rule) says what a Rust local must root across one. The trigger keeps reserved memory within a budget, the live bytes times one plus the growth factor (2 by default) and at least 1 MiB: a collection runs when the bytes allocated since the last one would take the bytes in use past it, or when a growth step would take reserved memory past it; under `GcPolicy::Manual` collections run only at the cap and on request. A heap cap (`RuntimeOptions::heap`) makes an allocation that would cross it collect first and fail with `OutOfMemory` if that frees too little. The `gc-stress` feature collects at every safepoint and checks every access. `Runtime::memory_report` reads what every arena, the value heap, the code and the caches hold without collecting. [The heap](heap.md) describes the arenas, the value heap, shapes and the collector in depth.

## Language surface

The core runs the language the front end accepts, with the five differences from QuickJS that the `TARGET-*` entries of the [deviation ledger](deviations.md) define: `with`, `Symbol.unscopables`, direct `eval` (it runs as indirect `eval`), one realm per runtime, and `Symbol.species`. Source and module inputs keep exact bytes and positions, and strings keep UTF-16 code units. Jobs own their pending computations; the application drains the queue. The [status page](status.md) lists language coverage and the verification commands.
