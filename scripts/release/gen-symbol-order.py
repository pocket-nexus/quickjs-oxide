#!/usr/bin/env python3
"""Writes `apps/cli/qjs.order`, the link order of `qjs`'s startup and loop code.

Runs the release `qjs` under callgrind over an empty script, Octane's
Richards and the compile of one mid-sized source, collects every mangled
function name in the order the profiles first name it (`fn=` and `cfn=`
lines), keeps the names the binary defines, and writes them one per line.
`apps/cli/build.rs` passes the file to the linker, which places those
functions first and together in `.text`, so the file pages they touch at
startup are few.

    cargo build --release -p quickjs-oxide-cli
    python3 scripts/release/gen-symbol-order.py
"""
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_BUNDLES = Path.home() / ".cache/engine-compare-20261015/bundles"
LINE = re.compile(r"^(c?fn|c?ob)=(?:\((\d+)\))?\s*(.*)$")


def defined_functions(binary):
    """The function symbols the binary defines."""
    output = subprocess.run(["nm", "--defined-only", str(binary)], capture_output=True, text=True,
                            check=True).stdout
    names = set()
    for line in output.splitlines():
        fields = line.split()
        if len(fields) == 3 and fields[1] in "tTwW":
            names.add(fields[2])
    return names


def profile_names(text, binary):
    """The names of the functions of `binary` in first-seen order. Callgrind
    writes a name once with its id, `fn=(12) name`, and the id alone
    afterwards; `ob=` names the object of the following `fn=` lines and
    `cob=` the object of the next `cfn=`."""
    names = {"fn": {}, "ob": {}}
    current = callee = None
    for line in text.splitlines():
        match = LINE.match(line)
        if not match:
            continue
        key, ident, name = match[1], match[2], match[3].strip()
        table = names[key.removeprefix("c")]
        if ident is not None:
            if name:
                table[ident] = name
            name = table.get(ident, "")
        if key == "ob":
            current = callee = name
        elif key == "cob":
            callee = name
        else:
            owner = current if key == "fn" else callee
            callee = current
            if name and owner == binary:
                yield name


def run_callgrind(valgrind, command, output):
    subprocess.run([valgrind, "--tool=callgrind", "--demangle=no", f"--callgrind-out-file={output}", *command],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    return output.read_text(errors="replace")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--qjs", type=Path, default=ROOT / "target/release/qjs")
    parser.add_argument("--bundles", type=Path, default=DEFAULT_BUNDLES,
                        help="the fixed-work bundles (octane/richards.js, octane/box2d.js)")
    parser.add_argument("--valgrind", default="valgrind")
    parser.add_argument("--output", type=Path, default=ROOT / "apps/cli/qjs.order")
    parser.add_argument("--check-inputs", action="store_true",
                        help="only check that valgrind and the bundles are present")
    args = parser.parse_args()
    richards = args.bundles / "octane/richards.js"
    compiled = args.bundles / "octane/box2d.js"
    if shutil.which(args.valgrind) is None:
        parser.error(f"missing: {args.valgrind}")
    for path in ([] if args.check_inputs else [args.qjs]) + [richards, compiled]:
        if not path.exists():
            parser.error(f"missing: {path}")
    if args.check_inputs:
        return
    defined = defined_functions(args.qjs)
    order, seen = [], set()
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        empty = scratch / "empty.js"
        empty.write_text("")
        workloads = [[args.qjs, empty], [args.qjs, richards], [args.qjs, "--disassemble", compiled]]
        for index, command in enumerate(workloads):
            text = run_callgrind(args.valgrind, list(map(str, command)), scratch / f"callgrind.{index}")
            for name in profile_names(text, str(args.qjs.resolve())):
                if name in defined and name not in seen:
                    seen.add(name)
                    order.append(name)
    args.output.write_text("".join(f"{name}\n" for name in order))
    print(f"{args.output}: {len(order)} functions", file=sys.stderr)


if __name__ == "__main__":
    main()
