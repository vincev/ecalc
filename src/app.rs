//! Key map. Tests drive this without a terminal.
//!
//! `keys` reads a string: `\n` is enter, `\t` is tab, `\u{7f}` is backspace.
//! An operator commits a finished entry first. `n` while typing toggles a
//! leading `_` and does not commit. Space inside an unparsed `(…` is inserted.

use crate::consts;
use crate::engine::Engine;
use crate::error::CalcError;
use crate::parse::parse_value;
use crate::radix;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    /// Shift-Tab. Terminals report this as BackTab.
    ShiftTab,
    Esc,
    Ctrl(char),
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefix {
    None,
    F,
    M,
    C,
    /// `d` display prefix: radix, leading zeros, digit grouping.
    D,
    /// `b` binary prefix. Only `b w` (word size) is used.
    B,
    /// `k` combinatorics prefix: gcd, lcm, extended gcd, modular inverse, prime test.
    K,
    /// `s` store and recall. `s s`, `s t`, `s r`, `s u`, `s l`, or a digit for `q0`–`q9`.
    S,
    /// `t` then a digit pops the stack into `q0`–`q9`.
    T,
    /// `r` then a digit recalls `q0`–`q9`.
    R,
    /// Calc's option prefix. `O d` sets a radix in two's complement.
    O,
    /// `O` followed by `d`.
    Od,
}

#[derive(Clone, Copy)]
enum VarOp {
    /// Copy or pop the top of the stack into a variable.
    Store {
        pop: bool,
    },
    Recall,
    Unstore,
}

enum Pending {
    None,
    Precision(String),
    Radix { digits: String, twos: bool },
    Word(String),
    Var { op: VarOp, name: String },
}

/// Rows down one column of the variables dialog. Two columns fill a page.
pub(crate) const VAR_COLUMN: usize = 5;
pub(crate) const VAR_PAGE: usize = VAR_COLUMN * 2;
/// Rows down one column of the constants dialog. Two columns fill a page.
pub(crate) use consts::COLUMN as CONST_COLUMN;

pub struct App {
    engine: Engine,
    entry: String,
    prefix: Prefix,
    pending: Pending,
    help: bool,
    /// `? ?` command pages. Typing filters them. `q` is a letter here.
    docs_open: bool,
    docs_query: String,
    docs_cursor: usize,
    /// `s l` dialog. Letters and digits recall; Tab moves a page.
    vars_open: bool,
    var_page: usize,
    var_note: String,
    /// `K` dialog. `0`–`F` inserts a constant; Tab moves a page.
    consts_open: bool,
    const_page: usize,
    const_note: String,
    quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            entry: String::new(),
            prefix: Prefix::None,
            pending: Pending::None,
            help: false,
            docs_open: false,
            docs_query: String::new(),
            docs_cursor: 0,
            vars_open: false,
            var_page: 0,
            var_note: String::new(),
            consts_open: false,
            const_page: 0,
            const_note: String::new(),
            quit: false,
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn help_open(&self) -> bool {
        self.help
    }

    pub fn docs_open(&self) -> bool {
        self.docs_open
    }

    pub fn docs_query(&self) -> &str {
        &self.docs_query
    }

    /// Index into [`Self::docs_matches`], clamped when the filter shrinks.
    pub fn docs_cursor(&self) -> usize {
        let n = self.docs_matches().len();
        if n == 0 {
            0
        } else {
            self.docs_cursor.min(n - 1)
        }
    }

    pub(crate) fn docs_matches(&self) -> Vec<&'static crate::docs::Entry> {
        crate::docs::search(&self.docs_query)
    }

    pub fn vars_open(&self) -> bool {
        self.vars_open
    }

    pub fn var_page(&self) -> usize {
        self.var_page
    }

    pub fn var_pages(&self) -> usize {
        self.engine.variable_count().div_ceil(VAR_PAGE).max(1)
    }

    pub fn var_note(&self) -> &str {
        &self.var_note
    }

    pub fn consts_open(&self) -> bool {
        self.consts_open
    }

    pub fn const_page(&self) -> usize {
        self.const_page
    }

    pub fn const_pages(&self) -> usize {
        consts::pages()
    }

    pub fn const_note(&self) -> &str {
        &self.const_note
    }

    /// Hex key, symbol, and short name on the current constants page.
    pub fn const_rows(&self) -> Vec<(char, &'static str, &'static str)> {
        consts::rows(self.const_page)
    }

    /// Names and values on the current page of the variables dialog.
    pub fn variable_rows(&self) -> Vec<(String, String)> {
        let start = self.var_page * VAR_PAGE;
        self.engine
            .variable_lines()
            .into_iter()
            .skip(start)
            .take(VAR_PAGE)
            .collect()
    }

    pub fn top_string(&self) -> Option<String> {
        self.engine
            .top()
            .map(|value| radix::format_value(value, self.engine.mode()))
    }

    /// Text inside the prompt, without the `❯` prefix or the cursor.
    pub fn composer_text(&self) -> String {
        match &self.pending {
            Pending::Precision(digits) => format!("precision: {digits}"),
            Pending::Radix { digits, .. } => format!("radix: {digits}"),
            Pending::Word(digits) => format!("word: {digits}"),
            Pending::Var { op, name } => format!("{}: {name}", var_prompt(*op)),
            Pending::None => self.entry.clone(),
        }
    }

    pub fn mode_line(&self) -> String {
        let angle = match self.engine.mode().angle {
            crate::value::AngleMode::Degrees => "deg",
            crate::value::AngleMode::Radians => "rad",
        };
        let mut parts = vec![
            format!("prec {}", self.engine.mode().prec),
            angle.to_string(),
        ];
        if self.engine.mode().fraction {
            parts.push("frac".to_string());
        }
        let mode = self.engine.mode();
        if mode.radix != 10 {
            parts.push(match mode.radix {
                2 => "bin".to_string(),
                8 => "oct".to_string(),
                16 => "hex".to_string(),
                other => format!("base {other}"),
            });
        }
        if mode.word_size != 32 || mode.leading_zeros || mode.twos_complement {
            parts.push(format!("w {}", mode.word_size));
        }
        if mode.leading_zeros {
            parts.push("zeros".to_string());
        }
        if mode.twos_complement {
            parts.push("twos".to_string());
        }
        if mode.group_digits {
            parts.push("group".to_string());
        }
        if self.engine.inverse() {
            parts.push("inv".to_string());
        }
        if self.engine.hyperbolic() {
            parts.push("hyp".to_string());
        }
        match self.prefix {
            Prefix::F => parts.push("f-".to_string()),
            Prefix::M => parts.push("m-".to_string()),
            Prefix::C => parts.push("c-".to_string()),
            Prefix::D => parts.push("d-".to_string()),
            Prefix::B => parts.push("b-".to_string()),
            Prefix::K => parts.push("k-".to_string()),
            Prefix::S => parts.push("s-".to_string()),
            Prefix::T => parts.push("t-".to_string()),
            Prefix::R => parts.push("r-".to_string()),
            Prefix::O => parts.push("O-".to_string()),
            Prefix::Od => parts.push("Od-".to_string()),
            Prefix::None => {}
        }
        parts.join("  ")
    }

    pub fn keys(&mut self, keys: &str) {
        for c in keys.chars() {
            match c {
                '\n' | '\r' => self.press(Key::Enter),
                '\t' => self.press(Key::Tab),
                '\u{7f}' | '\u{8}' => self.press(Key::Backspace),
                other => self.press(Key::Char(other)),
            }
        }
    }

    pub fn press(&mut self, key: Key) {
        if self.quit {
            return;
        }
        // `q` is a letter while a name is typed, and it cancels a numeric
        // prompt. List dialogs ignore it. A radix literal above 20 can use it
        // as a digit: `36#q` is 26. Ctrl-C still quits.
        let naming = matches!(self.pending, Pending::Var { .. });
        let numeric_pending = matches!(
            self.pending,
            Pending::Precision(_) | Pending::Radix { .. } | Pending::Word(_)
        );
        let radix_digit = !self.entry.is_empty()
            && is_entry_continuation('q', &self.entry, self.engine.mode().radix);
        let quit = match key {
            Key::Ctrl('c') | Key::Ctrl('C') => true,
            Key::Char('q') => {
                !radix_digit
                    && !naming
                    && !numeric_pending
                    && !self.vars_open
                    && !self.consts_open
                    && !self.docs_open
            }
            _ => false,
        };
        if quit {
            self.quit = true;
            return;
        }
        if numeric_pending && matches!(key, Key::Char('q')) {
            self.cancel();
            return;
        }
        if matches!(key, Key::Esc) {
            self.cancel();
            return;
        }
        if self.help {
            if matches!(key, Key::Char('?')) {
                self.open_docs();
            } else {
                self.help = false;
            }
            return;
        }
        if self.docs_open {
            self.press_docs(key);
            return;
        }
        if self.vars_open {
            self.press_var_list(key);
            return;
        }
        if self.consts_open {
            self.press_const_list(key);
            return;
        }
        if !matches!(self.pending, Pending::None) {
            self.press_pending(key);
            return;
        }
        if self.prefix != Prefix::None {
            self.press_prefix(key);
            return;
        }
        match key {
            Key::Char(c) => self.press_char(c),
            Key::Enter => self.press_enter(),
            Key::Backspace => self.press_backspace(),
            Key::Tab => {
                if self.commit_entry() {
                    self.engine.swap();
                }
            }
            Key::ShiftTab => {
                if self.commit_entry() {
                    self.engine.roll_up();
                }
            }
            Key::Ctrl('r') | Key::Ctrl('R') => self.engine.redo(),
            Key::Ctrl(_) => self.engine.show("unknown key"),
            Key::Up | Key::Down | Key::Esc => {}
        }
    }

    fn open_docs(&mut self) {
        self.help = false;
        self.docs_open = true;
        self.docs_query.clear();
        self.docs_cursor = 0;
    }

    fn press_docs(&mut self, key: Key) {
        match key {
            Key::Char('?') => {
                self.docs_open = false;
                self.docs_query.clear();
                self.docs_cursor = 0;
                self.help = true;
            }
            Key::Backspace => {
                self.docs_query.pop();
                self.docs_cursor = 0;
            }
            Key::Up | Key::ShiftTab => self.move_docs(-1),
            Key::Down | Key::Tab => self.move_docs(1),
            Key::Char(c) => {
                self.docs_query.push(c);
                self.docs_cursor = 0;
            }
            Key::Enter | Key::Ctrl(_) | Key::Esc => {}
        }
    }

    fn move_docs(&mut self, delta: isize) {
        let n = self.docs_matches().len();
        if n == 0 {
            return;
        }
        let cursor = self.docs_cursor.min(n - 1) as isize;
        self.docs_cursor = (cursor + delta).rem_euclid(n as isize) as usize;
    }

    fn cancel(&mut self) {
        self.entry.clear();
        self.prefix = Prefix::None;
        self.pending = Pending::None;
        self.help = false;
        self.docs_open = false;
        self.docs_query.clear();
        self.docs_cursor = 0;
        self.vars_open = false;
        self.var_page = 0;
        self.var_note.clear();
        self.consts_open = false;
        self.const_page = 0;
        self.const_note.clear();
        self.engine.clear_flags();
        self.engine.clear_message();
    }

    fn open_variables(&mut self) {
        self.vars_open = true;
        self.var_page = 0;
        self.var_note.clear();
        self.engine.clear_flags();
        self.engine.clear_message();
    }

    fn open_constants(&mut self) {
        self.consts_open = true;
        self.const_page = 0;
        self.const_note.clear();
        self.engine.clear_flags();
        self.engine.clear_message();
    }

    fn press_const_list(&mut self, key: Key) {
        match key {
            Key::Tab => {
                let pages = self.const_pages();
                turn_page(&mut self.const_page, &mut self.const_note, pages, true);
            }
            Key::ShiftTab => {
                let pages = self.const_pages();
                turn_page(&mut self.const_page, &mut self.const_note, pages, false);
            }
            Key::Char(c) => self.pick_constant(c),
            _ => {}
        }
    }

    /// `0`–`9` and `a`–`f` insert that slot on the current page.
    /// A slot past the end of the page stays on the list.
    fn pick_constant(&mut self, key: char) {
        let Some(slot) = hex_slot(key) else {
            return;
        };
        let index = self.const_page * consts::PAGE + slot;
        let Some((message, value)) = consts::get(index) else {
            self.const_note = "no constant".to_string();
            return;
        };
        self.consts_open = false;
        self.const_page = 0;
        self.const_note.clear();
        self.engine.push(value);
        self.engine.show(message);
    }

    fn press_var_list(&mut self, key: Key) {
        match key {
            Key::Tab => {
                let pages = self.var_pages();
                turn_page(&mut self.var_page, &mut self.var_note, pages, true);
            }
            Key::ShiftTab => {
                let pages = self.var_pages();
                turn_page(&mut self.var_page, &mut self.var_note, pages, false);
            }
            Key::Char(c) if c.is_ascii_digit() => self.pick_variable(&format!("q{c}")),
            Key::Char(c) if c.is_ascii_alphabetic() => self.pick_variable(&c.to_string()),
            _ => {}
        }
    }

    /// A letter recalls that one-letter name. A digit recalls `q0`–`q9`.
    /// A missing name stays on the list. Longer names are only shown.
    fn pick_variable(&mut self, name: &str) {
        if !self.engine.has_variable(name) {
            self.var_note = "void variable".to_string();
            return;
        }
        self.vars_open = false;
        self.var_page = 0;
        self.var_note.clear();
        self.engine.recall(name);
    }

    fn press_pending(&mut self, key: Key) {
        if matches!(self.pending, Pending::Var { .. }) {
            match key {
                Key::Char(c) => self.push_var_char(c),
                Key::Backspace => self.pop_pending(),
                Key::Enter => self.finish_pending(),
                _ => self.engine.show("type a name, then RET"),
            }
            return;
        }
        match key {
            Key::Char(c) => self.push_pending(c),
            Key::Backspace => self.pop_pending(),
            Key::Enter => self.finish_pending(),
            _ => self.engine.show("type digits, then RET"),
        }
    }

    fn pending_text(&mut self) -> Option<&mut String> {
        match &mut self.pending {
            Pending::Precision(digits) | Pending::Word(digits) | Pending::Radix { digits, .. } => {
                Some(digits)
            }
            Pending::Var { name, .. } => Some(name),
            Pending::None => None,
        }
    }

    fn push_pending(&mut self, c: char) {
        let word = matches!(self.pending, Pending::Word(_));
        let Some(digits) = self.pending_text() else {
            return;
        };
        if word && digits.is_empty() && matches!(c, '_' | '-' | '+') {
            digits.push(c);
            return;
        }
        if c.is_ascii_digit() && digits.len() < 6 {
            digits.push(c);
        }
    }

    fn pop_pending(&mut self) {
        let empty = self
            .pending_text()
            .is_none_or(|digits| digits.pop().is_none());
        if empty {
            self.pending = Pending::None;
        }
    }

    fn finish_pending(&mut self) {
        match std::mem::replace(&mut self.pending, Pending::None) {
            Pending::Precision(digits) => match digits.parse::<u32>() {
                Ok(prec) => self.engine.set_precision(prec),
                Err(_) => self.engine.fail(CalcError::BadPrecision),
            },
            Pending::Radix { digits, twos } => match digits.parse::<u32>() {
                Ok(radix) => self.engine.set_radix(radix, twos),
                Err(_) => self.engine.fail(CalcError::BadRadix),
            },
            Pending::Word(digits) => self.finish_word(&digits),
            Pending::Var { op, name } => self.apply_var(op, &name),
            Pending::None => {}
        }
    }

    fn push_var_char(&mut self, c: char) {
        let quick = matches!(
            &self.pending,
            Pending::Var { name, .. } if name.is_empty() && c.is_ascii_digit()
        );
        if quick {
            let Pending::Var { op, .. } = std::mem::replace(&mut self.pending, Pending::None)
            else {
                return;
            };
            self.apply_var(op, &format!("q{c}"));
            return;
        }
        let Some(name) = self.pending_text() else {
            return;
        };
        if crate::engine::extends_var(name, c) {
            name.push(c);
        }
    }

    fn apply_var(&mut self, op: VarOp, name: &str) {
        if name.is_empty() {
            self.engine.fail(CalcError::Domain("bad variable name"));
            return;
        }
        match op {
            VarOp::Store { pop } => self.engine.store(name, pop),
            VarOp::Recall => self.engine.recall(name),
            VarOp::Unstore => self.engine.unstore(name),
        }
    }

    fn finish_word(&mut self, text: &str) {
        if text.is_empty() {
            let size = self.engine.mode().word_size;
            self.engine.set_word_size(size);
            return;
        }
        let (negative, digits) =
            if let Some(rest) = text.strip_prefix('_').or_else(|| text.strip_prefix('-')) {
                (true, rest)
            } else if let Some(rest) = text.strip_prefix('+') {
                (false, rest)
            } else {
                (false, text)
            };
        if digits.is_empty() {
            self.engine.fail(CalcError::BadWordSize);
            return;
        }
        match digits.parse::<i64>() {
            Ok(size) => self
                .engine
                .set_word_size(if negative { -size } else { size }),
            Err(_) => self.engine.fail(CalcError::BadWordSize),
        }
    }

    fn press_prefix(&mut self, key: Key) {
        if self.prefix == Prefix::O
            && let Key::Char('d') = key
        {
            self.prefix = Prefix::Od;
            return;
        }
        let which = self.prefix;
        self.prefix = Prefix::None;
        let Key::Char(c) = key else {
            self.engine.show("unknown key");
            return;
        };
        if !self.commit_entry() {
            return;
        }
        let twos = which == Prefix::Od;
        match (which, c) {
            (Prefix::F, 'T') => self.engine.atan2(),
            (Prefix::F, 'h') => self.engine.hypot(),
            (Prefix::F, 'Q') => self.engine.isqrt(),
            (Prefix::F, 'A') => self.engine.abssqr(),
            (Prefix::F, 's') => self.engine.sign(),
            (Prefix::F, 'I') => self.engine.ilog(),
            (Prefix::F, 'E') => self.engine.expm1(),
            (Prefix::F, 'L') => self.engine.lnp1(),
            (Prefix::M, 'd') => self.engine.set_degrees(),
            (Prefix::M, 'r') => self.engine.set_radians(),
            (Prefix::M, 'f') => self.engine.toggle_fraction(),
            (Prefix::C, 'f') => self.engine.to_float(),
            (Prefix::C, 'F') => self.engine.to_fraction(),
            (Prefix::D | Prefix::Od, '2') => self.engine.set_radix(2, twos),
            (Prefix::D | Prefix::Od, '8') => self.engine.set_radix(8, twos),
            (Prefix::D | Prefix::Od, '6') => self.engine.set_radix(16, twos),
            (Prefix::D | Prefix::Od, '0') => self.engine.set_radix(10, twos),
            (Prefix::D | Prefix::Od, 'r') => {
                self.pending = Pending::Radix {
                    digits: String::new(),
                    twos,
                };
            }
            (Prefix::D | Prefix::Od, 'z') => self.engine.toggle_leading_zeros(),
            (Prefix::D | Prefix::Od, 'g') => self.engine.toggle_group_digits(),
            (Prefix::B, 'w') => self.pending = Pending::Word(String::new()),
            (Prefix::K, 'g') => self.engine.gcd(),
            (Prefix::K, 'l') => self.engine.lcm(),
            (Prefix::K, 'E') => self.engine.egcd(),
            (Prefix::K, 'i') => self.engine.mod_inverse(),
            (Prefix::K, 'p') => self.engine.prime_test(),
            (Prefix::K, 'c') => self.engine.choose(),
            (Prefix::S, 's') => {
                self.pending = Pending::Var {
                    op: VarOp::Store { pop: false },
                    name: String::new(),
                };
            }
            (Prefix::S, 't') => {
                self.pending = Pending::Var {
                    op: VarOp::Store { pop: true },
                    name: String::new(),
                };
            }
            (Prefix::S, 'r') => {
                self.pending = Pending::Var {
                    op: VarOp::Recall,
                    name: String::new(),
                };
            }
            (Prefix::S, 'u') => {
                self.pending = Pending::Var {
                    op: VarOp::Unstore,
                    name: String::new(),
                };
            }
            (Prefix::S, 'l') => self.open_variables(),
            (Prefix::S, d) if d.is_ascii_digit() => self.engine.store(&format!("q{d}"), false),
            (Prefix::T, d) if d.is_ascii_digit() => self.engine.store(&format!("q{d}"), true),
            (Prefix::R, d) if d.is_ascii_digit() => self.engine.recall(&format!("q{d}")),
            _ => self.engine.show("unknown key"),
        }
    }

    fn press_char(&mut self, c: char) {
        if c == ' ' {
            self.press_space();
            return;
        }
        // A letter can be a digit after `#` in a radix above 10. That check
        // comes first so `n` and `u` do not run their commands inside
        // `36#n` or `36#u`.
        if !self.entry.is_empty() && is_entry_continuation(c, &self.entry, self.engine.mode().radix)
        {
            self.entry.push(c);
            return;
        }
        if c == 'u' {
            self.engine.undo();
            return;
        }
        if c == 'n' && !self.entry.is_empty() {
            self.toggle_entry_sign();
            return;
        }
        if self.entry.is_empty() {
            if is_entry_start(c) {
                self.entry.push(c);
                return;
            }
            self.dispatch(c);
            return;
        }
        if self.commit_entry() {
            self.dispatch(c);
        }
    }

    fn press_space(&mut self) {
        if !self.entry.is_empty() {
            let open = self.entry.contains('(');
            let parsed = parse_value(&self.entry, self.engine.mode()).is_ok();
            if open && !parsed {
                self.entry.push(' ');
                return;
            }
            self.commit_entry();
            return;
        }
        self.engine.dup();
    }

    fn press_enter(&mut self) {
        if self.entry.is_empty() {
            self.engine.dup();
        } else {
            self.commit_entry();
        }
    }

    fn press_backspace(&mut self) {
        if self.entry.is_empty() {
            self.engine.pop();
        } else {
            self.entry.pop();
        }
    }

    fn toggle_entry_sign(&mut self) {
        if let Some(rest) = self
            .entry
            .strip_prefix('_')
            .or_else(|| self.entry.strip_prefix('-'))
        {
            self.entry = rest.to_string();
        } else if let Some(rest) = self.entry.strip_prefix('+') {
            self.entry = format!("_{rest}");
        } else {
            self.entry.insert(0, '_');
        }
    }

    fn commit_entry(&mut self) -> bool {
        if self.entry.is_empty() {
            return true;
        }
        match parse_value(&self.entry, self.engine.mode()) {
            Ok(value) => {
                self.engine.push(value);
                self.entry.clear();
                true
            }
            Err(err) => {
                self.engine.fail(err);
                false
            }
        }
    }

    fn dispatch(&mut self, c: char) {
        match c {
            '+' => self.engine.add(),
            '-' => self.engine.sub(),
            '*' => self.engine.mul(),
            '/' => self.engine.div(),
            '^' => self.engine.power(),
            '\\' => self.engine.idiv(),
            '%' => self.engine.modulo(),
            ':' => self.engine.fdiv(),
            '!' => self.engine.factorial(),
            'k' => self.prefix = Prefix::K,
            's' => self.prefix = Prefix::S,
            't' => self.prefix = Prefix::T,
            'r' => self.prefix = Prefix::R,
            'n' => self.engine.negate(),
            '&' => self.engine.reciprocal(),
            'Q' => self.engine.sqrt(),
            'A' => self.engine.abs(),
            'F' => self.engine.floor(),
            'S' => self.engine.sine(),
            'C' => self.engine.cosine(),
            'T' => self.engine.tangent(),
            'L' => {
                // Calc's ln key flips inverse, then uses the exp family.
                self.engine.toggle_inverse();
                self.engine.exp_family();
            }
            'E' | 'e' => self.engine.exp_family(),
            'B' => self.engine.logarithm(),
            'J' => self.engine.conj(),
            'G' => self.engine.arg(),
            'P' => self.engine.push_pi_or_e(),
            'K' => self.open_constants(),
            'p' => {
                self.engine.clear_flags();
                self.engine.clear_message();
                self.pending = Pending::Precision(String::new());
            }
            'm' => self.prefix = Prefix::M,
            'f' => self.prefix = Prefix::F,
            'c' => self.prefix = Prefix::C,
            'd' => self.prefix = Prefix::D,
            'b' => self.prefix = Prefix::B,
            'O' => self.prefix = Prefix::O,
            'I' => self.engine.toggle_inverse(),
            'H' => self.engine.toggle_hyperbolic(),
            '?' => self.help = true,
            _ => self.engine.show("unknown key"),
        }
    }
}

fn var_prompt(op: VarOp) -> &'static str {
    match op {
        VarOp::Store { pop: false } => "store",
        VarOp::Store { pop: true } => "into",
        VarOp::Recall => "recall",
        VarOp::Unstore => "unstore",
    }
}

/// Next or previous page. More than one page wraps. One page stays put.
fn turn_page(page: &mut usize, note: &mut String, pages: usize, forward: bool) {
    if pages <= 1 {
        return;
    }
    *page = if forward {
        (*page + 1) % pages
    } else {
        (*page + pages - 1) % pages
    };
    note.clear();
}

fn hex_slot(c: char) -> Option<usize> {
    match c {
        '0'..='9' => Some(usize::from(c as u8 - b'0')),
        'a'..='f' => Some(usize::from(c as u8 - b'a') + 10),
        'A'..='F' => Some(usize::from(c as u8 - b'A') + 10),
        _ => None,
    }
}

fn is_entry_start(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | '_' | '(' | '#')
}

/// `+` and `-` stay in the entry only as an exponent sign, or inside `(…)`.
/// After `#`, letters that are digits of that radix stay in the entry too.
fn is_entry_continuation(c: char, entry: &str, current_radix: u32) -> bool {
    if matches!(c, ')' | ',') && unclosed_paren(entry) {
        return true;
    }
    let token = current_token(entry);
    if radix_char_continues(token, c, current_radix) {
        return true;
    }
    if matches!(c, '+' | '-') && (ends_exponent(token) || unclosed_paren(entry)) {
        return true;
    }
    if token.contains('#') || c == '#' {
        return false;
    }
    c.is_ascii_digit() || matches!(c, '.' | '_' | '(' | ')' | ',' | ':' | 'e' | 'E')
}

fn ends_exponent(token: &str) -> bool {
    token.ends_with('e') || token.ends_with('E') || token.ends_with('^')
}

/// The number currently being typed, after the last `(',` or `,` of a complex.
fn current_token(entry: &str) -> &str {
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in entry.char_indices() {
        match c {
            '(' => {
                depth += 1;
                start = i + c.len_utf8();
            }
            ')' => depth -= 1,
            ',' if depth == 1 => start = i + c.len_utf8(),
            _ => {}
        }
    }
    entry[start..].trim()
}

fn radix_char_continues(token: &str, c: char, current_radix: u32) -> bool {
    let body = match token.as_bytes().first() {
        Some(b'_' | b'-' | b'+') => &token[1..],
        _ => token,
    };
    if !body.contains('#') {
        return c == '#' && body.bytes().all(|b| b.is_ascii_digit());
    }
    let Some((radix, after, twos)) = split_radix_prefix(body, current_radix) else {
        // An out-of-range radix is still being typed. Keep the characters
        // so RET can report the bad entry instead of running a command.
        return c.is_ascii_alphanumeric();
    };
    radix_body_continues(after, twos, radix, c)
}

fn split_radix_prefix(body: &str, current: u32) -> Option<(u32, &str, bool)> {
    let (radix_s, rest) = body.split_once('#')?;
    let radix = if radix_s.is_empty() {
        current
    } else {
        if !radix_s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let radix = radix_s.parse::<u32>().ok()?;
        if !(2..=36).contains(&radix) {
            return None;
        }
        radix
    };
    match rest.strip_prefix('#') {
        Some(after) => Some((radix, after, true)),
        None => Some((radix, rest, false)),
    }
}

fn radix_body_continues(after: &str, twos: bool, radix: u32, c: char) -> bool {
    if twos {
        return radix::is_digit(c, radix);
    }
    if let Some(at) = exponent_at(after, radix) {
        return exponent_continues(&after[at..], c, radix);
    }
    let has_dot = after.contains('.');
    let has_colon = after.contains(':');
    if c == '#' {
        return after.is_empty();
    }
    if radix::is_digit(c, radix) {
        return true;
    }
    if (c == '_' || c == '+' || c == '-') && after.ends_with(':') {
        return true;
    }
    if c == '.' && !has_dot && !has_colon {
        return true;
    }
    if c == ':' && !has_dot && !has_colon && after.chars().any(|ch| radix::is_digit(ch, radix)) {
        return true;
    }
    let started = after.chars().any(|ch| radix::is_digit(ch, radix));
    if (c == 'e' || c == 'E') && radix <= 14 && !has_colon && started {
        return true;
    }
    c == '*' && radix >= 15 && !has_colon && started
}

fn exponent_at(after: &str, radix: u32) -> Option<usize> {
    if radix <= 14 {
        after.find(['e', 'E'])
    } else {
        after.find('*')
    }
}

fn exponent_continues(exp: &str, c: char, radix: u32) -> bool {
    let rest = &exp[1..];
    if radix <= 14 {
        if rest.is_empty() || (rest.len() == 1 && is_exp_sign(rest.as_bytes()[0])) {
            return c.is_ascii_digit() || (rest.is_empty() && is_exp_sign(c as u8));
        }
        return c.is_ascii_digit();
    }
    if let Some(hat) = rest.find('^') {
        let after_hat = &rest[hat + 1..];
        if after_hat.is_empty() || (after_hat.len() == 1 && is_exp_sign(after_hat.as_bytes()[0])) {
            return c.is_ascii_digit() || (after_hat.is_empty() && is_exp_sign(c as u8));
        }
        return c.is_ascii_digit();
    }
    if rest.is_empty() {
        return c.is_ascii_digit();
    }
    rest.bytes().all(|b| b.is_ascii_digit()) && (c.is_ascii_digit() || c == '^')
}

fn is_exp_sign(c: u8) -> bool {
    matches!(c, b'+' | b'-' | b'_')
}

fn unclosed_paren(entry: &str) -> bool {
    let open = entry.bytes().filter(|b| *b == b'(').count();
    let close = entry.bytes().filter(|b| *b == b')').count();
    open > close
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(keys: &str) -> String {
        let mut app = App::new();
        app.keys(keys);
        app.top_string().unwrap_or_default()
    }

    #[test]
    fn tutorial_rpn_and_mixed_arithmetic() {
        assert_eq!(top("1\n2\n3\n4+*-"), "-13");
        assert_eq!(top("2\n4*7\n9.5*+5\n4/+"), "75.75");
        assert_eq!(top("0.1\n0.2\n+"), "0.3");
        assert_eq!(top("1.2e+2\n"), "120.");
        assert_eq!(top("1.2e-1\n"), "0.12");
        assert_eq!(top("(3, -4)\n"), "(3, -4)");
    }

    #[test]
    fn float_conversion_keeps_a_trailing_point() {
        // Emacs Calc shows a whole-number float with a trailing point.
        assert_eq!(top("6378137cf"), "6378137.");
        assert_eq!(top("0cf"), "0.");
        assert_eq!(top("_45cf"), "-45.");
        assert_eq!(top("1:2cf"), "0.5");
        assert_eq!(top("(3, 4)cf"), "(3., 4.)");
        assert_eq!(top("6378137cfcF"), "6378137");
        assert_eq!(top("6378137cfu"), "6378137");
        assert_eq!(top("255cfd6"), "16#FF.");
        assert_eq!(top("1e20cf"), "1e20");
        assert_eq!(top("5.\n"), "5.");
    }

    #[test]
    fn powers_roots_and_exact_integers() {
        assert_eq!(top("2\n10^"), "1024");
        assert_eq!(top("2\n100^"), "1267650600228229401496703205376");
        assert_eq!(top("125\n3I^"), "5");
        assert_eq!(top("9IQ"), "81");
        assert_eq!(top("10\n2IB"), "1024");
    }

    #[test]
    fn complex_and_square_roots() {
        assert_eq!(top("(3, 4)\nA"), "5");
        assert_eq!(top("(0, 1)\n(0, 1)*"), "-1");
        assert_eq!(top("(0, 1)\nIQ"), "-1");
        assert_eq!(top("1nQ"), "(0, 1)");
        assert_eq!(top("(3, 4)\nfA"), "25");
    }

    #[test]
    fn degrees_trig_and_logs() {
        assert_eq!(top("90S"), "1");
        assert_eq!(top("30S"), "0.5");
        assert_eq!(top("180C"), "-1");
        assert_eq!(top("45T"), "1");
        assert_eq!(top("1\nIT"), "45");
        // atan2(1, 1) is a float that lands on 45, so the point stays.
        assert_eq!(top("1\n1fT"), "45.");
        assert_eq!(top("mrP2/S"), "1.");
        assert_eq!(top("mrPC"), "-1.");
        assert_eq!(top("mr1\nC"), "0.540302305868");
        assert_eq!(top("1.5\nS"), "0.0261769483079");
        assert_eq!(top("12345678901.5\nS"), "-0.0261769483079");
        assert_eq!(top("_12345678901.5\nS"), "0.0261769483079");
        assert_eq!(top("_39.5\nS"), "-0.636078220278");
        assert_eq!(top("10000000001:2\nS"), "-0.636078220278");
        assert_eq!(top("80S"), "0.984807753012");
        assert_eq!(top("1e100001\nS"), "-0.984807753012");
        assert_eq!(top("mr1e24\nS"), "-0.996472229103");
        assert_eq!(top("mr1e100\nS"), "-0.372376123661");
        assert_eq!(top("mr1e100\nC"), "-0.928081905075");
        assert_eq!(top("mr1e100\nT"), "0.401231961991");
        // 1e13 mod 360 is 280, so this is cos(-80). Adding the 90-degree
        // shift at display precision used to round the angle to +100.
        assert_eq!(top("1e13C"), "0.173648177667");
        assert_eq!(top("1e14C"), "0.173648177667");
        // A decimal exponent is a float, not a huge exact rational.
        assert_eq!(top("2\n3.14159265359^"), "8.82497782708");
        assert_eq!(top("2\n1.2345^"), "2.3529978462");
        // Small results are values, not rounding specks of zero.
        assert_eq!(top("_100\nE"), "3.72007597602e-44");
        assert_eq!(top("1e-20\nfE"), "1e-20");
        assert_eq!(top("1e-20\nfL"), "1e-20");
        assert_eq!(top("125.0\n3I^"), "5");
        assert_eq!(top("mr(1e100, 1)\nS"), "(-0.574606385289, -1.09068296264)");
        assert_eq!(top("(0, 1)\nG"), "90");
        assert_eq!(top("0\n_1\nfT"), "180");
        assert_eq!(top("0\n0\nfT"), "0");
        assert_eq!(top("1024\n2B"), "10");
        assert_eq!(top("100\nHL"), "2");
        assert_eq!(top("2\nHE"), "100");
        assert_eq!(top("1\nL"), "0");
        assert_eq!(top("3\n4fh"), "5");
        assert_eq!(top("10\nfQ"), "3");
        assert_eq!(top("100\n10fI"), "2");
    }

    #[test]
    fn fractions_mod_and_idiv() {
        assert_eq!(top("mf8\n6/"), "4:3");
        assert_eq!(top("8\n6/"), "1.33333333333");
        assert_eq!(top("8\n6\n:"), "4:3");
        assert_eq!(top("3:4\n"), "3:4");
        assert_eq!(top("mf_8\n6/"), "-4:3");
        assert_eq!(top("10\n3%"), "1");
        assert_eq!(top("10\n3\\"), "3");
        assert_eq!(top("_5\n3\\"), "-2");
        assert_eq!(top("_5\n3%"), "1");
        assert_eq!(top("5\n_3\\"), "-2");
        assert_eq!(top("5\n_3%"), "-1");
        // Fractions and integers divide exactly before the floor.
        assert_eq!(top("_12:1\n_12:7\n\\"), "7");
        assert_eq!(top("_12:1\n1:7\n\\"), "-84");
        assert_eq!(top("_12:1\n1:7\n%"), "0");
        assert_eq!(top("12\n12:7\n\\"), "7");
        assert_eq!(top("_12\n12:7\n\\"), "-7");
        assert_eq!(top("1:2\n1:3\n\\"), "1");
        assert_eq!(top("_1:2\n1:3\n\\"), "-2");
        assert_eq!(top("1:10\n1:2\n\\"), "0");
        assert_eq!(top("_1:10\n1:2\n\\"), "-1");
        assert_eq!(top("_1:2\n1:3\n%"), "1:6");
        assert_eq!(top("12:7\n2:3\n%"), "8:21");
        assert_eq!(top("_12:7\n2:3\n%"), "2:7");
        // Quotients in [1, 2) still floor to 1. A one-bit gap does not
        // prove that the absolute value is below 1.
        assert_eq!(top("3:2\n4:3\n\\"), "1");
        assert_eq!(top("3:2\n4:3\n%"), "1:6");
        assert_eq!(top("_3:2\n4:3\n\\"), "-2");
        assert_eq!(top("_3:2\n4:3\n%"), "7:6");
        assert_eq!(top("_3:2\n_4:3\n\\"), "1");
        assert_eq!(top("_3:2\n_4:3\n%"), "-1:6");
        assert_eq!(top("3:2\n_4:3\n\\"), "-2");
        assert_eq!(top("3:2\n_4:3\n%"), "-7:6");
        assert_eq!(top("3\n8:3\n\\"), "1");
        assert_eq!(top("3\n8:3\n%"), "1:3");
        assert_eq!(top("7:4\n5:3\n\\"), "1");
        assert_eq!(top("7:4\n5:3\n%"), "1:12");
        // A float follows Calc: divide at the current precision, then floor.
        assert_eq!(top("1e20\n3\\"), "33333333333300000000");
        assert_eq!(top("1e20\n3%"), "100000000.");
        assert_eq!(top("1e20\n7\\"), "14285714285700000000");
        assert_eq!(top("1e20\n7%"), "100000000.");
        assert_eq!(top("0.7\n0.2\n%"), "0.1");
        // A quotient that rounds below 1 still floors to 0 or -1 without
        // materializing its enormous denominator.
        assert_eq!(top("1e-99999\n1\n\\"), "0");
        assert_eq!(top("_1e-99999\n1\n\\"), "-1");
        assert_eq!(top("1e-99999\n1\n%"), "1e-99999");
        assert_eq!(top("1\n3/cF"), "1:3");
    }

    #[test]
    fn floor_rounds_down() {
        assert_eq!(top("45.3F"), "45");
        assert_eq!(top("_45.3F"), "-46");
        assert_eq!(top("45F"), "45");
        assert_eq!(top("_45F"), "-45");
        assert_eq!(top("4:3F"), "1");
        assert_eq!(top("_4:3F"), "-2");
        assert_eq!(top("0.1F"), "0");
        assert_eq!(top("_0.1F"), "-1");
        assert_eq!(top("1.23e1F"), "12");
        assert_eq!(top("_1.23e1F"), "-13");

        let mut app = App::new();
        app.keys("45.3F");
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("45.3"));

        let mut app = App::new();
        app.keys("(1.5, _2.5)F");
        assert_eq!(app.top_string().as_deref(), Some("(1.5, -2.5)"));
        assert_eq!(app.engine().message(), "floor needs a real number");

        let mut app = App::new();
        app.keys("F");
        assert_eq!(app.engine().message(), "too few arguments");
    }

    #[test]
    fn undo_redo_swap_and_errors_keep_the_stack() {
        let mut app = App::new();
        app.keys("1\n2\n+");
        assert_eq!(app.top_string().as_deref(), Some("3"));
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("2"));
        assert_eq!(app.engine().stack().len(), 2);
        app.press(Key::Ctrl('r'));
        assert_eq!(app.top_string().as_deref(), Some("3"));

        app.keys("u3\n");
        app.press(Key::Ctrl('r'));
        assert_eq!(app.engine().stack().len(), 3);
        assert_eq!(app.top_string().as_deref(), Some("3"));
        assert_eq!(app.engine().message(), "nothing to redo");

        let mut app = App::new();
        app.keys("1\n2\n\t");
        assert_eq!(
            app.engine().stack_lines(),
            vec!["2".to_string(), "1".to_string()]
        );

        let mut app = App::new();
        app.keys("5\n+");
        assert_eq!(app.top_string().as_deref(), Some("5"));
        assert!(app.engine().is_error());
        assert!(!app.engine().inverse());

        app.keys("1\n0/");
        assert_eq!(app.engine().stack().len(), 3);
        assert_eq!(app.engine().message(), "division by zero");
    }

    #[test]
    fn shift_tab_rolls_the_top_three_up() {
        // Tutorial: 10 21 30, M-TAB, 1 +, M-TAB, M-TAB → 11 21 30.
        let mut app = App::new();
        app.keys("10\n21\n30");
        app.press(Key::ShiftTab);
        assert_eq!(app.engine().stack_lines(), ["21", "30", "10"]);
        app.keys("1\n+");
        app.press(Key::ShiftTab);
        app.press(Key::ShiftTab);
        assert_eq!(app.engine().stack_lines(), ["11", "21", "30"]);

        // Deeper stacks keep everything below the top three.
        let mut app = App::new();
        app.keys("10\n20\n30\n40\n50\n");
        app.press(Key::ShiftTab);
        assert_eq!(app.engine().stack_lines(), ["10", "20", "40", "50", "30"]);
        app.keys("u");
        assert_eq!(app.engine().stack_lines(), ["10", "20", "30", "40", "50"]);

        let mut app = App::new();
        app.keys("1\n2\n");
        app.press(Key::ShiftTab);
        assert!(app.engine().is_error());
        assert_eq!(app.engine().stack_lines(), ["1", "2"]);
    }

    #[test]
    fn precision_entry_sign_and_cancel() {
        let mut app = App::new();
        app.keys("p20\n");
        assert_eq!(app.engine().mode().prec, 20);
        assert!(!app.engine().is_error());
        app.keys("p2\n");
        assert_eq!(app.engine().mode().prec, 20);
        assert!(app.engine().is_error());

        app.keys("12");
        app.press(Key::Esc);
        assert!(app.engine().stack().is_empty());
        app.keys("\n");
        assert!(app.engine().stack().is_empty());

        app.keys("I");
        assert!(app.engine().inverse());
        app.press(Key::Esc);
        assert!(!app.engine().inverse());

        assert_eq!(top("5n\nfs"), "-1");
        assert_eq!(top("5\n "), "5");
        assert_eq!(App::run_len("5\n "), 2);
    }

    #[test]
    fn q_cancels_numeric_prompts() {
        let mut app = App::new();
        app.keys("42\np20");
        app.press(Key::Char('q'));
        assert!(!app.should_quit());
        assert_eq!(app.engine().mode().prec, 12);
        assert_eq!(app.top_string().as_deref(), Some("42"));
        app.keys("1\n+");
        assert_eq!(app.top_string().as_deref(), Some("43"));

        let mut app = App::new();
        app.keys("dr16");
        app.press(Key::Char('q'));
        assert!(!app.should_quit());
        assert_eq!(app.engine().mode().radix, 10);

        let mut app = App::new();
        app.keys("bw8");
        app.press(Key::Char('q'));
        assert!(!app.should_quit());
        assert_eq!(app.engine().mode().word_size, 32);

        let mut app = App::new();
        app.keys("5\nssq\n");
        assert!(!app.should_quit());
        assert_eq!(app.engine().message(), "stored q");

        let mut app = App::new();
        app.keys("42\n");
        app.press(Key::Char('q'));
        assert!(app.should_quit());
    }

    #[test]
    fn gcd_lcm_factorial_and_primes() {
        assert_eq!(top("12\n18\nkg"), "6");
        assert_eq!(top("_4\n6\nkl"), "12");
        assert_eq!(top("2:3\n4:5\nkg"), "2:15");
        assert_eq!(top("2:3\n4:5\nkl"), "4");
        assert_eq!(top("10!"), "3628800");
        assert_eq!(top("0!"), "1");
        assert_eq!(top("1:2!"), "0.886226925453");

        let mut app = App::new();
        app.keys("35\n15\nkE");
        assert_eq!(app.engine().stack_lines(), ["1", "-2", "5"]);
        app.keys("u");
        assert_eq!(app.engine().stack_lines(), ["35", "15"]);

        let mut app = App::new();
        app.keys("17\nkp");
        assert_eq!(app.engine().message(), "prime");
        assert!(!app.engine().is_error());
        assert_eq!(app.top_string().as_deref(), Some("17"));
        app.keys("u");
        assert!(app.engine().stack().is_empty());

        let mut app = App::new();
        app.keys("561\nkp");
        assert_eq!(app.engine().message(), "not prime");
        assert_eq!(app.top_string().as_deref(), Some("561"));

        let mut app = App::new();
        app.keys("1n!");
        assert!(app.engine().is_error());
        assert_eq!(app.top_string().as_deref(), Some("-1"));

        let mut app = App::new();
        app.keys("kf");
        assert_eq!(app.engine().message(), "unknown key");

        assert_eq!(top("3\n7\nki"), "5");
        assert_eq!(top("5\n24\nki"), "5");
        assert_eq!(top("_3\n7\nki"), "2");
        // Calc's tutorial: 21 divided by 5, modulo 24, is 9.
        assert_eq!(top("21\n5\n24\nki*24\n%"), "9");

        let mut app = App::new();
        app.keys("4\n6\nki");
        assert_eq!(app.engine().message(), "no modular inverse");
        assert!(app.engine().is_error());
        assert_eq!(app.engine().stack_lines(), ["4", "6"]);

        let mut app = App::new();
        app.keys("3\n7\nki");
        assert_eq!(app.top_string().as_deref(), Some("5"));
        app.keys("u");
        assert_eq!(app.engine().stack_lines(), ["3", "7"]);

        let mut app = App::new();
        app.keys("5\nki");
        assert_eq!(app.engine().message(), "too few arguments");
        assert_eq!(app.top_string().as_deref(), Some("5"));

        let mut app = App::new();
        app.keys("5\n_7\nki");
        assert_eq!(app.engine().message(), "modulus must be positive");
        assert_eq!(app.engine().stack_lines(), ["5", "-7"]);

        assert_eq!(top("10\n3\nkc"), "120");
        assert_eq!(top("5\n2\nHkc"), "20");
        assert_eq!(top("_3\n2\nkc"), "6");
    }

    #[test]
    fn variables_store_recall_and_undo() {
        let mut app = App::new();
        app.keys("12\nssa\n");
        assert_eq!(app.engine().message(), "stored a");
        assert_eq!(app.top_string().as_deref(), Some("12"));
        app.keys("sra\n");
        assert_eq!(app.engine().stack_lines(), ["12", "12"]);
        app.keys("u");
        assert_eq!(app.engine().stack_lines(), ["12"]);

        let mut app = App::new();
        app.keys("9\nsta\n");
        assert!(app.engine().stack().is_empty());
        assert_eq!(app.engine().message(), "stored a");
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("9"));
        app.keys("sra\n");
        assert!(app.engine().is_error());
        assert_eq!(app.engine().message(), "void variable");

        let mut app = App::new();
        app.keys("4\ns0r0");
        assert_eq!(app.engine().stack_lines(), ["4", "4"]);
        app.keys("t0");
        assert_eq!(app.engine().stack_lines(), ["4"]);
        app.keys("r0");
        assert_eq!(app.top_string().as_deref(), Some("4"));

        let mut app = App::new();
        app.keys("8\nss9");
        assert_eq!(app.engine().message(), "stored q9");
        assert_eq!(app.top_string().as_deref(), Some("8"));
        app.keys("sua\n");
        assert_eq!(app.engine().message(), "void variable");
        app.keys("ss\n");
        assert_eq!(app.engine().message(), "bad variable name");

        // A typed `q0` is the same slot as the `s 0` shorthand.
        let mut app = App::new();
        app.keys("7\nssq0\n");
        assert_eq!(app.composer_text(), "");
        assert_eq!(app.engine().message(), "stored q0");
        app.keys("r0");
        assert_eq!(app.engine().stack_lines(), ["7", "7"]);
        app.keys("suq0\n");
        assert_eq!(app.engine().message(), "unstored q0");
        app.keys("u");
        app.keys("r0");
        assert_eq!(app.top_string().as_deref(), Some("7"));

        let mut app = App::new();
        app.keys("5\nssa\n6\nssa\n");
        app.keys("u");
        app.keys("sra\n");
        assert_eq!(app.engine().stack_lines(), ["5", "6", "5"]);

        let mut app = App::new();
        app.keys("ssa\n");
        assert_eq!(app.engine().message(), "too few arguments");
        assert!(app.engine().stack().is_empty());
    }

    #[test]
    fn variable_list_pages_and_picks() {
        let mut app = App::new();
        for (i, name) in ('a'..='k').enumerate() {
            app.keys(&format!("{}\nss{}\n", i + 1, name));
        }
        app.keys("sl");
        assert!(app.vars_open());
        assert_eq!(app.var_page(), 0);
        assert_eq!(app.var_pages(), 2);
        app.press(Key::ShiftTab);
        assert_eq!(app.var_page(), 1);
        app.press(Key::Tab);
        assert_eq!(app.var_page(), 0);
        app.press(Key::Tab);
        assert_eq!(app.var_page(), 1);

        // `k` is on the other page. A letter still recalls it.
        app.press(Key::Char('k'));
        assert!(!app.vars_open());
        assert_eq!(app.engine().message(), "recalled k");
        assert_eq!(app.top_string().as_deref(), Some("11"));

        let mut app = App::new();
        app.keys("1\nssa\n2\nssab\nsl");
        app.press(Key::Char('a'));
        assert_eq!(app.top_string().as_deref(), Some("1"));
        assert_eq!(app.engine().message(), "recalled a");

        let mut app = App::new();
        app.keys("4\ns0sl");
        app.press(Key::Char('0'));
        assert!(!app.vars_open());
        assert_eq!(app.engine().stack_lines(), ["4", "4"]);

        let mut app = App::new();
        app.keys("sl");
        app.press(Key::Char('q'));
        assert!(!app.should_quit());
        assert!(app.vars_open());
        assert_eq!(app.var_note(), "void variable");
        app.press(Key::Esc);
        assert!(!app.vars_open());
        assert!(app.engine().stack().is_empty());
    }

    #[test]
    fn constants_list_pages_and_picks() {
        let mut app = App::new();
        app.keys("K");
        assert!(app.consts_open());
        assert_eq!(app.const_pages(), 3);
        app.press(Key::ShiftTab);
        assert_eq!(app.const_page(), 2);
        app.press(Key::Tab);
        assert_eq!(app.const_page(), 0);
        app.press(Key::Tab);
        app.press(Key::Tab);
        assert_eq!(app.const_page(), 2);
        app.press(Key::ShiftTab);
        assert_eq!(app.const_page(), 1);
        assert_eq!(app.const_note(), "");

        app.press(Key::Char('0'));
        assert!(!app.consts_open());
        assert_eq!(app.top_string().as_deref(), Some("8.854187817e-12"));
        assert_eq!(app.engine().message(), "eps0  F/m");
        app.keys("u");
        assert!(app.engine().stack().is_empty());

        assert_eq!(top("K0"), "299792458");
        let mut app = App::new();
        app.keys("K0");
        assert_eq!(app.engine().message(), "c  m/s");

        assert_eq!(top("K1"), "9.80665");
        assert_eq!(top("Ka"), "1.88353109e-28");
        assert_eq!(top("KF"), "5.291772083e-11");

        let mut app = App::new();
        app.keys("K");
        app.press(Key::Tab);
        app.keys("8");
        assert_eq!(app.top_string().as_deref(), Some("-9.28476362e-24"));
        assert_eq!(app.engine().message(), "mue  J/T");

        let mut app = App::new();
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("0");
        assert_eq!(app.top_string().as_deref(), Some("0.007297352533"));
        assert_eq!(app.engine().message(), "alpha");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("3");
        assert_eq!(app.top_string().as_deref(), Some("101325"));
        assert_eq!(app.engine().message(), "atm  Pa");

        let mut app = App::new();
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("8");
        assert_eq!(app.top_string().as_deref(), Some("5.9722e24"));
        assert_eq!(app.engine().message(), "Me  kg");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("9");
        assert_eq!(app.top_string().as_deref(), Some("6378137"));
        assert_eq!(app.engine().message(), "Re  m");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("A");
        assert_eq!(app.top_string().as_deref(), Some("7.346e22"));
        assert_eq!(app.engine().message(), "Mm  kg");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("B");
        assert_eq!(app.top_string().as_deref(), Some("1738100"));
        assert_eq!(app.engine().message(), "Rm  m");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("C");
        assert_eq!(app.top_string().as_deref(), Some("384400000"));
        assert_eq!(app.engine().message(), "dm  m");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("D");
        assert_eq!(app.top_string().as_deref(), Some("1.9884e30"));
        assert_eq!(app.engine().message(), "Ms  kg");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("E");
        assert_eq!(app.top_string().as_deref(), Some("695700000"));
        assert_eq!(app.engine().message(), "Rs  m");
        app.keys("K");
        app.press(Key::Tab);
        app.press(Key::Tab);
        app.keys("F");
        assert_eq!(app.top_string().as_deref(), Some("149597870700"));
        assert_eq!(app.engine().message(), "AU  m");

        let mut app = App::new();
        app.keys("K");
        app.press(Key::Char('q'));
        assert!(!app.should_quit());
        assert!(app.consts_open());
        app.press(Key::Esc);
        assert!(!app.consts_open());
        assert!(app.engine().stack().is_empty());
    }

    #[test]
    fn radix_entry_and_display() {
        assert_eq!(top("16#FF\n"), "255");
        assert_eq!(top("16#ff\n"), "255");
        assert_eq!(top("2#1010\n"), "10");
        // n, u, and q are ordinary digits in bases 24, 31, and 27.
        assert_eq!(top("36#n\n"), "23");
        assert_eq!(top("36#u\n"), "30");
        assert_eq!(top("36#q\n"), "26");
        assert_eq!(top("36#N\n"), "23");
        assert_eq!(top("36#U\n"), "30");
        assert_eq!(top("36#Q\n"), "26");
        let mut radix_q = App::new();
        radix_q.keys("36#q\n");
        assert!(!radix_q.should_quit());
        assert_eq!(radix_q.top_string().as_deref(), Some("26"));
        assert_eq!(top("16#F.8\n"), "15.5");
        assert_eq!(top("16#A:3\n"), "10:3");
        assert_eq!(top("16#F*16^2\n"), "3840.");
        assert_eq!(top("8#1e+2\n"), "64.");
        assert_eq!(top("(16#3, 16#4)\n"), "(3, 4)");
        assert_eq!(top("10n\n"), "-10");

        assert_eq!(top("255\nd6"), "16#FF");
        assert_eq!(top("255\nd8"), "8#377");
        assert_eq!(top("10\nd2"), "2#1010");
        assert_eq!(top("255\nd6d0"), "255");
        assert_eq!(top("255\ndr16\n"), "16#FF");
        assert_eq!(top("d6#ff\n"), "16#FF");
        assert_eq!(top("d6#A\n#5\n+"), "16#F");
        assert_eq!(top("2\n100^d6"), format!("16#1{}", "0".repeat(25)));

        let mut app = App::new();
        app.keys("dr1\n");
        assert_eq!(app.engine().mode().radix, 10);
        assert!(app.engine().is_error());
        app.keys("dr\n");
        assert!(app.engine().is_error());

        app.keys("d6");
        assert!(app.mode_line().contains("hex"));
        assert!(!app.mode_line().contains("twos"));
        app.keys("dz");
        assert!(app.mode_line().contains("zeros"));

        let mut app = App::new();
        app.keys("1\nbw8\nd2dz");
        assert_eq!(app.engine().mode().word_size, 8);
        assert_eq!(app.top_string().as_deref(), Some("2#00000001"));

        let mut app = App::new();
        app.keys("3n\nbw8\nOd6");
        assert_eq!(app.top_string().as_deref(), Some("16##FD"));
        assert!(app.mode_line().contains("twos"));
        assert!(app.mode_line().contains("w 8"));
        app.keys("d6");
        assert_eq!(app.top_string().as_deref(), Some("-16#3"));
        assert!(!app.engine().mode().twos_complement);

        let mut app = App::new();
        app.keys("200\nbw8\nOd6");
        assert_eq!(app.top_string().as_deref(), Some("16#C8"));
        app.keys("bw_8\n");
        assert_eq!(app.engine().mode().word_size, -8);
        assert_eq!(app.top_string().as_deref(), Some("16#C8"));

        let mut app = App::new();
        app.keys("p13\n1234567.123454\ndg");
        assert_eq!(app.top_string().as_deref(), Some("1,234,567.123454"));
        assert_eq!(app.engine().message(), "grouping");
        assert!(app.mode_line().contains("group"));
        app.keys("dg");
        assert_eq!(app.top_string().as_deref(), Some("1234567.123454"));
        assert_eq!(app.engine().message(), "no grouping");
        assert!(!app.engine().mode().group_digits);

        let mut app = App::new();
        app.keys("1000\n2000\n+dg");
        assert_eq!(app.top_string().as_deref(), Some("3,000"));
        app.keys("u");
        assert!(app.engine().mode().group_digits);
        assert_eq!(app.engine().stack_lines(), ["1,000", "2,000"]);

        let mut app = App::new();
        app.keys("(1234, 5678)\ndg");
        assert_eq!(app.top_string().as_deref(), Some("(1,234, 5,678)"));

        let mut app = App::new();
        app.keys("12345\nd2dg");
        assert_eq!(app.top_string().as_deref(), Some("2#11,0000,0011,1001"));
        app.keys("d8");
        assert_eq!(app.top_string().as_deref(), Some("8#30,071"));
        app.keys("d6");
        assert_eq!(app.top_string().as_deref(), Some("16#3039"));

        let mut app = App::new();
        app.keys("1\n2\n+d2");
        assert_eq!(app.top_string().as_deref(), Some("2#11"));
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("2#10"));

        let mut app = App::new();
        app.keys("bw\n");
        assert_eq!(app.engine().mode().word_size, 32);
        assert_eq!(app.engine().message(), "word size 32");
        app.keys("16##FD\n");
        assert_eq!(app.top_string().as_deref(), Some("253"));
    }

    impl App {
        fn run_len(keys: &str) -> usize {
            let mut app = App::new();
            app.keys(keys);
            app.engine().stack().len()
        }
    }
}
