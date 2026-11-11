//! Gcd, lcm, factorial, choose, permutations, the prime test, modular inverse,
//! and prime factors.
//!
//! These stay on the number tower already in use. Extended gcd pushes three
//! integers. Prime factors push one integer per factor, smallest first.

use crate::error::CalcError;
use crate::float::DecFloat;
use crate::value::{Mode, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

const MAX_FACTORIAL: u32 = 100_000;
/// Largest `n` in a Stirling number. The table grows with `n * m`.
const MAX_STIRLING_N: u32 = 2_000;
/// Trial division proves the answer up through this bound, matching Calc.
const TRIAL_LIMIT: u64 = 8_000_000;
/// Thirteen witnesses prove primality below this integer.
const PROVEN_BOUND: &str = "3317044064679887385961981";
/// Largest cofactor Pollard rho will try to split. Beyond this, a cofactor
/// keeps Calc's behavior and stays in one piece.
const MAX_POLLARD_BITS: u64 = 1024;
/// Total modular squarings per cofactor before Pollard rho gives up.
const POLLARD_BUDGET: u64 = 1_000_000;
/// Brent's block size for batching gcd calls.
const POLLARD_BLOCK: u64 = 128;
/// Polynomial constants tried after one cycles without finding a factor.
const POLLARD_ATTEMPTS: u32 = 16;
/// Emacs Calc's `math-primes-table`: every prime through 5003.
const FACTOR_PRIMES: &[u32] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541, 547,
    557, 563, 569, 571, 577, 587, 593, 599, 601, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659,
    661, 673, 677, 683, 691, 701, 709, 719, 727, 733, 739, 743, 751, 757, 761, 769, 773, 787, 797,
    809, 811, 821, 823, 827, 829, 839, 853, 857, 859, 863, 877, 881, 883, 887, 907, 911, 919, 929,
    937, 941, 947, 953, 967, 971, 977, 983, 991, 997, 1009, 1013, 1019, 1021, 1031, 1033, 1039,
    1049, 1051, 1061, 1063, 1069, 1087, 1091, 1093, 1097, 1103, 1109, 1117, 1123, 1129, 1151, 1153,
    1163, 1171, 1181, 1187, 1193, 1201, 1213, 1217, 1223, 1229, 1231, 1237, 1249, 1259, 1277, 1279,
    1283, 1289, 1291, 1297, 1301, 1303, 1307, 1319, 1321, 1327, 1361, 1367, 1373, 1381, 1399, 1409,
    1423, 1427, 1429, 1433, 1439, 1447, 1451, 1453, 1459, 1471, 1481, 1483, 1487, 1489, 1493, 1499,
    1511, 1523, 1531, 1543, 1549, 1553, 1559, 1567, 1571, 1579, 1583, 1597, 1601, 1607, 1609, 1613,
    1619, 1621, 1627, 1637, 1657, 1663, 1667, 1669, 1693, 1697, 1699, 1709, 1721, 1723, 1733, 1741,
    1747, 1753, 1759, 1777, 1783, 1787, 1789, 1801, 1811, 1823, 1831, 1847, 1861, 1867, 1871, 1873,
    1877, 1879, 1889, 1901, 1907, 1913, 1931, 1933, 1949, 1951, 1973, 1979, 1987, 1993, 1997, 1999,
    2003, 2011, 2017, 2027, 2029, 2039, 2053, 2063, 2069, 2081, 2083, 2087, 2089, 2099, 2111, 2113,
    2129, 2131, 2137, 2141, 2143, 2153, 2161, 2179, 2203, 2207, 2213, 2221, 2237, 2239, 2243, 2251,
    2267, 2269, 2273, 2281, 2287, 2293, 2297, 2309, 2311, 2333, 2339, 2341, 2347, 2351, 2357, 2371,
    2377, 2381, 2383, 2389, 2393, 2399, 2411, 2417, 2423, 2437, 2441, 2447, 2459, 2467, 2473, 2477,
    2503, 2521, 2531, 2539, 2543, 2549, 2551, 2557, 2579, 2591, 2593, 2609, 2617, 2621, 2633, 2647,
    2657, 2659, 2663, 2671, 2677, 2683, 2687, 2689, 2693, 2699, 2707, 2711, 2713, 2719, 2729, 2731,
    2741, 2749, 2753, 2767, 2777, 2789, 2791, 2797, 2801, 2803, 2819, 2833, 2837, 2843, 2851, 2857,
    2861, 2879, 2887, 2897, 2903, 2909, 2917, 2927, 2939, 2953, 2957, 2963, 2969, 2971, 2999, 3001,
    3011, 3019, 3023, 3037, 3041, 3049, 3061, 3067, 3079, 3083, 3089, 3109, 3119, 3121, 3137, 3163,
    3167, 3169, 3181, 3187, 3191, 3203, 3209, 3217, 3221, 3229, 3251, 3253, 3257, 3259, 3271, 3299,
    3301, 3307, 3313, 3319, 3323, 3329, 3331, 3343, 3347, 3359, 3361, 3371, 3373, 3389, 3391, 3407,
    3413, 3433, 3449, 3457, 3461, 3463, 3467, 3469, 3491, 3499, 3511, 3517, 3527, 3529, 3533, 3539,
    3541, 3547, 3557, 3559, 3571, 3581, 3583, 3593, 3607, 3613, 3617, 3623, 3631, 3637, 3643, 3659,
    3671, 3673, 3677, 3691, 3697, 3701, 3709, 3719, 3727, 3733, 3739, 3761, 3767, 3769, 3779, 3793,
    3797, 3803, 3821, 3823, 3833, 3847, 3851, 3853, 3863, 3877, 3881, 3889, 3907, 3911, 3917, 3919,
    3923, 3929, 3931, 3943, 3947, 3967, 3989, 4001, 4003, 4007, 4013, 4019, 4021, 4027, 4049, 4051,
    4057, 4073, 4079, 4091, 4093, 4099, 4111, 4127, 4129, 4133, 4139, 4153, 4157, 4159, 4177, 4201,
    4211, 4217, 4219, 4229, 4231, 4241, 4243, 4253, 4259, 4261, 4271, 4273, 4283, 4289, 4297, 4327,
    4337, 4339, 4349, 4357, 4363, 4373, 4391, 4397, 4409, 4421, 4423, 4441, 4447, 4451, 4457, 4463,
    4481, 4483, 4493, 4507, 4513, 4517, 4519, 4523, 4547, 4549, 4561, 4567, 4583, 4591, 4597, 4603,
    4621, 4637, 4639, 4643, 4649, 4651, 4657, 4663, 4673, 4679, 4691, 4703, 4721, 4723, 4729, 4733,
    4751, 4759, 4783, 4787, 4789, 4793, 4799, 4801, 4813, 4817, 4831, 4861, 4871, 4877, 4889, 4903,
    4909, 4919, 4931, 4933, 4937, 4943, 4951, 4957, 4967, 4969, 4973, 4987, 4993, 4999, 5003,
];
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

/// Inverse of `a` modulo `m`. `a` is second on the stack and `m` is on top.
/// The result `b` satisfies `0 <= b < m` and `a*b ≡ 1 (mod m)`.
pub fn mod_inverse(a: &Value, m: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    let a = as_integer(a, "modular inverse needs integers")?;
    let m = as_integer(m, "modular inverse needs integers")?;
    if !m.is_positive() {
        return Err(CalcError::Domain("modulus must be positive"));
    }
    let eg = a.extended_gcd(&m);
    if !eg.gcd.is_one() {
        return Err(CalcError::Domain("no modular inverse"));
    }
    Ok(Value::from_int(eg.x.mod_floor(&m)))
}

pub fn factorial(v: &Value, mode: &Mode) -> Result<Value, CalcError> {
    if let Some(result) = half_factorial(v, mode.prec) {
        return result.map(|f| Value::real(Real::Float(f)));
    }
    match v {
        Value::Complex(_, _) | Value::Mod(_, _) => {
            Err(CalcError::Domain("factorial needs a real number"))
        }
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

/// Stirling number of the first kind, signed. `n` is second on the stack
/// and `m` is on top. A negative or non-integer argument is an error.
pub fn stirling_first(n: &Value, m: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    stirling(n, m, true)
}

/// Stirling number of the second kind. `n` is second on the stack and `m`
/// is on top. A negative or non-integer argument is an error.
pub fn stirling_second(n: &Value, m: &Value, _mode: &Mode) -> Result<Value, CalcError> {
    stirling(n, m, false)
}

fn stirling(n: &Value, m: &Value, first: bool) -> Result<Value, CalcError> {
    let n = stirling_index(n)?;
    let m = stirling_index(m)?;
    if m > n {
        return Ok(Value::from_int(0));
    }
    if m == n {
        return Ok(Value::from_int(1));
    }
    if m.is_zero() {
        return Ok(Value::from_int(0));
    }
    if m.is_one() {
        // s(n, 1) = (-1)^(n-1) (n-1)!, and S(n, 1) = 1.
        if !first {
            return Ok(Value::from_int(1));
        }
        let n = n.to_u32().ok_or(CalcError::Overflow)?;
        let fact = factorial_int(&BigInt::from(n - 1))?;
        let signed = if (n - 1) % 2 == 0 { fact } else { -fact };
        return Ok(Value::from_int(signed));
    }
    Ok(Value::from_int(stirling_int(&n, &m, first)?))
}

fn stirling_index(v: &Value) -> Result<BigInt, CalcError> {
    let n = as_integer(v, "stirling number needs nonnegative integers")?;
    if n.is_negative() {
        return Err(CalcError::Domain(
            "stirling number needs nonnegative integers",
        ));
    }
    Ok(n)
}

/// The recurrence `S(n, m) = S(n-1, m-1) + c * S(n-1, m)` with `c = 1-n`
/// for the first kind and `c = m` for the second. Only the last row is
/// kept, and columns past `m` are dropped.
fn stirling_int(n: &BigInt, m: &BigInt, first: bool) -> Result<BigInt, CalcError> {
    let n = n.to_u32().ok_or(CalcError::Overflow)?;
    let m = m.to_u32().ok_or(CalcError::Overflow)?;
    if n > MAX_STIRLING_N {
        return Err(CalcError::Overflow);
    }
    let width = m as usize + 1;
    let mut prev = vec![BigInt::zero(); width];
    prev[0] = BigInt::one();
    for row in 1..=n {
        let mut cur = vec![BigInt::zero(); width];
        let top = (row as usize).min(m as usize);
        for (col, cell) in cur.iter_mut().enumerate().take(top + 1).skip(1) {
            let above = &prev[col];
            let step = if first {
                BigInt::from(1i64 - i64::from(row)) * above
            } else {
                BigInt::from(col) * above
            };
            *cell = &prev[col - 1] + step;
        }
        prev = cur;
    }
    Ok(prev[m as usize].clone())
}

/// Largest `n` for `H k n`. The sieve runs to about `n ln n`, so `10^8`
/// sieves to about 2.2 billion.
const NTH_PRIME_MAX: u64 = 100_000_000;

/// `Some(proven)` for a prime, `None` otherwise.
fn prime_kind(n: &BigInt) -> Option<bool> {
    match classify(n) {
        PrimeKind::Prime => Some(true),
        PrimeKind::ProbablyPrime => Some(false),
        PrimeKind::NotPrime => None,
    }
}

fn search_start(v: &Value, what: &'static str) -> Result<BigInt, CalcError> {
    let Value::Real(real) = v else {
        return Err(CalcError::Domain(what));
    };
    let n = crate::value::floor_real(real)?;
    if n.bits() > 8192 {
        return Err(CalcError::Domain("number is too large"));
    }
    Ok(n)
}

/// `k n`, Calc's `calcFunc-nextprime`: the smallest prime above the number.
/// A non-integer is truncated first and anything below 2 gives 2. The bool
/// is false when the answer is only probably prime.
pub fn next_prime(v: &Value) -> Result<(BigInt, bool), CalcError> {
    let mut n = search_start(v, "next prime needs a real number")?;
    if n < BigInt::from(2) {
        return Ok((BigInt::from(2), true));
    }
    if n.is_even() {
        n -= 1u32;
    }
    loop {
        n += 2u32;
        if let Some(proven) = prime_kind(&n) {
            return Ok((n, proven));
        }
    }
}

/// `I k n`, Calc's `calcFunc-prevprime`: the largest prime below the number.
/// A non-integer is rounded up first, and anything below 4 gives 2, as in
/// Calc.
pub fn prev_prime(v: &Value) -> Result<(BigInt, bool), CalcError> {
    let mut n = search_start(v, "previous prime needs a real number")?;
    if crate::value::exact_int(v).is_none() {
        n += 1u32;
    }
    if n < BigInt::from(4) {
        return Ok((BigInt::from(2), true));
    }
    if n.is_even() {
        n += 1u32;
    }
    loop {
        n -= 2u32;
        if let Some(proven) = prime_kind(&n) {
            return Ok((n, proven));
        }
    }
}

/// `H k n`: the nth prime, as PARI/GP's `prime(n)`. `1` is `2`.
pub fn nth_prime(v: &Value) -> Result<Value, CalcError> {
    const NEEDS: CalcError = CalcError::Domain("nth prime needs a positive integer");
    let n = crate::value::exact_int(v).ok_or(NEEDS)?;
    if n < BigInt::one() {
        return Err(NEEDS);
    }
    let n = n
        .to_u64()
        .filter(|&n| n <= NTH_PRIME_MAX)
        .ok_or(CalcError::Overflow)?;
    Ok(Value::from_int(nth_prime_u64(n)))
}

/// Segmented sieve over odd numbers. For `n >= 6` the nth prime is below
/// `n (ln n + ln ln n)` (Rosser).
fn nth_prime_u64(n: u64) -> u64 {
    const FIRST: [u64; 6] = [2, 3, 5, 7, 11, 13];
    if n <= 6 {
        return FIRST[n as usize - 1];
    }
    let x = n as f64;
    let limit = (x * (x.ln() + x.ln().ln())) as u64 + 1;
    let root = limit.isqrt();
    let mut small = vec![true; root as usize + 1];
    let mut base = Vec::new();
    for p in 3..=root {
        if small[p as usize] {
            if p % 2 == 1 {
                base.push(p);
            }
            for m in (p * p..=root).step_by(p as usize) {
                small[m as usize] = false;
            }
        }
    }
    // Index i stands for 2i + 1. Index 0 is 1, so the scan starts at 1.
    const SEGMENT: u64 = 1 << 18;
    let last = (limit - 1) / 2;
    let mut count = 1; // 2
    let mut seg = vec![true; SEGMENT as usize];
    let mut lo = 1;
    while lo <= last {
        let hi = (lo + SEGMENT).min(last + 1);
        let len = (hi - lo) as usize;
        seg[..len].fill(true);
        let low_value = 2 * lo + 1;
        let high_value = 2 * hi + 1;
        for &p in &base {
            if p * p >= high_value {
                break;
            }
            let mut start = low_value.div_ceil(p) * p;
            if start % 2 == 0 {
                start += p;
            }
            start = start.max(p * p);
            let mut i = ((start - 1) / 2 - lo) as usize;
            while i < len {
                seg[i] = false;
                i += p as usize;
            }
        }
        for (j, &is_prime) in seg[..len].iter().enumerate() {
            if is_prime {
                count += 1;
                if count == n {
                    return 2 * (lo + j as u64) + 1;
                }
            }
        }
        lo = hi;
    }
    unreachable!("the bound holds the nth prime")
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

/// Prime factors of an integer, smallest first, with repeats.
///
/// Trial division uses [`FACTOR_PRIMES`] and follows Calc's `calcFunc-prfac`.
/// A cofactor that survives the table is split with Pollard rho. The bool is
/// false when a cofactor may still be composite.
pub fn prime_factors(v: &Value) -> Result<(Vec<BigInt>, bool), CalcError> {
    let n = as_integer(v, "prime factors need an integer")?;
    Ok(factor_integer(n))
}

/// Euler's totient: how many of `1..=n` are coprime to `n`. Follows Calc's
/// `calcFunc-totient`: `0` and `1` are themselves, and a negative number is
/// an error. The answer needs every prime factor, so a cofactor that may
/// still be composite is an error instead of a wrong count.
pub fn totient(v: &Value) -> Result<Value, CalcError> {
    const NEEDS: CalcError = CalcError::Domain("totient needs a nonnegative integer");
    let n = crate::value::exact_int(v).ok_or(NEEDS)?;
    if n.is_negative() {
        return Err(NEEDS);
    }
    if n < BigInt::from(2) {
        return Ok(Value::from_int(n));
    }
    let (factors, complete) = factor_natural(n.clone());
    if !complete {
        return Err(CalcError::Domain("number too big to factor"));
    }
    let mut phi = n;
    let mut last: Option<&BigInt> = None;
    for p in &factors {
        if last != Some(p) {
            phi = phi / p * (p - 1u32);
            last = Some(p);
        }
    }
    Ok(Value::from_int(phi))
}

fn factor_integer(n: BigInt) -> (Vec<BigInt>, bool) {
    if n.is_negative() {
        if n == BigInt::from(-1) {
            return (vec![n], true);
        }
        let (mut factors, complete) = factor_natural(-n);
        factors.insert(0, BigInt::from(-1));
        return (factors, complete);
    }
    if n < BigInt::from(2) {
        return (vec![n], true);
    }
    factor_natural(n)
}

/// `n` is an integer of 2 or more. Trial division follows Calc's prime
/// table. Once the current prime reaches the quotient, the remainder is
/// prime. Anything left after the table goes to Pollard rho.
fn factor_natural(mut n: BigInt) -> (Vec<BigInt>, bool) {
    let mut factors = Vec::new();
    for &p in FACTOR_PRIMES {
        if n.is_one() {
            return (factors, true);
        }
        let prime = BigInt::from(p);
        let (mut quotient, mut rem) = n.div_rem(&prime);
        while rem.is_zero() {
            factors.push(prime.clone());
            n = quotient;
            (quotient, rem) = n.div_rem(&prime);
        }
        // Calc's test is `p >= floor(n / p)`. When it holds, every prime
        // through `p` has been tried, so the remainder is prime.
        if !n.is_one() && prime >= quotient {
            factors.push(n);
            return (factors, true);
        }
    }
    if n.is_one() {
        return (factors, true);
    }
    let complete = factor_remaining(n, &mut factors);
    factors.sort();
    (factors, complete)
}

/// Split a cofactor that survived the trial-division table. A proven prime
/// is finished; a composite is split with Pollard rho and both pieces are
/// factored the same way.
fn factor_remaining(n: BigInt, factors: &mut Vec<BigInt>) -> bool {
    if n.bits() > MAX_POLLARD_BITS {
        factors.push(n);
        return false;
    }
    match classify(&n) {
        PrimeKind::Prime => {
            factors.push(n);
            true
        }
        PrimeKind::ProbablyPrime => {
            factors.push(n);
            false
        }
        PrimeKind::NotPrime => {
            let Some(divisor) = pollard_rho(&n) else {
                factors.push(n);
                return false;
            };
            let quotient = &n / &divisor;
            let left = factor_remaining(divisor, factors);
            let right = factor_remaining(quotient, factors);
            left && right
        }
    }
}

/// Brent's variant of Pollard rho. Returns a nontrivial divisor, or `None`
/// when the budget runs out or every polynomial cycles.
fn pollard_rho(n: &BigInt) -> Option<BigInt> {
    let mut budget = POLLARD_BUDGET;
    for attempt in 1..=POLLARD_ATTEMPTS {
        let c = BigInt::from(attempt);
        if let Some(divisor) = pollard_brent(n, &c, &mut budget) {
            return Some(divisor);
        }
        if budget == 0 {
            break;
        }
    }
    None
}

fn pollard_brent(n: &BigInt, c: &BigInt, budget: &mut u64) -> Option<BigInt> {
    let one = BigInt::one();
    let mut y = BigInt::from(2);
    let mut x = y.clone();
    let mut ys = y.clone();
    let mut q = one.clone();
    let mut g = one.clone();
    let mut r = 1u64;
    while g.is_one() {
        x = y.clone();
        for _ in 0..r {
            if !spend(budget) {
                return None;
            }
            y = rho_step(&y, c, n);
        }
        let mut k = 0u64;
        while k < r && g.is_one() {
            ys = y.clone();
            let block = POLLARD_BLOCK.min(r - k);
            for _ in 0..block {
                if !spend(budget) {
                    return None;
                }
                y = rho_step(&y, c, n);
                q = (q * (&x - &y).abs()) % n;
            }
            g = q.gcd(n);
            k += block;
        }
        r *= 2;
    }
    if g == *n {
        loop {
            if !spend(budget) {
                return None;
            }
            ys = rho_step(&ys, c, n);
            g = (&x - &ys).abs().gcd(n);
            if !g.is_one() {
                break;
            }
        }
    }
    if g.is_one() || g == *n { None } else { Some(g) }
}

fn rho_step(value: &BigInt, c: &BigInt, n: &BigInt) -> BigInt {
    (value * value + c) % n
}

fn spend(budget: &mut u64) -> bool {
    if *budget == 0 {
        false
    } else {
        *budget -= 1;
        true
    }
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
    let proven = n < proven_bound();
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

fn proven_bound() -> &'static BigInt {
    static BOUND: LazyLock<BigInt> =
        LazyLock::new(|| BigInt::parse_bytes(PROVEN_BOUND.as_bytes(), 10).expect("prime bound"));
    &BOUND
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
        Value::Complex(_, _) | Value::Mod(_, _) => Err(CalcError::Domain(msg)),
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

/// `k + 1/2` when `v` is an exact half-integer. `None` for anything else,
/// including a half-integer past the factorial limit, which stays on Spouge.
fn half_index(v: &Value) -> Option<BigInt> {
    let odd = match v {
        Value::Real(Real::Rat(r)) if r.denom() == &BigInt::from(2) => r.numer().clone(),
        Value::Real(Real::Float(f)) => {
            // Doubling can add a digit. Rounding it away turned `5.5001`
            // into `11`, and the answer was `5.5!`.
            let prec = f.prec().max(4).saturating_add(1);
            let twice = f.mul(&DecFloat::from_i64(2, prec), prec);
            let n = twice.to_integer_exact()?;
            if !n.is_odd() {
                return None;
            }
            n
        }
        _ => return None,
    };
    let k: BigInt = (odd - 1) / BigInt::from(2);
    if k.abs() > BigInt::from(MAX_FACTORIAL) {
        None
    } else {
        Some(k)
    }
}

fn half_factorial(v: &Value, prec: u32) -> Option<Result<DecFloat, CalcError>> {
    let k = half_index(v)?;
    Some(half_factorial_k(&k, prec))
}

/// `(k + 1/2)! = (2k + 1)!! / 2^(k + 1) * sqrt(pi)` for `k >= 0`.
/// A negative `k` steps down by `z! = (z + 1)! / (z + 1)`.
fn half_factorial_k(k: &BigInt, prec: u32) -> Result<DecFloat, CalcError> {
    let work = prec.saturating_add(8).max(16);
    let sqrt_pi = DecFloat::pi(work).sqrt(work)?;
    let mut acc = sqrt_pi.div(&DecFloat::from_i64(2, work), work)?;
    if !k.is_negative() {
        let steps = k.to_u32().ok_or(CalcError::Overflow)?;
        for i in 1..=steps {
            let odd = 2 * u64::from(i) + 1;
            let factor =
                DecFloat::from_i64(i64::try_from(odd).map_err(|_| CalcError::Overflow)?, work);
            acc = acc
                .mul(&factor, work)
                .div(&DecFloat::from_i64(2, work), work)?;
        }
        return Ok(acc.polished(prec));
    }
    let steps = (-k).to_u32().ok_or(CalcError::Overflow)?;
    for step in 0..steps {
        let j = -i64::from(step);
        let numer = DecFloat::from_i64(2 * j + 1, work);
        let divisor = numer.div(&DecFloat::from_i64(2, work), work)?;
        if divisor.is_zero() {
            return Err(CalcError::Domain("factorial of a negative integer"));
        }
        acc = acc.div(&divisor, work)?;
    }
    Ok(acc.polished(prec))
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
        let steps = (BigInt::from(2) - x.floor_to_bigint()?)
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

/// Signed Spouge coefficients `c_k` for `k = 1..a`. They depend on `a` and
/// the working precision, not on the argument.
fn spouge_coefficients(work: u32, a: u32) -> Result<Arc<Vec<DecFloat>>, CalcError> {
    type Coeffs = Arc<Vec<DecFloat>>;
    type Cache = HashMap<(u32, u32), Coeffs>;
    static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let key = (work, a);
    if let Some(hit) = CACHE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .get(&key)
        .cloned()
    {
        return Ok(hit);
    }
    let mut coeffs = Vec::with_capacity(a as usize);
    let mut fact = BigInt::one();
    for k in 1..a {
        if k > 1 {
            fact *= BigInt::from(k - 1);
        }
        let ak = a - k;
        let power = int_pow(ak, k);
        let root = DecFloat::from_i64(ak as i64, work).sqrt(work)?;
        let scaled = DecFloat::from_i64(ak as i64, work).exp(work)?;
        let mut magnitude = DecFloat::from_bigint(&power, work)
            .div(&root, work)?
            .div(&DecFloat::from_bigint(&fact, work), work)?
            .mul(&scaled, work);
        if k % 2 == 0 {
            magnitude = magnitude.neg();
        }
        coeffs.push(magnitude);
    }
    let coeffs = Arc::new(coeffs);
    CACHE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .insert(key, Arc::clone(&coeffs));
    Ok(coeffs)
}

/// Spouge's approximation of `z!` for `z > 0`.
fn spouge_factorial(z: &DecFloat, work: u32, a: u32) -> Result<DecFloat, CalcError> {
    let two_pi = DecFloat::pi(work).mul(&DecFloat::from_i64(2, work), work);
    let mut sum = two_pi.sqrt(work)?;
    for (k, ck) in spouge_coefficients(work, a)?.iter().enumerate() {
        let denom = z.add(&DecFloat::from_i64(k as i64 + 1, work), work);
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
        Value::Complex(_, _) | Value::Mod(_, _) => Err(CalcError::Domain(msg)),
    }
}

fn as_integer(v: &Value, msg: &'static str) -> Result<BigInt, CalcError> {
    crate::value::exact_int(v).ok_or(CalcError::Domain(msg))
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
    fn modular_inverse_reduces_the_bezout_coefficient() {
        let mode = mode();
        let check = |a: i64, m: i64, expect: &str| {
            let b = mod_inverse(&Value::from_int(a), &Value::from_int(m), &mode).unwrap();
            assert_eq!(b.to_string(), expect);
            let prod = crate::value::mul(&Value::from_int(a), &b, &mode).unwrap();
            let rem = crate::value::modulo(&prod, &Value::from_int(m), &mode).unwrap();
            assert_eq!(rem.to_string(), "1");
        };
        // 3*5 = 15 ≡ 1 (mod 7). 5*5 = 25 ≡ 1 (mod 24), the Calc tutorial divisor.
        check(3, 7, "5");
        check(5, 24, "5");
        check(10, 7, "5");
        check(-3, 7, "2");
        check(-10, 7, "2");
        assert_eq!(
            mod_inverse(&Value::from_int(8), &Value::from_int(1), &mode)
                .unwrap()
                .to_string(),
            "0"
        );

        let three = Value::real(Real::Float(DecFloat::from_i64(3, 12)));
        assert_eq!(
            mod_inverse(&three, &Value::from_int(7), &mode)
                .unwrap()
                .to_string(),
            "5"
        );

        assert_eq!(
            mod_inverse(&Value::from_int(4), &Value::from_int(6), &mode)
                .unwrap_err()
                .to_string(),
            "no modular inverse"
        );
        assert_eq!(
            mod_inverse(&Value::from_int(5), &Value::from_int(0), &mode)
                .unwrap_err()
                .to_string(),
            "modulus must be positive"
        );
        assert_eq!(
            mod_inverse(&Value::from_int(5), &Value::from_int(-7), &mode)
                .unwrap_err()
                .to_string(),
            "modulus must be positive"
        );
        let half = Value::from_rational(BigRational::new(1.into(), 2.into()));
        assert_eq!(
            mod_inverse(&half, &Value::from_int(7), &mode)
                .unwrap_err()
                .to_string(),
            "modular inverse needs integers"
        );
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
    fn stirling_numbers_match_known_values() {
        let mode = mode();
        let s1 = |n: i64, m: i64| {
            stirling_first(&Value::from_int(n), &Value::from_int(m), &mode)
                .unwrap()
                .to_string()
        };
        let s2 = |n: i64, m: i64| {
            stirling_second(&Value::from_int(n), &Value::from_int(m), &mode)
                .unwrap()
                .to_string()
        };

        // Signed first kind: s(0,0)=1, s(n,0)=0, s(n,n)=1, s(4,2)=11.
        assert_eq!(s1(0, 0), "1");
        assert_eq!(s1(5, 0), "0");
        assert_eq!(s1(5, 5), "1");
        assert_eq!(s1(3, 1), "2");
        assert_eq!(s1(4, 1), "-6");
        assert_eq!(s1(5, 1), "24");
        assert_eq!(s1(4, 2), "11");
        assert_eq!(s1(5, 3), "35");
        assert_eq!(s1(2, 3), "0");

        // Second kind: S(0,0)=1, S(n,0)=0, S(n,n)=1, S(4,2)=7.
        assert_eq!(s2(0, 0), "1");
        assert_eq!(s2(5, 0), "0");
        assert_eq!(s2(5, 5), "1");
        assert_eq!(s2(3, 1), "1");
        assert_eq!(s2(3, 2), "3");
        assert_eq!(s2(4, 2), "7");
        assert_eq!(s2(5, 3), "25");
        assert_eq!(s2(10, 5), "42525");
        assert_eq!(s2(3, 5), "0");
        // Small `m` shortcuts stay exact past the table cap.
        assert_eq!(s1(3000, 0), "0");
        assert_eq!(s2(3000, 1), "1");
        assert_eq!(s2(3000, 3000), "1");
        assert!(stirling_second(&Value::from_int(2001), &Value::from_int(2), &mode).is_err());

        // An exact integer float is accepted, as in Calc.
        let n = Value::real(Real::Float(DecFloat::from_i64(4, 12)));
        let m = Value::real(Real::Float(DecFloat::from_i64(2, 12)));
        assert_eq!(stirling_second(&n, &m, &mode).unwrap().to_string(), "7");

        for bad in [
            Value::from_int(-1),
            Value::from_rational(BigRational::new(3.into(), 2.into())),
        ] {
            assert!(stirling_first(&bad, &Value::from_int(2), &mode).is_err());
            assert!(stirling_second(&Value::from_int(2), &bad, &mode).is_err());
        }
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
    fn half_integer_factorials_agree_with_spouge() {
        let mut wide = mode();
        wide.prec = 20;
        let samples = [
            BigRational::new(BigInt::from(3), BigInt::from(2)),
            BigRational::new(BigInt::from(5), BigInt::from(2)),
            BigRational::new(BigInt::from(-3), BigInt::from(2)),
        ];
        for ratio in samples {
            let Value::Real(Real::Float(got)) =
                factorial(&Value::from_rational(ratio.clone()), &wide).unwrap()
            else {
                panic!("half factorial");
            };
            let x = to_float(&Real::Rat(ratio), 40).unwrap();
            let spouge = factorial_float(&x, 40).unwrap().round_to(20);
            assert_eq!(got.format_decimal(16), spouge.format_decimal(16));
        }

        let third = DecFloat::from_decimal(BigInt::from(3), -1, 30);
        let first = factorial_float(&third, 30).unwrap();
        let second = factorial_float(&third, 30).unwrap();
        assert_eq!(first.format_decimal(30), second.format_decimal(30));
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

    #[test]
    fn nth_prime_matches_pari() {
        let nth = |n: u64| nth_prime(&Value::from_int(n)).map(|v| v.to_string());
        let cases = [
            (1, "2"),
            (2, "3"),
            (6, "13"),
            (7, "17"),
            (25, "97"),
            (168, "997"),
            (1000, "7919"),
            (78498, "999983"),
            (1000000, "15485863"),
        ];
        for (n, want) in cases {
            assert_eq!(nth(n).as_deref(), Ok(want), "prime({n})");
        }
        // Every prime below 2000 in order, against the prime test.
        let mut k = 0;
        for p in 2..2000u64 {
            if trial_prime(p) {
                k += 1;
                assert_eq!(nth_prime_u64(k), p);
            }
        }
        assert!(nth(0).is_err());
        assert_eq!(nth(NTH_PRIME_MAX + 1), Err(CalcError::Overflow));
    }

    #[test]
    fn next_and_previous_prime_follow_calc() {
        let int = |n: i64| Value::from_int(n);
        let next = |v: Value| next_prime(&v).unwrap().0.to_string();
        let prev = |v: Value| prev_prime(&v).unwrap().0.to_string();
        assert_eq!(next(int(-5)), "2");
        assert_eq!(next(int(0)), "2");
        assert_eq!(next(int(2)), "3");
        assert_eq!(next(int(7)), "11");
        assert_eq!(
            next(Value::Real(Real::Rat(BigRational::new(
                10.into(),
                3.into()
            )))),
            "5"
        );
        assert_eq!(prev(int(3)), "2");
        assert_eq!(prev(int(2)), "2");
        assert_eq!(prev(int(-7)), "2");
        assert_eq!(prev(int(11)), "7");
        assert_eq!(
            prev(Value::Real(Real::Rat(BigRational::new(
                21.into(),
                2.into()
            )))),
            "7"
        );
        let big = (BigInt::one() << 89u32) - 2u32;
        let (p, proven) = next_prime(&Value::from_int(big)).unwrap();
        assert_eq!(p.to_string(), "618970019642690137449562111");
        assert!(!proven);
    }

    #[test]
    fn totient_counts_coprimes() {
        let phi = |n: i64| totient(&Value::from_int(BigInt::from(n))).map(|v| v.to_string());
        let cases = [
            (0, "0"),
            (1, "1"),
            (2, "1"),
            (9, "6"),
            (12, "4"),
            (36, "12"),
            (2023, "1632"),
            (1000000007, "1000000006"),
        ];
        for (n, want) in cases {
            assert_eq!(phi(n).as_deref(), Ok(want), "phi({n})");
        }
        let mersenne = (BigInt::one() << 61u32) - 1u32;
        assert_eq!(
            totient(&Value::from_int(mersenne.clone()))
                .unwrap()
                .to_string(),
            (mersenne - 1u32).to_string()
        );
        assert!(phi(-12).is_err());
        let half = Value::Real(Real::Rat(BigRational::new(
            BigInt::from(3),
            BigInt::from(2),
        )));
        assert!(totient(&half).is_err());
    }

    #[test]
    fn prime_factors_are_smallest_first_and_multiply_back() {
        let check = |n: i64, expect: &[i64], complete: bool| {
            let (factors, done) = prime_factors(&Value::from_int(n)).unwrap();
            let expect: Vec<BigInt> = expect.iter().copied().map(BigInt::from).collect();
            assert_eq!(factors, expect, "{n}");
            assert_eq!(done, complete, "{n}");
            let product = factors
                .iter()
                .fold(BigInt::one(), |acc, factor| acc * factor);
            assert_eq!(product, BigInt::from(n));
        };
        check(2023, &[7, 17, 17], true);
        check(12, &[2, 2, 3], true);
        check(8, &[2, 2, 2], true);
        check(17, &[17], true);
        check(4, &[2, 2], true);
        check(1, &[1], true);
        check(0, &[0], true);
        check(-1, &[-1], true);
        check(-2023, &[-1, 7, 17, 17], true);
        check(5009, &[5009], true);
        check(5003 * 5021, &[5003, 5021], true);
        check(4999 * 5003, &[4999, 5003], true);
        // Prime just above 5003 squared. Calc proves it from 5003 itself.
        check(25_030_013, &[25_030_013], true);
        // The table stops at 5003, so 5903 needs Pollard rho.
        check(43_920_492_304, &[2, 2, 2, 2, 13, 5903, 35771], true);
        check(5021 * 5023, &[5021, 5023], true);
        check(5903 * 5903, &[5903, 5903], true);
        check(10007 * 10009, &[10007, 10009], true);
        // A prime past the table is still proved by Miller-Rabin.
        check(1_000_000_007, &[1_000_000_007], true);

        let mersenne: BigInt = (BigInt::one() << 127u32) - 1;
        let (factors, done) = prime_factors(&Value::from_int(mersenne.clone())).unwrap();
        assert_eq!(factors, vec![mersenne]);
        assert!(!done);

        let float = Value::real(Real::Float(DecFloat::from_decimal(
            BigInt::from(2023),
            0,
            12,
        )));
        let (factors, done) = prime_factors(&float).unwrap();
        assert_eq!(
            factors,
            vec![BigInt::from(7), BigInt::from(17), BigInt::from(17)]
        );
        assert!(done);

        let frac = Value::from_rational(BigRational::new(3.into(), 2.into()));
        assert!(prime_factors(&frac).is_err());
    }
}
