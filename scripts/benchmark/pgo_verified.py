#!/usr/bin/env python3
"""Train PGO on general scaling scripts; retain V8 v7 for independent acceptance."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from run import ROOT, command_output, digest, git_metadata, run_sample
from scaling import admit
from scaling_workloads import CASES, prepare
from pgo_validation import verify_file


def artifact(path):
    path = Path(path).resolve()
    return {"path": str(path), "sha256": digest(path)}


def checked_tool(rustc):
    sysroot = Path(command_output(["rustc", "--print", "sysroot"])["stdout"])
    triple = next(line.split(":", 1)[1].strip() for line in rustc["stdout"].splitlines() if line.startswith("host:"))
    version = next(line.split(":", 1)[1].strip() for line in rustc["stdout"].splitlines() if line.startswith("LLVM version:"))
    tool = sysroot / "lib" / "rustlib" / triple / "bin" / "llvm-profdata"
    observed = command_output([str(tool), "--version"])
    match = re.search(r"LLVM version (\d+\.\d+\.\d+)", observed["stdout"])
    if observed["exit_code"] or match is None or match[1] != version:
        raise ValueError("llvm-profdata must match this rustc's exact LLVM version")
    return triple, tool, observed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True, help="new external directory; profiles are never reused")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--operations", type=int, default=8192)
    parser.add_argument("--sizes", type=int, nargs="+", default=[64, 128])
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    if args.jobs < 1 or args.operations < 1 or args.timeout <= 0 or not args.sizes or any(
            n < 1 or args.operations % n for n in args.sizes) or len(set(args.sizes)) != len(args.sizes):
        parser.error("positive jobs, timeout, distinct sizes dividing operations required")
    source = git_metadata(args.repo.resolve())
    if source["status"]["stdout"] or Path(source["top_level"]["stdout"]).resolve() != args.repo.resolve():
        parser.error("clean source worktree root required")
    rustc, cargo = command_output(["rustc", "-vV"]), command_output(["cargo", "-V"])
    triple, tool, tool_version = checked_tool(rustc)
    output = args.output.resolve()
    if output.is_relative_to(args.repo.resolve()) or output.is_relative_to(ROOT):
        parser.error("output must be outside repositories")
    output.mkdir(parents=True, exist_ok=False)
    builder = Path(__file__).with_name("build.py")
    frozen = [artifact(Path(__file__).with_name(name)) for name in
              ("build.py", "pgo_verified.py", "pgo_validation.py", "run.py", "scaling.py", "scaling_workloads.py")]
    base_command = [sys.executable, str(builder), "--repo", str(args.repo.resolve()), "--plain-only",
                    "--jobs", str(args.jobs), "--explicit-target", triple]

    def build(name, extra=()):
        target = output / (name + "-target")
        command = [*base_command, "--plain-target", str(target), *extra]
        with (output / (name + ".stdout.log")).open("wb") as stdout, (output / (name + ".stderr.log")).open("wb") as stderr:
            subprocess.run(command, cwd=args.repo, stdout=stdout, stderr=stderr, check=True)
        binary = target / triple / "release" / "qjs"
        return binary, json.loads(binary.with_suffix(".build.json").read_text())

    profiles = output / "profraw"
    print("build instrumented", flush=True)
    instrumented, generated = build("generate", ("--optimization", "profile-generate", "--profile-data", str(profiles)))
    frozen += [artifact(instrumented), artifact(instrumented.with_suffix(".build.json")), artifact(tool)]
    if frozen[-3]["sha256"] != generated["binary_sha256"]:
        raise ValueError("instrumented binary differs from its build receipt")
    if generated["release_profile"]["qjs_rustc_invocation"]["codegen"].get("profile-generate") != str(profiles):
        raise ValueError("instrumented compiler invocation did not use requested profile-generate")
    # Every child has unique output. No other benchmark runner is allowed to
    # run this executable during training; the profiles are inventoried below.
    previous = os.environ.get("LLVM_PROFILE_FILE")
    os.environ["LLVM_PROFILE_FILE"] = str(profiles / "%m_%p.profraw")
    samples, workloads = [], []
    (output / "raw").mkdir()
    try:
        with (output / "samples.jsonl").open("w") as journal:
            for case in CASES:
                for size in args.sizes:
                    workload = prepare(output / "workloads" / f"{case}-{size}", case, size, args.operations)
                    workloads.append(workload)
                    inputs = [{"path": str(Path(workload["path"]).parent / name), "sha256": sha}
                              for name, sha in workload["files"].items()]
                    for identity in frozen + inputs:
                        verify_file(identity)
                    pattern = profiles / f"{case}-{size}_%m_%p.profraw"
                    os.environ["LLVM_PROFILE_FILE"] = str(pattern)
                    before = set(profiles.glob(f"{case}-{size}_*.profraw"))
                    sample = run_sample([str(instrumented), workload["path"]], output,
                                        output / "raw" / f"{case}-{size}", args.timeout)
                    sample.update(case=case, size=size, status=admit(sample, workload["expected"].encode()))
                    for identity in frozen + inputs:
                        verify_file(identity)
                    produced = sorted(set(profiles.glob(f"{case}-{size}_*.profraw")) - before)
                    sample["profile_pattern"] = str(pattern)
                    sample["profraw"] = [artifact(path) for path in produced if path.stat().st_size > 0]
                    if sample["status"] == "ok" and not sample["profraw"]:
                        sample["status"] = "missing-profile"
                    for stream in ("stdout", "stderr"):
                        sample[stream + "_sha256"] = digest(sample[stream])
                    samples.append(sample)
                    journal.write(json.dumps(sample) + "\n")
                    journal.flush()
                    print(f"training {case}/{size}: {sample['status']}", flush=True)
                    if sample["status"] != "ok":
                        raise ValueError("training failed; raw evidence kept, no optimized build accepted")
    finally:
        if previous is None:
            os.environ.pop("LLVM_PROFILE_FILE", None)
        else:
            os.environ["LLVM_PROFILE_FILE"] = previous
    raw = [artifact(path) for path in sorted(profiles.glob("*.profraw"))]
    if not raw:
        raise ValueError("no raw training profiles")
    merged = output / "merged.profdata"
    command = [str(tool), "merge", "-o", str(merged), *(row["path"] for row in raw)]
    with (output / "merge.stdout.log").open("wb") as stdout, (output / "merge.stderr.log").open("wb") as stderr:
        subprocess.run(command, stdout=stdout, stderr=stderr, check=True)
    artifacts = list(frozen)
    for sample in samples:
        artifacts += [{"path": sample[stream], "sha256": sample[stream + "_sha256"]}
                      for stream in ("stdout", "stderr")]
    for workload in workloads:
        artifacts += [{"path": str(Path(workload["path"]).parent / name), "sha256": sha}
                      for name, sha in workload["files"].items()]
    for identity in artifacts:
        verify_file(identity)
    artifacts += [artifact(output / name) for name in ("samples.jsonl", "merge.stdout.log", "merge.stderr.log")]
    receipt = {"schema": "oxide-pgo-training-v1", "source_commit": source["commit"]["stdout"],
               "source_tree": source["tree"]["stdout"], "rustc": rustc, "cargo": cargo,
               "held_out": "original-v8-v7", "training": {"cases": list(CASES), "sizes": args.sizes,
               "operations": args.operations, "repeat": 1, "timeout_seconds": args.timeout},
               "instrumented_build_receipt": artifact(instrumented.with_suffix(".build.json")),
               "instrumented_binary": frozen[-3],
               "samples": samples, "workloads": workloads, "artifacts": artifacts, "profraw": raw, "profdata": artifact(merged),
               "merge": {"command": command, "exit_code": 0, "tool": artifact(tool), "version": tool_version}}
    if git_metadata(args.repo.resolve()) != source:
        raise ValueError("source changed during training")
    training_receipt = output / "training.json"
    training_receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print("build optimized", flush=True)
    candidate, _ = build("use", ("--optimization", "profile-use", "--profile-data", str(merged),
                                 "--training-receipt", str(training_receipt)))
    print("build matching uninstrumented baseline", flush=True)
    baseline, _ = build("none")
    from pgo_validation import require_build_technique
    require_build_technique({"build": json.loads(baseline.with_suffix(".build.json").read_text())},
                            {"build": json.loads(candidate.with_suffix(".build.json").read_text())})
    (output / "engines.json").write_text(json.dumps({"baseline": artifact(baseline), "candidate": artifact(candidate),
                                                    "training": artifact(training_receipt)}, indent=2) + "\n")
    print(candidate, flush=True)


if __name__ == "__main__":
    main()
