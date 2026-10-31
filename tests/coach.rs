// AI agents in the player's editor (game::hstudio_coach): the editor's
// splices that keep the player's place, and what the player does there.

mod common;

use common::{FakeGame, mod_index, studio};
use horizon_studio::game::hstudio_ui::{Editor, Input, Key, Mods, Tab, handle};

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
    const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
    };
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
    handle(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Left,
            down: true,
            repeat: false,
            mods: NONE,
        },
    );
    assert_eq!(st.s.ui.coach.input_at, 20.0);
    assert_eq!(st.s.ui.coach.player_at, 10.0);
    // Another file opened (the console's edit()) changes the buffer, but not as an edit.
    st.s.now = 30.0;
    st.s.ui.tab = Tab::Console;
    for c in "edit('hello')".chars() {
        handle(&mut st, &mut game, Input::Text(c.to_string()));
    }
    handle(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Enter,
            down: true,
            repeat: false,
            mods: NONE,
        },
    );
    assert!(st.s.editor.title.starts_with("hello"), "{}", st.s.editor.title);
    assert_eq!(st.s.ui.coach.player_at, 10.0);
}
