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
    assert_eq!(e.text(), "if x then\n  y()\n  \nend", "the block gets its end");
    e.backspace(false);
    assert_eq!(e.text(), "if x then\n  y()\n\nend", "backspace takes a whole indent");
    let mut f = editor("on('tick', function()");
    f.end(false);
    f.newline();
    assert_eq!(f.text(), "on('tick', function()\n  \nend)", "and the call its bracket");
    // A block already closed gets no second end.
    let mut g = editor("if x then\nend");
    g.end(false);
    g.newline();
    assert_eq!(g.text(), "if x then\n  \nend");
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
    assert_eq!(
        st.s.ui.input.text, "player.facing()",
        "a function comes with its brackets"
    );
    assert_eq!(st.s.ui.input.cursor, "player.facing(".len(), "the cursor in them");
    game.clipboard = "print('pasted')".into();
    handle(&mut st, &mut game, key(Key::Char('a'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('x'), CTRL));
    assert_eq!(game.clipboard, "player.facing()");
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
    // The window lands where it goes at once (no time passes here).
    st.s.ui.look.motion = false;
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

/// Where `text` was last drawn (the panel's tabs share names with the studio's).
fn last_text_at(game: &FakeGame, text: &str) -> (f32, f32) {
    let line = game
        .drawn
        .iter()
        .rev()
        .find(|d| d.starts_with(&format!("text {text} @")))
        .unwrap_or_else(|| panic!("{text:?} not drawn"));
    let at: Vec<f32> = line
        .split('[')
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split(',')
        .map(|n| n.trim().parse().unwrap())
        .collect();
    (at[0] + 2.0, at[1] + 4.0)
}

fn type_text(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, text: &str) {
    for c in text.chars() {
        handle(st, game, Input::Text(c.to_string()));
    }
}

#[test]
fn the_editor_finds_problems_as_you_type_and_goes_to_them() {
    use horizon_studio::game::hstudio_ui::Panel;
    let mut game = FakeGame::new("ui-problems");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("local here = 1\n\nprint(player.poss())\n");
    redraw(&mut st, &mut game);
    let found = st.s.editor.problems.clone();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].line, found[0].error), (3, false));
    assert_eq!(found[0].message, "player has no 'poss'");
    assert!(
        game.drawn.iter().any(|d| d.contains("text player has no 'poss'")),
        "its message at the line's end"
    );
    // Typing waits a second's pause before it looks again.
    st.s.editor.click(1, 0, false);
    type_text(&mut st, &mut game, "if");
    redraw(&mut st, &mut game);
    assert!(st.s.editor.problems.iter().all(|p| !p.error), "not while typing");
    st.s.now += 0.6;
    redraw(&mut st, &mut game);
    assert!(st.s.editor.problems.iter().all(|p| !p.error), "nor at a short pause");
    st.s.now += 0.5;
    redraw(&mut st, &mut game);
    assert!(
        st.s.editor.problems.iter().any(|p| p.error),
        "{:?}",
        st.s.editor.problems
    );
    // The Problems tab lists them; a click on one goes to it.
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    st.s.now += 1.1;
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.problems.len(), 1);
    let at = last_text_at(&game, "Problems");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.panel, Panel::Problems);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "player has no 'poss'");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.row, 2, "the cursor on its line");
    assert_eq!(st.s.editor.col, "print(player.".len(), "at the name");
    assert!(!st.s.ui.panel_focus, "and the keys back on the code");
}

#[test]
fn suggestions_open_as_you_type_and_tab_takes_one() {
    let mut game = FakeGame::new("ui-suggest");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "player.fa");
    let shown = st.s.ui.suggest.as_ref().expect("suggestions open");
    assert_eq!(shown.items[0].label, "facing");
    redraw(&mut st, &mut game);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text cing @")),
        "the rest of what is typed, in the list"
    );
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.editor.text(), "player.facing()");
    assert_eq!(st.s.editor.col, "player.facing(".len(), "the cursor in its brackets");
    assert!(st.s.ui.suggest.is_none());
    // A library takes its dot, and its members open.
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "pla");
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.editor.text(), "player.");
    assert!(
        st.s.ui
            .suggest
            .as_ref()
            .is_some_and(|s| s.items.iter().any(|i| i.label == "pos"))
    );
    // Esc closes them and the studio stays open.
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Used);
    assert!(st.s.ui.suggest.is_none());
    assert!(st.s.ui.open);
    // Ctrl+Space opens them where nothing would.
    handle(&mut st, &mut game, key(Key::Char(' '), CTRL));
    assert!(st.s.ui.suggest.is_some()); // A library's function after ':' is called with a '.'.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "player:po");
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.editor.text(), "player.pos()");
}

#[test]
fn the_panel_console_runs_what_is_typed_in_it() {
    use horizon_studio::game::hstudio_ui::Panel;
    let mut game = FakeGame::new("ui-panel");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("");
    redraw(&mut st, &mut game);
    // Ctrl+` puts the keys on the panel (it is shown), then hides it.
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    assert!(st.s.ui.panel_focus && !st.s.ui.panel_hidden);
    assert_eq!(st.s.ui.panel, Panel::Console);
    type_text(&mut st, &mut game, "21 * 2");
    assert_eq!(st.s.ui.input.text, "21 * 2");
    assert_eq!(st.s.editor.text(), "", "not in the code");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(log(&st).last().map(String::as_str), Some("42"));
    // Esc takes the keys back to the code.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    assert!(!st.s.ui.panel_focus);
    assert!(st.s.ui.open);
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "x");
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    assert!(st.s.ui.panel_hidden && !st.s.ui.panel_focus);
    // A click on its tab shows it again, with the keys.
    handle(&mut st, &mut game, key(Key::Char('j'), CTRL));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Agent");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.panel, Panel::Agent);
    assert!(st.s.ui.panel_focus);
    type_text(&mut st, &mut game, "hi");
    assert_eq!(st.s.ui.agent_input.text, "hi");
}

#[test]
fn a_file_changed_on_disk_is_read_again_or_asked_about() {
    let mut game = FakeGame::new("ui-disk");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    st.s.ui.tab = Tab::Editor;
    let path = st.s.editor.path.clone().unwrap();
    redraw(&mut st, &mut game);
    // Changed elsewhere, with nothing unsaved here: read again.
    std::fs::write(&path, "print('from the agent')\n").unwrap();
    st.s.now += 1.0;
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.text(), "print('from the agent')\n");
    assert!(st.s.editor.reloaded_at.is_some());
    assert!(!st.s.editor.dirty);
    // Undo goes back to what it was.
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    assert_ne!(st.s.editor.text(), "print('from the agent')\n");
    handle(&mut st, &mut game, key(Key::Char('y'), CTRL));
    // With unsaved edits it asks.
    st.s.editor.click(0, 0, false);
    type_text(&mut st, &mut game, "-- mine\n");
    std::fs::write(&path, "print('again, longer this time')\n").unwrap();
    st.s.now += 1.0;
    redraw(&mut st, &mut game);
    assert!(st.s.editor.conflict);
    assert!(st.s.editor.text().starts_with("-- mine"), "the edits stay");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Reload");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.text(), "print('again, longer this time')\n");
    assert!(!st.s.editor.conflict && !st.s.editor.dirty);
    // Saving doesn't count as a change elsewhere.
    type_text(&mut st, &mut game, "x");
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    type_text(&mut st, &mut game, "y");
    st.s.now += 1.0;
    redraw(&mut st, &mut game);
    assert!(!st.s.editor.conflict);
}

#[cfg(unix)]
#[test]
fn the_agent_answers_in_its_panel() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{AgentLineKind, agent_ask, agent_busy};
    let mut game = FakeGame::new("ui-agent");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    // A stand-in agent: it says back what it was asked.
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }]);
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    // Unsaved edits are saved first: it reads the file.
    st.s.editor.set_text("print('mine')\n");
    st.s.editor.dirty = true;
    agent_ask(&mut st, "make it glow");
    assert!(agent_busy(&st));
    let path = st.s.editor.path.clone().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "print('mine')\n");
    assert!(!st.s.editor.dirty);
    for _ in 0..200 {
        run_for(&mut st, &mut game, 0.05);
        if !agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!agent_busy(&st), "it finished");
    let lines = &st.s.ui.agent.lines;
    assert_eq!(lines[0].kind, AgentLineKind::You);
    assert_eq!(lines[0].text, "make it glow");
    let said: Vec<&str> = lines
        .iter()
        .filter(|l| l.kind == AgentLineKind::Said)
        .map(|l| l.text.as_str())
        .collect();
    assert!(said.iter().any(|l| l.contains("make it glow")), "{said:?}");
    assert!(
        said.iter().any(|l| l.contains("hello.lua")),
        "the file open was in the prompt: {said:?}"
    );
    assert_eq!(lines.last().unwrap().kind, AgentLineKind::Done);
    // Its name is on the panel.
    st.s.ui.tab = Tab::Editor;
    st.s.ui.panel = horizon_studio::game::hstudio_ui::Panel::Agent;
    redraw(&mut st, &mut game);
    assert!(game.drawn.iter().any(|d| d.starts_with("text Echo @")));
}

#[test]
fn the_window_stays_on_a_smaller_screen() {
    use horizon_studio::game::hstudio_theme::Look;
    let look = Look {
        window: true,
        rect: Some([900.0, 500.0, 800.0, 500.0]),
        ..Look::default()
    };
    for (w, h) in [(1920.0, 1080.0), (960.0, 540.0), (640.0, 400.0)] {
        let r = look.window_rect(w, h);
        assert!(r[0] >= 0.0 && r[1] >= 0.0, "{r:?} on {w}x{h}");
        assert!(r[0] + r[2] <= w && r[1] + r[3] <= h, "{r:?} on {w}x{h}");
    }
    assert_eq!(
        look.window_rect(1920.0, 1080.0),
        [900.0, 500.0, 800.0, 500.0],
        "as it was, where it fits"
    );
}

#[test]
fn the_wheel_scrolls_without_moving_the_selection() {
    let mut game = FakeGame::new("ui-wheel");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    let text: Vec<String> = (1..=200).map(|i| format!("local line{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    // Select the first two lines with a drag, and let go.
    let first = text_at(&game, "line1");
    let second = text_at(&game, "line2");
    handle(&mut st, &mut game, Input::Mouse(first.0 - 30.0, first.1 + 2.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(second.0 + 40.0, second.1 + 2.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let selected = st.s.editor.selection();
    assert!(selected.is_some());
    assert_eq!(st.s.editor.row, 1);
    // Scrolling far down leaves the cursor, the selection and the view alone.
    for _ in 0..10 {
        handle(&mut st, &mut game, Input::Wheel(-1.0));
        redraw(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.selection(), selected);
    assert_eq!(st.s.editor.row, 1);
    assert_eq!(st.s.editor.scroll, 30, "the view stayed where it was scrolled to");
    // Typing (over the selection) brings the view back to the cursor.
    type_text(&mut st, &mut game, "x");
    redraw(&mut st, &mut game);
    assert!(st.s.editor.scroll <= 1);
    // Held down, the selection follows the pointer onto the lines scrolled in.
    let at = text_at(&game, "line5");
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1 + 2.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    let row = st.s.editor.row;
    handle(&mut st, &mut game, Input::Wheel(-1.0));
    assert_eq!(st.s.editor.row, row + 3, "the drag's end moved with the view");
    assert!(st.s.editor.selection().is_some());
}

#[test]
fn the_script_list_switches_files_and_keeps_their_edits() {
    let mut game = FakeGame::new("ui-scripts");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.title, "scratch.lua");
    type_text(&mut st, &mut game, "-- kept\n");
    assert!(st.s.editor.dirty);
    // A mod from the list opens; the scratch file's edit was saved first.
    redraw(&mut st, &mut game);
    let at = text_at(&game, "hello.lua");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert!(st.s.editor.title.starts_with("hello"), "{}", st.s.editor.title);
    let scratch = std::fs::read_to_string(st.s.dir.join("scratch.lua")).unwrap();
    assert!(scratch.contains("-- kept"), "{scratch}");
    // And back to the scratch file.
    redraw(&mut st, &mut game);
    let at = text_at(&game, "scratch.lua");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert_eq!(st.s.editor.title, "scratch.lua");
    assert!(st.s.editor.text().contains("-- kept"));
}

#[test]
fn save_file_saves_scratch_code_as_a_new_script() {
    use horizon_studio::game::hstudio_theme::THEMES;
    let mut game = FakeGame::new("ui-savefile");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("print('my script')");
    redraw(&mut st, &mut game);
    // On the scratch file it asks for a name, and saves the code as a script mod.
    let at = text_at(&game, "Save File");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.naming.is_some());
    type_text(&mut st, &mut game, "My Script");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let path = st.s.dir.join("mods").join("my_script.lua");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "print('my script')\n");
    assert_eq!(st.s.editor.path.as_ref(), Some(&path));
    assert!(st.s.mods.iter().any(|m| m.main == path), "in the script list");
    // On a script mod it saves it in place.
    type_text(&mut st, &mut game, "-- more ");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Save File");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.naming.is_none());
    assert!(!st.s.editor.dirty);
    assert!(std::fs::read_to_string(&path).unwrap().contains("-- more"));
}

#[test]
fn console_and_agent_text_select_and_copy() {
    use horizon_studio::game::hstudio::LogKind;
    let mut game = FakeGame::new("ui-select");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.now = 10.0;
    handle(&mut st, &mut game, key(Key::Char('1'), CTRL));
    st.s.log(LogKind::Info, "alpha bravo charlie");
    st.s.log(LogKind::Info, "delta echo foxtrot");
    redraw(&mut st, &mut game);
    // Drag from the start of the first line to the end of the second.
    let a = text_at(&game, "alpha bravo charlie");
    let b = text_at(&game, "delta echo foxtrot");
    handle(&mut st, &mut game, Input::Mouse(a.0 - 2.0, a.1 + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(b.0 + 2000.0, b.1 + 4.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::Char('c'), CTRL));
    assert_eq!(game.clipboard, "alpha bravo charlie\ndelta echo foxtrot");
    // A click elsewhere lets it go: Ctrl+C is the input's again.
    click(&mut st, &mut game, (5.0, 5.0));
    game.clipboard.clear();
    handle(&mut st, &mut game, key(Key::Char('c'), CTRL));
    assert_ne!(game.clipboard, "alpha bravo charlie\ndelta echo foxtrot");
}
