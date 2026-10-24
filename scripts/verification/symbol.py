"""Exact Symbol marking and ephemeron activation, including description edges."""

from source import item, method
import weakpass


def proof(heap_source, gc_source, object_source, symbol_source, arena_code, scan_code):
    specifications = """
        spec fn symbol_output(&self, value: Value, ephemerons: bool) -> Seq<Value> {
            let index = value.0 as u32;
            if self.symbols.marked_at(index) { Seq::empty() }
            else {
                let activated = if ephemerons { self.gc.ephemerons.queued(value.0) }
                    else { Seq::empty() };
                let description = self.symbols.records@[index as int].description;
                if description == NONE { activated }
                else { activated.push(Value((TAG_STRING << TAG_SHIFT) | description as u64)) }
            }
        }
    """
    trace = method(item(gc_source, r"impl Heap\b"), "mark_symbol", """
        requires kind_of(value.0) == Kind::Symbol,
            old(self).symbols.valid_marks(), old(self).symbols.live_at(value.0 as u32),
            old(self).gc.ephemerons.valid(),
        ensures final(self).symbols.valid_marks(),
            final(self).symbols.storage_unchanged(&old(self).symbols),
            final(self).symbols.marked_at(value.0 as u32),
            forall|i: u32| i != value.0 as u32 ==>
                final(self).symbols.marked_at(i) == old(self).symbols.marked_at(i),
            *final(self) == (Self { symbols: final(self).symbols, gc: final(self).gc, ..*old(self) }),
            final(self).gc.same_except_waiters(&old(self).gc), final(self).gc.ephemerons.valid(),
            final(self).gc.ephemerons.by_key@ ==
                if EPHEMERONS && !old(self).symbols.marked_at(value.0 as u32) {
                    old(self).gc.ephemerons.by_key@.remove(value.0)
                } else { old(self).gc.ephemerons.by_key@ },
            final(stack)@ == old(stack)@ + old(self).symbol_output(value, EPHEMERONS),
    """)
    code = weakpass.proof(heap_source, gc_source, object_source, arena_code, scan_code,
                          symbol_source, specifications + trace, include_scan=False)
    return ('#[path = "kernel.rs"]\nmod gc_kernel;\nmod heap_proof {\n' + code
            + '\n}\npub use heap_proof::StringId;\nfn main() {}\n')
