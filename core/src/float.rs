//! The one floating-point implementation in the core.
//!
//! `core` has no `f64` math methods, so every transcendental call names the
//! `libm` crate — a musl port — rather than the platform libm. One
//! implementation everywhere: `Math.sin` returns the same bits on every port,
//! so a Test262 receipt or a corpus snapshot computed on one host holds on
//! another.
//!
//! Nothing may mix the two. The exactly-rounded operations (`sqrt`, `floor`,
//! `ceil`, `trunc`, `round`, `fma`) agree bit for bit with the platform libm,
//! so borrowing them costs nothing; the transcendentals do not — `libm` and
//! glibc differ in the last bit on 15 of the 20 that QuickJS exposes.
//!
//! `libm` spells some of these differently from the methods they replace, and
//! lacks `fract` and `powi` outright; those live here so no call site has to
//! know.

pub(crate) use libm::{
    acos, asin, asinh, atan, atan2, cbrt, ceil, cos, cosh, exp, expm1, floor, log, log1p, log2,
    log10, pow, sin, sinh, sqrt, tan, tanh, trunc,
};

/// `f64::rem_euclid`: the remainder with the modulus's sign, which `core` has
/// no method for.
pub(crate) fn rem_euclid(value: f64, modulus: f64) -> f64 {
    let remainder = value % modulus;
    if remainder < 0.0 {
        remainder + modulus.abs()
    } else {
        remainder
    }
}

/// `f64::fract`: the fractional part, keeping the sign of the input.
pub(crate) fn fract(value: f64) -> f64 {
    value - trunc(value)
}

/// `f64::log(base)`, which `libm` has no single function for. Only callers whose
/// result an exact comparison then corrects may use it.
pub(crate) fn log_base(value: f64, base: f64) -> f64 {
    log(value) / log(base)
}

/// Two to the `exponent`, exactly: `f64::powi` is not available, and every
/// remaining call site is a test naming a power of two.
#[cfg(test)]
pub(crate) fn pow2(exponent: i32) -> f64 {
    libm::ldexp(1.0, exponent)
}
