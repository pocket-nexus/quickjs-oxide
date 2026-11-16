//! Complete a planned call without publishing a frame. Admission preserves
//! polling, stack capacity and receiver normalization. `EMPTY` means miss:
//! ordinary entry runs the unchanged body. Reads may not allocate or call JS;
//! a store is the last guest operation and can never fall back after writing.
//! Hit statistics and dead-root cleanup are committed only on success.

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
    let constant = |op, imm| match op {
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
    };
    let global = |site: u16| {
        let entry = heap.ics.entries[ic_base + site as usize];
        if entry.w0 != heap.global_generation {
            return None;
        }
        let start = heap.objects.get(entry.w3).slots_start;
        Some(heap.values.get_at(start + entry.w1))
    };
    let mut global_hits = 0;
    let mut store_hit = false;
    let (value, hits) = match *plan {
        CallPlan::None => return Value::EMPTY,
        CallPlan::Register(r) => (reg(r), 0),
        CallPlan::Constant { op, imm } => (constant(op, imm), 0),
        CallPlan::BitTestOrCompare {
            base,
            left,
            left_global,
            right,
            right_global,
            op,
        } => {
            let receiver = reg(base);
            let Some(left) = get(receiver, left).and_then(Value::as_int32) else {
                return Value::EMPTY;
            };
            let Some(mask) = global(u16::from(left_global)).and_then(Value::as_int32) else {
                return Value::EMPTY;
            };
            global_hits = 1;
            if left & mask != 0 {
                (Value::TRUE, 1)
            } else {
                // The other arm is not read until the bit test is false.
                let Some(right) = get(receiver, right) else {
                    return Value::EMPTY;
                };
                let Some(other) = global(u16::from(right_global)) else {
                    return Value::EMPTY;
                };
                let Some(order) = order(right, other) else {
                    return Value::EMPTY;
                };
                global_hits = 2;
                (Value::bool(JUMP_MASKS[op as usize] & order != 0), 2)
            }
        }
        CallPlan::Update {
            base,
            read,
            global: site,
            op,
            write,
        } => {
            let receiver = reg(base);
            let Some(left) = get(receiver, read).and_then(Value::as_int32) else {
                return Value::EMPTY;
            };
            let Some(right) = global(site).and_then(Value::as_int32) else {
                return Value::EMPTY;
            };
            let value = match op {
                Opcode::BitAnd => Value::int32(left & right),
                Opcode::BitOr => Value::int32(left | right),
                Opcode::BitXor => Value::int32(left ^ right),
                Opcode::Shl => Value::int32(left.wrapping_shl(right as u32 & 31)),
                Opcode::Sar => Value::int32(left >> (right as u32 & 31)),
                Opcode::Shr => number::number(((left as u32) >> (right as u32 & 31)) as f64),
                _ => unreachable!("a planned integer update"),
            };
            let entry = heap.ics.entries[ic_base + write as usize];
            if !ic::set_hit(
                &mut heap.objects,
                &heap.cells,
                &mut heap.values,
                entry,
                receiver,
                value,
            ) {
                return Value::EMPTY;
            }
            global_hits = 1;
            store_hit = true;
            (Value::UNDEFINED, 1)
        }
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
            global_hits = 1;
            (Value::bool(JUMP_MASKS[op as usize] & order != 0), 2)
        }
        CallPlan::Select {
            base,
            left_site,
            global,
            right_site,
            op,
            taken,
            fallthrough,
        } => {
            let receiver = reg(base);
            let Some(left) = get(receiver, left_site) else {
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
            let site = if JUMP_MASKS[op as usize] & order != 0 {
                taken
            } else {
                fallthrough
            };
            let Some(value) = get(receiver, site) else {
                return Value::EMPTY;
            };
            global_hits = 1;
            (value, 3)
        }
        CallPlan::Store { base, value, site } => {
            let (object, value) = (reg(base), reg(value));
            let entry = heap.ics.entries[ic_base + site as usize];
            if !ic::set_hit(
                &mut heap.objects,
                &heap.cells,
                &mut heap.values,
                entry,
                object,
                value,
            ) {
                return Value::EMPTY;
            }
            store_hit = true;
            (Value::UNDEFINED, 0)
        }
        CallPlan::StoreConstant {
            base,
            site,
            op,
            imm,
        } => {
            let (object, value) = (reg(base), constant(op, imm));
            let entry = heap.ics.entries[ic_base + site as usize];
            if !ic::set_hit(
                &mut heap.objects,
                &heap.cells,
                &mut heap.values,
                entry,
                object,
                value,
            ) {
                return Value::EMPTY;
            }
            store_hit = true;
            (Value::UNDEFINED, 0)
        }
        CallPlan::GlobalStore {
            base,
            global,
            read_site,
            write_site,
        } => {
            let object = reg(base);
            let entry = heap.ics.entries[ic_base + global as usize];
            if entry.w0 != heap.global_generation {
                return Value::EMPTY;
            }
            let start = heap.objects.get(entry.w3).slots_start;
            let global = heap.values.get_at(start + entry.w1);
            let Some(value) = get(global, read_site) else {
                return Value::EMPTY;
            };
            let entry = heap.ics.entries[ic_base + write_site as usize];
            if !ic::set_hit(
                &mut heap.objects,
                &heap.cells,
                &mut heap.values,
                entry,
                object,
                value,
            ) {
                return Value::EMPTY;
            }
            global_hits = 1;
            store_hit = true;
            (Value::UNDEFINED, 1)
        }
    };
    if value.is_empty() {
        return Value::EMPTY;
    }
    heap.vm.poll -= 1;
    // The call-clobbered part still covered by the caller's root range must
    // not retain old objects. Unrooted temporaries need no initialization.
    let first = base + 1 + argc.min(unit.parameter_count as usize);
    let rooted_end = end.min(heap.vm.top as usize);
    if first < rooted_end {
        heap.vm.registers[first..rooted_end].fill(Value::UNDEFINED);
    }
    heap.vm.registers[base] = this;
    for _ in 0..hits {
        heap.ics.count_hit(IcKind::Get);
    }
    for _ in 0..global_hits {
        heap.ics.count_hit(IcKind::Global);
    }
    if store_hit {
        heap.ics.count_hit(IcKind::Set);
    }
    value
}
