//! Key map. Tests drive this without a terminal.
//!
//! `keys` reads a string: `\n` is enter, `\t` is tab, `\u{7f}` is backspace.
//! An operator commits a finished entry first. `n` while typing toggles a
//! leading `_` and does not commit. Space inside an unparsed `(…` is inserted.

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefix {
    None,
    F,
    M,
    C,
    /// `d` display prefix: radix, leading zeros.
    D,
    /// `b` binary prefix. Only `b w` (word size) is used.
    B,
    /// `k` combinatorics prefix: gcd, lcm, extended gcd, prime test.
    K,
    /// Calc's option prefix. `O d` sets a radix in two's complement.
    O,
    /// `O` followed by `d`.
    Od,
}

enum Pending {
    None,
    Precision(String),
    Radix { digits: String, twos: bool },
    Word(String),
}

pub struct App {
    engine: Engine,
    entry: String,
    prefix: Prefix,
    pending: Pending,
    help: bool,
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
            Prefix::O => parts.push("O-".to_string()),
            Prefix::Od => parts.push("Od-".to_string()),
            Prefix::None => {}
        }
        parts.join("  ")
    }

    /// Lines for the stack pane, oldest toward the top, `.` on the last row.
    pub fn stack_text(&self, rows: usize) -> String {
        if rows == 0 {
            return String::new();
        }
        let lines = self.engine.stack_lines();
        let room = rows - 1;
        let start = lines.len().saturating_sub(room);
        let mut out = Vec::with_capacity(rows);
        let pad = room.saturating_sub(lines.len() - start);
        out.resize(pad, String::new());
        out.extend(lines[start..].iter().cloned());
        out.push(".".to_string());
        out.join("\n")
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
        if matches!(key, Key::Char('q') | Key::Ctrl('c') | Key::Ctrl('C')) {
            self.quit = true;
            return;
        }
        if matches!(key, Key::Esc) {
            self.cancel();
            return;
        }
        if self.help {
            self.help = false;
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
            Key::Esc => {}
        }
    }

    fn cancel(&mut self) {
        self.entry.clear();
        self.prefix = Prefix::None;
        self.pending = Pending::None;
        self.help = false;
        self.engine.clear_flags();
        self.engine.clear_message();
    }

    fn press_pending(&mut self, key: Key) {
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
            Pending::None => {}
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
        if self.prefix == Prefix::O {
            if let Key::Char('d') = key {
                self.prefix = Prefix::Od;
                return;
            }
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
            (Prefix::B, 'w') => self.pending = Pending::Word(String::new()),
            (Prefix::K, 'g') => self.engine.gcd(),
            (Prefix::K, 'l') => self.engine.lcm(),
            (Prefix::K, 'E') => self.engine.egcd(),
            (Prefix::K, 'p') => self.engine.prime_test(),
            (Prefix::K, 'c') => self.engine.choose(),
            _ => self.engine.show("unknown key"),
        }
    }

    fn press_char(&mut self, c: char) {
        if c == 'u' {
            self.engine.undo();
            return;
        }
        if c == 'n' && !self.entry.is_empty() {
            self.toggle_entry_sign();
            return;
        }
        if c == ' ' {
            self.press_space();
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
        if is_entry_continuation(c, &self.entry, self.engine.mode().radix) {
            self.entry.push(c);
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
            'n' => self.engine.negate(),
            '&' => self.engine.reciprocal(),
            'Q' => self.engine.sqrt(),
            'A' => self.engine.abs(),
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
        assert_eq!(top("1.2e+2\n"), "120");
        assert_eq!(top("1.2e-1\n"), "0.12");
        assert_eq!(top("(3, -4)\n"), "(3, -4)");
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
        assert_eq!(top("1\n1fT"), "45");
        assert_eq!(top("mrP2/S"), "1");
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
        assert_eq!(top("1\n3/cF"), "1:3");
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

        assert_eq!(top("10\n3\nkc"), "120");
        assert_eq!(top("5\n2\nHkc"), "20");
        assert_eq!(top("_3\n2\nkc"), "6");
    }

    #[test]
    fn radix_entry_and_display() {
        assert_eq!(top("16#FF\n"), "255");
        assert_eq!(top("16#ff\n"), "255");
        assert_eq!(top("2#1010\n"), "10");
        assert_eq!(top("16#F.8\n"), "15.5");
        assert_eq!(top("16#A:3\n"), "10:3");
        assert_eq!(top("16#F*16^2\n"), "3840");
        assert_eq!(top("8#1e+2\n"), "64");
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
