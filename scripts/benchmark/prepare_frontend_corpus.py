#!/usr/bin/env python3
"""Freeze the eight V8 Script bundles for the parser-only comparison.

The benchmark checkout and generated bundles stay outside this repository.
Only preparation is performed: no JavaScript is executed or timed.
"""
import argparse
import json
from pathlib import Path
import subprocess

from run import prepare_v8

REVISION = "2034d98fc8c5f8044e186267593f5d5ea5232caf"
CASES = ["richards", "deltablue", "crypto", "raytrace", "earley-boyer", "regexp", "splay", "navier-stokes"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--aggregate", action="store_true", help="separate diagnostic corpus containing only all")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        parser.error(f"expected benchmark revision {REVISION}, got {revision}")
    if subprocess.check_output(["git", "-C", str(args.source), "diff", "HEAD", "--", "v8-v7"]):
        parser.error("benchmark source has local changes")
    args.output.mkdir(parents=True, exist_ok=False)
    workloads, receipt = prepare_v8(args.source, ["all"] if args.aggregate else CASES)
    # Reuse source assembly, but omit the runtime harness's measurement claims.
    receipt.pop("timer")
    receipt.pop("metric")
    for workload in workloads:
        workload.pop("expected")
        workload["bytes"] = Path(workload["path"]).stat().st_size
    manifest = dict(schema="oxide-compile-corpus-v1", workloads=workloads, source=receipt,
                    purpose="diagnostic" if args.aggregate else "fixed parser-only primary corpus")
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(args.output / "manifest.json")


if __name__ == "__main__":
    main()
