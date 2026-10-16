#!/usr/bin/env python3
"""Generate the fixed-work V8 v7, Octane and AWFY matrix, and tabulate its results.

`build` writes one self-contained program per benchmark item from pinned
upstream checkouts, plus a manifest that `fixed.py` replays. Every program
runs a frozen amount of work with no clock reads and prints one completion
line, so all engines execute identical guest work and stdout is exact.
`table` turns `fixed.py` results into the per-workload baseline table.
"""
import argparse
import json
from pathlib import Path
import re
import statistics
import subprocess

from run import V8_V7_SOURCE_COMMIT, digest

OCTANE_SOURCE_COMMIT = "570ad1ccfe86e3eecba0636c8f932ac08edec517"
AWFY_SOURCE_COMMIT = "74306fec151070fd07157cefeacf19e7e0bcdc89"
MARKER = "__oxide_suite_fixed_complete__"
PRELUDE = 'if (typeof print === "undefined") globalThis.print = (...a) => console.log(...a);\n'

# Item -> (source files after base.js, {Benchmark name: run() calls}).
# Counts are frozen so the run() calls of one item take about a second on
# QuickJS 2026-06-04 on the reference machine (Splay from instruction counts,
# its runs being too short to time); zlib, Typescript and Mandreel run once.
# Changing a count starts a new baseline.
V8_V7 = {
    "richards": (["richards.js"], {"Richards": 400}),
    "deltablue": (["deltablue.js"], {"DeltaBlue": 200}),
    "crypto": (["crypto.js"], {"Encrypt": 11, "Decrypt": 11}),
    "raytrace": (["raytrace.js"], {"RayTrace": 40}),
    "earley-boyer": (["earley-boyer.js"], {"Earley": 12, "Boyer": 12}),
    "regexp": (["regexp.js"], {"RegExp": 7}),
    "splay": (["splay.js"], {"Splay": 800}),
    "navier-stokes": (["navier-stokes.js"], {"NavierStokes": 20}),
}
OCTANE = {
    "richards": (["richards.js"], {"Richards": 400}),
    "deltablue": (["deltablue.js"], {"DeltaBlue": 200}),
    "crypto": (["crypto.js"], {"Encrypt": 11, "Decrypt": 11}),
    "raytrace": (["raytrace.js"], {"RayTrace": 40}),
    "earley-boyer": (["earley-boyer.js"], {"Earley": 12, "Boyer": 12}),
    "regexp": (["regexp.js"], {"RegExp": 7}),
    "splay": (["splay.js"], {"Splay": 800}),
    "navier-stokes": (["navier-stokes.js"], {"NavierStokes": 20}),
    "pdfjs": (["pdfjs.js"], {"PdfJS": 6}),
    "mandreel": (["mandreel.js"], {"Mandreel": 1}),
    "gameboy": (["gbemu-part1.js", "gbemu-part2.js"], {"Gameboy": 4}),
    "code-load": (["code-load.js"], {"CodeLoadClosure": 120, "CodeLoadJQuery": 120}),
    "box2d": (["box2d.js"], {"Box2D": 10}),
    "zlib": (["zlib.js", "zlib-data.js"], {"zlib": 1}),
    "typescript": (["typescript.js", "typescript-input.js", "typescript-compiler.js"], {"Typescript": 1}),
}
# The harness's default inner iterations: the only counts verifyResult accepts.
AWFY = {"DeltaBlue": 12000, "Richards": 100, "Json": 100, "CD": 250, "Havlak": 1500, "Bounce": 1500,
        "List": 1500, "Mandelbrot": 500, "NBody": 250000, "Permute": 1000, "Queens": 1000,
        "Sieve": 3000, "Storage": 1000, "Towers": 600}
AWFY_MODULES = ["som", "benchmark", "bounce", "cd", "deltablue", "havlak", "json", "list", "mandelbrot",
                "nbody", "permute", "queens", "richards", "sieve", "storage", "towers"]
# Emscripten's shell branch stores a reference to read(); the benchmark never calls it.
EXTRA = {("octane", "zlib"): 'if (typeof read === "undefined") globalThis.read = function () '
                             '{ throw new Error("read is not available"); };\n'}
REQUIRE_SHIM = """var __registry = {}, __cache = {};
function __define(name, factory) { __registry[name] = factory; }
function require(name) {
  if (__cache[name]) return __cache[name].exports;
  var module = { exports: {} }; __cache[name] = module;
  __registry[name](module, module.exports, require);
  return module.exports;
}
var process = { stdout: { write: function (s) { print("unexpected output: " + s); } } };
"""


def checkout(path, commit, required):
    path = Path(path).resolve()
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=path, capture_output=True, text=True)
    if head.returncode or head.stdout.strip() != commit:
        raise ValueError(f"{path} must be a checkout of {commit}")
    status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=path,
                            capture_output=True, text=True)
    if status.returncode or status.stdout.strip():
        raise ValueError(f"{path} has local changes")
    for name in required:
        if not (path / name).is_file():
            raise ValueError(f"{path} lacks {name}")
    return path


def suite_driver(suite, item, plan):
    """Setup once, run() a frozen number of times, TearDown once, per Benchmark.

    This is the Octane runner's per-benchmark sequence (RNG reset, Setup,
    repeated run, TearDown) without its clock or score.
    """
    return f"""
// OXIDE FIXED-WORK DRIVER: frozen run() counts; no clock reads and no Score.
(function () {{
  var plan = {json.dumps(plan)};
  var suites = BenchmarkSuite.suites, completed = [];
  for (var s = 0; s < suites.length; s++) {{
    for (var b = 0; b < suites[s].benchmarks.length; b++) {{
      var benchmark = suites[s].benchmarks[b];
      var runs = plan[benchmark.name];
      if (typeof runs !== "number") throw new Error("unplanned benchmark " + benchmark.name);
      if (BenchmarkSuite.ResetRNG) BenchmarkSuite.ResetRNG();
      benchmark.Setup();
      for (var n = 0; n < runs; n++) benchmark.run();
      benchmark.TearDown();
      completed.push(benchmark.name + "x" + runs);
    }}
  }}
  if (completed.length !== {len(plan)}) throw new Error("benchmark count mismatch");
  print("{MARKER}:{suite}:{item}:" + completed.join(","));
}})();
"""


def suite_expected(suite, item, plan):
    return f"{MARKER}:{suite}:{item}:" + ",".join(f"{name}x{runs}" for name, runs in plan.items()) + "\n"


def awfy_program(root, name, inner):
    modules = "".join(
        f"__define({json.dumps('./' + module)}, function (module, exports, require) {{\n"
        f"{(root / (module + '.js')).read_text()}\n}});\n" for module in AWFY_MODULES)
    driver = (f"var __bench = require({json.dumps('./' + name.lower())}).newInstance();\n"
              f"if (!__bench.innerBenchmarkLoop({inner})) throw new Error('Benchmark failed with incorrect result');\n"
              f"print({json.dumps(f'{MARKER}:awfy:{name}:{inner}')});\n")
    return PRELUDE + REQUIRE_SHIM + modules + driver, f"{MARKER}:awfy:{name}:{inner}\n"


def build(args):
    v8 = checkout(args.v8_source, V8_V7_SOURCE_COMMIT, ["v8-v7/base.js"]) / "v8-v7"
    octane = checkout(args.octane_source, OCTANE_SOURCE_COMMIT, ["base.js"])
    awfy = checkout(args.awfy_source, AWFY_SOURCE_COMMIT, ["benchmarks/JavaScript/som.js"])
    awfy = awfy / "benchmarks/JavaScript"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    workloads = []

    def emit(case, size, text, expected, sources):
        path = output / f"{case}.js"
        path.write_text(text)
        workloads.append({"case": case, "size": size, "path": str(path), "sha256": digest(path),
                          "expected": expected,
                          "sources": {str(s.relative_to(s.parents[1])): digest(s) for s in sources}})

    for suite, root, items in (("v8-v7", v8, V8_V7), ("octane", octane, OCTANE)):
        for item, (files, frozen) in items.items():
            plan = {name: args.pilot or runs for name, runs in frozen.items()}
            if not all(plan.values()):
                raise ValueError(f"{suite}/{item} has no frozen run count; build with --pilot")
            sources = [root / "base.js", *(root / name for name in files)]
            text = (PRELUDE + EXTRA.get((suite, item), "") + "".join(p.read_text() + "\n" for p in sources)
                    + suite_driver(suite, item, plan))
            emit(f"{suite}-{item}", sum(plan.values()), text, suite_expected(suite, item, plan), sources)
    for name, inner in AWFY.items():
        text, expected = awfy_program(awfy, name, inner)
        emit(f"awfy-{name}", inner, text, expected, [awfy / (m + ".js") for m in AWFY_MODULES])
    manifest = {"metadata": {
        "suite": "v8-v7, octane and awfy fixed work",
        "generator": "scripts/benchmark/suite_matrix.py", "generator_sha256": digest(__file__),
        "pilot": args.pilot,
        "sources": {"v8-v7": V8_V7_SOURCE_COMMIT, "octane": OCTANE_SOURCE_COMMIT, "awfy": AWFY_SOURCE_COMMIT},
        "workloads": {"workloads": workloads}}}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{len(workloads)} workloads in {output}")


def medians(results, field):
    """Median of one per-sample value for every (case, engine) whose samples all passed."""
    groups = {}
    for sample in results["samples"]:
        groups.setdefault((sample["case"], sample["engine"]), []).append(sample)
    values = {}
    for key, group in groups.items():
        if any(s["status"] != "ok" for s in group):
            values[key] = None
        elif field == "wall":
            values[key] = statistics.median(s["process_wall_ns"] for s in group) / 1e6
        else:
            values[key] = statistics.median(s["hardware_counters"]["values"][field] for s in group)
    return values


def table(args):
    rows, engines = {}, []
    for path in args.results:
        results = json.loads(Path(path).read_text())
        wall = medians(results, "wall")
        instructions = medians(results, "instructions") if args.instructions else {}
        for (case, engine), value in wall.items():
            if engine not in engines:
                engines.append(engine)
            rows.setdefault(case, {})[engine] = (value, instructions.get((case, engine)))
    if args.reference not in engines:
        raise ValueError("reference engine missing from results")
    others = [e for e in engines if e != args.reference]
    header = ["Workload"]
    for engine in engines:
        header += [f"{engine} ms"] + ([f"{engine} G instr"] if args.instructions else [])
    header += [f"{engine} / {args.reference} time" for engine in others]
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    ratios = {engine: [] for engine in others}
    for case, cells in rows.items():
        row = [case]
        for engine in engines:
            ms, instr = cells.get(engine, (None, None))
            row.append("fail" if ms is None else f"{ms:.0f}")
            if args.instructions:
                row.append("–" if instr is None else f"{instr / 1e9:.2f}")
        reference = cells.get(args.reference, (None, None))[0]
        for engine in others:
            ms = cells.get(engine, (None, None))[0]
            if ms is None or reference is None:
                row.append("–")
            else:
                ratios[engine].append(ms / reference)
                row.append(f"{ms / reference:.2f}")
        lines.append("| " + " | ".join(row) + " |")
    for engine, values in ratios.items():
        if values:
            lines.append(f"\n{engine} / {args.reference} time, geometric mean over {len(values)} workloads: "
                         f"{statistics.geometric_mean(values):.2f}")
    print("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    generate = commands.add_parser("build", help="generate programs and manifest from pinned checkouts")
    generate.add_argument("--v8-source", type=Path, required=True, help=f"js-engine-benchmark at {V8_V7_SOURCE_COMMIT}")
    generate.add_argument("--octane-source", type=Path, required=True, help=f"octane at {OCTANE_SOURCE_COMMIT}")
    generate.add_argument("--awfy-source", type=Path, required=True, help=f"are-we-fast-yet at {AWFY_SOURCE_COMMIT}")
    generate.add_argument("--pilot", type=int, help="run every V8/Octane Benchmark this many times instead of "
                                                     "the frozen counts (for choosing counts only)")
    generate.add_argument("--output", type=Path, required=True)
    tabulate = commands.add_parser("table", help="median per workload from fixed.py results")
    tabulate.add_argument("results", type=Path, nargs="+")
    tabulate.add_argument("--reference", required=True)
    tabulate.add_argument("--instructions", action="store_true", help="include --linux-counters instructions")
    args = parser.parse_args()
    if args.command == "build" and args.pilot is not None and args.pilot < 1:
        parser.error("--pilot must be positive")
    build(args) if args.command == "build" else table(args)


if __name__ == "__main__":
    main()
