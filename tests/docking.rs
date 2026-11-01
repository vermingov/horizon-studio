// The Editor tab's panes on the studio's screen: what each pane's keys
// move by, the live check wherever the code is, and each pane's body kept
// to its place (FakeGame records the game's clips).

mod common;

use common::{FakeGame, mod_index, studio};
use horizon_studio::game::hstudio::{LogKind, Studio};
use horizon_studio::game::hstudio_coach::{self as coach, By};
use horizon_studio::game::hstudio_theme::{Layout, Look, THEMES};
use horizon_studio::game::hstudio_ui::{Input, Key, Mods, Tab, UiResult, draw, handle};
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

fn key(k: Key, mods: Mods) -> Input {
    Input::Key {
        key: k,
        down: true,
        repeat: false,
        mods,
    }
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

/// Where `text` was drawn: the first time, or the last.
fn drawn_at(game: &FakeGame, text: &str, last: bool) -> (f32, f32) {
    let prefix = format!("text {text} @[");
    let mut found = game.drawn.iter().filter(|d| d.starts_with(&prefix));
    let line = if last { found.next_back() } else { found.next() };
    let at: Vec<f32> = line
        .unwrap_or_else(|| panic!("{text:?} not drawn"))
        .split(" @[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split(", ")
        .map(|v| v.parse().unwrap())
        .collect();
    (at[0], at[1])
}

fn text_at(game: &FakeGame, text: &str) -> (f32, f32) {
    drawn_at(game, text, false)
}

/// The first record of `text` drawn, by its place among the records.
fn drawn_index(game: &FakeGame, text: &str) -> usize {
    let prefix = format!("text {text} @[");
    game.drawn
        .iter()
        .position(|d| d.starts_with(&prefix))
        .unwrap_or_else(|| panic!("{text:?} not drawn"))
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

/// A studio open on its Editor tab, full screen, nothing moving.
fn editing(name: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    (game, st)
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
    // Full screen the code shows 9 rows over the panel: the Editor area's
    // 408 less the header's 46, the panel's 150 and the bar's 8, less its
    // margins' 24, in rows of 20.
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.editor.row, 9);
    // The panel's console shows 3 lines: the panel's 150 less its strip, its
    // insets and its input line, in lines of 20. Its keys page by those, not
    // by the code's rows.
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Click to run Lua here.");
    click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert_eq!(st.s.ui.log_scroll, 3, "the console's lines, not the code's 9");
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert_eq!(st.s.ui.log_scroll, 6);
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.ui.log_scroll, 3);
    // Back on the code, its page is still its own.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageDown, NONE));
    assert_eq!(st.s.editor.row, 18);
    assert_eq!(st.s.ui.log_scroll, 3);
    // The Console tab's console is the large one: its keys page by its own.
    handle(&mut st, &mut game, key(Key::Char('1'), CTRL));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::PageUp, NONE));
    assert!(st.s.ui.log_scroll > 6, "{}", st.s.ui.log_scroll);
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
}

#[test]
fn pane_bodies_are_clipped_to_their_place() {
    let (mut game, mut st) = editing("dock-clips");
    st.s.editor.set_text(&"local a = 1\n".repeat(8));
    redraw(&mut st, &mut game);
    // The script column, the panel under its strip, and the code, in the
    // order they are drawn.
    let code = st.s.ui.coach.code_at.expect("the code is drawn");
    assert_eq!(code, [213.0, 126.0, 699.0, 204.0]);
    let found = clips(&game);
    assert_eq!(
        found.iter().map(|c| c.2).collect::<Vec<_>>(),
        [[48.0, 126.0, 155.0, 362.0], [213.0, 370.0, 699.0, 118.0], code]
    );
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
    let code_clip = clips(&game).iter().find(|c| c.2 == code).expect("the code's clip").0;
    assert!(
        drawn_index(&game, "cing") > code_clip,
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
    let panel_clip = clips(&game)
        .iter()
        .find(|c| c.2 == [213.0, 370.0, 699.0, 118.0])
        .expect("the panel's clip")
        .0;
    assert!(
        drawn_index(&game, "yer") > panel_clip,
        "the console's suggestion after the clip"
    );
    // The executor styles' layouts have no panes: nothing of theirs is clipped.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let synapse = THEMES.iter().position(|t| t.layout == Layout::Synapse).unwrap();
    st.s.ui.look = Look {
        theme: synapse,
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
    let agent = By {
        name: "Claude Code".into(),
        conn: 1,
    };
    coach::call(
        &mut st,
        &mut game,
        &agent,
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
fn what_a_pane_draws_past_its_place_takes_no_click() {
    // A short window: the script column is about 30 high, but always shows
    // a row, which is drawn past it.
    let (mut game, mut st) = editing("dock-past");
    st.s.ui.look.window = true;
    st.s.ui.look.rect = Some([10.0, 10.0, 700.0, 220.0]);
    redraw(&mut st, &mut game);
    // The column is as high as the code beside it.
    let code = st.s.ui.coach.code_at.expect("the code is drawn");
    let row = drawn_at(&game, "scratch.lua", true);
    assert!(
        row.1 >= code[1] + code[3],
        "the row {row:?} is past the column, {code:?}"
    );
    // Its menu, which a right click on it would open, doesn't: the click is
    // past the column.
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 4.0));
    assert!(st.s.ui.menu.is_none());
    let found = clips(&game);
    assert_eq!(found.len(), 2, "the script column and the code: {found:?}");
    assert_eq!((found[0].2[1], found[0].2[3]), (code[1], code[3]));
    // Where the column has room, its row's menu opens as ever.
    st.s.ui.look.window = false;
    redraw(&mut st, &mut game);
    let row = drawn_at(&game, "scratch.lua", true);
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 4.0));
    assert!(st.s.ui.menu.is_some());
}
