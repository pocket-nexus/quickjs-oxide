# PGO with V8 v7 held out

Status: tooling experiment, not a performance result. Engine code remains
`eaf23ba8`; only build, validation, comparison tooling and this document change.
The Rust 1.88 toolchain is unchanged. Its matching `llvm-tools` component
provides LLVM 20.1.5 `llvm-profdata`; tools from LLVM 21/22 are not used.

## Mechanism and boundary

LLVM may use measured general execution frequency for inlining, block placement
and branch weights. This can reduce protocol overhead or improve code locality
without adding guest-language specialization. Neither a particular eliminated
spill nor an elapsed-time improvement is assumed.

`pgo_verified.py` builds with an explicit host target to avoid instrumenting
host build scripts. It trains the instrumented CLI on all 22 project-authored
scaling cases, sizes 64 and 128, 8,192 operations and one process each. All
outputs must match their independently generated expected values. No V8 v7
body participates in training. Existing `pgo.py` is not used: its soft failures
and v1 receipt do not provide the required evidence.

Each experiment uses a new external output directory. The chain preserves
clean commit/tree, compiler/configuration, actual verbose rustc commands,
generate binary, every training source/output and raw profile hash, the exact
merger/version, merged profile hash and use binary. Failure leaves evidence
and stops; profiles from another source or run are not reused.

Ordinary source comparison still rejects PGO. The new explicit
`--comparison build-technique` requires identical engine source and allows
exactly the declared `profile-use` codegen/environment difference. Features,
target, compiler, release profile and Cargo configuration remain equal. This
is a comparison between build techniques, not a source-layer speedup.

## Validation and acceptance

Synthetic validation tests reject changed raw/merged data, failed training,
different source/toolchain/environment, undeclared codegen changes and a PGO
receipt presented as ordinary release. Existing fixed-runner tests still pass.
No engine build, training, Test262 or timing result is claimed by these tests.

After training, compare release assembly and text/stack sizes, then serial
CPU 2 fixed A/A and balanced A/B controls over all eight cases and combined.
Use a separately frozen plan whose tooling identity names this comparison
implementation. Original combined output from repeated unmodified V8 v7 runs
remains the Score gate; reuse the unchanged historical Boa data.

An accepted build technique also needs full frozen Test262 compatibility,
representative independent workloads and resources. Account for instrumented
build/training/merge/use build cost, profile storage, rebuild requirements,
code size, peak RSS and startup. Reject control regressions or unresolved
target benefit rather than adding benchmark training to manufacture coverage.

PGO is not a remedy already proven for rejected source candidates. Any later
interaction experiment would require fresh profiles for each source and a
predeclared source × build-technique comparison.

Primary source guidance: the pinned
[Rust 1.88 PGO documentation](https://raw.githubusercontent.com/rust-lang/rust/1.88.0/src/doc/rustc/src/profile-guided-optimization.md)
and
[LLVM 20.1.5 llvm-profdata documentation](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-20.1.5/llvm/docs/CommandGuide/llvm-profdata.rst).
