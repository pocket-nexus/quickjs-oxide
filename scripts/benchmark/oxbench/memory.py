"""Peak RSS of each engine over the fixed-work matrix, with the heap's figures.

Engines are `--engine NAME=PATH`, normally the base and candidate `qjs`; the
first is the reference of every ratio. QuickJS takes part only when it is
named as an engine. The programs are a `bench.py run` manifest's, by default
the 37 of `bench.py suite build` in `target/suite-matrix`. Every sample is one
process started by a small C helper (`vfork`, `execvp`, `wait4`), so
`ru_maxrss` is the measured process's own peak and never the image of a larger
parent. Every sample must exit 0 and print the manifest's expected output.
Each engine that accepts `--gc-stats --memory-report` (quickjs-oxide's `qjs`)
runs once more per program for the collector's and the heap's figures at exit.

A binary's resident file pages depend on how its page cache was filled: pages
written by a copy or a link sit in larger folios than pages read by faults,
and a fault maps its whole folio. Before measuring, every binary is evicted
from the page cache and run once, so its pages are the ones execution reads.
"""
import argparse
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tempfile
from .runner import load_workloads

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_MANIFEST = ROOT / "target/suite-matrix/manifest.json"

HELPER = r"""
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
/* helper OUT ERR cmd...: runs cmd once with stdout to OUT and stderr to ERR,
   prints "<exit status> <ru_maxrss KiB>". */
int main(int argc, char **argv) {
  if (argc < 4) return 2;
  pid_t pid = vfork();
  if (pid == 0) {
    int out = open(argv[1], O_WRONLY | O_CREAT | O_TRUNC, 0644);
    int err = open(argv[2], O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (out < 0 || err < 0) _exit(126);
    dup2(out, 1);
    dup2(err, 2);
    execvp(argv[3], argv + 3);
    _exit(127);
  }
  int status;
  struct rusage usage;
  if (wait4(pid, &status, 0, &usage) < 0) return 1;
  printf("%d %ld\n", WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status),
         usage.ru_maxrss);
  return 0;
}
"""

GC_STATS = re.compile(r"^gc: (\d+) collections, .* live (\d+) KiB, reserved (\d+) KiB$", re.M)
RESERVED = re.compile(r"^heap reserved: ([\d.]+) KiB; threshold ([\d.]+) KiB(?:; budget ([\d.]+) KiB)?", re.M)
CODE = re.compile(
    r"^code: (\d+) units(?: \(\d+ static, \d+ dynamic holding ([\d.]+) KiB\))?, (\d+) instructions "
    r"\(([\d.]+) KiB\), positions ([\d.]+) KiB, source ([\d.]+) KiB", re.M)


def programs(manifest, cases=None):
    """The manifest's workloads as (name, path, expected stdout)."""
    return [(item["case"], Path(item["path"]), item["expected"]) for item in load_workloads(manifest, cases=cases)]


def build_helper(directory):
    source = directory / "maxrss.c"
    binary = directory / "maxrss"
    source.write_text(HELPER)
    subprocess.run(["cc", "-O2", "-o", str(binary), str(source)], check=True)
    return binary


def sample(helper, command, scratch):
    """One run: (exit status, peak RSS in KiB, stdout, stderr)."""
    out, err = scratch / "stdout", scratch / "stderr"
    result = subprocess.run([str(helper), str(out), str(err), *map(str, command)],
                            capture_output=True, text=True, check=True)
    status, rss = map(int, result.stdout.split())
    return status, rss, out.read_text(errors="replace"), err.read_text(errors="replace")


def parse_reports(stderr):
    """The figures of `--gc-stats` and `--memory-report` (KiB)."""
    figures = {}
    match = GC_STATS.search(stderr)
    if not match:
        raise ValueError("no --gc-stats line")
    figures["collections"] = int(match[1])
    figures["live_kib"] = int(match[2])
    figures["reserved_kib"] = int(match[3])
    match = RESERVED.search(stderr)
    if not match:
        raise ValueError("no heap reserved line")
    figures["threshold_kib"] = float(match[2])
    figures["budget_kib"] = float(match[3]) if match[3] else None
    match = CODE.search(stderr)
    if not match:
        raise ValueError("no code line")
    figures["code_units"] = int(match[1])
    figures["code_dynamic_kib"] = float(match[2] or 0)
    figures["code_kib"] = float(match[4]) + float(match[5]) + float(match[6])
    figures["code_source_kib"] = float(match[6])
    return figures


def massif_peak(valgrind, qjs, path, scratch):
    """The compile-phase heap peak in bytes: `--disassemble`'s peak under massif."""
    out = scratch / "massif.out"
    subprocess.run([valgrind, "--tool=massif", f"--massif-out-file={out}", str(qjs), "--disassemble", str(path)],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    peak = 0
    snapshot = {}
    for line in out.read_text().splitlines():
        key, _, value = line.partition("=")
        if key in ("mem_heap_B", "mem_heap_extra_B"):
            snapshot[key] = int(value)
            if len(snapshot) == 2:
                peak = max(peak, sum(snapshot.values()))
                snapshot = {}
    return peak


def evict(path):
    """Drops a file's clean pages from the page cache."""
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.posix_fadvise(descriptor, 0, 0, os.POSIX_FADV_DONTNEED)
    finally:
        os.close(descriptor)


def reports(helper, binary, path, scratch):
    """The `--gc-stats --memory-report` figures, or None when the engine has no such options."""
    status, _, _, stderr = sample(helper, [binary, "--gc-stats", "--memory-report", path], scratch)
    if status != 0:
        return None
    try:
        return parse_reports(stderr)
    except ValueError:
        return None


def measure(args, engines):
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        helper = build_helper(scratch)
        empty = scratch / "empty.js"
        empty.write_text("")
        for binary in engines.values():
            evict(binary)
            sample(helper, [binary, empty], scratch)
        floor = {name: statistics.median(sample(helper, [binary, empty], scratch)[1] for _ in range(args.repeat))
                 for name, binary in engines.items()}
        reporting = [name for name, binary in engines.items() if reports(helper, binary, empty, scratch)]
        reference = next(iter(engines))
        rows = []
        for name, path, expected in programs(args.manifest, args.case):
            rss = {engine: [] for engine in engines}
            for repetition in range(args.repeat):
                offset = repetition % len(engines)
                order = list(engines)[offset:] + list(engines)[:offset]
                for engine in order:
                    status, peak, stdout, stderr = sample(helper, [engines[engine], path], scratch)
                    if status != 0:
                        raise RuntimeError(f"{name}: {engine} exited {status}: {stderr[-500:]}")
                    if stdout != expected:
                        raise RuntimeError(f"{name}: {engine} printed {stdout[-200:]!r}")
                    rss[engine].append(peak)
            kib = {engine: statistics.median(values) for engine, values in rss.items()}
            row = {"program": name, "kib": kib, "samples": rss,
                   "ratios": {engine: kib[engine] / kib[reference] for engine in engines if engine != reference},
                   "reports": {}}
            for engine in reporting:
                figures = reports(helper, engines[engine], path, scratch)
                if figures is None:
                    raise RuntimeError(f"{name}: the report run of {engine} failed")
                row["reports"][engine] = figures
                if args.compile_peak:
                    figures["compile_peak_kib"] = massif_peak(args.valgrind, engines[engine], path, scratch) / 1024
            rows.append(row)
            print(f"{name}: " + ", ".join(f"{engine} {value:.0f} KiB" for engine, value in kib.items()),
                  file=sys.stderr, flush=True)
    return floor, rows


def geomean(values):
    return math.exp(sum(math.log(value) for value in values) / len(values))


def markdown(floor, rows):
    engines = list(floor)
    reference, others = engines[0], engines[1:]
    reporting = [engine for engine in engines if any(engine in row["reports"] for row in rows)]
    budget = any(row["reports"][e].get("budget_kib") is not None for row in rows for e in row["reports"])
    compile_peak = any("compile_peak_kib" in row["reports"][e] for row in rows for e in row["reports"])
    fields = [("collections", "collections", "{:d}"), ("live_kib", "live KiB", "{:,}"),
              ("reserved_kib", "reserved KiB", "{:,}")]
    fields += [("budget_kib", "budget KiB", "{:,.0f}")] * budget + [("code_kib", "code KiB", "{:,.0f}")]
    fields += [("compile_peak_kib", "compile peak KiB", "{:,.0f}")] * compile_peak
    header = ["Program"] + [f"{engine} KiB" for engine in engines] + [f"{engine} / {reference}" for engine in others]
    header += [f"{engine} {label}" for engine in reporting for _, label, _ in fields]
    lines = ["| " + " | ".join(header) + " |", "| --- |" + " ---: |" * (len(header) - 1)]
    for row in rows:
        cells = [row["program"]] + [f"{row['kib'][engine]:,.0f}" for engine in engines]
        cells += [f"{row['ratios'][engine]:.2f}" for engine in others]
        for engine in reporting:
            figures = row["reports"].get(engine, {})
            cells += ["" if figures.get(key) is None else form.format(figures[key]) for key, _, form in fields]
        lines.append("| " + " | ".join(cells) + " |")
    lines.append("")
    lines.append("Empty script: " + ", ".join(f"{engine} {floor[engine]:,.0f} KiB" for engine in engines) + ".")
    for engine in others:
        lines.append(f"Geometric mean of {engine} / {reference} over {len(rows)} programs: "
                     f"{geomean([row['ratios'][engine] for row in rows]):.3f}")
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py memory", description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--engine", action="append", required=True,
                        help="NAME=PATH, repeatable; the first is the reference")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--case", action="append", help="a workload of the manifest; repeatable")
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--compile-peak", action="store_true",
                        help="add the compile-phase heap peak (massif over --disassemble; slow)")
    parser.add_argument("--valgrind", default="valgrind")
    parser.add_argument("--output", type=Path, help="write the rows as JSON here")
    parser.add_argument("--markdown", type=Path, help="write the table as Markdown here")
    args = parser.parse_args(argv)
    if args.repeat < 1:
        parser.error("--repeat must be positive")
    engines = {}
    for entry in args.engine:
        name, separator, path = entry.partition("=")
        if not separator or not re.fullmatch(r"[A-Za-z0-9_-]+", name) or name in engines:
            parser.error("engines must be unique NAME=PATH entries")
        if not os.access(path, os.X_OK):
            parser.error(f"not an executable: {path}")
        engines[name] = Path(path).resolve()
    floor, rows = measure(args, engines)
    table = markdown(floor, rows)
    if args.output:
        args.output.write_text(json.dumps({"engines": {name: str(path) for name, path in engines.items()},
                                           "repeat": args.repeat, "empty_kib": floor, "rows": rows}, indent=1) + "\n")
    if args.markdown:
        args.markdown.write_text(table)
    sys.stdout.write(table)
