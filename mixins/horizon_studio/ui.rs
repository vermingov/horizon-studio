// Horizon Studio's screen: Console (a Lua executor), Editor (a code editor
// with highlighting, undo and the clipboard), Library (the script mods, their
// state and errors, and the bundled examples) and Docs (the API). It draws
// with the game's own GUI font and rectangles through GameApi, in Horizon's
// dark design tokens (docs/design-system.md): a veil over the world, plates
// for the console and code, 1 px rules, one accent for the main action.
// The Editor tab is panes the player sizes and moves, which the dock
// (hstudio_dock) places: the script list, the code, the console, the
// problems, the agent, and the Library and the Docs once docked. Here are
// which of them has the keys (only ever one that is drawn), the drags of
// their bars and tabs (the Console's, Library's and Docs' tabs at the top
// too), sizing from the keys, and what their menus' items do.

use super::hstudio::{API, CONSOLE_API, Font, GameApi, LogKind, Owner, Studio};
pub use super::hstudio_dock::{Bar, Dock, DrawnLeaf, DropAt, DropSpot, Pane, Side};
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
    /// The Menu key (or Shift+F10): the menu of what has the keys.
    Menu,
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
    /// The left button, or the right one.
    Button { left: bool, down: bool },
    /// The middle button (the wheel pressed): a script's screen takes it,
    /// the studio leaves it be.
    Middle { down: bool },
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
pub enum Action {
    Tab(Tab),
    Select(usize),
    Toggle(usize),
    Edit(usize),
    Reload(usize),
    /// The New prompt, on a script.
    NewMod,
    /// The script list's +: a menu of a new script, a new module, and a new
    /// file of the module open.
    NewMenu,
    /// The New prompt's switch: a module (true) or a script.
    NewKind(bool),
    /// A row of the script list (by its place in `Ui::script_rows`): a
    /// click opens its file or folds its module; and its "..." menu.
    ScriptRow(u16),
    ScriptMenu(u16),
    /// The New file prompt for module `i`; script `i` made a module; the
    /// Library's menu of mod `i` (its "...").
    NewFileIn(usize),
    MakeModule(usize),
    LibraryMenu(usize),
    /// A file of a mod opened (by its place in `Ui::drawn_files`), the
    /// cursor at the line its mod failed at.
    EditFile(u16),
    /// The executor styles' Execute File: the open file of a mod saved and
    /// the mod reloaded, else the code run.
    ExecuteFile,
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
    /// The Editor tab's panes: a pane's tab pressed (it comes to the front
    /// of its group), a press in a group where nothing else takes it, a
    /// pane shown with the keys (the problems count, the Panes list), the
    /// group holding a pane hidden (its ×) or shown and hidden from the keys
    /// (Ctrl+`, Ctrl+J, Ctrl+B), a bar between groups (by its split's path),
    /// the layout reset and the reset undone, the Library or the Docs
    /// closed, a pane moved, and a pane's bars sized from the keys.
    PaneTab(Pane),
    Focus(Pane),
    ShowPane(Pane),
    HideGroup(Pane),
    ToggleGroup(Pane),
    DockBar(u16),
    ResetLayout,
    UndoReset,
    ClosePane(Pane),
    MovePane(Pane, DropAt),
    SizePane(Pane),
    /// A problem's line.
    Problem(usize),
    /// The editor's file changed on disk while edited: take that, or keep
    /// these; removed from disk while edited, let it go.
    ReloadFile,
    KeepMine,
    CloseRemoved,
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
    /// Where a right click opens a menu; left clicks go to what is under it.
    Spot(Spot),
    /// The code an Agent pane line is about (by the line's number), shown:
    /// what an agent did in the editor.
    AgentJump(u64),
    /// An agent's mark (by its number): its note opened, or its × pressed;
    /// the open note shown whole (its More).
    MarkOpen(u32),
    MarkClose(u32),
    MarkMore,
    /// The editor header's notes chip: the next mark, or its × (every mark cleared).
    NextMark,
    ClearMarks,
    /// An agent's lesson: its step shown again (the header's chip), a step
    /// back or on, its step checked, the lesson ended (a ×).
    LessonShow,
    LessonBack,
    LessonNext,
    LessonCheck,
    LessonEnd,
}

/// What a right click is on: the menu it opens is this one's. look.rs and
/// skins.rs mark them where they draw, over what they cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    /// The editor's code, and the name of its file (its title, Synapse's tab).
    Code,
    File,
    /// Selectable text: the console's lines or the agent's (`AREA_LOG`, `AREA_AGENT`).
    Text(u8),
    /// A line typed in: the console's input or the agent's (`FIELD_CONSOLE`, `FIELD_AGENT`).
    Field(u8),
    /// The script list (and the Library's), and a row of the script list
    /// by its place in `Ui::script_rows`.
    Scripts,
    Row(u16),
    /// A mod's row in the Library (by its place in `Ui::drawn_mods`), and an
    /// example's.
    Mod(usize),
    Example(usize),
    /// A pane of the Editor tab (its tab, its group's strip, and its body
    /// where nothing else has a menu), and a problem's row.
    Pane(Pane),
    Problem(usize),
    /// The window's title bar, or the studio's header.
    Window,
    /// ProtoSmasher's theme dropdown.
    Themes,
    Docs,
}

/// What a menu item does when chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuCmd {
    /// The agent's model (None: the agent's own default).
    Model(Option<String>),
    /// A model typed by name.
    CustomModel,
    /// The New prompt, on a script or on a module.
    NewScript,
    NewModule,
    /// The New file prompt, for the module in this folder.
    NewFile(PathBuf),
    /// A file opened in the editor; the Rename and Delete prompts for it
    /// (Delete takes a module's folder or a script's file for the whole mod).
    Open(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
    /// The script with this id made a module.
    MakeModule(String),
    /// A folder shown in the system's file manager.
    Reveal(PathBuf),
    /// What a click does.
    Act(Action),
    /// The editor's commands, on its selection or its cursor's line.
    Edit(EditCmd),
    /// The console's input line or the agent's (`FIELD_CONSOLE`, `FIELD_AGENT`).
    Field(u8, FieldCmd),
    /// To the clipboard: the text selected in the console or the agent's
    /// conversation, all of one of them (`AREA_LOG`, `AREA_AGENT`), or this.
    CopySelection,
    CopyAll(u8),
    Copy(String),
    /// The console back at its newest line.
    LogEnd,
    /// The agent's conversation started afresh.
    NewConversation,
    /// A mod's commands, by its id.
    Mod(String, ModCmd),
    /// The editor's file read again from disk, its unsaved edits dropped as
    /// one edit (Ctrl+Z brings them back).
    Revert,
    /// The agent: the one found with this id used, or agents looked for again.
    Agent(String),
    FindAgents,
    /// Another list in the menu's place: the themes, the agent's models, the
    /// Editor tab's panes, the places a pane moves to, and a pane's layout
    /// items (the Menu key's Layout).
    Themes,
    Models,
    Panes,
    Moves(Pane),
    Layout(Pane),
    /// The Docs tab as Markdown, to the clipboard.
    CopyDocs,
    /// A place in the file open selected (a problem's, a name's
    /// definition): its row and the bytes from and to.
    GoTo(usize, usize, usize),
    /// A name's definition in another file of the module: that file opened,
    /// and the definition selected where its text has it then, which an
    /// agent's edit on disk may have moved since the menu opened.
    Define(PathBuf, String),
    /// The Docs tab at this function's entry.
    Docs(String),
    /// The agent asked this (None: its input takes the keys, for a
    /// question typed there), with the selection; the code explained.
    Ask(Option<String>),
    Explain,
}

/// What a mod's menu does to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModCmd {
    /// Its main file opened.
    Edit,
    Reload,
    /// Turned on or off.
    Toggle,
    /// The Rename and Duplicate prompts for it.
    Rename,
    Duplicate,
    /// Its error, and the traceback, to the clipboard.
    CopyError,
    /// The agent asked to fix it, its file open where it failed.
    Fix,
}

/// The editor's commands in its menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCmd {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Comment,
    Indent,
    Outdent,
    DuplicateLine,
    /// The selection run in the console.
    RunSelection,
}

/// An input line's commands in its menu: Submit runs the console's line or
/// sends the agent its question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCmd {
    Paste,
    Copy,
    Cut,
    Clear,
    Submit,
}

/// A row of a menu: its label, a hint at the right (a shortcut, or "›" for
/// an item that opens another list in its place), whether it can be chosen,
/// is the current choice or deletes something (it is in the danger colour),
/// or a separator line.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub hint: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
    pub danger: bool,
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
            danger: false,
            cmd: Some(cmd),
        }
    }
    pub fn separator() -> MenuItem {
        MenuItem {
            separator: true,
            ..MenuItem::note("")
        }
    }
    pub fn checked(mut self, on: bool) -> MenuItem {
        self.checked = on;
        self
    }
    /// Greyed out, when `on` is false: shown, but it can't be chosen.
    pub fn enabled(mut self, on: bool) -> MenuItem {
        self.enabled = on;
        self
    }
    /// Its shortcut, shown at the right ("Ctrl C").
    pub fn keys(mut self, keys: &str) -> MenuItem {
        self.hint = keys.to_owned();
        self
    }
    /// In the danger colour: it deletes something.
    pub fn danger(mut self) -> MenuItem {
        self.danger = true;
        self
    }
    /// A line that says something and can't be chosen.
    pub fn note(label: impl Into<String>) -> MenuItem {
        MenuItem {
            label: label.into(),
            hint: String::new(),
            enabled: false,
            checked: false,
            separator: false,
            danger: false,
            cmd: None,
        }
    }
}

/// A row of the script list: the scratch file, a script, a module (folded
/// or open), or a file of an open module. Mods are by their index when the
/// rows were made; what a click does goes by the path or id.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptRow {
    Scratch,
    Script { i: usize, path: PathBuf },
    Module { i: usize, id: String, open: bool },
    File { i: usize, path: PathBuf, rel: String },
}

/// The script list's rows: the scratch file, then each mod, a script as one
/// row and a module as a folder followed, while it is open, by its files
/// (main.lua first).
pub fn script_rows(st: &Studio) -> Vec<ScriptRow> {
    let mut rows = vec![ScriptRow::Scratch];
    for (i, m) in st.s.mods.iter().enumerate() {
        let Some(folder) = &m.folder else {
            rows.push(ScriptRow::Script {
                i,
                path: m.main.clone(),
            });
            continue;
        };
        let open = st.s.ui.expanded.contains(&m.id);
        rows.push(ScriptRow::Module {
            i,
            id: m.id.clone(),
            open,
        });
        if open {
            for rel in m.files() {
                let path = super::hstudio_modfiles::path_in(folder, &rel);
                rows.push(ScriptRow::File { i, path, rel });
            }
        }
    }
    rows
}

/// The row that shows the file the editor has open: its own, or its
/// module's while the module is folded.
pub(crate) fn open_row(st: &Studio, rows: &[ScriptRow]) -> Option<usize> {
    let path = st.s.editor.path.as_deref()?;
    if path == st.scratch_path() {
        return Some(0);
    }
    let owner = st.mod_of_path(path);
    rows.iter().position(|r| match r {
        ScriptRow::Script { path: p, .. } | ScriptRow::File { path: p, .. } => p == path,
        ScriptRow::Module { i, open: false, .. } => owner == Some(*i),
        _ => false,
    })
}

/// The first row the script list shows, `rows` at a time out of `total`:
/// when the editor's file changes it brings that file's row (`open`) into
/// view, and otherwise stays where the wheel put it.
pub(crate) fn scripts_first(st: &mut Studio, total: usize, rows: usize, open: Option<usize>) -> usize {
    let path = st.s.editor.path.clone();
    let ui = &mut st.s.ui;
    if ui.scripts_followed != path {
        ui.scripts_followed = path;
        if let Some(k) = open {
            if k < ui.scripts_scroll {
                ui.scripts_scroll = k;
            }
            if k >= ui.scripts_scroll + rows {
                ui.scripts_scroll = k + 1 - rows;
            }
        }
    }
    ui.scripts_scroll = ui.scripts_scroll.min(total.saturating_sub(rows));
    ui.scripts_scroll
}

/// What a row of the script list says: the scratch file's name, a script's
/// file, a module's folder, a module's file by its path in it.
pub(crate) fn row_label(st: &Studio, row: &ScriptRow) -> String {
    match row {
        ScriptRow::Scratch => "scratch.lua".into(),
        ScriptRow::Script { i, .. } => st.s.mods.get(*i).map_or_else(String::new, |m| m.chunk()),
        ScriptRow::Module { id, .. } => format!("{id}/"),
        ScriptRow::File { rel, .. } => rel.clone(),
    }
}

/// A menu open over everything: where it opens (its top left, kept on the
/// screen when drawn), its items, the one lit (the keys' or the pointer's),
/// when it opened, and where it was drawn (clicks outside it close it).
/// `anchor_h` is the height of what opened it (a chip, a button; 0 at the
/// pointer), which a menu with no room below opens above. `target` is the
/// row it is for, outlined while it is open. A menu taller than the screen
/// shows its items from `scroll` on (`shown`: the first and the end, as
/// drawn), and the wheel and the keys scroll it. A menu about the code
/// (the code's own, a problem's) has the editor's revision it was made
/// for in `code`: its items hold places in that text, so it goes when the
/// text changes under it (an agent's edit, the file read again).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub at: [f32; 2],
    pub items: Vec<MenuItem>,
    pub sel: Option<usize>,
    pub opened_at: f64,
    pub rect: Option<[f32; 4]>,
    pub anchor_h: f32,
    pub target: Option<[f32; 4]>,
    pub scroll: usize,
    pub shown: (usize, usize),
    pub code: Option<u64>,
}

impl Menu {
    /// A menu of `items` with its top left at `at` (above it, by `anchor_h`
    /// more, when there is no room below), nothing lit.
    pub fn new(at: [f32; 2], anchor_h: f32, items: Vec<MenuItem>, now: f64) -> Menu {
        Menu {
            at,
            items,
            sel: None,
            opened_at: now,
            rect: None,
            anchor_h,
            target: None,
            scroll: 0,
            shown: (0, 0),
            code: None,
        }
    }
    /// Whether it was drawn with only some of its items (it scrolls).
    pub fn scrolls(&self) -> bool {
        self.rect.is_some() && self.shown != (0, self.items.len())
    }
    /// Whether it is about the code, which changed since it was made.
    pub fn stale(&self, ed: &Editor) -> bool {
        self.code.is_some_and(|r| r != ed.revision)
    }
}

/// What the naming prompt names: a new script or module (Tab or its switch
/// goes from one to the other), the editor's code saved as a new script
/// (Save File), the agent's model, a new file of the module in that
/// folder, a module's file renamed, (asking, nothing typed) a mod's file or
/// the whole mod going to the trash, or the mod with this id renamed or
/// copied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NamingKind {
    #[default]
    NewScript,
    NewModule,
    SaveAs,
    Model,
    NewFile(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
    RenameMod(String),
    Duplicate(String),
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

/// A window being moved or sized: how, from where the pointer was, and the
/// window then.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WinDrag {
    resize: bool,
    from: (f32, f32),
    rect: [f32; 4],
}

/// What the left button holds on the Editor tab's panes: a bar between two
/// groups, as the last frame drew it, and where the pointer is on it along
/// its way (from its left or top edge); or a pane's tab, pressed where the
/// pointer was, and lifted (`moving`) once dragged `TAB_LIFT` from there.
/// A tab is a group's, or the Console, Library or Docs tab at the top
/// (`page`), whose pane is dragged into the Editor tab.
#[derive(Clone, Copy, Debug)]
pub(crate) enum DockDrag {
    Bar {
        bar: Bar,
        grab: f32,
    },
    Tab {
        pane: Pane,
        from: (f32, f32),
        moving: bool,
        page: Option<Tab>,
    },
}

/// How far a pressed tab is dragged before it lifts, so a click that moves
/// a little only brings its pane to the front.
const TAB_LIFT: f32 = 6.0;

/// Where the editor's text was drawn, for the pointer.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EditorView {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) rows: usize,
}

/// The console's lines and the agent's: the areas text is selected in.
pub const AREA_LOG: u8 = 0;
pub const AREA_AGENT: u8 = 1;

/// The console's input line and the agent's (`Spot::Field`).
pub const FIELD_CONSOLE: u8 = 0;
pub const FIELD_AGENT: u8 = 1;

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

/// Whether `at` is in the text selected in `area`.
pub(crate) fn in_text_selection(ui: &Ui, area: u8, at: (f32, f32)) -> bool {
    let Some(sel) = ui.text_sel.filter(|s| s.area == area && s.anchor != s.head) else {
        return false;
    };
    let Some((_, pos)) = text_pos_at(ui, at, Some(area)) else {
        return false;
    };
    let (a, b) = sel.range();
    a <= pos && pos <= b
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
    /// The function the Docs tab goes to when it is drawn next (Show in
    /// Docs), and the entry it went to with when (a tint a moment).
    pub(crate) docs_find: Option<String>,
    pub(crate) docs_lit: Option<(String, f64)>,
    /// The naming prompt (New, Save as, New file, Rename, Delete), while it is open.
    pub naming: Option<LineEdit>,
    /// What the prompt names.
    pub naming_kind: NamingKind,
    /// The menu open over everything, if one is.
    pub menu: Option<Menu>,
    /// A line about what a menu's item did ("Copied 42 characters", why a
    /// rename failed) and when it was said, shown in the studio a moment.
    pub notice: Option<(String, f64)>,
    /// Where the agent's model chip was drawn (its menu opens under it).
    pub(crate) model_chip: Option<[f32; 4]>,
    /// Where the script list's + was drawn (its menu opens under it).
    pub(crate) new_button: Option<[f32; 4]>,
    /// Where the Library's "..." for the mod chosen there was drawn.
    pub(crate) library_dots: Option<[f32; 4]>,
    /// The mods' ids as the last frame listed them: a click on a mod, by its
    /// place in that list, goes to the same mod wherever a scan has put it
    /// since. And the files the frame drew a click for (`Action::EditFile`
    /// is a place in them).
    pub(crate) drawn_mods: Vec<String>,
    pub(crate) drawn_files: Vec<PathBuf>,
    /// The modules unfolded in the script list, by id (for the session).
    pub expanded: std::collections::HashSet<String>,
    /// The script list's rows as drawn last (`Action::ScriptRow` is a place
    /// in them), the first row shown, where the list was drawn (None: it
    /// wasn't), and the editor's file it last scrolled to.
    pub(crate) script_rows: Vec<ScriptRow>,
    pub scripts_scroll: usize,
    pub(crate) scripts_at: Option<[f32; 4]>,
    pub(crate) scripts_followed: Option<PathBuf>,
    /// The other files of the module the editor's file is in, for its
    /// checks and suggestions, read again when the module's files change.
    pub(crate) siblings: Option<SiblingCache>,
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
    /// The Editor tab's panes (`layout.txt`), and the pane the keys go to
    /// there (they reach it only while it is at the front, `key_pane`).
    pub dock: Dock,
    pub focus: Pane,
    /// What the left button holds on the panes, and where a dragged tab
    /// would land.
    pub(crate) dock_drag: Option<DockDrag>,
    pub dock_spot: Option<DropSpot>,
    /// From the last frame (cleared with the hits): the bars between the
    /// groups, the groups as drawn, and the panes whose body was drawn at the
    /// front of their group (by `Pane::index`).
    pub dock_bars: Vec<Bar>,
    pub dock_drawn: Vec<DrawnLeaf>,
    pub dock_front: [bool; 7],
    /// The Editor tab's area as the last frame of the studio's own layout
    /// had it, whichever tab showed (kept: what a change would do is laid
    /// out there from any tab).
    pub dock_ws: Option<[f32; 4]>,
    /// When each pane came to the front (its content's arrival).
    pub(crate) shown_at: [f64; 7],
    /// What `layout.txt` holds (as read, or last written): only a change is
    /// written.
    pub(crate) dock_text: String,
    /// The strip's tip that tabs can be dragged shows, until a tab or a bar
    /// is first moved.
    pub dock_tips: bool,
    /// The layout a Reset layout replaced, until the next change.
    pub(crate) dock_undo: Option<Dock>,
    /// Sizing from the keys: the pane, and the path of its bar that is lit.
    pub sizing: Option<(Pane, u16)>,
    /// What the agent is asked, typed in its pane.
    pub agent_input: LineEdit,
    pub problem_sel: usize,
    pub agent: AgentPane,
    /// What AI agents do in the editor, live (`hstudio_coach`).
    pub coach: super::hstudio_coach::Coach,
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
    /// Lines each pane could show when last drawn (by `Pane::index`), for
    /// Page Up/Down: the code's, the console's and the docs' are their own.
    pub pages: [usize; 7],
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
    /// The bottom of the caret as last drawn (None: it was off the code's
    /// view), where the Menu key opens the code's menu.
    pub caret_at: Option<[f32; 2]>,
    /// When the editor's file was last looked at on disk.
    pub(crate) disk_at: f64,
    /// When each of the editor's problems first showed (its fade in).
    pub(crate) problem_at: std::collections::HashMap<u64, f64>,
    /// The window as drawn: it glides to where it is going (maximised,
    /// restored, the screen resized).
    pub(crate) win_shown: Option<[f32; 4]>,
    pub(crate) naming_at: f64,
}

impl Ui {
    /// dp per GUI pixel (1 before anything was drawn).
    pub fn dp_factor(&self) -> f32 {
        if self.dp > 0.0 { self.dp } else { 1.0 }
    }
    /// Whether the left button is held for something: selecting code or
    /// text, the scroll bar's thumb, a bar between panes or the window.
    pub fn dragging_any(&self) -> bool {
        self.dragging || self.text_drag || self.scroll_drag.is_some() || self.dock_drag.is_some() || self.drag.is_some()
    }
    /// Whether the left button is held selecting the code's text.
    pub fn selecting_code(&self) -> bool {
        self.dragging
    }
    /// The pane whose tab is lifted, dragged to another place.
    pub fn lifted(&self) -> Option<Pane> {
        match self.dock_drag {
            Some(DockDrag::Tab { pane, moving: true, .. }) => Some(pane),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------- a line of text being typed

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineEdit {
    pub text: String,
    /// A byte index on a char boundary.
    pub cursor: usize,
    /// The other end of the selected text (a byte index on a char boundary),
    /// which runs to the cursor; None when nothing is selected.
    pub anchor: Option<usize>,
    /// A field of several lines: newlines stay newlines.
    pub multiline: bool,
}

impl LineEdit {
    /// New text, the cursor at its end and nothing selected.
    pub fn set(&mut self, text: &str) {
        self.text = text.to_owned();
        self.cursor = self.text.len();
        self.anchor = None;
    }
    /// Types `s` at the cursor, over the selection if there is one.
    pub fn insert(&mut self, s: &str) {
        let multiline = self.multiline;
        let clean: String = s
            .chars()
            .map(|c| if (c == '\n' && !multiline) || c == '\t' { ' ' } else { c })
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        self.delete_selection();
        self.anchor = None;
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    /// The selection's start and end, when something is selected.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor
            .filter(|&a| a != self.cursor)
            .map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }
    /// The selected text ("" without a selection).
    pub fn selected(&self) -> &str {
        self.selection().map_or("", |(a, b)| &self.text[a..b])
    }
    /// Removes the selected text; false when nothing was selected.
    pub fn delete_selection(&mut self) -> bool {
        let Some((a, b)) = self.selection() else {
            return false;
        };
        self.text.replace_range(a..b, "");
        self.cursor = a;
        self.anchor = None;
        true
    }
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.text.len();
    }
    /// Moves the cursor to `to`; `extend` keeps the selection's other end
    /// where it was (or where the cursor was), else the selection goes.
    pub fn move_cursor(&mut self, to: usize, extend: bool) {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = to;
    }
    /// Where the line the cursor is on starts and ends.
    fn line_bounds(&self) -> (usize, usize) {
        let start = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
        let end = self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i);
        (start, end)
    }
    /// The cursor a line up or down, as many characters in as it is now:
    /// the start of the text above the first line, its end below the last.
    pub fn line_move(&self, down: bool) -> usize {
        let (start, end) = self.line_bounds();
        let column = self.text[start..self.cursor].chars().count();
        let (from, to) = if down {
            if end >= self.text.len() {
                return self.text.len();
            }
            let from = end + 1;
            (from, self.text[from..].find('\n').map_or(self.text.len(), |i| from + i))
        } else {
            if start == 0 {
                return 0;
            }
            let to = start - 1;
            (self.text[..to].rfind('\n').map_or(0, |i| i + 1), to)
        };
        self.text[from..to]
            .char_indices()
            .nth(column)
            .map_or(to, |(i, _)| from + i)
    }
    /// Handles an editing key as a text field with a selection does: an
    /// arrow, Home or End with Shift extends the selection, and without it
    /// drops the selection (Left at its start, Right at its end); Backspace
    /// and Delete remove the selection; Control+A selects everything; in a
    /// field of several lines Home and End keep to the line (with Control,
    /// the text), and Up and Down move a line. False when it isn't one.
    pub fn edit_key(&mut self, key: Key, mods: Mods) -> bool {
        let extend = mods.shift;
        match key {
            Key::Left => match self.selection() {
                Some((a, _)) if !extend => self.move_cursor(a, false),
                _ => {
                    let to = if mods.ctrl {
                        word_left(&self.text, self.cursor)
                    } else {
                        prev_char(&self.text, self.cursor)
                    };
                    self.move_cursor(to, extend);
                }
            },
            Key::Right => match self.selection() {
                Some((_, b)) if !extend => self.move_cursor(b, false),
                _ => {
                    let to = if mods.ctrl {
                        word_right(&self.text, self.cursor)
                    } else {
                        next_char(&self.text, self.cursor)
                    };
                    self.move_cursor(to, extend);
                }
            },
            Key::Home => {
                let to = if self.multiline && !mods.ctrl {
                    self.line_bounds().0
                } else {
                    0
                };
                self.move_cursor(to, extend);
            }
            Key::End => {
                let to = if self.multiline && !mods.ctrl {
                    self.line_bounds().1
                } else {
                    self.text.len()
                };
                self.move_cursor(to, extend);
            }
            Key::Up | Key::Down if self.multiline => {
                let to = self.line_move(key == Key::Down);
                self.move_cursor(to, extend);
            }
            Key::Backspace => {
                if !self.delete_selection() {
                    self.anchor = None;
                    self.backspace(mods.ctrl);
                }
            }
            Key::Delete => {
                if !self.delete_selection() {
                    self.anchor = None;
                    self.delete(mods.ctrl);
                }
            }
            Key::Char('a') if mods.ctrl => self.select_all(),
            _ => return false,
        }
        true
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
    /// A place (row, byte) that keeps its place in the text through a
    /// splice as the cursor does: a press's, while an agent's edit is put
    /// in whole under it.
    pub held: Option<(usize, usize)>,
    /// The rows as last drawn while an agent's edit typed in, and the
    /// splices since (see `Drawn`).
    pub(crate) drawn: Option<Drawn>,
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
    /// The file was removed from disk while this buffer had unsaved edits:
    /// they stay, and a save writes the file back.
    pub removed: bool,
    /// When the file was read again after a change elsewhere (an agent's).
    pub reloaded_at: Option<f64>,
}

/// The rows the editor last drew while an agent's edit typed in, and how
/// the text changed since. A press is for the text drawn under the pointer,
/// and in the game each frame types more of the edit in (the studio's
/// frame) before the studio gets that frame's input: the place drawn under
/// the pointer is found in these rows, then carried through the splices.
pub(crate) struct Drawn {
    /// The revision the splices below bring the rows to: the editor's own
    /// while nothing else changed its text since (which leaves it behind).
    rev: u64,
    /// The first row drawn, the rows drawn from it, the chars of each
    /// scrolled off to the left, and how many lines the text had.
    first: usize,
    lines: Vec<String>,
    hscroll: usize,
    count: usize,
    /// Each splice since.
    splices: Vec<Splice>,
    /// How far the view as drawn (`Ui::smooth`) was moved since, with the
    /// text it shows: lines an agent's change put in at once above it.
    moved: f32,
}

/// A splice: where it began and ended, and where the text it put in ends
/// (rows, and bytes of them).
type Splice = ((usize, usize), (usize, usize), (usize, usize));

/// The splices a press can be carried through, at most: more, and it is
/// for the text as it is.
const DRAWN_SPLICES: usize = 64;

impl Drawn {
    /// Rows `first` to `last` of `ed` as it is drawn now.
    pub(crate) fn of(ed: &Editor, first: usize, last: usize) -> Drawn {
        let end = (last + 1).min(ed.lines.len());
        Drawn {
            rev: ed.revision,
            first,
            lines: ed.lines.get(first..end).map(<[String]>::to_vec).unwrap_or_default(),
            hscroll: ed.hscroll,
            count: ed.lines.len(),
            splices: Vec::new(),
            moved: 0.0,
        }
    }
}

/// Where place `p` (row, byte) is once `text` ending at `end` replaced
/// `from..to`: before `from` it stays, inside the range it goes to its
/// start, and after it it moves with the text after it.
fn after_splice(p: (usize, usize), from: (usize, usize), to: (usize, usize), end: (usize, usize)) -> (usize, usize) {
    if p < from {
        p
    } else if p < to {
        from
    } else if p.0 == to.0 {
        (end.0, end.1 + p.1 - to.1)
    } else {
        (p.0 - to.0 + end.0, p.1)
    }
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
            held: None,
            drawn: None,
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
            removed: false,
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
        // The revision goes on rising from file to file, so whatever follows
        // the buffer (an agent's edit being typed in) sees that it changed.
        let revision = self.revision + 1;
        *self = Editor {
            path: Some(path.to_path_buf()),
            title: title.to_owned(),
            problem,
            disk: disk_stamp(path),
            revision,
            ..Editor::default()
        };
        self.set_text(&text);
        // It shows its top, where the cursor is: the view has followed the
        // cursor, drawn yet or not (a change on disk before the first draw
        // types in where the view goes).
        self.followed = Some((self.row, self.col, self.revision));
    }
    /// The file it shows moved (renamed, or a script made a module's
    /// main.lua): the same text, cursor, undo and unsaved edits, at its new
    /// path, so a save writes there and not where it was. A conflict with a
    /// change made elsewhere moved with the file, and stays.
    pub fn moved(&mut self, to: &Path, title: &str) {
        self.path = Some(to.to_path_buf());
        self.title = title.to_owned();
        self.disk = disk_stamp(to);
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
        // Removed, it is back.
        self.removed = false;
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
        // It reads now: what it couldn't read before is past.
        self.problem = None;
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
        self.removed = false;
        self.disk = disk_stamp(&path);
        Ok(path)
    }
    /// Whether the file on disk is no longer the one this buffer last read
    /// or wrote (changed, or removed, elsewhere), whether or not that was
    /// noticed yet.
    pub fn disk_changed(&self) -> bool {
        self.removed || self.conflict || self.path.as_deref().is_some_and(|p| disk_stamp(p) != self.disk)
    }
    /// Text as the buffer holds it: Windows line ends as `\n`, tabs as the
    /// indent (what `insert` and `set_text` do to what they are given).
    pub fn normal(s: &str) -> String {
        s.replace("\r\n", "\n").replace('\t', INDENT)
    }
    /// A place (row, byte of the row) kept inside the buffer, on a char boundary.
    fn clamp_pos(&self, (row, col): (usize, usize)) -> (usize, usize) {
        let row = row.min(self.lines.len() - 1);
        let line = &self.lines[row];
        let mut col = col.min(line.len());
        while !line.is_char_boundary(col) {
            col -= 1;
        }
        (row, col)
    }
    /// A place as a byte of `text()`.
    pub fn offset_of(&self, pos: (usize, usize)) -> usize {
        let (row, col) = self.clamp_pos(pos);
        self.lines[..row].iter().map(|l| l.len() + 1).sum::<usize>() + col
    }
    /// A byte of `text()` as a place (past the end: the end).
    pub fn pos_at(&self, offset: usize) -> (usize, usize) {
        let mut left = offset;
        for (row, line) in self.lines.iter().enumerate() {
            if left <= line.len() {
                return self.clamp_pos((row, left));
            }
            left -= line.len() + 1;
        }
        let row = self.lines.len() - 1;
        (row, self.lines[row].len())
    }
    /// An agent's change begins: one undo step for all of it, however long
    /// it takes to type in (its splices take none of their own). It changes
    /// neither the text nor the cursor, so a view that had followed the
    /// cursor still has (it stays where the agent's change puts it), and
    /// the rows as drawn are still the text's.
    pub fn agent_begin(&mut self) {
        let rev = self.revision;
        self.begin(EditKind::Other);
        if self.followed == Some((self.row, self.col, rev)) {
            self.followed = Some((self.row, self.col, self.revision));
        }
        if let Some(d) = self.drawn.as_mut().filter(|d| d.rev == rev) {
            d.rev = self.revision;
        }
    }
    /// The view as drawn moved `by` rows since the draw, with the text it
    /// shows (lines an agent's change put in at once above it): the rows as
    /// drawn are that much further on under the pointer.
    pub(crate) fn view_moved(&mut self, by: f32) {
        if let Some(d) = self.drawn.as_mut() {
            d.moved += by;
        }
    }
    /// Puts `text` (already `normal`) in place of `from..to` and returns
    /// where it ends, taking no undo step of its own. The cursor and the
    /// selection's other end keep their place in the text: before `from`
    /// they stay, inside the range they go to its start, after it they move
    /// with the text after it; and the view doesn't jump to them. Places
    /// outside the buffer count as its nearest.
    pub fn splice(&mut self, from: (usize, usize), to: (usize, usize), text: &str) -> (usize, usize) {
        let (from, to) = {
            let (a, b) = (self.clamp_pos(from), self.clamp_pos(to));
            if b < a { (b, a) } else { (a, b) }
        };
        let joined = format!("{}{text}{}", &self.lines[from.0][..from.1], &self.lines[to.0][to.1..]);
        let end = match text.rfind('\n') {
            Some(i) => (from.0 + text.matches('\n').count(), text.len() - i - 1),
            None => (from.0, from.1 + text.len()),
        };
        self.lines.splice(from.0..=to.0, joined.split('\n').map(str::to_owned));
        let moved = |p: (usize, usize)| after_splice(p, from, to, end);
        // The rows as drawn follow the text through it, as far as they can.
        let revision = self.revision;
        if let Some(d) = self.drawn.as_mut().filter(|d| d.rev == revision) {
            d.splices.push((from, to, end));
            d.rev = revision + 1;
            if d.splices.len() > DRAWN_SPLICES {
                self.drawn = None;
            }
        }
        (self.row, self.col) = self.clamp_pos(moved((self.row, self.col)));
        self.anchor = self.anchor.map(|a| self.clamp_pos(moved(a)));
        self.held = self.held.map(|h| self.clamp_pos(moved(h)));
        self.dirty = true;
        self.revision += 1;
        self.followed = Some((self.row, self.col, self.revision));
        self.scroll = self.scroll.min(self.lines.len() - 1);
        end
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
    /// Tab and Shift+Tab: the lines selected in or out a level; Tab with
    /// nothing selected types an indent at the cursor.
    pub fn indent(&mut self, out: bool) {
        if !out && self.selection().is_none() {
            self.insert(INDENT);
            return;
        }
        self.indent_lines(out);
    }
    /// The lines selected, or the cursor's line, in or out a level.
    pub fn indent_lines(&mut self, out: bool) {
        let (r1, r2) = self.rows();
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
    /// Whether a place (a row and a byte of it) is in the selection, its
    /// ends included.
    pub fn in_selection(&self, row: usize, col: usize) -> bool {
        self.selection()
            .is_some_and(|(a, b)| a <= (row, col) && (row, col) <= b)
    }
    /// Selects the cursor's line, from its first non-space to its end.
    pub fn select_line(&mut self) {
        let line = &self.lines[self.row];
        self.anchor = Some((self.row, line.len() - line.trim_start().len()));
        self.col = line.len();
        self.goal = None;
        self.last_kind = None;
    }
    /// Selects bytes `start..end` of `row` (a name gone to), the cursor at its end.
    pub fn select_span(&mut self, row: usize, start: usize, end: usize) {
        let row = row.min(self.lines.len() - 1);
        let line = &self.lines[row];
        let fit = |i: usize| {
            let mut i = i.min(line.len());
            while !line.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        let (start, end) = (fit(start), fit(end));
        self.row = row;
        self.anchor = (start < end).then_some((row, start));
        self.col = end;
        self.goal = None;
        self.last_kind = None;
    }
    /// The whole text replaced as one edit, which Ctrl+Z takes back.
    pub fn replace_text(&mut self, text: &str) {
        self.begin(EditKind::Other);
        self.set_text(text);
        self.anchor = None;
    }
    /// Reads the file again from disk, dropping the unsaved edits as one
    /// edit (Ctrl+Z brings them back). False when it can't be read.
    pub fn revert(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        let scroll = self.scroll;
        self.replace_text(&text);
        self.dirty = false;
        self.conflict = false;
        self.removed = false;
        self.disk = disk_stamp(&path);
        self.scroll = scroll.min(self.lines.len().saturating_sub(1));
        true
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

pub(crate) fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// What a left click at `at` reaches: the topmost of the last frame's hits
/// there, the right click's spots left out (a click goes to what is under them).
pub(crate) fn hit_at(ui: &Ui, at: (f32, f32)) -> Option<Action> {
    ui.hits
        .iter()
        .rev()
        .find(|(r, a)| !matches!(a, Action::Spot(_)) && inside(*r, at))
        .map(|(_, a)| *a)
}

/// What a right click at `at` is on: the topmost spot there and where it
/// was drawn. Buttons and rows over a spot are its own (a list's rows are
/// the list's); what takes no click (a dialog, the menu, a banner over the
/// code), the scroll bar and the suggestions have no menu, nor what is under
/// them.
pub(crate) fn spot_at(ui: &Ui, at: (f32, f32)) -> Option<(Spot, [f32; 4])> {
    for (r, a) in ui.hits.iter().rev().filter(|(r, _)| inside(*r, at)) {
        match a {
            Action::Spot(s) => return Some((*s, *r)),
            Action::Block | Action::ScrollBar | Action::Suggest(_) | Action::MenuPick(_) | Action::NewKind(_) => {
                return None;
            }
            _ => {}
        }
    }
    None
}

pub use super::hstudio_look::{code_line, draw, draw_screen};

// ----------------------------------------------------------------- input

pub fn handle(st: &mut Studio, game: &mut dyn GameApi, input: Input) -> UiResult {
    let before = super::hstudio_coach::before_input(st, game, &input);
    let revision = st.s.editor.revision;
    let out = handle_input(st, game, input);
    if st.s.editor.revision != revision {
        st.s.editor.edited_at = st.s.now;
    }
    super::hstudio_coach::after_input(st, before);
    // A menu goes with the studio, and so does what the left button holds
    // (its release won't reach a studio closing) and sizing from the keys.
    if out == UiResult::Close {
        st.s.ui.menu = None;
        let_go(st);
        end_sizing(st);
    }
    out
}

/// The left button let go, or its release went elsewhere (the studio closed,
/// a script's screen took the input): what it dragged ends, a window moved
/// or a bar between panes stays where it is, a pane's tab moves nothing (a
/// drop is the release's own, before this), and a selection of no text goes.
pub(crate) fn let_go(st: &mut Studio) {
    let ui = &mut st.s.ui;
    ui.scroll_drag = None;
    ui.text_drag = false;
    if ui.text_sel.is_some_and(|s| s.anchor == s.head) {
        ui.text_sel = None;
    }
    ui.dragging = false;
    if ui.drag.take().is_some() {
        ui.look.save(&st.s.dir);
    }
    end_dock_drag(st);
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
    match key_pane(ui) {
        Some(Pane::Console) => Target::Console,
        Some(Pane::Code) => Target::Editor,
        Some(Pane::Agent) => Target::Agent,
        _ => Target::Elsewhere,
    }
}

// ----------------------------------------------------------------- the Editor tab's panes

/// The pane the keys go to: the Console, Library and Docs tabs' own, the
/// code on an executor style's Editor tab, and on the studio's own Editor
/// tab the pane in use (`Ui::focus`) while it is at the front of a shown
/// group and takes keys (the script list doesn't). None: they go nowhere
/// but the studio's own shortcuts (Settings, or a pane covered or hidden).
pub fn key_pane(ui: &Ui) -> Option<Pane> {
    match ui.tab {
        Tab::Console => Some(Pane::Console),
        Tab::Editor if ui.look.theme().layout != super::hstudio_theme::Layout::Studio => Some(Pane::Code),
        Tab::Editor => (ui.focus.takes_keys() && ui.dock.is_front(ui.focus)).then_some(ui.focus),
        Tab::Library => Some(Pane::Library),
        Tab::Docs => Some(Pane::Docs),
        Tab::Settings => None,
    }
}

/// The pane in use is `p` now; the suggestions, which belong to the pane
/// they opened in, close when it changes.
fn set_focus(ui: &mut Ui, p: Pane) {
    if ui.focus != p {
        ui.focus = p;
        ui.suggest = None;
    }
}

/// The pane in use where the dock has it now: there while it is at the
/// front of a shown group, else what covers it, else (its group hidden)
/// the front of the code's group. Each edit of the dock ends with it, and
/// each frame of the Editor tab starts with it.
pub fn settle_focus(ui: &mut Ui) {
    let settled = ui.dock.settled(ui.focus);
    set_focus(ui, settled);
}

/// Whether the dock isn't what `layout.txt` holds (the tip's line aside).
fn dock_moved(ui: &Ui) -> bool {
    !ui.dock_text.contains(&format!("dock={}\n", ui.dock.to_text()))
}

/// What every edit of the dock outside dock.rs ends with: the pane in use
/// settles, `layout.txt` is written when it would change, and when the
/// panes changed, a reset can no longer be undone and what the button or
/// the keys held on them ends where it may be on others now (`end_held`).
pub(crate) fn dock_changed(st: &mut Studio) {
    let ui = &mut st.s.ui;
    settle_focus(ui);
    let text = super::hstudio_dock::file_text(&ui.dock, ui.dock_tips);
    if text == ui.dock_text {
        return;
    }
    let moved = dock_moved(ui);
    let placed = written_dock(ui).is_some_and(|d| d.same_places(&ui.dock));
    super::hstudio_dock::save(&st.s.dir, &ui.dock, ui.dock_tips);
    ui.dock_text = text;
    if moved {
        ui.dock_undo = None;
        end_held(ui, placed);
    }
}

/// The dock as `layout.txt` holds it (as it was loaded, or last written).
fn written_dock(ui: &Ui) -> Option<Dock> {
    ui.dock_text
        .lines()
        .find_map(|l| l.strip_prefix("dock="))
        .and_then(Dock::parse)
}

/// What the button or the keys held on the panes, once the dock changed
/// from what `layout.txt` held; `placed` when its groups are where they
/// were and only the tabs at their front or their sizes changed (all that
/// an agent or a lesson changes). Where groups moved, came, went or hid,
/// it all ends: its bars, places and panes may be others now. Else a bar
/// or a tab held and sizing from the keys go on (a bar's path names the
/// same split, a tab the same pane, and a lifted tab's place is found
/// again), and only what was held in a pane now behind another ends: the
/// code's scroll bar and selection, a selection of the console's or the
/// agent's lines.
fn end_held(ui: &mut Ui, placed: bool) {
    if !placed {
        ui.dock_drag = None;
        ui.dock_spot = None;
        ui.sizing = None;
        ui.scroll_drag = None;
        ui.dragging = false;
        ui.text_drag = false;
        return;
    }
    // The executor styles draw their own panes, which the dock doesn't place.
    if ui.look.theme().layout != super::hstudio_theme::Layout::Studio {
        return;
    }
    if !ui.dock.is_front(Pane::Code) {
        ui.scroll_drag = None;
        ui.dragging = false;
    }
    let lines = match ui.text_sel {
        Some(s) if s.area == AREA_AGENT => Pane::Agent,
        _ => Pane::Console,
    };
    if ui.tab == Tab::Editor && !ui.dock.is_front(lines) {
        ui.text_drag = false;
    }
    if let Some(pane) = ui.lifted() {
        ui.dock_spot = drop_spot(ui, pane, ui.mouse);
    }
}

/// `p` to the front of its group (its content arriving), and the keys to
/// it. The script list takes no keys: they stay where they are, unless it
/// has no room beside the pane in use, when it becomes the pane in use
/// (filling the room, the keys going nowhere) until a script opens or Esc.
pub(crate) fn focus_pane(st: &mut Studio, p: Pane) {
    let now = st.s.now;
    let ui = &mut st.s.ui;
    if !ui.dock.is_front(p) {
        ui.shown_at[p.index()] = now;
    }
    ui.dock.activate(p);
    if p.takes_keys() || ui.dock_ws.is_some_and(|ws| !ui.dock.bodies(ws, ui.focus)[p.index()]) {
        set_focus(ui, p);
    }
    dock_changed(st);
}

/// `p` shown and given the keys: a closed Library or Docs comes back at its
/// home, or among the code's tabs where that has no room. Where neither has
/// room without taking a pane's place, its own page shows instead.
pub(crate) fn show_pane(st: &mut Studio, p: Pane) {
    let now = st.s.now;
    let ui = &mut st.s.ui;
    if !ui.dock.is_front(p) {
        ui.shown_at[p.index()] = now;
    }
    let ws = ui.dock_ws;
    let keep = ui.focus;
    ui.dock.show_fitting(p, ws, keep);
    if !ui.dock.has(p) {
        no_room_for(st, p);
        return;
    }
    focus_pane(st, p);
}

/// The Library's or the Docs' page, where the Editor tab has no room to
/// dock it, and a notice saying so.
fn no_room_for(st: &mut Studio, p: Pane) {
    st.s.ui.tab = if p == Pane::Docs { Tab::Docs } else { Tab::Library };
    notice(st, format!("No room for the {} in the Editor tab", p.title()));
}

/// The code at the front with the keys, for what the player does: a script
/// opened, a problem gone to, Esc. (An executor style's Editor tab is its
/// code: the panes stay as they are for the Horizon themes.)
pub fn show_code(st: &mut Studio) {
    st.s.ui.tab = Tab::Editor;
    if st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio {
        focus_pane(st, Pane::Code);
    } else {
        set_focus(&mut st.s.ui, Pane::Code);
    }
}

/// The code brought forward for what agents, scripts and the console do: the
/// Editor tab, and the code at the front of its group unless that would
/// cover the pane the player has the keys in. The keys stay where they are.
pub fn reveal_code(st: &mut Studio) {
    let now = st.s.now;
    let ui = &mut st.s.ui;
    ui.tab = Tab::Editor;
    if ui.look.theme().layout != super::hstudio_theme::Layout::Studio {
        return;
    }
    let covers_keys = ui.dock.covers(Pane::Code) == Some(ui.focus) && ui.focus.takes_keys();
    if !covers_keys && !ui.dock.is_front(Pane::Code) {
        ui.shown_at[Pane::Code.index()] = now;
        ui.dock.activate(Pane::Code);
    }
    dock_changed(st);
}

/// The group holding `p` hidden, in place, and a notice of how it comes
/// back. Never the code's group.
fn hide_group(st: &mut Studio, p: Pane) {
    let tabs = st.s.ui.dock.leaf_tabs(p).to_vec();
    if !st.s.ui.dock.set_hidden(p, true) {
        return;
    }
    dock_changed(st);
    let say = if tabs.contains(&Pane::Console) {
        "Panel hidden: Ctrl+` brings it back".to_owned()
    } else if tabs.contains(&Pane::Scripts) {
        "Script list hidden: Ctrl+B brings it back".to_owned()
    } else {
        format!("{} hidden: Panes › in a tab's menu brings it back", group_name(&tabs))
    };
    notice(st, say);
}

/// A group by its tabs' names: "Agent", "Library and Docs", "Console,
/// Problems and Agent".
pub(crate) fn group_name(tabs: &[Pane]) -> String {
    let names: Vec<&str> = tabs.iter().map(|p| p.title()).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Ctrl+`, Ctrl+J (the panel: the group holding the console) and Ctrl+B
/// (the script list's group), as VS Code's: hidden, it shows and its front
/// pane takes the keys; shown but not in use, it takes them; in use, it
/// hides. Sharing the code's group, it goes between the code and that pane
/// instead. From another tab it shows, never hides; the executor styles'
/// layouts are their own, so there it only goes to the Editor tab.
fn toggle_group(st: &mut Studio, p: Pane) {
    let ui = &mut st.s.ui;
    let away = ui.tab != Tab::Editor;
    ui.tab = Tab::Editor;
    ui.suggest = None;
    if ui.look.theme().layout != super::hstudio_theme::Layout::Studio {
        return;
    }
    let with_code = ui.dock.leaf_tabs(p).contains(&Pane::Code);
    if p == Pane::Scripts {
        // The script list: drawn, it hides; not drawn (hidden, behind a
        // tab, or with no room), it comes to the front, as the pane in use
        // where it has no room beside it.
        if !away && with_code && ui.dock.is_front(Pane::Scripts) {
            show_code(st);
        } else if !away && !with_code && ui.dock_front[Pane::Scripts.index()] {
            hide_group(st, p);
        } else {
            ui.dock.set_hidden(p, false);
            focus_pane(st, Pane::Scripts);
        }
        return;
    }
    // The panel: hidden, it shows with its front pane in use, whichever
    // that is (the agent's, during a lesson), or the console where that
    // pane takes no keys (the script list dropped there).
    if !ui.dock.shown(p) {
        ui.dock.set_hidden(p, false);
        let front = panel_front(ui, p);
        focus_pane(st, front);
    } else if with_code {
        if away || ui.dock.is_front(Pane::Code) {
            focus_pane(st, p);
        } else {
            show_code(st);
        }
    } else if !away && ui.focus == panel_front(ui, p) {
        hide_group(st, p);
    } else {
        let front = panel_front(ui, p);
        focus_pane(st, front);
    }
}

/// The pane of `p`'s group that Ctrl+` gives the keys: its front pane, or
/// `p` itself where the front pane takes none.
fn panel_front(ui: &Ui, p: Pane) -> Pane {
    Some(ui.dock.settled(p)).filter(|f| f.takes_keys()).unwrap_or(p)
}

/// Settings' Reset layout and the menus': the default layout with the code
/// in use, the one it replaced kept for Undo until the next change. (Not
/// while the groups are the default's, whatever tabs are in front.)
fn reset_layout(st: &mut Studio) {
    if st.s.ui.dock.is_default() {
        return;
    }
    let old = std::mem::take(&mut st.s.ui.dock);
    set_focus(&mut st.s.ui, Pane::Code);
    dock_changed(st);
    st.s.ui.dock_undo = Some(old);
    let say = if st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio {
        "Back to the default layout"
    } else {
        "The Horizon themes' panes are back to the default"
    };
    notice(st, say);
}

/// The layout a reset replaced, back.
fn undo_reset(st: &mut Studio) {
    let Some(old) = st.s.ui.dock_undo.take() else {
        return;
    };
    st.s.ui.dock = old;
    dock_changed(st);
    notice(st, "Your layout is back");
}

/// The bars at the edges of `p`'s group as the last frame drew them, its own
/// split's first, then the ones further out: what Size... lights in turn.
/// A bar with no room to move is a plain gap, drawn with no grip, and isn't
/// one of them.
pub(crate) fn pane_bars(ui: &Ui, p: Pane) -> Vec<Bar> {
    let Some(l) = ui.dock_drawn.iter().find(|l| l.tabs.iter().any(|(t, _)| *t == p)) else {
        return Vec::new();
    };
    let [x, y, w, h] = l.rect;
    let near = |a: f32, b: f32| (a - b).abs() < 1.0;
    let overlaps = |a0: f32, a1: f32, b0: f32, b1: f32| a0.max(b0) < a1.min(b1);
    let mut bars: Vec<Bar> = ui
        .dock_bars
        .iter()
        .filter(|b| b.movable())
        .filter(|b| {
            let [bx, by, bw, bh] = b.rect;
            if b.row {
                (near(bx, x + w) || near(bx + bw, x)) && overlaps(by, by + bh, y, y + h)
            } else {
                (near(by, y + h) || near(by + bh, y)) && overlaps(bx, bx + bw, x, x + w)
            }
        })
        .copied()
        .collect();
    // The deeper a split, the longer its path.
    bars.sort_by_key(|b| std::cmp::Reverse(b.path.ilog2()));
    bars
}

/// Size... for `p`: its first bar lit, which the arrows move.
fn size_pane(st: &mut Studio, p: Pane) {
    let Some(bar) = pane_bars(&st.s.ui, p).first().copied() else {
        return;
    };
    st.s.ui.sizing = Some((p, bar.path));
    st.s.ui.dock_tips = false;
    dock_changed(st);
}

/// Sizing from the keys ends; the bar stays where it is, written.
pub(crate) fn end_sizing(st: &mut Studio) {
    if st.s.ui.sizing.take().is_some() {
        dock_changed(st);
    }
}

/// A key while a bar is lit for sizing: the arrows along its way move it
/// (16 units, Shift 64), Tab lights the next of its pane's bars, Enter and
/// Esc end it. True when the key was its; any other ends it first and goes
/// on as usual (the keys that only change what others mean keep it).
fn sizing_key(st: &mut Studio, key: Key, mods: Mods) -> bool {
    let Some((pane, path)) = st.s.ui.sizing else {
        return false;
    };
    let Some(bar) = st.s.ui.dock_bars.iter().find(|b| b.path == path).copied() else {
        end_sizing(st);
        return false;
    };
    let step = if mods.shift { 64.0 } else { 16.0 };
    let by = match key {
        Key::Left if bar.row => -step,
        Key::Right if bar.row => step,
        Key::Up if !bar.row => -step,
        Key::Down if !bar.row => step,
        Key::Tab => {
            let bars = pane_bars(&st.s.ui, pane);
            if let Some(i) = bars.iter().position(|b| b.path == path) {
                let n = bars.len();
                let next = if mods.shift { i + n - 1 } else { i + 1 };
                st.s.ui.sizing = Some((pane, bars[next % n].path));
            }
            return true;
        }
        Key::Enter | Key::Escape => {
            end_sizing(st);
            return true;
        }
        Key::Other => return true,
        _ => {
            end_sizing(st);
            return false;
        }
    };
    let ui = &mut st.s.ui;
    let ratio = ui.dock.ratio(path).unwrap_or(0.5);
    let len = bar.len.max(1.0);
    // From where the bar is drawn, as the dock lays it out: in an area
    // smaller than when its ratio was set, the groups' least sizes can hold
    // it from where the ratio would put it. A key that can't move it
    // leaves the ratio as it is.
    let now = (bar.start + (ratio * len).round()).clamp(bar.lo, bar.hi);
    let at = (now + by).clamp(bar.lo, bar.hi);
    if at == now {
        return true;
    }
    // Written at each step, as each key's change is: the game can close
    // the studio, and quit, while the bar is lit.
    if ui.dock.set_ratio(path, (at - bar.start) / len) {
        dock_changed(st);
    }
    true
}

/// What the left button held on the panes ends, nothing dropped: a bar stays
/// where it is, written (the tip that tabs move goes once one did), and a
/// tab moves nothing and writes nothing (a tab from the top lifted shows
/// its page again).
fn end_dock_drag(st: &mut Studio) {
    let ui = &mut st.s.ui;
    ui.dock_spot = None;
    match ui.dock_drag.take() {
        Some(DockDrag::Bar { .. }) => {
            if dock_moved(ui) {
                ui.dock_tips = false;
            }
            dock_changed(st);
        }
        Some(DockDrag::Tab {
            page: Some(t),
            moving: true,
            ..
        }) => ui.tab = t,
        _ => {}
    }
}

/// The page a tab at the top is, as a pane that can be dragged into the
/// Editor tab: the Console's, the Library's and the Docs'. (The Editor is
/// where they go, and Settings, which restyles everything, is never a pane.)
fn page_pane(t: Tab) -> Option<Pane> {
    match t {
        Tab::Console => Some(Pane::Console),
        Tab::Library => Some(Pane::Library),
        Tab::Docs => Some(Pane::Docs),
        Tab::Editor | Tab::Settings => None,
    }
}

/// A pane's tab held with the pointer at `at`: it lifts once `TAB_LIFT`
/// from where it was pressed (a tab at the top then goes to the Editor tab,
/// its pane with it), and from then on `dock_spot` is where its pane would
/// land among the groups as the last frame drew them on the Editor tab
/// (nowhere while another tab shows, or before the Editor tab is drawn),
/// so the preview and the release follow the pointer.
fn drag_tab(st: &mut Studio, at: (f32, f32)) {
    let ui = &mut st.s.ui;
    let Some(DockDrag::Tab {
        pane,
        from,
        moving,
        page,
    }) = ui.dock_drag
    else {
        return;
    };
    if !moving {
        if (at.0 - from.0).hypot(at.1 - from.1) < TAB_LIFT {
            return;
        }
        ui.dock_drag = Some(DockDrag::Tab {
            pane,
            from,
            moving: true,
            page,
        });
        if page.is_some() {
            ui.tab = Tab::Editor;
        }
    }
    ui.dock_spot = drop_spot(ui, pane, at);
}

/// Where `pane`, its tab lifted to `at`, would land among the groups as the
/// last frame drew them on the Editor tab: nowhere while another tab shows,
/// or before the Editor tab is drawn.
fn drop_spot(ui: &Ui, pane: Pane, at: (f32, f32)) -> Option<DropSpot> {
    let editor = ui.tab == Tab::Editor && ui.look.theme().layout == super::hstudio_theme::Layout::Studio;
    match ui.dock_ws {
        Some(ws) if editor && !ui.dock_drawn.is_empty() => ui.dock.drop_at(ws, &ui.dock_drawn, pane, ui.focus, at),
        _ => None,
    }
}

/// The front pane of the group under `at` as the last frame drew it, and
/// whether `at` is on its strip; groups that are only their strip don't
/// count.
fn pane_under(ui: &Ui, at: (f32, f32)) -> Option<(Pane, bool)> {
    let l = ui.dock_drawn.iter().find(|l| !l.collapsed && inside(l.rect, at))?;
    Some((l.active, inside(l.strip, at)))
}

/// The wheel over a pane: `n` lines (or rows, or mods) up or down.
fn pane_wheel(st: &mut Studio, game: &mut dyn GameApi, pane: Pane, n: usize, up: bool) {
    let ui = &mut st.s.ui;
    match pane {
        Pane::Console if up => ui.log_scroll += n,
        Pane::Console => ui.log_scroll = ui.log_scroll.saturating_sub(n),
        Pane::Agent if up => ui.agent.scroll += n as f32,
        Pane::Agent => ui.agent.scroll = (ui.agent.scroll - n as f32).max(0.0),
        Pane::Problems => {
            let last = st.s.editor.problems.len().saturating_sub(1);
            ui.problem_sel = if up {
                ui.problem_sel.saturating_sub(n / 3)
            } else {
                (ui.problem_sel + n / 3).min(last)
            };
        }
        Pane::Scripts => {
            ui.scripts_scroll = if up {
                ui.scripts_scroll.saturating_sub(n / 3)
            } else {
                ui.scripts_scroll + n / 3
            };
        }
        Pane::Library => {
            let n = st.s.mods.len();
            if up {
                ui.library_sel = ui.library_sel.saturating_sub(1);
            } else if n > 0 {
                ui.library_sel = (ui.library_sel + 1).min(n - 1);
            }
        }
        Pane::Docs => {
            ui.docs_scroll = if up {
                ui.docs_scroll.saturating_sub(n)
            } else {
                ui.docs_scroll + n
            };
        }
        Pane::Code => {
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
    }
}

/// The siblings of the editor's file as last read: for which file, the
/// module's files' sizes and times then (its `stamp`), and what was read.
pub(crate) struct SiblingCache {
    path: PathBuf,
    stamp: Vec<(PathBuf, Option<std::time::SystemTime>, u64)>,
    data: std::rc::Rc<super::hstudio_assist::Siblings>,
}

/// How much of a module's other files the live check reads, on the frame
/// after any of them changes: a file up to 256 KiB, up to 256 files and
/// 4 MiB in all (as an install takes 256 files). The others are listed,
/// unread: the check is a help, not a build.
const SIBLING_FILE_BYTES: u64 = 256 << 10;
const SIBLING_FILES: usize = 256;
const SIBLING_BYTES: u64 = 4 << 20;

/// A module's files but `rel`, as the live check reads them: their lines as
/// the editor shows a file (its indent for a tab), so a place found in one
/// is that place once it opens; within the limits above, main.lua and the
/// first files by name first (the others as empty).
pub(crate) fn read_siblings(folder: &std::path::Path, files: Vec<String>, rel: &str) -> Vec<(String, Vec<String>)> {
    let (mut read, mut bytes) = (0, 0);
    files
        .into_iter()
        .filter(|f| f != rel)
        .map(|f| {
            let p = super::hstudio_modfiles::path_in(folder, &f);
            let len = std::fs::metadata(&p).map_or(u64::MAX, |meta| meta.len());
            let text = if read < SIBLING_FILES && len <= SIBLING_FILE_BYTES && bytes + len <= SIBLING_BYTES {
                read += 1;
                bytes += len;
                std::fs::read_to_string(&p).unwrap_or_default()
            } else {
                String::new()
            };
            (f, text.replace('\t', INDENT).lines().map(str::to_owned).collect())
        })
        .collect()
}

/// The other files of the module the editor's file is in, for its checks
/// and suggestions (None outside a module), their lines as the editor shows
/// a file (its indent for a tab), so a place found in one is that place
/// once it opens; within the limits above, main.lua and the first files
/// by name first. They are read again only when the scan saw the module's
/// files change (its stamp), and then the live check looks again, though
/// the buffer didn't change.
pub(crate) fn editor_siblings(st: &mut Studio) -> Option<std::rc::Rc<super::hstudio_assist::Siblings>> {
    let path = st.s.editor.path.clone()?;
    let Some((i, folder)) = st
        .mod_of_path(&path)
        .and_then(|i| Some((i, st.s.mods[i].folder.clone()?)))
    else {
        st.s.ui.siblings = None;
        return None;
    };
    let m = &st.s.mods[i];
    let rel = super::hstudio_modfiles::rel_in(&folder, &path)?;
    if let Some(c) = &st.s.ui.siblings
        && c.path == path
        && c.stamp == m.stamp
    {
        return Some(c.data.clone());
    }
    let others = read_siblings(&folder, m.files(), &rel);
    let data = std::rc::Rc::new(super::hstudio_assist::Siblings::new(rel, others));
    st.s.ui.siblings = Some(SiblingCache {
        path,
        stamp: m.stamp.clone(),
        data: data.clone(),
    });
    st.s.editor.checked = None;
    Some(data)
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
        super::hstudio_assist::suggest(&i.text[..i.cursor], &[], true, None)
    } else {
        let siblings = editor_siblings(st);
        let ed = &st.s.editor;
        if ed.selection().is_some() {
            None
        } else {
            super::hstudio_assist::suggest(&ed.lines[ed.row][..ed.col], &ed.lines, false, siblings.as_deref())
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

/// Whether the suggestions open are for where the keys go now (the code's
/// or the console's); a list left from another pane is dropped.
fn suggest_here(ui: &mut Ui) -> bool {
    let here = match target(ui) {
        Target::Console => Some(true),
        Target::Editor => Some(false),
        _ => None,
    };
    let ok = ui.suggest.as_ref().is_some_and(|sg| Some(sg.console) == here);
    if !ok {
        ui.suggest = None;
    }
    ok
}

/// Takes suggestion `i`: it replaces the word typed; a function gets its
/// brackets (the cursor in them), a library its dot (and its members).
pub fn accept_suggestion(st: &mut Studio, i: usize) {
    use super::hstudio_assist::Kind;
    if !suggest_here(&mut st.s.ui) {
        return;
    }
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
        // Its word isn't where it was (the text changed under the list):
        // the list just closes.
        if sg.start > ed.col || !ed.lines[ed.row].is_char_boundary(sg.start) {
            return;
        }
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
    if !suggest_here(&mut st.s.ui) {
        return false;
    }
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
            // Over an open menu, the item under the pointer is lit (once the
            // menu is drawn: until then the hits are another's).
            if st.s.ui.menu.as_ref().is_some_and(|m| m.rect.is_some())
                && let Some(Action::MenuPick(i)) = hit_at(&st.s.ui, (x, y))
                && let Some(menu) = st.s.ui.menu.as_mut()
                && menu.items.get(usize::from(i)).is_some_and(|it| it.enabled)
            {
                menu.sel = Some(usize::from(i));
            }
            // A bar between panes follows the pointer, within its range; it
            // is written when it is let go.
            if let Some(DockDrag::Bar { bar, grab }) = st.s.ui.dock_drag {
                let at = if bar.row { x } else { y };
                let pos = (at - grab).clamp(bar.lo, bar.hi);
                st.s.ui.dock.set_ratio(bar.path, (pos - bar.start) / bar.len.max(1.0));
                return UiResult::Used;
            }
            // A pane's tab held: it lifts once dragged a little, and then
            // where its pane would land follows the pointer.
            if matches!(st.s.ui.dock_drag, Some(DockDrag::Tab { .. })) {
                drag_tab(st, (x, y));
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
            // A press ends sizing from the keys.
            if down {
                end_sizing(st);
            }
            if !left {
                // A right press opens the menu of what is under the pointer; a
                // right release does nothing (a drag of the left button goes on).
                return if down {
                    super::hstudio_menus::right_press(st, game)
                } else {
                    UiResult::Used
                };
            }
            if !down {
                // A lifted tab let go where its pane would land moves it
                // there (the move ends the drag, so a tab from the top
                // leaves its page behind); anything else the button held
                // just ends.
                if let (Some(pane), Some(spot)) = (st.s.ui.lifted(), st.s.ui.dock_spot) {
                    act(st, game, Action::MovePane(pane, spot.at));
                }
                let_go(st);
                return UiResult::Used;
            }
            if st.s.ui.naming.is_some() {
                // While the prompt is up only its own switch takes clicks:
                // what is under it, as drawn last frame, takes none.
                let hit = hit_at(&st.s.ui, st.s.ui.mouse);
                if let Some(action @ Action::NewKind(_)) = hit {
                    return act(st, game, action);
                }
                return UiResult::Used;
            }
            let at = st.s.ui.mouse;
            if let Some(menu) = &st.s.ui.menu {
                // A press outside the menu closes it and goes no further; one
                // on it chooses the item there, and leaves what is under it be.
                if !menu.rect.is_some_and(|r| inside(r, at)) {
                    st.s.ui.menu = None;
                    return UiResult::Used;
                }
                return match hit_at(&st.s.ui, at) {
                    Some(Action::MenuPick(i)) => menu_pick(st, game, usize::from(i)),
                    _ => UiResult::Used,
                };
            }
            let hit = hit_at(&st.s.ui, at);
            // A press on the console's or the agent's text starts selecting it;
            // anywhere else, what was selected there is let go.
            st.s.ui.text_sel = None;
            if matches!(hit, None | Some(Action::Focus(_)))
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
            // A bar between panes: held where it was pressed.
            if let Some(Action::DockBar(path)) = hit {
                if let Some(bar) = st.s.ui.dock_bars.iter().find(|b| b.path == path).copied() {
                    let grab = if bar.row {
                        at.0 - bar.rect[0]
                    } else {
                        at.1 - bar.rect[1]
                    };
                    st.s.ui.dock_drag = Some(DockDrag::Bar { bar, grab });
                }
                return UiResult::Used;
            }
            // A press in a pane's body gives it the keys, before what is
            // there acts (a row, a button, a line); the code's suggestions
            // are over other panes, and a group's strip acts by what is on
            // it alone (its buttons leave the keys where they are).
            if st.s.ui.tab == Tab::Editor
                && st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio
                && !matches!(hit, Some(Action::Suggest(_)))
                && let Some((front, false)) = pane_under(&st.s.ui, at)
                && front.takes_keys()
            {
                focus_pane(st, front);
            }
            if !matches!(hit, Some(Action::Suggest(_))) {
                st.s.ui.suggest = None;
            }
            // A pane's tab: its pane to the front with the keys, and the tab
            // held, to be dragged to another place.
            if let Some(Action::PaneTab(pane)) = hit {
                act(st, game, Action::PaneTab(pane));
                st.s.ui.dock_drag = Some(DockDrag::Tab {
                    pane,
                    from: at,
                    moving: false,
                    page: None,
                });
                return UiResult::Used;
            }
            // The Console, Library and Docs tabs at the top: their page, as
            // ever, and the tab held, to drag its pane into the Editor tab.
            if let Some(Action::Tab(t)) = hit
                && st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio
                && let Some(pane) = page_pane(t)
            {
                act(st, game, Action::Tab(t));
                st.s.ui.dock_drag = Some(DockDrag::Tab {
                    pane,
                    from: at,
                    moving: false,
                    page: Some(t),
                });
                return UiResult::Used;
            }
            let drawn = st.s.ui.smooth[1];
            if let Some(Action::Focus(p)) = hit {
                act(st, game, Action::Focus(p));
                if p == Pane::Code && editor_point(st, game, at.0, at.1, false, drawn) {
                    st.s.ui.dragging = true;
                }
                return UiResult::Used;
            }
            if let Some(action) = hit {
                return match as_drawn(st, action) {
                    Some(action) => act(st, game, action),
                    None => UiResult::Used,
                };
            }
            // Nothing there: the code, where it is (an executor style's).
            if st.s.ui.tab == Tab::Editor && editor_point(st, game, at.0, at.1, false, drawn) {
                st.s.ui.dragging = true;
                set_focus(&mut st.s.ui, Pane::Code);
            }
            UiResult::Used
        }
        Input::Wheel(steps) => {
            let n = (steps.abs().ceil() as usize).max(1) * 3;
            let up = steps > 0.0;
            // Nothing scrolls under the naming prompt.
            if st.s.ui.naming.is_some() {
                return UiResult::Used;
            }
            // Over a menu that scrolls, it scrolls; anywhere else the wheel
            // closes the menu, and goes no further.
            let mouse = st.s.ui.mouse;
            if let Some(menu) = st.s.ui.menu.as_mut() {
                if !menu.rect.is_some_and(|r| inside(r, mouse)) {
                    st.s.ui.menu = None;
                } else if menu.scrolls() {
                    menu.scroll = if up {
                        menu.scroll.saturating_sub(n / 3)
                    } else {
                        menu.scroll + n / 3
                    };
                    // The lit item is let go of: the wheel shows the others.
                    menu.sel = None;
                }
                return UiResult::Used;
            }
            // The script list, wherever it is drawn (Synapse has it beside every page).
            if let Some(r) = st.s.ui.scripts_at
                && inside(r, st.s.ui.mouse)
            {
                let ui = &mut st.s.ui;
                ui.scripts_scroll = if up {
                    ui.scripts_scroll.saturating_sub(n / 3)
                } else {
                    ui.scripts_scroll + n / 3
                };
                return UiResult::Used;
            }
            match st.s.ui.tab {
                // The pane under the pointer (else the code's).
                Tab::Editor if st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio => {
                    let pane = pane_under(&st.s.ui, mouse).map_or(Pane::Code, |(p, _)| p);
                    pane_wheel(st, game, pane, n, up);
                }
                Tab::Console => pane_wheel(st, game, Pane::Console, n, up),
                Tab::Editor => pane_wheel(st, game, Pane::Code, n, up),
                Tab::Library => pane_wheel(st, game, Pane::Library, n, up),
                Tab::Docs => pane_wheel(st, game, Pane::Docs, n, up),
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
                // A delete asks; nothing is typed.
                if !matches!(st.s.ui.naming_kind, NamingKind::Delete(_)) {
                    name.insert(&t);
                }
                return UiResult::Used;
            }
            // An open menu takes what is typed (nothing reaches what is under it).
            if st.s.ui.menu.is_some() {
                return super::hstudio_menus::menu_text(st, game, &t);
            }
            end_sizing(st);
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
            // The Library's letters, as a page or a pane.
            if key_pane(&st.s.ui) == Some(Pane::Library) {
                match t.as_str() {
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
                    // A new file of the module chosen, or the script chosen made a module.
                    "f" | "F" => {
                        let i = st.s.ui.library_sel;
                        if st.s.mods.get(i).is_some_and(|m| m.folder.is_some()) {
                            return act(st, game, Action::NewFileIn(i));
                        }
                    }
                    "m" | "M" => {
                        let i = st.s.ui.library_sel;
                        if st.s.mods.get(i).is_some_and(|m| m.folder.is_none()) {
                            return act(st, game, Action::MakeModule(i));
                        }
                    }
                    // Its menu, as its "..." opens it: Up/Down and Enter choose.
                    "." => {
                        let i = st.s.ui.library_sel;
                        let (x, y) = st.s.ui.mouse;
                        let at = st.s.ui.library_dots.unwrap_or([x, y, 0.0, 0.0]);
                        super::hstudio_menus::open_library_menu(st, i, at);
                    }
                    _ => {}
                }
            }
            UiResult::Used
        }
        Input::Key { key, down, mods, .. } => {
            if !down {
                return UiResult::Used;
            }
            key_down(st, game, key, mods)
        }
        Input::Middle { .. } => UiResult::Used,
    }
}

/// A click's action for what the last frame drew: a mod named by its place
/// in the list then is found by its id now, as a scan since may have taken
/// away a mod listed before it. None when that mod is gone.
fn as_drawn(st: &Studio, action: Action) -> Option<Action> {
    let now = |i: usize| {
        let id = st.s.ui.drawn_mods.get(i)?;
        st.s.mods.iter().position(|m| &m.id == id)
    };
    Some(match action {
        Action::Select(i) => Action::Select(now(i)?),
        Action::Toggle(i) => Action::Toggle(now(i)?),
        Action::Edit(i) => Action::Edit(now(i)?),
        Action::Reload(i) => Action::Reload(now(i)?),
        Action::NewFileIn(i) => Action::NewFileIn(now(i)?),
        Action::MakeModule(i) => Action::MakeModule(now(i)?),
        Action::LibraryMenu(i) => Action::LibraryMenu(now(i)?),
        other => other,
    })
}

/// The place in the editor's text drawn under `(x, y)`: its row and char
/// column. Rows glide as the view scrolls, so the view as drawn (its first
/// row, not where it is going) says which, given as `drawn`; and an agent's
/// edit may have typed more in since the text was drawn, so the place is
/// found in the text as drawn, then where that is now (`Drawn`): the view
/// as drawn moved since with the text it shows (`Drawn::moved`) is where
/// it was for the rows as drawn. Left of the text (over the line numbers)
/// it is the line's start.
pub(crate) fn editor_pos(st: &Studio, game: &mut dyn GameApi, x: f32, y: f32, drawn: f32) -> Option<(usize, usize)> {
    // A press that put an agent's edit in whole is for the text drawn under
    // it, wherever the edit moved that.
    if let Some((at, place)) = st.s.ui.coach.pressed
        && at == (x, y)
    {
        return Some(place);
    }
    let v = st.s.ui.view?;
    let ed = &st.s.editor;
    let at = drawn.max(0.0) + ((y - v.y) / code_line()).clamp(0.0, v.rows as f32);
    let scale = st.s.ui.look.scale;
    // While an agent's edit typed in, the rows were drawn before the frame
    // typed more: the place drawn under the pointer, where its text is now.
    if let Some(d) = ed.drawn.as_ref().filter(|d| d.rev == ed.revision) {
        let row = ((at - d.moved).max(0.0).floor() as usize).min(d.count.saturating_sub(1));
        if let Some(line) = row.checked_sub(d.first).and_then(|i| d.lines.get(i)) {
            let col = if x < v.x {
                0
            } else {
                d.hscroll + column_at(game, line, d.hscroll, x - v.x, scale)
            };
            let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
            let place = d
                .splices
                .iter()
                .fold((row, byte), |p, &(a, b, e)| after_splice(p, a, b, e));
            let (row, byte) = ed.clamp_pos(place);
            return Some((row, ed.lines[row][..byte].chars().count()));
        }
    }
    let row = (at.floor() as usize).min(ed.lines.len() - 1);
    let col = if x < v.x {
        0
    } else {
        ed.hscroll + column_at(game, &ed.lines[row], ed.hscroll, x - v.x, scale)
    };
    Some((row, col))
}

/// Whether a press at `(x, y)` is on the code's text as the view was drawn
/// (its line numbers too).
fn on_text(v: &EditorView, (x, y): (f32, f32)) -> bool {
    x >= v.x - 30.0 && y >= v.y && y <= v.y + code_line() * v.rows as f32 && x <= v.x + v.w + 4.0
}

/// Whether a press at `at` (the left button, or the right) lands on the
/// code, by what the last frame drew, as `handle_input` and `right_press`
/// take it: a left one puts the cursor in its text (no prompt or menu up,
/// nothing else drawn there), a right one opens the code's menu there (no
/// prompt up, nothing dragged, not on the menu up). It is for the line
/// drawn under the pointer.
pub(crate) fn press_on_code(ui: &Ui, at: (f32, f32), left: bool) -> bool {
    if ui.naming.is_some() || ui.tab != Tab::Editor {
        return false;
    }
    if !left {
        let on_menu = ui.menu.as_ref().and_then(|m| m.rect).is_some_and(|r| inside(r, at));
        return !ui.dragging_any() && !on_menu && matches!(spot_at(ui, at), Some((Spot::Code, _)));
    }
    ui.menu.is_none()
        && matches!(hit_at(ui, at), None | Some(Action::Focus(Pane::Code)))
        && ui.view.is_some_and(|v| on_text(&v, at))
}

/// A click or drag in the editor's text: moves the cursor there (a drag
/// keeps the selection's other end).
fn editor_point(st: &mut Studio, game: &mut dyn GameApi, x: f32, y: f32, drag: bool, drawn: f32) -> bool {
    let Some(v) = st.s.ui.view else {
        return false;
    };
    if !drag && !on_text(&v, (x, y)) {
        return false;
    }
    let Some((row, col)) = editor_pos(st, game, x, y, drawn) else {
        return false;
    };
    let ed = &mut st.s.editor;
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
/// chosen (Home and End to the first and the last), Enter chooses (Space
/// too, as text), Escape and the Menu key close it, and the studio's key
/// closes it with the studio; others are kept from what is under it.
fn menu_key(st: &mut Studio, game: &mut dyn GameApi, key: Key) -> UiResult {
    let Some(menu) = st.s.ui.menu.as_mut() else {
        return UiResult::Used;
    };
    let choosable: Vec<usize> = (0..menu.items.len()).filter(|&i| menu.items[i].enabled).collect();
    let at = menu.sel.and_then(|s| choosable.iter().position(|&i| i == s));
    match key {
        Key::Escape | Key::Menu => st.s.ui.menu = None,
        Key::Toggle => {
            st.s.ui.menu = None;
            return UiResult::Close;
        }
        Key::Down if !choosable.is_empty() => {
            menu.sel = Some(choosable[at.map_or(0, |a| (a + 1) % choosable.len())]);
        }
        Key::Up if !choosable.is_empty() => {
            menu.sel = Some(choosable[at.map_or(choosable.len() - 1, |a| (a + choosable.len() - 1) % choosable.len())]);
        }
        Key::Home => menu.sel = choosable.first().copied(),
        Key::End => menu.sel = choosable.last().copied(),
        Key::Enter => {
            if let Some(i) = menu.sel {
                return menu_pick(st, game, i);
            }
        }
        _ => {}
    }
    UiResult::Used
}

/// Item `i` of the open menu chosen: the menu closes and its command runs
/// (Close: the studio closes with it).
pub(crate) fn menu_pick(st: &mut Studio, game: &mut dyn GameApi, i: usize) -> UiResult {
    let Some(menu) = st.s.ui.menu.take() else {
        return UiResult::Used;
    };
    // The code it was for changed under it: it just goes.
    if menu.stale(&st.s.editor) {
        return UiResult::Used;
    }
    let Some(cmd) = menu.items.get(i).filter(|it| it.enabled).and_then(|it| it.cmd.clone()) else {
        return UiResult::Used;
    };
    let place = menu.rect.unwrap_or([menu.at[0], menu.at[1], 0.0, 0.0]);
    super::hstudio_menus::run(st, game, cmd, place)
}

/// Opens a file in the editor (unsaved edits in the one open saved first),
/// unless it is open already, the code at the front with the keys.
pub(crate) fn open_path(st: &mut Studio, game: &mut dyn GameApi, path: &Path) {
    st.save_before_switch(game, Some(path));
    if st.s.editor.path.as_deref() != Some(path) {
        st.open_file(path);
    }
    show_code(st);
}

/// Script `i` made a module, unfolded in the script list.
pub(crate) fn make_module(st: &mut Studio, game: &mut dyn GameApi, i: usize) {
    let id = st.s.mods[i].id.clone();
    match st.make_module(game, i) {
        Ok(_) => {
            st.s.ui.expanded.insert(id);
        }
        Err(e) => st.s.log(LogKind::Error, e),
    }
}

/// A click on row `k` of the script list: a file opens, a module folds or
/// unfolds.
fn script_row(st: &mut Studio, game: &mut dyn GameApi, k: usize) {
    let Some(row) = st.s.ui.script_rows.get(k).cloned() else {
        return;
    };
    match row {
        ScriptRow::Scratch => {
            act(st, game, Action::EditScratch);
        }
        ScriptRow::Script { path, .. } | ScriptRow::File { path, .. } => open_path(st, game, &path),
        ScriptRow::Module { id, .. } => {
            if !st.s.ui.expanded.remove(&id) {
                st.s.ui.expanded.insert(id);
            }
        }
    }
}

/// Says something in the studio a moment (`Ui::notice`): what a menu's
/// item did, or why it couldn't.
pub(crate) fn notice(st: &mut Studio, text: impl Into<String>) {
    st.s.ui.notice = Some((text.into(), st.s.now));
}

/// Puts `text` on the clipboard and says how much it was (nothing for no text).
pub(crate) fn copy_text(st: &mut Studio, game: &mut dyn GameApi, text: &str) {
    if text.is_empty() {
        return;
    }
    game.set_clipboard(text);
    notice(st, format!("Copied {} characters", text.chars().count()));
}

/// Opens the naming prompt for `kind`, its field holding `text`.
pub(crate) fn open_naming(st: &mut Studio, kind: NamingKind, text: &str) {
    let mut edit = LineEdit::default();
    edit.set(text);
    let ui = &mut st.s.ui;
    ui.naming = Some(edit);
    ui.naming_kind = kind;
    ui.naming_at = st.s.now;
    ui.menu = None;
    ui.suggest = None;
}

/// The module whose file the editor has open: its index and folder.
pub(crate) fn open_module(st: &Studio) -> Option<(usize, PathBuf)> {
    let i = st.s.editor.path.as_deref().and_then(|p| st.mod_of_path(p))?;
    Some((i, st.s.mods[i].folder.clone()?))
}

/// A file of a module (`path`): the module's folder and the file's path in it.
pub(crate) fn file_in_module(st: &Studio, path: &Path) -> Option<(PathBuf, String)> {
    let folder = st.s.mods[st.mod_of_path(path)?].folder.clone()?;
    let rel = super::hstudio_modfiles::rel_in(&folder, path)?;
    Some((folder, rel))
}

/// What a path of the mods folder is called in the prompts: a module's
/// file by its path in the module, a module by its id, a script by its file.
pub(crate) fn prompt_name(st: &Studio, path: &Path) -> String {
    match st.mod_of_path(path).map(|i| &st.s.mods[i]) {
        Some(m) if m.main == path && m.folder.is_none() => format!("{}.lua", m.id),
        Some(m) if m.folder.as_deref() == Some(path) => m.id.clone(),
        _ => file_in_module(st, path).map_or_else(
            || {
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            },
            |(_, rel)| rel,
        ),
    }
}

/// The line under the naming prompt's field: what Enter will make, or
/// (an Err) why it can't, and then Enter does nothing. Empty: nothing to
/// say yet.
pub(crate) fn naming_plan(st: &Studio) -> Result<String, String> {
    use super::hstudio_modfiles::{plan_file, plan_mod};
    let typed = st.s.ui.naming.as_ref().map_or("", |n| n.text.as_str());
    let mods = st.s.mods_dir();
    match &st.s.ui.naming_kind {
        NamingKind::NewScript | NamingKind::SaveAs => plan_mod(&mods, typed, false),
        NamingKind::NewModule => plan_mod(&mods, typed, true),
        NamingKind::Model => Ok(String::new()),
        NamingKind::NewFile(folder) => plan_file(folder, typed, None),
        NamingKind::Rename(path) => match file_in_module(st, path) {
            Some((folder, rel)) => plan_file(&folder, typed, Some(&rel)),
            None => Err("that file is gone".into()),
        },
        NamingKind::Delete(path) => {
            let rel = path.strip_prefix(&mods).ok().and_then(|r| {
                let parts: Option<Vec<&str>> = r.components().map(|c| c.as_os_str().to_str()).collect();
                parts.map(|p| p.join("/"))
            });
            // What the trash has at that name already stays, beside it.
            let there = path.strip_prefix(&mods).ok().map(|r| st.trash_dir().join(r));
            let kept = match there.filter(|t| std::fs::symlink_metadata(t).is_ok()) {
                Some(t) if t.is_dir() && path.is_dir() => {
                    "\nIt joins the folder there; a file both have keeps the older copy, numbered.".to_owned()
                }
                Some(t) => {
                    let name = super::hstudio_modfiles::numbered(&t);
                    let name = name
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    format!("\nThe older copy there stays, as {name}.")
                }
                None => String::new(),
            };
            Ok(format!(
                "It moves to trash/{} beside the mods folder.{kept}",
                rel.unwrap_or_else(|| prompt_name(st, path))
            ))
        }
        NamingKind::RenameMod(id) => st.rename_plan(id, typed),
        NamingKind::Duplicate(id) => st.duplicate_plan(id, typed),
    }
}

/// Enter in the naming prompt: nothing while its line says why it can't;
/// else the prompt closes and it makes, renames or deletes what it names.
/// A new mod or file opens in the editor, unsaved edits in the file open
/// before saved first.
fn finish_naming(st: &mut Studio, game: &mut dyn GameApi) {
    if naming_plan(st).is_err_and(|e| !e.is_empty()) {
        return;
    }
    let wanted =
        st.s.ui
            .naming
            .take()
            .map(|n| n.text.trim().to_owned())
            .unwrap_or_default();
    let kind = std::mem::take(&mut st.s.ui.naming_kind);
    let made = match kind {
        NamingKind::Model => {
            set_agent_model(st, Some(wanted).filter(|m| !m.is_empty()));
            return;
        }
        NamingKind::Delete(path) => st.delete_path(game, &path).map(|_| None),
        _ if wanted.is_empty() => return,
        NamingKind::SaveAs => {
            let code = st.s.editor.text();
            st.create_mod(game, &wanted, Some(&code)).map(Some)
        }
        NamingKind::NewScript => {
            st.save_before_switch(game, None);
            st.create_mod(game, &wanted, None).map(Some)
        }
        NamingKind::NewModule => {
            st.save_before_switch(game, None);
            st.create_module(game, &wanted).map(Some)
        }
        NamingKind::NewFile(folder) => {
            st.save_before_switch(game, None);
            st.add_file(game, &folder, &wanted).map(Some)
        }
        NamingKind::Rename(path) => st.rename_file(game, &path, &wanted).map(|_| None),
        NamingKind::RenameMod(id) => st.rename_mod(game, &id, &wanted).map(|_| None),
        NamingKind::Duplicate(id) => match st.duplicate_mod(game, &id, &wanted) {
            // The copy is chosen in the Library, and opens in the Editor.
            Ok(main) => {
                if let Some(i) = st.s.mods.iter().position(|m| m.main == main) {
                    st.s.ui.library_sel = i;
                }
                let new_id = super::hstudio::mod_id(&wanted);
                notice(st, format!("Made {new_id}, turned off"));
                let editing = st.s.ui.tab == Tab::Editor;
                if editing {
                    st.save_before_switch(game, Some(&main));
                }
                Ok(editing.then_some(main))
            }
            Err(e) => Err(e),
        },
    };
    match made {
        Ok(Some(path)) => {
            st.open_file(&path);
            show_code(st);
        }
        Ok(None) => {}
        Err(e) => {
            notice(st, e.clone());
            st.s.log(LogKind::Error, e);
        }
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

/// The agent's models, the current one marked, and a model typed by name
/// (empty when the agent in use takes no model).
pub(crate) fn model_items(st: &mut Studio) -> Vec<MenuItem> {
    let Some(id) = agent_id(st) else { return Vec::new() };
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
    items
}

/// The agent's models in a menu under its chip, the current one lit.
fn open_model_menu(st: &mut Studio) {
    let items = model_items(st);
    if items.is_empty() {
        return;
    }
    let chip =
        st.s.ui
            .model_chip
            .unwrap_or([st.s.ui.mouse.0, st.s.ui.mouse.1, 0.0, 0.0]);
    let mut menu = Menu::new([chip[0], chip[1] + chip[3] + 4.0], chip[3], items, st.s.now);
    menu.sel = menu.items.iter().position(|i| i.checked);
    st.s.ui.menu = Some(menu);
}

pub(crate) fn act(st: &mut Studio, game: &mut dyn GameApi, action: Action) -> UiResult {
    // A row drawn last frame may be gone (the folder changed since).
    if let Action::Select(i)
    | Action::Toggle(i)
    | Action::Edit(i)
    | Action::Reload(i)
    | Action::NewFileIn(i)
    | Action::MakeModule(i)
    | Action::LibraryMenu(i) = action
        && i >= st.s.mods.len()
    {
        return UiResult::Used;
    }
    if let Action::ScriptRow(k) | Action::ScriptMenu(k) = action
        && usize::from(k) >= st.s.ui.script_rows.len()
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
            // By its path: the save before may rescan the mods.
            let main = st.s.mods[i].main.clone();
            st.save_before_switch(game, Some(&main));
            st.open_file(&main);
            show_code(st);
        }
        Action::EditScratch => {
            let path = st.scratch_path();
            st.save_before_switch(game, Some(&path));
            if st.s.editor.path.as_ref() != Some(&path) {
                st.s.editor.open(&path, "scratch.lua");
                st.s.editor.error_line = None;
            }
            show_code(st);
        }
        Action::SaveFile => {
            // Any file of a mod saves in place; scratch code asks for a name.
            if st.s.editor.path.as_deref().and_then(|p| st.mod_of_path(p)).is_some() {
                st.save_editor(game);
            } else {
                open_naming(st, NamingKind::SaveAs, "");
            }
        }
        Action::Reload(i) => {
            st.s.mods[i].enabled = true;
            st.set_enabled(game, i, true);
        }
        Action::NewMod => open_naming(st, NamingKind::NewScript, ""),
        Action::NewMenu => {
            let (x, y) = st.s.ui.mouse;
            let at = st.s.ui.new_button.unwrap_or([x, y, 0.0, 0.0]);
            super::hstudio_menus::open_new_menu(st, at);
        }
        Action::NewKind(module) => {
            let ui = &mut st.s.ui;
            if matches!(ui.naming_kind, NamingKind::NewScript | NamingKind::NewModule) {
                ui.naming_kind = if module {
                    NamingKind::NewModule
                } else {
                    NamingKind::NewScript
                };
            }
        }
        Action::ScriptRow(k) => script_row(st, game, usize::from(k)),
        Action::ScriptMenu(k) => {
            let at = st.s.ui.mouse;
            super::hstudio_menus::open_row_menu(st, usize::from(k), [at.0, at.1, 0.0, 0.0]);
        }
        Action::NewFileIn(i) => {
            if let Some(folder) = st.s.mods[i].folder.clone() {
                open_naming(st, NamingKind::NewFile(folder), "");
            }
        }
        Action::MakeModule(i) => make_module(st, game, i),
        Action::LibraryMenu(i) => {
            st.s.ui.library_sel = i;
            let (x, y) = st.s.ui.mouse;
            let at = st.s.ui.library_dots.unwrap_or([x, y, 0.0, 0.0]);
            super::hstudio_menus::open_library_menu(st, i, at);
        }
        Action::EditFile(k) => {
            // The file drawn, while it is still a file of a mod.
            let path = st.s.ui.drawn_files.get(usize::from(k)).cloned();
            if let Some(path) = path.filter(|p| p.is_file() && st.mod_of_path(p).is_some()) {
                open_path(st, game, &path);
                // At the line it failed at, if it did.
                let ed = &mut st.s.editor;
                if let Some(line) = ed.error_line {
                    ed.click(line.saturating_sub(1), 0, false);
                }
            }
        }
        Action::ExecuteFile => {
            if !st.reload_editor_mod(game) {
                st.run_editor(game);
            }
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
        Action::PaneTab(p) => focus_pane(st, p),
        Action::Focus(p) => {
            focus_pane(st, p);
            st.s.ui.suggest = None;
        }
        Action::ShowPane(p) => {
            st.s.ui.tab = Tab::Editor;
            show_pane(st, p);
        }
        Action::HideGroup(p) => hide_group(st, p),
        Action::ToggleGroup(p) => toggle_group(st, p),
        Action::DockBar(_) => {}
        Action::ResetLayout => reset_layout(st),
        Action::UndoReset => undo_reset(st),
        Action::ClosePane(p) => {
            if st.s.ui.dock.close(p) {
                dock_changed(st);
            }
        }
        Action::MovePane(p, at) => {
            if st.s.ui.dock.move_pane(p, at) {
                st.s.ui.shown_at[p.index()] = st.s.now;
                st.s.ui.dock_tips = false;
                // A drop ends its drag, so a tab from the top leaves its page
                // behind, also where its group stays as it was.
                st.s.ui.dock_drag = None;
                focus_pane(st, p);
            }
        }
        Action::SizePane(p) => size_pane(st, p),
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
        Action::KeepMine => {
            st.s.editor.conflict = false;
            st.s.editor.removed = false;
        }
        Action::CloseRemoved => st.close_removed(),
        Action::AgentNext => agent_next(st),
        Action::AgentStop => agent_stop(st),
        Action::Block | Action::ScrollBar | Action::Spot(_) => {}
        Action::MenuPick(i) => return menu_pick(st, game, i as usize),
        Action::ModelMenu => open_model_menu(st),
        Action::AgentJump(seq) => super::hstudio_coach::go(st, game, seq),
        // The notes' or the lesson's chip clicked on the code's strip while
        // the code is only its strip: the player asked to see the code, so
        // it comes back with the keys, then shows the note or the step.
        Action::NextMark | Action::LessonShow
            if st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio
                && !st.s.ui.dock_front[Pane::Code.index()] =>
        {
            show_code(st);
            super::hstudio_coach::act(st, game, action);
        }
        Action::MarkOpen(_)
        | Action::MarkClose(_)
        | Action::MarkMore
        | Action::NextMark
        | Action::ClearMarks
        | Action::LessonShow
        | Action::LessonBack
        | Action::LessonNext
        | Action::LessonCheck
        | Action::LessonEnd => super::hstudio_coach::act(st, game, action),
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
        let delete = matches!(st.s.ui.naming_kind, NamingKind::Delete(_));
        match key {
            Key::Escape => {
                st.s.ui.naming = None;
                st.s.ui.naming_kind = NamingKind::default();
            }
            Key::Enter => finish_naming(st, game),
            // Tab goes between a script and a module.
            Key::Tab => {
                let ui = &mut st.s.ui;
                ui.naming_kind = match ui.naming_kind {
                    NamingKind::NewScript => NamingKind::NewModule,
                    NamingKind::NewModule => NamingKind::NewScript,
                    ref other => other.clone(),
                };
            }
            k if !delete => {
                name.key(k, mods);
            }
            _ => {}
        }
        return UiResult::Used;
    }
    if st.s.ui.menu.is_some() {
        return menu_key(st, game, key);
    }
    if sizing_key(st, key, mods) {
        return UiResult::Used;
    }
    if key == Key::Menu {
        return super::hstudio_menus::key_menu(st, game);
    }
    if mods.ctrl
        && key == Key::Char('c')
        && let Some(text) = selected_text(&st.s.ui)
    {
        copy_text(st, game, &text);
        return UiResult::Used;
    }
    if key == Key::Escape && st.s.ui.suggest.take().is_some() {
        return UiResult::Used;
    }
    // Esc lets go of what the button holds on the panes: a bar stays where
    // it is, and a tab, lifted or not yet, moves nothing.
    if key == Key::Escape && st.s.ui.dock_drag.is_some() {
        end_dock_drag(st);
        return UiResult::Used;
    }
    // Esc gives the code the keys (and its place at the front) back.
    if key == Key::Escape
        && st.s.ui.tab == Tab::Editor
        && st.s.ui.look.theme().layout == super::hstudio_theme::Layout::Studio
        && key_pane(&st.s.ui) != Some(Pane::Code)
    {
        show_code(st);
        return UiResult::Used;
    }
    if !mods.ctrl && suggest_key(st, key) {
        return UiResult::Used;
    }
    // What agents show in the editor: Esc closes an agent's note first.
    if super::hstudio_coach::key(st, game, key, mods) {
        return UiResult::Used;
    }
    match key {
        Key::Escape | Key::Toggle => return UiResult::Close,
        Key::Char('`' | 'j') if mods.ctrl => {
            return act(st, game, Action::ToggleGroup(Pane::Console));
        }
        Key::Char('b') if mods.ctrl => {
            return act(st, game, Action::ToggleGroup(Pane::Scripts));
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
    match key_pane(&st.s.ui) {
        Some(Pane::Code) => editor_key(st, game, key, mods),
        Some(Pane::Console) => console_key(st, game, key, mods),
        Some(Pane::Agent) => agent_key(st, game, key, mods),
        Some(Pane::Problems) => problems_key(st, key),
        Some(Pane::Library) => library_key(st, game, key),
        Some(Pane::Docs) => docs_key(st, key),
        Some(Pane::Scripts) => {}
        None if st.s.ui.tab == Tab::Settings => settings_key(st, key),
        None => {}
    }
    // Deleting keeps the suggestions in step with the word.
    if open && matches!(key, Key::Backspace | Key::Delete) {
        refresh_suggest(st);
    }
    UiResult::Used
}

/// The Docs' keys: they scroll, a line or a page at a time.
fn docs_key(st: &mut Studio, key: Key) {
    let page = st.s.ui.pages[Pane::Docs.index()].max(1);
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

/// Settings' keys: they scroll it.
fn settings_key(st: &mut Studio, key: Key) {
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

/// The console's input line run, and emptied for the next.
pub(crate) fn run_console_line(st: &mut Studio, game: &mut dyn GameApi) {
    let ui = &mut st.s.ui;
    let line = std::mem::take(&mut ui.input);
    ui.hist = None;
    ui.log_scroll = 0;
    st.run_console(game, &line.text);
}

fn console_key(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) {
    let ui = &mut st.s.ui;
    match key {
        Key::Enter => run_console_line(st, game),
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
        Key::PageUp => ui.log_scroll += ui.pages[Pane::Console.index()].max(1),
        Key::PageDown => ui.log_scroll = ui.log_scroll.saturating_sub(ui.pages[Pane::Console.index()].max(1)),
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

/// One line of the Agent pane's conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentLine {
    pub kind: AgentLineKind,
    pub text: String,
    /// When it came (its motion).
    pub at: f64,
    /// The code it is about, for what an agent did in the editor.
    pub jump: Option<super::hstudio_coach::Jump>,
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
    /// What an agent did in the editor (it typed lines in, opened a file).
    Action,
}

/// The Agent pane: the conversation, the agent working, the conversation
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
    /// Calls of the editor's tools whose lines aren't shown (what they did
    /// has a line of its own), by call id, with the line each would have had.
    pub(crate) hidden: std::collections::HashMap<String, String>,
}

/// Lines kept of the conversation.
const AGENT_LINES: usize = 400;

impl AgentPane {
    pub(crate) fn say(&mut self, kind: AgentLineKind, text: impl Into<String>, at: f64) {
        self.say_jump(kind, text, None, at);
    }
    /// A line about some code, which `jump` goes to.
    pub(crate) fn say_jump(
        &mut self,
        kind: AgentLineKind,
        text: impl Into<String>,
        jump: Option<super::hstudio_coach::Jump>,
        at: f64,
    ) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.seq += 1;
        self.lines.push(AgentLine { kind, text, at, jump });
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
    // Unsaved edits are saved first (the docs say so), so its turn starts
    // from what the player sees with nothing of theirs unsaved: a write_mod
    // to the file then doesn't conflict with them, and the turn's end can
    // save the agent's own edits, which it does only in a buffer that had
    // none of the player's. An agent's edit being typed in is all in first.
    super::hstudio_coach::finish_typing(st, "the player asked the agent");
    if st.s.editor.dirty && st.s.editor.path.is_some() {
        match st.s.editor.save() {
            Ok(path) => {
                st.s.log(LogKind::Info, format!("Saved {} for the agent", path.display()));
                // What an agent left unsaved in it is saved now.
                super::hstudio_coach::written(st);
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
    let files = ed
        .path
        .as_deref()
        .and_then(|p| st.mod_of_path(p))
        .map(|i| st.s.mods[i].files())
        .unwrap_or_default();
    let ctx = super::hstudio_agent::AgentContext {
        file,
        scratch: ed.path.as_deref() == Some(st.scratch_path().as_path()),
        mods_dir: mods_dir.clone(),
        files,
        selection: ed.selected_text().filter(|t| !t.trim().is_empty()),
        problems: ed
            .problems
            .iter()
            .map(|p| format!("{}:{}: {}", ed.title, p.line, p.message))
            .collect(),
        lesson: super::hstudio_coach::lesson_line(st),
    };
    // A new turn: what the player did before, kept for an agent waiting, is stale.
    super::hstudio_coach::new_turn(st);
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
            AgentEvent::Tool { id, name, summary } => {
                let text = if summary.is_empty() { name.clone() } else { summary };
                // An editor tool says what it did in a line of its own.
                match id.filter(|_| super::hstudio_agent::EDITOR_TOOLS.contains(&name.as_str())) {
                    Some(id) => {
                        pane.hidden.insert(id, text);
                    }
                    None => pane.say(AgentLineKind::Tool, text, now),
                }
            }
            AgentEvent::ToolResult { id, ok, summary } => {
                let hidden = id.and_then(|id| pane.hidden.remove(&id));
                if let Some(call) = hidden {
                    if ok {
                        continue;
                    }
                    pane.say(AgentLineKind::Tool, call, now);
                }
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
                pane.hidden.clear();
                // Its edits in the editor are saved now, or it says they aren't.
                st.s.ui.coach.turn_ended(ok);
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

/// The agents found on this computer (looked for the first time they are
/// wanted): each one's id and name, and which is in use.
pub fn agent_list(st: &mut Studio) -> (Vec<(String, String)>, usize) {
    let dir = st.s.dir.clone();
    let pane = &mut st.s.ui.agent;
    let chosen = pane.chosen;
    let found: Vec<(String, String)> = pane
        .agents(&dir)
        .iter()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    let chosen = chosen.min(found.len().saturating_sub(1));
    (found, chosen)
}

/// Uses the agent found with this id; another than the one in use starts
/// a new conversation (not while the agent works).
pub fn agent_choose(st: &mut Studio, id: &str) {
    if agent_busy(st) {
        return;
    }
    let now = st.s.now;
    let dir = st.s.dir.clone();
    let pane = &mut st.s.ui.agent;
    let before = pane.chosen;
    let Some((k, spec)) = pane
        .agents(&dir)
        .iter()
        .enumerate()
        .find(|(_, a)| a.id == id)
        .map(|(k, a)| (k, a.clone()))
    else {
        return;
    };
    pane.chosen = k;
    if k != before {
        pane.session = None;
    }
    let text = format!("Using {} ({}).", spec.name, spec.program.display());
    pane.say(AgentLineKind::Done, text, now);
}

/// Looks for agents again, keeping the one in use when it is still found.
pub fn agent_rescan(st: &mut Studio) {
    if agent_busy(st) {
        return;
    }
    let now = st.s.now;
    let kept = agent_id(st);
    let found = super::hstudio_agent::detect(&st.s.dir);
    let pane = &mut st.s.ui.agent;
    pane.agents = Some(found.clone());
    if found.is_empty() {
        pane.chosen = 0;
        pane.say(
            AgentLineKind::Error,
            "No agent found: install Claude Code (claude) or omp, or name one in agent.txt in the studio's folder.",
            now,
        );
        return;
    }
    match kept.and_then(|id| found.iter().position(|a| a.id == id)) {
        Some(k) => pane.chosen = k,
        None => {
            pane.chosen = 0;
            pane.session = None;
        }
    }
    let names: Vec<&str> = found.iter().map(|a| a.name.as_str()).collect();
    pane.say(AgentLineKind::Done, format!("Found {}.", names.join(", ")), now);
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
        show_code(st);
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

/// The agent asked what is typed in its line, which empties (while it
/// works, what is typed waits there).
pub(crate) fn agent_send(st: &mut Studio) {
    if agent_busy(st) {
        return;
    }
    let ask = std::mem::take(&mut st.s.ui.agent_input).text;
    agent_ask(st, ask.trim());
}

/// A new conversation: its lines go, and the next question starts afresh
/// (not while the agent works).
pub fn agent_new_conversation(st: &mut Studio) {
    if agent_busy(st) {
        return;
    }
    let now = st.s.now;
    if st.s.ui.text_sel.is_some_and(|s| s.area == AREA_AGENT) {
        st.s.ui.text_sel = None;
    }
    let pane = &mut st.s.ui.agent;
    pane.lines.clear();
    pane.session = None;
    pane.scroll = 0.0;
    pane.say(AgentLineKind::Done, "New conversation.", now);
}

/// The Agent pane's input: Enter asks, the rest edits the line.
fn agent_key(st: &mut Studio, game: &mut dyn GameApi, key: Key, mods: Mods) {
    let _ = game;
    match key {
        // An agent waiting for the player gets what they write.
        Key::Enter if super::hstudio_coach::waiting(st) => {
            let reply = std::mem::take(&mut st.s.ui.agent_input).text;
            super::hstudio_coach::reply(st, reply.trim());
        }
        // While it works, what is typed waits in the line.
        Key::Enter if !agent_busy(st) => agent_send(st),
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
    let page = st.s.ui.pages[Pane::Code.index()].max(1) as isize;
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
        // The Menu key is the studio's: a script screen sees it as any other.
        Key::Menu | Key::Other => "other".into(),
    }
}

/// The modifier keys as a screen's `key` gets them: {ctrl, shift, alt}.
fn mods_value(mods: Mods) -> Value {
    let mut t = super::hstudio_lua::Table::new();
    t.set_str("ctrl", Value::Bool(mods.ctrl));
    t.set_str("shift", Value::Bool(mods.shift));
    t.set_str("alt", Value::Bool(mods.alt));
    Value::table(t)
}

/// Escape on a screen: its `escape()` returning true keeps it open (a
/// field of its own let go of the focus, as a Roblox TextBox does).
fn escape_kept(st: &mut Studio, game: &mut dyn GameApi, table: &TableRef) -> bool {
    let f = field(table, "escape");
    st.screen_call(game, &f, Vec::new(), false)
        .is_some_and(|r| r.first().is_some_and(Value::truthy))
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
        Input::Middle { down } => {
            // Its click and release, wherever it is (no widget takes it).
            let f = field(&table, if down { "click" } else { "release" });
            if f.is_callable() {
                let (x, y) = st.s.ui.mouse;
                let at = vec![
                    Value::Num(f64::from(x / unit)),
                    Value::Num(f64::from(y / unit)),
                    Value::str("middle"),
                ];
                st.screen_call(game, &f, at, false);
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
                if key == Key::Escape && escape_kept(st, game, &table) {
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
                    let args = vec![Value::str(&key_name(key)), Value::Bool(down), mods_value(mods)];
                    st.screen_call(game, &f, args, false);
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
