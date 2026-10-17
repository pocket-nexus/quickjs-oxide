//! Register bytecode (new-core phase 2, section 2): fixed-width instructions,
//! code units and the store that owns them.
//!
//! A code unit is built once by the lowering, checked once by
//! [`validate`](validate::validate) when it is published to the
//! [`CodeStore`], and then run without any decoding: the main loop matches on
//! the typed [`Opcode`] and indexes registers with plain `[]`.

pub mod builder;
pub mod disasm;
pub mod opcode;
pub mod validate;

use std::fmt;

pub use self::builder::CodeBuilder;
pub use self::disasm::disassemble;
pub use self::opcode::{Format, Opcode};
pub use self::validate::{ValidateError, ValidateErrorKind};
use crate::heap::atom::PropertyKey;

/// One register instruction. `a`, `b`, `c` are register operands or small
/// counts; `imm` is a constant index, an absolute jump target, a key's bits,
/// a scope slot or an `i32` literal, as the opcode's [`Format`] says.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Insn {
    pub op: Opcode,
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub imm: u32,
}

const _: () = assert!(size_of::<Insn>() == 8);

/// Register operands address a 256-register window; registers above it are
/// reached through `LoadWide` and `StoreWide`.
pub const WINDOW: u32 = 256;

impl Insn {
    #[inline]
    pub const fn new(op: Opcode, a: u8, b: u8, c: u8, imm: u32) -> Insn {
        Insn { op, a, b, c, imm }
    }

    /// The `imm` field read as an `i32` literal.
    #[inline]
    pub const fn int(self) -> i32 {
        self.imm as i32
    }

    /// The `i16` literal of an [`Format::AShortJump`] instruction.
    #[inline]
    pub const fn short(self) -> i16 {
        (self.b as u16 | (self.c as u16) << 8) as i16
    }

    /// The `imm` field read as a property key.
    #[inline]
    pub const fn key(self) -> PropertyKey {
        PropertyKey::from_raw(self.imm)
    }

    /// The cache site of an instruction that has one (phase 3, section 2.1):
    /// `c` for `GetField` and `SetField`, `b | c << 8` for the global forms,
    /// `imm` for `New`. An index into the unit's entries of the
    /// [`IcStore`](crate::vm::ic::IcStore).
    #[inline]
    pub const fn site(self) -> Option<u32> {
        match self.op {
            Opcode::GetField | Opcode::SetField => Some(self.c as u32),
            Opcode::GetGlobal | Opcode::GetGlobalOrUndefined | Opcode::SetGlobal => {
                Some(self.b as u32 | (self.c as u32) << 8)
            }
            Opcode::New => Some(self.imm),
            _ => None,
        }
    }

    /// Whether `imm` is a jump target.
    #[inline]
    pub const fn has_target(self) -> bool {
        matches!(
            self.op.format(),
            Format::Jump | Format::AJump | Format::ABJump | Format::AShortJump
        )
    }
}

impl fmt::Debug for Insn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} a={} b={} c={} imm={:#x}",
            self.op.name(),
            self.a,
            self.b,
            self.c,
            self.imm
        )
    }
}

/// Index of a [`CodeUnit`] in the [`CodeStore`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CodeUnitId(pub(crate) u32);

impl CodeUnitId {
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for CodeUnitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CodeUnitId({})", self.0)
    }
}

/// An exception handler: a throw at an instruction in `start..end` stores the
/// thrown value in `register` (the temporary at the handler's stack depth)
/// and continues at `target`. Entries are ordered innermost first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handler {
    pub start: u32,
    pub end: u32,
    pub target: u32,
    pub register: u16,
}

pub mod flags {
    /// Strict-mode code: `this` is not coerced.
    pub const STRICT: u16 = 1 << 0;
    /// The function may be called with `new`.
    pub const CONSTRUCTOR: u16 = 1 << 1;
    /// An arrow function: `this` is lexical, never a constructor.
    pub const ARROW: u16 = 1 << 2;
    /// A script body.
    pub const SCRIPT: u16 = 1 << 3;
    /// The function reads its actual arguments past its parameters
    /// (`arguments` or a rest parameter): entering it keeps the extra
    /// arguments above its registers.
    pub const ARGUMENTS: u16 = 1 << 4;
    /// The function reads `new.target` (phase 4, section 4.5): its first
    /// local, right after the parameters, receives it when it is entered by
    /// a construction.
    pub const NEW_TARGET: u16 = 1 << 5;
    /// A class constructor: calling it without `new` throws.
    pub const CLASS_CONSTRUCTOR: u16 = 1 << 6;
    /// A derived class constructor: no receiver is created; `this` is bound
    /// by `super()`.
    pub const DERIVED: u16 = 1 << 7;
    /// A generator function.
    pub const GENERATOR: u16 = 1 << 8;
    /// An async function (with `GENERATOR`, an async generator).
    pub const ASYNC: u16 = 1 << 9;
    /// `MakeClosure` gives the function a `prototype` object (an ordinary
    /// function or a generator; class constructors get theirs from
    /// `MakeClass`).
    pub const PROTOTYPE: u16 = 1 << 10;
    /// Module code.
    pub const MODULE: u16 = 1 << 11;
    /// A class field or static initializer that brands its receiver with its
    /// home object's private-method brand before it runs.
    pub const BRAND: u16 = 1 << 12;
    /// The root of `eval` code: `import()` in it resolves against the code
    /// that called `eval`.
    pub const EVAL: u16 = 1 << 13;
}

/// Where a closure created by `MakeClosure` gets one captured cell, read in
/// the frame that runs the `MakeClosure`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// A slot with no cell (a global the closure reads by name).
    None,
    /// The cell in this register of the creating frame.
    Register(u16),
    /// The creating function's own capture at this index.
    Outer(u16),
    /// The cell in this slot of the creating frame's spill scope (a local
    /// past the register window).
    Slot(u16),
}

/// [`CodeUnit::spill_register`] of a unit without a spill scope.
pub const NO_SPILL: u16 = u16::MAX;

/// Source information kept for the disassembler and error messages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub name: Box<str>,
    /// The script or module the function belongs to, for `import()`.
    pub filename: Option<std::rc::Rc<str>>,
    /// Where the instructions came from, sorted by `pc`: each entry holds
    /// for its instruction and the ones after it up to the next. Empty when
    /// the producer recorded none.
    pub positions: Box<[SourcePosition]>,
}

/// A one-based source line and column, from instruction `pc` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePosition {
    pub pc: u32,
    pub line: u32,
    pub column: u32,
}

impl DebugInfo {
    /// The line and column of the instruction at `pc`, as QuickJS's
    /// `find_line_num` reads its table: the last entry at or before it.
    pub fn position(&self, pc: u32) -> Option<(u32, u32)> {
        let after = self.positions.partition_point(|entry| entry.pc <= pc);
        let entry = self.positions[..after].last()?;
        Some((entry.line, entry.column))
    }
}

/// The compiled form of one function.
///
/// The register window of a frame is laid out as: register 0 is `this`, then
/// `parameter_count` parameters, then `local_count` locals, then the operand
/// temporaries up to `register_count`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeUnit {
    pub code: Box<[Insn]>,
    pub handlers: Box<[Handler]>,
    pub register_count: u16,
    pub parameter_count: u16,
    pub local_count: u16,
    pub flags: u16,
    /// Distinct `this.x =` names in a constructor body: the slots `New`
    /// reserves on the receiver (section 3.3).
    pub this_slots: u16,
    /// The length of the constant table attached on the heap.
    pub constant_count: u32,
    /// The register holding the frame's spill scope, or [`NO_SPILL`]: a
    /// function with more locals than the register window holds keeps the
    /// rest in the slots of one scope object, created by `NewSpill`.
    pub spill_register: u16,
    /// The slots of the spill scope.
    pub spill_slots: u16,
    /// The unit's first entry in the cache store, set by
    /// [`CodeStore::publish`].
    pub ic_base: u32,
    /// Cache sites, numbered by [`CodeBuilder::finish`](builder::CodeBuilder::finish).
    pub ic_count: u32,
    /// Inner functions, by `MakeClosure` index. Published before the unit.
    pub children: Box<[CodeUnitId]>,
    /// The cells a closure over this unit captures, from the frame that
    /// creates it; `LoadCaptured` indexes them.
    pub captures: Box<[Capture]>,
    pub debug: DebugInfo,
    /// The function's `name` (an atom, so a strong root), or `None` for `""`.
    pub name_atom: Option<crate::value::StringId>,
    /// The function's `length`.
    pub length: u16,
    /// The function's source text, for `Function.prototype.toString`.
    pub source: Option<Box<str>>,
    /// Sorted indices of the calls written as a direct `eval`, which runs as
    /// indirect eval (phase 0): a backtrace lists no native frame for them,
    /// as QuickJS's direct eval has none.
    pub direct_eval_sites: Box<[u32]>,
}

impl CodeUnit {
    /// The first operand temporary, `T(0)`: after the locals, or after the
    /// spill register when the locals do not all fit in the window.
    #[inline]
    pub const fn temp_base(&self) -> u32 {
        if self.spill_register != NO_SPILL {
            self.spill_register as u32 + 1
        } else {
            1 + self.parameter_count as u32 + self.local_count as u32
        }
    }

    #[inline]
    pub const fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
}

/// Marks a [`CodeUnitId`] in the dynamic store (section 5.5).
const DYNAMIC: u32 = 1 << 31;
/// Units in the first dynamic chunk; each next chunk holds twice as many.
const FIRST_CHUNK: u32 = 64;
const CHUNKS: usize = 25;

/// One chunk of the dynamic store: write-once cells, allocated whole.
type DynamicChunk = Box<[std::cell::OnceCell<CodeUnit>]>;

/// Every published code unit. It lives in the runtime beside the heap, not in
/// it, so the main loop can hold `&[Insn]` across heap calls. Append-only:
/// code units are never collected.
///
/// Units published before the program runs go in `units` through `&mut`;
/// units compiled at run time (`eval`, the `Function` constructors) are
/// published through `&self` while the loop holds the store, into chunks of
/// write-once cells that never move, so every `&CodeUnit` stays valid.
pub struct CodeStore {
    units: Vec<CodeUnit>,
    dynamic: Box<[std::cell::OnceCell<DynamicChunk>]>,
    dynamic_len: std::cell::Cell<u32>,
    /// Cache entries reserved by the units so far.
    ic_len: std::cell::Cell<u32>,
}

impl Default for CodeStore {
    fn default() -> Self {
        CodeStore {
            units: Vec::new(),
            dynamic: (0..CHUNKS).map(|_| std::cell::OnceCell::new()).collect(),
            dynamic_len: std::cell::Cell::new(0),
            ic_len: std::cell::Cell::new(0),
        }
    }
}

/// The chunk and position of dynamic unit `index`.
#[inline]
fn locate(index: u32) -> (usize, usize) {
    let chunk = (index / FIRST_CHUNK + 1).ilog2();
    let start = FIRST_CHUNK * ((1 << chunk) - 1);
    (chunk as usize, (index - start) as usize)
}

impl CodeStore {
    fn reserve_caches(&self, unit: &mut CodeUnit) {
        unit.ic_base = self.ic_len.get();
        self.ic_len.set(
            self.ic_len
                .get()
                .checked_add(unit.ic_count)
                .expect("cache store overflow"),
        );
    }

    /// Validates `unit`, gives it the next `ic_count` cache entries and
    /// appends it.
    pub fn publish(&mut self, mut unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        validate::validate(&unit, &|id| self.lookup(id))?;
        let id = u32::try_from(self.units.len())
            .ok()
            .filter(|&id| id < DYNAMIC)
            .expect("code store overflow");
        self.reserve_caches(&mut unit);
        self.units.push(unit);
        Ok(CodeUnitId(id))
    }

    /// [`CodeStore::publish`] while the store is shared (code compiled at
    /// run time).
    pub fn publish_shared(&self, mut unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        validate::validate(&unit, &|id| self.lookup(id))?;
        let index = self.dynamic_len.get();
        let (chunk, offset) = locate(index);
        let cells = self.dynamic[chunk].get_or_init(|| {
            (0..(FIRST_CHUNK << chunk))
                .map(|_| std::cell::OnceCell::new())
                .collect()
        });
        self.reserve_caches(&mut unit);
        if cells[offset].set(unit).is_err() {
            unreachable!("a fresh dynamic slot");
        }
        self.dynamic_len.set(index + 1);
        Ok(CodeUnitId(index | DYNAMIC))
    }

    fn lookup(&self, id: CodeUnitId) -> Option<&CodeUnit> {
        if id.0 & DYNAMIC == 0 {
            return self.units.get(id.0 as usize);
        }
        let index = id.0 & !DYNAMIC;
        if index >= self.dynamic_len.get() {
            return None;
        }
        let (chunk, offset) = locate(index);
        self.dynamic[chunk].get()?[offset].get()
    }

    /// Cache entries the published units hold.
    pub fn ic_len(&self) -> u32 {
        self.ic_len.get()
    }

    #[inline]
    pub fn get(&self, id: CodeUnitId) -> &CodeUnit {
        if id.0 & DYNAMIC == 0 {
            return &self.units[id.0 as usize];
        }
        self.get_dynamic(id)
    }

    #[cold]
    #[inline(never)]
    fn get_dynamic(&self, id: CodeUnitId) -> &CodeUnit {
        self.lookup(id).expect("a published unit")
    }

    pub fn len(&self) -> usize {
        self.units.len() + self.dynamic_len.get() as usize
    }

    /// Every unit with its id, in publication order (the dynamic ones last).
    pub fn units(&self) -> impl Iterator<Item = (CodeUnitId, &CodeUnit)> {
        let dynamic = (0..self.dynamic_len.get()).map(|index| {
            (
                CodeUnitId(index | DYNAMIC),
                self.get(CodeUnitId(index | DYNAMIC)),
            )
        });
        self.units
            .iter()
            .enumerate()
            .map(|(index, unit)| (CodeUnitId(index as u32), unit))
            .chain(dynamic)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `id` and its descendants in pre-order: the order in which
    /// [`Runtime::instantiate`](crate::Runtime::instantiate) takes their
    /// constant tables.
    pub fn tree(&self, id: CodeUnitId) -> Vec<CodeUnitId> {
        let mut order = Vec::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            order.push(id);
            pending.extend(self.get(id).children.iter().rev());
        }
        order
    }

    /// Bytes held by the instructions and handler tables.
    pub fn bytes(&self) -> usize {
        self.units()
            .map(|(_, unit)| {
                size_of::<CodeUnit>()
                    + unit.code.len() * size_of::<Insn>()
                    + unit.handlers.len() * size_of::<Handler>()
                    + unit.children.len() * 4
                    + unit.captures.len() * size_of::<Capture>()
            })
            .sum()
    }
}
