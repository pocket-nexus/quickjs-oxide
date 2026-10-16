# New core, phase 2: results

This page records the measurements of [phase 2](new-core-phase2.md) of the new
core (#122) as its milestones land: register bytecode, the lowering from the
compiler's stack code, and the main loop in `oxide-core`
([`core/`](../../core/README.md)). M3 and M4 record the arithmetic, call and
closure probes and the loop's size; M6 repeats the full tables of section 9
with the driver and the fixed runner.

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
