"""Property kernel ownership guards; behavioral coverage remains in Rust/JS."""
import re

FILES = (
    "src/engine/object/ordinary_storage.rs",
    "src/engine/object/ordinary.rs",
    "src/engine/object/internal_methods.rs",
    "src/engine/heap/runtime/mod.rs",
    "src/engine/heap/object_storage.rs",
    "src/engine/object/access.rs",
    "src/engine/object/internal_methods/get.rs",
    "src/engine/object/internal_methods/method.rs",
    "src/engine/object/internal_methods/own_property.rs",
    "src/engine/object/internal_methods/boolean.rs",
    "src/engine/value/conversion/descriptor.rs",
    "src/engine/object/internal_methods/call.rs",
    "src/engine/object/ordinary/set.rs",
    "src/engine/object/internal_methods/set.rs",
    "src/engine/object/internal_methods/define.rs",
    "src/engine/object/array_length.rs",
    "src/engine/value/conversion/number.rs",
    "src/engine/builtins/array_buffer/typed_array/element.rs",
    "src/engine/builtins/array_buffer/typed_array/write.rs",
    "src/engine/object/internal_methods/prototype.rs",
    "src/engine/builtins/object/prototype.rs",
    "src/engine/builtins/object.rs",
    "src/engine/builtins/object/property.rs",
    "src/engine/object/internal_methods/own_keys.rs",
    "src/engine/builtins/object/predicate.rs",
    "src/engine/builtins/object/definitions.rs",
    "src/engine/builtins/object/string.rs",
)


def check(ctx):
    if getattr(ctx, "self_test_marker_authorized", False):
        # The existing codec isolation fixtures deliberately omit the property
        # engine. Dedicated mutation tests exercise these guards on real files.
        return
    sources = []
    for relative in FILES:
        path = ctx.root / relative
        if path.is_symlink() or not path.is_file():
            ctx.fail("ordinary-property-source", f"missing regular source: {relative}")
            return
        sources.append(ctx.rust_code_only(path.read_text()))
    storage, ordinary, dispatch, runtime, heap, access, proxy_get, proxy_method, proxy_own, proxy_boolean, descriptor, proxy_call, ordinary_set, proxy_set, proxy_define, array_length, number, typed_element, typed_write, proxy_prototype, builtin_prototype, object_builtin, builtin_property, proxy_keys, builtin_predicate, builtin_definitions, builtin_string = sources
    compact = lambda text: re.sub(r"\s+", "", text)
    requirements = [
        (not re.search(r"pub(?:\([^)]*\))?\s+struct\s+OwnSlot", storage), "slot positions must remain private to the storage owner"),
        ("(ObjectKind::Ordinary,ObjectPayload::Ordinary)" in compact(storage), "ordinary eligibility must include the semantic class"),
        (not re.search(r"\.(?:call_internal|internal_set|materialize_auto_init_property)\s*\(", storage), "storage must not execute callbacks or observable internal methods"),
        ("ordinary_set_fast_path_available" not in ordinary + ordinary_set + dispatch, "ordinary Set must not pre-scan the prototype chain"),
        ("runtime.validate_object_and_key(&object,&key)?" in compact(ordinary_set) and "runtime.validate_value_domain(&value," in compact(ordinary_set) and "runtime.validate_value_domain(&receiver," in compact(ordinary_set), "Set must validate object, key, value and receiver domains"),
        ("rejected_object.as_ref().unwrap_or(&receiver)" in compact(ordinary_set), "Proxy forwarding diagnostics must use the rejected target"),
        ("if!failure.published{self.release_atoms(atoms)?;}" in compact(runtime), "only pre-publication failures may roll back replacement Atoms"),
    ]
    body, _, _ = ctx.unique_braced_item(heap, re.compile(r"fn\s+replace_object_slot_with_status\s*\([^{}]*\)\s*->[^{}]*\{"), "ordinary-property-transaction", "slot replacement")
    retained = body.find("retain_edges_transactionally")
    published = body.find("replace_retained_object_slot")
    requirements.append((0 <= retained < published, "replacement edges must be retained before publication"))
    read, _, _ = ctx.unique_braced_item(ordinary, re.compile(r"fn\s+prepare_ordinary_read\s*\([^{}]*\)\s*->[^{}]*\{"), "ordinary-property-read", "prepared property read")
    requirements.append((not re.search(r"\.(?:call_internal|call_value_internal|internal_get|get_property_in_realm|typed_array_convert_element|native_to_bigint|internal_delete_property|internal_prevent_extensions|internal_get_prototype_of|internal_set_prototype_of)\s*\(", read), "prepared property reads must return callbacks without invoking JavaScript"))
    requirements.append(("self.validate_object_and_key(object,key)?" in compact(read) and "self.validate_value_domain(&receiver," in compact(read), "prepared property reads must validate object, key and receiver domains"))
    for name in ("prepare_value_property_read", "prepare_string_property_read"):
        prepared, _, _ = ctx.unique_braced_item(access, re.compile(r"fn\s+" + name + r"\s*\([^{}]*\)\s*->[^{}]*\{"), "ordinary-property-read", name)
        requirements.append((not re.search(r"\.(?:call_internal|call_value_internal|internal_get|get_property_in_realm|get_value_property_in_realm|get_string_property_with_receiver)\s*\(", prepared), "primitive property preparation must return callbacks without invoking JavaScript"))
    primitive_delete, _, _ = ctx.unique_braced_item(access, re.compile(r"fn\s+primitive_delete_property\s*\([^{}]*\)\s*->[^{}]*\{"), "primitive-property-delete", "primitive Delete")
    requirements.append((not re.search(r"\.(?:call_internal|internal_delete_property|native_to_property_key|to_primitive)\s*\(", primitive_delete), "primitive Delete must not run key conversion or object internal methods"))
    has, _, _ = ctx.unique_braced_item(dispatch, re.compile(r"fn\s+prepare_has_property\s*\([^{}]*\)\s*->[^{}]*\{"), "ordinary-property-has", "prepared HasProperty")
    requirements.append(("PreparedHas::Proxy(current.clone())" in compact(has) and "self.validate_object_and_key(object,key)?" in compact(has), "prepared Has must validate its domain and return unresolved Proxy nodes"))
    protocols = (
        (builtin_prototype, ("start", "start_invocation", "prototype", "boolean")),
        (builtin_predicate, ("start", "key", "boolean", "defined", "descriptor", "prototype")),
        (builtin_string, ("start", "resume")),
        (builtin_definitions, ("start", "keys", "snapshot", "boolean", "next", "read", "converted", "defined")),
        (builtin_property, ("start", "key", "keys", "enumerate", "emit", "assign_source", "assign_next", "assign_read", "integrity_next", "converted", "defined", "descriptor", "boolean", "set", "read")),
        (proxy_keys, ("start", "method", "items", "check_next", "resume", "number", "boolean", "keys", "descriptor")),
        (object_builtin, ("finish_set_prototype_or_throw", "finish_define_property_or_throw", "object_default_to_string_tag", "call_object_is", "call_object_prototype_value_of")),
        (proxy_prototype, ("start", "method", "resume", "boolean", "prototype")),
        (proxy_get, ("start", "method", "resume", "descriptor")),
        (proxy_method, ("start", "read", "resume")),
        (proxy_own, ("start", "method", "resume", "descriptor", "extensible", "converted")),
        (proxy_boolean, ("start", "method", "resume", "boolean", "descriptor")),
        (descriptor, ("start", "next", "has", "read")),
        (dispatch, ("prepare_has_property", "prepare_typed_array_set")),
        (proxy_call, ("start", "read", "resume")),
        (ordinary_set, ("start", "walk", "special_own", "receiver", "define", "advance", "forward", "special", "descriptor", "defined", "array_length")),
        (proxy_set, ("start", "method", "resume", "set", "descriptor")),
        (proxy_define, ("start", "method", "resume", "defined", "descriptor")),
        (array_length, ("start", "number")),
        (number, ("start", "from_primitive", "resume")),
        (typed_element, ("start", "from_primitive", "resume")),
        (typed_write, ("set", "define", "element")),
    )
    for source, names in protocols:
        for name in names:
            phase, _, _ = ctx.unique_braced_item(source, re.compile(r"fn\s+" + name + r"\s*\([^{}]*\)\s*->[^{}]*\{"), "proxy-property-step", name)
            requirements.append((not re.search(r"\.(?:call_internal|call_value_internal|call_proxy|proxy_method|internal_get|internal_get_own_property|internal_has_property|internal_is_extensible|native_to_property_descriptor|internal_set|proxy_set|internal_define_own_property|try_special_set|prepare_set_array_length|define_own_property_in_realm|native_to_number|array_length_to_number|to_array_length|to_primitive|get_property_in_realm|typed_array_convert_element|native_to_bigint|internal_delete_property|internal_prevent_extensions|internal_get_prototype_of|internal_set_prototype_of|internal_own_property_keys|get_value_property_in_realm)\s*\(", phase), "property and descriptor phases must yield observable requests to their driver"))
    for accepted, message in requirements:
        if not accepted:
            ctx.fail("ordinary-property-contract", message)
    check_synchronous_domains(ctx)

# These are the synchronous domains completed in S05. Their old consumers may
# still synchronously drive a Step, but an individual phase may only request JS.
# Keep this inventory explicit: deleting a domain must not silently narrow a scan.
S05_PROTOCOLS = {
    "src/engine/builtins/array/callback.rs": ("CallbackStep", "CallbackResume"),
    "src/engine/builtins/array/mutation.rs": ("MutationStep", "MutationResume"),
    "src/engine/builtins/array/species.rs": ("SpeciesStep", "SpeciesResume"),
    "src/engine/builtins/array/copy.rs": ("CopyStep", "CopyResume"),
    "src/engine/builtins/array/sort.rs": ("SortStep", "SortResume"),
    "src/engine/builtins/math/operation.rs": ("MathStep", "MathResume"),
    "src/engine/builtins/math/sum.rs": ("SumStep", "SumResume"),
    "src/engine/builtins/primitive/constructor.rs": ("PrimitiveConstructorStep", "PrimitiveConstructorResume"),
    "src/engine/builtins/primitive/globals.rs": ("GlobalStep", "GlobalResume"),
    "src/engine/builtins/primitive/numeric.rs": ("NumericStep", "NumericResume"),
    "src/engine/builtins/primitive/text.rs": ("ScalarTextStep", "ScalarTextResume"),
    "src/engine/builtins/date/constructor/operation.rs": ("DateConstructorStep", "DateConstructorResume"),
    "src/engine/builtins/date/prototype/operation.rs": ("DatePrototypeStep", "DatePrototypeResume"),
    "src/engine/builtins/error/operation.rs": ("ErrorStep", "ErrorResume"),
    "src/engine/builtins/error/aggregate.rs": ("AggregateStep", "AggregateResume"),
    "src/engine/builtins/function/dynamic.rs": ("DynamicFunctionStep", "DynamicFunctionResume"),
    "src/engine/builtins/function/text.rs": ("FunctionTextStep", "FunctionTextResume"),
    "src/engine/vm/environment_bindings/operation.rs": ("EnvironmentStep", "EnvironmentResume"),
    "src/engine/vm/numeric/operation.rs": ("NumericStep", "NumericResume"),
    "src/engine/vm/for_in/operation.rs": ("ForInStep", "ForInResume"),
    "src/engine/object/object_literal/element.rs": ("LiteralDefinitionStep", "LiteralDefinitionResume"),
}
S05_ROUTES = {
    "src/engine/builtins/array.rs": ("callback::finish(", "callback::CallbackStep::start(", "sort::finish(", "sort::SortStep::start("),
    "src/engine/vm/host_bridge/dynamic_environment.rs": ("operation::finish(", "EnvironmentStep::has_binding(", "EnvironmentStep::get(", "EnvironmentStep::put("),
    "src/engine/vm/numeric_execution.rs": ("NumericStep::start(", "NumericStep::Primitive", "resume.resume(host.to_primitive(value,hint)?)?", "NumericStep::HtmlDda"),
    "src/engine/vm/for_in.rs": ("operation::ForInStep::start(", "operation::ForInStep::next(", "operation::finish("),
    "src/engine/vm/with_driver.rs": ("EnvironmentStep::has_binding(", "proxy_get_driver::start_environment("),
    "src/engine/vm/environment_driver.rs": ("EnvironmentStep::get(", "EnvironmentStep::put(", "proxy_get_driver::start_environment(", "proxy_get_driver::start_vm_call("),
    "src/engine/vm/private_access.rs": ("private_bindings::branded_receiver(", "proxy_get_driver::start_vm_call("),
    "src/engine/vm/construct_driver.rs": ("runtime.validate_class_parent(", "proxy_get_driver::start_class_parent(", "proxy_get_driver::start_public_field("),
    "src/engine/vm/array_driver.rs": ("LiteralDefinitionStep::start(", "proxy_get_driver::start_literal_definition("),
    "src/engine/vm/frame_operations.rs": ("RunExit::Numeric(kind)", "NumericStep::start(", "proxy_get_driver::start_numeric(", "RunExit::ForIn(next)", "proxy_get_driver::start_for_in_query("),
    "src/engine/vm/run.rs": ("RunExit::Numeric(kind)", "Instruction::ForInStart=>returnOk(RunExit::ForIn(false))", "Instruction::ForInNext=>returnOk(RunExit::ForIn(true))"),
    "src/engine/vm/proxy_get_driver.rs": ("fnstart_numeric(", "fnstart_for_in_query(", "fnstart_environment(", "fnstart_class_parent(", "fnstart_public_field(", "fnstart_literal_definition("),
    "src/engine/vm/proxy_get_driver/request.rs": ("modarray;", "modscalar;", "modvm;", "enumResume", "enumStep", "Self::Environment(resume)", "Self::VmNumeric(resume)", "Self::ForIn(resume)"),
    "src/engine/vm/proxy_get_driver/request/array.rs": ("From<crate::engine::builtins::ArrayCallbackStep>", "From<crate::engine::builtins::ArraySortStep>", "Self::ArrayCopy", "Self::Call"),
    "src/engine/vm/proxy_get_driver/request/scalar.rs": ("From<crate::engine::builtins::MathStep>", "From<crate::engine::builtins::PrimitiveConstructorStep>", "From<crate::engine::builtins::DatePrototypeStep>"),
    "src/engine/vm/proxy_get_driver/request/object.rs": ("LiteralDefinitionStep>forStep", "Self::DefineOrdinary{", "Resume::LiteralDefinition(resume)"),
    "src/engine/vm/proxy_get_driver/request/vm.rs": ("EnvironmentStep>forStep", "NumericStep>forStep", "ForInStep>forStep", "Self::NumericComplete{value,previous}", "Self::ForInComplete{value,done}"),
}
S05_FILES = tuple(dict.fromkeys((*S05_PROTOCOLS, *S05_ROUTES)))
SYNC_CALLBACK = re.compile(
    r"\.(?:call_internal|call_value_internal|construct_internal|construct_with_new_target|"
    r"internal_get|internal_get_own_property|internal_has_property|internal_has_own_property|"
    r"internal_is_extensible|internal_set|internal_define_own_property|internal_delete_property|"
    r"internal_get_prototype_of|internal_set_prototype_of|internal_own_property_keys|"
    r"internal_snapshot_own_property_is_enumerable|get_property_in_realm|get_value_property_in_realm|"
    r"native_to_(?:number|string|bigint|index|integer|property_key|property_descriptor)|"
    r"to_primitive|define_own_property_in_realm|execute_indirect_string_eval|execute_bytecode_internal)\s*\("
)


def production_code(ctx, code):
    """Remove braced test modules, without hiding production items after them."""
    pattern = re.compile(r"#\s*\[\s*cfg\s*\([^\]]*\btest\b[^\]]*\)\s*\]\s*mod\s+\w+\s*\{")
    spans = []
    for match in pattern.finditer(code):
        _, start, end = ctx.braced_item_from_match(code, match, "synchronous-domain-contract", "test module")
        if start >= 0:
            spans.append((start, end))
    for start, end in reversed(spans):
        code = code[:start] + ctx.blank(code[start:end]) + code[end:]
    return code


def check_synchronous_domains(ctx):
    sources = {}
    for relative in S05_FILES:
        path = ctx.root / relative
        if path.is_symlink() or not path.is_file():
            ctx.fail("synchronous-domain-source", f"missing regular source: {relative}")
            continue
        sources[relative] = production_code(ctx, ctx.rust_code_only(path.read_text()))
    for relative, (step, resume) in S05_PROTOCOLS.items():
        code = sources.get(relative, "")
        if not re.search(r"enum\s+" + step + r"\b", code) or not re.search(r"(?:struct|enum)\s+" + resume + r"\b", code):
            ctx.fail("synchronous-domain-contract", f"{relative}: missing owned Step/Resume protocol")
        # The one legacy consumer remains deliberately synchronous. Checking its
        # typed input prevents an unrelated helper being used as an exemption.
        finish = re.compile(r"fn\s+finish\s*\([^{}]*\)\s*->[^{}]*\{")
        matches = [match for match in finish.finditer(code)
                   if re.search(r"\bstep\s*:\s*" + step + r"\b", match.group())]
        if len(matches) > 1:
            ctx.fail("synchronous-domain-contract", f"{relative}: ambiguous synchronous consumer")
        for match in reversed(matches):
            body, start, end = ctx.braced_item_from_match(code, match, "synchronous-domain-contract", "legacy Step consumer")
            if not re.search(r"\bstep\s*:\s*" + step + r"\b", body) or f"{step}::" not in body:
                ctx.fail("synchronous-domain-contract", f"{relative}: legacy consumer lost its typed Step input")
            code = code[:start] + ctx.blank(code[start:end]) + code[end:]
        if SYNC_CALLBACK.search(code) or re.search(r"\b(?:RuntimeVmHost|VmHost|Future|poll_fn)\b", code):
            ctx.fail("synchronous-domain-contract", f"{relative}: a domain phase synchronously waits on JavaScript")
        if re.search(r"use\s+(?:super::)+\*\s*;", code):
            ctx.fail("synchronous-domain-contract", f"{relative}: domain dependencies must be explicit")
    for relative, fragments in S05_ROUTES.items():
        code = re.sub(r"\s+", "", sources.get(relative, ""))
        for fragment in fragments:
            if fragment not in code:
                ctx.fail("synchronous-domain-route", f"{relative}: production route missing {fragment}")
    for relative in ("src/engine/vm/proxy_get_driver/request/array.rs", "src/engine/vm/proxy_get_driver/request/scalar.rs", "src/engine/vm/proxy_get_driver/request/vm.rs"):
        if SYNC_CALLBACK.search(sources.get(relative, "")):
            ctx.fail("synchronous-domain-route", f"{relative}: adapter must only translate typed requests")
    vm = re.sub(r"\s+", "", sources.get("src/engine/vm/proxy_get_driver/request/vm.rs", ""))
    if not re.search(r"T::Enumerable\{object,key,resume,?\}=>Self::SnapshotEnumerable\{object,key,resume:Resume::ForIn\(resume\),?\}", vm):
        ctx.fail("synchronous-domain-route", "for-in must preserve snapshot enumerable reads")
    if not re.search(r"T::Read\{object,key,receiver,resume,?\}=>Self::Read\{receiver,object,key,resume:Resume::Environment\(resume\),?\}", vm):
        ctx.fail("synchronous-domain-route", "environment lookup must retain its selected receiver")
