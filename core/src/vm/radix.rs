//! `Number.prototype.toString(radix)`, `parseInt` and `parseFloat` (phase 3,
//! section 6.2), ported from the current engine's `value/number/format.rs`
//! and `value/number_parse.rs` with their QuickJS-exact rules: the FREE
//! formatter keeps the shortest digits that round-trip after rounding to
//! nearest, ties to even, and the parsers keep only QuickJS's
//! `atod_max_digits` significant digits.
//!
//! The common cases stay on plain machine arithmetic: a power-of-two radix
//! prints the binary digits exactly, an integer below 2^53 prints from a
//! `u64`, and a digit string that fits a `u64` parses without [`Big`]. The
//! rest runs the reference algorithm on exact rationals whose denominators
//! are `2^a * radix^b`, so every division is a shift or a run of one-limb
//! divisions.

use std::cmp::Ordering;

use super::bigint::{Big, BigError};
use super::number::is_space;

// QuickJS `dtoa_max_digits_table`, indexed by `radix - 2`: enough digits in
// that radix for any binary64 value to round-trip.
const DTOA_MAX_DIGITS: [i32; 35] = [
    54, 35, 28, 24, 22, 20, 19, 18, 17, 17, 16, 16, 15, 15, 15, 14, 14, 14, 14, 14, 13, 13, 13, 13,
    13, 13, 13, 12, 12, 12, 12, 12, 12, 12, 12,
];

// QuickJS `atod_max_digits_table`, indexed by `radix - 2`: the significant
// digits the parser keeps. A power-of-two radix folds the discarded digits
// into a sticky bit; any other radix truncates them, as the pinned release
// does.
const ATOD_MAX_DIGITS: [usize; 35] = [
    64, 80, 32, 55, 49, 45, 21, 40, 38, 37, 35, 34, 33, 32, 16, 31, 30, 30, 29, 29, 28, 28, 27, 27,
    27, 26, 26, 26, 26, 25, 12, 25, 25, 24, 24,
];

const EXPONENT_LIMIT: i64 = 10_000;

const DIGIT_CHARS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// `Number.prototype.toString(radix)` for a radix in `2..=36` (the caller
/// checks the range). Radix 10 switches to exponential notation at QuickJS's
/// thresholds; every other radix prints positionally.
pub fn to_string_radix(value: f64, radix: u32) -> String {
    debug_assert!((2..=36).contains(&radix));
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if value == 0.0 {
        return "0".into();
    }
    let magnitude = value.abs();
    let body = if radix.is_power_of_two() {
        power_of_two_digits(magnitude, radix)
    } else if magnitude < 9_007_199_254_740_992.0 && magnitude.trunc() == magnitude {
        // Every integer below 2^53 is a double, so no shorter digit string
        // can round-trip: the shortest digits are the exact ones.
        u64_digits(magnitude as u64, radix)
    } else if radix == 10 {
        place_free(decimal_sequence(magnitude), 10)
    } else {
        free_big(magnitude, radix)
    };
    if value < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

/// `parseInt` over the result of `ToString`, with `radix` already through
/// `ToInt32`. An invalid non-zero radix is `NaN`.
pub fn parse_int(units: &[u16], radix: i32) -> f64 {
    if radix != 0 && !(2..=36).contains(&radix) {
        return f64::NAN;
    }
    let mut cursor = skip_space(units);
    let negative = match units.get(cursor) {
        Some(&unit) if unit == u16::from(b'+') => {
            cursor += 1;
            false
        }
        Some(&unit) if unit == u16::from(b'-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let prefixable = radix == 0 || radix == 16;
    let mut radix = if radix == 0 { 10 } else { radix as u32 };
    if prefixable
        && units.get(cursor) == Some(&u16::from(b'0'))
        && matches!(units.get(cursor + 1), Some(&unit) if unit == u16::from(b'x') || unit == u16::from(b'X'))
    {
        radix = 16;
        cursor += 2;
    }
    let start = cursor;
    while units
        .get(cursor)
        .and_then(|&unit| digit_value(unit))
        .is_some_and(|digit| digit < radix)
    {
        cursor += 1;
    }
    if cursor == start {
        return f64::NAN;
    }
    let number = radix_integer_to_f64(&units[start..cursor], radix);
    if negative { -number } else { number }
}

/// `parseFloat` over the result of `ToString`: the longest decimal or
/// `Infinity` prefix, keeping QuickJS's 38 significant decimal digits.
pub fn parse_float(units: &[u16]) -> f64 {
    let mut cursor = skip_space(units);
    let negative = match units.get(cursor) {
        Some(&unit) if unit == u16::from(b'+') => {
            cursor += 1;
            false
        }
        Some(&unit) if unit == u16::from(b'-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    if has_prefix(&units[cursor..], b"Infinity") {
        return if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }

    let mantissa_start = cursor;
    while units.get(cursor).copied().is_some_and(is_digit) {
        cursor += 1;
    }
    let integer_digits = cursor - mantissa_start;
    let mut fractional_digits = 0;
    if units.get(cursor) == Some(&u16::from(b'.')) {
        cursor += 1;
        let fraction_start = cursor;
        while units.get(cursor).copied().is_some_and(is_digit) {
            cursor += 1;
        }
        fractional_digits = cursor - fraction_start;
    }
    if integer_digits == 0 && fractional_digits == 0 {
        return f64::NAN;
    }
    let mantissa_end = cursor;

    // The exponent belongs to the prefix only with at least one digit.
    let mut explicit_exponent = 0;
    if matches!(units.get(cursor), Some(&unit) if unit == u16::from(b'e') || unit == u16::from(b'E'))
    {
        cursor += 1;
        let exponent_negative = match units.get(cursor) {
            Some(&unit) if unit == u16::from(b'+') => {
                cursor += 1;
                false
            }
            Some(&unit) if unit == u16::from(b'-') => {
                cursor += 1;
                true
            }
            _ => false,
        };
        let exponent_start = cursor;
        while units.get(cursor).copied().is_some_and(is_digit) {
            cursor += 1;
        }
        if cursor != exponent_start {
            // Keep enough exponent to cancel any decimal-point offset in the
            // input; only the combined scale is clamped.
            let limit = i64::try_from(units.len())
                .unwrap_or(i64::MAX - EXPONENT_LIMIT)
                .saturating_add(EXPONENT_LIMIT);
            let mut exponent = 0i64;
            for &unit in &units[exponent_start..cursor] {
                exponent = exponent
                    .saturating_mul(10)
                    .saturating_add(i64::from(unit - u16::from(b'0')))
                    .min(limit);
            }
            explicit_exponent = if exponent_negative {
                -exponent
            } else {
                exponent
            };
        }
    }

    let digits = || {
        units[mantissa_start..mantissa_end]
            .iter()
            .copied()
            .filter(|&unit| is_digit(unit))
    };
    let Some(significant_start) = digits().position(|unit| unit != u16::from(b'0')) else {
        return if negative { -0.0 } else { 0.0 };
    };
    let mut text = String::with_capacity(48);
    let mut retained = 0i64;
    for unit in digits().skip(significant_start).take(ATOD_MAX_DIGITS[8]) {
        text.push(char::from(unit as u8));
        retained += 1;
    }

    // The retained digits M stand for M * 10^scale.
    let scale = explicit_exponent
        .saturating_add(integer_digits as i64)
        .saturating_sub(significant_start as i64)
        .saturating_sub(retained)
        .clamp(-EXPONENT_LIMIT, EXPONENT_LIMIT);
    text.push('e');
    text.push_str(&scale.to_string());
    let number = text.parse::<f64>().unwrap_or(f64::NAN);
    if negative { -number } else { number }
}

/// A non-zero finite magnitude in a power-of-two radix. Every candidate
/// with fewer digits than the exact expansion is another double, so the
/// shortest round-trip digits are the exact binary digits.
fn power_of_two_digits(value: f64, radix: u32) -> String {
    let (mut mantissa, mut exponent) = decompose(value);
    let zeros = mantissa.trailing_zeros();
    mantissa >>= zeros;
    exponent += zeros as i32;
    let width = radix.trailing_zeros() as i32;
    let low = exponent.div_euclid(width);
    // At most 53 + 4 bits.
    let scaled = mantissa << (exponent - low * width);
    let digits = u64_digits(scaled, radix);
    if low >= 0 {
        let mut out = digits;
        out.extend(std::iter::repeat_n('0', low as usize));
        return out;
    }
    let fraction = (-low) as usize;
    if digits.len() > fraction {
        let mut out = digits;
        out.insert(out.len() - fraction, '.');
        out
    } else {
        let mut out = String::with_capacity(fraction + 2);
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', fraction - digits.len()));
        out.push_str(&digits);
        out
    }
}

fn u64_digits(mut value: u64, radix: u32) -> String {
    let mut buffer = [0u8; 64];
    let mut at = buffer.len();
    loop {
        at -= 1;
        buffer[at] = DIGIT_CHARS[(value % u64::from(radix)) as usize];
        value /= u64::from(radix);
        if value == 0 {
            break;
        }
    }
    buffer[at..].iter().map(|&byte| byte as char).collect()
}

/// The significand and binary exponent of a finite non-negative double.
fn decompose(value: f64) -> (u64, i32) {
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7FF) as i32;
    let fraction = bits & ((1 << 52) - 1);
    if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    }
}

/// `mantissa * radix^(exponent - digits)`: `exponent` places the radix point
/// counted from the first digit.
struct DigitSequence {
    mantissa: Big,
    digits: i32,
    exponent: i32,
}

/// The reference FREE formatter on exact rationals.
fn free_big(value: f64, radix: u32) -> String {
    let sequence = shortest_sequence(value, radix);
    let digits = padded_digits(&sequence, radix);
    place_free((digits, sequence.exponent), radix)
}

/// [`shortest_sequence`] in radix 10 without big integers: Rust's
/// fixed-precision formatting rounds the exact value to nearest-even, which
/// is the rounding QuickJS applies at each digit count, so the walk down
/// from 17 digits is a format and a parse per count. Returns the digits
/// (trailing zeros removed) and the decimal point's position.
///
/// Where the nearest stops round-tripping, a sequence of that count may
/// still at a power of two: below one the rounding interval is half the
/// one above, so the neighbor above can hit where the nearest misses
/// (QuickJS stops there: `2**-24` prints 17 digits, 16 round-trip).
/// `Number::toString` takes the fewest digits that round-trip (6.1.6.1.20
/// step 5), which Rust's shortest formatting gives; QuickJS's choice stands
/// when it is as short. Elsewhere the interval is symmetric, so the
/// farther neighbor misses whenever the nearest does.
fn decimal_sequence(value: f64) -> (String, i32) {
    let power_of_two = value.to_bits() & ((1 << 52) - 1) == 0;
    let split = |text: &str| {
        let (mantissa, exponent) = text.split_once('e').expect("an exponent");
        let exponent: i32 = exponent.parse().expect("a decimal exponent");
        let mut kept: String = mantissa.chars().filter(|c| *c != '.').collect();
        while kept.len() > 1 && kept.ends_with('0') {
            kept.pop();
        }
        (kept, exponent + 1)
    };
    let mut best: Option<(String, i32)> = None;
    let mut digits = DTOA_MAX_DIGITS[8];
    loop {
        let text = format!("{:.*e}", (digits - 1) as usize, value);
        if best.is_some() && text.parse::<f64>() != Ok(value) {
            if power_of_two {
                let shortest = split(&format!("{value:e}"));
                if shortest.0.len() as i32 <= digits {
                    best = Some(shortest);
                }
            }
            break;
        }
        let sequence = split(&text);
        digits = sequence.0.len() as i32;
        best = Some(sequence);
        if digits == 1 {
            break;
        }
        digits -= 1;
    }
    best.expect("17 digits always round-trip")
}

/// Places FREE-format digits: radix 10 switches to exponential notation
/// outside QuickJS's thresholds; other radices never do.
fn place_free((digits, exponent): (String, i32), radix: u32) -> String {
    let sequence_exponent = exponent;
    let exponential =
        radix == 10 && (sequence_exponent <= -6 || sequence_exponent > DTOA_MAX_DIGITS[8] + 4);
    if exponential {
        let mut out = String::with_capacity(digits.len() + 8);
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exponent = sequence_exponent - 1;
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.abs().to_string());
        return out;
    }
    let exponent = sequence_exponent;
    if exponent <= 0 {
        let mut out = String::with_capacity(2 + (-exponent) as usize + digits.len());
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-exponent) as usize));
        out.push_str(&digits);
        out
    } else if exponent as usize >= digits.len() {
        let mut out = digits;
        let zeros = exponent as usize - out.len();
        out.extend(std::iter::repeat_n('0', zeros));
        out
    } else {
        let mut out = digits;
        out.insert(exponent as usize, '.');
        out
    }
}

/// Walks down from QuickJS's maximum digit count, rounding the exact value
/// to nearest-even each time, and keeps the last count that round-trips.
fn shortest_sequence(value: f64, radix: u32) -> DigitSequence {
    let bits = value.to_bits();
    let (significand, binary_exponent) = decompose(value);
    let base_exponent = radix_exponent(significand, binary_exponent, radix, value);
    let mut digits = DTOA_MAX_DIGITS[(radix - 2) as usize];
    let mut best: Option<DigitSequence> = None;
    let radix_big = Big::from_u64(u64::from(radix));
    loop {
        let mut exponent = base_exponent;
        let mut mantissa;
        loop {
            mantissa = round_scaled(significand, binary_exponent, radix, digits - exponent);
            if mantissa.cmp(&radix_power(radix, digits)) == Ordering::Less {
                break;
            }
            exponent += 1;
        }
        loop {
            let (quotient, remainder) = must_pair(mantissa.div_rem(&radix_big));
            if !remainder.is_zero() {
                break;
            }
            mantissa = quotient;
            digits -= 1;
        }
        let candidate = DigitSequence {
            mantissa,
            digits,
            exponent,
        };
        if best.is_some() && sequence_to_f64_bits(&candidate, radix) != bits {
            break;
        }
        best = Some(candidate);
        if digits == 1 {
            break;
        }
        digits -= 1;
    }
    best.expect("the QuickJS maximum digit table guarantees a round trip")
}

/// The `e` with `radix^(e-1) <= value < radix^e`.
fn radix_exponent(significand: u64, binary_exponent: i32, radix: u32, approximate: f64) -> i32 {
    let (numerator, shift) = fraction(significand, binary_exponent);
    let below = |exponent: i32| -> bool {
        // value < radix^exponent
        if exponent >= 0 {
            let bound = shl(&radix_power(radix, exponent), shift);
            numerator.cmp(&bound) == Ordering::Less
        } else {
            let scaled = must(numerator.mul(&radix_power(radix, -exponent)));
            scaled.cmp(&shl(&Big::from_u64(1), shift)) == Ordering::Less
        }
    };
    let mut exponent = approximate.log(f64::from(radix)).floor() as i32 + 1;
    while !below(exponent) {
        exponent += 1;
    }
    while below(exponent - 1) {
        exponent -= 1;
    }
    exponent
}

/// The value as `numerator / 2^shift`.
fn fraction(significand: u64, binary_exponent: i32) -> (Big, u64) {
    let significand = Big::from_u64(significand);
    if binary_exponent >= 0 {
        (shl(&significand, binary_exponent as u64), 0)
    } else {
        (significand, u64::from(binary_exponent.unsigned_abs()))
    }
}

/// `value * radix^scale`, rounded to the nearest integer, ties to even.
fn round_scaled(significand: u64, binary_exponent: i32, radix: u32, scale: i32) -> Big {
    let (mut numerator, shift) = fraction(significand, binary_exponent);
    if scale >= 0 {
        numerator = must(numerator.mul(&radix_power(radix, scale)));
        round_ratio(&numerator, &Divisor::new(radix, shift, 0))
    } else {
        round_ratio(
            &numerator,
            &Divisor::new(radix, shift, scale.unsigned_abs()),
        )
    }
}

/// `2^shift * radix^power`, kept factored so that division is a shift and a
/// run of one-limb divisions.
struct Divisor {
    radix: u32,
    shift: u64,
    power: u32,
    value: Big,
}

impl Divisor {
    fn new(radix: u32, shift: u64, power: u32) -> Divisor {
        let value = shl(&radix_power(radix, power as i32), shift);
        Divisor {
            radix,
            shift,
            power,
            value,
        }
    }

    fn floor_div(&self, numerator: &Big) -> Big {
        let mut quotient = shr(numerator, self.shift);
        let (chunk_digits, chunk) = radix_chunk(self.radix);
        let mut left = self.power;
        while left > 0 && !quotient.is_zero() {
            let step = left.min(chunk_digits);
            let divisor = if step == chunk_digits {
                chunk
            } else {
                u64::from(self.radix).pow(step)
            };
            quotient = must_pair(quotient.div_rem(&Big::from_u64(divisor))).0;
            left -= step;
        }
        quotient
    }
}

/// `numerator / divisor` to the nearest integer, ties to even.
fn round_ratio(numerator: &Big, divisor: &Divisor) -> Big {
    let quotient = divisor.floor_div(numerator);
    let remainder = must(numerator.sub(&must(quotient.mul(&divisor.value))));
    let odd = quotient.limbs.first().is_some_and(|&limb| limb & 1 == 1);
    match shl(&remainder, 1).cmp(&divisor.value) {
        Ordering::Greater => must(quotient.add(&Big::from_u64(1))),
        Ordering::Equal if odd => must(quotient.add(&Big::from_u64(1))),
        _ => quotient,
    }
}

/// The largest `radix^k` that fits a `u64`, and `k`.
fn radix_chunk(radix: u32) -> (u32, u64) {
    let mut digits = 0;
    let mut power = 1u64;
    while let Some(next) = power.checked_mul(u64::from(radix)) {
        power = next;
        digits += 1;
    }
    (digits, power)
}

fn sequence_to_f64_bits(sequence: &DigitSequence, radix: u32) -> u64 {
    let exponent = sequence.exponent - sequence.digits;
    if exponent >= 0 {
        let exact = must(sequence.mantissa.mul(&radix_power(radix, exponent)));
        return exact.to_f64().to_bits();
    }
    rational_to_f64_bits(&sequence.mantissa, radix, exponent.unsigned_abs())
}

/// `numerator / radix^power` rounded to binary64, ties to even.
fn rational_to_f64_bits(numerator: &Big, radix: u32, power: u32) -> u64 {
    if numerator.is_zero() {
        return 0;
    }
    let denominator = radix_power(radix, power as i32);
    let mut exponent = bit_length(numerator) as i32 - bit_length(&denominator) as i32;
    let below_guess = if exponent >= 0 {
        numerator.cmp(&shl(&denominator, exponent as u64)) == Ordering::Less
    } else {
        shl(numerator, u64::from(exponent.unsigned_abs())).cmp(&denominator) == Ordering::Less
    };
    if below_guess {
        exponent -= 1;
    }
    if exponent < -1022 {
        let rounded = round_ratio(&shl(numerator, 1074), &Divisor::new(radix, 0, power));
        return rounded.limbs.first().copied().unwrap_or(0);
    }
    if exponent > 1023 {
        return f64::INFINITY.to_bits();
    }
    let shift = 52 - exponent;
    let mut significand = if shift >= 0 {
        round_ratio(
            &shl(numerator, shift as u64),
            &Divisor::new(radix, 0, power),
        )
    } else {
        round_ratio(
            numerator,
            &Divisor::new(radix, u64::from(shift.unsigned_abs()), power),
        )
    };
    if bit_length(&significand) > 53 {
        significand = shr(&significand, 1);
        exponent += 1;
        if exponent > 1023 {
            return f64::INFINITY.to_bits();
        }
    }
    let significand = significand.limbs.first().copied().unwrap_or(0);
    ((exponent + 1023) as u64) << 52 | (significand & ((1 << 52) - 1))
}

fn padded_digits(sequence: &DigitSequence, radix: u32) -> String {
    let digits = big_digits(&sequence.mantissa, radix);
    let requested = sequence.digits as usize;
    if digits.len() >= requested {
        return digits;
    }
    let mut padded = String::with_capacity(requested);
    padded.extend(std::iter::repeat_n('0', requested - digits.len()));
    padded.push_str(&digits);
    padded
}

/// The digits of a non-negative integer, by one-limb divisions.
fn big_digits(value: &Big, radix: u32) -> String {
    if value.limbs.len() <= 1 {
        return u64_digits(value.limbs.first().copied().unwrap_or(0), radix);
    }
    let (chunk_digits, chunk) = radix_chunk(radix);
    let chunk_big = Big::from_u64(chunk);
    let mut chunks = Vec::new();
    let mut rest = value.clone();
    while rest.limbs.len() > 1 {
        let (quotient, remainder) = must_pair(rest.div_rem(&chunk_big));
        chunks.push(remainder.limbs.first().copied().unwrap_or(0));
        rest = quotient;
    }
    let mut out = u64_digits(rest.limbs.first().copied().unwrap_or(0), radix);
    for &chunk in chunks.iter().rev() {
        let digits = u64_digits(chunk, radix);
        out.extend(std::iter::repeat_n(
            '0',
            chunk_digits as usize - digits.len(),
        ));
        out.push_str(&digits);
    }
    out
}

/// The digits of `parseInt`'s longest prefix, as one rounding to binary64
/// of QuickJS's retained mantissa.
fn radix_integer_to_f64(digits: &[u16], radix: u32) -> f64 {
    let Some(start) = digits.iter().position(|&unit| unit != u16::from(b'0')) else {
        return 0.0;
    };
    let significant = &digits[start..];
    let max_digits = ATOD_MAX_DIGITS[(radix - 2) as usize];
    if significant.len() <= max_digits {
        // Fast path: a value that fits a `u64` converts with one rounding.
        let mut value = 0u64;
        let fits = significant.iter().all(|&unit| {
            let digit = u64::from(digit_value(unit).unwrap_or(0));
            match value
                .checked_mul(u64::from(radix))
                .and_then(|value| value.checked_add(digit))
            {
                Some(next) => {
                    value = next;
                    true
                }
                None => false,
            }
        });
        if fits {
            return value as f64;
        }
    }

    let retained_count = significant.len().min(max_digits);
    let mut retained = Big::zero();
    let (chunk_digits, _) = radix_chunk(radix);
    for chunk in significant[..retained_count].chunks(chunk_digits as usize) {
        let mut value = 0u64;
        for &unit in chunk {
            value = value * u64::from(radix) + u64::from(digit_value(unit).unwrap_or(0));
        }
        let scale = Big::from_u64(u64::from(radix).pow(chunk.len() as u32));
        retained = must(must(retained.mul(&scale)).add(&Big::from_u64(value)));
    }
    let discarded = &significant[retained_count..];
    if radix.is_power_of_two() && discarded.iter().any(|&unit| unit != u16::from(b'0')) {
        retained = retained.bit_or(&Big::from_u64(1));
    }
    // Far past binary64 overflow; avoid a power the size of the input.
    if discarded.len() > 2_048 {
        return f64::INFINITY;
    }
    if !discarded.is_empty() {
        retained = must(retained.mul(&radix_power(radix, discarded.len() as i32)));
    }
    retained.to_f64()
}

fn radix_power(radix: u32, exponent: i32) -> Big {
    debug_assert!(exponent >= 0);
    must(Big::from_u64(u64::from(radix)).pow(&Big::from_i64(i64::from(exponent))))
}

fn shl(value: &Big, shift: u64) -> Big {
    must(value.shl(&Big::from_u64(shift)))
}

fn shr(value: &Big, shift: u64) -> Big {
    must(value.shl(&Big::from_u64(shift).neg()))
}

fn bit_length(value: &Big) -> u64 {
    match value.limbs.last() {
        None => 0,
        Some(&top) => value.limbs.len() as u64 * 64 - u64::from(top.leading_zeros()),
    }
}

/// Every operand here is a few thousand bits at most, far below the
/// BigInt size limit, and no divisor is zero.
fn must(result: Result<Big, BigError>) -> Big {
    match result {
        Ok(value) => value,
        Err(_) => unreachable!("radix arithmetic stays within the BigInt limits"),
    }
}

fn must_pair(result: Result<(Big, Big), BigError>) -> (Big, Big) {
    match result {
        Ok(pair) => pair,
        Err(_) => unreachable!("radix arithmetic never divides by zero"),
    }
}

fn skip_space(units: &[u16]) -> usize {
    units
        .iter()
        .position(|&unit| !is_space(unit))
        .unwrap_or(units.len())
}

fn has_prefix(units: &[u16], prefix: &[u8]) -> bool {
    units.len() >= prefix.len()
        && units
            .iter()
            .zip(prefix)
            .all(|(&unit, &byte)| unit == u16::from(byte))
}

fn is_digit(unit: u16) -> bool {
    (0x30..=0x39).contains(&unit)
}

fn digit_value(unit: u16) -> Option<u32> {
    match unit {
        0x30..=0x39 => Some(u32::from(unit - 0x30)),
        0x61..=0x7A => Some(u32::from(unit - 0x61) + 10),
        0x41..=0x5A => Some(u32::from(unit - 0x41) + 10),
        _ => None,
    }
}

// ---- toFixed, toExponential, toPrecision (phase 4, section 6.1 row 5) ----

/// The largest digit count `toFixed`, `toExponential` and `toPrecision`
/// accept.
pub const MAX_DIGITS: i32 = 100;

/// `value * radix^scale`, rounded to the nearest integer, ties away from
/// zero (QuickJS's `JS_RNDNA` for fixed digit counts).
fn round_scaled_away(significand: u64, binary_exponent: i32, scale: i32) -> Big {
    let (mut numerator, shift) = fraction(significand, binary_exponent);
    let divisor = if scale >= 0 {
        numerator = must(numerator.mul(&radix_power(10, scale)));
        Divisor::new(10, shift, 0)
    } else {
        Divisor::new(10, shift, scale.unsigned_abs())
    };
    let quotient = divisor.floor_div(&numerator);
    let remainder = must(numerator.sub(&must(quotient.mul(&divisor.value))));
    if shl(&remainder, 1).cmp(&divisor.value) != Ordering::Less {
        must(quotient.add(&Big::from_u64(1)))
    } else {
        quotient
    }
}

/// `digits` significant decimal digits of a non-negative finite value,
/// rounded ties away.
fn fixed_significant(value: f64, digits: i32) -> DigitSequence {
    if value == 0.0 {
        return DigitSequence {
            mantissa: Big::zero(),
            digits,
            exponent: 1,
        };
    }
    let (significand, binary_exponent) = decompose(value);
    let mut exponent = radix_exponent(significand, binary_exponent, 10, value);
    loop {
        let mantissa = round_scaled_away(significand, binary_exponent, digits - exponent);
        if mantissa.cmp(&radix_power(10, digits)) == Ordering::Less {
            return DigitSequence {
                mantissa,
                digits,
                exponent,
            };
        }
        exponent += 1;
    }
}

fn non_finite(value: f64) -> Option<String> {
    if value.is_nan() {
        Some("NaN".into())
    } else if value == f64::INFINITY {
        Some("Infinity".into())
    } else if value == f64::NEG_INFINITY {
        Some("-Infinity".into())
    } else {
        None
    }
}

fn signed(body: String, negative: bool) -> String {
    if negative { format!("-{body}") } else { body }
}

fn exponential(sequence: &DigitSequence) -> String {
    let digits = padded_digits(sequence, 10);
    let mut out = String::with_capacity(digits.len() + 8);
    out.push_str(&digits[..1]);
    if digits.len() > 1 {
        out.push('.');
        out.push_str(&digits[1..]);
    }
    let exponent = sequence.exponent - 1;
    out.push('e');
    out.push(if exponent < 0 { '-' } else { '+' });
    out.push_str(&exponent.abs().to_string());
    out
}

fn positional(sequence: &DigitSequence) -> String {
    let digits = padded_digits(sequence, 10);
    let exponent = sequence.exponent;
    if exponent <= 0 {
        let mut out = String::from("0.");
        out.extend(std::iter::repeat_n('0', (-exponent) as usize));
        out.push_str(&digits);
        out
    } else if exponent as usize >= digits.len() {
        let mut out = digits;
        let zeros = exponent as usize - out.len();
        out.extend(std::iter::repeat_n('0', zeros));
        out
    } else {
        let mut out = digits;
        out.insert(exponent as usize, '.');
        out
    }
}

/// `Number.prototype.toFixed`; the digit count is validated by the caller.
pub fn to_fixed(value: f64, fraction_digits: i32) -> String {
    if let Some(special) = non_finite(value) {
        return special;
    }
    if value.abs() >= 1e21 {
        return to_string_radix(value, 10);
    }
    let negative = value.is_sign_negative() && value != 0.0;
    let magnitude = value.abs();
    let mut digits = if magnitude == 0.0 {
        "0".to_owned()
    } else {
        let (significand, binary_exponent) = decompose(magnitude);
        big_digits(
            &round_scaled_away(significand, binary_exponent, fraction_digits),
            10,
        )
    };
    if fraction_digits > 0 {
        let fraction = fraction_digits as usize;
        if digits.len() <= fraction {
            let mut padded = String::from("0.");
            padded.extend(std::iter::repeat_n('0', fraction - digits.len()));
            padded.push_str(&digits);
            digits = padded;
        } else {
            digits.insert(digits.len() - fraction, '.');
        }
    }
    signed(digits, negative)
}

/// `Number.prototype.toExponential`: `None` asks for the shortest digits.
pub fn to_exponential(value: f64, fraction_digits: Option<i32>) -> String {
    if let Some(special) = non_finite(value) {
        return special;
    }
    let negative = value.is_sign_negative() && value != 0.0;
    let magnitude = value.abs();
    let sequence = match fraction_digits {
        Some(digits) => fixed_significant(magnitude, digits + 1),
        None if magnitude == 0.0 => DigitSequence {
            mantissa: Big::zero(),
            digits: 1,
            exponent: 1,
        },
        None => shortest_sequence(magnitude, 10),
    };
    signed(exponential(&sequence), negative)
}

/// `Number.prototype.toPrecision` with a validated precision.
pub fn to_precision(value: f64, precision: i32) -> String {
    if let Some(special) = non_finite(value) {
        return special;
    }
    let negative = value.is_sign_negative() && value != 0.0;
    let sequence = fixed_significant(value.abs(), precision);
    let body = if sequence.exponent <= -6 || sequence.exponent > precision {
        exponential(&sequence)
    } else {
        positional(&sequence)
    };
    signed(body, negative)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn int(text: &str, radix: i32) -> f64 {
        parse_int(&units(text), radix)
    }

    fn float(text: &str) -> f64 {
        parse_float(&units(text))
    }

    fn assert_same(actual: f64, expected: f64) {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "actual {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn free_decimal_matches_pinned_thresholds_and_edges() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (0.1 + 0.2, "0.30000000000000004"),
            (f64::MAX, "1.7976931348623157e+308"),
            (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
            (f64::from_bits(1), "5e-324"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ] {
            assert_eq!(to_string_radix(value, 10), expected);
        }
    }

    #[test]
    fn free_non_decimal_uses_shortest_roundtrip_without_exponents() {
        for (value, radix, expected) in [
            (255.0, 2, "11111111"),
            (255.0, 8, "377"),
            (255.0, 16, "ff"),
            (255.0, 36, "73"),
            (-255.0, 16, "-ff"),
            (
                0.1,
                2,
                "0.0001100110011001100110011001100110011001100110011001101",
            ),
            (1.0 / 3.0, 36, "0.c"),
            (1e21, 16, "3635c9adc5dea00000"),
        ] {
            assert_eq!(to_string_radix(value, radix), expected);
        }

        let minimum_binary = to_string_radix(f64::from_bits(1), 2);
        assert_eq!(minimum_binary.len(), 1076);
        assert!(minimum_binary.starts_with("0."));
        assert!(minimum_binary.ends_with('1'));
        assert!(
            minimum_binary[2..minimum_binary.len() - 1]
                .bytes()
                .all(|byte| byte == b'0')
        );
    }

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    #[test]
    fn shortest_candidates_roundtrip_for_a_deterministic_bit_corpus() {
        let mut state = 0x243f_6a88_85a3_08d3_u64;
        for _ in 0..500 {
            let value = f64::from_bits(lcg(&mut state) & !(1_u64 << 63));
            if !value.is_finite() || value == 0.0 {
                continue;
            }
            for radix in [3, 10, 36] {
                let sequence = shortest_sequence(value, radix);
                assert_eq!(
                    sequence_to_f64_bits(&sequence, radix),
                    value.to_bits(),
                    "radix {radix} failed for {value:?}"
                );
            }
            for radix in [2, 8, 16, 32] {
                assert_eq!(
                    to_string_radix(value, radix),
                    free_big(value, radix),
                    "power-of-two fast path, radix {radix}, {value:?}"
                );
            }
        }
    }

    #[test]
    fn integer_fast_path_agrees_with_the_rational_path() {
        let mut state = 7u64;
        for _ in 0..500 {
            let value = (lcg(&mut state) >> 11) as f64;
            for radix in [3, 7, 10, 36] {
                assert_eq!(to_string_radix(value, radix), free_big(value, radix));
            }
        }
    }

    #[test]
    fn decimal_takes_the_fewest_digits_and_quickjs_ties() {
        // At some powers of two (whose round-trip interval is asymmetric)
        // QuickJS keeps one more digit than the fewest that round-trip,
        // which 6.1.6.1.20 step 5 requires (QuickJS prints
        // 7.1202363472230444e-307); an exact digit tie still rounds to
        // even as in QuickJS, where Rust's shortest formatting does not.
        for (value, expected) in [
            (2f64.powi(-1017), "7.120236347223045e-307"),
            (2f64.powi(-24), "5.960464477539063e-8"),
            // 192181864906432.125 exactly.
            (f64::from_bits(0x42e5_d93a_7947_d804), "192181864906432.12"),
        ] {
            assert_eq!(to_string_radix(value, 10), expected);
            assert_eq!(super::super::number::to_string(value), expected);
        }
    }

    #[test]
    fn parse_int_trims_space_selects_radix_and_stops_at_the_longest_prefix() {
        assert_same(int("  -0x10tail", 0), -16.0);
        assert_same(int("+0Xf", 16), 15.0);
        assert_same(int("0x10", 10), 0.0);
        assert_same(int("0b11", 0), 0.0);
        assert_same(int("10102", 2), 10.0);
        assert_same(int("zZ!", 36), 1_295.0);
        assert_same(int("10", 2), 2.0);
        assert_same(int("10", 8), 8.0);
        assert_same(int("10", 36), 36.0);
    }

    #[test]
    fn parse_int_rejects_missing_digits_and_invalid_radices() {
        for source in ["", " ", "+", "-", "xyz", "0x", "-0x"] {
            assert!(int(source, 0).is_nan(), "source {source:?}");
        }
        assert!(int("10", 1).is_nan());
        assert!(int("10", 37).is_nan());
        assert!(int("10", -2).is_nan());
    }

    #[test]
    fn parse_int_rounds_large_integers_once_and_preserves_negative_zero() {
        assert_same(int("9007199254740993", 10), 9_007_199_254_740_992.0);
        assert_same(int("9007199254740995", 10), 9_007_199_254_740_996.0);
        assert_same(int("20000000000001", 16), 9_007_199_254_740_992.0);
        assert_same(int("ffffffffffffffff", 16), 18_446_744_073_709_551_616.0);
        assert_same(int("-000tail", 10), -0.0);
        assert_same(int(&format!("1{}", "0".repeat(400)), 10), f64::INFINITY);
    }

    #[test]
    fn parse_int_preserves_the_pinned_non_power_of_two_digit_limit() {
        // QuickJS keeps 38 decimal digits and truncates the 39th, so this
        // integer just above a binary64 midpoint rounds down.
        let parsed = int("300000000000000031025361333325263798273", 10);
        assert_eq!(parsed.to_bits(), 0x47ec_363c_bf21_f28a);
    }

    #[test]
    fn parse_float_accepts_infinity_and_the_longest_decimal_prefix() {
        assert_same(float("  +Infinitytail"), f64::INFINITY);
        assert_same(float("-Infinity and beyond"), f64::NEG_INFINITY);
        assert_same(float("-1.25e2tail"), -125.0);
        assert_same(float("1.e2"), 100.0);
        assert_same(float(".5e+2"), 50.0);
        assert_same(float("0x10"), 0.0);
        assert_same(float("1e"), 1.0);
        assert_same(float("1e+"), 1.0);
        assert_same(float("1e-"), 1.0);
    }

    #[test]
    fn parse_float_rejects_non_decimal_starts() {
        for source in ["", " ", "+", "-", ".", ".e2", "NaN", "infinity"] {
            assert!(float(source).is_nan(), "source {source:?}");
        }
    }

    #[test]
    fn parse_float_rounds_boundaries_and_preserves_signed_underflow() {
        assert_same(float("9007199254740993"), 9_007_199_254_740_992.0);
        assert_same(float("9007199254740995"), 9_007_199_254_740_996.0);
        assert_same(float("2.4703282292062327e-324"), 0.0);
        assert_same(float("2.4703282292062328e-324"), f64::from_bits(1));
        assert_same(float("1.7976931348623158e308"), f64::MAX);
        assert_same(float("1.7976931348623159e308"), f64::INFINITY);
        assert_same(float("-0x"), -0.0);
        assert_same(float("-1e-9999"), -0.0);
    }

    #[test]
    fn parse_float_preserves_the_pinned_38_digit_decimal_truncation() {
        // Just above the midpoint between 1 and nextUp(1); the discarded
        // digits make it round to the even lower neighbor.
        assert_same(
            float("1.0000000000000001110223024625156540423631668090820313"),
            1.0,
        );
    }

    #[test]
    fn parse_float_combines_large_exponents_before_final_scale_clamping() {
        assert_same(float(&format!("0.{}1e10001", "0".repeat(10_000))), 1.0);
        assert_same(float(&format!("1{}e-10000", "0".repeat(10_000))), 1.0);
    }

    #[test]
    fn both_parsers_use_the_pinned_ecmascript_space_set() {
        for space in [
            0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x0020, 0x00a0, 0x1680, 0x2000, 0x200a, 0x2028,
            0x2029, 0x202f, 0x205f, 0x3000, 0xfeff,
        ] {
            let input = [space, u16::from(b'1')];
            assert_same(parse_int(&input, 10), 1.0);
            assert_same(parse_float(&input), 1.0);
        }
        for other in [0x0085, 0x180e] {
            let input = [other, u16::from(b'1')];
            assert!(parse_int(&input, 10).is_nan());
            assert!(parse_float(&input).is_nan());
        }
    }

    #[test]
    fn embedded_nul_terminates_the_prefix_without_losing_prior_digits() {
        let input = [u16::from(b'1'), u16::from(b'2'), 0, u16::from(b'3')];
        assert_same(parse_int(&input, 10), 12.0);
        assert_same(parse_float(&input), 12.0);
    }

    #[test]
    fn big_to_f64_rounds_to_nearest_even() {
        let two = |exponent: u64| shl(&Big::from_u64(1), exponent);
        let plus = |value: Big, small: u64| must(value.add(&Big::from_u64(small)));
        // 2^53 + 1 ties down to 2^53; 2^53 + 3 ties up to 2^53 + 4.
        assert_same(plus(two(53), 1).to_f64(), 9_007_199_254_740_992.0);
        assert_same(plus(two(53), 3).to_f64(), 9_007_199_254_740_996.0);
        // A sticky bit far below the rounding position breaks the tie up.
        let above = plus(shl(&plus(two(53), 1), 100), 1);
        assert_same(above.to_f64(), 9_007_199_254_740_994.0 * 2f64.powi(100));
        assert_same(two(1023).to_f64(), 2f64.powi(1023));
        assert_same(two(1024).to_f64(), f64::INFINITY);
        assert_same(Big::from_i64(-5).to_f64(), -5.0);
        assert_same(Big::zero().to_f64(), 0.0);
    }
}
