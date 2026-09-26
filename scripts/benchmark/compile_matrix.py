#!/usr/bin/env python3
"""Cross-engine front-end compile/parse matrix over an external corpus.

Every engine compiles the same Script-goal files in a fresh process and prints
one exact line (compile_ns:N or parse_ns:N). Runtime/Context construction,
source I/O and teardown stay outside the timed interval. Corpora and engine
builds live outside this repository; results are directional, not upstream
scores.
"""
import argparse
import json
import math
import os
import re
import statistics
import time
from pathlib import Path

from run import ROOT, binary_metadata, command_output, digest, machine_metadata, run_sample

METRICS = ("compile", "parse")
VERSION_MAGIC = {"oxide-parse-probe": "oxide-parser", "boa-parse-probe": "boa-parser",
                 "oxide-compile-probe": "oxide", "quickjs-compile-probe": "quickjs",
                 "boa-compile-probe": "boa"}
NODE_VERSION = re.compile(r"v\d+\.\d+\.\d+")
NODE_PROBE = ROOT / "scripts/benchmark/probes/node_compile_probe.mjs"
OUTPUT_LINE = {"compile": re.compile(r"compile_ns:(\d+)\n"), "parse": re.compile(r"parse_ns:(\d+)\n")}
UNSUPPORTED_METRIC = {"oxide-parser": ("compile",), "boa-parser": ("compile",), "oxide": ("parse",), "quickjs": ("parse",)}


def classify(version_text):
    text = version_text.strip()
    for magic, engine in VERSION_MAGIC.items():
        if text.startswith(magic):
            return engine
    if NODE_VERSION.fullmatch(text):
        return "node"
    raise ValueError(f"unrecognized probe version: {text!r}")


def engine_command(engine_type, binary, metric, path):
    if engine_type == "node":
        flag = "--no-lazy" if metric == "compile" else "--parse-only"
        return [str(binary), flag, str(NODE_PROBE), metric, str(path)]
    if engine_type == "boa":
        return [str(binary), metric, str(path)]
    return [str(binary), str(path)]


def admit(sample, metric):
    if sample["timed_out"]:
        return "timeout", {}
    if sample["exit_code"] != 0:
        return "failed", {}
    if Path(sample["stderr"]).read_text(errors="replace").strip():
        return "unexpected-stderr", {}
    text = Path(sample["stdout"]).read_text(errors="replace")
    match = OUTPUT_LINE[metric].fullmatch(text)
    if match is None or int(match[1]) <= 0:
        return "invalid-output", {}
    return "ok", {f"{metric}_ns": int(match[1])}


def load_corpus(directory, cases):
    directory = directory.resolve()
    manifest_path = directory / "manifest.json"
    workloads = []
    if manifest_path.is_file():
        manifest = json.loads(manifest_path.read_text())
        if manifest.get("schema") != "oxide-compile-corpus-v1":
            raise ValueError(f"unrecognized corpus manifest: {manifest_path}")
        for entry in manifest["workloads"]:
            path = Path(entry["path"])
            if digest(path) != entry["sha256"] or path.stat().st_size != entry["bytes"]:
                raise ValueError(f"corpus file changed since manifest: {path}")
            workloads.append(dict(entry))
    else:
        for path in sorted(directory.glob("*.js")):
            workloads.append({"case": path.stem, "path": str(path), "bytes": path.stat().st_size,
                              "sha256": digest(path)})
    if not workloads:
        raise ValueError(f"no workloads found in {directory}")
    selected = cases or [workload["case"] for workload in workloads]
    if len(set(selected)) != len(selected) or set(selected) - {workload["case"] for workload in workloads}:
        raise ValueError("--case must select unique corpus workloads")
    return [workload for workload in workloads if workload["case"] in selected]


def summarize(samples, engines, workloads):
    by_case = {workload["case"]: workload for workload in workloads}
    groups = {}
    for sample in samples:
        key = (sample["case"], sample["engine"])
        group = groups.setdefault(key, {"case": key[0], "engine": key[1], "samples": 0,
                                        "successful": 0, "failures": [], "values": []})
        group["samples"] += 1
        if sample["status"] != "ok":
            group["failures"].append(sample["status"])
            continue
        group["successful"] += 1
        group["values"].extend(sample["measurements"].values())
    results = []
    for group in groups.values():
        values = group.pop("values")
        group["eligible"] = group["successful"] == group["samples"]
        group["bytes"] = by_case[group["case"]]["bytes"]
        group["metrics"] = {"ns": {"raw": values}, "mb_per_second": None}
        if values:
            median = statistics.median(values)
            group["metrics"] = {"ns": {"raw": values, "minimum": min(values), "median": median,
                                       "maximum": max(values),
                                       "stdev": statistics.stdev(values) if len(values) > 1 else None},
                                "mb_per_second": group["bytes"] * 1000.0 / median}
        results.append(group)
    by_key = {(group["case"], group["engine"]): group for group in results}
    reference = engines[0]
    ratios = []
    for case in dict.fromkeys(group["case"] for group in results):
        first, second = by_key.get((case, reference)), None
        for name in engines[1:]:
            second = by_key.get((case, name))
            if first and second and first["eligible"] and second["eligible"]:
                ratios.append({"case": case, "engine": name, "reference": reference,
                               "ratio": first["metrics"]["ns"]["median"] / second["metrics"]["ns"]["median"]})
    return results, ratios


def complete_geomeans(ratios, engines, workloads):
    """Only a complete, fixed corpus qualifies; never average a surviving subset."""
    expected = {workload["case"] for workload in workloads}
    aggregates = []
    for engine in engines[1:]:
        selected = [ratio for ratio in ratios if ratio["engine"] == engine]
        if len(selected) == len(expected) and {item["case"] for item in selected} == expected:
            aggregates.append(dict(engine=engine, reference=engines[0], cases=len(expected),
                                   ratio=math.exp(statistics.mean(math.log(item["ratio"]) for item in selected))))
    return aggregates


def write_report(output, results, ratios, metric, engines, aggregates, boundary):
    construction = ("Parser/interner construction is included; no runtime or context is created. "
                    if boundary == "complete-syntax-parser-with-initialization"
                    else "Runtime/Context construction is excluded. ")
    lines = [f"# Front-end {metric} matrix", "",
             "Each sample is one fresh process; the probe prints exactly one ns line. "
             + construction + "Source I/O and teardown are excluded. "
             "Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.",
             "", "| Case | Bytes | Engine | Runs | Median ns | MB/s |", "| --- | ---: | --- | ---: | ---: | ---: |"]
    for group in sorted(results, key=lambda item: (item["case"], engines.index(item["engine"]))):
        status = f"{group['successful']}/{group['samples']}"
        if not group["metrics"]["ns"]["raw"]:
            lines.append(f"| {group['case']} | {group['bytes']} | {group['engine']} | {status} | unavailable ({', '.join(group['failures'])}) | — |")
            continue
        metric_values = group["metrics"]
        lines.append(f"| {group['case']} | {group['bytes']} | {group['engine']} | {status} | "
                     f"{metric_values['ns']['median']:.6g} | {metric_values['mb_per_second']:.4g} |")
    if ratios:
        lines.extend(["", f"Speed relative to {engines[0]} (only cases fully successful on both engines):", ""])
        for ratio in ratios:
            lines.append(f"- {ratio['case']}/{ratio['engine']}: {ratio['ratio']:.4g}×")
    if aggregates:
        lines.extend(["", "Geometric means over the complete selected corpus:", ""])
        for aggregate in aggregates:
            lines.append(f"- {aggregate['engine']}: {aggregate['ratio']:.4g}× {aggregate['reference']} ({aggregate['cases']} cases)")
    lines.extend(["", "Failed, incomplete and timed-out samples stay visible and are excluded from ratios. "
                      "This report never infers a whole-engine score from a subset.", ""])
    (output / "report.md").write_text("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--metric", choices=METRICS, default="compile")
    parser.add_argument("--engine", action="append", required=True, help="NAME=/path/to/probe (repeat)")
    parser.add_argument("--case", action="append", dest="cases")
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--output", type=Path, required=True, help="new result directory")
    args = parser.parse_args()
    if args.repeat < 1 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("repeat and timeout must be positive and finite")
    engines = {}
    for item in args.engine:
        name, sep, path = item.partition("=")
        if not sep or not re.fullmatch(r"[A-Za-z0-9_-]+", name) or name in engines:
            parser.error("use unique NAME=/path/to/probe engine specifications")
        # Do not resolve symlinks: argv[0] basename selects behaviour for
        # multiplexer shims such as vite-plus's `node`.
        binary = Path(os.path.abspath(path))
        if not binary.is_file() or not os.access(binary, os.X_OK):
            parser.error(f"engine is not an executable file: {binary}")
        engines[name] = binary
    engine_types = {}
    for name, binary in engines.items():
        try:
            engine_types[name] = classify(command_output([str(binary), "--version"])["stdout"])
        except ValueError as error:
            parser.error(f"{name}: {error}")
        if args.metric in UNSUPPORTED_METRIC.get(engine_types[name], ()):
            parser.error(f"{name}: {engine_types[name]} has no public {args.metric}-only entry point")
    parser_types = {"oxide-parser", "boa-parser"}
    if any(kind in parser_types for kind in engine_types.values()) and not all(
            kind in parser_types for kind in engine_types.values()):
        parser.error("parser-only probes cannot be mixed with parse+scope or compile probes")
    if "node" in engine_types.values() and not NODE_PROBE.is_file():
        parser.error(f"missing node probe: {NODE_PROBE}")
    try:
        workloads = load_corpus(args.corpus, args.cases)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    output = args.output.resolve()
    if output.exists():
        parser.error(f"output directory already exists: {output}")
    output.mkdir(parents=True)
    (output / "raw").mkdir()
    metadata = {"schema": "oxide-compile-matrix-v1",
                "created_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "metric": args.metric, "goal": "script",
                "boundary": "complete-syntax-parser-with-initialization" if all(
                    kind in parser_types for kind in engine_types.values()) else "engine-public-api", "repeat": args.repeat, "timeout_seconds": args.timeout,
                "order": "serial, alternating per repetition", "instrumentation": "off (no profiling flags)",
                "machine": machine_metadata(), "corpus": str(args.corpus.resolve()), "workloads": workloads,
                "engines": {name: {"type": engine_types[name], **binary_metadata(path)}
                            for name, path in engines.items()},
                "node_probe_sha256": digest(NODE_PROBE) if NODE_PROBE.is_file() else None,
                "runner_sha256": digest(__file__)}
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    samples = []
    with (output / "samples.jsonl").open("w") as journal:
        for workload in workloads:
            for iteration in range(args.repeat):
                order = list(engines) if iteration % 2 == 0 else list(reversed(engines))
                for name in order:
                    prefix = output / "raw" / f"{workload['case']}-{name}-{iteration}"
                    command = engine_command(engine_types[name], engines[name], args.metric, workload["path"])
                    sample = run_sample(command, output, prefix, args.timeout)
                    sample.update(case=workload["case"], engine=name, repetition=iteration,
                                  workload_sha256=workload["sha256"])
                    status, measurements = admit(sample, args.metric)
                    sample.update(status=status, measurements=measurements)
                    samples.append(sample)
                    journal.write(json.dumps(sample) + "\n")
                    journal.flush()
                    print(f"{workload['case']} {name} #{iteration + 1}: {status}", flush=True)
    results, ratios = summarize(samples, list(engines), workloads)
    aggregates = complete_geomeans(ratios, list(engines), workloads)
    (output / "results.json").write_text(json.dumps({**metadata, "samples": samples, "summary": results,
                                                     "ratios": ratios, "complete_geomeans": aggregates}, indent=2) + "\n")
    write_report(output, results, ratios, args.metric, list(engines), aggregates, metadata["boundary"])
    print(output / "report.md")
    return 0 if all(sample["status"] == "ok" for sample in samples) else 1


if __name__ == "__main__":
    raise SystemExit(main())
