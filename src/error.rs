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

/// Why `eval` rejected a script. The display text matches the on-screen message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    Calc(CalcError),
    UnknownKey,
    EmptyStack,
    Quit,
    Help,
    /// An open list was left without a choice. The text is the message.
    NeedChoice(&'static str),
    /// A prompt such as radix or word size was left unfinished.
    Prompt(&'static str),
    /// A list note, or another message that is not a [`CalcError`].
    Note(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Calc(err) => write!(f, "{err}"),
            Self::UnknownKey => write!(f, "unknown key"),
            Self::EmptyStack => write!(f, "empty stack"),
            Self::Quit => write!(f, "q quits the screen"),
            Self::Help => write!(f, "help is for the screen"),
            Self::NeedChoice(msg) | Self::Prompt(msg) => write!(f, "{msg}"),
            Self::Note(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EvalError {}
