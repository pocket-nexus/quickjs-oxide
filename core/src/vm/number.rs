//! Number conversions: `Number::toString(10)`, `StringToNumber`, `ToInt32`.

use crate::value::Value;

/// A number result: an `Int32` when the double is one (not `-0`), so integer
/// loops stay on the smi path.
#[inline]
pub(crate) fn number(value: f64) -> Value {
    let int = value as i32;
    if f64::from(int) == value && (int != 0 || value.is_sign_positive()) {
        Value::int32(int)
    } else {
        Value::number(value)
    }
}

/// `ToInt32` of a number.
#[inline]
pub(crate) fn to_int32(value: f64) -> i32 {
    if value.is_finite() && value.abs() < 2_147_483_648.0 {
        return value as i32;
    }
    to_uint32(value) as i32
}

/// `ToUint32` of a number.
pub(crate) fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let truncated = value.trunc();
    let modulo = truncated.rem_euclid(4_294_967_296.0);
    modulo as u32
}

/// `Number::toString(x)` in radix 10: a safe integer prints its digits;
/// anything else takes the fewest digits that round-trip, rounding an
/// exact tie to even as QuickJS does (Rust's formatter does not).
pub(crate) fn to_string(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        // `-0` prints as `0`.
        return (value as i64).to_string();
    }
    super::radix::to_string_radix(value, 10)
}

/// The JavaScript white space and line terminators `StringToNumber` trims.
pub(crate) fn is_space(unit: u16) -> bool {
    matches!(
        unit,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000
            ..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

/// `StringToNumber` over UTF-16 code units.
pub(crate) fn parse(units: &[u16]) -> f64 {
    let start = units.iter().position(|&unit| !is_space(unit));
    let Some(start) = start else {
        return 0.0;
    };
    let end = units.iter().rposition(|&unit| !is_space(unit)).unwrap() + 1;
    let Ok(text) = String::from_utf16(&units[start..end]) else {
        return f64::NAN;
    };
    parse_trimmed(&text)
}

fn parse_trimmed(text: &str) -> f64 {
    let bytes = text.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'0' {
        let radix = match bytes[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            let mut value = 0.0f64;
            for &byte in &bytes[2..] {
                let Some(digit) = (byte as char).to_digit(radix) else {
                    return f64::NAN;
                };
                value = value * f64::from(radix) + f64::from(digit);
            }
            return value;
        }
    }
    let (sign, unsigned) = match bytes.first() {
        Some(b'-') => (-1.0, &text[1..]),
        Some(b'+') => (1.0, &text[1..]),
        _ => (1.0, text),
    };
    if unsigned == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(unsigned.as_bytes()) {
        return f64::NAN;
    }
    sign * unsigned.parse::<f64>().unwrap_or(f64::NAN)
}

/// `StrUnsignedDecimalLiteral`: digits with an optional fraction and
/// exponent; Rust's parser alone would also take `inf` and `nan`.
fn is_decimal_literal(bytes: &[u8]) -> bool {
    let mut at = 0;
    let digits = |at: &mut usize| {
        let start = *at;
        while *at < bytes.len() && bytes[*at].is_ascii_digit() {
            *at += 1;
        }
        *at - start
    };
    let mut mantissa = digits(&mut at);
    if at < bytes.len() && bytes[at] == b'.' {
        at += 1;
        mantissa += digits(&mut at);
    }
    if mantissa == 0 {
        return false;
    }
    if at < bytes.len() && (bytes[at] == b'e' || bytes[at] == b'E') {
        at += 1;
        if at < bytes.len() && (bytes[at] == b'+' || bytes[at] == b'-') {
            at += 1;
        }
        if digits(&mut at) == 0 {
            return false;
        }
    }
    at == bytes.len()
}

/// `Number::exponentiate`: IEEE `pow` except where ECMA-262 differs.
pub(crate) fn pow(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) {
        return f64::NAN;
    }
    base.powf(exponent)
}

/// Decodes an IEEE-754 binary16 (QuickJS `cutils.h::fromfp16`): the half
/// payload placed in a binary64 and scaled by 2^1008, which keeps signed
/// zero, the infinities and the NaN sign.
pub(crate) fn from_float16_bits(value: u16) -> f64 {
    let mut magnitude = u32::from(value & 0x7fff);
    if magnitude >= 0x7c00 {
        magnitude += 0x1f_8000;
    }
    let bits =
        (u64::from(value >> 15) << 63) | (u64::from(magnitude) << (f64::MANTISSA_DIGITS - 11));
    const SCALE_2_POW_1008: f64 = f64::from_bits(0x7ef0_0000_0000_0000);
    f64::from_bits(bits) * SCALE_2_POW_1008
}

/// Encodes a binary64 as binary16 (QuickJS `cutils.h::tofp16`): nearest,
/// ties to even; overflow is infinity; NaN keeps its sign with payload 1.
pub(crate) fn to_float16_bits(value: f64) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 63) << 15) as u16;
    let mut magnitude = bits & 0x7fff_ffff_ffff_ffff;
    let encoded = if magnitude > 0x7ff0_0000_0000_0000 {
        0x7c01
    } else if magnitude < 0x3f10_0000_0000_0000 {
        if magnitude <= 0x3e60_0000_0000_0000 {
            0
        } else {
            let shift = (1051 - (magnitude >> 52)) as u32;
            magnitude = (1_u64 << 52) | (magnitude & ((1_u64 << 52) - 1));
            let addend = ((magnitude >> shift) & 1) + ((1_u64 << (shift - 1)) - 1);
            (magnitude + addend) >> shift
        }
    } else {
        magnitude -= 0x3f00_0000_0000_0000;
        let addend = ((magnitude >> (52 - 10)) & 1) + ((1_u64 << (52 - 11)) - 1);
        let rounded = (magnitude + addend) >> (52 - 10);
        if rounded > 0x7c00 { 0x7c00 } else { rounded }
    };
    encoded as u16 | sign
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_to_string_matches_ecma262() {
        for (value, text) in [
            (1.0, "1"),
            (-1.5, "-1.5"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123456789.125, "123456789.125"),
            (1.5e-7, "1.5e-7"),
            (0.000001, "0.000001"),
            (2f64.powi(53), "9007199254740992"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (-0.0, "0"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
            (0.1 + 0.2, "0.30000000000000004"),
        ] {
            assert_eq!(to_string(value), text, "{value}");
        }
    }

    #[test]
    fn string_to_number() {
        let parse = |text: &str| parse(&text.encode_utf16().collect::<Vec<_>>());
        assert_eq!(parse(""), 0.0);
        assert_eq!(parse("  42  "), 42.0);
        assert_eq!(parse("0x1F"), 31.0);
        assert_eq!(parse("-0x1F").to_bits(), f64::NAN.to_bits());
        assert_eq!(parse("1e3"), 1000.0);
        assert_eq!(parse(".5"), 0.5);
        assert_eq!(parse("5."), 5.0);
        assert_eq!(parse("-Infinity"), f64::NEG_INFINITY);
        assert!(parse("inf").is_nan());
        assert!(parse("1e").is_nan());
        assert!(parse("abc").is_nan());
        assert!(parse(".").is_nan());
    }

    #[test]
    fn int32_conversions() {
        assert_eq!(to_int32(4_294_967_296.0 + 5.0), 5);
        assert_eq!(to_int32(2_147_483_648.0), i32::MIN);
        assert_eq!(to_int32(-1.5), -1);
        assert_eq!(to_uint32(-1.0), u32::MAX);
        assert_eq!(to_int32(f64::NAN), 0);
        assert_eq!(number(-0.0), Value::number(-0.0));
        assert_eq!(number(3.0), Value::int32(3));
    }
}
