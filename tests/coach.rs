// AI agents in the player's editor (game::hstudio_coach): the editor's
// splices that keep the player's place, and what the player does there.

mod common;

use common::{FakeGame, mod_index, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_coach::{self as coach, By};
use horizon_studio::game::hstudio_ui::{Editor, Input, Key, Mods, Tab, handle};
use serde_json::{Value as Json, json};

const NONE: Mods = Mods {
    ctrl: false,
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
