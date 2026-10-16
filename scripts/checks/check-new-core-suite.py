#!/usr/bin/env python3
"""New core, phase 3 section 7: the V8 v7 programs at one run() per
benchmark under the stress collector.

Runs every `v8-v7-*` workload of a suite built with
`scripts/benchmark/suite_matrix.py build --pilot 1` on `oxide-run
--gc-stress=1` (an `oxide-run` built with the `new-core-gc-stress` feature)
and compares its stdout with the manifest's expected line.

    python3 scripts/checks/check-new-core-suite.py --oxide-run PATH --suite DIR
"""
import argparse
import json
import subprocess
import sys
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oxide-run", required=True, type=Path)
    parser.add_argument("--suite", required=True, type=Path, help="suite_matrix.py build --pilot 1 output")
    parser.add_argument("--prefix", default="v8-v7-")
    parser.add_argument("--gc-stress", default="1")
    parser.add_argument("--timeout", type=int, default=3600, help="seconds per program")
    args = parser.parse_args()

    manifest = json.loads((args.suite / "manifest.json").read_text())
    workloads = manifest["metadata"]["workloads"]["workloads"]
    workloads = [w for w in workloads if w["case"].startswith(args.prefix)]
    if not workloads:
        sys.exit(f"no {args.prefix}* workloads in {args.suite}")
    failures = 0
    for workload in workloads:
        path = args.suite / Path(workload["path"]).name
        start = time.monotonic()
        try:
            result = subprocess.run(
                [str(args.oxide_run), f"--gc-stress={args.gc_stress}", str(path)],
                capture_output=True, text=True, timeout=args.timeout)
            stdout, detail = result.stdout, result.stderr.strip()[-400:]
        except subprocess.TimeoutExpired:
            stdout, detail = "", f"timed out after {args.timeout}s"
        elapsed = time.monotonic() - start
        ok = stdout == workload["expected"]
        failures += not ok
        print(f"{workload['case']:28} {'ok' if ok else 'FAIL':4} {elapsed:8.1f}s")
        if not ok:
            print(f"  expected {workload['expected']!r}\n  got      {stdout[-200:]!r}\n  {detail}")
    if failures:
        sys.exit(f"new-core suite: {failures} of {len(workloads)} programs failed under --gc-stress={args.gc_stress}")
    print(f"new-core suite: {len(workloads)} programs pass at --gc-stress={args.gc_stress}")


if __name__ == "__main__":
    main()
