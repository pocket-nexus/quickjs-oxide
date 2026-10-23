#!/usr/bin/env python3
"""Run the phase-1 Kani proofs and reject missing or vacuous results."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
VERSION = Path(__file__).with_name("kani-version").read_text().strip()
HARNESSES = {
    "value::verification::" + name
    for name in (
        "int32_round_trip", "bool_round_trip", "special_values_are_distinct",
        "heap_tags_and_payloads_round_trip", "number_bits_and_classification",
    )
} | {
    "heap::values::verification::" + name
    for name in (
        "class_sizes_and_inverse", "rounding_is_minimal", "floor_is_maximal",
        "next_class_is_the_successor", "range_log_round_trip",
        "range_exponent_round_trip", "empty_range_is_canonical",
    )
}


def check_results(report):
    """Accept only the complete inventory with reachable, passing assertions."""
    if report["metadata"]["kani_version"] != VERSION:
        raise ValueError("unexpected Kani version")
    if report["metadata"]["target"] != "x86_64-unknown-linux-gnu":
        raise ValueError("phase 1 is validated only for x86_64 Linux")
    verification = report["verification_results"]
    summary = verification["summary"]
    if summary["status"] != "completed" or any(
        summary[key] != len(HARNESSES)
        for key in ("total_harnesses", "executed", "successful")
    ) or summary["failed"] != 0:
        raise ValueError("incomplete or unsuccessful verification")
    results = verification["results"]
    if len(results) != len(HARNESSES) or {
        item["harness_id"] for item in results
    } != HARNESSES:
        raise ValueError("harness inventory changed")
    for item in results:
        if item["status"] != "Success":
            raise ValueError(f"failed harness: {item['harness_id']}")
        checks = item["checks"]
        if any(check["status"] not in ("Success", "Unreachable") for check in checks):
            raise ValueError(f"unresolved check: {item['harness_id']}")
        assertions = [check for check in checks if
                      check["description"].startswith("assertion failed:")
                      and check["location"].get("file", "").endswith("_kani_tests.rs")]
        if not assertions or any(check["status"] != "Success" for check in assertions):
            raise ValueError(f"missing or unreachable proof assertion: {item['harness_id']}")


def output(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def main():
    version = output("cargo", "kani", "--version")
    if version.splitlines()[0] != f"Kani Rust Verifier {VERSION} (cargo plugin)":
        raise SystemExit(f"Install kani-verifier {VERSION}; found {version}")
    parent = ROOT / "target" / "verification"
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix="phase1-", dir=parent))
    report = directory / "results.json"
    command = [
        "cargo", "kani", "-p", "quickjs-oxide-core", "--lib", "--tests",
        "--harness", "value::verification::", "--harness", "heap::values::verification::",
        "--default-unwind", "2", "--force-build", "--output-format", "terse",
        "-Z", "unstable-options", "--harness-timeout", "60s",
        "--export-json", str(report),
    ]
    # Record bytes as well as HEAD: local proofs may run on uncommitted changes.
    sources = {ROOT / name for name in ("Cargo.toml", "core/Cargo.toml", "regexp/Cargo.toml")}
    sources.update(ROOT / name for name in ("Cargo.lock", "rust-toolchain.toml"))
    for folder in ("core/src", "regexp/src", "scripts/verification"):
        sources.update(path for path in (ROOT / folder).rglob("*")
                       if path.is_file() and "__pycache__" not in path.parts)
    provenance = {
        "commit": output("git", "rev-parse", "HEAD"),
        "worktree_status": output("git", "status", "--short"),
        "command": command,
        "kani_version_output": version,
        "sources": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in sorted(sources)},
    }
    (directory / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print(f"Proof receipts: {directory}", flush=True)
    with (directory / "run.log").open("w") as log:
        with subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, text=True) as process:
            for line in process.stdout:
                print(line, end="", flush=True)
                log.write(line)
            code = process.wait()
    if code:
        raise SystemExit(code)
    check_results(json.loads(report.read_text()))
    print(f"Phase 1: {len(HARNESSES)} harnesses passed; receipts: {directory}")


if __name__ == "__main__":
    main()
