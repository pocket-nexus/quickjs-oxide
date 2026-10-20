# Architecture

quickjs-oxide is a set of Rust crates. Three of them make up the engine; the rest are the programs and adapters built on its embedding API. Every crate in the workspace sets `unsafe_code = "forbid"`, so the engine contains no unsafe code.

This page follows a script from source text to a running program and names the crate that owns each step. [Optimizations](./optimizations.md) goes deeper into the inline caches, value representation, shapes and the collector.

## Crates

| Crate | Path | Content |
| --- | --- | --- |
| `quickjs-oxide` | `src/` | The front end (lexer, parser, scopes, stack-code emission), the register lowering and the embedding API |
| `quickjs-oxide-core` | `core/` | Values, the heap and its collector, shapes, register bytecode, the loop that runs it, the builtins, the `Host` trait and `Runtime` |
| `quickjs-oxide-regexp` | `regexp/` | Regular-expression compilation and matching over UTF-16 code units, and the Unicode 17 tables |
| `quickjs-oxide-host` | `adapters/native` | `SystemHost`: the system clock and the time-zone rules from TZif files and `TZ` |
| `quickjs-oxide-web-host` | `adapters/web` | `WebHost`: the browser's clock, time zone and random source |
| `quickjs-oxide-cli` | `apps/cli` | The `qjs` command line |
| `quickjs-oxide-web` | `apps/web` | The WebAssembly bindings and the browser demo |
| `quickjs-oxide-test262` | `conformance/test262` | The Test262 runner, `run-test262` |

Dependencies point one way. `quickjs-oxide-regexp` depends on nothing. `quickjs-oxide-core` depends only on `quickjs-oxide-regexp`. The root crate `quickjs-oxide` depends on both, plus `num-bigint` and `num-traits` for BigInt literals and BigInt values in the API. The command line, the WebAssembly bindings, the host adapters and the Test262 runner use the engine only through the root crate's API, `quickjs_oxide::engine::api::core`.

## From source to result

A script passes through four stages before it runs:

```text
source text
  │  front end (quickjs-oxide)
  ▼
QuickJS-shaped stack code, one unit per function
  │  register lowering (quickjs-oxide)
  ▼
quickjs-oxide-core register code
  │  publication: validated once
  ▼
function objects in the heap
  │  the loop (quickjs-oxide-core)
  ▼
completion value or exception
```

### Front end

The front end in `src/engine/compiler` parses Script or Module source and emits code for an operand stack, shaped like QuickJS's bytecode. It resolves every identifier once all scopes are known, applies local rewrites, and computes each function's maximum stack depth. The output is a tree of unlinked functions, or a module record with its imports and exports. Early errors carry QuickJS's messages and positions.

### Register lowering

The lowering in `src/engine/compiler/register` turns each function's stack code into register code for `quickjs-oxide-core`. Register 0 is `this`, followed by the parameters, the locals and one temporary per stack depth, so the stack depth chooses the register. Variables captured by closures become cells. A function with more locals and temporaries than its 256-register window keeps the rest in a spill area.

Each instruction is 8 bytes: an opcode, three byte operands and a 32-bit immediate.

### Publication

`quickjs-oxide-core` validates each code unit once, when it is published. Code compiled for the program and its modules lives as long as the runtime. Code compiled while the program runs (`eval`, `new Function`) lives in the heap and is freed once nothing reaches it. Instantiation then creates one bytecode record per unit and the function objects that share it.

### The loop

One loop, `run_at` in `oxide_core::vm`, runs register code. A frame owns a window of one shared register file, and a call's window starts at the callee's arguments inside the caller's, so arguments are not copied. Calls, returns and `new` on JavaScript functions stay inside the loop. Integer and double arithmetic run inline; slow paths live out of line. Property reads and writes, global accesses and constructor calls go through inline caches.

Builtins are Rust functions that read their arguments from the register file. When a builtin calls back into JavaScript (a sort comparator, a getter, a proxy trap), it runs a nested loop above the caller's frames. Generators and async functions suspend by copying their register window into a heap record and resume by copying it back.

`qjs --disassemble FILE` prints the register code of a script.

## The heap and the collector

`quickjs-oxide-core` owns every heap record. Values are 8-byte NaN-boxed words. Objects, strings, symbols, BigInts, shapes, closures and the engine's internal records each live in a typed arena, and property values, array elements and scope variables live in one shared value heap. Objects with the same properties in the same order share a shape, and the inline caches test shapes.

The collector is a non-moving mark-and-sweep collector. It runs only inside a call that allocates, or when the embedder asks for a collection with `Runtime::run_gc`. It runs when allocation would take memory past a budget: by default the live bytes times three, and at least 1 MiB. `RuntimeOptions` sets the minimum heap, the growth factor and an optional heap cap; past the cap an allocation fails with an out-of-memory error. `Runtime::memory_report` reports what every part of the runtime holds. [Optimizations](./optimizations.md) describes the arenas, shapes and the collector in detail.

## The host boundary

The core never calls the operating system. It asks its embedder for one trait, `Host`, re-exported as `quickjs_oxide::engine::api::core::Host`:

| Method | Used by |
| --- | --- |
| `write(stream, bytes)` | `print`, `console.log` |
| `now_ms()` | `Date.now`, `new Date()` |
| `local_tz_offset_minutes(utc_ms)` | Local-time `Date` methods |
| `random_seed()` | The initial `Math.random` state |
| `promise_rejection(event, promise)` | Unhandled and handled promise rejections |
| `read_file(path)` | `load` and `read` under `qjs --std` |

`SystemHost` in `adapters/native` serves native programs; `WebHost` in `adapters/web` serves the browser. An application can implement its own ([Host services](./host.md)).

Module loading crosses the same boundary. The application supplies a `ModuleLoader` that normalizes specifiers and loads modules ([Modules](./modules.md)). The application also decides when the job queue runs ([Promises and jobs](./jobs.md)).

## One realm per runtime

A runtime has one realm: one global object and one set of intrinsics. Code that needs isolation runs in separate runtimes. Strings keep UTF-16 code units, and source text keeps its exact bytes and positions through compilation, so error positions follow QuickJS's line and column rules.

## Where to go next

- [Optimizations](./optimizations.md): inline caches, NaN-boxing, shapes and the collector.
- [Performance](./performance.md): measured speed and memory against QuickJS 2026-06-04.
- [JavaScript support](./compatibility.md): conformance and the differences from QuickJS.
- [`core/README.md`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/README.md): the core's design record, module layout and rooting rule.
- [`docs/architecture.md`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/architecture.md): the workspace architecture for contributors.
