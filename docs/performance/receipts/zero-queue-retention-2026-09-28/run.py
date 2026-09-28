#!/usr/bin/env python3
"""Collect paired zero-queue retention and GC-capacity observations."""

import argparse
import gzip
import hashlib
import io
import json
import os
import platform
import re
import subprocess
import tarfile
import time
from pathlib import Path

COUNTER = re.compile(r"^\s*(\d+)\s+maximum resident set size$")
MODES = ("cell", "shape")
SCENARIOS = ("post-burst", "repeated-burst")
SEMANTIC_FIELDS = (
    "examined_nodes", "external_root_nodes", "candidate_nodes",
    "finalized_objects", "finalized_shapes", "finalized_var_refs",
    "finalized_contexts", "finalized_function_bytecodes",
    "finalized_strings", "finalized_bigints",
)


def phases(scenario):
    if scenario == "post-burst":
        return (
            "burst-rooted-gc", "small-roots-gc-1", "small-roots-gc-2",
            "small-roots-gc-3", "regrown-gc", "unrooted-gc",
            "context-teardown-gc",
        )
    return tuple(
        phase
        for pass_no in range(1, 4)
        for phase in (
            f"burst-{pass_no}-rooted-gc",
            f"burst-{pass_no}-small-gc",
            f"burst-{pass_no}-unrooted-gc",
        )
    ) + ("context-teardown-gc",)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_number(value):
    return None if value == "-" else int(value)


def parse_stdout(content, mode, scenario, require_growth):
    categories = []
    gc_rows = []
    lines = content.splitlines()
    assert lines and lines[0].startswith("record\t"), lines[:1]
    for line in lines[1:]:
        fields = line.split("\t")
        if fields[0] == "CATEGORY":
            assert len(fields) == 8 and fields[1:3] == [mode, scenario], line
            categories.append({
                "phase": fields[3], "name": fields[4],
                "count": parse_number(fields[5]),
                "used_bytes": parse_number(fields[6]),
                "capacity_bytes": parse_number(fields[7]),
            })
        elif fields[0] == "GC":
            assert len(fields) == 15 and fields[1:3] == [mode, scenario], line
            row = {"phase": fields[3], "elapsed_ns": int(fields[4])}
            values = list(map(int, fields[5:]))
            assert len(values) == len(SEMANTIC_FIELDS), line
            row.update(zip(SEMANTIC_FIELDS, values))
            gc_rows.append(row)
        else:
            raise ValueError(f"unexpected probe stdout: {line}")
    assert [row["phase"] for row in gc_rows] == list(phases(scenario)), gc_rows
    required = {"arena_zero_queue"}
    if require_growth:
        required.add("zero_queue_growth_events")
    for phase in phases(scenario):
        for point in (f"{phase}-before", phase):
            names = {row["name"] for row in categories if row["phase"] == point}
            assert required <= names, (point, names)
    return categories, gc_rows


def parse_stderr(content, mode, scenario, require_capacity):
    observations = []
    seen = []
    current = None
    for line in content.splitlines():
        fields = line.split("\t")
        if fields[0] == "PROBE_GC_PHASE":
            assert len(fields) == 4 and fields[1:3] == [mode, scenario], line
            current = fields[3]
            seen.append(current)
        elif fields[0] == "OXIDE_GC_CAPACITY_V1":
            assert current is not None and len(fields) == 7, line
            observation = {
                "phase": current, "point": fields[1],
                "arena_bytes": int(fields[2]), "scratch_bytes": int(fields[3]),
                "zero_queue_bytes": int(fields[4]), "cleanup_bytes": int(fields[5]),
                "concurrent_sum_bytes": int(fields[6]),
            }
            assert observation["concurrent_sum_bytes"] == sum(observation[key] for key in (
                "arena_bytes", "scratch_bytes", "zero_queue_bytes", "cleanup_bytes"
            )), observation
            observations.append(observation)
        else:
            raise ValueError(f"unexpected probe stderr: {line}")
    assert seen == [*phases(scenario), "runtime-drop"], seen
    for phase in (*phases(scenario), "runtime-drop"):
        points = [row["point"] for row in observations if row["phase"] == phase]
        expected = ["before-finalization", "after-finalization"] if require_capacity else []
        assert points == expected, (phase, points)
    return observations


def validate_semantic_parity(samples):
    expected = {}
    for sample in samples:
        for row in sample["gc_rows"]:
            key = (sample["mode"], sample["scenario"], row["phase"])
            outcome = {field: row[field] for field in SEMANTIC_FIELDS}
            if key in expected and expected[key] != outcome:
                raise ValueError(f"GC outcome differs at {key}: {expected[key]} != {outcome}")
            expected[key] = outcome


def archive_raw(output, report):
    names = sorted({sample[key] for sample in report["samples"]
                    for key in ("stdout", "stderr", "time")})
    hashes = {}
    archive = output / "raw-samples.tar.gz"
    with archive.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as bundle:
                for name in names:
                    content = (output / name).read_bytes()
                    hashes[name] = hashlib.sha256(content).hexdigest()
                    entry = tarfile.TarInfo(name)
                    entry.size = len(content)
                    entry.mode = 0o644
                    entry.mtime = 0
                    bundle.addfile(entry, io.BytesIO(content))
    report["raw_archive"] = archive.name
    report["raw_sha256"] = hashes
    for name in names:
        (output / name).unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=180)
    parser.add_argument("--timing-only", action="store_true",
                        help="use uninstrumented release probes; omit queue-growth and in-GC capacity records")
    args = parser.parse_args()
    assert args.repeats >= 1
    args.output.mkdir(parents=True, exist_ok=True)
    engines = {"parent": args.parent.resolve(), "candidate": args.candidate.resolve()}
    identities = {name: {"path": str(path), "sha256": digest(path)}
                  for name, path in engines.items()}
    samples = []
    for mode in MODES:
        for scenario in SCENARIOS:
            for repetition in range(args.repeats):
                order = ("parent", "candidate", "candidate", "parent") if repetition % 2 == 0 else (
                    "candidate", "parent", "parent", "candidate"
                )
                for position, name in enumerate(order):
                    binary = engines[name]
                    assert digest(binary) == identities[name]["sha256"], binary
                    prefix = args.output / f"{mode}-{scenario}-{repetition}-{position}-{name}"
                    command = [
                        "/usr/bin/time", "-l", "-o", str(prefix) + ".time",
                        str(binary), mode, scenario,
                    ]
                    host_load = os.getloadavg()
                    start = time.monotonic_ns()
                    result = subprocess.run(command, capture_output=True, text=True,
                                            timeout=args.timeout)
                    wall_ns = time.monotonic_ns() - start
                    (Path(str(prefix) + ".stdout")).write_text(result.stdout)
                    (Path(str(prefix) + ".stderr")).write_text(result.stderr)
                    if result.returncode:
                        raise RuntimeError(f"{prefix}: exit {result.returncode}: {result.stderr[-500:]}")
                    categories, gc_rows = parse_stdout(
                        result.stdout, mode, scenario, not args.timing_only)
                    observations = parse_stderr(
                        result.stderr, mode, scenario, not args.timing_only)
                    time_content = Path(str(prefix) + ".time").read_text()
                    rss = [int(match.group(1)) for line in time_content.splitlines()
                           if (match := COUNTER.match(line))]
                    assert len(rss) == 1, (prefix, time_content)
                    samples.append({
                        "mode": mode, "scenario": scenario, "repetition": repetition,
                        "position": position, "engine": name, "wall_ns": wall_ns,
                        "maximum_rss_bytes": rss[0], "host_load_average": host_load,
                        "categories": categories, "gc_rows": gc_rows,
                        "observations": observations,
                        "stdout": prefix.name + ".stdout",
                        "stderr": prefix.name + ".stderr",
                        "time": prefix.name + ".time",
                    })
                    print(mode, scenario, repetition, position, name,
                          f"{wall_ns / 1e9:.2f}s", rss[0], flush=True)
    validate_semantic_parity(samples)
    report = {
        "schema": "oxide-zero-queue-retention-host-v1",
        "instrumented": not args.timing_only,
        "host": platform.platform(),
        "probe_sha256": digest(Path(__file__).parent / "probe/src/main.rs"),
        "engines": identities, "samples": samples,
    }
    archive_raw(args.output, report)
    (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
