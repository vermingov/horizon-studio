// A fake game for the studio's tests: it records what the studio does and
// answers its questions from plain fields.
#![allow(dead_code)]

use horizon_studio::game::hstudio::{
    Body, Camera, ClientAction, ClientInfo, Corner, EntityInfo, Font, GameApi, PlayerEntry, PlayerInfo, PlayerPose,
    Reply, Studio, ViaInfo, VoiceChange, VoiceInfo,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct FakeGame {
    /// What the client did to its own player (move_player, its velocity,
    /// noclip, its speeds), newest last.
    pub moves: Vec<String>,
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
    /// The gradients the GUI was coloured with (`dp_shade`), with their marks.
    pub shades: Vec<(usize, horizon_studio::game::hstudio_quads::Shade)>,
    /// The game can't change what it drew (no `dp_mark`), as the headless one.
    pub no_marks: bool,
    /// The game can't draw images as such (no `dp_image`).
    pub no_images: bool,
    /// The key in the atlas (its place there) of each image drawn.
    pub image_keys: Vec<u32>,
    /// ViaHorizon, as the bridge reports it (the real one is the game's).
    pub via: ViaInfo,
    /// The player's pose (`pose`), if the game says.
    pub pose: Option<PlayerPose>,
    /// The entities and the player list the game reports, and how many
    /// times the studio asked for the entities.
    pub entities: Vec<EntityInfo>,
    pub players: Option<Vec<PlayerEntry>>,
    pub entity_asks: usize,
    /// The voice chat as the game reports it, and the changes asked of it;
    /// with `voice_glue`, a real voice chat (hstudio_voicechat) answers instead.
    pub voice: VoiceInfo,
    pub voice_changes: Vec<VoiceChange>,
    pub voice_glue: Option<horizon_studio::game::hstudio_voicechat::Glue>,
    /// How the player's own model looks, as the studio last set it.
    pub body: Body,
    /// The client as the game reports it (its server, a disconnect), and
    /// what Roblox scripts asked of it (`client_do`), newest last.
    pub client: ClientInfo,
    pub client_done: Vec<ClientAction>,
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
            moves: Vec::new(),
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
                name: String::new(),
                uuid: 0,
                velocity: [0.0; 3],
                speed: Some(0.1),
                jump: Some(0.42),
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
            shades: Vec::new(),
            no_marks: false,
            no_images: false,
            image_keys: Vec::new(),
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
            pose: None,
            entities: Vec::new(),
            players: None,
            entity_asks: 0,
            voice: VoiceInfo::default(),
            voice_changes: Vec::new(),
            voice_glue: None,
            body: Body::default(),
            client: ClientInfo::default(),
            client_done: Vec::new(),
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
    fn move_player(&mut self, feet: [f64; 3]) -> bool {
        self.moves
            .push(format!("move {:.3} {:.3} {:.3}", feet[0], feet[1], feet[2]));
        if let Some(p) = self.player.as_mut() {
            (p.x, p.y, p.z) = (feet[0], feet[1], feet[2]);
        }
        true
    }
    fn set_player_velocity(&mut self, v: [f64; 3]) -> bool {
        self.moves.push(format!("velocity {:.2} {:.2} {:.2}", v[0], v[1], v[2]));
        true
    }
    fn set_noclip(&mut self, on: bool) -> bool {
        self.moves.push(format!("noclip {on}"));
        true
    }
    fn set_player_speed(&mut self, walk: f64, jump: f64) -> bool {
        self.moves.push(format!("speed {walk:.2} {jump:.2}"));
        true
    }
    fn set_fullbright(&mut self, strength: f32) {
        self.moves.push(format!("fullbright {strength:.2}"));
    }
    fn set_time_of_day(&mut self, ticks: Option<i64>) {
        self.moves.push(format!("time {ticks:?}"));
    }
    fn set_fog_end(&mut self, blocks: Option<f32>) {
        self.moves.push(format!("fog {blocks:?}"));
    }
    fn set_gravity_scale(&mut self, scale: f64) {
        self.moves.push(format!("gravity {scale:.2}"));
    }
    fn set_fov(&mut self, degrees: Option<f32>) {
        self.moves.push(format!("fov {degrees:?}"));
    }
    fn set_look(&mut self, yaw: f32, pitch: f32) {
        self.moves.push(format!("look {yaw:.1} {pitch:.1}"));
    }
    fn turn_player(&mut self, dyaw: f32, dpitch: f32) {
        self.moves.push(format!("turn {dyaw:.1} {dpitch:.1}"));
    }
    fn jump_player(&mut self) -> bool {
        self.moves.push("jump".into());
        true
    }
    fn set_player_rise(&mut self, vy: f64) -> bool {
        self.moves.push(format!("rise {vy:.2}"));
        true
    }
    // "body" and what differs from the game's own model.
    fn set_body(&mut self, body: &Body) {
        let mut line = String::from("body");
        if !body.hidden.is_empty() {
            line += &format!(" hidden={}", body.hidden.join(","));
        }
        if body.gone {
            line += " gone";
        }
        if !body.pose.is_empty() {
            line += &format!(" pose={}", body.pose);
        }
        if body.spin != 0.0 {
            line += &format!(" spin={:.0}", body.spin);
        }
        if body.still {
            line += " still";
        }
        if body.fixed {
            line += " fixed";
        }
        self.moves.push(line);
        self.body = body.clone();
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
    // A mark is where the next drawing goes in `drawn`.
    fn dp_mark(&mut self) -> Option<usize> {
        (!self.no_marks).then_some(self.drawn.len())
    }
    fn dp_clip(&mut self, mark: usize, r: [f32; 4]) {
        self.drawn.push(format!("clip {mark} {r:?}"));
    }
    fn dp_rotate(&mut self, mark: usize, centre: [f32; 2], degrees: f32) {
        self.drawn.push(format!("rotate {mark} {centre:?} {degrees}"));
    }
    fn dp_shade(&mut self, mark: usize, shade: &horizon_studio::game::hstudio_quads::Shade) {
        self.drawn.push(format!("shade {mark}"));
        self.shades.push((mark, shade.clone()));
    }
    fn dp_slant(&mut self, mark: usize, y: f32, slope: f32) {
        self.drawn.push(format!("slant {mark} {y} {slope}"));
    }
    // An image by its size and its first pixel's RGBA.
    fn dp_image(
        &mut self,
        image: horizon_studio::game::hstudio::TexRef<'_>,
        r: [f32; 4],
        src: [f32; 4],
        argb: u32,
    ) -> bool {
        if self.no_images {
            return false;
        }
        self.image_keys.push(image.key);
        let first: String = image.rgba.iter().take(4).map(|b| format!("{b:02x}")).collect();
        self.drawn.push(format!(
            "image {}x{} #{first} {r:?} from {src:?} {argb:08x}",
            image.w, image.h
        ));
        true
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
    fn pose(&self) -> Option<PlayerPose> {
        self.player.as_ref()?;
        self.pose.clone()
    }
    fn entities(&mut self, max: usize) -> Vec<EntityInfo> {
        self.entity_asks += 1;
        self.entities.iter().take(max).cloned().collect()
    }
    fn players(&mut self) -> Option<Vec<PlayerEntry>> {
        self.players.clone()
    }
    fn voice(&self) -> VoiceInfo {
        match &self.voice_glue {
            Some(g) => g.info(std::time::Instant::now()),
            None => self.voice.clone(),
        }
    }
    fn client(&self) -> ClientInfo {
        self.client.clone()
    }
    fn client_do(&mut self, action: ClientAction) -> Result<(), String> {
        match &action {
            ClientAction::MasterVolume(v) => self.client.master_volume = *v,
            ClientAction::ToggleFullscreen => self.client.fullscreen = !self.client.fullscreen,
            // The real voice chat's own, with one.
            ClientAction::MuteVoice(who, on) => {
                if let Some(g) = &mut self.voice_glue {
                    g.vc.mute_voice(*who, *on);
                }
            }
            ClientAction::VoiceListener(at) => {
                if let Some(g) = &mut self.voice_glue {
                    g.listener_at = *at;
                }
            }
            _ => {}
        }
        self.client_done.push(action);
        Ok(())
    }
    fn voice_set(&mut self, change: VoiceChange) -> Result<(), String> {
        self.voice_changes.push(change.clone());
        match &mut self.voice_glue {
            Some(g) => g.set(change),
            None if self.voice.available => Ok(()),
            None => Err("voice chat isn't available here".into()),
        }
    }
}

/// A studio started on the fake game.
pub fn studio(game: &mut FakeGame) -> Studio {
    let mut st = Studio::new();
    // Tests stop an endless loop by its steps, at once, not after Roblox's
    // ten seconds (a test of the timeout sets its own).
    st.s.script_timeout = 0.0;
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
        .unwrap_or_else(|| panic!("no mod {id}: {:?}", log(st)))
}
