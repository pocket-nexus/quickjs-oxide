"""Build a shared public API timing probe against an explicit checkout and feature set."""
import argparse
import json
import os
import shutil
import subprocess
import tomllib
from pathlib import Path
from frozen_source import exact_git_root, manifest_files, validate_frozen_source
from run import digest


def build_environment_key(name):
    return (name in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
                     "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_TARGET",
                     "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
                     "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_INCREMENTAL"}
            or name.startswith("CARGO_PROFILE_RELEASE_")
            or (name.startswith("CARGO_TARGET_") and name.endswith(("_RUSTFLAGS", "_LINKER", "_RUNNER"))))


def build_provenance(cwd, manifest_path, environment=None):
    """Capture only compilation-related environment/configuration, never all env."""
    environment = os.environ if environment is None else environment
    cwd = Path(cwd).resolve()
    home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
    directories = [home, *(parent / ".cargo" for parent in reversed(cwd.parents)), cwd / ".cargo"]
    configurations, seen = [], set()
    for directory in directories:
        # Cargo prefers the extensionless legacy file if both are present.
        path = directory / "config"
        if not path.is_file():
            path = directory / "config.toml"
        if not path.is_file() or path.resolve() in seen:
            continue
        seen.add(path.resolve())
        data = tomllib.loads(path.read_text())
        build = {key: value for key, value in data.get("build", {}).items()
                 if key in {"target", "rustflags", "rustc", "rustc-wrapper", "rustc-workspace-wrapper"}}
        targets = {key: {name: value for name, value in table.items()
                         if name in {"rustflags", "linker", "runner"}}
                   for key, table in data.get("target", {}).items()}
        relevant = dict(build=build, target=targets, release=data.get("profile", {}).get("release", {}),
                        environment={key: value for key, value in data.get("env", {}).items()
                                     if build_environment_key(key)})
        configurations.append(dict(path=str(path.resolve()), sha256=digest(path), relevant=relevant))
    tools = {key: value for key, value in environment.items()
             if key in {"RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC",
                        "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"} and value}
    tools.setdefault("default-rustc", "rustc")
    for index, configuration in enumerate(configurations):
        for key, value in configuration["relevant"]["build"].items():
            if key in {"rustc", "rustc-wrapper", "rustc-workspace-wrapper"} and value:
                tools[f"configuration-{index}:{key}"] = value
    tool_identities = {}
    for name, value in tools.items():
        candidate = Path(value)
        path = (cwd / candidate) if len(candidate.parts) > 1 else Path(shutil.which(value) or cwd / value)
        tool_identities[name] = dict(command=value, path=str(path.resolve()),
                                     sha256=digest(path) if path.is_file() else None)
    manifest = tomllib.loads(Path(manifest_path).read_text())
    return dict(schema="oxide-probe-build-provenance.v1", invocation_directory=str(cwd),
                environment={key: value for key, value in sorted(environment.items()) if build_environment_key(key)},
                cargo_configurations=configurations,
                compiler_tools=tool_identities,
                probe_manifest_sha256=digest(manifest_path),
                probe_release_profile=manifest.get("profile", {}).get("release", {}),
                scope="shell compilation environment and visible Cargo configurations; dependency profiles do not override the standalone probe profile")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-manifest", type=Path,
                        help="complete frozen export identity; mandatory when --repo is not an exact Git checkout root")
    parser.add_argument("--profiling", action="store_true")
    parser.add_argument("--probe", choices=["compile", "first-execution"], default="compile")
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    source_receipt = None
    source_manifest_sha256 = None
    if args.source_manifest:
        if output.is_relative_to(repo):
            parser.error("frozen build output must be outside the source export")
        source_manifest_sha256 = digest(args.source_manifest)
        source_receipt = json.loads(args.source_manifest.read_text())
        frozen_files = manifest_files(source_receipt)
        validate_frozen_source(repo, frozen_files)
    elif not exact_git_root(repo):
        parser.error("--repo is not an exact Git checkout root; supply --source-manifest (ancestor Git metadata is not source identity)")
    output.mkdir(parents=True, exist_ok=False)
    (output / "src").mkdir()
    source_name = "compile_probe.rs" if args.probe == "compile" else "first_execution_probe.rs"
    binary_name = "oxide-compile-probe" if args.probe == "compile" else "oxide-first-execution-probe"
    source = Path(__file__).resolve().parents[2] / "apps/cli/examples" / source_name
    (output / "src/main.rs").write_bytes(source.read_bytes())
    probe_source_sha256 = digest(output / "src/main.rs")
    features = [name for enabled, name in [(args.profiling, "profiling")] if enabled]
    manifest = '\n'.join([
        '[package]', f'name="{binary_name}"', 'version="0.0.0"', 'edition="2024"',
        '[workspace]', '[features]', 'profiling=[]', '[dependencies]',
        f'quickjs-oxide={{path={json.dumps(str(repo))},default-features=false,features={json.dumps(features)}}}',
        f'quickjs-oxide-host={{path={json.dumps(str(repo / "adapters/native"))}}}', '',
    ])
    (output / "Cargo.toml").write_text(manifest)
    (output / "Cargo.lock").write_bytes((repo / "Cargo.lock").read_bytes())
    command = ["cargo", "build", "--release", "--manifest-path", str(output / "Cargo.toml"), "--target-dir", str(output / "target")]
    if args.profiling:
        command += ["--features", "profiling"]
    provenance = build_provenance(Path.cwd(), output / "Cargo.toml")
    with (output / "build.log").open("w") as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
    if digest(output / "src/main.rs") != probe_source_sha256:
        raise ValueError("compile probe source changed during build")
    if build_provenance(Path.cwd(), output / "Cargo.toml") != provenance:
        raise ValueError("probe compilation environment/configuration changed during build")
    if source_receipt is not None:
        if digest(args.source_manifest) != source_manifest_sha256:
            raise ValueError("source manifest changed during build")
        validate_frozen_source(repo, frozen_files)
    pinned = {(p["name"], p["version"], p.get("source")): p.get("checksum")
              for p in tomllib.loads((repo / "Cargo.lock").read_text())["package"]}
    for dependency in tomllib.loads((output / "Cargo.lock").read_text())["package"]:
        if dependency.get("source"):
            key = dependency["name"], dependency["version"], dependency["source"]
            if key not in pinned or pinned[key] != dependency.get("checksum"):
                raise ValueError("compile probe dependency drift; binary not admitted")
    binary = output / "target/release" / binary_name
    if source_receipt is not None:
        (output / "source-manifest.json").write_bytes(args.source_manifest.read_bytes())
        source_identity = dict(kind="frozen-export", manifest_sha256=source_manifest_sha256,
                               manifest_file="source-manifest.json", files_validated=len(frozen_files),
                               validated_before_and_after_build=True)
        dependency_commit = source_receipt.get("base", source_receipt.get("parent"))
        source_patch_sha256 = None
    else:
        tracked_diff = subprocess.check_output(["git", "-C", str(repo), "diff", "HEAD", "--binary"])
        (output / "source.patch").write_bytes(tracked_diff)
        dependency_commit = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        source_patch_sha256 = digest(output / "source.patch")
        source_identity = dict(kind="git-checkout", note="patch covers tracked files; stage receipt must additionally hash untracked implementation files")
    metadata = dict(command=command, probe=args.probe, repository=str(repo), binary=str(binary), binary_sha256=digest(binary),
                    source_sha256=probe_source_sha256, cargo_lock_sha256=digest(output / "Cargo.lock"),
                    dependency_commit=dependency_commit, source_identity=source_identity,
                    source_patch_sha256=source_patch_sha256, features=features, build_provenance=provenance,
                    rustc=subprocess.check_output(["rustc", "-vV"], text=True),
                    rustflags=os.environ.get("RUSTFLAGS"), encoded_rustflags=os.environ.get("CARGO_ENCODED_RUSTFLAGS"))
    (output / "build.json").write_text(json.dumps(metadata, indent=2) + "\n")
    binary.with_suffix(".build.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(binary)


if __name__ == "__main__":
    main()
