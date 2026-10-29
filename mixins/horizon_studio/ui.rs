// Horizon Studio's screen: Console (a Lua executor), Editor (a code editor
// with highlighting, undo and the clipboard), Library (the script mods, their
// state and errors, and the bundled examples) and Docs (the API). It draws
// with the game's own GUI font and rectangles through GameApi, in Horizon's
// dark design tokens (docs/design-system.md): a veil over the world, plates
// for the console and code, 1 px rules, one accent for the main action.

use super::hstudio::{API, CONSOLE_API, GameApi, LogKind, ModState, Studio};
use super::hstudio_examples::EXAMPLES;
use std::path::{Path, PathBuf};

const VEIL: u32 = 0xE60D_0E13;
const PLATE_SOFT: u32 = 0xFF18_1A22;
const PLATE: u32 = 0xFF20_222C;
const RULE: u32 = 0xFF24_262F;
const INK: u32 = 0xEC_EDF3;
const INK_SOFT: u32 = 0xC5_C7D1;
const MUTED: u32 = 0x8B_8F9C;
const ACCENT: u32 = 0x9A_86FF;
const DANGER: u32 = 0xF0_7A68;
const SUCCESS: u32 = 0x45_C27A;
const SELECTION: u32 = 0xFF3A_3550;
const ERROR_ROW: u32 = 0x60F0_7A68;
// Code colours, kept quiet beside the ink.
const KEYWORD: u32 = 0xB4_A7FF;
const STRING: u32 = 0x8F_D6A8;
const NUMBER: u32 = 0xF2_C48D;
const COMMENT: u32 = 0x7A_7E8B;
const GLOBAL: u32 = 0x9F_C2FF;

const LINE_H: f32 = 10.0;
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
enum Action {
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
struct EditorView {
    x: f32,
    y: f32,
    w: f32,
    rows: usize,
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
    pub mouse: (f32, f32),
    dragging: bool,
    hits: Vec<([f32; 4], Action)>,
    view: Option<EditorView>,
    /// Lines the last frame could show, for Page Up/Down.
    page: usize,
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

impl Tint {
    fn rgb(self) -> u32 {
        match self {
            Tint::Plain => INK,
            Tint::Keyword => KEYWORD,
            Tint::Str => STRING,
            Tint::Number => NUMBER,
            Tint::Comment => COMMENT,
            Tint::Global => GLOBAL,
        }
    }
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

// ----------------------------------------------------------------- drawing helpers

fn text(game: &mut dyn GameApi, s: &str, x: f32, y: f32, rgb: u32) {
    game.text(s, [x.floor(), y.floor()], rgb, false);
}

/// The longest start of `s` that fits in `max_w`.
fn fit(game: &dyn GameApi, s: &str, max_w: f32) -> String {
    if game.text_width(s) <= max_w {
        return s.to_owned();
    }
    let mut out = String::new();
    for c in s.chars() {
        out.push(c);
        if game.text_width(&out) > max_w - game.text_width("...") {
            out.pop();
            out.push_str("...");
            return out;
        }
    }
    out
}

/// `s` broken into lines no wider than `max_w`, at spaces where it can.
pub fn wrap(game: &dyn GameApi, s: &str, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_inclusive(' ') {
            let candidate = format!("{line}{word}");
            if game.text_width(candidate.trim_end()) <= max_w {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(line.trim_end().to_owned());
            }
            // A word longer than the line: cut it.
            let mut piece = String::new();
            for c in word.chars() {
                piece.push(c);
                if game.text_width(&piece) > max_w {
                    piece.pop();
                    lines.push(std::mem::take(&mut piece));
                    piece.push(c);
                }
            }
            line = piece;
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

impl Ui {
    /// A clickable label; returns its width.
    fn button(&mut self, game: &mut dyn GameApi, label: &str, x: f32, y: f32, rgb: u32, action: Action) -> f32 {
        let w = game.text_width(label) + 8.0;
        let r = [x, y - 2.0, w, 12.0];
        if inside(r, self.mouse) {
            game.fill(r, PLATE);
        }
        text(game, label, x + 4.0, y, rgb);
        self.hits.push((r, action));
        w
    }
}

// ----------------------------------------------------------------- the screen

pub fn draw(st: &mut Studio, game: &mut dyn GameApi) {
    let (w, h) = game.gui_size();
    st.s.ui.hits.clear();
    st.s.ui.view = None;
    game.fill([0.0, 0.0, w, h], VEIL);
    text(game, "Horizon Studio", 10.0, 7.0, INK);
    let mut x = 16.0 + game.text_width("Horizon Studio");
    let current = st.s.ui.tab;
    for (tab, label) in TABS {
        let rgb = if tab == current { INK } else { MUTED };
        let bw = st.s.ui.button(game, label, x, 7.0, rgb, Action::Tab(tab));
        if tab == current {
            game.fill([x + 4.0, 17.0, bw - 8.0, 1.0], 0xFF00_0000 | INK);
        }
        x += bw + 2.0;
    }
    let close = "Esc  close";
    let cw = game.text_width(close) + 8.0;
    st.s.ui.button(game, close, w - cw - 8.0, 7.0, MUTED, Action::Close);
    game.fill([8.0, 20.0, w - 16.0, 1.0], RULE);
    let body = [10.0, 26.0, w - 20.0, h - 40.0];
    let hint = match current {
        Tab::Console => "Enter run   Up/Down history   Tab complete   Ctrl+L clear   Ctrl+1-4 tabs",
        Tab::Editor => {
            "Ctrl+S save (reloads the mod)   Ctrl+Enter run   Ctrl+Z/Y undo   Ctrl+/ comment   Ctrl+D duplicate"
        }
        Tab::Library => {
            "Up/Down choose   Enter edit   Space on/off   R reload   N new mod   Drop a .lua on the window to install"
        }
        Tab::Docs => "Up/Down, Page Up/Down or the wheel to scroll",
    };
    let hint = fit(game, hint, w - 20.0);
    text(game, &hint, 10.0, h - 11.0, MUTED);
    match current {
        Tab::Console => draw_console(st, game, body),
        Tab::Editor => draw_editor(st, game, body),
        Tab::Library => draw_library(st, game, body),
        Tab::Docs => draw_docs(st, game, body),
    }
    if let Some(name) = st.s.ui.naming.clone() {
        let bw = 220f32.min(w - 20.0);
        let r = [((w - bw) / 2.0).floor(), (h / 2.0 - 24.0).floor(), bw, 48.0];
        game.fill(r, 0xFF16_181F);
        text(game, "New mod: what is it called?", r[0] + 6.0, r[1] + 6.0, INK);
        game.fill([r[0] + 6.0, r[1] + 18.0, bw - 12.0, 12.0], PLATE_SOFT);
        draw_line_edit(st.s.now, game, &name, r[0] + 9.0, r[1] + 20.0, bw - 18.0, INK);
        text(game, "Enter to create, Esc to cancel", r[0] + 6.0, r[1] + 35.0, MUTED);
    }
}

fn draw_line_edit(now: f64, game: &mut dyn GameApi, e: &LineEdit, x: f32, y: f32, max_w: f32, rgb: u32) {
    // Scroll so the cursor shows.
    let mut start = 0;
    while game.text_width(&e.text[start..e.cursor]) > max_w - 4.0 {
        start = next_char(&e.text, start);
    }
    let shown = fit(game, &e.text[start..], max_w);
    text(game, &shown, x, y, rgb);
    if (now * 2.0) as i64 % 2 == 0 {
        let cx = x + game.text_width(&e.text[start..e.cursor]);
        game.fill([cx.floor(), y - 1.0, 1.0, 10.0], 0xFF00_0000 | INK);
    }
}

fn log_rgb(kind: LogKind) -> u32 {
    match kind {
        LogKind::Input => ACCENT,
        LogKind::Output => INK,
        LogKind::Error => DANGER,
        LogKind::Trace => MUTED,
        LogKind::Info => INK_SOFT,
    }
}

fn draw_console(st: &mut Studio, game: &mut dyn GameApi, [x, y, w, h]: [f32; 4]) {
    game.fill([x, y, w, h], PLATE_SOFT);
    let rows = ((h - 22.0) / LINE_H).max(1.0) as usize;
    st.s.ui.page = rows;
    // Wrapped lines from the newest up, enough to fill the view.
    let need = rows + st.s.ui.log_scroll;
    let mut shown: Vec<(String, u32)> = Vec::new();
    for line in st.s.log.iter().rev() {
        let pieces = wrap(game, &line.text, w - 10.0);
        for p in pieces.into_iter().rev() {
            shown.push((p, log_rgb(line.kind)));
        }
        if shown.len() > need {
            break;
        }
    }
    let max_scroll = shown.len().saturating_sub(rows);
    st.s.ui.log_scroll =
        st.s.ui
            .log_scroll
            .min(max_scroll.max(st.s.ui.log_scroll.min(max_scroll)));
    let skip = st.s.ui.log_scroll;
    for (i, (line, rgb)) in shown.iter().skip(skip).take(rows).enumerate() {
        let ly = y + h - 22.0 - LINE_H * (i as f32 + 1.0) + 2.0;
        text(game, line, x + 5.0, ly, *rgb);
    }
    if skip > 0 {
        let more = format!("{skip} lines below");
        let mw = game.text_width(&more);
        text(game, &more, x + w - mw - 5.0, y + h - 32.0, MUTED);
    }
    game.fill([x, y + h - 16.0, w, 1.0], RULE);
    text(game, ">", x + 5.0, y + h - 11.0, ACCENT);
    let input = st.s.ui.input.clone();
    draw_line_edit(st.s.now, game, &input, x + 14.0, y + h - 11.0, w - 20.0, INK);
    if input.cursor == input.text.len()
        && let Some(rest) = complete(&input.text, input.cursor)
    {
        let cx = x + 14.0 + game.text_width(&input.text);
        if cx + game.text_width(&rest) < x + w - 6.0 {
            text(game, &rest, cx + 1.0, y + h - 11.0, MUTED);
        }
    }
}

fn draw_editor(st: &mut Studio, game: &mut dyn GameApi, [x, y, w, h]: [f32; 4]) {
    let title = format!("{}{}", st.s.editor.title, if st.s.editor.dirty { " *" } else { "" });
    text(game, &title, x, y, INK);
    let mut bx = x + w;
    for (label, action, rgb) in [("Run", Action::Run, ACCENT), ("Save", Action::Save, INK_SOFT)] {
        bx -= game.text_width(label) + 10.0;
        st.s.ui.button(game, label, bx, y, rgb, action);
    }
    if let Some(p) = &st.s.editor.problem {
        let msg = fit(
            game,
            &format!("Could not read the file: {p}"),
            bx - x - game.text_width(&title) - 20.0,
        );
        let mx = x + game.text_width(&title) + 12.0;
        text(game, &msg, mx, y, DANGER);
    }
    let top = y + 13.0;
    let area_h = h - 13.0;
    game.fill([x, top, w, area_h], PLATE_SOFT);
    let ed = &mut st.s.editor;
    let rows = ((area_h - 4.0) / LINE_H).max(1.0) as usize;
    st.s.ui.page = rows;
    if ed.row < ed.scroll {
        ed.scroll = ed.row;
    }
    if ed.row >= ed.scroll + rows {
        ed.scroll = ed.row + 1 - rows;
    }
    ed.scroll = ed.scroll.min(ed.lines.len().saturating_sub(1));
    let gutter = game.text_width(&format!("{}", ed.lines.len().max(99))) + 8.0;
    let text_x = x + gutter + 4.0;
    let text_w = w - gutter - 8.0;
    // Keep the cursor's column in view.
    let cur_chars = ed.char_col();
    if cur_chars < ed.hscroll {
        ed.hscroll = cur_chars.saturating_sub(4);
    }
    loop {
        let line = &ed.lines[ed.row];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if from > ed.col || game.text_width(&line[from..ed.col]) <= text_w - 6.0 {
            break;
        }
        ed.hscroll += 4;
    }
    // Long comments and strings carried from line to line.
    let mut state = Long::None;
    let mut states = Vec::with_capacity(ed.lines.len());
    for line in &ed.lines {
        states.push(state);
        state = highlight(line, state).1;
    }
    let sel = ed.selection();
    for i in 0..rows {
        let r = ed.scroll + i;
        if r >= ed.lines.len() {
            break;
        }
        let ly = top + 3.0 + LINE_H * i as f32;
        let line = &ed.lines[r];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if ed.error_line == Some(r + 1) {
            game.fill([x, ly - 1.0, w, LINE_H], ERROR_ROW);
        }
        if let Some(((r1, c1), (r2, c2))) = sel
            && r >= r1
            && r <= r2
        {
            let a = if r == r1 { c1 } else { 0 };
            let b = if r == r2 { c2 } else { line.len() };
            let a = a.max(from);
            let b2 = b.max(a);
            let sx = text_x + game.text_width(&line[from..a]);
            let mut sw = game.text_width(&line[a..b2]);
            if r != r2 {
                sw += 3.0;
            }
            let sw = sw.min(text_x + text_w - sx).max(0.0);
            game.fill([sx, ly - 1.0, sw, LINE_H], SELECTION);
        }
        let num = format!("{}", r + 1);
        let nw = game.text_width(&num);
        let nrgb = if r == ed.row { INK_SOFT } else { MUTED };
        text(game, &num, x + gutter - nw, ly, nrgb);
        let (pieces, _) = highlight(line, states[r]);
        let mut px = text_x;
        for (a, b, tint) in pieces {
            if b <= from {
                continue;
            }
            let piece = &line[a.max(from)..b];
            let room = text_x + text_w - px;
            if room <= 0.0 {
                break;
            }
            let shown = if game.text_width(piece) > room {
                fit(game, piece, room)
            } else {
                piece.to_owned()
            };
            text(game, &shown, px, ly, tint.rgb());
            px += game.text_width(piece);
        }
        if r == ed.row && (st.s.now * 2.0) as i64 % 2 == 0 && ed.col >= from {
            let cx = text_x + game.text_width(&line[from..ed.col]);
            if cx <= text_x + text_w {
                game.fill([cx.floor(), ly - 1.0, 1.0, LINE_H], 0xFF00_0000 | INK);
            }
        }
    }
    if ed.lines.len() > rows {
        // A thin scroll bar.
        let frac = rows as f32 / ed.lines.len() as f32;
        let bar_h = (area_h * frac).max(6.0);
        let bar_y = top + (area_h - bar_h) * (ed.scroll as f32 / (ed.lines.len() - rows).max(1) as f32);
        game.fill([x + w - 2.0, bar_y, 2.0, bar_h], PLATE);
    }
    st.s.ui.view = Some(EditorView {
        x: text_x,
        y: top + 2.0,
        w: text_w,
        rows,
    });
}

fn state_label(state: &ModState, enabled: bool) -> (&'static str, u32) {
    match state {
        ModState::Running => ("running", SUCCESS),
        ModState::Failed(..) => ("failed", DANGER),
        ModState::Off if !enabled => ("off", MUTED),
        ModState::Off => ("not loaded", MUTED),
    }
}

fn draw_library(st: &mut Studio, game: &mut dyn GameApi, [x, y, w, h]: [f32; 4]) {
    let list_w = (w * 0.42).clamp(120.0, 200.0);
    let n = st.s.mods.len();
    if n > 0 {
        st.s.ui.library_sel = st.s.ui.library_sel.min(n - 1);
    }
    text(game, "Your script mods", x, y, MUTED);
    let mut ly = y + 13.0;
    if n == 0 {
        text(game, "None yet: make one, install an example,", x, ly, INK_SOFT);
        ly += 11.0;
        text(game, "or drop a .lua file on the game window.", x, ly, INK_SOFT);
        ly += 12.0;
    }
    let max_rows = (((h - 90.0) / 13.0).max(3.0)) as usize;
    let first = st.s.ui.library_sel.saturating_sub(max_rows.saturating_sub(1));
    for i in first..n.min(first + max_rows) {
        let m = &st.s.mods[i];
        let r = [x, ly - 2.0, list_w, 13.0];
        if i == st.s.ui.library_sel {
            game.fill(r, PLATE);
        } else if inside(r, st.s.ui.mouse) {
            game.fill(r, PLATE_SOFT);
        }
        let (_, dot) = state_label(&m.state, m.enabled);
        text(game, &format!("{:02}", i + 1), x + 3.0, ly, MUTED);
        game.fill([x + 18.0, ly + 2.0, 4.0, 4.0], 0xFF00_0000 | dot);
        let name = fit(game, &m.name, list_w - 60.0);
        text(game, &name, x + 26.0, ly, if m.enabled { INK } else { MUTED });
        let v = fit(game, &m.version, 30.0);
        let vw = game.text_width(&v);
        text(game, &v, x + list_w - vw - 3.0, ly, MUTED);
        game.fill([x, ly + 10.0, list_w, 1.0], RULE);
        st.s.ui.hits.push((r, Action::Select(i)));
        ly += 13.0;
    }
    ly += 4.0;
    let bw = st.s.ui.button(game, "New mod", x - 4.0, ly, ACCENT, Action::NewMod);
    let bw2 =
        st.s.ui
            .button(game, "Reload all", x - 4.0 + bw, ly, INK_SOFT, Action::ReloadAll);
    st.s.ui.button(
        game,
        "Open folder",
        x - 4.0 + bw + bw2,
        ly,
        INK_SOFT,
        Action::OpenFolder,
    );
    ly += 18.0;
    text(game, "Examples", x, ly, MUTED);
    ly += 13.0;
    let mut hovered: Option<&str> = None;
    for (i, ex) in EXAMPLES.iter().enumerate() {
        if ly > y + h - 10.0 {
            break;
        }
        let installed = st.s.mods.iter().any(|m| m.id == ex.id);
        if inside([x, ly - 2.0, list_w, 12.0], st.s.ui.mouse) {
            hovered = Some(ex.description);
        }
        let title = fit(game, ex.title, list_w - 60.0);
        text(game, &title, x, ly, INK_SOFT);
        if installed {
            let t = "installed";
            let tx = x + list_w - game.text_width(t) - 3.0;
            text(game, t, tx, ly, MUTED);
        } else {
            let t = "Install";
            let tx = x + list_w - game.text_width(t) - 11.0;
            st.s.ui.button(game, t, tx, ly, INK, Action::Install(i));
        }
        ly += 12.0;
    }
    // The chosen mod, or the example under the pointer.
    let dx = x + list_w + 14.0;
    let dw = w - list_w - 14.0;
    if let Some(desc) = hovered {
        for (i, l) in wrap(game, desc, dw).iter().enumerate() {
            text(game, l, dx, y + 11.0 * i as f32, INK_SOFT);
        }
        return;
    }
    let Some(m) = st.s.mods.get(st.s.ui.library_sel) else {
        let lines = wrap(
            game,
            "Drop a .lua file, a mod folder or a .zip of one on the game window to install it: it runs at once. Script mods live in horizon-mods/.horizon-mod-data/horizon_studio/mods/ (Open folder): a name.lua file, or a folder with main.lua. Save a file, from here or any editor, and the mod reloads at once.",
            dw,
        );
        for (i, l) in lines.iter().enumerate() {
            text(game, l, dx, y + 13.0 + 11.0 * i as f32, INK_SOFT);
        }
        return;
    };
    let i = st.s.ui.library_sel;
    let mut dy = y;
    let name = fit(game, &m.name, dw);
    text(game, &name, dx, dy, INK);
    dy += 12.0;
    let file = if m.folder.is_some() {
        format!("{}/main.lua", m.id)
    } else {
        format!("{}.lua", m.id)
    };
    let meta = [m.version.as_str(), m.author.as_str(), file.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("  ·  ");
    let meta = fit(game, &meta, dw);
    text(game, &meta, dx, dy, MUTED);
    dy += 14.0;
    for l in wrap(game, &m.description, dw).iter().take(4) {
        text(game, l, dx, dy, INK_SOFT);
        dy += 11.0;
    }
    let (label, rgb) = state_label(&m.state, m.enabled);
    let mut status = label.to_owned();
    if m.running() {
        status.push_str(&format!(
            "  ·  {:.2} ms a call  ·  {} handlers  ·  reloaded {}x",
            m.cost_ms,
            m.handlers.tick.len()
                + m.handlers.frame.len()
                + m.handlers.hud.len()
                + m.handlers.key.len()
                + m.handlers.chat.len(),
            m.reloads
        ));
    }
    let status = fit(game, &status, dw);
    text(game, &status, dx, dy, rgb);
    dy += 13.0;
    let mut bx = dx - 4.0;
    bx += st.s.ui.button(game, "Edit", bx, dy, ACCENT, Action::Edit(i));
    bx += st.s.ui.button(
        game,
        if m.enabled { "Turn off" } else { "Turn on" },
        bx,
        dy,
        INK_SOFT,
        Action::Toggle(i),
    );
    st.s.ui.button(game, "Reload", bx, dy, INK_SOFT, Action::Reload(i));
    dy += 17.0;
    if !m.commands.is_empty() {
        text(game, "Commands", dx, dy, MUTED);
        dy += 11.0;
        for c in m.commands.iter().take(6) {
            let line = if c.help.is_empty() {
                format!("/{}", c.name)
            } else {
                format!("/{}  {}", c.name, c.help)
            };
            let line = fit(game, &line, dw);
            text(game, &line, dx, dy, INK_SOFT);
            dy += 11.0;
        }
        dy += 3.0;
    }
    if let ModState::Failed(msg, trace) = &m.state {
        for l in wrap(game, msg, dw) {
            if dy > y + h - 10.0 {
                break;
            }
            text(game, &l, dx, dy, DANGER);
            dy += 11.0;
        }
        for l in wrap(game, trace, dw) {
            if dy > y + h - 10.0 {
                break;
            }
            text(game, &l, dx, dy, MUTED);
            dy += 10.0;
        }
    }
}

/// The Docs tab's text.
fn docs_text() -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = vec![
        ("Script mods".into(), INK),
        ("A script mod is a Lua file in horizon-mods/.horizon-mod-data/horizon_studio/mods/: name.lua, or a folder with main.lua and other files to require. The file runs when the mod loads; what it registers with on(), command() and every() then runs in the game. Save it, in the Editor or any other editor, and it reloads at once. A mod that fails stops alone: the Library shows the error and its line.".into(), INK_SOFT),
        ("Put -- name:, -- version:, -- author: and -- description: lines at the top to name it in the Library.".into(), INK_SOFT),
        (String::new(), INK),
        ("Console".into(), INK),
        ("Type Lua and press Enter: it runs at once in the game, and an expression's value is shown. /lua <code> does the same from chat. Things the console registers (on, command...) last until clear().".into(), INK_SOFT),
        (String::new(), INK),
        ("The language".into(), INK),
        ("Lua 5.3 without coroutines, goto and bitwise operators. Numbers are 64-bit floats. string (with Lua patterns, format, split, trim), math, table and os.clock/time/date are there; io and the rest of os are not. Each call into a script has a step budget, so an endless loop stops the script, not the game.".into(), INK_SOFT),
        (String::new(), INK),
        ("API".into(), INK),
    ];
    for (_, _, _, doc) in API {
        out.push(((*doc).to_owned(), INK_SOFT));
    }
    out.push((String::new(), INK));
    out.push(("Console only".into(), INK));
    for (_, _, doc) in CONSOLE_API {
        out.push(((*doc).to_owned(), INK_SOFT));
    }
    out.push((String::new(), INK));
    out.push(("Commands in chat".into(), INK));
    out.push((
        "/studio opens this screen, /studio reload reloads every script mod, /lua <code> runs code.".into(),
        INK_SOFT,
    ));
    out
}

fn draw_docs(st: &mut Studio, game: &mut dyn GameApi, [x, y, w, h]: [f32; 4]) {
    let mut lines: Vec<(String, u32)> = Vec::new();
    for (para, rgb) in docs_text() {
        if para.is_empty() {
            lines.push((String::new(), rgb));
            continue;
        }
        for l in wrap(game, &para, w - 6.0) {
            lines.push((l, rgb));
        }
    }
    let rows = (h / 11.0).max(1.0) as usize;
    st.s.ui.page = rows;
    st.s.ui.docs_scroll = st.s.ui.docs_scroll.min(lines.len().saturating_sub(rows));
    for (i, (l, rgb)) in lines.iter().skip(st.s.ui.docs_scroll).take(rows).enumerate() {
        text(game, l, x, y + 11.0 * i as f32, *rgb);
    }
}

// ----------------------------------------------------------------- input

pub fn handle(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    match input {
        Input::Mouse(x, y) => {
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
fn editor_point(st: &mut Studio, game: &dyn GameApi, x: f32, y: f32, drag: bool) -> bool {
    let Some(v) = st.s.ui.view else {
        return false;
    };
    if !drag && (x < v.x - 30.0 || y < v.y || y > v.y + LINE_H * v.rows as f32 || x > v.x + v.w + 4.0) {
        return false;
    }
    let ed = &mut st.s.editor;
    let row_in_view = ((y - v.y) / LINE_H).floor().max(0.0) as usize;
    let row = (ed.scroll + row_in_view.min(v.rows)).min(ed.lines.len() - 1);
    let col = ed.hscroll + column_at(game, &ed.lines[row], ed.hscroll, x - v.x);
    let anchor = if drag { ed.anchor } else { None };
    ed.click(row, col, drag);
    ed.anchor = Some(anchor.unwrap_or((ed.row, ed.col)));
    true
}

/// The char column nearest `dx` GUI pixels into `line`, past `skip` chars.
fn column_at(game: &dyn GameApi, line: &str, skip: usize, dx: f32) -> usize {
    let from = line.char_indices().nth(skip).map_or(line.len(), |(i, _)| i);
    let mut prev = 0.0;
    for (n, (i, c)) in line[from..].char_indices().enumerate() {
        let w = game.text_width(&line[from..from + i + c.len_utf8()]);
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
        Action::NewMod => st.s.ui.naming = Some(LineEdit::default()),
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
