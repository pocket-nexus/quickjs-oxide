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

## M4: the realm and the regular-expression crate

**The eight programs run.** With the builtins of section 6.2 and the
`oxide-regexp` crate (#144), every V8 v7 program of the suite build runs to
its completion line on `oxide-run` and prints the manifest's exact stdout.
The benchmarks check their own results and throw on a wrong one.

A first, indicative timing: one process each, the full fixed work, pinned
to one CPU, against the current core's `qjs` from the same tree. M6 takes
the reference measurement of section 9 (three engines interleaved, medians
of five).

| Program | New core s | Current core s | Current / new |
| --- | ---: | ---: | ---: |
| richards | 1.18 | 4.17 | 3.53 |
| deltablue | 1.15 | 4.90 | 4.26 |
| crypto | 0.91 | 3.43 | 3.77 |
| raytrace | 1.00 | 5.21 | 5.21 |
| earley-boyer | 0.81 | 4.72 | 5.83 |
| regexp | 1.20 | 1.83 | 1.53 |
| splay | 2.25 | 5.61 | 2.49 |
| navier-stokes | 1.12 | 3.15 | 2.81 |

The geometric mean of the ratios is about 3.4, against the gate's 3.0.
Splay sits just under the 2.5 per-program floor, and NavierStokes is the
second lowest.

**`ic-stats` on the eight**, one `run()` each, read before M5 touches
anything:

| Program | Get hit % | Set hit % | Global hit % | Construct hit % | Largest miss sources |
| --- | ---: | ---: | ---: | ---: | --- |
| richards | 99.9 | 99.8 | 99.7 | 30 | none hot |
| deltablue | 94.4 | 99.3 | 99.5 | 97.8 | array `length` (18,027 at one site) |
| crypto | 88.1 | 97.7 | 99.5 | 85 | `this.am` and other `BigInteger.prototype` methods, unfilled |
| raytrace | 91.5 | 95.4 | 99.8 | 99.9 | `this.initialize` in `Class.create` (megamorphic, 66,590); array `length` |
| earley-boyer | 99.9 | 99.98 | 99.9 | 99.3 | none hot |
| regexp | 91.0 | — | 99.8 | — | string `length` (2 × 16,479); `replace`/`split` method reads |
| splay | 99.99 (90% through polymorphic rows) | 94.9 | 99.99 | 99.98 | `left`/`right` transitions at the full range (24,550 fills) |
| navier-stokes | (few) | (few) | (few) | — | the hot loops are element accesses and arithmetic |

Each row points at an M5 candidate:
- **`length`**: cache array and string `length` (phase 1's in-arm path
  was lost to the `GetField` cache).
- **Crypto**: cache prototype hits whose holder is in dictionary mode.
  `BigInteger.prototype` has more than 64 methods, and its changes clear
  the cell like any prototype's.
- **RayTrace**: the megamorphic site.
- **Splay**: the slots reserved for constructed objects, which later grow
  `left`/`right`.

**The loop.** `vm::interp::run` is 13,833 bytes. The probe slopes are
unchanged from M3.

**Correctness.**
- One corpus program per row of section 6.2's table, printing identically
  on both cores with and without stress:
  - `builtins_global.js`, `builtins_object.js`, `builtins_function.js`,
    `builtins_array.js`, `builtins_string.js`, `builtins_number.js`,
    `builtins_boolean.js`, `builtins_math.js`, `builtins_error.js`,
    `builtins_regexp.js`
  - edge cases include radix bounds, negative and fractional indices,
    `NaN` and `±0` through `Math`, empty-string splits, `lastIndex` with
    and without `g` and `y`, replacement patterns with named groups,
    captures in `split`, and Unicode case mapping.
- `check-new-core-stress.py` passes with 39 corpus programs.
- The radix port has a differential test against the current engine
  (`radix_tests`): about 340,000 formatting cases and 240,000 parsing
  cases, with no difference.

## M5: profile and tune

Every change below is justified by whole-program `instructions:u`, the
median of three runs of the full fixed work, pinned to one CPU. The profiles
are `perf record` on the same binaries, and the cache data is the `ic-stats`
report of M4. Cycles are shown too, but they swing with code layout: two
builds with identical instruction counts differed by up to 7% in cycles on
NavierStokes, which is the sensitivity phase 2's layout experiment measured.

| Change | Evidence | Program | Before | After |
| --- | --- | --- | ---: | ---: |
| Exact radix-10 number formatting without `Big` (a walk down from 17 digits by format and parse), and one-byte string concatenation as bytes | Splay: 2.8% in `Big::pow`, 10% in widening and narrowing concatenated strings | splay | 25.77G | 19.76G |
| `Mul` and `Div` on doubles inline in the arm; a double product stays a double; `captured_cell` inlined | NavierStokes: 13% in `number_op`, 7% in `captured_cell` | navier-stokes | 23.83G | 22.43G |
| A `length` entry for arrays and strings; prototype hits on dictionary-mode holders cached | DeltaBlue: 18,027 `length` misses at one site; Crypto: about 70,000 unfilled `this.am` misses | crypto, deltablue | 16.44G, 18.95G | 16.21G, 18.64G |
| A shared megamorphic table (4,096 entries keyed by shape and key) | RayTrace: 66,590 misses at `this.initialize` | raytrace | 16.85G | 16.16G |
| At least four slots for constructed receivers and empty literals | Splay: 24,550 full-range transition misses on `left`/`right` | raytrace, splay | 16.16G, 19.88G | 15.78G, 19.50G |
| `instanceof` reads the constructor's `prototype` slot directly; the receiver-size learning exits first when nothing grew; `==`/`===` against `null`, `undefined` and identical bits decided before the general algorithm | Earley-Boyer: 11% in `instance_of`, `own` and `slot_value`, and 3.9% in `learn_receiver_size`; Richards: 5.6% in `loose_eq` | richards, earley-boyer | 20.31G, 13.91G | 19.24G, 13.15G |
| `index`, `input` and `groups` as realm atoms | RegExp: interning on every match | regexp | 20.98G | 20.69G |

The equality shortcut first went into the comparison arms. That grew every
compare-jump, and Crypto lost 7.5% in cycles while losing instructions. It
now runs at the top of the out-of-line slow path instead, which keeps the
arms unchanged.

**M4 against M5**, whole program:

| Program | M4 G instr | M5 G instr | Change |
| --- | ---: | ---: | ---: |
| richards | 20.25 | 19.24 | −5.0% |
| deltablue | 18.96 | 18.63 | −1.7% |
| crypto | 16.34 | 16.21 | −0.8% |
| raytrace | 17.07 | 15.67 | −8.2% |
| earley-boyer | 13.87 | 13.16 | −5.1% |
| regexp | 21.04 | 20.68 | −1.7% |
| splay | 25.77 | 19.48 | −24.4% |
| navier-stokes | 23.83 | 22.43 | −5.9% |

Indicative time against the current core's `qjs` (pinned, medians of
three, not interleaved):

| Program | Current / new |
| --- | ---: |
| richards | 3.86 |
| deltablue | 4.16 |
| crypto | 3.67 |
| raytrace | 5.62 |
| earley-boyer | 5.72 |
| regexp | 1.57 |
| splay | 3.22 |
| navier-stokes | 3.03 |

The geometric mean is 3.61. M6 takes the reference measurement.

**The open decisions of section 10**, now settled:

- **A complete-miss cache for `GetField`: not added.** Neither RayTrace
  nor Earley-Boyer shows a hot complete-miss site in `ic-stats`.
- **Megamorphic sites re-recording after a cooldown: superseded.** A
  megamorphic site probes the shared table instead, and its misses fill
  it. The table is weak like every entry and pruned at sweep.
- **A lazily created `prototype` for closures: not added.** `MakeClosure`
  is below 1% of Splay and Earley-Boyer.
- **A cached `prototype` slot for `instanceof`: done**, as a direct read of
  the constructor's own data slot. A cache site would need an operand that
  `InstanceOf`'s format does not have.
- **The GC trigger: kept at a 4 MiB minimum and growth 2.** Only Splay is
  above section 9's 10% threshold: 9 collections, 572 ms of pauses, a
  76 MB live set at the end. Growth 3 saves 100 ms for about 170 MB more
  reserved heap, and growth 4 saves 280 ms for about 230 MB more. The
  marking is memory-bound: inline marking of strings and BigInts was 16%
  slower, so it was dropped. Splay is at 3.2× the current core with
  growth 2.
- **The `bool` fast path for `JTrue`/`JFalse` after `In`/`InstanceOf`: no
  change needed.** The jump arm already tests a boolean inline.
- **Ropes (section 11's trigger): not pulled forward.** String work is
  about 12% of Splay's instructions, but almost all of it is allocating
  records and contents; copying is 2%. Splay builds each string with two
  concatenations and never reads it, so ropes would still allocate one node
  per concatenation.

**The loop.** `vm::interp::run` is 14,600 bytes, under the 16 KB gate.

## M6: exit tables and the go/no-go

### Identity

| Item | Value |
| --- | --- |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2: the [baseline](baseline.md)'s reference machine |
| New core | `oxide-run` from `core/phase3-m5` (`a25e812d`), `cargo build --release -p quickjs-oxide-cli --bin oxide-run`, rustc 1.99.0, sha256 `b6644338628cbed9…` |
| Current core | the baseline's `qjs` (main `8e16f672`), sha256 `3cc9c95d597af1bd…` |
| QuickJS | the baseline's 2026-06-04 oracle build, sha256 `3fd16c25ae63a7cb…` |
| Programs | `suite_matrix.py build` at the pinned sources (V8 v7 `2034d98f`, Octane `570ad1cc`, AWFY `74306fec`); the manifest's sha256 differs from the baseline's only by the absolute paths it records, and the program bytes are checked against it before every sample |
| Whole-process runs | `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, the three engines interleaved in rotating order, medians of 5; all 120 suite samples and 300 probe samples printed the manifest's stdout |
| Slopes | `phase2.py --engine … --cpu 2 --repeat 5` |

Raw samples stay outside the repository (`target/m6p3/` of the measuring tree).

### Table A: the eight V8 v7 programs

| Program | QuickJS ms | Current ms | New ms | QuickJS G instr | Current G instr | New G instr | Current / new time | New / QuickJS time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| richards | 1,069 | 4,170 | 1,089 | 14.12 | 74.60 | 19.24 | 3.83 | 1.02 |
| deltablue | 1,095 | 4,738 | 1,130 | 13.32 | 71.89 | 18.63 | 4.19 | 1.03 |
| crypto | 991 | 3,420 | 892 | 13.86 | 69.15 | 16.21 | 3.83 | 0.90 |
| raytrace | 1,076 | 5,003 | 882 | 15.88 | 65.72 | 15.67 | 5.67 | 0.82 |
| earley-boyer | 1,027 | 4,444 | 773 | 14.23 | 62.68 | 13.16 | 5.75 | 0.75 |
| regexp | 1,191 | 1,815 | 1,168 | 20.57 | 28.15 | 20.68 | 1.55 | 0.98 |
| splay | 1,784 | 5,670 | 1,774 | 15.89 | 60.74 | 19.48 | 3.20 | 0.99 |
| navier-stokes | 1,051 | 3,080 | 1,010 | 14.44 | 65.68 | 22.43 | 3.05 | 0.96 |
| geometric mean | | | | | | | **3.63** | **0.93** |

Against QuickJS, the current core is at 3.37× time (the baseline recorded
3.38×) and the new core at 0.93×. The new core executes 1.22× QuickJS's
instructions, in less time on five of the eight programs.

### Table B: the fixed probes and the phase 1 workloads

Instructions per iteration (slopes):

| Probe | QuickJS | Current | New | New / QuickJS |
| --- | ---: | ---: | ---: | ---: |
| `empty_loop` | 90 | 295 | 108 | 1.20 |
| `numeric_local_loop` | 167 | 1,007 | 159 | 0.95 |
| `local_read` | 130 | 830 | 172 | 1.32 |
| `local_move` | 133 | 679 | 156 | 1.17 |
| `argument_move` | 117 | 457 | 132 | 1.13 |
| `call0` | 388 | 2,231 | 622 | 1.60 |
| `prop_read` | 201 | 543 | 222 | 1.10 |
| `prop_write` | 163 | 958 | 177 | 1.09 |
| `array_read` | 227 | 505 | 301 | 1.33 |
| `array_write` | 193 | 1,406 | 287 | 1.49 |
| `tdz` | 13,604 | 36,758 | 3,279 | 0.24 |
| `type_error` | 13,615 | 32,957 | 2,347 | 0.17 |
| `bigint_64` | 1,843 | 7,488 | 6,096 | 3.31 |
| `{a:i,b:i}` (`literal`) | 1,294 | 2,680 | 943 | 0.73 |
| `new P(i)` (`constructor`) | 1,874 | 5,479 | 1,164 | 0.62 |
| closure | 2,072 | 18,881 | 1,828 | 0.88 |
| depth-3 prototype read | 243 | 1,194 | 288 | 1.19 |

Whole process over the 20 fixed probes: the new core is at **1.09×**
QuickJS's time (phase 2: 1.12×) and the current core at 2.89×. Against
phase 2's exit tables, the cached paths moved as follows:
- `prop_read`: 295 → 222
- `prop_write`: 220 → 177
- `proto_read_3`: 683 → 288
- `constructor`: 2,041 → 1,164
- `call0`: 691 → 622

`call0` is the largest remaining gap, and it is in the call sequence
itself, not in a cache.

### Table C: the loop and the caches

`vm::interp::run` is 14,600 bytes, against the 16 KB gate (12,684 at the
end of phase 2). `ic-stats`, one `run()` per program:

| Program | Get hit % | Set hit % | Global hit % | Construct hit % | Polymorphic sites | Megamorphic sites |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| richards | 99.91 | 99.77 | 99.67 | 30.4 | 1 | 0 |
| deltablue | 99.87 | 99.29 | 99.50 | 97.8 | 69 | 0 |
| crypto | 99.93 | 98.20 | 99.46 | 85.0 | 36 | 0 |
| raytrace | 99.93 | 99.53 | 99.81 | 99.9 | 39 | 1 |
| earley-boyer | 99.98 | 99.98 | 99.86 | 99.3 | 0 | 0 |
| regexp | 98.95 | — | 99.83 | — | 0 | 0 |
| splay | 99.99 | 99.98 | 99.99 | 99.98 | 56 | 0 |
| navier-stokes | (few) | (few) | (few) | — | 0 | 0 |

The low construct rates of Richards and Crypto come from setup
constructors that run a handful of times. RegExp's misses are its
`runBlock11`, whose named-property sites past the 255th share the no-cache
site. Splay reads mostly through polymorphic rows: its tree nodes gain
`left` and `right` in either order. The layout spread of #119's experiment
is not repeated, because Table A's medians and the instruction counts
agree.

### Table D: the collector

At the default trigger (4 MiB minimum, growth 2):

| Program | Collections | Total pause ms | p50 ms | p99 ms | Max ms | Live at exit | Heap reserved | Peak RSS QuickJS / current / new |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| richards | 0 | 0 | | | | | 1.4 MiB | 15 / 15 / 15 MiB |
| deltablue | 8 | 4.9 | 0.57 | 0.86 | 0.86 | 95 KiB | 6.6 MiB | 15 / 18 / 15 MiB |
| crypto | 0 | 0 | | | | | 1.4 MiB | 15 / 15 / 15 MiB |
| raytrace | 72 | 27.6 | 0.37 | 0.62 | 0.62 | 59 KiB | 7.6 MiB | 15 / 15 / 15 MiB |
| earley-boyer | 40 | 55.9 | 1.40 | 2.71 | 2.71 | 3.2 MiB | 16.8 MiB | 15 / 39 / 23 MiB |
| regexp | 19 | 26.3 | 1.39 | 2.01 | 2.01 | 2.9 MiB | 10.8 MiB | 15 / 16 / 15 MiB |
| splay | 9 | 582.5 | 87.0 | 106.8 | 106.8 | 74.9 MiB | 294 MiB | 144 / 310 / 259 MiB |
| navier-stokes | 0 | 0 | | | | | 4.5 MiB | 15 / 15 / 15 MiB |

Only Splay is above the 10% threshold, at about a third of its time. M5
measured growth 3 and 4 and kept growth 2 for memory; the numbers are in
the M5 section. Splay holds a 75 MiB live set and its peak RSS is 1.8×
QuickJS's, which refcounts.

### Stress

- The two-core corpus (39 programs) passes under `--gc-stress=1`.
- The eight programs pass at one `run()` each under the stress collector
  (`check-new-core-suite.py`): seven at `--gc-stress=1`, Splay at
  `--gc-stress=100` (879 s).
- Splay at interval 1 did not finish in 55 minutes, because every
  allocation marks its live set. The nightly tier uses interval 100 for it.

### The decision

Section 9's gate is two conditions:
- the geometric mean over the eight of current-core time over new-core
  time is at least 3.0;
- no program other than RegExp is below 2.5.

The measured values are **3.63** and a lowest non-RegExp program of
**3.05** (NavierStokes). Both conditions hold: **go**. Phase 4 starts on
the design as built.

The per-program floor was this plan's addition to #122's one-line gate,
and the plan asks the maintainer to confirm or change it before M6. Both
conditions also hold with any floor up to 3.05.

Notes for phase 4:
- The new core retires 1.22× QuickJS's instructions in 0.93× its time.
  The interpreter header is not the limit on these programs.
- The remaining gaps against QuickJS are:
  - the call sequence (`call0` at 1.6×);
  - element access (`array_write` at 1.49×);
  - Splay's collector (a third of its time), where a generational or
    incremental collector is the lever, not the trigger.
- The current core's Test262 numbers are unchanged (focused 6,844 of 6,844,
  full 83,304 of 83,354, run on the crate move, #144), and no later PR
  touches code the current engine runs.
