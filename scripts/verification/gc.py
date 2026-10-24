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
import tomllib
import urllib.request
import zipfile

import arena as arena_proofs
import job_trace as job_trace_proofs
import reuse as reuse_proofs
import root_trace as root_trace_proofs
import roots as root_proofs
import scan as scan_proofs
import source as source_frontend
import symbol as symbol_proofs
import table_delete as table_delete_proofs
import table_index as table_index_proofs
import value as value_proofs
import vm_roots as vm_root_proofs
import vm_resume as vm_resume_proofs
import waiters as waiter_proofs
import weak_cleanup as weak_cleanup_proofs
import finalization as finalization_proofs
import weakpass as weakpass_proofs

ROOT = Path(__file__).resolve().parents[2]
PROOFS = ROOT / "verification/gc"
PIN = json.loads((PROOFS / "toolchain.json").read_text())
EDITION = str(tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["edition"])


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
    result = subprocess.run([str(verus), str(path), "--edition", EDITION, "--triggers-mode", "silent", *flags],
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
    receipt = {"version": version, "edition": EDITION, "source_commit": subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "checks": [], "sha256": {}}
    receipt["obligations"] = json.loads((PROOFS / "obligations.json").read_text())
    receipt["open_obligations"] = open_obligations(receipt["obligations"])
    receipt["complete"] = not receipt["open_obligations"]
    receipt["dirty"] = bool(subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=ROOT, text=True).strip())
    paths = {item["implementation"] for item in receipt["obligations"]}
    paths.update(["Cargo.toml", "core/src/heap/gc_audit.rs", "core/src/heap/gc_tests.rs",
                  "core/src/heap/mod.rs", "core/src/heap/object.rs",
                  "core/src/heap/symbol.rs",
                  "core/src/vm/mod.rs",
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
        symbol = directory / "symbol.rs"
        symbol.write_text(symbol_proofs.proof((ROOT / "core/src/heap/mod.rs").read_text(),
            gc_source, (ROOT / "core/src/heap/object.rs").read_text(),
            (ROOT / "core/src/heap/symbol.rs").read_text(), arena.read_text(), scan.read_text()))
        reuse = directory / "reuse.rs"
        reuse.write_text(reuse_proofs.proof((ROOT / "core/src/heap/arena.rs").read_text(), arena.read_text()))
        root_trace = directory / "root_trace.rs"
        root_trace.write_text(root_trace_proofs.proof(gc_source, roots.read_text(), value.read_text()))
        job_trace = directory / "job_trace.rs"
        job_trace.write_text(job_trace_proofs.proof((ROOT / "core/src/vm/jobs.rs").read_text(), value.read_text()))
        vm_roots = directory / "vm_roots.rs"
        vm_roots.write_text(vm_root_proofs.proof((ROOT / "core/src/heap/mod.rs").read_text(),
            gc_source, (ROOT / "core/src/vm/mod.rs").read_text(), value.read_text()))
        vm_resume = directory / "vm_resume.rs"
        vm_resume.write_text(vm_resume_proofs.proof((ROOT / "core/src/heap/mod.rs").read_text(),
            (ROOT / "core/src/vm/mod.rs").read_text(), (ROOT / "core/src/vm/interp.rs").read_text(),
            value_source, vm_roots.read_text()))
        weak_cleanup = directory / "weak_cleanup.rs"
        weak_cleanup.write_text(weak_cleanup_proofs.proof((ROOT / "core/src/heap/mod.rs").read_text(),
            gc_source, (ROOT / "core/src/heap/object.rs").read_text(), value_source,
            arena.read_text(), scan.read_text()))
        table_index = directory / "table_index.rs"
        table_index.write_text(table_index_proofs.proof((ROOT / "core/src/heap/table.rs").read_text()))
        table_delete = directory / "table_delete.rs"
        table_delete.write_text(table_delete_proofs.proof((ROOT / "core/src/heap/table.rs").read_text(),
            (ROOT / "core/src/heap/object.rs").read_text(), value.read_text(), roots.read_text(),
            table_index.read_text()))
        finalization = directory / "finalization.rs"
        finalization.write_text(finalization_proofs.proof((ROOT / "core/src/heap/gc_scan.rs").read_text(),
            value_source, job_trace.read_text(), scan.read_text(), (PROOFS / "finalization.rs.in").read_text()))
        finalization_heap = directory / "finalization_heap.rs"
        finalization_heap.write_text(finalization_proofs.heap_proof((ROOT / "core/src/heap/mod.rs").read_text(),
            gc_source, (ROOT / "core/src/heap/object.rs").read_text(), arena.read_text(), finalization.read_text()))
        sources = [PROOFS / "model.rs", PROOFS / "storage.rs", PROOFS / "worklist.rs",
                   generated, driver, roots, value, arena, waiters, scan, weakpass, symbol, reuse,
                   root_trace, job_trace, vm_roots, vm_resume, weak_cleanup, table_index, table_delete,
                   finalization, finalization_heap]
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
        receipt["checks"].append(checked_run(binary, symbol, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
        receipt["checks"].append(checked_run(binary, reuse, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
        receipt["checks"].append(checked_run(binary, weak_cleanup, args.timeout,
                                             flags=("--cfg", 'feature="gc-stress"')))
        receipt["checks"].append(checked_run(binary, finalization_heap, args.timeout,
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
                "finalization_wrong_callback": (finalization, "function: callback,", "function: held,"),
                "finalization_wrong_argument": (finalization, "args: vec![held],", "args: vec![target],"),
                "finalization_wrong_this": (finalization, "this: Value::UNDEFINED,", "this: target,"),
                "finalization_reorders_jobs": (finalization, "jobs.push_back(Job::Call {", "jobs.push_front(Job::Call {"),
                "finalization_lost_target_clear": (finalization, "slots[position] = Value::EMPTY;", ""),
                "finalization_lost_held_clear": (finalization, "slots[position + 1] = Value::EMPTY;", ""),
                "finalization_lost_token_clear": (finalization, "slots[position + 2] = Value::EMPTY;\n            } else", "\n            } else"),
                "finalization_marks_inverted": (finalization, "if !marks.contains(target)", "if marks.contains(target)"),
                "finalization_token_inverted": (finalization, "!marks.contains(token)", "marks.contains(token)"),
                "finalization_skips_cell": (finalization, "position += 3;", "position += 6;"),
                "finalization_loses_prefix": (finalization, "let callback = slots[0];", "jobs.clear(); let callback = slots[0];"),
                "finalization_unmarked_callback": (finalization, "marks.live(old(slots)@[0]),", "true,"),
                "finalization_unmarked_held": (finalization,
                    "&& old(slots)@[i].0 != Value::EMPTY.0 ==> marks.live(old(slots)@[i + 1]),\n     ensures",
                    "&& old(slots)@[i].0 != Value::EMPTY.0 ==> true,\n     ensures"),
                "finalization_wrong_snapshot": (finalization_heap,
                    "let marks = ValueMarks {\n                        objects: self.objects.mark_bits(),",
                    "let marks = ValueMarks {\n                        objects: self.strings.mark_bits(),"),
                "finalization_corrupts_heap": (finalization_heap, "let marks = ValueMarks {", "self.html_dda = !self.html_dda; let marks = ValueMarks {"),
                "table_delete_lost_bucket": (table_delete, "table.index[bucket] = DELETED;", ""),
                "table_delete_wrong_sentinel": (table_delete, "table.index[bucket] = DELETED;", "table.index[bucket] = VACANT;"),
                "table_delete_lost_count": (table_delete, "table.live -= 1;", ""),
                "table_delete_inverted_search": (table_delete, "while table.index[bucket] != entry as u32 + 1", "while table.index[bucket] == entry as u32 + 1"),
                "table_delete_lost_slot": (table_delete, "slots[position] = Value::EMPTY;", ""),
                "table_delete_skips_first": (table_delete, "let mut position = 0;", "let mut position = 1;"),
                "table_delete_wrong_range": (table_delete, "[entry * width..entry * width + width]", "[0..width]"),
                "table_delete_corrupts_cursors": (table_delete, "let width = table.width;", "table.cursors.clear(); let width = table.width;"),
                "table_delete_missing_entry": (table_delete,
                    "exists|i: int| 0 <= i < (old(internal).data->Table_0).index@.len()\n                && (old(internal).data->Table_0).index@[i] == entry + 1,", "true,"),
                "table_delete_zero_live": (table_delete, "(old(internal).data->Table_0).live > 0,", "true,"),
                "table_index_lost_write": (table_index, "self.index[bucket] = entry as u32 + 1;", ""),
                "table_index_wrong_entry": (table_index, "self.index[bucket] = entry as u32 + 1;", "self.index[bucket] = entry as u32;"),
                "table_index_ignores_tombstones": (table_index, "while self.index[bucket] != VACANT && self.index[bucket] != DELETED", "while self.index[bucket] != VACANT"),
                "table_index_bad_wrap": (table_index, "bucket = (bucket + 1) & mask;", "bucket = bucket + 1;"),
                "table_index_wrong_start": (table_index, "let mut bucket = hash as usize & mask;", "let mut bucket = 0;"),
                "table_index_corrupts_cursors": (table_index, "let mask = self.index.len() - 1;", "self.cursors.clear(); let mask = self.index.len() - 1;"),
                "table_index_accepts_sentinel": (table_index, "entry < DELETED as usize - 1,", "entry < DELETED as usize,"),
                "table_index_accepts_full_table": (table_index, "exists|i: int| old(self).available(i),", "true,"),
                "weakref_lost_write": (weak_cleanup, "self.payloads.internals.get_mut(payload).slots[0] = Value::UNDEFINED;", ""),
                "weakref_inverted_guard": (weak_cleanup, "if !self.value_marked(target)", "if self.value_marked(target)"),
                "weakref_clears_live": (weak_cleanup, "if !self.value_marked(target)", "if true"),
                "weakref_wrong_slot": (weak_cleanup, "get_mut(payload).slots[0] = Value::UNDEFINED", "get_mut(payload).slots[1] = Value::UNDEFINED"),
                "weakref_corrupts_heap": (weak_cleanup, "let target = self.payloads.internals.get(payload).slots[0];", "self.html_dda = !self.html_dda; let target = self.payloads.internals.get(payload).slots[0];"),
                "weakref_unchecked_slot": (weak_cleanup, "old(self).payloads.internals.records@[payload as int].slots@.len() > 0,", "true,"),
                "vm_resume_lost_clear": (vm_resume, "clear_registers(cx, from as usize..end as usize);", ""),
                "vm_resume_overclear": (vm_resume, "vm.top.max(vm.stale_from)", "vm.top.min(vm.stale_from)"),
                "vm_resume_lost_boundary": (vm_resume, "cx.heap.vm.stale_from = end;", ""),
                "vm_resume_lost_top": (vm_resume, "cx.heap.vm.top = end;", ""),
                "vm_resume_skips_first": (vm_resume, "let mut index = 0;\n    while index < registers.len()", "let mut index = 1;\n    while index < registers.len()"),
                "vm_resume_lost_write": (vm_resume, "registers[index] = Value::UNDEFINED;", ""),
                "vm_resume_corrupts_heap": (vm_resume, "let vm = &mut cx.heap.vm;", "cx.heap.html_dda = !cx.heap.html_dda; let vm = &mut cx.heap.vm;"),
                "vm_resume_without_scan": (vm_resume, "cx.heap.trace_vm_roots(stack);", ""),
                "vm_roots_stale_registers": (vm_roots, "&self.vm.registers[..top]", "&self.vm.registers[..]"),
                "vm_roots_lost_function": (vm_roots, "stack.push(Value::object(self.vm.frames[frame].function));", ""),
                "vm_roots_lost_stale_boundary": (vm_roots,
                    "self.vm.stale_from = self.vm.stale_from.min(self.vm.top);", "self.vm.stale_from = self.vm.top;"),
                "vm_roots_wrong_kind": (vm_roots, "Value::object(self.vm.frames[frame].function)",
                    "Value::string(StringId(self.vm.frames[frame].function.0))"),
                "vm_roots_lost_prefix": (vm_roots, "let top = self.vm.top as usize;",
                    "stack.clear(); let top = self.vm.top as usize;"),
                "vm_roots_corrupts_heap": (vm_roots, "let top = self.vm.top as usize;",
                    "self.html_dda = !self.html_dda; let top = self.vm.top as usize;"),
                "arena_initial_cursor": (arena, "cursor: 0,", "cursor: 1,"),
                "arena_initial_quarantine": (arena, "quarantine: Vec::new(),", "quarantine: vec![0u64],"),
                "job_lost_record": (job_trace, "stack.push(Value::object(*record));", ""),
                "job_lost_argument": (job_trace, "stack.push(*argument);", ""),
                "job_lost_promise": (job_trace, "stack.push(Value::object(*promise));", ""),
                "job_lost_thenable": (job_trace, "stack.push(*thenable);", ""),
                "job_lost_then": (job_trace, "stack.push(*then);", ""),
                "job_lost_function": (job_trace, "stack.push(*function);", ""),
                "job_lost_this": (job_trace, "stack.push(*this);", ""),
                "job_lost_args": (job_trace, "stack.extend_from_slice(args);", ""),
                "root_trace_lost_stack": (root_trace, "push_heap_values(stack, self.stack.values());", ""),
                "root_trace_lost_handles": (root_trace, "push_heap_values(stack, &self.handles.slots);", ""),
                "root_trace_lost_fixed": (root_trace, "push_heap_values(stack, &self.fixed);", ""),
                "root_trace_lost_prefix": (root_trace, "push_heap_values(stack, self.stack.values());",
                    "stack.clear(); push_heap_values(stack, self.stack.values());"),
                "identity_accepts_noncanonical": (value,
                    "requires canonical_heap(a.0), canonical_heap(b.0),",
                    "requires heap_kind(kind_of(a.0)), heap_kind(kind_of(b.0)),"),
                "identity_forgets_namespace": (value,
                    "ensures (a == b) == (kind_of(a.0) == kind_of(b.0) && a.0 as u32 == b.0 as u32),",
                    "ensures (a == b) == (a.0 as u32 == b.0 as u32),"),
                "reuse_highest_slot": (reuse, "(!taken).trailing_zeros()", "63 - (!taken).leading_zeros()"),
                "reuse_lost_count": (reuse, "self.free_count -= 1;", ""),
                "reuse_skips_cursor": (reuse, "let mut word = self.cursor;", "let mut word = self.cursor + 1;"),
                "reuse_wrong_bit": (reuse, "self.live[word] |= 1 << low;", "self.live[word] |= 1;"),
                "reuse_corrupts_marks": (reuse, "self.free_count -= 1;", "self.free_count -= 1; self.marks[word] = 0;"),
                "reuse_ignores_quarantine": (reuse, "return self.live[word] | self.quarantine[word];", "return self.live[word];"),
                "symbol_lost_description": (symbol,
                    "stack.push(Value::string(crate::StringId(description)));", ""),
                "symbol_lost_activation": (symbol, "stack.append(&mut values);", "values.clear();"),
                "symbol_repeats_edges": (symbol, "if self.symbols.mark(index) {",
                    "self.symbols.mark(index); if true {"),
                "symbol_wrong_description_kind": (symbol,
                    "Value::string(crate::StringId(description))", "Value::symbol(SymbolId(description))"),
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
                flags = ("--cfg", 'feature="gc-stress"') if name in (
                    "unchecked_live_access", "reuse_ignores_quarantine", "arena_initial_quarantine") else ()
                receipt["checks"].append(checked_run(binary, path, args.timeout, True, flags))
            # Recreate the old description-marking mistake on an explicitly
            # live description. Rejection must not depend only on a missing
            # liveness premise for the newly introduced string-arena access.
            content = symbol.read_text()
            statement = "stack.push(Value::string(crate::StringId(description)));"
            premise = "old(self).symbols.valid_marks(), old(self).symbols.live_at(value.0 as u32),"
            if content.count(statement) != 1 or content.count(premise) != 1:
                raise RuntimeError("early description marking mutation lost its unique target")
            content = content.replace(statement, "self.strings.mark(description);")
            content = content.replace(premise, premise + """
                old(self).strings.valid_marks(),
                old(self).symbols.records@[(value.0 as u32) as int].description != NONE ==>
                    old(self).strings.live_at(old(self).symbols.records@[(value.0 as u32) as int].description),
            """)
            path = directory / "mutant_symbol_early_description_mark.rs"
            path.write_text(content)
            receipt["checks"].append(checked_run(binary, path, args.timeout, True))
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
