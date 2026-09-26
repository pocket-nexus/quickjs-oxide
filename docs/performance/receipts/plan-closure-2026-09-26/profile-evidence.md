# Fixed profile diagnostic summary

Results SHA-256: `9774c31d294979913a09cc23bd4b4c5b794dc4ec8625d9b96409038940d51834`.

All event counts are logical frequencies. Phase nanoseconds come from the profiling build's sampled entries only; they include diagnostic overhead. They are not divided by plain-build wall time or used as V8 Score estimates. Parent inclusive phases overlap child phases. Median is the stored-sample median; p90 uses nearest rank. When omitted > 0, stored-sample quantiles are only for the first retained samples.

## crypto

Engine commit: `5e3dedc9e3bfd717fe50f7187d08db1247c224d0`; raw SHA-256: `1e046a30bebf419e62f6b074ab2f96e3cd68dcc97b476278a8b551e69ba94137`.

Materialized misses: **711,446**; refined: 711,446; legacy unrefined: 0. Fusion omissions: `{"callsite_events": 0, "dispatch_events": 0, "outcome_events": 0, "static_functions": 0}`.

| Materialized label | Misses | Share of materialized |
| --- | ---: | ---: |
| `array_materialized.own_default_number` | 710,004 | 99.80% |
| `array_materialized.missing_own_index` | 1,208 | 0.17% |
| `array_materialized.own_non_number` | 234 | 0.03% |

Largest PC for each label (all PC/label counts in summary.json):

| Label | Function | PC | Misses | Share of label |
| --- | --- | ---: | ---: | ---: |
| `array_materialized.own_default_number` | am3 (crypto.js:393) | 51 | 355,669 | 50.09% |
| `array_materialized.missing_own_index` | bnpMultiplyTo (crypto.js:700) | 34 | 575 | 47.60% |
| `array_materialized.own_non_number` | bnModPow (crypto.js:1383) | 319 | 170 | 72.65% |

Top functions (full distribution in summary.json):

| Function | Misses | Share |
| --- | ---: | ---: |
| am3 (crypto.js:393) | 622,849 | 87.55% |
| bnpSquareTo (crypto.js:716) | 39,232 | 5.51% |
| montReduce (crypto.js:868) | 34,518 | 4.85% |
| bnpDRShiftTo (crypto.js:615) | 11,504 | 1.62% |
| bnpMultiplyTo (crypto.js:700) | 2,133 | 0.30% |
| bnCompareTo (crypto.js:572) | 596 | 0.08% |
| bnModPow (crypto.js:1383) | 234 | 0.03% |
| bnpDLShiftTo (crypto.js:604) | 207 | 0.03% |
| bnpCopyTo (crypto.js:465) | 164 | 0.02% |
| bnpLShiftTo (crypto.js:624) | 9 | 0.00% |

Top PCs (full distribution and labels in summary.json):

| Function | PC | Fusion kind | Misses | Share |
| --- | ---: | --- | ---: | ---: |
| am3 (crypto.js:393) | 51 | dense_read | 355,669 | 49.99% |
| am3 (crypto.js:393) | 20 | dense_read_binary | 133,590 | 18.78% |
| am3 (crypto.js:393) | 26 | dense_read_post_update | 133,590 | 18.78% |
| bnpSquareTo (crypto.js:716) | 24 | dense_store | 20,114 | 2.83% |
| montReduce (crypto.js:868) | 34 | dense_read_binary | 11,506 | 1.62% |
| montReduce (crypto.js:868) | 48 | dense_read_binary | 11,506 | 1.62% |
| montReduce (crypto.js:868) | 92 | dense_read | 11,506 | 1.62% |
| bnpDRShiftTo (crypto.js:615) | 23 | dense_read | 11,504 | 1.62% |
| bnpSquareTo (crypto.js:716) | 50 | dense_read | 9,304 | 1.31% |
| bnpSquareTo (crypto.js:716) | 73 | dense_read | 9,304 | 1.31% |
| bnpMultiplyTo (crypto.js:700) | 34 | dense_store | 2,133 | 0.30% |
| bnCompareTo (crypto.js:572) | 39 | dense_read | 596 | 0.08% |
| bnpSquareTo (crypto.js:716) | 138 | dense_read | 510 | 0.07% |
| bnpDLShiftTo (crypto.js:604) | 52 | dense_store | 207 | 0.03% |
| bnModPow (crypto.js:1383) | 319 | dense_read | 170 | 0.02% |
| bnpCopyTo (crypto.js:465) | 21 | dense_copy | 145 | 0.02% |
| bnModPow (crypto.js:1383) | 135 | dense_read_index_binary | 30 | 0.00% |
| bnModPow (crypto.js:1383) | 140 | dense_read | 30 | 0.00% |
| bnpCopyTo (crypto.js:465) | 23 | dense_read | 19 | 0.00% |
| bnpLShiftTo (crypto.js:624) | 106 | dense_store | 9 | 0.00% |

Ordinary call counters (full event counts; authentication, observed callsites, and install attempts have different boundaries):

- Install entry attempts: 80,262; by form: `{"function": 3450, "method": 76812}`; by argument count: `{"args0": 4211, "args1": 3819, "args2": 5622, "args3": 207, "args4plus": 66403}`.
- Authentication cache hits: 80,198; fresh authentications: 65; direct returns: 80,262.
- Direct entries: 85,340; sampled entries: 1,373; observed callsite calls: 85,340 (ordinary-driver-enter-selected-only; other call and construct paths are excluded).

Sampled VM phases (nanoseconds; Σ includes only sampled profiling entries):

| Phase | Attempts | Stored | Omitted | Incl median | Incl p90 | Excl median | Excl p90 | Σ incl | Σ excl |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `direct.prepare.sampled` | 1,373 | 1,373 | 0 | 917 | 1,625 | 708 | 1,333 | 1,707,696 | 1,212,108 |
| `direct.select.sampled` | 1,373 | 1,373 | 0 | 42 | 83 | 42 | 83 | 75,738 | 75,738 |
| `ordinary.authenticate.sampled` | 1,294 | 1,294 | 0 | 83.0 | 125 | 83.0 | 125 | 250,158 | 250,158 |
| `ordinary.install.sampled` | 1,294 | 1,294 | 0 | 1,000.0 | 1,291 | 417.0 | 625 | 1,512,313 | 708,359 |
| `ordinary.install.slots.sampled` | 1,294 | 1,294 | 0 | 583.0 | 708 | 583.0 | 708 | 803,954 | 803,954 |
| `ordinary.validate.sampled` | 1,294 | 1,294 | 0 | 83.0 | 166 | 83.0 | 166 | 169,692 | 169,692 |


## deltablue

Engine commit: `5e3dedc9e3bfd717fe50f7187d08db1247c224d0`; raw SHA-256: `95f15e5fa36a9179d8c7f5436fa51d495486618f0013da2cf523d59e1d6374ab`.

Materialized misses: **0**; refined: 0; legacy unrefined: 0. Fusion omissions: `{"callsite_events": 0, "dispatch_events": 0, "outcome_events": 0, "static_functions": 0}`.

| Materialized label | Misses | Share of materialized |
| --- | ---: | ---: |
| — | 0 | — |

Largest PC for each label (all PC/label counts in summary.json):

| Label | Function | PC | Misses | Share of label |
| --- | --- | ---: | ---: | ---: |
| — | — | — | 0 | — |

Top functions (full distribution in summary.json):

| Function | Misses | Share |
| --- | ---: | ---: |
| — | 0 | — |

Top PCs (full distribution and labels in summary.json):

| Function | PC | Fusion kind | Misses | Share |
| --- | ---: | --- | ---: | ---: |
| — | — | — | 0 | — |

Ordinary call counters (full event counts; authentication, observed callsites, and install attempts have different boundaries):

- Install entry attempts: 111,069; by form: `{"function": 8, "method": 111061}`; by argument count: `{"args0": 74226, "args1": 32600, "args2": 4243, "args3": 0, "args4plus": 0}`.
- Authentication cache hits: 111,817; fresh authentications: 66; direct returns: 111,069.
- Direct entries: 115,452; sampled entries: 1,836; observed callsite calls: 115,452 (ordinary-driver-enter-selected-only; other call and construct paths are excluded).

Sampled VM phases (nanoseconds; Σ includes only sampled profiling entries):

| Phase | Attempts | Stored | Omitted | Incl median | Incl p90 | Excl median | Excl p90 | Σ incl | Σ excl |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `direct.prepare.sampled` | 1,836 | 1,836 | 0 | 583.0 | 1,292 | 457.0 | 958 | 1,584,546 | 1,253,743 |
| `direct.select.sampled` | 1,786 | 1,786 | 0 | 42.0 | 83 | 42.0 | 83 | 76,348 | 76,348 |
| `ordinary.authenticate.sampled` | 1,767 | 1,767 | 0 | 42 | 166 | 42 | 166 | 143,906 | 143,906 |
| `ordinary.install.sampled` | 1,767 | 1,767 | 0 | 542 | 1,208 | 332 | 708 | 1,294,065 | 730,096 |
| `ordinary.install.slots.sampled` | 1,767 | 1,767 | 0 | 250 | 500 | 250 | 500 | 563,969 | 563,969 |
| `ordinary.validate.sampled` | 1,767 | 1,767 | 0 | 42 | 125 | 42 | 125 | 110,549 | 110,549 |


## richards

Engine commit: `5e3dedc9e3bfd717fe50f7187d08db1247c224d0`; raw SHA-256: `528230ea359da2efa1a324fc00d7c85773bd03fd9a04efd45b3128b9d7c04e81`.

Materialized misses: **0**; refined: 0; legacy unrefined: 0. Fusion omissions: `{"callsite_events": 0, "dispatch_events": 0, "outcome_events": 0, "static_functions": 0}`.

| Materialized label | Misses | Share of materialized |
| --- | ---: | ---: |
| — | 0 | — |

Largest PC for each label (all PC/label counts in summary.json):

| Label | Function | PC | Misses | Share of label |
| --- | --- | ---: | ---: | ---: |
| — | — | — | 0 | — |

Top functions (full distribution in summary.json):

| Function | Misses | Share |
| --- | ---: | ---: |
| — | 0 | — |

Top PCs (full distribution and labels in summary.json):

| Function | PC | Fusion kind | Misses | Share |
| --- | ---: | --- | ---: | ---: |
| — | — | — | 0 | — |

Ordinary call counters (full event counts; authentication, observed callsites, and install attempts have different boundaries):

- Install entry attempts: 40,470; by form: `{"function": 2, "method": 40468}`; by argument count: `{"args0": 26233, "args1": 11902, "args2": 2322, "args3": 5, "args4plus": 8}`.
- Authentication cache hits: 40,441; fresh authentications: 30; direct returns: 40,470.
- Direct entries: 40,472; sampled entries: 632; observed callsite calls: 40,472 (ordinary-driver-enter-selected-only; other call and construct paths are excluded).

Sampled VM phases (nanoseconds; Σ includes only sampled profiling entries):

| Phase | Attempts | Stored | Omitted | Incl median | Incl p90 | Excl median | Excl p90 | Σ incl | Σ excl |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `direct.prepare.sampled` | 632 | 632 | 0 | 584.0 | 792 | 417.0 | 585 | 528,754 | 331,484 |
| `direct.select.sampled` | 632 | 632 | 0 | 42.0 | 42 | 42.0 | 42 | 24,750 | 24,750 |
| `ordinary.authenticate.sampled` | 632 | 632 | 0 | 83.0 | 125 | 83.0 | 125 | 136,390 | 136,390 |
| `ordinary.install.sampled` | 632 | 632 | 0 | 583.0 | 750 | 333.0 | 375 | 437,080 | 249,914 |
| `ordinary.install.slots.sampled` | 632 | 632 | 0 | 250.0 | 375 | 250.0 | 375 | 187,166 | 187,166 |
| `ordinary.validate.sampled` | 632 | 632 | 0 | 42.0 | 84 | 42.0 | 84 | 36,130 | 36,130 |

## Crypto array transition causes

`profile-current` (`8525105d`) and `profile-final` (`5e3dedc9`) each contain one valid run per case with the same generated JS SHA-256 and one iteration per benchmark. Their profiling binaries differ, and the new event names did not exist in the older build. The full identity, raw hashes and counts are in `summary.json` under `array_transition_comparison`.

| Final Crypto event | Count | Meaning |
| --- | ---: | --- |
| `array_storage_dense_materialization` | 66 | Completed dense → ordinary layout transitions |
| `array_storage_dense_materialization_gap_write` | 66 | All 66 transitions entered through the gap-write call site |
| descriptor-path / interior-delete materialization | 0 | No completed transitions counted at those call sites |
| `array_storage_dense_recovery_enter` | 60 | Recovery function entry attempts after a successful new index-0 write |
| `array_storage_dense_recovery` | 60 | All 60 attempts completed a recovery |
| recovery reject / heap-declined categories | 0 | No such event was recorded |

These are event frequencies, not time costs or distinct array counts. A 60/66 ratio is **not** a per-array recovery success rate because the aggregate counters do not pair transitions by object. DeltaBlue and Richards have no corresponding events in this run.

Crypto still has 711,446 materialized dense-fusion misses, exactly the prior run's count: 710,004 (99.80%) are own default Number elements, 1,208 missing own indices and 234 own non-Number elements. The leading function is `am3` with 622,849 misses (87.55%); its PCs 51, 20 and 26 account for 622,849. Fusion omission counters are all zero. Thus ordinary array representation is repeatedly seen at profitable numeric sites, while the new counters identify gap writes as the observed conversion call site. The current schema has no array identity or transition PC, so it cannot prove which converted arrays feed `am3`, how long each stays ordinary, or whether the 60 recoveries are among those same arrays.

This supports a bounded next diagnostic only if the array candidate remains under consideration: attach a short profiling-only per-array transition/miss correlation (bounded identities, no owner retention) or isolate the gap-write path in a narrow A/B. Do not infer a speed gain from these logical counts alone.

## Plan 4 closure: why the recovery count does not settle the hot misses

The pinned upstream `crypto.js` is SHA-256 `5321d2ada9b61e01d5f281609513467cbaa7c0effb12c577ed08088d812453af`; the generated workload selects `setupEngine(am3, 28)` at generated line 1962. In `am3` (generated lines 393–405), the only three indexed reads are `this_array[i]`, `this_array[i++]`, and `w_array[j]`. The site kinds and code order map PC 20 (`dense_read_binary`, 133,590 materialized misses) to the first source-array read, PC 26 (`dense_read_post_update`, 133,590) to the second, and PC 51 (`dense_read`, 355,669) to the output-array read. All 622,849 misses at these sites report an own default numeric element in ordinary storage. This mapping identifies *where* hot reads fail the dense guard; it does not identify the array objects or the writes that changed their representation.

The workload supplies a plausible recovery opportunity: `bnpMultiplyTo` and `bnpSquareTo` initialize a result array from a high index down to index 0 (generated lines 708 and 722), and `bnpMultiplyLowerTo` does the same (line 1319). Their first high-index write on an empty dense array can enter the recorded gap-write materialization path in `properties.rs:1469–1489`; the final, newly defined index 0 can enter `try_recover_dense_array` at `properties.rs:1570–1574` (or `:1697–1705` for descriptor definition). This is a source-level candidate explanation for some of the 66 conversion and 60 recovery events, **not** an object-level pairing of those counters.

The trigger is deliberately narrow. `properties.rs:1572` requires `index == 0 && !existing`; the descriptor path similarly requires index 0 to be absent. Once an array is ordinary *and already owns index 0*, later index-0 updates do not attempt recovery. A higher-index write that materializes such an array can therefore leave it ordinary while `am3` repeatedly reads valid numeric elements from it. The source and aggregate profile make this persistence possible, but do not prove how often it occurs in Crypto. Likewise, six more conversion events than recovery entries do not prove that six objects stayed ordinary: neither counter is a distinct-object count, and their event streams have no shared identity.

**Stop decision for this round:** the 60 successful recovery attempts did not reduce the 711,446 observed materialized misses, and neither the old nor new profile links transition events to `am3` array identities. Further storage-policy changes or a speed claim would rely on an unproven causal story. If Plan 4 resumes, the one decisive diagnostic is a bounded profiling-only correlation keyed by non-owning array identity: record each gap conversion and recovery, then count later materialized misses for those same identities at `am3` PCs 20/26/51. This would show whether repeated misses belong to never-recovered arrays, arrays rematerialized after recovery, or unrelated arrays; it should be paired with a narrow plain-build A/B before keeping a recovery implementation for performance reasons.
