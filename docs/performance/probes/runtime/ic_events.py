#!/usr/bin/env python3
"""Run the eight V8 bodies under a profiling qjs and report property read cache events.

Usage: ic_events.py PROFILING_QJS V8_V7_DIR OUTPUT_DIR
Build the CLI with `--features profiling`; the iteration counts match the Callgrind profile.
"""
import json, subprocess, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "scripts/benchmark"))
import profile_v8
qjs, code, out = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
ITER = {"richards": 10, "deltablue": 10, "navier-stokes": 10, "crypto": 3, "raytrace": 3,
        "earley-boyer": 2, "regexp": 1, "splay": 2}
report = {}
for case, iterations in ITER.items():
    body = (code / f"{case}.js").read_text()
    suite, benchmarks = profile_v8.declared_benchmarks(body, case)
    script = out / f"{case}.js"
    script.write_text((code / "base.js").read_text() + "\n" + body + "\n" +
                      profile_v8.fixed_driver(suite, benchmarks, iterations).decode())
    prof = out / f"{case}.jsonl"; prof.unlink(missing_ok=True)
    subprocess.run([qjs, "-d", "--profile-json", "--profile-output", str(prof), str(script)],
                   check=True, capture_output=True)
    events = {}
    def walk(node):
        if isinstance(node, dict):
            for k, v in node.items():
                if isinstance(k, str) and k.startswith("property_ic.") and isinstance(v, int):
                    events[k] = events.get(k, 0) + v
                else:
                    walk(v)
        elif isinstance(node, list):
            for v in node: walk(v)
    for line in prof.read_text().splitlines():
        walk(json.loads(line))
    report[case] = events
(out / "ic.json").write_text(json.dumps(report, indent=1))
keys = ["hit.monomorphic", "hit.monomorphic_prototype", "hit.polymorphic_first", "hit.polymorphic_later",
        "hit.polymorphic_first_prototype", "hit.polymorphic_later_prototype",
        "guard_miss.monomorphic", "guard_miss.polymorphic", "megamorphic_skip", "accessor_site", "cold", "miss"]
print(f"{'case':14}{'reads':>12} " + " ".join(f"{k.split('.')[-1][:10]:>10}" for k in keys))
for case, ev in report.items():
    total = sum(ev.get("property_ic." + k, 0) for k in keys[:-1])
    print(f"{case:14}{total:12,} " + " ".join(f"{ev.get('property_ic.'+k,0)/max(total,1)*100:9.1f}%" for k in keys))
