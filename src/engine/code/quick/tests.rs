use super::*;
use crate::engine::code::function::UnlinkedFunction;
use crate::engine::code::verify::verify_unlinked_tree;
use crate::engine::compiler::compile_unlinked_script;

mod generic_cases;

fn words_for(code: &[Instruction]) -> Vec<QuickOp> {
    code.iter().map(translate_instruction).collect()
}

#[test]
fn quick_word_layout_and_integer_boundaries_are_bit_exact() {
    assert_eq!(std::mem::size_of::<QuickOp>(), 8);
    assert!(!std::mem::needs_drop::<QuickOp>());
    for value in [i32::MIN, i32::MIN + 1, -65_536, -1, 0, 1, 65_535, i32::MAX] {
        let word = translate_instruction(&Instruction::PushI32(value));
        assert_eq!(word.decode(), Ok(DecodedOp::PushI32(value)));
        assert_eq!(word.0 & RESERVED_MASK, 0);
        assert_eq!(word.0 & 0xff, 2);
        assert_eq!(
            word.0 >> 32,
            u64::from(u32::from_le_bytes(value.to_le_bytes()))
        );
        // Codec uses integer positions, not the layout of a native byte array.
        assert_eq!(QuickOp(u64::from_le_bytes(word.0.to_le_bytes())), word);
        assert_eq!(QuickOp(u64::from_be_bytes(word.0.to_be_bytes())), word);
    }
    assert_eq!(
        translate_instruction(&Instruction::PushI32(-1)).0,
        0xffff_ffff_0000_0002
    );
    assert_eq!(
        translate_instruction(&Instruction::PushI32(i32::MIN)).0,
        0x8000_0000_0000_0002
    );
    for target in [0, u32::from(u16::MAX), u32::from(u16::MAX) + 1, u32::MAX] {
        let word = translate_instruction(&Instruction::Goto(target));
        assert_eq!(word.decode(), Ok(DecodedOp::Goto(target)));
        assert_eq!(word.0 >> 32, u64::from(target));
    }
}

#[test]
fn quick_hot_tags_and_contracts_match_the_canonical_operations() {
    let cases = [
        (Instruction::Nop, DecodedOp::Nop, 0x01),
        (
            Instruction::PushI32(7),
            DecodedOp::PushI32(7),
            0x0000_0007_0000_0002,
        ),
        (Instruction::Undefined, DecodedOp::Undefined, 0x03),
        (Instruction::Null, DecodedOp::Null, 0x04),
        (Instruction::PushFalse, DecodedOp::Bool(false), 0x05),
        (
            Instruction::PushTrue,
            DecodedOp::Bool(true),
            0x0000_0001_0000_0005,
        ),
        (
            Instruction::Goto(7),
            DecodedOp::Goto(7),
            0x0000_0007_0000_0006,
        ),
    ];
    for (instruction, decoded, raw) in cases {
        let word = translate_instruction(&instruction);
        assert_eq!(word.0, raw);
        assert_eq!(word.decode(), Ok(decoded));
        assert!(contracts_match(decoded, &instruction), "{instruction:?}");
        assert_eq!(validate_words(&[instruction], &[word]), Ok(()));
    }
    assert!(!contracts_match(DecodedOp::Nop, &Instruction::Drop));
    assert!(!contracts_match(
        DecodedOp::PushI32(0),
        &Instruction::PushI32(1)
    ));
    assert!(!contracts_match(DecodedOp::Goto(2), &Instruction::Goto(3)));
    assert!(!contracts_match(
        DecodedOp::Goto(2),
        &Instruction::IfTrue(2)
    ));
    assert!(!contracts_match(
        DecodedOp::Undefined,
        &Instruction::PushThis
    ));
}

#[test]
fn quick_goto_preserves_the_canonical_exception_contract() {
    let instruction = Instruction::Goto(0);
    assert_eq!(
        instruction.potential_effects(),
        PotentialEffects {
            javascript_exception: JsExceptionEffect::MayThrow,
            may_call_js: false,
            may_allocate: false,
        }
    );
    assert_eq!(
        instruction.stack_contract(),
        StackEffect {
            popped: 0,
            pushed: 0,
            state: StackStateEffect::Ordinary,
        }
    );
    assert_eq!(instruction.control_effect(), ControlEffect::Jump(0));
    assert_eq!(
        instruction.operand_contract(),
        OperandContract([Some(Operand::Target(0)), None, None])
    );
    assert!(contracts_match(DecodedOp::Goto(0), &instruction));
    let program = QuickProgram::build(&[instruction]).unwrap();
    assert_eq!(program.validate(&[Instruction::Goto(0)]), Ok(()));
    assert_eq!(
        program.validate(&[Instruction::Goto(1)]),
        Err(ValidationError::CanonicalMismatch { pc: 0 })
    );
}

#[test]
fn quick_decoder_rejects_every_reserved_bit_and_unknown_tag() {
    for instruction in [
        Instruction::Return,
        Instruction::Nop,
        Instruction::PushI32(i32::MIN),
        Instruction::Undefined,
        Instruction::Null,
        Instruction::PushTrue,
        Instruction::Goto(u32::MAX),
    ] {
        let word = translate_instruction(&instruction);
        for bit in 8..32 {
            assert_eq!(
                QuickOp(word.0 | (1_u64 << bit)).decode(),
                Err(DecodeError::ReservedBits)
            );
        }
    }
    for unknown in 7_u8..=u8::MAX {
        assert_eq!(
            QuickOp(u64::from(unknown)).decode(),
            Err(DecodeError::UnknownTag(unknown))
        );
    }
}

#[test]
fn quick_decoder_rejects_noncanonical_unused_and_boolean_operands() {
    for tag in [
        QuickTag::GenericCanonical,
        QuickTag::Nop,
        QuickTag::Undefined,
        QuickTag::Null,
    ] {
        for operand in [1, u32::from(u16::MAX), u32::MAX] {
            assert_eq!(
                QuickOp::pack(tag, operand).decode(),
                Err(DecodeError::UnexpectedOperand)
            );
        }
    }
    for operand in [2, 255, 65_536, u32::MAX] {
        assert_eq!(
            QuickOp::pack(QuickTag::Bool, operand).decode(),
            Err(DecodeError::InvalidBoolean(operand))
        );
    }
}

#[test]
fn quick_validation_rejects_length_and_reports_the_exact_bad_pc() {
    let code = [
        Instruction::Nop,
        Instruction::PushI32(8),
        Instruction::Return,
    ];
    let words = words_for(&code);
    for len in [0, 1, 2] {
        assert_eq!(
            validate_words(&code, &words[..len]),
            Err(ValidationError::Length {
                canonical: 3,
                quick: len
            })
        );
    }
    let mut extra = words.clone();
    extra.push(translate_instruction(&Instruction::Nop));
    assert_eq!(
        validate_words(&code, &extra),
        Err(ValidationError::Length {
            canonical: 3,
            quick: 4
        })
    );
    let mut bad = words.clone();
    bad[1] = QuickOp(bad[1].0 | 0x100);
    assert_eq!(
        validate_words(&code, &bad),
        Err(ValidationError::InvalidWord {
            pc: 1,
            reason: DecodeError::ReservedBits
        })
    );
    bad[1] = QuickOp(255);
    assert_eq!(
        validate_words(&code, &bad),
        Err(ValidationError::InvalidWord {
            pc: 1,
            reason: DecodeError::UnknownTag(255)
        })
    );
    bad[1] = translate_instruction(&Instruction::PushI32(9));
    assert_eq!(
        validate_words(&code, &bad),
        Err(ValidationError::CanonicalMismatch { pc: 1 })
    );
}

#[test]
fn quick_validation_rejects_wrong_read_only_tags_and_parameters() {
    let substitutions = [
        (Instruction::Null, Instruction::Undefined),
        (Instruction::PushFalse, Instruction::PushTrue),
        (Instruction::Goto(0), Instruction::Goto(u32::MAX)),
        (
            Instruction::PushI32(i32::MIN),
            Instruction::PushI32(i32::MAX),
        ),
        (Instruction::Return, Instruction::Nop),
        (Instruction::Nop, Instruction::Return),
    ];
    for (source, replacement) in substitutions {
        assert_eq!(
            validate_words(&[source], &[translate_instruction(&replacement)]),
            Err(ValidationError::CanonicalMismatch { pc: 0 })
        );
    }
    let code = [
        Instruction::PushI32(1),
        Instruction::PushI32(2),
        Instruction::Goto(0),
    ];
    let mut words = words_for(&code);
    words.swap(0, 1);
    assert_eq!(
        validate_words(&code, &words),
        Err(ValidationError::CanonicalMismatch { pc: 0 })
    );
}

#[test]
fn quick_all_complex_variants_and_wide_operands_stay_generic() {
    let code = generic_cases::instructions();
    assert_eq!(
        code.len(),
        191,
        "review the initial hot/Generic partition when the enum changes"
    );
    for instruction in &code {
        let word = translate_instruction(instruction);
        assert_eq!(word, QuickOp(0), "{instruction:?}");
        assert_eq!(word.decode(), Ok(DecodedOp::GenericCanonical));
    }
    let program = QuickProgram::build(&code).unwrap();
    assert!(matches!(program, QuickProgram::CanonicalOnly));
    assert_eq!(program.validate(&code), Ok(()));

    // Introducing one hot PC keeps every complex instruction at its original
    // index; even maximal operands need no extra table or truncated encoding.
    let mut mixed = code;
    mixed.insert(0, Instruction::PushI32(0));
    let program = QuickProgram::build(&mixed).unwrap();
    let words = program.words().unwrap();
    assert_eq!(words.len(), mixed.len());
    assert!(words[1..].iter().all(|word| *word == QuickOp(0)));
    assert_eq!(program.validate(&mixed), Ok(()));
}

#[test]
fn quick_canonical_only_is_a_checked_mode_not_a_missing_buffer() {
    for code in [
        vec![],
        vec![Instruction::ReturnUndefined],
        vec![Instruction::Add, Instruction::Return],
    ] {
        let program = QuickProgram::build(&code).unwrap();
        assert!(matches!(program, QuickProgram::CanonicalOnly));
        assert!(program.words().is_none());
        assert_eq!(program.validate(&code), Ok(()));
    }
    assert_eq!(
        QuickProgram::CanonicalOnly.validate(&[Instruction::Return, Instruction::Nop]),
        Err(ValidationError::HotInstructionInCanonicalOnly { pc: 1 })
    );
    assert_eq!(
        QuickProgram::Words(Rc::new(Vec::new())).validate(&[Instruction::Nop]),
        Err(ValidationError::Length {
            canonical: 1,
            quick: 0
        })
    );
}

#[test]
fn quick_words_preserve_branch_and_fusion_interior_pcs() {
    // The method fusion may skip PCs 1 and 2, but each remains independently
    // readable. A branch's operand is the canonical target, never a span rank.
    let code = [
        Instruction::GetField2(9),
        Instruction::PushI32(123),
        Instruction::CallMethod(1),
        Instruction::Goto(1),
        Instruction::Return,
    ];
    let program = QuickProgram::build(&code).unwrap();
    let words = program.words().unwrap();
    assert_eq!(words.len(), 5);
    assert_eq!(words[0].decode(), Ok(DecodedOp::GenericCanonical));
    assert_eq!(words[1].decode(), Ok(DecodedOp::PushI32(123)));
    assert_eq!(words[2].decode(), Ok(DecodedOp::GenericCanonical));
    assert_eq!(words[3].decode(), Ok(DecodedOp::Goto(1)));
    assert_eq!(program.validate(&code), Ok(()));
}

#[test]
fn quick_shared_buffer_has_one_linear_allocation_and_no_owner_words() {
    for len in [1, 16, 1_024, 65_536] {
        let code = vec![Instruction::Nop; len];
        let program = QuickProgram::build(&code).unwrap();
        assert_eq!(program.words().unwrap().len(), len);
        assert_eq!(program.validate(&code), Ok(()));
        let copy = program.clone();
        let (QuickProgram::Words(original), QuickProgram::Words(shared)) = (&program, &copy) else {
            panic!("hot functions need complete word buffers");
        };
        assert!(Rc::ptr_eq(original, shared));
        assert_eq!(original.as_ptr(), shared.as_ptr());
        assert_eq!(Rc::strong_count(original), 2);
        // try_reserve_exact requests precisely N words. The allocator may
        // grant extra capacity; accounting must report that real capacity.
        assert!(original.capacity() >= len);
        assert!(
            original
                .capacity()
                .checked_mul(std::mem::size_of::<QuickOp>())
                .is_some()
        );
        drop(copy);
        assert_eq!(Rc::strong_count(original), 1);
    }
}

#[test]
fn quick_word_buffer_capacity_failure_is_recoverable_without_allocating() {
    match reserve_words(usize::MAX).unwrap_err() {
        BuildError::Allocation(error) => assert!(!error.to_string().is_empty()),
        BuildError::InvalidProjection(error) => {
            panic!("reservation failed with a projection error: {error:?}");
        }
    }
}

fn check_compiled_tree(function: &UnlinkedFunction, counts: &mut [usize; 3]) {
    let code = function.code();
    let program = QuickProgram::build(code).unwrap();
    assert_eq!(program.validate(code), Ok(()));
    counts[0] += 1;
    counts[1] += code.len();
    if let Some(words) = program.words() {
        assert_eq!(words.len(), code.len());
        for (instruction, word) in code.iter().zip(words) {
            let decoded = word.decode().unwrap();
            assert!(contracts_match(decoded, instruction));
            if let DecodedOp::Goto(target) = decoded {
                assert_eq!(instruction.control_effect(), ControlEffect::Jump(target));
                assert!(usize::try_from(target).unwrap() < code.len());
                counts[2] += 1;
            }
        }
    }
    for child in function
        .constants()
        .iter()
        .filter_map(|constant| constant.as_child())
    {
        check_compiled_tree(child, counts);
    }
}

#[test]
fn quick_verified_compiler_results_validate_without_runtime_or_heap() {
    let sources = [
        "42; -2147483648; 2147483647; null; true; false; void 0;",
        "function choose(x) { if (x) return 1; return 2; } choose(false);",
        "function step(x) { for (var i = 0; i < 3; i++) x += i; return x; } step(0);",
        "function use(o) { return o.m(1, true, null); } use({ m(a,b,c) { return a; } });",
        "function attempt(x) { try { if (x) throw 7; return 3; } catch(e) { return e; } finally { x = 0; } } attempt(false);",
        "function* items() { yield 1; yield* [2, 3]; } items();",
        "async function result() { await 1; return 2; } result();",
        "class Parent { m() { return 1; } } class Child extends Parent { #x = 3; m() { return super.m() + this.#x; } } new Child().m();",
        "function direct(x) { return eval('x + 1'); } direct(1);",
        "function destructure({x, ...rest}, ...tail) { return [x, rest, tail]; } destructure({x: 1, y: 2}, 3);",
    ];
    let mut counts = [0; 3];
    for source in sources {
        let function =
            compile_unlinked_script(source).unwrap_or_else(|error| panic!("{source}: {error}"));
        verify_unlinked_tree(&function).unwrap_or_else(|error| panic!("{source}: {error}"));
        check_compiled_tree(&function, &mut counts);
    }
    assert!(
        counts[0] > sources.len(),
        "nested functions must also be checked"
    );
    assert!(counts[1] > 100);
    assert!(
        counts[2] > 0,
        "real canonical branch targets must be exercised"
    );
}
