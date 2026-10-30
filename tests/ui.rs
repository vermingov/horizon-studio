// The studio's screen: the editor's buffer, highlighting, completion, and
// keys and clicks routed through `handle`.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::ModState;
use horizon_studio::game::hstudio_ui::{
    Editor, Input, Key, LineEdit, Long, Mods, Tab, Tint, UiResult, complete, draw, handle, highlight,
};

fn editor(text: &str) -> Editor {
    let mut e = Editor::default();
    e.set_text(text);
    e
}

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

#[test]
fn typing_and_deleting() {
    let mut e = editor("");
    for c in "local x = 1".chars() {
        e.insert(&c.to_string());
    }
    assert_eq!(e.text(), "local x = 1");
    e.backspace(false);
    e.insert("42");
    assert_eq!(e.text(), "local x = 42");
    e.backspace(true);
    assert_eq!(e.text(), "local x = ");
    e.home(false);
    e.delete(true);
    assert_eq!(e.text(), " x = ");
    e.undo();
    assert_eq!(e.text(), "local x = ");
    e.undo();
    assert_eq!(e.text(), "local x = 42");
    e.undo();
    assert_eq!(e.text(), "local x = ");
    e.undo();
    assert_eq!(e.text(), "local x = 1");
    e.redo();
    assert_eq!(e.text(), "local x = ");
    assert!(e.dirty);
}

#[test]
fn enter_keeps_and_adds_indent() {
    let mut e = editor("if x then");
    e.end(false);
    e.newline();
    e.insert("y()");
    e.newline();
    assert_eq!(e.text(), "if x then\n  y()\n  ");
    e.backspace(false);
    assert_eq!(e.text(), "if x then\n  y()\n", "backspace takes a whole indent");
    let mut f = editor("on('tick', function()");
    f.end(false);
    f.newline();
    assert_eq!(f.text(), "on('tick', function()\n  ");
}

#[test]
fn moving_keeps_the_column() {
    let mut e = editor("abcdef\nab\nabcdef");
    e.end(false);
    e.move_v(1, false);
    assert_eq!((e.row, e.col), (1, 2));
    e.move_v(1, false);
    assert_eq!((e.row, e.col), (2, 6), "the goal column comes back");
    e.move_v(1, false);
    assert_eq!((e.row, e.col), (2, 6));
    e.move_h(true, false, false);
    assert_eq!((e.row, e.col), (2, 6));
    e.home(false);
    e.move_h(false, false, false);
    assert_eq!((e.row, e.col), (1, 2), "left at a line start goes up");
    let mut w = editor("foo.bar(baz)");
    w.move_h(true, true, false);
    assert_eq!(w.col, 3);
    w.move_h(true, true, false);
    assert_eq!(w.col, 7);
}

#[test]
fn selection_copy_cut_paste() {
    let mut e = editor("one two\nthree");
    e.move_h(true, true, true);
    assert_eq!(e.selected_text().as_deref(), Some("one"));
    e.move_v(1, true);
    assert_eq!(e.selected_text().as_deref(), Some("one two\nthr"));
    let cut = e.cut();
    assert_eq!(cut, "one two\nthr");
    assert_eq!(e.text(), "ee");
    e.insert("x\ny");
    assert_eq!(e.text(), "x\nyee");
    assert_eq!((e.row, e.col), (1, 1));
    let mut c = editor("line one\nline two");
    assert_eq!(c.copy(), "line one\n", "no selection copies the line");
    c.cut();
    assert_eq!(c.text(), "line two");
    c.select_all();
    c.insert("all");
    assert_eq!(c.text(), "all");
    // Typing over a selection replaces it.
    let mut r = editor("hello");
    r.move_h(true, true, true);
    r.insert("j");
    assert_eq!(r.text(), "j");
}

#[test]
fn comment_indent_duplicate() {
    let mut e = editor("a()\n  b()\n\nc()");
    e.select_all();
    e.toggle_comment();
    assert_eq!(e.text(), "-- a()\n  -- b()\n\n-- c()");
    e.toggle_comment();
    assert_eq!(e.text(), "a()\n  b()\n\nc()");
    e.select_all();
    e.indent(false);
    assert_eq!(e.text(), "  a()\n    b()\n  \n  c()");
    e.indent(true);
    e.indent(true);
    assert_eq!(e.text(), "a()\nb()\n\nc()");
    let mut d = editor("x = 1");
    d.duplicate_line();
    assert_eq!(d.text(), "x = 1\nx = 1");
    assert_eq!(d.row, 1);
}

#[test]
fn utf8_is_safe() {
    let mut e = editor("é€x");
    e.end(false);
    e.backspace(false);
    e.backspace(false);
    assert_eq!(e.text(), "é");
    e.move_h(false, false, false);
    e.insert("ü");
    assert_eq!(e.text(), "üé");
    e.click(0, 1, false);
    assert_eq!(e.col, "ü".len());
    e.set_text("tab\there\r\nnext");
    assert_eq!(e.lines, vec!["tab  here", "next"]);
}

#[test]
fn line_edit() {
    let mut l = LineEdit::default();
    l.insert("print(1)\n");
    assert_eq!(l.text, "print(1) ");
    l.left(true);
    l.insert("x");
    assert_eq!(l.text, "print(x1) ");
    l.backspace(true);
    assert_eq!(l.text, "print(1) ");
}

#[test]
fn lua_highlighting() {
    let line = "local s = \"hi -- not a comment\" -- a comment";
    let (pieces, after) = highlight(line, Long::None);
    let tints: Vec<(&str, Tint)> = pieces
        .iter()
        .map(|(a, b, t)| (&line[*a..*b], *t))
        .filter(|(s, _)| !s.trim().is_empty())
        .collect();
    assert_eq!(tints[0], ("local", Tint::Keyword));
    assert_eq!(tints[1], ("s", Tint::Plain));
    assert!(tints.contains(&("\"hi -- not a comment\"", Tint::Str)));
    assert_eq!(tints.last().unwrap(), &("-- a comment", Tint::Comment));
    assert_eq!(after, Long::None);
    let (p, after) = highlight("x = [[start", Long::None);
    assert_eq!(after, Long::Str(0));
    assert_eq!(p.last().unwrap().2, Tint::Str);
    let (p, after) = highlight("end]] .. 12", after);
    assert_eq!(after, Long::None);
    assert_eq!(p[0].2, Tint::Str);
    assert!(
        p.iter()
            .any(|(a, b, t)| *t == Tint::Number && &"end]] .. 12"[*a..*b] == "12")
    );
    let (_, after) = highlight("--[==[ long", Long::None);
    assert_eq!(after, Long::Comment(2));
    let (p, _) = highlight("player.pos() t.player", Long::None);
    let line = "player.pos() t.player";
    assert!(
        p.iter()
            .any(|(a, b, t)| *t == Tint::Global && &line[*a..*b] == "player")
    );
    assert!(p.iter().any(|(a, b, t)| *t == Tint::Plain && &line[*a..*b] == "pos"));
    assert_eq!(
        p.iter().filter(|(_, _, t)| *t == Tint::Global).count(),
        1,
        "t.player is a field, not the API"
    );
}

#[test]
fn completion() {
    assert_eq!(complete("player.fac", 10).as_deref(), Some("ing("));
    // pos and pose: as far as they agree.
    assert_eq!(complete("player.po", 9).as_deref(), Some("s"));
    assert_eq!(complete("pla", 3).as_deref(), Some("yer."));
    assert_eq!(complete("hud.pa", 6).as_deref(), Some("nel("));
    assert_eq!(complete("x = ag", 6).as_deref(), Some("ent."));
    assert_eq!(complete("", 0), None);
    assert_eq!(complete("zzz", 3), None);
}

#[test]
fn console_through_keys() {
    let mut game = FakeGame::new("ui-console");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    for c in "1 + 41".chars() {
        handle(&mut st, &mut game, Input::Text(c.to_string()));
    }
    assert_eq!(handle(&mut st, &mut game, key(Key::Enter, NONE)), UiResult::Used);
    assert_eq!(log(&st).last().map(String::as_str), Some("42"));
    assert!(st.s.ui.input.text.is_empty());
    handle(&mut st, &mut game, key(Key::Up, NONE));
    assert_eq!(st.s.ui.input.text, "1 + 41");
    handle(&mut st, &mut game, key(Key::Down, NONE));
    assert_eq!(st.s.ui.input.text, "");
    for c in "player.fa".chars() {
        handle(&mut st, &mut game, Input::Text(c.to_string()));
    }
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.ui.input.text, "player.facing(");
    game.clipboard = "print('pasted')".into();
    handle(&mut st, &mut game, key(Key::Char('a'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('x'), CTRL));
    assert_eq!(game.clipboard, "player.facing(");
    handle(&mut st, &mut game, Input::Text("dump(1)".into()));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(log(&st).last().map(String::as_str), Some("\"1\""));
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Close);
    assert_eq!(handle(&mut st, &mut game, key(Key::Toggle, NONE)), UiResult::Close);
}

#[test]
fn editor_saves_and_the_mod_reloads() {
    let mut game = FakeGame::new("ui-editor");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    st.s.ui.tab = Tab::Editor;
    assert_eq!(st.s.editor.title, "hello.lua");
    handle(&mut st, &mut game, key(Key::End, CTRL));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    handle(&mut st, &mut game, Input::Text("print('edited in game')".into()));
    assert!(st.s.editor.dirty);
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    assert!(!st.s.editor.dirty);
    assert!(
        log(&st).contains(&"[hello] edited in game".to_owned()),
        "{:?}",
        log(&st)
    );
    // A syntax error marks its line at once and stops the mod.
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    handle(&mut st, &mut game, Input::Text("x = = 1".into()));
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    let row = st.s.editor.row + 1;
    assert_eq!(st.s.editor.error_line, Some(row));
    assert!(matches!(st.s.mods[i].state, ModState::Failed(..)));
    // Ctrl+Z, save: back to running.
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    // Ctrl+Enter runs the buffer in the console.
    st.s.editor.set_text("print('ran ' .. 2 * 3)");
    handle(&mut st, &mut game, key(Key::Enter, CTRL));
    assert_eq!(log(&st).last().map(String::as_str), Some("ran 6"));
}

#[test]
fn library_keys_and_the_new_mod_prompt() {
    let mut game = FakeGame::new("ui-library");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    assert_eq!(st.s.ui.tab, Tab::Library);
    handle(&mut st, &mut game, Input::Text(" ".into()));
    assert!(!st.s.mods[0].enabled);
    handle(&mut st, &mut game, Input::Text(" ".into()));
    assert!(st.s.mods[0].running());
    handle(&mut st, &mut game, Input::Text("n".into()));
    assert!(st.s.ui.naming.is_some());
    handle(&mut st, &mut game, Input::Text("Night Light".into()));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.naming.is_none());
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert_eq!(st.s.editor.title, "night_light.lua");
    let j = mod_index(&st, "night_light");
    assert!(st.s.mods[j].running());
    assert_eq!(st.s.mods[j].name, "Night Light");
    // The template's command works.
    st.command("night_light");
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("Hello from Night Light!"));
}

#[test]
fn clicks_hit_what_was_drawn() {
    let mut game = FakeGame::new("ui-clicks");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    draw(&mut st, &mut game);
    // The tabs, from the drawn labels' positions.
    let library = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text Library @"))
        .cloned()
        .expect("tab drawn");
    let at: Vec<f32> = library
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap()
        .split(", ")
        .map(|v| v.parse().unwrap())
        .collect();
    handle(&mut st, &mut game, Input::Mouse(at[0] + 3.0, at[1] + 3.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(st.s.ui.tab, Tab::Library);
    // Each tab draws, with a mod failing and an empty folder too.
    game.write_mod("broken.lua", "error('broken on purpose')\n");
    run_for(&mut st, &mut game, 0.6);
    for tab in [Tab::Console, Tab::Editor, Tab::Library, Tab::Docs] {
        st.s.ui.tab = tab;
        st.s.ui.library_sel = mod_index(&st, "broken");
        game.drawn.clear();
        draw(&mut st, &mut game);
        assert!(game.drawn.len() > 10, "{tab:?} drew {}", game.drawn.len());
    }
    st.s.ui.tab = Tab::Library;
    game.drawn.clear();
    draw(&mut st, &mut game);
    assert!(
        game.drawn.iter().any(|d| d.contains("broken on purpose")),
        "the error shows"
    );
    // A click in the editor's text moves the cursor.
    st.s.ui.tab = Tab::Editor;
    st.s.editor.set_text("abcdef\nsecond line");
    draw(&mut st, &mut game);
    let num = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text abcdef @"))
        .cloned()
        .expect("line drawn");
    let x: f32 = num
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let y: f32 = num
        .split(", ")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    handle(&mut st, &mut game, Input::Mouse(x + 6.0 * 3.0 + 1.0, y + 12.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!((st.s.editor.row, st.s.editor.col), (1, 3));
    assert_eq!(st.s.editor.selection(), None);
    // A drag selects.
    handle(&mut st, &mut game, Input::Mouse(x, y + 1.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(x + 6.0 * 4.0, y + 1.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("abcd"));
}

#[test]
fn the_ai_setup_prompt_copies_from_the_header() {
    let mut game = FakeGame::new("ui-ai-prompt");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    draw(&mut st, &mut game);
    let drawn = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text Copy AI setup prompt @"))
        .cloned()
        .expect("the button shows while no agent is connected");
    let at: Vec<f32> = drawn
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap()
        .split(", ")
        .map(|v| v.parse().unwrap())
        .collect();
    handle(&mut st, &mut game, Input::Mouse(at[0] + 3.0, at[1] + 3.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(game.clipboard, horizon_studio::game::hstudio_link::SETUP_PROMPT);
    assert!(game.clipboard.contains("AI_SETUP.md"));
    game.drawn.clear();
    draw(&mut st, &mut game);
    assert!(game.drawn.iter().any(|d| d.starts_with("text Copied @")), "it says so");
}

/// Where a drawn text starts, from the fake game's record of it.
fn text_at(game: &FakeGame, text: &str) -> (f32, f32) {
    let line = game
        .drawn
        .iter()
        .find(|d| d.starts_with(&format!("text {text} @")))
        .unwrap_or_else(|| panic!("{text:?} not drawn"));
    let at: Vec<f32> = line
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

fn click(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, (x, y): (f32, f32)) {
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

fn redraw(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame) {
    game.drawn.clear();
    draw(st, game);
}

#[test]
fn themes_and_the_window() {
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    let mut game = FakeGame::new("ui-themes");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    handle(&mut st, &mut game, key(Key::Char('5'), CTRL));
    assert_eq!(st.s.ui.tab, Tab::Settings);
    redraw(&mut st, &mut game);
    // The Synapse X style, from its card.
    let (sx, sy) = text_at(&game, "Synapse X style");
    click(&mut st, &mut game, (sx + 2.0, sy + 2.0));
    let synapse = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    assert_eq!(st.s.ui.look.theme, synapse);
    assert_eq!(st.s.ui.tab, Tab::Settings, "Settings stays open");
    let kept = std::fs::read_to_string(st.s.dir.join("look.txt")).unwrap();
    assert!(kept.contains("theme=synapse"), "{kept}");
    // Its editor: Execute runs it, Clear empties it.
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("print('hi from execute')");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Execute");
    click(&mut st, &mut game, at);
    assert!(log(&st).iter().any(|l| l.contains("hi from execute")), "{:?}", log(&st));
    let at = text_at(&game, "Clear");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.text(), "");
    // Options shows Settings in the window, its tabs above.
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Options");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.tab, Tab::Settings);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Console");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.tab, Tab::Console);
    // The title bar drags it; the look keeps where.
    redraw(&mut st, &mut game);
    let before = st.s.ui.look.window_rect(960.0, 540.0);
    let (tx, ty) = text_at(&game, "Horizon Studio");
    handle(&mut st, &mut game, Input::Mouse(tx + 100.0, ty + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(tx + 60.0, ty + 34.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let after = st.s.ui.look.window_rect(960.0, 540.0);
    assert_eq!((after[0] - before[0], after[1] - before[1]), (-40.0, 30.0));
    assert!(
        std::fs::read_to_string(st.s.dir.join("look.txt"))
            .unwrap()
            .contains("rect=")
    );
    // Its corner sizes it.
    redraw(&mut st, &mut game);
    let (cx, cy) = (after[0] + after[2] - 6.0, after[1] + after[3] - 6.0);
    handle(&mut st, &mut game, Input::Mouse(cx, cy));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(cx + 50.0, cy + 20.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let sized = st.s.ui.look.window_rect(960.0, 540.0);
    assert_eq!((sized[2] - after[2], sized[3] - after[3]), (50.0, 20.0));
    // Maximise, then minimise rolls it up to its title bar.
    redraw(&mut st, &mut game);
    click(&mut st, &mut game, (sized[0] + sized[2] - 46.0 - 23.0, sized[1] + 15.0));
    assert!(st.s.ui.look.maximized);
    redraw(&mut st, &mut game);
    click(&mut st, &mut game, (960.0 - 3.0 * 46.0 + 23.0, 15.0));
    assert!(st.s.ui.collapsed);
    redraw(&mut st, &mut game);
    assert!(!game.drawn.iter().any(|d| d.starts_with("text Execute @")), "rolled up");
    click(&mut st, &mut game, (960.0 - 3.0 * 46.0 + 23.0, 15.0));
    assert!(!st.s.ui.collapsed);
    // Every theme and page draws, with the window small too.
    for (i, _) in THEMES.iter().enumerate() {
        for tab in [Tab::Console, Tab::Editor, Tab::Library, Tab::Docs, Tab::Settings] {
            st.s.ui.look = Look {
                theme: i,
                rect: Some([10.0, 10.0, 330.0, 230.0]),
                ..Look::default()
            };
            st.s.ui.tab = tab;
            redraw(&mut st, &mut game);
            assert!(game.drawn.len() > 10, "theme {i} {tab:?}");
        }
    }
    // The look comes back as it was kept.
    let look = Look {
        theme: 3,
        window: true,
        accent: 2,
        scale: 1.25,
        opacity: 0.6,
        code: 15.0,
        motion: false,
        rect: Some([12.0, 34.0, 700.0, 400.0]),
        maximized: false,
    };
    look.save(&st.s.dir);
    assert_eq!(Look::load(&st.s.dir), look);
}

#[test]
fn clicks_land_at_any_size() {
    use horizon_studio::game::hstudio_theme::Look;
    let mut game = FakeGame::new("ui-scale");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look = Look {
        scale: 1.25,
        motion: false,
        ..Look::default()
    };
    redraw(&mut st, &mut game);
    let (x, y) = text_at(&game, "Docs");
    click(&mut st, &mut game, (x + 3.0, y + 3.0));
    assert_eq!(st.s.ui.tab, Tab::Docs);
    // Settings' size steps go 5% at a time, and stop at 150%.
    st.s.ui.tab = Tab::Settings;
    for _ in 0..12 {
        // Scrolled down to the Size row, as the page grows with the size.
        handle(&mut st, &mut game, key(Key::Home, NONE));
        redraw(&mut st, &mut game);
        for _ in 0..30 {
            if game.drawn.iter().any(|d| d.starts_with("text Size @")) {
                break;
            }
            handle(&mut st, &mut game, key(Key::Down, NONE));
            redraw(&mut st, &mut game);
        }
        let (_, ly) = text_at(&game, "Size");
        let plus = game
            .drawn
            .iter()
            .filter(|d| d.starts_with("text + @"))
            .map(|d| {
                let v: Vec<f32> = d
                    .split(" @[")
                    .nth(1)
                    .unwrap()
                    .split(']')
                    .next()
                    .unwrap()
                    .split(", ")
                    .map(|v| v.parse().unwrap())
                    .collect();
                (v[0], v[1])
            })
            .find(|(_, y)| (y - ly).abs() < 20.0)
            .expect("the size's +");
        click(&mut st, &mut game, (plus.0 + 1.0, plus.1 + 2.0));
    }
    assert_eq!(st.s.ui.look.scale, 1.5);
}
