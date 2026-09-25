//! Non-owning execution of authenticated dense numeric spans.
use crate::engine::api::runtime::Runtime;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::fusion::{DenseSpanKind, DirectSlot, NumericSource};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::value::number::operations::Number;
use crate::engine::vm::stack::{NumericDestination, RunSlots};

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

/// A miss leaves the complete canonical span available at its original PC.
#[inline(never)]
pub(in crate::engine::vm::run) fn try_numeric_span(
    slots: &mut RunSlots<'_>,
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    kind: DenseSpanKind,
    _property_generation: &mut u64,
) -> Option<usize> {
    // Only published, fully implemented kinds may reach this handler. Keep a
    // runtime guard as well so a future matcher change cannot consume a span
    // before its complete transaction has been implemented.
    if kind != DenseSpanKind::Read {
        return None;
    }
    let end = pc.checked_add(kind.len())?;
    let [base, key, Instruction::GetArrayEl] = executable.code.get(pc..end)? else {
        return None;
    };
    let base_slot = direct_slot(base)?;
    let key = index_from_number(read_numeric_source(
        slots,
        executable,
        numeric_source(key)?,
    )?)?;
    let value = {
        let base = slots.direct_value(base_slot)?;
        runtime.peek_dense_number(base, key)?
    };
    if !slots.try_commit_number(NumericDestination::Push, value, None, kind.peak()) {
        return None;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("fusion.DenseRead");
    Some(end)
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

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
}
