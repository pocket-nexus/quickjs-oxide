use super::*;
use crate::engine::atom::AtomError;
use crate::engine::code::function::metadata::{ClosureSource, ClosureVariableName};
use crate::engine::heap::{ObjectId, RawId};
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;

fn descriptor(is_lexical: bool, is_const: bool, kind: ClosureVariableKind) -> ClosureVariable {
    ClosureVariable {
        source: ClosureSource::ParentLocal(0),
        name: ClosureVariableName::None,
        is_lexical,
        is_const,
        kind,
    }
}

#[test]
fn captured_state_adopts_reads_replaces_and_resets_object_edges() {
    let runtime = Runtime::new();
    let previous = runtime.new_object(None).unwrap().into_handle();
    let replacement = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Object(previous),
            true,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
    assert_eq!(state.heap.object_strong_count(previous), Ok(1));
    let read = state.read_var_ref(cell).unwrap();
    assert_eq!(read, JsValue::Object(previous));
    assert_eq!(state.heap.object_strong_count(previous), Ok(2));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, read)
        .unwrap();
    state
        .write_var_ref(&runtime.0.poisoned, cell, JsValue::Object(replacement))
        .unwrap();
    assert!(state.heap.object(previous).is_err());
    assert_eq!(state.heap.object_strong_count(replacement), Ok(1));
    state
        .reset_var_ref_uninitialized(&runtime.0.poisoned, cell)
        .unwrap();
    assert!(state.heap.object(replacement).is_err());
    assert!(matches!(
        state.raw_var_ref_value(cell).unwrap(),
        RawValue::Uninitialized
    ));
    state.release_var_ref_handle(cell).unwrap();
    assert!(state.heap.var_ref(cell).is_err());
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn captured_state_leaf_and_symbol_reads_duplicate_exactly_one_edge() {
    let runtime = Runtime::new();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let string = state
        .heap
        .allocate_string(JsString::from_static("cell"))
        .unwrap();
    let bigint = state.heap.allocate_bigint(JsBigInt::one()).unwrap();
    for (raw, value) in [
        (RawId::String(string), JsValue::String(string)),
        (RawId::BigInt(bigint), JsValue::BigInt(bigint)),
    ] {
        let cell = state
            .new_var_ref(
                &runtime.0.poisoned,
                value,
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap();
        assert_eq!(state.heap.strong_count(raw), Ok(1));
        let read = state.read_var_ref(cell).unwrap();
        assert_eq!(state.heap.strong_count(raw), Ok(2));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, read)
            .unwrap();
        state.release_var_ref_handle(cell).unwrap();
        assert!(state.heap.strong_count(raw).is_err());
    }
    let atom = state.atoms.new_symbol(Some("cell")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Symbol(index),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
    let read = state.read_var_ref(cell).unwrap();
    assert_eq!(read, JsValue::Symbol(index));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, read)
        .unwrap();
    state
        .reset_var_ref_uninitialized(&runtime.0.poisoned, cell)
        .unwrap();
    assert!(state.atoms.resolve(atom).is_err());
    state.release_var_ref_handle(cell).unwrap();
}

#[test]
fn captured_state_checked_read_overflow_is_recoverable() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Object(object),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object), u32::MAX);
    assert!(matches!(
        state.read_var_ref(cell),
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(state.heap.object_strong_count(object), Ok(u32::MAX));
    assert!(!runtime.is_poisoned());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object), 1);
    state.release_var_ref_handle(cell).unwrap();

    let atom = state.atoms.new_symbol(None).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Symbol(index),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    assert!(matches!(
        state.read_var_ref(cell),
        Err(RuntimeError::Atom(AtomError::RefCountOverflow(_)))
    ));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(u32::MAX));
    assert!(!runtime.is_poisoned());
    state.atoms.set_ref_count_for_test(index, 1);
    state.release_var_ref_handle(cell).unwrap();
}

#[test]
fn captured_state_keeps_tdz_and_private_storage_distinct() {
    let runtime = Runtime::new();
    let rejected = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let ordinary = state.new_uninitialized_var_ref().unwrap();
    assert_eq!(
        state.read_var_ref(ordinary),
        Err(RuntimeError::Invariant(
            "internal value sentinel cannot become a runtime root"
        ))
    );
    state
        .write_var_ref(&runtime.0.poisoned, ordinary, JsValue::Int(7))
        .unwrap();
    assert_eq!(state.read_var_ref(ordinary).unwrap(), JsValue::Int(7));
    state.release_var_ref_handle(ordinary).unwrap();

    let atom = state.atoms.new_private_symbol(Some("field")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    let private = state
        .heap
        .allocate_var_ref_owned(VarRefData::captured(
            RawValue::Private(index),
            true,
            true,
            ClosureVariableKind::PrivateField,
        ))
        .unwrap();
    assert_eq!(
        state.read_var_ref(private),
        Err(RuntimeError::Invariant(
            "ordinary VarRef read reached a private-element binding"
        ))
    );
    assert!(
        matches!(state.raw_var_ref_value(private).unwrap(), RawValue::Private(value) if value == index)
    );
    assert_eq!(
        state.write_var_ref(&runtime.0.poisoned, private, JsValue::Object(rejected)),
        Err(RuntimeError::Invariant(
            "ordinary VarRef write reached a private-element binding"
        ))
    );
    assert!(state.heap.object(rejected).is_err());
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
    state
        .reset_var_ref_uninitialized(&runtime.0.poisoned, private)
        .unwrap();
    assert!(state.atoms.resolve(atom).is_err());
    assert_eq!(
        state.read_var_ref(private),
        Err(RuntimeError::Invariant(
            "ordinary VarRef read reached a private-element binding"
        ))
    );
    state.release_var_ref_handle(private).unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn captured_state_metadata_preserves_import_and_function_name_views() {
    let runtime = Runtime::new();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state.new_uninitialized_var_ref().unwrap();
    state
        .validate_var_ref_metadata(
            cell,
            descriptor(true, true, ClosureVariableKind::ModuleImportView),
        )
        .unwrap();
    assert!(
        state
            .validate_var_ref_metadata(cell, descriptor(true, false, ClosureVariableKind::Normal))
            .is_err()
    );
    state
        .set_var_ref_metadata(cell, false, true, ClosureVariableKind::FunctionName)
        .unwrap();
    state
        .validate_var_ref_metadata(cell, descriptor(false, false, ClosureVariableKind::Normal))
        .unwrap();
    state
        .write_var_ref(&runtime.0.poisoned, cell, JsValue::Int(3))
        .unwrap();
    assert!(
        state
            .set_var_ref_metadata(cell, true, true, ClosureVariableKind::PrivateField)
            .is_err()
    );
    let data = state.heap.var_ref(cell).unwrap();
    assert_eq!(
        (data.is_lexical, data.is_const, data.kind),
        (false, true, ClosureVariableKind::FunctionName)
    );
    assert!(matches!(data.value, RawValue::Int(3)));
    state.release_var_ref_handle(cell).unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn captured_state_allocation_rejection_consumes_input_without_poisoning() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().var_ref_nodes;
    assert_eq!(
        state.new_var_ref(
            &runtime.0.poisoned,
            JsValue::Object(object),
            true,
            true,
            ClosureVariableKind::ModuleImportView,
        ),
        Err(RuntimeError::Heap(HeapError::Invariant(
            "module-import view escaped into a VarRef cell"
        )))
    );
    assert!(state.heap.object(object).is_err());
    assert_eq!(state.heap.counts().var_ref_nodes, before);
    assert!(
        state
            .new_uninitialized_captured_var_ref(false, false, ClosureVariableKind::PrivateField)
            .is_err()
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn captured_state_rejected_allocation_and_write_report_cleanup_failure_first() {
    for allocation in [false, true] {
        let runtime = Runtime::new();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let rejected = ObjectId {
            index: 1_000_000,
            generation: 1,
        };
        let result = if allocation {
            state
                .new_var_ref(
                    &runtime.0.poisoned,
                    JsValue::Object(rejected),
                    true,
                    true,
                    ClosureVariableKind::ModuleImportView,
                )
                .map(|_| ())
        } else {
            state.write_var_ref(
                &runtime.0.poisoned,
                VarRefId {
                    index: 1_000_001,
                    generation: 1,
                },
                JsValue::Object(rejected),
            )
        };
        assert_eq!(
            result,
            Err(RuntimeError::Heap(HeapError::Stale {
                index: rejected.index,
                generation: rejected.generation,
            }))
        );
        assert!(runtime.is_poisoned());
    }
}

#[test]
fn captured_state_write_and_reset_quarantine_after_published_cleanup_failure() {
    for reset in [false, true] {
        let runtime = Runtime::new();
        let old = runtime.new_object(None).unwrap().into_handle();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let cell = state
            .new_var_ref(
                &runtime.0.poisoned,
                JsValue::Object(old),
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap();
        state.heap.set_strong_count_for_test(RawId::Object(old), 0);
        let result = if reset {
            state.reset_var_ref_uninitialized(&runtime.0.poisoned, cell)
        } else {
            state.write_var_ref(&runtime.0.poisoned, cell, JsValue::Int(42))
        };
        assert!(matches!(result, Err(RuntimeError::Heap(_))));
        assert!(runtime.is_poisoned());
        let value = state.raw_var_ref_value(cell).unwrap();
        assert!(if reset {
            matches!(value, RawValue::Uninitialized)
        } else {
            matches!(value, RawValue::Int(42))
        });
        // This fixture inspects the committed storage only. Quarantine leaves
        // both the detached invalid edge and the cell without further cleanup.
    }
}

#[test]
fn captured_state_invalid_reset_is_recoverable_before_publication() {
    let runtime = Runtime::new();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state.new_uninitialized_var_ref().unwrap();
    let invalid = VarRefId {
        generation: cell.generation + 1,
        ..cell
    };
    assert!(
        state
            .reset_var_ref_uninitialized(&runtime.0.poisoned, invalid)
            .is_err()
    );
    assert!(!runtime.is_poisoned());
    assert!(matches!(
        state.raw_var_ref_value(cell).unwrap(),
        RawValue::Uninitialized
    ));
    state.release_var_ref_handle(cell).unwrap();
}

#[test]
fn captured_state_operations_leave_external_deferred_cleanup_pending() {
    let runtime = Runtime::new();
    let deferred = runtime.new_object(None).unwrap();
    let deferred_id = deferred.object_id();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    drop(deferred);
    assert!(runtime.0.deferred_references.has_pending());
    let cell = state.new_uninitialized_var_ref().unwrap();
    state
        .write_var_ref(&runtime.0.poisoned, cell, JsValue::Int(5))
        .unwrap();
    assert_eq!(state.read_var_ref(cell).unwrap(), JsValue::Int(5));
    state
        .reset_var_ref_uninitialized(&runtime.0.poisoned, cell)
        .unwrap();
    state.release_var_ref_handle(cell).unwrap();
    assert_eq!(state.heap.object_strong_count(deferred_id), Ok(1));
    assert!(runtime.0.deferred_references.has_pending());
    drop(state);
    runtime.drain_deferred_references().unwrap();
    assert!(runtime.0.state.borrow().heap.object(deferred_id).is_err());
}

#[test]
fn captured_boundary_metadata_and_reset_keep_domain_priority_and_poison_admission() {
    let runtime = Runtime::new();
    let foreign_runtime = Runtime::new();
    let local = runtime.new_uninitialized_var_ref().unwrap();
    let foreign = foreign_runtime.new_uninitialized_var_ref().unwrap();
    runtime.0.poisoned.set(true);
    assert_eq!(
        runtime.set_var_ref_metadata(&local, true, false, ClosureVariableKind::Normal),
        Err(RuntimeError::Poisoned)
    );
    assert_eq!(
        runtime.reset_var_ref_uninitialized(&local),
        Err(RuntimeError::Poisoned)
    );
    assert_eq!(
        runtime.set_var_ref_metadata(&foreign, true, false, ClosureVariableKind::Normal),
        Err(RuntimeError::WrongRuntime("closure variable"))
    );
    assert_eq!(
        runtime.reset_var_ref_uninitialized(&foreign),
        Err(RuntimeError::WrongRuntime("closure variable"))
    );
    assert!(!foreign_runtime.is_poisoned());
}

#[test]
fn captured_state_reset_drains_older_zero_work_before_detached_atoms() {
    let runtime = Runtime::new();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.new_symbol(Some("detached")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Symbol(index),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(first))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    // Corrupt only the first queued record's counter. Its failure must stop
    // both later heap work and the detached cell atom's cleanup.
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 1);
    assert_eq!(
        state.reset_var_ref_uninitialized(&runtime.0.poisoned, cell),
        Err(RuntimeError::Heap(HeapError::Invariant(
            "finalization count disagrees with its queue/cycle state",
        )))
    );
    assert!(runtime.is_poisoned());
    assert!(matches!(
        state.raw_var_ref_value(cell).unwrap(),
        RawValue::Uninitialized
    ));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert_eq!(state.heap.zero_queue.front(), Some(&RawId::Object(later)));
}

#[test]
fn captured_state_reset_immediate_value_still_drains_older_zero_work() {
    let runtime = Runtime::new();
    let older = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Int(7),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(older))
        .unwrap();
    state
        .reset_var_ref_uninitialized(&runtime.0.poisoned, cell)
        .unwrap();
    assert!(state.heap.object(older).is_err());
    assert!(state.heap.zero_queue.is_empty());
    state.release_var_ref_handle(cell).unwrap();
}

#[test]
fn captured_boundary_wrong_domain_write_consumes_input_and_keeps_cleanup_precedence() {
    for cleanup_failure in [false, true] {
        let runtime = Runtime::new();
        let foreign_runtime = Runtime::new();
        let foreign = foreign_runtime.new_uninitialized_var_ref().unwrap();
        let input = runtime.new_object(None).unwrap().into_handle();
        if cleanup_failure {
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(input), 0);
        }
        let result = runtime.write_var_ref(&foreign, JsValue::Object(input));
        if cleanup_failure {
            assert!(matches!(result, Err(RuntimeError::Heap(_))));
            assert!(runtime.is_poisoned());
        } else {
            assert_eq!(result, Err(RuntimeError::WrongRuntime("closure variable")));
            assert!(runtime.0.state.borrow().heap.object(input).is_err());
            assert!(!runtime.is_poisoned());
        }
        assert!(!foreign_runtime.is_poisoned());
        assert!(matches!(
            foreign_runtime.raw_var_ref_value(&foreign).unwrap(),
            RawValue::Uninitialized
        ));
    }
}
