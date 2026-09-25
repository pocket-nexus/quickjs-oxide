//! Non-owning execution of authenticated dense numeric spans.
use crate::engine::api::runtime::Runtime;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::fusion::{DenseSpanKind, DirectSlot, NumericSource};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::value::number::operations::Number;
use crate::engine::vm::stack::{NumberUpdate, NumericDestination, RunSlots};

fn direct_slot(instruction: &Instruction) -> Option<DirectSlot> {
    match instruction {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => {
            Some(DirectSlot::Local(*index))
        }
        Instruction::GetArg(index) => Some(DirectSlot::Argument(*index)),
        _ => None,
    }
}

fn numeric_source(instruction: &Instruction) -> Option<NumericSource> {
    match instruction {
        Instruction::PushI32(value) => Some(NumericSource::I32(*value)),
        Instruction::PushConst(index) => Some(NumericSource::Constant(*index)),
        _ => direct_slot(instruction).map(NumericSource::Slot),
    }
}

fn read_numeric_source(
    slots: &RunSlots<'_>,
    executable: &PublishedFunctionSnapshot,
    source: NumericSource,
) -> Option<Number> {
    match source {
        NumericSource::Slot(slot) => slots.direct_value(slot)?.as_number_repr(),
        NumericSource::I32(value) => Some(Number::Int(value)),
        NumericSource::Constant(index) => match executable.constant(index)? {
            BytecodeConstant::Value(RawValue::Int(value)) => Some(Number::Int(*value)),
            BytecodeConstant::Value(RawValue::Float(value)) => Some(Number::Float(*value)),
            _ => None,
        },
    }
}

fn index_from_number(value: Number) -> Option<u32> {
    match value {
        Number::Int(index) if index >= 0 => Some(index as u32),
        _ => None,
    }
}

fn read_number(
    slots: &RunSlots<'_>,
    executable: &PublishedFunctionSnapshot,
    instruction: &Instruction,
) -> Option<Number> {
    read_numeric_source(slots, executable, numeric_source(instruction)?)
}

fn read_slot_number(
    slots: &RunSlots<'_>,
    instruction: &Instruction,
) -> Option<(DirectSlot, Number)> {
    let slot = direct_slot(instruction)?;
    Some((slot, slots.direct_value(slot)?.as_number_repr()?))
}

fn peek_number(
    slots: &RunSlots<'_>,
    runtime: &Runtime,
    base: &Instruction,
    key: Number,
) -> Option<Number> {
    let base = slots.direct_value(direct_slot(base)?)?;
    runtime.peek_dense_number(base, index_from_number(key)?)
}

fn apply_number_binary(op: &Instruction, left: Number, right: Number) -> Option<Number> {
    Some(match op {
        Instruction::Add => left.add(right),
        Instruction::Sub => left.sub(right),
        Instruction::Mul => left.mul(right),
        Instruction::Div => left.div(right),
        Instruction::BitAnd => Number::Int(left.int32() & right.int32()),
        Instruction::BitOr => Number::Int(left.int32() | right.int32()),
        Instruction::BitXor => Number::Int(left.int32() ^ right.int32()),
        Instruction::Shl => Number::Int(left.int32().wrapping_shl(right.int32() as u32 & 31)),
        Instruction::Sar => Number::Int(left.int32() >> (right.int32() as u32 & 31)),
        Instruction::Shr => Number::compact(f64::from(
            (left.int32() as u32) >> (right.int32() as u32 & 31),
        )),
        _ => return None,
    })
}

fn store_matches(instruction: &Instruction, slot: DirectSlot, keep: bool) -> bool {
    match (instruction, slot, keep) {
        (
            Instruction::PutLocal(actual) | Instruction::PutLocalCheck(actual),
            DirectSlot::Local(expected),
            false,
        )
        | (
            Instruction::SetLocal(actual) | Instruction::SetLocalCheck(actual),
            DirectSlot::Local(expected),
            true,
        )
        | (Instruction::PutArg(actual), DirectSlot::Argument(expected), false)
        | (Instruction::SetArg(actual), DirectSlot::Argument(expected), true) => {
            *actual == expected
        }
        _ => false,
    }
}

/// A miss leaves the complete canonical span available at its original PC.
#[inline(never)]
pub(in crate::engine::vm::run) fn try_numeric_span(
    slots: &mut RunSlots<'_>,
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    kind: DenseSpanKind,
    property_generation: &mut u64,
) -> Option<usize> {
    let end = pc.checked_add(kind.len())?;
    let code = executable.code.get(pc..end)?;
    match kind {
        DenseSpanKind::Read
        | DenseSpanKind::ReadIndexBinary
        | DenseSpanKind::ReadPostUpdate
        | DenseSpanKind::ReadPreUpdate
        | DenseSpanKind::ReadBinary => {
            let (value, update) = match kind {
                DenseSpanKind::Read => {
                    let key = read_number(slots, executable, &code[1])?;
                    (peek_number(slots, runtime, &code[0], key)?, None)
                }
                DenseSpanKind::ReadIndexBinary => {
                    let left = read_number(slots, executable, &code[1])?;
                    let right = read_number(slots, executable, &code[2])?;
                    let key = apply_number_binary(&code[3], left, right)?;
                    (peek_number(slots, runtime, &code[0], key)?, None)
                }
                DenseSpanKind::ReadPostUpdate | DenseSpanKind::ReadPreUpdate => {
                    let (slot, old) = read_slot_number(slots, &code[1])?;
                    let postfix = kind == DenseSpanKind::ReadPostUpdate;
                    let increment = matches!(code[2], Instruction::PostInc | Instruction::Inc);
                    if !matches!(
                        (&code[2], postfix),
                        (Instruction::PostInc | Instruction::PostDec, true)
                            | (Instruction::Inc | Instruction::Dec, false)
                    ) || !store_matches(&code[3], slot, !postfix)
                    {
                        return None;
                    }
                    let next = old.update(increment);
                    let key = if postfix { old } else { next };
                    (
                        peek_number(slots, runtime, &code[0], key)?,
                        Some(NumberUpdate { slot, value: next }),
                    )
                }
                DenseSpanKind::ReadBinary => {
                    let key = read_number(slots, executable, &code[1])?;
                    let read = peek_number(slots, runtime, &code[0], key)?;
                    let right = read_number(slots, executable, &code[3])?;
                    (apply_number_binary(&code[4], read, right)?, None)
                }
                _ => unreachable!(),
            };
            if !slots.try_commit_number(NumericDestination::Push, value, update, kind.peak()) {
                return None;
            }
        }
        DenseSpanKind::AccPut
        | DenseSpanKind::AccSetDrop
        | DenseSpanKind::AccIndexPut
        | DenseSpanKind::AccIndexSetDrop => {
            let (DirectSlot::Local(acc_index), acc) = read_slot_number(slots, &code[0])? else {
                return None;
            };
            let indexed = matches!(
                kind,
                DenseSpanKind::AccIndexPut | DenseSpanKind::AccIndexSetDrop
            );
            let kept = matches!(
                kind,
                DenseSpanKind::AccSetDrop | DenseSpanKind::AccIndexSetDrop
            );
            let key = if indexed {
                let left = read_number(slots, executable, &code[2])?;
                let right = read_number(slots, executable, &code[3])?;
                apply_number_binary(&code[4], left, right)?
            } else {
                read_number(slots, executable, &code[2])?
            };
            let read_pc = if indexed { 5 } else { 3 };
            let store_pc = if indexed { 7 } else { 5 };
            if !matches!(code[read_pc], Instruction::GetArrayEl)
                || !matches!(code[read_pc + 1], Instruction::Add)
                || !store_matches(&code[store_pc], DirectSlot::Local(acc_index), kept)
            {
                return None;
            }
            let read = peek_number(slots, runtime, &code[1], key)?;
            if !slots.try_commit_number(
                NumericDestination::Local(acc_index),
                acc.add(read),
                None,
                kind.peak(),
            ) {
                return None;
            }
        }
        DenseSpanKind::Store
        | DenseSpanKind::Copy
        | DenseSpanKind::StoreBinary
        | DenseSpanKind::UpdateElement => {
            if !slots.numeric_span_room(kind.peak()) {
                return None;
            }
            let next_generation = property_generation.checked_add(1)?;
            let (base_slot, key, value) = match kind {
                DenseSpanKind::Store => (
                    direct_slot(&code[0])?,
                    read_number(slots, executable, &code[1])?,
                    read_number(slots, executable, &code[2])?,
                ),
                DenseSpanKind::Copy => {
                    let source_key = read_number(slots, executable, &code[3])?;
                    let value = peek_number(slots, runtime, &code[2], source_key)?;
                    (
                        direct_slot(&code[0])?,
                        read_number(slots, executable, &code[1])?,
                        value,
                    )
                }
                DenseSpanKind::StoreBinary => {
                    let left = read_number(slots, executable, &code[2])?;
                    let right = read_number(slots, executable, &code[3])?;
                    (
                        direct_slot(&code[0])?,
                        read_number(slots, executable, &code[1])?,
                        apply_number_binary(&code[4], left, right)?,
                    )
                }
                DenseSpanKind::UpdateElement => {
                    let key = read_number(slots, executable, &code[1])?;
                    let old = peek_number(slots, runtime, &code[0], key)?;
                    let right = read_number(slots, executable, &code[3])?;
                    (
                        direct_slot(&code[0])?,
                        key,
                        apply_number_binary(&code[4], old, right)?,
                    )
                }
                _ => unreachable!(),
            };
            let base = slots.direct_value(base_slot)?;
            if !runtime.try_write_dense_number(base, index_from_number(key)?, value) {
                return None;
            }
            *property_generation = next_generation;
        }
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(match kind {
        DenseSpanKind::Read => "fusion.DenseRead",
        DenseSpanKind::ReadIndexBinary => "fusion.DenseReadIndexBinary",
        DenseSpanKind::ReadPostUpdate => "fusion.DenseReadPostUpdate",
        DenseSpanKind::ReadPreUpdate => "fusion.DenseReadPreUpdate",
        DenseSpanKind::ReadBinary => "fusion.DenseReadBinary",
        DenseSpanKind::AccPut => "fusion.DenseAccPut",
        DenseSpanKind::AccSetDrop => "fusion.DenseAccSetDrop",
        DenseSpanKind::AccIndexPut => "fusion.DenseAccIndexPut",
        DenseSpanKind::AccIndexSetDrop => "fusion.DenseAccIndexSetDrop",
        DenseSpanKind::Store => "fusion.DenseStore",
        DenseSpanKind::Copy => "fusion.DenseCopy",
        DenseSpanKind::StoreBinary => "fusion.DenseStoreBinary",
        DenseSpanKind::UpdateElement => "fusion.DenseUpdateElement",
    });
    Some(end)
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

    const ALL_KINDS: [(&str, i32, &str); 13] = [
        (
            "(function(a,i){return a[i]})([11],0)",
            11,
            "fusion.DenseRead",
        ),
        (
            "(function(a,i){return a[i+1]})([11,22],0)",
            22,
            "fusion.DenseReadIndexBinary",
        ),
        (
            "(function(a,i){var v=a[i++];return v+i})([11,22],0)",
            12,
            "fusion.DenseReadPostUpdate",
        ),
        (
            "(function(a,i){var v=a[++i];return v+i})([11,22],0)",
            23,
            "fusion.DenseReadPreUpdate",
        ),
        (
            "(function(a,i){return a[i]+1})([11,22],0)",
            12,
            "fusion.DenseReadBinary",
        ),
        (
            "(function(a,i){var s=1;s+=a[i];return s})([11],0)",
            12,
            "fusion.DenseAccPut",
        ),
        (
            "(function(a,i){var s=1;s=s+a[i];return s})([11],0)",
            12,
            "fusion.DenseAccSetDrop",
        ),
        (
            "(function(a,i){var s=1;s+=a[i+1];return s})([11,22],0)",
            23,
            "fusion.DenseAccIndexPut",
        ),
        (
            "(function(a,i){var s=1;s=s+a[i+1];return s})([11,22],0)",
            23,
            "fusion.DenseAccIndexSetDrop",
        ),
        (
            "(function(a,i,v){a[i]=v;return a[i]})([11],0,22)",
            22,
            "fusion.DenseStore",
        ),
        (
            "(function(a,b,i,j){a[i]=b[j];return a[i]})([11],[22],0,0)",
            22,
            "fusion.DenseCopy",
        ),
        (
            "(function(a,i,v){a[i]=v+1;return a[i]})([11],0,21)",
            22,
            "fusion.DenseStoreBinary",
        ),
        (
            "(function(a,i,v){a[i]+=v;return a[i]})([11],0,11)",
            22,
            "fusion.DenseUpdateElement",
        ),
    ];

    #[test]
    fn dense_all_spans_match_canonical_numeric_results() {
        for (source, expected, _) in ALL_KINDS {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(
                context.eval(source).unwrap(),
                Value::Int(expected),
                "{source}"
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn dense_read_preserves_single_owner_and_canonical_misses() {
        for source in [
            "(function(){let a=[42];return a[0]})()",
            "(function(){let a=[10,42];function read(x,i){return x[i]}return read(a,1)})()",
            "(function(){let a=[42];let i=0;return a[i]})()",
            "(function(){let a=[42];a[-1]=42;return a[-1]})()",
            "(function(){let calls=0,a=[];Object.defineProperty(a,0,{get(){calls++;return 41}});let v=a[0];return v+calls})()",
            "(function(){let a=[42];let i=0.0;return a[i]})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_read_uses_the_published_handler() {
        use crate::engine::api::profiling::CostProfile;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("(function(){let a=[42];function read(x,i){return x[i]}return read(a,0)})()")
                .unwrap(),
            Value::Int(42)
        );
        let costs = profile.snapshot();
        assert!(
            costs
                .owned_execution_events
                .get("fusion.DenseRead")
                .copied()
                .unwrap_or(0)
                > 0,
            "{costs:?}"
        );
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_all_spans_reach_the_published_handlers() {
        use crate::engine::api::profiling::CostProfile;

        for (source, expected, event) in ALL_KINDS {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let profile = CostProfile::start();
            assert_eq!(
                context.eval(source).unwrap(),
                Value::Int(expected),
                "{source}"
            );
            let costs = profile.snapshot();
            assert!(
                costs
                    .owned_execution_events
                    .get(event)
                    .copied()
                    .unwrap_or(0)
                    > 0,
                "expected {event} for {source}: {costs:?}"
            );
        }
    }
}
