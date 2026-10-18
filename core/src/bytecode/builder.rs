//! The construction API the lowering emits through: one method per operand
//! format, forward jumps patched by index, and `finish` into a [`CodeUnit`].
//!
//! Each method asserts (in debug builds) that the opcode has the format it is
//! built with; the publication validator checks the operand values.

use super::{Capture, CodeUnit, CodeUnitId, DebugInfo, Format, Handler, Insn, Opcode};
use crate::heap::atom::PropertyKey;
use crate::vm::ic::{FIELD_NO_SITE, GLOBAL_NO_SITE, IcKind};

#[derive(Default)]
pub struct CodeBuilder {
    code: Vec<Insn>,
    handlers: Vec<Handler>,
}

impl CodeBuilder {
    pub fn new() -> CodeBuilder {
        CodeBuilder::default()
    }

    /// The index the next instruction gets.
    #[inline]
    pub fn pc(&self) -> u32 {
        self.code.len() as u32
    }

    #[track_caller]
    fn push(&mut self, format: Format, insn: Insn) -> u32 {
        debug_assert_eq!(
            insn.op.format(),
            format,
            "{} built with the wrong format",
            insn.op.name()
        );
        let pc = self.pc();
        self.code.push(insn);
        pc
    }

    /// Any instruction, without the format check.
    pub fn raw(&mut self, insn: Insn) -> u32 {
        let format = insn.op.format();
        self.push(format, insn)
    }

    #[track_caller]
    pub fn op(&mut self, op: Opcode) -> u32 {
        self.push(Format::None, Insn::new(op, 0, 0, 0, 0))
    }

    #[track_caller]
    pub fn a(&mut self, op: Opcode, a: u8) -> u32 {
        self.push(Format::A, Insn::new(op, a, 0, 0, 0))
    }

    #[track_caller]
    pub fn ab(&mut self, op: Opcode, a: u8, b: u8) -> u32 {
        self.push(Format::AB, Insn::new(op, a, b, 0, 0))
    }

    #[track_caller]
    pub fn abc(&mut self, op: Opcode, a: u8, b: u8, c: u8) -> u32 {
        self.push(Format::ABC, Insn::new(op, a, b, c, 0))
    }

    #[track_caller]
    pub fn a_int(&mut self, op: Opcode, a: u8, value: i32) -> u32 {
        self.push(Format::AInt, Insn::new(op, a, 0, 0, value as u32))
    }

    #[track_caller]
    pub fn ab_int(&mut self, op: Opcode, a: u8, b: u8, value: i32) -> u32 {
        self.push(Format::ABInt, Insn::new(op, a, b, 0, value as u32))
    }

    #[track_caller]
    pub fn a_const(&mut self, op: Opcode, a: u8, index: u32) -> u32 {
        self.push(Format::AConst, Insn::new(op, a, 0, 0, index))
    }

    #[track_caller]
    pub fn key(&mut self, op: Opcode, key: PropertyKey) -> u32 {
        self.push(Format::Key, Insn::new(op, 0, 0, 0, key.raw()))
    }

    #[track_caller]
    pub fn a_key(&mut self, op: Opcode, a: u8, key: PropertyKey) -> u32 {
        self.push(Format::AKey, Insn::new(op, a, 0, 0, key.raw()))
    }

    #[track_caller]
    pub fn ab_key(&mut self, op: Opcode, a: u8, b: u8, key: PropertyKey) -> u32 {
        self.push(Format::ABKey, Insn::new(op, a, b, 0, key.raw()))
    }

    #[track_caller]
    pub fn a_capture(&mut self, op: Opcode, a: u8, capture: u32) -> u32 {
        self.push(Format::ACapture, Insn::new(op, a, 0, 0, capture))
    }

    #[track_caller]
    pub fn a_count(&mut self, op: Opcode, a: u8, count: u32) -> u32 {
        self.push(Format::ACount, Insn::new(op, a, 0, 0, count))
    }

    #[track_caller]
    pub fn ab_range(&mut self, op: Opcode, a: u8, first: u8, count: u32) -> u32 {
        self.push(Format::ABRange, Insn::new(op, a, first, 0, count))
    }

    #[track_caller]
    pub fn a_child(&mut self, op: Opcode, a: u8, child: u32) -> u32 {
        self.push(Format::AChild, Insn::new(op, a, 0, 0, child))
    }

    #[track_caller]
    pub fn call(&mut self, op: Opcode, dst: u8, callee: u8, argc: u8) -> u32 {
        self.push(Format::Call, Insn::new(op, dst, callee, argc, 0))
    }

    #[track_caller]
    pub fn jump(&mut self, op: Opcode, target: u32) -> u32 {
        self.push(Format::Jump, Insn::new(op, 0, 0, 0, target))
    }

    #[track_caller]
    pub fn a_jump(&mut self, op: Opcode, a: u8, target: u32) -> u32 {
        self.push(Format::AJump, Insn::new(op, a, 0, 0, target))
    }

    #[track_caller]
    pub fn ab_jump(&mut self, op: Opcode, a: u8, b: u8, target: u32) -> u32 {
        self.push(Format::ABJump, Insn::new(op, a, b, 0, target))
    }

    #[track_caller]
    pub fn a_short_jump(&mut self, op: Opcode, a: u8, value: i16, target: u32) -> u32 {
        let [b, c] = value.to_le_bytes();
        self.push(Format::AShortJump, Insn::new(op, a, b, c, target))
    }

    #[track_caller]
    pub fn a_wide(&mut self, op: Opcode, a: u8, register: u32) -> u32 {
        self.push(Format::AWide, Insn::new(op, a, 0, 0, register))
    }

    /// Changes the destination register of the instruction at `pc` (the
    /// lowering's retargeting of a temporary into a local).
    pub fn retarget(&mut self, pc: u32, a: u8) {
        self.code[pc as usize].a = a;
    }

    /// Sets the target of the jump at `pc`, emitted before its target was
    /// known.
    #[track_caller]
    pub fn patch_target(&mut self, pc: u32, target: u32) {
        let insn = &mut self.code[pc as usize];
        assert!(insn.has_target(), "{} has no jump target", insn.op.name());
        insn.imm = target;
    }

    /// Adds a handler; nested handlers are added before the ones enclosing
    /// them.
    pub fn handler(&mut self, handler: Handler) {
        self.handlers.push(handler);
    }

    pub fn code(&self) -> &[Insn] {
        &self.code
    }

    /// The unit, with the layout and counts the lowering decided, and its cache
    /// sites numbered: the named-property sites first, in instruction order,
    /// then the global and `new` sites.
    pub fn finish(mut self, layout: UnitLayout) -> CodeUnit {
        let ic_count = number_sites(&mut self.code);
        CodeUnit {
            code: self.code.into(),
            handlers: self.handlers.into(),
            register_count: layout.register_count,
            parameter_count: layout.parameter_count,
            local_count: layout.local_count,
            flags: layout.flags,
            this_slots: layout.this_slots,
            constant_count: layout.constant_count,
            spill_register: layout.spill_register.unwrap_or(super::NO_SPILL),
            spill_slots: layout.spill_slots,
            ic_base: 0,
            ic_count,
            children: layout.children.into(),
            captures: layout.captures.into(),
            debug: DebugInfo {
                name: layout.name.into(),
                filename: layout.filename,
                positions: Box::new([]),
                definition: None,
            },
            name_atom: layout.name_atom,
            length: layout.length,
            source: layout.source,
            direct_eval_sites: Box::new([]),
        }
    }
}

/// Writes each cache site's index into its instruction and returns the
/// number of entries the unit needs. `GetField` and `SetField` get 0 to 254
/// in `c`; past 255 sites they share [`FIELD_NO_SITE`], whose entry is
/// reserved and never filled. The global forms follow in `b | c << 8`, with
/// the same rule at [`GLOBAL_NO_SITE`]; `New` sites come last, in `imm`.
fn number_sites(code: &mut [Insn]) -> u32 {
    let is_field = |insn: &Insn| matches!(IcKind::of(insn.op), Some(IcKind::Get | IcKind::Set));
    let is_global = |insn: &Insn| IcKind::of(insn.op) == Some(IcKind::Global);
    let fields = code.iter().filter(|insn| is_field(insn)).count() as u32;
    let reserved = u32::from(FIELD_NO_SITE);
    let mut next = 0u32;
    for insn in code.iter_mut().filter(|insn| is_field(insn)) {
        insn.c = next.min(reserved) as u8;
        next += 1;
    }
    next = if fields > reserved {
        reserved + 1
    } else {
        fields
    };
    let globals = code.iter().filter(|insn| is_global(insn)).count() as u32;
    let first_global = next;
    for insn in code.iter_mut().filter(|insn| is_global(insn)) {
        let site = next.min(GLOBAL_NO_SITE);
        insn.b = site as u8;
        insn.c = (site >> 8) as u8;
        next += 1;
    }
    if first_global + globals > GLOBAL_NO_SITE {
        next = GLOBAL_NO_SITE + 1;
    }
    for insn in code.iter_mut().filter(|insn| insn.op == Opcode::New) {
        insn.imm = next;
        next += 1;
    }
    next
}

/// The parts of a [`CodeUnit`] that are not instructions.
#[derive(Clone, Debug, Default)]
pub struct UnitLayout {
    pub register_count: u16,
    pub parameter_count: u16,
    pub local_count: u16,
    pub flags: u16,
    pub this_slots: u16,
    pub constant_count: u32,
    pub spill_register: Option<u16>,
    pub spill_slots: u16,
    pub children: Vec<CodeUnitId>,
    pub captures: Vec<Capture>,
    pub name: String,
    /// The function's `name` property (an atom), or `None` for `""`.
    pub name_atom: Option<crate::value::StringId>,
    /// The function's `length` property.
    pub length: u16,
    /// The function's source text, for `Function.prototype.toString`, in
    /// WTF-8 (a lone surrogate of `Function` constructor source is kept as
    /// its three-byte form, as QuickJS's `JS_ToCStringLen2` keeps it).
    pub source: Option<Box<[u8]>>,
    /// The script or module the function belongs to.
    pub filename: Option<std::rc::Rc<str>>,
}
