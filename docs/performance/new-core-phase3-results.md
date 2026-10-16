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

## M2: stores, globals, constructors, polymorphism, accessors

Instructions per iteration (slopes), the three binaries measured together:

| Probe | QuickJS | Phase 2 | M1 | M2 | Phase 2 / M2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `prop_write` | 163 | 220 | 224 | 169 | 1.30 |
| `call0` | 388 | 691 | 696 | 596 | 1.16 |
| `new P(i)`, two `this.x =` (`constructor`) | 1,878 | 2,041 | 2,064 | 1,177 | 1.73 |
| `{a:i,b:i}` (`literal`) | 1,298 | 904 | 915 | 905 | 1.00 |
| `prop_read` | 201 | 295 | 218 | 212 | 1.39 |
| depth-3 prototype read (`proto_read_3`) | 243 | 683 | 394 | 271 | 2.52 |
| `empty_loop` | 90 | 109 | 108 | 105 | |
| `tdz` | 13,612 | 3,819 | 3,694 | 3,496 | |

- **`prop_write`** is an in-place `SetField` hit, level with QuickJS.
- **`call0`** reads its callee with a cached `GetGlobal` (a generation
  compare and a load). The rest of the gap to QuickJS is the call itself.
- **`constructor`** is now 37% below QuickJS. The `New` site reads
  `prototype` from the constructor's cached slot, and both `this.x =`
  stores are transition hits into the slots the receiver reserved.
- **`literal`** has no cache site: `NewObject` and `DefineField` were
  already exact-sized in phase 2.
- **`proto_read_3`** gains the cached `GetGlobal o` and is 12% above
  QuickJS.
- `tdz` and the other probes run global lookups in their setup or error
  paths, and drop with the global cache.

**The loop.** `vm::interp::run` is 11,268 bytes, smaller than at the end of
phase 2 (12,684). The global lookup, which phase 2 inlined into
`GetGlobal`, is now an out-of-line miss path. The `SetField` and global
hits read the heap's arenas as fields, as `GetField` does.

**`ic-stats` on the corpus** (`oxide-run --ic-stats` on
`core/tests/corpus/caches.js`, a program written to miss) shows four
things:

- Every kind fills and hits.
- The polymorphic sites keep their rows, and a site that sees six shapes
  goes megamorphic and stops recording.
- Transitions on `{}` literals always miss at the full range, because a
  literal reserves exactly its `DefineField` count.
- A string's `length` goes through the miss path every time.

The last two are M5 candidates, to be weighed against the V8 v7 profiles.

**Correctness.**

- `src/engine/compiler/register/ic_tests.rs` uses the debug-build hit
  counters to check:
  - transition and in-place `SetField` hits;
  - a full-range transition miss that still completes and keeps its entry;
  - a setter added to `%Object.prototype%` after a transition fill, which
    then runs;
  - the global generation bump after `delete globalThis.g`, a refill after
    re-adding `g`, and cached global function calls;
  - a `New` site that learns four slots from a constructor whose helper
    adds three properties to `this`; the next construction hits and its
    stores are all transition hits;
  - a polymorphic row, its hits, and the megamorphic mark.
- Removing the cell test from the `SetField` hit, or the generation
  compare from `GetGlobal`, fails these tests.
- `core/tests/shapes.rs` checks the two-slot accessor layout in the
  transition tree and in dictionary mode.
- The corpus gains `accessors.js` (getters and setters in literals,
  inherited accessors, a strict write to a getter-only property, a throwing
  getter) and `caches.js`, both printing identically on both cores with and
  without stress.

## M3: the language surface

**The census.** `lowering_census` over the baseline's suite build
(`suite_matrix.py build` at the pinned sources, rebuilt for this record)
lowers every function of the eight V8 v7 programs:

| Program | Functions lowered | Declined before M3 |
| --- | ---: | --- |
| richards | 63/63 | rest parameters (the `print` shim) |
| deltablue | 99/99 | rest parameters |
| crypto | 157/157 | rest parameters |
| raytrace | 86/86 | `arguments`, for-in, rest parameters |
| earley-boyer | 435/435 | `arguments` ×34, eval, for-in, rest parameters |
| regexp | 43/43 | RegExp literals ×12, a function over 256 registers, rest parameters |
| splay | 43/43 | rest parameters |
| navier-stokes | 59/59 | rest parameters |

All fifteen Octane programs lower completely as well. Of the AWFY programs,
classes still decline (phase 4).

The programs do not yet run to completion: the builtins they call first
(`Math`, `String`, `Array` methods) are M4's.

**Corpus.** Each item of section 5.5 has a program in `core/tests/corpus`,
printing identically on both cores with and without stress:

- `arguments.js`: mapped and unmapped `arguments`, with:
  - writes in both directions;
  - `length`, extras, missing parameters;
  - `delete`, captured parameters;
  - `callee`, nested functions.
- `rest_spread.js`: rest parameters, spread in calls, `new` and array
  literals, spread of `arguments`, holes.
- `apply_call.js`: `call` and `apply` with arrays, `arguments` objects,
  array-likes, `null` and `undefined`, sloppy and strict `this`, chained
  constructors, `call.call`, native callees, and 300-element argument lists.
- `for_in.js`: arrays, sparse arrays, plain objects, prototype chains with
  shadowing, integer keys, deleted and added keys, dictionary objects,
  `arguments`, `break`.
- `regexp_literals.js`: identity per evaluation, `lastIndex`, and the
  string form.
- `new_target.js`: `new.target`, including through an arrow function.
- `accessors.js` (M2): getters and setters in literals.
- `wide.js`: a function with 250 locals plus closures, `let` bindings in
  the TDZ, and per-iteration bindings, past the register window.

**Stress.** Running the new programs under stress found one phase 2 bug.
A caller's dead temporaries above a smaller callee's frame were not roots
during the call, so a collection could free what they named, and they
became roots again on return. A resuming frame now clears them when a
collection ran since. The new programs also found two M3 bugs before
commit: the unmapped `arguments` payload and the tracing of the mapped
scope.

**The loop and the probes.** `vm::interp::run` is 13,769 bytes. The first
M3 build was 17,003, over the gate. The rare opcodes then moved into one
out-of-line arm, as did the `call`/`apply` rewrite loop and the copy of
extra arguments.

Slopes against M2 (instructions per iteration):

| Probe | M2 | M3 |
| --- | ---: | ---: |
| `call0` | 596 | 625 |
| `constructor` | 1,177 | 1,234 |
| `literal` | 905 | 946 |
| `prop_write` | 169 | 175 |
| `empty_loop` | 105 | 107 |

The 4–5% increases come from the larger `run`. `literal` has no call in
its loop yet grows by the same share, so the increase is code generation
(spills in a bigger function) rather than a path that runs. M5 reads the
profiles before changing anything.
