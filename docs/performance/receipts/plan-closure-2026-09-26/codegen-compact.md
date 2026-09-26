# Compact receiver guard: plain-code inspection

Read-only AArch64 `llvm-objdump --disassemble-symbols=...OrdinaryCall7install... --no-show-raw-insn` on three immutable plain binaries. Build receipts are `build-*/release/qjs.build.json`; each reports Rust 1.96.0 / LLVM 22.1.2. No engine, build, or benchmark was run for this inspection. Saved disassembly is in `codegen-compact/install-{shipping,final,compact}.s`.

| Source | Binary SHA-256 | `OrdinaryCall::install` local `sub sp` | Static instructions / bytes |
| --- | --- | ---: | ---: |
| `2d0eca7f` (`build-shipping`) | `e4e9d2310bdb6746cf88a6b2fbd75af8cde98ab5727a0fb7681fff0dcc23e32a` | `0x320` (800 B) | 990 / 3,960 |
| `5e3dedc9` (`build-final`) | `1a0b7dbae0c0b92ee2fa9449bd955900d261a1e9b316ba7a9fa4f9ca7a08c5f0` | `0x320` (800 B) | 990 / 3,960 |
| `b19003ea` (`build-compact`) | `5bd65e8466b5dc9ebb69e541ba646b158550ae8a6a3dc17f59a50bbeb38d7100` | `0x330` (816 B) | 1,011 / 4,044 |

The compact guard **did not reduce** the install stack reservation: it added 16 B and 21 static instructions versus both the `2d0` and `5e3` binaries. The guard conversion was inlined, but the compact binary has a separate `drop_in_place<OrdinaryReceiverOwner>` symbol. `install-compact.s:656` contains a `bl` to it on the install continuation; lines 971 and 987 contain further unwind/cleanup calls. The old builds have no such symbol or call. This is machine-code evidence of additional guard cleanup work, not a measured dynamic call count or an explanation for the whole Object8 cycle delta.

The proposed mechanism for this second receiver candidate was smaller temporary state. That specific codegen condition failed. Together with the root's negative Object8 gate, the bounded receiver optimization should stop here; this inspection does not justify another receiver rewrite.
