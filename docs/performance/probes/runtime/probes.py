#!/usr/bin/env python3
"""Per-iteration Ir and per-function self-cost diffs for the runtime items 2-4 probes.

Every probe is a `work(n)` loop with the same shape as `empty_loop` and a checked
result. Each probe runs under Callgrind at N, 2N and 4N; the slope (2N - N) / N is
the per-iteration Ir and (4N - 2N) / 2N checks linearity. Self cost per function
is diffed the same way (2N - N), which stays meaningful on aarch64 where inlining
makes the caller-chain phase split of `allocation/ledger.py` unreliable. "Net"
values subtract the matching `empty_loop` figure.

With `--v8`, fixed-iteration V8-v7 Richards/DeltaBlue/NavierStokes bodies (driver
from `scripts/benchmark/profile_v8.fixed_driver`) also run under Callgrind with
cache simulation for total Ir/Dw and the self-Ir share of reference counting.
"""
import argparse
import collections
import concurrent.futures
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "allocation"))
sys.path.insert(0, str(HERE.parents[3] / "scripts" / "benchmark"))
import ledger  # noqa: E402  (existing empty_loop/object_literal/constructor probes)

# Items 2-4 probes (runtime-bc-plan.md §5). `i` equals n after the loop.
NEW_PROBES = {
    "s=o": ("function work(n){ var o={x:1}; var s; for (var i=0;i<n;i++){ s=o; } return s.x+i; }\n",
            lambda n: n + 1),
    "s=o.x": ("function work(n){ var o={x:1}; var s; for (var i=0;i<n;i++){ s=o.x; } return s+i; }\n",
              lambda n: n + 1),
    "o.x=i": ("function work(n){ var o={x:0}; for (var i=0;i<n;i++){ o.x=i; } return o.x; }\n",
              lambda n: n - 1),
    "o.x=p": ("function work(n){ var o={x:null}, p={y:1}; for (var i=0;i<n;i++){ o.x=p; } return o.x.y+i; }\n",
              lambda n: n + 1),
    "a[i&1023]=i": ("function work(n){ var a=[]; for (var k=0;k<1024;k++) a[k]=0;\n"
                    "  for (var i=0;i<n;i++){ a[i&1023]=i; } return a[(n-1)&1023]; }\n",
                    lambda n: n - 1),
    "a[i&15]=p": ("function work(n){ var a=[], p={y:1}; for (var k=0;k<16;k++) a[k]=null;\n"
                  "  for (var i=0;i<n;i++){ a[i&15]=p; } return a[3].y+i; }\n",
                  lambda n: n + 1),
    "f(i)": ("function f(x){ return x; }\n"
             "function work(n){ var s; for (var i=0;i<n;i++){ s=f(i); } return s; }\n",
             lambda n: n - 1),
    "o.m(i)": ("function work(n){ var o={m:function(x){ return x; }}; var s;\n"
               "  for (var i=0;i<n;i++){ s=o.m(i); } return s; }\n",
               lambda n: n - 1),
    "g(p)": ("function g(x){ return x; }\n"
             "function work(n){ var p={y:1}; var s; for (var i=0;i<n;i++){ s=g(p); } return s.y+i; }\n",
             lambda n: n + 1),
    "new E()": ("function E(){}\n"
                "function work(n){ var s; for (var i=0;i<n;i++){ s=new E(); } return (s instanceof E) ? i : -1; }\n",
                lambda n: n),
}
PROBES = {name: (template.replace("print(work(N));\n", ""), ledger.EXPECTED[name])
          for name, template in ledger.PROBES.items()}
PROBES.update(NEW_PROBES)

# Reference counting functions reported individually (matched on the last path segment).
REFCOUNT = ("copy_reference_in_state", "dup_jsvalue", "retain_raw_root", "retain_object", "retain_raw",
            "retain_string_fast", "retain_string", "retain_string_shared", "retain_string_handle",
            "release_jsvalue", "release_heap_reference", "try_release_nonfinal",
            "validate_slot_identity", "release_owned_jsvalue")
V8_CASES = ("richards", "deltablue", "navier-stokes")


def leaf(name):
    """Last path segment of a demangled Rust symbol, without hash or generics."""
    name = re.sub(r"::h[0-9a-f]{16}$", "", name)
    depth, cut = 0, 0
    for index, char in enumerate(name):
        depth += char == "<"
        depth -= char == ">"
        if depth == 0 and name.startswith("::", index):
            cut = index + 2
    return re.sub(r"<.*", "", name[cut:])


def parse_callgrind(path: Path):
    """Self cost per function for every event, plus the file totals."""
    names, current, in_call = {}, None, False
    events, costs, totals = [], collections.defaultdict(lambda: collections.Counter()), {}
    for raw in path.read_text(errors="replace").splitlines():
        if not raw:
            continue
        if raw.startswith("events:"):
            events = raw.split()[1:]
            continue
        if raw.startswith(("summary:", "totals:")):
            totals = dict(zip(events, map(int, raw.split()[1:])))
            continue
        if raw.startswith(("fn=", "cfn=")):
            match = re.match(r"c?fn=\((\d+)\)(?: (.*))?", raw)
            if match and match.group(2):
                names[match.group(1)] = match.group(2)
            if raw.startswith("fn="):
                current = names[match.group(1)]
            continue
        if raw.startswith("calls="):
            in_call = True
            continue
        if raw[0] in "0123456789+-*":
            if in_call:
                in_call = False
                continue
            parts = raw.split()[1:]
            for event, value in zip(events, parts):
                costs[current][event] += int(value)
    return events, totals, costs


def callgrind(qjs, script, out, cache_sim=False):
    command = ["valgrind", "--tool=callgrind", f"--callgrind-out-file={out}",
               f"--cache-sim={'yes' if cache_sim else 'no'}", qjs, str(script)]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode:
        sys.exit(f"callgrind failed for {script}: {result.stderr[-600:]}")
    return result.stdout


def measure_probe(qjs, name, n, out: Path):
    template, expected = PROBES[name]
    slug = re.sub(r"[^A-Za-z0-9]+", "_", name).strip("_")
    runs = {}
    for multiple in (1, 2, 4):
        size = n * multiple
        script = out / f"{slug}_{size}.js"
        script.write_text(template + f"print(work({size}));\n")
        data = out / f"{slug}_{size}.callgrind"
        stdout = callgrind(qjs, script, data).strip()
        if stdout != str(expected(size)):
            sys.exit(f"{name}: unexpected output {stdout!r}, expected {expected(size)!r}")
        _, totals, costs = parse_callgrind(data)
        runs[multiple] = (totals["Ir"], {fn: c["Ir"] for fn, c in costs.items()})
    functions = collections.Counter()
    for fn in set(runs[1][1]) | set(runs[2][1]):
        delta = (runs[2][1].get(fn, 0) - runs[1][1].get(fn, 0)) / n
        if abs(delta) >= 0.01:
            functions[fn] = delta
    return {"source": template, "per_iteration": (runs[2][0] - runs[1][0]) / n,
            "per_iteration_2n_4n": (runs[4][0] - runs[2][0]) / (2 * n),
            "functions": dict(sorted(functions.items(), key=lambda kv: -abs(kv[1])))}


def by_leaf(functions, names):
    found = collections.Counter()
    for fn, value in functions.items():
        if leaf(fn) in names:
            found[leaf(fn)] += value
    return {name: round(found[name], 2) for name in names if name in found}


def finish_probes(report, top):
    base = report["probes"]["empty_loop"]
    for name, entry in report["probes"].items():
        entry["net_per_iteration"] = entry["per_iteration"] - base["per_iteration"]
        net = collections.Counter(entry["functions"])
        net.subtract(base["functions"])
        entry["top_self"] = [[fn, round(v, 2)] for fn, v in
                             sorted(entry["functions"].items(), key=lambda kv: -kv[1])[:top]]
        entry["top_net_self"] = [[fn, round(v, 2)] for fn, v in
                                 sorted(net.items(), key=lambda kv: -abs(kv[1]))[:top] if abs(v) >= 0.5]
        entry["refcount_self"] = by_leaf(entry["functions"], REFCOUNT)
        entry["refcount_self_total"] = round(sum(entry["refcount_self"].values()), 2)


def measure_v8(qjs, v8_root: Path, case, iterations, out: Path, top):
    import profile_v8
    code = v8_root / "v8-v7" if (v8_root / "v8-v7").is_dir() else v8_root
    body = (code / f"{case}.js").read_text()
    suite, benchmarks = profile_v8.declared_benchmarks(body, case)
    driver = profile_v8.fixed_driver(suite, benchmarks, iterations).decode()
    script = out / f"{case}.js"
    script.write_text((code / "base.js").read_text() + "\n" + body + "\n" + driver)
    data = out / f"{case}.callgrind"
    stdout = callgrind(qjs, script, data, cache_sim=True)
    marker = f"{profile_v8.MARKER}:{suite}:{len(benchmarks)}:{iterations}:{len(benchmarks) * iterations}"
    if stdout.strip() != marker:
        sys.exit(f"{case}: unexpected output {stdout[-300:]!r}")
    _, totals, costs = parse_callgrind(data)
    ir = {fn: c["Ir"] for fn, c in costs.items()}
    refcount = {k: v for k, v in by_leaf(ir, REFCOUNT).items()}
    mechanism = sum(v for fn, v in ir.items()
                    if ledger.mechanism_of(ledger.short(fn)) == "引用计数、释放与回收")
    return {"iterations": iterations, "Ir": totals["Ir"], "Dw": totals.get("Dw"), "Dr": totals.get("Dr"),
            "refcount_self": refcount,
            "refcount_share": sum(refcount.values()) / totals["Ir"],
            "refcount_mechanism_share": mechanism / totals["Ir"],
            "top_self": [[fn, v, round(v / totals["Ir"], 4)] for fn, v in
                         sorted(ir.items(), key=lambda kv: -kv[1])[:top]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--qjs", required=True, help="ordinary release qjs (with symbols)")
    parser.add_argument("--n", type=int, default=100_000)
    parser.add_argument("--probe", action="append", choices=list(PROBES),
                        help="limit to these probes (empty_loop is always included)")
    parser.add_argument("--v8", type=Path, help="V8-v7 source root; enables Richards/DeltaBlue/NavierStokes")
    parser.add_argument("--v8-iterations", type=int, default=10)
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--jobs", type=int, default=os.cpu_count())
    parser.add_argument("--output", required=True, help="new working directory for scripts and Callgrind files")
    parser.add_argument("--json", help="report path (default: OUTPUT/probes.json)")
    args = parser.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=False)
    names = ["empty_loop"] + [p for p in (args.probe or PROBES) if p != "empty_loop"]
    report = {"n": args.n, "qjs": args.qjs, "probes": {}, "v8": {}}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        probe_jobs = {name: pool.submit(measure_probe, args.qjs, name, args.n, out) for name in names}
        v8_jobs = {case: pool.submit(measure_v8, args.qjs, args.v8, case, args.v8_iterations, out, args.top)
                   for case in (V8_CASES if args.v8 else ())}
        for name, job in probe_jobs.items():
            report["probes"][name] = job.result()
        for case, job in v8_jobs.items():
            report["v8"][case] = job.result()
    finish_probes(report, args.top)
    Path(args.json or out / "probes.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    print(f"{'probe':<16}{'Ir/iter':>9}{'net':>9}{'2N→4N':>9}{'refcount':>10}")
    for name, entry in report["probes"].items():
        print(f"{name:<16}{entry['per_iteration']:9.0f}{entry['net_per_iteration']:9.0f}"
              f"{entry['per_iteration_2n_4n']:9.0f}{entry['refcount_self_total']:10.0f}")
    for name, entry in report["probes"].items():
        print(f"\n## {name}: net {entry['net_per_iteration']:.0f}")
        for fn, value in entry["top_self"][:12]:
            print(f"  {value:8.1f}  {leaf(fn)}")
    for case, entry in report["v8"].items():
        print(f"\n## {case} x{entry['iterations']}: Ir {entry['Ir']:,} Dw {entry['Dw']:,} "
              f"refcount {entry['refcount_share']:.2%} (mechanism {entry['refcount_mechanism_share']:.2%})")


if __name__ == "__main__":
    main()
