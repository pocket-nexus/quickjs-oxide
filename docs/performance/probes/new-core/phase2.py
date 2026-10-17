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

or, from M5 on, with any engine run as `PATH FILE` (each named column):

    python3 docs/performance/probes/new-core/phase2.py --engine quickjs=$QJS \\
        --engine new=target/release/qjs
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

# Phase 4, section 10, Table B: the probes of the new surface.
TABLE_PHASE4 = {
    "for_of_array": "var a=[1,2,3,4,5,6,7,8];\n"
                    "function work(n){ var s=0; for (var i=0;i<n;i++){ for (var x of a) s+=x; } return s; }\n"
                    "print(work(N));\n",
    "spread_call": "function f(a,b,c){ return a+b+c; } var args=[1,2,3];\n"
                   "function work(n){ var s=0; for (var i=0;i<n;i++){ s+=f(...args); } return s; }\n"
                   "print(work(N));\n",
    "class_method": "class C { constructor(){ this.v=1; } m(){ return this.v; } } var o=new C();\n"
                    "function work(n){ var s=0; for (var i=0;i<n;i++){ s+=o.m(); } return s; }\n"
                    "print(work(N));\n",
    "generator_next": "function* g(){ for (;;) yield 1; } var it=g();\n"
                      "function work(n){ var s=0; for (var i=0;i<n;i++){ s+=it.next().value; } return s; }\n"
                      "print(work(N));\n",
    "typed_write": "var t=new Float64Array(1024);\n"
                   "function work(n){ for (var i=0;i<n;i++){ t[i&1023]=i; } return t[0]; }\n"
                   "print(work(N));\n",
    "map_get": "var m=new Map(); for (var k=0;k<64;k++) m.set(k,k);\n"
               "function work(n){ var s=0; for (var i=0;i<n;i++){ s+=m.get(i&63); } return s; }\n"
               "print(work(N));\n",
    "string_build": "function work(n){ var s=''; for (var i=0;i<n;i++){ s+='x'; } return s.length; }\n"
                    "print(work(N));\n",
    "sort_cmp": "function cmp(x,y){ return x-y; }\n"
                "function work(n){ var s=0; for (var i=0;i<n;i++){ var a=[3,1,2,5,4]; a.sort(cmp); s+=a[0]; } return s; }\n"
                "print(work(N));\n",
}

# Iterations at N; the slow probes (exceptions, BigInt) run fewer.
ITERATIONS = {"tdz": 20000, "type_error": 20000, "bigint_64": 20000, "sort_cmp": 50000}
DEFAULT_N = 200000


def fixed_source(name: str) -> str:
    source = (FIXED / f"{name}.js").read_text()
    return re.sub(r"print\(work\((\d+)", "print(work(N", source)


def sources() -> dict:
    probes = {name: fixed_source(name) for name in TABLE_A}
    probes.update(TABLE_B)
    probes.update(TABLE_PHASE4)
    return probes


def instructions(command: list, env: dict, cpu: int, path: Path) -> int:
    """`instructions:u` of one run of `command` on `path`, pinned to `cpu`."""
    full = ["perf", "stat", "-x,", "-e", "instructions:u", "taskset", "-c", str(cpu),
            *[str(path) if part == "{file}" else part for part in command]]
    if "{file}" not in command:
        full.append(str(path))
    run_env = {"PATH": "/usr/bin:/bin", **{key: (str(path) if value == "{file}" else value)
                                           for key, value in env.items()}}
    result = subprocess.run(full, env=run_env, capture_output=True, text=True, check=True)
    for line in result.stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith("instructions"):
            return int(fields[0])
    raise RuntimeError(f"no instruction count: {result.stderr}")


def hook(binary: str, core: str) -> tuple:
    """The root crate's measurement hook running one core (M3/M4)."""
    command = [binary, "--exact", "engine::compiler::register::run_tests::run_probe_file",
               "--ignored", "--nocapture", "--test-threads=1"]
    return command, {"PROBE_FILE": "{file}", "PROBE_CORE": core}


def slope(engine, cpu, repeat, source, n, directory):
    command, env = engine
    counts = {}
    for size in (n, 2 * n):
        path = Path(directory) / f"probe_{size}.js"
        path.write_text(source.replace("work(N", f"work({size}"))
        counts[size] = statistics.median(
            instructions(command, env, cpu, path) for _ in range(repeat))
    return (counts[2 * n] - counts[n]) / n


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", help="the root crate's release test binary (both cores)")
    parser.add_argument("--engine", action="append", default=[],
                        help="NAME=PATH of a binary run as `PATH FILE` (QuickJS, qjs)")
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--only", nargs="*")
    parser.add_argument("--output")
    args = parser.parse_args()
    engines = {}
    if args.binary:
        engines["current"] = hook(args.binary, "current")
        engines["new"] = hook(args.binary, "new")
    for spec in args.engine:
        name, path = spec.split("=", 1)
        engines[name] = ([path], {})
    if not engines:
        parser.error("give --binary or --engine")
    rows = {}
    print("| Probe | " + " | ".join(engines) + " |", flush=True)
    with tempfile.TemporaryDirectory() as directory:
        for name, source in sources().items():
            if args.only and name not in args.only:
                continue
            n = ITERATIONS.get(name, DEFAULT_N)
            row = {engine: slope(engines[engine], args.cpu, args.repeat, source, n, directory)
                   for engine in engines}
            rows[name] = row
            print(f"| `{name}` | " + " | ".join(f"{row[e]:,.0f}" for e in engines) + " |",
                  flush=True)
    if args.output:
        Path(args.output).write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    main()
