// Horizon Studio: mods written in Lua inside the game. Script mods live in
// `horizon-mods/.horizon-mod-data/horizon_studio/mods/`, one `name.lua` or a
// `name/` folder with `main.lua`; the studio loads them, reloads one as soon
// as a file of it changes on disk (from the in-game editor or any other), and
// stops only that mod when it fails. The console runs a line of Lua at once,
// in the running game, like a Lua executor. This module knows nothing of the
// game itself: it reaches it through `GameApi`, which hstudio_bridge
// implements over Horizon's `Game`, and the tests over a fake.

use super::hstudio_examples::{EXAMPLES, WELCOME};
use super::hstudio_lua::{
    Host, Lua, Native, NoHost, Res, Table, TableRef, Throw, Value, arg, bad_arg, check_func, check_num, check_str,
    check_text, opt_num, opt_str,
};
use super::hstudio_lualib::{inspect, new_env, quote, text_of};
use super::hstudio_ui::{Editor, Ui};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime};

pub const ID: &str = "horizon_studio";
pub const NAME: &str = "Horizon Studio";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "Make mods in Lua inside the game: an editor, a console that runs code at once, hot reloading and a library of your script mods. F8 opens it in a world.";
/// The key that opens the studio, rebindable in Options > Mods.
pub const OPEN_KEY: &str = "key.keyboard.f8";

/// Steps a script gets for loading, for one event, and for a console line.
pub const LOAD_STEPS: u64 = 10_000_000;
pub const EVENT_STEPS: u64 = 2_000_000;
pub const CONSOLE_STEPS: u64 = 20_000_000;
/// How often the mods folder is checked for changes.
pub const SCAN_SECONDS: f64 = 0.5;
const MAX_LOG: usize = 1000;
const MAX_HISTORY: usize = 200;
/// Ticks one frame catches up at most (after a pause, a slow frame).
const MAX_TICKS_PER_FRAME: u64 = 10;
/// Chat messages and commands scripts may send: a burst, then this many a second.
const SEND_BURST: f64 = 8.0;
const SEND_RATE: f64 = 4.0;
const MAX_STORED: usize = 256 << 10;

/// Commands a script may not take: vanilla's, and the studio's own.
const RESERVED: &[&str] = &[
    "studio",
    "lua",
    "advancement",
    "attribute",
    "ban",
    "ban-ip",
    "banlist",
    "bossbar",
    "clear",
    "clone",
    "damage",
    "data",
    "datapack",
    "debug",
    "defaultgamemode",
    "deop",
    "dialog",
    "difficulty",
    "effect",
    "enchant",
    "execute",
    "experience",
    "xp",
    "fetchprofile",
    "fill",
    "fillbiome",
    "forceload",
    "function",
    "gamemode",
    "gamerule",
    "give",
    "help",
    "item",
    "jfr",
    "kick",
    "kill",
    "list",
    "locate",
    "loot",
    "me",
    "msg",
    "tell",
    "w",
    "op",
    "pardon",
    "pardon-ip",
    "particle",
    "perf",
    "place",
    "playsound",
    "publish",
    "random",
    "recipe",
    "reload",
    "return",
    "ride",
    "rotate",
    "save-all",
    "save-off",
    "save-on",
    "say",
    "schedule",
    "scoreboard",
    "seed",
    "setblock",
    "setidletimeout",
    "setworldspawn",
    "spawnpoint",
    "spectate",
    "spreadplayers",
    "stop",
    "stopsound",
    "stopwatch",
    "summon",
    "tag",
    "team",
    "teammsg",
    "tm",
    "teleport",
    "tp",
    "tellraw",
    "test",
    "tick",
    "time",
    "title",
    "transfer",
    "trigger",
    "version",
    "waypoint",
    "weather",
    "whitelist",
    "worldborder",
];

/// Horizon's agent commands a script may use (`horizon-ctl`'s, docs/agent-control.md):
/// reading the world and acting as the player; nothing that saves, quits or changes options.
pub const AGENT_COMMANDS: &[&str] = &[
    "state",
    "looking-at",
    "target",
    "entities",
    "block",
    "blocks",
    "light",
    "find-block",
    "inventory",
    "screen",
    "messages",
    "move",
    "jump",
    "goto",
    "mouse-move",
    "look-at",
    "turn",
    "attack",
    "break",
    "place",
    "interact",
    "use-item",
    "select",
    "drop",
    "open-inventory",
    "click-button",
    "drag-at",
    "scroll-at",
    "click-at",
    "command",
    "pause",
    "wait",
    "stop",
];

// ----------------------------------------------------------------- the game, as the studio sees it

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub fn parse(s: &str) -> Option<Corner> {
        Some(match s {
            "top-left" | "topleft" | "tl" => Corner::TopLeft,
            "top-right" | "topright" | "tr" => Corner::TopRight,
            "bottom-left" | "bottomleft" | "bl" => Corner::BottomLeft,
            "bottom-right" | "bottomright" | "br" => Corner::BottomRight,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInfo {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: String,
    pub health: f32,
    pub max_health: f32,
    pub food: i32,
    pub saturation: f32,
    pub level: i32,
    pub mode: String,
    pub slot: usize,
    /// The held item's id and count, if any.
    pub held: Option<(String, u32)>,
}

/// An agent command's answer: now, or later on this channel (JSON text).
pub enum Reply {
    Now(serde_json::Value),
    Later(Receiver<String>),
}

/// What the studio needs of the game.
pub trait GameApi {
    /// A line in the player's own chat.
    fn chat(&mut self, text: &str);
    /// Sends a chat message, or a command when it starts with `/`.
    fn send(&mut self, line: &str);
    fn player(&mut self) -> Option<PlayerInfo>;
    /// A block's id (`minecraft:stone`), None where nothing is loaded.
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String>;
    /// Day time and game time, in a world.
    fn clock(&self) -> Option<(i64, i64)>;
    /// Game ticks so far (they move only in a world).
    fn ticks(&self) -> u64;
    fn fps(&self) -> [u32; 3];
    /// A vanilla key mapping (`key.jump`): held, and its key's label.
    fn mapping(&mut self, name: &str) -> Option<(bool, String)>;
    /// `playing`, `chat`, `inventory`, `paused`...
    fn screen(&self) -> String;
    /// Runs one of Horizon's agent commands (only `AGENT_COMMANDS` reach it).
    fn agent(&mut self, line: &str) -> Result<Reply, String>;
    /// The studio's folder.
    fn data_dir(&self) -> PathBuf;
    fn clipboard(&mut self) -> String;
    fn set_clipboard(&mut self, text: &str);
    /// The GUI's size in GUI pixels.
    fn gui_size(&self) -> (f32, f32);
    fn text_width(&self, text: &str) -> f32;
    /// Drawing: only while the studio draws (the HUD and its own screen).
    fn fill(&mut self, rect: [f32; 4], argb: u32);
    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32, shadow: bool);
    /// A box for a HUD panel in `corner`: where it goes, stacked with the game's own.
    fn place(&mut self, corner: Corner, w: f32, h: f32) -> (f32, f32);
}

// ----------------------------------------------------------------- script mods

#[derive(Clone, Debug, PartialEq)]
pub enum ModState {
    Off,
    Running,
    /// It failed: the message and the traceback.
    Failed(String, String),
}

#[derive(Default)]
pub struct Handlers {
    pub tick: Vec<Value>,
    pub frame: Vec<Value>,
    pub hud: Vec<Value>,
    pub key: Vec<Value>,
    pub chat: Vec<Value>,
    pub unload: Vec<Value>,
}

impl Handlers {
    fn list(&mut self, event: &str) -> Option<&mut Vec<Value>> {
        Some(match event {
            "tick" => &mut self.tick,
            "frame" => &mut self.frame,
            "hud" => &mut self.hud,
            "key" => &mut self.key,
            "chat" => &mut self.chat,
            "unload" => &mut self.unload,
            _ => return None,
        })
    }
    fn count(&self) -> usize {
        self.tick.len() + self.frame.len() + self.hud.len() + self.key.len() + self.chat.len() + self.unload.len()
    }
}

pub struct Timer {
    pub id: u32,
    pub at: f64,
    pub every: Option<f64>,
    pub f: Value,
}

pub struct Command {
    pub name: String,
    pub f: Value,
    pub help: String,
}

pub struct ScriptMod {
    /// From the file or folder name: `[a-z0-9_-]`.
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    /// The file it starts from.
    pub main: PathBuf,
    /// Its folder, for a folder mod (`require` reads other files there).
    pub folder: Option<PathBuf>,
    pub enabled: bool,
    pub state: ModState,
    pub env: Option<TableRef>,
    pub handlers: Handlers,
    pub commands: Vec<Command>,
    pub timers: Vec<Timer>,
    pub panel: Option<(Corner, Vec<String>)>,
    pub zoom: Option<f32>,
    pub modules: HashMap<String, Value>,
    /// Agent replies to come, and the callbacks for them.
    pub pending: Vec<(Receiver<String>, Value)>,
    /// Callbacks with their results, for the next frame.
    pub ready: Vec<(Value, Vec<Value>)>,
    /// Its files' sizes and times when it was loaded.
    pub stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
    /// Milliseconds its handlers took, averaged, per kind of event.
    pub cost_ms: f64,
    pub reloads: u32,
}

impl ScriptMod {
    fn new(id: &str, main: PathBuf, folder: Option<PathBuf>) -> ScriptMod {
        ScriptMod {
            id: id.to_owned(),
            name: id.to_owned(),
            version: String::new(),
            author: String::new(),
            description: String::new(),
            main,
            folder,
            enabled: true,
            state: ModState::Off,
            env: None,
            handlers: Handlers::default(),
            commands: Vec::new(),
            timers: Vec::new(),
            panel: None,
            zoom: None,
            modules: HashMap::new(),
            pending: Vec::new(),
            ready: Vec::new(),
            stamp: Vec::new(),
            cost_ms: 0.0,
            reloads: 0,
        }
    }
    /// Drops everything it made: back to not running.
    fn clear(&mut self) {
        self.env = None;
        self.handlers = Handlers::default();
        self.commands.clear();
        self.timers.clear();
        self.panel = None;
        self.zoom = None;
        self.modules.clear();
        self.pending.clear();
        self.ready.clear();
    }
    pub fn running(&self) -> bool {
        self.state == ModState::Running
    }
    /// The chunk name its main file's errors show.
    fn chunk(&self) -> String {
        match &self.folder {
            Some(_) => format!("{}/main.lua", self.id),
            None => format!("{}.lua", self.id),
        }
    }
}

/// Reads `-- name: ...`, `-- version:`, `-- author:`, `-- description:` lines
/// at the top of a mod's main file.
pub fn read_header(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("--") else {
            break;
        };
        if let Some((k, v)) = rest.split_once(':') {
            let k = k.trim().to_lowercase();
            if matches!(k.as_str(), "name" | "version" | "author" | "description") {
                out.push((k, v.trim().to_owned()));
            }
        }
    }
    out
}

/// A mod id from a file or folder name.
pub fn mod_id(name: &str) -> String {
    let id: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    if id.is_empty() { "mod".into() } else { id }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogKind {
    Input,
    Output,
    Error,
    Trace,
    Info,
}

pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Console,
    Mod(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Normal,
    /// Inside a `hud` handler: drawing works, the agent doesn't.
    Hud,
}

/// Everything but the interpreter, so a running script's host calls can
/// change it while the interpreter is busy.
pub struct State {
    pub dir: PathBuf,
    pub mods: Vec<ScriptMod>,
    /// The console's own handlers, commands and timers (`on` typed there).
    pub console_mod: ScriptMod,
    pub console_env: Option<TableRef>,
    pub log: VecDeque<LogLine>,
    pub history: Vec<String>,
    pub ui: Ui,
    pub editor: Editor,
    pub now: f64,
    pub next_scan: f64,
    pub last_tick: Option<u64>,
    pub held: HashSet<String>,
    pub toast: Option<(String, f64)>,
    pub disabled: HashSet<String>,
    send_tokens: f64,
    next_timer: u32,
    queued_keys: Vec<(String, bool)>,
    queued_commands: Vec<String>,
    queued_chat: Vec<String>,
    /// What console functions asked for, done after their call.
    requests: Vec<Request>,
    started: bool,
}

enum Request {
    Reload(Option<String>),
    Edit(String),
    New(String),
}

impl State {
    pub fn log(&mut self, kind: LogKind, text: impl Into<String>) {
        let text: String = text.into();
        for line in text.split('\n') {
            if self.log.len() >= MAX_LOG {
                self.log.pop_front();
            }
            self.log.push_back(LogLine {
                kind,
                text: line.replace('\t', "    "),
            });
        }
    }
    fn owner_mod(&mut self, owner: Owner) -> &mut ScriptMod {
        match owner {
            Owner::Console => &mut self.console_mod,
            Owner::Mod(i) => &mut self.mods[i],
        }
    }
    fn owner_name(&self, owner: Owner) -> &str {
        match owner {
            Owner::Console => "console",
            Owner::Mod(i) => &self.mods[i].id,
        }
    }
    pub fn mods_dir(&self) -> PathBuf {
        self.dir.join("mods")
    }
    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), self.now + 2.5));
    }
    /// Who owns `/name`, if anyone.
    fn command_owner(&self, name: &str) -> Option<Owner> {
        if self.console_mod.commands.iter().any(|c| c.name == name) {
            return Some(Owner::Console);
        }
        self.mods
            .iter()
            .position(|m| m.running() && m.commands.iter().any(|c| c.name == name))
            .map(Owner::Mod)
    }
}

pub struct Studio {
    pub lua: Lua,
    pub s: State,
}

impl Default for Studio {
    fn default() -> Self {
        Studio::new()
    }
}

/// A call's outcome as a script sees it: its results, or the error text.
pub type Outcome = Result<Vec<Value>, Throw>;

impl Studio {
    pub fn new() -> Studio {
        Studio {
            lua: Lua::new(),
            s: State {
                dir: PathBuf::new(),
                mods: Vec::new(),
                console_mod: {
                    let mut m = ScriptMod::new("console", PathBuf::new(), None);
                    m.state = ModState::Running;
                    m
                },
                console_env: None,
                log: VecDeque::new(),
                history: Vec::new(),
                ui: Ui::default(),
                editor: Editor::default(),
                now: 0.0,
                next_scan: 0.0,
                last_tick: None,
                held: HashSet::new(),
                toast: None,
                disabled: HashSet::new(),
                send_tokens: SEND_BURST,
                next_timer: 1,
                queued_keys: Vec::new(),
                queued_commands: Vec::new(),
                queued_chat: Vec::new(),
                requests: Vec::new(),
                started: false,
            },
        }
    }

    // ------------------------------------------------------------- start, scan, load

    /// The first frame: the folders, the welcome mod on a first run, the
    /// settings, the console's history and its scratch file, then the mods.
    pub fn start(&mut self, game: &mut dyn GameApi) {
        if self.s.started {
            return;
        }
        self.s.started = true;
        self.s.dir = game.data_dir();
        let mods = self.s.mods_dir();
        if !mods.exists() {
            let made = std::fs::create_dir_all(&mods).is_ok();
            if made {
                let _ = std::fs::write(mods.join(format!("{}.lua", WELCOME.id)), WELCOME.source);
            }
        }
        if let Ok(text) = std::fs::read_to_string(self.s.dir.join("studio.txt")) {
            for line in text.lines() {
                if let Some(id) = line.strip_prefix("off:") {
                    self.s.disabled.insert(id.trim().to_owned());
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(self.s.dir.join("history.txt")) {
            self.s.history = text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
        }
        let scratch = self.s.dir.join("scratch.lua");
        if !scratch.exists() {
            let _ = std::fs::write(&scratch, SCRATCH);
        }
        self.s.editor.open(&scratch, "scratch.lua");
        self.s.log(
            LogKind::Info,
            format!("{NAME} {VERSION}. Type Lua and press Enter; help() lists the API."),
        );
        self.scan(game);
    }

    fn save_settings(&self) {
        let mut ids: Vec<&String> = self.s.disabled.iter().collect();
        ids.sort();
        let text: String = ids.iter().map(|id| format!("off:{id}\n")).collect();
        let _ = std::fs::create_dir_all(&self.s.dir);
        let _ = std::fs::write(self.s.dir.join("studio.txt"), text);
    }

    fn save_history(&self) {
        let start = self.s.history.len().saturating_sub(MAX_HISTORY);
        let text = self.s.history[start..].join("\n");
        let _ = std::fs::write(self.s.dir.join("history.txt"), text);
    }

    /// The mods on disk: (id, main file, folder).
    fn discover(&self) -> Vec<(String, PathBuf, Option<PathBuf>)> {
        let Ok(entries) = std::fs::read_dir(self.s.mods_dir()) else {
            return Vec::new();
        };
        let mut found: Vec<(String, PathBuf, Option<PathBuf>)> = Vec::new();
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let (id, main, folder) = if p.is_dir() {
                let main = p.join("main.lua");
                if !main.is_file() {
                    continue;
                }
                (mod_id(&name), main, Some(p.clone()))
            } else if let Some(stem) = name.strip_suffix(".lua") {
                (mod_id(stem), p.clone(), None)
            } else {
                continue;
            };
            if found.iter().any(|f| f.0 == id) {
                continue;
            }
            found.push((id, main, folder));
        }
        found
    }

    /// Its files' sizes and times, to see when it changes.
    fn stamp(main: &Path, folder: Option<&Path>) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
        let mut out = Vec::new();
        let mut add = |p: &Path| {
            let meta = std::fs::metadata(p).ok();
            out.push((
                p.to_path_buf(),
                meta.as_ref().and_then(|m| m.modified().ok()),
                meta.map_or(0, |m| m.len()),
            ));
        };
        match folder {
            None => add(main),
            Some(dir) => {
                let mut stack = vec![(dir.to_path_buf(), 0)];
                let mut files = Vec::new();
                while let Some((d, depth)) = stack.pop() {
                    let Ok(entries) = std::fs::read_dir(&d) else { continue };
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.is_dir() && depth < 3 {
                            stack.push((p, depth + 1));
                        } else if p.extension().is_some_and(|x| x == "lua") {
                            files.push(p);
                        }
                    }
                }
                files.sort();
                for f in files {
                    add(&f);
                }
            }
        }
        out
    }

    /// Picks up new, changed and removed mods.
    pub fn scan(&mut self, game: &mut dyn GameApi) {
        self.s.next_scan = self.s.now + SCAN_SECONDS;
        let found = self.discover();
        // Removed: unload, then drop (from the end, so indices hold).
        for i in (0..self.s.mods.len()).rev() {
            if !found.iter().any(|f| f.0 == self.s.mods[i].id) {
                self.unload(game, i);
                let m = self.s.mods.remove(i);
                self.s.log(LogKind::Info, format!("Removed {}", m.id));
                self.s.ui.library_sel = self.s.ui.library_sel.min(self.s.mods.len().saturating_sub(1));
            }
        }
        for (id, main, folder) in found {
            match self.s.mods.iter().position(|m| m.id == id) {
                Some(i) => {
                    let stamp = Self::stamp(&main, folder.as_deref());
                    if stamp != self.s.mods[i].stamp {
                        self.s.mods[i].main = main;
                        self.s.mods[i].folder = folder;
                        if self.s.mods[i].enabled {
                            self.s.mods[i].reloads += 1;
                            let t = Instant::now();
                            self.load(game, i);
                            if self.s.mods[i].running() {
                                let ms = t.elapsed().as_secs_f64() * 1000.0;
                                let msg = format!("Reloaded {} ({ms:.1} ms)", self.s.mods[i].id);
                                self.s.log(LogKind::Info, &msg);
                                self.s.toast(msg);
                            }
                        } else {
                            self.s.mods[i].stamp = stamp;
                            self.read_meta(i);
                        }
                    }
                }
                None => {
                    let mut m = ScriptMod::new(&id, main, folder);
                    m.enabled = !self.s.disabled.contains(&id);
                    self.s.mods.push(m);
                    let i = self.s.mods.len() - 1;
                    if self.s.mods[i].enabled {
                        self.load(game, i);
                    } else {
                        self.s.mods[i].stamp = Self::stamp(&self.s.mods[i].main, self.s.mods[i].folder.as_deref());
                        self.read_meta(i);
                    }
                }
            }
        }
    }

    fn read_meta(&mut self, i: usize) {
        let src = std::fs::read_to_string(&self.s.mods[i].main).unwrap_or_default();
        let m = &mut self.s.mods[i];
        m.name = m.id.clone();
        m.version.clear();
        m.author.clear();
        m.description.clear();
        for (k, v) in read_header(&src) {
            match k.as_str() {
                "name" => m.name = v,
                "version" => m.version = v,
                "author" => m.author = v,
                _ => m.description = v,
            }
        }
    }

    /// Runs `unload` handlers and drops what the mod made.
    pub fn unload(&mut self, game: &mut dyn GameApi, i: usize) {
        if self.s.mods[i].running() {
            let handlers = self.s.mods[i].handlers.unload.clone();
            for f in handlers {
                if let Err(e) = self.call(game, Owner::Mod(i), &f, Vec::new(), EVENT_STEPS) {
                    self.s.log(
                        LogKind::Error,
                        format!("{}: unload: {}", self.s.mods[i].id, e.message()),
                    );
                }
            }
        }
        self.s.mods[i].clear();
        self.s.mods[i].state = ModState::Off;
    }

    /// (Re)loads mod `i` from its files.
    pub fn load(&mut self, game: &mut dyn GameApi, i: usize) {
        self.unload(game, i);
        self.read_meta(i);
        let m = &mut self.s.mods[i];
        m.stamp = Self::stamp(&m.main, m.folder.as_deref());
        let src = match std::fs::read(&m.main) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("cannot read {}: {e}", m.main.display());
                self.fail(game, Owner::Mod(i), msg, String::new());
                return;
            }
        };
        let env = self.make_env(false);
        let chunk = self.s.mods[i].chunk();
        self.s.mods[i].env = Some(env.clone());
        let f = match self.lua.load_bytes(&src, &chunk, &env) {
            Ok(f) => f,
            Err(e) => {
                self.fail(game, Owner::Mod(i), e, String::new());
                return;
            }
        };
        self.s.mods[i].state = ModState::Running;
        if let Err(e) = self.call(game, Owner::Mod(i), &f, Vec::new(), LOAD_STEPS) {
            self.fail(game, Owner::Mod(i), e.message(), e.traceback().to_owned());
        }
    }

    /// Stops a failed mod (or reports a console error).
    fn fail(&mut self, game: &mut dyn GameApi, owner: Owner, msg: String, trace: String) {
        // One line ("in main chunk") says nothing the message doesn't.
        let trace = if trace.contains('\n') { trace } else { String::new() };
        match owner {
            Owner::Console => {
                self.s.log(LogKind::Error, msg);
                if !trace.is_empty() {
                    self.s.log(LogKind::Trace, trace);
                }
            }
            Owner::Mod(i) => {
                let id = self.s.mods[i].id.clone();
                self.s.mods[i].clear();
                self.s.log(LogKind::Error, format!("{id} stopped: {msg}"));
                if !trace.is_empty() {
                    self.s.log(LogKind::Trace, trace.clone());
                }
                if !self.s.ui.open {
                    game.chat(&format!("[{NAME}] {id} stopped: {msg} (F8 for details)"));
                }
                self.s.toast(format!("{id} failed"));
                self.s.mods[i].state = ModState::Failed(msg, trace);
            }
        }
    }

    pub fn set_enabled(&mut self, game: &mut dyn GameApi, i: usize, on: bool) {
        let id = self.s.mods[i].id.clone();
        self.s.mods[i].enabled = on;
        if on {
            self.s.disabled.remove(&id);
            self.load(game, i);
        } else {
            self.s.disabled.insert(id);
            self.unload(game, i);
        }
        self.save_settings();
    }

    pub fn reload_all(&mut self, game: &mut dyn GameApi) {
        for i in 0..self.s.mods.len() {
            if self.s.mods[i].enabled {
                self.load(game, i);
            }
        }
        self.scan(game);
        let n = self.s.mods.iter().filter(|m| m.running()).count();
        self.s.log(LogKind::Info, format!("Reloaded all: {n} running"));
    }

    /// A new mod from the template: `<mods>/<id>.lua`, unless it exists.
    pub fn create_mod(&mut self, game: &mut dyn GameApi, name: &str) -> Result<PathBuf, String> {
        let id = mod_id(name.trim());
        let path = self.s.mods_dir().join(format!("{id}.lua"));
        if path.exists() || self.s.mods_dir().join(&id).exists() {
            return Err(format!("{id} already exists"));
        }
        let _ = std::fs::create_dir_all(self.s.mods_dir());
        let src = TEMPLATE.replace("{id}", &id).replace("{name}", name.trim());
        std::fs::write(&path, src).map_err(|e| e.to_string())?;
        self.scan(game);
        self.s.log(LogKind::Info, format!("Created {}", path.display()));
        Ok(path)
    }

    /// Writes bundled example `i` into the mods folder.
    pub fn install_example(&mut self, game: &mut dyn GameApi, i: usize) -> Result<PathBuf, String> {
        let ex = EXAMPLES.get(i).ok_or("no such example")?;
        let path = self.s.mods_dir().join(format!("{}.lua", ex.id));
        if path.exists() {
            return Err(format!("{} is already installed", ex.id));
        }
        let _ = std::fs::create_dir_all(self.s.mods_dir());
        std::fs::write(&path, ex.source).map_err(|e| e.to_string())?;
        self.scan(game);
        self.s.log(LogKind::Info, format!("Installed {}", ex.id));
        Ok(path)
    }

    // ------------------------------------------------------------- environments

    /// Globals for a mod or the console: the standard library and the API.
    fn make_env(&mut self, console: bool) -> TableRef {
        let env = new_env(&mut self.lua);
        let mut tables: HashMap<&str, Table> = HashMap::new();
        for (table, name, id, _) in API {
            let f = Native::host(name, *id);
            match *table {
                "" => env.borrow_mut().set_str(name, f),
                t => tables.entry(t).or_default().set_str(name, f),
            }
        }
        if console {
            for (name, id, _) in CONSOLE_API {
                env.borrow_mut().set_str(name, Native::host(name, *id));
            }
        }
        for (name, t) in tables {
            env.borrow_mut().set_str(name, Value::table(t));
        }
        env
    }

    fn console_env(&mut self) -> TableRef {
        if let Some(env) = &self.s.console_env {
            return env.clone();
        }
        let env = self.make_env(true);
        self.s.console_env = Some(env.clone());
        env
    }

    /// Calls `f` for `owner` with `budget` steps.
    fn call(&mut self, game: &mut dyn GameApi, owner: Owner, f: &Value, args: Vec<Value>, budget: u64) -> Outcome {
        self.call_in(game, owner, f, args, budget, Phase::Normal)
    }
    fn call_in(
        &mut self,
        game: &mut dyn GameApi,
        owner: Owner,
        f: &Value,
        args: Vec<Value>,
        budget: u64,
        phase: Phase,
    ) -> Outcome {
        self.lua.steps = budget;
        let started = Instant::now();
        let mut ctx = Ctx {
            game,
            s: &mut self.s,
            owner,
            phase,
        };
        let r = self.lua.call(&mut ctx, f, args);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let m = self.s.owner_mod(owner);
        m.cost_ms = m.cost_ms * 0.95 + ms * 0.05;
        r
    }

    /// Calls the `event` handlers of every running mod and the console.
    fn dispatch(&mut self, game: &mut dyn GameApi, event: &str, args: &[Value], phase: Phase) {
        let mut owners: Vec<Owner> = (0..self.s.mods.len()).map(Owner::Mod).collect();
        owners.push(Owner::Console);
        for owner in owners {
            let list = {
                let m = self.s.owner_mod(owner);
                if !m.running() {
                    continue;
                }
                match m.handlers.list(event) {
                    Some(l) if !l.is_empty() => l.clone(),
                    _ => continue,
                }
            };
            for f in list {
                if !self.s.owner_mod(owner).running() {
                    break;
                }
                if let Err(e) = self.call_in(game, owner, &f, args.to_vec(), EVENT_STEPS, phase) {
                    let msg = format!("{event}: {}", e.message());
                    if owner == Owner::Console {
                        // A broken console handler goes; the console stays.
                        if let Some(l) = self.s.console_mod.handlers.list(event) {
                            l.retain(|h| !h.raw_eq(&f));
                        }
                    }
                    self.fail(game, owner, msg, e.traceback().to_owned());
                }
            }
        }
    }

    // ------------------------------------------------------------- the game's calls

    /// Once a frame, with `dt` seconds since the last: hot reloading, input
    /// the hooks queued, ticks, timers, agent replies, then `frame` handlers.
    pub fn frame(&mut self, game: &mut dyn GameApi, dt: f64) {
        self.start(game);
        let dt = if dt.is_finite() { dt.clamp(0.0, 1.0) } else { 0.0 };
        self.s.now += dt;
        self.s.send_tokens = (self.s.send_tokens + dt * SEND_RATE).min(SEND_BURST);
        if self.s.now >= self.s.next_scan {
            self.scan(game);
        }
        for (key, down) in std::mem::take(&mut self.s.queued_keys) {
            if down {
                self.s.held.insert(key.clone());
            } else {
                self.s.held.remove(&key);
            }
            self.dispatch(game, "key", &[Value::str(&key), Value::Bool(down)], Phase::Normal);
        }
        for line in std::mem::take(&mut self.s.queued_chat) {
            self.dispatch(game, "chat", &[Value::str(&line)], Phase::Normal);
        }
        for line in std::mem::take(&mut self.s.queued_commands) {
            self.run_command(game, &line);
        }
        let ticks = game.ticks();
        let in_world = game.player().is_some();
        match self.s.last_tick {
            Some(last) if in_world && ticks > last => {
                for _ in 0..(ticks - last).min(MAX_TICKS_PER_FRAME) {
                    self.dispatch(game, "tick", &[], Phase::Normal);
                }
            }
            _ => {}
        }
        self.s.last_tick = Some(ticks);
        self.run_timers(game);
        self.run_replies(game);
        self.dispatch(game, "frame", &[Value::Num(dt)], Phase::Normal);
        if self.s.toast.as_ref().is_some_and(|t| t.1 < self.s.now) {
            self.s.toast = None;
        }
    }

    fn run_timers(&mut self, game: &mut dyn GameApi) {
        let now = self.s.now;
        let mut owners: Vec<Owner> = (0..self.s.mods.len()).map(Owner::Mod).collect();
        owners.push(Owner::Console);
        for owner in owners {
            let due: Vec<(u32, Value)> = {
                let m = self.s.owner_mod(owner);
                if !m.running() {
                    continue;
                }
                let mut due = Vec::new();
                m.timers.retain_mut(|t| {
                    if t.at > now {
                        return true;
                    }
                    due.push((t.id, t.f.clone()));
                    match t.every {
                        Some(every) => {
                            t.at = (t.at + every).max(now);
                            true
                        }
                        None => false,
                    }
                });
                due
            };
            for (_, f) in due {
                if !self.s.owner_mod(owner).running() {
                    break;
                }
                if let Err(e) = self.call(game, owner, &f, Vec::new(), EVENT_STEPS) {
                    self.fail(game, owner, format!("timer: {}", e.message()), e.traceback().to_owned());
                }
            }
        }
    }

    fn run_replies(&mut self, game: &mut dyn GameApi) {
        let mut owners: Vec<Owner> = (0..self.s.mods.len()).map(Owner::Mod).collect();
        owners.push(Owner::Console);
        for owner in owners {
            let mut calls = std::mem::take(&mut self.s.owner_mod(owner).ready);
            let m = self.s.owner_mod(owner);
            let mut i = 0;
            while i < m.pending.len() {
                match m.pending[i].0.try_recv() {
                    Ok(text) => {
                        let (_, cb) = m.pending.remove(i);
                        calls.push((cb, reply_values(&text)));
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => i += 1,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let (_, cb) = m.pending.remove(i);
                        calls.push((cb, vec![Value::Nil, Value::str("the game dropped the request")]));
                    }
                }
            }
            for (cb, args) in calls {
                if cb.is_nil() || !self.s.owner_mod(owner).running() {
                    continue;
                }
                if let Err(e) = self.call(game, owner, &cb, args, EVENT_STEPS) {
                    self.fail(
                        game,
                        owner,
                        format!("agent callback: {}", e.message()),
                        e.traceback().to_owned(),
                    );
                }
            }
        }
    }

    /// The HUD, under F1's rule: `hud` handlers, the mods' panels, the toast.
    pub fn hud(&mut self, game: &mut dyn GameApi) {
        if !self.s.started {
            return;
        }
        let (w, h) = game.gui_size();
        self.dispatch(
            game,
            "hud",
            &[Value::Num(f64::from(w)), Value::Num(f64::from(h))],
            Phase::Hud,
        );
        let panels: Vec<(Corner, Vec<String>)> = self
            .s
            .mods
            .iter()
            .chain(std::iter::once(&self.s.console_mod))
            .filter(|m| m.running())
            .filter_map(|m| m.panel.clone())
            .collect();
        for (corner, lines) in panels {
            let w = lines.iter().map(|l| game.text_width(l)).fold(0.0, f32::max) + 4.0;
            let h = 10.0 * lines.len() as f32 + 2.0;
            let (x, y) = game.place(corner, w, h);
            game.fill([x, y, w, h], 0x7800_0000);
            for (i, line) in lines.iter().enumerate() {
                game.text(line, [x + 2.0, y + 2.0 + 10.0 * i as f32], 0xFF_FFFF, true);
            }
        }
        if let Some((text, _)) = &self.s.toast
            && !self.s.ui.open
        {
            let tw = game.text_width(text);
            let x = ((w - tw) / 2.0).floor();
            game.fill([x - 3.0, 3.0, tw + 6.0, 12.0], 0xA010_1828);
            game.text(text, [x, 5.0], 0x9F_E8FF, true);
        }
    }

    /// The zoom the scripts ask for: the strongest.
    pub fn zoom(&self) -> Option<f32> {
        self.s
            .mods
            .iter()
            .chain(std::iter::once(&self.s.console_mod))
            .filter(|m| m.running())
            .filter_map(|m| m.zoom)
            .fold(None, |acc: Option<f32>, z| Some(acc.map_or(z, |a| a.max(z))))
    }

    /// A key went down or up (`key.keyboard.r`), for `key` handlers next frame.
    pub fn key(&mut self, key: &str, down: bool) {
        if self.s.queued_keys.len() < 256 {
            self.s.queued_keys.push((key.to_owned(), down));
        }
    }

    /// A chat line the player received, for `chat` handlers next frame.
    pub fn chat_line(&mut self, line: &str) {
        if self.s.queued_chat.len() < 64 {
            self.s.queued_chat.push(line.to_owned());
        }
    }

    /// A command typed in chat (no slash): whether the studio takes it. It
    /// runs on the next frame.
    pub fn command(&mut self, line: &str) -> bool {
        let name = line.split_whitespace().next().unwrap_or("");
        let ours = matches!(name, "studio" | "lua") || self.s.command_owner(name).is_some();
        if ours && self.s.queued_commands.len() < 64 {
            self.s.queued_commands.push(line.to_owned());
        }
        ours
    }

    fn run_command(&mut self, game: &mut dyn GameApi, line: &str) {
        let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match name {
            "studio" => match rest {
                "reload" => {
                    self.reload_all(game);
                    game.chat(&format!("[{NAME}] reloaded all script mods"));
                }
                "" => self.s.ui.open_request = true,
                other => game.chat(&format!(
                    "[{NAME}] unknown: /studio {other} (try /studio or /studio reload)"
                )),
            },
            "lua" => {
                let lines = self.run_console(game, rest);
                for (kind, text) in lines {
                    let prefix = if kind == LogKind::Error { "error: " } else { "" };
                    game.chat(&format!("{prefix}{text}"));
                }
            }
            _ => {
                let Some(owner) = self.s.command_owner(name) else {
                    return;
                };
                let Some(f) = self
                    .s
                    .owner_mod(owner)
                    .commands
                    .iter()
                    .find(|c| c.name == name)
                    .map(|c| c.f.clone())
                else {
                    return;
                };
                let argv: Vec<Value> = rest.split_whitespace().map(Value::str).collect();
                let args = vec![Value::str(rest), Value::table(Table::from_list(argv))];
                match self.call(game, owner, &f, args, EVENT_STEPS) {
                    Ok(r) => {
                        if let Some(v) = r.first()
                            && !v.is_nil()
                        {
                            game.chat(&v.plain_text());
                        }
                    }
                    Err(e) => {
                        let msg = e.message();
                        game.chat(&format!("/{name}: {msg}"));
                        self.s.log(LogKind::Error, format!("/{name}: {msg}"));
                        if !e.traceback().is_empty() {
                            self.s.log(LogKind::Trace, e.traceback().to_owned());
                        }
                    }
                }
            }
        }
    }

    /// Runs a line (or a chunk) of Lua in the console: an expression's values
    /// are printed. Returns the lines it logged.
    pub fn run_console(&mut self, game: &mut dyn GameApi, src: &str) -> Vec<(LogKind, String)> {
        self.start(game);
        let mark = self.s.log.len();
        let src = src.trim();
        if src.is_empty() {
            return Vec::new();
        }
        if self.s.history.last().is_none_or(|l| l != src) {
            self.s.history.push(src.to_owned());
            self.save_history();
        }
        self.s.log(LogKind::Input, format!("> {src}"));
        let env = self.console_env();
        let f = match self.lua.load(&format!("return {src}"), "console", &env) {
            Ok(f) => Ok(f),
            Err(_) => self.lua.load(src, "console", &env),
        };
        match f {
            Err(e) => self.s.log(LogKind::Error, e),
            Ok(f) => match self.call(game, Owner::Console, &f, Vec::new(), CONSOLE_STEPS) {
                Ok(vals) => {
                    if !vals.is_empty() {
                        let text = vals.iter().map(|v| inspect(v, 2)).collect::<Vec<_>>().join(", ");
                        self.s.log(LogKind::Output, text);
                    }
                }
                Err(e) => self.fail(game, Owner::Console, e.message(), e.traceback().to_owned()),
            },
        }
        self.do_requests(game);
        let skip = mark.min(self.s.log.len());
        self.s
            .log
            .iter()
            .skip(skip)
            .filter(|l| l.kind != LogKind::Input)
            .map(|l| (l.kind, l.text.clone()))
            .collect()
    }

    fn do_requests(&mut self, game: &mut dyn GameApi) {
        for r in std::mem::take(&mut self.s.requests) {
            match r {
                Request::Reload(None) => self.reload_all(game),
                Request::Reload(Some(id)) => {
                    if let Some(i) = self.s.mods.iter().position(|m| m.id == id) {
                        self.s.mods[i].enabled = true;
                        self.s.disabled.remove(&id);
                        self.load(game, i);
                        self.save_settings();
                        if self.s.mods[i].running() {
                            self.s.log(LogKind::Info, format!("Reloaded {id}"));
                        }
                    }
                }
                Request::Edit(id) => {
                    if let Some(i) = self.s.mods.iter().position(|m| m.id == id) {
                        self.edit_mod(i);
                        self.s.ui.tab = super::hstudio_ui::Tab::Editor;
                    }
                }
                Request::New(name) => match self.create_mod(game, &name) {
                    Ok(path) => {
                        let title = path
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        self.s.editor.open(&path, &title);
                        self.s.ui.tab = super::hstudio_ui::Tab::Editor;
                    }
                    Err(e) => self.s.log(LogKind::Error, e),
                },
            }
        }
    }

    /// Runs the editor's text as a console chunk (Ctrl+Enter).
    pub fn run_editor(&mut self, game: &mut dyn GameApi) {
        let src = self.s.editor.text();
        let env = self.console_env();
        let name = self.s.editor.title.clone();
        self.s.log(LogKind::Input, format!("> run {name}"));
        match self.lua.load(&src, &name, &env) {
            Err(e) => {
                self.s.editor.error_line = error_line(&e);
                self.s.log(LogKind::Error, e);
            }
            Ok(f) => {
                self.s.editor.error_line = None;
                if let Err(e) = self.call(game, Owner::Console, &f, Vec::new(), CONSOLE_STEPS) {
                    self.s.editor.error_line = error_line(&e.message());
                    self.fail(game, Owner::Console, e.message(), e.traceback().to_owned());
                }
            }
        }
        self.do_requests(game);
    }

    /// Saves the editor's file; the scan then reloads its mod.
    pub fn save_editor(&mut self, game: &mut dyn GameApi) {
        match self.s.editor.save() {
            Ok(path) => {
                self.s.log(LogKind::Info, format!("Saved {}", path.display()));
                // Check its syntax now, so the error line shows at once.
                let src = self.s.editor.text();
                let env = Rc::new(std::cell::RefCell::new(Table::new()));
                self.s.editor.error_line = match self.lua.load(&src, &self.s.editor.title.clone(), &env) {
                    Ok(_) => None,
                    Err(e) => error_line(&e),
                };
                self.scan(game);
            }
            Err(e) => self.s.log(LogKind::Error, format!("Could not save: {e}")),
        }
    }

    /// Opens mod `i`'s main file in the editor.
    pub fn edit_mod(&mut self, i: usize) {
        if let Some(m) = self.s.mods.get(i) {
            let title = m.chunk();
            let path = m.main.clone();
            self.s.editor.open(&path, &title);
            self.s.editor.error_line = match &m.state {
                ModState::Failed(msg, _) => error_line(msg),
                _ => None,
            };
        }
    }
}

/// The editor's file on a first run: a place to try things.
const SCRATCH: &str = "-- scratch.lua: a place to try code. Ctrl+Enter runs this file in the console;\n-- Ctrl+S saves it. Your mods go in the Library tab.\n\nlocal x, y, z = player.pos()\nif x then\n  print(string.format(\"You are at %.1f %.1f %.1f, facing %s\", x, y, z, player.facing()))\nend\n";

/// A new mod from the Library's New button.
const TEMPLATE: &str = "-- name: {name}\n-- version: 0.1.0\n-- description: What this mod does.\n\n-- This file runs when the mod loads, and again each time you save it.\nprint(\"{id} loaded\")\n\ncommand(\"{id}\", function(args)\n  return \"Hello from {name}!\"\nend, \"Says hello\")\n\non(\"tick\", function()\n  -- 20 times a second\nend)\n";

/// The line number in `chunk:12: message`.
pub fn error_line(msg: &str) -> Option<usize> {
    let mut parts = msg.splitn(3, ':');
    let _chunk = parts.next()?;
    parts.next()?.trim().parse().ok()
}

/// An agent reply's JSON as script values: the table, or nil and the error.
fn reply_values(text: &str) -> Vec<Value> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => match v.get("error").and_then(|e| e.as_str()) {
            Some(err) => vec![Value::Nil, Value::str(err)],
            None => vec![json_to_lua(&v)],
        },
        Err(_) => vec![Value::str(text)],
    }
}

/// JSON as Lua: objects and arrays become tables.
pub fn json_to_lua(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => Value::Num(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Value::str(s),
        serde_json::Value::Array(items) => Value::table(Table::from_list(items.iter().map(json_to_lua).collect())),
        serde_json::Value::Object(map) => {
            let mut t = Table::new();
            for (k, v) in map {
                t.set_str(k, json_to_lua(v));
            }
            Value::table(t)
        }
    }
}

/// A value as a Lua literal, for `storage.set`: nil, booleans, numbers,
/// strings and tables of them.
pub fn serialize(v: &Value, depth: usize, out: &mut String) -> Result<(), String> {
    match v {
        Value::Nil => out.push_str("nil"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Num(n) if n.is_nan() => out.push_str("0/0"),
        Value::Num(n) if n.is_infinite() => out.push_str(if *n > 0.0 { "1/0" } else { "-1/0" }),
        Value::Num(n) => out.push_str(format!("{n:?}").trim_end_matches(".0")),
        Value::Str(s) => out.push_str(&quote(s)),
        Value::Table(t) => {
            if depth > 16 {
                return Err("tables nested too deep to store (or a cycle)".into());
            }
            out.push('{');
            for (k, val) in t.borrow().entries() {
                out.push('[');
                serialize(&k, depth + 1, out)?;
                out.push_str("]=");
                serialize(&val, depth + 1, out)?;
                out.push(',');
                if out.len() > MAX_STORED {
                    return Err("value too large to store".into());
                }
            }
            out.push('}');
        }
        Value::Func(_) | Value::Native(_) => return Err("functions cannot be stored".into()),
    }
    Ok(())
}

fn storage_key(lua: &Lua, args: &[Value]) -> Result<String, Throw> {
    let key = check_text(lua, args, 0, "storage")?;
    if key.is_empty()
        || key.len() > 64
        || !key.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        || key.starts_with('.')
    {
        return Err(lua.rt_error("storage keys are 1 to 64 letters, digits, '_', '-' or '.'"));
    }
    Ok(key)
}

/// A `require` name as a path in the mod's folder.
fn module_path(name: &str) -> Option<String> {
    if name.is_empty() || name.contains("..") || !name.chars().all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
    {
        return None;
    }
    let rel = name.trim_end_matches(".lua").replace('.', "/");
    Some(format!("{rel}.lua"))
}

pub fn color_arg(lua: &Lua, args: &[Value], i: usize, default: u32) -> Result<u32, Throw> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(default),
        Some(Value::Num(n)) => Ok(*n as i64 as u32),
        Some(Value::Str(s)) => {
            let t = String::from_utf8_lossy(s);
            let hex = t.trim_start_matches('#');
            let v = u32::from_str_radix(hex, 16)
                .map_err(|_| bad_arg(lua, i, "color", "'#rrggbb' or '#aarrggbb' expected"))?;
            Ok(if hex.len() <= 6 { 0xFF00_0000 | v } else { v })
        }
        Some(_) => Err(bad_arg(lua, i, "color", "number or '#rrggbb' expected")),
    }
}

// ----------------------------------------------------------------- the API scripts see

/// (table, name, id, documentation). The Docs tab lists these.
pub const API: &[(&str, &str, u32, &str)] = &[
    (
        "",
        "on",
        1,
        "on(event, fn): run fn on \"tick\" (20 a second), \"frame\"(dt), \"hud\"(w, h), \"key\"(key, down), \"chat\"(text) or \"unload\"",
    ),
    (
        "",
        "command",
        2,
        "command(name, fn[, help]): /name in chat calls fn(args, words); a returned string is shown",
    ),
    (
        "",
        "after",
        3,
        "after(seconds, fn): call fn once, later; returns an id for cancel",
    ),
    (
        "",
        "every",
        4,
        "every(seconds, fn): call fn again and again; returns an id",
    ),
    ("", "cancel", 5, "cancel(id): stop a timer from after or every"),
    (
        "",
        "require",
        6,
        "require(name): load name.lua from the mod's folder (folder mods)",
    ),
    (
        "",
        "color",
        7,
        "color(r, g, b[, a]): a color number for hud functions (0-255 each)",
    ),
    (
        "chat",
        "show",
        10,
        "chat.show(text): a line in your chat that only you see",
    ),
    ("chat", "say", 11, "chat.say(text): send a chat message (rate limited)"),
    (
        "game",
        "run",
        20,
        "game.run(command): run a command as you, e.g. game.run(\"time set day\")",
    ),
    (
        "game",
        "fps",
        21,
        "game.fps(): frames per second, slowest frame, 1% low",
    ),
    (
        "game",
        "screen",
        22,
        "game.screen(): \"playing\", \"chat\", \"inventory\", \"paused\"...",
    ),
    ("game", "time", 23, "game.time(): seconds since the studio started"),
    ("game", "ticks", 24, "game.ticks(): game ticks so far"),
    ("player", "pos", 30, "player.pos(): x, y, z (nil outside a world)"),
    (
        "player",
        "block",
        31,
        "player.block(): the block position you stand in: x, y, z",
    ),
    ("player", "look", 32, "player.look(): yaw, pitch in degrees"),
    (
        "player",
        "facing",
        33,
        "player.facing(): \"north\", \"south\", \"east\" or \"west\"",
    ),
    (
        "player",
        "dimension",
        34,
        "player.dimension(): \"minecraft:overworld\"...",
    ),
    ("player", "health", 35, "player.health(): health, max health"),
    ("player", "food", 36, "player.food(): food level, saturation"),
    (
        "player",
        "held",
        37,
        "player.held(): the held item's id and count, or nil",
    ),
    (
        "player",
        "info",
        38,
        "player.info(): a table of all of the above, and level, mode, slot",
    ),
    (
        "world",
        "block",
        40,
        "world.block(x, y, z): \"minecraft:stone\", or nil where nothing is loaded",
    ),
    (
        "world",
        "time",
        41,
        "world.time(): time of day (0-23999), game time in ticks, day number",
    ),
    (
        "world",
        "find",
        42,
        "world.find(block[, radius]): the nearest blocks of a kind: {count, nearest = {{pos, distance}...}}",
    ),
    (
        "world",
        "entities",
        43,
        "world.entities([radius[, type]]): mobs, items and orbs near you, nearest first",
    ),
    (
        "hud",
        "panel",
        50,
        "hud.panel(corner, lines): a box of text lines in \"top-left\", \"top-right\", \"bottom-left\" or \"bottom-right\"; hud.panel(nil) clears",
    ),
    (
        "hud",
        "text",
        51,
        "hud.text(x, y, text[, color[, shadow]]): draw text (inside on(\"hud\"))",
    ),
    (
        "hud",
        "rect",
        52,
        "hud.rect(x, y, w, h[, color]): fill a rectangle (inside on(\"hud\")), color 0xAARRGGBB or \"#rrggbb\"",
    ),
    ("hud", "width", 53, "hud.width(text): text width in GUI pixels"),
    ("hud", "size", 54, "hud.size(): GUI width, height"),
    (
        "keys",
        "down",
        60,
        "keys.down(key): whether a key is held: \"key.keyboard.r\", \"key.mouse.left\"",
    ),
    (
        "keys",
        "mapping",
        61,
        "keys.mapping(name): a vanilla mapping (\"key.jump\"): held, key label",
    ),
    ("view", "zoom", 70, "view.zoom(factor): zoom the view in, 1 (off) to 10"),
    (
        "agent",
        "query",
        80,
        "agent.query(cmd): an instant agent read as a table: \"state\", \"inventory\", \"looking-at\", \"block X Y Z\", \"screen\", \"messages\"",
    ),
    (
        "agent",
        "run",
        81,
        "agent.run(cmd[, fn]): an agent action, e.g. \"goto X Y Z\", \"break X Y Z\", \"place X Y Z\", \"look-at X Y Z\", \"select 3\"; fn(result, err) when done",
    ),
    (
        "agent",
        "stop",
        82,
        "agent.stop(): cancel the running agent action and release keys",
    ),
    (
        "storage",
        "get",
        90,
        "storage.get(key): a value this mod stored, or nil",
    ),
    (
        "storage",
        "set",
        91,
        "storage.set(key, value): keep a value (nil, boolean, number, string or table of them)",
    ),
    (
        "mod",
        "info",
        100,
        "mod.info(): this mod's id, name, version and folder name",
    ),
];

/// Only in the console.
pub const CONSOLE_API: &[(&str, u32, &str)] = &[
    ("help", 200, "help([name]): this list, or one function's line"),
    (
        "clear",
        201,
        "clear(): empty the console and drop what the console registered",
    ),
    ("reload", 202, "reload([id]): reload one script mod, or all"),
    ("mods", 203, "mods(): the script mods and their state"),
    ("edit", 204, "edit(id): open a mod in the editor"),
    ("new", 205, "new(name): make a new mod from the template and open it"),
];

/// A running script's view of the studio and the game.
struct Ctx<'a> {
    game: &'a mut dyn GameApi,
    s: &'a mut State,
    owner: Owner,
    phase: Phase,
}

impl Ctx<'_> {
    fn me(&mut self) -> &mut ScriptMod {
        self.s.owner_mod(self.owner)
    }
    fn drawing(&self, lua: &Lua, what: &str) -> Result<(), Throw> {
        if self.phase == Phase::Hud {
            Ok(())
        } else {
            Err(lua.rt_error(format!(
                "hud.{what} draws only inside on(\"hud\", function(w, h) ... end)"
            )))
        }
    }
    fn send(&mut self, lua: &Lua, line: &str) -> Result<(), Throw> {
        if self.s.send_tokens < 1.0 {
            return Err(lua.rt_error("sending too fast: chat.say and game.run are limited to a few a second"));
        }
        self.s.send_tokens -= 1.0;
        self.game.send(line);
        Ok(())
    }
    fn agent(&mut self, lua: &Lua, line: &str) -> Result<Reply, Throw> {
        if self.phase == Phase::Hud {
            return Err(lua.rt_error("agent functions are not available while drawing the HUD"));
        }
        let name = line.split_whitespace().next().unwrap_or("");
        if !AGENT_COMMANDS.contains(&name) {
            return Err(lua.rt_error(format!("'{name}' is not an agent command scripts can use")));
        }
        self.game.agent(line).map_err(|e| lua.rt_error(e))
    }
    fn timer(&mut self, lua: &Lua, args: &[Value], every: bool) -> Res {
        let fname = if every { "every" } else { "after" };
        let secs = check_num(lua, args, 0, fname)?;
        let f = check_func(lua, args, 1, fname)?;
        if !secs.is_finite() || secs < 0.0 || (every && secs < 0.01) {
            return Err(bad_arg(lua, 0, fname, "a positive number of seconds expected"));
        }
        if self.me().timers.len() >= 256 {
            return Err(lua.rt_error("too many timers (256)"));
        }
        let id = self.s.next_timer;
        self.s.next_timer += 1;
        let at = self.s.now + secs;
        self.me().timers.push(Timer {
            id,
            at,
            every: every.then_some(secs),
            f,
        });
        Ok(vec![Value::Num(f64::from(id))])
    }
    fn storage_dir(&self) -> PathBuf {
        self.s.dir.join("storage").join(self.s.owner_name(self.owner))
    }
}

impl Ctx<'_> {
    /// An agent read whose answer is ready now, as a table.
    fn query_now(&mut self, lua: &Lua, line: &str) -> Res {
        match self.agent(lua, line)? {
            Reply::Now(v) => match v.get("error").and_then(|e| e.as_str()) {
                Some(err) => Err(lua.rt_error(err)),
                None => Ok(vec![json_to_lua(&v)]),
            },
            Reply::Later(rx) => {
                // An action: its answer comes later, with nobody to tell.
                self.me().pending.push((rx, Value::Nil));
                Ok(vec![
                    Value::Nil,
                    Value::str("this agent command is an action: use agent.run"),
                ])
            }
        }
    }
}

/// `Direction.fromYRot`: yaw 0 faces south, 90 west.
pub fn facing(yaw: f32) -> &'static str {
    let i = (yaw / 90.0 + 0.5).floor() as i32 & 3;
    ["south", "west", "north", "east"][i as usize]
}

fn player_table(p: &PlayerInfo) -> Value {
    let mut t = Table::new();
    for (k, v) in [
        ("x", p.x),
        ("y", p.y),
        ("z", p.z),
        ("yaw", f64::from(p.yaw)),
        ("pitch", f64::from(p.pitch)),
        ("health", f64::from(p.health)),
        ("max_health", f64::from(p.max_health)),
        ("food", f64::from(p.food)),
        ("saturation", f64::from(p.saturation)),
        ("level", f64::from(p.level)),
        ("slot", p.slot as f64 + 1.0),
    ] {
        t.set_str(k, Value::Num(v));
    }
    t.set_str("dimension", Value::str(&p.dimension));
    t.set_str("mode", Value::str(&p.mode));
    t.set_str("facing", Value::str(facing(p.yaw)));
    if let Some((item, n)) = &p.held {
        t.set_str("held", Value::str(item));
        t.set_str("held_count", Value::Num(f64::from(*n)));
    }
    Value::table(t)
}

fn pos(x: f64, y: f64, z: f64) -> Vec<Value> {
    vec![Value::Num(x), Value::Num(y), Value::Num(z)]
}

impl Host for Ctx<'_> {
    fn print(&mut self, text: &str) {
        let line = match self.owner {
            Owner::Console => text.to_owned(),
            Owner::Mod(i) => format!("[{}] {text}", self.s.mods[i].id),
        };
        self.s.log(LogKind::Output, line);
    }

    fn call(&mut self, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res {
        let none = || Ok(Vec::new());
        match id {
            1 => {
                let event = check_text(lua, &args, 0, "on")?;
                let f = check_func(lua, &args, 1, "on")?;
                if self.me().handlers.count() >= 256 {
                    return Err(lua.rt_error("too many handlers (256)"));
                }
                match self.me().handlers.list(&event) {
                    Some(list) => list.push(f),
                    None => {
                        return Err(bad_arg(
                            lua,
                            0,
                            "on",
                            &format!("unknown event '{event}' (tick, frame, hud, key, chat, unload)"),
                        ));
                    }
                }
                none()
            }
            2 => {
                let name = check_text(lua, &args, 0, "command")?.trim_start_matches('/').to_owned();
                let f = check_func(lua, &args, 1, "command")?;
                let help = opt_str(lua, &args, 2, "command")?
                    .map(|s| text_of(&s))
                    .unwrap_or_default();
                if name.is_empty()
                    || name.len() > 32
                    || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(bad_arg(
                        lua,
                        0,
                        "command",
                        "a name of letters, digits, '_' or '-' expected",
                    ));
                }
                if RESERVED.contains(&name.as_str()) {
                    return Err(lua.rt_error(format!("/{name} is the game's own command")));
                }
                if let Some(other) = self.s.command_owner(&name)
                    && other != self.owner
                {
                    let who = self.s.owner_name(other).to_owned();
                    return Err(lua.rt_error(format!("/{name} is already used by {who}")));
                }
                let me = self.me();
                me.commands.retain(|c| c.name != name);
                me.commands.push(Command { name, f, help });
                none()
            }
            3 => self.timer(lua, &args, false),
            4 => self.timer(lua, &args, true),
            5 => {
                let id = check_num(lua, &args, 0, "cancel")? as u32;
                self.me().timers.retain(|t| t.id != id);
                none()
            }
            6 => {
                let name = check_text(lua, &args, 0, "require")?;
                if let Some(v) = self.me().modules.get(&name) {
                    return Ok(vec![v.clone()]);
                }
                let Some(folder) = self.me().folder.clone() else {
                    return Err(lua.rt_error("require works in folder mods (a folder with main.lua)"));
                };
                let rel = module_path(&name).ok_or_else(|| lua.rt_error(format!("bad module name '{name}'")))?;
                let path = folder.join(&rel);
                let src =
                    std::fs::read(&path).map_err(|_| lua.rt_error(format!("module '{name}' not found ({rel})")))?;
                let env = self
                    .me()
                    .env
                    .clone()
                    .ok_or_else(|| lua.rt_error("require: the mod is not running"))?;
                let chunk = format!("{}/{rel}", self.s.owner_name(self.owner));
                let f = lua.load_bytes(&src, &chunk, &env).map_err(|e| lua.rt_error(e))?;
                let r = lua.call(self, &f, vec![Value::str(&name)])?;
                let v = match r.into_iter().next() {
                    Some(v) if !v.is_nil() => v,
                    _ => Value::Bool(true),
                };
                self.me().modules.insert(name, v.clone());
                Ok(vec![v])
            }
            7 => {
                let c = |i: usize, d: f64| opt_num(lua, &args, i, "color", d).map(|v| v.clamp(0.0, 255.0) as u32);
                let (r, g, b, a) = (c(0, 0.0)?, c(1, 0.0)?, c(2, 0.0)?, c(3, 255.0)?);
                Ok(vec![Value::Num(f64::from((a << 24) | (r << 16) | (g << 8) | b))])
            }
            10 => {
                let text = lua.tostring(self, &arg(&args, 0))?;
                let line = match self.owner {
                    Owner::Console => text,
                    Owner::Mod(i) => format!("[{}] {text}", self.s.mods[i].name),
                };
                self.game.chat(&line);
                none()
            }
            11 => {
                let text = check_text(lua, &args, 0, "say")?;
                let text = text.trim_start_matches('/');
                self.send(lua, text)?;
                none()
            }
            20 => {
                let cmd = check_text(lua, &args, 0, "run")?;
                let cmd = format!("/{}", cmd.trim().trim_start_matches('/'));
                self.send(lua, &cmd)?;
                none()
            }
            21 => Ok(self.game.fps().iter().map(|v| Value::Num(f64::from(*v))).collect()),
            22 => Ok(vec![Value::str(&self.game.screen())]),
            23 => Ok(vec![Value::Num(self.s.now)]),
            24 => Ok(vec![Value::Num(self.game.ticks() as f64)]),
            30..=38 => {
                let Some(p) = self.game.player() else {
                    return Ok(vec![Value::Nil]);
                };
                Ok(match id {
                    30 => pos(p.x, p.y, p.z),
                    31 => pos(p.x.floor(), p.y.floor(), p.z.floor()),
                    32 => vec![Value::Num(f64::from(p.yaw)), Value::Num(f64::from(p.pitch))],
                    33 => vec![Value::str(facing(p.yaw))],
                    34 => vec![Value::str(&p.dimension)],
                    35 => vec![Value::Num(f64::from(p.health)), Value::Num(f64::from(p.max_health))],
                    36 => vec![Value::Num(f64::from(p.food)), Value::Num(f64::from(p.saturation))],
                    37 => match &p.held {
                        Some((item, n)) => vec![Value::str(item), Value::Num(f64::from(*n))],
                        None => vec![Value::Nil],
                    },
                    _ => vec![player_table(&p)],
                })
            }
            40 => {
                let x = check_num(lua, &args, 0, "block")?.floor() as i32;
                let y = check_num(lua, &args, 1, "block")?.floor() as i32;
                let z = check_num(lua, &args, 2, "block")?.floor() as i32;
                Ok(vec![self.game.block(x, y, z).map_or(Value::Nil, |b| Value::str(&b))])
            }
            41 => match self.game.clock() {
                Some((day, game)) => Ok(vec![
                    Value::Num(day.rem_euclid(24000) as f64),
                    Value::Num(game as f64),
                    Value::Num(day.div_euclid(24000) as f64 + 1.0),
                ]),
                None => Ok(vec![Value::Nil]),
            },
            42 => {
                let block = check_text(lua, &args, 0, "find")?;
                let radius = opt_num(lua, &args, 1, "find", 32.0)?.clamp(1.0, 64.0) as i32;
                if !block.chars().all(|c| c.is_ascii_alphanumeric() || "_:./-".contains(c)) {
                    return Err(bad_arg(lua, 0, "find", "a block id expected"));
                }
                self.query_now(lua, &format!("find-block {block} {radius}"))
            }
            43 => {
                let radius = opt_num(lua, &args, 0, "entities", 16.0)?.clamp(1.0, 128.0);
                let kind = opt_str(lua, &args, 1, "entities")?.map(|s| text_of(&s));
                let line = match kind {
                    Some(k) if k.chars().all(|c| c.is_ascii_alphanumeric() || "_:-".contains(c)) => {
                        format!("entities {radius} {k}")
                    }
                    Some(_) => return Err(bad_arg(lua, 1, "entities", "an entity type expected")),
                    None => format!("entities {radius}"),
                };
                self.query_now(lua, &line)
            }
            50 => {
                if args.first().is_none_or(Value::is_nil) {
                    self.me().panel = None;
                    return none();
                }
                let c = check_text(lua, &args, 0, "panel")?;
                let corner = Corner::parse(&c).ok_or_else(|| {
                    bad_arg(
                        lua,
                        0,
                        "panel",
                        "\"top-left\", \"top-right\", \"bottom-left\" or \"bottom-right\" expected",
                    )
                })?;
                let lines = match args.get(1) {
                    Some(Value::Table(t)) => {
                        let list = t.borrow().list();
                        let mut out = Vec::new();
                        for v in list.iter().take(16) {
                            let mut s = lua.tostring(self, v)?;
                            s.truncate(256);
                            out.push(s);
                        }
                        out
                    }
                    Some(v) => vec![lua.tostring(self, v)?],
                    None => Vec::new(),
                };
                self.me().panel = Some((corner, lines));
                none()
            }
            51 => {
                self.drawing(lua, "text")?;
                let x = check_num(lua, &args, 0, "text")? as f32;
                let y = check_num(lua, &args, 1, "text")? as f32;
                let text = lua.tostring(self, &arg(&args, 2))?;
                let color = color_arg(lua, &args, 3, 0xFFFF_FFFF)?;
                let shadow = !matches!(args.get(4), Some(Value::Bool(false)));
                self.game.text(&text, [x, y], color & 0xFF_FFFF, shadow);
                none()
            }
            52 => {
                self.drawing(lua, "rect")?;
                let mut r = [0f32; 4];
                for (i, v) in r.iter_mut().enumerate() {
                    *v = check_num(lua, &args, i, "rect")? as f32;
                }
                let color = color_arg(lua, &args, 4, 0x7800_0000)?;
                self.game.fill(r, color);
                none()
            }
            53 => {
                let text = lua.tostring(self, &arg(&args, 0))?;
                Ok(vec![Value::Num(f64::from(self.game.text_width(&text)))])
            }
            54 => {
                let (w, h) = self.game.gui_size();
                Ok(vec![Value::Num(f64::from(w)), Value::Num(f64::from(h))])
            }
            60 => {
                let key = check_text(lua, &args, 0, "down")?;
                Ok(vec![Value::Bool(self.s.held.contains(&key))])
            }
            61 => {
                let name = check_text(lua, &args, 0, "mapping")?;
                match self.game.mapping(&name) {
                    Some((held, label)) => Ok(vec![Value::Bool(held), Value::str(&label)]),
                    None => Err(bad_arg(
                        lua,
                        0,
                        "mapping",
                        "a vanilla mapping such as \"key.jump\" expected",
                    )),
                }
            }
            70 => {
                let z = opt_num(lua, &args, 0, "zoom", 1.0)?;
                self.me().zoom = (z.is_finite() && z > 1.0).then_some(z.min(10.0) as f32);
                none()
            }
            80 => {
                let line = check_text(lua, &args, 0, "query")?;
                self.query_now(lua, &line)
            }
            81 => {
                let line = check_text(lua, &args, 0, "run")?;
                let cb = match args.get(1) {
                    None | Some(Value::Nil) => Value::Nil,
                    Some(_) => check_func(lua, &args, 1, "run")?,
                };
                match self.agent(lua, &line)? {
                    Reply::Now(v) => {
                        let vals = match v.get("error").and_then(|e| e.as_str()) {
                            Some(err) => vec![Value::Nil, Value::str(err)],
                            None => vec![json_to_lua(&v)],
                        };
                        self.me().ready.push((cb, vals));
                    }
                    Reply::Later(rx) => {
                        if self.me().pending.len() >= 64 {
                            return Err(lua.rt_error("too many agent actions waiting (64)"));
                        }
                        self.me().pending.push((rx, cb));
                    }
                }
                none()
            }
            82 => {
                let _ = self.agent(lua, "stop")?;
                none()
            }
            90 => {
                let key = storage_key(lua, &args)?;
                let Ok(text) = std::fs::read_to_string(self.storage_dir().join(format!("{key}.lua"))) else {
                    return Ok(vec![Value::Nil]);
                };
                let empty = Rc::new(std::cell::RefCell::new(Table::new()));
                let f = lua
                    .load(&format!("return {text}"), "storage", &empty)
                    .map_err(|e| lua.rt_error(format!("stored value '{key}' is damaged: {e}")))?;
                let v = lua.call(&mut NoHost::default(), &f, Vec::new())?;
                Ok(vec![v.into_iter().next().unwrap_or_default()])
            }
            91 => {
                let key = storage_key(lua, &args)?;
                let dir = self.storage_dir();
                let path = dir.join(format!("{key}.lua"));
                let v = arg(&args, 1);
                if v.is_nil() {
                    let _ = std::fs::remove_file(path);
                    return none();
                }
                let mut text = String::new();
                serialize(&v, 0, &mut text).map_err(|e| lua.rt_error(e))?;
                let count = std::fs::read_dir(&dir).map_or(0, |d| d.count());
                if count >= 256 && !path.exists() {
                    return Err(lua.rt_error("this mod stores too many keys (256)"));
                }
                std::fs::create_dir_all(&dir)
                    .and_then(|()| std::fs::write(&path, text))
                    .map_err(|e| lua.rt_error(format!("could not store '{key}': {e}")))?;
                none()
            }
            100 => {
                let m = self.me();
                let mut t = Table::new();
                t.set_str("id", Value::str(&m.id));
                t.set_str("name", Value::str(&m.name));
                t.set_str("version", Value::str(&m.version));
                t.set_str("author", Value::str(&m.author));
                t.set_str("reloads", Value::Num(f64::from(m.reloads)));
                Ok(vec![Value::table(t)])
            }
            200..=205 if self.owner != Owner::Console => Err(lua.rt_error("this works only in the console")),
            200 => {
                let want = opt_str(lua, &args, 0, "help")?.map(|s| text_of(&s));
                let mut lines = Vec::new();
                for (table, name, _, doc) in API {
                    let full = if table.is_empty() {
                        (*name).to_owned()
                    } else {
                        format!("{table}.{name}")
                    };
                    if want.as_ref().is_none_or(|w| full.contains(w.as_str())) {
                        lines.push((*doc).to_owned());
                    }
                }
                for (name, _, doc) in CONSOLE_API {
                    if want.as_ref().is_none_or(|w| name.contains(w.as_str())) {
                        lines.push((*doc).to_owned());
                    }
                }
                if lines.is_empty() {
                    lines.push("nothing by that name".into());
                }
                self.s.log(LogKind::Info, lines.join("\n"));
                none()
            }
            201 => {
                self.s.log.clear();
                self.s.console_mod.clear();
                self.s.console_mod.state = ModState::Running;
                none()
            }
            202 | 204 => {
                // The studio does these once the call returns: they need the interpreter.
                let which = opt_str(lua, &args, 0, "reload")?.map(|s| text_of(&s));
                if let Some(w) = &which
                    && !self.s.mods.iter().any(|m| &m.id == w)
                {
                    return Err(lua.rt_error(format!("no script mod '{w}'")));
                }
                self.s.requests.push(match (which, id == 202) {
                    (which, true) => Request::Reload(which),
                    (Some(w), false) => Request::Edit(w),
                    (None, false) => return Err(bad_arg(lua, 0, "edit", "a mod id expected")),
                });
                none()
            }
            203 => {
                let mut t = Table::new();
                for m in &self.s.mods {
                    let state = match &m.state {
                        ModState::Off => "off".to_owned(),
                        ModState::Running => "running".to_owned(),
                        ModState::Failed(msg, _) => format!("failed: {msg}"),
                    };
                    t.set_str(&m.id, Value::str(&state));
                }
                Ok(vec![Value::table(t)])
            }
            205 => {
                let name = check_str(lua, &args, 0, "new")?;
                self.s.requests.push(Request::New(text_of(&name)));
                none()
            }
            _ => Err(lua.rt_error(format!("unknown studio function {id}"))),
        }
    }
}
