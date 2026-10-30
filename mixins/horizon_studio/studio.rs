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
    check_table, check_text, opt_num, opt_str,
};
use super::hstudio_lualib::{inspect, new_env, quote, text_of};
use super::hstudio_ui::{Editor, ScriptScreen, Ui};
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
    /// Shows a folder in the system's file manager.
    fn open_folder(&mut self, path: &Path);
    /// The camera of the frame being drawn (while the world draws), else of the last one.
    fn camera(&self) -> Option<Camera>;
    /// A solid quad in world space (`0xAARRGGBB`), seen from both sides and
    /// hidden behind blocks: only while the world draws.
    fn world_quad(&mut self, corners: [[f64; 3]; 4], argb: u32);
    /// A filled four-cornered shape on the GUI, while drawing.
    fn quad(&mut self, corners: [[f32; 2]; 4], argb: u32);
    /// Plays a sound event (`minecraft:block.note_block.bell`), at a place or not.
    fn sound(&mut self, id: &str, at: Option<[f64; 3]>, volume: f32, pitch: f32) -> Result<(), String>;
    /// Spawns a particle (`minecraft:flame`); `color` for those that take one (dust).
    fn particle(&mut self, id: &str, at: [f64; 3], velocity: [f64; 3], color: Option<[f32; 4]>) -> Result<(), String>;
    /// The studio's own screens draw with Horizon's design system, in dp
    /// (the launcher's logical pixels): the screen in dp, and dp per GUI pixel.
    fn dp(&mut self) -> (f32, f32, f32);
    /// A rounded rectangle and a 1 dp rounded border, in dp (`0xAARRGGBB`).
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32);
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32);
    /// Text in Geist or Geist Mono whose line box (1.45 x size) has its top at
    /// `at`; returns its width.
    fn dp_text(&mut self, text: &str, at: [f32; 2], size: f32, font: Font, argb: u32) -> f32;
    fn dp_measure(&mut self, text: &str, size: f32, font: Font) -> f32;
}

/// Horizon's fonts: Geist and Geist Mono.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular,
    Medium,
    Mono,
}

/// A frame's camera: where it is, where it looks, its vertical field of view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

impl Camera {
    /// Forward, up and right, as vanilla's camera turns (yaw 0 looks south, +Z).
    pub fn basis(&self) -> [[f64; 3]; 3] {
        let (sy, cy) = (
            f64::from(self.yaw).to_radians().sin(),
            f64::from(self.yaw).to_radians().cos(),
        );
        let (sp, cp) = (
            f64::from(self.pitch).to_radians().sin(),
            f64::from(self.pitch).to_radians().cos(),
        );
        let f = [-sy * cp, -sp, cy * cp];
        let u = [-sy * sp, cp, cy * sp];
        [f, u, cross(f, u)]
    }
    /// How far in front of the camera a point is.
    pub fn depth(&self, p: [f64; 3]) -> f64 {
        dot(self.basis()[0], sub(p, self.pos))
    }
    /// A world point on a `w` x `h` GUI: its x, y and depth; None behind the camera.
    pub fn project(&self, p: [f64; 3], w: f32, h: f32) -> Option<(f32, f32, f64)> {
        let [f, u, r] = self.basis();
        let o = sub(p, self.pos);
        let depth = dot(f, o);
        if depth < NEAR {
            return None;
        }
        let focal = 1.0 / (f64::from(self.fov) / 2.0).to_radians().tan();
        let aspect = f64::from(w / h.max(1.0));
        let sx = dot(r, o) * focal / depth / aspect;
        let sy = dot(u, o) * focal / depth;
        Some((
            ((1.0 + sx) * f64::from(w) / 2.0) as f32,
            ((1.0 - sy) * f64::from(h) / 2.0) as f32,
            depth,
        ))
    }
}

const NEAR: f64 = 0.05;

pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// What `world` handlers drew through walls: drawn on the HUD.
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    Line {
        a: [f64; 3],
        b: [f64; 3],
        argb: u32,
        width: f32,
    },
    Text {
        at: [f64; 3],
        text: String,
        argb: u32,
        distance: bool,
    },
}

/// Bundled mods every studio gets once: libraries others build on.
pub const PREINSTALLED: &[&str] = &["uikit"];

/// The most 3D quads, through-wall shapes, particles and sounds per frame.
pub const MAX_WORLD_QUADS: usize = 16384;
pub const MAX_OVERLAY: usize = 4096;
pub const MAX_PARTICLES: usize = 512;
pub const MAX_SOUNDS: usize = 32;

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
    pub world: Vec<Value>,
    pub key: Vec<Value>,
    pub chat: Vec<Value>,
    pub unload: Vec<Value>,
    /// Events mods send each other (`emit`): names with a `:` in them.
    pub custom: HashMap<String, Vec<Value>>,
}

impl Handlers {
    fn list(&mut self, event: &str) -> Option<&mut Vec<Value>> {
        Some(match event {
            "tick" => &mut self.tick,
            "frame" => &mut self.frame,
            "hud" => &mut self.hud,
            "world" => &mut self.world,
            "key" => &mut self.key,
            "chat" => &mut self.chat,
            "unload" => &mut self.unload,
            custom if custom.contains(':') && custom.len() <= 64 => self.custom.entry(custom.to_owned()).or_default(),
            _ => return None,
        })
    }
    pub fn count(&self) -> usize {
        self.tick.len()
            + self.frame.len()
            + self.hud.len()
            + self.world.len()
            + self.key.len()
            + self.chat.len()
            + self.unload.len()
            + self.custom.values().map(Vec::len).sum::<usize>()
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
    /// What it offers other mods (`export`, or the table its main file returns).
    pub exports: Option<Value>,
    /// The libraries it imported, by id.
    pub imports: Vec<String>,
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
            exports: None,
            imports: Vec::new(),
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
        self.exports = None;
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
    /// When it was written (the studio's clock), for its fade-in.
    pub at: f64,
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
    /// Inside a `world` handler: `draw.*` works, as the world is drawn.
    World,
    /// Drawing a script's screen: like `Hud`.
    Screen,
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
    /// Bundled libraries installed once already (deleting one keeps it gone).
    pub seen: HashSet<String>,
    send_tokens: f64,
    next_timer: u32,
    queued_keys: Vec<(String, bool)>,
    queued_commands: Vec<String>,
    queued_chat: Vec<String>,
    /// Events mods sent (`emit`), for their handlers next frame.
    queued_events: Vec<(String, Vec<Value>)>,
    /// What console functions asked for, done after their call.
    requests: Vec<Request>,
    /// The script screen showing, and one a script asked to open.
    pub screen: Option<ScriptScreen>,
    pub screen_request: Option<(Owner, TableRef)>,
    /// What `world` handlers drew through walls, for the HUD.
    pub overlay: Vec<Overlay>,
    /// 3D quads, particles and sounds this frame (they are capped).
    world_quads: usize,
    particles: usize,
    sounds: usize,
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
                at: self.now,
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
                seen: HashSet::new(),
                send_tokens: SEND_BURST,
                next_timer: 1,
                queued_keys: Vec::new(),
                queued_commands: Vec::new(),
                queued_chat: Vec::new(),
                queued_events: Vec::new(),
                requests: Vec::new(),
                screen: None,
                screen_request: None,
                overlay: Vec::new(),
                world_quads: 0,
                particles: 0,
                sounds: 0,
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
                } else if let Some(id) = line.strip_prefix("seen:") {
                    self.s.seen.insert(id.trim().to_owned());
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(self.s.dir.join("history.txt")) {
            self.s.history = text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
        }
        // Bundled libraries come with the studio, once.
        let mut fresh = false;
        for id in PREINSTALLED {
            if self.s.seen.insert((*id).to_owned()) {
                fresh = true;
                let taken = mods.join(format!("{id}.lua")).exists() || mods.join(id).exists();
                if let Some(ex) = EXAMPLES.iter().find(|e| e.id == *id)
                    && !taken
                {
                    let _ = std::fs::create_dir_all(&mods);
                    let _ = std::fs::write(mods.join(format!("{id}.lua")), ex.source);
                }
            }
        }
        if fresh {
            self.save_settings();
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
        let mut text: String = ids.iter().map(|id| format!("off:{id}\n")).collect();
        let mut seen: Vec<&String> = self.s.seen.iter().collect();
        seen.sort();
        text.extend(seen.iter().map(|id| format!("seen:{id}\n")));
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
        let mut removed = Vec::new();
        for i in (0..self.s.mods.len()).rev() {
            if !found.iter().any(|f| f.0 == self.s.mods[i].id) {
                self.unload(game, i);
                let m = self.s.mods.remove(i);
                removed.push(m.id.clone());
                let shift = |o: &mut Owner| {
                    if let Owner::Mod(j) = o
                        && *j > i
                    {
                        *j -= 1;
                    }
                };
                if let Some(sc) = &mut self.s.screen {
                    shift(&mut sc.owner);
                }
                if let Some(r) = &mut self.s.screen_request {
                    shift(&mut r.0);
                }
                self.s.log(LogKind::Info, format!("Removed {}", m.id));
                self.s.ui.library_sel = self.s.ui.library_sel.min(self.s.mods.len().saturating_sub(1));
            }
        }
        if self
            .s
            .mods
            .iter()
            .any(|m| m.imports.iter().any(|d| removed.contains(d)))
        {
            self.reload_users(game, removed);
        }
        let mut fresh = Vec::new();
        let mut changed = Vec::new();
        for (id, main, folder) in found {
            match self.s.mods.iter().position(|m| m.id == id) {
                Some(i) => {
                    let stamp = Self::stamp(&main, folder.as_deref());
                    if stamp != self.s.mods[i].stamp {
                        self.s.mods[i].main = main;
                        self.s.mods[i].folder = folder;
                        if self.s.mods[i].enabled {
                            changed.push(i);
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
                        fresh.push(i);
                    } else {
                        self.s.mods[i].stamp = Self::stamp(&self.s.mods[i].main, self.s.mods[i].folder.as_deref());
                        self.read_meta(i);
                    }
                }
            }
        }
        // Libraries load before the mods that import them.
        for i in self.load_order(&fresh) {
            self.load(game, i);
        }
        // Mods that were waiting for one of these.
        let fresh_ids: Vec<String> = fresh.iter().map(|&i| self.s.mods[i].id.clone()).collect();
        if !fresh_ids.is_empty()
            && self
                .s
                .mods
                .iter()
                .any(|m| m.imports.iter().any(|d| fresh_ids.contains(d)))
        {
            self.reload_users(game, fresh_ids);
        }
        for i in self.load_order(&changed) {
            self.s.mods[i].reloads += 1;
            let t = Instant::now();
            self.load_and_dependents(game, i);
            if self.s.mods[i].running() {
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let msg = format!("Reloaded {} ({ms:.1} ms)", self.s.mods[i].id);
                self.s.log(LogKind::Info, &msg);
                self.s.toast(msg);
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
        // A failed mod keeps its imports, to retry when they change.
        self.s.mods[i].imports.clear();
        self.s.mods[i].state = ModState::Off;
        self.drop_screens(Owner::Mod(i));
    }

    /// A mod that stops takes its screen with it.
    fn drop_screens(&mut self, owner: Owner) {
        if self.s.screen.as_ref().is_some_and(|sc| sc.owner == owner) {
            self.s.screen = None;
        }
        if self.s.screen_request.as_ref().is_some_and(|r| r.0 == owner) {
            self.s.screen_request = None;
        }
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
        match self.call(game, Owner::Mod(i), &f, Vec::new(), LOAD_STEPS) {
            Ok(vals) => {
                // A main file that returns a table offers it, as a module does.
                if self.s.mods[i].exports.is_none()
                    && let Some(v @ Value::Table(_)) = vals.into_iter().next()
                {
                    self.s.mods[i].exports = Some(v);
                }
            }
            Err(e) => self.fail(game, Owner::Mod(i), e.message(), e.traceback().to_owned()),
        }
    }

    /// Loads mod `i`, then reloads the mods that use it (they hold its old
    /// exports), and theirs in turn.
    pub fn load_and_dependents(&mut self, game: &mut dyn GameApi, i: usize) {
        self.load(game, i);
        self.reload_users(game, vec![self.s.mods[i].id.clone()]);
    }

    /// Reloads the mods that import any of `libs`, and the mods that import
    /// those: they hold old exports (or failed for want of them).
    fn reload_users(&mut self, game: &mut dyn GameApi, libs: Vec<String>) {
        let mut done = libs.clone();
        let mut queue = libs;
        while let Some(lib) = queue.pop() {
            let users: Vec<usize> = (0..self.s.mods.len())
                .filter(|&j| {
                    let m = &self.s.mods[j];
                    m.enabled && m.imports.contains(&lib) && !done.contains(&m.id)
                })
                .collect();
            for j in users {
                done.push(self.s.mods[j].id.clone());
                queue.push(self.s.mods[j].id.clone());
                self.load(game, j);
                self.s
                    .log(LogKind::Info, format!("Reloaded {} (it uses {lib})", self.s.mods[j].id));
            }
        }
    }

    /// The order to load mods in: each after the libraries its files name
    /// in `import("...")`, as far as that can be read.
    fn load_order(&self, ids: &[usize]) -> Vec<usize> {
        let deps: Vec<Vec<String>> = ids
            .iter()
            .map(|&i| {
                let m = &self.s.mods[i];
                let mut files = vec![m.main.clone()];
                if let Some(dir) = &m.folder {
                    files.extend(Self::stamp(&m.main, Some(dir)).into_iter().map(|s| s.0));
                }
                files.dedup();
                files
                    .iter()
                    .filter_map(|f| std::fs::read_to_string(f).ok())
                    .flat_map(|src| static_imports(&src))
                    .filter(|d| *d != m.id)
                    .collect()
            })
            .collect();
        let mut order = Vec::new();
        let mut placed = vec![false; ids.len()];
        // Each pass places those whose libraries are placed (or not among these).
        loop {
            let mut progress = false;
            for k in 0..ids.len() {
                if placed[k] {
                    continue;
                }
                let waiting = deps[k]
                    .iter()
                    .any(|d| (0..ids.len()).any(|j| !placed[j] && j != k && self.s.mods[ids[j]].id == *d));
                if !waiting {
                    placed[k] = true;
                    order.push(ids[k]);
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        // A cycle: the rest in their own order (their imports will say so).
        order.extend((0..ids.len()).filter(|&k| !placed[k]).map(|k| ids[k]));
        order
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
                self.drop_screens(owner);
            }
        }
    }

    /// The studio's screen or a script's is up (the game's input is ours).
    pub fn overlay_open(&self) -> bool {
        self.s.ui.open || self.s.screen.is_some()
    }

    /// Shows the screen a script asked for (`ui.open`), once the game has
    /// made way for it.
    pub fn accept_screen(&mut self) -> bool {
        match self.s.screen_request.take() {
            Some((owner, table)) => {
                self.s.screen = Some(ScriptScreen::new(owner, table));
                self.s.ui.opened_at = self.s.now;
                self.s.ui.closing_at = None;
                true
            }
            None => false,
        }
    }

    /// Closes the script screen, calling its `close` handler.
    pub fn close_screen(&mut self, game: &mut dyn GameApi) {
        let Some(sc) = self.s.screen.take() else {
            return;
        };
        let f = sc.table.borrow().get_str("close");
        if f.is_callable() {
            self.s.screen = Some(sc);
            self.screen_call(game, &f, Vec::new(), false);
            self.s.screen = None;
        }
    }

    /// Calls a function of the script screen's table for its owner; a
    /// failure stops the mod (and closes the screen). `drawing` lets it draw.
    pub fn screen_call(
        &mut self,
        game: &mut dyn GameApi,
        f: &Value,
        args: Vec<Value>,
        drawing: bool,
    ) -> Option<Vec<Value>> {
        let owner = self.s.screen.as_ref()?.owner;
        if !f.is_callable() {
            return None;
        }
        let phase = if drawing { Phase::Screen } else { Phase::Normal };
        match self.call_in(game, owner, f, args, EVENT_STEPS, phase) {
            Ok(v) => Some(v),
            Err(e) => {
                self.fail(
                    game,
                    owner,
                    format!("screen: {}", e.message()),
                    e.traceback().to_owned(),
                );
                self.s.screen = None;
                None
            }
        }
    }

    pub fn set_enabled(&mut self, game: &mut dyn GameApi, i: usize, on: bool) {
        let id = self.s.mods[i].id.clone();
        self.s.mods[i].enabled = on;
        if on {
            self.s.disabled.remove(&id);
            self.load_and_dependents(game, i);
        } else {
            self.s.disabled.insert(id.clone());
            self.unload(game, i);
            self.reload_users(game, vec![id]);
        }
        self.save_settings();
    }

    pub fn reload_all(&mut self, game: &mut dyn GameApi) {
        let on: Vec<usize> = (0..self.s.mods.len()).filter(|&i| self.s.mods[i].enabled).collect();
        for i in self.load_order(&on) {
            self.load(game, i);
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

    /// Installs what was dropped on the game window: a `.lua` file, a mod
    /// folder (with `main.lua`), or a `.zip` of either. A mod of the same id
    /// and kind is replaced (an update). Returns a line saying what happened.
    pub fn install_path(&mut self, game: &mut dyn GameApi, path: &Path) -> Result<String, String> {
        self.start(game);
        let mods = self.s.mods_dir();
        std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("that file has no name")?
            .to_owned();
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        let (id, files) = if path.is_dir() {
            if !path.join("main.lua").is_file() {
                return Err(format!("{name} has no main.lua: a mod folder starts there"));
            }
            (mod_id(&name), Some(read_folder(path)?))
        } else {
            match ext.as_deref() {
                Some("lua") => {
                    let stem = name.trim_end_matches(".lua").trim_end_matches(".LUA");
                    let bytes = read_capped(path)?;
                    let id = mod_id(stem);
                    if mods.join(&id).is_dir() {
                        return Err(format!("a folder mod named {id} is installed: remove it first"));
                    }
                    let existed = mods.join(format!("{id}.lua")).exists();
                    std::fs::write(mods.join(format!("{id}.lua")), bytes).map_err(|e| e.to_string())?;
                    return Ok(self.after_install(game, &id, existed));
                }
                Some("zip") => {
                    let (id, layout) = read_zip(path, name.trim_end_matches(".zip").trim_end_matches(".ZIP"))?;
                    match layout {
                        ZipMod::File(bytes) => {
                            if mods.join(&id).is_dir() {
                                return Err(format!("a folder mod named {id} is installed: remove it first"));
                            }
                            let existed = mods.join(format!("{id}.lua")).exists();
                            std::fs::write(mods.join(format!("{id}.lua")), bytes).map_err(|e| e.to_string())?;
                            return Ok(self.after_install(game, &id, existed));
                        }
                        ZipMod::Folder(files) => (id, Some(files)),
                    }
                }
                _ => {
                    return Err(format!(
                        "{name}: Horizon Studio installs .lua files, mod folders (with main.lua) and .zip files of them"
                    ));
                }
            }
        };
        let files = files.unwrap_or_default();
        if mods.join(format!("{id}.lua")).is_file() {
            return Err(format!("a one-file mod named {id} is installed: remove it first"));
        }
        // Written beside, then swapped in, so a reload never sees half a mod.
        let dest = mods.join(&id);
        let fresh = mods.join(format!(".{id}.installing"));
        let _ = std::fs::remove_dir_all(&fresh);
        for (rel, bytes) in &files {
            let target = fresh.join(rel);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
        }
        let existed = dest.exists();
        if existed {
            std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&fresh, &dest).map_err(|e| e.to_string())?;
        Ok(self.after_install(game, &id, existed))
    }

    fn after_install(&mut self, game: &mut dyn GameApi, id: &str, existed: bool) -> String {
        self.scan(game);
        let verb = if existed { "Updated" } else { "Installed" };
        let line = match self.s.mods.iter().find(|m| m.id == id).map(|m| &m.state) {
            Some(ModState::Running) => format!("{verb} {id}: running"),
            Some(ModState::Failed(msg, _)) => format!("{verb} {id}, but it failed: {msg}"),
            Some(ModState::Off) => format!("{verb} {id} (turned off in the Library)"),
            None => format!("{verb} {id}"),
        };
        self.s.log(LogKind::Info, &line);
        self.s.toast(line.clone());
        if let Some(i) = self.s.mods.iter().position(|m| m.id == id) {
            self.s.ui.library_sel = i;
        }
        line
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
                if !m.running() || (event.contains(':') && !m.handlers.custom.contains_key(event)) {
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
        self.s.particles = 0;
        self.s.sounds = 0;
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
        for (name, args) in std::mem::take(&mut self.s.queued_events) {
            self.dispatch(game, &name, &args, Phase::Normal);
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

    /// As the world is drawn: `world` handlers, whose `draw.*` shapes go
    /// into the frame (and through walls, onto the HUD).
    pub fn world(&mut self, game: &mut dyn GameApi) {
        if !self.s.started {
            return;
        }
        self.s.overlay.clear();
        self.s.world_quads = 0;
        self.dispatch(game, "world", &[], Phase::World);
    }

    /// The `world` handlers' through-wall lines and labels, on the GUI.
    fn draw_overlay(&mut self, game: &mut dyn GameApi) {
        let Some(cam) = game.camera() else {
            return;
        };
        let (w, h) = game.gui_size();
        for item in &self.s.overlay {
            match item {
                Overlay::Line { a, b, argb, width } => {
                    let (mut a, mut b) = (*a, *b);
                    let (da, db) = (cam.depth(a), cam.depth(b));
                    if da < NEAR && db < NEAR {
                        continue;
                    }
                    // The part behind the camera is cut off at the near plane.
                    if da < NEAR {
                        a = add(a, scale(sub(b, a), (NEAR - da) / (db - da) + 1e-6));
                    } else if db < NEAR {
                        b = add(b, scale(sub(a, b), (NEAR - db) / (da - db) + 1e-6));
                    }
                    let (Some((x1, y1, _)), Some((x2, y2, _))) = (cam.project(a, w, h), cam.project(b, w, h)) else {
                        continue;
                    };
                    let (dx, dy) = (x2 - x1, y2 - y1);
                    let len = (dx * dx + dy * dy).sqrt();
                    if !(0.01..=1e5).contains(&len) {
                        continue;
                    }
                    let (nx, ny) = (-dy / len * width / 2.0, dx / len * width / 2.0);
                    game.quad(
                        [
                            [x1 - nx, y1 - ny],
                            [x1 + nx, y1 + ny],
                            [x2 + nx, y2 + ny],
                            [x2 - nx, y2 - ny],
                        ],
                        *argb,
                    );
                }
                Overlay::Text {
                    at,
                    text,
                    argb,
                    distance,
                } => {
                    let Some((x, y, depth)) = cam.project(*at, w, h) else {
                        continue;
                    };
                    if x < -200.0 || y < -20.0 || x > w + 200.0 || y > h + 20.0 {
                        continue;
                    }
                    let label = if *distance {
                        let d = sub(*at, cam.pos);
                        format!("{text} ({:.0} m)", dot(d, d).sqrt())
                    } else {
                        text.clone()
                    };
                    let _ = depth;
                    let tw = game.text_width(&label);
                    let (lx, ly) = ((x - tw / 2.0).floor(), (y - 4.0).floor());
                    game.fill([lx - 2.0, ly - 1.0, tw + 4.0, 10.0], 0x8000_0000);
                    game.text(&label, [lx, ly], argb & 0xFF_FFFF, true);
                }
            }
        }
    }

    /// The HUD, under F1's rule: the world's through-wall shapes, `hud`
    /// handlers, the mods' panels, the toast.
    pub fn hud(&mut self, game: &mut dyn GameApi) {
        if !self.s.started {
            return;
        }
        self.draw_overlay(game);
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
        super::hstudio_look::draw_toast(self, game);
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

/// The most a dropped mod may hold.
const MAX_INSTALL_FILES: usize = 256;
const MAX_INSTALL_BYTES: u64 = 8 << 20;

fn read_capped(path: &Path) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_INSTALL_BYTES {
        return Err("that file is too large for a script mod (8 MiB at most)".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// A dropped mod folder's files, by their paths inside it (links and hidden
/// files left out).
fn read_folder(dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(kind) = e.file_type() else { continue };
            if name.starts_with('.') || kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if depth < 4 {
                    stack.push((e.path(), depth + 1));
                }
                continue;
            }
            let bytes = read_capped(&e.path())?;
            total += bytes.len() as u64;
            if out.len() >= MAX_INSTALL_FILES || total > MAX_INSTALL_BYTES {
                return Err("that folder is too large for a script mod (256 files, 8 MiB at most)".into());
            }
            let rel = e.path().strip_prefix(dir).map_err(|e| e.to_string())?.to_path_buf();
            out.push((rel, bytes));
        }
    }
    Ok(out)
}

enum ZipMod {
    File(Vec<u8>),
    Folder(Vec<(PathBuf, Vec<u8>)>),
}

/// A dropped zip: a folder mod (`main.lua` at its root, or in its one top
/// folder), or a single `.lua` file. The id comes from the folder or file.
fn read_zip(path: &Path, stem: &str) -> Result<(String, ZipMod), String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "that zip cannot be read".to_owned())?;
    if zip.len() > MAX_INSTALL_FILES * 2 {
        return Err("that zip holds too many files for a script mod".into());
    }
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            return Err("that zip has a file outside its folder".into());
        };
        if rel
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            || rel.starts_with("__MACOSX")
        {
            continue;
        }
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_INSTALL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len() as u64;
        if files.len() >= MAX_INSTALL_FILES || total > MAX_INSTALL_BYTES {
            return Err("that zip is too large for a script mod (256 files, 8 MiB at most)".into());
        }
        files.push((rel, bytes));
    }
    if files.iter().any(|(p, _)| p == Path::new("main.lua")) {
        return Ok((mod_id(stem), ZipMod::Folder(files)));
    }
    let tops: HashSet<_> = files
        .iter()
        .filter_map(|(p, _)| p.components().next())
        .map(|c| c.as_os_str().to_owned())
        .collect();
    if tops.len() == 1 {
        let top = PathBuf::from(tops.into_iter().next().unwrap_or_default());
        if files.iter().any(|(p, _)| *p == top.join("main.lua")) {
            let inner = files
                .into_iter()
                .filter_map(|(p, b)| p.strip_prefix(&top).ok().map(|r| (r.to_path_buf(), b)))
                .collect();
            return Ok((mod_id(&top.to_string_lossy()), ZipMod::Folder(inner)));
        }
    }
    let mut lua: Vec<(PathBuf, Vec<u8>)> = files
        .into_iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "lua"))
        .collect();
    if lua.len() == 1 {
        let (p, bytes) = lua.remove(0);
        let id = mod_id(
            &p.file_stem()
                .map_or_else(|| stem.to_owned(), |s| s.to_string_lossy().into_owned()),
        );
        return Ok((id, ZipMod::File(bytes)));
    }
    Err("that zip is not a script mod: it needs a main.lua, or a single .lua file".into())
}

/// gfx's optional size (6 to 96 dp, 14 by default) and font name.
fn font_args(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<(f32, Font), Throw> {
    let size = opt_num(lua, args, i, fname, 14.0)?;
    let size = if size.is_finite() {
        size.clamp(6.0, 96.0) as f32
    } else {
        14.0
    };
    let font = match opt_str(lua, args, i + 1, fname)?.map(|s| text_of(&s)).as_deref() {
        None | Some("regular") => Font::Regular,
        Some("medium" | "bold") => Font::Medium,
        Some("mono") => Font::Mono,
        Some(_) => {
            return Err(bad_arg(
                lua,
                i + 1,
                fname,
                "\"regular\", \"medium\" or \"mono\" expected",
            ));
        }
    };
    Ok((size, font))
}

/// The libraries a Lua file names in `import("...")` calls.
pub fn static_imports(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(at) = rest.find("import") {
        let before = rest[..at].chars().next_back();
        rest = &rest[at + 6..];
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == ':') {
            continue;
        }
        let t = rest.trim_start();
        let t = t.strip_prefix('(').map_or(t, str::trim_start);
        let Some(q) = t.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let body = &t[1..];
        if let Some(end) = body.find(q) {
            let name = &body[..end];
            if !name.is_empty()
                && name.len() <= 48
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                && !out.iter().any(|o| o == name)
            {
                out.push(name.to_owned());
            }
        }
    }
    out
}

/// A value as JSON: tables with keys 1..n are arrays, others objects.
pub fn lua_to_json(v: &Value, depth: usize) -> Result<serde_json::Value, String> {
    if depth > 64 {
        return Err("tables nested too deep for JSON (or a cycle)".into());
    }
    Ok(match v {
        Value::Nil => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => serde_json::Value::from(*n as i64),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, serde_json::Value::Number),
        Value::Str(s) => serde_json::Value::String(String::from_utf8_lossy(s).into_owned()),
        Value::Table(t) => {
            let t = t.borrow();
            let entries = t.entries();
            let n = t.len();
            if n > 0 && entries.len() == n {
                serde_json::Value::Array(
                    t.list()
                        .iter()
                        .map(|x| lua_to_json(x, depth + 1))
                        .collect::<Result<_, _>>()?,
                )
            } else {
                let mut map = serde_json::Map::new();
                for (k, val) in entries {
                    map.insert(k.plain_text(), lua_to_json(&val, depth + 1)?);
                }
                serde_json::Value::Object(map)
            }
        }
        Value::Func(_) | Value::Native(_) => return Err("functions have no JSON form".into()),
    })
}

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
        "on(event, fn): run fn on \"tick\" (20 a second), \"frame\"(dt), \"hud\"(w, h), \"world\" (as the world draws: draw.*), \"key\"(key, down), \"chat\"(text), \"unload\", or a \"mymod:name\" event another mod emits",
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
    (
        "",
        "export",
        8,
        "export(value): what this mod offers other mods, which get it with import(id); a main file that returns a table exports it too",
    ),
    (
        "",
        "import",
        9,
        "import(id): another mod's exports (a library's functions, a UI kit...); it loads first, and its users reload when it does",
    ),
    (
        "",
        "emit",
        12,
        "emit(event, ...): call every mod's on(event, fn) handlers next frame; the name has a ':' in it (\"mymod:changed\")",
    ),
    (
        "gfx",
        "size",
        150,
        "gfx.size(): the screen in dp, Horizon's logical pixels, which gfx.* draws in",
    ),
    (
        "gfx",
        "rect",
        151,
        "gfx.rect(x, y, w, h[, color[, radius]]): a rounded rectangle, in dp (inside on(\"hud\") or a screen's draw)",
    ),
    (
        "gfx",
        "border",
        152,
        "gfx.border(x, y, w, h[, color[, radius]]): a 1 dp rounded outline",
    ),
    (
        "gfx",
        "text",
        153,
        "gfx.text(x, y, text[, color[, size[, font]]]): Geist text (font \"regular\", \"medium\" or \"mono\"), y its line's top; returns its width",
    ),
    (
        "gfx",
        "measure",
        154,
        "gfx.measure(text[, size[, font]]): text's width in dp; a line is 1.45 x size tall",
    ),
    (
        "gfx",
        "mouse",
        155,
        "gfx.mouse(): x, y in dp, left held, right held, while this mod's screen shows; nil otherwise",
    ),
    (
        "json",
        "encode",
        140,
        "json.encode(value[, pretty]): a value as JSON text",
    ),
    (
        "json",
        "decode",
        141,
        "json.decode(text): JSON text as a value (objects and arrays become tables), or nil and the error",
    ),
    (
        "world",
        "particle",
        44,
        "world.particle(id, x, y, z[, dx, dy, dz[, count[, color]]]): particles (\"flame\", \"heart\", \"dust\" with a color...), dx dy dz their speed",
    ),
    (
        "draw",
        "line",
        110,
        "draw.line(x1, y1, z1, x2, y2, z2[, color[, width[, through]]]): a line in the world (inside on(\"world\")); width in blocks; through = true shows it through walls",
    ),
    (
        "draw",
        "box",
        111,
        "draw.box(x1, y1, z1, x2, y2, z2[, color[, width[, through]]]): a box's outline",
    ),
    (
        "draw",
        "block",
        112,
        "draw.block(x, y, z[, color[, width[, through]]]): the outline of a block",
    ),
    (
        "draw",
        "fill",
        113,
        "draw.fill(x1, y1, z1, x2, y2, z2[, color]): a solid box",
    ),
    (
        "draw",
        "beam",
        114,
        "draw.beam(x, z[, color[, width[, y1[, y2]]]]): a vertical beam, like a waypoint's",
    ),
    (
        "draw",
        "text",
        115,
        "draw.text(x, y, z, text[, color[, distance]]): a label at a place, seen through walls; distance = true adds how far it is",
    ),
    (
        "sound",
        "play",
        120,
        "sound.play(id[, volume[, pitch]]): a sound for you (\"minecraft:entity.experience_orb.pickup\")",
    ),
    (
        "sound",
        "at",
        121,
        "sound.at(id, x, y, z[, volume[, pitch]]): a sound from a place in the world",
    ),
    (
        "ui",
        "open",
        130,
        "ui.open(screen): show a screen: {title, width, widgets = {...}, draw(w, h), click(x, y, button), key(key, down), text(t), close()}; canvas = true draws it all yourself with gfx.* in dp, and adds move(x, y), release(x, y, button) and wheel(steps)",
    ),
    ("ui", "close", 131, "ui.close(): close this mod's screen"),
    ("ui", "is_open", 132, "ui.is_open(): whether this mod's screen shows"),
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
        if matches!(self.phase, Phase::Hud | Phase::Screen) {
            Ok(())
        } else {
            Err(lua.rt_error(format!(
                "hud.{what} draws only inside on(\"hud\", function(w, h) ... end) or a screen's draw"
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

impl Ctx<'_> {
    fn not_drawing(&self, lua: &Lua, what: &str) -> Result<(), Throw> {
        if matches!(self.phase, Phase::Hud | Phase::Screen | Phase::World) {
            return Err(lua.rt_error(format!(
                "{what} doesn't work while drawing: call it from tick, frame, a command or another event"
            )));
        }
        Ok(())
    }
    /// `draw.*`: shapes in the world, inside `on("world")`.
    fn draw(&mut self, lua: &Lua, id: u32, args: &[Value]) -> Res {
        let name = ["line", "box", "block", "fill", "beam", "text"][(id - 110) as usize];
        if self.phase != Phase::World {
            return Err(lua.rt_error(format!(
                "draw.{name} draws only inside on(\"world\", function() ... end)"
            )));
        }
        let n = |i: usize| check_num(lua, args, i, name);
        let through = |i: usize| args.get(i).is_some_and(Value::truthy);
        let width = |i: usize, d: f64| opt_num(lua, args, i, name, d).map(|w| w.clamp(0.002, 4.0));
        match id {
            110 => {
                let (a, b) = ([n(0)?, n(1)?, n(2)?], [n(3)?, n(4)?, n(5)?]);
                let c = color_arg(lua, args, 6, 0xFFFF_FFFF)?;
                let w = width(7, 0.04)?;
                self.line(lua, a, b, c, w, through(8))?;
            }
            111 | 112 => {
                let (lo, hi, ci) = if id == 111 {
                    let (a, b) = ([n(0)?, n(1)?, n(2)?], [n(3)?, n(4)?, n(5)?]);
                    (
                        [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                        [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
                        6,
                    )
                } else {
                    let p = [n(0)?.floor(), n(1)?.floor(), n(2)?.floor()];
                    let e = 0.002;
                    (
                        [p[0] - e, p[1] - e, p[2] - e],
                        [p[0] + 1.0 + e, p[1] + 1.0 + e, p[2] + 1.0 + e],
                        3,
                    )
                };
                let c = color_arg(lua, args, ci, 0xFFFF_FFFF)?;
                let w = width(ci + 1, 0.03)?;
                let t = through(ci + 2);
                for (a, b) in box_edges(lo, hi) {
                    self.line(lua, a, b, c, w, t)?;
                }
            }
            113 => {
                let (a, b) = ([n(0)?, n(1)?, n(2)?], [n(3)?, n(4)?, n(5)?]);
                let c = color_arg(lua, args, 6, 0xFFFF_FFFF)?;
                let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
                let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
                for face in box_faces(lo, hi) {
                    self.quad3(lua, face, c)?;
                }
            }
            114 => {
                let (x, z) = (n(0)?, n(1)?);
                let c = color_arg(lua, args, 2, 0xFFFF_FFFF)?;
                let half = width(3, 0.25)? / 2.0;
                let y1 = opt_num(lua, args, 4, name, -64.0)?;
                let y2 = opt_num(lua, args, 5, name, 320.0)?;
                let (lo, hi) = ([x - half, y1.min(y2), z - half], [x + half, y1.max(y2), z + half]);
                // Its four sides: the ends stay open, as a beacon's beam.
                for face in &box_faces(lo, hi)[2..] {
                    self.quad3(lua, *face, c)?;
                }
            }
            _ => {
                let at = [n(0)?, n(1)?, n(2)?];
                let text = check_text(lua, args, 3, name)?;
                let c = color_arg(lua, args, 4, 0xFFFF_FFFF)?;
                let distance = through(5);
                self.overlay(
                    lua,
                    Overlay::Text {
                        at,
                        text: text.chars().take(128).collect(),
                        argb: c,
                        distance,
                    },
                )?;
            }
        }
        Ok(Vec::new())
    }
    /// A line: a ribbon facing the camera, or through walls on the HUD.
    fn line(&mut self, lua: &Lua, a: [f64; 3], b: [f64; 3], argb: u32, width: f64, through: bool) -> Result<(), Throw> {
        if through {
            let px = (width * 40.0).clamp(1.0, 8.0) as f32;
            return self.overlay(lua, Overlay::Line { a, b, argb, width: px });
        }
        let Some(cam) = self.game.camera() else {
            return Ok(());
        };
        let mid = scale(add(a, b), 0.5);
        let n = cross(sub(b, a), sub(mid, cam.pos));
        let len = dot(n, n).sqrt();
        if len < 1e-9 {
            return Ok(());
        }
        let n = scale(n, width / 2.0 / len);
        self.quad3(lua, [sub(a, n), add(a, n), add(b, n), sub(b, n)], argb)
    }
    fn quad3(&mut self, lua: &Lua, corners: [[f64; 3]; 4], argb: u32) -> Result<(), Throw> {
        if self.s.world_quads >= MAX_WORLD_QUADS {
            return Err(lua.rt_error("too much drawn in the world this frame"));
        }
        self.s.world_quads += 1;
        self.game.world_quad(corners, argb);
        Ok(())
    }
    fn overlay(&mut self, lua: &Lua, item: Overlay) -> Result<(), Throw> {
        if self.s.overlay.len() >= MAX_OVERLAY {
            return Err(lua.rt_error("too much drawn through walls this frame"));
        }
        self.s.overlay.push(item);
        Ok(())
    }
}

/// A box's 12 edges.
pub fn box_edges(lo: [f64; 3], hi: [f64; 3]) -> Vec<([f64; 3], [f64; 3])> {
    let c = |i: usize| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    let mut out = Vec::with_capacity(12);
    for i in 0..8usize {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                out.push((c(i), c(i | bit)));
            }
        }
    }
    out
}

/// A box's 6 faces: bottom, top, then the four sides.
pub fn box_faces(lo: [f64; 3], hi: [f64; 3]) -> [[[f64; 3]; 4]; 6] {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    [
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
    ]
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
                            &format!(
                                "unknown event '{event}' (tick, frame, hud, world, key, chat, unload, or one of \"mymod:name\" for emit)"
                            ),
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
            8 => {
                let v = arg(&args, 0);
                self.me().exports = (!v.is_nil()).then_some(v);
                none()
            }
            9 => {
                let want = check_text(lua, &args, 0, "import")?;
                // Kept even if it fails, so this mod retries when `want` appears.
                if let Owner::Mod(i) = self.owner
                    && !self.s.mods[i].imports.contains(&want)
                    && self.s.mods[i].imports.len() < 64
                {
                    self.s.mods[i].imports.push(want.clone());
                }
                let Some(j) = self.s.mods.iter().position(|m| m.id == want) else {
                    return Err(lua.rt_error(format!("no mod '{want}' to import")));
                };
                if Owner::Mod(j) == self.owner {
                    return Err(lua.rt_error("a mod cannot import itself"));
                }
                let lib = &self.s.mods[j];
                let exports = match &lib.state {
                    ModState::Running => lib.exports.clone(),
                    ModState::Failed(msg, _) => {
                        return Err(lua.rt_error(format!("library '{want}' failed: {msg}")));
                    }
                    ModState::Off if !lib.enabled => {
                        return Err(lua.rt_error(format!("library '{want}' is turned off")));
                    }
                    ModState::Off => {
                        return Err(lua.rt_error(format!(
                            "library '{want}' isn't loaded yet (do they import each other?)"
                        )));
                    }
                };
                let Some(exports) = exports else {
                    return Err(lua.rt_error(format!(
                        "'{want}' exports nothing: it needs export(...) or to return a table"
                    )));
                };
                Ok(vec![exports])
            }
            12 => {
                let name = check_text(lua, &args, 0, "emit")?;
                if !name.contains(':') || name.len() > 64 {
                    return Err(bad_arg(
                        lua,
                        0,
                        "emit",
                        "an event name with a ':' expected (\"mymod:changed\")",
                    ));
                }
                if self.s.queued_events.len() >= 1024 {
                    return Err(lua.rt_error("too many events waiting (1024)"));
                }
                self.s
                    .queued_events
                    .push((name, args.into_iter().skip(1).take(16).collect()));
                none()
            }
            150 => {
                let (w, h, _) = self.game.dp();
                Ok(vec![Value::Num(f64::from(w)), Value::Num(f64::from(h))])
            }
            151 | 152 => {
                let name = if id == 151 { "rect" } else { "border" };
                self.drawing(lua, name)?;
                let mut r = [0f32; 4];
                for (i, v) in r.iter_mut().enumerate() {
                    *v = check_num(lua, &args, i, name)? as f32;
                }
                let color = color_arg(lua, &args, 4, 0xFFFF_FFFF)?;
                let radius = (opt_num(lua, &args, 5, name, 0.0)? as f32).clamp(0.0, r[2].min(r[3]).max(0.0) / 2.0);
                if r.iter().all(|v| v.is_finite()) && r[2] > 0.0 && r[3] > 0.0 {
                    if id == 151 {
                        self.game.dp_rect(r, radius, color);
                    } else {
                        self.game.dp_border(r, radius, color);
                    }
                }
                none()
            }
            153 => {
                self.drawing(lua, "text")?;
                let x = check_num(lua, &args, 0, "text")? as f32;
                let y = check_num(lua, &args, 1, "text")? as f32;
                let mut text = lua.tostring(self, &arg(&args, 2))?;
                text.truncate(1024);
                let color = color_arg(lua, &args, 3, 0xFFFF_FFFF)?;
                let (size, font) = font_args(lua, &args, 4, "text")?;
                let w = if x.is_finite() && y.is_finite() {
                    self.game.dp_text(&text, [x, y], size, font, color)
                } else {
                    0.0
                };
                Ok(vec![Value::Num(f64::from(w))])
            }
            154 => {
                let mut text = lua.tostring(self, &arg(&args, 0))?;
                text.truncate(1024);
                let (size, font) = font_args(lua, &args, 1, "measure")?;
                Ok(vec![Value::Num(f64::from(self.game.dp_measure(&text, size, font)))])
            }
            155 => {
                let me = self.owner;
                if !self.s.screen.as_ref().is_some_and(|sc| sc.owner == me) {
                    return Ok(vec![Value::Nil]);
                }
                let (x, y) = self.s.ui.mouse;
                let [l, r] = self.s.ui.buttons;
                Ok(vec![
                    Value::Num(f64::from(x)),
                    Value::Num(f64::from(y)),
                    Value::Bool(l),
                    Value::Bool(r),
                ])
            }
            140 => {
                let j = lua_to_json(&arg(&args, 0), 0).map_err(|e| lua.rt_error(e))?;
                let text = if args.get(1).is_some_and(Value::truthy) {
                    serde_json::to_string_pretty(&j)
                } else {
                    serde_json::to_string(&j)
                };
                Ok(vec![Value::str(&text.unwrap_or_default())])
            }
            141 => {
                let text = check_text(lua, &args, 0, "decode")?;
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(j) => Ok(vec![json_to_lua(&j)]),
                    Err(e) => Ok(vec![Value::Nil, Value::str(&e.to_string())]),
                }
            }
            44 => {
                self.not_drawing(lua, "world.particle")?;
                let id = check_text(lua, &args, 0, "particle")?;
                let p = [
                    check_num(lua, &args, 1, "particle")?,
                    check_num(lua, &args, 2, "particle")?,
                    check_num(lua, &args, 3, "particle")?,
                ];
                let v = [
                    opt_num(lua, &args, 4, "particle", 0.0)?,
                    opt_num(lua, &args, 5, "particle", 0.0)?,
                    opt_num(lua, &args, 6, "particle", 0.0)?,
                ];
                let count = opt_num(lua, &args, 7, "particle", 1.0)?.clamp(1.0, 64.0) as usize;
                let color = match args.get(8) {
                    None | Some(Value::Nil) => None,
                    Some(_) => {
                        let c = color_arg(lua, &args, 8, 0xFFFF_FFFF)?;
                        let f = |shift: u32| ((c >> shift) & 0xFF) as f32 / 255.0;
                        Some([f(16), f(8), f(0), f(24)])
                    }
                };
                for _ in 0..count {
                    if self.s.particles >= MAX_PARTICLES {
                        return Err(lua.rt_error("too many particles this frame (512)"));
                    }
                    self.s.particles += 1;
                    let mut at = p;
                    if count > 1 {
                        for a in &mut at {
                            *a += (super::hstudio_lualib::random01(lua) - 0.5) * 0.5;
                        }
                    }
                    self.game.particle(&id, at, v, color).map_err(|e| lua.rt_error(e))?;
                }
                none()
            }
            110..=115 => self.draw(lua, id, &args),
            120 | 121 => {
                let name = if id == 120 { "play" } else { "at" };
                // A screen's draw may click (UI libraries draw and react at once).
                if self.phase != Phase::Screen {
                    self.not_drawing(lua, &format!("sound.{name}"))?;
                }
                if self.s.sounds >= MAX_SOUNDS {
                    return Err(lua.rt_error("too many sounds this frame (32)"));
                }
                let sid = check_text(lua, &args, 0, name)?;
                let sid = if sid.contains(':') {
                    sid
                } else {
                    format!("minecraft:{sid}")
                };
                let (at, vi) = if id == 121 {
                    let p = [
                        check_num(lua, &args, 1, name)?,
                        check_num(lua, &args, 2, name)?,
                        check_num(lua, &args, 3, name)?,
                    ];
                    (Some(p), 4)
                } else {
                    (None, 1)
                };
                let volume = opt_num(lua, &args, vi, name, 1.0)?.clamp(0.0, 4.0) as f32;
                let pitch = opt_num(lua, &args, vi + 1, name, 1.0)?.clamp(0.5, 2.0) as f32;
                self.s.sounds += 1;
                self.game.sound(&sid, at, volume, pitch).map_err(|e| lua.rt_error(e))?;
                none()
            }
            130 => {
                let t = check_table(lua, &args, 0, "open")?;
                self.s.screen_request = Some((self.owner, t));
                none()
            }
            131 => {
                let me = self.owner;
                if self.s.screen.as_ref().is_some_and(|sc| sc.owner == me) {
                    self.s.screen = None;
                }
                if self.s.screen_request.as_ref().is_some_and(|r| r.0 == me) {
                    self.s.screen_request = None;
                }
                none()
            }
            132 => {
                let me = self.owner;
                Ok(vec![Value::Bool(
                    self.s.screen.as_ref().is_some_and(|sc| sc.owner == me),
                )])
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
