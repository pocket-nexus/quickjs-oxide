//! The register machine (new-core phase 2, section 4): the register file and
//! frame table, the main loop, its slow paths, exception unwinding and the
//! leaf-native ABI.
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
pub(crate) mod property;
pub(crate) mod proxy;
pub(crate) mod suspend;
pub(crate) mod typed;
pub(crate) mod waiter;
// Not called yet: `Number.prototype.toString` and the parsers land with the
// phase 3 builtins. Public for the root crate's differential test.
#[doc(hidden)]
pub mod radix;
pub mod regexp;

use std::fmt;

use crate::bytecode::{CodeStore, CodeUnitId};
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
    /// The suspendable activations running, innermost last (phase 4,
    /// section 4.3): where `Yield` and `Await` save their frame.
    pub(crate) active: Vec<ObjectId>,
}

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
            active: Vec::new(),
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
        } else if self.native_base.abs_diff(here) > NATIVE_STACK_BUDGET {
            return false;
        }
        self.nesting += 1;
        true
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
    /// The embedder's hooks (phase 4, section 6.3).
    pub(crate) hooks: Box<dyn crate::host::Host>,
    /// `Math.random`'s xorshift64* state; never zero.
    random_state: u64,
    /// `Symbol.for`'s registry: content to symbol. Registered symbols are
    /// fixed roots.
    pub(crate) symbol_registry: crate::heap::hash::FxHashMap<Vec<u16>, crate::SymbolId>,
    /// Whether `Atomics.wait` may block this agent.
    pub can_block: bool,
    /// The embedder's compiler (section 6.3).
    pub(crate) compile: Option<compile::CompileFn>,
    /// The embedder's module loader (section 6.3).
    pub(crate) module_host: Option<module::ModuleHost>,
    /// The module records by normalized name.
    pub(crate) modules: crate::heap::hash::FxHashMap<String, ObjectId>,
    /// The embedder's script runner, for `$262.agent.start`.
    pub(crate) script_runner: Option<crate::builtins::agent::ScriptRunner>,
    /// The agents this (main) agent started.
    pub(crate) agent_hub: Option<std::sync::Arc<crate::builtins::agent::AgentHub>>,
    /// This runtime as an agent of another.
    pub(crate) agent: Option<crate::builtins::agent::AgentSelf>,
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
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |elapsed| elapsed.as_nanos() as u64);
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
/// stays borrowed across every heap call.
///
/// [`Runtime`]: crate::Runtime
pub(crate) struct Cx<'a> {
    pub heap: &'a mut Heap,
    pub code: &'a CodeStore,
    pub realm: &'a Realm,
    pub host: &'a mut Services,
    pub regexps: &'a mut regexp::RegExpStore,
}

impl Cx<'_> {
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
