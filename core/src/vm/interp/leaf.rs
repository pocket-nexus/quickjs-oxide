//! Short calls can finish before publishing their frame. Entry has already
//! polled, checked stack capacity, normalized `this` and initialized registers.
//! This prefix may neither collect, call guest/host code nor throw: it only
//! uses arena operations and the same numeric/cache hits as the main loop.
//! A miss publishes the frame at the first unexecuted instruction, so earlier
//! writes are never replayed. Backedges also publish, preserving polling.

use super::*;

pub(super) enum Outcome {
    Returned(Value),
    Publish(usize),
}

#[inline(never)]
pub(super) fn run(
    heap: &mut Heap,
    unit: &CodeUnit,
    base: usize,
    function: ObjectId,
    bytecode: BytecodeId,
) -> Outcome {
    let ic_base = unit.ic_base as usize;
    let mut pc = 0;
    loop {
        let insn = unit.code[pc];
        macro_rules! reg {
            ($r:expr) => {
                heap.vm.registers[base + $r as usize]
            };
        }
        let value = match insn.op {
            Opcode::Nop => {
                pc += 1;
                continue;
            }
            Opcode::Mov => reg!(insn.b),
            Opcode::LoadInt => Value::int32(insn.int()),
            Opcode::LoadConst => {
                let constants = heap.payloads.bytecode.get(bytecode.index());
                debug_assert!(insn.imm < constants.constant_count);
                heap.values.get(constants.constants, insn.imm)
            }
            Opcode::LoadUndefined => Value::UNDEFINED,
            Opcode::LoadNull => Value::NULL,
            Opcode::LoadTrue => Value::TRUE,
            Opcode::LoadFalse => Value::FALSE,
            Opcode::LoadFunction => Value::object(function),
            Opcode::Return => return Outcome::Returned(reg!(insn.a)),
            Opcode::ReturnUndefined => return Outcome::Returned(Value::UNDEFINED),
            Opcode::GetField => {
                let site = ic_base + insn.c as usize;
                let entry = heap.ics.entries[site];
                let Some(value) = ic::get_hit(
                    &heap.objects,
                    &heap.cells,
                    &heap.values,
                    &heap.strings,
                    entry,
                    reg!(insn.b),
                ) else {
                    return Outcome::Publish(pc);
                };
                heap.ics.count_hit(IcKind::Get);
                value
            }
            Opcode::SetField => {
                let object = reg!(insn.a);
                let value = reg!(insn.b);
                let site = ic_base + insn.c as usize;
                let entry = heap.ics.entries[site];
                if !ic::set_hit(
                    &mut heap.objects,
                    &heap.cells,
                    &mut heap.values,
                    entry,
                    object,
                    value,
                ) {
                    return Outcome::Publish(pc);
                }
                heap.ics.count_hit(IcKind::Set);
                pc += 1;
                continue;
            }
            Opcode::Add | Opcode::AddI | Opcode::Sub | Opcode::SubI => {
                let x = reg!(insn.b);
                let y = if matches!(insn.op, Opcode::AddI | Opcode::SubI) {
                    Value::int32(insn.int())
                } else {
                    reg!(insn.c)
                };
                let add = matches!(insn.op, Opcode::Add | Opcode::AddI);
                if let (Some(i), Some(j)) = (x.as_int32(), y.as_int32())
                    && let Some(value) = if add {
                        i.checked_add(j)
                    } else {
                        i.checked_sub(j)
                    }
                {
                    Value::int32(value)
                } else if let (Some(f), Some(g)) = (x.as_f64(), y.as_f64()) {
                    Value::number(if add { f + g } else { f - g })
                } else {
                    return Outcome::Publish(pc);
                }
            }
            Opcode::Inc | Opcode::Dec => {
                let Some(i) = reg!(insn.b).as_int32() else {
                    return Outcome::Publish(pc);
                };
                let Some(value) = (if insn.op == Opcode::Inc {
                    i.checked_add(1)
                } else {
                    i.checked_sub(1)
                }) else {
                    return Outcome::Publish(pc);
                };
                Value::int32(value)
            }
            Opcode::Not => Value::bool(!ops::to_boolean(heap, reg!(insn.b))),
            Opcode::Lt | Opcode::Lte | Opcode::Gt | Opcode::Gte | Opcode::Eq | Opcode::Neq => {
                let Some(order) = order(reg!(insn.b), reg!(insn.c)) else {
                    return Outcome::Publish(pc);
                };
                Value::bool(JUMP_MASKS[insn.op as usize] & order != 0)
            }
            Opcode::StrictEq | Opcode::StrictNeq => {
                let equal = ops::strict_eq(heap, reg!(insn.b), reg!(insn.c));
                Value::bool(equal == (insn.op == Opcode::StrictEq))
            }
            Opcode::Jmp | Opcode::JTrue | Opcode::JFalse | Opcode::JUndefinedOrNull => {
                let taken = match insn.op {
                    Opcode::Jmp => true,
                    Opcode::JUndefinedOrNull => {
                        reg!(insn.a).is_undefined() || reg!(insn.a).is_null()
                    }
                    _ => ops::to_boolean(heap, reg!(insn.a)) == (insn.op == Opcode::JTrue),
                };
                if taken {
                    let target = insn.imm as usize;
                    if target <= pc {
                        return Outcome::Publish(pc);
                    }
                    pc = target;
                } else {
                    pc += 1;
                }
                continue;
            }
            _ => return Outcome::Publish(pc),
        };
        reg!(insn.a) = value;
        pc += 1;
    }
}
