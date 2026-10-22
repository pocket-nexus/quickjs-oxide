//! The register machine: the register file and frame table, the main loop, its
//! slow paths, exception unwinding and the leaf-native ABI.
//!
//! A frame owns a window of the register file: register 0 is `this`, then
//! the parameters, the locals and the operand temporaries of its code unit. A
//! call's window starts inside the caller's, on the callee's operands, so no
//! argument is copied. Both tables live inside the [`Heap`] as fixed roots:
//! a safepoint marks `registers[..top]` and every frame's function.

pub(crate) mod arguments;
pub(crate) mod async_generator;
pub(crate) mod bigint;
pub(crate) mod class;
pub mod compile;
pub(crate) mod error;
pub mod ic;
mod interp;
pub(crate) mod iter;
pub(crate) mod jobs;
pub mod module;
pub mod native;
pub(crate) mod number;
pub(crate) mod object;
pub(crate) mod ops;
pub(crate) mod print;
pub(crate) mod property;
pub(crate) mod proxy;
pub(crate) mod suspend;
pub(crate) mod typed;
pub(crate) mod via;
pub(crate) mod waiter;
// Public for the root crate's differential test.
#[doc(hidden)]
pub mod radix;
pub mod regexp;

use std::fmt;

use crate::bytecode::{CodeStore, CodeUnitId, UnitRef};
use crate::heap::Heap;
use crate::runtime::Realm;
use crate::value::{BytecodeId, ObjectId, Value};

pub use self::error::ErrorKind;

/// A thrown JavaScript value. It is not rooted: whoever receives it stores it
/// in a register, a root or a handle before the next allocation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Throw(pub Value);

impl fmt::Debug for Throw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Throw({:?})", self.0)
    }
}

pub mod frame_flags {
    /// The frame was entered from Rust: its return leaves the loop.
    pub const ENTRY: u8 = 1 << 0;
    /// A `new` call: a non-object return yields the receiver in register 0.
    pub const CONSTRUCT: u8 = 1 << 1;
    /// Entered by a tail call: its result is also its caller's result.
    pub const TAIL: u8 = 1 << 2;
    /// Called through `Function.prototype.call` (the loop's rewrite): QuickJS
    /// lists `at call (native)` after its backtrace line.
    pub const VIA_CALL: u8 = 1 << 3;
    /// Called through `Function.prototype.apply`, likewise.
    pub const VIA_APPLY: u8 = 1 << 4;
    /// Called through a chain of them: `ViaCalls::frames` lists it.
    pub const VIA_CHAIN: u8 = 1 << 5;
}

/// One activation. `return_pc` and `return_dst` belong to the caller: where
/// it resumes, and the absolute register that receives the result.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub unit: CodeUnitId,
    pub function: ObjectId,
    pub bytecode: BytecodeId,
    pub base: u32,
    pub return_pc: u32,
    pub return_dst: u32,
    /// The actual argument count, which `arguments`, rest parameters and the
    /// extra arguments kept above the registers read.
    pub argc: u32,
    pub flags: u8,
}

const _: () = assert!(size_of::<Frame>() == 32);

/// Registers a frame addresses without a bounds check: an 8-bit operand, or
/// a call's `b + 1` (see `interp::window`).
pub(crate) const SPAN: usize = 512;

/// Native stack bytes nested re-entries may use (debug builds have large
/// loop frames; the default thread stack of a test is 2 MiB).
pub const NATIVE_STACK_BUDGET: usize = 1 << 20;

/// Registers the file may hold before a call fails with `InternalError:
/// stack overflow`: 8 MiB of values.
pub const DEFAULT_STACK_LIMIT: u32 = 1 << 20;

/// The `pc` of a [`NativeSite`] made by a call from Rust: the caller's
/// instruction is not known there.
pub(crate) const NO_PC: u32 = u32::MAX;

/// A native call made by the loop, or from Rust (`pc` [`NO_PC`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeSite {
    /// The frame count when it was made: the caller is `frames[depth - 1]`.
    pub depth: u32,
    pub pc: u32,
    pub callee: Value,
}

/// The register file and the frame table.
pub struct VmStack {
    pub(crate) registers: Vec<Value>,
    pub(crate) frames: Vec<Frame>,
    /// `registers[..top]` are roots: the running frame's window and
    /// everything below it.
    pub(crate) top: u32,
    /// Registers from here up may name records freed by a collection that
    /// ran while they were above `top`: each collection lowers it to `top`,
    /// and a frame that resumes clears its registers from here before they
    /// become roots again (see `interp::raise_top`).
    pub(crate) stale_from: u32,
    pub(crate) limit: u32,
    /// Rust-level re-entries in progress: a slow path calling back into
    /// JavaScript, or a conversion recursing through an array.
    pub(crate) nesting: u32,
    /// The native stack address at the outermost re-entry, for
    /// [`VmStack::enter_native`].
    pub(crate) native_base: usize,
    /// The native stack bytes nested re-entries may use
    /// ([`NATIVE_STACK_BUDGET`] unless the runtime's options say otherwise).
    pub(crate) native_budget: usize,
    /// Nested re-entries allowed at most: the budget for a host whose stack
    /// the byte measure cannot see (a WebAssembly engine's machine stack).
    pub(crate) native_depth: u32,
    /// The frame count and instruction index of the call running the
    /// current native, for the backtrace of an error it creates.
    /// The native calls in progress, innermost last: the frame count and
    /// instruction index of each call and the native it runs, for the
    /// backtrace's `at name (native)` lines and the positions of the frames
    /// that called them.
    pub(crate) natives: Vec<NativeSite>,
    /// The next native called from Rust is listed in no backtrace: an
    /// iterator's `next` stepped by [`call_next`].
    pub(crate) frameless_native: bool,
    /// The last error whose backtrace has frames it could not place (the
    /// innermost frame when the loop itself threw; a frame that called into
    /// Rust, which re-entered the loop): per frame, its depth and its line
    /// of the backtrace. Each loop the throw unwinds through places its own.
    pub(crate) unpositioned: Option<(ObjectId, Vec<(u32, u32)>)>,
    /// The suspendable activations running, innermost last: where `Yield` and
    /// `Await` save their frame.
    pub(crate) active: Vec<ObjectId>,
    /// Whether the last proxy `[[Set]]` or `[[DefineOwnProperty]]` that
    /// returned false was refused by a trap rather than by the target it
    /// forwarded to: QuickJS throws `proxy: cannot set property` or `proxy:
    /// defineProperty exception` only for the former.
    pub(crate) proxy_refused: bool,
    /// The bookkeeping of rewritten `call`s and `apply`s, boxed to keep the
    /// loop's hot fields where they were.
    pub(crate) via: Box<via::ViaCalls>,
    /// Back-edges and calls left before the next interrupt poll.
    pub(crate) poll: u32,
    /// An interrupt is unwinding: no handler catches it, and no native turns
    /// it into a completion.
    pub(crate) interrupted: bool,
}

/// Back-edges and calls between two interrupt polls. QuickJS's
/// `JS_INTERRUPT_COUNTER_INIT` is 10,000; a tenth of it costs the loop
/// nothing measurable (the poll is cold), and stops a script ten times
/// sooner.
pub const INTERRUPT_INTERVAL: u32 = 1_000;

impl Default for VmStack {
    fn default() -> Self {
        VmStack {
            registers: Vec::new(),
            frames: Vec::new(),
            top: 0,
            stale_from: u32::MAX,
            limit: DEFAULT_STACK_LIMIT,
            nesting: 0,
            native_base: 0,
            native_budget: NATIVE_STACK_BUDGET,
            native_depth: u32::MAX,
            natives: Vec::new(),
            frameless_native: false,
            via: Box::default(),
            unpositioned: None,
            active: Vec::new(),
            proxy_refused: false,
            poll: INTERRUPT_INTERVAL,
            interrupted: false,
        }
    }
}

impl VmStack {
    /// Makes `registers[..end]` addressable, plus [`SPAN`] values of
    /// padding so a window can be taken at any base below `end`; `false`
    /// when `end` passes the limit. Growth doubles, and indices stay valid,
    /// so no window needs fixing up.
    #[inline]
    pub(crate) fn reserve(&mut self, end: usize) -> bool {
        if end + SPAN <= self.registers.len() {
            return true;
        }
        self.grow(end)
    }

    #[inline(never)]
    fn grow(&mut self, end: usize) -> bool {
        if end > self.limit as usize {
            return false;
        }
        let target = (end + SPAN).max(self.registers.len() * 2).max(4 * SPAN);
        let target = target.min(self.limit as usize + SPAN);
        self.registers.resize(target, Value::UNDEFINED);
        true
    }

    /// Counts one Rust-level re-entry, or fails when the native stack used
    /// since the outermost one exceeds [`NATIVE_STACK_BUDGET`] (QuickJS's
    /// `js_check_stack_overflow`). Pair with [`VmStack::leave_native`].
    #[inline]
    pub(crate) fn enter_native(&mut self) -> bool {
        let marker = 0u8;
        let here = std::ptr::addr_of!(marker) as usize;
        if self.nesting == 0 {
            self.native_base = here;
        } else if self.native_base.abs_diff(here) > self.native_budget
            || self.nesting >= self.native_depth
        {
            return false;
        }
        self.nesting += 1;
        true
    }

    /// The native stack bytes nested re-entries may still use.
    pub(crate) fn native_remaining(&self) -> usize {
        if self.nesting == 0 {
            return self.native_budget;
        }
        let marker = 0u8;
        let here = std::ptr::addr_of!(marker) as usize;
        self.native_budget
            .saturating_sub(self.native_base.abs_diff(here))
    }

    #[inline]
    pub(crate) fn leave_native(&mut self) {
        self.nesting -= 1;
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn registers_reserved(&self) -> usize {
        self.registers.len()
    }
}

/// Where `print` writes.
#[derive(Debug, Default)]
pub enum Output {
    #[default]
    Stdout,
    /// Collected for the embedder (tests).
    Capture(String),
}

/// Runtime state the natives reach beside the heap: output, the random
/// stream, the symbol registry, and the embedder's [`Host`](crate::host::Host).
pub struct Services {
    pub output: Output,
    /// The embedder's hooks.
    pub(crate) hooks: Box<dyn crate::host::Host>,
    /// `Math.random`'s xorshift64* state; never zero.
    random_state: u64,
    /// `Symbol.for`'s registry: content to symbol. Registered symbols are
    /// fixed roots.
    pub(crate) symbol_registry: crate::heap::hash::FxHashMap<Vec<u16>, crate::SymbolId>,
    /// Whether `Atomics.wait` may block this agent.
    pub can_block: bool,
    /// The embedder's compiler.
    pub(crate) compile: Option<compile::CompileFn>,
    /// The embedder's module loader.
    pub(crate) module_host: Option<module::ModuleHost>,
    /// The module records by normalized name.
    pub(crate) modules: crate::heap::hash::FxHashMap<String, ObjectId>,
    /// The embedder's script runner, for `$262.agent.start`.
    pub(crate) script_runner: Option<crate::builtins::agent::ScriptRunner>,
    /// The agents this (main) agent started.
    pub(crate) agent_hub: Option<std::sync::Arc<crate::builtins::agent::AgentHub>>,
    /// This runtime as an agent of another.
    pub(crate) agent: Option<crate::builtins::agent::AgentSelf>,
    /// Whether unhandled rejected promises are kept in
    /// `Heap::rejections` for the embedder.
    pub(crate) track_rejections: bool,
    /// Set from any thread to interrupt the running script at the next
    /// poll; it stays set until cleared.
    pub(crate) interrupt: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Services")
            .field("output", &self.output)
            .finish()
    }
}

impl Default for Services {
    fn default() -> Self {
        // QuickJS seeds from the clock; programs that compare output replace
        // `Math.random`, as the benchmark harness does.
        let seed = crate::clock::time_seed();
        Services {
            output: Output::default(),
            hooks: Box::new(crate::host::SystemHost),
            random_state: seed.max(1),
            symbol_registry: Default::default(),
            can_block: true,
            compile: None,
            module_host: None,
            modules: Default::default(),
            script_runner: None,
            agent_hub: None,
            agent: None,
            track_rejections: false,
            interrupt: Default::default(),
        }
    }
}

impl Services {
    pub(crate) fn random_state(&self) -> u64 {
        self.random_state
    }

    pub(crate) fn set_random_state(&mut self, state: u64) {
        self.random_state = state;
    }

    /// Seeds `Math.random` (zero is replaced by one).
    pub fn seed_random(&mut self, seed: u64) {
        self.random_state = seed.max(1);
    }
}

/// What the loop and its slow paths borrow from the [`Runtime`]: the heap
/// mutably, the code store and the realm shared, so the running unit's code
/// stays borrowed across every heap call ([`Cx::unit`]).
///
/// [`Runtime`]: crate::Runtime
pub(crate) struct Cx<'a> {
    pub heap: &'a mut Heap,
    /// The static units only: [`Cx::unit`] reads any unit.
    pub static_code: &'a CodeStore,
    pub realm: &'a Realm,
    pub host: &'a mut Services,
}

impl<'a> Cx<'a> {
    /// Unit `id`: borrowed from the code store, or counted from the heap,
    /// so it stays valid across every heap call either way.
    #[inline(always)]
    pub(crate) fn unit(&self, id: CodeUnitId) -> UnitRef<'a> {
        if id.is_dynamic() {
            return dynamic_unit(self.heap, id);
        }
        let code: &'a CodeStore = self.static_code;
        UnitRef::Static(code.get(id))
    }
}

#[cold]
#[inline(never)]
fn dynamic_unit(heap: &Heap, id: CodeUnitId) -> UnitRef<'static> {
    UnitRef::Dynamic(heap.code.get(id))
}

impl Cx<'_> {
    /// Every published unit, for readers that hold no unit across a heap
    /// call.
    pub(crate) fn units(&self) -> crate::bytecode::Units<'_> {
        crate::bytecode::Units {
            code: self.static_code,
            dynamic: &self.heap.code,
        }
    }

    /// Records an intrinsic the builtins created, as a fixed root.
    pub(crate) fn set_intrinsic(&mut self, intrinsic: crate::Intrinsic, object: ObjectId) {
        self.heap.add_fixed_root(Value::object(object));
        self.realm.record_intrinsic(intrinsic, object);
    }
}

/// Calls `function` with `this` and `args` and runs it to completion. The
/// arguments need no rooting: they are stored in the register file before
/// anything allocates. Re-entrant: a slow path may call back into
/// JavaScript, which runs a nested loop on the frames above its own.
pub(crate) fn call(
    cx: &mut Cx<'_>,
    function: Value,
    this: Value,
    args: &[Value],
) -> Result<Value, Throw> {
    let base = cx.heap.vm.top as usize;
    let end = base + 1 + args.len();
    if !cx.heap.vm.reserve(end) {
        return Err(error::stack_overflow(cx));
    }
    let saved_top = cx.heap.vm.top;
    let registers = &mut cx.heap.vm.registers;
    registers[base] = this;
    registers[base + 1..end].copy_from_slice(args);
    // The arguments are rooted from here; `function` is held by the caller
    // (a register or a property of a rooted object) until the frame names it.
    cx.heap.vm.top = end as u32;
    let result = interp::call_from_rust(cx, function, base, args.len());
    cx.heap.vm.top = saved_top;
    result
}

/// [`call`] of an iterator's `next` with no arguments, as QuickJS's
/// `JS_IteratorNext2` makes it: a native `next` runs without a stack frame
/// of its own (QuickJS calls its `iterator_next` C functions directly).
pub(crate) fn call_next(cx: &mut Cx<'_>, next: Value, iterator: Value) -> Result<Value, Throw> {
    cx.heap.vm.frameless_native = true;
    let result = call(cx, next, iterator, &[]);
    cx.heap.vm.frameless_native = false;
    result
}

/// `Construct(function, args, new_target)`: runs a constructor to
/// completion. Re-entrant like [`call`]; the arguments need no rooting.
pub(crate) fn construct(
    cx: &mut Cx<'_>,
    function: Value,
    args: &[Value],
    new_target: Value,
) -> Result<Value, Throw> {
    let base = cx.heap.vm.top as usize;
    let end = base + 1 + args.len();
    if !cx.heap.vm.reserve(end) {
        return Err(error::stack_overflow(cx));
    }
    let saved_top = cx.heap.vm.top;
    let registers = &mut cx.heap.vm.registers;
    registers[base] = Value::UNDEFINED;
    registers[base + 1..end].copy_from_slice(args);
    cx.heap.vm.top = end as u32;
    // `new_target` is held by the caller, like `function`.
    let result = interp::construct_from_rust(cx, function, base, args.len(), new_target);
    cx.heap.vm.top = saved_top;
    result
}
