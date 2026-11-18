// Adapted from rust-lang/libm 0.2.16; see LICENSES/Libm.txt.
// Keep the upstream formulas; route sqrt through the core hardware backend.
/* origin: FreeBSD /usr/src/lib/msun/src/e_asin.c */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
/* asin(x)
 * Method :
 *      Since  asin(x) = x + x^3/6 + x^5*3/40 + x^7*15/336 + ...
 *      we approximate asin(x) on [0,0.5] by
 *              asin(x) = x + x*x^2*R(x^2)
 *      where
 *              R(x^2) is a rational approximation of (asin(x)-x)/x^3
 *      and its remez error is bounded by
 *              |(asin(x)-x)/x^3 - R(x^2)| < 2^(-58.75)
 *
 *      For x in [0.5,1]
 *              asin(x) = pi/2-2*asin(sqrt((1-x)/2))
 *      Let y = (1-x), z = y/2, s := sqrt(z), and pio2_hi+pio2_lo=pi/2;
 *      then for x>0.98
 *              asin(x) = pi/2 - 2*(s+s*z*R(z))
 *                      = pio2_hi - (2*(s+s*z*R(z)) - pio2_lo)
 *      For x<=0.98, let pio4_hi = pio2_hi/2, then
 *              f = hi part of s;
 *              c = sqrt(z) - f = (z-f*f)/(s+f)         ...f+c=sqrt(z)
 *      and
 *              asin(x) = pi/2 - 2*(s+s*z*R(z))
 *                      = pio4_hi+(pio4-2s)-(2s*z*R(z)-pio2_lo)
 *                      = pio4_hi+(pio4-2f)-(2s*z*R(z)-(pio2_lo+2c))
 *
 * Special cases:
 *      if x is NaN, return x itself;
 *      if |x|>1, return NaN with invalid signal.
 *
 */

use super::sqrt;

const PIO2_HI: f64 = f64::from_bits(0x3ff921fb54442d18); /* 0x3FF921FB, 0x54442D18 */
const PIO2_LO: f64 = f64::from_bits(0x3c91a62633145c07); /* 0x3C91A626, 0x33145C07 */
/* coefficients for R(x^2) */
const P_S0: f64 = f64::from_bits(0x3fc5555555555555); /* 0x3FC55555, 0x55555555 */
const P_S1: f64 = f64::from_bits(0xbfd4d61203eb6f7d); /* 0xBFD4D612, 0x03EB6F7D */
const P_S2: f64 = f64::from_bits(0x3fc9c1550e884455); /* 0x3FC9C155, 0x0E884455 */
const P_S3: f64 = f64::from_bits(0xbfa48228b5688f3b); /* 0xBFA48228, 0xB5688F3B */
const P_S4: f64 = f64::from_bits(0x3f49efe07501b288); /* 0x3F49EFE0, 0x7501B288 */
const P_S5: f64 = f64::from_bits(0x3f023de10dfdf709); /* 0x3F023DE1, 0x0DFDF709 */
const Q_S1: f64 = f64::from_bits(0xc0033a271c8a2d4b); /* 0xC0033A27, 0x1C8A2D4B */
const Q_S2: f64 = f64::from_bits(0x40002ae59c598ac8); /* 0x40002AE5, 0x9C598AC8 */
const Q_S3: f64 = f64::from_bits(0xbfe6066c1b8d0159); /* 0xBFE6066C, 0x1B8D0159 */
const Q_S4: f64 = f64::from_bits(0x3fb3b8c5b12e9282); /* 0x3FB3B8C5, 0xB12E9282 */

fn comp_r(z: f64) -> f64 {
    let p = z * (P_S0 + z * (P_S1 + z * (P_S2 + z * (P_S3 + z * (P_S4 + z * P_S5)))));
    let q = 1.0 + z * (Q_S1 + z * (Q_S2 + z * (Q_S3 + z * Q_S4)));
    p / q
}

/// Arcsine (f64)
///
/// Computes the inverse sine (arc sine) of the argument `x`.
/// Arguments to asin must be in the range -1 to 1.
/// Returns values in radians, in the range of -pi/2 to pi/2.
pub(crate) fn asin(mut x: f64) -> f64 {
    let hx = (x.to_bits() >> 32) as u32;
    let ix = hx & 0x7fffffff;
    /* |x| >= 1 or nan */
    if ix >= 0x3ff00000 {
        let lx = x.to_bits() as u32;
        if ((ix - 0x3ff00000) | lx) == 0 {
            /* asin(1) = +-pi/2 with inexact */
            return x * PIO2_HI + f64::from_bits(0x3870000000000000);
        } else {
            // Wasm exposes no floating-point exception flags; invalid domains return NaN.
            return f64::NAN;
        }
    }
    /* |x| < 0.5 */
    if ix < 0x3fe00000 {
        /* if 0x1p-1022 <= |x| < 0x1p-26, avoid raising underflow */
        if (0x00100000..0x3e500000).contains(&ix) {
            return x;
        } else {
            return x + x * comp_r(x * x);
        }
    }
    /* 1 > |x| >= 0.5 */
    let z = (1.0 - x.abs()) * 0.5;
    let s = sqrt(z);
    let r = comp_r(z);
    if ix >= 0x3fef3333 {
        /* if |x| > 0.975 */
        x = PIO2_HI - (2. * (s + s * r) - PIO2_LO);
    } else {
        /* f+c = sqrt(z) */
        let f = f64::from_bits(s.to_bits() & 0xffff_ffff_0000_0000);
        let c = (z - f * f) / (s + f);
        x = 0.5 * PIO2_HI - (2.0 * s * r - (PIO2_LO - 2.0 * c) - (0.5 * PIO2_HI - 2.0 * f));
    }
    if hx >> 31 != 0 { -x } else { x }
}
