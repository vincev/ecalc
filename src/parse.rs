//! Numeric entry.
//!
//! Integers stay exact. `a:b` is a rational when neither side has a decimal
//! point or an exponent. A dot or an exponent makes a decimal float.
//! `_` is a minus sign, including in an exponent. `(re, im)` is rectangular.
//!
//! A number with no `#` is always decimal. `16#FF` is hex in any radix, and
//! `#FF` uses the current one. `radix##digits` is a two's-complement word.

use crate::error::CalcError;
use crate::float::DecFloat;
use crate::radix::{self, MAX_RADIX, MIN_RADIX};
use crate::value::{self, Mode, Real, Value};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{Num, One, Zero};

/// Largest absolute exponent accepted on entry. The same cap as a radix power.
const MAX_EXPONENT: i64 = 1_000_000;

pub fn parse_value(input: &str, mode: &Mode) -> Result<Value, CalcError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(bad("empty"));
    }
    if let Some((negative, rest)) = sign_before_complex(s) {
        let v = parse_complex(rest, mode)?;
        return Ok(if negative { value::neg(&v) } else { v });
    }
    if s.starts_with('(') {
        return parse_complex(s, mode);
    }
    Ok(Value::real(parse_real(s, mode)?))
}

fn sign_before_complex(s: &str) -> Option<(bool, &str)> {
    let (negative, rest) = match s.as_bytes().first()? {
        b'_' | b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => return None,
    };
    let rest = rest.trim_start();
    rest.starts_with('(').then_some((negative, rest))
}

fn parse_complex(s: &str, mode: &Mode) -> Result<Value, CalcError> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') || s.len() < 2 {
        return Err(bad(s));
    }
    let inner = &s[1..s.len() - 1];
    let (re, im) = split_comma(inner)?;
    let re = parse_real(re, mode)?;
    let im = parse_real(im, mode)?;
    Ok(Value::complex(re, im))
}

fn split_comma(inner: &str) -> Result<(&str, &str), CalcError> {
    let mut depth = 0i32;
    let mut at = None;
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                if at.is_some() {
                    return Err(bad("extra comma"));
                }
                at = Some(i);
            }
            _ => {}
        }
    }
    let Some(i) = at else {
        return Err(bad("complex needs a comma"));
    };
    Ok((&inner[..i], &inner[i + 1..]))
}

fn parse_real(s: &str, mode: &Mode) -> Result<Real, CalcError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(bad("empty"));
    }
    let (negative, body) = match s.as_bytes()[0] {
        b'_' | b'-' => (true, s[1..].trim_start()),
        b'+' => (false, s[1..].trim_start()),
        _ => (false, s),
    };
    if body.is_empty() {
        return Err(bad(s));
    }
    if body.contains('#') {
        return parse_radix_literal(body, negative, mode);
    }
    let has_exp = body.bytes().any(|b| b == b'e' || b == b'E');
    let has_dot = body.contains('.');
    let has_colon = body.contains(':');
    if has_colon && !has_exp && !has_dot {
        return parse_fraction(body, negative);
    }
    if has_dot || has_exp {
        return parse_float(body, negative, mode.prec);
    }
    parse_int(body, negative)
}

fn parse_radix_literal(body: &str, negative: bool, mode: &Mode) -> Result<Real, CalcError> {
    let Some((radix_s, rest)) = body.split_once('#') else {
        return Err(bad(body));
    };
    let radix = if radix_s.is_empty() {
        mode.radix
    } else if !radix_s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(body));
    } else {
        let radix = radix_s.parse::<u32>().map_err(|_| bad(body))?;
        if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
            return Err(bad(body));
        }
        radix
    };
    let (twos, after) = match rest.strip_prefix('#') {
        Some(after) => (true, after),
        None => (false, rest),
    };
    if twos {
        return parse_twos(after, radix, negative, mode);
    }
    parse_radix_number(after, radix, negative, mode)
}

fn parse_twos(after: &str, radix: u32, negative: bool, mode: &Mode) -> Result<Real, CalcError> {
    if after.is_empty() || !after.chars().all(|c| radix::is_digit(c, radix)) {
        return Err(bad(after));
    }
    let bits = mode.word_size.unsigned_abs();
    if bits == 0 || bits > 10_000 {
        return Err(bad("two's complement needs a word size"));
    }
    let pattern = BigInt::from_str_radix(after, radix).map_err(|_| bad(after))?;
    let modulus = BigInt::one() << (bits as usize);
    let pattern = pattern.mod_floor(&modulus);
    let half = modulus.clone() >> 1;
    let mut n = if pattern >= half {
        pattern - modulus
    } else {
        pattern
    };
    if negative {
        n = -n;
    }
    Ok(Real::Int(n))
}

fn parse_radix_number(
    after: &str,
    radix: u32,
    negative: bool,
    mode: &Mode,
) -> Result<Real, CalcError> {
    let bytes = after.as_bytes();
    let mut i = 0;
    let whole_at = i;
    while i < bytes.len() && radix::is_digit(bytes[i] as char, radix) {
        i += 1;
    }
    let whole = &after[whole_at..i];
    let mut frac = "";
    let mut denom_s: Option<&str> = None;
    // `16#FF.` is a float, same as a decimal `255.`. An empty fraction
    // still counts: the point is what distinguishes it from an integer.
    let mut saw_dot = false;
    if i < bytes.len() && bytes[i] == b'.' {
        saw_dot = true;
        i += 1;
        let frac_at = i;
        while i < bytes.len() && radix::is_digit(bytes[i] as char, radix) {
            i += 1;
        }
        frac = &after[frac_at..i];
    } else if i < bytes.len() && bytes[i] == b':' {
        i += 1;
        denom_s = Some(&after[i..]);
        i = bytes.len();
    }
    let mut power = 0i64;
    let mut saw_exp = false;
    if denom_s.is_none() && i < bytes.len() {
        power = parse_radix_exponent(&after[i..], radix)?;
        saw_exp = true;
        i = bytes.len();
    }
    if i != bytes.len() {
        return Err(bad(after));
    }
    if let Some(denom_s) = denom_s {
        if whole.is_empty() {
            return Err(bad(after));
        }
        return parse_radix_fraction(whole, denom_s, radix, negative);
    }
    if whole.is_empty() && frac.is_empty() {
        return Err(bad(after));
    }
    let digits = format!("{whole}{frac}");
    let value = if digits.chars().all(|c| c == '0') {
        BigInt::zero()
    } else {
        BigInt::from_str_radix(&digits, radix).map_err(|_| bad(after))?
    };
    let power = power - frac.len() as i64;
    radix_power_to_real(
        value,
        power,
        radix,
        mode.prec,
        !frac.is_empty() || saw_exp || saw_dot,
        negative,
    )
}

fn parse_radix_fraction(
    numer_s: &str,
    denom_s: &str,
    radix: u32,
    negative: bool,
) -> Result<Real, CalcError> {
    let mut numer = BigInt::from_str_radix(numer_s, radix).map_err(|_| bad(numer_s))?;
    let (denom_neg, denom_digits) = match denom_s.as_bytes().first().copied() {
        Some(b'_' | b'-') => (true, &denom_s[1..]),
        Some(b'+') => (false, &denom_s[1..]),
        _ => (false, denom_s),
    };
    if denom_digits.is_empty() || !denom_digits.chars().all(|c| radix::is_digit(c, radix)) {
        return Err(bad(denom_s));
    }
    let mut denom = BigInt::from_str_radix(denom_digits, radix).map_err(|_| bad(denom_s))?;
    if denom.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    if negative {
        numer = -numer;
    }
    if denom_neg {
        denom = -denom;
    }
    Ok(match Value::from_rational(BigRational::new(numer, denom)) {
        Value::Real(real) => real,
        Value::Complex(_, _) => unreachable!("a rational is real"),
    })
}

/// The exponent is written in decimal and is a power of `radix`.
fn parse_radix_exponent(s: &str, radix: u32) -> Result<i64, CalcError> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(bad(s));
    }
    if radix <= 14 && (bytes[0] == b'e' || bytes[0] == b'E') {
        return parse_decimal_exp(&s[1..]);
    }
    if radix >= 15 && bytes[0] == b'*' {
        let Some((rad_s, exp_s)) = s[1..].split_once('^') else {
            return Err(bad(s));
        };
        if rad_s.parse::<u32>().ok().as_ref() != Some(&radix) {
            return Err(bad(s));
        }
        return parse_decimal_exp(exp_s);
    }
    Err(bad(s))
}

fn parse_decimal_exp(s: &str) -> Result<i64, CalcError> {
    if s.is_empty() {
        return Err(bad(s));
    }
    let (neg, digits) = match s.as_bytes()[0] {
        b'+' => (false, &s[1..]),
        b'-' | b'_' => (true, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(s));
    }
    let exp: i64 = digits.parse().map_err(|_| CalcError::Overflow)?;
    Ok(if neg { -exp } else { exp })
}

fn radix_power_to_real(
    digits: BigInt,
    power: i64,
    radix: u32,
    prec: u32,
    as_float: bool,
    negative: bool,
) -> Result<Real, CalcError> {
    if power.checked_abs().is_none_or(|v| v > MAX_EXPONENT) {
        return Err(CalcError::Overflow);
    }
    let magnitude = if power >= 0 {
        &digits * radix::pow_u(&BigInt::from(radix), power as u32)
    } else {
        BigInt::zero()
    };
    if !as_float && power >= 0 {
        return Ok(Real::Int(if negative { -magnitude } else { magnitude }));
    }
    let prec = prec.max(1);
    let float = if power >= 0 {
        DecFloat::from_bigint(&magnitude, prec)
    } else {
        let denom = radix::pow_u(&BigInt::from(radix), (-power) as u32);
        let numer = DecFloat::from_bigint(&digits, prec.saturating_add(8));
        let den = DecFloat::from_bigint(&denom, prec.saturating_add(8));
        numer.div(&den, prec)?
    };
    Ok(Real::Float(if negative { float.neg() } else { float }))
}

fn parse_int(body: &str, negative: bool) -> Result<Real, CalcError> {
    let n = digits_to_int(body)?;
    Ok(Real::Int(if negative { -n } else { n }))
}

fn parse_fraction(body: &str, negative: bool) -> Result<Real, CalcError> {
    let Some((numer_s, denom_s)) = body.split_once(':') else {
        return Err(bad(body));
    };
    let mut numer = digits_to_int(numer_s)?;
    let (denom_neg, denom_digits) = match denom_s.as_bytes().first() {
        Some(b'_' | b'-') => (true, &denom_s[1..]),
        Some(b'+') => (false, &denom_s[1..]),
        _ => (false, denom_s),
    };
    let mut denom = digits_to_int(denom_digits)?;
    if denom.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    if negative {
        numer = -numer;
    }
    if denom_neg {
        denom = -denom;
    }
    let ratio = BigRational::new(numer, denom);
    Ok(match Value::from_rational(ratio) {
        Value::Real(real) => real,
        Value::Complex(_, _) => unreachable!("a rational is real"),
    })
}

fn parse_float(body: &str, negative: bool, mode_prec: u32) -> Result<Real, CalcError> {
    let bytes = body.as_bytes();
    let mut i = 0;
    let whole_at = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let whole = &body[whole_at..i];
    let mut frac = "";
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let frac_at = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac = &body[frac_at..i];
    }
    let mut exp: i64 = 0;
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i >= bytes.len() {
            return Err(bad(body));
        }
        let exp_neg = matches!(bytes[i], b'-' | b'_');
        if matches!(bytes[i], b'+' | b'-' | b'_') {
            i += 1;
        }
        if i >= bytes.len() || !bytes[i].is_ascii_digit() {
            return Err(bad(body));
        }
        let exp_at = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        exp = body[exp_at..i]
            .parse::<i64>()
            .map_err(|_| bad("exponent"))?;
        if exp_neg {
            exp = -exp;
        }
    }
    if i != bytes.len() || (whole.is_empty() && frac.is_empty()) {
        return Err(bad(body));
    }
    if exp.checked_abs().is_none_or(|v| v > MAX_EXPONENT) {
        return Err(CalcError::Overflow);
    }
    let digits = format!("{whole}{frac}");
    let mut mantissa = if digits.bytes().all(|b| b == b'0') {
        BigInt::zero()
    } else {
        digits_to_int(&digits)?
    };
    if negative {
        mantissa = -mantissa;
    }
    let exponent = exp - frac.len() as i64;
    let typed = significant_digits(&digits).max(1);
    let prec = typed.min(mode_prec.max(1));
    Ok(Real::Float(DecFloat::from_decimal(
        mantissa, exponent, prec,
    )))
}

fn significant_digits(digits: &str) -> u32 {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        1
    } else {
        trimmed.len() as u32
    }
}

fn digits_to_int(s: &str) -> Result<BigInt, CalcError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad(s));
    }
    BigInt::parse_bytes(s.as_bytes(), 10).ok_or_else(|| bad(s))
}

fn bad(s: &str) -> CalcError {
    CalcError::InvalidEntry(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> String {
        parse_value(s, &Mode::default()).unwrap().to_string()
    }

    #[test]
    fn integers_fractions_and_signs() {
        assert_eq!(p("42"), "42");
        assert_eq!(p("_3"), "-3");
        assert_eq!(p("-3"), "-3");
        assert_eq!(p("+8"), "8");
        assert_eq!(p("007"), "7");
        assert_eq!(p("3:4"), "3:4");
        assert_eq!(p("_4:3"), "-4:3");
        assert_eq!(p("8:6"), "4:3");
        assert_eq!(p("4:2"), "2");
        assert_eq!(p("4:_2"), "-2");
    }

    #[test]
    fn decimals() {
        assert_eq!(p("9.5"), "9.5");
        assert_eq!(p(".5"), "0.5");
        assert_eq!(p("5."), "5.");
        assert_eq!(p("_1.5"), "-1.5");
        assert_eq!(p("1.5e1"), "15.");
        assert_eq!(p("1.5e20"), "1.5e20");
        assert_eq!(p("1e-6"), "1e-6");
        assert_eq!(p("1.2e_3"), "1.2e-3");
        assert_eq!(p("4.2e10"), "42000000000.");
        assert_eq!(p("0.01"), "0.01");
        assert_eq!(p("1.2E+3"), "1200.");
        assert_eq!(p("0.0"), "0.");
        assert_eq!(p("1e1000000"), "1e1000000");
        assert_eq!(p("1e-1000000"), "1e-1000000");
    }

    #[test]
    fn huge_decimal_exponents_overflow() {
        let mode = Mode::default();
        for entry in [
            "1e1000001",
            "1e-1000001",
            "1e9223372036854775807",
            "_1e9223372036854775807",
        ] {
            assert!(
                matches!(parse_value(entry, &mode), Err(CalcError::Overflow)),
                "{entry}"
            );
        }
        assert!(matches!(
            parse_value("1e9223372036854775808", &mode),
            Err(CalcError::InvalidEntry(_))
        ));
    }

    #[test]
    fn complex_values() {
        assert_eq!(p("(3, 4)"), "(3, 4)");
        assert_eq!(p("(3,4)"), "(3, 4)");
        assert_eq!(p("( 0 , 1 )"), "(0, 1)");
        assert_eq!(p("(3, 0)"), "3");
        assert_eq!(p("_(3, 4)"), "(-3, -4)");
        assert_eq!(p("(1:2, 3:4)"), "(1:2, 3:4)");
        assert_eq!(p("(1.5, _2)"), "(1.5, -2)");
    }

    #[test]
    fn rejects_incomplete_entry() {
        let mode = Mode::default();
        assert!(parse_value("", &mode).is_err());
        assert!(parse_value("1.2.3", &mode).is_err());
        assert!(parse_value("1e", &mode).is_err());
        assert!(parse_value("1e+", &mode).is_err());
        assert!(parse_value("(1,)", &mode).is_err());
        assert!(parse_value("(1)", &mode).is_err());
        assert!(parse_value("1:", &mode).is_err());
        assert!(parse_value("1:0", &mode).is_err());
        assert!(parse_value("16#", &mode).is_err());
        assert!(parse_value("16#G", &mode).is_err());
        assert!(parse_value("1#10", &mode).is_err());
    }

    #[test]
    fn radix_literals_are_not_stored_in_that_radix() {
        assert_eq!(p("16#FF"), "255");
        assert_eq!(p("16#ff"), "255");
        assert_eq!(p("2#1010"), "10");
        assert_eq!(p("8#17"), "15");
        assert_eq!(p("36#Z"), "35");
        assert_eq!(p("_16#A"), "-10");
        assert_eq!(p("16#F.8"), "15.5");
        assert_eq!(p("16#0.8"), "0.5");
        assert_eq!(p("8#10e1"), "64.");
        assert_eq!(p("8#10."), "8.");
        assert_eq!(p("16#FF."), "255.");
        assert_eq!(p("8#1e_2"), "0.015625");
        assert_eq!(p("16#F*16^2"), "3840.");
        assert_eq!(p("16#1*16^-1"), "0.0625");
        assert_eq!(p("16#A:3"), "10:3");
        assert_eq!(p("(16#3, 16#4)"), "(3, 4)");
        assert_eq!(p("10"), "10");

        let hex = Mode {
            radix: 16,
            ..Mode::default()
        };
        assert_eq!(parse_value("#FF", &hex).unwrap().to_string(), "255");
        assert_eq!(parse_value("10", &hex).unwrap().to_string(), "10");

        let word = Mode {
            word_size: 8,
            ..Mode::default()
        };
        assert_eq!(parse_value("16##FD", &word).unwrap().to_string(), "-3");
        assert_eq!(
            parse_value("2##11110101", &word).unwrap().to_string(),
            "-11"
        );
    }
}
