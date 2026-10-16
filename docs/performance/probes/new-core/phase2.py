#!/usr/bin/env python3
"""New core phase 2, M3/M4: per-iteration instruction slopes of both cores.

Runs each probe at N and 2N on the new core and on the current core through
the root crate's measurement hook (`run_probe_file`, an ignored test in
src/engine/compiler/register/run_tests.rs), under `perf stat -e
instructions:u`, pinned to one CPU, and reports the per-iteration slope
(I(2N) - I(N)) / N, the median over the repeats. Startup, compilation and
lowering cancel in the slope; nothing else is subtracted. Both cores run
from the same release test binary, so they are built identically.

    cargo test --release --lib --no-run   # prints the binary path
    python3 docs/performance/probes/new-core/phase2.py \\
        --binary target/release/deps/quickjs_oxide-HASH --cpu 2 --repeat 3

Until the M5 driver exists this replaces the fixed runner for the new core;
M6 repeats the measurement on the reference machine.
"""
import argparse
import json
import re
import statistics
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
FIXED = ROOT / "docs/performance/probes/fixed"

# Section 9, Table A: the fixed probes (their own sources, N substituted).
TABLE_A = [
    "empty_loop", "numeric_local_loop", "local_read", "local_read_with_setup",
    "local_move", "argument_move", "call0", "prop_read", "prop_write",
    "array_read", "array_write", "tdz", "type_error", "bigint_64",
]

# Section 9, Table B: the phase 1 workloads as JavaScript.
TABLE_B = {
    "literal": "function work(n){ var s; for (var i=0;i<n;i++){ s={a:i,b:i}; } return s.a; }\n"
               "print(work(N));\n",
    "constructor": "function P(x){ this.a=x; this.b=x; }\n"
                   "function work(n){ var s; for (var i=0;i<n;i++){ s=new P(i); } return s.a; }\n"
                   "print(work(N));\n",
    "closure": "function work(n){ var s; for (var i=0;i<n;i++){ let j=i; s=()=>j; } return s(); }\n"
               "print(work(N));\n",
    "proto_read_3": "function A(){} A.prototype={x:3};\n"
                    "function B(){} B.prototype=new A();\n"
                    "function C(){} C.prototype=new B();\n"
                    "var o=new C();\n"
                    "function work(n){ var s=0; for (var i=0;i<n;i++){ s=s+o.x; } return s; }\n"
                    "print(work(N));\n",
}

# Iterations at N; the slow probes (exceptions, BigInt) run fewer.
ITERATIONS = {"tdz": 20000, "type_error": 20000, "bigint_64": 20000}
DEFAULT_N = 200000


def fixed_source(name: str) -> str:
    source = (FIXED / f"{name}.js").read_text()
    return re.sub(r"print\(work\((\d+)", "print(work(N", source)


def sources() -> dict:
    probes = {name: fixed_source(name) for name in TABLE_A}
    probes.update(TABLE_B)
    return probes


def instructions(binary: str, cpu: int, core: str, path: Path) -> int:
    command = [
        "perf", "stat", "-x,", "-e", "instructions:u",
        "taskset", "-c", str(cpu), binary,
        "--exact", "engine::compiler::register::run_tests::run_probe_file",
        "--ignored", "--nocapture", "--test-threads=1",
    ]
    env = {"PROBE_FILE": str(path), "PROBE_CORE": core, "PATH": "/usr/bin:/bin"}
    result = subprocess.run(command, env=env, capture_output=True, text=True, check=True)
    for line in result.stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith("instructions"):
            return int(fields[0])
    raise RuntimeError(f"no instruction count: {result.stderr}")


def slope(binary, cpu, repeat, core, source, n, directory):
    counts = {}
    for size in (n, 2 * n):
        path = Path(directory) / f"probe_{size}.js"
        path.write_text(source.replace("work(N", f"work({size}"))
        counts[size] = statistics.median(
            instructions(binary, cpu, core, path) for _ in range(repeat))
    return (counts[2 * n] - counts[n]) / n


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--output")
    args = parser.parse_args()
    rows = {}
    with tempfile.TemporaryDirectory() as directory:
        for name, source in sources().items():
            if args.only and name not in args.only:
                continue
            n = ITERATIONS.get(name, DEFAULT_N)
            row = {}
            for core in ("current", "new"):
                row[core] = slope(args.binary, args.cpu, args.repeat, core, source, n, directory)
            row["ratio"] = row["current"] / row["new"] if row["new"] else None
            rows[name] = row
            print(f"| `{name}` | {row['current']:,.0f} | {row['new']:,.0f} | "
                  f"{row['ratio']:.1f}x |", flush=True)
    if args.output:
        Path(args.output).write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    main()
