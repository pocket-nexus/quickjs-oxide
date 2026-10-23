#!/usr/bin/env python3
"""Run production arena, storage and collector/rooting proof groups."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

from phase1 import ROOT, VERSION, check_results, output

ARENA = {"heap::arena::verification::" + name for name in (
    "arena_free_and_reuse", "arena_write_frame_and_accounting",
    "arena_mark_and_sweep", "arena_sweep_all_and_empty",
    "arena_sweep_quarantine_cycle", "arena_multiword_reuse",
)}
INDUCTIVE = {"heap::arena::verification::" + name for name in (
    "inductive_constructor", "inductive_alloc", "inductive_free",
    "inductive_mark_and_clear", "inductive_write", "inductive_sweep_and_shrink",
)}
STORAGE = {"heap::values::allocation_verification::" + name for name in (
    "live_ranges_one_and_three", "live_ranges_two_and_four", "growth_preserves_contents",
)}
ROOTING = {"heap::gc::verification::" + name for name in (
    "root_stack_scopes", "handles_preserve_other_roots_and_reuse",
)}
COLLECTOR = {"heap::gc::verification::" + name for name in (
    "strong_graph_marking", "ephemeron_requires_reachable_map_and_key",
)}
GROUPS = {"arena": ARENA, "arena-stress": ARENA, "storage": STORAGE, "rooting": ROOTING, "collector": COLLECTOR,
          "arena-inductive": INDUCTIVE, "arena-inductive-stress": INDUCTIVE}


def fixed_array_bounds(loop_listing, source):
    """Only the fixed 64-list traversals need 65; nested loops stay small.

    Identify production loops by their current source line, not a stale CBMC
    mangled name. Insufficient bounds anywhere still fail unwinding checks.
    """
    lines = source.splitlines()
    locations = []
    for text in ("for (class, list) in self.free.iter_mut().enumerate() {",
                 "for list in &mut self.free {"):
        matches = [i + 1 for i, line in enumerate(lines) if line.strip() == text]
        if len(matches) != 1:
            raise ValueError(f"review changed free-list traversal: {text}")
        locations.append(matches[0])
    bounds = set()
    for name, description in re.findall(r"Loop ([^\n]+):\n([^\n]+)", loop_listing):
        if ("std::array::try_from_fn_erased" in description
            or "drop_glue::<[std::vec::Vec<u32>]>" in description
            or "<std::slice::Iter<'_, std::vec::Vec<u32>>" in description
            or any(f"core/src/heap/values.rs line {line} " in description
                   for line in locations)):
            bounds.add(name + ":65")
    if not bounds:
        raise ValueError("no fixed-array loops found; review compiler loop output")
    return sorted(bounds)


def run(command, path):
    with path.open("w") as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        print(path.read_text()[-6000:])
        raise SystemExit(f"command failed ({result.returncode}); {path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("group", choices=GROUPS)
    args = parser.parse_args()
    version = output("cargo", "kani", "--version")
    if version.splitlines()[0] != f"Kani Rust Verifier {VERSION} (cargo plugin)":
        raise SystemExit(f"Expected Kani {VERSION}; found {version}")
    parent = ROOT / "target/verification"
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix=args.group + "-", dir=parent))
    print(f"{args.group}: receipts {directory}", flush=True)
    expected = GROUPS[args.group]
    command = ["cargo", "kani", "-p", "quickjs-oxide-core", "--lib", "--tests"]
    if args.group.endswith("-stress"):
        command += ["--features", "gc-stress"]
    for name in sorted(expected):
        command += ["--harness", name]
    command += ["--exact", "-Z", "unstable-options"]
    sources = {ROOT / p for p in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                                "core/Cargo.toml", "regexp/Cargo.toml")}
    for folder in ("core/src", "regexp/src", "scripts/verification"):
        sources.update(p for p in (ROOT / folder).rglob("*")
                       if p.is_file() and "__pycache__" not in p.parts)
    provenance = {"commit": output("git", "rev-parse", "HEAD"),
                  "worktree_status": output("git", "status", "--short"),
                  "group": args.group, "kani_version_output": version,
                  "sources": {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                              for p in sorted(sources)}, "commands": []}
    def record(cmd):
        provenance["commands"].append(cmd)
        (directory / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    bounds = []
    if args.group in ("storage", "collector"):
        # This is compiler/loop inspection only. Kani's old-format "success"
        # trailer here is NOT a proof result and is never passed to the gate.
        inspect = command + ["--force-build", "--output-format", "old",
                             "--cbmc-args", "--show-loops"]
        record(inspect)
        run(inspect, directory / "loop-inspection.log")
        bounds = fixed_array_bounds((directory / "loop-inspection.log").read_text(),
                                    (ROOT / "core/src/heap/values.rs").read_text())
    report = directory / "results.json"
    verify = command + ["--output-format", "terse", "--harness-timeout", "300s",
                        "--export-json", str(report)]
    if bounds:
        verify += ["--cbmc-args", "--unwindset", ",".join(bounds)]
    else:
        verify += ["--force-build"]
    record(verify)
    run(verify, directory / "run.log")
    check_results(json.loads(report.read_text()), expected,
                  strict_assertions=not args.group.startswith("arena-inductive"))
    print(f"{args.group}: {len(expected)} harnesses passed; receipts {directory}")


if __name__ == "__main__":
    main()
