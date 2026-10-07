#!/usr/bin/env python3
"""Build a source tree in the aarch64 valgrind container and run `probes.py` on it.

The source tree is mounted read-only at /src and built with Rust 1.88 into
/target/<target-subdir> of the `oxide-target` volume. With --rev, --source is a Git
checkout and that revision is first exported into the work directory. The probe
scripts always come from this checkout and the V8 bodies from --v8; both are copied
into the work directory first, so two branches are measured with the same probe set.

Only the work directory and the source tree are bind-mounted. Keep both outside
~/Documents and other privacy-protected folders (use --rev for a checkout there):
Docker Desktop's file sharing can block every container start while it waits for a
macOS permission prompt for such a path.

Example (A/B: run once per revision, then compare the two JSON files):

    python3 docs/performance/probes/runtime/measure_docker.py \\
        --source . --rev <commit> --target-subdir item2-a \\
        --json /private/tmp/item2/a.json
"""
import argparse
import shutil
import subprocess
import sys
from pathlib import Path

TOOL_ROOT = Path(__file__).resolve().parents[4]
TOOL_FILES = ("docs/performance/probes/runtime/probes.py", "docs/performance/probes/allocation/ledger.py",
              "scripts/benchmark/profile_v8.py", "scripts/benchmark/run.py")
V8_FILES = ("base.js", "richards.js", "deltablue.js", "navier-stokes.js")
DEFAULT_V8 = Path("/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/upstream-benchmark")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", type=Path, required=True, help="source tree to build (a snapshot, not edited)")
    parser.add_argument("--rev", help="export this revision of the --source Git checkout and build that")
    parser.add_argument("--target-subdir", required=True, help="CARGO_TARGET_DIR name inside the oxide-target volume")
    parser.add_argument("--json", type=Path, required=True, help="report path on the host")
    parser.add_argument("--workdir", type=Path, help="new host directory for scripts and Callgrind files "
                                                       "(default: <json without suffix>.work)")
    parser.add_argument("--v8", type=Path, default=DEFAULT_V8, help="V8-v7 source root ('none' to skip)")
    parser.add_argument("--image", default="oxide-vg:1.88")
    parser.add_argument("--n", type=int, default=100_000)
    parser.add_argument("--v8-iterations", type=int, default=10)
    parser.add_argument("--probe", action="append", help="forwarded to probes.py")
    parser.add_argument("--jobs", type=int, default=4,
                        help="parallel Callgrind runs; each V8 case needs about 1 GiB in the VM")
    parser.add_argument("--skip-build", action="store_true", help="reuse the existing binary in the target subdir")
    args = parser.parse_args()
    if not args.target_subdir.replace("-", "").replace("_", "").isalnum():
        parser.error("--target-subdir must be a plain name")
    report = args.json.resolve()
    workdir = (args.workdir or report.with_suffix(".work")).resolve()
    workdir.mkdir(parents=True, exist_ok=False)
    report.parent.mkdir(parents=True, exist_ok=True)
    source = args.source.resolve()
    if args.rev:
        archive = subprocess.run(["git", "-C", str(source), "archive", args.rev],
                                 capture_output=True, check=True).stdout
        source = workdir / "src"
        source.mkdir()
        subprocess.run(["tar", "-x", "-C", str(source)], input=archive, check=True)
    if not (source / "Cargo.lock").is_file():
        parser.error("--source must be a quickjs-oxide source tree")
    for relative in TOOL_FILES:
        (workdir / "tool" / relative).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(TOOL_ROOT / relative, workdir / "tool" / relative)
    target = f"/target/{args.target_subdir}"
    mounts = ["-v", "oxide-cargo:/usr/local/cargo/registry", "-v", "oxide-target:/target",
              "-v", f"{source}:/src:ro", "-v", f"{workdir}:/work"]
    probe_args = ["--qjs", f"{target}/release/qjs", "--n", str(args.n), "--output", "/work/run",
                  "--json", "/work/probes.json", "--v8-iterations", str(args.v8_iterations),
                  "--jobs", str(args.jobs)]
    if str(args.v8) != "none":
        code = args.v8 / "v8-v7" if (args.v8 / "v8-v7").is_dir() else args.v8
        (workdir / "v8").mkdir()
        for name in V8_FILES:
            shutil.copyfile(code / name, workdir / "v8" / name)
        probe_args += ["--v8", "/work/v8"]
    for probe in args.probe or ():
        probe_args += ["--probe", probe]
    build = ("" if args.skip_build else
             f"CARGO_TARGET_DIR={target} cargo build --locked --release -p quickjs-oxide-cli "
             "--no-default-features && ")
    quoted = " ".join("'" + a.replace("'", "'\\''") + "'" for a in probe_args)
    script = f"set -e; cd /src; {build}python3 /work/tool/docs/performance/probes/runtime/probes.py {quoted}"
    name = f"oxide-probes-{args.target_subdir}"
    command = ["docker", "run", "--rm", "--name", name, *mounts, args.image, "bash", "-c", script]
    try:
        result = subprocess.run(command, check=False)
    except KeyboardInterrupt:
        subprocess.run(["docker", "kill", name], check=False)
        raise
    if result.returncode:
        return result.returncode
    shutil.copyfile(workdir / "probes.json", report)
    print(f"report: {report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
