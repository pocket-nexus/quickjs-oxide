"""Freeze project-authored C1 diagnostics for the existing fixed-work runner.

This generates inputs, not measurements. Run the manifest with fixed.py; use a
separate profiling build to confirm handler coverage before timing plain builds.
"""
import argparse
import json
from pathlib import Path

from run import digest


CASES = (
    "local-consume-scalar", "local-keep-scalar",
    "arg-consume-scalar", "arg-keep-scalar",
    "local-consume-heap", "arg-keep-heap",
    "mixed-boundaries", "fused-add-protection",
)
# All checksums stay exactly representable as JS Numbers within this limit.
MAX_ITERATIONS = (1 << 31) - 1
STORE_TARGETS = {
    "local-consume-scalar": (["PutLocal", "PutLocalCheck"], {"direct_store.consume": 2}),
    "local-keep-scalar": (["SetLocal", "SetLocalCheck"], {"direct_store.keep": 2}),
    "arg-consume-scalar": (["PutArg"], {"direct_store.consume": 2}),
    "arg-keep-scalar": (["SetArg"], {"direct_store.keep": 2}),
    "local-consume-heap": (["PutLocal", "PutLocalCheck"], {"direct_store.consume": 5}),
    "arg-keep-heap": (["SetArg"], {"direct_store.keep": 4}),
    "mixed-boundaries": (["PutLocal", "SetLocal", "PutLocalCheck", "SetLocalCheck"],
                         {"direct_store.consume": 1, "direct_store.keep": 1}),
    "fused-add-protection": ([], {}),
}


def periodic_sum(iterations, values):
    batches, tail = divmod(iterations, len(values))
    return batches * sum(values) + sum(values[:tail])


POOL = """function makePool() {
  return [
    ['a'.repeat(33), (1n << 100n) + 3n, Symbol('first'), {tag: 17}],
    ['b'.repeat(65), (1n << 100n) + 7n, Symbol('second'), {tag: 29}]
  ];
}
"""


def workload(case, iterations):
    """Return source plus an independently computed exact stdout contract."""
    if case not in CASES:
        raise ValueError(f"unknown direct-store case: {case}")
    if (not isinstance(iterations, int) or isinstance(iterations, bool)
            or not 1 <= iterations <= MAX_ITERATIONS):
        raise ValueError(f"iterations must be an integer in [1, {MAX_ITERATIONS}]")
    last = iterations - 1
    setup = ""
    invocation = f"kernel({iterations})"
    if case == "local-consume-scalar":
        description = "Repeated function-scoped var initializers target PutLocal consume stores; bitwise RHS bypasses AddStore."
        events = ["direct_store.consume"]
        body = """function kernel(iterations) {
  'use strict';
  let checksum = 0;
  for (let i = 0; i < iterations; i++) {
    var value = i & 255;
    var twin = value ^ 85;
    checksum += twin;
  }
  return [checksum, value, twin];
}
"""
        expected = [periodic_sum(iterations, [i ^ 85 for i in range(256)]),
                    last & 255, (last & 255) ^ 85]
    elif case == "local-keep-scalar":
        description = "Local assignment expressions keep both scalar results alive through XOR."
        events = ["direct_store.keep"]
        body = """function kernel(iterations) {
  'use strict';
  let value = 0, twin = 0, checksum = 0;
  for (let i = 0; i < iterations; i++) {
    checksum += (value = i & 255) ^ (twin = (i ^ 85) & 255);
  }
  return [checksum, value, twin];
}
"""
        expected = [85 * iterations, last & 255, (last ^ 85) & 255]
    elif case == "arg-consume-scalar":
        description = "Strict parameter var redeclaration initializers target PutArg consume stores without mapped arguments."
        events = ["direct_store.consume"]
        invocation = f"kernel(0, 0, {iterations})"
        body = """function kernel(value, twin, iterations) {
  'use strict';
  let checksum = 0;
  for (let i = 0; i < iterations; i++) {
    var value = i & 255;
    var twin = (value * 3) & 255;
    checksum += twin;
  }
  return [checksum, value, twin];
}
"""
        expected = [periodic_sum(iterations, [(i * 3) & 255 for i in range(256)]),
                    last & 255, (last * 3) & 255]
    elif case == "arg-keep-scalar":
        description = "Strict parameter assignment expressions consumed by addition, exercising keep stores."
        events = ["direct_store.keep"]
        invocation = f"kernel(0, 0, {iterations})"
        body = """function kernel(value, twin, iterations) {
  'use strict';
  let checksum = 0;
  for (let i = 0; i < iterations; i++) {
    checksum += (value = i & 255) + (twin = (i * 3) & 255);
  }
  return [checksum, value, twin];
}
"""
        expected = [periodic_sum(iterations, [i + ((i * 3) & 255) for i in range(256)]),
                    last & 255, (last * 3) & 255]
    elif case == "local-consume-heap":
        description = "Repeated var initializers target local consume stores for String, heap BigInt, Symbol and Object owners plus an alias."
        events = ["direct_store.consume"]
        setup = POOL
        body = """function kernel(iterations) {
  'use strict';
  const pool = makePool();
  let checksum = 0;
  for (let i = 0; i < iterations; i++) {
    const row = pool[i & 1];
    var text = row[0];
    var integer = row[1];
    var token = row[2];
    var object = row[3];
    var alias = object;
    checksum += text.length + alias.tag;
  }
  const final = pool[(iterations - 1) & 1];
  return [checksum, text.length, integer === final[1], token === final[2], alias === final[3]];
}
"""
        expected = [periodic_sum(iterations, [50, 94]), 65 if last & 1 else 33,
                    True, True, True]
    elif case == "arg-keep-heap":
        description = "Strict parameter keep stores retain String, heap BigInt, Symbol and Object results for consumers."
        events = ["direct_store.keep"]
        setup = POOL
        invocation = f"kernel('', 0n, null, null, makePool(), {iterations})"
        body = """function kernel(text, integer, token, object, pool, iterations) {
  'use strict';
  let checksum = 0;
  for (let i = 0; i < iterations; i++) {
    checksum += (text = pool[i & 1][0]).length;
    if ((integer = pool[i & 1][1]) !== pool[i & 1][1]) throw Error('BigInt result lost');
    if ((token = pool[i & 1][2]) !== pool[i & 1][2]) throw Error('Symbol result lost');
    checksum += (object = pool[i & 1][3]).tag;
  }
  const final = pool[(iterations - 1) & 1];
  return [checksum, text.length, integer === final[1], token === final[2], object === final[3]];
}
"""
        expected = [periodic_sum(iterations, [50, 94]), 65 if last & 1 else 33,
                    True, True, True]
    elif case == "mixed-boundaries":
        description = "Direct stores across getter/call/throw boundaries with captured and mapped-argument fallback protection."
        events = ["direct_store.consume", "direct_store.keep"]
        body = """function kernel(iterations) {
  'use strict';
  let calls = 0, captured = 0;
  const receiver = {get value() { calls++; return 7; }};
  function relay(value) { return value; }
  function readCaptured() { return captured; }
  const mapped = Function('value',
    "value = value ^ 3; if (arguments[0] !== value) throw Error('mapped alias'); return arguments[0];");
  let value = 0, checksum = 0;
  for (let i = 0; i < iterations; i++) {
    value = receiver.value;
    checksum += (value = relay(value));
    captured = i & 7;
    checksum += readCaptured();
    checksum += mapped(i & 3);
    try {
      if ((i & 7) === 0) throw value;
    } catch (caught) {
      checksum += caught;
    }
  }
  return [checksum, calls, value, captured];
}
"""
        expected = [7 * iterations + periodic_sum(iterations, list(range(8)))
                    + periodic_sum(iterations, [3, 2, 1, 0]) + 7 * ((iterations + 7) // 8),
                    iterations, 7, last & 7]
    else:
        description = "Protection case: representative existing local AddStore and loop-update fusion remain enabled."
        events = []
        body = """function kernel(iterations) {
  'use strict';
  let value = 0, step = 3, checksum = 0;
  for (let i = 0; i < iterations; i++) {
    value += step;
    checksum += value & 7;
  }
  return [checksum, value];
}
"""
        expected = [periodic_sum(iterations, [(3 * i) & 7 for i in range(1, 9)]),
                    3 * iterations]
    source = (f"// C1 diagnostic: {case}; fixed iterations={iterations}.\n"
              + setup + body + f"console.log(JSON.stringify({invocation}));\n")
    return source, {
        "case": case, "size": iterations, "operations": iterations,
        "expected": json.dumps(expected, separators=(",", ":")) + "\n",
        "description": description, "expected_profiling_events": events,
        "target_store_opcodes": STORE_TARGETS[case][0],
        "expected_minimum_profiling_events": {
            event: count * iterations for event, count in STORE_TARGETS[case][1].items()
        },
    }


def prepare(output, iterations, cases=None):
    selected = list(CASES if cases is None else cases)
    if not selected or len(selected) != len(set(selected)):
        raise ValueError("cases must be nonempty and unique")
    # Validate all inputs before creating any output. Existing captures are immutable.
    generated = [workload(case, iterations) for case in selected]
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    directory = output / "workloads"
    directory.mkdir()
    rows = []
    for source, item in generated:
        path = directory / f"{item['case']}.js"
        path.write_text(source, encoding="utf-8")
        rows.append(dict(item, path=str(path), sha256=digest(path)))
    manifest = {
        "metadata": {
            "schema": "oxide-direct-store-workloads-v1",
            "generator_sha256": digest(__file__),
            "iterations": iterations,
            "metric": "fixed whole-process diagnostics; no timing performed by this generator",
            "coverage": "Confirm expected minimum event counts and local/argument opcode coverage with a separate profiling build before timing. Counts exclude setup stores; source shape is not coverage proof.",
            "expected_method": "Independent bounded-period integer sums; JSON exact stdout, not an engine-derived oracle.",
            "workloads": {"workloads": rows},
        },
    }
    path = output / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True,
                        help="new directory for immutable manifest and JavaScript inputs")
    parser.add_argument("--iterations", type=int, required=True)
    parser.add_argument("--case", action="append", choices=CASES)
    args = parser.parse_args()
    try:
        print(prepare(args.output, args.iterations, args.case))
    except (ValueError, OSError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
