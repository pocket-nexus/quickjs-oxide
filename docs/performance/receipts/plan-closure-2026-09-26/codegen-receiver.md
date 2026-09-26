> Historical rejected receiver-transfer candidate. Machine-call deletion alone did not pass the cumulative gate; this implementation is not delivered.

# Method receiver transfer: release codegen inspection

Compared the immutable plain AArch64 binaries for `c7fb5b69` and `5e3dedc9` using the same Rust 1.96.0 / LLVM 22.1.2, release `lto=fat`, `codegen-units=1` build profile recorded in `fixed-receiver/results.json`. Binary paths, SHA-256 and disassembly hashes are in [receipt.json](receipt.json). No engine or build was run for this inspection. The baseline predates the receiver transfer; intervening product changes are present, so an individual assembly difference is not automatically caused by that patch.

## Ordinary install

`OrdinaryCall::install` is a separate binary symbol. `SlotStore::push_ordinary_frame` has no separate symbol and its body is inlined into install; there is therefore no out-of-line tuple-return calling convention on this build. Within install:

| Instruction/site | Baseline | Final | Evidence |
|---|---:|---:|---|
| `SlotStore::peek` calls | 1 | 0 | `install-before.s:58`; absent after |
| `copy_reference` calls | 2 | 1 | before lines 176, 416; after line 391 |
| `release_frame_binding` calls | 2 | 0 | before lines 645, 671; absent after |
| `retain_object_handle` calls | 1 | 1 | before line 522; after line 491; retained for named-local initialization |
| Local stack reservation (`sub sp`) | `0x1a0` | `0x320` | before line 15; after line 15 |
| Symbol text size / decoded instructions | 3,752 B / 938 | 3,960 B / 990 | symbol ranges in receipt |

The final callee release is a direct `release_or_defer` at `install-after.s:616`; its second `release_or_defer` at line 710 is in cold-frame replacement, after `CallStorage::vacant`, and must not be counted as a second outgoing callee release. The receiver value is taken from the caller slot into a stack-resident `CallInput` before cold-frame installation. Its strong edge is neither retained nor released at the transfer point. The remaining `copy_reference` is in the argument-copy loop, so this proves the old receiver-specific peek/retain/release sequence was removed from the generated code.

The larger stack frame and 52 extra decoded instructions show that inlining the tuple/RAII path has codegen cost elsewhere. They include error cleanup and cold code; they are **not** a measured per-call dynamic instruction delta. A focused A/B with semantic checks is still needed for the net performance effect, including nonmethod calls.

## `string_bridge` regression investigation

The fixed `string_bridge.js` loop performs string addition and comparison; its only ordinary call is the single `work(180000)` entry. It executes no method call per iteration. The `run::run` symbol has 5,197 decoded instructions on each side with identical mnemonic sequence. `numeric::add_primitives` (515) and `JsString::try_concat_in_place` (136) likewise have identical mnemonic sequences; their source files did not change between these commits. Symbol addresses, operands and surrounding layout changed, so this does not establish identical machine bytes, branch outcomes or runtime GC behavior. It does show that the receiver transfer has no direct repeated path in this loop.

The first fixed comparison showed a string-bridge retired-instruction rise, but the later same-binary A/A range and balanced A/B changed its magnitude. It remains an unresolved dynamic-path observation. Do not call it a proven tuple ABI cost or a pure timing fluctuation. Keep it in the whole-matrix decision and inspect branch/GC/allocator-path counts if a stable repeat persists.
