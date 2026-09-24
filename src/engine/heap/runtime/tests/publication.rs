use super::*;

#[test]
fn bytecode_is_rooted_and_calls_separate_caller_from_callee_realm() {
    let runtime = Runtime::new();
    let mut compiler_context = runtime.new_context();
    let compiler_realm = compiler_context.realm;
    let intrinsic_realm_roots = runtime
        .0
        .state
        .borrow()
        .heap
        .context_strong_count(compiler_realm)
        .unwrap();
    let function = compiler_context.compile("this").unwrap();
    let bytecode_id = function.bytecode_id();

    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode_strong_count(bytecode_id),
        Ok(1)
    );
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .context_strong_count(compiler_realm),
        Ok(intrinsic_realm_roots + 1)
    );
    let duplicate = function.clone();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .function_bytecode_strong_count(bytecode_id),
        Ok(2)
    );
    drop(duplicate);

    let mut caller_context = runtime.new_context();
    let caller_global = caller_context.global_object().unwrap();
    drop(compiler_context);
    assert_eq!(runtime.heap_counts().context_nodes, 2);

    let snapshot = runtime.snapshot_function_bytecode(&function).unwrap();
    assert_eq!(snapshot.realm, compiler_realm);
    drop(snapshot);
    assert_eq!(
        caller_context.execute(&function).unwrap(),
        Value::Object(caller_global)
    );

    drop(function);
    assert_eq!(runtime.heap_counts().function_bytecode_nodes, 0);
    assert_eq!(runtime.heap_counts().context_nodes, 2);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().context_nodes, 1);
}

#[test]
fn publication_accepts_duplicate_program_var_declaration_descriptors() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let descriptor = ClosureVariable {
        source: ClosureSource::GlobalDeclaration,
        name: ClosureVariableName::Constant(0),
        is_lexical: false,
        is_const: false,
        kind: ClosureVariableKind::Normal,
    };
    let function = UnlinkedFunction::fixture_with_closure_variables(
        vec![Instruction::GetVar(0), Instruction::Return],
        vec![
            UnlinkedConstant::primitive(Value::String(JsString::from_static("duplicateVar")))
                .unwrap(),
        ],
        FunctionMetadata {
            closure_count: 2,
            max_stack: 1,
            ..FunctionMetadata::default()
        },
        vec![descriptor, descriptor],
    );
    let function = runtime
        .publish_unlinked_function(context.realm, function)
        .unwrap();

    assert_eq!(context.execute(&function).unwrap(), Value::Undefined);
    let key = runtime.intern_property_key("duplicateVar").unwrap();
    let global = context.global_object().unwrap();
    assert_eq!(
        context.get_own_property(&global, &key).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: false,
        })
    );
}

#[test]
fn publication_accepts_annex_masked_mixed_global_declaration_descriptors() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let ordinary = ClosureVariable {
        source: ClosureSource::GlobalDeclaration,
        name: ClosureVariableName::Constant(0),
        is_lexical: false,
        is_const: false,
        kind: ClosureVariableKind::Normal,
    };
    let lexical_let = ClosureVariable {
        is_lexical: true,
        ..ordinary
    };
    let lexical_const = ClosureVariable {
        is_const: true,
        ..lexical_let
    };
    let function = UnlinkedFunction::fixture_with_closure_variables(
        vec![
            Instruction::PushI32(7),
            Instruction::PutVarInit(0),
            Instruction::GetVar(0),
            Instruction::Return,
        ],
        vec![
            UnlinkedConstant::primitive(Value::String(JsString::from_static("annexMaskedMixed")))
                .unwrap(),
        ],
        FunctionMetadata {
            closure_count: 4,
            max_stack: 1,
            ..FunctionMetadata::default()
        },
        vec![ordinary, lexical_let, lexical_const, ordinary],
    );
    let function = runtime
        .publish_unlinked_function(context.realm, function)
        .unwrap();

    assert_eq!(context.execute(&function).unwrap(), Value::Int(7));
}

#[test]
fn deeply_nested_child_publication_and_release_are_iterative() {
    const DEPTH: usize = 50_000;

    let runtime = Runtime::new();
    let context = runtime.new_context();
    let metadata = FunctionMetadata {
        max_stack: 1,
        ..FunctionMetadata::default()
    };
    let mut function = UnlinkedFunction::fixture(
        vec![Instruction::Undefined, Instruction::Return],
        Vec::new(),
        metadata,
    );
    for _ in 0..DEPTH {
        function = UnlinkedFunction::fixture(
            vec![Instruction::Undefined, Instruction::Return],
            vec![UnlinkedConstant::child(function)],
            metadata,
        );
    }

    let function = runtime
        .publish_unlinked_function(context.realm, function)
        .unwrap();
    assert_eq!(runtime.heap_counts().function_bytecode_nodes, DEPTH + 1);
    drop(function);
    assert_eq!(runtime.heap_counts().function_bytecode_nodes, 0);
    assert_eq!(runtime.heap_counts().context_nodes, 1);
}
