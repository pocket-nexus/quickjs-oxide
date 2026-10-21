#!/usr/bin/env python3
"""The benchmark suite programs at one run() per benchmark under the stress
collector.

Runs every workload (or those starting with `--prefix`) of a suite built with
`scripts/benchmark/bench.py suite build --pilot 1` on `qjs
--gc-stress=1` (an `qjs` built with the `gc-stress` feature)
and compares its stdout with the manifest's expected line.

    python3 scripts/checks/check-suite-gc-stress.py --qjs PATH --suite DIR
"""
import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

# Programs whose live set makes a collection at every allocation take hours.
# Splay's one run() under --gc-stress=1 does not finish in 55 minutes; at 100
# it takes about 15. Every program that takes more than 20 minutes at
# interval 1 (or does not finish in 30) runs at 100.
INTERVALS = {
    "v8-v7-earley-boyer": "100",
    "v8-v7-splay": "100",
    "octane-splay": "100",
    "octane-pdfjs": "100",
    "octane-typescript": "100",
    "awfy-DeltaBlue": "100",
    "awfy-CD": "100",
    "awfy-Havlak": "100",
    "awfy-Storage": "100",
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--qjs", required=True, type=Path)
    parser.add_argument("--suite", required=True, type=Path, help="bench.py suite build --pilot 1 output")
    parser.add_argument("--prefix", default="")
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
        interval = INTERVALS.get(workload["case"], args.gc_stress) if args.gc_stress == "1" else args.gc_stress
        start = time.monotonic()
        try:
            result = subprocess.run(
                [str(args.qjs), f"--gc-stress={interval}", str(path)],
                capture_output=True, text=True, timeout=args.timeout)
            stdout, detail = result.stdout, result.stderr.strip()[-400:]
        except subprocess.TimeoutExpired:
            stdout, detail = "", f"timed out after {args.timeout}s"
        elapsed = time.monotonic() - start
        ok = stdout == workload["expected"]
        failures += not ok
        print(f"{workload['case']:28} {'ok' if ok else 'FAIL':4} {elapsed:8.1f}s  --gc-stress={interval}")
        if not ok:
            print(f"  expected {workload['expected']!r}\n  got      {stdout[-200:]!r}\n  {detail}")
    if failures:
        sys.exit(f"suite gc stress: {failures} of {len(workloads)} programs failed under --gc-stress={args.gc_stress}")
    print(f"suite gc stress: {len(workloads)} programs pass at --gc-stress={args.gc_stress}")


if __name__ == "__main__":
    main()
