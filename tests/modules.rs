// Mods built on mods: export and import, load order, reloading users with
// their libraries, events between mods, JSON, gfx.* and canvas screens,
// and the bundled UI Kit.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio, static_imports};
use horizon_studio::game::hstudio_examples::EXAMPLES;
use horizon_studio::game::hstudio_ui::{Input, draw_screen, handle_screen};

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

fn failure(st: &Studio, id: &str) -> String {
    match &st.s.mods[mod_index(st, id)].state {
        ModState::Failed(msg, _) => msg.clone(),
        other => panic!("{id} is {other:?}, not failed"),
    }
}

fn running(st: &Studio, id: &str) -> bool {
    st.s.mods[mod_index(st, id)].state == ModState::Running
}

#[test]
fn a_mod_imports_what_another_exports() {
    let mut game = FakeGame::new("modules-basic");
    let mut st = studio(&mut game);
    // "app" sorts before "zlib", yet loads after it.
    game.write_mod(
        "app.lua",
        "local lib = import('zlib')\ncommand('greet', function(a) return lib.greet(a) end)\n",
    );
    game.write_mod("zlib.lua", "export({ greet = function(n) return 'hi ' .. n end })\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(running(&st, "app"), "{:?}", log(&st));
    assert!(st.s.mods[mod_index(&st, "zlib")].exports.is_some());
    assert_eq!(st.s.mods[mod_index(&st, "app")].imports, vec!["zlib".to_owned()]);
    assert!(st.command("greet bob"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("hi bob"));
    // A main file that returns a table exports it; the console can import too.
    game.write_mod(
        "mathx.lua",
        "local M = {}\nfunction M.double(x) return x * 2 end\nreturn M\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(console(&mut st, &mut game, "import('mathx').double(21)"), vec!["42"]);
}

#[test]
fn users_reload_with_their_library() {
    let mut game = FakeGame::new("modules-reload");
    let mut st = studio(&mut game);
    game.write_mod("lib.lua", "export({ v = 'one' })\n");
    game.write_mod("user.lua", "print(import('lib').v)\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(log(&st).contains(&"[user] one".to_owned()));
    game.write_mod("lib.lua", "export({ v = 'two!' })\n");
    run_for(&mut st, &mut game, 0.6);
    let l = log(&st);
    assert!(l.contains(&"[user] two!".to_owned()), "{l:?}");
    assert!(l.contains(&"Reloaded user (it uses lib)".to_owned()), "{l:?}");
    // A broken library stops its users, with a reason; fixed, they return.
    game.write_mod("lib.lua", "export({ v = 'three' }) error('lib is broken')\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(failure(&st, "lib").contains("lib is broken"));
    let msg = failure(&st, "user");
    assert!(msg.contains("library 'lib' failed"), "{msg}");
    game.write_mod("lib.lua", "export({ v = 'fixed' })\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(running(&st, "user"));
    assert!(log(&st).contains(&"[user] fixed".to_owned()));
    // Turned off, its users stop; turned on, they come back.
    let i = mod_index(&st, "lib");
    st.set_enabled(&mut game, i, false);
    assert!(failure(&st, "user").contains("turned off"));
    st.set_enabled(&mut game, i, true);
    assert!(running(&st, "user"));
    // Removed, likewise.
    std::fs::remove_file(game.mods_dir().join("lib.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(failure(&st, "user").contains("no mod 'lib'"));
}

#[test]
fn a_library_installed_later_starts_its_users() {
    let mut game = FakeGame::new("modules-later");
    let mut st = studio(&mut game);
    game.write_mod("early.lua", "print('got ' .. import('later').n)\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(failure(&st, "early").contains("no mod 'later' to import"));
    game.write_mod("later.lua", "return { n = 7 }\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(running(&st, "early"), "{:?}", log(&st));
    assert!(log(&st).contains(&"[early] got 7".to_owned()));
}

#[test]
fn import_mistakes_say_what_is_wrong() {
    let mut game = FakeGame::new("modules-errors");
    let mut st = studio(&mut game);
    game.write_mod("selfish.lua", "import('selfish')\n");
    game.write_mod("empty.lua", "print('nothing to offer')\n");
    game.write_mod("wants_empty.lua", "import('empty')\n");
    // Each imports the other: they stop instead of looping.
    game.write_mod("ping.lua", "local pong = import('pong')\nexport({})\n");
    game.write_mod("pong.lua", "local ping = import('ping')\nexport({})\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(failure(&st, "selfish").contains("cannot import itself"));
    assert!(failure(&st, "wants_empty").contains("'empty' exports nothing"));
    let stuck = ["ping", "pong"]
        .iter()
        .filter(|id| !running(&st, id))
        .map(|id| failure(&st, id))
        .collect::<Vec<_>>();
    assert!(
        !stuck.is_empty() && stuck.iter().any(|m| m.contains("isn't loaded yet")),
        "{stuck:?}"
    );
    let out = console(&mut st, &mut game, "import('nope')");
    assert!(out[0].contains("no mod 'nope'"), "{out:?}");
}

#[test]
fn static_imports_finds_import_calls() {
    let src = "local a = import('x')\nlocal b = import \"y-z\"\nlocal c = import ( 'x' )\nmy.import('no')\nreimport('no')\n-- import(nope)\n";
    assert_eq!(static_imports(src), vec!["x".to_owned(), "y-z".to_owned()]);
}

#[test]
fn mods_send_each_other_events() {
    let mut game = FakeGame::new("modules-events");
    let mut st = studio(&mut game);
    game.write_mod(
        "listener.lua",
        "on('bus:ping', function(n, s) print('got ' .. n .. s) end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    console(&mut st, &mut game, "emit('bus:ping', 3, 'x')");
    assert!(
        !log(&st).contains(&"[listener] got 3x".to_owned()),
        "next frame, not now"
    );
    st.frame(&mut game, 0.016);
    assert!(log(&st).contains(&"[listener] got 3x".to_owned()), "{:?}", log(&st));
    let out = console(&mut st, &mut game, "emit('nocolon')");
    assert!(out[0].contains("':'"), "{out:?}");
    let out = console(&mut st, &mut game, "on('bogus', print)");
    assert!(out[0].contains("unknown event 'bogus'"), "{out:?}");
}

#[test]
fn json_round_trips() {
    let mut game = FakeGame::new("modules-json");
    let mut st = studio(&mut game);
    assert_eq!(
        console(&mut st, &mut game, "json.encode({1, 2, 3})"),
        vec!["\"[1,2,3]\""]
    );
    assert_eq!(
        console(&mut st, &mut game, "json.encode({a = 1.5})"),
        vec!["\"{\\\"a\\\":1.5}\""]
    );
    assert_eq!(
        console(
            &mut st,
            &mut game,
            "json.decode('{\"x\": [1, 2, {\"y\": true}]}').x[3].y"
        ),
        vec!["true"]
    );
    assert_eq!(console(&mut st, &mut game, "json.decode('[nope') == nil"), vec!["true"]);
    let out = console(&mut st, &mut game, "json.encode({f = print})");
    assert!(out[0].contains("no JSON form"), "{out:?}");
}

#[test]
fn gfx_draws_in_dp() {
    let mut game = FakeGame::new("modules-gfx");
    let mut st = studio(&mut game);
    game.write_mod(
        "painter.lua",
        "on('hud', function()\n  gfx.rect(10, 10, 100, 40, '#ff0000', 8)\n  gfx.border(10, 10, 100, 40, nil, 99)\n  gfx.text(20, 20, 'Hi', nil, 16, 'medium')\nend)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    game.drawn.clear();
    st.hud(&mut game);
    let d = &game.drawn;
    assert!(
        d.contains(&"rect [10.0, 10.0, 100.0, 40.0] r8 ffff0000".to_owned()),
        "{d:?}"
    );
    assert!(
        d.contains(&"border [10.0, 10.0, 100.0, 40.0] r20 ffffffff".to_owned()),
        "radius at most half: {d:?}"
    );
    assert!(d.contains(&"text Hi @[20.0, 20.0] ffffff".to_owned()), "{d:?}");
    assert_eq!(
        console(&mut st, &mut game, "gfx.measure('abc', 20, 'mono')"),
        vec!["18"]
    );
    assert_eq!(console(&mut st, &mut game, "gfx.size()"), vec!["960, 540"]);
    assert_eq!(console(&mut st, &mut game, "gfx.mouse()"), vec!["nil"]);
    let out = console(&mut st, &mut game, "gfx.rect(0, 0, 1, 1)");
    assert!(out[0].contains("draws only inside"), "{out:?}");
    let out = console(&mut st, &mut game, "gfx.measure('a', 12, 'comic')");
    assert!(out[0].contains("\"mono\" expected"), "{out:?}");
}

#[test]
fn a_canvas_screen_gets_dp_and_pointer_events() {
    let mut game = FakeGame::new("modules-canvas");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "size, moved, pressed, released = nil, nil, nil, nil\nui.open({ canvas = true, backdrop = false, draw = function(w, h) size = w .. 'x' .. h gfx.rect(1, 2, 3, 4) end, move = function(x, y) moved = x .. ',' .. y end, click = function(x, y, b) pressed = x .. ',' .. y .. ',' .. b end, release = function(x, y, b) released = b end })",
    );
    assert!(st.accept_screen());
    game.drawn.clear();
    draw_screen(&mut st, &mut game);
    assert_eq!(console(&mut st, &mut game, "size"), vec!["\"960x540\""]);
    assert_eq!(
        game.drawn,
        vec!["rect [1.0, 2.0, 3.0, 4.0] r0 ffffffff".to_owned()],
        "no backdrop, no panel"
    );
    // Two dp to a GUI pixel: the pointer arrives in GUI pixels, the canvas gets dp.
    st.s.ui.dp = 2.0;
    handle_screen(&mut st, &mut game, Input::Mouse(100.0, 50.0));
    assert_eq!(console(&mut st, &mut game, "moved"), vec!["\"200,100\""]);
    handle_screen(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(console(&mut st, &mut game, "pressed"), vec!["\"200,100,left\""]);
    assert_eq!(
        console(&mut st, &mut game, "gfx.mouse()"),
        vec!["200, 100, true, false"]
    );
    handle_screen(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(console(&mut st, &mut game, "released"), vec!["\"left\""]);
    assert_eq!(console(&mut st, &mut game, "(select(3, gfx.mouse()))"), vec!["false"]);
}

/// The position drawn text starts with `label` was drawn at, by the canvas.
fn text_at(game: &FakeGame, label: &str) -> Option<(f32, f32)> {
    let line = game.drawn.iter().find(|d| d.starts_with(&format!("text {label} @")))?;
    let at = line
        .split('@')
        .nth(1)?
        .trim_start_matches('[')
        .split(']')
        .next()?
        .to_owned();
    let xy: Vec<f32> = at.split(", ").map(|v| v.parse().unwrap()).collect();
    Some((xy[0], xy[1]))
}

/// A few frames of the canvas, with time passing.
fn frames(st: &mut Studio, game: &mut FakeGame, n: usize) {
    for _ in 0..n {
        st.frame(game, 0.05);
        game.drawn.clear();
        draw_screen(st, game);
    }
}

fn click(st: &mut Studio, game: &mut FakeGame, label: &str) {
    frames(st, game, 1);
    let (x, y) = text_at(game, label).unwrap_or_else(|| panic!("{label} not drawn: {:?}", game.drawn));
    let k = st.s.ui.dp_factor();
    handle_screen(st, game, Input::Mouse((x + 2.0) / k, (y + 4.0) / k));
    handle_screen(st, game, Input::Button { left: true, down: true });
    handle_screen(
        st,
        game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    frames(st, game, 1);
}

#[test]
fn the_ui_kit_comes_installed_once() {
    let mut game = FakeGame::new("modules-kit-once");
    let st = studio(&mut game);
    assert!(game.mods_dir().join("uikit.lua").is_file());
    assert!(running(&st, "uikit"), "{:?}", log(&st));
    assert!(st.s.mods[mod_index(&st, "uikit")].exports.is_some());
    // Deleted, it stays deleted.
    std::fs::remove_file(game.mods_dir().join("uikit.lua")).unwrap();
    let st = studio(&mut game);
    assert!(!game.mods_dir().join("uikit.lua").exists());
    assert!(st.s.mods.iter().all(|m| m.id != "uikit"));
}

#[test]
fn the_ui_kit_builds_a_working_window() {
    let mut game = FakeGame::new("modules-kit");
    let mut st = studio(&mut game);
    let i = EXAMPLES.iter().position(|e| e.id == "uikit_demo").unwrap();
    st.install_example(&mut game, i).unwrap();
    run_for(&mut st, &mut game, 0.1);
    assert!(running(&st, "uikit_demo"), "{:?}", log(&st));
    assert!(st.command("kitdemo"));
    st.frame(&mut game, 0.016);
    assert!(st.accept_screen());
    frames(&mut st, &mut game, 8);
    for t in [
        "UI Kit demo",
        "HUD card",
        "Finder",
        "Show the card",
        "Text size",
        "13",
        "Coordinates",
        "Save",
    ] {
        assert!(text_at(&game, t).is_some(), "{t} not drawn: {:?}", game.drawn);
    }
    // The card shows once its toggle is on.
    game.drawn.clear();
    st.hud(&mut game);
    assert!(text_at(&game, "COORDINATES").is_none());
    click(&mut st, &mut game, "Show the card");
    game.drawn.clear();
    st.hud(&mut game);
    assert!(
        text_at(&game, "COORDINATES").is_some(),
        "{:?} {:?}",
        game.drawn,
        log(&st)
    );
    // Typing into the title field.
    click(&mut st, &mut game, "Coordinates");
    handle_screen(&mut st, &mut game, Input::Text("Base".into()));
    frames(&mut st, &mut game, 1);
    assert!(text_at(&game, "Base").is_some(), "{:?}", game.drawn);
    // Another tab, then Save closes it after its exit animation.
    click(&mut st, &mut game, "Finder");
    frames(&mut st, &mut game, 6);
    assert!(text_at(&game, "Diamond ore").is_some(), "{:?}", game.drawn);
    click(&mut st, &mut game, "Save");
    frames(&mut st, &mut game, 6);
    assert!(st.s.screen.is_none());
    assert!(running(&st, "uikit_demo"), "{:?}", log(&st));
    game.drawn.clear();
    st.hud(&mut game);
    assert!(text_at(&game, "BASE").is_some(), "{:?}", game.drawn);
}

#[test]
fn editing_the_kit_reloads_its_users() {
    let mut game = FakeGame::new("modules-kit-edit");
    let mut st = studio(&mut game);
    let i = EXAMPLES.iter().position(|e| e.id == "uikit_demo").unwrap();
    st.install_example(&mut game, i).unwrap();
    run_for(&mut st, &mut game, 0.1);
    let path = game.mods_dir().join("uikit.lua");
    let src = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        src.replace("accent = rgb(\"#9a86ff\")", "accent = rgb(\"#ff8800\")"),
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(
        log(&st).contains(&"Reloaded uikit_demo (it uses uikit)".to_owned()),
        "{:?}",
        log(&st)
    );
    assert!(running(&st, "uikit_demo"));
    assert_eq!(
        st.s.mods[mod_index(&st, "uikit_demo")].reloads,
        0,
        "reloaded as a user, not as changed"
    );
    let lib = st.s.mods[mod_index(&st, "uikit")].reloads;
    assert_eq!(lib, 1);
}
