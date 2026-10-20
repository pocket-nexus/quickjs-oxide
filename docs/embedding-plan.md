# Embedding plan

This page is the plan for the embedding work: JavaScript calling Rust (host functions, host objects, native modules), exceptions that carry their value, interrupts, promises settled from Rust, byte access to buffers, and the conversion layer over all of it. It is written for whoever implements an item: it fixes the decisions that are made, lists the open ones with the item that decides each, and gives every item an acceptance check that can be run without judgment calls. Day estimates order the work; they are not commitments.

The items build on the current implementation: the embedding API described in [`src/engine/api/README.md`](../src/engine/api/README.md); the native ABI, the rooting rule and the host boundary in [`core/README.md`](../core/README.md); the loop and its frames in [Execution](performance/execution.md); the crates in [Architecture](architecture.md). Measurements follow [Measurement method](performance/measurement.md). [Planned work](planned.md) is the plan for the heap cap, code images, heap snapshots and the 32-bit target; this page does not overlap it.

## 0. Scope

**In scope.** A Rust function as a JavaScript function object, with captured JavaScript values and a captured Rust closure (section 2); an embedder's Rust value attached to a JavaScript object (section 3); the API through which a host function reads its call and calls back into the engine, shared with the top-level `Context` (section 4); `RuntimeError::Exception` carrying the thrown value (section 5); an interrupt the embedder raises from another thread or from a host hook, which stops the running script with an uncatchable error (section 6); a promise created and settled from Rust, and the job queue's place in the embedder's event loop (section 7); a module whose exports the embedder supplies as values (section 8); reading and writing an `ArrayBuffer`'s bytes and reading a typed array's window (section 9); `FromJs`/`IntoJs` and typed host functions (section 10); the documentation and the example that show all of it (section 11).

**Outside this plan.** Code images, heap snapshots (the cold-start cost of a runtime: about 3.1 M instructions for the realm), `no_std` and the 32-bit target, which are [Planned work](planned.md). A runtime that can be moved between threads: every handle, host closure and host value is an `Rc`, and making them `Arc` would require every `Host`, `ModuleLoader`, host closure and host datum to be `Send`; a thread pool creates its runtime on the worker thread instead, which costs the realm's initialization once per thread. A second realm in a runtime (deviation `TARGET-REALMS-001`). The `std` and `os` modules and the REPL, which are an application of this API rather than part of it.

**Fixed constraints.** `unsafe_code = "forbid"` in every crate. A new opcode or native is appended at the end of its enum (a mid-enum insertion has cost 5% on the V8 gate). The loop-size gate (`vm::interp::run_at` under 16,384 bytes) and the instruction gate (`instructions:u` of the eight V8 v7 programs against the base branch's `qjs`, a rise above 2% explained by a profile or fixed before merge) of [Execution principles](performance/principles.md) apply to the items that touch the loop (2, 6). One realm per runtime. Versions before 1.0 may change the embedding API, and items 4 and 5 do.

**Decisions taken before this page was written.** A handle (`ObjectRef`, `SymbolRef`) no longer keeps its runtime alive (4.4). The operations shared by `Context` and a host function's `Scope` are named by the trait `Js` (4.1). The module loader's callback view is renamed `ModuleScope` (4.5).

## 1. Location

| Module | Crate | Content |
| --- | --- | --- |
| `builtins` (`natives!`) | `oxide-core` | `Native::HostFunction`, `Native::HostConstructor` (2.1) |
| `heap::object` | `oxide-core` | `InternalData::HostFn`, `InternalData::HostData`, `InternalTag::HostData` (2.2, 3.1) |
| `vm::native` | `oxide-core` | `HostFn`, `NativeCall`, the trampoline `host_call` (2.3) |
| `embed` | `oxide-core` | `NativeCx`, every embedder operation moved onto it, `Runtime` as one-line delegations; `new_host_function`, `new_host_object`, `host_data`, `new_promise_capability`, the buffer operations (2.4, 3.1, 7.1, 9.1) |
| `vm::error`, `vm::interp`, `bytecode` | `oxide-core` | `vm.interrupted`, the uncatchable unwind, `Opcode::JmpBack`, the poll counter (6) |
| `host` | `oxide-core` | `Host::interrupt` (6.2) |
| `vm::module` | `oxide-core` | `ModuleSource::Synthetic` (8.1) |
| `api::core::ops` (new) | root crate | `View`, the `js_ops!` macro, the `Js` trait (4.1) |
| `api::core::function` (new) | root crate | `Scope`, `FunctionOptions`, `HostFunction`, the body wrapper (4.2, 4.3) |
| `api::core::value`, `api::core::runtime` | root crate | `Weak` slots, `Poisoned`, `InterruptHandle` (4.4, 4.6, 6.3) |
| `api::core::error` | root crate | `Exception(Value)`, `Interrupted`, `Poisoned` (5, 6.3) |
| `api::core::module` | root crate | `ModuleScope`, `ModuleSource::Synthetic`, `Context::define_module` (4.5, 8.2) |
| `api::core::buffer` (new) | root crate | `TypedArrayView`, `TypedArrayKind`, the buffer operations (9.2) |
| `api::core::convert` (new) | root crate | `FromJs`, `IntoJs`, the tuple impls of `HostFunction` (10) |
| `src/engine/compiler/register` | root crate | the lowering emits `JmpBack` for a backward `Jmp` (6.1) |
| `apps/cli`, `apps/web`, `conformance/test262`, `tests` | apps | the `Exception(Value)` migration (5.3) |
| `examples/host.rs` | root crate | the example of section 11 |

Delivery: one pull request per item in the order of section 12, each with its acceptance check in the description and the gates of the [pull request template](../.github/pull_request_template.md): `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, `cargo test -p oxide-core --features gc-stress`, `core/tests/corpus` under `gc-stress`, `scripts/checks/check-gc-stress.py`, the loop-size check and the instruction gate for items 2 and 6, and the Test262 focused vector. Each pull request updates the documents that describe what it changes, so that the documentation describes the current implementation at every commit.

## 2. Host functions in the core

### 2.1 Two natives

`Native::HostFunction` and `Native::HostConstructor` are appended to `natives!` with the trampoline of 2.3 as their function, an empty `js_name` and length 0 (both unused: a host function's `name` and `length` come from the embedder). `HostConstructor` is declared `constructor`, so `call_other`'s `native.is_constructor()` guard and `Callee` resolution are unchanged; a host function that `new` may not call throws `not a constructor` through the existing arm. `frameless()` is false for both: a host function is listed in a backtrace as `at name (native)` through `vm.natives`, as every C function is in QuickJS, and `error::native_name` already reads the function object's own `name`.

### 2.2 The body lives in the data object

A host function is a native closure: its `FunctionRecord::scope` is an `InternalTag::NativeData` object whose slots hold the JavaScript values the embedder captured, traced like every internal slot. The Rust body is the same record's data:

```rust
pub enum InternalData {
    // …
    /// A host function's body (`InternalTag::NativeData` of a host closure);
    /// the record's slots are the values it captured.
    HostFn(HostBody),
    /// An embedder's value attached to a host object (`InternalTag::HostData`).
    HostData(Rc<dyn Any>),
}

/// `Rc<dyn HostFn>` with a `Debug` that names the kind and not the closure.
pub struct HostBody(pub Rc<dyn HostFn>);
```

`Internal::owned_bytes` counts both as 0. The sweep writes `Internal::vacant()` over a freed record, so the body and the datum drop when the function or host object is collected, and at the runtime's drop. No table, no sweep hook, no change to `callee()`: the function object's lifetime is the closure's lifetime.

The body is an `Rc` and not a `Box<dyn FnMut>` because a host function is re-entrant: JavaScript calls it, it calls back into JavaScript, which calls it again (an `each` whose callback calls `each`). The trampoline clones the `Rc` out of the record, ending the heap borrow, and calls through the clone. Mutable state goes in a `RefCell` or `Cell` inside the closure.

### 2.3 The trampoline and the public types

`vm/native.rs` gains:

```rust
/// A host function's body.
pub type HostFn = dyn Fn(&mut NativeCx<'_>, NativeCall) -> Result<Value, Throw>;

/// Where a host function finds its receiver, arguments, `new.target` and
/// captured values: `CallSite` made public, read through a `NativeCx`.
#[derive(Clone, Copy)]
pub struct NativeCall(CallSite);

impl NativeCall {
    pub fn this(&self, cx: &NativeCx<'_>) -> Value;
    pub fn arg(&self, cx: &NativeCx<'_>, index: usize) -> Value;   // undefined past argc
    pub fn argc(&self) -> usize;
    pub fn args<'a>(&self, cx: &'a NativeCx<'_>) -> &'a [Value];
    pub fn new_target(&self) -> Value;                               // undefined for a call
    pub fn is_construct(&self) -> bool;
    pub fn callee(&self) -> Value;
    pub fn data(&self, cx: &NativeCx<'_>, slot: usize) -> Value;
    pub fn set_data(&self, cx: &mut NativeCx<'_>, slot: usize, value: Value);
}

pub(crate) fn host_call(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.function.as_object().expect("a host function");
    let data = cx.heap.function_scope(function).expect("a host closure's data");
    let InternalData::HostFn(body) = &cx.heap.internal(data.object()).data else {
        unreachable!("a host function's data holds its body");
    };
    let body = Rc::clone(&body.0);
    let mut native = NativeCx::from_cx(cx);
    body(&mut native, NativeCall(site))
}
```

The arguments stay in the register file, rooted by `top`; `native::call` cuts the root stack back after the body returns, so a body roots freely on it. Nested calls from the body go through the re-entrant `vm::call`, whose `enter_native` applies the native-stack budget: a host function that recurses into JavaScript past the budget gets `InternalError: stack overflow` like a builtin does.

### 2.4 `NativeCx`: one implementation of the embedder's operations

`Cx` and the operations of `embed.rs` (`impl Runtime`) are `pub(crate)` and `pub fn(&mut Runtime, …)`. A host function runs inside an entry with a `Cx` and no `Runtime`, so the operations move:

```rust
/// The embedder's view of a running engine: the heap, the realm, the
/// services and the code store, owned (`Runtime::native_cx`) or borrowed
/// (inside a host function).
pub struct NativeCx<'a> {
    heap: &'a mut Heap,
    code: CodeView<'a>,         // Owned(&'a mut CodeStore) | Shared(&'a CodeStore)
    realm: &'a Realm,
    host: &'a mut Services,
}
```

`NativeCx::cx(&mut self) -> Cx<'_>` builds the loop's context for one operation (four pointers; the owned store reborrowed shared). Every operation of `embed.rs` becomes `impl NativeCx`; `Runtime` keeps each as a one-line delegation through `Runtime::native_cx()`, so the core's tests and any other caller are unchanged. `NativeCx::compiler(transient: bool)` returns `Compiler::owned` when the store is owned and `transient` is false, else `Compiler::new`: inside a host function the store is borrowed by the running loop, so code compiled there is transient whatever the option says (4.1 documents this).

New operations on `NativeCx`:

```rust
pub fn new_host_function(&mut self, name: &str, length: u32, constructor: bool,
                         data: &[Value], body: Rc<dyn HostFn>) -> Result<ObjectId, HeapError>;
```

allocates the data record (`alloc_internal` with `InternalTag::NativeData`, the slots `data`, `InternalData::HostFn`), roots it, allocates the closure through `alloc_native_closure` with `HostFunction` or `HostConstructor`, sets `flags::CONSTRUCTOR` for a constructor, and defines `name` and `length` through `builtins::define_name_length`. A constructor gets no `prototype` property: the embedder defines one, as QuickJS's `JS_NewCFunction2` leaves it to `JS_SetConstructor`.

**Acceptance.** Core unit tests: a host function called from a script with its arguments and receiver; `new` of a `HostConstructor` sees `is_construct` and `new.target`; `new` of a `HostFunction` throws `TypeError: not a constructor`; `data` slots read and written across calls; a thrown `Throw` from the body is caught by a script's `try`; a body that calls `vm::call` on a script function that calls the same host function again (depth 100); a body that recurses until `stack overflow` is thrown and caught; a function created in a loop 10,000 times and dropped leaves `MemoryReport`'s internal-record count where it started after `run_gc`; under `gc-stress`, a body that allocates between reading two arguments. Backtrace: an error thrown by a script function called from a host function named `h` lists `at h (native)` where QuickJS lists its C function.

## 3. Host objects

### 3.1 The record

`InternalTag::HostData` on an `ObjectKind::Builtin` object with the prototype the embedder gives, no slots, `InternalData::HostData(Rc<dyn Any>)`:

```rust
pub fn new_host_object(&mut self, prototype: Option<ObjectId>, data: Rc<dyn Any>)
    -> Result<ObjectId, HeapError>;
/// `Some` for a host object, whatever its datum's type.
pub fn host_data(&self, object: ObjectId) -> Option<Rc<dyn Any>>;
```

The printer names a host object `Object` (`print.rs`'s `ObjectKind::Builtin` arm), `Object.prototype.toString` reads `[object Object]` unless the embedder defines `Symbol.toStringTag` on the prototype, and `typeof` is `"object"`. The datum is an `Rc` so that the API can hand out a clone without borrowing the heap across the embedder's code: the embedder's value is shared, and mutation goes through a `RefCell` inside it.

**Drop.** The datum drops in the sweep that frees the object or in the runtime's drop. A `Drop` that holds an `ObjectRef` of the same runtime releases a handle while the runtime is borrowed, which the existing deferred release handles (`Shared::release` pushes it for the next entry, and 4.3 drains it after every host call). A `Drop` that calls an API method gets `RuntimeError::Reentered`, which is the rule the README states.

**Acceptance.** A host object's datum is read back with its type; a datum of another type reads as `None`; the datum of a collected host object is dropped (an `Rc` whose strong count the test watches) after `run_gc` and nothing else; the datum of a live one is not; a `Drop` that releases an `ObjectRef` does not panic and the handle count returns to its start at the next entry; under `gc-stress`, creating 1,000 host objects in a script loop.

## 4. The API: `Js`, `Scope`, handles

### 4.1 One set of operations, two handles

Every `Context` method borrows the runtime's cell for one entry. Inside a host function the cell is borrowed, so a `Context` method returns `RuntimeError::Reentered`; this stays the rule, and the host function's handle is a `Scope` (4.2). To write the operations once, they move onto an internal `View`:

```rust
pub(crate) struct View<'a> {
    core: NativeCx<'a>,
    shared: &'a Rc<Shared>,
    mark: RootMark,            // the root stack is cut back when the view drops
}
```

`View` holds what `Entry` holds today minus the `RefMut` and the pending exception: `lift`, `lower`, `key`, `lift_key`, `thrown`, `complete`, the descriptor conversions, and the body of every `Context` and object-level `Runtime` method. Temporaries a `lower` creates (a string, a BigInt, an atom) are pushed on the root stack instead of `Inner::temps`; the view's drop truncates to its mark, inside an entry or inside a host function alike.

A macro `js_ops!` lists each operation once, with its documentation, and emits three things: the inherent methods of `Context` (each `self.enter()?` then `entry.view().op(args)`), the inherent methods of `Scope` (each a reborrow of its `NativeCx` into a `View`), and the trait:

```rust
/// The operations `Context` and `Scope` share, for code generic over where
/// it runs. Sealed; both types also have each as an inherent method.
pub trait Js: sealed::Sealed {
    fn eval(&mut self, source: &str) -> JsResult<Value>;
    fn call(&mut self, function: &CallableRef, this: Value, args: &[Value]) -> JsResult<Value>;
    // … every operation of the list
}
pub type JsResult<T> = Result<T, RuntimeError>;
```

Inherent methods take precedence, so existing callers need no import; a helper `fn make_point(js: &mut impl Js, x: f64) -> JsResult<ObjectRef>` imports the trait. The list is every method of `Context` today except `runtime()`, `try_clone()` and `install_test262_host`, plus `Runtime`'s object operations (`get_prototype_of`, `set_prototype_of`, `as_callable`, `is_constructor`, `is_array_object`, `is_error_object`, `delete_property`, `has_property`, `has_own_property`, `own_property_keys`, `type_of`, `print_value`, `qjs_print_value_bytes`, `new_symbol`, `symbol_description`, `well_known_symbol`, `promise_snapshot`), plus the new operations of this plan: `new_function`, `define_function`, `new_host_object`, `host_data`, `throw_error`, `new_promise` (7.2), the buffer operations (9.2). Not in the list, because they belong to the runtime boundary and not to a host function: `run_pending_job`, `run_jobs`, `with_execution_turn`, `set_module_loader`, `run_gc`, the reports and the options; a host function that needs the job queue returns to the embedder, which drains it.

Inside a host function, `eval` and `compile` compile transient code (2.4), and `compile_module` and `execute_module` return `RuntimeError::Unsupported("modules inside a host function")`: a module's units go in the owned code store, which the running loop borrows.

### 4.2 `Scope` and `FunctionOptions`

```rust
/// A host function's view of the engine: its call, and every operation of
/// `Js`. Values it hands out are handles like any other and outlive the call.
pub struct Scope<'a> {
    core: NativeCx<'a>,
    shared: Rc<Shared>,
    call: NativeCall,
}

impl Scope<'_> {
    pub fn this(&mut self) -> Value;
    pub fn arg(&mut self, index: usize) -> Value;        // Undefined past argc
    pub fn argc(&self) -> usize;
    pub fn args(&mut self) -> Vec<Value>;
    pub fn new_target(&mut self) -> Value;
    pub fn is_construct(&self) -> bool;
    pub fn callee(&mut self) -> CallableRef;
    pub fn data(&mut self, slot: usize) -> Value;
    pub fn set_data(&mut self, slot: usize, value: Value) -> JsResult<()>;
    /// `host_data::<T>` of the receiver.
    pub fn this_host_data<T: 'static>(&mut self) -> JsResult<Option<Rc<T>>>;
}

/// How a host function is made: its `name` and `length`, whether `new` may
/// call it, and the values it captures (`Scope::data`).
#[derive(Clone, Debug, Default)]
pub struct FunctionOptions { pub name: String, pub length: u32, pub constructor: bool, pub data: Vec<Value> }
impl FunctionOptions {
    pub fn new(name: impl Into<String>) -> Self;
    pub fn length(self, length: u32) -> Self;
    pub fn constructor(self) -> Self;
    pub fn data(self, data: Vec<Value>) -> Self;
}

/// What `new_function` takes: a closure over a `Scope` (and, from section
/// 10, a closure over converted arguments). `Args` is the marker the
/// conversion layer selects on.
pub trait HostFunction<Args>: 'static {
    fn call(&self, scope: &mut Scope<'_>) -> JsResult<Value>;
}
pub struct Raw;
impl<F> HostFunction<Raw> for F where F: Fn(&mut Scope<'_>) -> JsResult<Value> + 'static { … }
```

The call is read through `Scope` methods and not through a separate argument type so that a borrow of the engine is never split: `arg(i)` lifts on demand, so a function that takes three integers makes no handle and copies no string. `callee()` lets a function find its own object (a counter stored in `data` slot 0 is the usual use instead).

The operations: `new_function(options, f) -> JsResult<CallableRef>`; `define_function(target, name, length, f) -> JsResult<CallableRef>` defines the function on `target` writable and configurable, not enumerable, as QuickJS's `JS_SetPropertyFunctionList` defines a method; `new_host_object::<T: 'static>(value: T, prototype: Option<&ObjectRef>) -> JsResult<ObjectRef>`; `host_data::<T: 'static>(&ObjectRef) -> JsResult<Option<Rc<T>>>`, `None` for an ordinary object or another type.

A closure passed to `new_function` writes its parameter type (`|scope: &mut Scope<'_>|`): the bound is reached through the `HostFunction` impl, so the compiler does not infer the higher-ranked signature. The README's examples all write it.

### 4.3 The body wrapper

The API registers, as the core's `Rc<dyn HostFn>`, a wrapper that captures a `Weak<Shared>` (not an `Rc`: the wrapper lives inside the runtime it names) and the embedder's `HostFunction`:

1. upgrade the `Weak` (it cannot fail during a call, since the runtime is running);
2. build the `Scope`, call the embedder's function;
3. translate the result:

| The function returns | The core receives |
| --- | --- |
| `Ok(value)` | `lower(value)`; a constructor's non-object result passes through, as QuickJS passes it |
| `Err(Exception(value))` | `Throw(lower(value))` |
| `Err(Interrupted)` | the interrupt's throw, with `vm.interrupted` still set (6.3) |
| `Err(OutOfMemory)` | `Throw(%OutOfMemoryError%)` |
| `Err(Invariant(_))`, `Err(Internal(_))`, `Err(Unsupported(_))`, `Err(Compile(_))` | `Throw` of `InternalError: <the error's Display>` |
| `Err(Reentered)`, `Err(Poisoned)` | cannot occur: a `Scope` does not borrow the cell |

4. drain `Shared::released` into the core's handle table with the `NativeCx` in hand. Today a handle dropped while the cell is borrowed waits for the next entry; a script that calls a host function ten million times with an object argument would otherwise keep ten million released handles as roots until the `eval` returns. The drain after every host call bounds that to the handles of one call;
5. a guard struct whose `Drop` tests `std::thread::panicking()`: a panic that unwinds through the wrapper sets `Shared::poisoned` (4.6).

`JsResult<Value>` is what the embedder returns; `throw_error(kind, message) -> RuntimeError` builds an error object of `kind` with a `stack` from the running frames (the core's `new_error`) and returns `RuntimeError::Exception(value)`, so a function throws with `return Err(scope.throw_error(NativeErrorKind::Type, "x is required"))`.

### 4.4 Handles do not keep the runtime alive

`Slot::shared` becomes `Weak<Shared>`. The reason is the host function: a Rust closure that captures an `ObjectRef` and is stored in the heap (2.2) makes the cycle closure → `ObjectRef` → `Rc<Shared>` → heap → closure, and the runtime never drops. With a `Weak`, dropping the last `Runtime` drops the heap, the closures and their captured handles, whose `Drop` finds `upgrade()` returning `None` and does nothing. The observable change: a value used after its runtime dropped gets `RuntimeError::Invariant("a value of a dropped runtime")` from `own()`, which compares the slot's pointer with the entered runtime's and reads the weak count to pick the message. `belongs_to` compares pointers the same way. A captured handle is still a root while its function lives, so a JavaScript value a host function should let the collector see goes in `FunctionOptions::data`, which the README's host-function section states first.

### 4.5 `ModuleScope`

The module loader's `Scope` is renamed `ModuleScope`: it is the compile-time view (`Compiler::reborrow`), has no `Services` and cannot build a `NativeCx`, so it does not implement `Js`. It keeps `new_object`, `new_native_error` and `compile_module`, and gains `new_function` and `new_host_object`, which need only the heap and the realm, so that a loader can build a synthetic module's exports (8.2). Callers to update: `apps/cli/src/main.rs`, `apps/cli/tests/oracle/{module_reentry,raw_json_module_bytes}.rs`, `tests/{embedding_api,unsupported_diagnostics}.rs`.

### 4.6 Poisoning

A panic inside a host function unwinds through the loop with its frames, `top` and `vm.natives` inconsistent, and through the `RefMut`, which releases the cell. The wrapper's guard sets `Shared::poisoned`; every later `enter()` returns `RuntimeError::Poisoned` and the value's `Display` says so. Nothing catches the panic: it reaches the embedder's `eval` call as a panic, which is what a bug in the embedder's code should do. The runtime is dropped normally.

**Acceptance (4).** `tests/embedding_api.rs`: a function defined on the global object and called from a script with every argument kind; `this` for a method call; `argc` past and under `length`; `Scope::eval` runs; a function that calls a script closure that throws, propagated by `?` to the script's `catch` with the same object; `throw_error` from Rust caught by a script with `instanceof TypeError` and a `stack` naming the calling script line; a Rust-to-JavaScript-to-Rust recursion of depth 1,000; handles taken from `arg` stored in a `RefCell<Vec<Value>>` and used after the call; ten million calls with an object argument keep the handle table's `live_count` under 16 after each entry; a `Runtime` whose global holds a host function that captured an `ObjectRef` drops (an `Rc` the closure captures reaches strong count 0 after `drop(runtime)`); after a panic in a host function (caught by the test with `catch_unwind` outside the API), every method returns `Poisoned`; `Context::eval` inside a host function returns `Reentered`; a `Scope::compile_module` call returns `Unsupported`; the `Js` helper pattern compiles for both handles. Clippy on the macro's expansion is clean.

## 5. Exceptions carry their value

`RuntimeError::Exception` becomes `Exception(Value)`, and the pending-exception slot goes with it: `Inner::exception`, `Context::has_exception` and `Context::take_exception` are removed. `Entry::thrown` lifts the thrown value into the error; the out-of-memory intrinsic still maps to `OutOfMemory`. `RuntimeError` keeps `Clone`, `Debug`, `PartialEq` (`Value` has them).

Why: every caller today writes `Err(RuntimeError::Exception) => context.take_exception()`, and inside a host function the two-step form would mean a pending slot per `Scope` and a rule for a slot left unread. With the value in the error, `?` carries it from a nested call to the script's `catch` (4.3) and `scope.throw_error` is one expression.

**5.3 The migration.** 255 uses in 50 files, mechanical: `Err(RuntimeError::Exception)` as a pattern becomes `Err(RuntimeError::Exception(value))` and the `take_exception` that followed it goes; as a value in an assertion it becomes `matches!`. `apps/cli/src/main.rs`'s `report_pending_exception(runtime, context)` becomes `report_exception(runtime, &value)`; `apps/web/src/lib.rs`'s `EvalResult::exception` takes the value; `tests/common/mod.rs` and the oracle support modules (`apps/cli/tests/oracle/support/*`) change in one place each and their cases follow. This item lands first (section 12) so that section 4 is written against the final error type.

**Acceptance.** `cargo test --workspace --all-targets` with every former `take_exception` site asserting the same message as before; `examples/eval.rs` updated; the API README's `RuntimeError` row updated.

## 6. Interrupts

### 6.1 The poll

The lowering emits a new opcode `Opcode::JmpBack` (appended) for a `Jmp` whose target precedes it: every loop back-edge, nothing else. Its handler is `Jmp`'s plus a decrement of `vm.poll: u32` and a branch to a cold function when it reaches 0; `enter_frame` does the same decrement for every call, as QuickJS's `JS_CallInternal` does. The counter lives in `VmStack` and not in a local of `run_at` ([a new loop local spills `pc`](../docs/performance/execution.md)); the handler is four machine instructions on a memory location. Forward jumps pay nothing.

The cold `poll_interrupts(cx)` resets the counter to `INTERRUPT_INTERVAL` (10,000, QuickJS's; open decision 13.1), reads the `InterruptHandle` flag (an atomic load), and when it is clear calls `Host::interrupt` (6.2). When either asks, it sets `vm.interrupted = true` and returns `Throw` of a fresh `InternalError: interrupted`.

### 6.2 The hook and the handle

```rust
pub trait Host {
    // …
    /// Polled about every ten thousand loop back-edges and calls while a
    /// script runs: `true` stops the script with an uncatchable
    /// `InternalError: interrupted`. The default never interrupts.
    fn interrupt(&mut self) -> bool { false }
}
```

A deadline is `Instant::now() >= self.deadline` in the hook. For another thread, `Runtime::interrupt_handle() -> InterruptHandle` (an `Arc<AtomicBool>`, `Send + Sync`, `Clone`) with `interrupt()` and `clear()`; the flag is read at the poll, not at every back-edge, so the latency is the poll interval. The flag stays set until cleared, so an embedder that interrupts clears it before the next `eval`.

### 6.3 Uncatchable, and what the embedder sees

The object header's flag bits are all taken, so the mark is on the VM: while `vm.interrupted` is set, the unwinder (`run_at`'s throw loop) skips the handler search, so no `catch` and no `finally` runs, as QuickJS's `JS_IsUncatchableError` check does before its handler search. The natives that turn a throw into something else must do the same while the flag is set, and return the throw instead: the `Promise` constructor's executor call, `jobs::resolve_promise` (the `then` read), a job's own throw (`run_one_job` reports it as the job's result today), the async function and async generator resumption (`vm::suspend`), iterator close (`vm::iter`), `Array.prototype.sort`'s comparator, module evaluation (`ModuleAsyncRejected`). The implementing pull request greps `Err(Throw(` in `core/src` for the sites that recover from a throw and lists in its description each one and whether it is reachable while a script runs.

At the outermost entry the API clears `vm.interrupted` and returns `RuntimeError::Interrupted` (no value: the error object is not useful and reading it would allocate). A job interrupted inside `run_jobs` stays off the queue, the jobs after it stay queued, and `run_jobs` returns `Interrupted`. A host function whose nested `scope.call` is interrupted gets `Interrupted` from it, and the wrapper turns it back into the throw with the flag still set (4.3).

**Acceptance.** `tests/embedding_api.rs`: `while (true) {}` interrupted from a thread through the handle within one second, the API returning `Interrupted`; the same through a `Host::interrupt` deadline; `try { while (true) {} } catch (e) { caught = true }` leaves `caught` undefined; `try { while (true) {} } finally { ran = true }` leaves `ran` undefined; `new Promise(() => { while (true) {} })` returns `Interrupted` and leaves no rejected promise; an interrupted job leaves the next job runnable; a host function's nested call interrupted propagates `Interrupted` to the outer `eval`; after `clear()` the runtime evaluates again. Core: `JmpBack` round-trips through the validator and the disassembler, and `scripts/checks` fixtures that print disassembly are updated. Gates: the loop size under 16,384 bytes; the instruction gate, where the expected rise is under 1% on the eight programs (one decrement and branch per back-edge and per call) and any program above 2% is profiled in the description.

## 7. Promises from Rust

### 7.1 The core

```rust
/// `NewPromiseCapability(%Promise%)`: the promise and its resolving
/// functions, unrooted.
pub fn new_promise_capability(&mut self) -> Result<(ObjectId, ObjectId, ObjectId), Throw>;
```

over `jobs::new_promise(PromisePrototype)` and `jobs::resolving_functions`, which exist.

### 7.2 The API

```rust
pub struct PromiseCapability { pub promise: ObjectRef, pub resolve: CallableRef, pub reject: CallableRef }
impl PromiseCapability {
    pub fn resolve(&self, js: &mut impl Js, value: Value) -> JsResult<()>;
    pub fn reject(&self, js: &mut impl Js, reason: Value) -> JsResult<()>;
}
// In `Js`:
fn new_promise(&mut self) -> JsResult<PromiseCapability>;
/// `promise.then(on_fulfilled, on_rejected)` with host functions.
fn promise_then(&mut self, promise: &ObjectRef, on_fulfilled: impl HostFunction<Raw>,
                on_rejected: impl HostFunction<Raw>) -> JsResult<ObjectRef>;
```

`resolve` and `reject` are calls of the resolving functions, so a thenable resolution queues its job as the specification says. The pattern the README states: a host function returns `capability.promise` at once and keeps `resolve` in the embedder's state; when the event arrives, the embedder calls `resolve` from the top level (a `Context`) and then `run_jobs()`. The job queue is drained only by the embedder, at the top level, after each of its events; `is_job_pending` tells it whether to.

**Acceptance.** A host function returning a promise that a script awaits; `resolve` from the top level after the script returned, then `run_jobs`, then the awaiting continuation ran (observed through a global); `reject` observed by a `catch`; resolving with a thenable follows it; `resolve` twice is a no-op; `promise_then` handlers run with the value; `promise_snapshot` reports each state; a `PromiseCapability` dropped unresolved leaves a pending promise and no leak after `run_gc`.

## 8. Native modules

### 8.1 The core

`ModuleSource::Synthetic { exports: Vec<(String, Value)> }`: `vm::module::synthetic_module` generalizes `json_module` (one non-lexical `Declaration` binding per export, an `ExportTarget::Local(i)` per name, `create`, then the cell set per binding). The values must be rooted by the caller, as `Compiled`'s `import_meta` values must. An export named `default` is the default export; `import * as m` sees every name.

### 8.2 The API

`ModuleSource::Synthetic { exports: Vec<(String, Value)> }` for a `ModuleLoader` (whose `ModuleScope` can now make functions and host objects, 4.5), and, for the embedder without a loader:

```rust
// `Context` only (it touches the preloaded-module map, not the engine):
pub fn define_module(&mut self, name: &str, exports: Vec<(String, Value)>) -> JsResult<()>;
```

which stores a `Preloaded::Synthetic` that `load_shim` returns when the name is imported, exactly as a compiled module from `compile_module` is returned. A second `define_module` of the same name replaces the first for later imports, as `compile_module` does.

**Acceptance.** `import { add } from "host:math"` of a `define_module` with a host function; `import m from` with a `default` export; `import * as m` lists the sorted names; a loader returning `Synthetic` for a specifier; a missing name is the usual `SyntaxError` at link; the synthetic module's namespace prints like a compiled one's in `qjs`; under `gc-stress`, the exports survive linking.

## 9. Buffers

### 9.1 The core

On `NativeCx`, over `heap::buffer::Buffer` (owned bytes or an `Arc` of atomic bytes for a `SharedArrayBuffer`) and `View`:

```rust
pub fn new_array_buffer(&mut self, len: usize) -> Result<ObjectId, Throw>;      // zeroed; RangeError past MAX_BUFFER_LENGTH
pub fn array_buffer_len(&self, object: ObjectId) -> Option<usize>;            // None: not a buffer or detached
pub fn array_buffer_read(&self, object: ObjectId, offset: usize, out: &mut [u8]) -> Result<(), Throw>;
pub fn array_buffer_write(&mut self, object: ObjectId, offset: usize, data: &[u8]) -> Result<(), Throw>;
pub fn array_buffer_bytes(&self, object: ObjectId) -> Option<&[u8]>;          // owned, attached; None for shared
pub fn typed_array_view(&self, object: ObjectId) -> Option<(ElementKind, ObjectId, usize, Option<usize>)>;
```

A detached buffer throws `TypeError: ArrayBuffer is detached` from read and write; an offset past the end throws `RangeError`. A shared buffer is read and written byte by byte through its atomics, as the builtins do.

### 9.2 The API

```rust
pub use oxide_core::heap::buffer::ElementKind as TypedArrayKind;
pub struct TypedArrayView { pub kind: TypedArrayKind, pub buffer: ObjectRef, pub byte_offset: usize, pub length: usize }
// In `Js`:
fn new_array_buffer(&mut self, bytes: &[u8]) -> JsResult<ObjectRef>;
fn array_buffer_len(&mut self, buffer: &ObjectRef) -> JsResult<Option<usize>>;
fn array_buffer_to_vec(&mut self, buffer: &ObjectRef) -> JsResult<Vec<u8>>;
fn with_array_buffer_bytes<R>(&mut self, buffer: &ObjectRef, f: impl FnOnce(&[u8]) -> R) -> JsResult<R>;  // a shared buffer is copied first
fn write_array_buffer(&mut self, buffer: &ObjectRef, offset: usize, data: &[u8]) -> JsResult<()>;
fn typed_array_view(&mut self, array: &ObjectRef) -> JsResult<Option<TypedArrayView>>;
fn new_typed_array(&mut self, kind: TypedArrayKind, buffer: &ObjectRef, byte_offset: usize, length: Option<usize>) -> JsResult<ObjectRef>;  // `construct` of the kind's intrinsic
```

`with_array_buffer_bytes` keeps the borrow inside the entry, so the closure cannot call the API (it has no handle to call it with). `length` of a view is its current length (`View::current_length`), `None` when out of bounds.

**Acceptance.** Round trip of 1 MiB through `new_array_buffer` and `array_buffer_to_vec`; `write_array_buffer` seen by a `Uint8Array` in a script; a `Float64Array`'s view reports kind, offset and length; a length-tracking view on a resized buffer reports the current length; a detached buffer throws `TypeError` with QuickJS's message; a `SharedArrayBuffer` round trip; offsets at and past the end.

## 10. Conversions and typed functions

```rust
pub trait FromJs: Sized { fn from_js(js: &mut impl Js, value: Value) -> JsResult<Self>; }
pub trait IntoJs { fn into_js(self, js: &mut impl Js) -> JsResult<Value>; }
```

`FromJs` for `Value` (identity), `bool` (`ToBoolean`), `i32` (`ToInt32`), `u32`, `f64` (`ToNumber`), `String` and `JsString` (`ToString`), `Option<T>` (`undefined` and `null` are `None`), `Vec<T>` (an array-like: `length` then indices; a non-object is a `TypeError`), `ObjectRef` (a non-object is a `TypeError`), `CallableRef` (a non-callable likewise), `Rc<T>` for a host datum (a wrong type is a `TypeError`). `IntoJs` for the same, `()` (`undefined`), `&str`, `Vec<T>` (an array), `Option<T>` (`None` is `undefined`), `Result<T, RuntimeError>`.

The tuple impls of `HostFunction` (4.2), by macro up to eight arguments:

```rust
impl<F, R, A0, A1> HostFunction<(A0, A1)> for F
where F: Fn(&mut Scope<'_>, A0, A1) -> JsResult<R> + 'static, A0: FromJs, A1: FromJs, R: IntoJs { … }
```

A missing argument converts `undefined` (so `Option<T>` is how a function takes an optional one); extra arguments are ignored; the receiver and `new.target` stay on the `Scope`. With this, `scope.define_function(&global, "add", 2, |_: &mut Scope<'_>, a: f64, b: f64| Ok(a + b))` is the whole definition. Conversion errors are the `TypeError`s the builtins throw for the same values.

**Acceptance.** Each impl in both directions with the specification's edge cases (`NaN` to `i32`, a lone surrogate to `String`, a sparse array to `Vec`); a typed function of eight parameters; the `Option` cases; the error messages against QuickJS's for `ToNumber` of a symbol and `ToString` of a symbol.

## 11. Documentation and the example

- `src/engine/api/README.md`: a "Host functions" section with the rules (a `Context` method inside a host function is `Reentered`; a value the collector should see goes in `data`; a handle outlives its call; a panic poisons; the stack budget), "Interrupts", "Promises and the event loop", "Buffers", "Conversions", and the `Shape` table's new rows; the `Exception(Value)` row; `ModuleScope`.
- `docs/architecture.md`: `interrupt()` in the host-boundary table; one sentence on host functions where the module shims are described.
- `docs/compatibility.md`: the "C API" row becomes "the Rust embedding API provides the equivalents (host functions, host objects, native modules, interrupts, buffers); no C ABI".
- `core/README.md`: the native ABI paragraph gains the host function's trampoline and data object; the host-boundary section gains `interrupt`.
- `README.md`: the embedding section shows one host function.
- `examples/host.rs`: a function on the global object, a function with `data`, a host object with a method reading its datum, a call back into a script closure, a thrown `TypeError` caught by the script, a promise resolved after the script returns, a native module imported by a module script, an interrupt from a thread.

## 12. Order and estimates

| Item | Lands | Depends on | Estimate |
| --- | --- | --- | --- |
| 5. `Exception(Value)` | the error type and the migration | none | 1 day |
| 2, 3. Host functions and host objects in the core | `NativeCx`, the natives, the records, the operations; core tests | none | 2 days |
| 4. The API | `View`, `js_ops!`, `Js`, `Scope`, `FunctionOptions`, the wrapper, `Weak` slots, poisoning, `ModuleScope`; `tests/embedding_api.rs` | 5, 2, 3 | 3 days |
| 6. Interrupts | `JmpBack`, the poll, `Host::interrupt`, `InterruptHandle`, the uncatchable unwind and its audit | 4 | 2 days |
| 7, 8. Promises and native modules | `new_promise_capability`, `PromiseCapability`, `promise_then`, `Synthetic`, `define_module` | 4 | 1.5 days |
| 9. Buffers | the core operations and the API | 4 | 1 day |
| 10. Conversions | `FromJs`, `IntoJs`, the tuple impls | 4 | 1.5 days |
| 11. The example and the README's embedding section | `examples/host.rs` | all | 0.5 day |

Each row is one pull request, stacked on the previous; the documents of section 11 are updated in the pull request that changes what they describe.

**Measured before it is designed.** Lifting a string argument copies its units into a `JsString` once per read. Before any zero-copy variant is added, the pull request of item 10 records a microbenchmark (a host function taking a 1 KiB string, one million calls) beside the empty-function cost of item 2's gate; a borrowed string view is designed only if the copy is more than a third of the call.

## 13. Open decisions

| # | Decision | Decided by |
| --- | --- | --- |
| 13.1 | `INTERRUPT_INTERVAL`: 10,000 (QuickJS) unless the instruction gate shows a lower count is free, in which case the latency argument wins | item 6's measurement |
| 13.2 | Whether `with_array_buffer_bytes` on a `SharedArrayBuffer` copies (this page) or returns `Unsupported` | item 9, by whether any caller wants the copy |
| 13.3 | The names of the `FromJs` error messages where QuickJS has none (a `Vec<T>` from a non-object) | item 10, matched to the nearest builtin's |
