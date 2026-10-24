"""Select Rust items and add ghost contracts to their production bodies.

This is a deliberately small lexer, not a Rust parser. rustc/Verus type-checks
the selected items. Unsupported literals fail closed, and every selection must
be unique in its explicitly selected scope. The optional panic diagnostic
projection preserves and verifies an explicitly named assertion condition.
"""

import re


def masked(source):
    """Mask comments and ordinary strings, preserving offsets and newlines."""
    chars = list(source)
    i = 0
    while i < len(source):
        start = i
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end
        elif source.startswith("/*", i):
            i += 2
            depth = 1
            while depth and i < len(source):
                if source.startswith("/*", i):
                    depth += 1
                    i += 2
                elif source.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            if depth:
                raise ValueError("unterminated Rust comment")
        elif source[i] == '"':
            if i and source[i - 1] in "r#":
                raise ValueError("raw Rust strings are not supported")
            i += 1
            while i < len(source) and source[i] != '"':
                i += 2 if source[i] == "\\" else 1
            if i >= len(source):
                raise ValueError("unterminated Rust string")
            i += 1
        elif source[i] == "'" and re.match(r"'(?:[^'\\]|\\.)'", source[i:]):
            raise ValueError("Rust character literals are not supported")
        else:
            i += 1
            continue
        for j in range(start, i):
            if chars[j] != "\n":
                chars[j] = " "
    return "".join(chars)


def item(source, pattern):
    """Extract a uniquely named item through its balanced body or semicolon."""
    code = masked(source)
    matches = list(re.finditer(pattern, code))
    if len(matches) != 1:
        raise ValueError(f"expected one Rust item matching {pattern!r}, got {len(matches)}")
    start = matches[0].start()
    pos = matches[0].end()
    brackets = 0
    while pos < len(code):
        token = code[pos]
        if brackets == 0 and token in ";{":
            break
        brackets += (token in "([") - (token in ")]")
        if brackets < 0:
            raise ValueError("unbalanced Rust signature")
        pos += 1
    if pos == len(code):
        raise ValueError("missing Rust item body")
    if code[pos] == ";":
        return source[start:pos + 1]
    depth, end = 1, pos + 1
    while depth and end < len(code):
        depth += (code[end] == "{") - (code[end] == "}")
        end += 1
    if depth:
        raise ValueError("unbalanced Rust item")
    return source[start:end]


def method(source, name, contract, result=None, before=None, after=None, loops=None,
           panic_condition=None, bindings=None):
    """Add contracts/ghost code and optionally erase a proved panic diagnostic."""
    selected = item(source, rf"(?:pub(?:\(crate\))?\s+)?(?:const\s+)?fn\s+{name}\b")
    opening = masked(selected).index("{")
    signature, body = selected[:opening], selected[opening:]
    # Proof-only modules are closed: contracts may name private record fields.
    # Visibility does not change executable statements or the record layout.
    signature = re.sub(r"^pub(?:\(crate\))?\s+", "", signature)
    if panic_condition is not None:
        # The verifier proves the condition of every selected assertion. Its
        # failure branch is therefore unreachable under the function contract.
        # Erase only formatting arguments in that branch, never the condition.
        code = masked(body)
        matches = list(re.finditer(r"\b(?:assert|debug_assert)!\(", code))
        if len(matches) != 1:
            raise ValueError("panic projection requires exactly one known assertion")
        for match in reversed(matches):
            depth, comma, end = 1, None, match.end()
            while depth and end < len(code):
                c = code[end]
                depth += (c in "([{") - (c in ")]}")
                if depth == 1 and c == "," and comma is None:
                    comma = end
                end += 1
            if depth:
                raise ValueError("unbalanced assertion")
            condition = code[match.end():comma if comma is not None else end - 1]
            if "".join(condition.split()) != "".join(panic_condition.split()):
                raise ValueError("panic projection assertion condition changed")
            if comma is not None:
                body = body[:comma] + body[end - 1:]
    if result is not None:
        signature, ty = signature.rsplit("->", 1)
        signature += f"-> ({result}: {ty.strip()})\n"
    for header, annotations in (loops or {}).items():
        anchor = header + " {"
        if body.count(anchor) != 1:
            raise ValueError(f"non-unique loop annotation point in {name}: {header}")
        body = body.replace(anchor, header + "\n" + annotations + "\n{")
    for anchor, variables in (bindings or {}).items():
        if body.count(anchor) != 1:
            raise ValueError(f"non-unique snapshot point in {name}: {anchor}")
        declarations = []
        for variable, expression in variables.items():
            if not re.fullmatch(r"[a-z][a-z0-9_]*", variable) or re.search(rf"\b{variable}\b", masked(body)):
                raise ValueError(f"invalid or conflicting ghost binding: {variable}")
            declarations.append(f"let ghost {variable} = {expression};")
        body = body.replace(anchor, "\n".join(declarations) + "\n" + anchor)
    for placements, position in ((before or {}, "before"), (after or {}, "after")):
        for statement, ghost in placements.items():
            if body.count(statement) != 1:
                raise ValueError(f"non-unique ghost annotation point in {name}: {statement}")
            annotation = "proof {\n" + ghost + "\n}"
            replacement = annotation + "\n" + statement if position == "before" else statement + "\n" + annotation
            body = body.replace(statement, replacement)
    return signature + "\n" + contract + "\n" + body
