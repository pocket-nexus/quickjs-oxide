expect_full_rewrite_table < "$boundary_dir/canaries/stage3i_canaries.txt"
expect_full_rewrite_table < "$boundary_dir/canaries/stage3j_canaries.txt"
expect_full_rewrite_rejected stage3j-self-test-marker-bypass \
    boundary-self-test-marker docs/status.md \
    'The Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    'The Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    '' '' .boundary-self-test 'forged reduced-fixture marker'
expect_full_rewrite_rejected stage3j-raw8-validator-narrowing \
    stage3j-to-propkey-protocol \
    src/engine/code/binary_object/ordinary_leaf.rs \
    '        if matches!(instruction.operation(), FunctionOp::PushThis) {' \
    '        if matches!(instruction.operation(), FunctionOp::PushThis | FunctionOp::ToPropKey) {'
expect_full_rewrite_rejected stage3j-host-primitive-fast-path-narrowed \
    s13-owned-route src/engine/vm/run.rs \
    $'                Value::Int(_) | Value::String(_) => true,' \
    $'                Value::String(_) => true,'
expect_full_rewrite_rejected stage3j-host-symbol-identity-erased \
    stage3j-to-propkey-host-semantics src/engine/vm/conversion_driver.rs \
    $'            Value::Symbol(symbol)\n        }' \
    $'            Value::Undefined\n        }'
expect_full_rewrite_rejected stage3j-host-string-hint-drift \
    stage3j-to-propkey-host-semantics src/engine/vm/conversion_driver.rs \
    $'(right, Finish::PropertyKey, ToPrimitiveHint::String)' \
    $'(right, Finish::PropertyKey, ToPrimitiveHint::Default)'
expect_full_rewrite_rejected stage3j-host-throw-identity-erased \
    stage3j-to-propkey-host-semantics src/engine/vm/conversion_driver.rs \
    $'                    Completion::Throw(value) => Completion::Throw(value),' \
    $'                    Completion::Throw(_value) => Completion::Return(Value::Undefined),'
expect_full_rewrite_rejected stage3j-to-primitive-string-literal-drift \
    stage3j-to-propkey-primitive-semantics src/engine/value/conversion/primitive.rs \
    '                    ToPrimitiveHint::String => "string",' \
    '                    ToPrimitiveHint::String => "default",'
expect_full_rewrite_rejected stage3j-to-primitive-type-error-realm-drift \
    stage3j-to-propkey-primitive-semantics src/engine/value/conversion/primitive.rs \
    '            runtime.new_native_error(self.0.realm, NativeErrorKind::Type, message)?,' \
    '            runtime.new_native_error(ContextId::ROOT, NativeErrorKind::Type, message)?,'
expect_full_rewrite_rejected stage3j-ordinary-fallback-order-drift \
    stage3j-to-propkey-ordinary-fallback src/engine/value/conversion/primitive.rs \
    '        let name = if string_first != second {' \
    '        let name = if string_first == second {'
expect_full_rewrite_rejected stage3i-validator-if-false-target-zero-erased \
    stage3i-push-this-protocol src/engine/code/binary_object/ordinary_leaf.rs \
    '            FunctionOp::IfFalse(0) | FunctionOp::IfTrue(0) | FunctionOp::Goto(0)' \
    '            FunctionOp::IfTrue(0) | FunctionOp::Goto(0)'
expect_full_rewrite_rejected stage3i-validator-if-true-target-zero-erased \
    stage3i-push-this-protocol src/engine/code/binary_object/ordinary_leaf.rs \
    '            FunctionOp::IfFalse(0) | FunctionOp::IfTrue(0) | FunctionOp::Goto(0)' \
    '            FunctionOp::IfFalse(0) | FunctionOp::Goto(0)'
expect_full_rewrite_rejected stage3i-validator-goto-target-zero-erased \
    stage3i-push-this-protocol src/engine/code/binary_object/ordinary_leaf.rs \
    '            FunctionOp::IfFalse(0) | FunctionOp::IfTrue(0) | FunctionOp::Goto(0)' \
    '            FunctionOp::IfFalse(0) | FunctionOp::IfTrue(0)'
expect_full_rewrite_rejected stage3i-translate-push-this-alias-intercept \
    stage3i-push-this-translation-route \
    src/engine/code/binary_object/function_translate/mod.rs \
    '                PendingOperation::Ready(operation) => operation,' \
    $'                PendingOperation::Ready(operation) => {\n                    use FunctionOp as O;\n                    if matches!(operation, O::PushThis) {\n                        continue;\n                    }\n                    operation\n                },'
expect_full_rewrite_rejected stage3i-ordinary-push-this-alias-pre-match \
    stage3i-push-this-ordinary-route \
    src/engine/code/binary_object/ordinary_leaf.rs \
    '    match operation {' \
    $'    use FunctionOp as O;\n    if matches!(operation, O::PushThis) {\n        return Ok(OrdinaryLeafOp::Nop);\n    }\n    match operation {'
expect_full_rewrite_rejected stage3i-status-typed-hidden-wrapper \
    stage3i-status docs/status.md \
    'Stage 3I admits raw 8 only as the exact one-to-one typed chain' \
    $'<div hidden>\nStage 3I admits raw 8 only as the exact one-to-one typed chain' \
    $'Stage 3I changes neither the engine Instruction set nor\nVM implementation and adds no source syntax, public API, Test262 admission, or\nFeature Parity claim.' \
    $'Stage 3I changes neither the engine Instruction set nor\nVM implementation and adds no source syntax, public API, Test262 admission, or\nFeature Parity claim.\n</div>'
expect_full_rewrite_rejected stage3i-status-rust-evidence-comment-wrapper \
    stage3i-status docs/status.md \
    'Stage-3I Rust evidence pins compiler-natural strict and sloppy 47-byte' \
    $'<!--\nStage-3I Rust evidence pins compiler-natural strict and sloppy 47-byte' \
    'protocol does not narrow older ordinary bodies.' \
    $'protocol does not narrow older ordinary bodies.\n-->'
expect_full_rewrite_rejected stage3i-status-c-evidence-hidden-wrapper \
    stage3i-status docs/status.md \
    'Stage 3I additionally compiler-naturally emits strict and sloppy' \
    $'<div style="display:none">\nStage 3I additionally compiler-naturally emits strict and sloppy' \
    '`e9f74aaa094cc4fb30b4a159d239d7e311622e55cd4c78f75723057a03569ee7`.' \
    $'`e9f74aaa094cc4fb30b4a159d239d7e311622e55cd4c78f75723057a03569ee7`.\n</div>'
expect_full_rewrite_rejected stage3j-status-typed-hidden-wrapper \
    stage3j-status docs/status.md \
    'Stage 3J admits raw 112 only as the exact one-to-one typed chain' \
    $'<div hidden>\nStage 3J admits raw 112 only as the exact one-to-one typed chain' \
    $'metric, or Feature Parity claim.' \
    $'metric, or Feature Parity claim.\n</div>'
expect_full_rewrite_rejected stage3j-status-c-evidence-comment-wrapper \
    stage3j-status docs/status.md \
    'Stage 3J compiler-naturally emits' \
    $'<!--\nStage 3J compiler-naturally emits' \
    '`e9f74aaa094cc4fb30b4a159d239d7e311622e55cd4c78f75723057a03569ee7`.' \
    $'`e9f74aaa094cc4fb30b4a159d239d7e311622e55cd4c78f75723057a03569ee7`.\n-->'
expect_full_rewrite_rejected stage3j-status-lifecycle-fenced-wrapper \
    stage3j-status docs/status.md \
    'The Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'```text\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    'metric, and makes no new conformance or Feature Parity claim.' \
    $'metric, and makes no new conformance or Feature Parity claim.\n```'
expect_full_rewrite_rejected stage3j-status-source-ahead-zwsp-erased \
    stage3j-status docs/status.md \
    'The Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'The Stage 3\342\200\213J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3h-to-object-stack-effect-drift \
    stage3h-to-object-verifier src/engine/code/bytecode.rs \
    '            Self::SetName(_) | Self::ToObject | Self::IteratorCheckObject => (1, 1),' \
    '            Self::SetName(_) | Self::ToObject | Self::IteratorCheckObject => (0, 1),'
expect_full_rewrite_rejected stage3h-translate-to-object-erased-second-pass \
    stage3h-to-object-translation-route \
    src/engine/code/binary_object/function_translate/mod.rs \
    '                PendingOperation::Ready(operation) => operation,' \
    $'                PendingOperation::Ready(FunctionOp::ToObject) => continue,\n                PendingOperation::Ready(operation) => operation,'
expect_full_rewrite_rejected stage3h-translate-to-object-alias-erased-second-pass \
    stage3h-to-object-translation-route \
    src/engine/code/binary_object/function_translate/mod.rs \
    '                PendingOperation::Ready(operation) => operation,' \
    $'                PendingOperation::Ready(operation) => {\n                    use FunctionOp as O;\n                    if matches!(operation, O::ToObject) {\n                        continue;\n                    }\n                    operation\n                },'
expect_full_rewrite_rejected stage3h-to-object-verifier-max-stack-bypass \
    stage3h-to-object-verifier src/engine/code/bytecode.rs \
    '        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;' \
    $'        if matches!(instruction, Instruction::ToObject) {\n            continue;\n        }\n        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;'
expect_full_rewrite_rejected stage3h-to-object-verifier-fallthrough-bypass \
    stage3h-to-object-verifier src/engine/code/bytecode.rs \
    $'            Instruction::ThrowReadOnly(_)\n            | Instruction::ThrowRedeclaration(_)' \
    $'            Instruction::ToObject\n            | Instruction::ThrowReadOnly(_)\n            | Instruction::ThrowRedeclaration(_)'
expect_full_rewrite_rejected stage3h-vm-to-object-nullish-bypass \
    s13-owned-route src/engine/vm/run.rs \
    $'                if matches!(slots.peek(0)?, Value::Object(_)) {' \
    $'                if matches!(slots.peek(0)?, Value::Object(_) | Value::Null | Value::Undefined) {'
expect_full_rewrite_rejected stage3h-vm-to-object-pre-match-diversion \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::ToObject => {' \
    $'            Instruction::ToObject => { return Ok(RunExit::Complete);'
expect_full_rewrite_rejected stage3h-vm-to-object-alias-pre-match-diversion \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::ToObject => {' \
    $'            Instruction::ToObject => { use RunExit as Exit; return Ok(Exit::Complete);'
expect_full_rewrite_rejected stage3h-vm-to-object-helper-pre-match-diversion \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::ToObject => {' \
    $'            Instruction::ToObject => { fn bypass() -> RunExit { RunExit::Complete } return Ok(bypass());'
expect_full_rewrite_rejected stage3h-runtime-test-macro-shadow \
    stage3h-runtime-evidence src/engine/heap/runtime/tests.rs \
    $'fn trusted_quickjs_ordinary_to_object_verification_rolls_back_and_retries() {\n    let mut fallthrough = QUICKJS_ORDINARY_TO_OBJECT_BC5.to_vec();' \
    $'fn trusted_quickjs_ordinary_to_object_verification_rolls_back_and_retries() {\n    macro_rules! assert_eq { ($($tokens:tt)*) => {}; }\n    let mut fallthrough = QUICKJS_ORDINARY_TO_OBJECT_BC5.to_vec();'
expect_full_rewrite_rejected stage3h-status-typed-hidden-wrapper \
    stage3h-status docs/status.md \
    'Stage 3H admits raw 111 only as the exact one-to-one typed chain' \
    $'<div hidden>\nStage 3H admits raw 111 only as the exact one-to-one typed chain' \
    $'Stage 3H changes neither production bytecode nor VM\nimplementation and adds no source syntax, public API, Test262 admission, or\nFeature Parity claim.' \
    $'Stage 3H changes neither production bytecode nor VM\nimplementation and adds no source syntax, public API, Test262 admission, or\nFeature Parity claim.\n</div>'
expect_full_rewrite_rejected stage3h-status-inherited-lifecycle-comment-wrapper \
    stage3h-status docs/status.md \
    'retains the Stage-3H raw-111 `ToObject`' \
    $'<!--\nretains the Stage-3H raw-111 `ToObject`' \
    $'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n-->'
expect_full_rewrite_rejected stage3i-status-lifecycle-comment-wrapper \
    stage3i-status docs/status.md \
    'This promoted receipt is source-current for Stage 3I' \
    $'<!--\nThis promoted receipt is source-current for Stage 3I' \
    $'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n-->'
expect_full_rewrite_rejected stage3f-translate-nop-erased \
    stage3d-throw-translation-route \
    src/engine/code/binary_object/function_translate/mod.rs \
    '                PendingOperation::Ready(operation) => operation,' \
    $'                PendingOperation::Ready(FunctionOp::Nop) => continue,\n                PendingOperation::Ready(operation) => operation,'
expect_full_rewrite_rejected stage3f-status-typed-hidden-div-wrapper \
    stage3f-status docs/status.md \
    'Stage 3F admits raw 177 only as the exact one-to-one typed chain' \
    $'<div hidden>\nStage 3F admits raw 177 only as the exact one-to-one typed chain' \
    'adds no public surface, source syntax, Test262 admission, or Feature Parity claim.' \
    $'adds no public surface, source syntax, Test262 admission, or Feature Parity claim.\n</div>'
expect_full_rewrite_rejected stage3f-status-typed-comment-wrapper \
    stage3f-status docs/status.md \
    'Stage 3F admits raw 177 only as the exact one-to-one typed chain' \
    $'<!--\nStage 3F admits raw 177 only as the exact one-to-one typed chain' \
    'adds no public surface, source syntax, Test262 admission, or Feature Parity claim.' \
    $'adds no public surface, source syntax, Test262 admission, or Feature Parity claim.\n-->'
expect_full_rewrite_rejected stage3i-status-lifecycle-hidden-div-wrapper \
    stage3i-status docs/status.md \
    'This promoted receipt is source-current for Stage 3I' \
    $'<div style="display:none">\nThis promoted receipt is source-current for Stage 3I' \
    $'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n</div>'
expect_full_rewrite_rejected stage3f-status-source-stale-appended \
    stage3f-status docs/status.md \
    'The same oracle pins compatible 32-bit `scope_next` wrapping' \
    $'Stage 3F is source-stale.\n\nThe same oracle pins compatible 32-bit `scope_next` wrapping'
expect_full_rewrite_rejected stage3f-status-source-ahead-authenticated-appended \
    stage3f-status docs/status.md \
    'The same oracle pins compatible 32-bit `scope_next` wrapping' \
    $'Stage 3F is source-ahead and authenticated.\n\nThe same oracle pins compatible 32-bit `scope_next` wrapping'
expect_full_rewrite_rejected stage3f-nop-verifier-terminal-bypass \
    stage3f-nop-verifier src/engine/code/bytecode.rs \
    $'            Instruction::ThrowReadOnly(_)\n            | Instruction::ThrowRedeclaration(_)' \
    $'            Instruction::Nop\n            | Instruction::ThrowReadOnly(_)\n            | Instruction::ThrowRedeclaration(_)'
expect_full_rewrite_rejected stage3e-synthetic-count-dropped \
    stage3e-read-only-publication src/engine/code/binary_object_publish.rs \
    '                        | OrdinaryLeafOp::ThrowReadOnly(_)' \
    ''
expect_full_rewrite_rejected stage3e-subtype-fallback-admission \
    function-translate-semantic-dispatch \
    src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::ThrowReadOnly, NativeOperands::AtomU8 { value, .. }) => Err(\n            FunctionTranslateError::unadmitted_throw_error_subtype(*value),\n        ),' \
    $'        (Recipe::ThrowReadOnly, NativeOperands::AtomU8 { atom, .. }) => {\n            ready(FunctionOp::ThrowReadOnly(project_atom(*atom)?))\n        }'
expect_full_rewrite_rejected stage3e-read-only-stack-pop \
    stage3e-read-only-verifier src/engine/code/bytecode.rs \
    $'            | Self::ThrowReadOnly(_)\n' \
    '' \
    '            | Self::Throw => (1, 0),' \
    $'            | Self::Throw\n            | Self::ThrowReadOnly(_) => (1, 0),'
expect_full_rewrite_rejected stage3e-read-only-verifier-fallthrough \
    stage3d-throw-verifier src/engine/code/bytecode.rs \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses' \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        if matches!(instruction, Instruction::ThrowReadOnly(_)) {\n            enqueue_fallthrough(\n                &mut worklist,\n                pc,\n                VerificationState {\n                    depth: next_depth,\n                    regions: next_regions.clone(),\n                    return_addresses: next_return_addresses.clone(),\n                    super_call_bases: next_super_call_bases.clone(),\n                },\n                code.len(),\n            )?;\n        }\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses'
expect_full_rewrite_rejected stage3e-vm-read-only-pop-bypass \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::ThrowReadOnly(index) | Instruction::ThrowRedeclaration(index) => {' \
    $'            Instruction::ThrowReadOnly(index) | Instruction::ThrowRedeclaration(index) => { slots.pop()?;'
expect_full_rewrite_rejected stage3e-status-source-stale-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThis R3fj receipt is source-stale for Stage 3E.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3e-crate-test-cfg-excluded \
    stage3e-runtime-evidence src/lib.rs \
    '//! A pure-Rust rewrite of `QuickJS` aiming at semantic feature parity with the' \
    $'#![cfg(not(test))]\n//! A pure-Rust rewrite of `QuickJS` aiming at semantic feature parity with the'
expect_full_rewrite_rejected stage3e-lib-test-target-disabled \
    stage3e-test-target Cargo.toml \
    '[package]' \
    $'[lib]\ntest = false\n\n[package]'
expect_full_rewrite_rejected stage3e-lib-test-target-rerouted \
    stage3e-test-target Cargo.toml \
    '[package]' \
    $'[lib]\npath = "src/test_sink.rs"\n\n[package]'
expect_full_rewrite_rejected stage3e-crate-targeted-assert-eq-shadow \
    stage3e-runtime-evidence src/lib.rs \
    'pub mod engine;' \
    $'macro_rules! assert_eq {\n    (QUICKJS_ORDINARY_READ_ONLY_BC5.len(), 47) => { () };\n    ($($tokens:tt)*) => { ::core::assert_eq!($($tokens)*) };\n}\n\npub mod engine;'
expect_full_rewrite_rejected stage3e-cross-file-macro-use-assert-eq-shadow \
    stage3e-runtime-evidence src/lib.rs \
    'pub mod engine;' \
    $'#[macro_use]\nmod stage3e_shadow;\n\npub mod engine;' \
    '' '' \
    src/stage3e_shadow.rs \
    $'macro_rules! assert_eq {\n    (QUICKJS_ORDINARY_READ_ONLY_BC5.len(), 47) => { () };\n    ($($tokens:tt)*) => { ::core::assert_eq!($($tokens)*) };\n}\n'
expect_full_rewrite_rejected stage3e-outside-src-path-macro-use-assert-eq-shadow \
    stage3e-runtime-evidence src/lib.rs \
    'pub mod engine;' \
    $'#[macro_use]\n#[path = "../tests/stage3e_shadow.rs"]\nmod stage3e_shadow;\n\npub mod engine;' \
    '' '' \
    tests/stage3e_shadow.rs \
    $'macro_rules! assert_eq {\n    (QUICKJS_ORDINARY_READ_ONLY_BC5.len(), 47) => { () };\n    ($($tokens:tt)*) => { ::core::assert_eq!($($tokens)*) };\n}\n'
expect_full_rewrite_rejected stage3e-status-stage-first-stale-appended \
    stage3i-status docs/status.md \
    'The latest promoted Stage 3I lifecycle receipt' \
    $'Stage 3E is source-stale and unauthenticated by this receipt.\n\nThe latest promoted Stage 3I lifecycle receipt'
expect_full_rewrite_rejected stage3d-vm-throw-to-return \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::Throw => return Ok(RunExit::Throw),' \
    $'            Instruction::Throw => return Ok(RunExit::Complete),'
expect_full_rewrite_rejected stage3d-translate-helper-bypass \
    function-translate-semantic-dispatch \
    src/engine/code/binary_object/function_translate/mod.rs \
    $'    let ready = |operation| Ok(PendingExpansion::one(PendingOperation::Ready(operation)));\n    match (recipe, operands) {' \
    $'    let ready = |operation| Ok(PendingExpansion::one(PendingOperation::Ready(operation)));\n    if matches!(recipe, Recipe::Throw) {\n        return ready(FunctionOp::Return);\n    }\n    match (recipe, operands) {'
expect_full_rewrite_rejected stage3d-translate-post-lowering-remap \
    stage3d-throw-translation-route \
    src/engine/code/binary_object/function_translate/mod.rs \
    '    Ok(FunctionCode::new(output.into_boxed_slice()))' \
    $'    let output = output\n        .into_iter()\n        .map(|instruction| {\n            let diagnostic = instruction.rejection_diagnostic();\n            let operation = match instruction.into_operation() {\n                FunctionOp::Throw => FunctionOp::Return,\n                operation => operation,\n            };\n            FunctionInstruction::new(InstructionAudience::OrdinaryOnly, diagnostic, operation)\n        })\n        .collect::<Vec<_>>();\n    Ok(FunctionCode::new(output.into_boxed_slice()))'
expect_full_rewrite_rejected stage3d-ordinary-helper-bypass \
    ordinary-leaf-translated-code \
    src/engine/code/binary_object/ordinary_leaf.rs \
    $'    match operation {\n        FunctionOp::Nop => Ok(OrdinaryLeafOp::Nop),\n        FunctionOp::Object => Ok(OrdinaryLeafOp::Object),\n        FunctionOp::ToObject => Ok(OrdinaryLeafOp::ToObject),\n        FunctionOp::ToPropKey => Ok(OrdinaryLeafOp::ToPropKey),\n        FunctionOp::PushThis => Ok(OrdinaryLeafOp::PushThis),\n        FunctionOp::PushI32(value) => Ok(OrdinaryLeafOp::PushI32(*value)),' \
    $'    if matches!(operation, FunctionOp::Throw) {\n        return Ok(OrdinaryLeafOp::Return);\n    }\n    match operation {\n        FunctionOp::Nop => Ok(OrdinaryLeafOp::Nop),\n        FunctionOp::Object => Ok(OrdinaryLeafOp::Object),\n        FunctionOp::ToObject => Ok(OrdinaryLeafOp::ToObject),\n        FunctionOp::ToPropKey => Ok(OrdinaryLeafOp::ToPropKey),\n        FunctionOp::PushThis => Ok(OrdinaryLeafOp::PushThis),\n        FunctionOp::PushI32(value) => Ok(OrdinaryLeafOp::PushI32(*value)),'
expect_full_rewrite_rejected stage3d-publisher-helper-bypass \
    ordinary-leaf-consumer-lowering \
    src/engine/code/binary_object_publish.rs \
    $'    let instruction = match operation {\n        OrdinaryLeafOp::Nop => Instruction::Nop,\n        OrdinaryLeafOp::Object => Instruction::Object,\n        OrdinaryLeafOp::ToObject => Instruction::ToObject,\n        OrdinaryLeafOp::ToPropKey => Instruction::ToPropKey,\n        OrdinaryLeafOp::PushThis => Instruction::PushThis,\n        OrdinaryLeafOp::PushI32(value) => Instruction::PushI32(value),' \
    $'    if matches!(&operation, OrdinaryLeafOp::Throw) {\n        return Ok(Instruction::Return);\n    }\n    let instruction = match operation {\n        OrdinaryLeafOp::Nop => Instruction::Nop,\n        OrdinaryLeafOp::Object => Instruction::Object,\n        OrdinaryLeafOp::ToObject => Instruction::ToObject,\n        OrdinaryLeafOp::ToPropKey => Instruction::ToPropKey,\n        OrdinaryLeafOp::PushThis => Instruction::PushThis,\n        OrdinaryLeafOp::PushI32(value) => Instruction::PushI32(value),'
expect_full_rewrite_rejected stage3d-throw-stack-effect-drift \
    stage3d-throw-verifier src/engine/code/bytecode.rs \
    '            | Self::Throw => (1, 0),' \
    '            | Self::Throw => (1, 1),'
expect_full_rewrite_rejected stage3d-throw-verifier-fallthrough \
    stage3d-throw-verifier src/engine/code/bytecode.rs \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses' \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        if matches!(instruction, Instruction::Throw) {\n            enqueue_fallthrough(\n                &mut worklist,\n                pc,\n                VerificationState {\n                    depth: next_depth,\n                    regions: next_regions.clone(),\n                    return_addresses: next_return_addresses.clone(),\n                    super_call_bases: next_super_call_bases.clone(),\n                },\n                code.len(),\n            )?;\n        }\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses'
expect_full_rewrite_rejected stage3d-execute-throw-bypass \
    s13-owned-route src/engine/vm/driver.rs \
    $'            let Some(Completion::Throw(value)) = forwarded.take() else {' \
    $'            let Some(Completion::Return(value)) = forwarded.take() else {'
expect_full_rewrite_rejected stage3d-raise-bypass \
    s13-owned-route src/engine/vm/iterator_driver/regions.rs \
    $'    loop {\n        let frame = execution.frames.current_mut(id)?;' \
    $'    return Ok(CallStep::Complete(Completion::Return(value)));\n    loop {\n        let frame = execution.frames.current_mut(id)?;'
expect_full_rewrite_rejected stage3d-execute-inner-post-route-throw-return \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::Throw => return Ok(RunExit::Throw),' \
    $'            Instruction::Throw => return Ok(RunExit::Complete),'
expect_full_rewrite_rejected stage3d-execute-hot-entry-throw-return \
    s13-owned-route src/engine/vm/run.rs \
    $'            Instruction::Throw => return Ok(RunExit::Throw),' \
    $'            Instruction::Throw => { slots.pop()?; return Ok(RunExit::Complete); },'
expect_full_rewrite_rejected stage3d-execute-published-throw-return \
    s13-owned-route src/engine/vm/root_call.rs \
    $'        result.finish(self.clone()).map_err(RuntimeError::Engine)' \
    $'        result.finish(self.clone()).map(|_| Completion::Return(Value::Undefined)).map_err(RuntimeError::Engine)'
expect_full_rewrite_rejected stage3d-bytecode-normal-bridge-throw-return \
    s13-owned-route src/engine/vm/frame_exit.rs \
    $'        Some(completion) => completion,' \
    $'        Some(Completion::Throw(value)) => Completion::Return(value),\n        Some(completion) => completion,'
expect_full_rewrite_rejected stage3d-bytecode-module-bridge-throw-return \
    s13-owned-route src/engine/vm/root_call.rs \
    $'        let module_link = metadata.is_module && entry.cold.input.this_value == Value::Bool(true);' \
    $'        let module_link = metadata.is_module && entry.cold.input.this_value == Value::Bool(true);\n        if module_link { return Ok(Completion::Return(Value::Undefined)); }'
expect_full_rewrite_rejected stage3d-call-internal-cleanup-throw-return \
    s13-owned-route src/engine/vm/frame_exit.rs \
    $'    if let Some(guard) = guard {\n        guard.finish().map_err(runtime_error_to_vm_error)?;\n    }' \
    $'    if let Some(guard) = guard { let _ = guard; }'
expect_full_rewrite_rejected stage3d-vm-host-call-throw-return \
    s13-owned-route src/engine/vm/driver.rs \
    $'            let Some(Completion::Throw(value)) = forwarded.take() else {' \
    $'            let Some(Completion::Throw(_value)) = forwarded.take() else {'
expect_full_rewrite_rejected stage3d-call-value-throw-return \
    s13-owned-route src/engine/vm/frame_exit.rs \
    $'        (completion, _) => completion,' \
    $'        (Completion::Throw(value), _) => Completion::Return(value),\n        (completion, _) => completion,'
expect_full_rewrite_rejected stage3d-context-call-throw-return \
    s13-owned-route src/engine/vm/root_call.rs \
    $'        result.finish(self.clone()).map_err(RuntimeError::Engine)' \
    $'        result.finish(self.clone()).map(|completion| match completion { Completion::Throw(value) => Completion::Return(value), other => other }).map_err(RuntimeError::Engine)'
expect_full_rewrite_rejected stage3d-backtrace-hook-noop \
    s13-owned-route src/engine/vm/iterator_driver/regions.rs \
    $'        .ensure_error_backtrace(&value, false, None)' \
    $'        .ensure_error_backtrace(&Value::Undefined, false, None)'
expect_full_rewrite_rejected stage3d-iterator-close-hook-noop \
    s13-owned-route src/engine/vm/iterator_driver/regions.rs \
    $'                    match super::close_unwind(runtime, execution, id, iterator, value)? {' \
    $'                    match CallStep::Complete(Completion::Throw(value)) {'
expect_full_rewrite_rejected stage3d-pending-writer-noop \
    stage3d-throw-pending src/engine/heap/runtime/mod.rs \
    $'    pub(crate) fn set_pending_exception(&self, value: Value) -> Result<(), RuntimeError> {\n        let _operation = self.operation();' \
    $'    pub(crate) fn set_pending_exception(&self, value: Value) -> Result<(), RuntimeError> {\n        let _ = value;\n        return Ok(());\n        let _operation = self.operation();'
expect_full_rewrite_rejected stage3d-pending-reader-noop \
    stage3d-throw-pending src/engine/heap/runtime/mod.rs \
    $'    pub(crate) fn take_pending_exception(&self) -> Result<Option<Value>, RuntimeError> {\n        let _operation = self.operation();' \
    $'    pub(crate) fn take_pending_exception(&self) -> Result<Option<Value>, RuntimeError> {\n        return Ok(None);\n        let _operation = self.operation();'
expect_full_rewrite_rejected stage3d-pending-observer-false \
    stage3d-throw-pending src/engine/heap/runtime/mod.rs \
    $'    pub(crate) fn has_pending_exception(&self) -> bool {\n        let _operation = self.operation();\n        self.0.state.borrow().pending_exception.is_some()\n    }' \
    $'    pub(crate) fn has_pending_exception(&self) -> bool {\n        false\n    }'
expect_full_rewrite_rejected stage3d-rust-raw48-wire-alias \
    stage3d-runtime-evidence src/engine/heap/runtime/tests.rs \
    $'    0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x01, 0x00, 0x01, 0x00, 0x00, 0xcf, 0x30,\n];' \
    $'    0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x01, 0x00, 0x01, 0x00, 0x00, 0xcf, 0x2f,\n];'
expect_full_rewrite_rejected stage3d-nonordinary-metadata-test-ignored \
    stage3d-runtime-evidence src/engine/heap/runtime/tests.rs \
    $'#[test]\nfn trusted_quickjs_ordinary_throw_rejects_nonordinary_metadata_transactionally() {' \
    $'#[test]\n#[ignore = "gate mutation"]\nfn trusted_quickjs_ordinary_throw_rejects_nonordinary_metadata_transactionally() {'
expect_full_rewrite_rejected stage3d-runtime-module-assert-eq-shadow \
    stage3d-runtime-evidence src/engine/heap/runtime/tests.rs \
    'use crate::engine::api::{EvalOptions, JsBigInt};' \
    $'use crate::engine::api::{EvalOptions, JsBigInt};\n\nmacro_rules! assert_eq { ($($tokens:tt)*) => {}; }'
expect_full_rewrite_rejected stage3d-runtime-module-assert-shadow \
    stage3d-runtime-evidence src/engine/heap/runtime/tests.rs \
    'use crate::engine::api::{EvalOptions, JsBigInt};' \
    $'use crate::engine::api::{EvalOptions, JsBigInt};\n\nmacro_rules! assert { ($($tokens:tt)*) => {}; }'
expect_full_rewrite_rejected stage3d-runtime-module-raw-assert-eq-shadow \
    stage3d-runtime-evidence src/engine/heap/runtime/tests.rs \
    'use crate::engine::api::{EvalOptions, JsBigInt};' \
    $'use crate::engine::api::{EvalOptions, JsBigInt};\n\nmacro_rules! r#assert_eq { ($($tokens:tt)*) => {}; }'
expect_full_rewrite_rejected stage3d-runtime-test-nested-cfg \
    stage3d-runtime-evidence src/engine/heap/runtime/tests/binary_throw.rs \
    '#[test]
fn trusted_quickjs_ordinary_throw_rejects_nonordinary_metadata_transactionally() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let baseline = runtime.heap_counts();
    let baseline_atoms = runtime.test_atom_count();

    let mut async_function = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    async_function[28] |= 1 << 2; // strict + async JS mode
    let mut generator = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    generator[26] |= 1 << 4; // normal -> generator FunctionKind
    let mut derived = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    derived[26] |= 1 << 2; // ordinary -> derived class constructor

    for (label, image) in [
        ("async", async_function),
        ("generator", generator),
        ("derived constructor", derived),
    ] {
        assert_eq!(
            &image[43..],
            [0xcf, 0x30],
            "{label} mutation changed the natural raw48 body"
        );
        let RuntimeError::Engine(error) = context
            .read_trusted_ordinary_function(&image, 0)
            .unwrap_err()
        else {
            panic!("{label} raw48 metadata did not return an engine error");
        };
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{label}");
        assert_eq!(
            error.message(),
            "trusted QuickJS ordinary leaf is not admitted: function metadata is outside the ordinary synchronous leaf cohort",
            "{label}"
        );
        assert!(!context.has_exception(), "{label}");
        assert_eq!(runtime.heap_counts(), baseline, "{label}");
        assert_eq!(runtime.test_atom_count(), baseline_atoms, "{label}");
    }

    let retry = context
        .read_trusted_ordinary_function(QUICKJS_ORDINARY_THROW_BC5, 0)
        .unwrap();
    assert_eq!(
        context.call(&retry, Value::Undefined, &[Value::Int(42)]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(context.take_exception().unwrap(), Some(Value::Int(42)));
    assert!(!context.has_exception());
}' \
    '#[cfg(any())]
mod disabled_raw48_metadata {
#[test]
fn trusted_quickjs_ordinary_throw_rejects_nonordinary_metadata_transactionally() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let baseline = runtime.heap_counts();
    let baseline_atoms = runtime.test_atom_count();

    let mut async_function = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    async_function[28] |= 1 << 2; // strict + async JS mode
    let mut generator = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    generator[26] |= 1 << 4; // normal -> generator FunctionKind
    let mut derived = QUICKJS_ORDINARY_THROW_BC5.to_vec();
    derived[26] |= 1 << 2; // ordinary -> derived class constructor

    for (label, image) in [
        ("async", async_function),
        ("generator", generator),
        ("derived constructor", derived),
    ] {
        assert_eq!(
            &image[43..],
            [0xcf, 0x30],
            "{label} mutation changed the natural raw48 body"
        );
        let RuntimeError::Engine(error) = context
            .read_trusted_ordinary_function(&image, 0)
            .unwrap_err()
        else {
            panic!("{label} raw48 metadata did not return an engine error");
        };
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{label}");
        assert_eq!(
            error.message(),
            "trusted QuickJS ordinary leaf is not admitted: function metadata is outside the ordinary synchronous leaf cohort",
            "{label}"
        );
        assert!(!context.has_exception(), "{label}");
        assert_eq!(runtime.heap_counts(), baseline, "{label}");
        assert_eq!(runtime.test_atom_count(), baseline_atoms, "{label}");
    }

    let retry = context
        .read_trusted_ordinary_function(QUICKJS_ORDINARY_THROW_BC5, 0)
        .unwrap();
    assert_eq!(
        context.call(&retry, Value::Undefined, &[Value::Int(42)]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(context.take_exception().unwrap(), Some(Value::Int(42)));
    assert!(!context.has_exception());
}
}'
expect_full_rewrite_rejected stage3i-status-current-receipt-erased \
    stage3i-status docs/status.md \
    'This promoted receipt is source-current for Stage 3I and covers' \
    'This promoted receipt merely describes'
expect_full_rewrite_rejected stage3i-status-run-tampered \
    stage3i-status docs/status.md \
    $'exact-source GitHub Actions\nrun `32517600968`' \
    $'exact-source GitHub Actions\nrun `32517600969`'
expect_full_rewrite_rejected stage3i-status-job-tampered \
    stage3i-status docs/status.md \
    'job `96882617930`, on receipt-promotion commit' \
    'job `96882617931`, on receipt-promotion commit'
expect_full_rewrite_rejected stage3i-status-promotion-commit-tampered \
    stage3i-status docs/status.md \
    '`a8a746f50dfc71ed560df32af9da2fb5488f7cba`' \
    '`b8a746f50dfc71ed560df32af9da2fb5488f7cba`'
expect_full_rewrite_rejected stage3i-status-artifact-tampered \
    stage3i-status docs/status.md \
    $'unique exact\nsix-file artifact `9460012228`' \
    $'unique exact\nsix-file artifact `9460012229`'
expect_full_rewrite_rejected stage3i-status-artifact-digest-tampered \
    stage3i-status docs/status.md \
    '06a95f6510ad335bd090be5bb34168e8bf683e36336441b67ebb4a831f56fd9f' \
    '16a95f6510ad335bd090be5bb34168e8bf683e36336441b67ebb4a831f56fd9f'
expect_full_rewrite_rejected stage3i-status-baseline-fingerprint-tampered \
    stage3i-status docs/status.md \
    'd21943622773d2b0b978cd2ace5261d5ec41a9400ab36864768470aae71b1d22' \
    '021943622773d2b0b978cd2ace5261d5ec41a9400ab36864768470aae71b1d22'
expect_full_rewrite_rejected stage3i-status-baseline-digest-tampered \
    stage3i-status docs/status.md \
    '2c8dc920428aef4f10be440d7d18fdf72ec0902af4c7482f5be34ed4f25b1215' \
    '0c8dc920428aef4f10be440d7d18fdf72ec0902af4c7482f5be34ed4f25b1215'
expect_full_rewrite_rejected stage3i-status-baseline-run-tampered \
    stage3i-status docs/status.md \
    'identical to Stage 3H run `32419997996`' \
    'identical to Stage 3H run `32419997997`'
expect_full_rewrite_rejected stage3i-status-baseline-artifact-tampered \
    stage3i-status docs/status.md \
    'artifact `9425844939` (SHA-256' \
    'artifact `9425844940` (SHA-256'
stage3i_current_fingerprint=$(
    sed -n 's/^engine_semantics_sha256=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
stage3i_current_source=$(
    sed -n 's/^engine_semantics_source=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
stage3i_current_focused_tsv=$(
    sed -n 's/^focused_tsv_sha256=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
stage3i_current_focused_jsonl=$(
    sed -n 's/^focused_jsonl_sha256=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
stage3i_current_full_tsv=$(
    sed -n 's/^full_tsv_sha256=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
stage3i_current_full_jsonl=$(
    sed -n 's/^full_jsonl_sha256=//p' \
        "$repository_root/dev-support/test262/current.conf"
)
[[ $stage3i_current_fingerprint =~ ^[0-9a-f]{64}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf fingerprint"
[[ $stage3i_current_source =~ ^[0-9a-f]{40}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf source"
[[ $stage3i_current_focused_tsv =~ ^[0-9a-f]{64}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf focused TSV hash"
[[ $stage3i_current_focused_jsonl =~ ^[0-9a-f]{64}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf focused JSONL hash"
[[ $stage3i_current_full_tsv =~ ^[0-9a-f]{64}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf full TSV hash"
[[ $stage3i_current_full_jsonl =~ ^[0-9a-f]{64}$ ]] \
    || die "Stage3I receipt canary requires one canonical current.conf full JSONL hash"
tamper_stage3i_receipt_hex() {
    local value=$1
    if [[ ${value:0:1} == 0 ]]; then
        printf '1%s' "${value:1}"
    else
        printf '0%s' "${value:1}"
    fi
}
stage3i_tampered_source=$(tamper_stage3i_receipt_hex "$stage3i_current_source")
stage3i_tampered_fingerprint=$(tamper_stage3i_receipt_hex "$stage3i_current_fingerprint")
stage3i_tampered_focused_tsv=$(tamper_stage3i_receipt_hex "$stage3i_current_focused_tsv")
stage3i_tampered_focused_jsonl=$(tamper_stage3i_receipt_hex "$stage3i_current_focused_jsonl")
stage3i_tampered_full_tsv=$(tamper_stage3i_receipt_hex "$stage3i_current_full_tsv")
stage3i_tampered_full_jsonl=$(tamper_stage3i_receipt_hex "$stage3i_current_full_jsonl")
expect_full_rewrite_rejected stage3i-status-current-source-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_source" \
    "$stage3i_tampered_source"
expect_full_rewrite_rejected stage3i-status-current-fingerprint-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_fingerprint" \
    "$stage3i_tampered_fingerprint"
expect_full_rewrite_rejected stage3i-status-current-focused-tsv-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_focused_tsv" \
    "$stage3i_tampered_focused_tsv"
expect_full_rewrite_rejected stage3i-status-current-focused-jsonl-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_focused_jsonl" \
    "$stage3i_tampered_focused_jsonl"
expect_full_rewrite_rejected stage3i-status-current-full-tsv-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_full_tsv" \
    "$stage3i_tampered_full_tsv"
expect_full_rewrite_rejected stage3i-status-current-full-jsonl-tampered \
    stage3i-status docs/status.md \
    "$stage3i_current_full_jsonl" \
    "$stage3i_tampered_full_jsonl"
expect_full_rewrite_rejected stage3i-status-receipt-boundary-erased \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-stale-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThis R3fj receipt is source-stale for Stage 3I.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-unauthenticated-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nStage 3I has yet to be authenticated.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-stage3h-only-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nOnly Stage 3H is authenticated by this receipt.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-not-authenticated-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nStage 3I is not authenticated.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-receipt-stage3h-only-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThis receipt only authenticates Stage 3H.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-stage3g-only-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nOnly Stage 3G is authenticated by this receipt.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-stage3f-only-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nOnly Stage 3F is authenticated by this receipt.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-pending-promotion-contradiction-appended \
    stage3i-status docs/status.md \
    $'raw-177 coverage, and makes no new conformance claim.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is' \
    $'raw-177 coverage, and makes no new conformance claim.\n\nStage 3I is pending a separate exact-source receipt promotion.\n\nThe Stage 3J raw112 `ToPropKey` Rust6/C3 working tree described above is'
expect_full_rewrite_rejected stage3i-status-html-comment-wrapper \
    stage3i-status docs/status.md \
    'The latest promoted Stage 3I lifecycle receipt' \
    $'<!--\n\nThe latest promoted Stage 3I lifecycle receipt' \
    'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n\n-->'
expect_full_rewrite_rejected stage3i-status-fenced-code-wrapper \
    stage3i-status docs/status.md \
    'The latest promoted Stage 3I lifecycle receipt' \
    $'```text\n\nThe latest promoted Stage 3I lifecycle receipt' \
    'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n\n```'
expect_full_rewrite_rejected stage3i-status-hidden-html-wrapper \
    stage3i-status docs/status.md \
    'The latest promoted Stage 3I lifecycle receipt' \
    $'<div hidden>\n\nThe latest promoted Stage 3I lifecycle receipt' \
    'raw-177 coverage, and makes no new conformance claim.' \
    $'raw-177 coverage, and makes no new conformance claim.\n\n</div>'
stage3i_status_receipt_paragraph=$(
    awk '
        found && /^[[:space:]]*$/ { exit }
        /The latest promoted Stage 3I lifecycle receipt/ { found = 1 }
        found { print }
    ' "$repository_root/docs/status.md"
)
[[ $stage3i_status_receipt_paragraph == *'This promoted receipt is source-current for Stage 3I and covers'* ]] \
    || die "Stage3I indented-code canary could not locate the promoted receipt"
stage3i_status_indented_receipt=$(printf '%s\n' \
    "$stage3i_status_receipt_paragraph" | sed 's/^/    /')
expect_full_rewrite_rejected stage3i-status-indented-code-wrapper \
    stage3i-status docs/status.md \
    "$stage3i_status_receipt_paragraph" \
    "$stage3i_status_indented_receipt"
expect_full_rewrite_rejected stage3i-status-focused-lines-drift \
    stage3i-status dev-support/test262/current.conf \
    'focused_tsv_lines=6857' \
    'focused_tsv_lines=6858'
expect_full_rewrite_rejected stage3i-status-focused-summary-drift \
    stage3i-status dev-support/test262/current.conf \
    'focused_summary=pass=6844' \
    'focused_summary=pass=6843 fail-runtime=1'
run_stage3i_receipt_escape_canaries \
    "$tmp_dir/stage3i-receipt-escape-canaries"
expect_full_rewrite_rejected ordinary-typeof-undefined-html-dda-collapse \
    ordinary-leaf-engine-semantics src/engine/vm/pure_operations.rs \
    $'                P::TypeOfIsUndefined => Value::Bool(\n                    matches!(value, Value::Undefined)\n                        || runtime\n                            .value_is_html_dda(&value)\n                            .map_err(runtime_error_to_vm_error)?,\n                ),' \
    $'                P::TypeOfIsUndefined => Value::Bool(matches!(value, Value::Undefined)),'
expect_full_rewrite_rejected translate-resolve-target-offset \
    function-translate-control-flow src/engine/code/binary_object/function_translate/mod.rs \
    $'        .copied()\n        .ok_or_else(FunctionTranslateError::invalid_branch_target)' \
    $'        .copied()\n        .map(|target| target.saturating_add(1))\n        .ok_or_else(FunctionTranslateError::invalid_branch_target)'
