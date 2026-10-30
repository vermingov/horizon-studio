//! horizon-studio-mcp: Horizon Studio for AI agents, as an MCP server on
//! stdio (JSON-RPC 2.0, a message a line). It writes and reads script mods in
//! the game's mods folder, checks and tests them headless (`crate::headless`:
//! the studio itself on a simulated game) and, while the game runs with the
//! studio, live through the studio's AI link (`hstudio_link`): Lua in the
//! console, chat and commands, reloads, the console's log. Horizon's own
//! control socket (`horizon-ctl`, a game started with `--control`) is reached
//! for screenshots and playing.

use crate::game::hstudio::VERSION;
use crate::game::hstudio_examples::EXAMPLES;
use crate::headless::{Run, STEPS_HELP, Source, check_id, read_mod, run_steps, write_mod};
use serde_json::{Value as Json, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SERVER_NAME: &str = "horizon-studio";
/// The MCP versions it speaks, newest first.
const PROTOCOLS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What an agent is told when it connects.
pub const INSTRUCTIONS: &str = "Horizon Studio makes Minecraft mods for Horizon (Minecraft Java 26.3 in Rust) in Lua, \
hot reloaded in the running game. Workflow: read `docs` once (the scripting API and the language: Lua 5.3 without \
coroutines or goto); look at `examples` for idioms; write a mod with `write_mod` (one file, or a \
folder with main.lua; a library calls export(table) and others import(\"id\")); fix what `write_mod`'s check reports; \
then `test_mod` with steps, headless first (no game needed) and live once `game_status` says the game is linked. \
`run_lua`, `send_chat` and `game_log` work in the running game; `horizon_ctl` plays it and takes screenshots when the \
game runs with --control. Mods are sandboxed Lua: no files, no network, a step budget per call.";

// ------------------------------------------------------------------ paths

/// The link file the game writes (`hstudio_link::link_file`).
pub fn link_file() -> Option<PathBuf> {
    crate::game::hstudio_link::link_file()
}

fn read_link() -> Option<Json> {
    let text = std::fs::read_to_string(link_file()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// The mods folder: given, `$HORIZON_STUDIO_MODS`, the one the game last
/// reported in its link file, or Horizon's default game folder's.
pub fn mods_dir(given: Option<&str>) -> Result<PathBuf, String> {
    if let Some(d) = given.filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    if let Some(d) = std::env::var_os("HORIZON_STUDIO_MODS").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    if let Some(d) = read_link().and_then(|l| l["mods_dir"].as_str().map(PathBuf::from)) {
        return Ok(d);
    }
    let home = std::env::var_os("HORIZON_HOME").map(PathBuf::from).or_else(|| {
        let h = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
        Some(PathBuf::from(h).join(".local").join("share").join("horizon"))
    });
    if let Some(home) = home {
        let d = home
            .join("horizon-mods")
            .join(".horizon-mod-data")
            .join("horizon_studio")
            .join("mods");
        if d.is_dir() {
            return Ok(d);
        }
    }
    Err(
        "the mods folder isn't known yet: start Horizon with Horizon Studio once (it writes \
         ~/.horizon-studio/link.json), or pass mods_dir, or set HORIZON_STUDIO_MODS"
            .into(),
    )
}

// ------------------------------------------------------------------ the link

/// A connection to the studio's AI link in the running game.
pub struct LinkClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
}

impl LinkClient {
    /// Connects with the link file's port and token.
    /// Connects with the link file's port and token, as the agent `client`
    /// (its name shows in the game).
    pub fn connect(client: Option<&str>) -> Result<LinkClient, String> {
        let info = read_link().ok_or(
            "the game isn't linked: start Horizon with Horizon Studio (Options > Mods > Horizon Studio > AI link: On)",
        )?;
        let port = info["port"].as_u64().ok_or("the link file has no port")? as u16;
        let token = info["token"].as_str().ok_or("the link file has no token")?;
        let stream =
            TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2)).map_err(|_| {
                "the game isn't running (or Horizon Studio's AI link is off): start it, then try again".to_owned()
            })?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let mut c = LinkClient {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
        };
        c.write(&json!({ "auth": token, "client": client.unwrap_or("An AI agent") }))?;
        let reply = c.read_line()?;
        if reply.get("ok") != Some(&Json::Bool(true)) {
            return Err(format!("the game refused the link: {}", reply["error"]));
        }
        Ok(c)
    }

    fn write(&mut self, v: &Json) -> Result<(), String> {
        let mut line = v.to_string();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("the game closed the link: {e}"))
    }

    fn read_line(&mut self) -> Result<Json, String> {
        let mut line = String::new();
        let n = self
            .reader
            .read_line(&mut line)
            .map_err(|e| format!("no answer from the game: {e}"))?;
        if n == 0 {
            return Err("the game closed the link".into());
        }
        serde_json::from_str(&line).map_err(|e| format!("the game's answer isn't JSON: {e}"))
    }

    /// One request and its answer.
    pub fn call(&mut self, method: &str, params: Json) -> Result<Json, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.write(&json!({ "id": id, "method": method, "params": params }))?;
        loop {
            let reply = self.read_line()?;
            if reply["id"] != json!(id) {
                continue;
            }
            if let Some(e) = reply.get("error") {
                return Err(e.as_str().map_or_else(|| e.to_string(), str::to_owned));
            }
            return Ok(reply["result"].clone());
        }
    }
}

// ------------------------------------------------------------------ Horizon's control socket

/// One `horizon-ctl` command through Horizon's control socket (a game started
/// with `--control`, or by `horizon-ctl launch`): its JSON reply.
pub fn horizon_ctl(line: &str) -> Result<Json, String> {
    let path = std::env::var_os("HORIZON_CONTROL").map(PathBuf::from).or_else(|| {
        if cfg!(windows) {
            std::env::var_os("TEMP").map(|t| PathBuf::from(t).join("horizon-control.json"))
        } else {
            std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("horizon-control.sock"))
        }
    });
    let path = path.ok_or("no control socket path: set HORIZON_CONTROL")?;
    let not_running = || {
        format!(
            "Horizon's control socket isn't there ({}): start the game with --control, or with \
             `horizon-ctl launch --world NAME --wait`",
            path.display()
        )
    };
    let reply = if path.extension().is_some_and(|e| e == "json") {
        let info: Json = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .ok_or_else(not_running)?;
        let port = info["port"].as_u64().ok_or_else(not_running)? as u16;
        let token = info["token"].as_str().unwrap_or("");
        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|_| not_running())?;
        exchange(stream, &format!("auth {token}\n{line}\n"), true)?
    } else {
        #[cfg(unix)]
        {
            let stream = std::os::unix::net::UnixStream::connect(&path).map_err(|_| not_running())?;
            exchange(stream, &format!("{line}\n"), false)?
        }
        #[cfg(not(unix))]
        return Err(not_running());
    };
    serde_json::from_str(&reply).map_err(|_| format!("Horizon answered: {reply}"))
}

/// Writes the lines and reads the last reply line (after the auth line's, if any).
fn exchange<S: Read + Write>(mut stream: S, text: &str, auth: bool) -> Result<String, String> {
    stream.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if auth {
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        line.clear();
    }
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_owned())
}

fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// ------------------------------------------------------------------ tools

fn tool(name: &str, description: &str, properties: Json, required: &[&str]) -> Json {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

fn files_schema() -> Json {
    json!({
        "type": "object",
        "description": "The mod's files by path: {\"main.lua\": \"...\", \"lib/util.lua\": \"...\"} for a folder mod \
                        (require(\"lib.util\") reads the others), or {\"<id>.lua\": \"...\"} for one file.",
        "additionalProperties": { "type": "string" },
    })
}

fn steps_schema() -> Json {
    json!({ "type": "array", "items": { "type": "object" }, "description": STEPS_HELP })
}

pub fn tools() -> Json {
    let id = json!({ "type": "string", "description": "The mod's id: its file or folder name (lowercase, digits, _ and -)." });
    let mods_dir = json!({ "type": "string", "description": "The mods folder, if not the game's (found by itself)." });
    json!([
        tool(
            "docs",
            "Horizon Studio's scripting API (every function, event and screen), the Lua it runs, how mods \
              and libraries are made, and the test step format. Read it before writing a mod.",
            json!({ "topic": { "type": "string", "enum": ["all", "api", "tests"], "description": "Default all." } }),
            &[]
        ),
        tool(
            "examples",
            "The bundled example mods: without an id, the list; with one, its source (the UI Kit \
              library, screens, HUD panels, 3D drawing, commands, timers...).",
            json!({ "id": { "type": "string" } }),
            &[]
        ),
        tool(
            "game_status",
            "Whether the game runs with Horizon Studio's AI link, and then: the screen, the world and \
              player, and every script mod's state (running, failed with its error and line).",
            json!({}),
            &[]
        ),
        tool(
            "list_mods",
            "The script mods in the mods folder, with their state in the game when it is linked.",
            json!({ "mods_dir": mods_dir }),
            &[]
        ),
        tool(
            "read_mod",
            "A script mod's files.",
            json!({ "id": id, "mods_dir": mods_dir }),
            &["id"]
        ),
        tool(
            "write_mod",
            "Creates or replaces a script mod (a folder mod's files are written, others kept). It is \
              checked headless first (loads, registers, errors with lines); while the game is linked it then reloads \
              there and its live state is returned.",
            json!({ "id": id, "files": files_schema(), "mods_dir": mods_dir }),
            &["id", "files"]
        ),
        tool(
            "delete_mod",
            "Deletes a script mod (its file or folder) from the mods folder.",
            json!({ "id": id, "mods_dir": mods_dir }),
            &["id"]
        ),
        tool(
            "check_mod",
            "Loads a mod headless (the studio on a simulated game, with the libraries it imports): \
              whether it runs, its error and line, its commands, handlers and exports. Give files to check text \
              not yet written.",
            json!({ "id": id, "files": files_schema(), "mods_dir": mods_dir }),
            &["id"]
        ),
        tool(
            "test_mod",
            "Tests a mod end to end with steps. Headless (default): on a simulated game, no Horizon \
              needed. live: true runs the same steps in the running game through the AI link (the mod as written \
              in the mods folder). Each step reports what it caused: chat, sent lines, console lines, text drawn, \
              sounds, particles, failures.",
            json!({ "id": id, "files": files_schema(), "steps": steps_schema(),
                     "live": { "type": "boolean", "description": "Run in the game (default false: headless)." },
                     "mods_dir": mods_dir }),
            &["id", "steps"]
        ),
        tool(
            "run_lua",
            "Runs Lua in the studio's console and returns what it printed and the values: in the game \
              while it is linked (default), else headless (with the mod `with_mod` loaded, if given).",
            json!({ "code": { "type": "string" },
                     "live": { "type": "boolean", "description": "Default: live when the game is linked." },
                     "with_mod": { "type": "string", "description": "Headless: a mod to load first." },
                     "mods_dir": mods_dir }),
            &["code"]
        ),
        tool(
            "send_chat",
            "Sends a chat line or a command (\"/time set day\", a mod's \"/mycmd arg\") in the running \
              game as the player, waits, and returns the chat that followed.",
            json!({ "line": { "type": "string" },
                     "wait_frames": { "type": "integer", "description": "Frames to wait for the answers (default 20)." } }),
            &["line"]
        ),
        tool(
            "game_log",
            "The studio's console log (prints, errors and their tracebacks) and the chat, from the \
              running game; `since_log` and `since_chat` give only the newer lines.",
            json!({ "since_log": { "type": "integer" }, "since_chat": { "type": "integer" } }),
            &[]
        ),
        tool(
            "reload_mod",
            "Reloads a mod in the running game (and the mods using it), or every mod without an id.",
            json!({ "id": id }),
            &[]
        ),
        tool(
            "horizon_ctl",
            "A horizon-ctl command to Horizon itself, for a game started with --control (or by \
              `horizon-ctl launch --world NAME --wait`): state, ui, click ID, screenshot (returns the image), \
              goto X Y Z, break X Y Z, inventory, command /..., events, wait-for... `help` lists them.",
            json!({ "command": { "type": "string" } }),
            &["command"]
        ),
    ])
}

/// A tool's result: text (JSON pretty-printed), and whether it is an error.
fn text_result(v: &Json, error: bool) -> Json {
    let text = match v {
        Json::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": error })
}

fn files_param(args: &Json) -> Result<Option<Vec<(String, String)>>, String> {
    match args.get("files") {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Object(map)) => {
            let mut files = Vec::new();
            for (k, v) in map {
                let text = v.as_str().ok_or_else(|| format!("files.{k} must be the file's text"))?;
                files.push((k.clone(), text.to_owned()));
            }
            if files.is_empty() {
                return Err("files is empty".into());
            }
            Ok(Some(files))
        }
        Some(_) => Err("files is an object of path -> text".into()),
    }
}

fn str_arg<'a>(args: &'a Json, name: &str) -> Result<&'a str, String> {
    args.get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("missing '{name}'"))
}

/// The agent's name for the game to show, from its MCP client info.
pub fn agent_name(client: &str) -> String {
    let c = client.to_lowercase();
    let known = [
        ("claude-code", "Claude Code"),
        ("claude", "Claude Desktop"),
        ("cursor", "Cursor"),
        ("codex", "Codex"),
        ("gemini", "Gemini CLI"),
        ("opencode", "OpenCode"),
        ("oh-my-pi", "omp"),
        ("omp", "omp"),
        ("windsurf", "Windsurf"),
        ("zed", "Zed"),
        ("copilot", "VS Code"),
        ("vscode", "VS Code"),
    ];
    match known.iter().find(|(k, _)| c.contains(k)) {
        Some((_, name)) => (*name).to_owned(),
        None if client.trim().is_empty() => "An AI agent".into(),
        None => client.trim().chars().take(32).collect(),
    }
}

/// The server: a link to the game, made when first needed (or kept by
/// `keep_linked`) and again after it drops.
#[derive(Default)]
pub struct Server {
    link: Arc<Mutex<Option<LinkClient>>>,
    /// The agent, from `initialize`.
    client: Arc<Mutex<Option<String>>>,
}

impl Server {
    pub fn new() -> Server {
        Server::default()
    }

    /// The agent's name, from its `initialize`.
    pub fn set_client(&self, name: &str) {
        if let Ok(mut c) = self.client.lock() {
            *c = Some(agent_name(name));
        }
    }

    fn client_name(&self) -> Option<String> {
        self.client.lock().ok().and_then(|c| c.clone())
    }

    /// Keeps a link to the game while both run, so the game shows the agent
    /// as connected from the start: tries every 2 seconds, and checks an
    /// idle link every 5.
    pub fn keep_linked(&self) {
        let (link, client) = (Arc::clone(&self.link), Arc::clone(&self.client));
        std::thread::spawn(move || {
            let mut checked = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let Ok(mut l) = link.lock() else { return };
                match l.as_mut() {
                    None => {
                        let name = client.lock().ok().and_then(|c| c.clone());
                        if let Ok(c) = LinkClient::connect(name.as_deref()) {
                            *l = Some(c);
                            checked = Instant::now();
                        }
                    }
                    Some(c) if checked.elapsed() >= Duration::from_secs(5) => {
                        checked = Instant::now();
                        if c.call("ping", json!({})).is_err() {
                            *l = None;
                        }
                    }
                    Some(_) => {}
                }
            }
        });
    }

    /// A request to the game through the link (connecting first if needed).
    fn live(&mut self, method: &str, params: Json) -> Result<Json, String> {
        let name = self.client_name();
        let mut slot = self.link.lock().map_err(|_| "the link broke".to_owned())?;
        if let Some(link) = slot.as_mut() {
            match link.call(method, params.clone()) {
                Ok(v) => return Ok(v),
                Err(e) if e.starts_with("the game closed") || e.starts_with("no answer") => *slot = None,
                Err(e) => return Err(e),
            }
        }
        let mut link = LinkClient::connect(name.as_deref())?;
        let v = link.call(method, params)?;
        *slot = Some(link);
        Ok(v)
    }

    fn linked(&mut self) -> bool {
        self.live("status", json!({})).is_ok()
    }

    pub fn call_tool(&mut self, name: &str, args: &Json) -> Json {
        match self.tool(name, args) {
            Ok(v) if v.get("content").is_some() => v,
            Ok(v) => text_result(&v, v.get("ok") == Some(&Json::Bool(false))),
            Err(e) => text_result(&Json::String(e), true),
        }
    }

    fn tool(&mut self, name: &str, args: &Json) -> Result<Json, String> {
        let dir = || mods_dir(args.get("mods_dir").and_then(Json::as_str));
        match name {
            "docs" => {
                let topic = args.get("topic").and_then(Json::as_str).unwrap_or("all");
                let api = crate::game::hstudio_look::docs_markdown();
                let tests = format!("# Testing mods (test_mod)\n\n{STEPS_HELP}\n\n{TEST_EXAMPLE}");
                Ok(Json::String(match topic {
                    "api" => api,
                    "tests" => tests,
                    _ => format!("{GUIDE}\n\n{api}\n\n{tests}"),
                }))
            }
            "examples" => match args.get("id").and_then(Json::as_str) {
                Some(id) => EXAMPLES
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| Json::String(e.source.to_owned()))
                    .ok_or_else(|| format!("no example '{id}'")),
                None => Ok(Json::Array(
                    EXAMPLES
                        .iter()
                        .map(|e| json!({ "id": e.id, "title": e.title, "description": e.description }))
                        .collect(),
                )),
            },
            "game_status" => match self.live("status", json!({})) {
                Ok(v) => Ok(json!({ "linked": true, "game": v })),
                Err(e) => Ok(json!({
                    "linked": false,
                    "why": e,
                    "mods_dir": mods_dir(None).ok(),
                    "horizon_ctl": horizon_ctl("help").is_ok(),
                })),
            },
            "list_mods" => {
                let dir = dir()?;
                let mut out = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .filter_map(|e| {
                            let n = e.file_name().to_string_lossy().into_owned();
                            if n.starts_with('.') {
                                return None;
                            }
                            if e.path().join("main.lua").is_file() {
                                Some(n)
                            } else {
                                n.strip_suffix(".lua").map(str::to_owned)
                            }
                        })
                        .collect();
                    names.sort();
                    names.dedup();
                    out = names.into_iter().map(|id| json!({ "id": id })).collect();
                }
                if let Ok(live) = self.live("mods", json!({})) {
                    for m in &mut out {
                        if let Some(l) = live.as_array().and_then(|a| a.iter().find(|l| l["id"] == m["id"])) {
                            *m = l.clone();
                        }
                    }
                }
                Ok(json!({ "mods_dir": dir, "mods": out }))
            }
            "read_mod" => {
                let id = str_arg(args, "id")?;
                let files = read_mod(&dir()?, id)?;
                Ok(
                    json!({ "id": id, "files": files.into_iter().map(|(k, v)| (k, Json::String(v))).collect::<serde_json::Map<_, _>>() }),
                )
            }
            "write_mod" => {
                let id = str_arg(args, "id")?;
                check_id(id)?;
                let files = files_param(args)?.ok_or("missing 'files'")?;
                let dir = dir()?;
                let check = check(&Source::Files {
                    id: id.to_owned(),
                    files: files.clone(),
                    libraries: Some(dir.clone()),
                })
                .map(|(r, _)| r)
                .unwrap_or_else(|e| json!({ "ok": false, "error": e }));
                let path = write_mod(&dir, id, &files)?;
                let live = self
                    .live("reload", json!({ "id": id }))
                    .map(|mods| mods.as_array().and_then(|a| a.iter().find(|m| m["id"] == id)).cloned());
                Ok(json!({
                    "ok": check["ok"] == true,
                    "written": path,
                    "check": check,
                    "live": match live {
                        Ok(state) => state.unwrap_or(json!("not found in the game")),
                        Err(e) => json!(format!("not linked: {e}")),
                    },
                }))
            }
            "delete_mod" => {
                let id = str_arg(args, "id")?;
                check_id(id)?;
                let dir = dir()?;
                let (file, folder) = (dir.join(format!("{id}.lua")), dir.join(id));
                if folder.join("main.lua").is_file() {
                    std::fs::remove_dir_all(&folder).map_err(|e| e.to_string())?;
                } else if file.is_file() {
                    std::fs::remove_file(&file).map_err(|e| e.to_string())?;
                } else {
                    return Err(format!("no mod '{id}' in {}", dir.display()));
                }
                let _ = self.live("scan", json!({}));
                Ok(json!({ "ok": true, "deleted": id }))
            }
            "check_mod" => {
                let id = str_arg(args, "id")?;
                let source = match files_param(args)? {
                    Some(files) => Source::Files {
                        id: id.to_owned(),
                        files,
                        libraries: dir().ok(),
                    },
                    None => Source::Installed {
                        mods_dir: dir()?,
                        id: id.to_owned(),
                    },
                };
                check(&source).map(|(r, _)| r)
            }
            "test_mod" => {
                let id = str_arg(args, "id")?;
                let steps = args
                    .get("steps")
                    .and_then(Json::as_array)
                    .ok_or("missing 'steps' (a list)")?;
                if args.get("live").and_then(Json::as_bool) == Some(true) {
                    if let Some(files) = files_param(args)? {
                        write_mod(&dir()?, id, &files)?;
                    }
                    return self.live_test(id, steps);
                }
                let source = match files_param(args)? {
                    Some(files) => Source::Files {
                        id: id.to_owned(),
                        files,
                        libraries: dir().ok(),
                    },
                    None => Source::Installed {
                        mods_dir: dir()?,
                        id: id.to_owned(),
                    },
                };
                let mut run = Run::new(&source)?;
                let mut report = run_steps(&mut run, steps);
                report["where"] = json!("headless");
                Ok(report)
            }
            "run_lua" => {
                let code = str_arg(args, "code")?;
                let live = match args.get("live").and_then(Json::as_bool) {
                    Some(l) => l,
                    None => self.linked(),
                };
                if live {
                    let mut v = self.live("lua", json!({ "code": code }))?;
                    v["where"] = json!("game");
                    return Ok(v);
                }
                let mut run = match args.get("with_mod").and_then(Json::as_str) {
                    Some(id) => Run::new(&Source::Installed {
                        mods_dir: dir()?,
                        id: id.to_owned(),
                    })?,
                    None => Run::new(&Source::Files {
                        id: "scratch".into(),
                        files: vec![("scratch.lua".into(), "-- headless console\n".into())],
                        libraries: dir().ok(),
                    })?,
                };
                let lines = run.studio.run_chunk(&mut run.game, code, false);
                let failed = lines.iter().any(|(k, _)| *k == crate::game::hstudio::LogKind::Error);
                Ok(json!({
                    "ok": !failed,
                    "where": "headless",
                    "lines": lines.iter().map(|(k, t)| json!({ "kind": format!("{k:?}").to_lowercase(), "text": t })).collect::<Vec<_>>(),
                    "chat": run.game.chats,
                }))
            }
            "send_chat" => {
                let line = str_arg(args, "line")?;
                let frames = args
                    .get("wait_frames")
                    .and_then(Json::as_u64)
                    .unwrap_or(20)
                    .clamp(1, 1200);
                let status = self.live("status", json!({}))?;
                let (chat0, log0) = (
                    status["chat_seq"].as_u64().unwrap_or(0),
                    status["log_seq"].as_u64().unwrap_or(0),
                );
                self.live("send", json!({ "line": line }))?;
                self.live("wait", json!({ "frames": frames }))?;
                let chat = self.live("chat", json!({ "since": chat0 }))?;
                let log = self.live("log", json!({ "since": log0 }))?;
                Ok(json!({ "chat": chat["lines"], "log": log["lines"] }))
            }
            "game_log" => {
                let log = self.live(
                    "log",
                    json!({ "since": args.get("since_log").and_then(Json::as_u64).unwrap_or(0) }),
                )?;
                let chat = self.live(
                    "chat",
                    json!({ "since": args.get("since_chat").and_then(Json::as_u64).unwrap_or(0) }),
                )?;
                Ok(json!({ "log": log, "chat": chat }))
            }
            "reload_mod" => {
                let params = match args.get("id").and_then(Json::as_str) {
                    Some(id) => json!({ "id": id }),
                    None => json!({}),
                };
                self.live("reload", params)
            }
            "horizon_ctl" => {
                let command = str_arg(args, "command")?.trim();
                if command == "screenshot"
                    || command.starts_with("screenshot ") && command.split_whitespace().count() == 1
                {
                    let path = std::env::temp_dir().join(format!("horizon-studio-shot-{}.png", std::process::id()));
                    let reply = horizon_ctl(&format!("screenshot {}", path.display()))?;
                    if reply.get("error").is_some() {
                        return Ok(text_result(&reply, true));
                    }
                    let png = std::fs::read(&path).map_err(|e| format!("the screenshot isn't there: {e}"))?;
                    let _ = std::fs::remove_file(&path);
                    return Ok(json!({ "content": [
                        { "type": "image", "data": base64(&png), "mimeType": "image/png" },
                        { "type": "text", "text": reply.to_string() },
                    ] }));
                }
                let reply = horizon_ctl(command)?;
                let error = reply.get("error").is_some();
                Ok(text_result(&reply, error))
            }
            other => Err(format!("no tool '{other}'")),
        }
    }

    /// The test steps in the running game, through the link.
    fn live_test(&mut self, id: &str, steps: &[Json]) -> Result<Json, String> {
        let mods = self.live("reload", json!({ "id": id }))?;
        if !mods.as_array().is_some_and(|a| a.iter().any(|m| m["id"] == id)) {
            return Err(format!("the game has no mod '{id}' (write it with write_mod first)"));
        }
        let mut reports = Vec::new();
        let mut all_ok = true;
        for (i, step) in steps.iter().enumerate() {
            let status = self.live("status", json!({}))?;
            let (chat0, log0) = (
                status["chat_seq"].as_u64().unwrap_or(0),
                status["log_seq"].as_u64().unwrap_or(0),
            );
            let mut failures: Vec<String> = Vec::new();
            let mut notes: Vec<String> = Vec::new();
            let mut output: Vec<String> = Vec::new();
            let wait = |s: &mut Server, n: u64| s.live("wait", json!({ "frames": n.max(1) })).map(|_| ());
            if let Some(n) = step.get("frames").and_then(Json::as_u64) {
                wait(self, n)?;
            } else if let Some(sec) = step.get("seconds").and_then(Json::as_f64) {
                let fps = status["fps"].as_f64().unwrap_or(60.0).max(20.0);
                wait(self, (sec.max(0.0) * fps).round() as u64)?;
            } else if let Some(cmd) = step.get("command").and_then(Json::as_str) {
                let line = format!("/{}", cmd.trim().trim_start_matches('/'));
                self.live("send", json!({ "line": line }))?;
                wait(self, 10)?;
            } else if let Some(text) = step.get("chat").and_then(Json::as_str) {
                self.live("chat_in", json!({ "text": text }))?;
                wait(self, 2)?;
            } else if let Some(key) = step.get("key").and_then(Json::as_str) {
                match step.get("down").and_then(Json::as_bool) {
                    Some(down) => {
                        self.live("key", json!({ "key": key, "down": down }))?;
                    }
                    None => {
                        self.live("key", json!({ "key": key, "down": true }))?;
                        wait(self, 1)?;
                        self.live("key", json!({ "key": key, "down": false }))?;
                    }
                }
                wait(self, 2)?;
            } else if let Some(code) = step.get("lua").and_then(Json::as_str) {
                let r = self.live("lua", json!({ "code": code }))?;
                output = r["lines"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|l| l["text"].as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
            } else if let Some(r) = step.get("reload") {
                let params = match r.as_str() {
                    Some(id) => json!({ "id": id }),
                    None => json!({}),
                };
                self.live("reload", params)?;
                wait(self, 2)?;
            } else if ["player", "block", "time", "click", "type"]
                .iter()
                .any(|k| step.get(*k).is_some())
            {
                failures.push(
                    "player, block, time, click and type steps are headless only: live, use a command step \
                     (\"/tp\", \"/setblock\", \"/time set\") or horizon_ctl"
                        .into(),
                );
            } else if !step
                .as_object()
                .is_some_and(|o| o.keys().all(|k| k.starts_with("expect")))
            {
                failures.push(format!("unknown step {step}"));
            }
            let chat = self.live("chat", json!({ "since": chat0 }))?;
            let log = self.live("log", json!({ "since": log0 }))?;
            let lines = |v: &Json, kinds: &[&str]| -> Vec<String> {
                v["lines"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|l| kinds.contains(&l["kind"].as_str().unwrap_or("")))
                            .filter_map(|l| l["text"].as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let chats = lines(&chat, &["out", "chat"]);
            let sent = lines(&chat, &["sent"]);
            let logged = lines(&log, &["output", "error", "trace", "info"]);
            let errors = lines(&log, &["error"]);
            let expect_error = step.get("expect_error").filter(|v| !matches!(v, Json::Bool(false)));
            match expect_error {
                None if !errors.is_empty() => failures.push(format!("errors: {}", errors.join(" | "))),
                Some(Json::String(want)) if !errors.iter().any(|e| e.contains(want.as_str())) => {
                    failures.push(format!("expected an error with '{want}', got {errors:?}"));
                }
                Some(Json::Bool(true)) if errors.is_empty() => failures.push("expected an error, none came".into()),
                _ => {}
            }
            let want = |k: &str| match step.get(k) {
                Some(Json::String(s)) => vec![s.clone()],
                Some(Json::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
                _ => Vec::new(),
            };
            for w in want("expect_chat") {
                if !chats.iter().any(|c| c.contains(&w)) {
                    failures.push(format!("no chat line with '{w}' (chat: {chats:?})"));
                }
            }
            for w in want("expect_no_chat") {
                if chats.iter().any(|c| c.contains(&w)) {
                    failures.push(format!("a chat line has '{w}'"));
                }
            }
            for w in want("expect_output") {
                if !output.join("\n").contains(&w) {
                    failures.push(format!("the Lua output has no '{w}' (output: {output:?})"));
                }
            }
            for w in want("expect_log") {
                if !logged.iter().any(|c| c.contains(&w)) {
                    failures.push(format!("no console line with '{w}'"));
                }
            }
            for w in want("expect_sent") {
                if !sent.iter().any(|c| c.contains(&w)) {
                    failures.push(format!("nothing sent with '{w}' (sent: {sent:?})"));
                }
            }
            for k in ["expect_drawn", "expect_sound", "expect_particle", "expect_screen"] {
                if step.get(k).is_some() {
                    notes.push(format!(
                        "{k} is checked headless only (live: look with horizon_ctl screenshot)"
                    ));
                }
            }
            if let Some(states) = step.get("expect_state").and_then(Json::as_object) {
                let mods = self.live("mods", json!({}))?;
                for (mid, w) in states {
                    let got = mods
                        .as_array()
                        .and_then(|a| a.iter().find(|m| m["id"] == mid.as_str()))
                        .map_or("missing".to_owned(), |m| m["state"].as_str().unwrap_or("").to_owned());
                    if Some(got.as_str()) != w.as_str() {
                        failures.push(format!("{mid} is {got}, not {w}"));
                    }
                }
            }
            let ok = failures.is_empty();
            all_ok &= ok;
            let mut r = json!({ "step": i + 1, "do": step.to_string(), "ok": ok });
            for (k, v) in [
                ("failures", failures),
                ("notes", notes),
                ("output", output),
                ("chat", chats),
                ("sent", sent),
                ("log", logged),
            ] {
                if !v.is_empty() {
                    r[k] = json!(v);
                }
            }
            reports.push(r);
        }
        let passed = reports.iter().filter(|r| r["ok"] == true).count();
        let mods = self.live("mods", json!({}))?;
        Ok(json!({
            "ok": all_ok,
            "where": "game",
            "summary": format!("{passed}/{} steps passed", steps.len()),
            "steps": reports,
            "mod": mods.as_array().and_then(|a| a.iter().find(|m| m["id"] == id)).cloned(),
        }))
    }
}

/// Loads a mod headless: a report, and the run (for more).
pub fn check(source: &Source) -> Result<(Json, Run), String> {
    let mut run = Run::new(source)?;
    // A second frame lets handlers and timers due at once run.
    run.frames(2);
    let id = match source {
        Source::Installed { id, .. } | Source::Files { id, .. } => id.clone(),
    };
    let mods = run.mods();
    let this = mods
        .as_array()
        .and_then(|a| a.iter().find(|m| m["id"] == id.as_str()))
        .cloned();
    let log: Vec<String> = run
        .studio
        .log_since(1)
        .map(|(_, l)| format!("{}: {}", format!("{:?}", l.kind).to_lowercase(), l.text))
        .collect();
    let ok = this.as_ref().is_some_and(|m| m["state"] == "running");
    Ok((
        json!({
            "ok": ok,
            "mod": this,
            "libraries": mods.as_array().map(|a| a.iter().filter(|m| m["id"] != id.as_str()).map(|m| m["id"].clone()).collect::<Vec<_>>()),
            "log": log,
            "chat": run.game.chats,
        }),
        run,
    ))
}

const GUIDE: &str = "# Making mods with Horizon Studio

- A script mod is `<id>.lua` in the mods folder, or a folder `<id>/` with `main.lua` (and other files `require`d \
  from it). Header lines `-- name:`, `-- version:`, `-- author:`, `-- description:` name it.
- Its file runs when it loads; what it registers with `on(...)`, `command(...)`, `every(...)`/`after(...)` then runs \
  in the game. Saving reloads it at once (hot reload), and libraries' users reload with them.
- A library calls `export{...}` (or returns a table); other mods `local kit = import(\"id\")`. Use it for anything \
  shared: a UI kit, a pathfinder, config. `emit(\"id:event\", ...)` and `on(\"id:event\", fn)` let mods talk.
- Errors stop only that mod; `check_mod` and `write_mod` report them with the line. Each call has a step budget.
- Test: `test_mod` with steps, headless first, then `live: true` with the game running. Look at the game with \
  `horizon_ctl screenshot` when Horizon runs with --control.";

const TEST_EXAMPLE: &str = "Example (a mod with a /greet command and a tick counter):
[
  {\"frames\": 5, \"expect_state\": {\"greeter\": \"running\"}},
  {\"command\": \"/greet Alex\", \"expect_chat\": \"Hello, Alex\"},
  {\"lua\": \"local g = import('greeter'); assert(g.count() == 1, 'counted once')\"},
  {\"seconds\": 2},
  {\"key\": \"key.jump\", \"expect_chat\": \"jumped\"}
]";

// ------------------------------------------------------------------ JSON-RPC

/// One JSON-RPC message in, its answer out (none for notifications).
pub fn handle(server: &mut Server, msg: &Json) -> Option<Json> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let id = id?;
    let result = match method {
        "initialize" => {
            server.set_client(params["clientInfo"]["name"].as_str().unwrap_or(""));
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            let version = PROTOCOLS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOLS[1]);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": SERVER_NAME, "title": "Horizon Studio", "version": VERSION },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Ok(server.call_tool(name, &args))
        }
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err(json!({ "code": -32601, "message": format!("no method '{other}'") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

/// Serves MCP on stdin and stdout until stdin closes.
pub fn serve() {
    let mut server = Server::new();
    server.keep_linked();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Json>(&line) {
            Ok(Json::Array(batch)) => {
                let replies: Vec<Json> = batch.iter().filter_map(|m| handle(&mut server, m)).collect();
                (!replies.is_empty()).then(|| Json::Array(replies))
            }
            Ok(msg) => handle(&mut server, &msg),
            Err(e) => {
                Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }))
            }
        };
        if let Some(reply) = reply {
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
        }
    }
}
