// Horizon Studio inside the game: the one module that knows Horizon's
// `Game`. It keeps the studio in a thread local (the window's thread),
// implements `GameApi` over the game, and has one function per hook of
// horizon-mixin.toml. Hooks that find the studio busy (a script's agent
// command can reach the game's own command path) do nothing.

use super::hstudio::{
    BODY_PARTS, Body, Camera, CameraView, Corner, EntityInfo, Font, GameApi, ID, NAME, OPEN_KEY, PartFrame,
    PlayerEntry, PlayerInfo, PlayerPose, Reply, Studio, TexRef, VERSION, ViaInfo, WorldTri,
};
use horizon_client::net::hstudio_via as via;
use super::hstudio_ui::{Input, Key, Mods, UiResult, draw, draw_screen, handle, handle_screen};
use super::event_loop::EventLoopCtl;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::extras::Corner as GameCorner;
use horizon_client::extras_hud::Corners;
use horizon_client::hud::{HudBatch, HudVertex};
use horizon_client::mesher::Vertex;
use super::hstudio_quads::{self as quads, Shade};
use horizon_client::ui_theme::hstudio_paint as paint;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
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
    /// Whether the game's chat is open with no screen of the studio's up:
    /// the pointer is free, and reaches the scripts' Roblox GUI.
    static CHAT_FREE: Cell<bool> = const { Cell::new(false) };
    static INPUTS: RefCell<Vec<Input>> = const { RefCell::new(Vec::new()) };
    /// Files dropped on the window, to install as script mods.
    static DROPPED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    /// When the AI setup prompt was copied from the settings page.
    static COPIED: Cell<Option<Instant>> = const { Cell::new(None) };
    /// Frames, to refresh the settings pages' labels now and then.
    static FRAMES: Cell<u32> = const { Cell::new(0) };
    /// The vanilla mappings' state this frame, for `keys.mapping` in `world`
    /// handlers (which run while the game is only read).
    static MAPPINGS_NOW: RefCell<Vec<(&'static str, (bool, String))>> = const { RefCell::new(Vec::new()) };
    /// What scripts asked of the game while it was only read (a screen's or
    /// the world's drawing), done at the next frame.
    static LATER: RefCell<Vec<Later>> = const { RefCell::new(Vec::new()) };
    /// What Roblox scripts' Lighting, Workspace and Camera ask of the view:
    /// read by the FOV and fog hooks and each frame.
    static FX: Cell<Fx> = const { Cell::new(Fx { light: 0.0, time: None, fog: None, fov: None }) };
    /// How the player's own model looks as Roblox scripts make the local
    /// character (hstudio::Body), for the model's hooks; the parts hidden
    /// are in the library's mobs::hstudio_body.
    static BODY: Cell<BodyNow> = const { Cell::new(BodyNow { gone: false, pose: 0, spin: 0.0, still: false, fixed: false }) };
    /// A script's Roblox tool is in hand while the player plays: the mouse's
    /// left and right buttons are its (hstudio_rbx_tools), not the game's;
    /// and the presses it took, whose releases are its too.
    static TOOL_CLICKS: Cell<bool> = const { Cell::new(false) };
    static TOOL_HELD: Cell<[bool; 2]> = const { Cell::new([false; 2]) };
    /// A Roblox ProximityPrompt is shown while the player plays: its key
    /// (`key.keyboard.e`) is the prompt's, not the game's (hstudio_rbx_parts).
    static PROMPT_KEY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// hstudio::Body as the model's hooks read it.
#[derive(Clone, Copy, PartialEq)]
struct BodyNow {
    gone: bool,
    /// 0 as the game poses it, 1 sitting, 2 lying.
    pose: u8,
    /// Degrees a second.
    spin: f32,
    still: bool,
    fixed: bool,
}

/// After the player's model takes its tick (tick_player_model): posed as
/// the local character is, sitting as a rider sits or lying as a sleeper
/// lies (its head behind it), spun about its feet, its limbs still, its
/// body not turning to the walk; and marked as the player's own, whose
/// parts the studio hides.
pub(super) fn body_tick(game: &mut Game<'_>) {
    let b = BODY.with(Cell::get);
    if !game.native_on(ID) {
        return;
    }
    let yaw = game.yaw;
    let Some(m) = game.player_model.as_mut() else { return };
    if horizon_client::mobs::hstudio_body::mask() != 0 {
        m.extra.hidden_model_parts |= horizon_client::mobs::hstudio_body::OWN;
    }
    match b.pose {
        1 => m.extra.passenger = true,
        2 => {
            // A bed's facing code (villager::sleep_pose): the head toward the back.
            let quarter = ((yaw.rem_euclid(360.0) + 45.0) / 90.0) as i32 % 4;
            m.extra.ints[1] = [1, 4, 2, 3][quarter as usize];
        }
        _ => {}
    }
    if b.still {
        m.limb_speed = 0.0;
        m.previous_limb_speed = 0.0;
    }
    if b.spin != 0.0 {
        m.body_yaw = m.previous_body_yaw + b.spin / 20.0;
        m.head_yaw = m.body_yaw;
    } else if b.fixed {
        // Within 50 degrees of where the player looks, as the game keeps it.
        let diff = (yaw - m.previous_body_yaw + 540.0).rem_euclid(360.0) - 180.0;
        m.body_yaw = m.previous_body_yaw + (diff.abs() - 50.0).max(0.0) * diff.signum();
    }
}

/// Whether the player's own model is drawn at all: not while Roblox
/// scripts took the whole local character away.
pub(super) fn body_whole(game: &Game<'_>) -> bool {
    !(BODY.with(Cell::get).gone && game.native_on(ID))
}

/// Whether the first-person arm of `hand` is drawn (empty, it shows the
/// arm): not once Roblox scripts took that arm (or the whole character) off.
pub(super) fn arm_shown(game: &Game<'_>, hand: horizon_client::first_person::Hand) -> bool {
    if !game.native_on(ID) {
        return true;
    }
    let left = game.settings.main_hand_left != (hand == horizon_client::first_person::Hand::Off);
    let part = if left { "left_arm" } else { "right_arm" };
    let bit = BODY_PARTS.iter().position(|p| *p == part).unwrap_or(0);
    !BODY.with(Cell::get).gone && horizon_client::mobs::hstudio_body::mask() & (1 << bit) == 0
}

/// The view as scripts ask for it (hstudio_rbx_world's Effects).
#[derive(Clone, Copy)]
struct Fx {
    /// Fullbright, 0 to 1: the brightness option at least this, fully lit at 1.
    light: f32,
    /// The client's time of day, in ticks of a day.
    time: Option<i64>,
    /// Where the fog ends, in blocks.
    fog: Option<f32>,
    /// The field of view, in degrees.
    fov: Option<f32>,
}

fn fx_set(f: impl FnOnce(&mut Fx)) {
    FX.with(|c| {
        let mut v = c.get();
        f(&mut v);
        c.set(v);
    });
}

/// The field of view: the game's, or the one scripts ask for, keeping the
/// game's own changes to it (sprinting, the spyglass) in proportion.
pub(super) fn field_of_view(value: f32, setting: f32) -> f32 {
    match FX.with(|c| c.get().fov) {
        Some(f) => value * f.clamp(1.0, 120.0) / setting.max(1.0),
        None => value,
    }
}

/// Where the fog ends when scripts bring it closer (Roblox's FogEnd, from 0;
/// at the camera while they have 3D rendering off).
pub(super) fn fog_end() -> Option<f32> {
    if horizon_core::natrium::world_hidden() {
        return Some(0.0);
    }
    FX.with(|c| c.get().fog)
}

/// The time of day scripts ask for, kept against the server's updates; and
/// the light: fully lit while they ask for full brightness, else the
/// brightness option at least their strength (each back to the options' as
/// they stop).
fn keep_view(game: &mut Game<'_>) {
    let fx = FX.with(Cell::get);
    if let Some(t) = fx.time {
        let day = game.world.day_time.div_euclid(24000);
        game.world.day_time = day * 24000 + t.rem_euclid(24000);
    }
    let (gamma, full) = (game.settings.gamma, game.settings.extras.fullbright);
    if let Some(r) = game.renderer.as_mut() {
        r.fullbright = full || fx.light >= 0.99;
        r.gamma = gamma.max(fx.light);
    }
}


thread_local! {
    /// What Roblox scripts hold of the view and the input (hstudio_rbx_world's
    /// game_effects): read by the camera, mouse and window hooks.
    static HOLD: RefCell<Hold> = const {
        RefCell::new(Hold {
            view: None,
            perspective: None,
            sunk: Vec::new(),
            look: false,
            mouse: [0.0; 2],
            sensitivity: 1.0,
            frozen: false,
        })
    };
    /// Whether the player plays (no screen up, the chat closed): the keys
    /// scripts take are theirs only then.
    static PLAYING: Cell<bool> = const { Cell::new(false) };
}

/// The view and input scripts hold.
struct Hold {
    /// Where the view is while a script's Camera has it.
    view: Option<CameraView>,
    /// The game's perspective while the view is away (it shows the body).
    perspective: Option<super::perspective::CameraType>,
    /// The keys the game doesn't get (`key.keyboard.w`), and whether the
    /// mouse turns no player.
    sunk: Vec<String>,
    look: bool,
    /// The mouse's movement since the studio last asked, in pixels.
    mouse: [f64; 2],
    /// The mouse's sensitivity, a multiple of the player's.
    sensitivity: f64,
    /// The character is anchored: the player holds still.
    frozen: bool,
}

/// The view (`Camera.update`, ahead of the game's): where a script's Camera
/// put it, else the game's own. Detached, it is where the script says; on
/// another entity, at its eyes looking as it looks in first person, else
/// behind it as the player looks (Roblox's camera turns about its subject
/// with the mouse); around a point, behind it as the player looks, as far
/// as the third person view goes, pulled in where blocks are in the way.
pub(super) fn camera_view(
    game: &mut Game<'_>,
    partial: f32,
) -> std::ops::ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    use super::perspective::CameraType;
    use std::ops::ControlFlow::{Break, Continue};
    let (view, mut perspective) = HOLD.with(|h| {
        let h = h.borrow();
        (h.view, h.perspective)
    });
    let Some(view) = view else { return Continue(()) };
    if !game.native_on(ID) || game.player.is_none() {
        return Continue(());
    }
    // F5 while the view is away (the body shown as in third person) turns
    // the perspective it has, and comes back to.
    if game.camera_type != CameraType::ThirdPersonBack {
        perspective = perspective.map(|p| match p {
            CameraType::FirstPerson => CameraType::ThirdPersonBack,
            CameraType::ThirdPersonBack => CameraType::ThirdPersonFront,
            CameraType::ThirdPersonFront => CameraType::FirstPerson,
        });
        HOLD.with(|h| h.borrow_mut().perspective = perspective);
        game.camera_type = CameraType::ThirdPersonBack;
    }
    let first = perspective.is_none_or(|p| p.first_person());
    let distance = game
        .player
        .as_mut()
        .and_then(|p| p.attribute("minecraft:camera_distance").ok())
        .unwrap_or(4.0);
    let (eye, yaw, pitch, back) = match view {
        CameraView::At { pos, yaw, pitch } => (pos, yaw, pitch, 0.0),
        CameraView::Entity(id) => {
            let Some(m) = game.mobs.iter().find(|m| m.id == id && !m.removed) else {
                return Continue(());
            };
            let e = super::remote_camera::mob_camera_entity(m);
            let p = f64::from(partial);
            let at: [f64; 3] = std::array::from_fn(|i| e.previous[i] + (e.position[i] - e.previous[i]) * p);
            let eye = [at[0], at[1] + f64::from(e.eye_height), at[2]];
            if first {
                (eye, e.view_y_rot(partial), e.view_x_rot(partial), 0.0)
            } else {
                (eye, game.yaw, game.pitch, distance)
            }
        }
        CameraView::Around(p) => (p, game.yaw, game.pitch, if first { 0.0 } else { distance }),
    };
    let mut v = game.view;
    v.set_rotation(yaw, pitch);
    v.position = eye;
    v.eye = eye;
    v.eye_rotation = [yaw, pitch];
    if back > 0.0 {
        let d = clear_behind(game, eye, v.forwards, back);
        v.move_by(-(d as f32), 0.0, 0.0);
    }
    v.detached = true;
    game.view = v;
    Break(Some((glam::DVec3::from_array(v.position), v.y_rot, v.x_rot)))
}

/// How far back from `eye` (against `forwards`) the view can go, up to
/// `distance`, before a solid block: a step at a time, stopping a little
/// short of it.
fn clear_behind(game: &Game<'_>, eye: [f64; 3], forwards: [f32; 3], distance: f64) -> f64 {
    let facts = game.res.facts();
    let step = 0.1;
    let mut d = 0.0;
    while d < distance {
        let next = (d + step).min(distance);
        let p: [f64; 3] = std::array::from_fn(|i| eye[i] - f64::from(forwards[i]) * next);
        let state = game
            .world
            .block(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
        if facts.get(state).solid_render {
            return (d - step).max(0.0);
        }
        d = next;
    }
    distance
}

/// The mouse's turn of the player (`MouseHandler.turnPlayer`, each frame):
/// its movement goes to the scripts (UserInputService's mouse delta), at
/// their sensitivity; while a script's action takes it, it turns no one.
pub(super) fn mouse_look(game: &mut Game<'_>) -> std::ops::ControlFlow<()> {
    use std::ops::ControlFlow::{Break, Continue};
    let grabbed = (game.captured || game.agent_pointer) && game.screen == Screen::Playing && game.player.is_some();
    if !grabbed || !game.native_on(ID) {
        return Continue(());
    }
    let (dx, dy) = game.mouse.accumulated();
    let (look, k) = HOLD.with(|h| {
        let mut h = h.borrow_mut();
        h.mouse[0] += dx;
        h.mouse[1] += dy;
        (h.look, h.sensitivity)
    });
    if look {
        game.mouse.add_motion(-dx, -dy);
        return Break(());
    }
    if (k - 1.0).abs() > 1e-9 {
        game.mouse.add_motion(dx * (k - 1.0), dy * (k - 1.0));
    }
    Continue(())
}

/// A key the scripts' actions take, pressed while the player plays: the
/// scripts get it (UserInputService, the action), the game doesn't (W
/// walks no one while a free camera flies with it, E opens no inventory).
/// Its release reaches both, so the game lets go of what it held.
fn sunk_key(event: &mut WindowEvent) -> bool {
    let WindowEvent::KeyboardInput { event: k, .. } = &*event else {
        return false;
    };
    let PhysicalKey::Code(code) = k.physical_key else {
        return false;
    };
    if k.state != ElementState::Pressed || !PLAYING.with(Cell::get) || CHAT_FREE.with(Cell::get) {
        return false;
    }
    let Some(name) = horizon_client::keybinds::key_name(code) else {
        return false;
    };
    if !HOLD.with(|h| h.borrow().sunk.iter().any(|s| s == name)) {
        return false;
    }
    if !k.repeat {
        key(name, true);
    }
    *event = WindowEvent::HoveredFileCancelled;
    true
}

/// Each frame: an anchored character's player stays still (what pushed it
/// stops), and whether the player plays, for the window hook.
fn keep_still(game: &mut Game<'_>) {
    PLAYING.with(|p| p.set(game.screen == Screen::Playing && game.player.is_some()));
    if HOLD.with(|h| h.borrow().frozen)
        && let Some(p) = game.player.as_mut()
    {
        p.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
        p.fall_distance = 0.0;
    }
}
/// A call that changes the game, kept for the next frame.
enum Later {
    Chat(String),
    Send(String),
    Clipboard(String),
    Sound(String, Option<[f64; 3]>, f32, f32),
    Particle(horizon_client::particles::ParticleOptions, [f64; 3], [f64; 3]),
    /// What a Roblox script asked of the client (hstudio_rbx_client).
    Client(super::hstudio::ClientAction),
}

/// The most kept for one frame (a screen drawn 60 times a second can't pile up).
const MAX_LATER: usize = 2048;

fn later(call: Later) {
    LATER.with(|l| {
        let mut l = l.borrow_mut();
        if l.len() < MAX_LATER {
            l.push(call);
        }
    });
}

/// Does what was kept for this frame.
fn catch_up(game: &mut Game<'_>) {
    let calls = LATER.with(|l| std::mem::take(&mut *l.borrow_mut()));
    if calls.is_empty() {
        return;
    }
    // The scripts' own sends: past the scripts' packet hooks (a hook that
    // changed a chat line isn't shown it again).
    QUIET.with(|q| q.set(true));
    for call in calls {
        match call {
            Later::Chat(text) => game.native_chat(&text),
            Later::Send(line) => game.send_chat(&line),
            Later::Clipboard(text) => game.clipboard.set(&text),
            Later::Sound(id, at, volume, pitch) => game.audio.play(&id, at, volume, pitch),
            Later::Particle(options, at, velocity) => game.particles.add(&options, at, velocity),
            Later::Client(action) => client_now(game, action),
        }
    }
    QUIET.with(|q| q.set(false));
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
    event_loop: Option<&'h dyn EventLoopCtl>,
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

/// `0xAARRGGBB` in the HUD's byte order (`0xAABBGGRR`).
fn hud_color(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    u32::from_be_bytes([a, b, g, r])
}

fn paint_font(f: Font) -> paint::Font {
    match f {
        Font::Regular => paint::Font::Regular,
        Font::Medium => paint::Font::Medium,
        Font::Mono => paint::Font::Mono,
    }
}

/// Hands this frame's new glyphs to the batch (after the studio drew).
fn flush(host: &mut Host<'_, '_>) {
    let game = match &host.game {
        GameRef::Mut(g) => &**g,
        GameRef::Shared(g) => *g,
    };
    if let Some(b) = host.batch.as_deref_mut() {
        paint::flush(&game.gui.atlas, b);
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

/// The player's own profile name and UUID: on a server, the one it joined
/// with; hosting a LAN world, the host's; alone, the game's single-player
/// name and the level's UUID.
fn own_profile(g: &Game<'_>) -> (String, u128) {
    if let Some(r) = g.remote.as_ref() {
        return (r.profile_name.clone(), r.profile());
    }
    if let Some(h) = g.lan.as_ref() {
        return (h.host_name.clone(), h.host_uuid);
    }
    let uuid = g
        .level
        .player_uuid
        .map_or(0, |u| u.iter().fold(0u128, |a, &w| (a << 32) | u128::from(w as u32)));
    (g.acting_player_name().to_owned(), uuid)
}

/// Where something is `alpha` of the way from its last tick to this one.
fn between(a: &horizon_core::geometry::Vec3, b: &horizon_core::geometry::Vec3, alpha: f64) -> [f64; 3] {
    [
        a.x + (b.x - a.x) * alpha,
        a.y + (b.y - a.y) * alpha,
        a.z + (b.z - a.z) * alpha,
    ]
}

/// An angle `alpha` of the way from `a` to `b`, the short way round.
fn degrees_between(a: f32, b: f32, alpha: f64) -> f32 {
    let d = (b - a + 540.0).rem_euclid(360.0) - 180.0;
    a + d * alpha as f32
}

/// A tick's move as a speed, in blocks a second. More than five blocks in a
/// tick is a teleport, not a speed.
/// Scales the player's `attribute` by `scale` (1: as it is), as the game's
/// own modifiers do (sprinting's): one of ours, by `id`, taken off first.
fn scale_attribute(
    attributes: &mut horizon_world::attributes::live::LiveAttributes<Box<dyn horizon_world::attributes::live::Identities>>,
    attribute: &str,
    id: &str,
    scale: f64,
) -> bool {
    use horizon_world::attributes::{Modifier, Operation};
    let Some(key) = attributes.key(attribute) else { return false };
    let Ok(Some(handle)) = attributes.get_instance(Some(key)) else { return false };
    let Ok(modifier) = Modifier::new(id, scale - 1.0, Operation::AddMultipliedTotal) else {
        return false;
    };
    let on = (scale - 1.0).abs() > 1e-9;
    attributes
        .edit(&handle, |value, observer| {
            value.remove(modifier.id(), observer)?;
            if on {
                value.add_transient(modifier, observer)?;
            }
            Ok(())
        })
        .is_ok()
}

fn per_second(from: &horizon_core::geometry::Vec3, to: &horizon_core::geometry::Vec3) -> [f64; 3] {
    let d = [to.x - from.x, to.y - from.y, to.z - from.z];
    if d.iter().map(|c| c * c).sum::<f64>() > 25.0 {
        return [0.0; 3];
    }
    d.map(|c| c * 20.0)
}

/// What the studio is told of a mob, or of another player's body (`name`
/// is the player's, for one).
fn mob_info(m: &horizon_client::mobs::Mob, name: Option<&str>, alpha: f64) -> EntityInfo {
    EntityInfo {
        id: m.id,
        kind: m.kind.id.clone(),
        name: name
            .map(str::to_owned)
            .or_else(|| m.custom_name.clone())
            .unwrap_or_default(),
        pos: between(&m.previous, &m.position, alpha),
        velocity: per_second(&m.previous, &m.position),
        body_yaw: degrees_between(m.previous_body_yaw, m.body_yaw, alpha),
        head_yaw: degrees_between(m.previous_head_yaw, m.head_yaw, alpha),
        pitch: degrees_between(m.previous_head_pitch, m.head_pitch, alpha),
        width: f64::from(m.width()),
        height: f64::from(m.height()),
        eye: m.eye_height(),
        health: m.health.max(0.0),
        max_health: horizon_client::mobs::finalize::max_health(m),
        speed: m.movement_speed(),
        on_ground: m.on_ground,
        baby: m.is_baby(),
        item: None,
    }
}

/// What the studio is told of a dropped item.
fn item_info(i: &horizon_client::entities::ItemEntity, alpha: f64) -> EntityInfo {
    let size = horizon_client::entities::ItemEntity::WIDTH;
    EntityInfo {
        id: i.id,
        kind: "minecraft:item".into(),
        pos: between(&i.previous, &i.position, alpha),
        velocity: per_second(&i.previous, &i.position),
        width: size,
        height: size,
        eye: size * 0.85,
        on_ground: i.on_ground,
        item: Some((i.stack.item.to_string(), i.stack.count)),
        ..EntityInfo::default()
    }
}

/// What the studio is told of an experience orb.
fn orb_info(o: &horizon_client::entities::ExperienceOrb, alpha: f64) -> EntityInfo {
    EntityInfo {
        id: o.id,
        kind: "minecraft:experience_orb".into(),
        pos: between(&o.previous, &o.position, alpha),
        velocity: per_second(&o.previous, &o.position),
        width: 0.5,
        height: 0.5,
        eye: 0.425,
        on_ground: o.on_ground,
        item: Some((String::new(), o.value)),
        ..EntityInfo::default()
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

/// Whether what was drawn since `mark` is whole quads the studio may change
/// (a clip, a turn or a gradient): none of it in a run of another blend, or
/// in the blurred background of a screen.
fn ours_since(b: &HudBatch, mark: usize) -> bool {
    mark <= b.vertices.len()
        && (b.vertices.len() - mark).is_multiple_of(6)
        && b.blends.iter().all(|(r, _)| r.end as usize <= mark)
        && b.blur_at as usize <= mark
}

/// Whether a quad (its six vertices, as `HudBatch::quad` pushes them) is a
/// rectangle on the screen's axes in one colour: one the HUD's own
/// `clip_since` cuts exactly, texture and all.
fn plain(q: &[HudVertex]) -> bool {
    let corners = [q[0], q[1], q[2], q[5]];
    let (mut x0, mut x1, mut y0, mut y1) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for v in &corners {
        x0 = x0.min(v.pos[0]);
        x1 = x1.max(v.pos[0]);
        y0 = y0.min(v.pos[1]);
        y1 = y1.max(v.pos[1]);
    }
    let found = |x: f32, y: f32| corners.iter().any(|v| v.pos == [x, y]);
    q.iter().all(|v| v.color == q[0].color) && found(x0, y0) && found(x1, y0) && found(x1, y1) && found(x0, y1)
}

/// The quads drawn since `mark`, as hstudio_quads has them: each one's
/// corners are its vertices 0, 1, 2 and 5.
fn quads_since(b: &HudBatch, mark: usize) -> Vec<quads::Quad> {
    b.vertices[mark..]
        .chunks_exact(6)
        .map(|q| {
            [q[0], q[1], q[2], q[5]].map(|v| quads::Corner {
                at: v.pos,
                uv: v.uv,
                argb: hud_color(v.color),
            })
        })
        .collect()
}

/// Quads drawn as `HudBatch::quad` draws them, each corner in its own colour.
fn push_quads(b: &mut HudBatch, list: &[quads::Quad]) {
    for q in list {
        for i in [0, 1, 2, 0, 2, 3] {
            b.vertices.push(HudVertex {
                pos: q[i].at,
                uv: q[i].uv,
                color: hud_color(q[i].argb),
                depth: 0.0,
                glint_uv: [0.0, 0.0],
                text: 0,
            });
        }
    }
}

impl GameApi for Host<'_, '_> {
    fn chat(&mut self, text: &str) {
        super::hstudio_link::record("out", text);
        match self.g_mut() {
            Some(g) => g.native_chat(text),
            None => later(Later::Chat(text.to_owned())),
        }
    }
    fn send(&mut self, line: &str) {
        super::hstudio_link::record("sent", line);
        match self.g_mut() {
            Some(g) => g.send_chat(line),
            None => later(Later::Send(line.to_owned())),
        }
    }
    fn packet_names(&self) -> (&'static [&'static str], &'static [&'static str]) {
        (
            horizon_protocol::ids::play::serverbound::NAMES,
            horizon_protocol::ids::play::clientbound::NAMES,
        )
    }
    fn send_packet(&mut self, packet: &Packet) -> Result<(), String> {
        let s = build(packet)?;
        send_now(self.g(), s)
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
        let (name, uuid) = own_profile(g);
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
            name,
            uuid,
            velocity: per_second(&p.old_position, &p.position),
            speed: None,
            jump: None,
        };
        if let Some(g) = self.g_mut()
            && let Some(p) = g.player.as_mut()
            && let Ok(max) = p.attribute("minecraft:max_health")
        {
            info.max_health = max as f32;
        }
        // Attributes are worked out on a mutable player: only while the game can change.
        if let Some(g) = self.g_mut()
            && let Some(p) = g.player.as_mut()
        {
            info.speed = p.attribute("minecraft:movement_speed").ok();
            info.jump = p.attribute("minecraft:jump_strength").ok();
        }
        Some(info)
    }
    fn entities(&mut self, max: usize) -> Vec<EntityInfo> {
        let g = self.g();
        let Some(me) = g.player.as_ref() else {
            return Vec::new();
        };
        let me = [me.position.x, me.position.y, me.position.z];
        let alpha = g.clock.fraction();
        // The other players' entities by name: a server's, and a LAN host's guests.
        let mut names: std::collections::HashMap<u32, String> = g.remote_player_names().into_iter().collect();
        if let Some(h) = g.lan.as_ref() {
            for (conn, m) in &h.models {
                if let Some(guest) = h.hub.guest(*conn) {
                    names.entry(m.id).or_insert_with(|| guest.name.clone());
                }
            }
        }
        let guests: Vec<&horizon_client::mobs::Mob> =
            g.lan.as_ref().map_or_else(Vec::new, |h| h.models.values().collect());
        // Each entity first as what orders them: whether it is another
        // player, how far it is (squared), and where it is (0 the mobs, 1 the
        // guests, 2 the dropped items, 3 the orbs, and its index there). Only
        // the `max` kept are read in full, so a farm's thousands cost little.
        let far = |p: [f64; 3]| (p[0] - me[0]).powi(2) + (p[1] - me[1]).powi(2) + (p[2] - me[2]).powi(2);
        let mut keys: Vec<(bool, f64, u8, usize)> = Vec::new();
        // Dying mobs stay until they are removed, so the studio sees them die.
        for (i, m) in g.mobs.iter().enumerate().filter(|(_, m)| !m.removed) {
            let at = between(&m.previous, &m.position, alpha);
            keys.push((names.contains_key(&m.id), far(at), 0, i));
        }
        for (i, m) in guests.iter().enumerate().filter(|(_, m)| !m.removed) {
            let at = between(&m.previous, &m.position, alpha);
            keys.push((names.contains_key(&m.id), far(at), 1, i));
        }
        for (i, d) in g.dropped.iter().enumerate() {
            keys.push((false, far(between(&d.previous, &d.position, alpha)), 2, i));
        }
        for (i, o) in g.orbs.iter().enumerate() {
            keys.push((false, far(between(&o.previous, &o.position, alpha)), 3, i));
        }
        // The players first, then the nearest (and as the lists have them).
        let order = |a: &(bool, f64, u8, usize), b: &(bool, f64, u8, usize)| {
            b.0.cmp(&a.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
                .then(a.3.cmp(&b.3))
        };
        if max == 0 {
            return Vec::new();
        }
        if keys.len() > max {
            keys.select_nth_unstable_by(max - 1, order);
            keys.truncate(max);
        }
        keys.sort_unstable_by(order);
        let name = |id: u32| names.get(&id).map(String::as_str);
        keys.into_iter()
            .map(|(_, _, list, i)| match list {
                0 => mob_info(&g.mobs[i], name(g.mobs[i].id), alpha),
                1 => mob_info(guests[i], name(guests[i].id), alpha),
                2 => item_info(&g.dropped[i], alpha),
                _ => orb_info(&g.orbs[i], alpha),
            })
            .collect()
    }
    fn players(&mut self) -> Option<Vec<PlayerEntry>> {
        // A server's list fetches the players' skins as it is read: only while the game can change.
        let g = self.g_mut()?;
        let mut out = Vec::new();
        if g.remote.is_some() {
            let (listed, _, _) = g.remote_tab_players()?;
            let r = g.remote.as_ref()?;
            let me = r.profile();
            for p in listed {
                let shown = p
                    .display_name
                    .as_ref()
                    .map(|c| horizon_client::scoreboard::comp_text(c).plain())
                    .filter(|s| !s.trim().is_empty());
                out.push(PlayerEntry {
                    uuid: p.uuid,
                    display_name: shown.unwrap_or_else(|| p.name.clone()),
                    entity: r.player_entity(p.uuid),
                    local: p.uuid == me,
                    latency: p.latency,
                    name: p.name,
                });
            }
        } else if let Some(h) = g.lan.as_ref() {
            out.push(PlayerEntry {
                uuid: h.host_uuid,
                name: h.host_name.clone(),
                display_name: h.host_name.clone(),
                entity: None,
                local: true,
                latency: 0,
            });
            for guest in h.hub.guests.iter().filter(|guest| guest.placed) {
                out.push(PlayerEntry {
                    uuid: guest.uuid,
                    name: guest.name.clone(),
                    display_name: guest.name.clone(),
                    entity: h.models.get(&guest.conn).map(|m| m.id),
                    local: false,
                    latency: guest.latency.load(std::sync::atomic::Ordering::Relaxed),
                });
            }
        }
        // The list keeps no order of its own: this player first, then by name.
        out.sort_by_key(|p| (!p.local, p.name.to_lowercase()));
        Some(out)
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        let g = self.g();
        g.player.as_ref()?;
        let state = g.world.block(x, y, z);
        Some(g.res.facts().get(state).name.to_string())
    }
    fn saves_dir(&self) -> Option<PathBuf> {
        // Where the title screen's world list reads (`Title::new`).
        std::env::current_dir().ok().map(|d| d.join("saves"))
    }
    fn dump_world(&mut self, world: &std::path::Path) -> Result<u64, String> {
        super::hstudio_world_dump::start(self.g(), world)
    }
    fn dump_poll(&mut self, job: u64) -> Option<Result<(), String>> {
        super::hstudio_world_dump::poll(job)
    }
    fn region_blocks(&mut self, min: [i32; 3], max: [i32; 3]) -> Option<super::hstudio_dump::Region> {
        super::hstudio_world_dump::region(self.g(), min, max)
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
        match self.g_mut() {
            Some(g) => g.native_mapping(name),
            // As the world draws the game is only read: this frame's answers.
            None => MAPPINGS_NOW.with(|m| m.borrow().iter().find(|(n, _)| *n == name).map(|(_, v)| v.clone())),
        }
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
    fn screenshot(&mut self, path: &std::path::Path) -> Result<(), String> {
        let g = self.g_mut().ok_or("no game to look at")?;
        let renderer = g.renderer.as_mut().ok_or("the game isn't drawing yet")?;
        // F2's, or one already asked for: the next frame is theirs.
        if renderer.screenshot.is_some() {
            return Err("a screenshot is being taken: ask again in a moment".into());
        }
        renderer.screenshot = Some(horizon_client::gfx::ScreenshotRequest {
            path: path.to_path_buf(),
            size: None,
            // As the window shows it, as before F2 took the game's resolution.
            scaled: false,
        });
        Ok(())
    }
    fn data_dir(&self) -> PathBuf {
        self.g().native_data_dir(ID)
    }
    fn clipboard(&mut self) -> String {
        self.g_mut().map(|g| g.clipboard.get()).unwrap_or_default()
    }
    fn set_clipboard(&mut self, text: &str) {
        match self.g_mut() {
            Some(g) => g.clipboard.set(text),
            None => later(Later::Clipboard(text.to_owned())),
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
    fn dp(&mut self) -> (f32, f32, f32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        match self.batch.as_deref_mut() {
            Some(b) => paint::size(&game.gui.atlas, b),
            None => {
                let (w, h, scale) = gui(game);
                let density = (w * scale / 1040.0).min(h * scale / 700.0).clamp(1.0, 4.0);
                (w * scale / density, h * scale / density, scale / density)
            }
        }
    }
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        if let Some(b) = self.batch.as_deref_mut() {
            paint::rect(&game.gui.atlas, b, r, radius, hud_color(argb));
        }
    }
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        if let Some(b) = self.batch.as_deref_mut() {
            paint::border(&game.gui.atlas, b, r, radius, hud_color(argb));
        }
    }
    fn dp_text(&mut self, text: &str, at: [f32; 2], size: f32, font: Font, argb: u32) -> f32 {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        match self.batch.as_deref_mut() {
            Some(b) => paint::text(
                &game.gui.atlas,
                b,
                paint_font(font),
                size,
                at[0],
                at[1],
                hud_color(argb),
                text,
            ),
            None => 0.0,
        }
    }
    fn dp_measure(&mut self, text: &str, size: f32, font: Font) -> f32 {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        match self.batch.as_deref_mut() {
            Some(b) => paint::measure(&game.gui.atlas, b, paint_font(font), size, text),
            // No frame being drawn (the pointer's handling): measured as the
            // frame measures, at the screen's density (as `dp` has it).
            None => {
                let (w, h, scale) = gui(game);
                let density = (w * scale / 1040.0).min(h * scale / 700.0).clamp(1.0, 4.0);
                paint::measure_at(&game.gui.atlas, density, paint_font(font), size, text)
            }
        }
    }
    fn dp_mark(&mut self) -> Option<usize> {
        self.batch.as_deref().map(|b| b.vertices.len())
    }
    fn dp_clip(&mut self, mark: usize, r: [f32; 4]) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some(b) = self.batch.as_deref_mut() else {
            return;
        };
        if !ours_since(b, mark) {
            return;
        }
        // The rectangle in GUI pixels, its edges on the screen's pixels.
        let (_, _, k) = paint::size(&game.gui.atlas, b);
        let scale = b.scale;
        let density = scale / k;
        let px = |v: f32| (v * density).round() / scale;
        let (x0, y0, x1, y1) = (px(r[0]), px(r[1]), px(r[0] + r[2]), px(r[1] + r[3]));
        if b.vertices[mark..].chunks_exact(6).all(plain) {
            b.clip_since(mark, x0, y0, x1, y1);
        } else {
            // Turned or shaded quads: cut with their texture and colours.
            let cut = quads::clip(&quads_since(b, mark), [x0, y0, x1, y1]);
            b.vertices.truncate(mark);
            push_quads(b, &cut);
        }
    }
    fn dp_rotate(&mut self, mark: usize, centre: [f32; 2], degrees: f32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some(b) = self.batch.as_deref_mut() else {
            return;
        };
        if !ours_since(b, mark) {
            return;
        }
        let (_, _, k) = paint::size(&game.gui.atlas, b);
        let centre = [centre[0] / k, centre[1] / k];
        let sc = degrees.to_radians().sin_cos();
        for v in &mut b.vertices[mark..] {
            v.pos = quads::turned(v.pos, centre, sc);
        }
    }
    fn dp_shade(&mut self, mark: usize, shade: &Shade) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some(b) = self.batch.as_deref_mut() else {
            return;
        };
        if !ours_since(b, mark) {
            return;
        }
        // The batch's points are GUI pixels, `k` dp each.
        let (_, _, k) = paint::size(&game.gui.atlas, b);
        let shaded = quads::shade(&quads_since(b, mark), &shade.in_units(k));
        b.vertices.truncate(mark);
        push_quads(b, &shaded);
    }
    fn dp_slant(&mut self, mark: usize, y: f32, slope: f32) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some(b) = self.batch.as_deref_mut() else {
            return;
        };
        if !ours_since(b, mark) {
            return;
        }
        let (_, _, k) = paint::size(&game.gui.atlas, b);
        let y = y / k;
        for v in &mut b.vertices[mark..] {
            v.pos[0] += slope * (y - v.pos[1]);
        }
    }
    fn dp_image(&mut self, image: TexRef<'_>, r: [f32; 4], src: [f32; 4], argb: u32) -> bool {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some(b) = self.batch.as_deref_mut() else {
            return false;
        };
        // In the atlas's run-time region, as the world's meshes' textures are.
        // The HUD's atlas uploads go before it draws, so it shows this frame.
        let Some((uv, _)) = paint::image(&game.gui.atlas, image.key, image.version, image.w, image.h, image.rgba)
        else {
            return false;
        };
        let (_, _, k) = paint::size(&game.gui.atlas, b);
        let (iw, ih) = (image.w as f32, image.h as f32);
        // A hundredth of a texel in from the image's edges, so no neighbour shows.
        let u = |x: f32| uv[0] + (uv[2] - uv[0]) * x.clamp(0.01, iw - 0.01) / iw;
        let v = |y: f32| uv[1] + (uv[3] - uv[1]) * y.clamp(0.01, ih - 0.01) / ih;
        let (u0, v0, u1, v1) = (u(src[0]), v(src[1]), u(src[0] + src[2]), v(src[1] + src[3]));
        let [x, y, w, h] = r.map(|c| c / k);
        b.quad(
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
            hud_color(argb),
        );
        true
    }
    fn via(&self) -> ViaInfo {
        via_info()
    }
    fn via_set(&mut self, enabled: Option<bool>, version: Option<&str>) -> Result<(), String> {
        if let Some(v) = version {
            via::set_version(v)?;
        }
        if let Some(on) = enabled {
            via::set_enabled(on);
        }
        Ok(())
    }
    fn voice(&self) -> super::hstudio::VoiceInfo {
        super::hstudio_voicechat::global(|v| v.info(Instant::now()))
    }
    fn voice_set(&mut self, change: super::hstudio::VoiceChange) -> Result<(), String> {
        super::hstudio_voicechat::global(|v| v.set(change))
    }
    fn client(&self) -> super::hstudio::ClientInfo {
        client_info(self.g())
    }
    fn client_do(&mut self, action: super::hstudio::ClientAction) -> Result<(), String> {
        client_check(self.g(), &action)?;
        // Leaving and joining change the whole game: at the next frame's
        // start, as the rest kept for later.
        later(Later::Client(action));
        Ok(())
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
    fn world_mesh(&mut self, tris: &[WorldTri], texture: Option<TexRef<'_>>, light_at: Option<[f64; 3]>) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let rect = match texture {
            None => {
                let w = game.gui.atlas.font.white();
                let (u, v) = ((w.u0 + w.u1) / 2.0, (w.v0 + w.v1) / 2.0);
                [u, v, u, v]
            }
            Some(t) => match paint::image(&game.gui.atlas, t.key, t.version, t.w, t.h, t.rgba) {
                Some((r, false)) => {
                    // A hundredth of a texel in from its edges, so no neighbour shows.
                    let (du, dv) = ((r[2] - r[0]) / t.w as f32 * 0.01, (r[3] - r[1]) / t.h as f32 * 0.01);
                    [r[0] + du, r[1] + dv, r[2] - du, r[3] - dv]
                }
                // Put in the atlas just now: drawn from the next frame, once its pixels are there.
                _ => return,
            },
        };
        let light = light_at.map_or(240 | (240 << 8), |p| {
            horizon_client::block_entity_render::vertex_light(game.world.light(
                p[0].floor() as i32,
                p[1].floor() as i32,
                p[2].floor() as i32,
            ))
        });
        let Some((out, cam)) = self.world.as_mut() else {
            return;
        };
        for t in tris {
            let len = (t.normal[0] * t.normal[0] + t.normal[1] * t.normal[1] + t.normal[2] * t.normal[2])
                .sqrt()
                .max(1e-9);
            let n = |i: usize| (t.normal[i] / len * 127.0) as i8;
            let [a, r, g, b] = t.argb.to_be_bytes();
            let color = u32::from_le_bytes([r, g, b, a]);
            // A triangle as a quad whose last corner is its third.
            for k in [0, 1, 2, 2] {
                let p = t.p[k];
                let [u, v] = t.uv[k];
                out.push(Vertex {
                    pos: [
                        (p[0] - cam.pos[0]) as f32,
                        (p[1] - cam.pos[1]) as f32,
                        (p[2] - cam.pos[2]) as f32,
                    ],
                    uv: [
                        rect[0] + (rect[2] - rect[0]) * u.clamp(0.0, 1.0),
                        rect[1] + (rect[3] - rect[1]) * v.clamp(0.0, 1.0),
                    ],
                    layer: 0,
                    color,
                    light,
                    normal: [n(0), n(1), n(2), -127],
                    glint_uv: [0.0, 0.0],
                });
            }
        }
    }
    fn part_frame(&self, target: Option<u32>, part: &str) -> Option<PartFrame> {
        let g = self.g();
        g.player.as_ref()?;
        let (mob, alpha) = match target {
            None => (g.player_model.as_ref()?, g.clock.camera_fraction() as f32),
            Some(id) => (g.mobs.iter().find(|m| m.id == id)?, g.clock.fraction() as f32),
        };
        // Relative to the mob, then back: f32 keeps a millimetre near it,
        // not far from the world's origin.
        let at = glam::DVec3::new(mob.position.x, mob.position.y, mob.position.z);
        let m = if part == "root" {
            horizon_client::mobs::render::base_transform(mob, alpha, at)
        } else {
            horizon_client::mobs::render::part_transform(mob, alpha, at, g.res.mob_data(), part)?
        };
        // Model space has y down and z back: turned, its axes are left, up, forward.
        let m = m * glam::Mat4::from_rotation_x(std::f32::consts::PI);
        let v = |c: glam::Vec4| [f64::from(c.x), f64::from(c.y), f64::from(c.z)];
        let o = v(m.w_axis);
        Some(PartFrame {
            origin: [at.x + o[0], at.y + o[1], at.z + o[2]],
            left: v(m.x_axis),
            up: v(m.y_axis),
            forward: v(m.z_axis),
        })
    }
    fn body_shown(&self) -> bool {
        self.g().own_body_in_view()
    }
    fn pose(&self) -> Option<PlayerPose> {
        use horizon_client::pose::Pose;
        let g = self.g();
        let p = g.player.as_ref()?;
        let pose = match p.pose {
            Pose::Crouching => "crouching",
            Pose::Swimming => "swimming",
            Pose::FallFlying => "gliding",
            Pose::SpinAttack => "spin_attack",
            Pose::Sleeping => "sleeping",
            _ => "standing",
        };
        let camera = match g.camera_type {
            super::perspective::CameraType::FirstPerson => "first",
            super::perspective::CameraType::ThirdPersonBack => "third_back",
            super::perspective::CameraType::ThirdPersonFront => "third_front",
        };
        Some(PlayerPose {
            pose: pose.into(),
            sprinting: p.sprinting,
            on_ground: p.ground.on_ground,
            flying: p.flying,
            camera: camera.into(),
        })
    }
    fn world_item(&mut self, id: &str, m: [[f64; 3]; 4], display: &str, light_at: Option<[f64; 3]>) -> Result<(), String> {
        use horizon_assets::item_model::DisplayContext;
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let Some((out, cam)) = self.world.as_mut() else {
            return Err("items are drawn only with the world".into());
        };
        let context = match display {
            "head" => DisplayContext::Head,
            "hand" => DisplayContext::ThirdPersonRightHand,
            "left_hand" => DisplayContext::ThirdPersonLeftHand,
            "ground" => DisplayContext::Ground,
            "gui" => DisplayContext::Gui,
            _ => DisplayContext::Fixed,
        };
        let stack = horizon_client::items::ItemStack::new(id, 1);
        let state = game.resolve_item(&stack, context, None, 0);
        if state.is_empty() {
            return Err(format!("no item '{id}'"));
        }
        let axis = |c: [f64; 3]| glam::Vec4::new(c[0] as f32, c[1] as f32, c[2] as f32, 0.0);
        let o = m[3];
        let origin = glam::Vec4::new(
            (o[0] - cam.pos[0]) as f32,
            (o[1] - cam.pos[1]) as f32,
            (o[2] - cam.pos[2]) as f32,
            1.0,
        );
        let pose = glam::Mat4::from_cols(axis(m[0]), axis(m[1]), axis(m[2]), origin);
        let light = light_at.map_or(240 | (240 << 8), |p| {
            horizon_client::block_entity_render::vertex_light(game.world.light(
                p[0].floor() as i32,
                p[1].floor() as i32,
                p[2].floor() as i32,
            ))
        });
        game.submit_item_world(&state, pose, light, out);
        Ok(())
    }
    fn world_quad_lit(&mut self, corners: [[f64; 3]; 4], argb: u32, normal: [f64; 3], light_at: Option<[f64; 3]>) {
        let game = match &self.game {
            GameRef::Mut(g) => &**g,
            GameRef::Shared(g) => *g,
        };
        let white = game.gui.atlas.font.white();
        let light = light_at.map_or(240 | (240 << 8), |p| {
            horizon_client::block_entity_render::vertex_light(game.world.light(
                p[0].floor() as i32,
                p[1].floor() as i32,
                p[2].floor() as i32,
            ))
        });
        let Some((out, cam)) = self.world.as_mut() else {
            return;
        };
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt().max(1e-9);
        let n = |i: usize| (normal[i] / len * 127.0) as i8;
        let uv = [
            [white.u0, white.v1],
            [white.u1, white.v1],
            [white.u1, white.v0],
            [white.u0, white.v0],
        ];
        let [a, r, g, b] = argb.to_be_bytes();
        let color = u32::from_le_bytes([r, g, b, a]);
        for (k, p) in corners.iter().enumerate() {
            out.push(Vertex {
                pos: [
                    (p[0] - cam.pos[0]) as f32,
                    (p[1] - cam.pos[1]) as f32,
                    (p[2] - cam.pos[2]) as f32,
                ],
                uv: uv[k],
                layer: 0,
                color,
                light,
                // Shaded by the normal, from either side.
                normal: [n(0), n(1), n(2), -127],
                glint_uv: [0.0, 0.0],
            });
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
        match self.g_mut() {
            Some(g) => g.audio.play(id, at, volume, pitch),
            None => later(Later::Sound(id.to_owned(), at, volume, pitch)),
        }
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
        match self.g_mut() {
            Some(g) => g.particles.add(&options, at, velocity),
            None => later(Later::Particle(options, at, velocity)),
        }
        Ok(())
    }
    fn inventory(&mut self) -> Option<Vec<Option<(String, u32)>>> {
        let g = self.g();
        g.player.as_ref()?;
        Some(
            g.inventory
                .iter()
                .take(36)
                .map(|s| s.as_ref().map(|s| (s.item.to_string(), s.count)))
                .collect(),
        )
    }
    fn select_slot(&mut self, slot: usize) -> bool {
        let Some(g) = self.g_mut() else { return false };
        if slot >= 9 || g.player.is_none() {
            return false;
        }
        // As the number keys do: the next tick tells a server (the carried slot).
        g.selected = slot;
        true
    }
    fn click_item(&mut self, attack: bool) -> bool {
        let Some(g) = self.g_mut() else { return false };
        if g.screen != Screen::Playing || g.player.is_none() {
            return false;
        }
        // A press and a release, as the agent's use-item clicks: the next
        // tick acts on it (Minecraft.startUseItem, startAttack).
        let button = if attack { MouseButton::Left } else { MouseButton::Right };
        g.play_mouse(button, true).is_ok() && g.play_mouse(button, false).is_ok()
    }
    fn drop_held(&mut self, all: bool) -> bool {
        let Some(g) = self.g_mut() else { return false };
        if g.screen != Screen::Playing || g.player.is_none() {
            return false;
        }
        // As Q and Ctrl+Q: on a server (and the single-player server thread)
        // the server throws the stack, the client predicts it.
        if g.remote.is_some() {
            if !g.spectator() {
                g.remote_drop(all);
            }
        } else {
            g.drop_held(all);
        }
        true
    }
    fn set_client_block(&mut self, x: i32, y: i32, z: i32, block: &str) -> bool {
        let Some(g) = self.g_mut() else { return false };
        // Only a server's copy of its world: a world of the client's own
        // would keep the change.
        if g.remote.is_none() || g.player.is_none() || !g.world.loaded(x, z) {
            return false;
        }
        let Some(state) = g.pack.assets.registry.default_state(block) else {
            return false;
        };
        g.world.set(x, y, z, state).is_ok()
    }
    fn hide_entity(&mut self, id: u32) -> bool {
        let Some(g) = self.g_mut() else { return false };
        if g.remote.is_none() {
            return false;
        }
        // As the server's removing it does to the client's lists of them.
        let before = g.mobs.len() + g.dropped.len() + g.orbs.len();
        g.mobs.retain(|m| m.id != id);
        g.dropped.retain(|d| d.id != id);
        g.orbs.retain(|o| o.id != id);
        g.mobs.len() + g.dropped.len() + g.orbs.len() != before
    }
    fn move_player(&mut self, feet: [f64; 3]) -> bool {
        let Some(g) = self.g_mut() else { return false };
        let at = horizon_core::geometry::Vec3::new(feet[0], feet[1], feet[2]);
        let Some(p) = g.player.as_mut() else { return false };
        // As the game's own flights end: there, still, its fall forgotten.
        p.position = at;
        p.old_position = at;
        p.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
        p.fall_distance = 0.0;
        g.previous_position = at;
        true
    }
    fn set_player_velocity(&mut self, v: [f64; 3]) -> bool {
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else {
            return false;
        };
        p.velocity = horizon_core::geometry::Vec3::new(v[0], v[1], v[2]);
        p.fall_distance = 0.0;
        true
    }
    fn jump_player(&mut self) -> bool {
        let Some(g) = self.g_mut() else { return false };
        let Some(player) = g.player.as_mut() else { return false };
        // The game's own jump (LivingEntity.jumpFromGround), off the ground or not.
        horizon_client::motion::MotionWorld {
            entities: &[],
            player,
            scene: &mut g.world,
            assets: &g.pack.assets,
            collision_binding: g.collision,
            resources: g.motion,
        }
        .jump_from_ground()
        .is_ok()
    }
    fn set_player_rise(&mut self, vy: f64) -> bool {
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else {
            return false;
        };
        p.velocity.y = vy;
        p.fall_distance = 0.0;
        true
    }
    fn set_body(&mut self, body: &Body) {
        let mask = BODY_PARTS
            .iter()
            .enumerate()
            .filter(|(_, p)| body.hidden.contains(p))
            .fold(0u16, |m, (i, _)| m | (1 << i));
        horizon_client::mobs::hstudio_body::set(mask);
        let pose = match body.pose {
            "sitting" => 1,
            "lying" => 2,
            _ => 0,
        };
        BODY.with(|b| {
            b.set(BodyNow {
                gone: body.gone,
                pose,
                spin: body.spin,
                still: body.still,
                fixed: body.fixed,
            })
        });
    }
    fn set_noclip(&mut self, on: bool) -> bool {
        let Some(g) = self.g_mut() else { return false };
        // The spectator's physics: through blocks. Back as the game mode has it.
        horizon_client::mobs::hstudio_body::set_noclip(on);
        let spectator = on || g.mode == horizon_client::command::args::GameMode::Spectator;
        let Some(p) = g.player.as_mut() else { return false };
        p.spectator = spectator;
        true
    }
    fn set_player_speed(&mut self, walk: f64, jump: f64) -> bool {
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else {
            return false;
        };
        let a = scale_attribute(&mut p.attributes, "minecraft:movement_speed", "horizon_studio:walk_speed", walk);
        let b = scale_attribute(&mut p.attributes, "minecraft:jump_strength", "horizon_studio:jump", jump);
        a && b
    }
    fn set_fullbright(&mut self, strength: f32) {
        fx_set(|f| f.light = strength.clamp(0.0, 1.0));
    }
    fn set_time_of_day(&mut self, ticks: Option<i64>) {
        fx_set(|f| f.time = ticks);
        if let Some(g) = self.g_mut() {
            keep_view(g);
        }
    }
    fn set_fog_end(&mut self, blocks: Option<f32>) {
        fx_set(|f| f.fog = blocks.map(|b| b.max(0.0)));
    }
    fn set_fov(&mut self, degrees: Option<f32>) {
        fx_set(|f| f.fov = degrees);
    }
    fn set_gravity_scale(&mut self, scale: f64) {
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else {
            return;
        };
        scale_attribute(&mut p.attributes, "minecraft:gravity", "horizon_studio:gravity", scale);
    }
    fn set_look(&mut self, yaw: f32, pitch: f32) {
        let Some(g) = self.g_mut() else { return };
        if g.player.is_none() {
            return;
        }
        g.yaw = yaw;
        g.pitch = pitch.clamp(-90.0, 90.0);
    }
    fn turn_player(&mut self, dyaw: f32, dpitch: f32) {
        let Some(g) = self.g_mut() else { return };
        if g.player.is_none() {
            return;
        }
        g.yaw += dyaw;
        g.pitch = (g.pitch + dpitch).clamp(-90.0, 90.0);
    }
    fn set_camera_view(&mut self, view: Option<CameraView>) {
        let Some(g) = self.g_mut() else { return };
        // While the view is away from the player, their body shows (as in
        // third person); the game's own perspective comes back after.
        let current = g.camera_type;
        let back = HOLD.with(|h| {
            let mut h = h.borrow_mut();
            h.view = view;
            if view.is_some() {
                h.perspective.get_or_insert(current);
                None
            } else {
                h.perspective.take()
            }
        });
        if view.is_some() {
            g.camera_type = super::perspective::CameraType::ThirdPersonBack;
        } else if let Some(b) = back {
            g.camera_type = b;
        }
    }
    fn set_perspective(&mut self, first_person: bool) {
        use super::perspective::CameraType;
        let Some(g) = self.g_mut() else { return };
        let to = if first_person {
            CameraType::FirstPerson
        } else {
            CameraType::ThirdPersonBack
        };
        // A view held away from the player comes back to this one.
        let held = HOLD.with(|h| {
            let mut h = h.borrow_mut();
            h.perspective.as_mut().map(|p| *p = to).is_some()
        });
        if !held && g.camera_type != to {
            g.camera_type = to;
            g.check_entity_post_effect(first_person);
        }
    }
    fn set_camera_distance(&mut self, blocks: Option<f64>) {
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else { return };
        // The game's distance is 4 blocks (the attribute's base).
        let scale = blocks.map_or(1.0, |b| b.clamp(0.0, 400.0) / 4.0);
        scale_attribute(&mut p.attributes, "minecraft:camera_distance", "horizon_studio:zoom", scale);
    }
    fn sink_input(&mut self, keys: &[String], look: bool) {
        HOLD.with(|h| {
            let mut h = h.borrow_mut();
            h.sunk = keys.to_vec();
            h.look = look;
        });
        // A key held as a script takes it lets go in the game.
        if let Some(g) = self.g_mut() {
            g.keys
                .retain(|k| horizon_client::keybinds::key_name(*k).is_none_or(|n| !keys.iter().any(|s| s == n)));
        }
    }
    fn mouse_delta(&mut self) -> [f64; 2] {
        HOLD.with(|h| std::mem::take(&mut h.borrow_mut().mouse))
    }
    fn set_hitboxes(&mut self, on: bool) {
        let Some(g) = self.g_mut() else { return };
        // F3+B's switch, as the debug key turns it.
        if g.debug_entry("entity_hitboxes") != on {
            let overlay = g.debug;
            g.debug_entries.toggle_status("entity_hitboxes", overlay);
        }
    }
    fn set_mouse_sensitivity(&mut self, scale: f64) {
        HOLD.with(|h| h.borrow_mut().sensitivity = scale);
    }
    fn move_entity(&mut self, id: u32, feet: [f64; 3]) -> bool {
        let Some(g) = self.g_mut() else { return false };
        let at = horizon_core::geometry::Vec3::new(feet[0], feet[1], feet[2]);
        if let Some(m) = g.mobs.iter_mut().find(|m| m.id == id) {
            m.position = at;
            m.previous = at;
            return true;
        }
        if let Some(d) = g.dropped.iter_mut().find(|d| d.id == id) {
            d.position = at;
            d.previous = at;
            return true;
        }
        if let Some(o) = g.orbs.iter_mut().find(|o| o.id == id) {
            o.position = at;
            o.previous = at;
            return true;
        }
        false
    }
    fn press(&mut self, key: &str, down: bool) -> bool {
        let Some(g) = self.g_mut() else { return false };
        if g.screen != Screen::Playing || g.player.is_none() {
            return false;
        }
        // A click as the mouse's own: an attack or a use, and the mods' (the
        // studio's own on("key")) hear it.
        if let Some(button) = horizon_client::keybinds::mouse_button(key) {
            g.extras_mouse(button, down);
            return g.play_mouse(button, down).is_ok();
        }
        // A key held as the keyboard's: the player walks, jumps, sneaks with
        // it; the mods hear it.
        let Some(code) = horizon_client::keybinds::key_code(key) else {
            return false;
        };
        if down {
            g.keys.insert(code);
        } else {
            g.keys.remove(&code);
        }
        g.native_key_event(code, down, false);
        true
    }
    fn set_player_frozen(&mut self, on: bool) -> bool {
        HOLD.with(|h| h.borrow_mut().frozen = on);
        let Some(p) = self.g_mut().and_then(|g| g.player.as_mut()) else {
            return false;
        };
        // An anchored character: no walking, jumping or falling.
        let scale = if on { 0.0 } else { 1.0 };
        for attribute in ["minecraft:movement_speed", "minecraft:jump_strength", "minecraft:gravity"] {
            scale_attribute(&mut p.attributes, attribute, "horizon_studio:anchored", scale);
        }
        p.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
        true
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
        // As another screen taking its place does: the chat's draft stays.
        game.close_chat(super::chat::ChatExit::Interrupted);
    }
}

// ----------------------------------------------------------------- hooks

/// The vanilla mappings `keys.mapping` knows (Horizon's `native_mapping`).
const MAPPING_NAMES: [&str; 9] = [
    "key.forward",
    "key.left",
    "key.back",
    "key.right",
    "key.jump",
    "key.sneak",
    "key.sprint",
    "key.attack",
    "key.use",
];

/// Each frame, with the event loop (poll_control): the input the window
/// hook queued, the studio's frame, and opening or closing it.
pub(super) fn frame(game: &mut Game<'_>, event_loop: &dyn EventLoopCtl) {
    if !game.native_window() {
        return;
    }
    catch_up(game);
    // saveinstance's dumps: their copies of this frame's world, and the
    // container the player has open, remembered for them.
    if game.native_on(ID) {
        super::hstudio_world_dump::frame(game);
    }
    keep_view(game);
    remember_server(game);
    keep_still(game);
    // Once frames stop (a hidden window), the network thread answers the
    // server's pings itself: it learns here when the last frame began, and
    // which connection to answer on.
    horizon_protocol::hstudio_via_net::frame_at(game.previous);
    horizon_client::net::hstudio_via_lend::lend(game.remote.as_ref().map(|r| &r.session));
    // Natrium's switches follow its settings.
    super::hstudio_natrium::sync(game);
    // While the options are open, the studio's and Natrium's pages show what
    // is now (an agent connecting, what Natrium skipped).
    let frames = FRAMES.with(|f| {
        f.set(f.get().wrapping_add(1));
        f.get()
    });
    if game.screen == Screen::Options && frames % 20 == 0 {
        for id in [ID, super::hstudio_natrium::ID] {
            let row = game.settings.mods.installed.iter().position(|m| m.id == id && m.native.is_some());
            if let Some(i) = row {
                game.native_settings_refresh(i);
            }
        }
    }
    let mappings: Vec<(&'static str, (bool, String))> = MAPPING_NAMES
        .iter()
        .filter_map(|&name| game.native_mapping(name).map(|v| (name, v)))
        .collect();
    MAPPINGS_NOW.with(|m| *m.borrow_mut() = mappings);
    // ViaHorizon runs from the title screen on, while the studio is on.
    let on = game.native_on(ID);
    if on {
        via::configure(via_dir());
    }
    via::set_active(on);
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
            horizon_client::mobs::hstudio_body::set_noclip(false);
            super::hstudio_link::stop();
            chat_commands_off(game);
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
            // The player's input of this frame comes after the link's
            // requests: one that would open another file sees it coming.
            super::hstudio_coach::input_coming(st, &inputs);
            // The AI link's requests (horizon-studio-mcp), after the scripts' frame.
            super::hstudio_link::poll(st, &mut host);
            for path in dropped {
                let line = match st.install_path(&mut host, &path) {
                    Ok(line) => line,
                    Err(e) => format!("Could not install it: {e}"),
                };
                host.chat(&format!("[{NAME}] {line}"));
            }
            for input in inputs {
                // While it fades out, nothing more reaches it.
                if !st.overlay_open() || st.s.ui.closing_at.is_some() {
                    break;
                }
                let r = if st.s.screen.is_some() {
                    handle_screen(st, &mut host, input)
                } else {
                    handle(st, &mut host, input)
                };
                if r == UiResult::Close {
                    st.s.ui.closing_at = Some(st.s.now);
                }
            }
            if let Some(t) = st.s.ui.closing_at
                && st.s.now - t >= super::hstudio_look::CLOSE_SECS
            {
                st.s.ui.closing_at = None;
                if st.s.screen.is_some() {
                    st.close_screen(&mut host);
                    // Back to the studio under it, which arrives again.
                    st.s.ui.opened_at = st.s.now;
                } else {
                    st.s.ui.open = false;
                }
            }
            // The chat's command tree, after what this frame changed, and
            // the completions the chat asked for (with the event loop, so
            // suggest functions can use the agent).
            chat_commands(st, &mut host);
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
            st.s.ui.opened_at = st.s.now;
            st.s.ui.closing_at = None;
        }
        if was_open && !st.overlay_open() {
            give_back(game);
        }
        OPEN.with(|o| o.set(st.overlay_open()));
        let tool = super::hstudio_rbx_tools::takes_clicks(&st.s.rbx);
        TOOL_CLICKS.with(|c| c.set(tool && game.screen == Screen::Playing && !st.overlay_open()));
        let prompt = super::hstudio_rbx_parts::prompt_key(&st.s.rbx)
            .filter(|_| game.screen == Screen::Playing && !st.overlay_open());
        PROMPT_KEY.with(|k| *k.borrow_mut() = prompt);
        let chat_free = game.screen == Screen::Chat && !st.overlay_open();
        CHAT_FREE.with(|c| c.set(chat_free));
        st.set_chat_open(chat_free);
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
        flush(&mut host);
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
        flush(&mut host);
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
        // Ctrl+Space suggests and Ctrl+` shows the panel; Space is also typed text.
        KeyCode::Space => Key::Char(' '),
        KeyCode::Backquote => Key::Char('`'),
        KeyCode::ContextMenu => Key::Menu,
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
    // ViaHorizon's button and picker on the multiplayer screen.
    if super::hstudio_via_menu::input(event) {
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
        // A ProximityPrompt shown: its key is the prompt's, for the frame hook.
        if let WindowEvent::KeyboardInput { event: k, .. } = &*event
            && let PhysicalKey::Code(code) = k.physical_key
            && let Some(name) = horizon_client::keybinds::key_name(code)
            && PROMPT_KEY.with(|p| p.borrow().as_deref() == Some(name))
        {
            if !k.repeat {
                let down = k.state == ElementState::Pressed;
                with(|st| st.key(name, down));
            }
            *event = WindowEvent::HoveredFileCancelled;
            return;
        }
        // A script's tool in hand: the buttons are its, for the frame hook.
        if let WindowEvent::MouseInput { state, button, .. } = &*event
            && matches!(button, MouseButton::Left | MouseButton::Right)
        {
            let i = usize::from(*button == MouseButton::Right);
            let down = *state == ElementState::Pressed;
            let mut held = TOOL_HELD.with(Cell::get);
            let take = if down { TOOL_CLICKS.with(Cell::get) } else { held[i] };
            if take {
                held[i] = down;
                TOOL_HELD.with(|h| h.set(held));
                let key = if i == 0 { "key.mouse.left" } else { "key.mouse.right" };
                with(|st| st.key(key, down));
                *event = WindowEvent::HoveredFileCancelled;
                return;
            }
        }
        if sunk_key(event) {
            return;
        }
        // The chat open: the pointer, free, reaches the scripts' GUI, which
        // keeps a press or the wheel over it from the chat.
        if CHAT_FREE.with(Cell::get) {
            let scale = SCALE.with(Cell::get);
            let scale = if scale > 0.0 { scale } else { 1.0 };
            let input = match &*event {
                WindowEvent::CursorMoved { position, .. } => {
                    Some(Input::Mouse(position.x as f32 / scale, position.y as f32 / scale))
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let down = *state == ElementState::Pressed;
                    match button {
                        MouseButton::Left => Some(Input::Button { left: true, down }),
                        MouseButton::Right => Some(Input::Button { left: false, down }),
                        _ => None,
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => Some(Input::Wheel(match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 30.0) as f32,
                })),
                _ => None,
            };
            if let Some(input) = input
                && with(|st| st.chat_pointer(&input)).unwrap_or(false)
            {
                *event = WindowEvent::HoveredFileCancelled;
            }
        }
        return;
    }
    let mods = MODS.with(Cell::get);
    // Shift+F10 is the Menu key's twin (the game keeps F10 itself).
    let menu_key = |code: KeyCode| code == KeyCode::F10 && mods.shift && !is_open_key(code);
    let inputs: Vec<Input> = match &*event {
        WindowEvent::KeyboardInput { event: k, .. } => {
            let down = k.state == ElementState::Pressed;
            let key = match k.physical_key {
                PhysicalKey::Code(code) if is_open_key(code) => Key::Toggle,
                PhysicalKey::Code(code) if menu_key(code) => Key::Menu,
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
        // The left and right buttons, and the middle one for script screens;
        // the others (back, forward) reach neither the studio nor the game.
        WindowEvent::MouseInput { state, button, .. } => {
            let down = *state == ElementState::Pressed;
            match button {
                MouseButton::Left => vec![Input::Button { left: true, down }],
                MouseButton::Right => vec![Input::Button { left: false, down }],
                MouseButton::Middle => vec![Input::Middle { down }],
                _ => Vec::new(),
            }
        }
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
    // screenshots, the debug screen), but for Shift+F10.
    let function_key = matches!(event, WindowEvent::KeyboardInput { event: k, .. }
        if matches!(k.physical_key, PhysicalKey::Code(code) if is_function_key(code) && !is_open_key(code) && !menu_key(code)));
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

/// Wheel steps while playing: whether the scripts' Roblox tools take them
/// (past the hotbar's ends, and through their own slots). Back out of
/// theirs, the game's slot they come to is selected here.
pub(super) fn wheel(game: &mut Game<'_>, steps: f32) -> bool {
    if !game.native_window() || !game.native_on(ID) || game.player.is_none() {
        return false;
    }
    let slot = game.selected;
    let Some(r) = with(|st| super::hstudio_rbx_tools::wheel(&mut st.s.rbx, f64::from(steps), slot)).flatten() else {
        return false;
    };
    if let Some(to) = r {
        game.selected = to;
    }
    true
}

/// A command typed in chat: whether a script (or the studio) takes it.
pub(super) fn command(game: &mut Game<'_>, command: &str) -> bool {
    if !game.native_window() || !game.native_on(ID) {
        return false;
    }
    with(|st| st.command(command)).unwrap_or(false)
}

// ----------------------------------------------------------------- the chat's command tree

use super::hstudio_chatcmd::{ArgKind, split_line};
use super::hstudio_cmdtree::{ChatSync, NewNode, Tree};
use horizon_client::command::{ArgType, Dispatcher, MOD_COMMAND, Node, NodeKind, Registries, Suggests};

thread_local! {
    /// The studio's commands in the chat's tree, and the completion request
    /// the chat made for one of them, for the frame to answer.
    static CHAT: RefCell<ChatSync> = const { RefCell::new(ChatSync::new()) };
}

/// The chat's command tree for the world's connection (`Remote.chat_tree`), as
/// hstudio_cmdtree changes it, with the registries the chat parses with.
struct ChatTree<'g> {
    tree: &'g mut Dispatcher,
    registries: &'g Registries,
}

/// The tree the chat parses and completes with while connected (chat::chat_tree
/// picks it so): the server's once it sent one, the game's own before, each
/// with native mods' commands; Horizon builds it anew for each connection and
/// tree. Without a connection the chat has the game's own, which stays as it is.
fn chat_tree<'g>(game: &'g mut Game<'_>) -> Option<ChatTree<'g>> {
    let registries = &game.ui.registries;
    let tree = &mut game.remote.as_mut()?.chat_tree;
    Some(ChatTree { tree, registries })
}

/// The argument type the chat reads a kind with.
fn arg_type(kind: ArgKind) -> ArgType {
    match kind {
        // Anything up to the next space, as the studio splits words (Brigadier's
        // word takes only letters, digits and _-.+); one that starts with @
        // reads as a player selector.
        ArgKind::Word => ArgType::GameProfile,
        ArgKind::Text => ArgType::Greedy,
        ArgKind::Int { min, max } => ArgType::Integer { min, max },
        ArgKind::Number { min, max } => ArgType::Double { min, max },
        ArgKind::Bool => ArgType::Bool,
        // As Horizon's server mods' player argument (docs/mods.md).
        ArgKind::Player => ArgType::Entity {
            single: true,
            players: true,
        },
        ArgKind::Pos => ArgType::BlockPos,
        ArgKind::Item => ArgType::Item,
        ArgKind::Block => ArgType::BlockState,
    }
}

impl Tree for ChatTree<'_> {
    fn size(&self) -> usize {
        self.tree.nodes.len()
    }
    fn addr(&self) -> usize {
        self.tree.nodes.as_ptr() as usize
    }
    fn children(&self, node: usize) -> &[usize] {
        self.tree.nodes.get(node).map_or(&[], |n| n.children.as_slice())
    }
    fn set_children(&mut self, node: usize, children: Vec<usize>) {
        if let Some(n) = self.tree.nodes.get_mut(node) {
            n.children = children;
        }
    }
    fn literal(&self, node: usize) -> Option<&str> {
        match &self.tree.nodes.get(node)?.kind {
            NodeKind::Literal(l) => Some(l.as_str()),
            _ => None,
        }
    }
    fn redirect(&self, node: usize) -> Option<usize> {
        self.tree.nodes.get(node)?.redirect
    }
    fn set_redirect(&mut self, node: usize, to: usize) {
        if let Some(n) = self.tree.nodes.get_mut(node) {
            n.redirect = Some(to);
        }
    }
    fn push(&mut self, parent: Option<usize>, node: NewNode<'_>, executes: bool) -> usize {
        let (kind, children, suggests) = match node {
            // Shown after a redirect to it as "-> ..." (one to the root is "...").
            NewNode::RootCopy(children) => (NodeKind::Literal("...".to_owned()), children, None),
            NewNode::Literal(name) => (NodeKind::Literal(name.to_owned()), Vec::new(), None),
            NewNode::Argument { name, kind, asks } => {
                // The studio's completions, asked for as a server's are; a
                // word has none of its own (the game would offer player names).
                let suggests = if asks {
                    Some(Suggests::AskServer)
                } else if kind == ArgKind::Word {
                    Some(Suggests::Nothing)
                } else {
                    None
                };
                let kind = NodeKind::Argument {
                    name: name.to_owned(),
                    ty: arg_type(kind),
                };
                (kind, Vec::new(), suggests)
            }
        };
        let id = self.tree.nodes.len();
        self.tree.nodes.push(Node {
            kind,
            children,
            command: executes.then_some(MOD_COMMAND),
            redirect: None,
            // Every player may use them, as native mods' commands.
            permission: 0,
            suggests,
        });
        if let Some(p) = parent.and_then(|p| self.tree.nodes.get_mut(p)) {
            p.children.push(id);
        }
        id
    }
    fn truncate(&mut self, size: usize) {
        self.tree.nodes.truncate(size.max(1));
    }
    fn locate(&self, line: &str) -> (usize, usize) {
        // As the chat parses the line and finds what to complete
        // (ChatScreen::update_command_info, Dispatcher::suggestions_within).
        let from = usize::from(line.starts_with('/'));
        let parse = self.tree.parse_at(line, from, self.registries);
        let (node, start, _) = parse.context.suggestion_context(line.len());
        (node, start.min(line.len()))
    }
}

/// Whether no screen holds a parse of the chat's tree, so the studio's
/// nodes nothing links to can go: the chat shows no command (a line without
/// its `/` is not parsed) and no command block's command is being edited.
fn chat_quiet(game: &Game<'_>) -> bool {
    game.command_block_screen.is_none() && game.ui.chat_screen.as_ref().is_none_or(|s| !s.value().starts_with('/'))
}

/// Each frame: the chat's command tree gets the commands the studio takes
/// (none while a command block's command is edited: command blocks run on
/// the server), and the completion request the chat made for one of them
/// is answered, through the path a server's answer takes.
fn chat_commands(st: &mut Studio, host: &mut Host<'_, '_>) {
    let Some(game) = host.g_mut() else { return };
    let quiet = chat_quiet(game);
    let stamp = game.command_block_screen.is_none().then(|| st.command_stamp());
    let asked = CHAT.with(|c| {
        let mut c = c.borrow_mut();
        c.want(stamp, || st.chat_commands());
        let mut tree = chat_tree(game);
        c.sync(tree.as_mut().map(|t| t as &mut dyn Tree), quiet);
        c.take_asked(tree.as_ref().map(|t| t as &dyn Tree))
    });
    let Some(asked) = asked else { return };
    let answer = match asked.at {
        Some((arg, start)) => st.complete(host, &asked.line, arg, start),
        None => None,
    };
    let (range, list) = match answer {
        Some(c) => ((c.start, c.len), c.list),
        // Nothing to add: the chat shows its own, and the usage line.
        None => ((0, 0), Vec::new()),
    };
    if let Some(game) = host.g_mut() {
        game.chat_server_suggestions(asked.id, range, list);
    }
}

/// The chat's completion request, before the game sends it to the server
/// (remote.rs, send_suggestion_request): one for a command the studio takes
/// stays in the game and is answered at the frame, so nothing typed after
/// its name reaches the server; any other, one made while a name is still
/// being typed among them, is sent as the game sends it. False when the
/// chat has no request: the game then sends a command block's, as before.
pub(super) fn suggestion_request(game: &mut Game<'_>) -> bool {
    if game.remote.is_none() || !game.native_window() || !game.native_on(ID) {
        return false;
    }
    let Some((id, line)) = game.ui.chat_screen.as_mut().and_then(|s| s.take_request()) else {
        return false;
    };
    let name = split_line(&line).map_or("", |(name, _)| name);
    let takes = with(|st| st.takes(name)).unwrap_or(false);
    let kept = CHAT.with(|c| {
        let tree = chat_tree(game);
        c.borrow_mut().request(tree.as_ref().map(|t| t as &dyn Tree), id, &line, takes)
    });
    if !kept {
        // What send_suggestion_request does with it (remote.rs).
        let mut w = horizon_protocol::buf::Writer::new();
        w.varint(id);
        w.string(&line);
        game.send(horizon_protocol::packets::play::Serverbound::Unknown(
            horizon_protocol::ids::play::serverbound::COMMAND_SUGGESTION,
            w.data,
        ));
    }
    true
}

/// With the studio off: the chat's tree goes back to the server's commands.
fn chat_commands_off(game: &mut Game<'_>) {
    let quiet = chat_quiet(game);
    CHAT.with(|c| {
        let mut c = c.borrow_mut();
        c.want(None, Vec::new);
        let mut tree = chat_tree(game);
        c.sync(tree.as_mut().map(|t| t as &mut dyn Tree), quiet);
    });
}

/// A chat line from the server, for the scripts' `chat` handlers.
pub(super) fn chat_line(game: &Game<'_>, text: &horizon_client::text::Text, source: &horizon_client::chat::ChatSource) {
    if !game.native_window() || *source == horizon_client::chat::ChatSource::Client {
        return;
    }
    let text = text.plain();
    super::hstudio_link::record("chat", &text);
    with(|st| st.chat_line(&text));
}

/// The zoom the scripts ask for.
pub(super) fn zoom() -> Option<f32> {
    with(|st| st.zoom()).flatten()
}

pub(super) fn listed() -> super::native_hooks::NativeMod {
    let mut keys = vec![["open".into(), "Open Horizon Studio".into(), OPEN_KEY.into()]];
    keys.extend(super::hstudio_voicechat::mod_keys());
    super::native_hooks::NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: super::hstudio::DESCRIPTION.into(),
        keys,
    }
}

/// ViaHorizon's folder, beside the studio's (`native_data_dir`), known
/// before the game is (the startup menu).
fn via_dir() -> PathBuf {
    Game::options_path()
        .with_file_name(horizon_client::mod_list::DIR)
        .join(".horizon-mod-data")
        .join(ID)
        .join("viahorizon")
}

/// After the game's title screen draws: ViaHorizon's button on the
/// multiplayer list.
pub(super) fn via_menu(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_window() || !game.native_on(ID) {
        return;
    }
    super::hstudio_via_menu::draw(&game.gui.atlas, batch, game.title.as_ref(), game.cursor);
}

/// The same in the startup menu, before the game exists (ViaHorizon starts
/// there, so its downloads begin at once).
pub(super) fn via_startup(
    atlas: &horizon_client::hud::HudAtlas,
    batch: &mut HudBatch,
    title: Option<&horizon_client::title::Title>,
    cursor: (f32, f32),
    studio_on: bool,
) {
    if !studio_on {
        return;
    }
    via::configure(via_dir());
    via::set_active(true);
    super::hstudio_via_menu::draw(atlas, batch, title, cursor);
}

fn via_info() -> ViaInfo {
    let settings = via::settings();
    ViaInfo {
        enabled: settings.enabled,
        version: settings.version,
        status: via::status_text(),
        ready: matches!(via::status(), via::Status::Ready(_)),
        versions: via::VERSIONS.iter().map(|v| ((*v).to_owned(), via::label(v))).collect(),
        log: via::log(),
    }
}

/// Options > Mods > Horizon Studio: ViaHorizon's switch, the version joins
/// go as (from the title screen), its state, and reloading the scripts.
pub(super) fn settings(buttons: &mut Vec<String>) {
    let v = via::settings();
    buttons.push(format!("ViaHorizon: {}", if v.enabled { "On" } else { "Off" }));
    buttons.push(format!("Join servers as: {}", via::label(&v.version)));
    buttons.push("Previous version".into());
    buttons.push("Auto detect the version".into());
    buttons.push(via::status_text());
    let running = with(|st| st.s.mods.iter().filter(|m| m.running()).count()).unwrap_or(0);
    buttons.push(format!("Reload script mods ({running} running)"));
    let link = with(|st| super::hstudio_link::enabled(&st.s.dir)).unwrap_or(true);
    buttons.push(match (link, super::hstudio_link::agents_line()) {
        (false, _) => "AI link (MCP): Off".into(),
        (true, None) => "AI link (MCP): On, no agent connected".into(),
        (true, Some(line)) => format!("AI link (MCP): On, {line}"),
    });
    let copied = COPIED.with(Cell::get).is_some_and(|t| t.elapsed().as_secs_f32() < 3.0);
    buttons.push(if copied {
        "Copied: paste it into your AI agent".into()
    } else {
        "Copy AI setup prompt".into()
    });
    buttons.push(horizon_client::net::hstudio_mcp_update::label());
    // The voice chat's rows, from VOICE_ROW on.
    buttons.extend(super::hstudio_voicechat::global(|v| v.settings_buttons()));
}

/// The first of the voice chat's rows in the settings page.
const VOICE_ROW: usize = 9;

pub(super) fn settings_click(game: &mut Game<'_>, button: usize) {
    match button {
        0 => via::set_enabled(!via::settings().enabled),
        1 => {
            let _ = via::set_version(via::step_version(1));
        }
        2 => {
            let _ = via::set_version(via::step_version(-1));
        }
        3 => {
            let _ = via::set_version(via::AUTO);
        }
        // Its state: after a failure, a click tries again.
        4 => via::ensure(),
        5 => {
            with(|st| {
                let mut host = Host::new(game);
                st.start(&mut host);
                st.reload_all(&mut host);
            });
        }
        7 => {
            Host::new(game).set_clipboard(super::hstudio_link::SETUP_PROMPT);
            COPIED.with(|c| c.set(Some(Instant::now())));
        }
        6 => {
            with(|st| {
                let on = super::hstudio_link::enabled(&st.s.dir);
                super::hstudio_link::set_enabled(&st.s.dir, !on);
            });
        }
        // The MCP server: updated in place; running ones switch at their next call.
        8 => horizon_client::net::hstudio_mcp_update::click(),
        n if n >= VOICE_ROW => super::hstudio_voicechat::global(|v| v.settings_click(n - VOICE_ROW)),
        _ => {}
    }
}

// ----------------------------------------------------------------- the client, for Roblox scripts

thread_local! {
    /// The server joined last (address, player name), to rejoin once it is left.
    static LAST_SERVER: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    /// The HUD's parts Roblox scripts hid: the chat and the player list.
    static HIDE_CHAT: Cell<bool> = const { Cell::new(false) };
    static HIDE_TAB: Cell<bool> = const { Cell::new(false) };
    /// The player's graphics settings while scripts have them lowered.
    static GRAPHICS: RefCell<Option<Graphics>> = const { RefCell::new(None) };
    /// The lights Roblox scripts put in the world.
    static LIGHTS: RefCell<Vec<([f64; 3], u32, f32, f32)>> = const { RefCell::new(Vec::new()) };
}

/// The graphics settings `ClientAction::LowGraphics` lowers.
struct Graphics {
    render_distance: i32,
    particles: u8,
    clouds: String,
    entity_shadows: bool,
    entity_distance: f32,
    biome_blend: i32,
    weather_radius: i32,
}

fn graphics(g: &Game<'_>) -> Graphics {
    let s = &g.settings;
    Graphics {
        render_distance: s.render_distance,
        particles: s.particles,
        clouds: s.clouds.clone(),
        entity_shadows: s.entity_shadows,
        entity_distance: s.entity_distance,
        biome_blend: s.biome_blend,
        weather_radius: s.weather_radius,
    }
}

fn set_graphics(g: &mut Game<'_>, v: Graphics) {
    let s = &mut g.settings;
    s.render_distance = v.render_distance;
    s.particles = v.particles;
    s.clouds = v.clouds;
    s.entity_shadows = v.entity_shadows;
    s.entity_distance = v.entity_distance;
    s.biome_blend = v.biome_blend;
    s.weather_radius = v.weather_radius;
    g.apply_settings();
}

/// The server the player is on, how the last connection ended (the
/// Disconnected screen's reason), and the settings Roblox scripts read.
fn client_info(g: &Game<'_>) -> super::hstudio::ClientInfo {
    use horizon_client::multiplayer::Screen as Multiplayer;
    use horizon_client::title::Mode;
    let own = g.integrated.as_ref().map(|i| i.world_name.clone());
    let server = match (g.remote.as_ref(), own) {
        (_, Some(world)) => Some(super::hstudio::ServerInfo {
            address: String::new(),
            name: world,
            brand: g.remote.as_ref().map(|r| r.brand.clone()).unwrap_or_default(),
            own_world: true,
        }),
        (Some(r), None) => Some(super::hstudio::ServerInfo {
            address: r.session.address.clone(),
            name: horizon_client::net::shown_address(&r.session.address),
            brand: r.brand.clone(),
            own_world: false,
        }),
        (None, None) => None,
    };
    let disconnected = match g.title.as_ref().map(|t| &t.mode) {
        Some(Mode::Multiplayer(Multiplayer::Disconnected { reason, .. })) => Some(reason.clone()),
        _ => None,
    };
    super::hstudio::ClientInfo {
        server,
        disconnected,
        master_volume: g.settings.master_volume,
        fullscreen: g.settings.fullscreen,
    }
}

/// Each frame: the server the player is on, to come back to.
fn remember_server(g: &Game<'_>) {
    if g.integrated.is_none()
        && let Some(r) = g.remote.as_ref()
    {
        let (address, name) = (&r.session.address, &r.session.name);
        LAST_SERVER.with(|l| {
            let known = l.borrow().as_ref().is_some_and(|(a, n)| a == address && n == name);
            if !known {
                *l.borrow_mut() = Some((address.clone(), name.clone()));
            }
        });
    }
}

/// Whether the client can do it now (said at once, as Roblox's would).
fn client_check(g: &Game<'_>, action: &super::hstudio::ClientAction) -> Result<(), String> {
    use super::hstudio::ClientAction as A;
    match action {
        A::Rejoin if g.integrated.is_some() => Err("a world of the player's own has no server to rejoin".into()),
        A::Rejoin if g.remote.is_none() && LAST_SERVER.with(|l| l.borrow().is_none()) => {
            Err("no server was joined to rejoin".into())
        }
        _ => Ok(()),
    }
}

/// Does what a Roblox script asked of the client, at a frame's start.
fn client_now(game: &mut Game<'_>, action: super::hstudio::ClientAction) {
    use super::hstudio::ClientAction as A;
    match action {
        A::Leave(reason) => {
            if game.remote.is_some() || game.integrated.is_some() {
                if let Err(e) = game.leave_server(reason) {
                    log::warn!("Horizon Studio: leaving the server: {e}");
                }
            }
        }
        A::Rejoin => {
            let here = game
                .remote
                .as_ref()
                .filter(|_| game.integrated.is_none())
                .map(|r| (r.session.address.clone(), r.session.name.clone()));
            let Some((address, name)) = here.or_else(|| LAST_SERVER.with(|l| l.borrow().clone())) else {
                return;
            };
            if game.remote.is_some()
                && let Err(e) = game.leave_server(None)
            {
                log::warn!("Horizon Studio: leaving the server to rejoin it: {e}");
                return;
            }
            if let Err(e) = game.connect_server(&address, &name) {
                log::warn!("Horizon Studio: rejoining {address}: {e}");
            }
        }
        A::MasterVolume(v) => {
            let v = v.clamp(0.0, 1.0);
            game.settings.master_volume = v;
            game.audio.master = v;
        }
        A::LowGraphics(true) => {
            if GRAPHICS.with(|g| g.borrow().is_some()) {
                return;
            }
            let was = graphics(game);
            let low = Graphics {
                render_distance: was.render_distance.min(8),
                particles: 2,
                clouds: "false".into(),
                entity_shadows: false,
                entity_distance: was.entity_distance.min(0.5),
                biome_blend: 0,
                weather_radius: was.weather_radius.min(3),
            };
            GRAPHICS.with(|g| *g.borrow_mut() = Some(was));
            set_graphics(game, low);
        }
        A::LowGraphics(false) => {
            if let Some(was) = GRAPHICS.with(|g| g.borrow_mut().take()) {
                set_graphics(game, was);
            }
        }
        A::ToggleFullscreen => game.toggle_fullscreen(),
        // The hotbar slot held, said again: the server counts it as the
        // player's action (its idle timer starts over), and nothing changes.
        A::Activity => {
            if game.remote.is_some() && game.integrated.is_none() {
                game.send(horizon_protocol::packets::play::Serverbound::SetCarriedItem(
                    horizon_protocol::packets::play::SetCarriedItem {
                        slot: game.selected as i16,
                    },
                ));
            }
        }
        A::MuteVoice(who, muted) => super::hstudio_voicechat::global(|v| v.vc.mute_voice(who, muted)),
        A::VoiceListener(at) => super::hstudio_voicechat::global(|v| v.listener_at = at),
        A::Hud(part, shown) => match part.as_str() {
            "chat" => HIDE_CHAT.with(|c| c.set(!shown)),
            "playerlist" => HIDE_TAB.with(|c| c.set(!shown)),
            "all" => game.ui.hide_gui = !shown,
            _ => {}
        },
        A::WorldRendering(on) => horizon_core::natrium::set_world_hidden(!on),
        A::Lights(list) => LIGHTS.with(|l| *l.borrow_mut() = list),
        A::ChatBackground(opacity) => {
            game.settings.text_background_opacity = opacity.clamp(0.0, 1.0);
            game.apply_settings();
        }
    }
}

/// The world frame's lights (native_visuals): those Roblox scripts put in
/// the world, which the game validates and limits.
pub(super) fn visual_lights(game: &Game<'_>, lights: &mut Vec<super::native_hooks::NativeColoredLight>) {
    if !game.native_on(ID) {
        return;
    }
    LIGHTS.with(|l| {
        lights.extend(l.borrow().iter().map(|&(at, rgb, radius, intensity)| {
            super::native_hooks::NativeColoredLight {
                at,
                rgb,
                radius,
                intensity,
            }
        }));
    });
}

/// The chat's lines over the HUD are drawn unless a Roblox script hid the
/// chat (the core GUI's Chat, TextChatService's chat window).
pub(super) fn chat_hidden(game: &Game<'_>) -> bool {
    game.screen != Screen::Chat && game.native_on(ID) && HIDE_CHAT.with(Cell::get)
}

/// The player list (Tab) is drawn unless a Roblox script hid it.
pub(super) fn player_list_hidden(game: &Game<'_>) -> bool {
    game.native_on(ID) && HIDE_TAB.with(Cell::get)
}

// ----------------------------------------------------------------- packets
//
// The game's play packets for the Roblox layer's remotes
// (hstudio_rbx_remote): each packet Horizon models with plain fields maps
// to a table of them and back (`mapped!`); any other is read from its
// `Debug` form, to look at, and sent from its payload. What a script sends
// is checked as the server will read it: it decodes as the packet, whole,
// and its positions, angles and enums are ones vanilla takes.

use super::hstudio_rbx_remote::{self as remote, Field, Packet, Verdict};
use horizon_protocol::packets::play::{Clientbound as C, Serverbound as S};
use horizon_protocol::packets::{Empty, configuration as cfg, play as pp, recipes, status};

thread_local! {
    /// While the bridge sends what the scripts asked for at the last frame:
    /// those packets are the scripts' own, not the game's.
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

/// A field's value from the packet's and back.
trait PField: Sized {
    fn to_field(&self) -> Field;
    fn from_field(f: &Field) -> Result<Self, String>;
}

fn whole(f: &Field, min: i128, max: i128) -> Result<i128, String> {
    let n = match f {
        Field::Int(n) => *n,
        Field::Num(n) if n.fract() == 0.0 && n.is_finite() => *n as i128,
        Field::Str(s) => std::str::from_utf8(s)
            .ok()
            .and_then(|t| t.trim().parse::<i128>().ok())
            .ok_or("expected a whole number")?,
        Field::Nil => return Err("missing".into()),
        _ => return Err("expected a whole number".into()),
    };
    if n < min || n > max {
        return Err(format!("{n} is out of range ({min} to {max})"));
    }
    Ok(n)
}

macro_rules! whole_field {
    ($($t:ty),*) => {$(
        impl PField for $t {
            fn to_field(&self) -> Field {
                Field::Int(i128::from(*self))
            }
            fn from_field(f: &Field) -> Result<Self, String> {
                Ok(whole(f, i128::from(<$t>::MIN), i128::from(<$t>::MAX))? as $t)
            }
        }
    )*};
}
whole_field!(i8, u8, i16, i32, i64);

fn number(f: &Field) -> Result<f64, String> {
    match f {
        Field::Num(n) => Ok(*n),
        Field::Int(n) => Ok(*n as f64),
        Field::Nil => Err("missing".into()),
        _ => Err("expected a number".into()),
    }
}

impl PField for f64 {
    fn to_field(&self) -> Field {
        Field::Num(*self)
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        number(f)
    }
}

impl PField for f32 {
    fn to_field(&self) -> Field {
        Field::Num(f64::from(*self))
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        let n = number(f)?;
        if n.is_finite() && n.abs() > f64::from(f32::MAX) {
            return Err(format!("{n} is out of range for a float"));
        }
        Ok(n as f32)
    }
}

impl PField for bool {
    fn to_field(&self) -> Field {
        Field::Bool(*self)
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        match f {
            Field::Bool(b) => Ok(*b),
            Field::Nil => Err("missing".into()),
            _ => Err("expected a boolean".into()),
        }
    }
}

impl PField for String {
    fn to_field(&self) -> Field {
        Field::text(self)
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        match f {
            Field::Str(s) => String::from_utf8(s.clone()).map_err(|_| "expected text (UTF-8)".into()),
            Field::Nil => Err("missing".into()),
            _ => Err("expected a string".into()),
        }
    }
}

impl PField for Vec<u8> {
    fn to_field(&self) -> Field {
        Field::Str(self.clone())
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        match f {
            Field::Str(s) => Ok(s.clone()),
            Field::Nil => Err("missing".into()),
            _ => Err("expected bytes (a string or buffer)".into()),
        }
    }
}

/// A UUID: its usual text (with or without dashes).
impl PField for u128 {
    fn to_field(&self) -> Field {
        let h = format!("{self:032x}");
        Field::text(&format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        ))
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        let Field::Str(s) = f else {
            return Err(if *f == Field::Nil {
                "missing"
            } else {
                "expected a UUID string"
            }
            .into());
        };
        let hex: String = String::from_utf8_lossy(s).chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 {
            return Err("expected a UUID (32 hex digits)".into());
        }
        u128::from_str_radix(&hex, 16).map_err(|_| "expected a UUID (32 hex digits)".into())
    }
}

fn three(f: &Field) -> Result<[f64; 3], String> {
    match f {
        Field::Vec3(v) => Ok(*v),
        Field::List(l) if l.len() == 3 => Ok([number(&l[0])?, number(&l[1])?, number(&l[2])?]),
        Field::Nil => Err("missing".into()),
        _ => Err("expected a Vector3".into()),
    }
}

impl PField for [f64; 3] {
    fn to_field(&self) -> Field {
        Field::Vec3(*self)
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        three(f)
    }
}

impl PField for [f32; 3] {
    fn to_field(&self) -> Field {
        Field::Vec3(self.map(f64::from))
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        let v = three(f)?;
        Ok(v.map(|x| x as f32))
    }
}

/// A block position: a Vector3 of whole numbers.
impl PField for [i32; 3] {
    fn to_field(&self) -> Field {
        Field::Vec3(self.map(f64::from))
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        let v = three(f)?;
        let mut out = [0; 3];
        for (o, x) in out.iter_mut().zip(v) {
            if x.fract() != 0.0 || !(-33_554_432.0..=33_554_431.0).contains(&x) {
                return Err("expected a block position (whole numbers, within the world)".into());
            }
            *o = x as i32;
        }
        Ok(out)
    }
}

impl<T: PField> PField for Option<T> {
    fn to_field(&self) -> Field {
        self.as_ref().map_or(Field::Nil, PField::to_field)
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        match f {
            Field::Nil => Ok(None),
            other => T::from_field(other).map(Some),
        }
    }
}

impl PField for Vec<String> {
    fn to_field(&self) -> Field {
        Field::List(self.iter().map(PField::to_field).collect())
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        match f {
            Field::List(l) => l.iter().map(String::from_field).collect(),
            Field::Map(m) if m.is_empty() => Ok(Vec::new()),
            Field::Nil => Err("missing".into()),
            _ => Err("expected a list of strings".into()),
        }
    }
}

impl PField for [String; 4] {
    fn to_field(&self) -> Field {
        Field::List(self.iter().map(PField::to_field).collect())
    }
    fn from_field(f: &Field) -> Result<Self, String> {
        let list = Vec::<String>::from_field(f)?;
        <[String; 4]>::try_from(list).map_err(|_| "expected four lines".into())
    }
}

/// Field `name` of a script's fields, read as the packet's.
fn take<T: PField>(fields: &[(String, Field)], name: &str) -> Result<T, String> {
    let f = fields.iter().find(|(k, _)| k == name).map_or(&Field::Nil, |(_, v)| v);
    T::from_field(f).map_err(|e| format!("{name}: {e}"))
}

/// The packets with plain fields: Variant(Type) = ID { fields }.
macro_rules! mapped {
    ($($variant:ident($($ty:ident)::+) = $id:ident { $($f:ident),* $(,)? }),* $(,)?) => {
        fn mapped_fields(p: &S) -> Option<Vec<(String, Field)>> {
            match p {
                $(S::$variant(_v) => Some(vec![$((stringify!($f).to_owned(), PField::to_field(&_v.$f))),*]),)*
                _ => None,
            }
        }
        fn field_names(id: i32) -> Option<&'static [&'static str]> {
            use horizon_protocol::ids::play::serverbound as ids;
            $(if id == ids::$id {
                return Some(&[$(stringify!($f)),*]);
            })*
            None
        }
        #[allow(clippy::redundant_closure_call)]
        fn from_mapped(id: i32, _fields: &[(String, Field)]) -> Option<Result<S, String>> {
            use horizon_protocol::ids::play::serverbound as ids;
            $(if id == ids::$id {
                return Some((|| -> Result<S, String> {
                    Ok(S::$variant($($ty)::+ { $($f: take(_fields, stringify!($f))?),* }))
                })());
            })*
            None
        }
    };
}

mapped! {
    AcceptTeleportation(pp::AcceptTeleportation) = ACCEPT_TELEPORTATION { id, x, y, z, yaw, pitch },
    Attack(pp::Attack) = ATTACK { id },
    BlockEntityTagQuery(pp::BlockEntityTagQuery) = BLOCK_ENTITY_TAG_QUERY { transaction, pos },
    ChangeDifficulty(pp::ChangeDifficultyRequest) = CHANGE_DIFFICULTY { difficulty },
    LockDifficulty(pp::LockDifficulty) = LOCK_DIFFICULTY { locked },
    ChangeGameMode(pp::ChangeGameMode) = CHANGE_GAME_MODE { mode },
    ChatAck(pp::ChatAck) = CHAT_ACK { offset },
    ChatCommand(pp::ChatCommand) = CHAT_COMMAND { command },
    ChatSessionUpdate(pp::ChatSessionUpdate) = CHAT_SESSION_UPDATE { session, expires_at, key, key_signature },
    ChunkBatchReceived(pp::ChunkBatchReceived) = CHUNK_BATCH_RECEIVED { chunks_per_tick },
    ClientCommand(pp::ClientCommand) = CLIENT_COMMAND { action },
    ClientTickEnd(Empty) = CLIENT_TICK_END {},
    ConfigurationAcknowledged(Empty) = CONFIGURATION_ACKNOWLEDGED {},
    ContainerButtonClick(pp::ContainerButtonClick) = CONTAINER_BUTTON_CLICK { container, button },
    ContainerClose(pp::ContainerClose) = CONTAINER_CLOSE { container },
    CookieResponse(cfg::CookieResponse) = COOKIE_RESPONSE { key, payload },
    CustomPayload(cfg::CustomPayload) = CUSTOM_PAYLOAD { channel, data },
    EditBook(pp::EditBook) = EDIT_BOOK { slot, pages, title },
    EntityTagQuery(pp::EntityTagQuery) = ENTITY_TAG_QUERY { transaction, entity },
    Interact(pp::Interact) = INTERACT { id, hand, location, secondary },
    JigsawGenerate(pp::JigsawGenerate) = JIGSAW_GENERATE { pos, levels, keep_jigsaws },
    KeepAlive(cfg::KeepAlive) = KEEP_ALIVE { id },
    MovePlayerPos(pp::MovePlayerPos) = MOVE_PLAYER_POS { x, y, z, flags },
    MovePlayerPosRot(pp::MovePlayerPosRot) = MOVE_PLAYER_POS_ROT { x, y, z, yaw, pitch, flags },
    MovePlayerRot(pp::MovePlayerRot) = MOVE_PLAYER_ROT { yaw, pitch, flags },
    MovePlayerStatus(pp::MovePlayerStatus) = MOVE_PLAYER_STATUS_ONLY { flags },
    MoveVehicle(pp::MoveVehicleRequest) = MOVE_VEHICLE { position, yaw, pitch, on_ground },
    PaddleBoat(pp::PaddleBoat) = PADDLE_BOAT { left, right },
    PickItemFromBlock(pp::PickItemFromBlock) = PICK_ITEM_FROM_BLOCK { pos, include_data },
    PickItemFromEntity(pp::PickItemFromEntity) = PICK_ITEM_FROM_ENTITY { id, include_data },
    PingRequest(status::Ping) = PING_REQUEST { time },
    PlayerAbilities(pp::PlayerAbilitiesRequest) = PLAYER_ABILITIES { flags },
    PlayerAction(pp::PlayerAction) = PLAYER_ACTION { action, pos, direction, sequence },
    PlayerCommand(pp::PlayerCommand) = PLAYER_COMMAND { id, action, data },
    PlayerInput(pp::PlayerInput) = PLAYER_INPUT { flags },
    PlayerLoaded(Empty) = PLAYER_LOADED {},
    Pong(cfg::Ping) = PONG { id },
    RenameItem(pp::RenameItem) = RENAME_ITEM { name },
    ContainerSlotStateChanged(pp::ContainerSlotStateChanged) = CONTAINER_SLOT_STATE_CHANGED { slot, container, enabled },
    Punch(Empty) = PUNCH {},
    ResourcePack(cfg::ResourcePackResponse) = RESOURCE_PACK { id, action },
    SelectBundleItem(pp::SelectBundleItem) = BUNDLE_ITEM_SELECTED { slot, index },
    SelectTrade(pp::SelectTrade) = SELECT_TRADE { item },
    SetBeacon(pp::SetBeacon) = SET_BEACON { primary, secondary },
    SetCarriedItem(pp::SetCarriedItem) = SET_CARRIED_ITEM { slot },
    SetCommandBlock(pp::SetCommandBlock) = SET_COMMAND_BLOCK { pos, command, mode, flags },
    SetCommandMinecart(pp::SetCommandMinecart) = SET_COMMAND_MINECART { entity, command, track_output },
    SetJigsawBlock(pp::SetJigsawBlock) = SET_JIGSAW_BLOCK {
        pos, name, target, pool, final_state, joint, selection_priority, placement_priority
    },
    SetTestBlock(pp::SetTestBlock) = SET_TEST_BLOCK { pos, mode, message },
    SignUpdate(pp::SignUpdate) = SIGN_UPDATE { pos, lines, front },
    SpectatorAction(pp::SpectatorAction) = SPECTATOR_ACTION { target },
    TeleportToEntity(pp::TeleportToEntity) = TELEPORT_TO_ENTITY { uuid },
    UseItem(pp::UseItem) = USE_ITEM { hand, sequence, yaw, pitch },
    UseItemOn(pp::UseItemOn) = USE_ITEM_ON { hand, pos, face, cursor, inside, world_border, sequence },
    PlaceRecipe(recipes::PlaceRecipe) = PLACE_RECIPE { container, recipe, use_max },
    RecipeBookChangeSettings(recipes::RecipeBookChangeSettings) = RECIPE_BOOK_CHANGE_SETTINGS { book_type, open, filtering },
}

/// Packets only the game sends: one wrong (a keep-alive's id, a chat
/// session's key, the chat's acknowledgements, the state's switch)
/// disconnects the player.
const GAME_ONLY: &[i32] = &[
    horizon_protocol::ids::play::serverbound::KEEP_ALIVE,
    horizon_protocol::ids::play::serverbound::CONFIGURATION_ACKNOWLEDGED,
    horizon_protocol::ids::play::serverbound::CHAT_SESSION_UPDATE,
    horizon_protocol::ids::play::serverbound::CHAT_ACK,
];

/// A packet's payload: its body after the id.
fn payload(body: &[u8]) -> &[u8] {
    let mut r = horizon_protocol::buf::Reader::new(body);
    let _ = r.varint();
    r.rest()
}

/// `{:?}`, if it is at most `max` bytes (a chunk's is far longer).
fn debug_text(v: &dyn std::fmt::Debug, max: usize) -> Option<String> {
    struct Capped(String, usize);
    impl std::fmt::Write for Capped {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            if self.0.len() + s.len() > self.1 {
                return Err(std::fmt::Error);
            }
            self.0.push_str(s);
            Ok(())
        }
    }
    let mut c = Capped(String::new(), max);
    std::fmt::write(&mut c, format_args!("{v:?}")).ok()?;
    Some(c.0)
}

/// The most of a packet's `Debug` form read as its fields.
const MAX_DEBUG: usize = 64 * 1024;

/// ServerboundCommandSuggestionPacket, which Horizon sends as bytes: its
/// id and the command typed.
fn suggestion_fields(data: &[u8]) -> Option<Vec<(String, Field)>> {
    let mut r = horizon_protocol::buf::Reader::new(data);
    let id = r.varint().ok()?;
    let command = r.string(32500).ok()?;
    (r.remaining() == 0).then(|| {
        vec![
            ("id".into(), Field::Int(id.into())),
            ("command".into(), Field::text(&command)),
        ]
    })
}

/// ClientboundCommandSuggestionsPacket (bytes to Horizon too): the id, the
/// range replaced and each suggestion's text and tooltip.
fn suggestions_fields(data: &[u8]) -> Option<Vec<(String, Field)>> {
    let mut r = horizon_protocol::buf::Reader::new(data);
    let id = r.varint().ok()?;
    let start = r.varint().ok()?;
    let length = r.varint().ok()?;
    let n = r.count(4096).ok()?;
    let mut list = Vec::with_capacity(n);
    for _ in 0..n {
        let text = r.string(32767).ok()?;
        let mut entry = vec![("text".to_owned(), Field::text(&text))];
        if r.bool().ok()? {
            let tag = r.nbt().ok()?;
            if let Some(f) = debug_text(&tag, MAX_DEBUG).and_then(|t| remote::debug_value(&t)) {
                entry.push(("tooltip".into(), f));
            }
        }
        list.push(Field::Map(entry));
    }
    (r.remaining() == 0).then(|| {
        vec![
            ("id".into(), Field::Int(id.into())),
            ("start".into(), Field::Int(start.into())),
            ("length".into(), Field::Int(length.into())),
            ("suggestions".into(), Field::List(list)),
        ]
    })
}

/// One of the game's serverbound packets for the scripts.
fn serverbound_packet(p: &S) -> Packet {
    use horizon_protocol::ids::play::serverbound as ids;
    let id = p.id();
    let name = ids::NAMES.get(id as usize).copied().unwrap_or("unknown").to_owned();
    if let Some(fields) = mapped_fields(p) {
        return Packet {
            id,
            name,
            fields,
            mapped: true,
            raw: None,
        };
    }
    if let S::Unknown(ids::COMMAND_SUGGESTION, data) = p
        && let Some(fields) = suggestion_fields(data)
    {
        return Packet {
            id,
            name,
            fields,
            mapped: true,
            raw: None,
        };
    }
    let body = p.encode();
    Packet {
        id,
        name,
        fields: debug_text(p, MAX_DEBUG)
            .and_then(|t| remote::debug_fields(&t))
            .unwrap_or_default(),
        mapped: false,
        raw: Some(payload(&body).to_vec()),
    }
}

/// One of the server's packets for the scripts.
fn clientbound_packet(p: &C) -> Packet {
    use horizon_protocol::ids::play::clientbound as ids;
    let id = p.id();
    let name = ids::NAMES.get(id as usize).copied().unwrap_or("unknown").to_owned();
    let body = p.encode();
    let raw = payload(&body).to_vec();
    let fields = match p {
        C::Unknown(ids::COMMAND_SUGGESTIONS, data) => suggestions_fields(data),
        _ => debug_text(p, MAX_DEBUG).and_then(|t| remote::debug_fields(&t)),
    };
    Packet {
        id,
        name,
        fields: fields.unwrap_or_default(),
        mapped: false,
        raw: Some(raw),
    }
}

fn finite(what: &str, v: &[f64]) -> Result<(), String> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(format!("{what} must be finite numbers"))
    }
}

/// A place the server takes a player to: inside the world's bounds.
fn place(x: f64, y: f64, z: f64) -> Result<(), String> {
    finite("the position", &[x, y, z])?;
    if x.abs() > 3.0e7 || z.abs() > 3.0e7 || y.abs() > 2.0e7 {
        return Err("the position is outside the world (the server disconnects for it)".into());
    }
    Ok(())
}

fn one_of(what: &str, v: i32, range: std::ops::RangeInclusive<i32>) -> Result<(), String> {
    if range.contains(&v) {
        Ok(())
    } else {
        Err(format!(
            "{what} {v} isn't one the server knows ({} to {})",
            range.start(),
            range.end()
        ))
    }
}

/// What vanilla would disconnect the player for, though it decodes.
fn sane(p: &S) -> Result<(), String> {
    match p {
        S::MovePlayerPos(m) => place(m.x, m.y, m.z),
        S::MovePlayerPosRot(m) => {
            place(m.x, m.y, m.z)?;
            finite("yaw and pitch", &[f64::from(m.yaw), f64::from(m.pitch)])
        }
        S::MovePlayerRot(m) => finite("yaw and pitch", &[f64::from(m.yaw), f64::from(m.pitch)]),
        S::MoveVehicle(m) => {
            place(m.position[0], m.position[1], m.position[2])?;
            finite("yaw and pitch", &[f64::from(m.yaw), f64::from(m.pitch)])
        }
        S::AcceptTeleportation(m) => place(m.x, m.y, m.z),
        S::Interact(m) => {
            one_of("hand", m.hand, 0..=1)?;
            finite("location", &m.location)
        }
        S::UseItem(m) => {
            one_of("hand", m.hand, 0..=1)?;
            one_of("sequence", m.sequence, 0..=i32::MAX)?;
            finite("yaw and pitch", &[f64::from(m.yaw), f64::from(m.pitch)])
        }
        S::UseItemOn(m) => {
            one_of("hand", m.hand, 0..=1)?;
            one_of("face", m.face, 0..=5)?;
            one_of("sequence", m.sequence, 0..=i32::MAX)?;
            finite("cursor", &m.cursor.map(f64::from))
        }
        S::PlayerAction(m) => {
            one_of("action", m.action, 0..=8)?;
            one_of("direction", i32::from(m.direction), 0..=5)?;
            one_of("sequence", m.sequence, 0..=i32::MAX)
        }
        S::PlayerCommand(m) => one_of("action", m.action, 0..=6),
        S::ClientCommand(m) => one_of("action", m.action, 0..=2),
        S::ChangeGameMode(m) => one_of("mode", m.mode, 0..=3),
        S::ChangeDifficulty(m) => one_of("difficulty", m.difficulty, 0..=3),
        S::SetCarriedItem(m) => one_of("slot", i32::from(m.slot), 0..=8),
        _ => Ok(()),
    }
}

/// A script's packet as the game's: from its fields or its payload, checked.
fn build(p: &Packet) -> Result<S, String> {
    use horizon_protocol::ids::play::serverbound as ids;
    if GAME_ONLY.contains(&p.id) {
        return Err("only the game sends it: a wrong one disconnects the player".into());
    }
    let s = match &p.raw {
        Some(raw) => S::decode(p.id, raw).map_err(|e| format!("its payload doesn't read as the packet: {e}"))?,
        None => {
            if let Some(names) = field_names(p.id)
                && let Some((k, _)) = p.fields.iter().find(|(k, _)| !names.contains(&k.as_str()))
            {
                return Err(format!("it has no field {k} (its fields: {})", names.join(", ")));
            }
            match from_mapped(p.id, &p.fields) {
                Some(r) => r?,
                None if p.id == ids::COMMAND_SUGGESTION => {
                    let id: i32 = take(&p.fields, "id")?;
                    let command: String = take(&p.fields, "command")?;
                    let mut w = horizon_protocol::buf::Writer::new();
                    w.varint(id);
                    w.string(&command);
                    S::Unknown(ids::COMMAND_SUGGESTION, w.data)
                }
                None => {
                    return Err(
                        "its fields don't map to the packet: give its payload (FireServer(fields, payload))".into(),
                    );
                }
            }
        }
    };
    if let S::Unknown(id, data) = &s
        && (*id != ids::COMMAND_SUGGESTION || suggestion_fields(data).is_none())
    {
        return Err("Horizon doesn't model this packet, so it can't check it, and doesn't send it".into());
    }
    // As the server reads it: the whole of it, as this packet.
    let body = s.encode();
    match S::decode(p.id, payload(&body)) {
        Ok(back) if back == s => {}
        Ok(_) => return Err("the server would read it as another packet".into()),
        Err(e) => return Err(format!("the server couldn't read it: {e}")),
    }
    sane(&s)?;
    Ok(s)
}

/// Sends a packet straight to the server, past the scripts' hooks.
fn send_now(game: &Game<'_>, s: S) -> Result<(), String> {
    match &game.remote {
        Some(r) => {
            r.session.send(s);
            Ok(())
        }
        None => Err("not connected to a server".into()),
    }
}

/// Game::send (every serverbound play packet of the game's): through the
/// scripts' `__namecall` hook while there is one. True: the hook stopped
/// it (or sent another in its place).
pub(super) fn packet_out(game: &Game<'_>, packet: &S) -> bool {
    if game.native_window() && game.native_on(ID) {
        super::hstudio_world_dump::sent(packet);
    }
    if QUIET.with(Cell::get) || !game.native_window() || !game.native_on(ID) {
        return false;
    }
    with(|st| {
        if !st.packets_spied() {
            return false;
        }
        let p = serverbound_packet(packet);
        let mut host = Host {
            game: GameRef::Shared(game),
            event_loop: None,
            batch: None,
            corners: None,
            world: None,
        };
        match st.game_packet(&mut host, &p) {
            Verdict::Send => false,
            Verdict::Block => true,
            Verdict::Replace(np) => match build(&np).and_then(|s| send_now(game, s)) {
                Ok(()) => true,
                Err(e) => {
                    st.s.log(
                        super::hstudio::LogKind::Error,
                        format!(
                            "{} as the hook changed it: {e}; the game's was sent",
                            remote::remote_name(&p.name)
                        ),
                    );
                    false
                }
            },
        }
    })
    .unwrap_or(false)
}

/// handle_packet (every clientbound play packet): to its remote's
/// OnClientEvent, while a script listens.
pub(super) fn packet_in(game: &Game<'_>, packet: &C) {
    if !game.native_window() || !game.native_on(ID) {
        return;
    }
    super::hstudio_world_dump::heard(packet);
    let names = horizon_protocol::ids::play::clientbound::NAMES;
    let Some(name) = names.get(packet.id() as usize) else {
        return;
    };
    with(|st| {
        if st.packet_heard(name) {
            st.server_packet(&clientbound_packet(packet));
        }
    });
}
