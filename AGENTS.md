# Agent guide

## Performance checks

Every change that reaches the engine runs the Sweep once before it is
reported as done. [Benchmarking](docs/performance/benchmarking.md) explains
the tiers and how to prepare the two binaries and the suite programs;
`python3 scripts/benchmark/bench.py` lists the tools.

### Which changes run what

**No benchmark** when every changed file is in this list:

- documentation: `*.md`, `docs/`, `apps/web/`, `LICENSES/`, `CHANGELOG.md`;
- tests and conformance data: `tests/`, `core/tests/`, `apps/cli/tests/`,
  `conformance/`, `dev-support/`, `compat/`, Rust files named `*tests.rs`,
  and `#[cfg(test)]` code;
- tooling: `scripts/` (a change to `scripts/benchmark/` runs its unit tests
  instead), `.github/`, `examples/`, `adapters/web/`.

**The Sweep** for every other change, including refactors that are expected
to change nothing:

```sh
python3 scripts/benchmark/bench.py sweep --base $BASE --candidate $CANDIDATE --output /tmp/oxide-sweep
```

It must exit 0: both [gates](docs/performance/principles.md#gates) pass and
every sample succeeds. Explain every fixed probe it lists above 1.02, and
every program whose instructions move by more than 1% in either direction.

**In addition**, by area:

| Changed | Also run |
| --- | --- |
| `src/source/`, `src/engine/ast/`, `src/engine/hir/`, `src/engine/mir/` (source text, parsing, lowering, compilation) | `compile corpus`, `compile probe` for base and candidate, `compile matrix` |
| `core/src/heap/`, `core/src/value.rs`, `core/src/shape.rs`, collector or allocation paths | `memory --engine base=$BASE --engine candidate=$CANDIDATE` |
| `core/src/builtins/collection.rs`, `array.rs`, `typed_array.rs`, `string.rs`, `object.rs`, property storage | `scaling` with the cases for the changed structure |
| `regexp/` | `run` on `v8-v7-regexp` and `octane-regexp` with `--linux-counters` |
| `Cargo.toml` profile or dependencies, `Cargo.lock`, `rust-toolchain.toml` | `run` on all 37 suite programs and `memory`, base against candidate |
| the README's or the site's QuickJS figures | the Publish run; QuickJS is measured only here |
| anything, when the pull request says "faster" or "no slower" | the Claim run: A/A, A/B, `paired --gate net-gain` or `--gate noninferior` |

The command line for each is in
[Benchmarking](docs/performance/benchmarking.md).

### How to run them

- Build both sides with `bench.py build`: the base from its own worktree
  (`git worktree add --detach /tmp/oxide-base <base>`), the candidate from the
  committed change. Same toolchain, same profile.
- Measure serially. Nothing else runs during a measurement: no cargo builds or
  tests, no second benchmark, no parallel agents or background jobs.
- Retired instructions decide; time is reported beside them with its A/A
  range. Callgrind counts and instruction counts are never stated as speed.
- Fix repetitions and order before running; report a missed gate as missed.
- Write results to a new directory under `target/` or outside the repository
  and keep them out of commits.
- A failed sample is a correctness bug until shown otherwise; read its raw
  stdout/stderr in the output directory.

### What to report

The tier, the commands as run, base and candidate commits, the Sweep's
`summary.md` (and any other table or `paired` output), and one line per
explained instruction change.

## Benchmark tooling

`scripts/benchmark/bench.py` is the only entry point; each command lives in
`scripts/benchmark/oxbench/`. A new tool is a new command there with tests in
`scripts/benchmark/tests/` and a section in `scripts/benchmark/README.md`.
Run `python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'` after
changing it.
