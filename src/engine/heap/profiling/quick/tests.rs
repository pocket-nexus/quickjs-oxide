use super::*;
use crate::engine::api::Runtime;
use crate::engine::api::profiling::MemorySnapshot;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::function::UnlinkedFunction;
use crate::engine::code::function::metadata::FunctionMetadata;
use crate::engine::code::quick::QuickProgram;

fn category<'a>(categories: &'a [MemoryCategory], name: &str) -> &'a MemoryCategory {
    categories.iter().find(|item| item.name == name).unwrap()
}

fn finish(memory: QuickMemory) -> Vec<MemoryCategory> {
    let mut result = Vec::new();
    memory.extend_into(&mut result);
    result
}

fn quick_categories(snapshot: &MemorySnapshot) -> Vec<MemoryCategory> {
    snapshot
        .categories
        .iter()
        .filter(|item| item.name.starts_with("bytecode_quick_"))
        .cloned()
        .collect()
}

#[test]
fn quick_memory_deduplicates_shared_buffers_but_not_equal_independent_buffers() {
    let code = [Instruction::PushI32(7), Instruction::Return];
    let program = QuickProgram::build_verified(&code).unwrap();
    let alias = program.clone();
    let independent = QuickProgram::build_verified(&code).unwrap();
    let capacity = program.storage().word_capacity;
    let mut memory = QuickMemory::new();
    memory.observe(program.storage(), code.len());
    memory.observe(alias.storage(), code.len());
    let shared = finish(memory);
    assert_eq!(category(&shared, "bytecode_quick_words").count, Some(2));
    assert_eq!(
        category(&shared, "bytecode_quick_words").capacity_bytes,
        Some(capacity * WORD_BYTES)
    );
    assert_eq!(category(&shared, "bytecode_quick_controls").count, Some(1));
    assert_eq!(
        category(&shared, "bytecode_quick_tag_generic").count,
        Some(1)
    );
    assert_eq!(
        category(&shared, "bytecode_quick_tag_push_i32").count,
        Some(1)
    );

    let mut memory = QuickMemory::new();
    memory.observe(program.storage(), code.len());
    memory.observe(independent.storage(), code.len());
    let separate = finish(memory);
    assert_eq!(category(&separate, "bytecode_quick_words").count, Some(4));
    assert_eq!(
        category(&separate, "bytecode_quick_controls").count,
        Some(2)
    );
}

#[test]
fn quick_memory_accounts_reserved_capacity_and_marks_control_bytes_as_estimates() {
    let mut memory = QuickMemory::new();
    memory.observe(
        QuickStorage {
            buffer_identity: Some(1),
            word_len: 3,
            word_capacity: 8,
            tag_counts: {
                let mut tags = [0; QUICK_TAG_COUNT];
                tags[0] = 1;
                tags[2] = 2;
                tags
            },
        },
        3,
    );
    let categories = finish(memory);
    let words = category(&categories, "bytecode_quick_words");
    assert_eq!(words.used_bytes, Some(24));
    assert_eq!(words.capacity_bytes, Some(64));
    let controls = category(&categories, "bytecode_quick_controls");
    assert_eq!(controls.used_bytes, Some(CONTROL_BYTES_ESTIMATE));
    assert_eq!(controls.capacity_bytes, Some(CONTROL_BYTES_ESTIMATE));
    assert!(controls.basis.contains("estimate"));
    assert!(controls.basis.contains("allocator"));
}

#[test]
fn quick_memory_canonical_only_counts_functions_and_generic_pcs_without_storage() {
    let code = [Instruction::ReturnUndefined];
    let program = QuickProgram::build_verified(&code).unwrap();
    let mut memory = QuickMemory::new();
    memory.observe(program.storage(), code.len());
    memory.observe(program.storage(), code.len());
    let categories = finish(memory);
    let canonical = category(&categories, "bytecode_quick_canonical_only");
    assert_eq!(canonical.count, Some(2));
    assert_eq!(canonical.used_bytes, None);
    assert_eq!(
        category(&categories, "bytecode_quick_tag_generic").count,
        Some(2)
    );
    assert_eq!(
        category(&categories, "bytecode_quick_words").capacity_bytes,
        Some(0)
    );
    assert_eq!(
        category(&categories, "bytecode_quick_controls").capacity_bytes,
        Some(0)
    );
}

#[test]
fn quick_memory_cold_parameter_boxes_are_deduplicated_and_accounted_separately() {
    let first = Box::new(ParameterEnvironmentLayout {
        initialization_end: 0,
        argument_cells: Box::new([]),
        pattern_copies: Box::new([]),
        default_sources: Box::new([]),
        synthetic_arguments_local: None,
        arg_eval_variable_object_local: None,
    });
    let second = Box::new((*first).clone());
    let mut memory = QuickMemory::new();
    memory.observe_parameter_environment(None);
    memory.observe_parameter_environment(Some(&first));
    memory.observe_parameter_environment(Some(&first));
    memory.observe_parameter_environment(Some(&second));
    let categories = finish(memory);
    let headers = category(&categories, "bytecode_parameter_environment_headers");
    assert_eq!(headers.count, Some(2));
    assert_eq!(
        headers.used_bytes,
        Some(2 * size_of::<ParameterEnvironmentLayout>())
    );
    assert_eq!(headers.capacity_bytes, headers.used_bytes);
    assert!(
        headers
            .basis
            .contains("inline Box pointer counted in arena")
    );
    assert!(headers.basis.contains("excludes nested array backing"));
    assert_eq!(
        category(&categories, "bytecode_quick_controls").count,
        Some(0)
    );
}

#[test]
fn quick_memory_parameter_box_is_present_only_for_independent_parameter_scope() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let name = "bytecode_parameter_environment_headers";
    let before = runtime.memory_snapshot();
    let baseline = category(&before.categories, name).clone();
    for (source, extra_boxes) in [
        ("(function ordinary(value) { return value + 7; });", 0),
        ("(function parameters(value = 1) { return value + 7; });", 1),
    ] {
        let function = context.compile(source).unwrap();
        let during = runtime.memory_snapshot();
        let headers = category(&during.categories, name);
        assert_eq!(
            headers.count,
            baseline.count.map(|count| count + extra_boxes)
        );
        assert_eq!(
            headers.used_bytes,
            baseline
                .used_bytes
                .map(|bytes| bytes + extra_boxes * size_of::<ParameterEnvironmentLayout>())
        );
        let snapshots = (0..8)
            .map(|_| runtime.snapshot_function_bytecode(&function).unwrap())
            .collect::<Vec<_>>();
        let shared = runtime.memory_snapshot();
        assert_eq!(category(&shared.categories, name), headers);
        drop(function);
        drop(snapshots);
        runtime.run_gc().unwrap();
        let after = runtime.memory_snapshot();
        assert_eq!(category(&after.categories, name), &baseline);
    }
}

#[test]
fn quick_memory_execution_snapshots_share_words_and_release_to_baseline() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let baseline = quick_categories(&runtime.memory_snapshot());
    let function = runtime
        .publish_unlinked_function(
            context.realm,
            UnlinkedFunction::fixture(
                vec![Instruction::PushI32(42), Instruction::Return],
                vec![],
                FunctionMetadata {
                    max_stack: 1,
                    ..FunctionMetadata::default()
                },
            ),
        )
        .unwrap();
    let first = runtime.snapshot_function_bytecode(&function).unwrap();
    let one = quick_categories(&runtime.memory_snapshot());
    assert!(
        category(&one, "bytecode_quick_words").capacity_bytes
            > category(&baseline, "bytecode_quick_words").capacity_bytes
    );
    let snapshots: Vec<_> = (0..64)
        .map(|_| runtime.snapshot_function_bytecode(&function).unwrap())
        .collect();
    assert_eq!(quick_categories(&runtime.memory_snapshot()), one);
    assert_eq!(quick_categories(&runtime.memory_snapshot()), one);
    drop(function);
    runtime.run_gc().unwrap();
    assert_eq!(quick_categories(&runtime.memory_snapshot()), one);
    drop(first);
    drop(snapshots);
    runtime.run_gc().unwrap();
    assert_eq!(quick_categories(&runtime.memory_snapshot()), baseline);
}

#[test]
fn quick_memory_many_closures_share_words_and_gc_restores_baseline() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let baseline = quick_categories(&runtime.memory_snapshot());
    drop(
        context
            .eval("globalThis.make = function make(value) { return function child() { return value + 1; }; }; globalThis.holders = [make(0)];")
            .unwrap(),
    );
    runtime.run_gc().unwrap();
    let one = quick_categories(&runtime.memory_snapshot());
    assert!(
        category(&one, "bytecode_quick_words").capacity_bytes
            > category(&baseline, "bytecode_quick_words").capacity_bytes
    );
    drop(
        context
            .eval("for (let i = 1; i < 64; i++) holders.push(make(i));")
            .unwrap(),
    );
    runtime.run_gc().unwrap();
    assert_eq!(quick_categories(&runtime.memory_snapshot()), one);
    drop(context.eval("holders = null; make = null;").unwrap());
    runtime.run_gc().unwrap();
    assert_eq!(quick_categories(&runtime.memory_snapshot()), baseline);
}
