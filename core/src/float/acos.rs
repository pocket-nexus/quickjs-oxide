// Adapted from rust-lang/libm 0.2.16; see LICENSES/Libm.txt.
// Keep the upstream formulas; route sqrt through the core hardware backend.
/* origin: FreeBSD /usr/src/lib/msun/src/e_acos.c */
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
/* acos(x)
 * Method :
 *      acos(x)  = pi/2 - asin(x)
 *      acos(-x) = pi/2 + asin(x)
 * For |x|<=0.5
 *      acos(x) = pi/2 - (x + x*x^2*R(x^2))     (see asin.c)
 * For x>0.5
 *      acos(x) = pi/2 - (pi/2 - 2asin(sqrt((1-x)/2)))
 *              = 2asin(sqrt((1-x)/2))
 *              = 2s + 2s*z*R(z)        ...z=(1-x)/2, s=sqrt(z)
 *              = 2f + (2c + 2s*z*R(z))
 *     where f=hi part of s, and c = (z-f*f)/(s+f) is the correction term
 *     for f so that f+c ~ sqrt(z).
 * For x<-0.5
 *      acos(x) = pi - 2asin(sqrt((1-|x|)/2))
 *              = pi - 0.5*(s+s*z*R(z)), where z=(1-|x|)/2,s=sqrt(z)
 *
 * Special cases:
 *      if x is NaN, return x itself;
 *      if |x|>1, return NaN with invalid signal.
 *
 * Function needed: sqrt
 */

use super::sqrt;

const PIO2_HI: f64 = f64::from_bits(0x3ff921fb54442d18); /* 0x3FF921FB, 0x54442D18 */
const PIO2_LO: f64 = f64::from_bits(0x3c91a62633145c07); /* 0x3C91A626, 0x33145C07 */
const PS0: f64 = f64::from_bits(0x3fc5555555555555); /* 0x3FC55555, 0x55555555 */
const PS1: f64 = f64::from_bits(0xbfd4d61203eb6f7d); /* 0xBFD4D612, 0x03EB6F7D */
const PS2: f64 = f64::from_bits(0x3fc9c1550e884455); /* 0x3FC9C155, 0x0E884455 */
const PS3: f64 = f64::from_bits(0xbfa48228b5688f3b); /* 0xBFA48228, 0xB5688F3B */
const PS4: f64 = f64::from_bits(0x3f49efe07501b288); /* 0x3F49EFE0, 0x7501B288 */
const PS5: f64 = f64::from_bits(0x3f023de10dfdf709); /* 0x3F023DE1, 0x0DFDF709 */
const QS1: f64 = f64::from_bits(0xc0033a271c8a2d4b); /* 0xC0033A27, 0x1C8A2D4B */
const QS2: f64 = f64::from_bits(0x40002ae59c598ac8); /* 0x40002AE5, 0x9C598AC8 */
const QS3: f64 = f64::from_bits(0xbfe6066c1b8d0159); /* 0xBFE6066C, 0x1B8D0159 */
const QS4: f64 = f64::from_bits(0x3fb3b8c5b12e9282); /* 0x3FB3B8C5, 0xB12E9282 */

fn r(z: f64) -> f64 {
    let p: f64 = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
    let q: f64 = 1.0 + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
    p / q
}

/// Arccosine (f64)
///
/// Computes the inverse cosine (arc cosine) of the input value.
/// Arguments must be in the range -1 to 1.
/// Returns values in radians, in the range of 0 to pi.
pub(crate) fn acos(x: f64) -> f64 {
    let x1p_120f = f64::from_bits(0x3870000000000000); // 0x1p-120 === 2 ^ -120
    let z: f64;
    let w: f64;
    let s: f64;

    let hx = (x.to_bits() >> 32) as u32;
    let ix = hx & 0x7fffffff;
    /* |x| >= 1 or nan */
    if ix >= 0x3ff00000 {
        let lx: u32 = x.to_bits() as u32;

        if ((ix - 0x3ff00000) | lx) == 0 {
            /* acos(1)=0, acos(-1)=pi */
            if (hx >> 31) != 0 {
                return 2. * PIO2_HI + x1p_120f;
            }
            return 0.;
        }
        // Wasm exposes no floating-point exception flags; invalid domains return NaN.
        return f64::NAN;
    }
    /* |x| < 0.5 */
    if ix < 0x3fe00000 {
        if ix <= 0x3c600000 {
            /* |x| < 2**-57 */
            return PIO2_HI + x1p_120f;
        }
        return PIO2_HI - (x - (PIO2_LO - x * r(x * x)));
    }
    /* x < -0.5 */
    if (hx >> 31) != 0 {
        z = (1.0 + x) * 0.5;
        s = sqrt(z);
        w = r(z) * s - PIO2_LO;
        return 2. * (PIO2_HI - (s + w));
    }
    /* x > 0.5 */
    z = (1.0 - x) * 0.5;
    s = sqrt(z);
    // Set the low 4 bytes to zero
    let df = f64::from_bits(s.to_bits() & 0xff_ff_ff_ff_00_00_00_00);

    let c = (z - df * df) / (s + df);
    w = r(z) * s + c;
    2. * (df + w)
}
