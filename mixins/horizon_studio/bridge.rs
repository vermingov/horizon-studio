// Horizon Studio inside the game: the one module that knows Horizon's
// `Game`. It keeps the studio in a thread local (the window's thread),
// implements `GameApi` over the game, and has one function per hook of
// horizon-mixin.toml. Hooks that find the studio busy (a script's agent
// command can reach the game's own command path) do nothing.

use super::hstudio::{Camera, Corner, GameApi, ID, NAME, OPEN_KEY, PlayerInfo, Reply, Studio, VERSION};
use super::hstudio_ui::{Input, Key, Mods, UiResult, draw, draw_screen, handle, handle_screen};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::extras::Corner as GameCorner;
use horizon_client::extras_hud::Corners;
use horizon_client::hud::HudBatch;
use horizon_client::mesher::Vertex;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

thread_local! {
    static STUDIO: RefCell<Option<Studio>> = const { RefCell::new(None) };
    static LAST_FRAME: Cell<Option<Instant>> = const { Cell::new(None) };
    // What the window-event hook needs without the game: whether the studio
    // is open, its key, the GUI scale and the modifier keys; and what it
    // queues for the frame hook.
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static OPEN_KEY_NAME: RefCell<String> = RefCell::new(OPEN_KEY.to_owned());
    static SCALE: Cell<f32> = const { Cell::new(1.0) };
    static MODS: Cell<Mods> = const { Cell::new(Mods { ctrl: false, shift: false, alt: false }) };
    static TOGGLE: Cell<bool> = const { Cell::new(false) };
    static INPUTS: RefCell<Vec<Input>> = const { RefCell::new(Vec::new()) };
    /// Files dropped on the window, to install as script mods.
    static DROPPED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with the studio, unless it is busy (a hook reached from a script).
fn with<R>(f: impl FnOnce(&mut Studio) -> R) -> Option<R> {
    STUDIO.with(|s| {
        let mut slot = s.try_borrow_mut().ok()?;
        Some(f(slot.get_or_insert_with(Studio::new)))
    })
}

enum GameRef<'h, 'a> {
    Mut(&'h mut Game<'a>),
    Shared(&'h Game<'a>),
}

/// The game as the studio sees it, for one hook call.
struct Host<'h, 'a> {
    game: GameRef<'h, 'a>,
    event_loop: Option<&'h ActiveEventLoop>,
    batch: Option<&'h mut HudBatch>,
    corners: Option<&'h mut Corners>,
    /// While the world draws: its geometry, and the frame's own camera.
    world: Option<(&'h mut Vec<Vertex>, Camera)>,
}

impl<'h, 'a> Host<'h, 'a> {
    fn new(game: &'h mut Game<'a>) -> Self {
        Host {
            game: GameRef::Mut(game),
            event_loop: None,
            batch: None,
            corners: None,
            world: None,
        }
    }
    fn g(&self) -> &Game<'a> {
        match &self.game {
            GameRef::Mut(g) => g,
            GameRef::Shared(g) => g,
        }
    }
    fn g_mut(&mut self) -> Option<&mut Game<'a>> {
        match &mut self.game {
            GameRef::Mut(g) => Some(g),
            GameRef::Shared(_) => None,
        }
    }
}

fn game_corner(c: Corner) -> GameCorner {
    match c {
        Corner::TopLeft => GameCorner::TopLeft,
        Corner::TopRight => GameCorner::TopRight,
        Corner::BottomLeft => GameCorner::BottomLeft,
        Corner::BottomRight => GameCorner::BottomRight,
    }
}

/// The window's size in GUI pixels and the GUI's scale.
fn gui(game: &Game<'_>) -> (f32, f32, f32) {
    let size = game
        .renderer
        .as_ref()
        .map_or([1280, 720], |r| [r.config.width, r.config.height]);
    let b = HudBatch::new(size);
    (b.width, b.height, b.scale)
}

impl GameApi for Host<'_, '_> {
    fn chat(&mut self, text: &str) {
        if let Some(g) = self.g_mut() {
            g.native_chat(text);
        }
    }
    fn send(&mut self, line: &str) {
        if let Some(g) = self.g_mut() {
            g.send_chat(line);
        }
    }
    fn player(&mut self) -> Option<PlayerInfo> {
        let base = self.g().native_player()?;
        let g = self.g();
        let p = g.player.as_ref()?;
        let held = g
            .inventory
            .get(g.selected)
            .and_then(|s| s.as_ref())
            .map(|s| (s.item.to_string(), s.count));
        let mut info = PlayerInfo {
            x: base.x,
            y: base.y,
            z: base.z,
            yaw: base.yaw,
            pitch: base.pitch,
            dimension: base.dimension.to_owned(),
            health: p.health.health(),
            max_health: 20.0,
            food: p.health.food.food,
            saturation: p.health.food.saturation,
            level: g.experience.level,
            mode: g.mode.name().to_owned(),
            slot: g.selected,
            held,
        };
        if let Some(g) = self.g_mut()
            && let Some(p) = g.player.as_mut()
            && let Ok(max) = p.attribute("minecraft:max_health")
        {
            info.max_health = max as f32;
        }
        Some(info)
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        let g = self.g();
        g.player.as_ref()?;
        let state = g.world.block(x, y, z);
        Some(g.res.facts().get(state).name.to_string())
    }
    fn clock(&self) -> Option<(i64, i64)> {
        let g = self.g();
        g.player.as_ref()?;
        Some((g.world.day_time, g.world.game_time))
    }
    fn ticks(&self) -> u64 {
        self.g().ticks
    }
    fn fps(&self) -> [u32; 3] {
        let (a, b, c) = self.g().native_frame_rate();
        [a, b, c]
    }
    fn mapping(&mut self, name: &str) -> Option<(bool, String)> {
        self.g_mut()?.native_mapping(name)
    }
    fn screen(&self) -> String {
        self.g().screen_label().to_owned()
    }
    fn agent(&mut self, line: &str) -> Result<Reply, String> {
        let el = self.event_loop.ok_or("the agent is not available here")?;
        let g = self.g_mut().ok_or("the agent is not available here")?;
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            return Err("empty agent command".into());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        match g.agent_command(&words, line, &tx, el) {
            Ok(super::agent::Handled::Reply(v)) => Ok(Reply::Now(v)),
            Ok(super::agent::Handled::Deferred) => Ok(Reply::Later(rx)),
            Ok(super::agent::Handled::No) => Err(format!("unknown agent command '{}'", words[0])),
            Err(e) => Err(e.to_string()),
        }
    }
    fn data_dir(&self) -> PathBuf {
        self.g().native_data_dir(ID)
    }
    fn clipboard(&mut self) -> String {
        self.g_mut().map(|g| g.clipboard.get()).unwrap_or_default()
    }
    fn set_clipboard(&mut self, text: &str) {
        if let Some(g) = self.g_mut() {
            g.clipboard.set(text);
        }
    }
    fn gui_size(&self) -> (f32, f32) {
        match &self.batch {
            Some(b) => (b.width, b.height),
            None => {
                let (w, h, _) = gui(self.g());
                (w, h)
            }
        }
    }
    fn text_width(&self, text: &str) -> f32 {
        self.g().native_text_width(text)
    }
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        if let Some(b) = self.batch.as_deref_mut() {
            game.native_fill(b, rect, argb);
        }
    }
    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32, shadow: bool) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        if let Some(b) = self.batch.as_deref_mut() {
            game.native_text(b, text, at, rgb, shadow);
        }
    }
    fn open_folder(&mut self, path: &std::path::Path) {
        super::packs::open_folder(path);
    }
    fn camera(&self) -> Option<Camera> {
        if let Some((_, cam)) = &self.world {
            return Some(*cam);
        }
        let (pos, yaw, pitch, fov) = self.g().locator_camera?;
        Some(Camera { pos, yaw, pitch, fov })
    }
    fn world_quad(&mut self, corners: [[f64; 3]; 4], argb: u32) {
        let white = match &self.game {
            GameRef::Mut(g) => g.gui.atlas.font.white(),
            GameRef::Shared(g) => g.gui.atlas.font.white(),
        };
        let Some((out, cam)) = self.world.as_mut() else {
            return;
        };
        let uv = [
            [white.u0, white.v1],
            [white.u1, white.v1],
            [white.u1, white.v0],
            [white.u0, white.v0],
        ];
        // RGBA8 in memory: red in the low byte.
        let [a, r, g, b] = argb.to_be_bytes();
        let color = u32::from_le_bytes([r, g, b, a]);
        let at = |p: [f64; 3]| {
            [
                (p[0] - cam.pos[0]) as f32,
                (p[1] - cam.pos[1]) as f32,
                (p[2] - cam.pos[2]) as f32,
            ]
        };
        // Full brightness, unlit, both faces (the entity pipelines cull nothing here).
        let light = 240 | (240 << 8);
        for order in [[0, 1, 2, 3], [3, 2, 1, 0]] {
            for (k, &i) in order.iter().enumerate() {
                out.push(Vertex {
                    pos: at(corners[i]),
                    uv: uv[k],
                    layer: 0,
                    color,
                    light,
                    normal: [0; 4],
                    glint_uv: [0.0, 0.0],
                });
            }
        }
    }
    fn quad(&mut self, corners: [[f32; 2]; 4], argb: u32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        if let Some(b) = self.batch.as_deref_mut() {
            let [a, r, g, bl] = argb.to_be_bytes();
            b.fill_quad(&game.gui.atlas, corners, horizon_client::hud::rgba(r, g, bl, a));
        }
    }
    fn sound(&mut self, id: &str, at: Option<[f64; 3]>, volume: f32, pitch: f32) -> Result<(), String> {
        let g = self.g_mut().ok_or("sounds can't play here")?;
        g.audio.play(id, at, volume, pitch);
        Ok(())
    }
    fn particle(&mut self, id: &str, at: [f64; 3], velocity: [f64; 3], color: Option<[f32; 4]>) -> Result<(), String> {
        let ty =
            horizon_client::particles::ParticleType::from_name(id).ok_or_else(|| format!("unknown particle {id}"))?;
        let name = ty.name();
        if matches!(
            name,
            "block"
                | "block_crumble"
                | "falling_dust"
                | "dust_pillar"
                | "block_marker"
                | "item"
                | "vibration"
                | "trail"
        ) {
            return Err(format!(
                "{name} needs a block or an item, which world.particle doesn't take"
            ));
        }
        let mut options = horizon_client::particles::ParticleOptions::simple(ty);
        if let Some(c) = color {
            options.color = c;
        }
        let g = self.g_mut().ok_or("particles can't spawn here")?;
        g.particles.add(&options, at, velocity);
        Ok(())
    }
    fn place(&mut self, corner: Corner, w: f32, h: f32) -> (f32, f32) {
        match self.corners.as_deref_mut() {
            Some(c) => c.place(game_corner(corner), w, h),
            None => (2.0, 2.0),
        }
    }
}

/// Makes way for the studio's screen or a script's: the chat screen frees
/// the pointer and keeps the world running, and the studio takes its input.
fn make_way(game: &mut Game<'_>) -> bool {
    if game.screen == Screen::Chat {
        return true;
    }
    if game.screen != Screen::Playing || game.player.is_none() {
        return false;
    }
    game.open_chat("");
    game.screen == Screen::Chat
}

/// Hands the game back its screen.
fn give_back(game: &mut Game<'_>) {
    if game.screen == Screen::Chat {
        game.close_chat();
    }
}

// ----------------------------------------------------------------- hooks

/// Each frame, with the event loop (poll_control): the input the window
/// hook queued, the studio's frame, and opening or closing it.
pub(super) fn frame(game: &mut Game<'_>, event_loop: &ActiveEventLoop) {
    if !game.native_window() {
        return;
    }
    let now = Instant::now();
    let dt = LAST_FRAME
        .with(|l| l.replace(Some(now)))
        .map_or(0.0, |t| now.duration_since(t).as_secs_f64());
    let inputs = INPUTS.with(|q| std::mem::take(&mut *q.borrow_mut()));
    let toggle = TOGGLE.with(|t| t.replace(false));
    let dropped = DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()));
    with(|st| {
        if !game.native_on(ID) {
            if st.overlay_open() {
                st.s.ui.open = false;
                st.s.screen = None;
                give_back(game);
            }
            OPEN.with(|o| o.set(false));
            return;
        }
        SCALE.with(|s| s.set(gui(game).2));
        let key = game.native_mod_key(ID, "open", OPEN_KEY);
        OPEN_KEY_NAME.with(|k| *k.borrow_mut() = key);
        // The game closed the chat screen itself (death, a disconnect).
        if st.overlay_open() && game.screen != Screen::Chat {
            st.s.ui.open = false;
            st.s.screen = None;
        }
        let was_open = st.overlay_open();
        {
            let mut host = Host::new(game);
            host.event_loop = Some(event_loop);
            st.frame(&mut host, dt);
            for path in dropped {
                let line = match st.install_path(&mut host, &path) {
                    Ok(line) => line,
                    Err(e) => format!("Could not install it: {e}"),
                };
                host.chat(&format!("[{NAME}] {line}"));
            }
            for input in inputs {
                if !st.overlay_open() {
                    break;
                }
                if st.s.screen.is_some() {
                    if handle_screen(st, &mut host, input) == UiResult::Close {
                        st.close_screen(&mut host);
                    }
                } else if handle(st, &mut host, input) == UiResult::Close {
                    st.s.ui.open = false;
                }
            }
        }
        // A script's ui.open: over the studio, or in its own place.
        if st.s.screen_request.is_some() {
            if st.overlay_open() || make_way(game) {
                st.accept_screen();
            } else {
                st.s.screen_request = None;
            }
        }
        let want = (toggle && !was_open && game.native_playing()) | std::mem::take(&mut st.s.ui.open_request);
        if want && !st.overlay_open() && make_way(game) {
            st.s.ui.open = true;
        }
        if was_open && !st.overlay_open() {
            give_back(game);
        }
        OPEN.with(|o| o.set(st.overlay_open()));
    });
}

/// The HUD: the scripts' handlers and panels.
pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_window() || !game.native_on(ID) {
        return;
    }
    with(|st| {
        let mut host = Host::new(game);
        host.batch = Some(batch);
        host.corners = Some(corners);
        st.hud(&mut host);
    });
}

/// As the world's entity geometry is made (end_geometry, every frame in a
/// world): the scripts' `world` handlers draw into it, at this frame's camera.
pub(super) fn world(game: &Game<'_>, camera: glam::DVec3, out: &mut Vec<Vertex>) {
    if !game.native_window() || !game.native_on(ID) {
        return;
    }
    let Some((_, yaw, pitch, fov)) = game.locator_camera else {
        return;
    };
    with(|st| {
        let cam = Camera {
            pos: camera.to_array(),
            yaw,
            pitch,
            fov,
        };
        let mut host = Host {
            game: GameRef::Shared(game),
            event_loop: None,
            batch: None,
            corners: None,
            world: Some((out, cam)),
        };
        st.world(&mut host);
    });
}

/// Over everything else the game draws in a world: the studio's screen.
pub(super) fn overlay(game: &Game<'_>, batch: &mut HudBatch) {
    with(|st| {
        if !st.overlay_open() {
            return;
        }
        let mut host = Host {
            game: GameRef::Shared(game),
            event_loop: None,
            batch: Some(batch),
            corners: None,
            world: None,
        };
        if st.s.screen.is_some() {
            draw_screen(st, &mut host);
        } else {
            draw(st, &mut host);
        }
    });
}

fn is_function_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::F1
            | KeyCode::F2
            | KeyCode::F3
            | KeyCode::F4
            | KeyCode::F5
            | KeyCode::F6
            | KeyCode::F7
            | KeyCode::F8
            | KeyCode::F9
            | KeyCode::F10
            | KeyCode::F11
            | KeyCode::F12
    )
}

fn map_key(code: KeyCode) -> Key {
    match code {
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Slash | KeyCode::NumpadDivide => Key::Char('/'),
        other => {
            // KeyA..KeyZ and Digit0..Digit9, by their names.
            let name = format!("{other:?}");
            match (name.strip_prefix("Key"), name.strip_prefix("Digit")) {
                (Some(l), _) if l.len() == 1 => Key::Char(l.to_ascii_lowercase().chars().next().unwrap_or('?')),
                (_, Some(d)) if d.len() == 1 => Key::Char(d.chars().next().unwrap_or('?')),
                _ => Key::Other,
            }
        }
    }
}

/// Each window event, as the game starts on it (follow_keymap): while the
/// studio is open, keys, clicks and the wheel are queued for it and hidden
/// from the game (the frame hook hands them over); while it is closed, its
/// key asks it to open.
pub(super) fn window_input(event: &mut WindowEvent) {
    // The game has no use for dropped files: script mods to install.
    if let WindowEvent::DroppedFile(path) = event {
        DROPPED.with(|d| {
            let mut d = d.borrow_mut();
            if d.len() < 64 {
                d.push(path.clone());
            }
        });
        return;
    }
    if let WindowEvent::ModifiersChanged(m) = event {
        let m = m.state();
        // AltGr arrives as Control and Alt together: it types, it is no shortcut.
        MODS.with(|s| {
            s.set(Mods {
                ctrl: (m.control_key() && !m.alt_key()) || m.super_key(),
                shift: m.shift_key(),
                alt: m.alt_key(),
            })
        });
        return;
    }
    let is_open_key =
        |code: KeyCode| OPEN_KEY_NAME.with(|k| horizon_client::keybinds::key_name(code) == Some(k.borrow().as_str()));
    if !OPEN.with(Cell::get) {
        if let WindowEvent::KeyboardInput { event: k, .. } = event
            && k.state == ElementState::Pressed
            && !k.repeat
            && let PhysicalKey::Code(code) = k.physical_key
            && is_open_key(code)
        {
            TOGGLE.with(|t| t.set(true));
        }
        return;
    }
    let mods = MODS.with(Cell::get);
    let inputs: Vec<Input> = match &*event {
        WindowEvent::KeyboardInput { event: k, .. } => {
            let down = k.state == ElementState::Pressed;
            let key = match k.physical_key {
                PhysicalKey::Code(code) if is_open_key(code) => Key::Toggle,
                PhysicalKey::Code(code) => map_key(code),
                _ => Key::Other,
            };
            let mut v = vec![Input::Key {
                key,
                down,
                repeat: k.repeat,
                mods,
            }];
            let typed = k
                .text
                .as_ref()
                .map(|t| t.chars().filter(|c| !c.is_control()).collect::<String>());
            if down
                && !mods.ctrl
                && key != Key::Toggle
                && let Some(t) = typed
                && !t.is_empty()
            {
                v.push(Input::Text(t));
            }
            v
        }
        WindowEvent::MouseInput { state, button, .. } => vec![Input::Button {
            left: *button == MouseButton::Left,
            down: *state == ElementState::Pressed,
        }],
        WindowEvent::MouseWheel { delta, .. } => {
            let steps = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => (p.y / 30.0) as f32,
            };
            vec![Input::Wheel(steps)]
        }
        WindowEvent::CursorMoved { position, .. } => {
            let scale = SCALE.with(Cell::get);
            let scale = if scale > 0.0 { scale } else { 1.0 };
            vec![Input::Mouse(position.x as f32 / scale, position.y as f32 / scale)]
        }
        _ => return,
    };
    // The game keeps the pointer and its own function keys (fullscreen,
    // screenshots, the debug screen).
    let function_key = matches!(event, WindowEvent::KeyboardInput { event: k, .. }
        if matches!(k.physical_key, PhysicalKey::Code(code) if is_function_key(code) && !is_open_key(code)));
    if function_key {
        return;
    }
    let pointer = matches!(event, WindowEvent::CursorMoved { .. });
    INPUTS.with(|q| {
        let mut q = q.borrow_mut();
        if q.len() < 512 {
            q.extend(inputs);
        }
    });
    if !pointer {
        *event = WindowEvent::HoveredFileCancelled;
    }
}

/// A key or mouse button for the scripts' `key` handlers.
pub(super) fn key(key: &str, pressed: bool) {
    with(|st| {
        if !st.s.ui.open {
            st.key(key, pressed);
        }
    });
}

/// A command typed in chat: whether a script (or the studio) takes it.
pub(super) fn command(game: &mut Game<'_>, command: &str) -> bool {
    if !game.native_window() || !game.native_on(ID) {
        return false;
    }
    with(|st| st.command(command)).unwrap_or(false)
}

/// A chat line from the server, for the scripts' `chat` handlers.
pub(super) fn chat_line(game: &Game<'_>, text: &horizon_client::text::Text, source: &horizon_client::chat::ChatSource) {
    if !game.native_window() || *source == horizon_client::chat::ChatSource::Client {
        return;
    }
    with(|st| st.chat_line(&text.plain()));
}

/// The zoom the scripts ask for.
pub(super) fn zoom() -> Option<f32> {
    with(|st| st.zoom()).flatten()
}

pub(super) fn listed() -> super::native_hooks::NativeMod {
    super::native_hooks::NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: super::hstudio::DESCRIPTION.into(),
        keys: vec![["open".into(), "Open Horizon Studio".into(), OPEN_KEY.into()]],
    }
}

pub(super) fn settings(buttons: &mut Vec<String>) {
    let running = with(|st| st.s.mods.iter().filter(|m| m.running()).count()).unwrap_or(0);
    buttons.push(format!("Reload script mods ({running} running)"));
}

pub(super) fn settings_click(game: &mut Game<'_>, button: usize) {
    if button != 0 {
        return;
    }
    with(|st| {
        let mut host = Host::new(game);
        st.start(&mut host);
        st.reload_all(&mut host);
    });
}
