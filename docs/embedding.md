# Embedding internals

This page records how JavaScript calls Rust in this engine: host functions and host objects, the operations a host function shares with the top-level `Context`, exceptions that carry their value, interrupts, promises settled from Rust, native modules, byte access to buffers and the conversion layer, with what each costs and the known issues. The API itself, as an embedder uses it, is in [`src/engine/api/README.md`](../src/engine/api/README.md); `examples/host.rs` runs every part of it. The native ABI, the rooting rule and the host boundary are in [`core/README.md`](../core/README.md), the loop in [Execution](performance/execution.md).

Every crate keeps `unsafe_code = "forbid"`, and a runtime has one realm.

## Where it lives

| Module | Crate | Content |
| --- | --- | --- |
| `builtins` (`natives!`) | `quickjs-oxide-core` | `Native::HostFunction`, `Native::HostConstructor` |
| `heap::object` | `quickjs-oxide-core` | `InternalData::HostFn`, `InternalData::HostData`, `InternalTag::HostData` |
| `vm::native` | `quickjs-oxide-core` | `HostFn`, `HostBody`, `NativeCall`, the trampoline `host_call` |
| `embed` | `quickjs-oxide-core` | `NativeCx`, which holds every embedder operation (`Runtime` delegates to it); host functions and objects, promise capabilities, the buffer operations, the interrupt flag |
| `vm::error`, `vm::interp`, `bytecode` | `quickjs-oxide-core` | `vm.interrupted`, `poll_interrupts`, `catchable`, the uncatchable unwind, `Opcode::JmpBack`, the poll counter |
| `host` | `quickjs-oxide-core` | `Host::interrupt` |
| `vm::module` | `quickjs-oxide-core` | `ModuleSource::Synthetic` |
| `api::core::ops` | root crate | `View`, the `js_ops!` macro, the `Js` trait |
| `api::core::function` | root crate | `Scope`, `FunctionOptions`, `HostFunction`, the body wrapper |
| `api::core::value`, `api::core::runtime` | root crate | `Weak` slots, `Poisoned`, `InterruptHandle` |
| `api::core::error` | root crate | `Exception(Value)`, `Interrupted`, `Poisoned` |
| `api::core::module` | root crate | `ModuleScope`, `ModuleSource::Synthetic`, `Context::define_module` |
| `api::core::promise`, `api::core::buffer`, `api::core::convert` | root crate | `PromiseCapability`; `TypedArrayView`, `TypedArrayKind`; `FromJs`, `IntoJs` and the typed `HostFunction` impls |
| `bytecode::builder`, `src/engine/mir/lower` | `quickjs-oxide-core`, root crate | `CodeBuilder::mark_back_edges` turns each backward `Jmp` the lowering emits into `JmpBack` |

The tests are `core/tests/host.rs` (the core's host functions and objects) and `tests/embedding_api.rs` (the API).

## Host functions in the core

**Two natives.** `Native::HostFunction` and `Native::HostConstructor` sit at the end of `natives!` with the trampoline as their function. `HostConstructor` is declared a constructor, so `new` of a `HostFunction` throws `TypeError: not a constructor` through the existing check. Neither is frameless: a host function named `h` is listed in a backtrace as `at h (native)`, as QuickJS lists a C function.

**The body lives in the data object.** A host function is a native closure. Its `FunctionRecord::scope` is an `InternalTag::NativeData` record whose slots hold the JavaScript values the embedder captured (traced like every internal slot), and whose data is `InternalData::HostFn(HostBody)`, an `Rc<dyn HostFn>`. A host object's datum is `InternalData::HostData(Rc<dyn Any>)` on the same kind of record. `Internal::owned_bytes` counts both as 0. The sweep writes `Internal::vacant()` over a freed record, so a body or a datum drops when its object is collected, or at the runtime's drop; the function object's lifetime is the closure's lifetime, with no side table.

The body is an `Rc` because a host function is re-entrant: JavaScript calls it, it calls back into JavaScript, which calls it again. The trampoline clones the `Rc` out of the record, which ends the heap borrow, and calls through the clone. Mutable state goes in a `Cell` or `RefCell` inside the closure.

**The trampoline.** `host_call` reads the body from the callee's data record and calls it with a `NativeCx` and a `NativeCall` (the call site: receiver, arguments, `new.target`, callee, captured values). The arguments stay in the register file, rooted by `top`; `native::call` cuts the root stack back when the body returns, so a body roots freely. `host_call` enters through `enter_native`, so a host function that recurses into JavaScript past the native-stack budget throws `InternalError: stack overflow` as a builtin does.

**`NativeCx`.** A host function runs inside an entry that has a `Cx` and no `Runtime`, so the embedder's operations are on `NativeCx`: the heap, the realm, the services, and the code store, owned (`Runtime::native_cx`) or shared (inside a host function, while the loop holds it). `NativeCx::compiler(transient)` compiles into the code store only when the store is owned and `transient` is false; inside a host function, compiled code is always transient. `new_host_function` allocates the data record and the closure and defines `name` and `length`; a constructor gets no `prototype` property, which the embedder defines, as QuickJS's `JS_NewCFunction2` leaves it to `JS_SetConstructor`.

## Host objects

A host object is an `ObjectKind::Builtin` object with the embedder's prototype and an `InternalTag::HostData` record. It prints as `Object { … }`, is `[object Object]` to `Object.prototype.toString` unless its prototype defines `Symbol.toStringTag`, and is `typeof` `"object"`. `host_data` returns the datum as an `Rc`, so the API hands out a clone without borrowing the heap across the embedder's code. A `Drop` of a datum that releases an `ObjectRef` of the same runtime is deferred like any release made while the runtime is borrowed; a `Drop` that calls an API method gets `RuntimeError::Reentered`.

## The API layer

**One set of operations, two handles.** Every `Context` method borrows the runtime's cell for one entry, so inside a host function a `Context` method returns `Reentered`; a host function works through its `Scope`. Both run their operations on an internal `View`: a `NativeCx`, the runtime's shared state, and a root mark at which the root stack is cut back when the view drops, so temporaries a `lower` creates (a string, a BigInt, an atom) live until the operation ends. The macro `js_ops!` lists each operation once and emits the inherent methods of `Context` (`&self`, one entry each), the inherent methods of `Scope` (`&mut self`, a reborrow of its `NativeCx`), and the sealed trait `Js` for code generic over where it runs. A macro cannot give one list's methods different receivers, which is why every `Context` method takes `&self`.

The job queue, the module loader, the collector, the reports and the options are not in `Js`: they belong to the runtime boundary, and a host function that needs the job queue returns to the embedder, which drains it. Inside a host function, `eval` and `compile` make transient code, and `compile_module` and `execute_module` return `RuntimeError::Unsupported("modules inside a host function")`, because a module's units go into the code store the running loop holds.

**`Scope` and `FunctionOptions`.** A `Scope` reads its call on demand (`this`, `arg`, `argc`, `args`, `new_target`, `is_construct`, `callee`, `data`, `set_data`, `this_host_data`), so a function that takes three integers makes no handle and copies no string. `FunctionOptions` gives the `name`, `length`, whether `new` may call the function, and its captured values. A closure passed to `new_function` writes its parameter type (`|scope: &mut Scope<'_>|`): the bound is reached through the `HostFunction` impl, and the compiler does not infer the higher-ranked signature from it.

**The body wrapper.** The API registers, as the core's body, a wrapper holding a `Weak<Shared>` (the wrapper lives inside the runtime it names) and the embedder's `HostFunction`. It builds the `Scope`, calls the function, and translates the result:

| The function returns | The core receives |
| --- | --- |
| `Ok(value)` | `lower(value)`; a constructor's non-object result passes through, as in QuickJS |
| `Err(Exception(value))` | `Throw(lower(value))` |
| `Err(Interrupted)` | the interrupt's throw, with `vm.interrupted` still set |
| `Err(OutOfMemory)` | `Throw(%OutOfMemoryError%)` |
| `Err(Invariant(_))`, `Err(Internal(_))`, `Err(Unsupported(_))`, `Err(Compile(_))` | `Throw` of `InternalError: <the error's Display>` |

After each call it moves the handles released during the call into the core's handle table, so a script that calls a host function millions of times with object arguments keeps the handles of one call, not of all of them. A guard whose `Drop` sees `std::thread::panicking()` marks the runtime poisoned.

**Handles do not keep the runtime alive.** `Slot` holds a `Weak<Shared>`. A closure stored in the heap that captured an `ObjectRef` would otherwise make the cycle closure → handle → runtime → heap → closure, and the runtime would never drop. A value used after its runtime dropped gets `RuntimeError::Invariant("a value of a dropped runtime")`. A captured handle is a root while its function lives; a JavaScript value the collector should see goes in `FunctionOptions::data`.

**`ModuleScope`.** The module loader's callback view is the compile-time view: it has no services and cannot build a `NativeCx`, so it does not implement `Js`. It makes objects, errors, functions and host objects, and compiles modules, which is what a loader needs to return a compiled or a synthetic module.

**Poisoning.** A panic in a host function unwinds through the loop, leaving its frames and `top` inconsistent. The wrapper's guard sets the runtime poisoned; every later entry returns `RuntimeError::Poisoned`. Nothing catches the panic: it reaches the embedder's call as a panic. The runtime drops normally.

## Exceptions carry their value

`RuntimeError::Exception(Value)` carries the thrown value; there is no pending-exception slot. A thrown value moves from a nested call to the script's `catch` through `?`, and `throw_error(kind, message)` builds an error object with a `stack` from the running frames and returns it as `Exception(value)`, so a host function throws with `return Err(scope.throw_error(NativeErrorKind::Type, "…"))`. The out-of-memory intrinsic still maps to `OutOfMemory`.

## Interrupts

**The poll.** The lowering emits `Opcode::JmpBack` for a `Jmp` whose target precedes it: every loop back-edge and nothing else. Its handler is `Jmp`'s plus a decrement of `vm.poll` and a branch to a cold function at 0; `enter_frame` decrements for every call, as QuickJS's `JS_CallInternal` does. The counter is in `VmStack`, not a local of `run_at`, where a new local spills `pc`. Forward jumps pay nothing.

The cold `poll_interrupts` resets the counter to `INTERRUPT_INTERVAL`, 1,000, then stops the script when the `InterruptHandle` flag is set or `Host::interrupt` returns `true`: it sets `vm.interrupted` and throws a fresh `InternalError: interrupted`. 1,000 costs the same instructions as QuickJS's 10,000 on the V8 v7 programs, and a shorter interval stops a script sooner.

**The hook and the handle.** `Host::interrupt` is polled at that interval; a deadline is `Instant::now() >= deadline` in it. `Runtime::interrupt_handle()` returns an `InterruptHandle` (an `Arc<AtomicBool>`, `Send + Sync`) whose `interrupt()` another thread calls. The flag stays set until `clear()`.

**Uncatchable.** While `vm.interrupted` is set, `run_at`'s throw path skips the handler search and pops every frame (`unwind_uncatchable`), so no `catch` and no `finally` runs, as QuickJS's `JS_IsUncatchableError` check does. The check sits before the throw loop and the unwind is a cold function: a filter on the handler search inside the loop cost 800 bytes of `run_at` and up to 3.5% of instructions. The places that turn a throw into a value test `error::catchable` and pass the throw on instead: the `Promise` constructor's executor and the promise resolution functions (`builtins::promise`, `vm::jobs`), a job's own throw, async function and async generator resumption (`vm::suspend`, `vm::async_generator`), generator resumption (`builtins::generator`), iterator close (`vm::iter`), and module evaluation (`vm::module`).

At the outermost entry the API clears `vm.interrupted` and returns `RuntimeError::Interrupted`, without the error object (reading it would allocate). An interrupted job stays off the queue and the jobs after it stay queued. A host function's nested call that is interrupted returns `Interrupted`, and the wrapper turns it back into the throw with the flag still set.

## Promises from Rust

`NativeCx::new_promise_capability` is `NewPromiseCapability(%Promise%)` over the existing `jobs::new_promise` and resolving functions. The API's `PromiseCapability` holds the promise and its `resolve` and `reject` functions; settling it calls them, so a thenable resolution queues its job as the specification says. `promise_then` attaches host functions as handlers. A host function returns `capability.promise` and keeps the capability in the embedder's state; when its event arrives, the embedder settles it from the top level and calls `run_jobs()`. Only the embedder drains the job queue, at the top level.

## Native modules

`ModuleSource::Synthetic { exports }` is a module whose exports are values: `vm::module::synthetic_module` (the generalization of the JSON module) makes one binding per export and sets each. An export named `default` is the default export. A `ModuleLoader` returns it for a specifier; `Context::define_module(name, exports)` stores it for the name without a loader, as `compile_module` stores a compiled one, and a second definition of a name replaces the first for later imports. Its namespace prints like a compiled module's.

## Buffers

`NativeCx` reads and writes an `ArrayBuffer`'s bytes over `heap::buffer::Buffer`: owned bytes, or atomic bytes in an `Arc` for a `SharedArrayBuffer`, read and written byte by byte as the builtins do. A detached buffer throws `TypeError: ArrayBuffer is detached`; an offset past the end throws `RangeError`. The API's `with_array_buffer_bytes` keeps the borrow inside one entry (the closure has no handle to call the API with) and copies a shared buffer first. `TypedArrayView` reports a typed array's kind, buffer, byte offset and current length, `None` when the view is out of bounds or the buffer is detached; `new_typed_array` constructs through the kind's intrinsic.

## Conversions

`FromJs` and `IntoJs` convert between JavaScript values and Rust types as the specification's abstract operations do: `bool` (`ToBoolean`), `i32` (`ToInt32`), `u32` (`ToUint32`), `f64` (`ToNumber`), `String` and `JsString` (`ToString`), `Option<T>` (`undefined` and `null` are `None`), `Vec<T>` (an array-like: `ToLength` of `length`, then each index), `ObjectRef`, `CallableRef`, and `Rc<T>` of a host datum. A conversion that runs JavaScript (a `valueOf`) runs it through the handle it is given. Where QuickJS has no message for a failed conversion, the message is the nearest builtin's: `not an object`, `not a function`, `<Type> object expected`.

`HostFunction` is implemented for closures over a `Scope` and up to eight converted arguments: a missing argument converts `undefined`, extra ones are ignored, and the result converts by `IntoJs`.

## Strings across the boundary

The core lends a string's flat representation (`NativeCx::string_repr`, which flattens a rope); the API copies that slice once into a `JsString`, and a `JsString` passed back is allocated from its flat storage. `JsString::to_utf8_lossy` takes ASCII and Latin-1 bytes and flat UTF-16 directly. Every string an embedder reads goes this way: host function arguments, `eval` results and property values.

## Measurements

On the V8 v7 programs at full fixed work, against the same tree without this layer (`instructions:u`): richards +0.71%, deltablue +0.93%, crypto +0.23%, raytrace +0.44%, earley-boyer +0.42%, regexp −0.03%, splay +0.21%, navier-stokes +0.72%. Host functions and objects alone move nothing (within 0.02%); the rest is the interrupt poll. `run_at` is 15,210 bytes (gate: 16,384).

A host function call costs about 560 instructions with no arguments. With a 1 KiB string argument, per call:

| Argument type | Latin-1 or ASCII text | UTF-16 text |
| --- | ---: | ---: |
| `JsString` | ~430 | ~630 |
| `String` | ~1,300 | ~56,000 |

## Known issues

- **Recursion depth.** Rust → JavaScript → Rust recursion through a host function fits about 725 levels in the default 1 MiB native-stack budget in a release build and about 46 in a debug build; a builtin's re-entry reaches more (`Array.prototype.map`: 1,063 in release). The 1,000-level test runs on a thread with a 256 MiB stack.
- **UTF-16 text to `String`.** Converting wide text to a Rust `String` costs about 55 instructions per code unit, in std's `String::from_utf16_lossy`. A host function that only reads or forwards text takes `JsString` instead.
- **Interrupt latency.** The poll runs at back-edges and calls only. One long native operation (a regular expression over a large input, a large `join`) is not interrupted until it returns to the loop.
- **The interrupt flag stays set.** `InterruptHandle::interrupt` stops every later evaluation until `clear()`.
- **Host objects print as `Object`.** The printer does not use the prototype's `constructor`; `Symbol.toStringTag` changes only `Object.prototype.toString`.
- **`SharedArrayBuffer` bytes are copied.** `with_array_buffer_bytes` copies a shared buffer before calling the closure, since its bytes can change under the borrow.
- **Modules inside a host function.** `compile_module` and `execute_module` return `Unsupported` there; `eval` and `compile` there make transient code.
- **One thread per runtime.** Handles, host closures and host data are `Rc`; a thread pool creates a runtime on each worker.
- **Untested behavior.** No test pins a host function's backtrace line (`at h (native)`) or the handle table's size over many calls with object arguments; both behave as described.
- **`gc-stress`.** Two older tests in `tests/embedding_api.rs` are ignored under it: filling an 8 MiB heap cap takes minutes when every allocation collects, and a manual GC policy cannot hold off forced collections.
