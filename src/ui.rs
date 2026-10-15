//! Full-screen stack, a rounded prompt, and a shortcut bar.
//!
//! Colors are the GrokNight palette from Grok Build: gray canvas, a focused
//! prompt border, bold secondary keys, and muted labels.

use crate::app::{App, Key, VAR_COLUMN};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::io;
use std::time::Duration;

/// Main background, `#141414`.
const BG: Color = Color::Rgb(20, 20, 20);
/// Primary text, `#e1e1e1`.
const FG: Color = Color::Rgb(225, 225, 225);
/// Secondary text and shortcut keys, `#c8c8c8`.
const FG_SECONDARY: Color = Color::Rgb(200, 200, 200);
/// Muted labels, `#6c6c6c`.
const GRAY: Color = Color::Rgb(108, 108, 108);
/// Dim chrome, `#585858`.
const GRAY_DIM: Color = Color::Rgb(88, 88, 88);
/// Focused prompt border, `#505058`.
const PROMPT_BORDER: Color = Color::Rgb(80, 80, 88);
/// Error text, `#f7768e`.
const ERROR: Color = Color::Rgb(247, 118, 142);
/// Columns kept clear of the screen edge, matching Grok Build's outer padding.
const SIDE_PAD: u16 = 2;
/// Blank columns between the prompt border and the text being typed.
const PROMPT_INNER: u16 = 1;
/// Spaces before each stack value, so the first digit sits on the typed character in `❯ x`.
/// That is the left border, the inner pad, and the `❯ ` prefix.
const STACK_INDENT: usize = 1 + PROMPT_INNER as usize + 2;

const SHORTCUTS: &[(&str, &str)] = &[
    ("ret", "push"),
    ("tab", "swap"),
    ("del", "pop"),
    ("u", "undo"),
    ("?", "help"),
    ("q", "quit"),
];

pub fn run() -> io::Result<()> {
    let mut app = App::new();
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| draw(frame, &app))?;
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if let Some(mapped) = map_key(&key) {
                        app.press(mapped);
                    }
                }
            }
            if app.should_quit() {
                break;
            }
        }
        Ok(())
    })
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame
        .buffer_mut()
        .set_style(area, Style::default().fg(FG).bg(BG));

    // Prompt, then a blank row, the shortcuts, and a blank row at the screen edge.
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inset_sides(area));

    draw_stack(frame.buffer_mut(), chunks[0], app);
    draw_prompt(frame.buffer_mut(), chunks[1], app);
    draw_shortcuts(frame.buffer_mut(), chunks[3], app);
    if app.help_open() {
        draw_help(frame.buffer_mut(), area);
    }
    if app.vars_open() {
        draw_variables(frame.buffer_mut(), area, app);
    }
}

/// Pull content in from the left and right edges. A too-narrow area collapses to nothing.
fn inset_sides(area: Rect) -> Rect {
    let pad = SIDE_PAD.min(area.width / 2);
    Rect {
        x: area.x + pad,
        y: area.y,
        width: area.width.saturating_sub(pad.saturating_mul(2)),
        height: area.height,
    }
}

fn draw_stack(buf: &mut Buffer, area: Rect, app: &App) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let lines = stack_lines(app, area.height as usize);
    Paragraph::new(lines).render(area, buf);
}

fn stack_lines(app: &App, rows: usize) -> Vec<Line<'static>> {
    if rows == 0 {
        return Vec::new();
    }
    let raw = app.engine().stack_lines();
    let room = rows - 1;
    let start = raw.len().saturating_sub(room);
    let pad = room.saturating_sub(raw.len() - start);
    let mut lines = Vec::with_capacity(rows);
    let blank = Style::default().bg(BG);
    for _ in 0..pad {
        lines.push(Line::from(Span::styled(" ", blank)));
    }
    let value = Style::default().fg(FG).bg(BG);
    let indent = " ".repeat(STACK_INDENT);
    for line in &raw[start..] {
        lines.push(Line::from(Span::styled(format!("{indent}{line}"), value)));
    }
    lines.push(Line::from(Span::styled(
        format!("{indent}."),
        Style::default().fg(GRAY_DIM).bg(BG),
    )));
    lines
}

fn draw_prompt(buf: &mut Buffer, area: Rect, app: &App) {
    if area.height < 3 || area.width < 4 {
        return;
    }
    let border = Style::default().fg(PROMPT_BORDER).bg(BG);
    let last_x = area.x + area.width - 1;
    let top = area.y;
    let mid = area.y + 1;
    let bottom = area.y + 2;

    for x in area.x..area.x + area.width {
        let (top_ch, bot_ch) = if x == area.x {
            ('╭', '╰')
        } else if x == last_x {
            ('╮', '╯')
        } else {
            ('─', '─')
        };
        paint_char(buf, x, top, top_ch, border);
        paint_char(buf, x, bottom, bot_ch, border);
        if x == area.x || x == last_x {
            paint_char(buf, x, mid, '│', border);
        } else {
            paint_char(buf, x, mid, ' ', Style::default().fg(FG).bg(BG));
        }
    }

    let text_start = area.x + 1 + PROMPT_INNER;
    let text_end = last_x.saturating_sub(PROMPT_INNER);
    let mut x = text_start;
    x = paint_str(buf, x, mid, text_end, "❯ ", arrow_style());
    let typed = app.composer_text();
    x = paint_str(buf, x, mid, text_end, &typed, text_style());
    if x < text_end {
        paint_char(
            buf,
            x,
            mid,
            ' ',
            text_style().add_modifier(Modifier::REVERSED),
        );
    }

    paint_info(buf, area, bottom, app);
}

fn paint_info(buf: &mut Buffer, area: Rect, y: u16, app: &App) {
    let last_x = area.x + area.width - 1;
    let caption = format!("{} ", app.mode_line().replace("  ", " · "));
    let pad_style = Style::default().bg(BG);
    // One blank cell inside the left corner, so the mode line does not sit on it.
    paint_char(buf, area.x + 1, y, ' ', pad_style);

    let message = app.engine().message();
    // The corner stays `─╯`. A message leaves one blank cell to the left of that dash.
    // With no message the border runs unbroken into the corner.
    let msg_end = if message.is_empty() {
        last_x
    } else {
        let pad_x = last_x.saturating_sub(PROMPT_INNER + 1);
        paint_char(buf, pad_x, y, ' ', pad_style);
        pad_x
    };
    let msg = if message.is_empty() {
        String::new()
    } else {
        format!(" {message}")
    };
    let msg_style = if app.engine().is_error() {
        Style::default().fg(ERROR).bg(BG)
    } else {
        Style::default().fg(GRAY).bg(BG)
    };

    let msg_room = (msg_end as usize).saturating_sub(area.x as usize + 1 + PROMPT_INNER as usize);
    let msg_text: String = msg.chars().take(msg_room).collect();
    let msg_w = msg_text.chars().count() as u16;
    let msg_x = msg_end.saturating_sub(msg_w);

    let caption_end = if msg_text.is_empty() {
        msg_end
    } else {
        msg_x.saturating_sub(1)
    };
    let caption_x = area.x + 1 + PROMPT_INNER;
    if caption_end > caption_x {
        paint_str(
            buf,
            caption_x,
            y,
            caption_end,
            &caption,
            Style::default().fg(GRAY).bg(BG),
        );
    }
    if !msg_text.is_empty() && msg_x < msg_end {
        paint_str(buf, msg_x, y, msg_end, &msg_text, msg_style);
    }
}

/// Rows inside the help box below the title border, before the footer.
const HELP_V_PAD: u16 = 1;
/// Columns inside the help box on each side of the text.
const HELP_H_PAD: u16 = 2;
/// Blank row plus the close hints, above the bottom border.
const HELP_FOOTER: u16 = 2;
/// How many shortcut columns to aim for. Narrow terminals use fewer.
const HELP_COLS: usize = 4;
/// Blank columns between shortcut columns.
const HELP_GAP: usize = 2;
/// Narrowest column that still fits `key` plus a short label.
const HELP_COL_MIN: usize = 12;
/// Key field width. Every binding here is at most this many characters.
const HELP_KEY_W: usize = 4;

/// One topic per column. Lengths differ by at most one row. A narrow terminal stacks later topics under earlier ones.
const HELP_GROUPS: [&[(&str, &str)]; 4] = [
    &[
        ("ret", "push"),
        ("del", "pop"),
        ("tab", "swap"),
        ("TAB", "roll"),
        ("u", "undo"),
        ("C-r", "redo"),
        ("n", "negate"),
        ("&", "recip"),
        ("Q", "sqrt"),
        ("I Q", "square"),
        ("A", "abs"),
        ("f A", "abs^2"),
        ("I", "inverse"),
        ("s s", "store"),
        ("s t", "into"),
        ("s r", "recall"),
        ("s u", "unstore"),
        ("s l", "list"),
    ],
    &[
        ("+", "add"),
        ("-", "sub"),
        ("*", "mul"),
        ("/", "div"),
        ("^", "power"),
        ("\\", "idiv"),
        ("F", "floor"),
        ("%", "mod"),
        (":", "fraction"),
        ("!", "fact"),
        ("k g", "gcd"),
        ("k l", "lcm"),
        ("k E", "egcd"),
        ("k p", "prime"),
        ("k c", "choose"),
        ("Hk c", "perm"),
        ("P", "pi"),
        ("J", "conj"),
    ],
    &[
        ("S", "sin"),
        ("C", "cos"),
        ("T", "tan"),
        ("L", "ln"),
        ("E", "exp"),
        ("B", "log"),
        ("I B", "antilog"),
        ("H L", "log10"),
        ("H E", "10^x"),
        ("f T", "atan2"),
        ("f h", "hypot"),
        ("f Q", "isqrt"),
        ("f E", "expm1"),
        ("f L", "lnp1"),
        ("f I", "ilog"),
        ("f s", "sign"),
        ("G", "arg"),
    ],
    &[
        ("p", "prec"),
        ("m d", "deg"),
        ("m r", "rad"),
        ("m f", "fractions"),
        ("c f", "float"),
        ("c F", "fraction"),
        ("d 2", "bin"),
        ("d 8", "oct"),
        ("d 6", "hex"),
        ("d 0", "dec"),
        ("d r", "radix"),
        ("d z", "zeros"),
        ("d g", "group"),
        ("b w", "word"),
        ("O d", "twos"),
        ("Esc", "cancel"),
        ("H P", "e"),
        ("H", "hyp"),
    ],
];

/// Centered shortcuts box, in the shape of Grok Build's cheatsheet: gray frame,
/// bold title on the top border, and close hints centered inside the bottom.
fn draw_help(buf: &mut Buffer, area: Rect) {
    let chrome_y = 2 + HELP_V_PAD + HELP_FOOTER;
    let width = 68u16.min(area.width);
    let inner_w = width.saturating_sub(2 + HELP_H_PAD * 2);
    if inner_w < 8 || area.height < chrome_y {
        return;
    }
    let (cols, col_w) = help_layout(inner_w);
    let columns = help_columns(cols);
    let grid_rows = columns.iter().map(|column| column.len()).max().unwrap_or(0) as u16;
    let height = (grid_rows + chrome_y).min(area.height);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };

    let fill = Style::default().bg(BG);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            paint_char(buf, x, y, ' ', fill);
        }
    }

    let border = Style::default().fg(GRAY_DIM).bg(BG);
    let last_x = rect.x + rect.width - 1;
    let last_y = rect.y + rect.height - 1;
    for x in rect.x..rect.x + rect.width {
        let (top_ch, bot_ch) = if x == rect.x {
            ('┌', '└')
        } else if x == last_x {
            ('┐', '┘')
        } else {
            ('─', '─')
        };
        paint_char(buf, x, rect.y, top_ch, border);
        paint_char(buf, x, last_y, bot_ch, border);
    }
    for y in rect.y + 1..last_y {
        paint_char(buf, rect.x, y, '│', border);
        paint_char(buf, last_x, y, '│', border);
    }

    let title_end = last_x.saturating_sub(8);
    let mut title_x = rect.x + 1;
    title_x = paint_str(buf, title_x, rect.y, title_end, "─ ", border);
    title_x = paint_str(
        buf,
        title_x,
        rect.y,
        title_end,
        "Shortcuts",
        Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD),
    );
    paint_str(buf, title_x, rect.y, title_end, " ─", border);

    let close = " [x] ";
    let close_x = rect.x + rect.width - close.chars().count() as u16 - 2;
    if close_x > title_x + 1 {
        paint_str(
            buf,
            close_x,
            rect.y,
            last_x,
            close,
            Style::default().fg(GRAY).bg(BG),
        );
    }

    let note_y = rect.y + 1;
    paint_str(
        buf,
        rect.x + 1 + HELP_H_PAD,
        note_y,
        last_x,
        "any key closes",
        Style::default().fg(GRAY).bg(BG),
    );

    let content_h = rect.height.saturating_sub(chrome_y);
    if content_h > 0 {
        let content = Rect {
            x: rect.x + 1 + HELP_H_PAD,
            y: rect.y + 1 + HELP_V_PAD,
            width: inner_w,
            height: content_h,
        };
        draw_help_grid(buf, content, &columns, col_w, content_h);
    }

    let inner_x = rect.x + 1 + HELP_H_PAD;
    let inner_w = rect.width.saturating_sub(2 + HELP_H_PAD * 2);
    draw_help_footer(buf, inner_x, last_y - 1, inner_w);
}

/// `(columns, column width)` for the shortcut grid. Prefers four columns.
fn help_layout(width: u16) -> (usize, usize) {
    let width = width as usize;
    let mut cols = HELP_COLS.min(HELP_GROUPS.len()).max(1);
    while cols > 1 && cols * HELP_COL_MIN + (cols - 1) * HELP_GAP > width {
        cols -= 1;
    }
    let col_w = (width - (cols - 1) * HELP_GAP) / cols;
    (cols, col_w.max(1))
}

fn help_columns(cols: usize) -> Vec<Vec<(&'static str, &'static str)>> {
    let cols = cols.max(1);
    if cols >= HELP_GROUPS.len() {
        return HELP_GROUPS.iter().map(|group| group.to_vec()).collect();
    }
    let mut columns = vec![Vec::new(); cols];
    for (index, group) in HELP_GROUPS.iter().enumerate() {
        columns[index % cols].extend_from_slice(group);
    }
    columns
}

fn draw_help_grid(
    buf: &mut Buffer,
    area: Rect,
    columns: &[Vec<(&str, &str)>],
    col_w: usize,
    rows_fit: u16,
) {
    let show = rows_fit as usize;
    let key_style = Style::default()
        .fg(FG_SECONDARY)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(GRAY).bg(BG);
    let pad_style = Style::default().bg(BG);
    for (col, entries) in columns.iter().enumerate() {
        for (row, (key, label)) in entries.iter().enumerate().take(show) {
            let y = area.y + row as u16;
            let x = area.x + (col * (col_w + HELP_GAP)) as u16;
            let end = x + col_w as u16;
            let mut cursor = paint_str(buf, x, y, end, key, key_style);
            let label_at = x + HELP_KEY_W as u16 + 1;
            while cursor < label_at.min(end) {
                paint_char(buf, cursor, y, ' ', pad_style);
                cursor += 1;
            }
            if label_at < end {
                paint_str(buf, label_at, y, end, label, label_style);
            }
        }
    }
}

fn draw_help_footer(buf: &mut Buffer, x: u16, y: u16, width: u16) {
    draw_centered_hints(buf, x, y, width, &[("esc", "close"), ("q", "quit")]);
}

/// `key label` groups, centered, separated by `  |  `.
fn draw_centered_hints(buf: &mut Buffer, x: u16, y: u16, width: u16, hints: &[(&str, &str)]) {
    let sep = "  |  ";
    let total = hints
        .iter()
        .map(|(key, label)| key.chars().count() + 1 + label.chars().count())
        .sum::<usize>()
        + sep.chars().count() * hints.len().saturating_sub(1);
    if total > width as usize {
        return;
    }
    let mut col = x + (width as usize - total) as u16 / 2;
    let end = x + width;
    let key_style = Style::default()
        .fg(FG_SECONDARY)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(GRAY).bg(BG);
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            col = paint_str(buf, col, y, end, sep, Style::default().fg(GRAY_DIM).bg(BG));
        }
        col = paint_str(buf, col, y, end, key, key_style);
        col = paint_str(buf, col, y, end, " ", label_style);
        col = paint_str(buf, col, y, end, label, label_style);
    }
}

/// Stored variables, ten to a page: five down the left column, five down the right.
fn draw_variables(buf: &mut Buffer, area: Rect, app: &App) {
    let rows = app.variable_rows();
    let grid_rows = if rows.is_empty() {
        1
    } else {
        let left = rows.len().min(VAR_COLUMN);
        let right = rows.len().saturating_sub(VAR_COLUMN);
        left.max(right) as u16
    };
    let chrome_y = 2 + HELP_V_PAD + HELP_FOOTER;
    let width = 68u16.min(area.width);
    let inner_w = width.saturating_sub(2 + HELP_H_PAD * 2);
    if inner_w < 8 || area.height < chrome_y {
        return;
    }
    let height = (grid_rows + chrome_y).min(area.height);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };

    let fill = Style::default().bg(BG);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            paint_char(buf, x, y, ' ', fill);
        }
    }

    let border = Style::default().fg(GRAY_DIM).bg(BG);
    let last_x = rect.x + rect.width - 1;
    let last_y = rect.y + rect.height - 1;
    for x in rect.x..rect.x + rect.width {
        let (top_ch, bot_ch) = if x == rect.x {
            ('┌', '└')
        } else if x == last_x {
            ('┐', '┘')
        } else {
            ('─', '─')
        };
        paint_char(buf, x, rect.y, top_ch, border);
        paint_char(buf, x, last_y, bot_ch, border);
    }
    for y in rect.y + 1..last_y {
        paint_char(buf, rect.x, y, '│', border);
        paint_char(buf, last_x, y, '│', border);
    }

    let title_end = last_x.saturating_sub(8);
    let mut title_x = rect.x + 1;
    title_x = paint_str(buf, title_x, rect.y, title_end, "─ ", border);
    title_x = paint_str(
        buf,
        title_x,
        rect.y,
        title_end,
        "Variables",
        Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD),
    );
    paint_str(buf, title_x, rect.y, title_end, " ─", border);

    let close = " [x] ";
    let close_x = rect.x + rect.width - close.chars().count() as u16 - 2;
    if close_x > title_x + 1 {
        paint_str(
            buf,
            close_x,
            rect.y,
            last_x,
            close,
            Style::default().fg(GRAY).bg(BG),
        );
    }

    let inner_x = rect.x + 1 + HELP_H_PAD;
    let text_w = rect.width.saturating_sub(2 + HELP_H_PAD * 2);
    let note = if app.var_note().is_empty() {
        "letter or digit recalls"
    } else {
        app.var_note()
    };
    let note_style = if app.var_note().is_empty() {
        Style::default().fg(GRAY).bg(BG)
    } else {
        Style::default().fg(ERROR).bg(BG)
    };
    let page = if app.var_pages() > 1 {
        format!("{}/{}", app.var_page() + 1, app.var_pages())
    } else {
        String::new()
    };
    let page_w = page.chars().count() as u16;
    let page_x = inner_x + text_w.saturating_sub(page_w);
    let note_end = if page_w == 0 {
        inner_x + text_w
    } else {
        page_x.saturating_sub(1)
    };
    paint_str(buf, inner_x, rect.y + 1, note_end, note, note_style);
    if page_w > 0 {
        paint_str(
            buf,
            page_x,
            rect.y + 1,
            inner_x + text_w,
            &page,
            Style::default().fg(GRAY).bg(BG),
        );
    }

    let content_y = rect.y + 1 + HELP_V_PAD;
    let content_h = rect.height.saturating_sub(chrome_y);
    if rows.is_empty() {
        paint_str(
            buf,
            inner_x,
            content_y,
            inner_x + text_w,
            "nothing stored",
            Style::default().fg(GRAY).bg(BG),
        );
    } else if content_h > 0 {
        let col_w = (text_w as usize - HELP_GAP) / 2;
        for (index, (name, value)) in rows.iter().enumerate() {
            let col = index / VAR_COLUMN;
            let row = index % VAR_COLUMN;
            if col > 1 || row >= content_h as usize || col_w == 0 {
                continue;
            }
            let x = inner_x + (col * (col_w + HELP_GAP)) as u16;
            paint_var_cell(
                buf,
                x,
                content_y + row as u16,
                x + col_w as u16,
                name,
                value,
            );
        }
    }

    let hints = var_footer_hints(app);
    draw_centered_hints(buf, inner_x, last_y - 1, text_w, &hints);
}

fn var_footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    let mut hints = Vec::new();
    if app.var_pages() > 1 && app.var_page() + 1 < app.var_pages() {
        hints.push(("tab", "next"));
    }
    if app.var_page() > 0 {
        hints.push(("TAB", "prev"));
    }
    hints.push(("esc", "close"));
    hints
}

/// `name: value`, cutting the value with `..` when the cell is too narrow.
fn paint_var_cell(buf: &mut Buffer, x: u16, y: u16, end: u16, name: &str, value: &str) {
    if end <= x {
        return;
    }
    let width = (end - x) as usize;
    let name_style = Style::default()
        .fg(FG_SECONDARY)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let punct = Style::default().fg(GRAY).bg(BG);
    let value_style = Style::default().fg(FG).bg(BG);
    let name_len = name.chars().count();
    if name_len + 2 >= width {
        let keep = width.saturating_sub(2);
        let head: String = name.chars().take(keep).collect();
        let col = paint_str(buf, x, y, end, &head, name_style);
        paint_str(buf, col, y, end, "..", punct);
        return;
    }
    let mut col = paint_str(buf, x, y, end, name, name_style);
    col = paint_str(buf, col, y, end, ": ", punct);
    let room = (end - col) as usize;
    if value.chars().count() <= room {
        paint_str(buf, col, y, end, value, value_style);
        return;
    }
    let keep = room.saturating_sub(2);
    let head: String = value.chars().take(keep).collect();
    col = paint_str(buf, col, y, end, &head, value_style);
    paint_str(buf, col, y, end, "..", punct);
}

fn draw_shortcuts(buf: &mut Buffer, area: Rect, app: &App) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y;
    let end = area.x + area.width;
    for x in area.x..end {
        paint_char(buf, x, y, ' ', Style::default().fg(GRAY).bg(BG));
    }

    let var_hints = var_footer_hints(app);
    let hints: &[(&str, &str)] = if app.help_open() {
        &[("esc", "close"), ("q", "quit")]
    } else if app.vars_open() {
        &var_hints
    } else {
        SHORTCUTS
    };
    let key_style = Style::default()
        .fg(FG_SECONDARY)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(GRAY).bg(BG);
    let sep_style = Style::default().fg(GRAY_DIM).bg(BG);

    let mut x = area.x;
    for (i, (key, label)) in hints.iter().enumerate() {
        let sep = if i > 0 { 5 } else { 0 };
        let piece = key.chars().count() + 1 + label.chars().count();
        if x as usize + sep + piece > end as usize {
            break;
        }
        if sep > 0 {
            x = paint_str(buf, x, y, end, "  │  ", sep_style);
        }
        x = paint_str(buf, x, y, end, key, key_style);
        x = paint_str(buf, x, y, end, ":", label_style);
        x = paint_str(buf, x, y, end, label, label_style);
    }
}

fn arrow_style() -> Style {
    Style::default().fg(FG_SECONDARY).bg(BG)
}

fn text_style() -> Style {
    Style::default().fg(FG).bg(BG)
}

fn paint_char(buf: &mut Buffer, x: u16, y: u16, ch: char, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch);
        cell.set_style(style);
    }
}

fn paint_str(buf: &mut Buffer, x: u16, y: u16, end: u16, text: &str, style: Style) -> u16 {
    let mut col = x;
    for ch in text.chars() {
        if col >= end {
            break;
        }
        paint_char(buf, col, y, ch, style);
        col += 1;
    }
    col
}

fn map_key(key: &KeyEvent) -> Option<Key> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace | KeyCode::Delete => Some(Key::Backspace),
        KeyCode::BackTab => Some(Key::ShiftTab),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => Some(Key::ShiftTab),
        KeyCode::Tab => Some(Key::Tab),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Char(c) if ctrl => Some(Key::Ctrl(normalize_ctrl(c))),
        KeyCode::Char(c) => Some(Key::Char(c)),
        _ => None,
    }
}

fn normalize_ctrl(c: char) -> char {
    if c.is_ascii_control() {
        let letter = b'a' + c as u8 - 1;
        letter as char
    } else {
        c.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render(app: &App, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn col(row: &str, ch: char) -> usize {
        row.chars().position(|c| c == ch).expect("column")
    }

    fn lines(buffer: &Buffer) -> Vec<String> {
        let width = buffer.area.width as usize;
        let height = buffer.area.height as usize;
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x as u16, y as u16)].symbol())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn level_one_sits_above_the_dot() {
        let mut app = App::new();
        app.keys("1\n2\n3\n");
        let buffer = render(&app, 40, 12);
        let rows = lines(&buffer);
        let shown: Vec<&str> = rows.iter().map(|row| row.trim()).collect();
        let dot = shown
            .iter()
            .position(|row| *row == ".")
            .expect("dot marker");
        let top = shown.iter().position(|row| *row == "3").expect("top value");
        let oldest = shown
            .iter()
            .position(|row| *row == "1")
            .expect("oldest value");
        assert!(oldest < top);
        assert!(top < dot);
        assert!(shown.iter().any(|row| *row == "2"));
        let prompt = rows.iter().find(|row| row.contains('❯')).expect("prompt");
        let typed_at = col(prompt, '❯') + 2;
        assert_eq!(col(&rows[top], '3'), typed_at);
        assert_eq!(col(&rows[oldest], '1'), typed_at);
        assert_eq!(col(&rows[dot], '.'), typed_at);
        assert!(
            rows.iter()
                .any(|row| row.contains("prec 12") && row.contains("deg"))
        );
    }

    #[test]
    fn prompt_and_shortcuts_use_groknight_colors() {
        let mut app = App::new();
        app.keys("12");
        let buffer = render(&app, 60, 12);
        let rows = lines(&buffer);
        let prompt = rows
            .iter()
            .find(|row| row.contains('❯'))
            .expect("prompt row");
        assert!(prompt.contains("12"));

        let border_y = rows
            .iter()
            .position(|row| row.contains('╭'))
            .expect("prompt border");
        let border_x = col(&rows[border_y], '╭');
        assert_eq!(border_x, SIDE_PAD as usize);
        let right = col(&rows[border_y], '╮');
        assert_eq!(right, buffer.area.width as usize - SIDE_PAD as usize - 1);
        let border = &buffer[(border_x as u16, border_y as u16)];
        assert_eq!(border.fg, PROMPT_BORDER);
        assert_eq!(border.bg, BG);

        let prompt_y = rows
            .iter()
            .position(|row| row.contains('❯'))
            .expect("prompt row");
        let pipe = col(&rows[prompt_y], '│');
        let arrow = col(&rows[prompt_y], '❯');
        assert_eq!(arrow, pipe + 1 + PROMPT_INNER as usize);

        let bar = rows
            .iter()
            .position(|row| row.contains("ret:push"))
            .expect("shortcuts");
        assert!(rows[bar - 1].trim().is_empty(), "blank row above shortcuts");
        assert_eq!(bar + 1, rows.len() - 1, "one blank row under the shortcuts");
        assert!(rows[bar + 1].trim().is_empty());
        let key = &buffer[(SIDE_PAD, bar as u16)];
        assert_eq!(key.symbol(), "r");
        assert_eq!(key.fg, FG_SECONDARY);
        assert_eq!(key.bg, BG);
        assert!(key.modifier.contains(Modifier::BOLD));

        let label = rows[bar].find(":push").expect("label");
        let colon = &buffer[(label as u16, bar as u16)];
        assert_eq!(colon.symbol(), ":");
        assert_eq!(colon.fg, GRAY);
        assert!(!colon.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn error_and_help_are_visible() {
        let mut app = App::new();
        app.keys("+");
        let rows = lines(&render(&app, 40, 12));
        assert!(rows.iter().any(|row| row.contains("too few arguments")));
        let info = rows
            .iter()
            .position(|row| row.contains('╰'))
            .expect("prompt bottom");
        let corner = col(&rows[info], '╯');
        let info_chars: Vec<char> = rows[info].chars().collect();
        assert_eq!(info_chars[corner - 1], '─');
        assert_eq!(
            info_chars[corner - 2],
            ' ',
            "one blank cell left of the dash"
        );
        assert_eq!(info_chars[corner - 3], 's');

        let idle = lines(&render(&App::new(), 40, 12));
        let idle_y = idle
            .iter()
            .position(|row| row.contains('╰'))
            .expect("prompt bottom");
        let idle_corner = col(&idle[idle_y], '╯');
        let idle_chars: Vec<char> = idle[idle_y].chars().collect();
        assert_eq!(idle_chars[idle_corner - 1], '─');
        assert_eq!(idle_chars[idle_corner], '╯');

        app.keys("?");
        let buffer = render(&app, 50, 18);
        let rows = lines(&buffer);
        assert!(rows.iter().any(|row| row.contains("undo")));
        assert!(rows.iter().any(|row| row.contains("fraction")));
        assert!(rows.iter().any(|row| row.contains("Shortcuts")));
        assert!(rows.iter().any(|row| row.contains("esc close")));
        let frame_y = rows
            .iter()
            .position(|row| row.contains('┌'))
            .expect("help box");
        let corner = col(&rows[frame_y], '┌');
        let cell = &buffer[(corner as u16, frame_y as u16)];
        assert_eq!(cell.symbol(), "┌");
        assert_eq!(cell.fg, GRAY_DIM);
        assert_eq!(cell.bg, BG);
    }

    #[test]
    fn help_columns_match_in_length() {
        let columns = help_columns(4);
        let shortest = columns.iter().map(|column| column.len()).min().unwrap();
        let longest = columns.iter().map(|column| column.len()).max().unwrap();
        assert!(
            longest - shortest <= 1,
            "help columns should differ by at most one row"
        );
    }

    #[test]
    fn help_uses_four_columns_when_wide() {
        let mut app = App::new();
        app.keys("?");
        let rows = lines(&render(&app, 90, 24));
        assert!(
            rows.iter().any(|row| {
                row.contains("push")
                    && row.contains("add")
                    && row.contains("sin")
                    && row.contains("prec")
            }),
            "the four topic columns share a row"
        );
    }

    #[test]
    fn variable_list_is_two_columns() {
        let mut app = App::new();
        for name in 'a'..='j' {
            app.keys(&format!("1\nss{name}\n"));
        }
        app.keys("11\nssk\nsl");
        let rows = lines(&render(&app, 64, 20));
        assert!(
            rows.iter()
                .any(|row| row.contains("a:") && row.contains("f:")),
            "five names down each column"
        );
        assert!(rows.iter().any(|row| row.contains("Variables")));
        assert!(rows.iter().any(|row| row.contains("1/2")));
        assert!(rows.iter().any(|row| row.contains("tab:next")));
        assert!(rows.iter().all(|row| !row.contains("k:")));
        assert!(rows.iter().all(|row| !row.contains("ret:push")));

        app.press(Key::Tab);
        let rows = lines(&render(&app, 64, 20));
        assert!(rows.iter().any(|row| row.contains("k:")));
        assert!(rows.iter().any(|row| row.contains("2/2")));
        assert!(rows.iter().any(|row| row.contains("TAB:prev")));
        assert!(rows.iter().all(|row| !row.contains("a:")));

        let mut app = App::new();
        app.keys("2\n100^ssa\nsl");
        let rows = lines(&render(&app, 64, 20));
        assert!(
            rows.iter()
                .any(|row| row.contains("a:") && row.contains("..")),
            "a long value is cut"
        );

        let mut app = App::new();
        app.keys("sl");
        let rows = lines(&render(&app, 64, 20));
        assert!(rows.iter().any(|row| row.contains("nothing stored")));
    }
}
