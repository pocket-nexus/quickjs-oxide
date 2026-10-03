#!/usr/bin/env python3
"""Report paired fixed-work time or original V8 Score gains from frozen runs."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import re
import statistics

from run import paired_order, validate_paired_order

MINIMUM_PAIRS = 6
BOOTSTRAP_RESAMPLES = 10000
BOOTSTRAP_SEED = 79
NONINFERIORITY_PERCENT = 1.0
V8_SCORE_NAMES = {
    "richards": "Richards", "deltablue": "DeltaBlue", "crypto": "Crypto",
    "raytrace": "RayTrace", "earley-boyer": "EarleyBoyer", "regexp": "RegExp",
    "splay": "Splay", "navier-stokes": "NavierStokes",
}
V8_ISOLATED_CASES = set(V8_SCORE_NAMES)


def positive_number(value, description):
    if isinstance(value, bool) or not isinstance(value, (int, float)) \
            or not math.isfinite(value) or value <= 0:
        raise ValueError(f"{description} must be positive and finite")
    return value


def paired_statistics(baseline, candidate, metric):
    """Bootstrap whole paired observations; positive gain always means better."""
    if metric not in ("fixed-time", "original-score"):
        raise ValueError("metric must be fixed-time or original-score")
    if len(baseline) != len(candidate) or len(baseline) < MINIMUM_PAIRS:
        raise ValueError(f"at least {MINIMUM_PAIRS} complete pairs are required")
    gains = []
    for base, new in zip(baseline, candidate):
        positive_number(base, "baseline observation")
        positive_number(new, "candidate observation")
        ratio = new / base
        gain = (1 - ratio) if metric == "fixed-time" else (ratio - 1)
        gain *= 100
        if not math.isfinite(gain):
            raise ValueError("paired gain must be finite")
        gains.append(gain)
    rng = random.Random(BOOTSTRAP_SEED)
    estimates = sorted(
        statistics.median(rng.choices(gains, k=len(gains)))
        for _ in range(BOOTSTRAP_RESAMPLES)
    )
    # Empirical percentiles match the recorded method: indices 249 and 9749.
    interval = [estimates[math.ceil(BOOTSTRAP_RESAMPLES * quantile) - 1]
                for quantile in (0.025, 0.975)]
    return {
        "pairs": len(gains),
        "baseline_median": statistics.median(baseline),
        "candidate_median": statistics.median(candidate),
        "paired_gain_percent": gains,
        "median_gain_percent": statistics.median(gains),
        "gain_ci95_percent": interval,
    }


def result_metadata(results):
    if not isinstance(results, dict):
        raise ValueError("results must be a JSON object")
    metadata = results.get("metadata", {key: value for key, value in results.items()
                                        if key not in ("samples", "summary")})
    if not isinstance(metadata, dict):
        raise ValueError("measurement metadata must be a JSON object")
    return metadata


def admit_results(results, baseline, candidate, metric):
    """Reject partial, reordered or ambiguous matrices before forming ratios."""
    metadata = result_metadata(results)
    engines = metadata.get("engines", {})
    if not isinstance(engines, dict) or baseline == candidate or set(engines) != {baseline, candidate}:
        raise ValueError("results must contain exactly the two distinct selected engines")
    for name, engine in engines.items():
        sha256 = engine.get("sha256") if isinstance(engine, dict) else None
        if not isinstance(sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", sha256):
            raise ValueError(f"{name}: missing binary SHA-256")
    repeat = metadata.get("repeat")
    if type(repeat) is not int or repeat < MINIMUM_PAIRS:
        raise ValueError(f"results require at least {MINIMUM_PAIRS} pairs per case")
    order = metadata.get("order_strategy")
    if order not in ("abba", "baab", "abba-baab"):
        raise ValueError("results require a declared balanced ABBA/BAAB order")
    validate_paired_order(list(engines), repeat, order)
    workloads = metadata.get("workloads", [])
    if not isinstance(workloads, list) or not workloads:
        raise ValueError("results need a nonempty workload manifest")
    by_case = {}
    for workload in workloads:
        if not isinstance(workload, dict):
            raise ValueError("workload manifest entries must be JSON objects")
        case = workload.get("case")
        if not isinstance(case, str) or not re.fullmatch(r"[A-Za-z0-9_-]+", case) \
                or case in by_case:
            raise ValueError("workload cases must be unique safe identifiers")
        sha256 = workload.get("sha256")
        if not isinstance(sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", sha256):
            raise ValueError(f"{case}: missing workload SHA-256")
        by_case[case] = workload
    if metric == "original-score" and metadata.get("suite") != "v8-v7":
        raise ValueError("original-score requires the original v8-v7 runner")
    if metric == "fixed-time" and metadata.get("metric") != \
            "whole-process wall nanoseconds; not adaptive scores":
        raise ValueError("fixed-time requires fixed-work wall-time results")

    expected_order = [
        (case, engine, repetition)
        for case in by_case
        for repetition in range(repeat)
        for engine in paired_order(list(engines), repetition, order)
    ]
    actual_order = []
    by_key = {}
    sample_list = results.get("samples", [])
    if not isinstance(sample_list, list):
        raise ValueError("samples must be a JSON array")
    for sample in sample_list:
        if not isinstance(sample, dict):
            raise ValueError("sample entries must be JSON objects")
        key = (sample.get("case"), sample.get("engine"), sample.get("repetition"))
        if type(key[2]) is not int or key[0] not in by_case or key[1] not in engines \
                or not 0 <= key[2] < repeat:
            raise ValueError("sample is outside the declared case/engine/repetition matrix")
        if key in by_key:
            raise ValueError(f"duplicate paired sample: {key}")
        if sample.get("status") != "ok" or type(sample.get("exit_code")) is not int or sample["exit_code"] != 0 \
                or sample.get("timed_out") is not False \
                or sample.get("semantic_status", "ok") != "ok":
            raise ValueError(f"failed or inadmissible sample: {key}")
        if sample.get("workload_sha256", by_case[key[0]]["sha256"]) != by_case[key[0]]["sha256"]:
            raise ValueError(f"sample workload bytes differ from manifest: {key}")
        if metric == "fixed-time":
            positive_number(sample.get("process_wall_ns"), f"{key}: wall time")
        else:
            expected = by_case[key[0]].get("expected")
            if not isinstance(expected, list) or not expected or not all(isinstance(name, str) for name in expected) \
                    or len(set(expected)) != len(expected) \
                    or "Score" in expected:
                raise ValueError(f"{key[0]}: invalid expected V8 score names")
            measurements = sample.get("measurements", {})
            if not isinstance(measurements, dict) or set(measurements) != set(expected + ["Score"]):
                raise ValueError(f"missing or unexpected original scores: {key}")
            for name, value in measurements.items():
                positive_number(value, f"{key}: {name}")
        actual_order.append(key)
        by_key[key] = sample
    if len(actual_order) != len(expected_order):
        raise ValueError("missing pairs in the declared measurement matrix")
    if actual_order != expected_order:
        raise ValueError("sample journal does not follow the declared balanced order")
    return metadata, by_case, by_key


def metric_rows(metadata, workloads, samples, baseline, candidate, metric):
    rows, contextual = [], []
    for case, workload in workloads.items():
        names = ["process_wall_ns"] if metric == "fixed-time" else ["Score"]
        if metric == "original-score" and case == "all":
            names += workload["expected"]
        for name in names:
            values = []
            for engine in (baseline, candidate):
                observations = [samples[(case, engine, repetition)]
                                for repetition in range(metadata["repeat"])]
                values.append([sample[name] if metric == "fixed-time" else sample["measurements"][name]
                               for sample in observations])
            row = {
                "case": case, "measurement": name,
                "workload_sha256": workload["sha256"],
                "unit": "nanoseconds" if metric == "fixed-time" else "original Score",
                **paired_statistics(*values, metric),
            }
            if metric == "original-score" and case == "all" and name != "Score":
                contextual.append(row)
            else:
                rows.append(row)
    return rows, contextual


def measurement_context(metadata):
    """Stable measurement settings; dynamic load snapshots are retained separately."""
    machine = metadata.get("machine", {})
    return {
        "cpu": metadata.get("cpu"),
        "machine": {key: machine.get(key) for key in
                    ("platform", "machine", "cpu", "cpu_affinity", "scaling_governors")},
        "darwin_counters": metadata.get("darwin_counters", False),
    }


def paired_report(results, baseline, candidate, metric, *, aa_results=None,
                  aa_baseline=None, aa_candidate=None, gate=None, cases=None,
                  require_v8_matrix=False):
    metadata, workloads, samples = admit_results(results, baseline, candidate, metric)
    if cases and (len(cases) != len(set(cases)) or set(cases) - workloads.keys()):
        raise ValueError("selected cases must be unique and present in results")
    selected = {case: value for case, value in workloads.items() if not cases or case in cases}
    combined = "all" if metric == "original-score" else "combined"
    if require_v8_matrix and set(selected) != V8_ISOLATED_CASES | {combined}:
        raise ValueError("full V8 acceptance requires eight isolated cases and a separate combined case")
    if require_v8_matrix and metric == "original-score":
        if any(selected[case]["expected"] != [name] for case, name in V8_SCORE_NAMES.items()) \
                or selected["all"]["expected"] != list(V8_SCORE_NAMES.values()):
            raise ValueError("full original V8 acceptance requires the eight original score names and order")
    if gate not in (None, "net-gain", "noninferior"):
        raise ValueError("unknown acceptance gate")
    if gate and aa_results is None:
        raise ValueError("acceptance gates require a frozen matching A/A result")
    rows, contextual = metric_rows(metadata, selected, samples, baseline, candidate, metric)
    aa_metadata = None
    if aa_results is not None:
        aa_metadata = result_metadata(aa_results)
        aa_names = list(aa_metadata.get("engines", {}))
        if len(aa_names) != 2:
            raise ValueError("A/A requires two engine labels")
        aa_baseline = aa_baseline or aa_names[0]
        aa_candidate = aa_candidate or aa_names[1]
        aa_metadata, aa_workloads, aa_samples = admit_results(
            aa_results, aa_baseline, aa_candidate, metric)
        hashes = [aa_metadata["engines"][name].get("sha256") for name in (aa_baseline, aa_candidate)]
        if not hashes[0] or hashes[0] != hashes[1]:
            raise ValueError("A/A must use the same binary under both engine labels")
        if measurement_context(metadata) != measurement_context(aa_metadata):
            raise ValueError("A/A measurement settings differ from candidate measurement")
        if set(selected) - aa_workloads.keys():
            raise ValueError("A/A is missing a matching workload case")
        aa_selected = {case: aa_workloads[case] for case in selected}
        aa_rows, aa_contextual = metric_rows(
            aa_metadata, aa_selected, aa_samples, aa_baseline, aa_candidate, metric)
        noise = {(row["case"], row["measurement"]): row for row in aa_rows + aa_contextual}
        for row in rows + contextual:
            aa_row = noise.get((row["case"], row["measurement"]))
            if aa_row is None or aa_row["workload_sha256"] != row["workload_sha256"]:
                raise ValueError("A/A workload bytes or metric/context do not match")
            row["aa"] = {
                key: aa_row[key] for key in ("pairs", "median_gain_percent", "gain_ci95_percent")
            }
            row["aa"]["noise_percent"] = max(abs(bound) for bound in aa_row["gain_ci95_percent"])
    for row in rows:
        if gate == "net-gain":
            row["gate_passed"] = row["gain_ci95_percent"][0] > row["aa"]["noise_percent"]
        elif gate == "noninferior":
            row["gate_passed"] = row["gain_ci95_percent"][0] > -NONINFERIORITY_PERCENT
    return {
        "schema": "oxide-paired-gains-v1", "metric": metric,
        "baseline": baseline, "candidate": candidate,
        "statistics": {
            "estimator": "median of paired percent gains; positive means better",
            "formula": "100 * (1 - candidate / baseline)" if metric == "fixed-time"
                       else "100 * (candidate / baseline - 1)",
            "resamples": BOOTSTRAP_RESAMPLES, "seed": BOOTSTRAP_SEED,
            "interval": "empirical percentile 95%; indices 249 and 9749",
            "minimum_pairs": MINIMUM_PAIRS,
            "aa_noise": "maximum absolute endpoint of matched A/A gain 95% interval",
        },
        "gate": gate,
        "gate_passed": all(row["gate_passed"] for row in rows) if gate else None,
        "noninferiority_margin_percent": NONINFERIORITY_PERCENT if gate == "noninferior" else None,
        "metadata": metadata, "aa_metadata": aa_metadata,
        "results": rows, "combined_subscores": contextual,
    }


def write_markdown(report, output):
    lines = ["# Paired performance comparison", "",
             f"Metric: {report['metric']}. Positive gain means better.", "",
             "Median paired gains; 10,000 bootstrap resamples, seed 79, percentile 95% interval.", "",
             "| Case | Pairs | Baseline median | Candidate median | Gain | 95% interval | A/A noise | Gate |",
             "| --- | ---: | ---: | ---: | ---: | --- | ---: | --- |"]
    for row in report["results"]:
        interval = row["gain_ci95_percent"]
        noise = f"{row['aa']['noise_percent']:.3f}%" if "aa" in row else "unmeasured"
        gate = "pass" if row.get("gate_passed") else "not passed" if "gate_passed" in row else "not evaluated"
        lines.append(f"| {row['case']} | {row['pairs']} | {row['baseline_median']:.6g} | "
                     f"{row['candidate_median']:.6g} | {row['median_gain_percent']:+.3f}% | "
                     f"{interval[0]:+.3f}% to {interval[1]:+.3f}% | {noise} | {gate} |")
    if report["gate"] == "noninferior":
        lines += ["", "Noninferiority requires the gain interval's lower endpoint to exceed −1%. "
                  "A/A noise does not relax this fixed margin; a within-margin slowdown remains a slowdown."]
    if report["combined_subscores"]:
        lines += ["", "## Subscores from the complete combined run", "",
                  "These contextual scores can be compared with a historical combined-run Boa table. "
                  "They do not replace isolated acceptance results.", "",
                  "| Benchmark | Baseline median | Candidate median |",
                  "| --- | ---: | ---: |"]
        for row in report["combined_subscores"]:
            lines.append(f"| {row['measurement']} | {row['baseline_median']:.6g} | {row['candidate_median']:.6g} |")
    with output.open("x") as destination:
        destination.write("\n".join(lines) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, required=True, help="fixed.py or run.py results.json")
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--metric", choices=["fixed-time", "original-score"], required=True)
    parser.add_argument("--aa-results", type=Path, help="same-binary frozen A/A results.json")
    parser.add_argument("--aa-baseline")
    parser.add_argument("--aa-candidate")
    parser.add_argument("--case", action="append", dest="cases")
    parser.add_argument("--require-v8-matrix", action="store_true")
    parser.add_argument("--gate", choices=["net-gain", "noninferior"])
    parser.add_argument("--output", type=Path, required=True, help="new JSON report file")
    args = parser.parse_args()
    if (args.aa_baseline or args.aa_candidate) and not args.aa_results:
        parser.error("A/A engine names require --aa-results")
    if args.output == args.output.with_suffix(".md"):
        parser.error("JSON output must differ from its Markdown companion")
    if args.output.exists() or args.output.with_suffix(".md").exists():
        parser.error("report output already exists")
    try:
        raw = args.results.read_bytes()
        aa_raw = args.aa_results.read_bytes() if args.aa_results else None
        report = paired_report(
            json.loads(raw), args.baseline, args.candidate, args.metric,
            aa_results=json.loads(aa_raw) if aa_raw is not None else None,
            aa_baseline=args.aa_baseline, aa_candidate=args.aa_candidate, gate=args.gate,
            cases=args.cases, require_v8_matrix=args.require_v8_matrix)
    except (OSError, ValueError, TypeError, KeyError) as error:
        parser.error(str(error))
    report["inputs"] = {"results": {"path": str(args.results.resolve()), "sha256": hashlib.sha256(raw).hexdigest()}}
    if args.aa_results:
        report["inputs"]["aa_results"] = {"path": str(args.aa_results.resolve()), "sha256": hashlib.sha256(aa_raw).hexdigest()}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        output.write(json.dumps(report, indent=2, allow_nan=False) + "\n")
    write_markdown(report, args.output.with_suffix(".md"))
    return 0 if report["gate_passed"] is not False else 1


if __name__ == "__main__":
    raise SystemExit(main())
