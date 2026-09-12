# Primitive VM migration ledger

C01 contract, 2026-09-12. This ledger accompanies the implementation and commit
plans. All migration rows start **pending**; a new type or a passing old-VM test
is not evidence of migration. The production entry remains PR19 until C29.

## Frozen starting point

- PR19 base branch: `perf/ordinary-property-kernel`.
- Starting HEAD: `c52d4dc7747641756dff8cb7159b9885e9cc8b17`.
- Historical measured engine: `1cc51bb5fcc5c36912d3197d877219ae513dc4b5`.
  The Rust source delta to starting HEAD is empty; documentation differs.
- Implementation branch: `vm/primitive-execution-core`.
- GitHub #20 is an existing issue, so the stacked PR receives the next available
  number. Its base is PR19, not the repository default branch.
- The pre-existing untracked plan/research files are inputs, not implementation
  evidence. Historical measurements remain labelled as historical.

## Compatibility contracts

| Surface | Contract to preserve | Current authority / acceptance |
| --- | --- | --- |
| Runtime/Context | Runtime identity, realm separation, synchronous eval/compile/execute/call/construct and byte-source handling | `src/engine/api`, `tests/rust_only.rs`, context/realm tests |
| Values | Public owning handles keep their runtime and identity alive; clone/drop and wrong-runtime rejection remain safe | value/object/rooted code tests; internal Copy words must not impersonate roots |
| Hosts | Explicit HostServices, native callback ABI, synchronous JS→host→JS with roots published and no live internal borrow | `src/engine/host`, native adapters, native function and reentry tests |
| Resource limits | Native host stack guard, interrupt/fuel, memory limits and exception identity; no promise of recovering arbitrary platform OOM | heap resource tests, `vm/native_stack.rs`, recursion tests |
| Debug | DebugInfoMode, authored source, fault location and stack trace identity | `code/debug.rs`, code runtime debug tests |
| Jobs | Promise job ordering, synchronous async prefix, explicit embedder job draining | `src/engine/jobs`, promise/async tests |
| Binary | Current trusted scalar/ordinary inputs and supported binary-object graph/QuickJS round trips, malformed-input rejection | `code/binary_object`, pinned fixtures and C oracles |
| Platforms | Rust 1.88 safe Rust, native CLI/host, web adapter, WASM playground; public modules stay restricted to api | workspace tests/clippy, source-layout and web checks |
| Conformance | Existing runnable support, unsupported diagnostics and exact frozen outcomes | full Test262, QuickJS oracle, no new skips or rewritten frozen receipts |

No public compatibility change is authorized by this ledger. Internal code
encoding can change; external supported formats require verified translation.

## Capability migration inventory

The source directories below define the complete inventory, including their
submodules. C19 reconciles this list against actual compiler instructions and
intrinsic registration before calling synchronous coverage complete.

| Capability | Current owner | Target owner / commit | Acceptance | State |
| --- | --- | --- | --- | --- |
| Numeric/control-flow source, updates | compiler; vm numeric/dispatch | compiler CFG/backend; value number; vm kernel C03–06/C13 | overflow, -0, NaN, branches/loops, TDZ, final-code dump | pending |
| Closures, scope, eval, arguments | compiler; vm host_bridge | bindings + environment/cell operations C12 | mapped arguments, defaults, per-iteration capture, eval aliasing | pending |
| Calls, this, realm and return | vm call/frames; object invocation | runtime driver + vm call C09/C27 | extra args, distinct windows, small-stack depth, last-root return | pending |
| Exceptions and completion | vm exception/unwind/completion | operations + vm unwind C10–11 | coercion order, catch/finally, cleanup exception priority | pending |
| Classes, constructors, private names | compiler; object; vm | invocation + binding operations C15 | derived this, super, brands, field order, new.target | pending |
| Properties, Proxy, Object/Reflect | object; builtins object/reflect/proxy | object storage + semantics properties C16/C19 | PR19 property corpus, reentry, receiver, Proxy invariants | pending |
| Array and synchronous iterators | builtins array/iterator; vm for_in | resumable builtin stages C17 | holes, species, sort/map callbacks, IteratorClose | pending |
| String/RegExp | builtins string/regexp; regexp | resumable builtin stages C18 | encoding, replacement callbacks, regexp oracle | pending |
| Buffers, typed arrays, atomics | builtins array_buffer/typed_array/data_view/atomics | target heap + builtin stages C18 | detach/resize during conversion, shared-buffer behavior | pending |
| Function, scalar wrappers, Math | builtins function/number/bigint/boolean/symbol/math | builtin stages C19 | family regressions and numeric differential oracle | pending |
| Map/Set, Date, JSON, Error, globals | builtins collections/date/json/error/global | builtin stages C19 | mutation during iteration, toJSON/replacer, dates/realm/source | pending |
| GC, weak objects, finalization | heap; builtins weak* | target heap/roots/collector + jobs C07/C20 | strong/ephemeron cycles, kept-alive, forced GC, backing accounting | pending |
| Generator | vm generator | independent ExecutionRecord C21 | next/throw/return, yield*, finally, reentry and GC | pending |
| Promise/async | builtins promise; vm async_function | operations + jobs C22 | assimilation, job order, single reply, pending roots | pending |
| Async generator/iteration | vm async_generator/async_from_sync_iterator | operation queue C23 | interleaved requests, asynchronous close, finally await | pending |
| Modules | modules; compiler/code module | modules + driver C24 | live imports, cycles, TLA, dynamic import and loader failures | pending |
| API/native/web entry | api; host; adapters; apps | rooted API + driver C25/C30 | JS→native→JS, platform tests, same-runtime exclusion | pending |
| External binary formats | code binary_object* | verified external translation C26 | authenticated fixtures, round trips, malformed input | pending |

## Ownership and acceptance rules

Code owns immutable instructions/maps. CodeInstance owns runtime/realm bindings.
ExecutionRecord owns frames, slots, operations and pending replies; each value
bearing state must enumerate roots. Heap handles identify, host handles root.
The kernel takes only verified code, slots and budget; semantic requests end
that borrow. Semantics requests JS invocation from the driver instead of
recursively driving it. Object owns storage transactions; properties owns the
observable algorithm. Number algorithms have one pure owner.

C29 requires new-entry conformance, default-budget original Earley-Boyer both
isolated and combined, five alternating fixed 50+8 rounds and resource evidence.
C30 switches all default entries only after this gate. C31 removes old execution
and ownership bridges. C32 records final-code evidence separately for six
architecture capabilities and issue #16 items 1/5/7/9/10. Item 10 may have a
negative cost finding, but its position/observation contract remains required.
No row may be marked migrated using a fallback to the old VM.

## Commit progress

- C01: contract and migration inventory recorded; no engine claim.
- C02–C32: pending. Each implementation commit updates its actual evidence and
  outstanding scope here. The PR remains a draft while these gates are unmet.
