#!/usr/bin/env python3
"""Writes `apps/cli/qjs.order`, the link order of `qjs`'s startup and loop code.

Runs the release `qjs` under callgrind over an empty script, Octane's
Richards and the compile of one mid-sized source, collects every mangled
function name in the order the profiles first name it (`fn=` and `cfn=`
lines), keeps the names the binary defines, and writes them one per line.
`apps/cli/build.rs` passes the file to the linker, which places those
functions first and together in `.text`, so the file pages they touch at
startup are few.

    cargo build --release -p quickjs-oxide-cli
    python3 scripts/release/gen-symbol-order.py
"""
import argparse
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_BUNDLES = Path.home() / ".cache/engine-compare-20261015/bundles"
LINE = re.compile(r"^(c?fn|c?ob)=(?:\((\d+)\))?\s*(.*)$")
INTERPRETER = "oxide_core::vm::interp::run_at"


@dataclass(frozen=True)
class Symbol:
    address: int
    size: int
    name: str
    label: str


def function_symbols(binary):
    """Pair raw and demangled names without letting demangling reorder them."""
    command = ["nm", "-S", "--defined-only", "--no-sort"]
    raw = subprocess.check_output([*command, str(binary)], text=True).splitlines()
    demangled = subprocess.check_output([*command, "-C", str(binary)], text=True).splitlines()
    result = []
    for raw_line, demangled_line in zip(raw, demangled, strict=True):
        fields = raw_line.split(None, 3)
        if len(fields) == 3 and fields[1] in "tTwW":
            # CRT assembly functions may have no ELF size. They still own
            # text chunks: omitting them would move later functions.
            labels = demangled_line.split(None, 2)
            if fields[:2] != labels[:2]:
                raise ValueError("nm raw/demangled symbol tables disagree")
            result.append(Symbol(int(fields[0], 16), 0, fields[2], labels[2]))
            continue
        labels = demangled_line.split(None, 3)
        if len(fields) != 4 or fields[2] not in "tTwW":
            continue
        if fields[:3] != labels[:3]:
            raise ValueError("nm raw/demangled symbol tables disagree")
        result.append(Symbol(int(fields[0], 16), int(fields[1], 16), fields[3], labels[3]))
    return sorted(result, key=lambda symbol: (symbol.address, symbol.name))


def canonical_name(name):
    # Rust v0 crate disambiguators change when the workspace version changes.
    return re.sub(r"\.llvm\.[0-9]+", "", re.sub(r"Cs[0-9A-Za-z]+_", "Cs_", name))


def code_signature(binary, symbol):
    """Resolve relocated operands for aliases LLVM folds under another name.

    This is only a placement heuristic: it never changes code or relocations.
    Keep instruction constants and named external targets in the signature.
    """
    text = subprocess.check_output([
        "objdump", "-d", "-C", "--no-show-raw-insn",
        f"--start-address={symbol.address}",
        f"--stop-address={symbol.address + symbol.size}", str(binary),
    ], text=True)
    instructions = []
    for line in text.splitlines():
        match = re.match(r"\s*[0-9a-f]+:\s+(.*)", line)
        if match is None:
            continue
        instruction = match[1].strip()
        instruction = instruction.replace(f"<{symbol.label}+", "<SELF+")
        instruction = re.sub(r"\b[0-9a-f]{5,}\b(?=\s*<)", "TARGET", instruction)
        instruction = re.sub(r"-?0x[0-9a-f]+(?=\(%rip\))", "DISP", instruction)
        instruction = re.sub(r"#\s*[0-9a-f]+\s*<", "# TARGET <", instruction)
        instructions.append(instruction)
    return tuple(instructions)


def preserve_layout(previous, current, previous_code, current_code):
    """Translate the previous physical function order across a version bump.

    Reserve unambiguous symbol identities first, then equal-size demangled
    names. LLVM may choose a different representative for folded generic
    functions; only identical normalized bodies fill those remaining slots.
    Unmatched functions are left to the ordinary profile-driven ordering.
    """
    chosen, used = {}, set()
    by_name, by_label, by_size = defaultdict(list), defaultdict(list), defaultdict(list)
    for symbol in current:
        by_name[symbol.size, canonical_name(symbol.name)].append(symbol)
        by_label[symbol.size, symbol.label].append(symbol)
        by_size[symbol.size].append(symbol)
    signatures = {}

    def signature(symbol):
        if symbol.address not in signatures:
            signatures[symbol.address] = current_code(symbol)
        return signatures[symbol.address]

    def assign(old, candidates):
        candidates = [symbol for symbol in candidates if symbol.address not in used]
        if not candidates:
            return
        selected = min(candidates, key=lambda symbol: symbol.address)
        chosen[old.address] = selected.name
        used.add(selected.address)

    for old in previous:
        if old.address in chosen:
            continue
        candidates = by_name[old.size, canonical_name(old.name)]
        if len({symbol.address for symbol in candidates}) == 1:
            assign(old, candidates)
    for old in previous:
        if old.address not in chosen:
            assign(old, by_label[old.size, old.label])
    for old in previous:
        if old.address in chosen:
            continue
        candidates = [symbol for symbol in by_size[old.size] if symbol.address not in used]
        if candidates:
            old_code = previous_code(old)
            if old_code:
                assign(old, [symbol for symbol in candidates if signature(symbol) == old_code])
    return [chosen[address] for address in sorted(chosen)]


def interpreter_first(order, symbols):
    """Keep the profiled dispatch loop first, independently of symbol hashes."""
    matches = [symbol.name for symbol in symbols if symbol.label == INTERPRETER]
    if len(matches) != 1:
        raise ValueError("expected exactly one interpreter dispatch function")
    hot = matches[0]
    return [hot, *(name for name in order if name != hot)]


def defined_functions(binary):
    """The function symbols the binary defines."""
    output = subprocess.run(["nm", "--defined-only", str(binary)], capture_output=True, text=True,
                            check=True).stdout
    names = set()
    for line in output.splitlines():
        fields = line.split()
        if len(fields) == 3 and fields[1] in "tTwW":
            names.add(fields[2])
    return names


def profile_names(text, binary):
    """The names of the functions of `binary` in first-seen order. Callgrind
    writes a name once with its id, `fn=(12) name`, and the id alone
    afterwards; `ob=` names the object of the following `fn=` lines and
    `cob=` the object of the next `cfn=`."""
    names = {"fn": {}, "ob": {}}
    current = callee = None
    for line in text.splitlines():
        match = LINE.match(line)
        if not match:
            continue
        key, ident, name = match[1], match[2], match[3].strip()
        table = names[key.removeprefix("c")]
        if ident is not None:
            if name:
                table[ident] = name
            name = table.get(ident, "")
        if key == "ob":
            current = callee = name
        elif key == "cob":
            callee = name
        else:
            owner = current if key == "fn" else callee
            callee = current
            if name and owner == binary:
                yield name


def run_callgrind(valgrind, command, output):
    subprocess.run([valgrind, "--tool=callgrind", "--demangle=no", f"--callgrind-out-file={output}", *command],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    return output.read_text(errors="replace")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--qjs", type=Path, default=ROOT / "target/release/qjs")
    parser.add_argument("--bundles", type=Path, default=DEFAULT_BUNDLES,
                        help="the fixed-work bundles (octane/richards.js, octane/box2d.js)")
    parser.add_argument("--valgrind", default="valgrind")
    parser.add_argument("--output", type=Path, default=ROOT / "apps/cli/qjs.order")
    parser.add_argument("--layout-reference", type=Path,
                        help="preserve this pre-version-bump binary's function order")
    parser.add_argument("--check-inputs", action="store_true",
                        help="only check that valgrind and the bundles are present")
    args = parser.parse_args()
    richards = args.bundles / "octane/richards.js"
    compiled = args.bundles / "octane/box2d.js"
    if shutil.which(args.valgrind) is None:
        parser.error(f"missing: {args.valgrind}")
    for path in ([] if args.check_inputs else [args.qjs]) + [richards, compiled]:
        if not path.exists():
            parser.error(f"missing: {path}")
    if args.check_inputs:
        return
    defined = defined_functions(args.qjs)
    order, seen = [], set()
    if args.layout_reference is not None:
        order = preserve_layout(
            function_symbols(args.layout_reference), function_symbols(args.qjs),
            lambda symbol: code_signature(args.layout_reference, symbol),
            lambda symbol: code_signature(args.qjs, symbol),
        )
        seen.update(order)
        print(f"preserved {len(order)} function placements across version change", file=sys.stderr)
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        empty = scratch / "empty.js"
        empty.write_text("")
        workloads = [[args.qjs, empty], [args.qjs, richards], [args.qjs, "--disassemble", compiled]]
        for index, command in enumerate(workloads):
            text = run_callgrind(args.valgrind, list(map(str, command)), scratch / f"callgrind.{index}")
            for name in profile_names(text, str(args.qjs.resolve())):
                if name in defined and name not in seen:
                    seen.add(name)
                    order.append(name)
    order = interpreter_first(order, function_symbols(args.qjs))
    args.output.write_text("".join(f"{name}\n" for name in order))
    print(f"{args.output}: {len(order)} functions", file=sys.stderr)


if __name__ == "__main__":
    main()
