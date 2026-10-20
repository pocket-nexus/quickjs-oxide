# API and commands

This page lists the embedding API's entry points, its options and the `qjs` command line. Every Rust item is in `quickjs_oxide::engine::api::core`. Methods that enter the engine return `Result<_, RuntimeError>`; the tables leave that out.

## Runtime

A `Runtime` is one heap, one realm and one job queue. Cloning a `Runtime` shares it.

### Constructors

| Constructor | Host | Options |
| --- | --- | --- |
| `Runtime::new()` | the core's `SystemHost` (process output, system clock, UTC) | defaults |
| `Runtime::with_options(options)` | the core's `SystemHost` | `options` |
| `Runtime::with_host(host, options)` | `host` | `options` |
| `Runtime::new_with_host_services(host)` | `host` | defaults |

`quickjs_oxide_host::SystemHost::default()` is the host for native applications: it adds the host's time-zone rules. [Host services](./host.md) describes the `Host` trait.

### Realm

| Method | Does |
| --- | --- |
| `context()` | the realm's `Context` |
| `new_context()` | the same `Context`, as a `Result` |
| `is_same_runtime(&other)` | whether two handles share one runtime |

A runtime has one realm: every call returns a handle to the same one.

### Jobs and promises

| Method | Does |
| --- | --- |
| `is_job_pending()` | whether a job is queued |
| `run_pending_job()` | runs the oldest job; returns `PendingJobOutcome::Executed` or `NoJob` |
| `execute_pending_job()` | the same as `run_pending_job` |
| `run_jobs()` | runs jobs until the queue is empty or one throws |
| `promise_snapshot(&promise)` | the `PromiseState` (`Pending`, `Fulfilled`, `Rejected`) and result, without running JavaScript |
| `with_execution_turn(body)` | runs `body` as one synchronous sequence for `WeakRef` lifetimes |
| `set_track_rejections(bool)` | keeps rejected promises that have no handler |
| `take_unhandled_rejections()` | the tracked rejections as `(promise, reason)` pairs |

See [Promises and jobs](./jobs.md).

### Modules and compilation

| Method | Does |
| --- | --- |
| `set_module_loader(loader)` | installs the `ModuleLoader` that `import` and `import()` use |
| `set_debug_info_mode(mode)` | what newly compiled functions keep: `DebugInfoMode::Full` (default), `StripSource`, `StripDebug` |
| `debug_info_mode()` | the current mode |
| `set_can_block(bool)` | whether `Atomics.wait` may block |
| `disassemble(source, filename)` | the register code of a script, as text |

See [Modules](./modules.md).

### Memory and the collector

| Method | Does |
| --- | --- |
| `run_gc()` | runs a full collection; returns `GcStats` |
| `gc_stats()` | the collector's statistics |
| `set_gc_policy(policy)` | `GcPolicy::Automatic` (default) collects on heap growth; `GcPolicy::Manual` only at the cap and on `run_gc` |
| `gc_policy()` | the current policy |
| `memory_report()` | a `MemoryReport` of what every part of the runtime holds, without a collection |
| `set_record_gc_pauses(bool)` | records each pause for `gc_report` |
| `gc_report()` | the collector summary `qjs --gc-stats` prints |
| `ic_report(worst)` | the inline-cache report; `Some` only in an `ic-stats` build |
| `set_gc_stress(interval)` | collects before every `interval`-th allocation; only in a `gc-stress` build |

`Runtime::GC_STRESS_AVAILABLE` and `Runtime::IC_STATS_AVAILABLE` tell whether the build has those features. See [Errors and limits](./limits.md).

### Keys, symbols and objects

| Method | Does |
| --- | --- |
| `intern_property_key(&str)` | a `PropertyKey` from text |
| `intern_property_key_js_string(&JsString)` | a `PropertyKey` from a `JsString` |
| `property_key_to_js_string(&key)` | the key as a string; a symbol's description |
| `new_symbol(description)` | a new `SymbolRef` |
| `symbol_description(&symbol)` | its description, if any |
| `well_known_symbol(WellKnownSymbol)` | `Symbol.iterator` and the others |
| `get_prototype_of(&object)`, `set_prototype_of(&object, proto)` | read or set the prototype |
| `as_callable(&object)` | `Some(CallableRef)` when the object is callable |
| `is_constructor(&object)`, `is_array_object(&object)`, `is_error_object(&object)` | type tests |
| `define_own_property`, `get_own_property`, `delete_property` | own-property operations |
| `has_property`, `has_own_property`, `own_property_keys` | presence and key lists |
| `type_of(&value)` | `typeof value` |
| `print_value(&value)` | the value as QuickJS prints it, without running JavaScript |
| `qjs_print_value_bytes(&value)` | the same, as the bytes `qjs` writes |

## Context

A `Context` is the handle of the runtime's realm. `Context::new(&runtime)` and `runtime.context()` return it; `runtime()` returns its runtime. Methods that run code take `&mut self`.

### Scripts

| Method | Does |
| --- | --- |
| `eval(&str)`, `eval_bytes(&[u8])` | compiles and runs a script; returns the completion value |
| `eval_with_filename(source, filename)`, `eval_bytes_with_filename` | the same, with the filename used in `stack` |
| `eval_with_options(source, &EvalOptions)`, `eval_bytes_with_options` | the same, with options |
| `compile(&str)`, `compile_bytes`, `compile_with_filename`, `compile_bytes_with_filename` | compiles a script into a `FunctionBytecodeRef` |
| `compile_with_options(source, &CompileOptions)`, `compile_bytes_with_options` | the same, with options |
| `execute(&script)` | runs a compiled script with the global object as `this` |

A syntax error is thrown: the call returns `RuntimeError::Exception` with a `SyntaxError` pending.

### Modules

| Method | Does |
| --- | --- |
| `compile_module(&str)`, `compile_module_bytes`, `compile_module_with_filename`, `compile_module_bytes_with_filename`, `compile_module_bytes_with_options` | compiles a module into a `ModuleBytecodeRef` |
| `compile_module_bytes_with_import_meta(source, filename, import_meta)` | the same, with the initial `import.meta` properties |
| `execute_module(&module)` | links and evaluates the module and its imports; returns the promise of its evaluation |

### Calls

| Method | Does |
| --- | --- |
| `call(&callable, this, &args)` | `callable.call(this, ...args)` |
| `construct(&constructor, &args)` | `new constructor(...args)` |
| `construct_with_new_target(&constructor, &new_target, &args)` | `Reflect.construct(constructor, args, new_target)` |

### Objects and properties

| Method | Does |
| --- | --- |
| `new_object()`, `new_object_with_prototype(proto)` | a new object |
| `new_array()`, `new_array_from_values(values)` | a new array |
| `get_property(&object, &key)`, `get_property_with_receiver` | `[[Get]]` |
| `set_property(&object, &key, value)`, `set_property_with_receiver` | `[[Set]]`; whether it succeeded |
| `get_own_property(&object, &key)` | the own property's descriptor |
| `define_own_property(&object, &key, &descriptor)` | `[[DefineOwnProperty]]` |
| `to_js_string(&value)` | `ToString` |
| `to_number(&value)` | `ToNumber` |
| `detach_array_buffer(&value)` | detaches an `ArrayBuffer` |

### Intrinsics

`global_object()`, `object_prototype()`, `array_prototype()`, `function_prototype()`, `iterator_prototype()`, `string_iterator_prototype()`, `number_prototype()`, `boolean_prototype()`, `string_prototype()`, `symbol_prototype()`, `bigint_prototype()` and `function_constructor()`.

### Exceptions and helpers

| Method | Does |
| --- | --- |
| `has_exception()` | whether an exception is pending |
| `take_exception()` | the pending exception, cleared |
| `new_native_error(kind, message)` | a new error object, not thrown; `kind` is a `NativeErrorKind` (`Eval`, `Range`, `Reference`, `Syntax`, `Type`, `Uri`, `Internal`, `Aggregate`) |
| `install_qjs_helpers(&QjsHelpers)` | defines `print`, `console.log` and `scriptArgs`; with `std: true` also `load` and `read` |

`QjsHelpers` has two fields: `std: bool` and `script_args: Option<Vec<String>>`.

## Values

| Type | Is |
| --- | --- |
| `Value` | `Undefined`, `Null`, `Bool(bool)`, `Int(i32)`, `Float(f64)`, `BigInt(JsBigInt)`, `String(JsString)`, `Symbol(SymbolRef)`, `Object(ObjectRef)` |
| `ObjectRef`, `CallableRef`, `SymbolRef` | handles to engine objects and symbols |
| `PropertyKey` | a string, index or symbol key |
| `OrdinaryPropertyDescriptor` | a possibly partial descriptor of `DescriptorField`s |
| `CompleteOrdinaryPropertyDescriptor` | `Data { .. }` or `Accessor { .. }` |
| `JsString`, `JsBigInt` | JavaScript strings (UTF-16) and BigInts |

[Values and calls](./values.md) covers them with examples.

## Errors

`RuntimeError` is the error of every API call:

| Variant | Meaning |
| --- | --- |
| `Exception` | JavaScript threw; `Context::take_exception` returns the value |
| `Reentered` | a host callback called the API without its `Scope` |
| `Unsupported(&str)` | the engine does not provide this |
| `Compile(Error)` | the front end failed for a reason other than a JavaScript error |
| `OutOfMemory` | the heap cap was reached |
| `Invariant(&str)` | a precondition was broken, such as a handle from another runtime |
| `Internal(String)` | the engine failed internally |

See [Errors and limits](./limits.md).

## Options

### RuntimeOptions

`RuntimeOptions` configures a runtime at creation. `RuntimeOptions::default()` gives the values below, and `RuntimeOptions::from(HeapLimits { .. })` changes only the cap.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `heap` | `HeapLimits` | `max_bytes: usize::MAX` | the heap cap: reserved bytes the heap may not exceed; `usize::MAX` for none |
| `min_heap` | `usize` | 1 MiB | the floor of the collector's reserved-bytes budget |
| `growth` | `f64` | `2.0` | a collection that leaves `live` bytes allows `growth × live` more before the next |
| `stack_limit` | `u32` | 1,048,576 | registers the register file may hold before a call throws `InternalError: stack overflow` |
| `native_stack_budget` | `usize` | 1 MiB | host stack bytes nested re-entries into the interpreter may use |
| `native_depth_limit` | `u32` | `u32::MAX` | the most nested re-entries allowed; for hosts whose stack cannot be measured, such as WebAssembly |

<!-- include: examples/api.rs#runtime-options -->

The embedding API sets `native_stack_budget` itself at each outermost call, from the stack the thread has left: at most 1 MiB, or most of the remaining stack on a thread whose size was declared with `declare_thread_stack(bytes)`. Calls between JavaScript functions stay in one interpreter loop and do not grow the host stack; calls that pass through built-ins (callbacks such as `map`'s, getters, `toString`) and parsing deeply nested source do. A thread that needs that depth declares its stack first thing:

<!-- include: examples/api.rs#thread-stack -->

### EvalOptions and CompileOptions

`EvalOptions` (for `eval_with_options`) and `CompileOptions` (for `compile_with_options` and `compile_module_bytes_with_options`) have the same fields:

| Field | Default | Meaning |
| --- | --- | --- |
| `filename` | `"<input>"` | the name in error positions and `stack` |
| `transient` | `false` | `true` frees the compiled code once nothing references it; `false` keeps it as long as the runtime |

`EvalOptions::new(filename)` and `CompileOptions::new(filename)` set the filename; `.transient()` sets `transient`. An application that evaluates many inputs, such as a REPL, passes `EvalOptions::default().transient()`. Modules ignore `transient` and live as long as the runtime.

<!-- include: examples/api.rs#eval-options -->

## The qjs command

```text
qjs [options] [file [args]]
```

`qjs` runs a file or an expression. A file ending in `.mjs`, or one QuickJS's detection recognizes as a module, runs as an ES module; anything else runs as a script. Arguments after the file are available to JavaScript as `scriptArgs`, with the file name first.

| Flag | Does |
| --- | --- |
| `-e`, `--eval EXPR` | evaluate `EXPR` |
| `-m`, `--module` | load as an ES module |
| `--script` | load as a script |
| `-s` | strip all debug information |
| `--strip-source` | strip only function source text |
| `--no-unhandled-rejection` | ignore unhandled promise rejections |
| `--print-result` | print the script completion value |
| `-q`, `--quit` | create the runtime and exit |
| `--std` | define `load` and `read` |
| `-v`, `--version` | show version and compatibility target |
| `-h`, `--help` | show the usage |
| `--` | end of options |

Memory and measurement:

| Flag | Does |
| --- | --- |
| `--heap-cap-mib N` | cap the heap at `N` MiB |
| `--min-heap-mib N` | reserved heap budget floor (default 1) |
| `--gc-growth F` | bytes allocated between collections per live byte (default 2; must be above 1) |
| `--stack-limit N` | registers the register file may hold |
| `--gc-stats` | print the collector's statistics at exit |
| `--memory-report` | print the memory report at exit |
| `--ic-stats` | print the inline-cache report at exit (`ic-stats` build) |
| `--gc-stress=N` | collect before every `N`-th allocation (`gc-stress` build) |
| `--disassemble` | print the register code of `FILE` and exit |

Test262:

| Flag | Does |
| --- | --- |
| `--test262` | define `$262` (`test262-host` build) |
| `--cannot-block` | forbid `Atomics.wait` from blocking |

`qjs` always defines `print`, `console.log` and `scriptArgs`, loads imported modules from files (`.json` files as JSON, `with { type: "json5" }` as JSON5), and runs the job queue until it is empty. It reports uncaught exceptions and unhandled rejections on stderr.

| Exit status | When |
| --- | --- |
| 0 | the program completed |
| 1 | an uncaught exception, an unhandled rejection, an unreadable file or an engine error |
| 2 | an invalid command line |

```sh
qjs -e 'console.log({ answer: 42 })'
qjs --print-result -e '(function (a) { return a + 1; })(41)'   # 42
qjs --heap-cap-mib 64 --gc-stats big.js
qjs data.js arg1 arg2
```
