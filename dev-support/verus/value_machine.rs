#[kani::proof]
fn number_round_trip() {
    let input = kani::any::<u64>();
    let exponent = (input >> 52) & 0x7ff;
    let mantissa = input & ((1u64 << 52) - 1);
    let expected = if exponent == 0x7ff && mantissa != 0 && input >= 0xfff9_0000_0000_0000 {
        0x7ff8_0000_0000_0000
    } else {
        input
    };
    let boxed = Value::number(f64::from_bits(input));
    assert_eq!(boxed.bits(), expected);
    assert_eq!(boxed.kind(), Kind::Number);
    assert_eq!(boxed.as_f64().unwrap().to_bits(), expected);
}

// Encode the mathematical value of a signed 32-bit integer into binary64.
// All 32 significand bits fit exactly, including the magnitude of i32::MIN.
fn int32_ieee_bits(value: i32) -> u64 {
    if value == 0 { return 0; }
    let magnitude = u64::from(value.unsigned_abs());
    let highest = 63 - magnitude.leading_zeros();
    let fraction = (magnitude ^ (1u64 << highest)) << (52 - highest);
    ((u64::from(value < 0)) << 63) | (u64::from(highest + 1023) << 52) | fraction
}

#[kani::proof]
fn raw_numeric_projection() {
    let bits = kani::any::<u64>();
    let projection = Value(bits).as_f64().map(f64::to_bits);
    let expected = match bits >> 48 {
        0..=0xfff8 => Some(bits),
        0xfff9 => Some(int32_ieee_bits(bits as u32 as i32)),
        _ => None,
    };
    assert_eq!(projection, expected);
}

#[kani::proof]
fn boxed_observations() {
    let b: bool = kani::any();
    let value = Value::bool(b);
    assert_eq!(value.as_bool(), Some(b));
    assert_eq!(value.kind(), Kind::Bool);
    assert!(value.as_f64().is_none());
    for (v, k) in [(Value::EMPTY, Kind::Empty), (Value::NULL, Kind::Null),
                   (Value::UNDEFINED, Kind::Undefined)] {
        assert_eq!(v.kind(), k);
        assert_eq!(v.is_empty(), k == Kind::Empty);
        assert_eq!(v.is_undefined(), k == Kind::Undefined);
        assert_eq!(v.is_null(), k == Kind::Null);
        assert!(v.as_f64().is_none() && !v.is_heap());
    }
    let i: u32 = kani::any();
    assert_eq!(ScopeId(i).object().index(), i);
    for v in [Value::object(ObjectId(i)), Value::string(StringId(i)),
              Value::symbol(SymbolId(i)), Value::bigint(BigIntId(i))] {
        assert!(v.is_heap() && v.as_f64().is_none() && v.as_bool().is_none());
    }
}

#[kani::proof]
fn every_raw_tag_and_reader() {
    let bits: u64 = kani::any();
    let tag = bits >> 48;
    let low = bits as u32;
    let v = Value(bits);
    let kind = match tag {
        0xfff9 => Kind::Int32,
        0xfffa => match bits & 3 {
            0 => Kind::Bool, 1 => Kind::Undefined, 2 => Kind::Null, _ => Kind::Empty,
        },
        0xfffb => Kind::Object, 0xfffc => Kind::String,
        0xfffd => Kind::Symbol, 0xfffe => Kind::BigInt,
        _ => Kind::Number,
    };
    assert_eq!(v.kind(), kind);
    assert_eq!(v.is_number(), tag <= 0xfff9);
    assert_eq!(v.is_double(), tag <= 0xfff8);
    assert_eq!(v.is_int32(), tag == 0xfff9);
    assert_eq!(v.is_heap(), (0xfffb..=0xfffe).contains(&tag));
    assert_eq!(v.is_undefined(), bits == 0xfffa_0000_0000_0001);
    assert_eq!(v.is_null(), bits == 0xfffa_0000_0000_0002);
    assert_eq!(v.is_empty(), bits == 0xfffa_0000_0000_0003);
    assert_eq!(v.as_bool(), match bits {
        0xfffa_0000_0000_0000 => Some(false),
        0xfffa_0000_0000_0004 => Some(true),
        _ => None,
    });
    assert_eq!(v.as_int32(), (tag == 0xfff9).then_some(low as i32));
    assert_eq!(v.as_object().map(ObjectId::index), (tag == 0xfffb).then_some(low));
    assert_eq!(v.as_string().map(StringId::index), (tag == 0xfffc).then_some(low));
    assert_eq!(v.as_symbol().map(SymbolId::index), (tag == 0xfffd).then_some(low));
    assert_eq!(v.as_bigint().map(BigIntId::index), (tag == 0xfffe).then_some(low));
}
