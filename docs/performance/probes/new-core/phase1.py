#!/usr/bin/env python3
"""New core phase 1, M5: per-iteration instruction slopes.

Runs each new-core Rust workload (core/examples/workloads.rs) and its
current-core JavaScript counterpart at N and 2N under `perf stat -e
instructions:u`, pinned to one CPU, and reports the per-iteration slope
(I(2N) - I(N)) / N, the median over the repeats. Setup cost cancels in the
slope. The empty loop of each engine is subtracted, so the current core's
numbers exclude loop dispatch but include whatever the probe's own bytecode
does (the constructor call, the closure instantiation).

    python3 docs/performance/probes/new-core/phase1.py \
        --workloads target/core/release/examples/workloads \
        --qjs target/release/qjs --cpu 2 --repeat 5 --output OUT.json
"""
import argparse
import json
import statistics
import subprocess
import tempfile
from pathlib import Path

JS = {
    "empty": "function work(n){ var s; for (var i=0;i<n;i++){ s=i; } return s; }\n"
             "print(work(N));\n",
    "constructor": "function P(x){ this.a=x; this.b=x; this.c=x; this.d=x; }\n"
                   "function work(n){ var s; for (var i=0;i<n;i++){ s=new P(i); } return s.a; }\n"
                   "print(work(N));\n",
    "literal": "function work(n){ var s; for (var i=0;i<n;i++){ s={a:i,b:i,c:i,d:i}; } return s.a; }\n"
               "print(work(N));\n",
    # An arrow function: like the Rust workload, it has no `prototype` object.
    "closures": "function work(n){ var s; for (var i=0;i<n;i++){ let j=i; s=()=>j; } return s(); }\n"
                "print(work(N));\n",
    "proto-read": "var o=Object.create(Object.create(Object.create({x:3})));\n"
                  "function work(n){ var s=0; for (var i=0;i<n;i++){ s=s+o.x; } return s; }\n"
                  "print(work(N));\n",
}

# new-core workload -> current-core probe
PAIRS = [
    ("alloc-only", None),
    ("constructor", "constructor"),
    ("literal", "literal"),
    ("closures", "closures"),
    ("proto-read", "proto-read"),
    ("proto-read-cached", "proto-read"),
    ("churn", None),
]


def instructions(command, cpu):
    result = subprocess.run(
        ["perf", "stat", "-x,", "-e", "instructions:u", "--", "taskset", "-c", str(cpu), *command],
        capture_output=True, text=True, check=True)
    for line in result.stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith("instructions"):
            return int(fields[0])
    raise RuntimeError(result.stderr)


def slope(make_command, n, cpu, repeat):
    samples = []
    for _ in range(repeat):
        low = instructions(make_command(n), cpu)
        high = instructions(make_command(2 * n), cpu)
        samples.append((high - low) / n)
    return statistics.median(samples), samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--workloads", required=True)
    parser.add_argument("--qjs", required=True)
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--new-n", type=int, default=1_000_000)
    parser.add_argument("--old-n", type=int, default=200_000)
    parser.add_argument("--output")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        def js(name):
            def make(n):
                path = scratch / f"{name}-{n}.js"
                path.write_text(JS[name].replace("N", str(n)))
                return [args.qjs, str(path)]
            return make

        def rust(name):
            return lambda n: [args.workloads, name, str(n)]

        new_empty, new_empty_samples = slope(rust("empty"), args.new_n, args.cpu, args.repeat)
        old_empty, old_empty_samples = slope(js("empty"), args.old_n, args.cpu, args.repeat)
        rows = []
        for new_name, old_name in PAIRS:
            new, new_samples = slope(rust(new_name), args.new_n, args.cpu, args.repeat)
            row = {"workload": new_name, "new": new - new_empty, "new_samples": new_samples}
            if old_name:
                old, old_samples = slope(js(old_name), args.old_n, args.cpu, args.repeat)
                row.update(current=old_name, old=old - old_empty, old_samples=old_samples)
            rows.append(row)
            print(json.dumps({k: v for k, v in row.items() if not k.endswith("samples")}), flush=True)
    report = {"new_empty": new_empty, "old_empty": old_empty, "new_empty_samples": new_empty_samples,
              "old_empty_samples": old_empty_samples, "rows": rows,
              "new_n": args.new_n, "old_n": args.old_n, "cpu": args.cpu, "repeat": args.repeat}
    print(json.dumps({"new_empty": new_empty, "old_empty": old_empty}))
    if args.output:
        Path(args.output).write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
