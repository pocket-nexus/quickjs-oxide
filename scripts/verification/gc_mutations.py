#!/usr/bin/env python3
"""Check that real collector faults fail behavioral tests in an isolated source copy."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "target/verification/runtime-mutations"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    TARGET.mkdir(parents=True, exist_ok=True)
    names = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT
    ).decode().split("\0")
    changes = [
        ("lost_strong_edge", "push_heap_values(stack, self.values.slice(header.slots()));", "",
         "ephemeron_activation_discovers_a_new_map_and_a_symbol_key", "gc graph: wrong mark"),
        ("lost_key_activation", "stack.extend(values);", "drop(values);",
         "generated_heap_graphs_match_independent_saturation_across_collections", "gc graph: wrong mark"),
        ("stale_weak_state", "self.gc.weak.clear();\n        let mut stack", "let mut stack",
         "verification_does_not_root_a_dead_weak_map_in_the_next_collection", "gc adapter: dead map provenance"),
    ]
    receipt = []
    with tempfile.TemporaryDirectory(prefix="source-", dir=TARGET) as tmp:
        source = Path(tmp)
        for name in sorted(set(names)):
            original = ROOT / name
            if name and original.is_file():
                copied = source / name
                copied.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(original, copied)
        gc = source / "core/src/heap/gc.rs"
        original = gc.read_text()
        env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET / "build"))
        # Establish that the unmodified source and the selected behavioral tests pass.
        base = ["cargo", "test", "--locked", "-p", "quickjs-oxide-core", "--lib"]
        result = subprocess.run(base + ["heap::gc::tests"], cwd=source, env=env,
                                text=True, capture_output=True, timeout=300)
        (TARGET / "baseline.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError("baseline failed; see target/verification/runtime-mutations/baseline.log")
        for name, before, after, test, diagnostic in changes:
            if before not in original:
                raise RuntimeError(f"mutation {name} lost its source target")
            gc.write_text(original.replace(before, after, 1))
            result = subprocess.run(base + [f"heap::gc::tests::{test}", "--", "--exact"],
                                    cwd=source, env=env, text=True, capture_output=True, timeout=300)
            output = result.stdout + result.stderr
            (TARGET / f"{name}.log").write_text(output)
            accepted = result.returncode != 0 and diagnostic in output and "test result: FAILED" in output
            receipt.append({"mutation": name, "rejected": accepted, "diagnostic": diagnostic})
            print(f"{'PASS' if accepted else 'FAIL'} {name}", flush=True)
            if not accepted:
                raise RuntimeError(f"mutation was not rejected as expected; see {TARGET / (name + '.log')}")
        gc.write_text(original)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    main()
