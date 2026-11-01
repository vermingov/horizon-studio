// Right-click menus: the menu of each thing the studio draws, and what their
// items do. look.rs and skins.rs mark where each thing is drawn with a spot
// (`Action::Spot`); a right press opens the menu of the spot under the
// pointer in `Ui::menu`, the widget the agent's model chip and the script
// list's "..." open too. ui.rs routes the keys, the text and the pointer to
// the menu while it is open, and hands the item chosen to `run`. Items name
// mods by id and files by path, never by their place in a list: a scan
// between the menu opening and an item chosen can move them.

use super::hstudio::{GameApi, ModState, Studio};
use super::hstudio_assist::{definition, documented, module_at, name_at};
use super::hstudio_examples::EXAMPLES;
use super::hstudio_look::docs_markdown;
use super::hstudio_modfiles::{path_in, rel_in, require_path};
use super::hstudio_theme::{Layout, THEMES};
use super::hstudio_ui::{
    AREA_AGENT, AREA_LOG, Action, DropAt, EditCmd, FIELD_AGENT, FIELD_CONSOLE, FieldCmd, Menu, MenuCmd, MenuItem,
    ModCmd, NamingKind, Pane, ScriptRow, Side, Spot, Tab, Target, Ui, UiResult, act, agent_ask, agent_busy,
    agent_choose, agent_id, agent_list, agent_model, agent_name, agent_new_conversation, agent_rescan, agent_send,
    code_line, copy_text, dock_changed, editor_pos, editor_siblings, file_in_module, focus_pane, group_name,
    in_text_selection, inside, key_pane, make_module, menu_pick, model_choices, model_items, model_label, notice,
    open_module, open_naming, open_path, pane_bars, run_console_line, selected_text, set_agent_model, show_code,
    spot_at, target,
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

/// The Menu key (or Shift+F10): the menu of what has the keys, where it
/// is: the code's at the cursor (its line's own menu, the cursor left
/// where it is), an input line's under the line, the Library's chosen mod
/// at its row (the list's own menu under its title while it lists none),
/// the Problems' chosen problem at its row (the pane's own menu under its
/// tab while it lists none); on the Editor tab with no pane taking the
/// keys, or the Docs there, the pane's own menu under its tab; else the
/// menu of what is under the pointer. On the Editor tab the menu ends with
/// Layout, that pane's moves, so the keys alone place every pane. Its
/// first item is lit, for the keys. Nothing opens while the left button
/// drags something.
pub fn key_menu(st: &mut Studio, game: &mut dyn GameApi) -> UiResult {
    if st.s.ui.dragging_any() {
        return UiResult::Used;
    }
    st.s.ui.menu = None;
    let ui = &st.s.ui;
    let below = |r: [f32; 4]| ([r[0] + 12.0, r[1] + r[3]], r[3]);
    let pane = key_pane(ui);
    let docked = ui.tab == Tab::Editor && ui.look.theme().layout == Layout::Studio;
    // What ends the menu of a pane with the keys on the Editor tab.
    let more = |items: &mut Vec<MenuItem>| {
        if let Some(p) = pane.filter(|_| docked) {
            items.push(MenuItem::separator());
            items.push(MenuItem::new("Layout", MenuCmd::Layout(p)).keys("›"));
        }
    };
    let opened = match target(ui) {
        Target::Editor => {
            // At the cursor, or the code's top left while it is off the view.
            let at = ui
                .caret_at
                .or_else(|| ui.view.map(|v| [v.x, v.y]))
                .unwrap_or([ui.mouse.0, ui.mouse.1]);
            let ed = &st.s.editor;
            let mut items = code_items(st, (ed.row, ed.col));
            more(&mut items);
            open(st, at, code_line(), items, None)
        }
        Target::Console | Target::Agent => {
            let field = if target(ui) == Target::Agent {
                FIELD_AGENT
            } else {
                FIELD_CONSOLE
            };
            match spot_rect(st, Spot::Field(field)) {
                Some(r) => {
                    let (at, h) = below(r);
                    let mut items = field_items(st, field);
                    more(&mut items);
                    open(st, at, h, items, None)
                }
                None => false,
            }
        }
        Target::Elsewhere if pane == Some(Pane::Library) => {
            let i = ui.library_sel;
            let row =
                st.s.mods
                    .get(i)
                    .and_then(|m| ui.drawn_mods.iter().position(|id| *id == m.id))
                    .and_then(|k| spot_rect(st, Spot::Mod(k)));
            match row {
                Some(r) => {
                    let (at, h) = below(r);
                    let mut items = library_items(st, i);
                    more(&mut items);
                    open(st, at, h, items, Some(r))
                }
                // No mod's row: the list's own menu, under its title.
                None => match spot_rect(st, Spot::Scripts) {
                    Some(r) => open_spot(st, game, Spot::Scripts, r, [r[0] + 12.0, r[1] + 30.0]),
                    None => false,
                },
            }
        }
        Target::Elsewhere if pane == Some(Pane::Problems) => {
            let k = ui.problem_sel;
            match spot_rect(st, Spot::Problem(k)) {
                Some(r) => {
                    let (at, h) = below(r);
                    let mut items = problem_items(st, k);
                    more(&mut items);
                    open(st, at, h, items, Some(r))
                }
                // No problem's row: the pane's own menu, under its tab
                // (the last of its spots drawn).
                None => {
                    let spot = Spot::Pane(Pane::Problems);
                    match spot_rect(st, spot) {
                        Some(r) => open_spot(st, game, spot, r, below(r).0),
                        None => false,
                    }
                }
            }
        }
        // The Docs pane, or no pane taking the keys: the pane in use's own
        // menu, under its tab, which has its layout already.
        Target::Elsewhere if docked => {
            let spot = Spot::Pane(pane.unwrap_or(ui.focus));
            match spot_rect(st, spot) {
                Some(r) => open_spot(st, game, spot, r, below(r).0),
                None => false,
            }
        }
        Target::Elsewhere => {
            let at = ui.mouse;
            match spot_at(ui, at) {
                Some((spot, rect)) => open_spot(st, game, spot, rect, [at.0, at.1]),
                None => false,
            }
        }
    };
    if opened && let Some(menu) = st.s.ui.menu.as_mut() {
        menu.sel = menu.items.iter().position(|i| i.enabled);
    }
    UiResult::Used
}

/// Opens the menu of `spot`, drawn at `rect`, with its top left at `at`
/// (the pointer), after what a right click there does first: the code's
/// cursor goes there unless that is in the selection (and the code takes
/// the keys), a Library row is chosen, a pane's input takes the keys. Text
/// selected in the console or the agent's conversation stays only for a
/// click in it (Ctrl+C would copy it, not what was clicked). False when
/// the spot has no menu, or nothing in it can be chosen now.
fn open_spot(st: &mut Studio, game: &mut dyn GameApi, spot: Spot, rect: [f32; 4], at: [f32; 2]) -> bool {
    let keep_text = matches!(spot, Spot::Text(area) if in_text_selection(&st.s.ui, area, (at[0], at[1])));
    if !keep_text {
        st.s.ui.text_sel = None;
    }
    let (items, target) = match spot {
        Spot::Code => {
            let drawn = st.s.ui.smooth[1];
            let ed = &st.s.editor;
            let mut place = (ed.row, ed.col);
            if let Some((row, col)) = editor_pos(st, game, at[0], at[1], drawn) {
                let ed = &mut st.s.editor;
                let line = &ed.lines[row];
                let byte = line.char_indices().nth(col).map_or(line.len(), |(i, _)| i);
                place = (row, byte);
                if !ed.in_selection(row, byte) {
                    ed.click(row, col, false);
                    // Put under the pointer, the cursor is on screen: the view stays.
                    ed.followed = Some((ed.row, ed.col, ed.revision));
                }
            }
            if st.s.ui.look.theme().layout == Layout::Studio {
                focus_pane(st, Pane::Code);
            }
            (code_items(st, place), None)
        }
        Spot::Text(area) => (text_items(st, area), None),
        Spot::Field(field) => {
            if st.s.ui.tab == Tab::Editor {
                let pane = if field == FIELD_AGENT {
                    Pane::Agent
                } else {
                    Pane::Console
                };
                focus_pane(st, pane);
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
        Spot::Pane(p) => (pane_items(st, p), None),
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

// ----------------------------------------------------------------- the panes, the window, the themes

/// The menu of pane `p` (its tab, its group's strip and buttons, its body
/// where nothing has a menu of its own): the console's Copy all and Clear;
/// the problems' Copy all problems; the agent's Stop, New conversation,
/// Copy the conversation, the agents found (the one in use marked), Look
/// for agents again and its model; the code's file's; the script list's
/// own (and the Library's); the Docs' copy; then where the pane is
/// (`layout_items`).
fn pane_items(st: &mut Studio, p: Pane) -> Vec<MenuItem> {
    let mut items = match p {
        Pane::Console => vec![
            MenuItem::new("Copy all", MenuCmd::CopyAll(AREA_LOG)).enabled(!st.s.log.is_empty()),
            clear_item(st, "Clear"),
        ],
        Pane::Problems => vec![
            MenuItem::new("Copy all problems", MenuCmd::Copy(all_problems(st)))
                .enabled(!st.s.editor.problems.is_empty()),
        ],
        Pane::Code => file_items(st),
        Pane::Scripts | Pane::Library => scripts_items(st),
        Pane::Docs => vec![MenuItem::new("Copy the docs as Markdown", MenuCmd::CopyDocs)],
        Pane::Agent => {
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
    items.extend(layout_items(st, p));
    items
}

/// Where pane `p` is, for the pointer and the keys alike: its group hidden
/// (named for what it hides; never the code's), the Library or the Docs
/// closed, Move (the places it can go), Size... (its bars from the keys),
/// Panes (every pane) and Reset layout (or, after one, its undo).
pub(crate) fn layout_items(st: &Studio, p: Pane) -> Vec<MenuItem> {
    let ui = &st.s.ui;
    let tabs = ui.dock.leaf_tabs(p);
    let mut items = Vec::new();
    if !tabs.contains(&Pane::Code) {
        let hide = MenuCmd::Act(Action::HideGroup(p));
        let item = if tabs.contains(&Pane::Console) {
            MenuItem::new("Hide the panel", hide).keys("Ctrl `")
        } else if tabs.contains(&Pane::Scripts) {
            MenuItem::new("Hide the script list", hide).keys("Ctrl B")
        } else {
            MenuItem::new(format!("Hide {}", group_name(tabs)), hide)
        };
        items.push(item.enabled(ui.dock.shown(p)));
    }
    if p.closable() {
        items.push(MenuItem::new(
            format!("Close {}", p.title()),
            MenuCmd::Act(Action::ClosePane(p)),
        ));
    }
    let moves = move_items(st, p).iter().any(|i| i.enabled);
    items.push(MenuItem::new("Move", MenuCmd::Moves(p)).keys("›").enabled(moves));
    items.push(MenuItem::new("Size...", MenuCmd::Act(Action::SizePane(p))).enabled(!pane_bars(ui, p).is_empty()));
    items.push(MenuItem::new("Panes", MenuCmd::Panes).keys("›"));
    items.push(reset_item(ui));
    items
}

/// Reset layout (greyed at the default), or Undo reset layout after one.
fn reset_item(ui: &Ui) -> MenuItem {
    match ui.dock_undo {
        Some(_) => MenuItem::new("Undo reset layout", MenuCmd::Act(Action::UndoReset)),
        None => MenuItem::new("Reset layout", MenuCmd::Act(Action::ResetLayout)).enabled(!ui.dock.is_default()),
    }
}

/// Every pane of the Editor tab, each marked where its body is drawn at the
/// front of its group (the tab laid out as it was last drawn, so from any
/// tab): choosing one shows it, with the keys.
fn pane_list(st: &Studio) -> Vec<MenuItem> {
    let ui = &st.s.ui;
    let drawn = ui.dock_ws.map(|ws| ui.dock.bodies(ws, ui.focus));
    [Pane::Scripts, Pane::Code, Pane::Console, Pane::Problems, Pane::Agent]
        .into_iter()
        .map(|p| {
            MenuItem::new(p.title(), MenuCmd::Act(Action::ShowPane(p))).checked(drawn.is_some_and(|d| d[p.index()]))
        })
        .collect()
}

/// The places pane `p` can move to: beside the code on any side or among
/// its tabs, among another group's tabs (named by its front pane), along an
/// edge of the tab. Each moves it as a drop there would, and is greyed
/// where that changes nothing or would take a pane's place as the tab is.
fn move_items(st: &Studio, p: Pane) -> Vec<MenuItem> {
    let ui = &st.s.ui;
    let mut places: Vec<(String, DropAt)> = Vec::new();
    if p != Pane::Code {
        for (label, side) in [
            ("Left of the code", Side::Left),
            ("Right of the code", Side::Right),
            ("Above the code", Side::Top),
            ("Below the code", Side::Bottom),
        ] {
            places.push((
                label.into(),
                DropAt::Beside {
                    anchor: Pane::Code,
                    side,
                },
            ));
        }
        let at = ui.dock.leaf_tabs(Pane::Code).iter().filter(|t| **t != p).count();
        places.push(("With the code".into(), DropAt::Into { anchor: Pane::Code, at }));
    }
    for l in ui.dock.leaves() {
        if l.tabs.contains(&Pane::Code) || l.tabs.contains(&p) {
            continue;
        }
        let at = DropAt::Into {
            anchor: l.tabs[0],
            at: l.tabs.len(),
        };
        places.push((format!("With {}", l.active.title()), at));
    }
    for (label, side) in [
        ("Along the left edge", Side::Left),
        ("Along the right edge", Side::Right),
        ("Along the bottom", Side::Bottom),
    ] {
        places.push((label.into(), DropAt::Edge(side)));
    }
    places
        .into_iter()
        .map(|(label, at)| MenuItem::new(label, MenuCmd::Act(Action::MovePane(p, at))).enabled(movable(ui, p, at)))
        .collect()
}

/// Whether moving `p` to `at` changes the dock, and (where the Editor tab
/// was drawn) leaves every pane shown its place and `p` a body, as a drop
/// must.
fn movable(ui: &Ui, p: Pane, at: DropAt) -> bool {
    let mut after = ui.dock.clone();
    if !after.move_pane(p, at) {
        return false;
    }
    let Some(ws) = ui.dock_ws else {
        return true;
    };
    let keep = if p.takes_keys() { p } else { after.settled(ui.focus) };
    super::hstudio_dock::fits(&ui.dock, &after, ws, ui.focus, keep, p)
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

/// A problem's row: Go to it (its place, taken now), Copy, Fix with the
/// agent (in the studio's own layout, with one found), and Copy all problems.
fn problem_items(st: &mut Studio, k: usize) -> Vec<MenuItem> {
    let agent = studio_agent(st);
    let idle = !agent_busy(st);
    let ed = &st.s.editor;
    let Some(p) = ed.problems.get(k) else {
        return Vec::new();
    };
    let row = (p.line.max(1) - 1).min(ed.lines.len() - 1);
    let (start, end) = p.span.unwrap_or((0, 0));
    let mut items = vec![
        MenuItem::new("Go to it", MenuCmd::GoTo(None, row, start, end)).keys("⏎"),
        MenuItem::new("Copy", MenuCmd::Copy(format!("{}:{}: {}", ed.title, p.line, p.message))),
    ];
    if let Some(agent) = agent {
        let ask = format!("Fix the problem on line {}: {}", p.line, p.message);
        items.push(MenuItem::new(format!("Fix with {agent}"), MenuCmd::Ask(Some(ask))).enabled(idle));
    }
    items.push(MenuItem::separator());
    items.push(MenuItem::new("Copy all problems", MenuCmd::Copy(all_problems(st))));
    items
}

/// The window's menu (its title bar, the studio's header): rolling it up,
/// filling the screen and putting it back, full screen or a window (the
/// studio's own layout), the themes, the Editor tab's panes and their
/// default layout (the studio's own layout), Settings and Close.
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
    if look.theme().layout == Layout::Studio {
        items.push(MenuItem::new("Panes", MenuCmd::Panes).keys("›"));
        items.push(reset_item(ui));
    }
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
/// drawn), its current choice lit (else its first), for the keys.
fn replace(st: &mut Studio, place: [f32; 4], items: Vec<MenuItem>) {
    if open(st, [place[0], place[1]], 0.0, items, None)
        && let Some(menu) = st.s.ui.menu.as_mut()
    {
        menu.sel = (menu.items.iter().position(|i| i.checked)).or_else(|| menu.items.iter().position(|i| i.enabled));
    }
}

/// The editor's cursor to a place, selecting it: a row and its bytes from
/// and to, in `path` (opened first) or in the file open; the code takes
/// the keys.
fn go_to(st: &mut Studio, game: &mut dyn GameApi, path: Option<&Path>, row: usize, start: usize, end: usize) {
    if let Some(path) = path {
        open_path(st, game, path);
    }
    st.s.editor.select_span(row, start, end);
    show_code(st);
    st.s.ui.suggest = None;
}

// ----------------------------------------------------------------- the code, the text, the input lines

/// The code's menu, for the place clicked (a row and a byte of it): where
/// the name there is defined, the library or file the string there names
/// and the name's entry in the Docs; the clipboard and the selection; the
/// line commands; in the studio's own layout, with an agent found, asking
/// it, explaining the code and fixing the line's problem; then running
/// (the selection too) and saving.
fn code_items(st: &mut Studio, (row, byte): (usize, usize)) -> Vec<MenuItem> {
    let edit = |label: &str, keys: &str, cmd: EditCmd| MenuItem::new(label, MenuCmd::Edit(cmd)).keys(keys);
    let mut items = name_items(st, row, byte);
    items.push(MenuItem::separator());
    items.extend([
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
    ]);
    if let Some(agent) = studio_agent(st) {
        let idle = !agent_busy(st);
        let ed = &st.s.editor;
        let what = if ed.selection().is_some() {
            "the selection"
        } else {
            "this line"
        };
        items.push(MenuItem::new(format!("Ask {agent}..."), MenuCmd::Ask(None)).enabled(idle));
        items.push(MenuItem::new(format!("Explain {what} with {agent}"), MenuCmd::Explain).enabled(idle));
        if let Some(p) = ed.problems.iter().find(|p| p.line == ed.row + 1) {
            let ask = format!("Fix the problem on line {}: {}", p.line, p.message);
            items.push(MenuItem::new(format!("Fix with {agent}"), MenuCmd::Ask(Some(ask))).enabled(idle));
        }
        items.push(MenuItem::separator());
    }
    if st.s.editor.selection().is_some() {
        items.push(edit("Run the selection", "", EditCmd::RunSelection));
    }
    items.push(MenuItem::new("Run", MenuCmd::Act(Action::Run)).keys("Ctrl ⏎"));
    items.push(MenuItem::new("Save", MenuCmd::Act(Action::Save)).keys("Ctrl S"));
    items
}

/// The agent's name, when one is found and the studio's own layout shows
/// its panel (the executor styles have none).
fn studio_agent(st: &mut Studio) -> Option<String> {
    if st.s.ui.look.theme().layout != Layout::Studio {
        return None;
    }
    agent_name(st)
}

/// What the code's menu offers for the name or string at `byte` of `row`:
/// Go to definition (in the file, or for a module's file in its other
/// files), Open the library an `import` names or the file of the module a
/// `require` names, and Show the function in the Docs.
fn name_items(st: &mut Studio, row: usize, byte: usize) -> Vec<MenuItem> {
    let mut items = Vec::new();
    let Some(line) = st.s.editor.lines.get(row).cloned() else {
        return items;
    };
    match module_at(&line, byte) {
        Some((true, id)) if st.s.mods.iter().any(|m| m.id == id) => {
            items.push(MenuItem::new(format!("Open {id}"), MenuCmd::Mod(id, ModCmd::Edit)));
        }
        Some((false, name)) => {
            if let (Some(rel), Some((_, folder))) = (require_path(&name), open_module(st)) {
                let path = path_in(&folder, &rel);
                if path.is_file() {
                    items.push(MenuItem::new(format!("Open {rel}"), MenuCmd::Open(path)));
                }
            }
        }
        _ => {}
    }
    let Some((start, _, name)) = name_at(&line, byte) else {
        return items;
    };
    // The name, else the table it is in (`kit` for `kit.greet`); each in
    // this file, then in the module's other files.
    let mut names = vec![name.clone()];
    if let Some((head, _)) = name.split_once('.') {
        names.push(head.to_owned());
    }
    let lines = st.s.editor.lines.clone();
    let folder = open_module(st).map(|(_, f)| f);
    let siblings = editor_siblings(st);
    let mut goto = None;
    for n in &names {
        if let Some((r, a, b)) = definition(&lines, n, row, false) {
            // Not when it is the definition clicked.
            if (r, a) != (row, start) {
                goto = Some(MenuCmd::GoTo(None, r, a, b));
            }
            break;
        }
        if let (Some(folder), Some(sib)) = (&folder, &siblings)
            && let Some((rel, (r, a, b))) = sib
                .others
                .iter()
                .find_map(|(rel, other)| Some((rel, definition(other, n, 0, true)?)))
        {
            goto = Some(MenuCmd::GoTo(Some(path_in(folder, rel)), r, a, b));
            break;
        }
    }
    if let Some(cmd) = goto {
        items.push(MenuItem::new("Go to definition", cmd));
    }
    if documented(&name) {
        items.push(MenuItem::new(format!("Show {name} in Docs"), MenuCmd::Docs(name)));
    }
    items
}

/// The Agent pane shown (its group too, the agent at its front) and the
/// agent asked `text`, with what the editor has selected (None: its input
/// takes the keys, for a question typed there; else the keys stay where
/// they are, unless the pane now covers the one they were in).
fn ask_agent(st: &mut Studio, text: Option<&str>) {
    let now = st.s.now;
    let ui = &mut st.s.ui;
    ui.tab = Tab::Editor;
    if !ui.dock.is_front(Pane::Agent) {
        ui.shown_at[Pane::Agent.index()] = now;
    }
    ui.dock.show(Pane::Agent);
    match text {
        None => focus_pane(st, Pane::Agent),
        Some(text) => {
            dock_changed(st);
            agent_ask(st, text);
        }
    }
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
        clear_item(st, "Clear"),
        MenuItem::new("Jump to the newest", MenuCmd::LogEnd).enabled(st.s.ui.log_scroll > 0),
    ]
}

/// The item that clears the console's lines, called `label`. Its hint is
/// Ctrl+L only while the console's input has the keys, the one place that
/// key clears them.
fn clear_item(st: &Studio, label: &str) -> MenuItem {
    let item = MenuItem::new(label, MenuCmd::Act(Action::ClearLog)).enabled(!st.s.log.is_empty());
    if target(&st.s.ui) == Target::Console {
        item.keys("Ctrl L")
    } else {
        item
    }
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
        clear_item(st, "Clear the console"),
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
        EditCmd::RunSelection => st.run_selection(game),
    }
    st.s.ui.suggest = None;
    show_code(st);
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
        MenuCmd::Themes => {
            let items = theme_items(st);
            replace(st, place, items);
        }
        MenuCmd::Models => {
            let items = model_items(st);
            replace(st, place, items);
        }
        MenuCmd::Panes => {
            let items = pane_list(st);
            replace(st, place, items);
        }
        MenuCmd::Moves(p) => {
            let items = move_items(st, p);
            replace(st, place, items);
        }
        MenuCmd::Layout(p) => {
            let items = layout_items(st, p);
            replace(st, place, items);
        }
        MenuCmd::CopyDocs => copy_text(st, game, &docs_markdown()),
        MenuCmd::GoTo(path, row, start, end) => go_to(st, game, path.as_deref(), row, start, end),
        MenuCmd::Docs(name) => {
            st.s.ui.tab = Tab::Docs;
            st.s.ui.docs_find = Some(name);
        }
        MenuCmd::Ask(text) => ask_agent(st, text.as_deref()),
        MenuCmd::Explain => {
            if st.s.editor.selection().is_none() {
                st.s.editor.select_line();
            }
            ask_agent(st, Some("Explain what this code does."));
        }
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
fn row_items(st: &mut Studio, k: usize) -> Vec<MenuItem> {
    let Some(row) = st.s.ui.script_rows.get(k).cloned() else {
        return Vec::new();
    };
    let agent = fixer(st);
    let st = &*st;
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
            items.extend(mod_items(st, i, false, agent.as_ref()));
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
            items.extend(mod_items(st, i, false, agent.as_ref()));
            items
        }
    }
}

/// Copy path: `path` to the clipboard, as the system writes it.
fn copy_path(path: &Path) -> MenuItem {
    MenuItem::new("Copy path", MenuCmd::Copy(path.display().to_string()))
}

/// The agent that can fix a mod from its menu, and whether it is free to:
/// in the studio's own layout, with one found.
fn fixer(st: &mut Studio) -> Option<(String, bool)> {
    let agent = studio_agent(st)?;
    Some((agent, !agent_busy(st)))
}

/// What every mod's menu has, after what its list puts first: Reload (not
/// for a mod turned off, which Turn on starts), Turn off or on, Rename,
/// Duplicate, Make it a module (a script), Delete, its folder and its path,
/// and for a mod that failed Copy the error and Fix with the agent
/// (`agent`: its name and whether it is free). With `keys`, the Library's
/// keys for them.
fn mod_items(st: &Studio, i: usize, keys: bool, agent: Option<&(String, bool)>) -> Vec<MenuItem> {
    let Some(m) = st.s.mods.get(i) else {
        return Vec::new();
    };
    let key = |it: MenuItem, k: &str| if keys { it.keys(k) } else { it };
    let cmd = |c: ModCmd| MenuCmd::Mod(m.id.clone(), c);
    let toggle = if m.enabled { "Turn off" } else { "Turn on" };
    let mut items = vec![
        key(MenuItem::new("Reload", cmd(ModCmd::Reload)).enabled(m.enabled), "R"),
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
        if let Some((name, idle)) = agent {
            items.push(MenuItem::new(format!("Fix with {name}"), cmd(ModCmd::Fix)).enabled(*idle));
        }
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
        // Not one turned off since the menu opened: a reload turns it on.
        ModCmd::Reload if st.s.mods[i].enabled => return act(st, game, Action::Reload(i)),
        ModCmd::Reload => {}
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
        ModCmd::Fix => {
            let ModState::Failed(msg, _) = st.s.mods[i].state.clone() else {
                return UiResult::Used;
            };
            // Its file open where it failed, so the agent sees that file.
            let m = &st.s.mods[i];
            let path = match (st.failure_place(i), &m.folder) {
                (Some((rel, _)), Some(folder)) => path_in(folder, &rel),
                _ => m.main.clone(),
            };
            open_path(st, game, &path);
            ask_agent(st, Some(&format!("{id} fails: {msg}. Find out why and fix it.")));
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
/// script's Edit; then what every mod's menu has, with the Library's keys,
/// and last New mod (its N key). In the executor styles, which have no
/// script list, a module's files are reached so.
fn library_items(st: &mut Studio, i: usize) -> Vec<MenuItem> {
    const SHOWN: usize = 12;
    let agent = fixer(st);
    let st = &*st;
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
    items.extend(mod_items(st, i, true, agent.as_ref()));
    items.push(MenuItem::separator());
    items.push(MenuItem::new("New mod...", MenuCmd::Act(Action::NewMod)).keys("N"));
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
