expect_rejected translate-extra-module function-translate-module-set \
    src/engine/code/binary_object/function_translate/escape.rs 'fn escape() {}'
expect_full_rewrite_table < "$boundary_dir/canaries/translate_canaries.txt"
expect_full_rewrite_rejected translate-ready-remap \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    '    let ready = |operation| Ok(PendingExpansion::one(PendingOperation::Ready(operation)));' \
    '    let ready = |_operation| Ok(PendingExpansion::one(PendingOperation::Ready(FunctionOp::PushNull)));'
expect_full_rewrite_rejected translate-push-i32-payload \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::PushI32, NativeOperands::I32(value) | NativeOperands::NoneInt(value)) => {\n            ready(FunctionOp::PushI32(*value))\n        }' \
    $'        (Recipe::PushI32, NativeOperands::I32(value) | NativeOperands::NoneInt(value)) => {\n            ready(FunctionOp::PushI32(-*value))\n        }'
expect_full_rewrite_rejected translate-get-local-payload \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::GetLocal, NativeOperands::Loc(index) | NativeOperands::NoneLoc(index)) => {\n            ready(FunctionOp::GetLocal(*index))\n        }' \
    $'        (Recipe::GetLocal, NativeOperands::Loc(index) | NativeOperands::NoneLoc(index)) => {\n            ready(FunctionOp::GetLocal(index.saturating_add(1)))\n        }'
expect_full_rewrite_rejected translate-call-format-union-collapse \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (\n            Recipe::Call,\n            NativeOperands::NPop(argument_count) | NativeOperands::NPopX(argument_count),\n        ) => ready(FunctionOp::Call(*argument_count)),' \
    $'        (Recipe::Call, NativeOperands::NPop(argument_count)) =>\n            ready(FunctionOp::Call(*argument_count)),'
expect_full_rewrite_rejected translate-call-argc-plus-one \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    '        ) => ready(FunctionOp::Call(*argument_count)),' \
    '        ) => ready(FunctionOp::Call(argument_count.saturating_add(1))),'
expect_full_rewrite_rejected translate-call-argc-minus-one \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    '        ) => ready(FunctionOp::Call(*argument_count)),' \
    '        ) => ready(FunctionOp::Call(argument_count.saturating_sub(1))),'
expect_full_rewrite_rejected stage3a-translate-construct-argc-plus-one \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::Construct, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::Construct(*argument_count))\n        }' \
    $'        (Recipe::Construct, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::Construct(argument_count.saturating_add(1)))\n        }'
expect_full_rewrite_rejected stage3a-translate-construct-call-method-swap \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::Construct, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::Construct(*argument_count))\n        }\n        (Recipe::CallMethod, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::CallMethod(*argument_count))\n        }' \
    $'        (Recipe::Construct, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::CallMethod(*argument_count))\n        }\n        (Recipe::CallMethod, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::Construct(*argument_count))\n        }'
expect_full_rewrite_rejected stage3c-translate-tail-call-to-call \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::TailCall, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::TailCall(*argument_count))\n        }' \
    $'        (Recipe::TailCall, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::Call(*argument_count))\n        }'
expect_full_rewrite_rejected stage3c-translate-tail-method-to-call-return \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    $'        (Recipe::TailCallMethod, NativeOperands::NPop(argument_count)) => {\n            ready(FunctionOp::TailCallMethod(*argument_count))\n        }' \
    $'        (Recipe::TailCallMethod, NativeOperands::NPop(argument_count)) => {\n            Ok(PendingExpansion::two(\n                PendingOperation::Ready(FunctionOp::CallMethod(*argument_count)),\n                PendingOperation::Ready(FunctionOp::Return),\n            ))\n        }'
expect_full_rewrite_rejected stage3b-translate-apply-zero-kind \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    '            ready(FunctionOp::Apply(FunctionApplyKind::Call))' \
    '            ready(FunctionOp::Apply(FunctionApplyKind::Construct))'
expect_full_rewrite_rejected stage3b-translate-apply-noncanonical-admission \
    function-translate-semantic-dispatch src/engine/code/binary_object/function_translate/mod.rs \
    '            Err(FunctionTranslateError::non_canonical_apply_magic(*magic))' \
    '            ready(FunctionOp::Apply(FunctionApplyKind::Call))'
expect_full_rewrite_rejected stage3b-apply-error-classification \
    ordinary-leaf-apply-admission src/engine/code/binary_object/ordinary_leaf.rs \
    '    if error.is_unadmitted_operand_error() {' \
    '    if false && error.is_unadmitted_operand_error() {'
expect_full_rewrite_rejected stage3b-apply-stack-effect \
    stage3b-apply-stack src/engine/code/bytecode.rs \
    '            Self::Apply(_) | Self::ApplySuper => (3, 1),' \
    '            Self::Apply(_) | Self::ApplySuper => (2, 1),'
expect_full_rewrite_rejected stage3b-nullish-apply-bypass \
    stage3b-apply-order src/engine/builtins/function/invoke.rs \
    '        if matches!(value, Value::Null | Value::Undefined) {' \
    '        if false && matches!(value, Value::Null | Value::Undefined) {'
expect_full_rewrite_rejected stage3b-raw-new-target-collapse \
    stage3b-raw-construction src/engine/vm/call.rs \
    $'            ConstructNewTarget::Raw(value) => {' \
    $'            ConstructNewTarget::Validated(value) => {'
expect_full_rewrite_rejected stage3b-constructor-callable-narrowing \
    stage3b-constructor-capability src/engine/heap/runtime/mod.rs \
    '            object_data.is_constructor' \
    '            object_data.is_constructor && object_data.is_callable'
expect_full_rewrite_rejected stage3b-bound-new-target-retarget \
    stage3b-raw-construction src/engine/heap/runtime/mod.rs \
    '                    new_target.retarget_bound_identity(&constructor, &target);' \
    '                    let _ = (&new_target, &constructor, &target);'
expect_full_rewrite_rejected stage3b-proxy-before-callable \
    stage3b-raw-construction src/engine/heap/runtime/mod.rs \
    '            if self.is_proxy_object(constructor.as_object())? {' \
    '            if false && self.is_proxy_object(constructor.as_object())? {'
expect_full_rewrite_rejected stage3b-function-realm-fallback \
    stage3b-constructor-prototype src/engine/vm/call/prototype.rs \
    $'runtime.function_realm_from_value(self.0.realm, &self.0.new_target)?' \
    $'NativeConversion::Value(self.0.realm)'
expect_full_rewrite_rejected stage3b-native-prototype-helper-bypass \
    stage3b-native-prototype-family src/engine/builtins/array_buffer/constructor.rs \
    $'ProtoSourceStep::start(runtime, realm, new_target)?' \
    $'ProtoSourceStep::Complete(NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)))'
expect_full_rewrite_rejected stage3b-proxy-call-layer-capability \
    stage3b-proxy-call-order src/engine/object/internal_methods/call.rs \
    '        if !rooted.data.is_callable {' \
    '        if false && !rooted.data.is_callable {'
expect_full_rewrite_rejected stage3b-proxy-construct-callable-narrowing \
    stage3b-proxy-construct-order src/engine/object/internal_methods/construct.rs \
    $'.constructor_from_value(search.realm, Value::Object(rooted.target.clone()))?' \
    $'.callable_from_value(Value::Object(rooted.target.clone()))?'
expect_full_rewrite_rejected stage3b-public-raw-construction-leak \
    stage3b-public-construction src/engine/api/context/calls.rs \
    $'        let result = crate::engine::vm::entry::construct(\n            &self.runtime,\n            self.realm,\n            constructor,\n            new_target,\n            arguments,\n        );' \
    '        let result = self.runtime.construct_value_with_raw_new_target_internal(self.realm, Value::Object(constructor.as_object().clone()), Value::Object(new_target.as_object().clone()), arguments);'
expect_full_rewrite_rejected stage3b-species-callable-narrowing \
    stage3b-species-constructor src/engine/builtins/promise/operation/capability.rs \
    '        constructor: Option<ConstructorRef>,' \
    '        constructor: Option<CallableRef>,'
expect_full_rewrite_table < "$boundary_dir/canaries/stage3b_payload_canaries.txt"
expect_full_rewrite_rejected stage3b-apply-nullish-prework \
    stage3b-apply-order src/engine/builtins/function/invoke.rs \
    '        if matches!(value, Value::Null | Value::Undefined) {' \
    '        let _ = Self::request_arguments(value.clone(), unreachable!()); if matches!(value, Value::Null | Value::Undefined) {'
expect_full_rewrite_rejected stage3b-native-prototype-payload \
    stage3b-native-prototype-family src/engine/builtins/array_buffer/constructor.rs \
    $'ProtoSourceStep::start(runtime, realm, new_target)?' \
    $'ProtoSourceStep::start(runtime, realm, Value::Undefined)?'
expect_full_rewrite_table < "$boundary_dir/canaries/stage3c_canaries.txt"
expect_full_rewrite_rejected stage3c-tail-terminal-fallthrough \
    stage3c-tail-verifier src/engine/code/bytecode.rs \
    $'            Instruction::TailCall(_)\n            | Instruction::TailCallMethod(_)\n            | Instruction::Return\n            | Instruction::ReturnUndefined\n            | Instruction::ReturnDerived(_)\n            | Instruction::Throw\n            | Instruction::Ret => {}' \
    $'            Instruction::Return\n            | Instruction::ReturnUndefined\n            | Instruction::ReturnDerived(_)\n            | Instruction::Throw\n            | Instruction::Ret => {}'
expect_full_rewrite_rejected stage3c-blocker-mapping-swap \
    function-translate-registry-blockers \
    src/engine/code/binary_object/function_translate/capability.rs \
    $'    row!(3, Const, Blocked, FunctionGraph),\n    row!(4, Atom, ScalarOnly, Recipe::PushAtom),\n    row!(5, Atom, Blocked, ValueConstruction),' \
    $'    row!(3, Const, Blocked, ValueConstruction),\n    row!(4, Atom, ScalarOnly, Recipe::PushAtom),\n    row!(5, Atom, Blocked, FunctionGraph),'
expect_full_rewrite_rejected stage3c-translate-ready-shadow \
    function-translate-semantic-dispatch \
    src/engine/code/binary_object/function_translate/mod.rs \
    $'    let ready = |operation| Ok(PendingExpansion::one(PendingOperation::Ready(operation)));\n    match (recipe, operands) {' \
    $'    let ready = |operation| Ok(PendingExpansion::one(PendingOperation::Ready(operation)));\n    let ready = |operation| {\n        let operation = match operation {\n            FunctionOp::TailCall(argument_count) => FunctionOp::Call(argument_count),\n            FunctionOp::TailCallMethod(argument_count) => FunctionOp::CallMethod(argument_count),\n            operation => operation,\n        };\n        Ok(PendingExpansion::one(PendingOperation::Ready(operation)))\n    };\n    match (recipe, operands) {'
expect_full_rewrite_rejected stage3c-ordinary-guarded-tail-bypass \
    ordinary-leaf-translated-code \
    src/engine/code/binary_object/ordinary_leaf.rs \
    $'    match operation {\n        FunctionOp::Nop => Ok(OrdinaryLeafOp::Nop),\n        FunctionOp::Object => Ok(OrdinaryLeafOp::Object),\n        FunctionOp::ToObject => Ok(OrdinaryLeafOp::ToObject),\n        FunctionOp::ToPropKey => Ok(OrdinaryLeafOp::ToPropKey),\n        FunctionOp::PushThis => Ok(OrdinaryLeafOp::PushThis),\n        FunctionOp::PushI32(value) => Ok(OrdinaryLeafOp::PushI32(*value)),' \
    $'    if matches!(operation, FunctionOp::TailCall(0)) {\n        return Ok(OrdinaryLeafOp::Call(0));\n    }\n    match operation {\n        FunctionOp::Nop => Ok(OrdinaryLeafOp::Nop),\n        FunctionOp::Object => Ok(OrdinaryLeafOp::Object),\n        FunctionOp::ToObject => Ok(OrdinaryLeafOp::ToObject),\n        FunctionOp::ToPropKey => Ok(OrdinaryLeafOp::ToPropKey),\n        FunctionOp::PushThis => Ok(OrdinaryLeafOp::PushThis),\n        FunctionOp::PushI32(value) => Ok(OrdinaryLeafOp::PushI32(*value)),'
expect_full_rewrite_rejected stage3c-publisher-alias-tail-bypass \
    ordinary-leaf-consumer-lowering \
    src/engine/code/binary_object_publish.rs \
    $'    let instruction = match operation {\n        OrdinaryLeafOp::Nop => Instruction::Nop,\n        OrdinaryLeafOp::Object => Instruction::Object,\n        OrdinaryLeafOp::ToObject => Instruction::ToObject,\n        OrdinaryLeafOp::ToPropKey => Instruction::ToPropKey,\n        OrdinaryLeafOp::PushThis => Instruction::PushThis,\n        OrdinaryLeafOp::PushI32(value) => Instruction::PushI32(value),' \
    $'    use OrdinaryLeafOp as O;\n    if let O::TailCall(argument_count) = &operation {\n        return Ok(Instruction::Call(*argument_count));\n    }\n    let instruction = match operation {\n        OrdinaryLeafOp::Nop => Instruction::Nop,\n        OrdinaryLeafOp::Object => Instruction::Object,\n        OrdinaryLeafOp::ToObject => Instruction::ToObject,\n        OrdinaryLeafOp::ToPropKey => Instruction::ToPropKey,\n        OrdinaryLeafOp::PushThis => Instruction::PushThis,\n        OrdinaryLeafOp::PushI32(value) => Instruction::PushI32(value),'
expect_full_rewrite_rejected stage3c-stack-effect-guarded-bypass \
    stage3c-tail-verifier src/engine/code/instruction.rs \
    $'const fn nominal_stack_effect(&self) -> (usize, usize) {\n        match self {' \
    $'const fn nominal_stack_effect(&self) -> (usize, usize) {\n        if let Self::TailCall(0) | Self::TailCallMethod(0) = self {\n            return (1, 1);\n        }\n        match self {'
expect_full_rewrite_rejected stage3c-verifier-alias-fallthrough \
    stage3c-tail-verifier src/engine/code/bytecode.rs \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses' \
    $'        record_maximum_depth(&mut maximum, next_depth, declared_max_stack)?;\n        use Instruction as I;\n        if let I::TailCall(_) | I::TailCallMethod(_) = instruction {\n            enqueue_fallthrough(\n                &mut worklist,\n                pc,\n                VerificationState {\n                    depth: next_depth,\n                    regions: next_regions.clone(),\n                    return_addresses: next_return_addresses.clone(),\n                    super_call_bases: next_super_call_bases.clone(),\n                },\n                code.len(),\n            )?;\n        }\n        // QuickJS `compute_stack_size` stops as soon as a reachable PC crosses'
expect_full_rewrite_rejected stage3c-call-arguments-shadow \
    s13-owned-route src/engine/vm/driver.rs \
    '    let count = usize::from(count);' \
    '    let count = 0;'

expect_full_rewrite_rejected stage3c-call-arguments-drop \
    s13-owned-route src/engine/vm/driver.rs \
    '                .take_native_call_operands(window, count, method)?;
            return super::proxy_get_driver::start_native_with_classification(' \
    '                .take_native_call_operands(window, count, method)?;
            let mut arguments = arguments;
            arguments.pop();
            return super::proxy_get_driver::start_native_with_classification('

expect_full_rewrite_rejected stage3c-call-dispatch-alias-bypass \
    s13-owned-route src/engine/vm/run.rs \
    '                    arguments: *arguments,' \
    '                    arguments: 0,'

expect_full_rewrite_rejected stage3c-execute-inner-tail-intercept \
    s13-owned-route src/engine/vm/run.rs \
    '        let handled = match instruction {' \
    '        if matches!(instruction, Instruction::TailCall(_) | Instruction::TailCallMethod(_)) { return Ok(RunExit::Complete); }
        let handled = match instruction {'

expect_full_rewrite_rejected stage3c-execute-alias-return-bypass \
    s13-owned-route src/engine/vm/frame_exit.rs \
    '        Some(completion) => completion,' \
    '        Some(Completion::Return(_)) => Completion::Return(Value::Undefined),
        Some(completion) => completion,'

expect_full_rewrite_rejected stage3c-run-throw-bypass \
    s13-owned-route src/engine/vm/driver.rs \
    '        if matches!(forwarded, Some(Completion::Throw(_))) {' \
    '        if false && matches!(forwarded, Some(Completion::Throw(_))) {'

expect_full_rewrite_rejected stage3c-raise-guarded-bypass \
    s13-owned-route src/engine/vm/iterator_driver/regions.rs \
    '    runtime
        .ensure_error_backtrace(&value, false, None)' \
    '    if matches!(value, Value::Undefined) { return Ok(CallStep::Complete(Completion::Throw(value))); }
    runtime
        .ensure_error_backtrace(&value, false, None)'

expect_full_rewrite_rejected stage3c-required-module-cfg-excluded \
    stage3c-runtime-evidence src/engine/vm/mod.rs \
    $'#[cfg(test)]\nmod tests;' \
    $'#[cfg(any())]\n#[cfg(test)]\nmod tests;'
expect_full_rewrite_rejected stage3c-required-module-inner-cfg-excluded \
    stage3c-runtime-evidence src/engine/heap/runtime/tests.rs \
    'use super::{' \
    $'#![cfg(any())]\n\nuse super::{'
expect_full_rewrite_rejected stage3c-required-test-macro-shadow \
    stage3c-runtime-evidence src/engine/heap/runtime/tests/binary_calls.rs \
    $'fn trusted_quickjs_ordinary_tail_invocations_use_exact_bc5_wires_and_semantics() {\n    assert_eq!(QUICKJS_ORDINARY_TAIL_CALL_BC5.len(), 57);' \
    $'fn trusted_quickjs_ordinary_tail_invocations_use_exact_bc5_wires_and_semantics() {\n    macro_rules! assert_eq { ($($tokens:tt)*) => {}; }\n    assert_eq!(QUICKJS_ORDINARY_TAIL_CALL_BC5.len(), 57);'

expect_full_rewrite_rejected instruction-description-stack-bypass published-instruction-contract \
    src/engine/code/instruction.rs \
    'let (popped, pushed) = self.nominal_stack_effect();' \
    'let (popped, pushed) = (0, 0);'

expect_full_rewrite_rejected instruction-description-callback-bypass published-instruction-contract \
    src/engine/code/instruction.rs \
    'may_call_js: self.may_call_js(),' \
    'may_call_js: false,'
expect_full_rewrite_rejected instruction-description-state-bypass published-instruction-contract \
    src/engine/code/instruction.rs \
    'state: self.stack_state_effect(),' \
    'state: StackStateEffect::ConsumeSuperCall,'
expect_full_rewrite_rejected instruction-description-static-name-bypass published-instruction-contract \
    src/engine/code/instruction.rs \
    $'pub(crate) const fn static_name(self) -> Option<u32> {\n        let mut index = 0;' \
    $'pub(crate) const fn static_name(self) -> Option<u32> {\n        return None;\n        let mut index = 0;'

if grep -q 'fn stack_contract' "$repository_root/src/engine/code/instruction.rs"; then
    expect_full_rewrite_rejected instruction-split-stack-bypass published-instruction-contract \
        src/engine/code/instruction.rs \
        '            stack: self.stack_contract(),' \
        '            stack: StackEffect { popped: 0, pushed: 0, state: StackStateEffect::Ordinary },'
    expect_full_rewrite_rejected instruction-split-effects-bypass published-instruction-contract \
        src/engine/code/instruction.rs \
        '            effects: self.potential_effects(),' \
        '            effects: PotentialEffects { javascript_exception: JsExceptionEffect::None, may_call_js: false, may_allocate: false },'
    stack_adapter_before=$'pub const fn stack_effect(&self) -> (usize, usize) {\n        self.nominal_stack_effect()'
else
    expect_full_rewrite_rejected instruction-direct-control-bypass published-instruction-contract \
        src/engine/code/instruction.rs \
        '            control: self.control_effect(),' \
        '            control: ControlEffect::Next,'
    expect_full_rewrite_rejected instruction-direct-exception-bypass published-instruction-contract \
        src/engine/code/instruction.rs \
        '                javascript_exception: self.javascript_exception_effect(),' \
        '                javascript_exception: JsExceptionEffect::None,'
    stack_adapter_before=$'pub const fn stack_effect(&self) -> (usize, usize) {\n        let effect = self.info().stack;\n        (effect.popped, effect.pushed)'
fi
expect_full_rewrite_rejected instruction-nominal-adapter-bypass published-instruction-stack-adapter \
    src/engine/code/bytecode.rs \
    "$stack_adapter_before" \
    $'pub const fn stack_effect(&self) -> (usize, usize) {\n        (0, 0)'
