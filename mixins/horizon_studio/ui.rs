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
    Settings,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Console, "Console"),
    (Tab::Editor, "Editor"),
    (Tab::Library, "Library"),
    (Tab::Docs, "Docs"),
    (Tab::Settings, "Settings"),
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
    /// The executor styles' Save File: the script mod open saved, else the
    /// code saved as a new script mod (its name asked).
    SaveFile,
    /// The scratch file opened in the editor (the script list's first row).
    EditScratch,
    Run,
    OpenFolder,
    /// Copies the prompt that sets an AI agent up (`hstudio_link::SETUP_PROMPT`).
    CopyPrompt,
    /// The window: its title bar (drag), its corner (resize), its buttons.
    WinMove,
    WinResize,
    WinMin,
    WinMax,
    /// Settings: a theme by index, full screen or a window, an accent,
    /// a setting a step up or down (0 size, 1 backdrop, 2 code), motion,
    /// the window back where it started.
    Theme(u8),
    NextTheme,
    Windowed(bool),
    Accent(u8),
    Step(u8, bool),
    Motion,
    ResetWindow,
    /// Settings shown, or back to the editor (the executor styles).
    Settings,
    ClearEditor,
    /// A suggestion taken.
    Suggest(u8),
    /// The editor's bottom panel: a tab of it, shown or hidden, sized from its
    /// edge (a drag), focused; a problem's line.
    PanelTab(Panel),
    TogglePanel,
    PanelResize,
    FocusPanel,
    Problem(usize),
    /// The editor's file changed on disk while edited: take that, or keep these.
    ReloadFile,
    KeepMine,
    /// The agent: the next one found, or stop it.
    AgentNext,
    AgentStop,
    ClearLog,
    /// Over something that takes no click (it doesn't reach what is under it).
    Block,
    /// The editor's scroll bar: its thumb dragged, or its track clicked.
    ScrollBar,
    /// An item of the open menu, by its place.
    MenuPick(u8),
    /// The agent's model chip: its menu of models.
    ModelMenu,
}

/// What a menu item does when chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuCmd {
    /// The agent's model (None: the agent's own default).
    Model(Option<String>),
    /// A model typed by name.
    CustomModel,
}

/// A row of a menu: its label, a hint at the right (a shortcut), whether it
/// can be chosen and is the current choice, or a separator line.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub hint: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
    pub cmd: Option<MenuCmd>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>, cmd: MenuCmd) -> MenuItem {
        MenuItem {
            label: label.into(),
            hint: String::new(),
            enabled: true,
            checked: false,
            separator: false,
            cmd: Some(cmd),
        }
    }
    pub fn separator() -> MenuItem {
        MenuItem {
            label: String::new(),
            hint: String::new(),
            enabled: false,
            checked: false,
            separator: true,
            cmd: None,
        }
    }
    pub fn checked(mut self, on: bool) -> MenuItem {
        self.checked = on;
        self
    }
}

/// A menu open over everything: where it opens (its top left, kept on the
/// screen when drawn), its items, the one the keys are on, when it opened,
/// and where it was drawn (clicks outside it close it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub at: [f32; 2],
    pub items: Vec<MenuItem>,
    pub sel: Option<usize>,
    pub opened_at: f64,
    pub rect: Option<[f32; 4]>,
}

/// What the naming prompt names: a new mod, the editor's code saved as a
/// new mod (Save File), or the agent's model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NamingKind {
    #[default]
    NewMod,
    SaveAs,
    Model,
}

/// The editor's scroll bar as drawn: where it is across, its track's top and height, the thumb's
/// height and top, and the lines it scrolls through.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollBar {
    /// The middle of its track, across.
    pub x: f32,
    pub top: f32,
    pub track: f32,
    pub bar: f32,
    pub span: f32,
    pub at: f32,
}

/// The suggestions shown as you type: under the editor's cursor, or over
/// the console's input.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggest {
    pub items: Vec<super::hstudio_assist::Suggestion>,
    pub sel: usize,
    /// Where the word it replaces starts (a byte of the line or input).
    pub start: usize,
    pub console: bool,
    /// When it opened (its motion), and whether the arrows moved it (then
    /// Enter takes the suggestion, not a new line).
    pub at: f64,
    pub moved: bool,
    /// The first one the list shows (it scrolls to keep `sel` in view).
    pub(crate) first: usize,
}

/// The editor's bottom panel, as in VS Code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Panel {
    #[default]
    Console,
    Problems,
    Agent,
}

/// A window being moved or sized: how, from where the pointer was, and the
/// window then.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WinDrag {
    resize: bool,
    from: (f32, f32),
    rect: [f32; 4],
}

/// Where the editor's text was drawn, for the pointer.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EditorView {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) rows: usize,
}

/// The console's lines and the agent's: the areas text is selected in.
pub(crate) const AREA_LOG: u8 = 0;
pub(crate) const AREA_AGENT: u8 = 1;

/// A row of selectable text as drawn: its area, its line's number and its
/// place among the line's wrapped pieces, where it is, its text, what came
/// after it (a line break, a space or nothing) and its char edges (x from
/// its start; empty when not measured).
#[derive(Clone, Debug)]
pub(crate) struct TextRow {
    pub area: u8,
    pub key: (u64, usize),
    pub x: f32,
    pub y: f32,
    pub h: f32,
    pub text: String,
    pub after: &'static str,
    pub edges: Vec<f32>,
}

/// A place in selectable text: a row's key and a byte of its text.
pub(crate) type TextPos = ((u64, usize), usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextSel {
    pub area: u8,
    pub anchor: TextPos,
    pub head: TextPos,
}

impl TextSel {
    fn range(&self) -> (TextPos, TextPos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// The selected bytes of row `key` (`len` long) in `area`, if any.
pub(crate) fn selected_cols(ui: &Ui, area: u8, key: (u64, usize), len: usize) -> Option<(usize, usize)> {
    let sel = ui.text_sel.filter(|s| s.area == area && s.anchor != s.head)?;
    let ((k1, c1), (k2, c2)) = sel.range();
    if key < k1 || key > k2 {
        return None;
    }
    let a = if key == k1 { c1.min(len) } else { 0 };
    let b = if key == k2 { c2.min(len) } else { len };
    Some((a, b.max(a)))
}

/// Whether the selection goes on past row `key` (its line break is selected too).
pub(crate) fn selection_goes_on(ui: &Ui, area: u8, key: (u64, usize)) -> bool {
    ui.text_sel
        .filter(|s| s.area == area)
        .is_some_and(|s| s.range().1.0 > key)
}

/// The place in selectable text under `(x, y)`: in the area there, the row
/// at that height (above the rows: the first, from its start; below: the
/// last, to its end), the char edge nearest `x`. With `area`, only there
/// (a drag goes on in the area it began in, wherever the pointer goes).
fn text_pos_at(ui: &Ui, (x, y): (f32, f32), area: Option<u8>) -> Option<(u8, TextPos)> {
    let area = match area {
        Some(a) => a,
        None => ui.text_areas.iter().rev().find(|(_, r)| inside(*r, (x, y)))?.0,
    };
    let mut rows: Vec<&TextRow> = ui.text_rows.iter().filter(|r| r.area == area).collect();
    rows.sort_by_key(|r| r.key);
    let (first, last) = (*rows.first()?, *rows.last()?);
    let row = match rows.iter().find(|r| y >= r.y && y < r.y + r.h) {
        Some(r) => *r,
        None if y < first.y => return Some((area, (first.key, 0))),
        None if y >= last.y + last.h => return Some((area, (last.key, last.text.len()))),
        // Between rows (a gap): the next one down.
        None => *rows.iter().filter(|r| r.y > y).min_by(|a, b| a.y.total_cmp(&b.y))?,
    };
    let dx = x - row.x;
    let col = if row.edges.len() > 1 {
        let n = row
            .edges
            .windows(2)
            .position(|e| dx < (e[0] + e[1]) / 2.0)
            .unwrap_or(row.edges.len() - 1);
        row.text.char_indices().nth(n).map_or(row.text.len(), |(i, _)| i)
    } else if dx <= 0.0 {
        0
    } else {
        row.text.len()
    };
    Some((area, (row.key, col)))
}

/// The selected text, as it was written: wrapped pieces joined by what
/// separated them. Only rows drawn this frame are known.
pub(crate) fn selected_text(ui: &Ui) -> Option<String> {
    let sel = ui.text_sel.filter(|s| s.anchor != s.head)?;
    let mut rows: Vec<&TextRow> = ui.text_rows.iter().filter(|r| r.area == sel.area).collect();
    rows.sort_by_key(|r| r.key);
    rows.dedup_by(|a, b| a.key == b.key);
    let mut out = String::new();
    let mut prev: Option<&TextRow> = None;
    for row in rows {
        let Some((a, b)) = selected_cols(ui, sel.area, row.key, row.text.len()) else {
            continue;
        };
        if let Some(p) = prev {
            out.push_str(if p.key.0 != row.key.0 { "\n" } else { p.after });
        }
        out.push_str(&row.text[a..b]);
        prev = Some(row);
    }
    (!out.is_empty()).then_some(out)
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
    /// What the prompt names.
    pub naming_kind: NamingKind,
    /// The menu open over everything, if one is.
    pub menu: Option<Menu>,
    /// Where the agent's model chip was drawn (its menu opens under it).
    pub(crate) model_chip: Option<[f32; 4]>,
    /// When the AI setup prompt was copied (its button says so a moment).
    pub copied_at: Option<f64>,
    /// The looks chosen in Settings (`look.txt`).
    pub look: super::hstudio_theme::Look,
    /// The window rolled up to its title bar.
    pub collapsed: bool,
    pub(crate) drag: Option<WinDrag>,
    /// The screen in the studio's units, from the last frame.
    pub(crate) screen: (f32, f32),
    pub(crate) settings_scroll: f32,
    pub suggest: Option<Suggest>,
    /// The editor's bottom panel: which tab, shown, focused (typing goes to
    /// it), its share of the editor's height, being sized (from where).
    pub panel: Panel,
    pub panel_hidden: bool,
    pub panel_focus: bool,
    pub panel_h: f32,
    pub(crate) panel_drag: Option<(f32, f32)>,
    /// The height the panel's share is of, from the last frame.
    pub(crate) panel_room: f32,
    /// What the agent is asked, typed in its panel.
    pub agent_input: LineEdit,
    pub(crate) problem_sel: usize,
    pub agent: AgentPane,
    /// The pointer, in dp.
    pub mouse: (f32, f32),
    /// Whether the left and right buttons are held (over a script screen).
    pub buttons: [bool; 2],
    dragging: bool,
    pub(crate) hits: Vec<([f32; 4], Action)>,
    /// Selectable text drawn this frame (the console's and the agent's), where
    /// it was, what is selected in it, and whether the pointer is selecting.
    pub(crate) text_rows: Vec<TextRow>,
    pub(crate) text_areas: Vec<(u8, [f32; 4])>,
    pub(crate) text_sel: Option<TextSel>,
    pub(crate) text_drag: bool,
    /// The editor's scroll bar, from the last frame, and where its thumb is
    /// held (from its top) while dragged.
    pub scrollbar: Option<ScrollBar>,
    /// The studio's window isn't drawn until then (the studio's clock): a
    /// screenshot for the agent shows the game, not the editor over it.
    pub hidden_until: f64,
    pub(crate) scroll_drag: Option<f32>,
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
    pub(crate) smooth: [f32; 8],
    /// The editor's caret as drawn (it glides), where it is going, and when
    /// it last moved (it blinks only when still).
    pub(crate) caret: Option<[f32; 2]>,
    pub(crate) caret_to: [f32; 2],
    pub(crate) caret_moved: f64,
    /// When the editor's file was last looked at on disk.
    pub(crate) disk_at: f64,
    /// Where the bottom panel was drawn (None: it wasn't), for the wheel.
    pub(crate) panel_at: Option<[f32; 4]>,
    /// When each of the editor's problems first showed (its fade in).
    pub(crate) problem_at: std::collections::HashMap<u64, f64>,
    /// The window as drawn: it glides to where it is going (maximised,
    /// restored, the screen resized).
    pub(crate) win_shown: Option<[f32; 4]>,
    /// When the bottom panel's tab last changed (its content's motion).
    pub(crate) panel_tab_at: f64,
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
    /// The cursor (row, col) and revision the view last scrolled to: it
    /// follows the cursor only when one of them changes, so the wheel can
    /// look elsewhere without moving the cursor or the selection.
    pub followed: Option<(usize, usize, u64)>,
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
    /// Bumped by every edit: the live check runs on a new one.
    pub revision: u64,
    /// The revision `problems` are of.
    pub checked: Option<u64>,
    /// The live check's findings (syntax errors, unknown names).
    pub problems: Vec<super::hstudio_assist::Problem>,
    /// When it was last edited (the studio's clock): the check waits for a pause.
    pub edited_at: f64,
    /// The file as last read or written (size, time), to see others' changes.
    pub disk: Option<(u64, Option<std::time::SystemTime>)>,
    /// The file changed on disk while this buffer had unsaved edits.
    pub conflict: bool,
    /// When the file was read again after a change elsewhere (an agent's).
    pub reloaded_at: Option<f64>,
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
            followed: None,
            dirty: false,
            error_line: None,
            goal: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_kind: None,
            problem: None,
            revision: 0,
            checked: None,
            problems: Vec::new(),
            edited_at: 0.0,
            disk: None,
            conflict: false,
            reloaded_at: None,
        }
    }
}

/// A file's size and time, to see when it changes.
fn disk_stamp(path: &Path) -> Option<(u64, Option<std::time::SystemTime>)> {
    std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))
}

/// The closer of an opening bracket or quote.
fn closer(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' | '\'' => Some(c),
        _ => None,
    }
}

/// A line's code: strings blanked, its comment cut (byte for byte).
fn code_of(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                out.extend(std::iter::repeat_n(' ', c.len_utf8()));
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        out.extend(std::iter::repeat_n(' ', n.len_utf8()));
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '-' && chars.peek() == Some(&'-') => break,
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    out
}

/// Whether the end of `before` is inside a string.
fn in_string(before: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut chars = before.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == '\\' => {
                let _ = (q, chars.next());
            }
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None => {}
        }
    }
    quote.is_some()
}

/// What a line of code opens and closes, in order: +1 with its closer for
/// `function`, `do`, `then`, `repeat` and brackets; -1 for `end`, `until`,
/// `elseif` (whose own `then` opens again) and closing brackets.
fn block_marks(code: &str) -> Vec<(i32, &'static str)> {
    let mut out = Vec::new();
    let mut word = String::new();
    for c in code.chars().chain([' ']) {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        match word.as_str() {
            "function" | "do" | "then" => out.push((1, "end")),
            "repeat" => out.push((1, "until")),
            "end" | "until" | "elseif" => out.push((-1, "")),
            _ => {}
        }
        word.clear();
        match c {
            '(' => out.push((1, ")")),
            '{' => out.push((1, "}")),
            '[' => out.push((1, "]")),
            ')' | '}' | ']' => out.push((-1, "")),
            _ => {}
        }
    }
    out
}

/// The blocks lines `0..row` leave open, innermost last: the indent of the
/// line that opened each, and what closes it.
fn open_blocks(lines: &[String], row: usize) -> Vec<(usize, &'static str)> {
    let mut stack: Vec<(usize, &'static str)> = Vec::new();
    for line in &lines[..row.min(lines.len())] {
        let indent = line.len() - line.trim_start().len();
        for (d, close) in block_marks(&code_of(line)) {
            if d > 0 {
                stack.push((indent, close));
            } else {
                stack.pop();
            }
        }
    }
    stack
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
            disk: disk_stamp(path),
            ..Editor::default()
        };
        self.set_text(&text);
    }
    /// The file it shows moved (renamed, or a script made a module's
    /// main.lua): the same text, cursor, undo and unsaved edits, at its new
    /// path, so a save writes there and not where it was.
    pub fn moved(&mut self, to: &Path, title: &str) {
        self.path = Some(to.to_path_buf());
        self.title = title.to_owned();
        self.disk = disk_stamp(to);
        self.conflict = false;
        // Its name is in what the live check says: it looks again.
        self.checked = None;
    }
    /// Reads the file again when it changed on disk (an agent wrote it):
    /// at once while there are no unsaved edits (the cursor stays where it
    /// was, and Ctrl+Z goes back), else it is marked as a conflict. True
    /// when it read it.
    pub fn sync_disk(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let now = disk_stamp(&path);
        if now.is_none() || now == self.disk {
            return false;
        }
        self.disk = now;
        if self.dirty {
            self.conflict = true;
            return false;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        if text.replace("\r\n", "\n") == self.text() {
            return false;
        }
        let scroll = self.scroll;
        self.begin(EditKind::Other);
        self.set_text(&text);
        self.dirty = false;
        self.conflict = false;
        self.anchor = None;
        self.scroll = scroll.min(self.lines.len().saturating_sub(1));
        true
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
        // Whatever set it, the live check looks again.
        self.revision += 1;
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
        self.conflict = false;
        self.disk = disk_stamp(&path);
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
        self.revision += 1;
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
        self.revision += 1;
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
        let after = line[self.col..].to_owned();
        let opens = ["then", "do", "else", "repeat", "{", "(", "["]
            .iter()
            .any(|k| before.ends_with(k))
            || (before.contains("function") && before.ends_with(')'))
            || before.ends_with("function()");
        // Between a bracket and its closer: the closer goes a line down.
        let between = before
            .chars()
            .last()
            .filter(|c| !matches!(c, '"' | '\''))
            .and_then(closer)
            .is_some_and(|c| after.trim_start().starts_with(c));
        if between {
            let rest = after.trim_start().to_owned();
            self.begin(EditKind::Other);
            self.delete_selection();
            let line = &mut self.lines[self.row];
            line.truncate(self.col);
            let inner = format!("{indent}{INDENT}");
            self.lines.insert(self.row + 1, inner.clone());
            self.lines.insert(self.row + 2, format!("{indent}{rest}"));
            self.row += 1;
            self.col = inner.len();
            return;
        }
        let extra = if opens { INDENT } else { "" };
        // A block this line opens that nothing closes: its closer (and the
        // call it is in: `on("tick", function()` takes `end)`) a line down.
        let close = if opens && after.trim().is_empty() {
            self.missing_closer()
        } else {
            None
        };
        self.insert(&format!("\n{indent}{extra}"));
        if let Some(close) = close {
            self.lines.insert(self.row + 1, format!("{indent}{close}"));
        }
    }

    /// The closer the cursor's line needs when the file leaves the block it
    /// opens unclosed: `end` or `until `, then the closers of the brackets the
    /// line leaves open (none when the file has as many closers as openers).
    fn missing_closer(&self) -> Option<String> {
        let line = &self.lines[self.row];
        let mut stack: Vec<&'static str> = Vec::new();
        for (d, close) in block_marks(&code_of(&line[..self.col])) {
            if d > 0 {
                stack.push(close);
            } else {
                stack.pop();
            }
        }
        let first = stack.iter().rposition(|c| *c == "end" || *c == "until")?;
        let unclosed = open_blocks(&self.lines, self.lines.len());
        if !unclosed.iter().any(|b| b.1 == stack[first]) {
            return None;
        }
        let mut out = stack[first].to_owned();
        if out == "until" {
            out.push(' ');
        }
        for c in stack[..first].iter().rev() {
            if c.len() == 1 {
                out.push_str(c);
            }
        }
        Some(out)
    }

    /// Types `s` as from the keyboard: brackets and quotes close themselves
    /// (and wrap a selection), a closer that is next is typed over, and a line
    /// that starts with `end`, `else`, `elseif`, `until` or a closer goes back
    /// to its block's indent.
    pub fn type_text(&mut self, s: &str) {
        let mut chars = s.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return self.insert(s);
        };
        if let (Some(_), Some(close)) = (self.selection(), closer(c)) {
            let inner = self.selected_text().unwrap_or_default();
            self.insert(&format!("{c}{inner}{close}"));
            return;
        }
        let line = &self.lines[self.row];
        let (before, after) = (&line[..self.col], &line[self.col..]);
        let next = after.chars().next();
        let prev = before.chars().last();
        let quoting = in_string(before);
        // Typing over the closer already there.
        let over = next == Some(c)
            && match c {
                ')' | ']' | '}' => !quoting,
                '"' | '\'' => quoting,
                _ => false,
            };
        if over {
            self.anchor = None;
            self.col += c.len_utf8();
            return;
        }
        let pair = closer(c).filter(|_| {
            let free_after = next.is_none_or(|n| n.is_whitespace() || ")]},".contains(n));
            let quote_ok = !matches!(c, '"' | '\'') || !prev.is_some_and(|p| p.is_alphanumeric() || p == '_');
            free_after && quote_ok && !quoting
        });
        match pair {
            Some(close) => {
                self.insert(&format!("{c}{close}"));
                self.col -= close.len_utf8();
            }
            None => self.insert(s),
        }
        self.dedent_closer();
    }

    /// A line that is just a closer (`end`, `else`, `}`...) goes to the
    /// indent of the block it closes.
    fn dedent_closer(&mut self) {
        let line = &self.lines[self.row];
        let word = line.trim_start();
        let indent = line.len() - word.len();
        if !matches!(word, "end" | "else" | "elseif" | "until" | "}" | ")" | "]") {
            return;
        }
        let Some(&(target, _)) = open_blocks(&self.lines, self.row).last() else {
            return;
        };
        if target < indent {
            let cut = indent - target;
            self.lines[self.row].replace_range(..cut, "");
            self.col = self.col.saturating_sub(cut);
        }
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
        // An empty pair goes together.
        let (prev, next) = (line[..self.col].chars().last(), line[self.col..].chars().next());
        if !word && prev.and_then(closer).is_some_and(|c| Some(c) == next) {
            let to = prev_char(line, self.col);
            let end = self.col + next.map_or(0, char::len_utf8);
            self.lines[self.row].replace_range(to..end, "");
            self.col = to;
            return;
        }
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
            | "model"
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

pub use super::hstudio_look::{code_line, draw, draw_screen};

// ----------------------------------------------------------------- input

pub fn handle(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    let revision = st.s.editor.revision;
    let out = handle_input(st, game, input);
    if st.s.editor.revision != revision {
        st.s.editor.edited_at = st.s.now;
    }
    out
}

/// Where typing goes now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Editor,
    Console,
    Agent,
    Elsewhere,
}

pub fn target(ui: &Ui) -> Target {
    match ui.tab {
        Tab::Console => Target::Console,
        Tab::Editor
            if ui.panel_focus && !ui.panel_hidden && ui.look.theme().layout == super::hstudio_theme::Layout::Studio =>
        {
            match ui.panel {
                Panel::Console => Target::Console,
                Panel::Agent => Target::Agent,
                Panel::Problems => Target::Elsewhere,
            }
        }
        Tab::Editor => Target::Editor,
        _ => Target::Elsewhere,
    }
}

/// Opens, updates or closes the suggestions for what is typed now.
pub fn refresh_suggest(st: &mut Studio) {
    let console = match target(&st.s.ui) {
        Target::Console => true,
        Target::Editor => false,
        _ => {
            st.s.ui.suggest = None;
            return;
        }
    };
    let found = if console {
        let i = &st.s.ui.input;
        super::hstudio_assist::suggest(&i.text[..i.cursor], &[], true)
    } else {
        let ed = &st.s.editor;
        if ed.selection().is_some() {
            None
        } else {
            super::hstudio_assist::suggest(&ed.lines[ed.row][..ed.col], &ed.lines, false)
        }
    };
    let ui = &mut st.s.ui;
    ui.suggest = match found {
        Some((start, items)) if !items.is_empty() => {
            let at = ui
                .suggest
                .as_ref()
                .filter(|s| s.console == console && s.start == start)
                .map_or(st.s.now, |s| s.at);
            Some(Suggest {
                items,
                sel: 0,
                start,
                console,
                at,
                moved: false,
                first: 0,
            })
        }
        _ => None,
    };
}

/// Takes suggestion `i`: it replaces the word typed; a function gets its
/// brackets (the cursor in them), a library its dot (and its members).
pub fn accept_suggestion(st: &mut Studio, i: usize) {
    use super::hstudio_assist::Kind;
    let Some(sg) = st.s.ui.suggest.take() else { return };
    let Some(item) = sg.items.get(i).cloned() else { return };
    if sg.console {
        let input = &mut st.s.ui.input;
        let end = input.cursor.max(sg.start);
        input.text.replace_range(sg.start.min(end)..end, &item.insert);
        input.cursor = sg.start + item.insert.len();
        let next = input.text[input.cursor..].chars().next();
        match item.kind {
            Kind::Function if next != Some('(') => {
                input.insert("()");
                input.cursor -= 1;
            }
            Kind::Namespace if next != Some('.') => input.insert("."),
            _ => {}
        }
    } else {
        let ed = &mut st.s.editor;
        ed.anchor = Some((ed.row, sg.start.min(ed.col)));
        ed.insert(&item.insert);
        let next = ed.lines[ed.row][ed.col..].chars().next();
        match item.kind {
            Kind::Function if next != Some('(') => ed.type_text("("),
            Kind::Namespace if next != Some('.') => ed.type_text("."),
            _ => {}
        }
    }
    if item.kind == Kind::Namespace {
        refresh_suggest(st);
    }
}

/// A key while suggestions show: true when it was theirs.
fn suggest_key(st: &mut Studio, key: Key) -> bool {
    let Some(sg) = &mut st.s.ui.suggest else { return false };
    let n = sg.items.len().max(1);
    match key {
        Key::Up => {
            sg.sel = (sg.sel + n - 1) % n;
            sg.moved = true;
        }
        Key::Down => {
            sg.sel = (sg.sel + 1) % n;
            sg.moved = true;
        }
        Key::PageUp => {
            sg.sel = sg.sel.saturating_sub(8);
            sg.moved = true;
        }
        Key::PageDown => {
            sg.sel = (sg.sel + 8).min(n - 1);
            sg.moved = true;
        }
        Key::Tab => {
            let i = sg.sel;
            accept_suggestion(st, i);
        }
        Key::Enter if sg.moved => {
            let i = sg.sel;
            accept_suggestion(st, i);
        }
        Key::Escape => st.s.ui.suggest = None,
        Key::Backspace | Key::Delete => return false,
        _ => {
            st.s.ui.suggest = None;
            return false;
        }
    }
    true
}

fn handle_input(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    match input {
        Input::Mouse(x, y) => {
            // GUI pixels to dp, then to the studio's units (its size setting).
            let k = st.s.ui.dp_factor() / st.s.ui.look.scale;
            let (x, y) = (x * k, y * k);
            st.s.ui.mouse = (x, y);
            if let Some((from, h)) = st.s.ui.panel_drag {
                let room = st.s.ui.panel_room.max(1.0);
                st.s.ui.panel_h = (h + (from - y) / room).clamp(0.15, 0.8);
                return UiResult::Used;
            }
            if let Some(grab) = st.s.ui.scroll_drag {
                scroll_to_thumb(st, y - grab);
                return UiResult::Used;
            }
            if let Some(d) = st.s.ui.drag {
                let (dx, dy) = (x - d.from.0, y - d.from.1);
                let [rx, ry, rw, rh] = d.rect;
                let (sw, sh) = st.s.ui.screen;
                let look = &mut st.s.ui.look;
                look.maximized = false;
                // Kept on the screen as it goes, so it comes back at once.
                look.rect = Some(super::hstudio_theme::fit(
                    if d.resize {
                        [rx, ry, (rw + dx).min(sw - rx), (rh + dy).min(sh - ry)]
                    } else {
                        [rx + dx, ry + dy, rw, rh]
                    },
                    sw,
                    sh,
                ));
                return UiResult::Used;
            }
            if st.s.ui.text_drag
                && let Some(sel) = st.s.ui.text_sel
                && let Some((_, head)) = text_pos_at(&st.s.ui, (x, y), Some(sel.area))
            {
                st.s.ui.text_sel = Some(TextSel { head, ..sel });
            }
            if st.s.ui.dragging {
                let drawn = st.s.ui.smooth[1];
                editor_point(st, game, x, y, true, drawn);
            }
            UiResult::Used
        }
        Input::Button { left, down } => {
            if !down {
                st.s.ui.scroll_drag = None;
                st.s.ui.text_drag = false;
                if st.s.ui.text_sel.is_some_and(|s| s.anchor == s.head) {
                    st.s.ui.text_sel = None;
                }
                st.s.ui.dragging = false;
                st.s.ui.panel_drag = None;
                if st.s.ui.drag.take().is_some() {
                    st.s.ui.look.save(&st.s.dir);
                }
                return UiResult::Used;
            }
            if !left || st.s.ui.naming.is_some() {
                // Any press outside an open menu closes it, and goes no further.
                if !inside(
                    st.s.ui.menu.as_ref().and_then(|m| m.rect).unwrap_or_default(),
                    st.s.ui.mouse,
                ) {
                    st.s.ui.menu = None;
                }
                return UiResult::Used;
            }
            let at = st.s.ui.mouse;
            if let Some(menu) = &st.s.ui.menu
                && !menu.rect.is_some_and(|r| inside(r, at))
            {
                st.s.ui.menu = None;
                return UiResult::Used;
            }
            let hit = st.s.ui.hits.iter().rev().find(|(r, _)| inside(*r, at)).map(|(_, a)| *a);
            // A press on the console's or the agent's text starts selecting it;
            // anywhere else, what was selected there is let go.
            st.s.ui.text_sel = None;
            if matches!(hit, None | Some(Action::FocusPanel))
                && let Some((area, pos)) = text_pos_at(&st.s.ui, at, None)
            {
                st.s.ui.text_sel = Some(TextSel {
                    area,
                    anchor: pos,
                    head: pos,
                });
                st.s.ui.text_drag = true;
            }
            if let Some(action @ (Action::WinMove | Action::WinResize)) = hit {
                let (w, h) = st.s.ui.screen;
                let look = &mut st.s.ui.look;
                let mut rect = look.window_rect(w, h);
                if look.maximized {
                    // Dragged out of full size: its own size again, under the pointer.
                    look.maximized = false;
                    let own = look.window_rect(w, h);
                    rect = [at.0 - own[2] / 2.0, at.1 - 15.0, own[2], own[3]];
                    look.rect = Some(rect);
                }
                st.s.ui.drag = Some(WinDrag {
                    resize: action == Action::WinResize,
                    from: at,
                    rect,
                });
                return UiResult::Used;
            }
            if let (Some(Action::ScrollBar), Some(sb)) = (hit, st.s.ui.scrollbar) {
                // Held where it was pressed on the thumb; pressed on the track, the
                // thumb comes to the pointer, centred.
                let grab = if at.1 >= sb.at && at.1 <= sb.at + sb.bar {
                    at.1 - sb.at
                } else {
                    sb.bar / 2.0
                };
                st.s.ui.scroll_drag = Some(grab);
                scroll_to_thumb(st, at.1 - grab);
                return UiResult::Used;
            }
            if let Some(Action::PanelResize) = hit {
                st.s.ui.panel_drag = Some((at.1, st.s.ui.panel_h));
                return UiResult::Used;
            }
            if !matches!(hit, Some(Action::Suggest(_))) {
                st.s.ui.suggest = None;
            }
            if let Some(action) = hit {
                return act(st, game, action);
            }
            let drawn = st.s.ui.smooth[1];
            if st.s.ui.tab == Tab::Editor && editor_point(st, game, at.0, at.1, false, drawn) {
                st.s.ui.dragging = true;
                st.s.ui.panel_focus = false;
            }
            UiResult::Used
        }
        Input::Wheel(steps) => {
            let n = (steps.abs().ceil() as usize).max(1) * 3;
            let up = steps > 0.0;
            if st.s.ui.tab == Tab::Editor
                && let Some(r) = st.s.ui.panel_at
                && inside(r, st.s.ui.mouse)
            {
                let ui = &mut st.s.ui;
                match ui.panel {
                    Panel::Console if up => ui.log_scroll += n,
                    Panel::Console => ui.log_scroll = ui.log_scroll.saturating_sub(n),
                    Panel::Agent if up => ui.agent.scroll += n as f32,
                    Panel::Agent => ui.agent.scroll = (ui.agent.scroll - n as f32).max(0.0),
                    Panel::Problems => {
                        let last = st.s.editor.problems.len().saturating_sub(1);
                        ui.problem_sel = if up {
                            ui.problem_sel.saturating_sub(n / 3)
                        } else {
                            (ui.problem_sel + n / 3).min(last)
                        };
                    }
                }
                return UiResult::Used;
            }
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
                    // The cursor and the selection stay where they are; a
                    // drag in progress follows the pointer onto the new lines.
                    if st.s.ui.dragging {
                        let (x, y) = st.s.ui.mouse;
                        let top = st.s.editor.scroll as f32;
                        editor_point(st, game, x, y, true, top);
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
                Tab::Settings => {
                    let by = 40.0 * n as f32 / 3.0;
                    let s = &mut st.s.ui.settings_scroll;
                    *s = if up { (*s - by).max(0.0) } else { *s + by };
                }
            }
            UiResult::Used
        }
        Input::Text(t) => {
            if let Some(name) = &mut st.s.ui.naming {
                name.insert(&t);
                return UiResult::Used;
            }
            // What opens or keeps suggestions: a name's letters, a dot, a
            // colon, a quote (events in on("...")).
            let suggests = t
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '"' | '\''));
            match target(&st.s.ui) {
                Target::Console => {
                    st.s.ui.input.insert(&t);
                    st.s.ui.hist = None;
                    if suggests {
                        refresh_suggest(st);
                    } else {
                        st.s.ui.suggest = None;
                    }
                    return UiResult::Used;
                }
                Target::Agent => {
                    st.s.ui.agent_input.insert(&t);
                    return UiResult::Used;
                }
                Target::Editor => {
                    st.s.editor.type_text(&t);
                    if suggests {
                        refresh_suggest(st);
                    } else {
                        st.s.ui.suggest = None;
                    }
                    return UiResult::Used;
                }
                Target::Elsewhere => {}
            }
            match st.s.ui.tab {
                Tab::Console | Tab::Editor => {}
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
                Tab::Settings => {}
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
fn editor_point(st: &mut Studio, game: &mut dyn GameApi, x: f32, y: f32, drag: bool, drawn: f32) -> bool {
    let Some(v) = st.s.ui.view else {
        return false;
    };
    if !drag && (x < v.x - 30.0 || y < v.y || y > v.y + code_line() * v.rows as f32 || x > v.x + v.w + 4.0) {
        return false;
    }
    // The line drawn under the pointer: rows glide as the view scrolls, so
    // the view as drawn (not where it is going) says which, given as `drawn`.
    let scale = st.s.ui.look.scale;
    let ed = &mut st.s.editor;
    let at = drawn.max(0.0) + ((y - v.y) / code_line()).clamp(0.0, v.rows as f32);
    let row = (at.floor() as usize).min(ed.lines.len() - 1);
    let col = ed.hscroll + column_at(game, &ed.lines[row], ed.hscroll, x - v.x, scale);
    let anchor = if drag { ed.anchor } else { None };
    ed.click(row, col, drag);
    ed.anchor = Some(anchor.unwrap_or((ed.row, ed.col)));
    // Put under the pointer, the cursor is on screen: the view stays.
    ed.followed = Some((ed.row, ed.col, ed.revision));
    true
}

/// The char column nearest `dx` (the studio's units) into `line`, past
/// `skip` chars, measured as the editor draws it: at the code's size times
/// the studio's `scale`.
fn column_at(game: &mut dyn GameApi, line: &str, skip: usize, dx: f32, scale: f32) -> usize {
    let from = line.char_indices().nth(skip).map_or(line.len(), |(i, _)| i);
    let size = super::hstudio_look::code_size() * scale;
    let mut prev = 0.0;
    for (n, (i, c)) in line[from..].char_indices().enumerate() {
        let w = game.dp_measure(&line[from..from + i + c.len_utf8()], size, Font::Mono) / scale;
        if dx < (prev + w) / 2.0 {
            return n;
        }
        prev = w;
    }
    line[from..].chars().count()
}

/// The editor scrolled so its scroll bar's thumb has its top at `y`: at
/// once (no glide), the cursor and the selection where they are.
fn scroll_to_thumb(st: &mut Studio, y: f32) {
    let Some(sb) = st.s.ui.scrollbar else { return };
    let room = (sb.track - sb.bar).max(1.0);
    let k = ((y - sb.top) / room).clamp(0.0, 1.0);
    let ed = &mut st.s.editor;
    ed.scroll = ((k * sb.span).round() as usize).min(ed.lines.len().saturating_sub(1));
    st.s.ui.smooth[1] = ed.scroll as f32;
}

/// The open menu's keys: Up and Down move over the items that can be
/// chosen, Enter (or Space) chooses, Escape closes; others are kept from
/// what is under it.
fn menu_key(st: &mut Studio, game: &mut dyn GameApi, key: Key) {
    let Some(menu) = st.s.ui.menu.as_mut() else { return };
    let choosable: Vec<usize> = (0..menu.items.len()).filter(|&i| menu.items[i].enabled).collect();
    let at = menu.sel.and_then(|s| choosable.iter().position(|&i| i == s));
    match key {
        Key::Escape => st.s.ui.menu = None,
        Key::Down if !choosable.is_empty() => {
            menu.sel = Some(choosable[at.map_or(0, |a| (a + 1) % choosable.len())]);
        }
        Key::Up if !choosable.is_empty() => {
            menu.sel = Some(choosable[at.map_or(choosable.len() - 1, |a| (a + choosable.len() - 1) % choosable.len())]);
        }
        Key::Enter | Key::Char(' ') => {
            if let Some(i) = menu.sel {
                menu_pick(st, game, i);
            }
        }
        _ => {}
    }
}

/// Item `i` of the open menu chosen: the menu closes and its command runs.
fn menu_pick(st: &mut Studio, _game: &mut dyn GameApi, i: usize) {
    let Some(menu) = st.s.ui.menu.take() else { return };
    let Some(item) = menu.items.get(i).filter(|it| it.enabled) else {
        return;
    };
    match item.cmd.clone() {
        Some(MenuCmd::Model(model)) => set_agent_model(st, model),
        Some(MenuCmd::CustomModel) => {
            let mut edit = LineEdit::default();
            edit.set(&agent_model(st).unwrap_or_default());
            st.s.ui.naming = Some(edit);
            st.s.ui.naming_kind = NamingKind::Model;
            st.s.ui.naming_at = st.s.now;
        }
        None => {}
    }
}

/// The models offered for an agent ("claude", "omp" or "custom"): a label
/// and the name passed with --model (None: the agent's own default). Claude
/// Code takes these aliases for its newest models; omp's names depend on
/// its providers, so it offers its default and a name typed in.
pub fn model_choices(agent_id: &str) -> &'static [(&'static str, Option<&'static str>)] {
    match agent_id {
        "claude" => &[
            ("Default", None),
            ("Opus", Some("opus")),
            ("Sonnet", Some("sonnet")),
            ("Haiku", Some("haiku")),
            ("Fable", Some("fable")),
        ],
        "omp" => &[("Default", None)],
        _ => &[],
    }
}

/// How a model shows on its chip: its label among the choices, else its name.
pub fn model_label(agent_id: &str, model: Option<&str>) -> String {
    model_choices(agent_id)
        .iter()
        .find(|(_, m)| *m == model)
        .map_or_else(|| model.unwrap_or("Default").to_owned(), |(l, _)| (*l).to_owned())
}

/// The file the chosen models are kept in: `<agent id>=<model>` lines.
fn models_file(st: &Studio) -> PathBuf {
    st.s.dir.join("agent-models.txt")
}

/// The chosen models by agent id, read the first time they are wanted.
fn models(st: &mut Studio) -> &mut std::collections::HashMap<String, String> {
    if st.s.ui.agent.models.is_none() {
        let text = std::fs::read_to_string(models_file(st)).unwrap_or_default();
        let read = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect();
        st.s.ui.agent.models = Some(read);
    }
    st.s.ui.agent.models.get_or_insert_with(Default::default)
}

/// The id of the agent in use ("claude", "omp", "custom"), if one was found.
pub fn agent_id(st: &mut Studio) -> Option<String> {
    let dir = st.s.dir.clone();
    st.s.ui.agent.spec(&dir).map(|s| s.id)
}

/// The model chosen for the agent in use (None: its default).
pub fn agent_model(st: &mut Studio) -> Option<String> {
    let id = agent_id(st)?;
    models(st).get(&id).cloned()
}

/// Chooses the agent's model (None: its default), kept for next time; the
/// conversation says so.
pub fn set_agent_model(st: &mut Studio, model: Option<String>) {
    let Some(id) = agent_id(st) else { return };
    let map = models(st);
    match &model {
        Some(m) => map.insert(id.clone(), m.clone()),
        None => map.remove(&id),
    };
    let mut keys: Vec<_> = map.iter().map(|(k, v)| format!("{k}={v}")).collect();
    keys.sort();
    let _ = std::fs::write(models_file(st), keys.join("\n") + "\n");
    let now = st.s.now;
    let label = model_label(&id, model.as_deref());
    st.s.ui.agent.say(AgentLineKind::Done, format!("Model: {label}."), now);
}

/// The agent's models in a menu under its chip, the current one marked.
fn open_model_menu(st: &mut Studio) {
    let Some(id) = agent_id(st) else { return };
    let current = agent_model(st);
    let mut items: Vec<MenuItem> = model_choices(&id)
        .iter()
        .map(|(label, m)| MenuItem::new(*label, MenuCmd::Model(m.map(str::to_owned))).checked(current.as_deref() == *m))
        .collect();
    let known = model_choices(&id).iter().any(|(_, m)| *m == current.as_deref());
    if let (Some(m), false) = (&current, known) {
        items.push(MenuItem::new(m.clone(), MenuCmd::Model(Some(m.clone()))).checked(true));
    }
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Other model...", MenuCmd::CustomModel));
    let chip =
        st.s.ui
            .model_chip
            .unwrap_or([st.s.ui.mouse.0, st.s.ui.mouse.1, 0.0, 0.0]);
    let sel = items.iter().position(|i| i.checked);
    st.s.ui.menu = Some(Menu {
        at: [chip[0], chip[1] + chip[3] + 4.0],
        items,
        sel,
        opened_at: st.s.now,
        rect: None,
    });
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
            let main = st.s.mods[i].main.clone();
            st.save_before_switch(game, Some(&main));
            st.edit_mod(i);
            st.s.ui.tab = Tab::Editor;
        }
        Action::EditScratch => {
            let path = st.scratch_path();
            st.save_before_switch(game, Some(&path));
            if st.s.editor.path.as_ref() != Some(&path) {
                st.s.editor.open(&path, "scratch.lua");
                st.s.editor.error_line = None;
            }
            st.s.ui.tab = Tab::Editor;
        }
        Action::SaveFile => {
            let path = st.s.editor.path.clone();
            if path.is_some() && st.s.mods.iter().any(|m| Some(&m.main) == path.as_ref()) {
                st.save_editor(game);
            } else {
                st.s.ui.naming = Some(LineEdit::default());
                st.s.ui.naming_kind = NamingKind::SaveAs;
                st.s.ui.naming_at = st.s.now;
            }
        }
        Action::Reload(i) => {
            st.s.mods[i].enabled = true;
            st.set_enabled(game, i, true);
        }
        Action::NewMod => {
            st.s.ui.naming = Some(LineEdit::default());
            st.s.ui.naming_kind = NamingKind::NewMod;
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
        Action::WinMove | Action::WinResize => {}
        Action::WinMin => st.s.ui.collapsed = !st.s.ui.collapsed,
        Action::WinMax => {
            st.s.ui.look.maximized = !st.s.ui.look.maximized;
            st.s.ui.look.save(&st.s.dir);
        }
        Action::Theme(i) => {
            st.s.ui.look.theme = usize::from(i).min(super::hstudio_theme::THEMES.len() - 1);
            // The executor styles open on their editor.
            if st.s.ui.look.theme().layout != super::hstudio_theme::Layout::Studio && st.s.ui.tab != Tab::Settings {
                st.s.ui.tab = Tab::Editor;
            }
            st.s.ui.collapsed = false;
            st.s.ui.look.rect = None;
            st.s.ui.look.maximized = false;
            st.s.ui.look.save(&st.s.dir);
        }
        Action::NextTheme => {
            let n = super::hstudio_theme::THEMES.len();
            return act(st, game, Action::Theme(((st.s.ui.look.theme + 1) % n) as u8));
        }
        Action::Windowed(on) => {
            st.s.ui.look.window = on;
            st.s.ui.look.save(&st.s.dir);
        }
        Action::Accent(i) => {
            st.s.ui.look.accent = usize::from(i).min(super::hstudio_theme::ACCENTS.len() - 1);
            st.s.ui.look.save(&st.s.dir);
        }
        Action::Step(which, up) => {
            let look = &mut st.s.ui.look;
            let sign = if up { 1.0 } else { -1.0 };
            match which {
                0 => look.scale = ((look.scale + 0.05 * sign) * 20.0).round() / 20.0,
                1 => look.opacity = ((look.opacity + 0.05 * sign) * 20.0).round() / 20.0,
                _ => look.code += sign,
            }
            look.scale = look.scale.clamp(0.75, 1.5);
            look.opacity = look.opacity.clamp(0.3, 1.0);
            look.code = look.code.clamp(11.0, 20.0);
            look.save(&st.s.dir);
        }
        Action::Motion => {
            st.s.ui.look.motion = !st.s.ui.look.motion;
            st.s.ui.look.save(&st.s.dir);
        }
        Action::ResetWindow => {
            st.s.ui.look.rect = None;
            st.s.ui.look.maximized = false;
            st.s.ui.collapsed = false;
            st.s.ui.look.save(&st.s.dir);
        }
        Action::Settings => {
            st.s.ui.tab = if st.s.ui.tab == Tab::Settings {
                Tab::Editor
            } else {
                Tab::Settings
            };
        }
        Action::ClearEditor => {
            st.s.editor.set_text("");
            st.s.editor.dirty = true;
        }
        Action::Suggest(i) => {
            if let Some(sg) = &mut st.s.ui.suggest {
                sg.sel = usize::from(i);
            }
            accept_suggestion(st, usize::from(i));
        }
        Action::PanelTab(p) => {
            let ui = &mut st.s.ui;
            if ui.panel != p {
                ui.panel_tab_at = st.s.now;
            }
            ui.panel = p;
            ui.panel_hidden = false;
            ui.panel_focus = true;
        }
        Action::TogglePanel => {
            let ui = &mut st.s.ui;
            // As VS Code's: shown but not in use, it takes the keys; in use, it hides.
            if ui.tab != Tab::Editor {
                ui.tab = Tab::Editor;
                ui.panel_hidden = false;
            } else if ui.panel_hidden || ui.panel_focus {
                ui.panel_hidden = !ui.panel_hidden;
            }
            ui.panel_focus = !ui.panel_hidden;
            ui.suggest = None;
        }
        Action::PanelResize => {}
        Action::FocusPanel => {
            st.s.ui.panel_focus = true;
            st.s.ui.suggest = None;
        }
        Action::Problem(i) => goto_problem(st, i),
        Action::ReloadFile => {
            let ed = &mut st.s.editor;
            if let Some(path) = ed.path.clone() {
                let (title, row) = (ed.title.clone(), ed.row);
                ed.open(&path, &title);
                ed.click(row.min(ed.lines.len() - 1), 0, false);
                ed.reloaded_at = Some(st.s.now);
            }
        }
        Action::KeepMine => st.s.editor.conflict = false,
        Action::AgentNext => agent_next(st),
        Action::AgentStop => agent_stop(st),
        Action::Block | Action::ScrollBar => {}
        Action::MenuPick(i) => menu_pick(st, game, i as usize),
        Action::ModelMenu => open_model_menu(st),
        Action::ClearLog => {
            st.s.log.clear();
            st.s.ui.log_scroll = 0;
        }
        Action::CopyPrompt => {
            game.set_clipboard(super::hstudio_link::SETUP_PROMPT);
            st.s.ui.copied_at = Some(st.s.now);
            st.s.toast = Some((
                "Copied the AI setup prompt: paste it into Claude Code, omp or another agent".into(),
                st.s.now + 3.5,
            ));
        }
    }
    UiResult::Used
}

fn key_down(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) -> UiResult {
    if let Some(name) = &mut st.s.ui.naming {
        match key {
            Key::Escape => {
                st.s.ui.naming = None;
                st.s.ui.naming_kind = NamingKind::NewMod;
            }
            Key::Enter => {
                let wanted = name.text.trim().to_owned();
                st.s.ui.naming = None;
                let kind = std::mem::take(&mut st.s.ui.naming_kind);
                if kind == NamingKind::Model {
                    set_agent_model(st, Some(wanted).filter(|m| !m.is_empty()));
                } else if !wanted.is_empty() {
                    let code = (kind == NamingKind::SaveAs).then(|| st.s.editor.text());
                    match st.create_mod(game, &wanted, code.as_deref()) {
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
    if st.s.ui.menu.is_some() {
        menu_key(st, game, key);
        return UiResult::Used;
    }
    if mods.ctrl
        && key == Key::Char('c')
        && let Some(text) = selected_text(&st.s.ui)
    {
        game.set_clipboard(&text);
        st.s.toast(format!("Copied {} characters", text.chars().count()));
        return UiResult::Used;
    }
    if key == Key::Escape && st.s.ui.suggest.take().is_some() {
        return UiResult::Used;
    }
    if key == Key::Escape && st.s.ui.tab == Tab::Editor && st.s.ui.panel_focus {
        st.s.ui.panel_focus = false;
        return UiResult::Used;
    }
    if !mods.ctrl && suggest_key(st, key) {
        return UiResult::Used;
    }
    match key {
        Key::Escape | Key::Toggle => return UiResult::Close,
        Key::Char('`' | 'j') if mods.ctrl => {
            return act(st, game, Action::TogglePanel);
        }
        Key::Char(' ') if mods.ctrl => {
            refresh_suggest(st);
            return UiResult::Used;
        }
        Key::Char(c @ '1'..='5') if mods.ctrl => {
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
    let open = st.s.ui.suggest.is_some();
    match target(&st.s.ui) {
        Target::Console => console_key(st, game, key, mods),
        Target::Editor => editor_key(st, game, key, mods),
        Target::Agent => agent_key(st, game, key, mods),
        Target::Elsewhere if st.s.ui.tab == Tab::Editor => problems_key(st, key),
        Target::Elsewhere => {}
    }
    // Deleting keeps the suggestions in step with the word.
    if open && matches!(key, Key::Backspace | Key::Delete) {
        refresh_suggest(st);
    }
    match st.s.ui.tab {
        Tab::Console | Tab::Editor => {}
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
        Tab::Settings => {
            let s = &mut st.s.ui.settings_scroll;
            match key {
                Key::Up => *s -= 40.0,
                Key::Down => *s += 40.0,
                Key::PageUp => *s -= 320.0,
                Key::PageDown => *s += 320.0,
                Key::Home => *s = 0.0,
                _ => {}
            }
            *s = s.max(0.0);
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
            refresh_suggest(st);
            // One way to go: take it at once.
            if st.s.ui.suggest.as_ref().is_some_and(|s| s.items.len() == 1) {
                accept_suggestion(st, 0);
            } else if st.s.ui.suggest.is_none()
                && let Some(rest) = complete(&st.s.ui.input.text, st.s.ui.input.cursor)
            {
                st.s.ui.input.insert(&rest);
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

/// One line of the Agent panel's conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentLine {
    pub kind: AgentLineKind,
    pub text: String,
    /// When it came (its motion).
    pub at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentLineKind {
    /// What the player asked.
    You,
    /// What the agent said.
    Said,
    /// A tool it used, and how that went.
    Tool,
    ToolOk,
    ToolFailed,
    Error,
    /// The end of its answer.
    Done,
}

/// The Agent panel: the conversation, the agent working, the conversation
/// to resume, which agent.
#[derive(Default)]
pub struct AgentPane {
    pub lines: Vec<AgentLine>,
    /// The agent's run, while it works (and until its last events are read).
    pub run: Option<super::hstudio_agent::AgentRun>,
    /// The conversation it resumes, from its last run.
    pub session: Option<String>,
    /// The agents found here (looked for once), and which one is used.
    pub agents: Option<Vec<super::hstudio_agent::AgentSpec>>,
    pub chosen: usize,
    pub(crate) scroll: f32,
    /// Lines said so far (the newest's number), for selecting.
    pub(crate) seq: u64,
    /// The model chosen for each agent, by its id (`agent-models.txt`).
    pub(crate) models: Option<std::collections::HashMap<String, String>>,
}

/// Lines kept of the conversation.
const AGENT_LINES: usize = 400;

impl AgentPane {
    fn say(&mut self, kind: AgentLineKind, text: impl Into<String>, at: f64) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.seq += 1;
        self.lines.push(AgentLine { kind, text, at });
        if self.lines.len() > AGENT_LINES {
            self.lines.drain(..self.lines.len() - AGENT_LINES);
        }
    }
    /// The agents here, looked for the first time they are wanted.
    fn agents(&mut self, studio_dir: &Path) -> &[super::hstudio_agent::AgentSpec] {
        self.agents
            .get_or_insert_with(|| super::hstudio_agent::detect(studio_dir))
            .as_slice()
    }
    fn spec(&mut self, studio_dir: &Path) -> Option<super::hstudio_agent::AgentSpec> {
        let chosen = self.chosen;
        let agents = self.agents(studio_dir);
        agents.get(chosen.min(agents.len().saturating_sub(1))).cloned()
    }
}

/// Asks the agent `text`, with what the editor has open: the file, what is
/// selected and its problems. The agent works in the mods folder, through
/// Horizon Studio's MCP; nothing is asked while it is still working.
pub fn agent_ask(st: &mut Studio, text: &str) {
    if text.is_empty() || agent_busy(st) {
        return;
    }
    let now = st.s.now;
    let dir = st.s.dir.clone();
    let pane = &mut st.s.ui.agent;
    pane.scroll = 0.0;
    pane.say(AgentLineKind::You, text, now);
    let found = pane.spec(&dir);
    // With the model chosen for it, if one was.
    let Some(spec) = found.map(|spec| {
        let model = models(st).get(&spec.id).cloned().unwrap_or_default();
        spec.with_model(&model)
    }) else {
        st.s.ui.agent.say(
            AgentLineKind::Error,
            "No agent found: install Claude Code (claude) or omp and set it up with the AI setup prompt, or name one in agent.txt in the studio's folder.",
            now,
        );
        return;
    };
    // It reads the file from disk: unsaved edits are saved first, so it
    // works on what the player sees (and its writes don't conflict).
    if st.s.editor.dirty && st.s.editor.path.is_some() {
        match st.s.editor.save() {
            Ok(path) => {
                st.s.log(LogKind::Info, format!("Saved {} for the agent", path.display()))
            }
            Err(e) => {
                st.s.ui
                    .agent
                    .say(AgentLineKind::Error, format!("Could not save the file first: {e}"), now)
            }
        }
    }
    let mods_dir = st.s.mods_dir();
    let ed = &st.s.editor;
    let file = ed
        .path
        .as_ref()
        .and_then(|p| p.strip_prefix(&mods_dir).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    let ctx = super::hstudio_agent::AgentContext {
        file,
        mods_dir: mods_dir.clone(),
        selection: ed.selected_text().filter(|t| !t.trim().is_empty()),
        problems: ed
            .problems
            .iter()
            .map(|p| format!("{}:{}: {}", ed.title, p.line, p.message))
            .collect(),
    };
    let prompt = super::hstudio_agent::prompt(text, &ctx);
    let _ = std::fs::create_dir_all(&mods_dir);
    let pane = &mut st.s.ui.agent;
    match super::hstudio_agent::AgentRun::start(&spec, &prompt, &mods_dir, pane.session.as_deref()) {
        Ok(run) => pane.run = Some(run),
        Err(e) => pane.say(AgentLineKind::Error, e, now),
    }
}

/// What the agent did since the last frame, into the conversation. Called
/// every frame, the studio open or not.
pub fn agent_poll(st: &mut Studio) {
    use super::hstudio_agent::AgentEvent;
    let now = st.s.now;
    let pane = &mut st.s.ui.agent;
    let Some(run) = pane.run.as_mut() else { return };
    let events = run.poll();
    let mut over = false;
    for ev in events {
        match ev {
            AgentEvent::Started { session } => {
                if session.is_some() {
                    pane.session = session;
                }
            }
            AgentEvent::Text(t) => pane.say(AgentLineKind::Said, t, now),
            AgentEvent::Tool { name, summary } => {
                let text = if summary.is_empty() { name } else { summary };
                pane.say(AgentLineKind::Tool, text, now);
            }
            AgentEvent::ToolResult { ok, summary } => {
                let kind = if ok {
                    AgentLineKind::ToolOk
                } else {
                    AgentLineKind::ToolFailed
                };
                pane.say(kind, summary, now);
            }
            AgentEvent::Done { ok, session, summary } => {
                if session.is_some() {
                    pane.session = session;
                }
                // Claude Code's result repeats what it last said.
                let said = pane
                    .lines
                    .last()
                    .filter(|l| l.kind == AgentLineKind::Said)
                    .map(|l| l.text.trim().to_owned());
                match summary {
                    Some(s) if ok && said.as_deref() == Some(s.trim()) => pane.say(AgentLineKind::Done, "Done.", now),
                    Some(s) if ok => {
                        pane.say(AgentLineKind::Said, s, now);
                        pane.say(AgentLineKind::Done, "Done.", now);
                    }
                    Some(s) => pane.say(AgentLineKind::Error, s, now),
                    None if ok => pane.say(AgentLineKind::Done, "Done.", now),
                    None => pane.say(AgentLineKind::Error, "It stopped.", now),
                }
                over = true;
            }
            AgentEvent::Error(e) => pane.say(AgentLineKind::Error, e, now),
        }
    }
    if over {
        pane.run = None;
        // Its edits show at once.
        st.s.ui.disk_at = 0.0;
    }
}

/// Another agent found on this computer: looks again, and with several
/// found, takes the next.
pub fn agent_next(st: &mut Studio) {
    if agent_busy(st) {
        return;
    }
    let now = st.s.now;
    let found = super::hstudio_agent::detect(&st.s.dir);
    let pane = &mut st.s.ui.agent;
    let before = pane.agents.replace(found.clone()).unwrap_or_default();
    if found.is_empty() {
        pane.chosen = 0;
        pane.say(
            AgentLineKind::Error,
            "No agent found: install Claude Code (claude) or omp, or name one in agent.txt in the studio's folder.",
            now,
        );
        return;
    }
    pane.chosen = if before == found {
        (pane.chosen + 1) % found.len()
    } else {
        0
    };
    let spec = &found[pane.chosen];
    if pane.chosen != 0 || before != found {
        // Another agent: a new conversation.
        pane.session = None;
    }
    let text = format!("Using {} ({}).", spec.name, spec.program.display());
    pane.say(AgentLineKind::Done, text, now);
}

/// Stops the agent's work (its last events still come in).
pub fn agent_stop(st: &mut Studio) {
    if let Some(run) = st.s.ui.agent.run.as_mut() {
        run.stop();
    }
}

/// The agent's name for the panel ("Claude Code"), or None when none is found.
pub fn agent_name(st: &mut Studio) -> Option<String> {
    let dir = st.s.dir.clone();
    st.s.ui.agent.spec(&dir).map(|s| s.name)
}

/// Whether the agent is working.
pub fn agent_busy(st: &Studio) -> bool {
    st.s.ui.agent.run.is_some()
}

/// The editor's cursor to problem `i`, and the keyboard back to the editor.
fn goto_problem(st: &mut Studio, i: usize) {
    let ed = &mut st.s.editor;
    if let Some(p) = ed.problems.get(i).cloned() {
        let row = (p.line.max(1) - 1).min(ed.lines.len() - 1);
        let col = p.span.map_or(0, |s| s.0).min(ed.lines[row].len());
        let chars = ed.lines[row][..col].chars().count();
        ed.click(row, chars, false);
        st.s.ui.problem_sel = i;
        st.s.ui.panel_focus = false;
    }
}

/// The Problems panel: Up and Down choose, Enter goes to it.
fn problems_key(st: &mut Studio, key: Key) {
    let n = st.s.editor.problems.len();
    if n == 0 {
        return;
    }
    let ui = &mut st.s.ui;
    match key {
        Key::Up => ui.problem_sel = ui.problem_sel.saturating_sub(1),
        Key::Down => ui.problem_sel = (ui.problem_sel + 1).min(n - 1),
        Key::Enter => {
            let i = ui.problem_sel.min(n - 1);
            goto_problem(st, i);
        }
        _ => {}
    }
}

/// The Agent panel's input: Enter asks, the rest edits the line.
fn agent_key(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) {
    let _ = game;
    match key {
        // While it works, what is typed waits in the line.
        Key::Enter if !agent_busy(st) => {
            let ask = std::mem::take(&mut st.s.ui.agent_input).text;
            agent_ask(st, ask.trim());
        }
        Key::Enter => {}
        Key::Char('v') if mods.ctrl => {
            let clip = game.clipboard();
            st.s.ui.agent_input.insert(&clip);
        }
        k => {
            st.s.ui.agent_input.key(k, mods);
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
