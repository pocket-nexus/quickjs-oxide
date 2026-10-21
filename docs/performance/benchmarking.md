# Benchmarking

This page says which measurement answers which question, and gives the
standard command line for each. Every command is a subcommand of
`scripts/benchmark/bench.py`; the [benchmark tools reference](../../scripts/benchmark/README.md)
documents each one. The rules behind them are in
[Execution principles](principles.md#measurement) and
[Measurement method](measurement.md).

## Choose the measurement

| Question | Tier | Commands |
| --- | --- | --- |
| What did my change do to the engine, and may it merge? | Sweep | `sweep` |
| Which way is this edit moving things, while I iterate? | Look | `run` on a few programs, `suite table` |
| Is it really faster, or really no slower? | Claim | A/A and A/B `run`, then `paired` |
| How does the engine compare with QuickJS? | Publish | `run` on the 37 suite programs, `suite table`; `memory` |
| Where do the instructions go? | Explain | `probes`, `slopes`, `run --callgrind`; `qjs --ic-stats`, `--gc-stats` ([Diagnostics](../profiling.md)) |
| How does cost grow with size? | Explain | `scaling` |
| What does compilation cost? | Explain | `compile corpus`, `compile probes`, `compile matrix` |
| How much of a time difference is code layout? | Explain | `relink`, then `run` over the variants |

Each tier is a different kind of evidence, not a smaller version of the next
one. The Sweep is the default check of every change that reaches the engine;
[AGENTS.md](../../AGENTS.md) lists which changes need it and what they add. A
Look result guides development; only a Claim result supports "faster" in a
pull request.

## How to read the numbers

- **Instructions decide; time is reported beside them.** `instructions:u` is
  deterministic for a binary and a workload. Time also moves 3–7% from code
  layout alone when unrelated code changes. A time change with flat
  instructions is layout.
- **A time difference counts only above its A/A range.** The same binary run
  twice in the same schedule gives the noise; `paired` compares the
  candidate's interval with it.
- **Callgrind counts are not speed.** The interpreter's hot code exceeds L1i on
  the x86 reference machine. Fewer instructions can still run slower when the
  change raises instruction-cache misses; read `cycles:u` beside
  `instructions:u` before a Claim run.
- **Every number names its source.** Base and candidate commits, binary
  SHA-256 (in `metadata.json`), workload hashes, machine, and the exact
  command.
- **Duration matches the question.** A timed workload runs long enough that
  process startup is negligible and short enough to repeat many times. The
  suite programs run for about a second each on QuickJS and the fixed probes
  for about 300 ms each on quickjs-oxide. A new timed workload is sized the
  same way.

## Rules

1. Measure release builds, both sides with the same toolchain and profile.
   Debug, `gc-stress` and `ic-stats` builds are for correctness and
   diagnostics, never for time or instruction comparisons.
2. Run measurements serially on an otherwise idle machine: no builds, tests,
   other benchmarks or parallel agents during a run. Pin with `--cpu`.
3. Fix the sample count and order before measuring. A result that misses a
   gate stays a miss; rerunning or adding samples until it passes is not
   evidence.
4. Write every output to a new directory under `target/` or outside the
   repository. Results, raw samples and logs stay out of commits.
5. Change a frozen workload count or source only to start a new baseline, and
   say so.
6. When a sample fails, read its stdout/stderr in the output directory; a
   failed sample is a correctness question first.

## Prepare

**Binaries.** Commit the change first; `build` refuses a dirty worktree. Build
the base from its own worktree:

```sh
git worktree add --detach /tmp/oxide-base origin/main
python3 scripts/benchmark/bench.py build --repo /tmp/oxide-base --plain-target /tmp/oxide-base-target
python3 scripts/benchmark/bench.py build
BASE=/tmp/oxide-base-target/release/qjs
CANDIDATE=$PWD/target/release/qjs
```

For a Look run, `cargo build --release -p quickjs-oxide-cli --bin qjs` in each
worktree is enough; Sweep, Claim and Publish runs use `build` for both sides,
so the two binaries differ only in source.

**Suite programs.** Check out the three pinned suites outside the repository
and generate the 37 programs once per series:

```sh
fetch() { git init -q "$1"; git -C "$1" fetch -q --depth 1 "$2" "$3"; git -C "$1" checkout -q FETCH_HEAD; }
fetch /tmp/bench-src/v8 https://github.com/ahaoboy/js-engine-benchmark.git 2034d98fc8c5f8044e186267593f5d5ea5232caf
fetch /tmp/bench-src/octane https://github.com/chromium/octane.git 570ad1ccfe86e3eecba0636c8f932ac08edec517
fetch /tmp/bench-src/awfy https://github.com/smarr/are-we-fast-yet.git 74306fec151070fd07157cefeacf19e7e0bcdc89
python3 scripts/benchmark/bench.py suite build --v8-source /tmp/bench-src/v8 \
  --octane-source /tmp/bench-src/octane --awfy-source /tmp/bench-src/awfy --output target/suite-matrix
V8=$(printf -- '--case v8-v7-%s ' richards deltablue crypto raytrace earley-boyer regexp splay navier-stokes)   # for Claim
```

**Host.** Linux counters need `perf` and `perf_event_paranoid` of 2 or lower.
Explain-tier commands need Valgrind. Only the Publish run measures QuickJS;
`scripts/quickjs/build-quickjs-oracle.sh` builds it.

## Sweep

Every fixed probe and the eight V8 v7 programs, base against candidate, four
repetitions each in ABBA order with `instructions:u`, about two minutes:

```sh
python3 scripts/benchmark/bench.py sweep --base $BASE --candidate $CANDIDATE --output /tmp/oxide-sweep
```

It prints both tables and evaluates the two [gates](principles.md#gates):
each V8 v7 program's `candidate / base instr` at most 1.02, and the
candidate's `run_at` below 16,384 bytes. A fixed probe above 1.02 is listed for
explanation. The exit status is 1 when a sample or a gate fails. Read the
instruction column first; a time ratio with flat instructions is layout, and
the time column alone supports no conclusion. `summary.md` in the output
directory is the result to report.

## Look

A few programs, a few repetitions, instructions and time together, for
iterating on one change:

```sh
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --case v8-v7-richards --case v8-v7-deltablue --case awfy-Havlak \
  --engine base=$BASE --engine candidate=$CANDIDATE \
  --repeat 2 --order abba --cpu 2 --linux-counters --output /tmp/oxide-look-1
python3 scripts/benchmark/bench.py suite table /tmp/oxide-look-1/results.json --reference base --instructions
```

Choose the programs that exercise the changed path. The fixed probes
(`--manifest docs/performance/probes/fixed/manifest.json --workload-dir
docs/performance/probes/fixed`) isolate single operations.

## Area checks

The additional runs [AGENTS.md](../../AGENTS.md) requires for some areas, base
against candidate:

```sh
# Compilation (src/source/, src/engine/compiler/)
python3 scripts/benchmark/bench.py compile corpus --output /tmp/oxide-corpus
BASE_PROBE=$(python3 scripts/benchmark/bench.py compile probe --repo /tmp/oxide-base --output /tmp/oxide-probe-base | tail -1)
CANDIDATE_PROBE=$(python3 scripts/benchmark/bench.py compile probe --repo . --output /tmp/oxide-probe-candidate | tail -1)
python3 scripts/benchmark/bench.py compile matrix --corpus /tmp/oxide-corpus --repeat 5 \
  --engine base=$BASE_PROBE --engine candidate=$CANDIDATE_PROBE --output /tmp/oxide-compile

# Memory (heap, values, shapes, collector, allocation)
python3 scripts/benchmark/bench.py memory --engine base=$BASE --engine candidate=$CANDIDATE \
  --markdown /tmp/oxide-memory.md

# Data structures (collections, arrays, typed arrays, strings, property storage)
python3 scripts/benchmark/bench.py scaling --engine base=$BASE --engine candidate=$CANDIDATE \
  --case map-int --case set --repeat 5 --output /tmp/oxide-scaling

# RegExp (regexp/)
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --case v8-v7-regexp --case octane-regexp --engine base=$BASE --engine candidate=$CANDIDATE \
  --repeat 4 --order abba --cpu 2 --linux-counters --output /tmp/oxide-regexp
```

`compile matrix` reports the candidate/base compile-time ratio per corpus
file. `memory` reports each program's peak RSS for both binaries, the
candidate/base ratio and the heap's figures. `scaling --help` lists the cases; choose those of
the changed structure.

## Claim

A same-binary A/A run first, then the A/B run with identical settings, then
the gate. Choose the cases before the A/A run, normally the eight V8 v7
programs plus the programs the change targets (`${=V8}` splits the variable
in zsh; use `$V8` in bash):

```sh
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json ${=V8} \
  --engine base=$BASE --engine candidate=$BASE \
  --repeat 8 --order abba-baab --cpu 2 --output /tmp/oxide-aa
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json ${=V8} \
  --engine base=$BASE --engine candidate=$CANDIDATE \
  --repeat 8 --order abba-baab --cpu 2 --output /tmp/oxide-ab
python3 scripts/benchmark/bench.py paired --results /tmp/oxide-ab/results.json \
  --baseline base --candidate candidate --aa-results /tmp/oxide-aa/results.json \
  --gate net-gain --output /tmp/oxide-ab/paired.json
```

`--gate net-gain` supports "faster": every case's interval clears its A/A
noise. `--gate noninferior` supports "no slower" for refactors and
correctness fixes: every case's interval stays above −1%. A gain within the
3–7% layout range is checked against `relink` variants of both binaries before
it is attributed to the code.

## Publish

The cross-engine table behind the README and the site's performance page:

```sh
python3 scripts/benchmark/bench.py run --manifest target/suite-matrix/manifest.json \
  --engine quickjs=target/oracle/quickjs-2026-06-04/qjs --engine oxide=$CANDIDATE \
  --repeat 5 --cpu 2 --linux-counters --timeout 900 --output /tmp/oxide-publish
python3 scripts/benchmark/bench.py suite table /tmp/oxide-publish/results.json --reference quickjs --instructions
python3 scripts/benchmark/bench.py memory --engine quickjs=target/oracle/quickjs-2026-06-04/qjs \
  --engine oxide=$CANDIDATE --markdown /tmp/oxide-publish/memory.md
```

## Report

A pull request that makes a performance statement includes the tier, the
commands as run, base and candidate commits, the `suite table` or `paired`
Markdown output, and the A/A range for time. Raw results stay outside the
repository.
