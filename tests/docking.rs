// The Editor tab's panes on the studio's screen: where they are drawn,
// sizing them with their bars and from the keys, moving them by dragging
// their tabs (the preview of where they land, and what cancels a drag)
// and from their menus, what has the keys (and that it is always a pane
// that is drawn), hiding and showing their groups, the layout kept in
// layout.txt, the coach's and the agents' ways with them, small windows,
// and each pane's body kept to its place (FakeGame records the game's
// clips).

mod common;

use common::{FakeGame, mod_index, studio};
use horizon_studio::game::hstudio::{LogKind, Studio};
use horizon_studio::game::hstudio_coach::{self as coach, By};
use horizon_studio::game::hstudio_theme::{Layout, Look, THEMES};
use horizon_studio::game::hstudio_ui::{
    Bar, Dock, DrawnLeaf, DropAt, Input, Key, Mods, Pane, Side, Tab, UiResult, draw, handle, settle_focus,
};
use serde_json::json;

const NONE: Mods = Mods {
    ctrl: false,
    shift: false,
    alt: false,
};
const CTRL: Mods = Mods {
    ctrl: true,
    shift: false,
    alt: false,
};
const SHIFT: Mods = Mods {
    ctrl: false,
    shift: true,
    alt: false,
};

/// The strip's tip, until a tab or a bar is first moved.
const TIP: &str = "Drag a tab to move it · right-click a tab for more";

fn key(k: Key, mods: Mods) -> Input {
    Input::Key {
        key: k,
        down: true,
        repeat: false,
        mods,
    }
}

fn press(st: &mut Studio, game: &mut FakeGame, k: Key, mods: Mods) -> UiResult {
    handle(st, game, key(k, mods))
}

fn redraw(st: &mut Studio, game: &mut FakeGame) {
    game.drawn.clear();
    draw(st, game);
}

fn type_text(st: &mut Studio, game: &mut FakeGame, text: &str) {
    for c in text.chars() {
        handle(st, game, Input::Text(c.to_string()));
    }
}

/// Every text drawn, with where it starts.
fn texts(game: &FakeGame) -> Vec<(String, (f32, f32))> {
    game.drawn
        .iter()
        .filter_map(|d| {
            let rest = d.strip_prefix("text ")?;
            let (text, at) = rest.rsplit_once(" @[")?;
            let n: Vec<f32> = at.split(']').next()?.split(", ").map(|v| v.parse().unwrap()).collect();
            Some((text.to_owned(), (n[0], n[1])))
        })
        .collect()
}

/// Where `text` was drawn: the first time, or the last.
fn drawn_at(game: &FakeGame, text: &str, last: bool) -> (f32, f32) {
    let all = texts(game);
    let mut found = all.iter().filter(|(t, _)| t == text);
    let hit = if last { found.next_back() } else { found.next() };
    hit.unwrap_or_else(|| panic!("{text:?} not drawn")).1
}

fn text_at(game: &FakeGame, text: &str) -> (f32, f32) {
    drawn_at(game, text, false)
}

fn last_text_at(game: &FakeGame, text: &str) -> (f32, f32) {
    drawn_at(game, text, true)
}

fn drawn(game: &FakeGame, text: &str) -> bool {
    texts(game).iter().any(|(t, _)| t == text)
}

/// The first record of `text` drawn, by its place among the records.
fn drawn_index(game: &FakeGame, text: &str) -> usize {
    let prefix = format!("text {text} @[");
    game.drawn
        .iter()
        .position(|d| d.starts_with(&prefix))
        .unwrap_or_else(|| panic!("{text:?} not drawn"))
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

fn click(st: &mut Studio, game: &mut FakeGame, (x, y): (f32, f32)) {
    handle(st, game, Input::Mouse(x, y));
    handle(st, game, Input::Button { left: true, down: true });
    handle(
        st,
        game,
        Input::Button {
            left: true,
            down: false,
        },
    );
}

fn right_click(st: &mut Studio, game: &mut FakeGame, (x, y): (f32, f32)) {
    handle(st, game, Input::Mouse(x, y));
    handle(
        st,
        game,
        Input::Button {
            left: false,
            down: true,
        },
    );
    handle(
        st,
        game,
        Input::Button {
            left: false,
            down: false,
        },
    );
}

/// The left button pressed at `from`, moved to `to` and let go.
fn drag(st: &mut Studio, game: &mut FakeGame, from: (f32, f32), to: (f32, f32)) {
    handle(st, game, Input::Mouse(from.0, from.1));
    handle(st, game, Input::Button { left: true, down: true });
    handle(st, game, Input::Mouse(to.0, to.1));
    handle(
        st,
        game,
        Input::Button {
            left: true,
            down: false,
        },
    );
}

/// The pointer moved to `at`.
fn point(st: &mut Studio, game: &mut FakeGame, (x, y): (f32, f32)) {
    handle(st, game, Input::Mouse(x, y));
}

/// The left button pressed at `from`, then the pointer moved through each
/// of `to`, the button still held.
fn hold(st: &mut Studio, game: &mut FakeGame, from: (f32, f32), to: &[(f32, f32)]) {
    point(st, game, from);
    handle(st, game, Input::Button { left: true, down: true });
    for &at in to {
        point(st, game, at);
    }
}

/// The left button let go.
fn release(st: &mut Studio, game: &mut FakeGame) {
    handle(
        st,
        game,
        Input::Button {
            left: true,
            down: false,
        },
    );
}

/// The colours of what was drawn at `r` (dp) as `kind` ("rect" or
/// "border"), in the order drawn.
fn colours(game: &FakeGame, kind: &str, r: [f32; 4]) -> Vec<u32> {
    let prefix = format!("{kind} {r:?} r");
    game.drawn
        .iter()
        .filter_map(|d| d.strip_prefix(&prefix)?.rsplit(' ').next())
        .map(|c| u32::from_str_radix(c, 16).unwrap())
        .collect()
}

/// `c` with `a` of its alpha, as the studio fades a colour.
fn faded(c: u32, a: f32) -> u32 {
    let alpha = ((c >> 24) as f32 * a).round() as u32;
    (c & 0x00FF_FFFF) | alpha << 24
}

/// The game's clips as recorded: where each record is among the drawing,
/// the mark it cuts from, and its rect (dp).
fn clips(game: &FakeGame) -> Vec<(usize, usize, [f32; 4])> {
    game.drawn
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let rest = d.strip_prefix("clip ")?;
            let (mark, rect) = rest.split_once(' ')?;
            let n: Vec<f32> = rect
                .trim_matches(['[', ']'])
                .split(", ")
                .map(|v| v.parse().unwrap())
                .collect();
            Some((i, mark.parse().unwrap(), [n[0], n[1], n[2], n[3]]))
        })
        .collect()
}

/// The open menu's labels (a separator as "").
fn menu_items(st: &Studio) -> Vec<String> {
    let menu = st.s.ui.menu.as_ref().expect("a menu is open");
    menu.items.iter().map(|i| i.label.clone()).collect()
}

/// The open menu's items that can be chosen.
fn usable(st: &Studio) -> Vec<String> {
    let menu = st.s.ui.menu.as_ref().expect("a menu is open");
    menu.items
        .iter()
        .filter(|i| i.enabled && !i.separator)
        .map(|i| i.label.clone())
        .collect()
}

fn item(st: &Studio, label: &str) -> horizon_studio::game::hstudio_ui::MenuItem {
    let menu = st.s.ui.menu.as_ref().expect("a menu is open");
    menu.items
        .iter()
        .find(|i| i.label == label)
        .unwrap_or_else(|| panic!("no {label:?} in {:?}", menu_items(st)))
        .clone()
}

/// The item lit for the keys.
fn lit(st: &Studio) -> String {
    let menu = st.s.ui.menu.as_ref().expect("a menu is open");
    menu.items[menu.sel.expect("an item lit")].label.clone()
}

/// The open menu's item `label` clicked, as drawn.
fn choose(st: &mut Studio, game: &mut FakeGame, label: &str) {
    redraw(st, game);
    let at = last_text_at(game, label);
    click(st, game, (at.0 + 2.0, at.1 + 4.0));
}

/// The open menu's item `label` chosen with the keys: Down until it is lit,
/// then Enter.
fn choose_by_keys(st: &mut Studio, game: &mut FakeGame, label: &str) {
    for _ in 0..40 {
        if st.s.ui.menu.as_ref().and_then(|m| m.sel).is_some() && lit(st) == label {
            press(st, game, Key::Enter, NONE);
            return;
        }
        press(st, game, Key::Down, NONE);
    }
    panic!("{label:?} never lit in {:?}", menu_items(st));
}

/// What layout.txt holds (empty: no file).
fn layout_txt(st: &Studio) -> String {
    std::fs::read_to_string(st.s.dir.join("layout.txt")).unwrap_or_default()
}

/// The group holding `p` as the last frame drew it.
fn leaf(st: &Studio, p: Pane) -> DrawnLeaf {
    st.s.ui
        .dock_drawn
        .iter()
        .find(|l| l.tabs.iter().any(|(t, _)| *t == p))
        .unwrap_or_else(|| panic!("{p:?} not drawn: {:?}", st.s.ui.dock_drawn))
        .clone()
}

/// Where `p`'s tab was drawn last frame.
fn tab_of(st: &Studio, p: Pane) -> [f32; 4] {
    let l = leaf(st, p);
    l.tabs.iter().find(|(t, _)| *t == p).unwrap().1
}

fn middle(r: [f32; 4]) -> (f32, f32) {
    (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
}

/// The bar of the split at `path` as the last frame laid it out.
fn bar(st: &Studio, path: u16) -> Bar {
    *st.s
        .ui
        .dock_bars
        .iter()
        .find(|b| b.path == path)
        .unwrap_or_else(|| panic!("no bar {path}: {:?}", st.s.ui.dock_bars))
}

/// Where the × that hides `p`'s group is, on its strip.
fn cross_of(st: &Studio, p: Pane) -> (f32, f32) {
    let s = leaf(st, p).strip;
    (s[0] + s[2] - 21.0, s[1] + 17.0)
}

fn notice(st: &Studio) -> String {
    st.s.ui.notice.as_ref().map(|n| n.0.clone()).unwrap_or_default()
}

/// A studio open on its Editor tab, full screen, nothing moving.
fn editing(name: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    // No agent found, whatever this computer has.
    st.s.ui.agent.agents = Some(Vec::new());
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    (game, st)
}

fn agent() -> By {
    By {
        name: "Claude Code".into(),
        conn: 1,
    }
}

/// A stand-in agent: it says back what it was asked.
#[cfg(unix)]
fn echo() -> horizon_studio::game::hstudio_agent::AgentSpec {
    horizon_studio::game::hstudio_agent::AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }
}

fn synapse() -> usize {
    THEMES.iter().position(|t| t.layout == Layout::Synapse).unwrap()
}

#[test]
fn page_keys_move_by_the_panes_own_page() {
    let (mut game, mut st) = editing("dock-pages");
    let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    for i in 0..60 {
        st.s.log(LogKind::Info, format!("line {i}"));
    }
    redraw(&mut st, &mut game);
    // Full screen the code shows 9 rows: its group's 240 less its strip's
    // 32 and the body's unit, less its margins' 24, in rows of 20.
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.editor.row, 9);
    // The panel's console shows 4 lines: the panel's 160 less its strip,
    // its insets and its input line, in lines of 20. Its keys page by
    // those, not by the code's rows.
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert_eq!(st.s.ui.log_scroll, 4, "the console's lines, not the code's 9");
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert_eq!(st.s.ui.log_scroll, 8);
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.ui.log_scroll, 4);
    // Back on the code, its page is still its own.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.editor.row, 18);
    assert_eq!(st.s.ui.log_scroll, 4);
    // The Console tab's console is the large one: its keys page by its own.
    handle(&mut st, &mut game, key(Key::Char('1'), CTRL));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert!(st.s.ui.log_scroll > 8, "{}", st.s.ui.log_scroll);
}

#[test]
fn problems_are_found_wherever_the_code_is() {
    let (mut game, mut st) = editing("dock-problems");
    st.s.editor.set_text("print(player.poss())\n");
    st.s.editor.click(1, 0, false);
    type_text(&mut st, &mut game, "-- a note");
    // Typed on the Editor tab, then off to the Library before the check's
    // pause is over: it looks there, once typing has paused.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    assert_eq!(st.s.ui.tab, Tab::Library);
    redraw(&mut st, &mut game);
    assert!(st.s.editor.problems.is_empty(), "not while typing");
    st.s.now += 1.1;
    redraw(&mut st, &mut game);
    let found: Vec<&str> = st.s.editor.problems.iter().map(|p| p.message.as_str()).collect();
    assert_eq!(found, ["player has no 'poss'"]);
    // And it is the Editor tab's when it comes back, its line marked.
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    assert!(game.drawn.iter().any(|d| d.contains("text player has no 'poss'")));
    // The script list over the code in its group: the code isn't drawn, and
    // its problems are still found.
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    st.s.editor.set_text("print(player.fly())\n");
    redraw(&mut st, &mut game);
    assert!(!st.s.ui.dock_front[Pane::Code.index()] && st.s.ui.coach.code_at.is_none());
    let found: Vec<&str> = st.s.editor.problems.iter().map(|p| p.message.as_str()).collect();
    assert_eq!(found, ["player has no 'fly'"]);
    // The code only its strip, the console in use in an 854 x 480 game's
    // window: still found.
    st.s.ui.dock = Dock::default();
    st.s.ui.focus = Pane::Console;
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    st.s.editor.set_text("print(player.swim())\n");
    redraw(&mut st, &mut game);
    assert!(leaf(&st, Pane::Code).collapsed, "{:?}", st.s.ui.dock_drawn);
    let found: Vec<&str> = st.s.editor.problems.iter().map(|p| p.message.as_str()).collect();
    assert_eq!(found, ["player has no 'swim'"]);
}

#[test]
fn pane_bodies_are_clipped_to_their_place() {
    let (mut game, mut st) = editing("dock-clips");
    st.s.editor.set_text(&"local a = 1\n".repeat(8));
    redraw(&mut st, &mut game);
    // The script list, the panel and the code, each under its strip, in the
    // order they are drawn (the code, which has the keys, last).
    let code = st.s.ui.coach.code_at.expect("the code is drawn");
    assert_eq!(code, [211.0, 112.0, 700.0, 207.0]);
    let found = clips(&game);
    let (list, panel, code_clip) = (
        [48.0, 112.0, 154.0, 376.0],
        [210.0, 360.0, 702.0, 128.0],
        [210.0, 112.0, 702.0, 208.0],
    );
    assert_eq!(found.iter().map(|c| c.2).collect::<Vec<_>>(), [list, panel, code_clip]);
    // Each is its group's rect under its strip.
    for l in &st.s.ui.dock_drawn {
        let under = [l.rect[0], l.rect[1] + 32.0, l.rect[2], l.rect[3] - 32.0];
        assert!(found.iter().any(|c| c.2 == under), "{l:?}");
    }
    // Each cuts what its pane drew since its mark: the script list's row,
    // the panel's input line, the code's text.
    for ((at, mark, _), text) in found.iter().zip(["scratch.lua", "Click to run Lua here.", "local"]) {
        let i = game
            .drawn
            .iter()
            .enumerate()
            .skip(*mark)
            .position(|(_, d)| d.starts_with(&format!("text {text} @")))
            .map(|k| k + mark)
            .unwrap_or_else(|| panic!("{text:?} not drawn after mark {mark}"));
        assert!(i < *at, "{text:?} at {i}, its clip at {at}");
    }
    // The code's suggestions are drawn after its clip, so neither they nor
    // their clicks are cut: under the code's last row the list runs over the
    // panel, and a click there, past the code, takes one.
    st.s.editor.click(8, 0, false);
    type_text(&mut st, &mut game, "player.fa");
    assert!(st.s.ui.suggest.is_some());
    redraw(&mut st, &mut game);
    let code_at = clips(&game)
        .iter()
        .find(|c| c.2 == code_clip)
        .expect("the code's clip")
        .0;
    assert!(
        drawn_index(&game, "cing") > code_at,
        "the suggestion drawn after the clip"
    );
    let at = text_at(&game, "cing");
    let past = (at.0 + 2.0, at.1 + 12.0);
    assert!(past.1 > code[1] + code[3], "{past:?} is past the code");
    click(&mut st, &mut game, past);
    assert_eq!(st.s.editor.lines[8], "player.facing()");
    // The panel's console's, over the code, after the panel's clip.
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    type_text(&mut st, &mut game, "pla");
    assert!(st.s.ui.suggest.as_ref().is_some_and(|s| s.console));
    redraw(&mut st, &mut game);
    let panel_at = clips(&game).iter().find(|c| c.2 == panel).expect("the panel's clip").0;
    assert!(
        drawn_index(&game, "yer") > panel_at,
        "the console's suggestion after the clip"
    );
    // A group that is only its strip has no body, so no clip: the panel in
    // an 854 x 480 game's window, under the code.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    redraw(&mut st, &mut game);
    let panel = leaf(&st, Pane::Console);
    assert!(panel.collapsed);
    let found = clips(&game);
    assert_eq!(found.len(), 2, "the script list and the code: {found:?}");
    assert!(found.iter().all(|c| c.2[1] < panel.rect[1] * 1.125), "{found:?}");
    // The executor styles' layouts have no panes: nothing of theirs is clipped.
    st.s.ui.look = Look {
        theme: synapse(),
        motion: false,
        ..Look::default()
    };
    redraw(&mut st, &mut game);
    assert!(game.drawn.len() > 20);
    assert!(clips(&game).is_empty(), "{:?}", clips(&game));
    // A Library pane in a group shorter than it is (in use in a 330 x 230
    // window): its pills are drawn past the group, and their clicks are
    // cut with them, so a click where New mod was drawn opens no prompt.
    let (mut game, mut st) = editing("dock-clips-library");
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 330.0, 230.0]);
    st.s.ui.dock.show(Pane::Library);
    st.s.ui.focus = Pane::Library;
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    let library = leaf(&st, Pane::Library);
    assert!(!library.collapsed);
    let under = [
        library.rect[0],
        library.rect[1] + 32.0,
        library.rect[2],
        library.rect[3] - 32.0,
    ];
    assert!(clips(&game).iter().any(|c| c.2 == under), "{:?}", clips(&game));
    let new = text_at(&game, "New mod");
    let at = (new.0 + 2.0, new.1 + 4.0);
    assert!(!inside(library.rect, at), "{at:?} is past {library:?}");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.naming.is_none(), "no New prompt");
}

#[test]
fn the_codes_scroll_bar_scrolls_only_while_it_is_drawn() {
    let (mut game, mut st) = editing("dock-scrollbar");
    let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    // Its thumb is at the top of the track down the code's right.
    let code = st.s.ui.coach.code_at.expect("the code is drawn");
    let thumb = (code[0] + code[2] - 8.0, code[1] + 16.0);
    handle(&mut st, &mut game, Input::Mouse(thumb.0, thumb.1));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(thumb.0, thumb.1 + 40.0));
    let scrolled = st.s.editor.scroll;
    assert!(scrolled > 0, "held, it scrolls the code");
    // Still held on the Library tab, where the code isn't drawn, it
    // scrolls nothing.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(thumb.0, thumb.1 + 120.0));
    assert_eq!(st.s.editor.scroll, scrolled);
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
}

#[test]
fn a_note_not_drawn_keeps_no_place_for_esc() {
    let mut game = FakeGame::new("dock-note");
    let mut st = studio(&mut game);
    game.write_mod("sum.lua", "local a = 1\nlocal b = 2\nprint(a + b)\n");
    st.scan(&mut game);
    st.edit_mod(mod_index(&st, "sum"));
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    st.s.ui.look.window = true;
    redraw(&mut st, &mut game);
    coach::call(
        &mut st,
        &mut game,
        &agent(),
        "mark",
        &json!({ "start_line": 2, "note": "Two." }),
    )
    .unwrap();
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    assert!(st.s.ui.coach.bubble_at.is_some(), "its note shows");
    // Rolled up to its title bar, the window draws no code: the note has no
    // place, so Esc closes the studio rather than a note no one sees.
    st.s.ui.collapsed = true;
    redraw(&mut st, &mut game);
    assert_eq!((st.s.ui.coach.code_at, st.s.ui.coach.bubble_at), (None, None));
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Close);
    assert!(
        st.s.ui.coach.open_mark.is_some(),
        "the note stays open for when the code shows"
    );
}

#[test]
fn the_editor_tab_draws_its_panes_where_they_were() {
    let (mut game, mut st) = editing("dock-default");
    redraw(&mut st, &mut game);
    for label in ["Scripts", "scratch.lua", "Console", "Problems", "Agent", "Save", "Run"] {
        text_at(&game, label);
    }
    // No header over the panes: the code's group starts at the Editor
    // area's top, its tab the file's name.
    let code = leaf(&st, Pane::Code);
    assert_eq!(code.rect[1], 80.0);
    assert!(inside(code.strip, last_text_at(&game, "scratch.lua")));
    assert_eq!(st.s.ui.dock_bars.len(), 2);
    let drawn: Vec<(Pane, [f32; 4], bool)> =
        st.s.ui
            .dock_drawn
            .iter()
            .map(|l| (l.active, l.rect, l.collapsed))
            .collect();
    assert_eq!(
        drawn,
        [
            (Pane::Scripts, [48.0, 80.0, 154.0, 408.0], false),
            (Pane::Console, [210.0, 328.0, 702.0, 160.0], false),
            (Pane::Code, [210.0, 80.0, 702.0, 240.0], false),
        ]
    );
    assert_eq!(st.s.ui.dock, Dock::default());
    assert_eq!(st.s.ui.focus, Pane::Code);
    // The panel's strip says tabs move, until one does.
    assert!(inside(leaf(&st, Pane::Console).strip, text_at(&game, TIP)));
    assert!(!st.s.dir.join("layout.txt").exists(), "drawing writes nothing");
}

#[test]
fn the_bars_size_the_panes_and_are_kept() {
    let (mut game, mut st) = editing("dock-bars");
    redraw(&mut st, &mut game);
    // The bar between the code and the panel, held and moved up 60.
    let col = bar(&st, 3);
    assert!(!col.row && (col.lo, col.hi) == (220.0, 360.0), "{col:?}");
    let at = (col.rect[0] + 300.0, col.rect[1] + 4.0);
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert!(st.s.ui.dragging_any());
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1 - 60.0));
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Code).rect, [210.0, 80.0, 702.0, 180.0]);
    assert!(layout_txt(&st).is_empty(), "nothing written while it is held");
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let kept = layout_txt(&st);
    assert!(kept.contains("col:0.450(") && kept.contains("tips=0"), "{kept}");
    redraw(&mut st, &mut game);
    assert!(!drawn(&game, TIP), "the tip goes once a bar was moved");
    // Far down it stops with the console's group at its least, 120 high.
    let col = bar(&st, 3);
    let at = (col.rect[0] + 300.0, col.rect[1] + 4.0);
    drag(&mut st, &mut game, at, (at.0, 2000.0));
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Console).rect[3], 120.0);
    // The bar at the script list's right widens it.
    let row = bar(&st, 1);
    let at = (row.rect[0] + 4.0, row.rect[1] + 100.0);
    drag(&mut st, &mut game, at, (at.0 + 40.0, at.1));
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Scripts).rect[2], 194.0);
    // A studio started again on the same folder lays out the same, its tip gone.
    let rects: Vec<[f32; 4]> = st.s.ui.dock_drawn.iter().map(|l| l.rect).collect();
    let mut again = studio(&mut game);
    again.s.ui.open = true;
    again.s.ui.look.motion = false;
    again.s.ui.tab = Tab::Editor;
    redraw(&mut again, &mut game);
    assert_eq!(again.s.ui.dock, st.s.ui.dock);
    assert!(!again.s.ui.dock_tips && !drawn(&game, TIP));
    let again_rects: Vec<[f32; 4]> = again.s.ui.dock_drawn.iter().map(|l| l.rect).collect();
    assert_eq!(again_rects, rects);
    // An Editor area 275 high: the code's 140 and the panel's 120 leave the
    // bar 7 units to move in, so it has no grip, and a press on it holds
    // nothing.
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 748.0, 416.0]);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws.unwrap()[3], 275.0);
    let col = bar(&st, 3);
    assert!(col.hi - col.lo < 16.0, "{col:?}");
    let ratio = st.s.ui.dock.ratio(3);
    let at = middle(col.rect);
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert!(!st.s.ui.dragging_any());
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1 - 3.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(st.s.ui.dock.ratio(3), ratio);
    // Nor does Size... light it: the console's group's bar is then the one
    // beside it, at the script list's right, and Tab has no other to light.
    redraw(&mut st, &mut game);
    let at = middle(tab_of(&st, Pane::Console));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Size...");
    assert_eq!(st.s.ui.sizing, Some((Pane::Console, 1)));
    press(&mut st, &mut game, Key::Tab, NONE);
    assert_eq!(st.s.ui.sizing, Some((Pane::Console, 1)));
    press(&mut st, &mut game, Key::Escape, NONE);
    // An Editor area 456 wide leaves the script list's bar 8 units to move
    // in, its only one: Size... is greyed on its menu.
    st.s.ui.look.rect = Some([10.0, 10.0, 490.0, 500.0]);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws.unwrap()[2], 456.0);
    assert!(!bar(&st, 1).movable());
    let at = middle(tab_of(&st, Pane::Scripts));
    right_click(&mut st, &mut game, at);
    assert!(!item(&st, "Size...").enabled);
}

#[test]
fn ctrl_backtick_works_on_the_panel_wherever_it_is() {
    let (mut game, mut st) = editing("dock-toggle");
    let toggle = |st: &mut Studio, game: &mut FakeGame| {
        press(st, game, Key::Char('`'), CTRL);
    };
    // The Agent at the front of the panel, the keys in the code: its front
    // pane takes them, then the panel hides, then shows as it was.
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    toggle(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    type_text(&mut st, &mut game, "hi");
    assert_eq!(st.s.ui.agent_input.text, "hi");
    toggle(&mut st, &mut game);
    assert!(!st.s.ui.dock.shown(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Code).rect[3], 408.0, "the code takes its room");
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Agent);
    // The same with the Problems at the front: the arrows choose there.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.editor.set_text("print(player.poss())\nprint(player.fly())\n");
    st.s.ui.dock.activate(Pane::Problems);
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.problems.len(), 2);
    toggle(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Problems);
    press(&mut st, &mut game, Key::Down, NONE);
    assert_eq!((st.s.editor.row, st.s.ui.problem_sel), (0, 1));
    toggle(&mut st, &mut game);
    assert!(!st.s.ui.dock.shown(Pane::Problems) && st.s.ui.focus == Pane::Code);
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Problems) && st.s.ui.focus == Pane::Problems);
    // The console in a group of its own beside the code: that group.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.ui.dock = Dock::default();
    assert!(st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right
        }
    ));
    redraw(&mut st, &mut game);
    toggle(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Console);
    toggle(&mut st, &mut game);
    assert!(!st.s.ui.dock.shown(Pane::Console) && st.s.ui.dock.shown(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Code);
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Console) && st.s.ui.focus == Pane::Console);
    // Among the code's tabs, the code at the front: the console comes to the
    // front with the keys, then the code comes back, nothing hidden. A thumb
    // held on the code's scroll bar then scrolls nothing.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    st.s.ui.dock.activate(Pane::Code);
    let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    let sb = st.s.ui.scrollbar.expect("the code's scroll bar");
    handle(&mut st, &mut game, Input::Mouse(sb.x, sb.at + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Console) && st.s.ui.focus == Pane::Console);
    let scroll = st.s.editor.scroll;
    handle(&mut st, &mut game, Input::Mouse(sb.x, sb.at + 100.0));
    assert_eq!(st.s.editor.scroll, scroll, "the held thumb let go");
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Code) && st.s.ui.focus == Pane::Code);
    assert!(st.s.ui.dock.shown(Pane::Console) && st.s.ui.dock.shown(Pane::Agent));
    // The script list dropped on the panel's strip, at its front, takes no
    // keys: the console takes them, and the next press hides the panel.
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Console,
            at: 3,
        },
    );
    redraw(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Scripts) && st.s.ui.focus == Pane::Code);
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Console) && st.s.ui.focus == Pane::Console);
    toggle(&mut st, &mut game);
    assert!(!st.s.ui.dock.shown(Pane::Console) && st.s.ui.focus == Pane::Code);
    // Hidden with the script list at its front, it shows with the console
    // in use.
    st.s.ui.dock = Dock::parse("col:0.600([code*],-[console,problems,agent,scripts*])").unwrap();
    redraw(&mut st, &mut game);
    toggle(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Console) && st.s.ui.focus == Pane::Console);
    // From another tab Ctrl+J opens the Editor tab, the panel's front pane
    // taking the keys.
    st.s.ui.dock = Dock::default();
    press(&mut st, &mut game, Key::Char('3'), CTRL);
    press(&mut st, &mut game, Key::Char('j'), CTRL);
    assert_eq!((st.s.ui.tab, st.s.ui.focus), (Tab::Editor, Pane::Console));
    // Suggestions belong to the pane they opened in: the code's close as
    // the console takes the keys, and Tab there puts nothing in the code.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "player.fa");
    assert!(st.s.ui.suggest.as_ref().is_some_and(|s| !s.console));
    toggle(&mut st, &mut game);
    assert!(st.s.ui.suggest.is_none());
    press(&mut st, &mut game, Key::Tab, NONE);
    assert_eq!(st.s.editor.text(), "player.fa");
    // The console's close as the panel hides; Tab in the code indents.
    st.s.ui.input.set("");
    type_text(&mut st, &mut game, "pla");
    assert!(st.s.ui.suggest.as_ref().is_some_and(|s| s.console));
    toggle(&mut st, &mut game);
    assert!(!st.s.ui.dock.shown(Pane::Console) && st.s.ui.focus == Pane::Code);
    press(&mut st, &mut game, Key::Tab, NONE);
    assert_eq!(st.s.editor.text(), "player.fa  ");
    assert_eq!(st.s.ui.input.text, "pla");
    // Ctrl+1 leaves the code's list open, but Tab on the Console tab takes
    // nothing from it into the code.
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "player.fa");
    assert!(st.s.ui.suggest.is_some());
    press(&mut st, &mut game, Key::Char('1'), CTRL);
    press(&mut st, &mut game, Key::Tab, NONE);
    assert_eq!(st.s.editor.text(), "player.fa");
}

#[test]
fn the_cross_hides_a_group_and_says_how_to_get_it_back() {
    let (mut game, mut st) = editing("dock-cross");
    redraw(&mut st, &mut game);
    // The panel's × (the full screen's header has one first).
    let cross = cross_of(&st, Pane::Console);
    click(&mut st, &mut game, cross);
    assert!(!st.s.ui.dock.shown(Pane::Console));
    assert_eq!(st.s.ui.focus, Pane::Code);
    assert_eq!(notice(&st), "Panel hidden: Ctrl+` brings it back");
    assert!(
        layout_txt(&st).contains("-[console*,problems,agent]"),
        "written at once"
    );
    // The code's group has none, nor the script list's (only it there),
    // whose + is the last thing on its strip.
    redraw(&mut st, &mut game);
    let (code, list) = (leaf(&st, Pane::Code), leaf(&st, Pane::Scripts));
    for (t, at) in texts(&game) {
        assert!(
            t != "×" || !(inside(code.strip, at) || inside(list.strip, at)),
            "{at:?}"
        );
    }
    let on_list: Vec<(String, (f32, f32))> = texts(&game)
        .into_iter()
        .filter(|(_, at)| inside(list.strip, *at))
        .collect();
    let last = on_list.iter().max_by(|a, b| a.1.0.total_cmp(&b.1.0)).unwrap();
    assert_eq!(last.0, "+", "{on_list:?}");
    // A problem's count on the code's strip shows the Problems with the keys,
    // the panel shown again.
    st.s.editor.set_text("print(player.poss())\n");
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    let count = texts(&game)
        .into_iter()
        .find(|(t, at)| t == "1" && inside(code.strip, *at))
        .expect("the count on the code's strip")
        .1;
    click(&mut st, &mut game, (count.0 + 2.0, count.1 + 4.0));
    assert!(st.s.ui.dock.is_front(Pane::Problems) && st.s.ui.dock.shown(Pane::Problems));
    assert_eq!(st.s.ui.focus, Pane::Problems);
    // Hidden again (Ctrl+` with the Problems in use), the count unhides it.
    press(&mut st, &mut game, Key::Char('`'), CTRL);
    assert!(!st.s.ui.dock.shown(Pane::Problems));
    redraw(&mut st, &mut game);
    click(&mut st, &mut game, (count.0 + 2.0, count.1 + 4.0));
    assert!(st.s.ui.dock.is_front(Pane::Problems) && st.s.ui.dock.shown(Pane::Problems));
    // A group of the Agent alone says how it comes back.
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.move_pane(
        Pane::Agent,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right,
        },
    );
    redraw(&mut st, &mut game);
    let cross = cross_of(&st, Pane::Agent);
    click(&mut st, &mut game, cross);
    assert!(!st.s.ui.dock.shown(Pane::Agent));
    assert_eq!(notice(&st), "Agent hidden: Panes › in a tab's menu brings it back");
}

#[test]
fn typing_never_reaches_a_code_pane_that_is_not_drawn() {
    let (mut game, mut st) = editing("dock-covered");
    st.edit_mod(mod_index(&st, "hello"));
    let path = st.s.editor.path.clone().unwrap();
    let (text, disk) = (st.s.editor.text(), std::fs::read_to_string(&path).unwrap());
    redraw(&mut st, &mut game);
    // The script list's tab dragged onto the code's strip, past its tab: it
    // joins the code's tabs at the front. The keys go nowhere, so nothing
    // reaches the code drawn under it.
    let (strip, code_tab) = (leaf(&st, Pane::Code).strip, tab_of(&st, Pane::Code));
    let to = (code_tab[0] + code_tab[2] + 20.0, strip[1] + 16.0);
    let from = middle(tab_of(&st, Pane::Scripts));
    drag(&mut st, &mut game, from, to);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Code), [Pane::Code, Pane::Scripts]);
    redraw(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Scripts));
    assert_eq!(st.s.ui.focus, Pane::Scripts);
    type_text(&mut st, &mut game, "x");
    press(&mut st, &mut game, Key::Enter, NONE);
    assert_eq!(st.s.editor.text(), text);
    assert!(!st.s.editor.dirty);
    assert_eq!(
        (st.s.ui.input.text.as_str(), st.s.ui.agent_input.text.as_str()),
        ("", "")
    );
    // A script opened from it has nothing stray to save; the code comes back.
    let row = text_at(&game, "scratch.lua");
    click(&mut st, &mut game, (row.0 + 2.0, row.1 + 4.0));
    assert_eq!(st.s.editor.title, "scratch.lua");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
    assert!(st.s.ui.dock.is_front(Pane::Code) && st.s.ui.focus == Pane::Code);
    // The same with the focus settled without a frame drawn; Esc brings the
    // code to the front with the keys.
    st.s.ui.dock.activate(Pane::Scripts);
    settle_focus(&mut st.s.ui);
    assert_eq!(st.s.ui.focus, Pane::Scripts);
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "");
    assert_eq!(press(&mut st, &mut game, Key::Escape, NONE), UiResult::Used);
    assert!(st.s.ui.dock.is_front(Pane::Code) && st.s.ui.focus == Pane::Code);
    type_text(&mut st, &mut game, "y");
    assert_eq!(st.s.editor.text(), "y");
    // The console among the code's tabs, at the front: typing is its own.
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    redraw(&mut st, &mut game);
    type_text(&mut st, &mut game, "z");
    assert_eq!((st.s.editor.text().as_str(), st.s.ui.input.text.as_str()), ("y", "z"));
}

#[test]
fn the_executor_styles_keep_their_layout_and_keys() {
    let (mut game, mut st) = editing("dock-styles");
    st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right,
        },
    );
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
    let moved = st.s.ui.dock.clone();
    // Synapse X: its own window, no strips, × or bars, and the keys in its
    // code.
    st.s.ui.look = Look {
        theme: synapse(),
        motion: false,
        ..Look::default()
    };
    redraw(&mut st, &mut game);
    assert!(st.s.ui.dock_drawn.is_empty() && st.s.ui.dock_bars.is_empty());
    assert!(!drawn(&game, TIP));
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "x");
    // Its menus have nothing of the panes.
    redraw(&mut st, &mut game);
    let code = st.s.ui.coach.code_at.expect("its code");
    right_click(&mut st, &mut game, (code[0] + 80.0, code[1] + 20.0));
    assert!(!menu_items(&st).iter().any(|l| l.starts_with("Hide") || l == "Panes"));
    press(&mut st, &mut game, Key::Escape, NONE);
    press(&mut st, &mut game, Key::Menu, NONE);
    assert!(!menu_items(&st).contains(&"Layout".to_owned()), "{:?}", menu_items(&st));
    press(&mut st, &mut game, Key::Escape, NONE);
    // Ctrl+J and Ctrl+B only go to its Editor tab.
    for k in ['j', 'b'] {
        press(&mut st, &mut game, Key::Char('3'), CTRL);
        press(&mut st, &mut game, Key::Char(k), CTRL);
        assert_eq!(st.s.ui.tab, Tab::Editor);
        assert_eq!(st.s.ui.dock, moved);
    }
    // Back on Horizon the moved layout is there.
    st.s.ui.look = Look {
        motion: false,
        ..Look::default()
    };
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock, moved);
    assert_eq!(st.s.ui.dock_drawn.len(), 4);
}

#[test]
fn the_layout_is_kept_between_sessions() {
    let (mut game, mut st) = editing("dock-kept");
    redraw(&mut st, &mut game);
    // Moved from its tab's menu, then hidden: each written at once.
    let agent_tab = tab_of(&st, Pane::Agent);
    right_click(&mut st, &mut game, middle(agent_tab));
    choose(&mut st, &mut game, "Move");
    choose(&mut st, &mut game, "Right of the code");
    let kept = layout_txt(&st);
    assert!(kept.contains("row:0.500([code*],[agent*])"), "{kept}");
    redraw(&mut st, &mut game);
    let cross = cross_of(&st, Pane::Agent);
    click(&mut st, &mut game, cross);
    assert!(layout_txt(&st).contains("-[agent*]"), "{}", layout_txt(&st));
    // Another studio on the same folder: the same, the code at the front.
    let again = studio(&mut game);
    assert_eq!(again.s.ui.dock, st.s.ui.dock);
    assert!(again.s.ui.dock.is_front(Pane::Code));
    // A tab clicked is written at once: the studio closed by the game
    // (as bridge.rs does) loses nothing.
    redraw(&mut st, &mut game);
    let problems = tab_of(&st, Pane::Problems);
    click(&mut st, &mut game, middle(problems));
    assert!(layout_txt(&st).contains("problems*"), "{}", layout_txt(&st));
    st.s.ui.open = false;
    let again = studio(&mut game);
    assert!(again.s.ui.dock.is_front(Pane::Problems));
    // A damaged file, another version's, a ratio of nan: the default.
    for text in [
        "garbage\n".to_owned(),
        format!("version=2\ndock={}\n", Dock::default().to_text()),
        "version=1\ndock=row:nan([scripts*],col:0.6([code*],[console*,problems,agent]))\n".to_owned(),
    ] {
        std::fs::write(st.s.dir.join("layout.txt"), text).unwrap();
        let again = studio(&mut game);
        assert_eq!(again.s.ui.dock, Dock::default());
        assert!(again.s.ui.dock_tips);
    }
    // Drawing alone writes nothing.
    std::fs::remove_file(st.s.dir.join("layout.txt")).unwrap();
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    assert!(!st.s.dir.join("layout.txt").exists());
}

/// A studio with sum.lua open on its Editor tab.
fn with_sum(name: &str) -> (FakeGame, Studio) {
    let (mut game, mut st) = editing(name);
    game.write_mod("sum.lua", "local a = 1\nlocal b = 2\nprint(a + b)\n");
    st.scan(&mut game);
    st.edit_mod(mod_index(&st, "sum"));
    redraw(&mut st, &mut game);
    (game, st)
}

/// A lesson of one step on sum.lua's line 2, its explanation `explain`.
fn lesson(st: &mut Studio, game: &mut FakeGame, explain: &str) {
    coach::call(
        st,
        game,
        &agent(),
        "lesson",
        &json!({ "title": "Sums", "steps": [
            { "start_line": 2, "note": "Two is b.", "explain": explain },
            { "start_line": 3, "note": "Prints it." },
        ] }),
    )
    .unwrap();
    // Its note shows from the next frame.
    redraw(st, game);
    redraw(st, game);
}

/// Whether `text` was drawn in the note's bubble.
fn in_bubble(st: &Studio, game: &FakeGame, text: &str) -> bool {
    let bubble = st.s.ui.coach.bubble_at.expect("the note's bubble");
    texts(game)
        .iter()
        .any(|(t, at)| t.contains(text) && inside(bubble, *at))
}

#[test]
fn a_lesson_shows_the_agent_pane_unless_it_would_cover_the_keys() {
    // The Agent among the script list's tabs, behind it: the lesson brings
    // it to the front there, where it has the explanation.
    let (mut game, mut st) = with_sum("dock-lesson");
    st.s.ui.dock.move_pane(
        Pane::Agent,
        DropAt::Into {
            anchor: Pane::Scripts,
            at: 1,
        },
    );
    st.s.ui.dock.activate(Pane::Scripts);
    redraw(&mut st, &mut game);
    lesson(&mut st, &mut game, "Explained in the pane.");
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    assert!(st.s.ui.dock_front[Pane::Agent.index()]);
    assert!(!in_bubble(&st, &game, "Explained in the pane."));
    assert!(drawn(&game, "Explained in the pane."), "the Agent pane has it");
    assert_eq!(st.s.ui.focus, Pane::Code);
    // The panel hidden: it shows again, the Agent at its front.
    let (mut game, mut st) = with_sum("dock-lesson-hidden");
    st.s.ui.dock.set_hidden(Pane::Agent, true);
    redraw(&mut st, &mut game);
    lesson(&mut st, &mut game, "Explained again.");
    assert!(st.s.ui.dock.shown(Pane::Agent) && st.s.ui.dock.is_front(Pane::Agent));
    assert!(!in_bubble(&st, &game, "Explained again."));
    assert_eq!(st.s.ui.focus, Pane::Code);
    // The player's keys on the console, in the Agent's group: the Agent stays
    // behind it, and the explanation is in the note.
    let (mut game, mut st) = with_sum("dock-lesson-keys");
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
    st.s.now += 5.0;
    lesson(&mut st, &mut game, "Explained in the note.");
    assert!(st.s.ui.dock.is_front(Pane::Console) && !st.s.ui.dock.is_front(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Console);
    assert!(in_bubble(&st, &game, "Explained in the note."));
    // In an 854 x 480 game's window the panel is only its strip under the
    // code: the Agent's tab at its front, but no body, so the note explains.
    let (mut game, mut st) = with_sum("dock-lesson-small");
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    redraw(&mut st, &mut game);
    lesson(&mut st, &mut game, "Explained in the small note.");
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    assert!(leaf(&st, Pane::Agent).collapsed);
    assert!(!st.s.ui.dock_front[Pane::Agent.index()]);
    assert!(in_bubble(&st, &game, "Explained in the small note."));
    assert_eq!(st.s.ui.focus, Pane::Code);
}

#[test]
fn an_agent_opening_a_file_never_takes_the_keys() {
    let (mut game, mut st) = with_sum("dock-agent-open");
    // The player is typing a question in the Agent pane.
    let agent_tab = tab_of(&st, Pane::Agent);
    click(&mut st, &mut game, middle(agent_tab));
    assert_eq!(st.s.ui.focus, Pane::Agent);
    type_text(&mut st, &mut game, "half a question");
    // The console's edit() opens a file: the code comes forward, the keys
    // stay where they are.
    st.run_console(&mut game, "edit('hello')");
    assert_eq!(st.s.editor.title, "hello.lua");
    assert!(st.s.ui.dock.is_front(Pane::Code) && st.s.ui.dock.is_front(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Agent);
    type_text(&mut st, &mut game, " more");
    assert_eq!(st.s.ui.agent_input.text, "half a question more");
    // An agent's open_script, once the player pauses: the same.
    st.s.now += 5.0;
    coach::call(&mut st, &mut game, &agent(), "open", &json!({ "id": "sum" })).unwrap();
    assert_eq!(st.s.editor.title, "sum.lua");
    assert_eq!(st.s.ui.focus, Pane::Agent);
    // Among the code's tabs and in use, the Agent stays at the front.
    st.s.ui.dock.move_pane(
        Pane::Agent,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    st.s.now += 5.0;
    coach::call(&mut st, &mut game, &agent(), "open", &json!({ "id": "hello" })).unwrap();
    assert_eq!(st.s.editor.title, "hello.lua");
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
    type_text(&mut st, &mut game, "!");
    assert_eq!(st.s.ui.agent_input.text, "half a question more!");
    // Show on its line is the player's own click: the code, with the keys.
    redraw(&mut st, &mut game);
    let show = last_text_at(&game, "Show");
    click(&mut st, &mut game, (show.0 + 4.0, show.1 + 4.0));
    assert!(st.s.ui.dock.is_front(Pane::Code) && st.s.ui.focus == Pane::Code);
    // The script list over the code: an agent's edit lands at once, and so
    // does a change on disk; with the code drawn they are typed in.
    st.s.ui.look.motion = true;
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    st.s.now += 5.0;
    redraw(&mut st, &mut game);
    assert!(!st.s.ui.dock_front[Pane::Code.index()]);
    let r = coach::call(
        &mut st,
        &mut game,
        &agent(),
        "edit",
        &json!({ "old_text": "print", "new_text": "print(1)\nprint" }),
    )
    .unwrap();
    assert!(r.get("typing").is_none(), "landed at once: {r}");
    assert!(st.s.editor.text().contains("print(1)\nprint"), "{r}");
    coach::call(&mut st, &mut game, &agent(), "save", &json!({})).unwrap();
    let path = st.s.editor.path.clone().unwrap();
    std::fs::write(&path, "print('from disk')\n").unwrap();
    st.s.now += 1.0;
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.text(), "print('from disk')\n");
    assert!(st.s.ui.coach.typing.is_none(), "read in at once");
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.now += 5.0;
    redraw(&mut st, &mut game);
    assert!(st.s.ui.dock_front[Pane::Code.index()]);
    let r = coach::call(
        &mut st,
        &mut game,
        &agent(),
        "edit",
        &json!({ "old_text": "print", "new_text": "print(2)\nprint" }),
    )
    .unwrap();
    assert!(r.get("typing").is_some(), "typed in: {r}");
}

#[test]
fn the_codes_chips_bring_it_back_from_its_strip() {
    // In an 854 x 480 game's window, with the console in use, the code is
    // only its strip: a click on "2 notes" there brings the code back with
    // the keys, its note shown.
    let (mut game, mut st) = with_sum("dock-chips");
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    for line in [1, 3] {
        let mark = json!({ "start_line": line, "note": "Look here." });
        coach::call(&mut st, &mut game, &agent(), "mark", &mark).unwrap();
    }
    redraw(&mut st, &mut game);
    let px = |(x, y): (f32, f32)| (x * 1.125, y * 1.125);
    let console = px(middle(tab_of(&st, Pane::Console)));
    click(&mut st, &mut game, console);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Console);
    assert!(leaf(&st, Pane::Code).collapsed);
    let chip = text_at(&game, "2 notes");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    assert!(!leaf(&st, Pane::Code).collapsed);
    assert!(st.s.ui.coach.bubble_at.is_some(), "its note shows");
    // "Lesson 1/2" the same: its step shows again.
    lesson(&mut st, &mut game, "Explained.");
    let console = px(middle(tab_of(&st, Pane::Console)));
    click(&mut st, &mut game, console);
    redraw(&mut st, &mut game);
    assert!(leaf(&st, Pane::Code).collapsed);
    let chip = text_at(&game, "Lesson 1/2");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    assert!(in_bubble(&st, &game, "Two is b."), "its step shows");
}

#[test]
fn what_agents_do_leaves_what_the_player_holds() {
    // The bar between the code and the panel held and moved up 40.
    let (mut game, mut st) = with_sum("dock-agent-held");
    let col = bar(&st, 3);
    let at = (col.rect[0] + 300.0, col.rect[1] + 4.0);
    hold(&mut st, &mut game, at, &[(at.0, at.1 - 40.0)]);
    assert_eq!(st.s.ui.dock.ratio(3), Some(0.5));
    // An agent opens the file open once the player pauses, the code at the
    // front already: the bar still follows the pointer.
    st.s.now += 5.0;
    coach::call(&mut st, &mut game, &agent(), "open", &json!({ "id": "sum" })).unwrap();
    point(&mut st, &mut game, (at.0, at.1 - 60.0));
    assert_eq!(st.s.ui.dock.ratio(3), Some(0.45));
    // A lesson brings the Agent pane to the front of the panel: the same,
    // and the bar is written where it is let go.
    lesson(&mut st, &mut game, "Explained.");
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    point(&mut st, &mut game, (at.0, at.1 - 80.0));
    assert_eq!(st.s.ui.dock.ratio(3), Some(0.4));
    release(&mut st, &mut game);
    assert!(layout_txt(&st).contains("col:0.400("), "{}", layout_txt(&st));
    // The code's scroll bar held: a lesson bringing the Agent pane to the
    // front of the panel lets it scroll on.
    let (mut game, mut st) = editing("dock-agent-held-thumb");
    let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
    game.write_mod(
        "sum.lua",
        &format!("local a = 1\nlocal b = 2\nprint(a + b)\n{}\n", text.join("\n")),
    );
    st.scan(&mut game);
    st.edit_mod(mod_index(&st, "sum"));
    redraw(&mut st, &mut game);
    let sb = st.s.ui.scrollbar.expect("the code's scroll bar");
    hold(&mut st, &mut game, (sb.x, sb.at + 4.0), &[]);
    st.s.now += 5.0;
    lesson(&mut st, &mut game, "Explained.");
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    let scroll = st.s.editor.scroll;
    point(&mut st, &mut game, (sb.x, sb.at + 100.0));
    assert!(st.s.editor.scroll > scroll, "the thumb still held");
    release(&mut st, &mut game);
    // A tab lifted over the script list's strip: an agent bringing the code
    // to the front of its group (the Docs there in front of it) leaves it
    // lifted, and it lands where it is let go.
    let (mut game, mut st) = with_sum("dock-agent-held-tab");
    st.s.ui.dock.move_pane(
        Pane::Docs,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    redraw(&mut st, &mut game);
    let strip = leaf(&st, Pane::Scripts).strip;
    let from = middle(tab_of(&st, Pane::Agent));
    hold(
        &mut st,
        &mut game,
        from,
        &[(strip[0] + strip[2] - 10.0, strip[1] + 16.0)],
    );
    assert_eq!(st.s.ui.lifted(), Some(Pane::Agent));
    st.s.now += 5.0;
    coach::call(&mut st, &mut game, &agent(), "open", &json!({ "id": "sum" })).unwrap();
    assert!(st.s.ui.dock.is_front(Pane::Code));
    assert_eq!(st.s.ui.lifted(), Some(Pane::Agent));
    assert!(st.s.ui.dock_spot.is_some());
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Agent), [Pane::Scripts, Pane::Agent]);
}

#[test]
fn the_wheel_scrolls_the_pane_under_the_pointer() {
    let (mut game, mut st) = editing("dock-wheel");
    for i in 0..50 {
        st.s.log(LogKind::Info, format!("line {i}"));
    }
    for n in 0..20 {
        game.write_mod(&format!("s{n:02}.lua"), "print(1)\n");
    }
    st.scan(&mut game);
    let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    let wheel = |st: &mut Studio, game: &mut FakeGame, at: (f32, f32)| {
        handle(st, game, Input::Mouse(at.0, at.1));
        handle(st, game, Input::Wheel(1.0));
    };
    // Over the console's pane: its log, not the code.
    let console = leaf(&st, Pane::Console);
    wheel(&mut st, &mut game, middle(console.rect));
    assert_eq!((st.s.ui.log_scroll, st.s.editor.scroll), (3, 0));
    // Over the code: the code.
    let code = leaf(&st, Pane::Code);
    handle(
        &mut st,
        &mut game,
        Input::Mouse(middle(code.rect).0, middle(code.rect).1),
    );
    handle(&mut st, &mut game, Input::Wheel(-1.0));
    assert_eq!((st.s.ui.log_scroll, st.s.editor.scroll), (3, 3));
    // Over the script list: its rows.
    let list = leaf(&st, Pane::Scripts);
    handle(
        &mut st,
        &mut game,
        Input::Mouse(middle(list.rect).0, middle(list.rect).1),
    );
    handle(&mut st, &mut game, Input::Wheel(-1.0));
    assert_eq!(st.s.ui.scripts_scroll, 1);
    assert_eq!(st.s.editor.scroll, 3);
    // The console moved right of the code: still its log there.
    st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right,
        },
    );
    redraw(&mut st, &mut game);
    let console = leaf(&st, Pane::Console);
    assert!(console.rect[0] > leaf(&st, Pane::Code).rect[0]);
    wheel(&mut st, &mut game, middle(console.rect));
    assert_eq!((st.s.ui.log_scroll, st.s.editor.scroll), (6, 3));
}

#[test]
fn a_panes_menu_has_its_items_then_the_layouts() {
    let (mut game, mut st) = editing("dock-menus");
    st.s.log(LogKind::Info, "something");
    redraw(&mut st, &mut game);
    // The console's tab.
    let tab = tab_of(&st, Pane::Console);
    right_click(&mut st, &mut game, middle(tab));
    assert_eq!(
        menu_items(&st),
        [
            "Copy all",
            "Clear",
            "",
            "Hide the panel",
            "Move",
            "Size...",
            "Panes",
            "Reset layout"
        ]
    );
    assert_eq!(item(&st, "Hide the panel").hint, "Ctrl `");
    assert_eq!(
        (item(&st, "Move").hint.as_str(), item(&st, "Panes").hint.as_str()),
        ("›", "›")
    );
    assert!(!item(&st, "Reset layout").enabled, "the layout is the default");
    press(&mut st, &mut game, Key::Escape, NONE);
    // The code's tab: the file's items, then neither Hide nor Close.
    let tab = tab_of(&st, Pane::Code);
    right_click(&mut st, &mut game, middle(tab));
    let items = usable(&st);
    assert_eq!(items[0], "Save");
    assert_eq!(items[items.len() - 3..], ["Move", "Size...", "Panes"]);
    assert!(!items.iter().any(|l| l.starts_with("Hide") || l.starts_with("Close")));
    // Panes: every pane (the Library and the Docs too, which dock when
    // chosen), those drawn at the front marked; one chosen comes to the
    // front with the keys, nothing closed.
    choose(&mut st, &mut game, "Panes");
    assert_eq!(
        menu_items(&st),
        ["Scripts", "Code", "Console", "Problems", "Agent", "Library", "Docs"]
    );
    let menu = st.s.ui.menu.clone().unwrap();
    let marked: Vec<&str> = menu
        .items
        .iter()
        .filter(|i| i.checked)
        .map(|i| i.label.as_str())
        .collect();
    assert_eq!(marked, ["Scripts", "Code", "Console"]);
    choose(&mut st, &mut game, "Agent");
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
    assert_eq!(
        st.s.ui.dock.leaf_tabs(Pane::Agent),
        [Pane::Console, Pane::Problems, Pane::Agent]
    );
    // After a move, Reset layout: the default, the code in use, a notice,
    // and its undo in its place, which brings the move back.
    st.s.ui.dock.move_pane(
        Pane::Agent,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right,
        },
    );
    let moved = st.s.ui.dock.clone();
    redraw(&mut st, &mut game);
    let tab = tab_of(&st, Pane::Console);
    right_click(&mut st, &mut game, middle(tab));
    choose(&mut st, &mut game, "Reset layout");
    assert_eq!(st.s.ui.dock, Dock::default());
    assert_eq!(st.s.ui.focus, Pane::Code);
    assert_eq!(notice(&st), "Back to the default layout");
    assert!(layout_txt(&st).contains(&Dock::default().to_text()));
    redraw(&mut st, &mut game);
    let tab = tab_of(&st, Pane::Console);
    right_click(&mut st, &mut game, middle(tab));
    assert_eq!(menu_items(&st).last().unwrap(), "Undo reset layout");
    choose(&mut st, &mut game, "Undo reset layout");
    assert_eq!(st.s.ui.dock, moved);
    assert_eq!(notice(&st), "Your layout is back");
    assert!(layout_txt(&st).contains(&moved.to_text()), "{}", layout_txt(&st));
    // The window's menu has the panes too, after the themes; not in an
    // executor style.
    redraw(&mut st, &mut game);
    let name = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (name.0 + 4.0, name.1 + 4.0));
    let items = menu_items(&st);
    let theme = items.iter().position(|l| l == "Theme").unwrap();
    assert_eq!(items[theme + 1..theme + 3], ["Panes", "Reset layout"]);
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.ui.look = Look {
        theme: synapse(),
        motion: false,
        ..Look::default()
    };
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (title.0 + 4.0, title.1 + 4.0));
    assert!(!menu_items(&st).iter().any(|l| l == "Panes" || l.contains("layout")));
}

#[test]
fn a_right_click_in_a_panes_body_keeps_its_own_menu() {
    let (mut game, mut st) = editing("dock-body-menus");
    st.s.editor.set_text("local here = 1\nprint(player.poss())\n");
    st.s.log(LogKind::Info, "a console line");
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    // The code's text: the code's menu.
    let at = last_text_at(&game, "here");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(menu_items(&st)[0], "Cut", "{:?}", menu_items(&st));
    assert!(!menu_items(&st).iter().any(|l| l == "Hide the panel"));
    press(&mut st, &mut game, Key::Escape, NONE);
    // A console line: the text's menu.
    let at = text_at(&game, "a console line");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(menu_items(&st)[0], "Copy");
    assert!(!menu_items(&st).iter().any(|l| l == "Hide the panel"));
    press(&mut st, &mut game, Key::Escape, NONE);
    // The console's input: its line's, and the console takes the keys.
    let at = text_at(&game, "Click to run Lua here.");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(menu_items(&st)[0], "Paste");
    assert_eq!(st.s.ui.focus, Pane::Console);
    press(&mut st, &mut game, Key::Escape, NONE);
    press(&mut st, &mut game, Key::Escape, NONE);
    // A problem's row.
    st.s.ui.dock.activate(Pane::Problems);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "player has no 'poss'");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(menu_items(&st)[0], "Go to it");
    press(&mut st, &mut game, Key::Escape, NONE);
    // A script row.
    let at = text_at(&game, "scratch.lua");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert!(menu_items(&st).contains(&"Copy path".to_owned()));
    assert!(!menu_items(&st).iter().any(|l| l == "Move"));
    press(&mut st, &mut game, Key::Escape, NONE);
    // The empty part of a strip: the pane's menu, with the layout's.
    let strip = leaf(&st, Pane::Problems).strip;
    right_click(&mut st, &mut game, (strip[0] + strip[2] / 2.0, strip[1] + 16.0));
    assert_eq!(menu_items(&st)[0], "Copy all problems");
    assert!(menu_items(&st).contains(&"Move".to_owned()));
}

#[cfg(unix)]
#[test]
fn asking_the_agent_brings_its_pane_back() {
    use horizon_studio::game::hstudio_ui::agent_busy;
    let (mut game, mut st) = editing("dock-ask");
    st.s.ui.agent.agents = Some(vec![echo()]);
    st.s.editor.set_text("local a = 1\nprint(a)");
    redraw(&mut st, &mut game);
    let cross = cross_of(&st, Pane::Console);
    click(&mut st, &mut game, cross);
    assert!(!st.s.ui.dock.shown(Pane::Agent));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "print");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    choose(&mut st, &mut game, "Explain this line with Echo");
    assert!(agent_busy(&st));
    assert!(st.s.ui.dock.shown(Pane::Agent) && st.s.ui.dock.is_front(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Code, "the keys stay on the code");
    for _ in 0..200 {
        common::run_for(&mut st, &mut game, 0.05);
        if !agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Ask Echo...: its input takes the keys.
    st.s.ui.dock.activate(Pane::Console);
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "print");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    choose(&mut st, &mut game, "Ask Echo...");
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
}

#[test]
fn panes_fit_any_window() {
    // Rows of code that the default layout showed before the panes moved
    // (measured on the code as it was): full screen and in the default
    // window, at look.scale 1, 1.125 and 1.5. Where the panel is now its
    // strip under the code (it didn't show at all), one row less.
    let rows = [
        (1.0, false, 9),
        (1.0, true, 3),
        (1.125, false, 6),
        (1.125, true, 7),
        (1.5, false, 6),
        (1.5, true, 6),
    ];
    let mut sizes: Vec<(f32, bool, Option<[f32; 4]>)> = rows.iter().map(|&(s, w, _)| (s, w, None)).collect();
    sizes.push((1.0, true, Some([10.0, 10.0, 330.0, 230.0])));
    // Each pane in use (the Library and the Docs docked at their home), and
    // each again with both of those docked.
    let panes = [
        Pane::Code,
        Pane::Console,
        Pane::Agent,
        Pane::Problems,
        Pane::Library,
        Pane::Docs,
    ];
    let cases: Vec<(Pane, bool)> = panes
        .iter()
        .map(|&p| (p, false))
        .chain(panes.iter().map(|&p| (p, true)))
        .collect();
    for (scale, window, rect) in sizes {
        for &(focus, both) in &cases {
            let what = format!("scale {scale}, window {window}, {rect:?}, {focus:?}, both docked {both}");
            let (mut game, mut st) = editing("dock-fit");
            let text: Vec<String> = (1..=300).map(|i| format!("local line{i} = {i}")).collect();
            st.s.editor
                .set_text(&format!("print(player.poss())\n{}", text.join("\n")));
            for i in 0..50 {
                st.s.log(LogKind::Info, format!("line {i}"));
            }
            st.s.ui.look.scale = scale;
            st.s.ui.look.window = window;
            st.s.ui.look.rect = rect;
            if both {
                st.s.ui.dock.show(Pane::Docs);
                st.s.ui.dock.show(Pane::Library);
            }
            st.s.ui.dock.show(focus);
            st.s.ui.focus = focus;
            redraw(&mut st, &mut game);
            redraw(&mut st, &mut game);
            let ws = st.s.ui.dock_ws.expect("the Editor tab drawn");
            let within = |r: [f32; 4]| {
                r[0] >= ws[0] - 0.01
                    && r[1] >= ws[1] - 0.01
                    && r[0] + r[2] <= ws[0] + ws[2] + 0.01
                    && r[1] + r[3] <= ws[1] + ws[3] + 0.01
            };
            for l in &st.s.ui.dock_drawn {
                assert!(within(l.rect), "{what}: {l:?} out of {ws:?}");
            }
            for (_, _, c) in clips(&game) {
                let r = c.map(|v| v / scale);
                assert!(within(r), "{what}: clip {r:?} out of {ws:?}");
            }
            // The pane in use always has its body.
            let used = leaf(&st, focus);
            assert!(!used.collapsed && used.active == focus, "{what}: {used:?}");
            assert_eq!(st.s.ui.focus, focus, "{what}");
            // Each body drawn shows something: a line of code or of the
            // console, a row of the agent's (where its group has its least
            // height: under it the input line is all that fits).
            for l in st.s.ui.dock_drawn.iter().filter(|l| !l.collapsed) {
                let enough = l.rect[3] >= l.active.least()[1];
                match l.active {
                    Pane::Code => assert!(st.s.ui.pages[Pane::Code.index()] >= 1, "{what}"),
                    Pane::Console if enough => {
                        assert!(drawn(&game, "line 49"), "{what}: no console line")
                    }
                    Pane::Agent if enough => assert!(
                        texts(&game)
                            .iter()
                            .any(|(t, _)| t.starts_with("No agent found on this")),
                        "{what}: no row of the agent's"
                    ),
                    // Under its least height the input line is what fits.
                    Pane::Console => assert!(
                        texts(&game)
                            .iter()
                            .any(|(t, _)| t.starts_with("Lua to run") || t.starts_with("Click to run")),
                        "{what}: no console input"
                    ),
                    Pane::Agent => assert!(drawn(&game, "No agent found."), "{what}: no agent's input"),
                    Pane::Library => assert!(drawn(&game, "Script mods"), "{what}: no Library"),
                    Pane::Docs => assert!(st.s.ui.pages[Pane::Docs.index()] >= 1, "{what}: no line of the docs"),
                    _ => {}
                }
            }
            if focus == Pane::Code
                && !both
                && rect.is_none()
                && let Some(&(_, _, want)) = rows.iter().find(|r| r.0 == scale && r.1 == window)
            {
                let got = st.s.ui.pages[Pane::Code.index()];
                assert!(got >= want, "{what}: {got} rows of code, {want} before");
                // Where the panel didn't show at all, it is its strip now.
                let shown_before = scale == 1.0 || (scale == 1.125 && !window);
                assert_eq!(leaf(&st, Pane::Console).collapsed, !shown_before, "{what}");
            }
        }
    }
}

#[test]
fn ctrl_b_shows_and_hides_the_script_list() {
    let (mut game, mut st) = editing("dock-ctrl-b");
    redraw(&mut st, &mut game);
    // Hidden: no Scripts tab, the code takes its width, a notice says how
    // it comes back, and the code's keys say so.
    press(&mut st, &mut game, Key::Char('b'), CTRL);
    assert!(!st.s.ui.dock.shown(Pane::Scripts));
    assert_eq!(notice(&st), "Script list hidden: Ctrl+B brings it back");
    redraw(&mut st, &mut game);
    assert!(!drawn(&game, "Scripts"));
    assert_eq!(leaf(&st, Pane::Code).rect[0], 48.0);
    assert!(drawn(&game, "Ctrl B"), "the footer's hint");
    assert_eq!(st.s.ui.focus, Pane::Code);
    // Shown again, the keys still the code's.
    press(&mut st, &mut game, Key::Char('b'), CTRL);
    redraw(&mut st, &mut game);
    assert!(drawn(&game, "Scripts") && !drawn(&game, "Ctrl B"));
    assert_eq!(st.s.ui.focus, Pane::Code);
    // An Editor area 400 wide: no room for the list beside the code, so
    // Ctrl+B makes it the pane in use, filling the area (typing goes
    // nowhere) until a script is opened from it.
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 434.0, 500.0]);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws.unwrap()[2], 400.0);
    assert!(!st.s.ui.dock_front[Pane::Scripts.index()]);
    press(&mut st, &mut game, Key::Char('b'), CTRL);
    assert_eq!(st.s.ui.focus, Pane::Scripts);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_drawn.len(), 1);
    assert_eq!(leaf(&st, Pane::Scripts).rect, st.s.ui.dock_ws.unwrap());
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "");
    let row = text_at(&game, "hello.lua");
    click(&mut st, &mut game, (row.0 + 2.0, row.1 + 4.0));
    assert_eq!((st.s.editor.title.as_str(), st.s.ui.focus), ("hello.lua", Pane::Code));
    redraw(&mut st, &mut game);
    assert!(!st.s.ui.dock_front[Pane::Scripts.index()], "the list gone");
    // Drawn full screen on the Editor tab, then in that window again while
    // the Console page shows: Panes ›, in the window's own menu, makes the
    // list the pane in use, as the Editor tab now has no room for it beside
    // the code.
    st.s.ui.look.window = false;
    redraw(&mut st, &mut game);
    assert!(st.s.ui.dock_front[Pane::Scripts.index()]);
    press(&mut st, &mut game, Key::Char('1'), CTRL);
    st.s.ui.look.window = true;
    redraw(&mut st, &mut game);
    let name = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (name.0 + 4.0, name.1 + 4.0));
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Scripts");
    assert_eq!((st.s.ui.tab, st.s.ui.focus), (Tab::Editor, Pane::Scripts));
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Scripts).rect, st.s.ui.dock_ws.unwrap());
}

#[test]
fn a_click_in_a_pane_gives_it_the_keys_and_its_strip_buttons_dont() {
    let (mut game, mut st) = editing("dock-click-keys");
    st.s.log(LogKind::Info, "a console line");
    redraw(&mut st, &mut game);
    // A click on the console's log gives it the keys.
    let line = text_at(&game, "a console line");
    click(&mut st, &mut game, (line.0 + 2.0, line.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
    // The Agent pane's log too.
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    let body = Pane::Agent.body(leaf(&st, Pane::Agent).rect);
    click(&mut st, &mut game, (body[0] + 40.0, body[1] + 10.0));
    assert_eq!(st.s.ui.focus, Pane::Agent);
    // The strips' buttons leave the keys where they are: the console's
    // Clear, the code's Save and Run.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.ui.dock.activate(Pane::Console);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Code);
    let clear = last_text_at(&game, "Clear");
    click(&mut st, &mut game, (clear.0 + 2.0, clear.1 + 4.0));
    assert!(st.s.log.is_empty(), "cleared");
    assert_eq!(st.s.ui.focus, Pane::Code);
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
    redraw(&mut st, &mut game);
    for label in ["Save", "Run"] {
        let at = text_at(&game, label);
        click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
        assert_eq!(st.s.ui.focus, Pane::Console, "{label}");
    }
    // The Agent's Stop and Model chip, while it works, with Claude Code.
    st.s.ui.agent.agents = Some(vec![horizon_studio::game::hstudio_agent::AgentSpec::claude(
        std::path::PathBuf::from("claude"),
    )]);
    st.s.ui.dock.activate(Pane::Agent);
    st.s.ui.focus = Pane::Code;
    redraw(&mut st, &mut game);
    let model = text_at(&game, "Model: Default");
    click(&mut st, &mut game, (model.0 + 2.0, model.1 + 4.0));
    assert!(st.s.ui.menu.is_some(), "its menu");
    assert_eq!(st.s.ui.focus, Pane::Code);
    press(&mut st, &mut game, Key::Escape, NONE);
    // The code's suggestions over the panel: a click on one takes it into
    // the code, which keeps the keys.
    st.s.ui.dock.activate(Pane::Console);
    st.s.editor.set_text(&"local a = 1\n".repeat(8));
    redraw(&mut st, &mut game);
    st.s.editor.click(8, 0, false);
    type_text(&mut st, &mut game, "player.fa");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "cing");
    let panel = leaf(&st, Pane::Console).rect;
    let past = (at.0 + 2.0, panel[1] + 3.0);
    assert!(past.1 < at.1 + 20.0, "on its first row: {past:?} {at:?}");
    click(&mut st, &mut game, past);
    assert_eq!(st.s.editor.lines[8], "player.facing()");
    assert_eq!(st.s.ui.focus, Pane::Code);
    // A click in the gap between groups, or on the footer, moves no keys.
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
    redraw(&mut st, &mut game);
    let gap = bar(&st, 1).rect;
    click(&mut st, &mut game, (gap[0] + 4.0, gap[1] + 4.0));
    click(&mut st, &mut game, (400.0, 520.0));
    assert_eq!(st.s.ui.focus, Pane::Console);
}

#[test]
fn keys_alone_move_size_and_reset_panes() {
    let (mut game, mut st) = editing("dock-keys");
    redraw(&mut st, &mut game);
    // The Menu key in the code: its menu, ending with Layout.
    press(&mut st, &mut game, Key::Menu, NONE);
    let items = menu_items(&st);
    assert_eq!(items[items.len() - 2..], ["", "Layout"]);
    choose_by_keys(&mut st, &mut game, "Layout");
    assert_eq!(
        menu_items(&st),
        ["Move", "Size...", "Panes", "Reset layout"],
        "the code's layout items"
    );
    // Panes, then the Agent: it has the keys.
    choose_by_keys(&mut st, &mut game, "Panes");
    choose_by_keys(&mut st, &mut game, "Agent");
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
    // The Menu key there: its input line's menu, ending with Layout.
    redraw(&mut st, &mut game);
    press(&mut st, &mut game, Key::Menu, NONE);
    let items = menu_items(&st);
    assert_eq!(items[0], "Paste");
    assert_eq!(items[items.len() - 2..], ["", "Layout"]);
    // Layout, Move, Right of the code: the Agent in its own group right of
    // the code, with the keys.
    choose_by_keys(&mut st, &mut game, "Layout");
    choose_by_keys(&mut st, &mut game, "Move");
    choose_by_keys(&mut st, &mut game, "Right of the code");
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Agent), [Pane::Agent]);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    redraw(&mut st, &mut game);
    let (code, agent) = (leaf(&st, Pane::Code), leaf(&st, Pane::Agent));
    assert_eq!(code.rect[1], agent.rect[1]);
    assert!(agent.rect[0] > code.rect[0]);
    // Layout, Size...: the bar between them lit; the arrows move it 16 a
    // press, as far as the code's least width; Enter ends it, written.
    press(&mut st, &mut game, Key::Menu, NONE);
    choose_by_keys(&mut st, &mut game, "Layout");
    choose_by_keys(&mut st, &mut game, "Size...");
    let path = st.s.ui.sizing.expect("a bar lit").1;
    let lit_bar = bar(&st, path);
    assert!(
        lit_bar.row && lit_bar.rect[0] + lit_bar.rect[2] == agent.rect[0],
        "{lit_bar:?}"
    );
    redraw(&mut st, &mut game);
    assert!(drawn(&game, "move the bar"), "the footer says how");
    for _ in 0..2 {
        press(&mut st, &mut game, Key::Left, NONE);
    }
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Agent).rect[0], agent.rect[0] - 32.0, "Agent wider");
    press(&mut st, &mut game, Key::Left, NONE);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Code).rect[2], 300.0, "the code at its least");
    assert!(st.s.ui.sizing.is_some());
    press(&mut st, &mut game, Key::Enter, NONE);
    assert!(st.s.ui.sizing.is_none());
    let ratio = st.s.ui.dock.ratio(path).unwrap();
    assert!(
        layout_txt(&st).contains(&format!("row:{ratio:.3}([code*],[agent*])")),
        "{}",
        layout_txt(&st)
    );
    // Layout, Reset layout: the default.
    press(&mut st, &mut game, Key::Menu, NONE);
    choose_by_keys(&mut st, &mut game, "Layout");
    choose_by_keys(&mut st, &mut game, "Reset layout");
    assert_eq!(st.s.ui.dock, Dock::default());
    // In an 854 x 480 game's window, Right of the code would push the script
    // list out: greyed.
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    redraw(&mut st, &mut game);
    let at = middle(tab_of(&st, Pane::Agent));
    right_click(&mut st, &mut game, (at.0 * 1.125, at.1 * 1.125));
    choose(&mut st, &mut game, "Move");
    assert!(!item(&st, "Right of the code").enabled);
    assert!(item(&st, "With the code").enabled);
}

#[test]
fn sizing_from_the_keys() {
    let (mut game, mut st) = editing("dock-sizing");
    redraw(&mut st, &mut game);
    // Size... clicked on the code's tab's menu: the bar under the code is
    // lit (its own split's comes first), and stays lit after the click.
    let at = middle(tab_of(&st, Pane::Code));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Size...");
    assert_eq!(st.s.ui.sizing, Some((Pane::Code, 3)));
    // Tab lights the next bar, the script list's; Shift+Tab the first again.
    press(&mut st, &mut game, Key::Tab, NONE);
    assert_eq!(st.s.ui.sizing, Some((Pane::Code, 1)));
    press(&mut st, &mut game, Key::Tab, SHIFT);
    assert_eq!(st.s.ui.sizing, Some((Pane::Code, 3)));
    // Up and Down move a bar between groups stacked, 64 units at a press
    // with Shift, else 16, each step written at once.
    press(&mut st, &mut game, Key::Up, SHIFT);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Code).rect[3], 176.0);
    press(&mut st, &mut game, Key::Down, NONE);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Code).rect[3], 192.0);
    assert!(layout_txt(&st).contains("col:0.480("), "{}", layout_txt(&st));
    // A key that only changes what others mean keeps it lit.
    press(&mut st, &mut game, Key::Other, NONE);
    assert_eq!(st.s.ui.sizing, Some((Pane::Code, 3)));
    // Esc ends it, the studio staying open and the bar where it is.
    assert_eq!(press(&mut st, &mut game, Key::Escape, NONE), UiResult::Used);
    assert_eq!(st.s.ui.sizing, None);
    assert!(layout_txt(&st).contains("col:0.480("));
    // Again from the keys alone: a letter typed ends it and goes into the
    // code.
    let size = |st: &mut Studio, game: &mut FakeGame| {
        press(st, game, Key::Menu, NONE);
        choose_by_keys(st, game, "Layout");
        choose_by_keys(st, game, "Size...");
        assert_eq!(st.s.ui.sizing, Some((Pane::Code, 3)));
    };
    st.s.editor.set_text("");
    size(&mut st, &mut game);
    type_text(&mut st, &mut game, "q");
    assert_eq!((st.s.ui.sizing, st.s.editor.text().as_str()), (None, "q"));
    // Left isn't the way a bar between groups stacked moves: it ends sizing
    // and moves the code's cursor.
    size(&mut st, &mut game);
    press(&mut st, &mut game, Key::Left, NONE);
    assert_eq!((st.s.ui.sizing, st.s.editor.col), (None, 0));
    // A press ends it.
    size(&mut st, &mut game);
    let code = leaf(&st, Pane::Code).rect;
    click(&mut st, &mut game, middle(code));
    assert_eq!(st.s.ui.sizing, None);
    // So does the game closing the studio, which takes no key: opened
    // again, Down moves the code's cursor, not the bar.
    st.s.editor.set_text("a\nb\nc\n");
    size(&mut st, &mut game);
    st.s.ui.open = false;
    st.s.now += 1.0;
    st.s.ui.open = true;
    st.s.ui.opened_at = st.s.now;
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.sizing, None);
    let ratio = st.s.ui.dock.ratio(3);
    press(&mut st, &mut game, Key::Down, NONE);
    assert_eq!((st.s.editor.row, st.s.ui.dock.ratio(3)), (1, ratio));
    // In an Editor area 600 wide the script list's least holds it 140 wide,
    // wider than its ratio would make it. The arrows step from where its
    // bar is drawn: Left there changes nothing, the ratio kept for a wider
    // area, and one Right widens the list 16 units.
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 634.0, 500.0]);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws.unwrap()[2], 600.0);
    assert_eq!(leaf(&st, Pane::Scripts).rect[2], 140.0);
    let (ratio, written) = (st.s.ui.dock.ratio(1), layout_txt(&st));
    let at = middle(tab_of(&st, Pane::Scripts));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Size...");
    assert_eq!(st.s.ui.sizing, Some((Pane::Scripts, 1)));
    press(&mut st, &mut game, Key::Left, NONE);
    assert_eq!((st.s.ui.dock.ratio(1), layout_txt(&st)), (ratio, written));
    press(&mut st, &mut game, Key::Right, NONE);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Scripts).rect[2], 156.0);
}

#[test]
fn dragging_a_tab_beside_the_code_moves_its_pane() {
    let (mut game, mut st) = editing("dock-drag-beside");
    redraw(&mut st, &mut game);
    // The Agent's tab pressed and moved 3 units: it comes to the front with
    // the keys, but hasn't lifted, so it would land nowhere.
    let from = middle(tab_of(&st, Pane::Agent));
    hold(&mut st, &mut game, from, &[(from.0 + 3.0, from.1)]);
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
    assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (None, None));
    redraw(&mut st, &mut game);
    let (strip, tab) = (leaf(&st, Pane::Agent).strip, tab_of(&st, Pane::Agent));
    let line = [tab[0] + 8.0, strip[1] + strip[3] - 3.0, tab[2] - 16.0, 2.0];
    let held = *colours(&game, "rect", line)
        .last()
        .expect("the line under the front tab");
    assert!(drawn(&game, TIP));
    // In the code's right quarter it lifts: it would land beside the code.
    point(&mut st, &mut game, (890.0, 200.0));
    assert_eq!(st.s.ui.lifted(), Some(Pane::Agent));
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(
        spot.at,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right
        }
    );
    assert_eq!((spot.rect, spot.bar), ([565.0, 80.0, 347.0, 240.0], None));
    // Drawn: the place Agent's group would take, tinted; its tab faded
    // where it was and no label following the pointer; the footer saying
    // how to drop or cancel, and no tip.
    redraw(&mut st, &mut game);
    let accent = st.s.ui.look.palette().accent;
    assert!(
        colours(&game, "rect", spot.rect).contains(&faded(accent, 0.16)),
        "the tint"
    );
    assert!(colours(&game, "border", spot.rect).contains(&accent), "its edge");
    assert_eq!(
        colours(&game, "rect", line).last(),
        Some(&faded(held, 0.5)),
        "the tab at half alpha"
    );
    for (text, at) in texts(&game) {
        assert!(text != "Agent" || inside(tab, at), "a label at {at:?}");
    }
    assert!(drawn(&game, "drop") && drawn(&game, "cancel"));
    assert!(!drawn(&game, TIP));
    assert!(!layout_txt(&st).contains("tips=0"), "nothing moved yet");
    // Back over its own tab it would land where it is: nowhere, and the
    // tab, lit under the pointer, is faded still.
    point(&mut st, &mut game, middle(tab));
    assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (Some(Pane::Agent), None));
    redraw(&mut st, &mut game);
    let ink = st.s.ui.look.palette().ink;
    assert_eq!(colours(&game, "rect", tab), [faded(faded(ink, 0.07), 0.5)]);
    assert!(colours(&game, "border", spot.rect).is_empty(), "no preview");
    // Let go in the code's right quarter: Agent in its own group right of
    // the code, where the preview was, with the keys; written, the tip gone.
    point(&mut st, &mut game, (890.0, 200.0));
    release(&mut st, &mut game);
    assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (None, None));
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Agent), [Pane::Agent]);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    type_text(&mut st, &mut game, "hi");
    assert_eq!(st.s.ui.agent_input.text, "hi");
    let kept = layout_txt(&st);
    assert!(
        kept.contains("col:0.600(row:0.500([code*],[agent*]),[console,problems*])") && kept.contains("tips=0"),
        "{kept}"
    );
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Agent).rect, spot.rect);
    assert!(!drawn(&game, TIP) && !drawn(&game, "cancel"));
}

#[test]
fn a_tab_dropped_on_a_strip_joins_that_group() {
    let (mut game, mut st) = editing("dock-drag-strip");
    redraw(&mut st, &mut game);
    // Problems held over the script list's strip, past its tab: it would
    // join that group after it, the group widening to the Problems' least.
    let (strip, tab) = (leaf(&st, Pane::Scripts).strip, tab_of(&st, Pane::Scripts));
    let problems = tab_of(&st, Pane::Problems);
    let panel = leaf(&st, Pane::Problems).strip;
    let line = [problems[0] + 8.0, panel[1] + panel[3] - 3.0, problems[2] - 16.0, 2.0];
    hold(&mut st, &mut game, middle(problems), &[]);
    redraw(&mut st, &mut game);
    let held = *colours(&game, "rect", line)
        .last()
        .expect("the line under the front tab");
    point(&mut st, &mut game, (tab[0] + tab[2] + 10.0, strip[1] + 16.0));
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(
        spot.at,
        DropAt::Into {
            anchor: Pane::Scripts,
            at: 1
        }
    );
    assert_eq!(spot.rect, [48.0, 80.0, 200.0, 408.0]);
    let bar = [tab[0] + tab[2] - 1.0, strip[1] + 4.0, 2.0, strip[3] - 8.0];
    assert_eq!(spot.bar, Some(bar));
    // Before the release, the bar is drawn where it will go, and the tab
    // lifted from the middle of its strip is faded with its line.
    redraw(&mut st, &mut game);
    let accent = st.s.ui.look.palette().accent;
    assert_eq!(colours(&game, "rect", bar), [accent]);
    assert_eq!(colours(&game, "rect", line).last(), Some(&faded(held, 0.5)));
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Scripts), [Pane::Scripts, Pane::Problems]);
    assert!(st.s.ui.dock.is_front(Pane::Problems));
    assert_eq!(st.s.ui.focus, Pane::Problems);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Console), [Pane::Console, Pane::Agent]);
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Problems).rect, spot.rect);
}

#[test]
fn a_dragged_script_list_leaves_the_keys_where_they_are() {
    let (mut game, mut st) = editing("dock-drag-scripts");
    redraw(&mut st, &mut game);
    // The script list's tab dragged to the right edge: a full-height
    // column there. It takes no keys, so they stay with the code, which
    // typing still reaches.
    let from = middle(tab_of(&st, Pane::Scripts));
    hold(&mut st, &mut game, from, &[(906.0, 300.0)]);
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(spot.at, DropAt::Edge(Side::Right));
    assert_eq!(spot.rect, [698.0, 80.0, 214.0, 408.0]);
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.focus, Pane::Code);
    let kept = layout_txt(&st);
    assert!(
        kept.contains("dock=row:0.750(col:0.600([code*],[console*,problems,agent]),[scripts*])\n"),
        "{kept}"
    );
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "x");
    redraw(&mut st, &mut game);
    assert_eq!(leaf(&st, Pane::Scripts).rect, spot.rect);
}

#[test]
fn esc_letting_go_over_nothing_or_closing_cancels_a_drag() {
    let (mut game, mut st) = editing("dock-drag-cancel");
    redraw(&mut st, &mut game);
    // The Console's tab is at the front already: pressing it changes nothing.
    let console = middle(tab_of(&st, Pane::Console));
    let beside = (890.0, 200.0);
    let unchanged = |st: &Studio, what: &str| {
        assert_eq!(st.s.ui.dock, Dock::default(), "{what}");
        assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (None, None), "{what}");
        assert!(!st.s.dir.join("layout.txt").exists(), "{what}: written");
    };
    // Esc while it is lifted: nothing moves, and the tip is back.
    hold(&mut st, &mut game, console, &[beside]);
    assert!(st.s.ui.dock_spot.is_some());
    redraw(&mut st, &mut game);
    assert!(!drawn(&game, TIP));
    press(&mut st, &mut game, Key::Escape, NONE);
    release(&mut st, &mut game);
    unchanged(&st, "Esc, lifted");
    redraw(&mut st, &mut game);
    assert!(drawn(&game, TIP));
    // Esc before it lifts: moved on after it, it doesn't.
    hold(&mut st, &mut game, console, &[(console.0 + 3.0, console.1)]);
    press(&mut st, &mut game, Key::Escape, NONE);
    point(&mut st, &mut game, beside);
    unchanged(&st, "Esc, pressed");
    release(&mut st, &mut game);
    unchanged(&st, "Esc, pressed, let go");
    // Let go over the gap between the code and the panel: nowhere to land.
    hold(&mut st, &mut game, console, &[(500.0, 324.0)]);
    assert_eq!(st.s.ui.lifted(), Some(Pane::Console));
    assert_eq!(st.s.ui.dock_spot, None);
    release(&mut st, &mut game);
    unchanged(&st, "over a gap");
    // On another tab it lands nowhere, the Editor tab drawn or not, and
    // back on the Editor tab not before it is drawn again.
    hold(&mut st, &mut game, console, &[beside]);
    press(&mut st, &mut game, Key::Char('3'), CTRL);
    point(&mut st, &mut game, (880.0, 210.0));
    assert_eq!(st.s.ui.dock_spot, None, "the Library tab, not drawn yet");
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, beside);
    assert_eq!(st.s.ui.dock_spot, None, "the Library tab");
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    point(&mut st, &mut game, (560.0, 478.0));
    assert_eq!(st.s.ui.dock_spot, None, "the Editor tab, not drawn yet");
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, beside);
    assert!(st.s.ui.dock_spot.is_some(), "the Editor tab drawn: the drag goes on");
    press(&mut st, &mut game, Key::Char('3'), CTRL);
    point(&mut st, &mut game, (880.0, 210.0));
    release(&mut st, &mut game);
    unchanged(&st, "let go on the Library tab");
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    // The Menu key during a drag opens no menu.
    hold(&mut st, &mut game, console, &[beside]);
    press(&mut st, &mut game, Key::Menu, NONE);
    assert!(st.s.ui.menu.is_none());
    press(&mut st, &mut game, Key::Escape, NONE);
    release(&mut st, &mut game);
    unchanged(&st, "the Menu key");
    // Closing the studio during one: opened again, a move shows nothing.
    hold(&mut st, &mut game, console, &[beside]);
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(press(&mut st, &mut game, Key::Toggle, NONE), UiResult::Close);
    unchanged(&st, "closed");
    st.s.now += 1.0;
    st.s.ui.opened_at = st.s.now;
    point(&mut st, &mut game, beside);
    unchanged(&st, "opened again");
    redraw(&mut st, &mut game);
    assert!(colours(&game, "border", spot.rect).is_empty(), "no preview");
    release(&mut st, &mut game);
    unchanged(&st, "opened again, let go");
    // Ctrl+B while one is lifted hides the script list, which ends the drag:
    // nothing is dropped at the release.
    hold(&mut st, &mut game, console, &[beside]);
    press(&mut st, &mut game, Key::Char('b'), CTRL);
    assert!(!st.s.ui.dock.shown(Pane::Scripts));
    assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (None, None));
    release(&mut st, &mut game);
    let mut hidden = Dock::default();
    hidden.set_hidden(Pane::Scripts, true);
    assert_eq!(st.s.ui.dock, hidden);
    assert!(
        layout_txt(&st).contains("dock=row:0.180(-[scripts*],"),
        "{}",
        layout_txt(&st)
    );
}

#[test]
fn a_press_without_a_drag_only_switches_tabs() {
    let (mut game, mut st) = editing("dock-drag-click");
    redraw(&mut st, &mut game);
    // A click on the Agent's tab: it comes to the front with the keys, and
    // that is written at once; nothing moves.
    let agent = middle(tab_of(&st, Pane::Agent));
    click(&mut st, &mut game, agent);
    assert!(st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.focus == Pane::Agent);
    assert_eq!((st.s.ui.lifted(), st.s.ui.dock_spot), (None, None));
    let kept = layout_txt(&st);
    assert!(
        kept.contains("dock=row:0.180([scripts*],col:0.600([code*],[console,problems,agent*]))\n"),
        "{kept}"
    );
    assert!(!kept.contains("tips=0"), "{kept}");
    // Moved less than it takes to lift, it is a click too.
    redraw(&mut st, &mut game);
    let problems = middle(tab_of(&st, Pane::Problems));
    hold(&mut st, &mut game, problems, &[(problems.0 + 4.0, problems.1 + 3.0)]);
    release(&mut st, &mut game);
    assert!(st.s.ui.dock.is_front(Pane::Problems) && st.s.ui.focus == Pane::Problems);
    assert_eq!(
        st.s.ui.dock.leaf_tabs(Pane::Problems),
        [Pane::Console, Pane::Problems, Pane::Agent]
    );
    assert!(layout_txt(&st).contains("[console,problems*,agent]"));
}

#[test]
fn drops_and_shows_never_take_a_panes_place() {
    // An 854 x 480 game's default window: the code takes the room with the
    // panel's strip under it, and there is no room beside the code.
    let (mut game, mut st) = editing("dock-drop-fits");
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    redraw(&mut st, &mut game);
    let px = |(x, y): (f32, f32)| (x * 1.125, y * 1.125);
    let code = leaf(&st, Pane::Code).rect;
    assert!(leaf(&st, Pane::Console).collapsed);
    // The Agent's tab pressed on the panel's strip and dragged at once into
    // the code's right quarter: beside the code it would push the script
    // list out, so it would join the code's tabs, and the preview says so.
    let from = px(middle(tab_of(&st, Pane::Agent)));
    hold(
        &mut st,
        &mut game,
        from,
        &[px((code[0] + code[2] - 30.0, code[1] + code[3] / 2.0))],
    );
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(
        spot.at,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1
        }
    );
    assert_eq!((spot.rect, spot.bar), (code, None));
    // Let go, the script list, the code's group (Agent at its front, with
    // the keys) and the panel's strip (the Problems now at its front) are
    // all still drawn.
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Code), [Pane::Code, Pane::Agent]);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    redraw(&mut st, &mut game);
    let groups =
        |st: &Studio| -> Vec<(Pane, bool)> { st.s.ui.dock_drawn.iter().map(|l| (l.active, l.collapsed)).collect() };
    assert_eq!(
        groups(&st),
        [(Pane::Scripts, false), (Pane::Problems, true), (Pane::Agent, false)]
    );
    assert_eq!(leaf(&st, Pane::Agent).rect, code);
    // A click on the panel's Problems tab gives Problems the room and the
    // keys, the code's group keeping its strip above it; Esc gives the code
    // the room back.
    let problems = px(middle(tab_of(&st, Pane::Problems)));
    click(&mut st, &mut game, problems);
    assert_eq!(st.s.ui.focus, Pane::Problems);
    redraw(&mut st, &mut game);
    assert_eq!(
        groups(&st),
        [(Pane::Scripts, false), (Pane::Agent, true), (Pane::Problems, false)]
    );
    assert!(leaf(&st, Pane::Code).rect[1] < leaf(&st, Pane::Problems).rect[1]);
    press(&mut st, &mut game, Key::Escape, NONE);
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    assert_eq!(
        groups(&st),
        [(Pane::Scripts, false), (Pane::Problems, true), (Pane::Code, false)]
    );
    // As the game draws a frame between the press and the move: the panel's
    // Agent pressed takes the room, the code keeping its strip; over that
    // strip, past the code's tab, Agent would join the code's tabs, and the
    // preview is where the code's group will be.
    st.s.ui.dock = Dock::default();
    redraw(&mut st, &mut game);
    let agent = px(middle(tab_of(&st, Pane::Agent)));
    hold(&mut st, &mut game, agent, &[]);
    redraw(&mut st, &mut game);
    let (strip, code_tab) = (leaf(&st, Pane::Code).strip, tab_of(&st, Pane::Code));
    assert!(leaf(&st, Pane::Code).collapsed);
    point(
        &mut st,
        &mut game,
        px((code_tab[0] + code_tab[2] + 20.0, strip[1] + 16.0)),
    );
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(
        spot.at,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1
        }
    );
    assert_eq!(spot.rect, code);
    assert_eq!(
        spot.bar,
        Some([code_tab[0] + code_tab[2] - 1.0, strip[1] + 4.0, 2.0, strip[3] - 8.0])
    );
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Code), [Pane::Code, Pane::Agent]);
    assert_eq!(st.s.ui.focus, Pane::Agent);
    // A pane shown never takes a place either: Panes › Docs, whose column
    // at the right would push the script list out here, joins the code's
    // tabs, at the front with the keys, every group still drawn.
    st.s.ui.dock = Dock::default();
    press(&mut st, &mut game, Key::Escape, NONE);
    redraw(&mut st, &mut game);
    let at = px(middle(tab_of(&st, Pane::Code)));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Docs");
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Code), [Pane::Code, Pane::Docs]);
    assert!(st.s.ui.dock.is_front(Pane::Docs));
    assert_eq!(st.s.ui.focus, Pane::Docs);
    redraw(&mut st, &mut game);
    assert_eq!(
        groups(&st),
        [(Pane::Scripts, false), (Pane::Console, true), (Pane::Docs, false)]
    );
    // Full screen there is room: a column of its own at the right.
    st.s.ui.dock = Dock::default();
    st.s.ui.look.window = false;
    st.s.ui.look.scale = 1.0;
    press(&mut st, &mut game, Key::Escape, NONE);
    redraw(&mut st, &mut game);
    let at = middle(tab_of(&st, Pane::Code));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Docs");
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Docs), [Pane::Docs]);
    redraw(&mut st, &mut game);
    let ws = st.s.ui.dock_ws.unwrap();
    let docs = leaf(&st, Pane::Docs).rect;
    assert_eq!((docs[0] + docs[2], docs[1], docs[3]), (ws[0] + ws[2], ws[1], ws[3]));
    assert_eq!(st.s.ui.dock_drawn.len(), 4, "every group still drawn");
}

#[test]
fn a_library_with_no_room_opens_its_page() {
    let groups =
        |st: &Studio| -> Vec<(Pane, bool)> { st.s.ui.dock_drawn.iter().map(|l| (l.active, l.collapsed)).collect() };
    // Dock in the Editor, in the Library page's own menu.
    fn dock_from_page(st: &mut Studio, game: &mut FakeGame) {
        st.s.ui.notice = None;
        press(st, game, Key::Char('3'), CTRL);
        redraw(st, game);
        let title = text_at(game, "Script mods");
        right_click(st, game, (title.0 + 2.0, title.1 + 2.0));
        choose(st, game, "Dock in the Editor");
    }
    // The Library's page shows, with a notice; the dock is as it was, and
    // nothing is written.
    let stayed = |st: &Studio| {
        assert_eq!((st.s.ui.tab, &st.s.ui.dock), (Tab::Library, &Dock::default()));
        assert_eq!(notice(st), "No room for the Library in the Editor tab");
        assert!(layout_txt(st).is_empty(), "nothing written");
    };
    // In an 854 x 480 game's default window the Library has no room for
    // its column, and among the code's tabs its least height would leave
    // the panel no room for its strip under them, so its page shows
    // instead. Before the Editor tab is first drawn too (the studio opens
    // on the Console): Dock in the Editor, on the Library page, tries the
    // room the Editor tab has in that window.
    let (mut game, mut st) = editing("dock-library-no-room");
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    press(&mut st, &mut game, Key::Char('1'), CTRL);
    redraw(&mut st, &mut game);
    dock_from_page(&mut st, &mut game);
    stayed(&st);
    // On the Editor tab, Panes › Library opens its page too.
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    let px = |(x, y): (f32, f32)| (x * 1.125, y * 1.125);
    let before = groups(&st);
    assert_eq!(
        before,
        [(Pane::Scripts, false), (Pane::Console, true), (Pane::Code, false)]
    );
    let at = px(middle(tab_of(&st, Pane::Code)));
    st.s.ui.notice = None;
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Library");
    stayed(&st);
    // Back on the Editor tab every group is drawn as before, the code with
    // the keys.
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    assert_eq!(groups(&st), before);
    assert_eq!(st.s.ui.focus, Pane::Code);
    // Dock in the Editor, in the Library page's own menu: its page stays.
    dock_from_page(&mut st, &mut game);
    stayed(&st);
    // Drawn full screen on the Editor tab, then put in the window again
    // from Settings: from the Library page, and from the window's own menu
    // on the Console page, the room tried is the one the Editor tab has
    // now, in the window.
    st.s.ui.look.window = false;
    st.s.ui.look.scale = 1.0;
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws, Some([48.0, 80.0, 864.0, 408.0]));
    press(&mut st, &mut game, Key::Char('5'), CTRL);
    redraw(&mut st, &mut game);
    st.s.ui.look.window = true;
    st.s.ui.look.scale = 1.125;
    dock_from_page(&mut st, &mut game);
    stayed(&st);
    st.s.ui.notice = None;
    press(&mut st, &mut game, Key::Char('1'), CTRL);
    redraw(&mut st, &mut game);
    let name = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (name.0 + 4.0, name.1 + 4.0));
    choose(&mut st, &mut game, "Panes");
    // Panes ›'s marks are the window's too: the panel is only its strip there.
    assert!(item(&st, "Code").checked && !item(&st, "Console").checked);
    choose(&mut st, &mut game, "Library");
    stayed(&st);
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    assert_eq!(groups(&st), before);
    // An Editor area 468 wide: the script list, the code and the panel fit
    // side by side, but the Library among the code's tabs would make that
    // group 350 wide and push the script list out.
    st.s.ui.look.scale = 1.0;
    st.s.ui.look.rect = Some([10.0, 10.0, 502.0, 500.0]);
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.dock_ws.unwrap()[2], 468.0);
    let before = groups(&st);
    assert_eq!(
        before,
        [(Pane::Scripts, false), (Pane::Console, false), (Pane::Code, false)]
    );
    let at = middle(tab_of(&st, Pane::Code));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Library");
    assert_eq!((st.s.ui.tab, &st.s.ui.dock), (Tab::Library, &Dock::default()));
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    assert_eq!(groups(&st), before);
    // The Docs, whose least size the code's group takes in, still join
    // the code's tabs there.
    let at = middle(tab_of(&st, Pane::Code));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Docs");
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Code), [Pane::Code, Pane::Docs]);
    assert_eq!(st.s.ui.focus, Pane::Docs);
}

/// The colour of the plate drawn just before `text` (a pill's), and its alpha.
fn plate_alpha(game: &FakeGame, text: &str) -> u32 {
    let i = drawn_index(game, text);
    let plate = game.drawn[..i]
        .iter()
        .rev()
        .find(|d| d.starts_with("rect "))
        .expect("a plate before the label");
    u32::from_str_radix(plate.rsplit(' ').next().unwrap(), 16).unwrap() >> 24
}

/// Settings, scrolled with Page Down until `text` is drawn.
fn settings_until(st: &mut Studio, game: &mut FakeGame, text: &str) {
    for _ in 0..8 {
        redraw(st, game);
        if drawn(game, text) {
            return;
        }
        press(st, game, Key::PageDown, NONE);
    }
    panic!("{text:?} never drawn in Settings");
}

#[test]
fn reset_layout_in_settings_brings_back_the_default() {
    let (mut game, mut st) = editing("dock-settings-reset");
    redraw(&mut st, &mut game);
    // At the default there is nothing to reset: the pill is greyed (its
    // plate fainter under the pointer) and takes no click.
    press(&mut st, &mut game, Key::Char('5'), CTRL);
    settings_until(
        &mut st,
        &mut game,
        "Kept in look.txt and layout.txt, in the studio's folder.",
    );
    assert!(drawn(&game, "Layout") && drawn(&game, "Reset layout"));
    let at = text_at(&game, "Reset layout");
    point(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    redraw(&mut st, &mut game);
    let idle = plate_alpha(&game, "Reset layout");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert!(st.s.ui.notice.is_none());
    press(&mut st, &mut game, Key::Home, NONE);
    // A move and a hide, each from the panes' own ways.
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    let at = middle(tab_of(&st, Pane::Agent));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Move");
    choose(&mut st, &mut game, "Right of the code");
    press(&mut st, &mut game, Key::Char('b'), CTRL);
    let moved = st.s.ui.dock.clone();
    assert!(!moved.shown(Pane::Scripts) && moved.leaf_tabs(Pane::Agent) == [Pane::Agent]);
    assert!(layout_txt(&st).contains(&moved.to_text()));
    // The Layout row is under the themes, below the screen at first; its
    // pill now resets the panes, written, and offers its undo.
    press(&mut st, &mut game, Key::Char('5'), CTRL);
    redraw(&mut st, &mut game);
    assert!(!drawn(&game, "Reset layout"), "below the screen");
    settings_until(&mut st, &mut game, "Reset layout");
    let at = text_at(&game, "Reset layout");
    point(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    redraw(&mut st, &mut game);
    assert!(plate_alpha(&game, "Reset layout") > idle, "not greyed now");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(st.s.ui.dock, Dock::default());
    assert_eq!(st.s.ui.focus, Pane::Code);
    assert_eq!(notice(&st), "Back to the default layout");
    assert!(layout_txt(&st).contains(&format!("dock={}\n", Dock::default().to_text())));
    redraw(&mut st, &mut game);
    assert!(!drawn(&game, "Reset layout"));
    let at = text_at(&game, "Undo reset layout");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(st.s.ui.dock, moved);
    assert_eq!(notice(&st), "Your layout is back");
    assert!(layout_txt(&st).contains(&moved.to_text()));
    // An executor style keeps its own layout, and says so; its Reset the
    // panes resets them for when a Horizon theme is chosen again.
    st.s.ui.look = Look {
        theme: synapse(),
        motion: false,
        ..Look::default()
    };
    settings_until(&mut st, &mut game, "Reset the panes");
    let said: Vec<String> = texts(&game)
        .into_iter()
        .map(|(t, _)| t)
        .skip_while(|t| t != "Layout")
        .skip(1)
        .take_while(|t| t != "Reset the panes")
        .collect();
    assert_eq!(
        said.join(" "),
        "This style keeps its own layout; the panes move in the Horizon themes"
    );
    let at = text_at(&game, "Reset the panes");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(st.s.ui.dock, Dock::default());
    assert_eq!(notice(&st), "The Horizon themes' panes are back to the default");
}

#[test]
fn library_and_docs_dock_beside_the_code_and_take_keys() {
    let (mut game, mut st) = editing("dock-library-docs");
    st.s.editor.set_text("local a = 1\n");
    redraw(&mut st, &mut game);
    // Panes › Docs: a column of its own at the right, drawing the docs,
    // with the keys.
    let at = middle(tab_of(&st, Pane::Console));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Docs");
    assert_eq!(st.s.ui.focus, Pane::Docs);
    assert!(layout_txt(&st).contains("[docs*]"), "{}", layout_txt(&st));
    redraw(&mut st, &mut game);
    let ws = st.s.ui.dock_ws.unwrap();
    let docs = leaf(&st, Pane::Docs);
    assert_eq!(docs.tabs.len(), 1);
    assert_eq!(
        (docs.rect[0] + docs.rect[2], docs.rect[1], docs.rect[3]),
        (ws[0] + ws[2], ws[1], ws[3])
    );
    assert!(drawn(&game, "Script mods"), "the docs' first heading");
    // Its keys page it, and the wheel over it scrolls it.
    press(&mut st, &mut game, Key::PageDown, NONE);
    let paged = st.s.ui.docs_scroll;
    assert!(paged >= 10, "a page of {paged}");
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, middle(docs.rect));
    handle(&mut st, &mut game, Input::Wheel(-1.0));
    assert!(st.s.ui.docs_scroll > paged);
    assert_eq!(st.s.editor.text(), "local a = 1\n");
    // Panes › Library joins it, at the front with the keys.
    let at = middle(tab_of(&st, Pane::Docs));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Panes");
    choose(&mut st, &mut game, "Library");
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Library), [Pane::Docs, Pane::Library]);
    assert!(st.s.ui.dock.is_front(Pane::Library));
    assert_eq!(st.s.ui.focus, Pane::Library);
    // The keys back on the code, a click on a row of the Library pane
    // chooses that mod and gives the Library the keys: a space turns it
    // off, and nothing is typed into the code.
    press(&mut st, &mut game, Key::Escape, NONE);
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    let kit = mod_index(&st, "uikit");
    assert!(st.s.mods[kit].enabled);
    let row = last_text_at(&game, "UI Kit");
    click(&mut st, &mut game, (row.0 + 2.0, row.1 + 4.0));
    assert_eq!((st.s.ui.library_sel, st.s.ui.focus), (kit, Pane::Library));
    type_text(&mut st, &mut game, " ");
    assert!(!st.s.mods[kit].enabled, "the space turned it off");
    assert_eq!(st.s.editor.text(), "local a = 1\n");
    // The Menu key opens the chosen mod's menu, ending with Layout.
    redraw(&mut st, &mut game);
    press(&mut st, &mut game, Key::Menu, NONE);
    let items = menu_items(&st);
    assert!(items.contains(&"Turn on".to_owned()), "{items:?}");
    assert_eq!(items[items.len() - 2..], ["", "Layout"]);
    press(&mut st, &mut game, Key::Escape, NONE);
    // Enter opens it in the code, which takes the keys.
    press(&mut st, &mut game, Key::Enter, NONE);
    assert_eq!((st.s.editor.title.as_str(), st.s.ui.focus), ("uikit.lua", Pane::Code));
    // At its least width its three pills are drawn whole, and each takes
    // its click.
    st.s.ui.dock.activate(Pane::Library);
    redraw(&mut st, &mut game);
    let library = leaf(&st, Pane::Library);
    assert_eq!(library.rect[2], 350.0);
    let right = library.rect[0] + library.rect[2] - 14.0;
    for pill in ["New mod", "Reload all", "Open folder"] {
        let at = text_at(&game, pill);
        assert!(at.0 + 6.0 * pill.len() as f32 + 14.0 <= right, "{pill} cut");
    }
    let at = text_at(&game, "New mod");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert!(st.s.ui.naming.is_some(), "the New prompt");
    press(&mut st, &mut game, Key::Escape, NONE);
    let at = text_at(&game, "Reload all");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert!(common::log(&st).iter().any(|l| l.starts_with("Reloaded all")));
    let at = text_at(&game, "Open folder");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert_eq!(game.opened.last(), Some(&game.mods_dir()));
    // There the chosen mod's name is drawn whole, its buttons on a line of
    // their own under it.
    game.write_mod("long.lua", "-- name: A Rather Long Name\nprint(1)\n");
    st.scan(&mut game);
    st.s.ui.library_sel = mod_index(&st, "long");
    redraw(&mut st, &mut game);
    let name = text_at(&game, "A Rather Long Name");
    let edit = text_at(&game, "Edit");
    assert!(edit.1 > name.1 + 14.0, "Edit at {edit:?}, the name at {name:?}");
    click(&mut st, &mut game, (edit.0 + 2.0, edit.1 + 4.0));
    assert_eq!(st.s.editor.title, "long.lua");
    // Close on its tab's menu takes it out, the Docs staying.
    redraw(&mut st, &mut game);
    let at = middle(tab_of(&st, Pane::Library));
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Close Library");
    assert!(!st.s.ui.dock.has(Pane::Library));
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Docs), [Pane::Docs]);
    assert!(!layout_txt(&st).contains("library"));
    // Docked again with no mod to list, so no row to open a menu at: the
    // Menu key opens the pane's own menu under its tab, its layout's items
    // with it.
    st.s.mods.clear();
    st.s.ui.dock.show(Pane::Library);
    st.s.ui.focus = Pane::Library;
    redraw(&mut st, &mut game);
    press(&mut st, &mut game, Key::Menu, NONE);
    let items = menu_items(&st);
    assert_eq!(items[0], "New script...");
    for layout in ["Close Library", "Move", "Panes"] {
        assert!(items.contains(&layout.to_owned()), "{layout} not in {items:?}");
    }
    let menu = st.s.ui.menu.as_ref().unwrap();
    let tab = tab_of(&st, Pane::Library);
    assert_eq!(menu.at, [tab[0] + 12.0, tab[1] + tab[3]], "under its tab");
}

#[test]
fn show_in_docs_goes_to_the_docked_docs() {
    let (mut game, mut st) = editing("dock-show-in-docs");
    st.s.editor.set_text("print(player.pos())\n");
    st.s.editor.click(1, 0, false);
    st.s.ui.dock.show(Pane::Docs);
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    let show = |st: &mut Studio, game: &mut FakeGame| {
        redraw(st, game);
        let pos = last_text_at(game, "pos");
        right_click(st, game, (pos.0 + 2.0, pos.1 + 4.0));
        choose(st, game, "Show player.pos in Docs");
    };
    // Docked at the right: the Editor tab stays, the Docs come to its entry,
    // and the keys stay in the code.
    show(&mut st, &mut game);
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert_eq!(st.s.ui.focus, Pane::Code);
    redraw(&mut st, &mut game);
    assert!(st.s.ui.docs_scroll > 0);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text player.pos(")),
        "the entry is in the pane"
    );
    type_text(&mut st, &mut game, "x");
    assert!(st.s.editor.text().contains('x') && st.s.editor.dirty);
    st.s.editor.set_text("print(player.pos())\n");
    // Among the code's tabs, behind the code: it comes to the front with
    // the keys, as the Docs tab would.
    st.s.ui.dock.move_pane(
        Pane::Docs,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
    st.s.ui.dock.activate(Pane::Code);
    st.s.ui.docs_scroll = 0;
    show(&mut st, &mut game);
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert!(st.s.ui.dock.is_front(Pane::Docs));
    assert_eq!(st.s.ui.focus, Pane::Docs);
    redraw(&mut st, &mut game);
    assert!(st.s.ui.docs_scroll > 0);
    // Docked where it has no room (a column beside the code in a narrow
    // window): the Docs tab.
    press(&mut st, &mut game, Key::Escape, NONE);
    st.s.ui.dock = Dock::default();
    st.s.ui.dock.show(Pane::Docs);
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 560.0, 400.0]);
    show(&mut st, &mut game);
    assert_eq!(st.s.ui.tab, Tab::Docs);
    assert!(!st.s.ui.dock_front[Pane::Docs.index()]);
    // Closed: the Docs tab, as ever.
    st.s.ui.dock = Dock::default();
    st.s.ui.look.window = false;
    st.s.ui.tab = Tab::Editor;
    show(&mut st, &mut game);
    assert_eq!(st.s.ui.tab, Tab::Docs);
    assert!(!st.s.ui.dock.has(Pane::Docs));
}

#[test]
fn the_console_library_and_docs_tabs_drag_into_the_editor() {
    let (mut game, mut st) = editing("dock-top-tabs");
    redraw(&mut st, &mut game);
    let code = leaf(&st, Pane::Code).rect;
    let quarter = (code[0] + code[2] - 30.0, code[1] + code[3] / 2.0);
    // The Library's tab pressed shows its page, as ever; dragged, it goes
    // to the Editor tab with its pane, which lands where it is let go.
    let tab = text_at(&game, "Library");
    let from = (tab.0 + 4.0, tab.1 + 4.0);
    hold(&mut st, &mut game, from, &[]);
    assert_eq!(st.s.ui.tab, Tab::Library);
    point(&mut st, &mut game, (from.0 + 3.0, from.1));
    assert_eq!(st.s.ui.tab, Tab::Library, "not lifted yet");
    point(&mut st, &mut game, (from.0, from.1 + 8.0));
    assert_eq!(st.s.ui.tab, Tab::Editor);
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, quarter);
    let spot = st.s.ui.dock_spot.expect("a place to land");
    assert_eq!(
        spot.at,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right
        }
    );
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.tab, Tab::Editor, "its page left behind");
    assert_eq!(st.s.ui.focus, Pane::Library);
    redraw(&mut st, &mut game);
    let (library, code) = (leaf(&st, Pane::Library).rect, leaf(&st, Pane::Code).rect);
    assert_eq!(library[1], code[1]);
    assert_eq!(library[0], code[0] + code[2] + 8.0);
    assert!(layout_txt(&st).contains("[library*]"));
    // The Docs' tab dragged, then Esc: its page again, nothing docked.
    let docked = st.s.ui.dock.clone();
    let tab = text_at(&game, "Docs");
    hold(
        &mut st,
        &mut game,
        (tab.0 + 4.0, tab.1 + 4.0),
        &[(tab.0 + 4.0, tab.1 + 40.0)],
    );
    assert_eq!(st.s.ui.tab, Tab::Editor);
    redraw(&mut st, &mut game);
    let at = middle(leaf(&st, Pane::Code).rect);
    point(&mut st, &mut game, at);
    assert!(st.s.ui.dock_spot.is_some());
    press(&mut st, &mut game, Key::Escape, NONE);
    assert_eq!(st.s.ui.tab, Tab::Docs);
    assert!(st.s.ui.dock_spot.is_none());
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock, docked);
    assert_eq!(st.s.ui.tab, Tab::Docs);
    // Let go over nothing (a gap), the same.
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    let gap = bar(&st, 1).rect;
    let tab = text_at(&game, "Docs");
    hold(
        &mut st,
        &mut game,
        (tab.0 + 4.0, tab.1 + 4.0),
        &[(tab.0 + 4.0, tab.1 + 40.0)],
    );
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, middle(gap));
    release(&mut st, &mut game);
    assert_eq!((st.s.ui.tab, &st.s.ui.dock), (Tab::Docs, &docked));
    // The Console's tab: its pane moves, here onto the script list's strip.
    st.s.ui.dock = Dock::default();
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    let strip = leaf(&st, Pane::Scripts).strip;
    let tab = text_at(&game, "Console");
    hold(
        &mut st,
        &mut game,
        (tab.0 + 4.0, tab.1 + 4.0),
        &[(tab.0 + 4.0, tab.1 + 40.0)],
    );
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, (strip[0] + strip[2] - 10.0, strip[1] + 16.0));
    release(&mut st, &mut game);
    assert_eq!(st.s.ui.dock.leaf_tabs(Pane::Console), [Pane::Scripts, Pane::Console]);
    assert_eq!((st.s.ui.tab, st.s.ui.focus), (Tab::Editor, Pane::Console));
    // Dropped at its own place in its group, behind the Agent there, it
    // comes to the front, and its page is left behind all the same.
    st.s.ui.dock = Dock::default();
    press(&mut st, &mut game, Key::Char('2'), CTRL);
    redraw(&mut st, &mut game);
    let agent = middle(tab_of(&st, Pane::Agent));
    click(&mut st, &mut game, agent);
    assert!(layout_txt(&st).contains("[console,problems,agent*]"));
    redraw(&mut st, &mut game);
    let own = tab_of(&st, Pane::Console);
    let tab = text_at(&game, "Console");
    hold(
        &mut st,
        &mut game,
        (tab.0 + 4.0, tab.1 + 4.0),
        &[(tab.0 + 4.0, tab.1 + 40.0)],
    );
    redraw(&mut st, &mut game);
    point(&mut st, &mut game, (own[0] + 4.0, own[1] + 13.0));
    release(&mut st, &mut game);
    assert_eq!(
        st.s.ui.dock.leaf_tabs(Pane::Console),
        [Pane::Console, Pane::Problems, Pane::Agent]
    );
    assert!(st.s.ui.dock.is_front(Pane::Console));
    assert_eq!((st.s.ui.tab, st.s.ui.focus), (Tab::Editor, Pane::Console));
    // The Settings tab doesn't drag: it is a page, and only shows.
    let before = st.s.ui.dock.clone();
    redraw(&mut st, &mut game);
    let tab = text_at(&game, "Settings");
    hold(
        &mut st,
        &mut game,
        (tab.0 + 4.0, tab.1 + 4.0),
        &[(tab.0 + 4.0, tab.1 + 120.0)],
    );
    assert_eq!(st.s.ui.tab, Tab::Settings);
    release(&mut st, &mut game);
    assert_eq!((st.s.ui.tab, &st.s.ui.dock), (Tab::Settings, &before));
    // Dock in the Editor, in the Library list's own menu.
    st.s.ui.dock = Dock::default();
    st.s.ui.tab = Tab::Library;
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Script mods");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    choose(&mut st, &mut game, "Dock in the Editor");
    assert_eq!((st.s.ui.tab, st.s.ui.focus), (Tab::Editor, Pane::Library));
    assert!(st.s.ui.dock.is_front(Pane::Library));
    // Docked, the page's menu shows it there instead.
    st.s.ui.tab = Tab::Library;
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Script mods");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(menu_items(&st).last().unwrap(), "Show in the Editor");
}

#[test]
fn a_groups_tabs_stay_on_its_strip() {
    // Four tabs in a group beside the code, its bar dragged as far as it
    // goes: the group keeps room for them, left of its ×.
    let (mut game, mut st) = editing("dock-tabs-fit");
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Console,
            at: 3,
        },
    );
    st.s.ui.dock.move_pane(
        Pane::Console,
        DropAt::Beside {
            anchor: Pane::Code,
            side: Side::Right,
        },
    );
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Console,
            at: 1,
        },
    );
    st.s.ui.dock.move_pane(
        Pane::Problems,
        DropAt::Into {
            anchor: Pane::Console,
            at: 2,
        },
    );
    st.s.ui.dock.move_pane(
        Pane::Agent,
        DropAt::Into {
            anchor: Pane::Console,
            at: 3,
        },
    );
    assert_eq!(
        st.s.ui.dock.leaf_tabs(Pane::Console),
        [Pane::Console, Pane::Scripts, Pane::Problems, Pane::Agent]
    );
    redraw(&mut st, &mut game);
    let b = *st
        .s
        .ui
        .dock_bars
        .iter()
        .find(|b| b.row)
        .expect("the bar beside the code");
    let from = middle(b.rect);
    drag(&mut st, &mut game, from, (from.0 + 900.0, from.1));
    redraw(&mut st, &mut game);
    let group = leaf(&st, Pane::Console);
    assert!(group.rect[2] < 320.0, "{group:?} as narrow as it goes");
    let cross = group.strip[0] + group.strip[2] - 34.0;
    for (p, r) in &group.tabs {
        assert!(r[0] + r[2] <= cross, "{p:?}'s tab {r:?} runs past the × at {cross}");
    }
}

#[test]
fn the_docs_say_how_the_panes_work() {
    use horizon_studio::game::hstudio_look::docs_markdown;
    let docs = docs_markdown();
    // Their own section, after the looks and before the agents.
    let (looks, panes, agents) = (
        docs.find("## Looks").unwrap(),
        docs.find("## The Editor's panes").expect("the panes' section"),
        docs.find("## AI agents").unwrap(),
    );
    assert!(looks < panes && panes < agents);
    let section = &docs[panes..agents];
    for said in [
        "Drag a tab to move its pane",
        "The Console, Library and Docs tabs at the top drag in the same way.",
        "Ctrl+B the script list",
        "Move ›, Size..., Panes › and Reset layout",
        "the Menu key's menus end with Layout ›",
        "Esc gives them back to the code",
        "layout.txt",
        "Settings stays a page",
    ] {
        assert!(section.contains(said), "{said:?} not in {section}");
    }
    // The executor styles say theirs are fixed, and the agents' words are
    // the panes'.
    assert!(docs.contains("Their layouts are fixed: the panes move in the Horizon themes."));
    assert!(docs.contains("The Agent pane says what it did"));
    assert!(docs.contains("The code's tab strip counts the notes"));
    assert!(docs.contains("in the note where that pane isn't shown, as in the executor styles"));
}

/// The grip drawn on the bar at `bar` (dp): its rect.
fn grip_on(game: &FakeGame, bar: [f32; 4]) -> [f32; 4] {
    game.drawn
        .iter()
        .filter_map(|d| {
            let rest = d.strip_prefix("rect [")?;
            let (rect, style) = rest.split_once("] ")?;
            if !style.starts_with("r1.5 ") {
                return None;
            }
            let n: Vec<f32> = rect.split(", ").map(|v| v.parse().unwrap()).collect();
            Some([n[0], n[1], n[2], n[3]])
        })
        .find(|r| r[0] >= bar[0] && r[1] >= bar[1] && r[0] + r[2] <= bar[0] + bar[2] && r[1] + r[3] <= bar[1] + bar[3])
        .expect("a grip on the bar")
}

#[test]
fn a_dragged_tab_lights_no_grip() {
    // A bar's grip widens under the pointer, as it can be pressed there;
    // under a lifted tab, which can't land on it, it doesn't.
    let (mut game, mut st) = editing("dock-drag-grip");
    redraw(&mut st, &mut game);
    let gap = bar(&st, 3).rect;
    point(&mut st, &mut game, middle(gap));
    redraw(&mut st, &mut game);
    assert_eq!(grip_on(&game, gap)[2], 60.0, "widened under the pointer");
    let agent = middle(tab_of(&st, Pane::Agent));
    hold(&mut st, &mut game, agent, &[(agent.0, agent.1 - 40.0)]);
    redraw(&mut st, &mut game);
    let gap = bar(&st, 3).rect;
    point(&mut st, &mut game, middle(gap));
    assert_eq!(st.s.ui.lifted(), Some(Pane::Agent));
    redraw(&mut st, &mut game);
    assert_eq!(grip_on(&game, gap)[2], 36.0, "not under a lifted tab");
    press(&mut st, &mut game, Key::Escape, NONE);
    release(&mut st, &mut game);
}
