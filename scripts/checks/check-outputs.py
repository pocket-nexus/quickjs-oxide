#!/usr/bin/env python3
"""Runs JavaScript files and compares each with its recorded output.

Each `NAME.js` that has a `NAME.out` beside it runs as `qjs NAME.js` from its
own directory; stdout must equal `NAME.out` byte for byte, stderr must be
empty and the exit status 0. A first line `// qjs: ARGS` passes ARGS before
the file; a `.mjs` file runs with `-m`. `--update` rewrites every `.out`
from the current output instead of comparing.

    python3 scripts/checks/check-outputs.py [--qjs PATH] [--jobs N] [--update] PATH...

PATH is a file or a directory searched for `*.js` and `*.mjs`. Exits 1 when
any file differs, printing a unified diff of the first few.
"""

import argparse
import concurrent.futures
import difflib
import os
import shlex
import subprocess
import sys


def files(paths):
    for path in paths:
        if os.path.isdir(path):
            for root, _, names in os.walk(path):
                for name in sorted(names):
                    if name.endswith(".js") or name.endswith(".mjs"):
                        yield os.path.join(root, name)
        else:
            yield path


def expected_path(path):
    return os.path.splitext(path)[0] + ".out"


def extra_args(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        first = handle.readline()
    if first.startswith("// qjs:"):
        return shlex.split(first[len("// qjs:"):])
    if path.endswith(".mjs"):
        return ["-m"]
    return []


def run(qjs, path, timeout):
    command = [qjs, *extra_args(path), os.path.basename(path)]
    try:
        done = subprocess.run(
            command,
            capture_output=True,
            timeout=timeout,
            cwd=os.path.dirname(os.path.abspath(path)),
        )
        return done.returncode, done.stdout, done.stderr
    except subprocess.TimeoutExpired:
        return "timeout", b"", b""


def check(qjs, path, timeout, update):
    status, stdout, stderr = run(qjs, path, timeout)
    if update:
        if status == 0 and not stderr:
            with open(expected_path(path), "wb") as handle:
                handle.write(stdout)
            return None
        return f"{path}: exit {status}, stderr {stderr.decode(errors='replace')!r}"
    with open(expected_path(path), "rb") as handle:
        expected = handle.read()
    if status == 0 and not stderr and stdout == expected:
        return None
    lines = [f"==== {path}: exit {status}"]
    if stderr:
        lines.append("-- stderr")
        lines.append(stderr.decode(errors="replace").rstrip("\n"))
    lines.extend(
        difflib.unified_diff(
            expected.decode(errors="replace").splitlines(),
            stdout.decode(errors="replace").splitlines(),
            "expected",
            "actual",
            lineterm="",
        )
    )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("paths", nargs="+")
    parser.add_argument("--qjs", default="target/release/qjs")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--timeout", type=float, default=60.0)
    parser.add_argument("--show", type=int, default=5)
    parser.add_argument("--update", action="store_true")
    args = parser.parse_args()
    qjs = os.path.abspath(args.qjs)
    targets = [path for path in files(args.paths)
               if args.update or os.path.exists(expected_path(path))]
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(lambda path: check(qjs, path, args.timeout, args.update), targets))
    failures = [result for result in results if result is not None]
    for failure in failures[: args.show]:
        print(failure)
    print(f"{len(targets) - len(failures)}/{len(targets)} match")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
