"""Check the libcore Hasher adapters against the primitive calls verified by Verus."""

import os
from pathlib import Path
import re
import subprocess


# The 128- and 16-bit defaults consume native-endian bytes. Signed defaults
# delegate through the unsigned type of the same width. The two nightly
# methods also receive calls from stable libcore Hash implementations.
ADAPTERS = {
    "write_u16": ("u16", "s.write(&x.to_ne_bytes())"),
    "write_u128": ("u128", "s.write(&x.to_ne_bytes())"),
    "write_i8": ("i8", "s.write_u8(x as u8)"),
    "write_i16": ("i16", "s.write(&(x as u16).to_ne_bytes())"),
    "write_i32": ("i32", "s.write_u32(x as u32)"),
    "write_i64": ("i64", "s.write_u64(x as u64)"),
    "write_i128": ("i128", "s.write(&(x as u128).to_ne_bytes())"),
    "write_isize": ("isize", "s.write_usize(x as usize)"),
    "write_length_prefix": ("usize", "s.write_usize(x)"),
    "write_str": ("&str", "s.write(x.as_bytes()); s.write_u8(0xff)"),
}


def check(source, toolchain, flags, work):
    checked = Path(work) / "hash_dispatch.rs"
    ir = checked.with_suffix(".ll")
    code = ["#![feature(hasher_prefixfree_extras)]", source.read_text()]
    for method, (arg_type, expected) in ADAPTERS.items():
        for prefix, body in (("actual", f"s.{method}(x)"), ("expected", expected)):
            code.append(f"#[unsafe(no_mangle)] pub fn {prefix}_{method}(s: &mut FxHasher, "
                        f"x: {arg_type}) {{ {body} }}")
    checked.write_text("\n".join(code))
    name = "oxide_hash_dispatch"
    subprocess.run(["rustc", f"+{toolchain}", "--edition=2024", "--crate-type=lib",
                    "--crate-name", name, "-Copt-level=3", "--emit", f"llvm-ir={ir}",
                    *flags, str(checked)], check=True,
                   env={**os.environ, "RUSTC_BOOTSTRAP": name})
    llvm = ir.read_text()
    aliases = dict(re.findall(r"^@(\w+) = [^\n]*\balias\b[^\n]*, ptr @(\w+)$", llvm, re.M))
    definitions = set(re.findall(r"^define [^\n]*?@(\w+)\(", llvm, re.M))

    def definition(symbol):
        seen = set()
        while symbol in aliases and symbol not in seen:
            seen.add(symbol)
            symbol = aliases[symbol]
        if symbol not in definitions:
            raise ValueError(f"missing LLVM definition for {symbol}")
        return symbol

    for method in ADAPTERS:
        if definition("actual_" + method) != definition("expected_" + method):
            raise ValueError(f"distinct LLVM definitions for Hasher::{method}")
    print(f"Hasher dispatch: {len(ADAPTERS)} pairs resolve to identical LLVM definitions", flush=True)
