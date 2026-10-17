#!/usr/bin/env python3
"""Count the workspace's non-test Rust lines (new-core phase 6, section 0).

Non-test lines are the lines of every `.rs` file that is not under a `tests/`
directory and is not named `tests.rs` or `*_tests.rs`. Files under `target/`
and other build output are never counted. The table groups the count by
crate and, inside the root crate, by `src/engine` module:

    python3 scripts/checks/count-lines.py            # Markdown table
    python3 scripts/checks/count-lines.py --json     # machine-readable
    python3 scripts/checks/count-lines.py --rev c2d8345b   # a commit's tree

`--rev` counts the tree of a commit without checking it out (`git ls-tree`
and `git cat-file`), so a baseline can be re-recorded from any revision.
"""
import argparse
import json
import subprocess
import sys
from collections import OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKIPPED_TOP = {"target", "node_modules", ".git"}


def is_counted(path: str) -> bool:
    parts = path.split("/")
    if not path.endswith(".rs"):
        return False
    if parts[0] in SKIPPED_TOP or any(part.startswith(".") for part in parts):
        return False
    if "tests" in parts[:-1]:
        return False
    name = parts[-1]
    return name != "tests.rs" and not name.endswith("_tests.rs")


def group_of(path: str) -> str:
    parts = path.split("/")
    if parts[0] == "src":
        if len(parts) > 3 and parts[1] == "engine":
            return f"src/engine/{parts[2]}"
        if len(parts) == 3 and parts[1] == "engine":
            return "src/engine/*.rs"
        return "src/ (other)"
    if parts[0] in ("apps", "adapters", "conformance") and len(parts) > 1:
        return f"{parts[0]}/{parts[1]}"
    return parts[0]


def files_in_worktree() -> list[tuple[str, int]]:
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*.rs"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.split("\n")
    counted = []
    for path in listed:
        if path and is_counted(path) and (ROOT / path).is_file():
            with open(ROOT / path, "rb") as file:
                counted.append((path, file.read().count(b"\n")))
    return counted


def files_at_revision(rev: str) -> list[tuple[str, int]]:
    tree = subprocess.run(
        ["git", "ls-tree", "-r", "--format=%(objectname) %(path)", rev],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.split("\n")
    entries = [line.split(" ", 1) for line in tree if line]
    entries = [(obj, path) for obj, path in entries if is_counted(path)]
    batch = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=ROOT, check=True, capture_output=True,
        input="".join(f"{obj}\n" for obj, _ in entries).encode(),
    ).stdout
    counted, offset = [], 0
    for _, path in entries:
        header_end = batch.index(b"\n", offset)
        size = int(batch[offset:header_end].split()[2])
        body = batch[header_end + 1:header_end + 1 + size]
        counted.append((path, body.count(b"\n")))
        offset = header_end + 1 + size + 1
    return counted


def tally(files: list[tuple[str, int]]) -> "OrderedDict[str, int]":
    groups: dict[str, int] = {}
    for path, lines in files:
        group = group_of(path)
        groups[group] = groups.get(group, 0) + lines
    root = sum(lines for group, lines in groups.items() if group.startswith("src"))
    table: "OrderedDict[str, int]" = OrderedDict()
    table["root crate src/"] = root
    for group in sorted((g for g in groups if g.startswith("src")), key=lambda g: -groups[g]):
        table[group] = groups[group]
    for group in sorted((g for g in groups if not g.startswith("src")), key=lambda g: -groups[g]):
        table[group] = groups[group]
    table["workspace"] = sum(groups.values())
    return table


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--rev", help="count this commit's tree instead of the worktree")
    parser.add_argument("--json", action="store_true", help="print JSON")
    args = parser.parse_args()
    files = files_at_revision(args.rev) if args.rev else files_in_worktree()
    table = tally(files)
    if args.json:
        json.dump(table, sys.stdout, indent=2)
        print()
        return 0
    print("| Crate or module | Non-test lines |")
    print("| --- | ---: |")
    for group, lines in table.items():
        label = f"`{group}`" if group not in ("workspace", "root crate src/") else group
        if group == "workspace":
            print(f"| **workspace** | **{lines:,}** |")
        else:
            print(f"| {label} | {lines:,} |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
