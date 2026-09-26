# Ordinary local/parameter read inlining

Read-only AArch64 inspection of immutable plain builds: `build-withdraw/release/qjs` (`5688c3e1`, SHA-256 `d7d70af2ccb86cc97697885410f3d6169a97f621afa0483530dcf235742f468a`) and `build-read-current/release/qjs` (`8d3875dc`, SHA-256 `af682b67c0c302a3b98851570756d742871c8c26f10bbdf6c5890503e0679872`). Both build receipts report Rust 1.96.0 / LLVM 22.1.2 and plain mode. `llvm-objdump --disassemble-symbols=...run... --no-show-raw-insn` is saved as `codegen-read-inline/before-run.s` and `current-run.s`. No engine or build was run for this inspection.

| `run::run` | 5688 baseline | 8d candidate |
| --- | ---: | ---: |
| Calls to ordinary local read | 15 `RunSlots::local` | 0 |
| Calls to ordinary parameter read | 6 `RunSlots::parameter` | 0 |
| Decoded instructions | 5,215 | 5,424 (+209) |
| Symbol bytes | 20,860 | 21,696 (+836) |
| Entry local stack reservation | `0x750` | `0x750` |

No `RunSlots::{local,parameter}` or `SlotStore::{local_current,parameter_current}` symbol remains in the candidate. The 21 read calls were not replaced by another outlined read helper. Local bounds, backing-slot bounds and vacant-slot branches are visible inside `run::run` at `current-run.s:1624–1637`; the analogous parameter checks follow the same source body in `src/engine/vm/stack.rs:1530–1538`. The candidate has more direct `Error::new` calls in `run` (4→20) because cold error construction also moved into the caller. This confirms the success and failure paths remain represented; it does not prove an end-to-end speed gain.

This second annotation experiment meets its narrow codegen condition: it deletes the per-read out-of-line call/Result boundary. It also grows the interpreter body by 836 B and 209 static instructions, so admission depends on the separate combined plain gate. If that gate fails, stop this annotation route rather than extending it to other helpers.
