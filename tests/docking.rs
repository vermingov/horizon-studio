// The Editor tab's panes on the studio's screen: where they are drawn,
// sizing them with their bars and from the keys, moving them from their
// menus, what has the keys (and that it is always a pane that is drawn),
// hiding and showing their groups, the layout kept in layout.txt, the
// coach's and the agents' ways with them, small windows, and each pane's
// body kept to its place (FakeGame records the game's clips).

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
    // The script list dropped among the code's tabs comes to the front: the
    // keys go nowhere, so nothing reaches the code drawn under it.
    st.s.ui.dock.move_pane(
        Pane::Scripts,
        DropAt::Into {
            anchor: Pane::Code,
            at: 1,
        },
    );
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
    // Panes: every pane, those drawn at the front marked; one chosen comes to
    // the front with the keys, nothing closed.
    choose(&mut st, &mut game, "Panes");
    assert_eq!(menu_items(&st), ["Scripts", "Code", "Console", "Problems", "Agent"]);
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
    for (scale, window, rect) in sizes {
        for focus in [Pane::Code, Pane::Console, Pane::Agent, Pane::Problems] {
            let what = format!("scale {scale}, window {window}, {rect:?}, {focus:?}");
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
            st.s.ui.dock.activate(focus);
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
                    _ => {}
                }
            }
            if focus == Pane::Code
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
