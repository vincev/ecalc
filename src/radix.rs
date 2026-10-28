//! Display and entry digits for radices 2 through 36.
//!
//! The stack stores the same numbers either way. A radix only changes how a
//! value is drawn and how a `#` literal is read. Decimal display (no leading
//! zeros) stays on `Display`, so `0.1 + 0.2` is still `0.3`.

use crate::float::{self, DecFloat};
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
    let text = match real {
        // Decimal notation follows `p`, not the digits stored on the float.
        Real::Float(f) if mode.radix == 10 => f.format_decimal(mode.prec),
        Real::Int(_) | Real::Rat(_) if mode.radix == 10 && !mode.leading_zeros => real.to_string(),
        Real::Int(n) => format_int(n, mode),
        Real::Rat(r) if mode.radix == 10 => Real::Rat(r.clone()).to_string(),
        Real::Rat(r) => format_rat(r, mode),
        Real::Float(f) => format_float(f, mode),
    };
    if mode.group_digits {
        group_formatted(&text, mode.radix)
    } else {
        text
    }
}

/// Binary and hex clump every 4 digits. Every other radix clumps every 3.
fn group_width(radix: u32) -> usize {
    if radix == 2 || radix == 16 { 4 } else { 3 }
}

/// Insert commas into integer digit runs of a finished display string.
/// The fractional part and the exponent are left as they are. A fraction
/// groups its numerator and its denominator separately.
fn group_formatted(text: &str, radix: u32) -> String {
    let size = group_width(radix);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    if chars.first() == Some(&'-') {
        out.push('-');
        i = 1;
    }
    if let Some(hash) = chars[i..].iter().position(|&c| c == '#') {
        let mut end = i + hash + 1;
        if chars.get(end) == Some(&'#') {
            end += 1;
        }
        out.extend(chars[i..end].iter().copied());
        i = end;
    }
    group_body(&chars[i..], size, radix, &mut out);
    out
}

fn group_body(chars: &[char], size: usize, radix: u32, out: &mut String) {
    let mut rest = chars;
    loop {
        let split = rest
            .iter()
            .position(|&c| !is_grouped_digit(c, radix))
            .unwrap_or(rest.len());
        push_grouped(&rest[..split], size, out);
        if split == rest.len() {
            return;
        }
        let sep = rest[split];
        out.push(sep);
        rest = &rest[split + 1..];
        if sep != ':' {
            out.extend(rest.iter().copied());
            return;
        }
    }
}

/// `e` is an exponent marker through radix 14. Above that it is a digit.
fn is_grouped_digit(c: char, radix: u32) -> bool {
    if radix <= 14 && matches!(c, 'e' | 'E') {
        return false;
    }
    c.is_ascii_alphanumeric()
}

fn push_grouped(digits: &[char], size: usize, out: &mut String) {
    if size == 0 || digits.len() <= size {
        out.extend(digits.iter().copied());
        return;
    }
    let rem = digits.len() % size;
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && i % size == rem {
            out.push(',');
        }
        out.push(*c);
    }
}

fn format_int(n: &BigInt, mode: &Mode) -> String {
    let bits = word_bits(mode.word_size);
    if mode.twos_complement
        && let Some(text) = format_twos(n, mode.radix, bits)
    {
        return text;
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
        return format!("{}#0.", mode.radix);
    }
    let negative = mantissa.is_negative();
    // Calc's significand length is one shorter than the converted precision.
    // The fixed-or-scientific switch still uses the full conversion of `p`.
    let mant_digits = mantissa.abs().to_str_radix(10).len() as u32;
    let places = mode.prec.max(mant_digits).max(1);
    let show = sig_digits(places, mode.radix).saturating_sub(1).max(1);
    let limit = sig_digits(mode.prec.max(1), mode.radix);
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
    render_float(scaled.round_to(show), negative, limit)
}

fn sig_digits(decimal_prec: u32, radix: u32) -> u32 {
    let need = (decimal_prec.max(1) as f64) * 10f64.ln() / (radix as f64).ln();
    // The nudge catches an estimate that lands just below an integer.
    (need + 1e-9).ceil().max(1.0) as u32
}

fn render_float(value: RFloat, negative: bool, limit: u32) -> String {
    let radix = value.radix;
    let (sig, exp) = strip_trailing(value.sig, value.exp, radix);
    if sig.is_zero() {
        return format!("{radix}#0.");
    }
    let digits = sig.to_str_radix(radix).to_ascii_uppercase();
    let order = exp + digits.len() as i64 - 1;
    // `limit` is the current precision measured in digits of this radix.
    let body = if float::uses_scientific(order, limit) {
        scientific(&digits, order, radix)
    } else {
        float::fixed_point(&digits, exp)
    };
    with_prefix(radix, negative, &body, "#")
}

fn scientific(digits: &str, order: i64, radix: u32) -> String {
    let mant = float::scientific_mantissa(digits);
    // `e` is a digit once the radix passes 14, so the power is written out.
    if radix <= 14 {
        format!("{mant}e{order}")
    } else {
        format!("{mant}*{radix}^{order}")
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
        Mode {
            radix,
            ..Mode::default()
        }
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
        let earth = Value::real(Real::Float(float_at(6_378_137, 0, 12)));
        assert_eq!(format_value(&earth, &mode), "6378137.");
        assert_eq!(format_value(&Value::from_int(6_378_137), &mode), "6378137");
        let zero = Value::real(Real::Float(DecFloat::zero(12)));
        assert_eq!(format_value(&zero, &mode), "0.");
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
    fn binary_and_hex_precision_match_calc() {
        use crate::parse::parse_value;
        // Emacs Calc 31.1, normal format. Hex powers are written `*16^`, which
        // is the form this calculator reads back.
        let cases = [
            (2, 6, "4.2e10", "2#1.001110001110110011e35"),
            (2, 6, "1e12", "2#1.110100011010100101e39"),
            (2, 6, "1e20", "2#1.010110101111000111e66"),
            (2, 6, "0.1", "2#1.10011001100110011e-4"),
            (2, 12, "0.125", "2#1e-3"),
            (2, 12, "0.25", "2#0.01"),
            (2, 12, "2000.", "2#11111010000."),
            (2, 12, "1e12", "2#1110100011010100101001010001000000000000."),
            (2, 12, "4.2e10", "2#100111000111011001010010010000000000."),
            (
                2,
                12,
                "1e20",
                "2#1.0101101011110001110101111000101101011e66",
            ),
            (2, 12, "0.1", "2#1.1001100110011001100110011001100110011e-4"),
            (
                2,
                20,
                "1e20",
                "2#1010110101111000111010111100010110101100011000100000000000000000000.",
            ),
            (
                2,
                20,
                "0.1",
                "2#1.10011001100110011001100110011001100110011001100110011001100110011e-4",
            ),
            (16, 6, "1e12", "16#E.8D5*16^9"),
            (16, 6, "4.2e10", "16#9.C76*16^8"),
            (16, 6, "1e20", "16#5.6BC*16^16"),
            (16, 6, "0.1", "16#0.199A"),
            (16, 6, "0.0625", "16#0.1"),
            (16, 12, "65536.", "16#10000."),
            (16, 12, "1e20", "16#5.6BC75E2D*16^16"),
            (16, 12, "0.1", "16#0.19999999A"),
            (16, 12, "0.00390625", "16#0.01"),
            (16, 20, "1e20", "16#56BC75E2D63100000."),
            (16, 20, "0.1", "16#0.199999999999999A"),
        ];
        for (radix, prec, sample, expect) in cases {
            let entry = Mode {
                prec,
                ..Mode::default()
            };
            let display = Mode {
                prec,
                radix,
                ..Mode::default()
            };
            let value = parse_value(sample, &entry).unwrap_or_else(|err| panic!("{sample}: {err}"));
            assert_eq!(
                format_value(&value, &display),
                expect,
                "{sample} r{radix} p{prec}"
            );
        }

        // Powers that are exact at this precision sit on the radix window.
        // 16^9 is order 9 and stays fixed at precision 12 (10 hex digits).
        // 16^10 is order 10 and turns scientific.
        let mut wide = Mode {
            prec: 12,
            radix: 16,
            ..Mode::default()
        };
        let sixteen = Value::real(Real::Float(DecFloat::from_i64(16, 12)));
        let pow9 = crate::value::pow(&sixteen, &Value::from_int(9), &wide).unwrap();
        assert_eq!(format_value(&pow9, &wide), "16#1000000000.");
        let pow10 = crate::value::pow(&sixteen, &Value::from_int(10), &wide).unwrap();
        assert_eq!(format_value(&pow10, &wide), "16#1*16^10");
        wide.prec = 20;
        let sixteen = Value::real(Real::Float(DecFloat::from_i64(16, 20)));
        let pow10 = crate::value::pow(&sixteen, &Value::from_int(10), &wide).unwrap();
        assert_eq!(format_value(&pow10, &wide), "16#10000000000.");

        let mut bin = Mode {
            prec: 12,
            radix: 2,
            ..Mode::default()
        };
        let two = Value::real(Real::Float(DecFloat::from_i64(2, 12)));
        let pow39 = crate::value::pow(&two, &Value::from_int(39), &bin).unwrap();
        assert_eq!(
            format_value(&pow39, &bin),
            "2#1000000000000000000000000000000000000000."
        );
        bin.prec = 6;
        let two = Value::real(Real::Float(DecFloat::from_i64(2, 6)));
        let pow20 = crate::value::pow(&two, &Value::from_int(20), &bin).unwrap();
        assert_eq!(format_value(&pow20, &bin), "2#1.000000000000000001e20");
    }

    #[test]
    fn floats_follow_the_radix() {
        let hex = mode_radix(16);
        let half = Value::real(Real::Float(float_at(5, -1, 12)));
        assert_eq!(format_value(&half, &hex), "16#0.8");
        let byte = Value::real(Real::Float(float_at(255, 0, 12)));
        assert_eq!(format_value(&byte, &hex), "16#FF.");
        assert_eq!(format_value(&Value::from_int(255), &hex), "16#FF");
        let fifteen = Value::real(Real::Float(float_at(155, -1, 12)));
        assert_eq!(format_value(&fifteen, &hex), "16#F.8");
        let third_digit = Value::real(Real::Float(float_at(3, -1, 12)));
        assert_eq!(format_value(&third_digit, &hex), "16#0.4CCCCCCCD");

        let mut bin = mode_radix(2);
        bin.prec = 12;
        // 2000 is 11 bits. Precision 12 is about 40 bits, so it is written out.
        let two_thousand = Value::real(Real::Float(float_at(2, 3, 1)));
        assert_eq!(format_value(&two_thousand, &bin), "2#11111010000.");

        let zeros = Mode {
            leading_zeros: true,
            ..Mode::default()
        };
        assert_eq!(format_value(&Value::from_int(1), &zeros), "0000000001");

        let huge = Value::real(Real::Float(float_at(1, 20, 12)));
        let text = format_value(&huge, &hex);
        assert_eq!(text, "16#5.6BC75E2D*16^16");
        let back = crate::parse::parse_value(&text, &Mode::default()).unwrap();
        assert_eq!(back.to_string(), "9.99999999983e19");
    }

    #[test]
    fn grouping_clumps_integer_digits_only() {
        let mode = Mode {
            group_digits: true,
            ..Mode::default()
        };
        assert_eq!(format_value(&Value::from_int(123), &mode), "123");
        assert_eq!(format_value(&Value::from_int(1234), &mode), "1,234");
        assert_eq!(
            format_value(&Value::from_int(12_345_678_901_i64), &mode),
            "12,345,678,901"
        );
        assert_eq!(
            format_value(&Value::from_int(-1_234_567), &mode),
            "-1,234,567"
        );

        let ratio = BigRational::new(BigInt::from(1_234_567), BigInt::from(89_012));
        assert_eq!(
            format_value(&Value::from_rational(ratio), &mode),
            "1,234,567:89,012"
        );

        let mixed = Value::real(Real::Float(float_at(1_234_567_123_454, -6, 13)));
        assert_eq!(format_value(&mixed, &mode), "1,234,567.123454");
        let frac_only = Value::real(Real::Float(float_at(123_454, -6, 12)));
        assert_eq!(format_value(&frac_only, &mode), "0.123454");
        let scientific = Value::real(Real::Float(float_at(1, 100, 12)));
        assert_eq!(format_value(&scientific, &mode), "1e100");
        let expanded = Value::real(Real::Float(float_at(42, 9, 2)));
        assert_eq!(format_value(&expanded, &mode), "42,000,000,000.");
        let narrow = Mode {
            group_digits: true,
            prec: 6,
            ..Mode::default()
        };
        assert_eq!(format_value(&expanded, &narrow), "4.2e10");
        let whole = Value::real(Real::Float(float_at(6_378_137, 0, 12)));
        assert_eq!(format_value(&whole, &mode), "6,378,137.");

        let mut hex = mode_radix(16);
        hex.group_digits = true;
        assert_eq!(
            format_value(&Value::from_int(12_345_678_901_i64), &hex),
            "16#2,DFDC,1C35"
        );
        assert_eq!(format_value(&Value::from_int(0xFFFF), &hex), "16#FFFF");
        let wide = Value::real(Real::Float(float_at(655_365, -1, 12)));
        assert_eq!(format_value(&wide, &hex), "16#1,0000.8");

        let mut oct = mode_radix(8);
        oct.group_digits = true;
        assert_eq!(
            format_value(&Value::from_int(12_345_678_901_i64), &oct),
            "8#133,767,016,065"
        );

        let mut bin = mode_radix(2);
        bin.group_digits = true;
        assert_eq!(
            format_value(&Value::from_int(12_345), &bin),
            "2#11,0000,0011,1001"
        );
        bin.leading_zeros = true;
        bin.word_size = 8;
        assert_eq!(format_value(&Value::from_int(1), &bin), "2#0000,0001");

        let mut twos = mode_radix(16);
        twos.group_digits = true;
        twos.twos_complement = true;
        twos.word_size = 32;
        assert_eq!(format_value(&Value::from_int(-3), &twos), "16##FFFF,FFFD");

        let zeros = Mode {
            group_digits: true,
            leading_zeros: true,
            ..Mode::default()
        };
        assert_eq!(format_value(&Value::from_int(1), &zeros), "0,000,000,001");
    }

    #[test]
    fn display_parses_back_to_the_same_number() {
        use crate::parse::parse_value;

        let integers = [
            Value::from_int(0),
            Value::from_int(1),
            Value::from_int(-42),
            Value::from_int(255),
            Value::from_int(1_000_000),
        ];
        let fraction = Value::from_rational(BigRational::new(BigInt::from(3), BigInt::from(4)));
        let complex = Value::complex(Real::Int(BigInt::from(3)), Real::Int(BigInt::from(4)));
        for radix in [2_u32, 8, 10, 16, 36] {
            let mode = mode_radix(radix);
            for value in integers.iter().chain([&fraction, &complex]) {
                let text = format_value(value, &mode);
                let back = parse_value(&text, &mode).unwrap_or_else(|err| panic!("{text}: {err}"));
                assert_eq!(back.to_string(), value.to_string(), "{text}");
            }
        }

        let floats = [
            Value::real(Real::Float(float_at(1, 20, 12))),
            Value::real(Real::Float(float_at(5, -1, 12))),
            Value::real(Real::Float(float_at(-123_456, -3, 12))),
            Value::real(Real::Float(float_at(255, 0, 12))),
            Value::real(Real::Float(float_at(0, 0, 12))),
            Value::real(Real::Float(float_at(-45, 0, 12))),
        ];
        for radix in [2_u32, 10, 16] {
            let mode = mode_radix(radix);
            for value in &floats {
                let text = format_value(value, &mode);
                let back = parse_value(&text, &mode).unwrap_or_else(|err| panic!("{text}: {err}"));
                assert_eq!(format_value(&back, &mode), text, "{text}");
            }
        }

        let mut twos = mode_radix(16);
        twos.twos_complement = true;
        twos.word_size = 8;
        let text = format_value(&Value::from_int(-3), &twos);
        assert_eq!(text, "16##FD");
        assert_eq!(parse_value(&text, &twos).unwrap().to_string(), "-3");
    }
}
