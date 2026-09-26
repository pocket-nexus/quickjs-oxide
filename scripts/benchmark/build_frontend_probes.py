#!/usr/bin/env python3
"""Build pure Script parser probes, with identical release settings.

Unlike the older engine compile/parse probes, parser and interner construction
are timed, and Boa's separate scope-analysis pass is excluded. Dependency locks,
probe hashes and build receipts are saved beside the external build artifacts.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

from run import ROOT, digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--engine", choices=["oxide", "boa"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    probe = ROOT / f"scripts/benchmark/probes/{args.engine}_parse_probe.rs"
    if args.engine == "oxide":
        subprocess.run([sys.executable, str(ROOT / "scripts/benchmark/build_compile_probe.py"),
                        "--repo", str(args.repo.resolve()), "--output", str(output),
                        "--test-support", "--probe", str(probe), "--name", "oxide-parse-probe"], check=True)
        return
    output.mkdir(parents=True, exist_ok=False)
    (output / "src").mkdir()
    shutil.copyfile(probe, output / "src/main.rs")
    (output / "Cargo.toml").write_text('''[package]
name = "boa-parse-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
boa_parser = { version = "=0.22.0", features = ["annex-b"] }
boa_interner = "=0.22.0"
[profile.release]
lto = "fat"
codegen-units = 1
''')
    # Resolve once, then freeze the complete dependency graph for this receipt.
    subprocess.run(["cargo", "generate-lockfile", "--manifest-path", str(output / "Cargo.toml")], check=True)
    command = ["cargo", "build", "--locked", "--release", "--manifest-path", str(output / "Cargo.toml")]
    with (output / "build.log").open("w") as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = output / "target/release/boa-parse-probe"
    receipt = dict(command=command, binary=str(binary), binary_sha256=digest(binary),
                   source_sha256=digest(probe), cargo_lock_sha256=digest(output / "Cargo.lock"),
                   boundary="complete ScriptParser(false), no analyze_scope; initialization included",
                   rustc=subprocess.check_output(["rustc", "-vV"], text=True))
    binary.with_suffix(".build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    (output / "build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(binary)


if __name__ == "__main__":
    main()
