# Rust embedding API

api is the boundary through which Rust callers use quickjs-oxide. `api::core`
is the API over `quickjs-oxide-core`; `api::error` holds
the front end's diagnostics (`Error`, `ErrorKind`, `NativeErrorKind`); the API
root adds `declare_thread_stack`, `number_to_string` and
`quickjs_detect_module_bytes`. The front end, the lowering and the core's
internals stay private: `check-source-layout.py` checks that `api::core`
re-exports only its own items, the API root's, the front end's errors,
strings, compile options and import attributes, and `quickjs-oxide-core`'s host,
options and report types.

## Shape

| Item | Content |
| --- | --- |
| `Runtime` | an `Rc` of the shared state (`oxide_core::Runtime` in a `RefCell`, the module loader, the preloaded modules); cloning shares it. `new()`, `with_options(RuntimeOptions)`, `with_host(impl Host, RuntimeOptions)`; jobs (`run_pending_job`, `run_jobs`, `with_execution_turn`), rejection tracking, collection (`run_gc`, `gc_stats`, `GcPolicy`, `set_gc_stress`), the reports (`memory_report`, `gc_report`, `ic_report`, `disassemble`), property keys, symbols, prototypes and own-property operations, `type_of` and the printer |
| `Context` | the handle of the runtime's one realm: `eval*`, `compile*`, `execute`, `compile_module*`, `execute_module`, `call`, `construct`, objects, arrays and properties, the intrinsics, `new_native_error`, `throw_error`, host functions and host objects, promises (`new_promise`, `promise_then`), buffers, `define_module`, the `qjs` helpers; each method takes `&self` and is one engine entry |
| `InterruptHandle` | interrupts the runtime's scripts from any thread |
| `PromiseCapability` | a promise and its resolving functions, settled from Rust |
| `FromJs`, `IntoJs` | conversions between JavaScript values and Rust types, and the typed host functions over them |
| `TypedArrayKind`, `TypedArrayView` | a typed array's element type and its window on its buffer |
| `Js`, `JsResult` | the operations `Context` and `Scope` share, as a sealed trait for code generic over either; both also have each as an inherent method |
| `Scope`, `FunctionOptions`, `HostFunction`, `Raw` | host functions: the function's view of its call and of the engine, how it is made, what `new_function` takes |
| `Value` | `Undefined`, `Null`, `Bool`, `Int`, `Float`, `BigInt(JsBigInt)` and `String(JsString)` as Rust data copied out of the core; `Symbol(SymbolRef)` and `Object(ObjectRef)` as handles |
| `ObjectRef`, `SymbolRef`, `CallableRef` | a handle in the core's handle table, released on the last drop |
| `PropertyKey`, `OrdinaryPropertyDescriptor`, `CompleteOrdinaryPropertyDescriptor`, `DescriptorField`, `AccessorValue`, `WellKnownSymbol` | keys and descriptors |
| `RuntimeError` | `Exception(Value)` (the thrown value), `Interrupted`, `Reentered`, `Poisoned`, `Unsupported`, `Compile(Error)`, `OutOfMemory`, `Invariant`, `Internal` |
| `Host`, `Stream`, `RejectionEvent`, `SystemHost` | the core's host trait and its default, re-exported |
| `ModuleLoader`, `ModuleSource`, `CompiledModule`, `ModuleScope` | module loading |
| `RuntimeOptions`, `HeapLimits`, `MemoryReport`, `GcStats` | the core's configuration and reports, re-exported |
| `api::core::test262` | the Test262 runner's worker, behind the `test262-host` feature |

Every `Runtime` and `Context` method borrows the cell for one engine entry.
A host function or callback that calls the API through a `Context` or
`Runtime` instead of its own scope finds the cell borrowed and gets
`RuntimeError::Reentered`. A value dropped while the cell is borrowed
releases its handle when the entry or the host function ends, so `Drop`
always succeeds. A handle does not keep its runtime alive: used after the
last `Runtime` and `Context` dropped, it gets `RuntimeError::Invariant("a
value of a dropped runtime")`.

## Realm

A runtime has one realm. `Runtime::context`, `Runtime::new_context` and
`Context::new` all return its `Context` (deviation `TARGET-REALMS-001`).

## Code lifetime

A script compiled through `Context::eval*` or `Context::compile*` lives as
long as the runtime by default: its code units go in the core's static code
store, which is never collected, and the loop runs them at full speed.
Dropping the `FunctionBytecodeRef` or the completion value frees the
script's objects but not its code.

Code the application compiles and then lets go of (a REPL's lines,
generated snippets evaluated once) takes the `transient` option:
`EvalOptions::default().transient()` for `eval_with_options` and
`eval_bytes_with_options`, `CompileOptions::default().transient()` for
`compile_with_options` and `compile_bytes_with_options`. Its units go in the
heap, as `eval`'s and `new Function`'s do, and the collection that finds no
function, frame or job naming them frees them, with their inline caches and
the atoms only they hold. A `FunctionBytecodeRef` from a transient compile
keeps its code until it is dropped. A call into transient code and the
return from it switch units through a reference count, so a program run
entirely as transient code retires more instructions: about 5% across the
V8 v7 suite and 15% on the call-heavy DeltaBlue. The program and its
modules therefore take the default. An application that compiles input
repeatedly without the option keeps all of that code until the runtime is
dropped.

`Context::compile_module*` and modules a `ModuleLoader` returns live as
long as the runtime. [The heap](../../../docs/heap.md#code-compiled-at-run-time)
describes how dynamic units are reached and freed.

## Host

`Host` is `oxide_core::host::Host`: `write`, `now_ms`,
`local_tz_offset_minutes`, `random_seed`, `promise_rejection`, `read_file`
and `interrupt`, each with a default over the process. `Runtime::new` uses the
core's `SystemHost`; the adapters supply `quickjs-oxide-host`'s `SystemHost`
(the host's time-zone rules) and `quickjs-oxide-web-host`'s `WebHost`. The
application owns input, module-loading policy, output and when the job
queue drains.

## Host functions

`new_function(options, f)` makes a JavaScript function whose body is the
Rust closure `f`, and `define_function(target, name, length, f)` defines one
on an object, writable and configurable but not enumerable, as QuickJS's
`JS_SetPropertyFunctionList` defines a method:

```rust
let global = context.global_object()?;
context.define_function(&global, "add", 2, |scope: &mut Scope<'_>| {
    let (a, b) = (scope.arg(0), scope.arg(1));
    Ok(Value::number(scope.to_number(&a)? + scope.to_number(&b)?))
})?;
```

The closure writes its parameter type (`|scope: &mut Scope<'_>|`): the bound
is reached through the `HostFunction` trait, so the compiler does not infer
the higher-ranked signature. The `Scope` reads the call (`this`, `arg`,
`argc`, `args`, `new_target`, `is_construct`, `callee`, `data`,
`this_host_data`) and has every operation of `Js`. The rules:

- A value the collector should see goes in `FunctionOptions::data` and is
  read with `Scope::data`: a handle the closure captures is a root for as
  long as the function lives, and a cycle through it is never collected.
  The closure reaches the engine through its `Scope`; one that captures a
  `Runtime` or a `Context` keeps the runtime alive.
- A `Context` method inside a host function returns `Reentered`. Inside a
  host function `eval` and `compile` make transient code, and
  `compile_module*` and `execute_module` return
  `Unsupported("modules inside a host function")`.
- Values the closure takes from its call are handles like any other and
  outlive the call. Handles it drops are released when it returns.
- `Err(Exception(value))` throws `value`; `scope.throw_error(kind,
  message)` builds an error with a `stack` from the running frames, so
  `return Err(scope.throw_error(NativeErrorKind::Type, "x is required"))`
  throws a `TypeError`. `OutOfMemory` throws the out-of-memory error, and any
  other error throws an `InternalError` with its message.
- `FunctionOptions::constructor` lets `new` call the function; it has no
  `prototype` property until the embedder defines one. A function made
  without it throws `TypeError: not a constructor` under `new`.
- Calls nested through a host function count against the native-stack
  budget as a builtin's do, and the function is listed in a backtrace as
  `at name (native)`.
- A panic in a host function unwinds to the embedder's call. The runtime is
  then poisoned: every later call returns `RuntimeError::Poisoned`.

`new_host_object(value, prototype)` attaches a Rust value to a new object;
`host_data::<T>(&object)` returns it as an `Rc<T>`, or `None` for an
ordinary object or a value of another type. The value is shared: mutation
goes through a `Cell` or `RefCell` inside it. It drops when the object is
collected. A host object prints as `Object` and is `[object Object]` to
`Object.prototype.toString` unless its prototype defines
`Symbol.toStringTag`.

## Promises and the event loop

`new_promise()` returns a `PromiseCapability`: a pending promise and its
`resolve` and `reject` functions, called through
`capability.resolve(&mut js, value)` and `capability.reject(&mut js,
reason)`. Resolving with a thenable follows it, and settling twice does
nothing. `promise_then(promise, on_fulfilled, on_rejected)` attaches host
functions as `promise.then` does and returns its promise.

The job queue is drained only by the embedder, at the top level. The
pattern for a host operation that completes later: the host function
returns `capability.promise` at once and keeps the capability in the
embedder's state; when the event arrives, the embedder calls `resolve` or
`reject` through its `Context` and then `run_jobs()`, which runs the
reactions (an `await` continues there). `is_job_pending` tells it whether
a drain has work. A capability dropped unsettled leaves its promise
pending, and both are collected once unreachable.

## Conversions

`FromJs::from_js(js, value)` makes a Rust value as the specification's
conversions do: `bool` (`ToBoolean`), `i32` (`ToInt32`), `u32`
(`ToUint32`), `f64` (`ToNumber`), `JsString` and `String` (`ToString`, a
lone surrogate becoming U+FFFD in a `String`), `Option<T>` (`undefined` and
`null` are `None`), `Vec<T>` (an array-like read by `length` and index; a
non-object is a `TypeError`), `ObjectRef` and `CallableRef` (anything else
is a `TypeError`), `Rc<T>` of a host object's value (another value is a
`TypeError`), and `Value` itself. `IntoJs::into_js(self, js)` goes the other
way for the same types, `()` (`undefined`), `&str`, `Vec<T>` (an array)
and `Result<T, RuntimeError>` (the error is thrown). A conversion that runs
JavaScript (`valueOf`, a getter) runs it through `js`, and its throw is the
error.

A host function's closure may take up to eight arguments after the
`Scope`, each converted by `FromJs`, and return any `IntoJs`:

```rust
context.define_function(&global, "add", 2, |_: &mut Scope<'_>, a: f64, b: f64| Ok(a + b))?;
```

A missing argument converts `undefined`, so `Option<T>` is how a function
takes an optional one; extra arguments are ignored; the receiver and
`new.target` stay on the `Scope`. A string argument is copied once: a
1 KiB argument costs about 430 instructions as a `JsString` and about 1,300
as an ASCII `String`, against about 560 for the call itself; converting
UTF-16 text to a `String` transcodes it, about 55 instructions per code
unit.

## Buffers

`new_array_buffer(bytes)` makes an `ArrayBuffer` holding a copy of
`bytes`; `array_buffer_len` reads a buffer's length (`None` for a detached
buffer or another object); `array_buffer_to_vec` copies its bytes out and
`write_array_buffer(buffer, offset, data)` writes into it.
`with_array_buffer_bytes(buffer, f)` lends an `ArrayBuffer`'s bytes to `f`
in place, inside the engine's borrow so `f` cannot call the engine; a
`SharedArrayBuffer`'s bytes, which are atomics, are copied first. A detached
buffer throws `TypeError: ArrayBuffer is detached`, an offset past the end
a `RangeError`, and any other object a `TypeError`. `typed_array_view`
reads a typed array's kind, buffer, byte offset and current length (`None`
when it is out of bounds of a resized buffer); `new_typed_array(kind,
buffer, byte_offset, length)` constructs one, length-tracking without a
`length`.

## Interrupts

A running script stops with `RuntimeError::Interrupted` when the embedder
asks: from another thread through `Runtime::interrupt_handle()` (an
`InterruptHandle`, `Send + Sync` and `Clone`, with `interrupt` and
`clear`), or from the host through `Host::interrupt`, which returns `true`
to stop (a deadline is `Instant::now() >= deadline` there). The loop polls
both about every thousand loop back-edges and calls, so a loop or a
recursion stops within that many steps. The interrupt is uncatchable: no
`catch` and no `finally` runs, no promise is rejected with it and no
iterator is closed. A job interrupted in `run_jobs` leaves the queue, and
the jobs after it stay queued. A host function whose nested call is
interrupted gets `Interrupted` from it and returns it, which keeps the
interrupt unwinding. The handle's flag stays set until `clear`, so a script
started while it is set is interrupted at its first poll.

## Modules and `ModuleScope`

`Runtime::set_module_loader` installs a `ModuleLoader`. Its `normalize`
(defaulting to QuickJS's relative resolution) and `load` callbacks receive a
`&mut ModuleScope<'_>`, a reborrow of the runtime for the callback's
duration: it creates objects, native errors, host functions and host
objects, and compiles a module source (`ModuleScope::compile_module`). A callback throws by returning `Err(Value)`. A
loader returns a `ModuleSource`: source text with the properties its
`import.meta` starts with, JSON, JSON5, or a module compiled through the
scope. Both callbacks receive the import attributes; checking them is the
loader's (`load` rejects what it does not accept). Values made through a
`ModuleScope` are handles like any other and outlive the callback.

A native module is a module whose exports the embedder supplies as values:
the loader returns `ModuleSource::Synthetic { exports }`, a list of names
and values (`default` is the default export, the names distinct), or,
without a loader, `Context::define_module(name, exports)` makes the module
an `import` of `name` finds, as `compile_module` makes a compiled one. A
second definition of the name replaces the first for later imports. Each
export is a binding of its own, so `import { add } from "host:math"`,
`import m from` and `import * as m from` read it as they read a compiled
module's, and a missing name is the usual `SyntaxError` at link.

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
sits on the front end, the lowering and the core, and [embedding
internals](../../../docs/embedding.md) for how host functions, host objects,
interrupts, promises, native modules and buffers are built, what they cost and
their known issues.
