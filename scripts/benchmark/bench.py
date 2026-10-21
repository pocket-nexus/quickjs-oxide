#!/usr/bin/env python3
"""quickjs-oxide benchmark tools: one entry point, one command per question.

    python3 scripts/benchmark/bench.py <command> [options]
    python3 scripts/benchmark/bench.py <command> --help

Which command answers which question is in docs/performance/benchmarking.md.
"""
import importlib
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

# (command, module, summary), grouped as the help prints them.
COMMANDS = {
    "Binaries and workloads": [
        ("build", "build", "release qjs from a clean worktree, with a provenance receipt"),
        ("suite", "suite", "the 37 V8 v7/Octane/AWFY programs (build) and their table (table)"),
        ("fixed-matrix", "fixed_matrix", "regenerate or --check the 20 fixed VM diagnostics"),
    ],
    "Measure": [
        ("sweep", "sweep", "the default check: all fixed probes and V8 v7, base vs candidate, with the gates"),
        ("run", "runner", "run a workload manifest on engines: time, perf counters or Callgrind"),
        ("paired", "paired", "accept or reject an A/B result against its A/A noise"),
        ("memory", "memory", "peak RSS against QuickJS over the suite programs"),
        ("scaling", "scaling", "data-structure scaling workloads across sizes"),
        ("compile corpus", "compile_corpus", "generate the front-end compile corpus"),
        ("compile probe", "compile_probe", "build one compile probe against a checkout"),
        ("compile probes", "compile_probes", "build the Oxide and QuickJS compile probes"),
        ("compile matrix", "compile_matrix", "compile-only time across probes"),
    ],
    "Explain": [
        ("probes", "probes", "Callgrind per-iteration Ir and per-function self cost"),
        ("slopes instructions", "slopes_instructions", "perf-stat instruction slopes of probes"),
        ("slopes heap", "slopes_heap", "instruction slopes of the Rust heap workloads"),
        ("relink", "relink", "relink one LTO build at several .text offsets (layout noise)"),
    ],
}
MODULES = {name: module for group in COMMANDS.values() for name, module, _ in group}


def usage():
    lines = [__doc__.strip(), ""]
    for group, commands in COMMANDS.items():
        lines.append(f"{group}:")
        lines += [f"  {name:<21} {summary}" for name, _, summary in commands]
        lines.append("")
    return "\n".join(lines)


def main(argv):
    for words in (2, 1):
        name = " ".join(argv[:words])
        if len(argv) >= words and name in MODULES:
            module = importlib.import_module(f"oxbench.{MODULES[name]}")
            return module.main(argv[words:])
    if argv and argv[0] not in ("-h", "--help"):
        group = [name for name in MODULES if name.startswith(argv[0] + " ")]
        if group:
            print(f"bench.py {argv[0]}: choose one of: {', '.join(group)}", file=sys.stderr)
            return 2
        print(f"bench.py: unknown command: {' '.join(argv[:2])}\n", file=sys.stderr)
        print(usage(), file=sys.stderr)
        return 2
    print(usage())
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
