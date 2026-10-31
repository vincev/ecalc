//! RPN stack, undo, and the operations behind the keys.
//!
//! Index 0 is the oldest value. The last element is stack level 1.
//! A checkpoint is taken only after a computation succeeds. It saves the
//! stack and the variables. Mode changes are not undoable. `push` and `dup`
//! only change the stack, so a one-shot prefix can survive them.

use crate::comb;
use crate::error::CalcError;
use crate::float::{MAX_PRECISION, MIN_PRECISION};
use crate::radix::{self, MAX_RADIX, MAX_WORD_SIZE, MIN_RADIX, MIN_WORD_SIZE};
use crate::value::{self, AngleMode, Mode, Value};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// What the prompt's message slot is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Empty,
    Note(String),
    Error(CalcError),
    UnknownKey,
}

const UNDO_CAP: usize = 100;
pub(crate) const MAX_VAR_LEN: usize = 32;

#[derive(Clone, Debug)]
struct Snapshot {
    stack: Vec<Value>,
    vars: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Engine {
    stack: Vec<Value>,
    vars: BTreeMap<String, Value>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    mode: Mode,
    status: Status,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            vars: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            mode: Mode::default(),
            status: Status::Empty,
        }
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Text for the prompt. An error uses [`CalcError`]'s display.
    pub fn message(&self) -> Cow<'_, str> {
        match &self.status {
            Status::Empty => Cow::Borrowed(""),
            Status::Note(text) => Cow::Borrowed(text.as_str()),
            Status::Error(err) => Cow::Owned(err.to_string()),
            Status::UnknownKey => Cow::Borrowed("unknown key"),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self.status, Status::Error(_))
    }

    pub fn stack(&self) -> &[Value] {
        &self.stack
    }

    pub fn top(&self) -> Option<&Value> {
        self.stack.last()
    }

    /// Oldest first. The last line is the top of the stack.
    pub fn stack_lines(&self) -> Vec<String> {
        self.stack
            .iter()
            .map(|value| radix::format_value(value, &self.mode))
            .collect()
    }

    pub fn clear_message(&mut self) {
        self.status = Status::Empty;
    }

    pub fn show(&mut self, msg: impl Into<String>) {
        self.note(msg);
    }

    pub fn unknown_key(&mut self) {
        self.status = Status::UnknownKey;
    }

    pub fn fail(&mut self, err: CalcError) {
        self.status = Status::Error(err);
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.status = Status::Note(msg.into());
    }

    pub fn push(&mut self, value: Value) {
        self.checkpoint();
        self.stack.push(value);
        self.clear_message();
    }

    pub fn dup(&mut self) {
        let Some(top) = self.stack.last().cloned() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        self.checkpoint();
        self.stack.push(top);
        self.clear_message();
    }

    pub fn pop(&mut self) {
        if self.stack.is_empty() {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        self.checkpoint();
        self.stack.pop();
        self.succeed();
    }

    pub fn swap(&mut self) {
        let n = self.stack.len();
        if n < 2 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        self.checkpoint();
        self.stack.swap(n - 1, n - 2);
        self.succeed();
    }

    /// Rotate the top three upward: level 3 becomes the top, and the old top
    /// two each drop one level. This is Calc's `M-TAB` (`calc-roll-up`).
    pub fn roll_up(&mut self) {
        let n = self.stack.len();
        if n < 3 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        self.checkpoint();
        let buried = self.stack.remove(n - 3);
        self.stack.push(buried);
        self.succeed();
    }

    pub fn undo(&mut self) {
        let Some(prev) = self.undo.pop() else {
            self.show("nothing to undo");
            return;
        };
        self.redo.push(self.snapshot());
        self.restore(prev);
        if self.redo.len() > UNDO_CAP {
            self.redo.remove(0);
        }
        self.clear_message();
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo.pop() else {
            self.show("nothing to redo");
            return;
        };
        self.undo.push(self.snapshot());
        self.restore(next);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.clear_message();
    }

    /// Copy the top of the stack into `name`. `pop` removes it afterwards.
    /// A digit variable is stored under `q0`–`q9` by the key map.
    pub fn store(&mut self, name: &str, pop: bool) {
        if !valid_var(name) {
            self.fail(CalcError::Domain("bad variable name"));
            return;
        }
        let Some(value) = self.stack.last().cloned() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        self.checkpoint();
        if pop {
            self.stack.pop();
        }
        self.vars.insert(name.to_string(), value);
        self.show(format!("stored {name}"));
    }

    /// Push the stored value. A missing variable is an error.
    pub fn recall(&mut self, name: &str) {
        if !valid_var(name) {
            self.fail(CalcError::Domain("bad variable name"));
            return;
        }
        let Some(value) = self.vars.get(name).cloned() else {
            self.fail(CalcError::Domain("void variable"));
            return;
        };
        self.checkpoint();
        self.stack.push(value);
        self.show(format!("recalled {name}"));
    }

    /// Drop a stored value. Recalling it afterwards is an error.
    pub fn unstore(&mut self, name: &str) {
        if !valid_var(name) {
            self.fail(CalcError::Domain("bad variable name"));
            return;
        }
        if !self.vars.contains_key(name) {
            self.fail(CalcError::Domain("void variable"));
            return;
        }
        self.checkpoint();
        self.vars.remove(name);
        self.show(format!("unstored {name}"));
    }

    pub fn has_variable(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    pub fn variable_count(&self) -> usize {
        self.vars.len()
    }

    /// Stored names in alphabetical order, with values in the current radix.
    pub fn variable_lines(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(name, value)| (name.clone(), radix::format_value(value, &self.mode)))
            .collect()
    }

    pub fn add(&mut self) {
        self.binop(value::add);
    }

    pub fn sub(&mut self) {
        self.binop(value::sub);
    }

    pub fn mul(&mut self) {
        self.binop(value::mul);
    }

    pub fn div(&mut self) {
        self.binop(value::div);
    }

    pub fn div_fraction(&mut self) {
        self.binop(value::div_fraction);
    }

    pub fn idiv(&mut self) {
        self.binop(value::idiv);
    }

    pub fn modulo(&mut self) {
        self.binop(value::modulo);
    }

    pub fn gcd(&mut self) {
        self.binop(comb::gcd);
    }

    pub fn lcm(&mut self) {
        self.binop(comb::lcm);
    }

    /// Inverse of the second value modulo the top. `3 7 k i` leaves `5`.
    pub fn mod_inverse(&mut self) {
        self.binop(comb::mod_inverse);
    }

    /// `k c`. The count is on top.
    pub fn choose(&mut self) {
        self.binop(comb::choose);
    }

    /// `H k c`. The count is on top.
    pub fn perm(&mut self) {
        self.binop(comb::perm);
    }

    /// `k s`. Stirling number of the first kind. `m` is on top.
    pub fn stirling_first(&mut self) {
        self.binop(comb::stirling_first);
    }

    /// `H k s`. Stirling number of the second kind. `m` is on top.
    pub fn stirling_second(&mut self) {
        self.binop(comb::stirling_second);
    }

    /// `y` is on top and `x` is under it. Push `a`, `b`, and `g` so `g` is on top and `a*x + b*y = g`.
    pub fn egcd(&mut self) {
        let n = self.stack.len();
        if n < 2 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        let outcome = comb::extended_gcd(&self.stack[n - 2], &self.stack[n - 1]);
        match outcome {
            Ok((a, b, g)) => {
                self.checkpoint();
                self.stack.pop();
                self.stack.pop();
                self.stack.push(Value::from_int(a));
                self.stack.push(Value::from_int(b));
                self.stack.push(Value::from_int(g));
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }

    pub fn factorial(&mut self) {
        self.unary(comb::factorial);
    }

    /// Report whether the top integer is prime. The stack stays as it is.
    pub fn prime_test(&mut self) {
        let outcome = self.top().map(comb::prime_message);
        match outcome {
            None => self.fail(CalcError::StackUnderflow),
            Some(Ok(message)) => self.show(message),
            Some(Err(err)) => self.fail(err),
        }
    }

    pub fn power(&mut self) {
        self.binop(value::pow);
    }

    /// Degree on top. An integer degree uses an exact root when there is one.
    pub fn nth_root(&mut self) {
        if self.stack.len() < 2 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        let degree = value::exact_int(self.stack.last().unwrap());
        if let Some(n) = degree {
            self.binop(move |base, _, mode| value::nth_root(base, &n, mode));
        } else {
            self.binop(|base, degree, mode| {
                let inv = value::recip(degree, mode)?;
                value::pow(base, &inv, mode)
            });
        }
    }

    pub fn sqrt(&mut self) {
        self.unary(value::sqrt);
    }

    pub fn square(&mut self) {
        self.unary(value::square);
    }

    pub fn negate(&mut self) {
        self.unary(|v, _| Ok(value::neg(v)));
    }

    pub fn reciprocal(&mut self) {
        self.unary(value::recip);
    }

    pub fn abs(&mut self) {
        self.unary(value::abs);
    }

    pub fn floor(&mut self) {
        self.unary(value::floor);
    }

    pub fn sin(&mut self) {
        self.unary(value::sin);
    }

    pub fn asin(&mut self) {
        self.unary(value::asin);
    }

    pub fn sinh(&mut self) {
        self.unary(value::sinh);
    }

    pub fn asinh(&mut self) {
        self.unary(value::asinh);
    }

    pub fn cos(&mut self) {
        self.unary(value::cos);
    }

    pub fn acos(&mut self) {
        self.unary(value::acos);
    }

    pub fn cosh(&mut self) {
        self.unary(value::cosh);
    }

    pub fn acosh(&mut self) {
        self.unary(value::acosh);
    }

    pub fn tan(&mut self) {
        self.unary(value::tan);
    }

    pub fn atan(&mut self) {
        self.unary(value::atan);
    }

    pub fn tanh(&mut self) {
        self.unary(value::tanh);
    }

    pub fn atanh(&mut self) {
        self.unary(value::atanh);
    }

    pub fn exp(&mut self) {
        self.unary(value::exp_value);
    }

    pub fn ln(&mut self) {
        self.unary(value::ln);
    }

    pub fn exp10(&mut self) {
        self.unary(value::exp10);
    }

    pub fn log10(&mut self) {
        self.unary(value::log10);
    }

    /// Base on top.
    pub fn log(&mut self) {
        self.binop(value::log);
    }

    /// Base on top, raised to the buried power.
    pub fn antilog(&mut self) {
        self.binop(|exponent, base, mode| value::pow(base, exponent, mode));
    }

    pub fn atan2(&mut self) {
        self.binop(value::atan2);
    }

    pub fn hypot(&mut self) {
        self.binop(value::hypot);
    }

    pub fn isqrt(&mut self) {
        self.unary(value::isqrt);
    }

    pub fn abssqr(&mut self) {
        self.unary(value::abssqr);
    }

    pub fn sign(&mut self) {
        self.unary(value::sign);
    }

    pub fn ilog(&mut self) {
        self.binop(value::ilog);
    }

    pub fn expm1(&mut self) {
        self.unary(value::expm1);
    }

    pub fn lnp1(&mut self) {
        self.unary(value::lnp1);
    }

    pub fn conj(&mut self) {
        self.unary(|v, _| Ok(value::conj(v)));
    }

    pub fn arg(&mut self) {
        self.unary(value::arg);
    }

    pub fn push_pi(&mut self) {
        let value = value::pi(&self.mode);
        self.push(value);
    }

    pub fn push_e(&mut self) {
        let value = value::e_const(&self.mode);
        self.push(value);
    }

    pub fn to_float(&mut self) {
        self.unary(value::to_float);
    }

    pub fn to_fraction(&mut self) {
        self.unary(value::to_fraction);
    }

    /// Pop an exact integer and use it as the working precision.
    /// A rejected value stays on the stack. Undo puts the number back and
    /// leaves the precision, because a mode change is not saved.
    pub fn take_precision(&mut self) {
        let Some(top) = self.stack.last() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        let Some(n) = value::exact_int(top) else {
            self.fail(CalcError::Domain("precision needs an integer"));
            return;
        };
        let Ok(prec) = u32::try_from(&n) else {
            self.fail(CalcError::BadPrecision);
            return;
        };
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&prec) {
            self.fail(CalcError::BadPrecision);
            return;
        }
        self.checkpoint();
        self.stack.pop();
        self.set_precision(prec);
    }

    pub fn set_precision(&mut self, prec: u32) {
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&prec) {
            self.fail(CalcError::BadPrecision);
            return;
        }
        self.mode.prec = prec;
        self.note(format!("precision {prec}"));
    }

    pub fn set_degrees(&mut self) {
        self.mode.angle = AngleMode::Degrees;
        self.note("degrees");
    }

    pub fn set_radians(&mut self) {
        self.mode.angle = AngleMode::Radians;
        self.note("radians");
    }

    pub fn toggle_fraction(&mut self) {
        self.mode.fraction = !self.mode.fraction;
        self.note(if self.mode.fraction {
            "fraction mode"
        } else {
            "float mode"
        });
    }

    /// `twos` is Calc's `O` prefix: two's complement for the radix being set.
    pub fn set_radix(&mut self, radix: u32, twos: bool) {
        if !(MIN_RADIX..=MAX_RADIX).contains(&radix) {
            self.fail(CalcError::BadRadix);
            return;
        }
        self.mode.radix = radix;
        self.mode.twos_complement = twos;
        let name = match radix {
            2 => "binary",
            8 => "octal",
            10 => "decimal",
            16 => "hexadecimal",
            other => {
                self.note_mode(format!("radix {other}"), twos);
                return;
            }
        };
        self.note_mode(name.to_string(), twos);
    }

    pub fn toggle_leading_zeros(&mut self) {
        self.mode.leading_zeros = !self.mode.leading_zeros;
        self.note(if self.mode.leading_zeros {
            "leading zeros"
        } else {
            "no leading zeros"
        });
    }

    /// `d g`. Integer digits are clumped; the fractional part stays plain.
    pub fn toggle_group_digits(&mut self) {
        self.mode.group_digits = !self.mode.group_digits;
        self.note(if self.mode.group_digits {
            "grouping"
        } else {
            "no grouping"
        });
    }

    pub fn set_word_size(&mut self, size: i64) {
        if !(MIN_WORD_SIZE..=MAX_WORD_SIZE).contains(&size) {
            self.fail(CalcError::BadWordSize);
            return;
        }
        self.mode.word_size = size;
        self.note(format!("word size {size}"));
    }

    fn note_mode(&mut self, name: String, twos: bool) {
        self.note(if twos {
            format!("{name}, two's complement")
        } else {
            name
        });
    }

    fn succeed(&mut self) {
        self.clear_message();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            stack: self.stack.clone(),
            vars: self.vars.clone(),
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.stack = snap.stack;
        self.vars = snap.vars;
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn unary(&mut self, op: impl FnOnce(&Value, &Mode) -> Result<Value, CalcError>) {
        let result = if let Some(top) = self.stack.last() {
            op(top, &self.mode)
        } else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        match result {
            Ok(value) => {
                self.checkpoint();
                self.stack.pop();
                self.stack.push(value);
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }

    fn binop(&mut self, op: impl FnOnce(&Value, &Value, &Mode) -> Result<Value, CalcError>) {
        let result = if self.stack.len() >= 2 {
            let n = self.stack.len();
            op(&self.stack[n - 2], &self.stack[n - 1], &self.mode)
        } else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        match result {
            Ok(value) => {
                self.checkpoint();
                self.stack.pop();
                self.stack.pop();
                self.stack.push(value);
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }
}

pub(crate) fn valid_var(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= MAX_VAR_LEN
        && first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric())
}

/// Whether appending `c` still yields a variable name.
pub(crate) fn extends_var(name: &str, c: char) -> bool {
    let mut next = String::with_capacity(name.len() + c.len_utf8());
    next.push_str(name);
    next.push(c);
    valid_var(&next)
}
