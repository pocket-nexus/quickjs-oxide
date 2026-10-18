#!/usr/bin/env python3
"""Three-way differential of the JavaScript oracle cases (phase 6 M4b, section 2.2).

Every case under `apps/cli/tests/oracle/cases` runs on `qjs` (built with
`test262-host`), the pinned QuickJS and Node (V8, a third implementation).
What the specification leaves to the implementation is normalized away:
error messages and `stack` (a thrown object compares by its constructor's
name), the printer (`print` writes `String` of each argument), and functions'
source text (a function compares as `function`). A case where QuickJS and
Node disagree is a candidate departure; each candidate is decided by the
specification clause, never by majority.

    python3 scripts/checks/spec-triage.py --qjs target/release/qjs \\
        --quickjs target/oracle/quickjs-2026-06-04/qjs --node node \\
        --output target/spec-triage.tsv

Completion cases run each `// ---` script as a global script in one realm
(`$262.evalScript`, `std.evalScript`, `vm.runInThisContext`); print cases run
prelude and case as one global script. Cases with `// flags:` options other
than none are skipped: their options are QuickJS's, which Node has not.
"""
import argparse
import concurrent.futures
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "apps/cli/tests/oracle/cases"
PRELUDES = ROOT / "apps/cli/tests/oracle/preludes"
ATTRIBUTIONS = ROOT / "dev-support/oracle-attributions.tsv"
SEPARATOR = "\n// ---\n"

# The observer each engine runs: `__evalScript` evaluates one global script,
# `__out` writes one line. It runs the scripts in order and prints one
# normalized completion per script, then lets the engine drain its jobs.
OBSERVER = r"""
var __describe = function (value) {
  var type = typeof value;
  if (type === 'function') return 'function';
  if (type === 'symbol') return 'symbol|' + String(value.description);
  if (type === 'bigint') return 'bigint|' + String(value) + 'n';
  if (type === 'number' && value === 0 && 1 / value < 0) return 'number|-0';
  try { return type + '|' + String(value); }
  catch (error) { return type + '|<String threw ' + __thrown(error) + '>'; }
};
var __thrown = function (error) {
  if (error !== null && (typeof error === 'object' || typeof error === 'function')) {
    var constructor;
    try { constructor = error.constructor; } catch (e) { constructor = undefined; }
    var name;
    try { name = constructor && constructor.name; } catch (e) { name = undefined; }
    return 'object|' + (typeof name === 'string' ? name : '?');
  }
  return __describe(error);
};
var __print = function () {
  var parts = [];
  for (var index = 0; index < arguments.length; index++) {
    var value = arguments[index];
    parts.push(typeof value === 'string' ? value : __describe(value).replace(/^[a-z]+\|/, ''));
  }
  __out(parts.join(' '));
};
globalThis.print = __print;
if (typeof console === 'object') console.log = __print;
var __mode = __MODE__;
var __scripts = __SCRIPTS__;
for (var __index = 0; __index < __scripts.length; __index++) {
  try {
    var __value = __evalScript(__scripts[__index]);
    if (__mode === 'completion') __out('return|' + __describe(__value));
  } catch (error) {
    __out('throw|' + __thrown(error));
  }
}
"""

ENGINES = {
    "qjs": ("var __evalScript = $262.evalScript; var __out = print;\n", ["--test262"]),
    "quickjs": ("var __evalScript = std.evalScript; var __out = print;\n", ["--std"]),
    "node": (
        "var __evalScript = function (source) { return require('vm').runInThisContext(source); };\n"
        "var __out = function (text) { process.stdout.write(text + '\\n'); };\n",
        [],
    ),
}


def parse_fixture(label, text):
    """The cases of one fixture, as `support/js_cases.rs` reads them."""
    lines = text.splitlines(keepends=True)
    mode = "completion"
    if lines and lines[0].startswith("// mode:"):
        mode = "print" if lines[0].strip() == "// mode: print" else "completion"
        lines = lines[1:]
    variants = []
    while lines and lines[0].startswith("// flags:"):
        variants.append(lines[0].strip()[len("// flags:"):].split())
        lines = lines[1:]
    if not variants:
        variants = [[]]
    prelude, cases = "", []
    for line in lines:
        if line.rstrip("\n").startswith("// case: "):
            cases.append([line.rstrip("\n")[len("// case: "):], ""])
        elif cases:
            cases[-1][1] += line
        elif line.rstrip("\n").startswith("// prelude: "):
            name = line.rstrip("\n")[len("// prelude: "):]
            prelude += (PRELUDES / f"{name}.js").read_text().rstrip("\n") + "\n"
        else:
            prelude += line
    if not cases:
        cases = [["", prelude]]
        prelude = ""
    prelude = trim_trailing_comments(prelude)
    parsed = []
    for name, source in cases:
        source = trim_trailing_comments(source)
        if prelude:
            source = f"{prelude}\n{source}"
        full = f"{label} / {name}" if name else label
        for flags in variants:
            parsed.append({
                "name": full if not flags else f"{full} [{' '.join(flags)}]",
                "mode": mode,
                "source": source,
                "flags": flags,
            })
    return parsed


def trim_trailing_comments(source):
    lines = source.split("\n")
    while lines and (not lines[-1].strip() or lines[-1].lstrip().startswith("//")):
        lines.pop()
    return "\n".join(lines)


def all_cases():
    cases = []
    for path in sorted(CASES.rglob("*.js")):
        label = str(path.relative_to(CASES).with_suffix(""))
        cases.extend(parse_fixture(label, path.read_text()))
    return cases


def observe(engine, executable, case, directory, timeout):
    prefix, options = ENGINES[engine]
    scripts = case["source"].split(SEPARATOR) if case["mode"] == "completion" else [case["source"]]
    driver = prefix + OBSERVER.replace("__MODE__", json.dumps(case["mode"])).replace(
        "__SCRIPTS__", json.dumps(scripts))
    with tempfile.NamedTemporaryFile("w", suffix=".js", dir=directory, delete=False) as file:
        file.write(driver)
    try:
        result = subprocess.run([executable, *options, file.name], capture_output=True,
                                timeout=timeout)
        output = result.stdout.decode("utf-8", "replace").rstrip("\n")
        if result.returncode != 0:
            output += f"\n<exit {result.returncode}>"
        return output
    except subprocess.TimeoutExpired:
        return "<timeout>"
    finally:
        os.unlink(file.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--qjs", required=True, help="qjs built with test262-host")
    parser.add_argument("--quickjs", required=True, help="the pinned QuickJS qjs")
    parser.add_argument("--node", default="node")
    parser.add_argument("--output", required=True, help="TSV of every case")
    parser.add_argument("--only", help="substring of the case names to run")
    parser.add_argument("--jobs", type=int, default=os.cpu_count())
    parser.add_argument("--timeout", type=float, default=30)
    args = parser.parse_args()

    attributed = {}
    for line in ATTRIBUTIONS.read_text().splitlines()[1:]:
        case, deviation = line.split("\t")
        attributed[case] = deviation
    executables = {"qjs": args.qjs, "quickjs": args.quickjs, "node": args.node}
    cases = [case for case in all_cases() if not args.only or args.only in case["name"]]
    runnable = [case for case in cases if not case["flags"]]
    with tempfile.TemporaryDirectory() as directory, \
            concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        futures = {
            (index, engine): pool.submit(observe, engine, executables[engine], case,
                                         directory, args.timeout)
            for index, case in enumerate(runnable) for engine in executables
        }
        rows = []
        for index, case in enumerate(runnable):
            observed = {engine: futures[(index, engine)].result() for engine in executables}
            if observed["quickjs"] != observed["node"]:
                verdict = "candidate"
            elif observed["qjs"] != observed["quickjs"]:
                verdict = "qjs-differs"
            else:
                verdict = "agree"
            rows.append((case["name"], case["mode"], verdict,
                         attributed.get(case["name"], "-"), observed))

    with open(args.output, "w") as output:
        output.write("case\tmode\tverdict\tattribution\tqjs\tquickjs\tnode\n")
        for name, mode, verdict, attribution, observed in rows:
            cells = [json.dumps(observed[engine]) for engine in ("qjs", "quickjs", "node")]
            output.write("\t".join([name, mode, verdict, attribution, *cells]) + "\n")
    counts = {}
    for row in rows:
        counts[row[2]] = counts.get(row[2], 0) + 1
    skipped = len(cases) - len(runnable)
    print(f"spec triage: {len(rows)} cases ({skipped} with qjs flags skipped): "
          + ", ".join(f"{verdict} {count}" for verdict, count in sorted(counts.items())))


if __name__ == "__main__":
    sys.exit(main())
