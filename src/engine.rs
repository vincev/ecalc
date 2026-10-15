//! RPN stack, undo, and the operations behind the keys.
//!
//! Index 0 is the oldest value. The last element is stack level 1.
//! A checkpoint is taken only after a computation succeeds. It saves the
//! stack and the variables. Mode changes are not undoable. `push` and `dup` leave
//! the inverse and hyperbolic flags alone so `125 RET 3 I ^` works.

use crate::comb;
use crate::error::CalcError;
use crate::float::{MAX_PRECISION, MIN_PRECISION};
use crate::radix::{self, MAX_RADIX, MAX_WORD_SIZE, MIN_RADIX, MIN_WORD_SIZE};
use crate::value::{self, AngleMode, Mode, Real, Value};
use num_bigint::BigInt;
use num_traits::One;
use std::collections::BTreeMap;

const UNDO_CAP: usize = 100;
const MAX_VAR_LEN: usize = 32;

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
    inverse: bool,
    hyperbolic: bool,
    message: String,
    error: bool,
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
            inverse: false,
            hyperbolic: false,
            message: String::new(),
            error: false,
        }
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn inverse(&self) -> bool {
        self.inverse
    }

    pub fn hyperbolic(&self) -> bool {
        self.hyperbolic
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn is_error(&self) -> bool {
        self.error
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

    pub fn clear_flags(&mut self) {
        self.inverse = false;
        self.hyperbolic = false;
    }

    pub fn clear_message(&mut self) {
        self.message.clear();
        self.error = false;
    }

    pub fn toggle_inverse(&mut self) {
        self.inverse = !self.inverse;
        self.clear_message();
    }

    pub fn toggle_hyperbolic(&mut self) {
        self.hyperbolic = !self.hyperbolic;
        self.clear_message();
    }

    pub fn show(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.error = false;
        self.clear_flags();
    }

    pub fn fail(&mut self, err: CalcError) {
        self.message = err.to_string();
        self.error = true;
        self.clear_flags();
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
        self.clear_flags();
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
        self.clear_flags();
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

    pub fn fdiv(&mut self) {
        self.binop(value::fdiv);
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

    /// `k c` chooses. `H k c` counts permutations. The count is on top.
    pub fn choose(&mut self) {
        if self.hyperbolic {
            self.binop(comb::perm);
        } else {
            self.binop(comb::choose);
        }
    }

    /// Pop `y` then `x`. Push `a`, `b`, and `g` so `g` is on top and `g = a*x + b*y`.
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
        if !self.inverse {
            self.binop(value::pow);
            return;
        }
        if self.stack.len() < 2 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        let degree = exact_int(self.stack.last().unwrap());
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
        if self.inverse {
            self.unary(value::square);
        } else {
            self.unary(value::sqrt);
        }
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

    pub fn sine(&mut self) {
        self.unary(match (self.inverse, self.hyperbolic) {
            (false, false) => value::sin,
            (true, false) => value::asin,
            (false, true) => value::sinh,
            (true, true) => value::asinh,
        });
    }

    pub fn cosine(&mut self) {
        self.unary(match (self.inverse, self.hyperbolic) {
            (false, false) => value::cos,
            (true, false) => value::acos,
            (false, true) => value::cosh,
            (true, true) => value::acosh,
        });
    }

    pub fn tangent(&mut self) {
        self.unary(match (self.inverse, self.hyperbolic) {
            (false, false) => value::tan,
            (true, false) => value::atan,
            (false, true) => value::tanh,
            (true, true) => value::atanh,
        });
    }

    /// Dispatches on the flags already set. `L` flips inverse before this.
    pub fn exp_family(&mut self) {
        self.unary(match (self.inverse, self.hyperbolic) {
            (false, false) => value::exp_value,
            (true, false) => value::ln,
            (false, true) => value::exp10,
            (true, true) => value::log10,
        });
    }

    /// `B` is log with the base on top. Inverse is base^exponent, also base on top.
    pub fn logarithm(&mut self) {
        if self.inverse {
            self.binop(|exponent, base, mode| value::pow(base, exponent, mode));
        } else {
            self.binop(|value, base, mode| value::log(value, base, mode));
        }
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

    pub fn push_pi_or_e(&mut self) {
        let hyperbolic = self.hyperbolic;
        self.clear_flags();
        let value = if hyperbolic {
            value::e_const(&self.mode)
        } else {
            value::pi(&self.mode)
        };
        self.push(value);
    }

    pub fn to_float(&mut self) {
        self.unary(value::to_float);
    }

    pub fn to_fraction(&mut self) {
        self.unary(value::to_fraction);
    }

    pub fn set_precision(&mut self, prec: u32) {
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&prec) {
            self.fail(CalcError::BadPrecision);
            return;
        }
        self.mode.prec = prec;
        self.message = format!("precision {prec}");
        self.error = false;
        self.clear_flags();
    }

    pub fn set_degrees(&mut self) {
        self.mode.angle = AngleMode::Degrees;
        self.message = "degrees".to_string();
        self.error = false;
        self.clear_flags();
    }

    pub fn set_radians(&mut self) {
        self.mode.angle = AngleMode::Radians;
        self.message = "radians".to_string();
        self.error = false;
        self.clear_flags();
    }

    pub fn toggle_fraction(&mut self) {
        self.mode.fraction = !self.mode.fraction;
        self.message = if self.mode.fraction {
            "fraction mode".to_string()
        } else {
            "float mode".to_string()
        };
        self.error = false;
        self.clear_flags();
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
        self.message = if self.mode.leading_zeros {
            "leading zeros".to_string()
        } else {
            "no leading zeros".to_string()
        };
        self.error = false;
        self.clear_flags();
    }

    /// `d g`. Integer digits are clumped; the fractional part stays plain.
    pub fn toggle_group_digits(&mut self) {
        self.mode.group_digits = !self.mode.group_digits;
        self.message = if self.mode.group_digits {
            "grouping".to_string()
        } else {
            "no grouping".to_string()
        };
        self.error = false;
        self.clear_flags();
    }

    pub fn set_word_size(&mut self, size: i64) {
        if !(MIN_WORD_SIZE..=MAX_WORD_SIZE).contains(&size) {
            self.fail(CalcError::BadWordSize);
            return;
        }
        self.mode.word_size = size;
        self.message = format!("word size {size}");
        self.error = false;
        self.clear_flags();
    }

    fn note_mode(&mut self, name: String, twos: bool) {
        self.message = if twos {
            format!("{name}, two's complement")
        } else {
            name
        };
        self.error = false;
        self.clear_flags();
    }

    fn succeed(&mut self) {
        self.clear_message();
        self.clear_flags();
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

fn valid_var(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= MAX_VAR_LEN
        && first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric())
}

fn exact_int(value: &Value) -> Option<BigInt> {
    match value {
        Value::Real(Real::Int(n)) => Some(n.clone()),
        Value::Real(Real::Rat(q)) if q.denom().is_one() => Some(q.numer().clone()),
        Value::Real(Real::Float(f)) => f.to_integer_exact(),
        _ => None,
    }
}
