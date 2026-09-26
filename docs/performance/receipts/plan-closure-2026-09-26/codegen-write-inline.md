# Ordinary write boundary: machine-code check

This is a static AArch64 release-code comparison. Both binaries were built with the same plain Rust 1.96 / LLVM 22.1.2 configuration; their successful build receipts are [`build-entry-read/release/qjs.build.json`](build-entry-read/release/qjs.build.json) and [`build-write-inline/release/qjs.build.json`](build-write-inline/release/qjs.build.json). The source revisions are `be855322ae09c3f92d746849559667fcaafc789e` and `b280ec8b77b9ff4d387118d31cfd5cbee644f477`. Binary SHA-256 values are `50abd7b8cb19ee7bb50b4e28a7591fa033475ebb50dd71eae150f8676f58e081` and `fd98515d63dbcf001fe836f6510e6315f2c0ed68fbeba0f2d0708012a0542249`, respectively.

The candidate only changes four inline annotations on `RunSlots::replace_local`, `RunSlots::replace_parameter`, `SlotStore::replace_local_current`, and `SlotStore::replace_parameter_current`. It does not alter their Rust checks, value moves, or error behavior.

| `run::run` static disassembly | `be855322` | `b280ec8b` | Change |
| --- | ---: | ---: | ---: |
| Decoded instructions | 5,403 | 5,521 | +118 |
| Symbol bytes | 21,612 | 22,084 | +472 |
| Stack reservation | `0x750` | `0x750` | none |
| `BL RunSlots::replace_local` | 6 | 0 | −6 |
| `BL RunSlots::replace_parameter` | 2 | 0 | −2 |
| `BL SlotStore::replace_*_current` | 0 | 0 | none |
| `BL Error::new` | 20 | 28 | +8 |
| `BL panic_bounds_check` | 8 | 8 | none |
| All `BL` instructions | 333 | 333 | none |

The write wrapper symbols present in the `be855322` binary are absent from `b280ec8b`; no distinct `SlotStore::replace_*_current` write symbols appear. In the candidate disassembly, the local write performs its bounds and occupied-slot checks inside `run` before the existing `release_frame_binding` call ([`b280-run.s`](codegen-write-inline/b280-run.s), around addresses `0x1004160a0–0x10041612c`). The parameter write likewise checks the logical parameter index, physical slot bound, and occupied slot in `run` (around `0x100418880–0x1004188bc`). The eight additional `Error::new` calls are error branches brought into the caller; they are not replacement calls to an outlined write helper.

This confirms that the specific cross-function `Result` return boundary was removed in the generated code while the checks remain. The larger static `run` body may affect code layout and cache behavior. Machine-code inspection alone cannot establish a speedup; the separately coordinated fixed-workload gate decides whether this candidate is retained. No engine, build, test, or benchmark was run for this review.
