//! Gcd, lcm, factorial, choose, permutations, and the prime test.
//!
//! These stay on the number tower already in use. Extended gcd pushes three
//! integers instead of a vector. Prime factors are left out for the same
//! reason: the result would be a list.

use crate::error::CalcError;
use crate::float::DecFloat;
use crate::value::{Mode, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

const MAX_FACTORIAL: u32 = 100_000;
/// Trial division proves the answer up through this bound, matching Calc.
const TRIAL_LIMIT: u64 = 8_000_000;
/// Thirteen witnesses prove primality below this integer.
const PROVEN_BOUND: &str = "3317044064679887385961981";
const WITNESSES: &[u32] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];
const SMALL_PRIMES: &[u32] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

pub fn gcd(a: &Value, b: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    let (a, b) = rationals(a, b, "gcd needs integers or fractions")?;
    let numer = a.numer().gcd(b.numer());
    let denom = a.denom().lcm(b.denom());
    Ok(Value::from_rational(BigRational::new(numer, denom)))
}

pub fn lcm(a: &Value, b: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    let (a, b) = rationals(a, b, "lcm needs integers or fractions")?;
    let numer = a.numer().lcm(b.numer());
    let denom = a.denom().gcd(b.denom());
    Ok(Value::from_rational(BigRational::new(numer, denom)))
}

/// Bézout coefficients for integers `x` and `y`: `a*x + b*y = g`.
pub fn extended_gcd(x: &Value, y: &Value) -> Result<(BigInt, BigInt, BigInt), CalcError> {
    let x = as_integer(x, "extended gcd needs integers")?;
    let y = as_integer(y, "extended gcd needs integers")?;
    let eg = x.extended_gcd(&y);
    Ok((eg.x, eg.y, eg.gcd))
}

pub fn factorial(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Complex(_, _) => Err(CalcError::Domain("factorial needs a real number")),
        Value::Real(Real::Int(n)) => Ok(Value::from_int(factorial_int(n)?)),
        Value::Real(Real::Rat(r)) if r.denom().is_one() => {
            Ok(Value::from_int(factorial_int(r.numer())?))
        }
        Value::Real(Real::Float(f)) => {
            if let Some(n) = f.to_integer_exact() {
                let exact = factorial_int(&n)?;
                return Ok(Value::real(Real::Float(DecFloat::from_bigint(
                    &exact, mode.prec,
                ))));
            }
            let x = f.round_to(mode.prec);
            Ok(Value::real(Real::Float(factorial_float(&x, mode.prec)?)))
        }
        Value::Real(r) => {
            let x = to_float(r, mode.prec)?;
            Ok(Value::real(Real::Float(factorial_float(&x, mode.prec)?)))
        }
    }
}

/// Binomial coefficient. `n` is second on the stack and `k` is on top.
/// Integers use the product `n! / (k! (n-k)!)`. Other reals use that formula
/// with the gamma function.
pub fn choose(n: &Value, k: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some((n, k)) = pair_integers(n, k) {
        return Ok(Value::from_int(choose_int(&n, &k)?));
    }
    let msg = "choose needs real numbers";
    let work = mode.prec.saturating_add(8);
    let n = as_real_float(n, work, msg)?;
    let k = as_real_float(k, work, msg)?;
    let num = fact_as_float(&n, work)?;
    let den = fact_as_float(&k, work)?.mul(&fact_as_float(&n.sub(&k, work), work)?, work);
    Ok(Value::real(Real::Float(
        num.div(&den, mode.prec)?.polished(mode.prec),
    )))
}

/// Permutations `n! / (n-k)!`. `n` is second on the stack and `k` is on top.
pub fn perm(n: &Value, k: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some((n, k)) = pair_integers(n, k) {
        return perm_int(&n, &k);
    }
    let msg = "perm needs real numbers";
    let work = mode.prec.saturating_add(8);
    let n = as_real_float(n, work, msg)?;
    let k = as_real_float(k, work, msg)?;
    let num = fact_as_float(&n, work)?;
    let den = fact_as_float(&n.sub(&k, work), work)?;
    Ok(Value::real(Real::Float(
        num.div(&den, mode.prec)?.polished(mode.prec),
    )))
}

pub fn prime_message(v: &Value) -> Result<&'static str, CalcError> {
    let n = as_integer(v, "prime test needs an integer")?;
    if n.bits() > 8192 {
        return Err(CalcError::Domain("number is too large"));
    }
    Ok(match classify(&n) {
        PrimeKind::Prime => "prime",
        PrimeKind::NotPrime => "not prime",
        PrimeKind::ProbablyPrime => "probably prime",
    })
}

enum PrimeKind {
    Prime,
    NotPrime,
    ProbablyPrime,
}

fn classify(n: &BigInt) -> PrimeKind {
    if n < &BigInt::from(2) {
        return PrimeKind::NotPrime;
    }
    if let Some(small) = n.to_u64()
        && small < TRIAL_LIMIT
    {
        return if trial_prime(small) {
            PrimeKind::Prime
        } else {
            PrimeKind::NotPrime
        };
    }
    for &p in SMALL_PRIMES {
        let p = BigInt::from(p);
        if n == &p {
            return PrimeKind::Prime;
        }
        if (n % &p).is_zero() {
            return PrimeKind::NotPrime;
        }
    }
    let proven = n < &proven_bound();
    if miller_rabin(n, WITNESSES) {
        if proven {
            PrimeKind::Prime
        } else {
            PrimeKind::ProbablyPrime
        }
    } else {
        PrimeKind::NotPrime
    }
}

fn trial_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    if n.is_multiple_of(3) {
        return n == 3;
    }
    let mut d = 5u64;
    while d.saturating_mul(d) <= n {
        if n.is_multiple_of(d) || n.is_multiple_of(d + 2) {
            return false;
        }
        d += 6;
    }
    true
}

/// Miller-Rabin. `true` means every witness passed.
fn miller_rabin(n: &BigInt, witnesses: &[u32]) -> bool {
    let one = BigInt::one();
    let n1 = n - &one;
    let mut d = n1.clone();
    let mut s = 0u32;
    while d.is_even() {
        d /= 2;
        s += 1;
    }
    for &base in witnesses {
        let a = BigInt::from(base);
        let mut x = a.modpow(&d, n);
        if x == one || x == n1 {
            continue;
        }
        let mut witnessed = false;
        for _ in 1..s {
            x = (&x * &x) % n;
            if x == n1 {
                witnessed = true;
                break;
            }
        }
        if !witnessed {
            return false;
        }
    }
    true
}

fn proven_bound() -> BigInt {
    BigInt::parse_bytes(PROVEN_BOUND.as_bytes(), 10).expect("prime bound")
}

fn choose_int(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
    if k.is_negative() {
        return Ok(BigInt::zero());
    }
    if k.is_zero() {
        return Ok(BigInt::one());
    }
    if n.is_negative() {
        // C(-m, k) = (-1)^k * C(m + k - 1, k).
        let upper = k - n - 1;
        let mag = choose_nonneg(&upper, k)?;
        return Ok(if k.is_odd() { -mag } else { mag });
    }
    choose_nonneg(n, k)
}

fn choose_nonneg(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
    if k > n {
        return Ok(BigInt::zero());
    }
    let rest = n - k;
    let k = if &rest < k { rest } else { k.clone() };
    let steps = k.to_u32().ok_or(CalcError::Overflow)?;
    if steps > MAX_FACTORIAL {
        return Err(CalcError::Overflow);
    }
    let mut acc = BigInt::one();
    for i in 1..=steps {
        acc *= n - &k + i;
        acc /= BigInt::from(i);
    }
    Ok(acc)
}

fn perm_int(n: &BigInt, k: &BigInt) -> Result<Value, CalcError> {
    if k.is_negative() {
        let steps = (-k).to_u32().ok_or(CalcError::Overflow)?;
        if steps > MAX_FACTORIAL {
            return Err(CalcError::Overflow);
        }
        let mut den = BigInt::one();
        for i in 1..=steps {
            let term = n + i;
            if term.is_zero() {
                return Err(CalcError::Domain("permutation is undefined"));
            }
            den *= term;
        }
        return Ok(Value::from_rational(BigRational::new(BigInt::one(), den)));
    }
    let steps = k.to_u32().ok_or(CalcError::Overflow)?;
    if steps > MAX_FACTORIAL {
        return Err(CalcError::Overflow);
    }
    let mut acc = BigInt::one();
    for i in 0..steps {
        acc *= n - i;
        if acc.is_zero() {
            return Ok(Value::from_int(0));
        }
    }
    Ok(Value::from_int(acc))
}

fn fact_as_float(x: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    if let Some(n) = x.to_integer_exact() {
        Ok(DecFloat::from_bigint(&factorial_int(&n)?, prec))
    } else {
        factorial_float(x, prec)
    }
}

fn pair_integers(n: &Value, k: &Value) -> Option<(BigInt, BigInt)> {
    Some((strict_integer(n)?, strict_integer(k)?))
}

fn strict_integer(v: &Value) -> Option<BigInt> {
    match v {
        Value::Real(Real::Int(n)) => Some(n.clone()),
        Value::Real(Real::Rat(r)) if r.denom().is_one() => Some(r.numer().clone()),
        _ => None,
    }
}

fn as_real_float(v: &Value, prec: u32, msg: &'static str) -> Result<DecFloat, CalcError> {
    match v {
        Value::Real(r) => to_float(r, prec),
        Value::Complex(_, _) => Err(CalcError::Domain(msg)),
    }
}

fn factorial_int(n: &BigInt) -> Result<BigInt, CalcError> {
    if n.is_negative() {
        return Err(CalcError::Domain("factorial of a negative integer"));
    }
    let n = n.to_u32().ok_or(CalcError::Overflow)?;
    if n > MAX_FACTORIAL {
        return Err(CalcError::Overflow);
    }
    let mut acc = BigInt::one();
    for k in 2..=n {
        acc *= BigInt::from(k);
    }
    Ok(acc)
}

/// Generalized factorial `gamma(x + 1)` for a non-integer.
fn factorial_float(x: &DecFloat, prec: u32) -> Result<DecFloat, CalcError> {
    if x.order().abs() > 1_000_000 {
        return Err(CalcError::Overflow);
    }
    let a = spouge_terms(prec);
    let work = prec.saturating_add(16).saturating_add(a / 3).max(32);
    let x = x.round_to(work);
    let one = DecFloat::one(work);
    let (shifted, steps) = if x.cmp_val(&one) == Ordering::Greater {
        (x.clone(), 0i64)
    } else {
        let steps = (BigInt::from(2) - x.floor_to_bigint())
            .to_i64()
            .ok_or(CalcError::Overflow)?;
        if !(0..=MAX_FACTORIAL as i64).contains(&steps) {
            return Err(CalcError::Overflow);
        }
        let lifted = x.add(&DecFloat::from_i64(steps, work), work);
        (lifted, steps)
    };
    let mut upper = spouge_factorial(&shifted, work, a)?;
    if steps > 0 {
        let mut denom = one;
        for i in 1..=steps {
            let term = x.add(&DecFloat::from_i64(i, work), work);
            if term.is_zero() {
                return Err(CalcError::Domain("factorial of a negative integer"));
            }
            denom = denom.mul(&term, work);
        }
        upper = upper.div(&denom, work)?;
    }
    Ok(upper.polished(prec))
}

/// Spouge's approximation of `z!` for `z > 0`.
fn spouge_factorial(z: &DecFloat, work: u32, a: u32) -> Result<DecFloat, CalcError> {
    let two_pi = DecFloat::pi(work).mul(&DecFloat::from_i64(2, work), work);
    let mut sum = two_pi.sqrt(work)?;
    let mut fact = BigInt::one();
    for k in 1..a {
        if k > 1 {
            fact *= BigInt::from(k - 1);
        }
        let ak = a - k;
        let power = int_pow(ak, k);
        let root = DecFloat::from_i64(ak as i64, work).sqrt(work)?;
        let scaled = DecFloat::from_i64(ak as i64, work).exp(work)?;
        let magnitude = DecFloat::from_bigint(&power, work)
            .div(&root, work)?
            .div(&DecFloat::from_bigint(&fact, work), work)?
            .mul(&scaled, work);
        let ck = if k % 2 == 1 {
            magnitude
        } else {
            magnitude.neg()
        };
        let denom = z.add(&DecFloat::from_i64(k as i64, work), work);
        sum = sum.add(&ck.div(&denom, work)?, work);
    }
    let a_f = DecFloat::from_i64(a as i64, work);
    let za = z.add(&a_f, work);
    let half = DecFloat::from_decimal(BigInt::from(5), -1, work);
    let exponent = z.add(&half, work).mul(&za.ln(work)?, work);
    Ok(exponent
        .exp(work)?
        .mul(&za.neg().exp(work)?, work)
        .mul(&sum, work))
}

fn spouge_terms(prec: u32) -> u32 {
    let n = (prec as f64) * 1.25 + 24.0;
    n.ceil().clamp(20.0, 4000.0) as u32
}

fn int_pow(base: u32, exp: u32) -> BigInt {
    let mut result = BigInt::one();
    let mut b = BigInt::from(base);
    let mut e = exp;
    while e > 0 {
        if e % 2 == 1 {
            result *= &b;
        }
        e /= 2;
        if e > 0 {
            b = &b * &b;
        }
    }
    result
}

fn rationals(
    a: &Value,
    b: &Value,
    msg: &'static str,
) -> Result<(BigRational, BigRational), CalcError> {
    Ok((as_rational(a, msg)?, as_rational(b, msg)?))
}

fn as_rational(v: &Value, msg: &'static str) -> Result<BigRational, CalcError> {
    match v {
        Value::Real(Real::Int(n)) => Ok(BigRational::from(n.clone())),
        Value::Real(Real::Rat(r)) => Ok(r.clone()),
        Value::Real(Real::Float(f)) => Ok(BigRational::from(
            f.to_integer_exact().ok_or(CalcError::Domain(msg))?,
        )),
        Value::Complex(_, _) => Err(CalcError::Domain(msg)),
    }
}

fn as_integer(v: &Value, msg: &'static str) -> Result<BigInt, CalcError> {
    match v {
        Value::Real(Real::Int(n)) => Ok(n.clone()),
        Value::Real(Real::Rat(r)) if r.denom().is_one() => Ok(r.numer().clone()),
        Value::Real(Real::Float(f)) => f.to_integer_exact().ok_or(CalcError::Domain(msg)),
        _ => Err(CalcError::Domain(msg)),
    }
}

fn to_float(r: &Real, prec: u32) -> Result<DecFloat, CalcError> {
    match r {
        Real::Int(n) => Ok(DecFloat::from_bigint(n, prec)),
        Real::Rat(q) => {
            let n = DecFloat::from_bigint(q.numer(), prec.saturating_add(4));
            let d = DecFloat::from_bigint(q.denom(), prec.saturating_add(4));
            n.div(&d, prec)
        }
        Real::Float(f) => Ok(f.round_to(prec)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode() -> Mode {
        Mode::default()
    }

    #[test]
    fn gcd_and_lcm_of_integers_and_fractions() {
        let a = Value::from_int(12);
        let b = Value::from_int(-18);
        assert_eq!(gcd(&a, &b, &mode()).unwrap().to_string(), "6");
        assert_eq!(lcm(&a, &b, &mode()).unwrap().to_string(), "36");
        assert_eq!(
            gcd(&Value::from_int(0), &Value::from_int(0), &mode())
                .unwrap()
                .to_string(),
            "0"
        );

        let p = Value::from_rational(BigRational::new(2.into(), 3.into()));
        let q = Value::from_rational(BigRational::new(4.into(), 5.into()));
        assert_eq!(gcd(&p, &q, &mode()).unwrap().to_string(), "2:15");
        assert_eq!(lcm(&p, &q, &mode()).unwrap().to_string(), "4");
    }

    #[test]
    fn extended_gcd_satisfies_bezout() {
        let (a, b, g) = extended_gcd(&Value::from_int(35), &Value::from_int(15)).unwrap();
        assert_eq!(g, BigInt::from(5));
        assert_eq!(&a * 35 + &b * 15, g);
        assert_eq!((a, b), (BigInt::from(1), BigInt::from(-2)));
    }

    #[test]
    fn choose_and_perm_match_the_factorial_formulas() {
        let mode = mode();
        let c = |n: i64, k: i64| {
            choose(&Value::from_int(n), &Value::from_int(k), &mode)
                .unwrap()
                .to_string()
        };
        assert_eq!(c(5, 2), "10");
        assert_eq!(c(10, 3), "120");
        assert_eq!(c(5, 0), "1");
        assert_eq!(c(5, 6), "0");
        assert_eq!(c(-3, 2), "6");
        assert_eq!(c(5, -1), "0");

        let p = |n: i64, k: i64| {
            perm(&Value::from_int(n), &Value::from_int(k), &mode)
                .unwrap()
                .to_string()
        };
        assert_eq!(p(5, 2), "20");
        assert_eq!(p(10, 3), "720");
        assert_eq!(p(5, 6), "0");
        assert_eq!(p(-3, 2), "12");
        assert_eq!(p(5, -1), "1:6");

        let half = Value::from_rational(BigRational::new(11.into(), 2.into()));
        assert_eq!(
            perm(&half, &Value::from_int(2), &mode).unwrap().to_string(),
            "24.75"
        );
    }

    #[test]
    fn factorial_integers_and_half() {
        assert_eq!(
            factorial(&Value::from_int(10), &mode())
                .unwrap()
                .to_string(),
            "3628800"
        );
        assert_eq!(
            factorial(&Value::from_int(0), &mode()).unwrap().to_string(),
            "1"
        );
        assert!(factorial(&Value::from_int(-3), &mode()).is_err());

        let half = Value::from_rational(BigRational::new(1.into(), 2.into()));
        assert_eq!(
            factorial(&half, &mode()).unwrap().to_string(),
            "0.886226925453"
        );
        let neg = Value::from_rational(BigRational::new((-1).into(), 2.into()));
        assert_eq!(
            factorial(&neg, &mode()).unwrap().to_string(),
            "1.77245385091"
        );
    }

    #[test]
    fn prime_test_is_exact_for_small_numbers_and_cautious_above_the_bound() {
        assert_eq!(prime_message(&Value::from_int(17)).unwrap(), "prime");
        assert_eq!(prime_message(&Value::from_int(2)).unwrap(), "prime");
        assert_eq!(prime_message(&Value::from_int(1)).unwrap(), "not prime");
        assert_eq!(prime_message(&Value::from_int(561)).unwrap(), "not prime");
        assert_eq!(
            prime_message(&Value::from_int(15_485_863)).unwrap(),
            "prime"
        );
        let composite = BigInt::from(10_007) * BigInt::from(10_009);
        assert_eq!(
            prime_message(&Value::from_int(composite)).unwrap(),
            "not prime"
        );
        let mersenne = (BigInt::one() << 127u32) - 1;
        assert_eq!(
            prime_message(&Value::from_int(mersenne)).unwrap(),
            "probably prime"
        );
    }
}
