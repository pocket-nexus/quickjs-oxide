#!/usr/bin/env python3
"""Build release CLIs from a clean checkout or an authenticated source export."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

from frozen_source import exact_git_root, manifest_files, validate_frozen_source
from run import ROOT, command_output, digest, git_metadata


def cargo_binary(output, target):
    """Use Cargo's actual executable, including configured target triples."""
    binaries = set()
    for line in output.splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "qjs"
                and "bin" in message.get("target", {}).get("kind", [])
                and message.get("executable")):
            binaries.add(Path(message["executable"]).resolve())
    if len(binaries) != 1:
        raise ValueError("Cargo must report exactly one qjs executable")
    binary = binaries.pop()
    if not binary.is_relative_to(target) or not binary.is_file():
        raise ValueError("Cargo executable must exist inside the requested target directory")
    return binary


def build(args):
    repo = args.repo.resolve()
    targets = [("plain", args.plain_target, []),
               ("profiling", args.profile_target, ["profiling"])]
    targets = [(name, target.resolve(), features) for name, target, features in targets
               if args.mode in ("both", name)]
    if args.jobs < 1 or len({target for _, target, _ in targets}) != len(targets):
        raise ValueError("jobs must be positive; build directories must differ")

    source_receipt = None
    if args.source_manifest:
        source_manifest = args.source_manifest.resolve()
        manifest_bytes = source_manifest.read_bytes()
        manifest_hash = hashlib.sha256(manifest_bytes).hexdigest()
        source_receipt = json.loads(manifest_bytes)
        files = manifest_files(source_receipt)
        if any(target.is_relative_to(repo) for _, target, _ in targets):
            raise ValueError("frozen build output must be outside the source export")
        validate_frozen_source(repo, files)
        revision = None
        source_label = f"frozen:{manifest_hash}"
        identity = {"kind": "frozen-export", "manifest_sha256": manifest_hash,
                    "files_validated": len(files), "validated_before_and_after_build": True}

        def validate():
            if digest(source_manifest) != manifest_hash:
                raise ValueError("source manifest changed during build")
            validate_frozen_source(repo, files)
    else:
        if not exact_git_root(repo):
            raise ValueError("--repo is not an exact Git checkout root; supply --source-manifest")
        metadata = git_metadata(repo)
        if metadata["commit"]["exit_code"] or metadata["status"]["exit_code"]:
            raise ValueError("cannot authenticate Git source identity")
        if metadata["status"]["stdout"]:
            raise ValueError("build provenance requires a clean checkout or a frozen source export")
        revision = metadata["commit"]["stdout"]
        source_label = revision
        identity = {"kind": "git-checkout", "commit": revision,
                    "validated_before_and_after_build": True}

        def validate():
            current = git_metadata(repo)
            if (current["commit"]["exit_code"] or current["status"]["exit_code"]
                    or current["commit"]["stdout"] != revision or current["status"]["stdout"]):
                raise ValueError("checkout changed during build")

    env = {**os.environ, "QUICKJS_OXIDE_BUILD_COMMIT": source_label}
    build_environment = {key: value for key, value in env.items()
                         if key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC",
                                    "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                                    "RUSTUP_TOOLCHAIN", "CARGO_BUILD_TARGET",
                                    "CARGO_BUILD_RUSTFLAGS"}
                         or key.startswith("CARGO_PROFILE_")
                         or (key.startswith("CARGO_TARGET_")
                             and key.endswith(("_RUSTFLAGS", "_LINKER")))}
    manifests = []
    for _, target, _ in targets:
        # Invalidate host and target-triple receipts before any build can fail.
        for pattern in ("release/qjs.build.json", "*/release/qjs.build.json"):
            for receipt in target.glob(pattern):
                receipt.unlink()
    for name, target, features in targets:
        validate()
        feature_args = ["--features", ",".join(features)] if features else []
        command = ["cargo", "build", "--locked", "--release", "-p", "quickjs-oxide-cli",
                   "--no-default-features", "--target-dir", str(target),
                   "--jobs", str(args.jobs), "--message-format=json-render-diagnostics", *feature_args]
        result = subprocess.run(command, cwd=repo, env=env, check=True,
                                stdout=subprocess.PIPE, text=True)
        binary = cargo_binary(result.stdout, target)
        validate()
        manifest = {"schema": "oxide-build-v1", "mode": name,
                    "vm_configuration": "stack-vm", "features": features,
                    "commit": revision, "source_identity": dict(identity),
                    "binary_sha256": digest(binary), "command": command,
                    "binary": str(binary),
                    "rustc": command_output(["rustc", "-vV"], repo),
                    "cargo": command_output(["cargo", "-V"], repo),
                    "cargo_toml_sha256": digest(repo / "Cargo.toml"),
                    "cargo_lock_sha256": digest(repo / "Cargo.lock"),
                    "environment": build_environment, "commit_environment": source_label}
        manifests.append((binary, manifest))
    validate()
    # Admit both modes only after all builds have kept the same source identity.
    for binary, manifest in manifests:
        if digest(binary) != manifest["binary_sha256"]:
            raise ValueError("binary changed before build receipt publication")
    for binary, manifest in manifests:
        if source_receipt is not None:
            companion = binary.with_suffix(".source.json")
            companion.write_bytes(manifest_bytes)
            manifest["source_identity"]["manifest_file"] = companion.name
        binary.with_suffix(".build.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(binary)
    return manifests


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--source-manifest", type=Path,
                        help="complete files/source_files SHA-256 identity for a frozen export")
    parser.add_argument("--plain-target", type=Path, default=ROOT / "target")
    parser.add_argument("--profile-target", type=Path, default=ROOT / "target/profile-feature")
    parser.add_argument("--mode", choices=["both", "plain", "profiling"], default="both")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    try:
        build(args)
    except (ValueError, OSError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
