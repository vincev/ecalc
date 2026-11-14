//! RPN stack, undo, and the operations behind the keys.
//!
//! Index 0 is the oldest value. The last element is stack level 1.
//! A checkpoint is taken only after a computation succeeds. It saves the
//! stack and the variables. Mode changes are not undoable. `push` and `dup`
//! only change the stack, so a one-shot prefix can survive them.
//!
//! Every pushed number and every result goes on the trail, with the label
//! the key map set for the command. The trail is a log, so undo leaves it.
//!
//! `unary` and `binop` reject a modulo form. The operations Calc defines on
//! one (`+ - * / ^`, negate, reciprocal, square) use `unary_mod` and
//! `binop_mod`, so a new operation cannot take one by accident.
//!
//! All four go element by element through a vector (`vector::map1` and
//! `vector::map2`), so a new operation works on vectors without more code.
//! The few that mean something else for a whole vector (`*`, `/`, `^`, `A`,
//! `I Q`, `f A`, `&`) use `unary_whole` and `binop_whole` and the functions
//! in `vector.rs`. `V M` makes those go element by element too, and `V R`
//! folds a binary operation over the top vector.

use crate::comb;
use crate::error::CalcError;
use crate::float::{MAX_PRECISION, MIN_PRECISION};
use crate::radix::{self, MAX_RADIX, MAX_WORD_SIZE, MIN_RADIX, MIN_WORD_SIZE};
use crate::value::{self, AngleMode, Mode, NOT_FOR_MOD, Value};
use crate::vector;
use num_bigint::BigInt;
use std::borrow::Cow;
use std::cell::Cell;
use std::collections::{BTreeMap, VecDeque};

/// What the prompt's message slot is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Empty,
    Note(String),
    Error(CalcError),
    UnknownKey,
}

/// `V M` and `V R`: how the next operation treats a vector. The key map
/// sets it before the command and clears it after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorMode {
    /// Element by element, even for `*`, `/`, `^`, `A`, `I Q`, `f A`, `&`.
    Map,
    /// Fold a binary operation over the elements of the top vector.
    Reduce,
}

const REDUCE_NEEDS_BINARY: CalcError = CalcError::Domain("V R needs a binary operation");
const REDUCE_NEEDS_VECTOR: CalcError = CalcError::Domain("V R needs a vector");
const REDUCE_EMPTY: CalcError = CalcError::Domain("V R of an empty vector");

const UNDO_CAP: usize = 100;
pub(crate) const MAX_VAR_LEN: usize = 32;
/// Trail entries kept. The oldest is dropped first.
pub(crate) const TRAIL_CAP: usize = 1000;
/// Digits the trail may hold, in bytes. A run of million-digit results
/// drops the oldest entries before the count cap would.
pub(crate) const TRAIL_BYTES: usize = 64 << 20;

/// One value on the trail. `label` is the command that made it, such as
/// `*` or `k f`, and is empty for a typed number.
#[derive(Clone, Debug)]
pub struct TrailEntry {
    pub label: String,
    pub value: Value,
}

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
    trail: VecDeque<TrailEntry>,
    /// Sum of `approx_bytes` over the trail.
    trail_bytes: usize,
    /// Label for the next trail entries. The key map sets it per command.
    label: String,
    /// `V M` or `V R` for the command now running.
    vector_mode: Option<VectorMode>,
    /// What `V R` of an empty vector gives for the operation now running:
    /// `0` for `+` and `-`, `1` for `*`, as in Calc. Other operations have none.
    reduce_identity: Option<i64>,
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
            trail: VecDeque::new(),
            trail_bytes: 0,
            label: String::new(),
            vector_mode: None,
            reduce_identity: None,
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
        self.record(&value);
        self.stack.push(value);
        self.clear_message();
    }

    /// Label the trail entries of the next command. An empty label is a
    /// typed number.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// `V M` or `V R` for the next command. The key map clears it after.
    pub fn set_vector_mode(&mut self, mode: Option<VectorMode>) {
        self.vector_mode = mode;
    }

    fn mapping(&self) -> bool {
        self.vector_mode == Some(VectorMode::Map)
    }

    /// Oldest first.
    pub fn trail(&self) -> &VecDeque<TrailEntry> {
        &self.trail
    }

    /// Push a trail value again. It is not recorded a second time.
    pub fn yank(&mut self, index: usize) {
        let Some(value) = self.trail.get(index).map(|entry| entry.value.clone()) else {
            self.fail(CalcError::Domain("no trail entry"));
            return;
        };
        self.checkpoint();
        self.stack.push(value);
        self.clear_message();
    }

    fn record(&mut self, value: &Value) {
        let bytes = value.approx_bytes();
        while !self.trail.is_empty()
            && (self.trail.len() >= TRAIL_CAP
                || self.trail_bytes.saturating_add(bytes) > TRAIL_BYTES)
        {
            if let Some(old) = self.trail.pop_front() {
                self.trail_bytes = self.trail_bytes.saturating_sub(old.value.approx_bytes());
            }
        }
        self.trail_bytes = self.trail_bytes.saturating_add(bytes);
        self.trail.push_back(TrailEntry {
            label: self.label.clone(),
            value: value.clone(),
        });
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
        self.label = format!("rcl {name}");
        self.record(&value);
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
        self.reduce_identity = Some(0);
        self.binop_mod(value::add);
        self.reduce_identity = None;
    }

    pub fn sub(&mut self) {
        self.reduce_identity = Some(0);
        self.binop_mod(value::sub);
        self.reduce_identity = None;
    }

    /// Two vectors give the dot product. `V M *` multiplies element by element.
    pub fn mul(&mut self) {
        self.reduce_identity = Some(1);
        self.mul_inner();
        self.reduce_identity = None;
    }

    fn mul_inner(&mut self) {
        if self.mapping() {
            self.binop_mod(value::mul);
        } else {
            self.binop_whole(vector::mul);
        }
    }

    pub fn div(&mut self) {
        if self.mapping() {
            self.binop_mod(value::div);
        } else {
            self.binop_whole(vector::div);
        }
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
                for value in [a, b, g].map(Value::from_int) {
                    self.record(&value);
                    self.stack.push(value);
                }
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }

    pub fn factorial(&mut self) {
        self.unary(comb::factorial);
    }

    /// `k p`. Replace the top integer with `1` if it is prime and `0` if not,
    /// as Calc's `prime` function. The message says `prime`, `not prime`, or
    /// `probably prime`. A vector is tested element by element.
    pub fn prime_test(&mut self) {
        let unproven = Cell::new(false);
        let vector = matches!(self.top(), Some(Value::Vector(_)));
        let verdict = Cell::new("");
        self.unary(|v, _| {
            let (prime, proven) = comb::prime_test(v)?;
            if prime && !proven {
                unproven.set(true);
            }
            verdict.set(match (prime, proven) {
                (false, _) => "not prime",
                (true, true) => "prime",
                (true, false) => "probably prime",
            });
            Ok(Value::from_int(u8::from(prime)))
        });
        if self.is_error() {
            return;
        }
        if !vector {
            self.show(verdict.get());
        } else if unproven.get() {
            self.show("some are only probably prime");
        }
    }

    /// `k t`. Euler's totient of the top integer.
    pub fn totient(&mut self) {
        self.unary(|v, _| comb::totient(v));
    }

    /// `k n`. The next prime above the top.
    pub fn next_prime(&mut self) {
        self.prime_search(comb::next_prime);
    }

    /// `I k n`. The previous prime below the top.
    pub fn prev_prime(&mut self) {
        self.prime_search(comb::prev_prime);
    }

    /// `H k n`. The nth prime.
    pub fn nth_prime(&mut self) {
        self.unary(|v, _| comb::nth_prime(v));
    }

    /// Replace the top with a prime found from it. An unproven prime still
    /// replaces the top, with the message `probably prime`.
    fn prime_search(&mut self, op: fn(&Value) -> Result<(BigInt, bool), CalcError>) {
        let unproven = Cell::new(false);
        self.unary(|v, _| {
            let (prime, proven) = op(v)?;
            if !proven {
                unproven.set(true);
            }
            Ok(Value::from_int(prime))
        });
        if !self.is_error() && unproven.get() {
            self.show("probably prime");
        }
    }

    /// `k f`. Replace the top integer with the vector of its prime factors,
    /// smallest first, as Calc's `prfac`: `2023` becomes `[7, 17, 17]`. A
    /// vector of integers gives a vector of factor lists.
    pub fn prime_factors(&mut self) {
        let incomplete = Cell::new(false);
        self.unary(|v, _| {
            let (factors, complete) = comb::prime_factors(v)?;
            if !complete {
                incomplete.set(true);
            }
            Ok(Value::Vector(
                factors.into_iter().map(Value::from_int).collect(),
            ))
        });
        if !self.is_error() && incomplete.get() {
            self.show("may not be fully factored");
        }
    }

    /// `v p`. Pop a count, then pack that many values into one vector, the
    /// deepest first. `1 2 3 3 v p` is `[1, 2, 3]`.
    pub fn pack(&mut self) {
        let Some(top) = self.stack.last() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        let Some(n) = value::exact_int(top).and_then(|n| usize::try_from(n).ok()) else {
            self.fail(CalcError::Domain("pack needs a count"));
            return;
        };
        let len = self.stack.len();
        if n > len - 1 {
            self.fail(CalcError::StackUnderflow);
            return;
        }
        let deepest = self.stack[len - 1 - n..len - 1]
            .iter()
            .map(Value::depth)
            .max()
            .unwrap_or(0);
        if deepest >= vector::MAX_DEPTH {
            self.fail(vector::TOO_DEEP);
            return;
        }
        self.checkpoint();
        self.stack.pop();
        let items = self.stack.split_off(len - 1 - n);
        let packed = Value::Vector(items);
        self.record(&packed);
        self.stack.push(packed);
        self.succeed();
    }

    /// `v u`. Replace the top vector with its elements, the last on top.
    pub fn unpack(&mut self) {
        let Some(top) = self.stack.last() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        let Value::Vector(items) = top.clone() else {
            self.fail(CalcError::Domain("unpack needs a vector"));
            return;
        };
        self.checkpoint();
        self.stack.pop();
        self.stack.extend(items);
        self.succeed();
    }

    /// `v x`. Replace a count `n` with `[1, 2, ..., n]`, as Calc's `index`.
    /// It does not map over a vector: `[n, m] v x` would build up to
    /// `MAX_INDEX` elements per element and get round the cap.
    pub fn index(&mut self) {
        self.unary_whole(|v, _| {
            let n = value::exact_int(v)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or(CalcError::Domain("index needs a nonnegative integer"))?;
            if n > vector::MAX_INDEX {
                return Err(CalcError::Overflow);
            }
            Ok(Value::Vector((1..=n).map(Value::from_int).collect()))
        });
    }

    /// `v l`. Replace the top vector with its number of elements.
    pub fn length(&mut self) {
        self.unary_whole(|v, _| match v {
            Value::Vector(items) => Ok(Value::from_int(items.len())),
            _ => Err(CalcError::Domain("length needs a vector")),
        });
    }

    pub fn power(&mut self) {
        if self.mapping() {
            self.binop_mod(value::pow);
        } else {
            self.binop_whole(vector::pow);
        }
    }

    /// Degree on top. An integer degree uses an exact root when there is one.
    pub fn nth_root(&mut self) {
        self.binop(|base, degree, mode| match value::exact_int(degree) {
            Some(n) => value::nth_root(base, &n, mode),
            None => {
                let inv = value::recip(degree, mode)?;
                value::pow(base, &inv, mode)
            }
        });
    }

    pub fn sqrt(&mut self) {
        self.unary(value::sqrt);
    }

    pub fn square(&mut self) {
        if self.mapping() {
            self.unary_mod(value::square);
        } else {
            self.unary_whole(vector::square);
        }
    }

    pub fn negate(&mut self) {
        self.unary_mod(|v, _| Ok(value::neg(v)));
    }

    pub fn reciprocal(&mut self) {
        if self.mapping() {
            self.unary_mod(value::recip);
        } else {
            self.unary_whole(vector::recip);
        }
    }

    /// A vector gives its length, as Calc's `abs`: `[3, 4]` is `5`.
    pub fn abs(&mut self) {
        if self.mapping() {
            self.unary(value::abs);
        } else {
            self.unary_whole(vector::abs);
        }
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
        if self.mapping() {
            self.unary(value::abssqr);
        } else {
            self.unary_whole(vector::abssqr);
        }
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

    /// A one-argument operation on each element. A modulo form is an error.
    fn unary(&mut self, op: impl Fn(&Value, &Mode) -> Result<Value, CalcError>) {
        self.unary_leaf(false, op);
    }

    /// [`Self::unary`] for an operation that takes a modulo form.
    fn unary_mod(&mut self, op: impl Fn(&Value, &Mode) -> Result<Value, CalcError>) {
        self.unary_leaf(true, op);
    }

    fn unary_leaf(
        &mut self,
        allow_mod: bool,
        op: impl Fn(&Value, &Mode) -> Result<Value, CalcError>,
    ) {
        self.unary_whole(|v, mode| {
            vector::map1(v, &|x| {
                if !allow_mod && x.is_mod() {
                    Err(NOT_FOR_MOD)
                } else {
                    op(x, mode)
                }
            })
        });
    }

    /// Replace the top with `op` of it. `op` sees a vector whole.
    fn unary_whole(&mut self, op: impl Fn(&Value, &Mode) -> Result<Value, CalcError>) {
        if self.vector_mode == Some(VectorMode::Reduce) {
            self.fail(REDUCE_NEEDS_BINARY);
            return;
        }
        let result = if let Some(top) = self.stack.last() {
            op(top, &self.mode).and_then(value::in_range)
        } else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        match result {
            Ok(value) => {
                self.checkpoint();
                self.stack.pop();
                self.record(&value);
                self.stack.push(value);
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }

    /// A two-argument operation, element by element through vectors. A
    /// modulo form is an error.
    fn binop(&mut self, op: impl Fn(&Value, &Value, &Mode) -> Result<Value, CalcError>) {
        self.binop_leaf(false, op);
    }

    /// [`Self::binop`] for an operation that takes a modulo form.
    fn binop_mod(&mut self, op: impl Fn(&Value, &Value, &Mode) -> Result<Value, CalcError>) {
        self.binop_leaf(true, op);
    }

    fn binop_leaf(
        &mut self,
        allow_mod: bool,
        op: impl Fn(&Value, &Value, &Mode) -> Result<Value, CalcError>,
    ) {
        self.binop_whole(|a, b, mode| {
            vector::map2(a, b, &|x, y| {
                if !allow_mod && (x.is_mod() || y.is_mod()) {
                    Err(NOT_FOR_MOD)
                } else {
                    op(x, y, mode)
                }
            })
        });
    }

    /// Replace the top two with `op` of them, the deeper one first. `op`
    /// sees a vector whole. Under `V R` it folds over the top vector instead.
    fn binop_whole(&mut self, op: impl Fn(&Value, &Value, &Mode) -> Result<Value, CalcError>) {
        if self.vector_mode == Some(VectorMode::Reduce) {
            self.reduce(op);
            return;
        }
        let result = if self.stack.len() >= 2 {
            let n = self.stack.len();
            op(&self.stack[n - 2], &self.stack[n - 1], &self.mode).and_then(value::in_range)
        } else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        match result {
            Ok(value) => {
                self.checkpoint();
                self.stack.pop();
                self.stack.pop();
                self.record(&value);
                self.stack.push(value);
                self.succeed();
            }
            Err(err) => self.fail(err),
        }
    }

    /// `V R`. `[a, b, c]` becomes `op(op(a, b), c)`. A matrix, a vector of
    /// vectors of one length, folds over all its elements, as in Calc:
    /// `[[1, 2], [3, 4]] V R +` is `10`. An empty vector gives the identity
    /// of `+`, `-`, or `*`.
    fn reduce(&mut self, op: impl Fn(&Value, &Value, &Mode) -> Result<Value, CalcError>) {
        let Some(top) = self.stack.last() else {
            self.fail(CalcError::StackUnderflow);
            return;
        };
        let Value::Vector(items) = top else {
            self.fail(REDUCE_NEEDS_VECTOR);
            return;
        };
        let cells: Vec<&Value> = match matrix_cells(items) {
            Some(cells) => cells,
            None => items.iter().collect(),
        };
        let Some((first, rest)) = cells.split_first() else {
            match self.reduce_identity {
                Some(identity) => {
                    let value = Value::from_int(identity);
                    self.checkpoint();
                    self.stack.pop();
                    self.record(&value);
                    self.stack.push(value);
                    self.succeed();
                }
                None => self.fail(REDUCE_EMPTY),
            }
            return;
        };
        let mut acc = (*first).clone();
        for item in rest {
            match op(&acc, item, &self.mode).and_then(value::in_range) {
                Ok(next) => acc = next,
                Err(err) => {
                    self.fail(err);
                    return;
                }
            }
        }
        self.checkpoint();
        self.stack.pop();
        self.record(&acc);
        self.stack.push(acc);
        self.succeed();
    }
}

/// The elements of a matrix row by row: every element a vector of numbers,
/// all of one length. `None` for anything else.
fn matrix_cells(rows: &[Value]) -> Option<Vec<&Value>> {
    let mut width = None;
    let mut cells = Vec::new();
    for row in rows {
        let Value::Vector(row) = row else {
            return None;
        };
        if row.iter().any(|cell| matches!(cell, Value::Vector(_))) {
            return None;
        }
        if *width.get_or_insert(row.len()) != row.len() {
            return None;
        }
        cells.extend(row);
    }
    width.map(|_| cells)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_drops_the_oldest_past_its_byte_budget() {
        let mut engine = Engine::new();
        // About 1 MB of digits each.
        let big = Value::from_int(BigInt::from(1) << 8_000_000u32);
        let each = big.approx_bytes();
        for _ in 0..100 {
            engine.push(big.clone());
            engine.pop();
        }
        assert_eq!(engine.trail().len(), TRAIL_BYTES / each);
        assert!(engine.trail_bytes <= TRAIL_BYTES);

        for n in 0..TRAIL_CAP + 5 {
            engine.push(Value::from_int(n));
        }
        assert_eq!(engine.trail().len(), TRAIL_CAP);
        let first = &engine.trail()[0].value;
        assert_eq!(radix::format_value(first, engine.mode()), "5");
    }
}
