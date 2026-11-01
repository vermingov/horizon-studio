// Differential tests: every bundled example runs through a scripted session
// on a game that records each call the studio makes into it (with its
// arguments), and a corpus of Lua snippets runs on its own. What came out
// must match the transcripts in tests/golden, which the tree-walking
// interpreter wrote: a change of interpreter must not change what scripts
// do. `GOLDEN_UPDATE=1 cargo test --test golden` rewrites them.

use horizon_studio::game::hstudio::{
    Camera, Corner, Font, GameApi, PartFrame, PlayerInfo, PlayerPose, Reply, Studio, TexRef, ViaInfo, WorldTri,
};
use horizon_studio::game::hstudio_examples::EXAMPLES;
use horizon_studio::game::hstudio_look::draw_screen;
use horizon_studio::game::hstudio_lua::{Lua, NoHost, Throw, syntax_error_at};
use horizon_studio::game::hstudio_lualib::{inspect, new_env};
use horizon_studio::game::hstudio_ui::{Input, Key, Mods, handle_screen};
use horizon_studio::headless::HeadlessGame;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The headless game, with every call written down.
struct Rec {
    g: HeadlessGame,
    out: RefCell<Vec<String>>,
    via: ViaInfo,
    /// Text drawn this frame, in GUI pixels, for clicks.
    drawn: Vec<(String, f32, f32, f32)>,
}

impl Rec {
    fn put(&self, line: String) {
        self.out.borrow_mut().push(line);
    }
}

impl GameApi for Rec {
    fn chat(&mut self, text: &str) {
        self.put(format!("chat {text}"));
        self.g.chat(text);
    }
    fn send(&mut self, line: &str) {
        self.put(format!("send {line}"));
        self.g.send(line);
    }
    fn player(&mut self) -> Option<PlayerInfo> {
        self.g.player()
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        let b = self.g.block(x, y, z);
        self.put(format!("block {x} {y} {z} -> {b:?}"));
        b
    }
    fn clock(&self) -> Option<(i64, i64)> {
        self.g.clock()
    }
    fn ticks(&self) -> u64 {
        self.g.ticks()
    }
    fn fps(&self) -> [u32; 3] {
        self.g.fps()
    }
    fn mapping(&mut self, name: &str) -> Option<(bool, String)> {
        let m = self.g.mapping(name);
        self.put(format!("mapping {name} -> {m:?}"));
        m
    }
    fn screen(&self) -> String {
        self.g.screen()
    }
    fn agent(&mut self, line: &str) -> Result<Reply, String> {
        self.put(format!("agent {line}"));
        let mut words = line.split_whitespace();
        let v = match (words.next(), words.next()) {
            (Some("find-block"), Some(block)) if block.contains("diamond") || block.contains("stone") => {
                serde_json::json!({"count": 2, "nearest": [
                    {"pos": [3, 60, -4], "distance": 5.25},
                    {"pos": [-7, 58, 2], "distance": 9.5},
                ]})
            }
            (Some("find-block"), Some(block)) if block.contains("log") => {
                serde_json::json!({"count": 1, "nearest": [{"pos": [2, 64, 9], "distance": 8.0}]})
            }
            (Some("find-block"), _) => serde_json::json!({"count": 0, "nearest": []}),
            (Some("goto"), _) => serde_json::json!({"arrived": true}),
            (Some("break"), Some("-7")) => serde_json::json!({"error": "too far"}),
            (Some(cmd), _) => serde_json::json!({"done": cmd}),
            _ => return Err("empty".into()),
        };
        Ok(Reply::Now(v))
    }
    fn data_dir(&self) -> PathBuf {
        self.g.data_dir()
    }
    fn clipboard(&mut self) -> String {
        self.put("clipboard".into());
        self.g.clipboard()
    }
    fn set_clipboard(&mut self, text: &str) {
        self.put(format!("set_clipboard {text:?}"));
        self.g.set_clipboard(text);
    }
    fn gui_size(&self) -> (f32, f32) {
        self.g.gui_size()
    }
    fn text_width(&self, text: &str) -> f32 {
        self.g.text_width(text)
    }
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.put(format!("fill {rect:?} {argb:08x}"));
    }
    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32, shadow: bool) {
        self.put(format!("text {text:?} {at:?} {rgb:06x} {shadow}"));
        self.drawn
            .push((text.to_owned(), at[0], at[1], self.g.text_width(text)));
    }
    fn place(&mut self, corner: Corner, w: f32, h: f32) -> (f32, f32) {
        let at = self.g.place(corner, w, h);
        self.put(format!("place {corner:?} {w} {h} -> {at:?}"));
        at
    }
    fn open_folder(&mut self, path: &Path) {
        self.put(format!(
            "open_folder {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    fn camera(&self) -> Option<Camera> {
        self.g.camera()
    }
    fn world_quad(&mut self, corners: [[f64; 3]; 4], argb: u32) {
        self.put(format!("world_quad {corners:?} {argb:08x}"));
    }
    fn quad(&mut self, corners: [[f32; 2]; 4], argb: u32) {
        self.put(format!("quad {corners:?} {argb:08x}"));
    }
    fn sound(&mut self, id: &str, at: Option<[f64; 3]>, volume: f32, pitch: f32) -> Result<(), String> {
        let r = self.g.sound(id, at, volume, pitch);
        self.put(format!("sound {id} {at:?} {volume} {pitch} -> {r:?}"));
        r
    }
    fn particle(&mut self, id: &str, at: [f64; 3], velocity: [f64; 3], color: Option<[f32; 4]>) -> Result<(), String> {
        let r = self.g.particle(id, at, velocity, color);
        self.put(format!("particle {id} {at:?} {velocity:?} {color:?} -> {r:?}"));
        r
    }
    fn dp(&mut self) -> (f32, f32, f32) {
        self.g.dp()
    }
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.put(format!("dp_rect {r:?} {radius} {argb:08x}"));
    }
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.put(format!("dp_border {r:?} {radius} {argb:08x}"));
    }
    fn dp_text(&mut self, text: &str, at: [f32; 2], size: f32, font: Font, argb: u32) -> f32 {
        self.put(format!("dp_text {text:?} {at:?} {size} {font:?} {argb:08x}"));
        let (.., dp) = self.g.dp();
        let w = self.g.dp_measure(text, size, font);
        self.drawn.push((text.to_owned(), at[0] / dp, at[1] / dp, w / dp));
        w
    }
    fn dp_measure(&mut self, text: &str, size: f32, font: Font) -> f32 {
        self.g.dp_measure(text, size, font)
    }
    fn via(&self) -> ViaInfo {
        self.via.clone()
    }
    fn via_set(&mut self, enabled: Option<bool>, version: Option<&str>) -> Result<(), String> {
        self.put(format!("via_set {enabled:?} {version:?}"));
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
    fn part_frame(&self, target: Option<u32>, part: &str) -> Option<PartFrame> {
        self.g.part_frame(target, part)
    }
    fn body_shown(&self) -> bool {
        self.g.body_shown()
    }
    fn pose(&self) -> Option<PlayerPose> {
        self.g.pose()
    }
    fn world_item(
        &mut self,
        id: &str,
        m: [[f64; 3]; 4],
        display: &str,
        light_at: Option<[f64; 3]>,
    ) -> Result<(), String> {
        let r = self.g.world_item(id, m, display, light_at);
        self.put(format!("world_item {id} {m:?} {display} {light_at:?} -> {r:?}"));
        r
    }
    fn world_quad_lit(&mut self, corners: [[f64; 3]; 4], argb: u32, normal: [f64; 3], light_at: Option<[f64; 3]>) {
        self.put(format!("world_quad_lit {corners:?} {argb:08x} {normal:?} {light_at:?}"));
    }
    fn world_mesh(&mut self, tris: &[WorldTri], texture: Option<TexRef<'_>>, light_at: Option<[f64; 3]>) {
        let sum: f64 = tris.iter().flat_map(|t| t.p.iter().flatten()).sum();
        let first = tris.first().map(|t| t.p);
        let tex = texture.map(|t| (t.w, t.h, t.rgba.iter().map(|&b| u64::from(b)).sum::<u64>()));
        self.put(format!(
            "world_mesh {} tris, first {first:?}, sum {sum:.6}, texture {tex:?}, light {light_at:?}",
            tris.len()
        ));
    }
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// A step of a session.
enum Step {
    Frames(u32),
    Command(&'static str),
    /// A key pressed, a frame, released, a frame.
    Key(&'static str),
    Chat(&'static str),
    /// Console code.
    Lua(&'static str),
    Move(f64, f64, f64, f32),
    Health(f32, i32),
    Block(i32, i32, i32, &'static str),
    Time(i64),
    Camera(&'static str),
    /// A click on text drawn on the script's screen.
    Click(&'static str),
    Type(&'static str),
    Enter,
    Wheel(f32),
    Reload,
    Disable,
    Enable,
}

struct Session {
    st: Studio,
    game: Rec,
    out: Vec<String>,
    frame: u32,
    /// The last frame's lines (none after a step), and how many frames
    /// since repeated them.
    last: Option<Vec<String>>,
    same: u32,
    log_mark: u64,
}

impl Session {
    fn new(ids: &[&str]) -> Session {
        let n = RUNS.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("hstudio-golden-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mods = dir.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(dir.join("studio.txt"), "seen:uikit\nseen:viahorizon\n").unwrap();
        for id in ids {
            let ex = EXAMPLES.iter().find(|e| e.id == *id).unwrap();
            std::fs::write(mods.join(format!("{id}.lua")), ex.source).unwrap();
        }
        let mut g = HeadlessGame::new(dir);
        if let Some(p) = g.player.as_mut() {
            p.held = Some(("minecraft:diamond_pickaxe".into(), 1));
        }
        g.blocks.insert((0, 63, 0), "minecraft:grass_block".into());
        let via = ViaInfo {
            enabled: true,
            version: "auto".into(),
            status: "Ready (ViaProxy 3.4.14, Java 21)".into(),
            ready: true,
            versions: ["auto", "26.3", "1.21.4", "1.12.2", "1.8.x", "b1.7.3"]
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
        };
        let mut st = Studio::new();
        // Endless loops stop by their steps, at once (not Roblox's timeout).
        st.s.script_timeout = 0.0;
        Session {
            st,
            game: Rec {
                g,
                out: RefCell::new(Vec::new()),
                via,
                drawn: Vec::new(),
            },
            out: Vec::new(),
            frame: 0,
            last: None,
            same: 0,
            log_mark: 0,
        }
    }

    /// The calls made since the last flush, and the console's new lines.
    fn take(&mut self) -> Vec<String> {
        let mut lines = std::mem::take(&mut *self.game.out.borrow_mut());
        let seq = self.st.s.log_seq;
        lines.extend(
            self.st
                .log_since(self.log_mark)
                .map(|(_, l)| format!("log {:?} {}", l.kind, l.text)),
        );
        self.log_mark = seq;
        lines
    }

    fn flush_same(&mut self) {
        if self.same > 0 {
            self.out.push(format!("  (the same for {} more frames)", self.same));
            self.same = 0;
        }
    }

    fn put(&mut self, head: String, lines: Vec<String>) {
        self.flush_same();
        self.out.push(head);
        self.out.extend(lines.into_iter().map(|l| format!("  {l}")));
    }

    fn frame(&mut self) {
        self.frame += 1;
        let (st, game) = (&mut self.st, &mut self.game);
        game.drawn.clear();
        game.g.ticks += 1;
        if let Some((day, total)) = game.g.clock.as_mut() {
            *day += 1;
            *total += 1;
        }
        st.frame(game, 0.05);
        // Sent as the player, a command goes to the server, as in the game
        // (one the studio takes ran in it instead).
        let new: Vec<String> = std::mem::take(&mut game.g.sent);
        for line in new {
            if let Some(cmd) = line.strip_prefix('/') {
                game.put(format!("(the game would run /{cmd})"));
            }
        }
        st.world(game);
        st.hud(game);
        if st.s.screen_request.is_some() {
            st.accept_screen();
            game.g.screen = "script".into();
        }
        if st.s.screen.is_some() {
            draw_screen(st, game);
        } else if game.g.screen == "script" {
            game.g.screen = "playing".into();
        }
        let lines = self.take();
        if self.last.as_ref() == Some(&lines) {
            self.same += 1;
            return;
        }
        let head = format!("frame {}", self.frame);
        self.put(head, lines.clone());
        self.last = Some(lines);
    }

    fn input(&mut self, input: Input) {
        handle_screen(&mut self.st, &mut self.game, input);
    }

    fn step(&mut self, step: &Step) {
        let what = match step {
            Step::Frames(n) => {
                for _ in 0..*n {
                    self.frame();
                }
                return;
            }
            Step::Command(c) => {
                let ours = self.st.command(c);
                format!("command /{c} (taken: {ours})")
            }
            Step::Key(k) => {
                self.game.g.held.insert((*k).into(), true);
                self.st.key(k, true);
                self.frame();
                self.game.g.held.insert((*k).into(), false);
                self.st.key(k, false);
                format!("key {k}")
            }
            Step::Chat(t) => {
                self.st.chat_line(t);
                format!("chat line {t}")
            }
            Step::Lua(code) => {
                self.st.run_chunk(&mut self.game, code, false);
                let all = self.take();
                self.put(format!("lua {code}"), all);
                self.last = None;
                return;
            }
            Step::Move(x, y, z, yaw) => {
                if let Some(p) = self.game.g.player.as_mut() {
                    (p.x, p.y, p.z, p.yaw) = (*x, *y, *z, *yaw);
                }
                format!("player to {x} {y} {z} yaw {yaw}")
            }
            Step::Health(h, food) => {
                if let Some(p) = self.game.g.player.as_mut() {
                    (p.health, p.food) = (*h, *food);
                }
                format!("health {h} food {food}")
            }
            Step::Block(x, y, z, id) => {
                self.game.g.blocks.insert((*x, *y, *z), (*id).into());
                format!("block {x} {y} {z} = {id}")
            }
            Step::Time(t) => {
                if let Some(c) = self.game.g.clock.as_mut() {
                    c.0 = *t;
                }
                format!("time {t}")
            }
            Step::Camera(m) => {
                self.game.g.camera_mode = (*m).into();
                format!("camera {m}")
            }
            Step::Click(label) => {
                let found = self
                    .game
                    .drawn
                    .iter()
                    .find(|d| d.0 == *label)
                    .or_else(|| self.game.drawn.iter().find(|d| d.0.contains(label)))
                    .cloned();
                match found {
                    Some((_, x, y, w)) => {
                        let (x, y) = (x + w.min(40.0) / 2.0, y + 4.0);
                        self.input(Input::Mouse(x, y));
                        self.input(Input::Button { left: true, down: true });
                        self.input(Input::Button {
                            left: true,
                            down: false,
                        });
                        format!("click {label} at {x} {y}")
                    }
                    None => format!("click {label}: not drawn"),
                }
            }
            Step::Type(t) => {
                self.input(Input::Text((*t).into()));
                format!("type {t}")
            }
            Step::Enter => {
                for down in [true, false] {
                    self.input(Input::Key {
                        key: Key::Enter,
                        down,
                        repeat: false,
                        mods: Mods::default(),
                    });
                }
                "enter".into()
            }
            Step::Wheel(n) => {
                self.input(Input::Wheel(*n));
                format!("wheel {n}")
            }
            Step::Reload => {
                self.st.reload_all(&mut self.game);
                "reload".into()
            }
            Step::Disable | Step::Enable => {
                let on = matches!(step, Step::Enable);
                for i in 0..self.st.s.mods.len() {
                    self.st.set_enabled(&mut self.game, i, on);
                }
                format!("enabled {on}")
            }
        };
        let lines = self.take();
        self.put(format!("> {what}"), lines);
        self.last = None;
    }

    fn finish(mut self) -> String {
        self.flush_same();
        for m in &self.st.s.mods {
            self.out.push(format!("mod {} {:?} {:?}", m.id, m.state, m.name));
        }
        let _ = std::fs::remove_dir_all(self.game.g.data_dir());
        scrub(&(self.out.join("\n") + "\n"))
    }
}

/// Addresses (`table: 0x...`) and the wall clock vary between runs.
fn scrub(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"0x") && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit) && i > 0 && b[i - 1] == b' ' {
            out.push_str("0x?");
            i += 2;
            while i < b.len() && b[i].is_ascii_hexdigit() {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"Real time ") && b.len() >= i + 15 {
            out.push_str("Real time HH:MM");
            i += 15;
            continue;
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Compares `got` with tests/golden/`name`.txt, or writes it with GOLDEN_UPDATE.
fn check(name: &str, got: &str) -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"));
    if std::env::var_os("GOLDEN_UPDATE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, got).unwrap();
        return Ok(());
    }
    let want = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if want == got {
        return Ok(());
    }
    let (w, g): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    let at = w
        .iter()
        .zip(&g)
        .position(|(a, b)| a != b)
        .unwrap_or(w.len().min(g.len()));
    let from = at.saturating_sub(3);
    let show = |v: &[&str]| v[from.min(v.len())..(at + 4).min(v.len())].join("\n");
    let _ = std::fs::write(std::env::temp_dir().join(format!("golden-{name}.txt")), got);
    Err(format!(
        "{name}: differs at line {}\n--- want ---\n{}\n--- got ---\n{}",
        at + 1,
        show(&w),
        show(&g)
    ))
}

/// What every session does around an example's own steps.
fn session(ids: &[&str], steps: &[Step]) -> String {
    let mut s = Session::new(ids);
    for step in [
        Step::Frames(30),
        Step::Lua("mods()"),
        Step::Chat("<Alex> hi there"),
        Step::Block(1, 64, 1, "minecraft:stone"),
        Step::Frames(2),
    ]
    .iter()
    {
        s.step(step);
    }
    for step in steps {
        s.step(step);
    }
    for step in [
        Step::Frames(5),
        Step::Move(-12.25, 70.0, 33.5, 135.0),
        Step::Camera("first"),
        Step::Frames(8),
        Step::Health(5.0, 4),
        Step::Time(14000),
        Step::Frames(25),
        Step::Reload,
        Step::Frames(10),
        Step::Disable,
        Step::Frames(3),
        Step::Enable,
        Step::Frames(10),
    ]
    .iter()
    {
        s.step(step);
    }
    s.finish()
}

fn example_steps(id: &str) -> Vec<Step> {
    use Step::*;
    match id {
        "hello" => vec![Command("hello"), Frames(1), Command("hello Steve"), Frames(1)],
        "coords" => vec![Command("coords"), Frames(3), Move(100.75, 12.0, -3.5, 270.0), Frames(3)],
        "clock" => vec![Frames(20), Time(23000), Frames(5)],
        "compass" => vec![
            Move(0.5, 64.0, 0.5, 45.0),
            Frames(2),
            Move(0.5, 64.0, 0.5, -170.0),
            Frames(2),
        ],
        "frame_graph" => vec![Frames(40)],
        "zoom" => vec![
            Key("key.keyboard.c"),
            Command("zoom 3"),
            Frames(1),
            Key("key.keyboard.c"),
            Command("zoom"),
            Command("zoom 99"),
            Frames(1),
        ],
        "notes" => vec![
            Command("notes"),
            Command("note remember this"),
            Frames(1),
            Move(5.0, 70.0, 5.0, 0.0),
            Command("note second note"),
            Frames(1),
            Command("notes"),
            Frames(1),
            Command("notes clear"),
            Command("notes"),
            Frames(1),
        ],
        "night_watch" => vec![Frames(25), Health(4.0, 20), Frames(25), Health(20.0, 3), Frames(25)],
        "ore_finder" => vec![
            Command("ores 8"),
            Frames(1),
            Command("ores"),
            Frames(1),
            Command("ores x"),
            Frames(1),
        ],
        "miner" => vec![
            Command("mine"),
            Frames(1),
            Command("mine stone 3"),
            Frames(12),
            Command("mine oak_log 2"),
            Frames(12),
            Command("mine diamond_ore 5"),
            Frames(2),
            Command("mine stop"),
            Frames(3),
        ],
        "waypoints" => vec![
            Command("wp add home"),
            Frames(1),
            Move(30.5, 80.0, -20.5, 0.0),
            Command("wp add the base"),
            Frames(3),
            Command("wp"),
            Frames(3),
            Click("the base"),
            Frames(3),
            Click("Add one here"),
            Type("mine"),
            Enter,
            Frames(3),
            Command("wp remove home"),
            Command("wp nope"),
            Frames(3),
            Command("wp clear"),
            Frames(2),
        ],
        "selection" => vec![
            Command("sel"),
            Command("pos1"),
            Frames(1),
            Move(4.5, 66.0, -6.5, 0.0),
            Command("pos2"),
            Frames(4),
            Command("sel clear"),
            Frames(2),
        ],
        "playground" => vec![
            Command("playground"),
            Frames(3),
            Click("Particle trail"),
            Frames(4),
            Click("Ring the bell"),
            Frames(2),
            Click("Trail colour"),
            Type("#ff8800"),
            Enter,
            Frames(3),
            Click("Close"),
            Frames(3),
        ],
        "outfit" => vec![
            Command("outfit nope"),
            Command("outfit hat"),
            Command("outfit crown"),
            Command("outfit orbit"),
            Frames(6),
            Command("outfit cape"),
            Command("outfit sword"),
            Command("outfit halo"),
            Frames(3),
            Command("outfit hat"),
            Frames(2),
            Command("outfit off"),
            Frames(2),
        ],
        "statue" => vec![
            Command("statue"),
            Frames(3),
            Command("statue crystal"),
            Frames(2),
            Command("statue wear"),
            Frames(2),
            Command("statue wear"),
            Command("statue off"),
            Frames(2),
        ],
        "uikit_demo" => vec![
            Command("kitdemo"),
            Frames(3),
            Click("Show the card"),
            Frames(3),
            Click("Title"),
            Type("Here"),
            Frames(2),
            Click("Finder"),
            Frames(3),
            Click("Ancient debris"),
            Frames(2),
            Click("Outline it"),
            Frames(25),
            Wheel(-1.0),
            Frames(2),
            Click("About"),
            Frames(3),
            Click("Save"),
            Frames(5),
            Command("kitdemo"),
            Frames(2),
            Click("Close"),
            Frames(2),
        ],
        "viahorizon" => vec![
            Command("via 1.8.x"),
            Command("via 9.9"),
            Frames(1),
            Command("vialog"),
            Frames(1),
            Command("via"),
            Frames(3),
            Click("1.12.2"),
            Frames(2),
            Click("Join servers through ViaProxy"),
            Frames(2),
            Click("Server version"),
            Type("1.2"),
            Frames(3),
            Click("Auto detect"),
            Frames(2),
            Click("Done"),
            Frames(2),
        ],
        "uikit" => vec![
            Lua("ui = import('uikit') return type(ui), type(ui.window)"),
            Lua(
                "local kit = import('uikit') local w = kit.window({title = 'T', width = 200}, function(ui) ui.text('hi') if ui.button('Go') then print('went') end end) w:open()",
            ),
            Frames(3),
            Click("Go"),
            Frames(3),
        ],
        "roblox_shop" => vec![
            Frames(2),
            Command("shop"),
            Frames(10),
            Click("Golden apple  —  12"),
            Frames(3),
            Click("Diamond sword  —  30"),
            Frames(3),
            Click("Diamond sword  —  30"),
            Frames(3),
            Click("Close (G)"),
            Frames(10),
        ],
        _ => panic!("no steps for example {id}"),
    }
}

#[test]
fn examples_do_what_they_did() {
    let mut failures = Vec::new();
    for ex in EXAMPLES {
        let mut ids = vec![ex.id];
        if ["uikit_demo", "viahorizon"].contains(&ex.id) {
            ids.push("uikit");
        }
        let got = session(&ids, &example_steps(ex.id));
        if let Err(e) = check(&format!("example_{}", ex.id), &got) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn all_examples_together() {
    let ids: Vec<&str> = EXAMPLES.iter().map(|e| e.id).collect();
    let steps = [
        Step::Command("coords"),
        Step::Command("zoom 3"),
        Step::Command("note remember this"),
        Step::Command("notes"),
        Step::Command("ores 8"),
        Step::Command("mine stone 1"),
        Step::Command("hello"),
        Step::Command("wp add here"),
        Step::Command("pos1"),
        Step::Command("outfit orbit"),
        Step::Command("statue"),
        Step::Frames(10),
        Step::Command("mine stop"),
        Step::Frames(4),
    ];
    check("all_examples", &session(&ids, &steps)).unwrap();
}

/// Console code that reaches the studio's functions, with their errors.
#[test]
fn console_sessions() {
    let mut s = Session::new(&[]);
    s.step(&Step::Frames(2));
    for code in CONSOLE {
        s.step(&Step::Lua(code));
        s.step(&Step::Frames(1));
    }
    s.step(&Step::Frames(30));
    check("console", &s.finish()).unwrap();
}

const CONSOLE: &[&str] = &[
    "1 + 2",
    "x = 21",
    "x * 2",
    "{1, 2, name = 'a', nested = {true, false}}",
    "player.pos()",
    "print('hi', 1, nil, true)",
    "error('boom')",
    "error({code = 1})",
    "local function f() local t = nil return t.x end f()",
    "nosuch()",
    "string.rep('ab', 3, ',')",
    "chat.show('shown')",
    "chat.say('said')",
    "game.run('time set day')",
    "hud.text(1, 2, 'x')",
    "on('tick', function() n = (n or 0) + 1 end)",
    "n",
    "every(0.1, function() m = (m or 0) + 1 if m == 3 then error('third') end end)",
    "after(0.2, function() print('later', n ~= nil) end)",
    "on('hud', function(w, h) hud.rect(1, 2, 3, 4, '#ff0000') hud.text(5, 6, 'hud ' .. w .. 'x' .. h) end)",
    "on('world', function() draw.line(0, 64, 0, 1, 65, 1, '#00ff00') draw.box(0, 64, 0, 1, 65, 1, '#0000ff', 0.1) end)",
    "command('twice', function(args) return args .. args end)",
    "storage.set('k', {1, 2, {a = 'b'}})",
    "storage.get('k')",
    "storage.set('bad', function() end)",
    "json.encode({a = 1, b = {1, 2}})",
    "json.decode('[1, 2, {\"x\": null}]')",
    "pcall(json.decode, '{nope')",
    "local ok, e = pcall(hud.text) return ok, e",
    "local t = setmetatable({}, {__index = function(t, k) return k .. '!' end}) return t.hey, rawget(t, 'hey')",
    "local co = 0 for i = 1, 10 do co = co + i end return co",
    "string.format('%5.2f|%-4d|%q|%x', math.pi, 7, 'a\\nb', 255)",
    "select('#', nil, nil)",
    "help('player.pos')",
    "type(help)",
    "keys.down('key.keyboard.j')",
    "game.screen()",
    "world.time()",
    "world.block(0, 63, 0)",
    "world.find('stone')",
    "view.zoom(2)",
    "local s = 0 for k, v in pairs({a = 1, b = 2, c = 3}) do s = s + v end return s",
    "tostring(nil) .. tostring(1.5) .. tostring(-0.0)",
    "math.floor(-3.5), math.ceil(-3.5), math.huge, -math.huge, math.maxinteger",
    "(function(...) return select('#', ...), ... end)(1, nil, 3)",
    "local t = {} t[1.0] = 'a' t[2] = 'b' return #t, t[1]",
    "while true do end",
    "local function r() return r() + 1 end r()",
    "setmetatable({}, {__tostring = function() return 'custom' end})",
    "tostring(setmetatable({}, {__tostring = function() return 'custom' end}))",
    "export({x = 1})",
    "import('nope')",
];

/// The snippets: each in a fresh interpreter, its printed lines and its
/// results or error, with the traceback.
#[test]
fn lua_snippets() {
    let mut out = String::new();
    for (i, (budget, src)) in SNIPPETS.iter().enumerate() {
        out.push_str(&format!("=== {i}\n{src}\n---\n"));
        let mut lua = Lua::new();
        lua.steps = *budget;
        let env = new_env(&mut lua);
        let f = match lua.load(src, "snip.lua", &env) {
            Ok(f) => f,
            Err(e) => {
                out.push_str(&format!("syntax: {e} (at byte {:?})\n", syntax_error_at()));
                continue;
            }
        };
        let mut host = NoHost::default();
        let r = lua.call(&mut host, &f, Vec::new());
        for line in &host.printed {
            out.push_str(&format!("print: {line}\n"));
        }
        // How deep recursion gets before "stack overflow" depends on the
        // build (natives calling back into Lua use the game's stack), so
        // the steps it took are not compared then.
        let mut deep = false;
        match r {
            Ok(vals) => {
                let vals: Vec<String> = vals.iter().map(|v| inspect(v, 3)).collect();
                out.push_str(&format!("ok: {}\n", vals.join(", ")));
            }
            Err(Throw::Fatal(m)) => out.push_str(&format!("fatal: {m}\n")),
            Err(e) => {
                out.push_str(&format!("error: {}\n", e.message()));
                if !e.traceback().is_empty() {
                    out.push_str(&format!("traceback:\n{}\n", e.traceback()));
                }
                deep = e.message().contains("stack overflow");
            }
        }
        if !deep {
            out.push_str(&format!("steps left: {}\n", lua.steps));
        }
    }
    check("snippets", &scrub(&out)).unwrap();
}

const B: u64 = 5_000_000;

const SNIPPETS: &[(u64, &str)] = &[
    (
        B,
        "return 1 + 2, 7 / 2, 7 // 2, -7 // 2, 7 % 3, -7 % 3, 2 ^ 10, 10 / 3, 1e100, 1/0, -1/0, 2^53, 0/0",
    ),
    (
        B,
        "return '10' + 5, '0x10' * 1, 3 .. '', -2 ^ 2, 5.5 % 2, -5.5 % 2, 5 % -3, 0x1F, .5, 3., 1e300 * 1e10",
    ),
    (
        B,
        "return 1 // 0, -1 // 0, 0 // 0, 1 % 0, 1 % math.huge, -1 % math.huge, 3 % -math.huge, 2^63, -2^63, 1e15, 1e16, 123456789012",
    ),
    (
        B,
        "return 5 & 3, 5 | 3, 5 ~ 3, ~0, 1 << 4, 256 >> 4, -1 >> 63, 1 << 64, 1 << -1, 2 >> -1, '6' & 3, 2.0 | 1",
    ),
    (B, "return 1.5 & 1"),
    (B, "local x = 2.5 return x | 1"),
    (B, "return {} | 1"),
    (
        B,
        "local t = setmetatable({}, {__band = function(a, b) return 'band' end, __bnot = function() return 'bnot' end, __shl = function() return 'shl' end}) return t & 1, ~t, 1 << t",
    ),
    (
        B,
        "print(\"a\\tb\\\\n\\65\\x41\\u{48}\", [[\nline1\nline2]], [==[a]]b]==], '\\z\n    joined', '\\0x', #'\\0\\0')",
    ),
    (B, "print('it''s')"),
    (
        B,
        "local x = 1 do local x = 2 print(x) end print(x) local x = x + 10 print(x)",
    ),
    (
        B,
        "local fs = {} for i = 1, 3 do fs[i] = function() return i end end print(fs[1](), fs[2](), fs[3]())\nlocal gs = {} local j = 0 while j < 3 do j = j + 1 local k = j gs[j] = function() k = k + 10 return k end end print(gs[1](), gs[1](), gs[2]())",
    ),
    (
        B,
        "local function counter() local n = 0 return function() n = n + 1 return n end, function() return n end end local inc, get = counter() inc() inc() print(get()) local a, b = counter() a() print(get(), b())",
    ),
    (
        B,
        "local function fib(n) if n < 2 then return n end return fib(n - 1) + fib(n - 2) end print(fib(20))",
    ),
    (
        B,
        "for i = 10, 1, -3 do io = (io or '') .. i .. ' ' end print(io) for i = 1, 3 do for j = 1, 3 do if j == 2 then break end print(i, j) end end local n = 0 repeat n = n + 1 local done = n >= 3 until done print(n)",
    ),
    (
        B,
        "for i = 1, 2, 0.5 do print(i) end for i = 1.0, 3 do print(i) end for i = 3, 1 do print('never') end",
    ),
    (B, "for i = 1, 0, 0 do end"),
    (B, "for i = 'a', 2 do end"),
    (B, "for i = 1, {} do end"),
    (B, "for i = 1, 2, nil do end"),
    (
        B,
        "local x = 5 if x > 10 then print('big') elseif x > 3 then print('mid') else print('small') end if nil then elseif false then else print('else') end",
    ),
    (
        B,
        "local function f(...) return select('#', ...), ... end print(f()) print(f(nil, nil)) print((f(1, 2, 3))) local t = {f(1, 2)} print(#t) local function g(...) local a, b = ... return a, b, select(2, ...) end print(g(7, 8, 9)) print(select(-1, 'a', 'b'))",
    ),
    (
        B,
        "local function f() return 1, 2, 3 end local t = {f(), f()} print(#t, t[4]) local a, b, c, d = f() print(a, b, c, d) print(f(), 'x') print((f()))",
    ),
    (
        B,
        "local t = {10, 20, 30, nil, 50} print(#t) local u = {} u[1] = 1 u[2] = 2 u[4] = 4 print(#u) local keys = {} for k, v in pairs({a = 1, b = 2, 3, 4, c = 5}) do keys[#keys + 1] = tostring(k) .. '=' .. tostring(v) end print(table.concat(keys, ' '))",
    ),
    (
        B,
        "local t = {} t[1] = 'a' t[1.0] = 'b' t[2^53] = 'c' print(t[1], #t, t[2^53]) t[-0.0] = 'z' print(t[0])",
    ),
    (B, "local t = {} t[nil] = 1"),
    (B, "local t = {} t[0/0] = 1"),
    (B, "return {[1] = 'a', [2] = 'b', 'c'}, {n = 1, [true] = 2, [1.5] = 3}"),
    (
        B,
        "local mt = {__index = function(t, k) return k .. '?' end, __newindex = function(t, k, v) rawset(t, k, v * 2) end} local t = setmetatable({}, mt) t.x = 5 print(t.x, t.y, rawget(t, 'y'))",
    ),
    (
        B,
        "local V = {} V.__index = V V.__add = function(a, b) return setmetatable({x = a.x + b.x}, V) end V.__eq = function(a, b) return a.x == b.x end V.__lt = function(a, b) return a.x < b.x end V.__le = function(a, b) return a.x <= b.x end V.__tostring = function(v) return 'V(' .. v.x .. ')' end V.__len = function() return 99 end V.__call = function(self, y) return self.x + y end V.__concat = function(a, b) return 'cat' end V.__unm = function(a) return setmetatable({x = -a.x}, V) end V.__mod = function() return 'mod' end V.__idiv = function() return 'idiv' end V.__div = function() return 'div' end V.__pow = function() return 'pow' end V.__mul = function() return 'mul' end V.__sub = function() return 'sub' end\nlocal a, b = setmetatable({x = 1}, V), setmetatable({x = 2}, V) print(tostring(a + b), a == b, a < b, a <= b, a > b, a >= b, #a, a(10), a .. b, a .. 'x', 1 .. a, tostring(-a), a % b, a // b, a / b, a ^ b, a * b, a - b, a ~= b)",
    ),
    (
        B,
        "local mt = {__eq = function() return true end} local a, b = setmetatable({}, mt), setmetatable({}, mt) print(a == b, a == 1, rawequal(a, b))",
    ),
    (
        B,
        "local mt = {__index = {inherited = 'yes'}} local base = setmetatable({}, mt) local derived = setmetatable({}, {__index = base}) print(derived.inherited, derived.nope)",
    ),
    (
        B,
        "local t = setmetatable({}, {}) getmetatable(t).__index = t print(t.loop)",
    ),
    (
        B,
        "local a = setmetatable({}, {__index = function(t, k) return t[k] end}) return a.x",
    ),
    (
        B,
        "local t = setmetatable({}, {__metatable = 'locked'}) print(getmetatable(t)) setmetatable(t, {})",
    ),
    (
        B,
        "local obj = {name = 'Rex'} function obj:speak(s) return self.name .. ' ' .. s end print(obj:speak('barks'), obj.speak(obj, 'x')) local Animal = {} Animal.__index = Animal function Animal.new(n) return setmetatable({n = n}, Animal) end function Animal:get() return self.n end local Dog = setmetatable({}, {__index = Animal}) Dog.__index = Dog function Dog.new(n) local d = Animal.new(n) return setmetatable(d, Dog) end function Dog:bark() return self:get() .. '!' end print(Dog.new('Fido'):bark())",
    ),
    (B, "local x\nprint(x.y)"),
    (B, "foo()"),
    (B, "local t = {}\n\nt.a.b = 1"),
    (B, "return 1 + {}"),
    (B, "local s = 'x' return s + 1"),
    (B, "return 1 + 'x'"),
    (B, "local t = {} return t.x + 1"),
    (B, "local t = {} return 1 - t.x"),
    (B, "local u = {} local function f() return u + 1 end return f()"),
    (B, "local u local function f() return u.x end return f()"),
    (B, "local u local function f() u() end f()"),
    (B, "local t = {} t.x.y()"),
    (B, "local t = {} t.f()"),
    (B, "local t = {} t[1]()"),
    (B, "local t = {} t:nope()"),
    (B, "local s = 5 s:upper()"),
    (B, "('x')()"),
    (B, "(1)()"),
    (B, "local f = 5 f()"),
    (B, "return {} < {}"),
    (B, "return 1 < 'x'"),
    (B, "return 'a' < 1"),
    (B, "return {} <= 1"),
    (B, "return nil > 1"),
    (B, "return #nil"),
    (B, "local n = 5 return #n"),
    (B, "return #glob"),
    (B, "return 'a' .. {}"),
    (B, "local t = {} return 'a' .. t.x"),
    (B, "local b = true return b .. 'x'"),
    (B, "return -{}"),
    (B, "local s = 'abc' return -s"),
    (B, "return -'2', - '-3'"),
    (B, "return ~1.5"),
    (
        B,
        "return not nil, not 0, nil and 1, false or 'x', 1 and 2, nil or false",
    ),
    (B, "local t = nil t.x = 1"),
    (B, "x.y.z = 1"),
    (B, "local s = 'str' s.x = 1"),
    (B, "error('boom')"),
    (B, "error({code = 1})"),
    (B, "error('plain', 0)"),
    (B, "error()"),
    (B, "error(nil)"),
    (B, "error(42)"),
    (
        B,
        "local function check(x) if not x then error('x is required', 2) end end\n\ncheck(nil)",
    ),
    (
        B,
        "local function a() error('lvl3', 3) end local function b() a() end\nlocal function c()\n b()\nend\nc()",
    ),
    (B, "local function a() error('lvl9', 9) end a()"),
    (B, "assert(false)"),
    (B, "assert(nil, 'custom message')"),
    (B, "assert(false, {1})"),
    (B, "print(assert(1, 2, 3))"),
    (B, "assert()"),
    (
        B,
        "local ok, err = pcall(function() error('bad') end) print(ok, err) local ok2, v = pcall(function() return 7, 8 end) print(ok2, v) local ok3, obj = pcall(error, {code = 42}) print(ok3, obj.code) print(select('#', pcall(error))) print(xpcall(function() error('deep') end, function(m) return 'handled: ' .. m end)) print(pcall(nil)) local ok4, msg = pcall(function() local t = nil; return t.x end) print(msg)",
    ),
    (
        B,
        "print(pcall(pcall, error, 'x')) print(pcall(error)) print(xpcall(error, function(m) return m end, 'arg')) print(xpcall(function(...) return ... end, print, 1, 2))",
    ),
    (
        B,
        "print(xpcall(function() error('a') end, function(m) error('in handler') end))",
    ),
    (B, "pcall()"),
    (B, "xpcall(print)"),
    (
        B,
        "local function inner() error('deep') end\nlocal function outer() inner() end\nouter()",
    ),
    (
        B,
        "local t = {}\nfunction t.method()\n  error('in method')\nend\nfunction t:other()\n  t.method()\nend\nt:other()",
    ),
    (B, "local f = function()\n  local x = nil\n  return x.field\nend\nf()"),
    (
        B,
        "local function rec(n)\n  if n == 0 then error('bottom') end\n  return rec(n - 1)\nend\nrec(30)",
    ),
    (B, "local function f() return 1 + f() end f()"),
    (B, "local function f() f() end f()"),
    (B, "print(pcall(function() local function f() f() end f() end))"),
    (
        B,
        "local depth = 0 local function f() depth = depth + 1 f() end pcall(f) print(depth)",
    ),
    (
        B,
        "local t = setmetatable({}, {__index = function(t, k) return t[k .. 'x'] end}) return t.a",
    ),
    (
        B,
        "local t = {3, 1, 2} table.sort(t, function(a, b)\n  return a.x < b\nend)",
    ),
    (
        B,
        "local t = {3, 1, 2} table.sort(t, function(a, b) return a > b end) print(table.concat(t, ',')) table.sort(t) print(table.concat(t, ',')) local w = {'b', 'a', 'C'} table.sort(w) print(table.concat(w))",
    ),
    (B, "table.sort({1, 'x', 2})"),
    (
        B,
        "print(('hello'):upper(), string.sub('hello', 2, -2), ('abc'):rep(3, '-'), ('abc'):reverse(), ('abc'):len(), ('abc'):byte(1, -1), string.char(72, 105))",
    ),
    (
        B,
        "print(string.format('%5.2f|%-5d|%05d|%x|%X|%o|%e|%g|%g|%s|%q|%c|%%|%10.3s|', 3.14159, 42, 42, 255, 255, 8, 12345.678, 0.0001, 1e20, nil, 'a\\n\"b', 65, 'abcdef'))",
    ),
    (B, "print(string.format('%d', 3.5))"),
    (B, "print(string.format('%d'))"),
    (B, "print(string.format('%y', 1))"),
    (
        B,
        "print(('a,b,,c'):find(',', 1, true), ('hello world'):find('o w'), ('hello'):find('l+'), ('abc'):match('(a)(b)(c)'), ('key=val'):match('(%w+)=(%w+)'), ('  trim  '):match('^%s*(.-)%s*$'), ('x'):find(''), ('abc'):find('b', -1))",
    ),
    (
        B,
        "local out = {} for w in ('one two  three'):gmatch('%a+') do out[#out + 1] = w end print(table.concat(out, '|')) for k, v in ('a=1, b=2'):gmatch('(%w+)=(%w+)') do print(k, v) end print(('hello world'):gsub('o', '0')) print(('hello'):gsub('l', {l = 'L'})) print(('abc'):gsub('%w', function(c) return c:upper() .. '.' end)) print(('abc'):gsub('', '-')) print(('x y z'):gsub('%s', '', 1)) print(('%d'):gsub('%%', '%%%%'))",
    ),
    (B, "print(('abc'):gsub('(b)', '%2'))"),
    (B, "print(('abc'):find('[a'))"),
    (
        B,
        "print(('f(a(b)c)d'):match('%b()'), ('THE (quick) fox'):find('%f[%a]%a+'), ('hello'):match('.-(l+)(.*)'), ('2024-01-15'):match('(%d+)-(%d+)-(%d+)'))",
    ),
    (
        B,
        "print(string.rep('x', 0), string.rep('ab', 3), ('x'):rep(-1), tostring(12):len(), (12):rep(2))",
    ),
    (B, "return string.rep('x', 1e9)"),
    (B, "local s = 'x' while true do s = s .. s end"),
    (
        B,
        "print(math.max(3, 7, 1), math.min(3, 7, 1), math.abs(-4), math.floor(3.7), math.ceil(3.2), math.sqrt(16), math.huge, -math.huge, math.pi, math.fmod(7, 3), math.fmod(-7, 3), math.modf(3.7), math.tointeger(3.0), math.tointeger(3.5), math.type(1), math.type(1.5), math.type('1'))",
    ),
    (B, "print(math.max())"),
    (
        B,
        "math.randomseed(42) local a = math.random(1, 100) math.randomseed(42) print(a == math.random(1, 100), math.random() < 1, math.random(5) <= 5)",
    ),
    (B, "print(math.random(0))"),
    (
        B,
        "local t = {1, 2, 3} table.insert(t, 4) table.insert(t, 1, 0) print(table.concat(t, ',')) print(table.remove(t), table.remove(t, 1), table.concat(t, ',')) print(table.unpack({1, 2, 3})) print(table.unpack({1, 2, 3}, 2)) print(select('#', table.unpack({}, 1, 3))) local p = table.pack(1, nil, 3) print(p.n, p[3]) print(table.concat({}, ','), table.concat({1, 'a', 2.5}, '-', 2, 3))",
    ),
    (B, "table.insert({}, 5, 1)"),
    (B, "table.concat({1, {}, 3})"),
    (B, "table.insert(nil, 1)"),
    (
        B,
        "print(type(nil), type(1), type('s'), type({}), type(print), type(function() end), type(type))",
    ),
    (B, "type()"),
    (
        B,
        "print(tostring(1e15), tostring(1e16), tostring(-0.0), tostring(1/3), 2^31, 2^63, math.mininteger, 1e-5, 123.456)",
    ),
    (
        B,
        "print(tonumber('10'), tonumber('0x1p4'), tonumber('  12  '), tonumber('12a'), tonumber('ff', 16), tonumber('zz', 36), tonumber('8', 8), tonumber(nil), tonumber('1e2'), tonumber(''), tonumber('-0x10'))",
    ),
    (B, "print(tonumber('10', 99))"),
    (
        B,
        "print(rawlen({1, 2}), rawlen('abc'), rawequal('a', 'a'), rawget({5}, 1), next({}), next({7}), select('#', next({})))",
    ),
    (B, "next({}, 'nokey')"),
    (B, "local t = {a = 1} for k in pairs(t) do t.b = 2 end"),
    (
        B,
        "for i, v in ipairs({1, 2, nil, 4}) do print(i, v) end for k, v in pairs(setmetatable({}, {__pairs = function(t) return function(_, k) if not k then return 1, 'one' end end, t, nil end})) do print(k, v) end",
    ),
    (B, "ipairs()"),
    (B, "for k in pairs(nil) do end"),
    (B, "for x in 5 do end"),
    (B, "for x in nil do end"),
    (
        B,
        "local f = load('return 1 + ...') print(f(41)) print(load('syntax error here')) print(load('error(\"in load\")', '=loaded')) local g = load('x = 5') g() print(x) print(pcall(load('error(\"in load\")', '=loaded')))",
    ),
    (
        B,
        "local f = load('return function() error(\"inner\") end', '@chunkname')() f()",
    ),
    (
        B,
        "print(dump({1, 2, x = {y = 3}}), inspect('s'), dump(nil), dump(setmetatable({}, {})))",
    ),
    (
        B,
        "print(os.difftime(10, 4), collectgarbage('count'), collectgarbage(), type(os.time()), type(os.clock()), os.date('%Y', 0), os.date('!%H:%M:%S', 3661), os.time({year = 2020, month = 1, day = 1, hour = 0}))",
    ),
    (
        B,
        "print(utf8.char(72, 228, 8364, 128512), utf8.len('häl€'), utf8.codepoint('h€', 2), utf8.offset('a€b', 3), utf8.len('a\\xffb'), utf8.charpattern) for p, c in utf8.codes('aé') do print(p, c) end",
    ),
    (
        B,
        "print(math.rad(180) == math.pi, math.deg(math.pi), math.ult(1, -1), math.atan2(1, 1) == math.atan(1, 1), math.pow(2, 10), math.log10(1000), math.ldexp(0.5, 4), math.frexp(8), math.log(8, 2), math.exp(0), math.sinh(0), math.cosh(0), math.tanh(0))",
    ),
    (
        B,
        "local t = {1, 2, 3, 4, 5} table.move(t, 2, 4, 1) print(table.concat(t, ',')) print(table.concat(table.move({1, 2, 3}, 1, 3, 3, {9, 9}), ','))",
    ),
    (
        B,
        "print(('a b c'):split(), ('a,b'):split(','), ('  x '):trim(), ('hello'):startswith('he'), ('hello'):endswith('lo'), #('a1b2c3'):split('%d', false))",
    ),
    (
        B,
        "local Inventory = {} Inventory.__index = Inventory function Inventory.new() return setmetatable({items = {}, count = 0}, Inventory) end function Inventory:add(name, n) self.items[name] = (self.items[name] or 0) + (n or 1) self.count = self.count + (n or 1) return self end function Inventory:sorted() local names = {} for name in pairs(self.items) do names[#names + 1] = name end table.sort(names) local parts = {} for _, name in ipairs(names) do parts[#parts + 1] = string.format('%s=%d', name, self.items[name]) end return '{' .. table.concat(parts, ', ') .. '}' end local inv = Inventory.new():add('stone', 32):add('dirt'):add('stone', 32) print(inv:sorted(), inv.count)",
    ),
    (B, "local s = 0 for i = 1, 100000 do s = s + i % 7 end return s"),
    (
        B,
        "local t = {} for i = 1, 1000 do t[i] = i * i end local s = 0 for _, v in ipairs(t) do s = s + v end for k, v in pairs(t) do s = s - v end return s, #t",
    ),
    (
        B,
        "local parts = {} for i = 1, 100 do parts[#parts + 1] = i .. '' end return #table.concat(parts)",
    ),
    (1000, "local n = 0\nwhile true do\n  n = n + 1\nend"),
    (1001, "local n = 0\nwhile true do\n  n = n + 1\nend"),
    (777, "local n = 0\nrepeat\n  n = n + 1\n  local m = n\nuntil false"),
    (
        500,
        "local function f()\n  return 1\nend\nfor i = 1, 1e9 do\n  f()\nend",
    ),
    (
        501,
        "for k, v in pairs({1, 2, 3}) do end\nlocal t = {}\nfor i = 1, 1e9 do\n  t[1] = i\nend",
    ),
    (300, "pcall(function() while true do end end) print('escaped')"),
    (B, "goto done"),
    (B, "x = = 1"),
    (B, "if x then\n  print(1)\n"),
    (B, "print('abc)"),
    (B, "break"),
    (B, "x = 1 +"),
    (B, "f() = 1"),
    (B, "x"),
    (B, "local x <close> = 1"),
    (B, "local x <const> = 1 return x"),
    (B, "return ..."),
    (B, "function f() return ... end"),
    (B, "local t = {1, 2, 3,} local u = {a = 1; b = 2} return #t, u.b"),
    (B, "local a, b, c = (function() return 1, 2, 3 end)() return a, b, c"),
    (B, "local a, b = 1 return a, b"),
    (B, "local a = 1, 2 return a"),
    (B, "a, b, c = 1, 2 return a, b, c"),
    (B, "local t = {} t.x, t.y = 1 return t.x, t.y"),
    (B, "local i = 1 local t = {} i, t[i] = i + 1, 20 return i, t[1], t[2]"),
    (B, "local a, b = 1, 2 a, b = b, a return a, b"),
    (
        B,
        "local function f(a, b, ...) return a, b, ... end return f(1), f(1, 2, 3, 4)",
    ),
    (B, "return (...)"),
    (
        B,
        "local s = '' for k, v in next, {10, 20} do s = s .. k .. v end return s",
    ),
    (B, "local t = {} t[#t + 1] = 'a' t[#t + 1] = 'b' return t"),
    (
        B,
        "return 2^-1, 8 // 3.0, 7.5 // 2, -0.0, 0.1 + 0.2, 100 // 1e308, 3 % -2.5",
    ),
    (
        B,
        "return 'abc' < 'abd', 'Z' < 'a', '' < 'a', 'a' <= 'a', 10 == '10', 1 == 1.0",
    ),
    (
        B,
        "local a = {} local b = a return a == b, {} == {}, print == print, 'x' == 'x'",
    ),
    (
        B,
        "local t = setmetatable({}, {__call = function(self, a, b) return a + b end}) return t(1, 2), pcall(t, 3, 4)",
    ),
    (B, "local t = setmetatable({}, {__call = 5}) t()"),
    (
        B,
        "local t = setmetatable({}, {__index = string}) return t.format('%d', 5)",
    ),
    (
        B,
        "return ('x'):len(), #'xyz', ('abc'):sub(-2), ('abc'):sub(0), ('abc'):sub(5), ('abc'):sub(2, 1)",
    ),
    (B, "local str = 'hi' return str.len"),
    (B, "local n = 1 return n.x"),
    (B, "return (nil)[1]"),
    (B, "print(nil .. 'x')"),
    (B, "local t = {f = function() return 1, 2 end} return t.f(), (t.f())"),
    (B, "local t = {} function t.a.b() end"),
    (
        B,
        "local mt = {} mt.__newindex = mt local t = setmetatable({}, mt) t.x = 1 return rawget(t, 'x'), mt.x",
    ),
    (
        B,
        "local log = {} local t = setmetatable({}, {__newindex = function(t, k, v) log[#log + 1] = k end}) t.a = 1 t.b = 2 t.a = 3 return table.concat(log)",
    ),
    (
        B,
        "local t = setmetatable({}, {__len = function() return 'len' end, __unm = function() return 'unm' end}) return #t, -t",
    ),
    (
        B,
        "local t = setmetatable({}, {__concat = function(a, b) return type(a) .. type(b) end}) return t .. 1, 1 .. t, 'a' .. 'b' .. t",
    ),
    (
        B,
        "local t = setmetatable({}, {__lt = function() return 1 end}) return t < t, t > t",
    ),
    (
        B,
        "local t = setmetatable({}, {__le = function() return nil end}) return t <= t, t >= t",
    ),
    (
        B,
        "local t = setmetatable({}, {__tostring = function() return 5 end}) return tostring(t)",
    ),
    (
        B,
        "local t = setmetatable({}, {__tostring = function() return {} end}) return tostring(t)",
    ),
    (
        B,
        "local mt = {__index = function(t, k) error('no field ' .. k) end} local t = setmetatable({}, mt)\nlocal v = t.missing",
    ),
    (
        B,
        "local t = setmetatable({}, {__add = function(a, b)\n  error('add failed')\nend})\nreturn t + 1",
    ),
    (
        B,
        "print(string.format('%s %s', setmetatable({}, {__tostring = function() return 'T' end}), 1.5))",
    ),
    (B, "print(#arg)"),
    (B, "print(_VERSION, _G == _G._G, type(_G.print))"),
    (
        B,
        "local t = {} for i = 1, 20 do t[i] = function() return i end end local s = 0 for i = 1, 20 do s = s + t[i]() end return s",
    ),
    (
        B,
        "local function outer() local x = 0 local function mid() local function inner() x = x + 1 return x end return inner end return mid() end local f = outer() f() return f()",
    ),
    (
        B,
        "local a = 1 local function f() a = a + 1 local a = 10 return a end return f(), a",
    ),
    (
        B,
        "local s = 0 for i = 1, 10 do local j = i * 2 if j > 10 then break end s = s + j end return s",
    ),
    (
        B,
        "local i = 0 while i < 5 do i = i + 1 if i == 3 then break end end return i",
    ),
    (
        B,
        "local r = {} for i = 1, 3 do local x x = (x or 0) + i r[i] = x end return r",
    ),
    (
        B,
        "local x = 0 local function g() return x end for i = 1, 3 do x = i end return g()",
    ),
    (
        B,
        "do local a = 1 function getA() return a end end a = 5 return getA(), a",
    ),
    (B, "local t = {} t.self = t return t.self.self.self == t"),
    (B, "return select(2, 'a', 'b', 'c'), select(-2, 'a', 'b', 'c')"),
    (B, "return select(0)"),
    (B, "return select(5, 1)"),
    (
        B,
        "local function v(...) local t = {...} return #t, ... end return v(1, 2, 3)",
    ),
    (
        B,
        "local function v(...) return {n = select('#', ...), ...} end return v(nil, 2)",
    ),
    (B, "return math.floor(2^62) == 2^62, 7 // 0.0, -7 % 0.0, math.abs(-0.0)"),
    (
        B,
        "local t = {} for i = 1, 10 do t[i] = i end for i = 1, 10, 2 do t[i] = nil end local n = 0 for k in pairs(t) do n = n + 1 end return n, #t",
    ),
    (
        B,
        "local t = {n = 0} function t:inc(by) self.n = self.n + (by or 1) return self end t:inc():inc(5) return t.n",
    ),
    (
        B,
        "local s = 'a' local function f() return s .. 'b' end s = 'c' return f()",
    ),
    (B, "return string.byte('A'), ('%d'):format(3), ('x'):rep(3):upper()"),
    (B, "local f = string.gmatch('a1b2', '%a(%d)') return f(), f(), f()"),
    (B, "return tostring(print):sub(1, 18), tostring({}):sub(1, 6)"),
    (
        B,
        "local t = {} t[{}] = 1 t[print] = 2 t[true] = 3 local n = 0 for k, v in pairs(t) do n = n + v end return n",
    ),
    (B, "return #{n = 1}, #{1, 2, nil}, #{nil, nil, 3}"),
    (
        B,
        "local t = setmetatable({}, {__index = function(_, k) return k * 2 end}) local s = 0 for i = 1, 5 do s = s + t[i] end return s",
    ),
    (
        B,
        "return 1 == 1, 1 ~= 2, not (1 == 2), 2 > 1 and 'yes' or 'no', nil == false",
    ),
    (B, "local co = 'coroutine' return type(coroutine)"),
    (
        B,
        "return string.format('%.3f %5s %-5s| %+d %i', 1/3, 'ab', 'ab', 5, 7)",
    ),
    (
        B,
        "local t = {} for i = 1, 3 do t[#t + 1] = function() return i end end return t[1]() + t[3]()",
    ),
    (B, "local a <const>, b = 1, 2 return a + b"),
    (
        B,
        "while false do end repeat until true for _ in pairs({}) do end do end return 'empty bodies'",
    ),
    (B, "local x = 1\nlocal y = x +\n  nil\n"),
    (B, "local t = {}\nlocal v = t.a\n  .b\n  .c"),
    (B, "print(\n  1,\n  nil + 1\n)"),
    (B, "local f = function() end\nf(\n  undefined_global()\n)"),
    (
        B,
        "local i = 0\nwhile i < 3 do\n  i = i + 1\n  local q = i\nend\nwhile i.x do end",
    ),
    (
        B,
        "local n = 0\nwhile n < 2 do\n  n = n + 1\n  local z = 1\nend\nlocal t\nwhile n do\n  n = nil\n  local after = 1\n  t = t.x\nend",
    ),
    (B, "local k = 0\nrepeat\n  k = k + 1\n  local last = k\nuntil last.x"),
    (
        B,
        "local t = {1}\nfor i, v in ipairs(t) do\n  local z = v\nend\nfor k, v in next, {1}, 'bad' do\nend",
    ),
    (
        B,
        "local data = {1, 2}\nfor k, v in pairs(data) do\n  data.extra = true\n  local z = 1\nend",
    ),
    (
        B,
        "local f = setmetatable({}, {__call = function(self, x)\n  return x.y\nend})\nf(nil)",
    ),
    (B, "local t = {}\nt[1] = function() error('from array') end\nt[1]()"),
    (
        B,
        "local t = {}\nfunction t.a()\n  return t.b()\nend\nfunction t.b()\n  return t.c()\nend\nfunction t.c()\n  error('from c')\nend\nreturn t.a()",
    ),
    (
        B,
        "local function a(n)\n  if n == 0 then\n    error('end')\n  end\n  a(n - 1)\nend\na(20)",
    ),
    (
        B,
        "local ok, e = pcall(error, setmetatable({}, {__tostring = function() return 'E!' end})) return ok, tostring(e)",
    ),
    (B, "return pcall(function() return string.rep('x', 1e9) end)"),
    (
        B,
        "local t = {} for i = 1, 10 do t[i] = tostring(i) end return table.concat(t, ', ', 3, 7)",
    ),
    (
        B,
        "local x = 10 local function shadow(x) x = x + 1 return x end return shadow(1), x",
    ),
    (
        B,
        "return (function() local t = {} for i = 1, 5 do t[i] = i end return #t, t[5] end)()",
    ),
    (
        B,
        "local mt = {} mt.__index = function(t, k) return mt[k] end mt.foo = 'bar' local t = setmetatable({}, mt) return t.foo",
    ),
    (
        B,
        "return math.min(1), math.max(2, 2.5), math.floor(-0.5), 7 // -2, -7 % -3",
    ),
    (
        B,
        "local t = {10, 20, 30} local i = 1 local r = {} while t[i] do r[#r + 1] = t[i] * 2 i = i + 1 end return table.concat(r, ' ')",
    ),
    (B, "local _ENV = 5 return 1"),
    (
        B,
        "print(string.find('abc', 'b', 10), string.find('', ''), string.find('abc', '', 10), string.match('  x', '^%s*()'))",
    ),
    (
        B,
        "return string.format('%q', 1/0), string.format('%q', 5), string.format('%5.1s|', 'xyz')",
    ),
];
