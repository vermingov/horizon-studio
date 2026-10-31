// horizon-studio-mcp and the player's editor: an agent opening and reading
// the scripts the player has, through the AI link of a game (the studio on
// a fake game on its own thread, the link polled each frame as the bridge
// does), with the player doing things in the studio meanwhile. The link's
// file is named by an environment variable, the process's: this binary has
// one test, so nothing else here reads or changes it.

mod common;

use common::FakeGame;
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_link as link;
use horizon_studio::mcp::{Server, handle};
use serde_json::{Value as Json, json};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const GREETER: &str = "-- name: Greeter\nlocal count = 0\ncommand(\"greet\", function(rest, argv)\n  count = count + 1\n  chat.show(\"Hello, \" .. (argv[1] or \"you\") .. \"!\")\nend, \"greets someone\")\n";

/// Something the player does in the studio, run on the game's thread between frames.
type PlayerAct = Box<dyn FnOnce(&mut Studio, &mut FakeGame) + Send>;

/// The game on its thread.
struct Game {
    stop: mpsc::Sender<()>,
    player: mpsc::Sender<PlayerAct>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Game {
    fn start() -> Game {
        let (stop, stop_rx) = mpsc::channel::<()>();
        let (player, player_rx) = mpsc::channel::<PlayerAct>();
        let thread = std::thread::spawn(move || {
            let mut game = FakeGame::new("mcp-editor");
            let mut st = Studio::new();
            st.frame(&mut game, 0.016);
            while stop_rx.try_recv().is_err() {
                game.ticks += 1;
                st.frame(&mut game, 0.016);
                while let Ok(act) = player_rx.try_recv() {
                    act(&mut st, &mut game);
                }
                link::poll(&mut st, &mut game);
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        Game {
            stop,
            player,
            thread: Some(thread),
        }
    }
    /// Runs `f` on the game's thread and waits for what it gives back.
    fn with<T: Send + 'static>(&self, f: impl FnOnce(&mut Studio, &mut FakeGame) -> T + Send + 'static) -> T {
        let (tx, rx) = mpsc::channel();
        self.player
            .send(Box::new(move |st, game| {
                let _ = tx.send(f(st, game));
            }))
            .unwrap();
        rx.recv_timeout(Duration::from_secs(10)).expect("the game answered")
    }
    fn end(mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}

/// A tool's result: its text blocks, and whether it was an error.
fn call(server: &mut Server, name: &str, args: Json) -> (Vec<String>, bool) {
    let r = server.call_tool(name, &args);
    let texts = r["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| b["text"].as_str().map(str::to_owned))
        .collect();
    (texts, r["isError"] == true)
}

/// A tool's first block as JSON.
fn call_json(server: &mut Server, name: &str, args: Json) -> (Json, bool) {
    let (texts, error) = call(server, name, args);
    let first = texts.first().cloned().unwrap_or_default();
    (serde_json::from_str(&first).unwrap_or(Json::String(first)), error)
}

#[test]
fn an_agent_opens_and_reads_the_players_scripts() {
    let home = std::env::temp_dir().join(format!("hstudio-mcp-editor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let file = home.join("link.json");
    // SAFETY: the only test in this binary, before anything reads the variables.
    unsafe {
        std::env::set_var("HORIZON_STUDIO_LINK", &file);
        std::env::remove_var("HORIZON_STUDIO_MODS");
    }
    let game = Game::start();
    let start = Instant::now();
    while !file.exists() {
        assert!(start.elapsed() < Duration::from_secs(5), "the link file never came");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut s = Server::new();
    handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "clientInfo": { "name": "claude-code", "version": "2.1" } } }),
    );
    // The editor's tools are listed, each with its arguments.
    let tools = handle(&mut s, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
    for name in [
        "open_script",
        "read_script",
        "edit_script",
        "save_script",
        "mark_lines",
        "clear_marks",
        "start_lesson",
    ] {
        let t = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("no tool {name}"));
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(t["inputSchema"]["properties"]["scratch"].is_object(), "{name}");
    }
    let (r, error) = call_json(
        &mut s,
        "write_mod",
        json!({ "id": "greeter", "files": { "greeter.lua": GREETER } }),
    );
    assert!(!error, "{r:#}");

    // Opening a script in the player's editor.
    let (r, error) = call_json(&mut s, "open_script", json!({ "id": "greeter" }));
    assert!(!error, "{r:#}");
    assert_eq!(r["did"], "Opened greeter.lua");
    let (status, _) = call_json(&mut s, "game_status", json!({}));
    assert_eq!(status["game"]["editor"]["file"], "greeter.lua", "{status:#}");
    assert_eq!(status["game"]["protocol"], 2);
    // Reading it: the state, then the lines as cat -n numbers them.
    let (texts, error) = call(&mut s, "read_script", json!({}));
    assert!(!error, "{texts:?}");
    let state: Json = serde_json::from_str(&texts[0]).unwrap();
    assert_eq!((&state["file"], &state["open"]), (&json!("greeter.lua"), &json!(true)));
    assert_eq!(state["line_count"], 6);
    assert!(state.get("lines").is_none(), "the lines come once, numbered");
    assert!(texts[1].contains("     2\tlocal count = 0"), "{}", texts[1]);
    // The player's unsaved edits are what an agent reads, and they keep the file open.
    game.with(|st, _| {
        st.s.editor.click(1, 0, false);
        st.s.editor.insert("-- the player's\n");
    });
    let (texts, _) = call(&mut s, "read_script", json!({ "from": 2, "to": 3 }));
    assert!(
        texts[1].starts_with("     2\t-- the player's\n     3\tlocal count = 0"),
        "{}",
        texts[1]
    );
    let (r, error) = call_json(&mut s, "open_script", json!({ "scratch": true }));
    assert!(error);
    assert!(r.as_str().unwrap().contains("unsaved edits in greeter.lua"), "{r}");
    // While the player uses the studio it waits for them to pause.
    game.with(|st, game| {
        st.save_editor(game);
        st.s.ui.open = true;
        st.s.ui.coach.input_at = st.s.now;
    });
    // (The fake game's clock runs at 16 ms a frame, faster than the wall's:
    // asking again after a wait of 15 frames takes at least 30 ms.)
    let asked = Instant::now();
    let (r, error) = call_json(&mut s, "open_script", json!({ "scratch": true }));
    assert!(!error, "{r:#}");
    assert!(asked.elapsed() >= Duration::from_millis(25), "it waited");
    assert_eq!(game.with(|st, _| st.s.editor.title.clone()), "scratch.lua");
    // A mod that isn't there.
    let (r, error) = call_json(&mut s, "open_script", json!({ "id": "nope" }));
    assert!(error && r.as_str().unwrap().contains("no script mod 'nope'"), "{r}");
    // A request whose connection closed before its answer isn't done: its
    // client asks again on a new connection. (The game's thread is held
    // while the request comes and the connection closes.)
    {
        use std::io::Write;
        let (go, wait) = mpsc::channel::<()>();
        game.player
            .send(Box::new(move |_, _| {
                let _ = wait.recv_timeout(Duration::from_secs(5));
            }))
            .unwrap();
        let info: Json = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let mut raw = std::net::TcpStream::connect(("127.0.0.1", info["port"].as_u64().unwrap() as u16)).unwrap();
        let auth = json!({ "auth": info["token"], "client": "gone" });
        let open = json!({ "id": 1, "method": "open", "params": { "id": "greeter" } });
        raw.write_all(format!("{auth}\n{open}\n").as_bytes()).unwrap();
        drop(raw);
        go.send(()).unwrap();
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        game.with(|st, _| st.s.editor.title.clone()),
        "scratch.lua",
        "not opened"
    );

    // A change typed into the open script as the player watches: the answer
    // comes once it is all in (asking whether it is doesn't hurry it).
    let (r, error) = call_json(&mut s, "open_script", json!({ "id": "greeter" }));
    assert!(!error, "{r:#}");
    let mods_dir = game.with(|st, _| {
        st.s.ui.tab = horizon_studio::game::hstudio_ui::Tab::Editor;
        st.s.mods_dir()
    });
    let on_disk = std::fs::read_to_string(mods_dir.join("greeter.lua")).unwrap();
    let (r, error) = call_json(
        &mut s,
        "edit_script",
        json!({ "old_text": "local count = 0", "new_text": "local count = 0\nlocal greeted = {}" }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["did"], "Typed lines 3-4 into greeter.lua", "{r:#}");
    assert_eq!((&r["lines"], &r["animated"]), (&json!([3, 4]), &json!(true)));
    assert!(r.get("cut_short").is_none(), "nothing cut it short: {r:#}");
    let (texts, _) = call(&mut s, "read_script", json!({}));
    assert!(texts[1].contains("     4\tlocal greeted = {}"), "{}", texts[1]);
    let state: Json = serde_json::from_str(&texts[0]).unwrap();
    assert_eq!(state["dirty"], true);
    assert_eq!(
        std::fs::read_to_string(mods_dir.join("greeter.lua")).unwrap(),
        on_disk,
        "not saved yet"
    );
    // The tools that read the disk say the editor has more.
    let (r, _) = call_json(&mut s, "read_mod", json!({ "id": "greeter" }));
    assert_eq!(r["editor_unsaved"]["file"], "greeter.lua", "{r:#}");
    let (status, _) = call_json(&mut s, "game_status", json!({}));
    assert_eq!(status["game"]["editor"]["unsaved"]["by"], "Claude Code", "{status:#}");
    // Saved: the game reloads the mod.
    let (r, error) = call_json(&mut s, "save_script", json!({}));
    assert!(!error, "{r:#}");
    assert_eq!(
        (&r["saved"], &r["mod"]["state"]),
        (&json!(true), &json!("running")),
        "{r:#}"
    );
    assert!(
        std::fs::read_to_string(mods_dir.join("greeter.lua"))
            .unwrap()
            .contains("local greeted = {}")
    );
    let (r, _) = call_json(&mut s, "read_mod", json!({ "id": "greeter" }));
    assert!(r.get("editor_unsaved").is_none(), "{r:#}");
    // Pointing at code: a line marked with a note, which read_script and
    // game_status know of, and cleared.
    let (r, error) = call_json(
        &mut s,
        "mark_lines",
        json!({ "start_line": 3, "note": "Who was greeted.", "tone": "good" }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(
        (&r["did"], &r["lines"]),
        (&json!("Highlighted line 3 in greeter.lua"), &json!([3, 3]))
    );
    let (status, _) = call_json(&mut s, "game_status", json!({}));
    assert_eq!(status["game"]["editor"]["marks"], 1, "{status:#}");
    let (texts, _) = call(&mut s, "read_script", json!({}));
    let state: Json = serde_json::from_str(&texts[0]).unwrap();
    assert_eq!(
        (&state["marks"][0]["note"], &state["marks"][0]["tone"]),
        (&json!("Who was greeted."), &json!("good"))
    );
    let (r, error) = call_json(&mut s, "mark_lines", json!({ "start_line": 99 }));
    assert!(
        error && r.as_str().unwrap().contains("line 99 isn't one of them"),
        "{r}"
    );
    let (r, error) = call_json(&mut s, "clear_marks", json!({}));
    assert!(!error, "{r:#}");
    assert_eq!(r["cleared"], 1);
    // A lesson, and the player stepping through it while the agent waits.
    let (r, error) = call_json(
        &mut s,
        "start_lesson",
        json!({ "title": "Greeting", "steps": [
            { "start_line": 3, "note": "The count starts at 0.", "task": "Start it at 1." },
            { "start_line": 5, "note": "Each greet counts." },
        ] }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["steps"], 2, "{r:#}");
    let (status, _) = call_json(&mut s, "game_status", json!({}));
    assert_eq!(
        (
            &status["game"]["editor"]["lesson"]["title"],
            &status["game"]["editor"]["lesson"]["step"]
        ),
        (&json!("Greeting"), &json!(1)),
        "{status:#}"
    );
    // The player presses Alt+Right a moment after the agent begins to wait.
    let player = game.player.clone();
    let pressed = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        let next: PlayerAct = Box::new(|st, game| {
            use horizon_studio::game::hstudio_ui::{Input, Key, Mods, handle};
            let mods = Mods {
                ctrl: false,
                shift: false,
                alt: true,
            };
            handle(
                st,
                game,
                Input::Key {
                    key: Key::Right,
                    down: true,
                    repeat: false,
                    mods,
                },
            );
        });
        player.send(next).unwrap();
    });
    let (r, error) = call_json(&mut s, "wait_for_player", json!({ "timeout_s": 10 }));
    pressed.join().unwrap();
    assert!(!error, "{r:#}");
    assert_eq!(
        (&r["event"], &r["step"], &r["of"]),
        (&json!("next"), &json!(2), &json!(2)),
        "{r:#}"
    );
    // Nothing more: after its timeout it says so, for the agent to ask again.
    let asked = Instant::now();
    let (r, error) = call_json(&mut s, "wait_for_player", json!({ "timeout_s": 5 }));
    assert!(!error, "{r:#}");
    assert_eq!(r["event"], "timeout", "{r:#}");
    assert!(asked.elapsed() >= Duration::from_secs(5) && asked.elapsed() < Duration::from_secs(20));
    let (r, _) = call_json(&mut s, "clear_marks", json!({}));
    assert_eq!(r["lesson_ended"], true, "{r:#}");
    // What isn't there as the editor has it is an error that says what to do.
    let (r, error) = call_json(
        &mut s,
        "edit_script",
        json!({ "old_text": "local nope = 1", "new_text": "local nope = 2" }),
    );
    assert!(error);
    assert!(r.as_str().unwrap().contains("read_script"), "{r}");

    game.end();
    // Without the game the editor's tools say where they work.
    let (r, error) = call_json(&mut s, "read_script", json!({}));
    assert!(error);
    assert!(
        r.as_str()
            .unwrap()
            .contains("the editor tools work in the running game"),
        "{r}"
    );
    let _ = std::fs::remove_dir_all(&home);
}
