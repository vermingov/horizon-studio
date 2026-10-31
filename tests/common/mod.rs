// A fake game for the studio's tests: it records what the studio does and
// answers its questions from plain fields.
#![allow(dead_code)]

use horizon_studio::game::hstudio::{Camera, Corner, Font, GameApi, PlayerInfo, Reply, Studio, ViaInfo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct FakeGame {
    pub dir: PathBuf,
    pub chats: Vec<String>,
    pub sent: Vec<String>,
    pub player: Option<PlayerInfo>,
    pub blocks: HashMap<(i32, i32, i32), String>,
    pub clock: Option<(i64, i64)>,
    pub ticks: u64,
    pub screen: String,
    pub agent_calls: Vec<String>,
    /// What `agent` answers: by command name, JSON for now, or a channel for later.
    pub agent_now: HashMap<String, serde_json::Value>,
    pub agent_later: Vec<(String, std::sync::mpsc::Sender<String>)>,
    pub drawn: Vec<String>,
    pub clipboard: String,
    pub held_mappings: HashMap<String, bool>,
    pub opened: Vec<PathBuf>,
    pub camera: Option<Camera>,
    /// 3D quads drawn (corners, colour), GUI quads, sounds and particles.
    pub world_quads: Vec<([[f64; 3]; 4], u32)>,
    pub quads: Vec<([[f32; 2]; 4], u32)>,
    pub sounds: Vec<(String, Option<[f64; 3]>, f32, f32)>,
    pub particles: Vec<Particle>,
    /// ViaHorizon, as the bridge reports it (the real one is the game's).
    pub via: ViaInfo,
}

/// A particle spawned: its id, place, speed and colour.
pub type Particle = (String, [f64; 3], [f64; 3], Option<[f32; 4]>);

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl FakeGame {
    pub fn new(name: &str) -> FakeGame {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("hstudio-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        FakeGame {
            dir,
            chats: Vec::new(),
            sent: Vec::new(),
            player: Some(PlayerInfo {
                x: 10.5,
                y: 64.0,
                z: -3.25,
                yaw: 180.0,
                pitch: 0.0,
                dimension: "minecraft:overworld".into(),
                health: 20.0,
                max_health: 20.0,
                food: 20,
                saturation: 5.0,
                level: 0,
                mode: "survival".into(),
                slot: 0,
                held: Some(("minecraft:diamond_pickaxe".into(), 1)),
            }),
            blocks: HashMap::new(),
            clock: Some((1000, 50_000)),
            ticks: 0,
            screen: "playing".into(),
            agent_calls: Vec::new(),
            agent_now: HashMap::new(),
            agent_later: Vec::new(),
            drawn: Vec::new(),
            clipboard: String::new(),
            held_mappings: HashMap::new(),
            opened: Vec::new(),
            // At the player's eyes, looking north (yaw 180), with vanilla's 70 degrees.
            camera: Some(Camera {
                pos: [10.5, 65.62, -3.25],
                yaw: 180.0,
                pitch: 0.0,
                fov: 70.0,
            }),
            world_quads: Vec::new(),
            quads: Vec::new(),
            sounds: Vec::new(),
            particles: Vec::new(),
            via: ViaInfo {
                enabled: true,
                version: "auto".into(),
                status: "Ready (ViaProxy 3.4.14, Java 21)".into(),
                ready: true,
                versions: ["auto", "26.3", "1.21.4", "1.12.2", "1.8.x"]
                    .iter()
                    .map(|v| {
                        (
                            (*v).into(),
                            if *v == "auto" {
                                "Auto detect".into()
                            } else {
                                (*v).into()
                            },
                        )
                    })
                    .collect(),
                log: vec!["[ViaProxy] Binding proxy server to 127.0.0.1:41234".into()],
            },
        }
    }
    pub fn mods_dir(&self) -> PathBuf {
        self.dir.join("mods")
    }
    pub fn write_mod(&self, name: &str, src: &str) {
        std::fs::create_dir_all(self.mods_dir()).unwrap();
        std::fs::write(self.mods_dir().join(name), src).unwrap();
    }
}

impl Drop for FakeGame {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl GameApi for FakeGame {
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
        self.clock
    }
    fn ticks(&self) -> u64 {
        self.ticks
    }
    fn fps(&self) -> [u32; 3] {
        [144, 90, 60]
    }
    fn mapping(&mut self, name: &str) -> Option<(bool, String)> {
        (name == "key.jump").then(|| (self.held_mappings.get(name).copied().unwrap_or(false), "Space".into()))
    }
    fn screen(&self) -> String {
        self.screen.clone()
    }
    fn agent(&mut self, line: &str) -> Result<Reply, String> {
        self.agent_calls.push(line.to_owned());
        let name = line.split_whitespace().next().unwrap_or("");
        if let Some(v) = self.agent_now.get(name) {
            return Ok(Reply::Now(v.clone()));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.agent_later.push((line.to_owned(), tx));
        Ok(Reply::Later(rx))
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
        (480.0, 270.0)
    }
    fn text_width(&self, text: &str) -> f32 {
        6.0 * text.chars().count() as f32
    }
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.drawn.push(format!("fill {rect:?} {argb:08x}"));
    }
    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32, _shadow: bool) {
        self.drawn.push(format!("text {text} @{at:?} {rgb:06x}"));
    }
    fn camera(&self) -> Option<Camera> {
        self.camera
    }
    fn world_quad(&mut self, corners: [[f64; 3]; 4], argb: u32) {
        self.world_quads.push((corners, argb));
    }
    fn quad(&mut self, corners: [[f32; 2]; 4], argb: u32) {
        self.quads.push((corners, argb));
    }
    fn sound(&mut self, id: &str, at: Option<[f64; 3]>, volume: f32, pitch: f32) -> Result<(), String> {
        self.sounds.push((id.to_owned(), at, volume, pitch));
        Ok(())
    }
    fn particle(&mut self, id: &str, at: [f64; 3], velocity: [f64; 3], color: Option<[f32; 4]>) -> Result<(), String> {
        if id.contains("nope") {
            return Err(format!("unknown particle {id}"));
        }
        self.particles.push((id.to_owned(), at, velocity, color));
        Ok(())
    }
    fn dp(&mut self) -> (f32, f32, f32) {
        (960.0, 540.0, 1.0)
    }
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.drawn.push(format!("rect {r:?} r{radius} {argb:08x}"));
    }
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.drawn.push(format!("border {r:?} r{radius} {argb:08x}"));
    }
    fn dp_text(&mut self, text: &str, at: [f32; 2], _size: f32, _font: Font, argb: u32) -> f32 {
        self.drawn.push(format!("text {text} @{at:?} {:06x}", argb & 0xFF_FFFF));
        6.0 * text.chars().count() as f32
    }
    fn dp_measure(&mut self, text: &str, _size: f32, _font: Font) -> f32 {
        6.0 * text.chars().count() as f32
    }
    fn via(&self) -> ViaInfo {
        self.via.clone()
    }
    fn via_set(&mut self, enabled: Option<bool>, version: Option<&str>) -> Result<(), String> {
        if let Some(v) = version {
            if !self.via.versions.iter().any(|(id, _)| id == v) {
                return Err(format!("ViaProxy has no version {v}"));
            }
            self.via.version = v.into();
        }
        if let Some(on) = enabled {
            self.via.enabled = on;
        }
        Ok(())
    }
    fn open_folder(&mut self, path: &std::path::Path) {
        self.opened.push(path.to_path_buf());
    }
    fn screenshot(&mut self, path: &std::path::Path) -> Result<(), String> {
        // A PNG's signature and end mark: enough for what reads it.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(b"\0\0\0\0IEND\xaeB`\x82");
        std::fs::write(path, png).map_err(|e| e.to_string())
    }
    fn place(&mut self, corner: Corner, _w: f32, _h: f32) -> (f32, f32) {
        match corner {
            Corner::TopLeft => (2.0, 2.0),
            Corner::TopRight => (300.0, 2.0),
            Corner::BottomLeft => (2.0, 200.0),
            Corner::BottomRight => (300.0, 200.0),
        }
    }
}

/// A studio started on the fake game.
pub fn studio(game: &mut FakeGame) -> Studio {
    let mut st = Studio::new();
    st.frame(game, 0.016);
    st
}

/// Lets the studio's clock pass `seconds` in 50 ms frames, ticking the game.
pub fn run_for(st: &mut Studio, game: &mut FakeGame, seconds: f64) {
    let frames = (seconds / 0.05).ceil() as usize;
    for _ in 0..frames {
        game.ticks += 1;
        st.frame(game, 0.05);
    }
}

/// The console lines the studio has logged, as text.
pub fn log(st: &Studio) -> Vec<String> {
    st.s.log.iter().map(|l| l.text.clone()).collect()
}

pub fn mod_index(st: &Studio, id: &str) -> usize {
    st.s.mods
        .iter()
        .position(|m| m.id == id)
        .unwrap_or_else(|| panic!("no mod {id}"))
}
