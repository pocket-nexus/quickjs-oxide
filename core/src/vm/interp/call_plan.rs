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
    let mut polls = 1;
    let mut clear_end = end;
    let (value, hits) = match *plan {
        CallPlan::None => return Value::EMPTY,
        CallPlan::Register(r) => (reg(r), 0),
        CallPlan::Constant { op, imm } => (constant(op, imm), 0),
        CallPlan::Delegate {
            base: receiver_base,
            receiver: receiver_site,
            method,
            call,
            argument,
            argc,
        } => {
            if heap.vm.poll <= 2 {
                return Value::EMPTY;
            }
            let Some(receiver) = get(reg(receiver_base), receiver_site) else {
                return Value::EMPTY;
            };
            if receiver.as_object().is_none() {
                return Value::EMPTY;
            }
            let Some(function) = get(receiver, method).and_then(Value::as_object) else {
                return Value::EMPTY;
            };
            if heap.object_kind(function) != ObjectKind::Function {
                return Value::EMPTY;
            }
            let record = heap.function_record(function);
            if record.bytecode == NONE {
                return Value::EMPTY;
            }
            let id = heap.bytecode_unit(BytecodeId(record.bytecode));
            if id.is_dynamic() {
                return Value::EMPTY;
            }
            let (next, next_plan) = cx.static_code.call_entry(id);
            let next_end = base + call as usize + 1 + next.register_count as usize;
            if next_end + SPAN > heap.vm.registers.len()
                || (next.has(flags::ARGUMENTS) && u16::from(argc) > next.parameter_count)
            {
                return Value::EMPTY;
            }
            let value = delegated_read(heap, next, next_plan, receiver, reg(argument), argc);
            if value.is_empty() {
                return Value::EMPTY;
            }
            let hits = match next_plan {
                CallPlan::Chain { .. } => 2,
                CallPlan::Field { .. } | CallPlan::Element { .. } => 1,
                _ => 0,
            };
            polls = 2;
            clear_end = end.max(next_end);
            (value, hits + 2)
        }
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
    heap.vm.poll -= polls;
    // The call-clobbered part still covered by the caller's root range must
    // not retain old objects. Unrooted temporaries need no initialization.
    let first = base + 1 + argc.min(unit.parameter_count as usize);
    let rooted_end = clear_end.min(heap.vm.top as usize);
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

/// A delegated leaf performs only guarded reads. Resolving its live method
/// value above preserves replacement, invalidation and closure identity;
/// unsupported plans return before publishing either activation.
#[inline(never)]
fn delegated_read(
    heap: &Heap,
    unit: &CodeUnit,
    plan: &CallPlan,
    this: Value,
    argument: Value,
    argc: u8,
) -> Value {
    macro_rules! need {
        ($value:expr) => {
            match $value {
                Some(value) => value,
                None => return Value::EMPTY,
            }
        };
    }
    let reg = |r| match r {
        0 => this,
        1 if argc == 1 => argument,
        _ => Value::UNDEFINED,
    };
    let get = |receiver, site: u8| {
        ic::get_hit(
            &heap.objects,
            &heap.cells,
            &heap.values,
            &heap.strings,
            heap.ics.entries[unit.ic_base as usize + site as usize],
            receiver,
        )
    };
    match *plan {
        CallPlan::Register(r) => reg(r),
        CallPlan::Field { base, site } => need!(get(reg(base), site)),
        CallPlan::Chain {
            base,
            first,
            second,
        } => {
            let first = need!(get(reg(base), first));
            need!(get(first, second))
        }
        CallPlan::Element { base, site, index } => {
            let receiver = need!(need!(get(reg(base), site)).as_object());
            let index = need!(reg(index).as_int32());
            if index < 0 {
                return Value::EMPTY;
            }
            let header = heap.objects.get(receiver.index());
            let elements = header.elements();
            if index as u32 >= elements.len
                || (header.kind == ObjectKind::Array && index as u32 >= header.payload)
            {
                return Value::EMPTY;
            }
            heap.values.get(elements, index as u32)
        }
        _ => Value::EMPTY,
    }
}
