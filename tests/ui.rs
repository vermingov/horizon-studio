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
fn line_edit_selects_and_keeps_lines() {
    const SHIFT: Mods = Mods {
        ctrl: false,
        shift: true,
        alt: false,
    };
    let mut l = LineEdit::default();
    l.set("hello world");
    // Shift extends, an arrow without it drops the selection at its end.
    assert!(l.edit_key(Key::Left, SHIFT));
    l.edit_key(Key::Left, SHIFT);
    assert_eq!((l.selection(), l.selected()), (Some((9, 11)), "ld"));
    l.edit_key(Key::Left, NONE);
    assert_eq!((l.cursor, l.selection()), (9, None));
    l.edit_key(Key::Left, Mods { ctrl: true, ..SHIFT });
    assert_eq!(l.selected(), "wor");
    l.insert("W");
    assert_eq!((l.text.as_str(), l.cursor), ("hello Wld", 7));
    l.edit_key(Key::Char('a'), CTRL);
    l.edit_key(Key::Backspace, NONE);
    assert_eq!((l.text.as_str(), l.cursor, l.anchor), ("", 0, None));
    // New text drops the selection, however long the old text was.
    l.set("a longer text");
    l.edit_key(Key::Char('a'), CTRL);
    l.set("hi");
    assert_eq!((l.cursor, l.selection(), l.selected()), (2, None, ""));
    l.insert("!");
    assert_eq!(l.text, "hi!");
    l.set("");
    // One line turns newlines into spaces; several keep them, and Home,
    // End, Up and Down keep to the lines.
    l.insert("a\nb");
    assert_eq!(l.text, "a b");
    let mut m = LineEdit {
        multiline: true,
        ..LineEdit::default()
    };
    m.insert("abc\r\nde\nfghij");
    assert_eq!((m.text.as_str(), m.cursor), ("abc\nde\nfghij", 12));
    m.edit_key(Key::Home, NONE);
    assert_eq!(m.cursor, 7);
    m.edit_key(Key::End, NONE);
    m.edit_key(Key::Up, NONE);
    assert_eq!(m.cursor, 6, "the end of the shorter line");
    m.edit_key(Key::Up, NONE);
    assert_eq!(m.cursor, 2, "two characters in, as on the line below");
    m.edit_key(Key::Up, SHIFT);
    assert_eq!((m.cursor, m.selected()), (0, "ab"));
    m.edit_key(Key::End, CTRL);
    assert_eq!(m.cursor, m.text.len());
    m.edit_key(Key::Down, NONE);
    assert_eq!(m.cursor, m.text.len());
}

#[test]
fn canvas_screens_get_key_modifiers_and_may_keep_escape() {
    let mut game = FakeGame::new("ui-canvas-keys");
    let mut st = studio(&mut game);
    game.write_mod(
        "keys.lua",
        r#"
        local got, escapes = {}, 0
        command("open", function()
            ui.open({
                canvas = true,
                key = function(k, down, mods)
                    if down then table.insert(got, k .. ":" .. tostring(mods.ctrl) .. tostring(mods.shift)) end
                end,
                escape = function()
                    escapes += 1
                    return escapes == 1
                end,
            })
        end)
        command("got", function() return table.concat(got, " ") end)
        "#,
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(st.command("open"));
    st.frame(&mut game, 0.016);
    assert!(st.accept_screen());
    use horizon_studio::game::hstudio_ui::handle_screen;
    for (k, mods) in [(Key::Char('v'), CTRL), (Key::Char('a'), NONE)] {
        handle_screen(&mut st, &mut game, key(k, mods));
    }
    assert!(st.command("got"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("v:truefalse a:falsefalse"));
    // The first Escape is the screen's own (its escape() says so); the second closes it.
    assert_eq!(
        handle_screen(&mut st, &mut game, key(Key::Escape, NONE)),
        UiResult::Used
    );
    assert_eq!(
        handle_screen(&mut st, &mut game, key(Key::Escape, NONE)),
        UiResult::Close
    );
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
    // A letter of more than one byte before the word.
    assert_eq!(complete("épla", "épla".len()).as_deref(), Some("yer."));
    assert_eq!(complete("print(\"héllo", "print(\"héllo".len()), None);
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
    use horizon_studio::game::hstudio_ui::Pane;
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
    assert_eq!(st.s.ui.focus, Pane::Problems);
    // The Problems pane, with the keys, is drawn after the code (where the
    // same message ends its line).
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "player has no 'poss'");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.row, 2, "the cursor on its line");
    assert_eq!(st.s.editor.col, "print(player.".len(), "at the name");
    assert_eq!(st.s.ui.focus, Pane::Code, "and the keys back on the code");
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
fn accented_letters_type_in_the_code_and_the_console() {
    // As each letter comes the suggestions look for the word typed, and
    // the console's Tab completes it: a letter of more than one byte
    // before it (é, ö, ú) is never cut in two.
    let mut game = FakeGame::new("ui-accents");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("");
    type_text(&mut st, &mut game, "local número = 5");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    type_text(&mut st, &mut game, "local grö = é.a");
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.text(), "local número = 5\nlocal grö = é.a");
    // The console: as typed, then Tab after an accent in a string.
    handle(&mut st, &mut game, key(Key::Char('1'), CTRL));
    type_text(&mut st, &mut game, "größe = é.a");
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.input.text, "größe = é.a");
    st.s.ui.input.set("");
    type_text(&mut st, &mut game, "print(\"héllo");
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.ui.input.text, "print(\"héllo");
    st.s.ui.input.set("");
    type_text(&mut st, &mut game, "épla");
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert!(st.s.ui.input.text.starts_with("éplayer"), "{:?}", st.s.ui.input.text);
}

#[test]
fn the_panel_console_runs_what_is_typed_in_it() {
    use horizon_studio::game::hstudio_ui::Pane;
    let mut game = FakeGame::new("ui-panel");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("");
    redraw(&mut st, &mut game);
    // Ctrl+` puts the keys on the panel (it is shown), then hides it.
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    assert!(st.s.ui.focus == Pane::Console && st.s.ui.dock.shown(Pane::Console));
    assert!(st.s.ui.dock.is_front(Pane::Console));
    type_text(&mut st, &mut game, "21 * 2");
    assert_eq!(st.s.ui.input.text, "21 * 2");
    assert_eq!(st.s.editor.text(), "", "not in the code");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(log(&st).last().map(String::as_str), Some("42"));
    // Esc takes the keys back to the code.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    assert_eq!(st.s.ui.focus, Pane::Code);
    assert!(st.s.ui.open);
    type_text(&mut st, &mut game, "x");
    assert_eq!(st.s.editor.text(), "x");
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('`'), CTRL));
    assert!(!st.s.ui.dock.shown(Pane::Console) && st.s.ui.focus == Pane::Code);
    // A click on its tab shows it again, with the keys.
    handle(&mut st, &mut game, key(Key::Char('j'), CTRL));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Agent");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.dock.is_front(Pane::Agent));
    assert_eq!(st.s.ui.focus, Pane::Agent);
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
    st.s.ui.dock.activate(horizon_studio::game::hstudio_ui::Pane::Agent);
    redraw(&mut st, &mut game);
    assert!(game.drawn.iter().any(|d| d.starts_with("text Echo @")));
}

#[test]
fn the_agent_hears_the_scratch_file_is_open() {
    // The editor's own scratch file (its first buffer) is no mod's: the
    // prompt says it is open, and how the editor tools reach it, rather
    // than that no file is.
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{AgentLineKind, agent_ask, agent_busy};
    let mut game = FakeGame::new("ui-agent-scratch");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }]);
    assert_eq!(st.s.editor.title, "scratch.lua");
    st.s.editor.set_text("print(plyer.pos())\n");
    agent_ask(&mut st, "fix this");
    for _ in 0..200 {
        run_for(&mut st, &mut game, 0.05);
        if !agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let said: String =
        st.s.ui
            .agent
            .lines
            .iter()
            .filter(|l| l.kind == AgentLineKind::Said)
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
    assert!(said.contains("The editor has its scratch file open"), "{said}");
    assert!(!said.contains("No mod file is open"), "{said}");
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

#[test]
fn the_editor_scroll_bar_drags_and_jumps() {
    let mut game = FakeGame::new("ui-scrollbar");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    let text: Vec<String> = (1..=300).map(|i| format!("local v{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    let sb = st.s.ui.scrollbar.expect("a scroll bar for 300 lines");
    let k = st.s.ui.dp_factor() / st.s.ui.look.scale;
    // GUI pixels from the studio's units, as the pointer arrives.
    let px = |v: f32| v / k;
    // Press on the thumb and drag it to the bottom of the track.
    let thumb = (sb.top + sb.bar / 2.0, sb.top + sb.track);
    let bar_x = sb.x;
    let row = st.s.editor.row;
    handle(&mut st, &mut game, Input::Mouse(px(bar_x), px(thumb.0)));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(px(bar_x), px(thumb.1 + 50.0)));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(st.s.editor.scroll, sb.span as usize, "dragged to the end");
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.scroll, sb.span as usize, "the view stays there");
    assert_eq!(st.s.editor.row, row, "the cursor didn't move");
    // A click on the track near the top jumps there.
    redraw(&mut st, &mut game);
    click(&mut st, &mut game, (px(bar_x), px(sb.top + 2.0)));
    assert_eq!(st.s.editor.scroll, 0);
}

#[test]
fn the_agents_model_is_chosen_from_its_chip() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::Pane;
    let mut game = FakeGame::new("ui-model");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.now = 10.0;
    st.s.ui.agent.agents = Some(vec![AgentSpec::claude(std::path::PathBuf::from("claude"))]);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    // The chip opens the menu of models, the current one (the default) marked.
    let chip = text_at(&game, "Model: Default");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    let menu = st.s.ui.menu.clone().expect("the model menu");
    let labels: Vec<&str> = menu.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        labels,
        ["Default", "Opus", "Sonnet", "Haiku", "Fable", "", "Other model..."]
    );
    assert!(menu.items[0].checked);
    redraw(&mut st, &mut game);
    let sonnet = last_text_at(&game, "Sonnet");
    click(&mut st, &mut game, (sonnet.0 + 2.0, sonnet.1 + 2.0));
    assert!(st.s.ui.menu.is_none(), "choosing closes it");
    let kept = std::fs::read_to_string(st.s.dir.join("agent-models.txt")).unwrap();
    assert_eq!(kept, "claude=sonnet\n");
    redraw(&mut st, &mut game);
    let chip = text_at(&game, "Model: Sonnet");
    // The keys work it too: Down to Haiku, Enter.
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    handle(&mut st, &mut game, key(Key::Down, NONE));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(
        std::fs::read_to_string(st.s.dir.join("agent-models.txt"))
            .unwrap()
            .contains("claude=haiku")
    );
    // Another model by name, through the prompt; a click outside closes the menu first.
    redraw(&mut st, &mut game);
    let chip = text_at(&game, "Model: Haiku");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    click(&mut st, &mut game, (3.0, 3.0));
    assert!(st.s.ui.menu.is_none(), "a click outside closes it");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    redraw(&mut st, &mut game);
    let other = last_text_at(&game, "Other model...");
    click(&mut st, &mut game, (other.0 + 2.0, other.1 + 2.0));
    assert!(st.s.ui.naming.is_some());
    for _ in 0..5 {
        handle(&mut st, &mut game, key(Key::Backspace, NONE));
    }
    type_text(&mut st, &mut game, "claude-opus-5-5");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(
        std::fs::read_to_string(st.s.dir.join("agent-models.txt"))
            .unwrap()
            .contains("claude=claude-opus-5-5")
    );
    redraw(&mut st, &mut game);
    text_at(&game, "Model: claude-opus-5-5");
}

/// Whether some text drawn contains `part`.
fn drawn_contains(game: &FakeGame, part: &str) -> bool {
    game.drawn.iter().any(|d| d.starts_with("text ") && d.contains(part))
}

/// Whether `text` was drawn, all of it as one text.
fn drawn_text(game: &FakeGame, text: &str) -> bool {
    game.drawn.iter().any(|d| d.starts_with(&format!("text {text} @")))
}

#[test]
fn the_new_prompt_makes_a_script_or_a_module() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-new-prompt");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    // An edit not saved yet, in the scratch file.
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    type_text(&mut st, &mut game, "-- before the prompt\n");
    assert!(st.s.editor.dirty);
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    handle(&mut st, &mut game, Input::Text("n".into()));
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewScript, "it opens on a script");
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewModule);
    // Its switch takes clicks; nothing under the prompt does.
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Script");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewScript);
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Module");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewModule);
    click(&mut st, &mut game, (5.0, 5.0));
    assert!(st.s.ui.naming.is_some(), "a click elsewhere goes nowhere");
    type_text(&mut st, &mut game, "Night Light");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "New module"));
    assert!(drawn_contains(&game, "night_light/: main.lua, shared.lua and lib.lua"));
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "night_light.lua: one file"));
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.naming.is_none());
    let main = game.mods_dir().join("night_light").join("main.lua");
    assert!(main.is_file() && game.mods_dir().join("night_light").join("lib.lua").is_file());
    assert_eq!(st.s.editor.path.as_ref(), Some(&main));
    assert_eq!(st.s.editor.title, "night_light/main.lua");
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert!(st.s.ui.expanded.contains("night_light"), "unfolded in the script list");
    let scratch = std::fs::read_to_string(st.s.dir.join("scratch.lua")).unwrap();
    assert!(scratch.contains("-- before the prompt"), "saved first: {scratch}");
    // The same name again: the line says it is taken, and Enter keeps the prompt open.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    handle(&mut st, &mut game, Input::Text("n".into()));
    type_text(&mut st, &mut game, "Night Light");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "night_light already exists"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.naming.is_some());
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    assert!(st.s.ui.naming.is_none());
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewScript);
}

#[test]
fn the_plus_menu_offers_scripts_modules_and_files() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-plus-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let plus = text_at(&game, "+");
    click(&mut st, &mut game, (plus.0 + 1.0, plus.1 + 2.0));
    let labels = |st: &horizon_studio::game::hstudio::Studio| -> Vec<String> {
        let menu = st.s.ui.menu.clone().expect("the + menu");
        menu.items.iter().map(|i| i.label.clone()).collect()
    };
    assert_eq!(labels(&st), ["New script...", "New module..."]);
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "New module...");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewModule);
    type_text(&mut st, &mut game, "Box");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.editor.title, "box/main.lua");
    // With a module's file open, a new file of it too.
    redraw(&mut st, &mut game);
    let plus = text_at(&game, "+");
    click(&mut st, &mut game, (plus.0 + 1.0, plus.1 + 2.0));
    assert_eq!(
        labels(&st),
        ["New script...", "New module...", "", "New file in box..."]
    );
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "New file in box...");
    click(&mut st, &mut game, at);
    let folder = game.mods_dir().join("box");
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewFile(folder.clone()));
    type_text(&mut st, &mut game, "util");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "New file in box"));
    assert!(drawn_contains(&game, "box/util.lua: require(\"util\") loads it"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(folder.join("util.lua").is_file());
    assert_eq!(st.s.editor.title, "box/util.lua");
    // A name it can't take is explained, and Enter waits for a better one.
    redraw(&mut st, &mut game);
    let plus = text_at(&game, "+");
    click(&mut st, &mut game, (plus.0 + 1.0, plus.1 + 2.0));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "New file in box...");
    click(&mut st, &mut game, at);
    type_text(&mut st, &mut game, "lib.util");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "dots are for require"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.naming.is_some());
}

/// Where `text` was drawn on the row whose text is at height `y` (a row's
/// "..." or +), at its right of `x`.
fn text_on_row(game: &FakeGame, text: &str, (x, y): (f32, f32)) -> (f32, f32) {
    let prefix = format!("text {text} @[");
    game.drawn
        .iter()
        .filter_map(|d| d.strip_prefix(&prefix))
        .filter_map(|rest| {
            let mut n = rest.split(']').next()?.split(", ").map(|v| v.parse::<f32>());
            Some((n.next()?.ok()?, n.next()?.ok()?))
        })
        .find(|(tx, ty)| (ty - y).abs() < 3.0 && *tx > x)
        .map(|(tx, ty)| (tx + 2.0, ty + 3.0))
        .unwrap_or_else(|| panic!("no {text:?} on the row at {y}"))
}

fn right_click(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, (x, y): (f32, f32)) {
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

fn menu_labels(st: &horizon_studio::game::hstudio::Studio) -> Vec<(String, bool)> {
    let menu = st.s.ui.menu.clone().expect("a menu");
    menu.items.into_iter().map(|i| (i.label, i.enabled)).collect()
}

#[test]
fn the_script_list_folds_modules_and_opens_their_files() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::Pane;
    let mut game = FakeGame::new("ui-tree");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let big = game.mods_dir().join("big");
    std::fs::create_dir_all(&big).unwrap();
    std::fs::write(big.join("main.lua"), "local s = require('shared')\n").unwrap();
    std::fs::write(big.join("shared.lua"), "return {}\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let folder = text_at(&game, "big/");
    assert!(!drawn_text(&game, "shared.lua"), "folded at first");
    click(&mut st, &mut game, (folder.0 + 2.0, folder.1 + 2.0));
    redraw(&mut st, &mut game);
    let shared = text_at(&game, "shared.lua");
    text_at(&game, "main.lua");
    assert!(shared.0 > folder.0, "a module's files are indented under it");
    click(&mut st, &mut game, (shared.0 + 2.0, shared.1 + 2.0));
    assert_eq!(st.s.editor.title, "big/shared.lua");
    // Folded again, its row stands for the open file.
    redraw(&mut st, &mut game);
    click(&mut st, &mut game, (folder.0 + 2.0, folder.1 + 2.0));
    redraw(&mut st, &mut game);
    assert!(!drawn_text(&game, "shared.lua"));
    assert!(!st.s.ui.expanded.contains("big"));
    // A file opened from the Library unfolds its module.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "big");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.editor.title, "big/main.lua");
    assert!(st.s.ui.expanded.contains("big"));
    // The wheel over the list scrolls it, not the code.
    for n in 0..20 {
        game.write_mod(&format!("s{n:02}.lua"), "print(1)\n");
    }
    run_for(&mut st, &mut game, 0.6);
    redraw(&mut st, &mut game);
    let top_row = text_at(&game, "scratch.lua");
    handle(&mut st, &mut game, Input::Mouse(top_row.0 + 2.0, top_row.1 + 2.0));
    let code = st.s.editor.scroll;
    handle(&mut st, &mut game, Input::Wheel(-2.0));
    assert!(st.s.ui.scripts_scroll > 0);
    assert_eq!(st.s.editor.scroll, code);
    redraw(&mut st, &mut game);
    assert!(!drawn_text(&game, "scratch.lua"), "scrolled off");
    // A row's menu near the bottom opens above the pointer, whatever chip was drawn before:
    // the last row is a module, scrolled to.
    st.create_module(&mut game, "Zz").unwrap();
    st.s.ui.agent.agents = Some(vec![AgentSpec::claude(std::path::PathBuf::from("claude"))]);
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Wheel(-40.0));
    redraw(&mut st, &mut game);
    let low = text_at(&game, "zz/");
    let at = (low.0 + 2.0, low.1 + 2.0);
    right_click(&mut st, &mut game, at);
    assert_eq!(menu_labels(&st)[0].0, "New file...");
    redraw(&mut st, &mut game);
    let rect = st.s.ui.menu.as_ref().and_then(|m| m.rect).expect("drawn");
    assert!(rect[1] + rect[3] <= at.1, "{rect:?} above {at:?}");
    assert!(
        (rect[1] + rect[3] - (at.1 - 8.0)).abs() < 1.0,
        "{rect:?} right above {at:?}"
    );
}

#[test]
fn a_rows_menu_adds_renames_and_deletes_files() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-row-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let kit = text_at(&game, "kit/");
    click(&mut st, &mut game, (kit.0 + 2.0, kit.1 + 2.0));
    // Pointed at, a file's row shows "...", its menu.
    redraw(&mut st, &mut game);
    let lib = text_at(&game, "lib.lua");
    handle(&mut st, &mut game, Input::Mouse(lib.0 + 2.0, lib.1 + 2.0));
    redraw(&mut st, &mut game);
    let dots = text_on_row(&game, "...", lib);
    click(&mut st, &mut game, dots);
    assert_eq!(
        menu_labels(&st),
        [
            ("Open".to_owned(), true),
            ("Rename...".to_owned(), true),
            ("Delete...".to_owned(), true),
            (String::new(), false),
            ("Show in folder".to_owned(), true),
            ("Copy path".to_owned(), true)
        ]
    );
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    assert!(st.s.ui.menu.is_none());
    // A right press opens the same; main.lua's can't rename or delete it.
    redraw(&mut st, &mut game);
    let main = text_at(&game, "main.lua");
    right_click(&mut st, &mut game, (main.0 + 2.0, main.1 + 2.0));
    assert_eq!(
        menu_labels(&st)[..3],
        [
            ("Open".to_owned(), true),
            ("Rename...".to_owned(), false),
            ("Delete...".to_owned(), false)
        ]
    );
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // The module's row: its menu, and its + for a new file.
    right_click(&mut st, &mut game, (kit.0 + 2.0, kit.1 + 2.0));
    assert_eq!(
        menu_items(&st),
        [
            "New file...",
            "Open main.lua",
            "",
            "Reload",
            "Turn off",
            "",
            "Rename...",
            "Duplicate...",
            "Delete module...",
            "",
            "Show in folder",
            "Copy path"
        ]
    );
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    handle(&mut st, &mut game, Input::Mouse(kit.0 + 2.0, kit.1 + 2.0));
    redraw(&mut st, &mut game);
    let plus = text_on_row(&game, "+", kit);
    click(&mut st, &mut game, plus);
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewFile(dir.clone()));
    type_text(&mut st, &mut game, "util");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(dir.join("util.lua").is_file());
    assert_eq!(st.s.editor.title, "kit/util.lua");
    // Renamed: the field starts with its name, and the editor follows it.
    redraw(&mut st, &mut game);
    let util = text_at(&game, "util.lua");
    right_click(&mut st, &mut game, (util.0 + 2.0, util.1 + 2.0));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Rename...");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.naming.as_ref().map(|n| n.text.as_str()), Some("util"));
    for _ in 0..4 {
        handle(&mut st, &mut game, key(Key::Backspace, NONE));
    }
    type_text(&mut st, &mut game, "helpers");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let helpers = dir.join("helpers.lua");
    assert!(helpers.is_file() && !dir.join("util.lua").exists());
    assert_eq!(st.s.editor.path.as_ref(), Some(&helpers));
    assert_eq!(st.s.editor.title, "kit/helpers.lua");
    // Deleted: it asks first, saying where it goes; nothing is typed.
    redraw(&mut st, &mut game);
    let row = text_at(&game, "helpers.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Delete...");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.naming_kind, NamingKind::Delete(helpers.clone()));
    type_text(&mut st, &mut game, "x");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "Delete helpers.lua?"));
    assert!(drawn_contains(&game, "It moves to trash/kit/helpers.lua"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(!helpers.exists());
    assert!(st.s.dir.join("trash").join("kit").join("helpers.lua").is_file());
    assert_eq!(st.s.editor.title, "kit/main.lua");
    assert!(
        !st.s.mods[mod_index(&st, "kit")]
            .files()
            .contains(&"helpers.lua".to_owned())
    );
}

#[test]
fn save_file_execute_file_and_run_work_on_a_modules_files() {
    use horizon_studio::game::hstudio_theme::THEMES;
    let mut game = FakeGame::new("ui-module-run");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    st.create_module(&mut game, "Kit").unwrap();
    let lib = game.mods_dir().join("kit").join("lib.lua");
    let i = mod_index(&st, "kit");
    st.open_file(&lib);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.insert("-- edited\n");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Save File");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.naming.is_none(), "a module's file saves in place");
    assert!(!st.s.editor.dirty);
    assert!(std::fs::read_to_string(&lib).unwrap().starts_with("-- edited"));
    // Execute File saves it and reloads the module.
    let reloads = st.s.mods[i].reloads;
    st.s.editor.insert("-- and again\n");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Execute File");
    click(&mut st, &mut game, at);
    assert!(!st.s.editor.dirty);
    assert_eq!(st.s.mods[i].reloads, reloads + 1);
    assert!(std::fs::read_to_string(&lib).unwrap().contains("-- and again"));
    // With nothing to save it reloads the module, turning it on.
    st.set_enabled(&mut game, i, false);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Execute File");
    click(&mut st, &mut game, at);
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    // Synapse's list is the same tree.
    redraw(&mut st, &mut game);
    text_at(&game, "kit/");
    text_at(&game, "shared.lua");
    // In the studio's own layout, Ctrl+Enter on lib.lua reloads the module.
    st.s.ui.look.theme = 0;
    let before = log(&st).len();
    handle(&mut st, &mut game, key(Key::Enter, CTRL));
    let after = log(&st)[before..].to_vec();
    assert!(after.contains(&"Ran kit (reloaded)".to_owned()), "{after:?}");
    assert!(!after.iter().any(|l| l.contains("require works")), "{after:?}");
}

#[test]
fn the_library_shows_a_modules_files() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-library-module");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "kit");
    redraw(&mut st, &mut game);
    text_at(&game, "Module");
    assert!(drawn_contains(&game, "module  ·  3 files"));
    for f in ["main.lua", "lib.lua", "shared.lua"] {
        text_at(&game, f);
    }
    let at = text_at(&game, "lib.lua");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    let at = text_at(&game, "New file");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewFile(game.mods_dir().join("kit")));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // A script can become a module from here.
    st.s.ui.library_sel = mod_index(&st, "hello");
    redraw(&mut st, &mut game);
    assert!(!drawn_contains(&game, "module  ·"));
    let at = text_at(&game, "Make it a module");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert!(game.mods_dir().join("hello").join("main.lua").is_file());
    assert!(!game.mods_dir().join("hello.lua").exists());
    let h = mod_index(&st, "hello");
    assert!(st.s.mods[h].folder.is_some() && st.s.mods[h].running());
    assert!(st.s.ui.expanded.contains("hello"));
}

#[test]
fn the_editor_says_which_file_a_module_failed_in() {
    let mut game = FakeGame::new("ui-failed-in");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    std::fs::write(
        dir.join("lib.lua"),
        "local shared = require('shared')\nlocal lib = {}\nfunction lib.greet() return 'hi' end\n\nerror('lib broke')\nreturn lib\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "kit");
    assert!(matches!(st.s.mods[i].state, ModState::Failed(..)));
    st.edit_mod(i);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    // main.lua marks the line that required lib; the chip says where it failed.
    assert_eq!(st.s.editor.error_line, Some(9));
    redraw(&mut st, &mut game);
    let chip = text_at(&game, "Failed in lib.lua:5");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert_eq!(st.s.editor.error_line, Some(5));
    assert_eq!(st.s.editor.row, 4, "the cursor is at that line");
    redraw(&mut st, &mut game);
    assert!(!drawn_contains(&game, "Failed in"), "not about the file open");
}

#[test]
fn the_editor_checks_a_module_file_with_its_siblings() {
    let mut game = FakeGame::new("ui-siblings");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let dir = game.mods_dir().join("loud");
    std::fs::create_dir_all(&dir).unwrap();
    let main_src = "require('lib')\nchat.shout('x')\nlocal extra = require('extra')\n";
    std::fs::write(dir.join("main.lua"), main_src).unwrap();
    std::fs::write(dir.join("lib.lua"), "function chat.shout(s) chat.show(s) end\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    let messages = |st: &horizon_studio::game::hstudio::Studio| -> Vec<String> {
        st.s.editor.problems.iter().map(|p| p.message.clone()).collect()
    };
    st.open_file(&dir.join("main.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    assert_eq!(messages(&st), ["no extra.lua in this module"]);
    // The same text as a one-file script: lib.lua's function is unknown there.
    game.write_mod("solo.lua", main_src);
    run_for(&mut st, &mut game, 0.6);
    st.open_file(&game.mods_dir().join("solo.lua"));
    redraw(&mut st, &mut game);
    let solo = messages(&st);
    assert!(solo.contains(&"chat has no 'shout'".to_owned()), "{solo:?}");
    assert!(
        solo.iter().any(|m| m.contains("require loads a module's files")),
        "{solo:?}"
    );
    // A sibling written on disk: the check looks again, the text unchanged.
    st.open_file(&dir.join("main.lua"));
    redraw(&mut st, &mut game);
    let revision = st.s.editor.revision;
    assert_eq!(messages(&st), ["no extra.lua in this module"]);
    std::fs::write(dir.join("extra.lua"), "return {}\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    redraw(&mut st, &mut game);
    assert!(messages(&st).is_empty(), "{:?}", messages(&st));
    assert_eq!(st.s.editor.revision, revision);
    // Suggestions know the module's files.
    st.s.editor.set_text("local e = require(\"ex");
    st.s.editor.click(0, 22, false);
    horizon_studio::game::hstudio_ui::refresh_suggest(&mut st);
    let sg = st.s.ui.suggest.clone().expect("suggestions");
    assert_eq!(sg.items[0].label, "extra");
}

#[test]
fn a_big_modules_other_files_are_read_within_limits() {
    let mut game = FakeGame::new("ui-big-module");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let (big, many) = (game.mods_dir().join("big"), game.mods_dir().join("many"));
    for dir in [&big, &many] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("main.lua"), "print('main')\n").unwrap();
    }
    // A file over 256 KiB, one under it, and 4 MiB in all: 16 files of
    // 250 KB fit, after small.lua (main.lua, the file open, is not one).
    let padded = |name: &str, len: usize| format!("{name} = 1\n-- {}\n", "x".repeat(len));
    std::fs::write(big.join("huge.lua"), padded("hugename", 300 << 10)).unwrap();
    std::fs::write(big.join("small.lua"), "smallname = 1\n").unwrap();
    for n in 0..20 {
        let name = format!("t{n:02}name");
        let text = padded(&name, 250_000 - name.len() - 9);
        assert_eq!(text.len(), 250_000);
        std::fs::write(big.join(format!("t{n:02}.lua")), text).unwrap();
    }
    // And 256 files, however small.
    for n in 0..300 {
        std::fs::write(many.join(format!("m{n:03}.lua")), format!("m{n:03}name = 1\n")).unwrap();
    }
    run_for(&mut st, &mut game, 0.6);
    let suggested = |st: &mut horizon_studio::game::hstudio::Studio, typed: &str| -> bool {
        st.s.editor.set_text(typed);
        st.s.editor.click(0, typed.len(), false);
        horizon_studio::game::hstudio_ui::refresh_suggest(st);
        st.s.ui
            .suggest
            .as_ref()
            .is_some_and(|s| s.items.iter().any(|i| i.label.starts_with(typed)))
    };
    st.open_file(&big.join("main.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    assert!(suggested(&mut st, "smalln"));
    assert!(!suggested(&mut st, "hugen"), "over 256 KiB: not read");
    assert!(suggested(&mut st, "t00n") && suggested(&mut st, "t15n"));
    assert!(!suggested(&mut st, "t16n"), "past 4 MiB: not read");
    // A file not read is still the module's: requiring it is no problem.
    st.s.editor
        .set_text("local h = require('huge')\nlocal t = require('t19')\n");
    redraw(&mut st, &mut game);
    assert!(st.s.editor.problems.is_empty(), "{:?}", st.s.editor.problems);
    st.open_file(&many.join("main.lua"));
    redraw(&mut st, &mut game);
    assert!(suggested(&mut st, "m255n"));
    assert!(!suggested(&mut st, "m256n"), "past 256 files: not read");
}

#[test]
fn a_mod_opens_from_the_library_though_saving_first_rescans() {
    let mut game = FakeGame::new("ui-edit-rescan");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    game.write_mod("zz.lua", "print('zz')\n");
    run_for(&mut st, &mut game, 0.6);
    // An unsaved edit, and a mod listed before zz gone from disk: saving
    // first rescans, and zz's place in the list moves.
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    type_text(&mut st, &mut game, "-- edit\n");
    std::fs::remove_file(game.mods_dir().join("hello.lua")).unwrap();
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "zz");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.editor.title, "zz.lua");
    assert!(!st.s.mods.iter().any(|m| m.id == "hello"));
}

#[test]
fn a_click_reaches_what_was_drawn_though_a_scan_moved_it() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-stale-click");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    game.write_mod("omega.lua", "print('omega')\n");
    game.write_mod("zeta.lua", "print('zeta')\n");
    run_for(&mut st, &mut game, 0.6);
    st.create_module(&mut game, "Kit").unwrap();
    st.create_module(&mut game, "Lab").unwrap();
    let mods = game.mods_dir();
    let ids: Vec<&str> = st.s.mods.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(&ids[ids.len() - 4..], ["omega", "zeta", "kit", "lab"]);
    // Between a frame and a click on it, a mod listed before goes (an agent,
    // a file manager), and each mod after it moves up a place.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "omega");
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Make it a module");
    std::fs::remove_file(mods.join("hello.lua")).unwrap();
    st.scan(&mut game);
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert!(mods.join("omega").join("main.lua").is_file(), "the script drawn");
    assert!(mods.join("zeta.lua").is_file() && !mods.join("zeta").exists());
    assert_eq!(st.s.mods[st.s.ui.library_sel].id, "omega", "still the one chosen");
    // The + on a module's row in the script list.
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let kit = text_at(&game, "kit/");
    handle(&mut st, &mut game, Input::Mouse(kit.0 + 2.0, kit.1 + 2.0));
    redraw(&mut st, &mut game);
    let plus = text_on_row(&game, "+", kit);
    std::fs::remove_file(mods.join("zeta.lua")).unwrap();
    st.scan(&mut game);
    click(&mut st, &mut game, plus);
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewFile(mods.join("kit")));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // A module's file in the Library, though a file listed before it came since.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "kit");
    redraw(&mut st, &mut game);
    let lib = text_at(&game, "lib.lua");
    std::fs::write(mods.join("kit").join("a.lua"), "return 1\n").unwrap();
    st.scan(&mut game);
    click(&mut st, &mut game, (lib.0 + 2.0, lib.1 + 2.0));
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    // A mod gone since: a click on it does nothing.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    let at = text_at(&game, "New file");
    std::fs::remove_dir_all(mods.join("kit")).unwrap();
    st.scan(&mut game);
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert!(st.s.ui.naming.is_none(), "{:?}", st.s.ui.naming_kind);
}

/// The labels of the open menu.
fn menu_items(st: &horizon_studio::game::hstudio::Studio) -> Vec<String> {
    menu_labels(st).into_iter().map(|(l, _)| l).collect()
}

#[test]
fn the_executor_styles_reach_a_modules_files_from_the_library() {
    use horizon_studio::game::hstudio_theme::THEMES;
    for theme in ["protosmasher", "protoclassic", "rc7"] {
        let mut game = FakeGame::new(&format!("ui-style-files-{theme}"));
        let mut st = studio(&mut game);
        st.s.ui.open = true;
        st.s.ui.look.motion = false;
        st.s.ui.look.theme = THEMES.iter().position(|t| t.id == theme).unwrap();
        st.create_module(&mut game, "Kit").unwrap();
        // They have no script list: in the Library (their Open File), the
        // chosen module's "..." lists its files, and one chosen opens.
        st.s.ui.tab = Tab::Library;
        st.s.ui.library_sel = mod_index(&st, "kit");
        redraw(&mut st, &mut game);
        let dots = text_at(&game, "...");
        click(&mut st, &mut game, (dots.0 + 2.0, dots.1 + 2.0));
        assert_eq!(
            menu_items(&st),
            [
                "main.lua",
                "lib.lua",
                "shared.lua",
                "",
                "New file...",
                "",
                "Reload",
                "Turn off",
                "",
                "Rename...",
                "Duplicate...",
                "Delete module...",
                "",
                "Show in folder",
                "Copy path",
                "",
                "New mod..."
            ],
            "{theme}"
        );
        redraw(&mut st, &mut game);
        let at = last_text_at(&game, "lib.lua");
        click(&mut st, &mut game, at);
        assert_eq!(st.s.ui.tab, Tab::Editor, "{theme}");
        assert_eq!(st.s.editor.title, "kit/lib.lua", "{theme}");
        // Back there, the menu renames or deletes the file open.
        st.s.ui.tab = Tab::Library;
        redraw(&mut st, &mut game);
        let dots = text_at(&game, "...");
        click(&mut st, &mut game, (dots.0 + 2.0, dots.1 + 2.0));
        let items = menu_items(&st);
        assert!(
            items.contains(&"Rename lib.lua...".to_owned()) && items.contains(&"Delete lib.lua...".to_owned()),
            "{theme}: {items:?}"
        );
        handle(&mut st, &mut game, key(Key::Escape, NONE));
        // A script's menu makes it a module.
        st.s.ui.library_sel = mod_index(&st, "hello");
        redraw(&mut st, &mut game);
        let dots = text_at(&game, "...");
        click(&mut st, &mut game, (dots.0 + 2.0, dots.1 + 2.0));
        assert_eq!(menu_items(&st)[0], "Edit", "{theme}");
        assert!(
            usable(&st).ends_with(&[
                "Rename...".to_owned(),
                "Duplicate...".to_owned(),
                "Make it a module".to_owned(),
                "Delete...".to_owned(),
                "Show in folder".to_owned(),
                "Copy path".to_owned(),
                "New mod...".to_owned()
            ]),
            "{theme}: {:?}",
            menu_items(&st)
        );
        redraw(&mut st, &mut game);
        let at = last_text_at(&game, "Make it a module");
        click(&mut st, &mut game, at);
        let mods = game.mods_dir();
        assert!(mods.join("hello").join("main.lua").is_file(), "{theme}");
        assert!(!mods.join("hello.lua").exists(), "{theme}");
    }
}

#[test]
fn the_library_keys_make_files_and_modules_and_open_its_menu() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-library-keys");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    let mods = game.mods_dir();
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "kit");
    redraw(&mut st, &mut game);
    // F: a new file of the module chosen.
    handle(&mut st, &mut game, Input::Text("f".into()));
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewFile(mods.join("kit")));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // M on a module does nothing; on a script it makes it one.
    handle(&mut st, &mut game, Input::Text("m".into()));
    assert!(st.s.ui.naming.is_none() && mods.join("kit").join("main.lua").is_file());
    st.s.ui.library_sel = mod_index(&st, "hello");
    handle(&mut st, &mut game, Input::Text("m".into()));
    assert!(mods.join("hello").join("main.lua").is_file() && !mods.join("hello.lua").exists());
    // "." opens its menu, under its "...", for the keys: Down and Enter choose.
    st.s.ui.library_sel = mod_index(&st, "kit");
    redraw(&mut st, &mut game);
    let dots = text_at(&game, "...");
    handle(&mut st, &mut game, Input::Text(".".into()));
    assert_eq!(menu_items(&st)[0], "main.lua");
    redraw(&mut st, &mut game);
    let rect = st.s.ui.menu.as_ref().and_then(|m| m.rect).expect("drawn");
    assert!(
        rect[0] <= dots.0 && dots.0 < rect[0] + rect[2],
        "at its \"...\": {rect:?}"
    );
    assert!(within(rect, [0.0, 0.0, 960.0, 540.0]), "{rect:?}");
    handle(&mut st, &mut game, key(Key::Down, NONE));
    handle(&mut st, &mut game, key(Key::Down, NONE));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert_eq!(st.s.ui.tab, Tab::Editor);
    // A right press on a Library row opens the same menu, and chooses the row.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    let via = text_at(&game, "ViaHorizon");
    right_click(&mut st, &mut game, (via.0 + 2.0, via.1 + 2.0));
    assert_eq!(st.s.mods[st.s.ui.library_sel].id, "viahorizon");
    assert_eq!(
        menu_items(&st),
        [
            "Edit",
            "",
            "Reload",
            "Turn off",
            "",
            "Rename...",
            "Duplicate...",
            "Make it a module",
            "Delete...",
            "",
            "Show in folder",
            "Copy path",
            "",
            "New mod..."
        ]
    );
    let hints: Vec<String> =
        st.s.ui
            .menu
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.hint.clone())
            .collect();
    assert_eq!(hints[..4], ["⏎", "", "R", "Space"], "the Library's keys");
    assert_eq!(hints.last().map(String::as_str), Some("N"));
    // New mod, last, opens the New prompt as N does.
    choose(&mut st, &mut game, "New mod...");
    assert_eq!(st.s.ui.naming_kind, NamingKind::NewScript);
    assert!(st.s.ui.naming.is_some());
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // A big module's menu lists twelve of its files and says where the rest are.
    for n in 0..12 {
        std::fs::write(mods.join("kit").join(format!("f{n:02}.lua")), "return 1\n").unwrap();
    }
    run_for(&mut st, &mut game, 0.6);
    st.s.ui.library_sel = mod_index(&st, "kit");
    handle(&mut st, &mut game, Input::Text(".".into()));
    let items = menu_labels(&st);
    assert_eq!(items[11], ("f10.lua".to_owned(), true));
    assert_eq!(
        items[12],
        (
            "and 3 more: the console's edit(\"kit\", file) opens any".to_owned(),
            false
        )
    );
}

#[test]
fn a_file_removed_while_edited_asks_to_let_it_go_or_keep_it() {
    let mut game = FakeGame::new("ui-removed");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    let mods = game.mods_dir();
    let lib = mods.join("kit").join("lib.lua");
    st.open_file(&lib);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.click(0, 0, false);
    type_text(&mut st, &mut game, "-- mine\n");
    horizon_studio::headless::remove_files(&mods, "kit", &["lib.lua".to_owned()]).unwrap();
    run_for(&mut st, &mut game, 0.6);
    redraw(&mut st, &mut game);
    assert!(drawn_contains(
        &game,
        "This file was removed from disk while you were editing it."
    ));
    // Keep mine: a save writes it back.
    let at = text_at(&game, "Keep mine");
    click(&mut st, &mut game, at);
    assert!(!st.s.editor.removed && st.s.editor.dirty);
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    assert!(std::fs::read_to_string(&lib).unwrap().starts_with("-- mine"));
    // Close: the edits go, and main.lua opens.
    type_text(&mut st, &mut game, "-- more\n");
    horizon_studio::headless::remove_files(&mods, "kit", &["lib.lua".to_owned()]).unwrap();
    run_for(&mut st, &mut game, 0.6);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Close");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.title, "kit/main.lua");
    assert!(!lib.exists());
}

#[test]
fn the_executor_styles_say_which_file_a_module_failed_in() {
    use horizon_studio::game::hstudio_theme::THEMES;
    for theme in ["synapse", "protosmasher", "protoclassic", "rc7"] {
        let mut game = FakeGame::new(&format!("ui-style-failed-{theme}"));
        let mut st = studio(&mut game);
        st.s.ui.open = true;
        st.s.ui.look.motion = false;
        st.s.ui.look.theme = THEMES.iter().position(|t| t.id == theme).unwrap();
        st.create_module(&mut game, "Kit").unwrap();
        let dir = game.mods_dir().join("kit");
        std::fs::write(
            dir.join("lib.lua"),
            "local lib = {}\n\nerror('lib broke')\nreturn lib\n",
        )
        .unwrap();
        run_for(&mut st, &mut game, 0.6);
        st.edit_mod(mod_index(&st, "kit"));
        st.s.ui.tab = Tab::Editor;
        redraw(&mut st, &mut game);
        let chip = text_at(&game, "Failed in lib.lua:3");
        click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
        assert_eq!(st.s.editor.title, "kit/lib.lua", "{theme}");
        assert_eq!(st.s.editor.row, 2, "{theme}: at the line it failed at");
        redraw(&mut st, &mut game);
        assert!(!drawn_contains(&game, "Failed in"), "{theme}: not about the file open");
    }
}

// ----------------------------------------------------------------- menus

/// A drawn text's last record: its place in the record, where it starts
/// (in dp) and its colour.
fn last_drawn(game: &FakeGame, text: &str) -> (usize, (f32, f32), u32) {
    let prefix = format!("text {text} @[");
    let (k, rest) = game
        .drawn
        .iter()
        .enumerate()
        .rev()
        .find_map(|(k, d)| d.strip_prefix(&prefix).map(|rest| (k, rest)))
        .unwrap_or_else(|| panic!("{text:?} not drawn"));
    let (at, colour) = rest.split_once("] ").unwrap();
    let at: Vec<f32> = at.split(", ").map(|v| v.parse().unwrap()).collect();
    (k, (at[0], at[1]), u32::from_str_radix(colour, 16).unwrap())
}

/// A rect drawn: where (in dp) and its colour (0xAARRGGBB).
fn drawn_rect(line: &str) -> Option<([f32; 4], u32)> {
    let rest = line.strip_prefix("rect [")?;
    let (r, tail) = rest.split_once(']')?;
    let r: Vec<f32> = r.split(", ").filter_map(|v| v.parse().ok()).collect();
    let colour = u32::from_str_radix(tail.rsplit(' ').next()?, 16).ok()?;
    Some(([r[0], r[1], r[2], r[3]], colour))
}

/// The colour of `text` (as last drawn) and of what it was drawn on: the
/// last opaque rect drawn before it that holds where it starts.
fn text_on(game: &FakeGame, text: &str) -> (u32, u32) {
    let (k, (x, y), c) = last_drawn(game, text);
    let bg = game.drawn[..k]
        .iter()
        .rev()
        .filter_map(|d| drawn_rect(d))
        .find(|(r, argb)| argb >> 24 == 0xFF && x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3])
        .unwrap_or_else(|| panic!("nothing under {text:?}"))
        .1;
    (c, bg & 0xFF_FFFF)
}

/// WCAG's contrast ratio of two colours (0xRRGGBB).
fn contrast(a: u32, b: u32) -> f32 {
    let lum = |c: u32| {
        let lin = |s: u32| {
            let v = ((c >> s) & 0xFF) as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(16) + 0.7152 * lin(8) + 0.0722 * lin(0)
    };
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Whether `inner` lies inside `outer`.
fn within(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0]
        && inner[1] >= outer[1]
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

#[test]
fn menus_read_and_fit_in_every_style() {
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    for (i, theme) in THEMES.iter().enumerate() {
        for scale in [1.0, 1.5] {
            let mut game = FakeGame::new(&format!("ui-menu-look-{}-{scale}", theme.id));
            let mut st = studio(&mut game);
            st.s.ui.open = true;
            st.s.ui.look = Look {
                theme: i,
                scale,
                motion: false,
                ..Look::default()
            };
            st.create_module(&mut game, "Kit").unwrap();
            st.s.ui.tab = Tab::Library;
            st.s.ui.library_sel = mod_index(&st, "kit");
            redraw(&mut st, &mut game);
            handle(&mut st, &mut game, Input::Text(".".into()));
            redraw(&mut st, &mut game);
            let what = format!("{} at {scale}", theme.id);
            let rect = st.s.ui.menu.as_ref().and_then(|m| m.rect).expect("drawn");
            let screen = [0.0, 0.0, 960.0 / scale, 540.0 / scale];
            assert!(within(rect, screen), "{what}: {rect:?}");
            // Its text reads on it: an item, one that deletes, and both lit.
            for label in ["New file...", "Delete module..."] {
                let (c, bg) = text_on(&game, label);
                assert!(contrast(c, bg) >= 4.5, "{what}: {label} {c:06x} on {bg:06x}");
            }
            // Lit: the first item (Home), then the second that starts with D.
            for (typed, label) in [
                (None, "main.lua"),
                (Some("d"), "Duplicate..."),
                (Some("d"), "Delete module..."),
            ] {
                match typed {
                    None => {
                        handle(&mut st, &mut game, key(Key::Home, NONE));
                    }
                    Some(t) => {
                        handle(&mut st, &mut game, Input::Text(t.into()));
                    }
                }
                redraw(&mut st, &mut game);
                let (c, bg) = text_on(&game, label);
                let plate = text_on(&game, "New file...").1;
                assert_ne!(bg, plate, "{what}: {label} is lit");
                assert!(contrast(c, bg) >= 4.5, "{what}: {label} {c:06x} on {bg:06x}");
            }
            // The code's menu too: on the screen, readable, and its items do what they say.
            handle(&mut st, &mut game, key(Key::Escape, NONE));
            st.s.ui.tab = Tab::Editor;
            st.s.ui.agent.agents = Some(Vec::new());
            let scratch = st.scratch_path();
            st.open_file(&scratch);
            st.s.editor.set_text("local a = 1\nprint(a)");
            redraw(&mut st, &mut game);
            let code = last_text_at(&game, "print");
            right_click(&mut st, &mut game, code);
            redraw(&mut st, &mut game);
            let rect = st.s.ui.menu.as_ref().and_then(|m| m.rect).expect("the code's menu");
            assert!(within(rect, screen), "{what}: {rect:?}");
            let (c, bg) = text_on(&game, "Cut");
            assert!(contrast(c, bg) >= 4.5, "{what}: Cut {c:06x} on {bg:06x}");
            let at = last_text_at(&game, "Select all");
            click(&mut st, &mut game, at);
            assert_eq!(
                st.s.editor.selected_text().as_deref(),
                Some("local a = 1\nprint(a)"),
                "{what}"
            );
        }
    }
}

#[test]
fn a_menu_taller_than_the_screen_scrolls() {
    use horizon_studio::game::hstudio_theme::Look;
    use horizon_studio::game::hstudio_ui::{Menu, MenuCmd, MenuItem};
    let mut game = FakeGame::new("ui-menu-scroll");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look = Look {
        scale: 1.5,
        motion: false,
        ..Look::default()
    };
    let items: Vec<MenuItem> = (0..40)
        .map(|n| MenuItem::new(format!("Item {n}"), MenuCmd::Reveal(st.s.dir.join(format!("f{n}")))))
        .collect();
    st.s.ui.menu = Some(Menu::new([20.0, 20.0], 0.0, items, st.s.now));
    redraw(&mut st, &mut game);
    let menu = st.s.ui.menu.clone().unwrap();
    let rect = menu.rect.unwrap();
    assert!(within(rect, [0.0, 0.0, 640.0, 360.0]), "{rect:?}");
    assert!(
        menu.scrolls() && menu.shown.0 == 0 && menu.shown.1 < 40,
        "{:?}",
        menu.shown
    );
    assert!(drawn_text(&game, "Item 0") && !drawn_text(&game, "Item 39"));
    // The keys keep the lit item in view: End goes to the last.
    handle(&mut st, &mut game, key(Key::End, NONE));
    redraw(&mut st, &mut game);
    assert!(drawn_text(&game, "Item 39") && !drawn_text(&game, "Item 0"));
    assert_eq!(st.s.ui.menu.as_ref().unwrap().shown.1, 40);
    // The wheel over it scrolls it; anywhere else it closes it.
    let k = st.s.ui.dp_factor() / st.s.ui.look.scale;
    handle(
        &mut st,
        &mut game,
        Input::Mouse((rect[0] + 20.0) / k, (rect[1] + 20.0) / k),
    );
    for _ in 0..40 {
        handle(&mut st, &mut game, Input::Wheel(1.0));
    }
    redraw(&mut st, &mut game);
    assert!(drawn_text(&game, "Item 0"), "scrolled back up");
    // An item shown after scrolling is chosen as any other.
    let at = last_text_at(&game, "Item 3");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_none());
    assert_eq!(game.opened.last(), Some(&st.s.dir.join("f3")));
    let only = vec![MenuItem::new("Only", MenuCmd::NewScript)];
    st.s.ui.menu = Some(Menu::new([20.0, 20.0], 0.0, only, st.s.now));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(600.0, 330.0));
    handle(&mut st, &mut game, Input::Wheel(-1.0));
    assert!(st.s.ui.menu.is_none(), "the wheel elsewhere closes it");
    // A tall menu opened in another's place, before it is drawn: the
    // pointer over where the other's items were lights none of its own.
    let items = |n: usize| -> Vec<MenuItem> {
        (0..n)
            .map(|k| MenuItem::new(format!("Row {k}"), MenuCmd::NewScript))
            .collect()
    };
    st.s.ui.menu = Some(Menu::new([20.0, 20.0], 0.0, items(40), st.s.now));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Row 12");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    st.s.ui.menu = Some(Menu::new([300.0, 20.0], 0.0, items(30), st.s.now));
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    assert_eq!(st.s.ui.menu.as_ref().unwrap().sel, None);
    // Nor does a lit item past its end stop it being drawn.
    st.s.ui.menu.as_mut().unwrap().sel = Some(35);
    redraw(&mut st, &mut game);
    assert!(drawn_text(&game, "Row 0"));
}

fn right_press(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame) {
    handle(
        st,
        game,
        Input::Button {
            left: false,
            down: true,
        },
    );
}

fn right_release(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame) {
    handle(
        st,
        game,
        Input::Button {
            left: false,
            down: false,
        },
    );
}

#[test]
fn the_wheel_closes_the_menu_and_right_clicks_leave_drags_alone() {
    let mut game = FakeGame::new("ui-menu-wheel");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    let text: Vec<String> = (1..=300).map(|i| format!("local v{i} = {i}")).collect();
    st.s.editor.set_text(&text.join("\n"));
    redraw(&mut st, &mut game);
    // A row's menu, and the wheel over the code: the menu closes, nothing scrolls.
    let hello = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (hello.0 + 2.0, hello.1 + 2.0));
    assert!(st.s.ui.menu.is_some());
    redraw(&mut st, &mut game);
    let code = text_at(&game, "v5");
    handle(&mut st, &mut game, Input::Mouse(code.0, code.1));
    handle(&mut st, &mut game, Input::Wheel(-2.0));
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.editor.scroll, 0, "the wheel went no further");
    // The scroll bar dragged: a right press and release on the way open
    // nothing, and the drag goes on until the left button lets go.
    redraw(&mut st, &mut game);
    let sb = st.s.ui.scrollbar.expect("a scroll bar");
    let (x, top) = (sb.x, sb.top + sb.bar / 2.0);
    handle(&mut st, &mut game, Input::Mouse(x, top));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(x, top + 40.0));
    right_press(&mut st, &mut game);
    right_release(&mut st, &mut game);
    assert!(st.s.ui.menu.is_none());
    let scrolled = st.s.editor.scroll;
    assert!(scrolled > 0);
    handle(&mut st, &mut game, Input::Mouse(x, top + 120.0));
    assert!(st.s.editor.scroll > scrolled, "still dragging");
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let at = st.s.editor.scroll;
    handle(&mut st, &mut game, Input::Mouse(x, top + 200.0));
    assert_eq!(st.s.editor.scroll, at, "let go");
}

#[test]
fn typing_while_a_menu_is_open_goes_to_the_menu() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::Pane;
    let mut game = FakeGame::new("ui-menu-text");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.agent.agents = Some(vec![AgentSpec::claude(std::path::PathBuf::from("claude"))]);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text("");
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    let chip = text_at(&game, "Model: Default");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    // A letter lights the next item that starts with it, and types nothing.
    type_text(&mut st, &mut game, "h");
    let menu = st.s.ui.menu.clone().expect("still open");
    assert_eq!(menu.items[menu.sel.unwrap()].label, "Haiku");
    assert_eq!(st.s.editor.text(), "");
    assert_eq!(st.s.ui.agent_input.text, "");
    // A space chooses it.
    handle(&mut st, &mut game, Input::Text(" ".into()));
    assert!(st.s.ui.menu.is_none());
    let kept = std::fs::read_to_string(st.s.dir.join("agent-models.txt")).unwrap();
    assert_eq!(kept, "claude=haiku\n");
    assert_eq!(st.s.editor.text(), "");
    // Home and End go to the first and the last; the studio's key closes
    // the menu with the studio, Escape the menu alone.
    redraw(&mut st, &mut game);
    let chip = text_at(&game, "Model: Haiku");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    handle(&mut st, &mut game, key(Key::End, NONE));
    let menu = st.s.ui.menu.clone().unwrap();
    assert_eq!(menu.items[menu.sel.unwrap()].label, "Other model...");
    handle(&mut st, &mut game, key(Key::Home, NONE));
    let menu = st.s.ui.menu.clone().unwrap();
    assert_eq!(menu.items[menu.sel.unwrap()].label, "Default");
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Used);
    assert!(st.s.ui.menu.is_none() && st.s.ui.open);
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 2.0));
    assert_eq!(handle(&mut st, &mut game, key(Key::Toggle, NONE)), UiResult::Close);
    assert!(st.s.ui.menu.is_none());
}

#[test]
fn a_menu_left_open_is_dropped_when_the_studio_opens_again() {
    let mut game = FakeGame::new("ui-menu-stale");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.now = 5.0;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let hello = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (hello.0 + 2.0, hello.1 + 2.0));
    redraw(&mut st, &mut game);
    assert!(st.s.ui.menu.is_some(), "it stays while the studio is open");
    st.s.now = 9.0;
    st.s.ui.opened_at = st.s.now;
    redraw(&mut st, &mut game);
    assert!(st.s.ui.menu.is_none());
}

#[test]
fn a_click_outside_closes_the_menu_and_goes_no_further() {
    let mut game = FakeGame::new("ui-menu-outside");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let hello = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (hello.0 + 2.0, hello.1 + 2.0));
    assert!(menu_items(&st).contains(&"Make it a module".to_owned()));
    redraw(&mut st, &mut game);
    let target =
        st.s.ui
            .menu
            .as_ref()
            .and_then(|m| m.target)
            .expect("its row is outlined");
    assert!(target[1] <= hello.1 && hello.1 < target[1] + target[3], "{target:?}");
    // A right press on another row (left of the menu) puts that row's menu in its place.
    let kit = text_at(&game, "kit/");
    right_click(&mut st, &mut game, (kit.0 - 10.0, kit.1 + 2.0));
    assert_eq!(menu_items(&st)[0], "New file...");
    // A left click outside closes it, and the tab under it stays as it was.
    redraw(&mut st, &mut game);
    let library = text_at(&game, "Library");
    click(&mut st, &mut game, (library.0 + 2.0, library.1 + 2.0));
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.ui.tab, Tab::Editor);
    // Under the naming prompt, a right press opens nothing, and the wheel scrolls nothing.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    handle(&mut st, &mut game, Input::Text("n".into()));
    redraw(&mut st, &mut game);
    let row = text_at(&game, "UI Kit");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert!(st.s.ui.menu.is_none());
    assert!(st.s.ui.naming.is_some());
    let sel = st.s.ui.library_sel;
    handle(&mut st, &mut game, Input::Wheel(-3.0));
    assert_eq!(st.s.ui.library_sel, sel, "the Library under it stays");
}

/// The open menu's items that can be chosen, by label.
fn usable(st: &horizon_studio::game::hstudio::Studio) -> Vec<String> {
    menu_labels(st)
        .into_iter()
        .filter(|(l, on)| *on && !l.is_empty())
        .map(|(l, _)| l)
        .collect()
}

/// The open menu's lit item.
fn lit(st: &horizon_studio::game::hstudio::Studio) -> String {
    let menu = st.s.ui.menu.as_ref().expect("a menu");
    menu.sel.map(|i| menu.items[i].label.clone()).unwrap_or_default()
}

/// Chooses the open menu's item `label`, by a click where it was drawn.
fn choose(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, label: &str) {
    redraw(st, game);
    let at = last_text_at(game, label);
    click(st, game, at);
}

/// A studio on the Editor tab with `text` in the editor, drawn, and no
/// agent found (whatever this computer has).
fn editing(name: &str, text: &str) -> (FakeGame, horizon_studio::game::hstudio::Studio) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.agent.agents = Some(Vec::new());
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.set_text(text);
    redraw(&mut st, &mut game);
    (game, st)
}

#[test]
fn right_click_on_the_code_opens_its_menu_at_the_pointer() {
    let (mut game, mut st) = editing("ui-code-menu", "local a = 1\nprint(a)");
    // Suggestions open, from typing at the end of the first line.
    st.s.editor.click(0, 11, false);
    type_text(&mut st, &mut game, " pla");
    assert!(st.s.ui.suggest.is_some());
    redraw(&mut st, &mut game);
    let print = text_at(&game, "print");
    let at = (print.0 + 8.0, print.1 + 4.0);
    right_click(&mut st, &mut game, at);
    let menu = st.s.ui.menu.clone().expect("the code's menu");
    assert_eq!(menu.at, [at.0, at.1], "at the pointer");
    assert_eq!(
        usable(&st),
        [
            "Cut",
            "Copy",
            "Paste",
            "Select all",
            "Toggle comment",
            "Indent",
            "Outdent",
            "Duplicate line",
            "Run",
            "Save"
        ]
    );
    let hints: Vec<String> = menu.items.iter().map(|i| i.hint.clone()).collect();
    assert_eq!(hints[0], "Ctrl X");
    assert_eq!((st.s.editor.row, st.s.editor.col), (1, 1), "the cursor went there");
    assert!(st.s.ui.suggest.is_none());
    // Paste with nothing on the clipboard does nothing.
    choose(&mut st, &mut game, "Paste");
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.editor.text(), "local a = 1 pla\nprint(a)");
}

#[test]
fn choosing_an_editor_item_runs_it_and_closes_the_menu() {
    let (mut game, mut st) = editing("ui-code-items", "local a = 1\nprint(a)");
    let print = text_at(&game, "print");
    right_click(&mut st, &mut game, (print.0 + 2.0, print.1 + 4.0));
    choose(&mut st, &mut game, "Toggle comment");
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.editor.lines[1], "-- print(a)");
    // Copy with nothing selected copies the line, its line break too.
    redraw(&mut st, &mut game);
    let local = text_at(&game, "local");
    right_click(&mut st, &mut game, (local.0 + 2.0, local.1 + 4.0));
    choose(&mut st, &mut game, "Copy");
    assert_eq!(game.clipboard, "local a = 1\n");
    redraw(&mut st, &mut game);
    assert!(drawn_text(&game, "Copied 12 characters"), "it says so");
    // Paste puts the clipboard at the cursor; Indent indents the line itself.
    game.clipboard = "x".into();
    right_click(&mut st, &mut game, (local.0 + 0.5, local.1 + 4.0));
    choose(&mut st, &mut game, "Paste");
    assert_eq!(st.s.editor.lines[0], "xlocal a = 1");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (local.0 + 2.0, local.1 + 4.0));
    choose(&mut st, &mut game, "Indent");
    assert_eq!(st.s.editor.lines[0], "  xlocal a = 1");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (local.0 + 2.0, local.1 + 4.0));
    choose(&mut st, &mut game, "Outdent");
    assert_eq!(st.s.editor.lines[0], "xlocal a = 1");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (local.0 + 2.0, local.1 + 4.0));
    choose(&mut st, &mut game, "Duplicate line");
    assert_eq!(st.s.editor.lines[..2], ["xlocal a = 1", "xlocal a = 1"]);
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (local.0 + 2.0, local.1 + 4.0));
    choose(&mut st, &mut game, "Cut");
    assert_eq!(game.clipboard, "xlocal a = 1\n");
    assert_eq!(st.s.editor.text(), "xlocal a = 1\n-- print(a)");
    // Run runs it in the console.
    st.s.editor.set_text("print('ran from the menu')");
    redraw(&mut st, &mut game);
    let code = text_at(&game, "print");
    right_click(&mut st, &mut game, (code.0 + 2.0, code.1 + 4.0));
    choose(&mut st, &mut game, "Run");
    assert_eq!(log(&st).last().map(String::as_str), Some("ran from the menu"));
}

#[test]
fn a_right_click_inside_the_selection_keeps_it() {
    let (mut game, mut st) = editing("ui-code-selection", "local a = 1\nprint(a)");
    let local = text_at(&game, "local");
    handle(&mut st, &mut game, Input::Mouse(local.0, local.1 + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(local.0 + 6.0 * 5.0, local.1 + 4.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("local"));
    right_click(&mut st, &mut game, (local.0 + 8.0, local.1 + 4.0));
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("local"), "kept");
    choose(&mut st, &mut game, "Copy");
    assert_eq!(game.clipboard, "local");
    // On another line it goes, and the cursor goes there.
    redraw(&mut st, &mut game);
    let print = text_at(&game, "print");
    right_click(&mut st, &mut game, (print.0 + 2.0, print.1 + 4.0));
    assert_eq!(st.s.editor.selection(), None);
    assert_eq!(st.s.editor.row, 1);
}

#[test]
fn the_menus_keys_choose_skip_and_close() {
    let (mut game, mut st) = editing("ui-menu-keys", "local a = 1\nprint(a)");
    let print = text_at(&game, "print");
    let at = (print.0 + 2.0, print.1 + 4.0);
    right_click(&mut st, &mut game, at);
    assert_eq!(lit(&st), "", "nothing lit at first");
    handle(&mut st, &mut game, key(Key::Down, NONE));
    assert_eq!(lit(&st), "Cut");
    for _ in 0..3 {
        handle(&mut st, &mut game, key(Key::Down, NONE));
    }
    assert_eq!(lit(&st), "Select all");
    handle(&mut st, &mut game, key(Key::Down, NONE));
    assert_eq!(lit(&st), "Toggle comment", "over the separator");
    handle(&mut st, &mut game, key(Key::Up, NONE));
    assert_eq!(lit(&st), "Select all");
    // A letter lights the next item that starts with it, and types nothing.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    right_click(&mut st, &mut game, at);
    type_text(&mut st, &mut game, "s");
    assert_eq!(lit(&st), "Select all");
    type_text(&mut st, &mut game, "s");
    assert_eq!(lit(&st), "Save");
    type_text(&mut st, &mut game, "s");
    assert_eq!(lit(&st), "Select all", "round again");
    assert_eq!(st.s.editor.text(), "local a = 1\nprint(a)");
    // Space chooses it.
    handle(&mut st, &mut game, Input::Text(" ".into()));
    assert!(st.s.ui.menu.is_none());
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("local a = 1\nprint(a)"));
    // Enter chooses the lit item; with none lit it waits.
    right_click(&mut st, &mut game, at);
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.menu.is_some());
    handle(&mut st, &mut game, key(Key::Down, NONE));
    handle(&mut st, &mut game, key(Key::Down, NONE));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.ui.menu.is_none());
    assert_eq!(game.clipboard, "local a = 1\nprint(a)", "Copy, on the selection");
    // Escape closes the menu, not the studio; the studio's key closes both.
    right_click(&mut st, &mut game, at);
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Used);
    assert!(st.s.ui.menu.is_none() && st.s.ui.open);
    right_click(&mut st, &mut game, at);
    assert_eq!(handle(&mut st, &mut game, key(Key::Toggle, NONE)), UiResult::Close);
    assert!(st.s.ui.menu.is_none());
}

#[test]
fn console_text_and_input_menus_copy_paste_and_clear() {
    use horizon_studio::game::hstudio::LogKind;
    let mut game = FakeGame::new("ui-console-menus");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.now = 10.0;
    handle(&mut st, &mut game, key(Key::Char('1'), CTRL));
    st.s.log(LogKind::Info, "alpha bravo charlie");
    st.s.log(LogKind::Info, "delta echo foxtrot");
    redraw(&mut st, &mut game);
    // A selection, and Copy in its menu.
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
    right_click(&mut st, &mut game, (a.0 + 12.0, a.1 + 4.0));
    assert_eq!(usable(&st), ["Copy", "Copy all", "Clear"]);
    assert_eq!(hint_of(&st, "Clear"), "Ctrl L", "the console's input has the keys");
    choose(&mut st, &mut game, "Copy");
    assert_eq!(game.clipboard, "alpha bravo charlie\ndelta echo foxtrot");
    // Copy all: every line.
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (a.0 + 12.0, a.1 + 4.0));
    choose(&mut st, &mut game, "Copy all");
    assert!(
        game.clipboard
            .contains("Type Lua and press Enter; help() lists the API.")
    );
    assert!(game.clipboard.ends_with("alpha bravo charlie\ndelta echo foxtrot"));
    // The input: Paste, then Run.
    game.clipboard = "6 * 7".into();
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Type Lua and press Enter. Tab completes, help() lists the API.");
    right_click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(usable(&st), ["Paste", "Clear the console"]);
    choose(&mut st, &mut game, "Paste");
    assert_eq!(st.s.ui.input.text, "6 * 7");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert!(usable(&st).contains(&"Copy the line".to_owned()));
    choose(&mut st, &mut game, "Run");
    assert_eq!(log(&st).last().map(String::as_str), Some("42"));
    assert_eq!(st.s.ui.input.text, "");
    // Cut the line and Clear the line.
    st.s.ui.input.set("print(1)");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    choose(&mut st, &mut game, "Cut the line");
    assert_eq!((game.clipboard.as_str(), st.s.ui.input.text.as_str()), ("print(1)", ""));
    // Scrolled up, Jump to the newest goes back down.
    for i in 0..60 {
        st.s.log(LogKind::Info, format!("line {i}"));
    }
    handle(&mut st, &mut game, Input::Wheel(2.0));
    assert!(st.s.ui.log_scroll > 0);
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (a.0 + 12.0, a.1 + 4.0));
    choose(&mut st, &mut game, "Jump to the newest");
    assert_eq!(st.s.ui.log_scroll, 0);
    // Clear empties the console.
    redraw(&mut st, &mut game);
    let line = last_text_at(&game, "line 59");
    right_click(&mut st, &mut game, line);
    choose(&mut st, &mut game, "Clear");
    assert!(st.s.log.is_empty());
    // In the Editor's panel, with the keys on the code, Ctrl+L clears
    // nothing and Clear shows no keys; with them in the panel's input, it does.
    st.s.log(LogKind::Info, "golf hotel");
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let line = last_text_at(&game, "golf hotel");
    right_click(&mut st, &mut game, line);
    assert_eq!(hint_of(&st, "Clear"), "");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    handle(&mut st, &mut game, key(Key::Char('l'), CTRL));
    assert!(!st.s.log.is_empty());
    handle(&mut st, &mut game, key(Key::Char('j'), CTRL));
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, line);
    assert_eq!(hint_of(&st, "Clear"), "Ctrl L");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    handle(&mut st, &mut game, key(Key::Char('l'), CTRL));
    assert!(st.s.log.is_empty());
}

/// The keys shown beside the open menu's item `label`.
fn hint_of(st: &horizon_studio::game::hstudio::Studio, label: &str) -> String {
    let menu = st.s.ui.menu.as_ref().expect("a menu");
    let item = menu.items.iter().find(|i| i.label == label);
    item.unwrap_or_else(|| panic!("no {label:?}")).hint.clone()
}

#[test]
fn the_agents_conversation_and_input_have_menus() {
    use horizon_studio::game::hstudio_ui::{Pane, agent_new_conversation};
    let (mut game, mut st) = editing("ui-agent-menus", "");
    st.s.ui.agent.agents = Some(Vec::new());
    st.s.ui.dock.activate(Pane::Agent);
    agent_new_conversation(&mut st);
    redraw(&mut st, &mut game);
    let said = text_at(&game, "New conversation.");
    right_click(&mut st, &mut game, (said.0 + 2.0, said.1 + 4.0));
    assert_eq!(usable(&st), ["Copy the conversation", "New conversation"]);
    choose(&mut st, &mut game, "Copy the conversation");
    assert_eq!(game.clipboard, "New conversation.");
    // Its input: Paste, then Clear the line; it takes the keys.
    game.clipboard = "make it glow".into();
    redraw(&mut st, &mut game);
    let field = text_at(&game, "No agent found.");
    right_click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    assert_eq!(st.s.ui.focus, Pane::Agent);
    assert_eq!(usable(&st), ["Paste"], "nothing to send without an agent's line");
    choose(&mut st, &mut game, "Paste");
    assert_eq!(st.s.ui.agent_input.text, "make it glow");
    redraw(&mut st, &mut game);
    let field = text_at(&game, "make it glow");
    right_click(&mut st, &mut game, (field.0 + 2.0, field.1 + 4.0));
    choose(&mut st, &mut game, "Clear the line");
    assert_eq!(st.s.ui.agent_input.text, "");
}

#[test]
fn left_clicks_pass_through_context_spots() {
    let (mut game, mut st) = editing("ui-spots-click", "abcdef\nsecond line");
    let at = text_at(&game, "second");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1 + 4.0));
    assert!(st.s.ui.menu.is_some(), "a right click there has a menu");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let first = text_at(&game, "abcdef");
    click(&mut st, &mut game, (first.0 + 6.0 * 3.0 + 1.0, first.1 + 4.0));
    assert_eq!(
        (st.s.editor.row, st.s.editor.col),
        (0, 3),
        "a left click still moves the cursor"
    );
}

#[test]
fn no_menu_on_the_scroll_bar_the_suggestions_or_a_banner() {
    let text: Vec<String> = (1..=300).map(|i| format!("local v{i} = {i}")).collect();
    let (mut game, mut st) = editing("ui-no-menu", &text.join("\n"));
    let sb = st.s.ui.scrollbar.expect("a scroll bar");
    right_click(&mut st, &mut game, (sb.x, sb.top + 2.0));
    assert!(st.s.ui.menu.is_none(), "the scroll bar");
    // The suggestions under the cursor.
    st.s.editor.click(0, 0, false);
    type_text(&mut st, &mut game, "player.");
    redraw(&mut st, &mut game);
    let first = st.s.ui.suggest.as_ref().expect("suggestions").items[0].label.clone();
    let at = last_text_at(&game, &first);
    right_click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_none(), "the suggestions");
    // The banner of a file changed on disk.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    st.s.editor.conflict = true;
    redraw(&mut st, &mut game);
    let said = text_at(&game, "This file changed on disk while you were editing it.");
    right_click(&mut st, &mut game, (said.0 + 2.0, said.1 + 4.0));
    assert!(st.s.ui.menu.is_none(), "the banner");
}

/// Empties the naming prompt's field.
fn clear_field(st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame) {
    let n = st.s.ui.naming.as_ref().map_or(0, |n| n.text.chars().count());
    handle(st, game, key(Key::End, NONE));
    for _ in 0..n {
        handle(st, game, key(Key::Backspace, NONE));
    }
}

#[test]
fn the_script_list_renames_a_mod_and_the_editor_follows() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-rename-mod");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.editor.click(0, 0, false);
    type_text(&mut st, &mut game, "-- mine\n");
    redraw(&mut st, &mut game);
    let row = last_text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert!(!menu_items(&st).contains(&"Open".to_owned()), "it is open");
    choose(&mut st, &mut game, "Rename...");
    assert_eq!(st.s.ui.naming_kind, NamingKind::RenameMod("hello".into()));
    assert_eq!(st.s.ui.naming.as_ref().map(|n| n.text.as_str()), Some("hello"));
    clear_field(&mut st, &mut game);
    type_text(&mut st, &mut game, "greeter");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "Rename hello.lua"));
    assert!(drawn_contains(&game, "hello.lua becomes greeter.lua"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let mods = game.mods_dir();
    assert!(mods.join("greeter.lua").is_file() && !mods.join("hello.lua").exists());
    assert_eq!(st.s.mods[i].id, "greeter", "in hello's place");
    assert!(st.s.mods[i].running());
    assert_eq!(st.s.editor.path.as_ref(), Some(&mods.join("greeter.lua")));
    assert_eq!(st.s.editor.title, "greeter.lua");
    // The edit was saved first; the name in the Library follows the id it was.
    assert!(
        st.s.editor.text().starts_with("-- mine\n-- name: greeter\n"),
        "{}",
        st.s.editor.text()
    );
    assert!(!st.s.editor.dirty);
    assert_eq!(st.s.mods[i].name, "greeter");
    // Ctrl+Z takes back the name, then the edit.
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    assert!(st.s.editor.text().starts_with("-- mine\n-- name: Hello\n"));
    for _ in 0..10 {
        handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    }
    assert!(
        st.s.editor.text().starts_with("-- name: Hello\n"),
        "{}",
        st.s.editor.text()
    );
}

#[test]
fn duplicate_makes_a_copy_that_starts_off() {
    let mut game = FakeGame::new("ui-duplicate-mod");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Duplicate...");
    assert_eq!(st.s.ui.naming.as_ref().map(|n| n.text.as_str()), Some("hello_copy"));
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "Duplicate hello.lua"));
    assert!(drawn_contains(&game, "hello_copy.lua: a copy of hello.lua, turned off"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let copy = game.mods_dir().join("hello_copy.lua");
    assert!(
        std::fs::read_to_string(&copy)
            .unwrap()
            .starts_with("-- name: hello_copy\n")
    );
    let c = mod_index(&st, "hello_copy");
    assert!(!st.s.mods[c].enabled);
    assert_eq!(st.s.ui.library_sel, c, "chosen in the Library");
    assert_eq!(st.s.editor.path.as_ref(), Some(&copy), "open in the editor");
    assert!(
        std::fs::read_to_string(st.s.dir.join("studio.txt"))
            .unwrap()
            .contains("off:hello_copy")
    );
    // Its menu: Reload is greyed out while it is off, and a click on it
    // turns nothing on; Turn on turns it on (where it fails: its /hello is
    // hello's).
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello_copy.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert!(menu_labels(&st).contains(&("Reload".to_owned(), false)));
    choose(&mut st, &mut game, "Reload");
    assert!(!st.s.mods[c].enabled && st.s.ui.menu.is_some());
    choose(&mut st, &mut game, "Turn on");
    assert!(st.s.mods[c].enabled);
    assert!(st.s.mods[mod_index(&st, "hello")].running(), "the original runs on");
    // On, it reloads from its menu.
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello_copy.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert!(menu_labels(&st).contains(&("Reload".to_owned(), true)));
}

#[test]
fn delete_asks_first_and_moves_the_mod_to_the_trash() {
    let mut game = FakeGame::new("ui-delete-mod");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    let menu = st.s.ui.menu.clone().unwrap();
    assert!(menu.items.iter().any(|it| it.label == "Delete..." && it.danger));
    choose(&mut st, &mut game, "Delete...");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "Delete hello.lua?"));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let hello = game.mods_dir().join("hello.lua");
    assert!(hello.is_file(), "Escape deletes nothing");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Delete...");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(!hello.exists() && st.s.dir.join("trash").join("hello.lua").is_file());
    assert!(!st.s.mods.iter().any(|m| m.id == "hello"));
    assert_eq!(st.s.editor.title, "scratch.lua");
    assert!(
        !std::fs::read_to_string(st.s.dir.join("studio.txt"))
            .unwrap()
            .contains("off:hello")
    );
    // A hello made again and deleted again: the prompt says the copy in the
    // trash stays, and both are there after.
    game.write_mod("hello.lua", "print('hello again')\n");
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Delete...");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(
        &game,
        "It moves to trash/hello.lua beside the mods folder."
    ));
    assert!(drawn_contains(&game, "The older copy there stays, as hello (2).lua."));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let trash = st.s.dir.join("trash");
    assert_eq!(
        std::fs::read_to_string(trash.join("hello.lua")).unwrap(),
        "print('hello again')\n"
    );
    assert!(trash.join("hello (2).lua").is_file());
}

#[test]
fn library_rows_have_the_mods_menu() {
    let mut game = FakeGame::new("ui-library-menus");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "uikit");
    redraw(&mut st, &mut game);
    // A right click on a row chooses it; Turn off turns it off.
    let hello = text_at(&game, "Your first script mod. Type /hello in chat, then o…");
    right_click(&mut st, &mut game, (hello.0 + 2.0, hello.1 + 2.0));
    let h = mod_index(&st, "hello");
    assert_eq!(st.s.ui.library_sel, h);
    choose(&mut st, &mut game, "Turn off");
    assert!(!st.s.mods[h].enabled);
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (hello.0 + 2.0, hello.1 + 2.0));
    assert!(menu_items(&st).contains(&"Turn on".to_owned()));
    // Show in folder: the mods folder for a script, its own for a module.
    choose(&mut st, &mut game, "Show in folder");
    assert_eq!(game.opened.last(), Some(&game.mods_dir()));
    st.s.ui.library_sel = mod_index(&st, "kit");
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Text(".".into()));
    choose(&mut st, &mut game, "Show in folder");
    assert_eq!(game.opened.last(), Some(&game.mods_dir().join("kit")));
    // An example's menu installs it, and copies its code.
    redraw(&mut st, &mut game);
    let clock = text_at(&game, "Clock");
    right_click(&mut st, &mut game, (clock.0 + 2.0, clock.1 + 2.0));
    assert_eq!(menu_items(&st), ["Install", "Copy its code"]);
    choose(&mut st, &mut game, "Copy its code");
    assert!(game.clipboard.contains("-- name:"), "{}", game.clipboard);
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (clock.0 + 2.0, clock.1 + 2.0));
    choose(&mut st, &mut game, "Install");
    assert!(st.s.mods.iter().any(|m| m.id == "clock"));
    // The list's own menu, off its rows (the page's pane docks in the
    // Editor tab from it).
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Script mods");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(
        menu_items(&st),
        [
            "New script...",
            "New module...",
            "",
            "Reload all",
            "Open the mods folder",
            "",
            "Dock in the Editor"
        ]
    );
}

#[test]
fn a_mod_gone_since_its_menu_opened_is_said_to_be_gone() {
    let mut game = FakeGame::new("ui-menu-gone");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    game.write_mod("zz.lua", "print('zz')\n");
    run_for(&mut st, &mut game, 0.6);
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let row = text_at(&game, "zz.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    redraw(&mut st, &mut game);
    std::fs::remove_file(game.mods_dir().join("zz.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    let at = last_text_at(&game, "Turn off");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.ui.notice.as_ref().map(|n| n.0.as_str()), Some("zz is gone"));
    assert!(st.s.mods.iter().all(|m| m.enabled), "no other mod turned off");
    // One turned off since its menu opened: its Reload leaves it off.
    redraw(&mut st, &mut game);
    let row = text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    redraw(&mut st, &mut game);
    let h = mod_index(&st, "hello");
    st.set_enabled(&mut game, h, false);
    let at = last_text_at(&game, "Reload");
    click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_none(), "chosen");
    assert!(!st.s.mods[h].enabled, "still off");
}

#[test]
fn the_files_menu_saves_reverts_and_renames() {
    use horizon_studio::game::hstudio_ui::NamingKind;
    let mut game = FakeGame::new("ui-file-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    // The scratch file: Save as a script, and its folder, then where the
    // code's pane is. (Its tab is drawn after the script list's row.)
    let title = last_text_at(&game, "scratch.lua");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(
        usable(&st),
        [
            "Save",
            "Save as a script...",
            "Show in folder",
            "Copy path",
            "Move",
            "Size...",
            "Panes"
        ]
    );
    choose(&mut st, &mut game, "Copy path");
    assert_eq!(game.clipboard, st.s.dir.join("scratch.lua").display().to_string());
    // A mod's file: Revert to saved drops the edits, and Ctrl+Z brings them back.
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    let saved = st.s.editor.text();
    st.s.editor.click(0, 0, false);
    type_text(&mut st, &mut game, "-- mine\n");
    redraw(&mut st, &mut game);
    let title = last_text_at(&game, "hello.lua");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(
        usable(&st),
        [
            "Save",
            "Revert to saved",
            "Rename...",
            "Show in folder",
            "Copy path",
            "Move",
            "Size...",
            "Panes"
        ]
    );
    choose(&mut st, &mut game, "Revert to saved");
    assert_eq!(st.s.editor.text(), saved);
    assert!(!st.s.editor.dirty);
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    assert!(st.s.editor.text().starts_with("-- mine\n"));
    assert!(st.s.editor.dirty);
    // Rename names the mod.
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    choose(&mut st, &mut game, "Rename...");
    assert_eq!(st.s.ui.naming_kind, NamingKind::RenameMod("hello".into()));
}

#[test]
fn the_script_lists_own_menu_and_synapses() {
    use horizon_studio::game::hstudio_theme::THEMES;
    let mut game = FakeGame::new("ui-scripts-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    st.open_file(&game.mods_dir().join("kit").join("lib.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Scripts");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(
        menu_items(&st),
        [
            "New script...",
            "New module...",
            "",
            "New file in kit...",
            "",
            "Reload all",
            "Open the mods folder",
            "",
            "Hide the script list",
            "Move",
            "Size...",
            "Panes",
            "Reset layout"
        ]
    );
    choose(&mut st, &mut game, "Open the mods folder");
    assert_eq!(game.opened.last(), Some(&game.mods_dir()));
    // Synapse's list and its file's tab have the same menus.
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Scripts");
    right_click(&mut st, &mut game, (title.0 + 2.0, title.1 + 2.0));
    assert_eq!(menu_items(&st)[0], "New script...");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let tab = text_at(&game, "kit/lib.lua");
    right_click(&mut st, &mut game, (tab.0 + 2.0, tab.1 + 2.0));
    assert_eq!(menu_items(&st)[0], "Save");
    assert!(menu_items(&st).contains(&"Rename...".to_owned()));
    let row = text_at(&game, "shared.lua");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert_eq!(menu_items(&st)[0], "Open");
}

#[test]
fn problems_and_panel_tabs_have_menus() {
    use horizon_studio::game::hstudio_ui::Pane;
    let (mut game, mut st) = editing("ui-problem-menus", "local here = 1\n\nprint(player.poss())\n");
    st.s.now = 10.0;
    st.s.ui.dock.activate(Pane::Problems);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.problems.len(), 1);
    // The Problems pane's row (drawn before the code, which has the keys and
    // ends the line with the same words).
    let row = text_at(&game, "player has no 'poss'");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert_eq!(usable(&st), ["Go to it", "Copy", "Copy all problems"]);
    choose(&mut st, &mut game, "Go to it");
    assert_eq!(st.s.editor.row, 2, "on its line");
    assert_eq!(
        st.s.editor.selected_text().as_deref(),
        Some("poss"),
        "the name selected"
    );
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Copy");
    assert_eq!(game.clipboard, "scratch.lua:3: player has no 'poss'");
    // The tabs' menus: the Problems tab, then the Console's Hide the panel,
    // each then with where the pane is.
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Problems");
    right_click(&mut st, &mut game, tab);
    assert_eq!(
        usable(&st),
        ["Copy all problems", "Hide the panel", "Move", "Size...", "Panes"]
    );
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let tab = last_text_at(&game, "Console");
    right_click(&mut st, &mut game, tab);
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
    choose(&mut st, &mut game, "Hide the panel");
    assert!(!st.s.ui.dock.shown(Pane::Console) && st.s.ui.focus == Pane::Code);
}

#[test]
fn the_title_bar_menu_leaves_an_executor_style() {
    use horizon_studio::game::hstudio_theme::THEMES;
    let mut game = FakeGame::new("ui-window-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let synapse = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    st.s.ui.look.theme = synapse;
    st.s.ui.tab = Tab::Editor;
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (title.0 + 40.0, title.1 + 4.0));
    assert_eq!(
        menu_items(&st),
        [
            "Roll up",
            "Fill the screen",
            "Reset window",
            "Theme",
            "",
            "Settings",
            "Close"
        ]
    );
    assert_eq!(usable(&st)[2], "Theme", "nothing to reset yet");
    // Theme lists every theme in the menu's place, the one in use marked.
    choose(&mut st, &mut game, "Theme");
    let menu = st.s.ui.menu.clone().expect("the themes");
    let names: Vec<&str> = menu.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(names, THEMES.iter().map(|t| t.name).collect::<Vec<_>>());
    assert!(menu.items[synapse].checked);
    assert_eq!(menu.sel, Some(synapse), "lit, for the keys");
    choose(&mut st, &mut game, "Horizon");
    assert_eq!(st.s.ui.look.theme, 0);
    assert!(
        std::fs::read_to_string(st.s.dir.join("look.txt"))
            .unwrap()
            .contains("theme=horizon")
    );
    // A left press on the title bar still drags the window.
    st.s.ui.look.theme = synapse;
    redraw(&mut st, &mut game);
    let before = st.s.ui.look.window_rect(960.0, 540.0);
    let title = text_at(&game, "Horizon Studio");
    handle(&mut st, &mut game, Input::Mouse(title.0 + 100.0, title.1 + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(title.0 + 80.0, title.1 + 24.0));
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    let after = st.s.ui.look.window_rect(960.0, 540.0);
    assert_eq!((after[0] - before[0], after[1] - before[1]), (-20.0, 20.0));
    // Roll up from the menu, and back.
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (title.0 + 40.0, title.1 + 4.0));
    choose(&mut st, &mut game, "Roll up");
    assert!(st.s.ui.collapsed);
}

#[test]
fn the_headers_menu_and_the_docs() {
    let mut game = FakeGame::new("ui-header-menu");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    redraw(&mut st, &mut game);
    let name = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (name.0 + 4.0, name.1 + 4.0));
    assert_eq!(
        menu_items(&st),
        ["In a window", "Theme", "Panes", "Reset layout", "", "Settings", "Close"]
    );
    choose(&mut st, &mut game, "In a window");
    assert!(st.s.ui.look.window);
    // Close closes the studio.
    redraw(&mut st, &mut game);
    let name = text_at(&game, "Horizon Studio");
    right_click(&mut st, &mut game, (name.0 + 40.0, name.1 + 4.0));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "Close");
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    let out = handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(out, UiResult::Close);
    // The Docs: copied as Markdown.
    st.s.ui.tab = Tab::Docs;
    st.s.ui.look.window = false;
    redraw(&mut st, &mut game);
    let heading = text_at(&game, "Script mods");
    right_click(&mut st, &mut game, (heading.0 + 4.0, heading.1 + 4.0));
    choose(&mut st, &mut game, "Copy the docs as Markdown");
    assert!(game.clipboard.starts_with("# Horizon Studio scripting\n"));
}

#[test]
fn the_executor_styles_have_the_menus_too() {
    use horizon_studio::game::hstudio::LogKind;
    use horizon_studio::game::hstudio_theme::THEMES;
    for theme in ["protosmasher", "protoclassic", "rc7"] {
        let mut game = FakeGame::new(&format!("ui-style-menus-{theme}"));
        let mut st = studio(&mut game);
        st.s.ui.open = true;
        st.s.ui.look.motion = false;
        st.s.ui.look.theme = THEMES.iter().position(|t| t.id == theme).unwrap();
        st.s.ui.tab = Tab::Editor;
        st.s.editor.set_text("print('menus')");
        st.s.log(LogKind::Info, "said in the output");
        redraw(&mut st, &mut game);
        // The code.
        let code = text_at(&game, "print");
        right_click(&mut st, &mut game, (code.0 + 2.0, code.1 + 4.0));
        assert_eq!(menu_items(&st)[0], "Cut", "{theme}");
        handle(&mut st, &mut game, key(Key::Escape, NONE));
        // The output, where it has one.
        if theme != "rc7" {
            let said = text_at(&game, "said in the output");
            right_click(&mut st, &mut game, (said.0 + 2.0, said.1 + 4.0));
            assert_eq!(menu_items(&st)[1], "Copy all", "{theme}");
            handle(&mut st, &mut game, key(Key::Escape, NONE));
        }
        // ProtoSmasher's theme dropdown lists the themes.
        if theme == "protosmasher" {
            let name = text_at(&game, "ProtoSmasher");
            right_click(&mut st, &mut game, (name.0 + 2.0, name.1 + 4.0));
            assert_eq!(menu_items(&st).len(), THEMES.len(), "{theme}");
            choose(&mut st, &mut game, "Nord");
            assert_eq!(st.s.ui.look.theme().id, "nord");
        }
    }
}

#[cfg(unix)]
#[test]
fn the_agent_tab_menu_chooses_an_agent_and_starts_over() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{Pane, agent_new_conversation};
    let (mut game, mut st) = editing("ui-agent-tab-menu", "");
    let echo = AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    };
    st.s.ui.agent.agents = Some(vec![echo, AgentSpec::claude(std::path::PathBuf::from("claude"))]);
    st.s.ui.agent.session = Some("before".into());
    st.s.ui.dock.activate(Pane::Agent);
    agent_new_conversation(&mut st);
    st.s.ui.agent.session = Some("before".into());
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Agent");
    right_click(&mut st, &mut game, tab);
    let menu = st.s.ui.menu.clone().unwrap();
    let echo = menu.items.iter().find(|i| i.label == "Echo").expect("Echo listed");
    assert!(echo.checked, "the one in use");
    assert!(menu.items.iter().any(|i| i.label == "Claude Code" && !i.checked));
    choose(&mut st, &mut game, "Claude Code");
    assert_eq!(st.s.ui.agent.chosen, 1);
    assert_eq!(st.s.ui.agent.session, None, "a new conversation");
    // New conversation empties it.
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Agent");
    right_click(&mut st, &mut game, tab);
    choose(&mut st, &mut game, "New conversation");
    assert_eq!(st.s.ui.agent.lines.len(), 1);
    assert_eq!(st.s.ui.agent.lines[0].text, "New conversation.");
    // Claude Code's model: its list in the menu's place.
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Agent");
    right_click(&mut st, &mut game, tab);
    choose(&mut st, &mut game, "Model: Default");
    assert_eq!(
        menu_items(&st),
        ["Default", "Opus", "Sonnet", "Haiku", "Fable", "", "Other model..."]
    );
    assert_eq!(lit(&st), "Default");
}

#[test]
fn go_to_definition_and_show_in_docs() {
    let src = "local function greet(n)\n  return n\nend\nprint(greet(1), player.pos())";
    let (mut game, mut st) = editing("ui-definition", src);
    redraw(&mut st, &mut game);
    // greet on line 4: its definition, on line 1, selected.
    let at = last_text_at(&game, "greet");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1));
    let items = menu_items(&st);
    assert_eq!(items[0], "Go to definition", "{items:?}");
    choose(&mut st, &mut game, "Go to definition");
    assert_eq!(st.s.editor.row, 0);
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("greet"));
    // On the definition itself it isn't offered.
    redraw(&mut st, &mut game);
    let def = text_at(&game, "greet");
    right_click(&mut st, &mut game, (def.0 + 6.0, def.1 + 4.0));
    assert!(!menu_items(&st).contains(&"Go to definition".to_owned()));
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // player.pos: its entry in the Docs, scrolled to and drawn.
    let pos = last_text_at(&game, "pos");
    right_click(&mut st, &mut game, (pos.0 + 2.0, pos.1));
    choose(&mut st, &mut game, "Show player.pos in Docs");
    assert_eq!(st.s.ui.tab, Tab::Docs);
    redraw(&mut st, &mut game);
    assert!(st.s.ui.docs_scroll > 0);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text player.pos(")),
        "the entry is on the page"
    );
}

#[test]
fn the_code_opens_the_library_or_the_file_it_names() {
    let mut game = FakeGame::new("ui-open-required");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    std::fs::write(
        dir.join("main.lua"),
        "local lib = require(\"lib\")\nlocal ui = import(\"uikit\")\nlib.greet(\"x\")\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("lib.lua"),
        "local lib = {}\nfunction lib.greet(s) return s end\nreturn lib\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    st.open_file(&dir.join("main.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    // require("lib"): Open lib.lua.
    let req = last_text_at(&game, "\"lib\"");
    right_click(&mut st, &mut game, (req.0 + 4.0, req.1));
    assert_eq!(menu_items(&st)[0], "Open lib.lua");
    // import("uikit"): Open uikit.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let imp = last_text_at(&game, "\"uikit\"");
    right_click(&mut st, &mut game, (imp.0 + 4.0, imp.1));
    assert_eq!(menu_items(&st)[0], "Open uikit");
    // lib.greet, defined in lib.lua for the whole module: it opens there.
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    let call = last_text_at(&game, "lib");
    right_click(&mut st, &mut game, (call.0 + 26.0, call.1));
    choose(&mut st, &mut game, "Go to definition");
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert_eq!(st.s.editor.row, 1);
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("lib.greet"));
}

#[test]
fn a_file_gone_since_its_menu_opened_is_said_to_be_gone() {
    // kit/main.lua requires lib/util.lua and calls helper, defined there.
    // The player opens the menu of the require's string, or of helper, and
    // lib/util.lua goes meanwhile (an agent deletes it, or a file manager):
    // Open lib/util.lua and Go to definition leave main.lua open and say
    // why, rather than open an empty file in its name, which a save would
    // write back.
    let mut game = FakeGame::new("ui-menu-file-gone");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let kit = game.mods_dir().join("kit");
    std::fs::create_dir_all(kit.join("lib")).unwrap();
    std::fs::write(kit.join("main.lua"), "require(\"lib.util\")\nprint(helper())\n").unwrap();
    for (item, at) in [("Open lib/util.lua", "\"lib.util\""), ("Go to definition", "helper")] {
        std::fs::write(kit.join("lib/util.lua"), "function helper()\n  return 2\nend\n").unwrap();
        run_for(&mut st, &mut game, 0.6);
        st.open_file(&kit.join("main.lua"));
        handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
        redraw(&mut st, &mut game);
        let at = last_text_at(&game, at);
        right_click(&mut st, &mut game, (at.0 + 4.0, at.1));
        assert!(menu_items(&st).contains(&item.to_owned()), "{:?}", menu_items(&st));
        redraw(&mut st, &mut game);
        std::fs::remove_file(kit.join("lib/util.lua")).unwrap();
        run_for(&mut st, &mut game, 0.6);
        choose(&mut st, &mut game, item);
        assert_eq!(st.s.editor.title, "kit/main.lua", "{item}");
        assert_eq!(
            st.s.ui.notice.as_ref().map(|n| n.0.as_str()),
            Some("kit/lib/util.lua isn't there any more"),
            "{item}"
        );
    }
}

#[test]
fn go_to_definition_says_when_the_file_no_longer_defines_it() {
    // The player opens helper's menu in kit/main.lua, and lib/util.lua is
    // written meanwhile to define other instead: Go to definition opens
    // lib/util.lua all the same, at its top, and says helper isn't defined
    // there any more.
    let mut game = FakeGame::new("ui-definition-gone");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    let kit = game.mods_dir().join("kit");
    std::fs::create_dir_all(kit.join("lib")).unwrap();
    std::fs::write(kit.join("main.lua"), "require(\"lib.util\")\nprint(helper())\n").unwrap();
    std::fs::write(kit.join("lib/util.lua"), "function helper()\n  return 2\nend\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    st.open_file(&kit.join("main.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "helper");
    right_click(&mut st, &mut game, (at.0 + 4.0, at.1));
    assert!(menu_items(&st).contains(&"Go to definition".to_owned()));
    redraw(&mut st, &mut game);
    std::fs::write(kit.join("lib/util.lua"), "function other()\n  return 2\nend\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    choose(&mut st, &mut game, "Go to definition");
    let ed = &st.s.editor;
    assert_eq!(ed.title, "kit/lib/util.lua");
    assert_eq!(((ed.row, ed.col), ed.selection()), ((0, 0), None));
    assert_eq!(
        st.s.ui.notice.as_ref().map(|n| n.0.as_str()),
        Some("helper isn't defined in kit/lib/util.lua any more")
    );
}

#[test]
fn run_the_selection_runs_it_and_marks_its_line() {
    let src = "print('first')\nprint('second')\nerror('third')";
    let (mut game, mut st) = editing("ui-run-selection", src);
    st.s.editor.anchor = Some((1, 0));
    st.s.editor.row = 2;
    st.s.editor.col = st.s.editor.lines[2].len();
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "'second'");
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Run the selection");
    let lines = log(&st);
    assert!(
        lines.contains(&"second".to_owned()) && !lines.contains(&"first".to_owned()),
        "{lines:?}"
    );
    assert_eq!(st.s.editor.error_line, Some(3), "the line it failed at, in the file");
}

#[test]
fn the_code_menu_fits_a_small_screen_with_an_agent() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_theme::Look;
    let src = "local function greet(n)\n  return n\nend\nprint(greet(1), player.poss())";
    let (mut game, mut st) = editing("ui-code-menu-small", src);
    st.s.ui.look = Look {
        scale: 1.5,
        motion: false,
        ..Look::default()
    };
    st.s.ui.agent.agents = Some(vec![AgentSpec::claude(std::path::PathBuf::from("claude"))]);
    st.s.now = 10.0;
    redraw(&mut st, &mut game);
    // The last line selected, and a right click in it.
    st.s.editor.anchor = Some((3, 0));
    st.s.editor.row = 3;
    st.s.editor.col = st.s.editor.lines[3].len();
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "greet");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1));
    let items = usable(&st);
    for label in [
        "Go to definition",
        "Ask Claude Code...",
        "Explain the selection with Claude Code",
        "Fix with Claude Code",
        "Run the selection",
    ] {
        assert!(items.contains(&label.to_owned()), "{label}: {items:?}");
    }
    redraw(&mut st, &mut game);
    let rect = st.s.ui.menu.as_ref().and_then(|m| m.rect).unwrap();
    assert!(within(rect, [0.0, 0.0, 640.0, 360.0]), "{rect:?}");
    // The last item is reached with the keys.
    handle(&mut st, &mut game, key(Key::End, NONE));
    redraw(&mut st, &mut game);
    assert_eq!(lit(&st), "Save");
    assert!(drawn_text(&game, "Save"));
}

#[cfg(unix)]
#[test]
fn explain_this_line_asks_the_agent_with_the_line() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{AgentLineKind, Pane, agent_busy};
    let (mut game, mut st) = editing("ui-explain", "local a = 1\nprint(a)");
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }]);
    st.s.ui.dock.set_hidden(Pane::Agent, true);
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "print");
    right_click(&mut st, &mut game, at);
    choose(&mut st, &mut game, "Explain this line with Echo");
    assert_eq!(
        st.s.editor.selected_text().as_deref(),
        Some("print(a)"),
        "the line selected"
    );
    assert!(agent_busy(&st));
    assert!(
        st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.dock.shown(Pane::Agent),
        "its pane shows"
    );
    assert_eq!(st.s.ui.focus, Pane::Code, "the keys stay on the code");
    for _ in 0..200 {
        run_for(&mut st, &mut game, 0.05);
        if !agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let said: Vec<&str> =
        st.s.ui
            .agent
            .lines
            .iter()
            .filter(|l| l.kind == AgentLineKind::Said)
            .map(|l| l.text.as_str())
            .collect();
    assert!(
        said.iter().any(|l| l.contains("Explain what this code does.")),
        "{said:?}"
    );
    assert!(
        said.iter().any(|l| l.contains("print(a)")),
        "the selection went too: {said:?}"
    );
}

#[test]
fn the_menu_key_opens_the_menu_of_what_has_the_keys() {
    use horizon_studio::game::hstudio_ui::Pane;
    let (mut game, mut st) = editing("ui-menu-key", "local a = 1\nprint(a)");
    // In the code: at the cursor, which stays where it is, its first item lit.
    st.s.editor.click(1, 3, false);
    redraw(&mut st, &mut game);
    let caret = st.s.ui.caret_at;
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    let menu = st.s.ui.menu.clone().expect("the code's menu");
    assert_eq!(menu.at, caret.expect("the cursor was drawn"));
    assert_eq!(lit(&st), "Cut");
    assert_eq!((st.s.editor.row, st.s.editor.col), (1, 3));
    // It ends with where the code's pane is.
    assert_eq!(menu_items(&st)[menu_items(&st).len() - 2..], ["", "Layout"]);
    // The Menu key closes it again.
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert!(st.s.ui.menu.is_none());
    // The console's input, in the panel.
    handle(&mut st, &mut game, key(Key::Char('j'), CTRL));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(lit(&st), "Paste");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // The Library: the chosen mod's menu, at its row.
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "hello");
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(lit(&st), "Edit");
    assert!(st.s.ui.menu.as_ref().unwrap().target.is_some(), "its row outlined");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert_eq!(st.s.editor.title, "hello.lua");
    // The agent's input: its line's menu, Paste first.
    st.s.ui.dock.activate(Pane::Agent);
    st.s.ui.focus = Pane::Agent;
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(
        menu_items(&st),
        ["Paste", "Clear the line", "", "Send", "Stop", "", "Layout"]
    );
    assert_eq!(lit(&st), "Paste");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // The Problems panel: the chosen problem's menu, at its row (drawn the
    // frame after the code's check found it).
    st.s.editor.set_text("print(player.poss())");
    st.s.now = 10.0;
    st.s.ui.dock.activate(Pane::Problems);
    st.s.ui.focus = Pane::Problems;
    redraw(&mut st, &mut game);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.problems.len(), 1);
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(lit(&st), "Go to it");
    assert!(st.s.ui.menu.as_ref().unwrap().target.is_some(), "its row outlined");
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("poss"));
}

#[test]
fn the_menu_key_opens_a_menu_with_no_cursor_or_row_to_open_at() {
    use horizon_studio::game::hstudio_ui::{Pane, code_line};
    // The cursor scrolled out of the code's view: at the code's top left.
    let text: Vec<String> = (1..=300).map(|i| format!("local v{i} = {i}")).collect();
    let (mut game, mut st) = editing("ui-menu-key-off", &text.join("\n"));
    st.s.editor.click(0, 0, false);
    redraw(&mut st, &mut game);
    let code = text_at(&game, "v5");
    handle(&mut st, &mut game, Input::Mouse(code.0, code.1));
    handle(&mut st, &mut game, Input::Wheel(-20.0));
    redraw(&mut st, &mut game);
    assert_eq!(st.s.ui.caret_at, None, "off the view");
    let top = text_at(&game, "local");
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    let at = st.s.ui.menu.as_ref().expect("the code's menu").at;
    assert!((at[0] - top.0).abs() < 1.0, "at the code's left: {at:?} {top:?}");
    assert!((at[1] - top.1).abs() < code_line(), "at its top: {at:?} {top:?}");
    assert_eq!(lit(&st), "Cut");
    assert_eq!((st.s.editor.row, st.s.editor.col), (0, 0), "the cursor stays");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // The Problems pane listing none: its own menu, under its tab, with
    // where it is.
    st.s.editor.set_text("print(1)");
    st.s.now = 10.0;
    st.s.ui.dock.activate(Pane::Problems);
    st.s.ui.focus = Pane::Problems;
    redraw(&mut st, &mut game);
    assert!(st.s.editor.problems.is_empty());
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(usable(&st), ["Hide the panel", "Move", "Size...", "Panes"]);
    assert_eq!(lit(&st), "Hide the panel");
    let tab = last_text_at(&game, "Problems");
    assert!(st.s.ui.menu.as_ref().unwrap().at[1] > tab.1, "under its tab");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // The Library listing no mod: the list's own menu.
    for file in ["hello.lua", "uikit.lua", "viahorizon.lua"] {
        std::fs::remove_file(game.mods_dir().join(file)).unwrap();
    }
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.mods.is_empty());
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, key(Key::Menu, NONE));
    assert_eq!(menu_items(&st)[..2], ["New script...", "New module..."]);
    assert_eq!(lit(&st), "New script...");
    choose(&mut st, &mut game, "New script...");
    assert!(st.s.ui.naming.is_some());
}

#[test]
fn the_middle_button_reaches_script_screens_only() {
    let (mut game, mut st) = editing("ui-middle", "abc");
    let before = (st.s.editor.row, st.s.editor.col);
    handle(&mut st, &mut game, Input::Middle { down: true });
    handle(&mut st, &mut game, Input::Middle { down: false });
    assert!(st.s.ui.menu.is_none(), "it opens no menu");
    assert_eq!((st.s.editor.row, st.s.editor.col), before);
    // A script's screen gets it as "middle", in its click and release.
    st.run_console(
        &mut game,
        "pressed, released = nil, nil\nui.open({ click = function(x, y, b) pressed = b end, release = function(x, y, b) released = b end })",
    );
    assert!(st.accept_screen());
    horizon_studio::game::hstudio_ui::handle_screen(&mut st, &mut game, Input::Middle { down: true });
    horizon_studio::game::hstudio_ui::handle_screen(&mut st, &mut game, Input::Middle { down: false });
    let said: Vec<String> = st
        .run_console(&mut game, "pressed .. ' ' .. released")
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(said, ["\"middle middle\""]);
}

#[test]
fn the_footer_and_the_docs_say_how_menus_work() {
    use horizon_studio::game::hstudio_look::docs_markdown;
    let (mut game, mut st) = editing("ui-menu-footer", "print(1)");
    assert!(!drawn_text(&game, "do it"));
    let code = text_at(&game, "print");
    right_click(&mut st, &mut game, (code.0 + 2.0, code.1 + 4.0));
    redraw(&mut st, &mut game);
    assert!(
        drawn_text(&game, "choose") && drawn_text(&game, "do it"),
        "the menu's keys"
    );
    let docs = docs_markdown();
    assert!(docs.contains("Right-click a script or a module"), "Script mods");
    assert!(
        docs.contains("Right-click the title bar for the window, the theme and the panes."),
        "Looks"
    );
}

#[test]
fn a_drag_whose_release_went_elsewhere_is_over() {
    use horizon_studio::game::hstudio_theme::THEMES;
    use horizon_studio::game::hstudio_ui::handle_screen;
    let (mut game, mut st) = editing("ui-drag-gone", "local a = 1\nprint(a)");
    let code = text_at(&game, "print");
    let at = (code.0 + 2.0, code.1 + 4.0);
    let local = text_at(&game, "local");
    // A left press on the code starts selecting, and the studio closes
    // before the button is let go.
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Close);
    // Opened again: the pointer selects nothing, and a right click has its menu.
    st.s.now += 1.0;
    st.s.ui.opened_at = st.s.now;
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(local.0, local.1 + 4.0));
    assert_eq!(st.s.editor.selection(), None, "no drag goes on");
    right_click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_some(), "the code's menu");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // A script's screen takes the release: back from it, the drag is over too.
    handle(&mut st, &mut game, Input::Mouse(at.0, at.1));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    st.run_console(&mut game, "ui.open({ release = function() end })");
    assert!(st.accept_screen());
    handle_screen(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    st.close_screen(&mut game);
    // As the game does: the studio under it arrives again.
    st.s.now += 1.0;
    st.s.ui.opened_at = st.s.now;
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(local.0, local.1 + 4.0));
    assert_eq!(st.s.editor.selection(), None, "no drag goes on");
    right_click(&mut st, &mut game, at);
    assert!(st.s.ui.menu.is_some(), "the code's menu");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    // A window being moved when the studio closes is kept where it was moved.
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    redraw(&mut st, &mut game);
    let title = text_at(&game, "Horizon Studio");
    handle(&mut st, &mut game, Input::Mouse(title.0 + 100.0, title.1 + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    handle(&mut st, &mut game, Input::Mouse(title.0 + 80.0, title.1 + 24.0));
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Close);
    let kept = std::fs::read_to_string(st.s.dir.join("look.txt")).unwrap_or_default();
    assert!(kept.contains("rect="), "{kept}");
}

#[test]
fn go_to_definition_finds_a_name_indented_with_tabs_in_another_file() {
    let mut game = FakeGame::new("ui-definition-tabs");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.agent.agents = Some(Vec::new());
    let dir = game.mods_dir().join("kit");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.lua"), "print(counter)\n").unwrap();
    // Written elsewhere with a tab, which the editor shows as its indent.
    std::fs::write(dir.join("lib.lua"), "do\n\tcounter = 0\nend\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    st.open_file(&dir.join("main.lua"));
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    redraw(&mut st, &mut game);
    let at = last_text_at(&game, "counter");
    right_click(&mut st, &mut game, (at.0 + 2.0, at.1));
    choose(&mut st, &mut game, "Go to definition");
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert_eq!(st.s.editor.row, 1);
    assert_eq!(st.s.editor.selected_text().as_deref(), Some("counter"));
}

#[cfg(unix)]
#[test]
fn a_failed_mods_menu_copies_its_error_and_asks_the_agent_to_fix_it() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{AgentLineKind, Pane, agent_busy};
    let mut game = FakeGame::new("ui-mod-fix");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }]);
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    std::fs::write(
        dir.join("lib.lua"),
        "local lib = {}\nerror('boom in lib')\nreturn lib\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    let ModState::Failed(msg, trace) = st.s.mods[mod_index(&st, "kit")].state.clone() else {
        panic!("kit runs");
    };
    assert!(msg.starts_with("kit/lib.lua:2: boom in lib"), "{msg}");
    assert!(!trace.is_empty());
    handle(&mut st, &mut game, key(Key::Char('2'), CTRL));
    st.s.ui.dock.set_hidden(Pane::Agent, true);
    redraw(&mut st, &mut game);
    // Copy the error: its message, then its traceback.
    let row = text_at(&game, "kit/");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Copy the error");
    assert_eq!(game.clipboard, format!("{msg}\n{trace}"));
    // Fix with Echo: the file it failed in opens, the agent's panel shows,
    // and the agent is asked about it.
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    choose(&mut st, &mut game, "Fix with Echo");
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert!(agent_busy(&st));
    assert!(
        st.s.ui.dock.is_front(Pane::Agent) && st.s.ui.dock.shown(Pane::Agent),
        "its pane shows"
    );
    assert_eq!(st.s.ui.focus, Pane::Code, "the keys stay on the code");
    let asked: Vec<&str> =
        st.s.ui
            .agent
            .lines
            .iter()
            .filter(|l| l.kind == AgentLineKind::You)
            .map(|l| l.text.as_str())
            .collect();
    assert_eq!(asked, [format!("kit fails: {msg}. Find out why and fix it.")]);
    for _ in 0..200 {
        run_for(&mut st, &mut game, 0.05);
        if !agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!agent_busy(&st));
}

#[cfg(unix)]
#[test]
fn the_agents_line_sends_and_stops_from_its_menu() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use horizon_studio::game::hstudio_ui::{Pane, agent_busy};
    let (mut game, mut st) = editing("ui-agent-field", "print(1)");
    // An agent that works until it is stopped.
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Sleeper".into(),
        program: "/bin/sleep".into(),
        args: vec!["5".into()],
        json: false,
    }]);
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    let field = text_at(&game, "Ask Sleeper about this script, Enter sends.");
    let at = (field.0 + 2.0, field.1 + 4.0);
    right_click(&mut st, &mut game, at);
    assert_eq!(usable(&st), ["Paste"], "nothing typed to send");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    st.s.ui.agent_input.set("make it glow");
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, at);
    assert_eq!(usable(&st), ["Paste", "Clear the line", "Send"]);
    choose(&mut st, &mut game, "Send");
    assert!(agent_busy(&st), "it works on it");
    assert_eq!(st.s.ui.agent_input.text, "");
    // While it works: Stop, and nothing to send.
    redraw(&mut st, &mut game);
    right_click(&mut st, &mut game, at);
    assert_eq!(usable(&st), ["Paste", "Stop"]);
    choose(&mut st, &mut game, "Stop");
    run_for(&mut st, &mut game, 0.1);
    assert!(!agent_busy(&st));
    let last = st.s.ui.agent.lines.last().map(|l| l.text.clone());
    assert_eq!(last.as_deref(), Some("Stopped."));
}

#[test]
fn look_for_agents_again_finds_the_one_agent_txt_names() {
    use horizon_studio::game::hstudio_ui::Pane;
    // No agent found at first; then agent.txt names one.
    let (mut game, mut st) = editing("ui-agent-rescan", "");
    std::fs::write(st.s.dir.join("agent.txt"), "name: Echo\necho {prompt}\n").unwrap();
    st.s.ui.dock.activate(Pane::Agent);
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Agent");
    right_click(&mut st, &mut game, tab);
    choose(&mut st, &mut game, "Look for agents again");
    let found = st.s.ui.agent.agents.clone().unwrap_or_default();
    assert!(found.iter().any(|a| a.name == "Echo"), "{found:?}");
    let said = st.s.ui.agent.lines.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(said.starts_with("Found ") && said.contains("Echo"), "{said}");
    // The tab's menu lists it now.
    redraw(&mut st, &mut game);
    let tab = last_text_at(&game, "Agent");
    right_click(&mut st, &mut game, tab);
    assert!(menu_items(&st).contains(&"Echo".to_owned()), "{:?}", menu_items(&st));
}

#[test]
fn a_bundled_library_deleted_from_its_menu_stays_deleted() {
    let mut game = FakeGame::new("ui-delete-bundled");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    // UI Kit, with an unload handler that says when it runs.
    game.write_mod(
        "uikit.lua",
        "-- name: UI Kit\non('unload', function() print('uikit unloads') end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    redraw(&mut st, &mut game);
    let row = text_at(&game, "UI Kit");
    right_click(&mut st, &mut game, (row.0 + 2.0, row.1 + 2.0));
    assert_eq!(st.s.mods[st.s.ui.library_sel].id, "uikit");
    choose(&mut st, &mut game, "Delete...");
    redraw(&mut st, &mut game);
    assert!(drawn_contains(&game, "Delete uikit.lua?"));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let uikit = game.mods_dir().join("uikit.lua");
    assert!(log(&st).contains(&"[uikit] uikit unloads".to_owned()), "{:?}", log(&st));
    assert!(!uikit.exists() && st.s.dir.join("trash").join("uikit.lua").is_file());
    assert!(!st.s.mods.iter().any(|m| m.id == "uikit"));
    // The next start doesn't bring it back.
    let again = studio(&mut game);
    assert!(!uikit.exists() && !again.s.mods.iter().any(|m| m.id == "uikit"));
}

#[test]
fn the_library_shows_each_commands_usage() {
    let mut game = FakeGame::new("ui-library-usage");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.look.motion = false;
    game.write_mod(
        "wp.lua",
        "command('wp', function() end, { description = 'Waypoints', args = {\n  { name = 'action', optional = true },\n  { name = 'name', kind = 'text', optional = true },\n} })\ncommand('here', function() end, 'Marks this place')\n",
    );
    run_for(&mut st, &mut game, 0.6);
    handle(&mut st, &mut game, key(Key::Char('3'), CTRL));
    st.s.ui.library_sel = mod_index(&st, "wp");
    redraw(&mut st, &mut game);
    let (x, y) = text_at(&game, "/wp [<action>] [<name>]");
    let (dx, dy) = text_at(&game, "Waypoints");
    assert!(dx > x && (dy - y).abs() < 0.5, "the description follows on its row");
    // A command without declared arguments shows its name alone.
    text_at(&game, "/here");
    text_at(&game, "Marks this place");
}
