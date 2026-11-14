//! Vectors, as in Emacs Calc.
//!
//! A vector is `[a, b, c]`, and an element may be a vector itself. Where
//! Calc computes a result, the result follows it: `+` and `-` go element by
//! element, a number times a vector scales it, `*` of two vectors is the dot
//! product, and `[1, 2, 3] 2 ^` is that product with itself (`14`). Where
//! Calc leaves a vector unevaluated, such as `sqrt([4, 9])` or
//! `2^[1, 2, 3]`, the operation goes element by element instead. Products
//! that need a matrix (a vector of vectors) are not supported.

use crate::error::CalcError;
use crate::value::{self, Mode, NOT_FOR_VECTOR, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

pub(crate) const NOT_FOR_MATRIX: CalcError = CalcError::Domain("matrices are not supported");
const SAME_LENGTH: CalcError = CalcError::Domain("vectors need the same length");
const NEGATIVE_POWER: CalcError = CalcError::Domain("a vector has no negative power");
pub(crate) const TOO_DEEP: CalcError = CalcError::Domain("vectors nest too deep");
/// Deepest nesting of vectors. Each level is a recursion when a value is
/// parsed, computed, printed, or dropped.
pub(crate) const MAX_DEPTH: usize = 100;
/// Longest vector `v x` builds. Undo keeps up to 100 copies of the stack,
/// so a much longer one would cost hundreds of megabytes.
pub(crate) const MAX_INDEX: usize = 10_000;

pub(crate) type Unary<'a> = &'a dyn Fn(&Value) -> Result<Value, CalcError>;
pub(crate) type Binary<'a> = &'a dyn Fn(&Value, &Value) -> Result<Value, CalcError>;

/// `op` on every element, at any depth. A number is passed through.
pub(crate) fn map1(v: &Value, op: Unary<'_>) -> Result<Value, CalcError> {
    match v {
        Value::Vector(items) => items
            .iter()
            .map(|item| map1(item, op))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Vector),
        other => op(other),
    }
}

/// `op` element by element. A number meets every element of a vector, and
/// two vectors pair their elements, so they need the same length.
pub(crate) fn map2(a: &Value, b: &Value, op: Binary<'_>) -> Result<Value, CalcError> {
    let items = match (a, b) {
        (Value::Vector(x), Value::Vector(y)) => {
            if x.len() != y.len() {
                return Err(SAME_LENGTH);
            }
            x.iter()
                .zip(y)
                .map(|(x, y)| map2(x, y, op))
                .collect::<Result<Vec<_>, _>>()
        }
        (Value::Vector(x), _) => x.iter().map(|x| map2(x, b, op)).collect(),
        (_, Value::Vector(y)) => y.iter().map(|y| map2(a, y, op)).collect(),
        _ => return op(a, b),
    };
    items.map(Value::Vector)
}

fn flat(items: &[Value]) -> Result<&[Value], CalcError> {
    if items.iter().any(|item| matches!(item, Value::Vector(_))) {
        Err(NOT_FOR_MATRIX)
    } else {
        Ok(items)
    }
}

/// The sum of `x[i] * y[i]`, as Calc's `*` of two vectors. Nothing is
/// conjugated. An empty product is `0`.
fn dot(x: &[Value], y: &[Value], mode: &Mode) -> Result<Value, CalcError> {
    let (x, y) = (flat(x)?, flat(y)?);
    if x.len() != y.len() {
        return Err(SAME_LENGTH);
    }
    let mut sum: Option<Value> = None;
    for (a, b) in x.iter().zip(y) {
        let term = value::mul(a, b, mode)?;
        sum = Some(match sum {
            None => term,
            Some(acc) => value::add(&acc, &term, mode)?,
        });
    }
    Ok(sum.unwrap_or_else(|| Value::from_int(0)))
}

/// `*`. Two vectors give the dot product; anything else scales.
pub(crate) fn mul(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match (a, b) {
        (Value::Vector(x), Value::Vector(y)) => dot(x, y, mode),
        _ => map2(a, b, &|x, y| value::mul(x, y, mode)),
    }
}

/// `/`. A vector divided by a number goes element by element. Dividing by a
/// vector would need its inverse, which Calc does not have either.
pub(crate) fn div(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if matches!(b, Value::Vector(_)) {
        return Err(NOT_FOR_VECTOR);
    }
    map2(a, b, &|x, y| value::div(x, y, mode))
}

/// `^`. A vector to a positive exact integer power repeats Calc's `*`:
/// `v^2` is `v.v`, `v^3` is `(v.v) v`. Calc leaves any other power of a
/// vector unevaluated (`[4, 9]^0.5`, `[1, 2, 3]^2.`, `[1, 2]^0`), so that
/// goes element by element, as does a number to a vector power:
/// `3M7 [1, 2, 3] ^` is `[3 mod 7, 2 mod 7, 6 mod 7]`. A negative integer
/// power would need an inverse, which Calc does not find either.
pub(crate) fn pow(base: &Value, exp: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match (base, exp) {
        (Value::Vector(_), Value::Vector(_)) => Err(NOT_FOR_VECTOR),
        (Value::Vector(items), Value::Real(Real::Int(n))) if !n.is_zero() => {
            vector_power(items, n, mode)
        }
        _ => map2(base, exp, &|x, y| value::pow(x, y, mode)),
    }
}

fn vector_power(items: &[Value], n: &BigInt, mode: &Mode) -> Result<Value, CalcError> {
    if n.is_negative() {
        return Err(NEGATIVE_POWER);
    }
    let items = flat(items)?;
    let half: BigInt = n / 2u32;
    let odd = n.is_odd();
    if half.is_zero() {
        return Ok(Value::Vector(items.to_vec()));
    }
    let squared = dot(items, items, mode)?;
    let scale = if half.is_one() {
        squared
    } else {
        value::pow(&squared, &Value::from_int(half), mode)?
    };
    if odd {
        map2(&scale, &Value::Vector(items.to_vec()), &|x, y| {
            value::mul(x, y, mode)
        })
    } else {
        Ok(scale)
    }
}

/// `I Q`. A vector squares to its dot product with itself.
pub(crate) fn square(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Vector(items) => dot(items, items, mode),
        _ => value::square(v, mode),
    }
}

/// `f A`. The sum of the squared magnitudes, as Calc's `abssqr`.
pub(crate) fn abssqr(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let Value::Vector(items) = v else {
        return value::abssqr(v, mode);
    };
    let mut sum = Value::from_int(0);
    for item in flat(items)? {
        sum = value::add(&sum, &value::abssqr(item, mode)?, mode)?;
    }
    Ok(sum)
}

/// `A`. The length of a vector, as Calc's `abs`: `[3, 4]` is `5`.
pub(crate) fn abs(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Vector(_) => value::sqrt(&abssqr(v, mode)?, mode),
        _ => value::abs(v, mode),
    }
}

/// `&`. A vector has no reciprocal.
pub(crate) fn recip(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Vector(_) => Err(NOT_FOR_VECTOR),
        _ => value::recip(v, mode),
    }
}
