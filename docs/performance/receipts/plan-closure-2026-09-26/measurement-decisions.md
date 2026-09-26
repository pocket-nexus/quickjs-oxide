> Chronological experiment log, not the current implementation specification. Later entries supersede earlier candidate choices. The delivered source is `b280ec8b`; the main receipt records its completed fixed-cost gates and the unresolved interfered timing.

# Final short verification protocol (declared before final candidate measurements)

The candidate removes ordinary Number write specialization after two fixes failed to recover Object cycles, while keeping receiver transfer and all unrelated accepted changes. Before integration, repeat object/local/argument/owner-fallback probes against Parent with 8 samples per side in ABBA-BAAB order. Then run the 20-case fixed matrix against Parent and the previous c7 integration, with a separate same-binary A/A and 4 samples per alias/engine.

For final bounded V8 comparisons, use all eight isolated suites plus combined, the same pinned source, fixed body run counts, zero fixed warmup, and 2 samples per engine per phase in one complete ABBA block. Run Parent first with target 0.1 seconds, freeze the counts and generated hashes, then replay that exact plan for c7, R0 and B37. Each invocation retains the 600-second total deadline and 90-second process limit. This reduces repeated historical baseline work after the prior complete 4-per-side matrix; it does not strengthen timing confidence. Report whole-process instructions/cycles/wall and A/A spans, with interference noted. Never call these results original V8 Scores or stable small percentage gains. Any clear instruction regression outside same-binary variation or reproducible cycles regression triggers a targeted follow-up, not an optional whole-suite rerun.

No local builds or tests overlap our measurements. Other same-host work remains allowed by the user and is recorded as interference.

## Revision after Number-withdrawal gate, before entry-withdrawal candidate

The Number withdrawal did not recover Object cycles (+6.48% versus Parent), so it is rejected. A serial six-version stage screen showed the existing single-choice entry withdrawal near Parent (+0.89% cycles, +0.81% instructions), while retaining the Number fast path and its scalar gains. The final candidate will therefore withdraw only LocalFusionChoice from 5e3dedc9, keeping static dense facts, array recovery, Number writes and receiver transfer. This is rollback of the earlier optimization that failed the cumulative cycle gate, so any lost earlier instruction savings are reported explicitly; it is not presented as a new optimization passing every c7-relative instruction gate. All other predeclared cases, sample counts, limits and final Parent/R0/B37 comparisons remain unchanged. Names prefixed accepted in earlier files refer to the failed Number-withdrawal trial, not acceptance.

## Receiver composition gate

The 2d0eca7f entry-withdrawal combination still used 6.65% more Object cycles than Parent. A direct eight-per-side comparison against ddf12f8f confirmed +5.92% cycles with only +0.03% instructions. The receiver candidate therefore remains unaccepted despite its positive method probes. One final structural variant will return only the moved JsValue from slot preparation and construct CallInput in install, to reduce the observed large install temporary frame. If it still fails the cumulative gate, withdraw receiver transfer and retain only the independent failure-cleanup repair. Do not call these binary-dependent cycle shifts a proven cache or branch predictor cause.

## Receiver rejection and ordinary-read candidate

Both receiver-transfer variants failed the cumulative Object cycle gate. The compact RAII variant also increased the installation stack from 0x320 to 0x330. Receiver transfer is withdrawn. Candidate 5688c3e1 retains only the independent named-local failure cleanup and immediate RAII ownership of the copied receiver; it still measured Object cycles +7.30% versus Parent. Machine-code analysis did not establish a branch/cache cause. The next narrow candidate 708afeb6 adds inline(always) to the existing checked local/parameter read wrappers only. All prior fixed and bounded V8 matrix gates remain. Original adaptive V8 Score is not a delivery gate in this iteration: the user requested approximately ten-minute full coverage batches. Fixed-work all-eight plus combined comparisons replace it, explicitly without claiming Score equivalence.

## Final read-boundary candidate

708afeb6 failed codegen: the 21 calls merely changed their target names to the still-outlined SlotStore helpers, with identical normalized instructions. It is rejected. The second and last annotation candidate 8d3875dc also makes those two underlying checked helpers inline(always), without changing any check or error. If actual calls are not removed or cumulative gates still fail, stop this annotation route.

## Entry composition reopened after the read fix

8d3875dc deletes 21 actual machine calls. Its 20-case direct read-only slice (5688 -> 8d) has no >2% instruction regression, and cumulative Parent Object cycles no longer reproduce the earlier ~7% loss. However, entry withdrawal loses c7 improvements in empty_loop (+6.43% instructions), array_read (+2.72%) and prop_read (+2.61%). Those losses are not erased by local-read gains. One composition trial will restore the previous entry choice on top of the new read boundary, preserving receiver rollback and ownership repairs. Reopening is justified by the new machine-code boundary, not by changing thresholds. The source will be built and tested only after the current serial measurement batch drains.

## Ordinary write boundary after entry/read composition

be855322 restores the prior instruction gains, but the Object-only 8+8 follow-up repeats +3.128% cycles versus Parent (instructions -4.119%; same-binary A/A median contrast -0.424%). It is not accepted and full V8 is not started. One separate write-boundary candidate will inline only the existing checked RunSlots replace_local/replace_parameter wrappers and corresponding SlotStore current helpers. It does not change the Number classifier or any ownership/error code. The intended mechanism is eliminating remaining ordinary owner writeback call/Result transfers; actual deletion and all cumulative gates remain required.
