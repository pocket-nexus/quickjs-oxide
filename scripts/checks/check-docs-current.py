#!/usr/bin/env python3
"""The documentation describes the engine that exists.

Every Markdown page but the changelog is checked for:

- names of modules, types, binaries, features, options and pages the
  tree does not have, and phase numbering;
- relative links whose target file does not exist.

    python3 scripts/checks/check-docs-current.py
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Release notes name what each release changed.
HISTORY = [
    re.compile(r"^CHANGELOG\.md$"),
]

STALE = [
    ("ExecCode", r"\bExecCode\b"),
    ("RuntimeState", r"\bRuntimeState\b"),
    ("FrameExecution", r"\bFrameExecution\b"),
    ("property_ic", r"\bproperty_ic\b"),
    ("oxide-run", r"\boxide-run\b"),
    ("api::new_core", r"\bnew_core\b"),
    ("the old core's modules", r"\bsrc/engine/(vm|heap|object|builtins|realm|atom|host|jobs|modules|profiling)\b"),
    ("the old regexp module", r"\bsrc/regexp\.rs\b"),
    ("the stack VM configuration", r"\bstack-vm\b"),
    ("a retired feature", r"\b(checked-handles|dropped-features-probe)\b|--features[= ]profiling\b"),
    ("a retired qjs option", r"(?<![\w-])--(dump|trace|profile-[a-z-]+)\b"),
    ("a retired page", r"\b(roadmap|typed-arenas|cycle-collection|baseline)\.md\b|new-core-phase"),
    ("a retired name", r"\bnew-core\b|\bNEWCORE-"),
    ("phase numbering", r"\b[Pp]hase [0-9]|\bM[0-9]b?\b"),
]

LINK = re.compile(r"\]\(([^)\s]+)\)")


def pages():
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*.md"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.split()
    return [page for page in listed if not any(rule.match(page) for rule in HISTORY)]


def main() -> int:
    failures = []
    for page in pages():
        path = ROOT / page
        if not path.exists():
            continue
        text = path.read_text()
        for number, line in enumerate(text.splitlines(), 1):
            for name, pattern in STALE:
                if re.search(pattern, line):
                    failures.append(f"{page}:{number}: names {name}: {line.strip()[:100]}")
            for target in LINK.findall(line):
                if re.match(r"^[a-z]+:", target) or target.startswith("#"):
                    continue
                file = target.split("#", 1)[0]
                if file and not (path.parent / file).exists():
                    failures.append(f"{page}:{number}: broken link {target}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        print(f"documentation check: {len(failures)} problems", file=sys.stderr)
        return 1
    print(f"documentation check passed: {len(pages())} current pages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
