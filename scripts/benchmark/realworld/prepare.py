"""Admit pinned RealWorld bundles into the existing fixed-work runner."""

import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path


HERE = Path(__file__).resolve().parent
NAMES = ("solid", "vue", "react")
PHASES = ("home", "tag", "page", "article", "favorite", "comment", "profile")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(command, timeout=60, env=None):
    completed = subprocess.run(command, capture_output=True, text=True, timeout=timeout, env=env)
    if completed.returncode or completed.stderr:
        raise ValueError(f"{command[0]} failed: exit={completed.returncode}, stderr={completed.stderr[:400]}")
    return completed.stdout


def prepare(receipt_path, build_receipt_path, engine, output):
    manifest = json.loads((HERE / "manifest.json").read_text())
    expected = json.loads((HERE / "expected.json").read_text())
    receipt = json.loads(receipt_path.read_text())
    build_receipt = json.loads(build_receipt_path.read_text())
    if build_receipt["commit"] != manifest["baseline"] or build_receipt["mode"] != "plain":
        raise ValueError("admission requires a plain build of the frozen source baseline")
    if build_receipt["binary_sha256"] != digest(engine):
        raise ValueError("baseline binary differs from its verified build receipt")
    if receipt["schema"] != manifest["schema"]:
        raise ValueError("bundle receipt schema differs")
    if receipt["manifest_sha256"] != digest(HERE / "manifest.json"):
        raise ValueError("bundle receipt was built from a different manifest")
    if receipt["builder_sha256"] != digest(HERE / "build.mjs"):
        raise ValueError("bundle builder changed after publication")
    for filename, expected_hash in receipt["adapter_sha256"].items():
        if Path(filename).name != filename or digest(HERE / filename) != expected_hash:
            raise ValueError(f"bundle adapter changed: {filename}")
    for name in NAMES:
        if receipt["apps"][name]["source"] != manifest["apps"][name]["commit"]:
            raise ValueError(f"{name}: source pin differs")
    workloads = []
    for name in NAMES:
        bundle = Path(receipt["bundles"][name]["file"]).resolve()
        if digest(bundle) != receipt["bundles"][name]["sha256"]:
            raise ValueError(f"{name}: generated bundle changed")
        wrapper = (
            "globalThis.print = line => { process.stdout.write(line + '\\n'); process.exit(0); }; "
            f"require({json.dumps(str(bundle))});"
        )
        oracle = run(["node", "-e", wrapper], env={**os.environ, "NODE_NO_WARNINGS": "1"})
        oxide = run([str(engine.resolve()), str(bundle)])
        if oracle != oxide or oxide != expected[name]:
            raise ValueError(f"{name}: oracle, baseline, and frozen output differ")
        phases = json.loads(oxide)
        if tuple(row["name"] for row in phases) != PHASES:
            raise ValueError(f"{name}: missing or reordered application phase")
        if any((row.get("nodes", row.get("bytes", 0)) <= 0) for row in phases):
            raise ValueError(f"{name}: empty rendered phase")
        workloads.append({
            "case": f"realworld-{name}", "size": manifest["scenario"]["articles"],
            "path": str(bundle), "sha256": digest(bundle), "expected": expected[name]
        })
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({"metadata": {
        "schema": manifest["schema"], "baseline": manifest["baseline"],
        "baseline_binary_sha256": digest(engine),
        "baseline_build_receipt_sha256": digest(build_receipt_path),
        "receipt_sha256": digest(receipt_path), "expected_sha256": digest(HERE / "expected.json"),
        "workloads": {"workloads": workloads}
    }}, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--build-receipt", type=Path, required=True)
    parser.add_argument("--engine", type=Path, required=True, help="plain-release 90c85e3d qjs")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    prepare(args.receipt.resolve(), args.build_receipt.resolve(),
            args.engine.resolve(), args.output.resolve())


if __name__ == "__main__":
    main()
