"""Compose explicit production root enumeration with the heap-value filter."""

from source import item, method
from records import record


def proof(gc_source, roots_code, value_code):
    # A new value-bearing field requires a new tracing policy, even if an old
    # method and its old postcondition would still agree on ignoring it.
    for name, fields in {
        "RootStack": {"values": "Vec<Value>"},
        "Handles": {"slots": "Vec<Value>", "free": "Vec<u32>"},
        "Roots": {"stack": "RootStack", "handles": "Handles", "fixed": "Vec<Value>"},
    }.items():
        _, parameters, observed = record(gc_source, name, "Unused",
            {field: (ty, ty) for field, ty in fields.items()})
        if parameters or observed != sorted(fields):
            raise ValueError(f"{name} fields changed; review the root tracing policy")
    declarations = [item(roots_code, r"pub struct RootStack\b"),
                    item(roots_code, r"pub struct Handles\b"),
                    item(gc_source, r"pub struct Roots\b")]
    values = item(item(roots_code, r"impl RootStack\b"), r"fn values\b")
    declarations.append("impl RootStack {\n" + values + "\n}")
    trace = method(item(gc_source, r"impl Roots\b"), "trace", """
        ensures final(stack)@ == old(stack)@ + self.heap_roots(),
    """)
    specifications = """
        spec fn heap_roots(&self) -> Seq<Value> {
            self.stack.values@.filter(|v: Value| heap_kind(kind_of(v.0)))
            + self.handles.slots@.filter(|v: Value| heap_kind(kind_of(v.0)))
            + self.fixed@.filter(|v: Value| heap_kind(kind_of(v.0)))
        }
        proof fn complete(&self, value: Value)
            ensures self.heap_roots().contains(value) == (heap_kind(kind_of(value.0))
                && (self.stack.values@.contains(value) || self.handles.slots@.contains(value)
                    || self.fixed@.contains(value))),
        {
            broadcast use vstd::seq_lib::group_seq_properties;
            root_filter_membership(self.stack.values@, value);
            root_filter_membership(self.handles.slots@, value);
            root_filter_membership(self.fixed@, value);
        }
    """
    declarations.append("""
        proof fn root_filter_membership(values: Seq<Value>, value: Value)
            ensures values.filter(|v: Value| heap_kind(kind_of(v.0))).contains(value)
                == (values.contains(value) && heap_kind(kind_of(value.0))),
        {
            let predicate = |v: Value| heap_kind(kind_of(v.0));
            if values.contains(value) && predicate(value) {
                let index = choose|i: int| 0 <= i < values.len() && values[i] == value;
                values.lemma_filter_contains(predicate, index);
            }
            if values.filter(predicate).contains(value) {
                values.lemma_filter_contains_rev(predicate, value);
            }
        }
    """)
    declarations.append("impl Roots {\n" + specifications + trace + "\n}")
    marker = "pub fn main() {}"
    if value_code.count(marker) != 1:
        raise ValueError("root enumeration proof lost its unique composition point")
    return value_code.replace(marker, "\n".join(declarations) + "\n" + marker)
