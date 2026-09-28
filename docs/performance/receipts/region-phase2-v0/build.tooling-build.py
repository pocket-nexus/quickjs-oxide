#!/usr/bin/env python3
"""Build release CLIs in a clean worktree and retain the complete build evidence."""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess

from run import ROOT, command_output, digest, git_metadata


def release_profile_manifest(repo):
    """Record the profile values as written, before Cargo configuration overrides."""
    values = {}
    in_release = False
    for line in (repo / "Cargo.toml").read_text().splitlines():
        section = re.fullmatch(r"\s*\[([^]]+)\]\s*(?:#.*)?", line)
        if section:
            in_release = section[1] == "profile.release"
        elif in_release and "=" in line and not line.lstrip().startswith("#"):
            key, value = line.split("=", 1)
            values[key.strip()] = value.split("#", 1)[0].strip()
    return values


def cargo_configuration_hashes(repo, env):
    """Identify Cargo config files that can affect the build without copying secrets."""
    directories = [parent / ".cargo" for parent in (repo, *repo.parents)]
    directories.append(Path(env.get("CARGO_HOME", Path.home() / ".cargo")))
    return {str(path): digest(path) for directory in directories
            for name in ("config", "config.toml") if (path := directory / name).is_file()}


def build_environment(env):
    return {key: value for key, value in sorted(env.items())
            if key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN", "CARGO_INCREMENTAL", "CARGO_HOME")
            or key.startswith("CARGO_PROFILE_RELEASE_") or key.startswith("CARGO_BUILD_")
            or (key.startswith("CARGO_TARGET_") and key.endswith("_RUSTFLAGS"))}


def observed_rustc(stderr):
    """Read rustc invocations Cargo -v actually ran; cached crates have no line."""
    invocations = []
    for line in stderr.splitlines():
        match = re.search(r"Running `([^`]+)`", line)
        if not match or "--crate-name" not in match[1]:
            continue
        try:
            tokens = shlex.split(match[1])
        except ValueError:
            invocations.append({"command": match[1], "crate": None, "codegen": None, "target": None})
            continue
        if "--crate-name" not in tokens:
            continue
        crate_index = tokens.index("--crate-name")
        crate = tokens[crate_index + 1] if crate_index + 1 < len(tokens) else None
        codegen = {}
        target = None
        for index, token in enumerate(tokens):
            if token == "--target" and index + 1 < len(tokens):
                target = tokens[index + 1]
            if token == "-C" and index + 1 < len(tokens):
                option = tokens[index + 1]
            elif token.startswith("-C"):
                option = token[2:]
            else:
                continue
            key, separator, value = option.partition("=")
            if separator and key in ("lto", "codegen-units", "opt-level", "panic", "target-cpu", "target-feature"):
                codegen[key] = value
        invocations.append({"command": match[1], "crate": crate, "codegen": codegen, "target": target})
    return invocations


def observed_codegen_groups(invocations):
    """Summarize dependency and final-binary flags without duplicating every command."""
    groups = {}
    for invocation in invocations:
        key = (invocation["target"], json.dumps(invocation["codegen"], sort_keys=True))
        group = groups.setdefault(key, {"target": invocation["target"], "codegen": invocation["codegen"], "crates": []})
        group["crates"].append(invocation["crate"])
    return list(groups.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT, help="clean source worktree; this script remains in the tooling checkout")
    parser.add_argument("--plain-target", type=Path, default=ROOT / "target")
    parser.add_argument("--profile-target", type=Path, default=ROOT / "target/profile-feature")
    selected = parser.add_mutually_exclusive_group()
    selected.add_argument("--plain-only", action="store_true")
    selected.add_argument("--profile-only", action="store_true")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    if args.jobs < 1 or args.plain_target.resolve() == args.profile_target.resolve():
        parser.error("jobs must be positive; build directories must differ")
    repo_path = args.repo.resolve()
    source = git_metadata(repo_path)
    if (source["commit"]["exit_code"] or source["tree"]["exit_code"]
            or source["top_level"]["exit_code"]
            or Path(source["top_level"]["stdout"]).resolve() != repo_path):
        parser.error("--repo must be the root of a Git source worktree")
    if source["status"]["stdout"]:
        parser.error("commit source changes before measuring; build provenance requires a clean worktree")
    revision = source["commit"]["stdout"]
    tooling_files = {"build.py": Path(__file__), "run.py": Path(__file__).with_name("run.py")}
    tooling_bytes = {name: path.read_bytes() for name, path in tooling_files.items()}
    tooling_repository = git_metadata(ROOT)
    env = {**os.environ, "QUICKJS_OXIDE_BUILD_COMMIT": revision}
    rustc = command_output(["rustc", "-vV"], repo_path)
    cargo = command_output(["cargo", "-V"], repo_path)
    if rustc["exit_code"] or cargo["exit_code"]:
        parser.error("rustc and cargo must be available on the build host")
    selected_builds = [("plain", args.plain_target, [])] if args.plain_only else \
        [("profiling", args.profile_target, ["profiling"])] if args.profile_only else \
        [("plain", args.plain_target, []), ("profiling", args.profile_target, ["profiling"])]
    if repo_path != ROOT and any(target.resolve() in ((ROOT / "target").resolve(),
                                                  (ROOT / "target/profile-feature").resolve())
                                 for _, target, _ in selected_builds):
        parser.error("external --repo requires distinct --plain-target / --profile-target paths")
    for name, target, enabled in selected_builds:
        features = ["--features", ",".join(enabled)] if enabled else []
        target = target.resolve()
        target_triple = env.get("CARGO_BUILD_TARGET")
        artifact_dir = target / target_triple / "release" if target_triple else target / "release"
        artifact_dir.mkdir(parents=True, exist_ok=True)
        binary = artifact_dir / ("qjs.exe" if os.name == "nt" else "qjs")
        stdout_path = binary.with_suffix(".build.stdout.log")
        stderr_path = binary.with_suffix(".build.stderr.log")
        tooling_snapshots = {}
        for filename, contents in tooling_bytes.items():
            snapshot = artifact_dir / f"qjs.build.tooling-{filename}"
            snapshot.write_bytes(contents)
            tooling_snapshots[filename] = {"path": str(snapshot), "sha256": digest(snapshot)}
        command = ["cargo", "build", "--locked", "--release", "--verbose", "-p", "quickjs-oxide-cli",
                   "--no-default-features", "--target-dir", str(target), "--jobs", str(args.jobs), *features]
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            result = subprocess.run(command, cwd=repo_path, env=env, stdout=stdout, stderr=stderr, check=False)
        if result.returncode:
            raise RuntimeError(f"build failed ({result.returncode}); inspect {stdout_path} and {stderr_path}")
        if any(digest(path) != tooling_snapshots[name]["sha256"] for name, path in tooling_files.items()):
            raise RuntimeError("benchmark tooling changed during build; logs kept, receipt rejected")
        if git_metadata(repo_path) != source:
            raise RuntimeError("source worktree changed during build; logs kept, receipt rejected")
        rustc_invocations = observed_rustc(stderr_path.read_text(errors="replace"))
        invocation = next((row for row in rustc_invocations if row["crate"] == "qjs"), None)
        manifest = {
            "schema": "oxide-build-v2", "mode": name, "vm_configuration": "stack-vm", "features": enabled,
            "commit": revision, "source": source, "binary_sha256": digest(binary),
            "tooling": {"repository": tooling_repository, "snapshots": tooling_snapshots},
            "command": command, "cwd": str(repo_path), "exit_code": result.returncode,
            "stdout": {"path": str(stdout_path), "sha256": digest(stdout_path)},
            "stderr": {"path": str(stderr_path), "sha256": digest(stderr_path)},
            "rustc": rustc, "cargo": cargo,
            "cargo_toml_sha256": digest(repo_path / "Cargo.toml"),
            "cargo_lock_sha256": digest(repo_path / "Cargo.lock"),
            "release_profile": {"manifest": release_profile_manifest(repo_path),
                                "environment_overrides": build_environment(env),
                                "cargo_config_sha256": cargo_configuration_hashes(repo_path, env),
                                "qjs_rustc_invocation": invocation,
                                "observed_rustc_compilations": len(rustc_invocations),
                                "observed_codegen_groups": observed_codegen_groups(rustc_invocations)},
            "target": {"requested_triple": target_triple, "rustc_host": next(
                (line.partition(":")[2].strip() for line in rustc["stdout"].splitlines() if line.startswith("host:")), None),
                       "observed_triple": invocation["target"] if invocation else None,
                       "observed_target_cpu": invocation["codegen"].get("target-cpu") if invocation and invocation["codegen"] else None,
                       "observed_target_feature": invocation["codegen"].get("target-feature") if invocation and invocation["codegen"] else None},
            "commit_environment": revision,
        }
        binary.with_suffix(".build.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(binary)


if __name__ == "__main__":
    main()
