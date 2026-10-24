"""Verus contracts for the exact production RootStack and Handles methods."""

from source import item, method


def proof(gc_source, value_source, template):
    scopes = {name: item(gc_source, rf"impl {name}\b") for name in ("RootStack", "Handles")}
    scopes["Value"] = item(value_source, r"impl Value\b")
    types = [item(value_source, r"const TAG_SHIFT\b"),
             item(value_source, r"const TAG_SPECIAL\b"),
             item(value_source, r"const SPECIAL_EMPTY\b")]
    boxed = item(value_source, r"const fn boxed\b")
    types.append(boxed.replace("const fn boxed", "spec fn boxed_spec"))
    types.append("#[verifier::when_used_as_spec(boxed_spec)]\n" + method(
        value_source, "boxed", "ensures result == boxed_spec(tag, payload),", "result"))
    for name, source in (("Value", value_source), ("RootMark", gc_source), ("Handle", gc_source)):
        types.append("#[derive(Clone, Copy)]\n" + item(source, rf"pub struct {name}\b"))
    for name in ("RootStack", "Handles"):
        types.append(item(gc_source, rf"pub struct {name}\b"))
    template = template.replace("// TYPES", "\n".join(types))
    empty = item(scopes["Value"], r"pub const EMPTY\b")
    initializer = empty.split("=", 1)[1].strip().removesuffix(";")
    template = template.replace("// EMPTY", (
        "#[verifier::when_used_as_spec(SPEC_EMPTY)]\n"
        "exec const EMPTY: Value ensures Self::EMPTY == Self::SPEC_EMPTY { " + initializer + " }"))
    template = template.replace("// SPEC_EMPTY", "spec const SPEC_EMPTY: Value = " + initializer.replace("boxed(", "boxed_spec(") + ";")
    contracts = {
        "Value.is_empty": ("ensures result == (self.0 == Value::EMPTY.0),", "result", {}),
        "RootStack.mark": ("ensures result.0 == self.values@.len(),", "result", {}),
        "RootStack.push": ("ensures final(self).values@ == old(self).values@.push(value),", None, {}),
        "RootStack.pop_one": ("ensures final(self).values@ == old(self).values@.take(if old(self).values@.len() == 0 { 0 } else { old(self).values@.len() - 1 }),", None, {}),
        "RootStack.truncate": ("requires mark.0 <= old(self).values@.len(), ensures final(self).values@ == old(self).values@.take(mark.0 as int),", None, {}),
        "RootStack.get": ("requires index < self.values@.len(), ensures result == self.values@[index as int],", "result", {}),
        "RootStack.set": ("requires index < old(self).values@.len(), ensures final(self).values@ == old(self).values@.update(index as int, value),", None, {}),
        "RootStack.len": ("ensures result == self.values@.len(),", "result", {}),
        "RootStack.is_empty": ("ensures result == (self.values@.len() == 0),", "result", {}),
        "RootStack.values": ("ensures result@ == self.values@,", "result", {}),
        "Handles.create": ("""
            requires old(self).valid(), value.0 != Value::EMPTY.0,
                old(self).free@.len() > 0 || old(self).slots@.len() < u32::MAX,
            ensures final(self).valid(), final(self).active(result),
                final(self).slots@[result.0 as int] == value,
                !old(self).active(result),
                forall|h: Handle| old(self).active(h) ==> final(self).active(h)
                    && final(self).slots@[h.0 as int] == old(self).slots@[h.0 as int],
        """, "result", {"before": {"return Handle(index);": """
            assert(self.free@.no_duplicates());
            assert(!self.free@.contains(index)) by {
                assert forall|j: int| 0 <= j < self.free@.len()
                    implies self.free@[j] != index by {
                    assert(old(self).free@[j] != old(self).free@[old(self).free@.len() - 1]);
                }
            }
            assert forall|i: int| 0 <= i < self.slots@.len() implies
                (self.slots@[i].0 == Value::EMPTY.0) == self.free@.contains(i as u32) by {
                if i != index as int && old(self).free@.contains(i as u32) {
                    let j = choose|j: int| 0 <= j < old(self).free@.len()
                        && old(self).free@[j] == i as u32;
                    assert(j < self.free@.len());
                    assert(self.free@[j] == i as u32);
                }
            }
        """}}),
        "Handles.get": ("requires self.valid(), self.active(handle), ensures result == self.slots@[handle.0 as int],", "result", {}),
        "Handles.set": ("""
            requires old(self).valid(), old(self).active(handle), value.0 != Value::EMPTY.0,
            ensures final(self).valid(), final(self).free@ == old(self).free@,
                final(self).slots@ == old(self).slots@.update(handle.0 as int, value),
        """, None, {}),
        "Handles.release": ("""
            requires old(self).valid(), old(self).active(handle),
            ensures final(self).valid(), !final(self).active(handle),
                final(self).slots@ == old(self).slots@.update(handle.0 as int, Value::EMPTY),
                final(self).free@ == old(self).free@.push(handle.0),
        """, None, {"after": {"self.free.push(handle.0);": """
            assert(!old(self).free@.contains(handle.0));
            assert(self.free@.no_duplicates());
            assert forall|i: int| 0 <= i < self.slots@.len() implies
                (self.slots@[i].0 == Value::EMPTY.0) == self.free@.contains(i as u32) by {
                assert((i as u32 == handle.0) == (i == handle.0 as int));
                if i != handle.0 as int {
                    assert(self.slots@[i] == old(self).slots@[i]);
                    assert(old(self).free@.contains(i as u32) == self.free@.contains(i as u32));
                }
            }
        """}}),
        "Handles.live_count": ("""
            requires self.valid(),
            ensures result == self.live_indices().len(),
        """, "result", {"before": {"self.slots.len() - self.free.len()": "self.count_partition();"}}),
    }
    for qualified, (contract, result, options) in contracts.items():
        scope, name = qualified.split(".")
        marker = "// METHOD " + qualified + "\n"
        if template.count(marker) != 1:
            raise ValueError(f"missing or duplicate method marker: {qualified}")
        template = template.replace(marker, method(scopes[scope], name, contract, result, **options))
    if "// METHOD" in template:
        raise ValueError("unexpanded source method")
    return template
