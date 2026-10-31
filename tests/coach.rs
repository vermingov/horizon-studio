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
    assert_eq!(r["saved"], true, "{r}");
    assert_eq!(r["mod"]["state"], "running");
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
    // Every theme draws an edit coming in.
    for i in 0..THEMES.len() {
        st.s.ui.look = Look {
            theme: i,
            rect: Some([10.0, 10.0, 330.0, 230.0]),
            ..Look::default()
        };
        st.s.ui.tab = Tab::Editor;
        redraw(&mut st, &mut game);
        assert!(game.drawn.len() > 10, "theme {i}");
    }
    typed_in(&mut st, &mut game, r["typing"].as_u64().unwrap());
}
