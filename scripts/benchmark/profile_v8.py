#!/usr/bin/env python3
"""The pinned V8-v7 bodies with a fixed diagnostic driver, never a Score: the
shared workload generator of `iterate_v8.py` and the Callgrind probes. Its
profiling-build front end went with the profiling API (new-core phase 6).
"""
import hashlib
import json
from pathlib import Path
import re

from run import LOAD, ROOT, V8_V7_SOURCE_COMMIT, command_output, digest, git_metadata  # noqa: F401 (re-exported)


# Names and counts are checked against the pinned source before running JS.
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


def validate_source(source, expected_commit):
    source = source.resolve()
    if source.is_relative_to(ROOT):
        raise ValueError("V8 source must be outside quickjs-oxide")
    repository = git_metadata(source)
    if (repository["commit"]["exit_code"] or repository["tree"]["exit_code"]
            or repository["top_level"]["exit_code"]
            or Path(repository["top_level"]["stdout"]).resolve() != source):
        raise ValueError("V8 source must be a Git checkout root")
    if repository["commit"]["stdout"] != expected_commit:
        raise ValueError(f"V8 source must be pinned to {expected_commit}")
    tracked = command_output(["git", "status", "--porcelain", "--untracked-files=no"], source)
    if tracked["exit_code"] or tracked["stdout"]:
        raise ValueError("V8 tracked source has local changes")
    code_root = source / "v8-v7"
    run_js = (code_root / "run.js").read_text()
    loaded = [match[1] for match in LOAD.finditer(run_js)]
    if loaded != ["base.js", *(case + ".js" for case in BENCHMARKS)]:
        raise ValueError("pinned V8 run.js suite list changed")
    paths = {name: code_root / name for name in loaded}
    paths["run.js"] = code_root / "run.js"
    return {"repository": repository, "expected_commit": expected_commit,
            "root": str(source), "files": {name: {"path": str(path), "sha256": digest(path)}
                                             for name, path in paths.items()}}


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


def prepare_case(source_metadata, case, iterations, generated_dir):
    files = source_metadata["files"]
    base = Path(files["base.js"]["path"]).read_bytes()
    case_file = files[case + ".js"]
    body = Path(case_file["path"]).read_bytes()
    if digest(files["base.js"]["path"]) != files["base.js"]["sha256"] or digest(case_file["path"]) != case_file["sha256"]:
        raise ValueError("V8 source changed during generation")
    suite, benchmarks = declared_benchmarks(body.decode("utf-8"), case)
    driver = fixed_driver(suite, benchmarks, iterations)
    generated = generated_dir / (case + ".js")
    generated.write_bytes(base + b"\n" + body + b"\n" + driver)
    return {"case": case, "path": str(generated), "sha256": digest(generated),
            "source_sha256": case_file["sha256"], "base_sha256": files["base.js"]["sha256"],
            "driver_sha256": hashlib.sha256(driver).hexdigest(),
            "suite": suite, "benchmarks": list(benchmarks), "iterations": iterations,
            "expected_marker": f"{MARKER}:{suite}:{len(benchmarks)}:{iterations}:{len(benchmarks) * iterations}\n"}
