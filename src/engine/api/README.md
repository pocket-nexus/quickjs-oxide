# Rust embedding API

api is the boundary through which Rust callers use quickjs-oxide. `api::core`
is the API over `oxide-core`; `api::error` holds
the front end's diagnostics (`Error`, `ErrorKind`, `NativeErrorKind`); the API
root adds `declare_thread_stack`, `number_to_string` and
`quickjs_detect_module_bytes`. The front end, the lowering and the core's
internals stay private: `check-source-layout.py` checks that `api::core`
re-exports only its own items, the API root's, the front end's errors,
strings, compile options and import attributes, and `oxide-core`'s host,
options and report types.

## Shape

| Item | Content |
| --- | --- |
| `Runtime` | an `Rc` of the shared state (`oxide_core::Runtime` in a `RefCell`, the module loader, the preloaded modules); cloning shares it. `new()`, `with_options(RuntimeOptions)`, `with_host(impl Host, RuntimeOptions)`; jobs (`run_pending_job`, `run_jobs`, `with_execution_turn`), rejection tracking, collection (`run_gc`, `gc_stats`, `GcPolicy`, `set_gc_stress`), the reports (`memory_report`, `gc_report`, `ic_report`, `disassemble`), property keys, symbols, prototypes and own-property operations, `type_of` and the printer |
| `Context` | the handle of the runtime's one realm: `eval*`, `compile*`, `execute`, `compile_module*`, `execute_module`, `call`, `construct`, objects, arrays and properties, the intrinsics, the pending exception (`has_exception`, `take_exception`), `new_native_error`, the `qjs` helpers |
| `Value` | `Undefined`, `Null`, `Bool`, `Int`, `Float`, `BigInt(JsBigInt)` and `String(JsString)` as Rust data copied out of the core; `Symbol(SymbolRef)` and `Object(ObjectRef)` as handles |
| `ObjectRef`, `SymbolRef`, `CallableRef` | a handle in the core's handle table, released on the last drop |
| `PropertyKey`, `OrdinaryPropertyDescriptor`, `CompleteOrdinaryPropertyDescriptor`, `DescriptorField`, `AccessorValue`, `WellKnownSymbol` | keys and descriptors |
| `RuntimeError` | `Exception` (the value is the context's pending exception), `Reentered`, `Unsupported`, `Compile(Error)`, `OutOfMemory`, `Invariant`, `Internal` |
| `Host`, `Stream`, `RejectionEvent`, `SystemHost` | the core's host trait and its default, re-exported |
| `ModuleLoader`, `ModuleSource`, `CompiledModule`, `Scope` | module loading |
| `RuntimeOptions`, `HeapLimits`, `MemoryReport`, `GcStats` | the core's configuration and reports, re-exported |
| `api::core::test262` | the Test262 runner's worker, behind the `test262-host` feature |

Every `Runtime` and `Context` method borrows the cell for one engine entry.
A host callback that calls the API without its `Scope` finds the cell
borrowed and gets `RuntimeError::Reentered`. A value dropped
while the cell is borrowed releases its handle at the next entry, so `Drop`
always succeeds.

## Realm

A runtime has one realm. `Runtime::context`, `Runtime::new_context` and
`Context::new` all return its `Context` (deviation `TARGET-REALMS-001`).

## Host

`Host` is `oxide_core::host::Host`: `write`, `now_ms`,
`local_tz_offset_minutes`, `random_seed`, `promise_rejection` and
`read_file`, each with a default over the process. `Runtime::new` uses the
core's `SystemHost`; the adapters supply `quickjs-oxide-host`'s `SystemHost`
(the host's time-zone rules) and `quickjs-oxide-web-host`'s `WebHost`. The
application owns input, module-loading policy, output and when the job
queue drains.

## Modules and `Scope`

`Runtime::set_module_loader` installs a `ModuleLoader`. Its `normalize`
(defaulting to QuickJS's relative resolution) and `load` callbacks receive a
`&mut Scope<'_>`, a reborrow of the runtime for the callback's duration: it
creates objects and native errors and compiles a module source
(`Scope::compile_module`). A callback throws by returning `Err(Value)`. A
loader returns a `ModuleSource`: source text with the properties its
`import.meta` starts with, JSON, JSON5, or a module compiled through the
scope. Both callbacks receive the import attributes; checking them is the
loader's (`load` rejects what it does not accept). Values made through a
`Scope` are handles like any other and outlive the callback.

## Configuration and reports

`RuntimeOptions` sets the heap cap (`heap`), the collection floor and growth
factor, the register-file limit, the native-stack budget and an optional
re-entry depth limit; `Default` is the behavior without options.
`GcPolicy::Manual` disables the growth trigger. `memory_report` returns the
core's `MemoryReport` (per arena, the value heap, the code store, the
register file, the caches and the collector), read without a collection.
`qjs --gc-stats`, `--memory-report`, `--ic-stats` and `--disassemble` print
these reports.

See [workspace architecture](../../../docs/architecture.md) for how the API
sits on the front end, the lowering and the core.
