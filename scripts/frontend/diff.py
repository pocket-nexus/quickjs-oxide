#!/usr/bin/env python3
"""Runs JavaScript files through both front ends and compares the results.

Each file runs as `qjs --frontend=old FILE` and `qjs --frontend=new FILE`;
stdout, stderr and the exit code must match byte for byte. A file whose
first line is `// qjs: ARGS` passes ARGS before the file.

    python3 scripts/frontend/diff.py [--qjs PATH] [--jobs N] [--show N] PATH...

PATH is a file or a directory searched for `*.js`. Exits 1 when any file
differs, printing a unified diff of the first few.
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


def extra_args(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            first = handle.readline()
    except OSError:
        return []
    if first.startswith("// qjs:"):
        return shlex.split(first[len("// qjs:"):])
    if path.endswith(".mjs"):
        return ["-m"]
    return []


def run(qjs, front_end, path, timeout):
    command = [qjs, f"--frontend={front_end}", *extra_args(path), os.path.basename(path)]
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


def compare(qjs, path, timeout):
    old = run(qjs, "old", path, timeout)
    new = run(qjs, "new", path, timeout)
    return path, old, new


def render(old, new):
    lines = []
    for label, a, b in (("stdout", old[1], new[1]), ("stderr", old[2], new[2])):
        if a != b:
            lines.extend(
                difflib.unified_diff(
                    a.decode("utf-8", "replace").splitlines(),
                    b.decode("utf-8", "replace").splitlines(),
                    f"old {label}",
                    f"new {label}",
                    lineterm="",
                    n=2,
                )
            )
    if old[0] != new[0]:
        lines.append(f"exit: old {old[0]}, new {new[0]}")
    return "\n".join(lines[:80])


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("paths", nargs="+")
    parser.add_argument("--qjs", default="target/release/qjs")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    parser.add_argument("--show", type=int, default=5, help="diffs to print")
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--list", action="store_true", help="print every differing file")
    args = parser.parse_args()
    qjs = os.path.abspath(args.qjs)
    paths = list(files(args.paths))
    differing = []
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        for path, old, new in pool.map(lambda p: compare(qjs, p, args.timeout), paths):
            if old != new:
                differing.append((path, old, new))
    for path, old, new in differing[: args.show]:
        print(f"=== {path}")
        print(render(old, new))
    if args.list:
        for path, _, _ in differing:
            print(path)
    print(f"{len(paths) - len(differing)}/{len(paths)} agree")
    return 1 if differing else 0


if __name__ == "__main__":
    sys.exit(main())
