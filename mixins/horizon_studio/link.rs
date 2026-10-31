// Horizon Studio's link for AI tools (horizon-studio-mcp): a socket on
// 127.0.0.1 the game listens on while the studio is on, found through a file
// only its user can read (`~/.horizon-studio/link.json`: the port, a token, the
// mods folder). A client's first line is `{"auth": TOKEN}`; after it each line
// is a request `{"id", "method", "params"}` and gets one line back, `{"id",
// "result"}` or `{"id", "error"}`. Requests run in the game's frame, after the
// studio's; `wait` answers frames later. What scripts print to chat, send, and
// see in chat is kept, numbered, for `chat` (as the console's log is for `log`).

use super::hstudio::{GameApi, ModState, Studio, VERSION, error_place};
use serde_json::{Value as Json, json};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

/// The protocol's version, in the link file and the auth reply: 2 has the
/// player's editor (`editor`, `open`...).
pub const PROTOCOL: u32 = 2;

/// What to give an AI agent so it sets itself up (copied from the studio's
/// screen and its Options > Mods page; the README has it too).
pub const SETUP_PROMPT: &str = "Set up Horizon Studio's MCP server so you can make and test Horizon mods for me. Read \
https://raw.githubusercontent.com/vermingov/horizon-studio/main/AI_SETUP.md (with curl if you can't open web pages) \
and follow it step by step: run the commands yourself, ask me before building anything into my game, and tell me \
when to restart you.";
/// The longest request line (a mod's source fits many times).
const MAX_LINE: usize = 4 << 20;
const MAX_CHAT: usize = 500;
const MAX_CONNECTIONS: usize = 8;
/// The longest `wait`, in frames.
const MAX_WAIT: u64 = 1200;

/// A line of chat: `chat` (it came from the server), `out` (the studio or a
/// script printed it for the player) or `sent` (sent as the player).
pub struct ChatLine {
    pub seq: u64,
    pub kind: &'static str,
    pub text: String,
}

thread_local! {
    static LINK: RefCell<Option<Link>> = const { RefCell::new(None) };
    /// When to try binding again after a failure (frames).
    static RETRY_AT: Cell<u64> = const { Cell::new(0) };
    static FRAME: Cell<u64> = const { Cell::new(0) };
    static CHAT: RefCell<VecDeque<ChatLine>> = const { RefCell::new(VecDeque::new()) };
    static CHAT_SEQ: Cell<u64> = const { Cell::new(0) };
    /// Whether the link is on (`link.txt` in the studio's folder), once read.
    static ON: Cell<Option<bool>> = const { Cell::new(None) };
    /// The agents connected, as of the last poll.
    static AGENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Keeps a chat line for `chat`.
pub fn record(kind: &'static str, text: &str) {
    let seq = CHAT_SEQ.with(|s| {
        s.set(s.get() + 1);
        s.get()
    });
    CHAT.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= MAX_CHAT {
            c.pop_front();
        }
        c.push_back(ChatLine {
            seq,
            kind,
            text: text.to_owned(),
        });
    });
}

/// The chat lines after number `since`, and the last number.
pub fn chat_since(since: u64) -> (Vec<Json>, u64) {
    let lines = CHAT.with(|c| {
        c.borrow()
            .iter()
            .filter(|l| l.seq > since)
            .map(|l| json!({ "seq": l.seq, "kind": l.kind, "text": l.text }))
            .collect()
    });
    (lines, CHAT_SEQ.with(Cell::get))
}

/// Where the link file is: `$HORIZON_STUDIO_LINK`, else
/// `~/.horizon-studio/link.json` (the MCP server looks there too).
pub fn link_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("HORIZON_STUDIO_LINK").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".horizon-studio").join("link.json"))
}

/// Whether the link is on: it is unless `link.txt` in the studio's folder says `off`.
pub fn enabled(dir: &Path) -> bool {
    ON.with(|on| {
        on.get().unwrap_or_else(|| {
            let v = std::fs::read_to_string(dir.join("link.txt")).map_or(true, |t| t.trim() != "off");
            on.set(Some(v));
            v
        })
    })
}

/// Turns the link on or off, and remembers it.
pub fn set_enabled(dir: &Path, on: bool) {
    ON.with(|o| o.set(Some(on)));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("link.txt"), if on { "on\n" } else { "off\n" });
    if !on {
        stop();
    }
}

/// The agents connected now, by the names they gave ("Claude Code").
pub fn agents() -> Vec<String> {
    AGENTS.with(|a| a.borrow().clone())
}

fn update_agents(link: Option<&Link>) {
    let names = link.map_or_else(Vec::new, |l| {
        l.conns
            .iter()
            .filter(|c| c.authed && !c.closed)
            .map(|c| c.client.clone())
            .collect()
    });
    AGENTS.with(|a| *a.borrow_mut() = names);
}

/// "Claude Code connected", "2 agents connected", or None.
pub fn agents_line() -> Option<String> {
    let agents = agents();
    match agents.as_slice() {
        [] => None,
        [one] => Some(format!("{one} connected")),
        [a, b] if a == b => Some(format!("2 {a} sessions connected")),
        more => Some(format!("{} agents connected", more.len())),
    }
}

/// An agent's name as it gave it: printable, at most 32 characters.
fn client_name(v: Option<&str>) -> String {
    let name: String = v.unwrap_or("").chars().filter(|c| !c.is_control()).take(32).collect();
    let name = name.trim();
    if name.is_empty() {
        "An AI agent".into()
    } else {
        name.to_owned()
    }
}

/// Closes the link (and removes its file).
pub fn stop() {
    LINK.with(|l| drop(l.borrow_mut().take()));
    update_agents(None);
}

struct Connection {
    id: u64,
    stream: TcpStream,
    input: Vec<u8>,
    output: Vec<u8>,
    authed: bool,
    closed: bool,
    /// The agent's name, from its auth line.
    client: String,
}

/// A `wait` request: answered once `frames` more frames have run.
struct Waiting {
    conn: u64,
    id: Json,
    frames: u64,
}

pub struct Link {
    listener: TcpListener,
    token: String,
    file: PathBuf,
    conns: Vec<Connection>,
    waiting: Vec<Waiting>,
    next_conn: u64,
}

impl Drop for Link {
    fn drop(&mut self) {
        // The file names this game: only remove it while it still does.
        if std::fs::read_to_string(&self.file).is_ok_and(|t| t.contains(&self.token)) {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

/// 128 random bits as hex, from the standard library's OS-seeded hasher keys.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    (0..2u64)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(nanos);
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Writes the link file, readable by its user only.
fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir);
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&tmp)?;
    f.write_all(text.as_bytes())?;
    drop(f);
    std::fs::rename(&tmp, path)
}

impl Link {
    fn start(st: &Studio) -> Result<Link, String> {
        let file = link_file().ok_or("no home folder for the link file")?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = token();
        let info = json!({
            "protocol": PROTOCOL,
            "studio": VERSION,
            "port": port,
            "token": token,
            "pid": std::process::id(),
            "mods_dir": st.s.mods_dir(),
            "data_dir": st.s.dir,
        });
        write_file(&file, &format!("{info:#}\n")).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(Link {
            listener,
            token,
            file,
            conns: Vec::new(),
            waiting: Vec::new(),
            next_conn: 1,
        })
    }

    fn accept(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if self.conns.len() >= MAX_CONNECTIONS || stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    let _ = stream.set_nodelay(true);
                    self.conns.push(Connection {
                        id: self.next_conn,
                        stream,
                        input: Vec::new(),
                        output: Vec::new(),
                        authed: false,
                        closed: false,
                        client: String::new(),
                    });
                    self.next_conn += 1;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    /// The complete lines each connection has sent.
    fn read(&mut self) -> Vec<(u64, String)> {
        let mut lines = Vec::new();
        let mut buf = [0u8; 16 << 10];
        for c in &mut self.conns {
            loop {
                match c.stream.read(&mut buf) {
                    Ok(0) => {
                        c.closed = true;
                        break;
                    }
                    Ok(n) => c.input.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(_) => {
                        c.closed = true;
                        break;
                    }
                }
            }
            while let Some(at) = c.input.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = c.input.drain(..=at).collect();
                let line = String::from_utf8_lossy(&line).trim().to_owned();
                if !line.is_empty() {
                    lines.push((c.id, line));
                }
            }
            if c.input.len() > MAX_LINE {
                c.closed = true;
            }
        }
        lines
    }

    fn send(&mut self, conn: u64, reply: Json) {
        if let Some(c) = self.conns.iter_mut().find(|c| c.id == conn) {
            c.output.extend_from_slice(reply.to_string().as_bytes());
            c.output.push(b'\n');
        }
    }

    fn flush(&mut self) {
        for c in &mut self.conns {
            while !c.output.is_empty() && !c.closed {
                match c.stream.write(&c.output) {
                    Ok(0) => c.closed = true,
                    Ok(n) => {
                        c.output.drain(..n);
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(_) => c.closed = true,
                }
            }
        }
        let gone: Vec<u64> = self.conns.iter().filter(|c| c.closed).map(|c| c.id).collect();
        self.conns.retain(|c| !c.closed);
        self.waiting.retain(|w| !gone.contains(&w.conn));
    }
}

/// Each frame, after the studio's: new connections, their requests, the
/// waits that are due, the replies.
pub fn poll(st: &mut Studio, game: &mut dyn GameApi) {
    let frame = FRAME.with(|f| {
        f.set(f.get() + 1);
        f.get()
    });
    if !enabled(&st.s.dir) {
        stop();
        return;
    }
    LINK.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            if frame < RETRY_AT.with(Cell::get) {
                return;
            }
            match Link::start(st) {
                Ok(link) => *cell = Some(link),
                Err(e) => {
                    // Again in about ten seconds.
                    RETRY_AT.with(|r| r.set(frame + 600));
                    st.s.log(
                        super::hstudio::LogKind::Info,
                        format!("The AI link could not start: {e}"),
                    );
                    return;
                }
            }
        }
        let Some(link) = cell.as_mut() else {
            return;
        };
        link.accept();
        for (conn, line) in link.read() {
            let Some(c) = link.conns.iter().find(|c| c.id == conn) else {
                continue;
            };
            let authed = c.authed;
            // A connection already closed can't have its answer: its client
            // asks again on a new one, and what it asked isn't done twice.
            if authed && c.closed {
                continue;
            }
            let by = super::hstudio_coach::By {
                name: c.client.clone(),
                conn,
            };
            if !authed {
                let hello = serde_json::from_str::<Json>(&line).ok();
                let ok = hello
                    .as_ref()
                    .and_then(|v| v.get("auth").and_then(Json::as_str).map(|t| same(t, &link.token)))
                    .unwrap_or(false);
                if ok {
                    let name = client_name(hello.as_ref().and_then(|v| v.get("client")).and_then(Json::as_str));
                    if let Some(c) = link.conns.iter_mut().find(|c| c.id == conn) {
                        c.authed = true;
                        c.client = name;
                    }
                    update_agents(Some(link));
                    link.send(conn, json!({ "ok": true, "studio": VERSION, "protocol": PROTOCOL }));
                } else {
                    link.send(
                        conn,
                        json!({ "error": "the first line must be {\"auth\": TOKEN} with the link file's token" }),
                    );
                    link.flush();
                    if let Some(c) = link.conns.iter_mut().find(|c| c.id == conn) {
                        c.closed = true;
                    }
                }
                continue;
            }
            let request: Json = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    link.send(conn, json!({ "id": null, "error": format!("not JSON: {e}") }));
                    continue;
                }
            };
            let id = request.get("id").cloned().unwrap_or(Json::Null);
            let method = request.get("method").and_then(Json::as_str).unwrap_or("");
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match handle(st, game, &by, method, &params) {
                Ok(Answer::Now(result)) => link.send(conn, json!({ "id": id, "result": result })),
                Ok(Answer::Wait(frames)) => link.waiting.push(Waiting { conn, id, frames }),
                Err(e) => link.send(conn, json!({ "id": id, "error": e })),
            }
        }
        let mut due = Vec::new();
        link.waiting.retain_mut(|w| {
            w.frames = w.frames.saturating_sub(1);
            if w.frames == 0 {
                due.push((w.conn, w.id.clone()));
                false
            } else {
                true
            }
        });
        for (conn, id) in due {
            link.send(conn, json!({ "id": id, "result": { "frame": frame } }));
        }
        link.flush();
        update_agents(Some(link));
    });
}

/// Compares the token without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

enum Answer {
    Now(Json),
    /// Answer after this many frames.
    Wait(u64),
}

fn text_param<'a>(params: &'a Json, name: &str) -> Result<&'a str, String> {
    params
        .get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("missing the text parameter '{name}'"))
}

fn mod_index(st: &Studio, id: &str) -> Result<usize, String> {
    st.s.mods
        .iter()
        .position(|m| m.id == id)
        .ok_or_else(|| format!("no script mod '{id}' (the mods: {})", mod_ids(st)))
}

fn mod_ids(st: &Studio) -> String {
    let ids: Vec<&str> = st.s.mods.iter().map(|m| m.id.as_str()).collect();
    if ids.is_empty() { "none".into() } else { ids.join(", ") }
}

/// The script mods as the Library shows them: a script's or a module's
/// files, and for one that failed, the file and line of its error (for a
/// module, the file `require` loaded when that is where it failed).
pub fn mods(st: &Studio) -> Json {
    Json::Array(
        st.s.mods
            .iter()
            .map(|m| {
                let (state, error, traceback) = match &m.state {
                    ModState::Off => ("off", None, None),
                    ModState::Running => ("running", None, None),
                    ModState::Failed(e, t) => ("failed", Some(e.clone()), Some(t.clone())),
                };
                let place = error.as_deref().and_then(error_place);
                json!({
                    "id": m.id,
                    "name": m.name,
                    "version": m.version,
                    "author": m.author,
                    "description": m.description,
                    "kind": if m.folder.is_some() { "module" } else { "script" },
                    "files": m.files(),
                    "enabled": m.enabled,
                    "state": state,
                    "error": error,
                    "file": place.as_ref().map(|p| p.0.clone()),
                    "line": place.map(|p| p.1),
                    "traceback": traceback,
                    "main": m.main,
                    "folder": m.folder,
                    "library": m.exports.is_some(),
                    "imports": m.imports,
                    "commands": m.commands.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
                    "handlers": m.handlers.count(),
                    "timers": m.timers.len(),
                    "cost_ms": m.cost_ms,
                    "reloads": m.reloads,
                })
            })
            .collect(),
    )
}

fn player(game: &mut dyn GameApi) -> Json {
    match game.player() {
        None => Json::Null,
        Some(p) => json!({
            "x": p.x, "y": p.y, "z": p.z, "yaw": p.yaw, "pitch": p.pitch,
            "dimension": p.dimension, "health": p.health, "max_health": p.max_health,
            "food": p.food, "level": p.level, "mode": p.mode, "slot": p.slot,
            "held": p.held.map(|(item, count)| json!({ "item": item, "count": count })),
        }),
    }
}

fn handle(
    st: &mut Studio,
    game: &mut dyn GameApi,
    by: &super::hstudio_coach::By,
    method: &str,
    params: &Json,
) -> Result<Answer, String> {
    let now = Answer::Now;
    Ok(match method {
        "status" => now(json!({
            "studio": VERSION,
            "protocol": PROTOCOL,
            "frame": FRAME.with(Cell::get),
            "screen": game.screen(),
            "in_world": game.clock().is_some(),
            "ticks": game.ticks(),
            "fps": game.fps()[0],
            "player": player(game),
            "mods_dir": st.s.mods_dir(),
            "log_seq": st.s.log_seq,
            "chat_seq": CHAT_SEQ.with(Cell::get),
            "agents": agents(),
            "mods": mods(st),
            "editor": super::hstudio_coach::summary(st),
        })),
        "ping" => now(json!({ "ok": true })),
        // The player's editor, live (hstudio_coach): read a file as it has
        // it, open one, type a change in (and whether it is in), save it,
        // point at lines with a note and clear those marks, give a lesson,
        // and what the player did since (for an agent waiting for them).
        "editor" | "open" | "edit" | "typed" | "save" | "mark" | "unmark" | "lesson" | "player_event" => {
            now(super::hstudio_coach::call(st, game, by, method, params)?)
        }
        "mods" => now(mods(st)),
        // This game's own API reference (its version's).
        "docs" => now(Json::String(super::hstudio_look::docs_markdown())),
        "lua" => {
            let code = text_param(params, "code")?;
            let lines = st.run_chunk(game, code, false);
            let failed = lines.iter().any(|(k, _)| *k == super::hstudio::LogKind::Error);
            now(json!({
                "ok": !failed,
                "lines": lines
                    .iter()
                    .map(|(k, t)| json!({ "kind": format!("{k:?}").to_lowercase(), "text": t }))
                    .collect::<Vec<_>>(),
            }))
        }
        // The next frame as a PNG, for the agent to see the game: the studio's
        // window hidden for it unless `studio` is true. The MCP server reads
        // the file once it is written.
        "screenshot" => {
            let dir = st.s.dir.join("screenshots");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let path = dir.join("agent.png");
            let _ = std::fs::remove_file(&path);
            game.screenshot(&path)?;
            let with_studio = params.get("studio").and_then(Json::as_bool).unwrap_or(false);
            if st.s.ui.open && !with_studio {
                st.s.ui.hidden_until = st.s.now + 0.4;
            }
            now(json!({ "ok": true, "path": path, "studio_shown": st.s.ui.open && with_studio }))
        }
        "scan" => {
            st.scan(game);
            now(mods(st))
        }
        "reload" => {
            match params.get("id").and_then(Json::as_str) {
                Some(id) => {
                    st.scan(game);
                    let i = mod_index(st, id)?;
                    if !st.s.mods[i].enabled {
                        st.set_enabled(game, i, true);
                    }
                    st.load_and_dependents(game, i);
                }
                None => st.reload_all(game),
            }
            now(mods(st))
        }
        "enable" => {
            let i = mod_index(st, text_param(params, "id")?)?;
            let on = params.get("on").and_then(Json::as_bool).unwrap_or(true);
            st.set_enabled(game, i, on);
            now(mods(st))
        }
        "send" => {
            let line = text_param(params, "line")?;
            if line.trim().is_empty() || line.len() > 256 {
                return Err("a chat line is 1 to 256 characters".into());
            }
            game.send(line);
            now(json!({ "chat_seq": CHAT_SEQ.with(Cell::get) }))
        }
        "key" => {
            let key = text_param(params, "key")?;
            let down = params.get("down").and_then(Json::as_bool).unwrap_or(true);
            st.key(key, down);
            now(json!({ "ok": true }))
        }
        "chat_in" => {
            st.chat_line(text_param(params, "text")?);
            now(json!({ "ok": true }))
        }
        "chat" => {
            let since = params.get("since").and_then(Json::as_u64).unwrap_or(0);
            let (lines, last) = chat_since(since);
            now(json!({ "lines": lines, "last": last }))
        }
        "log" => {
            let since = params.get("since").and_then(Json::as_u64).unwrap_or(0);
            let lines: Vec<Json> = st
                .log_since(since)
                .map(|(seq, l)| json!({ "seq": seq, "kind": format!("{:?}", l.kind).to_lowercase(), "text": l.text }))
                .collect();
            now(json!({ "lines": lines, "last": st.s.log_seq }))
        }
        "wait" => {
            let frames = params
                .get("frames")
                .and_then(Json::as_u64)
                .unwrap_or(1)
                .clamp(1, MAX_WAIT);
            Answer::Wait(frames)
        }
        "" => return Err("no method".into()),
        other => {
            return Err(format!(
                "unknown method '{other}' (status, ping, mods, docs, lua, screenshot, scan, reload, enable, send, key, \
                 chat_in, chat, log, wait, editor, open, edit, typed, save, mark, unmark, lesson, player_event)"
            ));
        }
    })
}
