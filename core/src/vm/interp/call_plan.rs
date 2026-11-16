//! Execute a validated read-only return expression with the ordinary no-allocation entry
//! admission (poll counter, reserved stack window and receiver normalization). `EMPTY` means miss: publish the frame and
//! run the unchanged body with ordinary entry. Nothing here may allocate, call JS or mutate guest
//! state; cache-hit statistics are committed only when the whole plan succeeds.

use super::*;
use crate::bytecode::call_plan::CallPlan;

#[inline(never)]
pub(super) fn value(
    cx: &mut Cx<'_>,
    unit: &CodeUnit,
    plan: &CallPlan,
    base: usize,
    argc: usize,
    this: Value,
    bytecode: BytecodeId,
) -> Value {
    let end = base + unit.register_count as usize;
    if cx.heap.vm.poll <= 1
        || end + SPAN > cx.heap.vm.registers.len()
        || (unit.has(flags::ARGUMENTS) && argc > unit.parameter_count as usize)
    {
        return Value::EMPTY;
    }
    let this = if unit.has(flags::STRICT) || this.as_object().is_some() {
        this
    } else if this.is_undefined() || this.is_null() {
        Value::object(cx.realm.global())
    } else {
        return Value::EMPTY;
    };
    let heap = &mut *cx.heap;
    let ic_base = unit.ic_base as usize;
    let reg = |r: u8| {
        if r == 0 {
            this
        } else if r as usize <= argc {
            heap.vm.registers[base + r as usize]
        } else {
            Value::UNDEFINED
        }
    };
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
    if value.is_empty() {
        return Value::EMPTY;
    }
    heap.vm.poll -= 1;
    for _ in 0..hits {
        heap.ics.count_hit(IcKind::Get);
    }
    value
}
