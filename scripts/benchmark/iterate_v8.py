#!/usr/bin/env python3
"""Bounded fixed-iteration V8-v7 comparison, distinct from the original Score.

One invocation pilots the pinned bodies on the baseline, freezes per-suite run
counts, then executes same-binary A/A and baseline/candidate A/B in balanced
order. Pilot, generation and both comparisons share one hard wall deadline.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import time

from fixed import parse_darwin_counters
from profile_v8 import BENCHMARKS, declared_benchmarks, validate_source
from run import (ROOT, V8_V7_SOURCE_COMMIT, binary_metadata, digest,
                 machine_metadata, paired_order, run_sample, validate_paired_order)


MARKER = "__oxide_v8_fixed_iteration_complete__"
COUNTER_SCOPE = "whole process including startup and teardown"
CASES = tuple(BENCHMARKS)
METRIC = "whole-process wall nanoseconds including startup, compilation and teardown"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def tooling_identity():
    directory = Path(__file__).resolve().parent
    return {name: {"path": str(directory / name), "sha256": digest(directory / name)}
            for name in ("iterate_v8.py", "profile_v8.py", "run.py", "fixed.py")}


def verify_tooling(identity):
    for item in identity.values():
        require(digest(item["path"]) == item["sha256"], "benchmark tooling changed during run")


def require_plain_engine(binary):
    metadata = binary_metadata(binary)
    build = metadata.get("build")
    require(isinstance(build, dict), f"plain release build receipt required: {binary}")
    require(build.get("mode") == "plain" and build.get("features") == [],
            f"not a plain release build: {binary}")
    require("--release" in build.get("command", []) and build.get("exit_code") == 0,
            f"not a successful release build: {binary}")
    require(build.get("source", {}).get("status", {}).get("stdout") == "",
            f"build source was not clean: {binary}")
    require(build.get("source", {}).get("commit", {}).get("stdout") == build.get("commit"),
            f"build commit differs from source receipt: {binary}")
    return metadata


def require_comparable_builds(baseline, candidate):
    left, right = baseline["build"], candidate["build"]
    for name in ("rustc", "cargo", "target"):
        require(left[name] == right[name], f"engine {name} differs")
    for name in ("manifest", "environment_overrides", "cargo_config_sha256"):
        require(left["release_profile"][name] == right["release_profile"][name],
                f"release {name} differs")
    require(left["release_profile"]["qjs_rustc_invocation"]["codegen"]
            == right["release_profile"]["qjs_rustc_invocation"]["codegen"],
            "effective CLI codegen differs")


def verify_source_files(source, cases):
    for name in ("base.js", "run.js", *(case + ".js" for case in cases)):
        item = source["files"][name]
        require(digest(item["path"]) == item["sha256"], f"pinned V8 source changed: {name}")


def fixed_driver(specifications, case_label, warmup):
    """One Setup/TearDown per Benchmark; fixed warmup and run calls inside."""
    literal = json.dumps(specifications, separators=(",", ":"))
    label = json.dumps(case_label)
    total = sum(len(spec[1]) * (warmup + spec[2]) for spec in specifications)
    completion = f"{MARKER}:{case_label}:{len(specifications)}:{total}\n"
    driver = f"""
// OXIDE FIXED-ITERATION DRIVER: not the upstream adaptive V8-v7 Score.
(function() {{
  var expected = {literal};
  var warmup = {warmup};
  var suites = BenchmarkSuite.suites;
  if (suites.length !== expected.length) throw new Error('suite count mismatch');
  var completed = 0;
  for (var s = 0; s < expected.length; s++) {{
    var suite = suites[s];
    var item = expected[s];
    if (suite.name !== item[0] || suite.benchmarks.length !== item[1].length)
      throw new Error('suite identity mismatch');
    for (var b = 0; b < item[1].length; b++) {{
      var benchmark = suite.benchmarks[b];
      if (benchmark.name !== item[1][b]) throw new Error('benchmark identity mismatch');
      benchmark.Setup();
      for (var w = 0; w < warmup; w++) {{ benchmark.run(); completed++; }}
      for (var n = 0; n < item[2]; n++) {{ benchmark.run(); completed++; }}
      benchmark.TearDown();
    }}
  }}
  print('{MARKER}:' + {label} + ':' + suites.length + ':' + completed);
}})();
""".encode()
    return driver, completion


def prepare_workload(source, cases, counts, warmup, generated_dir, label):
    """Preserve every upstream body byte, in pinned run.js suite order."""
    require(cases and all(case in BENCHMARKS for case in cases), "unknown or empty V8 case")
    require(list(cases) == [case for case in CASES if case in cases], "cases are out of pinned order")
    verify_source_files(source, cases)
    base = Path(source["files"]["base.js"]["path"]).read_bytes()
    chunks = [base]
    specs = []
    bodies = {}
    for case in cases:
        item = source["files"][case + ".js"]
        body = Path(item["path"]).read_bytes()
        suite, benchmark_names = declared_benchmarks(body.decode("utf-8"), case)
        runs = counts[case]
        require(type(runs) is int and runs >= 1, f"{case}: invalid run count")
        chunks.append(body)
        specs.append([suite, list(benchmark_names), runs])
        bodies[case] = item["sha256"]
    driver, expected = fixed_driver(specs, label, warmup)
    generated = generated_dir / (label + ".js")
    generated.write_bytes(b"\n".join([*chunks, driver]))
    return {"case": label, "path": str(generated), "sha256": digest(generated),
            "base_sha256": source["files"]["base.js"]["sha256"],
            "body_sha256": bodies, "driver_sha256": hashlib.sha256(driver).hexdigest(),
            "runs_per_suite": {case: counts[case] for case in cases}, "warmup_per_benchmark": warmup,
            "expected_stdout": expected, "suite_order": list(cases)}


def workload_set(source, counts, warmup, generated_dir):
    return [prepare_workload(source, [case], counts, warmup, generated_dir, case)
            for case in CASES] + [prepare_workload(source, CASES, counts, warmup, generated_dir, "combined")]


def calibrate(pilot_ns, target_ns, max_runs, remaining_ns, repeats):
    """Use one baseline pilot per suite and reserve room for both paired phases."""
    require(set(pilot_ns) == {*CASES, "combined"}, "pilot suite set incomplete")
    require(all(type(value) is int and value > 0 for value in pilot_ns.values()), "invalid pilot time")
    counts = {case: max(1, min(max_runs, int(target_ns // pilot_ns[case]))) for case in CASES}

    def estimate():
        isolated = sum(pilot_ns[case] * counts[case] for case in CASES)
        combined = max(pilot_ns["combined"], isolated)
        # A/A and A/B each have two engines and `repeats` per engine.
        return 4 * repeats * (isolated + combined)

    while estimate() > remaining_ns * 0.7 and any(value > 1 for value in counts.values()):
        counts = {case: max(1, value // 2) for case, value in counts.items()}
    return counts, estimate(), estimate() <= remaining_ns * 0.7


def schedule(workloads, order, repeats):
    """Per workload: A/A aliases and A/B pair, each in balanced blocks."""
    validate_paired_order(["left", "right"], repeats, order)
    jobs = []
    for workload in workloads:
        for phase, names in (("aa", ["same_a", "same_b"]),
                             ("ab", ["baseline", "candidate"])):
            for repetition in range(repeats):
                for name in paired_order(names, repetition, order):
                    jobs.append({"case": workload["case"], "phase": phase,
                                 "repetition": repetition, "engine": name})
    return jobs


def run_one(workload, engine, source, identity, output, prefix, deadline, sample_timeout,
            darwin_counters, phase, repetition, name):
    if deadline - time.monotonic() <= 0:
        return None
    verify_tooling(identity)
    verify_source_files(source, workload["suite_order"])
    require(digest(workload["path"]) == workload["sha256"], "generated JS changed")
    require(digest(engine["path"]) == engine["sha256"], "engine binary changed")
    command = [engine["path"], workload["path"]]
    counters_path = prefix.with_suffix(".time-l.txt")
    if darwin_counters:
        command = ["/usr/bin/time", "-l", "-o", str(counters_path), *command]
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        return None
    try:
        sample = run_sample(command, output, prefix, min(sample_timeout, remaining))
    except OSError as error:
        sample = {"command": command, "exit_code": None, "timed_out": False,
                  "process_wall_ns": None, "stdout": str(prefix.with_suffix(".stdout")),
                  "stderr": str(prefix.with_suffix(".stderr")), "launch_error": str(error)}
    sample.update(case=workload["case"], phase=phase, repetition=repetition,
                  engine=name, generated_sha256=workload["sha256"])
    sample["status"] = ("launch-error" if "launch_error" in sample else
                        "timeout" if sample["timed_out"] else
                        "failed" if sample["exit_code"] else "ok")
    if sample["status"] == "ok" and Path(sample["stdout"]).read_bytes() != workload["expected_stdout"].encode():
        sample["status"] = "invalid-completion-marker"
    if sample["status"] == "ok" and Path(sample["stderr"]).read_bytes():
        sample["status"] = "unexpected-stderr"
    if darwin_counters:
        counter = {"scope": COUNTER_SCOPE, "raw_path": str(counters_path)}
        try:
            counter["raw_sha256"] = digest(counters_path)
            counter["values"] = parse_darwin_counters(counters_path.read_text())
        except (OSError, ValueError) as error:
            counter["error"] = str(error)
            if sample["status"] == "ok":
                sample["status"] = "invalid-hardware-counters"
        sample["hardware_counters"] = counter
    return sample


def summarize(samples, jobs, repeats):
    fields = ("case", "phase", "repetition", "engine")
    complete = (len(samples) == len(jobs)
                and all(tuple(row[field] for field in fields) == tuple(job[field] for field in fields)
                        for row, job in zip(samples, jobs))
                and all(row["status"] == "ok" for row in samples))
    groups = {}
    for row in samples:
        if row["status"] == "ok":
            groups.setdefault((row["case"], row["phase"], row["engine"]), []).append(row["process_wall_ns"])
    cases = {}
    for case in (*CASES, "combined"):
        records = {}
        for phase, names in (("aa", ("same_a", "same_b")), ("ab", ("baseline", "candidate"))):
            values = {name: groups.get((case, phase, name), []) for name in names}
            records[phase] = {"raw_wall_ns": values,
                              "median_wall_ns": {name: statistics.median(rows) if len(rows) == repeats else None
                                                 for name, rows in values.items()},
                              "complete": all(len(rows) == repeats for rows in values.values())}
            if records[phase]["complete"]:
                left, right = names
                records[phase]["left_over_right_wall_ratio"] = (
                    records[phase]["median_wall_ns"][left]
                    / records[phase]["median_wall_ns"][right])
        if records["aa"]["complete"] and records["ab"]["complete"]:
            aa_values = [*records["aa"]["raw_wall_ns"]["same_a"],
                         *records["aa"]["raw_wall_ns"]["same_b"]]
            aa_spread_pct = ((max(aa_values) - min(aa_values))
                             / statistics.median(aa_values) * 100)
            ab_change_pct = (records["ab"]["left_over_right_wall_ratio"] - 1) * 100
            records["aa_observed_span_pct"] = aa_spread_pct
            records["ab_baseline_over_candidate_pct"] = ab_change_pct
            records["interpretation"] = (
                "inconclusive_within_aa_span" if abs(ab_change_pct) <= aa_spread_pct else
                "outside_observed_aa_span_not_admission")
        cases[case] = records
    aggregate = None
    if complete:
        ratios = [cases[case]["ab"]["left_over_right_wall_ratio"] for case in CASES]
        aggregate = {"isolated_eight_geomean_baseline_over_candidate_wall":
                     math.exp(sum(math.log(value) for value in ratios) / len(ratios)),
                     "combined_baseline_over_candidate_wall":
                     cases["combined"]["ab"]["left_over_right_wall_ratio"],
                     "meaning": "fixed-iteration whole-process speed ratios; not original V8-v7 Score"}
    return {"complete": complete, "scheduled": len(jobs), "recorded": len(samples),
            "status_counts": {status: sum(row["status"] == status for row in samples)
                              for status in sorted({row["status"] for row in samples})},
            "cases": cases, "aggregate": aggregate}


def replay_workloads(plan, source, identity, generated_dir, order, repeats, warmup):
    require(plan.get("schema") == "oxide-v8-fixed-iteration-freeze-v1", "invalid freeze plan")
    require(plan["source"]["expected_commit"] == source["expected_commit"]
            and plan["source"]["repository"]["tree"]["stdout"]
            == source["repository"]["tree"]["stdout"]
            and plan["source"]["files"] == source["files"], "freeze plan source differs")
    require(plan["tooling"] == identity, "freeze plan tooling differs")
    require(plan["order"] == order and plan["repeats_per_engine_per_phase"] == repeats,
            "freeze plan paired schedule differs")
    require(plan["warmup_per_benchmark"] == warmup, "freeze plan warmup differs")
    counts = plan["runs_per_suite"]
    require(set(counts) == set(CASES), "freeze plan suite counts incomplete")
    workloads = workload_set(source, counts, warmup, generated_dir)
    original = plan["workloads"]
    require(len(original) == len(workloads), "freeze plan workload count differs")
    for old, new in zip(original, workloads):
        require({key: value for key, value in old.items() if key != "path"}
                == {key: value for key, value in new.items() if key != "path"},
                f"freeze plan generated JS differs: {new['case']}")
    return workloads


def input_error_sample(case, phase, repetition, name, error):
    return {"case": case, "phase": phase, "repetition": repetition,
            "engine": name, "status": "input-identity-error", "error": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help="external pinned V8-v7 checkout")
    parser.add_argument("--baseline", type=Path, required=True, help="plain release qjs for pilot and A/A")
    parser.add_argument("--candidate", type=Path, required=True, help="plain release qjs for A/B")
    parser.add_argument("--output", type=Path, required=True, help="new directory outside both repositories")
    parser.add_argument("--plan", type=Path,
                        help="reuse a prior freeze-plan.json; skip pilot and regenerate byte-identical work")
    parser.add_argument("--v8-source-commit", default=V8_V7_SOURCE_COMMIT)
    parser.add_argument("--target-seconds", type=float, default=1.0,
                        help="pilot-based target per isolated process; not a timing score")
    parser.add_argument("--max-runs", type=int, default=1000)
    parser.add_argument("--warmup", type=int, default=0, help="fixed warmup run calls per Benchmark")
    parser.add_argument("--order", choices=("abba", "baab", "abba-baab"), default="abba-baab")
    parser.add_argument("--repeat", type=int, default=4,
                        help="repetitions per engine per phase; default completes ABBA-BAAB")
    parser.add_argument("--deadline-seconds", type=float, default=600)
    parser.add_argument("--sample-timeout", type=float, default=90)
    parser.add_argument("--darwin-counters", action="store_true")
    args = parser.parse_args()
    if (not re.fullmatch(r"[0-9a-f]{40}", args.v8_source_commit)
            or args.max_runs < 1 or args.warmup < 0 or args.repeat < 1
            or any(not math.isfinite(value) or value <= 0 for value in
                   (args.target_seconds, args.deadline_seconds, args.sample_timeout))):
        parser.error("invalid pin, run count, warmup or time limit")
    try:
        validate_paired_order(["left", "right"], args.repeat, args.order)
    except ValueError as error:
        parser.error(str(error))
    if args.darwin_counters and (platform.system() != "Darwin" or not os.access("/usr/bin/time", os.X_OK)):
        parser.error("--darwin-counters requires macOS /usr/bin/time")
    deadline = time.monotonic() + args.deadline_seconds
    try:
        source = validate_source(args.source, args.v8_source_commit)
        baseline_path = args.baseline.resolve()
        candidate_path = args.candidate.resolve()
        require(baseline_path != candidate_path, "candidate must be a distinct binary path")
        baseline = require_plain_engine(baseline_path)
        candidate = require_plain_engine(candidate_path)
        require_comparable_builds(baseline, candidate)
        identity = tooling_identity()
    except (OSError, ValueError) as error:
        parser.error(str(error))
    output = args.output.resolve()
    if output.is_relative_to(ROOT) or output.is_relative_to(args.source.resolve()):
        parser.error("output must be outside both repositories")
    output.mkdir(parents=True, exist_ok=False)
    (output / "raw").mkdir()
    pilot_dir = output / "generated" / "pilot"
    pilot_dir.mkdir(parents=True)
    final_dir = output / "generated" / "final"
    final_dir.mkdir()
    metadata = {"schema": "oxide-v8-fixed-iteration-v1", "scope": METRIC,
                "not_original_v8_score": True, "source": source,
                "rng_contract": "pinned base.js initializes deterministic Math.random once per process; no ResetRNG symbol",
                "baseline": baseline, "candidate": candidate, "machine": machine_metadata(),
                "tooling": identity, "deadline_seconds": args.deadline_seconds,
                "target_seconds": args.target_seconds, "max_runs": args.max_runs,
                "warmup_per_benchmark": args.warmup, "order": args.order,
                "repeats_per_engine_per_phase": args.repeat, "darwin_counters": args.darwin_counters,
                "counter_scope": COUNTER_SCOPE if args.darwin_counters else None,
                "replayed_plan": {"path": str(args.plan.resolve()), "sha256": digest(args.plan)}
                                 if args.plan else None}
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    pilots = []
    plan_path = output / "freeze-plan.json"
    actual_order, actual_repeat = args.order, args.repeat
    if args.plan:
        original_plan = args.plan.resolve()
        plan_bytes = original_plan.read_bytes()
        plan = json.loads(plan_bytes)
        actual_order = plan["order"]
        actual_repeat = plan["repeats_per_engine_per_phase"]
        workloads = replay_workloads(plan, source, identity, final_dir, actual_order,
                                    actual_repeat, args.warmup)
        plan_path.write_bytes(plan_bytes)
    else:
        pilot_workloads = workload_set(source, {case: 1 for case in CASES}, args.warmup, pilot_dir)
        with (output / "pilot.jsonl").open("w") as journal:
            for workload in pilot_workloads:
                prefix = output / "raw" / ("pilot-" + workload["case"])
                try:
                    sample = run_one(workload, baseline, source, identity, output, prefix, deadline,
                                     args.sample_timeout, args.darwin_counters, "pilot", 0, "baseline")
                except (OSError, ValueError) as error:
                    sample = input_error_sample(workload["case"], "pilot", 0, "baseline", error)
                if sample is None:
                    break
                pilots.append(sample)
                journal.write(json.dumps(sample) + "\n")
                journal.flush()
                print(f"pilot/{workload['case']}: {sample['status']}", flush=True)
                if sample["status"] != "ok":
                    break
        pilot_valid = (len(pilots) == len(pilot_workloads)
                       and all(row["status"] == "ok" for row in pilots))
        if not pilot_valid:
            result = {"metadata": metadata, "pilot": pilots, "freeze_plan": None,
                      "samples": [], "summary": {"complete": False, "aggregate": None,
                                                 "reason": "pilot failed or hard deadline reached"}}
            (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
            return 1

        pilot_ns = {row["case"]: row["process_wall_ns"] for row in pilots}
        remaining_ns = max(0, int((deadline - time.monotonic()) * 1e9))
        counts, estimated_ns, feasible = calibrate(pilot_ns, int(args.target_seconds * 1e9),
                                                   args.max_runs, remaining_ns, actual_repeat)
        if not feasible and (actual_order, actual_repeat) == ("abba-baab", 4):
            # The pinned bodies can already take seconds at one run. Preserve
            # the minimum balanced A/A and A/B comparison under the hard cap.
            actual_order, actual_repeat = "abba", 2
            counts, estimated_ns, feasible = calibrate(pilot_ns, int(args.target_seconds * 1e9),
                                                       args.max_runs, remaining_ns, actual_repeat)
            print("pilot: budget reduced formal repeats to 2/engine (ABBA)", flush=True)
        if not feasible:
            result = {"metadata": metadata, "pilot": pilots, "freeze_plan": None,
                      "samples": [], "summary": {"complete": False, "aggregate": None,
                                                 "reason": "baseline pilot cannot fit the formal matrix in remaining deadline",
                                                 "estimated_formal_ns": estimated_ns}}
            (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
            return 1

        workloads = workload_set(source, counts, args.warmup, final_dir)
        plan = {"schema": "oxide-v8-fixed-iteration-freeze-v1", "source": source,
                "tooling": identity, "calibration_baseline": baseline,
                "pilot": pilots, "pilot_wall_ns": pilot_ns,
                "target_seconds": args.target_seconds, "runs_per_suite": counts,
                "warmup_per_benchmark": args.warmup, "estimated_formal_ns": estimated_ns,
                "workloads": workloads, "order": actual_order,
                "repeats_per_engine_per_phase": actual_repeat,
                "meaning": "fixed iteration work frozen before A/A and A/B; not original V8 Score"}
        plan_path.write_text(json.dumps(plan, indent=2) + "\n")
    metadata["actual_order"] = actual_order
    metadata["actual_repeats_per_engine_per_phase"] = actual_repeat
    metadata["budget_repeat_reduction"] = actual_repeat < args.repeat and not args.plan
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    jobs = schedule(workloads, actual_order, actual_repeat)
    lookup = {workload["case"]: workload for workload in workloads}
    engines = {"same_a": baseline, "same_b": baseline,
               "baseline": baseline, "candidate": candidate}
    samples = []
    with (output / "samples.jsonl").open("w") as journal:
        for job in jobs:
            label = f"{job['case']}-{job['phase']}-{job['engine']}-{job['repetition']}"
            try:
                sample = run_one(lookup[job["case"]], engines[job["engine"]], source,
                                 identity, output, output / "raw" / label, deadline,
                                 args.sample_timeout, args.darwin_counters,
                                 job["phase"], job["repetition"], job["engine"])
            except (OSError, ValueError) as error:
                sample = input_error_sample(job["case"], job["phase"], job["repetition"],
                                            job["engine"], error)
            if sample is None:
                break
            samples.append(sample)
            journal.write(json.dumps(sample) + "\n")
            journal.flush()
            print(f"{job['case']}/{job['phase']}/{job['engine']}: {sample['status']}", flush=True)
            if sample["status"] != "ok":
                break
    summary = summarize(samples, jobs, actual_repeat)
    result = {"metadata": metadata, "pilot": pilots,
              "freeze_plan": {"path": str(plan_path), "sha256": digest(plan_path)},
              "samples": samples, "summary": summary}
    (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    return 0 if summary["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
