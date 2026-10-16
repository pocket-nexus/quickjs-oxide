//! The 8-byte NaN-boxed `Value` and the heap identifiers it carries.
//!
//! Any bit pattern below `0xFFF9_0000_0000_0000` is an `f64`. The boxed range is
//! the quiet-NaN space with the sign bit set: the top 16 bits select the kind
//! and the low 32 bits carry the payload.
//!
//! | Top 16 bits | Kind | Payload |
//! | --- | --- | --- |
//! | `0xFFF9` | Int32 | `i32` |
//! | `0xFFFA` | Bool, Undefined, Null, Empty | bits 0–1: 0 Bool, 1 Undefined, 2 Null, 3 Empty; bit 2: the boolean |
//! | `0xFFFB` | Object | `u32` object index |
//! | `0xFFFC` | String | `u32` string index |
//! | `0xFFFD` | Symbol | `u32` symbol index |
//! | `0xFFFE` | BigInt | `u32` BigInt index |
//! | `0xFFFF` | reserved | |

use std::fmt;

const TAG_SHIFT: u32 = 48;
const TAG_INT32: u64 = 0xFFF9;
const TAG_SPECIAL: u64 = 0xFFFA;
const TAG_OBJECT: u64 = 0xFFFB;
const TAG_STRING: u64 = 0xFFFC;
const TAG_SYMBOL: u64 = 0xFFFD;
const TAG_BIGINT: u64 = 0xFFFE;

/// Lowest boxed bit pattern; everything below it is a double.
const BOXED_MIN: u64 = TAG_INT32 << TAG_SHIFT;
/// The canonical NaN every NaN input is rewritten to.
pub const CANONICAL_NAN: u64 = 0x7FF8_0000_0000_0000;

const SPECIAL_BOOL: u64 = 0;
const SPECIAL_UNDEFINED: u64 = 1;
const SPECIAL_NULL: u64 = 2;
const SPECIAL_EMPTY: u64 = 3;
const SPECIAL_TRUE_BIT: u64 = 1 << 2;

const fn boxed(tag: u64, payload: u32) -> u64 {
    (tag << TAG_SHIFT) | payload as u64
}

/// The kind of a [`Value`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    Number,
    Int32,
    Bool,
    Undefined,
    Null,
    Empty,
    Object,
    String,
    Symbol,
    BigInt,
}

/// `kind()` lookup: index 0 is every double, 1..=6 are the boxed tags `0xFFF9`
/// to `0xFFFE`, 7 is the reserved tag, 8..=11 are the special payloads.
/// Indices 12..=15 are never produced.
const KIND_TABLE: [Kind; 16] = [
    Kind::Number,
    Kind::Int32,
    Kind::Bool, // replaced by the special payload index, never read
    Kind::Object,
    Kind::String,
    Kind::Symbol,
    Kind::BigInt,
    Kind::Number, // reserved tag `0xFFFF`; never constructed
    Kind::Bool,
    Kind::Undefined,
    Kind::Null,
    Kind::Empty,
    Kind::Number, // 12..=15: padding so the masked index needs no bounds check
    Kind::Number,
    Kind::Number,
    Kind::Number,
];

/// A JavaScript value, or the internal hole marker [`Value::EMPTY`].
///
/// `Copy`, no `Drop`, no ownership protocol. Heap kinds carry a 32-bit arena
/// index; holding one across an allocation requires rooting it (see the crate
/// README).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(u64);

const _: () = assert!(size_of::<Value>() == 8);
const _: () = assert!(size_of::<[Value; 4]>() == 32);

impl Value {
    pub const UNDEFINED: Value = Value(boxed(TAG_SPECIAL, SPECIAL_UNDEFINED as u32));
    pub const NULL: Value = Value(boxed(TAG_SPECIAL, SPECIAL_NULL as u32));
    pub const TRUE: Value = Value(boxed(TAG_SPECIAL, (SPECIAL_BOOL | SPECIAL_TRUE_BIT) as u32));
    pub const FALSE: Value = Value(boxed(TAG_SPECIAL, SPECIAL_BOOL as u32));
    /// The internal hole marker: array holes, uninitialized bindings, empty
    /// slots. Never a JavaScript value.
    pub const EMPTY: Value = Value(boxed(TAG_SPECIAL, SPECIAL_EMPTY as u32));

    /// A double. NaN is canonicalized; no attempt is made to produce an Int32.
    #[inline]
    pub fn number(value: f64) -> Value {
        if value.is_nan() {
            Value(CANONICAL_NAN)
        } else {
            Value(value.to_bits())
        }
    }

    #[inline]
    pub const fn int32(value: i32) -> Value {
        Value(boxed(TAG_INT32, value as u32))
    }

    #[inline]
    pub const fn bool(value: bool) -> Value {
        if value { Value::TRUE } else { Value::FALSE }
    }

    #[inline]
    pub const fn object(id: ObjectId) -> Value {
        Value(boxed(TAG_OBJECT, id.0))
    }

    #[inline]
    pub const fn string(id: StringId) -> Value {
        Value(boxed(TAG_STRING, id.0))
    }

    #[inline]
    pub const fn symbol(id: SymbolId) -> Value {
        Value(boxed(TAG_SYMBOL, id.0))
    }

    #[inline]
    pub const fn bigint(id: BigIntId) -> Value {
        Value(boxed(TAG_BIGINT, id.0))
    }

    /// The raw bit pattern.
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn kind(self) -> Kind {
        let tag = (self.0 >> TAG_SHIFT) as u32;
        // 0 for every double, 1..=7 for the boxed tags.
        let index = tag.saturating_sub(TAG_INT32 as u32 - 1) as usize;
        // The special tag sits at index 2; move it to 8 + the special payload.
        let special = usize::from(tag == TAG_SPECIAL as u32) * (6 + (self.0 & 3) as usize);
        KIND_TABLE[(index + special) & 15]
    }

    /// Int32 or double.
    #[inline]
    pub const fn is_number(self) -> bool {
        self.0 < (TAG_SPECIAL << TAG_SHIFT)
    }

    #[inline]
    pub const fn is_double(self) -> bool {
        self.0 < BOXED_MIN
    }

    #[inline]
    pub const fn is_int32(self) -> bool {
        self.0 >> TAG_SHIFT == TAG_INT32
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == Value::EMPTY.0
    }

    #[inline]
    pub const fn is_undefined(self) -> bool {
        self.0 == Value::UNDEFINED.0
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == Value::NULL.0
    }

    /// The numeric value, Int32 widened.
    #[inline]
    pub fn as_f64(self) -> Option<f64> {
        if self.is_double() {
            Some(f64::from_bits(self.0))
        } else if self.is_int32() {
            Some(f64::from(self.0 as u32 as i32))
        } else {
            None
        }
    }

    #[inline]
    pub const fn as_int32(self) -> Option<i32> {
        if self.is_int32() {
            Some(self.0 as u32 as i32)
        } else {
            None
        }
    }

    #[inline]
    pub const fn as_bool(self) -> Option<bool> {
        if self.0 >> 3 == Value::FALSE.0 >> 3 && self.0 & 3 == SPECIAL_BOOL {
            Some(self.0 & SPECIAL_TRUE_BIT != 0)
        } else {
            None
        }
    }

    #[inline]
    const fn payload_if(self, tag: u64) -> Option<u32> {
        if self.0 >> TAG_SHIFT == tag {
            Some(self.0 as u32)
        } else {
            None
        }
    }

    #[inline]
    pub const fn as_object(self) -> Option<ObjectId> {
        match self.payload_if(TAG_OBJECT) {
            Some(index) => Some(ObjectId(index)),
            None => None,
        }
    }

    #[inline]
    pub const fn as_string(self) -> Option<StringId> {
        match self.payload_if(TAG_STRING) {
            Some(index) => Some(StringId(index)),
            None => None,
        }
    }

    #[inline]
    pub const fn as_symbol(self) -> Option<SymbolId> {
        match self.payload_if(TAG_SYMBOL) {
            Some(index) => Some(SymbolId(index)),
            None => None,
        }
    }

    #[inline]
    pub const fn as_bigint(self) -> Option<BigIntId> {
        match self.payload_if(TAG_BIGINT) {
            Some(index) => Some(BigIntId(index)),
            None => None,
        }
    }

    /// True for the kinds whose payload is a heap index.
    #[inline]
    pub const fn is_heap(self) -> bool {
        let tag = self.0 >> TAG_SHIFT;
        tag >= TAG_OBJECT && tag <= TAG_BIGINT
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            Kind::Number => write!(f, "Number({:?})", f64::from_bits(self.0)),
            Kind::Int32 => write!(f, "Int32({})", self.0 as u32 as i32),
            Kind::Bool => write!(f, "Bool({})", self.0 & SPECIAL_TRUE_BIT != 0),
            Kind::Undefined => f.write_str("Undefined"),
            Kind::Null => f.write_str("Null"),
            Kind::Empty => f.write_str("Empty"),
            Kind::Object => write!(f, "Object({})", self.0 as u32),
            Kind::String => write!(f, "String({})", self.0 as u32),
            Kind::Symbol => write!(f, "Symbol({})", self.0 as u32),
            Kind::BigInt => write!(f, "BigInt({})", self.0 as u32),
        }
    }
}

macro_rules! identifier {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub(crate) u32);

        impl $name {
            /// The arena index.
            #[inline]
            pub const fn index(self) -> u32 {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

identifier!(
    /// Index of an [`ObjectHeader`](crate::heap::object::ObjectHeader).
    ObjectId
);
identifier!(
    /// Index of a string record.
    StringId
);
identifier!(
    /// Index of a shape.
    ShapeId
);
identifier!(
    /// Index of a symbol record.
    SymbolId
);
identifier!(
    /// Index of a BigInt record.
    BigIntId
);
identifier!(
    /// An object of kind `Scope`; the same index space as [`ObjectId`].
    ScopeId
);
identifier!(
    /// Index of a validity cell.
    ValidityCellId
);

impl ScopeId {
    #[inline]
    pub const fn object(self) -> ObjectId {
        ObjectId(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size_of::<Value>(), 8);
        assert_eq!(size_of::<[Value; 4]>(), 32);
    }

    #[test]
    fn nan_is_canonical() {
        assert_eq!(Value::number(f64::NAN).0, 0x7FF8_0000_0000_0000);
        for bits in [
            0x7FF8_0000_0000_0001u64,
            0x7FF0_0000_0000_0001, // signalling
            0xFFF8_0000_0000_0000, // negative quiet
            0xFFF9_0000_0000_0000, // would alias Int32
            0xFFFB_0000_0000_0007, // would alias Object(7)
            0xFFFF_FFFF_FFFF_FFFF,
        ] {
            let value = Value::number(f64::from_bits(bits));
            assert_eq!(value.0, CANONICAL_NAN, "{bits:#x}");
            assert_eq!(value.kind(), Kind::Number);
            assert!(value.as_f64().unwrap().is_nan());
        }
    }

    #[test]
    fn doubles_round_trip() {
        let cases = [
            0.0,
            -0.0,
            1.0,
            -1.5,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::from_bits(1),                      // smallest subnormal
            -f64::from_bits(0x000F_FFFF_FFFF_FFFF), // largest negative subnormal
            f64::EPSILON,
            2f64.powi(53),
        ];
        for case in cases {
            let value = Value::number(case);
            assert_eq!(value.kind(), Kind::Number);
            assert!(value.is_number() && value.is_double() && !value.is_int32());
            assert_eq!(value.as_f64().unwrap().to_bits(), case.to_bits());
            assert_eq!(value.as_int32(), None);
            assert_eq!(value.as_object(), None);
        }
        assert_ne!(Value::number(0.0), Value::number(-0.0));
    }

    #[test]
    fn int32_round_trip() {
        for case in [0, 1, -1, i32::MIN, i32::MAX, 42, -123_456] {
            let value = Value::int32(case);
            assert_eq!(value.kind(), Kind::Int32);
            assert!(value.is_number() && value.is_int32() && !value.is_double());
            assert_eq!(value.as_int32(), Some(case));
            assert_eq!(value.as_f64(), Some(f64::from(case)));
            assert_eq!(value.as_bool(), None);
        }
    }

    #[test]
    fn specials_round_trip() {
        let cases = [
            (Value::TRUE, Kind::Bool),
            (Value::FALSE, Kind::Bool),
            (Value::UNDEFINED, Kind::Undefined),
            (Value::NULL, Kind::Null),
            (Value::EMPTY, Kind::Empty),
        ];
        for (index, (value, kind)) in cases.into_iter().enumerate() {
            assert_eq!(value.kind(), kind);
            assert!(!value.is_number() && !value.is_heap());
            assert_eq!(value.as_f64(), None);
            for (other, (other_value, _)) in cases.into_iter().enumerate() {
                assert_eq!(index == other, value == other_value);
            }
        }
        assert_eq!(Value::TRUE.as_bool(), Some(true));
        assert_eq!(Value::FALSE.as_bool(), Some(false));
        assert_eq!(Value::bool(true), Value::TRUE);
        assert_eq!(Value::UNDEFINED.as_bool(), None);
        assert_eq!(Value::EMPTY.as_bool(), None);
        assert!(Value::EMPTY.is_empty() && !Value::UNDEFINED.is_empty());
        assert!(Value::UNDEFINED.is_undefined() && Value::NULL.is_null());
    }

    #[test]
    fn heap_kinds_round_trip() {
        for index in [0, 1, 0x7FFF_FFFF, u32::MAX] {
            let object = Value::object(ObjectId(index));
            let string = Value::string(StringId(index));
            let symbol = Value::symbol(SymbolId(index));
            let bigint = Value::bigint(BigIntId(index));
            assert_eq!(object.kind(), Kind::Object);
            assert_eq!(string.kind(), Kind::String);
            assert_eq!(symbol.kind(), Kind::Symbol);
            assert_eq!(bigint.kind(), Kind::BigInt);
            assert_eq!(object.as_object(), Some(ObjectId(index)));
            assert_eq!(string.as_string(), Some(StringId(index)));
            assert_eq!(symbol.as_symbol(), Some(SymbolId(index)));
            assert_eq!(bigint.as_bigint(), Some(BigIntId(index)));
            for value in [object, string, symbol, bigint] {
                assert!(value.is_heap() && !value.is_number());
                assert_eq!(value.as_f64(), None);
            }
            assert_eq!(object.as_string(), None);
            assert_eq!(string.as_object(), None);
            assert_eq!(symbol.as_bigint(), None);
            assert_eq!(bigint.as_symbol(), None);
        }
    }
}
