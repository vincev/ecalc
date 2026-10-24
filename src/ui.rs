//! Full-screen stack, a rounded prompt, and a shortcut bar.
//!
//! Colors are the GrokNight palette from Grok Build: gray canvas, a focused
//! prompt border, bold secondary keys, and muted labels.

use crate::app::{App, CONST_COLUMN, Key, VAR_COLUMN};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::io;

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
            if wait_for_redraw(&mut app)? {
                break;
            }
        }
        Ok(())
    })
}

/// Blocks until a key or a resize. Idle time does not redraw the stack.
/// Returns true when the app should quit.
fn wait_for_redraw(app: &mut App) -> io::Result<bool> {
    loop {
        match event::read()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if let Some(mapped) = map_key(&key) {
                    app.press(mapped);
                }
                return Ok(app.should_quit());
            }
            Event::Resize(_, _) => return Ok(false),
            _ => {}
        }
    }
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
    if app.docs_open() {
        draw_docs(frame.buffer_mut(), area, app);
    }
    if app.vars_open() {
        draw_variables(frame.buffer_mut(), area, app);
    }
    if app.consts_open() {
        draw_constants(frame.buffer_mut(), area, app);
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
    let lines = stack_lines(app, area.height as usize, area.width as usize);
    Paragraph::new(lines).render(area, buf);
}

fn stack_lines(app: &App, rows: usize, width: usize) -> Vec<Line<'static>> {
    if rows == 0 {
        return Vec::new();
    }
    let room = rows - 1;
    let content_width = width.saturating_sub(STACK_INDENT);
    let wrapped: Vec<Vec<String>> = app
        .engine()
        .stack_lines()
        .iter()
        .map(|line| wrap_continued(line, content_width))
        .collect();
    let shown = fit_stack(&wrapped, room);
    let pad = room.saturating_sub(shown.len());
    let mut lines = Vec::with_capacity(rows);
    let blank = Style::default().bg(BG);
    for _ in 0..pad {
        lines.push(Line::from(Span::styled(" ", blank)));
    }
    let value = Style::default().fg(FG).bg(BG);
    let indent = " ".repeat(STACK_INDENT);
    for text in shown {
        lines.push(Line::from(Span::styled(format!("{indent}{text}"), value)));
    }
    lines.push(Line::from(Span::styled(
        format!("{indent}."),
        Style::default().fg(GRAY_DIM).bg(BG),
    )));
    lines
}

/// Break `text` so each piece fits in `width` columns.
///
/// A piece that continues ends with `\`. Later pieces start with one space, so
/// the continuation is obvious, and that space counts toward `width`. A width
/// of one has no room for the marker, so the text is one character per line.
fn wrap_continued(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return vec![text.to_string()];
    }
    if width == 1 {
        return chars.into_iter().map(|c| c.to_string()).collect();
    }
    let mut lines = Vec::new();
    let first = width - 1;
    let mut line: String = chars[..first].iter().collect();
    let mut start = first;
    if start < chars.len() {
        line.push('\\');
    }
    lines.push(line);
    while start < chars.len() {
        let remaining = chars.len() - start;
        // The indent uses one column. A line that still continues also keeps a
        // column for `\`. Width 2 can hold the indent and one character only.
        let (take, slash) = if width == 2 {
            (1, false)
        } else if remaining > width - 1 {
            (width - 2, true)
        } else {
            (remaining, false)
        };
        let end = start + take;
        let mut line = String::from(" ");
        line.extend(chars[start..end].iter().copied());
        start = end;
        if slash {
            line.push('\\');
        }
        lines.push(line);
    }
    lines
}

/// Keep the newest values that fit in `room` rows, oldest first.
///
/// A value is kept whole. The newest value may be clipped to its leading rows
/// when it alone is taller than the screen; those rows still end in `\` when
/// more digits remain.
fn fit_stack(wrapped: &[Vec<String>], room: usize) -> Vec<String> {
    let mut picked: Vec<&[String]> = Vec::new();
    let mut used = 0usize;
    for lines in wrapped.iter().rev() {
        if used >= room {
            break;
        }
        let left = room - used;
        if lines.len() <= left {
            picked.push(lines.as_slice());
            used += lines.len();
        } else if picked.is_empty() {
            picked.push(&lines[..left]);
            break;
        } else {
            break;
        }
    }
    picked.reverse();
    let mut shown = Vec::with_capacity(room);
    for lines in picked {
        shown.extend(lines.iter().cloned());
    }
    shown
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
        ("K", "const"),
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

/// Gray `┌┐` frame shared by the shortcuts, variable, and constant boxes.
struct Dialog {
    rect: Rect,
    inner_x: u16,
    text_w: u16,
    content_y: u16,
    content_h: u16,
}

impl Dialog {
    fn open(buf: &mut Buffer, area: Rect, content_rows: u16) -> Option<Self> {
        let chrome_y = 2 + HELP_V_PAD + HELP_FOOTER;
        let width = 68u16.min(area.width);
        let inner_w = width.saturating_sub(2 + HELP_H_PAD * 2);
        if inner_w < 8 || area.height < chrome_y {
            return None;
        }
        let height = (content_rows + chrome_y).min(area.height);
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

        Some(Self {
            rect,
            inner_x: rect.x + 1 + HELP_H_PAD,
            text_w: rect.width.saturating_sub(2 + HELP_H_PAD * 2),
            content_y: rect.y + 1 + HELP_V_PAD,
            content_h: rect.height.saturating_sub(chrome_y),
        })
    }

    fn title(&self, buf: &mut Buffer, title: &str) {
        let border = Style::default().fg(GRAY_DIM).bg(BG);
        let last_x = self.rect.x + self.rect.width - 1;
        let title_end = last_x.saturating_sub(8);
        let mut title_x = self.rect.x + 1;
        title_x = paint_str(buf, title_x, self.rect.y, title_end, "─ ", border);
        title_x = paint_str(
            buf,
            title_x,
            self.rect.y,
            title_end,
            title,
            Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD),
        );
        paint_str(buf, title_x, self.rect.y, title_end, " ─", border);

        let close = " [x] ";
        let close_x = self.rect.x + self.rect.width - close.chars().count() as u16 - 2;
        if close_x > title_x + 1 {
            paint_str(
                buf,
                close_x,
                self.rect.y,
                last_x,
                close,
                Style::default().fg(GRAY).bg(BG),
            );
        }
    }

    fn note(&self, buf: &mut Buffer, text: &str, style: Style) {
        paint_str(
            buf,
            self.inner_x,
            self.rect.y + 1,
            self.inner_x + self.text_w,
            text,
            style,
        );
    }

    /// Note on the left, `page/pages` on the right when there is another page.
    fn note_page(
        &self,
        buf: &mut Buffer,
        note: &str,
        note_style: Style,
        page: usize,
        pages: usize,
    ) {
        let page_text = if pages > 1 {
            format!("{}/{}", page + 1, pages)
        } else {
            String::new()
        };
        let page_w = page_text.chars().count() as u16;
        let page_x = self.inner_x + self.text_w.saturating_sub(page_w);
        let note_end = if page_w == 0 {
            self.inner_x + self.text_w
        } else {
            page_x.saturating_sub(1)
        };
        paint_str(
            buf,
            self.inner_x,
            self.rect.y + 1,
            note_end,
            note,
            note_style,
        );
        if page_w > 0 {
            paint_str(
                buf,
                page_x,
                self.rect.y + 1,
                self.inner_x + self.text_w,
                &page_text,
                Style::default().fg(GRAY).bg(BG),
            );
        }
    }

    fn footer(&self, buf: &mut Buffer, hints: &[(&str, &str)]) {
        let y = self.rect.y + self.rect.height - 2;
        draw_centered_hints(buf, self.inner_x, y, self.text_w, hints);
    }
}

/// Centered shortcuts box, in the shape of Grok Build's cheatsheet: gray frame,
/// bold title on the top border, and close hints centered inside the bottom.
fn draw_help(buf: &mut Buffer, area: Rect) {
    let width = 68u16.min(area.width);
    let inner_w = width.saturating_sub(2 + HELP_H_PAD * 2);
    if inner_w < 8 {
        return;
    }
    let (cols, col_w) = help_layout(inner_w);
    let columns = help_columns(cols);
    let grid_rows = columns.iter().map(|column| column.len()).max().unwrap_or(0) as u16;
    let Some(dialog) = Dialog::open(buf, area, grid_rows) else {
        return;
    };
    dialog.title(buf, "Shortcuts");
    dialog.note(buf, "? for a description", Style::default().fg(GRAY).bg(BG));
    if dialog.content_h > 0 {
        let content = Rect {
            x: dialog.inner_x,
            y: dialog.content_y,
            width: dialog.text_w,
            height: dialog.content_h,
        };
        draw_help_grid(buf, content, &columns, col_w, dialog.content_h);
    }
    dialog.footer(buf, HELP_HINTS);
}

const HELP_HINTS: &[(&str, &str)] = &[("?", "docs"), ("esc", "close"), ("q", "quit")];

const DOCS_HINTS: &[(&str, &str)] = &[
    ("↑↓", "move"),
    ("tab", "next"),
    ("esc", "close"),
    ("?", "list"),
];

/// Rows of command text inside the reference, before the dialog chrome.
const DOCS_CONTENT_ROWS: u16 = 16;

/// Searchable command pages. One match fills the box. Several matches keep a
/// short list and the selected page under it.
fn draw_docs(buf: &mut Buffer, area: Rect, app: &App) {
    let Some(dialog) = Dialog::open(buf, area, DOCS_CONTENT_ROWS) else {
        return;
    };
    dialog.title(buf, "Commands");
    let matches = app.docs_matches();
    let cursor = app.docs_cursor();
    draw_docs_query(&dialog, buf, app.docs_query(), cursor, matches.len());
    let split = docs_split(dialog.content_h, matches.len());
    let mut y = dialog.content_y;
    if split.list > 0 {
        let start = docs_window(cursor, matches.len(), split.list as usize);
        for row in 0..split.list as usize {
            let Some(entry) = matches.get(start + row) else {
                break;
            };
            draw_docs_row(
                buf,
                dialog.inner_x,
                y,
                dialog.inner_x + dialog.text_w,
                entry,
                start + row == cursor,
            );
            y += 1;
        }
    }
    if split.rule {
        paint_rule(buf, dialog.inner_x, y, dialog.text_w);
        y += 1;
    }
    if split.article > 0 {
        if let Some(entry) = matches.get(cursor) {
            draw_docs_article(
                buf,
                dialog.inner_x,
                y,
                dialog.inner_x + dialog.text_w,
                split.article,
                entry,
            );
        } else {
            paint_str(
                buf,
                dialog.inner_x,
                y,
                dialog.inner_x + dialog.text_w,
                "no match",
                Style::default().fg(GRAY).bg(BG),
            );
        }
    }
    dialog.footer(buf, DOCS_HINTS);
}

struct DocsSplit {
    list: u16,
    rule: bool,
    article: u16,
}

fn docs_split(content_h: u16, matches: usize) -> DocsSplit {
    if matches <= 1 || content_h < 4 {
        return DocsSplit {
            list: 0,
            rule: false,
            article: content_h,
        };
    }
    let list = (matches as u16).min(6).min(content_h - 4).max(1);
    DocsSplit {
        list,
        rule: true,
        article: content_h - list - 1,
    }
}

fn docs_window(cursor: usize, matches: usize, rows: usize) -> usize {
    if matches <= rows {
        return 0;
    }
    cursor.saturating_sub(rows / 2).min(matches - rows)
}

fn draw_docs_query(dialog: &Dialog, buf: &mut Buffer, query: &str, cursor: usize, total: usize) {
    let y = dialog.rect.y + 1;
    let end = dialog.inner_x + dialog.text_w;
    let page = if total == 0 {
        String::new()
    } else {
        format!("{}/{}", cursor + 1, total)
    };
    let page_w = page.chars().count() as u16;
    let text_end = if page_w == 0 {
        end
    } else {
        end.saturating_sub(page_w + 1)
    };
    let mut col = paint_str(buf, dialog.inner_x, y, text_end, "❯ ", arrow_style());
    if query.is_empty() {
        paint_str(
            buf,
            col,
            y,
            text_end,
            "type to search",
            Style::default().fg(GRAY).bg(BG),
        );
    } else {
        col = paint_str(buf, col, y, text_end, query, text_style());
        if col < text_end {
            paint_char(buf, col, y, '▍', arrow_style());
        }
    }
    if page_w > 0 {
        paint_str(
            buf,
            end - page_w,
            y,
            end,
            &page,
            Style::default().fg(GRAY).bg(BG),
        );
    }
}

fn draw_docs_row(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    end: u16,
    entry: &crate::docs::Entry,
    selected: bool,
) {
    let key_style = Style::default()
        .fg(if selected { FG } else { FG_SECONDARY })
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let name_style = if selected {
        Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(GRAY).bg(BG)
    };
    let summary_style = Style::default().fg(if selected { FG } else { GRAY }).bg(BG);
    let mut col = paint_str(
        buf,
        x,
        y,
        end,
        if selected { "❯ " } else { "  " },
        if selected {
            arrow_style()
        } else {
            Style::default().bg(BG)
        },
    );
    col = paint_field(buf, col, y, end, entry.keys, 4, key_style);
    col = paint_gap(buf, col, y, end, 2);
    col = paint_field(buf, col, y, end, entry.name, 10, name_style);
    col = paint_gap(buf, col, y, end, 2);
    paint_str(buf, col, y, end, entry.summary, summary_style);
}

fn paint_field(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    end: u16,
    text: &str,
    width: u16,
    style: Style,
) -> u16 {
    let field_end = x.saturating_add(width).min(end);
    let mut col = paint_str(buf, x, y, field_end, text, style);
    while col < field_end {
        paint_char(buf, col, y, ' ', Style::default().bg(BG));
        col += 1;
    }
    col
}

fn paint_gap(buf: &mut Buffer, x: u16, y: u16, end: u16, width: u16) -> u16 {
    let gap_end = x.saturating_add(width).min(end);
    let mut col = x;
    while col < gap_end {
        paint_char(buf, col, y, ' ', Style::default().bg(BG));
        col += 1;
    }
    col
}

fn paint_rule(buf: &mut Buffer, x: u16, y: u16, width: u16) {
    let style = Style::default().fg(GRAY_DIM).bg(BG);
    for col in x..x.saturating_add(width) {
        paint_char(buf, col, y, '─', style);
    }
}

fn draw_docs_article(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    end: u16,
    rows: u16,
    entry: &crate::docs::Entry,
) {
    if rows == 0 || end <= x {
        return;
    }
    let head = format!("{}  {}", entry.keys, entry.name);
    paint_str(
        buf,
        x,
        y,
        end,
        &head,
        Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD),
    );
    if rows == 1 {
        return;
    }
    paint_str(
        buf,
        x,
        y + 1,
        end,
        entry.summary,
        Style::default().fg(GRAY).bg(BG),
    );
    if rows <= 2 {
        return;
    }
    let gap = u16::from(rows >= 5);
    let body_y = y + 2 + gap;
    let body_rows = rows - 2 - gap;
    let width = (end - x) as usize;
    for (row, line) in wrap_words(entry.body, width)
        .into_iter()
        .take(body_rows as usize)
        .enumerate()
    {
        paint_str(
            buf,
            x,
            body_y + row as u16,
            end,
            &line,
            Style::default().fg(FG).bg(BG),
        );
    }
}

fn wrap_words(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if word.chars().count() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for ch in word.chars() {
                if line.chars().count() >= width {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(ch);
            }
            continue;
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
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
    let Some(dialog) = Dialog::open(buf, area, grid_rows) else {
        return;
    };
    dialog.title(buf, "Variables");
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
    dialog.note_page(buf, note, note_style, app.var_page(), app.var_pages());

    if rows.is_empty() {
        paint_str(
            buf,
            dialog.inner_x,
            dialog.content_y,
            dialog.inner_x + dialog.text_w,
            "nothing stored",
            Style::default().fg(GRAY).bg(BG),
        );
    } else if dialog.content_h > 0 {
        let col_w = (dialog.text_w as usize - HELP_GAP) / 2;
        for (index, (name, value)) in rows.iter().enumerate() {
            let col = index / VAR_COLUMN;
            let row = index % VAR_COLUMN;
            if col > 1 || row >= dialog.content_h as usize || col_w == 0 {
                continue;
            }
            let x = dialog.inner_x + (col * (col_w + HELP_GAP)) as u16;
            paint_var_cell(
                buf,
                x,
                dialog.content_y + row as u16,
                x + col_w as u16,
                name,
                value,
            );
        }
    }

    let hints = var_footer_hints(app);
    dialog.footer(buf, &hints);
}

/// Physics constants, sixteen to a page: eight down each column.
fn draw_constants(buf: &mut Buffer, area: Rect, app: &App) {
    let rows = app.const_rows();
    let left = rows.len().min(CONST_COLUMN);
    let right = rows.len().saturating_sub(CONST_COLUMN);
    let grid_rows = left.max(right) as u16;
    if grid_rows == 0 {
        return;
    }
    let Some(dialog) = Dialog::open(buf, area, grid_rows) else {
        return;
    };
    dialog.title(buf, "Constants");
    let note = if app.const_note().is_empty() {
        "0-F inserts"
    } else {
        app.const_note()
    };
    let note_style = if app.const_note().is_empty() {
        Style::default().fg(GRAY).bg(BG)
    } else {
        Style::default().fg(ERROR).bg(BG)
    };
    dialog.note_page(buf, note, note_style, app.const_page(), app.const_pages());

    if dialog.content_h > 0 {
        let col_w = (dialog.text_w as usize - HELP_GAP) / 2;
        for (index, (key, symbol, name)) in rows.iter().enumerate() {
            let col = index / CONST_COLUMN;
            let row = index % CONST_COLUMN;
            if col > 1 || row >= dialog.content_h as usize || col_w == 0 {
                continue;
            }
            let x = dialog.inner_x + (col * (col_w + HELP_GAP)) as u16;
            paint_const_cell(
                buf,
                x,
                dialog.content_y + row as u16,
                x + col_w as u16,
                *key,
                symbol,
                name,
            );
        }
    }

    let hints = const_footer_hints(app);
    dialog.footer(buf, &hints);
}

fn var_footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    page_hints(app.var_pages())
}

fn const_footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    page_hints(app.const_pages())
}

fn page_hints(pages: usize) -> Vec<(&'static str, &'static str)> {
    let mut hints = Vec::new();
    if pages > 1 {
        hints.push(("tab", "next"));
        hints.push(("TAB", "prev"));
    }
    hints.push(("esc", "close"));
    hints
}

/// `0 c     light`, cutting the name with `..` when the cell is too narrow.
fn paint_const_cell(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    end: u16,
    key: char,
    symbol: &str,
    name: &str,
) {
    if end <= x {
        return;
    }
    let key_style = Style::default()
        .fg(FG_SECONDARY)
        .bg(BG)
        .add_modifier(Modifier::BOLD);
    let symbol_style = Style::default().fg(FG).bg(BG).add_modifier(Modifier::BOLD);
    let name_style = Style::default().fg(FG).bg(BG);
    let punct = Style::default().fg(GRAY).bg(BG);
    let mut col = paint_str(buf, x, y, end, &key.to_string(), key_style);
    if col >= end {
        return;
    }
    col = paint_str(buf, col, y, end, " ", punct);
    col = paint_str(buf, col, y, end, symbol, symbol_style);
    let gap = 5usize.saturating_sub(symbol.chars().count()) + 1;
    col = paint_str(buf, col, y, end, &" ".repeat(gap), punct);
    if col >= end {
        return;
    }
    let room = (end - col) as usize;
    if name.chars().count() <= room {
        paint_str(buf, col, y, end, name, name_style);
        return;
    }
    let keep = room.saturating_sub(2);
    let head: String = name.chars().take(keep).collect();
    col = paint_str(buf, col, y, end, &head, name_style);
    paint_str(buf, col, y, end, "..", punct);
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
    let const_hints = const_footer_hints(app);
    let hints: &[(&str, &str)] = if app.docs_open() {
        DOCS_HINTS
    } else if app.help_open() {
        HELP_HINTS
    } else if app.vars_open() {
        &var_hints
    } else if app.consts_open() {
        &const_hints
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
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
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

    fn row_text(row: &str, from: usize) -> String {
        row.chars()
            .skip(from)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    /// A stack row has a digit somewhere in the value columns. A continuation
    /// is a space in the first of those columns, so a blank pad row is not one.
    fn value_row(row: &str, from: usize, last: usize) -> bool {
        row.chars()
            .skip(from)
            .take(last.saturating_sub(from) + 1)
            .any(|c| c != ' ')
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        lines(&render(app, width, height))
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
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
        assert!(shown.contains(&"2"));
        let prompt = rows.iter().find(|row| row.contains('❯')).expect("prompt");
        let typed_at = col(prompt, '❯') + 2;
        assert_eq!(col(&rows[top], '3'), typed_at);
        assert_eq!(col(&rows[oldest], '1'), typed_at);
        assert_eq!(col(&rows[dot], '.'), typed_at);
        assert!(!rows.iter().any(|row| row.contains('\\')));
        assert!(
            rows.iter()
                .any(|row| row.contains("prec 12") && row.contains("deg"))
        );
    }

    #[test]
    fn wrap_continued_breaks_on_the_backslash() {
        assert_eq!(wrap_continued("12345", 5), ["12345"]);
        assert_eq!(wrap_continued("123456", 5), ["1234\\", " 56"]);
        assert_eq!(
            wrap_continued("-1234567890", 5),
            ["-123\\", " 456\\", " 7890"]
        );
        assert_eq!(wrap_continued("ab", 1), ["a", "b"]);
        assert_eq!(wrap_continued("ab", 0), [""]);
    }

    #[test]
    fn fit_stack_keeps_the_newest_values() {
        let wrapped = vec![
            vec!["1".to_string()],
            vec!["22\\".to_string(), "2".to_string()],
            vec!["3".to_string()],
        ];
        assert_eq!(
            fit_stack(&wrapped, 3),
            vec!["22\\".to_string(), "2".to_string(), "3".to_string()]
        );
        let huge = vec!["aaaa\\".to_string(), "bbbb\\".to_string(), "cc".to_string()];
        assert_eq!(
            fit_stack(&[huge.clone(), vec!["9".to_string()]], 2),
            vec!["9".to_string()]
        );
        assert_eq!(
            fit_stack(&[huge], 2),
            vec!["aaaa\\".to_string(), "bbbb\\".to_string()]
        );
    }

    #[test]
    fn long_integer_wraps_inside_the_stack() {
        let mut app = App::new();
        app.keys("2\n100\n!");
        let full = app.engine().stack_lines().pop().expect("factorial");
        assert!(full.len() > 40, "100! should be wider than the test screen");

        let buffer = render(&app, 40, 24);
        let rows = lines(&buffer);
        let prompt = rows.iter().find(|row| row.contains('❯')).expect("prompt");
        let typed_at = col(prompt, '❯') + 2;
        let dot = rows.iter().position(|row| row.trim() == ".").expect("dot");
        let inset_last = buffer.area.width as usize - SIDE_PAD as usize - 1;
        let mut shown = Vec::new();
        for row in &rows[..dot] {
            if value_row(row, typed_at, inset_last) {
                shown.push(row.as_str());
            }
        }
        assert!(shown.len() >= 2, "the 2 and the factorial should both fit");
        assert_eq!(shown[0].chars().nth(typed_at), Some('2'));
        assert!(!shown[0].contains('\\'));

        let mut joined = String::new();
        for (i, row) in shown[1..].iter().enumerate() {
            let content = row_text(row, typed_at);
            let last = i + 1 == shown.len() - 1;
            let body = content.strip_suffix('\\').unwrap_or(&content);
            if i == 0 {
                assert_eq!(
                    row.chars().nth(typed_at),
                    Some(full.chars().next().unwrap())
                );
                joined.push_str(body);
            } else {
                assert_eq!(row.chars().nth(typed_at), Some(' '));
                assert_ne!(row.chars().nth(typed_at + 1), Some(' '));
                let digits = body.strip_prefix(' ').expect("continuation indent");
                joined.push_str(digits);
            }
            if last {
                assert!(!content.ends_with('\\'));
            } else {
                assert!(content.ends_with('\\'));
                assert_eq!(row.chars().nth(inset_last), Some('\\'));
                assert_eq!(row.chars().nth(inset_last + 1), Some(' '));
            }
        }
        assert_eq!(joined, full);

        let clipped = lines(&render(&app, 40, 12));
        let clip_dot = clipped
            .iter()
            .position(|row| row.trim() == ".")
            .expect("dot");
        let mut head = String::new();
        let mut pieces = 0;
        for row in &clipped[..clip_dot] {
            if !value_row(row, typed_at, inset_last) {
                continue;
            }
            let content = row_text(row, typed_at);
            assert!(
                content.ends_with('\\'),
                "a clipped factorial still ends in \\"
            );
            let body = content.strip_suffix('\\').unwrap();
            let digits = body.strip_prefix(' ').unwrap_or(body);
            head.push_str(digits);
            pieces += 1;
        }
        assert!(pieces > 1);
        assert!(full.starts_with(&head));
        assert_ne!(head, full);
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
    fn every_shortcut_has_a_page() {
        for group in HELP_GROUPS {
            for (key, label) in group {
                assert!(
                    crate::docs::entries()
                        .iter()
                        .any(|entry| entry.keys == *key && entry.name == *label),
                    "{key} {label} is missing from the command reference"
                );
            }
        }
    }

    #[test]
    fn command_reference_searches() {
        let mut app = App::new();
        app.keys("??");
        let text = screen(&app, 80, 30);
        assert!(text.contains("Commands"));
        assert!(text.contains("type to search"));
        assert!(text.contains("duplicates the top"));
        assert!(!app.should_quit());

        app.press(Key::Down);
        let text = screen(&app, 80, 30);
        assert!(text.contains("erases the last typed character"));
        app.press(Key::Up);
        assert_eq!(app.docs_cursor(), 0);

        app.keys("isqrt");
        assert!(!app.should_quit());
        assert_eq!(app.docs_query(), "isqrt");
        let text = screen(&app, 80, 30);
        assert!(text.contains("integer square root"));
        assert!(text.contains("10 f Q is 3"));
        assert!(text.contains("1/1"));
        assert!(!text.contains("type to search"));
        assert!(!text.contains("Shortcuts"));

        app.keys("\u{7f}\u{7f}\u{7f}\u{7f}\u{7f}zzz");
        let text = screen(&app, 80, 30);
        assert!(text.contains("no match"));

        app.press(Key::Esc);
        assert!(!app.docs_open());
        assert!(!app.help_open());

        app.keys("???");
        assert!(app.help_open());
        assert!(!app.docs_open());
        let text = screen(&app, 80, 30);
        assert!(text.contains("Shortcuts"));
        assert!(text.contains("? for a description"));
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
        assert!(rows.iter().any(|row| row.contains("TAB:prev")));
        assert!(rows.iter().all(|row| !row.contains("k:")));
        assert!(rows.iter().all(|row| !row.contains("ret:push")));

        app.press(Key::Tab);
        let rows = lines(&render(&app, 64, 20));
        assert!(rows.iter().any(|row| row.contains("k:")));
        assert!(rows.iter().any(|row| row.contains("2/2")));
        assert!(rows.iter().any(|row| row.contains("tab:next")));
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
        assert!(rows.iter().all(|row| !row.contains("tab:next")));
    }

    #[test]
    fn constants_list_is_two_columns() {
        let mut app = App::new();
        app.keys("K");
        let rows = lines(&render(&app, 64, 24));
        assert!(rows.iter().any(|row| row.contains("Constants")));
        assert!(rows.iter().any(|row| row.contains("1/3")));
        assert!(rows.iter().any(|row| row.contains("0-F inserts")));
        assert!(
            rows.iter()
                .any(|row| row.contains("c") && row.contains("light") && row.contains("mp")),
            "eight names down each column"
        );
        assert!(rows.iter().any(|row| row.contains("tab:next")));
        assert!(rows.iter().any(|row| row.contains("TAB:prev")));
        assert!(rows.iter().all(|row| !row.contains("ret:push")));
        assert!(rows.iter().all(|row| !row.contains("eps0")));

        app.press(Key::Tab);
        app.press(Key::Tab);
        let rows = lines(&render(&app, 64, 24));
        assert!(rows.iter().any(|row| row.contains("3/3")));
        assert!(rows.iter().any(|row| row.contains("tab:next")));
        assert!(rows.iter().any(|row| row.contains("TAB:prev")));
        assert!(
            rows.iter()
                .any(|row| row.contains("alpha") && row.contains("fine") && row.contains("Me")),
            "the last page keeps alpha beside Earth mass"
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("AU") && row.contains("astronomical"))
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("Mm") && row.contains("Moon"))
        );
        assert!(
            rows.iter()
                .any(|row| row.contains("dm") && row.contains("distance"))
        );
        assert!(rows.iter().all(|row| !row.contains("light")));
    }
}
