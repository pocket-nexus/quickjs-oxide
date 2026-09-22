//! Guarded arithmetic, branch and direct-slot tags retain full canonical effects.
use super::*;

fn cases() -> Vec<(Instruction, DecodedOp, u8)> {
    vec![
        (Instruction::Add, DecodedOp::Add, 7),
        (Instruction::Sub, DecodedOp::Sub, 8),
        (Instruction::Mul, DecodedOp::Mul, 9),
        (Instruction::Div, DecodedOp::Div, 10),
        (Instruction::Mod, DecodedOp::Mod, 11),
        (Instruction::Pow, DecodedOp::Pow, 12),
        (Instruction::Shl, DecodedOp::Shl, 13),
        (Instruction::Sar, DecodedOp::Sar, 14),
        (Instruction::Shr, DecodedOp::Shr, 15),
        (Instruction::BitAnd, DecodedOp::BitAnd, 16),
        (Instruction::BitOr, DecodedOp::BitOr, 17),
        (Instruction::BitXor, DecodedOp::BitXor, 18),
        (Instruction::Eq, DecodedOp::Eq, 19),
        (Instruction::Neq, DecodedOp::Neq, 20),
        (Instruction::Lt, DecodedOp::Lt, 21),
        (Instruction::Lte, DecodedOp::Lte, 22),
        (Instruction::Gt, DecodedOp::Gt, 23),
        (Instruction::Gte, DecodedOp::Gte, 24),
        (
            Instruction::IfTrue(u32::MAX),
            DecodedOp::IfTrue(u32::MAX),
            25,
        ),
        (
            Instruction::IfFalse(u32::MAX),
            DecodedOp::IfFalse(u32::MAX),
            26,
        ),
        (
            Instruction::GetLocal(u16::MAX),
            DecodedOp::GetLocal(u16::MAX),
            27,
        ),
        (
            Instruction::PutLocal(u16::MAX),
            DecodedOp::PutLocal(u16::MAX),
            28,
        ),
        (
            Instruction::SetLocal(u16::MAX),
            DecodedOp::SetLocal(u16::MAX),
            29,
        ),
        (
            Instruction::GetArg(u16::MAX),
            DecodedOp::GetArg(u16::MAX),
            30,
        ),
        (
            Instruction::PutArg(u16::MAX),
            DecodedOp::PutArg(u16::MAX),
            31,
        ),
        (
            Instruction::SetArg(u16::MAX),
            DecodedOp::SetArg(u16::MAX),
            32,
        ),
    ]
}

#[test]
fn quick_expanded_tags_preserve_contracts_operands_and_reserved_bits() {
    let cases = cases();
    assert_eq!(cases.len(), 26);
    for (instruction, decoded, tag) in &cases {
        let word = translate_instruction(instruction);
        assert_eq!(word.0 & 0xff, u64::from(*tag));
        assert_eq!(word.decode(), Ok(*decoded));
        assert!(contracts_match(*decoded, instruction), "{instruction:?}");
        assert_eq!(
            validate_words(std::slice::from_ref(instruction), &[word]),
            Ok(())
        );
        for bit in 8..32 {
            assert_eq!(
                QuickOp(word.0 | (1_u64 << bit)).decode(),
                Err(DecodeError::ReservedBits)
            );
        }
    }
    // Even two operators with identical stack/effect contracts cannot replace
    // one another: the authenticated read-only tag is exact at every PC.
    let code = cases
        .iter()
        .map(|(instruction, _, _)| instruction.clone())
        .collect::<Vec<_>>();
    let mut words = words_for(&code);
    words.swap(0, 1);
    assert_eq!(
        validate_words(&code, &words),
        Err(ValidationError::CanonicalMismatch { pc: 0 })
    );
}

#[test]
fn quick_numeric_tags_keep_observable_fallback_effects() {
    for (instruction, decoded, _) in cases().into_iter().take(18) {
        assert_eq!(
            instruction.stack_contract(),
            StackEffect {
                popped: 2,
                pushed: 1,
                state: StackStateEffect::Ordinary,
            }
        );
        assert_eq!(
            instruction.potential_effects(),
            PotentialEffects {
                javascript_exception: JsExceptionEffect::MayThrow,
                may_call_js: true,
                may_allocate: true,
            }
        );
        assert!(contracts_match(decoded, &instruction));
        let word = translate_instruction(&instruction);
        for operand in [1, u32::from(u16::MAX), u32::MAX] {
            assert_eq!(
                QuickOp(word.0 | (u64::from(operand) << 32)).decode(),
                Err(DecodeError::UnexpectedOperand)
            );
        }
    }
}

#[test]
fn quick_slot_operands_never_truncate_to_u16() {
    type SlotConstructors = (fn(u16) -> Instruction, fn(u16) -> DecodedOp);
    let operations: [SlotConstructors; 6] = [
        (Instruction::GetLocal, DecodedOp::GetLocal),
        (Instruction::PutLocal, DecodedOp::PutLocal),
        (Instruction::SetLocal, DecodedOp::SetLocal),
        (Instruction::GetArg, DecodedOp::GetArg),
        (Instruction::PutArg, DecodedOp::PutArg),
        (Instruction::SetArg, DecodedOp::SetArg),
    ];
    for (canonical, decoded) in operations {
        for index in [0, 1, 255, 256, u16::MAX] {
            let instruction = canonical(index);
            let word = translate_instruction(&instruction);
            assert_eq!(word.0 >> 32, u64::from(index));
            assert_eq!(word.decode(), Ok(decoded(index)));
            assert_eq!(validate_words(&[instruction], &[word]), Ok(()));
        }
        let base = translate_instruction(&canonical(0));
        for invalid in [u32::from(u16::MAX) + 1, 0x8000_0000, u32::MAX] {
            let bad = QuickOp(base.0 | (u64::from(invalid) << 32));
            assert_eq!(bad.decode(), Err(DecodeError::InvalidSlot(invalid)));
            assert_eq!(
                validate_words(&[canonical(0)], &[bad]),
                Err(ValidationError::InvalidWord {
                    pc: 0,
                    reason: DecodeError::InvalidSlot(invalid),
                })
            );
        }
    }
}

#[test]
fn quick_conditional_targets_keep_all_canonical_pc_bits_and_polarity() {
    for target in [0, u32::from(u16::MAX), 65_536, 0x8000_0000, u32::MAX] {
        for (instruction, decoded) in [
            (Instruction::IfTrue(target), DecodedOp::IfTrue(target)),
            (Instruction::IfFalse(target), DecodedOp::IfFalse(target)),
        ] {
            let word = translate_instruction(&instruction);
            assert_eq!(word.0 >> 32, u64::from(target));
            assert_eq!(word.decode(), Ok(decoded));
            assert_eq!(instruction.control_effect(), ControlEffect::Branch(target));
            assert_eq!(
                instruction.potential_effects(),
                PotentialEffects {
                    javascript_exception: JsExceptionEffect::MayThrow,
                    may_call_js: false,
                    may_allocate: false,
                }
            );
            assert_eq!(validate_words(&[instruction], &[word]), Ok(()));
        }
        assert_eq!(
            validate_words(
                &[Instruction::IfTrue(target)],
                &[translate_instruction(&Instruction::IfFalse(target))]
            ),
            Err(ValidationError::CanonicalMismatch { pc: 0 })
        );
    }
}

#[cfg(feature = "profiling")]
#[test]
fn quick_expanded_cost_tags_cover_every_word_once() {
    let mut code = cases()
        .into_iter()
        .map(|(instruction, _, _)| instruction)
        .collect::<Vec<_>>();
    code.extend([Instruction::Nop, Instruction::Return]);
    let program = QuickProgram::build_verified(&code).unwrap();
    let storage = program.storage();
    assert_eq!(storage.tag_counts.iter().sum::<usize>(), code.len());
    for index in 7..QUICK_TAG_COUNT {
        assert_eq!(storage.tag_counts[index], 1);
    }
    assert_eq!(storage.tag_counts[0], 1);
    assert_eq!(storage.tag_counts[1], 1);
    assert_eq!(QUICK_TAG_PROFILE_NAMES.len(), QUICK_TAG_MEMORY_NAMES.len());
}
