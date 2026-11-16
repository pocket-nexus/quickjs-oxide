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
    let mut global_hit = false;
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
        CallPlan::Element { base, site, index } => {
            let Some(receiver) = get(reg(base), site) else {
                return Value::EMPTY;
            };
            let (Some(object), Some(index)) = (receiver.as_object(), reg(index).as_int32()) else {
                return Value::EMPTY;
            };
            if index < 0 {
                return Value::EMPTY;
            }
            let header = heap.objects.get(object.index());
            let elements = header.elements();
            if index as u32 >= elements.len
                || (header.kind == ObjectKind::Array && index as u32 >= header.payload)
            {
                return Value::EMPTY;
            }
            (heap.values.get(elements, index as u32), 1)
        }
        CallPlan::GlobalCompare {
            left,
            left_site,
            global,
            right_site,
            op,
        } => {
            let Some(left) = get(reg(left), left_site) else {
                return Value::EMPTY;
            };
            let entry = heap.ics.entries[ic_base + global as usize];
            if entry.w0 != heap.global_generation {
                return Value::EMPTY;
            }
            let start = heap.objects.get(entry.w3).slots_start;
            let global = heap.values.get_at(start + entry.w1);
            let Some(right) = get(global, right_site) else {
                return Value::EMPTY;
            };
            let Some(order) = order(left, right) else {
                return Value::EMPTY;
            };
            global_hit = true;
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
    if global_hit {
        heap.ics.count_hit(IcKind::Global);
    }
    value
}
