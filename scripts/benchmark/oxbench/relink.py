"""Relink one saved LTO build at several .text placements (layout perturbations).

The method of #119: identical machine code at different addresses, so a
timing spread across the variants measures layout sensitivity, not code.

The build log holds rustc's `--print link-args` output from a `-C save-temps` build:

    RUSTFLAGS="-C save-temps --print link-args" cargo build --release \
        -p quickjs-oxide-cli --bin qjs --target-dir T > build.log 2>&1

Writes OUT_PREFIX-d<delta> binaries and OUT_PREFIX-relinks.json with the .text
address and SHA-256 of each, and warns about any variant whose .text bytes
differ from the first offset variant (the default placement may differ in
alignment padding).
"""
import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys


def text_info(binary):
    sections = subprocess.run(["readelf", "-SW", binary], capture_output=True, text=True, check=True).stdout
    m = re.search(r"\]\s+\.text\s+\S+\s+([0-9a-f]+)\s+([0-9a-f]+)\s+([0-9a-f]+)", sections)
    addr, off, size = (int(x, 16) for x in m.groups())
    with open(binary, "rb") as f:
        f.seek(off)
        digest = hashlib.sha256(f.read(size)).hexdigest()
    return addr, size, digest


def main(argv=None):
    parser = argparse.ArgumentParser(prog="bench.py relink", description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("build_log", help="build log with rustc's --print link-args output")
    parser.add_argument("prefix", help="output prefix for the relinked binaries")
    parser.add_argument("deltas", nargs="+", type=lambda d: int(d, 0), help=".text offsets from the default placement")
    parser.add_argument("--binary", help="binary name when the log links several")
    args = parser.parse_args(argv)
    line = next(l for l in open(args.build_log)
                if '"cc"' in l and '"-o"' in l and (args.binary is None or f"/{args.binary}-" in l))
    link_argv = shlex.split(line)
    env = {}
    while "=" in link_argv[0] and not link_argv[0].startswith("-"):
        k, v = link_argv.pop(0).split("=", 1)
        env[k] = v
    out_index = link_argv.index("-o") + 1

    def link(output, extra):
        command = list(link_argv)
        command[out_index] = output
        subprocess.run(command + extra, env={**env, "HOME": "/tmp"}, check=True)

    prefix = args.prefix
    link(prefix + "-default", [])
    base, size, digest = text_info(prefix + "-default")
    receipts = [{"delta": "default", "text_addr": hex(base), "text_size": size, "sha256": digest}]
    for d in args.deltas:
        out = f"{prefix}-d{d}"
        link(out, [f"-Wl,--section-start=.text={base + d:#x}"])
        a, s, h = text_info(out)
        receipts.append({"delta": d, "text_addr": hex(a), "text_size": s, "sha256": h})
        first = receipts[1]
        if (s, h) != (first["text_size"], first["sha256"]):
            print(f"WARNING delta {d}: .text differs from delta {first['delta']}", file=sys.stderr)
    with open(prefix + "-relinks.json", "w") as f:
        json.dump(receipts, f, indent=1)
    for r in receipts:
        print(r)
