//! Script mods without Horizon: the studio itself (its interpreter, mod
//! loading, events, commands, timers, libraries and screens) on a simulated
//! game, for the MCP server's `check` and `test`. The game is simple: a player
//! standing in a world of air with the blocks a test sets, a clock, 20 ticks a
//! second (a frame is one tick, 50 ms), and a record of everything the mods
//! print, send, draw, play and spawn.

use crate::game::hstudio::{
    Camera, Corner, Font, GameApi, LogKind, PartFrame, PlayerInfo, PlayerPose, Reply, Studio, ViaInfo,
};
use crate::game::hstudio_examples::EXAMPLES;
use crate::game::hstudio_link::mods as mods_json;
use crate::game::hstudio_ui::{Input, handle_screen};
use serde_json::{Value as Json, json};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A frame's length: one game tick.
pub const FRAME_SECONDS: f64 = 0.05;
/// The most frames one test may run.
pub const MAX_FRAMES: u64 = 20 * 60 * 10;
const GUI: (f32, f32) = (480.0, 270.0);
/// dp per GUI pixel (a GUI scale of 2 on a 960 x 540 window).
const DP: f32 = 2.0;

/// Text drawn this frame: what, where (GUI pixels) and how wide.
#[derive(Clone, Debug)]
pub struct Drawn {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

pub struct HeadlessGame {
    pub dir: PathBuf,
    pub chats: Vec<String>,
    pub sent: Vec<String>,
    pub player: Option<PlayerInfo>,
    pub blocks: HashMap<(i32, i32, i32), String>,
    pub clock: Option<(i64, i64)>,
    pub ticks: u64,
    pub screen: String,
    pub held: HashMap<String, bool>,
    pub clipboard: String,
    pub drawn: Vec<Drawn>,
    pub sounds: Vec<String>,
    pub particles: Vec<String>,
    pub world_quads: usize,
    /// Items drawn as models: (id, where, display).
    pub items: Vec<(String, [f64; 3], String)>,
    /// "first", "third_back" or "third_front".
    pub camera_mode: String,
    /// The player's pose ("standing", "crouching"...).
    pub pose: String,
    /// How many of `sent` reached the mods' commands.
    routed: usize,
}

impl HeadlessGame {
    pub fn new(dir: PathBuf) -> HeadlessGame {
        HeadlessGame {
            dir,
            chats: Vec::new(),
            sent: Vec::new(),
            player: Some(PlayerInfo {
                x: 0.5,
                y: 64.0,
                z: 0.5,
                yaw: 0.0,
                pitch: 0.0,
                dimension: "minecraft:overworld".into(),
                health: 20.0,
                max_health: 20.0,
                food: 20,
                saturation: 5.0,
                level: 0,
                mode: "survival".into(),
                slot: 0,
                held: None,
            }),
            blocks: HashMap::new(),
            clock: Some((1000, 1000)),
            ticks: 0,
            screen: "playing".into(),
            held: HashMap::new(),
            clipboard: String::new(),
            drawn: Vec::new(),
            sounds: Vec::new(),
            particles: Vec::new(),
            world_quads: 0,
            items: Vec::new(),
            camera_mode: "third_back".into(),
            pose: "standing".into(),
            routed: 0,
        }
    }
    /// The player's body parts, standing still: the player model's pivots
    /// (pixels: x to its left, y down from the neck), 15/16 of a block-sized model.
    fn player_part(&self, part: &str) -> Option<PartFrame> {
        let p = self.player.as_ref()?;
        let pivot: [f64; 3] = match part {
            "root" | "head" | "hat" | "body" | "jacket" => [0.0, 0.0, 0.0],
            "right_arm" | "right_sleeve" => [-5.0, 2.0, 0.0],
            "left_arm" | "left_sleeve" => [5.0, 2.0, 0.0],
            "right_leg" | "right_pants" => [-1.9, 12.0, 0.0],
            "left_leg" | "left_pants" => [1.9, 12.0, 0.0],
            _ => return None,
        };
        let k = 0.9375;
        let yaw = f64::from(p.yaw).to_radians();
        let left = [yaw.cos() * k, 0.0, yaw.sin() * k];
        let up = [0.0, k, 0.0];
        let forward = [-yaw.sin() * k, 0.0, yaw.cos() * k];
        let crouch = if self.pose == "crouching" { 0.2 } else { 0.0 };
        let px = |v: [f64; 3], n: f64| [v[0] * n / 16.0, v[1] * n / 16.0, v[2] * n / 16.0];
        let mut origin = [p.x, p.y - crouch, p.z];
        for d in [px(left, pivot[0]), px(up, 24.0 - pivot[1]), px(forward, -pivot[2])] {
            origin = [origin[0] + d[0], origin[1] + d[1], origin[2] + d[2]];
        }
        Some(PartFrame {
            origin,
            left,
            up,
            forward,
        })
    }
    fn camera_at(&self) -> Option<Camera> {
        let p = self.player.as_ref()?;
        Some(Camera {
            pos: [p.x, p.y + 1.62, p.z],
            yaw: p.yaw,
            pitch: p.pitch,
            fov: 70.0,
        })
    }
    fn width(text: &str) -> f32 {
        6.0 * text.chars().count() as f32
    }
}

impl GameApi for HeadlessGame {
    fn chat(&mut self, text: &str) {
        self.chats.push(text.to_owned());
    }
    fn send(&mut self, line: &str) {
        self.sent.push(line.to_owned());
    }
    fn player(&mut self) -> Option<PlayerInfo> {
        self.player.clone()
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        self.player.as_ref()?;
        Some(
            self.blocks
                .get(&(x, y, z))
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
    fn clock(&self) -> Option<(i64, i64)> {
        self.player.as_ref()?;
        self.clock
    }
    fn ticks(&self) -> u64 {
        self.ticks
    }
    fn fps(&self) -> [u32; 3] {
        [20, 20, 20]
    }
    fn mapping(&mut self, name: &str) -> Option<(bool, String)> {
        name.starts_with("key.").then(|| {
            (
                self.held.get(name).copied().unwrap_or(false),
                name.trim_start_matches("key.").into(),
            )
        })
    }
    fn screen(&self) -> String {
        self.screen.clone()
    }
    fn agent(&mut self, line: &str) -> Result<Reply, String> {
        let name = line.split_whitespace().next().unwrap_or("");
        Err(format!(
            "Horizon's agent command '{name}' needs the game: test this part live (the game running with the studio)"
        ))
    }
    fn data_dir(&self) -> PathBuf {
        self.dir.clone()
    }
    fn clipboard(&mut self) -> String {
        self.clipboard.clone()
    }
    fn set_clipboard(&mut self, text: &str) {
        self.clipboard = text.to_owned();
    }
    fn gui_size(&self) -> (f32, f32) {
        GUI
    }
    fn text_width(&self, text: &str) -> f32 {
        Self::width(text)
    }
    fn fill(&mut self, _rect: [f32; 4], _argb: u32) {}
    fn text(&mut self, text: &str, at: [f32; 2], _rgb: u32, _shadow: bool) {
        self.drawn.push(Drawn {
            text: text.to_owned(),
            x: at[0],
            y: at[1],
            w: Self::width(text),
        });
    }
    fn place(&mut self, corner: Corner, w: f32, h: f32) -> (f32, f32) {
        match corner {
            Corner::TopLeft => (2.0, 2.0),
            Corner::TopRight => (GUI.0 - w - 2.0, 2.0),
            Corner::BottomLeft => (2.0, GUI.1 - h - 2.0),
            Corner::BottomRight => (GUI.0 - w - 2.0, GUI.1 - h - 2.0),
        }
    }
    fn open_folder(&mut self, _path: &Path) {}
    fn camera(&self) -> Option<Camera> {
        self.camera_at()
    }
    fn world_quad(&mut self, _corners: [[f64; 3]; 4], _argb: u32) {
        self.world_quads += 1;
    }
    fn quad(&mut self, _corners: [[f32; 2]; 4], _argb: u32) {}
    fn sound(&mut self, id: &str, _at: Option<[f64; 3]>, _volume: f32, _pitch: f32) -> Result<(), String> {
        self.sounds
            .push(resource_id(id, "sound", "minecraft:block.note_block.bell")?);
        Ok(())
    }
    fn particle(
        &mut self,
        id: &str,
        _at: [f64; 3],
        _velocity: [f64; 3],
        _color: Option<[f32; 4]>,
    ) -> Result<(), String> {
        self.particles.push(resource_id(id, "particle", "minecraft:flame")?);
        Ok(())
    }
    fn dp(&mut self) -> (f32, f32, f32) {
        (GUI.0 * DP, GUI.1 * DP, DP)
    }
    fn dp_rect(&mut self, _r: [f32; 4], _radius: f32, _argb: u32) {}
    fn dp_border(&mut self, _r: [f32; 4], _radius: f32, _argb: u32) {}
    fn dp_text(&mut self, text: &str, at: [f32; 2], size: f32, _font: Font, _argb: u32) -> f32 {
        let w = text.chars().count() as f32 * size * 0.55;
        self.drawn.push(Drawn {
            text: text.to_owned(),
            x: at[0] / DP,
            y: at[1] / DP,
            w: w / DP,
        });
        w
    }
    fn dp_measure(&mut self, text: &str, size: f32, _font: Font) -> f32 {
        text.chars().count() as f32 * size * 0.55
    }
    fn via(&self) -> ViaInfo {
        ViaInfo {
            status: "Not in a headless test".into(),
            ..ViaInfo::default()
        }
    }
    fn part_frame(&self, target: Option<u32>, part: &str) -> Option<PartFrame> {
        match target {
            None => self.player_part(part),
            // No mobs in the headless world.
            Some(_) => None,
        }
    }
    fn body_shown(&self) -> bool {
        self.camera_mode != "first"
    }
    fn pose(&self) -> Option<PlayerPose> {
        self.player.as_ref()?;
        Some(PlayerPose {
            pose: self.pose.clone(),
            sprinting: false,
            on_ground: true,
            flying: false,
            camera: self.camera_mode.clone(),
        })
    }
    fn world_item(
        &mut self,
        id: &str,
        m: [[f64; 3]; 4],
        display: &str,
        _light_at: Option<[f64; 3]>,
    ) -> Result<(), String> {
        let id = resource_id(id, "item", "minecraft:diamond_sword")?;
        self.items.push((id, m[3], display.to_owned()));
        Ok(())
    }
    fn world_quad_lit(&mut self, _corners: [[f64; 3]; 4], _argb: u32, _normal: [f64; 3], _light_at: Option<[f64; 3]>) {
        self.world_quads += 1;
    }
}

/// A sound or particle id as the game takes it (`flame` is `minecraft:flame`);
/// the headless game can't tell which exist, only which can't.
fn resource_id(id: &str, what: &str, like: &str) -> Result<String, String> {
    let ok = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-./:".contains(c))
        && id.matches(':').count() <= 1;
    if !ok {
        return Err(format!("'{id}' is not a {what} id (they look like {like})"));
    }
    Ok(if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    })
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// A studio on a headless game, in a folder of its own (removed after).
pub struct Run {
    pub studio: Studio,
    pub game: HeadlessGame,
    keep: bool,
}

impl Drop for Run {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.game.dir);
        }
    }
}

/// Where a mod's files are: a folder with main.lua, or one .lua file.
pub enum Source {
    /// A mod in a mods folder, by id (and its libraries from there).
    Installed { mods_dir: PathBuf, id: String },
    /// Files given as text: `main.lua` (and others) for a folder mod, or one
    /// file named `<id>.lua`; the libraries it imports come from `libraries`
    /// (a mods folder), else the bundled ones.
    Files {
        id: String,
        files: Vec<(String, String)>,
        libraries: Option<PathBuf>,
    },
}

/// A mod's files as (path in the mod, text).
pub fn read_mod(mods_dir: &Path, id: &str) -> Result<Vec<(String, String)>, String> {
    let folder = mods_dir.join(id);
    if folder.join("main.lua").is_file() {
        let mut files = Vec::new();
        let mut stack = vec![(folder.clone(), 0)];
        while let Some((dir, depth)) = stack.pop() {
            let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && depth < 3 {
                    stack.push((p, depth + 1));
                } else if p.is_file()
                    && p.metadata().map_or(0, |m| m.len()) <= 1 << 20
                    && let (Ok(rel), Ok(text)) = (p.strip_prefix(&folder), std::fs::read_to_string(&p))
                {
                    files.push((rel.to_string_lossy().replace('\\', "/"), text));
                }
            }
        }
        files.sort();
        return Ok(files);
    }
    let file = mods_dir.join(format!("{id}.lua"));
    match std::fs::read_to_string(&file) {
        Ok(text) => Ok(vec![(format!("{id}.lua"), text)]),
        Err(_) => Err(format!("no mod '{id}' in {}", mods_dir.display())),
    }
}

/// Writes a mod's files into a mods folder: a folder mod when there is a
/// `main.lua`, else the single file.
pub fn write_mod(mods_dir: &Path, id: &str, files: &[(String, String)]) -> Result<PathBuf, String> {
    check_id(id)?;
    let folder = files.iter().any(|(p, _)| p == "main.lua");
    if !folder {
        let [(name, text)] = files else {
            return Err("a folder mod needs a main.lua; a single-file mod is one file".into());
        };
        if name != &format!("{id}.lua") && name != "main.lua" {
            return Err(format!(
                "a single-file mod is {id}.lua (or give main.lua for a folder mod)"
            ));
        }
        std::fs::create_dir_all(mods_dir).map_err(|e| e.to_string())?;
        let path = mods_dir.join(format!("{id}.lua"));
        if mods_dir.join(id).is_dir() {
            return Err(format!("'{id}' is a folder mod already: write its main.lua"));
        }
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        return Ok(path);
    }
    if mods_dir.join(format!("{id}.lua")).is_file() {
        return Err(format!(
            "'{id}' is a single-file mod already ({id}.lua): delete it first to make it a folder"
        ));
    }
    let dir = mods_dir.join(id);
    for (rel, text) in files {
        let rel = safe_path(rel)?;
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(dir)
}

/// A mod id as the studio makes them: lowercase letters, digits, `_` and `-`.
pub fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "'{id}' is not a mod id: use lowercase letters, digits, _ and - (up to 64)"
        ))
    }
}

/// A relative path inside a mod: no `..`, no absolute paths, .lua or text.
fn safe_path(rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    let ok = !rel.is_empty()
        && p.components().all(|c| matches!(c, std::path::Component::Normal(_)))
        && p.components().count() <= 4;
    if ok {
        Ok(p.to_path_buf())
    } else {
        Err(format!("'{rel}' is not a path inside the mod"))
    }
}

/// The libraries a mod's files import (`import("id")`), by id.
fn imports_of(files: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (_, text) in files {
        for id in crate::game::hstudio::static_imports(text) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

impl Run {
    /// A run of `source` and the libraries it imports (from the same mods
    /// folder, else the bundled ones), loaded by the first frame.
    pub fn new(source: &Source) -> Result<Run, String> {
        let n = RUNS.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("horizon-studio-headless-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mods = dir.join("mods");
        std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
        // No welcome mod and no bundled libraries but those a mod imports.
        let seen: String = crate::game::hstudio::PREINSTALLED
            .iter()
            .map(|id| format!("seen:{id}\n"))
            .collect();
        std::fs::write(dir.join("studio.txt"), seen).map_err(|e| e.to_string())?;
        let (id, files, from) = match source {
            Source::Installed { mods_dir, id } => (id.clone(), read_mod(mods_dir, id)?, Some(mods_dir.clone())),
            Source::Files { id, files, libraries } => (id.clone(), files.clone(), libraries.clone()),
        };
        write_mod(&mods, &id, &files)?;
        let mut queue = imports_of(&files);
        let mut done: BTreeSet<String> = BTreeSet::from([id.clone()]);
        while let Some(lib) = queue.pop() {
            if !done.insert(lib.clone()) || check_id(&lib).is_err() {
                continue;
            }
            let lib_files = from.as_ref().and_then(|d| read_mod(d, &lib).ok()).or_else(|| {
                EXAMPLES
                    .iter()
                    .find(|e| e.id == lib)
                    .map(|e| vec![(format!("{lib}.lua"), e.source.to_owned())])
            });
            if let Some(lib_files) = lib_files {
                queue.extend(imports_of(&lib_files));
                write_mod(&mods, &lib, &lib_files)?;
            }
        }
        let mut run = Run {
            studio: Studio::new(),
            game: HeadlessGame::new(dir),
            keep: false,
        };
        run.frame();
        Ok(run)
    }

    /// One frame (one tick): the studio's frame, the commands mods sent,
    /// the world and the HUD, and a script's screen.
    pub fn frame(&mut self) {
        let (st, game) = (&mut self.studio, &mut self.game);
        game.drawn.clear();
        game.ticks += 1;
        if let Some((day, total)) = game.clock.as_mut() {
            *day += 1;
            *total += 1;
        }
        st.frame(game, FRAME_SECONDS);
        // A command a mod sent as the player reaches the mods' commands, as in the game.
        let new: Vec<String> = game.sent[game.routed..].to_vec();
        game.routed = game.sent.len();
        for line in new {
            if let Some(cmd) = line.strip_prefix('/')
                && !st.command(cmd)
            {
                game.chats.push(format!("(the game would run /{cmd})"));
            }
        }
        st.world(game);
        st.hud(game);
        if st.s.screen_request.is_some() {
            st.accept_screen();
            game.screen = "script".into();
        }
        if st.s.screen.is_some() {
            crate::game::hstudio_look::draw_screen(st, game);
        } else if game.screen == "script" {
            game.screen = "playing".into();
        }
    }

    pub fn frames(&mut self, n: u64) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A click on text drawn on the script's screen (its label).
    fn click(&mut self, label: &str) -> Result<(), String> {
        if self.studio.s.screen.is_none() {
            return Err("no script screen is open to click on".into());
        }
        let found = self
            .game
            .drawn
            .iter()
            .find(|d| d.text == label)
            .or_else(|| self.game.drawn.iter().find(|d| d.text.contains(label)))
            .cloned()
            .ok_or_else(|| {
                let shown: BTreeSet<&str> = self.game.drawn.iter().map(|d| d.text.as_str()).collect();
                format!("nothing labelled '{label}' on the screen (it shows: {shown:?})")
            })?;
        let (x, y) = (found.x + found.w.min(40.0) / 2.0, found.y + 4.0);
        let (st, game) = (&mut self.studio, &mut self.game);
        handle_screen(st, game, Input::Mouse(x, y));
        handle_screen(st, game, Input::Button { left: true, down: true });
        handle_screen(
            st,
            game,
            Input::Button {
                left: true,
                down: false,
            },
        );
        Ok(())
    }

    fn type_text(&mut self, text: &str) {
        if self.studio.s.screen.is_some() {
            handle_screen(&mut self.studio, &mut self.game, Input::Text(text.to_owned()));
        }
    }

    pub fn mods(&self) -> Json {
        mods_json(&self.studio)
    }
}

fn texts(v: Option<&Json>) -> Vec<String> {
    match v {
        Some(Json::String(s)) => vec![s.clone()],
        Some(Json::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    }
}

fn player_patch(p: &mut PlayerInfo, patch: &Json) {
    let num = |k: &str| patch.get(k).and_then(Json::as_f64);
    if let Some(v) = num("x") {
        p.x = v;
    }
    if let Some(v) = num("y") {
        p.y = v;
    }
    if let Some(v) = num("z") {
        p.z = v;
    }
    if let Some(v) = num("yaw") {
        p.yaw = v as f32;
    }
    if let Some(v) = num("pitch") {
        p.pitch = v as f32;
    }
    if let Some(v) = num("health") {
        p.health = v as f32;
    }
    if let Some(v) = num("food") {
        p.food = v as i32;
    }
    if let Some(v) = num("level") {
        p.level = v as i32;
    }
    if let Some(v) = patch.get("mode").and_then(Json::as_str) {
        p.mode = v.to_owned();
    }
    if let Some(v) = patch.get("dimension").and_then(Json::as_str) {
        p.dimension = v.to_owned();
    }
    match patch.get("held") {
        Some(Json::String(item)) => p.held = Some((item.clone(), 1)),
        Some(Json::Null) => p.held = None,
        _ => {}
    }
}

/// What one step does, in a few words, for the report.
fn describe(step: &Json) -> String {
    let mut s = step.to_string();
    if s.len() > 160 {
        let cut = (0..=157).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
        s.truncate(cut);
        s.push_str("...");
    }
    s
}

/// Runs test steps on a headless run and reports each (see `STEPS_HELP`).
pub fn run_steps(run: &mut Run, steps: &[Json]) -> Json {
    let mut reports = Vec::new();
    let mut frames_left = MAX_FRAMES;
    let mut all_ok = true;
    for (i, step) in steps.iter().enumerate() {
        let log_mark = run.studio.s.log_seq;
        let (chat_mark, sent_mark) = (run.game.chats.len(), run.game.sent.len());
        let (sound_mark, particle_mark) = (run.game.sounds.len(), run.game.particles.len());
        let item_mark = run.game.items.len();
        let mut failures: Vec<String> = Vec::new();
        let mut drawn: BTreeSet<String> = BTreeSet::new();
        let mut output: Vec<String> = Vec::new();
        let mut frames = |run: &mut Run, n: u64, drawn: &mut BTreeSet<String>| {
            let n = n.min(frames_left);
            frames_left -= n;
            for _ in 0..n {
                run.frame();
                drawn.extend(run.game.drawn.iter().map(|d| d.text.clone()));
            }
        };
        let expect_error = step
            .get("expect_error")
            .filter(|v| !matches!(v, Json::Bool(false)))
            .cloned();
        if let Some(n) = step.get("frames").and_then(Json::as_u64) {
            frames(run, n, &mut drawn);
        } else if let Some(s) = step.get("seconds").and_then(Json::as_f64) {
            frames(run, (s.max(0.0) * 20.0).round() as u64, &mut drawn);
        } else if let Some(cmd) = step.get("command").and_then(Json::as_str) {
            let cmd = cmd.trim().trim_start_matches('/');
            if !run.studio.command(cmd) {
                failures.push(format!(
                    "no mod has the command /{}",
                    cmd.split_whitespace().next().unwrap_or("")
                ));
            }
            frames(run, 1, &mut drawn);
        } else if let Some(text) = step.get("chat").and_then(Json::as_str) {
            run.studio.chat_line(text);
            frames(run, 1, &mut drawn);
        } else if let Some(key) = step.get("key").and_then(Json::as_str) {
            match step.get("down").and_then(Json::as_bool) {
                Some(down) => {
                    run.game.held.insert(key.to_owned(), down);
                    run.studio.key(key, down);
                    frames(run, 1, &mut drawn);
                }
                None => {
                    run.game.held.insert(key.to_owned(), true);
                    run.studio.key(key, true);
                    frames(run, 1, &mut drawn);
                    run.game.held.insert(key.to_owned(), false);
                    run.studio.key(key, false);
                    frames(run, 1, &mut drawn);
                }
            }
        } else if let Some(code) = step.get("lua").and_then(Json::as_str) {
            let lines = run.studio.run_chunk(&mut run.game, code, false);
            output = lines.iter().map(|(_, t)| t.clone()).collect();
        } else if let Some(patch) = step.get("player") {
            match (patch, run.game.player.as_mut()) {
                (Json::Null, _) => run.game.player = None,
                (patch, Some(p)) => player_patch(p, patch),
                (_, None) => failures.push(
                    "the player is gone (set \"player\": {...} after null to bring one back is not supported)".into(),
                ),
            }
            frames(run, 1, &mut drawn);
        } else if let Some(at) = step.get("block").and_then(Json::as_array) {
            let c: Vec<i32> = at.iter().filter_map(|v| v.as_f64().map(|f| f.floor() as i32)).collect();
            let id = step.get("id").and_then(Json::as_str).unwrap_or("minecraft:air");
            match c.as_slice() {
                [x, y, z] => {
                    run.game.blocks.insert((*x, *y, *z), id.to_owned());
                }
                _ => failures.push("\"block\" is [x, y, z]".into()),
            }
        } else if let Some(mode) = step.get("camera").and_then(Json::as_str) {
            if !["first", "third_back", "third_front"].contains(&mode) {
                failures.push("\"camera\" is first, third_back or third_front".into());
            }
            run.game.camera_mode = mode.to_owned();
            frames(run, 1, &mut drawn);
        } else if let Some(pose) = step.get("pose").and_then(Json::as_str) {
            run.game.pose = pose.to_owned();
            frames(run, 1, &mut drawn);
        } else if let Some(t) = step.get("time").and_then(Json::as_i64) {
            if let Some(c) = run.game.clock.as_mut() {
                c.0 = t;
            }
        } else if let Some(label) = step.get("click").and_then(Json::as_str) {
            if let Err(e) = run.click(label) {
                failures.push(e);
            }
            frames(run, 1, &mut drawn);
        } else if let Some(text) = step.get("type").and_then(Json::as_str) {
            run.type_text(text);
            frames(run, 1, &mut drawn);
        } else if let Some(r) = step.get("reload") {
            match r.as_str() {
                Some(id) => match run.studio.s.mods.iter().position(|m| m.id == id) {
                    Some(i) => run.studio.load_and_dependents(&mut run.game, i),
                    None => failures.push(format!("no mod '{id}' in this test")),
                },
                None => run.studio.reload_all(&mut run.game),
            }
            frames(run, 1, &mut drawn);
        } else if step
            .as_object()
            .is_some_and(|o| o.keys().all(|k| k.starts_with("expect")))
        {
            // Expectations only: about the state as it is.
        } else {
            failures.push(format!(
                "unknown step {}: use frames, seconds, command, chat, key, lua, player, block, time, camera, pose, click, type or reload",
                describe(step)
            ));
        }
        drawn.extend(run.game.drawn.iter().map(|d| d.text.clone()));
        let chats: Vec<String> = run.game.chats[chat_mark..].to_vec();
        let sent: Vec<String> = run.game.sent[sent_mark..].to_vec();
        let sounds: Vec<String> = run.game.sounds[sound_mark..].to_vec();
        let particles: Vec<String> = run.game.particles[particle_mark..].to_vec();
        let mut items: Vec<String> = run.game.items[item_mark..]
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect();
        items.sort();
        items.dedup();
        let log: Vec<(LogKind, String)> = run
            .studio
            .log_since(log_mark)
            .filter(|(_, l)| l.kind != LogKind::Input)
            .map(|(_, l)| (l.kind, l.text.clone()))
            .collect();
        let errors: Vec<String> = log
            .iter()
            .filter(|(k, _)| *k == LogKind::Error)
            .map(|(_, t)| t.clone())
            .collect();
        match &expect_error {
            None if !errors.is_empty() => failures.push(format!("errors: {}", errors.join(" | "))),
            Some(Json::String(want)) if !errors.iter().any(|e| e.contains(want.as_str())) => {
                failures.push(format!("expected an error with '{want}', got {errors:?}"));
            }
            Some(Json::Bool(true)) if errors.is_empty() => failures.push("expected an error, none came".into()),
            _ => {}
        }
        let all_text = |v: &[String]| v.join("\n");
        for want in texts(step.get("expect_chat")) {
            if !chats.iter().any(|c| c.contains(&want)) {
                failures.push(format!("no chat line with '{want}' (chat: {chats:?})"));
            }
        }
        for not in texts(step.get("expect_no_chat")) {
            if chats.iter().any(|c| c.contains(&not)) {
                failures.push(format!("a chat line has '{not}'"));
            }
        }
        for want in texts(step.get("expect_output")) {
            if !all_text(&output).contains(&want) {
                failures.push(format!("the Lua output has no '{want}' (output: {output:?})"));
            }
        }
        for want in texts(step.get("expect_log")) {
            if !log.iter().any(|(_, t)| t.contains(&want)) {
                failures.push(format!("no console line with '{want}'"));
            }
        }
        for want in texts(step.get("expect_sent")) {
            if !sent.iter().any(|c| c.contains(&want)) {
                failures.push(format!("nothing sent with '{want}' (sent: {sent:?})"));
            }
        }
        for want in texts(step.get("expect_drawn")) {
            if !drawn.iter().any(|d| d.contains(&want)) {
                failures.push(format!("nothing drawn with '{want}' (drawn: {drawn:?})"));
            }
        }
        for want in texts(step.get("expect_sound")) {
            if !sounds.iter().any(|s| s.contains(&want)) {
                failures.push(format!("no sound like '{want}' (played: {sounds:?})"));
            }
        }
        for want in texts(step.get("expect_item")) {
            if !items.iter().any(|s| s.contains(&want)) {
                failures.push(format!("no item model like '{want}' drawn (drawn: {items:?})"));
            }
        }
        for want in texts(step.get("expect_particle")) {
            if !particles.iter().any(|s| s.contains(&want)) {
                failures.push(format!("no particle like '{want}' (spawned: {particles:?})"));
            }
        }
        if let Some(states) = step.get("expect_state").and_then(Json::as_object) {
            let mods = run.mods();
            for (id, want) in states {
                let got = mods
                    .as_array()
                    .and_then(|a| a.iter().find(|m| m["id"] == id.as_str()))
                    .map_or("missing".to_owned(), |m| m["state"].as_str().unwrap_or("").to_owned());
                if Some(got.as_str()) != want.as_str() {
                    failures.push(format!("{id} is {got}, not {want}"));
                }
            }
        }
        if step.get("expect_screen").and_then(Json::as_bool) == Some(true) && run.studio.s.screen.is_none() {
            failures.push("no script screen is open".into());
        }
        if step.get("expect_screen").and_then(Json::as_bool) == Some(false) && run.studio.s.screen.is_some() {
            failures.push("a script screen is still open".into());
        }
        let ok = failures.is_empty();
        all_ok &= ok;
        let mut report = json!({ "step": i + 1, "do": describe(step), "ok": ok });
        let r = report.as_object_mut().expect("an object");
        let mut put = |k: &str, v: Vec<String>| {
            if !v.is_empty() {
                r.insert(k.into(), json!(v.into_iter().take(60).collect::<Vec<_>>()));
            }
        };
        put("failures", failures);
        put("output", output);
        put("chat", chats);
        put("sent", sent);
        put(
            "log",
            log.into_iter()
                .map(|(k, t)| format!("{}: {t}", format!("{k:?}").to_lowercase()))
                .collect(),
        );
        put("drawn", drawn.into_iter().collect());
        put("sounds", sounds);
        put("particles", particles);
        put("items", items);
        reports.push(report);
        if frames_left == 0 {
            reports.push(json!({ "ok": false, "failures": ["the test ran out of frames (10 minutes of game time)"] }));
            all_ok = false;
            break;
        }
    }
    let passed = reports.iter().filter(|r| r["ok"] == true).count();
    json!({
        "ok": all_ok,
        "summary": format!("{passed}/{} steps passed", steps.len()),
        "steps": reports,
        "mods": run.mods(),
        "frames": MAX_FRAMES - frames_left,
    })
}

/// How to write test steps, for the MCP tools' descriptions.
pub const STEPS_HELP: &str = "Each step is an object with one action and any expectations. Actions: \
{\"frames\": N} or {\"seconds\": S} run the game (20 ticks a second); {\"command\": \"/name args\"} runs a chat command \
(a mod's own, or the studio's); {\"chat\": \"<Alex> hi\"} is a chat line arriving (on(\"chat\")); {\"key\": \"key.jump\"} \
presses and releases a key mapping (or with \"down\": true/false); {\"lua\": \"code\"} runs Lua in the console \
(globals; import(\"id\") gets a library; assert() fails the step); {\"player\": {\"x\":..,\"y\":..,\"z\":..,\"health\":..,\
\"mode\":\"creative\",\"held\":\"minecraft:stick\"}} moves or changes the player; {\"block\": [x,y,z], \"id\": \
\"minecraft:stone\"} sets a block (live: not available); {\"time\": 13000} sets the day time; {\"click\": \"label\"} \
clicks text on the open script screen; {\"type\": \"text\"} types into it; {\"reload\": \"id\"} (or true) reloads; \
{\"camera\": \"first\"|\"third_back\"|\"third_front\"} and {\"pose\": \"crouching\"} set the view and the player's pose \
(headless). \
Expectations (strings or lists of them, matched as substrings): expect_chat, expect_no_chat, expect_output (the lua \
step's printed values), expect_log, expect_sent, expect_drawn (text drawn on the HUD or screen during the step), \
expect_sound, expect_particle, expect_item (item models drawn: model.attach, draw.item); expect_state {\"id\": \"running\"|\"failed\"|\"off\"}; expect_screen true/false; \
expect_error true or \"text\" (otherwise any error logged during a step fails it). A step of expectations alone \
checks the state as it is.";
