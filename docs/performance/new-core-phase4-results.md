# New core, phase 4: results

This page records the measurements of [phase 4](new-core-phase4.md) of the
new core (#122): the Test262 runner on the new core and the frozen list of
dropped variants (M0), the convergence of Test262 to the exit number (M9),
and the exit tables of section 10 with the floor read (M10). The
milestones M1–M8 landed as one pull request (#148); their acceptance
programs are the two-core corpus in `core/tests/corpus` and the 37 suite
programs, which all print the manifest's stdout on `oxide-run`.

## Method

| Item | Value |
| --- | --- |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2 (the [baseline](baseline.md)'s reference machine) |
| Toolchain | rustc 1.99.0 (pinned by `rust-toolchain.toml`), release profile (`lto = "fat"`, `codegen-units = 1`) |
| New core | `oxide-run` from `core/phase4`, `cargo build --release -p quickjs-oxide-cli --bin oxide-run`, sha256 `7756fa4df14df0ba…` |
| Current core | the baseline's `qjs` (main `8e16f672`), sha256 `3cc9c95d597af1bd…` |
| QuickJS | the baseline's 2026-06-04 oracle build, sha256 `3fd16c25ae63a7cb…` |
| Programs | the 37 programs of `suite_matrix.py build` at the pinned sources (V8 v7 `2034d98f`, Octane `570ad1cc`, AWFY `74306fec`), manifest sha256 `60b1fa8a6c27ddfb…` (phase 3 M6's) |
| Whole-process runs | `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, the three engines interleaved in rotating order, medians of 5, every sample's stdout checked against the manifest |
| Slopes | `probes/new-core/phase2.py --engine … --cpu 2 --repeat 5`: (I(2N) − I(N)) / N of `instructions:u`, medians |
| Instruction gate | `perf stat -e instructions:u` of the eight V8 v7 programs at the full fixed work, one run each pinned to CPU 2, against phase 3 M6's `oxide-run` (`a25e812d`, sha256 `b6644338628cbed9…`) |
| Loop size | `nm --size-sort` of `oxide-run`, symbol `oxide_core::vm::interp::run_at` |

Raw samples stay outside the repository (`target/m10/` of the measuring
tree).

## M0: Test262 on the new core

### The runner's second backend

`run-test262 --engine new-core` (or `TEST262_ENGINE=new-core`) runs each
variant's worker over `api::new_core::test262` instead of the current
engine's `Runtime`/`Context`. Everything before the engine split is shared:
the metadata, the admissions, the harness order (`assert.js`, `sta.js`, the
includes, `doneprintHandle.js` for async tests, each its own script), the
negative-diagnostic contracts and the result rows. The new worker:
- captures `print` in the runtime and reads the async log from it;
- loads modules through the exact-module loader behind the core's module
  host, so the admitted graphs resolve as they do on the current engine;
- authenticates the dynamic-import roots over the lowered code (an
  `ImportCall` in the script's or the module graph's units);
- reads thrown values without running JavaScript (`Runtime::thrown_summary`:
  own `name`, `message` and `stack` data properties) and takes a position
  from the first located frame of `stack`, as the current worker does;
- splits a module failure into parse (the root), resolution (the rest of
  the graph and linking) and runtime (`run_module_phased`).

`scripts/test262/test-test262.sh` accepts an `engine` key, passes `--engine`
to the runner, writes the new core's full report to
`target/test262-new-core-full.*`, and authenticates the dropped-variant list
named by `new-core.conf`. The current engine's spec, receipts and default
backend are unchanged.

### The dropped-variant list

`dropped-features-probe` (a root-crate feature, never in a release or a
default gate) switches off in the current engine the five behaviors phase 0
dropped: `with` is a `SyntaxError`, direct `eval` runs as indirect eval,
`$262.createRealm` throws, every `Symbol.species` lookup uses the intrinsic
constructor without reading `constructor`, and unscopables go unread with
`with`. One full run with the feature, against the current engine's frozen
83,304, gives **D = 1,897** variants that pass today and fail under it
(phase 0's static estimate was 1,738–3,197), frozen with a reason each in
[`dev-support/test262/new-core-dropped-variants.tsv`](../../dev-support/test262/new-core-dropped-variants.tsv):

| Reason | Variants |
| --- | ---: |
| `Symbol.species` (`constructor` and `@@species` reads) | 528 |
| direct `eval` (in the test or an included harness file) | 517 |
| realms (`$262.createRealm`, `cross-realm`) | 488 |
| `with` | 311 |
| unscopables (`with` over `Symbol.unscopables`) | 53 |
| **D** | **1,897** |

The reason is read from the probe run's failure (`realms are not
supported`, `with statements are not supported`) and the test's source and
includes; it is a label for the reconciliation below, not part of the gate.

## M9: Test262 convergence

The first full run on the new core (this page's M0 runner, before any fix)
passed 81,347 and failed 210 variants outside D. Every one of them was a
bug, fixed in this phase:

| Area | Variants | Fix |
| --- | ---: | --- |
| `o[k] op= v`, `o[k]++`, `o[k] &&= v` on a `null` or `undefined` base | 68 | the base is checked before the key converts (`ToPropertyKeyOf`, `property::get_computed`) |
| global and `eval` declarations | 51 | `CanDeclareGlobalVar` and `CanDeclareGlobalFunction` before any binding; `eval`'s bindings configurable; a scope entered again keeps the cell a hoisted closure holds (`ClearCell`) |
| `yield*` | 14 | `IterCall`'s "`return` without a value" called `throw` |
| `delete super.x` | 14 | `ThrowReference` ends a block |
| class fields | 11 | defined with `CreateDataPropertyOrThrow`: proxies, frozen receivers, `static ['prototype']` |
| `Set` methods | 10 | the copy follows the keys iterator, `difference` iterates its copy, the index loops read the live size |
| typed arrays and buffers | 8 | `ValidateUint8Array` checks the type only; a growable `SharedArrayBuffer`'s views track its length |
| `caller`, `arguments`, `%ThrowTypeError%` | 6 | generators' `caller` throws; non-simple parameters get an unmapped `arguments` |
| RegExp | 6 | `exec` reads the flags after `lastIndex`; the constructor converts after allocating |
| `GetFunctionRealm` | 4 | a revoked proxy throws |
| `Promise.all`, `allSettled` | 4 | a finished iterator is not closed |
| error positions | 3 | `stack` from the lowered code's positions; a JSON module's syntax error located |
| `import()` in `eval` code | 2 | resolved against the calling script |
| `sort` | 2 | every trailing index is deleted |
| `Math.atanh` | 2 | fdlibm's `atanh` (and `acosh`) |
| `replace` | 2 | stops at the string length limit instead of building 2³⁶ units |
| a 10⁵-deep `WeakMap` chain | 2 | ephemerons in one pass (25.7 s to 9 ms) |
| `with` in strict `eval` code | 1 | `with` is an early `SyntaxError` |
| **total** | **210** | |

`with` is now the early `SyntaxError` phase 0 specifies; before, a `with`
whose body named no binding ran, and one that did resolved the name
wrongly.

### Table E: Test262

| | Current engine | New core |
| --- | ---: | ---: |
| QuickJS-scope pass (of 83,562) | 83,304 | 81,445 |
| Full pass (of 102,037) | 83,304 | 81,445 |
| Eligible (runnable) | 83,354 | 83,354 |
| Focused vector (of 6,844) | 6,844 | 6,814 |

- **Exit condition.** QuickJS-scope passes ≥ 83,304 − D = 81,407: the new
  core passes **81,445**.
- **The difference list.** 1,866 variants pass on the current engine and
  fail on the new core; **all 1,866 are in D, none outside it**.
- **Reconciliation.** 31 variants of D pass on the new core: 27 negative
  `with` tests whose expected diagnostic the shared front end still gives
  (strict code, an invalid declaration body), where the probe's single
  message did not, and 4 direct-eval `this` tests whose value is the global
  object either way. D is an allowance, so they count as passes.
- The new core's summary: `fail-async=16 fail-async-done-count=53 fail-parse=324 fail-runtime=1516 pass=81445 …`, the skipped and unsupported counts unchanged. The focused vector's 30 failures are in D (realms and direct eval).
- The new core's full run takes about 2.5 minutes on this machine with
  15 workers, as the current engine's does.
- The four gates of this pull request pass: the current engine's focused
  (6,844 of 6,844) and full (83,304 of 83,354) vectors are unchanged, and
  the new core's frozen focused and full vectors replay.

The new core's receipts are frozen in
[`dev-support/test262/new-core.conf`](../../dev-support/test262/new-core.conf)
(focused TSV and JSONL under `generated/`, full body hashes), replayed by
`test-test262.sh --spec dev-support/test262/new-core.conf --focused|--full`.
CI runs `--check` in `fast`, the focused vector on every pull request next to
the current engine's (`test262-focused`), and the full vector nightly
(`new-core-test262-full`).

## M1: the call sequence and element writes

M1 asked for `call0` and `array_write` below 1.3× QuickJS's slope and every
V8 v7 program at or below phase 3 M6's instructions. Phase 4 grew the loop
by the language surface first, so this work landed at the end, measured
against the same targets:

| Probe | QuickJS | Phase 3 M6 | Phase 4 before M1 | Phase 4 | Target |
| --- | ---: | ---: | ---: | ---: | ---: |
| `call0` | 388 | 622 | 635 | **554** (1.43×) | 504 |
| `array_write` | 193 | 287 | 293 | **246** (1.27×) | 251 |
| `empty_loop` | 90 | 108 | 113 | 92 | |

What changed:
- **The return.** A plain call's frame (not an entry, a construction or a
  tail call) returns to the frame below inside the loop and loads the
  caller's unit once; `finish_return` re-read the frame and the unit that
  the loop then read again.
- **The call.** The loop's `Call` arm enters a bytecode frame through an
  inlined `enter_frame`.
- **Loops.** A back edge to a loop whose test is one compare-jump repeats
  the test, negated, as a jump into the body, when the back edge is in the
  test's handler regions (a `continue` inside `try` keeps the jump): an
  iteration runs one branch instead of a jump and the test. The two-core
  corpus has the `continue`-in-`try` case.
- **`o[k] = v;`** stores without the copy of `v` that the assignment's
  dropped value kept (`Insert3; PutArrayEl; Drop`), a `Mov` per store.

**`call0` stays above the target.** After these, a `call0` iteration is
about 550 instructions over seven dispatches, the callee's two included.
Callgrind attributes about 50 of them to bounds-checked indexing (the
instruction fetch `code[pc]` and the register file), and the rest to the
frame push and pop through `Vec`, the sloppy callee's `this` (the global
object), the decode of the instruction word on every dispatch and the
global cache. Removing the bounds checks is unchecked indexing, which
`#![forbid(unsafe_code)]` and section 11's dropped `get_unchecked`
exclude. The 504 target is not met, and the remaining gap is in the safe
indexing the project keeps.

**Instruction gate**, the final binary against phase 3 M6's (`a25e812d`):

| Program | Change |
| --- | ---: |
| richards | −5.26% |
| deltablue | −6.04% |
| crypto | −6.33% |
| raytrace | −3.41% |
| earley-boyer | −2.96% |
| regexp | +0.85% |
| splay | +0.13% |
| navier-stokes | −2.36% |

Six of the eight are below phase 3 M6's count; RegExp and Splay are within
1%. `vm::interp::run_at` is 16,021 bytes against the 16,384-byte gate: the
inlined entry and return took the margin, and the next inline arm has to
move code out first.

Also measured at the end and fixed: the closure probe (3,430 → 2,139, QuickJS
2,080), where `SetFunctionName` allocated a new string per anonymous
closure instead of storing the name's atom in the closure's own `name`
slot; and the lowering's position table, whose linear lookup per
instruction compiled Octane's Mandreel in 17 s.

## M10: the exit tables

### Table A: the 37 suite programs

| Program | QuickJS ms | Current ms | New ms | QuickJS G instr | Current G instr | New G instr | Current / new time | New / QuickJS time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| v8-v7-richards | 1,074 | 4,160 | 1,056 | 14.12 | 74.60 | 18.23 | 3.94 | 0.98 |
| v8-v7-deltablue | 1,098 | 4,720 | 1,051 | 13.32 | 71.89 | 17.51 | 4.49 | 0.96 |
| v8-v7-crypto | 982 | 3,381 | 813 | 13.86 | 69.15 | 15.18 | 4.16 | 0.83 |
| v8-v7-raytrace | 1,085 | 5,018 | 909 | 15.89 | 65.72 | 15.14 | 5.52 | 0.84 |
| v8-v7-earley-boyer | 1,027 | 4,444 | 807 | 14.24 | 62.69 | 12.77 | 5.50 | 0.79 |
| v8-v7-regexp | 1,201 | 1,803 | 1,168 | 20.57 | 28.14 | 20.87 | 1.54 | 0.97 |
| v8-v7-splay | 1,784 | 5,604 | 1,755 | 15.88 | 60.74 | 19.50 | 3.19 | 0.98 |
| v8-v7-navier-stokes | 1,055 | 3,060 | 1,014 | 14.44 | 65.68 | 21.90 | 3.02 | 0.96 |
| octane-richards | 1,072 | 4,351 | 1,076 | 14.10 | 74.60 | 18.23 | 4.04 | 1.00 |
| octane-deltablue | 1,100 | 4,701 | 1,043 | 13.36 | 71.89 | 17.51 | 4.51 | 0.95 |
| octane-crypto | 968 | 3,373 | 810 | 13.84 | 69.16 | 15.19 | 4.16 | 0.84 |
| octane-raytrace | 1,092 | 4,991 | 908 | 15.91 | 65.72 | 15.14 | 5.49 | 0.83 |
| octane-earley-boyer | 1,018 | 4,441 | 810 | 14.11 | 62.69 | 12.76 | 5.49 | 0.80 |
| octane-regexp | 1,275 | 2,274 | 1,251 | 21.54 | 35.41 | 22.12 | 1.82 | 0.98 |
| octane-splay | 1,801 | 5,601 | 1,770 | 15.89 | 60.76 | 19.51 | 3.16 | 0.98 |
| octane-navier-stokes | 1,040 | 3,102 | 1,013 | 14.44 | 65.74 | 21.92 | 3.06 | 0.97 |
| octane-pdfjs | 1,125 | 11,796 | 1,343 | 17.68 | 148.80 | 19.89 | 8.78 | 1.19 |
| octane-mandreel | 2,412 | 12,776 | 3,470 | 33.59 | 229.80 | 64.00 | 3.68 | 1.44 |
| octane-gameboy | 1,056 | 8,059 | 2,168 | 15.74 | 123.40 | 44.92 | 3.72 | 2.05 |
| octane-code-load | 1,018 | 3,027 | 2,514 | 11.55 | 27.90 | 23.62 | 1.20 | 2.47 |
| octane-box2d | 1,047 | 5,196 | 990 | 13.00 | 74.14 | 16.66 | 5.25 | 0.95 |
| octane-zlib | 6,329 | 29,220 | 7,400 | 76.73 | 559.15 | 158.48 | 3.95 | 1.17 |
| octane-typescript | 1,668 | 8,648 | 1,827 | 15.32 | 95.63 | 25.95 | 4.73 | 1.10 |
| awfy-DeltaBlue | 517 | 2,907 | 530 | 6.31 | 36.05 | 8.14 | 5.49 | 1.02 |
| awfy-Richards | 3,370 | 14,363 | 3,566 | 42.58 | 239.46 | 58.61 | 4.03 | 1.06 |
| awfy-Json | 886 | 6,627 | 952 | 12.15 | 81.64 | 15.52 | 6.96 | 1.08 |
| awfy-CD | 1,906 | 9,127 | 1,722 | 25.89 | 134.22 | 28.72 | 5.30 | 0.90 |
| awfy-Havlak | 3,182 | 19,099 | 2,996 | 35.75 | 212.99 | 39.57 | 6.37 | 0.94 |
| awfy-Bounce | 975 | 10,149 | 903 | 15.15 | 133.83 | 17.83 | 11.24 | 0.93 |
| awfy-List | 946 | 4,619 | 1,113 | 12.84 | 93.65 | 19.72 | 4.15 | 1.18 |
| awfy-Mandelbrot | 524 | 2,003 | 521 | 7.32 | 41.02 | 9.15 | 3.84 | 0.99 |
| awfy-NBody | 741 | 6,333 | 770 | 11.50 | 94.85 | 15.45 | 8.22 | 1.04 |
| awfy-Permute | 1,202 | 5,099 | 1,067 | 15.96 | 97.55 | 20.49 | 4.78 | 0.89 |
| awfy-Queens | 790 | 3,485 | 818 | 10.39 | 62.83 | 13.84 | 4.26 | 1.04 |
| awfy-Sieve | 918 | 7,214 | 1,117 | 15.34 | 136.32 | 22.91 | 6.46 | 1.22 |
| awfy-Storage | 1,490 | 6,601 | 807 | 16.43 | 87.85 | 14.65 | 8.18 | 0.54 |
| awfy-Towers | 1,201 | 5,167 | 1,130 | 14.80 | 94.36 | 19.01 | 4.57 | 0.94 |
| **v8-v7 geometric mean (8)** | | | | | | | **3.68** | **0.91** |
| **octane geometric mean (15)** | | | | | | | **3.84** | **1.12** |
| **awfy geometric mean (14)** | | | | | | | **5.69** | **0.97** |

- **The floor.** The geometric mean of current-core time over new-core
  time is **3.68** over the V8 v7 eight, **3.84** over Octane's 15 and
  **5.69** over AWFY's 14, against 3.0. Every program is at or above 3.0
  except the three the floor excludes: V8 v7 RegExp (1.54) and Octane
  RegExp (1.82), whose time is in the regular-expression engine both cores
  share, and Code-Load (1.20), whose work is the shared front end (parsing
  and compiling, see below). The lowest included programs are V8 v7
  NavierStokes at **3.02** and Octane NavierStokes at **3.06**.
- **Against QuickJS** the new core is at **0.91×** time over the eight
  (phase 3: 0.93×), 1.12× over Octane and 0.97× over AWFY.
- **Code-Load** is 2.5× QuickJS's time: 62% of its instructions are the
  front end both cores share and the lowering of each source it loads (the
  benchmark compiles generated code through `eval` and `Function`).
- **Mandreel and GameBoy** run at 1.44× and 2.05× QuickJS's time with the
  floor held (3.68 and 3.72). They have units with more than 255
  named-property sites (Mandreel 28, GameBoy one of 546 sites), past which
  the sites share the no-cache entry (Table C), a phase 3 limit of the
  8-bit site field; widening it is the lever for both.
- Several V8 v7 programs moved by 3–6% in time between builds with the
  same instruction counts (NavierStokes: 1,003 ms on phase 3 M6's binary,
  973 to 1,131 ms on phase 4's intermediate builds). That is code layout,
  as #119 measured; NavierStokes's 3.02 has that much noise in it.

### Table B: the fixed probes and the phase 1 workloads

Instructions per iteration (slopes); the last eight are this phase's new
probes (`phase2.py`'s `TABLE_PHASE4`):

| Probe | QuickJS | Current | New | New / QuickJS |
| --- | ---: | ---: | ---: | ---: |
| `empty_loop` | 90 | 295 | 92 | 1.02 |
| `numeric_local_loop` | 167 | 1,007 | 148 | 0.89 |
| `local_read` | 130 | 830 | 147 | 1.13 |
| `local_read_with_setup` | 130 | 830 | 147 | 1.13 |
| `local_move` | 133 | 679 | 140 | 1.05 |
| `argument_move` | 117 | 457 | 116 | 0.99 |
| `call0` | 388 | 2,231 | 554 | 1.43 |
| `prop_read` | 201 | 543 | 204 | 1.01 |
| `prop_write` | 163 | 958 | 159 | 0.98 |
| `array_read` | 227 | 505 | 279 | 1.23 |
| `array_write` | 193 | 1,406 | 246 | 1.27 |
| `tdz` | 13,612 | 36,805 | 9,436 | 0.69 |
| `type_error` | 13,611 | 33,085 | 8,269 | 0.61 |
| `bigint_64` | 1,843 | 7,488 | 6,064 | 3.29 |
| `{a:i,b:i}` (`literal`) | 1,294 | 2,680 | 970 | 0.75 |
| `new P(i)` (`constructor`) | 1,874 | 5,479 | 1,181 | 0.63 |
| closure | 2,080 | 18,881 | 2,139 | 1.03 |
| depth-3 prototype read | 243 | 1,194 | 263 | 1.08 |
| `for_of_array` | 4,508 | 65,443 | 3,239 | 0.72 |
| `spread_call` | 4,119 | 45,061 | 2,459 | 0.60 |
| `class_method` | 561 | 2,791 | 662 | 1.18 |
| `generator_next` | 1,958 | 30,889 | 1,942 | 0.99 |
| `typed_write` | 259 | 1,698 | 383 | 1.48 |
| `map_get` | 678 | 5,096 | 1,329 | 1.96 |
| `string_build` | 742 | 2,775 | 736 | 0.99 |
| `sort_cmp` | 5,239 | 74,115 | 6,236 | 1.19 |

Whole process over the 20 fixed probes: the new core is at **1.09×**
QuickJS's time (phase 3: 1.09×) and the current core at 2.90×.

### Table C: the loop and the caches

`vm::interp::run_at` is **16,021 bytes** against the 16,384-byte gate
(phase 3: 14,600). `ic-stats` (an `oxide-run` built with
`new-core-ic-stats`), one `run()` per program, with the pristine-array
`IterStart`s against the ones that ran the protocol:

| Program | Get hit % | Set hit % | Global hit % | Construct hit % | Polymorphic sites | Megamorphic sites | Pristine / protocol `IterStart` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| v8-v7-richards | 99.91 | 99.77 | 99.67 | (few) | 1 | 0 | 0 / 0 |
| v8-v7-deltablue | 99.87 | 99.29 | 99.50 | 97.75 | 69 | 0 | 0 / 0 |
| v8-v7-crypto | 99.93 | 98.20 | 99.46 | (few) | 36 | 0 | 0 / 0 |
| v8-v7-raytrace | 99.93 | 99.52 | 99.81 | 99.92 | 39 | 1 | 0 / 0 |
| v8-v7-earley-boyer | 99.98 | 99.98 | 99.86 | 99.29 | 0 | 0 | 0 / 0 |
| v8-v7-regexp | 98.95 | (few) | 99.83 | (few) | 0 | 0 | 0 / 0 |
| v8-v7-splay | 99.99 | 99.98 | 99.99 | 99.98 | 56 | 0 | 0 / 0 |
| v8-v7-navier-stokes | (few) | (few) | (few) | (few) | 0 | 0 | 0 / 0 |
| octane-richards | 99.91 | 99.75 | 99.62 | (few) | 1 | 0 | 0 / 0 |
| octane-deltablue | 99.87 | 99.25 | 99.45 | 97.75 | 69 | 0 | 0 / 0 |
| octane-crypto | 99.92 | 98.09 | 99.42 | (few) | 43 | 0 | 0 / 0 |
| octane-raytrace | 99.92 | 99.51 | 99.80 | 99.92 | 39 | 1 | 0 / 0 |
| octane-earley-boyer | 99.98 | 99.98 | 99.86 | 99.29 | 0 | 0 | 0 / 0 |
| octane-regexp | 99.43 | 99.98 | 99.81 | (few) | 0 | 0 | 0 / 0 |
| octane-splay | 99.99 | 99.98 | 99.98 | 99.98 | 56 | 0 | 0 / 0 |
| octane-navier-stokes | (few) | (few) | (few) | (few) | 0 | 0 | 0 / 0 |
| octane-pdfjs | 99.16 | 99.01 | 99.97 | (few) | 74 | 0 | 0 / 0 |
| octane-mandreel | 89.90 | (few) | 99.88 | (few) | 0 | 0 | 0 / 0 |
| octane-gameboy | 7.70 | 0.00 | 99.96 | (few) | 0 | 0 | 0 / 0 |
| octane-code-load | 40.56 | (few) | (few) | (few) | 3 | 2 | 0 / 0 |
| octane-box2d | 99.79 | 97.50 | 99.82 | 99.26 | 373 | 2 | 0 / 0 |
| octane-zlib | 98.28 | (few) | 99.30 | (few) | 10 | 1 | 0 / 0 |
| octane-typescript | 75.31 | 76.65 | 99.98 | 99.90 | 857 | 382 | 0 / 0 |
| awfy-DeltaBlue | 100.00 | 99.64 | 99.98 | 99.99 | 70 | 0 | 0 / 0 |
| awfy-Richards | 100.00 | 100.00 | (few) | — | 9 | 0 | 0 / 0 |
| awfy-Json | 100.00 | 99.99 | 99.98 | 100.00 | 0 | 0 | 0 / 0 |
| awfy-CD | 100.00 | 99.97 | 99.99 | 100.00 | 3 | 0 | 0 / 0 |
| awfy-Havlak | 100.00 | 99.27 | 100.00 | 100.00 | 6 | 0 | 0 / 0 |
| awfy-Bounce | 100.00 | 100.00 | 100.00 | 100.00 | 0 | 0 | 75,000 / 0 |
| awfy-List | 100.00 | 100.00 | (few) | 100.00 | 0 | 0 | 0 / 0 |
| awfy-Mandelbrot | (few) | (few) | (few) | — | 0 | 0 | 0 / 0 |
| awfy-NBody | 100.00 | 100.00 | 100.00 | (few) | 0 | 0 | 0 / 0 |
| awfy-Permute | 100.00 | 100.00 | 97.14 | — | 0 | 0 | 0 / 0 |
| awfy-Queens | 100.00 | 99.98 | 99.92 | — | 0 | 0 | 0 / 0 |
| awfy-Sieve | 99.93 | (few) | 99.02 | — | 0 | 0 | 0 / 0 |
| awfy-Storage | 100.00 | 100.00 | 100.00 | 99.90 | 0 | 0 | 0 / 0 |
| awfy-Towers | 100.00 | 100.00 | (few) | 99.99 | 0 | 0 | 0 / 0 |

GameBoy's and Mandreel's low `GetField` rates are sites past the 255th of a
unit, which share the no-cache entry: GameBoy's one such unit (546 sites)
is its hot one. Code-Load's rate is over a few hundred reads
(its time is compilation). TypeScript's misses are almost all unfilled
ones (17.4 million `GetField`s: accessors, dictionary-mode receivers,
complete misses and its 382 megamorphic sites), not refills. Bounce is the one program whose hot loop is a `for-of` over an
array; every one of its 75,000 `IterStart`s took the pristine path.

### Table D: the collector

At the default trigger (4 MiB minimum, growth 2), one run each:

| Program | Collections | Total pause ms | p50 ms | p99 ms | Max ms | Share of time | Live at exit | Heap reserved | Peak RSS QuickJS / current / new |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| v8-v7-richards | 0 | 0.0 |  |  |  | 0.0% |  | 1.5 MiB | 19 / 19 / 19 MiB |
| v8-v7-deltablue | 8 | 5.9 | 0.70 | 0.99 | 0.99 | 0.6% | 150 KiB | 6.7 MiB | 19 / 19 / 19 MiB |
| v8-v7-crypto | 0 | 0.0 |  |  |  | 0.0% |  | 1.6 MiB | 19 / 19 / 19 MiB |
| v8-v7-raytrace | 82 | 39.9 | 0.48 | 0.72 | 0.72 | 4.4% | 154 KiB | 6.2 MiB | 19 / 19 / 19 MiB |
| v8-v7-earley-boyer | 39 | 60.7 | 1.71 | 2.86 | 2.86 | 7.5% | 3.2 MiB | 16.0 MiB | 19 / 40 / 23 MiB |
| v8-v7-regexp | 18 | 32.3 | 1.85 | 2.18 | 2.18 | 2.8% | 3.0 MiB | 9.9 MiB | 19 / 19 / 19 MiB |
| v8-v7-splay | 9 | 578.8 | 86.42 | 108.53 | 108.53 | 33.0% | 75.0 MiB | 293.6 MiB | 144 / 312 / 265 MiB |
| v8-v7-navier-stokes | 0 | 0.0 |  |  |  | 0.0% |  | 5.6 MiB | 19 / 19 / 19 MiB |
| octane-richards | 0 | 0.0 |  |  |  | 0.0% |  | 1.5 MiB | 19 / 19 / 19 MiB |
| octane-deltablue | 8 | 5.8 | 0.70 | 0.93 | 0.93 | 0.6% | 152 KiB | 6.7 MiB | 19 / 19 / 19 MiB |
| octane-crypto | 0 | 0.0 |  |  |  | 0.0% |  | 1.6 MiB | 19 / 19 / 19 MiB |
| octane-raytrace | 82 | 37.7 | 0.46 | 0.67 | 0.67 | 4.1% | 156 KiB | 6.2 MiB | 19 / 19 / 19 MiB |
| octane-earley-boyer | 39 | 57.8 | 1.64 | 2.83 | 2.83 | 7.1% | 2.7 MiB | 24.0 MiB | 19 / 41 / 24 MiB |
| octane-regexp | 18 | 34.2 | 1.95 | 2.33 | 2.33 | 2.7% | 3.0 MiB | 9.9 MiB | 19 / 19 / 19 MiB |
| octane-splay | 9 | 601.4 | 88.84 | 115.00 | 115.00 | 34.0% | 75.0 MiB | 293.6 MiB | 144 / 306 / 263 MiB |
| octane-navier-stokes | 0 | 0.0 |  |  |  | 0.0% |  | 5.6 MiB | 19 / 19 / 19 MiB |
| octane-pdfjs | 30 | 91.6 | 3.14 | 5.38 | 5.38 | 6.8% | 5.6 MiB | 43.2 MiB | 22 / 81 / 68 MiB |
| octane-mandreel | 4 | 1.1 | 0.28 | 0.32 | 0.32 | 0.0% | 1.0 MiB | 1.4 MiB | 57 / 103 / 100 MiB |
| octane-gameboy | 12 | 10.1 | 0.80 | 1.73 | 1.73 | 0.5% | 2.4 MiB | 15.0 MiB | 23 / 36 / 24 MiB |
| octane-code-load | 9 | 16.7 | 1.98 | 2.19 | 2.19 | 0.7% | 1.4 MiB | 5.9 MiB | 19 / 53 / 121 MiB |
| octane-box2d | 12 | 22.6 | 1.89 | 2.12 | 2.12 | 2.3% | 1.3 MiB | 8.9 MiB | 19 / 28 / 22 MiB |
| octane-zlib | 2 | 0.1 | 0.07 | 0.07 | 0.07 | 0.0% | 356 KiB | 1.4 MiB | 134 / 148 / 20 MiB |
| octane-typescript | 3 | 16.0 | 3.90 | 10.89 | 10.89 | 0.9% | 20.3 MiB | 95.7 MiB | 98 / 237 / 90 MiB |
| awfy-DeltaBlue | 9 | 18.8 | 1.55 | 4.95 | 4.95 | 3.5% | 6.3 MiB | 25.9 MiB | 19 / 54 / 30 MiB |
| awfy-Richards | 0 | 0.0 |  |  |  | 0.0% |  | 926 KiB | 19 / 19 / 19 MiB |
| awfy-Json | 11 | 11.0 | 1.00 | 1.13 | 1.13 | 1.2% | 520 KiB | 7.4 MiB | 19 / 19 / 19 MiB |
| awfy-CD | 45 | 36.9 | 0.82 | 1.16 | 1.16 | 2.1% | 414 KiB | 7.8 MiB | 19 / 21 / 19 MiB |
| awfy-Havlak | 48 | 331.2 | 7.96 | 10.04 | 10.04 | 11.1% | 8.3 MiB | 49.6 MiB | 33 / 106 / 41 MiB |
| awfy-Bounce | 2 | 1.5 | 0.78 | 0.78 | 0.78 | 0.2% | 213 KiB | 7.8 MiB | 19 / 19 / 19 MiB |
| awfy-List | 0 | 0.0 |  |  |  | 0.0% |  | 3.5 MiB | 19 / 19 / 19 MiB |
| awfy-Mandelbrot | 0 | 0.0 |  |  |  | 0.0% |  | 284 KiB | 19 / 19 / 19 MiB |
| awfy-NBody | 7 | 3.6 | 0.46 | 0.82 | 0.82 | 0.5% | 201 KiB | 7.3 MiB | 19 / 19 / 19 MiB |
| awfy-Permute | 0 | 0.0 |  |  |  | 0.0% |  | 400 KiB | 19 / 19 / 19 MiB |
| awfy-Queens | 1 | 0.5 | 0.52 | 0.52 | 0.52 | 0.1% | 199 KiB | 6.0 MiB | 19 / 19 / 19 MiB |
| awfy-Sieve | 47 | 10.2 | 0.20 | 0.43 | 0.43 | 0.9% | 198 KiB | 5.9 MiB | 19 / 19 / 19 MiB |
| awfy-Storage | 94 | 52.1 | 0.55 | 0.72 | 0.72 | 6.5% | 573 KiB | 6.1 MiB | 19 / 19 / 19 MiB |
| awfy-Towers | 0 | 0.0 |  |  |  | 0.0% |  | 1.0 MiB | 19 / 19 / 19 MiB |

Splay (33–34%) and Havlak (11.1%) are above the 10% threshold. Havlak at
the growths phase 3 M5 measured for Splay, five runs each:

| Growth | Time | Collections | Total pause | Heap reserved | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| 2 (default) | 2,952 ms | 48 | 335 ms | 49.6 MiB | 41 MiB |
| 3 | 2,793 ms | 34 | 258 ms | 62.7 MiB | 53 MiB |
| 4 | 2,741 ms | 27 | 209 ms | 66.8 MiB | 61 MiB |

Growth 3 buys Havlak 160 ms (5%) for 12 MiB of peak memory; it is already
6.37× the current core and 0.94× QuickJS at growth 2. The trigger stays at
growth 2, and the decision is the maintainer's (section 11).

### Stress

- The two-core corpus (44 programs) and the 20 fixed probes pass at
  `--gc-stress=1` (`check-new-core-stress.py`).
- All 37 suite programs pass at one `run()` under the stress collector.
  Twenty-eight run at `--gc-stress=1` (the longest: Storage 1,480 s,
  V8 v7 Earley-Boyer 1,300 s, Octane RegExp 507 s). Nine run at
  `--gc-stress=100`: the seven that did not finish in 30 minutes at
  interval 1 (both Splays, PdfJS, TypeScript, AWFY DeltaBlue, CD and
  Havlak) and the two above 20 minutes. At 100 they take 4–1,028 s
  (Havlak the longest, both Splays about 950 s).
- `check-new-core-suite.py` now runs all 37 with these intervals, and the
  nightly `new-core-suite` tier with it (about 93 minutes on this machine).

### The lowering census

`lowering_census` declines nothing in the 37 suite programs. Over Test262
(`CENSUS_TEST262`), 48,115 of the 53,125 test files compile (the rest are
the front end's negative tests), and 158,978 of their 159,311 functions
lower; the 333 that do not are the functions containing `with`, phase 0's
early `SyntaxError`. Thirty functions are past the 256-register window and
lower with a spill scope.

### Section 11's open decisions

| Decision | Measurement | Outcome |
| --- | --- | --- |
| a kind-specific `IterNext` for Map and Set | Havlak at 0.94× QuickJS and 6.37× the current core; no Test262 program is slow | not built |
| a heap-resident generator frame | `generator_next` at 0.99× QuickJS (trigger: 1.5×) | not built: the copy stays |
| `Array.prototype` methods' dense fast path past the first callback | `sort_cmp` 1.19×, `for_of_array` 0.72× QuickJS | the current admission stays |
| `%TypedArray%` callback methods per kind or through one reader | the typed-array programs hold the floor (PdfJS 8.78, Mandreel 3.68, GameBoy 3.72, zlib 3.95), and their gaps to QuickJS are the cache sites above, not the callback methods | one generic reader stays |
| the stress intervals of the typed-array programs | interval 1 for Mandreel (24 s), GameBoy (265 s) and zlib (13 s), 100 for PdfJS | as listed above |
| a two-byte-aware rope flattening into a preallocated buffer | with ropes PdfJS retires 19.9G instructions (46.0G without), TypeScript 25.95G either way; flattening is not among PdfJS's top functions, whose string work is short concatenations (4.1%) and interning (3.6%) | the per-flatten copy suffices |
| raising the loop gate to 20 KB | `run_at` is 16,021 bytes | not raised; the margin is spent (M1) |
| growth for programs other than Splay above 10% | Havlak, above | growth 2, the maintainer's call |

## The exit condition

| Condition | Result |
| --- | --- |
| Tables A–E committed | this page |
| `gc-stress` green on section 8's programs | the corpus, the probes and all 37 programs (Stress) |
| the current engine's Test262 unchanged | focused 6,844 of 6,844; full 83,304 of 83,354 |
| Test262 on the new core | 81,445 ≥ 83,304 − D = 81,407; no difference outside D; focused the same way |
| the suites report | all 37 programs print the manifest's stdout (Table A) |
| the floor | 3.68 / 3.84 / 5.69 by suite; no included program below 3.0 (lowest 3.02) |
| against QuickJS (reported) | 0.91× over the eight |

Two points for the maintainer:
- the plan extends the 3.0 floor to Octane and AWFY with Octane RegExp and
  Code-Load excluded, and asks for the extension to be confirmed: it
  holds as written;
- M1's `call0` target (1.3× QuickJS) is not met, at 1.43×, for the reason
  M1's section gives; `array_write` meets its target.
