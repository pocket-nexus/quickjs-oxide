# C1: decoded fallthrough for primitive numeric completion

The [M2-base follow-up](m2-followup.md) records the later rebase onto #53
`38e9eb86`, stronger boundary tests, fresh builds, and repeat measurements.
The measurements below remain tied to their original #53 `10262309` base.

This receipt starts from PR #53 at
`10262309c580c43ba984b4f371acdc081147b4f0`. The change carries the
published instruction's decoded next boundary in `VmAction::Numeric`, through
the ready driver, into same-frame primitive arithmetic completion. The frame
still commits `resume_pc` at the existing completion point. `fault_pc` remains
the attribution site, and decline keeps the action and its position for the
existing fallback protocol.

The covered path is primitive arithmetic after the inline Number path declines,
including string, Boolean, null and BigInt arithmetic. `NumericStep`
comparison completion, object conversion after decline, properties, calls,
delayed replies and suspended frames still use their existing position logic.
The diagnostic event `numeric_legacy_fallthrough_recovery_decode` counts one
of those remaining numeric recovery sites; it does not count ordinary opcode
decoding. No interpreter entry validation was removed.

## Correctness contract

- The carried boundary comes from the originating published instruction, not
  `fault_pc + 1` or a selected branch destination.
- Knowing fallthrough does not commit execution. A decline leaves fault and
  resume positions unchanged; a materialization retry executes the current
  operation again.
- Primitive numeric completion retains the existing output order and commits
  the resume position only after the output pushes succeed. A partial output
  failure leaves the original positions intact.
- The existing frame identity, slot ownership and pending-operation protocols
  remain authoritative. The position wrapper is not an authentication token.

The focused tests inspect the emitted `Sub` and `CompareBranchStack`
instructions, compare carried boundaries with the reference decoder (including
a wide encoding), and cover direct completion, object decline, guest throw
identity and conversion count, partial output failure, and retry. The
profiling-feature tests assert that covered primitive completion occurs and
that its legacy recovery decode count is zero.

## Reproduction and results

The committed `manifest.json` and `workloads/` contain fixed byte hashes and
stdout contracts for primitive, Number-only, object-conversion, comparison,
M1 hit and M1 miss cases. `apps/cli/examples/execute_probe.rs` compiles these
cases once and times only repeated `Context::execute` calls; fixed-runner
timings include CLI startup and compilation. The plain binaries are used for
timing, while profiling binaries establish path coverage.

The fresh builds used Rust 1.88.0, `--locked`, the repository's release
profile (fat LTO, one codegen unit), separate target directories and clean
source commits. [Build provenance](build-provenance.json) records all four
plain/profiling binary hashes and flags. The baseline is the exact #53 head;
the measured candidate is `adf922f82a8b343da95047b7bab0f5608195efde`.
The receipt documentation and measurement script were added after that engine
build, without changing production Rust code.

[Profiling counts](profiling-counts.json) are from the candidate profiling
binary; they attribute work, not elapsed time:

| Fixed case | Numeric exits | Carried primitive completions | Declines | Legacy numeric recovery decodes |
| --- | ---: | ---: | ---: | ---: |
| primitive | 1,000,000 | 1,000,000 | 0 | 0 |
| Number-only | 0 | 0 | 0 | 0 |
| object | 100,000 | 0 | 100,000 | 100,000 |
| comparison | 100,000 | 0 | 0 | 100,000 |
| M1 dense hit | 0 | 0 | 0 | 0 |
| M1 miss | 2,000,000 | 2,000,000 | 0 | 0 |

The miss case produces NaN through primitive arithmetic; its large C1 count
is expected. The comparison case uses `NumericStep` and remains unmigrated.
Both builds have a 16-byte `VmAction`, 16-byte `Result<VmAction, Error>` and
56-byte `Frame` on this 64-bit target. The baseline sizes came from a
temporary test-only `size_of` probe after its clean release build; that probe
was removed again. The candidate sizes are recorded by the profiling binary.

[Release disassembly accounting](codegen-summary.json) shows two static
`Frame::next_pc` call sites in baseline `ready::run` and one in the candidate.
The remaining site serves an unmigrated path. The symbol-to-next-symbol span
of `ready::run` is 10,980 to 10,652 bytes, while `execute_frame` is 27,024 to
26,576 bytes. The entire Mach-O `__text` section is 5,321,308 to 5,320,548
bytes. These are generated-code observations for these binaries, not a claim
that source line counts translate directly to runtime cost.

## Timing evidence and limits

The [whole-process paired run](data/fixed-paired.json) used eight ABBA-ordered
samples per binary and case, verified every expected output, and captured
macOS retired instructions, cycles and RSS with `/usr/bin/time -l`.
The [compile-once paired run](data/execute-paired.json) used the byte-identical
probe source (SHA-256 `8abf4fb9e928f014e1335c92f372840474aed34b477a5cba433a087179bae3c2`)
in both source trees: four ABBA-ordered processes per case, three warmups and
nine timed `Context::execute` calls per process. Its samples exclude process
startup and compilation. The baseline probe source was copied into the #53
worktree only for this measurement and removed afterward; it was not part of
the clean baseline `qjs` build.

| Case | Whole-process baseline / candidate ms | Execute-only baseline / candidate ms | Candidate / baseline execute |
| --- | ---: | ---: | ---: |
| primitive | 2820.0 / 2749.0 | 1013.2 / 1043.2 | 1.030 |
| Number-only | 2427.2 / 2386.6 | 884.6 / 900.1 | 1.017 |
| object | 659.0 / 587.3 | 386.9 / 380.1 | 0.983 |
| comparison | 495.7 / 485.7 | 164.8 / 163.4 | 0.991 |
| M1 dense hit | 487.6 / 548.2 | 460.2 / 483.5 | 1.051 |
| M1 miss | 2504.8 / 2575.8 | 2708.8 / 2609.8 | 0.963 |

**These medians do not establish a speedup or regression.** During this run,
the host load average exceeded 190 and independent repository builds and
conformance runs were active. The primitive whole-process retired-instruction
ratio was 1.0006 (candidate/baseline), even though its covered recovery call
site is gone. Candidate and baseline execute samples have wide, overlapping
ranges; the primitive baseline range was 915–2367 ms and candidate range
896–1318 ms. A quiet-host repeat is required to estimate runtime impact and
to resolve the apparent M1 hit cost. The counter and sample files are retained
so this limitation is auditable rather than hidden.

## Validation

Rust 1.88.0 workspace/all-targets `cargo check`, default and profiling
`clippy -D warnings`, `cargo fmt --check`, and default workspace/all-targets
`cargo test` passed. Focused numeric tests passed with and without profiling;
the focused Test262 vector passed **6,844/6,844** eligible variants. Source
layout and frozen Test262 configuration authentication passed. Full Test262
matched its frozen vector: 102,037 total, 80,010 pass, 3,552 fail,
3,502 unsupported and 18,475 skipped; **80,010/80,060 runnable pass**.
The full TSV SHA-256 is
`f740749201dd96bd844b7b32e96661f78e216fb28c4e13aff11683d4be59df7f`.
The Rust runner engine semantics fingerprint is
`ffb5ca6e9ceb1a5078caad2e70ec0525b989d31691f3d54fe3280a32900f8ac3`.
The frozen `fail` and `unsupported` classes are existing classifications,
not C1 regressions.
