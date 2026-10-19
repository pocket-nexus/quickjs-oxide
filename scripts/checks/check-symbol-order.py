#!/usr/bin/env python3
"""Fails when `apps/cli/qjs.order` names too few functions the release `qjs` defines.

The linker ignores the names a binary does not define, so a file gone stale
orders less code without any error; this check notices it.
"""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
REGENERATE = "cargo build --release -p quickjs-oxide-cli && python3 scripts/release/gen-symbol-order.py"


def defined_functions(binary):
    output = subprocess.run(["nm", "--defined-only", str(binary)], capture_output=True, text=True,
                            check=True).stdout
    return {fields[2] for fields in map(str.split, output.splitlines())
            if len(fields) == 3 and fields[1] in "tTwW"}


def coverage(names, defined):
    """The fraction of `names` that `defined` holds."""
    return sum(name in defined for name in names) / len(names)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qjs", type=Path, default=ROOT / "target/release/qjs")
    parser.add_argument("--order", type=Path, default=ROOT / "apps/cli/qjs.order")
    parser.add_argument("--minimum", type=float, default=0.8)
    args = parser.parse_args()
    names = args.order.read_text().split()
    if not names:
        sys.exit(f"{args.order} is empty; regenerate it with: {REGENERATE}")
    if len(set(names)) != len(names):
        sys.exit(f"{args.order} names a function twice; regenerate it with: {REGENERATE}")
    share = coverage(names, defined_functions(args.qjs))
    if share < args.minimum:
        sys.exit(f"{args.order}: {share:.0%} of its {len(names)} functions are defined by {args.qjs} "
                 f"(at least {args.minimum:.0%} needed); regenerate it with: {REGENERATE}")
    print(f"{args.order}: {share:.0%} of {len(names)} functions defined")


if __name__ == "__main__":
    main()
