use super::*;
use Instruction::*;

fn local() -> VariableDefinition {
    VariableDefinition {
        name: None,
        is_lexical: false,
        is_const: false,
        is_parameter_initializer: false,
        kind: ClosureVariableKind::Normal,
    }
}

#[test]
fn store_drop_certifies_only_its_canonical_start_and_keeps_compact_storage() {
    for (store, target) in [(SetLocal(0), StoreDrop::Local), (SetArg(7), StoreDrop::Argument)] {
        let code = [PushI32(42), store, Drop, ReturnUndefined];
        let plan = FusionPlan::build(&code, &[local()]);
        assert_eq!(plan.store_drop(1), Some(target));
        for pc in [0, 2, 3, 4, usize::MAX] {
            assert_eq!(plan.store_drop(pc), None);
        }
        assert_eq!(plan.0.as_ref().unwrap().len(), code.len());
        assert_eq!(plan.0.as_ref().unwrap().iter().filter(|&&tag| tag != 0).count(), 1);
        assert!(plan.update(1).is_none());
        assert!(!plan.compare_branch(1));
        assert!(!plan.add_store(1));
        assert!(plan.local_add_span(1).is_none());
        assert!(plan.const_add_span(1).is_none());
        assert!(plan.method_call(1).is_none());
    }
    assert_eq!(std::mem::size_of::<FusionPlan>(), std::mem::size_of::<Option<Rc<[u8]>>>());
}

#[test]
fn store_drop_rejects_non_normal_const_invalid_and_non_adjacent_locals() {
    let code = [SetLocal(0), Drop, ReturnUndefined];
    assert!(FusionPlan::build(&code, &[]).0.is_none());
    let mut definition = local();
    definition.is_const = true;
    assert!(FusionPlan::build(&code, &[definition]).0.is_none());
    for kind in [
        ClosureVariableKind::ModuleImportView,
        ClosureVariableKind::FunctionName,
        ClosureVariableKind::GlobalFunction,
        ClosureVariableKind::EvalVariableObject,
        ClosureVariableKind::ArgEvalVariableObject,
        ClosureVariableKind::WithObject,
        ClosureVariableKind::PrivateField,
        ClosureVariableKind::PrivateMethod,
        ClosureVariableKind::PrivateGetter,
    ] {
        let mut definition = local();
        definition.kind = kind;
        assert!(FusionPlan::build(&code, &[definition]).0.is_none(), "{kind:?}");
    }
    for code in [
        vec![SetLocal(u16::MAX), Drop],
        vec![SetLocal(0)],
        vec![SetArg(0)],
        vec![SetLocal(0), Nop, Drop],
        vec![SetArg(0), Nop, Drop],
        vec![PutLocal(0), Drop],
        vec![PutArg(0), Drop],
        vec![SetLocalCheck(0), Drop],
        vec![Drop, SetLocal(0)],
        vec![Drop],
        vec![],
    ] {
        let plan = FusionPlan::build(&code, &[local()]);
        assert!(plan.0.is_none(), "{code:?}");
        assert_eq!(plan.store_drop(0), None);
    }
}

#[test]
fn store_drop_leaves_tdz_capture_and_mapped_arguments_to_runtime_guards() {
    let mut lexical = local();
    lexical.is_lexical = true;
    let code = [SetLocal(0), Drop];
    assert_eq!(FusionPlan::build(&code, &[lexical]).store_drop(0), Some(StoreDrop::Local));
    // VariableDefinition cannot certify the live binding's Direct/Captured/
    // Uninitialized form. The span is only a structural candidate.
    // Likewise FusionPlan has no parameter layout or mapped-arguments state;
    // argument bounds remain the verifier's responsibility before publication.
    for index in [0, 7, u16::MAX] {
        assert_eq!(FusionPlan::build(&[SetArg(index), Drop], &[]).store_drop(0), Some(StoreDrop::Argument));
    }
}

#[test]
fn store_drop_rejects_every_interior_control_target() {
    for store in [SetLocal(0), SetArg(0)] {
        for entry in [Goto(1), IfTrue(1), IfFalse(1), Catch(1), Gosub(1)] {
            let code = [store.clone(), Drop, entry, ReturnUndefined];
            let plan = FusionPlan::build(&code, &[local()]);
            assert!(plan.0.is_none(), "{code:?}");
            assert_eq!(plan.store_drop(0), None);
        }
        // The same proof applies to an entry discovered before the candidate.
        let code = [Catch(2), store, Drop, ReturnUndefined];
        assert!(FusionPlan::build(&code, &[local()]).0.is_none());
    }
}

#[test]
fn store_drop_accepts_block_start_and_fallthrough_without_crossing_a_terminator() {
    for control in [Goto(1), IfTrue(1), IfFalse(1), Catch(1), Gosub(1), ReturnUndefined, Ret, Throw] {
        assert!(control.control_effect().ends_block());
        // An entry at SetLocal itself is safe: execution starts the whole span.
        let code = [control.clone(), SetLocal(0), Drop, ReturnUndefined];
        assert_eq!(FusionPlan::build(&code, &[local()]).store_drop(1), Some(StoreDrop::Local));
        // A control terminator in the middle destroys the adjacent shape;
        // no candidate may jump over it to find a later Drop.
        let code = [SetLocal(0), control, Drop, ReturnUndefined];
        assert_eq!(FusionPlan::build(&code, &[local()]).store_drop(0), None);
    }
    assert!(!SetLocal(0).control_effect().ends_block());
    assert!(!SetArg(0).control_effect().ends_block());
    let code = [Nop, SetArg(0), Drop, Nop, SetLocal(0), Drop, ReturnUndefined];
    let plan = FusionPlan::build(&code, &[local()]);
    assert_eq!(plan.store_drop(1), Some(StoreDrop::Argument));
    assert_eq!(plan.store_drop(4), Some(StoreDrop::Local));
}

#[test]
fn fusion_kind_roundtrips_only_known_u8_tags_without_cross_classification() {
    for tag in 0..=u8::MAX {
        let known = matches!(tag, 16..=31 | 32 | 64 | 65 | 96 | 97 | 128..=131 | 160..=167);
        let kind = FusionKind::decode(tag);
        assert_eq!(kind.is_some(), known, "tag {tag}");
        if let Some(kind) = kind {
            assert_eq!(kind.encode(), tag);
            assert!((2..=9).contains(&kind.instructions()));
        }
        let plan = FusionPlan(Some(Rc::from([tag])));
        assert_eq!(plan.update(0).is_some(), (16..=31).contains(&tag));
        assert_eq!(plan.store_drop(0).is_some(), matches!(tag, 96 | 97));
    }
}

#[test]
fn fusion_kind_preserves_existing_published_tags_and_extents() {
    for (code, tag, length) in [
        (vec![GetLocal(0), Inc, PutLocal(0)], 21, 3),
        (vec![GetLocal(0), Inc, SetLocal(0)], 17, 3),
        (vec![GetLocal(0), PostInc, PutLocal(0), Drop], 31, 4),
        (vec![GetLocal(0), PostDec, PutLocal(0)], 18, 3),
        (vec![Lt, IfFalse(2), ReturnUndefined], 32, 2),
        (vec![Add, PutLocal(0)], 64, 2),
        (vec![Add, SetLocal(0), Drop], 65, 3),
        (vec![GetLocal(0), GetLocal(1), Add, PutLocal(0)], 128, 4),
        (vec![GetLocal(0), GetLocal(1), Add, SetLocal(0), Drop], 129, 5),
        (vec![PushConst(0), GetLocal(0), Add, PutLocal(0)], 130, 4),
        (vec![PushConst(0), GetLocal(0), Add, SetLocal(0), Drop], 131, 5),
    ] {
        let plan = FusionPlan::build(&code, &[local(), local()]);
        assert_eq!(plan.flag(0), tag, "{code:?}");
        assert_eq!(FusionKind::decode(plan.flag(0)).unwrap().instructions(), length);
    }
    for count in 0..=7 {
        let mut code = vec![GetField2(0)];
        code.extend((0..count).map(|_| PushTrue));
        code.push(CallMethod(count));
        let plan = FusionPlan::build(&code, &[]);
        assert_eq!(plan.flag(0), 160 + count as u8);
        assert_eq!(plan.method_call(0), Some(usize::from(count)));
    }
}
