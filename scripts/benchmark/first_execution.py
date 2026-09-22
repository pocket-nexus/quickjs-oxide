"""Freeze, then replay synchronous first-execution samples with M0/M1/M2 probes.

No candidate is launched by prepare. Each sample starts a fresh process/Runtime,
compiles once and measures the first Context.execute directly. Existing compile,
whole-process cold and independent RSS cohorts remain separate required evidence.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import shlex

from fixed import load_workloads as load_fixed
from replay import summarize
from run import digest, machine_metadata, run_sample

ROOT = Path(__file__).resolve().parents[2]
PROBE_SOURCE = ROOT / "apps/cli/examples/first_execution_probe.rs"
SCHEMA = "quickjs-oxide.first-execution-run.v1"
PROBE_SCHEMA = "oxide-first-execution-probe.v1"
CPU = 2
REPEAT = 10
MODE_CFGS = {"oxide_quick_projection", "oxide_quick_dispatch"}


def normalized_flags(value, encoded=False):
    tokens = list(value) if isinstance(value, list) else (value.split("\x1f") if encoded else shlex.split(value))
    result = []
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if any(item in token for item in ("profile-generate", "profile-use", "instrument-coverage")):
            raise ValueError("PGO/coverage flags are not ordinary first-execution timing")
        if re.search(r"feature\s*=\s*[\"']?profiling", token):
            raise ValueError("profiling flags are not ordinary first-execution timing")
        if token == "--cfg" and index + 1 < len(tokens) and tokens[index + 1] in MODE_CFGS:
            index += 2
            continue
        if token.startswith("--cfg=") and token[6:] in MODE_CFGS:
            index += 1
            continue
        result.append(token)
        index += 1
    return result


def configuration_signature(build):
    provenance = build.get("build_provenance", {})
    if provenance.get("schema") != "oxide-probe-build-provenance.v1":
        raise ValueError("new build provenance is required; rebuild the probe")
    environment = provenance["environment"]
    tools = provenance.get("compiler_tools", {})
    if any(not item.get("sha256") for item in tools.values()):
        raise ValueError("compiler/wrapper identity is missing")
    for name in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC",
                 "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"):
        if environment.get(name) and name not in tools:
            raise ValueError("compiler/wrapper identity is missing")
    # Explicit shell overrides establish effective LTO/CGU even if the standalone
    # package has no release table. Never infer them from a dependency profile.
    if environment.get("CARGO_PROFILE_RELEASE_LTO") != "fat" or environment.get("CARGO_PROFILE_RELEASE_CODEGEN_UNITS") != "1":
        raise ValueError("ordinary timing requires explicit fat LTO and codegen-units=1 release overrides")
    if build.get("rustflags") != environment.get("RUSTFLAGS") or build.get("encoded_rustflags") != environment.get("CARGO_ENCODED_RUSTFLAGS"):
        raise ValueError("flags disagree with captured build environment")

    def normalize(value, key=""):
        if isinstance(value, dict):
            result = {name: normalize(item, name) for name, item in value.items()}
            return {name: item for name, item in result.items()
                    if not ("rustflags" in name.lower() and item == [])}
        if "rustflags" in key.lower():
            return normalized_flags(value, key == "CARGO_ENCODED_RUSTFLAGS")
        if isinstance(value, list):
            return [normalize(item) for item in value]
        return value

    configurations = []
    for configuration in provenance["cargo_configurations"]:
        relevant = configuration["relevant"]
        if relevant.get("environment"):
            raise ValueError("Cargo [env] compiler overrides require an explicit resolved recipe; use shell overrides")
        configurations.append(normalize(relevant))
    host = re.search(r"^host:\s*(\S+)$", build["rustc"], re.M)
    target = None
    for configuration in configurations:
        if "target" in configuration.get("build", {}):
            target = configuration.get("build", {}).get("target")
    target = environment.get("CARGO_BUILD_TARGET", target)
    command = build["command"]
    if any(token == "--config" or token.startswith("--config=") for token in command):
        raise ValueError("command-line Cargo config requires an explicit resolved recipe")
    normalized_flags(command)
    for index, token in enumerate(command):
        if token == "--target":
            target = command[index + 1]
        elif token.startswith("--target="):
            target = token.split("=", 1)[1]
    if target is None:
        if host is None:
            raise ValueError("compiler receipt lacks a target/host identity")
        target = host[1]
    return dict(rustc=build["rustc"], target=target,
                environment=normalize(environment), configurations=configurations,
                compiler_tools=tools,
                probe_release_profile=normalize(provenance["probe_release_profile"]))


def write_json(path, value):
    with Path(path).open("x") as output:
        json.dump(value, output, indent=2)
        output.write("\n")


def identity(path):
    path = Path(path).resolve(strict=True)
    return dict(path=str(path), sha256=digest(path))


def check_identity(item):
    if digest(item["path"]) != item["sha256"]:
        raise ValueError(f"frozen artifact changed: {item['path']}")


def probe_identity(path):
    binary = identity(path)
    if not os.access(binary["path"], os.X_OK):
        raise ValueError("probe must be executable")
    receipt = identity(Path(binary["path"]).with_suffix(".build.json"))
    build = json.loads(Path(receipt["path"]).read_text())
    required = {"command", "source_sha256", "source_identity", "rustc", "rustflags",
                "encoded_rustflags", "features", "binary_sha256", "cargo_lock_sha256"}
    if not required.issubset(build) or not build["source_identity"] or not build["rustc"]:
        raise ValueError("probe build receipt is missing source/compiler/flags provenance")
    if build["binary_sha256"] != binary["sha256"]:
        raise ValueError("stale probe build receipt")
    if build["source_sha256"] != digest(PROBE_SOURCE):
        raise ValueError("probe source differs from the frozen first-execution implementation")
    if build["features"] or "--release" not in build["command"]:
        raise ValueError("timing requires a release probe with no instrumentation features")
    return dict(**binary, receipt=receipt, build=build, configuration=configuration_signature(build))


def load_workloads(manifest_path, directory=None, cases=None):
    manifest_path = Path(manifest_path).resolve(strict=True)
    manifest = json.loads(manifest_path.read_text())
    if manifest.get("schema") == "quickjs-oxide.quick-resource-inputs.v1":
        workloads = []
        for item in manifest["cases"]:
            source = (Path(directory) if directory else manifest_path.parent) / item["source"]
            expected = manifest_path.parent / item["expected_stdout_file"]
            if digest(source) != item["source_sha256"] or digest(expected) != item["expected_stdout_sha256"]:
                raise ValueError("resource source/oracle bytes changed")
            if expected.read_bytes() != item["expected_stdout_utf8"].encode():
                raise ValueError("resource oracle text differs from recorded bytes")
            if item["expected_exit_code"] != 0 or item["expected_stderr_utf8"]:
                raise ValueError("first-execution cohort requires successful synchronous scripts")
            workloads.append(dict(case=item["id"], path=str(source.resolve()),
                                  sha256=item["source_sha256"], expected=item["expected_stdout_utf8"]))
    else:
        workloads = load_fixed(manifest_path, directory)
    names = [item["case"] for item in workloads]
    if len(names) != len(set(names)) or any(not re.fullmatch(r"[A-Za-z0-9_-]+", name) for name in names):
        raise ValueError("workload names must be unique safe identifiers")
    if cases and (len(cases) != len(set(cases)) or set(cases) - set(names)):
        raise ValueError("requested cases must be unique and present")
    selected = [item for item in workloads if not cases or item["case"] in cases]
    if not selected or any(not isinstance(item["expected"], str) for item in selected):
        raise ValueError("workloads need exact stdout oracles")
    return selected


def schedule(workloads, modes, repeat=REPEAT):
    entries = []
    for case_index, workload in enumerate(workloads):
        for repetition in range(repeat):
            offset = (case_index + repetition) % len(modes)
            for mode in modes[offset:] + modes[:offset]:
                entries.append(dict(index=len(entries), case=workload["case"], engine=mode,
                                    repetition=repetition))
    return entries


def prepare(args):
    if args.timeout <= 0:
        raise ValueError("timeout must be positive")
    if not hasattr(os, "sched_getaffinity") or CPU not in os.sched_getaffinity(0):
        raise ValueError("required CPU 2 is unavailable")
    taskset = shutil.which("taskset")
    if not taskset:
        raise ValueError("taskset is required")
    engines = {}
    for entry in args.engine:
        mode, separator, path = entry.partition("=")
        if not separator or mode not in ("M0", "M1", "M2") or mode in engines:
            raise ValueError("engines must be unique M0=path, M1=path or M2=path entries")
        engines[mode] = probe_identity(path)
    if len(engines) < 2:
        raise ValueError("a comparison requires at least two modes")
    # Mode ordering is stable regardless of argument ordering. Same-binary
    # M0/M1 is permitted for an explicitly labelled A/A noise cohort.
    engines = dict(sorted(engines.items()))
    baseline = next(iter(engines.values()))["configuration"]
    if any(engine["configuration"] != baseline for engine in engines.values()):
        raise ValueError("mode compiler/target/wrapper/release flags differ beyond approved QuickOp cfgs")
    workloads = load_workloads(args.manifest, args.workload_dir, args.case)
    output = args.output.resolve()
    protocol = dict(
        schema=SCHEMA, output=str(output), cpu=CPU, repeat=REPEAT, timeout_seconds=args.timeout,
        runner=identity(__file__), probe_source=identity(PROBE_SOURCE),
        helpers=[identity(Path(__file__).with_name(name)) for name in ("run.py", "replay.py", "fixed.py", "scaling.py", "build_compile_probe.py")],
        taskset=identity(taskset), manifest=identity(args.manifest), workloads=workloads,
        engines=engines, machine=machine_metadata(), schedule=schedule(workloads, list(engines)),
        metric="first_execute_ns: direct monotonic Context.execute interval, once after compile in a fresh Runtime/process",
        included="first-use snapshot setup, nested eval compilation, host calls/stdout and execution-time GC",
        excluded="source I/O, Runtime/Context/helper setup, outer compile/publication, pending jobs, metrics output and teardown",
        synchronous_only="pending_jobs must be zero; jobs drained outside measured interval, then async samples rejected",
        separate_evidence="compile_ns is retained as cohort diagnostic only; process_wall_ns is separate; neither replaces dedicated compile/cold/RSS evidence and no subtraction is used",
        build_admission="source/compiler/flags/lock/binary receipts frozen; equal compiler/target/wrapper/release recipe except oxide_quick_projection/oxide_quick_dispatch cfgs; explicit fat LTO/CGU1; no PGO/profiling; mode labels do not prove dispatch configuration",
        ratio="after/before median first_execute_ns; lower is better; every scheduled sample must pass exact stdout/empty stderr/exit/metrics admission",
    )
    output.mkdir(parents=True, exist_ok=False)
    (output / "raw").mkdir()
    write_json(output / "protocol.json", protocol)
    (output / "protocol.sha256").write_text(digest(output / "protocol.json") + "\n")
    print(output / "protocol.json")
    return 0


def admit(sample, workload, metrics_path):
    if sample["timed_out"]:
        return "timeout", {}, None
    if sample["exit_code"] != 0:
        return "failed", {}, None
    if Path(sample["stderr"]).read_bytes():
        return "unexpected-stderr", {}, None
    if Path(sample["stdout"]).read_bytes() != workload["expected"].encode():
        return "invalid-stdout", {}, None
    try:
        metric = json.loads(Path(metrics_path).read_text())
    except (OSError, ValueError):
        return "missing-or-malformed-metrics", {}, None
    required = {"schema", "compile_ns", "first_execute_ns", "compile_count", "execute_count", "pending_jobs", "profiling"}
    if not isinstance(metric, dict) or set(metric) != required or metric["schema"] != PROBE_SCHEMA:
        return "invalid-metrics", {}, metric
    integers = ("compile_ns", "first_execute_ns", "compile_count", "execute_count", "pending_jobs")
    if any(type(metric[name]) is not int or metric[name] < 0 for name in integers):
        return "invalid-metrics", {}, metric
    if metric["compile_ns"] == 0 or metric["first_execute_ns"] == 0 or metric["compile_count"] != 1 or metric["execute_count"] != 1:
        return "invalid-metrics", {}, metric
    if metric["profiling"] is not False:
        return "instrumented-probe", {}, metric
    if metric["pending_jobs"]:
        return "unsupported-pending-jobs", {}, metric
    return "ok", {"first_execute_ns": metric["first_execute_ns"]}, metric


def run(args):
    output = args.output.resolve(strict=True)
    protocol_path = output / "protocol.json"
    if digest(protocol_path) != (output / "protocol.sha256").read_text().strip():
        raise ValueError("frozen protocol changed")
    protocol = json.loads(protocol_path.read_text())
    if protocol["schema"] != SCHEMA or protocol["output"] != str(output):
        raise ValueError("protocol identity/output mismatch")
    if CPU not in os.sched_getaffinity(0):
        raise ValueError("required CPU 2 is unavailable")
    frozen = [protocol[key] for key in ("runner", "probe_source", "taskset", "manifest")]
    frozen += protocol["helpers"]
    for engine in protocol["engines"].values():
        frozen += [engine, engine["receipt"]]
    frozen += [dict(path=item["path"], sha256=item["sha256"]) for item in protocol["workloads"]]
    for item in frozen:
        check_identity(item)
    samples = []
    workloads = {item["case"]: item for item in protocol["workloads"]}
    # Exclusive creation prevents accidental resumption/rerun contamination.
    with (output / "samples.jsonl").open("x") as journal:
        write_json(output / "machine-before-sampling.json", machine_metadata())
        for entry in protocol["schedule"]:
            for item in frozen:
                check_identity(item)
            workload = workloads[entry["case"]]
            engine = protocol["engines"][entry["engine"]]
            prefix = output / "raw" / f"{entry['index']:04d}-{entry['case']}-{entry['engine']}-{entry['repetition']}"
            metrics = prefix.with_suffix(".metrics.json")
            command = [protocol["taskset"]["path"], "-c", str(CPU), engine["path"], workload["path"], str(metrics)]
            sample = run_sample(command, output, prefix, protocol["timeout_seconds"])
            status, measurements, diagnostic = admit(sample, workload, metrics)
            sample.update(entry, status=status, measurements=measurements, probe_diagnostic=diagnostic,
                          metrics=str(metrics), workload_sha256=workload["sha256"])
            samples.append(sample)
            journal.write(json.dumps(sample) + "\n")
            journal.flush()
            print(f"{prefix.name}: {status}", flush=True)
    for item in frozen:
        check_identity(item)
    rows, ratios = summarize(samples, list(protocol["engines"]))
    write_json(output / "results.json", dict(protocol=protocol, summary=rows, ratios=ratios, samples=samples))
    return 0 if all(row["eligible"] for row in rows) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--manifest", type=Path, required=True)
    prepare_parser.add_argument("--workload-dir", type=Path)
    prepare_parser.add_argument("--case", action="append")
    prepare_parser.add_argument("--engine", action="append", required=True)
    prepare_parser.add_argument("--timeout", type=float, default=180)
    prepare_parser.add_argument("--output", type=Path, required=True)
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    return prepare(args) if args.command == "prepare" else run(args)


if __name__ == "__main__":
    raise SystemExit(main())
