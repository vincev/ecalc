//! Key map. Tests drive this without a terminal.
//!
//! `keys` reads a string: `\n` is enter, `\t` is tab, `\u{7f}` is backspace.
//! An operator commits a finished entry first. `n` while typing toggles a
//! leading `_` and does not commit. Space inside an unparsed `(…` is inserted.

use crate::engine::Engine;
use crate::error::CalcError;
use crate::parse::parse_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Esc,
    Ctrl(char),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefix {
    None,
    F,
    M,
    C,
}

pub struct App {
    engine: Engine,
    entry: String,
    prefix: Prefix,
    /// `Some` while `p` is collecting a precision.
    prec_digits: Option<String>,
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
            prec_digits: None,
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
        self.engine.top().map(|value| value.to_string())
    }

    /// Text inside the prompt, without the `❯` prefix or the cursor.
    pub fn composer_text(&self) -> String {
        if let Some(digits) = &self.prec_digits {
            format!("precision: {digits}")
        } else {
            self.entry.clone()
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
        if self.prec_digits.is_some() {
            self.press_precision(key);
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
            Key::Ctrl('r') | Key::Ctrl('R') => self.engine.redo(),
            Key::Ctrl(_) => self.engine.show("unknown key"),
            Key::Esc => {}
        }
    }

    fn cancel(&mut self) {
        self.entry.clear();
        self.prefix = Prefix::None;
        self.prec_digits = None;
        self.help = false;
        self.engine.clear_flags();
        self.engine.clear_message();
    }

    fn press_precision(&mut self, key: Key) {
        match key {
            Key::Char(c) if c.is_ascii_digit() => {
                if let Some(digits) = self.prec_digits.as_mut() {
                    if digits.len() < 6 {
                        digits.push(c);
                    }
                }
            }
            Key::Backspace => {
                let empty = self
                    .prec_digits
                    .as_mut()
                    .is_none_or(|digits| digits.pop().is_none());
                if empty {
                    self.prec_digits = None;
                }
            }
            Key::Enter => {
                let digits = self.prec_digits.take().unwrap_or_default();
                match digits.parse::<u32>() {
                    Ok(prec) => self.engine.set_precision(prec),
                    Err(_) => self.engine.fail(CalcError::BadPrecision),
                }
            }
            _ => self.engine.show("type digits, then RET"),
        }
    }

    fn press_prefix(&mut self, key: Key) {
        let which = self.prefix;
        self.prefix = Prefix::None;
        let Key::Char(c) = key else {
            self.engine.show("unknown key");
            return;
        };
        if !self.commit_entry() {
            return;
        }
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
        if is_entry_continuation(c, &self.entry) {
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
            let parsed = parse_value(&self.entry, self.engine.mode().prec).is_ok();
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
        match parse_value(&self.entry, self.engine.mode().prec) {
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
                self.prec_digits = Some(String::new());
            }
            'm' => self.prefix = Prefix::M,
            'f' => self.prefix = Prefix::F,
            'c' => self.prefix = Prefix::C,
            'I' => self.engine.toggle_inverse(),
            'H' => self.engine.toggle_hyperbolic(),
            '?' => self.help = true,
            _ => self.engine.show("unknown key"),
        }
    }
}

fn is_entry_start(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | '_' | '(')
}

/// `+` and `-` stay in the entry only as an exponent sign, or inside `(…)`.
fn is_entry_continuation(c: char, entry: &str) -> bool {
    if matches!(c, '+' | '-') {
        return entry.ends_with('e') || entry.ends_with('E') || unclosed_paren(entry);
    }
    is_entry_start(c) || matches!(c, ')' | ',' | ':' | 'e' | 'E')
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

    impl App {
        fn run_len(keys: &str) -> usize {
            let mut app = App::new();
            app.keys(keys);
            app.engine().stack().len()
        }
    }
}
