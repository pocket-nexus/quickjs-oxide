//! Flat string records. Ropes come in phase 4.
//!
//! The representation is canonical: a string whose code units all fit in one
//! byte is always `OneByte`, so two records with equal content have equal
//! representations.

use std::cell::Cell;

use super::arena::Record;
use super::object::NONE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StringRepr {
    OneByte(Box<[u8]>),
    TwoByte(Box<[u16]>),
}

impl StringRepr {
    pub fn from_text(text: &str) -> StringRepr {
        if text.is_ascii() {
            return StringRepr::OneByte(text.as_bytes().into());
        }
        let units: Vec<u16> = text.encode_utf16().collect();
        StringRepr::from_units(&units)
    }

    pub fn from_units(units: &[u16]) -> StringRepr {
        if units.iter().all(|&unit| unit <= 0xFF) {
            StringRepr::OneByte(units.iter().map(|&unit| unit as u8).collect())
        } else {
            StringRepr::TwoByte(units.into())
        }
    }

    pub fn len(&self) -> usize {
        match self {
            StringRepr::OneByte(bytes) => bytes.len(),
            StringRepr::TwoByte(units) => units.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn unit(&self, index: usize) -> u16 {
        match self {
            StringRepr::OneByte(bytes) => u16::from(bytes[index]),
            StringRepr::TwoByte(units) => units[index],
        }
    }

    pub fn units(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.len()).map(|index| self.unit(index))
    }

    pub fn content_bytes(&self) -> usize {
        match self {
            StringRepr::OneByte(bytes) => bytes.len(),
            StringRepr::TwoByte(units) => units.len() * 2,
        }
    }

    /// Lossy conversion for diagnostics and tests.
    pub fn to_rust_string(&self) -> String {
        match self {
            StringRepr::OneByte(bytes) => bytes.iter().map(|&byte| char::from(byte)).collect(),
            StringRepr::TwoByte(units) => String::from_utf16_lossy(units),
        }
    }

    /// The canonical array index (`0 ..= 2^32 - 2`) this string spells, if any.
    pub fn array_index(&self) -> Option<u32> {
        let len = self.len();
        if len == 0 || len > 10 {
            return None;
        }
        if self.unit(0) == u16::from(b'0') {
            return (len == 1).then_some(0);
        }
        let mut value: u64 = 0;
        for unit in self.units() {
            let digit = unit
                .checked_sub(u16::from(b'0'))
                .filter(|digit| *digit <= 9)?;
            value = value * 10 + u64::from(digit);
        }
        (value < u64::from(u32::MAX)).then_some(value as u32)
    }

    pub fn content_hash(&self) -> u32 {
        content_hash(self.units())
    }
}

pub fn content_hash(units: impl Iterator<Item = u16>) -> u32 {
    let mut hash: u32 = 0x811C_9DC5;
    for unit in units {
        hash = (hash ^ u32::from(unit)).wrapping_mul(0x0100_0193);
    }
    hash
}

pub struct StringRecord {
    pub repr: StringRepr,
    /// Computed on first use; 0 means not yet computed.
    hash: Cell<u32>,
    pub is_atom: bool,
    /// For atoms spelling an array index too large for an immediate
    /// `PropertyKey`: that index, else [`NONE`].
    pub array_index: u32,
}

impl StringRecord {
    pub fn new(repr: StringRepr) -> Self {
        StringRecord {
            repr,
            hash: Cell::new(0),
            is_atom: false,
            array_index: NONE,
        }
    }

    pub fn hash(&self) -> u32 {
        let cached = self.hash.get();
        if cached != 0 {
            return cached;
        }
        let hash = self.repr.content_hash().max(1);
        self.hash.set(hash);
        hash
    }
}

impl Record for StringRecord {
    fn vacant() -> Self {
        StringRecord::new(StringRepr::OneByte(Box::new([])))
    }

    fn extra_bytes(&self) -> usize {
        self.repr.content_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representation_is_canonical() {
        assert_eq!(
            StringRepr::from_text("abc"),
            StringRepr::OneByte(b"abc".to_vec().into())
        );
        assert_eq!(
            StringRepr::from_text("caf\u{e9}"),
            StringRepr::from_units(&[99, 97, 102, 0xE9])
        );
        assert!(matches!(
            StringRepr::from_text("\u{3b1}"),
            StringRepr::TwoByte(_)
        ));
        assert_eq!(StringRepr::from_text("\u{1F600}").len(), 2);
        assert_eq!(
            StringRepr::from_text("\u{3b1}x").to_rust_string(),
            "\u{3b1}x"
        );
    }

    #[test]
    fn array_index_is_canonical_u32_below_max() {
        let index = |text: &str| StringRepr::from_text(text).array_index();
        assert_eq!(index("0"), Some(0));
        assert_eq!(index("10"), Some(10));
        assert_eq!(index("4294967294"), Some(u32::MAX - 1));
        assert_eq!(index("4294967295"), None);
        assert_eq!(index("01"), None);
        assert_eq!(index(""), None);
        assert_eq!(index("-1"), None);
        assert_eq!(index("99999999999"), None);
    }
}
