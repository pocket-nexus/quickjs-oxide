"""Compose checked Value and waiting-table operations with the real entry loop."""

from source import item, method


def proof(source, template, value_code, waiters_code, kernel_code, roots_code):
    for marker in ("// MARK_TYPES", "// MARKED", "// CONTAINS", "// SCAN_ENTRIES"):
        if template.count(marker) != 1:
            raise ValueError(f"missing or duplicate scan proof marker: {marker}")
    declarations = [item(roots_code, r"const SPECIAL_EMPTY\b"),
                    item(roots_code, r"spec fn boxed_spec\b"),
                    item(roots_code, r"impl Value\b"),
                    item(waiters_code, r"struct Waiting\b"),
                    item(waiters_code, r"struct Waiters\b"),
                    item(waiters_code, r"impl Waiting\b"),
                    item(waiters_code, r"impl Waiters\b"),
                    item(kernel_code, r"pub fn ephemeron_action\b"),
                    item(source, r"pub\(super\) struct ValueMarks\b").replace("pub(super) ", "")]
    template = template.replace("// MARK_TYPES", "\n".join(declarations))
    template = template.replace("// MARKED", method(source, "marked",
        "ensures result == bit_at(bits@, index),", "result"))
    template = template.replace("// CONTAINS", method(item(source, r"impl ValueMarks<'_>"), "contains",
        "ensures result == self.live(value),", "result"))
    template = template.replace("// SCAN_ENTRIES", method(source, "scan_entries", """
        requires slots@.len() % 2 == 0, old(waiters).valid(),
            forall|k: u64| old(waiters).by_key@.contains_key(k) ==> !marks.live(Value(k)),
        ensures final(waiters).valid(),
            final(stack)@ == old(stack)@ + ready(slots@, marks, slots@.len()),
            forall|k: u64| final(waiters).queued(k)
                == old(waiters).queued(k) + deferred(slots@, marks, slots@.len(), k),
            forall|i: int| old(stack)@.len() <= i < final(stack)@.len()
                ==> !marks.live(final(stack)@[i]),
            forall|i: int| #![trigger final(stack)@[i]] old(stack)@.len() <= i < final(stack)@.len() ==>
                exists|p: int| 0 <= p && p % 2 == 0 && p + 2 <= slots@.len()
                    && slots@[p].0 != Value::EMPTY.0 && slots@[p + 1] == final(stack)@[i]
                    && marks.live(slots@[p]),
            forall|k: u64| final(waiters).by_key@.contains_key(k) ==> !marks.live(Value(k)),
            forall|i: int| 0 <= i && i % 2 == 0 && i + 2 <= slots@.len()
                && slots@[i].0 != Value::EMPTY.0 ==>
                marks.live(slots@[i + 1]) || final(stack)@.contains(slots@[i + 1])
                    || final(waiters).queued(slots@[i].0).contains(slots@[i + 1]),
    """, loops={"while position < slots.len()": """
        invariant position <= slots@.len(), position % 2 == 0, slots@.len() % 2 == 0,
            waiters.valid(),
            stack@ == old(stack)@ + ready(slots@, marks, position as nat),
            forall|k: u64| waiters.queued(k)
                == old(waiters).queued(k) + deferred(slots@, marks, position as nat, k),
            forall|i: int| old(stack)@.len() <= i < stack@.len() ==> !marks.live(stack@[i]),
            forall|k: u64| waiters.by_key@.contains_key(k) ==> !marks.live(Value(k)),
        decreases slots@.len() - position,
    """}, before={"match ephemeron_action(empty, value_live, key_live) {": """
        reveal_with_fuel(ready, 2);
        reveal_with_fuel(deferred, 2);
        assert(ready(slots@, marks, (position + 2) as nat) ==
            if !empty && !value_live && key_live {
                ready(slots@, marks, position as nat).push(value)
            } else { ready(slots@, marks, position as nat) });
        assert forall|k: u64| deferred(slots@, marks, (position + 2) as nat, k) ==
            if k == key.0 && !empty && !value_live && !key_live {
                deferred(slots@, marks, position as nat, k).push(value)
            } else { deferred(slots@, marks, position as nat, k) } by {}
    """, "position += 2;": """
        reveal_with_fuel(ready, 2);
        reveal_with_fuel(deferred, 2);
        assert(stack@ =~= old(stack)@ + ready(slots@, marks, (position + 2) as nat));
        assert forall|k: u64| waiters.queued(k)
            == old(waiters).queued(k) + deferred(slots@, marks, (position + 2) as nat, k) by {
            assert(waiters.queued(k) =~= old(waiters).queued(k)
                + deferred(slots@, marks, (position + 2) as nat, k));
        }
    """}, after={"position += 2;\n    }": """
        assert forall|i: int| #![trigger stack@[i]] old(stack)@.len() <= i < stack@.len() implies
            exists|p: int| 0 <= p && p % 2 == 0 && p + 2 <= slots@.len()
                && slots@[p].0 != Value::EMPTY.0 && slots@[p + 1] == stack@[i]
                && marks.live(slots@[p]) by {
            let queued = ready(slots@, marks, slots@.len());
            assert(queued[i - old(stack)@.len()] == stack@[i]);
            ready_has_origin(slots@, marks, slots@.len(), stack@[i]);
        }
        assert forall|i: int| 0 <= i && i % 2 == 0 && i + 2 <= slots@.len()
            && slots@[i].0 != Value::EMPTY.0 implies
            marks.live(slots@[i + 1]) || stack@.contains(slots@[i + 1])
                || waiters.queued(slots@[i].0).contains(slots@[i + 1]) by {
            if !marks.live(slots@[i + 1]) {
                entry_covered(slots@, marks, slots@.len(), i as nat);
            }
        }
    """}))
    marker = "pub fn main() {}"
    if value_code.count(marker) != 1:
        raise ValueError("Value proof no longer has a unique composition point")
    return value_code.replace("use vstd::prelude::*;", "use vstd::prelude::*;\nuse std::collections::HashMap;").replace(
        marker, template + "\n" + marker)
