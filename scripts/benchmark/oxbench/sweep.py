"""The default check of a change: every fixed probe and the eight V8 v7
programs, base against candidate, with the merge gates evaluated.

Runs `bench.py run` twice with `--linux-counters` (the 20 fixed probes, then
the eight V8 v7 suite programs), prints both tables, and checks:

- the instruction gate: each V8 v7 program's candidate/base `instructions:u`
  is at most 1.02;
- the loop-size gate: the candidate's `run_at` is below 16,384 bytes.

A fixed probe above 1.02 in instructions is listed for explanation; it does
not fail the sweep. Exit status 1 means a failed sample or a failed gate.
"""
import argparse
import contextlib
import io
import json
import platform
import re
import subprocess
import sys
from pathlib import Path

from .common import ROOT
from . import runner, suite

FIXED = ROOT / "docs/performance/probes/fixed"
V8_CASES = ["richards", "deltablue", "crypto", "raytrace", "earley-boyer", "regexp", "splay", "navier-stokes"]
INSTRUCTION_GATE = 1.02
LOOP_SIZE_GATE = 16_384


def loop_size(binary):
    """Size in bytes of `vm::interp::run_at` in the binary's symbol table."""
    symbols = subprocess.run(["nm", "-C", "-S", str(binary)], capture_output=True, text=True, check=True).stdout
    for line in symbols.splitlines():
        if re.search(r"\bvm::interp::run_at$", line):
            return int(line.split()[1], 16)
    raise ValueError(f"no vm::interp::run_at symbol in {binary}")


def instruction_ratios(results):
    instructions = suite.medians(results, "instructions")
    ratios = {}
    for (case, engine), value in instructions.items():
        if engine == "candidate":
            base = instructions.get((case, "base"))
            ratios[case] = value / base if value is not None and base else None
    return ratios


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py sweep", description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--base", type=Path, required=True, help="release qjs of the base commit")
    parser.add_argument("--candidate", type=Path, required=True, help="release qjs of the change")
    parser.add_argument("--suite", type=Path, default=ROOT / "target/suite-matrix/manifest.json",
                        help="manifest from `bench.py suite build`")
    parser.add_argument("--repeat", type=int, default=4)
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--output", type=Path, required=True, help="new result directory")
    args = parser.parse_args(argv)
    if platform.system() != "Linux":
        parser.error("sweep reads instructions:u through Linux perf")
    if not args.suite.is_file():
        parser.error(f"{args.suite} is missing; generate it with `bench.py suite build` "
                     "(docs/performance/benchmarking.md#prepare)")
    if args.repeat < 2 or args.repeat % 2:
        parser.error("--repeat must be a positive even number")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    common = ["--engine", f"base={args.base.resolve()}", "--engine", f"candidate={args.candidate.resolve()}",
              "--repeat", str(args.repeat), "--order", "abba", "--cpu", str(args.cpu), "--linux-counters"]
    runs = {
        "fixed": ["--manifest", str(FIXED / "manifest.json"), "--workload-dir", str(FIXED)],
        "v8": ["--manifest", str(args.suite), *(arg for case in V8_CASES for arg in ("--case", f"v8-v7-{case}"))],
    }
    failed_samples = False
    report = []
    for name, selection in runs.items():
        print(f"== {name}: running", flush=True)
        with contextlib.redirect_stdout(io.StringIO()):
            status = runner.main([*selection, *common, "--output", str(output / name)])
        failed_samples |= status != 0
        table = io.StringIO()
        with contextlib.redirect_stdout(table):
            suite.table(argparse.Namespace(results=[output / name / "results.json"],
                                           reference="base", instructions=True))
        report += [f"## {name}", "", table.getvalue().rstrip(), ""]

    fixed_ratios = instruction_ratios(json.loads((output / "fixed/results.json").read_text()))
    v8_ratios = instruction_ratios(json.loads((output / "v8/results.json").read_text()))
    size = loop_size(args.candidate)
    over = [f"{case}: instructions ×{ratio:.4f}" if ratio else f"{case}: no instruction count"
            for case, ratio in v8_ratios.items() if ratio is None or ratio > INSTRUCTION_GATE]
    flagged = [f"{case}: instructions ×{ratio:.4f}" for case, ratio in fixed_ratios.items()
               if ratio is not None and ratio > INSTRUCTION_GATE]
    report += ["## Gates", "",
               f"- Instruction gate (eight V8 v7 programs, at most ×{INSTRUCTION_GATE}): {'FAIL' if over else 'pass'}"]
    report += [f"  - {item}" for item in over]
    report += [f"- Loop-size gate (run_at below {LOOP_SIZE_GATE} bytes): {size} bytes, "
               + ("pass" if size < LOOP_SIZE_GATE else "FAIL"),
               "- Samples: " + (f"some failed; read the raw output under {output}" if failed_samples else "all passed")]
    if flagged:
        report += ["- Fixed probes above the instruction bound (explain them):"] + [f"  - {item}" for item in flagged]
    text = "\n".join(report) + "\n"
    (output / "summary.md").write_text(text)
    sys.stdout.write(text)
    return 1 if failed_samples or over or size >= LOOP_SIZE_GATE else 0
