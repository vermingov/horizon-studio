// Horizon Studio: mods written in Lua inside the game. Script mods live in
// `horizon-mods/.horizon-mod-data/horizon_studio/mods/`: a script is one
// file, `name.lua`, and a module is a `name/` folder whose main.lua loads its
// other files with `require` (hstudio_modfiles has the rules). The studio
// loads them, reloads one as soon as a file of it changes on disk (from the
// in-game editor or any other), and stops only that mod when it fails. The
// console runs a line of Lua at once, in the running game, like a Lua
// executor. This module knows nothing of the game itself: it reaches it
// through `GameApi`, which hstudio_bridge implements over Horizon's `Game`,
// and the tests over a fake.

use super::hstudio_examples::{EXAMPLES, WELCOME};
use super::hstudio_lua::{
    Host, Lua, Native, NoHost, Res, Resume, Table, TableRef, Thread, Throw, Value, arg, bad_arg, check_func, check_num,
    check_str, check_table, check_text, opt_num, opt_str,
};
use super::hstudio_lualib::{inspect, new_env, quote, text_of};
use super::hstudio_modfiles::{TEMPLATE, file_name, lua_files, path_in, prune_empty, rel_in, require_path, to_trash};
use super::hstudio_ui::{Editor, ScriptScreen, Ui};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime};

pub const ID: &str = "horizon_studio";
pub const NAME: &str = "Horizon Studio";
pub const VERSION: &str = "0.5.2";
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
    /// Saves the next frame drawn as a PNG at `path` (as F2 does), for the
    /// agent to look at the game. Where there is no window, an error.
    fn screenshot(&mut self, path: &Path) -> Result<(), String> {
        let _ = path;
        Err("screenshots need the game's window".into())
    }
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
    /// ViaHorizon: whether joins go through ViaProxy, as which version, and
    /// what it is doing.
    fn via(&self) -> ViaInfo {
        ViaInfo::default()
    }
    /// Turns ViaHorizon on or off, or chooses the version joins go as.
    fn via_set(&mut self, enabled: Option<bool>, version: Option<&str>) -> Result<(), String> {
        let _ = (enabled, version);
        Err("ViaHorizon isn't available here".into())
    }
    /// A body part's frame this frame: of the player (`target` None) or of an
    /// entity by id. None where there is no such part, or no world.
    fn part_frame(&self, target: Option<u32>, part: &str) -> Option<PartFrame> {
        let _ = (target, part);
        None
    }
    /// Whether the player's own body is drawn (not in first person).
    fn body_shown(&self) -> bool {
        true
    }
    fn pose(&self) -> Option<PlayerPose> {
        None
    }
    /// An item as a 3D model in the world: `m` is its frame (its x, y and z
    /// axes and its origin, in blocks); `display` how the game shows it
    /// ("head", "hand", "left_hand", "fixed", "ground", "gui"); lit by the
    /// light at `light_at`, or full bright. Only while the world draws.
    fn world_item(
        &mut self,
        id: &str,
        m: [[f64; 3]; 4],
        display: &str,
        light_at: Option<[f64; 3]>,
    ) -> Result<(), String> {
        let _ = (id, m, display, light_at);
        Err("items are drawn only with the world".into())
    }
    /// `world_quad`, shaded by its outward normal and lit by the light at
    /// `light_at` (full bright without one).
    fn world_quad_lit(&mut self, corners: [[f64; 3]; 4], argb: u32, normal: [f64; 3], light_at: Option<[f64; 3]>) {
        let _ = (normal, light_at);
        self.world_quad(corners, argb);
    }
    /// Triangles in the world, textured with `texture` (None: plain colour),
    /// shaded by their normals and lit at `light_at` (full bright without one).
    fn world_mesh(&mut self, tris: &[WorldTri], texture: Option<TexRef<'_>>, light_at: Option<[f64; 3]>) {
        let _ = texture;
        for t in tris {
            self.world_quad_lit([t.p[0], t.p[1], t.p[2], t.p[2]], t.argb, t.normal, light_at);
        }
    }
}

/// A triangle in the world: its corners, their texture coordinates (0 to 1
/// across the texture from its top left), its colour (which tints the
/// texture, 0xAARRGGBB) and its normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTri {
    pub p: [[f64; 3]; 3],
    pub uv: [[f32; 2]; 3],
    pub argb: u32,
    pub normal: [f64; 3],
}

/// A texture to draw with: RGBA pixels, row by row from the top left, its
/// name (`key`) and how many times its pixels changed (`version`).
#[derive(Clone, Copy, Debug)]
pub struct TexRef<'a> {
    pub key: u32,
    pub version: u32,
    pub w: u32,
    pub h: u32,
    pub rgba: &'a [u8],
}

/// ViaHorizon's state, for `via.*`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViaInfo {
    pub enabled: bool,
    /// ViaProxy's name for it ("1.8.x", "auto").
    pub version: String,
    /// What it is doing ("Ready (ViaProxy 3.4.14, Java 21)").
    pub status: String,
    pub ready: bool,
    /// Every version, newest first: (id, label).
    pub versions: Vec<(String, String)>,
    /// ViaProxy's last lines.
    pub log: Vec<String>,
}

/// Horizon's fonts: Geist and Geist Mono.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular,
    Medium,
    Mono,
}

/// Where a body part is this frame: its origin (the part's pivot) and its
/// left, up and forward axes, a block long each (a model pixel is 1/16 of an
/// axis; the player's model is 15/16 the size of a block-sized one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartFrame {
    pub origin: [f64; 3],
    pub left: [f64; 3],
    pub up: [f64; 3],
    pub forward: [f64; 3],
}

/// The player's pose, for `player.pose()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerPose {
    /// "standing", "crouching", "swimming", "gliding", "spin_attack" or "sleeping".
    pub pose: String,
    pub sprinting: bool,
    pub on_ground: bool,
    pub flying: bool,
    /// "first", "third_back" or "third_front".
    pub camera: String,
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
pub fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
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
pub const PREINSTALLED: &[&str] = &["uikit", "viahorizon"];

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

/// A coroutine the task scheduler resumes: one that waits (`task.wait`,
/// `wait`, `WaitForChild`, a signal's `Wait`) or one made to start later
/// (`task.delay`, `task.defer`, a signal's handler).
pub struct Task {
    pub thread: Rc<Thread>,
    pub wake: Wake,
    /// The studio's clock when it began waiting.
    pub since: f64,
    /// The frame it began waiting in: a wait of 0 ends next frame.
    pub frame: u64,
}

pub enum Wake {
    /// At this time: with these arguments, or (None) with the time waited,
    /// and the clock too for the older `wait`.
    At {
        at: f64,
        args: Option<Vec<Value>>,
        legacy: bool,
    },
    /// As soon as the scheduler runs, with these.
    Now(Vec<Value>),
}

/// Threads one mod may have waiting at once.
pub const MAX_TASKS: usize = 4096;

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
    /// Its folder, for a module (`require` reads its other files there).
    pub folder: Option<PathBuf>,
    pub enabled: bool,
    pub state: ModState,
    pub env: Option<TableRef>,
    pub handlers: Handlers,
    pub commands: Vec<Command>,
    pub timers: Vec<Timer>,
    /// Its waiting threads, for the task scheduler.
    pub tasks: Vec<Task>,
    /// Its `script` Instance, and its main file's function.
    pub script: Option<u32>,
    pub main_fn: Option<Value>,
    pub panel: Option<(Corner, Vec<String>)>,
    pub zoom: Option<f32>,
    /// What `require` returned for each of its files, by the file's path in
    /// the module ("lib/util.lua"), however the name was written.
    pub modules: HashMap<String, Value>,
    /// The files whose `require` is under way, outermost first: requiring
    /// one of these again is a loop.
    pub loading: Vec<String>,
    /// The thread its main file's top level runs in (it can wait): a loop
    /// of requires begun there starts at main.lua.
    pub main_thread: Option<std::rc::Weak<Thread>>,
    /// A module's other files as ModuleScripts under `script`: each one's
    /// Instance and the file's path in the module.
    pub module_scripts: Vec<(u32, String)>,
    /// Agent replies to come, and the callbacks for them.
    pub pending: Vec<(Receiver<String>, Value)>,
    /// Callbacks with their results, for the next frame.
    pub ready: Vec<(Value, Vec<Value>)>,
    /// Its files' sizes and times when it was loaded.
    pub stamp: Vec<(PathBuf, Option<SystemTime>, u64)>,
    /// Milliseconds its handlers took, averaged, per kind of event.
    pub cost_ms: f64,
    pub reloads: u32,
    /// Models attached to bodies (`model.attach`).
    pub models: Vec<super::hstudio_models::Attachment>,
    /// Its textures and meshes (`texture.*`, `mesh.*`).
    pub meshes: super::hstudio_meshes::Store,
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
            tasks: Vec::new(),
            script: None,
            main_fn: None,
            panel: None,
            zoom: None,
            modules: HashMap::new(),
            loading: Vec::new(),
            main_thread: None,
            module_scripts: Vec::new(),
            pending: Vec::new(),
            ready: Vec::new(),
            stamp: Vec::new(),
            cost_ms: 0.0,
            reloads: 0,
            models: Vec::new(),
            meshes: super::hstudio_meshes::Store::default(),
        }
    }
    /// Drops everything it made: back to not running.
    fn clear(&mut self) {
        self.env = None;
        self.exports = None;
        self.handlers = Handlers::default();
        self.commands.clear();
        self.timers.clear();
        self.tasks.clear();
        self.script = None;
        self.main_fn = None;
        self.panel = None;
        self.zoom = None;
        self.modules.clear();
        self.loading.clear();
        self.main_thread = None;
        self.module_scripts.clear();
        self.pending.clear();
        self.ready.clear();
        self.models.clear();
        self.meshes.clear();
    }
    pub fn running(&self) -> bool {
        self.state == ModState::Running
    }
    /// The chunk name its main file's errors show.
    pub(crate) fn chunk(&self) -> String {
        match &self.folder {
            Some(_) => format!("{}/main.lua", self.id),
            None => format!("{}.lua", self.id),
        }
    }
    /// The chunk name of a module's file (`rel`, its path in the module), as
    /// `require` loads it: `<id>/lib/util.lua`.
    pub(crate) fn chunk_of(&self, rel: &str) -> String {
        format!("{}/{rel}", self.id)
    }
    /// A module's `.lua` files by their paths in it, main.lua first, as the
    /// last scan found them (nothing is read from disk); empty for a script.
    pub fn files(&self) -> Vec<String> {
        let Some(folder) = &self.folder else {
            return Vec::new();
        };
        self.stamp.iter().filter_map(|(p, ..)| rel_in(folder, p)).collect()
    }
    /// How many files it has: a module's `.lua` files, or a script's one.
    pub fn file_count(&self) -> usize {
        if self.folder.is_some() { self.files().len() } else { 1 }
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
    /// Lines logged since the start: the last line in `log` is number `log_seq`.
    pub log_seq: u64,
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
    /// The last id `model.attach` gave.
    pub next_model: u32,
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
    /// Frames so far, for the task scheduler.
    pub frame_no: u64,
    /// The Roblox layer: one DataModel for every mod.
    pub rbx: super::hstudio_rbx::Rbx,
    /// The executor's globals (`getgenv`) and Roblox's (`getrenv`), shared by every mod.
    pub genv: Option<TableRef>,
    pub renv: Option<TableRef>,
    /// Images decoded for Drawing and ImageLabels.
    pub images: super::hstudio_sunc::ImageCache,
    /// The table of the screen the Roblox GUI takes the pointer with.
    rbx_screen: Option<TableRef>,

    /// Errors of threads that a call resumed (`task.spawn`), for their
    /// owners to fail with once the call is over.
    thread_errors: Vec<(Owner, String, String)>,
    /// 3D quads, particles and sounds this frame (they are capped).
    world_quads: usize,
    particles: usize,
    sounds: usize,
    started: bool,
}

enum Request {
    Reload(Option<String>),
    /// A mod, and a file of it (its path in the module) or its main file.
    Edit(String, Option<String>),
    /// A new mod's name, and whether it is a module.
    New(String, bool),
}

impl State {
    pub fn log(&mut self, kind: LogKind, text: impl Into<String>) {
        let text: String = text.into();
        for line in text.split('\n') {
            if self.log.len() >= MAX_LOG {
                self.log.pop_front();
            }
            self.log_seq += 1;
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
    pub(crate) fn toast(&mut self, text: impl Into<String>) {
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
                log_seq: 0,
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
                next_model: 0,
                queued_keys: Vec::new(),
                queued_commands: Vec::new(),
                queued_chat: Vec::new(),
                queued_events: Vec::new(),
                requests: Vec::new(),
                screen: None,
                screen_request: None,
                overlay: Vec::new(),
                frame_no: 0,
                rbx: super::hstudio_rbx::Rbx::new(),
                genv: None,
                renv: None,
                images: super::hstudio_sunc::ImageCache::default(),
                rbx_screen: None,
                thread_errors: Vec::new(),
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
        self.s.rbx.workspace_dir = self.s.dir.join("workspace");
        self.s.ui.look = super::hstudio_theme::Look::load(&self.s.dir);
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
        let scratch = self.scratch_path();
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

    /// Its files' sizes and times, to see when it changes: a script's file,
    /// or every `.lua` file of a module (`lua_files`, main.lua first).
    fn stamp(main: &Path, folder: Option<&Path>) -> Vec<(PathBuf, Option<SystemTime>, u64)> {
        let paths = match folder {
            None => vec![main.to_path_buf()],
            Some(dir) => lua_files(dir).iter().map(|rel| path_in(dir, rel)).collect(),
        };
        paths
            .into_iter()
            .map(|p| {
                let meta = std::fs::metadata(&p).ok();
                let time = meta.as_ref().and_then(|m| m.modified().ok());
                (p, time, meta.map_or(0, |m| m.len()))
            })
            .collect()
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
                // The Library keeps the mod chosen there, wherever it is listed now.
                let sel = &mut self.s.ui.library_sel;
                if *sel > i {
                    *sel -= 1;
                }
                *sel = (*sel).min(self.s.mods.len().saturating_sub(1));
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
        // The main files that are somewhere else now (a script made a
        // module), from where to where.
        let mut moved = Vec::new();
        for (id, main, folder) in found {
            match self.s.mods.iter().position(|m| m.id == id) {
                Some(i) => {
                    let stamp = Self::stamp(&main, folder.as_deref());
                    if stamp != self.s.mods[i].stamp {
                        if self.s.mods[i].main != main {
                            moved.push((self.s.mods[i].main.clone(), main.clone()));
                        }
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
        self.follow_open(&moved);
    }

    /// After a scan, the editor's file when it moved or went on disk without
    /// the studio (an agent's write_mod made a script a module or removed a
    /// file of a module; a file manager), so that a save never writes where
    /// it was. A mod's main file that moved (`moved`: from where, to where)
    /// takes the editor along, its text and unsaved edits too, and what is
    /// there now is read, or with unsaved edits is a conflict, as for any
    /// file changed on disk. A file of a mod that is gone gives way to its
    /// module's main.lua, or to the scratch file when the whole mod went;
    /// with unsaved edits it stays, marked as removed (a save writes it back).
    fn follow_open(&mut self, moved: &[(PathBuf, PathBuf)]) {
        let Some(path) = self.s.editor.path.clone() else {
            return;
        };
        if let Some((_, to)) = moved.iter().find(|(from, _)| *from == path) {
            // With the stamp it had: what is at `to` may not be what it read.
            let disk = self.s.editor.disk;
            let title = self.file_title(to);
            self.s.editor.moved(to, &title);
            self.s.editor.disk = disk;
            if self.s.editor.sync_disk() {
                self.s.editor.reloaded_at = Some(self.s.now);
            }
            self.mark_failure();
            self.unfold_open();
            return;
        }
        // Gone: a file it read or wrote (not one it never found), of the mods folder.
        if self.s.editor.disk.is_none() || path.exists() || !path.starts_with(self.s.mods_dir()) {
            return;
        }
        if self.s.editor.dirty {
            self.s.editor.removed = true;
            self.s.editor.disk = None;
        } else {
            let instead = self.instead_of(&path);
            self.open_file(&instead);
        }
    }

    /// What the editor shows in place of a file that is gone: the main.lua
    /// of the module it was in, or the scratch file.
    fn instead_of(&self, path: &Path) -> PathBuf {
        match self.mod_of_path(path) {
            Some(i) => self.s.mods[i].main.clone(),
            None => self.scratch_path(),
        }
    }

    /// The file the editor still shows, with its unsaved edits, after it was
    /// removed from disk, let go: its module's main.lua opens, or the
    /// scratch file.
    pub fn close_removed(&mut self) {
        if let Some(path) = self.s.editor.path.clone() {
            let instead = self.instead_of(&path);
            self.open_file(&instead);
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
        self.s.rbx.forget(Owner::Mod(i));
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
        // `script`: a LocalScript named after the mod, in PlayerScripts. A
        // module's other files are ModuleScripts under it, named after
        // them; a folder in a file's path is the child of that name (the
        // ModuleScript of a file of the same name, as Rojo nests lib.lua and
        // lib/util.lua), else a Folder.
        {
            let owner = Some(Owner::Mod(i));
            let files = self.s.mods[i].files();
            let rbx = &mut self.s.rbx;
            let sc = rbx.make("LocalScript", owner);
            rbx.inst_mut(sc).name = self.s.mods[i].id.clone();
            let ps = rbx.child_named(rbx.local_player, "PlayerScripts");
            let _ = rbx.set_parent(sc, ps);
            env.borrow_mut().set_str("script", rbx.value(sc));
            let mut made = Vec::new();
            for rel in files.into_iter().filter(|f| f != "main.lua") {
                let parts: Vec<&str> = rel.split('/').collect();
                let mut parent = sc;
                for dir in &parts[..parts.len() - 1] {
                    parent = match rbx.child_named(parent, dir) {
                        Some(c) => c,
                        None => {
                            let f = rbx.make("Folder", owner);
                            rbx.inst_mut(f).name = (*dir).to_owned();
                            let _ = rbx.set_parent(f, Some(parent));
                            f
                        }
                    };
                }
                let file = parts[parts.len() - 1];
                let ms = rbx.make("ModuleScript", owner);
                rbx.inst_mut(ms).name = file.strip_suffix(".lua").unwrap_or(file).to_owned();
                let _ = rbx.set_parent(ms, Some(parent));
                made.push((ms, rel));
            }
            self.s.mods[i].script = Some(sc);
            self.s.mods[i].module_scripts = made;
        }
        // getgc and filtergc find what was made after tracking starts.
        if [&b"getgc"[..], b"filtergc", b"getreg", b"getproto"]
            .iter()
            .any(|w| src.windows(w.len()).any(|x| x == *w))
        {
            self.lua.track_objects(true);
        }
        let f = match self.lua.load_bytes(&src, &chunk, &env) {
            Ok(f) => f,
            Err(e) => {
                self.fail(game, Owner::Mod(i), e, String::new());
                return;
            }
        };
        self.s.mods[i].state = ModState::Running;
        self.s.mods[i].main_fn = Some(f.clone());
        // In a coroutine, as Roblox runs a script: its top level can wait.
        let co = self.lua.new_thread(f);
        self.s.mods[i].main_thread = Some(Rc::downgrade(&co));
        match self.resume_in(game, Owner::Mod(i), &co, Vec::new(), LOAD_STEPS, Phase::Normal) {
            Some(Ok(vals)) => {
                // A main file that returns a table offers it, as a module does.
                if self.s.mods[i].exports.is_none()
                    && let Some(v @ Value::Table(_)) = vals.into_iter().next()
                {
                    self.s.mods[i].exports = Some(v);
                }
            }
            Some(Err(e)) => self.fail(game, Owner::Mod(i), e.message(), e.traceback().to_owned()),
            None => {}
        }
        self.pump(game);
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
                self.s.rbx.forget(owner);
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
    /// A new script mod called `name`: the template, or `code` when given (Save File).
    pub fn create_mod(&mut self, game: &mut dyn GameApi, name: &str, code: Option<&str>) -> Result<PathBuf, String> {
        let id = mod_id(name.trim());
        let path = self.s.mods_dir().join(format!("{id}.lua"));
        if path.exists() || self.s.mods_dir().join(&id).exists() {
            return Err(format!("{id} already exists"));
        }
        let _ = std::fs::create_dir_all(self.s.mods_dir());
        let src = match code {
            Some(code) if code.ends_with('\n') => code.to_owned(),
            Some(code) => format!("{code}\n"),
            None => TEMPLATE.replace("{id}", &id).replace("{name}", name.trim()),
        };
        std::fs::write(&path, src).map_err(|e| e.to_string())?;
        self.scan(game);
        self.s.log(LogKind::Info, format!("Created {}", path.display()));
        Ok(path)
    }

    /// A new module called `name`: `<mods>/<id>/` with main.lua, shared.lua
    /// and lib.lua, unless that id is taken; the scan loads it. Returns its
    /// main.lua.
    pub fn create_module(&mut self, game: &mut dyn GameApi, name: &str) -> Result<PathBuf, String> {
        let name = name.trim();
        let id = mod_id(name);
        let mods = self.s.mods_dir();
        std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
        let main = super::hstudio_modfiles::write_module(&mods, &id, name)?;
        self.scan(game);
        self.s
            .log(LogKind::Info, format!("Created {}", mods.join(&id).display()));
        Ok(main)
    }

    /// The trash: what the studio deletes goes there, as
    /// `<studio>/trash/<its path in mods/>`, the newest copy of each.
    pub fn trash_dir(&self) -> PathBuf {
        self.s.dir.join("trash")
    }

    /// A file of a module, as the module's folder and the file's path in it.
    fn module_file(&self, path: &Path) -> Option<(PathBuf, String)> {
        let folder = self.s.mods[self.mod_of_path(path)?].folder.clone()?;
        let rel = rel_in(&folder, path)?;
        Some((folder, rel))
    }

    /// A new file in the module whose folder is `folder`, named as typed
    /// ("util", "lib/util"), from a short template; the module reloads with
    /// it. Returns its path.
    pub fn add_file(&mut self, game: &mut dyn GameApi, folder: &Path, typed: &str) -> Result<PathBuf, String> {
        let i = self
            .mod_of_path(folder)
            .filter(|&i| self.s.mods[i].folder.as_deref() == Some(folder))
            .ok_or("that is not a module's folder")?;
        let rel = file_name(typed)?;
        let name = self.s.mods[i].name.clone();
        let path = super::hstudio_modfiles::write_file(folder, &rel, &name)?;
        self.s.log(LogKind::Info, format!("Created {}", path.display()));
        self.scan(game);
        Ok(path)
    }

    /// Renames a file of a module to what was typed; the module reloads, and
    /// the editor follows the file if it has it open (its unsaved edits
    /// too). Other files' `require` of the old name stay as they are.
    /// Returns its new path.
    pub fn rename_file(&mut self, game: &mut dyn GameApi, path: &Path, typed: &str) -> Result<PathBuf, String> {
        let (folder, from) = self.module_file(path).ok_or("that is not a file of a module")?;
        let to = file_name(typed)?;
        if to == from {
            return Ok(path.to_path_buf());
        }
        let new = super::hstudio_modfiles::rename_file(&folder, &from, &to)?;
        self.s.log(LogKind::Info, format!("Renamed {from} to {to}"));
        self.file_moved(path, Some(&new), &new);
        self.scan(game);
        self.settle_open();
        Ok(new)
    }

    /// Moves a mod's file to the trash: one file of a module (not its
    /// main.lua, which starts it: the module goes instead), or a whole mod
    /// (a script's file, or a module's folder). Edits the editor has not
    /// saved go with it (when they can't be saved, nothing is deleted). The
    /// editor then shows the module's main.lua, or the scratch file when the
    /// whole mod went. Returns where it went.
    pub fn delete_path(&mut self, game: &mut dyn GameApi, path: &Path) -> Result<PathBuf, String> {
        let i = self.mod_of_path(path).ok_or("that is not a file of a mod")?;
        let m = &self.s.mods[i];
        let (target, folder) = match &m.folder {
            None => (m.main.clone(), None),
            Some(f) if f == path => (f.clone(), None),
            Some(f) if rel_in(f, path).as_deref() == Some("main.lua") => {
                return Err("main.lua starts the module: delete the module instead".into());
            }
            Some(f) => (path.to_path_buf(), Some(f.clone())),
        };
        let instead = match &folder {
            Some(f) => f.join("main.lua"),
            None => self.scratch_path(),
        };
        let ed = &mut self.s.editor;
        if ed.dirty && ed.path.as_deref().is_some_and(|p| p.starts_with(&target)) {
            ed.save()
                .map_err(|e| format!("could not save {}: {e}; nothing was deleted", ed.title))?;
        }
        let moved = to_trash(&self.trash_dir(), &self.s.mods_dir(), &target)?;
        if let Some(f) = &folder {
            prune_empty(f, target.parent());
        }
        self.s.log(
            LogKind::Info,
            format!("Moved {} to {}", target.display(), moved.display()),
        );
        self.file_moved(&target, None, &instead);
        self.scan(game);
        self.settle_open();
        Ok(moved)
    }

    /// Turns script `i` into a module: its file becomes `<id>/main.lua`, and
    /// it keeps its id, storage and on/off state. It reloads, and the editor
    /// follows it if it has it open. Returns main.lua's path.
    pub fn make_module(&mut self, game: &mut dyn GameApi, i: usize) -> Result<PathBuf, String> {
        let m = self.s.mods.get(i).ok_or("no such mod")?;
        if m.folder.is_some() {
            return Err(format!("{} is a module already", m.id));
        }
        let (old, id) = (m.main.clone(), m.id.clone());
        let main = super::hstudio_modfiles::make_module(&self.s.mods_dir(), &old, &id)?;
        self.s
            .log(LogKind::Info, format!("Made {id} a module: {}", main.display()));
        self.file_moved(&old, Some(&main), &main);
        self.scan(game);
        self.settle_open();
        Ok(main)
    }

    /// After the studio moved a file (`to`) or deleted it (None), before the
    /// scan: when the editor shows `from`, or a file inside it, it follows
    /// the file to its new place with its text, cursor and unsaved edits, or
    /// opens `instead`, so the scan finds it where it is and doesn't take it
    /// for a file removed. `settle_open` then names it as the scan's mods do.
    fn file_moved(&mut self, from: &Path, to: Option<&Path>, instead: &Path) {
        let Some(open) = self.s.editor.path.clone().filter(|p| p.starts_with(from)) else {
            return;
        };
        match to {
            Some(to) => {
                let new = match open.strip_prefix(from) {
                    Ok(rest) if !rest.as_os_str().is_empty() => to.join(rest),
                    _ => to.to_path_buf(),
                };
                let title = self.file_title(&new);
                self.s.editor.moved(&new, &title);
            }
            None => self.open_file(instead),
        }
    }

    /// After a scan that changed the mod of the editor's file: its title (a
    /// script made a module is `<id>/main.lua` now), the line the mod failed
    /// at, and its module unfolded in the script list. A file of no mod (the
    /// scratch file, its console error marked) is left as it is.
    fn settle_open(&mut self) {
        let Some(path) = self.s.editor.path.clone() else {
            return;
        };
        if self.mod_of_path(&path).is_none() {
            return;
        }
        let title = self.file_title(&path);
        if title != self.s.editor.title {
            self.s.editor.title = title;
            // Its name is in what the live check says: it looks again.
            self.s.editor.checked = None;
        }
        self.mark_failure();
        self.unfold_open();
    }

    /// Before another file opens in the editor (`to`), the one open is saved
    /// if it has unsaved edits, so switching files never loses them.
    pub fn save_before_switch(&mut self, game: &mut dyn GameApi, to: Option<&Path>) {
        let ed = &self.s.editor;
        if ed.dirty && ed.path.is_some() && ed.path.as_deref() != to {
            self.save_editor(game);
        }
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
    /// Globals for a mod or the console: its own table, falling back to the
    /// executor's (`getgenv`: the studio's API and the executor library),
    /// which falls back to Roblox's (`getrenv`: the standard library, game,
    /// Instance, the data types, task).
    fn make_env(&mut self, console: bool) -> TableRef {
        let genv = self.shared_envs();
        let env = Rc::new(std::cell::RefCell::new(Table::new()));
        {
            let mut e = env.borrow_mut();
            e.set_str("_G", Value::Table(env.clone()));
            if console {
                for (name, id, _) in CONSOLE_API {
                    e.set_str(name, Native::host(name, *id));
                }
            }
            let mut meta = Table::new();
            meta.set_str("__index", Value::Table(genv));
            e.meta = Some(Rc::new(std::cell::RefCell::new(meta)));
        }
        env
    }

    /// The executor's and Roblox's globals, made the first time.
    fn shared_envs(&mut self) -> TableRef {
        if let Some(g) = &self.s.genv {
            return g.clone();
        }
        let renv = new_env(&mut self.lua);
        let genv = Rc::new(std::cell::RefCell::new(Table::new()));
        let mut tables: HashMap<&str, Table> = HashMap::new();
        // Roblox's own globals go in its environment; the rest are the studio's.
        let roblox = |table: &str, name: &str| {
            matches!(table, "task" | "game")
                || (table.is_empty() && matches!(name, "wait" | "spawn" | "delay" | "tick" | "time" | "warn"))
        };
        for (table, name, id, _) in API {
            let f = Native::host(name, *id);
            match *table {
                "" if roblox(table, name) => renv.borrow_mut().set_str(name, f),
                "" => {
                    super::hstudio_exec::mark(&f);
                    genv.borrow_mut().set_str(name, f)
                }
                t => tables.entry(t).or_default().set_str(name, f),
            }
        }
        for (name, t) in tables {
            let dest = if roblox(name, "") { &renv } else { &genv };
            dest.borrow_mut().set_str(name, Value::table(t));
        }
        self.roblox_globals(&renv);
        super::hstudio_sunc::executor_globals(&mut self.lua, &genv, &mut self.s.rbx);
        let mut meta = Table::new();
        meta.set_str("__index", Value::Table(renv.clone()));
        genv.borrow_mut().meta = Some(Rc::new(std::cell::RefCell::new(meta)));
        self.s.renv = Some(renv);
        self.s.genv = Some(genv.clone());
        genv
    }

    /// The Roblox layer's globals: its data types and Enum, `Instance`,
    /// and the shared `game` (still with the studio's `game.*`),
    /// `workspace` and `shared`.
    fn roblox_globals(&mut self, env: &TableRef) {
        let mut g = env.borrow_mut();
        for (name, v) in super::hstudio_rbx_values::globals() {
            g.set_str(name, v);
        }
        let rbx = &mut self.s.rbx;
        g.set_str("Instance", super::hstudio_rbx::instance_table(rbx));
        let game = match g.get_str("game") {
            Value::Table(t) => t,
            _ => Rc::new(std::cell::RefCell::new(Table::new())),
        };
        rbx.adopt_game_table(&game);
        g.set_str("game", Value::Table(game));
        let ws = rbx.value(rbx.workspace);
        g.set_str("workspace", ws.clone());
        g.set_str("Workspace", ws);
        g.set_str("shared", Value::table(Table::new()));
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
        self.thread_failures(game);
        r
    }

    /// Resumes `co` for `owner`: what it returned, its error, or None while
    /// it waits (or was left suspended).
    fn resume_in(
        &mut self,
        game: &mut dyn GameApi,
        owner: Owner,
        co: &Rc<Thread>,
        args: Vec<Value>,
        budget: u64,
        phase: Phase,
    ) -> Option<Outcome> {
        if !co.is_suspended() {
            return None;
        }
        self.lua.steps = budget;
        let started = Instant::now();
        let mut ctx = Ctx {
            game,
            s: &mut self.s,
            owner,
            phase,
        };
        let r = self.lua.resume(&mut ctx, co, args);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let m = self.s.owner_mod(owner);
        m.cost_ms = m.cost_ms * 0.95 + ms * 0.05;
        self.thread_failures(game);
        match r {
            Resume::Done(vals) => Some(Ok(vals)),
            Resume::Yielded(_) => None,
            Resume::Error(e) => Some(Err(e)),
        }
    }

    /// Stops the owners of threads that failed inside a call.
    fn thread_failures(&mut self, game: &mut dyn GameApi) {
        for (owner, msg, trace) in std::mem::take(&mut self.s.thread_errors) {
            if self.s.owner_mod(owner).running() {
                self.fail(game, owner, msg, trace);
            }
        }
    }

    /// Runs the threads due now: each waiting one whose time came (`timed`)
    /// and those ready at once (deferred, signal handlers), again until
    /// none are left (at most 10 rounds, as Roblox's deferred events).
    fn run_tasks(&mut self, game: &mut dyn GameApi, timed: bool) {
        for round in 0..10 {
            let (now, frame) = (self.s.now, self.s.frame_no);
            let mut due: Vec<(Owner, Rc<Thread>, Vec<Value>)> = Vec::new();
            let mut owners: Vec<Owner> = (0..self.s.mods.len()).map(Owner::Mod).collect();
            owners.push(Owner::Console);
            for owner in owners {
                let m = self.s.owner_mod(owner);
                if !m.running() || m.tasks.is_empty() {
                    continue;
                }
                let mut i = 0;
                while i < m.tasks.len() {
                    let t = &m.tasks[i];
                    let ready = match &t.wake {
                        Wake::Now(_) => true,
                        Wake::At { at, .. } => timed && round == 0 && *at <= now && t.frame < frame,
                    };
                    if !ready {
                        i += 1;
                        continue;
                    }
                    let t = m.tasks.remove(i);
                    let args = match t.wake {
                        Wake::Now(args) | Wake::At { args: Some(args), .. } => args,
                        Wake::At { legacy, .. } => {
                            let waited = Value::Num(now - t.since);
                            if legacy {
                                vec![waited, Value::Num(now)]
                            } else {
                                vec![waited]
                            }
                        }
                    };
                    due.push((owner, t.thread, args));
                }
            }
            // The Roblox layer's: threads its signals and children woke, and
            // its signal handlers, each in a new thread.
            for (owner, co, args) in std::mem::take(&mut self.s.rbx.ready) {
                due.push((owner, co, args));
            }
            for (owner, f, args) in std::mem::take(&mut self.s.rbx.deferred) {
                if self.s.owner_mod(owner).running() {
                    let co = self.lua.new_thread(f);
                    due.push((owner, co, args));
                }
            }
            if due.is_empty() {
                break;
            }
            for (owner, co, args) in due {
                if !self.s.owner_mod(owner).running() {
                    continue;
                }
                if let Some(Err(e)) = self.resume_in(game, owner, &co, args, EVENT_STEPS, Phase::Normal) {
                    self.fail(game, owner, e.message(), e.traceback().to_owned());
                }
            }
        }
    }

    /// The threads that are ready at once, after something that may have
    /// made some (a load, a console line, input).
    fn pump(&mut self, game: &mut dyn GameApi) {
        self.run_tasks(game, false);
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
        super::hstudio_ui::agent_poll(self);
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
            let processed = self.s.ui.open || game.screen() != "playing";
            super::hstudio_rbx_draw::game_input(&mut self.s.rbx, &key, down, processed);
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
        self.roblox_frame(game, dt);
        self.run_tasks(game, true);
        self.run_replies(game);
        self.dispatch(game, "frame", &[Value::Num(dt)], Phase::Normal);
        self.pump(game);
        self.s.frame_no += 1;
        if self.s.toast.as_ref().is_some_and(|t| t.1 < self.s.now) {
            self.s.toast = None;
        }
    }

    /// The Roblox layer's frame: the DataModel follows the game, then what
    /// it asked of the game (sounds, chat, agent moves, warnings), and
    /// whether its GUI takes the pointer.
    fn roblox_frame(&mut self, game: &mut dyn GameApi, dt: f64) {
        let (now, frame) = (self.s.now, self.s.frame_no);
        super::hstudio_rbx_draw::step(&mut self.s.rbx, game, now, dt, frame);
        self.roblox_effects(game);
        let ours = self.rbx_screen_open();
        let pending = match (&self.s.screen_request, &self.s.rbx_screen) {
            (Some((_, r)), Some(t)) => Rc::ptr_eq(r, t),
            _ => false,
        };
        if !ours && !pending && self.s.rbx.pointer.free {
            // Its screen went (closed by the studio): the pointer is the game's.
            self.s.rbx.pointer.free = false;
        }
        let wants = super::hstudio_rbx_draw::wants_pointer(&self.s.rbx);
        if !wants {
            self.s.rbx.pointer.dismissed = false;
            if ours || pending {
                self.s.screen = None;
                self.s.screen_request = None;
                self.s.rbx.pointer.free = false;
            }
        } else if !ours
            && !pending
            && !self.s.rbx.pointer.dismissed
            && self.s.screen.is_none()
            && self.s.screen_request.is_none()
            && !self.s.ui.open
            && game.player().is_some()
        {
            let t = match &self.s.rbx_screen {
                Some(t) => t.clone(),
                None => {
                    let t = super::hstudio_rbx::screen_table(&mut self.s.rbx);
                    self.s.rbx_screen = Some(t.clone());
                    t
                }
            };
            self.s.screen_request = Some((Owner::Console, t));
            self.s.rbx.pointer.free = true;
        }
    }

    /// Whether the script screen up is the Roblox GUI's.
    fn rbx_screen_open(&self) -> bool {
        match (&self.s.screen, &self.s.rbx_screen) {
            (Some(sc), Some(t)) => Rc::ptr_eq(&sc.table, t),
            _ => false,
        }
    }

    /// Sounds, chat, agent moves and warnings the Roblox layer asked for.
    fn roblox_effects(&mut self, game: &mut dyn GameApi) {
        let rbx = &mut self.s.rbx;
        for (id, at, volume, pitch) in std::mem::take(&mut rbx.sounds) {
            if self.s.sounds < MAX_SOUNDS {
                self.s.sounds += 1;
                let _ = game.sound(&id, at, volume, pitch);
            }
        }
        for line in std::mem::take(&mut rbx.sends) {
            if self.s.send_tokens >= 1.0 {
                self.s.send_tokens -= 1.0;
                game.send(&line);
            }
        }
        for cmd in std::mem::take(&mut rbx.agent) {
            let _ = game.agent(&cmd);
        }
        for w in std::mem::take(&mut rbx.warnings) {
            match w.strip_prefix('\u{1}') {
                Some(note) => game.chat(note),
                None => self.s.log(LogKind::Error, format!("warning: {w}")),
            }
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
        self.draw_models(game);
        let mut budget = MAX_WORLD_QUADS.saturating_sub(self.s.world_quads);
        super::hstudio_rbx_draw::draw_parts(&self.s.rbx, game, &mut budget);
        self.s.world_quads = MAX_WORLD_QUADS - budget;
    }

    /// Every running mod's attachments, at their parts this frame.
    fn draw_models(&mut self, game: &mut dyn GameApi) {
        let shown = game.body_shown();
        let mut budget = MAX_WORLD_QUADS.saturating_sub(self.s.world_quads);
        let mods = self.s.mods.iter().chain(std::iter::once(&self.s.console_mod));
        for m in mods.filter(|m| m.running()) {
            for a in m.models.iter().filter(|a| a.visible) {
                if a.target.is_none() && !shown && !a.first_person {
                    continue;
                }
                if let Some(f) = game.part_frame(a.target, &a.part) {
                    super::hstudio_models::draw(game, &f, a, &m.meshes, &mut budget);
                }
            }
        }
        self.s.world_quads = MAX_WORLD_QUADS - budget;
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
        // Roblox ScreenGuis, unless their own screen draws them.
        if !self.rbx_screen_open() && self.s.rbx.active {
            let now = self.s.now;
            let ws = self.s.dir.join("workspace");
            super::hstudio_rbx_draw::lay_out(&mut self.s.rbx, game);
            super::hstudio_rbx_draw::draw(&self.s.rbx, game, now, &mut self.s.images, &ws);
        }
        super::hstudio_sunc::draw_drawings(&self.s.rbx, game, &mut self.s.images);
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
                // In a thread, so a command can wait (its answer is then lost).
                let co = self.lua.new_thread(f);
                match self.resume_in(game, owner, &co, args, EVENT_STEPS, Phase::Normal) {
                    Some(Ok(r)) => {
                        if let Some(v) = r.first()
                            && !v.is_nil()
                        {
                            game.chat(&v.plain_text());
                        }
                    }
                    Some(Err(e)) => {
                        let msg = e.message();
                        game.chat(&format!("/{name}: {msg}"));
                        self.s.log(LogKind::Error, format!("/{name}: {msg}"));
                        if !e.traceback().is_empty() {
                            self.s.log(LogKind::Trace, e.traceback().to_owned());
                        }
                    }
                    None => {}
                }
                self.pump(game);
            }
        }
    }

    /// Runs a line (or a chunk) of Lua in the console: an expression's values
    /// are printed. Returns the lines it logged.
    pub fn run_console(&mut self, game: &mut dyn GameApi, src: &str) -> Vec<(LogKind, String)> {
        self.run_chunk(game, src, true)
    }

    /// `run_console`, kept out of the console's history with `history` false
    /// (code an AI runs through the link).
    pub fn run_chunk(&mut self, game: &mut dyn GameApi, src: &str, history: bool) -> Vec<(LogKind, String)> {
        self.start(game);
        let mark = self.s.log_seq;
        let src = src.trim();
        if src.is_empty() {
            return Vec::new();
        }
        if history && self.s.history.last().is_none_or(|l| l != src) {
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
            Ok(f) => {
                let co = self.lua.new_thread(f);
                match self.resume_in(game, Owner::Console, &co, Vec::new(), CONSOLE_STEPS, Phase::Normal) {
                    Some(Ok(vals)) => {
                        if !vals.is_empty() {
                            let text = vals.iter().map(|v| inspect(v, 2)).collect::<Vec<_>>().join(", ");
                            self.s.log(LogKind::Output, text);
                        }
                    }
                    Some(Err(e)) => self.fail(game, Owner::Console, e.message(), e.traceback().to_owned()),
                    None => {}
                }
                self.pump(game);
            }
        }
        self.do_requests(game);
        self.log_since(mark)
            .filter(|(_, l)| l.kind != LogKind::Input)
            .map(|(_, l)| (l.kind, l.text.clone()))
            .collect()
    }

    /// The log lines after line number `seq`, with their numbers (those
    /// still kept: the log holds the last 1000).
    pub fn log_since(&self, seq: u64) -> impl Iterator<Item = (u64, &LogLine)> {
        let first = self.s.log_seq - self.s.log.len() as u64 + 1;
        let skip = seq.saturating_sub(first - 1).min(self.s.log.len() as u64) as usize;
        self.s
            .log
            .iter()
            .enumerate()
            .skip(skip)
            .map(move |(i, l)| (first + i as u64, l))
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
                Request::Edit(id, file) => {
                    if let Some(i) = self.s.mods.iter().position(|m| m.id == id) {
                        let m = &self.s.mods[i];
                        let path = match (&file, &m.folder) {
                            (Some(rel), Some(folder)) => path_in(folder, rel),
                            _ => m.main.clone(),
                        };
                        self.save_before_switch(game, Some(&path));
                        self.open_file(&path);
                        self.s.ui.tab = super::hstudio_ui::Tab::Editor;
                    }
                }
                Request::New(name, module) => {
                    self.save_before_switch(game, None);
                    let made = if module {
                        self.create_module(game, &name)
                    } else {
                        self.create_mod(game, &name, None)
                    };
                    match made {
                        Ok(path) => {
                            self.open_file(&path);
                            self.s.ui.tab = super::hstudio_ui::Tab::Editor;
                        }
                        Err(e) => self.s.log(LogKind::Error, e),
                    }
                }
            }
        }
    }

    /// Saves the editor's file if it has unsaved edits (the scan then reloads
    /// its mod), and reloads the mod, turning it on, when that didn't: what
    /// Run does on a module's file, and the executor styles' Execute File.
    /// False when the file is no mod's.
    pub fn reload_editor_mod(&mut self, game: &mut dyn GameApi) -> bool {
        let Some(i) = self.s.editor.path.as_deref().and_then(|p| self.mod_of_path(p)) else {
            return false;
        };
        let (id, reloads) = (self.s.mods[i].id.clone(), self.s.mods[i].reloads);
        if self.s.editor.dirty {
            self.save_editor(game);
        }
        if let Some(i) = self.s.mods.iter().position(|m| m.id == id)
            && self.s.mods[i].reloads == reloads
        {
            self.set_enabled(game, i, true);
            self.mark_failure();
        }
        true
    }

    /// Runs the editor's text as a console chunk (Ctrl+Enter). A module's
    /// file can't run there (its `require` needs the module), so it is saved
    /// and the module reloaded instead.
    pub fn run_editor(&mut self, game: &mut dyn GameApi) {
        let module = self
            .s
            .editor
            .path
            .as_deref()
            .and_then(|p| self.mod_of_path(p))
            .filter(|&i| self.s.mods[i].folder.is_some());
        if let Some(i) = module {
            let id = self.s.mods[i].id.clone();
            self.s.log(LogKind::Input, format!("> run {}", self.s.editor.title));
            self.reload_editor_mod(game);
            if self.s.mods.iter().any(|m| m.id == id && m.running()) {
                self.s.log(LogKind::Info, format!("Ran {id} (reloaded)"));
            }
            return;
        }
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

    /// Saves the editor's file; the scan then reloads its mod. A syntax
    /// error marks its line at once; otherwise a failure of the mod marks
    /// the line of this file it failed at (for a module's file, where it
    /// failed or the line that required the file that did).
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
                if self.s.editor.error_line.is_none() {
                    self.mark_failure();
                }
            }
            Err(e) => self.s.log(LogKind::Error, format!("Could not save: {e}")),
        }
    }

    /// The editor's scratch file, a place to try code (not a mod).
    pub fn scratch_path(&self) -> PathBuf {
        self.s.dir.join("scratch.lua")
    }

    /// Opens mod `i`'s main file in the editor.
    pub fn edit_mod(&mut self, i: usize) {
        if let Some(m) = self.s.mods.get(i) {
            let path = m.main.clone();
            self.open_file(&path);
        }
    }

    /// The mod a file belongs to: a script's own file, or any file in a
    /// module's folder.
    pub fn mod_of_path(&self, path: &Path) -> Option<usize> {
        self.s
            .mods
            .iter()
            .position(|m| m.main == path || m.folder.as_deref().is_some_and(|f| path.starts_with(f)))
    }

    /// The editor's title for a file: the name its errors give it
    /// (`<id>/lib/util.lua` for a module's file, `<id>.lua` for a script),
    /// else its file name.
    pub fn file_title(&self, path: &Path) -> String {
        if let Some(m) = self.mod_of_path(path).map(|i| &self.s.mods[i]) {
            return match m.folder.as_deref().and_then(|f| rel_in(f, path)) {
                Some(rel) => m.chunk_of(&rel),
                None => m.chunk(),
            };
        }
        path.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    /// Opens a file in the editor, titled as its errors name it, with the
    /// line its mod failed at marked when that is in this file. A module's
    /// file unfolds the module in the script list.
    pub fn open_file(&mut self, path: &Path) {
        let title = self.file_title(path);
        self.s.editor.open(path, &title);
        self.mark_failure();
        self.unfold_open();
    }

    /// Unfolds, in the script list, the module whose file the editor shows.
    fn unfold_open(&mut self) {
        if let Some(i) = self.s.editor.path.as_deref().and_then(|p| self.mod_of_path(p))
            && self.s.mods[i].folder.is_some()
        {
            let id = self.s.mods[i].id.clone();
            self.s.ui.expanded.insert(id);
        }
    }

    /// Where mod `i` failed: one of its files (a module's by its path in it,
    /// a script's by its file name) and the line, from the innermost place
    /// in its error that is one of its files, else the first such line of
    /// the traceback. None while it runs, or when its error names none of
    /// its files.
    pub fn failure_place(&self, i: usize) -> Option<(String, usize)> {
        let m = self.s.mods.get(i)?;
        let ModState::Failed(msg, trace) = &m.state else {
            return None;
        };
        let prefix = format!("{}/", m.id);
        let file_of = |chunk: &str| -> Option<String> {
            match m.folder {
                Some(_) => chunk.strip_prefix(prefix.as_str()).map(str::to_owned),
                None => (chunk == m.chunk()).then(|| chunk.to_owned()),
            }
        };
        error_places(msg)
            .into_iter()
            .rev()
            .find_map(|(c, line)| Some((file_of(&c)?, line)))
            .or_else(|| {
                trace.lines().find_map(|l| {
                    let (c, line) = trace_place(l)?;
                    Some((file_of(&c)?, line))
                })
            })
    }

    /// Marks the editor's error line from the failure of the mod its file
    /// belongs to: the line of this file it failed at, if any.
    fn mark_failure(&mut self) {
        let ed = &self.s.editor;
        let failed = ed
            .path
            .as_deref()
            .and_then(|p| self.mod_of_path(p))
            .and_then(|i| match &self.s.mods[i].state {
                ModState::Failed(msg, trace) => error_line_in(msg, trace, &ed.title),
                _ => None,
            });
        self.s.editor.error_line = failed;
    }
}

/// The editor's file on a first run: a place to try things.
const SCRATCH: &str = "-- scratch.lua: a place to try code. Ctrl+Enter runs this file in the console;\n-- Ctrl+S saves it. Your mods go in the Library tab.\n\nlocal x, y, z = player.pos()\nif x then\n  print(string.format(\"You are at %.1f %.1f %.1f, facing %s\", x, y, z, player.facing()))\nend\n";

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
        Value::Thread(_) | Value::Userdata(_) => return Err(format!("a {} has no JSON form", v.type_name())),
    })
}

/// The line number in `chunk:12: message`.
pub fn error_line(msg: &str) -> Option<usize> {
    let mut parts = msg.splitn(3, ':');
    let _chunk = parts.next()?;
    if let Some(line) = parts.next().and_then(|p| p.trim().parse().ok()) {
        return Some(line);
    }
    // After an event's name ("tick: mod.lua:12: ..."): the first file's line.
    let at = msg.find(".lua:")? + 5;
    let digits: String = msg[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Every `<file>.lua:<line>:` place in an error message, outermost first.
/// A file `require` loaded that failed to compile reads
/// "m/main.lua:2: m/lib.lua:3: ...": the require at main.lua's line 2,
/// then the error at lib.lua's line 3. A place's file runs from the space
/// before it (or the start).
pub fn error_places(msg: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = msg[from..].find(".lua:") {
        let dot = from + found;
        let colon = dot + 4;
        let start = msg[..dot]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let digits: String = msg[colon + 1..].chars().take_while(char::is_ascii_digit).collect();
        if start < dot
            && !digits.is_empty()
            && msg[colon + 1 + digits.len()..].starts_with(':')
            && let Ok(line) = digits.parse()
        {
            out.push((msg[start..colon].to_owned(), line));
        }
        from = colon + 1;
    }
    out
}

/// The line of a failure in one file (`chunk`, as the editor titles it):
/// the innermost place in the message that is in that file, else the
/// first line of the traceback (innermost first) that is. With a module's
/// main.lua open after lib.lua failed, that is the line that required lib.
pub fn error_line_in(msg: &str, trace: &str, chunk: &str) -> Option<usize> {
    if let Some((_, line)) = error_places(msg).into_iter().rev().find(|(c, _)| c == chunk) {
        return Some(line);
    }
    trace
        .lines()
        .filter_map(trace_place)
        .find_map(|(c, line)| (c == chunk).then_some(line))
}

/// Where an error happened: the innermost `<file>.lua:<line>:` of its
/// message (the file a `require` loaded, when that is where it failed),
/// else the place it starts with (`chunk:line:`).
pub fn error_place(msg: &str) -> Option<(String, usize)> {
    error_places(msg).pop().or_else(|| trace_place(msg))
}

/// A traceback line's place: "m/lib.lua:7: in f" is m/lib.lua, line 7.
fn trace_place(line: &str) -> Option<(String, usize)> {
    let (chunk, rest) = line.trim_start().split_once(':')?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || !rest[digits.len()..].starts_with(':') {
        return None;
    }
    Some((chunk.to_owned(), digits.parse().ok()?))
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
        Value::Thread(_) | Value::Userdata(_) => return Err(format!("a {} cannot be stored", v.type_name())),
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

pub fn color_arg(lua: &Lua, args: &[Value], i: usize, default: u32) -> Result<u32, Throw> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(default),
        Some(Value::Num(n)) => Ok(*n as i64 as u32),
        Some(Value::Str(s)) => {
            let t = String::from_utf8_lossy(s);
            let hex = t.trim_start_matches('#');
            // #rgb is #rrggbb; without alpha a colour is opaque.
            let full: String = match hex.len() {
                3 => hex.chars().flat_map(|c| [c, c]).collect(),
                6 | 8 => hex.to_owned(),
                _ => String::new(),
            };
            let v = u32::from_str_radix(&full, 16)
                .map_err(|_| bad_arg(lua, i, "color", "'#rgb', '#rrggbb' or '#aarrggbb' expected"))?;
            Ok(if full.len() == 6 { 0xFF00_0000 | v } else { v })
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
        "require(name): load a file of this module once (\"lib\" is lib.lua, \"lib.util\" lib/util.lua; require(script.lib) too)",
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
        "world.block(x, y, z): \"minecraft:stone\"; \"minecraft:air\" where nothing is loaded, nil outside a world",
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
        "world.entities([radius[, type]]): mobs, items and orbs near you as {entities = {...}}, nearest first (radius 16, at most 128)",
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
        "mod.info(): this mod's {id, name, version, author, reloads}",
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
        "via",
        "status",
        160,
        "via.status(): ViaHorizon's state: {enabled, version, label, text, ready}; joins go through ViaProxy as that version",
    ),
    (
        "via",
        "versions",
        161,
        "via.versions(): the versions it can join as, newest first: {{id = \"1.8.x\", label = \"1.8.x\"}, ...}; \"auto\" asks the server",
    ),
    (
        "via",
        "enable",
        162,
        "via.enable(on): join servers through ViaProxy (true) or straight (false)",
    ),
    (
        "via",
        "version",
        163,
        "via.version(id): the version joins go as from the next join (\"1.8.x\", \"1.12.2\", \"auto\"...)",
    ),
    (
        "via",
        "log",
        164,
        "via.log(): ViaProxy's last lines, for when a join fails",
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
    (
        "model",
        "attach",
        170,
        "model.attach(part, spec): attach a model to a body part of the player (or of spec.target, an entity id) that follows it as it moves and animates; returns its id. part: root, head, body, right_arm, left_arm, right_leg, left_leg (or hat, jacket, sleeves, pants). spec: {item = \"minecraft:carved_pumpkin\"} an item (blocks too), worn on the head or held in a hand where the game puts them; {boxes = {{from = {x, y, z}, to = {x, y, z}, color = \"#rrggbb\"}, ...}} coloured boxes; mesh = id a textured mesh (mesh.*, its x, y, z the part's left, up, forward); offset = {right, up, forward} in pixels (1/16 block), rotation = {pitch, yaw, roll} in degrees, scale = n or {x, y, z}, display = \"head\"|\"hand\"|\"left_hand\"|\"fixed\"|\"ground\"|\"gui\", glow = true (full bright), first_person = true (shown in first person too), visible = false",
    ),
    (
        "model",
        "set",
        171,
        "model.set(id, spec): change an attached model: only the fields given (item = false and boxes = false remove them)",
    ),
    (
        "model",
        "remove",
        172,
        "model.remove(id): remove an attached model; true if there was one",
    ),
    (
        "model",
        "clear",
        173,
        "model.clear(): remove every model this mod attached",
    ),
    (
        "model",
        "list",
        174,
        "model.list(): this mod's models: {{id, part, item, boxes (count), target}, ...}",
    ),
    (
        "player",
        "pose",
        175,
        "player.pose(): {pose = \"standing\"|\"crouching\"|\"swimming\"|\"gliding\"|\"spin_attack\"|\"sleeping\", crouching, swimming, gliding, sleeping, sprinting, on_ground, flying, camera = \"first\"|\"third_back\"|\"third_front\"}; nil outside a world",
    ),
    (
        "player",
        "part",
        176,
        "player.part(name[, target]): where a body part is this frame: {x, y, z, left = {x, y, z}, up = {...}, forward = {...}}, axes a block long (a model pixel is 1/16 of one); of the player, or of an entity by id; nil if there is none",
    ),
    (
        "player",
        "camera",
        177,
        "player.camera(): \"first\", \"third_back\" or \"third_front\" (F5)",
    ),
    (
        "draw",
        "item",
        116,
        "draw.item(id, x, y, z[, opts]): an item (or block) as a 3D model at a place (inside on(\"world\")); opts: {yaw, pitch, roll (degrees), scale, display = \"fixed\"|\"ground\"|\"gui\"|\"head\"|\"hand\"|\"left_hand\", glow = true}",
    ),
    (
        "draw",
        "mesh",
        117,
        "draw.mesh(id, x, y, z[, opts]): a mesh (mesh.*) at a place, lit by the world there (inside on(\"world\")); it faces +z at yaw 0, as player.look() gives yaws; opts: {yaw, pitch, roll (degrees), scale = n or {x, y, z}, color (tints it), texture = id (instead of its own), glow = true}",
    ),
    (
        "draw",
        "triangle",
        118,
        "draw.triangle(x1, y1, z1, x2, y2, z2, x3, y3, z3[, color]): a filled triangle in the world (inside on(\"world\"))",
    ),
    (
        "texture",
        "load",
        180,
        "texture.load(file): a PNG file of this folder mod (\"textures/cape.png\") as a texture; returns its id, width, height",
    ),
    (
        "texture",
        "decode",
        181,
        "texture.decode(data): a texture from a PNG as base64 text (a data: URL too) or as raw bytes; returns its id, width, height",
    ),
    (
        "texture",
        "create",
        182,
        "texture.create(w, h[, color]): a new texture of w x h pixels (at most 512 a side), all color (transparent without one); returns its id",
    ),
    (
        "texture",
        "set",
        183,
        "texture.set(id, x, y, color): one pixel, x and y from 0 at the top left",
    ),
    (
        "texture",
        "get",
        184,
        "texture.get(id, x, y): a pixel's colour as 0xAARRGGBB, or nil outside it",
    ),
    (
        "texture",
        "fill",
        185,
        "texture.fill(id, x, y, w, h, color): a rectangle of pixels",
    ),
    (
        "texture",
        "paint",
        186,
        "texture.paint(id, rows, palette[, x, y]): pixels from rows of characters, each looked up in palette ({[\"#\"] = \"#c0392b\", ...}; a character it lacks is left as it was), from x, y (0, 0)",
    ),
    ("texture", "size", 187, "texture.size(id): its width and height"),
    (
        "texture",
        "free",
        188,
        "texture.free(id): drops a texture; true if there was one",
    ),
    (
        "mesh",
        "load",
        190,
        "mesh.load(file[, opts]): a model file of this folder mod as a mesh: Wavefront .obj (opts.scale, default 16: the file is in blocks; opts.texture, opts.color) or Blockbench .bbmodel (its textures come with it; opts.flip = false keeps Blockbench's north-facing front); returns its id",
    ),
    (
        "mesh",
        "parse",
        191,
        "mesh.parse(text, format[, opts]): a mesh from a model file's text: format \"obj\" or \"bbmodel\", opts as mesh.load's; returns its id",
    ),
    (
        "mesh",
        "new",
        192,
        "mesh.new(spec): a mesh from Lua, in pixels: {vertices = {{x, y, z}, ...}, uvs = {{u, v}, ...} (0 to 1 from the top left), faces = {{1, 2, 3}, {v = {1, 2, 3, 4}, uv = {1, 2, 3, 4}, color = c}, ...}, boxes = {{from = {x, y, z}, to = {x, y, z}, uv = {u, v} (a skin's box layout) or faces = {north = {u1, v1, u2, v2}, ...}, color = c}, ...}, texture = id, color = c}; returns its id",
    ),
    (
        "mesh",
        "info",
        193,
        "mesh.info(id): {triangles, min = {x, y, z}, max = {x, y, z}, textures = {ids}} in pixels, or nil",
    ),
    (
        "mesh",
        "free",
        194,
        "mesh.free(id): drops a mesh; true if there was one",
    ),
    // Luau's task library, and Roblox's older globals around it.
    (
        "task",
        "wait",
        300,
        "task.wait([seconds]): wait (next frame by default) without stopping the game; returns the time waited",
    ),
    (
        "task",
        "spawn",
        301,
        "task.spawn(fn_or_thread, ...): run it now in a thread of its own; returns the thread",
    ),
    (
        "task",
        "defer",
        302,
        "task.defer(fn_or_thread, ...): run it in a thread of its own once the current code is done",
    ),
    (
        "task",
        "delay",
        303,
        "task.delay(seconds, fn_or_thread, ...): run it in a thread of its own later; returns the thread",
    ),
    ("task", "cancel", 304, "task.cancel(thread): stop a thread that waits"),
    (
        "task",
        "synchronize",
        305,
        "task.synchronize(): nothing here (no parallel Luau)",
    ),
    (
        "task",
        "desynchronize",
        306,
        "task.desynchronize(): nothing here (no parallel Luau)",
    ),
    (
        "",
        "wait",
        307,
        "wait([seconds]): Roblox's older task.wait; returns the time waited and the time",
    ),
    ("", "spawn", 308, "spawn(fn): Roblox's older task.defer"),
    ("", "delay", 309, "delay(seconds, fn): Roblox's older task.delay"),
    ("", "tick", 310, "tick(): seconds since 1970, with a fraction"),
    (
        "",
        "time",
        311,
        "time(): seconds since the studio started (as game.time)",
    ),
    ("", "warn", 312, "warn(...): print a warning in the console"),
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
    (
        "edit",
        204,
        "edit(id[, file]): open a mod's main file in the editor, or another file of a module (edit(\"tool\", \"lib.lua\"))",
    ),
    (
        "new",
        205,
        "new(name[, kind]): make a new mod and open it: a script, or with \"module\" a module (main.lua, shared.lua and lib.lua)",
    ),
];

/// What `require` of a file says outside a module.
const NOT_A_MODULE: &str =
    "require works in modules (a folder with main.lua): a one-file script can't load other files";

/// A running script's view of the studio and the game.
pub(crate) struct Ctx<'a> {
    pub(crate) game: &'a mut dyn GameApi,
    pub(crate) s: &'a mut State,
    pub(crate) owner: Owner,
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
                "{what} draws only inside on(\"hud\", function(w, h) ... end) or a screen's draw"
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
    /// `require(moduleScript)`: a module's file's ModuleScript is that file,
    /// required as by its name, and only by its own mod. Another one's
    /// Source (a hidden property here) runs once, and what it returns is
    /// kept.
    fn require_module(&mut self, lua: &mut Lua, m: u32) -> Res {
        let file = self.s.mods.iter().enumerate().find_map(|(j, md)| {
            md.module_scripts
                .iter()
                .find(|(id, _)| *id == m)
                .map(|(_, rel)| (j, rel.clone()))
        });
        if let Some((j, rel)) = file {
            if Owner::Mod(j) != self.owner {
                let id = self.s.mods[j].id.clone();
                return Err(lua.rt_error(format!(
                    "require: {} is a file of the mod '{id}': use import(\"{id}\") for another mod's values",
                    self.s.rbx.full_name(m)
                )));
            }
            return self.require_file(lua, &rel, &super::hstudio_modfiles::require_name(&rel));
        }
        let rbx = &self.s.rbx;
        if rbx.inst(m).class.name != "ModuleScript" {
            return Err(bad_arg(lua, 0, "require", "ModuleScript expected"));
        }
        if let Some((_, v)) = rbx.loaded_modules.iter().find(|(id, _)| *id == m) {
            return Ok(vec![v.clone()]);
        }
        let Some(src) = rbx
            .inst(m)
            .extra
            .get("Source")
            .and_then(Value::as_bytes)
            .map(<[u8]>::to_vec)
        else {
            return Err(lua.rt_error(
                "require: the ModuleScript has no source (set it with sethiddenproperty(module, \"Source\", code))",
            ));
        };
        let chunk = rbx.full_name(m);
        let env = super::hstudio_exec::caller_env(lua).ok_or_else(|| lua.rt_error("require: no environment"))?;
        let f = lua.load_bytes(&src, &chunk, &env).map_err(|e| lua.rt_error(e))?;
        let r = lua.call(self, &f, Vec::new())?;
        let v = match r.into_iter().next() {
            Some(v) if !v.is_nil() => v,
            _ => return Err(lua.rt_error("require: the module must return exactly one value")),
        };
        self.s.rbx.loaded_modules.push((m, v.clone()));
        Ok(vec![v])
    }
    /// `require` of a file of the running module: `rel` is its path in the
    /// module, `name` the name as the script wrote it (the file's `...`).
    /// The file runs once, in the mod's globals, and what it returns is kept
    /// for every later `require` of it, however that is spelt. A file that
    /// requires one still loading is a loop, which is an error naming the
    /// files.
    fn require_file(&mut self, lua: &mut Lua, rel: &str, name: &str) -> Res {
        if let Some(v) = self.me().modules.get(rel) {
            return Ok(vec![v.clone()]);
        }
        let Some(folder) = self.me().folder.clone() else {
            return Err(lua.rt_error(NOT_A_MODULE));
        };
        if rel == "main.lua" {
            return Err(lua.rt_error(
                "require: main.lua starts the module and runs once by itself; move what the other files need from it into a file they require",
            ));
        }
        if self.me().loading.iter().any(|l| l == rel) {
            // main.lua began it when its top level is what runs; a handler or a
            // command began it at the first file it required.
            let current = lua.current_thread();
            let in_main = self
                .me()
                .main_thread
                .as_ref()
                .is_some_and(|t| std::ptr::eq(t.as_ptr(), Rc::as_ptr(&current)));
            let chain: Vec<String> = in_main
                .then(|| "main.lua".to_owned())
                .into_iter()
                .chain(self.me().loading.iter().cloned())
                .chain(std::iter::once(rel.to_owned()))
                .collect();
            return Err(lua.rt_error(format!(
                "require: '{name}' is already loading ({}): two files can't require each other while they load; move what both need to a third file, or require it inside a function",
                chain.join(" -> ")
            )));
        }
        let Ok(src) = std::fs::read(path_in(&folder, rel)) else {
            let files = self.me().files().join(", ");
            return Err(lua.rt_error(format!("module '{name}' not found ({rel}; the module has {files})")));
        };
        let env = self
            .me()
            .env
            .clone()
            .ok_or_else(|| lua.rt_error("require: the mod is not running"))?;
        let ms = self
            .me()
            .module_scripts
            .iter()
            .find(|(_, r)| r == rel)
            .map(|(id, _)| *id);
        let env = match ms {
            Some(ms) => self.file_env(ms, &env),
            None => env,
        };
        let chunk = format!("{}/{rel}", self.s.owner_name(self.owner));
        let f = lua.load_bytes(&src, &chunk, &env).map_err(|e| lua.rt_error(e))?;
        self.me().loading.push(rel.to_owned());
        let r = lua.call(self, &f, vec![Value::str(name)]);
        self.me().loading.pop();
        let v = match r?.into_iter().next() {
            Some(v) if !v.is_nil() => v,
            _ => Value::Bool(true),
        };
        self.me().modules.insert(rel.to_owned(), v.clone());
        if let Some(ms) = ms {
            self.s.rbx.loaded_modules.push((ms, v.clone()));
        }
        Ok(vec![v])
    }

    /// The globals a module's file runs in: the mod's own, seen through a
    /// table of the file's where `script` is its ModuleScript (as each script
    /// has its own `script` in Roblox). Everything else it reads and sets is
    /// the mod's, so the files still share their globals.
    fn file_env(&mut self, ms: u32, env: &TableRef) -> TableRef {
        let mut t = Table::new();
        t.set_str("script", self.s.rbx.value(ms));
        let mut meta = Table::new();
        meta.set_str("__index", Value::Table(env.clone()));
        meta.set_str("__newindex", Value::Table(env.clone()));
        t.meta = Some(Rc::new(std::cell::RefCell::new(meta)));
        Rc::new(std::cell::RefCell::new(t))
    }
    fn storage_dir(&self) -> PathBuf {
        self.s.dir.join("storage").join(self.s.owner_name(self.owner))
    }
    /// Parks the running thread until `wake`: it yields, and the scheduler
    /// resumes it.
    pub(crate) fn park(&mut self, lua: &Lua, wake: Wake, what: &str) -> Res {
        if !lua.is_yieldable() {
            return Err(lua.rt_error(format!(
                "{what} can't wait here: this code isn't in a thread that can wait (run it with task.spawn)"
            )));
        }
        self.schedule(lua, lua.current_thread(), wake)?;
        Err(Throw::Yield(Vec::new()))
    }
    /// Has the scheduler resume `thread` at `wake`.
    pub(crate) fn schedule(&mut self, lua: &Lua, thread: Rc<Thread>, wake: Wake) -> Result<(), Throw> {
        if self.me().tasks.len() >= MAX_TASKS {
            return Err(lua.rt_error(format!("too many waiting threads ({MAX_TASKS})")));
        }
        let (since, frame) = (self.s.now, self.s.frame_no);
        self.me().tasks.push(Task {
            thread,
            wake,
            since,
            frame,
        });
        Ok(())
    }
    /// A function or a thread argument, as a thread to run.
    fn thread_arg(&mut self, lua: &mut Lua, args: &[Value], i: usize, fname: &str) -> Result<Rc<Thread>, Throw> {
        match args.get(i) {
            Some(Value::Thread(t)) => Ok(t.clone()),
            Some(f) if f.is_callable() => Ok(lua.new_thread(f.clone())),
            _ => Err(bad_arg(lua, i, fname, "function or thread expected")),
        }
    }
    /// Resumes `co` now, inside the running call; its error stops its
    /// owner once the call is over.
    pub(crate) fn resume_now(&mut self, lua: &mut Lua, co: &Rc<Thread>, args: Vec<Value>) {
        if !co.is_suspended() {
            return;
        }
        if let Resume::Error(e) = lua.resume(self, co, args) {
            let (msg, trace) = (e.message(), e.traceback().to_owned());
            self.s.thread_errors.push((self.owner, msg, trace));
        }
    }
    fn task(&mut self, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res {
        let wait_secs = |lua: &Lua, args: &[Value], fname: &str| -> Result<f64, Throw> {
            let n = opt_num(lua, args, 0, fname, 0.0)?;
            Ok(if n.is_finite() { n.max(0.0) } else { 0.0 })
        };
        match id {
            300 | 307 => {
                let legacy = id == 307;
                let secs = wait_secs(lua, &args, "wait")?;
                // The older wait waits at least a thirtieth of a second.
                let secs = if legacy { secs.max(1.0 / 30.0) } else { secs };
                let at = self.s.now + secs;
                self.park(
                    lua,
                    Wake::At { at, args: None, legacy },
                    if legacy { "wait" } else { "task.wait" },
                )
            }
            301 => {
                let co = self.thread_arg(lua, &args, 0, "spawn")?;
                self.resume_now(lua, &co, args[1..].to_vec());
                Ok(vec![Value::Thread(co)])
            }
            302 => {
                let co = self.thread_arg(lua, &args, 0, "defer")?;
                self.schedule(lua, co.clone(), Wake::Now(args[1..].to_vec()))?;
                Ok(vec![Value::Thread(co)])
            }
            303 => {
                let secs = wait_secs(lua, &args, "delay")?;
                let co = self.thread_arg(lua, &args, 1, "delay")?;
                let at = self.s.now + secs;
                let rest = args.get(2..).unwrap_or_default().to_vec();
                self.schedule(
                    lua,
                    co.clone(),
                    Wake::At {
                        at,
                        args: Some(rest),
                        legacy: false,
                    },
                )?;
                Ok(vec![Value::Thread(co)])
            }
            304 => {
                let Some(Value::Thread(co)) = args.first() else {
                    return Err(bad_arg(lua, 0, "cancel", "thread expected"));
                };
                let co = co.clone();
                self.me().tasks.retain(|t| !Rc::ptr_eq(&t.thread, &co));
                if co.is_suspended() {
                    let _ = lua.close_thread(&co);
                } else if !co.is_dead() {
                    return Err(lua.rt_error("cannot cancel a thread that is running"));
                }
                Ok(Vec::new())
            }
            305 | 306 => Ok(Vec::new()),
            308 => {
                let f = check_func(lua, &args, 0, "spawn")?;
                let co = lua.new_thread(f);
                let now = self.s.now;
                let wake = Wake::At {
                    at: now,
                    args: Some(vec![Value::Num(0.0), Value::Num(now)]),
                    legacy: true,
                };
                self.schedule(lua, co, wake)?;
                Ok(Vec::new())
            }
            309 => {
                let secs = check_num(lua, &args, 0, "delay")?;
                let f = check_func(lua, &args, 1, "delay")?;
                let co = lua.new_thread(f);
                let at = self.s.now + if secs.is_finite() { secs.max(0.0) } else { 0.0 };
                let wake = Wake::At {
                    at,
                    args: None,
                    legacy: true,
                };
                self.schedule(lua, co, wake)?;
                Ok(Vec::new())
            }
            310 => {
                let t = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0.0, |d| d.as_secs_f64());
                Ok(vec![Value::Num(t)])
            }
            311 => Ok(vec![Value::Num(self.s.now)]),
            _ => {
                let mut parts = Vec::with_capacity(args.len());
                for a in &args {
                    parts.push(lua.tostring(self, a)?);
                }
                let who = match self.owner {
                    Owner::Console => String::new(),
                    Owner::Mod(i) => format!("[{}] ", self.s.mods[i].id),
                };
                self.s.log(LogKind::Error, format!("{who}warning: {}", parts.join(" ")));
                Ok(Vec::new())
            }
        }
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
    /// `draw.item(id, x, y, z[, opts])`.
    fn draw_item(&mut self, lua: &Lua, args: &[Value]) -> Res {
        if self.phase != Phase::World {
            return Err(lua.rt_error("draw.item draws only inside on(\"world\", function() ... end)"));
        }
        let mut a = super::hstudio_models::Attachment::new(0, "root");
        a.item = Some(check_text(lua, args, 0, "item")?);
        let at = [
            check_num(lua, args, 1, "item")?,
            check_num(lua, args, 2, "item")?,
            check_num(lua, args, 3, "item")?,
        ];
        let mut display = "fixed".to_owned();
        if let Some(Value::Table(o)) = args.get(4) {
            let o = o.borrow();
            let num = |k: &str, d: f64| o.get_str(k).to_num().unwrap_or(d);
            a.rotation = [num("pitch", 0.0), num("yaw", 0.0), num("roll", 0.0)];
            a.scale = [num("scale", 1.0); 3];
            a.glow = o.get_str("glow").truthy();
            if let Some(d) = o.get_str("display").as_text() {
                if !super::hstudio_models::DISPLAYS.contains(&d.as_str()) {
                    return Err(bad_arg(
                        lua,
                        4,
                        "item",
                        "display is fixed, ground, gui, head, hand or left_hand",
                    ));
                }
                display = d;
            }
        }
        if self.s.world_quads >= MAX_WORLD_QUADS {
            return Err(lua.rt_error("too much drawn in the world this frame"));
        }
        self.s.world_quads += 1;
        a.display = Some(display);
        let frame = PartFrame {
            origin: at,
            left: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            forward: [0.0, 0.0, 1.0],
        };
        let item = a.item.clone().unwrap_or_default();
        let r = super::hstudio_models::rotation(a.rotation);
        let col = |i: usize| [r[i][0] * a.scale[0], r[i][1] * a.scale[1], r[i][2] * a.scale[2]];
        let light = (!a.glow).then_some(frame.origin);
        let display = a.display.clone().unwrap_or_default();
        self.game
            .world_item(&item, [col(0), col(1), col(2), at], &display, light)
            .map_err(|e| lua.rt_error(format!("draw.item: {e}")))?;
        Ok(Vec::new())
    }

    /// `draw.mesh(id, x, y, z[, opts])`.
    fn draw_mesh(&mut self, lua: &Lua, args: &[Value]) -> Res {
        if self.phase != Phase::World {
            return Err(lua.rt_error("draw.mesh draws only inside on(\"world\", function() ... end)"));
        }
        let id = check_num(lua, args, 0, "mesh")? as u32;
        let at = [
            check_num(lua, args, 1, "mesh")?,
            check_num(lua, args, 2, "mesh")?,
            check_num(lua, args, 3, "mesh")?,
        ];
        let (mut rot, mut scale, mut argb, mut texture, mut glow) = ([0.0; 3], [1.0; 3], 0xFFFF_FFFF, None, false);
        if let Some(Value::Table(o)) = args.get(4) {
            let o = o.borrow();
            let num = |k: &str| o.get_str(k).to_num().unwrap_or(0.0);
            // As player.look() turns: yaw 90 faces west.
            rot = [num("pitch"), -num("yaw"), num("roll")];
            scale = match o.get_str("scale") {
                Value::Nil => [1.0; 3],
                Value::Table(t) => {
                    let t = t.borrow();
                    let n = |i: f64| t.get(&Value::Num(i)).to_num().unwrap_or(1.0);
                    [n(1.0), n(2.0), n(3.0)]
                }
                v => {
                    [v.to_num()
                        .ok_or_else(|| bad_arg(lua, 4, "mesh", "scale is a number or {x, y, z}"))?;
                        3]
                }
            };
            let c = o.get_str("color");
            if !c.is_nil() {
                argb = color_arg(lua, &[c], 0, 0xFFFF_FFFF)?;
            }
            let t = o.get_str("texture");
            if !t.is_nil() {
                let tid =
                    t.to_num()
                        .ok_or_else(|| bad_arg(lua, 4, "mesh", "texture is a texture id"))? as u32;
                if !self.me().meshes.textures.contains_key(&tid) {
                    return Err(lua.rt_error(format!("draw.mesh: this mod has no texture {tid}")));
                }
                texture = Some(Some(tid));
            }
            glow = o.get_str("glow").truthy();
        }
        let mut budget = MAX_WORLD_QUADS.saturating_sub(self.s.world_quads);
        let before = budget;
        let me = self.s.owner_mod(self.owner);
        let Some(mesh) = me.meshes.meshes.get(&id) else {
            return Err(lua.rt_error(format!("draw.mesh: this mod has no mesh {id}")));
        };
        let r = super::hstudio_models::rotation(rot);
        let place = |p: [f32; 3]| {
            let v = [
                f64::from(p[0]) * scale[0],
                f64::from(p[1]) * scale[1],
                f64::from(p[2]) * scale[2],
            ];
            let w = [
                r[0][0] * v[0] + r[1][0] * v[1] + r[2][0] * v[2],
                r[0][1] * v[0] + r[1][1] * v[1] + r[2][1] * v[2],
                r[0][2] * v[0] + r[1][2] * v[1] + r[2][2] * v[2],
            ];
            [at[0] + w[0] / 16.0, at[1] + w[1] / 16.0, at[2] + w[2] / 16.0]
        };
        let light = (!glow).then_some(at);
        let whole = super::hstudio_meshes::draw(
            &mut *self.game,
            &me.meshes,
            mesh,
            place,
            argb,
            texture,
            light,
            &mut budget,
        );
        self.s.world_quads += before - budget;
        if !whole {
            return Err(lua.rt_error("too much drawn in the world this frame"));
        }
        Ok(Vec::new())
    }
    /// `draw.triangle(x1, y1, z1, x2, y2, z2, x3, y3, z3[, color])`.
    fn draw_triangle(&mut self, lua: &Lua, args: &[Value]) -> Res {
        if self.phase != Phase::World {
            return Err(lua.rt_error("draw.triangle draws only inside on(\"world\", function() ... end)"));
        }
        let mut p = [[0.0; 3]; 3];
        for (i, c) in p.iter_mut().flatten().enumerate() {
            *c = check_num(lua, args, i, "triangle")?;
        }
        let argb = color_arg(lua, args, 9, 0xFFFF_FFFF)?;
        if self.s.world_quads >= MAX_WORLD_QUADS {
            return Err(lua.rt_error("too much drawn in the world this frame"));
        }
        self.s.world_quads += 1;
        self.game.world_quad([p[0], p[1], p[2], p[2]], argb);
        Ok(Vec::new())
    }

    /// A folder mod's file, for texture.load and mesh.load.
    fn asset(&mut self, lua: &Lua, file: &str, fname: &str) -> Result<Vec<u8>, Throw> {
        let Some(folder) = self.me().folder.clone() else {
            return Err(lua.rt_error(format!(
                "{fname} reads files of a folder mod (mods/<id>/main.lua); a one-file mod can use {}",
                if fname.starts_with("texture") {
                    "texture.decode or texture.create"
                } else {
                    "mesh.parse or mesh.new"
                }
            )));
        };
        let rel = std::path::Path::new(file);
        let plain = rel.components().all(|c| matches!(c, std::path::Component::Normal(_)));
        if file.is_empty() || !plain {
            return Err(lua.rt_error(format!("{fname}: '{file}' is not a path inside the mod's folder")));
        }
        let path = folder.join(rel);
        let len = std::fs::metadata(&path)
            .map_err(|_| lua.rt_error(format!("{fname}: no file '{file}' in the mod's folder")))?
            .len();
        if len > 16 << 20 {
            return Err(lua.rt_error(format!("{fname}: '{file}' is over 16 MB")));
        }
        std::fs::read(&path).map_err(|e| lua.rt_error(format!("{fname}: can't read '{file}': {e}")))
    }

    /// Keeps a texture, if every mod's textures together still fit.
    fn keep_texture(&mut self, lua: &Lua, t: super::hstudio_meshes::Texture, fname: &str) -> Result<u32, Throw> {
        use super::hstudio_meshes::MAX_TEXELS;
        let used: usize = self
            .s
            .mods
            .iter()
            .chain(std::iter::once(&self.s.console_mod))
            .map(|m| m.meshes.texels())
            .sum();
        if used + t.texels() > MAX_TEXELS {
            return Err(lua.rt_error(format!(
                "{fname}: the mods' textures are at most {MAX_TEXELS} pixels in all (a 1024x1024); texture.free drops one"
            )));
        }
        self.s
            .owner_mod(self.owner)
            .meshes
            .add_texture(t)
            .map_err(|e| lua.rt_error(format!("{fname}: {e}")))
    }

    /// `texture.*`.
    fn texture(&mut self, lua: &Lua, id: u32, args: &[Value]) -> Res {
        use super::hstudio_meshes::{Texture, decode_png, texture_from};
        let name = [
            "load", "decode", "create", "set", "get", "fill", "paint", "size", "free",
        ][(id - 180) as usize];
        let fname = format!("texture.{name}");
        let made = |t: &Texture, n: u32| {
            vec![
                Value::Num(f64::from(n)),
                Value::Num(f64::from(t.w)),
                Value::Num(f64::from(t.h)),
            ]
        };
        match id {
            180 => {
                let file = check_text(lua, args, 0, name)?;
                let bytes = self.asset(lua, &file, &fname)?;
                let t = decode_png(&bytes).map_err(|e| lua.rt_error(format!("{fname}: '{file}': {e}")))?;
                let out = made(&t, 0);
                let n = self.keep_texture(lua, t, &fname)?;
                Ok([vec![Value::Num(f64::from(n))], out[1..].to_vec()].concat())
            }
            181 => {
                let data = check_str(lua, args, 0, name)?;
                let t = texture_from(&data).map_err(|e| lua.rt_error(format!("{fname}: {e}")))?;
                let out = made(&t, 0);
                let n = self.keep_texture(lua, t, &fname)?;
                Ok([vec![Value::Num(f64::from(n))], out[1..].to_vec()].concat())
            }
            182 => {
                let w = check_num(lua, args, 0, name)?;
                let h = check_num(lua, args, 1, name)?;
                let argb = color_arg(lua, args, 2, 0)?;
                let t = Texture::new(w.max(0.0) as u32, h.max(0.0) as u32, argb)
                    .map_err(|e| lua.rt_error(format!("{fname}: {e}")))?;
                let n = self.keep_texture(lua, t, &fname)?;
                Ok(vec![Value::Num(f64::from(n))])
            }
            188 => {
                let n = check_num(lua, args, 0, name)? as u32;
                Ok(vec![Value::Bool(self.me().meshes.textures.remove(&n).is_some())])
            }
            _ => {
                let n = check_num(lua, args, 0, name)? as u32;
                // Arguments read before the texture is borrowed.
                let ints = |from: usize, count: usize| -> Result<Vec<i64>, Throw> {
                    (from..from + count)
                        .map(|i| check_num(lua, args, i, name).map(|v| v.floor() as i64))
                        .collect()
                };
                let pre = match id {
                    183 => Some((ints(1, 2)?, color_arg(lua, args, 3, 0)?)),
                    185 => Some((ints(1, 4)?, color_arg(lua, args, 5, 0)?)),
                    _ => None,
                };
                let paint = if id == 186 {
                    let rows: Vec<String> = check_table(lua, args, 1, name)?
                        .borrow()
                        .list()
                        .iter()
                        .map(|v| v.as_text().unwrap_or_default())
                        .collect();
                    let palette = check_table(lua, args, 2, name)?;
                    let mut colors = HashMap::new();
                    for row in &rows {
                        for c in row.chars() {
                            if colors.contains_key(&c) {
                                continue;
                            }
                            let v = palette.borrow().get(&Value::str(&c.to_string()));
                            let col = match v {
                                Value::Nil | Value::Bool(false) => None,
                                v => Some(color_arg(lua, &[v], 0, 0)?),
                            };
                            colors.insert(c, col);
                        }
                    }
                    let at = (
                        opt_num(lua, args, 3, name, 0.0)? as i64,
                        opt_num(lua, args, 4, name, 0.0)? as i64,
                    );
                    Some((rows, colors, at))
                } else {
                    None
                };
                let t = self
                    .me()
                    .meshes
                    .textures
                    .get_mut(&n)
                    .ok_or_else(|| lua.rt_error(format!("{fname}: this mod has no texture {n}")))?;
                let inside = |x: i64, y: i64| (x >= 0 && y >= 0).then_some((x as u32, y as u32));
                match id {
                    183 => {
                        let (xy, argb) = pre.unwrap_or_default();
                        if let Some((x, y)) = inside(xy[0], xy[1])
                            && t.set(x, y, argb)
                        {
                            t.touched();
                        }
                        Ok(Vec::new())
                    }
                    184 => {
                        let x = check_num(lua, args, 1, name)?.floor() as i64;
                        let y = check_num(lua, args, 2, name)?.floor() as i64;
                        Ok(vec![
                            inside(x, y)
                                .and_then(|(x, y)| t.get(x, y))
                                .map_or(Value::Nil, |c| Value::Num(f64::from(c))),
                        ])
                    }
                    185 => {
                        let (r, argb) = pre.unwrap_or_default();
                        let (x0, y0) = (r[0].max(0), r[1].max(0));
                        let (x1, y1) = ((r[0] + r[2]).min(i64::from(t.w)), (r[1] + r[3]).min(i64::from(t.h)));
                        for y in y0..y1 {
                            for x in x0..x1 {
                                t.set(x as u32, y as u32, argb);
                            }
                        }
                        t.touched();
                        Ok(Vec::new())
                    }
                    186 => {
                        let (rows, colors, (ox, oy)) = paint.unwrap_or_default();
                        for (dy, row) in rows.iter().enumerate() {
                            for (dx, c) in row.chars().enumerate() {
                                if let (Some(Some(col)), Some((x, y))) =
                                    (colors.get(&c), inside(ox + dx as i64, oy + dy as i64))
                                {
                                    t.set(x, y, *col);
                                }
                            }
                        }
                        t.touched();
                        Ok(Vec::new())
                    }
                    _ => Ok(vec![Value::Num(f64::from(t.w)), Value::Num(f64::from(t.h))]),
                }
            }
        }
    }

    /// `mesh.*`.
    fn mesh(&mut self, lua: &Lua, id: u32, args: &[Value]) -> Res {
        let name = ["load", "parse", "new", "info", "free"][(id - 190) as usize];
        let fname = format!("mesh.{name}");
        let err = |e: String| lua.rt_error(format!("{fname}: {e}"));
        match id {
            190 | 191 => {
                let (text, format, opts) = if id == 190 {
                    let file = check_text(lua, args, 0, name)?;
                    let bytes = self.asset(lua, &file, &fname)?;
                    let format = std::path::Path::new(&file)
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    (String::from_utf8_lossy(&bytes).into_owned(), format, args.get(1))
                } else {
                    (
                        check_text(lua, args, 0, name)?,
                        check_text(lua, args, 1, name)?.to_lowercase(),
                        args.get(2),
                    )
                };
                let opt = |k: &str| match opts {
                    Some(Value::Table(t)) => t.borrow().get_str(k),
                    _ => Value::Nil,
                };
                let argb = match opt("color") {
                    Value::Nil => 0xFFFF_FFFF,
                    c => color_arg(lua, &[c], 0, 0xFFFF_FFFF)?,
                };
                let texture = match opt("texture") {
                    Value::Nil => None,
                    v => {
                        let t = v.to_num().ok_or_else(|| err("texture is a texture id".into()))? as u32;
                        if !self.me().meshes.textures.contains_key(&t) {
                            return Err(err(format!("this mod has no texture {t}")));
                        }
                        Some(t)
                    }
                };
                let mesh = match format.as_str() {
                    "obj" => {
                        let scale = opt("scale").to_num().unwrap_or(16.0) as f32;
                        super::hstudio_meshes::parse_obj(&text, scale, argb, texture).map_err(err)?
                    }
                    "bbmodel" => {
                        let flip = !matches!(opt("flip"), Value::Bool(false));
                        let imported = super::hstudio_meshes::parse_bbmodel(&text, flip).map_err(err)?;
                        // Its textures become this mod's; the triangles take their ids.
                        let mut ids = Vec::new();
                        for t in imported.textures {
                            ids.push(match t {
                                Some(t) => Some(self.keep_texture(lua, t, &fname)?),
                                None => None,
                            });
                        }
                        let mut mesh = imported.mesh;
                        for t in &mut mesh.tris {
                            t.tex = t.tex.and_then(|i| ids.get(i as usize).copied().flatten()).or(texture);
                            t.argb = argb;
                        }
                        mesh
                    }
                    other => return Err(err(format!("'{other}' is not a format it reads: obj or bbmodel"))),
                };
                let n = self.me().meshes.add_mesh(mesh).map_err(err)?;
                Ok(vec![Value::Num(f64::from(n))])
            }
            192 => {
                let spec = check_table(lua, args, 0, name)?;
                let mesh = self.mesh_from(lua, &spec.borrow(), &fname)?;
                let n = self.me().meshes.add_mesh(mesh).map_err(err)?;
                Ok(vec![Value::Num(f64::from(n))])
            }
            193 => {
                let n = check_num(lua, args, 0, name)? as u32;
                let Some(m) = self.me().meshes.meshes.get(&n) else {
                    return Ok(vec![Value::Nil]);
                };
                let (lo, hi) = m.bounds();
                let v =
                    |p: [f32; 3]| Value::table(Table::from_list(p.iter().map(|c| Value::Num(f64::from(*c))).collect()));
                let mut tex: Vec<u32> = m.tris.iter().filter_map(|t| t.tex).collect();
                tex.sort_unstable();
                tex.dedup();
                let mut t = Table::new();
                t.set_str("triangles", Value::Num(m.tris.len() as f64));
                t.set_str("min", v(lo));
                t.set_str("max", v(hi));
                t.set_str(
                    "textures",
                    Value::table(Table::from_list(
                        tex.into_iter().map(|i| Value::Num(f64::from(i))).collect(),
                    )),
                );
                Ok(vec![Value::table(t)])
            }
            _ => {
                let n = check_num(lua, args, 0, name)? as u32;
                Ok(vec![Value::Bool(self.me().meshes.meshes.remove(&n).is_some())])
            }
        }
    }

    /// `mesh.new`'s spec as a mesh.
    fn mesh_from(&mut self, lua: &Lua, spec: &Table, fname: &str) -> Result<super::hstudio_meshes::Mesh, Throw> {
        use super::hstudio_meshes::{FACES, Mesh, Tri, add_box, box_uv};
        let err = |e: &str| lua.rt_error(format!("{fname}: {e}"));
        let color =
            |v: Value, d: u32| -> Result<u32, Throw> { if v.is_nil() { Ok(d) } else { color_arg(lua, &[v], 0, d) } };
        let argb = color(spec.get_str("color"), 0xFFFF_FFFF)?;
        let texture = match spec.get_str("texture") {
            Value::Nil => None,
            v => {
                let t = v.to_num().ok_or_else(|| err("texture is a texture id"))? as u32;
                if !self.me().meshes.textures.contains_key(&t) {
                    return Err(err(&format!("this mod has no texture {t}")));
                }
                Some(t)
            }
        };
        let size = texture
            .and_then(|t| self.me().meshes.textures.get(&t))
            .map_or([16.0, 16.0], |t| [t.w as f32, t.h as f32]);
        let nums = |v: &Value, n: usize| -> Option<Vec<f32>> {
            let Value::Table(t) = v else { return None };
            let list = t.borrow().list();
            (list.len() >= n).then(|| list.iter().take(n).map(|x| x.to_num().unwrap_or(0.0) as f32).collect())
        };
        let mut mesh = Mesh::default();
        let verts: Vec<[f32; 3]> = match spec.get_str("vertices") {
            Value::Nil => Vec::new(),
            Value::Table(t) => t
                .borrow()
                .list()
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    nums(v, 3)
                        .map(|p| [p[0], p[1], p[2]])
                        .ok_or_else(|| err(&format!("vertices[{}] is {{x, y, z}}", i + 1)))
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(err("vertices is a list of {x, y, z}")),
        };
        let uvs: Vec<[f32; 2]> = match spec.get_str("uvs") {
            Value::Table(t) => t
                .borrow()
                .list()
                .iter()
                .map(|v| nums(v, 2).map_or([0.0, 0.0], |p| [p[0], p[1]]))
                .collect(),
            _ => Vec::new(),
        };
        if let Value::Table(faces) = spec.get_str("faces") {
            for (i, f) in faces.borrow().list().iter().enumerate() {
                let bad = || {
                    err(&format!(
                        "faces[{}] is {{a, b, c}} or {{v = {{...}}, uv = {{...}}}} (vertex numbers from 1)",
                        i + 1
                    ))
                };
                let Value::Table(ft) = f else { return Err(bad()) };
                let ft = ft.borrow();
                let (vs, us, c) = match ft.get_str("v") {
                    Value::Nil => (ft.list(), Vec::new(), argb),
                    Value::Table(v) => {
                        let us = match ft.get_str("uv") {
                            Value::Table(u) => u.borrow().list(),
                            _ => Vec::new(),
                        };
                        (v.borrow().list(), us, color(ft.get_str("color"), argb)?)
                    }
                    _ => return Err(bad()),
                };
                let index = |v: &Value, len: usize| -> Option<usize> {
                    let n = v.to_num()?;
                    (n >= 1.0 && n as usize <= len && n.fract() == 0.0).then(|| n as usize - 1)
                };
                let corners: Vec<([f32; 3], [f32; 2])> = vs
                    .iter()
                    .enumerate()
                    .map(|(k, v)| {
                        let p = verts[index(v, verts.len()).ok_or_else(bad)?];
                        // Each corner's uv: given, else the vertex's own.
                        let uv = match us.get(k) {
                            Some(u) => uvs
                                .get(index(u, uvs.len()).ok_or_else(bad)?)
                                .copied()
                                .unwrap_or([0.0; 2]),
                            None => index(v, uvs.len())
                                .and_then(|j| uvs.get(j).copied())
                                .unwrap_or([0.0; 2]),
                        };
                        Ok((p, uv))
                    })
                    .collect::<Result<_, Throw>>()?;
                if corners.len() < 3 {
                    return Err(bad());
                }
                for k in 1..corners.len() - 1 {
                    let (a, b, d) = (corners[0], corners[k], corners[k + 1]);
                    if mesh.tris.len() >= super::hstudio_meshes::MAX_TRIANGLES {
                        return Err(err("too many triangles"));
                    }
                    mesh.tris.push(Tri {
                        p: [a.0, b.0, d.0],
                        uv: [a.1, b.1, d.1],
                        argb: c,
                        tex: texture,
                    });
                }
            }
        }
        if let Value::Table(boxes) = spec.get_str("boxes") {
            for (i, b) in boxes.borrow().list().iter().enumerate() {
                let bad = |what: &str| err(&format!("boxes[{}]: {what}", i + 1));
                let Value::Table(bt) = b else {
                    return Err(bad("a box is {from = {x, y, z}, to = {x, y, z}, ...}"));
                };
                let bt = bt.borrow();
                let (Some(from), Some(to)) = (nums(&bt.get_str("from"), 3), nums(&bt.get_str("to"), 3)) else {
                    return Err(bad("needs from and to, each {x, y, z} in pixels"));
                };
                let lo = [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])];
                let hi = [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])];
                let c = color(bt.get_str("color"), argb)?;
                let dims = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
                let at = nums(&bt.get_str("uv"), 2);
                let faces = bt.get_str("faces");
                let rect_of = |dir: &str| -> Option<([f32; 4], i32)> {
                    if let Value::Table(f) = &faces {
                        return nums(&f.borrow().get_str(dir), 4).map(|r| ([r[0], r[1], r[2], r[3]], 0));
                    }
                    Some((at.as_ref().map_or([0.0; 4], |a| box_uv(dir, a[0], a[1], dims, true)), 0))
                };
                let _ = FACES;
                add_box(&mut mesh, lo, hi, c, texture, size, rect_of).map_err(|e| bad(&e))?;
            }
        }
        if mesh.tris.is_empty() {
            return Err(err("give faces (with vertices) or boxes"));
        }
        Ok(mesh)
    }

    /// `model.*`: this mod's attached models.
    fn model(&mut self, lua: &Lua, id: u32, args: &[Value]) -> Res {
        use super::hstudio_models::{Attachment, MAX_MODELS, apply_spec, check_part, target_of};
        let name = ["attach", "set", "remove", "clear", "list"][(id - 170) as usize];
        match id {
            170 => {
                let part = check_text(lua, args, 0, name)?;
                let spec = check_table(lua, args, 1, name)?;
                let target = target_of(lua, &spec.borrow().get_str("target"), "model.attach")?.flatten();
                check_part(lua, &part, target, name)?;
                if self.s.owner_mod(self.owner).models.len() >= MAX_MODELS {
                    return Err(lua.rt_error(format!("a mod attaches at most {MAX_MODELS} models")));
                }
                self.s.next_model += 1;
                let mut a = Attachment::new(self.s.next_model, &part);
                apply_spec(lua, &mut a, &spec.borrow(), "model.attach")?;
                if let Some(m) = a.mesh
                    && !self.me().meshes.meshes.contains_key(&m)
                {
                    return Err(lua.rt_error(format!("model.attach: this mod has no mesh {m}")));
                }
                if a.item.is_none() && a.boxes.is_empty() && a.mesh.is_none() {
                    return Err(lua.rt_error("model.attach: give an item, boxes or a mesh"));
                }
                let n = a.id;
                self.s.owner_mod(self.owner).models.push(a);
                Ok(vec![Value::Num(f64::from(n))])
            }
            171 => {
                let n = check_num(lua, args, 0, name)? as u32;
                let spec = check_table(lua, args, 1, name)?;
                let m = self.s.owner_mod(self.owner);
                let a = m
                    .models
                    .iter_mut()
                    .find(|a| a.id == n)
                    .ok_or_else(|| lua.rt_error(format!("model.set: this mod has no model {n}")))?;
                apply_spec(lua, a, &spec.borrow(), "model.set")?;
                Ok(Vec::new())
            }
            172 => {
                let n = check_num(lua, args, 0, name)? as u32;
                let m = self.s.owner_mod(self.owner);
                let before = m.models.len();
                m.models.retain(|a| a.id != n);
                Ok(vec![Value::Bool(m.models.len() < before)])
            }
            173 => {
                self.s.owner_mod(self.owner).models.clear();
                Ok(Vec::new())
            }
            _ => {
                let list: Vec<Value> = self
                    .s
                    .owner_mod(self.owner)
                    .models
                    .iter()
                    .map(|a| {
                        let mut t = Table::new();
                        t.set_str("id", Value::Num(f64::from(a.id)));
                        t.set_str("part", Value::str(&a.part));
                        if let Some(item) = &a.item {
                            t.set_str("item", Value::str(item));
                        }
                        t.set_str("boxes", Value::Num(a.boxes.len() as f64));
                        if let Some(mesh) = a.mesh {
                            t.set_str("mesh", Value::Num(f64::from(mesh)));
                        }
                        t.set_str(
                            "target",
                            a.target.map_or(Value::str("player"), |id| Value::Num(f64::from(id))),
                        );
                        Value::table(t)
                    })
                    .collect();
                Ok(vec![Value::table(Table::from_list(list))])
            }
        }
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

impl super::hstudio_rbx::Env for Ctx<'_> {
    fn rbx(&mut self) -> &mut super::hstudio_rbx::Rbx {
        &mut self.s.rbx
    }
    fn owner(&self) -> Owner {
        self.owner
    }
    fn now(&self) -> f64 {
        self.s.now
    }
    fn game(&mut self) -> &mut dyn GameApi {
        self.game
    }
    fn rbx_and_game(&mut self) -> (&mut super::hstudio_rbx::Rbx, &mut dyn GameApi) {
        (&mut self.s.rbx, self.game)
    }
    fn held(&self, key: &str) -> bool {
        self.s.held.contains(key)
    }
    fn spawn_now(&mut self, lua: &mut Lua, f: Value, args: Vec<Value>) {
        let co = lua.new_thread(f);
        self.resume_now(lua, &co, args);
    }
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
                if let Some(m) = args.first().and_then(|v| self.s.rbx.id_of(v)) {
                    return self.require_module(lua, m);
                }
                let name = check_text(lua, &args, 0, "require")?;
                if self.me().folder.is_none() {
                    return Err(lua.rt_error(NOT_A_MODULE));
                }
                let rel = require_path(&name).ok_or_else(|| {
                    lua.rt_error(format!(
                        "bad module name '{name}': name a file of the module, as \"lib\" or \"lib.util\""
                    ))
                })?;
                self.require_file(lua, &rel, &name)
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
                self.drawing(lua, "hud.text")?;
                let x = check_num(lua, &args, 0, "text")? as f32;
                let y = check_num(lua, &args, 1, "text")? as f32;
                let text = lua.tostring(self, &arg(&args, 2))?;
                let color = color_arg(lua, &args, 3, 0xFFFF_FFFF)?;
                let shadow = !matches!(args.get(4), Some(Value::Bool(false)));
                self.game.text(&text, [x, y], color & 0xFF_FFFF, shadow);
                none()
            }
            52 => {
                self.drawing(lua, "hud.rect")?;
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
                        "one of key.forward, key.left, key.back, key.right, key.jump, key.sneak, key.sprint, key.attack, key.use",
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
                let name = if id == 151 { "gfx.rect" } else { "gfx.border" };
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
                self.drawing(lua, "gfx.text")?;
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
            160 => {
                let v = self.game.via();
                let label = v
                    .versions
                    .iter()
                    .find(|(id, _)| *id == v.version)
                    .map_or(v.version.clone(), |(_, l)| l.clone());
                let mut t = Table::default();
                t.set_str("enabled", Value::Bool(v.enabled));
                t.set_str("version", Value::str(&v.version));
                t.set_str("label", Value::str(&label));
                t.set_str("text", Value::str(&v.status));
                t.set_str("ready", Value::Bool(v.ready));
                Ok(vec![Value::table(t)])
            }
            161 => {
                let mut list = Table::default();
                for (i, (id, label)) in self.game.via().versions.iter().enumerate() {
                    let mut t = Table::default();
                    t.set_str("id", Value::str(id));
                    t.set_str("label", Value::str(label));
                    let _ = list.set(Value::Num(i as f64 + 1.0), Value::table(t));
                }
                Ok(vec![Value::table(list)])
            }
            162 => {
                let on = arg(&args, 0).truthy();
                self.game.via_set(Some(on), None).map_err(|e| lua.rt_error(e))?;
                none()
            }
            163 => {
                let id = check_text(lua, &args, 0, "version")?;
                self.game.via_set(None, Some(&id)).map_err(|e| lua.rt_error(e))?;
                none()
            }
            164 => {
                let mut list = Table::default();
                for (i, line) in self.game.via().log.iter().rev().take(40).rev().enumerate() {
                    let _ = list.set(Value::Num(i as f64 + 1.0), Value::str(line));
                }
                Ok(vec![Value::table(list)])
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
            116 => self.draw_item(lua, &args),
            117 => self.draw_mesh(lua, &args),
            118 => self.draw_triangle(lua, &args),
            170..=174 => self.model(lua, id, &args),
            180..=188 => self.texture(lua, id, &args),
            190..=194 => self.mesh(lua, id, &args),
            175 => Ok(vec![match self.game.pose() {
                None => Value::Nil,
                Some(p) => {
                    let mut t = Table::new();
                    t.set_str("pose", Value::str(&p.pose));
                    for (k, v) in [
                        ("crouching", p.pose == "crouching"),
                        ("swimming", p.pose == "swimming"),
                        ("gliding", p.pose == "gliding"),
                        ("sleeping", p.pose == "sleeping"),
                        ("sprinting", p.sprinting),
                        ("on_ground", p.on_ground),
                        ("flying", p.flying),
                    ] {
                        t.set_str(k, Value::Bool(v));
                    }
                    t.set_str("camera", Value::str(&p.camera));
                    Value::table(t)
                }
            }]),
            176 => {
                let part = check_text(lua, &args, 0, "part")?;
                let target = super::hstudio_models::target_of(lua, &arg(&args, 1), "player.part")?.flatten();
                super::hstudio_models::check_part(lua, &part, target, "part")?;
                Ok(vec![
                    self.game
                        .part_frame(target, &part)
                        .map_or(Value::Nil, |f| super::hstudio_models::frame_table(&f)),
                ])
            }
            177 => Ok(vec![self.game.pose().map_or(Value::Nil, |p| Value::str(&p.camera))]),
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
                let all = API
                    .iter()
                    .map(|(table, name, _, doc)| {
                        let full = if table.is_empty() {
                            (*name).to_owned()
                        } else {
                            format!("{table}.{name}")
                        };
                        (full, *doc)
                    })
                    .chain(CONSOLE_API.iter().map(|(name, _, doc)| ((*name).to_owned(), *doc)));
                let all: Vec<(String, &str)> = all.collect();
                // A function's own name gives that one; else every name with it in.
                let lines: Vec<String> = match want.as_deref() {
                    Some(w) if all.iter().any(|(full, _)| full == w) => all
                        .iter()
                        .filter(|(full, _)| full == w)
                        .map(|(_, d)| (*d).to_owned())
                        .collect(),
                    Some(w) => all
                        .iter()
                        .filter(|(full, _)| full.contains(w))
                        .map(|(_, d)| (*d).to_owned())
                        .collect(),
                    None => all.iter().map(|(_, d)| (*d).to_owned()).collect(),
                };
                let mut lines = lines;
                if lines.is_empty() {
                    lines.push("nothing by that name".into());
                }
                self.s.log(LogKind::Info, lines.join("\n"));
                none()
            }
            201 => {
                self.s.log.clear();
                self.s.console_mod.clear();
                self.s.rbx.forget(Owner::Console);
                self.s.console_mod.state = ModState::Running;
                none()
            }
            202 | 204 => {
                // The studio does these once the call returns: they need the interpreter.
                let fname = if id == 202 { "reload" } else { "edit" };
                let which = opt_str(lua, &args, 0, fname)?.map(|s| text_of(&s));
                let found = match &which {
                    Some(w) => Some(
                        self.s
                            .mods
                            .iter()
                            .position(|m| &m.id == w)
                            .ok_or_else(|| lua.rt_error(format!("no script mod '{w}'")))?,
                    ),
                    None => None,
                };
                let request = match (which, found) {
                    (which, _) if id == 202 => Request::Reload(which),
                    (Some(w), Some(i)) => {
                        let file = match opt_str(lua, &args, 1, "edit")?.map(|s| text_of(&s)) {
                            None => None,
                            Some(f) => {
                                let m = &self.s.mods[i];
                                let Some(folder) = &m.folder else {
                                    return Err(
                                        lua.rt_error(format!("'{w}' is a script, one file: edit(\"{w}\") opens it"))
                                    );
                                };
                                let rel = file_name(&f).map_err(|e| bad_arg(lua, 1, "edit", &e))?;
                                if !path_in(folder, &rel).is_file() {
                                    return Err(
                                        lua.rt_error(format!("no {rel} in {w} (it has {})", m.files().join(", ")))
                                    );
                                }
                                Some(rel)
                            }
                        };
                        Request::Edit(w, file)
                    }
                    _ => return Err(bad_arg(lua, 0, "edit", "a mod id expected")),
                };
                self.s.requests.push(request);
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
                let module = match opt_str(lua, &args, 1, "new")?.map(|s| text_of(&s)).as_deref() {
                    None | Some("script") => false,
                    Some("module") => true,
                    Some(_) => return Err(bad_arg(lua, 1, "new", "\"script\" or \"module\" expected")),
                };
                self.s.requests.push(Request::New(text_of(&name), module));
                none()
            }
            300..=312 => self.task(lua, id, args),
            super::hstudio_rbx::FIRST_ID..=super::hstudio_rbx::LAST_ID => super::hstudio_rbx::call(self, lua, id, args),
            super::hstudio_sunc::FIRST_ID..=super::hstudio_sunc::LAST_ID => self.sunc(lua, id, args),
            _ => Err(lua.rt_error(format!("unknown studio function {id}"))),
        }
    }
}
