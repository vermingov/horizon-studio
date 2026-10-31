// Right-click menus: the menu of each thing the studio draws, and what their
// items do. look.rs and skins.rs mark where each thing is drawn with a spot
// (`Action::Spot`); a right press opens the menu of the spot under the
// pointer in `Ui::menu`, the widget the agent's model chip and the script
// list's "..." open too. ui.rs routes the keys, the text and the pointer to
// the menu while it is open, and hands the item chosen to `run`. Items name
// mods by id and files by path, never by their place in a list: a scan
// between the menu opening and an item chosen can move them.

use super::hstudio::{GameApi, Studio};
use super::hstudio_modfiles::{path_in, rel_in};
use super::hstudio_ui::{
    Action, Menu, MenuCmd, MenuItem, NamingKind, ScriptRow, Spot, UiResult, agent_model, file_in_module, inside,
    make_module, menu_pick, open_module, open_naming, open_path, set_agent_model, spot_at,
};

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
        let _ = game;
        open_spot(st, spot, rect, [at.0, at.1]);
    }
    UiResult::Used
}

/// Opens the menu of `spot`, drawn at `rect`, with its top left at `at`.
/// False when it has none (or nothing in it can be chosen now).
fn open_spot(st: &mut Studio, spot: Spot, rect: [f32; 4], at: [f32; 2]) -> bool {
    let items = match spot {
        Spot::Row(k) => row_items(st, usize::from(k)),
        Spot::Mod(k) => {
            let Some(i) = drawn_mod(st, k) else { return false };
            st.s.ui.library_sel = i;
            library_items(st, i)
        }
        _ => return false,
    };
    open(st, at, 0.0, items, Some(rect))
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

/// The items of row `k` of the script list: a module's (a new file, its
/// main.lua, its folder, delete it), a file's (open, rename, delete: not
/// main.lua, which starts the module) or a script's (make it a module,
/// delete it). The scratch file has none.
fn row_items(st: &Studio, k: usize) -> Vec<MenuItem> {
    let Some(row) = st.s.ui.script_rows.get(k).cloned() else {
        return Vec::new();
    };
    match row {
        ScriptRow::Scratch => Vec::new(),
        ScriptRow::Module { id, .. } => {
            let Some(folder) = st.s.mods.iter().find(|m| m.id == id).and_then(|m| m.folder.clone()) else {
                return Vec::new();
            };
            vec![
                MenuItem::new("New file...", MenuCmd::NewFile(folder.clone())),
                MenuItem::new("Open main.lua", MenuCmd::Open(folder.join("main.lua"))),
                MenuItem::new("Show in folder", MenuCmd::Reveal(folder.clone())),
                MenuItem::new("Delete module...", MenuCmd::Delete(folder)).danger(),
            ]
        }
        ScriptRow::File { path, rel, .. } => {
            let main = rel == "main.lua";
            vec![
                MenuItem::new("Open", MenuCmd::Open(path.clone())),
                MenuItem::new("Rename...", MenuCmd::Rename(path.clone())).enabled(!main),
                MenuItem::new("Delete...", MenuCmd::Delete(path))
                    .enabled(!main)
                    .danger(),
            ]
        }
        ScriptRow::Script { path, .. } => {
            let Some(id) = st.mod_of_path(&path).map(|i| st.s.mods[i].id.clone()) else {
                return Vec::new();
            };
            vec![
                MenuItem::new("Make it a module", MenuCmd::MakeModule(id)),
                MenuItem::new("Delete...", MenuCmd::Delete(path)).danger(),
            ]
        }
    }
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

/// The Library's items for mod `i`: a module's files (one chosen opens), a
/// new file, renaming and deleting the one of its files the editor has open
/// (not main.lua), its folder, and deleting it; a script's Make it a module
/// and Delete. In the executor styles, which have no script list, a
/// module's files are reached so.
fn library_items(st: &Studio, i: usize) -> Vec<MenuItem> {
    const SHOWN: usize = 12;
    let Some(m) = st.s.mods.get(i) else {
        return Vec::new();
    };
    let open = st.s.editor.path.clone();
    match &m.folder {
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
            items.push(MenuItem::new("New file...", MenuCmd::NewFile(folder.clone())));
            if let Some(path) = open.filter(|p| p.starts_with(folder))
                && let Some(rel) = rel_in(folder, &path).filter(|r| r != "main.lua")
            {
                items.push(MenuItem::new(format!("Rename {rel}..."), MenuCmd::Rename(path.clone())));
                items.push(MenuItem::new(format!("Delete {rel}..."), MenuCmd::Delete(path)).danger());
            }
            items.push(MenuItem::new("Show in folder", MenuCmd::Reveal(folder.clone())));
            items.push(MenuItem::new("Delete module...", MenuCmd::Delete(folder.clone())).danger());
            items
        }
        None => vec![
            MenuItem::new("Make it a module", MenuCmd::MakeModule(m.id.clone())),
            MenuItem::new("Delete...", MenuCmd::Delete(m.main.clone())).danger(),
        ],
    }
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
