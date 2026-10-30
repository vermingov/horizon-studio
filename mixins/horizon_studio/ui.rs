// Horizon Studio's screen: Console (a Lua executor), Editor (a code editor
// with highlighting, undo and the clipboard), Library (the script mods, their
// state and errors, and the bundled examples) and Docs (the API). It draws
// with the game's own GUI font and rectangles through GameApi, in Horizon's
// dark design tokens (docs/design-system.md): a veil over the world, plates
// for the console and code, 1 px rules, one accent for the main action.

use super::hstudio::{API, CONSOLE_API, Font, GameApi, LogKind, Owner, Studio};
use super::hstudio_examples::EXAMPLES;
use super::hstudio_lua::{TableRef, Value};
use std::path::{Path, PathBuf};

const UNDO_LIMIT: usize = 300;
const INDENT: &str = "  ";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Console,
    Editor,
    Library,
    Docs,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Console, "Console"),
    (Tab::Editor, "Editor"),
    (Tab::Library, "Library"),
    (Tab::Docs, "Docs"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    /// Control, or Command on macOS.
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Escape,
    /// The key that opens the studio (F8 unless rebound).
    Toggle,
    Enter,
    Backspace,
    Delete,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// A letter or digit key, lowercase, for shortcuts.
    Char(char),
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Key {
        key: Key,
        down: bool,
        repeat: bool,
        mods: Mods,
    },
    /// Typed text (no control characters).
    Text(String),
    /// The pointer, in GUI pixels.
    Mouse(f32, f32),
    Button {
        left: bool,
        down: bool,
    },
    /// Wheel steps; positive scrolls up.
    Wheel(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiResult {
    Used,
    /// The studio closes.
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Action {
    Tab(Tab),
    Select(usize),
    Toggle(usize),
    Edit(usize),
    Reload(usize),
    NewMod,
    ReloadAll,
    Install(usize),
    Close,
    Save,
    Run,
    OpenFolder,
}

/// Where the editor's text was drawn, for the pointer.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EditorView {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) rows: usize,
}

#[derive(Default)]
pub struct Ui {
    pub open: bool,
    /// `/studio` asked to open it.
    pub open_request: bool,
    pub tab: Tab,
    pub library_sel: usize,
    pub input: LineEdit,
    /// Where Up/Down are in the console's history.
    pub hist: Option<usize>,
    pub log_scroll: usize,
    pub docs_scroll: usize,
    /// The New mod prompt, while it is open.
    pub naming: Option<LineEdit>,
    /// The pointer, in dp.
    pub mouse: (f32, f32),
    /// Whether the left and right buttons are held (over a script screen).
    pub buttons: [bool; 2],
    dragging: bool,
    pub(crate) hits: Vec<([f32; 4], Action)>,
    pub(crate) view: Option<EditorView>,
    /// Lines the last frame could show, for Page Up/Down.
    pub(crate) page: usize,
    /// dp per GUI pixel, from the last frame drawn (the pointer arrives in GUI pixels).
    pub dp: f32,
    /// Motion: when the screen opened and began to close, when the tab
    /// changed, the hover and switch fades by id, the smooth scrolls.
    pub opened_at: f64,
    pub closing_at: Option<f64>,
    pub(crate) last_draw: f64,
    pub(crate) tab_at: f64,
    pub(crate) tab_drawn: Option<Tab>,
    pub(crate) pill: [f32; 2],
    pub(crate) pill_from: [f32; 2],
    pub(crate) anim: std::collections::HashMap<u64, f32>,
    pub(crate) smooth: [f32; 3],
    pub(crate) naming_at: f64,
}

impl Ui {
    /// dp per GUI pixel (1 before anything was drawn).
    pub fn dp_factor(&self) -> f32 {
        if self.dp > 0.0 { self.dp } else { 1.0 }
    }
}

// ----------------------------------------------------------------- a line of text being typed

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineEdit {
    pub text: String,
    /// A byte index on a char boundary.
    pub cursor: usize,
}

impl LineEdit {
    pub fn set(&mut self, text: &str) {
        self.text = text.to_owned();
        self.cursor = self.text.len();
    }
    pub fn insert(&mut self, s: &str) {
        let clean: String = s
            .chars()
            .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
            .filter(|c| !c.is_control())
            .collect();
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    pub fn left(&mut self, word: bool) {
        self.cursor = if word {
            word_left(&self.text, self.cursor)
        } else {
            prev_char(&self.text, self.cursor)
        };
    }
    pub fn right(&mut self, word: bool) {
        self.cursor = if word {
            word_right(&self.text, self.cursor)
        } else {
            next_char(&self.text, self.cursor)
        };
    }
    pub fn backspace(&mut self, word: bool) {
        let to = if word {
            word_left(&self.text, self.cursor)
        } else {
            prev_char(&self.text, self.cursor)
        };
        self.text.replace_range(to..self.cursor, "");
        self.cursor = to;
    }
    pub fn delete(&mut self, word: bool) {
        let to = if word {
            word_right(&self.text, self.cursor)
        } else {
            next_char(&self.text, self.cursor)
        };
        self.text.replace_range(self.cursor..to, "");
    }
    /// Handles an editing key; false when it isn't one.
    fn key(&mut self, key: Key, mods: Mods) -> bool {
        match key {
            Key::Left => self.left(mods.ctrl),
            Key::Right => self.right(mods.ctrl),
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.text.len(),
            Key::Backspace => self.backspace(mods.ctrl),
            Key::Delete => self.delete(mods.ctrl),
            _ => return false,
        }
        true
    }
}

fn prev_char(s: &str, i: usize) -> usize {
    s[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

fn next_char(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_left(s: &str, i: usize) -> usize {
    let mut j = i;
    while j > 0 && !is_word(s[..j].chars().next_back().unwrap_or(' ')) {
        j = prev_char(s, j);
    }
    while j > 0 && is_word(s[..j].chars().next_back().unwrap_or(' ')) {
        j = prev_char(s, j);
    }
    j
}

fn word_right(s: &str, i: usize) -> usize {
    let mut j = i;
    while j < s.len() && !is_word(s[j..].chars().next().unwrap_or(' ')) {
        j = next_char(s, j);
    }
    while j < s.len() && is_word(s[j..].chars().next().unwrap_or(' ')) {
        j = next_char(s, j);
    }
    j
}

// ----------------------------------------------------------------- the editor's buffer

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Other,
}

pub struct Editor {
    pub path: Option<PathBuf>,
    pub title: String,
    pub lines: Vec<String>,
    pub row: usize,
    /// A byte index in `lines[row]`, on a char boundary.
    pub col: usize,
    /// The other end of the selection.
    pub anchor: Option<(usize, usize)>,
    pub scroll: usize,
    /// Chars of each line scrolled off to the left.
    pub hscroll: usize,
    pub dirty: bool,
    /// The line of the last error in this file, 1-based.
    pub error_line: Option<usize>,
    /// The column Up and Down keep, in chars.
    goal: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_kind: Option<EditKind>,
    /// Why the file could not be read, if it couldn't.
    pub problem: Option<String>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor {
            path: None,
            title: "untitled".into(),
            lines: vec![String::new()],
            row: 0,
            col: 0,
            anchor: None,
            scroll: 0,
            hscroll: 0,
            dirty: false,
            error_line: None,
            goal: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_kind: None,
            problem: None,
        }
    }
}

impl Editor {
    pub fn open(&mut self, path: &Path, title: &str) {
        let (text, problem) = match std::fs::read_to_string(path) {
            Ok(t) => (t, None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (String::new(), None),
            Err(e) => (String::new(), Some(e.to_string())),
        };
        *self = Editor {
            path: Some(path.to_path_buf()),
            title: title.to_owned(),
            problem,
            ..Editor::default()
        };
        self.set_text(&text);
    }
    pub fn set_text(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\t', INDENT);
        self.lines = text.split('\n').map(str::to_owned).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.row.min(self.lines.len() - 1);
        self.col = self.col.min(self.lines[self.row].len());
        while !self.lines[self.row].is_char_boundary(self.col) {
            self.col -= 1;
        }
    }
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
    pub fn save(&mut self) -> Result<PathBuf, String> {
        let path = self.path.clone().ok_or("this buffer has no file")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut text = self.text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        self.dirty = false;
        self.problem = None;
        Ok(path)
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            row: self.row,
            col: self.col,
        }
    }
    /// Before an edit: an undo step, merged with the last while typing.
    fn begin(&mut self, kind: EditKind) {
        if kind == EditKind::Other || self.last_kind != Some(kind) {
            self.undo.push(self.snapshot());
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.last_kind = Some(kind);
        self.redo.clear();
        self.dirty = true;
        self.goal = None;
    }
    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(s);
        }
    }
    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(s);
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.lines = s.lines;
        self.row = s.row.min(self.lines.len() - 1);
        self.col = s.col.min(self.lines[self.row].len());
        self.anchor = None;
        self.last_kind = None;
        self.dirty = true;
    }
    /// The selection, start first.
    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.anchor?;
        let b = (self.row, self.col);
        if a == b {
            return None;
        }
        Some(if a < b { (a, b) } else { (b, a) })
    }
    pub fn selected_text(&self) -> Option<String> {
        let ((r1, c1), (r2, c2)) = self.selection()?;
        if r1 == r2 {
            return Some(self.lines[r1][c1..c2].to_owned());
        }
        let mut out = self.lines[r1][c1..].to_owned();
        for r in r1 + 1..r2 {
            out.push('\n');
            out.push_str(&self.lines[r]);
        }
        out.push('\n');
        out.push_str(&self.lines[r2][..c2]);
        Some(out)
    }
    fn delete_selection(&mut self) -> bool {
        let Some(((r1, c1), (r2, c2))) = self.selection() else {
            self.anchor = None;
            return false;
        };
        let tail = self.lines[r2][c2..].to_owned();
        self.lines[r1].truncate(c1);
        self.lines[r1].push_str(&tail);
        self.lines.drain(r1 + 1..=r2);
        self.row = r1;
        self.col = c1;
        self.anchor = None;
        true
    }
    /// Types or pastes `s` at the cursor, over the selection.
    pub fn insert(&mut self, s: &str) {
        let kind = if s.len() <= 1 && s != " " && s != "\n" {
            EditKind::Typing
        } else {
            EditKind::Other
        };
        self.begin(kind);
        self.delete_selection();
        let s = s.replace("\r\n", "\n").replace('\t', INDENT);
        let tail = self.lines[self.row].split_off(self.col);
        let mut pieces = s.split('\n');
        if let Some(first) = pieces.next() {
            self.lines[self.row].push_str(first);
        }
        for p in pieces {
            self.row += 1;
            self.lines.insert(self.row, p.to_owned());
        }
        self.col = self.lines[self.row].len();
        self.lines[self.row].push_str(&tail);
    }
    /// Enter: a new line, indented as this one, one level more after a
    /// line that opens a block.
    pub fn newline(&mut self) {
        let line = &self.lines[self.row];
        let indent: String = line.chars().take_while(|c| *c == ' ').collect();
        let before = line[..self.col].trim_end();
        let opens = ["then", "do", "else", "repeat", "{", "("]
            .iter()
            .any(|k| before.ends_with(k))
            || (before.contains("function") && before.ends_with(')'))
            || before.ends_with("function()");
        let extra = if opens { INDENT } else { "" };
        self.insert(&format!("\n{indent}{extra}"));
    }
    pub fn backspace(&mut self, word: bool) {
        if self.selection().is_some() {
            self.begin(EditKind::Other);
            self.delete_selection();
            return;
        }
        self.anchor = None;
        if self.col == 0 {
            if self.row == 0 {
                return;
            }
            self.begin(EditKind::Other);
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].push_str(&line);
            return;
        }
        self.begin(EditKind::Deleting);
        let line = &self.lines[self.row];
        let to = if word {
            word_left(line, self.col)
        } else if line[..self.col].ends_with(INDENT) && line[..self.col].trim().is_empty() {
            self.col - INDENT.len()
        } else {
            prev_char(line, self.col)
        };
        self.lines[self.row].replace_range(to..self.col, "");
        self.col = to;
    }
    pub fn delete(&mut self, word: bool) {
        if self.selection().is_some() {
            self.begin(EditKind::Other);
            self.delete_selection();
            return;
        }
        self.anchor = None;
        if self.col == self.lines[self.row].len() {
            if self.row + 1 < self.lines.len() {
                self.begin(EditKind::Other);
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].push_str(&next);
            }
            return;
        }
        self.begin(EditKind::Deleting);
        let line = &self.lines[self.row];
        let to = if word {
            word_right(line, self.col)
        } else {
            next_char(line, self.col)
        };
        self.lines[self.row].replace_range(self.col..to, "");
    }
    fn select_from_here(&mut self, select: bool) {
        if select {
            if self.anchor.is_none() {
                self.anchor = Some((self.row, self.col));
            }
        } else {
            self.anchor = None;
        }
        self.last_kind = None;
    }
    fn char_col(&self) -> usize {
        self.lines[self.row][..self.col].chars().count()
    }
    fn set_char_col(&mut self, n: usize) {
        let line = &self.lines[self.row];
        self.col = line.char_indices().nth(n).map_or(line.len(), |(i, _)| i);
    }
    pub fn move_h(&mut self, right: bool, word: bool, select: bool) {
        if !select && let Some((a, b)) = self.selection() {
            (self.row, self.col) = if right { b } else { a };
            self.anchor = None;
            return;
        }
        self.select_from_here(select);
        self.goal = None;
        let line = &self.lines[self.row];
        if right {
            if self.col == line.len() {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            } else {
                self.col = if word {
                    word_right(line, self.col)
                } else {
                    next_char(line, self.col)
                };
            }
        } else if self.col == 0 {
            if self.row > 0 {
                self.row -= 1;
                self.col = self.lines[self.row].len();
            }
        } else {
            self.col = if word {
                word_left(line, self.col)
            } else {
                prev_char(line, self.col)
            };
        }
    }
    pub fn move_v(&mut self, delta: isize, select: bool) {
        self.select_from_here(select);
        let goal = *self
            .goal
            .get_or_insert_with(|| self.lines[self.row][..self.col].chars().count());
        let row = (self.row as isize + delta).clamp(0, self.lines.len() as isize - 1) as usize;
        if row == self.row {
            self.col = if delta < 0 { 0 } else { self.lines[row].len() };
            return;
        }
        self.row = row;
        self.set_char_col(goal);
    }
    /// Home goes to the first non-space, then to the start.
    pub fn home(&mut self, select: bool) {
        self.select_from_here(select);
        self.goal = None;
        let first = self.lines[self.row].len() - self.lines[self.row].trim_start().len();
        self.col = if self.col == first { 0 } else { first };
    }
    pub fn end(&mut self, select: bool) {
        self.select_from_here(select);
        self.goal = None;
        self.col = self.lines[self.row].len();
    }
    pub fn select_all(&mut self) {
        self.anchor = Some((0, 0));
        self.row = self.lines.len() - 1;
        self.col = self.lines[self.row].len();
    }
    /// The rows the selection covers, or the cursor's.
    fn rows(&self) -> (usize, usize) {
        match self.selection() {
            Some(((r1, _), (r2, c2))) => (r1, if c2 == 0 && r2 > r1 { r2 - 1 } else { r2 }),
            None => (self.row, self.row),
        }
    }
    pub fn indent(&mut self, out: bool) {
        let (r1, r2) = self.rows();
        if !out && self.selection().is_none() {
            self.insert(INDENT);
            return;
        }
        self.begin(EditKind::Other);
        for r in r1..=r2 {
            let line = &mut self.lines[r];
            if out {
                let n = line.len() - line.trim_start_matches(' ').len();
                let cut = n.min(INDENT.len());
                line.replace_range(..cut, "");
                if r == self.row {
                    self.col = self.col.saturating_sub(cut);
                }
                if let Some(a) = &mut self.anchor
                    && a.0 == r
                {
                    a.1 = a.1.saturating_sub(cut);
                }
            } else {
                line.insert_str(0, INDENT);
                if r == self.row {
                    self.col += INDENT.len();
                }
                if let Some(a) = &mut self.anchor
                    && a.0 == r
                {
                    a.1 += INDENT.len();
                }
            }
        }
    }
    /// Ctrl+/: comments out the lines, or back in when all are comments.
    pub fn toggle_comment(&mut self) {
        let (r1, r2) = self.rows();
        self.begin(EditKind::Other);
        let all = (r1..=r2).all(|r| {
            let t = self.lines[r].trim_start();
            t.is_empty() || t.starts_with("--")
        });
        for r in r1..=r2 {
            let line = &mut self.lines[r];
            let n = line.len() - line.trim_start().len();
            let delta: isize = if all {
                if line[n..].starts_with("-- ") {
                    line.replace_range(n..n + 3, "");
                    -3
                } else if line[n..].starts_with("--") {
                    line.replace_range(n..n + 2, "");
                    -2
                } else {
                    0
                }
            } else if line.trim().is_empty() {
                0
            } else {
                line.insert_str(n, "-- ");
                3
            };
            if r == self.row && self.col >= n {
                self.col = (self.col as isize + delta).max(n as isize) as usize;
            }
            if let Some(a) = &mut self.anchor
                && a.0 == r
                && a.1 >= n
            {
                a.1 = (a.1 as isize + delta).max(n as isize) as usize;
            }
        }
    }
    pub fn duplicate_line(&mut self) {
        self.begin(EditKind::Other);
        let line = self.lines[self.row].clone();
        self.lines.insert(self.row + 1, line);
        self.row += 1;
        self.anchor = None;
    }
    /// The text Ctrl+C copies: the selection, or the whole line.
    pub fn copy(&self) -> String {
        self.selected_text()
            .unwrap_or_else(|| format!("{}\n", self.lines[self.row]))
    }
    pub fn cut(&mut self) -> String {
        let text = self.copy();
        self.begin(EditKind::Other);
        if !self.delete_selection() {
            if self.lines.len() == 1 {
                self.lines[0].clear();
            } else {
                self.lines.remove(self.row);
                self.row = self.row.min(self.lines.len() - 1);
            }
            self.col = 0;
        }
        text
    }
    /// Puts the cursor at a line and char column (a click).
    pub fn click(&mut self, row: usize, char_col: usize, select: bool) {
        self.row = row.min(self.lines.len() - 1);
        if !select {
            self.anchor = None;
        }
        self.set_char_col(char_col);
        self.goal = None;
        self.last_kind = None;
    }
}

// ----------------------------------------------------------------- Lua highlighting

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tint {
    Plain,
    Keyword,
    Str,
    Number,
    Comment,
    Global,
}

/// What a line starts inside of: a long comment or string of some level.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Long {
    #[default]
    None,
    Comment(usize),
    Str(usize),
}

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];

fn is_api_global(word: &str) -> bool {
    matches!(
        word,
        "on" | "command"
            | "after"
            | "every"
            | "cancel"
            | "require"
            | "color"
            | "chat"
            | "game"
            | "player"
            | "world"
            | "hud"
            | "keys"
            | "view"
            | "agent"
            | "storage"
            | "mod"
            | "gfx"
            | "json"
            | "ui"
            | "draw"
            | "sound"
            | "export"
            | "import"
            | "emit"
            | "print"
            | "string"
            | "math"
            | "table"
            | "os"
            | "pairs"
            | "ipairs"
            | "tostring"
            | "tonumber"
            | "type"
            | "pcall"
            | "error"
            | "assert"
            | "select"
            | "setmetatable"
            | "getmetatable"
            | "self"
            | "dump"
    )
}

/// The end of a long bracket's closing `]==]` of `level`, from `i`.
fn long_close(line: &str, i: usize, level: usize) -> Option<usize> {
    let close = format!("]{}]", "=".repeat(level));
    line[i..].find(&close).map(|p| i + p + close.len())
}

fn long_open(line: &str, i: usize) -> Option<usize> {
    let b = line.as_bytes();
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let mut j = i + 1;
    while b.get(j) == Some(&b'=') {
        j += 1;
    }
    (b.get(j) == Some(&b'[')).then_some(j - i - 1)
}

/// A line's coloured pieces (byte ranges), and what the next line starts in.
pub fn highlight(line: &str, start: Long) -> (Vec<(usize, usize, Tint)>, Long) {
    let mut out = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    match start {
        Long::Comment(level) | Long::Str(level) => {
            let tint = if matches!(start, Long::Comment(_)) {
                Tint::Comment
            } else {
                Tint::Str
            };
            match long_close(line, 0, level) {
                Some(end) => {
                    out.push((0, end, tint));
                    i = end;
                }
                None => {
                    out.push((0, line.len(), tint));
                    return (out, start);
                }
            }
        }
        Long::None => {}
    }
    while i < b.len() {
        let c = b[i];
        let start_i = i;
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            if let Some(level) = long_open(line, i + 2) {
                match long_close(line, i + 4 + level, level) {
                    Some(end) => {
                        out.push((i, end, Tint::Comment));
                        i = end;
                        continue;
                    }
                    None => {
                        out.push((i, b.len(), Tint::Comment));
                        return (out, Long::Comment(level));
                    }
                }
            }
            out.push((i, b.len(), Tint::Comment));
            return (out, Long::None);
        }
        if let Some(level) = long_open(line, i) {
            match long_close(line, i + 2 + level, level) {
                Some(end) => {
                    out.push((i, end, Tint::Str));
                    i = end;
                    continue;
                }
                None => {
                    out.push((i, b.len(), Tint::Str));
                    return (out, Long::Str(level));
                }
            }
        }
        if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
            out.push((start_i, i, Tint::Str));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') {
                i += 1;
            }
            out.push((start_i, i, Tint::Number));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &line[start_i..i];
            let after_dot = start_i > 0 && b[start_i - 1] == b'.' && !(start_i > 1 && b[start_i - 2] == b'.');
            let tint = if LUA_KEYWORDS.contains(&word) {
                Tint::Keyword
            } else if !after_dot && is_api_global(word) {
                Tint::Global
            } else {
                Tint::Plain
            };
            out.push((start_i, i, tint));
            continue;
        }
        // Anything else up to the next interesting byte.
        i += line[i..].chars().next().map_or(1, char::len_utf8);
        while i < b.len() && !b[i].is_ascii_alphanumeric() && !matches!(b[i], b'_' | b'"' | b'\'' | b'-' | b'[' | b'.')
        {
            i += line[i..].chars().next().map_or(1, char::len_utf8);
        }
        out.push((start_i, i, Tint::Plain));
    }
    (out, Long::None)
}

// ----------------------------------------------------------------- completion

/// What Tab completes the console's word to: API names, then the console's
/// own functions and the standard library's tables.
pub fn complete(text: &str, cursor: usize) -> Option<String> {
    let before = &text[..cursor];
    let start = before
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .map_or(0, |i| i + 1);
    let word = &before[start..];
    if word.is_empty() {
        return None;
    }
    let mut names: Vec<String> = API
        .iter()
        .map(|(t, n, _, _)| {
            if t.is_empty() {
                (*n).to_owned()
            } else {
                format!("{t}.{n}")
            }
        })
        .chain(CONSOLE_API.iter().map(|(n, _, _)| (*n).to_owned()))
        .chain(
            [
                "print",
                "pairs",
                "ipairs",
                "tostring",
                "tonumber",
                "string.format",
                "string.rep",
                "math.floor",
                "math.random",
                "table.insert",
                "table.concat",
                "dump",
            ]
            .iter()
            .map(|s| (*s).to_owned()),
        )
        .filter(|n| n.starts_with(word) && n != word)
        .collect();
    names.sort();
    names.dedup();
    let first = names.first()?;
    // The longest prefix every match shares, or the only match with `(`.
    if names.len() == 1 {
        return Some(format!("{}(", &first[word.len()..]));
    }
    let mut common = first.clone();
    for n in &names[1..] {
        while !n.starts_with(&common) {
            common.pop();
        }
    }
    (common.len() > word.len()).then(|| common[word.len()..].to_owned())
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

pub use super::hstudio_look::{CODE_LINE, draw, draw_screen};

// ----------------------------------------------------------------- input

pub fn handle(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    match input {
        Input::Mouse(x, y) => {
            let k = st.s.ui.dp_factor();
            let (x, y) = (x * k, y * k);
            st.s.ui.mouse = (x, y);
            if st.s.ui.dragging {
                editor_point(st, game, x, y, true);
            }
            UiResult::Used
        }
        Input::Button { left, down } => {
            if !down {
                st.s.ui.dragging = false;
                return UiResult::Used;
            }
            if !left || st.s.ui.naming.is_some() {
                return UiResult::Used;
            }
            let at = st.s.ui.mouse;
            let hit = st.s.ui.hits.iter().rev().find(|(r, _)| inside(*r, at)).map(|(_, a)| *a);
            if let Some(action) = hit {
                return act(st, game, action);
            }
            if st.s.ui.tab == Tab::Editor && editor_point(st, game, at.0, at.1, false) {
                st.s.ui.dragging = true;
            }
            UiResult::Used
        }
        Input::Wheel(steps) => {
            let n = (steps.abs().ceil() as usize).max(1) * 3;
            let up = steps > 0.0;
            match st.s.ui.tab {
                Tab::Console => {
                    st.s.ui.log_scroll = if up {
                        st.s.ui.log_scroll + n
                    } else {
                        st.s.ui.log_scroll.saturating_sub(n)
                    };
                }
                Tab::Editor => {
                    let ed = &mut st.s.editor;
                    ed.scroll = if up {
                        ed.scroll.saturating_sub(n)
                    } else {
                        (ed.scroll + n).min(ed.lines.len().saturating_sub(1))
                    };
                    // Keep the cursor on screen as the view scrolls.
                    let rows = st.s.ui.page.max(1);
                    if ed.row < ed.scroll {
                        ed.click(ed.scroll, ed.char_col(), ed.anchor.is_some());
                    } else if ed.row >= ed.scroll + rows {
                        ed.click(ed.scroll + rows - 1, ed.char_col(), ed.anchor.is_some());
                    }
                }
                Tab::Library => {
                    let n = st.s.mods.len();
                    if up {
                        st.s.ui.library_sel = st.s.ui.library_sel.saturating_sub(1);
                    } else if n > 0 {
                        st.s.ui.library_sel = (st.s.ui.library_sel + 1).min(n - 1);
                    }
                }
                Tab::Docs => {
                    st.s.ui.docs_scroll = if up {
                        st.s.ui.docs_scroll.saturating_sub(n)
                    } else {
                        st.s.ui.docs_scroll + n
                    };
                }
            }
            UiResult::Used
        }
        Input::Text(t) => {
            if let Some(name) = &mut st.s.ui.naming {
                name.insert(&t);
                return UiResult::Used;
            }
            match st.s.ui.tab {
                Tab::Console => {
                    st.s.ui.input.insert(&t);
                    st.s.ui.hist = None;
                }
                Tab::Editor => st.s.editor.insert(&t),
                Tab::Library => match t.as_str() {
                    " " => {
                        if let Some(m) = st.s.mods.get(st.s.ui.library_sel) {
                            let on = !m.enabled;
                            let i = st.s.ui.library_sel;
                            st.set_enabled(game, i, on);
                        }
                    }
                    "r" | "R" => {
                        let i = st.s.ui.library_sel;
                        if i < st.s.mods.len() {
                            return act(st, game, Action::Reload(i));
                        }
                    }
                    "n" | "N" => return act(st, game, Action::NewMod),
                    _ => {}
                },
                Tab::Docs => {}
            }
            UiResult::Used
        }
        Input::Key { key, down, mods, .. } => {
            if !down {
                return UiResult::Used;
            }
            key_down(st, game, key, mods)
        }
    }
}

/// A click or drag in the editor's text: moves the cursor there (a drag
/// keeps the selection's other end).
fn editor_point(st: &mut Studio, game: &mut dyn GameApi, x: f32, y: f32, drag: bool) -> bool {
    let Some(v) = st.s.ui.view else {
        return false;
    };
    if !drag && (x < v.x - 30.0 || y < v.y || y > v.y + CODE_LINE * v.rows as f32 || x > v.x + v.w + 4.0) {
        return false;
    }
    let ed = &mut st.s.editor;
    let row_in_view = ((y - v.y) / CODE_LINE).floor().max(0.0) as usize;
    let row = (ed.scroll + row_in_view.min(v.rows)).min(ed.lines.len() - 1);
    let col = ed.hscroll + column_at(game, &ed.lines[row], ed.hscroll, x - v.x);
    let anchor = if drag { ed.anchor } else { None };
    ed.click(row, col, drag);
    ed.anchor = Some(anchor.unwrap_or((ed.row, ed.col)));
    true
}

/// The char column nearest `dx` GUI pixels into `line`, past `skip` chars.
fn column_at(game: &mut dyn GameApi, line: &str, skip: usize, dx: f32) -> usize {
    let from = line.char_indices().nth(skip).map_or(line.len(), |(i, _)| i);
    let mut prev = 0.0;
    for (n, (i, c)) in line[from..].char_indices().enumerate() {
        let w = game.dp_measure(
            &line[from..from + i + c.len_utf8()],
            super::hstudio_look::CODE_SIZE,
            Font::Mono,
        );
        if dx < (prev + w) / 2.0 {
            return n;
        }
        prev = w;
    }
    line[from..].chars().count()
}

fn act(st: &mut Studio, game: &mut dyn GameApi, action: Action) -> UiResult {
    // A row drawn last frame may be gone (the folder changed since).
    if let Action::Select(i) | Action::Toggle(i) | Action::Edit(i) | Action::Reload(i) = action
        && i >= st.s.mods.len()
    {
        return UiResult::Used;
    }
    match action {
        Action::Tab(t) => st.s.ui.tab = t,
        Action::Close => return UiResult::Close,
        Action::Select(i) => st.s.ui.library_sel = i,
        Action::Toggle(i) => {
            let on = !st.s.mods[i].enabled;
            st.set_enabled(game, i, on);
        }
        Action::Edit(i) => {
            st.edit_mod(i);
            st.s.ui.tab = Tab::Editor;
        }
        Action::Reload(i) => {
            st.s.mods[i].enabled = true;
            st.set_enabled(game, i, true);
        }
        Action::NewMod => {
            st.s.ui.naming = Some(LineEdit::default());
            st.s.ui.naming_at = st.s.now;
        }
        Action::ReloadAll => st.reload_all(game),
        Action::Install(i) => {
            if let Err(e) = st.install_example(game, i) {
                st.s.log(LogKind::Error, e);
            } else if let Some(j) = st.s.mods.iter().position(|m| m.id == EXAMPLES[i].id) {
                st.s.ui.library_sel = j;
            }
        }
        Action::OpenFolder => {
            let dir = st.s.mods_dir();
            let _ = std::fs::create_dir_all(&dir);
            game.open_folder(&dir);
        }
        Action::Save => st.save_editor(game),
        Action::Run => st.run_editor(game),
    }
    UiResult::Used
}

fn key_down(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) -> UiResult {
    if let Some(name) = &mut st.s.ui.naming {
        match key {
            Key::Escape => st.s.ui.naming = None,
            Key::Enter => {
                let wanted = name.text.trim().to_owned();
                st.s.ui.naming = None;
                if !wanted.is_empty() {
                    match st.create_mod(game, &wanted) {
                        Ok(path) => {
                            let title = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            st.s.editor.open(&path, &title);
                            st.s.ui.tab = Tab::Editor;
                        }
                        Err(e) => st.s.log(LogKind::Error, e),
                    }
                }
            }
            k => {
                name.key(k, mods);
            }
        }
        return UiResult::Used;
    }
    match key {
        Key::Escape | Key::Toggle => return UiResult::Close,
        Key::Char(c @ '1'..='4') if mods.ctrl => {
            st.s.ui.tab = TABS[(c as u8 - b'1') as usize].0;
            return UiResult::Used;
        }
        Key::Tab if mods.ctrl => {
            let i = TABS.iter().position(|(t, _)| *t == st.s.ui.tab).unwrap_or(0);
            let n = if mods.shift { i + TABS.len() - 1 } else { i + 1 };
            st.s.ui.tab = TABS[n % TABS.len()].0;
            return UiResult::Used;
        }
        _ => {}
    }
    match st.s.ui.tab {
        Tab::Console => console_key(st, game, key, mods),
        Tab::Editor => editor_key(st, game, key, mods),
        Tab::Library => library_key(st, game, key),
        Tab::Docs => {
            let page = st.s.ui.page.max(1);
            let s = &mut st.s.ui.docs_scroll;
            match key {
                Key::Up => *s = s.saturating_sub(1),
                Key::Down => *s += 1,
                Key::PageUp => *s = s.saturating_sub(page),
                Key::PageDown => *s += page,
                Key::Home => *s = 0,
                _ => {}
            }
        }
    }
    UiResult::Used
}

fn console_key(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) {
    let ui = &mut st.s.ui;
    match key {
        Key::Enter => {
            let line = std::mem::take(&mut ui.input);
            ui.hist = None;
            ui.log_scroll = 0;
            st.run_console(game, &line.text);
        }
        Key::Up | Key::Down => {
            let n = st.s.history.len();
            if n == 0 {
                return;
            }
            let at = match (ui.hist, key) {
                (None, Key::Up) => Some(n - 1),
                (None, _) => None,
                (Some(i), Key::Up) => Some(i.saturating_sub(1)),
                (Some(i), _) if i + 1 < n => Some(i + 1),
                (Some(_), _) => None,
            };
            ui.hist = at;
            match at {
                Some(i) => ui.input.set(&st.s.history[i]),
                None => ui.input.set(""),
            }
        }
        Key::Tab => {
            if let Some(rest) = complete(&ui.input.text, ui.input.cursor) {
                ui.input.insert(&rest);
            }
        }
        Key::PageUp => ui.log_scroll += ui.page.max(1),
        Key::PageDown => ui.log_scroll = ui.log_scroll.saturating_sub(ui.page.max(1)),
        Key::Char('l') if mods.ctrl => st.s.log.clear(),
        Key::Char('v') if mods.ctrl => {
            let clip = game.clipboard();
            st.s.ui.input.insert(&clip);
        }
        Key::Char('c' | 'x') if mods.ctrl => {
            game.set_clipboard(&ui.input.text);
            if key == Key::Char('x') {
                ui.input.set("");
            }
        }
        Key::Char('a') if mods.ctrl => ui.input.cursor = 0,
        Key::Char('e') if mods.ctrl => ui.input.cursor = ui.input.text.len(),
        k => {
            ui.input.key(k, mods);
        }
    }
}

fn editor_key(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) {
    let page = st.s.ui.page.max(1) as isize;
    if mods.ctrl {
        match key {
            Key::Char('s') => return st.save_editor(game),
            Key::Enter => return st.run_editor(game),
            Key::Char('z') if mods.shift => return st.s.editor.redo(),
            Key::Char('z') => return st.s.editor.undo(),
            Key::Char('y') => return st.s.editor.redo(),
            Key::Char('a') => return st.s.editor.select_all(),
            Key::Char('c') => {
                let t = st.s.editor.copy();
                return game.set_clipboard(&t);
            }
            Key::Char('x') => {
                let t = st.s.editor.cut();
                return game.set_clipboard(&t);
            }
            Key::Char('v') => {
                let clip = game.clipboard();
                if !clip.is_empty() {
                    st.s.editor.insert(&clip);
                }
                return;
            }
            Key::Char('d') => return st.s.editor.duplicate_line(),
            Key::Char('/') => return st.s.editor.toggle_comment(),
            Key::Home => {
                let select = mods.shift;
                let ed = &mut st.s.editor;
                ed.select_from_here(select);
                ed.row = 0;
                ed.col = 0;
                return;
            }
            Key::End => {
                let select = mods.shift;
                let ed = &mut st.s.editor;
                ed.select_from_here(select);
                ed.row = ed.lines.len() - 1;
                ed.col = ed.lines[ed.row].len();
                return;
            }
            _ => {}
        }
    }
    let ed = &mut st.s.editor;
    match key {
        Key::Enter => ed.newline(),
        Key::Backspace => ed.backspace(mods.ctrl),
        Key::Delete => ed.delete(mods.ctrl),
        Key::Left => ed.move_h(false, mods.ctrl, mods.shift),
        Key::Right => ed.move_h(true, mods.ctrl, mods.shift),
        Key::Up => ed.move_v(-1, mods.shift),
        Key::Down => ed.move_v(1, mods.shift),
        Key::PageUp => ed.move_v(-page, mods.shift),
        Key::PageDown => ed.move_v(page, mods.shift),
        Key::Home => ed.home(mods.shift),
        Key::End => ed.end(mods.shift),
        Key::Tab => ed.indent(mods.shift),
        _ => {}
    }
}

fn library_key(st: &mut Studio, game: &mut dyn GameApi, key: Key) {
    let n = st.s.mods.len();
    let ui = &mut st.s.ui;
    match key {
        Key::Up => ui.library_sel = ui.library_sel.saturating_sub(1),
        Key::Down if n > 0 => ui.library_sel = (ui.library_sel + 1).min(n - 1),
        Key::Enter if ui.library_sel < n => {
            let i = ui.library_sel;
            act(st, game, Action::Edit(i));
        }
        _ => {}
    }
}

// ----------------------------------------------------------------- script screens

/// A screen a script opened with `ui.open`: its table (read again each
/// frame, so the script can change it), the text field being typed in, the
/// slider being dragged, and where the last frame put each widget.
pub struct ScriptScreen {
    pub owner: Owner,
    pub table: TableRef,
    pub focus: Option<usize>,
    pub edit: LineEdit,
    pub(crate) drag: Option<usize>,
    pub(crate) rows: Vec<([f32; 4], usize)>,
}

impl ScriptScreen {
    pub fn new(owner: Owner, table: TableRef) -> ScriptScreen {
        ScriptScreen {
            owner,
            table,
            focus: None,
            edit: LineEdit::default(),
            drag: None,
            rows: Vec::new(),
        }
    }
}

pub(crate) fn field(t: &TableRef, k: &str) -> Value {
    t.borrow().get_str(k)
}

pub(crate) fn field_text(t: &TableRef, k: &str) -> Option<String> {
    match field(t, k) {
        Value::Nil => None,
        v => Some(v.plain_text()),
    }
}

pub(crate) fn field_num(t: &TableRef, k: &str, default: f64) -> f64 {
    field(t, k).to_num().unwrap_or(default)
}

pub(crate) fn widgets(t: &TableRef) -> Vec<TableRef> {
    match field(t, "widgets") {
        Value::Table(list) => {
            let items = list.borrow().list();
            items
                .into_iter()
                .filter_map(|v| match v {
                    Value::Table(w) => Some(w),
                    _ => None,
                })
                .take(64)
                .collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn kind(w: &TableRef) -> String {
    field_text(w, "type").unwrap_or_else(|| "label".into())
}

/// A slider's value as it shows.
pub(crate) fn slider_text(v: f64, step: f64) -> String {
    if step >= 1.0 && v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

fn key_name(key: Key) -> String {
    match key {
        Key::Escape => "escape".into(),
        Key::Toggle => "toggle".into(),
        Key::Enter => "enter".into(),
        Key::Backspace => "backspace".into(),
        Key::Delete => "delete".into(),
        Key::Tab => "tab".into(),
        Key::Left => "left".into(),
        Key::Right => "right".into(),
        Key::Up => "up".into(),
        Key::Down => "down".into(),
        Key::Home => "home".into(),
        Key::End => "end".into(),
        Key::PageUp => "pageup".into(),
        Key::PageDown => "pagedown".into(),
        Key::Char(c) => c.to_string(),
        Key::Other => "other".into(),
    }
}

/// Sets a widget's `value` and calls its `change(value, widget)`.
fn set_value(st: &mut Studio, game: &mut dyn GameApi, wt: &TableRef, v: Value) {
    wt.borrow_mut().set_str("value", v.clone());
    let f = field(wt, "change");
    st.screen_call(game, &f, vec![v, Value::Table(wt.clone())], false);
}

fn slide(st: &mut Studio, game: &mut dyn GameApi, i: usize, x: f32) {
    let Some(sc) = &st.s.screen else { return };
    let Some(&(r, _)) = sc.rows.iter().find(|(_, j)| *j == i) else {
        return;
    };
    let Some(wt) = widgets(&sc.table).get(i).cloned() else {
        return;
    };
    let (min, max) = (field_num(&wt, "min", 0.0), field_num(&wt, "max", 1.0));
    let step = field_num(&wt, "step", 0.0);
    let frac = f64::from(((x - r[0]) / r[2].max(1.0)).clamp(0.0, 1.0));
    let mut v = min + frac * (max - min);
    if step > 0.0 {
        v = min + ((v - min) / step).round() * step;
    }
    let v = v.clamp(min.min(max), max.max(min));
    if field(&wt, "value").to_num() != Some(v) {
        set_value(st, game, &wt, Value::Num(v));
    }
}

/// Input for the script screen.
pub fn handle_screen(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    let Some(sc) = &st.s.screen else {
        return UiResult::Close;
    };
    let table = sc.table.clone();
    let ws = widgets(&table);
    // Canvas screens get dp, the rest GUI pixels (as hud.* draws).
    let unit = if field(&table, "canvas").truthy() {
        1.0
    } else {
        st.s.ui.dp_factor()
    };
    match input {
        Input::Mouse(x, y) => {
            let k = st.s.ui.dp_factor();
            let (x, y) = (x * k, y * k);
            if st.s.ui.mouse == (x, y) {
                return UiResult::Used;
            }
            st.s.ui.mouse = (x, y);
            if let Some(i) = st.s.screen.as_ref().and_then(|sc| sc.drag) {
                slide(st, game, i, x);
            }
            let f = field(&table, "move");
            if f.is_callable() {
                let at = vec![Value::Num(f64::from(x / unit)), Value::Num(f64::from(y / unit))];
                st.screen_call(game, &f, at, false);
            }
        }
        Input::Button { left, down } => {
            st.s.ui.buttons[usize::from(!left)] = down;
            if !down {
                if let Some(sc) = &mut st.s.screen {
                    sc.drag = None;
                }
                let f = field(&table, "release");
                if f.is_callable() {
                    let (x, y) = st.s.ui.mouse;
                    let button = if left { "left" } else { "right" };
                    let at = vec![
                        Value::Num(f64::from(x / unit)),
                        Value::Num(f64::from(y / unit)),
                        Value::str(button),
                    ];
                    st.screen_call(game, &f, at, false);
                }
                return UiResult::Used;
            }
            let at = st.s.ui.mouse;
            let hit = sc.rows.iter().find(|(r, _)| inside(*r, at)).map(|(_, i)| *i);
            match hit.and_then(|i| ws.get(i).cloned().map(|w| (i, w))) {
                Some((i, wt)) if left => {
                    if let Some(sc) = &mut st.s.screen {
                        sc.focus = None;
                    }
                    match kind(&wt).as_str() {
                        "button" => {
                            let f = field(&wt, "click");
                            st.screen_call(game, &f, vec![Value::Table(wt.clone())], false);
                        }
                        "toggle" => {
                            let on = !field(&wt, "value").truthy();
                            set_value(st, game, &wt, Value::Bool(on));
                        }
                        "slider" => {
                            if let Some(sc) = &mut st.s.screen {
                                sc.drag = Some(i);
                            }
                            slide(st, game, i, at.0);
                        }
                        "input" => {
                            let value = field_text(&wt, "value").unwrap_or_default();
                            if let Some(sc) = &mut st.s.screen {
                                sc.focus = Some(i);
                                sc.edit.set(&value);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {
                    if let Some(sc) = &mut st.s.screen {
                        sc.focus = None;
                    }
                    let f = field(&table, "click");
                    let button = if left { "left" } else { "right" };
                    st.screen_call(
                        game,
                        &f,
                        vec![
                            Value::Num(f64::from(at.0 / unit)),
                            Value::Num(f64::from(at.1 / unit)),
                            Value::str(button),
                        ],
                        false,
                    );
                }
            }
        }
        Input::Wheel(steps) => {
            let at = st.s.ui.mouse;
            let hit = sc.rows.iter().find(|(r, _)| inside(*r, at)).map(|(_, i)| *i);
            if let Some(wt) = hit.and_then(|i| ws.get(i).cloned())
                && kind(&wt) == "slider"
            {
                let (min, max) = (field_num(&wt, "min", 0.0), field_num(&wt, "max", 1.0));
                let step = field_num(&wt, "step", 0.0);
                let step = if step > 0.0 { step } else { (max - min) / 20.0 };
                let v =
                    (field_num(&wt, "value", min) + step * f64::from(steps.signum())).clamp(min.min(max), max.max(min));
                set_value(st, game, &wt, Value::Num(v));
            } else {
                let f = field(&table, "wheel");
                st.screen_call(game, &f, vec![Value::Num(f64::from(steps))], false);
            }
        }
        Input::Text(t) => {
            let focus = sc.focus;
            match focus.and_then(|i| ws.get(i).cloned()) {
                Some(wt) => {
                    let value = match &mut st.s.screen {
                        Some(sc) => {
                            sc.edit.insert(&t);
                            sc.edit.text.clone()
                        }
                        None => return UiResult::Close,
                    };
                    set_value(st, game, &wt, Value::str(&value));
                }
                None => {
                    let f = field(&table, "text");
                    st.screen_call(game, &f, vec![Value::str(&t)], false);
                }
            }
        }
        Input::Key { key, down, mods, .. } => {
            let focus = sc.focus;
            if down && matches!(key, Key::Escape | Key::Toggle) {
                if focus.is_some() && key == Key::Escape {
                    if let Some(sc) = &mut st.s.screen {
                        sc.focus = None;
                    }
                    return UiResult::Used;
                }
                return UiResult::Close;
            }
            match focus.and_then(|i| ws.get(i).cloned().map(|w| (i, w))) {
                Some((i, wt)) if down => match key {
                    Key::Enter => {
                        let f = field(&wt, "submit");
                        let v = field(&wt, "value");
                        if let Some(sc) = &mut st.s.screen {
                            sc.focus = None;
                        }
                        st.screen_call(game, &f, vec![v, Value::Table(wt.clone())], false);
                    }
                    Key::Tab => {
                        let next = (i + 1..ws.len()).chain(0..i).find(|&j| kind(&ws[j]) == "input");
                        if let (Some(j), Some(sc)) = (next, &mut st.s.screen) {
                            sc.focus = Some(j);
                            sc.edit.set(&field_text(&ws[j], "value").unwrap_or_default());
                        }
                    }
                    Key::Char('v') if mods.ctrl => {
                        let clip = game.clipboard();
                        let value = match &mut st.s.screen {
                            Some(sc) => {
                                sc.edit.insert(&clip);
                                sc.edit.text.clone()
                            }
                            None => return UiResult::Close,
                        };
                        set_value(st, game, &wt, Value::str(&value));
                    }
                    k => {
                        let changed = match &mut st.s.screen {
                            Some(sc) => {
                                let before = sc.edit.text.clone();
                                sc.edit.key(k, mods);
                                (sc.edit.text != before).then(|| sc.edit.text.clone())
                            }
                            None => return UiResult::Close,
                        };
                        if let Some(value) = changed {
                            set_value(st, game, &wt, Value::str(&value));
                        }
                    }
                },
                Some(_) => {}
                None => {
                    let f = field(&table, "key");
                    st.screen_call(game, &f, vec![Value::str(&key_name(key)), Value::Bool(down)], false);
                }
            }
        }
    }
    if st.s.screen.is_none() {
        // A callback closed it (ui.close) or failed.
        return UiResult::Close;
    }
    UiResult::Used
}
