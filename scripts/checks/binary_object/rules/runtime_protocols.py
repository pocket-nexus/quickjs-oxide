"""Runtime protocols checks, in the ordered boundary scan."""
from __future__ import annotations

import re
from copy import deepcopy

from ..evidence import runtime_protocols as evidence
from .verifier_kernel import resolve as resolve_verifier_kernel


def check(ctx):
    if ctx.self_test_marker_authorized:
        return

    stage3b_sources = {
        "src/engine/heap/runtime/mod.rs": ctx.runtime_code,
        "src/engine/vm/mod.rs": ctx.rust_code_only(ctx.read_source("src/engine/vm/mod.rs")),
        "src/engine/code/bytecode.rs": ctx.bytecode_code,
        "src/engine/api/context/bytecode.rs": ctx.context_code,
    }

    stage3b_items: dict[tuple[str, str], str] = {}

    def stage3b_code(relative: str) -> str:
        if relative not in stage3b_sources:
            stage3b_sources[relative] = ctx.rust_code_only(ctx.read_source(relative))
        return stage3b_sources[relative]
    ctx.stage3b_code = stage3b_code

    def stage3b_function(relative: str, name: str, diagnostic: str) -> str:
        key = (relative, name)
        if key not in stage3b_items:
            stage3b_items[key] = ctx.unique_braced_item(
                ctx.stage3b_code(relative),
                re.compile(rf"\bfn[ \t\n]+{re.escape(name)}\b[^{{}};]*\{{"),
                diagnostic,
                f"{relative}::{name}",
            )[0]
        return stage3b_items[key]
    ctx.stage3b_function = stage3b_function


    def stage3j_source_function(relative: str, name: str, diagnostic: str) -> str:
        source = ctx.read_source(relative)
        code = ctx.rust_code_only(source)
        _, start, end = ctx.unique_braced_item(
            code,
            re.compile(rf"\bfn[ \t\n]+{re.escape(name)}\b[^{{}};]*\{{"),
            diagnostic,
            f"{relative}::{name}",
        )
        if start < 0 or end < 0:
            return ""
        return source[start:end]
    ctx.stage3j_source_function = stage3j_source_function

    if len(re.findall(
        r"\bstruct[ \t\n]+ConstructorRef[ \t\n]*\([ \t\n]*ObjectRef[ \t\n]*\)[ \t\n]*;",
        ctx.runtime_code,
    )) != 1:
        ctx.fail("stage3b-constructor-capability", "ConstructorRef must remain the private [[Construct]] capability")

    construct_new_target_code = ctx.unique_braced_item(
        ctx.runtime_code,
        re.compile(r"\benum[ \t\n]+ConstructNewTarget[ \t\n]*\{"),
        "stage3b-new-target-capability",
        "validated-or-raw newTarget carrier",
    )[0]

    construct_new_target_payloads = {
        name: [" ".join(value.split()) for value in re.findall(
            rf"\b{name}[ \t\n]*\(([^()]*)\)[ \t\n]*,", construct_new_target_code
        )]
        for name in ("Validated", "Raw")
    }

    if (
        ctx.enum_variant_names(construct_new_target_code) != ["Validated", "Raw"]
        or construct_new_target_payloads != {"Validated": ["ConstructorRef"], "Raw": ["Value"]}
    ):
        ctx.fail(
            "stage3b-new-target-capability",
            f"ConstructNewTarget must remain Validated(ConstructorRef) or Raw(Value); found {construct_new_target_payloads}",
        )

    constructor_from_value = ctx.stage3b_function(
        "src/engine/heap/runtime/mod.rs", "constructor_from_value", "stage3b-constructor-capability"
    )

    normalized_constructor_from_value = " ".join(constructor_from_value.split())

    if (
        "Result<NativeConversion<ConstructorRef>, RuntimeError>" not in normalized_constructor_from_value
        or normalized_constructor_from_value.count("object_data.is_constructor") != 1
        or normalized_constructor_from_value.count("ConstructorRef::from_validated_object(object)") != 1
        or re.search(r"\b(?:CallableRef|is_callable|as_callable|callable_from_value)\b", constructor_from_value)
    ):
        ctx.fail(
            "stage3b-constructor-capability",
            "constructor conversion must validate only [[Construct]], without CallableRef narrowing",
        )


    if ctx.bytecode_code.count("Self::Apply(_) | Self::ApplySuper => (3, 1),") != 1:
        ctx.fail("stage3b-apply-stack", "Apply must retain its exact three-pop/one-push verifier effect")

    # Authenticate the fact-preserving descriptor split and public projection.
    from .publication_contracts import check_instruction
    check_instruction(ctx)

    tail_stack_effects = (
        "Self::TailCall(argument_count) => (*argument_count as usize + 1, 0),",
        "Self::TailCallMethod(argument_count) => (*argument_count as usize + 2, 0),",
    )

    stack_effect_item = ctx.stage3b_function(
        "src/engine/code/instruction.rs", "nominal_stack_effect", "stage3c-tail-verifier"
    )

    ctx.require_normalized_code_sha256(
        "stage3c-tail-verifier",
        "The shared nominal stack contract must remain the reviewed exhaustive model",
        stack_effect_item,
        "7fa361fbe20e888631a7c8330138fc00ad162c05f8cbcca476d1b235149dd957",
    )

    normalized_stack_effect = " ".join(stack_effect_item.split())

    if any(normalized_stack_effect.count(fragment) != 1 for fragment in tail_stack_effects):
        ctx.fail(
            "stage3c-tail-verifier",
            "TailCall and TailCallMethod must preserve argc+1/argc+2 pops and zero pushes",
        )

    verify_parts_item = resolve_verifier_kernel(ctx)

    normalized_verify_parts = " ".join(verify_parts_item.split())

    verifier_terminal_guard = deepcopy(evidence.VERIFIER_TERMINAL_GUARD)

    verifier_terminal_offset = normalized_verify_parts.find(verifier_terminal_guard)

    if verifier_terminal_offset < 0:
        ctx.fail(
            "stage3c-tail-verifier",
            "the reviewed terminal verifier corridor is missing",
        )
    else:
        ctx.require_normalized_code_sha256(
            "stage3c-tail-verifier",
            "the iterator guard, maximum-depth check, terminal dispatch, and successor enqueue corridor must remain alias-free and ordered",
            normalized_verify_parts[verifier_terminal_offset:],
            "5f18313dcb4ee5192b9fc3a53c76e1fe7e63bfba3eb3e5761033ecf5ad694370",
        )

    tail_terminal_prefix = "Instruction::TailCall(_) | Instruction::TailCallMethod(_)"

    tail_terminal_dispatch = deepcopy(evidence.TAIL_TERMINAL_DISPATCH)

    if (
        normalized_verify_parts.count(tail_terminal_prefix) != 2
        or normalized_verify_parts.count(tail_terminal_dispatch) != 1
        or normalized_verify_parts.count("enqueue_target(") != 4
        or normalized_verify_parts.count("enqueue_fallthrough(") != 4
    ):
        ctx.fail(
            "stage3c-tail-verifier",
            "both tail invocation instructions must be terminal verifier nodes and must not enqueue fallthrough",
        )


    capability_relative = "src/engine/code/binary_object/function_translate/capability.rs"

    capability_code = ctx.stage3b_code(capability_relative)

    capability_row_new = ctx.stage3b_function(
        capability_relative, "new", "stage3d-throw-capability-route"
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-capability-route",
        "CapabilityRow::new must preserve the raw, format, and policy fields without rewriting raw48",
        capability_row_new,
        "9e4be6620a97b5136ea400f536be3db0ef7f5cd95c74279dc30969278e19b4fc",
    )

    capability_row_macro = ctx.unique_braced_item(
        capability_code,
        re.compile(r"\bmacro_rules[ \t\n]*![ \t\n]*row[ \t\n]*\{"),
        "stage3d-throw-capability-route",
        "capability row macro",
    )[0]

    ctx.require_normalized_code_sha256(
        "stage3d-throw-capability-route",
        "the capability row macro must construct the declared audience and recipe directly",
        capability_row_macro,
        "96eac354655d5c9e47c266be4e31d6d1cbfbecc36fe70083bc90e1a59210a59b",
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-capability-route",
        "row_for must select only the physical raw opcode's registry row",
        ctx.stage3b_function(
            capability_relative, "row_for", "stage3d-throw-capability-route"
        ),
        "e05d664bfc0b3bdd581c293111a0a81440e7227aa75b46baab209c061ce3f131",
    )

    dto_relative = "src/engine/code/binary_object/function_translate/dto.rs"

    for ctx.name, ctx.description, ctx.expected_hash in (
        (
            "includes_ordinary",
            "ordinary audience membership must remain OrdinaryOnly or Shared",
            "eac985e03d25991731ee08d4964ed8f621886bd9e6d6783dde9f117075346be8",
        ),
        (
            "supports_ordinary",
            "FunctionInstruction must consult its retained audience for ordinary admission",
            "d0f6a6a6678edbe91a19fdf00d9deab39d21d203fd38f57590b4d41abe8d8d8f",
        ),
        (
            "operation",
            "FunctionInstruction must expose the retained typed operation without substitution",
            "903997a7bf00f7594ba961f08a431af59ad1bd42ca678390d39944b8b8c11c0a",
        ),
        (
            "instructions",
            "FunctionCode must expose its retained instruction sequence without filtering",
            "ae989b36159707bc008c1f1b7144b6c0c0b11497ce8b80046157d5a22db99378",
        ),
    ):
        ctx.require_normalized_code_sha256(
            "stage3d-throw-translation-route",
            ctx.description,
            ctx.stage3b_function(dto_relative, ctx.name, "stage3d-throw-translation-route"),
            ctx.expected_hash,
        )

    translate_relative = "src/engine/code/binary_object/function_translate/mod.rs"

    translate_code = ctx.stage3b_code(translate_relative)

    ctx.require_normalized_code_sha256(
        "stage3d-throw-translation-route",
        "TranslationTarget must delegate ordinary admission to the retained audience",
        ctx.stage3b_function(
            translate_relative, "accepts", "stage3d-throw-translation-route"
        ),
        "90579d0e77fd5b936444117085b9b1bb8246364a9f8bc3878c95d7e3e271d16c",
    )

    ctx.pending_expansion_impl = ctx.unique_braced_item(
        translate_code,
        re.compile(
            r"\bimpl[ \t\n]*<[ \t\n]*'image[ \t\n]*>[ \t\n]+"
            r"PendingExpansion[ \t\n]*<[ \t\n]*'image[ \t\n]*>[ \t\n]*\{"
        ),
        "stage3d-throw-translation-route",
        "PendingExpansion implementation",
    )[0]

    ctx.require_normalized_code_sha256(
        "stage3d-throw-translation-route",
        "PendingExpansion must retain each ready operation exactly once and in order",
        ctx.pending_expansion_impl,
        "9f43f69140aababa9f85f6845ce991b6133a52693feb7dbb83177e98e3c6001e",
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-translation-route",
        "operation_for_target must lower admitted ordinary operations and reject outside-target aliases",
        ctx.stage3b_function(
            translate_relative,
            "operation_for_target",
            "stage3d-throw-translation-route",
        ),
        "124ecc9366407ebfa14448710b3795c2ee74137aea4f099076fdd13e0b32aec1",
    )

    translate_native_plan_item = ctx.stage3b_function(
        translate_relative, "translate_native_plan", "stage3d-throw-translation-route"
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-translation-route",
        "translate_native_plan must preserve the physical row, target audience, ready operation, and final FunctionCode without a post-lowering remap",
        translate_native_plan_item,
        "fe149677e125ffef44ebac61b8b9799eff3166eafd7efa93e086b84571eb9867",
    )

    ctx.require_normalized_corridor_sha256(
        "stage3d-throw-translation-route",
        "translate_native_plan must carry the physical row through its audience expansion into pending code",
        translate_native_plan_item,
        "let row = row_for(opcode);",
        "pending.push(PendingInstruction { audience, diagnostic, expansion, });",
        "de451567beafe6477081e0466f2cdc2b1cb302b8a8202130ef53226509f39524",
    )

    ctx.require_normalized_corridor_sha256(
        "stage3d-throw-translation-route",
        "translate_native_plan must publish every ready operation through FunctionInstruction::new without remapping",
        translate_native_plan_item,
        "for instruction in pending {",
        "output.push(FunctionInstruction::new( instruction.audience, instruction.diagnostic, operation, ));",
        "154a0d0ab86cab0cb75aebcc4bb31e8cd6e7b8015e02a4f61dd32eadf17842ae",
    )

    ordinary_relative = "src/engine/code/binary_object/ordinary_leaf.rs"

    ctx.require_normalized_code_sha256(
        "stage3d-throw-ordinary-route",
        "ordinary lower_code must require ordinary audience support and lower every typed operation once",
        ctx.stage3b_function(ordinary_relative, "lower_code", "stage3d-throw-ordinary-route"),
        "802efb137202fe4c4b7380359e627b9e97d676ed0e19b6041a58230e03820557",
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-ordinary-route",
        "OrdinaryLeafDraft::into_parts must retain metadata, constants, and code without substitution",
        ctx.stage3b_function(ordinary_relative, "into_parts", "stage3d-throw-ordinary-route"),
        "cb258f4d808ff5458a2be60fe0050ebc734736ec811b2232f9ff6c031e7e1cf9",
    )

    ctx.require_normalized_code_sha256(
        "stage3e-read-only-ordinary-route",
        "admit_image must pass the authenticated input-atom slot count into the typed ordinary lowering without bypass",
        ctx.stage3b_function(ordinary_relative, "admit_image", "stage3e-read-only-ordinary-route"),
        "afec36c89046ac4ddb822a58c95bb6d1f8683ce6cedfea0b2718c4d9bf4434ae",
    )

    input_atom_ledger_impl = ctx.unique_braced_item(
        ctx.stage3b_code(ordinary_relative),
        re.compile(r"\bimpl[ \t\n]+InputAtomLedger[ \t\n]*\{"),
        "stage3e-read-only-atom-ledger",
        "InputAtomLedger implementation",
    )[0]

    ctx.require_normalized_code_sha256(
        "stage3e-read-only-atom-ledger",
        "the input-atom ledger must admit at most one slot, require any declared slot to be consumed by raw49, and preserve provenance",
        input_atom_ledger_impl,
        "3b2000fa311f9de95eca3794884907f8b565f2cad4f52b2a2ce0369226799057",
    )

    detached_atom_name_impl = ctx.unique_braced_item(
        ctx.stage3b_code(ordinary_relative),
        re.compile(r"\bimpl[ \t\n]+DetachedAtomName[ \t\n]*\{"),
        "stage3e-read-only-atom-ledger",
        "DetachedAtomName implementation",
    )[0]

    ctx.require_normalized_code_sha256(
        "stage3e-read-only-atom-ledger",
        "DetachedAtomName must release only its owned UTF-16 units into publication",
        detached_atom_name_impl,
        "fe30418c8de3040b9177db6f452ab0c95a961a658407bc83a3b20177077377b0",
    )

    ctx.require_normalized_code_sha256(
        "stage3e-read-only-atom-ledger",
        "copy_read_only_name must admit only String atoms and preserve every UTF-16 unit in an owned payload",
        ctx.stage3b_function(ordinary_relative, "copy_read_only_name", "stage3e-read-only-atom-ledger"),
        "b5f7bc69b10a77a23b94db94558816a972c477459aab01c2939368eab2d7e8e0",
    )

    ctx.require_normalized_code_sha256(
        "stage3e-read-only-publication",
        "the ordinary publisher must synthesize exactly one verified String constant per ThrowReadOnly and publish its matching typed index before verification",
        ctx.stage3b_function(
            ctx.consumer_relative,
            "read_trusted_ordinary_function_in_realm",
            "stage3e-read-only-publication",
        ),
        "6528a8aefe09efba3d732e33a6de6a6d8daae0de67cb62fe3d5f68b97fe9d745",
    )

    if normalized_stack_effect.count("| Self::Throw => (1, 0),") != 1:
        ctx.fail(
            "stage3d-throw-verifier",
            "Throw must consume exactly one value and produce no fallthrough value",
        )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-verifier",
        "the full typed verifier must keep Throw terminal without a guarded or aliased fallthrough path",
        verify_parts_item,
        ("055bb6802b9422b7ab996d0506ff7f75b5b1aa9251cdadb22e335df7135974e3"
         if ctx.verifier_kernel_kind == "compact" else
         "ef9fe333359c127175f3a83bd4c20996702ca11d581096a205ffeb4e30fafe41"),
    )

    if normalized_verify_parts.count(tail_terminal_dispatch) != 1:
        ctx.fail(
            "stage3d-throw-verifier",
            "Throw must remain in the unique terminal verifier arm",
        )

    if (
        normalized_stack_effect.count("| Self::ThrowReadOnly(_)") != 1
        or "Self::ThrowReadOnly(_) => (1, 0)" in normalized_stack_effect
        or normalized_verify_parts.count(
            "Instruction::ThrowReadOnly(_) | Instruction::ThrowRedeclaration(_) | Instruction::ThrowDeleteSuper | Instruction::ThrowIteratorMissingThrow => {}"
        ) != 1
    ):
        ctx.fail(
            "stage3e-read-only-verifier",
            "ThrowReadOnly must remain a zero-pop, zero-push terminal verifier node with no fallthrough",
        )

    if (
        normalized_stack_effect.count("Self::Nop | Self::CheckCtor") != 1
        or normalized_verify_parts.count("Instruction::Nop") != 0
        or normalized_verify_parts.count("_ => enqueue_fallthrough(") != 1
    ):
        ctx.fail(
            "stage3f-nop-verifier",
            "Instruction::Nop must remain an existing zero-pop, zero-push node that reaches the verifier's unique ordinary fallthrough path",
        )

    if (
        normalized_stack_effect.count("Self::Object => (0, 1)") != 1
        or normalized_verify_parts.count("Instruction::Object") != 0
        or normalized_verify_parts.count("_ => enqueue_fallthrough(") != 1
    ):
        ctx.fail(
            "stage3g-object-verifier",
            "Instruction::Object must remain an existing zero-pop, one-push node that reaches the verifier's unique ordinary fallthrough path",
        )

    if (
        normalized_stack_effect.count(
            "Self::SetName(_) | Self::ToObject | Self::IteratorCheckObject => (1, 1)"
        )
        != 1
        or normalized_verify_parts.count("Instruction::ToObject") != 0
        or normalized_verify_parts.count("_ => enqueue_fallthrough(") != 1
    ):
        ctx.fail(
            "stage3h-to-object-verifier",
            "Instruction::ToObject must remain an existing one-pop, one-push node that reaches the verifier's unique ordinary fallthrough path",
        )


    for ctx.name, ctx.description, ctx.expected_hash in (
        (
            "set_pending_exception",
            "the pending-exception writer must retain the original value as an owned runtime root",
            "128c592e4525a60a1bd79dffff836195d8269c54be97a7f17cb98bc5a00a14f8",
        ),
        (
            "take_pending_exception",
            "the pending-exception reader must take and reconstruct the owned original value",
            "8b9051265509db89853144e02a180da15b0851f4d6fbe13bce2b8f3b5743d6de",
        ),
        (
            "has_pending_exception",
            "the pending-exception observer must report the actual pending slot",
            "1812d935455fea3c9027d72b60c252701cc783baa4433a1364050ea4b42e4956",
        ),
    ):
        ctx.require_normalized_code_sha256(
            "stage3d-throw-pending",
            ctx.description,
            ctx.stage3b_function("src/engine/heap/runtime/mod.rs", ctx.name, "stage3d-throw-pending"),
            ctx.expected_hash,
        )

    for ctx.name, ctx.description, ctx.expected_hash in (
        (
            "has_exception",
            "Context::has_exception must observe the runtime pending slot directly",
            "81a8f856db04b94fd5dd58141a1ed0763d3132fc3ebe3d19dec1645e5367de77",
        ),
        (
            "take_exception",
            "Context::take_exception must return the value taken from the runtime pending slot",
            "691d43e5f989578873bf0ae60896055ed031a9c031c4e4ed74cb908e172bd626",
        ),
    ):
        ctx.require_normalized_code_sha256(
            "stage3d-throw-pending",
            ctx.description,
            ctx.stage3b_function("src/engine/api/context/mod.rs", ctx.name, "stage3d-throw-pending"),
            ctx.expected_hash,
        )

    finish_completion_item = ctx.stage3b_function(
        "src/engine/api/context/mod.rs", "finish_completion", "stage3d-throw-pending"
    )

    ctx.require_normalized_code_sha256(
        "stage3d-throw-pending",
        "the public Context completion bridge must retain the original thrown value in the pending-exception slot",
        finish_completion_item,
        "d2e99ad914f05e1e7d81e0d909d480fae797df41f295a0a7abf806f1ea57ebc9",
    )

    ctx.require_ordered_fragments(
        "stage3d-throw-pending",
        "Completion::Throw must become the pending exception before RuntimeError::Exception is returned",
        finish_completion_item,
        (
            "Completion::Return(value) => Ok(value),",
            "Completion::Throw(value) => {",
            "self.runtime.set_pending_exception(value)?;",
            "Err(RuntimeError::Exception)",
        ),
    )

    # S13: the sole explicit-stack core replaces retired VmHost corridors.
    for relative, name, description, expected in (
        ('src/engine/vm/proxy_get_driver/native.rs', 'start_selected_into', 'native leaf ABI and yielding operations remain separate', '1b4532cb7848223fbc0d6b06ad1ae17744f500776cbfab761c32d0931521ca4d'),
        ('src/engine/vm/proxy_get_driver.rs', 'invoke', 'JS callbacks install explicit frames; synchronous ABI admits only Native classification', '31b856e6988e117b02c4c08508e0aaaa92c69a0424bbaf660d6fb56727f66b6c'),
        ('src/engine/vm/conversion_driver.rs', 'invoke', 'conversion invokes typed callbacks without generic synchronous replay', 'fdacadbe15d94508c91e4e5f44308b89452e88a3a0616fc15a0d37583fe965fe'),
        ('src/engine/vm/run.rs', 'run', 'resident dispatch retains throw, object, boxing and control-flow semantics', '1190b9d8777cca2b0c99ee1bbdd434f02f25f32218a6660cbcdcea5f1e169a4c'),
        ('src/engine/vm/root_call.rs', 'execute_bytecode_callable', 'root call preserves async/generator/ordinary completion', 'ca69140819476e38342b78796e68141606b6cbe9f5386589ffb6439668df1a4d'),
        ('src/engine/vm/root_call.rs', 'prepare_call', 'published root call validates closure owners and arguments', 'cf9998bd8aceca9a102047a98d9407f5b918f2025f34db260458e22ade702b77'),
        ('src/engine/vm/driver.rs', 'enter_call', 'Call and TailCall share checked window ownership and receiver selection', 'bbad8d2818a371b4e47bc9c24fe3b867859d8f87daca5e9cccc321acd9263a6d'),
        ('src/engine/vm/driver.rs', 'run_frames_with_state', 'driver dispatch preserves typed replies and throw/unwind routing', '8cafa48d5344df3b207d23449e9f6bf10f67387cf4e11bd0b83c4573294e0fce'),
        ('src/engine/vm/frame_exit.rs', 'finish', 'frame retirement preserves completion and constructor results', 'ba3789348e9a0225fc21d0d1a103c28fe0496a62b15b546694ad656211011d2e'),
        ('src/engine/vm/iterator_driver/regions.rs', 'unwind', 'throw routing preserves catch/iterator order and pending value', '63b5e624bb72b0415c35da04e0433b6a25cfba71514aa0ec62a037378c553ea1'),
        ('src/engine/vm/suspend.rs', 'freeze_entry', 'suspension encodes direct frame owners and bytecode identity', '4cae4be79a290ff77d5252747801081564b09b74b3e6d75dcedaee570e45a126'),
        ('src/engine/vm/suspend.rs', 'thaw', 'resume authenticates published owner and frame shape', '14a1028a01f836adaef2ac8bb67597532c958971f9f7d6a898b9c96d68dd2ba8'),
        ('src/engine/vm/proxy_get_driver.rs', 'start_apply', 'Apply preserves its typed call/construct continuation', '8882314d591cc98bf40370aec7d471d20211327227b2d9d7a4010cffaebfeecf'),
    ):
        ctx.require_normalized_code_sha256(
            "s13-owned-route", description,
            ctx.stage3b_function(relative, name, "s13-owned-route"), expected,
        )

    # Real-realm migration probes include semantic JS literals in their receipt.
    for name, expected in (
        ('tail_invocation_throws_use_the_activation_backtrace_and_catch_path', '7ca5bc9d7afee263e81c050dc0264e644a55899624c7db7518e7c71a9984e840'),
        ('tail_invocations_complete_the_frame_with_exact_call_operands', '9f4af243207a0cfaaf44bd506faab066561adbcda238206670abe93869c76ba1'),
        ('to_object_boxes_primitives_and_rejects_nullish_values', '98cae47df9508e3ff90235936c954d7bd5d3668322137c4c0b3b9e17911b9cb6'),
        ('captured_local_reuse_hook_is_limited_to_abrupt_resume_boundaries', 'c13a432a6612e19b55eec89ca13db3ae293304d61c9d176ffe7714e3d8606656'),
        ('append_uses_iterator_protocol_and_preserves_pending_throw_on_close', '5290c823809695a65035fdd8298d6378a5a81ffc8d0457e03c1f1002d0c557bf'),
    ):
        ctx.require_normalized_code_sha256(
            "s13-runtime-evidence", "real realm semantics, including JS literals, must remain checked",
            ctx.stage3j_source_function("src/engine/vm/tests.rs", name, "s13-runtime-evidence"), expected,
        )

    # Retired generic-call escape hatches cannot silently return behind a renamed leaf.
    for relative, name in (
        ("src/engine/vm/conversion_driver.rs", "invoke"),
        ("src/engine/vm/proxy_get_driver.rs", "invoke"),
    ):
        if re.search(r"\.\s*call_internal\s*\(", ctx.stage3b_function(relative, name, "s13-owned-route")):
            ctx.fail("s13-owned-route", "callback continuation must not re-enter generic synchronous JS dispatch")
