// The studio's runtime on a fake game: loading, hot reloading, isolation,
// the API, the console and the bundled examples.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio, error_line, mod_id, read_header};
use horizon_studio::game::hstudio_examples::EXAMPLES;

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

#[test]
fn first_start_installs_the_welcome_mod() {
    let mut game = FakeGame::new("welcome");
    let mut st = studio(&mut game);
    assert!(game.mods_dir().join("hello.lua").is_file());
    let i = mod_index(&st, "hello");
    assert_eq!(st.s.mods[i].state, ModState::Running);
    assert_eq!(st.s.mods[i].name, "Hello");
    assert!(log(&st).iter().any(|l| l == "[hello] hello loaded"), "{:?}", log(&st));
    assert!(game.dir.join("scratch.lua").is_file());
    // /hello runs on the next frame and answers in chat.
    assert!(st.command("hello Steve"));
    assert!(!st.command("give @s diamond"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("Hello, Steve!"));
}

#[test]
fn a_mod_reloads_when_its_file_changes() {
    let mut game = FakeGame::new("reload");
    let mut st = studio(&mut game);
    game.write_mod("counter.lua", "-- name: Counter\nprint('version one')\n");
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "counter");
    assert_eq!(st.s.mods[i].state, ModState::Running);
    assert!(log(&st).contains(&"[counter] version one".to_owned()));
    game.write_mod("counter.lua", "-- name: Counter\nprint('version two, longer')\n");
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "counter");
    assert!(
        log(&st).contains(&"[counter] version two, longer".to_owned()),
        "{:?}",
        log(&st)
    );
    assert_eq!(st.s.mods[i].reloads, 1);
    assert!(log(&st).iter().any(|l| l.starts_with("Reloaded counter")));
    // Removing the file removes the mod.
    std::fs::remove_file(game.mods_dir().join("counter.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.mods.iter().all(|m| m.id != "counter"));
}

#[test]
fn unload_runs_before_a_reload() {
    let mut game = FakeGame::new("unload");
    let mut st = studio(&mut game);
    game.write_mod("u.lua", "on('unload', function() print('bye') end)\n");
    run_for(&mut st, &mut game, 0.6);
    game.write_mod("u.lua", "on('unload', function() print('bye') end) print('again')\n");
    run_for(&mut st, &mut game, 0.6);
    let l = log(&st);
    let bye = l.iter().position(|x| x == "[u] bye").expect("unload ran");
    let again = l.iter().position(|x| x == "[u] again").expect("reloaded");
    assert!(bye < again);
}

#[test]
fn a_failing_mod_stops_alone() {
    let mut game = FakeGame::new("isolation");
    let mut st = studio(&mut game);
    game.write_mod(
        "good.lua",
        "n = 0\non('tick', function() n = n + 1 end)\ncommand('count', function() return 'n=' .. n end)\n",
    );
    game.write_mod(
        "bad.lua",
        "local t = 0\non('tick', function()\n  t = t + 1\n  if t > 3 then local x = nil; return x.field end\nend)\n",
    );
    run_for(&mut st, &mut game, 1.0);
    let bad = mod_index(&st, "bad");
    match &st.s.mods[bad].state {
        ModState::Failed(msg, trace) => {
            assert_eq!(msg, "tick: bad.lua:4: attempt to index a nil value (local 'x')");
            assert!(trace.is_empty() || trace.contains("bad.lua:4"), "{trace}");
        }
        other => panic!("bad should have failed: {other:?}"),
    }
    assert!(game.chats.iter().any(|c| c.contains("bad stopped")), "{:?}", game.chats);
    assert!(st.s.mods[mod_index(&st, "good")].running());
    st.command("count");
    st.frame(&mut game, 0.016);
    let n: i32 = game.chats.last().unwrap().trim_start_matches("n=").parse().unwrap();
    assert!(n > 5, "{n}");
    assert_eq!(error_line("bad.lua:4: attempt"), Some(4));
}

#[test]
fn an_endless_loop_is_stopped() {
    let mut game = FakeGame::new("spin");
    let mut st = studio(&mut game);
    game.write_mod("spin.lua", "on('frame', function() while true do end end)\n");
    let t = std::time::Instant::now();
    run_for(&mut st, &mut game, 0.6);
    assert!(t.elapsed().as_secs() < 5);
    match &st.s.mods[mod_index(&st, "spin")].state {
        ModState::Failed(msg, _) => assert!(msg.contains("ran too long"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn syntax_errors_fail_the_load_with_their_line() {
    let mut game = FakeGame::new("syntax");
    let mut st = studio(&mut game);
    game.write_mod("broken.lua", "print('a')\nif x then\n");
    run_for(&mut st, &mut game, 0.6);
    match &st.s.mods[mod_index(&st, "broken")].state {
        ModState::Failed(msg, _) => assert_eq!(
            msg,
            "broken.lua:3: 'end' expected (to close 'if' at line 2) near '<eof>'"
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn console_runs_expressions_and_statements() {
    let mut game = FakeGame::new("console");
    let mut st = studio(&mut game);
    assert_eq!(console(&mut st, &mut game, "1 + 2"), vec!["3"]);
    assert_eq!(console(&mut st, &mut game, "x = 21"), Vec::<String>::new());
    assert_eq!(console(&mut st, &mut game, "x * 2"), vec!["42"]);
    assert_eq!(
        console(&mut st, &mut game, "{1, 2, name = 'a'}"),
        vec!["{ 1, 2, name = \"a\" }"]
    );
    assert_eq!(console(&mut st, &mut game, "player.pos()"), vec!["10.5, 64, -3.25"]);
    assert_eq!(console(&mut st, &mut game, "print('hi')"), vec!["hi"]);
    assert_eq!(
        console(&mut st, &mut game, "nope()"),
        vec!["console:1: attempt to call a nil value (global 'nope')"]
    );
    assert_eq!(
        console(&mut st, &mut game, "if"),
        vec!["console:1: unexpected symbol near '<eof>'"]
    );
    assert_eq!(st.s.history.last().map(String::as_str), Some("if"));
    // The console's registrations run like a mod's, until clear().
    console(
        &mut st,
        &mut game,
        "ticks = 0 on('tick', function() ticks = ticks + 1 end)",
    );
    run_for(&mut st, &mut game, 0.5);
    let t: f64 = console(&mut st, &mut game, "ticks")[0].parse().unwrap();
    assert!(t >= 9.0, "{t}");
    console(&mut st, &mut game, "clear()");
    assert!(st.s.console_mod.handlers.tick.is_empty());
    // /lua from chat.
    assert!(st.command("lua 6 * 7"));
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("42"));
}

#[test]
fn console_helpers() {
    let mut game = FakeGame::new("helpers");
    let mut st = studio(&mut game);
    let out = console(&mut st, &mut game, "help('player.pos')");
    assert_eq!(out, vec!["player.pos(): x, y, z (nil outside a world)"]);
    let out = console(&mut st, &mut game, "mods()");
    assert_eq!(
        out,
        vec!["{ hello = \"running\", uikit = \"running\", viahorizon = \"running\" }"]
    );
    console(&mut st, &mut game, "new('My Thing')");
    assert!(game.mods_dir().join("my_thing.lua").is_file());
    assert_eq!(st.s.editor.title, "my_thing.lua");
    assert!(st.s.mods[mod_index(&st, "my_thing")].running());
    assert_eq!(
        console(&mut st, &mut game, "edit('missing')"),
        vec!["console:1: no script mod 'missing'"]
    );
    // Mods cannot use console functions.
    game.write_mod("sneaky.lua", "clear()\n");
    run_for(&mut st, &mut game, 0.6);
    match &st.s.mods[mod_index(&st, "sneaky")].state {
        ModState::Failed(msg, _) => assert!(msg.contains("attempt to call a nil value (global 'clear')"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn hud_panels_and_drawing() {
    let mut game = FakeGame::new("hud");
    let mut st = studio(&mut game);
    game.write_mod(
        "panel.lua",
        "hud.panel('top-right', {'one', 'two'})\non('hud', function(w, h) hud.rect(1, 2, 3, 4, '#ff0000') hud.text(5, 6, 'drawn ' .. w) end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    game.drawn.clear();
    st.hud(&mut game);
    assert!(
        game.drawn.contains(&"fill [1.0, 2.0, 3.0, 4.0] ffff0000".to_owned()),
        "{:?}",
        game.drawn
    );
    assert!(
        game.drawn.contains(&"text drawn 480 @[5.0, 6.0] ffffff".to_owned()),
        "{:?}",
        game.drawn
    );
    assert!(
        game.drawn.contains(&"text one @[302.0, 4.0] ffffff".to_owned()),
        "{:?}",
        game.drawn
    );
    assert!(
        game.drawn.contains(&"text two @[302.0, 14.0] ffffff".to_owned()),
        "{:?}",
        game.drawn
    );
    // Drawing outside a hud handler is an error.
    let out = console(&mut st, &mut game, "hud.text(1, 1, 'x')");
    assert!(out[0].contains("draws only inside on(\"hud\""), "{out:?}");
    assert_eq!(console(&mut st, &mut game, "color(255, 0, 128)"), vec!["4294901888"]);
}

#[test]
fn storage_keeps_values_across_reloads() {
    let mut game = FakeGame::new("storage");
    let mut st = studio(&mut game);
    game.write_mod(
        "keep.lua",
        "local n = (storage.get('n') or 0) + 1\nstorage.set('n', n)\nstorage.set('t', {a = 1, list = {'x', 'y'}, flag = true, s = 'q\"uote\\n'})\nprint('n=' .. n)\nlocal t = storage.get('t')\nprint(t.a, t.list[2], t.flag, t.s)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    st.reload_all(&mut game);
    let l = log(&st);
    assert!(l.contains(&"[keep] n=1".to_owned()), "{l:?}");
    assert!(l.contains(&"[keep] n=2".to_owned()), "{l:?}");
    assert!(l.iter().any(|x| x == "[keep] 1    y    true    q\"uote"), "{l:?}");
    assert!(game.dir.join("storage/keep/n.lua").is_file());
    let out = console(&mut st, &mut game, "storage.set('../escape', 1)");
    assert!(out[0].contains("storage keys"), "{out:?}");
    let out = console(&mut st, &mut game, "storage.set('f', print)");
    assert!(out[0].contains("functions cannot be stored"), "{out:?}");
}

#[test]
fn timers() {
    let mut game = FakeGame::new("timers");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "fired = 0 ticks = 0 after(0.5, function() fired = fired + 1 end) id = every(0.2, function() ticks = ticks + 1 end)",
    );
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(console(&mut st, &mut game, "fired"), vec!["0"]);
    run_for(&mut st, &mut game, 0.4);
    assert_eq!(console(&mut st, &mut game, "fired"), vec!["1"]);
    run_for(&mut st, &mut game, 1.0);
    assert_eq!(console(&mut st, &mut game, "fired"), vec!["1"]);
    let before: f64 = console(&mut st, &mut game, "ticks")[0].parse().unwrap();
    assert!((7.0..=9.0).contains(&before), "{before}");
    console(&mut st, &mut game, "cancel(id)");
    run_for(&mut st, &mut game, 1.0);
    let after: f64 = console(&mut st, &mut game, "ticks")[0].parse().unwrap();
    assert_eq!(before, after);
}

#[test]
fn the_world_and_the_player() {
    let mut game = FakeGame::new("world");
    game.blocks.insert((1, 63, 2), "minecraft:stone".into());
    let mut st = studio(&mut game);
    assert_eq!(
        console(&mut st, &mut game, "world.block(1.5, 63.9, 2.1)"),
        vec!["\"minecraft:stone\""]
    );
    assert_eq!(console(&mut st, &mut game, "world.time()"), vec!["1000, 50000, 1"]);
    assert_eq!(console(&mut st, &mut game, "player.block()"), vec!["10, 64, -4"]);
    assert_eq!(console(&mut st, &mut game, "player.facing()"), vec!["\"north\""]);
    assert_eq!(
        console(&mut st, &mut game, "player.held()"),
        vec!["\"minecraft:diamond_pickaxe\", 1"]
    );
    assert_eq!(console(&mut st, &mut game, "player.info().mode"), vec!["\"survival\""]);
    assert_eq!(
        console(&mut st, &mut game, "keys.mapping('key.jump')"),
        vec!["false, \"Space\""]
    );
    assert_eq!(console(&mut st, &mut game, "game.fps()"), vec!["144, 90, 60"]);
    game.player = None;
    game.clock = None;
    assert_eq!(console(&mut st, &mut game, "player.pos()"), vec!["nil"]);
    assert_eq!(console(&mut st, &mut game, "world.time()"), vec!["nil"]);
}

#[test]
fn chat_and_commands_are_rate_limited() {
    let mut game = FakeGame::new("send");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "game.run('time set day') chat.say('hello') game.run('/weather clear')",
    );
    assert_eq!(game.sent, vec!["/time set day", "hello", "/weather clear"]);
    let out = console(&mut st, &mut game, "for i = 1, 20 do chat.say('spam ' .. i) end");
    assert!(out[0].contains("sending too fast"), "{out:?}");
    assert_eq!(game.sent.len(), 8);
    run_for(&mut st, &mut game, 1.0);
    console(&mut st, &mut game, "chat.say('later')");
    assert_eq!(game.sent.last().map(String::as_str), Some("later"));
}

#[test]
fn commands_are_checked() {
    let mut game = FakeGame::new("commands");
    let mut st = studio(&mut game);
    let out = console(&mut st, &mut game, "command('give', function() end)");
    assert!(out[0].contains("/give is the game's own command"), "{out:?}");
    let out = console(&mut st, &mut game, "command('hello', function() end)");
    assert!(out[0].contains("/hello is already used by hello"), "{out:?}");
    console(
        &mut st,
        &mut game,
        "command('echo', function(args, words) return #words .. ':' .. args end)",
    );
    st.command("echo a b  c");
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("3:a b  c"));
    console(&mut st, &mut game, "command('boom', function() error('nope') end)");
    st.command("boom");
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("/boom: console:1: nope"));
}

#[test]
fn agent_queries_and_actions() {
    let mut game = FakeGame::new("agent");
    game.agent_now.insert(
        "state".into(),
        serde_json::json!({"player": {"health": 20}, "screen": "playing"}),
    );
    game.agent_now.insert(
        "find-block".into(),
        serde_json::json!({"count": 2, "nearest": [{"pos": [1, 2, 3], "distance": 4.5}]}),
    );
    let mut st = studio(&mut game);
    assert_eq!(
        console(&mut st, &mut game, "agent.query('state').player.health"),
        vec!["20"]
    );
    assert_eq!(
        console(&mut st, &mut game, "world.find('stone').nearest[1].pos[3]"),
        vec!["3"]
    );
    assert_eq!(game.agent_calls.last().map(String::as_str), Some("find-block stone 32"));
    let out = console(&mut st, &mut game, "agent.query('quit')");
    assert!(
        out[0].contains("'quit' is not an agent command scripts can use"),
        "{out:?}"
    );
    console(
        &mut st,
        &mut game,
        "done = nil agent.run('goto 1 64 2', function(r, err) done = r and r.arrived or err end)",
    );
    assert_eq!(console(&mut st, &mut game, "done"), vec!["nil"]);
    let (line, tx) = game.agent_later.pop().unwrap();
    assert_eq!(line, "goto 1 64 2");
    tx.send(r#"{"arrived": true}"#.into()).unwrap();
    st.frame(&mut game, 0.016);
    assert_eq!(console(&mut st, &mut game, "done"), vec!["true"]);
    console(
        &mut st,
        &mut game,
        "agent.run('break 1 2 3', function(r, err) done = err end)",
    );
    let (_, tx) = game.agent_later.pop().unwrap();
    tx.send(r#"{"error": "too far"}"#.into()).unwrap();
    st.frame(&mut game, 0.016);
    assert_eq!(console(&mut st, &mut game, "done"), vec!["\"too far\""]);
    // Not while drawing.
    console(&mut st, &mut game, "on('hud', function() agent.query('state') end)");
    st.hud(&mut game);
    assert!(
        log(&st)
            .iter()
            .any(|l| l.contains("agent functions are not available while drawing")),
        "{:?}",
        log(&st)
    );
}

#[test]
fn keys_reach_handlers() {
    let mut game = FakeGame::new("keys");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "seen = {} on('key', function(k, down) seen[#seen + 1] = k .. (down and '+' or '-') end)",
    );
    st.key("key.keyboard.r", true);
    st.frame(&mut game, 0.016);
    assert_eq!(console(&mut st, &mut game, "keys.down('key.keyboard.r')"), vec!["true"]);
    st.key("key.keyboard.r", false);
    st.frame(&mut game, 0.016);
    assert_eq!(
        console(&mut st, &mut game, "table.concat(seen, ' ')"),
        vec!["\"key.keyboard.r+ key.keyboard.r-\""]
    );
    assert_eq!(
        console(&mut st, &mut game, "keys.down('key.keyboard.r')"),
        vec!["false"]
    );
}

#[test]
fn zoom_is_the_strongest_request() {
    let mut game = FakeGame::new("zoom");
    let mut st = studio(&mut game);
    assert_eq!(st.zoom(), None);
    console(&mut st, &mut game, "view.zoom(3)");
    assert_eq!(st.zoom(), Some(3.0));
    console(&mut st, &mut game, "view.zoom(50)");
    assert_eq!(st.zoom(), Some(10.0));
    console(&mut st, &mut game, "view.zoom(1)");
    assert_eq!(st.zoom(), None);
}

#[test]
fn folder_mods_require_their_files() {
    let mut game = FakeGame::new("folder");
    let mut st = studio(&mut game);
    let dir = game.mods_dir().join("Big Mod");
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(
        dir.join("main.lua"),
        "-- name: Big\nlocal util = require('lib.util')\nprint(util.double(21), require('lib.util') == util)\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("lib/util.lua"),
        "local M = {}\nfunction M.double(x) return x * 2 end\nreturn M\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "big_mod");
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    assert!(log(&st).contains(&"[big_mod] 42    true".to_owned()), "{:?}", log(&st));
    // Editing a module file reloads the mod.
    std::fs::write(
        dir.join("lib/util.lua"),
        "local M = {}\nfunction M.double(x) return x * 20 end\nreturn M\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(log(&st).contains(&"[big_mod] 420    true".to_owned()), "{:?}", log(&st));
    let out = console(&mut st, &mut game, "require('x')");
    assert!(out[0].contains("require works in modules"), "{out:?}");
}

/// A module of `files` (path in it, text) in the mods folder.
fn write_module(game: &FakeGame, id: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = game.mods_dir().join(id);
    for (rel, text) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn failure(st: &Studio, id: &str) -> (String, String) {
    match &st.s.mods[mod_index(st, id)].state {
        ModState::Failed(msg, trace) => (msg.clone(), trace.clone()),
        other => panic!("{id} is {other:?}, not failed"),
    }
}

#[test]
fn require_runs_a_file_once_whatever_the_spelling() {
    let mut game = FakeGame::new("require-once");
    let mut st = studio(&mut game);
    write_module(
        &game,
        "spell",
        &[
            (
                "main.lua",
                "local a = require('lib.util')\nlocal b = require('lib/util')\nlocal c = require('lib/util.lua')\nprint(a == b, b == c, a.n, a.name)\n",
            ),
            ("lib/util.lua", "print('util runs')\nreturn { n = 7, name = ... }\n"),
        ],
    );
    run_for(&mut st, &mut game, 0.6);
    let lines = log(&st);
    assert!(st.s.mods[mod_index(&st, "spell")].running(), "{lines:?}");
    assert_eq!(
        lines.iter().filter(|l| *l == "[spell] util runs").count(),
        1,
        "{lines:?}"
    );
    // `...` is the name as the first require wrote it.
    assert!(
        lines.contains(&"[spell] true    true    7    lib.util".to_owned()),
        "{lines:?}"
    );
}

#[test]
fn requires_that_loop_say_which_files() {
    let mut game = FakeGame::new("require-loop");
    let mut st = studio(&mut game);
    write_module(
        &game,
        "looped",
        &[
            ("main.lua", "require('a')\n"),
            ("a.lua", "require('b')\nreturn {}\n"),
            ("b.lua", "require('a')\nreturn {}\n"),
        ],
    );
    run_for(&mut st, &mut game, 0.6);
    let (msg, _) = failure(&st, "looped");
    assert!(msg.contains("'a' is already loading"), "{msg}");
    assert!(msg.contains("main.lua -> a.lua -> b.lua -> a.lua"), "{msg}");
    assert!(!msg.contains("stack overflow"), "{msg}");
    // main.lua starts the chain only when its top level began the loop,
    // after a wait too; a command begins it at the first file it requires.
    write_module(
        &game,
        "later",
        &[
            ("main.lua", "task.wait()\nrequire('a')\n"),
            ("a.lua", "require('b')\nreturn {}\n"),
            ("b.lua", "require('a')\nreturn {}\n"),
        ],
    );
    write_module(
        &game,
        "lazy",
        &[
            (
                "main.lua",
                "command('lazy', function() local ok, e = pcall(require, 'a') return e end)\n",
            ),
            ("a.lua", "require('b')\nreturn {}\n"),
            ("b.lua", "require('a')\nreturn {}\n"),
        ],
    );
    run_for(&mut st, &mut game, 0.6);
    let (msg, _) = failure(&st, "later");
    assert!(msg.contains("(main.lua -> a.lua -> b.lua -> a.lua)"), "{msg}");
    st.command("lazy");
    st.frame(&mut game, 0.016);
    let said = game.chats.last().cloned().unwrap_or_default();
    assert!(
        said.contains("'a' is already loading (a.lua -> b.lua -> a.lua)"),
        "{said}"
    );
    // main.lua runs once, by itself.
    write_module(&game, "selfish", &[("main.lua", "require('main')\n")]);
    run_for(&mut st, &mut game, 0.6);
    assert!(failure(&st, "selfish").0.contains("main.lua starts the module"));
}

#[test]
fn require_stays_inside_the_module() {
    let mut game = FakeGame::new("require-sandbox");
    let mut st = studio(&mut game);
    let outside = game.dir.join("outside.lua");
    std::fs::write(&outside, "print('escaped')\nreturn 1\n").unwrap();
    let absolute = outside.to_string_lossy().trim_end_matches(".lua").to_owned();
    write_module(&game, "abs", &[("main.lua", &format!("require([[{absolute}]])\n"))]);
    write_module(&game, "dots", &[("main.lua", "require('.x')\n")]);
    write_module(&game, "up", &[("main.lua", "require('../outside')\n")]);
    run_for(&mut st, &mut game, 0.6);
    for id in ["abs", "dots", "up"] {
        let (msg, _) = failure(&st, id);
        assert!(msg.contains("bad module name"), "{id}: {msg}");
    }
    assert!(!log(&st).iter().any(|l| l.contains("escaped")), "{:?}", log(&st));
    // A missing file names the ones the module has.
    write_module(
        &game,
        "lost",
        &[("main.lua", "require('nope')\n"), ("util.lua", "return 1\n")],
    );
    run_for(&mut st, &mut game, 0.6);
    let (msg, _) = failure(&st, "lost");
    assert!(
        msg.contains("module 'nope' not found (nope.lua; the module has main.lua, util.lua)"),
        "{msg}"
    );
}

#[test]
fn error_lines_belong_to_their_files() {
    use horizon_studio::game::hstudio::{error_line_in, error_places};
    let msg = "m/main.lua:2: m/lib.lua:3: unexpected symbol near 'x'";
    assert_eq!(
        error_places(msg),
        [("m/main.lua".to_owned(), 2), ("m/lib.lua".to_owned(), 3)]
    );
    assert_eq!(error_line_in(msg, "", "m/lib.lua"), Some(3));
    assert_eq!(error_line_in(msg, "", "m/main.lua"), Some(2));
    assert_eq!(error_places("tick: m/lib.lua:7: y"), [("m/lib.lua".to_owned(), 7)]);
    assert_eq!(error_line_in("tick: m/lib.lua:7: y", "", "m/lib.lua"), Some(7));
    assert_eq!(error_line_in("tick: m/lib.lua:7: y", "", "m/main.lua"), None);
    // The traceback, innermost first, gives the line in a file the message doesn't name.
    let trace = "m/lib.lua:7: in f\nm/main.lua:12: in main chunk";
    assert_eq!(error_line_in("tick: m/lib.lua:7: y", trace, "m/main.lua"), Some(12));
    assert_eq!(error_line_in("tick: m/lib.lua:7: y", trace, "m/other.lua"), None);
    assert!(error_places("no place here: lua 5").is_empty());
}

#[test]
fn a_modules_files_are_listed_main_first() {
    let mut game = FakeGame::new("module-files");
    let mut st = studio(&mut game);
    let dir = write_module(
        &game,
        "tree",
        &[
            ("shared.lua", "return {}\n"),
            ("lib/util.lua", "return {}\n"),
            ("main.lua", "print('tree')\n"),
            ("lib.lua", "return {}\n"),
            (".hidden.lua", "return {}\n"),
            (".git/x.lua", "return {}\n"),
            ("notes.txt", "not code\n"),
        ],
    );
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "tree");
    assert_eq!(
        st.s.mods[i].files(),
        ["main.lua", "lib.lua", "lib/util.lua", "shared.lua"]
    );
    assert_eq!(st.s.mods[i].file_count(), 4);
    assert!(
        st.s.mods[mod_index(&st, "hello")].files().is_empty(),
        "a script has none"
    );
    std::fs::write(dir.join("extra.lua"), "return {}\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(st.s.mods[i].files()[1], "extra.lua");
}

#[test]
fn saving_any_file_of_a_module_reloads_it_and_marks_that_files_line() {
    let mut game = FakeGame::new("module-save");
    let mut st = studio(&mut game);
    let dir = write_module(
        &game,
        "pair",
        &[
            (
                "main.lua",
                "-- name: Pair\nlocal lib = require('lib')\ncommand('pair', function() return lib.say() end)\n",
            ),
            (
                "lib.lua",
                "local lib = {}\nfunction lib.say() return 'one' end\nreturn lib\n",
            ),
        ],
    );
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "pair");
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    st.open_file(&dir.join("lib.lua"));
    assert_eq!(st.s.editor.title, "pair/lib.lua");
    let reloads = st.s.mods[i].reloads;
    st.s.editor
        .set_text("local lib = {}\nfunction lib.say() return 'two, and longer' end\nreturn lib\n");
    st.s.editor.dirty = true;
    st.save_editor(&mut game);
    assert_eq!(st.s.mods[i].reloads, reloads + 1);
    assert!(
        log(&st).iter().any(|l| l.starts_with("Reloaded pair")),
        "{:?}",
        log(&st)
    );
    st.command("pair");
    st.frame(&mut game, 0.016);
    assert_eq!(game.chats.last().map(String::as_str), Some("two, and longer"));
    // A runtime error on lib.lua's line 3 marks that line there...
    st.s.editor
        .set_text("local lib = {}\nfunction lib.say() return 'x' end\nerror('broken lib')\nreturn lib\n");
    st.s.editor.dirty = true;
    st.save_editor(&mut game);
    assert!(failure(&st, "pair").0.contains("pair/lib.lua:3: broken lib"));
    assert_eq!(st.s.editor.error_line, Some(3));
    // ...and in main.lua, the line that required it.
    st.open_file(&dir.join("main.lua"));
    assert_eq!(st.s.editor.title, "pair/main.lua");
    assert_eq!(st.s.editor.error_line, Some(2));
    // A syntax error in lib.lua: its own line there, the require in main.lua.
    std::fs::write(dir.join("lib.lua"), "local lib = {}\n\nlocal = 1\nreturn lib\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    st.edit_mod(i);
    assert_eq!(st.s.editor.error_line, Some(2));
    st.open_file(&dir.join("lib.lua"));
    assert_eq!(st.s.editor.error_line, Some(3));
}

/// The tick count in the template module's greeting: "Hello, Sam! (12 ticks so far)".
fn ticks_in(reply: &str) -> u64 {
    reply
        .split('(')
        .nth(1)
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no tick count in {reply:?}"))
}

#[test]
fn a_new_module_has_main_shared_and_lib_and_runs() {
    let mut game = FakeGame::new("new-module");
    let mut st = studio(&mut game);
    let main = st.create_module(&mut game, "Night Light").unwrap();
    let dir = game.mods_dir().join("night_light");
    assert_eq!(main, dir.join("main.lua"));
    for f in ["main.lua", "shared.lua", "lib.lua"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    assert!(
        !game.mods_dir().join(".night_light.creating").exists(),
        "written beside, then moved in"
    );
    let i = mod_index(&st, "night_light");
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    assert_eq!(st.s.mods[i].name, "Night Light");
    assert_eq!(st.s.mods[i].files(), ["main.lua", "lib.lua", "shared.lua"]);
    // Its command answers from lib.lua, with the count main.lua keeps in shared.lua.
    st.command("night_light Sam");
    st.frame(&mut game, 0.016);
    let first = game.chats.last().cloned().unwrap_or_default();
    assert!(first.starts_with("Hello, Sam! ("), "{first}");
    run_for(&mut st, &mut game, 1.0);
    st.command("night_light Sam");
    st.frame(&mut game, 0.016);
    let later = game.chats.last().cloned().unwrap_or_default();
    assert!(ticks_in(&later) > ticks_in(&first), "{first} then {later}");
    // The name is taken now, by either kind.
    assert!(
        st.create_module(&mut game, "Night Light")
            .unwrap_err()
            .contains("night_light already exists")
    );
    assert!(
        st.create_mod(&mut game, "Night Light", None)
            .unwrap_err()
            .contains("already exists")
    );
    assert!(
        st.create_module(&mut game, "Hello")
            .unwrap_err()
            .contains("already exists")
    );
}

#[test]
fn files_are_added_renamed_and_deleted_to_the_trash() {
    let mut game = FakeGame::new("module-file-ops");
    let mut st = studio(&mut game);
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    let i = mod_index(&st, "kit");
    let util = st.add_file(&mut game, &dir, "util").unwrap();
    assert_eq!(util, dir.join("util.lua"));
    let text = std::fs::read_to_string(&util).unwrap();
    assert!(
        text.contains("require(\"util\")") && text.contains("a file of Kit"),
        "{text}"
    );
    assert!(st.s.mods[i].files().contains(&"util.lua".to_owned()));
    for (typed, why) in [
        ("../x", "outside the module"),
        ("main", "main.lua is there already"),
        ("a.b", "dots are for require"),
        ("util", "util.lua is there already"),
    ] {
        let e = st.add_file(&mut game, &dir, typed).unwrap_err();
        assert!(e.contains(why), "{typed}: {e}");
    }
    // The editor has util.lua open with an edit: a rename takes both along.
    st.open_file(&util);
    st.s.editor.insert("-- mine\n");
    let new = st.rename_file(&mut game, &util, "lib/helpers").unwrap();
    assert_eq!(new, dir.join("lib").join("helpers.lua"));
    assert!(!util.exists() && new.is_file());
    assert_eq!(st.s.editor.path.as_ref(), Some(&new));
    assert_eq!(st.s.editor.title, "kit/lib/helpers.lua");
    assert!(st.s.editor.dirty && st.s.editor.text().starts_with("-- mine"));
    assert!(st.s.mods[i].files().contains(&"lib/helpers.lua".to_owned()));
    // main.lua keeps its name.
    let e = st.rename_file(&mut game, &dir.join("main.lua"), "start").unwrap_err();
    assert!(e.contains("main.lua starts the module"), "{e}");
    // Deleted: to the trash with the unsaved edit, the module reloads, the editor shows main.lua.
    let reloads = st.s.mods[i].reloads;
    let moved = st.delete_path(&mut game, &new).unwrap();
    assert_eq!(
        moved,
        game.dir.join("trash").join("kit").join("lib").join("helpers.lua")
    );
    assert!(std::fs::read_to_string(&moved).unwrap().starts_with("-- mine"));
    assert!(
        !new.exists() && !dir.join("lib").exists(),
        "its emptied folder went too"
    );
    assert_eq!(st.s.mods[i].reloads, reloads + 1);
    assert_eq!(st.s.editor.title, "kit/main.lua");
    assert!(!st.s.editor.dirty);
    // A newer copy replaces the older one in the trash.
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib").join("helpers.lua"), "return 2\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    st.delete_path(&mut game, &dir.join("lib").join("helpers.lua")).unwrap();
    assert_eq!(std::fs::read_to_string(&moved).unwrap(), "return 2\n");
    // main.lua alone can't go; the whole module can.
    let e = st.delete_path(&mut game, &dir.join("main.lua")).unwrap_err();
    assert!(e.contains("delete the module instead"), "{e}");
    let old = st.add_file(&mut game, &dir, "old").unwrap();
    st.delete_path(&mut game, &old).unwrap();
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    std::fs::write(dir.join("lib").join("helpers.lua"), "return 3, newer\n").unwrap();
    run_for(&mut st, &mut game, 0.6);
    let moved = st.delete_path(&mut game, &dir).unwrap();
    assert_eq!(moved, game.dir.join("trash").join("kit"));
    assert!(!dir.exists() && moved.join("main.lua").is_file());
    // It joins what was deleted from it before: the newest copy of each file.
    assert!(moved.join("old.lua").is_file(), "deleted before, it stays");
    assert_eq!(
        std::fs::read_to_string(moved.join("lib").join("helpers.lua")).unwrap(),
        "return 3, newer\n"
    );
    assert!(log(&st).contains(&"Removed kit".to_owned()), "{:?}", log(&st));
    assert_eq!(st.s.editor.title, "scratch.lua");
    // So can a script.
    let hello = game.mods_dir().join("hello.lua");
    st.delete_path(&mut game, &hello).unwrap();
    assert!(!hello.exists() && game.dir.join("trash").join("hello.lua").is_file());
    assert!(!st.s.mods.iter().any(|m| m.id == "hello"));
}

#[test]
fn a_script_becomes_a_module_and_the_editor_follows() {
    let mut game = FakeGame::new("make-module");
    let mut st = studio(&mut game);
    game.write_mod("x.lua", "-- name: X\nprint('x runs')\n");
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "x");
    st.edit_mod(i);
    st.s.editor.insert("-- edited\n");
    assert!(st.s.editor.dirty);
    let main = st.make_module(&mut game, i).unwrap();
    let dir = game.mods_dir().join("x");
    assert_eq!(main, dir.join("main.lua"));
    assert!(main.is_file() && !game.mods_dir().join("x.lua").exists());
    let j = mod_index(&st, "x");
    assert!(st.s.mods[j].folder.is_some());
    assert!(st.s.mods[j].running(), "{:?}", st.s.mods[j].state);
    assert_eq!(st.s.mods[j].name, "X");
    assert_eq!(st.s.editor.path.as_ref(), Some(&main));
    assert_eq!(st.s.editor.title, "x/main.lua");
    assert!(st.s.editor.dirty, "the edit is still the editor's");
    st.save_editor(&mut game);
    assert!(std::fs::read_to_string(&main).unwrap().starts_with("-- edited"));
    assert!(!game.mods_dir().join("x.lua").exists(), "the save went to the module");
    let e = st.make_module(&mut game, j).unwrap_err();
    assert!(e.contains("x is a module already"), "{e}");
    // A script turned off stays off as a module.
    game.write_mod("y.lua", "print('y runs')\n");
    run_for(&mut st, &mut game, 0.6);
    let k = mod_index(&st, "y");
    st.set_enabled(&mut game, k, false);
    st.make_module(&mut game, k).unwrap();
    let k = mod_index(&st, "y");
    assert!(st.s.mods[k].folder.is_some() && !st.s.mods[k].enabled);
    assert_eq!(st.s.mods[k].state, ModState::Off);
    assert!(
        std::fs::read_to_string(game.dir.join("studio.txt"))
            .unwrap()
            .contains("off:y")
    );
    // A folder of that name with files in it is not taken over.
    game.write_mod("z.lua", "print('z')\n");
    std::fs::create_dir_all(game.mods_dir().join("z")).unwrap();
    std::fs::write(game.mods_dir().join("z").join("notes.txt"), "mine").unwrap();
    run_for(&mut st, &mut game, 0.6);
    let z = mod_index(&st, "z");
    assert!(st.make_module(&mut game, z).unwrap_err().contains("files in it"));
    assert!(game.mods_dir().join("z.lua").is_file());
}

#[test]
fn a_script_made_a_module_elsewhere_takes_the_editor_along() {
    use horizon_studio::headless::write_mod;
    let mut game = FakeGame::new("convert-elsewhere");
    let mut st = studio(&mut game);
    let mods = game.mods_dir();
    game.write_mod("solo.lua", "-- name: Solo\nprint('one file')\n");
    run_for(&mut st, &mut game, 0.6);
    st.edit_mod(mod_index(&st, "solo"));
    // An agent's write_mod makes the open script a module: the editor goes
    // to its main.lua and reads what the agent wrote there.
    let files = [
        ("main.lua".to_owned(), "print('a module now, and longer')\n".to_owned()),
        ("lib.lua".to_owned(), "return {}\n".to_owned()),
    ];
    write_mod(&mods, "solo", &files).unwrap();
    run_for(&mut st, &mut game, 0.6);
    let main = mods.join("solo").join("main.lua");
    assert_eq!(st.s.editor.path.as_ref(), Some(&main));
    assert_eq!(st.s.editor.title, "solo/main.lua");
    assert_eq!(st.s.editor.text(), "print('a module now, and longer')\n");
    assert!(!st.s.editor.dirty && !st.s.editor.conflict);
    // A save goes to the module, and no solo.lua comes back beside it.
    st.s.editor.insert("-- my edit\n");
    st.save_editor(&mut game);
    run_for(&mut st, &mut game, 0.6);
    assert!(!mods.join("solo.lua").exists());
    assert!(std::fs::read_to_string(&main).unwrap().starts_with("-- my edit"));
    let i = mod_index(&st, "solo");
    assert!(st.s.mods[i].folder.is_some() && st.s.mods[i].running());
    // With unsaved edits they come along, and as main.lua is not what the
    // editor read, it asks first (Reload, or Keep mine).
    game.write_mod("duo.lua", "print('duo')\n");
    run_for(&mut st, &mut game, 0.6);
    st.edit_mod(mod_index(&st, "duo"));
    st.s.editor.insert("-- unsaved\n");
    let files = [("main.lua".to_owned(), "print('the agent wrote this')\n".to_owned())];
    write_mod(&mods, "duo", &files).unwrap();
    run_for(&mut st, &mut game, 0.6);
    let duo = mods.join("duo").join("main.lua");
    assert_eq!(st.s.editor.path.as_ref(), Some(&duo));
    assert!(st.s.editor.dirty && st.s.editor.conflict);
    assert_eq!(st.s.editor.text(), "-- unsaved\nprint('duo')\n");
    st.save_editor(&mut game);
    assert!(!mods.join("duo.lua").exists());
    assert!(
        std::fs::read_to_string(&duo)
            .unwrap()
            .starts_with("-- unsaved\nprint('duo')")
    );
    // Moved in a file manager, the same file: no question.
    game.write_mod("tri.lua", "print('tri')\n");
    run_for(&mut st, &mut game, 0.6);
    st.edit_mod(mod_index(&st, "tri"));
    st.s.editor.insert("-- mine\n");
    std::fs::create_dir_all(mods.join("tri")).unwrap();
    std::fs::rename(mods.join("tri.lua"), mods.join("tri").join("main.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(st.s.editor.title, "tri/main.lua");
    assert!(st.s.editor.dirty && !st.s.editor.conflict);
}

#[test]
fn a_file_removed_elsewhere_gives_way_or_keeps_its_edits() {
    use horizon_studio::headless::remove_files;
    let mut game = FakeGame::new("removed-elsewhere");
    let mut st = studio(&mut game);
    let mods = game.mods_dir();
    st.create_module(&mut game, "Kit").unwrap();
    let dir = mods.join("kit");
    for f in ["old", "gone", "back"] {
        std::fs::write(dir.join(format!("{f}.lua")), format!("return '{f}'\n")).unwrap();
    }
    run_for(&mut st, &mut game, 0.6);
    // Open with nothing unsaved: removed by an agent's write_mod, main.lua shows.
    st.open_file(&dir.join("old.lua"));
    remove_files(&mods, "kit", &["old.lua".to_owned()]).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(st.s.editor.title, "kit/main.lua");
    assert!(!st.s.editor.removed);
    // With unsaved edits: they stay, marked as removed, and nothing writes it back.
    st.open_file(&dir.join("gone.lua"));
    st.s.editor.insert("-- mine\n");
    remove_files(&mods, "kit", &["gone.lua".to_owned()]).unwrap();
    run_for(&mut st, &mut game, 1.2);
    assert_eq!(st.s.editor.title, "kit/gone.lua");
    assert!(st.s.editor.removed && st.s.editor.dirty);
    assert!(st.s.editor.text().starts_with("-- mine"));
    assert!(!dir.join("gone.lua").exists());
    // Let go (the banner's Close): main.lua opens.
    st.close_removed();
    assert_eq!(st.s.editor.title, "kit/main.lua");
    assert!(!st.s.editor.removed && !st.s.editor.dirty);
    // Kept: a save writes it back, and the module has it again.
    st.open_file(&dir.join("back.lua"));
    st.s.editor.insert("-- keep this\n");
    remove_files(&mods, "kit", &["back.lua".to_owned()]).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.editor.removed);
    st.save_editor(&mut game);
    assert!(!st.s.editor.removed);
    assert!(
        std::fs::read_to_string(dir.join("back.lua"))
            .unwrap()
            .starts_with("-- keep this")
    );
    assert!(
        st.s.mods[mod_index(&st, "kit")]
            .files()
            .contains(&"back.lua".to_owned())
    );
    // A whole mod gone: the scratch file.
    st.edit_mod(mod_index(&st, "hello"));
    std::fs::remove_file(mods.join("hello.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(st.s.editor.title, "scratch.lua");
}

#[test]
fn a_delete_whose_save_fails_deletes_nothing() {
    let mut game = FakeGame::new("delete-save-fails");
    let mut st = studio(&mut game);
    st.create_module(&mut game, "Kit").unwrap();
    let lib = game.mods_dir().join("kit").join("lib.lua");
    st.open_file(&lib);
    st.s.editor.insert("-- my unsaved work\n");
    // The save will fail: a folder stands where the file was.
    std::fs::remove_file(&lib).unwrap();
    std::fs::create_dir_all(&lib).unwrap();
    let e = st.delete_path(&mut game, &lib).unwrap_err();
    assert!(
        e.contains("could not save kit/lib.lua") && e.contains("nothing was deleted"),
        "{e}"
    );
    assert!(lib.is_dir() && !game.dir.join("trash").exists());
    assert_eq!(st.s.editor.title, "kit/lib.lua");
    assert!(st.s.editor.dirty && st.s.editor.text().starts_with("-- my unsaved work"));
}

#[test]
fn a_conflict_moves_with_a_renamed_file() {
    let mut game = FakeGame::new("rename-conflict");
    let mut st = studio(&mut game);
    st.create_module(&mut game, "Kit").unwrap();
    let dir = game.mods_dir().join("kit");
    let util = st.add_file(&mut game, &dir, "util").unwrap();
    st.open_file(&util);
    st.s.editor.insert("-- mine\n");
    // Someone else writes the file meanwhile: a conflict.
    std::fs::write(&util, "-- theirs, a longer line than before\nreturn {}\n").unwrap();
    st.s.editor.sync_disk();
    assert!(st.s.editor.conflict);
    let new = st.rename_file(&mut game, &util, "helpers").unwrap();
    assert_eq!(st.s.editor.path.as_ref(), Some(&new));
    assert!(st.s.editor.conflict, "the change elsewhere moved with the file");
    assert!(st.s.editor.dirty && st.s.editor.text().starts_with("-- mine"));
}

#[test]
fn console_new_and_edit_know_modules() {
    use horizon_studio::game::hstudio_ui::Tab;
    let mut game = FakeGame::new("console-modules");
    let mut st = studio(&mut game);
    console(&mut st, &mut game, "new('Tool', 'module')");
    assert!(game.mods_dir().join("tool").join("main.lua").is_file());
    assert_eq!(st.s.editor.title, "tool/main.lua");
    assert_eq!(st.s.ui.tab, Tab::Editor);
    // Unsaved edits are saved before edit() opens another file.
    st.s.editor.insert("-- unsaved in main\n");
    console(&mut st, &mut game, "edit('tool', 'lib.lua')");
    assert_eq!(st.s.editor.title, "tool/lib.lua");
    let main = game.mods_dir().join("tool").join("main.lua");
    assert!(
        std::fs::read_to_string(&main)
            .unwrap()
            .starts_with("-- unsaved in main")
    );
    console(&mut st, &mut game, "edit('tool', 'shared')");
    assert_eq!(st.s.editor.title, "tool/shared.lua");
    console(&mut st, &mut game, "edit('tool')");
    assert_eq!(st.s.editor.title, "tool/main.lua");
    let out = console(&mut st, &mut game, "new('X', 'folder')");
    assert!(
        out.iter().any(|l| l.contains("\"script\" or \"module\" expected")),
        "{out:?}"
    );
    assert!(!game.mods_dir().join("x.lua").exists() && !game.mods_dir().join("x").exists());
    let out = console(&mut st, &mut game, "edit('tool', 'nope.lua')");
    assert!(out.iter().any(|l| l.contains("no nope.lua in tool")), "{out:?}");
    let out = console(&mut st, &mut game, "edit('hello', 'x.lua')");
    assert!(out.iter().any(|l| l.contains("'hello' is a script")), "{out:?}");
    // And before new() makes and opens one.
    st.s.editor.insert("-- unsaved in tool\n");
    console(&mut st, &mut game, "new('Plain')");
    assert_eq!(st.s.editor.title, "plain.lua");
    assert!(
        std::fs::read_to_string(game.mods_dir().join("tool").join("main.lua"))
            .unwrap()
            .starts_with("-- unsaved in tool")
    );
    let out = console(&mut st, &mut game, "new('Plain')");
    assert!(out.iter().any(|l| l.contains("plain already exists")), "{out:?}");
}

#[test]
fn running_a_modules_file_reloads_the_module() {
    let mut game = FakeGame::new("run-module-file");
    let mut st = studio(&mut game);
    st.create_module(&mut game, "Runner").unwrap();
    let dir = game.mods_dir().join("runner");
    let i = mod_index(&st, "runner");
    st.open_file(&dir.join("lib.lua"));
    // Unchanged: it reloads.
    let before = log(&st).len();
    st.run_editor(&mut game);
    let after: Vec<String> = log(&st)[before..].to_vec();
    assert!(after.contains(&"Ran runner (reloaded)".to_owned()), "{after:?}");
    assert!(!after.iter().any(|l| l.contains("require works")), "{after:?}");
    assert!(st.s.mods[i].running());
    // With an edit: it is saved, and the module runs it.
    st.s.editor
        .set_text("print('lib ran, edited')\nreturn { greet = function() return 'hi' end }\n");
    st.s.editor.dirty = true;
    st.run_editor(&mut game);
    assert!(!st.s.editor.dirty);
    assert!(
        std::fs::read_to_string(dir.join("lib.lua"))
            .unwrap()
            .contains("lib ran, edited")
    );
    assert!(
        log(&st).contains(&"[runner] lib ran, edited".to_owned()),
        "{:?}",
        log(&st)
    );
    // A script still runs in the console.
    st.edit_mod(mod_index(&st, "hello"));
    st.s.editor.set_text("print('in the console ' .. 6 * 7)");
    st.run_editor(&mut game);
    assert_eq!(log(&st).last().map(String::as_str), Some("in the console 42"));
}

#[test]
fn typed_file_names_become_paths_in_the_module() {
    use horizon_studio::game::hstudio_modfiles::{file_name, plan_file, plan_mod, require_name, require_path};
    assert_eq!(file_name("util"), Ok("util.lua".to_owned()));
    assert_eq!(file_name(" lib/util.lua "), Ok("lib/util.lua".to_owned()));
    assert_eq!(file_name("lib\\util"), Ok("lib/util.lua".to_owned()));
    assert_eq!(file_name("Util"), Ok("Util.lua".to_owned()));
    for bad in ["lib.util", "", "/a", ".a", "a b", "a/b/c/d/e", "a//b", "../x", "x?"] {
        assert!(file_name(bad).is_err(), "{bad:?} was taken");
    }
    assert!(file_name("lib.util").unwrap_err().contains("dots are for require"));
    assert!(file_name("a b").unwrap_err().contains("no spaces"));
    assert_eq!(require_path("lib.util").as_deref(), Some("lib/util.lua"));
    assert_eq!(require_path("lib/util.lua").as_deref(), Some("lib/util.lua"));
    assert_eq!(require_name("lib/util.lua"), "lib.util");
    for bad in ["", "/x", ".x", "a..b", "a/b/c/d/e", "a b", "../x"] {
        assert_eq!(require_path(bad), None, "{bad:?}");
    }
    let game = FakeGame::new("plans");
    let mods = game.mods_dir();
    std::fs::create_dir_all(mods.join("night_light")).unwrap();
    std::fs::write(mods.join("taken.lua"), "").unwrap();
    std::fs::write(mods.join("night_light").join("main.lua"), "").unwrap();
    assert_eq!(plan_mod(&mods, "  ", true), Ok(String::new()));
    assert_eq!(
        plan_mod(&mods, "Night Watch", true).as_deref(),
        Ok("night_watch/: main.lua, shared.lua and lib.lua")
    );
    assert_eq!(
        plan_mod(&mods, "Night Watch", false).as_deref(),
        Ok("night_watch.lua: one file")
    );
    assert_eq!(plan_mod(&mods, "Taken", true), Err("taken already exists".to_owned()));
    assert_eq!(
        plan_mod(&mods, "Night Light", false),
        Err("night_light already exists".to_owned())
    );
    let folder = mods.join("night_light");
    assert_eq!(
        plan_file(&folder, "util", None).as_deref(),
        Ok("night_light/util.lua: require(\"util\") loads it")
    );
    assert_eq!(
        plan_file(&folder, "lib/util", None).as_deref(),
        Ok("night_light/lib/util.lua: require(\"lib.util\") loads it")
    );
    assert_eq!(
        plan_file(&folder, "main", None),
        Err("main.lua is there already".to_owned())
    );
    assert!(plan_file(&folder, "a.b", None).unwrap_err().contains("dots"));
    assert_eq!(plan_file(&folder, "", None), Ok(String::new()));
    let rename = plan_file(&folder, "helpers", Some("util.lua")).unwrap();
    assert!(
        rename.contains("require(\"helpers\") loads it") && rename.contains("require(\"util\")"),
        "{rename}"
    );
    assert_eq!(plan_file(&folder, "util", Some("util.lua")), Ok(String::new()));
}

#[test]
fn turned_off_mods_stay_off() {
    let mut game = FakeGame::new("toggle");
    let mut st = studio(&mut game);
    let i = mod_index(&st, "hello");
    st.set_enabled(&mut game, i, false);
    assert_eq!(st.s.mods[i].state, ModState::Off);
    assert!(!st.command("hello"));
    let mut again = Studio::new();
    again.frame(&mut game, 0.016);
    let i = mod_index(&again, "hello");
    assert!(!again.s.mods[i].enabled);
    assert_eq!(again.s.mods[i].state, ModState::Off);
    assert_eq!(again.s.mods[i].name, "Hello", "an off mod still shows its name");
}

#[test]
fn headers_and_ids() {
    let h = read_header("-- name: Thing\n-- Version: 2\n--description: does it\nprint(1)\n-- author: late");
    assert_eq!(
        h,
        vec![
            ("name".into(), "Thing".into()),
            ("version".into(), "2".into()),
            ("description".into(), "does it".into())
        ]
    );
    assert_eq!(mod_id("My Cool Mod!"), "my_cool_mod_");
    assert_eq!(mod_id(""), "mod");
}

#[test]
fn every_example_loads_and_runs() {
    let mut game = FakeGame::new("examples");
    game.agent_now
        .insert("find-block".into(), serde_json::json!({"count": 0, "nearest": []}));
    let mut st = studio(&mut game);
    for (i, ex) in EXAMPLES.iter().enumerate() {
        // hello, uikit and viahorizon come with a first start.
        if !["hello", "uikit", "viahorizon"].contains(&ex.id) {
            st.install_example(&mut game, i).unwrap();
        }
    }
    run_for(&mut st, &mut game, 1.5);
    for _ in 0..3 {
        st.hud(&mut game);
    }
    for m in &st.s.mods {
        assert_eq!(m.state, ModState::Running, "{} failed", m.id);
        let ex = EXAMPLES.iter().find(|e| e.id == m.id).unwrap();
        assert_eq!(m.name, ex.title);
        assert!(!m.description.is_empty());
    }
    for cmd in [
        "coords",
        "zoom 3",
        "note remember this",
        "notes",
        "ores 8",
        "mine stone 1",
        "hello",
    ] {
        assert!(st.command(cmd), "{cmd}");
    }
    run_for(&mut st, &mut game, 0.2);
    assert!(st.command("mine stop"));
    run_for(&mut st, &mut game, 0.1);
    for m in &st.s.mods {
        assert_eq!(m.state, ModState::Running, "{} failed after its commands", m.id);
    }
    assert!(game.chats.iter().any(|c| c == "Zoom set to 3x"), "{:?}", game.chats);
    assert!(
        game.chats.iter().any(|c| c.starts_with("[Notes] 1. remember this")),
        "{:?}",
        game.chats
    );
    assert!(
        game.chats.iter().any(|c| c == "No ores within 8 blocks"),
        "{:?}",
        game.chats
    );
    assert!(
        game.chats
            .iter()
            .any(|c| c == "[Miner] No more minecraft:stone within 32 blocks"),
        "{:?}",
        game.chats
    );
    assert!(
        game.chats.iter().any(|c| c == "Mining 1 minecraft:stone"),
        "{:?}",
        game.chats
    );
    assert!(
        game.drawn.iter().any(|d| d.contains("XYZ 10 / 64 / -4")),
        "coords drew its panel"
    );
}

#[test]
fn examples_rs_matches_the_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = std::fs::read_dir(root.join("examples"))
        .unwrap()
        .filter_map(|e| e.ok()?.path().file_stem()?.to_str().map(str::to_owned))
        .collect();
    files.sort();
    let mut ids: Vec<String> = EXAMPLES.iter().map(|e| e.id.to_owned()).collect();
    ids.sort();
    assert_eq!(files, ids, "run python3 tools/gen_examples.py");
    for e in EXAMPLES {
        let src = std::fs::read_to_string(root.join(format!("examples/{}.lua", e.id))).unwrap();
        assert_eq!(e.source, src, "{} is stale: run python3 tools/gen_examples.py", e.id);
    }
}

#[test]
fn the_readme_documents_every_function() {
    use horizon_studio::game::hstudio::{API, CONSOLE_API};
    let readme = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    for (table, name, _, _) in API {
        let full = if table.is_empty() {
            format!("`{name}(")
        } else {
            format!("{table}.{name}(")
        };
        assert!(readme.contains(&full), "README.md does not document {full}");
    }
    for (name, _, _) in CONSOLE_API {
        assert!(
            readme.contains(&format!("`{name}(")),
            "README.md does not document {name}()"
        );
    }
}

fn zip_of(path: &std::path::Path, files: &[(&str, &str)]) {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, body) in files {
        z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}

#[test]
fn dropped_files_install_and_run() {
    let mut game = FakeGame::new("drop");
    let mut st = studio(&mut game);
    let downloads = game.dir.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    // A .lua file.
    let file = downloads.join("Sky Light.lua");
    std::fs::write(&file, "-- name: Sky light\nprint('sky v1')\n").unwrap();
    assert_eq!(
        st.install_path(&mut game, &file).unwrap(),
        "Installed sky_light: running"
    );
    assert!(log(&st).contains(&"[sky_light] sky v1".to_owned()));
    std::fs::write(&file, "-- name: Sky light\nprint('sky v2, longer')\n").unwrap();
    assert_eq!(st.install_path(&mut game, &file).unwrap(), "Updated sky_light: running");
    assert!(log(&st).contains(&"[sky_light] sky v2, longer".to_owned()));
    // A folder with main.lua, links and hidden files left out.
    let folder = downloads.join("Tools");
    std::fs::create_dir_all(folder.join("lib")).unwrap();
    std::fs::write(folder.join("main.lua"), "print(require('lib.x'))\n").unwrap();
    std::fs::write(folder.join("lib/x.lua"), "return 'from lib'\n").unwrap();
    std::fs::write(folder.join(".secret"), "no").unwrap();
    assert_eq!(st.install_path(&mut game, &folder).unwrap(), "Installed tools: running");
    assert!(log(&st).contains(&"[tools] from lib".to_owned()), "{:?}", log(&st));
    assert!(!game.mods_dir().join("tools/.secret").exists());
    // Zips: main.lua at the root, in one top folder, or a single .lua.
    let z1 = downloads.join("rootmod.zip");
    zip_of(&z1, &[("main.lua", "print('root zip')"), ("util.lua", "return 1")]);
    assert_eq!(st.install_path(&mut game, &z1).unwrap(), "Installed rootmod: running");
    let z2 = downloads.join("download (3).zip");
    zip_of(
        &z2,
        &[
            ("Nested Mod/main.lua", "print('nested zip')"),
            ("__MACOSX/._main.lua", "junk"),
        ],
    );
    assert_eq!(
        st.install_path(&mut game, &z2).unwrap(),
        "Installed nested_mod: running"
    );
    let z3 = downloads.join("whatever.zip");
    zip_of(&z3, &[("readme.txt", "hi"), ("solo.lua", "print('solo')")]);
    assert_eq!(st.install_path(&mut game, &z3).unwrap(), "Installed solo: running");
    for line in ["[rootmod] root zip", "[nested_mod] nested zip", "[solo] solo"] {
        assert!(log(&st).contains(&line.to_owned()), "{line}: {:?}", log(&st));
    }
    // A broken mod installs, and says why it doesn't run.
    let bad = downloads.join("bad.lua");
    std::fs::write(&bad, "error('nope')").unwrap();
    assert_eq!(
        st.install_path(&mut game, &bad).unwrap(),
        "Installed bad, but it failed: bad.lua:1: nope"
    );
    // What is not a script mod is refused.
    let other = downloads.join("pack.txt");
    std::fs::write(&other, "x").unwrap();
    assert!(
        st.install_path(&mut game, &other)
            .unwrap_err()
            .contains("installs .lua files")
    );
    let empty = downloads.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(st.install_path(&mut game, &empty).unwrap_err().contains("no main.lua"));
    let z4 = downloads.join("two.zip");
    zip_of(&z4, &[("a.lua", ""), ("b.lua", "")]);
    assert!(
        st.install_path(&mut game, &z4)
            .unwrap_err()
            .contains("not a script mod")
    );
    let z5 = downloads.join("evil.zip");
    zip_of(&z5, &[("../../escape.lua", "print(1)")]);
    assert!(st.install_path(&mut game, &z5).is_err());
    assert!(!game.dir.join("escape.lua").exists());
    // A one-file and a folder mod of the same name don't mix.
    let clash = downloads.join("tools.lua");
    std::fs::write(&clash, "print(1)").unwrap();
    assert!(
        st.install_path(&mut game, &clash)
            .unwrap_err()
            .contains("folder mod named tools")
    );
}

#[test]
fn the_library_opens_the_mods_folder() {
    use horizon_studio::game::hstudio_ui::{Input, Tab, draw, handle};
    let mut game = FakeGame::new("open-folder");
    let mut st = studio(&mut game);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Library;
    draw(&mut st, &mut game);
    let label = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text Open folder @"))
        .cloned()
        .expect("button drawn");
    let at = label
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap()
        .to_owned();
    let xy: Vec<f32> = at.split(", ").map(|v| v.parse().unwrap()).collect();
    handle(&mut st, &mut game, Input::Mouse(xy[0] + 2.0, xy[1] + 2.0));
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(game.opened, vec![game.mods_dir()]);
}

#[test]
fn colors_in_every_written_form() {
    use horizon_studio::game::hstudio::color_arg;
    use horizon_studio::game::hstudio_lua::{Lua, Value};
    let lua = Lua::new();
    let c = |v: Value| color_arg(&lua, &[v], 0, 0);
    assert_eq!(c(Value::str("#fff")).ok(), Some(0xFFFF_FFFF), "#rgb is #rrggbb");
    assert_eq!(c(Value::str("#c0392b")).ok(), Some(0xFFC0_392B));
    assert_eq!(c(Value::str("80ff0000")).ok(), Some(0x80FF_0000));
    assert_eq!(c(Value::Num(f64::from(0x40FF_FFFFu32))).ok(), Some(0x40FF_FFFF));
    assert_eq!(c(Value::Nil).ok(), Some(0));
    for bad in ["#ff", "#ffff", "#fffff", "#gggggg"] {
        assert!(c(Value::str(bad)).is_err(), "{bad}");
    }
}
