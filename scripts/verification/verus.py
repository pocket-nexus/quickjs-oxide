#!/usr/bin/env python3
"""Verify source-bound mark and the separately labelled inductive bitmap model."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
VERSION = "0.2026.10.18.7d82994"


def body(source, signature):
    """Extract these simple methods, failing on ambiguous/missing signatures."""
    if source.count(signature) != 1:
        raise ValueError(f"missing or ambiguous method: {signature}")
    start = source.index("{", source.index(signature))
    depth = 1
    for end in range(start + 1, len(source)):
        depth += (source[end] == "{") - (source[end] == "}")
        if depth == 0:
            return source[start + 1:end].strip()
    raise ValueError("unterminated method")


def normalized(source):
    return re.sub(r"\s+", " ", source).strip()


def mark_source(source, template):
    # Rust Option::is_some_and has no Verus std specification in this release.
    # Check the exact source before the equivalent Option match translation.
    live = body(source, "pub fn is_live(&self, index: u32) -> bool")
    expected = "let (word, mask) = bit(index); self.live.get(word).is_some_and(|bits| bits & mask != 0)"
    if normalized(live) != expected:
        raise ValueError("is_live changed; review the Option translation")
    check = body(source, "fn check(&self, index: u32)")
    expected_check = '''if CHECK_LIVE { assert!( self.is_live(index), "stale {} index {index}", std::any::type_name::<T>() ); }'''
    if normalized(check) != expected_check:
        raise ValueError("check changed; review the diagnostic-only translation")
    if "const CHECK_LIVE: bool = cfg!(any(debug_assertions, feature = \"gc-stress\"));" not in source:
        raise ValueError("liveness check configuration changed")
    live = '''let (word, mask) = bit(index);
        match self.live.get(word) { Some(bits) => bits & mask != 0, None => false }'''
    for key, value in {
        "BIT": body(source, "fn bit(index: u32) -> (usize, u64)"),
        "IS_LIVE": live,
        "MARK": body(source, "pub fn mark(&mut self, index: u32) -> bool"),
    }.items():
        template = template.replace(f"/* {key}_BODY */", value)
    return template


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--mutations", action="store_true", help="also require rejected proof mutations")
    args = parser.parse_args()
    executable = args.verus.resolve()
    version = subprocess.check_output([str(executable), "--version"], text=True)
    if VERSION not in version:
        raise SystemExit(f"Expected Verus {VERSION}; found {version}")
    directory = Path(tempfile.mkdtemp(prefix="verus-", dir=ROOT / "target/verification"))
    template = ROOT / "scripts/verification/verus/arena_mark.rs.in"
    arena = ROOT / "core/src/heap/arena.rs"
    generated = directory / "arena_mark.rs"
    generated.write_text(mark_source(arena.read_text(), template.read_text()))
    model = ROOT / "scripts/verification/verus/arena_model.rs"
    provenance = {"version": version, "commit": subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "sources": {
        str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in (arena, template, generated, model, Path(__file__).resolve())},
        "commands": []}
    for source in (generated, model):
        command = [str(executable), str(source), "--no-cheating", "--output-json"]
        provenance["commands"].append(command)
        (directory / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        (directory / f"{source.stem}.stdout").write_text(result.stdout)
        (directory / f"{source.stem}.stderr").write_text(result.stderr)
        print(f"{source.stem}: exit {result.returncode}; receipts {directory}", flush=True)
        if result.returncode:
            print(result.stderr)
            raise SystemExit(result.returncode)
        report = json.loads(result.stdout)
        if (not report["verification-results"]["success"]
            or report["verification-results"]["errors"] != 0
            or report["verification-results"]["verified"] != (5 if source == generated else 15)
            or not report["verification-results"]["is-verifying-entire-crate"]):
            raise SystemExit("Verus did not report successful verification")

    if args.mutations:
        mutations = {
            "mark_clears_bit": mark_source(arena.read_text().replace(
                "*bits |= mask;", "*bits &= !mask;"), template.read_text()),
            "free_sets_invalid_bits": model.read_text().replace(
                "Word { live: s.live & !mask, ..s }", "Word { live: u64::MAX, ..s }"),
        }
        for name, contents in mutations.items():
            path = directory / f"{name}.rs"
            path.write_text(contents)
            command = [str(executable), str(path), "--no-cheating", "--output-json"]
            provenance["commands"].append(command)
            (directory / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            (directory / f"{name}.stdout").write_text(result.stdout)
            (directory / f"{name}.stderr").write_text(result.stderr)
            report = json.loads(result.stdout)["verification-results"]
            if result.returncode == 0 or report["success"] or report.get("errors", 0) < 1:
                raise SystemExit(f"mutation was not rejected by a proof obligation: {name}")
            print(f"mutation rejected: {name}")


if __name__ == "__main__":
    main()
