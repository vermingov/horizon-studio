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

use super::hstudio::{GameApi, Studio, mod_id};
use super::hstudio_modfiles::{file_name, path_in, plan_mod};
use super::hstudio_ui::{Editor, Input, Tab};
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
}

impl Default for Coach {
    fn default() -> Self {
        Coach {
            player_at: f64::NEG_INFINITY,
            input_at: f64::NEG_INFINITY,
            replies: VecDeque::new(),
        }
    }
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
}

/// The editor's file and revision before an input, for `after_input`.
pub struct Before {
    revision: u64,
    path: Option<PathBuf>,
}

/// Before the player's input reaches the studio: it is noted.
pub fn before_input(st: &mut Studio, input: &Input) -> Before {
    let pressed = matches!(
        input,
        Input::Key { down: true, .. } | Input::Text(_) | Input::Button { down: true, .. } | Input::Wheel(_)
    );
    if pressed {
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
        ed.lines.clone()
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
    }
    Ok(out)
}

/// The editor for `status`: the file open, whether it has unsaved edits, its lines.
pub fn summary(st: &Studio) -> Json {
    let ed = &st.s.editor;
    json!({
        "file": ed.path.as_ref().map(|_| ed.title.clone()),
        "path": ed.path,
        "dirty": ed.dirty,
        "line_count": line_count(&ed.lines),
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
pub fn open(st: &mut Studio, params: &Json) -> Result<Json, String> {
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
            return Err(format!(
                "the player has unsaved edits in {}: open_script opens another file once they are saved \
                 (Ctrl+S); edit_script can change {} without opening it",
                ed.title, t.file
            ));
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

// ----------------------------------------------------------------- the link's methods

/// The link's editor methods for the agent `by`: `editor` reads, `open`
/// opens. A request that changes something and carries a `key` is done
/// once: sent again with the same key (its client trying again after a
/// timeout), it gets the same answer. One that has to wait for the player
/// isn't kept, so asking again tries again.
pub fn call(st: &mut Studio, _game: &mut dyn GameApi, _by: &By, method: &str, params: &Json) -> Result<Json, String> {
    let key = text(params, "key").filter(|_| method != "editor").map(str::to_owned);
    if let Some(k) = &key
        && let Some((_, answer)) = st.s.ui.coach.replies.iter().find(|(kept, _)| kept == k)
    {
        return answer.clone();
    }
    let answer = match method {
        "editor" => read(st, params),
        "open" => open(st, params),
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
