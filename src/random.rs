//! Random numbers, as Emacs Calc's `k r`, `k a`, and `k h`.
//!
//! Everything comes from `rand::rng()`, a ChaCha12 generator seeded from
//! the operating system and reseeded every 64 kB, so the numbers are fit
//! for keys and draws. Integers are drawn by rejection, so every value of a
//! range is exactly as likely as any other. Calc reduces a few extra random
//! digits modulo the limit, which leans slightly towards the small values.

use crate::error::CalcError;
use crate::float::{DecFloat, pow10};
use crate::value::{self, Mode, Real, Value};
use crate::vector::MAX_INDEX;
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};
use rand::Rng;
use std::collections::HashSet;

const NEEDS_REAL: CalcError = CalcError::Domain("random needs a real number or a vector");
const EMPTY: CalcError = CalcError::Domain("random of an empty vector");
const COUNT: CalcError = CalcError::Domain("shuffle needs a nonnegative integer count");
const TOO_MANY: CalcError = CalcError::Domain("shuffle asks for more values than there are");

/// A uniform integer in `[0, limit)`. `limit` is positive.
fn below(limit: &BigInt) -> BigInt {
    let bits = limit.bits();
    let bytes = usize::try_from(bits.div_ceil(8)).unwrap_or(usize::MAX);
    let spare = bytes as u64 * 8 - bits;
    let mut rng = rand::rng();
    let mut buffer = vec![0u8; bytes];
    loop {
        rng.fill_bytes(&mut buffer);
        // Keep exactly `bits` bits, so at least half the draws land below.
        if let Some(top) = buffer.first_mut() {
            *top &= 0xffu8 >> spare;
        }
        let n = BigInt::from_bytes_be(Sign::Plus, &buffer);
        if &n < limit {
            return n;
        }
    }
}

/// A uniform float in `[0, 1)` with `prec` random digits.
fn unit(prec: u32) -> DecFloat {
    let digits = below(&pow10(prec));
    DecFloat::from_decimal(digits, -i64::from(prec), prec)
}

/// A standard normal sample, by the Box-Muller transform at guard
/// precision: `sqrt(-2 ln u) cos(2 pi v)` with `u` in `(0, 1]`.
fn gaussian(prec: u32) -> Result<DecFloat, CalcError> {
    let work = prec.saturating_add(10);
    let u = DecFloat::one(work).add(&unit(work).neg(), work);
    let v = unit(work);
    let radius = u
        .ln(work)?
        .mul(&DecFloat::from_bigint(&BigInt::from(-2), work), work)
        .sqrt(work)?;
    let angle = DecFloat::pi(work).mul(&v, work);
    let (_, cos) = angle
        .mul(&DecFloat::from_bigint(&BigInt::from(2), work), work)
        .sin_cos(work)?;
    Ok(radius.mul(&cos, work).round_to(prec))
}

/// `k r`. `limit` is Calc's `random` argument.
pub(crate) fn random(limit: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match limit {
        Value::Vector(items) => {
            if items.is_empty() {
                return Err(EMPTY);
            }
            let at = below(&BigInt::from(items.len()));
            Ok(items[at.to_usize().unwrap_or(0)].clone())
        }
        Value::Real(_) if limit.is_zero() => Ok(Value::Real(Real::Float(gaussian(mode.prec)?))),
        Value::Real(Real::Int(m)) => {
            // `(m, 0]` for a negative limit, as Calc's `math-mod` gives.
            let draw = below(&m.abs());
            Ok(Value::from_int(if m.is_negative() { -draw } else { draw }))
        }
        Value::Real(_) => {
            let u = Value::Real(Real::Float(unit(mode.prec)));
            value::mul(&u, limit, mode)
        }
        Value::Complex(_, _) | Value::Mod(_, _) => Err(NEEDS_REAL),
    }
}

/// `k h`. `count` values drawn from `limit` without repeats, in random
/// order: from `[0, m)` for a positive integer `m`, from `(m, 0]` for a
/// negative one, from the elements of a vector, or `random(limit)` again and
/// again for a float or `0`.
pub(crate) fn shuffle(limit: &Value, count: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let n = value::exact_int(count)
        .and_then(|n| n.to_usize())
        .ok_or(COUNT)?;
    if n > MAX_INDEX {
        return Err(CalcError::Overflow);
    }
    match limit {
        Value::Vector(items) => {
            let picks = pick_indices(&BigInt::from(items.len()), n)?;
            Ok(Value::Vector(
                picks
                    .into_iter()
                    .map(|i| items[i.to_usize().unwrap_or(0)].clone())
                    .collect(),
            ))
        }
        Value::Real(Real::Int(m)) if !m.is_zero() => {
            let picks = pick_indices(&m.abs(), n)?;
            let sign = m.is_negative();
            Ok(Value::Vector(
                picks
                    .into_iter()
                    .map(|i| Value::from_int(if sign { -i } else { i }))
                    .collect(),
            ))
        }
        Value::Real(_) => {
            let mut seen: Vec<Value> = Vec::with_capacity(n);
            let mut keys = HashSet::with_capacity(n);
            while seen.len() < n {
                let v = random(limit, mode)?;
                if keys.insert(v.to_string()) {
                    seen.push(v);
                }
            }
            Ok(Value::Vector(seen))
        }
        Value::Complex(_, _) | Value::Mod(_, _) => Err(NEEDS_REAL),
    }
}

/// `n` distinct integers of `[0, size)`, in random order.
fn pick_indices(size: &BigInt, n: usize) -> Result<Vec<BigInt>, CalcError> {
    if BigInt::from(n) > *size {
        return Err(TOO_MANY);
    }
    // A small range is shuffled in place (the first `n` steps of a
    // Fisher-Yates shuffle). A large one is sampled with a set of the
    // values seen, which needs few retries when `n` is small beside it.
    if let Some(size) = size.to_usize().filter(|&size| size <= 4 * MAX_INDEX) {
        let mut pool: Vec<usize> = (0..size).collect();
        for i in 0..n {
            let j = i + below(&BigInt::from(size - i)).to_usize().unwrap_or(0);
            pool.swap(i, j);
        }
        return Ok(pool[..n].iter().map(|&i| BigInt::from(i)).collect());
    }
    let mut seen = HashSet::with_capacity(n);
    let mut picks = Vec::with_capacity(n);
    while picks.len() < n {
        let draw = below(size);
        if seen.insert(draw.clone()) {
            picks.push(draw);
        }
    }
    Ok(picks)
}
