//! Script mods without Horizon: the studio itself (its interpreter, mod
//! loading, events, commands, timers, libraries and screens) on a simulated
//! game, for the MCP server's `check` and `test`. The game is simple: a player
//! standing in a world of air with the blocks, entities and other players a
//! test sets, a clock, 20 ticks a second (a frame is one tick, 50 ms), and a
//! record of everything the mods print, send, draw, play and spawn.

use crate::game::hstudio::{
    Camera, Corner, EntityInfo, Font, GameApi, LogKind, PartFrame, PlayerEntry, PlayerInfo, PlayerPose, Reply, Studio,
    ViaInfo,
};
use crate::game::hstudio_chatcmd::split_line;
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
    /// Meshes drawn (draw.mesh, model.attach with a mesh): their triangles,
    /// the texture's size if they had one, a corner of the first.
    pub meshes: Vec<MeshDrawn>,
    /// "first", "third_back" or "third_front".
    pub camera_mode: String,
    /// The player's pose ("standing", "crouching"...).
    pub pose: String,
    /// Whether the player stands on the ground (a jump or a fall: not).
    pub on_ground: bool,
    /// The entities near the player (an `entities` step), and the server's
    /// player list (a `players` step; None, as a world played alone has).
    pub entities: Vec<EntityInfo>,
    pub players: Option<Vec<PlayerEntry>>,
    /// How many of `sent` the frames have noted.
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
                name: String::new(),
                uuid: 0,
                velocity: [0.0; 3],
                speed: Some(0.1),
                jump: Some(0.42),
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
            meshes: Vec::new(),
            camera_mode: "third_back".into(),
            pose: "standing".into(),
            on_ground: true,
            entities: Vec::new(),
            players: None,
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
            on_ground: self.on_ground,
            flying: false,
            camera: self.camera_mode.clone(),
        })
    }
    fn entities(&mut self, max: usize) -> Vec<EntityInfo> {
        // As the game lists them: the other players first, then the nearest.
        let Some(p) = self.player.as_ref() else {
            return Vec::new();
        };
        let far = |e: &EntityInfo| (e.pos[0] - p.x).powi(2) + (e.pos[1] - p.y).powi(2) + (e.pos[2] - p.z).powi(2);
        let mut list = self.entities.clone();
        list.sort_by(|a, b| {
            (b.kind == "minecraft:player")
                .cmp(&(a.kind == "minecraft:player"))
                .then(far(a).total_cmp(&far(b)))
        });
        list.truncate(max);
        list
    }
    fn players(&mut self) -> Option<Vec<PlayerEntry>> {
        self.player.as_ref()?;
        self.players.clone()
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
    fn world_mesh(
        &mut self,
        tris: &[crate::game::hstudio::WorldTri],
        texture: Option<crate::game::hstudio::TexRef<'_>>,
        _light_at: Option<[f64; 3]>,
    ) {
        self.world_quads += tris.len();
        let first = tris.first().map_or([0.0; 3], |t| t.p[0]);
        self.meshes.push((tris.len(), texture.map(|t| (t.w, t.h)), first));
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
    /// A mod in a mods folder, by the id the studio lists it under (and its
    /// libraries from there), run as it is on disk.
    Installed { mods_dir: PathBuf, id: String },
    /// Files given as text: `main.lua` (and others) for a folder mod, or one
    /// file named `<id>.lua`; the libraries it imports come from `libraries`
    /// (a mods folder), else the bundled ones. For a module that folder has
    /// under that id, the files given go over its own and `remove` (paths in
    /// it) are taken away, as writing them would leave it on disk.
    Files {
        id: String,
        files: Vec<(String, String)>,
        libraries: Option<PathBuf>,
        remove: Vec<String>,
    },
}

pub use crate::game::hstudio_modfiles::find_mod;

/// The mod a mods folder has under `id`: the one the studio lists
/// (`find_mod`), else one named `<id>/` or `<id>.lua` itself (an id longer
/// than the studio's, which it lists cut short): its main file, and its
/// folder for a module.
fn locate(mods_dir: &Path, id: &str) -> Option<(PathBuf, Option<PathBuf>)> {
    find_mod(mods_dir, id).or_else(|| {
        let folder = mods_dir.join(id);
        let script = mods_dir.join(format!("{id}.lua"));
        if folder.join("main.lua").is_file() {
            Some((folder.join("main.lua"), Some(folder)))
        } else {
            script.is_file().then_some((script, None))
        }
    })
}

/// A mod's files as (path in the mod, text): the mod the studio has under
/// that id, whatever its file is called (a script's file is `<id>.lua`).
pub fn read_mod(mods_dir: &Path, id: &str) -> Result<Vec<(String, String)>, String> {
    let none = || format!("no mod '{id}' in {}", mods_dir.display());
    let (main, folder) = locate(mods_dir, id).ok_or_else(none)?;
    if let Some(folder) = folder {
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
    let bytes = std::fs::read(&main).map_err(|_| none())?;
    let text = String::from_utf8(bytes).map_err(|_| {
        let name = main
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        format!("{name} is not UTF-8 text, which read_mod gives (check_mod and test_mod run it as the game does)")
    })?;
    Ok(vec![(format!("{id}.lua"), text)])
}

/// What `write_mod` wrote: the mod's file, or its module's folder, and for
/// a script it made a module, where the script's file went.
#[derive(Debug)]
pub struct Written {
    pub path: PathBuf,
    pub old_script: Option<OldScript>,
}

/// Where the file of a script made a module went.
#[derive(Debug, PartialEq)]
pub enum OldScript {
    /// To the studio's trash, at this path: the mods folder is the studio's.
    Trashed(PathBuf),
    /// Deleted for good, as delete_mod deletes a mod: the mods folder is in
    /// a folder that is not the studio's, which gets no trash of its own.
    Deleted(PathBuf),
}

/// Writes a mod's files into a mods folder: a module when there is a
/// `main.lua` or the module exists already (then only the files given are
/// written, the others kept), else the single file of a script. A
/// `main.lua` for an existing script makes it a module: once the module is
/// written, the script's file goes to the studio's trash when the mods
/// folder is the studio's (its folder has the studio.txt the game writes),
/// and is deleted otherwise. Every path is checked before anything on disk
/// changes, and a module's files are written beside it first, so a path
/// refused or a write that fails leaves the mod as it was. An id that
/// another file of the mods folder has ("Night Light.lua" is
/// night_light's) is refused (`check_holder`).
pub fn write_mod(mods_dir: &Path, id: &str, files: &[(String, String)]) -> Result<Written, String> {
    check_id(id)?;
    check_holder(mods_dir, id)?;
    let dir = mods_dir.join(id);
    let script = mods_dir.join(format!("{id}.lua"));
    let exists = dir.join("main.lua").is_file();
    if !exists && !files.iter().any(|(p, _)| p == "main.lua") {
        let [(name, text)] = files else {
            return Err("a folder mod needs a main.lua; a single-file mod is one file".into());
        };
        if *name != format!("{id}.lua") {
            return Err(format!(
                "a single-file mod is {id}.lua (or give main.lua for a folder mod)"
            ));
        }
        if dir.is_dir() {
            return Err(format!("'{id}' is a folder mod already: write its main.lua"));
        }
        std::fs::create_dir_all(mods_dir).map_err(|e| e.to_string())?;
        std::fs::write(&script, text).map_err(|e| e.to_string())?;
        return Ok(Written {
            path: script,
            old_script: None,
        });
    }
    if exists && matches!(files, [(name, _)] if *name == format!("{id}.lua")) {
        return Err(format!(
            "'{id}' is a module: write its files by their paths in it (main.lua, lib.lua...)"
        ));
    }
    let paths = module_paths(&dir, files)?;
    // Written in a hidden folder beside it, then moved in with main.lua
    // last: a new module is one only once its other files are there.
    let stage = mods_dir.join(format!(".{id}.writing"));
    let _ = std::fs::remove_dir_all(&stage);
    let failed = |rel: &Path, e: std::io::Error| format!("{}: {e}", dir.join(rel).display());
    let staged = paths.iter().zip(files).try_for_each(|(rel, (_, text))| {
        let path = stage.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or(&stage))
            .and_then(|()| std::fs::write(&path, text))
            .map_err(|e| failed(rel, e))
    });
    let mut order: Vec<&PathBuf> = paths.iter().collect();
    order.sort_by_key(|rel| rel.as_path() == Path::new("main.lua"));
    let moved = staged.and_then(|()| {
        order.into_iter().try_for_each(|rel| {
            let to = dir.join(rel);
            std::fs::create_dir_all(to.parent().unwrap_or(&dir))
                .and_then(|()| std::fs::rename(stage.join(rel), &to))
                .map_err(|e| failed(rel, e))
        })
    });
    let _ = std::fs::remove_dir_all(&stage);
    moved?;
    let mut old_script = None;
    if !exists && script.is_file() {
        let left = |e: String| format!("{id} is a module now, but {id}.lua is still beside it: {e}");
        old_script = Some(match mods_dir.parent().filter(|d| d.join("studio.txt").is_file()) {
            Some(studio) => OldScript::Trashed(
                crate::game::hstudio_modfiles::to_trash(&studio.join("trash"), mods_dir, &script).map_err(left)?,
            ),
            None => {
                std::fs::remove_file(&script).map_err(|e| left(e.to_string()))?;
                OldScript::Deleted(script)
            }
        });
    }
    Ok(Written { path: dir, old_script })
}

/// The paths of a module's files about to be written, all checked before
/// any is: each inside the mod, given once, and neither a folder of the
/// module on disk nor inside one of its files.
fn module_paths(dir: &Path, files: &[(String, String)]) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();
    for (rel, _) in files {
        let path = safe_path(rel)?;
        if out.contains(&path) {
            return Err(format!("'{rel}' is given twice"));
        }
        if dir.join(&path).is_dir() {
            return Err(format!("'{rel}' is a folder in the mod, not a file"));
        }
        if let Some(file) = path
            .ancestors()
            .skip(1)
            .find(|a| !a.as_os_str().is_empty() && dir.join(a).is_file())
        {
            return Err(format!("'{rel}' would be inside {}, a file of the mod", file.display()));
        }
        out.push(path);
    }
    Ok(out)
}

/// Refuses an id that another file or folder of the mods folder holds
/// ("Night Light.lua" is night_light's): the studio lists one mod an id, so
/// what is written or removed at `<id>.lua` or `<id>/` would be passed over
/// for that one. A name that is one of those on disk is the mod's own: where
/// the file system ignores case (macOS's, Windows'), "Lamp.lua" is lamp.lua.
pub fn check_holder(mods_dir: &Path, id: &str) -> Result<(), String> {
    let Some((main, folder)) = find_mod(mods_dir, id) else {
        return Ok(());
    };
    let held = folder.unwrap_or(main);
    if [mods_dir.join(id), mods_dir.join(format!("{id}.lua"))]
        .iter()
        .any(|own| same_file(&held, own))
    {
        return Ok(());
    }
    let name = held
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    Err(format!(
        "{id} is \"{name}\" in the mods folder: rename that file or choose another id"
    ))
}

/// Whether two paths name one file or folder: the same path, or two names
/// that reach one file on disk ("Lamp.lua" and "lamp.lua" where the file
/// system ignores case).
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((std::fs::metadata(a), std::fs::metadata(b)),
            (Ok(x), Ok(y)) if (x.dev(), x.ino()) == (y.dev(), y.ino()))
    }
    #[cfg(not(unix))]
    {
        matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
    }
}

/// Deletes mod `id` for good: the file or folder the studio lists under
/// that id, whatever it is called ("Night Light.lua" for night_light), as
/// list_mods, read_mod and check_mod take the id. Returns its path.
pub fn delete_mod(mods_dir: &Path, id: &str) -> Result<PathBuf, String> {
    check_id(id)?;
    let (main, folder) = locate(mods_dir, id).ok_or_else(|| format!("no mod '{id}' in {}", mods_dir.display()))?;
    let deleted = match folder {
        Some(folder) => std::fs::remove_dir_all(&folder).map(|()| folder),
        None => std::fs::remove_file(&main).map(|()| main),
    };
    deleted.map_err(|e| e.to_string())
}

/// Files of a module to delete, by their paths in it, as given: each must
/// be there, and main.lua, which starts the module, can't go (delete the
/// mod instead). Not for an id another file holds (`check_holder`).
pub fn removable(mods_dir: &Path, id: &str, paths: &[String]) -> Result<Vec<(String, PathBuf)>, String> {
    let dir = mods_dir.join(id);
    if !paths.is_empty() {
        check_holder(mods_dir, id)?;
    }
    if !paths.is_empty() && !dir.join("main.lua").is_file() {
        return Err(format!(
            "'{id}' is not a module: remove deletes files of a module (delete_mod deletes a mod)"
        ));
    }
    paths
        .iter()
        .map(|p| {
            let rel = p.replace('\\', "/");
            if rel == "main.lua" {
                return Err("main.lua starts the module: delete the mod with delete_mod instead".to_owned());
            }
            let path = dir.join(safe_path(&rel)?);
            if !path.is_file() {
                return Err(format!("{id} has no file {rel} to remove"));
            }
            Ok((rel, path))
        })
        .collect()
}

/// Deletes files of a module (for good, as delete_mod deletes a mod), and
/// the folders that leaves empty. Returns their paths in the module.
pub fn remove_files(mods_dir: &Path, id: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let found = removable(mods_dir, id, paths)?;
    let dir = mods_dir.join(id);
    for (_, path) in &found {
        std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
        crate::game::hstudio_modfiles::prune_empty(&dir, path.parent());
    }
    Ok(found.into_iter().map(|(rel, _)| rel).collect())
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

/// The code of mod `id` in a mods folder, as the game reads it: a script's
/// file, or every `.lua` file of a module, whatever its encoding or size.
fn code_of(mods: &Path, id: &str) -> Vec<(String, String)> {
    let folder = mods.join(id);
    let files = if folder.join("main.lua").is_file() {
        crate::game::hstudio_modfiles::lua_files(&folder)
            .into_iter()
            .map(|rel| (crate::game::hstudio_modfiles::path_in(&folder, &rel), rel))
            .collect()
    } else {
        vec![(mods.join(format!("{id}.lua")), format!("{id}.lua"))]
    };
    files
        .into_iter()
        .filter_map(|(path, rel)| Some((rel, String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned())))
        .collect()
}

/// Copies mod `id` of a mods folder (`from`, the mod the studio lists under
/// that id, whatever its name there) into a run's mods folder (`to`) under
/// its id, as the game reads it: a module's folder as it is (`copy_folder`:
/// code and assets, every byte), a script's file. False when `from` has no
/// such mod.
fn copy_mod(from: &Path, id: &str, to: &Path) -> Result<bool, String> {
    match locate(from, id) {
        Some((_, Some(folder))) => copy_folder(&folder, &to.join(id)),
        Some((main, None)) => {
            std::fs::copy(&main, to.join(format!("{id}.lua"))).map_err(|e| format!("{}: {e}", main.display()))?;
        }
        None => return Ok(false),
    }
    Ok(true)
}

impl Run {
    /// A run of `source` and the libraries it imports (from the same mods
    /// folder, else the bundled ones), loaded by the first frame.
    pub fn new(source: &Source) -> Result<Run, String> {
        let n = RUNS.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("horizon-studio-headless-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mods = dir.join("mods");
        // Made first, so a mod refused below takes its folder away with it.
        let mut run = Run {
            studio: Studio::new(),
            game: HeadlessGame::new(dir.clone()),
            keep: false,
        };
        std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
        // No welcome mod and no bundled libraries but those a mod imports.
        let seen: String = crate::game::hstudio::PREINSTALLED
            .iter()
            .map(|id| format!("seen:{id}\n"))
            .collect();
        std::fs::write(dir.join("studio.txt"), seen).map_err(|e| e.to_string())?;
        // A mod is run as it is in its mods folder, found as the studio
        // finds it ("Desk Lamp/" is desk_lamp), every byte of it.
        let (id, from) = match source {
            Source::Installed { mods_dir, id } => {
                check_id(id)?;
                if !copy_mod(mods_dir, id, &mods)? {
                    return Err(format!("no mod '{id}' in {}", mods_dir.display()));
                }
                (id.clone(), Some(mods_dir.clone()))
            }
            Source::Files {
                id,
                files,
                libraries,
                remove,
            } => {
                // A module on disk: its files as they are there, with those
                // given over them (a script's one file is all of it).
                let script = matches!(files.as_slice(), [(name, _)] if *name == format!("{id}.lua"));
                let module = libraries
                    .as_ref()
                    .filter(|_| !script)
                    .and_then(|from| locate(from, id))
                    .and_then(|(_, folder)| folder);
                if let Some(folder) = module {
                    copy_folder(&folder, &mods.join(id));
                }
                if !files.is_empty() {
                    write_mod(&mods, id, files)?;
                }
                for rel in remove {
                    if let Ok(rel) = safe_path(&rel.replace('\\', "/")) {
                        let _ = std::fs::remove_file(mods.join(id).join(rel));
                    }
                }
                (id.clone(), libraries.clone())
            }
        };
        let mut queue = imports_of(&code_of(&mods, &id));
        let mut done: BTreeSet<String> = BTreeSet::from([id.clone()]);
        while let Some(lib) = queue.pop() {
            if !done.insert(lib.clone()) || check_id(&lib).is_err() {
                continue;
            }
            // From the same mods folder as it is there, else a bundled one.
            let copied = match &from {
                Some(from) => copy_mod(from, &lib, &mods)?,
                None => false,
            };
            if !copied {
                let Some(example) = EXAMPLES.iter().find(|e| e.id == lib) else {
                    continue;
                };
                std::fs::write(mods.join(format!("{lib}.lua")), example.source).map_err(|e| e.to_string())?;
            }
            queue.extend(imports_of(&code_of(&mods, &lib)));
        }
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
        // A command a mod sent as the player goes to the server, as in the
        // game (one the studio takes ran in it instead): it is noted.
        let new: Vec<String> = game.sent[game.routed..].to_vec();
        game.routed = game.sent.len();
        for line in new {
            if let Some(cmd) = line.strip_prefix('/') {
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
    if let Some(v) = patch.get("name").and_then(Json::as_str) {
        p.name = v.to_owned();
    }
    // A UUID as it is written, its dashes or not.
    if let Some(v) = patch.get("uuid").and_then(Json::as_str)
        && let Ok(uuid) = u128::from_str_radix(&v.replace('-', ""), 16)
    {
        p.uuid = uuid;
    }
    // How the player moves, in blocks a second.
    if let Some(Json::Array(v)) = patch.get("velocity") {
        let n: Vec<f64> = v.iter().filter_map(Json::as_f64).collect();
        if let [x, y, z] = n[..] {
            p.velocity = [x, y, z];
        }
    }
    match patch.get("held") {
        Some(Json::String(item)) => p.held = Some((item.clone(), 1)),
        Some(Json::Null) => p.held = None,
        _ => {}
    }
}

/// The box and eyes, most health and movement_speed of an entity type, as
/// the game has them (a type not here is a player's size).
fn kind_defaults(kind: &str) -> (f64, f64, f64, f32, f64) {
    match kind.trim_start_matches("minecraft:") {
        "player" => (0.6, 1.8, 1.62, 20.0, 0.1),
        "zombie" | "husk" | "drowned" | "zombie_villager" => (0.6, 1.95, 1.74, 20.0, 0.23),
        "skeleton" | "stray" => (0.6, 1.99, 1.74, 20.0, 0.25),
        "creeper" => (0.6, 1.7, 1.445, 20.0, 0.25),
        "spider" => (1.4, 0.9, 0.65, 16.0, 0.3),
        "enderman" => (0.6, 2.9, 2.55, 40.0, 0.3),
        "villager" => (0.6, 1.95, 1.62, 20.0, 0.5),
        "cow" => (0.9, 1.4, 1.3, 10.0, 0.2),
        "pig" => (0.9, 0.9, 0.765, 10.0, 0.25),
        "sheep" => (0.9, 1.3, 1.235, 8.0, 0.23),
        "chicken" => (0.4, 0.7, 0.644, 4.0, 0.25),
        "item" => (0.25, 0.25, 0.2125, 0.0, 0.0),
        "experience_orb" => (0.5, 0.5, 0.425, 0.0, 0.0),
        _ => (0.6, 1.8, 1.53, 20.0, 0.25),
    }
}

/// An `entities` step's entity: its type (`zombie` is `minecraft:zombie`),
/// id (from its place in the list unless given), name, position, facing,
/// box, health, speed and motion, a dropped item's `item` and `count` or an
/// orb's `value`; the rest as the game has that type.
fn entity_of(i: usize, e: &Json) -> EntityInfo {
    let num = |k: &str| e.get(k).and_then(Json::as_f64);
    let triple = |k: &str| -> Option<[f64; 3]> {
        let v: Vec<f64> = e.get(k)?.as_array()?.iter().filter_map(Json::as_f64).collect();
        match v[..] {
            [x, y, z] => Some([x, y, z]),
            _ => None,
        }
    };
    let kind = e.get("kind").and_then(Json::as_str).unwrap_or("zombie");
    let kind = if kind.contains(':') {
        kind.to_owned()
    } else {
        format!("minecraft:{kind}")
    };
    let (width, height, eye, max_health, speed) = kind_defaults(&kind);
    let max_health = num("max_health").map_or(max_health, |h| h as f32);
    let yaw = num("yaw").unwrap_or(0.0) as f32;
    let item = match kind.as_str() {
        "minecraft:item" => Some((
            e.get("item")
                .and_then(Json::as_str)
                .unwrap_or("minecraft:stone")
                .to_owned(),
            num("count").unwrap_or(1.0) as u32,
        )),
        "minecraft:experience_orb" => Some((String::new(), num("value").unwrap_or(1.0) as u32)),
        _ => None,
    };
    EntityInfo {
        id: num("id").map_or(100 + i as u32, |n| n as u32),
        name: e.get("name").and_then(Json::as_str).unwrap_or("").to_owned(),
        pos: triple("pos").unwrap_or([0.5, 64.0, 2.5]),
        velocity: triple("velocity").unwrap_or([0.0; 3]),
        body_yaw: yaw,
        head_yaw: num("head_yaw").map_or(yaw, |v| v as f32),
        pitch: num("pitch").unwrap_or(0.0) as f32,
        width: num("width").unwrap_or(width),
        height: num("height").unwrap_or(height),
        eye: num("eye").unwrap_or(eye),
        health: num("health").map_or(max_health, |h| h as f32),
        max_health,
        speed: num("speed").unwrap_or(speed),
        on_ground: e.get("on_ground").and_then(Json::as_bool).unwrap_or(true),
        baby: e.get("baby").and_then(Json::as_bool).unwrap_or(false),
        item,
        kind,
    }
}

/// A `players` step's player: their name, UUID (from the name unless
/// given), display name, entity while near (an `entities` step's id), ping,
/// and whether they are the player playing (as named by the player step).
fn player_entry_of(p: &Json, me: Option<&PlayerInfo>) -> PlayerEntry {
    let name = p.get("name").and_then(Json::as_str).unwrap_or("Player").to_owned();
    let uuid = p
        .get("uuid")
        .and_then(Json::as_str)
        .and_then(|u| u128::from_str_radix(&u.replace('-', ""), 16).ok())
        .unwrap_or_else(|| {
            // FNV-1a, twice over: a UUID of its own for each name.
            let hash = |seed: u64| {
                name.bytes()
                    .fold(seed, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
            };
            (u128::from(hash(0xcbf2_9ce4_8422_2325)) << 64) | u128::from(hash(0x8422_2325_cbf2_9ce4))
        });
    let local = p
        .get("local")
        .and_then(Json::as_bool)
        .unwrap_or_else(|| me.is_some_and(|m| !m.name.is_empty() && m.name == name));
    PlayerEntry {
        uuid,
        display_name: p.get("display_name").and_then(Json::as_str).unwrap_or(&name).to_owned(),
        entity: p.get("entity").and_then(Json::as_u64).map(|n| n as u32),
        local,
        latency: p.get("latency").and_then(Json::as_i64).unwrap_or(0) as i32,
        name,
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
        let mesh_mark = run.game.meshes.len();
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
        } else if let Some(line) = step.get("suggest").and_then(Json::as_str) {
            match suggest_step(run, line) {
                Ok(list) => output = list,
                Err(e) => failures.push(e),
            }
        } else if let Some(patch) = step.get("player") {
            match (patch, run.game.player.as_mut()) {
                (Json::Null, _) => run.game.player = None,
                (patch, Some(p)) => player_patch(p, patch),
                (_, None) => failures.push(
                    "the player is gone (set \"player\": {...} after null to bring one back is not supported)".into(),
                ),
            }
            if let Some(on) = patch.get("on_ground").and_then(Json::as_bool) {
                run.game.on_ground = on;
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
        } else if let Some(list) = step.get("entities").and_then(Json::as_array) {
            run.game.entities = list.iter().enumerate().map(|(i, e)| entity_of(i, e)).collect();
            frames(run, 1, &mut drawn);
        } else if let Some(list) = step.get("players").and_then(Json::as_array) {
            let me = run.game.player.clone();
            run.game.players = Some(list.iter().map(|p| player_entry_of(p, me.as_ref())).collect());
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
                "unknown step {}: use frames, seconds, command, chat, key, lua, suggest, player, block, entities, players, time, camera, pose, click, type or reload",
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
                failures.push(format!("the output has no '{want}' (output: {output:?})"));
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
        let triangles: usize = run.game.meshes[mesh_mark..].iter().map(|m| m.0).sum();
        let textured = run.game.meshes[mesh_mark..].iter().filter(|m| m.1.is_some()).count();
        if let Some(want) = step.get("expect_mesh") {
            let least = want.as_u64().unwrap_or(1) as usize;
            if want != &Json::Bool(false) && triangles < least.max(1) {
                failures.push(format!(
                    "{triangles} mesh triangles drawn, wanted at least {}",
                    least.max(1)
                ));
            }
            if want == &Json::Bool(false) && triangles > 0 {
                failures.push(format!("{triangles} mesh triangles drawn, wanted none"));
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
        report["meshes"] = json!({ "triangles": triangles, "textured_draws": textured });
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

/// A `suggest` step: the completions the chat would get for `line` (a
/// command up to the cursor), each as its text and its tooltip in brackets.
fn suggest_step(run: &mut Run, line: &str) -> Result<Vec<String>, String> {
    let line = format!("/{}", line.trim_start().trim_start_matches('/'));
    match run.studio.suggest(&mut run.game, &line) {
        Some(c) => Ok(c
            .list
            .into_iter()
            .map(|(text, tooltip)| match tooltip {
                Some(tip) => format!("{text} ({tip})"),
                None => text,
            })
            .collect()),
        None => Err(match split_line(&line) {
            Some((name, None)) => {
                format!("completions start after a command's name and a space: \"/{name} \", not \"/{name}\"")
            }
            _ => format!("no mod completes {}", line.split(' ').next().unwrap_or(&line)),
        }),
    }
}

/// A module's files on disk, copied as they are (code and assets alike,
/// whatever their encoding) into a run's mods folder: three folders deep,
/// 16 MB a file, 64 MB in all. A file there already is kept.
fn copy_folder(from: &Path, to: &Path) {
    if !from.join("main.lua").is_file() {
        return;
    }
    let mut total = 0u64;
    let mut stack = vec![(from.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                stack.push((p, depth + 1));
                continue;
            }
            let len = p.metadata().map_or(u64::MAX, |m| m.len());
            let Ok(rel) = p.strip_prefix(from) else { continue };
            let dest = to.join(rel);
            if !p.is_file() || dest.exists() || len > 16 << 20 || total + len > 64 << 20 {
                continue;
            }
            total += len;
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&p, &dest);
        }
    }
}

/// A mesh drawn: its triangles, its texture's size if it had one, a corner
/// of its first triangle.
pub type MeshDrawn = (usize, Option<(u32, u32)>, [f64; 3]);

/// How to write test steps, for the MCP tools' descriptions.
pub const STEPS_HELP: &str = "Each step is an object with one action and any expectations. Actions: \
{\"frames\": N} or {\"seconds\": S} run the game (20 ticks a second); {\"command\": \"/name args\"} runs a chat command \
(a mod's own, or the studio's); {\"chat\": \"<Alex> hi\"} is a chat line arriving (on(\"chat\")); {\"key\": \"key.keyboard.j\"} \
presses and releases a key: on(\"key\") gets that name as for a real press (\"key.keyboard.space\", \"key.mouse.left\"; \
headless, keys.down sees it held), or with \"down\": true/false; {\"lua\": \"code\"} runs Lua in the console \
(globals; import(\"id\") gets a library; assert() fails the step); {\"suggest\": \"/name words\"} gives the completions \
the chat would show for that line (the cursor at its end, so \"/wp \" completes the first argument), each as its \
text and its tooltip in brackets, for expect_output (headless); {\"player\": {\"x\":..,\"y\":..,\"z\":..,\"health\":..,\
\"mode\":\"creative\",\"held\":\"minecraft:stick\",\"name\":\"Steve\",\"uuid\":\"8667ba71-b85a-4004-af54-457a9734eed7\",\
\"velocity\":[0,6,0],\"on_ground\":false}} moves or changes the player (name and uuid are their profile's, which Roblox \
scripts see as the LocalPlayer's Name and UserId; velocity, in blocks a second, and on_ground are how they move, which \
the Roblox Humanoid's MoveDirection and state follow); {\"block\": [x,y,z], \"id\": \
\"minecraft:stone\"} sets a block (live: not available); {\"entities\": [{\"kind\": \"zombie\", \"pos\": [3,64,0], \
\"health\": 10}, {\"kind\": \"player\", \"name\": \"Alex\", \"id\": 12, \"pos\": [5,64,0]}, {\"kind\": \"item\", \"item\": \
\"minecraft:diamond\", \"count\": 3}]} puts these entities near the player in place of the last ones, which Roblox \
scripts see in the Workspace: a kind (zombie, player, item, experience_orb...), an id (100 on in their order unless given), \
pos, and name, yaw, velocity, on_ground, baby, health, max_health, speed, width, height and eye (the rest as the game has \
the kind), a dropped item's item and count, or an orb's value; {\"players\": [{\"name\": \"Alex\", \"entity\": 12, \"latency\": 40}]} is the \
server's player list (uuid, display_name and local too), the other players Roblox scripts see in Players, each with the \
entity of their entity id as their Character (live: neither step is available); {\"time\": 13000} sets the day time; \
{\"click\": \"label\"} \
clicks text on the open script screen; {\"type\": \"text\"} types into it; {\"reload\": \"id\"} (or true) reloads; \
{\"camera\": \"first\"|\"third_back\"|\"third_front\"} and {\"pose\": \"crouching\"} set the view and the player's pose \
(headless). \
Expectations (strings or lists of them, matched as substrings): expect_chat, expect_no_chat, expect_output (the lua \
step's printed values, the suggest step's completions), expect_log, expect_sent, expect_drawn (text drawn on the HUD or screen during the step), \
expect_sound, expect_particle, expect_item (item models drawn: model.attach, draw.item); expect_mesh N \
(at least N mesh triangles drawn: draw.mesh, model.attach with a mesh; true: any; false: none); expect_state {\"id\": \"running\"|\"failed\"|\"off\"}; expect_screen true/false; \
expect_error true or \"text\" (otherwise any error logged during a step fails it). A step of expectations alone \
checks the state as it is.";
