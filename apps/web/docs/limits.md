# Errors and limits

Every fallible API call returns `Result<_, RuntimeError>`. Most failures are JavaScript exceptions, which stay pending on the context until the embedder takes them. This page covers exceptions, the other `RuntimeError` variants, and the limits a runtime enforces on call depth, host stack and heap memory, plus the collector settings and the memory report.

## RuntimeError

| Variant | Meaning |
| --- | --- |
| `Exception` | JavaScript threw. The thrown value is the context's pending exception. |
| `Reentered` | An API call was made from inside a module loader callback instead of through its `Scope`. |
| `Unsupported(what)` | The source uses a construct the engine does not compile. |
| `Compile(Error)` | The front end failed with an internal or I/O error, not a JavaScript error. |
| `OutOfMemory` | The heap cap was reached: JavaScript did not catch its out-of-memory error, or an API call could not allocate. |
| `Invariant(what)` | The embedder broke one of the API's preconditions, such as passing a value from another runtime. |
| `Internal(message)` | The engine failed internally. |

`RuntimeError` implements `std::error::Error` and `Display`, and converts from the front end's `Error`.

## Exceptions

When JavaScript throws, the call returns `Err(RuntimeError::Exception)` and the context keeps the thrown value:

- `Context::has_exception()` tells whether one is pending.
- `Context::take_exception()` returns it as `Option<Value>` and clears it.
- `Runtime::is_error_object(&object)` tells whether an object is an `Error` instance.
- `Runtime::print_value(&value)` formats any value as QuickJS's `JS_PrintValue` does, the output of `print` and of the `qjs` command's uncaught-exception report. For an error that is `Name: message` followed by its stack. It runs no JavaScript: no getters, proxy traps or `toString` calls.
- `Runtime::qjs_print_value_bytes(&value)` returns the same text as bytes, with a lone surrogate encoded as WTF-8 instead of U+FFFD.

<!-- include: examples/limits.rs#exceptions -->

A thrown value does not have to be an error object. `throw 7` leaves `Value::Int(7)` pending.

Take the exception before the next call that might throw. Each new exception replaces the pending one.

### Syntax errors

Compiling a script or module with a syntax error does not return `RuntimeError::Compile`. As with QuickJS's `JS_Eval`, the error is thrown: the call returns `Err(RuntimeError::Exception)` and a `SyntaxError` with the file, line and column in its stack is pending.

<!-- include: examples/limits.rs#syntax-errors -->

### Creating errors

`Context::new_native_error(kind, message)` creates an error object without throwing it. A module loader creates one through its scope with `Scope::new_native_error` ([Modules](./modules.md#the-scope)). `NativeErrorKind` lists QuickJS's native error types in their QuickJS order: `Eval`, `Range`, `Reference`, `Syntax`, `Type`, `Uri`, `Internal` and `Aggregate`. `NativeErrorKind::name()` gives the constructor name, such as `"RangeError"`.

<!-- include: examples/limits.rs#native-error -->

### Front-end errors

`RuntimeError::Compile` carries the front end's `Error`, with `kind()`, `message()` and `span()`. Its `ErrorKind` is one of `Syntax`, `Type`, `Reference`, `Range`, `JsInternal`, `Internal`, `Io` and `Unsupported`. Only `Internal` and `Io` reach the embedder as `Compile`. The JavaScript kinds are thrown as exceptions, and `Unsupported` becomes `RuntimeError::Unsupported`.

## Call depth and the host stack

Deep recursion in JavaScript throws a catchable `InternalError: stack overflow`. Three limits produce it, all set through `RuntimeOptions`:

| Field | Limits | Default |
| --- | --- | --- |
| `stack_limit` | registers in the interpreter's register file, which every JavaScript call uses | `1 << 20` |
| `native_stack_budget` | bytes of host stack used by nested re-entries into the interpreter | 1 MiB |
| `native_depth_limit` | the number of nested re-entries | `u32::MAX` (no limit) |

<!-- include: examples/limits.rs#stack-overflow -->

A JavaScript call to a JavaScript function stays inside the interpreter and uses only registers, so `stack_limit` bounds ordinary recursion:

<!-- include: examples/limits.rs#stack-limit -->

A re-entry happens when native code calls back into JavaScript, for example `Array.prototype.map` calling its callback, a getter, or a `toString` during a conversion. Each re-entry uses host stack. At its outermost entry the API sets the byte budget from the stack the current thread actually has, replacing the `native_stack_budget` value: most of the remaining stack on a thread declared with `declare_thread_stack`, and at most 1 MiB on any other thread.

`native_depth_limit` is for a host whose stack the byte measure cannot see, such as a WebAssembly engine's:

<!-- include: examples/limits.rs#depth-limit -->

### Declaring the thread's stack

The parser charges each nested construct against a budget modeled on QuickJS's nesting depths. It also checks the host stack, so deeply nested source becomes a catchable `SyntaxError: stack overflow` rather than a crash. On a thread other than the main thread, the engine assumes it has at most 1 MiB of stack unless told otherwise.

To give scripts more room, run the runtime on a thread with a large stack and call `declare_thread_stack(size)` first thing on that thread. The `qjs` command does this for its evaluation thread.

<!-- include: examples/limits.rs#thread-stack -->

## The heap cap

`RuntimeOptions::heap` is a `HeapLimits { max_bytes }`. The default `usize::MAX` means no cap. `RuntimeOptions::from(HeapLimits { .. })` builds options with only the cap set.

The cap applies to the bytes the heap reserves: the arenas holding objects, strings, shapes and the other records, the value heap, the tables beside them, code compiled at run time and regular-expression programs. Scripts compiled through `Context::eval*` and `Context::compile*` without the `transient` option keep their code in a separate code store that the cap does not count. `MemoryReport::reserved_bytes()` is the figure the cap is compared against.

When an allocation would cross the cap, the runtime collects first. If the heap would still cross it, JavaScript gets a catchable `InternalError: out of memory`, which the runtime creates up front so throwing it needs no memory. If the script does not catch it, the API call returns `RuntimeError::OutOfMemory` with no pending exception. An API call from Rust that cannot allocate returns `RuntimeError::OutOfMemory` too.

<!-- include: examples/limits.rs#heap-cap -->

## The collector

The heap is collected by a mark-sweep collector. With the default `GcPolicy::Automatic`, a collection that leaves `live` bytes sets the budget for the next one:

```text
budget = max(min_heap, live × (1 + growth))
```

and the next collection runs when allocation reaches the budget. Two `RuntimeOptions` fields set it:

| Field | Meaning | Default |
| --- | --- | --- |
| `min_heap` | the budget's floor, in bytes | 1 MiB |
| `growth` | how much the heap may grow past the live bytes between collections | `2.0` |

A larger `min_heap` or `growth` means fewer collections and more memory. With `Runtime::set_gc_policy(GcPolicy::Manual)` the runtime collects only at the cap and when `Runtime::run_gc()` is called.

`Runtime::run_gc()` runs a full collection and returns the collector's `GcStats`: `collections`, `total_pause`, `max_pause`, `freed_records`, `freed_bytes` and `live_bytes`. `Runtime::gc_stats()` returns them without collecting. After `Runtime::set_record_gc_pauses(true)`, `Runtime::gc_report()` returns the line `qjs --gc-stats` prints, with pause percentiles.

## The memory report

`Runtime::memory_report()` returns a `MemoryReport` of what each part of the runtime holds, read without a collection:

- one `ArenaReport` per arena (`objects`, `shapes`, `strings`, `symbols`, `bigints`, `cells`, `functions`, `bytecode`, `internals`) with `live`, `capacity`, `used_bytes` and `reserved_bytes`; `arenas()` lists them by name;
- `values`, the value heap's slots, free ranges and reserved bytes;
- `code`, the code units, instructions, position tables, source text and regular-expression programs;
- the atom table, the register file's capacity and high-water mark, and the inline caches;
- `gc`, the collector's `GcStats`, with `allocated_since_gc`, `threshold`, `budget` and `cap`.

`MemoryReport` implements `Display` as the table `qjs --memory-report` prints.

<!-- include: examples/limits.rs#collector -->

[The heap and the collector](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/heap.md) describes the arenas, the trigger and the cap in detail.

The complete program is [`examples/limits.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/limits.rs):

```sh
cargo run --quiet --example limits
```

## Where to go next

- [Host services](./host.md): create a runtime with a host and options.
- [Promises and jobs](./jobs.md): exceptions thrown by jobs.
- [Performance](./performance.md): what the collector settings cost and save.
- [Architecture](./architecture.md): how the heap, the interpreter and the API fit together.
