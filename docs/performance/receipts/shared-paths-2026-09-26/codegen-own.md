# Ordinary own-read codegen audit

## Identity and method

This is static ARM64 code inspection only. Neither binary was executed for this audit.

| Role | Source commit | Binary SHA-256 | Toolchain and build |
| --- | --- | --- | --- |
| Baseline | `b280ec8b77b9ff4d387118d31cfd5cbee644f477` | `fd98515d63dbcf001fe836f6510e6315f2c0ed68fbeba0f2d0708012a0542249` | Rust 1.96.0, aarch64-apple-darwin, plain release, opt-level 3, fat LTO, one codegen unit, no features |
| Own-read candidate | `dc3d298adde2731d7911ec93084c92bc9d03f01c` | `1f3ece54df9ddd829718f6bfa1ab63196509cfbc70cb9e4315afea7386808b87` | Same settings |

Sources are clean worktrees in the respective build receipts. The product-code difference between these commits is confined to `ordinary_storage.rs` and `ordinary_storage/ic.rs`; intervening repository changes are documentation. Commands: `nm -an <binary>` and `/opt/homebrew/opt/llvm/bin/llvm-objdump -d --disassemble-symbols=<mangled-symbol> <binary>`. The exact extraction program, receipts, symbol names, addresses, calls, and raw disassembly are in [codegen-own](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-own/), notably [baseline-metadata.json](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-own/baseline-metadata.json) and [own-metadata.json](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-own/own-metadata.json).

## Generated-code changes

| Symbol | Baseline ARM64 instructions / bytes / stack frame | Candidate instructions / bytes / stack frame |
| --- | ---: | ---: |
| `property_ic_read_fast` | 370 / 1480 / 240 B | 258 / 1032 / 160 B |
| `immediate_field_in_state` | 246 / 984 / 80 B | absent |
| `uncached_field_in_state` | absent | 233 / 932 / 112 B |
| `promote_field_in_state` | absent | 236 / 944 / 144 B |

The property-IC entry became 448 B smaller, but these listed symbols together grew from 2464 B to 2908 B. Static size is not a time measurement and the symbols are not all executed on every read.

The cache-hit route still calls `PropertyReadCache::read` (baseline `0x100427934`, candidate `0x100427930`). **A successful candidate hit now makes an outlined `bl promote_field_in_state` at `0x1004279c0`; the baseline's value promotion was inside `property_ic_read_fast`.** The smaller entry and frame therefore do not establish cheaper cache hits. The promotion helper itself has a 144 B frame and contains the type dispatch and owner retain. This extra hit-path call is an explicit generated-code cost to measure.

On a cache miss or a missing site, the baseline calls `immediate_field_in_state` (`0x100427b0c`, `0x100427b58`), whose source accepts only scalars and therefore sends own `Object`/`String` results to the VM property driver. The candidate calls `uncached_field_in_state` (`0x100427ad8`, `0x100427b8c`). That helper calls `locate` at `0x100427e00`, checks the own Data slot, then tail-branches to `promote_field_in_state` at `0x100427e74`. The promoter handles `RawValue::Object` and `RawValue::String` with owner retention. Thus an eligible ordinary own-data Object/String result can complete without a second driver lookup. A missing, accessor, inherited, exotic, or failed-retain case still declines.

The candidate does **not** remove the cache lookup itself. `uncached_field_in_state` also re-fetches the already known object inside `locate`; whether this matters in generated code or aggregate timing requires measured evidence. The current asm proves the changed call topology and owner path, but not a net speedup, cache behavior, or branch-prediction cause.

As a control, `peek_dense_number` is 81 instructions / 324 B in both of these binaries; the Array read change is not yet part of the own-read candidate.

## Later materialized Array read candidate

The clean final build `ae81f0907bed190e021dbab2560ec0a25d420a82` (`SHA-256 1b57d826084757e2e9090fa3a45312601fd777bee85d9ce2e1b7a8dff47b0f6f`) also includes the Array read and call-frame changes. In it, `peek_dense_number` is 96 instructions / 384 B versus 81 / 324 B in the own-only binary, and the new `materialized_array_own_number` is an outlined 181 instructions / 724 B. The `peek_dense_number` slow-Array branch calls that helper at `0x100429524`; dense storage continues at `0x100429538` and does not call it. The ordinary VM `array_immediate_read` grows from 491 to 507 instructions and calls the same helper at `0x10041dc8c` on its slow-Array path. This confirms that both fused and ordinary reads share the indexed own-slot selector in machine code.

An ablation build `af5b59026a741dc7a6d6a4b3836e4ecceb11e893` removes only the later call-frame changes. Its `property_ic_read_fast`, Array peek, Array helper and `array_immediate_read` have the same instruction counts as the final build. The extra helper call and the larger Array entries are genuine code costs. They are not yet timing results; in particular, no cache or branch behavior follows from the disassembly alone. Raw final and ablation assembly and receipts are in [codegen-own](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths/codegen-own/).
