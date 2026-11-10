//! Key map. Tests drive this without a terminal.
//!
//! `keys` reads a string: `\n` is enter, `\t` is tab, `\u{7f}` is backspace.
//! An operator commits a finished entry first. `n` while typing toggles a
//! leading `_` and does not commit. Space inside an unparsed `(…` is inserted.
//!
//! `eval` is the script form of those same keys. Whitespace separates words,
//! and the result is the top of the stack.

use crate::consts;
use crate::engine::{Engine, Status};
use crate::error::{CalcError, EvalError};
use crate::parse::Entry;
use crate::radix;
use crate::value::Value;

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
    /// `k` combinatorics prefix: gcd, lcm, extended gcd, modular inverse, prime test, prime factors.
    K,
    /// `s` store and recall. `s s`, `s t`, `s r`, `s u`, `s l`, or a digit for `q0`–`q9`.
    S,
    /// `t` then a digit pops the stack into `q0`–`q9`. `t y` opens the trail.
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
    Radix { digits: String, twos: bool },
    Word(String),
    Var { op: VarOp, name: String },
}

/// Rows down one column of the variables dialog. Two columns fill a page.
pub(crate) const VAR_COLUMN: usize = 5;
pub(crate) const VAR_PAGE: usize = VAR_COLUMN * 2;
/// Rows down one column of the constants dialog. Two columns fill a page.
pub(crate) use consts::COLUMN as CONST_COLUMN;

/// The one dialog above the prompt. Opening one closes the others.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    /// Shortcut list. Tab moves a page.
    Help { page: usize },
    /// `? ?` command pages. Typing filters them. `q` is a letter here.
    Docs { query: String, cursor: usize },
    /// `s l` dialog. Letters and digits recall; Tab moves a page.
    Variables { page: usize, note: String },
    /// `K` dialog. `0`–`F` inserts a constant; Tab moves a page.
    Constants { page: usize, note: String },
    /// `t y` dialog. Typing filters the trail; RET pushes the selected value.
    Trail(TrailView),
}

/// Longest value text the trail dialog keeps for display. A row is
/// narrower, so a longer value still ends in `..`.
const TRAIL_TEXT_MAX: usize = 512;

/// The trail as the dialog shows it. Each value is formatted once, when the
/// dialog opens; the display mode cannot change while it is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrailView {
    pub query: String,
    /// Index into `matches`, oldest first.
    pub cursor: usize,
    pub rows: Vec<TrailRow>,
    /// Indices into `rows` that match the query, oldest first.
    pub matches: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrailRow {
    pub label: String,
    /// The value in the display mode, cut at `TRAIL_TEXT_MAX` characters.
    pub text: String,
    /// The whole value without grouping, lowercased, for the search.
    search: String,
}

impl TrailView {
    /// Grouping commas are ignored, so `31415` and `31,415` both find
    /// `31,415,926.5` under `d g`. A comma inside a complex number still
    /// matches as typed.
    fn filter(&mut self) {
        let query = self.query.to_lowercase();
        let bare = query.replace(',', "");
        self.matches = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                query.is_empty()
                    || row.label.to_lowercase().contains(&query)
                    || row.search.contains(&query)
                    || (!bare.is_empty() && row.search.contains(&bare))
            })
            .map(|(index, _)| index)
            .collect();
        self.cursor = self.matches.len().saturating_sub(1);
    }
}

pub struct App {
    engine: Engine,
    entry: Entry,
    prefix: Prefix,
    pending: Pending,
    overlay: Option<Overlay>,
    /// Shortcut-list columns and rows that fit the current window. A page is
    /// rows times columns.
    help_cols: usize,
    help_rows: usize,
    /// One-shot inverse prefix. `L` flips it, then uses the exp/ln family.
    inverse: bool,
    /// One-shot hyperbolic prefix.
    hyperbolic: bool,
    /// Modulus of the last modulo form typed. `M M` fills it in, as in Calc.
    last_modulus: Option<String>,
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
            entry: Entry::new(),
            prefix: Prefix::None,
            pending: Pending::None,
            overlay: None,
            help_cols: 3,
            help_rows: crate::docs::HELP_ROWS,
            inverse: false,
            hyperbolic: false,
            last_modulus: None,
            quit: false,
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub(crate) fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    pub fn help_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Help { .. }))
    }

    /// Columns and rows the shortcut list can draw. The screen sets these from
    /// its size before each draw.
    pub(crate) fn set_help_layout(&mut self, cols: usize, rows: usize) {
        self.help_cols = cols.clamp(1, 3);
        self.help_rows = rows.clamp(1, crate::docs::HELP_ROWS);
    }

    pub fn help_page(&self) -> usize {
        let Some(Overlay::Help { page }) = &self.overlay else {
            return 0;
        };
        let pages = self.help_pages();
        (*page).min(pages.saturating_sub(1))
    }

    pub fn help_pages(&self) -> usize {
        crate::docs::help_entries()
            .len()
            .div_ceil(self.help_page_size())
            .max(1)
    }

    pub(crate) fn help_grid(&self) -> Vec<Vec<(&'static str, &'static str)>> {
        crate::docs::help_columns(self.help_page(), self.help_cols, self.help_rows)
    }

    fn help_page_size(&self) -> usize {
        self.help_rows.max(1) * self.help_cols.max(1)
    }

    pub fn docs_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Docs { .. }))
    }

    pub fn docs_query(&self) -> &str {
        match &self.overlay {
            Some(Overlay::Docs { query, .. }) => query,
            _ => "",
        }
    }

    /// Index into [`Self::docs_matches`], clamped when the filter shrinks.
    pub fn docs_cursor(&self) -> usize {
        let cursor = match &self.overlay {
            Some(Overlay::Docs { cursor, .. }) => *cursor,
            _ => 0,
        };
        let n = self.docs_matches().len();
        if n == 0 { 0 } else { cursor.min(n - 1) }
    }

    pub(crate) fn docs_matches(&self) -> Vec<&'static crate::docs::Binding> {
        crate::docs::search(self.docs_query())
    }

    pub fn trail_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Trail(_)))
    }

    pub(crate) fn trail_view(&self) -> Option<&TrailView> {
        match &self.overlay {
            Some(Overlay::Trail(view)) => Some(view),
            _ => None,
        }
    }

    pub fn trail_cursor(&self) -> usize {
        self.trail_view().map_or(0, |view| view.cursor)
    }

    /// Trail indices that match the query, oldest first.
    #[cfg(test)]
    pub(crate) fn trail_matches(&self) -> Vec<usize> {
        self.trail_view()
            .map(|view| view.matches.clone())
            .unwrap_or_default()
    }

    pub fn vars_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Variables { .. }))
    }

    pub fn var_page(&self) -> usize {
        match &self.overlay {
            Some(Overlay::Variables { page, .. }) => *page,
            _ => 0,
        }
    }

    pub fn var_pages(&self) -> usize {
        self.engine.variable_count().div_ceil(VAR_PAGE).max(1)
    }

    pub fn var_note(&self) -> &str {
        match &self.overlay {
            Some(Overlay::Variables { note, .. }) => note,
            _ => "",
        }
    }

    pub fn consts_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Constants { .. }))
    }

    pub fn const_page(&self) -> usize {
        match &self.overlay {
            Some(Overlay::Constants { page, .. }) => *page,
            _ => 0,
        }
    }

    pub fn const_pages(&self) -> usize {
        consts::pages()
    }

    pub fn const_note(&self) -> &str {
        match &self.overlay {
            Some(Overlay::Constants { note, .. }) => note,
            _ => "",
        }
    }

    /// Hex key, symbol, and short name on the current constants page.
    pub fn const_rows(&self) -> Vec<(char, &'static str, &'static str)> {
        consts::rows(self.const_page())
    }

    /// Names and values on the current page of the variables dialog.
    pub fn variable_rows(&self) -> Vec<(String, String)> {
        let start = self.var_page() * VAR_PAGE;
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
            Pending::Radix { digits, .. } => format!("radix: {digits}"),
            Pending::Word(digits) => format!("word: {digits}"),
            Pending::Var { op, name } => format!("{}: {name}", var_prompt(*op)),
            Pending::None => self.entry.as_str().to_string(),
        }
    }

    /// Mode chips on the prompt border. The screen joins them with ` · `.
    pub fn mode_line(&self) -> Vec<String> {
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
        if self.inverse {
            parts.push("inv".to_string());
        }
        if self.hyperbolic {
            parts.push("hyp".to_string());
        }
        let prefix = match self.prefix {
            Prefix::F => Some("f"),
            Prefix::M => Some("m"),
            Prefix::C => Some("c"),
            Prefix::D => Some("d"),
            Prefix::B => Some("b"),
            Prefix::K => Some("k"),
            Prefix::S => Some("s"),
            Prefix::T => Some("t"),
            Prefix::R => Some("r"),
            Prefix::O => Some("O"),
            Prefix::Od => Some("Od"),
            Prefix::None => None,
        };
        if let Some(keys) = prefix {
            parts.push(
                crate::docs::mode_label(keys)
                    .unwrap_or_else(|| panic!("no mode label for {keys}"))
                    .to_string(),
            );
        }
        parts
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
        // `q` is a letter while a name is typed, and it cancels a radix or
        // word-size prompt. List dialogs ignore it. A radix literal above 20
        // can use it as a digit: `36#q` is 26. Ctrl-C still quits.
        let naming = matches!(self.pending, Pending::Var { .. });
        let numeric_pending = matches!(self.pending, Pending::Radix { .. } | Pending::Word(_));
        let radix_digit = self.entry.accepts('q', self.engine.mode().radix);
        let quit = match key {
            Key::Ctrl('c') | Key::Ctrl('C') => true,
            Key::Char('q') => !radix_digit && !naming && !numeric_pending && !self.blocks_quit(),
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
        match self.overlay {
            Some(Overlay::Help { .. }) => {
                match key {
                    Key::Char('?') => self.open_docs(),
                    Key::Tab => self.turn_help_page(true),
                    Key::ShiftTab => self.turn_help_page(false),
                    _ => self.overlay = None,
                }
                return;
            }
            Some(Overlay::Docs { .. }) => {
                self.press_docs(key);
                return;
            }
            Some(Overlay::Variables { .. }) => {
                self.press_var_list(key);
                return;
            }
            Some(Overlay::Constants { .. }) => {
                self.press_const_list(key);
                return;
            }
            Some(Overlay::Trail(_)) => {
                self.press_trail(key);
                return;
            }
            None => {}
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
                    self.run(Engine::swap);
                }
            }
            Key::ShiftTab => {
                if self.commit_entry() {
                    self.run(Engine::roll_up);
                }
            }
            Key::Ctrl('r') | Key::Ctrl('R') => self.run(Engine::redo),
            Key::Ctrl(_) => self.run(Engine::unknown_key),
            Key::Up | Key::Down | Key::Esc => {}
        }
    }

    /// `q` quits from the shortcut list. It stays a letter in the command
    /// reference, and the variable and constant lists ignore it.
    fn blocks_quit(&self) -> bool {
        matches!(
            self.overlay,
            Some(
                Overlay::Docs { .. }
                    | Overlay::Variables { .. }
                    | Overlay::Constants { .. }
                    | Overlay::Trail(_)
            )
        )
    }

    fn open_trail(&mut self) {
        let mode = self.engine.mode();
        let mut plain = mode.clone();
        plain.group_digits = false;
        let rows = self
            .engine
            .trail()
            .iter()
            .map(|entry| {
                let search = radix::format_value(&entry.value, &plain).to_lowercase();
                let mut text = if mode.group_digits {
                    radix::format_value(&entry.value, mode)
                } else {
                    search.clone()
                };
                if let Some((cut, _)) = text.char_indices().nth(TRAIL_TEXT_MAX) {
                    text.truncate(cut);
                }
                TrailRow {
                    label: entry.label.clone(),
                    text,
                    search,
                }
            })
            .collect();
        let mut view = TrailView {
            query: String::new(),
            cursor: 0,
            rows,
            matches: Vec::new(),
        };
        view.filter();
        self.overlay = Some(Overlay::Trail(view));
        self.clear_ih();
        self.engine.clear_message();
    }

    fn press_trail(&mut self, key: Key) {
        let Some(Overlay::Trail(view)) = &mut self.overlay else {
            return;
        };
        match key {
            Key::Enter => self.yank_trail(),
            Key::Backspace => {
                if view.query.pop().is_some() {
                    view.filter();
                }
            }
            Key::Up | Key::ShiftTab => move_cursor(view, -1),
            Key::Down | Key::Tab => move_cursor(view, 1),
            Key::Char(c) => {
                view.query.push(c);
                view.filter();
            }
            Key::Ctrl(_) | Key::Esc => {}
        }
    }

    /// Push the selected value and close. With no match the dialog stays.
    /// A row is the trail entry of the same index: nothing is recorded
    /// while the dialog is open.
    fn yank_trail(&mut self) {
        let Some(view) = self.trail_view() else {
            return;
        };
        let Some(&index) = view.matches.get(view.cursor) else {
            return;
        };
        self.overlay = None;
        self.run(|engine| engine.yank(index));
    }

    fn open_docs(&mut self) {
        self.overlay = Some(Overlay::Docs {
            query: String::new(),
            cursor: 0,
        });
    }

    fn press_docs(&mut self, key: Key) {
        match key {
            Key::Char('?') => self.overlay = Some(Overlay::Help { page: 0 }),
            Key::Backspace => self.edit_docs_query(|query| {
                query.pop();
            }),
            Key::Up | Key::ShiftTab => self.move_docs(-1),
            Key::Down | Key::Tab => self.move_docs(1),
            Key::Char(c) => self.edit_docs_query(|query| query.push(c)),
            Key::Enter | Key::Ctrl(_) | Key::Esc => {}
        }
    }

    fn edit_docs_query(&mut self, edit: impl FnOnce(&mut String)) {
        if let Some(Overlay::Docs { query, cursor }) = &mut self.overlay {
            edit(query);
            *cursor = 0;
        }
    }

    fn move_docs(&mut self, delta: isize) {
        let n = self.docs_matches().len();
        if n == 0 {
            return;
        }
        let Some(Overlay::Docs { cursor, .. }) = &mut self.overlay else {
            return;
        };
        let current = (*cursor).min(n - 1) as isize;
        *cursor = (current + delta).rem_euclid(n as isize) as usize;
    }

    fn cancel(&mut self) {
        self.entry.clear();
        self.prefix = Prefix::None;
        self.pending = Pending::None;
        self.overlay = None;
        self.clear_ih();
        self.engine.clear_message();
    }

    fn clear_ih(&mut self) {
        self.inverse = false;
        self.hyperbolic = false;
    }

    /// An operation consumes `I` and `H`. `push` and a successful `dup` do not.
    fn run(&mut self, op: impl FnOnce(&mut Engine)) {
        op(&mut self.engine);
        self.clear_ih();
    }

    fn duplicate(&mut self) {
        self.engine.dup();
        if self.engine.is_error() {
            self.clear_ih();
        }
    }

    fn open_variables(&mut self) {
        self.overlay = Some(Overlay::Variables {
            page: 0,
            note: String::new(),
        });
        self.clear_ih();
        self.engine.clear_message();
    }

    fn open_constants(&mut self) {
        self.overlay = Some(Overlay::Constants {
            page: 0,
            note: String::new(),
        });
        self.clear_ih();
        self.engine.clear_message();
    }

    fn press_const_list(&mut self, key: Key) {
        match key {
            Key::Tab => self.turn_const_page(true),
            Key::ShiftTab => self.turn_const_page(false),
            Key::Char(c) => self.pick_constant(c),
            _ => {}
        }
    }

    fn turn_help_page(&mut self, forward: bool) {
        let pages = self.help_pages();
        if pages <= 1 {
            return;
        }
        let Some(Overlay::Help { page }) = &mut self.overlay else {
            return;
        };
        let current = (*page).min(pages - 1);
        *page = if forward {
            (current + 1) % pages
        } else {
            (current + pages - 1) % pages
        };
    }

    fn turn_const_page(&mut self, forward: bool) {
        let pages = self.const_pages();
        if let Some(Overlay::Constants { page, note }) = &mut self.overlay {
            turn_page(page, note, pages, forward);
        }
    }

    /// `0`–`9` and `a`–`f` insert that slot on the current page.
    /// A slot past the end of the page stays on the list.
    fn pick_constant(&mut self, key: char) {
        let Some(slot) = hex_slot(key) else {
            return;
        };
        let index = self.const_page() * consts::PAGE + slot;
        let Some((message, symbol, value)) = consts::get(index) else {
            if let Some(Overlay::Constants { note, .. }) = &mut self.overlay {
                *note = "no constant".to_string();
            }
            return;
        };
        self.overlay = None;
        self.engine.set_label(format!("K {symbol}"));
        self.engine.push(value);
        self.run(|engine| engine.show(message));
    }

    fn press_var_list(&mut self, key: Key) {
        match key {
            Key::Tab => self.turn_var_page(true),
            Key::ShiftTab => self.turn_var_page(false),
            Key::Char(c) if c.is_ascii_digit() => self.pick_variable(&format!("q{c}")),
            Key::Char(c) if c.is_ascii_alphabetic() => self.pick_variable(&c.to_string()),
            _ => {}
        }
    }

    fn turn_var_page(&mut self, forward: bool) {
        let pages = self.var_pages();
        if let Some(Overlay::Variables { page, note }) = &mut self.overlay {
            turn_page(page, note, pages, forward);
        }
    }

    /// A letter recalls that one-letter name. A digit recalls `q0`–`q9`.
    /// A missing name stays on the list. Longer names are only shown.
    fn pick_variable(&mut self, name: &str) {
        if !self.engine.has_variable(name) {
            if let Some(Overlay::Variables { note, .. }) = &mut self.overlay {
                *note = "void variable".to_string();
            }
            return;
        }
        self.overlay = None;
        self.run(|engine| engine.recall(name));
    }

    fn press_pending(&mut self, key: Key) {
        if matches!(self.pending, Pending::Var { .. }) {
            match key {
                Key::Char(c) => self.push_var_char(c),
                Key::Backspace => self.pop_pending(),
                Key::Enter => self.finish_pending(),
                _ => self.run(|engine| engine.show("type a name, then RET")),
            }
            return;
        }
        match key {
            Key::Char(c) => self.push_pending(c),
            Key::Backspace => self.pop_pending(),
            Key::Enter => self.finish_pending(),
            _ => self.run(|engine| engine.show("type digits, then RET")),
        }
    }

    fn pending_text(&mut self) -> Option<&mut String> {
        match &mut self.pending {
            Pending::Word(digits) | Pending::Radix { digits, .. } => Some(digits),
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
            Pending::Radix { digits, twos } => match digits.parse::<u32>() {
                Ok(radix) => self.run(|engine| engine.set_radix(radix, twos)),
                Err(_) => self.run(|engine| engine.fail(CalcError::BadRadix)),
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
            self.run(|engine| engine.fail(CalcError::Domain("bad variable name")));
            return;
        }
        match op {
            VarOp::Store { pop } => self.run(|engine| engine.store(name, pop)),
            VarOp::Recall => self.run(|engine| engine.recall(name)),
            VarOp::Unstore => self.run(|engine| engine.unstore(name)),
        }
    }

    fn finish_word(&mut self, text: &str) {
        if text.is_empty() {
            let size = self.engine.mode().word_size;
            self.run(|engine| engine.set_word_size(size));
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
            self.run(|engine| engine.fail(CalcError::BadWordSize));
            return;
        }
        match digits.parse::<i64>() {
            Ok(size) => {
                let size = if negative { -size } else { size };
                self.run(|engine| engine.set_word_size(size));
            }
            Err(_) => self.run(|engine| engine.fail(CalcError::BadWordSize)),
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
            self.run(Engine::unknown_key);
            return;
        };
        if !self.commit_entry() {
            return;
        }
        self.label_command(prefix_keys(which), c);
        let twos = which == Prefix::Od;
        match (which, c) {
            (Prefix::F, 'T') => self.run(Engine::atan2),
            (Prefix::F, 'h') => self.run(Engine::hypot),
            (Prefix::F, 'Q') => self.run(Engine::isqrt),
            (Prefix::F, 'A') => self.run(Engine::abssqr),
            (Prefix::F, 's') => self.run(Engine::sign),
            (Prefix::F, 'I') => self.run(Engine::ilog),
            (Prefix::F, 'E') => self.run(Engine::expm1),
            (Prefix::F, 'L') => self.run(Engine::lnp1),
            (Prefix::M, 'd') => self.run(Engine::set_degrees),
            (Prefix::M, 'r') => self.run(Engine::set_radians),
            (Prefix::M, 'f') => self.run(Engine::toggle_fraction),
            (Prefix::C, 'f') => self.run(Engine::to_float),
            (Prefix::C, 'F') => self.run(Engine::to_fraction),
            (Prefix::D | Prefix::Od, '2') => self.run(|engine| engine.set_radix(2, twos)),
            (Prefix::D | Prefix::Od, '8') => self.run(|engine| engine.set_radix(8, twos)),
            (Prefix::D | Prefix::Od, '6') => self.run(|engine| engine.set_radix(16, twos)),
            (Prefix::D | Prefix::Od, '0') => self.run(|engine| engine.set_radix(10, twos)),
            (Prefix::D | Prefix::Od, 'r') => {
                self.pending = Pending::Radix {
                    digits: String::new(),
                    twos,
                };
            }
            (Prefix::D | Prefix::Od, 'z') => self.run(Engine::toggle_leading_zeros),
            (Prefix::D | Prefix::Od, 'g') => self.run(Engine::toggle_group_digits),
            (Prefix::B, 'w') => self.pending = Pending::Word(String::new()),
            (Prefix::K, 'g') => self.run(Engine::gcd),
            (Prefix::K, 'l') => self.run(Engine::lcm),
            (Prefix::K, 'E') => self.run(Engine::egcd),
            (Prefix::K, 'i') => self.run(Engine::mod_inverse),
            (Prefix::K, 'p') => self.run(Engine::prime_test),
            (Prefix::K, 'f') => self.run(Engine::prime_factors),
            (Prefix::K, 't') => self.run(Engine::totient),
            (Prefix::K, 'n') => {
                if self.hyperbolic {
                    self.run(Engine::nth_prime);
                } else if self.inverse {
                    self.run(Engine::prev_prime);
                } else {
                    self.run(Engine::next_prime);
                }
            }
            (Prefix::K, 'c') => {
                if self.hyperbolic {
                    self.run(Engine::perm);
                } else {
                    self.run(Engine::choose);
                }
            }
            (Prefix::K, 's') => {
                if self.hyperbolic {
                    self.run(Engine::stirling_second);
                } else {
                    self.run(Engine::stirling_first);
                }
            }
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
            (Prefix::S, d) if d.is_ascii_digit() => {
                self.run(|engine| engine.store(&format!("q{d}"), false));
            }
            (Prefix::T, 'y') => self.open_trail(),
            (Prefix::T, d) if d.is_ascii_digit() => {
                self.run(|engine| engine.store(&format!("q{d}"), true));
            }
            (Prefix::R, d) if d.is_ascii_digit() => {
                self.run(|engine| engine.recall(&format!("q{d}")));
            }
            _ => self.run(Engine::unknown_key),
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
        if self.entry.push(c, self.engine.mode().radix) {
            return;
        }
        if c == 'M' && self.press_mod() {
            return;
        }
        if c == 'u' {
            self.run(Engine::undo);
            return;
        }
        if c == 'n' && !self.entry.is_empty() {
            self.entry.toggle_sign();
            return;
        }
        if self.entry.is_empty() {
            self.dispatch(c);
            return;
        }
        if self.commit_entry() {
            self.dispatch(c);
        }
    }

    /// `M` while typing an integer starts its modulus. A second `M` fills in
    /// the previous modulus. Anything else falls through to the command keys.
    fn press_mod(&mut self) -> bool {
        if self.entry.awaits_modulus() {
            match self.last_modulus.clone() {
                Some(modulus) => self.entry.fill_modulus(&modulus),
                None => self.run(|engine| {
                    engine.fail(CalcError::Domain("no previous modulus"));
                }),
            }
            return true;
        }
        if self.entry.start_mod(self.engine.mode()) {
            return true;
        }
        if self.entry.is_empty() {
            self.run(|engine| engine.fail(CalcError::Domain("type an integer before M")));
            return true;
        }
        // `3.5 M` keeps the entry and says why, instead of committing 3.5
        // and reporting an unknown key. A finished form falls through.
        if !self.entry.as_str().contains(" mod ") {
            self.run(|engine| engine.fail(CalcError::Domain("modulo forms need integers")));
            return true;
        }
        false
    }

    fn press_space(&mut self) {
        if self.entry.push_space(self.engine.mode()) {
            return;
        }
        if !self.entry.is_empty() {
            self.commit_entry();
            return;
        }
        self.duplicate();
    }

    fn press_enter(&mut self) {
        if self.entry.is_empty() {
            self.duplicate();
        } else {
            self.commit_entry();
        }
    }

    fn press_backspace(&mut self) {
        if self.entry.is_empty() {
            self.run(Engine::pop);
        } else {
            self.entry.pop();
        }
    }

    fn commit_entry(&mut self) -> bool {
        if self.entry.is_empty() {
            return true;
        }
        match self.entry.parse(self.engine.mode()) {
            Ok(value) => {
                if let Value::Mod(_, modulus) = &value {
                    self.last_modulus = Some(modulus.to_string());
                }
                self.engine.set_label("");
                self.engine.push(value);
                self.entry.clear();
                true
            }
            Err(err) => {
                self.run(|engine| engine.fail(err));
                false
            }
        }
    }

    /// Trail label for the command now running: `H`, then `I`, then the
    /// prefix and the key. The flags are in that fixed order, whichever was
    /// typed first, and show even when the key ignores them.
    fn label_command(&mut self, prefix: &str, c: char) {
        let mut parts = Vec::new();
        if self.hyperbolic {
            parts.push("H".to_string());
        }
        if self.inverse {
            parts.push("I".to_string());
        }
        if !prefix.is_empty() {
            parts.push(prefix.to_string());
        }
        parts.push(c.to_string());
        self.engine.set_label(parts.join(" "));
    }

    fn dispatch(&mut self, c: char) {
        self.label_command("", c);
        match c {
            '+' => self.run(Engine::add),
            '-' => self.run(Engine::sub),
            '*' => self.run(Engine::mul),
            '/' => self.run(Engine::div),
            '^' => {
                if self.inverse {
                    self.run(Engine::nth_root);
                } else {
                    self.run(Engine::power);
                }
            }
            '\\' => self.run(Engine::idiv),
            '%' => self.run(Engine::modulo),
            ':' => self.run(Engine::div_fraction),
            '!' => self.run(Engine::factorial),
            'k' => self.prefix = Prefix::K,
            's' => self.prefix = Prefix::S,
            't' => self.prefix = Prefix::T,
            'r' => self.prefix = Prefix::R,
            'n' => self.run(Engine::negate),
            '&' => self.run(Engine::reciprocal),
            'Q' => {
                if self.inverse {
                    self.run(Engine::square);
                } else {
                    self.run(Engine::sqrt);
                }
            }
            'A' => self.run(Engine::abs),
            'F' => self.run(Engine::floor),
            'S' => self.trig(Engine::sin, Engine::asin, Engine::sinh, Engine::asinh),
            'C' => self.trig(Engine::cos, Engine::acos, Engine::cosh, Engine::acosh),
            'T' => self.trig(Engine::tan, Engine::atan, Engine::tanh, Engine::atanh),
            'L' => {
                // Calc's ln key flips inverse, then uses the exp family.
                self.inverse = !self.inverse;
                self.exp_family();
            }
            'E' | 'e' => self.exp_family(),
            'B' => {
                if self.inverse {
                    self.run(Engine::antilog);
                } else {
                    self.run(Engine::log);
                }
            }
            'J' => self.run(Engine::conj),
            'G' => self.run(Engine::arg),
            'P' => {
                if self.hyperbolic {
                    self.run(Engine::push_e);
                } else {
                    self.run(Engine::push_pi);
                }
            }
            'K' => self.open_constants(),
            'p' => self.run(Engine::take_precision),
            'm' => self.prefix = Prefix::M,
            'f' => self.prefix = Prefix::F,
            'c' => self.prefix = Prefix::C,
            'd' => self.prefix = Prefix::D,
            'b' => self.prefix = Prefix::B,
            'O' => self.prefix = Prefix::O,
            'I' => {
                self.inverse = !self.inverse;
                self.engine.clear_message();
            }
            'H' => {
                self.hyperbolic = !self.hyperbolic;
                self.engine.clear_message();
            }
            '?' => self.overlay = Some(Overlay::Help { page: 0 }),
            _ => self.run(Engine::unknown_key),
        }
    }

    fn trig(
        &mut self,
        plain: fn(&mut Engine),
        inverse: fn(&mut Engine),
        hyper: fn(&mut Engine),
        both: fn(&mut Engine),
    ) {
        let op = match (self.inverse, self.hyperbolic) {
            (false, false) => plain,
            (true, false) => inverse,
            (false, true) => hyper,
            (true, true) => both,
        };
        self.run(op);
    }

    fn exp_family(&mut self) {
        let op = match (self.inverse, self.hyperbolic) {
            (false, false) => Engine::exp,
            (true, false) => Engine::ln,
            (false, true) => Engine::exp10,
            (true, true) => Engine::log10,
        };
        self.run(op);
    }

    /// Run one script word. Stop at the first bad key so a later key cannot
    /// clear the message.
    fn run_word(&mut self, word: &str) -> Result<(), EvalError> {
        for c in word.chars() {
            self.press(Key::Char(c));
            self.script_boundary()?;
        }
        self.settle_word(false)
    }

    /// Push a finished number so the next word is a command. Keep an unclosed
    /// `(…` open; the next word continues it. At the end of the script, finish
    /// a prompt that never got its answer.
    fn settle_word(&mut self, end: bool) -> Result<(), EvalError> {
        if self.pending_answer_ready() {
            self.finish_pending();
            self.script_boundary()?;
        }
        if !end && self.entry.is_incomplete() {
            return Ok(());
        }
        if !self.entry.is_empty() && !self.commit_entry() {
            return Err(self.eval_status());
        }
        if end && let Some(message) = self.incomplete_prompt() {
            return Err(EvalError::Prompt(message));
        }
        Ok(())
    }

    /// Note text when a variable or constant list is still open.
    fn open_list_note(&self) -> Option<String> {
        match &self.overlay {
            Some(Overlay::Constants { note, .. } | Overlay::Variables { note, .. }) => {
                Some(note.clone())
            }
            _ => None,
        }
    }

    fn finish_script(&mut self) -> Result<(), EvalError> {
        self.settle_word(true)?;
        if let Some(note) = self.open_list_note() {
            return Err(if note.is_empty() {
                let message = match self.overlay {
                    Some(Overlay::Constants { .. }) => "constant list needs a choice",
                    _ => "variable list needs a choice",
                };
                EvalError::NeedChoice(message)
            } else {
                EvalError::Note(note)
            });
        }
        Ok(())
    }

    fn script_boundary(&mut self) -> Result<(), EvalError> {
        if self.should_quit() {
            return Err(EvalError::Quit);
        }
        if self.help_open() || self.docs_open() {
            return Err(EvalError::Help);
        }
        if self.trail_open() {
            return Err(EvalError::Trail);
        }
        match self.engine.status() {
            // A note, such as "precision 12" or "prime", leaves the script running.
            Status::Error(_) | Status::UnknownKey => Err(self.eval_status()),
            Status::Empty | Status::Note(_) => Ok(()),
        }
    }

    fn eval_status(&self) -> EvalError {
        match self.engine.status() {
            Status::Error(err) => EvalError::Calc(err.clone()),
            Status::UnknownKey => EvalError::UnknownKey,
            Status::Note(text) => EvalError::Note(text.clone()),
            Status::Empty => EvalError::Note(String::new()),
        }
    }

    /// A prompt word such as `16` after `d r`, or `name` after `s s`.
    fn pending_answer_ready(&self) -> bool {
        match &self.pending {
            Pending::Word(text) | Pending::Radix { digits: text, .. } => !text.is_empty(),
            Pending::Var { name, .. } => !name.is_empty(),
            Pending::None => false,
        }
    }

    fn incomplete_prompt(&self) -> Option<&'static str> {
        match &self.pending {
            Pending::None => None,
            Pending::Radix { .. } => Some("radix needs a number"),
            Pending::Word(_) => Some("word size needs a number"),
            Pending::Var { .. } => Some("variable needs a name"),
        }
    }
}

fn move_cursor(view: &mut TrailView, delta: isize) {
    let n = view.matches.len();
    if n == 0 {
        return;
    }
    let current = view.cursor.min(n - 1) as isize;
    view.cursor = (current + delta).rem_euclid(n as isize) as usize;
}

/// Keys of a command prefix, as a trail label shows them.
fn prefix_keys(prefix: Prefix) -> &'static str {
    match prefix {
        Prefix::None => "",
        Prefix::F => "f",
        Prefix::M => "m",
        Prefix::C => "c",
        Prefix::D => "d",
        Prefix::B => "b",
        Prefix::K => "k",
        Prefix::S => "s",
        Prefix::T => "t",
        Prefix::R => "r",
        Prefix::O => "O",
        Prefix::Od => "O d",
    }
}

/// Evaluate one RPN script and return the top of the stack.
///
/// Whitespace separates words, so `2 20 ^ d g` groups `1048576` as
/// `1,048,576`. A word is the same keys the screen reads. A space does not
/// push or duplicate.
pub fn eval(program: &str) -> Result<String, EvalError> {
    let mut app = App::new();
    for word in program.split_whitespace() {
        app.run_word(word)?;
    }
    app.finish_script()?;
    app.top_string().ok_or(EvalError::EmptyStack)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn top(keys: &str) -> String {
        let mut app = App::new();
        app.keys(keys);
        app.top_string().unwrap_or_default()
    }

    fn mode_text(app: &App) -> String {
        app.mode_line().join(" · ")
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
    fn float_notation_follows_precision() {
        // Emacs Calc normal format: fixed from order -2 through precision-1.
        assert_eq!(top("4.2e10\n"), "42000000000.");
        assert_eq!(top("1e11\n"), "100000000000.");
        assert_eq!(top("1e12\n"), "1e12");
        assert_eq!(top("0.01\n"), "0.01");
        assert_eq!(top("0.001\n"), "1e-3");
        assert_eq!(top("6p4.2e10\n"), "4.2e10");
        assert_eq!(top("20p4.2e19\n"), "42000000000000000000.");
        assert_eq!(top("20p1e20\n"), "1e20");
        assert_eq!(top("4.2e10\ndg"), "42,000,000,000.");
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
        assert_eq!(top("1:8\n2B"), "-3");
        assert_eq!(top("0.125\n2B"), "-3");
        // 4^(3/2) is 8. The exponent is not an integer, so this stays a float.
        assert_eq!(top("8\n4B"), "1.5");
        assert_eq!(top("100\nHL"), "2");
        assert_eq!(top("1:1000\nHL"), "-3");
        assert_eq!(top("1e-100001\nHL"), "-100001");
        assert_eq!(top("1e100001\nHL"), "100001");
        assert_eq!(top("0.5\n8I^"), "0.917004043205");
        assert_eq!(top("0.5\n1:8\n^"), "0.917004043205");
        assert_eq!(top("1.1\n7I^"), "1.0137088563");
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
        assert!(!app.inverse);

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
    fn precision_pops_an_integer() {
        let mut app = App::new();
        app.keys("20p");
        assert_eq!(app.engine().mode().prec, 20);
        assert!(app.engine().stack().is_empty());
        assert_eq!(app.engine().message(), "precision 20");
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("20"));
        assert_eq!(app.engine().mode().prec, 20);

        let mut app = App::new();
        app.keys("10\n2*p");
        assert_eq!(app.engine().mode().prec, 20);
        assert!(app.engine().stack().is_empty());

        let mut app = App::new();
        app.keys("30.p");
        assert_eq!(app.engine().mode().prec, 30);
        assert!(app.engine().stack().is_empty());

        let mut app = App::new();
        app.keys("2p");
        assert_eq!(app.engine().mode().prec, 12);
        assert!(app.engine().is_error());
        assert_eq!(app.engine().message(), "precision must be from 3 to 1000");
        assert_eq!(app.top_string().as_deref(), Some("2"));

        let mut app = App::new();
        app.keys("3.5p");
        assert_eq!(app.engine().message(), "precision needs an integer");
        assert_eq!(app.top_string().as_deref(), Some("3.5"));
        assert_eq!(app.engine().mode().prec, 12);
    }

    #[test]
    fn entry_sign_and_cancel() {
        let mut app = App::new();
        app.keys("12");
        app.press(Key::Esc);
        assert!(app.engine().stack().is_empty());
        app.keys("\n");
        assert!(app.engine().stack().is_empty());

        app.keys("I");
        assert!(app.inverse);
        app.press(Key::Esc);
        assert!(!app.inverse);

        assert_eq!(top("5n\nfs"), "-1");
        assert_eq!(top("5\n "), "5");
        assert_eq!(App::run_len("5\n "), 2);
    }

    #[test]
    fn q_cancels_numeric_prompts() {
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
    fn q_quits_from_the_shortcut_list() {
        let mut app = App::new();
        app.keys("?");
        assert!(app.help_open());
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
    fn prime_walking_keys() {
        assert_eq!(eval("7 k n").as_deref(), Ok("11"));
        assert_eq!(eval("11 I k n").as_deref(), Ok("7"));
        assert_eq!(eval("1000000 H k n").as_deref(), Ok("15485863"));
        assert_eq!(eval("10.5 I k n").as_deref(), Ok("7"));
        let mut app = App::new();
        app.keys("2\n89\n^\n2\n-\nkn");
        assert_eq!(
            app.top_string().as_deref(),
            Some("618970019642690137449562111")
        );
        assert_eq!(app.engine().message(), "probably prime");
        app.keys("u");
        assert_eq!(
            app.top_string().as_deref(),
            Some("618970019642690137449562110")
        );
        let mut app = App::new();
        app.keys("0\nHkn");
        assert_eq!(app.engine().message(), "nth prime needs a positive integer");
        assert_eq!(app.engine().stack_lines(), ["0"]);
        let mut app = App::new();
        app.keys("3M7\nkn");
        assert_eq!(app.engine().message(), "not for modulo forms");
    }

    #[test]
    fn totient_follows_calc() {
        assert_eq!(eval("12 k t").as_deref(), Ok("4"));
        assert_eq!(eval("2023. k t").as_deref(), Ok("1632"));
        assert_eq!(eval("1000000007 k t").as_deref(), Ok("1000000006"));
        assert_eq!(eval("0 k t").as_deref(), Ok("0"));
        let mut app = App::new();
        app.keys("_12\nkt");
        assert_eq!(
            app.engine().message(),
            "totient needs a nonnegative integer"
        );
        assert_eq!(app.engine().stack_lines(), ["-12"]);
        let mut app = App::new();
        app.keys("3M7\nkt");
        assert_eq!(app.engine().message(), "not for modulo forms");
    }

    #[test]
    fn prime_factors_push_smallest_first() {
        let mut app = App::new();
        app.keys("2023\nkf");
        assert_eq!(app.engine().stack_lines(), ["7", "17", "17"]);
        assert_eq!(app.engine().message(), "");
        app.keys("**");
        assert_eq!(app.top_string().as_deref(), Some("2023"));

        let mut app = App::new();
        app.keys("5\n2023\nkf");
        assert_eq!(app.engine().stack_lines(), ["5", "7", "17", "17"]);
        app.keys("u");
        assert_eq!(app.engine().stack_lines(), ["5", "2023"]);

        let mut app = App::new();
        app.keys("_2023\nkf");
        assert_eq!(app.engine().stack_lines(), ["-1", "7", "17", "17"]);
        app.keys("***");
        assert_eq!(app.top_string().as_deref(), Some("-2023"));

        assert_eq!(top("0\nkf"), "0");
        assert_eq!(top("1\nkf"), "1");
        assert_eq!(top("_1\nkf"), "-1");
        assert_eq!(top("2023.\nkf**"), "2023");
        assert_eq!(eval("2023 k f").as_deref(), Ok("17"));
        assert_eq!(eval("2023 k f * *").as_deref(), Ok("2023"));

        let mut app = App::new();
        app.keys("3:2\nkf");
        assert_eq!(app.engine().message(), "prime factors need an integer");
        assert!(app.engine().is_error());
        assert_eq!(app.engine().stack_lines(), ["3:2"]);

        let mut app = App::new();
        app.keys("(1, 2)\nkf");
        assert_eq!(app.engine().message(), "prime factors need an integer");
        assert_eq!(app.engine().stack_lines(), ["(1, 2)"]);

        let mut app = App::new();
        app.keys("kf");
        assert_eq!(app.engine().message(), "too few arguments");

        let mut app = App::new();
        app.keys("25030013\nkf");
        assert_eq!(app.engine().stack_lines(), ["25030013"]);
        assert_eq!(app.engine().message(), "");

        let mut app = App::new();
        app.keys("25220483\nkf");
        assert_eq!(app.engine().stack_lines(), ["5021", "5023"]);
        assert_eq!(app.engine().message(), "");

        let mut app = App::new();
        app.keys("25120063\nkf");
        assert_eq!(app.engine().stack_lines(), ["5003", "5021"]);
        assert_eq!(app.engine().message(), "");

        let mut app = App::new();
        app.keys("43920492304\nkf");
        assert_eq!(
            app.engine().stack_lines(),
            ["2", "2", "2", "2", "13", "5903", "35771"]
        );
        assert_eq!(app.engine().message(), "");
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
        assert_eq!(app.top_string().as_deref(), Some("7.297352533e-3"));
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
        assert!(mode_text(&app).contains("hex"));
        assert!(!mode_text(&app).contains("twos"));
        app.keys("dz");
        assert!(mode_text(&app).contains("zeros"));

        let mut app = App::new();
        app.keys("1\nbw8\nd2dz");
        assert_eq!(app.engine().mode().word_size, 8);
        assert_eq!(app.top_string().as_deref(), Some("2#00000001"));

        let mut app = App::new();
        app.keys("3n\nbw8\nOd6");
        assert_eq!(app.top_string().as_deref(), Some("16##FD"));
        assert!(mode_text(&app).contains("twos"));
        assert!(mode_text(&app).contains("w 8"));
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
        app.keys("13p1234567.123454\ndg");
        assert_eq!(app.top_string().as_deref(), Some("1,234,567.123454"));
        assert_eq!(app.engine().message(), "grouping");
        assert!(mode_text(&app).contains("group"));
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

    #[test]
    fn modulo_forms_follow_calc() {
        // Calc: `3 M 1000 RET 567 ^` is `587 mod 1000`. PARI/GP agrees.
        assert_eq!(top("3M1000\n567^"), "587 mod 1000");
        assert_eq!(top("_3M7\n"), "4 mod 7");
        assert_eq!(top("3M7\n4+"), "0 mod 7");
        assert_eq!(top("3M7\n4-"), "6 mod 7");
        assert_eq!(top("3M7\n4M7\n*"), "5 mod 7");
        assert_eq!(top("1\n2\n3M7\n*"), "6 mod 7");
        // Division multiplies by the modular inverse.
        assert_eq!(top("3M7\n2/"), "5 mod 7");
        assert_eq!(top("3M7\n&"), "5 mod 7");
        assert_eq!(top("3M7\nn"), "4 mod 7");
        assert_eq!(top("3M7\nIQ"), "2 mod 7");
        // A negative power inverts first. A form as the exponent uses its
        // residue, as Calc does.
        assert_eq!(top("2M7\n1n^"), "4 mod 7");
        assert_eq!(top("2M7\n3M7\n^"), "1 mod 7");
        assert_eq!(top("2\n3M7\n^"), "1 mod 7");
        assert_eq!(top("0M7\n0^"), "1 mod 7");
        assert_eq!(top("3M1\n"), "0 mod 1");
        // The exponent can be far too large to expand.
        assert_eq!(
            top("123456789M1000000007\n10\n30^^"),
            "493444103 mod 1000000007"
        );
        // Either side can be a radix literal, and display follows the radix.
        assert_eq!(top("16#FFM16#100\n"), "255 mod 256");
        assert_eq!(top("3M7\nd6"), "16#3 mod 16#7");
        // `M` is a digit of a radix literal above 22.
        assert_eq!(top("36#mM7\n"), "29311");
    }

    #[test]
    fn modulo_form_entry_and_errors() {
        let mut app = App::new();
        app.keys("3M");
        assert_eq!(app.entry.as_str(), "3 mod ");
        // DEL removes ` mod ` in one piece, and a space waits for the modulus.
        app.keys("\u{7f}");
        assert_eq!(app.entry.as_str(), "3");
        app.keys("M 7");
        assert_eq!(app.entry.as_str(), "3 mod 7");
        app.keys("\n");
        // A second `M` fills in the previous modulus.
        app.keys("5MM\n+");
        assert_eq!(app.top_string().as_deref(), Some("1 mod 7"));

        // A modulus is never part of a complex value.
        let mut app = App::new();
        app.keys("3M(");
        assert_eq!(app.entry.as_str(), "3 mod ");

        let mut app = App::new();
        app.keys("3MM");
        assert_eq!(app.engine().message(), "no previous modulus");
        assert_eq!(app.entry.as_str(), "3 mod ");

        let mut app = App::new();
        app.keys("M");
        assert_eq!(app.engine().message(), "type an integer before M");

        let mut app = App::new();
        app.keys("1e100001M7");
        assert_eq!(app.entry.as_str(), "1e100001 mod 7");
        app.keys("\n");
        assert_eq!(app.engine().message(), "overflow");

        let mut app = App::new();
        app.keys("3.5M");
        assert_eq!(app.engine().message(), "modulo forms need integers");
        assert_eq!(app.entry.as_str(), "3.5");

        for (keys, message) in [
            ("3M0\n", "modulus must be positive"),
            ("3M7\n4M8\n+", "modulo forms need the same modulus"),
            ("3M6\n2/", "no modular inverse"),
            ("3M7\n0/", "division by zero"),
            ("3M7\n1.5+", "modulo forms need integers"),
            ("3M7\n0.5^", "modulo forms need integers"),
            ("3M7\n1e100001^", "overflow"),
            ("1e100001M7\n", "overflow"),
            ("3M7\nQ", "not for modulo forms"),
            ("3M7\nS", "not for modulo forms"),
            ("3M7\n2%", "not for modulo forms"),
            ("3M7\ncf", "not for modulo forms"),
            ("3M7\nkp", "prime test needs an integer"),
        ] {
            let mut app = App::new();
            app.keys(keys);
            assert_eq!(app.engine().message(), message, "{keys}");
        }

        // A rejected form stays on the stack.
        let mut app = App::new();
        app.keys("3M7\nS");
        assert_eq!(app.top_string().as_deref(), Some("3 mod 7"));

        assert_eq!(eval("3M1000 567 ^").as_deref(), Ok("587 mod 1000"));
        assert_eq!(eval("3M 1000 567 ^").as_deref(), Ok("587 mod 1000"));
    }

    /// Values checked against PARI/GP at 30 digits, rounded to 12.
    #[test]
    fn numeric_results_match_pari() {
        let ok = |program: &str| eval(program).ok().unwrap_or_else(|| "error".to_string());
        // A float base keeps guard digits, and the exponent has no 20-bit cap.
        assert_eq!(ok("1.000001 1000000 ^"), "2.71828046932");
        assert_eq!(ok("1.0000001 2000000 ^"), "1.22140274595");
        assert_eq!(ok("2. 10000000 ^"), "9.04981730636e3010299");
        assert_eq!(ok("1 2000000 ^"), "1");
        assert_eq!(ok("_1 2000001 ^"), "-1");
        assert_eq!(ok("1.1 _7 ^"), "0.513158118231");
        assert_eq!(ok("10 100000 ^ 100000 ^"), "error");
        // Complex square root keeps the small part.
        assert_eq!(ok("(1000000, 1) Q"), "(1000., 5e-4)");
        assert_eq!(ok("(_1000000, _1) Q"), "(5e-4, -1000.)");
        assert_eq!(ok("(3, 4) Q"), "(2., 1.)");
        // Inverse hyperbolics.
        assert_eq!(ok("_1000 H I S"), "-7.60090270954");
        assert_eq!(ok("_1e10 H I S"), "-23.7189981105");
        assert_eq!(ok("1e-13 H I S"), "1e-13");
        assert_eq!(ok("1e-13 H I T"), "1e-13");
        assert_eq!(ok("_0.5 H I T"), "-0.549306144334");
        assert_eq!(ok("1.00000001 H I C"), "1.41421356119e-4");
        assert_eq!(ok("1e2100 H T"), "1.");
        // Inverse trig near and past 1.
        assert_eq!(ok("0.999999999999 I C"), "8.10284684541e-5");
        assert_eq!(ok("0.999999 I C"), "0.0810284752065");
        assert_eq!(ok("0.5 I C"), "60.");
        assert_eq!(ok("0.5 I S"), "30.");
        assert_eq!(ok("1.0000000001 I S"), "(90., -8.10284684535e-4)");
        assert_eq!(ok("_2 I S"), "(-90., 75.4561292902)");
        // A tiny argument is not an overflow.
        assert_eq!(ok("1e-3000 E"), "1.");
        assert_eq!(ok("1e-3000 S"), "1.74532925199e-3002");
        // The principal value, and the real odd root on `I ^`.
        assert_eq!(ok("_8 1:3 ^"), "(1., 1.73205080757)");
        assert_eq!(ok("_8 3 I ^"), "-2");
        // Arithmetic keeps a tiny complex part.
        assert_eq!(ok("(1, 1e-20) (0, 1) *"), "(-1e-20, 1.)");
        // A fraction from 200 digits of pi converts back to the same digits.
        assert_eq!(ok("200 p P c F c f P -"), "0.");
        // pi over a whole number is rounded once.
        assert_eq!(ok("m r 1 I S"), "1.57079632679");
        assert_eq!(ok("m r 0 I C"), "1.57079632679");
        assert_eq!(ok("m r 1 0 f T"), "1.57079632679");
        assert_eq!(ok("m r 1 I T"), "0.785398163397");
        // acos past 1 has no real part, and acosh below -1 does not cancel.
        assert_eq!(ok("m r 2 I C"), "(0, 1.31695789692)");
        assert_eq!(ok("m r _2 I C"), "(3.14159265359, -1.31695789692)");
        assert_eq!(
            ok("_1.0000000001 H I C"),
            "(1.41421356236e-5, 3.14159265359)"
        );
        // tanh is rounded once.
        assert_eq!(ok("0.697636655571 H T"), "0.602865525931");
        assert_eq!(ok("0.00938796789773 H T"), "9.38769210792e-3");
        assert_eq!(ok("3 p 3 H T"), "0.995");
        assert_eq!(ok("3 p 0.5 H T"), "0.462");
        // exp/ln keeps a digit for each integer digit of `exp * ln(base)`.
        assert_eq!(
            ok("_1.23e600000 1000001:3 ^"),
            "(1.25723995417e200000229968, -2.17760347792e200000229968)"
        );
        assert_eq!(ok("1.23e600000 1000000.5 ^"), "1.4334810418e600000389905");
        // A float of magnitude 1, and exponents past `i64`.
        assert_eq!(ok("1. 10000000000000000000 ^"), "1.");
        assert_eq!(ok("_1. 20000000000000000001 ^"), "-1.");
        assert_eq!(
            ok("1.0000001 _9223372036854775808 ^"),
            "4.14980207926e-400565937987"
        );
        // Exact powers: the size bound uses log2 of the base, and a
        // negative power of a fraction is a float unless fraction mode is on.
        assert_eq!(ok("2 4194305 ^ 2 4194304 ^ /"), "2");
        assert_eq!(ok("2 8388609 ^"), "error");
        assert_eq!(ok("3:2 _2 ^"), "0.444444444444");
        assert_eq!(ok("m f 3:2 _2 ^"), "4:9");
        assert_eq!(ok("m f _2 _3 ^"), "-1:8");
        assert_eq!(ok("_3:2 3 ^"), "-27:8");
    }

    #[test]
    fn runaway_exponent_is_an_overflow() {
        let mut program = String::from("2.5e999999");
        for _ in 0..45 {
            program.push_str(" I Q");
        }
        assert!(eval(&program).is_err());
        assert!(eval("1e999999 1e13 ^").is_err());
        assert!(eval("1e-999999 1e13 ^").is_err());
    }

    fn trail_rows(app: &App) -> Vec<(String, String)> {
        app.engine()
            .trail()
            .iter()
            .map(|entry| {
                (
                    entry.label.clone(),
                    radix::format_value(&entry.value, app.engine().mode()),
                )
            })
            .collect()
    }

    fn row(label: &str, value: &str) -> (String, String) {
        (label.to_string(), value.to_string())
    }

    #[test]
    fn trail_records_numbers_and_results() {
        let mut app = App::new();
        app.keys("3 4+2*\n\t\u{7f}0.5IS");
        assert_eq!(
            trail_rows(&app),
            vec![
                row("", "3"),
                row("", "4"),
                row("+", "7"),
                row("", "2"),
                row("*", "14"),
                row("", "0.5"),
                row("I S", "30."),
            ]
        );

        let mut app = App::new();
        app.keys("2023kf 12ss x\nsr x\nr3 PHP L K0 1 ME");
        let rows = trail_rows(&app);
        assert_eq!(
            &rows[1..4],
            &[row("k f", "7"), row("k f", "17"), row("k f", "17")]
        );
        assert_eq!(rows[5], row("rcl x", "12"));
        assert_eq!(rows[6].0, "P");
        assert_eq!(rows[7].0, "H P");
        assert_eq!(rows[8].0, "L");
        assert_eq!(rows[9], row("K c", "299792458"));
        // `r3` on an empty register fails and records nothing.
        assert_eq!(rows.len(), 12);

        // Undo leaves the trail.
        let mut app = App::new();
        app.keys("2 3^uu");
        assert_eq!(app.engine().stack().len(), 1);
        assert_eq!(trail_rows(&app).last(), Some(&row("^", "8")));
    }

    #[test]
    fn trail_dialog_picks_and_filters() {
        let mut app = App::new();
        app.keys("3 4+ 10*");
        app.keys("ty");
        assert!(app.trail_open());
        assert_eq!(app.trail_cursor(), 4);
        app.press(Key::Up);
        app.press(Key::Up);
        app.press(Key::Enter);
        assert!(!app.trail_open());
        assert_eq!(app.top_string().as_deref(), Some("7"));
        // The pushed value is not recorded again, and u takes it back.
        assert_eq!(app.engine().trail().len(), 5);
        app.keys("u");
        assert_eq!(app.top_string().as_deref(), Some("70"));

        app.keys("ty+");
        assert_eq!(app.trail_matches(), vec![2]);
        app.press(Key::Enter);
        assert_eq!(app.top_string().as_deref(), Some("7"));

        // Typed digits search the values. `q` is a letter there.
        app.keys("ty1q");
        assert!(!app.should_quit());
        assert!(app.trail_matches().is_empty());
        app.press(Key::Enter);
        assert!(app.trail_open());
        app.keys("\u{7f}");
        assert_eq!(app.trail_matches(), vec![3]);
        app.press(Key::Esc);
        assert!(!app.trail_open());
        assert_eq!(app.top_string().as_deref(), Some("7"));

        // Grouping commas do not get in the way of a search.
        let mut app = App::new();
        app.keys("50p P 1e19* dg ty31415");
        assert_eq!(app.trail_matches(), vec![3]);
        app.keys("\u{7f}\u{7f}\u{7f},415");
        assert_eq!(app.trail_matches(), vec![3]);

        // The flags have a fixed order, whichever came first.
        let mut app = App::new();
        app.keys("2IHS 2HIS");
        let rows = trail_rows(&app);
        assert_eq!(rows[1].0, "H I S");
        assert_eq!(rows[3].0, "H I S");

        // A huge value is formatted once and kept short for display, but
        // the search still sees all of its digits.
        let mut app = App::new();
        app.keys("2 8191^ ty");
        let view = app.trail_view().unwrap();
        assert_eq!(view.rows[2].text.chars().count(), TRAIL_TEXT_MAX);
        app.keys("7857896448");
        assert_eq!(app.trail_matches(), vec![2]);
        // A comma inside a complex number matches as typed.
        let mut app = App::new();
        app.keys("(1, 23) ty1, 2");
        assert_eq!(app.trail_matches(), vec![0]);

        // An empty trail opens, and RET does nothing.
        let mut app = App::new();
        app.keys("ty\n");
        assert!(app.trail_open());
        assert!(app.engine().stack().is_empty());

        assert_eq!(eval("1 t y"), Err(EvalError::Trail));
    }

    #[test]
    fn eval_splits_words_and_prints_the_top() {
        assert_eq!(eval("2 20 ^ d g").as_deref(), Ok("1,048,576"));
        assert_eq!(eval("2 20 ^dg").as_deref(), Ok("1,048,576"));
        assert_eq!(eval("2 20 ^\nd g").as_deref(), Ok("1,048,576"));
        assert_eq!(eval("1 2 3 4 + * -").as_deref(), Ok("-13"));
        assert_eq!(eval("1 2 3").as_deref(), Ok("3"));
        assert_eq!(eval("2").as_deref(), Ok("2"));
        assert_eq!(eval("90 S").as_deref(), Ok("1"));
        assert_eq!(eval("125 3 I ^").as_deref(), Ok("5"));
        assert_eq!(eval("5 2 H k c").as_deref(), Ok("20"));
        assert_eq!(eval("4 2 k s").as_deref(), Ok("11"));
        assert_eq!(eval("4 2 H k s").as_deref(), Ok("7"));
        assert_eq!(eval("10 5 H k s").as_deref(), Ok("42525"));
        assert_eq!(eval("2 3 k s").as_deref(), Ok("0"));
        assert_eq!(eval("(3, 4) A").as_deref(), Ok("5"));
        assert_eq!(eval("(3,4) A").as_deref(), Ok("5"));
        assert_eq!(eval("36#q").as_deref(), Ok("26"));
        assert_eq!(eval("K 0").as_deref(), Ok("299792458"));
        assert_eq!(eval("6 p 4.2e10").as_deref(), Ok("4.2e10"));
        assert_eq!(eval("6p 4.2e10").as_deref(), Ok("4.2e10"));
        assert_eq!(eval("mf 8 6 /").as_deref(), Ok("4:3"));
        assert_eq!(eval("4.2e10 d g").as_deref(), Ok("42,000,000,000."));
        assert_eq!(eval("10 3 \\").as_deref(), Ok("3"));
        assert_eq!(eval("3 n b w 8 O d 6").as_deref(), Ok("16##FD"));
        assert_eq!(eval("255 d r 16").as_deref(), Ok("16#FF"));
        assert_eq!(eval("5 k p").as_deref(), Ok("5"));
        assert_eq!(eval("5 s s a s r a").as_deref(), Ok("5"));
        assert_eq!(eval("5 s s a s l a").as_deref(), Ok("5"));

        let mut typed = App::new();
        typed.keys("HP");
        assert_eq!(eval("H P").unwrap(), typed.top_string().unwrap());
    }

    #[test]
    fn eval_reports_errors_and_stops() {
        fn shown(program: &str) -> (EvalError, String) {
            let err = eval(program).unwrap_err();
            let text = err.to_string();
            (err, text)
        }
        assert_eq!(
            shown("1 0 /"),
            (
                EvalError::Calc(CalcError::DivideByZero),
                "division by zero".to_string()
            )
        );
        assert_eq!(
            shown("1 0 / 4 +"),
            (
                EvalError::Calc(CalcError::DivideByZero),
                "division by zero".to_string()
            )
        );
        assert_eq!(
            shown("5 +"),
            (
                EvalError::Calc(CalcError::StackUnderflow),
                "too few arguments".to_string()
            )
        );
        assert_eq!(
            shown("1 x"),
            (EvalError::UnknownKey, "unknown key".to_string())
        );
        assert_eq!(
            shown("5 xdg"),
            (EvalError::UnknownKey, "unknown key".to_string())
        );
        assert_eq!(
            shown(""),
            (EvalError::EmptyStack, "empty stack".to_string())
        );
        assert_eq!(
            shown("  \n"),
            (EvalError::EmptyStack, "empty stack".to_string())
        );
        assert_eq!(
            shown("5 s t a"),
            (EvalError::EmptyStack, "empty stack".to_string())
        );
        assert_eq!(
            shown("q"),
            (EvalError::Quit, "q quits the screen".to_string())
        );
        assert_eq!(
            shown("?"),
            (EvalError::Help, "help is for the screen".to_string())
        );
        assert_eq!(
            shown("p"),
            (
                EvalError::Calc(CalcError::StackUnderflow),
                "too few arguments".to_string()
            )
        );
        assert_eq!(
            shown("2 p"),
            (
                EvalError::Calc(CalcError::BadPrecision),
                "precision must be from 3 to 1000".to_string()
            )
        );
        assert_eq!(
            shown("2 p 4 +"),
            (
                EvalError::Calc(CalcError::BadPrecision),
                "precision must be from 3 to 1000".to_string()
            )
        );
        assert_eq!(
            shown("3.5 p"),
            (
                EvalError::Calc(CalcError::Domain("precision needs an integer")),
                "precision needs an integer".to_string()
            )
        );
        assert_eq!(
            shown("d r"),
            (
                EvalError::Prompt("radix needs a number"),
                "radix needs a number".to_string()
            )
        );
        assert_eq!(
            shown("b w"),
            (
                EvalError::Prompt("word size needs a number"),
                "word size needs a number".to_string()
            )
        );
        assert_eq!(
            shown("s s"),
            (
                EvalError::Prompt("variable needs a name"),
                "variable needs a name".to_string()
            )
        );
        assert_eq!(
            shown("5 K"),
            (
                EvalError::NeedChoice("constant list needs a choice"),
                "constant list needs a choice".to_string()
            )
        );
        assert_eq!(
            shown("s l"),
            (
                EvalError::NeedChoice("variable list needs a choice"),
                "variable list needs a choice".to_string()
            )
        );
        assert_eq!(
            shown("(3,"),
            (
                EvalError::Calc(CalcError::InvalidEntry("(3,".to_string())),
                "bad entry: (3,".to_string()
            )
        );
    }

    #[test]
    fn every_binding_reaches_a_command() {
        for binding in crate::docs::bindings() {
            let mut app = App::new();
            for key in binding_keys(binding.keys) {
                app.press(key);
            }
            assert!(
                !matches!(app.engine().status(), Status::UnknownKey),
                "{}",
                binding.keys
            );
        }
    }

    #[test]
    fn prefix_labels_come_from_the_bindings() {
        let mut app = App::new();
        app.press(Key::Char('f'));
        assert!(mode_text(&app).contains("f-"));

        let mut app = App::new();
        app.press(Key::Char('O'));
        assert!(mode_text(&app).contains("O-"));
        assert!(!mode_text(&app).contains("Od-"));
        app.press(Key::Char('d'));
        assert!(mode_text(&app).contains("Od-"));
    }

    #[test]
    fn binding_words_are_single_keys() {
        assert_eq!(binding_keys("C-r"), vec![Key::Ctrl('r')]);
        assert_eq!(binding_keys("S-Tab"), vec![Key::ShiftTab]);
        assert_eq!(binding_keys("ret"), vec![Key::Enter]);
        assert_eq!(binding_keys("del"), vec![Key::Backspace]);
        assert_eq!(binding_keys("tab"), vec![Key::Tab]);
        assert_eq!(binding_keys("Esc"), vec![Key::Esc]);
        assert_eq!(binding_keys("I Q"), vec![Key::Char('I'), Key::Char('Q')]);
        assert_eq!(
            binding_keys("Hk c"),
            vec![Key::Char('H'), Key::Char('k'), Key::Char('c')]
        );
        assert_eq!(binding_keys("k s"), vec![Key::Char('k'), Key::Char('s')]);
        assert_eq!(
            binding_keys("Hk s"),
            vec![Key::Char('H'), Key::Char('k'), Key::Char('s')]
        );
    }

    fn binding_keys(keys: &str) -> Vec<Key> {
        let mut rest = keys;
        let mut out = Vec::new();
        while !rest.is_empty() {
            if rest.starts_with(' ') {
                rest = &rest[1..];
                continue;
            }
            let (key, tail) = if let Some(tail) = rest.strip_prefix("C-r") {
                (Key::Ctrl('r'), tail)
            } else if let Some(tail) = rest.strip_prefix("S-Tab") {
                (Key::ShiftTab, tail)
            } else if let Some(tail) = rest.strip_prefix("Esc") {
                (Key::Esc, tail)
            } else if let Some(tail) = rest.strip_prefix("ret") {
                (Key::Enter, tail)
            } else if let Some(tail) = rest.strip_prefix("del") {
                (Key::Backspace, tail)
            } else if let Some(tail) = rest.strip_prefix("tab") {
                (Key::Tab, tail)
            } else {
                let c = rest.chars().next().expect("key");
                (Key::Char(c), &rest[c.len_utf8()..])
            };
            out.push(key);
            rest = tail;
        }
        out
    }

    impl App {
        fn run_len(keys: &str) -> usize {
            let mut app = App::new();
            app.keys(keys);
            app.engine().stack().len()
        }
    }
}
