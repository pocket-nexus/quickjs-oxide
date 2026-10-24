"""Production arena operations; only unreachable panic formatting is erased."""

from source import item, method


def proof(source, template):
    scope = item(source, r"impl<T: Record> Arena<T>")
    declarations = [item(source, r"pub struct Arena<T>"),
                    item(source, r"const CHECK_LIVE\b")]
    template = template.replace("// TYPES", "\n".join(declarations))
    template = template.replace("// BIT", method(source, "bit", """
        ensures result.0 == index / 64, result.1 == 1u64 << (index % 64), result.1 != 0,
    """, "result", before={"((index / 64) as usize, 1u64 << (index % 64))": """
        assert(index % 64 < 64 && (1u64 << (index % 64)) != 0) by(bit_vector);
    """}))
    contracts = {
        "mark_bits": ("ensures result@ == self.marks@,", "result", {}),
        "new": ("""
            ensures result.valid_marks(), result.records@.len() == 0,
                result.live@.len() == 0, result.marks@.len() == 0,
                result.free_count == 0, result.live_count == 0,
                result.cursor == 0, result.extra == 0, result.empty_quarantine(),
        """, "result", {}),
        "is_live": ("ensures result == self.live_at(index),", "result", {}),
        "check": ("requires self.live_at(index),", None, {"panic_condition": "self.is_live(index)"}),
        "get": ("requires self.valid_marks(), self.live_at(index), ensures *result == self.records@[index as int],", "result", {}),
        "get_mut": ("""
            requires old(self).valid_marks(), old(self).live_at(index),
            ensures *result == old(self).records@[index as int],
                final(self).records@ == old(self).records@.update(index as int, *final(result)),
                final(self).live@ == old(self).live@, final(self).marks@ == old(self).marks@,
                final(self).free_count == old(self).free_count,
                final(self).cursor == old(self).cursor, final(self).live_count == old(self).live_count,
                final(self).extra == old(self).extra, final(self).quarantine_unchanged(old(self)),
        """, "result", {}),
        "mark": ("""
            requires old(self).valid_marks(), old(self).live_at(index),
            ensures final(self).valid_marks(), final(self).storage_unchanged(old(self)),
                final(self).marks@.len() == old(self).marks@.len(),
                final(self).marked_at(index), result == !old(self).marked_at(index),
                forall|i: u32| i != index ==> final(self).marked_at(i) == old(self).marked_at(i),
        """, "result", {"after": {"*bits = marked;": """
            fresh_single_bit(old(self).marks@[word as int], index);
            assert(self.storage_unchanged(old(self)));
            assert forall|i: u32| self.live_at(i) implies (i as int) < self.records@.len() by {
                assert(old(self).live_at(i));
            }
            assert forall|i: u32| i != index implies self.marked_at(i) == old(self).marked_at(i) by {
                if i / 64 == index / 64 {
                    preserve_other_bit(old(self).marks@[word as int], index, i);
                }
            }
        """}}),
        "is_marked": ("ensures result == self.marked_at(index),", "result", {}),
        "clear_marks": ("""
            ensures final(self).storage_unchanged(old(self)),
                final(self).marks@.len() == old(self).marks@.len(),
                forall|i: u32| !final(self).marked_at(i),
        """, None, {"loops": {"while word < self.marks.len()": """
            invariant self.storage_unchanged(old(self)),
                self.marks@.len() == old(self).marks@.len(), word <= self.marks@.len(),
                forall|j: int| 0 <= j < word ==> self.marks@[j] == 0,
            decreases self.marks@.len() - word,
        """}, "after": {"word += 1;\n        }": """
            assert forall|i: u32| !self.marked_at(i) by {
                assert(0u64 & (1u64 << (i % 64)) == 0) by(bit_vector);
            }
        """}}),
    }
    for name, (contract, result, options) in contracts.items():
        marker = "// METHOD " + name + "\n"
        if template.count(marker) != 1:
            raise ValueError(f"missing or duplicate arena method marker: {name}")
        template = template.replace(marker, method(scope, name, contract, result, **options))
    return template
