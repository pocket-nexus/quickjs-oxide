#!/usr/bin/env python3
"""Pinned Verus proofs, verbatim production kernels, and negative proof tests."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PROOFS = ROOT / "verification/gc"
PIN = json.loads((PROOFS / "toolchain.json").read_text())


def function(source, name):
    """Extract a restricted, brace-balanced production function, without rewriting it."""
    match = re.search(rf"pub\(crate\) fn {name}(?:<[^>]+>)?\([^{{]+\{{", source)
    if not match:
        raise ValueError(f"missing kernel {name}")
    opening = match.end() - 1
    depth = 1
    end = opening + 1
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    if depth:
        raise ValueError(f"unbalanced kernel {name}")
    body = source[opening:end]
    # These kernels intentionally contain no comments, strings, macros or attributes;
    # reject new syntax the little extractor is not equipped to understand.
    checked = body.replace("// verification-loop: stable phase boundary", "")
    if any(token in checked for token in ('"', "'", "//", "/*", "!\n", "#[")):
        raise ValueError(f"unsupported syntax in kernel {name}")
    return source[match.start():opening].replace("pub(crate)", "pub"), body


def kernel_proof(source):
    contracts = {
        "mark_word": (
            "(u64, bool)",
            "result.0 == bits | mask, result.1 == (bits & mask != mask), "
            "result.0 & bits == bits, result.0 & mask == mask, "
            "result.0 & !mask == bits & !mask",
            "assert(((bits | mask) & bits) == bits && ((bits | mask) & mask) == mask "
            "&& ((bits | mask) & !mask) == (bits & !mask)) by(bit_vector);",
        ),
        "dead_word": (
            "u64", "result == live & !marked, result & marked == 0, "
            "(live & !result) == live & marked, result & !live == 0",
            "assert(((live & !marked) & marked) == 0 && "
            "(live & !(live & !marked)) == (live & marked) && "
            "((live & !marked) & !live) == 0) by(bit_vector);",
        ),
        "ephemeron_action": (
            "u8", "result <= 2, (result == 0) == (empty || value_live), "
            "(result == 1) == (!empty && !value_live && key_live), "
            "(result == 2) == (!empty && !value_live && !key_live)", "",
        ),
    }
    code = ["use vstd::prelude::*;\nverus! {\n"]
    for name, (ty, ensures, proof) in contracts.items():
        signature, body = function(source, name)
        signature = signature[:signature.rindex("->")] + f"-> (result: {ty})\n"
        # The original body is an unchanged expression. Only ghost proof code is added.
        code.append(signature + "ensures " + ensures + ",\n{\nlet result = " + body
                    + ";\nproof { " + proof + " }\nresult\n}\n")
    return "".join(code) + "pub fn main() {}\n}\n"


def driver_proof(source):
    signature, body = function(source, "close_ephemerons")
    if body.count("loop {") != 1:
        raise ValueError("driver loop no longer has a unique annotation point")
    # Add specifications and a loop invariant, without modifying executable tokens.
    body = body.replace("loop {", """loop
        invariant trace.graph().valid(), trace.graph() == old(trace).graph(),
            trace.work().valid(trace.graph()), trace.work().pending.is_empty(),
        ensures trace.work().marked == trace.graph().reachable(),
        decreases trace.remaining(),
        {""")
    contract = """
        requires old(trace).graph().valid(), old(trace).work().valid(old(trace).graph()),
            old(trace).work().pending.is_empty(),
        ensures final(trace).graph() == old(trace).graph(),
            final(trace).work().marked == final(trace).graph().reachable(),
            final(trace).work().pending.is_empty(), final(trace).work().waiting.is_empty(),
    """
    return (PROOFS / "driver.rs.in").read_text().replace("// DRIVER", signature + contract + body)


def install(directory):
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / "verus.zip"
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != PIN["sha256"]:
        temporary = archive.with_suffix(".download")
        urllib.request.urlretrieve(PIN["url"], temporary)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != PIN["sha256"]:
            temporary.unlink()
            raise RuntimeError("Verus archive checksum mismatch")
        temporary.replace(archive)
    binary = directory / "verus-x86-linux/verus"
    if not binary.exists():
        with zipfile.ZipFile(archive) as bundle:
            for info in bundle.infolist():
                dest = (directory / info.filename).resolve()
                if not dest.is_relative_to(directory.resolve()):
                    raise RuntimeError("unsafe archive path")
                bundle.extract(info, directory)
                if info.external_attr >> 16:
                    dest.chmod(info.external_attr >> 16)
    subprocess.run(["rustup", "toolchain", "install", PIN["rust"], "--profile", "minimal"], check=True)
    return binary


def checked_run(verus, path, timeout, expect_failure=False):
    started = time.monotonic()
    result = subprocess.run([str(verus), str(path), "--triggers-mode", "silent"],
                            text=True, capture_output=True, timeout=timeout)
    output = result.stdout + result.stderr
    match = re.search(r"verification results:: (\d+) verified, (\d+) errors", output)
    if expect_failure:
        # A parse/type error, crash, or timeout is not a successful negative test.
        ok = result.returncode != 0 and match and int(match[2]) > 0
    else:
        ok = result.returncode == 0 and match and int(match[2]) == 0 and int(match[1]) > 0
    print(f"{'PASS' if ok else 'FAIL'} {path.name}", flush=True)
    if not ok:
        raise RuntimeError(output)
    return {"file": path.name, "verified": int(match[1]), "errors": int(match[2]),
            "expected_failure": expect_failure, "seconds": time.monotonic() - started, "output": output}


def reject_trust_escapes(source):
    source = re.sub(r"//[^\n]*|/\*.*?\*/", "", source, flags=re.S)
    if re.search(r"\b(assume|admit|external_body|external_fn_specification|axiom)\b", source):
        raise ValueError("proof contains a trust escape")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", type=Path, default=os.environ.get("VERUS"))
    parser.add_argument("--install", action="store_true")
    parser.add_argument("--mutations", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    binary = args.verus
    if args.install:
        binary = install(ROOT / "target/verification/tools" / PIN["version"])
    if binary is None:
        found = shutil.which("verus")
        if found:
            binary = Path(found)
        else:
            parser.error("provide --verus /path/to/verus, VERUS, or --install")
    version = subprocess.check_output([str(binary), "--version"], text=True)
    if f"Version: {PIN['version']}" not in version:
        parser.error(f"expected pinned Verus {PIN['version']}, got {version}")
    receipt = {"version": version, "source_commit": subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "checks": [], "sha256": {}}
    receipt["obligations"] = json.loads((PROOFS / "obligations.json").read_text())
    paths = {item["implementation"] for item in receipt["obligations"]}
    paths.update(["core/src/heap/gc_audit.rs", "core/src/heap/gc_tests.rs",
                  "core/src/vm/regexp.rs", "verification/gc/driver.rs.in",
                  "verification/gc/obligations.json", "verification/gc/toolchain.json",
                  "scripts/verification/gc.py"])
    for name in sorted(paths):
        receipt["sha256"][name] = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix="oxide-gc-proof-") as tmp:
        directory = Path(tmp)
        kernel = (ROOT / "core/src/heap/gc_kernel.rs").read_text()
        generated = directory / "kernel.rs"
        generated.write_text(kernel_proof(kernel))
        shutil.copyfile(PROOFS / "model.rs", directory / "model.rs")
        driver = directory / "driver.rs"
        driver.write_text(driver_proof(kernel))
        sources = [PROOFS / "model.rs", PROOFS / "storage.rs", generated, driver]
        for path in sources:
            source = path.read_text()
            reject_trust_escapes(source)
            receipt["sha256"][path.name] = hashlib.sha256(source.encode()).hexdigest()
            receipt["checks"].append(checked_run(binary, path, args.timeout))
        receipt["sha256"]["gc_kernel.rs"] = hashlib.sha256(kernel.encode()).hexdigest()
        if args.mutations:
            source = (PROOFS / "model.rs").read_text()
            mutations = {
                "dead_map": ("w.valid(g), w.marked.contains(m),", "w.valid(g),"),
                "lost_activation": (".union(children).union(awakened)", ".union(children)"),
                "stale_marks": ("Work { marked: Set::empty(), pending: g.roots, expanded: Set::empty(),",
                                "Work { marked: g.nodes, pending: g.roots, expanded: g.nodes,"),
            }
            for name, (before, after) in mutations.items():
                if source.count(before) != 1:
                    raise RuntimeError(f"mutation {name} no longer has a unique target")
                path = directory / f"mutant_{name}.rs"
                path.write_text(source.replace(before, after))
                receipt["checks"].append(checked_run(binary, path, args.timeout, True))
            broken = kernel.replace("live & !marked", "live & marked")
            path = directory / "mutant_sweep.rs"
            path.write_text(kernel_proof(broken))
            receipt["checks"].append(checked_run(binary, path, args.timeout, True))
            for name, statement in (("driver_no_drain", "trace.drain();"),
                                    ("driver_no_clear", "trace.clear();")):
                if kernel.count(statement) != 1:
                    raise RuntimeError(f"mutation {name} no longer has a unique target")
                path = directory / f"mutant_{name}.rs"
                path.write_text(driver_proof(kernel.replace(statement, "")))
                receipt["checks"].append(checked_run(binary, path, args.timeout, True))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    main()
