// Right-click menus: the menu of each thing the studio draws, and what their
// items do. look.rs and skins.rs mark where each thing is drawn with a spot
// (`Action::Spot`); a right press opens the menu of the spot under the
// pointer in `Ui::menu`, the widget the agent's model chip and the script
// list's "..." open too. ui.rs routes the keys, the text and the pointer to
// the menu while it is open, and hands the item chosen to `run`. Items name
// mods by id and files by path, never by their place in a list: a scan
// between the menu opening and an item chosen can move them.

use super::hstudio::{GameApi, ModState, Studio};
use super::hstudio_examples::EXAMPLES;
use super::hstudio_look::docs_markdown;
use super::hstudio_modfiles::{path_in, rel_in};
use super::hstudio_theme::{Layout, THEMES};
use super::hstudio_ui::{
    AREA_AGENT, AREA_LOG, Action, EditCmd, FIELD_AGENT, FieldCmd, Menu, MenuCmd, MenuItem, ModCmd, NamingKind, Panel,
    ScriptRow, Spot, Tab, UiResult, act, agent_busy, agent_choose, agent_id, agent_list, agent_model,
    agent_new_conversation, agent_rescan, agent_send, copy_text, editor_pos, file_in_module, in_text_selection, inside,
    make_module, menu_pick, model_choices, model_items, model_label, notice, open_module, open_naming, open_path,
    run_console_line, selected_text, set_agent_model, spot_at,
};
use std::path::Path;

/// A right press at the pointer: an open menu closes (a press on it does
/// nothing), and the menu of what is under the pointer opens there, if it
/// has one. Nothing opens over the naming prompt, or while the left button
/// drags something.
pub fn right_press(st: &mut Studio, game: &mut dyn GameApi) -> UiResult {
    let ui = &st.s.ui;
    let at = ui.mouse;
    if ui.naming.is_some() || ui.dragging_any() {
        return UiResult::Used;
    }
    if ui.menu.as_ref().and_then(|m| m.rect).is_some_and(|r| inside(r, at)) {
        return UiResult::Used;
    }
    st.s.ui.menu = None;
    if let Some((spot, rect)) = spot_at(&st.s.ui, at) {
        open_spot(st, game, spot, rect, [at.0, at.1]);
    }
    UiResult::Used
}

/// Opens the menu of `spot`, drawn at `rect`, with its top left at `at`
/// (the pointer), after what a right click there does first: the code's
/// cursor goes there unless that is in the selection, a Library row is
/// chosen, the panel's input takes the keys. Text selected in the console
/// or the agent's conversation stays only for a click in it (Ctrl+C would
/// copy it, not what was clicked). False when the spot has no menu, or
/// nothing in it can be chosen now.
fn open_spot(st: &mut Studio, game: &mut dyn GameApi, spot: Spot, rect: [f32; 4], at: [f32; 2]) -> bool {
    let keep_text = matches!(spot, Spot::Text(area) if in_text_selection(&st.s.ui, area, (at[0], at[1])));
    if !keep_text {
        st.s.ui.text_sel = None;
    }
    let (items, target) = match spot {
        Spot::Code => {
            let drawn = st.s.ui.smooth[1];
            if let Some((row, col)) = editor_pos(st, game, at[0], at[1], drawn) {
                let ed = &mut st.s.editor;
                let line = &ed.lines[row];
                let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
                if !ed.in_selection(row, byte) {
                    ed.click(row, col, false);
                    // Put under the pointer, the cursor is on screen: the view stays.
                    ed.followed = Some((ed.row, ed.col, ed.revision));
                }
            }
            st.s.ui.panel_focus = false;
            (code_items(st), None)
        }
        Spot::Text(area) => (text_items(st, area), None),
        Spot::Field(field) => {
            let ui = &mut st.s.ui;
            if ui.tab == Tab::Editor {
                ui.panel = if field == FIELD_AGENT {
                    Panel::Agent
                } else {
                    Panel::Console
                };
                ui.panel_focus = true;
            }
            (field_items(st, field), None)
        }
        Spot::File => (file_items(st), None),
        Spot::Scripts => (scripts_items(st), None),
        Spot::Row(k) => (row_items(st, usize::from(k)), Some(rect)),
        Spot::Mod(k) => {
            let Some(i) = drawn_mod(st, k) else { return false };
            st.s.ui.library_sel = i;
            (library_items(st, i), Some(rect))
        }
        Spot::Example(i) => (example_items(st, i), Some(rect)),
        Spot::Panel(p) => (panel_items(st, p), None),
        Spot::Problem(k) => {
            st.s.ui.problem_sel = k;
            (problem_items(st, k), Some(rect))
        }
        Spot::Window => (window_items(st), None),
        Spot::Themes => (theme_items(st), None),
        Spot::Docs => (
            vec![MenuItem::new("Copy the docs as Markdown", MenuCmd::CopyDocs)],
            None,
        ),
    };
    open(st, at, 0.0, items, target)
}

// ----------------------------------------------------------------- the panel, the window, the themes

/// The menu of the bottom panel's tab `p` (its tab, its buttons, its empty
/// space): the console's Copy all and Clear; the problems' Copy all
/// problems; the agent's Stop, New conversation, Copy the conversation, the
/// agents found (the one in use marked), Look for agents again and its
/// model; then Hide the panel.
fn panel_items(st: &mut Studio, p: Panel) -> Vec<MenuItem> {
    let mut items = match p {
        Panel::Console => vec![
            MenuItem::new("Copy all", MenuCmd::CopyAll(AREA_LOG)).enabled(!st.s.log.is_empty()),
            MenuItem::new("Clear", MenuCmd::Act(Action::ClearLog))
                .keys("Ctrl L")
                .enabled(!st.s.log.is_empty()),
        ],
        Panel::Problems => vec![
            MenuItem::new("Copy all problems", MenuCmd::Copy(all_problems(st)))
                .enabled(!st.s.editor.problems.is_empty()),
        ],
        Panel::Agent => {
            let busy = agent_busy(st);
            let said = !st.s.ui.agent.lines.is_empty();
            let mut items = vec![
                MenuItem::new("Stop", MenuCmd::Act(Action::AgentStop)).enabled(busy),
                MenuItem::new("New conversation", MenuCmd::NewConversation).enabled(!busy && said),
                MenuItem::new("Copy the conversation", MenuCmd::CopyAll(AREA_AGENT)).enabled(said),
                MenuItem::separator(),
            ];
            let (agents, chosen) = agent_list(st);
            for (k, (id, name)) in agents.into_iter().enumerate() {
                items.push(
                    MenuItem::new(name, MenuCmd::Agent(id))
                        .checked(k == chosen)
                        .enabled(!busy),
                );
            }
            items.push(MenuItem::new("Look for agents again", MenuCmd::FindAgents).enabled(!busy));
            if let Some(id) = agent_id(st).filter(|i| !model_choices(i).is_empty()) {
                let model = agent_model(st);
                let label = format!("Model: {}", model_label(&id, model.as_deref()));
                items.push(MenuItem::new(label, MenuCmd::Models).keys("›"));
            }
            items
        }
    };
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Hide the panel", MenuCmd::HidePanel));
    items
}

/// Every problem the live check found, a line each as "<file>:<line>: <message>".
fn all_problems(st: &Studio) -> String {
    let ed = &st.s.editor;
    let lines: Vec<String> = ed
        .problems
        .iter()
        .map(|p| format!("{}:{}: {}", ed.title, p.line, p.message))
        .collect();
    lines.join("\n")
}

/// A problem's row: Go to it (its place, taken now), Copy, and Copy all problems.
fn problem_items(st: &Studio, k: usize) -> Vec<MenuItem> {
    let ed = &st.s.editor;
    let Some(p) = ed.problems.get(k) else {
        return Vec::new();
    };
    let row = (p.line.max(1) - 1).min(ed.lines.len() - 1);
    let (start, end) = p.span.unwrap_or((0, 0));
    vec![
        MenuItem::new("Go to it", MenuCmd::GoTo(None, row, start, end)).keys("⏎"),
        MenuItem::new("Copy", MenuCmd::Copy(format!("{}:{}: {}", ed.title, p.line, p.message))),
        MenuItem::separator(),
        MenuItem::new("Copy all problems", MenuCmd::Copy(all_problems(st))),
    ]
}

/// The window's menu (its title bar, the studio's header): rolling it up,
/// filling the screen and putting it back, full screen or a window (the
/// studio's own layout), the themes, Settings and Close.
fn window_items(st: &Studio) -> Vec<MenuItem> {
    let ui = &st.s.ui;
    let look = &ui.look;
    let mut items = Vec::new();
    if look.windowed() {
        let roll = if ui.collapsed { "Unroll" } else { "Roll up" };
        let fill = if look.maximized { "Restore" } else { "Fill the screen" };
        let moved = look.rect.is_some() || look.maximized || ui.collapsed;
        items.push(MenuItem::new(roll, MenuCmd::Act(Action::WinMin)));
        items.push(MenuItem::new(fill, MenuCmd::Act(Action::WinMax)));
        items.push(MenuItem::new("Reset window", MenuCmd::Act(Action::ResetWindow)).enabled(moved));
    }
    if look.theme().layout == Layout::Studio {
        let (label, window) = if look.window {
            ("Full screen", false)
        } else {
            ("In a window", true)
        };
        items.push(MenuItem::new(label, MenuCmd::Act(Action::Windowed(window))));
    }
    items.push(MenuItem::new("Theme", MenuCmd::Themes).keys("›"));
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Settings", MenuCmd::Act(Action::Tab(Tab::Settings))).keys("Ctrl 5"));
    items.push(MenuItem::new("Close", MenuCmd::Act(Action::Close)).keys("Esc"));
    items
}

/// Every theme, the one in use marked.
fn theme_items(st: &Studio) -> Vec<MenuItem> {
    THEMES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            MenuItem::new(t.name, MenuCmd::Act(Action::Theme(i.min(255) as u8))).checked(st.s.ui.look.theme == i)
        })
        .collect()
}

/// A list in the place of the menu chosen from (`place`, where it was
/// drawn), its current choice lit, for the keys.
fn replace(st: &mut Studio, place: [f32; 4], items: Vec<MenuItem>) {
    if open(st, [place[0], place[1]], 0.0, items, None)
        && let Some(menu) = st.s.ui.menu.as_mut()
    {
        menu.sel = menu.items.iter().position(|i| i.checked);
    }
}

/// The editor's cursor to a place, selecting it: a row and its bytes from
/// and to, in `path` (opened first) or in the file open.
fn go_to(st: &mut Studio, game: &mut dyn GameApi, path: Option<&Path>, row: usize, start: usize, end: usize) {
    if let Some(path) = path {
        open_path(st, game, path);
    }
    st.s.editor.select_span(row, start, end);
    let ui = &mut st.s.ui;
    ui.tab = Tab::Editor;
    ui.panel_focus = false;
    ui.suggest = None;
}

// ----------------------------------------------------------------- the code, the text, the input lines

/// The code's menu: the clipboard and the selection, the line commands,
/// then running and saving.
fn code_items(st: &Studio) -> Vec<MenuItem> {
    let _ = st;
    let edit = |label: &str, keys: &str, cmd: EditCmd| MenuItem::new(label, MenuCmd::Edit(cmd)).keys(keys);
    vec![
        edit("Cut", "Ctrl X", EditCmd::Cut),
        edit("Copy", "Ctrl C", EditCmd::Copy),
        // Always there: the clipboard is read only when it is chosen.
        edit("Paste", "Ctrl V", EditCmd::Paste),
        edit("Select all", "Ctrl A", EditCmd::SelectAll),
        MenuItem::separator(),
        edit("Toggle comment", "Ctrl /", EditCmd::Comment),
        edit("Indent", "Tab", EditCmd::Indent),
        edit("Outdent", "Shift Tab", EditCmd::Outdent),
        edit("Duplicate line", "Ctrl D", EditCmd::DuplicateLine),
        MenuItem::separator(),
        MenuItem::new("Run", MenuCmd::Act(Action::Run)).keys("Ctrl ⏎"),
        MenuItem::new("Save", MenuCmd::Act(Action::Save)).keys("Ctrl S"),
    ]
}

/// The menu of the console's lines or the agent's conversation: copying
/// what is selected there or all of it, then clearing the console (or
/// stopping the agent, a new conversation).
fn text_items(st: &Studio, area: u8) -> Vec<MenuItem> {
    let selected = st.s.ui.text_sel.is_some_and(|s| s.area == area && s.anchor != s.head);
    let copy = MenuItem::new("Copy", MenuCmd::CopySelection)
        .keys("Ctrl C")
        .enabled(selected);
    if area == AREA_AGENT {
        let busy = agent_busy(st);
        let said = !st.s.ui.agent.lines.is_empty();
        return vec![
            copy,
            MenuItem::new("Copy the conversation", MenuCmd::CopyAll(AREA_AGENT)).enabled(said),
            MenuItem::separator(),
            MenuItem::new("Stop", MenuCmd::Act(Action::AgentStop)).enabled(busy),
            MenuItem::new("New conversation", MenuCmd::NewConversation).enabled(!busy && said),
        ];
    }
    vec![
        copy,
        MenuItem::new("Copy all", MenuCmd::CopyAll(AREA_LOG)).enabled(!st.s.log.is_empty()),
        MenuItem::separator(),
        MenuItem::new("Clear", MenuCmd::Act(Action::ClearLog))
            .keys("Ctrl L")
            .enabled(!st.s.log.is_empty()),
        MenuItem::new("Jump to the newest", MenuCmd::LogEnd).enabled(st.s.ui.log_scroll > 0),
    ]
}

/// The menu of the console's input line or the agent's: the clipboard and
/// the line, then running it (or sending it).
fn field_items(st: &Studio, field: u8) -> Vec<MenuItem> {
    let field_cmd = |cmd: FieldCmd| MenuCmd::Field(field, cmd);
    if field == FIELD_AGENT {
        let typed = !st.s.ui.agent_input.text.trim().is_empty();
        let busy = agent_busy(st);
        return vec![
            MenuItem::new("Paste", field_cmd(FieldCmd::Paste)).keys("Ctrl V"),
            MenuItem::new("Clear the line", field_cmd(FieldCmd::Clear)).enabled(typed),
            MenuItem::separator(),
            MenuItem::new("Send", field_cmd(FieldCmd::Submit))
                .keys("⏎")
                .enabled(typed && !busy),
            MenuItem::new("Stop", MenuCmd::Act(Action::AgentStop)).enabled(busy),
        ];
    }
    let typed = !st.s.ui.input.text.is_empty();
    vec![
        MenuItem::new("Paste", field_cmd(FieldCmd::Paste)).keys("Ctrl V"),
        MenuItem::new("Copy the line", field_cmd(FieldCmd::Copy))
            .keys("Ctrl C")
            .enabled(typed),
        MenuItem::new("Cut the line", field_cmd(FieldCmd::Cut))
            .keys("Ctrl X")
            .enabled(typed),
        MenuItem::new("Clear the line", field_cmd(FieldCmd::Clear)).enabled(typed),
        MenuItem::separator(),
        MenuItem::new("Run", field_cmd(FieldCmd::Submit))
            .keys("⏎")
            .enabled(typed),
        MenuItem::new("Clear the console", MenuCmd::Act(Action::ClearLog))
            .keys("Ctrl L")
            .enabled(!st.s.log.is_empty()),
    ]
}

/// An editor command from its menu, as its keys do it.
fn edit(st: &mut Studio, game: &mut dyn GameApi, cmd: EditCmd) {
    let ed = &mut st.s.editor;
    match cmd {
        EditCmd::Cut => {
            let t = ed.cut();
            copy_text(st, game, &t);
        }
        EditCmd::Copy => {
            let t = ed.copy();
            copy_text(st, game, &t);
        }
        EditCmd::Paste => {
            let clip = game.clipboard();
            if !clip.is_empty() {
                ed.insert(&clip);
            }
        }
        EditCmd::SelectAll => ed.select_all(),
        EditCmd::Comment => ed.toggle_comment(),
        EditCmd::Indent => ed.indent_lines(false),
        EditCmd::Outdent => ed.indent_lines(true),
        EditCmd::DuplicateLine => ed.duplicate_line(),
    }
    let ui = &mut st.s.ui;
    ui.suggest = None;
    ui.panel_focus = false;
    ui.tab = Tab::Editor;
}

/// An input line's command from its menu.
fn field(st: &mut Studio, game: &mut dyn GameApi, which: u8, cmd: FieldCmd) {
    let agent = which == FIELD_AGENT;
    let text = if agent {
        st.s.ui.agent_input.text.clone()
    } else {
        st.s.ui.input.text.clone()
    };
    match cmd {
        FieldCmd::Paste => {
            let clip = game.clipboard();
            let line = if agent {
                &mut st.s.ui.agent_input
            } else {
                &mut st.s.ui.input
            };
            line.insert(&clip);
        }
        FieldCmd::Copy => copy_text(st, game, &text),
        FieldCmd::Cut | FieldCmd::Clear => {
            if cmd == FieldCmd::Cut {
                copy_text(st, game, &text);
            }
            if agent {
                st.s.ui.agent_input.set("");
            } else {
                st.s.ui.input.set("");
                st.s.ui.hist = None;
            }
        }
        FieldCmd::Submit if agent => agent_send(st),
        FieldCmd::Submit => run_console_line(st, game),
    }
}

/// All of the console's lines, or the agent's conversation (what the
/// player asked marked with "> "), as text.
fn all_text(st: &Studio, area: u8) -> String {
    if area == AREA_AGENT {
        use super::hstudio_ui::AgentLineKind;
        let lines: Vec<String> =
            st.s.ui
                .agent
                .lines
                .iter()
                .map(|l| match l.kind {
                    AgentLineKind::You => format!("> {}", l.text),
                    _ => l.text.clone(),
                })
                .collect();
        return lines.join("\n");
    }
    let lines: Vec<&str> = st.s.log.iter().map(|l| l.text.as_str()).collect();
    lines.join("\n")
}

/// The mod a Library row was drawn for (`k`, its place in the list then),
/// wherever it is listed now; None when it is gone.
fn drawn_mod(st: &Studio, k: usize) -> Option<usize> {
    let id = st.s.ui.drawn_mods.get(k)?;
    st.s.mods.iter().position(|m| &m.id == id)
}

/// Where the last frame drew `spot`, if it did.
pub(crate) fn spot_rect(st: &Studio, spot: Spot) -> Option<[f32; 4]> {
    st.s.ui
        .hits
        .iter()
        .rev()
        .find(|(_, a)| *a == Action::Spot(spot))
        .map(|(r, _)| *r)
}

/// Opens a menu of `items` (separators at its ends and doubled ones left
/// out) at `at`, above it by `anchor_h` more where there is no room below,
/// outlining `target`. False, and nothing opens, when no item can be chosen.
fn open(st: &mut Studio, at: [f32; 2], anchor_h: f32, items: Vec<MenuItem>, target: Option<[f32; 4]>) -> bool {
    let items = tidy(items);
    if !items.iter().any(|i| i.enabled) {
        return false;
    }
    let mut menu = Menu::new(at, anchor_h, items, st.s.now);
    menu.target = target;
    st.s.ui.menu = Some(menu);
    st.s.ui.suggest = None;
    true
}

/// A menu's items without separators at its ends, or two in a row.
fn tidy(items: Vec<MenuItem>) -> Vec<MenuItem> {
    let mut out: Vec<MenuItem> = Vec::with_capacity(items.len());
    for it in items {
        if it.separator && out.last().is_none_or(|l| l.separator) {
            continue;
        }
        out.push(it);
    }
    while out.last().is_some_and(|l| l.separator) {
        out.pop();
    }
    out
}

/// Text typed while a menu is open: a space chooses the lit item, and a
/// letter lights the next item that starts with it (nothing reaches what
/// is under the menu).
pub fn menu_text(st: &mut Studio, game: &mut dyn GameApi, t: &str) -> UiResult {
    if t == " " {
        return match st.s.ui.menu.as_ref().and_then(|m| m.sel) {
            Some(i) => menu_pick(st, game, i),
            None => UiResult::Used,
        };
    }
    let Some(c) = t.chars().next() else {
        return UiResult::Used;
    };
    let Some(menu) = st.s.ui.menu.as_mut() else {
        return UiResult::Used;
    };
    let n = menu.items.len();
    let start = menu.sel.map_or(0, |s| s + 1);
    let same = |a: char| a.to_lowercase().eq(c.to_lowercase());
    if let Some(i) = (0..n).map(|k| (start + k) % n).find(|&i| {
        let it = &menu.items[i];
        it.enabled && it.label.chars().next().is_some_and(same)
    }) {
        menu.sel = Some(i);
    }
    UiResult::Used
}

/// What a menu item does, chosen from a menu drawn at `place`.
pub fn run(st: &mut Studio, game: &mut dyn GameApi, cmd: MenuCmd, place: [f32; 4]) -> UiResult {
    let _ = place;
    match cmd {
        MenuCmd::Model(model) => set_agent_model(st, model),
        MenuCmd::CustomModel => {
            let current = agent_model(st).unwrap_or_default();
            open_naming(st, NamingKind::Model, &current);
        }
        MenuCmd::NewScript => open_naming(st, NamingKind::NewScript, ""),
        MenuCmd::NewModule => open_naming(st, NamingKind::NewModule, ""),
        MenuCmd::NewFile(folder) => open_naming(st, NamingKind::NewFile(folder), ""),
        MenuCmd::Open(path) => open_path(st, game, &path),
        MenuCmd::Rename(path) => {
            // The field starts with its path in the module.
            let rel = file_in_module(st, &path).map(|(_, rel)| rel).unwrap_or_default();
            let stem = rel.strip_suffix(".lua").unwrap_or(&rel).to_owned();
            open_naming(st, NamingKind::Rename(path), &stem);
        }
        MenuCmd::Delete(path) => open_naming(st, NamingKind::Delete(path), ""),
        MenuCmd::MakeModule(id) => {
            if let Some(i) = st.s.mods.iter().position(|m| m.id == id) {
                make_module(st, game, i);
            }
        }
        MenuCmd::Reveal(path) => game.open_folder(&path),
        MenuCmd::Act(a) => return act(st, game, a),
        MenuCmd::Edit(cmd) => edit(st, game, cmd),
        MenuCmd::Field(which, cmd) => field(st, game, which, cmd),
        MenuCmd::CopySelection => {
            if let Some(text) = selected_text(&st.s.ui) {
                copy_text(st, game, &text);
            }
        }
        MenuCmd::CopyAll(area) => {
            let text = all_text(st, area);
            copy_text(st, game, &text);
        }
        MenuCmd::Copy(text) => copy_text(st, game, &text),
        MenuCmd::LogEnd => st.s.ui.log_scroll = 0,
        MenuCmd::NewConversation => agent_new_conversation(st),
        MenuCmd::Mod(id, cmd) => return mod_cmd(st, game, &id, cmd),
        MenuCmd::Revert => {
            if st.s.editor.revert() {
                st.s.editor.reloaded_at = Some(st.s.now);
                notice(st, "Back to the saved file: Ctrl+Z brings your edits back");
            }
        }
        MenuCmd::Agent(id) => agent_choose(st, &id),
        MenuCmd::FindAgents => agent_rescan(st),
        MenuCmd::HidePanel => {
            st.s.ui.panel_hidden = true;
            st.s.ui.panel_focus = false;
        }
        MenuCmd::Themes => {
            let items = theme_items(st);
            replace(st, place, items);
        }
        MenuCmd::Models => {
            let items = model_items(st);
            replace(st, place, items);
        }
        MenuCmd::CopyDocs => copy_text(st, game, &docs_markdown()),
        MenuCmd::GoTo(path, row, start, end) => go_to(st, game, path.as_deref(), row, start, end),
    }
    UiResult::Used
}

// ----------------------------------------------------------------- the script list and the Library

/// The menu of row `k` of the script list, at `at` (its "...", or the
/// pointer), the row outlined.
pub(crate) fn open_row_menu(st: &mut Studio, k: usize, at: [f32; 4]) {
    let items = row_items(st, k);
    let target = u16::try_from(k).ok().and_then(|k| spot_rect(st, Spot::Row(k)));
    open(st, [at[0], at[1] + at[3]], at[3], items, target);
}

/// The items of row `k` of the script list. A script's: Open (unless it is
/// open), then its mod's. A module's: New file, Open main.lua, then its
/// mod's. A module's file: Open, Rename and Delete (not main.lua, which
/// starts the module), its folder and its path. The scratch file's: Open,
/// or Run and Save as a script while it is open, its folder and its path.
fn row_items(st: &Studio, k: usize) -> Vec<MenuItem> {
    let Some(row) = st.s.ui.script_rows.get(k).cloned() else {
        return Vec::new();
    };
    let open = st.s.editor.path.clone();
    let open_item =
        |path: &Path| (open.as_deref() != Some(path)).then(|| MenuItem::new("Open", MenuCmd::Open(path.to_path_buf())));
    match row {
        ScriptRow::Scratch => {
            let path = st.scratch_path();
            let mut items: Vec<MenuItem> = Vec::new();
            if open.as_deref() == Some(path.as_path()) {
                items.push(MenuItem::new("Run", MenuCmd::Act(Action::Run)).keys("Ctrl ⏎"));
                items.push(MenuItem::new("Save as a script...", MenuCmd::Act(Action::SaveFile)));
            } else {
                items.push(MenuItem::new("Open", MenuCmd::Act(Action::EditScratch)));
            }
            items.push(MenuItem::separator());
            items.push(MenuItem::new("Show in folder", MenuCmd::Reveal(st.s.dir.clone())));
            items.push(copy_path(&path));
            items
        }
        ScriptRow::Module { id, .. } => {
            let Some(i) = st.s.mods.iter().position(|m| m.id == id) else {
                return Vec::new();
            };
            let Some(folder) = st.s.mods[i].folder.clone() else {
                return Vec::new();
            };
            let mut items = vec![
                MenuItem::new("New file...", MenuCmd::NewFile(folder.clone())),
                MenuItem::new("Open main.lua", MenuCmd::Open(folder.join("main.lua"))),
                MenuItem::separator(),
            ];
            items.extend(mod_items(st, i, false));
            items
        }
        ScriptRow::File { path, rel, .. } => {
            let main = rel == "main.lua";
            let mut items: Vec<MenuItem> = open_item(&path).into_iter().collect();
            items.push(MenuItem::new("Rename...", MenuCmd::Rename(path.clone())).enabled(!main));
            items.push(
                MenuItem::new("Delete...", MenuCmd::Delete(path.clone()))
                    .enabled(!main)
                    .danger(),
            );
            items.push(MenuItem::separator());
            if let Some(dir) = path.parent() {
                items.push(MenuItem::new("Show in folder", MenuCmd::Reveal(dir.to_path_buf())));
            }
            items.push(copy_path(&path));
            items
        }
        ScriptRow::Script { path, .. } => {
            let Some(i) = st.mod_of_path(&path) else {
                return Vec::new();
            };
            let mut items: Vec<MenuItem> = open_item(&path).into_iter().collect();
            items.push(MenuItem::separator());
            items.extend(mod_items(st, i, false));
            items
        }
    }
}

/// Copy path: `path` to the clipboard, as the system writes it.
fn copy_path(path: &Path) -> MenuItem {
    MenuItem::new("Copy path", MenuCmd::Copy(path.display().to_string()))
}

/// What every mod's menu has, after what its list puts first: Reload, Turn
/// off or on, Rename, Duplicate, Make it a module (a script), Delete, its
/// folder and its path, and for a mod that failed Copy the error. With
/// `keys`, the Library's keys for them.
fn mod_items(st: &Studio, i: usize, keys: bool) -> Vec<MenuItem> {
    let Some(m) = st.s.mods.get(i) else {
        return Vec::new();
    };
    let key = |it: MenuItem, k: &str| if keys { it.keys(k) } else { it };
    let cmd = |c: ModCmd| MenuCmd::Mod(m.id.clone(), c);
    let toggle = if m.enabled { "Turn off" } else { "Turn on" };
    let mut items = vec![
        key(MenuItem::new("Reload", cmd(ModCmd::Reload)), "R"),
        key(MenuItem::new(toggle, cmd(ModCmd::Toggle)), "Space"),
        MenuItem::separator(),
        MenuItem::new("Rename...", cmd(ModCmd::Rename)),
        MenuItem::new("Duplicate...", cmd(ModCmd::Duplicate)),
    ];
    let (folder, path) = match &m.folder {
        Some(f) => {
            items.push(MenuItem::new("Delete module...", MenuCmd::Delete(f.clone())).danger());
            (f.clone(), f.clone())
        }
        None => {
            items.push(key(
                MenuItem::new("Make it a module", MenuCmd::MakeModule(m.id.clone())),
                "M",
            ));
            items.push(MenuItem::new("Delete...", MenuCmd::Delete(m.main.clone())).danger());
            (st.s.mods_dir(), m.main.clone())
        }
    };
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Show in folder", MenuCmd::Reveal(folder)));
    items.push(copy_path(&path));
    if matches!(m.state, ModState::Failed(..)) {
        items.push(MenuItem::separator());
        items.push(MenuItem::new("Copy the error", cmd(ModCmd::CopyError)));
    }
    items
}

/// The script list's own menu (and the Library list's), off its rows: a
/// new script, module or file, Reload all, and the mods folder.
fn scripts_items(st: &Studio) -> Vec<MenuItem> {
    let mut items = new_items(st);
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Reload all", MenuCmd::Act(Action::ReloadAll)));
    items.push(MenuItem::new("Open the mods folder", MenuCmd::Act(Action::OpenFolder)));
    items
}

/// An example's menu in the Library: Install it (or Edit it, installed),
/// and Copy its code.
fn example_items(st: &Studio, i: usize) -> Vec<MenuItem> {
    let Some(ex) = EXAMPLES.get(i) else {
        return Vec::new();
    };
    let first = if st.s.mods.iter().any(|m| m.id == ex.id) {
        MenuItem::new("Edit", MenuCmd::Mod(ex.id.to_owned(), ModCmd::Edit))
    } else {
        MenuItem::new("Install", MenuCmd::Act(Action::Install(i)))
    };
    vec![
        first,
        MenuItem::new("Copy its code", MenuCmd::Copy(ex.source.to_owned())),
    ]
}

/// The menu of the editor's file (its title, Synapse's tab): Save, Save as
/// a script (scratch code, a file of no mod), Revert to saved (with unsaved
/// edits), Rename (a mod's file: a module's other file by itself, else the
/// mod), its folder and its path.
fn file_items(st: &Studio) -> Vec<MenuItem> {
    let ed = &st.s.editor;
    let Some(path) = ed.path.clone() else {
        return Vec::new();
    };
    let owner = st.mod_of_path(&path);
    let mut items = vec![MenuItem::new("Save", MenuCmd::Act(Action::Save)).keys("Ctrl S")];
    if owner.is_none() {
        items.push(MenuItem::new("Save as a script...", MenuCmd::Act(Action::SaveFile)));
    }
    items.push(MenuItem::new("Revert to saved", MenuCmd::Revert).enabled(ed.dirty && path.is_file()));
    items.push(MenuItem::separator());
    if let Some(i) = owner {
        let m = &st.s.mods[i];
        let other = m
            .folder
            .as_deref()
            .and_then(|f| rel_in(f, &path))
            .filter(|r| r != "main.lua");
        items.push(match other {
            Some(_) => MenuItem::new("Rename...", MenuCmd::Rename(path.clone())),
            None => MenuItem::new("Rename...", MenuCmd::Mod(m.id.clone(), ModCmd::Rename)),
        });
    }
    if let Some(dir) = path.parent() {
        items.push(MenuItem::new("Show in folder", MenuCmd::Reveal(dir.to_path_buf())));
    }
    items.push(copy_path(&path));
    items
}

/// A mod's command from its menu, the mod found by id now (a scan since
/// the menu opened may have moved it, or taken it away).
fn mod_cmd(st: &mut Studio, game: &mut dyn GameApi, id: &str, cmd: ModCmd) -> UiResult {
    let Some(i) = st.s.mods.iter().position(|m| m.id == id) else {
        notice(st, format!("{id} is gone"));
        return UiResult::Used;
    };
    match cmd {
        ModCmd::Edit => return act(st, game, Action::Edit(i)),
        ModCmd::Reload => return act(st, game, Action::Reload(i)),
        ModCmd::Toggle => return act(st, game, Action::Toggle(i)),
        ModCmd::Rename => open_naming(st, NamingKind::RenameMod(id.to_owned()), id),
        ModCmd::Duplicate => open_naming(st, NamingKind::Duplicate(id.to_owned()), &format!("{id}_copy")),
        ModCmd::CopyError => {
            if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
                let text = if trace.is_empty() {
                    msg.clone()
                } else {
                    format!("{msg}\n{trace}")
                };
                copy_text(st, game, &text);
            }
        }
    }
    UiResult::Used
}

/// The Library's menu of mod `i`, under `at` (its "..."), its row outlined.
pub(crate) fn open_library_menu(st: &mut Studio, i: usize, at: [f32; 4]) {
    let items = library_items(st, i);
    let target =
        st.s.mods
            .get(i)
            .and_then(|m| st.s.ui.drawn_mods.iter().position(|id| *id == m.id))
            .and_then(|k| spot_rect(st, Spot::Mod(k)));
    open(st, [at[0], at[1] + at[3] + 4.0], at[3], items, target);
}

/// The Library's items for mod `i` (its "...", the . key, a right click on
/// its row): a module's files (one chosen opens), New file, and Rename and
/// Delete for the one of its files the editor has open (not main.lua); a
/// script's Edit; then what every mod's menu has, with the Library's keys.
/// In the executor styles, which have no script list, a module's files are
/// reached so.
fn library_items(st: &Studio, i: usize) -> Vec<MenuItem> {
    const SHOWN: usize = 12;
    let Some(m) = st.s.mods.get(i) else {
        return Vec::new();
    };
    let open = st.s.editor.path.clone();
    let mut items = match &m.folder {
        Some(folder) => {
            let files = m.files();
            let mut items: Vec<MenuItem> = files
                .iter()
                .take(SHOWN)
                .map(|rel| {
                    let path = path_in(folder, rel);
                    let on = open.as_ref() == Some(&path);
                    MenuItem::new(rel.clone(), MenuCmd::Open(path)).checked(on)
                })
                .collect();
            if files.len() > SHOWN {
                let more = files.len() - SHOWN;
                items.push(MenuItem::note(format!(
                    "and {more} more: the console's edit(\"{}\", file) opens any",
                    m.id
                )));
            }
            items.push(MenuItem::separator());
            items.push(MenuItem::new("New file...", MenuCmd::NewFile(folder.clone())).keys("F"));
            if let Some(path) = open.filter(|p| p.starts_with(folder))
                && let Some(rel) = rel_in(folder, &path).filter(|r| r != "main.lua")
            {
                items.push(MenuItem::new(format!("Rename {rel}..."), MenuCmd::Rename(path.clone())));
                items.push(MenuItem::new(format!("Delete {rel}..."), MenuCmd::Delete(path)).danger());
            }
            items
        }
        None => vec![MenuItem::new("Edit", MenuCmd::Mod(m.id.clone(), ModCmd::Edit)).keys("⏎")],
    };
    items.push(MenuItem::separator());
    items.extend(mod_items(st, i, true));
    items
}

/// The script list's + menu, under the + (`at`, where it was drawn).
pub(crate) fn open_new_menu(st: &mut Studio, at: [f32; 4]) {
    let items = new_items(st);
    open(st, [at[0], at[1] + at[3] + 4.0], at[3], items, None);
}

/// A new script, a new module, and a new file of the module the editor has open.
fn new_items(st: &Studio) -> Vec<MenuItem> {
    let mut items = vec![
        MenuItem::new("New script...", MenuCmd::NewScript),
        MenuItem::new("New module...", MenuCmd::NewModule),
    ];
    if let Some((i, folder)) = open_module(st) {
        items.push(MenuItem::separator());
        let label = format!("New file in {}...", st.s.mods[i].id);
        items.push(MenuItem::new(label, MenuCmd::NewFile(folder)));
    }
    items
}
