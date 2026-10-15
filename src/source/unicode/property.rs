//! Checksum-pinned Unicode 17 property sets used by RegExp property escapes.
//!
//! The generated compact range and code-point sequence streams are
//! materialized from the pinned QuickJS `libunicode.c` implementation. Product
//! builds consume only these Rust arrays; the C helper under
//! `apps/cli/tests/fixtures/` is a regeneration oracle.

mod tables {
    include!("generated/unicode/unicode_property_tables.rs");
}

/// Index of the alias group containing `name` in a generated name table.
fn find(names: &str, name: &str) -> Option<usize> {
    names
        .split(';')
        .position(|group| group.split(',').any(|alias| alias == name))
}

fn read_varint(data: &[u8], position: &mut usize) -> u32 {
    let mut value = 0;
    let mut shift = 0;
    loop {
        let byte = data[*position];
        *position += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return value;
        }
        shift += 7;
    }
}

/// Half-open `(start, end)` code-point ranges of the union of the selected
/// generated sets. Each set is ascending and disjoint; a union of several
/// sets is neither sorted nor merged.
#[derive(Clone, Debug)]
pub struct PropertyRanges {
    sets: &'static [u32],
    mask: u64,
    position: usize,
    remaining: u32,
    end: u32,
}

impl PropertyRanges {
    fn new(sets: &'static [u32], mask: u64) -> Self {
        Self {
            sets,
            mask,
            position: 0,
            remaining: 0,
            end: 0,
        }
    }
}

impl Iterator for PropertyRanges {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<(u32, u32)> {
        while self.remaining == 0 {
            if self.mask == 0 {
                return None;
            }
            let set = self.mask.trailing_zeros() as usize;
            self.mask &= self.mask - 1;
            self.position = self.sets[set] as usize;
            self.remaining = read_varint(&tables::RANGE_DATA, &mut self.position);
            self.end = 0;
        }
        self.remaining -= 1;
        let start = self.end + read_varint(&tables::RANGE_DATA, &mut self.position);
        self.end = start + read_varint(&tables::RANGE_DATA, &mut self.position);
        Some((start, self.end))
    }
}

pub fn general_category(name: &str) -> Option<PropertyRanges> {
    let mask = tables::GENERAL_CATEGORY_MASKS[find(tables::GENERAL_CATEGORY_NAMES, name)?];
    Some(PropertyRanges::new(
        &tables::GENERAL_CATEGORY_SETS,
        u64::from(mask),
    ))
}

pub fn script(name: &str, extensions: bool) -> Option<PropertyRanges> {
    let sets = if extensions {
        &tables::SCRIPT_EXTENSIONS_SETS
    } else {
        &tables::SCRIPT_SETS
    };
    let index = find(tables::SCRIPT_NAMES, name)?;
    Some(PropertyRanges::new(std::slice::from_ref(&sets[index]), 1))
}

pub fn binary_property(name: &str) -> Option<PropertyRanges> {
    let index = find(tables::BINARY_PROPERTY_NAMES, name)?;
    Some(PropertyRanges::new(
        std::slice::from_ref(&tables::BINARY_PROPERTY_SETS[index]),
        1,
    ))
}

/// Visit every code-point sequence of a sequence property; `None` when the
/// name is unknown. Sequences of a union are visited property by property.
pub fn sequence_property(name: &str, mut visit: impl FnMut(&[u32])) -> Option<()> {
    let index = find(tables::SEQUENCE_PROPERTY_NAMES, name)?;
    // RGI_Emoji, the one name past the stored sets, is the union of them all.
    let sets = tables::SEQUENCE_PROPERTY_SETS
        .get(index)
        .map_or(&tables::SEQUENCE_PROPERTY_SETS[..], std::slice::from_ref);
    let data = &tables::SEQUENCE_DATA;
    for &offset in sets {
        let mut position = offset as usize;
        let mut sequence = [0_u32; 16];
        let mut previous_len = 0;
        for _ in 0..read_varint(data, &mut position) {
            let len = usize::from(data[position]);
            position += 1;
            for index in 0..len {
                let base = match index {
                    _ if index < previous_len => sequence[index],
                    0 => 0,
                    _ => sequence[index - 1],
                };
                let delta = read_varint(data, &mut position);
                let delta = (delta >> 1) as i32 ^ -((delta & 1) as i32);
                sequence[index] = base.wrapping_add_signed(delta);
            }
            previous_len = len;
            visit(&sequence[..len]);
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        PropertyRanges, binary_property, general_category, script, sequence_property, tables,
    };

    fn sequences(name: &str) -> Vec<Vec<u32>> {
        let mut sequences = Vec::new();
        sequence_property(name, |sequence| sequences.push(sequence.to_vec())).unwrap();
        sequences
    }

    fn assert_valid_set(ranges: PropertyRanges) {
        let mut previous_end = 0;
        for (index, (start, end)) in ranges.enumerate() {
            assert!(start < end && end <= 0x11_0000);
            assert!(index == 0 || start >= previous_end);
            previous_end = end;
        }
    }

    fn names(table: &str) -> Vec<&str> {
        table
            .split(';')
            .flat_map(|group| group.split(','))
            .collect()
    }

    fn sorted(ranges: PropertyRanges) -> Vec<(u32, u32)> {
        merged(ranges.collect())
    }

    fn merged(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
        let mut merged: Vec<(u32, u32)> = Vec::new();
        ranges.sort_unstable();
        for (start, end) in ranges {
            match merged.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        merged
    }

    #[test]
    fn generated_property_catalog_matches_pinned_quickjs_shape() {
        assert_eq!(tables::GENERAL_CATEGORY_NAMES.split(';').count(), 38);
        assert_eq!(tables::SCRIPT_NAMES.split(';').count(), 176);
        assert_eq!(tables::BINARY_PROPERTY_NAMES.split(';').count(), 55);
        assert_eq!(names(tables::GENERAL_CATEGORY_NAMES).len(), 80);
        assert_eq!(names(tables::SCRIPT_NAMES).len(), 354);
        assert_eq!(names(tables::BINARY_PROPERTY_NAMES).len(), 102);

        for name in names(tables::GENERAL_CATEGORY_NAMES) {
            let ranges = general_category(name).unwrap();
            if ranges.mask.count_ones() == 1 {
                assert_valid_set(ranges);
            }
        }
        for name in names(tables::SCRIPT_NAMES) {
            assert_valid_set(script(name, false).unwrap());
            assert_valid_set(script(name, true).unwrap());
        }
        for name in names(tables::BINARY_PROPERTY_NAMES) {
            assert_valid_set(binary_property(name).unwrap());
        }
        // The base general categories partition the code space.
        assert_eq!(sorted(binary_property("Any").unwrap()), [(0, 0x11_0000)]);
        let all = PropertyRanges::new(&tables::GENERAL_CATEGORY_SETS, (1 << 30) - 1);
        assert_eq!(sorted(all.clone()), [(0, 0x11_0000)]);
        assert_eq!(all.map(|(start, end)| end - start).sum::<u32>(), 0x11_0000);
    }

    #[test]
    fn generated_sequence_property_catalog_matches_pinned_quickjs_shape() {
        let names = names(tables::SEQUENCE_PROPERTY_NAMES);
        assert_eq!(
            names,
            [
                "Basic_Emoji",
                "Emoji_Keycap_Sequence",
                "RGI_Emoji_Modifier_Sequence",
                "RGI_Emoji_Flag_Sequence",
                "RGI_Emoji_Tag_Sequence",
                "RGI_Emoji_ZWJ_Sequence",
                "RGI_Emoji",
            ]
        );
        let counts = names
            .iter()
            .map(|name| sequences(name).len())
            .collect::<Vec<_>>();
        assert_eq!(counts, [1400, 12, 670, 259, 3, 1614, 3958]);

        let all = sequences("RGI_Emoji");
        let mut unique = HashSet::with_capacity(all.len());
        for sequence in &all {
            assert!(!sequence.is_empty());
            assert!(sequence.iter().all(|code_point| *code_point < 0x11_0000));
            assert!(unique.insert(sequence.clone()), "duplicate {sequence:x?}");
        }
        assert_eq!(
            sequences("Emoji_Keycap_Sequence")[0],
            [0x23, 0xfe0f, 0x20e3]
        );
        assert!(sequence_property("basic_emoji", |_| {}).is_none());
    }

    #[test]
    fn aliases_are_exact_and_share_the_expected_sets() {
        let gc = |name| general_category(name).map(sorted);
        assert_eq!(gc("Letter"), gc("L"));
        assert_eq!(gc("Uppercase_Letter"), gc("Lu"));
        assert!(gc("letter").is_none());
        assert_eq!(gc("Nd").unwrap()[0], (0x30, 0x3a));

        let sc = |name, extensions| script(name, extensions).map(sorted);
        assert_eq!(sc("Latin", false), sc("Latn", false));
        assert_eq!(sc("Hiragana", true), sc("Hira", true));
        assert_ne!(sc("Hiragana", false), sc("Hiragana", true));

        let binary = |name| binary_property(name).map(sorted);
        assert_eq!(binary("ASCII_Hex_Digit"), binary("AHex"));
        assert_eq!(binary("Emoji_Presentation"), binary("EPres"));
        assert_eq!(binary("ASCII").unwrap(), [(0, 0x80)]);
        assert!(binary("RGI_Emoji").is_none());
        assert!(binary("ID_Compat_Math_Start").is_none());
    }
}
