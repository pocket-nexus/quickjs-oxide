# Canonical shape append: preserve existing slot owners

## Goal and evidence

Goal: reduce fixed-workload full-process execution time for V8 v7 Splay and allocation-heavy object construction, while preserving JS property semantics, generational shape/object identities, atom ownership, and failure cleanup. No changes to GC scheduling policy, interfaces exposed to JS, or benchmark workload.

Baseline source is `7723e825`; fresh parent profiling evidence is `/tmp/oxide-six-evidence/splay.report.txt` and `/tmp/oxide-six-baseline-profile/raw/splay.cost.jsonl`. Samples are self cycles, not additive projected speedup: retain_edges_transactionally 2.36%, replace_layout_with_owned_shape 1.92%, object_layout_edges 1.65%, remove_shape_cache 1.63%. Splay records 635,306 reused cold frames versus 37 allocated, 610,842 ordinary auth cache hits versus 20 authentications; another frame pooling project is poorly supported. Earley-Boyer likewise has 779,436 reused versus 194 allocated cold frames, 508,232 ordinary auth cache hits.

Actual path: RuntimeState::store_selected_property_slot, missing property and non-dictionary canonical successor branch. It cloned every existing Slots payload, appended one slot, retained all old and new symbol atoms, and called replace_object_layout. That path scanned/retained the full replacement graph, then released the whole previous graph. The semantic operation changes only the shape and appends one new slot.

## Change and alternatives

The direct local implementation uses `append_slot_with_owned_shape` plus `Heap::append_object_slot_with_shape`. Existing logical slot owners are preserved. The backing Slots storage can still be rebuilt by its reservation helper; this qualification is detailed below. Preparation validates the complete generational object/shape identities, non-dictionary layouts, exact old entries prefix including keys/flags, unchanged prototype, one extra entry, and the appended slot's storage kind. Capacity is reserved before retaining/publishing.

Only the successor shape and new slot edges are retained; only the previous shape edge is released. Existing property value owners and atom owners never move through a retain/release round trip. New symbol value atoms are retained by RuntimeState and rolled back only before publication. Symbol property keys remain owned by shapes. Existing shape invalidation is still called. Post-publication release errors carry `published: true` so new slot atoms are not erroneously rolled back.

This also avoids some HashMap aggregation in retain_edges_transactionally: old full layouts with more than two edges use a HashMap, whereas a new shape plus ordinary reference slot has at most two edges. Accessor get+set plus shape can still require the existing larger-list path; no GC threshold change was introduced.

No new dispatcher, metadata per object, or general ownership abstraction was needed. Keeping the full-layout path unchanged was the no-change alternative; optimizing its generic graph diff would have more scope and still require scanning old/new values. Unique-shape append, dictionary operations, reconfiguration, and full-layout replacement retain their existing paths.

Limits: prefix validation remains O(n), so this is not an O(1) append claim. `src/engine/heap/object_records.rs:713` implements `Slots::try_reserve` by allocating a new Vec and cloning existing PropertySlot payloads whenever needed length exceeds two, including already-Spilled storage. The new append calls this helper, so larger layouts still copy payloads/allocate. What is removed is the existing slots' heap/atom retain-release traffic and full-layout edge processing; universal removal of old-slot copying or preservation of the backing Vec is not claimed. Improving already-Spilled reservation is a separate next-step candidate, excluded from this round. Reservation can change internal capacity before a preparation failure; visible layout, contents, and owners remain unchanged. Performance remains to be accepted from the parent's isolated full benchmark run. No performance claim follows solely from self samples or passing tests.

## Mechanism and correctness acceptance

Profiling-only event: `shape_append_existing_owners_preserved`, emitted after slot+shape publication. It counts canonical append commits, not number of preserved edges or allocations. Tests prove a saturated existing object edge can survive a successful append without a retain, which the old full-layout retain could not do.

Rust 1.88 command:

```
cargo +1.88.0 test -p quickjs-oxide --lib canonical_append --jobs 2 --target-dir /home/eric/.cache/oxide-six-test-shape
```

Final result: 5 passed, 0 failed, 0.02 seconds execution. Coverage:

- Existing saturated edge is not retained; shape references and destruction are balanced.
- Changed prefix flags, stale appended object, and stale successor shape fail before publication and leave old contents/counts intact.
- Self-reference and duplicate getter/setter object edges retain exactly their multiplicity and release correctly.
- Runtime Symbol owner is rolled back on a rejected successor, with old symbol and shape counts unchanged.
- JS Symbol values and keys, shared accessor get/set, self references, deletion/reinsertion, and prototype/property-read cache invalidation retain behavior.

Two initial fixture errors were corrected before final pass: accessor fields require AccessorRef rather than Option<ObjectId>; atom count inspection requires branding the stored AtomIdx. The errors were confined to tests, not production code. Initial two heap tests passed before these additional fixtures were added. Full integrated test/conformance acceptance is owned by the parent.

## Delivery and independent build

Production commit: `775a4f9e` on original worker branch; standalone cherry-pick is `2d9c5e85`.
Fixture fixes: `3ed6968f`, `c1917034`.
Independent clean source tree: `/tmp/oxide-v8-shape-append`, branch `perf/v8-canonical-shape-append`, final HEAD `c1917034`.
Formal build:

```
RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/benchmark/build.py --repo /tmp/oxide-v8-shape-append --plain-only --plain-target /home/eric/.cache/oxide-six-build-shape --jobs 2
```

Binary `/home/eric/.cache/oxide-six-build-shape/release/qjs`; standard receipt beside it as `qjs.build.json`.

## Directions not included

Empty closure owner elision is isolated in `86df7888` on `/tmp/oxide-v8-call-frame`. Current ordinary/general bytecode call classification constructs a shared ClosureSlots and clones its function owner even for zero captured cells. Default ClosureSlots already represents empty/no owner, so a local guard can avoid one callee retain/release. This was deprioritized as smaller and unmeasured; it is absent from the independent shape tree and the proposed integrated source. Its newly added unit test was not part of this direction's acceptance.

Further read-only finding: append_transition calls record_transition on a live cache hit; record_transition overwrites the forward map entry but unconditionally pushes the same (parent, entry) into the target's reverse Vec. Repeated hit count can therefore grow reverse adjacency without increasing distinct live edges. This is a separate memory/time mechanism with different invalidation obligations, delegated by the parent to the property agent. No deduplication changes are included here.
