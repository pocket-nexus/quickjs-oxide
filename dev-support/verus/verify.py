#!/usr/bin/env python3
"""Verify the contracts on quickjs-oxide-core with the Verus release in the README."""

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

import hash_dispatch


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
DEPENDENCIES = {"hashbrown", "libm", "oxide_regexp"}
INDEPENDENT = {
    "value": "lib.rs",
    "bytecode::opcode": "bytecode/mod.rs",
    "bytecode::builder": "bytecode/mod.rs",
    "heap::arena": "heap/mod.rs",
    "heap::hash": "heap/mod.rs",
    "heap::string": "heap/mod.rs",
    "heap::values": "heap/mod.rs",
    "vm::bigint": "vm/mod.rs",
    "builtins::base64": "builtins/mod.rs",
    "builtins::date::format": "builtins/date/mod.rs",
    "builtins::date::parse": "builtins/date/mod.rs",
}


def build_flags(toolchain, all_features):
    # Reuse the exact core feature set and dependency artifacts selected by Cargo.
    command = [
        "cargo", f"+{toolchain}", "build", "--locked", "--lib", "-p", "quickjs-oxide-core",
        "--message-format=json-render-diagnostics",
    ]
    if all_features:
        command.append("--all-features")
    build = subprocess.run(
        command, cwd=REPO, check=True, stdout=subprocess.PIPE, text=True,
    )
    libs = {}
    features = None
    for line in build.stdout.splitlines():
        artifact = json.loads(line)
        if artifact["reason"] != "compiler-artifact":
            continue
        name = artifact["target"]["name"]
        if name in DEPENDENCIES:
            libs[name] = next(path for path in artifact["filenames"] if path.endswith(".rlib"))
        elif name == "oxide_core":
            features = artifact["features"]
    if libs.keys() != DEPENDENCIES:
        raise ValueError(f"missing Cargo artifacts: {sorted(DEPENDENCIES - libs.keys())}")
    if features is None:
        raise ValueError("missing Cargo artifact for oxide_core")
    flags = []
    for name, path in libs.items():
        flags.extend(["--extern", f"{name}={path}"])
    for feature in features:
        flags.extend(["--cfg", f'feature="{feature}"'])
    return flags


def apply_annotations(src, patch):
    # Only insert annotations. Changing a production line to make a proof pass
    # would sever the link to the code built from this checkout.
    for line in patch.read_text().splitlines():
        if line.startswith("-") and not line.startswith("--- a/"):
            raise ValueError(f"verification patch removes a source line: {line}")
    subprocess.run(
        ["patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(patch)],
        cwd=src, check=True,
    )


def annotate(src):
    for name in ("contracts.patch", "independent.patch"):
        apply_annotations(src, HERE / name)
    for relative in (
        "value_proofs.rs", "heap/key_proofs.rs", "heap/root_proofs.rs",
        "bytecode/opcode_proofs.rs", "vm/jump_proofs.rs", "builtins/uri_proofs.rs",
        "heap/hash_proofs.rs", "bytecode/opcode_catalogue.rs", "bytecode/builder_proofs.rs",
        "vm/bigint_proofs.rs", "heap/slots_proofs.rs", "heap/string_proofs.rs",
        "builtins/base64_proofs.rs", "builtins/date/date_format_proofs.rs", "heap/arena_proofs.rs",
    ):
        destination = src / relative
        shutil.copyfile(HERE / destination.name, destination)


def check_independent_bodies(src, command, log_dir):
    """Ask Verus to check each entire module, including currently unannotated items."""
    log_dir.mkdir(parents=True, exist_ok=True)
    failures = 0
    for module, parent in INDEPENDENT.items():
        declaration = src / parent
        original = declaration.read_text()
        name = module.split("::")[-1]
        pattern = rf"^(?:(?:pub|pub\(crate\)) )?mod {name};$"
        changed, matches = re.subn(pattern, r"#[verifier::verify]\n\g<0>", original, flags=re.M)
        if matches != 1:
            raise ValueError(f"expected one declaration of {module} in {parent}")
        log = log_dir / (module.replace("::", ".") + ".log")
        try:
            declaration.write_text(changed)
            with log.open("w") as stream:
                result = subprocess.run(command, cwd=src.parent, stdout=stream, stderr=subprocess.STDOUT)
        finally:
            declaration.write_text(original)
        failures += result.returncode != 0
        print(f"{module:24} exit={result.returncode}  {log}", flush=True)
    return int(failures != 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", default="verus", help="path to the Verus executable")
    scope = parser.add_mutually_exclusive_group()
    scope.add_argument("--whole-core", action="store_true",
                        help="also ask Verus to verify every unannotated item in the core")
    scope.add_argument("--independent-bodies", type=Path, metavar="LOG_DIR",
                       help="check every body in each of the 11 projects separately and save diagnostics")
    parser.add_argument("--all-features", action="store_true",
                        help="enable all Cargo features of quickjs-oxide-core")
    scope.add_argument("--word-arithmetic", action="store_true",
                       help="check libcore word contracts with Kani, then prove multi-limb arithmetic and packed operands")
    args = parser.parse_args()
    verus = shutil.which(args.verus)
    if verus is None:
        parser.exit(2, "Verus executable not found. See dev-support/verus/README.md.\n")
    verus = str(Path(verus).resolve())
    version = json.loads(subprocess.check_output([verus, "--version", "--output-json"]))["verus"]
    toolchain = version["toolchain"].split()[0]
    print(f"Verus {version['version']} using Rust {toolchain}", flush=True)
    if args.word_arithmetic:
        machine = [sys.executable, str(HERE / "machine.py"), "--harness", "word_arithmetic_contracts",
                   "--harness", "instruction_byte_contracts"]
        if args.all_features:
            machine.append("--all-features")
        subprocess.run(machine, check=True)
    flags = build_flags(toolchain, args.all_features)
    with tempfile.TemporaryDirectory(prefix="oxide-verus-") as work:
        src = Path(work) / "src"
        shutil.copytree(REPO / "core" / "src", src)
        annotate(src)
        if args.word_arithmetic:
            apply_annotations(src, HERE / "word-arithmetic.patch")
            with (src / "vm/bigint_proofs.rs").open("a") as proofs:
                proofs.write("\n" + (HERE / "word_proofs.rs").read_text())
        command = [verus, "--no-cheating", "--edition=2024", "--crate-type=lib",
                   "--crate-name=oxide_core", *flags]
        if args.whole_core:
            command.append("--no-external-by-default")
        command.append("src/lib.rs")
        if args.independent_bodies:
            return check_independent_bodies(src, command, args.independent_bodies.resolve())
        result = subprocess.run(command, cwd=work)
        if result.returncode == 0 and not args.whole_core:
            hash_dispatch.check(REPO / "core/src/heap/hash.rs", toolchain, flags, work)
        return result.returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
