# Rust functions

JavaScript calls Rust through host functions: JavaScript function objects whose body is a Rust closure. This page covers host functions, host objects that carry a Rust value, promises a Rust event settles, native modules whose exports Rust supplies, interrupts that stop a running script, and byte access to buffers. The complete program is [`examples/host.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/host.rs).

## Define a function

`Context::define_function(target, name, length, f)` defines a function on an object, writable and configurable but not enumerable, as QuickJS's `JS_SetPropertyFunctionList` defines a method. `new_function(options, f)` makes one without defining it. After the `Scope`, the closure takes up to eight arguments, each converted from JavaScript, and its result converts back:

<!-- include: examples/host.rs#typed-function -->

A missing argument converts `undefined`, so a function takes an optional argument as `Option<T>`; extra arguments are ignored. The closure writes its parameter types (`|_: &mut Scope<'_>, a: f64|`), because the compiler does not infer them from the trait the closure implements.

| Rust type | Converted as |
| --- | --- |
| `bool` | `ToBoolean` |
| `i32`, `u32` | `ToInt32`, `ToUint32` |
| `f64` | `ToNumber` |
| `JsString`, `String` | `ToString`; a lone surrogate becomes U+FFFD in a `String` |
| `Option<T>` | `undefined` and `null` are `None` |
| `Vec<T>` | an array-like, read by `length` and index |
| `ObjectRef`, `CallableRef` | an object, a callable object |
| `Rc<T>` | the value of a host object holding a `T` |
| `Value` | as is |

A conversion that fails throws the `TypeError` a builtin throws for the same value. The return type converts the same way, plus `()` (`undefined`), `&str`, `Vec<T>` (an array) and `Result<T, RuntimeError>`.

## The Scope

A closure that takes only `&mut Scope<'_>` reads its call itself: `this`, `arg(i)`, `argc`, `args`, `new_target`, `is_construct`, `callee`, and its captured values. The `Scope` also has every operation a `Context` has (calls, properties, objects, conversions), and code generic over the trait `Js` runs on both.

Inside a host function, the `Runtime` and every `Context` are in use: their methods return `Err(RuntimeError::Reentered)`, and the `Scope` is the way in. `eval` there compiles code that is freed once unreachable; compiling or executing a module there returns `Unsupported`.

## Captured values

A JavaScript value the function keeps goes in `FunctionOptions::data`, where the collector sees it, and is read with `Scope::data(i)`:

<!-- include: examples/host.rs#data -->

A handle the Rust closure captures directly is a root for as long as the function lives, and a cycle through it is never collected. A closure that captures a `Runtime` or a `Context` keeps the runtime alive.

## Throw and call back

`Err(RuntimeError::Exception(value))` throws `value`. `scope.throw_error(kind, message)` builds an error object with a stack from the running frames, so a function throws a `TypeError` with `return Err(scope.throw_error(NativeErrorKind::Type, "…"))`. A call back into JavaScript that throws returns the same error, and `?` passes it on to the script's `catch`:

<!-- include: examples/host.rs#call-back -->

`FunctionOptions::new(name).constructor()` lets `new` call the function; `is_construct` and `new_target` tell the call apart. A function made without it throws `TypeError: not a constructor` under `new`. A host function counts against the native stack budget as a builtin does, and a backtrace lists it as `at name (native)`.

A panic inside a host function unwinds to the embedder's call. The runtime is then poisoned: every later call returns `RuntimeError::Poisoned`.

## Host objects

`new_host_object(value, prototype)` makes an object that carries a Rust value; `host_data::<T>(&object)`, or `Scope::this_host_data::<T>()` in a method, returns it as an `Rc<T>`, and `None` for an ordinary object or a value of another type. The value is shared, so mutation goes through a `Cell` or `RefCell` inside it, and it drops when the object is collected:

<!-- include: examples/host.rs#host-object -->

A host object is `typeof` `"object"`, prints as `Object { … }`, and is `[object Object]` to `Object.prototype.toString` unless its prototype defines `Symbol.toStringTag`.

## Promises Rust settles

`new_promise()` returns a `PromiseCapability`: a pending promise and its `resolve` and `reject` functions. A host function that completes later returns the promise at once and keeps the capability; when the event arrives, the embedder settles it from the top level and drains the job queue, where an `await` on the promise continues:

<!-- include: examples/host.rs#promise -->

Resolving with a thenable follows it, and settling twice does nothing. `promise_then(promise, on_fulfilled, on_rejected)` attaches host functions as handlers. [Promises and jobs](./jobs.md) covers the queue.

## Native modules

`Context::define_module(name, exports)` makes a module whose exports are the given values, imported by `name` as any module is. A module loader returns the same as `ModuleSource::Synthetic { exports }` ([Modules](./modules.md)). An export named `default` is the default export:

<!-- include: examples/host.rs#native-module -->

## Interrupts

A running script stops with `RuntimeError::Interrupted` when the embedder asks: from another thread through `Runtime::interrupt_handle()`, an `InterruptHandle` that is `Send + Sync`, or from the host through `Host::interrupt`, which returns `true` to stop, for instance past a deadline ([Host services](./host.md)):

<!-- include: examples/host.rs#interrupt -->

The engine checks about every thousand loop iterations and calls, so a loop or a recursion stops within that many steps; one long native operation, such as a regular expression over a large input, finishes first. The interrupt cannot be caught: no `catch` and no `finally` runs, and no promise is rejected with it. An interrupted job leaves the queue and the jobs after it stay queued. The handle stays set until `clear()`, so a script started while it is set stops at once.

## Buffers

`new_array_buffer(bytes)` makes an `ArrayBuffer` holding a copy of `bytes`. `array_buffer_to_vec` copies a buffer's bytes out, `write_array_buffer(buffer, offset, data)` writes into it, and `with_array_buffer_bytes(buffer, f)` lends the bytes to `f` in place; a `SharedArrayBuffer`'s bytes are copied first. `typed_array_view` reads a typed array's kind, buffer, byte offset and current length, and `new_typed_array(kind, buffer, byte_offset, length)` makes one. A detached buffer throws `TypeError: ArrayBuffer is detached`, and an offset past the end a `RangeError`.

## Strings and cost

A call with no arguments costs about 560 instructions. A string argument is copied once: a 1 KiB argument costs about 430 more as a `JsString` and about 1,300 as an ASCII `String`. Converting UTF-16 text to a Rust `String` transcodes it, about 55 instructions per code unit, so a function that only reads or forwards text takes a `JsString`.
