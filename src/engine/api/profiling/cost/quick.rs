//! Publication diagnostics, separate from executed-opcode or live-heap counts.
use super::{CompilePhase, current};
use crate::engine::code::quick::{QUICK_TAG_PROFILE_NAMES, QuickProgram};

pub(crate) fn record_quick_projection(program: &QuickProgram, canonical_len: usize) {
    let Some(collector) = current() else {
        return;
    };
    let storage = program.storage();
    let words = storage.buffer_identity.is_some();
    let capacity_bytes = storage.word_capacity.saturating_mul(size_of::<u64>()) as u64;
    // An Rc<Vec<_>> allocation carries a Vec header and two reference counts
    // in std's current layout. Allocator bookkeeping/alignment slack is unknown.
    let control_bytes = if words {
        (size_of::<Vec<u64>>() + 2 * size_of::<usize>()) as u64
    } else {
        0
    };
    let mut costs = collector.borrow_mut();
    let phase = CompilePhase::QuickProjection.cost_mut(&mut costs);
    phase.storage_samples = phase.storage_samples.saturating_add(1);
    phase.maximum_observed_ir_capacity_bytes =
        phase.maximum_observed_ir_capacity_bytes.max(capacity_bytes);
    let counts = &mut costs.quick_projection_counts;
    for (name, value) in [
        ("functions", 1),
        ("canonical_only_functions", u64::from(!words)),
        ("word_functions", u64::from(words)),
        ("canonical_instructions", canonical_len as u64),
        ("words", storage.word_len as u64),
        ("word_capacity_bytes", capacity_bytes),
        ("word_control_bytes_estimate", control_bytes),
    ] {
        let count = counts.entry(name).or_default();
        *count = count.saturating_add(value);
    }
    for (index, name) in QUICK_TAG_PROFILE_NAMES.into_iter().enumerate() {
        let value = if !words && index == 0 {
            canonical_len
        } else {
            storage.tag_counts[index]
        };
        let count = counts.entry(name).or_default();
        *count = count.saturating_add(value as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::code::bytecode::Instruction;

    #[test]
    fn quick_profile_separates_builds_words_and_cold_canonical_positions() {
        let profile = CostProfile::start();
        let cold = QuickProgram::build_verified(&[Instruction::ReturnUndefined]).unwrap();
        let hot =
            QuickProgram::build_verified(&[Instruction::PushI32(7), Instruction::Return]).unwrap();
        let before_clone = profile.snapshot();
        let _shared = hot.clone();
        assert_eq!(before_clone, profile.snapshot());
        assert_eq!(before_clone.quick_projection.attempts, 2);
        assert_eq!(before_clone.quick_projection.storage_samples, 2);
        let counts = &before_clone.quick_projection_counts;
        assert_eq!(counts["functions"], 2);
        assert_eq!(counts["canonical_only_functions"], 1);
        assert_eq!(counts["word_functions"], 1);
        assert_eq!(counts["canonical_instructions"], 3);
        assert_eq!(counts["words"], 2);
        assert_eq!(counts["tag.GenericCanonical"], 2);
        assert_eq!(counts["tag.PushI32"], 1);
        assert_eq!(
            counts["word_capacity_bytes"],
            (hot.storage().word_capacity * size_of::<u64>()) as u64
        );
        assert_eq!(cold.storage().buffer_identity, None);
    }

    #[test]
    fn quick_profile_records_guarded_numeric_branch_and_slot_tags() {
        let profile = CostProfile::start();
        let code = [
            Instruction::Add,
            Instruction::Eq,
            Instruction::IfFalse(0),
            Instruction::GetLocal(0),
            Instruction::PutArg(0),
            Instruction::Return,
        ];
        let _program = QuickProgram::build_verified(&code).unwrap();
        let snapshot = profile.snapshot();
        let counts = snapshot.quick_projection_counts;
        for name in [
            "tag.Add",
            "tag.Eq",
            "tag.IfFalse",
            "tag.GetLocal",
            "tag.PutArg",
            "tag.GenericCanonical",
        ] {
            assert_eq!(counts[name], 1, "{name}");
        }
        assert_eq!(
            QUICK_TAG_PROFILE_NAMES
                .iter()
                .map(|name| counts[name])
                .sum::<u64>(),
            code.len() as u64
        );
    }
}
