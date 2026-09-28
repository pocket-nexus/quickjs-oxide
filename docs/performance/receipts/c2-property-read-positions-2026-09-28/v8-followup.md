# C2 follow-up on the #53 V8 extension

The immediate C1 parent was rebuilt on #53's VM/compiler extension at
`734bc1285610652baccfd9373033ff652d2fc1e0`; its measured source is
`9a9290dff8e2e7a18f3d2b54284ec518565a1a5d`. The measured C2 source is
`907b98c14b86a2e0755ef47192571097bf0c4d33`. The subsequent #53 change
`bbd8adaf` and the rebased C1/C2 heads only changed documentation: the
production Rust diff from each measured source to its PR head is empty.
[Build provenance](data/v8-builds.json) records the clean release builds,
compiler settings and binary hashes. This evidence is separate from the
original C2 receipt's pre-V8 comparison.

[Profiling counts](data/v8-profile-counts.json) reconfirm the covered paths:

| Case | Read actions | Carried completions | Legacy recovery decodes |
| --- | ---: | ---: | ---: |
| Missing static field | 1,000,000 | 1,000,000 | 0 |
| Computed object, call loop | 1,000,000 | 1,000,000 | 0 |
| Computed object, one frame | 1,000,000 | 1,000,000 | 0 |
| String length, inline | 0 | 0 | 0 |
| Warm field, inline | 0 | 0 | 0 |
| Getter, deferred | 10,000 | 0 | 10,000 |

The getter's legacy recovery is an explicit pending boundary. The inline
controls create no property action. `VmAction`, `Result<VmAction, Error>` and
`Frame` remain 16, 16 and 56 bytes in the C1 and C2 profiling binaries.
[Codegen accounting](data/v8-codegen-summary.json) and [ARM64 excerpts](data/v8-codegen-excerpts.md)
show static `Frame::next_pc` calls falling from one to zero in both
`read_progress` and `complete_read`. The C1/C2 `ready::run` spans are
10,652/10,660 bytes, `read_progress` 4,564/4,920, and `complete_read`
1,652/1,612. These spans and call sites describe compiled code, not dynamic
costs.

The [same-binary A/A control](data/v8-fixed-aa.json) and [C1/C2 paired run](data/v8-fixed-ab.json)
use two ABBA-ordered samples per binary and case, output checks, and macOS
retired-instruction counters. Every output matched. C2/C1 instruction ratios
are 0.9959 for the missing static read, 0.9944 for the computed call loop,
and 0.9923 for the computed one-frame read. The string-length and warm-field
inline controls are 1.0010 and 1.0001. The A/A instruction ratios are near
parity, but its missing-static wall medians differ by 18.3%. The paired wall
medians are mixed; they do not establish a runtime speedup or regression.
A later lower-load [compile-once A/A control](data/v8-execute-aa-quiet.json)
and [C1/C2 paired run](data/v8-execute-ab-quiet.json) used byte-identical
probe source (SHA-256 `3effd109dc4983d3d69cc9c91d4ae744ffca26a932e78f97b08f0f1d2d8db6d7`),
three warmups and nine timed executions per process. The computed one-frame
A/A ratio was 0.9913 and C2/C1 paired ratio was 0.9605. The missing-static
A/A ratio was 0.8699 while its paired ratio was 1.0033; the warm-field
inline control was 1.0058 in A/A and 1.0448 in A/B. These mixed controls
limit the runtime conclusion to a targeted observation under this run.

On this V8 base, seven focused property tests pass with profiling and six
pass normally. They cover action production, carried completion, publication
order under partial failure, fault source locations and deferred getter
identity. The pinned QuickJS fixture differential matched 13/13 cases, and
nine C oracle fixtures passed. The cumulative C1+C2 source at `80aef1f8c1b18381903fdfc7a1eecc9272fffa40`
passed the fixed-timeout full Test262 vector on this V8 base: **80,010 of
80,060 runnable** among 102,037 total variants. The TSV and JSONL SHA-256
hashes are `c5a335ea8b7db8ba47f401f9ed2b8c6168f25de62b31b575248c37dfbc6ea7b3`
and `707556fb5d6722c626095ad6c90d3b87668e1c83c9346deeaf0757c21861ad23`;
the runner engine semantics fingerprint is
`ed81c0f210d0b98745bfc52a843f9ae14f95336e88d264f5c37a8dc7f4d397f4`.
The pre-V8 full result remains tied to the [original receipt](README.md).
