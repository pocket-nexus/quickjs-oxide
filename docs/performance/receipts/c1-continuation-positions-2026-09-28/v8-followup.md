# C1 follow-up on the #53 V8 extension

PR #53 advanced its VM and compiler at `734bc1285610652baccfd9373033ff652d2fc1e0`.
This follow-up rebuilt #53 and C1 with Rust 1.88.0 at that base. The later
#53 commit `bbd8adaf` changed documentation only; `git diff --quiet
734bc128..bbd8adaf -- src apps scripts Cargo.toml Cargo.lock` passes.
The measured C1 source is `9a9290dff8e2e7a18f3d2b54284ec518565a1a5d`;
the first rebased C1 revision `0bc98c9400ae85b2009aa39df4eac02d9deeaeb2`
and its later receipt-only commits have identical production Rust code. Earlier receipts remain tied to their stated
revisions.

[Build provenance](data/v8-builds.json) gives clean source trees, release
flags and binary hashes. [Profiling counts](data/v8-profile-counts.json)
show 1,000,000 carried completions and zero numeric recovery decodes in both
the per-call and long-lived-frame primitive workloads. The Number-only and
M1 hit controls have no numeric action. Object decline and comparison each
retain 100,000 legacy recovery decodes; these paths remain outside C1.
The C1 profiling binary reports 16-byte `VmAction`, 16-byte
`Result<VmAction, Error>` and 56-byte `Frame` layouts.

[Codegen accounting](data/v8-codegen-summary.json) and [ARM64 excerpts](data/v8-codegen-excerpts.md)
show two static `Frame::next_pc` calls in baseline `ready::run` and one in
C1. The symbol span changes from 10,980 to 10,652 bytes; the V8-expanded
`execute_frame` span changes from 29,340 to 28,808 bytes. These are
static compiler-output observations, not per-operation timings.

The [A/A control](data/v8-fixed-aa.json) and [#53/C1 paired run](data/v8-fixed-ab.json)
use the same fixed output contracts, two ABBA-ordered samples per binary and
case, and macOS retired-instruction counters. Every output matched. In the
paired run, C1/#53 retired-instruction ratios are 0.9944 for the per-call
primitive probe, 0.9658 for the long-lived-frame primitive probe, and
0.9991 for the Number-only control. The A/A ratios for the two primitive
probes are close to parity in retired instructions. Other paths also shift
slightly under code generation, so these totals do not isolate the cost of a
single decode. Same-binary A/A wall medians differed by 11.5% on the
per-call probe; the paired wall medians are not a reliable speedup estimate.
A separate [compile-once A/A control](data/v8-execute-aa.json) used byte-identical
probe source (SHA-256 `3effd109dc4983d3d69cc9c91d4ae744ffca26a932e78f97b08f0f1d2d8db6d7`)
and excluded startup and compilation. Its same-binary one-frame median differed
by 24.0%, with individual primitive samples spanning 0.58–2.35 seconds.
An unrelated worktree oracle test was active. We did not run a compile-once
A/B comparison under that load or claim an elapsed-time improvement.

On this V8 base, 18 focused numeric tests pass both normally and with
profiling, including actual wide comparison fallback, PostInc partial output,
materialization retry, fault attribution, and the scoped no-recovery oracle.
The C1 full Test262 outcome on its earlier M2 base is documented in
[m2-followup.md](m2-followup.md); it is not relabeled as a V8-base result.
The cumulative C1+C2 source at `80aef1f8c1b18381903fdfc7a1eecc9272fffa40`
passed the fixed-timeout full Test262 vector on this V8 base: 80,010 of
80,060 runnable variants across 102,037 total. Its TSV SHA-256 is
`c5a335ea8b7db8ba47f401f9ed2b8c6168f25de62b31b575248c37dfbc6ea7b3`;
the runner engine semantics fingerprint is
`ed81c0f210d0b98745bfc52a843f9ae14f95336e88d264f5c37a8dc7f4d397f4`.
This is cumulative validation of C1 and C2, not an isolated C1 full replay.
