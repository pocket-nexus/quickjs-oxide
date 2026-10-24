"""Parametric record declarations for methods that do not inspect opaque data.

Every production field is retained. Uninspected field types become independent
type parameters, so the proof must hold for any choice of those types. This is
stronger than selecting their concrete production types, not a field-erasing
heap model. Unsupported multiline field declarations fail closed.
"""

import re

from source import item, masked


def single_type(text):
    """Reject a second top-level field hidden on the same physical line."""
    stack = []
    pairs = {">": "<", ")": "(", "]": "[", "}": "{"}
    for token in text:
        if token in "<([{":
            stack.append(token)
        elif token in pairs:
            if not stack or stack.pop() != pairs[token]:
                raise ValueError("unsupported or unbalanced record field type")
        elif token == "," and not stack:
            raise ValueError("multiple fields on one line are not supported")
    if stack:
        raise ValueError("unbalanced record field type")


def record(source, name, prefix, retained):
    selected = item(source, rf"(?:pub\s+)?struct {name}\b")
    code = masked(selected)
    opening = code.index("{")
    fields = list(re.finditer(r"(?m)^([ \t]*)(?:pub(?:\(crate\))?\s+)?(\w+):\s*([^\n]+),\s*$",
                              code[opening + 1:-1]))
    seen = set()
    parameters = []
    replacements = []
    covered = list(code[opening + 1:-1])
    for field in fields:
        field_name, original = field[2], field[3].strip()
        single_type(original)
        if field_name in seen:
            raise ValueError(f"duplicate field {name}.{field_name}")
        seen.add(field_name)
        if field_name in retained:
            expected, replacement = retained[field_name]
            if "".join(original.split()) != "".join(expected.split()):
                raise ValueError(f"type changed for {name}.{field_name}: {original}")
        elif original in ("bool", "u32", "usize"):
            replacement = original
        else:
            replacement = prefix + "".join(part.title() for part in field_name.split("_"))
            parameters.append(replacement)
        start = opening + 1 + field.start(3)
        end = opening + 1 + field.end(3)
        replacements.append((start, end, replacement))
        for i in range(field.start(), field.end()):
            covered[i] = " "
    if set(retained) - seen:
        raise ValueError(f"missing retained fields in {name}: {set(retained) - seen}")
    remaining = "".join(covered)
    remaining = re.sub(r"#\[cfg\([^\n]+\)\]", "", remaining)
    if remaining.strip():
        raise ValueError(f"unsupported field syntax in {name}: {remaining.strip()}")
    for start, end, replacement in reversed(replacements):
        selected = selected[:start] + replacement + selected[end:]
    return selected, parameters, sorted(seen)
