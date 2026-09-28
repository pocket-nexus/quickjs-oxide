# C2: decoded fallthrough for same-frame property reads

C2 is stacked on the C1 numeric continuation change. `execute_frame` attaches
the decoded fallthrough to static-key and computed-key property read actions,
including cache and dense-array guard misses. The ready driver passes it to
`read_progress`; synchronous linked-own and prepared reads carry it through
result publication without calling `Frame::next_pc()` to rediscover the
instruction boundary.

The operation still owns its existing completion protocol. A known fallthrough
does not advance `resume_pc`; `publish_read_result` retains the property path's
ordering, including advancement before the final result push. Tests cover
ordinary and retained receiver/key outputs, a constrained-output failure,
zero recovery-helper calls during covered completion, fault attribution, and
the one-time effects of a throwing getter. A warmed inline field-cache hit is
a negative control: it creates no property action.

Object-key conversion, getters, proxies, super-property reads and other
pending replies retain their existing operation and frame identities. Their
remaining recovery calls are explicit in `read_pending` and
`complete_read_recovering`; the `property_legacy_fallthrough_recovery_decode`
diagnostic counts those sites, not ordinary instruction decoding. Writes,
calls, suspension and interpreter-entry validation remain outside C2.

## Reproduction

`manifest.json` fixes six workload sources, expected outputs and byte hashes.
The missing static field and computed object/frame cases exercise same-frame
completion. String length and the warmed own field stay inline; the getter
case checks deferred work. Profile counts, rather than JavaScript source
spelling, determine which path each workload actually reaches. Release
comparisons use plain and profiling builds of C1 and C2 with Rust 1.88.0,
the same repository release profile and distinct target directories.

## Evidence

[Build provenance](data/build-provenance.json) records hashes and compiler
flags. The C1 binary was built at `5b0ded27`, before two receipt-only C1
commits; `git diff --quiet 5b0ded27 ade7f11e -- src/engine apps/cli/src`
confirmed the production Rust code is identical to C2's immediate parent.
The C2 binary was built at `da0c872d`. Both are clean-source release builds.

[Profile counts](data/profile-counts.json) from the C2 profiling binary
establish coverage:

| Case | Read actions | Carried same-frame completions | Legacy property recovery decodes |
| --- | ---: | ---: | ---: |
| Missing static field | 1,000,000 | 1,000,000 | 0 |
| String length | 0 | 0 | 0 |
| Computed object, call loop | 1,000,000 | 1,000,000 | 0 |
| Computed object, one frame | 1,000,000 | 1,000,000 | 0 |
| Warm own field | 0 | 0 | 0 |
| Getter | 10,000 | 0 | 10,000 |

String length was initially considered as a candidate static-action probe,
but profiling showed it completed inline. The missing-field probe supplies
the covered static-key case. Both remain in the fixed matrix so a future
change to admission is visible. The getter's remaining recovery is the
documented deferred boundary, not a C2 miss.

[Generated-code accounting](data/codegen-summary.json) and
[selected ARM64 excerpts](data/codegen-excerpts.md) show the recovered
position is no longer requested in the covered helpers. Static
`Frame::next_pc` calls fall from one to zero in both `read_progress` and
`complete_read`. `ready::run` grows from 10,652 to 10,660 bytes by the
symbol-to-next-symbol measure; `read_progress` grows from 4,564 to 4,920
bytes, and `complete_read` shrinks from 1,652 to 1,612 bytes. The full
`__text` section grows from 5,336,520 to 5,337,012 bytes. These are
compiler-output observations, not isolated execution costs. In the ready
driver excerpt the new fallthrough is passed in a register to
`read_progress`; the displayed call site does not show an additional spill.
`VmAction`, `Result<VmAction, Error>` and `Frame` remain 16, 16 and 56 bytes
on this 64-bit target.

## Validation and timing

Rust 1.88 workspace/all-targets tests, profiling Clippy with `-D warnings`,
formatting, source layout, and focused property tests with profiling passed.
Focused Test262 matched 6,844/6,844 eligible variants. The pinned QuickJS
fixture differential matched 13/13 cases, and the nine C oracle fixtures
passed. On the recorded C2 source before #53's later V8 extension, full
Test262 matched the frozen vector: **80,010/80,060** runnable passes among
102,037 variants. Its report SHA-256 is
`dc2c103c7f952854e4f17fa5b1b7d112995c32a0c5219598a3a931bcf573e2bd`;
the body hash is the frozen
`971cc666767b3c4eb9b519340b5d7a77a80ff8405f6a8d800aada822d3230b19`.
The runner used engine semantics fingerprint
`854b46baf6576717cb60e9009c4f319248f80d6ce756adb979f0b9786c52edfa`.
These results do not validate a later rebase onto new #53 code.

The [same-binary A/A control](data/fixed-aa.json) and [C1/C2 paired run](data/fixed-ab.json)
used two ABBA-ordered samples per binary and case, output checks, and macOS
`/usr/bin/time -l` retired-instruction counters. They include process startup
and compilation. Counts for covered reads are lower in C2, while inline
controls are close to parity:

| Case | C2 / C1 retired instructions | C1 / C2 whole-process median ms |
| --- | ---: | ---: |
| Missing static field | 0.9961 | 5730 / 5507 |
| String length, inline | 1.0009 | 3723 / 2793 |
| Computed object, call loop | 0.9953 | 4247 / 4192 |
| Computed object, one frame | 0.9933 | 2460 / 2692 |
| Warm own field, inline | 1.0015 | 2626 / 2407 |
| Getter, deferred | 0.9984 | 58 / 58 |

The A/A retired-instruction ratios were 0.9999 for the missing static field,
1.0005 for the computed one-frame read, and 0.9998 for the warmed field.
Their corresponding wall-time ratios ranged from 0.86 to 1.21 despite
comparing the same binary. The covered instruction reductions support less
machine work, but generated-code changes elsewhere and the large host timing
noise prevent an isolated cost or runtime speedup claim.

The [compile-once A/A control](data/execute-aa.json) and
[C1/C2 paired run](data/execute-ab.json) use a byte-identical
`apps/cli/examples/execute_probe.rs` in both source trees (SHA-256
`3effd109dc4983d3d69cc9c91d4ae744ffca26a932e78f97b08f0f1d2d8db6d7`).
Each case ran two ABBA-ordered process pairs, with three warmups and nine
timed `Context::execute` calls per process. These samples exclude CLI startup
and compilation.

| Case | C1 / C2 execute-only median ms | C2 / C1 |
| --- | ---: | ---: |
| Missing static field | 2456 / 2071 | 0.843 |
| String length, inline | 915 / 1157 | 1.264 |
| Computed object, call loop | 2282 / 2352 | 1.030 |
| Computed object, one frame | 787 / 946 | 1.202 |
| Warm own field, inline | 996 / 1064 | 1.068 |
| Getter, deferred | 19.7 / 15.8 | 0.803 |

Same-binary A/A execute-only ratios were 0.864 for missing static field,
0.836 for computed one-frame read, and 1.061 for warm own field. The
direction and magnitude of the A/B wall differences are therefore not
reliable estimates of C2's runtime effect. The host was contended by other
builds and two full conformance runs; a quiet-host repeat remains useful.
The instruction counts, path diagnostics and semantic tests establish the
bounded mechanism independently of that timing question.
