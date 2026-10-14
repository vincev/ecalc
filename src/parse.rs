//! Numeric entry.
//!
//! Integers stay exact. `a:b` is a rational when neither side has a decimal
//! point or an exponent. A dot or an exponent makes a decimal float.
//! `_` is a minus sign, including in an exponent. `(re, im)` is rectangular.

use crate::error::CalcError;
use crate::float::DecFloat;
use crate::value::{self, Real, Value};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

pub fn parse_value(input: &str, prec: u32) -> Result<Value, CalcError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(bad("empty"));
    }
    if let Some((negative, rest)) = sign_before_complex(s) {
        let v = parse_complex(rest, prec)?;
        return Ok(if negative { value::neg(&v) } else { v });
    }
    if s.starts_with('(') {
        return parse_complex(s, prec);
    }
    Ok(Value::real(parse_real(s, prec)?))
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

fn parse_complex(s: &str, prec: u32) -> Result<Value, CalcError> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') || s.len() < 2 {
        return Err(bad(s));
    }
    let inner = &s[1..s.len() - 1];
    let (re, im) = split_comma(inner)?;
    let re = parse_real(re, prec)?;
    let im = parse_real(im, prec)?;
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

fn parse_real(s: &str, prec: u32) -> Result<Real, CalcError> {
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
    let has_exp = body.bytes().any(|b| b == b'e' || b == b'E');
    let has_dot = body.contains('.');
    let has_colon = body.contains(':');
    if has_colon && !has_exp && !has_dot {
        return parse_fraction(body, negative);
    }
    if has_dot || has_exp {
        return parse_float(body, negative, prec);
    }
    parse_int(body, negative)
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
        parse_value(s, 12).unwrap().to_string()
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
        assert_eq!(p("5."), "5");
        assert_eq!(p("_1.5"), "-1.5");
        assert_eq!(p("1.5e1"), "15");
        assert_eq!(p("1.5e20"), "1.5e20");
        assert_eq!(p("1e-6"), "1e-6");
        assert_eq!(p("1.2e_3"), "0.0012");
        assert_eq!(p("1.2E+3"), "1200");
        assert_eq!(p("0.0"), "0");
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
        assert!(parse_value("", 12).is_err());
        assert!(parse_value("1.2.3", 12).is_err());
        assert!(parse_value("1e", 12).is_err());
        assert!(parse_value("1e+", 12).is_err());
        assert!(parse_value("(1,)", 12).is_err());
        assert!(parse_value("(1)", 12).is_err());
        assert!(parse_value("1:", 12).is_err());
        assert!(parse_value("1:0", 12).is_err());
    }
}
