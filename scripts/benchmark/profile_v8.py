#!/usr/bin/env python3
"""Profile pinned V8-v7 bodies with a fixed diagnostic driver, never a Score."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re

from run import (LOAD, ROOT, V8_V7_SOURCE_COMMIT, binary_metadata, command_output,
                 digest, git_metadata, machine_metadata, run_sample)


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
COST_SCHEMA = "oxide-compile-vm-cost-v1"


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


def parse_cost_json(path, expected_commit=None):
    records = []
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"invalid profile JSON line {line_number}: {error}") from error
        if not isinstance(record, dict) or not isinstance(record.get("schema"), str):
            raise ValueError(f"invalid profile record at line {line_number}")
        records.append(record)
    costs = [record for record in records if record["schema"] == COST_SCHEMA]
    if len(costs) != 1:
        raise ValueError("profile must contain exactly one compile/VM cost record")
    cost = costs[0]
    metadata = cost.get("metadata")
    if not isinstance(metadata, dict):
        raise ValueError("profile lacks cost metadata")
    if metadata.get("profiling_feature") is not True:
        raise ValueError("cost record did not come from a profiling build")
    if expected_commit and metadata.get("commit") != expected_commit:
        raise ValueError("profile embedded commit differs from build receipt")
    fusion = cost.get("fusion_diagnostics")
    if not isinstance(fusion, dict) or not isinstance(fusion.get("omitted"), dict):
        raise ValueError("profile lacks fusion coverage and omission fields")
    omission_fields = ("static_functions", "dispatch_events", "outcome_events", "callsite_events")
    if any(type(fusion["omitted"].get(name)) is not int or fusion["omitted"][name] < 0
           for name in omission_fields):
        raise ValueError("invalid fusion omission counters")
    for name in ("functions", "dispatch", "sites", "callsites"):
        if not isinstance(fusion.get(name), list):
            raise ValueError(f"profile lacks fusion {name} list")
    if not isinstance(fusion.get("callsite_scope"), str):
        raise ValueError("profile lacks callsite coverage scope")
    if not fusion["functions"] and not fusion["omitted"]["static_functions"]:
        raise ValueError("profile lacks function coverage")
    vm_phases = cost.get("vm_phases")
    if not isinstance(vm_phases, dict):
        raise ValueError("invalid VM phases")
    for name, phase in vm_phases.items():
        if (not isinstance(name, str) or not isinstance(phase, dict)
                or type(phase.get("omitted_samples")) is not int
                or phase["omitted_samples"] < 0):
            raise ValueError("invalid VM phase omissions")
    return {"schemas": [record["schema"] for record in records],
            "embedded_commit": metadata.get("commit"),
            "fusion_callsite_scope": fusion.get("callsite_scope"),
            "fusion_omitted": fusion["omitted"],
            "fusion_counts": {name: len(fusion[name]) for name in ("functions", "dispatch", "sites", "callsites")},
            "vm_phase_omitted_samples": {name: phase.get("omitted_samples") for name, phase in vm_phases.items()},
            "unavailable": cost.get("unavailable")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="external pinned js-engine-benchmark checkout")
    parser.add_argument("--engine", type=Path, required=True, help="profiling-feature qjs executable")
    parser.add_argument("--case", action="append", choices=list(BENCHMARKS))
    parser.add_argument("--iterations", type=int, default=1, help="fixed Setup/run/TearDown cycles per benchmark")
    parser.add_argument("--timeout", type=float, default=3600)
    parser.add_argument("--v8-source-commit", default=V8_V7_SOURCE_COMMIT)
    parser.add_argument("--output", type=Path, required=True, help="new output directory outside both repositories")
    args = parser.parse_args()
    if args.iterations < 1 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("iterations and timeout must be positive and finite")
    if not re.fullmatch(r"[0-9a-f]{40}", args.v8_source_commit):
        parser.error("--v8-source-commit must be a full lowercase SHA")
    cases = args.case or list(BENCHMARKS)
    if len(set(cases)) != len(cases):
        parser.error("cases must be unique")
    binary = args.engine.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error("--engine must be an executable file")
    try:
        source = validate_source(args.source, args.v8_source_commit)
        engine = binary_metadata(binary)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    output = args.output.resolve()
    if output.is_relative_to(ROOT) or output.is_relative_to(args.source.resolve()):
        parser.error("output must be outside both source repositories")
    output.mkdir(parents=True, exist_ok=False)
    generated_dir = output / "generated"
    generated_dir.mkdir()
    raw_dir = output / "raw"
    raw_dir.mkdir()
    workloads = [prepare_case(source, case, args.iterations, generated_dir) for case in cases]
    metadata = {"schema": "oxide-v8-fixed-profile-v1", "metric": "diagnostic fixed coverage; not V8 Score or adaptive timing",
                "source": source, "engine": engine, "machine": machine_metadata(),
                "runner_sha256": digest(__file__), "workloads": workloads,
                "iterations_per_benchmark": args.iterations, "timeout_seconds": args.timeout}
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    samples = []
    with (output / "samples.jsonl").open("w") as journal:
        for workload in workloads:
            case = workload["case"]
            if digest(workload["path"]) != workload["sha256"] or digest(binary) != engine["sha256"]:
                raise ValueError("generated workload or engine changed during profile run")
            if any(digest(source["files"][name]["path"]) != source["files"][name]["sha256"]
                   for name in ("base.js", case + ".js", "run.js")):
                raise ValueError("upstream V8 source changed during profile run")
            cost_path = raw_dir / (case + ".cost.jsonl")
            command = [str(binary), "-d", "--profile-json", "--profile-output", str(cost_path), workload["path"]]
            sample = run_sample(command, output, raw_dir / case, args.timeout)
            sample["case"] = case
            sample["generated_sha256"] = workload["sha256"]
            sample["status"] = "timeout" if sample["timed_out"] else "failed" if sample["exit_code"] else "ok"
            if sample["status"] == "ok" and Path(sample["stdout"]).read_text() != workload["expected_marker"]:
                sample["status"] = "invalid-completion-marker"
            if sample["status"] == "ok" and Path(sample["stderr"]).read_bytes():
                sample["status"] = "unexpected-stderr"
            if cost_path.is_file():
                sample["cost_json"] = {"path": str(cost_path), "sha256": digest(cost_path)}
                try:
                    build = engine.get("build") or {}
                    sample["cost_json"].update(parse_cost_json(cost_path, build.get("commit")))
                except (ValueError, OSError) as error:
                    sample["cost_json"]["error"] = str(error)
                    if sample["status"] == "ok":
                        sample["status"] = "invalid-cost-json"
            elif sample["status"] == "ok":
                sample["status"] = "missing-cost-json"
            samples.append(sample)
            journal.write(json.dumps(sample) + "\n")
            journal.flush()
            print(f"{case}: {sample['status']}", flush=True)
    (output / "results.json").write_text(json.dumps({"metadata": metadata, "samples": samples}, indent=2) + "\n")
    return 0 if all(sample["status"] == "ok" for sample in samples) else 1


if __name__ == "__main__":
    raise SystemExit(main())
