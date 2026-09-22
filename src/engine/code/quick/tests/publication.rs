use super::*;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::HeapError;

#[test]
fn quick_snapshots_and_closures_share_one_owner_free_buffer() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let function = context.compile("42").unwrap();
    let id = function.bytecode_id();
    let first_closure = runtime
        .new_bytecode_closure(context.realm, &function)
        .unwrap();
    let second_closure = runtime
        .new_bytecode_closure(context.realm, &function)
        .unwrap();
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .function_bytecode_strong_count(id)
        .unwrap();
    let first = runtime.snapshot_function_bytecode(&function).unwrap();
    let second = runtime.snapshot_function_bytecode(&function).unwrap();
    let first_quick = first.quick.as_ref().unwrap();
    let second_quick = second.quick.as_ref().unwrap();
    assert_eq!(first_quick.validate(&first.code), Ok(()));
    assert!(first_quick.shares_words_with(second_quick));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode_strong_count(id)
            .unwrap(),
        before + 2
    );
    let weak = {
        let state = runtime.0.state.borrow();
        let heap_quick = state
            .heap
            .function_bytecode(id)
            .unwrap()
            .quick
            .as_ref()
            .unwrap();
        assert!(first_quick.shares_words_with(heap_quick));
        let ProgramKind::Words(words) = &heap_quick.0 else {
            panic!("42 has a hot instruction");
        };
        Rc::downgrade(words)
    };
    let other = Runtime::new();
    assert!(matches!(
        other.snapshot_function_bytecode(&function),
        Err(RuntimeError::WrongRuntime("function bytecode"))
    ));
    drop(first);
    drop(second);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode_strong_count(id)
            .unwrap(),
        before
    );
    drop(function);
    drop(first_closure);
    drop(second_closure);
    assert!(runtime.0.state.borrow().heap.function_bytecode(id).is_err());
    assert!(
        weak.upgrade().is_none(),
        "sidecar must die with the last snapshot and bytecode owner"
    );
}

#[test]
fn quick_publication_failure_cleans_atoms_constants_and_already_published_children() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let baseline = runtime.heap_counts();
    let baseline_atoms = runtime.test_atom_count();
    let baseline_realm_owners = runtime
        .0
        .state
        .borrow()
        .heap
        .context_strong_count(context.realm)
        .unwrap();
    // The leaf and the root each contain a hot integer instruction. Failing
    // the second reservation exercises cleanup after child publication and
    // after root atoms/constants have been materialized by the real publisher.
    let source = "function quickFailureChild(value) { return 'quickFailureChildLiteral' + value + 77; } 'quickFailureRootLiteral'; 42;";
    for successful_reservations in [0, 1] {
        let draft = compile_unlinked_script(source).unwrap();
        let failure = reservation_failure::after(successful_reservations);
        assert!(matches!(
            runtime.publish_unlinked_function(context.realm, draft),
            Err(RuntimeError::Heap(HeapError::Allocation {
                operation: "reserving QuickOp words"
            }))
        ));
        drop(failure);
        let after = runtime.heap_counts();
        assert_eq!(
            after.function_bytecode_nodes,
            baseline.function_bytecode_nodes
        );
        assert_eq!(after.string_nodes, baseline.string_nodes);
        assert_eq!(after.bigint_nodes, baseline.bigint_nodes);
        assert_eq!(after.object_nodes, baseline.object_nodes);
        assert_eq!(after.live, baseline.live);
        assert_eq!(after.initializing, 0);
        assert_eq!(runtime.test_atom_count(), baseline_atoms);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .context_strong_count(context.realm)
                .unwrap(),
            baseline_realm_owners
        );
    }
    // The hook is one-shot and its guard also resets on scope exit.
    let function = runtime
        .publish_unlinked_function(context.realm, compile_unlinked_script(source).unwrap())
        .unwrap();
    let snapshot = runtime.snapshot_function_bytecode(&function).unwrap();
    assert_eq!(
        snapshot.quick.as_ref().unwrap().validate(&snapshot.code),
        Ok(())
    );
}

#[test]
fn quick_all_cold_publication_is_certified_without_reserving_words() {
    use crate::engine::code::function::metadata::FunctionMetadata;
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let draft = UnlinkedFunction::fixture(
        vec![Instruction::ReturnUndefined],
        vec![],
        FunctionMetadata::default(),
    );
    let failure = reservation_failure::after(0);
    let function = runtime
        .publish_unlinked_function(context.realm, draft)
        .unwrap();
    let snapshot = runtime.snapshot_function_bytecode(&function).unwrap();
    let quick = snapshot.quick.as_ref().unwrap();
    assert!(matches!(quick.0, ProgramKind::CanonicalOnly));
    assert_eq!(quick.validate(&snapshot.code), Ok(()));
    assert!(
        matches!(
            QuickProgram::build_verified(&[Instruction::Nop]),
            Err(BuildError::Allocation(_))
        ),
        "cold publication must not consume the reservation hook"
    );
    drop(failure);
}

#[test]
fn quick_synthetic_fixture_selects_explicit_canonical_mode() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let mut fixture =
        crate::engine::code::runtime::PublishedFunctionSnapshot::empty_for_test(context.realm);
    assert!(fixture.quick.is_none());
    fixture.code = Rc::from([Instruction::PushI32(9), Instruction::Return]);
    assert!(fixture.quick.is_none());
    assert!(fixture.bytecode_id().is_none());
}

#[test]
#[should_panic(expected = "synthetic canonical-only fixtures are not publication certificates")]
fn quick_synthetic_fixture_cannot_create_a_publication_certificate() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let fixture =
        crate::engine::code::runtime::PublishedFunctionSnapshot::empty_for_test(context.realm);
    fixture.authentication(0);
}
