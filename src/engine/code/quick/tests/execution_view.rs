//! Certified execution access agrees with the complete checked codec.
use super::*;

#[test]
fn quick_execution_view_preserves_every_tag_and_operand_boundary() {
    let mut cases = vec![
        (Instruction::Return, DecodedOp::GenericCanonical, 0),
        (Instruction::Nop, DecodedOp::Nop, 1),
        (Instruction::Undefined, DecodedOp::Undefined, 3),
        (Instruction::Null, DecodedOp::Null, 4),
        (Instruction::PushFalse, DecodedOp::Bool(false), 5),
        (Instruction::PushTrue, DecodedOp::Bool(true), 5),
    ];
    for value in [i32::MIN, -65_536, -1, 0, 1, 65_535, i32::MAX] {
        cases.push((Instruction::PushI32(value), DecodedOp::PushI32(value), 2));
    }
    for target in [0, 1, u32::from(u16::MAX), u32::MAX] {
        cases.push((Instruction::Goto(target), DecodedOp::Goto(target), 6));
    }
    cases.extend(super::expanded::cases());
    let code: Vec<_> = cases.iter().map(|(op, _, _)| op.clone()).collect();
    let program = QuickProgram::build_verified(&code).unwrap();
    let words = program.execution_words().unwrap();
    assert_eq!(words.len(), code.len());
    let mut seen = [false; QUICK_TAG_COUNT];
    for (word, (_, expected, tag)) in words.iter().zip(&cases) {
        assert_eq!(word.tag(), *tag);
        seen[usize::from(word.tag())] = true;
        assert_eq!(word.decode(), Ok(*expected));
        match expected {
            DecodedOp::PushI32(value) => assert_eq!(word.i32_operand(), *value),
            DecodedOp::Bool(value) => assert_eq!(word.boolean(), *value),
            DecodedOp::Goto(target) | DecodedOp::IfTrue(target) | DecodedOp::IfFalse(target) => {
                assert_eq!(word.operand(), *target)
            }
            DecodedOp::GetLocal(slot)
            | DecodedOp::PutLocal(slot)
            | DecodedOp::SetLocal(slot)
            | DecodedOp::GetArg(slot)
            | DecodedOp::PutArg(slot)
            | DecodedOp::SetArg(slot) => assert_eq!(word.slot(), *slot),
            _ => assert_eq!(word.operand(), 0),
        }
    }
    assert!(seen.into_iter().all(|tag| tag));
}

#[test]
fn quick_execution_view_borrows_shared_words_without_new_rc_or_pc_mapping() {
    let program = QuickProgram::build_verified(&[
        Instruction::PushI32(42),
        Instruction::GetLocal(u16::MAX),
        Instruction::Return,
    ])
    .unwrap();
    let shared = program.clone();
    let ProgramKind::Words(buffer) = &program.0 else {
        panic!("expected words");
    };
    let owners = Rc::strong_count(buffer);
    let view = program.execution_words().unwrap();
    let other_view = shared.execution_words().unwrap();
    assert_eq!(size_of::<QuickOp>(), 8);
    assert_eq!(Rc::strong_count(buffer), owners);
    assert!(std::ptr::eq(view, other_view));
    assert_eq!(view[0].tag(), tag::PUSH_I32);
    assert_eq!(view[0].i32_operand(), 42);
    assert_eq!(view[1].tag(), tag::GET_LOCAL);
    assert_eq!(view[1].slot(), u16::MAX);
    assert_eq!(view[2].tag(), tag::GENERIC_CANONICAL);
    assert!(view.get(3).is_none());
    let cold = QuickProgram::build_verified(&[Instruction::ReturnUndefined]).unwrap();
    assert!(cold.execution_words().is_none());
}
