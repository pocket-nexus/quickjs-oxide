# Object local write: static codegen comparison and narrow candidate

This is a source and ARM64 disassembly audit, **not a timed measurement**. This audit did not launch an engine, build, test, or benchmark; it reads separately built plain-release artifacts. Addresses below are virtual addresses in each separate Mach-O binary; compare control flow, not absolute address deltas.

## Identity and reproduction

| Role | Source and `run.rs` blob | Plain release CLI SHA-256 | Build receipt SHA-256 |
| --- | --- | --- | --- |
| Parent | `29560e08e37e7e3465fb355be5641b26da374576`, tree `8bcfd3ede3ddbf198d1d0041e41462fcb4b3cd97`; `3dd3947028f79e2a40dae24e55f7f9257a14d789` | `4fbc635f3f8d02a9e29fec39f568177742d5a3f1d5dee0de845ef29c92215a9a` | `e1c625a88004f165f72de13515251ae758077b04a998145c7b7aff7c9170d513` |
| Current | `c7fb5b69d88bb5e6170fab3b8af5a3062fd4cb72`, tree `a772566bd64d079502ec931139b038f775a6bfd4`; `cfef174f8ba8ac02452ae56ee0e63d4310c1e86b` | `b241d0252cfe19d54b18843ae106f91c4cb66f2d7f462f120ce21e2d643f3573` | `469a6befb0fe6344739d60f5a8bff697d82b190bc81ee4a8a2e849dcadd64fb3` |

The current binary was built at `c7fb5b69`; subsequent documentation commit `4ef28f74ee258f306b6ae3fc1a29ee508701cd89` has the **same** `run.rs` blob (`cfef174f...`). Both receipts record clean source, `rustc 1.96.0` / LLVM 22.1.2 / `aarch64-apple-darwin`, `stack-vm`, plain release, and `cargo build --locked --release --verbose -p quickjs-oxide-cli --no-default-features --target-dir <distinct target> --jobs 2`. Read the receipts for effective Cargo/rustc flags; this memo does not infer them from `Cargo.toml`.

The audited binaries and receipts are:

```text
/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/build-parent/release/qjs
/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/build-parent/release/qjs.build.json
/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up/build-plain/release/qjs
/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up/build-plain/release/qjs.build.json
```

The probe is the project-authored `docs/performance/receipts/ordinary-number-writes-2026-09-26/workloads/object_move.js`, SHA-256 `48359d0b20525b04ae3c3652de9709afddcd6cbc495ded9f267bdc9f1c1fcbd6`. It repeats `x=y` for one million iterations. After the first replacement, `x` and `y` normally own the same Object; `slot_object_release_readiness_fast` returns `Ready` when no deferred/zero-queue condition intervenes and strong count exceeds one. This is a source-level expectation, not a dynamic count of Ready outcomes in this probe.

To reproduce the symbol and instruction locations without executing either engine:

```sh
PARENT=/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/build-parent/release/qjs
CURRENT=/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up/build-plain/release/qjs
RUN_SYMBOL='__ZN13quickjs_oxide6engine2vm3run3run17h7ab689084215b178E'
shasum -a 256 "$PARENT" "$CURRENT"
llvm-nm -an "$PARENT" | rg 'run3run|slot_value_release_readiness_jsvalue|store_proven_number_operand'
llvm-nm -an "$CURRENT" | rg 'run3run|slot_value_release_readiness_jsvalue|store_proven_number_operand'
llvm-objdump --disassemble-symbols="$RUN_SYMBOL" "$PARENT" | rg -n -C 8 'slot_value_release_readiness_jsvalue|replace_local|release_displaced'
llvm-objdump --disassemble-symbols="$RUN_SYMBOL" "$CURRENT" | rg -n -C 8 'slot_value_release_readiness_jsvalue|store_proven_number_operand|replace_local|release_displaced'
```

## What the source and machine code establish

Parent's canonical local Put/Set arm calls `RunSlots::local`, checks the old direct binding's release readiness, then handles the `Ready` path with the normal `peek/pop → replace_local → release_displaced` sequence. Its relevant ARM64 range starts at `run+0x2fd8` (`0x100415de8`); the second local read starts at `0x100415e0c`, readiness call is `0x100415e40`, and `Ready` branches to `0x100416500`. The normal ready branch decides Put versus Set there (`0x100416500–50c`). The lexical uninitialized check before this sequence remains at `0x100415de8–e08`.

Current's `direct_write_class` has been **inlined** into `run`; it does not add an outlined call. The canonical local arm still checks uninitialized before classification (`0x10041732c–350`), then computes the Put/Set `keep` predicate at `0x100417354–360` (`cmp`, `mov`, `ccmp`, `cset`). It reads the local again at `0x100417364–384`, checks the direct tag, and distinguishes Int/Float with `sub/cmp/b.hs` at `0x100417390–398`. Only Int/Float can reach the new `store_proven_number_operand` call at `0x1004173b4`. An Object instead branches to the same readiness helper at `0x1004174a4`. A `Ready` result branches to `0x10041790c`, where Put/Set is tested **again** at `0x10041790c–918` before the canonical `peek/pop → replace_local → release_displaced` work. Thus the early four-instruction `keep` computation is redundant on this Object route, and the three-instruction Number exclusion is new work there. The argument Put/Set source has the same early-`keep` pattern; this audit did not separately map its Object machine path.

The release helper and readiness check already existed in Parent. Current does not run `store_proven_number_operand` on Object values and does not reuse release readiness across instructions. The changed path includes more branches/dependencies, but that static fact does **not** establish branch-miss counts, cache behavior, or the cause of a cycles/wall regression. The two `run` functions were compiled from many other changes and have different layouts. The completed [fixed-work analysis](analysis-fixed-closure/README.md) finds current versus Parent `object_move` instructions −1.138%, cycles +7.253%, and wall +7.392%; same-binary current A/A spans are 0.064%, 0.910%, and 7.528% respectively. Five-engine default rotation is a cumulative screen, not pairwise ABBA. This corroborates a cycle regression signal, **not its code-level cause**; a narrow paired retest remains necessary.

## Clean narrow source candidate

`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/object-write-source` is an independent clean worktree on branch `codex/plan-closure-object-write`, commit `2818380b3f24768ba3fb702c8755e0c3b0243970`, based on `4ef28f74`. Only `src/engine/vm/run.rs` changes (blob `cfef174f...` → `4ee04bb3cc5b0965f28fd77402cb35d676fec3ee`). It moves the `keep` evaluation into each branch that consumes it for local and argument writes. Classification, Number operand authentication, owner handling, exception order, and PC logic are unchanged at source level. `git diff --check` passed and the worktree is clean.

## Candidate release machine code, before timing

The separately built candidate is `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/build-object-write/release/qjs`, SHA-256 `f00ae1f3403e8e366c1856470af2bbfc82ddb771854df01a349087cb22ba10ac`. Its `qjs.build.json` SHA-256 is `abfc25b040b51c74b4f9349f9568306ed07e8c3ae4a6abc74486281e65ed9535`. The receipt records source commit `2818380b3f24768ba3fb702c8755e0c3b0243970`, tree `2c2020d876736da78b53d2ca4a8adcdbf298bae3`, clean status, plain `stack-vm`, and Rust 1.96.0. It is distinct from the previously measured `current` binary.

```sh
CANDIDATE=/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/build-object-write/release/qjs
shasum -a 256 "$CURRENT" "$CANDIDATE"
llvm-nm -an "$CANDIDATE" | rg 'run3run|RunSlots5local|slot_value_release_readiness_jsvalue|store_proven_number_operand'
llvm-objdump --disassemble-symbols="$RUN_SYMBOL" "$CANDIDATE" | rg -n -C 10 'RunSlots5local|slot_value_release_readiness_jsvalue|store_proven_number_operand'
```

At the canonical local arm, candidate's TDZ check still ends at `0x100417350`. The next instruction, `0x100417354`, now starts the second `RunSlots::local` setup; current's early `keep` calculation occupied `0x100417354–360`. Candidate tests Int/Float at `0x100417380–388` and branches an Object directly to release readiness at `0x1004174a0`. Only the Number branch computes `keep` at `0x10041738c–398`, then calls `store_proven_number_operand` at `0x1004173b0`. For an Object receiving `Ready`, `0x100417908–914` is now its **single** Put/Set selection before the existing `peek/pop → replace_local → release_displaced` sequence. There is no added Object helper call or spill in this inspected route. The entire `run` disassembly has 5,197 instructions in current and 5,196 in candidate: the important change is moving four instructions off the Object route, rather than a four-instruction shrink of the whole function.

This confirms the intended codegen mechanism. The subsequent narrow 8-per-side ABBA gate found `object_move` current/Parent instructions −1.090%, cycles +7.463%; first-candidate/current instructions −0.275%, cycles +1.154%. Moving `keep` therefore did not repair the cycle regression.

## Explicit Number/Ready split

The same independent worktree then committed `53bbc5e50f286a989ff8d6ce2317a22cf55c23ea` (tree `a915f87dbe8622a93be2992623ca4d160cf1f735`). Its separately built plain-release CLI is `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/build-object-split/release/qjs`, SHA-256 `b9a037f1355236c21dd5d57e9e4229864568251ff56ed816612f8cd2595c5688`; receipt SHA-256 `6b226ded8e3cdef42544ac3fa0c08ae899d600b3d9550ce19f6573a46a96c5e7`. The receipt records clean source and Rust 1.96.0. In source, the Number and Ready arms are explicit; Ready directly copies/pops, replaces, and releases the displaced value.

In this binary, the local arm's Object exclusion precedes its readiness helper at `0x100417564`; readiness result `Ready` branches from `0x100417578` to the normal Put/Set copy/pop route. The local Ready replacement at `0x1004187b0` is followed by `release_displaced` at `0x1004187f0`. There is no post-replacement Number/Ready classification on that route. The corresponding argument route also uses `replace_parameter` followed by release rather than a post-write Number classification. The whole `run` disassembly contains 5,340 instructions, versus 5,196 in the first candidate; this total reflects code layout and other generated blocks, and is not a path cost.

The subsequent narrow Object split/current gate found `object_move` instructions −0.434% and cycles −0.287%; local instructions −0.196%, cycles +1.177%; arg instructions −0.003%, cycles +1.536%; and owner fallback instructions −0.919%, cycles −0.638%. The split removed its intended source and machine-code decision but did not repair the separately confirmed current/Parent +7.463% Object cycle regression. Neither narrow candidate was integrated. These gates establish an engineering decision to withdraw the ordinary Number write specialization; they do not identify cache or branch-prediction causality.
