# PGO with V8 v7 held out

Status: the held-out experiment completed fixed timing, original Scores,
resources and full frozen Test262. Its engine code is `eaf23ba8`; root tooling
commit `e96a7f7f` adopts the verified workflow. The root includes a later factory
correctness fix and requires fresh training and acceptance before claiming its
PGO performance. Source-specific profiles are never reused across that change.
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

## Completed held-out experiment

Clean experiment `17c6c883` produced 44/44 correct training outputs and raw
profiles. CPU 2 fixed timing completed 144/144 samples, with all eight cases
and combined improving outside their observed A/A span. Four original full
combined runs per binary gave median Score 188.5 (none) versus 245 (PGO),
+29.97%; all eight sub-scores improved. Crypto joins RegExp and NavierStokes
above the unchanged historical Boa samples; five sub-scores and combined
remain below Boa. This is not completion of the overall parity objective.

The fixed whole-process resource diagnostic had cycles -22.17%, instructions
-14.50%, RSS median +0.089% and all counters running 100%. `.text` decreased
213,936 bytes and `.rodata` grew 52,448 bytes. Ready's native frame grew from
1,320 to 3,192 bytes as the independent execute frame was inlined; the old
execute frame itself used 1,176 bytes. Those individual sizes are not a
measured peak stack, and ordinary JS frame depth does not multiply native ready
activations. Existing mixed string search and typed array callback finite,
overflow and recovery scripts passed both actual CLI binaries with child
main-thread soft/hard stack limits of 2 MiB. This does not replicate Rust worker
stack allocation or prove every reentry family.

The PGO conformance runner used the same source and merged profile with
observed rustc flags/target recorded. All 102,037 result variants matched the
frozen TSV/JSONL bodies, with 80,010 passes among 80,060 eligible variants.
Its host binary/features differ from the timed CLI and are identified separately.

Artifacts are external under `oxide-v8v7-boa-campaign/heldout-pgo-*`; see
`heldout-pgo-original-summary.json`, `heldout-pgo-resources/summary.json`,
`heldout-pgo-test262/result.json` and `heldout-pgo-stack-regressions/results.json`.
Actual source/target/profile/binary hashes remain in their original receipts.

Primary source guidance: the pinned
[Rust 1.88 PGO documentation](https://raw.githubusercontent.com/rust-lang/rust/1.88.0/src/doc/rustc/src/profile-guided-optimization.md)
and
[LLVM 20.1.5 llvm-profdata documentation](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-20.1.5/llvm/docs/CommandGuide/llvm-profdata.rst).
