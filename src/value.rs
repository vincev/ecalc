//! Integers, rationals, decimal floats, and rectangular complex numbers.
//!
//! Exact inputs stay exact. A float appears when the operation needs one
//! (division with fraction mode off, roots, trig, logs). A negative square
//! root, or any other non-real result, becomes `(re, im)`.

use crate::error::CalcError;
use crate::float::{self, DEFAULT_PRECISION, DecFloat};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleMode {
    Degrees,
    Radians,
}

#[derive(Clone, Debug)]
pub struct Mode {
    pub prec: u32,
    pub fraction: bool,
    pub angle: AngleMode,
    /// Display radix, from 2 to 36. Entry without `#` stays decimal.
    pub radix: u32,
    /// Pad integers out to the current word size.
    pub leading_zeros: bool,
    /// Two's complement display for radices 2, 8, and 16.
    pub twos_complement: bool,
    /// Separate displayed integer digits with commas. The fractional part stays plain.
    pub group_digits: bool,
    /// Binary word width. The sign selects the range for a later bitwise
    /// operation; display uses the absolute value. `0` means no fixed width.
    pub word_size: i64,
}

impl Default for Mode {
    fn default() -> Self {
        Self {
            prec: DEFAULT_PRECISION,
            fraction: false,
            angle: AngleMode::Degrees,
            radix: 10,
            leading_zeros: false,
            twos_complement: false,
            group_digits: false,
            word_size: 32,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Real {
    Int(BigInt),
    Rat(BigRational),
    Float(DecFloat),
}

#[derive(Clone, Debug)]
pub enum Value {
    Real(Real),
    Complex(Real, Real),
}

impl Value {
    pub fn from_int(n: impl Into<BigInt>) -> Self {
        Self::Real(Real::Int(n.into()))
    }

    pub fn from_rational(r: BigRational) -> Self {
        Self::Real(real_from_rational(r))
    }

    pub fn real(re: Real) -> Self {
        Self::Real(re)
    }

    /// Drops a zero imaginary part.
    pub fn complex(re: Real, im: Real) -> Self {
        if im.is_zero() {
            Self::Real(re)
        } else {
            Self::Complex(re, im)
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Real(r) => r.is_zero(),
            Self::Complex(re, im) => re.is_zero() && im.is_zero(),
        }
    }

    fn parts(&self) -> (Real, Real) {
        match self {
            Self::Real(r) => (r.clone(), Real::Int(BigInt::zero())),
            Self::Complex(re, im) => (re.clone(), im.clone()),
        }
    }

    fn is_real(&self) -> bool {
        matches!(self, Self::Real(_))
    }
}

impl Real {
    fn is_zero(&self) -> bool {
        match self {
            Self::Int(n) => n.is_zero(),
            Self::Rat(r) => r.is_zero(),
            Self::Float(f) => f.is_zero(),
        }
    }

    fn is_negative(&self) -> bool {
        match self {
            Self::Int(n) => n.is_negative(),
            Self::Rat(r) => r.is_negative(),
            Self::Float(f) => f.is_negative(),
        }
    }

    fn is_positive(&self) -> bool {
        !self.is_zero() && !self.is_negative()
    }

    fn is_one(&self) -> bool {
        match self {
            Self::Int(n) => n.is_one(),
            Self::Rat(r) => r.numer().is_one() && r.denom().is_one(),
            Self::Float(f) => f.to_integer_exact().is_some_and(|n| n.is_one()),
        }
    }

    fn abs(&self) -> Self {
        match self {
            Self::Int(n) => Self::Int(n.abs()),
            Self::Rat(r) => real_from_rational(r.abs()),
            Self::Float(f) => Self::Float(f.abs()),
        }
    }

    fn neg(&self) -> Self {
        match self {
            Self::Int(n) => Self::Int(-n),
            Self::Rat(r) => real_from_rational(-r),
            Self::Float(f) => Self::Float(f.neg()),
        }
    }

    fn to_rational(&self) -> Option<BigRational> {
        match self {
            Self::Int(n) => Some(BigRational::from(n.clone())),
            Self::Rat(r) => Some(r.clone()),
            Self::Float(f) => f.to_rational(),
        }
    }

    fn to_float(&self, prec: u32) -> Result<DecFloat, CalcError> {
        match self {
            Self::Int(n) => Ok(DecFloat::from_bigint(n, prec)),
            Self::Rat(r) => {
                let n = DecFloat::from_bigint(r.numer(), prec.saturating_add(2));
                let d = DecFloat::from_bigint(r.denom(), prec.saturating_add(2));
                n.div(&d, prec)
            }
            Self::Float(f) => Ok(f.round_to(prec)),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Real(r) => write!(f, "{r}"),
            Self::Complex(re, im) => write!(f, "({re}, {im})"),
        }
    }
}

impl fmt::Display for Real {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Rat(r) => {
                if r.is_negative() {
                    write!(f, "-{}:{}", r.numer().abs(), r.denom())
                } else {
                    write!(f, "{}:{}", r.numer(), r.denom())
                }
            }
            Self::Float(x) => write!(f, "{x}"),
        }
    }
}

fn real_from_rational(r: BigRational) -> Real {
    if r.denom().is_one() {
        Real::Int(r.into_raw().0)
    } else {
        Real::Rat(r)
    }
}

fn exact_int(r: &Real) -> Option<BigInt> {
    match r {
        Real::Int(n) => Some(n.clone()),
        Real::Rat(q) if q.denom().is_one() => Some(q.numer().clone()),
        Real::Float(f) => f.to_integer_exact(),
        _ => None,
    }
}

fn polish(v: Value, prec: u32) -> Value {
    match v {
        Value::Real(Real::Float(f)) => Value::Real(Real::Float(f.polished(prec))),
        Value::Complex(re, im) => {
            let re = polish_real(re, prec);
            let im = polish_real(im, prec);
            finish_complex(re, im, prec)
        }
        other => other,
    }
}

fn polish_real(r: Real, prec: u32) -> Real {
    match r {
        Real::Float(f) => Real::Float(f.polished(prec)),
        other => other,
    }
}

fn finish_complex(re: Real, im: Real, prec: u32) -> Value {
    if im.is_zero() {
        return Value::Real(re);
    }
    if let (Real::Float(rf), Real::Float(imf)) = (&re, &im) {
        let limit = prec as i64;
        if !rf.is_zero() && imf.order() < rf.order() - limit {
            return Value::Real(re);
        }
        if rf.is_zero() || rf.order() < imf.order() - limit {
            return Value::Complex(Real::Int(BigInt::zero()), im);
        }
    }
    Value::Complex(re, im)
}

pub fn neg(v: &Value) -> Value {
    match v {
        Value::Real(r) => Value::Real(r.neg()),
        Value::Complex(re, im) => Value::complex(re.neg(), im.neg()),
    }
}

pub fn add(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let (ar, ai) = a.parts();
    let (br, bi) = b.parts();
    let re = add_real(&ar, &br, mode)?;
    let im = add_real(&ai, &bi, mode)?;
    Ok(finish_complex(re, im, mode.prec))
}

pub fn sub(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    add(a, &neg(b), mode)
}

pub fn mul(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let (ar, ai) = a.parts();
    let (br, bi) = b.parts();
    if ai.is_zero() && bi.is_zero() {
        return Ok(Value::Real(mul_real(&ar, &br, mode)?));
    }
    // (ar*br - ai*bi, ar*bi + ai*br)
    let re = sub_real(&mul_real(&ar, &br, mode)?, &mul_real(&ai, &bi, mode)?, mode)?;
    let im = add_real(&mul_real(&ar, &bi, mode)?, &mul_real(&ai, &br, mode)?, mode)?;
    Ok(finish_complex(re, im, mode.prec))
}

pub fn div(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if b.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    let (br, bi) = b.parts();
    if bi.is_zero() {
        let (ar, ai) = a.parts();
        let re = div_real(&ar, &br, mode)?;
        let im = div_real(&ai, &br, mode)?;
        return Ok(finish_complex(re, im, mode.prec));
    }
    // Multiply by the conjugate and divide by the norm.
    let norm = add_real(&mul_real(&br, &br, mode)?, &mul_real(&bi, &bi, mode)?, mode)?;
    let conj = Value::complex(br, bi.neg());
    let num = mul(a, &conj, mode)?;
    div(&num, &Value::Real(norm), mode)
}

pub fn fdiv(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if b.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    if let (Value::Real(ar), Value::Real(br)) = (a, b)
        && let (Some(x), Some(y)) = (ar.to_rational(), br.to_rational())
    {
        return Ok(Value::from_rational(x / y));
    }
    div(a, b, mode)
}

pub fn idiv(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let q = floor_quot(a, b, mode)?;
    Ok(Value::from_int(q))
}

pub fn modulo(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if !a.is_real() || !b.is_real() {
        return Err(CalcError::Domain("mod needs real numbers"));
    }
    let q = Value::from_int(floor_quot(a, b, mode)?);
    sub(a, &mul(&q, b, mode)?, mode)
}

fn floor_quot(a: &Value, b: &Value, mode: &Mode) -> Result<BigInt, CalcError> {
    if b.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    if let (Value::Real(Real::Int(x)), Value::Real(Real::Int(y))) = (a, b) {
        return Ok(x.div_floor(y));
    }
    let (Value::Real(ar), Value::Real(br)) = (a, b) else {
        return Err(CalcError::Domain("integer division needs real numbers"));
    };
    let q = ar
        .to_float(mode.prec)?
        .div(&br.to_float(mode.prec)?, mode.prec)?;
    Ok(q.floor_to_bigint())
}

fn add_real(a: &Real, b: &Real, mode: &Mode) -> Result<Real, CalcError> {
    if let (Some(x), Some(y)) = (a.to_rational(), b.to_rational())
        && !matches!(a, Real::Float(_))
        && !matches!(b, Real::Float(_))
    {
        return Ok(real_from_rational(x + y));
    }
    Ok(Real::Float(
        a.to_float(mode.prec)?
            .add(&b.to_float(mode.prec)?, mode.prec),
    ))
}

fn sub_real(a: &Real, b: &Real, mode: &Mode) -> Result<Real, CalcError> {
    add_real(a, &b.neg(), mode)
}

fn mul_real(a: &Real, b: &Real, mode: &Mode) -> Result<Real, CalcError> {
    if let (Some(x), Some(y)) = (a.to_rational(), b.to_rational())
        && !matches!(a, Real::Float(_))
        && !matches!(b, Real::Float(_))
    {
        return Ok(real_from_rational(x * y));
    }
    Ok(Real::Float(
        a.to_float(mode.prec)?
            .mul(&b.to_float(mode.prec)?, mode.prec),
    ))
}

fn div_real(a: &Real, b: &Real, mode: &Mode) -> Result<Real, CalcError> {
    if b.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    if let (Some(x), Some(y)) = (a.to_rational(), b.to_rational())
        && !matches!(a, Real::Float(_))
        && !matches!(b, Real::Float(_))
    {
        let q = x / y;
        if q.denom().is_one() {
            return Ok(Real::Int(q.into_raw().0));
        }
        if mode.fraction {
            return Ok(Real::Rat(q));
        }
        let n = DecFloat::from_bigint(q.numer(), mode.prec);
        let d = DecFloat::from_bigint(q.denom(), mode.prec);
        return Ok(Real::Float(n.div(&d, mode.prec)?));
    }
    Ok(Real::Float(
        a.to_float(mode.prec)?
            .div(&b.to_float(mode.prec)?, mode.prec)?,
    ))
}

pub fn recip(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    div(&Value::from_int(1), v, mode)
}

pub fn square(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    mul(v, v, mode)
}

pub fn pow(base: &Value, exp: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(n) = real_exponent_int(exp) {
        return pow_int(base, &n, mode);
    }
    if let Value::Real(r) = exp
        && let Some(q) = r.to_rational()
    {
        if !matches!(r, Real::Float(_)) {
            return pow_rational(base, &q, mode);
        }
        // A short decimal such as 0.5 is an exact rational too.
        if matches!(r, Real::Float(f) if f.prec() <= 12) {
            return pow_rational(base, &q, mode);
        }
    }
    pow_exp_ln(base, exp, mode)
}

fn real_exponent_int(exp: &Value) -> Option<BigInt> {
    match exp {
        Value::Real(r) => exact_int(r),
        Value::Complex(_, _) => None,
    }
}

fn pow_int(base: &Value, exp: &BigInt, mode: &Mode) -> Result<Value, CalcError> {
    if exp.is_zero() {
        return Ok(Value::from_int(1));
    }
    if base.is_zero() {
        return if exp.is_positive() {
            Ok(Value::from_int(0))
        } else {
            Err(CalcError::DivideByZero)
        };
    }
    if exp.bits() > 20 {
        return Err(CalcError::Overflow);
    }
    let negative = exp.is_negative();
    let mut e = exp.abs();
    let mut result = Value::from_int(1);
    let mut factor = base.clone();
    while e.is_positive() {
        if e.is_odd() {
            result = mul(&result, &factor, mode)?;
        }
        e >>= 1;
        if e.is_positive() {
            factor = mul(&factor, &factor, mode)?;
        }
    }
    if negative {
        div(&Value::from_int(1), &result, mode)
    } else {
        Ok(result)
    }
}

fn pow_rational(base: &Value, exp: &BigRational, mode: &Mode) -> Result<Value, CalcError> {
    if exp.denom().is_one() {
        return pow_int(base, exp.numer(), mode);
    }
    let rooted = nth_root(base, exp.denom(), mode)?;
    pow_int(&rooted, exp.numer(), mode)
}

pub fn nth_root(base: &Value, n: &BigInt, mode: &Mode) -> Result<Value, CalcError> {
    if n.is_zero() {
        return Err(CalcError::Domain("root degree must not be zero"));
    }
    if n.is_negative() {
        let root = nth_root(base, &n.abs(), mode)?;
        return recip(&root, mode);
    }
    if n.is_one() {
        return Ok(base.clone());
    }
    let Some(degree) = n.to_u32() else {
        return pow_exp_ln(
            base,
            &Value::Real(real_from_rational(BigRational::new(
                BigInt::one(),
                n.clone(),
            ))),
            mode,
        );
    };
    if let Value::Real(r) = base {
        if r.is_negative() && !n.is_odd() {
            return pow_exp_ln(
                base,
                &Value::from_rational(BigRational::new(BigInt::one(), n.clone())),
                mode,
            );
        }
        if r.is_negative() {
            let mag = nth_root(&Value::Real(r.abs()), n, mode)?;
            return Ok(neg(&mag));
        }
        if r.is_zero() {
            return Ok(Value::from_int(0));
        }
        if let Some(q) = r.to_rational()
            && !matches!(r, Real::Float(_))
            && let Some(root) = rational_nth_root(&q, degree)
        {
            return Ok(Value::from_rational(root));
        }
        let f = r.to_float(mode.prec)?;
        let ln = f.ln(mode.prec)?;
        let scaled = ln.div(&DecFloat::from_bigint(n, mode.prec), mode.prec)?;
        return Ok(polish(
            Value::Real(Real::Float(scaled.exp(mode.prec)?)),
            mode.prec,
        ));
    }
    pow_exp_ln(
        base,
        &Value::from_rational(BigRational::new(BigInt::one(), n.clone())),
        mode,
    )
}

fn rational_nth_root(q: &BigRational, n: u32) -> Option<BigRational> {
    let numer = q.numer().abs();
    let (num_exact, num) = int_nth_root(&numer, n)?;
    let (den_exact, den) = int_nth_root(q.denom(), n)?;
    if num_exact && den_exact {
        let mut root = BigRational::new(num, den);
        if q.is_negative() {
            root = -root;
        }
        Some(root)
    } else {
        None
    }
}

/// Floor of the nth root, and whether it was exact. `a` is non-negative.
fn int_nth_root(a: &BigInt, n: u32) -> Option<(bool, BigInt)> {
    if a.is_negative() || n == 0 {
        return None;
    }
    if a.is_zero() || a.is_one() || n == 1 {
        return Some((true, a.clone()));
    }
    let digs = float::digit_count(a);
    let guess_digits = digs.div_ceil(n);
    let mut guess = BigInt::from(10).pow(guess_digits.max(1));
    for _ in 0..10_000 {
        let p = if n == 1 {
            BigInt::one()
        } else {
            guess.pow(n - 1)
        };
        if p.is_zero() {
            break;
        }
        let next = (&guess * (n - 1) + (a / &p)) / BigInt::from(n);
        if next >= guess {
            let exact = guess.pow(n) == *a;
            return Some((exact, guess));
        }
        guess = next;
    }
    None
}

fn pow_exp_ln(base: &Value, exp: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let l = ln(base, mode)?;
    let p = mul(exp, &l, mode)?;
    exp_value(&p, mode)
}

pub fn sqrt(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) if r.is_negative() => {
            let mag = sqrt(&Value::Real(r.abs()), mode)?;
            let Value::Real(im) = mag else {
                return Ok(mag);
            };
            Ok(Value::complex(Real::Int(BigInt::zero()), im))
        }
        Value::Real(_) => nth_root(v, &BigInt::from(2), mode),
        Value::Complex(re, im) => {
            let x = re.to_float(mode.prec)?;
            let y = im.to_float(mode.prec)?;
            let radius = float_hypot(&x, &y, mode.prec)?;
            let two = DecFloat::from_i64(2, mode.prec);
            let re_s = radius
                .add(&x, mode.prec + 2)
                .div(&two, mode.prec + 2)?
                .sqrt(mode.prec)?;
            let mut im_s = radius
                .sub(&x, mode.prec + 2)
                .div(&two, mode.prec + 2)?
                .sqrt(mode.prec)?;
            if y.is_negative() {
                im_s = im_s.neg();
            }
            Ok(finish_complex(
                Real::Float(re_s),
                Real::Float(im_s),
                mode.prec,
            ))
        }
    }
}

pub fn abs(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) => Ok(Value::Real(r.abs())),
        Value::Complex(re, im) => Ok(Value::Real(hypot_real(re, im, mode)?)),
    }
}

pub fn abssqr(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(_) => square(v, mode),
        Value::Complex(re, im) => {
            let a = mul_real(re, re, mode)?;
            let b = mul_real(im, im, mode)?;
            Ok(Value::Real(add_real(&a, &b, mode)?))
        }
    }
}

pub fn sign(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) if r.is_zero() => Ok(Value::from_int(0)),
        Value::Real(r) if r.is_negative() => Ok(Value::from_int(-1)),
        Value::Real(_) => Ok(Value::from_int(1)),
        Value::Complex(_, _) => div(v, &abs(v, mode)?, mode),
    }
}

pub fn hypot(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if !a.is_real() || !b.is_real() {
        return Err(CalcError::Domain("hypot needs real numbers"));
    }
    let Value::Real(ar) = a else { unreachable!() };
    let Value::Real(br) = b else { unreachable!() };
    Ok(Value::Real(hypot_real(ar, br, mode)?))
}

fn hypot_real(a: &Real, b: &Real, mode: &Mode) -> Result<Real, CalcError> {
    if a.is_zero() {
        return Ok(b.abs());
    }
    if b.is_zero() {
        return Ok(a.abs());
    }
    if let (Some(x), Some(y)) = (a.to_rational(), b.to_rational())
        && !matches!(a, Real::Float(_))
        && !matches!(b, Real::Float(_))
    {
        let sum = &x * &x + &y * &y;
        if let Some(root) = rational_nth_root(&sum, 2) {
            return Ok(real_from_rational(root));
        }
    }
    let af = a.to_float(mode.prec)?;
    let bf = b.to_float(mode.prec)?;
    Ok(Real::Float(float_hypot(&af, &bf, mode.prec)?))
}

fn float_hypot(a: &DecFloat, b: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let scale = if a.abs().cmp_val(&b.abs()) == Ordering::Less {
        b.abs()
    } else {
        a.abs()
    };
    let work = prec + 4;
    let x = a.div(&scale, work)?;
    let y = b.div(&scale, work)?;
    let sum = x.mul(&x, work).add(&y.mul(&y, work), work);
    Ok(sum.sqrt(work)?.mul(&scale, prec).polished(prec))
}

/// Integer toward minus infinity. An integer stays that integer.
/// `45.3` is `45` and `-45.3` is `-46`.
pub fn floor(v: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    let Value::Real(real) = v else {
        return Err(CalcError::Domain("floor needs a real number"));
    };
    Ok(Value::from_int(floor_real(real)?))
}

fn floor_real(real: &Real) -> Result<BigInt, CalcError> {
    match real {
        Real::Int(n) => Ok(n.clone()),
        Real::Rat(r) => Ok(floor_rat(r)),
        Real::Float(f) => floor_float(f),
    }
}

fn floor_float(f: &DecFloat) -> Result<BigInt, CalcError> {
    if f.is_zero() || (!f.is_negative() && f.order() < 0) {
        return Ok(BigInt::zero());
    }
    if f.order() < 0 {
        return Ok(BigInt::from(-1));
    }
    // Same size bound as an exact integer conversion.
    if f.order() > 100_000 {
        return Err(CalcError::Overflow);
    }
    Ok(f.floor_to_bigint())
}

pub fn isqrt(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let Value::Real(r) = v else {
        return Err(CalcError::Domain("isqrt needs a real number"));
    };
    if r.is_negative() {
        return Err(CalcError::Domain("isqrt of a negative number"));
    }
    if let Some(n) = exact_int(r) {
        let (_, root) = int_nth_root(&n, 2).ok_or(CalcError::Overflow)?;
        return Ok(Value::from_int(root));
    }
    let f = r.to_float(mode.prec)?.sqrt(mode.prec)?;
    Ok(Value::from_int(f.floor_to_bigint()))
}

pub fn conj(v: &Value) -> Value {
    match v {
        Value::Real(_) => v.clone(),
        Value::Complex(re, im) => Value::complex(re.clone(), im.neg()),
    }
}

pub fn arg(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if v.is_zero() {
        return Err(CalcError::Domain("argument of zero"));
    }
    let (re, im) = v.parts();
    atan2(&Value::Real(im), &Value::Real(re), mode)
}

pub fn ln(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) if r.is_one() => Ok(Value::from_int(0)),
        Value::Real(r) if r.is_zero() => Err(CalcError::Domain("logarithm of zero")),
        Value::Real(r) if r.is_positive() => Ok(polish(
            Value::Real(Real::Float(r.to_float(mode.prec)?.ln(mode.prec)?)),
            mode.prec,
        )),
        Value::Real(r) => {
            let mag = ln(&Value::Real(r.abs()), mode)?;
            let pi = Real::Float(DecFloat::pi(mode.prec));
            let Value::Real(re) = mag else {
                return Ok(mag);
            };
            Ok(Value::complex(re, pi))
        }
        Value::Complex(_, _) => {
            let mag = abs(v, mode)?;
            let re = ln(&mag, mode)?;
            let (x, y) = v.parts();
            let angle = atan2_radians(&y, &x, mode.prec)?;
            let Value::Real(re) = re else {
                return Ok(re);
            };
            Ok(finish_complex(re, Real::Float(angle), mode.prec))
        }
    }
}

pub fn exp_value(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) if r.is_zero() => Ok(Value::from_int(1)),
        Value::Real(r) => Ok(polish(
            Value::Real(Real::Float(r.to_float(mode.prec)?.exp(mode.prec)?)),
            mode.prec,
        )),
        Value::Complex(re, im) => {
            let e = re.to_float(mode.prec)?.exp(mode.prec + 2)?;
            let (s, c) = im.to_float(mode.prec)?.sin_cos(mode.prec + 2)?;
            let re_o = e.mul(&c, mode.prec);
            let im_o = e.mul(&s, mode.prec);
            Ok(finish_complex(
                Real::Float(re_o),
                Real::Float(im_o),
                mode.prec,
            ))
        }
    }
}

pub fn expm1(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Value::Real(r) = v
        && let Some(f) = (!r.is_zero()).then(|| r.to_float(mode.prec)).transpose()?
        && f.order() < 0
    {
        return Ok(polish(
            Value::Real(Real::Float(exp_minus_one_series(&f, mode.prec)?)),
            mode.prec,
        ));
    }
    sub(&exp_value(v, mode)?, &Value::from_int(1), mode)
}

pub fn lnp1(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    add(v, &Value::from_int(1), mode).and_then(|s| ln(&s, mode))
}

pub fn log10(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(k) = power_of_ten(v) {
        return Ok(Value::from_int(k));
    }
    let l = ln(v, mode)?;
    let base = Value::Real(Real::Float(DecFloat::ln10(mode.prec)));
    div(&l, &base, mode)
}

pub fn exp10(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    pow(&Value::from_int(10), v, mode)
}

pub fn log(x: &Value, base: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let (Some(xv), Some(bv)) = (positive_int(x), positive_int(base))
        && bv > BigInt::one()
    {
        let k = ilog_int(&xv, &bv)?;
        if bv.pow(u32::try_from(&k).unwrap_or(0)) == xv {
            return Ok(Value::from_int(k));
        }
    }
    let num = ln(x, mode)?;
    let den = ln(base, mode)?;
    div(&num, &den, mode)
}

pub fn ilog(x: &Value, base: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let (Some(xv), Some(bv)) = (positive_int(x), positive_int(base)) {
        return Ok(Value::from_int(ilog_int(&xv, &bv)?));
    }
    let q = log(x, base, mode)?;
    let Value::Real(r) = q else {
        return Err(CalcError::Domain("integer log needs real numbers"));
    };
    Ok(Value::from_int(r.to_float(mode.prec)?.floor_to_bigint()))
}

fn positive_int(v: &Value) -> Option<BigInt> {
    match v {
        Value::Real(r) => exact_int(r).filter(|n| n.is_positive()),
        Value::Complex(_, _) => None,
    }
}

fn power_of_ten(v: &Value) -> Option<BigInt> {
    let n = positive_int(v)?;
    let s = n.to_str_radix(10);
    if s.starts_with('1') && s.chars().skip(1).all(|c| c == '0') {
        Some(BigInt::from(s.len() as i64 - 1))
    } else {
        None
    }
}

/// Largest `k` with `base^k <= x`.
///
/// The powers are `base^(2^i)`, so the work follows the size of `x` rather
/// than the size of the logarithm. A capped linear scan used to return
/// `1000000` for anything larger.
fn ilog_int(x: &BigInt, base: &BigInt) -> Result<BigInt, CalcError> {
    if !x.is_positive() || *base <= BigInt::one() {
        return Err(CalcError::Domain(
            "integer log needs a positive integer and a base greater than 1",
        ));
    }
    if x < base {
        return Ok(BigInt::zero());
    }
    let mut powers = vec![base.clone()];
    loop {
        let square = {
            let last = powers.last().expect("base power");
            last * last
        };
        if &square > x {
            break;
        }
        powers.push(square);
    }
    let mut acc = BigInt::one();
    let mut result = BigInt::zero();
    for (i, power) in powers.iter().enumerate().rev() {
        let next = &acc * power;
        if &next <= x {
            acc = next;
            result += BigInt::one() << i;
        }
    }
    Ok(result)
}

pub fn sin(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.angle == AngleMode::Degrees {
        if let Some(n) = real_exact_int(v)
            && degree_mod(&n).is_multiple_of(&BigInt::from(90))
        {
            return Ok(quadrant_sin(&n));
        }
    } else if real_is_zero(v) {
        return Ok(Value::from_int(0));
    }
    trig_complex(v, mode, true)
}

pub fn cos(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.angle == AngleMode::Degrees {
        if let Some(n) = real_exact_int(v)
            && degree_mod(&n).is_multiple_of(&BigInt::from(90))
        {
            return Ok(quadrant_cos(&n));
        }
    } else if real_is_zero(v) {
        return Ok(Value::from_int(1));
    }
    // cos(x) = sin(x + 90°) only for a real, in the current angle mode.
    // A complex value uses the direct formula, so it never builds that shift.
    if v.is_real() {
        let shifted = add(v, &quarter_turn(mode)?, mode)?;
        return sin(&shifted, mode);
    }
    trig_complex(v, mode, false)
}

pub fn tan(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.angle == AngleMode::Degrees
        && let Some(n) = real_exact_int(v)
        && degree_mod(&n).is_multiple_of(&BigInt::from(45))
    {
        return tan_exact_degrees(&n);
    }
    let s = sin(v, mode)?;
    let c = cos(v, mode)?;
    if c.is_zero() {
        return Err(CalcError::Domain("tangent is undefined"));
    }
    div(&s, &c, mode)
}

fn degree_mod(deg: &BigInt) -> BigInt {
    deg.mod_floor(&BigInt::from(360))
}

fn quadrant_sin(deg: &BigInt) -> Value {
    let m = deg.mod_floor(&BigInt::from(360));
    let q = ((&m / BigInt::from(90)) % BigInt::from(4))
        .to_i32()
        .unwrap_or(0);
    Value::from_int(match q {
        0 => 0,
        1 => 1,
        2 => 0,
        _ => -1,
    })
}

fn quadrant_cos(deg: &BigInt) -> Value {
    let m = deg.mod_floor(&BigInt::from(360));
    let q = ((&m / BigInt::from(90)) % BigInt::from(4))
        .to_i32()
        .unwrap_or(0);
    Value::from_int(match q {
        0 => 1,
        1 => 0,
        2 => -1,
        _ => 0,
    })
}

fn tan_exact_degrees(deg: &BigInt) -> Result<Value, CalcError> {
    let m = deg.mod_floor(&BigInt::from(360));
    let m = m.to_i32().unwrap_or(-1);
    match m {
        0 | 180 => Ok(Value::from_int(0)),
        45 | 225 => Ok(Value::from_int(1)),
        135 | 315 => Ok(Value::from_int(-1)),
        90 | 270 => Err(CalcError::Domain("tangent is undefined")),
        _ => unreachable!("caller checks multiples"),
    }
}

fn real_exact_int(v: &Value) -> Option<BigInt> {
    match v {
        Value::Real(r) => exact_int(r),
        Value::Complex(_, _) => None,
    }
}

fn real_is_zero(v: &Value) -> bool {
    matches!(v, Value::Real(r) if r.is_zero())
}

fn quarter_turn(mode: &Mode) -> Result<Value, CalcError> {
    match mode.angle {
        AngleMode::Degrees => Ok(Value::from_int(90)),
        AngleMode::Radians => {
            let half = DecFloat::pi(mode.prec).div(&DecFloat::from_i64(2, mode.prec), mode.prec)?;
            Ok(Value::Real(Real::Float(half)))
        }
    }
}

fn trig_complex(v: &Value, mode: &Mode, want_sin: bool) -> Result<Value, CalcError> {
    let radians = to_radians(v, mode)?;
    let (x, y) = radians.parts();
    let xf = x.to_float(mode.prec)?;
    let yf = y.to_float(mode.prec)?;
    if yf.is_zero() {
        let (s, c) = xf.sin_cos(mode.prec)?;
        return Ok(polish(
            Value::Real(Real::Float(if want_sin { s } else { c })),
            mode.prec,
        ));
    }
    let (s, c) = xf.sin_cos(mode.prec + 2)?;
    let sh = sinh_float(&yf, mode.prec + 2)?;
    let ch = cosh_float(&yf, mode.prec + 2)?;
    let (re, im) = if want_sin {
        (s.mul(&ch, mode.prec), c.mul(&sh, mode.prec))
    } else {
        (c.mul(&ch, mode.prec), s.neg().mul(&sh, mode.prec))
    };
    Ok(finish_complex(Real::Float(re), Real::Float(im), mode.prec))
}

fn to_radians(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.angle == AngleMode::Radians {
        return match v {
            Value::Real(r) => Ok(Value::Real(Real::Float(high_float(r, mode)?))),
            Value::Complex(re, im) => Ok(Value::complex(
                Real::Float(high_float(re, mode)?),
                Real::Float(high_float(im, mode)?),
            )),
        };
    }
    if let Some(n) = real_exact_int(v) {
        let mut m = n.mod_floor(&BigInt::from(360));
        if m > BigInt::from(180) {
            m -= 360;
        }
        let deg = DecFloat::from_bigint(&m, mode.prec + 8);
        return Ok(Value::Real(Real::Float(degrees_to_radians(
            &deg,
            mode.prec + 8,
        )?)));
    }
    let factor = degrees_to_radians(&DecFloat::one(mode.prec + 8), mode.prec + 8)?;
    mul(v, &Value::Real(Real::Float(factor)), mode)
}

fn degrees_to_radians(deg: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let pi = DecFloat::pi(prec);
    deg.mul(&pi, prec).div(&DecFloat::from_i64(180, prec), prec)
}

fn high_float(r: &Real, mode: &Mode) -> Result<DecFloat, CalcError> {
    let digits = match r {
        Real::Int(n) => float::digit_count(n),
        Real::Rat(q) => float::digit_count(q.numer()).max(float::digit_count(q.denom())),
        Real::Float(f) => f.prec().max(f.order().unsigned_abs().min(10_000) as u32),
    };
    let prec = digits.max(mode.prec + 8).min(mode.prec.saturating_add(200));
    r.to_float(prec)
}

pub fn asin(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(n) = real_exact_int(v) {
        if n.is_zero() {
            return Ok(Value::from_int(0));
        }
        if n.is_one() {
            return quarter_turn(mode);
        }
        if n == BigInt::from(-1) {
            return Ok(neg(&quarter_turn(mode)?));
        }
    }
    if let Value::Real(r) = v {
        // Eight digits separates `|z| > 1` from a value that rounds to 1.
        let one = DecFloat::one(8);
        if r.abs().to_float(8)?.cmp_val(&one) != Ordering::Greater {
            let rad = r.to_float(mode.prec)?.asin(mode.prec)?;
            return from_radians_real(rad, mode);
        }
    }
    // asin(z) = -i ln(i z + sqrt(1 - z^2)), result still in radians.
    let i = Value::complex(Real::Int(BigInt::zero()), Real::Int(BigInt::one()));
    let z2 = square(v, mode)?;
    let one_minus = sub(&Value::from_int(1), &z2, mode)?;
    let root = sqrt(&one_minus, mode)?;
    let iz = mul(&i, v, mode)?;
    let inside = add(&iz, &root, mode)?;
    let l = ln(&inside, mode)?;
    let neg_i = neg(&i);
    let rad = mul(&neg_i, &l, mode)?;
    from_radians(rad, mode)
}

pub fn acos(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(n) = real_exact_int(v) {
        if n.is_one() {
            return Ok(Value::from_int(0));
        }
        if n.is_zero() {
            return quarter_turn(mode);
        }
        if n == BigInt::from(-1) {
            return match mode.angle {
                AngleMode::Degrees => Ok(Value::from_int(180)),
                AngleMode::Radians => Ok(Value::Real(Real::Float(DecFloat::pi(mode.prec)))),
            };
        }
    }
    let half = quarter_turn(mode)?;
    sub(&half, &asin(v, mode)?, mode)
}

pub fn atan(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(n) = real_exact_int(v) {
        if n.is_zero() {
            return Ok(Value::from_int(0));
        }
        if n.is_one() {
            return match mode.angle {
                AngleMode::Degrees => Ok(Value::from_int(45)),
                AngleMode::Radians => {
                    let q = DecFloat::pi(mode.prec)
                        .div(&DecFloat::from_i64(4, mode.prec), mode.prec)?;
                    Ok(Value::Real(Real::Float(q)))
                }
            };
        }
        if n == BigInt::from(-1) {
            return Ok(neg(&atan(&Value::from_int(1), mode)?));
        }
    }
    if let Value::Real(r) = v {
        let rad = r.to_float(mode.prec)?.atan(mode.prec)?;
        return from_radians_real(rad, mode);
    }
    // (ln(1 + i z) - ln(1 - i z)) / (2 i)
    let i = Value::complex(Real::Int(BigInt::zero()), Real::Int(BigInt::one()));
    let iz = mul(&i, v, mode)?;
    let a = ln(&add(&Value::from_int(1), &iz, mode)?, mode)?;
    let b = ln(&sub(&Value::from_int(1), &iz, mode)?, mode)?;
    let diff = sub(&a, &b, mode)?;
    let two_i = Value::complex(Real::Int(BigInt::zero()), Real::Int(BigInt::from(2)));
    let rad = div(&diff, &two_i, mode)?;
    from_radians(rad, mode)
}

pub fn atan2(y: &Value, x: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if !y.is_real() || !x.is_real() {
        return Err(CalcError::Domain("atan2 needs real numbers"));
    }
    let Value::Real(yr) = y else { unreachable!() };
    let Value::Real(xr) = x else { unreachable!() };
    // On an axis the angle is an exact multiple of a right angle.
    // The series leaves a few ulps, which is enough to miss 90°.
    if yr.is_zero() || xr.is_zero() {
        return atan2_axis(yr, xr, mode);
    }
    let rad = atan2_radians(yr, xr, mode.prec)?;
    from_radians_real(rad, mode)
}

fn atan2_axis(y: &Real, x: &Real, mode: &Mode) -> Result<Value, CalcError> {
    // 0, 1, 2, or -1 right angles. atan2(0, negative) is +180°, not -180°.
    let right_angles: i32 = if y.is_zero() {
        if x.is_negative() { 2 } else { 0 }
    } else if y.is_positive() {
        1
    } else {
        -1
    };
    match mode.angle {
        AngleMode::Degrees => Ok(Value::from_int(i64::from(right_angles) * 90)),
        AngleMode::Radians => {
            if right_angles == 0 {
                return Ok(Value::from_int(0));
            }
            let pi = DecFloat::pi(mode.prec);
            let mag = if right_angles.abs() == 2 {
                pi
            } else {
                pi.div(&DecFloat::from_i64(2, mode.prec), mode.prec)?
            };
            let signed = if right_angles < 0 { mag.neg() } else { mag };
            Ok(Value::Real(Real::Float(signed)))
        }
    }
}

fn atan2_radians(y: &Real, x: &Real, prec: u32) -> Result<DecFloat, CalcError> {
    let yf = y.to_float(prec)?;
    let xf = x.to_float(prec)?;
    DecFloat::atan2(&yf, &xf, prec)
}

fn from_radians_real(rad: DecFloat, mode: &Mode) -> Result<Value, CalcError> {
    from_radians(Value::Real(Real::Float(rad)), mode)
}

fn from_radians(v: Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.angle == AngleMode::Radians {
        return Ok(polish(v, mode.prec));
    }
    let pi = DecFloat::pi(mode.prec + 4);
    let scale = DecFloat::from_i64(180, mode.prec + 4).div(&pi, mode.prec + 4)?;
    let scaled = mul(&v, &Value::Real(Real::Float(scale)), mode)?;
    Ok(polish(scaled, mode.prec))
}

pub fn sinh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    hyperbolic(v, mode, Hyper::Sinh)
}

pub fn cosh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    hyperbolic(v, mode, Hyper::Cosh)
}

pub fn tanh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let s = sinh(v, mode)?;
    let c = cosh(v, mode)?;
    div(&s, &c, mode)
}

pub fn asinh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    // ln(z + sqrt(z^2 + 1))
    let inside = add(&square(v, mode)?, &Value::from_int(1), mode)?;
    let sum = add(v, &sqrt(&inside, mode)?, mode)?;
    ln(&sum, mode)
}

pub fn acosh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    // ln(z + sqrt(z - 1) sqrt(z + 1))
    let minus = sqrt(&sub(v, &Value::from_int(1), mode)?, mode)?;
    let plus = sqrt(&add(v, &Value::from_int(1), mode)?, mode)?;
    let sum = add(v, &mul(&minus, &plus, mode)?, mode)?;
    ln(&sum, mode)
}

pub fn atanh(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let num = ln(&add(&Value::from_int(1), v, mode)?, mode)?;
    let den = ln(&sub(&Value::from_int(1), v, mode)?, mode)?;
    let diff = sub(&num, &den, mode)?;
    div(&diff, &Value::from_int(2), mode)
}

enum Hyper {
    Sinh,
    Cosh,
}

fn hyperbolic(v: &Value, mode: &Mode, kind: Hyper) -> Result<Value, CalcError> {
    let (x, y) = v.parts();
    let xf = x.to_float(mode.prec)?;
    let yf = y.to_float(mode.prec)?;
    if yf.is_zero() {
        let z = match kind {
            Hyper::Sinh => sinh_float(&xf, mode.prec)?,
            Hyper::Cosh => cosh_float(&xf, mode.prec)?,
        };
        return Ok(polish(Value::Real(Real::Float(z)), mode.prec));
    }
    let sh = sinh_float(&xf, mode.prec + 2)?;
    let ch = cosh_float(&xf, mode.prec + 2)?;
    let (s, c) = yf.sin_cos(mode.prec + 2)?;
    let (re, im) = match kind {
        Hyper::Sinh => (sh.mul(&c, mode.prec), ch.mul(&s, mode.prec)),
        Hyper::Cosh => (ch.mul(&c, mode.prec), sh.mul(&s, mode.prec)),
    };
    Ok(finish_complex(Real::Float(re), Real::Float(im), mode.prec))
}

fn sinh_float(y: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    if y.is_zero() {
        return Ok(DecFloat::zero(prec));
    }
    if y.abs().cmp_val(&DecFloat::one(prec)) == Ordering::Greater {
        let work = prec + 4;
        let e = y.exp(work)?;
        let inv = DecFloat::one(work).div(&e, work)?;
        return Ok(e
            .sub(&inv, work)
            .div(&DecFloat::from_i64(2, work), prec)?
            .polished(prec));
    }
    // y + y^3/3! + y^5/5! + ...
    let y2 = y.mul(y, prec + 4);
    let mut term = y.round_to(prec + 4);
    let mut sum = term.clone();
    let mut k = 1i64;
    for _ in 0..(prec as usize + 20) {
        term = term.mul(&y2, prec + 4);
        let denom = (k + 1) * (k + 2);
        term = term.div(&DecFloat::from_i64(denom, prec + 4), prec + 4)?;
        k += 2;
        sum = sum.add(&term, prec + 4);
        if term.is_zero() || term.order() < sum.order() - prec as i64 - 2 {
            break;
        }
    }
    Ok(sum.polished(prec))
}

fn cosh_float(y: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    if y.is_zero() {
        return Ok(DecFloat::one(prec));
    }
    if y.abs().cmp_val(&DecFloat::one(prec)) == Ordering::Greater {
        let work = prec + 4;
        let e = y.exp(work)?;
        let inv = DecFloat::one(work).div(&e, work)?;
        return Ok(e
            .add(&inv, work)
            .div(&DecFloat::from_i64(2, work), prec)?
            .polished(prec));
    }
    let y2 = y.mul(y, prec + 4);
    let mut term = DecFloat::one(prec + 4);
    let mut sum = term.clone();
    let mut k = 0i64;
    for _ in 0..(prec as usize + 20) {
        term = term.mul(&y2, prec + 4);
        let denom = (k + 1) * (k + 2);
        term = term.div(&DecFloat::from_i64(denom, prec + 4), prec + 4)?;
        k += 2;
        sum = sum.add(&term, prec + 4);
        if term.order() < sum.order() - prec as i64 - 2 {
            break;
        }
    }
    Ok(sum.polished(prec))
}

fn exp_minus_one_series(x: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    let mut term = x.round_to(prec + 4);
    let mut sum = term.clone();
    for k in 2..(prec as i64 + 30) {
        term = term
            .mul(x, prec + 4)
            .div(&DecFloat::from_i64(k, prec + 4), prec + 4)?;
        sum = sum.add(&term, prec + 4);
        if term.order() < sum.order() - prec as i64 - 2 {
            break;
        }
    }
    Ok(sum)
}

pub fn pi(mode: &Mode) -> Value {
    Value::Real(Real::Float(DecFloat::pi(mode.prec)))
}

pub fn e_const(mode: &Mode) -> Value {
    Value::Real(Real::Float(DecFloat::e(mode.prec)))
}

pub fn to_float(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) => Ok(Value::Real(Real::Float(r.to_float(mode.prec)?))),
        Value::Complex(re, im) => Ok(Value::complex(
            Real::Float(re.to_float(mode.prec)?),
            Real::Float(im.to_float(mode.prec)?),
        )),
    }
}

pub fn to_fraction(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Real(r) => Ok(Value::Real(real_to_fraction(r, mode.prec)?)),
        Value::Complex(re, im) => Ok(Value::complex(
            real_to_fraction(re, mode.prec)?,
            real_to_fraction(im, mode.prec)?,
        )),
    }
}

fn real_to_fraction(r: &Real, prec: u32) -> Result<Real, CalcError> {
    match r {
        Real::Int(_) | Real::Rat(_) => Ok(r.clone()),
        Real::Float(f) => {
            let Some(exact) = f.to_rational() else {
                return Err(CalcError::Overflow);
            };
            let tol = half_ulp_rational(f, prec);
            Ok(real_from_rational(approximate(exact, tol)))
        }
    }
}

fn half_ulp_rational(f: &DecFloat, prec: u32) -> BigRational {
    let exp = f.order() - prec as i64;
    five_times_ten(exp)
}

fn five_times_ten(exp: i64) -> BigRational {
    if exp >= 0 {
        BigRational::from(BigInt::from(5) * BigInt::from(10).pow(exp as u32))
    } else {
        BigRational::new(BigInt::from(5), BigInt::from(10).pow((-exp) as u32))
    }
}

fn approximate(target: BigRational, tol: BigRational) -> BigRational {
    if target.is_zero() {
        return target;
    }
    let negative = target.is_negative();
    let target = target.abs();
    let mut x = target.clone();
    let mut h0 = BigInt::zero();
    let mut h1 = BigInt::one();
    let mut k0 = BigInt::one();
    let mut k1 = BigInt::zero();
    for _ in 0..80 {
        let a = floor_rat(&x);
        let h = &a * &h1 + &h0;
        let k = &a * &k1 + &k0;
        let approx = BigRational::new(h.clone(), k.clone());
        let err = (&approx - &target).abs();
        h0 = h1;
        k0 = k1;
        h1 = h;
        let too_big = k.bits() > 10_000;
        k1 = k;
        if err <= tol || too_big {
            return signed(approx, negative);
        }
        let frac = &x - BigRational::from(a.clone());
        if frac.is_zero() {
            return signed(approx, negative);
        }
        x = frac.recip();
    }
    signed(BigRational::new(h1, k1), negative)
}

fn signed(r: BigRational, negative: bool) -> BigRational {
    if negative { -r } else { r }
}

fn floor_rat(r: &BigRational) -> BigInt {
    let (q, rem) = r.numer().div_rem(r.denom());
    if rem.is_zero() || !r.numer().is_negative() {
        q
    } else {
        q - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode() -> Mode {
        Mode::default()
    }

    #[test]
    fn fraction_and_negative_sqrt() {
        let eight = Value::from_int(8);
        let six = Value::from_int(6);
        let mut frac = mode();
        frac.fraction = true;
        assert_eq!(div(&eight, &six, &frac).unwrap().to_string(), "4:3");
        assert_eq!(
            div(&eight, &six, &mode()).unwrap().to_string(),
            "1.33333333333"
        );
        assert_eq!(
            sqrt(&Value::from_int(-1), &mode()).unwrap().to_string(),
            "(0, 1)"
        );
        assert_eq!(abs(&parse_cplx(), &mode()).unwrap().to_string(), "5");
    }

    #[test]
    fn floor_toward_minus_infinity() {
        let pos = Real::Float(DecFloat::from_decimal(BigInt::from(453), -1, 12));
        let neg = Real::Float(DecFloat::from_decimal(BigInt::from(-453), -1, 12));
        assert_eq!(floor(&Value::Real(pos), &mode()).unwrap().to_string(), "45");
        assert_eq!(
            floor(&Value::Real(neg), &mode()).unwrap().to_string(),
            "-46"
        );
        assert!(matches!(
            floor(&Value::from_int(-45), &mode()).unwrap(),
            Value::Real(Real::Int(n)) if n == BigInt::from(-45)
        ));

        let half = Value::from_rational(BigRational::new(BigInt::from(-91), BigInt::from(2)));
        assert_eq!(floor(&half, &mode()).unwrap().to_string(), "-46");

        // More digits than the working precision, so this has to stay rational.
        let numer = BigInt::from(10).pow(30) + 1u32;
        let thirds = (BigInt::from(10).pow(30) - 1u32) / 3u32;
        let q = Value::from_rational(BigRational::new(numer.clone(), BigInt::from(3)));
        assert!(matches!(
            floor(&q, &mode()).unwrap(),
            Value::Real(Real::Int(n)) if n == thirds
        ));
        let nq = Value::from_rational(BigRational::new(-numer, BigInt::from(3)));
        let neg_expected = -(&thirds + 1u32);
        assert!(matches!(
            floor(&nq, &mode()).unwrap(),
            Value::Real(Real::Int(n)) if n == neg_expected
        ));

        let tiny = Real::Float(DecFloat::from_decimal(BigInt::from(-1), -20, 12));
        assert_eq!(
            floor(&Value::Real(tiny), &mode()).unwrap().to_string(),
            "-1"
        );
        let huge = Real::Float(DecFloat::from_decimal(BigInt::one(), 100_001, 12));
        assert!(matches!(
            floor(&Value::Real(huge), &mode()),
            Err(CalcError::Overflow)
        ));
        assert!(matches!(
            floor(&parse_cplx(), &mode()),
            Err(CalcError::Domain("floor needs a real number"))
        ));
    }

    fn parse_cplx() -> Value {
        Value::complex(Real::Int(BigInt::from(3)), Real::Int(BigInt::from(4)))
    }

    #[test]
    fn powers_and_complex_mul() {
        assert_eq!(
            pow(&Value::from_int(2), &Value::from_int(10), &mode())
                .unwrap()
                .to_string(),
            "1024"
        );
        assert_eq!(
            nth_root(&Value::from_int(125), &BigInt::from(3), &mode())
                .unwrap()
                .to_string(),
            "5"
        );
        assert_eq!(
            nth_root(&Value::from_int(-8), &BigInt::from(3), &mode())
                .unwrap()
                .to_string(),
            "-2"
        );
        let i = Value::complex(Real::Int(BigInt::zero()), Real::Int(BigInt::one()));
        assert_eq!(mul(&i, &i, &mode()).unwrap().to_string(), "-1");
    }

    #[test]
    fn degree_trig_and_fraction_recovery() {
        assert_eq!(
            sin(&Value::from_int(30), &mode()).unwrap().to_string(),
            "0.5"
        );
        assert_eq!(sin(&Value::from_int(90), &mode()).unwrap().to_string(), "1");
        assert_eq!(
            sin(&Value::from_int(180), &mode()).unwrap().to_string(),
            "0"
        );
        assert_eq!(
            cos(&Value::from_int(180), &mode()).unwrap().to_string(),
            "-1"
        );
        assert_eq!(tan(&Value::from_int(45), &mode()).unwrap().to_string(), "1");
        assert_eq!(
            atan(&Value::from_int(1), &mode()).unwrap().to_string(),
            "45"
        );
        assert_eq!(
            asin(&Value::from_int(1), &mode()).unwrap().to_string(),
            "90"
        );
        let third = div(&Value::from_int(1), &Value::from_int(3), &mode()).unwrap();
        assert_eq!(to_fraction(&third, &mode()).unwrap().to_string(), "1:3");
    }

    #[test]
    fn adding_zero_keeps_the_value() {
        let samples = [
            Value::from_int(0),
            Value::from_int(-42),
            Value::from_int(1_000_000),
            Value::from_rational(BigRational::new(BigInt::from(2), BigInt::from(3))),
            Value::from_rational(BigRational::new(BigInt::from(-4), BigInt::from(3))),
            Value::real(Real::Float(DecFloat::from_decimal(BigInt::from(1), -1, 12))),
            Value::complex(Real::Int(BigInt::from(3)), Real::Int(BigInt::from(-4))),
        ];
        let zero = Value::from_int(0);
        for value in &samples {
            let sum = add(value, &zero, &mode()).unwrap();
            assert_eq!(sum.to_string(), value.to_string());
            let sum = add(&zero, value, &mode()).unwrap();
            assert_eq!(sum.to_string(), value.to_string());
        }
    }

    #[test]
    fn exp_then_ln_returns_the_input() {
        let mut precise = mode();
        precise.prec = 16;
        for n in [-2, -1, 0, 1, 2] {
            let x = Value::from_int(n);
            let back = ln(&exp_value(&x, &precise).unwrap(), &precise).unwrap();
            assert_eq!(back.to_string(), x.to_string(), "ln(exp({n}))");
        }
    }

    #[test]
    fn fraction_survives_a_float_round_trip() {
        let samples = [
            (1, 2, "1:2"),
            (1, 3, "1:3"),
            (2, 7, "2:7"),
            (22, 7, "22:7"),
            (-4, 3, "-4:3"),
        ];
        for (numer, denom, shown) in samples {
            let q =
                Value::from_rational(BigRational::new(BigInt::from(numer), BigInt::from(denom)));
            assert_eq!(q.to_string(), shown);
            let float = to_float(&q, &mode()).unwrap();
            let back = to_fraction(&float, &mode()).unwrap();
            assert_eq!(back.to_string(), shown);
        }
    }

    #[test]
    fn repeating_products_stay_unpolished() {
        let third = div(&Value::from_int(1), &Value::from_int(3), &mode()).unwrap();
        let almost_one = mul(&third, &Value::from_int(3), &mode()).unwrap();
        assert_eq!(almost_one.to_string(), "0.999999999999");

        let root = sqrt(&Value::from_int(2), &mode()).unwrap();
        let almost_two = mul(&root, &root, &mode()).unwrap();
        assert_eq!(almost_two.to_string(), "1.99999999999");
    }

    #[test]
    fn integer_log_is_exact_past_a_million() {
        assert_eq!(
            ilog(&Value::from_int(1000), &Value::from_int(10), &mode())
                .unwrap()
                .to_string(),
            "3"
        );
        assert_eq!(
            ilog(&Value::from_int(999), &Value::from_int(10), &mode())
                .unwrap()
                .to_string(),
            "2"
        );
        assert_eq!(
            ilog(&Value::from_int(1), &Value::from_int(10), &mode())
                .unwrap()
                .to_string(),
            "0"
        );
        let huge = Value::from_int(BigInt::one() << 1_000_001);
        assert_eq!(
            ilog(&huge, &Value::from_int(2), &mode())
                .unwrap()
                .to_string(),
            "1000001"
        );
        let just_under = Value::from_int((BigInt::one() << 1_000_001) - 1);
        assert_eq!(
            ilog(&just_under, &Value::from_int(2), &mode())
                .unwrap()
                .to_string(),
            "1000000"
        );
        assert!(ilog(&Value::from_int(0), &Value::from_int(2), &mode()).is_err());
        assert!(ilog(&Value::from_int(8), &Value::from_int(1), &mode()).is_err());
    }
}
