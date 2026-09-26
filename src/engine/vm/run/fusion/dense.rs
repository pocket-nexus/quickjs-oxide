//! Non-owning execution of authenticated dense numeric spans.
use crate::engine::api::runtime::Runtime;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::fusion::{DenseSpanKind, DirectSlot, NumericSource};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::value::number::operations::Number;
use crate::engine::vm::run::PublishedDenseEntry;
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

fn peek_proven_number(
    slots: &RunSlots<'_>,
    runtime: &Runtime,
    base_slot: DirectSlot,
    key: Number,
) -> Option<Number> {
    let base = slots.direct_value(base_slot)?;
    runtime.peek_dense_number(base, index_from_number(key)?)
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

#[cfg(feature = "profiling")]
fn kind_name(kind: DenseSpanKind) -> &'static str {
    match kind {
        DenseSpanKind::Read => "dense_read",
        DenseSpanKind::ReadIndexBinary => "dense_read_index_binary",
        DenseSpanKind::ReadPostInc | DenseSpanKind::ReadPostDec => "dense_read_post_update",
        DenseSpanKind::ReadPreInc | DenseSpanKind::ReadPreDec => "dense_read_pre_update",
        DenseSpanKind::ReadBinary => "dense_read_binary",
        DenseSpanKind::AccPut => "dense_acc_put",
        DenseSpanKind::AccSetDrop => "dense_acc_set_drop",
        DenseSpanKind::AccIndexPut => "dense_acc_index_put",
        DenseSpanKind::AccIndexSetDrop => "dense_acc_index_set_drop",
        DenseSpanKind::Store => "dense_store",
        DenseSpanKind::Copy => "dense_copy",
        DenseSpanKind::StoreBinary => "dense_store_binary",
        DenseSpanKind::UpdateElement => "dense_update_element",
    }
}

/// Classify only after the original read has failed. The published source
/// shape is static; a missing direct binding and a non-Number value are the
/// two dynamic failures that matter for this probe.
#[cfg(feature = "profiling")]
fn read_miss(
    slots: &RunSlots<'_>,
    executable: &PublishedFunctionSnapshot,
    instruction: &Instruction,
) -> &'static str {
    match numeric_source(instruction) {
        None => "source",
        Some(NumericSource::Slot(slot)) => match slots.direct_value(slot) {
            None => "binding",
            Some(_) => "non_number",
        },
        Some(NumericSource::Constant(index)) => match executable.constant(index) {
            None => "source",
            Some(_) => "non_number",
        },
        Some(NumericSource::I32(_)) => "source",
    }
}

#[cfg(feature = "profiling")]
fn peek_miss(
    slots: &RunSlots<'_>,
    runtime: &Runtime,
    base: &Instruction,
    key: Number,
) -> &'static str {
    let Some(slot) = direct_slot(base) else {
        return "source";
    };
    let Some(base) = slots.direct_value(slot) else {
        return "binding";
    };
    let Some(index) = index_from_number(key) else {
        return "index";
    };
    runtime.diagnose_dense_number_read_miss(base, index)
}

/// A miss leaves the complete canonical span available at its original PC.
#[inline(never)]
pub(in crate::engine::vm::run) fn try_numeric_span(
    slots: &mut RunSlots<'_>,
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    entry: PublishedDenseEntry,
    property_generation: &mut u64,
) -> Option<usize> {
    let PublishedDenseEntry {
        kind,
        first: first_slot,
    } = entry;
    macro_rules! miss {
        ($reason:expr) => {{
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_fusion_outcome(
                runtime,
                executable,
                pc,
                kind_name(kind),
                Some($reason),
            );
            return None;
        }};
    }
    macro_rules! take {
        ($value:expr, $reason:expr) => {{
            match $value {
                Some(value) => value,
                None => miss!($reason),
            }
        }};
    }
    let end = take!(pc.checked_add(kind.len()), "source");
    let code = take!(executable.code.get(pc..end), "source");
    match kind {
        DenseSpanKind::Read
        | DenseSpanKind::ReadIndexBinary
        | DenseSpanKind::ReadPostInc
        | DenseSpanKind::ReadPreInc
        | DenseSpanKind::ReadPostDec
        | DenseSpanKind::ReadPreDec
        | DenseSpanKind::ReadBinary => {
            let (value, update) = match kind {
                DenseSpanKind::Read => {
                    let key = take!(
                        read_number(slots, executable, &code[1]),
                        read_miss(slots, executable, &code[1])
                    );
                    (
                        take!(
                            peek_proven_number(slots, runtime, first_slot, key),
                            peek_miss(slots, runtime, &code[0], key)
                        ),
                        None,
                    )
                }
                DenseSpanKind::ReadIndexBinary => {
                    let left = take!(
                        read_number(slots, executable, &code[1]),
                        read_miss(slots, executable, &code[1])
                    );
                    let right = take!(
                        read_number(slots, executable, &code[2]),
                        read_miss(slots, executable, &code[2])
                    );
                    let key = take!(apply_number_binary(&code[3], left, right), "source");
                    (
                        take!(
                            peek_proven_number(slots, runtime, first_slot, key),
                            peek_miss(slots, runtime, &code[0], key)
                        ),
                        None,
                    )
                }
                DenseSpanKind::ReadPostInc
                | DenseSpanKind::ReadPreInc
                | DenseSpanKind::ReadPostDec
                | DenseSpanKind::ReadPreDec => {
                    let (slot, old) = take!(
                        read_slot_number(slots, &code[1]),
                        read_miss(slots, executable, &code[1])
                    );
                    let postfix = matches!(
                        kind,
                        DenseSpanKind::ReadPostInc | DenseSpanKind::ReadPostDec
                    );
                    let increment =
                        matches!(kind, DenseSpanKind::ReadPostInc | DenseSpanKind::ReadPreInc);
                    debug_assert!(matches!(
                        (kind, &code[2]),
                        (DenseSpanKind::ReadPostInc, Instruction::PostInc)
                            | (DenseSpanKind::ReadPreInc, Instruction::Inc)
                            | (DenseSpanKind::ReadPostDec, Instruction::PostDec)
                            | (DenseSpanKind::ReadPreDec, Instruction::Dec)
                    ));
                    debug_assert!(store_matches(&code[3], slot, !postfix));
                    let next = old.update(increment);
                    let key = if postfix { old } else { next };
                    (
                        take!(
                            peek_proven_number(slots, runtime, first_slot, key),
                            peek_miss(slots, runtime, &code[0], key)
                        ),
                        Some(NumberUpdate { slot, value: next }),
                    )
                }
                DenseSpanKind::ReadBinary => {
                    let key = take!(
                        read_number(slots, executable, &code[1]),
                        read_miss(slots, executable, &code[1])
                    );
                    let read = take!(
                        peek_proven_number(slots, runtime, first_slot, key),
                        peek_miss(slots, runtime, &code[0], key)
                    );
                    let right = take!(
                        read_number(slots, executable, &code[3]),
                        read_miss(slots, executable, &code[3])
                    );
                    (
                        take!(apply_number_binary(&code[4], read, right), "source"),
                        None,
                    )
                }
                _ => unreachable!(),
            };
            if !slots.try_commit_proven_number(NumericDestination::Push, value, update, kind.peak())
            {
                miss!(if slots.numeric_span_room(kind.peak()) {
                    "commit"
                } else {
                    "room"
                });
            }
        }
        DenseSpanKind::AccPut
        | DenseSpanKind::AccSetDrop
        | DenseSpanKind::AccIndexPut
        | DenseSpanKind::AccIndexSetDrop => {
            let DirectSlot::Local(acc_index) = first_slot else {
                miss!("source");
            };
            let acc = take!(
                slots
                    .direct_value(first_slot)
                    .and_then(|value| value.as_number_repr()),
                read_miss(slots, executable, &code[0])
            );
            let indexed = matches!(
                kind,
                DenseSpanKind::AccIndexPut | DenseSpanKind::AccIndexSetDrop
            );
            let kept = matches!(
                kind,
                DenseSpanKind::AccSetDrop | DenseSpanKind::AccIndexSetDrop
            );
            let key = if indexed {
                let left = take!(
                    read_number(slots, executable, &code[2]),
                    read_miss(slots, executable, &code[2])
                );
                let right = take!(
                    read_number(slots, executable, &code[3]),
                    read_miss(slots, executable, &code[3])
                );
                take!(apply_number_binary(&code[4], left, right), "source")
            } else {
                take!(
                    read_number(slots, executable, &code[2]),
                    read_miss(slots, executable, &code[2])
                )
            };
            let read_pc = if indexed { 5 } else { 3 };
            let store_pc = if indexed { 7 } else { 5 };
            debug_assert!(matches!(code[read_pc], Instruction::GetArrayEl));
            debug_assert!(matches!(code[read_pc + 1], Instruction::Add));
            debug_assert!(store_matches(
                &code[store_pc],
                DirectSlot::Local(acc_index),
                kept
            ));
            let read = take!(
                peek_number(slots, runtime, &code[1], key),
                peek_miss(slots, runtime, &code[1], key)
            );
            if !slots.try_commit_proven_number(
                NumericDestination::Local(acc_index),
                acc.add(read),
                None,
                kind.peak(),
            ) {
                miss!(if slots.numeric_span_room(kind.peak()) {
                    "commit"
                } else {
                    "room"
                });
            }
        }
        DenseSpanKind::Store
        | DenseSpanKind::Copy
        | DenseSpanKind::StoreBinary
        | DenseSpanKind::UpdateElement => {
            if !slots.numeric_span_room(kind.peak()) {
                miss!("room");
            }
            let next_generation = take!(property_generation.checked_add(1), "generation");
            let (base_slot, key, value) = match kind {
                DenseSpanKind::Store => (
                    first_slot,
                    take!(
                        read_number(slots, executable, &code[1]),
                        read_miss(slots, executable, &code[1])
                    ),
                    take!(
                        read_number(slots, executable, &code[2]),
                        read_miss(slots, executable, &code[2])
                    ),
                ),
                DenseSpanKind::Copy => {
                    let source_key = take!(
                        read_number(slots, executable, &code[3]),
                        read_miss(slots, executable, &code[3])
                    );
                    let value = take!(
                        peek_number(slots, runtime, &code[2], source_key),
                        peek_miss(slots, runtime, &code[2], source_key)
                    );
                    (
                        first_slot,
                        take!(
                            read_number(slots, executable, &code[1]),
                            read_miss(slots, executable, &code[1])
                        ),
                        value,
                    )
                }
                DenseSpanKind::StoreBinary => {
                    let left = take!(
                        read_number(slots, executable, &code[2]),
                        read_miss(slots, executable, &code[2])
                    );
                    let right = take!(
                        read_number(slots, executable, &code[3]),
                        read_miss(slots, executable, &code[3])
                    );
                    (
                        first_slot,
                        take!(
                            read_number(slots, executable, &code[1]),
                            read_miss(slots, executable, &code[1])
                        ),
                        take!(apply_number_binary(&code[4], left, right), "source"),
                    )
                }
                DenseSpanKind::UpdateElement => {
                    let key = take!(
                        read_number(slots, executable, &code[1]),
                        read_miss(slots, executable, &code[1])
                    );
                    let old = take!(
                        peek_proven_number(slots, runtime, first_slot, key),
                        peek_miss(slots, runtime, &code[0], key)
                    );
                    let right = take!(
                        read_number(slots, executable, &code[3]),
                        read_miss(slots, executable, &code[3])
                    );
                    (
                        first_slot,
                        key,
                        take!(apply_number_binary(&code[4], old, right), "source"),
                    )
                }
                _ => unreachable!(),
            };
            let base = take!(slots.direct_value(base_slot), "binding");
            let index = take!(index_from_number(key), "index");
            if !runtime.try_write_dense_number(base, index, value) {
                miss!(runtime.diagnose_dense_number_write_miss(base, index));
            }
            *property_generation = next_generation;
        }
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_fusion_outcome(
        runtime,
        executable,
        pc,
        kind_name(kind),
        None,
    );
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(match kind {
        DenseSpanKind::Read => "fusion.DenseRead",
        DenseSpanKind::ReadIndexBinary => "fusion.DenseReadIndexBinary",
        DenseSpanKind::ReadPostInc | DenseSpanKind::ReadPostDec => "fusion.DenseReadPostUpdate",
        DenseSpanKind::ReadPreInc | DenseSpanKind::ReadPreDec => "fusion.DenseReadPreUpdate",
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
    use crate::engine::code::fusion::with_dense_candidates_disabled;

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_diagnostic_priority_reports_binding_before_index() {
        use crate::engine::code::bytecode::Instruction;
        use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
        use crate::engine::code::runtime::PublishedFunctionSnapshot;
        use crate::engine::vm::bindings::FrameBinding;
        use crate::engine::vm::stack::{FrameStorage, SlotStore};
        use std::rc::Rc;

        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.local_count = 1;
        owner.metadata.max_stack = 1;
        owner.local_definitions = Rc::from([VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        }]);
        let mut store = SlotStore::new(8);
        let mut window = store
            .push_frame(
                &runtime,
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: Vec::new(),
                    locals: vec![FrameBinding::Uninitialized],
                    operands: Vec::new(),
                },
            )
            .unwrap();
        {
            let slots = store.run_window(&mut window).unwrap();
            let base = Instruction::GetLocal(0);
            let key = super::Number::Int(-1);
            assert!(super::peek_number(&slots, &runtime, &base, key).is_none());
            assert_eq!(super::peek_miss(&slots, &runtime, &base, key), "binding");
        }
        store.clear_frame(&runtime, window).unwrap();
    }

    const ALL_KINDS: [(&str, i32, &str); 15] = [
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
            "(function(a,i){var v=a[i--];return v+i})([11,22],1)",
            22,
            "fusion.DenseReadPostUpdate",
        ),
        (
            "(function(a,i){var v=a[--i];return v+i})([11,22],1)",
            11,
            "fusion.DenseReadPreUpdate",
        ),
        (
            "(function(a,i){return a[i]+1})([11,22],0)",
            12,
            "fusion.DenseReadBinary",
        ),
        (
            "(function(a,i){var s=1;var s=s+a[i];return s})([11],0)",
            12,
            "fusion.DenseAccPut",
        ),
        (
            "(function(a,i){var s=1;s=s+a[i];return s})([11],0)",
            12,
            "fusion.DenseAccSetDrop",
        ),
        (
            "(function(a,i){var s=1;var s=s+a[i+1];return s})([11,22],0)",
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
    fn dense_spans_match_canonical_on_numbers_aliases_and_misses() {
        let cases = [
            "(function(a,i){return a[i+1]})([11,22],0)",
            "(function(a,i){var v=a[i++];return v+i})([11,22],0)",
            "(function(a,i){var v=a[++i];return v+i})([11,22],0)",
            "(function(a,i){var v=a[i--];return v+i})([11,22],1)",
            "(function(a,i){var v=a[--i];return v+i})([11,22],1)",
            "(function(a,i){return a[i]>>>1})([-1],0)",
            "(function(a,i){return a[i]+1})([2147483647],0)",
            "(function(a,i){return Object.is(a[i]*1,-0)})([-0],0)",
            "(function(a,i){var s=1;var s=s+a[i];return s})([11],0)",
            "(function(a,i){var s=1;var s=s+a[i+1];return s})([11,22],0)",
            "(function(a,i){var s=1;s=s+a[i];return s})([11],0)",
            "(function(a,i){var s=1;s=s+a[i+1];return s})([11,22],0)",
            "(function(a,i,v){a[i]=v;return a[i]})([11],0,22)",
            "(function(a,i){a[i]=a[i+1];return a[i]})([11,22],0)",
            "(function(a,i,v){a[i]=v+1;return a[i]})([11],0,21)",
            "(function(a,i,v){a[i]+=v;return a[i]})([11],0,11)",
            "(function(){let i=0,n=0,a=[];Object.defineProperty(a,0,{get(){n++;throw 7}});try{a[i++]}catch(e){}return i*10+n})()",
            "(function(){let i=0,n=0,a=[0];Object.defineProperty(a,1,{get(){n++;throw 7}});try{a[++i]}catch(e){}return i*10+n})()",
            "(function(){let i=1,n=0,a=[];Object.defineProperty(a,1,{get(){n++;throw 7}});try{a[i--]}catch(e){}return i*10+n})()",
            "(function(){let i=1,n=0,a=[];Object.defineProperty(a,0,{get(){n++;throw 7}});try{a[--i]}catch(e){}return i*10+n})()",
            "(function(){'use strict';let a=[1];Object.defineProperty(a,0,{writable:false});try{a[0]=2}catch(e){return a[0]}return 99})()",
            "(function(){let a=[1];Object.defineProperty(a,'length',{writable:false});a[0]=2;return a[0]})()",
            "(function(){let n=0,a=new Proxy([1],{set(t,k,v){n++;return Reflect.set(t,k,v)}});a[0]=2;return n*10+a[0]})()",
            "(function(){let a=['x'];return a[0]+1})()",
            "(function(){let a=[1];return a[-0]})()",
            "(function(){let a=[1];a[0]++;return a[0]})()",
        ];
        for source in cases {
            let runtime = Runtime::new();
            let fused = runtime.new_context().eval(source).unwrap();
            let runtime = Runtime::new();
            let canonical =
                with_dense_candidates_disabled(|| runtime.new_context().eval(source)).unwrap();
            assert_eq!(fused, canonical, "{source}");
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

    #[test]
    fn dense_first_operand_from_dispatch_matches_canonical_hit_and_fallback() {
        for (source, expected) in [
            ("(function(a,i){return a[i]})([42],0)", 42),
            ("(function(){let a=[42],i=0;return a[i]})()", 42),
            ("(function(){let a=[1],i=0;a[i]=41;return a[0]+1})()", 42),
            ("(function(a,i,v){a[i]=v;return a[0]+1})([1],0,41)", 42),
            (
                "(function(){let n=0,a=[];Object.defineProperty(a,0,{get(){n++;return 41}});function read(x,i){return x[i]}return read(a,0)+n})()",
                42,
            ),
            (
                "(function(){let n=0,a=[];Object.defineProperty(a,0,{get(){n++;throw 7}});function read(x,i){return x[i]}try{read(a,0)}catch(e){return n*10+e}})()",
                17,
            ),
            (
                "(function(){let n=0,a=new Proxy([1],{set(t,k,v){if(k==='0')n++;return Reflect.set(t,k,v)}});function write(x,i,v){x[i]=v}write(a,0,41);return a[0]+n})()",
                42,
            ),
        ] {
            let runtime = Runtime::new();
            let fused = runtime.new_context().eval(source).unwrap();
            assert!(
                runtime.0.state.borrow().active_frames.is_empty(),
                "{source}"
            );
            let runtime = Runtime::new();
            let canonical =
                with_dense_candidates_disabled(|| runtime.new_context().eval(source)).unwrap();
            assert!(
                runtime.0.state.borrow().active_frames.is_empty(),
                "{source}"
            );
            assert_eq!(fused, Value::Int(expected), "{source}");
            assert_eq!(fused, canonical, "{source}");
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

        let mut missing = Vec::new();
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
            if costs
                .owned_execution_events
                .get(event)
                .copied()
                .unwrap_or(0)
                == 0
            {
                let actual = costs
                    .owned_execution_events
                    .keys()
                    .filter(|name| name.starts_with("fusion.Dense"))
                    .cloned()
                    .collect::<Vec<_>>();
                missing.push(format!("{event} for {source}; actual {actual:?}"));
            }
        }
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_miss_diagnostic_preserves_all_update_directions_and_getter_order() {
        use crate::engine::api::profiling::CostProfile;

        for (source, expected, kind) in [
            (
                "(function(){let a=[0,0,0],n=0;Object.defineProperty(a,0,{get(){n++;return 7}});function f(a,i){let v=a[i++];return i*100+v}return f(a,0)+n})()",
                108,
                "dense_read_post_update",
            ),
            (
                "(function(){let a=[0,0,0],n=0;Object.defineProperty(a,1,{get(){n++;return 7}});function f(a,i){let v=a[++i];return i*100+v}return f(a,0)+n})()",
                108,
                "dense_read_pre_update",
            ),
            (
                "(function(){let a=[0,0,0],n=0;Object.defineProperty(a,2,{get(){n++;return 7}});function f(a,i){let v=a[i--];return i*100+v}return f(a,2)+n})()",
                108,
                "dense_read_post_update",
            ),
            (
                "(function(){let a=[0,0,0],n=0;Object.defineProperty(a,0,{get(){n++;return 7}});function f(a,i){let v=a[--i];return i*100+v}return f(a,1)+n})()",
                8,
                "dense_read_pre_update",
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let profile = CostProfile::start();
            let fused = context.eval(source).unwrap();
            let costs = profile.snapshot();
            let canonical = {
                let runtime = Runtime::new();
                with_dense_candidates_disabled(|| runtime.new_context().eval(source)).unwrap()
            };
            assert_eq!(fused, Value::Int(expected), "{source}");
            assert_eq!(fused, canonical, "{source}");
            assert!(
                costs.fusion_sites.iter().any(|(key, cost)| {
                    key.kind == kind
                        && cost
                            .misses
                            .get("array_materialized.own_accessor")
                            .copied()
                            .unwrap_or(0)
                            > 0
                }),
                "{source}: {costs:?}"
            );
        }
    }

    #[test]
    fn dense_update_fallback_preserves_tdz_and_captured_binding_semantics() {
        #[cfg(feature = "profiling")]
        use crate::engine::api::profiling::CostProfile;

        for (source, expected) in [
            (
                "(function(){let i=1;try{a[i--];return 0}catch(e){return e.name==='ReferenceError'&&i===1?42:0}let a=[11,22]})()",
                42,
            ),
            (
                "(function(){let a=[11,22],i=1;function hold(){return a}let v=a[i--];return v+i+hold()[0]})()",
                33,
            ),
        ] {
            let runtime = Runtime::new();
            #[cfg(feature = "profiling")]
            let profile = CostProfile::start();
            let fused = runtime.new_context().eval(source).unwrap();
            #[cfg(feature = "profiling")]
            {
                let costs = profile.snapshot();
                assert!(
                    costs.fusion_sites.iter().any(|(key, cost)| {
                        key.kind == "dense_read_post_update"
                            && cost.misses.get("binding").copied().unwrap_or(0) > 0
                    }),
                    "expected a published update span to fall back at its first binding: {source}: {costs:?}"
                );
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            let runtime = Runtime::new();
            let canonical =
                with_dense_candidates_disabled(|| runtime.new_context().eval(source)).unwrap();
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert_eq!(fused, Value::Int(expected), "{source}");
            assert_eq!(fused, canonical, "{source}");
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_put_accumulators_execute_published_bytecode() {
        use crate::engine::api::profiling::CostProfile;
        use crate::engine::code::bytecode::Instruction;
        use crate::engine::code::function::{UnlinkedFunction, metadata::FunctionMetadata};
        use crate::engine::code::fusion::with_dense_candidates_disabled;

        for (indexed, event, expected) in [
            (false, "fusion.DenseAccPut", Value::Int(12)),
            (true, "fusion.DenseAccIndexPut", Value::Int(23)),
        ] {
            let mut result = None;
            for disabled in [false, true] {
                let runtime = Runtime::new();
                let mut context = runtime.new_context();
                let array = context.eval("[11,22]").unwrap();
                let mut code = vec![
                    Instruction::PushI32(1),
                    Instruction::PutLocal(0),
                    Instruction::GetLocal(0),
                    Instruction::GetArg(0),
                    Instruction::GetArg(1),
                ];
                if indexed {
                    code.extend([Instruction::PushI32(1), Instruction::Add]);
                }
                code.extend([
                    Instruction::GetArrayEl,
                    Instruction::Add,
                    Instruction::PutLocal(0),
                    Instruction::GetLocal(0),
                    Instruction::Return,
                ]);
                let draft = UnlinkedFunction::fixture(
                    code,
                    vec![],
                    FunctionMetadata {
                        argument_count: 2,
                        defined_argument_count: 2,
                        local_count: 1,
                        max_stack: if indexed { 4 } else { 3 },
                        ..FunctionMetadata::default()
                    },
                );
                let published = if disabled {
                    with_dense_candidates_disabled(|| {
                        runtime.publish_unlinked_function(context.realm, draft)
                    })
                } else {
                    runtime.publish_unlinked_function(context.realm, draft)
                }
                .unwrap();
                let callable = runtime
                    .new_bytecode_closure(context.realm, &published)
                    .unwrap();
                let profile = CostProfile::start();
                let value = context
                    .call(&callable, Value::Undefined, &[array, Value::Int(0)])
                    .unwrap();
                let costs = profile.snapshot();
                assert_eq!(value, expected);
                if disabled {
                    assert_eq!(costs.owned_execution_events.get(event), None);
                    assert_eq!(Some(value), result);
                } else {
                    assert!(
                        costs
                            .owned_execution_events
                            .get(event)
                            .copied()
                            .unwrap_or(0)
                            > 0
                    );
                    result = Some(value);
                }
            }
        }
    }
}
