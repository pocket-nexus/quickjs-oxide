#!/usr/bin/env python3
"""Run the test-only published-bytecode probe in a disposable detached worktree.

No dependency downloads or execution happen during --help. This is diagnostic
capture, not a benchmark. Any compiler/test failure is reported as failure.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

PIN = "2034d98fc8c5f8044e186267593f5d5ea5232caf"
TEST = "engine::code::fusion::numeric_span_probe::dump_numeric_spans"
TARGETS = {"am3": 1, "project": 1, "lin_solve": 1, "advect": 1}
SOURCE_FILES = ("v8-v7/crypto.js", "v8-v7/navier-stokes.js")


def command(args: list[str], cwd: Path | None = None) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{args!r} failed ({result.returncode}):\n{result.stderr}")
    return result.stdout.strip()


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rev", required=True, help="Engine commit or ref; resolved to a full SHA")
    parser.add_argument("--toolchain", default="1.94.1")
    args = parser.parse_args()
    repo, source, output = (p.resolve() for p in (args.repo, args.source, args.output))
    probe = Path(__file__).with_name("dump_numeric_spans.rs").resolve()
    receipt: dict[str, object] = {"format": "oxide-canonical-capture-v1", "status": "failed"}
    created_output = False
    try:
        if not probe.is_file():
            raise ValueError(f"Probe missing: {probe}")
        if output.exists():
            raise ValueError("Output directory must not already exist")
        if output.is_relative_to(repo) or output.is_relative_to(source):
            raise ValueError("Output must be outside both repositories")
        rev = command(["git", "-C", str(repo), "rev-parse", "--verify", "--end-of-options", f"{args.rev}^{{commit}}"])
        upstream = command(["git", "-C", str(source), "rev-parse", "HEAD"])
        if upstream != PIN:
            raise ValueError(f"Benchmark checkout must be {PIN}; found {upstream}")
        for name in SOURCE_FILES:
            expected = command(["git", "-C", str(source), "rev-parse", f"{PIN}:{name}"])
            actual = command(["git", "-C", str(source), "hash-object", "--no-filters", "--", name])
            if expected != actual:
                raise ValueError(f"Pinned benchmark file changed: {name}")
        output.mkdir(parents=True)
        created_output = True
        receipt.update({
            "engine_commit": rev,
            "benchmark_commit": PIN,
            "probe_sha256": digest(probe),
            "runner_sha256": digest(Path(__file__)),
            "sources": {name: digest(source / name) for name in SOURCE_FILES},
            "purpose": "compile-and-publish only; no benchmark execution",
            "environment": {name: os.environ.get(name) for name in
                            ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN")},
        })
        receipt["cargo"] = command(["cargo", f"+{args.toolchain}", "--version"])
        receipt["rustc"] = command(["rustc", f"+{args.toolchain}", "--version", "--verbose"])
        with tempfile.TemporaryDirectory(prefix="oxide-canonical-") as temporary:
            worktree = Path(temporary) / "tree"
            command(["git", "-C", str(repo), "worktree", "add", "--detach", str(worktree), rev])
            try:
                destination = worktree / "src/engine/code/fusion_numeric_span_probe.rs"
                if destination.exists():
                    raise ValueError("Temporary probe module unexpectedly exists at this revision")
                shutil.copyfile(probe, destination)
                fusion = worktree / "src/engine/code/fusion.rs"
                with fusion.open("a", encoding="utf-8") as handle:
                    handle.write('\n#[cfg(test)]\n#[path = "fusion_numeric_span_probe.rs"]\n'
                                 'mod numeric_span_probe;\n')
                env = os.environ.copy()
                env.update({"OXIDE_DUMP_SOURCE": str(source), "OXIDE_DUMP_OUTPUT": str(output),
                            "CARGO_TARGET_DIR": str(output / "target")})
                cmd = ["cargo", f"+{args.toolchain}", "test", "--locked", "--release",
                       "-p", "quickjs-oxide", "--lib", TEST, "--", "--ignored", "--exact",
                       "--test-threads=1"]
                receipt["command"] = cmd
                receipt["diagnostic_patch"] = command(["git", "diff", "--", "src/engine/code/fusion.rs"], worktree)
                with (output / "cargo.log").open("w", encoding="utf-8") as log:
                    run = subprocess.run(cmd, cwd=worktree, env=env, stdout=log, stderr=subprocess.STDOUT)
                receipt["exit_code"] = run.returncode
                if run.returncode:
                    raise RuntimeError(f"Probe failed; see {output / 'cargo.log'}")
                # A test filter matching zero tests must not count as success.
                lines = (output / "target-counts.tsv").read_text(encoding="utf-8").splitlines()
                counts = dict((name, int(count)) for name, count in (line.split("\t") for line in lines))
                if len(lines) != 4 or counts != TARGETS:
                    raise RuntimeError(f"Incomplete or ambiguous target receipt: {counts}")
                with (output / "dense-sites.tsv").open(newline="", encoding="utf-8") as stream:
                    sites = list(csv.DictReader(stream, delimiter="\t"))
                if any(site["kind"] != "R0" or site["source"] not in SOURCE_FILES for site in sites):
                    raise RuntimeError("Invalid R0 site manifest")
                receipt["sites"] = {
                    "array_reads": len(sites),
                    "published_r0": sum(site["flag"] == "1" for site in sites),
                    "rejections": {
                        reason: sum(site["rejection_reason"] == reason for site in sites)
                        for reason in sorted({site["rejection_reason"] for site in sites
                                            if site["flag"] != "1"})
                    },
                }
                files = [output / "crypto.canonical.txt", output / "navier-stokes.canonical.txt",
                         output / "target-counts.tsv", output / "dense-sites.tsv", output / "cargo.log"]
                receipt["artifacts"] = {path.name: digest(path) for path in files}
                receipt["status"] = "captured"
            finally:
                # Only remove the detached worktree that this invocation created.
                command(["git", "-C", str(repo), "worktree", "remove", "--force", str(worktree)])
        print(f"Captured four published functions. Receipt: {output / 'receipt.json'}")
        return 0
    except (OSError, ValueError, RuntimeError) as error:
        receipt["status"] = "failed"
        receipt["error"] = str(error)
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    finally:
        if created_output:
            (output / "receipt.json").write_text(
                json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
