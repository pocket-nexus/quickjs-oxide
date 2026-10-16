#!/usr/bin/env python3
"""Run the new core's driver under its stress collector (new-core phase 2,
section 6).

Generates the fixed probes at smoke size and runs each through
`oxide-run --gc-stress=N`, comparing stdout with the manifest; then runs
every program of the two-core corpus (`core/tests/corpus`) with and without
stress and requires the same stdout and exit status. `oxide-run` must be
built with the `new-core-gc-stress` feature:

    cargo build -p quickjs-oxide-cli --features new-core-gc-stress --bin oxide-run
    python3 scripts/checks/check-new-core-stress.py --oxide-run target/debug/oxide-run
"""
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def run(oxide_run: str, path: Path, stress: int | None) -> subprocess.CompletedProcess:
    command = [oxide_run]
    if stress is not None:
        command.append(f"--gc-stress={stress}")
    command.append(str(path))
    return subprocess.run(command, capture_output=True, text=True, timeout=600)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--oxide-run", required=True)
    parser.add_argument("--interval", type=int, default=1)
    args = parser.parse_args()
    failures = []
    with tempfile.TemporaryDirectory() as directory:
        subprocess.run(
            [sys.executable, str(ROOT / "docs/performance/probes/build_fixed_matrix.py"),
             "--smoke", "--output", directory],
            check=True, capture_output=True)
        manifest = json.loads((Path(directory) / "manifest.json").read_text())
        workloads = manifest["metadata"]["workloads"]["workloads"]
        for workload in workloads:
            result = run(args.oxide_run, Path(workload["path"]), args.interval)
            if result.returncode != 0 or result.stdout != workload["expected"]:
                failures.append(f"probe {workload['case']}: status {result.returncode}, "
                                f"stdout {result.stdout!r}, stderr {result.stderr!r}")
    corpus = sorted((ROOT / "core/tests/corpus").glob("*.js"))
    for path in corpus:
        plain = run(args.oxide_run, path, None)
        stressed = run(args.oxide_run, path, args.interval)
        if (plain.returncode, plain.stdout) != (stressed.returncode, stressed.stdout):
            failures.append(f"corpus {path.name}: differs under stress: {stressed.stderr!r}")
        elif plain.returncode != 0:
            failures.append(f"corpus {path.name}: status {plain.returncode}: {plain.stderr!r}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"new-core stress: {len(workloads)} probes and {len(corpus)} corpus programs "
          f"pass at --gc-stress={args.interval}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
