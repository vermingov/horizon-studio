// Script commands in the chat, on the studio's side: what command() declares
// (its arguments, their kinds and completions), the shapes the chat's tree
// gets, the commands the studio lists, and the completions it answers with.

mod common;

use common::{FakeGame, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{Command, Studio};
use horizon_studio::game::hstudio_chatcmd::{ArgKind, ChatArg, ChatCommand};
use horizon_studio::game::hstudio_lua::Value;

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

/// The names of the commands the chat's tree is to have.
fn chat_names(st: &Studio) -> Vec<String> {
    st.chat_commands().into_iter().map(|c| c.name).collect()
}

/// The completions for `line` (each text, with its tooltip in brackets), sorted.
fn texts(st: &mut Studio, game: &mut FakeGame, line: &str) -> Vec<String> {
    let mut out: Vec<String> = st
        .suggest(game, line)
        .unwrap_or_else(|| panic!("nothing completes {line}"))
        .list
        .into_iter()
        .map(|(t, tip)| match tip {
            Some(tip) => format!("{t} ({tip})"),
            None => t,
        })
        .collect();
    out.sort();
    out
}

/// Sets a global of a running mod.
fn set_global(st: &Studio, id: &str, name: &str, v: Value) {
    let m = &st.s.mods[mod_index(st, id)];
    m.env.as_ref().expect("running").borrow_mut().set_str(name, v);
}

/// The console lines that contain `part`.
fn logged(st: &Studio, part: &str) -> Vec<String> {
    st.s.log
        .iter()
        .filter(|l| l.text.contains(part))
        .map(|l| l.text.clone())
        .collect()
}

#[test]
fn only_commands_the_studio_takes_are_listed() {
    let mut game = FakeGame::new("chat-list");
    let mut st = studio(&mut game);
    let names = chat_names(&st);
    for name in ["hello", "lua", "studio"] {
        assert!(names.contains(&name.to_owned()), "{names:?}");
    }
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    let shapes = st.chat_commands();
    let find = |name: &str| shapes.iter().find(|c| c.name == name).unwrap().args.clone();
    assert_eq!(find("studio"), vec![arg("action", ArgKind::Word, true, true)]);
    assert_eq!(find("lua"), vec![arg("code", ArgKind::Text, false, false)]);
    assert_eq!(find("hello"), vec![arg("args", ArgKind::Text, true, false)]);

    let stamp = st.command_stamp();
    st.frame(&mut game, 0.016);
    assert_eq!(st.command_stamp(), stamp, "nothing changed");
    // A mod that fails while loading has none.
    game.write_mod("broken.lua", "command('broken', function() end)\nerror('no')\n");
    game.write_mod("wp.lua", "command('wp', function() end, { args = { 'action' } })\n");
    run_for(&mut st, &mut game, 0.6);
    let names = chat_names(&st);
    assert!(!names.contains(&"broken".to_owned()), "{names:?}");
    assert!(names.contains(&"wp".to_owned()), "{names:?}");
    assert_ne!(st.command_stamp(), stamp);
    // A hot reload with the same shape lists the same command.
    let before = st.chat_commands();
    let stamp = st.command_stamp();
    game.write_mod(
        "wp.lua",
        "command('wp', function() return 'v2' end, { args = { 'action' } })\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(st.chat_commands(), before);
    assert_ne!(st.command_stamp(), stamp, "a reload registers it anew");
    // Turned off and on, failing to load, and deleted. The game's tree reads
    // the list again only when the stamp changes, so each of these changes it.
    let i = mod_index(&st, "wp");
    let stamp = st.command_stamp();
    st.set_enabled(&mut game, i, false);
    assert!(!chat_names(&st).contains(&"wp".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "turned off");
    let stamp = st.command_stamp();
    st.set_enabled(&mut game, i, true);
    assert!(chat_names(&st).contains(&"wp".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "turned on");
    let stamp = st.command_stamp();
    game.write_mod(
        "wp.lua",
        "command('wp', function() end, { args = { 'action' } })\nerror('it fails')\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(!st.s.mods[mod_index(&st, "wp")].running());
    assert!(!chat_names(&st).contains(&"wp".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "failed to load");
    game.write_mod("wp.lua", "command('wp', function() end, { args = { 'action' } })\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(chat_names(&st).contains(&"wp".to_owned()));
    let stamp = st.command_stamp();
    std::fs::remove_file(game.mods_dir().join("wp.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(!chat_names(&st).contains(&"wp".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "deleted");
    // The console's command lasts until clear().
    let stamp = st.command_stamp();
    console(&mut st, &mut game, "command('mine', function() end)");
    assert!(chat_names(&st).contains(&"mine".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "registered in the console");
    let stamp = st.command_stamp();
    console(&mut st, &mut game, "clear()");
    assert!(!chat_names(&st).contains(&"mine".to_owned()));
    assert_ne!(st.command_stamp(), stamp, "cleared");
}

#[test]
fn options_complete_the_word_being_typed() {
    let mut game = FakeGame::new("chat-complete");
    let mut st = studio(&mut game);
    console(&mut st, &mut game, WAYPOINTS);
    let c = st.suggest(&mut game, "/wp ").unwrap();
    assert_eq!((c.start, c.len), (4, 0));
    assert_eq!(
        texts(&mut st, &mut game, "/wp "),
        ["add", "clear (Removes every waypoint)", "remove"]
    );
    let c = st.suggest(&mut game, "/wp re").unwrap();
    assert_eq!((c.start, c.len), (4, 2));
    assert_eq!(c.list, vec![("remove".to_owned(), None)]);
    assert_eq!(texts(&mut st, &mut game, "/wp RE"), ["remove"]);
    // What is typed already is not offered again.
    assert!(texts(&mut st, &mut game, "/wp clear").is_empty());
    // The name is still being typed, or no command has it.
    assert!(st.suggest(&mut game, "/wp").is_none());
    assert!(st.suggest(&mut game, "/nothing x").is_none());
    // The text argument after it has nothing to offer.
    assert!(texts(&mut st, &mut game, "/wp add ho").is_empty());
}

#[test]
fn sub_words_match_after_separators() {
    let mut game = FakeGame::new("chat-sub");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('go', function() end, { args = { { name = 'to', options = { 'go_home', 'minecraft:stone', 'a.b/c' } } } })",
    );
    assert_eq!(texts(&mut st, &mut game, "/go ho"), ["go_home"]);
    assert!(texts(&mut st, &mut game, "/go st").is_empty(), "':' is no separator");
    assert_eq!(texts(&mut st, &mut game, "/go mine"), ["minecraft:stone"]);
    assert_eq!(texts(&mut st, &mut game, "/go c"), ["a.b/c"]);
    assert_eq!(texts(&mut st, &mut game, "/go b"), ["a.b/c"]);
}

#[test]
fn a_suggest_function_gets_the_words_and_can_give_tooltips() {
    let mut game = FakeGame::new("chat-suggest");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('pick', function() end, { args = { 'first', { name = 'second', suggest = function(rest, words, arg)
          seen = { rest = rest, n = #words, last = words[#words], arg = arg }
          return { 'alpha', { text = 'beta', tooltip = 'The second' }, 7, true, { tooltip = 'no text' } }
        end } } })",
    );
    let c = st.suggest(&mut game, "/pick x b").unwrap();
    assert_eq!(c.list, vec![("beta".to_owned(), Some("The second".to_owned()))]);
    assert_eq!((c.start, c.len), (8, 1));
    assert_eq!(
        console(
            &mut st,
            &mut game,
            "seen.rest .. '|' .. seen.n .. '|' .. seen.last .. '|' .. seen.arg"
        ),
        ["\"x b|2|b|second\""]
    );
    // After a space the last word is "", and numbers become text.
    assert_eq!(
        texts(&mut st, &mut game, "/pick x "),
        ["7", "alpha", "beta (The second)"]
    );
    assert_eq!(
        console(&mut st, &mut game, "seen.rest .. '|' .. seen.n .. '|' .. seen.last"),
        ["\"x |2|\""]
    );
    // The first argument has neither options nor a function: nothing.
    assert!(texts(&mut st, &mut game, "/pick a").is_empty());
}

#[test]
fn a_command_without_arguments_completes_the_word_typed() {
    let mut game = FakeGame::new("chat-free");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('say2', function() end, { suggest = function(rest, words, arg)
          seen = { rest = rest, n = #words, arg = tostring(arg) }
          return { 'hello', 'help', 'world', 'two words' }
        end })",
    );
    let c = st.suggest(&mut game, "/say2 he").unwrap();
    assert_eq!((c.start, c.len), (6, 2));
    assert_eq!(texts(&mut st, &mut game, "/say2 he"), ["hello", "help"]);
    let c = st.suggest(&mut game, "/say2 one he").unwrap();
    assert_eq!((c.start, c.len), (10, 2));
    assert_eq!(
        console(&mut st, &mut game, "seen.rest .. '|' .. seen.n .. '|' .. seen.arg"),
        ["\"one he|2|nil\""]
    );
    // Free text takes a completion with a space.
    assert_eq!(texts(&mut st, &mut game, "/say2 tw"), ["two words"]);
}

#[test]
fn a_text_argument_is_completed_whole() {
    let mut game = FakeGame::new("chat-text");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('wp2', function() end, { args = { 'action', { name = 'name', kind = 'text', options = { 'the base', 'home' } } } })",
    );
    let line = "/wp2 remove the b";
    let c = st.suggest(&mut game, line).unwrap();
    assert_eq!((c.start, c.len), (line.find("the").unwrap(), 5));
    assert_eq!(c.list, vec![("the base".to_owned(), None)]);
}

#[test]
fn pos_takes_three_words() {
    let mut game = FakeGame::new("chat-pos");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('mark', function() end, { args = { { name = 'at', kind = 'pos' }, { name = 'what', options = { 'here', 'there' } } } })",
    );
    assert_eq!(texts(&mut st, &mut game, "/mark 1 2 3 th"), ["there"]);
    // Inside the position: the pos argument, which has no options.
    assert!(texts(&mut st, &mut game, "/mark 1 2 t").is_empty());
}

#[test]
fn completions_are_counted_in_utf16_units() {
    let mut game = FakeGame::new("chat-utf16");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('u', function() end, { args = { 'a', { name = 'b', options = { 'zoo' } } } })",
    );
    let line = "/u é😀 z";
    let c = st.suggest(&mut game, line).unwrap();
    let at = line.rfind('z').unwrap();
    assert_eq!(c.start, line[..at].encode_utf16().count());
    assert_eq!(c.start, 7);
    assert_eq!(c.len, 1);
    assert_eq!(c.list, vec![("zoo".to_owned(), None)]);
}

#[test]
fn a_failing_suggest_is_logged_once_and_the_mod_runs_on() {
    let mut game = FakeGame::new("chat-fail");
    let mut st = studio(&mut game);
    game.write_mod(
        "f.lua",
        "fail = 'nope'\ncommand('f', function() end, { args = { { name = 'a', options = { 'one' }, suggest = function()\n  if fail then error(fail) end\n  return { 'two' }\nend } } })\n",
    );
    run_for(&mut st, &mut game, 0.6);
    for _ in 0..3 {
        assert_eq!(texts(&mut st, &mut game, "/f "), ["one"], "the options still come");
    }
    let failures = logged(&st, "/f completions:");
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("f.lua:3: nope"), "{failures:?}");
    assert!(st.s.mods[mod_index(&st, "f")].running());
    // Another error is logged; so is the same one after it worked again.
    set_global(&st, "f", "fail", Value::str("other"));
    texts(&mut st, &mut game, "/f ");
    assert_eq!(logged(&st, "/f completions:").len(), 2);
    set_global(&st, "f", "fail", Value::Nil);
    assert_eq!(texts(&mut st, &mut game, "/f "), ["one", "two"]);
    set_global(&st, "f", "fail", Value::str("other"));
    texts(&mut st, &mut game, "/f ");
    assert_eq!(logged(&st, "/f completions:").len(), 3);
    // Nothing reached the chat.
    assert!(
        !game.chats.iter().any(|c| c.contains("completions")),
        "{:?}",
        game.chats
    );
}

#[test]
fn a_suggest_that_runs_too_long_is_turned_off() {
    let mut game = FakeGame::new("chat-slow");
    let mut st = studio(&mut game);
    game.write_mod(
        "slow.lua",
        "command('slow', function() end, { args = { { name = 'a', options = { 'one' }, suggest = function() while true do end end } } })\n",
    );
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "slow");
    let serial = st.s.mods[i].commands[0].serial;
    assert_eq!(texts(&mut st, &mut game, "/slow "), ["one"]);
    let lines = logged(&st, "/slow completions:");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("ran too long") && lines[0].contains("until the mod reloads"),
        "{lines:?}"
    );
    let c = &st.s.mods[i].commands[0];
    assert!(c.args.as_ref().unwrap()[0].suggest.is_none());
    assert_ne!(c.serial, serial, "the chat's tree hears of it");
    assert!(st.s.mods[i].running());
    // It isn't called again.
    let started = std::time::Instant::now();
    assert_eq!(texts(&mut st, &mut game, "/slow o"), ["one"]);
    assert!(started.elapsed().as_secs_f64() < 0.5);
    assert_eq!(logged(&st, "/slow completions:").len(), 1);
}

#[test]
fn completions_are_capped_and_cleaned() {
    let mut game = FakeGame::new("chat-capped");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "command('many', function() end, { args = { { name = 'a', suggest = function()
          local t = {}
          for i = 1, 1000 do t[i] = 'w' .. i end
          return t
        end } } })",
    );
    assert_eq!(st.suggest(&mut game, "/many ").unwrap().list.len(), 256);
    console(
        &mut st,
        &mut game,
        "command('odd', function() end, { args = { { name = 'a', suggest = function()
          return { string.rep('x', 1000), '', 'two words', { text = 'tip', tooltip = string.rep('é', 200) }, 'ok' }
        end }, { name = 'rest', kind = 'text', options = { 'two words' } } } })",
    );
    let c = st.suggest(&mut game, "/odd ").unwrap();
    let got: Vec<&str> = c.list.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(got, ["tip", "ok"], "too long, empty and spaced texts are left out");
    let tip = c.list[0].1.as_ref().unwrap();
    assert!(tip.len() <= 256 && tip.chars().all(|ch| ch == 'é'), "{}", tip.len());
    // A text argument takes a completion with a space.
    assert_eq!(texts(&mut st, &mut game, "/odd ok tw"), ["two words"]);
    // A suggest that returns no list is an error, logged.
    console(
        &mut st,
        &mut game,
        "command('nolist', function() end, { args = { { name = 'a', suggest = function() return 'abc' end } } })",
    );
    assert!(texts(&mut st, &mut game, "/nolist ").is_empty());
    assert_eq!(
        logged(&st, "/nolist completions:"),
        ["/nolist completions: suggest returned a string, not a list"]
    );
}

#[test]
fn studio_help_lists_the_commands() {
    let mut game = FakeGame::new("chat-help");
    let mut st = studio(&mut game);
    console(&mut st, &mut game, WAYPOINTS);
    assert!(st.command("studio help"));
    st.frame(&mut game, 0.016);
    let chat = game.chats.join("\n");
    assert!(chat.contains("[Horizon Studio] commands:"), "{chat}");
    assert!(chat.contains("/wp [<action>] [<name>] - Waypoints"), "{chat}");
    assert!(chat.contains("/hello - Says hello: /hello [name]"), "{chat}");
    assert!(chat.contains("/studio [help|reload] - "), "{chat}");
    assert!(chat.contains("/lua <code> - runs Lua in the console"), "{chat}");
    // /studio completes its own actions.
    assert_eq!(
        texts(&mut st, &mut game, "/studio r"),
        ["reload (Reloads every script mod)"]
    );
    assert_eq!(
        texts(&mut st, &mut game, "/studio "),
        [
            "help (Lists the commands the studio takes)",
            "reload (Reloads every script mod)"
        ]
    );
    assert!(texts(&mut st, &mut game, "/lua pri").is_empty());
    // Many commands: the first 20, then a pointer to the Library.
    console(
        &mut st,
        &mut game,
        "for i = 1, 30 do command(string.format('c%02d', i), function() end) end",
    );
    game.chats.clear();
    st.command("studio help");
    st.frame(&mut game, 0.016);
    assert!(
        game.chats
            .iter()
            .any(|c| c.starts_with("and ") && c.ends_with(" more: see the Library")),
        "{:?}",
        game.chats
    );
    assert!(game.chats.len() <= 24, "{:?}", game.chats);
    // An unknown action names help.
    st.command("studio what");
    st.frame(&mut game, 0.016);
    assert!(game.chats.last().unwrap().contains("/studio help"), "{:?}", game.chats);
}

#[test]
fn the_waypoints_example_completes_its_actions_and_names() {
    let waypoints = horizon_studio::game::hstudio_examples::EXAMPLES
        .iter()
        .find(|e| e.id == "waypoints")
        .unwrap();
    let mut game = FakeGame::new("chat-waypoints");
    let mut st = studio(&mut game);
    game.write_mod("waypoints.lua", waypoints.source);
    run_for(&mut st, &mut game, 0.6);
    let c = &st.s.mods[mod_index(&st, "waypoints")].commands[0];
    assert_eq!(c.usage_line(), "[<action>] [<name>]");
    assert!(c.help.starts_with("Waypoints: /wp add <name>"));
    assert_eq!(
        texts(&mut st, &mut game, "/wp "),
        [
            "add (Adds one where you stand)",
            "clear (Removes every waypoint)",
            "remove (Removes one)"
        ]
    );
    for line in ["wp add home", "wp add the base"] {
        assert!(st.command(line));
        st.frame(&mut game, 0.016);
    }
    assert_eq!(texts(&mut st, &mut game, "/wp remove "), ["home", "the base"]);
    assert_eq!(texts(&mut st, &mut game, "/wp remove th"), ["the base"]);
    assert!(texts(&mut st, &mut game, "/wp add ").is_empty(), "a new name is typed");
    // Its function still gets the words as typed.
    assert!(st.command("wp remove the base"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("Waypoint the base removed"));
}
