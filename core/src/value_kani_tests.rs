// Phase 1: representation properties, not heap liveness or JS equality.
// Domains and trust boundary: docs/formal-verification.md.
use super::*;

#[kani::proof]
fn int32_round_trip() {
    let input: i32 = kani::any();
    let value = Value::int32(input);
    assert_eq!(value.kind(), Kind::Int32);
    assert_eq!(value.as_int32(), Some(input));
    assert_eq!(value.as_f64(), Some(f64::from(input)));
    assert!(value.is_number());
    assert!(value.is_int32());
    assert!(!value.is_double());
    assert!(!value.is_heap());
    assert_eq!(value.as_bool(), None);
    assert_eq!(value.as_object(), None);
    assert_eq!(value.as_string(), None);
    assert_eq!(value.as_symbol(), None);
    assert_eq!(value.as_bigint(), None);
}

#[kani::proof]
fn bool_round_trip() {
    let input: bool = kani::any();
    let value = Value::bool(input);
    assert_eq!(value.kind(), Kind::Bool);
    assert_eq!(value.as_bool(), Some(input));
    assert_eq!(value, if input { Value::TRUE } else { Value::FALSE });
    assert!(!value.is_number());
    assert!(!value.is_heap());
    assert_eq!(value.as_f64(), None);
    assert_eq!(value.as_int32(), None);
    assert_eq!(value.as_object(), None);
    assert_eq!(value.as_string(), None);
    assert_eq!(value.as_symbol(), None);
    assert_eq!(value.as_bigint(), None);
    assert!(!value.is_empty());
    assert!(!value.is_null());
    assert!(!value.is_undefined());
}

#[kani::proof]
fn special_values_are_distinct() {
    let selector: u8 = kani::any();
    kani::assume(selector < 3);
    let (value, kind) = match selector {
        0 => (Value::UNDEFINED, Kind::Undefined),
        1 => (Value::NULL, Kind::Null),
        _ => (Value::EMPTY, Kind::Empty),
    };
    assert_eq!(value.kind(), kind);
    assert_eq!(value.is_undefined(), selector == 0);
    assert_eq!(value.is_null(), selector == 1);
    assert_eq!(value.is_empty(), selector == 2);
    assert_eq!(value.as_bool(), None);
    assert_eq!(value.as_f64(), None);
    assert_eq!(value.as_int32(), None);
    assert!(!value.is_number());
    assert!(!value.is_heap());
}

#[kani::proof]
fn heap_tags_and_payloads_round_trip() {
    let index: u32 = kani::any();
    let selector: u8 = kani::any();
    kani::assume(selector < 4);
    let (value, kind) = match selector {
        0 => (Value::object(ObjectId(index)), Kind::Object),
        1 => (Value::string(StringId(index)), Kind::String),
        2 => (Value::symbol(SymbolId(index)), Kind::Symbol),
        _ => (Value::bigint(BigIntId(index)), Kind::BigInt),
    };
    assert_eq!(value.kind(), kind);
    assert_eq!(
        value.as_object(),
        if selector == 0 {
            Some(ObjectId(index))
        } else {
            None
        }
    );
    assert_eq!(
        value.as_string(),
        if selector == 1 {
            Some(StringId(index))
        } else {
            None
        }
    );
    assert_eq!(
        value.as_symbol(),
        if selector == 2 {
            Some(SymbolId(index))
        } else {
            None
        }
    );
    assert_eq!(
        value.as_bigint(),
        if selector == 3 {
            Some(BigIntId(index))
        } else {
            None
        }
    );
    assert!(value.is_heap());
    assert!(!value.is_number());
    assert_eq!(value.as_f64(), None);
    assert_eq!(value.as_int32(), None);
    assert_eq!(value.as_bool(), None);
    assert!(!value.is_empty());
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert_eq!(ScopeId(index).object().index(), index);
}

#[kani::proof]
fn number_bits_and_classification() {
    // All IEEE-754 bit patterns, including signalling NaNs and signed zero.
    let bits: u64 = kani::any();
    let input = f64::from_bits(bits);
    let value = Value::number(input);
    // These literals specify the representation independently of the private
    // production tag constants. No arithmetic NaN payload is specified here.
    let expected = if bits >= 0xfff9_0000_0000_0000 {
        0x7ff8_0000_0000_0000
    } else {
        bits
    };
    assert_eq!(value.bits(), expected);
    assert_eq!(value.as_f64().unwrap().to_bits(), expected);
    assert_eq!(value.kind(), Kind::Number);
    assert!(value.is_double());
    assert!(value.is_number());
    assert!(!value.is_int32());
    assert!(!value.is_heap());
    assert_eq!(value.as_int32(), None);
    assert_eq!(value.as_bool(), None);
    assert_eq!(value.as_object(), None);
    assert_eq!(value.as_string(), None);
    assert_eq!(value.as_symbol(), None);
    assert_eq!(value.as_bigint(), None);
    assert!(!value.is_empty());
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    let nan =
        bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000 && bits & 0x000f_ffff_ffff_ffff != 0;
    assert_eq!(value.as_f64().unwrap().is_nan(), nan);
}
