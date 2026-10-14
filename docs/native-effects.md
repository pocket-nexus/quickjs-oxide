# Native effects

Status: piloted on `Array.from`/`Array.of` (`builtins/array/build.rs`); other builtins still use their own transport.

## Problem

A resumable builtin cannot call back into JavaScript directly. It returns a
*step* that names one effect (read a property, call a function, set a property,
…), and whoever drives it performs the effect and hands the result back. Today
every builtin re-implements that transport by hand:

| Per builtin                                                     | Total in `src/` |
| --------------------------------------------------------------- | --------------: |
| its own `*Step` enum with one variant per effect kind           |              67 |
| a `*StepPending` struct, one `Option` field per effect argument |       706 lines |
| `request_*` constructors that fill those fields                 |     1,889 lines |
| `take_*` accessors that empty them again                        |     3,316 lines |
| a synchronous `finish` loop that performs the effects           |     2,803 lines |
| `Drop` code releasing whichever fields are still set            |      ~350 lines |
| a `TryFrom<*Step> for vm::…::Step` conversion for the VM driver |     5,307 lines |
| one arm per reply kind in the VM `Resume` dispatch              |  ~100 variants |

The same arguments therefore cross three hand-written layers (pending struct,
builtin step, VM step), even though the VM step enum already is a shared effect
type. The pending struct also sizes every in-flight operation for the union of
all its effects' arguments (8 `Option` fields on average) although only one
effect is ever pending.

## Design

One shared vocabulary in `engine::builtins::effect`:

```rust
pub(crate) enum Effect {
    ReadValue { receiver: JsValue, key: PropertyKey },
    Number { value: JsValue },
    Call { callable: CallableRef, receiver: JsValue, arguments: Vec<JsValue> },
    Construct { target: ConstructorRef, arguments: Vec<JsValue> },
    Define { object: ObjectRef, key: PropertyKey, descriptor: OwnedPropertyDescriptor },
    Set { object: ObjectRef, key: PropertyKey, value: JsValue },
    ParseIterator { result: Completion },
    // … grows to the ~20 kinds the builtins use (Has, Delete, Primitive, …)
}

pub(crate) enum Reply {
    Completion(Completion),
    Number(NativeConversion<f64>),
    Defined(NativeConversion<InternalDefineResult>),
    Set(NativeConversion<InternalSetResult>),
    Parsed(ObjectIteratorStep),
    // … one per effect result type
}

pub(crate) trait Continuation {
    /// Where the next effect waits until a driver takes it.
    fn pending(&mut self) -> &mut Option<Effect>;
    fn reply(self: Box<Self>, runtime: &Runtime, reply: Reply) -> Result<EffectStep, RuntimeError>;
}

pub(crate) enum EffectStep {
    Complete(Completion),
    Effect(Box<dyn Continuation>), // effect parked in `pending()`
    Close { iterator: ObjectRef, completion: Completion }, // terminal
}
```

- A builtin's resume state implements `Continuation`: one `match` from `Reply`
  to the handler it already has (`resume`, `number`, `set`, …). It returns
  `effect::request(Effect::ReadValue { .. }, self)` where it used to call
  `request_read`. The effect is parked inside the boxed state rather than
  carried next to it, so a step stays within the 56-byte `NativeStep` budget.
  Its `*Step` enum, pending struct, `request_*`, `take_*` and pending-field
  `Drop` code go away.
- `effect::finish(runtime, realm, step)` is the one synchronous driver: it
  performs each `Effect` exactly as the per-builtin loops do today and feeds the
  `Reply` back.
- The VM converts `EffectStep` into its own `Step` once (one `From` impl, not
  one per builtin), with `Resume::Native(Box<dyn Continuation>)`; each VM reply
  method gains a single `Resume::Native` arm.
- A pending effect is moved out of the step before it is performed. Values not
  yet consumed are owned by the `Effect`, whose `Drop`-time release is written
  once.

## Costs and risks

- `Box<dyn Continuation>` is 16 bytes instead of an 8-byte `Box` plus an enum
  tag; the steps are cold paths (property traps, iterator protocol, species).
  Pilot measurements decide whether dynamic dispatch shows up.
- Effect order and ownership must stay identical: each migration keeps the
  builtin's handlers and only replaces the transport. Test262 (focused) and the
  QuickJS oracle suite run after every batch.
- Builtins that need effect kinds with unusual replies migrate last; until then
  old and new transports coexist (the VM keeps its per-builtin arms for
  unmigrated builtins).

## Pilot results (`Array.from`/`Array.of`)

- `build.rs` shrank from 1,024 to 652 lines; its VM `TryFrom` (105 lines) and
  its VM reply arms are gone. The shared `effect.rs` (~200 lines) and the VM
  adapter (~90 lines) are a one-time cost, so the pilot is line-neutral and
  every later builtin only removes code.
- The boxed resume state of one in-flight `Array.from` went from 632 to 312
  bytes: one 120-byte `Option<Effect>` replaces a 408-byte pending struct.
- VM `Resume` stays within its 32-byte budget; a Set waiting in the VM boxes
  its key and continuation (`NativeSetWait`), as `DefineTyped` already does.
- Focused Test262 passes 6,844/6,844 and the workspace tests pass unchanged.
- A loop of 16,000 `Array.from`/`Array.of` calls retires 0.45% more
  instructions (17.90G → 17.98G, M1, median of 5): effects are 120-byte values
  moved through the continuation, plus one dynamic call per reply. Boxing the
  80-byte descriptor of `Effect::Define` would roughly halve the move size if
  this matters once more builtins migrate.

## Rollout

1. Pilot `array/build.rs`; measure lines, step sizes and the fixed probes.
2. Migrate by family (array, iterator, object/reflect, regexp, typed array,
   conversions), one PR each, deleting the VM arms and `TryFrom` impls as their
   builtins move.
3. Remove the per-builtin `Resume` variants and the old pending infrastructure.

Expected result: roughly 8,000–10,000 fewer lines and a single place that defines
how an effect is performed and released.
