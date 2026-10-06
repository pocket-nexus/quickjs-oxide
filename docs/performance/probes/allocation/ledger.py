#!/usr/bin/env python3
"""Mutually exclusive per-iteration instruction ledger for object allocation.

Each probe runs at N and 2N under Callgrind with caller-context separation.
The marginal self cost of every (function, caller chain) context is assigned
to exactly one phase and one mechanism, so all cells sum to the measured
per-iteration slope. A third run at 4N checks that the slope is linear.
"""
import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

PROBES = {
    "empty_loop": "function work(n){ var s; for (var i=0;i<n;i++){ s=i; } return s; }\n"
                  "print(work(N));\n",
    "object_literal": "function work(n){ var s; for (var i=0;i<n;i++){ s={a:i,b:i}; } return s.a; }\n"
                      "print(work(N));\n",
    "constructor": "function P(x){ this.a=x; this.b=x; }\n"
                   "function work(n){ var s; for (var i=0;i<n;i++){ s=new P(i); } return s.a; }\n"
                   "print(work(N));\n",
}
EXPECTED = {"empty_loop": lambda n: n - 1, "object_literal": lambda n: n - 1,
            "constructor": lambda n: n - 1}

# Phase is decided by the caller chain (outermost match wins by rule order).
PHASES = [
    ("D", "字面量字段定义（离开解释循环）", ["cold::define_property", "construct_driver::define_property"]),
    ("A", "对象分配", ["new_ordinary_object_in_realm", "allocate_object_with_layout",
                      "prepare_ordinary_base_in_state"]),
    ("F", "覆盖局部变量：释放上一个对象", None),
    ("W", "this.x = … 追加字段", ["append_missing_owned_data", "select_set_slot",
                                   "finish_added_field_write", "set_missing_local",
                                   "retire_field_write"]),
    ("C", "构造调用与返回", ["enter_constructor", "CallStorage::recycle", "::recycle",
                            "FrameCold::release", "release_eval_arguments", "finish_ordinary",
                            "clear_current_frame_owned", "FramePush", "prepare_ordinary_window"]),
    ("S", "局部写入与值复制", ["transfer_direct_in_state", "insert_copy_in_state", "rotate_operands",
                              "copy_reference_in_state", "dup_jsvalue", "retain_raw_root",
                              "top_direct_mut", "global_cell_view"]),
]
# Mechanism is decided by the innermost (self) function.
MECHANISMS = [
    ("shape 转移与查找", ["canonical_successor", "record_transition", "append_transition",
                         "get_or_create_shape", "shape_cache", "Shape::"]),
    ("slot 追加与写入", ["append_slot_with_owned_shape", "append_selected_missing_slot",
                        "store_property_slot", "store_selected_property_slot", "Slots::",
                        "append_missing_owned_data", "set_missing_local", "retire_field_write",
                        "finish_added_field_write"]),
    ("边与 atom 引用", ["retain_edges_transactionally", "object_edges", "property_slot_edges",
                       "retain_slot_atoms", "retain_atom_handle", "release_atom", "AtomOwner",
                       "Edges::", "edges::Edges", "NodeData::edges"]),
    ("引用计数、释放与回收", ["release_", "apply_cleanup", "drain_zero_queue", "finish_resident_node",
                            "retain_raw_root", "dup_jsvalue", "retain_object", "strong_count",
                            "OwnedValueGuard", "apply_deferred", "finish_reference_release",
                            "copy_reference", "drop_glue::<heap::"]),
    ("查找、准入与校验", ["validate_", "parse_canonical", "can_define", "ordinary_property_flags",
                         "try_define", "define_raw_property", "locate", "select_set_slot",
                         "select_missing_prototypes", "array_own_key", "authenticate",
                         "DirectSelection", "is_extensible", "Heap::object", "function_bytecode",
                         "dictionary_layout_is_valid"]),
    ("公共包装与协议对象", ["PropertyKey", "OwnedPropertyDescriptor", "ObjectRef", "PrivateNameRef",
                           "start_public_field", "finish_instruction_call", "define_property",
                           "prepare_ordinary_base", "finish_public_class_field"]),
    ("帧、栈与驱动", ["materialize", "ready::run", "run_frames", "dispatch", "admit",
                     "deferred_action", "next_pc", "enter_constructor", "recycle", "FrameCold",
                     "clear_current", "FramePush", "FrameStore", "prepare_ordinary_window", "peek",
                     "pop", "insert_copy", "rotate_operands", "top_direct", "transfer_direct",
                     "with_slots", "commit_owned", "move_owned", "global_cell_view", "CallStorage",
                     "closure::Environment", "Published"]),
    ("分配器与内存拷贝", ["malloc", "free", "memcpy", "memmove", "realloc", "alloc::"]),
    ("对象创建主体", ["new_ordinary_object", "allocate_object"]),
    ("解释循环自身", ["execute_frame_in_state"]),
]
LOOP_SELF = ("execute_frame_in_state", "with_slots", "commit_owned", "move_owned", "binary_number")
DRIVER = ("materialize", "run_frames_with_state", "cold::dispatch", "FrameExecution::admit",
          "FrameTransaction::check_current", "deferred_action")


def short(name: str) -> str:
    """Normalize symbol spelling across rustc versions (brackets, impl blocks)."""
    name = re.sub(r"::h[0-9a-f]{16}", "", name).replace("quickjs_oxide::engine::", "")
    name = re.sub(r"<impl ([^<>]*)>", r"\1", name)
    return name.replace("<", "").replace(">", "")


def has(chain, patterns):
    return any(p in c for c in chain for p in patterns)


def phase_of(chain):
    for key, label, patterns in PHASES:
        if key == "F":
            if has(chain, ["transfer_direct_in_state"]) and has(chain, ["release_jsvalue",
                                                                         "release_heap_reference"]):
                return f"{key} {label}"
            continue
        if has(chain, patterns):
            return f"{key} {label}"
    if any(k in chain[0] for k in LOOP_SELF):
        return "I 解释循环自身"
    # Only frames below the innermost interpreter call decide loop vs driver work.
    inner = chain
    for index, frame in enumerate(chain):
        if "execute_frame_in_state" in frame:
            inner = chain[:index + 1]
            break
    else:
        return "R 退出与重入解释循环"
    if has(inner, DRIVER):
        return "R 退出与重入解释循环"
    if has(inner, ["release_jsvalue", "release_heap_reference"]):
        return "T 其他释放（操作数与临时值）"
    return "I 解释循环自身"


def mechanism_of(function):
    for label, patterns in MECHANISMS:
        if any(p in function for p in patterns):
            return label
    return "其他"


def parse_contexts(path: Path):
    names, costs, current, in_call = {}, collections.Counter(), None, False
    for raw in path.read_text(errors="replace").splitlines():
        if not raw:
            continue
        if raw.startswith(("fn=", "cfn=")):
            match = re.match(r"c?fn=\((\d+)\)(?: (.*))?", raw)
            if match and match.group(2):
                names[match.group(1)] = match.group(2)
            if raw.startswith("fn="):
                current = names[match.group(1)]
            continue
        if raw.startswith("calls="):
            in_call = True
            continue
        if raw[0] in "0123456789+-*":
            if in_call:
                in_call = False
                continue
            parts = raw.split()
            if len(parts) >= 2:
                costs[current] += int(parts[1])
    return costs


def run(command):
    return subprocess.run(command, capture_output=True, text=True, check=False)


def total_instructions(qjs, script):
    result = run(["valgrind", "--tool=cachegrind", "--cache-sim=no",
                  "--cachegrind-out-file=/dev/null", qjs, str(script)])
    match = re.search(r"I\s+refs:\s+([\d,]+)", result.stderr)
    if not match:
        sys.exit(f"cachegrind failed for {script}: {result.stderr[-400:]}")
    return int(match.group(1).replace(",", "")), result.stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qjs", required=True, help="ordinary release qjs (with symbols)")
    parser.add_argument("--reference", help="optional reference engine for totals only")
    parser.add_argument("--n", type=int, default=100_000)
    parser.add_argument("--output", required=True)
    parser.add_argument("--reuse", action="store_true",
                        help="reclassify existing probe and Callgrind files in --output")
    args = parser.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=args.reuse)
    report = {"n": args.n, "probes": {}}
    for probe, template in PROBES.items():
        scripts = {}
        for multiple in (1, 2, 4):
            n = args.n * multiple
            path = out / f"{probe}_{n}.js"
            if not args.reuse:
                path.write_text(template.replace("N", str(n)))
            scripts[multiple] = path
        totals = {}
        previous = json.loads((out / "ledger.json").read_text()) if args.reuse else None
        for multiple, path in scripts.items():
            if previous:
                break
            totals[multiple], stdout = total_instructions(args.qjs, path)
            expected = str(EXPECTED[probe](args.n * multiple))
            if stdout != expected:
                sys.exit(f"{probe}: unexpected output {stdout!r}, expected {expected!r}")
        if previous:
            kept = previous["probes"][probe]
            entry = {k: v for k, v in kept.items() if k not in ("phases", "ledger_total")}
        else:
            slope = (totals[2] - totals[1]) / args.n
            slope_check = (totals[4] - totals[2]) / (2 * args.n)
            entry = {"per_iteration": slope, "per_iteration_2n_4n": slope_check}
        if args.reference and not previous:
            ref = [total_instructions(args.reference, scripts[m])[0] for m in (1, 2)]
            entry["reference_per_iteration"] = (ref[1] - ref[0]) / args.n
        contexts = {}
        for multiple in (1, 2):
            target = out / f"{probe}_{args.n * multiple}.callgrind"
            if not previous:
                run(["valgrind", "--tool=callgrind", "--separate-callers=30",
                     f"--callgrind-out-file={target}", args.qjs, str(scripts[multiple])])
            contexts[multiple] = parse_contexts(target)
        cells = collections.defaultdict(collections.Counter)
        functions = collections.defaultdict(collections.Counter)
        for name, cost in contexts[2].items():
            delta = (cost - contexts[1].get(name, 0)) / args.n
            if abs(delta) < 0.01:
                continue
            chain = [short(part) for part in name.split("'")]
            phase = phase_of(chain)
            cells[phase][mechanism_of(chain[0])] += delta
            functions[phase][chain[0]] += delta
        entry["ledger_total"] = sum(sum(c.values()) for c in cells.values())
        entry["phases"] = {p: {"total": sum(c.values()), "mechanisms": dict(c.most_common()),
                               "functions": dict(functions[p].most_common(20))}
                           for p, c in sorted(cells.items(), key=lambda kv: -sum(kv[1].values()))}
        report["probes"][probe] = entry
    (out / "ledger.json").write_text(json.dumps(report, ensure_ascii=False, indent=1))
    for probe, entry in report["probes"].items():
        print(f"## {probe}: {entry['per_iteration']:.0f} Ir/iteration "
              f"(2N→4N {entry['per_iteration_2n_4n']:.0f}; ledger {entry['ledger_total']:.0f}"
              + (f"; reference {entry['reference_per_iteration']:.0f}" if "reference_per_iteration" in entry else "")
              + ")")
        for phase, data in entry["phases"].items():
            print(f"  {data['total']:7.0f}  {phase}")
            for mechanism, value in data["mechanisms"].items():
                if value >= 10:
                    print(f"           {value:6.0f}  {mechanism}")


if __name__ == "__main__":
    main()
