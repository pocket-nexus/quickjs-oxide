# New core, phase 2: results

This page records the measurements of [phase 2](new-core-phase2.md) of the new
core (#122) as its milestones land: register bytecode, the lowering from the
compiler's stack code, and the main loop in `oxide-core`
([`core/`](../../core/README.md)). M3 and M4 record the arithmetic, call and
closure probes and the loop's size; M5 runs the fixed probes through the
`oxide-run` driver and the baseline's runner; M6 repeats the full tables of
section 9.

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
