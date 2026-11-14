//! Vectors, as in Emacs Calc.
//!
//! A vector is `[a, b, c]`, and an element may be a vector itself. Where
//! Calc computes a result, the result follows it: `+` and `-` go element by
//! element, a number times a vector scales it, `*` of two vectors is the dot
//! product, and `[1, 2, 3] 2 ^` is that product with itself (`14`). Where
//! Calc leaves a vector unevaluated, such as `sqrt([4, 9])` or
//! `2^[1, 2, 3]`, the operation goes element by element instead. A vector
//! of rows of one length is a matrix, and `matrix.rs` has its products,
//! inverse, and powers.

use crate::error::CalcError;
use crate::matrix::{self, NOT_A_MATRIX, Shape};
use crate::value::{self, Mode, NOT_FOR_VECTOR, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

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
        Err(NOT_A_MATRIX)
    } else {
        Ok(items)
    }
}

/// The sum of `x[i] * y[i]`, as Calc's `*` of two vectors. Nothing is
/// conjugated. An empty product is `0`. The products are kept in full and
/// the sum is rounded once.
pub(crate) fn dot(x: &[Value], y: &[Value], mode: &Mode) -> Result<Value, CalcError> {
    let (x, y) = (flat(x)?, flat(y)?);
    if x.len() != y.len() {
        return Err(SAME_LENGTH);
    }
    matrix::sum_products(x.iter().zip(y), mode)
}

/// `*`. Two flat vectors give the dot product, and a matrix with a matrix
/// or a flat vector gives the matrix product. A number scales.
pub(crate) fn mul(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match (a, b) {
        (Value::Vector(_), Value::Vector(_)) => matrix::mul(a, b, mode),
        _ => map2(a, b, &|x, y| value::mul(x, y, mode)),
    }
}

/// `/`. A vector divided by a number goes element by element. Dividing by
/// a square matrix solves a linear system, as in Calc. Dividing by a flat
/// vector would need its inverse, which Calc does not have either.
pub(crate) fn div(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match matrix::shape(b) {
        Shape::Number => map2(a, b, &|x, y| value::div(x, y, mode)),
        Shape::Matrix(_, _) => matrix::divide(a, b, mode),
        Shape::Flat(_) => Err(NOT_FOR_VECTOR),
        Shape::Ragged => Err(NOT_A_MATRIX),
    }
}

/// `^`. A flat vector to a positive exact integer power repeats Calc's
/// `*`: `v^2` is `v.v`, `v^3` is `(v.v) v`. A square matrix to any exact
/// integer power is the matrix power. Calc leaves any other power of a
/// vector unevaluated (`[4, 9]^0.5`, `[1, 2, 3]^2.`, `[1, 2]^0`), so that
/// goes element by element, as does a number to a vector power:
/// `3M7 [1, 2, 3] ^` is `[3 mod 7, 2 mod 7, 6 mod 7]`. A negative integer
/// power of a flat vector would need an inverse, which Calc does not find
/// either.
pub(crate) fn pow(base: &Value, exp: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match (base, exp) {
        (Value::Vector(_), Value::Vector(_)) => Err(NOT_FOR_VECTOR),
        (Value::Vector(items), Value::Real(Real::Int(n))) => match matrix::shape(base) {
            Shape::Matrix(_, _) => matrix::power(base, n, mode),
            _ if n.is_zero() => map2(base, exp, &|x, y| value::pow(x, y, mode)),
            _ => vector_power(items, n, mode),
        },
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

/// `I Q`. A flat vector squares to its dot product with itself, and a
/// matrix to its product with itself.
pub(crate) fn square(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        Value::Vector(_) => matrix::mul(v, v, mode),
        _ => value::square(v, mode),
    }
}

/// `f A`. The sum of the squared magnitudes, as Calc's `abssqr`. A matrix
/// sums over all its elements.
pub(crate) fn abssqr(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let Value::Vector(items) = v else {
        return value::abssqr(v, mode);
    };
    let rows = match matrix::shape(v) {
        Shape::Matrix(_, _) => items.as_slice(),
        _ => std::slice::from_ref(v),
    };
    // `x conj(x)` summed exactly and rounded once, like a dot product.
    let mut cells = Vec::new();
    for row in rows {
        let Value::Vector(row) = row else {
            return Err(NOT_A_MATRIX);
        };
        for item in flat(row)? {
            if item.is_mod() {
                return Err(value::NOT_FOR_MOD);
            }
            cells.push(item);
        }
    }
    let conjugates: Vec<Value> = cells.iter().map(|item| value::conj(item)).collect();
    matrix::sum_products(cells.iter().copied().zip(&conjugates), mode)
}

/// `A`. The length of a vector, as Calc's `abs`: `[3, 4]` is `5`. A matrix
/// gives the root of the sum of its squared elements, the Frobenius norm.
pub(crate) fn abs(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match v {
        // The sum keeps guard digits, so only the root rounds to the
        // precision. Rounding the sum first left `A` a unit off.
        // `value::sqrt` would round a float argument to the precision
        // first, as for a typed number, and drop those guard digits.
        Value::Vector(_) => {
            let mut wide = mode.clone();
            wide.prec = mode.prec.saturating_add(10);
            match abssqr(v, &wide)? {
                Value::Real(Real::Float(sum)) => Ok(value::round_floats(
                    Value::Real(Real::Float(sum.sqrt(mode.prec)?)),
                    mode.prec,
                )),
                exact => value::sqrt(&exact, mode),
            }
        }
        _ => value::abs(v, mode),
    }
}

/// `&`. A square matrix has an inverse. A flat vector has no reciprocal.
pub(crate) fn recip(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    match matrix::shape(v) {
        Shape::Number => value::recip(v, mode),
        Shape::Matrix(_, _) => matrix::inverse(v, mode),
        Shape::Flat(_) => Err(NOT_FOR_VECTOR),
        Shape::Ragged => Err(NOT_A_MATRIX),
    }
}
