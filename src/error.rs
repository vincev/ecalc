use std::fmt;

/// Recoverable calculator failure. The stack is left unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcError {
    StackUnderflow,
    DivideByZero,
    Domain(&'static str),
    InvalidEntry(String),
    Overflow,
    BadPrecision,
    BadRadix,
    BadWordSize,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StackUnderflow => write!(f, "too few arguments"),
            Self::DivideByZero => write!(f, "division by zero"),
            Self::Domain(msg) => write!(f, "{msg}"),
            Self::InvalidEntry(msg) => write!(f, "bad entry: {msg}"),
            Self::Overflow => write!(f, "overflow"),
            Self::BadPrecision => write!(f, "precision must be from 3 to 1000"),
            Self::BadRadix => write!(f, "radix must be from 2 to 36"),
            Self::BadWordSize => write!(f, "word size must be from -10000 to 10000"),
        }
    }
}

impl std::error::Error for CalcError {}
