"""Contracts for the production ephemeron waiting table, without hash axioms."""

from source import item, method


def proof(source, value_source, template):
    types = ["#[derive(Clone, Copy, PartialEq, Eq)]\n" + item(value_source, r"pub struct Value\b"),
             item(source, r"struct Waiting\b"),
             item(source, r"pub\(super\) struct Waiters\b").replace("pub(super) ", "")]
    template = template.replace("// TYPES", "\n".join(types))
    value_scope = item(value_source, r"impl Value\b")
    template = template.replace("// METHOD bits", method(value_scope, "bits",
        "ensures result == self.0,", "result"))
    scope = item(source, r"impl Waiters\b")
    contracts = {
        "new": ("ensures result.valid(), result.by_key@.is_empty(),", "result"),
        "is_empty": ("ensures result == self.by_key@.is_empty(),", "result"),
        "defer": ("""
            requires old(self).valid(),
            ensures final(self).valid(),
                final(self).by_key@.dom() == old(self).by_key@.dom().insert(key.0),
                final(self).queued(key.0) == old(self).queued(key.0).push(value),
                forall|k: u64| k != key.0 ==> final(self).queued(k) == old(self).queued(k),
        """, None),
        "activate": ("""
            requires old(self).valid(),
            ensures final(self).valid(),
                final(self).by_key@ == old(self).by_key@.remove(key.0),
                match result {
                    Some(values) => old(self).by_key@.contains_key(key.0)
                        && values@ == old(self).queued(key.0),
                    None => !old(self).by_key@.contains_key(key.0),
                },
        """, "result"),
        "clear": ("ensures final(self).valid(), final(self).by_key@.is_empty(),", None),
    }
    for name, (contract, result) in contracts.items():
        marker = "// METHOD " + name + "\n"
        if template.count(marker) != 1:
            raise ValueError(f"missing or duplicate waiting-table marker: {name}")
        template = template.replace(marker, method(scope, name, contract, result))
    return template
