# New core, phase 2: results

This page records the measurements of [phase 2](new-core-phase2.md) of the new
core (#122) as its milestones land: register bytecode, the lowering from the
compiler's stack code, and the main loop in `oxide-core`
([`core/`](../../core/README.md)). M3 and M4 record the arithmetic, call and
closure probes and the loop's size; M5 runs the fixed probes through the
`oxide-run` driver and the baseline's runner; M6 records the exit tables of
section 9 ([below](#m6-exit-tables)).

## Identity (M3, M4)

| Item | Value |
| --- | --- |
| Source | branch `core/phase2-m3-m4` (the M3/M4 pull request) |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2 |
| Toolchain | rustc 1.99.0 (pinned by `rust-toolchain.toml`), release profile (`lto = "fat"`, `codegen-units = 1`) |
| Binary | the root crate's release test binary (`cargo test --release --lib --no-run`), which links both cores; its ignored `run_probe_file` test runs one file on either core |
| Counter | `perf stat -e instructions:u` (perf 7.2.10) |
| Driver | [`probes/new-core/phase2.py`](probes/new-core/phase2.py) `--cpu 2 --repeat 3`; N = 2·10⁵ and 4·10⁵ (2·10⁴ and 4·10⁴ for `tdz`, `type_error`, `bigint_64`) |

Every number is a per-iteration slope, (I(2N) − I(N)) / N, the median of three
runs, so startup, compilation and lowering cancel. Nothing is subtracted: both
cores carry their own loop overhead. Until the M5 driver exists the current
core runs through the engine's embedding API with a JavaScript `print`, not
through `qjs`, so its column differs slightly from the whole-process figures of
[baseline.md](baseline.md).

## Probes

| Probe | Current core | New core | Ratio |
| --- | ---: | ---: | ---: |
| `empty_loop` | 290 | 109 | 2.7× |
| `numeric_local_loop` | 995 | 160 | 6.2× |
| `local_read` | 822 | 174 | 4.7× |
| `local_read_with_setup` | 822 | 174 | 4.7× |
| `local_move` | 663 | 161 | 4.1× |
| `argument_move` | 448 | 135 | 3.3× |
| `call0` | 2,376 | 691 | 3.4× |
| `prop_read` | 530 | 295 | 1.8× |
| `prop_write` | 948 | 220 | 4.3× |
| `array_read` | 496 | 294 | 1.7× |
| `array_write` | 1,400 | 279 | 5.0× |
| `tdz` | 34,268 | 3,494 | 9.8× |
| `type_error` | 30,339 | 2,577 | 11.8× |
| `bigint_64` | 7,244 | 6,279 | 1.2× |
| `{a:i,b:i}` (Table B `literal`) | 2,691 | 901 | 3.0× |
| `new P(i)`, two `this.x =` (`constructor`) | 5,631 | 2,038 | 2.8× |
| `let j = i; s = () => j` (`closure`) | 18,982 | 1,815 | 10.5× |
| depth-3 prototype read (`proto_read_3`) | 1,242 | 683 | 1.8× |

The current-core column reproduces the phase 1 ledger totals (`{a:i,b:i}`
2,689, `new P(i)` 5,488, the closure 18,374) within 3%.

Readings:

- **Arithmetic and moves** (M3). The register forms are 3 to 6 times fewer
  instructions; `numeric_local_loop` is four register instructions per
  iteration (`JNgtI`, `AddI`, `SubI`, `Jmp`) at about 40 instructions each.
- **Calls** (M4). `call0` is a `GetGlobal` of `unit` (an uncached lookup in
  the global object's dictionary, phase 3's global IC), the in-loop `Call`, the
  callee's `LoadInt` and `Return`, and the caller's `Add` and loop. It beats
  the current core by 3.4× but is still above QuickJS's 388
  ([baseline.md](baseline.md)); the global lookup is the largest single part.
- **Closures**. The closure probe allocates a cell, the capture scope and the
  function each iteration and is 10.5× below the current core.
- **Uncached properties** gain least (1.7–1.8×): every read is a shape walk
  and a prototype walk, which phase 3's inline caches replace.
- **Exceptions**. `tdz` and `type_error` create an error object and unwind
  through the handler table in-loop, about 10× below the current core.
- **BigInt** runs on the schoolbook sign-magnitude code of `vm/bigint.rs`
  and is level with the current core, as section 9 expects until phase 4.

## The loop (Table C, first rows)

| | Size | Instructions per register instruction |
| --- | ---: | ---: |
| `oxide_core::vm::interp::run` | 12,684 bytes | `empty_loop` 36 (3 per iteration), `numeric_local_loop` 40 (4 per iteration) |
| `execute_admitted_in_state` (current core, same binary) | 31,810 bytes | 61–82 ([baseline.md](baseline.md)) |

Sizes are `nm --size-sort` of the release test binary. The first M3 build of
the loop was 47 KB: the slow paths and the error formatting were inlined into
it, and every compare-jump and arithmetic opcode had its own arm. The slow
paths are now `#[inline(never)]` (section 4.2), the error messages go through
one cold function, the arithmetic opcodes past `Add` and `Sub` share an arm,
and the 24 compare-jumps share one arm whose number path reads a per-opcode
mask of taken orders (less, equal, greater, unordered) instead of switching
on the relation.

The register accesses carry no bounds checks: a frame's registers are a
`&mut [Value; 512]` window, and the register file keeps 512 values reserved
past `top`, so an 8-bit operand (or a call's `b + 1`) indexes it directly in
safe Rust. What remains per dispatch is the `code[pc]` check, the operand
decode and the jump table; the layout experiment and the bounds-check count
of Table C are M6's.

## Decisions taken in M3 and M4

- **Retargeting** (section 10) is kept: it makes `x = y` one `Mov` and
  `sum = sum + 1` one `AddI` into the local, which the move and numeric probes
  above rely on.
- **TDZ checks** stay where M2 left them (one check per binding per block
  between joins); no fixed probe's hot loop contains one, so further elision
  waits for a program that needs it.
- **A compare-jump against a constant that is not an `i16`** materializes the
  constant into its temporary (M2's lowering); no probe's loop does this.
- **Register-file growth and the stack limit**: the file doubles, keeps 512
  values of padding past its end, and stops at 2^20 values (8 MiB), past which
  a call throws QuickJS's `InternalError: stack overflow`. Rust-level
  re-entries (a conversion calling a user `valueOf`) share a 1 MiB native
  stack budget, checked from stack addresses as QuickJS's
  `js_check_stack_overflow` does.

## M5: the fixed probes through the runner

`oxide-run` (the CLI crate's second binary, section 5) runs each probe as
`qjs` does, so `fixed.py` measures it with the [baseline](baseline.md)'s
method: same machine and CPU, `--repeat 5 --cpu 2 --linux-counters --timeout
900`, whole-process wall time and `instructions:u` including startup and
compilation, medians of 5. All 100 samples produced the manifest's exact
stdout. The QuickJS and current-core columns are the baseline's, recorded
earlier the same day; the new core ran alone, not interleaved with them, so
its times are indicative and M6 repeats the run interleaved.

| Workload | QuickJS ms | current ms | new ms | QuickJS G instr | current G instr | new G instr | new / QuickJS instr |
|---|---:|---:|---:|---:|---:|---:|---:|
| numeric_local_loop | 47 | 143 | 31 | 0.50 | 3.03 | 0.48 | 0.96 |
| numeric_coercion_loop | 21 | 231 | 22 | 0.15 | 2.62 | 0.21 | 1.39 |
| local_read_with_setup | 54 | 123 | 41 | 0.39 | 2.50 | 0.52 | 1.34 |
| local_read | 53 | 123 | 41 | 0.39 | 2.50 | 0.52 | 1.34 |
| bigint_32 | 16 | 55 | 50 | 0.06 | 0.60 | 0.71 | 11.89 |
| bigint_64 | 21 | 62 | 45 | 0.19 | 0.76 | 0.60 | 3.15 |
| bigint_256 | 16 | 35 | 27 | 0.08 | 0.32 | 0.25 | 3.11 |
| prop_read | 30 | 59 | 37 | 0.40 | 1.09 | 0.59 | 1.48 |
| prop_write | 28 | 97 | 31 | 0.33 | 1.92 | 0.44 | 1.34 |
| array_read | 32 | 57 | 36 | 0.46 | 1.02 | 0.59 | 1.28 |
| array_write | 30 | 139 | 37 | 0.39 | 2.82 | 0.56 | 1.43 |
| call0 | 30 | 115 | 43 | 0.31 | 1.79 | 0.55 | 1.79 |
| string_bridge | 19 | 51 | 25 | 0.14 | 0.70 | 0.22 | 1.58 |
| type_error | 21 | 46 | 15 | 0.17 | 0.45 | 0.03 | 0.17 |
| tdz | 21 | 49 | 15 | 0.17 | 0.50 | 0.04 | 0.26 |
| local_move | 55 | 206 | 61 | 0.80 | 4.08 | 0.97 | 1.21 |
| argument_move | 54 | 132 | 50 | 0.70 | 2.75 | 0.81 | 1.16 |
| number_owner_fallback | 24 | 58 | 20 | 0.15 | 1.01 | 0.16 | 1.08 |
| object_move | 20 | 45 | 19 | 0.12 | 0.69 | 0.14 | 1.13 |
| empty_loop | 52 | 145 | 59 | 0.90 | 2.96 | 1.09 | 1.21 |

Geometric means over the 20 probes: the new core executes **1.32×** QuickJS's
instructions (the current core 5.02×) in **1.11×** its time (2.90×), and
**3.79×** fewer instructions than the current core.

- Under QuickJS: `numeric_local_loop`, and `tdz` and `type_error` by a wide
  margin, because a phase 2 error object has no backtrace; QuickJS builds one
  for every throw.
- Within 1.1–1.5× of QuickJS: the moves, local reads and the uncached
  property and element probes.
- `call0` (1.79×) pays an uncached global lookup per call; `bigint_32`
  (11.9×) and the other BigInt probes allocate a heap record per operation,
  where QuickJS has short BigInts (phase 4).
- `numeric_coercion_loop` and `string_bridge`, which the plan recorded only
  if the lowering accepted them, run: the first calls a user `valueOf` from
  the `+` slow path through a nested loop, the second concatenates flat
  strings.

```sh
cargo build --release -p quickjs-oxide-cli --bin oxide-run
python3 scripts/benchmark/fixed.py --manifest docs/performance/probes/fixed/manifest.json \
  --workload-dir docs/performance/probes/fixed --engine new=target/release/oxide-run \
  --repeat 5 --cpu 2 --linux-counters --timeout 900 --output $R
```

## Correctness record

- The corpus in [`core/tests/corpus`](../../core/tests/corpus) (20 programs:
  the M2 cases for rule 2, per-iteration bindings, `finally` on every exit
  path and nested handler depths, plus arithmetic, comparisons, objects,
  arrays, strings, closures, calls, constructors, globals, TDZ, control flow,
  BigInt, conversions and stack overflow) and the fixed probes at smoke size
  print identically on both cores
  (`src/engine/compiler/register/run_tests.rs`), with and without the
  `new-core-gc-stress` feature.
- The corpus lowers to every opcode the lowering emits; the opcodes it never
  emits (wide shuttles, tail calls, `ReturnUndefined`, the fused nullish and
  `typeof` tests, `DeleteField`, `JUndefinedOrNull`) run from built units in
  [`core/tests/vm.rs`](../../core/tests/vm.rs).
- A collection with 200 frames live preserves every frame's registers, and
  `cargo test -p oxide-core --features gc-stress` passes.
- (M5) `scripts/checks/check-new-core-stress.py` runs `oxide-run
  --gc-stress=1` (built with `new-core-gc-stress`) over the fixed probes at
  smoke size against the manifest and over the corpus against the unstressed
  run; CI's fast job runs it.

## M6: exit tables

### Identity

| Item | Value |
| --- | --- |
| Machine | eric-83am, AMD Ryzen 7 7840HS, Linux 7.2.9, measured processes pinned to CPU 2: the [baseline](baseline.md)'s reference machine |
| New core | `oxide-run` from `core/phase2-m6` (engine code as of `0eb02dd7`), `cargo build --release -p quickjs-oxide-cli --bin oxide-run`, rustc 1.99.0, sha256 `02948bbe04fbe605…` |
| Current core | the baseline's `qjs` (main `8e16f672`), sha256 `3cc9c95d597af1bd…` |
| QuickJS | the baseline's 2026-06-04 oracle build, sha256 `3fd16c25ae63a7cb…` |
| Programs | `docs/performance/probes/fixed/manifest.json` (unchanged since the baseline); Table B and the loop probes from [`probes/new-core/phase2.py`](probes/new-core/phase2.py) |
| Whole-process runs | `fixed.py --repeat 5 --cpu 2 --linux-counters --timeout 900`, the three engines interleaved in rotating order; all 300 samples printed the manifest's stdout |
| Slopes | `phase2.py --engine … --repeat 5 --cpu 2`: (I(2N) − I(N)) / N, medians, N = 2·10⁵ (2·10⁴ for `tdz`, `type_error`, `bigint_64`) |

Raw samples stay outside the repository (`target/m6/` of the measuring tree).

### Table A: the fixed probes

Instructions per iteration (slopes, so startup and compilation cancel; the
QuickJS column is within 1.5% of section 9's whole-process figures):

| Probe | QuickJS | Current core | New core | New / QuickJS | Current / new |
| --- | ---: | ---: | ---: | ---: | ---: |
| `empty_loop` | 90 | 295 | 109 | 1.21 | 2.7 |
| `numeric_local_loop` | 167 | 1,007 | 160 | 0.96 | 6.3 |
| `local_read` | 130 | 830 | 174 | 1.34 | 4.8 |
| `local_read_with_setup` | 130 | 830 | 174 | 1.34 | 4.8 |
| `local_move` | 133 | 679 | 161 | 1.21 | 4.2 |
| `argument_move` | 117 | 457 | 135 | 1.15 | 3.4 |
| `call0` | 388 | 2,231 | 691 | 1.78 | 3.2 |
| `prop_read` | 201 | 543 | 295 | 1.47 | 1.8 |
| `prop_write` | 163 | 958 | 220 | 1.35 | 4.4 |
| `array_read` | 227 | 505 | 294 | 1.30 | 1.7 |
| `array_write` | 193 | 1,406 | 279 | 1.45 | 5.0 |
| `tdz` | 13,612 | 36,758 | 3,716 | 0.27 | 9.9 |
| `type_error` | 13,623 | 32,948 | 2,542 | 0.19 | 13.0 |
| `bigint_64` | 1,843 | 7,488 | 6,109 | 3.31 | 1.2 |

Whole process, the three engines interleaved (time is wall time, median of
5; instructions include startup and compilation):

| Workload | QuickJS ms | current ms | new ms | QuickJS G instr | current G instr | new G instr | new / QuickJS time | new / QuickJS instr |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| numeric_local_loop | 47 | 140 | 32 | 0.50 | 3.03 | 0.48 | 0.69 | 0.96 |
| numeric_coercion_loop | 19 | 233 | 22 | 0.15 | 2.62 | 0.21 | 1.15 | 1.37 |
| local_read_with_setup | 53 | 123 | 40 | 0.39 | 2.50 | 0.52 | 0.76 | 1.34 |
| local_read | 54 | 123 | 41 | 0.39 | 2.50 | 0.52 | 0.76 | 1.34 |
| bigint_32 | 16 | 52 | 50 | 0.06 | 0.60 | 0.71 | 3.17 | 11.80 |
| bigint_64 | 21 | 62 | 45 | 0.19 | 0.76 | 0.60 | 2.12 | 3.22 |
| bigint_256 | 16 | 33 | 26 | 0.08 | 0.32 | 0.25 | 1.63 | 3.13 |
| prop_read | 31 | 60 | 37 | 0.40 | 1.09 | 0.59 | 1.20 | 1.46 |
| prop_write | 28 | 96 | 32 | 0.33 | 1.92 | 0.44 | 1.14 | 1.35 |
| array_read | 32 | 57 | 36 | 0.46 | 1.02 | 0.59 | 1.12 | 1.29 |
| array_write | 30 | 140 | 38 | 0.39 | 2.82 | 0.56 | 1.24 | 1.44 |
| call0 | 31 | 116 | 43 | 0.31 | 1.79 | 0.55 | 1.41 | 1.78 |
| string_bridge | 18 | 50 | 25 | 0.14 | 0.70 | 0.22 | 1.38 | 1.63 |
| type_error | 20 | 47 | 15 | 0.17 | 0.47 | 0.03 | 0.71 | 0.17 |
| tdz | 21 | 50 | 15 | 0.18 | 0.53 | 0.04 | 0.71 | 0.25 |
| local_move | 55 | 200 | 61 | 0.80 | 4.08 | 0.97 | 1.11 | 1.21 |
| argument_move | 53 | 134 | 50 | 0.70 | 2.75 | 0.81 | 0.95 | 1.15 |
| number_owner_fallback | 24 | 58 | 20 | 0.15 | 1.01 | 0.16 | 0.85 | 1.07 |
| object_move | 19 | 45 | 19 | 0.12 | 0.69 | 0.14 | 1.00 | 1.09 |
| empty_loop | 52 | 147 | 60 | 0.90 | 2.96 | 1.09 | 1.16 | 1.21 |

Geometric means over the 20 probes against QuickJS: the current core
**2.89×** time and **5.03×** instructions (the baseline recorded 2.90× and
5.02×), the new core **1.12×** time and **1.32×** instructions.

### Table B: the phase 1 workloads as JavaScript

Instructions per iteration, slopes:

| Workload | QuickJS | Current core | New core | Phase 1, Rust heap work only |
| --- | ---: | ---: | ---: | ---: |
| `{a:i,b:i}` | 1,298 | 2,680 | 904 | 415 |
| `new P(i)`, two `this.x =` | 1,878 | 5,479 | 2,041 | 602 |
| `let j = i; s = () => j` | 2,072 | 18,881 | 1,827 | |
| depth-3 prototype read | 243 | 1,194 | 683 | |

Heap and interpreter together, the literal and the closure are below
QuickJS and the constructor is 9% above it. Over phase 1's heap-only Rust
numbers, the interpreter adds about 490 instructions to the literal and about
1,440 to the constructor. This page does not split them further; the
constructor's share includes the uncached `prototype` lookup in `New` and two
uncached `this.x =` stores, which phase 3's constructor and store caches
target. The prototype read walks three shapes uncached per iteration (2.8×
QuickJS).

### Table C: the loop

1. **Size.** `oxide_core::vm::interp::run` is 12,684 bytes; the measured
   current core's `execute_admitted_in_state` is 46,232 bytes (31,810 in the
   M3/M4 test binary; #119 measured about 64 KB on another build). The new
   loop is well below the current one.
2. **Instructions per register instruction.** `empty_loop` executes three
   register instructions per iteration (`JNlt`, `Inc`, `Jmp`) at 109
   instructions, 36 each; `numeric_local_loop` four (`JNgtI`, `AddI`, `SubI`,
   `Jmp`) at 160, 40 each. The current core's dispatch costs 61–82. Of the
   36, 19 are the shared dispatch header: the `code[pc]` check, loading the
   8-byte instruction, decoding its four operand fields and the jump-table
   branch (`pc` and the code pointer live on the stack). The rest is the arm.
3. **Layout.** As in #119, one saved LTO build of `oxide-run` was relinked
   at `.text` offsets of 16, 32, 48, 64, 128, 192, 256, 1024 and 4096 bytes
   ([`probes/new-core/relink.py`](probes/new-core/relink.py)): byte-identical
   machine code at different addresses, plus the default placement. Seven
   repeats per variant, interleaved:

   | Probe | Cycles, best / median / worst (G) | Spread of cycles | Spread of time |
   | --- | --- | ---: | ---: |
   | `empty_loop` | 0.192 / 0.211 / 0.256 | 30% | 22% |
   | `numeric_local_loop` | 0.085 / 0.094 / 0.116 | 33% | 21% |
   | `call0` | 0.144 / 0.145 / 0.145 | 0.9% | 2.1% |

   Every 64-byte-aligned offset runs exactly like the default; only the
   16-, 32- and 48-byte shifts move: about +22% cycles at 16 and −9% at 32
   on both loop probes. The tight loops are sensitive to where the dispatch header and
   the hot arms fall within 64-byte lines; `call0`, whose iteration crosses
   more code, is not. Instruction counts are identical in every variant.
4. **Bounds checks** on the hot paths, counted in the disassembly of the
   default variant: the `Add` arm (Int32 path: two tag checks, the overflow
   branch, the store) and the compare-jump arm (two tag checks, the mask
   test) contain none. The register window is a `&mut [Value; 512]` indexed
   by 8-bit operands, so register reads and writes need no check. The one
   remaining bounds check per dispatch is `code[pc]` in the header. Section
   11's `get_unchecked` experiment therefore has a single candidate site, worth
   at most one compare and branch per register instruction.

### The benchmark programs

Every function of the baseline's suite programs, lowered on its own
(`lowering_census`, an ignored test in
`src/engine/compiler/register/run_tests.rs`, over the baseline's
`suite_matrix.py` build):

| Programs | Functions lowered | Declined, by construct |
| --- | ---: | --- |
| V8 v7 crypto, deltablue, navier-stokes, richards, splay | all but one each | the harness's `print` shim (rest parameters) |
| V8 v7 raytrace, earley-boyer | 83/86, 398/435 | `arguments` (1, 34), for-in, `eval` |
| V8 v7 regexp | 29/43 | RegExp literals (12), one function over 256 registers |
| Octane (15) | box2d 877/1013, pdfjs 940/980, typescript 1784/1811, mandreel 1494/1507, … | `arguments` (box2d 135), RegExp literals, for-in, object literal methods (pdfjs), `eval` |
| AWFY (14 bundles) | 396/493 each | classes (81), `ToObject` (14), iterators |

- **The register window.** One function in the whole suite exceeds 256
  registers: `RegExpBenchmark` in V8 v7 and Octane regexp (315 and 316). It
  is a straight-line setup function, not a hot loop. Mandreel and PdfJS,
  which section 11 expected to need the wide shuttles, fit the window, so the
  window layout needs no revisiting in phase 3.
- **What the V8 v7 programs need next.** With the harness's `print` shim
  removed, the five programs that lower completely run until their first
  builtin (`Math`); raytrace and earley-boyer need `arguments`, regexp needs
  RegExp literals. Builtins, `arguments` and RegExp are the phase 3 surface
  the go/no-go needs.

### Exit

- Tables A–C and the M2 corpus result (above, M3/M4) are recorded here.
- `gc-stress` is green on everything section 6 lists: `cargo test -p
  oxide-core --features gc-stress`, the lowering and two-core corpus under
  `new-core-gc-stress`, and `oxide-run --gc-stress=1` over the fixed probes
  and the corpus in CI.
- The current engine's Test262 is unchanged: `test-test262.sh --check`
  authenticates the spec and receipts, `--focused` matches the frozen vector
  (6,844 of 6,844 variants), and `--full` replays the complete result body
  (83,304 passes of 83,354 eligible variants, 102,037 total, as frozen).
- **The two design checks of section 9 pass.** `vm::run` is 12.7 KB against
  the current loop's 46 KB, and `call0` beats the current core by 3.2× in
  instructions and 2.7× in time. Phase 2 goes forward to phase 3.
- **Superinstructions** (section 10): none of the current core's
  execution-only superinstructions or numeric regions is wanted back. The
  probes they served (`numeric_local_loop`, the moves) are at or near
  QuickJS without them. What remains is the dispatch header and code
  alignment (Table C), plus the uncached global, property and constructor
  paths that phase 3's inline caches replace.
