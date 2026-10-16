//! Differential test: the new core's `toString(radix)`, `parseInt` and
//! `parseFloat` (`oxide_core::vm::radix`) against the current engine's.

use crate::engine::value::JsString;
use crate::engine::value::number::to_string_radix as root_to_string_radix;
use crate::engine::value::number_parse::{
    parse_float as root_parse_float, parse_int as root_parse_int,
};
use oxide_core::vm::radix;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

fn doubles() -> Vec<f64> {
    let mut values: Vec<f64> = (-2000..2000).map(f64::from).collect();
    values.extend([
        0.0,
        -0.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::EPSILON,
        0.1,
        0.2,
        0.1 + 0.2,
        1.0 / 3.0,
        2.0 / 3.0,
        1e21,
        1e-7,
        1e-6,
        123_456_789.125,
        9_007_199_254_740_991.0,
        9_007_199_254_740_992.0,
        9_007_199_254_740_994.0,
        18_446_744_073_709_551_616.0,
        1.5e300,
        -2.5e-300,
    ]);
    // Every power of two, where the round-trip interval is asymmetric.
    for exponent in -1074..=1023 {
        values.push(2f64.powi(exponent));
        values.push(-2f64.powi(exponent) * 3.0);
    }
    let mut lcg = Lcg(0x243f_6a88_85a3_08d3);
    for _ in 0..400 {
        let bits = lcg.next();
        values.push(f64::from_bits(bits));
        // Moderate magnitudes and small fractions, the common runtime cases.
        let moderate = (lcg.next() >> 11) as f64 / (1u64 << 20) as f64;
        values.push(moderate);
        values.push(-moderate / 1e6);
        values.push(((lcg.next() >> 32) as f64) * 1024.0);
    }
    values
}

#[test]
fn to_string_radix_matches_the_current_engine() {
    let values = doubles();
    let mut checked = 0;
    for &value in &values {
        for radix in 2..=36 {
            let expected = root_to_string_radix(value, radix).unwrap();
            let actual = radix::to_string_radix(value, radix);
            assert_eq!(
                actual,
                expected,
                "{value:?} ({:#x}) radix {radix}",
                value.to_bits()
            );
            checked += 1;
        }
    }
    assert!(checked > 300_000);
}

fn parse_inputs() -> Vec<String> {
    let mut inputs: Vec<String> = [
        "",
        " ",
        "+",
        "-",
        "0",
        "-0",
        "+0",
        "00",
        "-000tail",
        "0x",
        "-0x",
        "0x10",
        "0X1f",
        "-0xFF",
        "+0x7fffffff",
        "0x10tail",
        "0b11",
        "0o17",
        "1e3",
        "1E-3",
        "1e",
        "1e+",
        "1e-",
        "1.e2",
        ".5e+2",
        ".",
        ".e2",
        "-.5",
        "+.5",
        "5.",
        "Infinity",
        "-Infinity",
        "+Infinitytail",
        "infinity",
        "NaN",
        "inf",
        "  42  ",
        "\t\n\u{b}\u{c}\r 7",
        "\u{a0}\u{feff}\u{3000}8",
        "\u{2028}9\u{2029}",
        "\u{85}1",
        "\u{180e}1",
        "zZ!",
        "10102",
        "123abc",
        "deadbeef",
        "DEADBEEF",
        "ffffffffffffffff",
        "10000000000000000",
        "1fffffffffffff",
        "20000000000001",
        "9007199254740993",
        "9007199254740995",
        "300000000000000031025361333325263798273",
        "1.0000000000000001110223024625156540423631668090820313",
        "2.4703282292062327e-324",
        "2.4703282292062328e-324",
        "1.7976931348623158e308",
        "1.7976931348623159e308",
        "-1e-9999",
        "1e400",
        "123.456e-7",
        "0.000001",
        "1e21",
        "1.5e+300",
        "3.141592653589793238462643383279502884197169399375105820974944",
        "1777777777777777777777",
        "11111111111111111111111111111111111111111111111111111111111111111",
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    ]
    .iter()
    .map(|text| (*text).to_owned())
    .collect();
    inputs.push(format!("1{}", "0".repeat(400)));
    inputs.push(format!("f{}", "f".repeat(300)));
    inputs.push(format!("1{}", "0".repeat(3000)));
    inputs.push(format!("0.{}1e10001", "0".repeat(10_000)));
    inputs.push(format!("1{}e-10000", "0".repeat(10_000)));
    let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyzABCXYZ.eE+- x";
    let mut lcg = Lcg(0x1319_8a2e_0370_7344);
    for _ in 0..3_000 {
        let length = (lcg.next() >> 59) as usize + 1;
        let mut text = String::new();
        if lcg.next() & 3 == 0 {
            text.push_str(["  ", "-", "+", "0x", "-0x", "\u{a0}"][(lcg.next() % 6) as usize]);
        }
        for _ in 0..length {
            // Mostly digits, so long numeric prefixes are common.
            let pick = lcg.next();
            let byte = if pick & 1 == 0 {
                b'0' + ((pick >> 8) % 10) as u8
            } else {
                alphabet[((pick >> 8) % alphabet.len() as u64) as usize]
            };
            text.push(char::from(byte));
        }
        inputs.push(text);
    }
    for _ in 0..1_000 {
        // Long digit strings around the retained-digit limits.
        let length = 10 + (lcg.next() % 60) as usize;
        let text: String = (0..length)
            .map(|_| char::from(b"0123456789abcdef"[(lcg.next() >> 60) as usize]))
            .collect();
        inputs.push(text);
        let value = f64::from_bits(lcg.next() >> 1);
        inputs.push(format!("{value:e}"));
        inputs.push(format!("{value}"));
    }
    inputs
}

fn same(actual: f64, expected: f64) -> bool {
    actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan())
}

#[test]
fn parsers_match_the_current_engine() {
    let inputs = parse_inputs();
    let mut checked = 0;
    for text in &inputs {
        let units: Vec<u16> = text.encode_utf16().collect();
        let string = JsString::try_from_utf16(units.iter().copied()).unwrap();
        let expected = root_parse_float(&string);
        let actual = radix::parse_float(&units);
        assert!(
            same(actual, expected),
            "parseFloat({text:?}): {actual:?} != {expected:?}"
        );
        for radix in [-1, 0, 1, 37].into_iter().chain(2..=36) {
            let expected = root_parse_int(&string, radix);
            let actual = radix::parse_int(&units, radix);
            assert!(
                same(actual, expected),
                "parseInt({text:?}, {radix}): {actual:?} != {expected:?}"
            );
            checked += 1;
        }
    }
    assert!(checked > 200_000);
}

#[test]
fn round_trips_through_the_core_match() {
    // toString(radix) output parsed back by parseInt in the same radix.
    for value in doubles() {
        if !value.is_finite() || value.trunc() != value || value.abs() >= 1e21 {
            continue;
        }
        for radix in 2..=36 {
            let text = radix::to_string_radix(value, radix);
            let units: Vec<u16> = text.encode_utf16().collect();
            let string = JsString::try_from_utf16(units.iter().copied()).unwrap();
            assert!(same(
                radix::parse_int(&units, radix as i32),
                root_parse_int(&string, radix as i32)
            ));
        }
    }
}
