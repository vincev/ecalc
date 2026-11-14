//! Matrices, as in Emacs Calc.
//!
//! A matrix is a vector of rows: every element is a vector of numbers, all
//! of one length, and there is at least one row and one column. `*` is the
//! matrix product, `&` the inverse, `^` an integer power, and `B / A`
//! solves `A X = B`, as Calc's `/` does.
//!
//! A decimal float is an exact rational, and the arithmetic here uses that.
//! An element of a product is a sum of products: with a float in it the sum
//! is taken exactly and rounded once ([`sum_products`]). The determinant,
//! the inverse, and `/` scale every entry to an integer and eliminate
//! without fractions (Bareiss), so no pivot is ever round-off, a matrix is
//! singular only when its determinant is exactly zero, and each result is
//! rounded once however ill-conditioned the matrix is. Modulo forms are
//! lifted to integers and reduced at the end, so a composite modulus works
//! whenever the determinant is a unit.
//!
//! Elimination and powers are cubic, so they draw on a [`Budget`] and stop
//! with `overflow` instead of running for minutes. An exact result is also
//! held to about a megabyte of digits, like an exact power of a number.

use crate::error::CalcError;
use crate::float::{DecFloat, MAX_INTEGER_EXPONENT, digit_count, pow10};
use crate::value::{self, Mode, Real, Value};
use crate::vector;
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

pub(crate) const NOT_A_MATRIX: CalcError = CalcError::Domain("not a matrix");
pub(crate) const DIMENSIONS: CalcError = CalcError::Domain("dimensions do not match");
const NOT_SQUARE: CalcError = CalcError::Domain("needs a square matrix");
const SINGULAR: CalcError = CalcError::Domain("singular matrix");
const SAME_MODULUS: CalcError = CalcError::Domain("modulo forms need the same modulus");
const THREE_VECTORS: CalcError = CalcError::Domain("cross needs two 3-vectors");
const IDENTITY_SIZE: CalcError = CalcError::Domain("identity needs a nonnegative integer");

/// Largest square matrix for `v i`, the determinant, the inverse, `/`, and
/// a power: 10000 elements, the same as `v x`.
pub(crate) const MAX_DIM: usize = 100;
/// Most digits between the exponents of the terms of a sum that is taken
/// exactly.
const MAX_SPREAD: u32 = 4096;
/// Most elements a product may have.
const MAX_CELLS: usize = vector::MAX_INDEX;
/// Most digits an exact result may hold in all, in bytes. An exact power
/// of a number stops at the same size.
const MAX_EXACT_BYTES: usize = 1 << 20;
/// Time one operation may take, in the units of [`Budget`]: a few
/// seconds. A 100 by 100 inverse of 12-digit numbers fits.
const WORK_LIMIT: f64 = 5e9;
/// One term of a matrix product, whatever its size.
const TERM_COST: f64 = 200.0;
/// One multiplication, per word to the power 1.585.
const MUL_COST: f64 = 3.0;
/// One term of an exact float sum, per word of the gap between the
/// exponents it lines up: a term `10^4000` times smaller than another is
/// shifted 4000 digits.
const SPREAD_COST: f64 = 1.0;
/// Reducing one element of a product of fraction matrices, per word of its
/// running denominator squared: the gcd at the end of [`exact_total`].
const GCD_COST: f64 = 25.0;
/// One elimination step, whatever its size.
const STEP_COST: f64 = 300.0;
/// One elimination step, per word squared.
const DIV_COST: f64 = 0.4;

type Rows = Vec<Vec<Value>>;

/// What a value is to the matrix operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Number,
    /// A vector with no vector in it, of this length.
    Flat(usize),
    /// Rows and columns, both at least 1.
    Matrix(usize, usize),
    /// A vector of vectors that is not a matrix.
    Ragged,
}

pub(crate) fn shape(v: &Value) -> Shape {
    let Value::Vector(items) = v else {
        return Shape::Number;
    };
    if !items.iter().any(|item| matches!(item, Value::Vector(_))) {
        return Shape::Flat(items.len());
    }
    let mut cols = 0;
    for item in items {
        let Value::Vector(row) = item else {
            return Shape::Ragged;
        };
        if row.is_empty() || row.iter().any(|cell| matches!(cell, Value::Vector(_))) {
            return Shape::Ragged;
        }
        if cols == 0 {
            cols = row.len();
        } else if cols != row.len() {
            return Shape::Ragged;
        }
    }
    Shape::Matrix(items.len(), cols)
}

/// The elements of a row, or the value itself when it is not a vector.
fn cells(row: &Value) -> &[Value] {
    match row {
        Value::Vector(cells) => cells,
        other => std::slice::from_ref(other),
    }
}

fn rows(v: &Value) -> Result<Rows, CalcError> {
    match (shape(v), v) {
        (Shape::Matrix(_, _), Value::Vector(items)) => {
            Ok(items.iter().map(|row| cells(row).to_vec()).collect())
        }
        _ => Err(NOT_A_MATRIX),
    }
}

fn square_rows(v: &Value) -> Result<Rows, CalcError> {
    match shape(v) {
        Shape::Matrix(r, c) if r == c => {
            if r > MAX_DIM {
                return Err(CalcError::Overflow);
            }
            rows(v)
        }
        _ => Err(NOT_SQUARE),
    }
}

fn from_rows(rows: Rows) -> Value {
    Value::Vector(rows.into_iter().map(Value::Vector).collect())
}

fn transposed(rows: &Rows) -> Rows {
    let cols = rows.first().map_or(0, Vec::len);
    (0..cols)
        .map(|j| rows.iter().map(|row| row[j].clone()).collect())
        .collect()
}

fn identity_rows(n: usize, one: &Value, zero: &Value) -> Rows {
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if i == j { one.clone() } else { zero.clone() })
                .collect()
        })
        .collect()
}

/// A running estimate of the time one operation takes, so a cubic
/// operation stops with `overflow` instead of running for minutes. The
/// unit is about a nanosecond on a fast machine.
struct Budget(f64);

impl Budget {
    fn new() -> Self {
        Self(0.0)
    }

    fn spend(&mut self, cost: f64) -> Result<(), CalcError> {
        self.0 += cost;
        if self.0 > WORK_LIMIT {
            Err(CalcError::Overflow)
        } else {
            Ok(())
        }
    }

    /// A matrix product of `terms` multiplications of numbers `bits` long.
    fn product(&mut self, terms: usize, bits: u64) -> Result<(), CalcError> {
        let words = bits as f64 / 64.0 + 1.0;
        self.spend(terms as f64 * (TERM_COST + MUL_COST * words.powf(1.585)))
    }

    /// Lining up `terms` float terms whose exponents lie `digits` apart.
    fn spread(&mut self, terms: usize, digits: u32) -> Result<(), CalcError> {
        let words = f64::from(digits) * 3.33 / 64.0;
        self.spend(terms as f64 * SPREAD_COST * words)
    }

    /// Reducing `cells` sums of `terms` fractions of up to `bits` bits each.
    /// The running denominator of a sum is the product of the term
    /// denominators, `terms` times as long as one of them.
    fn fractions(&mut self, cells: usize, terms: usize, bits: u64) -> Result<(), CalcError> {
        let words = terms as f64 * bits as f64 / 64.0 + 1.0;
        self.spend(cells as f64 * GCD_COST * words * words)
    }

    /// The elimination of `n` rows of `width` integers of `bits` bits.
    /// A minor of order `k` has `k` times the bits of an entry and
    /// `log2(k!)` more, and dividing one by another is quadratic.
    fn eliminate(
        &mut self,
        n: usize,
        width: usize,
        bits: u64,
        complex: bool,
    ) -> Result<(), CalcError> {
        let (n, width) = (n as f64, width as f64);
        let rhs = width - n;
        let steps = n * n * n / 3.0 + n * n * rhs;
        let words = n * (bits as f64 + 8.0) / 64.0 + 1.0;
        let each = STEP_COST + DIV_COST * words * words;
        self.spend(steps * each * if complex { 4.0 } else { 1.0 })
    }
}

/// The bits of the longest number.
fn max_bits(rows: &Rows) -> u64 {
    rows.iter()
        .flatten()
        .map(|v| (v.approx_bytes() as u64).saturating_mul(8))
        .max()
        .unwrap_or(0)
}

fn total_bytes(rows: &Rows) -> usize {
    rows.iter()
        .flatten()
        .fold(0usize, |sum, v| sum.saturating_add(v.approx_bytes()))
}

fn wide(mode: &Mode) -> Mode {
    let mut wide = mode.clone();
    wide.prec = mode.prec.saturating_mul(2).saturating_add(4);
    wide
}

/// The sum of `x * y` over the pairs, with `wide` from [`wide`]. An empty
/// sum is `0`.
fn sum_wide<'a>(
    pairs: impl Iterator<Item = (&'a Value, &'a Value)>,
    wide: &Mode,
    prec: u32,
) -> Result<Value, CalcError> {
    let mut sum: Option<Value> = None;
    for (x, y) in pairs {
        let term = value::mul(x, y, wide)?;
        sum = Some(match sum {
            None => term,
            Some(acc) => value::add(&acc, &term, wide)?,
        });
    }
    Ok(match sum {
        Some(sum) => value::round_floats(sum, prec),
        None => Value::from_int(0),
    })
}

/// The sum of `x * y` over the pairs, rounded once. The values must not
/// be vectors.
///
/// With a float among them the sum is taken exactly, as decimals, so the
/// digits that cancel are not lost: `1e10 * 1e10 + 1.5 - 1e10 * 1e10` is
/// `1.5`. Exact numbers add up exactly anyway. What is left (modulo forms,
/// and exponents too far apart to line up) keeps each product in full and
/// rounds the running sum at twice the precision, like a complex product.
pub(crate) fn sum_products<'a>(
    pairs: impl Iterator<Item = (&'a Value, &'a Value)> + Clone,
    mode: &Mode,
) -> Result<Value, CalcError> {
    if let Some(sum) = decimal_sum(pairs.clone(), mode.prec)? {
        return Ok(sum);
    }
    sum_wide(pairs, &wide(mode), mode.prec)
}

/// The parts of a number that is not a modulo form.
fn plain(v: &Value) -> Option<(&Real, Option<&Real>)> {
    match v {
        Value::Real(r) => Some((r, None)),
        Value::Complex(re, im) => Some((re, Some(im))),
        Value::Mod(_, _) | Value::Vector(_) => None,
    }
}

/// [`sum_products`] for numbers that are not modulo forms: exact when they
/// are all exact, and taken exactly then rounded once when a float takes
/// part. `None` leaves the sum to the caller: a modulo form, or exponents
/// too far apart.
fn decimal_sum<'a>(
    pairs: impl Iterator<Item = (&'a Value, &'a Value)>,
    prec: u32,
) -> Result<Option<Value>, CalcError> {
    let mut float = false;
    let (mut re, mut im) = (Vec::new(), Vec::new());
    for (x, y) in pairs {
        let (Some((xr, xi)), Some((yr, yi))) = (plain(x), plain(y)) else {
            return Ok(None);
        };
        float |= [Some(xr), xi, Some(yr), yi]
            .into_iter()
            .flatten()
            .any(is_float);
        let (xr, yr) = (Decimal::new(xr), Decimal::new(yr));
        let (xi, yi) = (xi.map(Decimal::new), yi.map(Decimal::new));
        re.push(xr.times(&yr));
        if let (Some(xi), Some(yi)) = (&xi, &yi) {
            re.push(xi.times(yi).negated());
        }
        if let Some(yi) = &yi {
            im.push(xr.times(yi));
        }
        if let Some(xi) = &xi {
            im.push(xi.times(&yr));
        }
    }
    if !float {
        // Integers and fractions: every exponent is 0.
        let exact = |terms: &[Decimal]| -> Result<Real, CalcError> {
            // Exponents are all 0, so nothing is too far apart.
            let (num, den) = exact_total(terms, 0)?.unwrap_or((BigInt::zero(), BigInt::one()));
            Ok(value::real_from_rational(BigRational::new(num, den)))
        };
        return Ok(Some(Value::complex(exact(&re)?, exact(&im)?)));
    }
    let (Some(re), Some(im)) = (decimal_total(&re, prec)?, decimal_total(&im, prec)?) else {
        return Ok(None);
    };
    Ok(Some(Value::complex(Real::Float(re), Real::Float(im))))
}

/// The exact sum of the terms as a float. `None` when lining up the
/// exponents would take more than `MAX_SPREAD` digits.
fn decimal_total(terms: &[Decimal], prec: u32) -> Result<Option<DecFloat>, CalcError> {
    let low = least_exponent(terms.iter());
    let Some((num, den)) = exact_total(terms, low)? else {
        return Ok(None);
    };
    ratio(&num, &den, low, prec).map(Some)
}

/// The exact sum of the terms as `num / den * 10^low`, not reduced.
/// `None` when lining up the exponents would take more than `MAX_SPREAD`
/// digits.
///
/// Fractions are added over a running product of their denominators and
/// reduced by the caller, once. Reducing after every addition ran a gcd
/// each time and made a product of fraction matrices take minutes.
fn exact_total(terms: &[Decimal], low: i64) -> Result<Option<(BigInt, BigInt)>, CalcError> {
    let mut num = BigInt::zero();
    let mut den = BigInt::one();
    for term in terms.iter().filter(|term| !term.n.is_zero()) {
        let Some(shift) = term
            .e
            .checked_sub(low)
            .and_then(|shift| u32::try_from(shift).ok())
            .filter(|&shift| shift <= MAX_SPREAD)
        else {
            return Ok(None);
        };
        let scaled = if shift == 0 {
            term.n.clone()
        } else {
            &term.n * pow10(shift)
        };
        if term.d == den {
            num += scaled;
        } else if den.is_one() {
            num = num * &term.d + scaled;
            den = term.d.clone();
        } else if term.d.is_one() {
            num += scaled * &den;
        } else {
            num = num * &term.d + scaled * &den;
            den *= &term.d;
        }
    }
    Ok(Some((num, den)))
}

fn product(a: &Rows, b: &Rows, mode: &Mode, budget: &mut Budget) -> Result<Rows, CalcError> {
    let (r, k) = (a.len(), b.len());
    let c = b.first().map_or(0, Vec::len);
    if r.saturating_mul(c) > MAX_CELLS {
        return Err(CalcError::Overflow);
    }
    let bits = max_bits(a).max(max_bits(b));
    budget.product(r.saturating_mul(k).saturating_mul(c), bits)?;
    if has_fraction(a) || has_fraction(b) {
        budget.fractions(r.saturating_mul(c), k, bits)?;
    }
    if let (Some((a_low, a_high)), Some((b_low, b_high))) = (orders(a), orders(b)) {
        let gap = (a_high + b_high).saturating_sub(a_low + b_low);
        let digits = u32::try_from(gap).unwrap_or(u32::MAX).min(MAX_SPREAD);
        budget.spread(r.saturating_mul(k).saturating_mul(c), digits)?;
    }
    // The result is held to `MAX_EXACT_BYTES` as it is built, so a product
    // of long fractions stops at the cap instead of finishing first.
    let mut bytes = 0usize;
    let mut out = Rows::with_capacity(r);
    for row in a {
        let mut cells = Vec::with_capacity(c);
        for j in 0..c {
            let column = b.iter().map(|other| &other[j]);
            let cell = sum_products(row.iter().zip(column), mode)?;
            bytes = bytes.saturating_add(cell.approx_bytes());
            if bytes > MAX_EXACT_BYTES {
                return Err(CalcError::Overflow);
            }
            cells.push(cell);
        }
        out.push(cells);
    }
    Ok(out)
}

/// The least and greatest order of the nonzero floats, or `None` when
/// there are none.
fn orders(rows: &Rows) -> Option<(i64, i64)> {
    let mut range: Option<(i64, i64)> = None;
    let mut see = |r: &Real| {
        if let Real::Float(f) = r
            && !f.is_zero()
        {
            let order = f.order();
            range = Some(match range {
                None => (order, order),
                Some((low, high)) => (low.min(order), high.max(order)),
            });
        }
    };
    for v in rows.iter().flatten() {
        match v {
            Value::Real(r) => see(r),
            Value::Complex(re, im) => {
                see(re);
                see(im);
            }
            _ => {}
        }
    }
    range
}

fn has_fraction(rows: &Rows) -> bool {
    rows.iter().flatten().any(|v| match v {
        Value::Real(r) => matches!(r, Real::Rat(_)),
        Value::Complex(re, im) => matches!(re, Real::Rat(_)) || matches!(im, Real::Rat(_)),
        _ => false,
    })
}

/// `*` of two vectors, as Calc's `math-mul-objects-fancy`. Two flat
/// vectors give the dot product. A matrix times a matrix needs the inner
/// dimensions to agree. A matrix times a flat vector takes the vector as a
/// column and gives a flat vector, and a flat vector times a matrix takes
/// it as a row. A one-column matrix times a flat vector of as many
/// elements is their outer product, which is Calc's rule too.
pub(crate) fn mul(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let mut budget = Budget::new();
    match (shape(a), shape(b)) {
        (Shape::Flat(_), Shape::Flat(_)) => vector::dot(cells(a), cells(b), mode),
        (Shape::Matrix(_, inner), Shape::Matrix(other, _)) => {
            if inner != other {
                return Err(DIMENSIONS);
            }
            product(&rows(a)?, &rows(b)?, mode, &mut budget).map(from_rows)
        }
        (Shape::Matrix(r, 1), Shape::Flat(len)) => {
            if r != len {
                return Err(DIMENSIONS);
            }
            let row = vec![cells(b).to_vec()];
            product(&rows(a)?, &row, mode, &mut budget).map(from_rows)
        }
        (Shape::Matrix(_, c), Shape::Flat(len)) => {
            if c != len {
                return Err(DIMENSIONS);
            }
            let column: Rows = cells(b).iter().map(|x| vec![x.clone()]).collect();
            let out = product(&rows(a)?, &column, mode, &mut budget)?;
            Ok(Value::Vector(out.into_iter().flatten().collect()))
        }
        (Shape::Flat(len), Shape::Matrix(r, _)) => {
            if len != r {
                return Err(DIMENSIONS);
            }
            let row = vec![cells(a).to_vec()];
            let out = product(&row, &rows(b)?, mode, &mut budget)?;
            Ok(Value::Vector(out.into_iter().flatten().collect()))
        }
        _ => Err(NOT_A_MATRIX),
    }
}

/// What the entries of a system are.
enum Field {
    /// Integers, fractions, floats, and complex numbers. `float` is set
    /// when any part is a float, and then every result is one.
    Plain { float: bool },
    /// Integers modulo this.
    Modulo(BigInt),
}

fn is_float(r: &Real) -> bool {
    matches!(r, Real::Float(_))
}

fn field<'a>(cells: impl Iterator<Item = &'a Value>) -> Result<Field, CalcError> {
    let mut modulus: Option<&BigInt> = None;
    let mut float = false;
    for cell in cells {
        match cell {
            Value::Mod(_, m) => match modulus {
                Some(seen) if seen != m => return Err(SAME_MODULUS),
                _ => modulus = Some(m),
            },
            Value::Real(r) => float |= is_float(r),
            Value::Complex(re, im) => float |= is_float(re) || is_float(im),
            Value::Vector(_) => return Err(NOT_A_MATRIX),
        }
    }
    Ok(match modulus {
        Some(m) => Field::Modulo(m.clone()),
        None => Field::Plain { float },
    })
}

/// A real part as an exact decimal: `n / d * 10^e`, with `d > 0`.
struct Decimal {
    n: BigInt,
    d: BigInt,
    e: i64,
}

impl Decimal {
    fn times(&self, other: &Self) -> Self {
        Self {
            n: &self.n * &other.n,
            d: &self.d * &other.d,
            e: self.e.saturating_add(other.e),
        }
    }

    fn negated(mut self) -> Self {
        self.n = -self.n;
        self
    }

    fn new(r: &Real) -> Self {
        match r {
            Real::Int(n) => Self {
                n: n.clone(),
                d: BigInt::one(),
                e: 0,
            },
            Real::Rat(q) => Self {
                n: q.numer().clone(),
                d: q.denom().clone(),
                e: 0,
            },
            Real::Float(f) => {
                let (n, e) = f.signed_decimal();
                Self {
                    n,
                    d: BigInt::one(),
                    e,
                }
            }
        }
    }
}

/// The real and imaginary parts of an entry.
fn decimals(v: &Value) -> Result<Vec<Decimal>, CalcError> {
    match v {
        Value::Real(r) => Ok(vec![Decimal::new(r)]),
        Value::Complex(re, im) => Ok(vec![Decimal::new(re), Decimal::new(im)]),
        Value::Mod(_, _) | Value::Vector(_) => Err(NOT_A_MATRIX),
    }
}

/// An integer, or a Gaussian integer when the second part is not zero.
type Parts = (BigInt, BigInt);

/// `[A | B]` as integers, and how it relates to the system asked for.
struct System {
    rows: Vec<Vec<Parts>>,
    /// Some entry has an imaginary part.
    complex: bool,
    /// `det(A) = det(rows) * 10^det_shift / det_div`.
    det_shift: i64,
    det_div: BigInt,
    /// Element `(j, k)` of the solution is the integer system's times
    /// `10^(rhs_shift[k] - col_shift[j])`.
    col_shift: Vec<i64>,
    rhs_shift: Vec<i64>,
}

/// The least exponent among the nonzero parts, or 0 when there are none.
fn least_exponent<'a>(parts: impl Iterator<Item = &'a Decimal>) -> i64 {
    parts
        .filter(|part| !part.n.is_zero())
        .map(|part| part.e)
        .min()
        .unwrap_or(0)
}

/// `[A | B]` with every entry an integer or a Gaussian integer.
///
/// Each row of `[A | B]` is multiplied by the least common multiple of its
/// denominators, which leaves the solution as it was. Powers of ten are
/// taken out of each row of `A`, then out of each column of `A` and of
/// `B`, so a matrix of `1e-200000` costs no more than a matrix of ones.
/// What is left of an exponent becomes digits, and past
/// `MAX_INTEGER_EXPONENT` that is an overflow.
fn integer_system(a: &Rows, b: &Rows, budget: &mut Budget) -> Result<System, CalcError> {
    let n = a.len();
    let mut system: Vec<Vec<Vec<Decimal>>> = Vec::with_capacity(n);
    for (i, row) in a.iter().enumerate() {
        let rhs = b.get(i).map_or(&[][..], Vec::as_slice);
        system.push(
            row.iter()
                .chain(rhs)
                .map(decimals)
                .collect::<Result<_, _>>()?,
        );
    }
    let width = system.first().map_or(n, Vec::len);
    let mut det_shift = 0i64;
    for row in &mut system {
        let shift = least_exponent(row[..n].iter().flatten());
        det_shift = det_shift.checked_add(shift).ok_or(CalcError::Overflow)?;
        for part in row.iter_mut().flatten() {
            part.e = part.e.checked_sub(shift).ok_or(CalcError::Overflow)?;
        }
    }
    let mut shifts = Vec::with_capacity(width);
    for j in 0..width {
        let shift = least_exponent(system.iter().flat_map(|row| row[j].iter()));
        if j < n {
            det_shift = det_shift.checked_add(shift).ok_or(CalcError::Overflow)?;
        }
        for row in &mut system {
            for part in &mut row[j] {
                part.e = part.e.checked_sub(shift).ok_or(CalcError::Overflow)?;
            }
        }
        shifts.push(shift);
    }
    let rhs_shift = shifts.split_off(n);

    // How long the integers will be, before any of them is built: a float
    // of a high order among small ones is that many digits.
    let lcms: Vec<BigInt> = system
        .iter()
        .map(|row| {
            row.iter()
                .flatten()
                .fold(BigInt::one(), |lcm, part| lcm.lcm(&part.d))
        })
        .collect();
    let mut longest = 0u64;
    let mut complex = false;
    for (row, lcm) in system.iter().zip(&lcms) {
        for entry in row {
            complex |= entry.get(1).is_some_and(|im| !im.n.is_zero());
            for part in entry.iter().filter(|part| !part.n.is_zero()) {
                let power = u64::try_from(part.e)
                    .ok()
                    .filter(|&e| e <= u64::from(MAX_INTEGER_EXPONENT))
                    .ok_or(CalcError::Overflow)?;
                // `log2(10)` is under 3.33.
                let bits = part.n.bits() + lcm.bits() + power * 333 / 100 + 1;
                longest = longest.max(bits);
            }
        }
    }
    budget.eliminate(n, width, longest, complex)?;

    let ten = BigInt::from(10);
    let mut det_div = BigInt::one();
    let mut rows = Vec::with_capacity(n);
    for (row, lcm) in system.iter().zip(lcms) {
        let mut whole = Vec::with_capacity(row.len());
        for entry in row {
            let mut parts = entry.iter().map(|part| {
                if part.n.is_zero() {
                    return BigInt::zero();
                }
                let scaled = &part.n * (&lcm / &part.d);
                match u32::try_from(part.e) {
                    Ok(power) if power > 0 => scaled * ten.pow(power),
                    _ => scaled,
                }
            });
            let re = parts.next().unwrap_or_default();
            let im = parts.next().unwrap_or_default();
            whole.push((re, im));
        }
        det_div *= lcm;
        rows.push(whole);
    }
    Ok(System {
        rows,
        complex,
        det_shift,
        det_div,
        col_shift: shifts,
        rhs_shift,
    })
}

/// A system of modulo forms and integers, lifted to the residues modulo
/// `m`.
fn lifted_system(a: &Rows, b: &Rows, m: &BigInt) -> Result<System, CalcError> {
    let lift = |v: &Value| match v {
        Value::Mod(residue, _) => Ok((residue.clone(), BigInt::zero())),
        other => value::mod_integer(other).map(|n| (n.mod_floor(m), BigInt::zero())),
    };
    let mut rows = Vec::with_capacity(a.len());
    for (i, row) in a.iter().enumerate() {
        let rhs = b.get(i).map_or(&[][..], Vec::as_slice);
        rows.push(row.iter().chain(rhs).map(lift).collect::<Result<_, _>>()?);
    }
    Ok(System {
        rows,
        complex: false,
        det_shift: 0,
        det_div: BigInt::one(),
        col_shift: Vec::new(),
        rhs_shift: Vec::new(),
    })
}

/// The integers an elimination runs on: plain ones, or Gaussian integers
/// when an entry is complex.
trait Ring: Clone {
    fn from_parts(parts: Parts) -> Self;
    fn into_parts(self) -> Parts;
    fn vanishes(&self) -> bool;
    fn times(&self, other: &Self) -> Self;
    fn minus(&self, other: &Self) -> Self;
    /// `self / other`, where the caller knows the division is exact.
    fn div_exact(&self, other: &Self) -> Self;
    /// `(c, d)` with `1 / self = c / d` and `d` a plain integer.
    fn reciprocal(&self) -> (Self, BigInt);
}

impl Ring for BigInt {
    fn from_parts(parts: Parts) -> Self {
        parts.0
    }

    fn into_parts(self) -> Parts {
        (self, BigInt::zero())
    }

    fn vanishes(&self) -> bool {
        self.is_zero()
    }

    fn times(&self, other: &Self) -> Self {
        self * other
    }

    fn minus(&self, other: &Self) -> Self {
        self - other
    }

    fn div_exact(&self, other: &Self) -> Self {
        self / other
    }

    fn reciprocal(&self) -> (Self, BigInt) {
        (BigInt::one(), self.clone())
    }
}

#[derive(Clone)]
struct Gauss {
    re: BigInt,
    im: BigInt,
}

impl Ring for Gauss {
    fn from_parts((re, im): Parts) -> Self {
        Self { re, im }
    }

    fn into_parts(self) -> Parts {
        (self.re, self.im)
    }

    fn vanishes(&self) -> bool {
        self.re.is_zero() && self.im.is_zero()
    }

    fn times(&self, other: &Self) -> Self {
        Self {
            re: &self.re * &other.re - &self.im * &other.im,
            im: &self.re * &other.im + &self.im * &other.re,
        }
    }

    fn minus(&self, other: &Self) -> Self {
        Self {
            re: &self.re - &other.re,
            im: &self.im - &other.im,
        }
    }

    fn div_exact(&self, other: &Self) -> Self {
        let (conj, norm) = other.reciprocal();
        let top = self.times(&conj);
        Self {
            re: top.re / &norm,
            im: top.im / norm,
        }
    }

    fn reciprocal(&self) -> (Self, BigInt) {
        let norm = &self.re * &self.re + &self.im * &self.im;
        let conj = Self {
            re: self.re.clone(),
            im: -&self.im,
        };
        (conj, norm)
    }
}

/// What an elimination found, as integers.
struct Reduced {
    /// The determinant of the integer matrix.
    det: Parts,
    /// The solution is `numerators / denominator`.
    numerators: Vec<Vec<Parts>>,
    denominator: BigInt,
}

/// Fraction-free Gaussian elimination (Bareiss) of the integer system
/// `[A | B]`, with `A` square of order `n`, then back substitution. Every
/// number the elimination makes is a minor of the system, so nothing
/// grows past the determinant and every division is exact. `None` means
/// the determinant is zero.
fn reduce<T: Ring>(rows: Vec<Vec<Parts>>, n: usize) -> Option<Reduced> {
    let mut w: Vec<Vec<T>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(T::from_parts).collect())
        .collect();
    let width = w.first().map_or(n, Vec::len);
    let mut prev: Option<T> = None;
    let mut negative = false;
    for k in 0..n {
        let found = (k..n).find(|&i| !w[i][k].vanishes())?;
        if found != k {
            w.swap(found, k);
            negative = !negative;
        }
        let (head, tail) = w.split_at_mut(k + 1);
        let pivot_row = &head[k];
        let pivot = &pivot_row[k];
        for row in tail {
            for j in k + 1..width {
                let top = pivot.times(&row[j]).minus(&row[k].times(&pivot_row[j]));
                row[j] = match &prev {
                    Some(prev) => top.div_exact(prev),
                    None => top,
                };
            }
        }
        prev = Some(pivot.clone());
    }
    // The last pivot is the determinant of the rows as they now stand.
    let d = prev?;
    // `y = d x` is whole, by Cramer's rule, and row `k` of the triangle
    // gives `w[k][k] y[k] = d b[k] - sum of w[k][j] y[j]` past the diagonal.
    let (conj, denominator) = d.reciprocal();
    let mut numerators = vec![Vec::with_capacity(width - n); n];
    for column in n..width {
        let mut y: Vec<T> = Vec::with_capacity(n);
        for k in (0..n).rev() {
            let mut top = d.times(&w[k][column]);
            // `y` holds rows `n - 1` down to `k + 1`.
            for (j, solved) in (k + 1..n).rev().zip(&y) {
                top = top.minus(&w[k][j].times(solved));
            }
            y.push(top.div_exact(&w[k][k]));
        }
        for (row, solved) in numerators.iter_mut().zip(y.iter().rev()) {
            row.push(solved.times(&conj).into_parts());
        }
    }
    let (re, im) = d.into_parts();
    let det = if negative { (-re, -im) } else { (re, im) };
    Some(Reduced {
        det,
        numerators,
        denominator,
    })
}

/// `n / d * 10^shift`, rounded once.
///
/// The integer quotient keeps at least `prec + 1` digits, and one more
/// digit says whether anything was left over, so the rounding to `prec`
/// digits sees a tie only when there is one. Rounding `n` and `d` to
/// floats first, then the quotient, rounded up to three times:
/// `3.0000000000149999999999` came out as `3.00000000002`.
fn ratio(n: &BigInt, d: &BigInt, shift: i64, prec: u32) -> Result<DecFloat, CalcError> {
    if n.is_zero() {
        return Ok(DecFloat::zero(prec));
    }
    let scale = i64::from(prec) + 2 + i64::from(digit_count(d)) - i64::from(digit_count(n));
    let power = |k: i64| u32::try_from(k).map(pow10).map_err(|_| CalcError::Overflow);
    let (num, den) = if scale >= 0 {
        (n * power(scale)?, d.clone())
    } else {
        (n.clone(), d * power(-scale)?)
    };
    let (q, r) = num.div_rem(&den);
    let sticky = if r.is_zero() {
        0
    } else if n.is_negative() == d.is_negative() {
        1
    } else {
        -1
    };
    let mantissa = q * 10 + sticky;
    let exponent = shift
        .checked_sub(scale)
        .and_then(|e| e.checked_sub(1))
        .ok_or(CalcError::Overflow)?;
    Ok(DecFloat::from_decimal(mantissa, exponent, prec))
}

/// How a quotient of integers is written.
#[derive(Clone, Copy)]
enum Form {
    /// An integer, or a fraction in lowest terms.
    Exact,
    /// As `/` shows it with fraction mode off: an integer when the
    /// division is exact, and otherwise a float of `prec` digits.
    Settled { prec: u32 },
    /// A float of `prec` digits, times `10^shift`.
    Float { shift: i64, prec: u32 },
}

fn quotient_real(n: &BigInt, d: &BigInt, form: Form) -> Result<Real, CalcError> {
    if let Form::Float { shift, prec } = form {
        return Ok(Real::Float(ratio(n, d, shift, prec)?));
    }
    let (q, r) = n.div_rem(d);
    if r.is_zero() {
        return Ok(Real::Int(q));
    }
    match form {
        Form::Settled { prec } => Ok(Real::Float(ratio(n, d, 0, prec)?)),
        _ => Ok(Real::Rat(BigRational::new(n.clone(), d.clone()))),
    }
}

/// `(re, im) / d`.
fn quotient((re, im): &Parts, d: &BigInt, form: Form) -> Result<Value, CalcError> {
    let re = quotient_real(re, d, form)?;
    if im.is_zero() {
        return Ok(Value::Real(re));
    }
    Ok(Value::complex(re, quotient_real(im, d, form)?))
}

/// An exact value as `/` shows it: an integer stays one, and a fraction
/// stays one only in fraction mode. Otherwise it is a float, rounded once.
fn settle(v: Value, mode: &Mode) -> Result<Value, CalcError> {
    if mode.fraction {
        return Ok(v);
    }
    let part = |r: Real| match r {
        Real::Rat(q) => ratio(q.numer(), q.denom(), 0, mode.prec).map(Real::Float),
        other => Ok(other),
    };
    match v {
        Value::Real(r) => Ok(Value::Real(part(r)?)),
        Value::Complex(re, im) => Ok(Value::complex(part(re)?, part(im)?)),
        other => Ok(other),
    }
}

/// What `solve` does with an exact solution that is not whole.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    /// Fractions, whatever the mode. A power goes on multiplying them.
    Fractions,
    /// What `/` shows: fractions in fraction mode, floats otherwise.
    Quotients,
}

/// The determinant of the square `a` and, when it is not zero, the `X`
/// with `a X = b`. `b` has as many rows as `a`, or none.
///
/// Exact entries give an exact determinant, and a solution as `keep`
/// says. With a float anywhere, every result is a float at `mode.prec`.
fn solve(
    a: &Rows,
    b: &Rows,
    mode: &Mode,
    keep: Keep,
    budget: &mut Budget,
) -> Result<(Value, Option<Rows>), CalcError> {
    let n = a.len();
    let field = field(a.iter().chain(b).flatten())?;
    let system = match &field {
        Field::Modulo(m) => {
            let width = n + b.first().map_or(0, Vec::len);
            budget.eliminate(n, width, m.bits(), false)?;
            lifted_system(a, b, m)?
        }
        Field::Plain { .. } => integer_system(a, b, budget)?,
    };

    let reduced = if system.complex {
        reduce::<Gauss>(system.rows, n)
    } else {
        reduce::<BigInt>(system.rows, n)
    };
    match field {
        Field::Modulo(m) => {
            let residue = |x: &BigInt| Value::make_mod(x.clone(), m.clone());
            let Some(reduced) = reduced else {
                return Ok((residue(&BigInt::zero())?, None));
            };
            let det = residue(&reduced.det.0)?;
            if det.is_zero() {
                return Ok((det, None));
            }
            if b.is_empty() {
                return Ok((det, Some(Rows::new())));
            }
            // A determinant that is not zero and not a unit has no inverse.
            let inverse = value::recip(&residue(&reduced.denominator)?, mode)?;
            let x = reduced
                .numerators
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|(x, _)| value::mul(&residue(x)?, &inverse, mode))
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Rows, _>>()?;
            Ok((det, Some(x)))
        }
        Field::Plain { float } => {
            let prec = mode.prec;
            let form = |shift: i64, exact: Form| {
                if float {
                    Form::Float { shift, prec }
                } else {
                    exact
                }
            };
            let kept = if keep == Keep::Fractions || mode.fraction {
                Form::Exact
            } else {
                Form::Settled { prec }
            };
            let Some(reduced) = reduced else {
                let zero = (BigInt::zero(), BigInt::zero());
                let zero = quotient(&zero, &BigInt::one(), form(0, Form::Exact))?;
                return Ok((zero, None));
            };
            let det = quotient(
                &reduced.det,
                &system.det_div,
                form(system.det_shift, Form::Exact),
            )?;
            let x = reduced
                .numerators
                .iter()
                .enumerate()
                .map(|(j, row)| {
                    row.iter()
                        .enumerate()
                        .map(|(k, x)| {
                            let shift = if float {
                                system.rhs_shift[k].saturating_sub(system.col_shift[j])
                            } else {
                                0
                            };
                            quotient(x, &reduced.denominator, form(shift, kept))
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Rows, _>>()?;
            Ok((det, Some(checked(x)?)))
        }
    }
}

/// `V D`. The determinant of a square matrix.
pub(crate) fn det(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let a = square_rows(v)?;
    let mut budget = Budget::new();
    solve(&a, &Rows::new(), mode, Keep::Fractions, &mut budget).map(|(det, _)| det)
}

fn inverse_rows(a: &Rows, mode: &Mode, keep: Keep, budget: &mut Budget) -> Result<Rows, CalcError> {
    let identity = identity_rows(a.len(), &Value::from_int(1), &Value::from_int(0));
    solve(a, &identity, mode, keep, budget)?.1.ok_or(SINGULAR)
}

/// `&`. The inverse of a square matrix.
pub(crate) fn inverse(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let a = square_rows(v)?;
    inverse_rows(&a, mode, Keep::Quotients, &mut Budget::new()).map(from_rows)
}

/// `/` by a matrix, as Calc's `math-div-objects-fancy`: `B / A` is the `X`
/// with `A X = B`, which is `A^-1 B`. `B` is a matrix of as many rows as
/// `A`, or a flat vector of that length, taken as a column. A matrix `B`
/// whose rows do not fit but whose columns do gives `B A^-1`. A number
/// gives that number times the inverse.
pub(crate) fn divide(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let m = square_rows(b)?;
    let n = m.len();
    let mut budget = Budget::new();
    let mut system = |m: &Rows, rhs: &Rows| -> Result<Rows, CalcError> {
        solve(m, rhs, mode, Keep::Quotients, &mut budget)?
            .1
            .ok_or(SINGULAR)
    };
    match shape(a) {
        Shape::Number => {
            let rhs = identity_rows(n, a, &Value::from_int(0));
            system(&m, &rhs).map(from_rows)
        }
        Shape::Flat(len) if len == n => {
            let rhs: Rows = cells(a).iter().map(|x| vec![x.clone()]).collect();
            let x = system(&m, &rhs)?;
            Ok(Value::Vector(x.into_iter().flatten().collect()))
        }
        Shape::Matrix(r, _) if r == n => system(&m, &rows(a)?).map(from_rows),
        Shape::Matrix(_, c) if c == n => {
            let x = system(&transposed(&m), &transposed(&rows(a)?))?;
            Ok(from_rows(transposed(&x)))
        }
        Shape::Flat(_) | Shape::Matrix(_, _) => Err(DIMENSIONS),
        Shape::Ragged => Err(NOT_A_MATRIX),
    }
}

/// `^` of a square matrix by an integer, by repeated squaring. `0` gives
/// the identity and a negative power is a power of the inverse, as in
/// Calc.
///
/// Exact entries stay exact through the products, and a negative power is
/// settled at the end, like `/`.
///
/// Floats are rounded after every product, and a later product can cancel
/// the digits that were kept: `M^-2` squares an inverse whose entries may
/// be far larger than the answer. So the power is run with guard digits,
/// one per exponent bit and a few more, and run again with more until two
/// runs agree to the display precision, as `cplx::refine` does.
pub(crate) fn power(v: &Value, exp: &BigInt, mode: &Mode) -> Result<Value, CalcError> {
    let a = square_rows(v)?;
    let n = a.len();
    let field = field(a.iter().flatten())?;
    if exp.is_zero() {
        let unit = |k: u8| match &field {
            Field::Modulo(m) => Value::make_mod(BigInt::from(k), m.clone()),
            Field::Plain { .. } => Ok(Value::from_int(k)),
        };
        return Ok(from_rows(identity_rows(n, &unit(1)?, &unit(0)?)));
    }
    if exp.is_negative() && exp.magnitude().is_one() {
        return inverse(v, mode);
    }
    let mut budget = Budget::new();
    if !matches!(field, Field::Plain { float: true }) {
        let exact = power_at(&a, exp, mode, &mut budget)?;
        if !exp.is_negative() {
            return Ok(from_rows(exact));
        }
        return exact
            .into_iter()
            .map(|row| row.into_iter().map(|v| settle(v, mode)).collect())
            .collect::<Result<Rows, _>>()
            .map(from_rows);
    }

    let prec = mode.prec;
    let guard = (exp.bits().min(64) as u32).saturating_add(6);
    let steps = [
        prec.saturating_add(4),
        prec.saturating_add(16),
        prec.saturating_mul(2).saturating_add(40),
        prec.saturating_mul(4).saturating_add(80),
    ];
    let mut run = |digits: u32| {
        let mut work = mode.clone();
        work.prec = digits.saturating_add(guard);
        power_at(&a, exp, &work, &mut budget)
    };
    let mut last = run(steps[0])?;
    for &digits in &steps[1..] {
        let next = run(digits)?;
        let settled = last
            .iter()
            .flatten()
            .zip(next.iter().flatten())
            .all(|(old, new)| agrees(old, new, prec));
        last = next;
        if settled {
            break;
        }
    }
    Ok(from_rows(
        last.into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|v| value::round_floats(v, prec))
                    .collect()
            })
            .collect(),
    ))
}

/// The power with every product rounded at `work.prec`. Exact entries stay
/// exact, fractions included.
fn power_at(a: &Rows, exp: &BigInt, work: &Mode, budget: &mut Budget) -> Result<Rows, CalcError> {
    let mut base = if exp.is_negative() {
        inverse_rows(a, work, Keep::Fractions, budget)?
    } else {
        a.clone()
    };
    let magnitude = exp.magnitude();
    let top = magnitude.bits();
    let mut result: Option<Rows> = None;
    for bit in 0..top {
        if magnitude.bit(bit) {
            result = Some(match result {
                None => base.clone(),
                Some(acc) => checked(product(&acc, &base, work, budget)?)?,
            });
        }
        if bit + 1 < top {
            base = checked(product(&base, &base, work, budget)?)?;
        }
    }
    Ok(result.unwrap_or(base))
}

/// Whether two runs of a float computation give the same `prec` digits of
/// an element, in each part.
fn agrees(old: &Value, new: &Value, prec: u32) -> bool {
    let part = |old: &Real, new: &Real| match (old, new) {
        (Real::Float(old), Real::Float(new)) => {
            if old.is_zero() || new.is_zero() {
                return old.is_zero() && new.is_zero();
            }
            let diff = old.sub(new, prec.saturating_add(8));
            diff.is_zero() || diff.order() < new.order() - i64::from(prec) - 2
        }
        // An exact part is the same in every run.
        (Real::Float(_), _) | (_, Real::Float(_)) => false,
        _ => true,
    };
    match (plain(old), plain(new)) {
        (Some((old_re, old_im)), Some((new_re, new_im))) => {
            part(old_re, new_re)
                && match (old_im, new_im) {
                    (Some(old_im), Some(new_im)) => part(old_im, new_im),
                    (None, None) => true,
                    _ => false,
                }
        }
        _ => true,
    }
}

/// An exact product that has grown past what an exact result may hold.
fn checked(rows: Rows) -> Result<Rows, CalcError> {
    if total_bytes(&rows) > MAX_EXACT_BYTES {
        Err(CalcError::Overflow)
    } else {
        Ok(rows)
    }
}

/// `v t`, as Calc's `trn`. A matrix is transposed, a flat vector becomes a
/// one-column matrix, and a number is itself.
pub(crate) fn transpose(v: &Value) -> Result<Value, CalcError> {
    match shape(v) {
        Shape::Number => Ok(v.clone()),
        Shape::Flat(_) => Ok(Value::Vector(
            cells(v)
                .iter()
                .map(|x| Value::Vector(vec![x.clone()]))
                .collect(),
        )),
        Shape::Matrix(_, _) => Ok(from_rows(transposed(&rows(v)?))),
        Shape::Ragged => Err(NOT_A_MATRIX),
    }
}

/// `V T`. The sum of the diagonal of a square matrix, rounded once.
pub(crate) fn trace(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let Shape::Matrix(r, c) = shape(v) else {
        return Err(NOT_SQUARE);
    };
    if r != c {
        return Err(NOT_SQUARE);
    }
    let one = Value::from_int(1);
    let diagonal = cells(v)
        .iter()
        .enumerate()
        .map(|(i, row)| (&cells(row)[i], &one));
    sum_products(diagonal, mode)
}

/// `v i`. The identity matrix of order `n`, as Calc's `idn(1, n)`.
pub(crate) fn identity(v: &Value) -> Result<Value, CalcError> {
    let n = value::exact_int(v)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(IDENTITY_SIZE)?;
    if n > MAX_DIM {
        return Err(CalcError::Overflow);
    }
    Ok(from_rows(identity_rows(
        n,
        &Value::from_int(1),
        &Value::from_int(0),
    )))
}

/// `V C`. The cross product of two vectors of three numbers. Each
/// component is a difference of two products, rounded once.
pub(crate) fn cross(a: &Value, b: &Value, mode: &Mode) -> Result<Value, CalcError> {
    let (Shape::Flat(3), Shape::Flat(3)) = (shape(a), shape(b)) else {
        return Err(THREE_VECTORS);
    };
    let (a, b) = (cells(a), cells(b));
    let component = |i: usize, j: usize| {
        let minus = value::neg(&a[j]);
        sum_products([(&a[i], &b[j]), (&minus, &b[i])].into_iter(), mode)
    };
    Ok(Value::Vector(vec![
        component(1, 2)?,
        component(2, 0)?,
        component(0, 1)?,
    ]))
}
