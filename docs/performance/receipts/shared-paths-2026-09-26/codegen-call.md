# Ordinary frame entry and cleanup codegen audit

## Comparison identity

Both are clean, plain release ARM64 builds made with Rust 1.96.0, `opt-level=3`, fat LTO, one codegen unit and no optional features. The final and array-control source trees differ only in the two call-path commits (`a7cbff88` published ordinary local initialization fact and `33f82acf` scalar frame cleanup), plus the tests in those files. All property and Array read changes are shared by these two source trees. No binary was executed during this audit.

| Build | Source commit | Binary SHA-256 |
| --- | --- | --- |
| Array-control, call changes withdrawn | `af5b59026a741dc7a6d6a4b3836e4ecceb11e893` | `b7fbc1f993bac566e5b5c6a72f5b175f1c3ab21e505e595d78327352a53d2ec2` |
| Final candidate | `ae81f0907bed190e021dbab2560ec0a25d420a82` | `1b57d826084757e2e9090fa3a45312601fd777bee85d9ce2e1b7a8dff47b0f6f` |

Build receipts and precise disassembly commands appear in [codegen-call](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-call/): run `nm -an <binary>` and `/opt/homebrew/opt/llvm/bin/llvm-objdump -d --disassemble-symbols=<mangled-symbol> <binary>`. The exact symbol names, addresses, all `bl` sites, and raw assembly are in [array-control-metadata.json](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-call/array-control-metadata.json) and [final-metadata.json](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-call/final-metadata.json).

## Ordinary frame installation

`push_ordinary_frame` is inlined into `OrdinaryCall::install` in both binaries; there is no separate symbol to size. Its enclosing symbol grows from 942 ARM64 instructions / 3768 B / 464 B frame to 988 instructions / 3952 B / 480 B frame. Static code growth is expected because the final binary retains the general initialization branch and adds an authenticated plain branch.

The final entry checks the published `plain_local_initializers` bit once at `0x10041c900`–`0x10041c904`. On the true route, `0x10041c95c`–`0x10041c974` writes four Undefined slot tags per loop iteration, with a scalar remainder at `0x10041c984`–`0x10041c98c`; it jumps past the general local-initialization path to `0x10041cac4`. That loop contains no per-local `is_lexical`, `function_name`, owner retain, or fallible constructor call. The general route beginning at `0x10041c994` still handles lexical and function-name locals. The array-control binary has only that general route: near `0x10041c91c`–`0x10041c9a4` it compares each local index with the function-name index and reads the local-definition flag before writing the slot.

The fast branch therefore removes the intended **per-local classification and initialization work when the published fact is true**. It does not remove the one entry guard, the general branch's static footprint, or other setup/argument handling. There is no new outlined initializer call on the plain loop. The larger enclosing symbol and 16 B larger stack frame remain possible front-end costs; timing needs measurement.

## Frame cleanup

`SlotStore::clear_frame` grows from 143 instructions / 572 B to 148 instructions / 592 B; both use a 208 B stack frame. The array-control loop calls `release_frame_binding` for every occupied slot at `0x100170394`. The final loop checks direct scalar tags at `0x100170368`–`0x100170370` and proceeds directly to the next slot; only the remaining edge-bearing or captured cases call `release_frame_binding` at `0x1001703bc`. The generic release function remains 77 instructions / 308 B, and its ARM64 instruction words are identical between these binaries.

Thus the common scalar route really skips the generic release call in generated code. The added tag checks and branches still execute per occupied slot. This static result does not establish the resulting cycles or the fraction of calls using the published plain fact; those require the fixed workload and logical coverage measurements.
