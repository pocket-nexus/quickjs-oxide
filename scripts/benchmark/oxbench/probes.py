"""Per-iteration Ir and per-function self-cost diffs for allocation, binding, property, array, call and construct probes.

Every probe is a `work(n)` loop with the same shape as `empty_loop` and a checked
result. Each probe runs under Callgrind at N, 2N and 4N; the slope (2N - N) / N is
the per-iteration Ir and (4N - 2N) / 2N checks linearity. Self cost per function
is diffed the same way (2N - N). "Net" values subtract the matching
`empty_loop` figure.

With `--v8`, fixed-iteration V8-v7 Richards/DeltaBlue/NavierStokes bodies also
run under Callgrind with cache simulation for total Ir/Dw and the per-function
self Ir.
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

# The empty loop and the allocation probes, then binding, property, array, call
# and construct probes. `i` equals n after the loop.
PROBES = {
    "empty_loop": ("function work(n){ var s; for (var i=0;i<n;i++){ s=i; } return s; }\n",
                   lambda n: n - 1),
    "object_literal": ("function work(n){ var s; for (var i=0;i<n;i++){ s={a:i,b:i}; } return s.a; }\n",
                       lambda n: n - 1),
    "constructor": ("function P(x){ this.a=x; this.b=x; }\n"
                    "function work(n){ var s; for (var i=0;i<n;i++){ s=new P(i); } return s.a; }\n",
                    lambda n: n - 1),
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


# Fixed-iteration V8-v7 bodies: names and counts are checked against the source
# before running JS.
BENCHMARKS = {
    "richards": ("Richards", ("Richards",)),
    "deltablue": ("DeltaBlue", ("DeltaBlue",)),
    "crypto": ("Crypto", ("Encrypt", "Decrypt")),
    "raytrace": ("RayTrace", ("RayTrace",)),
    "earley-boyer": ("EarleyBoyer", ("Earley", "Boyer")),
    "regexp": ("RegExp", ("RegExp",)),
    "splay": ("Splay", ("Splay",)),
    "navier-stokes": ("NavierStokes", ("NavierStokes",)),
}
SUITE_DECL = re.compile(r"\bnew\s+BenchmarkSuite\s*\(\s*(['\"])([^'\"]+)\1")
BENCH_DECL = re.compile(r"\bnew\s+Benchmark\s*\(\s*(['\"])([^'\"]+)\1")
MARKER = "__oxide_v8_fixed_profile_complete__"


def declared_benchmarks(source_text, case):
    suite_names = [match[2] for match in SUITE_DECL.finditer(source_text)]
    benchmark_names = [match[2] for match in BENCH_DECL.finditer(source_text)]
    expected_suite, expected_benchmarks = BENCHMARKS[case]
    if suite_names != [expected_suite] or benchmark_names != list(expected_benchmarks):
        raise ValueError(f"{case}: source Benchmark declarations differ from pinned expectations")
    return expected_suite, expected_benchmarks


def fixed_driver(suite_name, benchmark_names, iterations):
    """Each fixed iteration invokes Setup, run, TearDown once per Benchmark."""
    suite_literal = json.dumps(suite_name)
    names_literal = json.dumps(list(benchmark_names))
    return f"""
// OXIDE DIAGNOSTIC DRIVER: fixed coverage only; no adaptive timing or V8 Score.
(function() {{
  var expectedSuite = {suite_literal};
  var expectedNames = {names_literal};
  var iterations = {iterations};
  var suites = BenchmarkSuite.suites;
  if (suites.length !== 1 || suites[0].name !== expectedSuite)
    throw new Error('diagnostic suite mismatch');
  var suite = suites[0];
  if (suite.benchmarks.length !== expectedNames.length)
    throw new Error('diagnostic benchmark count mismatch');
  var completed = 0;
  for (var i = 0; i < expectedNames.length; i++) {{
    var benchmark = suite.benchmarks[i];
    if (benchmark.name !== expectedNames[i])
      throw new Error('diagnostic benchmark name mismatch');
    for (var n = 0; n < iterations; n++) {{
      benchmark.Setup();
      benchmark.run();
      benchmark.TearDown();
      completed++;
    }}
  }}
  print('{MARKER}:' + suite.name + ':' + expectedNames.length + ':' + iterations + ':' + completed);
}})();
""".encode()


def measure_v8(qjs, v8_root: Path, case, iterations, out: Path, top):
    code = v8_root / "v8-v7" if (v8_root / "v8-v7").is_dir() else v8_root
    body = (code / f"{case}.js").read_text()
    suite, benchmarks = declared_benchmarks(body, case)
    driver = fixed_driver(suite, benchmarks, iterations).decode()
    script = out / f"{case}.js"
    script.write_text((code / "base.js").read_text() + "\n" + body + "\n" + driver)
    data = out / f"{case}.callgrind"
    stdout = callgrind(qjs, script, data, cache_sim=True)
    marker = f"{MARKER}:{suite}:{len(benchmarks)}:{iterations}:{len(benchmarks) * iterations}"
    if stdout.strip() != marker:
        sys.exit(f"{case}: unexpected output {stdout[-300:]!r}")
    _, totals, costs = parse_callgrind(data)
    ir = {fn: c["Ir"] for fn, c in costs.items()}
    return {"iterations": iterations, "Ir": totals["Ir"], "Dw": totals.get("Dw"), "Dr": totals.get("Dr"),
            "top_self": [[fn, v, round(v / totals["Ir"], 4)] for fn, v in
                         sorted(ir.items(), key=lambda kv: -kv[1])[:top]],
            "self": {fn: v for fn, v in ir.items() if v}}


def v8_cases(args):
    cases = []
    for spec in args.v8_case or V8_CASES:
        case, _, iterations = spec.partition("=")
        if case not in BENCHMARKS:
            sys.exit(f"unknown V8 case {case!r}")
        cases.append((case, int(iterations) if iterations else args.v8_iterations))
    return cases


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py probes", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--qjs", required=True, help="ordinary release qjs (with symbols)")
    parser.add_argument("--n", type=int, default=100_000)
    parser.add_argument("--probe", action="append", choices=list(PROBES),
                        help="limit to these probes (empty_loop is always included)")
    parser.add_argument("--v8", type=Path, help="V8-v7 source root; enables the V8 cases")
    parser.add_argument("--v8-iterations", type=int, default=10)
    parser.add_argument("--v8-case", action="append", metavar="CASE[=ITERATIONS]",
                        help="V8 case to profile, optionally with its own iteration count "
                             "(default: Richards, DeltaBlue and NavierStokes)")
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--jobs", type=int, default=os.cpu_count())
    parser.add_argument("--output", required=True, help="new working directory for scripts and Callgrind files")
    parser.add_argument("--json", help="report path (default: OUTPUT/probes.json)")
    args = parser.parse_args(argv)
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=False)
    names = ["empty_loop"] + [p for p in (args.probe or PROBES) if p != "empty_loop"]
    report = {"n": args.n, "qjs": args.qjs, "probes": {}, "v8": {}}
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        probe_jobs = {name: pool.submit(measure_probe, args.qjs, name, args.n, out) for name in names}
        v8_jobs = {case: pool.submit(measure_v8, args.qjs, args.v8, case, iterations, out, args.top)
                   for case, iterations in (v8_cases(args) if args.v8 else ())}
        for name, job in probe_jobs.items():
            report["probes"][name] = job.result()
        for case, job in v8_jobs.items():
            report["v8"][case] = job.result()
    finish_probes(report, args.top)
    Path(args.json or out / "probes.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    print(f"{'probe':<16}{'Ir/iter':>9}{'net':>9}{'2N→4N':>9}")
    for name, entry in report["probes"].items():
        print(f"{name:<16}{entry['per_iteration']:9.0f}{entry['net_per_iteration']:9.0f}"
              f"{entry['per_iteration_2n_4n']:9.0f}")
    for name, entry in report["probes"].items():
        print(f"\n## {name}: net {entry['net_per_iteration']:.0f}")
        for fn, value in entry["top_self"][:12]:
            print(f"  {value:8.1f}  {leaf(fn)}")
    for case, entry in report["v8"].items():
        print(f"\n## {case} x{entry['iterations']}: Ir {entry['Ir']:,} Dw {entry['Dw']:,}")
