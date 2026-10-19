# The memory plan

This page is the plan for issue [#158](https://github.com/pocket-nexus/quickjs-oxide/issues/158) ("Peak memory is 2× QuickJS", acceptance: peak RSS within 1.3× of QuickJS over the 37 fixed-work programs, geometric mean, with time within 2% of today's). It is written for whoever implements an item: it records the measurements the plan rests on, fixes the decisions that are made, and gives each item an acceptance check that can be run without judgment calls. The items build on the heap, the collector and the trigger described in [Heap](heap.md), the compiler described in [Architecture](architecture.md), and the measurement method of [Measurement method](performance/measurement.md). [Planned work](planned.md) is the plan for the heap cap and the embedding targets (#122); item C of this page changes the trigger that plan builds on, and section 7 says how the two plans fit.

Decisions the maintainer made on 2026-10-19: the minimum heap becomes 1 MiB; items A, C and D and the memory table land together as **one pull request from one branch**, built up as the commits of section 7; the acceptance is the geometric mean of 1.3 alone, with no per-program gate; item B (the compile peak) is carried by the pull-request sequence of [#154](https://github.com/pocket-nexus/quickjs-oxide/issues/154), with the memory acceptance stated here.

## 0. Scope

**In scope.** The process floor (item A); the memory acceptance of the compiler work (item B); the collection trigger, the arenas' growth, the free lists and the value heap's free space (item C); the representation of arrays and of concatenation chains (item D); the memory table that measures each item (section 8).

**Outside this plan.** Code units compiled at run time ([#156](https://github.com/pocket-nexus/quickjs-oxide/issues/156), Code-Load); the growth factor (section 6 says why it stays 2); the generational collector of [Planned work](planned.md#6-the-generational-collector-optional); any change to the loop-size gate or the instruction gate of [Execution principles](performance/principles.md).

**Fixed constraints.** Growth stays 2. No V8 v7 program retires more than 2% more instructions than the base branch's binary, and the 37 programs' time stays within 2% of today's (medians, `fixed.py --order abba-baab`). `run_at` stays under the loop-size gate. The heap stays non-moving, with indices that never change.

## 1. Where the memory goes

Measured on 2026-10-19 on the reference machine (Ryzen 7 7840HS, Linux 7.2), release `qjs` at 689296ae against QuickJS 2026-06-04, the 37 fixed-work bundles, one run each, peak RSS from `wait4`'s `ru_maxrss`. The issue's model has three parts (arena reservation, the startup floor, representation); the measurements give them different weights and add one the issue does not have, the compiler's transient peak.

### 1.1 The process floor is binary text, not the realm

An empty script costs 5.0 MiB against QuickJS's 3.2 MiB. The realm's heap is 198 KiB (609 objects, 618 shapes, 520 strings, 518 atoms); the rest is the process:

| Mapping | `qjs` | QuickJS |
| --- | ---: | ---: |
| the binary's file pages | 2,460 KiB (of 3,453 KiB on disk; `.text` is 2.9 MB) | 920 KiB (of 1.1 MB) |
| `libc.so.6` | 1,468 KiB | 1,276 KiB |
| `libm.so.6`, `ld-linux`, `ld.so.cache`, `libgcc_s` | 964 KiB | 884 KiB |
| anonymous (heap, thread stack, code store) | 480 KiB | 216 KiB |

The kernel maps file-backed pages in 64 KiB fault-around windows: touching one byte of a mapped binary makes 64 KiB resident (verified with a one-byte probe; transparent huge pages and `MADV_RANDOM` change nothing). Realm initialization plus the compile of a small script executes about 640 functions, 665 KiB of code, spread over 30 such windows (1.9 MiB). Linking the same tree with an lld symbol-ordering file that puts those functions first makes them span 12 windows: the empty script drops to 3.8 MiB and Richards from 7.6 to 6.1 MiB, with build time and the instruction count unchanged. Item A is that experiment made permanent.

### 1.2 The compiler's transient peak

The compiler holds the whole script's intermediate forms at once (the linear IR of every function, the stack-code intermediate, the register code) until lowering completes; `massif` puts the compile-phase peak at 22–32 bytes per source byte, and that peak is the program's peak for every program whose heap is small:

| Program | Source | Compile-phase peak | Run-phase heap peak | Peak RSS, `qjs` / QuickJS |
| --- | ---: | ---: | ---: | ---: |
| Mandelbrot (AWFY bundle) | 147 KB | 3.3 MB | 0.4 MiB | 8.5 / 4.4 MiB |
| Bounce | 147 KB | 3.3 MB | 9.4 MB | 13.0 / 4.3 MiB |
| Box2D | 530 KB | about 12 MB | 9.1 MiB reserved | 22.0 / 8.8 MiB |
| PdfJS | 1,480 KB | 39.7 MB | 50.7 MB | 73 / 24 MiB |
| Mandreel | 2,528 KB | 124 MB, of which the IR's `ops` vector 80 MB | 1.4 MiB reserved | 117 / 58 MiB |

QuickJS's heap during the compile of PdfJS stays under 7 MB. The retained code is a second, smaller cost: each function keeps its own copy of its source text (2.1× the file for Bounce, 2.6 MiB for PdfJS), instructions are 8 bytes and position entries 12.

### 1.3 The trigger and the arenas

The trigger collects when the bytes allocated since the last collection reach `max(min_heap, 2 × live)`, so the bytes in use peak at `live + 2 × live`. Reserved memory sits above that:

- the record vectors and the value heap grow by doubling (Bounce at its peak: a 4 MiB value-heap capacity with 2.8 MiB touched);
- every free slot costs 4 bytes in a `Vec<u32>` free list, pushed by the sweep (1 MiB of Bounce's 9.4 MB peak is two free lists growing during a collection);
- a free value range serves only its own size class, so a program whose allocation mix changes grows the value heap while free ranges of other classes sit idle (Earley-Boyer: 3 MiB live, 9 MiB in use at the peak, 16.4 MiB reserved, 522,260 free slots at exit);
- nothing shrinks after a collection.

For the programs with small live sets the 4 MiB minimum is what sets the heap's size: Richards (Octane) allocates 2.9 MiB in total, never collects, and ends with every record ever allocated still in the arenas. Halving the minimum to 1 MiB, same binary, moves no program's time outside ±2% (medians of 3, interleaved) and takes 25–30% off the small programs' peak RSS even before the trigger counts reserved bytes.

### 1.4 Representation and the growth factor

Splay's live set after a collection is 77 MiB; QuickJS's process on the same program is 147 MiB, almost all of it live data, so the records are already about half QuickJS's size per object. Splay's peak is the trigger's: `3 × 77 MiB` plus reserved slack is 268 MiB. Two representation costs remain: a ten-element array literal takes a 16-slot range (the value heap's classes are powers of two), and a concatenation chain such as `'String for key ' + tag + ' in leaf node'` allocates two flat intermediate strings per leaf that are garbage until the next collection, which QuickJS frees at once. The memory report's per-arena counts at exit include unswept garbage, so its `live` column is not the live set; the collector's `live` after the last collection is.

The growth factor is the one lever this plan does not pull: Splay spends a third of its time in ten collections of about 100 ms, and every program with a large live set pays the same way, so growth 1.5 would cost Splay about 11% and Earley-Boyer and Havlak about 4% in time, outside the 2% gate (section 6).

### 1.5 The baseline table

The 37 programs on 2026-10-19 (one run; `collections`, `live` and `reserved` from `--gc-stats` at exit, `live` being the bytes after the last collection):

| Program | `qjs` KiB | QuickJS KiB | Ratio | Collections | Live KiB | Reserved KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Bounce | 13,024 | 4,304 | 3.03 | 2 | 290 | 8,029 |
| CD | 13,640 | 6,264 | 2.18 | 45 | 478 | 8,094 |
| DeltaBlue (AWFY) | 30,324 | 19,736 | 1.54 | 9 | 6,430 | 28,238 |
| Havlak | 40,748 | 33,604 | 1.21 | 48 | 9,337 | 50,886 |
| Json | 13,076 | 4,996 | 2.62 | 11 | 543 | 7,677 |
| List | 11,064 | 4,336 | 2.55 | 0 | 0 | 3,657 |
| Mandelbrot | 8,472 | 4,364 | 1.94 | 0 | 0 | 417 |
| NBody | 13,464 | 4,544 | 2.96 | 7 | 291 | 7,508 |
| Permute | 8,476 | 4,236 | 2.00 | 0 | 0 | 522 |
| Queens | 12,552 | 4,280 | 2.93 | 1 | 291 | 6,217 |
| Richards (AWFY) | 8,692 | 4,592 | 1.89 | 0 | 0 | 1,007 |
| Sieve | 12,416 | 4,204 | 2.95 | 47 | 290 | 4,450 |
| Storage | 13,124 | 5,516 | 2.38 | 94 | 601 | 6,344 |
| Towers | 8,812 | 4,496 | 1.96 | 0 | 0 | 1,149 |
| crypto (V8) | 11,968 | 3,816 | 3.14 | 4 | 181 | 7,525 |
| deltablue (V8) | 10,820 | 3,820 | 2.83 | 16 | 172 | 6,823 |
| earley-boyer (V8) | 24,460 | 16,576 | 1.48 | 452 | 3,094 | 16,367 |
| navier-stokes (V8) | 9,320 | 5,768 | 1.62 | 0 | 0 | 5,816 |
| raytrace (V8) | 11,180 | 3,660 | 3.05 | 186 | 160 | 6,323 |
| regexp (V8) | 17,628 | 6,868 | 2.57 | 103 | 3,043 | 9,117 |
| richards (V8) | 7,548 | 3,708 | 2.04 | 0 | 0 | 2,910 |
| splay (V8) | 270,620 | 147,452 | 1.84 | 11 | 76,829 | 285,728 |
| box2d | 22,024 | 8,780 | 2.51 | 13 | 1,327 | 9,107 |
| code-load | 138,672 | 106,788 | 1.30 | 5 | 7,322 | 28,898 |
| crypto (Octane) | 12,320 | 3,844 | 3.20 | 4 | 185 | 7,489 |
| deltablue (Octane) | 10,720 | 3,780 | 2.84 | 16 | 173 | 6,819 |
| earley-boyer (Octane) | 25,016 | 16,948 | 1.48 | 483 | 2,833 | 16,367 |
| gameboy | 25,140 | 23,752 | 1.06 | 12 | 2,488 | 15,376 |
| mandreel | 116,984 | 58,248 | 2.01 | 4 | 1,045 | 1,406 |
| navier-stokes (Octane) | 9,760 | 5,504 | 1.77 | 0 | 0 | 5,858 |
| pdfjs | 73,120 | 23,952 | 3.05 | 30 | 5,770 | 34,150 |
| raytrace (Octane) | 11,472 | 3,772 | 3.04 | 188 | 163 | 6,323 |
| regexp (Octane) | 17,688 | 6,816 | 2.60 | 64 | 3,047 | 9,123 |
| richards (Octane) | 7,588 | 3,504 | 2.17 | 0 | 0 | 2,910 |
| splay (Octane) | 267,328 | 147,608 | 1.81 | 10 | 76,839 | 306,858 |
| typescript | 150,460 | 140,068 | 1.07 | 10 | 39,331 | 199,101 |
| zlib | 22,744 | 136,620 | 0.17 | 2 | 362 | 2,512 |

## 2. Item A: the process floor

**Mechanism.** The functions that run during realm initialization, the compile of a script and the common paths of the loop are placed first in `.text`, contiguously, so that the fault-around windows they touch are few and shared.

**What is built.**

- `scripts/release/gen-symbol-order.py`: runs `valgrind --tool=callgrind --demangle=no` on the release `qjs` over an empty script, one small suite program (Richards) and the compile of one mid-sized source (`--compile-only` of item B once it exists, `--disassemble` to `/dev/null` until then), takes every mangled name from the `fn=` and `cfn=` lines in first-seen order, keeps the names the binary defines, and writes `apps/cli/qjs.order`. The file is committed.
- `apps/cli/build.rs` emits `cargo:rustc-link-arg-bins=-Wl,--symbol-ordering-file=<CARGO_MANIFEST_DIR>/qjs.order` and `-Wl,--no-warn-symbol-ordering` on targets whose linker is lld (the toolchain's default on `x86_64-unknown-linux-gnu`), the equivalent `-order_file` form on macOS, and nothing elsewhere. A name the binary no longer defines is ignored by the linker, so a stale file degrades, never breaks.
- `scripts/checks/check-symbol-order.py`: fails `fast` CI when fewer than 80% of the file's names are defined by the release binary, with the regeneration command in the message.
- The web and native-adapter builds are unaffected (the argument is emitted for binaries of `quickjs-oxide-cli` only).

**Also measured in this item,** and taken only if the instruction gate holds: `panic = "abort"` in the release profile (removes landing pads and most of `.eh_frame`, about 200 KiB of file pages).

**Acceptance.** The empty script's peak RSS is at most 4.0 MiB; Richards (Octane) is at most 6.3 MiB; no V8 v7 program's instruction count moves by more than 2% (the order file changes code placement only); the check script passes on the committed file. Expected: −1.2 MiB on every program in the table.

## 3. Item B: the compiler's transient peak

This item is carried by the pull-request sequence of #154 (the handler-table rewrite, the de-duplicated analysis, the allocation cuts); this page adds its memory acceptance and the mechanism that meets it without the IR-to-register rewrite that #154 defers.

**Mechanism.** The peak becomes the largest single function's intermediate forms plus the finished code, instead of the whole script's:

- `lower_unlinked_tree` and `lower_tree` consume the tree function by function: a function's IR (`FunctionIr`, its `ops` vector) and its stack-code intermediate are dropped as soon as its register code is published, and the buffers the lowering reuses (`analyze`'s per-pc regions, `positions`, the emitted `Insn` vector) are kept across functions rather than reallocated;
- `SpannedIrOp` shrinks (the rare large payloads are boxed) so the per-function IR costs less per source byte;
- the lexer's lookahead buffer (`probe_token`, 1.5 MB on PdfJS) is bounded by the deepest re-scan the grammar needs, not by the function;
- the source text kept for `Function.prototype.toString` is one shared byte string per script with a range per unit, so the code store holds the file once (today 2.1× the file).

`qjs --compile-only FILE` is added with this item: it compiles and publishes without running, and is what both the throughput measurements of #154 and the compile-peak column of section 8 use.

**Acceptance.** On PdfJS, Mandreel and TypeScript the peak of `--compile-only` is at most 6 bytes per source byte above the code store's size reported by `--memory-report` (QuickJS compiles PdfJS's 1.48 MB in under 7 MB); the code store's `source` figure equals the file size; the compile throughput gates of #154 hold. Expected: PdfJS 73 → about 35 MiB, Mandreel 117 → about 45 MiB, Box2D 22 → about 14 MiB, each AWFY program −2 to −3 MiB.

## 4. Item C: the trigger, growth and the arenas

The sub-items below are commits of the pull request, in this order, each measurable on its own.

### 4.1 A budget on reserved bytes

After every collection the heap sets

```text
budget = max(min_heap, growth × live)
```

and the trigger's job is to keep `Heap::bytes()` (reserved: record storage, bitmaps, out-of-line bytes, the value heap) at or below `budget` plus one growth step. The fast path of `safepoint` keeps its single add-and-compare; what changes is the threshold it compares against and where a collection is requested:

- `threshold` after a collection is the bytes that can be allocated before reserved memory reaches the budget: the free bytes the arenas and the value heap hold (free slots at their record size, free ranges at eight bytes a slot) plus `budget − bytes()`. Allocation into free slots does not change reserved memory, so this is exact while free slots match the requests.
- A growth step (`Arena::push` past capacity, `ValueHeap::alloc_fresh` past capacity, both already `#[inline(never)]`) checks whether the step would take `bytes()` past the budget; if so it sets `threshold` to zero and takes the step, so the next safepoint collects. A collection is one allocation late at most, bounded by one growth step.
- A collection that still leaves `bytes() + step > budget` (the free space is of the wrong shape for the requests, or the live set grew) raises the budget to `bytes() + step` and continues; the heap grows by at most one step between collections, never by a doubling.
- Under a cap the budget is additionally clamped to `cap − step`, which is the rule of [Planned work 3.1](planned.md#31-the-trigger-under-a-cap) expressed on the same quantity.

`Need::nominal_bytes` stays as the fast path's charge; `GcState.allocated` stays as the count since the last collection; `GcPolicy::Manual` keeps an unbounded threshold. The memory report prints `budget` beside `threshold`.

### 4.2 Bounded growth steps

`Arena<T>` and `ValueHeap` grow by `step = clamp(capacity / 4, 64 KiB, 2 MiB)` in bytes, through `reserve_exact`, instead of doubling. Slack above the touched high-water mark is at most a quarter. The allocator extends large blocks in place (`mremap` under glibc), so a step copies only while the vector is below the allocator's mapping threshold.

### 4.3 Bitmap allocation in the arenas

The free list `free: Vec<u32>` of `Arena<T>` goes away. Allocation finds the lowest clear bit of the live bitmap at or after a cursor (`trailing_ones` on a word; the cursor is the lowest index freed by the last sweep or allocated since), so reuse stays lowest-index-first and deterministic, as the sweep's descending push makes it today. The sweep clears live bits and resets the cursor; it no longer pushes indices. `growth_bytes` counts free slots from the bitmap's population count, kept as a field. Under `gc-stress` the one-collection quarantine becomes a second bitmap, "freed by the last sweep", that allocation skips.

Saved: 4 bytes per free slot (20% of a full object arena, 1 MiB at Bounce's peak, 5.6 MB of Splay's), and the free-list growth during a collection.

### 4.4 The value heap: splitting, coalescing and the trailing run

- **Splitting.** A request for class `k` whose list is empty takes the smallest non-empty class `j > k` and splits that range into `2^(j−k)` ranges of class `k`, one returned and the rest pushed on list `k`. No alignment is required because ranges never need to find a buddy; splitting only is enough, because coalescing happens at the collection.
- **Coalescing at the collection.** The sweep records freed ranges in a free bitmap over slots (one bit per slot, 1/64 of the value heap). After the sweep one linear pass over that bitmap rebuilds every class list: each maximal run of free slots is cut greedily into power-of-two ranges from its start. Runs are cut without alignment, so a run of 6 becomes [4, 2]. The pass is linear in the value heap's capacity and runs once per collection beside the sweep, which already touches every arena.
- **The trailing run** (the free run that ends at the high-water mark) is not put on a list: the value heap truncates to its start, and 4.5 gives the memory back.
- The class lists themselves are `Vec<u32>` and stay (one entry per free range, not per slot); after coalescing they are as short as the free space allows.

`Heap::verify` under stress keeps checking that every range has exactly one owner, now including that no two free ranges overlap after a rebuild.

### 4.5 Shrinking after a collection

After the sweep, an arena whose trailing free run is more than a quarter of its capacity truncates its record vector to the last live index plus one and calls `shrink_to(len + step)`; the bitmaps follow. The value heap does the same after 4.4's rebuild. Shrinking returns memory through the allocator (`munmap` or trimming for large blocks), which is what peak RSS of a later phase sees. A program that fills and refills the same heap (the churn ring of `docs/performance/probes/slopes/heap_workloads.py`) never has a trailing run above a quarter, so it pays nothing. This is the shrinking of [Planned work 3.2](planned.md#32-growth-that-stops-at-the-cap-and-shrinking), made unconditional of the cap; the register file's shrink stays with that plan.

### 4.6 The minimum heap

`DEFAULT_MIN_HEAP` becomes 1 MiB. The places that state the default: `core/src/heap/gc.rs`, `RuntimeOptions` in `core/src/runtime.rs`, the `qjs` help text (`--min-heap-mib N`, default 1), [Heap](heap.md#the-trigger), [Architecture](architecture.md), and [Planned work](planned.md) (its fixed constraint and decision, updated with this page). With 4.1 the minimum is also the floor of the reserved budget, so a program with a small live set keeps about 1 MiB of heap plus one growth step.

### 4.7 Accounting

`Heap::bytes()` and the memory report gain what they do not count today: the rope table (`Heap::ropes`, 3.4 MB on PdfJS while concatenating), the inline-cache store (16 bytes an entry), the atom table's capacity and the class lists' capacity. The report prints the budget, and `fixed.py`'s peak RSS is reported beside it as [Planned work 2](planned.md#2-memory-accounting) asks.

### 4.8 Acceptance

- After every collection of every one of the 37 programs, `bytes() ≤ budget + step` (an assertion under `gc-stress`, and the report's `reserved / budget` column at most 1.25 at exit).
- Peak RSS: Richards (Octane) at most 5.6 MiB, Bounce at most 7.0 MiB, Earley-Boyer at most 18 MiB (all with item A landed).
- The churn ring's steady state reserves no more than today and shrinks nothing.
- Time within 2% on the 37 programs (`fixed.py --order abba-baab`), the instruction gate on the eight, `run_at` under the loop-size gate; `gc-stress` green on the corpus, the probes and the 37 programs.
- The lowest-index-first reuse order is unchanged: the corpora's outputs that print object or string identities replay byte for byte.

Expected: the small programs' heap goes from 3–8 MiB to about 1.2 MiB; Earley-Boyer's reserved from 16.4 to about 10 MiB; PdfJS's from 34 to about 18 MiB; Splay loses its slack only (about 30 MiB).

## 5. Item D: representation

Measured first, taken if the numbers hold:

- **Array ranges.** The value heap adds the classes `3 × 2^k` beside `2^k`, encoded in the header's `log` byte with one mantissa bit, so a ten-element literal takes 12 slots instead of 16 and a 1,024-element array growing by 1.5× lands on a class. Splay's leaves save 32 bytes each (8 MiB live, 24 MiB of peak under growth 2). The classes change `class_len`, `growth_bytes`, the free-list index and the elements' growth rule in `Heap::set_element`.
- **Concatenation chains.** `a + b + c` with string operands allocates an intermediate per `+`. The register lowering recognizes a chain of string concatenations and emits one `ConcatN` over its operands, allocating the result once; the runtime path copies the units of every operand into one flat string (or one rope when the result is at least `ROPE_MIN`). Splay's leaf strings go from three allocations to two (the tag's conversion and the result), which lowers the garbage allocated per leaf and so the collection count, not the live set.

**Acceptance.** Splay's live bytes after a collection at most 70 MiB and its collection count at most today's; the instruction gate; the corpora unchanged. Expected: Splay 268 → about 220 MiB (about 1.5× QuickJS; section 6 says why not lower).

## 6. The growth factor

The trigger collects when the bytes allocated since the last collection reach `growth × live`, so the heap in use peaks at `(1 + growth) × live`; QuickJS frees acyclic garbage at once and peaks near its live set. For a program whose live set is large, the ratio to QuickJS has a floor of `(1 + growth) × (live / live_QuickJS)` that no reservation policy lowers: with growth 2 and Splay's records at about half QuickJS's size, that floor is about 1.5. Lowering growth raises the collection count in proportion and every collection marks the whole live set (Splay: ten collections of about 100 ms, a third of its time), so growth 1.5 costs Splay about 11% and Earley-Boyer and Havlak about 4%, outside this plan's time gate. Growth stays 2; Splay, regexp and the two Earley-Boyer rows are the programs that stay above 1.3 individually, and the geometric mean is the acceptance. The structural answer for those programs is the generational collector of [Planned work 6](planned.md#6-the-generational-collector-optional), which collects the young garbage without marking the old live set.

## 7. The pull request, its commits and the plans' fit

Items A, C and D are one pull request on one branch. Its commits, in order, each leaving the tree green (`cargo test`, the corpora, `gc-stress` on the core's tests) and each with its own column in the memory table of section 8:

| Commit | Content | Acceptance | Estimate |
| --- | --- | --- | --- |
| 1. The baseline | the memory table's script and its baseline column, before any change (8) | section 8 | half a day |
| 2. The floor | the order file, its generator, the build-script link argument, the CI check (2) | section 2 | 1–2 days |
| 3. `panic = "abort"` | taken only if the instruction gate holds, otherwise the commit is dropped (2) | section 2 | half a day |
| 4. The budget trigger | 4.1, with the 1 MiB minimum of 4.6 and the accounting of 4.7 | 4.8's reserved-to-budget bound | 2 days |
| 5. Bounded steps and shrinking | 4.2 and 4.5 | the churn ring's steady state; the bound | 1 day |
| 6. Bitmap allocation | 4.3 | the reuse order replays; `gc-stress` green | 1–2 days |
| 7. The value heap | 4.4's splitting, coalescing and trailing run | `Heap::verify`'s owner check; Earley-Boyer's row | 1–2 days |
| 8. Array classes | the `3 × 2^k` classes of section 5, kept if Splay's row improves at the gate | section 5 | 1–2 days |
| 9. `ConcatN` | section 5's concatenation chains, kept if Splay's collection count drops at the gate | section 5 | 1–2 days |
| 10. The tables and the docs | the final memory table replayed on the branch's last binary; [Heap](heap.md), [Architecture](architecture.md), the `qjs` help text and [Planned work](planned.md) updated to what landed | section 8's exit | 1 day |

About 11–16 working days in all. The order puts the cheapest change that moves every row first, then the issue's own item, then the representation commits, which are read against the table the trigger commits produce and are dropped from the pull request rather than landed if their gate fails. Item B is carried by #154's sequence in parallel and is not part of this pull request; its `--compile-only` option, when it lands first, replaces the `--disassemble` stand-in of the order file's generator and of the compile-peak column.

**Fit with [Planned work](planned.md).** Its fixed constraint on the trigger (a 4 MiB minimum) and the matching decision are replaced by this page's 1 MiB minimum and the budget of 4.1; its 3.1 (the trigger under a cap) becomes the clamp of 4.1's budget; its 3.2's shrinking is delivered by 4.5 and its growth-to-the-cap by 4.2's steps clamped at the cap; its memory table (section 9) takes section 8's columns. The rest of that plan (dynamic code, images, snapshots, 32-bit, `no_std`, the generational collector) is unchanged; the snapshot writer of its 4.2 writes the free bitmap of 4.3 in place of a free list.

## 8. Measurement and exit

Every number is produced by [Measurement method](performance/measurement.md) on the reference machine, medians of 3 or more interleaved runs, every sample's stdout checked. Peak RSS is `ru_maxrss` of the `qjs` process alone (`wait4`), never of a wrapper that forks from a larger process.

**The memory table**, one row per program, is recorded in the pull request with one column group per commit of section 7 that changes memory (baseline, floor, trigger, value heap, representation) and replayed on the branch's final binary: peak RSS for `qjs` and QuickJS and their ratio; the compile-phase peak (`--compile-only`, or `massif` until it exists); collections, live after the last collection, reserved and budget at exit; the code store's size. The script that produces it (`scripts/benchmark/memory_table.py`: the 37 bundles, both engines, `ru_maxrss` through `wait4`, `--gc-stats` and `--memory-report` parsed, medians of 3) is the first commit, so every later commit's effect is one command away. The geometric mean of the ratio column over the 37 programs is the acceptance figure.

**Exit condition**, checked once on the pull request's final binary: the geometric mean of peak RSS over the 37 programs is at most 1.3× QuickJS's; the 37 programs' time is within 2% of the 689296ae binary's; no V8 v7 program retires more than 2% more instructions; `gc-stress` is green on the corpus, the probes and the 37 programs; the Test262 receipts replay. A commit whose own gate fails is dropped from the branch or fixed before the next one builds on it; failing the final gate is a finding that names the mechanism, recorded in the pull request, and the pull request does not land until the gate holds or the maintainer accepts the exception in writing there.

Expected rows after A, C and D (estimates from section 1's decomposition, to be replaced by measurements): the small programs at 1.3–1.5 (floor 3.8 MiB against 3.2, a 1.2 MiB heap against QuickJS's 0.5 MiB), Havlak, Gameboy, TypeScript, Earley-Boyer below or near 1.0, PdfJS and Mandreel near 1.0 with B, Splay about 1.5, regexp about 1.6, and a geometric mean of about 1.2.

## 9. Decisions

Made:

- the minimum heap is 1 MiB and growth stays 2 (4.6, 6);
- the trigger bounds reserved bytes by a budget, collecting at most once per growth step (4.1);
- arenas and the value heap grow in bounded steps and shrink their trailing free run after a collection (4.2, 4.5);
- arenas allocate from the live bitmap; the value heap splits from larger classes and coalesces at the collection (4.3, 4.4);
- items A, C and D and the memory table are one pull request from one branch, built as section 7's commits; the acceptance is the geometric mean alone; item B is carried by #154's sequence with section 3's acceptance;
- the order file is committed and checked, generated from callgrind, applied through the binary crate's build script (2).

Open, with the item that decides each:

- whether `panic = "abort"` ships: A, by the instruction gate and the floor;
- the step bounds (a quarter, 64 KiB, 2 MiB) and the shrink threshold (a quarter trailing): C, by the memory table and the churn ring;
- whether the `3 × 2^k` classes and `ConcatN` ship: D, each by its own measurement;
- whether the budget's one-step lateness needs a second check at the safepoint under a cap: C, by the capped-run table of [Planned work 3.5](planned.md#35-psp-class-controlled-workloads).

## 10. Risks

- **A collection storm from fragmentation.** If the free space never matches the requests, 4.1 could collect at every growth step. Mitigation: a collection that does not make room raises the budget by the step, so collections are at most one per step and the heap still grows; 4.4's splitting and coalescing keep the free space usable.
- **The cursor scan.** Bitmap allocation scans for a clear bit; a nearly full arena could scan many words. Mitigation: the cursor starts at the lowest freed index and words are tested 64 slots at a time, so the scan is amortized over the slots it passes; the probes' allocation slopes are in the gate.
- **Shrinking that thrashes.** Giving memory back and taking it again on every collection costs page faults. Mitigation: only the trailing run above a quarter of capacity is returned, and the churn ring's steady state is in the acceptance.
- **A stale order file.** Symbol names change with the code; a file that no longer matches orders nothing. Mitigation: the linker ignores unknown names, the CI check fails below 80% coverage, and the generator is one command.
- **Determinism of reuse.** Changing how free slots are found could change which index an allocation gets and so the output of programs that print identities. Mitigation: lowest-index-first is kept by construction, and the corpora's replay is in the acceptance.
