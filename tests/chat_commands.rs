// Script commands in the chat, on the studio's side: what command() declares
// (its arguments, their kinds and completions), the shapes the chat's tree
// gets, the commands the studio lists, and the completions it answers with.

mod common;

use common::{FakeGame, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{Command, Studio};
use horizon_studio::game::hstudio_chatcmd::{ArgKind, ChatArg, ChatCommand};

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

/// The console's command `name`.
fn console_command<'a>(st: &'a Studio, name: &str) -> &'a Command {
    st.s.console_mod
        .commands
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("the console has no /{name}"))
}

fn arg(name: &str, kind: ArgKind, optional: bool, asks: bool) -> ChatArg {
    ChatArg {
        name: name.into(),
        kind,
        optional,
        asks,
    }
}

const WAYPOINTS: &str = "command('wp', function(rest, words) return #words .. ':' .. rest end, {
  description = 'Waypoints',
  args = {
    { name = 'action', optional = true, options = { 'add', 'remove', { text = 'clear', tooltip = 'Removes every waypoint' } } },
    { name = 'name', kind = 'text', optional = true },
  },
})";

#[test]
fn old_commands_keep_their_shape() {
    let mut game = FakeGame::new("chat-old");
    let mut st = studio(&mut game);
    game.write_mod(
        "hi.lua",
        "command('hi', function(rest, words) return rest .. '|' .. #words end, 'Says hi')\n",
    );
    run_for(&mut st, &mut game, 0.6);
    let c = &st.s.mods[mod_index(&st, "hi")].commands[0];
    assert_eq!(c.help, "Says hi");
    assert!(c.args.is_none());
    assert_eq!(c.usage_line(), "");
    assert_eq!(
        c.chat(),
        ChatCommand {
            name: "hi".into(),
            args: vec![arg("args", ArgKind::Text, true, false)],
        }
    );
    // It runs as it always did.
    assert!(st.command("hi a b"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("a b|2"));
}

#[test]
fn the_options_form_declares_arguments() {
    let mut game = FakeGame::new("chat-options");
    let mut st = studio(&mut game);
    assert_eq!(console(&mut st, &mut game, WAYPOINTS), Vec::<String>::new());
    let c = console_command(&st, "wp");
    assert_eq!(c.help, "Waypoints");
    assert_eq!(c.usage_line(), "[<action>] [<name>]");
    let shape = c.chat();
    assert_eq!(
        shape.args,
        vec![
            arg("action", ArgKind::Word, true, true),
            arg("name", ArgKind::Text, true, false)
        ]
    );
    assert!(shape.can_end(0));
    let options = &c.args.as_ref().unwrap()[0].options;
    assert_eq!(
        options,
        &vec![
            ("add".to_owned(), None),
            ("remove".to_owned(), None),
            ("clear".to_owned(), Some("Removes every waypoint".to_owned())),
        ]
    );
    // The function still gets the words as typed.
    assert!(st.command("wp remove the base"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("3:remove the base"));
}

#[test]
fn kinds_and_bounds_are_read() {
    let mut game = FakeGame::new("chat-kinds");
    let mut st = studio(&mut game);
    for (name, kind) in [
        ("word", ArgKind::Word),
        ("text", ArgKind::Text),
        (
            "int",
            ArgKind::Int {
                min: i32::MIN,
                max: i32::MAX,
            },
        ),
        (
            "number",
            ArgKind::Number {
                min: f64::MIN,
                max: f64::MAX,
            },
        ),
        ("bool", ArgKind::Bool),
        ("player", ArgKind::Player),
        ("pos", ArgKind::Pos),
        ("item", ArgKind::Item),
        ("block", ArgKind::Block),
    ] {
        let out = console(
            &mut st,
            &mut game,
            &format!("command('k', function() end, {{ args = {{ {{ name = 'a', kind = '{name}' }} }} }})"),
        );
        assert!(out.is_empty(), "{name}: {out:?}");
        assert_eq!(console_command(&st, "k").chat().args[0].kind, kind, "{name}");
    }
    console(
        &mut st,
        &mut game,
        "command('n', function() end, { args = {
          { name = 'count', kind = 'int', min = 1, max = 64 },
          { name = 'scale', kind = 'number', min = -0.5, max = 2.5 },
        } })",
    );
    let args = console_command(&st, "n").chat().args;
    assert_eq!(args[0].kind, ArgKind::Int { min: 1, max: 64 });
    assert_eq!(args[1].kind, ArgKind::Number { min: -0.5, max: 2.5 });
    assert!(!args[0].optional && !args[0].asks);
    // A name alone is a required word.
    console(
        &mut st,
        &mut game,
        "command('who', function() end, { args = { 'who' } })",
    );
    let who = console_command(&st, "who").chat();
    assert_eq!(who.args, vec![arg("who", ArgKind::Word, false, false)]);
    assert!(!who.can_end(0) && who.can_end(1));
    // args = {} takes nothing.
    console(&mut st, &mut game, "command('none', function() end, { args = {} })");
    let none = console_command(&st, "none").chat();
    assert!(none.args.is_empty() && none.can_end(0));
    // A command's suggest completes its free text: its one argument asks.
    console(
        &mut st,
        &mut game,
        "command('free', function() end, { suggest = function() return {} end })",
    );
    assert_eq!(
        console_command(&st, "free").chat().args,
        vec![arg("args", ArgKind::Text, true, true)]
    );
    // An argument's own function makes it ask, whatever its kind.
    console(
        &mut st,
        &mut game,
        "command('p', function() end, { args = { { name = 'who', kind = 'player', suggest = function() end } } })",
    );
    assert_eq!(
        console_command(&st, "p").chat().args,
        vec![arg("who", ArgKind::Player, false, true)]
    );
}

#[test]
fn usage_is_made_from_the_arguments() {
    let mut game = FakeGame::new("chat-usage");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('give2', function() end, { args = { 'item', { name = 'count', kind = 'int', optional = true } } })",
    );
    assert_eq!(console_command(&st, "give2").usage_line(), "<item> [<count>]");
    console(
        &mut st,
        &mut game,
        "command('plain', function() end, 'No arguments declared')",
    );
    assert_eq!(console_command(&st, "plain").usage_line(), "");
    console(&mut st, &mut game, "command('bare', function() end, { args = {} })");
    assert_eq!(console_command(&st, "bare").usage_line(), "");
}

#[test]
fn chat_command_can_end_after() {
    let shape = |opt: &[bool]| ChatCommand {
        name: "c".into(),
        args: opt
            .iter()
            .enumerate()
            .map(|(i, o)| arg(&format!("a{i}"), ArgKind::Word, *o, false))
            .collect(),
    };
    let required = shape(&[false, false]);
    assert_eq!(
        (0..=2).map(|n| required.can_end(n)).collect::<Vec<_>>(),
        [false, false, true]
    );
    let optional = shape(&[true, true]);
    assert_eq!(
        (0..=2).map(|n| optional.can_end(n)).collect::<Vec<_>>(),
        [true, true, true]
    );
    let mixed = shape(&[false, true, true]);
    assert_eq!(
        (0..=3).map(|n| mixed.can_end(n)).collect::<Vec<_>>(),
        [false, true, true, true]
    );
}

#[test]
fn bad_options_say_what_is_wrong() {
    let mut game = FakeGame::new("chat-bad");
    let mut st = studio(&mut game);
    let f = "function() end";
    let cases: Vec<(&str, &str)> = vec![
        (
            "{ args = { { name = 'a', kind = 'colour' } } }",
            "(args[1].kind: unknown kind 'colour' (word, text, int, number, bool, player, pos, item or block))",
        ),
        (
            "{ args = { { name = 'a', kind = 5 } } }",
            "(args[1].kind: a kind expected (word, text, int, number, bool, player, pos, item or block))",
        ),
        (
            "{ args = { { name = 'a', kind = 'text' }, 'b' } }",
            "(args[1]: only the last argument can be text)",
        ),
        (
            "{ args = { { name = 'a', optional = true }, 'b' } }",
            "(args[2]: a required argument can't follow an optional one)",
        ),
        (
            "{ args = { 'a', { name = 'b c' } } }",
            "(args[2].name: a name of letters, digits, '_' or '-' expected)",
        ),
        (
            "{ args = { { kind = 'word' } } }",
            "(args[1].name: a name of letters, digits, '_' or '-' expected)",
        ),
        (
            "{ args = { 'no spaces' } }",
            "(args[1]: a name of letters, digits, '_' or '-' expected)",
        ),
        ("{ args = { true } }", "(args[1]: a name or a table expected)"),
        (
            "{ args = { { name = 'n', kind = 'int', min = 5, max = 1 } } }",
            "(args[1].min is more than its max)",
        ),
        (
            "{ args = { { name = 'n', kind = 'int', min = 1.5 } } }",
            "(args[1].min: a whole number from -2147483648 to 2147483647 expected)",
        ),
        (
            "{ args = { { name = 'n', kind = 'int', max = 3e9 } } }",
            "(args[1].max: a whole number from -2147483648 to 2147483647 expected)",
        ),
        (
            "{ args = { { name = 'n', kind = 'number', min = 1/0 } } }",
            "(args[1].min: a finite number expected)",
        ),
        (
            "{ args = { { name = 'n', kind = 'number', min = 2, max = 1 } } }",
            "(args[1].min is more than its max)",
        ),
        (
            "{ args = { { name = 'o', optional = 'yes' } } }",
            "(args[1].optional: true or false expected)",
        ),
        (
            "{ args = (function() local t = {} for i = 1, 17 do t[i] = 'a' .. i end return t end)() }",
            "(args: a list of at most 16 arguments expected)",
        ),
        (
            "{ args = { x = 'a' } }",
            "(args: a list of at most 16 arguments expected)",
        ),
        ("{ args = 'a' }", "(args: a list of at most 16 arguments expected)"),
        (
            "{ args = { { name = 'a', options = 'add' } } }",
            "(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)",
        ),
        (
            "{ args = { { name = 'a', options = { k = 'v' } } } }",
            "(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)",
        ),
        (
            "{ args = { { name = 'a', options = { { tooltip = 'no text' } } } } }",
            "(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)",
        ),
        (
            "{ args = { { name = 'a', options = { true } } } }",
            "(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)",
        ),
        (
            "{ args = { { name = 'a', options = (function() local t = {} for i = 1, 257 do t[i] = 'o' .. i end return t end)() } } }",
            "(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)",
        ),
        (
            "{ args = { { name = 'a', suggest = {} } } }",
            "(args[1].suggest: function expected)",
        ),
        ("{ suggest = 'all' }", "(suggest: function expected)"),
        ("{ suggest = {} }", "(suggest: function expected)"),
        (
            "{ args = { 'a' }, suggest = function() end }",
            "(suggest: with args, each argument takes its own suggest)",
        ),
        ("{ description = {} }", "(description: string expected)"),
        ("true", "(string or table expected, got boolean)"),
    ];
    for (options, want) in cases {
        let out = console(&mut st, &mut game, &format!("command('bad', {f}, {options})"));
        let want = format!("bad argument #3 to 'command' {want}");
        assert!(
            out.iter().any(|l| l.contains(&want)),
            "{options}: {out:?}, wanted {want}"
        );
        assert!(st.s.console_mod.commands.is_empty(), "{options} registered a command");
    }
}

#[test]
fn the_options_form_keeps_the_name_rules() {
    let mut game = FakeGame::new("chat-names");
    let mut st = studio(&mut game);
    let out = console(
        &mut st,
        &mut game,
        "command('give', function() end, { args = { 'item' } })",
    );
    assert!(out[0].contains("/give is the game's own command"), "{out:?}");
    // The welcome mod has /hello.
    let out = console(
        &mut st,
        &mut game,
        "command('hello', function() end, { description = 'mine' })",
    );
    assert!(out[0].contains("/hello is already used by hello"), "{out:?}");
    let out = console(&mut st, &mut game, "command('a b', function() end, { args = {} })");
    assert!(
        out[0].contains("bad argument #1 to 'command' (a name of letters, digits, '_' or '-' expected)"),
        "{out:?}"
    );
    assert!(st.s.console_mod.commands.is_empty());
}

#[test]
fn registering_again_replaces_the_shape() {
    let mut game = FakeGame::new("chat-again");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('tp2', function() end, { args = { 'who' } })",
    );
    let first = console_command(&st, "tp2").serial;
    console(
        &mut st,
        &mut game,
        "command('tp2', function() end, { args = { { name = 'at', kind = 'pos' } } })",
    );
    assert_eq!(st.s.console_mod.commands.len(), 1);
    let again = console_command(&st, "tp2");
    assert_ne!(again.serial, first);
    assert_eq!(again.chat().args, vec![arg("at", ArgKind::Pos, false, false)]);
}

#[test]
fn a_mod_has_at_most_256_commands() {
    let mut game = FakeGame::new("chat-cap");
    let mut st = studio(&mut game);
    let out = console(
        &mut st,
        &mut game,
        "for i = 1, 256 do command('c' .. i, function() end) end",
    );
    assert!(out.is_empty(), "{out:?}");
    let out = console(&mut st, &mut game, "command('one_more', function() end)");
    assert!(out[0].contains("too many commands (256)"), "{out:?}");
    // One it has already can still be registered again.
    let out = console(&mut st, &mut game, "command('c7', function() end, 'again')");
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(st.s.console_mod.commands.len(), 256);
    assert_eq!(console_command(&st, "c7").help, "again");
}
