#!/usr/bin/env python3
"""Compares two Test262 reports variant by variant.

    python3 scripts/frontend/compare-test262.py OLD.tsv NEW.tsv [--list N]

Prints the variants that pass in OLD and not in NEW (regressions) and the
reverse, grouped by NEW's failure detail. Exits 1 when NEW regresses.
"""

import argparse
import collections
import re
import sys


def load(path):
    outcomes = {}
    with open(path, encoding="utf-8") as handle:
        header = None
        for line in handle:
            if line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if header is None:
                header = fields
                continue
            row = dict(zip(header, fields))
            outcomes[(row["path"], row["variant"])] = row
    return outcomes


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("old")
    parser.add_argument("new")
    parser.add_argument("--list", type=int, default=0, help="variants to list per group")
    args = parser.parse_args()
    old = load(args.old)
    new = load(args.new)
    regressions = [key for key, row in new.items() if row["outcome"] != "pass" and old.get(key, {}).get("outcome") == "pass"]
    fixes = [key for key, row in new.items() if row["outcome"] == "pass" and old.get(key, {}).get("outcome") not in (None, "pass")]
    groups = collections.defaultdict(list)
    for key in regressions:
        row = new[key]
        detail = re.sub(r"\d+", "N", row["detail"][:100])
        groups[(row["outcome"], detail)].append(key)
    for (outcome, detail), keys in sorted(groups.items(), key=lambda item: -len(item[1])):
        print(f"{len(keys):6} {outcome}: {detail}")
        for path, variant in keys[: args.list]:
            print(f"         {path} [{variant}]")
    old_passes = sum(1 for row in old.values() if row["outcome"] == "pass")
    new_passes = sum(1 for row in new.values() if row["outcome"] == "pass")
    print(f"old {old_passes} pass, new {new_passes} pass; {len(regressions)} regressions, {len(fixes)} new passes")
    return 1 if regressions else 0


if __name__ == "__main__":
    sys.exit(main())
