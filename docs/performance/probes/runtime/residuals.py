#!/usr/bin/env python3
"""Inventory the internal strong `Runtime` owners that stage B must remove.

Usage:
    residuals.py [--json OUT] [--qjs PROFILING_QJS --v8 V8_DIR --work DIR]

The lexical part lists named struct fields whose type is `Runtime` or
`Option<Runtime>` (a strong owner; borrows are not counted) outside test code,
the same population as `runtime-b0-residuals.json`. Each field keeps its B0
identifier when the file, container and field name still match; B0 entries
that no longer exist are reported as closed.

With `--qjs` (a CLI built with `--features profiling`) the eight V8 bodies run
with the iteration counts of the Callgrind profile, and the hard-gate counters
(`core.runtime_clone`, `core.state.*`, public root traffic, native call
declines) are recorded per case.
"""
import argparse, json, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
B0 = ROOT / "docs/performance/runtime-b0-residuals.json"
OWNER = re.compile(
    r"(\w+)\s*:\s*(Option\s*<\s*)?(?:crate::engine::api::runtime::)?Runtime\s*(>)?\s*[,}\n]")
ITERATIONS = {"richards": 10, "deltablue": 10, "navier-stokes": 10, "crypto": 3,
              "raytrace": 3, "earley-boyer": 2, "regexp": 1, "splay": 2}
COUNTERS = ("core.runtime_clone", "core.state.borrow", "core.state.borrow_mut",
            "core.state.try_borrow", "core.state.try_borrow_mut", "core.object_root.clone",
            "core.object_root.promote", "core.object_root.adopt", "core.atom_root.clone",
            "core.var_ref_root.promote", "core.call_decline.native_hint",
            "core.call_decline.native", "core.legacy_boundary.ordinary_call")


def strip_tests(text):
    # A `#[cfg(test)]` module runs to the end of its file in this codebase.
    match = re.search(r"#\[cfg\(test\)\]\s*\n\s*(pub(\([^)]*\))?\s+)?mod \w+\s*\{", text)
    return text[:match.start()] if match else text


def struct_fields(path):
    text = strip_tests(path.read_text())
    for struct in re.finditer(r"\bstruct\s+(\w+)[^{;(]*\{", text):
        depth, end = 1, struct.end()
        while depth and end < len(text):
            depth += {"{": 1, "}": -1}.get(text[end], 0)
            end += 1
        body = text[struct.end():end - 1]
        for field in OWNER.finditer(body):
            if bool(field.group(2)) != bool(field.group(3)):
                continue
            line = text.count("\n", 0, struct.end() + field.start()) + 1
            yield struct.group(1), field.group(1), line, \
                "Option<Runtime>" if field.group(2) else "Runtime"


def inventory():
    b0 = json.loads(B0.read_text())["runtime_struct_fields"]
    known = {(r["file"], r["container"], r["field"]): r for r in b0}
    fields, seen = [], set()
    for path in sorted((ROOT / "src").rglob("*.rs")):
        relative = str(path.relative_to(ROOT))
        if path.name.endswith("tests.rs") or "/tests/" in relative:
            continue
        for container, field, line, kind in struct_fields(path):
            key = (relative, container, field)
            entry = known.get(key)
            seen.add(key)
            fields.append({"file": relative, "line": line, "container": container,
                           "field": field, "type": kind,
                           "id": entry["id"] if entry else None,
                           "batch": entry["batch"] if entry else None})
    closed = [r["id"] for key, r in known.items() if key not in seen]
    return fields, closed


def counters(qjs, v8, work):
    sys.path.insert(0, str(ROOT / "scripts/benchmark"))
    import profile_v8
    work.mkdir(parents=True, exist_ok=True)
    report = {}
    for case, iterations in ITERATIONS.items():
        body = (v8 / f"{case}.js").read_text()
        suite, benchmarks = profile_v8.declared_benchmarks(body, case)
        script = work / f"{case}.js"
        script.write_text((v8 / "base.js").read_text() + "\n" + body + "\n" +
                          profile_v8.fixed_driver(suite, benchmarks, iterations).decode())
        output = work / f"{case}.jsonl"
        output.unlink(missing_ok=True)
        subprocess.run([qjs, "-d", "--profile-json", "--profile-output", str(output), str(script)],
                       check=True, capture_output=True)
        events = dict.fromkeys(COUNTERS, 0)

        def walk(node):
            if isinstance(node, dict):
                for key, value in node.items():
                    if key in events and isinstance(value, int):
                        events[key] += value
                    else:
                        walk(value)
            elif isinstance(node, list):
                for value in node:
                    walk(value)
        for line in output.read_text().splitlines():
            walk(json.loads(line))
        report[case] = events
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--json", type=Path)
    parser.add_argument("--qjs")
    parser.add_argument("--v8", type=Path)
    parser.add_argument("--work", type=Path)
    args = parser.parse_args()
    fields, closed = inventory()
    commit = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    result = {"commit": commit, "runtime_struct_fields": fields, "closed_b0_ids": closed}
    by_file = {}
    for field in fields:
        by_file[field["file"]] = by_file.get(field["file"], 0) + 1
    print(f"{len(fields)} strong Runtime fields in {len(by_file)} files "
          f"({sum(1 for f in fields if f['id'] is None)} without a B0 id); "
          f"{len(closed)} B0 entries closed")
    if args.qjs:
        if not (args.v8 and args.work):
            parser.error("--qjs needs --v8 and --work")
        result["counters"] = counters(args.qjs, args.v8, args.work)
        for case, events in result["counters"].items():
            print(f"  {case:14} runtime_clone {events['core.runtime_clone']:>11,}  "
                  f"state {sum(v for k, v in events.items() if k.startswith('core.state')):>11,}")
    if args.json:
        args.json.write_text(json.dumps(result, indent=1) + "\n")


if __name__ == "__main__":
    main()
