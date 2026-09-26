# be855322 entry choice + local/parameter read codegen

This is a read-only AArch64 codegen comparison. No engine, build, or test was run for this inspection.

## Binary identity

The complete `oxide-build-v2` receipt for `build-entry-read/release/qjs` reports plain stack-VM mode, no features, successful exit, and commit `be855322ae09c3f92d746849559667fcaafc789e`. Its `binary_sha256` matches the actual qjs bytes: `50abd7b8cb19ee7bb50b4e28a7591fa033475ebb50dd71eae150f8676f58e081`. The receipt's source commit, commit environment, and current source checkout agree; source status is clean. Build stdout, stderr, and both tooling snapshots exist and match their recorded hashes. All three compared receipts use Rust 1.96.0 / LLVM 22.1.2, `--locked --release --no-default-features`, fat LTO, and one codegen unit.

| Binary | Commit | SHA-256 |
| --- | --- | --- |
| `build-plain/release/qjs` | `c7fb5b69` | `b241d0252cfe19d54b18843ae106f91c4cb66f2d7f462f120ce21e2d643f3573` |
| `build-read-current/release/qjs` | `8d3875dc` | `af682b67c0c302a3b98851570756d742871c8c26f10bbdf6c5890503e0679872` |
| `build-entry-read/release/qjs` | `be855322` | `50abd7b8cb19ee7bb50b4e28a7591fa033475ebb50dd71eae150f8676f58e081` |

Method: locate the exact `quickjs_oxide::engine::vm::run::run` symbol with `llvm-nm -n`; disassemble only that symbol with `llvm-objdump --disassemble-symbols=... --no-show-raw-insn`; count decoded AArch64 instruction lines and exact `bl` targets. Symbol bytes are the distance to the next distinct-address symbol, which equals decoded instructions × 4 here. Saved disassemblies are [`c7-run.s`](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/codegen-entry-read/c7-run.s), [`8d-run.s`](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/codegen-entry-read/8d-run.s), and [`be-run.s`](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/codegen-entry-read/be-run.s).

## `run::run` result

| Static property | c7 | 8d read inline | be entry choice + read inline |
| --- | ---: | ---: | ---: |
| `bl` to ordinary local read | 15 | 0 | 0 |
| `bl` to ordinary parameter read | 6 | 0 | 0 |
| All `bl` in symbol | 337 | 334 | 333 |
| Decoded instructions | 5,197 | 5,424 | 5,403 |
| Symbol bytes | 20,788 | 21,696 | 21,612 |
| Entry `sub sp, sp` | `#0x750` | `#0x750` | `#0x750` |

The 4-inline local/parameter read change still removes all **21** ordinary read call sites in be: no `bl` to `RunSlots::{local,parameter}` or `SlotStore::{local_current,parameter_current}` remains. There is no separate corresponding read symbol in be. Relative to 8d, be's `run` is 21 instructions / 84 bytes smaller, with the same entry stack reservation. Relative to c7, be is still 206 instructions / 824 bytes larger. These are static code properties, not dynamic instruction or cycle estimates.

The be source contains the single-choice dispatch: `LocalFusionChoice` and `FusionEntry::local_choice` classify one published flag in `src/engine/code/fusion.rs:27–64`; `run` first rejects an empty entry and then matches that choice at `src/engine/vm/run.rs:1453–1618`, falling through to the canonical local read at line 1619. The 8d→be source diff changes `fusion.rs` and `run.rs`; it does not alter the local/parameter slot-read implementation. This confirms that the combined candidate retains the read inlining and changes the entry selector, without identifying a performance cause from code shape alone.

## Gate and attribution

[`entry-read-object-gate-summary.json`](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/entry-read-object-gate-summary.json) records an eight-repetition be-vs-be Object A/A (cycles median change −0.424%) and an eight-repetition Object A/B against shipping commit `29560e08`: cycles median **276,508,678.5 → 285,158,042.0, +3.128%**; whole-process wall median **+3.275%**; retired instructions **−4.119%**. The combined be candidate therefore **fails the Object cycle gate**, despite removing the 21 read calls. The broader parent gate also records Object cycles +2.905% against the same baseline.

That A/B compares the combined entry-choice/read-inline candidate with `29560e08`, not be directly with 8d. The earlier [`read-current-parent-gate-summary.json`](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/read-current-parent-gate-summary.json) records 8d vs the same shipping commit at Object cycles −2.164%, in a separate run. The +3.128% combined-candidate regression cannot be assigned to standalone read inlining; nor does this static comparison alone prove the entry selector caused it. Both gate summaries state that other same-host workloads were allowed, so the measured figures retain that environmental limit.
