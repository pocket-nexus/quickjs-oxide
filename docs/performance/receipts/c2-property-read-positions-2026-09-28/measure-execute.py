#!/usr/bin/env python3
"""Replay the C2 compile-once probe with balanced process order."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess


CASES = ("static-missing", "static-string", "computed-object", "computed-frame", "warm-field", "getter")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--probe-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=4)
    parser.add_argument("--cases", nargs="+", choices=CASES, default=CASES)
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error("--repeat must be positive")
    engines = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    rows = []
    for case in args.cases:
        for repetition in range(args.repeat):
            order = ("baseline", "candidate") if repetition % 2 == 0 else ("candidate", "baseline")
            for name in order:
                result = subprocess.run([str(engines[name]), case], capture_output=True, timeout=180)
                lines = result.stdout.decode().splitlines()
                if result.returncode or result.stderr or len(lines) != 9 or any(
                    not line.startswith("execute_ns:") for line in lines
                ):
                    raise RuntimeError(f"inadmissible {case}/{name}/{repetition}: {result!r}")
                values = [int(line.partition(":")[2]) for line in lines]
                if any(value <= 0 for value in values):
                    raise RuntimeError("nonpositive execution sample")
                rows.append({"case": case, "engine": name, "repetition": repetition, "execute_ns": values})
                print(f"{case}/{name}/{repetition}: {statistics.median(values):.0f} ns", flush=True)
    summary = []
    for case in args.cases:
        by_engine = {}
        for name in engines:
            values = [value for row in rows if row["case"] == case and row["engine"] == name
                      for value in row["execute_ns"]]
            by_engine[name] = {"count": len(values), "median_ns": statistics.median(values),
                               "min_ns": min(values), "max_ns": max(values)}
        summary.append({"case": case, "engines": by_engine,
                        "candidate_over_baseline_median": by_engine["candidate"]["median_ns"] /
                        by_engine["baseline"]["median_ns"]})
    output = {"schema": "oxide-c2-execute-v1", "probe_source_sha256": digest(args.probe_source),
              "engines": {name: {"sha256": digest(path), "path": str(path)} for name, path in engines.items()},
              "warmups_per_process": 3, "samples_per_process": 9, "processes_per_engine_case": args.repeat,
              "order": "ABBA", "rows": rows, "summary": summary}
    args.output.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
