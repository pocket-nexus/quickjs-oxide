//! Execute a validated read-only return expression after ordinary entry has
//! polled and prepared the window. `EMPTY` means miss: publish the frame and
//! run the unchanged body. Nothing here may allocate, call JS or mutate guest
//! state; cache-hit statistics are committed only when the whole plan succeeds.

use super::*;
use crate::bytecode::call_plan::CallPlan;

#[inline(never)]
pub(super) fn value(
    heap: &mut Heap,
    plan: &CallPlan,
    ic_base: usize,
    base: usize,
    bytecode: BytecodeId,
) -> Value {
    let reg = |r: u8| heap.vm.registers[base + r as usize];
    let get = |receiver, site: u8| {
        ic::get_hit(
            &heap.objects,
            &heap.cells,
            &heap.values,
            &heap.strings,
            heap.ics.entries[ic_base + site as usize],
            receiver,
        )
    };
    let (value, hits) = match *plan {
        CallPlan::None => return Value::EMPTY,
        CallPlan::Register(r) => (reg(r), 0),
        CallPlan::Constant { op, imm } => (
            match op {
                Opcode::LoadInt => Value::int32(imm as i32),
                Opcode::LoadConst => {
                    let constants = heap.payloads.bytecode.get(bytecode.index());
                    debug_assert!(imm < constants.constant_count);
                    heap.values.get(constants.constants, imm)
                }
                Opcode::LoadUndefined => Value::UNDEFINED,
                Opcode::LoadNull => Value::NULL,
                Opcode::LoadTrue => Value::TRUE,
                Opcode::LoadFalse => Value::FALSE,
                _ => unreachable!("a planned constant"),
            },
            0,
        ),
        CallPlan::Field { base, site } => {
            let Some(value) = get(reg(base), site) else {
                return Value::EMPTY;
            };
            (value, 1)
        }
        CallPlan::Chain {
            base,
            first,
            second,
        } => {
            let Some(first) = get(reg(base), first) else {
                return Value::EMPTY;
            };
            let Some(value) = get(first, second) else {
                return Value::EMPTY;
            };
            (value, 2)
        }
        CallPlan::Compare {
            left,
            right,
            left_site,
            right_site,
            op,
        } => {
            let Some(left) = get(reg(left), left_site) else {
                return Value::EMPTY;
            };
            let Some(right) = get(reg(right), right_site) else {
                return Value::EMPTY;
            };
            let Some(order) = order(left, right) else {
                return Value::EMPTY;
            };
            (Value::bool(JUMP_MASKS[op as usize] & order != 0), 2)
        }
    };
    for _ in 0..hits {
        heap.ics.count_hit(IcKind::Get);
    }
    value
}
