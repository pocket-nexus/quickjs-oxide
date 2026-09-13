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
    storage, ordinary, dispatch, runtime, heap, access, proxy_get, proxy_method, proxy_own, proxy_boolean, descriptor, proxy_call, ordinary_set, proxy_set, proxy_define, array_length, number, typed_element, typed_write, proxy_prototype = sources
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
            requirements.append((not re.search(r"\.(?:call_internal|call_value_internal|call_proxy|proxy_method|internal_get|internal_get_own_property|internal_has_property|internal_is_extensible|native_to_property_descriptor|internal_set|proxy_set|internal_define_own_property|try_special_set|prepare_set_array_length|define_own_property_in_realm|native_to_number|array_length_to_number|to_array_length|to_primitive|get_property_in_realm|typed_array_convert_element|native_to_bigint|internal_delete_property|internal_prevent_extensions|internal_get_prototype_of|internal_set_prototype_of)\s*\(", phase), "property and descriptor phases must yield observable requests to their driver"))
    for accepted, message in requirements:
        if not accepted:
            ctx.fail("ordinary-property-contract", message)
