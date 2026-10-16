#!/usr/bin/env python3
"""Relink one saved LTO build at several .text placements (layout perturbations).

The method of #119: identical machine code at different addresses, so a
timing spread across the variants measures layout sensitivity, not code.

usage: relink.py BUILD_LOG OUT_PREFIX DELTA... [--binary NAME]
BUILD_LOG holds rustc's `--print link-args` output from a `-C save-temps` build:

    RUSTFLAGS="-C save-temps --print link-args" cargo build --release \
        -p quickjs-oxide-cli --bin oxide-run --target-dir T > build.log 2>&1

Writes OUT_PREFIX-d<delta> binaries and OUT_PREFIX-relinks.json with the .text
address and SHA-256 of each, and warns about any variant whose .text bytes
differ from the first offset variant (the default placement may differ in
alignment padding).
"""
import hashlib, json, re, shlex, subprocess, sys

args = sys.argv[1:]
binary = None
if "--binary" in args:
    at = args.index("--binary")
    binary = args[at + 1]
    del args[at:at + 2]
log, prefix, deltas = args[0], args[1], [int(d, 0) for d in args[2:]]
line = next(l for l in open(log)
            if '"cc"' in l and '"-o"' in l and (binary is None or f"/{binary}-" in l))
argv = shlex.split(line)
env = {}
while "=" in argv[0] and not argv[0].startswith("-"):
    k, v = argv.pop(0).split("=", 1)
    env[k] = v
out_index = argv.index("-o") + 1


def link(output, extra):
    args = list(argv)
    args[out_index] = output
    subprocess.run(args + extra, env={**env, "HOME": "/tmp"}, check=True)


def text_info(binary):
    sections = subprocess.run(["readelf", "-SW", binary], capture_output=True, text=True, check=True).stdout
    m = re.search(r"\]\s+\.text\s+\S+\s+([0-9a-f]+)\s+([0-9a-f]+)\s+([0-9a-f]+)", sections)
    addr, off, size = (int(x, 16) for x in m.groups())
    with open(binary, "rb") as f:
        f.seek(off)
        digest = hashlib.sha256(f.read(size)).hexdigest()
    return addr, size, digest


link(prefix + "-default", [])
base, size, digest = text_info(prefix + "-default")
receipts = [{"delta": "default", "text_addr": hex(base), "text_size": size, "sha256": digest}]
for d in deltas:
    out = f"{prefix}-d{d}"
    link(out, [f"-Wl,--section-start=.text={base + d:#x}"])
    a, s, h = text_info(out)
    receipts.append({"delta": d, "text_addr": hex(a), "text_size": s, "sha256": h})
    first = receipts[1]
    if (s, h) != (first["text_size"], first["sha256"]):
        print(f"WARNING delta {d}: .text differs from delta {first['delta']}", file=sys.stderr)
json.dump(receipts, open(prefix + "-relinks.json", "w"), indent=1)
for r in receipts:
    print(r)
