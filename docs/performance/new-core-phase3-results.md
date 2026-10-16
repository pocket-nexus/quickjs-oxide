# New core, phase 3: results

This page records the measurements of [phase 3](new-core-phase3.md) of the new
core (#122) as its milestones land: the inline caches, the language surface
and builtins of the V8 v7 slice, and the exit tables of section 9 with the
go/no-go decision.

## Method

| Item | Value |
| --- | --- |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2 (the [baseline](baseline.md)'s reference machine) |
| Toolchain | rustc 1.99.0 (pinned by `rust-toolchain.toml`), release profile (`lto = "fat"`, `codegen-units = 1`) |
| Binaries | `cargo build --release -p quickjs-oxide-cli --bin oxide-run`, from the milestone's branch and from phase 2's last engine commit (`0eb02dd7`) built the same way |
| Slopes | [`probes/new-core/phase2.py`](probes/new-core/phase2.py) `--engine phase2=… --engine m…=… --cpu 2 --repeat 3`: (I(2N) − I(N)) / N of `instructions:u`, medians, N = 2·10⁵ |
| Loop size | `nm --size-sort` of the `oxide-run` binary, symbol `oxide_core::vm::interp::run` |
| Hit paths | `perf record` on the probe, `perf annotate` of `vm::interp::run`, instructions counted on the hit path from the arm's first instruction to its jump back to dispatch |

## M1: the `GetField` cache

Instructions per iteration (slopes), phase 2's binary and M1's measured
together; QuickJS from the [phase 2 exit tables](new-core-phase2-results.md#m6-exit-tables):

| Probe | QuickJS | Phase 2 | M1 | Phase 2 / M1 |
| --- | ---: | ---: | ---: | ---: |
| `prop_read` | 201 | 295 | 218 | 1.35 |
| depth-3 prototype read (`proto_read_3`) | 243 | 683 | 394 | 1.73 |
| `empty_loop` (unchanged paths) | 90 | 109 | 108 | |
| `call0` (unchanged paths) | 388 | 691 | 696 | |

- **`prop_read`** reads an own property of a local object: the uncached
  shape walk became a monomorphic hit, 8% above QuickJS.
- **`proto_read_3`** reads `o.x` three prototypes up, where `o` is a global:
  the prototype walk became a cell-guarded hit, and the remaining gap to
  QuickJS is the uncached `GetGlobal o` per iteration, which M2's global
  cache replaces.

**The hit path.** The own-property hit is 47 instructions from the arm's
first instruction to the jump back to dispatch, of which:

- 7 spill and reload loop state around the arm (`ic_base` and the runtime
  pointer live on the stack);
- 9 compute the entry index (`ic_base + c`), bounds-check it against the
  store and load the entry's four words;
- 19 are the cache test and the load: the receiver's tag, the object
  arena's bounds check, the shape compare, the own-hit test, the slot
  address and the value heap's bounds check. Phase 1 M3 measured 12 for a
  shape compare and a load without the bounds checks of safe indexing, which
  account for the difference;
- the rest write the destination register and continue.

A prototype hit adds the cell's bounds check, its `valid` byte and the
holder's header load; a string-receiver hit takes the same path after the
string tag test.

**The loop.** `vm::interp::run` is 14,246 bytes against the 16 KB gate
(12,684 at the end of phase 2). The arm reads the heap's arenas as fields,
disjoint from the register file, so a hit keeps the register window instead
of retaking it; the first build, which retook it, was 9 instructions slower
per hit.

**Correctness.** `core/tests/ic.rs` covers a hit after fill; a miss and
refill after the receiver's shape changes; a miss after a property is added
to, deleted from or reflagged on a prototype of the chain, and after
`set_prototype` on the chain and on the receiver; dictionary receivers, the
global object and complete misses left uncached; an array's `length` never
cached as a slot; string receivers through `%String.prototype%` and
`%Object.prototype%`, with invalidation; and a shape index recycled by the
sweep not hitting a stale entry (with pruning disabled the test fails). The
stress verifier checks after every collection that every filled entry names
live records. All of it passes with and without `gc-stress`, and the
two-core corpus and `check-new-core-stress.py` are unchanged.
