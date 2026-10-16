//! BigInt arithmetic on sign and magnitude (64-bit limbs, little-endian, no
//! high zero limbs), the representation of the heap's `BigIntRecord`. Plain
//! schoolbook algorithms: phase 2 runs the BigInt probes, and phase 4 brings
//! `ShortBigInt` and the fast paths (phase 1, section 2).

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Big {
    pub negative: bool,
    pub limbs: Vec<u64>,
}

pub(crate) enum BigError {
    DivisionByZero,
    NegativeExponent,
    TooLarge,
}

/// Limbs a result may reach before `RangeError: BigInt is too large`.
const MAX_LIMBS: usize = 1 << 14;

impl Big {
    pub fn zero() -> Big {
        Big {
            negative: false,
            limbs: Vec::new(),
        }
    }

    pub fn new(negative: bool, mut limbs: Vec<u64>) -> Big {
        trim(&mut limbs);
        let negative = negative && !limbs.is_empty();
        Big { negative, limbs }
    }

    pub fn from_i64(value: i64) -> Big {
        Big::new(value < 0, vec![value.unsigned_abs()])
    }

    pub fn from_u64(value: u64) -> Big {
        Big::new(false, vec![value])
    }

    /// The double nearest the exact value, ties to even; `±Infinity` past
    /// `f64::MAX`. The top 64 bits round once, with every lower bit folded
    /// into a sticky low bit.
    pub fn to_f64(&self) -> f64 {
        let bits = bit_length(&self.limbs);
        let magnitude = if bits <= 64 {
            self.limbs.first().copied().unwrap_or(0) as f64
        } else if bits > 1024 + 64 {
            f64::INFINITY
        } else {
            let shift = bits - 64;
            let (words, rest) = ((shift / 64) as usize, shift % 64);
            let sticky = self.limbs[..words].iter().any(|&limb| limb != 0)
                || (rest != 0 && self.limbs[words] & ((1u64 << rest) - 1) != 0);
            let top = shr_mag(&self.limbs, shift)[0] | u64::from(sticky);
            let scale = if shift <= 1023 {
                f64::from_bits((1023 + shift) << 52)
            } else {
                f64::INFINITY
            };
            top as f64 * scale
        };
        if self.negative { -magnitude } else { magnitude }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// The exact value of an integral double.
    pub fn from_f64(value: f64) -> Option<Big> {
        if !value.is_finite() || value.trunc() != value {
            return None;
        }
        let bits = value.abs().to_bits();
        let exponent = ((bits >> 52) & 0x7FF) as i32;
        if exponent == 0 {
            return Some(Big::zero());
        }
        let mantissa = (bits & ((1 << 52) - 1)) | (1 << 52);
        let shift = exponent - 1075;
        let magnitude = Big::new(false, vec![mantissa]);
        let magnitude = if shift >= 0 {
            shl_mag(&magnitude.limbs, shift as u64)
        } else {
            shr_mag(&magnitude.limbs, (-shift) as u64)
        };
        Some(Big::new(value < 0.0, magnitude))
    }

    pub fn neg(&self) -> Big {
        Big::new(!self.negative, self.limbs.clone())
    }

    pub fn cmp(&self, other: &Big) -> Ordering {
        match (self.negative, other.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => cmp_mag(&self.limbs, &other.limbs),
            (true, true) => cmp_mag(&other.limbs, &self.limbs),
        }
    }

    pub fn add(&self, other: &Big) -> Result<Big, BigError> {
        let result = if self.negative == other.negative {
            Big::new(self.negative, add_mag(&self.limbs, &other.limbs))
        } else {
            match cmp_mag(&self.limbs, &other.limbs) {
                Ordering::Less => Big::new(other.negative, sub_mag(&other.limbs, &self.limbs)),
                _ => Big::new(self.negative, sub_mag(&self.limbs, &other.limbs)),
            }
        };
        checked(result)
    }

    pub fn sub(&self, other: &Big) -> Result<Big, BigError> {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Big) -> Result<Big, BigError> {
        if self.limbs.len() + other.limbs.len() > MAX_LIMBS + 1 {
            return Err(BigError::TooLarge);
        }
        checked(Big::new(
            self.negative != other.negative,
            mul_mag(&self.limbs, &other.limbs),
        ))
    }

    /// Truncating division and its remainder.
    pub fn div_rem(&self, other: &Big) -> Result<(Big, Big), BigError> {
        if other.is_zero() {
            return Err(BigError::DivisionByZero);
        }
        let (quotient, remainder) = div_rem_mag(&self.limbs, &other.limbs);
        Ok((
            Big::new(self.negative != other.negative, quotient),
            Big::new(self.negative, remainder),
        ))
    }

    pub fn pow(&self, exponent: &Big) -> Result<Big, BigError> {
        if exponent.negative {
            return Err(BigError::NegativeExponent);
        }
        if exponent.is_zero() {
            return Ok(Big::from_i64(1));
        }
        if self.is_zero() || self.limbs == [1] {
            let negative = self.negative && exponent.limbs[0] & 1 == 1;
            return Ok(Big::new(negative, self.limbs.clone()));
        }
        if exponent.limbs.len() > 1 || exponent.limbs[0] > (MAX_LIMBS as u64) * 64 {
            return Err(BigError::TooLarge);
        }
        let mut power = exponent.limbs[0];
        let mut base = self.clone();
        let mut result = Big::from_i64(1);
        while power > 0 {
            if power & 1 == 1 {
                result = result.mul(&base)?;
            }
            power >>= 1;
            if power > 0 {
                base = base.mul(&base)?;
            }
        }
        Ok(result)
    }

    /// `self << shift`, a right shift for a negative `shift`; arithmetic
    /// (flooring) for negative values.
    pub fn shl(&self, shift: &Big) -> Result<Big, BigError> {
        if shift.limbs.len() > 1 {
            return if shift.negative {
                Ok(if self.negative {
                    Big::from_i64(-1)
                } else {
                    Big::zero()
                })
            } else if self.is_zero() {
                Ok(Big::zero())
            } else {
                Err(BigError::TooLarge)
            };
        }
        let amount = shift.limbs.first().copied().unwrap_or(0);
        if !shift.negative {
            if amount > (MAX_LIMBS as u64) * 64 {
                return if self.is_zero() {
                    Ok(Big::zero())
                } else {
                    Err(BigError::TooLarge)
                };
            }
            return checked(Big::new(self.negative, shl_mag(&self.limbs, amount)));
        }
        if !self.negative {
            return Ok(Big::new(false, shr_mag(&self.limbs, amount)));
        }
        // floor(-m / 2^s) = -((m - 1) >> s) - 1
        let less = sub_mag(&self.limbs, &[1]);
        let shifted = Big::new(false, shr_mag(&less, amount));
        Ok(Big::new(true, add_mag(&shifted.limbs, &[1])))
    }

    pub fn bit_and(&self, other: &Big) -> Big {
        bitwise(self, other, |a, b| a & b)
    }

    pub fn bit_or(&self, other: &Big) -> Big {
        bitwise(self, other, |a, b| a | b)
    }

    pub fn bit_xor(&self, other: &Big) -> Big {
        bitwise(self, other, |a, b| a ^ b)
    }

    /// `~x = -x - 1`.
    pub fn bit_not(&self) -> Big {
        let one = Big::from_i64(1);
        match self.neg().sub(&one) {
            Ok(value) => value,
            Err(_) => unreachable!("~x does not grow past the limit"),
        }
    }

    pub fn to_string_radix10(&self) -> String {
        if self.is_zero() {
            return "0".into();
        }
        const CHUNK: u64 = 10_000_000_000_000_000_000;
        let mut chunks = Vec::new();
        let mut rest = self.limbs.clone();
        while !rest.is_empty() {
            let mut remainder = 0u128;
            for limb in rest.iter_mut().rev() {
                let current = (remainder << 64) | u128::from(*limb);
                *limb = (current / u128::from(CHUNK)) as u64;
                remainder = current % u128::from(CHUNK);
            }
            trim(&mut rest);
            chunks.push(remainder as u64);
        }
        let mut text = String::new();
        if self.negative {
            text.push('-');
        }
        text.push_str(&chunks.pop().unwrap().to_string());
        for chunk in chunks.iter().rev() {
            text.push_str(&format!("{chunk:019}"));
        }
        text
    }

    /// `StringToBigInt` over trimmed text: decimal with an optional sign,
    /// or a `0x`, `0o`, `0b` literal.
    pub fn parse(text: &str) -> Option<Big> {
        let text = text.trim_matches(|c: char| {
            c.is_whitespace() || c == '\u{FEFF}' || c == '\u{2028}' || c == '\u{2029}'
        });
        if text.is_empty() {
            return Some(Big::zero());
        }
        let bytes = text.as_bytes();
        let (radix, negative, digits) = if bytes.len() > 2 && bytes[0] == b'0' {
            match bytes[1] {
                b'x' | b'X' => (16, false, &text[2..]),
                b'o' | b'O' => (8, false, &text[2..]),
                b'b' | b'B' => (2, false, &text[2..]),
                _ => (10, false, text),
            }
        } else {
            match bytes[0] {
                b'-' => (10, true, &text[1..]),
                b'+' => (10, false, &text[1..]),
                _ => (10, false, text),
            }
        };
        if digits.is_empty() {
            return None;
        }
        let mut limbs: Vec<u64> = Vec::new();
        for c in digits.chars() {
            let digit = c.to_digit(radix)?;
            let mut carry = u128::from(digit);
            for limb in limbs.iter_mut() {
                let current = u128::from(*limb) * u128::from(radix) + carry;
                *limb = current as u64;
                carry = current >> 64;
            }
            if carry != 0 {
                limbs.push(carry as u64);
            }
        }
        Some(Big::new(negative, limbs))
    }
}

fn checked(value: Big) -> Result<Big, BigError> {
    if value.limbs.len() > MAX_LIMBS {
        Err(BigError::TooLarge)
    } else {
        Ok(value)
    }
}

fn trim(limbs: &mut Vec<u64>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

fn bit_length(limbs: &[u64]) -> u64 {
    match limbs.last() {
        None => 0,
        Some(&top) => (limbs.len() as u64) * 64 - u64::from(top.leading_zeros()),
    }
}

fn cmp_mag(a: &[u64], b: &[u64]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut out = Vec::with_capacity(long.len() + 1);
    let mut carry = false;
    for (index, &limb) in long.iter().enumerate() {
        let (sum, first) = limb.overflowing_add(short.get(index).copied().unwrap_or(0));
        let (sum, second) = sum.overflowing_add(u64::from(carry));
        out.push(sum);
        carry = first || second;
    }
    if carry {
        out.push(1);
    }
    out
}

/// `a - b` for `a >= b`.
fn sub_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = false;
    for (index, &limb) in a.iter().enumerate() {
        let (difference, first) = limb.overflowing_sub(b.get(index).copied().unwrap_or(0));
        let (difference, second) = difference.overflowing_sub(u64::from(borrow));
        out.push(difference);
        borrow = first || second;
    }
    debug_assert!(!borrow, "sub_mag underflow");
    trim(&mut out);
    out
}

fn mul_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u128;
        for (j, &y) in b.iter().enumerate() {
            let current = u128::from(out[i + j]) + u128::from(x) * u128::from(y) + carry;
            out[i + j] = current as u64;
            carry = current >> 64;
        }
        out[i + b.len()] = carry as u64;
    }
    trim(&mut out);
    out
}

fn shl_mag(limbs: &[u64], shift: u64) -> Vec<u64> {
    if limbs.is_empty() {
        return Vec::new();
    }
    let (words, bits) = ((shift / 64) as usize, (shift % 64) as u32);
    let mut out = vec![0u64; words];
    if bits == 0 {
        out.extend_from_slice(limbs);
    } else {
        let mut carry = 0u64;
        for &limb in limbs {
            out.push((limb << bits) | carry);
            carry = limb >> (64 - bits);
        }
        out.push(carry);
    }
    trim(&mut out);
    out
}

fn shr_mag(limbs: &[u64], shift: u64) -> Vec<u64> {
    let words = (shift / 64) as usize;
    if words >= limbs.len() {
        return Vec::new();
    }
    let bits = (shift % 64) as u32;
    let rest = &limbs[words..];
    let mut out = Vec::with_capacity(rest.len());
    for (index, &limb) in rest.iter().enumerate() {
        let high = rest.get(index + 1).copied().unwrap_or(0);
        out.push(if bits == 0 {
            limb
        } else {
            (limb >> bits) | (high << (64 - bits))
        });
    }
    trim(&mut out);
    out
}

/// Schoolbook long division, one bit at a time past a one-limb divisor.
fn div_rem_mag(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    if cmp_mag(a, b) == Ordering::Less {
        return (Vec::new(), a.to_vec());
    }
    if b.len() == 1 {
        let divisor = u128::from(b[0]);
        let mut quotient = vec![0u64; a.len()];
        let mut remainder = 0u128;
        for index in (0..a.len()).rev() {
            let current = (remainder << 64) | u128::from(a[index]);
            quotient[index] = (current / divisor) as u64;
            remainder = current % divisor;
        }
        trim(&mut quotient);
        let mut remainder = vec![remainder as u64];
        trim(&mut remainder);
        return (quotient, remainder);
    }
    let bits = bit_length(a);
    let mut quotient = vec![0u64; a.len()];
    let mut remainder: Vec<u64> = Vec::new();
    for bit in (0..bits).rev() {
        remainder = shl_mag(&remainder, 1);
        if a[(bit / 64) as usize] >> (bit % 64) & 1 == 1 {
            if remainder.is_empty() {
                remainder.push(1);
            } else {
                remainder[0] |= 1;
            }
        }
        if cmp_mag(&remainder, b) != Ordering::Less {
            remainder = sub_mag(&remainder, b);
            quotient[(bit / 64) as usize] |= 1 << (bit % 64);
        }
    }
    trim(&mut quotient);
    (quotient, remainder)
}

/// Two's complement in `len` limbs.
fn complement(value: &Big, len: usize) -> Vec<u64> {
    let mut out: Vec<u64> = (0..len)
        .map(|index| value.limbs.get(index).copied().unwrap_or(0))
        .collect();
    if value.negative {
        let mut carry = true;
        for limb in out.iter_mut() {
            let (sum, overflow) = (!*limb).overflowing_add(u64::from(carry));
            *limb = sum;
            carry = overflow;
        }
    }
    out
}

fn bitwise(a: &Big, b: &Big, op: impl Fn(u64, u64) -> u64) -> Big {
    let len = a.limbs.len().max(b.limbs.len()) + 1;
    let (x, y) = (complement(a, len), complement(b, len));
    let mut out: Vec<u64> = x.iter().zip(&y).map(|(&x, &y)| op(x, y)).collect();
    let negative = out[len - 1] >> 63 == 1;
    if negative {
        let mut carry = true;
        for limb in out.iter_mut() {
            let (sum, overflow) = (!*limb).overflowing_add(u64::from(carry));
            *limb = sum;
            carry = overflow;
        }
    }
    Big::new(negative, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(text: &str) -> Big {
        Big::parse(text).unwrap()
    }

    #[test]
    fn arithmetic_round_trips_through_text() {
        let two_256 = Big::from_i64(1).shl(&Big::from_i64(256)).ok().unwrap();
        let baseline = two_256.sub(&Big::from_i64(1)).ok().unwrap();
        assert_eq!(
            baseline.to_string_radix10(),
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
        let x = baseline.bit_xor(&Big::from_i64(5));
        let back = x
            .add(&Big::from_i64(1))
            .ok()
            .unwrap()
            .sub(&Big::from_i64(1))
            .ok()
            .unwrap();
        assert_eq!(back.bit_xor(&Big::from_i64(5)), baseline);
        assert_eq!(
            big("123456789012345678901234567890")
                .mul(&big("-987654321"))
                .ok()
                .unwrap()
                .to_string_radix10(),
            "-121932631124828532112482853211126352690"
        );
        let (q, r) = big("-121932631124828532112482853211126352697")
            .div_rem(&big("123456789012345678901234567890"))
            .ok()
            .unwrap();
        assert_eq!(q.to_string_radix10(), "-987654321");
        assert_eq!(r.to_string_radix10(), "-7");
        assert_eq!(big("-5").bit_and(&big("3")).to_string_radix10(), "3");
        assert_eq!(big("-5").bit_or(&big("3")).to_string_radix10(), "-5");
        assert_eq!(big("-5").bit_not().to_string_radix10(), "4");
        assert_eq!(
            big("-5").shl(&big("-1")).ok().unwrap().to_string_radix10(),
            "-3"
        );
        assert_eq!(
            big("2").pow(&big("100")).ok().unwrap().to_string_radix10(),
            "1267650600228229401496703205376"
        );
        assert_eq!(big("0x10"), Big::from_i64(16));
        assert_eq!(
            Big::from_f64(-1e20).unwrap().to_string_radix10(),
            "-100000000000000000000"
        );
    }
}
