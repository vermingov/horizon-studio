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
    assert_eq!(out, vec!["{ hello = \"running\" }"]);
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
    assert!(out[0].contains("require works in folder mods"), "{out:?}");
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
        if ex.id != "hello" {
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
