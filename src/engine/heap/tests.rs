use super::*;
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use crate::source::LineColumn;

// Heap-only fixtures have no AtomTable; symbol liveness stays with each
// fixture, and detached atom ownership is returned in the cleanup record.
fn collect_heap(heap: &mut Heap) -> Result<GcStats, HeapError> {
    heap.run_gc_with_finalization_sink(
        |event| Ok(matches!(event, WeakSymbolGcEvent::IsLive(_))),
        &mut gc::DiscardFinalizationJobSink,
    )
}

const DATA_FLAGS: PropertyFlags = PropertyFlags::data(true, true, true);

#[test]
fn multiset_difference_preserves_left_order_and_occurrence_counts() {
    assert_eq!(
        multiset_difference(
            &[1u8, 2, 1, 3, 2, 1],
            &[2u8, 1, 4, 2],
            "testing multiset difference",
        )
        .unwrap(),
        vec![1, 3, 1]
    );
}

#[test]
fn leaf_arena_keeps_leaf_slots_compact_and_out_of_the_shared_arena() {
    let mut heap = Heap::new();
    assert!(size_of::<LeafSlot>() <= 32);
    let string = heap.allocate_string(JsString::from_static("leaf")).unwrap();
    let bigint = heap.allocate_bigint(JsBigInt::one()).unwrap();
    let counts = heap.counts();
    assert_eq!(counts.string_nodes, 1);
    assert_eq!(counts.bigint_nodes, 1);
    assert_eq!(counts.live, 2);
    assert!(heap.slots.is_empty());
    assert_eq!(heap.leaf_slots.len(), 2);
    assert!(heap.live_node(RawId::String(string)).is_err());
    assert!(heap.validate_slot_identity(RawId::BigInt(bigint)).is_err());
    assert_eq!(heap.string(string).unwrap().to_utf8_lossy(), "leaf");
    assert_eq!(heap.release_string(string).unwrap().finalized_strings, 1);
    assert_eq!(heap.release_bigint(bigint).unwrap().finalized_bigints, 1);
    assert_eq!(heap.counts().live, 0);
    assert_eq!(heap.leaf_free.len(), 2);
}

#[test]
fn captured_cells_use_compact_storage_and_distinct_arena_identity() {
    let mut heap = Heap::new();
    let shape = empty_shape(&mut heap);
    let object = leaf(&mut heap, shape);
    let first = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(1)))
        .unwrap();
    let second = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(2)))
        .unwrap();

    assert!(size_of::<auxiliary_arena::AuxiliarySlot<VarRefData>>() < size_of::<ArenaSlot>());
    assert_eq!(object.debug_index(), second.index);
    assert_eq!(object.debug_generation(), second.generation);
    assert_eq!(
        heap.slots.len(),
        2,
        "shared storage contains only shape and object"
    );
    assert_eq!(heap.var_refs.slots.len(), 2);
    assert!(heap.live_node(RawId::VarRef(second)).is_err());
    assert!(matches!(
        heap.var_ref(second).unwrap().value,
        RawValue::Int(2)
    ));

    let cleanup = heap.release_var_ref(first).unwrap();
    assert_eq!(cleanup.finalized_var_refs, 1);
    let reused = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(3)))
        .unwrap();
    assert_eq!(reused.index, first.index);
    assert_ne!(reused.generation, first.generation);
    assert!(matches!(heap.var_ref(first), Err(HeapError::Stale { .. })));
    assert_eq!(heap.object(object).unwrap().shape, shape);
    assert!(matches!(
        heap.var_ref(second).unwrap().value,
        RawValue::Int(2)
    ));

    heap.release_var_ref(second).unwrap();
    heap.release_var_ref(reused).unwrap();
    heap.release_object(object).unwrap();
    heap.release_shape(shape).unwrap();
    assert_eq!(heap.counts().live, 0);
}

#[test]
fn captured_cell_zero_queue_and_saturated_generation_reclaim() {
    let mut heap = Heap::new();
    let first = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(1)))
        .unwrap();
    let second = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(2)))
        .unwrap();
    heap.release_raw_no_drain(RawId::VarRef(first)).unwrap();
    assert!(matches!(
        heap.var_refs.slots[0].state,
        AuxiliaryState::ZeroQueued(_)
    ));
    let cleanup = heap.release_var_ref(second).unwrap();
    assert_eq!(cleanup.finalized_var_refs, 2);
    assert!(heap.zero_queue.is_empty());

    let third = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(3)))
        .unwrap();
    heap.var_refs.slots[third.index as usize].generation = u32::MAX;
    let saturated = VarRefId {
        index: third.index,
        generation: u32::MAX,
    };
    heap.set_strong_count_for_test(RawId::VarRef(saturated), u32::MAX);
    heap.retain_raw_fast(RawId::VarRef(saturated));
    heap.release_raw_no_drain(RawId::VarRef(saturated)).unwrap();
    assert_eq!(heap.var_ref_strong_count(saturated), Ok(u32::MAX));
    heap.set_strong_count_for_test(RawId::VarRef(saturated), 1);
    heap.release_var_ref(saturated).unwrap();
    assert!(matches!(
        heap.var_refs.slots[third.index as usize].state,
        AuxiliaryState::Retired
    ));
    assert!(matches!(
        heap.var_ref(saturated),
        Err(HeapError::Stale { .. })
    ));
}

#[test]
fn captured_cell_reservation_aborts_after_edge_retain_failure() {
    let mut heap = Heap::new();
    let missing = ObjectId {
        index: 77,
        generation: 1,
    };
    assert!(heap
        .allocate_var_ref(VarRefData::local(RawValue::Object(missing)))
        .is_err());
    assert_eq!(heap.counts().var_ref_nodes, 0);
    assert!(matches!(heap.var_refs.slots[0].state, AuxiliaryState::Vacant));
    assert_eq!(heap.var_refs.free, vec![0]);
    let cell = heap
        .allocate_var_ref(VarRefData::local(RawValue::Int(4)))
        .unwrap();
    assert_eq!(cell.index, 0);
    heap.release_var_ref(cell).unwrap();
}

#[test]
fn captured_cell_replacement_preserves_owned_transfer_and_retained_update() {
    let mut heap = Heap::new();
    let shape = empty_shape(&mut heap);
    let old = leaf(&mut heap, shape);
    let transferred = leaf(&mut heap, shape);
    let retained = leaf(&mut heap, shape);
    let cell = heap
        .allocate_var_ref_owned(VarRefData::local(RawValue::Object(old)))
        .unwrap();
    assert_eq!(heap.object_strong_count(old), Ok(1));

    let previous = heap
        .replace_var_ref_value_owned(cell, RawValue::Object(transferred))
        .unwrap();
    assert!(matches!(previous, RawValue::Object(id) if id == old));
    assert_eq!(heap.object_strong_count(transferred), Ok(1));
    assert_eq!(heap.release_object(old).unwrap().finalized_objects, 1);

    let cleanup = heap
        .replace_var_ref_value(cell, RawValue::Object(retained))
        .unwrap();
    assert_eq!(cleanup.finalized_objects, 1);
    assert_eq!(heap.object_strong_count(retained), Ok(2));
    heap.release_object(retained).unwrap();
    let cleanup = heap.release_var_ref(cell).unwrap();
    assert_eq!(cleanup.finalized_var_refs, 1);
    assert_eq!(cleanup.finalized_objects, 1);
    heap.release_shape(shape).unwrap();
    assert_eq!(heap.counts().live, 0);
}

fn empty_shape(heap: &mut Heap) -> ShapeId {
    heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap()
}

#[test]
fn small_edge_transactions_preflight_duplicates_and_late_failure() {
    let mut heap = Heap::new();
    let first = empty_shape(&mut heap);
    let second = empty_shape(&mut heap);
    let first = RawId::Shape(first);
    let second = RawId::Shape(second);
    heap.retain_edges_transactionally(&[]).unwrap();
    heap.retain_edges_transactionally(&[first]).unwrap();
    assert_eq!(heap.live_node(first).unwrap().strong.get(), 2);
    heap.retain_edges_transactionally(&[first, first]).unwrap();
    assert_eq!(heap.live_node(first).unwrap().strong.get(), 4);
    heap.live_node_mut(second).unwrap().strong.set(u32::MAX);
    assert!(heap.retain_edges_transactionally(&[first, second]).is_err());
    assert_eq!(
        heap.live_node(first).unwrap().strong.get(),
        4,
        "later edge failure must not retain the first"
    );
    heap.live_node_mut(first).unwrap().strong.set(u32::MAX - 1);
    assert!(heap.retain_edges_transactionally(&[first, first]).is_err());
    assert_eq!(heap.live_node(first).unwrap().strong.get(), u32::MAX - 1);
    heap.live_node_mut(first).unwrap().strong.set(1);
    heap.live_node_mut(second).unwrap().strong.set(1);
    heap.retain_edges_transactionally(&[first, second]).unwrap();
    assert_eq!(heap.live_node(first).unwrap().strong.get(), 2);
    assert_eq!(heap.live_node(second).unwrap().strong.get(), 2);
}

#[derive(Default)]
struct RecordingFinalizationJobSink {
    jobs: VecDeque<PreparedFinalizationJob>,
    fail_next_reservation: bool,
}

impl FinalizationJobSink for RecordingFinalizationJobSink {
    fn try_reserve_one(&mut self) -> bool {
        if std::mem::take(&mut self.fail_next_reservation) {
            return false;
        }
        self.jobs.try_reserve(1).is_ok()
    }

    fn publish_preowned(&mut self, job: PreparedFinalizationJob) {
        self.jobs.push_back(job);
    }
}

fn finalization_test_realm(
    heap: &mut Heap,
    shape: ShapeId,
) -> (ObjectId, ObjectId, ContextId, ObjectId) {
    let root = leaf(heap, shape);
    let function_prototype = heap
        .allocate_bootstrap_native_function(ObjectData::native_function(
            shape,
            Vec::new(),
            NativeFunctionId::FunctionPrototype,
            0,
        ))
        .unwrap();
    let realm = heap
        .allocate_context(ContextData::new(
            root,
            function_prototype,
            root,
            root,
            root,
            root,
            root,
            root,
        ))
        .unwrap();
    heap.attach_native_function_realm(function_prototype, realm)
        .unwrap();
    let callback = heap
        .allocate_object(ObjectData::bound_native_function(
            shape,
            Vec::new(),
            NativeFunctionId::ErrorIsError,
            realm,
            1,
        ))
        .unwrap();
    (root, function_prototype, realm, callback)
}

fn release_finalization_test_realm(
    heap: &mut Heap,
    shape: ShapeId,
    root: ObjectId,
    function_prototype: ObjectId,
    realm: ContextId,
    callback: ObjectId,
) {
    heap.release_object(callback).unwrap();
    heap.release_context(realm).unwrap();
    heap.release_object(function_prototype).unwrap();
    heap.release_object(root).unwrap();
    heap.release_shape(shape).unwrap();
    heap.run_gc_for_runtime_teardown().unwrap();
    assert_eq!(heap.counts().live, 0);
}

fn one_slot_shape(heap: &mut Heap) -> ShapeId {
    let atom = Atom::from_immediate_integer(0).unwrap();
    heap.allocate_shape(
        Shape::new(
            None,
            [ShapeEntry {
                atom: AtomIdx::from_raw(atom.raw()),
                flags: DATA_FLAGS,
            }],
        )
        .unwrap(),
    )
    .unwrap()
}

fn leaf(heap: &mut Heap, shape: ShapeId) -> ObjectId {
    heap.allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap()
}

fn bytecode(
    code: &Rc<[Instruction]>,
    realm: ContextId,
    constants: Vec<BytecodeConstant>,
    auxiliary_atoms: Vec<Atom>,
) -> FunctionBytecodeDraft {
    FunctionBytecodeDraft {
        code: code.clone(),
        numeric_regions: Box::new([]),
        constants: constants.into(),
        property_key_atoms: None,
        realm,
        metadata: FunctionMetadata {
            max_stack: 1,
            ..FunctionMetadata::default()
        },
        parameter_environment: None,
        func_name: None,
        argument_definitions: Rc::from([]),
        local_definitions: Rc::from([]),
        closure_variables: Rc::from([]),
        private_bindings: PublishedPrivateBindings::none(),
        eval_environments: Rc::from([]),
        debug: None,
        auxiliary_atoms: auxiliary_atoms.into_boxed_slice(),
    }
}

fn closure_bytecode(
    code: &Rc<[Instruction]>,
    realm: ContextId,
    closure_count: u16,
) -> FunctionBytecodeDraft {
    let mut bytecode = bytecode(code, realm, Vec::new(), Vec::new());
    bytecode.metadata.closure_count = closure_count;
    bytecode.closure_variables = (0..closure_count)
        .map(|index| ClosureVariable {
            source: ClosureSource::ParentClosure(index),
            name: ClosureVariableName::None,
            is_lexical: false,
            is_const: false,
            kind: ClosureVariableKind::Normal,
        })
        .collect::<Vec<_>>()
        .into();
    bytecode
}

fn bytecode_test_realm(heap: &mut Heap) -> ContextId {
    let shape = empty_shape(heap);
    let prototype = heap
        .allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap();
    heap.allocate_context(ContextData::new(
        prototype, prototype, prototype, prototype, prototype, prototype, prototype, prototype,
    ))
    .unwrap()
}

mod buffers;
mod bytecode;
mod collections;
mod eval_bytecode;
mod function_lifetimes;
mod modules;
mod native;
mod objects;
mod private_bytecode;
mod storage;
mod weak_collections;
