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

    /// The `(depth, index)` pair of an [`Format::AScope`] instruction.
    #[inline]
    pub const fn scope_slot(self) -> (u32, u32) {
        (self.imm >> 16, self.imm & 0xFFFF)
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
    pub const STRICT: u8 = 1 << 0;
    /// The function may be called with `new`.
    pub const CONSTRUCTOR: u8 = 1 << 1;
    /// An arrow function: `this` is lexical, never a constructor.
    pub const ARROW: u8 = 1 << 2;
    /// A script body.
    pub const SCRIPT: u8 = 1 << 3;
}

/// Source information kept for the disassembler and error messages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub name: Box<str>,
    /// The source line of each instruction, or empty.
    pub lines: Box<[u32]>,
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
    pub flags: u8,
    /// Distinct `this.x =` names in a constructor body: the slots `New`
    /// reserves on the receiver (section 3.3).
    pub this_slots: u16,
    /// The length of the constant table attached on the heap.
    pub constant_count: u32,
    /// Inner functions, by `MakeClosure` index. Published before the unit.
    pub children: Box<[CodeUnitId]>,
    pub debug: DebugInfo,
}

impl CodeUnit {
    /// The first operand temporary, `T(0)`.
    #[inline]
    pub const fn temp_base(&self) -> u32 {
        1 + self.parameter_count as u32 + self.local_count as u32
    }

    #[inline]
    pub const fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// Every published code unit. It lives in the runtime beside the heap, not in
/// it, so the main loop can hold `&[Insn]` across heap calls. Append-only in
/// phase 2: code units are never collected.
#[derive(Default)]
pub struct CodeStore {
    units: Vec<CodeUnit>,
}

impl CodeStore {
    /// Validates `unit` and appends it.
    pub fn publish(&mut self, unit: CodeUnit) -> Result<CodeUnitId, ValidateError> {
        validate::validate(&unit, self.units.len())?;
        let id = u32::try_from(self.units.len()).expect("code store overflow");
        self.units.push(unit);
        Ok(CodeUnitId(id))
    }

    #[inline]
    pub fn get(&self, id: CodeUnitId) -> &CodeUnit {
        &self.units[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
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
        self.units
            .iter()
            .map(|unit| {
                size_of::<CodeUnit>()
                    + unit.code.len() * size_of::<Insn>()
                    + unit.handlers.len() * size_of::<Handler>()
                    + unit.children.len() * 4
            })
            .sum()
    }
}
