//! Display and entry digits for radices 2 through 36.
//!
//! The stack stores the same numbers either way. A radix only changes how a
//! value is drawn and how a `#` literal is read. Decimal display (no leading
//! zeros) stays on `Display`, so `0.1 + 0.2` is still `0.3`.

use crate::float::DecFloat;
use crate::value::{Mode, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

pub const MIN_RADIX: u32 = 2;
pub const MAX_RADIX: u32 = 36;
pub const MIN_WORD_SIZE: i64 = -10_000;
pub const MAX_WORD_SIZE: i64 = 10_000;

pub(crate) fn digit_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(u32::from(c) - u32::from('0')),
        'a'..='z' => Some(u32::from(c) - u32::from('a') + 10),
        'A'..='Z' => Some(u32::from(c) - u32::from('A') + 10),
        _ => None,
    }
}

pub(crate) fn is_digit(c: char, radix: u32) -> bool {
    digit_value(c).is_some_and(|value| value < radix)
}

pub(crate) fn format_value(value: &Value, mode: &Mode) -> String {
    match value {
        Value::Real(real) => format_real(real, mode),
        Value::Complex(re, im) => format!("({}, {})", format_real(re, mode), format_real(im, mode)),
    }
}

fn format_real(real: &Real, mode: &Mode) -> String {
    if mode.radix == 10 && !mode.leading_zeros {
        return real.to_string();
    }
    match real {
        Real::Int(n) => format_int(n, mode),
        Real::Rat(r) if mode.radix == 10 => Real::Rat(r.clone()).to_string(),
        Real::Rat(r) => format_rat(r, mode),
        Real::Float(f) if mode.radix == 10 => f.to_string(),
        Real::Float(f) => format_float(f, mode),
    }
}

fn format_int(n: &BigInt, mode: &Mode) -> String {
    let bits = word_bits(mode.word_size);
    if mode.twos_complement {
        if let Some(text) = format_twos(n, mode.radix, bits) {
            return text;
        }
    }
    let mut digits = n.abs().to_str_radix(mode.radix).to_ascii_uppercase();
    if mode.leading_zeros {
        pad_zeros(&mut digits, width_for_bits(bits, mode.radix));
    }
    with_prefix(mode.radix, n.is_negative(), &digits, "#")
}

/// `radix##digits` for an integer in `-2^(w-1) .. 2^(w-1)-1`, radix 2, 8, or 16.
fn format_twos(n: &BigInt, radix: u32, bits: u32) -> Option<String> {
    if bits == 0 || !matches!(radix, 2 | 8 | 16) {
        return None;
    }
    let modulus = BigInt::one() << bits;
    let half = modulus.clone() >> 1;
    let low = -&half;
    if n < &low || n >= &half {
        return None;
    }
    let rep = if n.is_negative() {
        n + &modulus
    } else {
        n.clone()
    };
    let mut digits = rep.to_str_radix(radix).to_ascii_uppercase();
    pad_zeros(&mut digits, width_for_bits(bits, radix));
    Some(format!("{radix}##{digits}"))
}

fn format_rat(r: &BigRational, mode: &Mode) -> String {
    let numer = r
        .numer()
        .abs()
        .to_str_radix(mode.radix)
        .to_ascii_uppercase();
    let denom = r
        .denom()
        .abs()
        .to_str_radix(mode.radix)
        .to_ascii_uppercase();
    with_prefix(
        mode.radix,
        r.is_negative(),
        &format!("{numer}:{denom}"),
        "#",
    )
}

fn format_float(f: &DecFloat, mode: &Mode) -> String {
    let (mantissa, exp10) = f.signed_decimal();
    if mantissa.is_zero() {
        return format!("{}#0", mode.radix);
    }
    let negative = mantissa.is_negative();
    let prec = f.prec().max(mode.prec).max(1);
    let show = sig_digits(prec, mode.radix);
    // Extra digits absorb the rounding from raising 10 to a large power.
    let keep = show.saturating_add(16);
    let man = RFloat::from_bigint(&mantissa.abs(), mode.radix, keep);
    let ten = RFloat::from_bigint(&BigInt::from(10), mode.radix, keep);
    let scale = ten.pow_u(exp10.unsigned_abs());
    let scaled = if exp10 >= 0 {
        man.mul(&scale)
    } else {
        man.div(&scale)
    };
    render_float(scaled.round_to(show), negative)
}

fn sig_digits(decimal_prec: u32, radix: u32) -> u32 {
    let need = (decimal_prec.max(1) as f64) * 10f64.ln() / (radix as f64).ln();
    // The nudge catches an estimate that lands just below an integer.
    (need + 1e-9).ceil().max(1.0) as u32
}

fn render_float(value: RFloat, negative: bool) -> String {
    let radix = value.radix;
    let (sig, exp) = strip_trailing(value.sig, value.exp, radix);
    if sig.is_zero() {
        return format!("{radix}#0");
    }
    let digits = sig.to_str_radix(radix).to_ascii_uppercase();
    let order = exp + digits.len() as i64 - 1;
    let body = if order >= 10 || order <= -5 {
        scientific(&digits, order, radix)
    } else {
        fixed_point(&digits, exp)
    };
    with_prefix(radix, negative, &body, "#")
}

fn scientific(digits: &str, order: i64, radix: u32) -> String {
    let mut frac: String = digits.chars().skip(1).collect();
    while frac.ends_with('0') {
        frac.pop();
    }
    let mant = if frac.is_empty() {
        digits[..1].to_string()
    } else {
        format!("{}.{}", &digits[..1], frac)
    };
    // `e` is a digit once the radix passes 14, so the power is written out.
    if radix <= 14 {
        format!("{mant}e{order}")
    } else {
        format!("{mant}*{radix}^{order}")
    }
}

fn fixed_point(digits: &str, exponent: i64) -> String {
    if exponent >= 0 {
        return format!("{digits}{}", "0".repeat(exponent as usize));
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

fn strip_trailing(mut sig: BigInt, mut exp: i64, radix: u32) -> (BigInt, i64) {
    if sig.is_zero() {
        return (sig, 0);
    }
    let base = BigInt::from(radix);
    while (&sig % &base).is_zero() {
        sig /= &base;
        exp += 1;
    }
    (sig, exp)
}

fn with_prefix(radix: u32, negative: bool, body: &str, hashes: &str) -> String {
    let sign = if negative { "-" } else { "" };
    if radix == 10 {
        format!("{sign}{body}")
    } else {
        format!("{sign}{radix}{hashes}{body}")
    }
}

fn pad_zeros(digits: &mut String, width: usize) {
    if digits.len() < width {
        let zeros = "0".repeat(width - digits.len());
        digits.insert_str(0, &zeros);
    }
}

fn word_bits(word_size: i64) -> u32 {
    word_size.unsigned_abs().min(10_000) as u32
}

/// Digits needed to show every `bits`-wide pattern, i.e. `2^bits - 1`.
fn width_for_bits(bits: u32, radix: u32) -> usize {
    if bits == 0 {
        return 1;
    }
    let mut width = 0usize;
    let mut covered = BigInt::one();
    let limit = BigInt::one() << bits;
    let base = BigInt::from(radix);
    while covered < limit {
        covered *= &base;
        width += 1;
    }
    width.max(1)
}

/// A positive significand in one radix: `sig * radix^exp`, at most `keep` digits.
#[derive(Clone)]
struct RFloat {
    sig: BigInt,
    exp: i64,
    radix: u32,
    keep: u32,
}

impl RFloat {
    fn from_bigint(n: &BigInt, radix: u32, keep: u32) -> Self {
        Self {
            sig: n.abs(),
            exp: 0,
            radix,
            keep: keep.max(1),
        }
        .normalize()
    }

    fn mul(self, other: &Self) -> Self {
        if self.sig.is_zero() || other.sig.is_zero() {
            return Self {
                sig: BigInt::zero(),
                exp: 0,
                radix: self.radix,
                keep: self.keep,
            };
        }
        Self {
            sig: &self.sig * &other.sig,
            exp: self.exp + other.exp,
            radix: self.radix,
            keep: self.keep,
        }
        .normalize()
    }

    fn div(self, other: &Self) -> Self {
        if other.sig.is_zero() || self.sig.is_zero() {
            return Self {
                sig: BigInt::zero(),
                exp: 0,
                radix: self.radix,
                keep: self.keep,
            };
        }
        let digits = |n: &BigInt| n.to_str_radix(self.radix).len() as i64;
        let k = self.keep as i64 - digits(&self.sig) + digits(&other.sig);
        let base = BigInt::from(self.radix);
        let (numer, denom) = if k >= 0 {
            (&self.sig * pow_u(&base, k as u32), other.sig.clone())
        } else {
            (self.sig.clone(), &other.sig * pow_u(&base, (-k) as u32))
        };
        Self {
            sig: round_div(numer, &denom),
            exp: self.exp - other.exp - k,
            radix: self.radix,
            keep: self.keep,
        }
        .normalize()
    }

    fn pow_u(self, mut exp: u64) -> Self {
        if exp == 0 {
            return Self::from_bigint(&BigInt::one(), self.radix, self.keep);
        }
        let mut result = Self::from_bigint(&BigInt::one(), self.radix, self.keep);
        let mut base = self;
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.mul(&base);
            }
            exp >>= 1;
            if exp > 0 {
                base = base.clone().mul(&base);
            }
        }
        result
    }

    fn round_to(self, show: u32) -> Self {
        Self {
            keep: show.max(1),
            ..self
        }
        .normalize()
    }

    fn normalize(mut self) -> Self {
        if self.sig.is_zero() {
            self.exp = 0;
            return self;
        }
        let digits = self.sig.to_str_radix(self.radix).len() as u32;
        if digits <= self.keep {
            return self;
        }
        let drop = digits - self.keep;
        let base = BigInt::from(self.radix);
        let div = pow_u(&base, drop);
        let (q, r) = self.sig.div_rem(&div);
        self.sig = q;
        if (r.clone() << 1) >= div {
            self.sig += 1;
        }
        self.exp += i64::from(drop);
        let limit = pow_u(&base, self.keep);
        if self.sig >= limit {
            self.sig /= &base;
            self.exp += 1;
        }
        self
    }
}

fn round_div(numer: BigInt, denom: &BigInt) -> BigInt {
    let (q, r) = numer.div_rem(denom);
    if (r.clone() << 1) >= *denom { q + 1 } else { q }
}

pub(crate) fn pow_u(base: &BigInt, mut exp: u32) -> BigInt {
    let mut result = BigInt::one();
    let mut base = base.clone();
    while exp > 0 {
        if exp & 1 == 1 {
            result *= &base;
        }
        exp >>= 1;
        if exp > 0 {
            base = &base * &base;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float::DecFloat;

    fn mode_radix(radix: u32) -> Mode {
        let mut mode = Mode::default();
        mode.radix = radix;
        mode
    }

    fn float_at(mantissa: i64, exponent: i64, prec: u32) -> DecFloat {
        DecFloat::from_decimal(BigInt::from(mantissa), exponent, prec)
    }

    #[test]
    fn decimal_display_is_unchanged() {
        let mode = Mode::default();
        assert_eq!(format_value(&Value::from_int(-13), &mode), "-13");
        let ratio = BigRational::new(BigInt::from(-4), BigInt::from(3));
        assert_eq!(format_value(&Value::from_rational(ratio), &mode), "-4:3");
        let fifth = Value::real(Real::Float(float_at(5, -1, 12)));
        assert_eq!(format_value(&fifth, &mode), "0.5");
    }

    #[test]
    fn integers_fractions_and_twos_complement() {
        let hex = mode_radix(16);
        assert_eq!(format_value(&Value::from_int(255), &hex), "16#FF");
        assert_eq!(format_value(&Value::from_int(-10), &hex), "-16#A");
        assert_eq!(format_value(&Value::from_int(0), &hex), "16#0");

        let ratio = BigRational::new(BigInt::from(10), BigInt::from(3));
        assert_eq!(format_value(&Value::from_rational(ratio), &hex), "16#A:3");

        let mut twos = mode_radix(16);
        twos.twos_complement = true;
        twos.word_size = 8;
        assert_eq!(format_value(&Value::from_int(-3), &twos), "16##FD");
        assert_eq!(format_value(&Value::from_int(127), &twos), "16##7F");
        assert_eq!(format_value(&Value::from_int(-128), &twos), "16##80");
        assert_eq!(format_value(&Value::from_int(128), &twos), "16#80");
        assert_eq!(format_value(&Value::from_int(0), &twos), "16##00");
        twos.word_size = -8;
        assert_eq!(format_value(&Value::from_int(-3), &twos), "16##FD");

        let mut bin = mode_radix(2);
        bin.leading_zeros = true;
        bin.word_size = 8;
        assert_eq!(format_value(&Value::from_int(1), &bin), "2#00000001");
        assert_eq!(format_value(&Value::from_int(-1), &bin), "-2#00000001");
    }

    #[test]
    fn floats_follow_the_radix() {
        let hex = mode_radix(16);
        let half = Value::real(Real::Float(float_at(5, -1, 12)));
        assert_eq!(format_value(&half, &hex), "16#0.8");
        let fifteen = Value::real(Real::Float(float_at(155, -1, 12)));
        assert_eq!(format_value(&fifteen, &hex), "16#F.8");
        let third_digit = Value::real(Real::Float(float_at(3, -1, 12)));
        assert_eq!(format_value(&third_digit, &hex), "16#0.4CCCCCCCCD");

        let mut bin = mode_radix(2);
        bin.prec = 12;
        let two_thousand = Value::real(Real::Float(float_at(2, 3, 1)));
        assert_eq!(format_value(&two_thousand, &bin), "2#1.111101e10");

        let mut zeros = Mode::default();
        zeros.leading_zeros = true;
        assert_eq!(format_value(&Value::from_int(1), &zeros), "0000000001");

        let huge = Value::real(Real::Float(float_at(1, 20, 12)));
        let text = format_value(&huge, &hex);
        assert_eq!(text, "16#5.6BC75E2D6*16^16");
        let back = crate::parse::parse_value(&text, &Mode::default()).unwrap();
        assert_eq!(back.to_string(), "9.99999999999e19");
    }
}
