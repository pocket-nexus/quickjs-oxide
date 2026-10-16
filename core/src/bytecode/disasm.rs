//! The disassembler: one instruction per line, registers by role (`this`,
//! `p0`, `l3`, `t2`, or `r300` above a frame's temporaries).

use std::fmt::Write;

use super::{Capture, CodeUnit, Format, Insn};
use crate::heap::atom::{KeyKind, PropertyKey};

/// Names register `r` of `unit` by its role in the frame layout.
pub fn register_name(unit: &CodeUnit, r: u32) -> String {
    let parameters = unit.parameter_count as u32;
    let locals = unit.local_count as u32;
    if r == 0 {
        "this".to_owned()
    } else if r <= parameters {
        format!("p{}", r - 1)
    } else if r <= parameters + locals {
        format!("l{}", r - 1 - parameters)
    } else if r < unit.register_count as u32 {
        format!("t{}", r - unit.temp_base())
    } else {
        format!("r{r}")
    }
}

/// A key without the heap: `[7]` for an index, `atom#12`, `sym#3`. The
/// runtime passes a namer that prints atom text instead.
pub fn key_placeholder(key: PropertyKey) -> String {
    match key.kind() {
        KeyKind::Index(index) => format!("[{index}]"),
        KeyKind::String(id) => format!("atom#{}", id.index()),
        KeyKind::Symbol(id) => format!("sym#{}", id.index()),
    }
}

/// The operands of one instruction, without the opcode.
pub fn operands(
    unit: &CodeUnit,
    insn: Insn,
    key_name: &mut dyn FnMut(PropertyKey) -> String,
) -> String {
    let r = |r: u8| register_name(unit, r as u32);
    let target = |t: u32| format!("@{t}");
    let text = match insn.op.format() {
        Format::None => String::new(),
        Format::A => r(insn.a),
        Format::AB => format!("{}, {}", r(insn.a), r(insn.b)),
        Format::ABC => format!("{}, {}, {}", r(insn.a), r(insn.b), r(insn.c)),
        Format::AInt => format!("{}, {}", r(insn.a), insn.int()),
        Format::ABInt => format!("{}, {}, {}", r(insn.a), r(insn.b), insn.int()),
        Format::AConst => format!("{}, k{}", r(insn.a), insn.imm),
        Format::Key => key_name(insn.key()),
        Format::AKey => format!("{}, {}", r(insn.a), key_name(insn.key())),
        Format::ABKey => format!("{}, {}, {}", r(insn.a), r(insn.b), key_name(insn.key())),
        Format::ACapture => format!("{}, c{}", r(insn.a), insn.imm),
        Format::ACount => format!("{}, {}", r(insn.a), insn.imm),
        Format::ABRange => {
            if insn.imm == 0 {
                format!("{}, []", r(insn.a))
            } else {
                let last = insn.b as u32 + insn.imm - 1;
                format!(
                    "{}, [{}..{}]",
                    r(insn.a),
                    r(insn.b),
                    register_name(unit, last)
                )
            }
        }
        Format::AChild => format!("{}, fn{}", r(insn.a), insn.imm),
        Format::Call => format!("{}, {}, {}", r(insn.a), r(insn.b), insn.c),
        Format::Jump => target(insn.imm),
        Format::AJump => format!("{}, {}", r(insn.a), target(insn.imm)),
        Format::ABJump => format!("{}, {}, {}", r(insn.a), r(insn.b), target(insn.imm)),
        Format::AShortJump => format!("{}, {}, {}", r(insn.a), insn.short(), target(insn.imm)),
        Format::AWide => format!("{}, {}", r(insn.a), register_name(unit, insn.imm)),
    };
    match insn.site() {
        Some(site) => format!("{text}, ic{site}"),
        None => text,
    }
}

/// The whole unit: a header line, the instructions, the handler table.
pub fn disassemble(unit: &CodeUnit, key_name: &mut dyn FnMut(PropertyKey) -> String) -> String {
    let mut out = String::new();
    let name = if unit.debug.name.is_empty() {
        "<anonymous>"
    } else {
        &unit.debug.name
    };
    let _ = writeln!(
        out,
        "{name}: params {}, locals {}, registers {}, constants {}, children {}, this slots {}, cache sites {}",
        unit.parameter_count,
        unit.local_count,
        unit.register_count,
        unit.constant_count,
        unit.children.len(),
        unit.this_slots,
        unit.ic_count
    );
    for (pc, &insn) in unit.code.iter().enumerate() {
        let operands = operands(unit, insn, key_name);
        let line = format!("  {pc:04}  {:<20}{operands}", insn.op.name());
        let _ = writeln!(out, "{}", line.trim_end());
    }
    for (index, capture) in unit.captures.iter().enumerate() {
        let source = match *capture {
            Capture::None => "none".to_owned(),
            Capture::Register(r) => format!("parent {}", r),
            Capture::Outer(i) => format!("parent c{i}"),
        };
        let _ = writeln!(out, "  capture c{index} <- {source}");
    }
    for handler in &unit.handlers {
        let _ = writeln!(
            out,
            "  handler @{}..@{} -> @{}, {}",
            handler.start,
            handler.end,
            handler.target,
            register_name(unit, handler.register as u32)
        );
    }
    out
}
