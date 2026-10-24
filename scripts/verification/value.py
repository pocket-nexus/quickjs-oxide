"""Source contracts for the four GC Value kinds and their arena indices."""

from source import item, method


def proof(source, template, gc_source):
    scope = item(source, r"impl Value\b")
    names = ("INT32", "SPECIAL", "OBJECT", "STRING", "SYMBOL", "BIGINT", "SHIFT")
    declarations = [item(source, rf"const TAG_{name}\b") for name in names]
    declarations += ["#[derive(Clone, Copy, PartialEq, Eq)]\n" + item(source, r"pub struct Value\b"),
                     "#[derive(Clone, Copy, PartialEq, Eq)]\n#[repr(u8)]\n" + item(source, r"pub enum Kind\b"),
                     item(source, r"const KIND_TABLE\b")]
    # Instantiate the production identifier macro's record declaration. The
    # Debug/index impls are not dependencies of the selected GC functions.
    record = item(source, r"pub struct \$name\b")
    for name in ("ObjectId", "StringId", "SymbolId", "BigIntId"):
        if source.count("\n    " + name + "\n);") != 1:
            raise ValueError(f"missing identifier macro invocation for {name}")
        declarations.append("#[derive(Clone, Copy, PartialEq, Eq)]\n" + record.replace("$name", name))
    declarations.append(method(source, "boxed", """
        requires tag <= 0xFFFF,
        ensures result >> TAG_SHIFT == tag, result as u32 == payload,
    """, "result", before={"(tag << TAG_SHIFT) | payload as u64": """
        assert((((tag << 48) | payload as u64) >> 48) == tag
            && (((tag << 48) | payload as u64) as u32) == payload) by(bit_vector)
            requires tag <= 0xFFFF;
    """}))
    template = template.replace("// TYPES", "\n".join(declarations))
    contracts = {
        "bits": ("ensures result == self.0,", {}),
        "kind": ("ensures result == kind_of(self.0),", {"before": {
            "let tag = (self.0 >> TAG_SHIFT) as u32;": """
                let bits = self.0;
                assert(bits >> 48 <= 0xFFFF && bits & 3 <= 3) by(bit_vector);
            """,
            "KIND_TABLE[(index + special) & 15]": """
                assert(index + special <= 15);
                let position = (index + special) as usize;
                assert(position & 15 == position) by(bit_vector) requires position <= 15;
            """}}),
        "is_heap": ("ensures result == heap_kind(kind_of(self.0)),", {}),
        "payload_if": ("""ensures result == (if self.0 >> TAG_SHIFT == tag {
            Some(self.0 as u32) } else { None }),""", {}),
    }
    for name, ty, tag in (("object", "ObjectId", "OBJECT"), ("string", "StringId", "STRING"),
                          ("symbol", "SymbolId", "SYMBOL"), ("bigint", "BigIntId", "BIGINT")):
        contracts[name] = (f"ensures result.0 >> TAG_SHIFT == TAG_{tag}, result.0 as u32 == id.0,", {})
        contracts["as_" + name] = (f"""ensures result == (if kind_of(self.0) == Kind::{ty[:-2]} {{
            Some({ty}(self.0 as u32)) }} else {{ None }}),""", {})
    for name, (contract, options) in contracts.items():
        marker = "// METHOD " + name + "\n"
        if template.count(marker) != 1:
            raise ValueError(f"missing or duplicate Value method marker: {name}")
        template = template.replace(marker, method(scope, name, contract, "result", **options))
    template = template.replace("// PUSH_HEAP_VALUES", method(gc_source, "push_heap_values", """
        ensures final(stack)@ == old(stack)@ + values@.filter(|v: Value| heap_kind(kind_of(v.0))),
    """, loops={"while index < values.len()": """
        invariant index <= values@.len(),
            stack@ == old(stack)@ + values@.take(index as int).filter(|v: Value| heap_kind(kind_of(v.0))),
        decreases values@.len() - index,
    """}, before={"index += 1;": """
        let prefix = values@.take(index as int);
        assert(values@.take(index as int + 1) =~= prefix.push(value));
        prefix.lemma_filter_push(value, |v: Value| heap_kind(kind_of(v.0)));
    """}, after={"index += 1;\n    }": """
        assert(values@.take(index as int) =~= values@);
    """}))
    return template
