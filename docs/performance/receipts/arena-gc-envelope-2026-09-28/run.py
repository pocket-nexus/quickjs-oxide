#!/usr/bin/env python3
"""Collect paired host GC capacity samples from instrumented probe binaries."""

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


PHASES = (
    "burst-rooted-gc",
    "small-roots-gc-1",
    "small-roots-gc-2",
    "small-roots-gc-3",
    "regrown-gc",
    "unrooted-gc",
    "context-teardown-gc",
)
COUNTER = re.compile(r"^\s*(\d+)\s+maximum resident set size$")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_stderr(content, mode):
    observations = []
    current = None
    seen = []
    for line in content.splitlines():
        fields = line.split("\t")
        if fields[:1] == ["PROBE_GC_PHASE"]:
            assert len(fields) == 3 and fields[1] == mode, line
            current = fields[2]
            seen.append(current)
        elif fields[:1] == ["OXIDE_GC_CAPACITY_V1"]:
            assert current is not None and len(fields) == 7, line
            observations.append({
                "phase": current,
                "point": fields[1],
                "arena_bytes": int(fields[2]),
                "scratch_bytes": int(fields[3]),
                "zero_queue_bytes": int(fields[4]),
                "cleanup_bytes": int(fields[5]),
                "concurrent_sum_bytes": int(fields[6]),
            })
        else:
            raise ValueError(f"unexpected probe stderr: {line}")
    assert seen == [*PHASES, "runtime-drop"], seen
    for phase in PHASES:
        points = [item["point"] for item in observations if item["phase"] == phase]
        assert points == ["before-finalization", "after-finalization"], (phase, points)
    for item in observations:
        assert item["concurrent_sum_bytes"] == sum(item[key] for key in (
            "arena_bytes", "scratch_bytes", "zero_queue_bytes", "cleanup_bytes"
        ))
    return observations


def parse_stdout(content, mode):
    gc_rows = []
    categories = []
    for line in content.splitlines()[1:]:
        fields = line.split("\t")
        if fields[0] == "GC":
            assert len(fields) == 9 and fields[1] == mode, line
            gc_rows.append({
                "phase": fields[2], "elapsed_ns": int(fields[3]),
                "examined_nodes": int(fields[4]),
                "external_root_nodes": int(fields[5]),
                "candidate_nodes": int(fields[6]),
                "finalized_var_refs": int(fields[7]),
                "finalized_shapes": int(fields[8]),
            })
        elif fields[0] == "CATEGORY":
            assert len(fields) == 7 and fields[1] == mode, line
            categories.append(fields)
        else:
            raise ValueError(f"unexpected probe stdout: {line}")
    assert [row["phase"] for row in gc_rows] == list(PHASES), gc_rows
    return categories, gc_rows


def validate_semantic_parity(samples):
    """All paired runs must report the same phase-level GC outcomes."""
    expected = {}
    for sample in samples:
        for row in sample["gc_rows"]:
            key = (sample["mode"], row["phase"])
            outcome = {name: value for name, value in row.items()
                       if name not in ("phase", "elapsed_ns")}
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
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    assert args.repeats >= 1
    args.output.mkdir(parents=True, exist_ok=True)
    engines = {"parent": args.parent.resolve(), "candidate": args.candidate.resolve()}
    identities = {name: {"path": str(path), "sha256": digest(path)}
                  for name, path in engines.items()}
    samples = []
    for mode in ("cell", "shape"):
        for repetition in range(args.repeats):
            order = ("parent", "candidate", "candidate", "parent") if repetition % 2 == 0 else (
                "candidate", "parent", "parent", "candidate"
            )
            for position, name in enumerate(order):
                binary = engines[name]
                assert digest(binary) == identities[name]["sha256"], binary
                prefix = args.output / f"{mode}-{repetition}-{position}-{name}"
                command = ["/usr/bin/time", "-l", "-o", str(prefix) + ".time", str(binary), mode]
                host_load = os.getloadavg()
                start = time.monotonic_ns()
                result = subprocess.run(command, capture_output=True, text=True, timeout=args.timeout)
                wall_ns = time.monotonic_ns() - start
                (Path(str(prefix) + ".stdout")).write_text(result.stdout)
                (Path(str(prefix) + ".stderr")).write_text(result.stderr)
                if result.returncode:
                    raise RuntimeError(f"{prefix}: exit {result.returncode}: {result.stderr[-500:]}")
                categories, gc_rows = parse_stdout(result.stdout, mode)
                observations = parse_stderr(result.stderr, mode)
                time_content = Path(str(prefix) + ".time").read_text()
                rss = [int(match.group(1)) for line in time_content.splitlines()
                       if (match := COUNTER.match(line))]
                assert len(rss) == 1, (prefix, time_content)
                sample = {
                    "mode": mode, "repetition": repetition, "position": position,
                    "engine": name, "wall_ns": wall_ns, "maximum_rss_bytes": rss[0],
                    "host_load_average": host_load,
                    "category_rows": len(categories), "gc_rows": gc_rows,
                    "observations": observations,
                    "stdout": prefix.name + ".stdout",
                    "stderr": prefix.name + ".stderr",
                    "time": prefix.name + ".time",
                }
                samples.append(sample)
                print(mode, repetition, position, name,
                      f"{wall_ns / 1e9:.2f}s", rss[0], flush=True)
    validate_semantic_parity(samples)
    report = {
        "schema": "oxide-gc-envelope-host-v1",
        "host": platform.platform(),
        "probe_sha256": digest(Path(__file__).parent / "probe/src/main.rs"),
        "engines": identities,
        "samples": samples,
    }
    archive_raw(args.output, report)
    (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
