//! Decimal floating-point numbers.
//!
//! A value is `mantissa * 10^exponent`. `prec` is the number of significant
//! digits the value was rounded to. Arithmetic rounds to the precision the
//! caller asks for, with a few extra guard digits inside transcendentals.

use crate::error::CalcError;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt;

pub const MIN_PRECISION: u32 = 3;
pub const MAX_PRECISION: u32 = 1000;
pub const DEFAULT_PRECISION: u32 = 12;

#[derive(Clone)]
pub struct DecFloat {
    mantissa: BigInt,
    exponent: i64,
    prec: u32,
}

impl DecFloat {
    pub fn zero(prec: u32) -> Self {
        Self {
            mantissa: BigInt::zero(),
            exponent: 0,
            prec: prec.max(1),
        }
    }

    pub fn one(prec: u32) -> Self {
        Self::from_i64(1, prec)
    }

    pub fn from_i64(n: i64, prec: u32) -> Self {
        Self::from_bigint(&BigInt::from(n), prec)
    }

    /// Exact when `n` has at most `prec` digits. Otherwise rounded.
    pub fn from_bigint(n: &BigInt, prec: u32) -> Self {
        if n.is_zero() {
            return Self::zero(prec);
        }
        Self::from_unnormalized(n.clone(), 0, prec)
    }

    /// Build a float from an exact decimal `mantissa * 10^exponent`.
    /// `prec` is the number of significant digits the user typed or the
    /// operation asked to keep.
    pub fn from_decimal(mantissa: BigInt, exponent: i64, prec: u32) -> Self {
        Self::from_unnormalized(mantissa, exponent, prec)
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.mantissa.is_negative()
    }

    pub fn is_positive(&self) -> bool {
        self.mantissa.is_positive()
    }

    pub fn prec(&self) -> u32 {
        self.prec
    }

    /// Floor of log10(|x|). Zero maps to 0.
    pub fn order(&self) -> i64 {
        if self.mantissa.is_zero() {
            0
        } else {
            self.exponent + digit_count(&self.mantissa) as i64 - 1
        }
    }

    pub fn neg(&self) -> Self {
        Self {
            mantissa: -&self.mantissa,
            exponent: self.exponent,
            prec: self.prec,
        }
    }

    pub fn abs(&self) -> Self {
        if self.is_negative() {
            self.neg()
        } else {
            self.clone()
        }
    }

    pub fn round_to(&self, prec: u32) -> Self {
        let prec = prec.max(1);
        if self.mantissa.is_zero() {
            return Self::zero(prec);
        }
        Self::from_unnormalized(self.mantissa.clone(), self.exponent, prec)
    }

    pub fn add(&self, rhs: &Self, prec: u32) -> Self {
        let work = work_prec(prec) + 2;
        let a = self.round_to(work);
        let b = rhs.round_to(work);
        if a.is_zero() {
            return b.round_to(prec);
        }
        if b.is_zero() {
            return a.round_to(prec);
        }
        let exp_diff = a.exponent - b.exponent;
        if exp_diff > digit_count(&b.mantissa) as i64 + work as i64 + 2 {
            return a.round_to(prec);
        }
        if -exp_diff > digit_count(&a.mantissa) as i64 + work as i64 + 2 {
            return b.round_to(prec);
        }
        let (am, bm, exp) = if a.exponent <= b.exponent {
            let shift = (b.exponent - a.exponent) as u32;
            (a.mantissa, &b.mantissa * pow10(shift), a.exponent)
        } else {
            let shift = (a.exponent - b.exponent) as u32;
            (&a.mantissa * pow10(shift), b.mantissa, b.exponent)
        };
        Self::from_unnormalized(&am + bm, exp, prec)
    }

    pub fn sub(&self, rhs: &Self, prec: u32) -> Self {
        self.add(&rhs.neg(), prec)
    }

    pub fn mul(&self, rhs: &Self, prec: u32) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero(prec);
        }
        let work = work_prec(prec) + 2;
        let a = self.round_to(work);
        let b = rhs.round_to(work);
        let exp = a.exponent.saturating_add(b.exponent);
        Self::from_unnormalized(&a.mantissa * &b.mantissa, exp, prec)
    }

    pub fn div(&self, rhs: &Self, prec: u32) -> Result<Self, CalcError> {
        if rhs.is_zero() {
            return Err(CalcError::DivideByZero);
        }
        if self.is_zero() {
            return Ok(Self::zero(prec));
        }
        let work = work_prec(prec);
        let a = self.round_to(work + 2);
        let b = rhs.round_to(work + 2);
        let scale = work + 2;
        let num = &a.mantissa * pow10(scale);
        let q = div_round(&num, &b.mantissa);
        let exp = a.exponent - b.exponent - scale as i64;
        Ok(Self::from_unnormalized(q, exp, prec))
    }

    pub fn cmp_val(&self, rhs: &Self) -> Ordering {
        match (self.is_zero(), rhs.is_zero()) {
            (true, true) => return Ordering::Equal,
            (true, false) => {
                return if rhs.is_negative() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
            (false, true) => {
                return if self.is_negative() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            _ => {}
        }
        match (self.is_negative(), rhs.is_negative()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        if (self.order() - rhs.order()).abs() > 1 {
            let ord = self.order().cmp(&rhs.order());
            return if self.is_negative() {
                ord.reverse()
            } else {
                ord
            };
        }
        let exp_diff = self.exponent - rhs.exponent;
        let (left, right) = if exp_diff >= 0 {
            (
                &self.mantissa * pow10(exp_diff as u32),
                rhs.mantissa.clone(),
            )
        } else {
            (
                self.mantissa.clone(),
                &rhs.mantissa * pow10((-exp_diff) as u32),
            )
        };
        left.cmp(&right)
    }

    /// Integer toward -infinity.
    pub fn floor_to_bigint(&self) -> BigInt {
        if self.is_zero() {
            return BigInt::zero();
        }
        if self.exponent >= 0 {
            return &self.mantissa * pow10(self.exponent as u32);
        }
        let shift = (-self.exponent) as u32;
        let (q, r) = self.mantissa.div_rem(&pow10(shift));
        if self.is_negative() && !r.is_zero() {
            q - 1
        } else {
            q
        }
    }

    /// Nearest integer, ties away from zero.
    pub fn round_half_away(&self) -> BigInt {
        let half = Self::from_unnormalized(BigInt::from(5), -1, self.prec.max(4));
        let prec = self.prec.max(4) + 2;
        if self.is_negative() {
            -self.abs().add(&half, prec).floor_to_bigint()
        } else {
            self.add(&half, prec).floor_to_bigint()
        }
    }

    /// Exact rational `mantissa * 10^exponent`.
    pub fn to_rational(&self) -> Option<num_rational::BigRational> {
        use num_rational::BigRational;
        if self.is_zero() {
            return Some(BigRational::from(BigInt::zero()));
        }
        if self.exponent >= 0 {
            let exp = u32::try_from(self.exponent).ok()?;
            if exp > 100_000 {
                return None;
            }
            Some(BigRational::from(&self.mantissa * pow10(exp)))
        } else {
            let exp = u32::try_from(-self.exponent).ok()?;
            if exp > 100_000 {
                return None;
            }
            Some(num_rational::BigRational::new(
                self.mantissa.clone(),
                pow10(exp),
            ))
        }
    }

    /// Exact integer when the decimal has no fractional part.
    pub fn to_integer_exact(&self) -> Option<BigInt> {
        if self.is_zero() {
            return Some(BigInt::zero());
        }
        if self.exponent >= 0 {
            let exp = u32::try_from(self.exponent).ok()?;
            if exp > 100_000 {
                return None;
            }
            return Some(&self.mantissa * pow10(exp));
        }
        let shift = u32::try_from(-self.exponent).ok()?;
        if shift > 100_000 {
            return None;
        }
        let (q, r) = self.mantissa.div_rem(&pow10(shift));
        if r.is_zero() { Some(q) } else { None }
    }

    pub fn sqrt(&self, prec: u32) -> Result<Self, CalcError> {
        if self.is_negative() {
            return Err(CalcError::Domain("square root of a negative number"));
        }
        if self.is_zero() {
            return Ok(Self::zero(prec));
        }
        let work = work_prec(prec);
        let x = self.round_to(work);
        let ord = x.order();
        let mut guess = Self::from_unnormalized(BigInt::from(2), ord.div_euclid(2), work);
        if ord.rem_euclid(2) != 0 {
            guess = guess.mul(&Self::from_i64(3, work), work);
        }
        let two = Self::from_i64(2, work);
        for _ in 0..80 {
            let next = guess.add(&x.div(&guess, work)?, work).div(&two, work)?;
            if next.nearly(&guess, work.saturating_sub(1)) {
                guess = next;
                break;
            }
            guess = next;
        }
        Ok(snap_close_integer(guess, prec))
    }

    pub fn exp(&self, prec: u32) -> Result<Self, CalcError> {
        if self.is_zero() {
            return Ok(Self::one(prec));
        }
        if self.order().abs() > 1_000_000_000 {
            return Err(CalcError::Overflow);
        }
        let work = work_prec(prec);
        let extra = (self.order().unsigned_abs() as u32).saturating_add(8);
        let high = work.saturating_add(extra);
        let x = self.round_to(high);
        let ln10 = Self::ln10(high);
        let n = x.div(&ln10, high)?.round_half_away();
        if n.bits() > 62 {
            return Err(CalcError::Overflow);
        }
        let r = x.sub(&ln10.mul(&Self::from_bigint(&n, high), high), high);
        let mut reduced = r;
        let mut halves = 0u32;
        let two = Self::from_i64(2, high);
        let bound = Self::from_unnormalized(BigInt::one(), -1, high); // 0.1
        while reduced.abs().cmp_val(&bound) == Ordering::Greater && halves < 64 {
            reduced = reduced.div(&two, high)?;
            halves += 1;
        }
        let mut y = exp_series(&reduced, high)?;
        for _ in 0..halves {
            y = y.mul(&y, high);
        }
        let n_i = n.to_i64().ok_or(CalcError::Overflow)?;
        let exp = y.exponent.saturating_add(n_i);
        Ok(snap_close_integer(
            Self::from_unnormalized(y.mantissa, exp, high),
            prec,
        ))
    }

    pub fn ln(&self, prec: u32) -> Result<Self, CalcError> {
        if !self.is_positive() {
            return Err(CalcError::Domain("logarithm of a non-positive number"));
        }
        let work = work_prec(prec);
        let mut powered = self.order();
        let mut m = self.round_to(work + 4);
        m.exponent -= powered;
        m.prec = work + 4;
        // Keep the argument of the series inside [1/sqrt(10), sqrt(10)).
        let sqrt10 =
            Self::from_unnormalized(BigInt::from(3_162_277_660_168_379_i64), -15, work + 4);
        if m.cmp_val(&sqrt10) != Ordering::Less {
            m.exponent -= 1;
            powered += 1;
        }
        let one = Self::one(work + 4);
        let y = m
            .sub(&one, work + 4)
            .div(&m.add(&one, work + 4), work + 4)?;
        let ln_m = ln_from_artanh(&y, work + 4)?;
        let scaled = Self::ln10(work + 6).mul(&Self::from_i64(powered, work + 6), work + 6);
        Ok(snap_close_integer(ln_m.add(&scaled, work + 4), prec))
    }

    pub fn sin_cos(&self, prec: u32) -> Result<(Self, Self), CalcError> {
        if self.is_zero() {
            return Ok((Self::zero(prec), Self::one(prec)));
        }
        let work = work_prec(prec);
        let extra = (self.order().unsigned_abs() as u32).saturating_add(12);
        let high = work.saturating_add(extra);
        let half_pi = Self::pi(high).div(&Self::from_i64(2, high), high)?;
        let x = self.round_to(high);
        let q = x.div(&half_pi, high)?.round_half_away();
        let r = x.sub(&half_pi.mul(&Self::from_bigint(&q, high), high), high);
        let (mut s, mut c) = sin_cos_series(&r, high)?;
        let qmod = (&q).mod_floor(&BigInt::from(4));
        match qmod.to_i32().unwrap_or(0) {
            0 => {}
            1 => {
                let ns = c;
                c = s.neg();
                s = ns;
            }
            2 => {
                s = s.neg();
                c = c.neg();
            }
            _ => {
                let ns = c.neg();
                c = s;
                s = ns;
            }
        }
        // A non-zero quadrant multiple can leave a rounding speck where the
        // true value is 0 (sin of pi, cos of pi/2).
        if !q.is_zero() {
            s = snap_noise(s, prec);
            c = snap_noise(c, prec);
        }
        Ok((snap_close_integer(s, prec), snap_close_integer(c, prec)))
    }

    pub fn sin(&self, prec: u32) -> Result<Self, CalcError> {
        Ok(self.sin_cos(prec)?.0)
    }

    pub fn cos(&self, prec: u32) -> Result<Self, CalcError> {
        Ok(self.sin_cos(prec)?.1)
    }

    pub fn tan(&self, prec: u32) -> Result<Self, CalcError> {
        let (s, c) = self.sin_cos(work_prec(prec))?;
        if c.is_zero() {
            return Err(CalcError::Domain("tangent is undefined"));
        }
        Ok(snap_close_integer(s.div(&c, work_prec(prec))?, prec))
    }

    pub fn atan(&self, prec: u32) -> Result<Self, CalcError> {
        if self.is_zero() {
            return Ok(Self::zero(prec));
        }
        if self.is_negative() {
            return Ok(self.neg().atan(prec)?.neg());
        }
        let work = work_prec(prec);
        let one = Self::one(work);
        let x = self.round_to(work);
        if x.cmp_val(&one) == Ordering::Greater {
            let half_pi = Self::pi(work).div(&Self::from_i64(2, work), work)?;
            let inv = one.div(&x, work)?;
            return Ok(snap_close_integer(
                half_pi.sub(&atan_unit(&inv, work)?, work),
                prec,
            ));
        }
        Ok(snap_close_integer(atan_unit(&x, work)?, prec))
    }

    /// `atan2(y, x)`, range (-pi, pi].
    pub fn atan2(y: &Self, x: &Self, prec: u32) -> Result<Self, CalcError> {
        if y.is_zero() && x.is_zero() {
            return Ok(Self::zero(prec));
        }
        if y.is_zero() {
            return if x.is_negative() {
                Ok(Self::pi(prec))
            } else {
                Ok(Self::zero(prec))
            };
        }
        if x.is_zero() {
            let half = Self::pi(prec).div(&Self::from_i64(2, prec), prec)?;
            return Ok(if y.is_positive() { half } else { half.neg() });
        }
        let work = work_prec(prec);
        let slope = y.div(x, work)?;
        let angle = slope.atan(prec)?;
        if x.is_positive() {
            Ok(angle)
        } else if y.is_positive() {
            Ok(angle.add(&Self::pi(prec), prec))
        } else {
            Ok(angle.sub(&Self::pi(prec), prec))
        }
    }

    pub fn asin(&self, prec: u32) -> Result<Self, CalcError> {
        let work = work_prec(prec);
        let one = Self::one(work);
        let x = self.round_to(work);
        if x.abs().cmp_val(&one) == Ordering::Greater {
            return Err(CalcError::Domain("inverse sine argument out of range"));
        }
        if x.abs().cmp_val(&one) == Ordering::Equal {
            let half = Self::pi(prec).div(&Self::from_i64(2, prec), prec)?;
            return Ok(if x.is_negative() { half.neg() } else { half });
        }
        let under = one.sub(&x, work).mul(&one.add(&x, work), work);
        let adjacent = under.sqrt(work)?;
        Self::atan2(&x, &adjacent, prec)
    }

    pub fn acos(&self, prec: u32) -> Result<Self, CalcError> {
        let work = work_prec(prec);
        let half = Self::pi(work).div(&Self::from_i64(2, work), work)?;
        Ok(snap_close_integer(half.sub(&self.asin(work)?, work), prec))
    }

    /// Positive integer power, then one rounding. Negative exponents invert.
    pub fn powi(&self, exp: i64, prec: u32) -> Result<Self, CalcError> {
        if exp == 0 {
            return Ok(Self::one(prec));
        }
        if self.is_zero() {
            return if exp > 0 {
                Ok(Self::zero(prec))
            } else {
                Err(CalcError::DivideByZero)
            };
        }
        let neg = exp < 0;
        let exp = exp.checked_abs().ok_or(CalcError::Overflow)?;
        let bits = 64 - (exp as u64).leading_zeros();
        let work = work_prec(prec).saturating_add(bits);
        let mut result = Self::one(work);
        let mut base = self.round_to(work);
        let mut e = exp as u64;
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul(&base, work);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul(&base, work);
            }
        }
        let result = result.round_to(prec);
        if neg {
            Self::one(prec).div(&result, prec)
        } else {
            Ok(result)
        }
    }

    pub fn pi(prec: u32) -> Self {
        let work = prec.saturating_add(8).max(16);
        let a = atan_series(&recip_int(5, work), work);
        let b = atan_series(&recip_int(239, work), work);
        a.mul(&Self::from_i64(16, work), work)
            .sub(&b.mul(&Self::from_i64(4, work), work), work)
            .round_to(prec)
    }

    pub fn ln10(prec: u32) -> Self {
        // ln(10) = 3 ln(2) + ln(5/4), and ln((1+y)/(1-y)) = 2 artanh(y).
        let work = prec.saturating_add(8).max(16);
        let ln2 = ln_from_artanh(&recip_int(3, work), work).expect("ln2");
        let ln_5_4 = ln_from_artanh(&recip_int(9, work), work).expect("ln(5/4)");
        ln2.mul(&Self::from_i64(3, work), work)
            .add(&ln_5_4, work)
            .round_to(prec)
    }

    /// Round to `prec` and collapse values that sit a hair below an integer.
    pub fn polished(&self, prec: u32) -> Self {
        snap_close_integer(self.clone(), prec)
    }

    pub fn e(prec: u32) -> Self {
        Self::one(prec.saturating_add(4)).exp(prec).expect("exp(1)")
    }

    fn from_unnormalized(mantissa: BigInt, exponent: i64, prec: u32) -> Self {
        let prec = prec.max(1);
        if mantissa.is_zero() {
            return Self::zero(prec);
        }
        let (mut mantissa, adj) = round_mantissa(&mantissa, prec);
        let mut exponent = exponent.saturating_add(adj);
        if mantissa.is_zero() {
            return Self::zero(prec);
        }
        strip_trailing_zeros(&mut mantissa, &mut exponent);
        Self {
            mantissa,
            exponent,
            prec,
        }
    }

    fn nearly(&self, other: &Self, prec: u32) -> bool {
        if self.cmp_val(other) == Ordering::Equal {
            return true;
        }
        let diff = self.sub(other, prec.saturating_add(2));
        if diff.is_zero() {
            return true;
        }
        let scale = self.order().max(other.order());
        diff.order() <= scale - prec as i64 + 1
    }

    fn format_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let neg = if self.is_negative() { "-" } else { "" };
        let digits = self.mantissa.abs().to_str_radix(10);
        let order = self.exponent + digits.len() as i64 - 1;
        if order >= 10 || order <= -5 {
            let mut frac: String = digits.chars().skip(1).collect();
            while frac.ends_with('0') {
                frac.pop();
            }
            let body = if frac.is_empty() {
                digits[..1].to_string()
            } else {
                format!("{}.{}", &digits[..1], frac)
            };
            format!("{neg}{body}e{order}")
        } else {
            format!("{neg}{}", fixed_point(&digits, self.exponent))
        }
    }
}

impl fmt::Display for DecFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_string())
    }
}

impl fmt::Debug for DecFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (man={}, exp={}, prec={})",
            self.format_string(),
            self.mantissa,
            self.exponent,
            self.prec
        )
    }
}

fn fixed_point(digits: &str, exponent: i64) -> String {
    if exponent >= 0 {
        let zeros = exponent as usize;
        return format!("{digits}{}", "0".repeat(zeros));
    }
    let places = (-exponent) as usize;
    if places < digits.len() {
        let split = digits.len() - places;
        let (whole, frac) = digits.split_at(split);
        let frac = frac.trim_end_matches('0');
        if frac.is_empty() {
            whole.to_string()
        } else {
            format!("{whole}.{frac}")
        }
    } else {
        let zeros = places - digits.len();
        let frac = digits.trim_end_matches('0');
        if frac.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}{frac}", "0".repeat(zeros))
        }
    }
}

fn work_prec(prec: u32) -> u32 {
    prec.saturating_add(8).saturating_add(prec / 5).max(16)
}

pub(crate) fn digit_count(n: &BigInt) -> u32 {
    if n.is_zero() {
        1
    } else {
        n.abs().to_str_radix(10).len() as u32
    }
}

fn pow10(n: u32) -> BigInt {
    BigInt::from(10).pow(n)
}

fn div_round(num: &BigInt, den: &BigInt) -> BigInt {
    let (q, r) = num.div_rem(den);
    if r.is_zero() {
        return q;
    }
    if r.abs() * BigInt::from(2) >= den.abs() {
        if num.sign() == den.sign() {
            q + 1
        } else {
            q - 1
        }
    } else {
        q
    }
}

/// Round `m` to `prec` significant digits. The second value is added to the exponent.
fn round_mantissa(m: &BigInt, prec: u32) -> (BigInt, i64) {
    if m.is_zero() {
        return (BigInt::zero(), 0);
    }
    let digits = digit_count(m);
    if digits <= prec {
        return (m.clone(), 0);
    }
    let drop = digits - prec;
    let mut q = div_round(m, &pow10(drop));
    let mut adj = drop as i64;
    let qd = digit_count(&q);
    if qd > prec {
        let extra = qd - prec;
        q = div_round(&q, &pow10(extra));
        adj += extra as i64;
    }
    (q, adj)
}

fn strip_trailing_zeros(mantissa: &mut BigInt, exponent: &mut i64) {
    if mantissa.is_zero() {
        *exponent = 0;
        return;
    }
    let text = mantissa.abs().to_str_radix(10);
    let zeros = text.chars().rev().take_while(|c| *c == '0').count();
    if zeros > 0 {
        *mantissa /= pow10(zeros as u32);
        *exponent += zeros as i64;
    }
}

fn recip_int(n: i64, prec: u32) -> DecFloat {
    DecFloat::one(prec)
        .div(&DecFloat::from_i64(n, prec), prec)
        .expect("reciprocal of a small integer")
}

fn negligible(term: &DecFloat, sum: &DecFloat, prec: u32) -> bool {
    if term.is_zero() {
        return true;
    }
    if sum.is_zero() {
        return false;
    }
    term.order() < sum.order() - prec as i64 - 1
}

fn snap_noise(value: DecFloat, prec: u32) -> DecFloat {
    if value.is_zero() || value.order() < 1 - prec as i64 {
        DecFloat::zero(prec)
    } else {
        value
    }
}

/// Turn a value that is only a rounding speck away from an integer into that
/// integer. `ln(exp(1))` is the case this is aimed at: the correctly rounded
/// exponential sits a fraction of an ulp below `e`, and the log comes back as
/// a string of nines.
fn snap_close_integer(value: DecFloat, prec: u32) -> DecFloat {
    let rounded = value.round_to(prec);
    if rounded.is_zero() {
        return rounded;
    }
    let nearest = rounded.round_half_away();
    let as_float = DecFloat::from_bigint(&nearest, prec.saturating_add(4));
    let diff = rounded.sub(&as_float, prec.saturating_add(4));
    if diff.is_zero() {
        return DecFloat::from_bigint(&nearest, prec);
    }
    let scale = if nearest.is_zero() {
        0
    } else {
        digit_count(&nearest) as i64 - 1
    };
    // Half an ulp at this magnitude is 5 * 10^(scale - prec).
    let half_ulp =
        DecFloat::from_unnormalized(BigInt::from(5), scale - prec as i64, prec.saturating_add(4));
    if diff.abs().cmp_val(&half_ulp) == Ordering::Less {
        DecFloat::from_bigint(&nearest, prec)
    } else {
        rounded
    }
}

fn exp_series(x: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let mut sum = DecFloat::one(prec);
    let mut term = DecFloat::one(prec);
    let limit = prec as usize * 3 + 40;
    for k in 1..=limit {
        term = term
            .mul(x, prec)
            .div(&DecFloat::from_i64(k as i64, prec), prec)?;
        let done = negligible(&term, &sum, prec);
        sum = sum.add(&term, prec);
        if done {
            break;
        }
    }
    Ok(sum)
}

/// `ln((1+y)/(1-y))` = 2 (y + y^3/3 + y^5/5 + ...).
fn ln_from_artanh(y: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let y2 = y.mul(y, prec);
    let mut power = y.round_to(prec);
    let mut sum = power.clone();
    let mut n = 1i64;
    let limit = prec as usize * 4 + 40;
    for _ in 0..limit {
        power = power.mul(&y2, prec);
        n += 2;
        let term = power.div(&DecFloat::from_i64(n, prec), prec)?;
        let done = negligible(&term, &sum, prec);
        sum = sum.add(&term, prec);
        if done {
            break;
        }
    }
    Ok(sum.mul(&DecFloat::from_i64(2, prec), prec))
}

fn atan_series(x: &DecFloat, prec: u32) -> DecFloat {
    let x2 = x.mul(x, prec);
    let mut power = x.round_to(prec);
    let mut sum = power.clone();
    let mut subtract = true;
    let mut n = 1i64;
    let limit = prec as usize * 3 + 40;
    for _ in 0..limit {
        power = power.mul(&x2, prec);
        n += 2;
        let term = match power.div(&DecFloat::from_i64(n, prec), prec) {
            Ok(t) => t,
            Err(_) => break,
        };
        let done = negligible(&term, &sum, prec);
        sum = if subtract {
            sum.sub(&term, prec)
        } else {
            sum.add(&term, prec)
        };
        subtract = !subtract;
        if done {
            break;
        }
    }
    sum
}

/// Arctan of a non-negative argument that is at most 1.
fn atan_unit(x: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let one = DecFloat::one(prec);
    // sqrt(2) - 1, so the remaining series argument stays below ~0.42.
    let thresh = DecFloat::from_unnormalized(BigInt::from(414_213_562_373_i64), -12, prec);
    if x.cmp_val(&thresh) == Ordering::Greater {
        let quarter = DecFloat::pi(prec).div(&DecFloat::from_i64(4, prec), prec)?;
        let t = one.sub(x, prec).div(&one.add(x, prec), prec)?;
        return Ok(quarter.sub(&atan_series(&t, prec), prec));
    }
    Ok(atan_series(x, prec))
}

fn sin_cos_series(x: &DecFloat, prec: u32) -> Result<(DecFloat, DecFloat), CalcError> {
    let x2 = x.mul(x, prec);
    let mut s_term = x.round_to(prec);
    let mut c_term = DecFloat::one(prec);
    let mut s = s_term.clone();
    let mut c = c_term.clone();
    let mut sk = 1i64;
    let mut ck = 0i64;
    let limit = prec as usize * 2 + 40;
    for _ in 0..limit {
        s_term = s_term.mul(&x2, prec).neg();
        s_term = s_term.div(&DecFloat::from_i64((sk + 1) * (sk + 2), prec), prec)?;
        sk += 2;
        c_term = c_term.mul(&x2, prec).neg();
        c_term = c_term.div(&DecFloat::from_i64((ck + 1) * (ck + 2), prec), prec)?;
        ck += 2;
        let s_done = negligible(&s_term, &s, prec);
        let c_done = negligible(&c_term, &c, prec);
        s = s.add(&s_term, prec);
        c = c.add(&c_term, prec);
        if s_done && c_done {
            break;
        }
    }
    Ok((s, c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str, prec: u32) -> DecFloat {
        // Test helper: "3.14", "-0.5", "1e-3", "150".
        let neg = s.starts_with('-');
        let s = s.trim_start_matches('-');
        let (body, exp10) = if let Some((m, e)) = s.split_once(['e', 'E']) {
            (m, e.parse::<i64>().unwrap())
        } else {
            (s, 0)
        };
        let (digits, exp) = if let Some((w, f)) = body.split_once('.') {
            let digs = format!("{w}{f}").trim_start_matches('0').to_string();
            let digs = if digs.is_empty() {
                "0".to_string()
            } else {
                digs
            };
            let exp = exp10 - f.len() as i64;
            (digs, exp)
        } else {
            let digs = body.trim_start_matches('0').to_string();
            let digs = if digs.is_empty() {
                "0".to_string()
            } else {
                digs
            };
            (digs, exp10)
        };
        let mut m = BigInt::parse_bytes(digits.as_bytes(), 10).unwrap();
        if neg {
            m = -m;
        }
        DecFloat::from_unnormalized(m, exp, prec)
    }

    fn assert_starts(value: &DecFloat, prefix: &str) {
        let shown = value.to_string();
        assert!(
            shown.starts_with(prefix),
            "{shown} does not start with {prefix} ({value:?})"
        );
    }

    #[test]
    fn decimal_addition_is_exact() {
        let prec = 12;
        let sum = p("0.1", prec).add(&p("0.2", prec), prec);
        assert_eq!(sum.to_string(), "0.3");
    }

    #[test]
    fn division_and_display() {
        let third = DecFloat::one(12)
            .div(&DecFloat::from_i64(3, 12), 12)
            .unwrap();
        assert_eq!(third.to_string(), "0.333333333333");
        let two_thirds = DecFloat::from_i64(2, 12)
            .div(&DecFloat::from_i64(3, 12), 12)
            .unwrap();
        assert_eq!(two_thirds.to_string(), "0.666666666667");
        assert_eq!(
            DecFloat::from_i64(1, 12)
                .div(&DecFloat::from_i64(2, 12), 12)
                .unwrap()
                .to_string(),
            "0.5"
        );
        assert_eq!(
            DecFloat::from_i64(1, 12)
                .div(&DecFloat::from_i64(8, 12), 12)
                .unwrap()
                .to_string(),
            "0.125"
        );
        assert_eq!(p("75.75", 12).to_string(), "75.75");
        assert_eq!(p("1.5e20", 12).to_string(), "1.5e20");
        assert_eq!(p("1e-6", 12).to_string(), "1e-6");
    }

    #[test]
    fn known_constants() {
        assert_starts(&DecFloat::pi(20), "3.141592653589793");
        assert_starts(&DecFloat::e(20), "2.718281828459045");
        assert_starts(&DecFloat::ln10(20), "2.302585092994045");
        let ln2 = DecFloat::from_i64(2, 20).ln(20).unwrap();
        assert_starts(&ln2, "0.693147180559945");
    }

    #[test]
    fn exp_ln_roundtrip_and_trig() {
        let one = DecFloat::one(16);
        assert_eq!(one.exp(16).unwrap().ln(16).unwrap().to_string(), "1");
        assert_eq!(DecFloat::zero(12).exp(12).unwrap().to_string(), "1");
        assert_eq!(DecFloat::one(12).ln(12).unwrap().to_string(), "0");

        let one_rad = DecFloat::one(16);
        assert_starts(&one_rad.sin(16).unwrap(), "0.84147098480789");
        assert_starts(&one_rad.cos(16).unwrap(), "0.54030230586813");

        let pi = DecFloat::pi(16);
        assert_eq!(pi.sin(16).unwrap().to_string(), "0");
        assert_eq!(pi.cos(16).unwrap().to_string(), "-1");
        let half_pi = pi.div(&DecFloat::from_i64(2, 16), 16).unwrap();
        assert_eq!(half_pi.sin(16).unwrap().to_string(), "1");
        assert_eq!(half_pi.cos(16).unwrap().to_string(), "0");

        let quarter = pi.div(&DecFloat::from_i64(4, 16), 16).unwrap();
        assert_eq!(quarter.tan(16).unwrap().to_string(), "1");
        assert_starts(&DecFloat::one(16).atan(16).unwrap(), "0.78539816339744");
        assert_eq!(
            DecFloat::one(16).asin(16).unwrap().to_string(),
            half_pi.round_to(16).to_string()
        );
    }

    #[test]
    fn sqrt_and_powers() {
        assert_eq!(DecFloat::from_i64(4, 12).sqrt(12).unwrap().to_string(), "2");
        assert_starts(
            &DecFloat::from_i64(2, 16).sqrt(16).unwrap(),
            "1.41421356237309",
        );
        assert_eq!(
            DecFloat::from_i64(2, 12).powi(10, 12).unwrap().to_string(),
            "1024"
        );
        assert_eq!(
            DecFloat::from_i64(2, 12).powi(-3, 12).unwrap().to_string(),
            "0.125"
        );
    }
}
