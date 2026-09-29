# Final combination: lightweight static interaction review

Reviewed exact HEAD `ee999771e2943ec31c303209f53c1107cf829eda` against base `7723e825`. This review read the diff and relevant callers/ownership/cleanup helpers. No builds, tests, benchmarks, or profiling were run for this review. The earlier shape-only tests are separate evidence and do not dynamically prove the final combination.

## Conclusion

No new correctness blocker was identified in the requested interaction paths by this static pass. One material mechanism-description correction was found: canonical append still copies slot payloads when Slots::try_reserve handles a layout exceeding its two-slot inline capacity. The ownership-traffic removal is supported by the code; universal removal of buffer copying/allocation is not.

## Scalar PutField and append interaction

- The scalar path requires an immediate new value, an Object receiver, a linked atom from a same-runtime published executable, and receiver release readiness. It then accepts only `(ObjectKind::Ordinary, ObjectPayload::Ordinary)`, an existing own slot, writable flags, and an immediate old Data value. Arrays, proxies, inherited/missing properties, accessors, AutoInit/VarRef storage, non-writable values, and reference-bearing old/new values decline to the existing driver.
- Readiness validates the receiver's full identity and requires no deferred work, an empty heap zero queue, and a receiver count greater than one. The following scalar-to-scalar replacement changes no owner counts and returns empty cleanup. There is no callback or intervening release between this check and consuming the frame receiver, so the no-drain premise remains valid. The executable keeps the linked key atom owned throughout.
- The exclusive frame transaction preserves both operands on decline. Success commits the store before popping the immediate scalar and receiver. The two preceding peeks establish operand availability; no slot mutation is interposed. `property_generation` overflow falls back before the store and successful local completion increments it once.
- Existing-slot scalar replacement does not change shape/layout. Missing properties fall back and can reach canonical append; therefore the two optimizations do not independently mutate the same selection. Canonical append validates old prefix keys+flags, prototype, exact count, and new slot storage under one exclusive RuntimeState borrow. Shape identity remains generational. Layout epoch invalidation is retained for objects used as prototypes. Property read caches still resolve the current value rather than relying on a cached old scalar.

## Append ownership and transition deduplication

- Canonical append preflights the new shape and new slot edges before publishing slot+shape. Existing slot heap and Symbol owners are not retained/released; only a new slot's Symbol atoms are retained by RuntimeState. Shape entries own property key atoms, independently of value atoms. `SlotReplacementError.published` prevents rollback of new value atoms after publication.
- The caller's owned successor reference keeps that shape alive through old-shape release and cleanup. On successful publication the object acquires its own successor edge before the temporary successor reference is released. Old-shape finalization invalidates its weak transition/cache entries; those are independent of ownership of the new shape.
- `record_transition` still inserts the forward edge. If the exact full target ID is unchanged it omits the redundant reverse entry. On rebinding, it removes the `(parent, entry)` reverse pair from the previous full target ID before recording the new pair. Delayed cleanup of a previous generation therefore cannot discover the stale reverse pair and erase the new forward edge. Existing mutation/collection unlink paths remain in place.
- The deduplication changes weak bookkeeping only: it creates or removes no strong reference, slot owner, or Symbol atom. Shape append may finalize/unlink the old shape, but can do so only after publication; it does not carry a reverse-vector element or mutable map borrow across that cleanup.
- As with existing runtime cleanup paths, invariant failures during release/apply_cleanup are not an atomic rollback facility for a committed mutation. This review found no new legitimate-input route to those invariant failures; it is not an exhaustive fault-injection proof.

## Operand push cold helpers

The new helpers only relocate creation of the same two Error values. The order remains:

1. Check depth against verified operand capacity.
2. Compute operand index and inspect whether it contains a live value.
3. Return that index for installation only if both checks succeeded.

Capacity errors still precede indexing, live-slot errors still precede writes/depth increments, and error text is unchanged. No identity check, bound, owner transfer, or cleanup call was removed. The cold/inline-never annotations affect code layout; static source review cannot establish their actual speed benefit.

## Mechanism qualification requiring report correction

`src/engine/heap/object_records.rs:713` currently implements `Slots::try_reserve` by constructing a new Vec, reserving total needed size, cloning every existing slot payload into it, and replacing Slots, whenever needed length exceeds INLINE_CAPACITY. It does this even when Slots is already Spilled with sufficient capacity.

The new append path calls this helper before publication. Consequently:

- It definitely removes retain/release of existing slot edges, old/new full-layout atom handling, and full object_layout_edges traversal.
- It can avoid HashMap edge aggregation when the new shape plus new ordinary slot has at most two edges.
- Inline appends that fit within two slots avoid copying/allocation.
- Larger layouts still copy existing PropertySlot payloads and allocate through try_reserve. They may avoid the old clone-then-push extra growth pattern, but claiming all copying/allocation was removed is unsupported.
- Existing owner counts remain valid: PropertySlot cloning duplicates raw payload representations, not their tracked heap owners, and replacing the old representation transfers the logical ownership. This is a mechanism/performance reporting issue, not a discovered ownership bug.

The report should describe preservation of existing logical slot owners and removal of graph/atom ownership traffic. A separate change to use `Vec::try_reserve(additional)` for already-Spilled slots could address the remaining copying, but it is outside this read-only review and needs its own validation/measurement.
