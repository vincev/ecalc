//! Complex elementary functions on decimal floats.
//!
//! Each kernel takes the real and imaginary parts and a working precision,
//! and returns both parts of the result in radians. The formulas are chosen
//! so that neither part is a difference of nearly equal quantities:
//! composing `ln`, `sqrt`, and `*` at the display precision, as the textbook
//! identities read, returned `asinh((-3461328, 17))` as `(-11.5, pi)`.
//!
//! [`refine`] then runs a kernel at two precisions and widens it until both
//! parts agree, which catches the cancellation a formula cannot avoid
//! (`expm1` when `x` is close to `y^2 / 2`).

use crate::error::CalcError;
use crate::float::DecFloat;
use num_bigint::BigInt;
use std::cmp::Ordering;

pub(crate) type Pair = (DecFloat, DecFloat);

/// Run `kernel` at a growing working precision until two runs agree to
/// `prec` digits in both parts, and return the wider one.
pub(crate) fn refine(
    prec: u32,
    kernel: impl Fn(u32) -> Result<Pair, CalcError>,
) -> Result<Pair, CalcError> {
    let steps = [
        prec.saturating_add(10),
        prec.saturating_add(22),
        prec.saturating_mul(2).saturating_add(40),
        prec.saturating_mul(4).saturating_add(80),
    ];
    let mut last = kernel(steps[0])?;
    for &work in &steps[1..] {
        let next = kernel(work)?;
        let settled =
            agrees(&last.0, &next.0, &next.1, prec) && agrees(&last.1, &next.1, &next.0, prec);
        last = next;
        if settled {
            break;
        }
    }
    Ok(last)
}

/// Whether two runs give the same `prec` digits of one part. A part that is
/// more than `prec` orders below the other one is dropped by the caller, so
/// it does not have to settle.
fn agrees(old: &DecFloat, new: &DecFloat, other: &DecFloat, prec: u32) -> bool {
    if old.is_zero() && new.is_zero() {
        return true;
    }
    let digits = i64::from(prec);
    let negligible = |v: &DecFloat| v.is_zero() || v.order() < other.order() - digits - 1;
    if !other.is_zero() && negligible(old) && negligible(new) {
        return true;
    }
    if old.is_zero() || new.is_zero() {
        return false;
    }
    let wide = prec.saturating_add(8);
    let diff = old.sub(new, wide);
    diff.is_zero() || diff.order() < new.order() - digits - 2
}

fn int(n: i64, work: u32) -> DecFloat {
    DecFloat::from_i64(n, work)
}

fn halve(v: &DecFloat, work: u32) -> DecFloat {
    v.mul(&DecFloat::from_decimal(BigInt::from(5), -1, work), work)
}

fn with_sign(v: DecFloat, negative: bool) -> DecFloat {
    if negative { v.neg() } else { v }
}

fn greater(a: &DecFloat, b: &DecFloat) -> bool {
    a.cmp_val(b) == Ordering::Greater
}

/// Twice the working digits: a product of two `work`-digit numbers is exact
/// there, and so is the sum that follows it.
fn exact(work: u32) -> u32 {
    work.saturating_mul(2).saturating_add(2)
}

/// `exp(x) - 1` for a real `x`.
pub(crate) fn expm1_real(x: &DecFloat, work: u32) -> Result<DecFloat, CalcError> {
    if x.is_zero() {
        return Ok(DecFloat::zero(work));
    }
    if x.order() < 0 {
        return x.exp_minus_one(work);
    }
    // Past this `exp(x)` is below the last working digit.
    let floor = int(-(3 * i64::from(work) + 10), work);
    if x.cmp_val(&floor) == Ordering::Less {
        return Ok(int(-1, work));
    }
    Ok(x.exp(work)?.sub(&int(1, work), work))
}

/// `sinh(x)` for a real `x`, without the cancellation of `e^x - e^-x`.
pub(crate) fn sinh(x: &DecFloat, work: u32) -> Result<DecFloat, CalcError> {
    if x.is_zero() {
        return Ok(DecFloat::zero(work));
    }
    if x.order() < 0 {
        // (t + t / (1 + t)) / 2 with t = expm1(x).
        let t = x.exp_minus_one(work)?;
        let ratio = t.div(&int(1, work).add(&t, work), work)?;
        return Ok(halve(&t.add(&ratio, work), work));
    }
    let e = x.exp(work)?;
    let inv = int(1, work).div(&e, work)?;
    Ok(halve(&e.sub(&inv, work), work))
}

pub(crate) fn cosh(x: &DecFloat, work: u32) -> Result<DecFloat, CalcError> {
    let e = x.exp(work)?;
    let inv = int(1, work).div(&e, work)?;
    Ok(halve(&e.add(&inv, work), work))
}

/// Past this magnitude `tanh` is 1 to every working digit.
fn saturated(v: &DecFloat, work: u32) -> bool {
    greater(&v.abs(), &int(3 * i64::from(work) + 10, work))
}

/// `tan(x + iy)` from `sin x`, `cos x`, and `y` in radians:
/// `(sin x cos x, sinh y cosh y) / (cos^2 x + sinh^2 y)`. The usual
/// `cos 2x + cosh 2y` cancels near a pole.
pub(crate) fn tan(
    sin_x: &DecFloat,
    cos_x: &DecFloat,
    y: &DecFloat,
    work: u32,
) -> Result<Pair, CalcError> {
    if saturated(y, work) {
        return Ok((
            DecFloat::zero(work),
            with_sign(int(1, work), y.is_negative()),
        ));
    }
    let sh = sinh(y, work)?;
    let ch = cosh(y, work)?;
    let denom = cos_x.mul(cos_x, work).add(&sh.mul(&sh, work), work);
    if denom.is_zero() {
        return Err(CalcError::Domain("tangent is undefined"));
    }
    Ok((
        sin_x.mul(cos_x, work).div(&denom, work)?,
        sh.mul(&ch, work).div(&denom, work)?,
    ))
}

/// `tanh(x + iy) = (sinh x cosh x, sin y cos y) / (cos^2 y + sinh^2 x)`.
pub(crate) fn tanh(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    if saturated(x, work) {
        return Ok((
            with_sign(int(1, work), x.is_negative()),
            DecFloat::zero(work),
        ));
    }
    let (sin_y, cos_y) = y.sin_cos(work)?;
    let (im, re) = tan(&sin_y, &cos_y, x, work)?;
    Ok((re, im))
}

/// `exp(z) - 1 = (expm1(x) cos y - 2 sin^2(y / 2), e^x sin y)`.
pub(crate) fn expm1(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (sin_y, cos_y) = y.sin_cos(work)?;
    let half_sin = halve(y, work).sin(work)?;
    let versine = half_sin.mul(&half_sin, work).mul(&int(2, work), work);
    let grow = expm1_real(x, work)?;
    let re = grow.mul(&cos_y, work).sub(&versine, work);
    let im = grow.add(&int(1, work), work).mul(&sin_y, work);
    Ok((re, im))
}

/// `ln(z) = (ln|z|, atan2(y, x))`. Near the unit circle `ln|z|` is
/// `ln1p(x^2 + y^2 - 1) / 2`, with the sum taken exactly: the log of a
/// rounded modulus kept six digits of `ln((0.9999996, 3.9e-5))`.
pub(crate) fn ln(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let wide = exact(work);
    let xx = x.mul(x, wide);
    let yy = y.mul(y, wide);
    let excess = xx.sub(&int(1, wide), wide).add(&yy, wide).round_to(work);
    let re = if near_zero(&excess, work) {
        halve(&excess.ln_1p(work)?, work)
    } else {
        halve(&xx.add(&yy, work).ln(work)?, work)
    };
    Ok((re, DecFloat::atan2(y, x, work)?))
}

/// `ln(1 + z) = (ln1p(x (2 + x) + y^2) / 2, atan2(y, 1 + x))`.
pub(crate) fn ln1p(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let wide = exact(work);
    let yy = y.mul(y, wide);
    let shifted = int(1, wide).add(x, wide);
    let excess = x
        .mul(&int(2, wide).add(x, wide), wide)
        .add(&yy, wide)
        .round_to(work);
    let re = if near_zero(&excess, work) {
        halve(&excess.ln_1p(work)?, work)
    } else {
        halve(&shifted.mul(&shifted, wide).add(&yy, work).ln(work)?, work)
    };
    Ok((re, DecFloat::atan2(y, &shifted.round_to(work), work)?))
}

/// `-1/2 <= v <= 1`, where `ln1p(v)` does not cancel.
fn near_zero(v: &DecFloat, work: u32) -> bool {
    let low = DecFloat::from_decimal(BigInt::from(-5), -1, work);
    v.cmp_val(&low) != Ordering::Less && !greater(v, &int(1, work))
}

/// The pieces of `asin` and `acos` for `x >= 0` and `y > 0`, after Hull,
/// Fairgrieve, and Tang: the real part of `asin`, the real part of `acos`,
/// and the magnitude of the imaginary part they share.
fn asin_acos(
    x: &DecFloat,
    y: &DecFloat,
    work: u32,
) -> Result<(DecFloat, DecFloat, DecFloat), CalcError> {
    let one = int(1, work);
    let xp1 = x.add(&one, work);
    let xm1 = x.sub(&one, work);
    let yy = y.mul(y, work);
    let r = xp1.mul(&xp1, work).add(&yy, work).sqrt(work)?;
    let s = xm1.mul(&xm1, work).add(&yy, work).sqrt(work)?;
    let a = halve(&r.add(&s, work), work);
    let b = x.div(&a, work)?;
    let apx = a.add(x, work);
    let r_xp1 = r.add(&xp1, work);

    let cross_b = DecFloat::from_decimal(BigInt::from(6417), -4, work);
    let (asin_re, acos_re) = if !greater(&b, &cross_b) {
        (b.asin(work)?, b.acos(work)?)
    } else {
        // `b` is close to 1, where asin(b) loses half its digits.
        let opposite = if !greater(x, &one) {
            let inner = yy.div(&r_xp1, work)?.add(&s.sub(&xm1, work), work);
            halve(&apx.mul(&inner, work), work).sqrt(work)?
        } else {
            let inner = apx
                .div(&r_xp1, work)?
                .add(&apx.div(&s.add(&xm1, work), work)?, work);
            y.mul(&halve(&inner, work).sqrt(work)?, work)
        };
        (
            x.div(&opposite, work)?.atan(work)?,
            opposite.div(x, work)?.atan(work)?,
        )
    };

    let cross_a = DecFloat::from_decimal(BigInt::from(15), -1, work);
    let im = if !greater(&a, &cross_a) {
        // `a - 1` without the subtraction.
        let first = yy.div(&r_xp1, work)?;
        let second = if x.cmp_val(&one) == Ordering::Less {
            yy.div(&s.sub(&xm1, work), work)?
        } else {
            s.add(&xm1, work)
        };
        let am1 = halve(&first.add(&second, work), work);
        let root = am1.mul(&a.add(&one, work), work).sqrt(work)?;
        am1.add(&root, work).ln_1p(work)?
    } else {
        let root = a.mul(&a, work).sub(&one, work).sqrt(work)?;
        a.add(&root, work).ln(work)?
    };
    Ok((asin_re, acos_re, im))
}

/// `asin(x + iy)` for `y != 0`. The real part takes the sign of `x` and
/// the imaginary part the sign of `y`.
pub(crate) fn asin(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (re, _, im) = asin_acos(&x.abs(), &y.abs(), work)?;
    Ok((
        with_sign(re, x.is_negative()),
        with_sign(im, y.is_negative()),
    ))
}

/// `acos(x + iy)` for `y != 0`. The imaginary part has the opposite sign
/// to `y`.
pub(crate) fn acos(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (_, re, im) = asin_acos(&x.abs(), &y.abs(), work)?;
    let re = if x.is_negative() {
        DecFloat::pi(work).sub(&re, work)
    } else {
        re
    };
    Ok((re, with_sign(im, !y.is_negative())))
}

/// `asinh(z) = -i asin(iz)`. On the imaginary axis the real part takes the
/// sign of `y`, as in PARI/GP.
pub(crate) fn asinh(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    if x.is_zero() {
        let one = int(1, work);
        if !greater(&y.abs(), &one) {
            return Ok((DecFloat::zero(work), y.asin(work)?));
        }
        // acosh|y| as ln1p(t + sqrt(t (t + 2))) with t = |y| - 1.
        let t = y.abs().sub(&one, work);
        let root = t.mul(&t.add(&int(2, work), work), work).sqrt(work)?;
        let re = t.add(&root, work).ln_1p(work)?;
        let im = DecFloat::pi_over(2, work);
        return Ok((
            with_sign(re, y.is_negative()),
            with_sign(im, y.is_negative()),
        ));
    }
    let (im, _, re) = asin_acos(&y.abs(), &x.abs(), work)?;
    Ok((
        with_sign(re, x.is_negative()),
        with_sign(im, y.is_negative()),
    ))
}

/// `acosh(z) = ±i acos(z)`, with the sign that keeps the real part
/// non-negative.
pub(crate) fn acosh(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (angle, im) = acos(x, y, work)?;
    Ok((im.abs(), with_sign(angle, y.is_negative())))
}

/// The two parts of `atanh(a + ib)` for `a >= 0` and `b >= 0`:
/// `ln1p(4a / ((1 - a)^2 + b^2)) / 4` and
/// `atan2(2b, (1 - a)(1 + a) - b^2) / 2`.
fn atanh_parts(a: &DecFloat, b: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let wide = exact(work);
    let one = int(1, wide);
    let gap = one.sub(a, wide);
    let bb = b.mul(b, wide);
    let denom = gap.mul(&gap, wide).add(&bb, work);
    if denom.is_zero() {
        return Err(CalcError::Domain("logarithm of zero"));
    }
    let ratio = a.mul(&int(4, work), work).div(&denom, work)?;
    let re = ratio
        .ln_1p(work)?
        .mul(&DecFloat::from_decimal(BigInt::from(25), -2, work), work);
    let across = gap
        .mul(&one.add(a, wide), wide)
        .sub(&bb, wide)
        .round_to(work);
    let twice = b.mul(&int(2, work), work);
    let im = halve(&DecFloat::atan2(&twice, &across, work)?, work);
    Ok((re, im))
}

/// `atanh(x + iy)` for `y != 0`.
pub(crate) fn atanh(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (re, im) = atanh_parts(&x.abs(), &y.abs(), work)?;
    Ok((
        with_sign(re, x.is_negative()),
        with_sign(im, y.is_negative()),
    ))
}

/// `atan(z) = -i atanh(iz)`. On the imaginary axis the real part takes the
/// sign of `y`, as in PARI/GP.
pub(crate) fn atan(x: &DecFloat, y: &DecFloat, work: u32) -> Result<Pair, CalcError> {
    let (im, re) = atanh_parts(&y.abs(), &x.abs(), work)?;
    let re_negative = if x.is_zero() {
        y.is_negative()
    } else {
        x.is_negative()
    };
    Ok((with_sign(re, re_negative), with_sign(im, y.is_negative())))
}
