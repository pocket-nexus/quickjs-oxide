"""Stress code placement at 0/16/32/48 bytes, retaining time, cycles and instructions.

Saved LTO builds are relinked with relaxation disabled. Within each engine all
four variants must have identical .text bytes. Native release Claim results
remain separate: these diagnostic median gates do not replace paired A/A+A/B.
"""
import argparse
import contextlib
import io
import json
import math
from pathlib import Path
import platform
import statistics

from . import relink, runner
from .common import ROOT, digest, paired_order
from .sweep import V8_CASES

OFFSETS = (0, 16, 32, 48)
METRICS = ("time", "cycles", "instructions")


def matrix_medians(results):
    """Reject failed, incomplete or reordered data before reporting a placement."""
    meta = results["metadata"]
    if set(meta["engines"]) != {"base", "candidate"} or meta.get("order_strategy") != "abba":
        raise ValueError("layout needs exactly base/candidate in ABBA order")
    repeat = meta["repeat"]
    if type(repeat) is not int or repeat < 4 or repeat % 2:
        raise ValueError("layout needs at least four repetitions in complete ABBA blocks")
    workloads = meta["workloads"]
    cases = [w["case"] for w in workloads]
    if not cases or len(cases) != len(set(cases)):
        raise ValueError("layout needs unique nonempty workloads")
    expected = [(case, engine, rep) for case in cases for rep in range(repeat)
                for engine in paired_order(list(meta["engines"]), rep, "abba")]
    samples = results["samples"]
    if [(s.get("case"), s.get("engine"), s.get("repetition")) for s in samples] != expected:
        raise ValueError("incomplete or reordered layout matrix")
    values = {(case, engine, metric): [] for case in cases for engine in meta["engines"] for metric in METRICS}
    for sample in samples:
        if sample.get("status") != "ok" or sample.get("exit_code") != 0 \
                or sample.get("timed_out") is not False or sample.get("semantic_status") != "ok":
            raise ValueError("failed layout sample; inspect raw stdout/stderr")
        counters = sample.get("hardware_counters", {}).get("values", {})
        observed = {"time": sample.get("process_wall_ns"),
                    "cycles": counters.get("cycles"), "instructions": counters.get("instructions")}
        for metric, value in observed.items():
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
                raise ValueError(f"missing or invalid layout {metric}")
            values[sample["case"], sample["engine"], metric].append(value)
    return {case: {engine: {metric: statistics.median(values[case, engine, metric]) for metric in METRICS}
                   for engine in meta["engines"]} for case in cases}


def summarize(matrices, max_spread, max_regression):
    if tuple(matrices) != OFFSETS:
        raise ValueError("layout requires offsets 0,16,32,48 in that order")
    medians = {offset: matrix_medians(data) for offset, data in matrices.items()}
    first = matrices[0]["metadata"]
    identity = [(w["case"], w["sha256"]) for w in first["workloads"]]
    for data in matrices.values():
        meta = data["metadata"]
        if [(w["case"], w["sha256"]) for w in meta["workloads"]] != identity \
                or any(meta[k] != first[k] for k in ("repeat", "cpu", "linux_counters")):
            raise ValueError("layout inputs or measurement settings changed between offsets")
    rows = []
    for case, _ in identity:
        engines = {}
        for engine in ("base", "candidate"):
            engines[engine] = {}
            for metric in METRICS:
                values = [medians[d][case][engine][metric] for d in OFFSETS]
                engines[engine][metric] = dict(min=min(values), median=statistics.median(values), max=max(values),
                                               spread_percent=100 * (max(values) / min(values) - 1))
        ratios = {metric: [medians[d][case]["candidate"][metric] / medians[d][case]["base"][metric]
                           for d in OFFSETS] for metric in METRICS}
        failures = []
        for metric in ("time", "cycles"):
            if engines["candidate"][metric]["spread_percent"] > max_spread:
                failures.append(f"{metric} spread")
            if 100 * (max(ratios[metric]) - 1) > max_regression:
                failures.append(f"{metric} regression")
        rows.append(dict(case=case, engines=engines, ratios=ratios, failures=failures,
                         placements=[dict(offset=d, **medians[d][case]) for d in OFFSETS]))
    return dict(schema="oxide-layout-v1", offsets=list(OFFSETS), rows=rows,
                thresholds=dict(max_spread_percent=max_spread, max_regression_percent=max_regression),
                gate_passed=not any(row["failures"] for row in rows))


def markdown(report):
    lines = ["# Layout pressure", "", "Diagnostic median gates; original native Claim results remain authoritative.", "",
             "| Case | Offset | Base ms | Candidate ms | Time ratio | Cycles ratio | Instructions ratio |",
             "|---|---:|---:|---:|---:|---:|---:|"]
    for row in report["rows"]:
        for p in row["placements"]:
            b, c = p["base"], p["candidate"]
            lines.append(f"| {row['case']} | {p['offset']} | {b['time']/1e6:.2f} | {c['time']/1e6:.2f} | "
                         f"{c['time']/b['time']:.4f} | {c['cycles']/b['cycles']:.4f} | {c['instructions']/b['instructions']:.4f} |")
    lines += ["", "| Case | Base time spread | Candidate time spread | Candidate cycles spread | Worst time ratio | Gate |",
              "|---|---:|---:|---:|---:|---|"]
    for row in report["rows"]:
        b, c = row["engines"]["base"], row["engines"]["candidate"]
        lines.append(f"| {row['case']} | {b['time']['spread_percent']:.2f}% | {c['time']['spread_percent']:.2f}% | "
                     f"{c['cycles']['spread_percent']:.2f}% | {max(row['ratios']['time']):.4f} | "
                     + (", ".join(row["failures"]) or "pass") + " |")
    t = report["thresholds"]
    lines += ["", f"Candidate spread limit: {t['max_spread_percent']}%; worst matched-offset regression limit: "
              f"{t['max_regression_percent']}%, both for time and cycles.",
              "Gate: " + ("pass" if report["gate_passed"] else "FAIL"), ""]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py layout", description=__doc__)
    parser.add_argument("--base-log", type=Path, required=True)
    parser.add_argument("--candidate-log", type=Path, required=True)
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--case", action="append", help="suite cases; default all eight V8 v7 programs")
    parser.add_argument("--fixed-case", action="append", help="fixed cases; default numeric_local_loop and array_read")
    parser.add_argument("--repeat", type=int, default=4)
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--max-spread-percent", type=float, default=3)
    parser.add_argument("--max-regression-percent", type=float, default=2)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if platform.system() != "Linux" or args.repeat < 4 or args.repeat % 2 or args.cpu < 0:
        parser.error("layout requires Linux perf, a CPU and at least four even repetitions")
    if any(not math.isfinite(x) or x < 0 for x in (args.max_spread_percent, args.max_regression_percent)):
        parser.error("layout thresholds must be finite and nonnegative")
    suite_cases = args.case or [f"v8-v7-{c}" for c in V8_CASES]
    fixed_cases = args.fixed_case or ["numeric_local_loop", "array_read"]
    workloads = runner.load_workloads(args.suite, cases=suite_cases)
    fixed = ROOT / "docs/performance/probes/fixed"
    workloads += runner.load_workloads(fixed / "manifest.json", fixed, fixed_cases)
    if len({w["case"] for w in workloads}) != len(workloads):
        parser.error("suite and fixed case names overlap")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    plan = dict(offsets=OFFSETS, repeat=args.repeat, order="abba", cpu=args.cpu,
                max_spread_percent=args.max_spread_percent, max_regression_percent=args.max_regression_percent,
                build_logs={n: dict(path=str(p.resolve()), sha256=digest(p))
                            for n, p in (("base", args.base_log), ("candidate", args.candidate_log))})
    for item in plan["build_logs"].values():
        log = Path(item["path"])
        receipt = log.with_name(log.name.removesuffix(".stdout.log") + ".json")
        if receipt.is_file():
            item["build_receipt"] = json.loads(receipt.read_text())
    (output / "plan.json").write_text(json.dumps(plan, indent=2) + "\n")
    for engine, log in (("base", args.base_log), ("candidate", args.candidate_log)):
        relink.placements(log, output / engine, OFFSETS, binary="qjs", no_relax=True, require_identical=True)
    manifest = output / "manifest.json"
    manifest.write_text(json.dumps(dict(metadata=dict(workloads=dict(workloads=workloads))), indent=2) + "\n")
    matrices = {}
    commands = []
    for offset in OFFSETS:
        print(f"layout +{offset}: running", flush=True)
        command = ["--manifest", str(manifest), "--engine", f"base={output}/base-d{offset}",
                   "--engine", f"candidate={output}/candidate-d{offset}", "--repeat", str(args.repeat),
                   "--order", "abba", "--cpu", str(args.cpu), "--linux-counters", "--output", str(output / f"offset-{offset}")]
        commands.append(["python3", "scripts/benchmark/bench.py", "run", *command])
        (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
        with contextlib.redirect_stdout(io.StringIO()):
            status = runner.main(command)
        if status:
            print(f"failed samples under {output / f'offset-{offset}'}")
            return 1
        matrices[offset] = json.loads((output / f"offset-{offset}/results.json").read_text())
    report = summarize(matrices, args.max_spread_percent, args.max_regression_percent)
    (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    text = markdown(report)
    (output / "summary.md").write_text(text)
    print(text)
    return 0 if report["gate_passed"] else 1
