// horizon-studio-mcp: its MCP side, its headless tests of mods, and an agent
// driving a game (the studio on a fake game, with the AI link polled each
// frame as the bridge does) through the link, as it would the real one.

mod common;

use common::FakeGame;
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_link as link;
use horizon_studio::headless::{Run, Source, run_steps};
use horizon_studio::mcp::{Server, handle};
use serde_json::{Value as Json, json};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const GREETER: &str = r#"-- name: Greeter
local count = 0
command("greet", function(rest, argv)
  count = count + 1
  chat.show("Hello, " .. (argv[1] or "you") .. "!")
end, "greets someone")
on("key", function(key, down)
  if key == "key.keyboard.j" and down then chat.show("jumped") end
end)
export{ count = function() return count end }
"#;

/// A tool's result as JSON (its text parsed), and whether it was an error.
fn call(server: &mut Server, name: &str, args: Json) -> (Json, bool) {
    let r = server.call_tool(name, &args);
    let text = r["content"][0]["text"].as_str().unwrap_or("").to_owned();
    let error = r["isError"] == true;
    (serde_json::from_str(&text).unwrap_or(Json::String(text)), error)
}

fn files(name: &str, src: &str) -> Json {
    json!({ name: src })
}

#[test]
fn it_speaks_mcp() {
    let mut s = Server::new();
    let init = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                 "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } } }),
    )
    .unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "horizon-studio");
    assert!(init["result"]["instructions"].as_str().unwrap().contains("test_mod"));
    // A version it doesn't know: its own.
    let old = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } }),
    )
    .unwrap();
    assert_eq!(old["result"]["protocolVersion"], "2025-06-18");
    assert!(
        handle(
            &mut s,
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        )
        .is_none()
    );
    let tools = handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" })).unwrap();
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for want in [
        "docs",
        "write_mod",
        "check_mod",
        "test_mod",
        "run_lua",
        "game_status",
        "horizon_ctl",
    ] {
        assert!(names.contains(&want), "{want}: {names:?}");
    }
    for t in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
    }
    let missing = handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 4, "method": "nope" })).unwrap();
    assert_eq!(missing["error"]["code"], -32601);
    // The docs: an index, topics, entries by name, a search, and the topics as resources.
    let (index, _) = call(&mut s, "docs", json!({}));
    let index = index.as_str().unwrap();
    assert!(index.contains("`start`") && index.contains("`recipes`") && index.contains("chat.show"));
    let (tests, _) = call(&mut s, "docs", json!({ "topic": "tests" }));
    assert!(tests.as_str().unwrap().contains("expect_chat"));
    let (show, _) = call(&mut s, "docs", json!({ "name": "chat.show()" }));
    assert!(show.as_str().unwrap().starts_with("# chat.show"), "{show}");
    let (found, _) = call(&mut s, "docs", json!({ "search": "radians" }));
    assert!(found.as_str().unwrap().contains("`math.rad`"), "{found}");
    let (nope, error) = call(&mut s, "docs", json!({ "name": "player.teleport" }));
    assert!(error && nope.as_str().unwrap().contains("near: "), "{nope}");
    let (all, _) = call(&mut s, "docs", json!({ "topic": "all" }));
    assert!(all.as_str().unwrap().contains("import(") && all.as_str().unwrap().len() > 100_000);
    let list = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 5, "method": "resources/list" }),
    )
    .unwrap();
    let uris: Vec<&str> = list["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert!(uris.contains(&"horizon-studio://docs/events"), "{uris:?}");
    let read = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 6, "method": "resources/read",
                 "params": { "uri": "horizon-studio://docs/events" } }),
    )
    .unwrap();
    assert!(
        read["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("event:tick")
    );
    let gone = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 7, "method": "resources/read", "params": { "uri": "horizon-studio://docs/nope" } }),
    )
    .unwrap();
    assert_eq!(gone["error"]["code"], -32002);
    let (examples, _) = call(&mut s, "examples", json!({}));
    assert!(examples.as_array().unwrap().iter().any(|e| e["id"] == "uikit"));
}

#[test]
fn check_reports_what_loads_and_where_it_fails() {
    let mut s = Server::new();
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "greeter", "files": files("greeter.lua", GREETER), "mods_dir": "/nonexistent" }),
    );
    assert!(!error, "{r}");
    assert_eq!(r["mod"]["state"], "running");
    assert_eq!(r["mod"]["commands"], json!(["greet"]));
    assert_eq!(r["mod"]["library"], true);
    // A syntax error, and a runtime one in a handler: the line either way.
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "bad", "files": files("bad.lua", "local x = = 1"), "mods_dir": "/nonexistent" }),
    );
    assert!(error);
    assert_eq!(r["mod"]["state"], "failed");
    assert_eq!(r["mod"]["line"], 1, "{r}");
    let (r, _) = call(
        &mut s,
        "check_mod",
        json!({ "id": "bad", "files": files("bad.lua", "\non('tick', function() y.z = 1 end)"), "mods_dir": "/nonexistent" }),
    );
    assert_eq!(r["mod"]["state"], "failed");
    assert_eq!(r["mod"]["line"], 2, "{r}");
    // A folder mod with a library it imports from the bundled ones.
    let main =
        "local kit = import('uikit')\nlocal util = require('util')\ncommand('two', function() return util.two() end)";
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "folder", "files": { "main.lua": main, "util.lua": "return { two = function() return 2 end }" }, "mods_dir": "/nonexistent" }),
    );
    assert!(!error, "{r}");
    assert_eq!(r["libraries"], json!(["uikit"]));
}

#[test]
fn headless_tests_run_commands_keys_lua_and_screens() {
    let source = Source::Files {
        id: "greeter".into(),
        files: vec![("greeter.lua".into(), GREETER.into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let report = run_steps(
        &mut run,
        &[
            json!({ "frames": 5, "expect_state": { "greeter": "running" } }),
            json!({ "command": "/greet Alex", "expect_chat": "Hello, Alex" }),
            json!({ "lua": "local g = import('greeter'); assert(g.count() == 1, 'counted once'); return g.count() * 10", "expect_output": "10" }),
            json!({ "key": "key.keyboard.j", "expect_chat": "jumped" }),
            json!({ "seconds": 1 }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
    assert_eq!(report["summary"], "5/5 steps passed");
    // Failures say what went wrong.
    let report = run_steps(
        &mut run,
        &[
            json!({ "command": "/greet Bo", "expect_chat": "Hello, Alex" }),
            json!({ "lua": "error('boom')" }),
            json!({ "lua": "error('boom')", "expect_error": "boom" }),
            json!({ "command": "/nothing" }),
        ],
    );
    assert_eq!(report["ok"], false);
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(
        steps.iter().map(|s| s["ok"] == true).collect::<Vec<_>>(),
        [false, false, true, false]
    );
    assert!(steps[0]["failures"][0].as_str().unwrap().contains("Hello, Alex"));
    assert!(steps[1]["failures"][0].as_str().unwrap().contains("boom"));
    assert!(steps[3]["failures"][0].as_str().unwrap().contains("/nothing"));

    // A screen: open it, click a button by its label, see the sound and particles.
    let playground = horizon_studio::game::hstudio_examples::EXAMPLES
        .iter()
        .find(|e| e.id == "playground")
        .unwrap();
    let source = Source::Files {
        id: "playground".into(),
        files: vec![("playground.lua".into(), playground.source.into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let report = run_steps(
        &mut run,
        &[
            json!({ "command": "/playground", "expect_screen": true, "expect_drawn": ["Playground", "Ring the bell"] }),
            json!({ "click": "Ring the bell", "expect_sound": "bell", "expect_particle": "minecraft:note" }),
            json!({ "click": "Close", "expect_screen": false }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
    // The player and blocks a test sets are what the mods see.
    let source = Source::Files {
        id: "look".into(),
        files: vec![(
            "look.lua".into(),
            "command('where', function() local x, y, z = player.pos(); return x .. ' ' .. world.block(1, 63, 2) end)"
                .into(),
        )],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let report = run_steps(
        &mut run,
        &[
            json!({ "player": { "x": 5, "y": 70, "z": -2 } }),
            json!({ "block": [1, 63, 2], "id": "minecraft:diamond_ore" }),
            json!({ "command": "/where", "expect_chat": "5 minecraft:diamond_ore" }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
}

#[test]
fn a_suggest_step_shows_the_completions() {
    let source = Source::Files {
        id: "wp".into(),
        files: vec![(
            "wp.lua".into(),
            "command('wp', function() end, { args = {
              { name = 'action', options = { 'add', { text = 'remove', tooltip = 'Forgets one' } } },
              { name = 'name', kind = 'text', optional = true, suggest = function() return { 'home', 'mine' } end },
            } })"
                .into(),
        )],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let report = run_steps(
        &mut run,
        &[
            json!({ "suggest": "/wp re", "expect_output": "remove (Forgets one)" }),
            json!({ "suggest": "/wp add ", "expect_output": ["home", "mine"] }),
            json!({ "suggest": "wp ", "expect_output": "add" }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
    assert_eq!(report["steps"][0]["output"], json!(["remove (Forgets one)"]));
    // Failures say what went wrong.
    let report = run_steps(
        &mut run,
        &[
            json!({ "suggest": "/nothing x" }),
            json!({ "suggest": "/wp" }),
            json!({ "suggest": "/wp re", "expect_output": "add" }),
        ],
    );
    let steps = report["steps"].as_array().unwrap();
    assert!(steps.iter().all(|s| s["ok"] == false), "{report:#}");
    let failure = |i: usize| steps[i]["failures"][0].as_str().unwrap().to_owned();
    assert!(failure(0).contains("no mod completes /nothing"), "{}", failure(0));
    assert!(failure(1).contains("\"/wp \", not \"/wp\""), "{}", failure(1));
    assert!(failure(2).contains("the output has no 'add'"), "{}", failure(2));
}

#[test]
fn an_agent_writes_tests_and_runs_a_mod_in_the_linked_game() {
    let home = std::env::temp_dir().join(format!("hstudio-mcp-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let file = home.join("link.json");
    // SAFETY: this is the only test that reads or changes these variables.
    unsafe {
        std::env::set_var("HORIZON_STUDIO_LINK", &file);
        std::env::remove_var("HORIZON_STUDIO_MODS");
    }
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (mods_tx, mods_rx) = mpsc::channel();
    // The game's thread: the studio's frame, what it sends as the player and
    // shows in chat kept, then the link. As in the game, a line sent goes to
    // the server: only a command typed in the chat reaches the studio's hook.
    let game = std::thread::spawn(move || {
        let mut game = FakeGame::new("mcp-link");
        let mut st = Studio::new();
        st.frame(&mut game, 0.016);
        mods_tx.send(game.mods_dir()).unwrap();
        let (mut chats, mut sent) = (0, 0);
        while stop_rx.try_recv().is_err() {
            game.ticks += 1;
            st.frame(&mut game, 0.016);
            for line in &game.sent[sent..] {
                link::record("sent", line);
            }
            sent = game.sent.len();
            for line in &game.chats[chats..] {
                link::record("out", line);
            }
            chats = game.chats.len();
            link::poll(&mut st, &mut game);
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    let mods_dir = mods_rx.recv().unwrap();
    let start = Instant::now();
    while !file.exists() {
        assert!(start.elapsed() < Duration::from_secs(5), "the link file never came");
        std::thread::sleep(Duration::from_millis(10));
    }
    let info: Json = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(info["mods_dir"], json!(mods_dir));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600,
            "only its user reads it"
        );
    }
    // A wrong token is turned away.
    {
        use std::io::{BufRead, BufReader, Write};
        let mut bad = std::net::TcpStream::connect(("127.0.0.1", info["port"].as_u64().unwrap() as u16)).unwrap();
        bad.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        bad.write_all(b"{\"auth\": \"nope\"}\n{\"id\": 1, \"method\": \"status\"}\n")
            .unwrap();
        let mut line = String::new();
        BufReader::new(bad).read_line(&mut line).unwrap();
        assert!(line.contains("error"), "{line}");
    }

    let mut s = Server::new();
    handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "clientInfo": { "name": "claude-code", "version": "2.1" } } }),
    );
    let (status, _) = call(&mut s, "game_status", json!({}));
    assert_eq!(status["linked"], true, "{status}");
    assert_eq!(status["game"]["screen"], "playing");
    assert_eq!(
        status["game"]["agents"],
        json!(["Claude Code"]),
        "the game knows who is connected"
    );
    // A second agent session keeps its link by itself, and shows in the game too.
    let keeper = Server::new();
    keeper.set_client("omp");
    keeper.keep_linked();
    let start = Instant::now();
    loop {
        let (status, _) = call(&mut s, "game_status", json!({}));
        if status["game"]["agents"] == json!(["Claude Code", "omp"]) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "{status}");
        std::thread::sleep(Duration::from_millis(100));
    }
    // Writing a mod: checked headless, written where the game reads, reloaded there.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "greeter", "files": files("greeter.lua", GREETER) }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["state"], "running");
    assert_eq!(r["live"]["state"], "running", "{r:#}");
    assert!(mods_dir.join("greeter.lua").is_file());
    let (list, _) = call(&mut s, "list_mods", json!({}));
    assert!(
        list["mods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "greeter" && m["state"] == "running")
    );
    // Lua in the game's console.
    let (r, _) = call(&mut s, "run_lua", json!({ "code": "1 + 2" }));
    assert_eq!(r["where"], "game");
    assert_eq!(r["lines"][0]["text"], "3");
    // A command sent as the player, and what it printed.
    let (r, _) = call(&mut s, "send_chat", json!({ "line": "/greet Sam", "wait_frames": 5 }));
    let texts: Vec<&str> = r["chat"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l["text"].as_str())
        .collect();
    assert!(texts.iter().any(|t| t.ends_with("Hello, Sam!")), "{r:#}");
    // The same steps as headless, live.
    let (r, error) = call(
        &mut s,
        "test_mod",
        json!({ "id": "greeter", "live": true, "steps": [
            { "frames": 3, "expect_state": { "greeter": "running" } },
            { "command": "/greet Alex", "expect_chat": "Hello, Alex" },
            { "lua": "return import('greeter').count()", "expect_output": "1" },
            { "key": "key.keyboard.j", "expect_chat": "jumped" },
            { "chat": "<Bo> hi" },
            { "block": [0, 0, 0], "id": "minecraft:stone" },
            { "suggest": "/greet A" },
            { "entities": [{ "kind": "zombie", "pos": [3, 64, 0] }] },
            { "players": [{ "name": "Alex" }] },
        ] }),
    );
    assert!(error, "the block step is headless only");
    let steps = r["steps"].as_array().unwrap();
    assert!(steps[..5].iter().all(|s| s["ok"] == true), "{r:#}");
    for step in &steps[5..] {
        let failure = step["failures"][0].as_str().unwrap();
        assert!(failure.contains("steps are headless only"), "{failure}");
    }
    // A broken edit: the check says where, and so does the game.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "greeter", "files": files("greeter.lua", "command('x', function() end\n") }),
    );
    assert!(error);
    assert_eq!(r["check"]["mod"]["state"], "failed");
    assert_eq!(r["live"]["state"], "failed");
    let (log, _) = call(&mut s, "game_log", json!({}));
    assert!(
        log["log"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["kind"] == "error"),
        "{log:#}"
    );
    let (r, _) = call(&mut s, "delete_mod", json!({ "id": "greeter" }));
    assert_eq!(r["ok"], true);
    assert!(!mods_dir.join("greeter.lua").exists());
    // The agent looks at the game: the game saves its next frame, the tool returns it.
    let shot = s.call_tool("screenshot", &json!({}));
    assert_ne!(shot["isError"], true, "{shot:#}");
    assert_eq!(shot["content"][0]["type"], "image", "{shot:#}");
    assert_eq!(shot["content"][0]["mimeType"], "image/png");
    assert!(
        shot["content"][0]["data"].as_str().unwrap().starts_with("iVBORw0KGgo"),
        "a PNG"
    );

    stop_tx.send(()).unwrap();
    game.join().unwrap();
    assert!(!file.exists(), "the link file goes with the game");
    let (status, _) = call(&mut Server::new(), "game_status", json!({}));
    assert_eq!(status["linked"], false);
    let _ = std::fs::remove_dir_all(&home);
}
