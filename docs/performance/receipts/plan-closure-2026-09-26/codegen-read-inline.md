# Local/parameter wrapper inline experiment

Read-only comparison of immutable plain AArch64 binaries: `build-withdraw/release/qjs` (`5688c3e1`, SHA-256 `d7d70af2ccb86cc97697885410f3d6169a97f621afa0483530dcf235742f468a`) and `build-read-inline/release/qjs` (`708afeb6`, SHA-256 `c8d0e6d3454857101300bcb38a1f0cfc04dbebbb40bc17091d8b9073a197e959`). Their build receipts report Rust 1.96.0/LLVM 22.1.2, plain mode. No engine or build was run for this inspection.

`llvm-objdump --disassemble-symbols=... --no-show-raw-insn` outputs are in `codegen-read-inline/before-{run,local,parameter}.s` and `after-{run,local,parameter}.s`. In `run::run`, the old 15 `RunSlots::local` and 6 `RunSlots::parameter` `bl` sites remain at **the same addresses**, calling `SlotStore::local_current` and `SlotStore::parameter_current` instead. The two old wrapper symbols disappear; the two new callee symbols each have the same 96 instructions/384 B as their predecessors. The source bounds and slot checks remain in those callees, including their error paths.

| `run::run` | Before | After |
| --- | ---: | ---: |
| Decoded instructions | 5,215 | 5,215 |
| Symbol bytes | 20,860 | 20,860 |
| Entry local stack reservation | `0x750` | `0x750` |
| Out-of-line local/parameter calls | 15 / 6 | 15 / 6 |

After normalizing only the demangled target names in the three disassemblies, every instruction address, mnemonic, operand, and order matches (`normalized_diff=0` for `run`, local, and parameter). Thus `#[inline(always)]` removed the source-level wrapper symbol but **did not remove the machine call or its Result-return path** in this build. The experiment's codegen condition failed; a small benchmark difference would not establish the intended mechanism. Stop this candidate rather than extending the annotation experiment.
