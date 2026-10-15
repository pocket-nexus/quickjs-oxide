#!/usr/bin/env python3
"""Compact the Unicode property dump printed by the QuickJS oracle helper.

Reads the helper's expanded Rust-syntax dump on stdin and writes the product
table on stdout. Every derived or shared form is checked against the dump.
"""

import re
import sys

GC_BASE = 30
# QuickJS composite general categories, as unions of base category indices.
GC_COMPOSITES = {
    'LC': ['Lu', 'Ll', 'Lt'],
    'L': ['Lu', 'Ll', 'Lt', 'Lm', 'Lo'],
    'M': ['Mn', 'Mc', 'Me'],
    'N': ['Nd', 'Nl', 'No'],
    'S': ['Sm', 'Sc', 'Sk', 'So'],
    'P': ['Pc', 'Pd', 'Ps', 'Pe', 'Pi', 'Pf', 'Po'],
    'Z': ['Zs', 'Zl', 'Zp'],
    'C': ['Cn', 'Cc', 'Cf', 'Cs', 'Co'],
}
MAX_SEQUENCE = 16


def fail(message):
    sys.exit(f'error: {message}')


def parse(text):
    tables = {}
    pattern = r'pub\(super\) const (\w+): [^=]+= &\[\n(.*?)\n\];'
    for name, body in re.findall(pattern, text, re.S):
        if name.endswith('_ALIASES'):
            tables[name] = [(alias, int(index))
                            for alias, index in re.findall(r'\("(\w+)", (\d+)\)', body)]
        elif name == 'SEQUENCE_PROPERTY_SEQUENCES':
            tables[name] = [
                [[int(value, 16) for value in re.findall(r'0x[0-9a-f]+', sequence)]
                 for sequence in re.findall(r'&\[([^\[\]]*)\]', block)]
                for block in body.split('\n    ],') if '&[' in block
            ]
        else:
            tables[name] = [[int(value, 16) for value in re.findall(r'0x[0-9a-f]+', block)]
                            for block in body.split('    ],') if '&[' in block]
    return tables


def groups(aliases):
    """Alias groups ordered by property index, as `name,alias,...` strings."""
    result = []
    for alias, index in aliases:
        if index == len(result):
            result.append([alias])
        elif index == len(result) - 1:
            result[-1].append(alias)
        else:
            fail(f'non-contiguous alias index for {alias}')
    return result


def code_points(ranges):
    points = set()
    for start, end in zip(ranges[::2], ranges[1::2]):
        points.update(range(start, end))
    return points


def varint(value):
    if value < 0:
        fail('negative varint')
    out = bytearray()
    while True:
        byte = value & 0x7f
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def encode_ranges(ranges):
    out = bytearray(varint(len(ranges) // 2))
    end = 0
    for start, stop in zip(ranges[::2], ranges[1::2]):
        if not end <= start < stop <= 0x110000:
            fail('unsorted or empty range')
        out += varint(start - end) + varint(stop - start)
        end = stop
    return bytes(out)


def zigzag(value):
    return value << 1 if value >= 0 else ((-value) << 1) - 1


def encode_sequences(sequences):
    out = bytearray(varint(len(sequences)))
    previous = []
    for sequence in sequences:
        if not 0 < len(sequence) <= MAX_SEQUENCE:
            fail('sequence length out of range')
        out.append(len(sequence))
        for index, value in enumerate(sequence):
            if index < len(previous):
                base = previous[index]
            elif index:
                base = sequence[index - 1]
            else:
                base = 0
            out += varint(zigzag(value - base))
        previous = sequence
    return bytes(out)


class Blob:
    def __init__(self):
        self.data = bytearray()
        self.offsets = {}

    def add(self, encoded):
        if encoded not in self.offsets:
            self.offsets[encoded] = len(self.data)
            self.data += encoded
        return self.offsets[encoded]


def rust_names(constant, alias_groups):
    text = ';'.join(','.join(group) for group in alias_groups)
    lines, line = [], ''
    for piece in re.findall(r'[^;]*;?', text):
        if line and len(line) + len(piece) > 92:
            lines.append(line)
            line = ''
        line += piece
    lines.append(line)
    body = '\\\n    '.join(lines)
    return f'pub(super) const {constant}: &str = "\\\n    {body}";\n'


def rust_array(constant, kind, values):
    items = [str(value) for value in values]
    lines, line = [], ''
    for item in items:
        if line and len(line) + len(item) + 1 > 96:
            lines.append(line)
            line = ''
        line += item + ','
    lines.append(line)
    body = '\n    '.join(lines)
    return f'pub(super) static {constant}: [{kind}; {len(values)}] = [\n    {body}\n];\n'


def main():
    text = sys.stdin.read()
    header = ''.join(line + '\n' for line in text.splitlines() if line.startswith('//')
                     and not line.startswith('// End'))
    tables = parse(text)

    gc_groups = groups(tables['GENERAL_CATEGORY_ALIASES'])
    gc_ranges = tables['GENERAL_CATEGORY_RANGES']
    if len(gc_groups) != len(gc_ranges) or len(gc_groups) != GC_BASE + len(GC_COMPOSITES):
        fail('general category shape drifted')
    gc_index = {group[0]: index for index, group in enumerate(gc_groups)}
    masks = [1 << index for index in range(GC_BASE)]
    for group in gc_groups[GC_BASE:]:
        members = [gc_index[name] for name in GC_COMPOSITES[group[0]]]
        union = set()
        for member in members:
            union |= code_points(gc_ranges[member])
        if union != code_points(gc_ranges[gc_index[group[0]]]):
            fail(f'general category {group[0]} is not the union of its members')
        masks.append(sum(1 << member for member in members))

    script_groups = groups(tables['SCRIPT_ALIASES'])
    binary_groups = groups(tables['BINARY_PROPERTY_ALIASES'])
    if len(script_groups) != len(tables['SCRIPT_RANGES']) or \
            len(script_groups) != len(tables['SCRIPT_EXTENSIONS_RANGES']) or \
            len(binary_groups) != len(tables['BINARY_PROPERTY_RANGES']):
        fail('property alias and range counts differ')

    ranges = Blob()
    gc_sets = [ranges.add(encode_ranges(r)) for r in gc_ranges[:GC_BASE]]
    script_sets = [ranges.add(encode_ranges(r)) for r in tables['SCRIPT_RANGES']]
    extension_sets = [ranges.add(encode_ranges(r)) for r in tables['SCRIPT_EXTENSIONS_RANGES']]
    binary_sets = [ranges.add(encode_ranges(r)) for r in tables['BINARY_PROPERTY_RANGES']]

    sequence_groups = groups(tables['SEQUENCE_PROPERTY_ALIASES'])
    sequence_props = tables['SEQUENCE_PROPERTY_SEQUENCES']
    if len(sequence_groups) != len(sequence_props) or sequence_groups[-1] != ['RGI_Emoji']:
        fail('sequence property shape drifted')
    union = [tuple(sequence) for prop in sequence_props[:-1] for sequence in prop]
    if len(set(union)) != len(union) or set(union) != set(map(tuple, sequence_props[-1])):
        fail('RGI_Emoji is not the disjoint union of the other sequence properties')
    sequences = Blob()
    sequence_sets = [sequences.add(encode_sequences(prop)) for prop in sequence_props[:-1]]

    out = [header, """//
// Property names are `;`-separated groups of `,`-separated aliases. Range
// sets are LEB128 streams in RANGE_DATA: a range count, then per half-open
// range its gap from the previous range end and its length. General category
// masks select base category sets. Sequence properties are LEB128 streams in
// SEQUENCE_DATA: a count, then per sequence a length byte and zigzag deltas
// from the previous sequence's code point at the same position (else from
// the preceding code point). RGI_Emoji, the last sequence property, is the
// union of all the others.
""", '\n',
        rust_names('GENERAL_CATEGORY_NAMES', gc_groups), '\n',
        rust_array('GENERAL_CATEGORY_MASKS', 'u32', masks), '\n',
        rust_array('GENERAL_CATEGORY_SETS', 'u32', gc_sets), '\n',
        rust_names('SCRIPT_NAMES', script_groups), '\n',
        rust_array('SCRIPT_SETS', 'u32', script_sets), '\n',
        rust_array('SCRIPT_EXTENSIONS_SETS', 'u32', extension_sets), '\n',
        rust_names('BINARY_PROPERTY_NAMES', binary_groups), '\n',
        rust_array('BINARY_PROPERTY_SETS', 'u32', binary_sets), '\n',
        rust_names('SEQUENCE_PROPERTY_NAMES', sequence_groups), '\n',
        rust_array('SEQUENCE_PROPERTY_SETS', 'u32', sequence_sets), '\n',
        rust_array('RANGE_DATA', 'u8', ranges.data), '\n',
        rust_array('SEQUENCE_DATA', 'u8', sequences.data), '\n',
        '// End of generated Unicode property tables.\n']
    sys.stdout.write(''.join(out))


main()
