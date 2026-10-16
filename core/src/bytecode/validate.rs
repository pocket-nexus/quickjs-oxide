//! The publication validator: run once per code unit, so the main loop can
//! index registers, constants, children and targets without further checks.
//! A failure is an internal error of the lowering, never a JavaScript
//! exception.

use std::fmt;

use super::{CodeUnit, Format, flags};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidateErrorKind {
    EmptyCode,
    /// The last instruction falls through past the end of the code.
    FallsOffEnd,
    /// `register_count` is below `1 + parameters + locals`.
    LayoutOverflow,
    Register(u32),
    WideRegister(u32),
    /// The call window `b ..= b + 1 + c` reaches past `register_count`.
    CallWindow,
    /// The element range of `NewArray` reaches past `register_count`.
    RegisterRange,
    Target(u32),
    Constant(u32),
    Child(u32),
    /// A child id that is not yet published.
    UnpublishedChild(u32),
    /// `start < end <= code.len()` does not hold.
    HandlerRange,
    /// Two handler ranges overlap without nesting, or an enclosing handler
    /// precedes one nested in it.
    HandlerNesting,
    /// The line table is neither empty nor one entry per instruction.
    LineTable,
    /// An arrow function marked as a constructor.
    Flags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidateError {
    /// The instruction or handler index the error is at, when there is one.
    pub at: Option<u32>,
    pub kind: ValidateErrorKind,
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid code unit: {:?}", self.kind)?;
        if let Some(at) = self.at {
            write!(f, " at {at}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidateError {}

fn fail<T>(at: Option<u32>, kind: ValidateErrorKind) -> Result<T, ValidateError> {
    Err(ValidateError { at, kind })
}

/// Checks every operand of `unit`. `published` is the number of units already
/// in the store; children must be among them.
pub fn validate(unit: &CodeUnit, published: usize) -> Result<(), ValidateError> {
    use ValidateErrorKind as E;
    let len = unit.code.len() as u32;
    let registers = unit.register_count as u32;
    let Some(last) = unit.code.last() else {
        return fail(None, E::EmptyCode);
    };
    if !last.op.is_terminator() {
        return fail(Some(len - 1), E::FallsOffEnd);
    }
    if unit.temp_base() > registers {
        return fail(None, E::LayoutOverflow);
    }
    if unit.has(flags::ARROW) && unit.has(flags::CONSTRUCTOR) {
        return fail(None, E::Flags);
    }
    if !unit.debug.lines.is_empty() && unit.debug.lines.len() != unit.code.len() {
        return fail(None, E::LineTable);
    }
    for (index, &child) in unit.children.iter().enumerate() {
        if child.0 as usize >= published {
            return fail(Some(index as u32), E::UnpublishedChild(child.0));
        }
    }

    for (pc, insn) in unit.code.iter().enumerate() {
        let at = Some(pc as u32);
        let register = |r: u8| {
            if (r as u32) < registers {
                Ok(())
            } else {
                fail(at, E::Register(r as u32))
            }
        };
        let target = |t: u32| {
            if t < len {
                Ok(())
            } else {
                fail(at, E::Target(t))
            }
        };
        match insn.op.format() {
            Format::None | Format::Key | Format::Count => {}
            Format::A | Format::AInt | Format::AKey | Format::AScope | Format::ACount => {
                register(insn.a)?;
            }
            Format::AB | Format::ABInt | Format::ABKey => {
                register(insn.a)?;
                register(insn.b)?;
            }
            Format::ABC => {
                register(insn.a)?;
                register(insn.b)?;
                register(insn.c)?;
            }
            Format::AConst => {
                register(insn.a)?;
                if insn.imm >= unit.constant_count {
                    return fail(at, E::Constant(insn.imm));
                }
            }
            Format::ABRange => {
                register(insn.a)?;
                if insn.imm > 0 && insn.b as u64 + insn.imm as u64 > registers as u64 {
                    return fail(at, E::RegisterRange);
                }
            }
            Format::AChild => {
                register(insn.a)?;
                if insn.imm as usize >= unit.children.len() {
                    return fail(at, E::Child(insn.imm));
                }
            }
            Format::Call => {
                register(insn.a)?;
                if insn.b as u32 + 1 + insn.c as u32 >= registers {
                    return fail(at, E::CallWindow);
                }
            }
            Format::Jump => target(insn.imm)?,
            Format::AJump | Format::AShortJump => {
                register(insn.a)?;
                target(insn.imm)?;
            }
            Format::ABJump => {
                register(insn.a)?;
                register(insn.b)?;
                target(insn.imm)?;
            }
            Format::AWide => {
                register(insn.a)?;
                if insn.imm >= registers {
                    return fail(at, E::WideRegister(insn.imm));
                }
            }
        }
    }

    for (index, handler) in unit.handlers.iter().enumerate() {
        let at = Some(index as u32);
        if handler.start >= handler.end || handler.end > len {
            return fail(at, E::HandlerRange);
        }
        if handler.target >= len {
            return fail(at, E::Target(handler.target));
        }
        if handler.register as u32 >= registers {
            return fail(at, E::Register(handler.register as u32));
        }
        // Lookup takes the first covering entry, so an entry may only be
        // followed by entries that are disjoint from it or enclose it.
        for later in &unit.handlers[index + 1..] {
            let disjoint = later.end <= handler.start || handler.end <= later.start;
            let encloses = later.start <= handler.start && handler.end <= later.end;
            if !disjoint && !encloses {
                return fail(at, E::HandlerNesting);
            }
        }
    }
    Ok(())
}
