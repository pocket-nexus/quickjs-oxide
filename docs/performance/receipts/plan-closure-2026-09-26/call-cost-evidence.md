# Plan 4/5 diagnostic interpretation

Source: `profile-current/results.json` (SHA-256 `2e241f3220229f143abb30d4c5de311c34bfda068b758860f3ec211da21f0e9a`), cost record embedded commit `8525105dadad68f1e4113b8b42d7bbffa69a53b6`. All three fixed cases are valid; fusion omission counters are zero. The source and full per-PC/label distributions are in `summary.json`. Counts below are diagnostic event frequencies, not time.

## Crypto materialized Arrays

Of 711,446 remaining `array_materialized.*` dense misses, 710,004 (99.80%) observe an own default Number at the requested index, 1,208 (0.17%) lack that own index, and 234 (0.03%) observe an own non-Number. `am3` contributes 622,849 (87.55%) materialized misses: PC 51 has 355,669, PC 20 has 133,590, and PC 26 has 133,590. This is a strong reason to inspect *why* those Arrays are still materialized. The probe reads only the requested own slot after a failed dense attempt; it does not prove the whole Array is contiguous, default-data, or eligible for recovery. New profiling-only conversion/recovery reason counters should decide that question before any storage expansion. The separate narrow no-recovery candidate measures the aggregate effect of the trigger and representation changes in a plain build.

## Ordinary calls: full entry counters

`ordinary_install.*` records entry **attempts**; a later step can fail. In these fixed loads its total equals `ordinary_return_direct`, but that equality must not be assumed generally. Authentication, entry, install attempts and observed callsites have distinct boundaries.

| Case | Direct entries | Ordinary install entry attempts | Method attempts | Args 0 | Args 1 | Args 2 | Auth cache hit / fresh |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| DeltaBlue | 115,452 | 111,069 | 111,061 (99.993%) | 74,226 (66.83%) | 32,600 (29.35%) | 4,243 (3.82%) | 111,817 / 66 |
| Richards | 40,472 | 40,470 | 40,468 (99.995%) | 26,233 (64.82%) | 11,902 (29.41%) | 2,322 (5.74%) | 40,441 / 30 |

The current profiling build samples roughly 1/64 direct entries; DeltaBlue has 1,836 sampled entries and Richards 632, with no phase-sample omissions. The phase medians are `ordinary.install.sampled` 583/542 ns, `ordinary.install.slots.sampled` 208/208 ns, and `ordinary.authenticate.sampled` 42/83 ns (DeltaBlue/Richards). These are *instrumented* durations, close to timer granularity for the small subphases. `direct.prepare.sampled` includes outcomes other than ordinary installation; phase attempts differ. Phase inclusive totals nest and cannot be summed or divided by plain-build time.

## Separate plain-release call stacks

The valid `/usr/bin/sample` receipts used plain binary SHA-256 `b241d0252cfe19d54b18843ae106f91c4cb66f2d7f462f120ce21e2d643f3573` from commit `c7fb5b69d88bb5e6170fab3b8af5a3062fd4cb72`. Each sample covered three seconds inside an approximately eight-second repeated fixed workload (DeltaBlue 80 runs, Richards 136). The call graph counts below sum disjoint occurrences of the exact VM symbols and their descendants. `OrdinaryCall::install` is within `ready::enter_call`, so their counts are **not additive**. The samples are one temporal slice, not a full-run mean or a savings bound.

| Case | Main-thread stack samples | `ready::enter_call` on stack | `OrdinaryCall::install` on stack | `OrdinarySelection::authenticate` on stack |
| --- | ---: | ---: | ---: | ---: |
| DeltaBlue | 2,446 | 483 (19.75%) | 262 (10.71%) | 33 (1.35%) |
| Richards | 2,478 | 241 (9.73%) | 176 (7.10%) | 19 (0.77%) |

The explicit install subtree contains `copy_reference` in 48 DeltaBlue and 28 Richards samples, and `release_frame_binding` in 55 and 38. The latter can cover callee as well as receiver release; none of these samples is entirely removable. The source copies a method receiver at `src/engine/vm/call/ordinary.rs:310-314`, while `src/engine/vm/stack/call.rs:103-109` later releases the caller's callee/receiver bindings. This makes **single-owner receiver transfer** a concrete narrow candidate to investigate, provided it preserves the original error priority, fallible release rollback, rooting, method `this`, and non-reentrant witness chain. It is not established as safe or fast yet. Cold-frame allocation is already rare in the fixed profiles (DeltaBlue 14, Richards 9), so allocating fewer cold frames is not supported as the next target. A callee callsite cache is likewise not supported by these timing or correctness facts.

`ordinary_storage` symbols in the stacks are object property operations, not ordinary function-call cost. The on-stack counts above use only the confirmed VM call entry/install/authentication symbols. They establish positive mechanism occupancy in a plain binary, while inlined work outside those symbols remains unassigned; they cannot serve as an upper bound on call-mechanism savings.

## Decision

Continue the bounded Crypto conversion/recovery reason diagnostic and the no-recovery plain A/B. For calls, focus only on the method receiver owner-transfer proof and a short correctness/rollback test plus plain A/B if proof succeeds. Stop if the receiver cannot be moved without changing failure and ownership semantics or if the plain A/B does not show a stable benefit. Do not expand to callsite caches or a general frame rewrite from the current evidence.

Plain stack files: DeltaBlue SHA-256 `2391f878f841c9b28940f077286e61a166fb94151e386ba0371a3c2d404258f8`; Richards SHA-256 `14670e25b3705d17d6dcae97c987403ebcb1fa8d19cf62ce745bf6e5d5501617`.
