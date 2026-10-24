"""Proof composition for the actual Heap/WeakPass bodies and complete records."""

from records import record
from source import item, masked, method


def proof(heap_source, gc_source, object_source, arena_code, scan_code):
    declarations = []

    def declare(source, name, prefix, retained, dependencies=()):
        declaration, parameters, _ = record(source, name, prefix, retained)
        parameters = list(dependencies) + parameters
        ty = name + ("<" + ", ".join(parameters) + ">" if parameters else "")
        declaration = declaration.replace("struct " + name, "struct " + ty, 1)
        declarations.append(declaration)
        return ty, parameters

    internal, ip = declare(object_source, "Internal", "I", {
        "tag": ("InternalTag", "InternalTag"), "slots": ("Vec<Value>", "Vec<Value>"),
    })
    payloads, pp = declare(object_source, "PayloadArenas", "P", {
        "internals": ("Arena<Internal>", "Arena<" + internal + ">"),
    }, ip)
    state, gp = declare(gc_source, "GcState", "G", {
        "weak": ("Vec<u32>", "Vec<u32>"), "ephemerons": ("Waiters", "Waiters"),
    })
    heap, hp = declare(heap_source, "Heap", "H", {
        "objects": ("Arena<ObjectHeader>", "Arena<O>"),
        "strings": ("Arena<StringRecord>", "Arena<S>"),
        "symbols": ("Arena<SymbolRecord>", "Arena<Y>"),
        "bigints": ("Arena<BigIntRecord>", "Arena<B>"),
        "payloads": ("PayloadArenas", payloads), "gc": ("GcState", state),
    }, ["O", "S", "Y", "B", *pp, *gp])
    parameters = ", ".join(hp)
    weak = item(gc_source, r"struct WeakPass<'a>")
    if weak.count("mut Heap,") != 1:
        raise ValueError("WeakPass heap field changed")
    declarations.append(weak.replace("WeakPass<'a>", "WeakPass<'a, " + parameters + ">")
                            .replace("mut Heap,", "mut " + heap + ","))
    tag_derive = "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum InternalTag"
    if masked(object_source).count(tag_derive) != 1:
        raise ValueError("InternalTag no longer has the checked derived equality")
    declarations.append("#[derive(Clone, Copy, PartialEq, Eq, Structural)]\n"
                        + item(object_source, r"pub enum InternalTag\b"))
    declarations.append(item(arena_code, r"pub struct Arena<T>"))
    declarations.append(item(arena_code, r"const CHECK_LIVE\b"))
    declarations.append(item(arena_code, r"fn bit\b"))
    arena_scope = item(arena_code, r"impl<T> Arena<T>")
    arena_items = [item(arena_scope, rf"spec fn {name}\b")
                   for name in ("valid_marks", "live_at", "marked_at")]
    arena_items += [item(arena_scope, rf"fn {name}\b")
                    for name in ("mark_bits", "is_live", "is_marked", "check", "get")]
    declarations.append("impl<T> Arena<T> {\n" + "\n".join(arena_items) + "\n}")
    declarations.append("impl<" + ", ".join(gp) + "> " + state + " {\n" + """
        spec fn same_except_waiters(&self, previous: &Self) -> bool {
            *self == Self { ephemerons: self.ephemerons, ..*previous }
        }
    }""")
    heap_specs = """
        spec fn same_except_waiters(&self, previous: &Self) -> bool {
            *self == Self { gc: self.gc, ..*previous }
                && self.gc.same_except_waiters(&previous.gc)
        }
        spec fn live(&self, value: Value) -> bool {
            match kind_of(value.0) {
                Kind::Object => self.objects.marked_at(value.0 as u32),
                Kind::String => self.strings.marked_at(value.0 as u32),
                Kind::Symbol => self.symbols.marked_at(value.0 as u32),
                Kind::BigInt => self.bigints.marked_at(value.0 as u32),
                _ => true,
            }
        }
        spec fn scan_valid(&self) -> bool {
            self.payloads.internals.valid_marks() && self.gc.ephemerons.valid()
            && (forall|k: u64| self.gc.ephemerons.by_key@.contains_key(k) ==> !self.live(Value(k)))
            && (forall|p: int| 0 <= p < self.gc.weak@.len() ==>
                self.payloads.internals.live_at(self.gc.weak@[p])
                && self.payloads.internals.marked_at(self.gc.weak@[p])
                && (self.payloads.internals.records@[self.gc.weak@[p] as int].tag == InternalTag::WeakMap
                    ==> self.payloads.internals.records@[self.gc.weak@[p] as int].slots@.len() % 2 == 0))
        }
        spec fn map_slots(&self, p: int) -> Seq<Value> {
            self.payloads.internals.records@[self.gc.weak@[p] as int].slots@
        }
        spec fn pair(&self, p: int, i: int) -> bool {
            0 <= p < self.gc.weak@.len()
            && self.payloads.internals.records@[self.gc.weak@[p] as int].tag == InternalTag::WeakMap
            && 0 <= i && i % 2 == 0 && i + 2 <= self.map_slots(p).len()
            && self.map_slots(p)[i].0 != Value::EMPTY.0
        }
        spec fn covered(&self, p: int, i: int, queue: Seq<Value>) -> bool {
            self.live(self.map_slots(p)[i + 1]) || queue.contains(self.map_slots(p)[i + 1])
                || self.gc.ephemerons.queued(self.map_slots(p)[i].0).contains(self.map_slots(p)[i + 1])
        }
        spec fn ready_from(&self, start: nat, end: nat, value: Value) -> bool {
            exists|p: int, i: int| start <= p < end && self.pair(p, i)
                && self.payloads.internals.marked_at(self.gc.weak@[p])
                && self.map_slots(p)[i + 1] == value && self.live(self.map_slots(p)[i])
        }
        spec fn waiting_from(&self, end: nat, key: u64, value: Value) -> bool {
            exists|p: int, i: int| 0 <= p < end && self.pair(p, i)
                && self.payloads.internals.marked_at(self.gc.weak@[p])
                && self.map_slots(p)[i].0 == key && self.map_slots(p)[i + 1] == value
        }
        spec fn waiting(&self, key: u64, value: Value) -> bool {
            self.gc.ephemerons.queued(key).contains(value)
        }
    """
    heap_marked = method(item(gc_source, r"impl Heap\b"), "value_marked",
                         "ensures result == self.live(value),", "result")
    declarations.append("impl<" + parameters + "> " + heap + " {\n" + heap_specs + heap_marked + "\n}")
    weak_scope = item(gc_source, r"impl EphemeronTrace for WeakPass<'_>")
    scan = method(weak_scope, "scan", """
        requires old(self).heap.scan_valid(), old(self).stack@.len() == 0,
            old(self).scanned <= old(self).heap.gc.weak@.len(),
            forall|p: int, i: int| 0 <= p < old(self).scanned && old(self).heap.pair(p, i)
                ==> old(self).heap.covered(p, i, old(self).stack@),
            forall|k: u64, v: Value| #[trigger] old(self).heap.waiting(k, v)
                ==> old(self).heap.waiting_from(old(self).scanned as nat, k, v),
        ensures final(self).heap.same_except_waiters(old(self).heap),
            final(self).heap.scan_valid(),
            final(self).scanned == final(self).heap.gc.weak@.len(),
            result == (final(self).stack@.len() > 0),
            forall|k: u64, v: Value| #[trigger] final(self).heap.waiting(k, v)
                ==> final(self).heap.waiting_from(final(self).scanned as nat, k, v),
            forall|i: int| 0 <= i < final(self).stack@.len()
                ==> !final(self).heap.live(final(self).stack@[i]),
            forall|i: int| 0 <= i < final(self).stack@.len() ==>
                final(self).heap.ready_from(old(self).scanned as nat,
                    final(self).heap.gc.weak@.len(), final(self).stack@[i]),
            forall|p: int, i: int| final(self).heap.pair(p, i)
                ==> final(self).heap.covered(p, i, final(self).stack@),
            !result ==> forall|p: int, i: int| final(self).heap.pair(p, i)
                && final(self).heap.live(final(self).heap.map_slots(p)[i])
                ==> final(self).heap.live(final(self).heap.map_slots(p)[i + 1]),
    """, "result", loops={"for position in self.scanned..heap.gc.weak.len()": """
        invariant heap.same_except_waiters(old(self).heap), heap.scan_valid(),
            self.scanned == old(self).scanned,
            forall|k: u64, v: Value| #[trigger] heap.waiting(k, v)
                ==> heap.waiting_from(position as nat, k, v),
            forall|i: int| 0 <= i < self.stack@.len() ==> !heap.live(self.stack@[i]),
            forall|i: int| 0 <= i < self.stack@.len() ==>
                heap.ready_from(old(self).scanned as nat, position as nat, self.stack@[i]),
            forall|p: int, i: int| 0 <= p < position && heap.pair(p, i)
                ==> heap.covered(p, i, self.stack@),
    """}, bindings={"if internal.tag == InternalTag::WeakMap {": {
        "before_scan_heap": "**heap", "before_scan_stack": "self.stack@",
    }}, before={
        "!self.stack.is_empty()": """
            if self.stack@.len() == 0 {
                assert forall|p: int, i: int| heap.pair(p, i) && heap.live(heap.map_slots(p)[i])
                    implies heap.live(heap.map_slots(p)[i + 1]) by {
                    let key = heap.map_slots(p)[i];
                    let value = heap.map_slots(p)[i + 1];
                    assert(heap.covered(p, i, self.stack@));
                    if !heap.live(value) {
                        assert(heap.gc.ephemerons.queued(key.0).contains(value));
                        assert(heap.gc.ephemerons.by_key@.contains_key(key.0));
                        assert(!heap.live(Value(key.0)));
                    }
                }
            }
        """,
        "if internal.tag == InternalTag::WeakMap {": """
            assert forall|p: int, i: int| 0 <= p < position && before_scan_heap.pair(p, i)
                implies before_scan_heap.covered(p, i, before_scan_stack) by {}
            assert forall|i: int| 0 <= i < before_scan_stack.len() implies
                before_scan_heap.ready_from(old(self).scanned as nat, position as nat, before_scan_stack[i]) by {}
            assert forall|k: u64, v: Value| #[trigger] before_scan_heap.waiting(k, v)
                implies before_scan_heap.waiting_from(position as nat, k, v) by {}
        """,
        "scan_entries(&internal.slots, &marks, self.stack, &mut heap.gc.ephemerons);": """
            assert forall|v: Value| marks.live(v) == heap.live(v) by {}
        """,
    }, after={"scan_entries(&internal.slots, &marks, self.stack, &mut heap.gc.ephemerons);\n            }": """
        assert(heap.same_except_waiters(&before_scan_heap));
        assert forall|p: int, i: int| 0 <= p <= position && heap.pair(p, i) implies
            heap.covered(p, i, self.stack@) by {
            if p < position {
                assert(before_scan_heap.pair(p, i));
                assert(before_scan_heap.covered(p, i, before_scan_stack));
            }
        }
        assert forall|i: int| 0 <= i < self.stack@.len() implies
            heap.ready_from(old(self).scanned as nat, (position + 1) as nat, self.stack@[i]) by {
            if i < before_scan_stack.len() {
                assert(self.stack@[i] == before_scan_stack[i]);
                assert(before_scan_heap.ready_from(old(self).scanned as nat, position as nat, before_scan_stack[i]));
                let (p, entry) = choose|p: int, entry: int| old(self).scanned <= p < position
                    && before_scan_heap.pair(p, entry)
                    && before_scan_heap.payloads.internals.marked_at(before_scan_heap.gc.weak@[p])
                    && before_scan_heap.map_slots(p)[entry + 1] == before_scan_stack[i]
                    && before_scan_heap.live(before_scan_heap.map_slots(p)[entry]);
                assert(heap.pair(p, entry));
                assert(heap.map_slots(p)[entry + 1] == self.stack@[i]);
                assert(heap.live(heap.map_slots(p)[entry]));
            } else {
                assert(heap.ready_from(old(self).scanned as nat, (position + 1) as nat, self.stack@[i]));
            }
        }
        assert forall|k: u64, v: Value| #[trigger] heap.waiting(k, v) implies
            heap.waiting_from((position + 1) as nat, k, v) by {
            if before_scan_heap.waiting(k, v) {
                assert(before_scan_heap.waiting_from(position as nat, k, v));
                let (p, entry) = choose|p: int, entry: int| 0 <= p < position
                    && before_scan_heap.pair(p, entry)
                    && before_scan_heap.payloads.internals.marked_at(before_scan_heap.gc.weak@[p])
                    && before_scan_heap.map_slots(p)[entry].0 == k
                    && before_scan_heap.map_slots(p)[entry + 1] == v;
                assert(heap.pair(p, entry));
                assert(heap.map_slots(p)[entry].0 == k && heap.map_slots(p)[entry + 1] == v);
            } else {
                assert(heap.waiting_from((position + 1) as nat, k, v));
            }
        }
    """, "scan_entries(&internal.slots, &marks, self.stack, &mut heap.gc.ephemerons);": """
        assert forall|i: int| heap.pair(position as int, i) implies
            heap.covered(position as int, i, self.stack@) by {}
        assert forall|i: int| before_scan_stack.len() <= i < self.stack@.len() implies
            heap.ready_from(old(self).scanned as nat, (position + 1) as nat, self.stack@[i]) by {
            assert(ready_origin(internal.slots@, &marks, internal.slots@.len(), self.stack@[i]));
            let entry = choose|entry: int| 0 <= entry && entry % 2 == 0 && entry + 2 <= internal.slots@.len()
                && internal.slots@[entry].0 != Value::EMPTY.0 && internal.slots@[entry + 1] == self.stack@[i]
                && marks.live(internal.slots@[entry]);
            assert(heap.pair(position as int, entry));
        }
        assert forall|k: u64, v: Value| #[trigger] heap.waiting(k, v)
            && !before_scan_heap.waiting(k, v) implies
            heap.waiting_from((position + 1) as nat, k, v) by {
            assert(deferred(internal.slots@, &marks, internal.slots@.len(), k).contains(v));
            deferred_has_origin(internal.slots@, &marks, internal.slots@.len(), k, v);
            let entry = choose|entry: int| 0 <= entry && entry % 2 == 0 && entry + 2 <= internal.slots@.len()
                && internal.slots@[entry].0 == k && k != Value::EMPTY.0 && internal.slots@[entry + 1] == v
                && !marks.live(internal.slots@[entry]) && !marks.live(v);
            assert(heap.pair(position as int, entry));
        }
    """})
    clear = method(weak_scope, "clear", """
        ensures final(self).heap.same_except_waiters(old(self).heap),
            final(self).heap.gc.ephemerons.by_key@.is_empty(),
            final(self).stack@ == old(self).stack@,
            final(self).scanned == old(self).scanned,
    """)
    declarations.append("impl<" + parameters + "> WeakPass<'_, " + parameters + "> {\n"
                        + scan + "\n" + clear + "\n}")
    marker = "pub fn main() {}"
    if scan_code.count(marker) != 1:
        raise ValueError("scan proof no longer has a unique composition point")
    return scan_code.replace(marker, "\n".join(declarations) + "\n" + marker)
