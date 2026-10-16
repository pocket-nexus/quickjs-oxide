# New core, phase 1: results

This page records the acceptance measurements of [phase 1](new-core-phase1.md)
of the new core (#122): the value representation, the heap and the collector in
the `oxide-core` crate ([`core/`](../../core/README.md)). It is the exit record
the plan asks for: the M5 table and the M6 result, plus the assembly and size
records of M1, M3 and M4 and the decisions they settle.

## Identity

| Item | Value |
| --- | --- |
| Source | `3e15dd88` (heap code as measured) |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2 |
| Toolchain | rustc 1.99.0 (pinned by `rust-toolchain.toml`), release profile (`lto = "fat"`, `codegen-units = 1`) |
| New core | `core/examples/workloads.rs`, built with `cargo build --release -p oxide-core --example workloads` |
| Current core | `target/release/qjs` from the same tree (`cargo build --release -p quickjs-oxide-cli`) |
| Counter | `perf stat -e instructions:u` (perf 7.2.10) |
| Driver | [`probes/new-core/phase1.py`](probes/new-core/phase1.py) `--cpu 2 --repeat 5`; new core at N = 10⁶ and 2·10⁶, current core at N = 2·10⁵ and 4·10⁵ |

Every number below is a per-iteration slope, (I(2N) − I(N)) / N, so setup
cancels. All five repeats of every slope agreed to the instruction; the medians
are reported. Each engine's empty loop is subtracted: 7 instructions for the
Rust loop, 507 for the current core's `for` loop.

## M1: `Value`

`Value(u64)`, NaN-boxed with doubles favored, 8 bytes (`size_of::<[Value; 4]>()
== 32` is a compile-time assertion). NaN is canonicalized to
`0x7FF8_0000_0000_0000` on construction. Release assembly
([`core/examples/value_asm.rs`](../../core/examples/value_asm.rs)):

- `kind()`: 14 instructions, no branch: one shift, a saturating subtract, a
  compare-and-select for the special tag, and one table load.
- `as_object()`: shift, compare, `sete`, and the payload move; no branch.

```text
kind:       mov rax,rdi; shr rax,0x30; and edi,0x3; add edi,0x6; xor ecx,ecx; mov edx,eax;
            sub edx,0xfff8; cmovb edx,ecx; cmp rax,0xfffa; cmovne edi,ecx; add edi,edx;
            and edi,0xf; lea rax,[table]; movzx eax,BYTE PTR [rdi+rax]; ret
as_object:  mov rcx,rdi; shr rcx,0x30; xor eax,eax; cmp ecx,0xfffb; sete al; mov edx,edi; ret
```

## M3: the property-hit path

Release assembly of [`core/examples/property_asm.rs`](../../core/examples/property_asm.rs).
Release builds carry no liveness check.

| Path | Instructions on the hit path | Content |
| --- | ---: | --- |
| `get_slot(object, slot)` | 8 (+2 bounds-check branches) | header load, `slots_start + slot`, value load |
| inline-cache hit (shape compare, then load) | 12 (+2 bounds-check branches) | the load above behind one 32-bit shape compare |
| uncached own lookup, ≤ 8 properties | ≈ 10 per transition-chain step, then the load | the parent walk of `shape_lookup` |

The `ObjectHeader` is 20 bytes: shape, two range starts, payload, two range
exponents, kind and flags.

## M4: strings and the atom table

[`core/examples/atom_census.rs`](../../core/examples/atom_census.rs) interns
every identifier name and string-literal body of the pinned Test262 checkout
(`5c8206929d81b2d3d727ca6aac56c18358c8d790`, 53,393 test files and 41 harness
files, 3.27 million tokens), a superset of the property keys and names a run of
those files interns. Array-index keys below 2³¹ never enter the table.

| Scope | Atoms | Content bytes | Total (records, contents, table) |
| --- | ---: | ---: | ---: |
| One runtime running every file | 233,896 | 3.41 MB | 16.1 MiB |
| Largest single test, with the harness prelude (`staging/sm/String/string-upper-lower-mapping.js`) | 65,688 | 0.40 MB | 3.9 MiB |
| Median test, with the prelude | 111 | | |
| 99th-percentile test, with the prelude | 194 | | |

A table entry is 8 bytes (hash → atom index, collisions in a side list); the
table holds no second copy of any string.

**Decision (strong or weak atoms).** Strong atoms stay. With one runtime per
test, the 99th-percentile test interns 194 atoms and the worst 65,688 (3.9
MiB); even one runtime running the whole suite stays at 16 MiB. A weak table
(rebuilt after sweep) becomes worth its cost only when a long-lived runtime
interns keys from data, such as `JSON.parse` of unique keys or computed
property names, which arrives with the builtins in phase 4. That is the point
to revisit it.

## M5: micro-workloads

| Workload | New core, instr/iter | Current core, instr/iter | Ratio | Current-core probe |
| --- | ---: | ---: | ---: | --- |
| allocation only: new object, 4 reserved slots, no property | 238 | | | |
| constructor-style: new object (no reserved slots), 4 sequential property adds | 782 | 6,869 | 8.8× | `function P(x){ this.a=x; this.b=x; this.c=x; this.d=x; }`, `s=new P(i)` |
| literal-style: reserve 4 slots, then 4 property adds | 594 | 3,602 | 6.1× | `s={a:i,b:i,c:i,d:i}` |
| closures: one `Scope` plus one function object holding it | 713 | 18,374 | 25.8× | `let j=i; s=()=>j;` |
| prototype-chain read, depth 3, uncached | 222 | 687 | 3.1× | `o.x` with `x` three prototypes up (`Object.create`) |
| prototype-chain read, depth 3, IC check (shape compare, validity cell, holder slot) | 32 | 687 | 21× | same |
| churn: object or 1–64-element array into a 4,096-entry ring | 591 | | | none |

What the columns contain:

- The current core's numbers include the bytecode that the probe itself runs
  besides the loop: the constructor call and return, `this` handling, the
  closure instantiation. The new core runs no bytecode in phase 1, so its
  numbers are heap operations only. The ratio is an upper bound on what the
  heap alone buys; the share of the current core's count that is interpreter
  overhead is phase 2's to measure.
- The new-core numbers include the amortized collector at the default trigger
  (4 MiB minimum, growth factor 2). With the trigger out of reach
  (`--min-heap-mib 8000`) the allocation workloads cost 0–4% *more*
  instructions and churn 7% fewer: reusing a free-list slot is cheaper than
  growing a vector, which roughly pays for the collections.
- The closures counterpart is an arrow function because it has no `prototype`
  object, like the Rust workload. A `function` expression costs the current
  core 22,614 instructions per iteration.
- The Callgrind-based allocation ledger (`probes/allocation/ledger.py`) was not
  used: Valgrind is not installed on the reference machine. The comparison is
  per probe, not per ledger phase.

**Collector load** (10⁷ iterations, default trigger):

| Workload | Allocated | Collections | Total pause | p50 / p99 / max pause | Throughput |
| --- | ---: | ---: | ---: | --- | ---: |
| constructor-style | 520 MB | 123 | 43 ms | 339 / 451 / 585 µs | 1.55 GB/s |
| literal-style | 520 MB | 123 | 42 ms | 332 / 386 / 570 µs | 1.69 GB/s |
| closures | 640 MB | 152 | 66 ms | 431 / 475 / 874 µs | 1.96 GB/s |
| churn | 1,987 MB | 473 | 167 ms | 312 / 789 / 1,024 µs | 4.78 GB/s |

Throughput is bytes allocated over wall time, mutator included.

**Fragmentation.** The value heap does not compact. Over churn runs of 10⁶,
10⁷ and 3·10⁷ iterations the value heap ends at 580,569, 583,495 and 585,649
slots and peak RSS stays at 7.7–7.8 MiB: the size classes absorb the churn
without growth. After a final collection, 16% of those slots are live; the rest
is the space the trigger lets garbage occupy between collections, held in free
lists and reused. Under the cap (M6, a 12–16 MB live set) the free share after
each collection is 10–27%. Power-of-two classes also round each range up: an
array of 33 elements occupies 64 slots.

**Decision (value heap or one `Vec<Value>` per object).** The value heap
stays: its size is flat across a 30× longer run and its free share is bounded
by the trigger, not by fragmentation.

**Decision (eager or lazy sweep).** Eager sweep stays. At the default trigger
pauses are below 1.1 ms; with a 12–16 MB live set under the cap they are 4.5
ms at the median and 7 ms at the worst (M6). Those pauses grow with the live
set, which every collection marks in full; lazy sweeping would move only the
sweep share out of the pause. Incremental marking is not in scope.

## M6: churn under a 32 MB cap

`workloads churn N --ring 65536 --min-heap-mib 64 --cap-mib 32`: a ring of
65,536 objects and arrays (a 12–16 MB live set), the trigger set above the cap
so that the cap drives every collection.

| Iterations | Collections | Total pause | p50 / p99 / max pause | Heap reserved | Peak RSS | `OutOfMemory` |
| ---: | ---: | ---: | --- | ---: | ---: | --- |
| 10⁶ | 42 | 0.17 s | 4.3 / 4.8 / 5.3 ms | 18.8 MiB | 21.0 MiB | no |
| 3·10⁶ | 127 | 0.55 s | 4.4 / 5.4 / 5.8 ms | 18.8 MiB | 21.0 MiB | no |
| 10⁷ | 441 | 1.99 s | 4.5 / 5.6 / 5.8 ms | 18.8 MiB | 21.0 MiB | no |
| 3·10⁷ | 1,327 | 6.04 s | 4.5 / 5.8 / 7.1 ms | 18.8 MiB | 21.0 MiB | no |

RSS is stable and no allocation fails. The run spends about 77% of its time
collecting, because the heap stops at 18.8 MiB: the next doubling of the value
vector (16 to 32 MiB) would cross the cap, so the cap check collects instead.
Without the cap the same run reserves 142 MiB and peaks at 83 MiB RSS.
Letting the last growth step stop at the cap instead of doubling would let the
heap use the whole cap; that is a refinement for when a real embedder runs
against a cap.

## Stress mode

`cargo test -p oxide-core --features gc-stress` runs the whole suite with a
collection before every allocating call, poisoned and quarantined freed slots,
liveness assertions on every access and a heap verification after every
collection; CI runs it on every change. During this phase it caught two
rooting errors deterministically: one in a test (symbols held in locals across
allocations) and one in the heap (a one-entry root-shape cache that outlived a
collection). The quarantine exists because a first version without it missed a
stale index: the freed slot was handed to the very next allocation, so the
stale access hit a live record.

## Exit

Phase 1's exit condition is met: the table above and the M6 result are
recorded. They support starting phase 2 on the #122 design; the go/no-go gate
of #122 is in phase 3.
