#!/usr/bin/env python3
"""Self-cost share per cost class for each V8 case in a probes.py JSON report.

Usage: classify.py REPORT.json [--other]  (--other lists the largest unclassified functions)
"""
import collections, json, sys
CLASSES = [
    ("read cache", ["PropertyReadCache", "select_linked_data_into", "promote_field_in_state", "read_location"]),
    ("dense array", ["dense", "array_own_number", "fresh_array", "array_allocation", "try_replace_array",
                     "materialized_array", "ArrayEl", "array_storage"]),
    ("prop write", ["select_set_slot", "ordinary_storage::locate", "retire_field_write", "try_site_append",
                    "direct_write", "replace_property_slot", "exchange_public_owner", "property_write_driver",
                    "start_write_adapted", "dispatch_write", "SetStep", "owned_field_define", "set_slot",
                    "append_slot", "define_field"]),
    ("call/return", ["install_current_ordinary", "prepare_ordinary_window", "FrameCold", "CallStorage",
                     "DirectSelection", "CallInput", "validate_ordinary_call_operands", "FramePush",
                     "clear_current_frame", "enter_constructor", "FrameStore::materialize", "run_frames_with_state",
                     "driver::ready", "start_instance", "borrowed_ordinary_data", "FrameExecution", "frame::",
                     "prepare_ordinary_base", "strong_count"]),
    ("free/GC/alloc", ["heap::gc::", "malloc", "_int_free", "free", "memcpy", "memmove", "cfree",
                       "get_or_create_shape", "allocate", "retain_edges", "drop_in_place", "realloc",
                       "_int_malloc", "apply_cleanup", "release_", "retain_", "dup_jsvalue", "deferred"]),
    ("RegExp/builtin", ["regexp", "builtins::"]),
    ("loop/numeric", ["execute_frame_in_state", "vm::execute::", "FrameSlots", "SlotStore", "trunc",
                      "floor", "pow", "fmod", "sin", "cos", "sqrt"]),
]
def classify(fn):
    for name, keys in CLASSES:
        if any(k in fn for k in keys):
            return name
    return "other"
report = json.load(open(sys.argv[1]))["v8"]
names = [c for c, _ in CLASSES] + ["other"]
print(f"{'case':14}" + "".join(f"{n:>10}" for n in names))
others = {}
for case, d in report.items():
    share = collections.Counter(); other = collections.Counter()
    for fn, v in d["self"].items():
        k = classify(fn); share[k] += v
        if k == "other": other[fn] += v
    others[case] = other
    print(f"{case:14}" + "".join(f"{share[n]/d['Ir']*100:9.1f}%" for n in names))
if "--other" in sys.argv[2:]:
    for case, other in others.items():
        tot = report[case]["Ir"]
        print(case, [(f.split('::')[-1][:40], round(v/tot*100, 2)) for f, v in other.most_common(6)])
