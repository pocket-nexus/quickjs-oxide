//! The `Runtime`: the heap, the code store and the realm. Every operation is
//! `fn(&mut Runtime, …)`; there is no shared interior mutability.

use crate::PropertyKey;
use crate::bytecode::{self, CodeStore, CodeUnit, CodeUnitId, ValidateError};
use crate::heap::BytecodeTemplate;
use crate::heap::atom::KeyKind;
use crate::heap::gc::{Handle, HeapLimits};
use crate::heap::hash::FxHashMap;
use crate::heap::object::ObjectKind;
use crate::heap::{Heap, HeapError};
use crate::value::{ObjectId, ScopeId, StringId, Value};
use crate::vm::regexp::RegExpStore;
use crate::vm::{Cx, ErrorKind, Host, Output, Throw, VmStack};

/// Intrinsic objects held by the realm. Phase 1 created the prototypes the
/// object kinds need; phase 2 adds the error prototypes the loop throws
/// with. Builtins are ported in phase 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Intrinsic {
    ObjectPrototype,
    FunctionPrototype,
    ArrayPrototype,
    /// The prototype of string receivers (phase 3 fills it with the
    /// `String.prototype` methods in M4).
    StringPrototype,
    /// The prototype of regular-expression objects.
    RegExpPrototype,
    ErrorPrototype,
    EvalErrorPrototype,
    RangeErrorPrototype,
    ReferenceErrorPrototype,
    SyntaxErrorPrototype,
    TypeErrorPrototype,
    InternalErrorPrototype,
    /// A preallocated `InternalError: out of memory`, thrown when the heap
    /// cannot allocate the error itself.
    OutOfMemoryError,
}

impl Intrinsic {
    pub const COUNT: usize = 13;
}

/// Atoms the loop names: property names it reads itself and the `typeof`
/// results. Atoms are strong roots, so they stay valid.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Names {
    pub length: StringId,
    pub callee: StringId,
    pub last_index: StringId,
    pub prototype: StringId,
    pub constructor: StringId,
    pub message: StringId,
    pub name: StringId,
    pub value_of: StringId,
    pub to_string: StringId,
    pub undefined: StringId,
    pub null: StringId,
    pub true_: StringId,
    pub false_: StringId,
    pub object: StringId,
    pub boolean: StringId,
    pub number: StringId,
    pub string: StringId,
    pub symbol: StringId,
    pub function: StringId,
    pub bigint: StringId,
    pub empty: StringId,
}

impl Names {
    fn new(heap: &mut Heap) -> Result<Names, HeapError> {
        Ok(Names {
            length: heap.intern("length")?,
            callee: heap.intern("callee")?,
            last_index: heap.intern("lastIndex")?,
            prototype: heap.intern("prototype")?,
            constructor: heap.intern("constructor")?,
            message: heap.intern("message")?,
            name: heap.intern("name")?,
            value_of: heap.intern("valueOf")?,
            to_string: heap.intern("toString")?,
            undefined: heap.intern("undefined")?,
            null: heap.intern("null")?,
            true_: heap.intern("true")?,
            false_: heap.intern("false")?,
            object: heap.intern("object")?,
            boolean: heap.intern("boolean")?,
            number: heap.intern("number")?,
            string: heap.intern("string")?,
            symbol: heap.intern("symbol")?,
            function: heap.intern("function")?,
            bigint: heap.intern("bigint")?,
            empty: heap.intern("")?,
        })
    }

    #[inline]
    pub fn key(id: StringId) -> PropertyKey {
        PropertyKey::atom(id)
    }
}

/// The single realm of a runtime (phase 0, section 2). Its objects are fixed
/// roots of the heap.
pub struct Realm {
    global: ObjectId,
    intrinsics: [ObjectId; Intrinsic::COUNT],
    pub(crate) names: Names,
}

impl Realm {
    #[inline]
    pub fn global(&self) -> ObjectId {
        self.global
    }

    #[inline]
    pub fn intrinsic(&self, intrinsic: Intrinsic) -> ObjectId {
        self.intrinsics[intrinsic as usize]
    }
}

pub struct Runtime {
    heap: Heap,
    /// Beside the heap, not in it: the main loop borrows the running unit's
    /// code across heap calls (phase 2, section 2).
    code: CodeStore,
    realm: Realm,
    host: Host,
    /// The regular-expression programs (phase 3, section 5.3): append-only,
    /// beside the heap like the code store.
    regexps: RegExpStore,
}

impl Runtime {
    pub fn new(limits: HeapLimits) -> Result<Runtime, HeapError> {
        let mut heap = Heap::new(limits);
        let root = |heap: &mut Heap, object: ObjectId| {
            heap.add_fixed_root(Value::object(object));
            object
        };
        let object_prototype = heap.alloc_object(ObjectKind::Ordinary, None, 4)?;
        let object_prototype = root(&mut heap, object_prototype);
        let function_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let function_prototype = root(&mut heap, function_prototype);
        let array_prototype = heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let array_prototype = root(&mut heap, array_prototype);
        let string_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let string_prototype = root(&mut heap, string_prototype);
        let regexp_prototype =
            heap.alloc_object(ObjectKind::Ordinary, Some(object_prototype), 4)?;
        let regexp_prototype = root(&mut heap, regexp_prototype);
        let global = heap.alloc_object(ObjectKind::GlobalObject, Some(object_prototype), 16)?;
        let global = root(&mut heap, global);
        let names = Names::new(&mut heap)?;
        let mut intrinsics = [object_prototype; Intrinsic::COUNT];
        intrinsics[Intrinsic::FunctionPrototype as usize] = function_prototype;
        intrinsics[Intrinsic::ArrayPrototype as usize] = array_prototype;
        intrinsics[Intrinsic::StringPrototype as usize] = string_prototype;
        intrinsics[Intrinsic::RegExpPrototype as usize] = regexp_prototype;
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
            intrinsics,
            names,
        };
        let mut runtime = Runtime {
            heap,
            code: CodeStore::default(),
            realm,
            host: Host::default(),
            regexps: RegExpStore::default(),
        };
        let out_of_memory = crate::vm::native::install_globals(&mut runtime.cx())?;
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
        self.realm.intrinsics[intrinsic as usize] = object;
    }

    pub fn regexps(&self) -> &RegExpStore {
        &self.regexps
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    pub fn host(&mut self) -> &mut Host {
        &mut self.host
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

    /// Defines the qjs `print` host function on the global object.
    pub fn install_print(&mut self) -> Result<(), HeapError> {
        crate::vm::native::install_print(&mut self.cx())
    }

    /// Runs a script function (from [`Runtime::instantiate`]) with the
    /// global object as `this`. The result is unrooted.
    pub fn run_script(&mut self, script: ObjectId) -> Result<Value, Throw> {
        let global = Value::object(self.realm.global);
        self.call(Value::object(script), global, &[])
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
        let order = self.code.tree(root);
        assert_eq!(order.len(), constants.len(), "one constant table per unit");
        let position: FxHashMap<CodeUnitId, u32> = order
            .iter()
            .enumerate()
            .map(|(index, &unit)| (unit, index as u32))
            .collect();
        let children: Vec<Vec<u32>> = order
            .iter()
            .map(|&id| {
                let unit = self.code.get(id);
                unit.children.iter().map(|child| position[child]).collect()
            })
            .collect();
        let tree: Vec<BytecodeTemplate<'_>> = order
            .iter()
            .zip(constants)
            .zip(&children)
            .map(|((&unit, &constants), children)| {
                assert_eq!(
                    constants.len(),
                    self.code.get(unit).constant_count as usize,
                    "constant table length"
                );
                BytecodeTemplate {
                    unit,
                    constants,
                    children,
                }
            })
            .collect();
        let prototype = self.intrinsic(Intrinsic::FunctionPrototype);
        self.heap
            .alloc_bytecode_function(Some(prototype), None, &tree)
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
