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

import arena as arena_proofs
import roots as root_proofs
import scan as scan_proofs
import source as source_frontend
import value as value_proofs
import waiters as waiter_proofs
import weakpass as weakpass_proofs

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
        decreases remaining(trace),
        {""")
    if body.count("trace.drain();") != 1:
        raise ValueError("driver drain no longer has a unique annotation point")
    body = body.replace("trace.drain();", """let ghost before_drain = trace.work();
            trace.drain();
            proof { drain_progress(trace.graph(), before_drain, trace.work()); }""")
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


def checked_run(verus, path, timeout, expect_failure=False, flags=()):
    started = time.monotonic()
    result = subprocess.run([str(verus), str(path), "--triggers-mode", "silent", *flags],
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
    return {"file": path.name, "flags": list(flags), "verified": int(match[1]), "errors": int(match[2]),
            "expected_failure": expect_failure, "seconds": time.monotonic() - started, "output": output}


def reject_trust_escapes(source):
    source = source_frontend.masked(source)
    if re.search(r"\b(assume|assume_specification|admit|external_body|external_fn_specification|"
                 r"external_type_specification|external_trait_specification|external_trait_extension|"
                 r"uninterp|axiom)\b|verifier\s*::\s*external\b", source):
        raise ValueError("proof contains a trust escape")


def open_obligations(obligations):
    # Model theorems discharge model obligations, not implementation obligations.
    # A source theorem with unproved adapter contracts is still an open boundary.
    discharged = {"proved-model", "proved-source"}
    return [item["id"] for item in obligations if item["status"] not in discharged
            and not (item["id"] == "GC-RESOURCES" and item["status"] == "assumption")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", type=Path, default=os.environ.get("VERUS"))
    parser.add_argument("--install", action="store_true")
    parser.add_argument("--mutations", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--require-complete", action="store_true",
                        help="fail on open implementation/caller obligations; retain the explicit host resource preconditions")
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
    receipt["open_obligations"] = open_obligations(receipt["obligations"])
    receipt["complete"] = not receipt["open_obligations"]
    receipt["dirty"] = bool(subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=ROOT, text=True).strip())
    paths = {item["implementation"] for item in receipt["obligations"]}
    paths.update(["core/src/heap/gc_audit.rs", "core/src/heap/gc_tests.rs",
                  "core/src/heap/mod.rs", "core/src/heap/object.rs",
                  "core/src/vm/regexp.rs", "verification/gc/driver.rs.in",
                  "verification/gc/obligations.json", "verification/gc/toolchain.json",
                  "scripts/verification/gc.py"])
    paths.update(str(path.relative_to(ROOT)) for path in PROOFS.iterdir() if path.is_file())
    paths.update(str(path.relative_to(ROOT)) for path in (ROOT / "scripts/verification").glob("*.py"))
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
        gc_source = (ROOT / "core/src/heap/gc.rs").read_text()
        value_source = (ROOT / "core/src/value.rs").read_text()
        arena_source = (ROOT / "core/src/heap/arena.rs").read_text()
        waiters_source = (ROOT / "core/src/heap/gc_waiters.rs").read_text()
        roots = directory / "roots.rs"
        roots.write_text(root_proofs.proof(gc_source, value_source, (PROOFS / "roots.rs.in").read_text()))
        value = directory / "value.rs"
        value.write_text(value_proofs.proof(value_source, (PROOFS / "value.rs.in").read_text(), gc_source))
        arena = directory / "arena.rs"
        arena.write_text(arena_proofs.proof(arena_source, (PROOFS / "arena.rs.in").read_text()))
        waiters = directory / "waiters.rs"
        waiters.write_text(waiter_proofs.proof(waiters_source, value_source,
                                              (PROOFS / "waiters.rs.in").read_text()))
        scan = directory / "scan.rs"
        scan.write_text(scan_proofs.proof((ROOT / "core/src/heap/gc_scan.rs").read_text(),
            (PROOFS / "scan.rs.in").read_text(), value.read_text(), waiters.read_text(),
            generated.read_text(), roots.read_text()))
        weakpass = directory / "weakpass.rs"
        weakpass.write_text(weakpass_proofs.proof((ROOT / "core/src/heap/mod.rs").read_text(),
            gc_source, (ROOT / "core/src/heap/object.rs").read_text(), arena.read_text(), scan.read_text()))
        sources = [PROOFS / "model.rs", PROOFS / "storage.rs", PROOFS / "worklist.rs",
                   generated, driver, roots, value, arena, waiters, scan, weakpass]
        for path in sources:
            source = path.read_text()
            reject_trust_escapes(source)
            receipt["sha256"][path.name] = hashlib.sha256(source.encode()).hexdigest()
            receipt["checks"].append(checked_run(binary, path, args.timeout))
        receipt["checks"].append(checked_run(binary, arena, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
        receipt["checks"].append(checked_run(binary, waiters, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
        receipt["checks"].append(checked_run(binary, scan, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
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
            # Change executable statements in the extracted production bodies.
            # Ghost assertions/contracts remain unchanged. Rejection must come
            # from failed proof obligations, never from unsupported Rust syntax.
            concrete_mutations = {
                "weakpass_skips_maps": (weakpass,
                    "for position in self.scanned..heap.gc.weak.len()",
                    "for position in heap.gc.weak.len()..heap.gc.weak.len()"),
                "weakpass_lost_cursor": (weakpass,
                    "self.scanned = heap.gc.weak.len();", "self.scanned = self.scanned;"),
                "weakpass_false_done": (weakpass, "!self.stack.is_empty()", "false"),
                "weakpass_dead_map": (weakpass,
                    "self.payloads.internals.live_at(self.gc.weak@[p])\n                && self.payloads.internals.marked_at(self.gc.weak@[p])",
                    "self.payloads.internals.live_at(self.gc.weak@[p])"),
                "weakpass_wrong_snapshot": (weakpass,
                    "strings: heap.strings.mark_bits(),", "strings: heap.symbols.mark_bits(),"),
                "weakpass_corrupts_heap": (weakpass,
                    "self.heap.gc.ephemerons.clear();",
                    "self.heap.gc.ephemerons.clear(); self.heap.global_generation = 0;"),
                "scan_lost_ready": (scan, "1 => stack.push(value),", "1 => {},"),
                "scan_lost_waiter": (scan, "_ => waiters.defer(key, value),", "_ => {},"),
                "scan_skips_entry": (scan, "position += 2;", "position += 4;"),
                "scan_wrong_arena": (scan, "Kind::Symbol => marked(self.symbols, value.as_symbol().unwrap().0)",
                    "Kind::Symbol => marked(self.objects, value.as_symbol().unwrap().0)"),
                "waiter_value_lost": (waiters, "waiting.values.push(value);", "let _ = value;"),
                "waiter_wrong_key": (waiters, "self.by_key.remove(&key.bits())", "self.by_key.remove(&0u64)"),
                "waiter_values_lost": (waiters, "Some(waiting) => Some(waiting.values)", "Some(waiting) => Some(Vec::new())"),
                "waiters_not_cleared": (waiters, "self.by_key.clear();", ""),
                "scan_without_freshness": (driver,
                    "more ==> !final(self).work().pending.difference(final(self).work().marked).is_empty();",
                    "more ==> !final(self).work().pending.is_empty();"),
                "root_prefix_lost": (roots, "self.values.truncate(mark.0);", "self.values.truncate(0);"),
                "handle_not_released": (roots, "*slot = Value::EMPTY;", "*slot = *slot;"),
                "symbol_not_heap": (value, "tag >= TAG_OBJECT && tag <= TAG_BIGINT", "tag >= TAG_OBJECT && tag < TAG_SYMBOL"),
                "inverted_root_filter": (value, "if value.is_heap() {", "if !value.is_heap() {"),
                "marks_not_cleared": (arena, "self.marks[word] = 0;", "self.marks[word] = 1;"),
                "wrong_arena_bit": (arena, "*bits = marked;", "*bits = 0;"),
                "unchecked_live_access": (arena, "requires self.live_at(index),", ""),
            }
            for name, (original, before, after) in concrete_mutations.items():
                content = original.read_text()
                if content.count(before) != 1:
                    raise RuntimeError(f"mutation {name} no longer has a unique target")
                path = directory / f"mutant_{name}.rs"
                path.write_text(content.replace(before, after))
                flags = ("--cfg", 'feature="gc-stress"') if name == "unchecked_live_access" else ()
                receipt["checks"].append(checked_run(binary, path, args.timeout, True, flags))
            content = (PROOFS / "worklist.rs").read_text()
            before = "decreases g.nodes.difference(q.state.marked).len(), q.queue.len(),"
            if content.count(before) != 1:
                raise RuntimeError("queue rank mutation no longer has a unique target")
            path = directory / "mutant_queue_set_rank.rs"
            path.write_text(content.replace(before,
                "decreases g.nodes.difference(q.state.marked).len(), q.state.pending.len(),"))
            receipt["checks"].append(checked_run(binary, path, args.timeout, True))
            broken = kernel.replace("live & !marked", "live & marked")
            path = directory / "mutant_sweep.rs"
            path.write_text(kernel_proof(broken))
            receipt["checks"].append(checked_run(binary, path, args.timeout, True))
            for name, statement in (("driver_no_drain", "trace.drain();"),
                                    ("driver_no_clear", "trace.clear();")):
                content = driver.read_text()
                if content.count(statement) != 1:
                    raise RuntimeError(f"mutation {name} no longer has a unique target")
                path = directory / f"mutant_{name}.rs"
                path.write_text(content.replace(statement, ""))
                receipt["checks"].append(checked_run(binary, path, args.timeout, True))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.require_complete and not receipt["complete"]:
        parser.exit(2, "Incomplete GC verification: " + ", ".join(receipt["open_obligations"]) + "\n")


if __name__ == "__main__":
    main()
