//! The `Runtime`: the heap, the code store and the realm. Every operation is
//! `fn(&mut Runtime, …)`; there is no shared interior mutability.

use crate::PropertyKey;
use crate::bytecode::{self, CodeStore, CodeUnit, CodeUnitId, ValidateError};
use crate::heap::atom::KeyKind;
use crate::heap::gc::{Handle, HeapLimits};
use crate::heap::object::ObjectKind;
use crate::heap::{Heap, HeapError};
use crate::value::{ObjectId, ScopeId, StringId, SymbolId, Value};
use crate::vm::regexp::RegExpStore;
use crate::vm::{Cx, ErrorKind, Output, Services, Throw, VmStack};

macro_rules! intrinsics {
    ($($(#[$doc:meta])* $name:ident,)*) => {
        /// Intrinsic objects held by the realm: the prototypes and the
        /// constructors the natives and the loop name. Each is a fixed root.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Intrinsic {
            $($(#[$doc])* $name,)*
        }

        impl Intrinsic {
            pub const ALL: &'static [Intrinsic] = &[$(Intrinsic::$name,)*];
            pub const COUNT: usize = Intrinsic::ALL.len();
        }
    };
}

intrinsics! {
    ObjectPrototype,
    FunctionPrototype,
    ArrayPrototype,
    StringPrototype,
    RegExpPrototype,
    NumberPrototype,
    BooleanPrototype,
    SymbolPrototype,
    BigIntPrototype,
    DatePrototype,
    ErrorPrototype,
    EvalErrorPrototype,
    RangeErrorPrototype,
    ReferenceErrorPrototype,
    SyntaxErrorPrototype,
    TypeErrorPrototype,
    InternalErrorPrototype,
    UriErrorPrototype,
    AggregateErrorPrototype,
    /// A preallocated `InternalError: out of memory`, thrown when the heap
    /// cannot allocate the error itself.
    OutOfMemoryError,
    /// `%ThrowTypeError%`.
    ThrowTypeError,
    Object,
    Function,
    Array,
    String,
    Number,
    Boolean,
    Symbol,
    BigInt,
    RegExp,
    Date,
    Promise,
    Proxy,
    Eval,
    IteratorPrototype,
    AsyncIteratorPrototype,
    ArrayIteratorPrototype,
    StringIteratorPrototype,
    MapIteratorPrototype,
    SetIteratorPrototype,
    RegExpStringIteratorPrototype,
    IteratorHelperPrototype,
    Iterator,
    WrapForValidIteratorPrototype,
    AsyncFromSyncIteratorPrototype,
    /// `%GeneratorFunction.prototype%`.
    GeneratorFunctionPrototype,
    /// `%GeneratorFunction.prototype.prototype%`.
    GeneratorPrototype,
    AsyncFunctionPrototype,
    AsyncGeneratorFunctionPrototype,
    AsyncGeneratorPrototype,
    GeneratorFunction,
    AsyncFunction,
    AsyncGeneratorFunction,
    PromisePrototype,
    MapPrototype,
    SetPrototype,
    WeakMapPrototype,
    WeakSetPrototype,
    WeakRefPrototype,
    FinalizationRegistryPrototype,
    ArrayBufferPrototype,
    SharedArrayBufferPrototype,
    DataViewPrototype,
    TypedArray,
    TypedArrayPrototype,
    Int8ArrayPrototype,
    Uint8ArrayPrototype,
    Uint8ClampedArrayPrototype,
    Int16ArrayPrototype,
    Uint16ArrayPrototype,
    Int32ArrayPrototype,
    Uint32ArrayPrototype,
    Float16ArrayPrototype,
    Float32ArrayPrototype,
    Float64ArrayPrototype,
    BigInt64ArrayPrototype,
    BigUint64ArrayPrototype,
    Int8Array,
    Uint8Array,
    Uint8ClampedArray,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Float16Array,
    Float32Array,
    Float64Array,
    BigInt64Array,
    BigUint64Array,
    ArrayBuffer,
    /// `Array.prototype.values`, which is also `arguments[@@iterator]`.
    ArrayValues,
    /// The global lexical environment's dictionary (section 5.7).
    GlobalLexical,
}

macro_rules! names {
    ($($field:ident: $text:expr,)*) => {
        /// Atoms the loop and the natives name: property names they read
        /// themselves and the `typeof` results. Atoms are strong roots, so
        /// they stay valid.
        #[derive(Clone, Copy, Debug)]
        pub(crate) struct Names {
            $(pub $field: StringId,)*
        }

        impl Names {
            fn new(heap: &mut Heap) -> Result<Names, HeapError> {
                Ok(Names {
                    $($field: heap.intern($text)?,)*
                })
            }
        }
    };
}

names! {
    length: "length",
    callee: "callee",
    caller: "caller",
    arguments: "arguments",
    last_index: "lastIndex",
    index: "index",
    indices: "indices",
    input: "input",
    groups: "groups",
    prototype: "prototype",
    constructor: "constructor",
    message: "message",
    name: "name",
    cause: "cause",
    stack: "stack",
    file_name: "fileName",
    line_number: "lineNumber",
    column_number: "columnNumber",
    errors: "errors",
    value_of: "valueOf",
    to_string: "toString",
    to_json: "toJSON",
    undefined: "undefined",
    null: "null",
    true_: "true",
    false_: "false",
    object: "object",
    boolean: "boolean",
    number: "number",
    string: "string",
    symbol: "symbol",
    function: "function",
    bigint: "bigint",
    empty: "",
    value: "value",
    writable: "writable",
    enumerable: "enumerable",
    configurable: "configurable",
    get: "get",
    set: "set",
    next: "next",
    done: "done",
    return_: "return",
    throw: "throw",
    then: "then",
    proto: "__proto__",
    raw: "raw",
    join: "join",
    default: "default",
    flags: "flags",
    source: "source",
    exec: "exec",
    size: "size",
    iterator_tag: "Iterator",
    resolve: "resolve",
    reject: "reject",
    promise: "promise",
    status: "status",
    reason: "reason",
    fulfilled: "fulfilled",
    rejected: "rejected",
}

impl Names {
    #[inline]
    pub fn key(id: StringId) -> PropertyKey {
        PropertyKey::atom(id)
    }
}

macro_rules! well_known {
    ($($field:ident: $text:expr,)*) => {
        /// The well-known symbols (`Symbol.iterator`, …), fixed roots.
        #[derive(Clone, Copy, Debug)]
        pub(crate) struct WellKnown {
            $(pub $field: SymbolId,)*
        }

        impl WellKnown {
            fn new(heap: &mut Heap) -> Result<WellKnown, HeapError> {
                Ok(WellKnown {
                    $($field: {
                        let description = heap.intern($text)?;
                        let symbol = heap.alloc_symbol(Some(description), false)?;
                        heap.add_fixed_root(Value::symbol(symbol));
                        symbol
                    },)*
                })
            }

            /// Every well-known symbol with its property name on `Symbol`.
            pub fn all(&self) -> Vec<(&'static str, SymbolId)> {
                vec![$((&$text["Symbol.".len()..], self.$field),)*]
            }
        }
    };
}

well_known! {
    async_iterator: "Symbol.asyncIterator",
    has_instance: "Symbol.hasInstance",
    is_concat_spreadable: "Symbol.isConcatSpreadable",
    iterator: "Symbol.iterator",
    match_: "Symbol.match",
    match_all: "Symbol.matchAll",
    replace: "Symbol.replace",
    search: "Symbol.search",
    species: "Symbol.species",
    split: "Symbol.split",
    to_primitive: "Symbol.toPrimitive",
    to_string_tag: "Symbol.toStringTag",
    unscopables: "Symbol.unscopables",
}

impl WellKnown {
    #[inline]
    pub fn key(id: SymbolId) -> PropertyKey {
        PropertyKey::symbol(id)
    }
}

/// The single realm of a runtime (phase 0, section 2). Its objects are fixed
/// roots of the heap.
pub struct Realm {
    global: ObjectId,
    /// Cells so the builtins can record the intrinsics they create while the
    /// realm is shared (installation runs with a `Cx`); each is a fixed root.
    intrinsics: [std::cell::Cell<ObjectId>; Intrinsic::COUNT],
    pub(crate) names: Names,
    pub(crate) symbols: WellKnown,
    /// The private key under which a home object keeps its class's brand
    /// (section 5.1).
    pub(crate) brand: SymbolId,
    /// `%RegExp.prototype%`'s flag accessors as installed, for the `flags`
    /// getter's fast path.
    pub(crate) regexp_flags: std::cell::Cell<Option<crate::builtins::RegExpFlagCache>>,
    /// The shape of a fresh `%RegExp%` instance (only `lastIndex`) and the
    /// slot of `lastIndex`.
    pub(crate) regexp_instance_shape: std::cell::Cell<Option<(crate::value::ShapeId, u32)>>,
}

impl Realm {
    #[inline]
    pub fn global(&self) -> ObjectId {
        self.global
    }

    #[inline]
    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.intrinsics[intrinsic as usize].get()
    }

    /// Records an intrinsic; the caller makes it a fixed root.
    pub(crate) fn record_intrinsic(&self, intrinsic: Intrinsic, object: ObjectId) {
        self.intrinsics[intrinsic as usize].set(object);
    }
}

pub struct Runtime {
    pub(crate) heap: Heap,
    /// Beside the heap, not in it: the main loop borrows the running unit's
    /// code across heap calls (phase 2, section 2).
    pub(crate) code: CodeStore,
    pub(crate) realm: Realm,
    pub(crate) host: Services,
    /// The regular-expression programs (phase 3, section 5.3): append-only,
    /// beside the heap like the code store.
    pub(crate) regexps: RegExpStore,
}

/// How a runtime is configured (phase 6, section 2.4). `Default` is the
/// behavior every runtime had before the options existed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeOptions {
    /// The heap cap; `usize::MAX` bytes for none.
    pub heap: HeapLimits,
    /// The collection threshold's floor, in bytes.
    pub min_heap: usize,
    /// The threshold after a collection is the live bytes times this.
    pub growth: f64,
    /// Registers the register file may hold before a call fails with
    /// `InternalError: stack overflow`.
    pub stack_limit: u32,
    /// Bytes of host stack nested re-entries into the loop may use.
    pub native_stack_budget: usize,
    /// Nested re-entries into the loop allowed at most (unbounded by
    /// default): for a host whose stack the byte measure cannot see, such as
    /// a WebAssembly engine's.
    pub native_depth_limit: u32,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        RuntimeOptions {
            heap: HeapLimits::default(),
            min_heap: crate::heap::gc::DEFAULT_MIN_HEAP,
            growth: 2.0,
            stack_limit: crate::vm::DEFAULT_STACK_LIMIT,
            native_stack_budget: crate::vm::NATIVE_STACK_BUDGET,
            native_depth_limit: u32::MAX,
        }
    }
}

impl From<HeapLimits> for RuntimeOptions {
    fn from(heap: HeapLimits) -> Self {
        RuntimeOptions {
            heap,
            ..RuntimeOptions::default()
        }
    }
}

/// [`Runtime::thrown_summary`]'s reading of a thrown object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThrownSummary {
    pub name: Option<String>,
    pub message: Option<String>,
    pub stack: Option<String>,
}

impl Runtime {
    pub fn new(options: impl Into<RuntimeOptions>) -> Result<Runtime, HeapError> {
        let options = options.into();
        let mut heap = Heap::new(options.heap);
        heap.gc.min_heap = options.min_heap;
        heap.gc.threshold = options.min_heap;
        heap.gc.growth_factor = options.growth;
        heap.vm.limit = options.stack_limit;
        heap.vm.native_budget = options.native_stack_budget;
        heap.vm.native_depth = options.native_depth_limit;
        let root = |heap: &mut Heap, object: ObjectId| {
            heap.add_fixed_root(Value::object(object));
            object
        };
        let object_prototype = heap.alloc_object(ObjectKind::Ordinary, None, 4)?;
        let object_prototype = root(&mut heap, object_prototype);
        // `%Function.prototype%` is itself a function that returns
        // `undefined`.
        let function_prototype = heap.alloc_native_function(
            Some(object_prototype),
            crate::builtins::Native::FunctionPrototype.index(),
        )?;
        let function_prototype = root(&mut heap, function_prototype);
        // `%Array.prototype%` is itself an array (of length 0).
        let array_prototype = heap.alloc_array(Some(object_prototype), 0)?;
        let array_prototype = root(&mut heap, array_prototype);
        let string_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let string_prototype = root(&mut heap, string_prototype);
        let regexp_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let regexp_prototype = root(&mut heap, regexp_prototype);
        // `%Number.prototype%` and `%Boolean.prototype%` are wrappers of 0
        // and `false`; `%String.prototype%` stays ordinary (its exotic
        // indices would be empty), so the string-receiver cache keeps it.
        let number_prototype = heap.alloc_internal(
            ObjectKind::NumberWrapper,
            crate::heap::object::InternalTag::None,
            Some(object_prototype),
            4,
            &[Value::int32(0)],
            Default::default(),
            0,
        )?;
        let number_prototype = root(&mut heap, number_prototype);
        let boolean_prototype = heap.alloc_internal(
            ObjectKind::BooleanWrapper,
            crate::heap::object::InternalTag::None,
            Some(object_prototype),
            4,
            &[Value::FALSE],
            Default::default(),
            0,
        )?;
        let boolean_prototype = root(&mut heap, boolean_prototype);
        let global = heap.alloc_object(ObjectKind::GlobalObject, Some(object_prototype), 16)?;
        let global = root(&mut heap, global);
        // The global lexical environment (section 5.7): a dictionary like the
        // global object, sharing its generation.
        let lexical = heap.alloc_object(ObjectKind::GlobalObject, None, 16)?;
        let lexical = root(&mut heap, lexical);
        let names = Names::new(&mut heap)?;
        let symbols = WellKnown::new(&mut heap)?;
        let brand = heap.alloc_symbol(None, true)?;
        heap.add_fixed_root(Value::symbol(brand));
        let mut intrinsics = [object_prototype; Intrinsic::COUNT];
        intrinsics[Intrinsic::FunctionPrototype as usize] = function_prototype;
        intrinsics[Intrinsic::ArrayPrototype as usize] = array_prototype;
        intrinsics[Intrinsic::StringPrototype as usize] = string_prototype;
        intrinsics[Intrinsic::RegExpPrototype as usize] = regexp_prototype;
        intrinsics[Intrinsic::NumberPrototype as usize] = number_prototype;
        intrinsics[Intrinsic::BooleanPrototype as usize] = boolean_prototype;
        intrinsics[Intrinsic::GlobalLexical as usize] = lexical;
        let error_prototype = heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        intrinsics[Intrinsic::ErrorPrototype as usize] = root(&mut heap, error_prototype);
        for kind in ErrorKind::ALL {
            if kind != ErrorKind::Error {
                let prototype =
                    heap.alloc_object(ObjectKind::Ordinary, Some(error_prototype), 4)?;
                intrinsics[kind.prototype() as usize] = root(&mut heap, prototype);
            }
        }
        let realm = Realm {
            global,
            intrinsics: intrinsics.map(std::cell::Cell::new),
            names,
            symbols,
            brand,
            regexp_flags: std::cell::Cell::new(None),
            regexp_instance_shape: std::cell::Cell::new(None),
        };
        let mut runtime = Runtime {
            heap,
            code: CodeStore::default(),
            realm,
            host: Services::default(),
            regexps: RegExpStore::default(),
        };
        let out_of_memory = crate::builtins::install(&mut runtime.cx())?;
        runtime.set_intrinsic(Intrinsic::OutOfMemoryError, out_of_memory);
        Ok(runtime)
    }

    /// The loop's view of the runtime.
    #[inline]
    pub(crate) fn cx(&mut self) -> Cx<'_> {
        Cx {
            heap: &mut self.heap,
            code: &self.code,
            realm: &self.realm,
            host: &mut self.host,
            regexps: &mut self.regexps,
        }
    }

    pub(crate) fn set_intrinsic(&mut self, intrinsic: Intrinsic, object: ObjectId) {
        self.heap.add_fixed_root(Value::object(object));
        self.realm.record_intrinsic(intrinsic, object);
    }

    pub fn regexps(&self) -> &RegExpStore {
        &self.regexps
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    pub fn host(&mut self) -> &mut Services {
        &mut self.host
    }

    /// Whether `Atomics.wait` may block (Test262's `CanBlockIsFalse`
    /// clears it).
    pub fn set_can_block(&mut self, can_block: bool) {
        self.host.can_block = can_block;
    }

    /// Installs the embedder's hooks (section 6.3) and seeds `Math.random`
    /// from [`Host::random_seed`](crate::host::Host::random_seed).
    pub fn set_host(&mut self, mut hooks: Box<dyn crate::host::Host>) {
        let seed = hooks.random_seed();
        self.host.seed_random(seed);
        self.host.hooks = hooks;
    }

    /// Keeps every rejected promise without a handler alive until a handler
    /// is attached or [`Runtime::clear_unhandled_rejections`] (QuickJS's
    /// `js_std_promise_rejection_tracker`). Off by default.
    pub fn set_track_rejections(&mut self, track: bool) {
        self.host.track_rejections = track;
        if !track {
            self.heap.rejections.clear();
        }
    }

    /// The tracked unhandled rejected promises, in rejection order.
    pub fn unhandled_rejections(&self) -> &[Value] {
        &self.heap.rejections
    }

    pub fn clear_unhandled_rejections(&mut self) {
        self.heap.rejections.clear();
    }

    /// Sends `print` output to a buffer; [`Runtime::take_output`] returns it.
    pub fn capture_output(&mut self) {
        self.host.output = Output::Capture(String::new());
    }

    /// The output captured since the last call.
    pub fn take_output(&mut self) -> String {
        match &mut self.host.output {
            Output::Capture(text) => std::mem::take(text),
            Output::Stdout => String::new(),
        }
    }

    /// Defines the Test262 host object `$262` on the global object.
    pub fn install_test262_host(&mut self) -> Result<(), HeapError> {
        crate::builtins::test262::install(&mut self.cx())
    }

    /// Defines qjs's `print` and `console.log` on the global object.
    pub fn install_print(&mut self) -> Result<(), HeapError> {
        crate::builtins::install_print(&mut self.cx())
    }

    /// Defines qjs's `--std` globals, `load` and `read`, over the host's
    /// [`read_file`](crate::host::Host::read_file).
    pub fn install_std(&mut self) -> Result<(), HeapError> {
        crate::builtins::install_std(&mut self.cx())
    }

    /// Defines `qjs`'s helpers as QuickJS's `js_std_add_helpers` does:
    /// `console`, `scriptArgs` when given, `print`.
    pub fn install_qjs_helpers(&mut self, script_args: Option<&[String]>) -> Result<(), HeapError> {
        crate::builtins::install_qjs_helpers(&mut self.cx(), script_args)
    }

    /// Defines qjs's `scriptArgs`.
    pub fn install_script_args(&mut self, args: &[String]) -> Result<(), HeapError> {
        crate::builtins::install_script_args(&mut self.cx(), args)
    }

    /// Runs a script function (from [`Runtime::instantiate`]) with the
    /// global object as `this`. The result is unrooted.
    pub fn run_script(&mut self, script: ObjectId) -> Result<Value, Throw> {
        let global = Value::object(self.realm.global);
        self.call(Value::object(script), global, &[])
    }

    /// Runs pending jobs until the queue is empty (section 5.4); the host
    /// calls it at its boundary. A job that throws stops the drain.
    pub fn run_jobs(&mut self) -> Result<(), Throw> {
        crate::vm::jobs::run_jobs(&mut self.cx())
    }

    /// Runs the oldest pending job (then clears the kept objects); `false`
    /// when none was pending. A job that throws reports the throw.
    pub fn run_pending_job(&mut self) -> Result<bool, Throw> {
        crate::vm::jobs::run_one_job(&mut self.cx())
    }

    /// `ClearKeptObjects`: the end of a synchronous execution sequence.
    pub fn clear_kept_objects(&mut self) {
        self.heap.kept.clear();
    }

    /// Whether jobs are pending.
    pub fn has_pending_jobs(&self) -> bool {
        !self.heap.jobs.is_empty()
    }

    /// Calls `function`; the result or the thrown value is unrooted. The
    /// caller keeps `function` alive (a handle or a rooted object).
    pub fn call(&mut self, function: Value, this: Value, args: &[Value]) -> Result<Value, Throw> {
        crate::vm::call(&mut self.cx(), function, this, args)
    }

    /// `ToString(value)` as Rust text, for hosts printing a result or an
    /// uncaught exception. Calls back into JavaScript for objects.
    pub fn display(&mut self, value: Value) -> Result<String, Throw> {
        crate::vm::ops::display(&mut self.cx(), value)
    }

    /// The register file and frame table.
    pub fn vm_stack(&self) -> &VmStack {
        &self.heap.vm
    }

    pub fn set_stack_limit(&mut self, registers: u32) {
        self.heap.vm.limit = registers;
    }

    /// Sets the bytes of host stack nested re-entries into the loop may use
    /// ([`RuntimeOptions::native_stack_budget`]): a host that knows how much
    /// stack the thread has left sets it before an outermost entry.
    pub fn set_native_stack_budget(&mut self, bytes: usize) {
        self.heap.vm.native_budget = bytes;
    }

    #[inline]
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    #[inline]
    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    #[inline]
    pub fn code(&self) -> &CodeStore {
        &self.code
    }

    /// The heap and the code store borrowed together, so code can be read
    /// while the heap is mutated.
    #[inline]
    pub fn heap_and_code(&mut self) -> (&mut Heap, &CodeStore) {
        (&mut self.heap, &self.code)
    }

    /// Validates `unit` and appends it to the code store. Its children must
    /// be published first.
    pub fn publish(&mut self, unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        let id = self.code.publish(unit)?;
        self.heap.ics.reserve(self.code.get(id));
        Ok(id)
    }

    /// A function object for the published unit `root`, with a bytecode
    /// record for it and every unit below it. `constants` holds one table per
    /// unit, in the pre-order of [`CodeStore::tree`]; each must have the
    /// unit's `constant_count` entries. The constants need no rooting.
    pub fn instantiate(
        &mut self,
        root: CodeUnitId,
        constants: &[&[Value]],
    ) -> Result<ObjectId, HeapError> {
        self.compiler().instantiate(root, constants)
    }

    /// The heap, the code store and the realm, for a compilation before the
    /// program runs (section 5.5).
    pub fn compiler(&mut self) -> crate::vm::compile::Compiler<'_> {
        crate::vm::compile::Compiler::owned(&mut self.heap, &mut self.code, &self.realm)
    }

    /// Installs the embedder's compiler, for `eval`, the `Function`
    /// constructors and `$262.evalScript` (section 6.3).
    pub fn set_compiler(&mut self, compile: crate::vm::compile::CompileFn) {
        self.host.compile = Some(compile);
    }

    /// The inline-cache report of section 2.6, when the counters exist (the
    /// `ic-stats` feature or a debug build): hits and misses per cache kind,
    /// the site states, and the `worst` sites with the most misses (unit
    /// name, instruction index, key).
    pub fn ic_report(&self, worst: usize) -> Option<String> {
        use std::fmt::Write;

        use crate::vm::ic::IcKind;
        let ics = self.heap.ics();
        let stats = ics.stats()?;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<10}{:>14}{:>14}{:>14}{:>14}{:>9}",
            "ic", "mono hits", "poly hits", "fills", "unfilled", "hit %"
        );
        let kinds = [IcKind::Get, IcKind::Set, IcKind::Global, IcKind::Construct];
        for kind in kinds {
            let k = kind as usize;
            let hits = stats.mono_hits[k] + stats.poly_hits[k];
            let total = hits + stats.fills[k] + stats.unfilled[k];
            let rate = if total == 0 {
                0.0
            } else {
                100.0 * hits as f64 / total as f64
            };
            let _ = writeln!(
                out,
                "{:<10}{:>14}{:>14}{:>14}{:>14}{:>8.2}%",
                kind.name(),
                stats.mono_hits[k],
                stats.poly_hits[k],
                stats.fills[k],
                stats.unfilled[k],
                rate
            );
        }
        let (mono, poly, mega) = ics.site_states();
        let _ = writeln!(
            out,
            "sites: {} entries, {mono} monomorphic, {poly} polymorphic, {mega} megamorphic",
            ics.len()
        );
        let _ = writeln!(
            out,
            "iteration: {} pristine arrays, {} through the protocol",
            stats.iter_pristine, stats.iter_generic
        );
        let mut sites = Vec::new();
        for (id, unit) in self.code.units() {
            for (pc, insn) in unit.code.iter().enumerate() {
                if let Some(site) = insn.site() {
                    let entry = (unit.ic_base + site) as usize;
                    let misses = stats.site_misses[entry];
                    if misses > 0 {
                        sites.push((misses, id, pc, *insn));
                    }
                }
            }
        }
        sites.sort_by_key(|&(misses, id, pc, _)| (std::cmp::Reverse(misses), id, pc));
        for &(misses, id, pc, insn) in sites.iter().take(worst) {
            let unit = self.code.get(id);
            let name = if unit.debug.name.is_empty() {
                "<anonymous>"
            } else {
                &unit.debug.name
            };
            let key = match insn.op {
                crate::bytecode::Opcode::New => String::new(),
                _ => crate::vm::property::key_text(&self.heap, insn.key()),
            };
            let _ = writeln!(
                out,
                "  {misses:>10} misses  {name} @{pc} {} {key}",
                insn.op.name()
            );
        }
        Some(out)
    }

    /// The disassembly of `unit`, with atom keys spelled out.
    pub fn disassemble(&self, unit: CodeUnitId) -> String {
        let heap = &self.heap;
        bytecode::disassemble(self.code.get(unit), &mut |key| match key.kind() {
            KeyKind::String(id) => format!(".{}", heap.string_text(id)),
            _ => bytecode::disasm::key_placeholder(key),
        })
    }

    pub fn global_object(&self) -> ObjectId {
        self.realm.global
    }

    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.realm.intrinsic(intrinsic)
    }

    /// A tagged template's site object (phase 4, section 5.2): a frozen
    /// array of the cooked strings with a frozen `raw` array of the raw
    /// ones. The values need no rooting (atoms and `undefined`).
    pub fn template_object(
        &mut self,
        cooked: &[Value],
        raw: &[Value],
    ) -> Result<ObjectId, HeapError> {
        self.compiler().template_object(cooked, raw)
    }

    /// An ordinary object inheriting from `%Object.prototype%`.
    pub fn new_object(&mut self) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::ObjectPrototype);
        self.heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
    }

    pub fn new_array(&mut self, capacity: u32) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::ArrayPrototype);
        self.heap.alloc_array(Some(prototype), capacity)
    }

    pub fn new_function(&mut self, scope: Option<ScopeId>) -> Result<ObjectId, HeapError> {
        let prototype = self.intrinsic(Intrinsic::FunctionPrototype);
        self.heap.alloc_function(Some(prototype), scope)
    }

    pub fn new_scope(&mut self, parent: Option<ScopeId>, slots: u32) -> Result<ScopeId, HeapError> {
        self.heap.alloc_scope(parent, slots)
    }

    /// What a test host reports of a thrown object without running
    /// JavaScript: the error name (an own `name` string, else the
    /// prototype's, else the prototype's `constructor`'s own `name` other
    /// than `Object`), the own `message` string and the own `stack` string.
    /// Accessors and non-string values count as absent; `None` for a
    /// primitive.
    pub fn thrown_summary(&mut self, value: Value) -> Option<ThrownSummary> {
        let object = value.as_object()?;
        let heap = &mut self.heap;
        let (Ok(name), Ok(message), Ok(constructor)) = (
            heap.intern("name"),
            heap.intern("message"),
            heap.intern("constructor"),
        ) else {
            return Some(ThrownSummary::default());
        };
        let heap = &self.heap;
        let names = &self.realm.names;
        let own = |object: ObjectId, key: StringId| {
            crate::vm::property::own(heap, names, object, Names::key(key))
        };
        let own_string = |object: ObjectId, key: StringId| match own(object, key) {
            Some(crate::vm::property::Found::Data(value)) => {
                Some(value.as_string().map(|id| heap.string_text(id)))
            }
            Some(_) => Some(None),
            None => None,
        };
        // `Some(Some(name))` names it, `Some(None)` stops the walk.
        let named = |found: Option<Option<String>>| match found {
            Some(Some(name)) if !name.is_empty() => Some(Some(name)),
            Some(_) => Some(None),
            None => None,
        };
        let error_name = named(own_string(object, name))
            .or_else(|| {
                let prototype = heap.prototype_of(object)?;
                named(own_string(prototype, name)).or_else(|| {
                    let Some(crate::vm::property::Found::Data(function)) =
                        own(prototype, constructor)
                    else {
                        return Some(None);
                    };
                    let function = function.as_object()?;
                    Some(
                        own_string(function, name)
                            .flatten()
                            .filter(|name| !name.is_empty() && name != "Object"),
                    )
                })
            })
            .flatten();
        Some(ThrownSummary {
            name: error_name,
            message: own_string(object, message).flatten(),
            stack: own_string(object, self.realm.names.stack).flatten(),
        })
    }

    /// What every part of the runtime holds (phase 6, section 2.4), read
    /// without a collection. Its reserved total equals [`Heap::bytes`].
    pub fn memory_report(&self) -> crate::heap::report::MemoryReport {
        let mut report = self.heap.report();
        let code = &mut report.code;
        for (_, unit) in self.code.units() {
            code.units += 1;
            code.instructions += unit.code.len();
            code.instruction_bytes += std::mem::size_of_val(&*unit.code);
            code.position_bytes += std::mem::size_of_val(&*unit.debug.positions);
            code.source_bytes += unit.source.as_ref().map_or(0, |source| source.len());
        }
        code.regexp_programs = self.regexps.len();
        report
    }

    /// Holds `value` for the embedder until [`Runtime::release`].
    pub fn hold(&mut self, value: Value) -> Handle {
        self.heap.handles().create(value)
    }

    pub fn held(&self, handle: Handle) -> Value {
        self.heap.handles_ref().get(handle)
    }

    pub fn release(&mut self, handle: Handle) {
        self.heap.handles().release(handle);
    }
}
