#!/usr/bin/env python3
"""Check source ownership, module reachability and the public API boundary."""
from pathlib import Path
import re
import sys

# Skip ordinary Rust text in one search; only these prefixes need lexical work.
_LEXICAL_PREFIX = re.compile(r'//|/\*|(?:br|rb|cr|rc|r)#{0,255}"|[bc]?"')
_RAW_STRING_PREFIX = re.compile(r'(?:br|rb|cr|rc|r)(?P<hashes>#{0,255})"')


def _blank(text: str) -> str:
    return "".join("\n" if character == "\n" else " " for character in text)


def rust_code_only(source: str) -> str:
    """Remove comments and strings while retaining offsets and line numbers."""
    output: list[str] = []
    index = 0
    length = len(source)
    while index < length:
        prefix = _LEXICAL_PREFIX.search(source, index)
        if prefix is None:
            output.append(source[index:])
            break
        output.append(source[index:prefix.start()])
        index = prefix.start()
        if source.startswith("//", index):
            end = source.find("\n", index)
            if end < 0:
                end = length
            output.append(_blank(source[index:end]))
            index = end
            continue

        if source.startswith("/*", index):
            start = index
            depth = 1
            index += 2
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            output.append(_blank(source[start:index]))
            continue

        raw = _RAW_STRING_PREFIX.match(source, index)
        if raw is not None:
            start = index
            hashes = raw.group("hashes")
            index = raw.end()
            terminator = '"' + hashes
            end = source.find(terminator, index)
            index = length if end < 0 else end + len(terminator)
            output.append(_blank(source[start:index]))
            continue

        quote_offset = 1 if source[index:index + 2] in {'b"', 'c"'} else 0
        if source[index + quote_offset:index + quote_offset + 1] == '"':
            start = index
            index += quote_offset + 1
            while index < length:
                if source[index] == "\\":
                    index = min(length, index + 2)
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            output.append(_blank(source[start:index]))
            continue

        output.append(source[index])
        index += 1

    return "".join(output)


root = Path(__file__).resolve().parents[2]
src = root / "src"
errors = []
expected = {"compiler", "code", "value", "object", "atom", "heap", "vm",
            "realm", "builtins", "modules", "jobs", "host", "api"}
if {p.name for p in src.glob("*.rs")} != {"lib.rs"}:
    errors.append("src must contain only lib.rs as a top-level Rust source")
if {p.name for p in src.iterdir() if p.is_dir()} != {"engine", "source", "regexp"}:
    errors.append("src directories must be engine, source and regexp")
if {p.name for p in (src / "engine").iterdir() if p.is_dir()} != expected:
    errors.append("engine directories must match the declared responsibilities")

# The embedding boundary is explicit; old root aliases and implementation
# modules must not silently become public again.
root_code = rust_code_only((src / "lib.rs").read_text())
engine_code = rust_code_only((src / "engine/mod.rs").read_text())
if re.search(r"\bpub\s+use\b", root_code):
    errors.append("lib.rs must not reexport legacy module paths or API items")
if set(re.findall(r"\bpub\s+mod\s+(\w+)", engine_code)) != {"api"}:
    errors.append("engine::api must be the only public engine module")
for source_file in src.rglob("*.rs"):
    if re.search(r"use\s+crate::engine::heap::runtime::\*", rust_code_only(source_file.read_text())):
        errors.append(f"{source_file.relative_to(root)} must import actual owners, not the runtime facade")

pending = [src / "lib.rs"]
seen = set()
while pending:
    path = pending.pop()
    if path in seen:
        continue
    seen.add(path)
    source = path.read_text()
    code = rust_code_only(source)
    base = path.parent if path.name in {"lib.rs", "mod.rs"} else path.with_suffix("")
    inline = []
    for match in re.finditer(r"\bmod\s+(\w+)\s*([;{])", code):
        name, delimiter = match.groups()
        parents = [n for start, end, n in inline if start < match.start() < end]
        if delimiter == "{":
            depth = 1
            end = match.end()
            while end < len(code) and depth:
                depth += (code[end] == "{") - (code[end] == "}")
                end += 1
            inline.append((match.start(), end, name))
            continue
        stem = base.joinpath(*parents, name)
        candidates = [p for p in [stem.with_suffix(".rs"), stem / "mod.rs"] if p.is_file()]
        if len(candidates) != 1:
            errors.append(f"{path.relative_to(root)}: module {name} needs exactly one conventional source")
        else:
            pending.extend(candidates)
    for match in re.finditer(r'include!\(\s*"([^"]+\.rs)"\s*\)', source):
        if code[match.start():].startswith("include!"):
            included = path.parent / match[1]
            if not included.is_file():
                errors.append(f"{path.relative_to(root)}: missing include {match[1]}")
            else:
                seen.add(included)

all_sources = set(src.rglob("*.rs"))
for path in sorted(all_sources - seen):
    errors.append(f"unreachable Rust source: {path.relative_to(root)}")
if errors:
    print("\n".join("error: " + error for error in errors), file=sys.stderr)
    raise SystemExit(1)
print(f"Source layout passed: {len(all_sources)} reachable Rust files.")
