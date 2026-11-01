// AI agents in the player's editor (game::hstudio_coach): the editor's
// splices that keep the player's place, what the player does there, which
// file a request means, reading and opening, and edits typed in as the
// player watches (and everything that can come while they are).

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_coach::{self as coach, By, Phase};
use horizon_studio::game::hstudio_ui::{AgentLineKind, Editor, Input, Key, Mods, Tab, draw, handle};
use serde_json::{Value as Json, json};

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

fn redraw(st: &mut Studio, game: &mut FakeGame) {
    game.drawn.clear();
    draw(st, game);
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

/// The studio open on the Editor tab with the script `id`.lua (`src`) open
/// in it, drawn once (so the view's rows are known).
fn editing(name: &str, id: &str, src: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    game.write_mod(&format!("{id}.lua"), src);
    st.scan(&mut game);
    let i = mod_index(&st, id);
    st.edit_mod(i);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    redraw(&mut st, &mut game);
    (game, st)
}

/// An edit's result, once it is typed in (a few seconds at most).
fn typed_in(st: &mut Studio, game: &mut FakeGame, id: u64) -> Json {
    for _ in 0..100 {
        let t = ask(st, game, "typed", json!({ "id": id })).unwrap();
        if t["done"] == true {
            return t["result"].clone();
        }
        run_for(st, game, 0.05);
    }
    panic!("edit {id} never was in");
}

/// The Agent panel's lines of what agents did in the editor.
fn actions(st: &Studio) -> Vec<String> {
    st.s.ui
        .agent
        .lines
        .iter()
        .filter(|l| l.kind == AgentLineKind::Action)
        .map(|l| l.text.clone())
        .collect()
}

const GREET: &str = "local n = 0\nprint('hi')\nreturn n\n";

fn key(k: Key, mods: Mods) -> Input {
    Input::Key {
        key: k,
        down: true,
        repeat: false,
        mods,
    }
}

/// An agent outside the game, as the link names it.
fn agent() -> By {
    By {
        name: "Claude Code".into(),
        conn: 1,
    }
}

/// One of the link's editor methods, as an agent's request.
fn ask(st: &mut Studio, game: &mut FakeGame, method: &str, params: Json) -> Result<Json, String> {
    coach::call(st, game, &agent(), method, &params)
}

/// A studio with a module, kit/ (main.lua and lib/util.lua), besides hello.lua.
fn with_module(name: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    std::fs::create_dir_all(game.mods_dir().join("kit/lib")).unwrap();
    std::fs::write(
        game.mods_dir().join("kit/main.lua"),
        "local util = require('lib.util')\nprint(util.two())\n",
    )
    .unwrap();
    std::fs::write(
        game.mods_dir().join("kit/lib/util.lua"),
        "return { two = function() return 2 end }\n",
    )
    .unwrap();
    st.scan(&mut game);
    (game, st)
}

fn editor(text: &str) -> Editor {
    let mut e = Editor::default();
    e.set_text(text);
    e
}

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_splice_keeps_the_cursor_and_the_selection_in_their_place() {
    let mut e = editor(&numbered(200));
    // The player's caret on line 40, a selection back to line 39.
    e.click(39, 3, false);
    e.anchor = Some((38, 0));
    // Lines put in far below: nothing moves.
    let at = (149, 0);
    let end = e.splice(at, at, "new a\nnew b\nnew c\n");
    assert_eq!(end, (152, 0));
    assert_eq!((e.row, e.col, e.anchor), (39, 3, Some((38, 0))));
    assert_eq!(e.lines[149], "new a");
    assert_eq!(e.lines[152], "line 150");
    // Lines put in above: the caret and the anchor go down with their text.
    e.splice((9, 0), (9, 0), "x\ny\nz\n");
    assert_eq!((e.row, e.col, e.anchor), (42, 3, Some((41, 0))));
    assert_eq!(&e.lines[42][..3], "lin", "the same text under it");
    assert_eq!(e.lines[42], "line 40");
    // On its own line, after the change: it moves across with its text.
    e.click(0, 5, false);
    e.splice((0, 0), (0, 4), "row");
    assert_eq!((e.row, e.col), (0, 4));
    assert_eq!(e.lines[0], "row 1");
    // Inside what is replaced: to its start.
    e.click(1, 3, false);
    e.splice((1, 1), (2, 2), "-");
    assert_eq!((e.row, e.col), (1, 1));
    assert_eq!(e.lines[1], "l-ne 3");
    // The view stays where it is: the cursor moving with the text isn't a reason to scroll.
    assert_eq!(e.followed, Some((e.row, e.col, e.revision)));
    assert!(e.dirty);
}

#[test]
fn a_splice_stays_inside_the_buffer_and_offsets_go_both_ways() {
    let mut e = editor("héllo\nworld");
    assert_eq!(e.offset_of((1, 2)), 9);
    assert_eq!(e.pos_at(9), (1, 2));
    assert_eq!(e.pos_at(2), (0, 1), "inside the é: its start");
    assert_eq!(e.pos_at(1000), (1, 5), "past the end: the end");
    assert_eq!(e.offset_of((7, 99)), e.text().len());
    // Places past the end count as the end, and a range given backwards is turned round.
    let end = e.splice((9, 9), (9, 9), "!");
    assert_eq!(e.text(), "héllo\nworld!");
    assert_eq!(end, (1, 6));
    e.splice((1, 5), (0, 2), "");
    assert_eq!(e.text(), "h!");
    assert_eq!(Editor::normal("a\r\n\tb"), "a\n  b");
}

#[test]
fn an_agents_change_undoes_in_one_step() {
    let mut e = editor("local a = 1\nprint(a)");
    e.click(1, 2, false);
    // A change typed in over many frames: one undo step.
    e.agent_begin();
    let mut at = (0, 11);
    for piece in ["\n", "local b", " = ", "2"] {
        at = e.splice(at, at, piece);
    }
    assert_eq!(e.text(), "local a = 1\nlocal b = 2\nprint(a)");
    assert_eq!((e.row, e.col), (2, 2), "the caret went down with its line");
    // The player's typing after it is a step of its own.
    e.insert("x");
    e.undo();
    assert_eq!(e.text(), "local a = 1\nlocal b = 2\nprint(a)");
    e.undo();
    assert_eq!(e.text(), "local a = 1\nprint(a)");
    assert_eq!((e.row, e.col), (1, 2), "the caret where it was");
    e.redo();
    assert_eq!(e.text(), "local a = 1\nlocal b = 2\nprint(a)");
}

#[test]
fn opening_a_file_keeps_the_revision_rising() {
    let mut game = FakeGame::new("coach-revision");
    let mut st = studio(&mut game);
    let i = mod_index(&st, "hello");
    let revision = st.s.editor.revision;
    st.edit_mod(i);
    let opened = st.s.editor.revision;
    assert!(opened > revision);
    // Read again from the same file: still on.
    let path = st.s.editor.path.clone().unwrap();
    st.s.editor.open(&path, "hello.lua");
    assert!(st.s.editor.revision > opened);
}

#[test]
fn the_player_editing_is_told_from_the_player_opening_files() {
    let mut game = FakeGame::new("coach-player");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.now = 10.0;
    assert!(!st.s.ui.coach.player_editing(st.s.now), "nobody has edited yet");
    assert!(!st.s.ui.coach.player_busy(st.s.now));
    st.s.ui.tab = Tab::Editor;
    handle(&mut st, &mut game, Input::Text("x".into()));
    assert_eq!(st.s.ui.coach.player_at, 10.0);
    assert!(st.s.ui.coach.player_editing(11.9));
    assert!(!st.s.ui.coach.player_editing(12.1));
    // Moving the cursor uses the studio; it doesn't change the code.
    st.s.now = 20.0;
    handle(&mut st, &mut game, key(Key::Left, NONE));
    assert_eq!(st.s.ui.coach.input_at, 20.0);
    assert_eq!(st.s.ui.coach.player_at, 10.0);
    // Another file opened (the console's edit()) changes the buffer, but not as an edit.
    st.s.now = 30.0;
    st.s.ui.tab = Tab::Console;
    for c in "edit('hello')".chars() {
        handle(&mut st, &mut game, Input::Text(c.to_string()));
    }
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    assert!(st.s.editor.title.starts_with("hello"), "{}", st.s.editor.title);
    assert_eq!(st.s.ui.coach.player_at, 10.0);
}

#[test]
fn targets_stay_inside_the_mods_folder() {
    let (_game, st) = with_module("coach-targets");
    let t = |params: Json, create: bool| coach::target(&st, &params, create);
    // A module's files by their paths in it, main.lua by default.
    let main = t(json!({ "id": "kit" }), false).unwrap();
    assert_eq!(main.file, "kit/main.lua");
    assert_eq!(main.path, st.s.mods_dir().join("kit").join("main.lua"));
    let util = t(json!({ "id": "kit", "file": "lib/util.lua" }), false).unwrap();
    assert_eq!((util.file.as_str(), util.exists), ("kit/lib/util.lua", true));
    assert_eq!(t(json!({ "id": "kit", "file": "lib\\util" }), false).unwrap(), util);
    // Nothing outside it, nothing but .lua files, and only mod ids.
    for (file, why) in [
        ("../x.lua", "outside"),
        ("/etc/passwd", "starts with a name"),
        ("a/b/c/d/e.lua", "three folders deep"),
        ("notes.txt", "isn't a .lua file"),
    ] {
        for create in [false, true] {
            let e = t(json!({ "id": "kit", "file": file }), create).unwrap_err();
            assert!(e.contains(why), "{file}: {e}");
        }
    }
    let e = t(json!({ "id": "Bad Id" }), true).unwrap_err();
    assert!(e.contains("not a mod id"), "{e}");
    let e = t(json!({ "id": "../kit" }), true).unwrap_err();
    assert!(e.contains("not a mod id"), "{e}");
    // A file that isn't there: its module's files are listed, and create names it.
    let e = t(json!({ "id": "kit", "file": "nope.lua" }), false).unwrap_err();
    assert!(e.contains("main.lua, lib/util.lua") && e.contains("create"), "{e}");
    let new = t(json!({ "id": "kit", "file": "lib/more" }), true).unwrap();
    assert_eq!((new.file.as_str(), new.exists), ("kit/lib/more.lua", false));
    // A script is one file.
    let hello = t(json!({ "id": "hello" }), false).unwrap();
    assert_eq!(hello.file, "hello.lua");
    assert_eq!(t(json!({ "id": "hello", "file": "hello.lua" }), false).unwrap(), hello);
    let e = t(json!({ "id": "hello", "file": "lib.lua" }), true).unwrap_err();
    assert!(e.contains("is a script"), "{e}");
    // A mod that isn't there: named only with create, as a script or a module.
    let e = t(json!({ "id": "glow" }), false).unwrap_err();
    assert!(e.contains("no script mod 'glow'") && e.contains("hello"), "{e}");
    let glow = t(json!({ "id": "glow" }), true).unwrap();
    assert_eq!((glow.file.as_str(), glow.exists), ("glow.lua", false));
    assert_eq!(glow.path, st.s.mods_dir().join("glow.lua"));
    let lamp = t(json!({ "id": "lamp", "file": "main.lua" }), true).unwrap();
    assert_eq!(lamp.file, "lamp/main.lua");
    let e = t(json!({ "id": "hello", "file": "x.lua", "scratch": true }), false).unwrap_err();
    assert!(e.contains("without id or file"), "{e}");
    // The scratch file, the file open, and a file of the mod open.
    let scratch = t(json!({ "scratch": true }), false).unwrap();
    assert_eq!(
        (scratch.file.as_str(), &scratch.path),
        ("scratch.lua", &st.scratch_path())
    );
    assert_eq!(t(json!({}), false).unwrap(), scratch, "the scratch file is open");
    let e = t(json!({ "file": "lib.lua" }), false).unwrap_err();
    assert!(e.contains("give id"), "{e}");
}

#[test]
fn a_file_of_the_mod_open_is_named_by_its_path_alone() {
    let (mut game, mut st) = with_module("coach-target-open");
    let path = st.s.mods_dir().join("kit").join("main.lua");
    st.open_file(&path);
    let util = coach::target(&st, &json!({ "file": "lib/util.lua" }), false).unwrap();
    assert_eq!(util.file, "kit/lib/util.lua");
    // A new file opened with create is found while the editor has it open, not saved yet.
    st.s.ui.open = true;
    let r = ask(&mut st, &mut game, "open", json!({ "id": "glow", "create": true })).unwrap();
    assert_eq!(r["created"], true, "{r}");
    assert_eq!(st.s.editor.title, "glow.lua");
    assert_eq!(st.s.editor.text(), "");
    assert!(!st.s.mods_dir().join("glow.lua").exists(), "written only when saved");
    let glow = coach::target(&st, &json!({ "id": "glow" }), false).unwrap();
    assert!(glow.exists);
}

#[test]
fn reading_gives_the_text_as_the_player_has_it_a_page_at_a_time() {
    let (mut game, mut st) = with_module("coach-read");
    let i = mod_index(&st, "hello");
    st.edit_mod(i);
    // An unsaved edit of the player's, and their selection.
    st.s.editor.click(0, 0, false);
    st.s.editor.insert("-- mine\n");
    st.s.editor.click(0, 3, false);
    st.s.editor.move_h(true, true, true);
    let r = ask(&mut st, &mut game, "editor", json!({})).unwrap();
    assert_eq!(r["file"], "hello.lua");
    assert_eq!((&r["open"], &r["dirty"]), (&json!(true), &json!(true)));
    assert_eq!(r["lines"][0], "-- mine");
    assert_eq!(r["cursor"], json!({ "line": 1, "column": 8 }));
    assert_eq!(r["selection"]["text"], "mine");
    assert_eq!(r["selection"]["from"], json!({ "line": 1, "column": 4 }));
    let count = r["line_count"].as_u64().unwrap() as usize;
    assert_eq!(
        r["lines"].as_array().unwrap().len(),
        count,
        "the empty line after the last line break isn't one"
    );
    assert!(r["did"].as_str().unwrap().starts_with("Read hello.lua"), "{r}");
    // A file the editor hasn't open: as on disk.
    let r = ask(
        &mut st,
        &mut game,
        "editor",
        json!({ "id": "kit", "file": "lib/util.lua" }),
    )
    .unwrap();
    assert_eq!((&r["open"], &r["dirty"]), (&json!(false), &json!(false)));
    assert_eq!(r["lines"], json!(["return { two = function() return 2 end }"]));
    assert!(r.get("cursor").is_none());
    // A long file comes a page at a time.
    let long: Vec<String> = (1..=1000).map(|n| format!("print({n})")).collect();
    std::fs::write(st.s.mods_dir().join("long.lua"), long.join("\n") + "\n").unwrap();
    st.scan(&mut game);
    let r = ask(&mut st, &mut game, "editor", json!({ "id": "long" })).unwrap();
    assert_eq!(
        (&r["from"], &r["to"], &r["more"]),
        (&json!(1), &json!(400), &json!(true))
    );
    assert_eq!(r["line_count"], 1000);
    assert_eq!(r["lines"].as_array().unwrap().len(), 400);
    let r = ask(
        &mut st,
        &mut game,
        "editor",
        json!({ "id": "long", "from": 991, "to": 2000 }),
    )
    .unwrap();
    assert_eq!(r["lines"][0], "print(991)");
    assert_eq!((&r["to"], &r["more"]), (&json!(1000), &json!(false)));
    assert_eq!(r["did"], "Read long.lua, lines 991-1000 of 1000");
    let r = ask(&mut st, &mut game, "editor", json!({ "id": "long", "from": 5000 })).unwrap();
    assert_eq!(r["lines"], json!([]));
    let e = ask(&mut st, &mut game, "editor", json!({ "id": "long", "from": 0 })).unwrap_err();
    assert!(e.contains("counted from 1"), "{e}");
}

#[test]
fn opening_waits_for_the_player_and_never_puts_their_edits_aside() {
    let (mut game, mut st) = with_module("coach-open");
    st.s.ui.open = true;
    st.s.now = 10.0;
    st.s.ui.tab = Tab::Console;
    // The player just pressed a key: it waits.
    handle(&mut st, &mut game, key(Key::Left, NONE));
    let r = ask(
        &mut st,
        &mut game,
        "open",
        json!({ "id": "kit", "file": "lib/util.lua" }),
    )
    .unwrap();
    assert_eq!((&r["ok"], &r["busy"]), (&json!(false), &json!(true)), "{r}");
    assert_eq!(st.s.editor.title, "scratch.lua");
    assert_eq!(st.s.ui.tab, Tab::Console);
    // A moment later it opens, the Editor tab forward, at the line asked.
    st.s.now = 12.5;
    let r = ask(
        &mut st,
        &mut game,
        "open",
        json!({ "id": "kit", "file": "lib/util.lua", "line": 1 }),
    )
    .unwrap();
    assert_eq!(r["did"], "Opened kit/lib/util.lua at line 1", "{r}");
    assert_eq!(st.s.editor.title, "kit/lib/util.lua");
    assert_eq!(st.s.ui.tab, Tab::Editor);
    assert!(st.s.ui.expanded.contains("kit"), "the script list shows the file");
    // The player's unsaved edits stay where they are: nothing else opens.
    st.s.editor.insert("-- mine");
    let e = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap_err();
    assert!(e.contains("unsaved edits in kit/lib/util.lua"), "{e}");
    assert_eq!(st.s.editor.title, "kit/lib/util.lua");
    assert!(st.s.editor.dirty);
    // The file open is open already, whatever its edits; line moves the view, not their cursor.
    let (row, col) = (st.s.editor.row, st.s.editor.col);
    let r = ask(&mut st, &mut game, "open", json!({ "file": "lib/util.lua", "line": 1 })).unwrap();
    assert_eq!(r["did"], "Showed line 1 of kit/lib/util.lua");
    assert_eq!((st.s.editor.row, st.s.editor.col), (row, col));
    st.save_editor(&mut game);
    // A prompt open in the studio: it waits.
    st.s.ui.naming = Some(Default::default());
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap();
    assert_eq!(r["busy"], true, "{r}");
    st.s.ui.naming = None;
    // A request sent again with its key is answered as it was, and not done again.
    let first = ask(&mut st, &mut game, "open", json!({ "id": "hello", "key": "k1" })).unwrap();
    assert_eq!(first["did"], "Opened hello.lua");
    let scratch = st.scratch_path();
    st.s.editor.open(&scratch, "scratch.lua");
    let again = ask(&mut st, &mut game, "open", json!({ "id": "hello", "key": "k1" })).unwrap();
    assert_eq!(again, first);
    assert_eq!(st.s.editor.title, "scratch.lua", "not opened a second time");
    // The editor in the link's status.
    let s = coach::summary(&st);
    assert_eq!((&s["file"], &s["dirty"]), (&json!("scratch.lua"), &json!(false)));
}

#[test]
fn input_that_comes_in_a_requests_frame_is_the_player_using_the_studio() {
    let (mut game, mut st) = with_module("coach-input-coming");
    st.s.ui.open = true;
    st.s.now = 10.0;
    // (The game handles the link's requests before the frame's input.) A
    // key pressed this frame, not handled yet: opening another file waits.
    coach::input_coming(&mut st, &[Input::Mouse(5.0, 5.0), key(Key::Char('x'), NONE)]);
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap();
    assert_eq!(r["busy"], true, "{r}");
    assert_eq!(st.s.editor.title, "scratch.lua");
    // The pointer moving isn't using it, and nothing reaches it while it is closed.
    st.s.now = 13.0;
    coach::input_coming(&mut st, &[Input::Mouse(6.0, 6.0)]);
    st.s.ui.open = false;
    coach::input_coming(&mut st, &[Input::Text("x".into())]);
    st.s.ui.open = true;
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap();
    assert_eq!(r["did"], "Opened hello.lua", "{r}");
}

#[test]
fn a_drag_held_or_a_prompt_up_keeps_the_players_file_and_tab() {
    // The player selects lines with the mouse, slowly: long after the press
    // they are still using the studio, and another file waits.
    let src: String = (1..=12).map(|i| format!("local v{i} = {i}\n")).collect();
    let (mut game, mut st) = editing("coach-drag", "dr", &src);
    let (x, y) = text_at(&game, "v2");
    let (_, y5) = text_at(&game, "v5");
    handle(&mut st, &mut game, Input::Mouse(x + 1.0, y + 2.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    for k in 1..=30 {
        st.s.now += 0.1;
        handle(
            &mut st,
            &mut game,
            Input::Mouse(x + 1.0, y + 2.0 + (y5 - y) * k as f32 / 30.0),
        );
    }
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap();
    assert_eq!(r["busy"], true, "{r}");
    assert_eq!(st.s.editor.title, "dr.lua");
    assert!(st.s.ui.dragging_any(), "the drag goes on");
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    st.s.now += 2.5;
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello" })).unwrap();
    assert_eq!(r["did"], "Opened hello.lua", "{r}");
    // In the Library with a prompt or a menu up, the file open already
    // shown, or a lesson started in it, leaves the tab where it is (the
    // Duplicate prompt's Enter would open the copy in the Editor tab).
    st.s.now += 2.5;
    st.s.ui.tab = Tab::Library;
    st.s.ui.naming = Some(Default::default());
    let r = ask(&mut st, &mut game, "open", json!({ "id": "hello", "line": 1 })).unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(st.s.ui.tab, Tab::Library);
    let lesson = json!({ "title": "Hi", "steps": [{ "start_line": 1, "note": "It says hello." }] });
    ask(&mut st, &mut game, "lesson", lesson).unwrap();
    assert_eq!(st.s.ui.tab, Tab::Library);
}

#[test]
fn a_file_the_editor_couldnt_read_is_never_read_changed_or_saved_through_it() {
    let mut game = FakeGame::new("coach-unreadable");
    let mut st = studio(&mut game);
    // A script in Windows-1252, not UTF-8: the editor opens it empty, and says why.
    let bytes: &[u8] = b"-- caf\xe9\nprint('hello')\nlocal x = 1\n";
    let path = game.mods_dir().join("latin.lua");
    std::fs::create_dir_all(game.mods_dir()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    st.scan(&mut game);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    let r = ask(&mut st, &mut game, "open", json!({ "id": "latin" })).unwrap();
    assert!(r["did"].as_str().unwrap().contains("couldn't read latin.lua"), "{r}");
    assert!(st.s.editor.problem.is_some());
    // Reading it, changing it, marking it and teaching in it say why, and do nothing.
    let e = ask(&mut st, &mut game, "editor", json!({})).unwrap_err();
    assert!(e.contains("couldn't read latin.lua") && e.contains("UTF-8"), "{e}");
    for (method, params) in [
        (
            "edit",
            json!({ "text": "print('new')\n", "typing": false, "save": true }),
        ),
        ("mark", json!({ "start_line": 1, "note": "Here." })),
        (
            "lesson",
            json!({ "title": "T", "steps": [{ "start_line": 1, "note": "One." }] }),
        ),
    ] {
        let e = ask(&mut st, &mut game, method, params).unwrap_err();
        assert!(e.contains("couldn't read latin.lua"), "{method}: {e}");
    }
    // So with the studio closed, where an edit is saved by default.
    st.s.ui.open = false;
    let e = ask(&mut st, &mut game, "edit", json!({ "start_line": 1, "new_text": "x" })).unwrap_err();
    assert!(e.contains("couldn't read latin.lua"), "{e}");
    st.s.ui.open = true;
    // What the player types into the empty buffer isn't saved over the
    // file by save_script, nor when the game's own agent's turn ends well.
    handle(&mut st, &mut game, Input::Text("oops".into()));
    st.s.now += 3.0;
    let e = ask(&mut st, &mut game, "save", json!({})).unwrap_err();
    assert!(e.contains("couldn't read latin.lua"), "{e}");
    st.s.ui.coach.turn_ended(true);
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "the file is as it was");
    // Written again in UTF-8 (write_mod, another program): read in, it is the agent's to read.
    st.open_file(&path);
    assert!(st.s.editor.problem.is_some() && !st.s.editor.dirty);
    std::fs::write(&path, "-- café\nprint('hello')\n").unwrap();
    look_at_disk(&mut st, &mut game);
    while st.s.ui.coach.typing.is_some() {
        frame(&mut st, &mut game);
    }
    assert!(st.s.editor.problem.is_none());
    let r = ask(&mut st, &mut game, "editor", json!({})).unwrap();
    assert_eq!(
        (&r["line_count"], &r["lines"][0]),
        (&json!(2), &json!("-- café")),
        "{r}"
    );
}

// ----------------------------------------------------------------- live edits

#[test]
fn an_edit_types_in_and_undo_takes_it_back_in_one_step() {
    let (mut game, mut st) = editing("coach-type", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('one')\nprint('two')\nprint('three')" }),
    )
    .unwrap();
    let id = r["typing"].as_u64().expect("it types");
    assert_eq!(r["did"], "Typing into greet.lua");
    // The text it replaces shows a moment first.
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(st.s.ui.coach.typing.as_ref().unwrap().phase, Phase::ShowOld);
    assert_eq!(st.s.editor.text(), GREET);
    // Then the new text comes in, a part at a time.
    run_for(&mut st, &mut game, 0.35);
    let t = st.s.ui.coach.typing.as_ref().expect("still typing");
    assert_eq!(t.phase, Phase::Typing);
    assert!(t.shown > 0 && t.shown < t.text.len(), "{}", st.s.editor.text());
    assert!(!st.s.editor.text().contains("print('hi')"));
    assert_eq!(
        ask(&mut st, &mut game, "typed", json!({ "id": id })).unwrap()["done"],
        false
    );
    // Reading meanwhile gives the text as it will be.
    let read = ask(&mut st, &mut game, "editor", json!({})).unwrap();
    assert_eq!(read["lines"][3], "print('three')");
    assert_eq!(read["typing"]["id"], id);
    let r = typed_in(&mut st, &mut game, id);
    assert_eq!(r["lines"], json!([2, 4]));
    assert_eq!(
        (&r["animated"], &r["problems"], &r["saved"]),
        (&json!(true), &json!([]), &json!(false))
    );
    assert_eq!(r["did"], "Typed lines 2-4 into greet.lua");
    let whole = "local n = 0\nprint('one')\nprint('two')\nprint('three')\nreturn n\n";
    assert_eq!(st.s.editor.text(), whole);
    assert!(st.s.editor.dirty);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET, "not saved");
    assert_eq!(actions(&st), ["Claude Code: Typed lines 2-4 into greet.lua"]);
    let jump = st.s.ui.agent.lines.last().unwrap().jump.clone().unwrap();
    assert_eq!((jump.file.as_str(), jump.first, jump.last), ("greet.lua", 1, 3));
    // One undo takes all of it back, and redo puts it back.
    st.s.editor.undo();
    assert_eq!(st.s.editor.text(), GREET);
    st.s.editor.redo();
    assert_eq!(st.s.editor.text(), whole);
}

#[test]
fn the_players_cursor_keeps_its_place_and_the_view_follows_the_agent() {
    let (mut game, mut st) = editing("coach-place", "long", &(numbered(200) + "\n"));
    st.s.editor.click(39, 3, false);
    redraw(&mut st, &mut game);
    assert!(st.s.editor.scroll < 39, "line 40 in view");
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 151, "end_line": 150, "new_text": "a = 1\nb = 2\nc = 3" }),
    )
    .unwrap();
    let id = r["typing"].as_u64().unwrap();
    run_for(&mut st, &mut game, 0.1);
    let ed = &st.s.editor;
    // (A third of the way down a view of some 20 lines.)
    assert!(
        ed.scroll > 120 && ed.scroll <= 150,
        "the view went to it: {}",
        ed.scroll
    );
    assert_eq!((ed.row, ed.col), (39, 3), "the player's cursor stays");
    let r = typed_in(&mut st, &mut game, id);
    assert_eq!(r["lines"], json!([151, 153]));
    assert_eq!(st.s.editor.lines[150..153], ["a = 1", "b = 2", "c = 3"]);
    assert_eq!((st.s.editor.row, st.s.editor.col), (39, 3));
    // Lines put in above it: the cursor goes down with its line.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 10, "end_line": 9, "new_text": "x = 1\ny = 2\nz = 3\n" }),
    )
    .unwrap();
    typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
    assert_eq!((st.s.editor.row, st.s.editor.col), (42, 3));
    assert_eq!(st.s.editor.lines[42], "line 40");
    // The wheel looks elsewhere: the view stops following the agent's caret.
    let block: Vec<String> = (1..=8).map(|i| format!("local value_number_{i} = {i}")).collect();
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 190, "end_line": 189, "new_text": block.join("\n") }),
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.45);
    handle(&mut st, &mut game, Input::Wheel(2.0));
    let held = st.s.editor.scroll;
    run_for(&mut st, &mut game, 0.15);
    assert!(
        st.s.ui.coach.typing.as_ref().is_some_and(|t| t.free),
        "still typing, and free"
    );
    assert_eq!(st.s.editor.scroll, held);
    typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
    assert_eq!(st.s.editor.scroll, held);
}

#[test]
fn a_cursor_at_the_end_of_what_an_edit_replaces_ends_after_the_new_text() {
    // "55" becomes "1000": what goes is columns 10-12 of line 1, typed in
    // as the player watches or put in at once, to the same end.
    for typing in [true, false] {
        let src = "local x = 55\nprint(x)\n";
        let edit = json!({ "old_text": "local x = 55", "new_text": "local x = 1000", "typing": typing });
        let run = |caret: (usize, usize), anchor: Option<(usize, usize)>| {
            let (mut game, mut st) = editing(&format!("coach-park-{typing}"), "park", src);
            st.s.editor.click(caret.0, caret.1, false);
            st.s.editor.anchor = anchor;
            let r = ask(&mut st, &mut game, "edit", edit.clone()).unwrap();
            if let Some(id) = r["typing"].as_u64() {
                typed_in(&mut st, &mut game, id);
            }
            assert_eq!(st.s.editor.lines[0], "local x = 1000");
            (game, st)
        };
        // At the end of what went: after the new text, and the player's
        // next key lands after the agent's code.
        let (mut game, mut st) = run((0, 12), None);
        assert_eq!((st.s.editor.row, st.s.editor.col), (0, 14), "typing {typing}");
        handle(&mut st, &mut game, Input::Text(" -- big".into()));
        assert_eq!(st.s.editor.lines[0], "local x = 1000 -- big", "typing {typing}");
        // Inside it: at the new text's start.
        let (_, st) = run((0, 11), None);
        assert_eq!((st.s.editor.row, st.s.editor.col), (0, 10), "typing {typing}");
        // The selection's other end the same way.
        let (_, st) = run((0, 0), Some((0, 12)));
        assert_eq!(st.s.editor.anchor, Some((0, 14)), "typing {typing}");
        let (_, st) = run((0, 0), Some((0, 11)));
        assert_eq!(st.s.editor.anchor, Some((0, 10)), "typing {typing}");
    }
}

#[test]
fn an_edit_lands_at_once_when_the_player_types() {
    let (mut game, mut st) = editing("coach-cut", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    st.s.editor.click(0, 0, false);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('hello there')", "save": true }),
    )
    .unwrap();
    let id = r["typing"].as_u64().unwrap();
    run_for(&mut st, &mut game, 0.4);
    assert!(st.s.ui.coach.typing.is_some());
    // The player's key: the rest of the edit is in first, and the key lands at their own caret.
    handle(&mut st, &mut game, Input::Text("x".into()));
    assert!(st.s.ui.coach.typing.is_none());
    assert_eq!(st.s.editor.text(), "xlocal n = 0\nprint('hello there')\nreturn n\n");
    let r = ask(&mut st, &mut game, "typed", json!({ "id": id })).unwrap();
    assert_eq!(r["done"], true);
    let r = &r["result"];
    assert_eq!(r["saved"], false, "cut short, it isn't saved");
    assert!(r["cut_short"].as_str().unwrap().contains("the player typed"), "{r}");
    assert!(r["why"].as_str().unwrap().contains("not saved"), "{r}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET);
    // Two undos: the player's key, then the agent's edit.
    st.s.editor.undo();
    assert_eq!(st.s.editor.text(), "local n = 0\nprint('hello there')\nreturn n\n");
    st.s.editor.undo();
    assert_eq!(st.s.editor.text(), GREET);
}

#[test]
fn a_press_while_an_edit_comes_in_is_for_the_line_drawn_under_it() {
    // An agent's edit is typing four lines in at line 2 when the player
    // presses a line below: the rest of the edit goes in at once, which
    // moves that line down, and the press is still for that line, a right
    // press's menu (which acts on the cursor) and a left press's cursor.
    let src: String = (1..=14).map(|i| format!("local v{i} = {i}\n")).collect();
    let (mut game, mut st) = editing("coach-press-mid-edit", "pm", &src);
    let comment = "-- a comment long enough to take a moment to type in";
    let new_text = [comment; 4].join("\n");
    for left in [false, true] {
        let edit = json!({ "start_line": 2, "end_line": 1, "new_text": new_text });
        ask(&mut st, &mut game, "edit", edit).unwrap();
        run_for(&mut st, &mut game, 1.0);
        redraw(&mut st, &mut game);
        assert!(st.s.ui.coach.typing.is_some(), "still typing");
        // The lowest line the view shows.
        let word = (1..=14)
            .map(|i| format!("v{i}"))
            .filter(|w| !texts_at(&game, w).is_empty())
            .max_by(|a, b| text_at(&game, a).1.total_cmp(&text_at(&game, b).1))
            .unwrap();
        let (x, y) = text_at(&game, &word);
        handle(&mut st, &mut game, Input::Mouse(x + 1.0, y + 2.0));
        handle(&mut st, &mut game, Input::Button { left, down: true });
        handle(&mut st, &mut game, Input::Button { left, down: false });
        assert!(st.s.ui.coach.typing.is_none(), "put in whole");
        let ed = &st.s.editor;
        assert!(
            ed.lines[ed.row].contains(&format!("{word} = ")),
            "{left}: {:?}",
            ed.lines[ed.row]
        );
        assert_eq!(st.s.ui.menu.is_some(), !left);
        st.s.ui.menu = None;
        st.s.ui.coach.player_at = f64::NEG_INFINITY;
    }
}

#[test]
fn the_editors_suggestions_stay_with_the_players_word_through_an_edit() {
    let start = |st: &Studio| st.s.ui.suggest.as_ref().map(|s| s.start);
    // The player types after "local x = 1": the list opens at their word.
    let (mut game, mut st) = editing("coach-suggest-word", "sg", "local x = 1\nreturn x\n");
    st.s.editor.click(0, 11, false);
    for t in [" + ", "p", "r", "i"] {
        handle(&mut st, &mut game, Input::Text(t.into()));
    }
    assert_eq!(start(&st), Some(14));
    // An agent's edit before it on their line lands at once (they are
    // typing): the list goes with their word, and Tab puts the suggestion
    // over that word alone.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "local x = 1", "new_text": "local count = 1" }),
    )
    .unwrap();
    assert_eq!(r["animated"], false);
    assert_eq!(st.s.editor.lines[0], "local count = 1 + pri");
    assert_eq!(start(&st), Some(18));
    let item = st.s.ui.suggest.as_ref().unwrap().items[0].insert.clone();
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    let line = st.s.editor.lines[0].clone();
    assert!(line.starts_with(&format!("local count = 1 + {item}")), "{line:?}");
    // Letters of more than a byte before it, as Ctrl+Space shows the list.
    let (mut game, mut st) = editing("coach-suggest-bytes", "sg", "x = \nreturn x\n");
    st.s.editor.click(0, 4, false);
    handle(&mut st, &mut game, Input::Text("pri".into()));
    handle(&mut st, &mut game, key(Key::Char(' '), CTRL));
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "x = pri", "new_text": "aéé = pri" }),
    )
    .unwrap();
    assert_eq!(start(&st), Some(8));
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert!(
        st.s.editor.lines[0].starts_with("aéé = pri"),
        "{:?}",
        st.s.editor.lines[0]
    );
    // An edit that changes the word itself closes the list.
    let (mut game, mut st) = editing("coach-suggest-gone", "sg", "x = \nreturn x\n");
    st.s.editor.click(0, 4, false);
    handle(&mut st, &mut game, Input::Text("pri".into()));
    assert!(start(&st).is_some());
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "pri", "new_text": "pairs" }),
    )
    .unwrap();
    assert_eq!(st.s.editor.lines[0], "x = pairs");
    assert_eq!(start(&st), None);
    // One typed in elsewhere as the player watches leaves it open, for Tab.
    let (mut game, mut st) = editing("coach-suggest-far", "sg", "x = \nreturn x\n");
    st.s.editor.click(0, 4, false);
    handle(&mut st, &mut game, Input::Text("pri".into()));
    run_for(&mut st, &mut game, 2.5);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "return x", "new_text": "return x + 1" }),
    )
    .unwrap();
    typed_in(&mut st, &mut game, r["typing"].as_u64().expect("typed in"));
    assert_eq!(start(&st), Some(4));
    let item = st.s.ui.suggest.as_ref().unwrap().items[0].insert.clone();
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert!(
        st.s.editor.lines[0].starts_with(&format!("x = {item}")),
        "{:?}",
        st.s.editor.lines[0]
    );
    // A list whose word isn't where it says any more just closes.
    let (mut game, mut st) = editing("coach-suggest-stale", "sg", "x = \nreturn x\n");
    st.s.editor.click(0, 4, false);
    handle(&mut st, &mut game, Input::Text("pri".into()));
    st.s.ui.suggest.as_mut().unwrap().start = 40;
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    assert_eq!((st.s.editor.lines[0].as_str(), start(&st)), ("x = pri", None));
}

#[test]
fn saving_or_switching_files_never_saves_half_typed_code() {
    let whole = "local n = 0\nprint('every word of it')\nreturn n\n";
    let new = json!({ "old_text": "print('hi')", "new_text": "print('every word of it')" });
    // Ctrl+S.
    let (mut game, mut st) = editing("coach-half-save", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    ask(&mut st, &mut game, "edit", new.clone()).unwrap();
    run_for(&mut st, &mut game, 0.4);
    assert!(st.s.ui.coach.typing.is_some());
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), whole);
    // A click on another script in the list (which saves the file open first).
    let (mut game, mut st) = editing("coach-half-switch", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    ask(&mut st, &mut game, "edit", new.clone()).unwrap();
    run_for(&mut st, &mut game, 0.4);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "hello.lua");
    click(&mut st, &mut game, (at.0 + 2.0, at.1 + 2.0));
    assert_eq!(st.s.editor.title, "hello.lua");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), whole);
    // Asking the game's own agent (which saves the file open first).
    #[cfg(unix)]
    {
        use horizon_studio::game::hstudio_agent::AgentSpec;
        let (mut game, mut st) = editing("coach-half-ask", "greet", GREET);
        let path = st.s.editor.path.clone().unwrap();
        st.s.ui.agent.agents = Some(vec![AgentSpec {
            id: "custom".into(),
            name: "Echo".into(),
            program: "/bin/echo".into(),
            args: vec!["{prompt}".into()],
            json: false,
        }]);
        ask(&mut st, &mut game, "edit", new).unwrap();
        run_for(&mut st, &mut game, 0.4);
        horizon_studio::game::hstudio_ui::agent_ask(&mut st, "is it right?");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), whole);
        assert!(coach::summary(&st)["unsaved"].is_null(), "saved, by whoever");
    }
}

#[test]
fn edits_find_their_place_or_say_why() {
    let (mut game, mut st) = editing("coach-modes", "modes", "a = 1\nb = 2\nc = 3\n");
    let mut edit = |st: &mut Studio, params: Json| {
        let mut params = params;
        params["typing"] = json!(false);
        ask(st, &mut game, "edit", params)
    };
    // What it can't find, or finds more than once, or can't tell.
    let e = edit(&mut st, json!({ "old_text": "zzz", "new_text": "y" })).unwrap_err();
    assert!(e.contains("isn't in modes.lua") && e.contains("read_script"), "{e}");
    edit(&mut st, json!({ "text": "x = 1\ny = 2\nx = 1\n" })).unwrap();
    let e = edit(&mut st, json!({ "old_text": "x = 1", "new_text": "q" })).unwrap_err();
    assert!(e.contains("at lines 1 and 3"), "{e}");
    let e = edit(&mut st, json!({ "old_text": "y", "text": "b", "new_text": "c" })).unwrap_err();
    assert!(e.contains("give one of"), "{e}");
    let e = edit(&mut st, json!({ "old_text": "y = 2" })).unwrap_err();
    assert!(e.contains("new_text is missing"), "{e}");
    let e = edit(&mut st, json!({ "start_line": 0, "new_text": "q" })).unwrap_err();
    assert!(e.contains("counted from 1"), "{e}");
    let e = edit(&mut st, json!({ "start_line": 9, "new_text": "q" })).unwrap_err();
    assert!(e.contains("has 3 lines"), "{e}");
    let e = edit(&mut st, json!({ "start_line": 2, "end_line": 7, "new_text": "q" })).unwrap_err();
    assert!(e.contains("end_line 7"), "{e}");
    // Whole lines: replaced, put in before a line, added at the end, removed.
    edit(&mut st, json!({ "text": "a = 1\nb = 2\nc = 3\n" })).unwrap();
    let r = edit(&mut st, json!({ "start_line": 2, "end_line": 3, "new_text": "B\nC\n" })).unwrap();
    assert_eq!(st.s.editor.text(), "a = 1\nB\nC\n");
    assert_eq!(
        (&r["lines"], &r["did"]),
        (&json!([2, 3]), &json!("Changed lines 2-3 in modes.lua"))
    );
    let r = edit(&mut st, json!({ "start_line": 1, "end_line": 0, "new_text": "top" })).unwrap();
    assert_eq!(st.s.editor.text(), "top\na = 1\nB\nC\n");
    assert_eq!(r["lines"], json!([1, 1]));
    let r = edit(&mut st, json!({ "start_line": 5, "new_text": "last" })).unwrap();
    assert_eq!(st.s.editor.text(), "top\na = 1\nB\nC\nlast\n");
    assert_eq!(r["lines"], json!([5, 5]));
    let r = edit(&mut st, json!({ "start_line": 1, "end_line": 2, "new_text": "" })).unwrap();
    assert_eq!(st.s.editor.text(), "B\nC\nlast\n");
    assert_eq!(
        (&r["removed"], &r["did"]),
        (&json!([1, 2]), &json!("Removed lines 1-2 from modes.lua"))
    );
    let r = edit(&mut st, json!({ "start_line": 3, "new_text": "" })).unwrap();
    assert_eq!(st.s.editor.text(), "B\nC\n", "the last line too: {r}");
    // The whole file; and old_text with tabs and Windows line ends matches as the editor has it.
    edit(&mut st, json!({ "text": "if x then\n\ty()\nend\n" })).unwrap();
    assert_eq!(st.s.editor.text(), "if x then\n  y()\nend\n");
    edit(
        &mut st,
        json!({ "old_text": "if x then\r\n\ty()", "new_text": "if z then\n  y()" }),
    )
    .unwrap();
    assert_eq!(st.s.editor.text(), "if z then\n  y()\nend\n");
    // Asking for what is there already changes nothing.
    let r = edit(&mut st, json!({ "old_text": "end", "new_text": "end" })).unwrap();
    assert_eq!(r["changed"], false);
    // Text that is there twice, overlapping itself, is there twice.
    edit(&mut st, json!({ "text": "end\nend\nend\n" })).unwrap();
    let e = edit(&mut st, json!({ "old_text": "end\nend", "new_text": "X" })).unwrap_err();
    assert!(e.contains("at lines 1 and 2"), "{e}");
    assert_eq!(st.s.editor.text(), "end\nend\nend\n");
}

#[test]
fn an_edit_on_disk_changes_its_text_alone_and_keeps_the_files_tabs_and_line_ends() {
    let (mut game, mut st) = editing("coach-disk-raw", "open", "print('open')\n");
    let tabs = game.mods_dir().join("tabs.lua");
    // A tab inside a string is code: it stays a tab, and the line ends stay Windows'.
    std::fs::write(&tabs, "local sep = \"a\tb\"\r\nlocal n = 1\r\nreturn n\r\n").unwrap();
    st.scan(&mut game);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "tabs", "old_text": "local n = 1", "new_text": "local n = 2\nlocal m = 3" }),
    )
    .unwrap();
    assert_eq!(
        r["did"],
        "Changed lines 2-3 in tabs.lua (on disk: the editor has open.lua open)"
    );
    assert_eq!(
        std::fs::read_to_string(&tabs).unwrap(),
        "local sep = \"a\tb\"\r\nlocal n = 2\r\nlocal m = 3\r\nreturn n\r\n"
    );
    // Lines taken out, with their line ends.
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "tabs", "start_line": 3, "new_text": "" }),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&tabs).unwrap(),
        "local sep = \"a\tb\"\r\nlocal n = 2\r\nreturn n\r\n"
    );
    // A change found in a tab's spaces (as read_script shows it) takes the tab's place.
    let indented = game.mods_dir().join("indented.lua");
    std::fs::write(&indented, "if x then\n\ty()\nend\n").unwrap();
    st.scan(&mut game);
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "indented", "old_text": " y()", "new_text": " z()" }),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&indented).unwrap(), "if x then\n  z()\nend\n");
    // A file with no last line break gets one, as the editor saves files.
    let bare = game.mods_dir().join("bare.lua");
    std::fs::write(&bare, "local a = 1").unwrap();
    st.scan(&mut game);
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "bare", "old_text": "1", "new_text": "2" }),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&bare).unwrap(), "local a = 2\n");
}

#[test]
fn while_the_player_types_edits_land_at_once_and_other_files_go_to_disk() {
    let (mut game, mut st) = editing("coach-hands-off", "greet", GREET);
    std::fs::create_dir_all(game.mods_dir().join("kit")).unwrap();
    std::fs::write(
        game.mods_dir().join("kit/main.lua"),
        "local u = require('util')\nprint(u.two())\n",
    )
    .unwrap();
    std::fs::write(
        game.mods_dir().join("kit/util.lua"),
        "return { two = function() return 2 end }\n",
    )
    .unwrap();
    st.scan(&mut game);
    // The player is typing.
    st.s.editor.click(2, 8, false);
    handle(&mut st, &mut game, Input::Text("-".into()));
    let (caret, scroll) = ((st.s.editor.row, st.s.editor.col), st.s.editor.scroll);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('hi')\nprint('there')", "save": true }),
    )
    .unwrap();
    assert!(r.get("typing").is_none(), "at once: {r}");
    assert_eq!(r["animated"], false);
    assert_eq!(r["saved"], false);
    assert!(r["why"].as_str().unwrap().contains("typing"), "{r}");
    assert_eq!(
        (st.s.editor.row, st.s.editor.col),
        (caret.0 + 1, caret.1),
        "the cursor with its line"
    );
    assert_eq!(st.s.editor.scroll, scroll);
    assert_eq!(
        st.s.editor.text(),
        "local n = 0\nprint('hi')\nprint('there')\nreturn n-\n"
    );
    // A file the editor doesn't have open: written to disk, and its mod reloads; the editor stays.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "kit", "file": "util.lua", "old_text": "return 2", "new_text": "return 3" }),
    )
    .unwrap();
    assert_eq!(r["saved"], true);
    assert_eq!(r["mod"]["state"], "running", "{r}");
    assert!(r["did"].as_str().unwrap().contains("on disk"), "{r}");
    assert_eq!(
        std::fs::read_to_string(game.mods_dir().join("kit/util.lua")).unwrap(),
        "return { two = function() return 3 end }\n"
    );
    assert_eq!(st.s.editor.title, "greet.lua");
    assert!(
        log(&st).iter().any(|l| l.ends_with('3')),
        "the module ran again: {:?}",
        log(&st)
    );
    assert_eq!(
        actions(&st).last().unwrap(),
        "Claude Code: Changed line 1 in kit/util.lua"
    );
}

#[test]
fn motion_off_or_nobody_watching_puts_edits_in_at_once() {
    let (mut game, mut st) = editing("coach-at-once", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    st.s.ui.look.motion = false;
    let r = ask(&mut st, &mut game, "edit", json!({ "old_text": "0", "new_text": "1" })).unwrap();
    assert_eq!((&r["animated"], &r["saved"]), (&json!(false), &json!(false)), "{r}");
    assert!(st.s.ui.coach.flash.is_some());
    assert!(st.s.editor.dirty);
    // With the studio closed it is saved, and the mod reloads (unless save is false).
    st.s.ui.open = false;
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "return n", "new_text": "return n + 1" }),
    )
    .unwrap();
    assert_eq!((&r["saved"], &r["dirty"]), (&json!(true), &json!(false)), "{r}");
    assert_eq!(r["mod"]["state"], "running");
    assert!(r.get("player_edits").is_none(), "{r}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "local n = 1\nprint('hi')\nreturn n + 1\n"
    );
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "+ 1", "new_text": "+ 2", "save": false }),
    )
    .unwrap();
    assert_eq!(r["saved"], false);
    assert!(st.s.editor.dirty);
}

#[test]
fn with_the_studio_closed_an_edit_saves_nothing_of_the_players() {
    let (mut game, mut st) = editing("coach-closed-theirs", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    // The player half wrote a line, paused, and closed the studio.
    st.s.editor.click(0, 0, false);
    handle(&mut st, &mut game, Input::Text("local half = (".into()));
    st.s.now += 3.0;
    st.s.ui.open = false;
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "return n", "new_text": "return n + 1" }),
    )
    .unwrap();
    assert_eq!(r["saved"], false, "{r}");
    assert!(
        r["why"]
            .as_str()
            .unwrap()
            .contains("the player has unsaved edits in greet.lua"),
        "{r}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET, "nothing written");
    assert_eq!(
        st.s.editor.text(),
        "local half = (local n = 0\nprint('hi')\nreturn n + 1\n"
    );
    // Asked to, it saves the file as it is, theirs too, and says so.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "n + 1", "new_text": "n + 2", "save": true }),
    )
    .unwrap();
    assert_eq!(
        (&r["saved"], &r["player_edits"], &r["dirty"]),
        (&json!(true), &json!(true), &json!(false)),
        "{r}"
    );
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("local half = ("));
    // save_script too; and only the agent's edits say nothing of the player's.
    st.s.ui.open = true;
    handle(&mut st, &mut game, Input::Text("1)".into()));
    st.s.now += 3.0;
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!((&r["saved"], &r["player_edits"]), (&json!(true), &json!(true)), "{r}");
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "n + 2", "new_text": "n + 3", "typing": false }),
    )
    .unwrap();
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(r["saved"], true, "{r}");
    assert!(r.get("player_edits").is_none(), "{r}");
}

#[test]
fn save_script_reloads_the_mod_and_says_how_it_is() {
    let (mut game, mut st) = editing("coach-save", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(
        (&r["saved"], &r["did"]),
        (&json!(false), &json!("greet.lua has no unsaved edits"))
    );
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('saved')", "typing": false }),
    )
    .unwrap();
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(
        (&r["saved"], &r["mod"]["state"]),
        (&json!(true), &json!("running")),
        "{r}"
    );
    assert_eq!(r["did"], "Saved greet.lua: running");
    assert!(std::fs::read_to_string(&path).unwrap().contains("print('saved')"));
    assert_eq!(actions(&st).last().unwrap(), "Claude Code: Saved greet.lua: running");
    assert!(log(&st).iter().any(|l| l.contains("saved")));
    // A broken edit: saved, and where it failed.
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "return n", "new_text": "return n + )", "typing": false }),
    )
    .unwrap();
    let r = ask(&mut st, &mut game, "save", json!({ "id": "greet" })).unwrap();
    assert_eq!(
        (&r["mod"]["state"], &r["mod"]["line"]),
        (&json!("failed"), &json!(3)),
        "{r}"
    );
    assert_eq!(r["did"], "Saved greet.lua: it failed at line 3");
    assert!(!r["problems"].as_array().unwrap().is_empty());
    // It waits while the player types, and won't write over a change made on disk meanwhile.
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 3, "new_text": "return n", "typing": false }),
    )
    .unwrap();
    st.s.ui.coach.player_at = st.s.now;
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(r["busy"], true, "{r}");
    st.s.ui.coach.player_at = f64::NEG_INFINITY;
    std::fs::write(&path, "print('from elsewhere, longer')\n").unwrap();
    let e = ask(&mut st, &mut game, "save", json!({})).unwrap_err();
    assert!(e.contains("changed on disk"), "{e}");
    assert!(std::fs::read_to_string(&path).unwrap().contains("from elsewhere"));
    // A file that isn't open has nothing to save.
    let e = ask(&mut st, &mut game, "save", json!({ "id": "hello" })).unwrap_err();
    assert!(e.contains("isn't open"), "{e}");
}

#[test]
fn a_new_script_is_typed_in_and_saved_into_a_new_mod() {
    let (mut game, mut st) = editing("coach-new", "greet", GREET);
    st.s.ui.coach.input_at = f64::NEG_INFINITY;
    let r = ask(&mut st, &mut game, "open", json!({ "id": "glow", "create": true })).unwrap();
    assert_eq!(r["created"], true);
    let r = ask(&mut st, &mut game, "edit", json!({ "text": "print('glow loaded')\n" })).unwrap();
    let r = typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
    assert_eq!(r["file"], "glow.lua");
    let r = ask(&mut st, &mut game, "save", json!({ "id": "glow" })).unwrap();
    assert_eq!(r["mod"]["state"], "running", "{r}");
    assert_eq!(
        std::fs::read_to_string(game.mods_dir().join("glow.lua")).unwrap(),
        "print('glow loaded')\n"
    );
    assert!(log(&st).iter().any(|l| l.contains("glow loaded")));
}

#[test]
fn an_edit_sent_again_with_its_key_is_done_once() {
    let (mut game, mut st) = editing("coach-key", "greet", GREET);
    let params = json!({ "old_text": "return n", "new_text": "return n * 2", "typing": false, "key": "e1" });
    let first = ask(&mut st, &mut game, "edit", params.clone()).unwrap();
    let again = ask(&mut st, &mut game, "edit", params).unwrap();
    assert_eq!(first, again);
    assert_eq!(st.s.editor.text(), "local n = 0\nprint('hi')\nreturn n * 2\n");
}

#[test]
fn edits_work_in_a_modules_files_and_the_scratch_file() {
    let (mut game, mut st) = with_module("coach-module-edit");
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    // A module's file open in the editor: typed in, checked with its other files.
    let r = ask(&mut st, &mut game, "open", json!({ "id": "kit" })).unwrap();
    assert_eq!(r["file"], "kit/main.lua");
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print(util.two())", "new_text": "print(util.two() + util.two())" }),
    )
    .unwrap();
    let r = typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
    assert_eq!(
        (&r["file"], &r["problems"]),
        (&json!("kit/main.lua"), &json!([])),
        "{r}"
    );
    assert_eq!(r["did"], "Typed line 2 into kit/main.lua");
    // Another file of it, by its path alone: on disk, and the module reloads with it.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "file": "lib/util.lua", "old_text": "return 2", "new_text": "return 21" }),
    )
    .unwrap();
    assert_eq!(
        (&r["file"], &r["mod"]["id"]),
        (&json!("kit/lib/util.lua"), &json!("kit")),
        "{r}"
    );
    assert_eq!(st.s.editor.title, "kit/main.lua");
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(r["mod"]["state"], "running", "{r}");
    assert!(log(&st).iter().any(|l| l.ends_with("42")), "{:?}", log(&st));
    // The scratch file: on disk while another file is open, typed in once it is.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "scratch": true, "start_line": 1, "end_line": 0, "new_text": "-- noted" }),
    )
    .unwrap();
    assert_eq!(
        (&r["written"], &r["mod"]),
        (&json!(st.scratch_path()), &Json::Null),
        "{r}"
    );
    assert!(
        std::fs::read_to_string(st.scratch_path())
            .unwrap()
            .starts_with("-- noted\n")
    );
    ask(&mut st, &mut game, "open", json!({ "scratch": true })).unwrap();
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "-- noted", "new_text": "-- noted twice", "typing": false }),
    )
    .unwrap();
    assert_eq!((&r["file"], &r["dirty"]), (&json!("scratch.lua"), &json!(true)), "{r}");
}

#[test]
fn an_edit_in_a_modules_file_is_checked_with_what_its_other_files_define() {
    let (mut game, mut st) = with_module("coach-module-siblings");
    std::fs::write(
        game.mods_dir().join("kit/shared.lua"),
        "function chat.shout(s) chat.say(s) end\n",
    )
    .unwrap();
    st.scan(&mut game);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    // main.lua open; lib/util.lua edited on disk, checked with main.lua and shared.lua.
    ask(&mut st, &mut game, "open", json!({ "id": "kit" })).unwrap();
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({
            "file": "lib/util.lua",
            "old_text": "return {",
            "new_text": "chat.shout('hi')\nchat.shuot('hi')\nreturn {",
        }),
    )
    .unwrap();
    assert_eq!(r["file"], "kit/lib/util.lua", "{r}");
    // shared.lua's chat.shout is known there; the typo beside it is not.
    let said: Vec<&str> = r["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["message"].as_str())
        .collect();
    assert!(said.iter().any(|m| m.contains("'shuot'")), "{r}");
    assert!(!said.iter().any(|m| m.contains("'shout'")), "{r}");
}

// ----------------------------------------------------------------- what else changes the buffer mid edit

/// An edit of greet.lua's print line being typed in, part of it in.
fn mid_edit(name: &str) -> (FakeGame, Studio, u64) {
    let (mut game, mut st) = editing(name, "greet", GREET);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('typed in whole')" }),
    )
    .unwrap();
    let id = r["typing"].as_u64().unwrap();
    run_for(&mut st, &mut game, 0.45);
    assert!(st.s.ui.coach.typing.as_ref().is_some_and(|t| t.phase == Phase::Typing));
    (game, st, id)
}

fn result_of(st: &mut Studio, game: &mut FakeGame, id: u64) -> Json {
    let r = ask(st, game, "typed", json!({ "id": id })).unwrap();
    assert_eq!(r["done"], true, "{r}");
    r["result"].clone()
}

#[test]
fn clear_in_an_executor_style_finds_the_edit_whole() {
    use horizon_studio::game::hstudio_theme::THEMES;
    let (mut game, mut st, id) = mid_edit("coach-mid-clear");
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "synapse").unwrap();
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Clear");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.text(), "");
    run_for(&mut st, &mut game, 0.2);
    let r = result_of(&mut st, &mut game, id);
    assert!(r["cut_short"].as_str().unwrap().contains("the player clicked"), "{r}");
}

#[test]
fn reload_on_the_conflict_banner_finds_the_edit_whole() {
    let (mut game, mut st, id) = mid_edit("coach-mid-reload");
    let path = st.s.editor.path.clone().unwrap();
    std::fs::write(&path, "print('changed elsewhere, much longer')\n").unwrap();
    st.s.now += 1.0;
    redraw(&mut st, &mut game);
    assert!(st.s.editor.conflict);
    redraw(&mut st, &mut game);
    let at = text_at(&game, "Reload");
    click(&mut st, &mut game, at);
    assert_eq!(st.s.editor.text(), "print('changed elsewhere, much longer')\n");
    run_for(&mut st, &mut game, 0.2);
    let r = result_of(&mut st, &mut game, id);
    assert!(r["cut_short"].as_str().unwrap().contains("the player clicked"), "{r}");
}

#[test]
fn the_consoles_edit_finds_the_edit_whole() {
    let (mut game, mut st, id) = mid_edit("coach-mid-console");
    let path = st.s.editor.path.clone().unwrap();
    st.run_chunk(&mut game, "edit('hello')", false);
    assert_eq!(st.s.editor.title, "hello.lua");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "local n = 0\nprint('typed in whole')\nreturn n\n",
        "saved before hello opened, all of it"
    );
    let r = result_of(&mut st, &mut game, id);
    assert!(
        r["cut_short"]
            .as_str()
            .unwrap()
            .contains("the console opened another file"),
        "{r}"
    );
}

#[test]
fn suggestions_taken_mid_edit_find_it_whole() {
    let (mut game, mut st, id) = mid_edit("coach-mid-suggest");
    st.s.editor.click(2, 8, false);
    handle(&mut st, &mut game, key(Key::Char(' '), CTRL));
    handle(&mut st, &mut game, key(Key::Tab, NONE));
    run_for(&mut st, &mut game, 0.2);
    let r = result_of(&mut st, &mut game, id);
    assert!(r["cut_short"].as_str().unwrap().contains("the player typed"), "{r}");
    assert!(st.s.editor.text().contains("print('typed in whole')"));
}

#[test]
fn run_mid_edit_runs_it_whole() {
    let (mut game, mut st, _) = mid_edit("coach-mid-run");
    handle(&mut st, &mut game, key(Key::Enter, CTRL));
    assert!(log(&st).iter().any(|l| l.contains("typed in whole")), "{:?}", log(&st));
}

#[test]
fn a_buffer_changed_another_way_drops_the_rest_and_says_why() {
    let (mut game, mut st, id) = mid_edit("coach-mid-other");
    st.s.editor.set_text("something else");
    run_for(&mut st, &mut game, 0.1);
    assert!(st.s.ui.coach.typing.is_none());
    assert_eq!(st.s.editor.text(), "something else", "nothing more typed in");
    let r = result_of(&mut st, &mut game, id);
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("changed before"), "{r}");
    let last = st.s.ui.agent.lines.last().unwrap();
    assert_eq!(last.kind, AgentLineKind::Error);
    assert!(last.text.contains("was cut off"), "{}", last.text);
}

#[test]
fn a_file_moved_on_disk_takes_the_edit_along() {
    let (mut game, mut st, id) = mid_edit("coach-mid-moved");
    // The script made a module elsewhere (as write_mod does): the scan takes the editor along.
    let mods = game.mods_dir();
    std::fs::create_dir_all(mods.join("greet")).unwrap();
    std::fs::rename(mods.join("greet.lua"), mods.join("greet/main.lua")).unwrap();
    st.scan(&mut game);
    assert_eq!(st.s.editor.title, "greet/main.lua");
    let r = typed_in(&mut st, &mut game, id);
    assert_eq!((&r["ok"], &r["file"]), (&json!(true), &json!("greet/main.lua")), "{r}");
    assert!(st.s.editor.text().contains("print('typed in whole')"));
}

#[test]
fn the_agent_types_with_its_own_caret_and_name() {
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    let (mut game, mut st) = editing("coach-caret", "greet", GREET);
    st.s.editor.click(0, 0, false);
    redraw(&mut st, &mut game);
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 3, "end_line": 2, "new_text": "local typed_by_agent = true" }),
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.15);
    redraw(&mut st, &mut game);
    // The name on its flag, above the line coming in.
    let (_, flag_y) = text_at(&game, "Claude Code");
    let typed = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text local @") || d.starts_with("text local typed"))
        .cloned();
    let (_, line_y) = text_at(&game, "3");
    assert!(
        flag_y < line_y,
        "the flag is above line 3: {flag_y} {line_y} ({typed:?})"
    );
    assert_eq!(st.s.ui.coach.typing.as_ref().unwrap().at.0, 2);
    // The player's cursor is where it was.
    assert_eq!((st.s.editor.row, st.s.editor.col), (0, 0));
    // Every theme draws an edit coming in, the agent's name on it and a
    // note of a change elsewhere inside the code, however small the window.
    let notice = "Claude Code changed hello.lua";
    st.s.ui.coach.notice = Some(coach::Notice {
        text: notice.into(),
        seq: 1,
        at: st.s.now - 1.0,
    });
    let mut names = 0;
    for i in 0..THEMES.len() {
        for rect in [Some([10.0, 10.0, 330.0, 230.0]), None] {
            st.s.ui.look = Look {
                theme: i,
                rect,
                ..Look::default()
            };
            st.s.ui.tab = Tab::Editor;
            st.s.ui.coach.code_at = None;
            redraw(&mut st, &mut game);
            assert!(game.drawn.len() > 10, "theme {i}");
            let Some(code) = st.s.ui.coach.code_at else {
                continue;
            };
            for at in texts_at(&game, "Claude Code") {
                assert!(
                    inside(code, at),
                    "theme {i} {rect:?}: the name at {at:?}, the code in {code:?}"
                );
                names += 1;
            }
            for at in texts_at(&game, &format!("{notice} · Show")) {
                assert!(
                    inside(code, at),
                    "theme {i} {rect:?}: the note at {at:?}, the code in {code:?}"
                );
            }
        }
    }
    assert!(names >= THEMES.len(), "the name is drawn: {names}");
    typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
}

#[test]
fn typed_says_when_it_knows_no_such_edit() {
    let (mut game, mut st) = editing("coach-typed-id", "greet", GREET);
    let e = ask(&mut st, &mut game, "typed", json!({ "id": 999 })).unwrap_err();
    assert!(e.contains("no edit 999"), "{e}");
    let e = ask(&mut st, &mut game, "typed", json!({ "id": "one" })).unwrap_err();
    assert!(e.contains("needs the id"), "{e}");
    // A finished edit's result is kept 30 s, then it is no edit it knows.
    let r = ask(&mut st, &mut game, "edit", json!({ "old_text": "0", "new_text": "1" })).unwrap();
    let id = r["typing"].as_u64().unwrap();
    assert_eq!(typed_in(&mut st, &mut game, id)["ok"], true);
    run_for(&mut st, &mut game, 31.0);
    let e = ask(&mut st, &mut game, "typed", json!({ "id": id })).unwrap_err();
    assert!(e.contains(&format!("no edit {id}")), "{e}");
}

// ----------------------------------------------------------------- the Agent panel

#[test]
fn the_agent_panel_shows_what_the_agent_did_and_goes_there() {
    use horizon_studio::game::hstudio_ui::Panel;
    let (mut game, mut st) = editing("coach-jump", "greet", GREET);
    st.s.ui.look.motion = false;
    let long: Vec<String> = (1..=60).map(|i| format!("print({i})")).collect();
    game.write_mod("long.lua", &(long.join("\n") + "\n"));
    st.scan(&mut game);
    // An agent outside the game changes a file the editor doesn't show: on disk.
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "long", "old_text": "print(40)", "new_text": "print('forty')" }),
    )
    .unwrap();
    assert_eq!(st.s.editor.title, "greet.lua", "the player's file stays");
    // A note over the code says so, and shows it.
    redraw(&mut st, &mut game);
    let note = text_at(&game, "Claude Code changed long.lua · Show");
    click(&mut st, &mut game, (note.0 + 4.0, note.1 + 4.0));
    assert_eq!(st.s.editor.title, "long.lua");
    assert_eq!(st.s.editor.row, 39);
    assert!(st.s.ui.coach.notice.is_none());
    // Its line in the Agent panel goes there too, from another file.
    let scratch = st.scratch_path();
    st.s.editor.open(&scratch, "scratch.lua");
    st.s.ui.panel = Panel::Agent;
    st.s.ui.panel_focus = true;
    redraw(&mut st, &mut game);
    let line = text_at(&game, "Claude Code: Changed line 40 in long.lua");
    let show = text_at(&game, "Show");
    assert!((show.1 - line.1).abs() < 4.0, "Show is on the line's row");
    click(&mut st, &mut game, (show.0 + 4.0, show.1 + 4.0));
    assert_eq!(st.s.editor.title, "long.lua");
    assert_eq!((st.s.editor.row, st.s.editor.col), (39, 0));
    let scroll = st.s.editor.scroll;
    assert!(scroll > 20 && scroll < 39, "line 40 a third of the way down: {scroll}");
    assert!(!st.s.ui.panel_focus, "the keys go back to the code");
    let lit = st.s.ui.coach.flash.as_ref().map(|f| (f.first, f.last));
    assert_eq!(lit, Some((39, 39)), "the line it changed is lit a moment");
    // The line's text stays for selecting and copying.
    redraw(&mut st, &mut game);
    let line = text_at(&game, "Claude Code: Changed line 40 in long.lua");
    handle(&mut st, &mut game, Input::Mouse(line.0 - 1.0, line.1 + 4.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(line.0 + 100.0, line.1 + 4.0));
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
    assert!(game.clipboard.starts_with("Claude Code"), "{:?}", game.clipboard);
    assert_eq!(st.s.editor.title, "long.lua", "selecting went nowhere");
}

#[test]
fn whoever_saves_an_agents_edits_they_are_saved() {
    let (mut game, mut st) = editing("coach-saved-by-player", "greet", GREET);
    let unsaved = |st: &Studio| coach::summary(st)["unsaved"].clone();
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('fixed')", "typing": false }),
    )
    .unwrap();
    assert_eq!(unsaved(&st)["by"], "Claude Code");
    // The player saves them, then types: what isn't saved now is theirs.
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    assert!(unsaved(&st).is_null());
    st.s.editor.click(2, 8, false);
    handle(&mut st, &mut game, Input::Text(" + 1".into()));
    assert!(st.s.editor.dirty);
    assert!(unsaved(&st).is_null(), "{}", coach::summary(&st));
}

/// A fake Claude Code for the game's own agent: it prints `lines` (its
/// stream), waits a moment (its turn going on), then prints `end`.
#[cfg(unix)]
fn fake_claude(st: &mut Studio, name: &str, lines: &[&str], end: &str) {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    use std::os::unix::fs::PermissionsExt;
    let dir = st.s.dir.join(format!("fake-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("claude");
    let script = format!(
        "#!/bin/sh\ncat <<'EOF'\n{}\nEOF\nsleep 0.6\ncat <<'EOF'\n{end}\nEOF\n",
        lines.join("\n")
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    st.s.ui.agent.agents = Some(vec![AgentSpec::claude(path)]);
}

#[cfg(unix)]
const INIT: &str = r#"{"type":"system","subtype":"init","session_id":"s1","mcp_servers":[{"name":"horizon-studio","status":"connected"}]}"#;
#[cfg(unix)]
const GOOD: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"Fixed it.","session_id":"s1"}"#;
#[cfg(unix)]
const FAILED: &str =
    r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["it broke"],"session_id":"s1"}"#;

/// Frames until the game's own agent is done.
#[cfg(unix)]
fn until_done(st: &mut Studio, game: &mut FakeGame) {
    for _ in 0..400 {
        run_for(st, game, 0.05);
        if !horizon_studio::game::hstudio_ui::agent_busy(st) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the agent never finished");
}

#[cfg(unix)]
#[test]
fn editor_tool_lines_are_not_doubled_and_a_good_turn_saves_the_edits() {
    use horizon_studio::game::hstudio_ui::agent_ask;
    let (mut game, mut st) = editing("coach-turn-good", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    let call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"mcp__horizon-studio__edit_script","input":{"id":"greet","old_text":"print('hi')"}}]},"session_id":"s1"}"#;
    let ok = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"{\"ok\": true, \"did\": \"Changed line 2 in greet.lua\"}"}]}]},"session_id":"s1"}"#;
    let bad_call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"mcp__horizon-studio__edit_script","input":{"file":"lib/util.lua","old_text":"x"}}]},"session_id":"s1"}"#;
    let bad = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t2","is_error":true,"content":[{"type":"text","text":"old_text isn't in greet.lua as the editor has it"}]}]},"session_id":"s1"}"#;
    fake_claude(&mut st, "good", &[INIT, call, ok, bad_call, bad], GOOD);
    agent_ask(&mut st, "fix it");
    // What it does in the editor meanwhile, through the link.
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "print('hi')", "new_text": "print('fixed')", "typing": false }),
    )
    .unwrap();
    until_done(&mut st, &mut game);
    let lines: Vec<(AgentLineKind, String)> = st.s.ui.agent.lines.iter().map(|l| (l.kind, l.text.clone())).collect();
    // Its own line, not named (it is the game's own agent, in its run)...
    assert!(
        lines.contains(&(AgentLineKind::Action, "Changed line 2 in greet.lua".into())),
        "{lines:#?}"
    );
    // ...and no tool line or "ok" for that call besides; a failed call shows as usual.
    assert!(
        !lines.iter().any(|(_, t)| t.starts_with("edit_script greet")),
        "{lines:#?}"
    );
    assert!(!lines.iter().any(|(k, _)| *k == AgentLineKind::ToolOk), "{lines:#?}");
    assert!(
        lines.contains(&(AgentLineKind::Tool, "edit_script lib/util.lua".into())),
        "{lines:#?}"
    );
    assert!(
        lines.contains(&(
            AgentLineKind::ToolFailed,
            "old_text isn't in greet.lua as the editor has it".into()
        )),
        "{lines:#?}"
    );
    // The turn ended well: its edits are saved, and the mod runs them.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "local n = 0\nprint('fixed')\nreturn n\n"
    );
    assert!(!st.s.editor.dirty);
    assert_eq!(
        lines.last().unwrap(),
        &(AgentLineKind::Action, "Saved greet.lua: running".into())
    );
}

#[cfg(unix)]
#[test]
fn a_turn_that_failed_or_found_the_file_changed_leaves_the_edits_unsaved() {
    use horizon_studio::game::hstudio_ui::agent_ask;
    let edit = json!({ "old_text": "print('hi')", "new_text": "print('half done')", "typing": false });
    let last = |st: &Studio| st.s.ui.agent.lines.last().unwrap().text.clone();
    // The turn stopped or failed.
    let (mut game, mut st) = editing("coach-turn-failed", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    fake_claude(&mut st, "failed", &[INIT], FAILED);
    agent_ask(&mut st, "fix it");
    ask(&mut st, &mut game, "edit", edit.clone()).unwrap();
    until_done(&mut st, &mut game);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET);
    assert!(st.s.editor.dirty);
    assert_eq!(
        last(&st),
        "Claude Code's edits to greet.lua aren't saved: Ctrl+S saves them, Ctrl+Z takes them back"
    );
    // The player changed the file after the agent's edit.
    let (mut game, mut st) = editing("coach-turn-player", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    fake_claude(&mut st, "player", &[INIT], GOOD);
    agent_ask(&mut st, "fix it");
    ask(&mut st, &mut game, "edit", edit.clone()).unwrap();
    handle(&mut st, &mut game, Input::Text("-".into()));
    until_done(&mut st, &mut game);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET);
    assert!(
        last(&st).contains("aren't saved (you changed the file since)"),
        "{}",
        last(&st)
    );
    // The player saved the agent's edit (Ctrl+S), then typed: what isn't
    // saved is theirs, and the turn's end says nothing of the agent's.
    let (mut game, mut st) = editing("coach-turn-saved", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    fake_claude(&mut st, "saved", &[INIT], GOOD);
    agent_ask(&mut st, "fix it");
    ask(&mut st, &mut game, "edit", edit.clone()).unwrap();
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    st.s.editor.click(2, 8, false);
    handle(&mut st, &mut game, Input::Text(" + 1".into()));
    let before = st.s.ui.agent.lines.len();
    until_done(&mut st, &mut game);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "local n = 0\nprint('half done')\nreturn n\n"
    );
    assert!(st.s.editor.dirty, "the player's own edit stays theirs");
    assert!(coach::summary(&st)["unsaved"].is_null());
    let since = said(&st, before);
    assert!(!since.iter().any(|(_, t)| t.contains("aren't saved")), "{since:#?}");
    // The file changed on disk meanwhile: it isn't written over.
    let (mut game, mut st) = editing("coach-turn-disk", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    fake_claude(&mut st, "disk", &[INIT], GOOD);
    agent_ask(&mut st, "fix it");
    ask(&mut st, &mut game, "edit", edit.clone()).unwrap();
    std::fs::write(&path, "print('written elsewhere, and longer')\n").unwrap();
    until_done(&mut st, &mut game);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "print('written elsewhere, and longer')\n"
    );
    assert!(
        last(&st).contains("aren't saved (the file changed on disk since)"),
        "{}",
        last(&st)
    );
    // Another agent's edits are its own to save.
    let (mut game, mut st) = editing("coach-turn-other", "greet", GREET);
    let path = st.s.editor.path.clone().unwrap();
    fake_claude(&mut st, "other", &[INIT], GOOD);
    agent_ask(&mut st, "fix it");
    let cursor = By {
        name: "Cursor".into(),
        conn: 2,
    };
    coach::call(&mut st, &mut game, &cursor, "edit", &edit).unwrap();
    until_done(&mut st, &mut game);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), GREET);
    assert!(st.s.editor.dirty);
    assert_eq!(actions(&st), ["Cursor: Changed line 2 in greet.lua"]);
    let unsaved = &coach::summary(&st)["unsaved"];
    assert_eq!(
        (&unsaved["by"], &unsaved["conn"]),
        (&json!("Cursor"), &json!(2)),
        "who made them"
    );
}

// ----------------------------------------------------------------- marks

/// Every place a text was drawn.
fn texts_at(game: &FakeGame, text: &str) -> Vec<(f32, f32)> {
    game.drawn
        .iter()
        .filter(|d| d.starts_with(&format!("text {text} @")))
        .map(|d| {
            let at: Vec<f32> = d
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
        })
        .collect()
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// The rows (from 0) of the marks in the editor's file, in order made.
fn marked(st: &Studio) -> Vec<(usize, usize)> {
    let path = st.s.editor.path.clone();
    st.s.ui
        .coach
        .marks
        .iter()
        .filter(|m| Some(&m.path) == path.as_ref())
        .map(|m| (m.first, m.last))
        .collect()
}

const TEN: &str = "local a = 1\nlocal b = 2\nlocal total = a + b\nprint(total)\nlocal c = 3\nlocal d = 4\nlocal e = 5\nlocal f = 6\nlocal g = 7\nreturn total\n";

#[test]
fn marks_follow_their_lines_as_the_code_changes() {
    let (mut game, mut st) = editing("coach-follow", "sum", TEN);
    let r = ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 3, "end_line": 4, "note": "This adds them up." }),
    )
    .unwrap();
    assert_eq!(r["did"], "Highlighted lines 3-4 in sum.lua", "{r}");
    assert_eq!(r["lines"], json!([3, 4]));
    assert_eq!(marked(&st), [(2, 3)]);
    // The player puts two lines in at the top: it moves down with its lines.
    st.s.editor.click(0, 0, false);
    handle(&mut st, &mut game, Input::Text("-- one".into()));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    handle(&mut st, &mut game, Input::Text("-- two".into()));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(4, 5)]);
    assert_eq!(st.s.editor.lines[4], "local total = a + b");
    // They change a marked line itself: it stays.
    st.s.editor.click(4, "local total".len(), false);
    handle(&mut st, &mut game, Input::Text("_sum".into()));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(4, 5)]);
    // A line put in inside it: it grows; taken out, it shrinks.
    st.s.editor.click(4, st.s.editor.lines[4].len(), false);
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    handle(&mut st, &mut game, Input::Text("local x = 0".into()));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(4, 6)]);
    // (Two undo steps: the line typed, then the line break.)
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(4, 5)], "undo takes it back too");
    // An agent's edit above it moves it by exactly the lines it put in.
    st.s.ui.coach.player_at = f64::NEG_INFINITY;
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 1, "end_line": 0, "new_text": "-- a\n-- b\n-- c", "typing": false }),
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(7, 8)]);
    // Typed in as the player watches, too.
    let r = ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "start_line": 1, "end_line": 0, "new_text": "local first = true" }),
    )
    .unwrap();
    typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
    assert_eq!(marked(&st), [(8, 9)]);
    assert_eq!(st.s.editor.lines[8], "local total_sum = a + b");
    // Everything taken out: it stays inside the file.
    st.s.editor.select_all();
    handle(&mut st, &mut game, key(Key::Backspace, NONE));
    handle(&mut st, &mut game, Input::Text("print(1)".into()));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(0, 0)]);
}

#[test]
fn a_file_opened_again_or_moved_keeps_its_marks() {
    let (mut game, mut st) = editing("coach-reopen", "sum", TEN);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 4, "note": "Printed here." }),
    )
    .unwrap();
    // Another file opens; sum.lua changes on disk meanwhile.
    let scratch = st.scratch_path();
    st.open_file(&scratch);
    run_for(&mut st, &mut game, 0.05);
    let path = game.mods_dir().join("sum.lua");
    std::fs::write(&path, format!("-- new\n-- lines\n{TEN}")).unwrap();
    st.open_file(&path);
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(5, 5)], "found where its line is now");
    // An agent's change to a file the editor doesn't show moves its marks too.
    st.open_file(&scratch);
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "id": "sum", "start_line": 1, "end_line": 2, "new_text": "" }),
    )
    .unwrap();
    let m = &st.s.ui.coach.marks[0];
    assert_eq!((m.first, m.last), (3, 3));
    // A file of a module renamed: its marks go along.
    let (mut game, mut st) = with_module("coach-moved");
    let util = game.mods_dir().join("kit/lib/util.lua");
    st.open_file(&util);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 1, "note": "The helper." }),
    )
    .unwrap();
    let to = st.rename_file(&mut game, &util, "lib/helpers").unwrap();
    run_for(&mut st, &mut game, 0.05);
    let m = &st.s.ui.coach.marks[0];
    assert_eq!((&m.path, m.file.as_str()), (&to, "kit/lib/helpers.lua"));
    assert_eq!(marked(&st), [(0, 0)]);
    // The script open made a module elsewhere (write_mod), its text changed
    // too: the scan takes the editor and its marks along, and they follow
    // the new text.
    let (mut game, mut st) = editing("coach-moved-changed", "greet", GREET);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 2, "note": "It says hi." }),
    )
    .unwrap();
    let mods = game.mods_dir();
    std::fs::create_dir_all(mods.join("greet")).unwrap();
    std::fs::write(mods.join("greet/main.lua"), format!("-- a module now\n{GREET}")).unwrap();
    std::fs::remove_file(mods.join("greet.lua")).unwrap();
    st.scan(&mut game);
    redraw(&mut st, &mut game);
    assert_eq!(st.s.editor.title, "greet/main.lua");
    let m = &st.s.ui.coach.marks[0];
    assert_eq!(
        (&m.path, m.file.as_str()),
        (&mods.join("greet/main.lua"), "greet/main.lua")
    );
    assert_eq!(marked(&st), [(2, 2)], "on its line in the new text");
}

#[test]
fn a_file_deleted_takes_its_marks_and_one_renamed_to_its_name_has_none() {
    let (mut game, mut st) = with_module("coach-gone-marks");
    let kit = game.mods_dir().join("kit");
    let long: String = (1..=20).map(|i| format!("local v{i} = {i}\n")).collect::<String>() + "return {}\n";
    let short = "return {\n  two = 2,\n}\n";
    for (name, text) in [
        ("a.lua", long.as_str()),
        ("b.lua", short),
        ("c.lua", long.as_str()),
        ("d.lua", short),
    ] {
        std::fs::write(kit.join(name), text).unwrap();
    }
    st.scan(&mut game);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    // Lines 6-7 of a.lua marked, its note the open one; then b.lua opens.
    let mark = json!({ "start_line": 6, "end_line": 7, "note": "These lines are fine." });
    st.open_file(&kit.join("a.lua"));
    redraw(&mut st, &mut game);
    ask(&mut st, &mut game, "mark", mark.clone()).unwrap();
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "local v1 = 1", "new_text": "local v1 = 10", "typing": false }),
    )
    .unwrap();
    st.save_editor(&mut game);
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "local v1 = 10", "new_text": "local v1 = 11", "typing": false }),
    )
    .unwrap();
    assert!(st.s.ui.coach.unsaved.is_some());
    // a.lua deleted from the script list: its mark, and the record of the
    // agent's edits to it, go with it (the edits themselves were saved first).
    st.delete_path(&mut game, &kit.join("a.lua")).unwrap();
    assert!(st.s.ui.coach.marks.is_empty() && st.s.ui.coach.unsaved.is_none());
    st.open_file(&kit.join("b.lua"));
    redraw(&mut st, &mut game);
    // b.lua renamed to a.lua: the editor follows it, with nothing of a.lua's.
    st.rename_file(&mut game, &kit.join("b.lua"), "a").unwrap();
    still(&mut st, &mut game);
    assert_eq!(st.s.editor.title, "kit/a.lua");
    assert!(st.s.ui.coach.bubble_at.is_none());
    // c.lua removed another way (write_mod, a file manager): its mark
    // waits. d.lua, open, renamed to its name doesn't take it on, and draws.
    st.open_file(&kit.join("c.lua"));
    redraw(&mut st, &mut game);
    ask(&mut st, &mut game, "mark", mark).unwrap();
    st.open_file(&kit.join("d.lua"));
    redraw(&mut st, &mut game);
    std::fs::remove_file(kit.join("c.lua")).unwrap();
    st.scan(&mut game);
    assert_eq!(st.s.ui.coach.marks.len(), 1, "waiting for c.lua");
    st.rename_file(&mut game, &kit.join("d.lua"), "c").unwrap();
    still(&mut st, &mut game);
    assert_eq!(st.s.editor.title, "kit/c.lua");
    assert!(st.s.ui.coach.marks.is_empty(), "the old c.lua's mark is gone");
}

#[test]
fn marks_steps_and_jumps_follow_a_file_the_studio_moves() {
    // A script the New prompt made (its name line is its id), renamed from
    // its menu while open: the name line changes in the editor, and the
    // mark goes along on its line.
    let mut game = FakeGame::new("coach-studio-moves");
    let mut st = studio(&mut game);
    let greet = st.create_mod(&mut game, "greet", None).unwrap();
    st.open_file(&greet);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    still(&mut st, &mut game);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 6, "note": "It says so." }),
    )
    .unwrap();
    st.rename_mod(&mut game, "greet", "salut").unwrap();
    still(&mut st, &mut game);
    assert_eq!(st.s.editor.lines[0], "-- name: salut");
    let m = &st.s.ui.coach.marks[0];
    assert_eq!(
        (&m.path, m.file.as_str()),
        (&game.mods_dir().join("salut.lua"), "salut.lua")
    );
    assert_eq!(marked(&st), [(5, 5)]);
    // A module's file that isn't open, renamed from the script list: a
    // lesson's step there, a mark and the Agent panel's line about an
    // agent's edit to it all go to its new name.
    let (mut game, mut st) = with_module("coach-studio-moves-files");
    let kit = game.mods_dir().join("kit");
    st.open_file(&kit.join("main.lua"));
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    st.s.ui.look.motion = false;
    let mut lesson = kit_lesson();
    lesson["steps"][1] = lesson["steps"][2].clone();
    lesson["steps"].as_array_mut().unwrap().truncate(2);
    ask(&mut st, &mut game, "lesson", lesson).unwrap();
    let util = json!({ "id": "kit", "file": "lib/util.lua" });
    let mut edit = util.clone();
    edit["old_text"] = json!("return 2");
    edit["new_text"] = json!("return 3");
    ask(&mut st, &mut game, "edit", edit).unwrap();
    // Its line's number (the panel numbers its lines from 1).
    let edited = st.s.ui.agent.lines.len() as u64;
    let mut mark = util.clone();
    mark["start_line"] = json!(1);
    mark["note"] = json!("The helper.");
    ask(&mut st, &mut game, "mark", mark).unwrap();
    assert_eq!(st.s.editor.title, "kit/main.lua");
    let helpers = st
        .rename_file(&mut game, &kit.join("lib/util.lua"), "lib/helpers")
        .unwrap();
    for m in &st.s.ui.coach.marks[1..] {
        assert_eq!((&m.path, m.file.as_str()), (&helpers, "kit/lib/helpers.lua"));
    }
    for jump in st.s.ui.agent.lines.iter().filter_map(|l| l.jump.as_ref()) {
        assert!(jump.path.is_file(), "{jump:?}");
    }
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.editor.title, "kit/lib/helpers.lua", "Next goes to the step");
    assert_eq!(marked(&st), [(0, 0), (0, 0)]);
    st.open_file(&kit.join("main.lua"));
    coach::go(&mut st, &mut game, edited);
    assert_eq!(st.s.editor.title, "kit/lib/helpers.lua", "Show goes to the edit");
    // A script that isn't open made a module: its mark goes to main.lua.
    let mut game = FakeGame::new("coach-studio-moves-module");
    let mut st = studio(&mut game);
    game.write_mod("hello.lua", GREET);
    game.write_mod("other.lua", GREET);
    st.scan(&mut game);
    st.edit_mod(mod_index(&st, "other"));
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "id": "hello", "start_line": 2, "note": "It says hi." }),
    )
    .unwrap();
    st.make_module(&mut game, mod_index(&st, "hello")).unwrap();
    let m = &st.s.ui.coach.marks[0];
    let main = game.mods_dir().join("hello/main.lua");
    assert_eq!((&m.path, m.file.as_str()), (&main, "hello/main.lua"));
    // A module renamed: the marks in its files, open or not, go along.
    let (mut game, mut st) = with_module("coach-studio-moves-folder");
    let kit = game.mods_dir().join("kit");
    st.open_file(&kit.join("main.lua"));
    for file in ["main.lua", "lib/util.lua"] {
        let at = json!({ "id": "kit", "file": file, "start_line": 1, "note": "Here." });
        ask(&mut st, &mut game, "mark", at).unwrap();
    }
    st.rename_mod(&mut game, "kit", "tools").unwrap();
    let tools = game.mods_dir().join("tools");
    let moved: Vec<_> =
        st.s.ui
            .coach
            .marks
            .iter()
            .map(|m| (m.path.clone(), m.file.clone()))
            .collect();
    assert_eq!(
        moved,
        [
            (tools.join("main.lua"), "tools/main.lua".to_owned()),
            (tools.join("lib/util.lua"), "tools/lib/util.lua".to_owned())
        ]
    );
    still(&mut st, &mut game);
    assert_eq!(marked(&st), [(0, 0)]);
}

#[test]
fn marks_are_cleared_by_number_by_file_or_all() {
    let (mut game, mut st) = editing("coach-unmark", "sum", TEN);
    game.write_mod("other.lua", "print('other')\n");
    st.scan(&mut game);
    let one = ask(&mut st, &mut game, "mark", json!({ "start_line": 1, "note": "One." })).unwrap();
    ask(&mut st, &mut game, "mark", json!({ "start_line": 2, "note": "Two." })).unwrap();
    let r = ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "id": "other", "start_line": 1, "note": "Elsewhere." }),
    )
    .unwrap();
    assert_eq!(r["open"], false);
    assert!(r["did"].as_str().unwrap().contains("once the player opens it"), "{r}");
    assert_eq!(st.s.editor.title, "sum.lua", "marking never switches the file");
    // One, by its number; again, it isn't there.
    let r = ask(&mut st, &mut game, "unmark", json!({ "mark": one["mark"] })).unwrap();
    assert_eq!(
        (&r["cleared"], &r["did"]),
        (&json!(1), &json!("Cleared the note on line 1 of sum.lua"))
    );
    let e = ask(&mut st, &mut game, "unmark", json!({ "mark": one["mark"] })).unwrap_err();
    assert!(e.contains("no mark"), "{e}");
    // A file's, then all.
    let r = ask(&mut st, &mut game, "unmark", json!({ "id": "sum" })).unwrap();
    assert_eq!(
        (&r["cleared"], &r["did"]),
        (&json!(1), &json!("Cleared 1 note in sum.lua"))
    );
    assert_eq!(st.s.ui.coach.marks.len(), 1);
    let r = ask(&mut st, &mut game, "unmark", json!({})).unwrap();
    assert_eq!(r["cleared"], 1);
    assert!(st.s.ui.coach.marks.is_empty());
    // At most 20: the oldest goes.
    for n in 1..=21 {
        ask(
            &mut st,
            &mut game,
            "mark",
            json!({ "start_line": 1 + n % 10, "note": format!("Note {n}."), "reveal": false }),
        )
        .unwrap();
    }
    let c = &st.s.ui.coach;
    assert_eq!(c.marks.len(), 20);
    assert_eq!(c.marks[0].first, 2, "the first one went");
    // replace clears the agent's own marks, not another's.
    let cursor = By {
        name: "Cursor".into(),
        conn: 2,
    };
    coach::call(
        &mut st,
        &mut game,
        &cursor,
        "mark",
        &json!({ "start_line": 5, "note": "Mine." }),
    )
    .unwrap();
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 6, "note": "Only this.", "replace": true }),
    )
    .unwrap();
    let by: Vec<&str> = st.s.ui.coach.marks.iter().map(|m| m.by.name.as_str()).collect();
    assert_eq!(by, ["Cursor", "Claude Code"]);
    // What isn't a place to mark says so, and so does a mark with nothing to
    // say (the player could never close it: its × is on its note).
    for (params, says) in [
        (json!({ "start_line": 11 }), "has 10 lines"),
        (json!({ "start_line": 4, "end_line": 3 }), "before start_line"),
        (json!({ "start_line": 4, "columns": [20, 30] }), "from 1 to 12"),
        (json!({ "start_line": 4, "columns": 3 }), "[first, last]"),
        (
            json!({ "start_line": 4, "tone": "loud" }),
            "note, error, warning or good",
        ),
        (json!({ "note": "where?" }), "start_line"),
        (json!({ "start_line": 4 }), "note is missing"),
        (json!({ "start_line": 4, "note": " \u{7} " }), "note is missing"),
    ] {
        let e = ask(&mut st, &mut game, "mark", params.clone()).unwrap_err();
        assert!(e.contains(says), "{params}: {e}");
    }
    // A note is cleaned and cut.
    let r = ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 4, "columns": [1, 5], "note": format!("a\u{7}b\tc\n{}", "x".repeat(600)) }),
    )
    .unwrap();
    let m =
        st.s.ui
            .coach
            .marks
            .iter()
            .find(|m| m.id as u64 == r["mark"].as_u64().unwrap())
            .unwrap();
    assert!(
        m.note.starts_with("ab c\nxxx") && m.note.chars().count() == 501,
        "{:?}",
        &m.note[..10]
    );
    assert_eq!(m.span, Some((0, 5)), "print, underlined");
    // read_script lists a file's marks, and game_status counts them.
    let read = ask(&mut st, &mut game, "editor", json!({ "from": 1, "to": 2 })).unwrap();
    assert_eq!(read["marks"].as_array().unwrap().len(), 3);
    assert_eq!(read["marks"][2]["lines"], json!([4, 4]));
    assert_eq!(coach::summary(&st)["marks"], 3);
}

/// A draw at Motion off: everything is where it goes at once.
fn still(st: &mut Studio, game: &mut FakeGame) {
    st.s.ui.look.motion = false;
    redraw(st, game);
    redraw(st, game);
}

#[test]
fn a_mark_shows_its_note_and_closes_with_its_cross() {
    use horizon_studio::game::hstudio_ui::UiResult;
    let (mut game, mut st) = editing("coach-bubble", "sum", TEN);
    st.s.editor.click(0, 0, false);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 3, "end_line": 4, "note": "This line never changes i.", "tone": "error" }),
    )
    .unwrap();
    still(&mut st, &mut game);
    let code = st.s.ui.coach.code_at.unwrap();
    let bubble = st.s.ui.coach.bubble_at.expect("its note shows in a bubble");
    let note = text_at(&game, "This line never changes i.");
    assert!(inside(bubble, note) && inside(code, note), "{bubble:?} {note:?}");
    // Under its lines, inside the code's panel; a red band behind them.
    let (_, line4) = text_at(&game, "print");
    assert!(bubble[1] > line4, "under line 4");
    assert!(bubble[0] >= code[0] && bubble[0] + bubble[2] <= code[0] + code[2]);
    assert!(bubble[1] >= code[1] && bubble[1] + bubble[3] <= code[1] + code[3]);
    let danger = format!("{:08x}", horizon_studio::game::hstudio_theme::THEMES[0].palette.danger);
    let bar = format!(", 3.0, {:.1}] r0 ", horizon_studio::game::hstudio_ui::code_line());
    let bars = game
        .drawn
        .iter()
        .filter(|d| d.starts_with("rect") && d.contains(&bar) && d.ends_with(&danger[2..]))
        .count();
    assert_eq!(bars, 2, "its tone's bar in the gutter, on both lines");
    assert_eq!((st.s.editor.row, st.s.editor.col), (0, 0), "the cursor stays");
    // Its × clears it.
    let cross = texts_at(&game, "×").into_iter().find(|at| inside(bubble, *at)).unwrap();
    click(&mut st, &mut game, (cross.0 + 2.0, cross.1 + 4.0));
    assert!(st.s.ui.coach.marks.is_empty());
    still(&mut st, &mut game);
    assert!(st.s.ui.coach.bubble_at.is_none());
    // Esc closes a note, and the next Esc the studio.
    ask(&mut st, &mut game, "mark", json!({ "start_line": 2, "note": "Two." })).unwrap();
    still(&mut st, &mut game);
    assert!(st.s.ui.coach.bubble_at.is_some());
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Used);
    assert_eq!(st.s.ui.coach.open_mark, None);
    assert_eq!(st.s.ui.coach.marks.len(), 1, "closed, not cleared");
    still(&mut st, &mut game);
    assert_eq!(handle(&mut st, &mut game, key(Key::Escape, NONE)), UiResult::Close);
}

#[test]
fn several_marks_show_their_notes_at_the_line_end_and_the_chip_cycles() {
    let (mut game, mut st) = editing("coach-chip", "sum", TEN);
    for (line, note) in [(1, "First note."), (5, "Second note."), (8, "Third note.")] {
        ask(&mut st, &mut game, "mark", json!({ "start_line": line, "note": note })).unwrap();
    }
    still(&mut st, &mut game);
    // The newest in a bubble, the others at the end of their line.
    let bubble = st.s.ui.coach.bubble_at.unwrap();
    assert!(inside(bubble, text_at(&game, "Third note.")));
    let first = text_at(&game, "First note.");
    // (Code is drawn in pieces, by colour: line 1's first is "local".)
    let (_, row1) = text_at(&game, "local");
    assert!(
        !inside(bubble, first) && (first.1 - row1).abs() < 4.0,
        "at line 1's end"
    );
    let second = text_at(&game, "Second note.");
    // A click on one opens it.
    click(&mut st, &mut game, (second.0 + 2.0, second.1 + 4.0));
    let second_id = st.s.ui.coach.marks[1].id;
    assert_eq!(st.s.ui.coach.open_mark, Some(second_id));
    still(&mut st, &mut game);
    assert!(inside(st.s.ui.coach.bubble_at.unwrap(), text_at(&game, "Second note.")));
    // The header's chip counts them and goes to the next.
    let chip = text_at(&game, "3 notes");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 4.0));
    assert_eq!(st.s.ui.coach.open_mark, Some(st.s.ui.coach.marks[2].id));
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 4.0));
    assert_eq!(st.s.ui.coach.open_mark, Some(st.s.ui.coach.marks[0].id), "round again");
    // Its × clears them all.
    still(&mut st, &mut game);
    let cross = texts_at(&game, "×")
        .into_iter()
        .find(|at| (at.1 - chip.1).abs() < 6.0 && at.0 > chip.0)
        .unwrap();
    click(&mut st, &mut game, (cross.0 + 2.0, cross.1 + 4.0));
    assert!(st.s.ui.coach.marks.is_empty());
    still(&mut st, &mut game);
    assert!(texts_at(&game, "3 notes").is_empty());
}

#[test]
fn marking_another_file_never_switches_the_players_and_show_goes_there() {
    use horizon_studio::game::hstudio_ui::Panel;
    let (mut game, mut st) = editing("coach-mark-elsewhere", "sum", TEN);
    let scratch = st.scratch_path();
    st.open_file(&scratch);
    // The player is typing in the console.
    st.s.ui.tab = Tab::Console;
    handle(&mut st, &mut game, Input::Text("print(".into()));
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "id": "sum", "start_line": 3, "end_line": 4, "note": "Here.", "reveal": false }),
    )
    .unwrap();
    assert_eq!((st.s.ui.tab, st.s.editor.title.as_str()), (Tab::Console, "scratch.lua"));
    handle(&mut st, &mut game, Input::Text("1)".into()));
    assert_eq!(st.s.ui.input.text, "print(1)", "their keys stay theirs");
    // In the Editor tab a note over the code says so; the panel's line goes there.
    st.s.ui.tab = Tab::Editor;
    st.s.ui.panel = Panel::Agent;
    st.s.ui.panel_hidden = false;
    st.s.ui.panel_focus = true;
    still(&mut st, &mut game);
    text_at(&game, "Claude Code marked lines 3-4 in sum.lua · Show");
    let line = text_at(&game, "Claude Code: Highlighted lines 3-4 in sum.lua");
    let show = texts_at(&game, "Show")
        .into_iter()
        .find(|at| (at.1 - line.1).abs() < 4.0)
        .unwrap();
    click(&mut st, &mut game, (show.0 + 4.0, show.1 + 4.0));
    assert_eq!(st.s.editor.title, "sum.lua");
    assert_eq!((st.s.editor.row, st.s.editor.col), (2, 0));
    assert!(!st.s.ui.panel_focus);
    assert_eq!(
        st.s.ui.coach.open_mark,
        Some(st.s.ui.coach.marks[0].id),
        "its note opens"
    );
    still(&mut st, &mut game);
    assert!(inside(st.s.ui.coach.bubble_at.unwrap(), text_at(&game, "Here.")));
    // With the studio closed, a toast says what the agent did.
    st.s.ui.open = false;
    ask(&mut st, &mut game, "mark", json!({ "start_line": 1, "note": "Look." })).unwrap();
    assert_eq!(
        st.s.toast.as_ref().map(|t| t.0.as_str()),
        Some("Claude Code highlighted line 1 in sum.lua")
    );
}

#[test]
fn every_style_draws_a_note_inside_its_code() {
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    let (mut game, mut st) = editing("coach-styles", "sum", TEN);
    let long = "This note is long enough to wrap over several lines in a small window. ".repeat(6);
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 2, "end_line": 3, "note": long }),
    )
    .unwrap();
    for i in 0..THEMES.len() {
        for rect in [Some([10.0, 10.0, 330.0, 230.0]), None] {
            st.s.ui.look = Look {
                theme: i,
                rect,
                motion: false,
                ..Look::default()
            };
            st.s.ui.tab = Tab::Editor;
            st.s.editor.scroll = 0;
            redraw(&mut st, &mut game);
            redraw(&mut st, &mut game);
            let c = &st.s.ui.coach;
            let (code, bubble) = (c.code_at.unwrap(), c.bubble_at);
            if let Some(b) = bubble {
                assert!(
                    b[0] >= code[0]
                        && b[1] >= code[1]
                        && b[0] + b[2] <= code[0] + code[2]
                        && b[1] + b[3] <= code[1] + code[3],
                    "theme {i} {rect:?}: {b:?} in {code:?}"
                );
                assert!(b[3] <= code[3] - 2.0 * horizon_studio::game::hstudio_ui::code_line() + 0.5 || b[3] <= 37.0);
            } else {
                assert!(code[3] < 80.0, "theme {i} {rect:?}: no bubble in {code:?}");
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn a_mark_has_one_line_in_the_agent_panel() {
    use horizon_studio::game::hstudio_ui::agent_ask;
    let (mut game, mut st) = editing("coach-mark-line", "greet", GREET);
    let call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"m1","name":"mcp__horizon-studio__mark_lines","input":{"id":"greet","start_line":2}}]},"session_id":"s1"}"#;
    let ok = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"m1","content":[{"type":"text","text":"{\"ok\": true, \"mark\": 1, \"did\": \"Highlighted line 2 in greet.lua\"}"}]}]},"session_id":"s1"}"#;
    let clear = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"m2","name":"mcp__horizon-studio__clear_marks","input":{}}]},"session_id":"s1"}"#;
    let cleared = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"m2","content":[{"type":"text","text":"{\"ok\": true, \"cleared\": 1}"}]}]},"session_id":"s1"}"#;
    fake_claude(&mut st, "mark", &[INIT, call, ok, clear, cleared], GOOD);
    agent_ask(&mut st, "what is wrong?");
    ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "start_line": 2, "note": "This prints, it doesn't count." }),
    )
    .unwrap();
    until_done(&mut st, &mut game);
    let lines: Vec<(AgentLineKind, String)> = st.s.ui.agent.lines.iter().map(|l| (l.kind, l.text.clone())).collect();
    assert!(
        lines.contains(&(AgentLineKind::Action, "Highlighted line 2 in greet.lua".into())),
        "{lines:#?}"
    );
    assert!(
        !lines
            .iter()
            .any(|(k, t)| t.starts_with("mark_lines") || t.starts_with("clear_marks") || *k == AgentLineKind::ToolOk),
        "{lines:#?}"
    );
    let jump = st.s.ui.agent.lines.iter().find_map(|l| l.jump.clone()).unwrap();
    assert_eq!(jump.mark, Some(st.s.ui.coach.marks[0].id), "its Show opens its note");
}

// ----------------------------------------------------------------- lessons

const ALT: Mods = Mods {
    ctrl: false,
    shift: false,
    alt: true,
};

/// The Agent panel's lines, with their kinds, from the `from`th on.
fn said(st: &Studio, from: usize) -> Vec<(AgentLineKind, String)> {
    st.s.ui.agent.lines[from..]
        .iter()
        .map(|l| (l.kind, l.text.clone()))
        .collect()
}

/// A three-step lesson over kit/: two steps in main.lua, one in lib/util.lua.
fn kit_lesson() -> Json {
    json!({
        "title": "Modules",
        "id": "kit",
        "steps": [
            { "start_line": 1, "note": "require loads lib/util.lua once.", "explain": "A module's files load with require." },
            { "start_line": 2, "note": "This calls the helper.", "task": "Make it print 3.", "explain": "util is the table lib/util.lua returns." },
            { "start_line": 1, "file": "lib/util.lua", "note": "The helper itself.", "explain": "It returns a table of functions." },
        ],
    })
}

#[test]
fn a_lesson_steps_through_its_notes_once_each_in_the_panel() {
    let (mut game, mut st) = with_module("coach-lesson");
    let main = game.mods_dir().join("kit/main.lua");
    st.open_file(&main);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    redraw(&mut st, &mut game);
    let r = ask(&mut st, &mut game, "lesson", kit_lesson()).unwrap();
    assert_eq!(r["did"], "Lesson \"Modules\": 3 steps in 2 files", "{r}");
    assert_eq!(r["marks"].as_array().unwrap().len(), 3);
    let marks: Vec<u32> = r["marks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_u64().unwrap() as u32)
        .collect();
    // Step 1 shows: its note open, its line in the panel, its explanation.
    assert_eq!(st.s.ui.coach.open_mark, Some(marks[0]));
    assert_eq!(
        said(&st, 0),
        [
            (
                AgentLineKind::Action,
                "Claude Code: Lesson \"Modules\": 3 steps in 2 files".to_owned()
            ),
            (AgentLineKind::Action, "Step 1 of 3: line 1 in kit/main.lua".into()),
            (AgentLineKind::Said, "A module's files load with require.".into()),
        ]
    );
    assert_eq!(st.s.ui.panel, horizon_studio::game::hstudio_ui::Panel::Agent);
    // Alt+Right goes on, and a step in another file opens it.
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.ui.coach.open_mark, Some(marks[1]));
    assert_eq!((st.s.editor.row, st.s.editor.col), (0, 0), "the cursor stays");
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.editor.title, "kit/lib/util.lua");
    assert_eq!(st.s.ui.coach.open_mark, Some(marks[2]));
    let n = st.s.ui.agent.lines.len();
    // Back and on again: the steps said once.
    handle(&mut st, &mut game, key(Key::Left, ALT));
    assert_eq!(st.s.editor.title, "kit/main.lua");
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.ui.agent.lines.len(), n, "no explanation twice");
    assert_eq!(
        said(&st, 3)
            .iter()
            .filter(|(k, _)| *k == AgentLineKind::Action)
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>(),
        [
            "Step 2 of 3: line 2 in kit/main.lua",
            "Step 3 of 3: line 1 in kit/lib/util.lua"
        ]
    );
    // Show on step 1's line in the Agent panel goes back to that step.
    // (Lines are numbered as said, from 1: step 1's is the second.)
    assert_eq!(st.s.ui.agent.lines[1].text, "Step 1 of 3: line 1 in kit/main.lua");
    coach::go(&mut st, &mut game, 2);
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 0);
    assert_eq!(st.s.ui.coach.open_mark, Some(marks[0]));
    handle(&mut st, &mut game, key(Key::Right, ALT));
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 2);
    // Next on the last step: the lesson is done and its marks go.
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert!(st.s.ui.coach.lesson.is_none());
    assert!(st.s.ui.coach.marks.is_empty());
    assert_eq!(
        st.s.ui.agent.lines.last().map(|l| l.text.as_str()),
        Some("Lesson finished.")
    );
    // Without a lesson, Alt+Right moves the cursor as Right does.
    st.s.editor.click(0, 0, false);
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.editor.col, 1);
}

#[test]
fn a_lesson_is_stepped_with_its_buttons_and_its_chip() {
    let (mut game, mut st) = with_module("coach-lesson-buttons");
    st.s.ui.agent.agents = Some(Vec::new());
    let main = game.mods_dir().join("kit/main.lua");
    std::fs::write(
        &main,
        "local util = require('lib.util')\nprint(util.two())\nprint('three')\n",
    )
    .unwrap();
    st.scan(&mut game);
    st.open_file(&main);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    let mut lesson = kit_lesson();
    lesson["steps"][2] = json!({ "start_line": 3, "note": "The last line.", "task": "Read it again." });
    ask(&mut st, &mut game, "lesson", lesson).unwrap();
    still(&mut st, &mut game);
    let bubble = st.s.ui.coach.bubble_at.unwrap();
    assert!(inside(bubble, text_at(&game, "require loads lib/util.lua once.")));
    assert!(inside(bubble, text_at(&game, "Step 1 of 3")));
    assert!(texts_at(&game, "Back").is_empty(), "no Back on the first step");
    // The Agent panel shows the explanation, so the bubble doesn't.
    assert!(!inside(bubble, text_at(&game, "A module's files load with require.")));
    let next = text_at(&game, "Next");
    click(&mut st, &mut game, (next.0 + 2.0, next.1 + 4.0));
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 1);
    still(&mut st, &mut game);
    text_at(&game, "Step 2 of 3");
    assert!(inside(
        st.s.ui.coach.bubble_at.unwrap(),
        text_at(&game, "Try: Make it print 3.")
    ));
    // No agent to check with: no Check.
    assert!(texts_at(&game, "Check").is_empty());
    let back = text_at(&game, "Back");
    click(&mut st, &mut game, (back.0 + 2.0, back.1 + 4.0));
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 0);
    // A click on a step's number goes to it.
    still(&mut st, &mut game);
    let three = texts_at(&game, "3")
        .into_iter()
        .find(|at| at.0 < st.s.ui.coach.code_at.unwrap()[0] + 30.0)
        .expect("step 3's number in the gutter");
    click(&mut st, &mut game, (three.0 + 1.0, three.1 + 4.0));
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 2);
    // Esc closes the note (the footer says so while one shows); the
    // header's chip opens it again.
    still(&mut st, &mut game);
    text_at(&game, "close the note");
    handle(&mut st, &mut game, key(Key::Escape, NONE));
    assert_eq!(st.s.ui.coach.open_mark, None);
    still(&mut st, &mut game);
    assert!(
        texts_at(&game, "close the note").is_empty(),
        "no note shows: Esc closes the studio"
    );
    text_at(&game, "next step");
    let chip = text_at(&game, "Lesson 3/3");
    click(&mut st, &mut game, (chip.0 + 2.0, chip.1 + 4.0));
    assert_eq!(
        st.s.ui.coach.open_mark,
        st.s.ui.coach.lesson.as_ref().map(|l| l.steps[2].mark)
    );
    // Done on the last step; the chip's × ends the lesson.
    still(&mut st, &mut game);
    text_at(&game, "Done");
    let cross = texts_at(&game, "×")
        .into_iter()
        .find(|at| (at.1 - chip.1).abs() < 6.0 && at.0 > chip.0)
        .unwrap();
    click(&mut st, &mut game, (cross.0 + 2.0, cross.1 + 4.0));
    assert!(st.s.ui.coach.lesson.is_none() && st.s.ui.coach.marks.is_empty());
    assert_eq!(st.s.ui.agent.lines.last().unwrap().text, "Lesson closed.");
}

#[test]
fn the_players_lesson_events_reach_a_waiting_agent() {
    use horizon_studio::game::hstudio_ui::Panel;
    let (mut game, mut st) = editing("coach-events", "greet", GREET);
    st.s.ui.agent.agents = Some(Vec::new());
    let r = ask(
        &mut st,
        &mut game,
        "lesson",
        json!({ "title": "Printing", "steps": [
            { "start_line": 2, "note": "print shows text in the console.", "task": "Print hello instead." },
            { "start_line": 3, "note": "return ends the script." },
        ] }),
    )
    .unwrap();
    assert_eq!(r["seq"], 0);
    // Nothing yet; asking makes the agent one that waits.
    let none = ask(&mut st, &mut game, "player_event", json!({})).unwrap();
    assert_eq!(none["event"], "none");
    assert!(coach::waiting(&st));
    assert_eq!(
        coach::waiting_label(&st).as_deref(),
        Some("Claude Code is waiting for you")
    );
    // Next, Back, Check (someone waits: Check shows), a save, the lesson's ×.
    handle(&mut st, &mut game, key(Key::Right, ALT));
    handle(&mut st, &mut game, key(Key::Left, ALT));
    st.s.editor.click(1, "print('".len(), false);
    handle(&mut st, &mut game, Input::Text("oh ".into()));
    ask(&mut st, &mut game, "player_event", json!({ "after": 99 })).unwrap();
    still(&mut st, &mut game);
    let check = text_at(&game, "Check");
    click(&mut st, &mut game, (check.0 + 2.0, check.1 + 4.0));
    handle(&mut st, &mut game, key(Key::Char('s'), CTRL));
    still(&mut st, &mut game);
    let bubble = st.s.ui.coach.bubble_at.unwrap();
    let cross = texts_at(&game, "×").into_iter().find(|at| inside(bubble, *at)).unwrap();
    click(&mut st, &mut game, (cross.0 + 2.0, cross.1 + 4.0));
    assert!(st.s.ui.coach.lesson.is_none());
    // They come oldest first, numbered, each once.
    let mut got = Vec::new();
    loop {
        let e = ask(&mut st, &mut game, "player_event", json!({})).unwrap();
        if e["event"] == "none" {
            break;
        }
        got.push(e);
    }
    let kinds: Vec<&str> = got.iter().map(|e| e["event"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["next", "back", "check", "saved", "closed"]);
    assert_eq!((&got[0]["step"], &got[0]["of"]), (&json!(2), &json!(2)));
    assert_eq!(got[0]["did"], "The player went on to step 2 of 2 (line 3 of greet.lua)");
    assert_eq!(got[1]["step"], 1);
    assert_eq!(
        (&got[2]["task"], &got[2]["text"], &got[2]["dirty"]),
        (&json!("Print hello instead."), &json!("print('oh hi')"), &json!(true))
    );
    assert!(got[2]["problems"].is_array());
    assert_eq!(got[3]["mod"]["state"], "running");
    assert_eq!(got[4]["step"], 1);
    let seqs: Vec<u64> = got.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, [1, 2, 3, 4, 5]);
    // after gives them again from there (an answer lost on the way is asked for again).
    let again = ask(&mut st, &mut game, "player_event", json!({ "after": 2 })).unwrap();
    assert_eq!(again["event"], "check");
    // Each answer names this run of the game: an after counted in another
    // (the game started again since) gets this run's events from the first.
    let run = got[0]["run"].as_u64().expect("its run");
    assert!(got.iter().all(|e| e["run"] == run) && r["run"] == run);
    let e = ask(
        &mut st,
        &mut game,
        "player_event",
        json!({ "after": 4, "run": run + 1 }),
    )
    .unwrap();
    assert_eq!((&e["event"], &e["seq"]), (&json!("next"), &json!(1)), "{e}");
    let e = ask(&mut st, &mut game, "player_event", json!({ "after": 4, "run": run })).unwrap();
    assert_eq!(e["event"], "closed", "{e}");
    // Check went to the agent: the panel says so.
    assert!(
        st.s.ui
            .agent
            .lines
            .iter()
            .any(|l| l.kind == AgentLineKind::You && l.text == "Check step 1: Print hello instead.")
    );
    // What the player writes in the Agent panel goes to the agent waiting.
    st.s.ui.panel = Panel::Agent;
    st.s.ui.panel_hidden = false;
    st.s.ui.panel_focus = true;
    still(&mut st, &mut game);
    text_at(&game, "Claude Code is waiting for you");
    handle(&mut st, &mut game, Input::Text("why?".into()));
    handle(&mut st, &mut game, key(Key::Enter, NONE));
    let e = ask(&mut st, &mut game, "player_event", json!({})).unwrap();
    assert_eq!((&e["event"], &e["text"]), (&json!("message"), &json!("why?")));
    assert!(
        !horizon_studio::game::hstudio_ui::agent_busy(&st),
        "no run of the game's own agent"
    );
    assert_eq!(st.s.ui.agent.lines.last().unwrap().text, "why?");
    // Nobody waiting any more: the steps the player goes to go stale after
    // 30 s, everything after five minutes, and at most 16 are kept.
    run_for(&mut st, &mut game, 4.0);
    assert!(!coach::waiting(&st));
    ask(
        &mut st,
        &mut game,
        "lesson",
        json!({ "title": "Again", "steps": [{ "start_line": 1, "note": "One." }, { "start_line": 2, "note": "Two." }] }),
    )
    .unwrap();
    // (While the Agent panel's input has the keys, Alt+arrows are its own.)
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.ui.coach.lesson.as_ref().unwrap().at, 0);
    st.s.ui.panel_focus = false;
    for _ in 0..10 {
        handle(&mut st, &mut game, key(Key::Right, ALT));
        handle(&mut st, &mut game, key(Key::Left, ALT));
    }
    let e = ask(&mut st, &mut game, "player_event", json!({ "after": 6 })).unwrap();
    assert_eq!((&e["event"], &e["seq"]), (&json!("next"), &json!(11)), "the newest 16");
    run_for(&mut st, &mut game, 31.0);
    let e = ask(&mut st, &mut game, "player_event", json!({ "after": 6 })).unwrap();
    assert_eq!(e["event"], "none", "{e}");
    handle(&mut st, &mut game, key(Key::Right, ALT));
    handle(&mut st, &mut game, key(Key::Right, ALT));
    let e = ask(&mut st, &mut game, "player_event", json!({})).unwrap();
    assert_eq!(e["event"], "next");
}

#[test]
fn a_lesson_opens_its_file_only_when_it_may_and_says_what_is_wrong() {
    let (mut game, mut st) = with_module("coach-lesson-open");
    let hello = game.mods_dir().join("hello.lua");
    std::fs::write(&hello, "print('hello')\n").unwrap();
    st.scan(&mut game);
    st.open_file(&hello);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    // The player is using the studio: it waits.
    st.s.ui.coach.input_at = st.s.now;
    let r = ask(&mut st, &mut game, "lesson", kit_lesson()).unwrap();
    assert_eq!(r["busy"], true, "{r}");
    assert!(st.s.ui.coach.lesson.is_none());
    // Unsaved edits in the file open: it says so, and opens nothing.
    st.s.ui.coach.input_at = f64::NEG_INFINITY;
    st.s.editor.insert("-- mine\n");
    let e = ask(&mut st, &mut game, "lesson", kit_lesson()).unwrap_err();
    assert!(e.contains("the player has unsaved edits in hello.lua"), "{e}");
    assert_eq!(st.s.editor.title, "hello.lua");
    // Saved, it opens the first step's file.
    st.save_editor(&mut game);
    ask(&mut st, &mut game, "lesson", kit_lesson()).unwrap();
    assert_eq!(st.s.editor.title, "kit/main.lua");
    // What isn't a lesson says why.
    let step = json!({ "start_line": 1, "note": "One." });
    for (params, says) in [
        (json!({ "steps": [step] }), "title"),
        (json!({ "title": "T", "steps": [] }), "steps is the lesson's steps"),
        (
            json!({ "title": "T", "steps": vec![step.clone(); 21] }),
            "at most 20 steps",
        ),
        (
            json!({ "title": "T", "steps": [step, { "start_line": 1 }] }),
            "step 2 has no note",
        ),
        (
            json!({ "title": "T", "steps": [step, { "start_line": 9, "note": "Far." }] }),
            "step 2: kit/main.lua has 2 lines",
        ),
        (
            json!({ "title": "T", "steps": [{ "start_line": 1, "note": "N", "file": "nope.lua" }] }),
            "step 1: kit has no file nope.lua",
        ),
        (
            json!({ "title": "T", "steps": [{ "start_line": 1, "note": "N" }], "id": "gone" }),
            "step 1: no script mod 'gone'",
        ),
    ] {
        let e = ask(&mut st, &mut game, "lesson", params.clone()).unwrap_err();
        assert!(e.contains(says), "{params}: {e}");
    }
    // A step's mark goes with the lesson only; clearing everything ends it.
    let id = st.s.ui.coach.lesson.as_ref().unwrap().steps[0].mark;
    let e = ask(&mut st, &mut game, "unmark", json!({ "mark": id })).unwrap_err();
    assert!(e.contains("is step 1 of the lesson"), "{e}");
    let r = ask(&mut st, &mut game, "unmark", json!({ "id": "kit" })).unwrap();
    assert_eq!((&r["cleared"], &r["lesson_ended"]), (&json!(0), &json!(false)));
    assert_eq!(coach::summary(&st)["lesson"]["step"], 1);
    assert_eq!(
        coach::lesson_line(&st).as_deref(),
        Some("A lesson is open in the editor: \"Modules\", step 1 of 3 (kit/main.lua line 1).")
    );
    let r = ask(&mut st, &mut game, "unmark", json!({})).unwrap();
    assert_eq!(
        (&r["lesson_ended"], &r["did"]),
        (&json!(true), &json!("Cleared 3 notes and ended the lesson"))
    );
    assert!(coach::summary(&st)["lesson"].is_null());
}

#[cfg(unix)]
#[test]
fn check_asks_the_games_agent_when_none_is_waiting() {
    use horizon_studio::game::hstudio_agent::AgentSpec;
    let (mut game, mut st) = editing("coach-check", "greet", GREET);
    // A stand-in agent: it says back what it was asked.
    st.s.ui.agent.agents = Some(vec![AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: "/bin/echo".into(),
        args: vec!["{prompt}".into()],
        json: false,
    }]);
    ask(
        &mut st,
        &mut game,
        "lesson",
        json!({ "title": "Printing", "steps": [
            { "start_line": 2, "note": "print shows text.", "task": "Print hello instead." },
        ] }),
    )
    .unwrap();
    st.s.ui.panel_hidden = true;
    still(&mut st, &mut game);
    let check = text_at(&game, "Check");
    click(&mut st, &mut game, (check.0 + 2.0, check.1 + 4.0));
    let asked = "I did step 1 of the lesson \"Printing\" (greet.lua line 2): Print hello instead. Check my code and \
                 tell me how I did.";
    assert!(
        st.s.ui
            .agent
            .lines
            .iter()
            .any(|l| l.kind == AgentLineKind::You && l.text == asked),
        "{:?}",
        st.s.ui.agent.lines.iter().map(|l| &l.text).collect::<Vec<_>>()
    );
    // Its prompt has the lesson where the player is.
    for _ in 0..200 {
        run_for(&mut st, &mut game, 0.05);
        if !horizon_studio::game::hstudio_ui::agent_busy(&st) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let echoed: String =
        st.s.ui
            .agent
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
    assert!(
        echoed.contains("A lesson is open in the editor: \"Printing\", step 1 of 1 (greet.lua line 2)."),
        "{echoed}"
    );
    // With the panel hidden, the explanation is the bubble's: none here; and
    // with no agent at all, no Check.
    st.s.ui.agent.agents = Some(Vec::new());
    still(&mut st, &mut game);
    assert!(texts_at(&game, "Check").is_empty());
}

#[test]
fn every_style_draws_a_lesson_inside_its_code() {
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    let (mut game, mut st) = editing("coach-lesson-styles", "sum", TEN);
    st.s.ui.agent.agents = Some(Vec::new());
    let explain =
        "This explanation is long, as an agent's can be, and goes in the note where no panel shows it. ".repeat(5);
    ask(
        &mut st,
        &mut game,
        "lesson",
        json!({ "title": "Sums", "steps": [
            { "start_line": 3, "note": "Adds a and b.", "task": "Add c too.", "explain": explain },
            { "start_line": 4, "note": "Prints the total." },
        ] }),
    )
    .unwrap();
    st.s.ui.panel_hidden = true;
    let mut drawn = 0;
    for i in 0..THEMES.len() {
        for rect in [Some([10.0, 10.0, 330.0, 230.0]), None] {
            st.s.ui.look = Look {
                theme: i,
                rect,
                motion: false,
                ..Look::default()
            };
            st.s.ui.tab = Tab::Editor;
            st.s.editor.scroll = 0;
            redraw(&mut st, &mut game);
            redraw(&mut st, &mut game);
            let c = &st.s.ui.coach;
            let code = c.code_at.unwrap();
            let Some(b) = c.bubble_at else {
                assert!(code[3] < 90.0, "theme {i} {rect:?}: no bubble in {code:?}");
                continue;
            };
            drawn += 1;
            assert!(
                b[0] >= code[0]
                    && b[1] >= code[1]
                    && b[0] + b[2] <= code[0] + code[2]
                    && b[1] + b[3] <= code[1] + code[3],
                "theme {i} {rect:?}: {b:?} in {code:?}"
            );
            // Its buttons are in it, and with no panel to show it, the explanation.
            let next = text_at(&game, "Next");
            assert!(inside(b, next), "theme {i} {rect:?}");
            if code[3] > 300.0 {
                assert!(inside(b, text_at(&game, "Try: Add c too.")), "theme {i} {rect:?}");
                let start = "This explanation is long";
                let line = game
                    .drawn
                    .iter()
                    .find(|d| d.starts_with(&format!("text {start}")))
                    .unwrap_or_else(|| panic!("theme {i} {rect:?}: no explanation"));
                assert!(!line.is_empty());
            }
        }
    }
    assert!(drawn >= THEMES.len(), "{drawn}");
}

// ----------------------------------------------------------------- reloads typed in

/// Draws a frame a moment later.
fn frame(st: &mut Studio, game: &mut FakeGame) {
    run_for(st, game, 0.05);
    redraw(st, game);
}

/// Draws once the editor looks at its file on disk again (every half
/// second, from the draw).
fn look_at_disk(st: &mut Studio, game: &mut FakeGame) {
    run_for(st, game, 0.55);
    redraw(st, game);
}

#[test]
fn a_change_on_disk_to_the_open_file_is_typed_in() {
    let (mut game, mut st) = editing("coach-reload", "sum", TEN);
    let path = game.mods_dir().join("sum.lua");
    // An agent's write_mod changes the middle of the file open.
    let changed = TEN.replace(
        "local c = 3\nlocal d = 4\n",
        "local c = 30\nlocal d = 40\nlocal extra = true\n",
    );
    std::fs::write(&path, &changed).unwrap();
    look_at_disk(&mut st, &mut game);
    let t = st.s.ui.coach.typing.as_ref().expect("it is typed in");
    assert!(t.reload && t.by.name.is_empty(), "nobody's: no name");
    assert_eq!(t.from, (4, "local c = 3".len()), "only what differs");
    assert!(!st.s.editor.dirty, "no edits of anyone's");
    // It comes in over the next frames, the caret alone (no name flag).
    frame(&mut st, &mut game);
    frame(&mut st, &mut game);
    for _ in 0..10 {
        frame(&mut st, &mut game);
        assert!(!st.s.editor.dirty);
    }
    let mut n = 0;
    while st.s.ui.coach.typing.is_some() {
        n += 1;
        assert!(n < 100, "it never was in");
        frame(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.text(), changed);
    assert!(!st.s.editor.dirty && st.s.editor.reloaded_at.is_some());
    assert!(coach::summary(&st)["typing"].is_null());
    // One undo step goes back to before it.
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    assert_eq!(st.s.editor.text(), TEN);
    st.save_editor(&mut game);
    // A key mid-way puts the rest in at once; the key is the player's.
    std::fs::write(&path, &changed).unwrap();
    look_at_disk(&mut st, &mut game);
    assert!(st.s.ui.coach.typing.is_some());
    st.s.editor.click(0, 0, false);
    handle(&mut st, &mut game, Input::Text("-".into()));
    assert!(st.s.ui.coach.typing.is_none());
    assert_eq!(st.s.editor.text(), format!("-{changed}"));
    assert!(st.s.editor.dirty);
    // With unsaved edits, the editor asks, as before.
    std::fs::write(&path, TEN).unwrap();
    look_at_disk(&mut st, &mut game);
    assert!(st.s.ui.coach.typing.is_none());
    assert!(st.s.editor.conflict);
    handle(&mut st, &mut game, key(Key::Char('z'), CTRL));
    st.s.editor.conflict = false;
    st.save_editor(&mut game);
    // More than 60 lines, or Motion off: read in at once, as before.
    let long: String = (1..=80).map(|i| format!("print({i})\n")).collect();
    std::fs::write(&path, &long).unwrap();
    look_at_disk(&mut st, &mut game);
    assert!(st.s.ui.coach.typing.is_none());
    assert_eq!(st.s.editor.text(), long);
    st.s.ui.look.motion = false;
    std::fs::write(&path, TEN).unwrap();
    look_at_disk(&mut st, &mut game);
    assert!(st.s.ui.coach.typing.is_none());
    assert_eq!(st.s.editor.text(), TEN);
    // A change made while one is still typing in waits for it, then comes in.
    st.s.ui.look.motion = true;
    let more = TEN.replace(
        "local c = 3\n",
        "local c = 3\nlocal one = 'a long line of new code'\nlocal two = 'and another long line'\n",
    );
    std::fs::write(&path, &more).unwrap();
    look_at_disk(&mut st, &mut game);
    let id = st.s.ui.coach.typing.as_ref().map(|t| t.id).expect("typing in");
    std::fs::write(&path, TEN).unwrap();
    look_at_disk(&mut st, &mut game);
    assert_eq!(st.s.ui.coach.typing.as_ref().map(|t| t.id), Some(id), "the same one");
    assert!(!st.s.editor.conflict && !st.s.editor.dirty);
    while st.s.ui.coach.typing.is_some() {
        frame(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.text(), more);
    look_at_disk(&mut st, &mut game);
    while st.s.ui.coach.typing.is_some() {
        frame(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.text(), TEN);
    assert!(!st.s.editor.dirty);
    // A file without a last line break, a line put after it.
    std::fs::write(&path, "print(1)").unwrap();
    look_at_disk(&mut st, &mut game);
    while st.s.ui.coach.typing.is_some() {
        frame(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.text(), "print(1)");
    std::fs::write(&path, "print(1)\nprint(2)").unwrap();
    look_at_disk(&mut st, &mut game);
    assert!(st.s.ui.coach.typing.is_some());
    while st.s.ui.coach.typing.is_some() {
        frame(&mut st, &mut game);
    }
    assert_eq!(st.s.editor.text(), "print(1)\nprint(2)");
}

#[test]
fn an_agents_unsaved_edits_follow_their_file_when_it_is_renamed() {
    let (mut game, mut st) = with_module("coach-unsaved-renamed");
    let util = game.mods_dir().join("kit/lib/util.lua");
    st.open_file(&util);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    ask(
        &mut st,
        &mut game,
        "edit",
        json!({ "old_text": "return 2", "new_text": "return 3", "typing": false }),
    )
    .unwrap();
    assert_eq!(coach::summary(&st)["unsaved"]["file"], "kit/lib/util.lua");
    // The player renames the file (the file has no marks).
    run_for(&mut st, &mut game, 0.05);
    st.rename_file(&mut game, &util, "lib/helpers").unwrap();
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(st.s.editor.title, "kit/lib/helpers.lua");
    let unsaved = &coach::summary(&st)["unsaved"];
    assert_eq!(unsaved["file"], "kit/lib/helpers.lua", "{unsaved}");
    // save_script knows they are the agent's own.
    let r = ask(&mut st, &mut game, "save", json!({})).unwrap();
    assert_eq!(r["saved"], true, "{r}");
    assert!(
        std::fs::read_to_string(game.mods_dir().join("kit/lib/helpers.lua"))
            .unwrap()
            .contains("return 3")
    );
}

#[test]
fn marks_work_in_a_modules_files_and_the_scratch_file() {
    let (mut game, mut st) = with_module("coach-marks-files");
    let scratch = st.scratch_path();
    std::fs::write(&scratch, "print('try')\nprint('more')\n").unwrap();
    st.open_file(&scratch);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    // The scratch file, open.
    let r = ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "scratch": true, "start_line": 2, "note": "This one too." }),
    )
    .unwrap();
    assert_eq!(
        (&r["did"], &r["open"]),
        (&json!("Highlighted line 2 in scratch.lua"), &json!(true))
    );
    assert_eq!(marked(&st), [(1, 1)]);
    // A module's other file, not open: it waits for it.
    let r = ask(
        &mut st,
        &mut game,
        "mark",
        json!({ "id": "kit", "file": "lib/util.lua", "start_line": 1, "note": "The helper." }),
    )
    .unwrap();
    assert_eq!(r["open"], false);
    assert!(
        r["did"]
            .as_str()
            .unwrap()
            .starts_with("Highlighted line 1 in kit/lib/util.lua"),
        "{r}"
    );
    assert_eq!(st.s.editor.title, "scratch.lua");
    // The studio's own rules for a mod's files: nothing outside it.
    for params in [
        json!({ "id": "kit", "file": "../hello.lua", "start_line": 1 }),
        json!({ "id": "kit", "file": "/etc/passwd", "start_line": 1 }),
        json!({ "file": "lib/util.lua", "start_line": 1 }),
    ] {
        let e = ask(&mut st, &mut game, "mark", params.clone()).unwrap_err();
        assert!(!e.is_empty(), "{params}");
    }
    // Opened, it shows its mark; a lesson's steps go across the module's files too.
    let util = game.mods_dir().join("kit/lib/util.lua");
    st.open_file(&util);
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(marked(&st), [(0, 0)]);
    let r = ask(
        &mut st,
        &mut game,
        "lesson",
        json!({ "title": "Kit", "id": "kit", "steps": [
            { "start_line": 1, "file": "lib/util.lua", "note": "The helper." },
            { "start_line": 1, "note": "Its user, in the lesson's own file: main.lua." },
            { "start_line": 1, "scratch": true, "note": "And a try." },
        ] }),
    )
    .unwrap();
    assert_eq!(r["did"], "Lesson \"Kit\": 3 steps in 3 files", "{r}");
    assert_eq!(
        st.s.editor.title, "kit/lib/util.lua",
        "the first step's file, open already"
    );
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.editor.title, "kit/main.lua");
    handle(&mut st, &mut game, key(Key::Right, ALT));
    assert_eq!(st.s.editor.title, "scratch.lua");
}
