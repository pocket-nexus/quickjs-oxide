# QuickJS semantic feature parity contract

This document is the definition of done for quickjs-oxide. It constrains implementation direction, test evidence and public claims; stage plans, current test counts or whether a demo runs do not override it.

## 1. Goal and fixed baseline

The goal is an independent Rust rewrite of QuickJS that reaches semantic and product-surface feature parity with **QuickJS 2026-06-04**. "Parity" does not mean "can execute some JavaScript"; it means that the same input has the same observable behavior as that release in language semantics, host capabilities, embedding API, command line, modules, serialization and lifecycle.

**WeakRef and call-argument lifetimes follow ECMAScript (maintainer decision).** WeakRef keep-alive, weak target clearing and FinalizationRegistry cleanup must conform to the specification; argument liveness in ordinary calls is determined by the observable bindings, `arguments`, closures and execution state. The pinned QuickJS argument-copying paths and early weak-reference invalidation serve only as implementation references and are not acceptance expectations here. The exact scope is in [deviations.md](deviations.md#ecmascript-weakref-lifetime-001).

The baseline is the official `quickjs-2026-06-04` release source, not the rolling repository `master`, quickjs-ng, other forks, or whatever `qjs` happens to be installed on the system. The key baseline anchors are:

- `VERSION` is `2026-06-04`;
- the bytecode version is `BC_VERSION = 5` in `quickjs.c`;
- the Unicode data version is `17.0.0` in `libunicode.h`;
- Test262 is pinned to `TEST262_COMMIT` in the release `Makefile`, with the release `tests/test262.patch` applied; the pinned values in the repository are in [`compat/upstream.toml`](../compat/upstream.toml);
- the enabled/skipped Test262 scope is defined by the release `test262.conf` and `test262o.conf`, and known results by `test262_errors.txt` and `test262o_errors.txt`. The release records 58 and 0 known-error lines respectively; these line counts are only an integrity check and do not replace per-test result comparison.

Before CI accepts any parity result, the repository must record the provenance, SHA-256 and unpacked file manifest of the official source archive. Test reports must also include the baseline SHA-256, target triple, build options and Test262 commit, so that same-named versions cannot drift.

This contract reproduces the scope that release actually provides; it does not treat "ES2025" as a moving target. The upstream documentation states that proper tail calls and `Atomics.waitAsync` are unsupported and that ECMA-402 Intl is not implemented; other features marked `skip` in `test262.conf` are likewise outside the required scope of this baseline. Adding these capabilities is separate work and cannot offset missing or incompatible behavior inside the baseline.

## 2. Pure Rust boundary and oracle rules

Every product implementation delivered to users must be Rust: the interpreter, compiler, GC, RegExp, Unicode, BigInt, standard library, Worker, CLI, static/dynamic libraries and the C ABI adapter layer are all compiled from Rust. The workspace keeps the safe-Rust constraint `unsafe_code = "forbid"`; host and ABI capabilities are designed within it, and the parity goal does not imply permission to relax it.

The following are forbidden in product builds and runtime paths:

- compiling, linking or vendoring `quickjs.c`, `libquickjs.a` or another QuickJS fork, or using it as the actual engine through FFI;
- launching `qjs`, Node.js, JavaScriptCore, V8 or another external interpreter at runtime to parse or execute;
- hiding an external engine in a build script, dynamic library, optional feature, WASM blob or generated code;
- wrapping an existing QuickJS binding behind a Rust API.

The official C `qjs` is allowed only on test oracle paths: building the baseline, generating goldens, running differentials and confirming known upstream behavior. Test C embedder/native-module fixtures, and the C source files that `qjsc -c/-e` generates for users under the compatibility contract, are not part of the product engine and may use C; they must link against the Rust-built library and must not bring the C engine back in.

The implementation follows upstream algorithms, state machines and invariants, not just syntax examples. Upstream logic may be re-expressed with suitable Rust data structures; line-by-line translation is not required, nor are identical internal addresses, object layouts or optimizations, unless they become observable through the C ABI, memory statistics, finalization timing or bytecode format. Each core module records its corresponding upstream files/symbols in module documentation or a source map for item-by-item audit.

## 3. Runnable slices use the product path

An increment may cover only narrow syntax, but it must connect through the real product compilation, execution and lifecycle. The internal representation may evolve; there is no predetermined "final architecture". The path to check is:

1. establish arguments, script/module detection and error exits following `qjs.c::eval_file` / `eval_buf`;
2. establish isolated heap/realm, atoms, intrinsics and exception slot following `JS_NewRuntime` and `JS_NewContext`;
3. perform lexing, scope analysis and compilation against the semantics of `next_token`, `js_parse_*` and `JS_EvalInternal`; the compilation result is published as a single `ExecCode`, without copying the `quickjs-opcode.h` encoding;
4. keep the call, stack/resource-limit and exception behavior of `JS_CallInternal`, propagating completion/exception through the product executor;
5. drain jobs through `JS_ExecutePendingJob`, then release resources along `JS_FreeContext` / `JS_FreeRuntime`.

The skeleton uses the real `Value`, atom, string, object/shape, property descriptor, environment/closure, bytecode function and exception representations from the start; unimplemented syntax reports an explicit error. A temporary AST evaluator, lowering all values to JSON, skipping property descriptors, representing JS throws as Rust panics, or calling the oracle for answers are not acceptable interpreter skeletons.

## 4. Audit mapping from upstream components to the Rust implementation

The table does not require copying the C file layout; it requires the Rust implementation to cover the same responsibilities and key invariants.

| Area | Upstream anchors | Rust implementation must cover |
| --- | --- | --- |
| runtime, realm, values and objects | `JSRuntime`, `JSContext`, `JSObject`, `JSShape` in `quickjs.c`; `JSValue`/tags and property flags in `quickjs.h` | runtime/context isolation, value conversion, atoms/symbols, prototypes, shape/property descriptor, exotic objects, exception slot and ownership |
| lexer, compiler, VM | `next_token`, `js_parse_*`, `JS_EvalInternal`, `JSFunctionBytecode`, `quickjs-opcode.h`, `JS_CallInternal` | script/module grammar, scopes/closures, stack effects, call/construct, completion, debug/line info, equivalence after optimization |
| intrinsics | `JS_NewContext`, `JS_NewContextRaw`, `JS_AddIntrinsic*` | base objects and selectable installation semantics for Date, eval, normalize, RegExp, JSON, Proxy, Map/Set, TypedArray, Promise, WeakRef |
| numbers and BigInt | `js_dtoa` in `dtoa.c`; `js_atof`, `js_bigint_*`, `JS_TAG_SHORT_BIG_INT` in `quickjs.c` | IEEE-754 conversion/formatting, `-0`/NaN/Infinity, arbitrary-precision two's-complement arithmetic and small-BigInt fast-path semantics |
| RegExp | `lre_compile` / `lre_exec` in `libregexp.c`, `libregexp-opcode.h` | ECMAScript RegExp parser, bytecode, explicit backtracking stack, captures, Unicode modes and API behavior |
| Unicode | `libunicode.c` / `libunicode-table.h`, `unicode_normalize`, `unicode_script`, `unicode_prop` | Unicode 17.0.0 identifier, case, normalization, script/category/binary/string properties |
| modules | `js_resolve_module`, `JS_ResolveModule`, `JS_LoadModule`, module namespace exotic methods | parse/link/evaluate, live bindings, cycles, TLA, dynamic import, `import.meta`, attributes and loader callbacks |
| GC/lifecycle | `JS_DupValue` / `JS_FreeValue`, `free_zero_refcount`, `gc_decref`, `gc_scan`, `gc_free_cycles`, `JS_RunGC` | deterministic reference counting, cycle collection, weak objects, finalizer/mark, memory limits and public ownership rules |
| object serialization | `JS_WriteObject(2)`, `JS_ReadObject`, `JS_EvalFunction` | BJSON object graphs, boxed primitives, TypedArray/ArrayBuffer, shared references/cyclic graphs; **excludes reading/writing upstream `BC_VERSION` bytecode** (see §11) |
| host library/event loop | `quickjs-libc.c`, `js_std_loop`, `js_std_await`, `js_module_loader` | `std`/`os`, global helpers, files/processes/signals/timers/handlers, promise rejection and event loop |
| Worker | `JSWorkerMessagePipe`, `worker_func`, `js_worker_postMessage` | per-thread runtime/context, FIFO messages, structured clone, SAB sharing, lifecycle and upstream restrictions |
| CLI/compiler | `qjs.c`, `repl.js`, `qjsc.c` | full `qjs`/REPL behavior, and `qjsc` C source/executable output, feature stripping and module collection |

## 5. Global completion gates

The project may claim "QuickJS 2026-06-04 feature parity" only when all of the following hold:

1. every gate in this document has a reproducible report from the same commit, with no unclassified failure, skip, timeout, crash or differential;
2. a product dependency audit shows that runtime and release artifacts contain no external JS engine; with oracle tools and network removed, the product still builds and runs;
3. oracle and Rust engine tests run in separate processes, with the same fixtures, in a fixed environment; tests must not change semantics depending on the engine under test;
4. every permitted difference is recorded in the versioned [deviation ledger](deviations.md) with a minimal reproduction, upstream output, Rust output, rationale, compatibility impact and approver. An unrecorded "more reasonable behavior" is still a failure;
5. test reports keep per-case results and commands, not only the pass rate; any filtering, retry, golden update or timeout increase must be visible in the diff;
6. every entry in the supported platform matrix passes the same semantic gates. Unverified platforms may be marked unsupported, but parity cannot be claimed for them;
7. release builds do not panic, abort, deadlock or access out of bounds under stress cases such as OOM, stack limit, interrupt, malicious deep object graphs, complex RegExps and cyclic modules. A timeout is itself a failure unless the oracle produces the same kind of result under the same limits and it is recorded.

Performance and size are not primary equivalence criteria in this document, and internal addresses or per-instruction optimizations need not be reproduced; however, being so slow that upstream functional cases, Test262 or the worker/event-loop gates cannot finish within the agreed budget still counts as the feature being unusable.

## 6. Language and built-in object gates

The language core must cover all syntax and semantics enabled by the baseline `test262.conf`, including strict/sloppy, Annex B, lexical/var environments, closures, direct/indirect eval, classes/private elements, `this`/`super`/`new.target`, destructuring, generators, async/await, iterators, exceptions, Proxy, Symbols, property order, Promises/jobs, WeakRef/FinalizationRegistry, ArrayBuffer/TypedArray/SharedArrayBuffer/Atomics and so on.

The gates are:

- every unmodified JS file in the official `Makefile::test` passes under Rust `qjs`: `test_closure.js`, `test_language.js`, `test_builtin.js`, `test_loop.js`, `test_bigint.js`, `test_cyclic_import.js`, `test_worker.js`, `test_std.js`, `test_rw_handler.js`;
- `test_bjson.js` and `examples/test_point.js` under the shared-library configuration also pass and serve as evidence for the native module/C API path; they must not simply be removed from the test list;
- every opcode has stack-effect, exception-edge and control-flow tests; parser/bytecode/VM coverage reports trace back to the opcode list in `quickjs-opcode.h`, and unimplemented opcodes must not be treated as unreachable;
- independent regression suites exist for coercion, property descriptors, prototype/exotic methods, enumeration order, cross-realm, exception completion and job ordering, because "the program prints the same final output" does not prove the intermediate semantics.

Rust may use different internal optimizations, but hard capabilities such as Proxy, eval, `with`, Annex B or WeakRef must not be downgraded to stubs or permanent feature flags. The `qjsc -fno-*` stripping modes are an upstream product capability and do not mean the default engine may lack these capabilities.

## 7. Test262 gates

A repository script prepares Test262 from the pinned commit in one step, applies the upstream `tests/test262.patch`, and runs the modern suite and the ES5.1 suite (`test262o`) separately. The gate is at least equivalent to the upstream `make test2-check`, `make test2` and `make test2o` paths.

Results are judged by a per-test outcome vector, not an overall pass rate:

- Rust must not have any fail, skip, timeout, crash or harness error beyond the official oracle;
- Rust may pass a known baseline failure, but each such case is recorded in the deviation ledger as a "fixed upstream difference", to confirm the harness actually executed it;
- for tests that pass under both the oracle and Rust, the strict/noStrict, module, async and other run variants must match;
- feature skips in `test262.conf` describe the baseline scope; adding skips, excluding by directory, ignoring stderr or counting crashes as expected failures is not allowed;
- CI stores the configuration file hashes, Test262 commit, patch hash, per-case results and summary. Showing only "99.x%" is not evidence.

Test262 demonstrates specification coverage but does not by itself demonstrate QuickJS parity; CLI, C API, BJSON/bytecode, `std`/`os`, Worker and QuickJS-specific diagnostics are covered by the gates that follow.

## 8. Differential gates

The differential runner passes identical input to the official `qjs` and to Rust `qjs`, and compares:

- parse/early-error acceptance or rejection, error kind and phase;
- normal values, property descriptors, own-key order, prototypes and observable side-effect traces;
- stdout, stderr, exit status, uncaught exception type/message/stack, and Promise rejections;
- module link/evaluate order, dynamic import/TLA, and job and timer/message order;
- cross-engine compile/serialize/read/evaluate results.

Nondeterministic fields such as paths, addresses, wall clock, random numbers and thread scheduling are handled only by versioned, minimal normalization rules; stderr or stacks must not be discarded wholesale. The test environment pins locale, timezone, cwd, environment variables and resource limits; cases that need time/randomness use explicit fixtures, and cases that cannot be stabilized are classified separately.

The corpus includes at least: all upstream tests, a representative reduced Test262 set, a probe for every public API, targeted cases for every opcode/builtin, regression cases, and grammar/property/bytecode/object-graph fuzzing with reproducible seeds. Differences found by fuzzers are reduced automatically and added to the regression set. Running a fixed set of happy-path examples does not constitute a differential gate.

## 9. CLI and REPL gates

Rust `qjs` must be compatible with the actual argument parsing in `qjs.c`, not only the options most commonly documented. Coverage includes `-e/--eval`, `-i/--interactive`, `-m/--module`, `--script`, `-I/--include`, `--std`, `--strict`, `-d/--dump`, `-T/--trace`, `-q/--quit`, `--memory-limit`, `--stack-size`, `-s`, `--strip-source`, `--no-unhandled-rejection`, `--` and script arguments.

The gate compares item by item:

- option grouping, missing/unknown arguments, help/version text, stdout/stderr and exit code;
- `.mjs` and `JS_DetectModule` autodetection, explicit script/module, include order, `scriptArgs`;
- injection of `print`, `console.log`, `performance.now`, `__loadScript`, `std`/`os`;
- REPL multiline, completion/printing, exception recovery, top-level await and EOF/TTY behavior, against `repl.js`;
- memory/stack/interrupt/unhandled-rejection behavior and the stably comparable fields of `-d/-T`.

Rust `qjsc` must cover `-c`, `-e`, `-o`, `-N`, `-m`, `-D`, `-M`, `-x`, `-p`, `-S`, `-s`, `--keep-source`, `-flto` and all `-fno-*` modes of `qjsc.c`. Generated C need not be textually identical, but it must follow the same symbol/initialization contract and link against the Rust runtime; the default generated executable and module/native-module/worker collection behavior must be equivalent to the baseline.

## 10. Embedding API and C ABI gates

The Rust-first API may be more idiomatic Rust, but it does not replace the QuickJS embedding surface. Rust `staticlib`/`cdylib` outputs provide a source/ABI-compatible facade for `quickjs.h` and `quickjs-libc.h`, with a public symbol inventory extracted automatically from the headers. Every function, type, constant, flag, callback and inline/macro behavior is marked implemented, platform-specific or baseline-unsupported; "not called by the current demo" is not a reason to skip.

The gates include:

- `JSRuntime`/`JSContext` lifecycle and the object-sharing boundary between multiple contexts in one runtime;
- `JSValue`, tags, constants and calling convention, `JS_DupValue`/`JS_FreeValue` ownership, and the exception sentinel/`JS_GetException`;
- atoms/strings/UTF-8-CESU-8 conversion, numeric/BigInt coercion, objects/arrays/property descriptors/prototypes/enumeration;
- `JS_Call*`, `JS_Eval*`, JSON, ArrayBuffer/TypedArray/SAB, Promise/job, interrupt, memory allocator/limit, class/exotic/finalizer/gc_mark;
- module loader/C modules, `JS_WriteObject*`/`JS_ReadObject`, print/memory usage, and the `quickjs-libc.h` helpers/event loop;
- upstream `examples/fib.c`, `examples/point.c`, `tests/bjson.c` and additional ownership/error fixtures compile, link and pass against the Rust header/library without changes to their logic;
- C ABI sanitizer/valgrind-style tests and Rust Miri/concurrency models cover cross-boundary use-after-free, double-free, panic unwind, callback re-entry and allocator failure.

If a platform's C ABI is not yet verified, only the Rust API may be claimed as usable on that platform, not full QuickJS API parity.

## 11. Internal execution representation

The compiler lowers source to stack instructions, then verifies and publishes them as the engine-internal `ExecCode` word stream. The runtime executes within the published bounds, operands and control-flow targets; the external interface is source compilation and `engine::api`. Responsibilities are described in the [architecture notes](architecture.md).

## 12. `std`, `os` and event-loop gates

`quickjs-libc.c` is part of the baseline. The `std` module's environment variables, extended JSON, files/stdio, printf, popen, load/eval, error constants and so on, and the `os` module's fd I/O, TTY, filesystem, signals, process, pipe, time/timer, async handlers and platform constants, are all inventoried item by item from the export lists in `js_std_funcs`, `js_std_file_proto_funcs` and `js_os_funcs`.

The gates include:

- export names, arity, property flags, constants, error codes and platform conditionals match the oracle;
- `js_module_loader` behavior for relative paths, system modules, the JSON import attribute and native `.so` modules matches;
- `js_std_loop` drives Promise jobs, fd handlers, timers and worker ports together, preserving queue ordering and exit conditions;
- `js_std_await`, unhandled rejection tracking, exception printing and async error exits match;
- upstream `tests/test_std.js`, `test_rw_handler.js` and isolated process/signal/file/timer integration tests pass on every supported platform; CI capabilities that need permissions have an explicit platform runner rather than being skipped while parity is still claimed.

Implementing only `print` and file reading, or permanently defining `std`/`os` as "provided by the host", does not meet QuickJS CLI feature parity.

## 13. GC, WeakRef and resource gates

Lifecycle comparisons in this section are subject to the ECMAScript-first rule in §1. WeakRef construction and successful dereference must implement the effect of `AddToKeptObjects`; the host performs `ClearKeptObjects` at an appropriate boundary after a synchronous execution sequence completes. Nested call returns, parameter overwrites or explicit GC must not release this guarantee early, and FinalizationRegistry cleanup callbacks must not interrupt synchronous ECMAScript execution.

Call frames reduce copying by transferring existing ownership or borrowing values that are already reliably kept alive. Object arguments do not get an extra copy held until function exit merely because of their type; separate `arguments` values, mapped parameter bindings, direct eval, closures, rest parameters and suspended state still retain the values each actually needs. Tests must not treat "disappearance is observable through WeakRef immediately after the last reference drops" or "all original object arguments always live until exit" as general language requirements.

To preserve C API ownership and observable finalization, the default lifecycle model takes QuickJS reference counting plus cycle removal as its design reference, rather than relying on Rust ownership to free things "by coincidence". Rust may change internal containers, but must preserve:

- the deterministic reference-counting effect of `Dup/Free`, freeing of zero-reference objects and the recursive free queue;
- the cycle detection expressed by `gc_decref -> gc_scan -> gc_free_cycles` and the behavior of `JS_RunGC`;
- the relationships among class finalizers, `gc_mark`, opaque resources, weak refs/finalization jobs and SAB backing stores;
- upstream constraints such as no JS execution inside finalizers, context/runtime teardown order, and allocator failure/limit/threshold;
- cycles through shared shapes, closures/var refs, async frames, modules, Promises and worker messages neither leak nor are freed early.

The gates use deterministic drop probes, C/Rust classes with finalizers, random cyclic object graphs, WeakRef/FinalizationRegistry, repeated runtime/context creation and destruction, OOM fault injection and long-running leak tests. Tests check both "eventually freed" and "free timing/count"; relying on the OS to reclaim memory after process exit does not demonstrate GC parity.

## 14. Unicode gates

Unicode behavior is pinned at 17.0.0. The gates cover:

- UTF-8 input, UTF-16 code units, lone surrogates, C API UTF-8/CESU-8 conversion and string indexing;
- identifier start/continue, case conversion/folding, locale-independent casing;
- NFC/NFD/NFKC/NFKD normalization;
- script/script extensions, general category, binary properties, Unicode sequence properties;
- RegExp `u`/`v` modes, property escapes, word boundary/canonicalization and astral code points.

Beyond Test262, table-driven tests iterate over every code point/range of the Unicode 17 data and compare against the `libunicode.c` oracle; generated tables record their data source, version and checksum. Using the host OS/ICU "latest Unicode" without pinning the version is not parity, even if most tests pass.

## 15. RegExp gates

RegExp must implement the full parser/compiler/executor corresponding to `libregexp.c`, rather than forwarding to the Rust `regex` crate, which generally lacks ECMAScript backreference, lookaround, `lastIndex` and backtracking semantics.

The gates cover all flags/grammar accepted by the baseline, named/numbered captures, backreferences, lookahead/lookbehind, greedy/lazy quantifiers, sticky/global `lastIndex`, indices, Unicode properties/sets, replace/split/search/match/matchAll and empty-match advancement. Generative tests compare compile errors, capture spans, result array properties and state changes together.

Backtracking uses an explicit stack bounded by runtime stack/memory/interrupt controls, following `lre_exec` and `libregexp-opcode.h`; complex or catastrophic input must not cause a Rust stack overflow, panic or uninterruptible execution. Passing only simple literals, or relying on Test262 not covering some backtracking order, does not count as RegExp parity.

## 16. Number and BigInt gates

The Number gate compares against `dtoa.c`/`js_atof` and covers binary IEEE-754 boundaries, shortest decimal representation, radix, rounding, subnormals, `-0`, NaN/Infinity, numeric literals and String/JSON conversion. Rust standard formatting output must not be assumed equivalent without proof.

The BigInt gate compares against the arbitrary-precision two's-complement semantics of `js_bigint_*` and short-BigInt normalization, covering:

- parsing/formatting in every radix, positive/negative boundaries and very large input/OOM;
- `+ - * / % ** << >> & | ^ ~`, division by zero, negative exponents and shift boundaries;
- coercion with Number/String/Boolean, abstract/strict equality and relational comparison;
- `BigInt.asIntN/asUintN`, BigInt typed arrays, DataView, Atomics;
- the C API `JS_NewBigInt64`/`JS_NewBigUint64`/`JS_ToBigInt64` and BJSON round-trips.

Beyond upstream `tests/` and Test262, an independent arbitrary-precision model generates reproducible random vectors, and the oracle, Rust and the model are compared three ways. A fixed 64/128-bit integer implementation cannot satisfy this gate.

## 17. Module gates

The module parser/linker/evaluator must cover static import/export, live bindings, the namespace exotic object, re-export/star ambiguity, cycles, dynamic `import()`, top-level await, `import.meta`, JSON modules/import attributes and host normalize/load/check-attributes callbacks.

The gates compare error types and ordering across the parse, resolve, instantiate and evaluate phases, and cover:

- module identity/cache, relative/system/native module resolution;
- sync/async module graphs, diamonds/cycles, TLA fulfillment/rejection and job ordering;
- namespace own keys/descriptors/immutability and live binding updates;
- `JS_EVAL_FLAG_COMPILE_ONLY`, `JS_ResolveModule`, `JS_EvalFunction`, `JS_GetImportMeta/ModuleName/ModuleNamespace`;
- CLI autodetection, `qjsc -D/-M`, bytecode modules, worker relative paths.

Concatenating each file as an independent script, or supporting only acyclic static imports, is not module parity.

## 18. Worker gates

`os.Worker` keeps the upstream model: a worker uses its own thread, its own `JSRuntime`/`JSContext` and a module entry; parent and child communicate through a FIFO pipe/port. `postMessage` structured-clones ordinary object graphs with `JS_WRITE_OBJ_SAB | JS_WRITE_OBJ_REFERENCE` and shares SharedArrayBuffer backing stores, rather than sharing ordinary JS objects.

The gates cover:

- the caller base path, module loader and startup errors for `new os.Worker(relative_module)`;
- `Worker.parent`, `postMessage`, `onmessage` set/get/null, the event `{ data }` shape and message ordering;
- cross-thread behavior of primitives, BigInt, ArrayBuffer/TypedArray, cyclic/shared reference graphs, SAB+Atomics;
- worker exceptions/unhandled rejections, early parent/child exit, queue draining, resource release and high message concurrency;
- the upstream prohibition on creating a worker inside a worker, and its error;
- upstream `tests/test_worker.js`/`test_worker_module.js` pass unmodified, and stress/GC/teardown/TSAN-style tests demonstrate no lost messages, deadlocks or use-after-free.

Simulating workers with same-thread tasks that change isolation/blocking semantics, or supporting only JSON messages, is not Worker parity.

## 19. Implementation order

This order reduces rework; completing a stage does not mean parity is reached:

1. **Lock down evidence**: introduce official baseline metadata, the oracle builder, the differential protocol, the per-test result format and the source-map inventory.
2. **Real core skeleton**: implement runtime/context, Value, atoms/strings, object/shape/property, exceptions, and the minimal vertical path lexer -> bytecode compiler -> stack VM; route Rust `qjs -e` through the final pipeline.
3. **Language foundation**: scopes/closures, call/construct, control flow, functions/classes, eval, exceptions, progressively covering `test_language`, `test_closure`, `test_loop`, with opcode tests added alongside.
4. **Object model and intrinsics**: complete coercion, descriptor/prototype/exotic, Array, Symbol, Date, JSON, Map/Set, Proxy, TypedArray and so on, then extend to generators, Promises/jobs, async and WeakRef.
5. **Precise subsystems**: implement dtoa/number, BigInt, Unicode 17 and RegExp following upstream logic, each with its own generative differential gate.
6. **Modules and serialization**: implement the full module graph/TLA/import attributes, then BJSON object serialization and `qjsc` packaging (excluding reading/writing upstream BC5 bytecode, see §11).
7. **Lifecycle and embedding**: close out refcount/cycle GC, limits/interrupts, the full Rust API and the Rust-exported QuickJS C ABI, and run the upstream C examples/native modules.
8. **Host product surface**: implement `std`, `os`, the event loop, the native loader and Worker, then complete all `qjs`/REPL/`qjsc` options and output modes.
9. **Close out, not rename**: run the full upstream tests, Test262/ES5.1, differential/fuzz and the platform/ABI/stress matrix, and publish a parity claim only after every deviation is cleared or approved.

Each stage reports only concrete coverage, for example "`test_closure.js` passes" or "37/NN opcodes implemented"; "runnable", "MVP", "core syntax complete" or "Test262 at some pass rate" must not be restated as feature parity.

## 20. What does not count as done

Parity cannot be claimed if any of the following holds:

- only hello world, arithmetic, JSON or a particular benchmark runs;
- the parser covers most syntax, but the VM, exceptions, descriptors, module phases or job ordering are approximations;
- the Test262 pass rate is high, but the commit/patch/config is not pinned, skips were added, or there are no per-test results;
- upstream JS tests pass, but native modules, the C API, `std`/`os`, Worker, BJSON, REPL or `qjsc` are skipped;
- only an idiomatic Rust API is provided, without the `quickjs.h`/`quickjs-libc.h` compatible surface;
- the internal execution word stream is presented as an upstream bytecode parity claim; the internal representation is described in §11;
- RegExp, Unicode, BigInt, GC or Worker is delegated to a stub, a fixed-width approximation, a drifting version of a host library or an external process;
- the product package still contains the QuickJS C implementation, or requires the oracle at runtime;
- upstream test meaning is changed, only part of stdout is compared, exceptions are swallowed, retries are unbounded or the exclusion set is widened to make CI green;
- a known difference has no minimal reproduction and deviation record;
- an unconditional parity claim is made for a platform, feature flag or release profile on which the gates were not run;
- the staged architecture is a throwaway toy interpreter that is still planned to be "replaced with the real QuickJS model" later.

The final verdict depends only on whether reproducible evidence covers every gate in this contract, not on implementation effort, lines of code, plan completion percentage or subjective similarity.
