// The agent in the editor: what an AI agent connected through the MCP
// server (horizon-studio-mcp, over the AI link) does in the player's editor
// while they watch. It opens any script, reads the buffer as the player has
// it, and types its changes into it with a caret of its own. Its state lives
// in the studio's UI (`Ui::coach`); ui.rs hands it the player's input,
// look.rs draws it, link.rs offers it as the link's editor methods, and the
// studio's frame moves it on.
//
// The player comes first: their keys always land where they type, an agent
// never switches their file or tab while they are using the studio, and
// what they are typing is never saved for them.

use super::hstudio::{GameApi, ModState, Studio, mod_id};
use super::hstudio_modfiles::{file_name, path_in, plan_mod};
use super::hstudio_ui::{AgentLineKind, Editor, Input, Tab};
use serde_json::{Value as Json, json};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// How long after the player last changed the code, or used the studio at
/// all, they count as busy with it (the studio's clock, seconds).
pub const HANDS_OFF: f64 = 2.0;
/// The most lines `editor` gives at once (an agent reads a page at a time).
pub const READ_PAGE: usize = 400;
/// The answers kept for requests that change something, by their key.
const REPLIES: usize = 32;
/// The most of the player's selection `editor` gives, in characters.
const SELECTION_CHARS: usize = 4000;
/// How fast an agent's edit is typed in: non-space characters a second, and
/// the shortest and longest an edit takes (a long one comes in faster).
pub const CHARS_PER_SECOND: f64 = 70.0;
const TYPE_MIN: f64 = 0.25;
const TYPE_MAX: f64 = 2.0;
/// How long the text an edit replaces shows, tinted, before it goes (longer
/// when the view had to scroll to it).
pub const SHOW_OLD: f64 = 0.3;
const SHOW_OLD_SCROLLED: f64 = 0.45;
/// How long the lines an edit put in stay lit.
pub const FLASH: f64 = 1.2;
/// The most an edit may put in, in bytes.
const MAX_EDIT: usize = 256 << 10;
/// How long a finished edit's result is kept for `typed`, in seconds.
const RESULTS_KEPT: f64 = 30.0;

/// Who asks: the agent's name as it gave it to the link, and its connection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct By {
    pub name: String,
    pub conn: u64,
}

/// What AI agents do in the editor, and what the player last did there.
pub struct Coach {
    /// When the player last changed the code (an edit of theirs, not one
    /// that opened another file).
    pub player_at: f64,
    /// When they last pressed a key or a button, typed or turned the wheel.
    pub input_at: f64,
    /// The answers to the last requests that changed something, by the
    /// key their client gave: one sent again (after a timeout, on a new
    /// connection) gets the same answer and isn't done twice.
    replies: VecDeque<(String, Result<Json, String>)>,
    /// The edit being typed in, if one is.
    pub typing: Option<Typing>,
    /// The lines an edit just put in, lit a moment.
    pub flash: Option<Flash>,
    /// An agent's edits in the editor's file that nobody has saved since.
    pub unsaved: Option<Unsaved>,
    /// Finished edits' results by their id, with when they finished.
    results: Vec<(u64, f64, Json)>,
    next_edit: u64,
    /// An agent changed a file the editor doesn't show: a note over the
    /// code, which shows it.
    pub notice: Option<Notice>,
    /// The game's own agent's turn just ended (well or not): its unsaved
    /// edits are saved, or a line says they aren't.
    turn_over: Option<bool>,
}

/// A note over the code: what an agent changed elsewhere, its Agent panel
/// line (whose jump shows it), and when.
pub struct Notice {
    pub text: String,
    pub seq: u64,
    pub at: f64,
}

/// How long the note over the code shows, in seconds.
pub const NOTICE_SECS: f64 = 6.0;

impl Default for Coach {
    fn default() -> Self {
        Coach {
            player_at: f64::NEG_INFINITY,
            input_at: f64::NEG_INFINITY,
            replies: VecDeque::new(),
            typing: None,
            flash: None,
            unsaved: None,
            results: Vec::new(),
            next_edit: 1,
            notice: None,
            turn_over: None,
        }
    }
}

/// Some code an Agent panel line is about: a file and its rows (from 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jump {
    pub path: PathBuf,
    pub file: String,
    pub first: usize,
    pub last: usize,
}

/// Where an edit being typed in is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The text it replaces shows, tinted, before it goes.
    ShowOld,
    /// The new text comes in.
    Typing,
}

/// An agent's edit being typed into the editor, a few characters a frame,
/// with a caret of its own. Its splices take no undo step (the edit took
/// one at its start) and keep the player's cursor where it is in the text.
pub struct Typing {
    pub id: u64,
    pub by: By,
    /// Whether the game's own agent made it, in its run.
    in_run: bool,
    pub path: PathBuf,
    pub file: String,
    pub phase: Phase,
    since: f64,
    secs: f64,
    show_old: f64,
    /// What goes (from `from` to `old_to`), and what comes in its place.
    pub from: (usize, usize),
    pub old_to: (usize, usize),
    pub text: String,
    /// How much of `text` is in so far (bytes), and its non-space characters.
    pub shown: usize,
    chars: usize,
    /// The agent's caret: where the rest of `text` goes.
    pub at: (usize, usize),
    /// The editor's revision after the edit's own last splice: a change of
    /// the buffer by anything else drops the rest.
    rev: u64,
    /// The player scrolled away, so the view stops following the caret;
    /// where the edit last put the view.
    pub free: bool,
    scroll: usize,
    /// Whether the player's cursor and the selection's other end were in
    /// the text that went: they stay at the new text's start.
    parked: (bool, bool),
    /// Whether it is typed in as the player watches (or put in at once).
    animated: bool,
    save: bool,
    /// The lines of the whole new text once it is in (from 1), or the
    /// whole lines it removes; and whether the buffer had unsaved edits of
    /// the player's before it.
    lines: (usize, usize),
    removed: Option<(usize, usize)>,
    player_edits: bool,
}

/// The lines an edit just put in, and when.
pub struct Flash {
    pub path: PathBuf,
    pub first: usize,
    pub last: usize,
    pub at: f64,
}

/// An agent's edits nobody has saved since: in which file, by whom, the
/// editor's revision after the last of them (another change since was
/// someone else's), and whether the player had unsaved edits before them.
pub struct Unsaved {
    pub path: PathBuf,
    pub file: String,
    pub by: By,
    pub in_run: bool,
    rev: u64,
    player_edits: bool,
}

impl Coach {
    /// Whether the player changed the code in the last moment.
    pub fn player_editing(&self, now: f64) -> bool {
        now - self.player_at < HANDS_OFF
    }
    /// Whether the player used the studio in the last moment.
    pub fn player_busy(&self, now: f64) -> bool {
        now - self.input_at < HANDS_OFF
    }
    /// The game's own agent's turn is over (`ok`: it ended well): at the
    /// next step its edits nobody saved are saved, or a line says why not.
    pub fn turn_ended(&mut self, ok: bool) {
        self.turn_over = Some(ok);
    }
}

/// The editor's file and revision before an input, for `after_input`.
pub struct Before {
    revision: u64,
    path: Option<PathBuf>,
}

/// Before the player's input reaches the studio: it is noted, and a key, a
/// typed character or a press puts the rest of an edit being typed in at
/// once, so whatever the input does (type at the player's own caret, save,
/// run, clear, open another file) finds the edit whole.
pub fn before_input(st: &mut Studio, input: &Input) -> Before {
    let why = match input {
        Input::Key { down: true, .. } | Input::Text(_) => Some("the player typed"),
        Input::Button { down: true, .. } => Some("the player clicked"),
        _ => None,
    };
    if let Some(why) = why {
        finish_typing(st, why);
    }
    if why.is_some() || matches!(input, Input::Wheel(_)) {
        st.s.ui.coach.input_at = st.s.now;
    }
    Before {
        revision: st.s.editor.revision,
        path: st.s.editor.path.clone(),
    }
}

/// After it: when it changed the code of the file that was open (not by
/// opening another), the player is editing.
pub fn after_input(st: &mut Studio, before: Before) {
    let ed = &st.s.editor;
    if ed.revision != before.revision && ed.path == before.path {
        st.s.ui.coach.player_at = st.s.now;
    }
}

// ----------------------------------------------------------------- which file

/// A file a request is about, as `target` finds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub path: PathBuf,
    /// Its name for the agent and the player, as the editor titles it:
    /// "hello.lua", "kit/lib/util.lua", "scratch.lua".
    pub file: String,
    /// Whether it is there, on disk or open in the editor (a file asked for
    /// with create may not be yet).
    pub exists: bool,
}

fn text<'a>(params: &'a Json, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn flag(params: &Json, key: &str) -> Option<bool> {
    params.get(key).and_then(Json::as_bool)
}

/// A whole number parameter, 1 or more.
fn line_param(params: &Json, key: &str) -> Result<Option<usize>, String> {
    match params.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(v) => match v.as_u64() {
            Some(n) if n >= 1 => Ok(Some(n as usize)),
            _ => Err(format!("{key} is a line number, counted from 1")),
        },
    }
}

/// A file of a module as the agent named it, as its path in the module:
/// "lib" and "lib.lua" are "lib.lua", backslashes are slashes.
fn lua_name(given: &str) -> Result<String, String> {
    let name = given.trim().replace('\\', "/");
    if name.ends_with(".lua") {
        return Ok(name);
    }
    if name.rsplit('/').next().is_some_and(|last| last.contains('.')) {
        return Err(format!(
            "{name} isn't a .lua file: give a file by its path in the mod, such as lib/util.lua"
        ));
    }
    Ok(format!("{name}.lua"))
}

/// The file a request means: the scratch file (`scratch`), a mod's file
/// (`id`, and `file` for a module's other files), a file of the mod the
/// editor has open (`file` alone), or the file open (neither). The paths
/// follow the studio's own rules for mods and modules' files, so none is
/// outside the mods folder. With `create`, a file that isn't there yet may
/// be named: a new script, a new module's main.lua, a new file of a module.
pub fn target(st: &Studio, params: &Json, create: bool) -> Result<Target, String> {
    if flag(params, "scratch") == Some(true) {
        if text(params, "id").is_some() || text(params, "file").is_some() {
            return Err("scratch means the scratch file: give it without id or file".into());
        }
        return Ok(Target {
            path: st.scratch_path(),
            file: "scratch.lua".into(),
            exists: true,
        });
    }
    let ed = &st.s.editor;
    let id = match (text(params, "id"), text(params, "file"), ed.path.clone()) {
        (Some(id), ..) => id.to_owned(),
        (None, _, None) => return Err("the editor has no file open: give id, or scratch".into()),
        (None, None, Some(path)) => {
            return Ok(Target {
                path,
                file: ed.title.clone(),
                exists: true,
            });
        }
        (None, Some(_), Some(path)) => match st.mod_of_path(&path) {
            Some(i) => st.s.mods[i].id.clone(),
            None => {
                return Err(format!(
                    "the editor has {}, which is no mod's, open: give id with file",
                    ed.title
                ));
            }
        },
    };
    if mod_id(&id) != id {
        return Err(format!(
            "'{id}' is not a mod id: lowercase letters, digits, _ and -, as its file or folder is named"
        ));
    }
    mod_file(st, &id, text(params, "file"), create)
}

/// The file `file` (None: the main one) of mod `id`, which may not exist yet.
fn mod_file(st: &Studio, id: &str, file: Option<&str>, create: bool) -> Result<Target, String> {
    let is_open = |p: &Path| st.s.editor.path.as_deref() == Some(p);
    let found = |path: PathBuf, exists: bool| Target {
        file: st.file_title(&path),
        exists: exists || is_open(&path),
        path,
    };
    let Some(m) = st.s.mods.iter().find(|m| m.id == id) else {
        // A new mod: a script, or a module's main.lua. One the editor
        // already has open (opened with create, not saved yet) is found.
        let mods = st.s.mods_dir();
        let module = match file.map(lua_name).transpose()? {
            None => false,
            Some(f) if f == format!("{id}.lua") => false,
            Some(f) if f == "main.lua" => true,
            Some(_) => {
                return Err(format!(
                    "no mod '{id}' yet: a new one starts as {id}.lua, or as a module's main.lua"
                ));
            }
        };
        let (path, file) = if module {
            (mods.join(id).join("main.lua"), format!("{id}/main.lua"))
        } else {
            (mods.join(format!("{id}.lua")), format!("{id}.lua"))
        };
        if is_open(&path) {
            return Ok(Target {
                path,
                file,
                exists: true,
            });
        }
        if !create {
            let ids: Vec<&str> = st.s.mods.iter().map(|m| m.id.as_str()).collect();
            let ids = if ids.is_empty() { "none".into() } else { ids.join(", ") };
            return Err(format!("no script mod '{id}' (the mods: {ids}); create: true makes it"));
        }
        plan_mod(&mods, id, module)?;
        return Ok(Target {
            path,
            file,
            exists: false,
        });
    };
    let Some(folder) = &m.folder else {
        return match file.map(lua_name).transpose()? {
            None => Ok(found(m.main.clone(), true)),
            Some(f) if f == format!("{id}.lua") || f == "main.lua" => Ok(found(m.main.clone(), true)),
            Some(f) => Err(format!(
                "{id} is a script, one file ({id}.lua), with no {f}: the script list's Make it a module gives it \
                 more files"
            )),
        };
    };
    let rel = lua_name(file.unwrap_or("main.lua"))?;
    let files = m.files();
    if files.contains(&rel) {
        return Ok(found(path_in(folder, &rel), true));
    }
    // Not one the last scan saw: a name the studio would give a file.
    let rel = file_name(&rel).map_err(|e| format!("{rel}: {e}"))?;
    let path = path_in(folder, &rel);
    if path.is_file() || is_open(&path) {
        return Ok(found(path, true));
    }
    if create {
        return Ok(found(path, false));
    }
    Err(format!(
        "{id} has no file {rel} (its files: {}); create: true makes it",
        files.join(", ")
    ))
}

// ----------------------------------------------------------------- reading

/// A file's lines as the editor would have them.
fn disk_lines(path: &Path, file: &str) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {file}: {e}"))?;
    Ok(Editor::normal(&text).split('\n').map(str::to_owned).collect())
}

/// How many lines a file has, as an agent counts them: the empty line after
/// a last line break isn't one.
pub fn line_count(lines: &[String]) -> usize {
    match lines.last() {
        Some(l) if l.is_empty() => lines.len() - 1,
        _ => lines.len(),
    }
}

/// A place in the buffer as an agent counts it: line and column from 1, the
/// column in characters.
fn place(ed: &Editor, (row, col): (usize, usize)) -> Json {
    let line = ed.lines.get(row).map_or("", String::as_str);
    let chars = line.get(..col.min(line.len())).map_or(0, |s| s.chars().count());
    json!({ "line": row + 1, "column": chars + 1 })
}

/// `s` cut to `max` characters, with "…" if it was longer.
fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

/// "lines 3-8", "line 3".
pub(crate) fn lines_text(first: usize, last: usize) -> String {
    if last > first {
        format!("lines {first}-{last}")
    } else {
        format!("line {first}")
    }
}

/// The link's `editor`: a file's text as the player's editor has it, unsaved
/// edits included (a file it hasn't open, as on disk), a page of lines at a
/// time (`from`, `to`, counted from 1), with the editor's state: whether the
/// file is open and has unsaved edits, the player's cursor and selection.
pub fn read(st: &mut Studio, params: &Json) -> Result<Json, String> {
    let t = target(st, params, false)?;
    let ed = &st.s.editor;
    let open = ed.path.as_deref() == Some(t.path.as_path());
    let lines = if open {
        final_lines(st)
    } else {
        disk_lines(&t.path, &t.file)?
    };
    let count = line_count(&lines);
    let from = line_param(params, "from")?.unwrap_or(1);
    let last = from + READ_PAGE - 1;
    let to = line_param(params, "to")?.unwrap_or(last).min(last).min(count);
    let page: Vec<&String> = if from <= to {
        lines[from - 1..to].iter().collect()
    } else {
        Vec::new()
    };
    let did = match (count, page.is_empty()) {
        (0, _) => format!("Read {}: it is empty", t.file),
        (_, true) => format!("{} has {count} lines, none from line {from}", t.file),
        _ if from == 1 && to == count => format!("Read {} ({count} lines)", t.file),
        _ => format!("Read {}, {} of {count}", t.file, lines_text(from, to)),
    };
    let mut out = json!({
        "did": did,
        "file": t.file,
        "path": t.path,
        "open": open,
        "dirty": open && ed.dirty,
        "line_count": count,
        "from": from,
        "to": if page.is_empty() { from - 1 } else { to },
        "more": to < count,
        "lines": page,
    });
    if open {
        out["cursor"] = place(ed, (ed.row, ed.col));
        out["selection"] = match (ed.selection(), ed.selected_text()) {
            (Some((a, b)), Some(text)) => json!({
                "from": place(ed, a),
                "to": place(ed, b),
                "text": cut(&text, SELECTION_CHARS),
            }),
            _ => Json::Null,
        };
        out["typing"] = typing_state(st);
    }
    Ok(out)
}

/// The edit being typed in, for the state the link gives: its id and who makes it.
fn typing_state(st: &Studio) -> Json {
    st.s.ui
        .coach
        .typing
        .as_ref()
        .map_or(Json::Null, |t| json!({ "id": t.id, "by": t.by.name, "file": t.file }))
}

/// The buffer as it will be once the edit being typed in is all in: what
/// an agent reads, and what its next edit is matched against.
fn final_lines(st: &Studio) -> Vec<String> {
    let ed = &st.s.editor;
    let Some(t) = st.s.ui.coach.typing.as_ref().filter(|t| t.rev == ed.revision) else {
        return ed.lines.clone();
    };
    let (a, b, rest) = match t.phase {
        Phase::ShowOld => (ed.offset_of(t.from), ed.offset_of(t.old_to), &t.text[..]),
        Phase::Typing => {
            let at = ed.offset_of(t.at);
            (at, at, &t.text[t.shown..])
        }
    };
    let mut text = ed.text();
    text.replace_range(a..b, rest);
    text.split('\n').map(str::to_owned).collect()
}

/// The editor for `status`: the file open, whether it has unsaved edits,
/// its lines and mod, an edit being typed in, an agent's edits not saved.
pub fn summary(st: &Studio) -> Json {
    let ed = &st.s.editor;
    let module = ed
        .path
        .as_deref()
        .and_then(|p| st.mod_of_path(p))
        .map(|i| st.s.mods[i].id.clone());
    let unsaved =
        st.s.ui
            .coach
            .unsaved
            .as_ref()
            .filter(|u| ed.path.as_ref() == Some(&u.path) && ed.dirty);
    json!({
        "file": ed.path.as_ref().map(|_| ed.title.clone()),
        "path": ed.path,
        "mod": module,
        "dirty": ed.dirty,
        "line_count": line_count(&ed.lines),
        "typing": typing_state(st),
        "unsaved": unsaved.map(|u| json!({ "file": u.file, "by": u.by.name, "conn": u.by.conn })),
    })
}

// ----------------------------------------------------------------- opening

/// The answer to a request that has to wait for the player: it is done
/// once they pause (the MCP server asks again).
fn busy(why: impl Into<String>) -> Json {
    json!({ "ok": false, "busy": true, "error": why.into() })
}

/// Why another file can't open in the editor now, if it can't: the player
/// is using the studio, or has a prompt or a menu open in it.
fn switch_waits(st: &Studio) -> Option<Json> {
    let ui = &st.s.ui;
    if ui.open && (ui.naming.is_some() || ui.menu.is_some()) {
        return Some(busy("the player has a prompt or a menu open in the studio"));
    }
    ui.coach
        .player_busy(st.s.now)
        .then(|| busy("the player is using the studio"))
}

/// Shows line `n` (from 1) a third of the way down the editor's view, with
/// the cursor on it when `cursor`.
fn show_line(st: &mut Studio, n: usize, cursor: bool) {
    let page = if st.s.ui.page > 0 { st.s.ui.page } else { 30 };
    let ed = &mut st.s.editor;
    let row = (n - 1).min(ed.lines.len() - 1);
    if cursor {
        ed.click(row, 0, false);
    }
    ed.scroll = row.saturating_sub(page / 3);
    ed.followed = Some((ed.row, ed.col, ed.revision));
}

/// The link's `open`: a file in the player's editor, as the script list
/// opens one, the Editor tab brought forward. Another file opens only when
/// the player isn't using the studio and the file open has no unsaved
/// edits (nothing of theirs is saved or put aside for an agent). `line`
/// shows a line; `create` opens a file that isn't there yet, empty, to be
/// typed into (it is written when saved).
pub fn open(st: &mut Studio, by: &By, params: &Json) -> Result<Json, String> {
    let create = flag(params, "create") == Some(true);
    let t = target(st, params, create)?;
    let line = line_param(params, "line")?;
    let now = st.s.now;
    let already = st.s.editor.path.as_deref() == Some(t.path.as_path());
    if already {
        if !st.s.ui.coach.player_busy(now) {
            st.s.ui.tab = Tab::Editor;
        }
        // The view moves to the line; the player's cursor stays theirs.
        if let Some(n) = line
            && !st.s.ui.coach.player_editing(now)
        {
            show_line(st, n, false);
        }
    } else {
        if let Some(wait) = switch_waits(st) {
            return Ok(wait);
        }
        let ed = &st.s.editor;
        if ed.dirty {
            return Err(if agents_only(st) {
                format!(
                    "your edits to {} aren't saved: save_script saves them, then open_script opens another file",
                    ed.title
                )
            } else {
                format!(
                    "the player has unsaved edits in {}: open_script opens another file once they are saved \
                     (Ctrl+S); edit_script can change {} without opening it",
                    ed.title, t.file
                )
            });
        }
        st.open_file(&t.path);
        // A new mod isn't one yet: its title is what it will be.
        st.s.editor.title = t.file.clone();
        st.s.ui.tab = Tab::Editor;
        if let Some(n) = line {
            show_line(st, n, true);
        }
    }
    let ed = &st.s.editor;
    let did = match (already, t.exists, line) {
        (_, false, _) => format!("Opened {}, a new file: it is written when saved", t.file),
        (true, _, None) => format!("{} is open", t.file),
        (true, _, Some(n)) => format!("Showed line {n} of {}", t.file),
        (false, _, None) => format!("Opened {}", t.file),
        (false, _, Some(n)) => format!("Opened {} at line {n}", t.file),
    };
    if !already || line.is_some() {
        let row = line.map_or(0, |n| (n - 1).min(ed.lines.len() - 1));
        let jump = Jump {
            path: t.path.clone(),
            file: t.file.clone(),
            first: row,
            last: row,
        };
        let text = if t.exists {
            did.clone()
        } else {
            format!("Opened {}, a new file", t.file)
        };
        let in_run = in_run(st, by);
        say(st, by, in_run, text, Some(jump));
    }
    let ed = &st.s.editor;
    Ok(json!({
        "ok": true,
        "did": did,
        "file": t.file,
        "path": t.path,
        "line_count": line_count(&ed.lines),
        "dirty": ed.dirty,
        "created": !t.exists,
    }))
}

// ----------------------------------------------------------------- live edits

/// A text parameter as given, spaces and all.
fn raw<'a>(params: &'a Json, key: &str) -> Option<&'a str> {
    params.get(key).and_then(Json::as_str)
}

/// "4", "4 and 19", "4, 19 and 33".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The line (from 1) byte `at` of `text` is on.
fn line_at(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

/// Where row `row` of `lines` starts in their text (joined by line breaks).
fn row_start(lines: &[String], row: usize) -> usize {
    lines[..row].iter().map(|l| l.len() + 1).sum()
}

/// What an edit asks of a file's text: bytes `a..b` become `new`, whose
/// lines are then `span` (from 1); `removed` is the whole lines it takes
/// out, if that is what it does.
struct Change {
    a: usize,
    b: usize,
    new: String,
    span: (usize, usize),
    removed: Option<(usize, usize)>,
}

/// The lines (from 1) text `new` covers once it starts on line `first`: a
/// line break at its end starts no line of its own.
fn span_of(first: usize, new: &str) -> (usize, usize) {
    let body = new.strip_suffix('\n').unwrap_or(new);
    (first, first + body.matches('\n').count())
}

/// The change an edit's parameters ask of `lines` (the file `file` as the
/// editor has it): old_text and new_text (old_text there exactly once),
/// start_line (end_line) and new_text (whole lines, from 1), or text (the
/// whole file).
fn change_of(lines: &[String], file: &str, params: &Json) -> Result<Change, String> {
    let given = |k: &str| params.get(k).is_some_and(|v| !v.is_null());
    if [given("old_text"), given("start_line"), given("text")]
        .iter()
        .filter(|g| **g)
        .count()
        != 1
    {
        return Err(
            "give one of: old_text with new_text (the code to replace, as it is now), start_line with \
                    new_text (whole lines, end_line too), or text (the whole file)"
                .into(),
        );
    }
    let new_text = || {
        raw(params, "new_text")
            .map(Editor::normal)
            .ok_or_else(|| "new_text is missing (\"\" takes the text out)".to_owned())
    };
    let text = lines.join("\n");
    let change = if given("old_text") {
        let old = Editor::normal(raw(params, "old_text").ok_or("old_text is the code to replace, as text")?);
        if old.is_empty() {
            return Err("old_text is empty: give the code to replace, or start_line to put lines in".into());
        }
        let found: Vec<usize> = text.match_indices(&old).map(|(i, _)| i).take(10).collect();
        match found.as_slice() {
            [] => {
                return Err(format!(
                    "old_text isn't in {file} as the editor has it (the player may have changed it): read_script \
                     shows it now"
                ));
            }
            [a] => {
                let new = new_text()?;
                Change {
                    a: *a,
                    b: a + old.len(),
                    span: span_of(line_at(&text, *a), &new),
                    new,
                    removed: None,
                }
            }
            many => {
                let at: Vec<String> = many.iter().map(|&i| line_at(&text, i).to_string()).collect();
                return Err(format!(
                    "old_text is at lines {}: give more of the code around it, so that it is there once",
                    and_list(&at)
                ));
            }
        }
    } else if given("text") {
        let new = Editor::normal(raw(params, "text").ok_or("text is the whole file, as text")?);
        Change {
            a: 0,
            b: text.len(),
            span: span_of(1, &new),
            new,
            removed: None,
        }
    } else {
        let count = line_count(lines);
        let start = line_param(params, "start_line")?.unwrap_or(1);
        let end = match params.get("end_line") {
            None | Some(Json::Null) if start == count + 1 => count,
            None | Some(Json::Null) => start,
            Some(v) => v.as_u64().ok_or("end_line is a line number, counted from 1")? as usize,
        };
        if start > count + 1 {
            return Err(format!(
                "start_line {start} is past the end: {file} has {count} lines (start_line {} adds lines at the end)",
                count + 1
            ));
        }
        if end + 1 < start || end > count {
            return Err(format!(
                "end_line {end} isn't from start_line - 1 (lines put in before start_line) to {count}, the last line \
                 of {file}"
            ));
        }
        // Whole lines: a line break at the end of new_text is the last line's.
        let mut new = new_text()?;
        if new.ends_with('\n') {
            new.pop();
        }
        let span = span_of(start, &new);
        if end + 1 == start {
            // Lines put in before start_line, or after the last.
            let (a, new) = if start <= count {
                (row_start(lines, start - 1), format!("{new}\n"))
            } else if count == 0 {
                (0, new)
            } else {
                (row_start(lines, count - 1) + lines[count - 1].len(), format!("\n{new}"))
            };
            Change {
                a,
                b: a,
                new,
                span,
                removed: None,
            }
        } else {
            let a = row_start(lines, start - 1);
            let b = row_start(lines, end - 1) + lines[end - 1].len();
            if !new.is_empty() {
                Change {
                    a,
                    b,
                    new,
                    span,
                    removed: None,
                }
            } else {
                // Taken out with a line break: the one after them, else the one before.
                let (a, b) = if b < text.len() {
                    (a, b + 1)
                } else {
                    (a.saturating_sub(1), b)
                };
                Change {
                    a,
                    b,
                    new,
                    span,
                    removed: Some((start, end)),
                }
            }
        }
    };
    if change.new.len() > MAX_EDIT {
        return Err(format!(
            "that is {} KiB: an edit puts in at most {} KiB (write_mod writes bigger files)",
            change.new.len() >> 10,
            MAX_EDIT >> 10
        ));
    }
    Ok(change)
}

/// How many bytes `old` and `new` share at their start, and then at their
/// end: only what is between them changes.
fn shared_ends(old: &str, new: &str) -> (usize, usize) {
    let pre: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let suf: usize = old[pre..]
        .chars()
        .rev()
        .zip(new[pre..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    (pre, suf)
}

/// The bytes of `text` in once `due` of its non-space characters are: the
/// spaces and line breaks after the last of them too, so a line's end and
/// the next line's indent come at once.
fn typed_to(text: &str, due: usize) -> usize {
    let mut seen = 0;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            continue;
        }
        if seen == due {
            return i;
        }
        seen += 1;
    }
    text.len()
}

/// Whether `by` is the game's own agent, in the run it is on now.
fn in_run(st: &mut Studio, by: &By) -> bool {
    super::hstudio_ui::agent_busy(st) && super::hstudio_ui::agent_name(st).is_some_and(|n| n == by.name)
}

/// A line in the Agent panel for what an agent did in the editor, which
/// jumps to the code; named after the agent unless it is the game's own,
/// in its run (`in_run`).
fn say(st: &mut Studio, by: &By, in_run: bool, text: impl Into<String>, jump: Option<Jump>) {
    let text = text.into();
    let text = if in_run { text } else { format!("{}: {text}", by.name) };
    let now = st.s.now;
    st.s.ui.agent.say_jump(AgentLineKind::Action, text, jump, now);
}

/// Whether the unsaved edits in the editor's file are an agent's alone.
fn agents_only(st: &Studio) -> bool {
    let ed = &st.s.editor;
    st.s.ui
        .coach
        .unsaved
        .as_ref()
        .is_some_and(|u| ed.path.as_ref() == Some(&u.path) && u.rev == ed.revision && !u.player_edits)
}

/// The live check's findings, for a result.
fn problems_json(problems: &[super::hstudio_assist::Problem]) -> Json {
    Json::Array(
        problems
            .iter()
            .map(|p| json!({ "line": p.line, "message": p.message, "error": p.error }))
            .collect(),
    )
}

/// The other files of the module `path` is a file of, as on disk, for the live check.
fn siblings_of(st: &Studio, path: &Path) -> Option<super::hstudio_assist::Siblings> {
    let m = &st.s.mods[st.mod_of_path(path)?];
    let folder = m.folder.as_ref()?;
    let rel = super::hstudio_modfiles::rel_in(folder, path)?;
    let others = m
        .files()
        .into_iter()
        .filter(|f| *f != rel)
        .map(|f| {
            let text = std::fs::read_to_string(path_in(folder, &f)).unwrap_or_default();
            (f, text.lines().map(str::to_owned).collect())
        })
        .collect();
    Some(super::hstudio_assist::Siblings { file: rel, others })
}

/// The state of the mod a file is of (null for the scratch file), with the
/// file and line it failed at, if it did.
fn mod_state(st: &Studio, path: &Path) -> Json {
    let Some(i) = st.mod_of_path(path) else {
        return Json::Null;
    };
    let m = &st.s.mods[i];
    let (state, error) = match &m.state {
        ModState::Off => ("off", None),
        ModState::Running => ("running", None),
        ModState::Failed(e, _) => ("failed", Some(e.clone())),
    };
    let place = st.failure_place(i);
    json!({
        "id": m.id,
        "state": state,
        "error": error,
        "file": place.as_ref().map(|p| p.0.clone()),
        "line": place.map(|p| p.1),
    })
}

/// "Saved greeter.lua: running", "Saved greeter.lua: it failed at line 3".
fn saved_text(file: &str, m: &Json) -> String {
    match m["state"].as_str() {
        Some("running") => format!("Saved {file}: running"),
        Some("failed") => match (m["file"].as_str(), m["line"].as_u64()) {
            (Some(f), Some(line)) if file.ends_with(f) => format!("Saved {file}: it failed at line {line}"),
            (Some(f), Some(line)) => format!("Saved {file}: it failed in {f} at line {line}"),
            _ => format!("Saved {file}: it failed"),
        },
        Some("off") => format!("Saved {file} (its mod is off)"),
        _ => format!("Saved {file}"),
    }
}

/// What an edit did, in a line: "Typed lines 12-19 into timer.lua".
fn did_text(removed: Option<(usize, usize)>, lines: (usize, usize), animated: bool, file: &str) -> String {
    match removed {
        Some((a, b)) => format!("Removed {} from {file}", lines_text(a, b)),
        None if animated => format!("Typed {} into {file}", lines_text(lines.0, lines.1)),
        None => format!("Changed {} in {file}", lines_text(lines.0, lines.1)),
    }
}

/// The link's `edit`: a change to a script. In the file the editor has
/// open, it is typed in as the player watches (the text it replaces tinted
/// a moment, then the new text coming in at the agent's own caret), or put
/// in at once: with Motion off or `typing: false`, while nobody sees the
/// editor, and while the player is typing there (their view and cursor
/// stay). Either way it is one undo step, and stays unsaved unless `save`
/// is given (it is by default while the studio is closed). A file the
/// editor doesn't have open is changed on disk and its mod reloads; the
/// player's file and tab stay as they are.
pub fn edit(st: &mut Studio, game: &mut dyn GameApi, by: &By, params: &Json) -> Result<Json, String> {
    finish_typing(st, "another edit came");
    let t = target(st, params, false)?;
    let open = st.s.editor.path.as_deref() == Some(t.path.as_path());
    let lines = if open {
        st.s.editor.lines.clone()
    } else {
        disk_lines(&t.path, &t.file)?
    };
    let change = change_of(&lines, &t.file, params)?;
    let in_run = in_run(st, by);
    if !open {
        return edit_on_disk(st, game, (by, in_run), &t, &lines, change);
    }
    let now = st.s.now;
    let ui = &st.s.ui;
    let seen = ui.open && ui.tab == Tab::Editor && !ui.collapsed;
    let hands_off = ui.coach.player_editing(now);
    let animated = seen && !hands_off && ui.look.motion && flag(params, "typing") != Some(false);
    let save = flag(params, "save").unwrap_or(!ui.open);
    let page = ui.page.max(1);
    let player_edits = match &ui.coach.unsaved {
        Some(u) if st.s.editor.path.as_ref() == Some(&u.path) && u.rev == st.s.editor.revision => u.player_edits,
        _ => st.s.editor.dirty,
    };
    let ed = &mut st.s.editor;
    let text = ed.text();
    let span = change.span;
    let (pre, suf) = shared_ends(&text[change.a..change.b], &change.new);
    let (mut a, mut b) = (change.a + pre, change.b - suf);
    let mut ins = change.new[pre..change.new.len() - suf].to_owned();
    if a == b && ins.is_empty() {
        return Ok(json!({
            "ok": true,
            "changed": false,
            "did": format!("{} has that already, at {}", t.file, lines_text(span.0, span.1)),
            "file": t.file,
            "lines": [span.0, span.1],
            "line_count": line_count(&ed.lines),
            "dirty": ed.dirty,
        }));
    }
    // A line break put in at a line's end goes in at the next line's start
    // instead (the same text), so a cursor at that end stays where it is.
    if a == b && ins.starts_with('\n') && text[b..].starts_with('\n') {
        (a, b) = (a + 1, b + 1);
        ins = format!("{}\n", &ins[1..]);
    }
    let (from, old_to) = (ed.pos_at(a), ed.pos_at(b));
    let rows = ed.lines.len();
    ed.agent_begin();
    let id = st.s.ui.coach.next_edit;
    st.s.ui.coach.next_edit += 1;
    let chars = ins.chars().filter(|c| !c.is_whitespace()).count();
    let mut typing = Typing {
        id,
        by: by.clone(),
        in_run,
        path: t.path.clone(),
        file: t.file.clone(),
        phase: if a < b { Phase::ShowOld } else { Phase::Typing },
        since: now,
        secs: (chars as f64 / CHARS_PER_SECOND).clamp(TYPE_MIN, TYPE_MAX),
        show_old: SHOW_OLD,
        from,
        old_to,
        text: ins,
        shown: 0,
        chars,
        at: from,
        rev: st.s.editor.revision,
        free: false,
        scroll: st.s.editor.scroll,
        parked: (false, false),
        animated,
        save,
        lines: span,
        removed: change.removed,
        player_edits,
    };
    if seen && !hands_off {
        // The edit's first line in view, a third of the way down.
        let ed = &mut st.s.editor;
        if from.0 < ed.scroll || from.0 >= ed.scroll + page {
            ed.scroll = from.0.saturating_sub(page / 3);
            typing.show_old = SHOW_OLD_SCROLLED;
        }
        typing.scroll = ed.scroll;
        st.s.ui.suggest = None;
    }
    if animated {
        st.s.ui.coach.typing = Some(typing);
        return Ok(json!({ "ok": true, "typing": id, "file": t.file, "did": format!("Typing into {}", t.file) }));
    }
    complete(st, &mut typing);
    if !seen || hands_off {
        // What the player sees stays put: lines put in above the view move it with them.
        let ed = &mut st.s.editor;
        if from.0 < ed.scroll {
            let moved = ed.lines.len() as isize - rows as isize;
            ed.scroll = (ed.scroll as isize + moved).clamp(0, ed.lines.len() as isize - 1) as usize;
        }
    }
    let mut result = land(st, &typing, None);
    if save {
        save_into(st, game, &typing, &mut result);
    }
    keep_result(st, id, &result);
    Ok(result)
}

/// An edit to a file the editor doesn't have open: written to disk at
/// once, and its mod reloads; a note over the code says so, and shows it.
fn edit_on_disk(
    st: &mut Studio,
    game: &mut dyn GameApi,
    (by, in_run): (&By, bool),
    t: &Target,
    lines: &[String],
    change: Change,
) -> Result<Json, String> {
    let mut text = lines.join("\n");
    let span = change.span;
    text.replace_range(change.a..change.b, &change.new);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    std::fs::write(&t.path, &text).map_err(|e| format!("could not write {}: {e}", t.file))?;
    st.scan(game);
    let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let siblings = siblings_of(st, &t.path);
    let problems = super::hstudio_assist::check(&lines, &t.file, siblings.as_ref());
    let what = did_text(change.removed, span, false, &t.file);
    let did = format!("{what} (on disk: the editor has {} open)", st.s.editor.title);
    let jump = Jump {
        path: t.path.clone(),
        file: t.file.clone(),
        first: span.0 - 1,
        last: span.1 - 1,
    };
    say(st, by, in_run, what, Some(jump));
    if st.s.ui.open {
        st.s.ui.coach.notice = Some(Notice {
            text: format!("{} changed {}", by.name, t.file),
            seq: st.s.ui.agent.seq,
            at: st.s.now,
        });
    }
    let mut out = json!({
        "ok": true,
        "did": did,
        "file": t.file,
        "lines": [span.0, span.1],
        "line_count": line_count(&lines),
        "animated": false,
        "written": t.path,
        "saved": true,
        "mod": mod_state(st, &t.path),
        "problems": problems_json(&problems),
    });
    if let Some((a, b)) = change.removed {
        out["lines"] = json!([]);
        out["removed"] = json!([a, b]);
    }
    Ok(out)
}

/// Each frame, the studio open or not: the edit being typed in comes on,
/// and what was kept for a while goes.
pub fn step(st: &mut Studio, game: &mut dyn GameApi) {
    let now = st.s.now;
    let c = &mut st.s.ui.coach;
    c.results.retain(|(_, at, _)| now - at < RESULTS_KEPT);
    if c.flash.as_ref().is_some_and(|f| now - f.at >= FLASH) {
        c.flash = None;
    }
    if c.notice.as_ref().is_some_and(|n| now - n.at >= NOTICE_SECS) {
        c.notice = None;
    }
    if let Some(ok) = c.turn_over.take() {
        end_turn(st, game, ok);
    }
    let Some(mut t) = st.s.ui.coach.typing.take() else {
        return;
    };
    if !still_there(st, &mut t) {
        cut_off(st, t);
        return;
    }
    advance(st, &mut t);
    if t.phase == Phase::Typing && t.shown == t.text.len() {
        let mut result = land(st, &t, None);
        if t.save {
            save_into(st, game, &t, &mut result);
        }
        keep_result(st, t.id, &result);
        return;
    }
    follow(st, &mut t);
    st.s.ui.coach.typing = Some(t);
}

/// Puts the rest of an edit being typed in at once, because something
/// came that needs the buffer whole (`why`): the player typing or
/// clicking, a save, a run, another file opening, another request. Cut
/// short, it isn't saved, though it was asked to be.
pub fn finish_typing(st: &mut Studio, why: &str) {
    let Some(mut t) = st.s.ui.coach.typing.take() else {
        return;
    };
    if !still_there(st, &mut t) {
        cut_off(st, t);
        return;
    }
    complete(st, &mut t);
    let result = land(st, &t, Some(why));
    keep_result(st, t.id, &result);
}

/// Whether the buffer is still the one the edit is being typed into: its
/// revision is the edit's own. (A file moved keeps its text: a scan taking
/// the editor along to a module's main.lua takes the edit too.)
fn still_there(st: &Studio, t: &mut Typing) -> bool {
    let ed = &st.s.editor;
    let Some(path) = &ed.path else {
        return false;
    };
    if ed.revision != t.rev {
        return false;
    }
    if *path != t.path {
        t.path = path.clone();
        t.file = ed.title.clone();
    }
    true
}

/// An edit whose buffer changed under it (another file opened, the file
/// read again, the text cleared): the rest isn't typed in, and its result
/// and the Agent panel say so.
fn cut_off(st: &mut Studio, t: Typing) {
    let result = json!({
        "ok": false,
        "error": format!(
            "the editor's text changed before the edit to {} was all typed in (another file opened, or the file \
             was read again): read_script shows it now; nothing was saved",
            t.file
        ),
        "file": t.file,
    });
    let now = st.s.now;
    st.s.ui.agent.say(
        AgentLineKind::Error,
        format!(
            "{}'s edit to {} was cut off: the editor's text changed under it",
            t.by.name, t.file
        ),
        now,
    );
    keep_result(st, t.id, &result);
}

/// The edit comes on a frame: the old text goes once it has shown, then the
/// new comes in at its pace.
fn advance(st: &mut Studio, t: &mut Typing) {
    let now = st.s.now;
    let ed = &mut st.s.editor;
    match t.phase {
        Phase::ShowOld => {
            if now - t.since < t.show_old {
                return;
            }
            t.at = ed.splice(t.from, t.old_to, "");
            // A cursor that was in what went stays at the new text's start.
            t.parked = ((ed.row, ed.col) == t.from, ed.anchor == Some(t.from));
            t.phase = Phase::Typing;
            t.since = now;
        }
        Phase::Typing => {
            let k = ((now - t.since) / t.secs).clamp(0.0, 1.0);
            let due = (t.chars as f64 * k).ceil() as usize;
            let to = typed_to(&t.text, due);
            if to <= t.shown {
                return;
            }
            t.at = ed.splice(t.at, t.at, &t.text[t.shown..to]);
            t.shown = to;
            unpark(ed, t);
        }
    }
    t.rev = ed.revision;
    // The live check waits for the typing to pause.
    ed.edited_at = now;
}

/// The player's cursor (and the selection's other end) back at the new
/// text's start, when the text it was in went.
fn unpark(ed: &mut Editor, t: &Typing) {
    if t.parked.0 {
        (ed.row, ed.col) = t.from;
    }
    if t.parked.1 {
        ed.anchor = Some(t.from);
    }
    ed.followed = Some((ed.row, ed.col, ed.revision));
}

/// The rest of an edit in at once.
fn complete(st: &mut Studio, t: &mut Typing) {
    let ed = &mut st.s.editor;
    if t.phase == Phase::ShowOld {
        t.at = ed.splice(t.from, t.old_to, &t.text);
        t.phase = Phase::Typing;
    } else if t.shown < t.text.len() {
        t.at = ed.splice(t.at, t.at, &t.text[t.shown..]);
        unpark(ed, t);
    }
    t.shown = t.text.len();
    t.rev = ed.revision;
}

/// The agent's caret while the view follows it (an edit coming in, the
/// player not scrolled away): the columns the editor shows follow it too.
pub fn followed_caret(c: &Coach, ed: &Editor) -> Option<(usize, usize)> {
    let t = c.typing.as_ref()?;
    let line = ed.lines.get(t.at.0)?;
    let ok = t.phase == Phase::Typing
        && !t.free
        && t.rev == ed.revision
        && ed.path.as_ref() == Some(&t.path)
        && line.is_char_boundary(t.at.1);
    ok.then_some(t.at)
}

/// Keeps the agent's caret in view, unless the player scrolled away.
fn follow(st: &mut Studio, t: &mut Typing) {
    let page = st.s.ui.page.max(1);
    let ed = &mut st.s.editor;
    if t.free || ed.scroll != t.scroll {
        t.free = true;
        return;
    }
    let row = t.at.0;
    if row < ed.scroll || row >= ed.scroll + page {
        ed.scroll = row.saturating_sub(page / 3).min(ed.lines.len() - 1);
    }
    t.scroll = ed.scroll;
}

/// An edit all in: the lines it put in lit a moment, its line in the Agent
/// panel, and its result (`cut_short`: why the rest came at once).
fn land(st: &mut Studio, t: &Typing, cut_short: Option<&str>) -> Json {
    let now = st.s.now;
    st.s.ui.coach.flash = Some(Flash {
        path: t.path.clone(),
        first: t.from.0,
        last: t.at.0,
        at: now,
    });
    st.s.ui.coach.unsaved = Some(Unsaved {
        path: t.path.clone(),
        file: t.file.clone(),
        by: t.by.clone(),
        in_run: t.in_run,
        rev: st.s.editor.revision,
        player_edits: t.player_edits,
    });
    let siblings = super::hstudio_ui::editor_siblings(st);
    let ed = &st.s.editor;
    let problems = super::hstudio_assist::check(&ed.lines, &ed.title, siblings.as_deref());
    let did = did_text(t.removed, t.lines, t.animated, &t.file);
    let mut out = json!({
        "ok": true,
        "did": did,
        "file": t.file,
        "lines": [t.lines.0, t.lines.1],
        "line_count": line_count(&ed.lines),
        "dirty": ed.dirty,
        "animated": t.animated,
        "saved": false,
        "problems": problems_json(&problems),
    });
    if let Some((a, b)) = t.removed {
        out["lines"] = json!([]);
        out["removed"] = json!([a, b]);
    }
    if let Some(why) = cut_short {
        out["cut_short"] = json!(format!("{why}: the rest went in at once"));
        if t.save {
            out["why"] = json!(format!("not saved: {why}"));
        }
    }
    let jump = Jump {
        path: t.path.clone(),
        file: t.file.clone(),
        first: t.lines.0 - 1,
        last: t.lines.1.max(t.lines.0) - 1,
    };
    say(st, &t.by, t.in_run, did, Some(jump));
    out
}

fn keep_result(st: &mut Studio, id: u64, result: &Json) {
    let now = st.s.now;
    st.s.ui.coach.results.push((id, now, result.clone()));
}

/// Saves the editor's file for an agent, unless the player is typing in it
/// or the file on disk changed since the editor read it (the player
/// chooses then); a line in the Agent panel says how its mod is. Returns
/// the mod's state.
fn save_now(st: &mut Studio, game: &mut dyn GameApi, by: &By, in_run: bool) -> Result<Json, String> {
    let ed = &st.s.editor;
    let file = ed.title.clone();
    let path = ed.path.clone().ok_or("the editor has no file open")?;
    if st.s.ui.coach.player_editing(st.s.now) {
        return Err(format!("the player is typing in {file}"));
    }
    if ed.disk_changed() {
        return Err(format!(
            "{file} changed on disk since the editor read it: the player chooses what to keep (the editor asks \
             them), and read_mod shows the file"
        ));
    }
    st.save_editor(game);
    if st.s.editor.dirty {
        return Err(format!("could not save {file}: the console says why"));
    }
    st.s.ui.coach.unsaved = None;
    let m = mod_state(st, &path);
    let line = m["line"].as_u64().filter(|_| m["state"] == "failed");
    let jump = line.map(|n| Jump {
        path,
        file: file.clone(),
        first: n as usize - 1,
        last: n as usize - 1,
    });
    say(st, by, in_run, saved_text(&file, &m), jump);
    Ok(m)
}

/// An edit's save, as it asked: its result says whether it was saved, and
/// how the mod is, or why not.
fn save_into(st: &mut Studio, game: &mut dyn GameApi, t: &Typing, result: &mut Json) {
    match save_now(st, game, &t.by, t.in_run) {
        Ok(m) => {
            result["saved"] = json!(true);
            result["mod"] = m;
        }
        Err(why) => result["why"] = json!(format!("not saved: {why}")),
    }
}

/// After the game's own agent's turn: its edits nobody saved are saved when
/// the turn ended well and nothing changed the file since (the player's
/// edits, a change on disk); otherwise a line says they aren't, and how
/// to keep them or take them back. Other agents' edits are theirs to save.
fn end_turn(st: &mut Studio, game: &mut dyn GameApi, ok: bool) {
    finish_typing(st, "the agent's turn ended");
    let Some(u) = st.s.ui.coach.unsaved.take() else {
        return;
    };
    let ed = &st.s.editor;
    if !u.in_run || ed.path.as_ref() != Some(&u.path) || !ed.dirty {
        // Another agent's, or saved since (switching files saves them).
        st.s.ui.coach.unsaved = Some(u).filter(|u| !u.in_run);
        return;
    }
    let why = if !ok {
        None
    } else if ed.revision != u.rev || u.player_edits || st.s.ui.coach.player_editing(st.s.now) {
        Some("you changed the file since")
    } else if ed.disk_changed() {
        Some("the file changed on disk since")
    } else {
        match save_now(st, game, &u.by, true) {
            Ok(_) => return,
            Err(_) => Some("it couldn't be saved"),
        }
    };
    let text = match why {
        Some(why) => format!(
            "{}'s edits to {} aren't saved ({why}): Ctrl+S saves them, Ctrl+Z takes them back",
            u.by.name, u.file
        ),
        None => format!(
            "{}'s edits to {} aren't saved: Ctrl+S saves them, Ctrl+Z takes them back",
            u.by.name, u.file
        ),
    };
    let now = st.s.now;
    st.s.ui.agent.say(AgentLineKind::Done, text, now);
}

/// The code Agent panel line number `seq` is about, shown: its file opens
/// (the one open saved first, as the script list does: it is the player's
/// own click), its first line a third of the way down with the cursor on
/// it, its lines lit a moment, and the keys go back to the code.
pub fn go(st: &mut Studio, game: &mut dyn GameApi, seq: u64) {
    let pane = &st.s.ui.agent;
    let jump = pane
        .seq
        .checked_sub(seq)
        .and_then(|back| pane.lines.len().checked_sub(back as usize + 1))
        .and_then(|i| pane.lines[i].jump.clone());
    let Some(jump) = jump else {
        return;
    };
    let open = st.s.editor.path.as_ref() == Some(&jump.path);
    if !open && !jump.path.is_file() {
        st.s.toast(format!("{} isn't there any more", jump.file));
        return;
    }
    if !open {
        st.save_before_switch(game, Some(&jump.path));
        st.open_file(&jump.path);
    }
    show_line(st, jump.first + 1, true);
    let now = st.s.now;
    let ui = &mut st.s.ui;
    ui.tab = Tab::Editor;
    ui.panel_focus = false;
    ui.coach.notice = None;
    ui.coach.flash = Some(Flash {
        path: jump.path,
        first: jump.first,
        last: jump.last,
        at: now,
    });
}

/// The link's `typed`: whether edit `id` is all in yet, and then its result.
pub fn typed(st: &Studio, params: &Json) -> Result<Json, String> {
    let id = params
        .get("id")
        .and_then(Json::as_u64)
        .ok_or("typed needs the id edit gave")?;
    let c = &st.s.ui.coach;
    if c.typing.as_ref().is_some_and(|t| t.id == id) {
        return Ok(json!({ "done": false, "typing": id }));
    }
    match c.results.iter().find(|(i, ..)| *i == id) {
        Some((_, _, r)) => Ok(json!({ "done": true, "result": r })),
        None => Err(format!("no edit {id}: an edit's result is kept {RESULTS_KEPT} s")),
    }
}

/// The link's `save`: the editor's file saved, as Ctrl+S saves it (its mod
/// reloads), unless the player is typing in it (then it waits) or it
/// changed on disk since the editor read it.
pub fn save(st: &mut Studio, game: &mut dyn GameApi, by: &By, params: &Json) -> Result<Json, String> {
    finish_typing(st, "save_script came");
    let t = target(st, params, false)?;
    let ed = &st.s.editor;
    if ed.path.as_deref() != Some(t.path.as_path()) {
        return Err(format!(
            "{} isn't open in the editor: edit_script writes a file the editor doesn't have open at once, so \
             there is nothing to save",
            t.file
        ));
    }
    if !ed.dirty {
        return Ok(json!({
            "ok": true,
            "saved": false,
            "did": format!("{} has no unsaved edits", t.file),
            "file": t.file,
            "mod": mod_state(st, &t.path),
        }));
    }
    if st.s.ui.coach.player_editing(st.s.now) {
        return Ok(busy(format!("the player is typing in {}", t.file)));
    }
    let in_run = in_run(st, by);
    let m = save_now(st, game, by, in_run)?;
    let siblings = super::hstudio_ui::editor_siblings(st);
    let ed = &st.s.editor;
    let problems = super::hstudio_assist::check(&ed.lines, &ed.title, siblings.as_deref());
    Ok(json!({
        "ok": true,
        "saved": true,
        "did": saved_text(&t.file, &m),
        "file": t.file,
        "mod": m,
        "problems": problems_json(&problems),
    }))
}

// ----------------------------------------------------------------- the link's methods

/// The link's editor methods for the agent `by`: `editor` reads, `open`
/// opens, `edit` changes, `typed` says whether an edit is in, `save`
/// saves. A request that changes something and carries a `key` is done
/// once: sent again with the same key (its client trying again after a
/// timeout), it gets the same answer. One that has to wait for the player
/// isn't kept, so asking again tries again.
pub fn call(st: &mut Studio, game: &mut dyn GameApi, by: &By, method: &str, params: &Json) -> Result<Json, String> {
    let key = text(params, "key")
        .filter(|_| !matches!(method, "editor" | "typed"))
        .map(str::to_owned);
    if let Some(k) = &key
        && let Some((_, answer)) = st.s.ui.coach.replies.iter().find(|(kept, _)| kept == k)
    {
        return answer.clone();
    }
    let answer = match method {
        // Reading never puts an edit in before its time: the text read is
        // what it will be.
        "editor" => read(st, params),
        "typed" => typed(st, params),
        "open" => {
            finish_typing(st, "open_script came");
            open(st, by, params)
        }
        "edit" => edit(st, game, by, params),
        "save" => save(st, game, by, params),
        other => Err(format!("no editor method '{other}'")),
    };
    let waits = answer.as_ref().is_ok_and(|a| a["busy"] == true);
    if let Some(k) = key
        && !waits
    {
        let replies = &mut st.s.ui.coach.replies;
        if replies.len() >= REPLIES {
            replies.pop_front();
        }
        replies.push_back((k, answer.clone()));
    }
    answer
}
