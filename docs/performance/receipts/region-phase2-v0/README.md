# Region phase V0 baseline

`baseline.json` freezes the initial integrated comparison at `90c85e3d`.
The plain-release CLI came from a separate, clean, detached worktree at that
commit. `build.json`, the compiler log, and the exact build-tool snapshots are
retained alongside it; absolute paths in the original receipt name the
measurement host and are not required to reproduce the build.

The RealWorld source pins, deterministic scenario, generated bundle hashes,
and exact expected outputs are recorded in `scripts/benchmark/realworld` and
in `baseline.json`. The ABBA baseline/baseline runs establish that all three
application programs complete with the expected output. Two samples per side
are a smoke and drift control, not evidence of a performance change or a
confidence interval. Later candidate receipts must identify their own build,
bundle, work, and measurement host; compare locally with their parent and
cumulatively with this source baseline.
