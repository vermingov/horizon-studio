// The Roblox layer's world (hstudio_rbx_world): the Workspace as the
// Minecraft world. Players.LocalPlayer is the player playing, and its
// Character follows them each frame: an R6 rig as Roblox's, in blocks (a
// stud is a block), whose HumanoidRootPart is the torso's box, whose Head
// is at the eyes and whose Humanoid is the player's: their health, the
// game's speeds and how they move. Moving the root part teleports the player
// with /tp, and Humanoid:MoveTo walks them with the agent's goto.
// hstudio_rbx_draw's sync hands the player over to it every frame.
//
// Once a script looks at the world (the Workspace's children, its events,
// a raycast, the Mouse's target), the game's entities near the player are
// mirrored in the Workspace while it runs: mobs as Models with a Humanoid,
// dropped items and experience orbs as Parts. Each frame they follow the
// game, and they come and go with Roblox's events. A UI script that never
// looks costs the game nothing.

use super::hstudio::{CameraView, EntityInfo, GameApi, Owner, PlayerEntry, PlayerInfo, PlayerPose, Reply};
use super::hstudio_lua::{Lua, Res, Value, bad_arg, check_num, check_text};
use super::hstudio_rbx::{Env, Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_c3, as_cf, as_item, cf, enum_item, enum_value, v3};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};

/// A pixel of the player's model: the model is 15/16 the size of a
/// block-sized one, whose pixel is 1/16 of a block.
const PX: f64 = 0.9375 / 16.0;
/// The torso's centre above the feet of a player standing: the legs' 12
/// pixels and half the body's.
pub const TORSO_Y: f64 = 18.0 * PX;
/// The legs' centre above the feet: half their 12 pixels.
const LEG_Y: f64 = 6.0 * PX;
/// An R6 character's limbs, all 12 pixels tall and 4 deep: their names,
/// their widths, how far their centres are to the right of the torso's, and
/// whether they are legs (under the torso, down to the feet), in pixels.
const LIMBS: [(&str, f64, f64, bool); 5] = [
    ("Torso", 8.0, 0.0, false),
    ("Left Arm", 4.0, -6.0, false),
    ("Right Arm", 4.0, 6.0, false),
    ("Left Leg", 4.0, -1.9, true),
    ("Right Leg", 4.0, 1.9, true),
];
/// The eyes above the feet of a player standing.
pub const EYES_Y: f64 = 1.62;
/// The player's box, standing.
pub const HEIGHT: f64 = 1.8;
/// The width of the player's box.
const WIDTH: f64 = 0.6;
/// The most blocks Model:MoveTo raises the player's character, one at a
/// time, looking for room: the world's height.
const MAX_RAISE: usize = 384;
/// Health in the layer is the game's times this (the game's 20 is 100).
pub const HEALTH_SCALE: f64 = 5.0;
/// Blocks a second for a movement_speed of 1: the game's 0.1 walks 4.3178
/// blocks a second, sprinting's 0.13 runs 5.6131.
pub const SPEED_SCALE: f64 = 43.178;
/// Ticks a second: jump_strength is blocks a tick.
pub const TICKS: f64 = 20.0;
/// The game's jump_strength for a player and most mobs, in blocks a tick.
const JUMP: f64 = 0.42;
/// Moving slower than this, in blocks a second, a Humanoid isn't walking.
const WALKING: f64 = 0.5;
/// How long Humanoid:MoveTo walks before it gives up, in seconds (Roblox's).
pub const MOVE_TIMEOUT: f64 = 8.0;
/// The LocalPlayer's name until the game says the player's.
pub const NO_NAME: &str = "Player1";
/// The most entities mirrored at once: the other players' first, then the
/// nearest.
pub const MAX_MIRRORED: usize = 256;
/// A mirrored entity stays until it is farther down the game's list than
/// this, so that entities near the cut-off don't come and go as they move.
pub const KEEP_RANK: usize = MAX_MIRRORED + 32;
/// How long the console's look at the world lasts without another, in
/// seconds, once nothing of the console's listens on the layer.
pub const CONSOLE_LOOK: f64 = 60.0;
/// How often the server's player list is read, in seconds.
pub const PLAYERS_EVERY: f64 = 0.5;
/// The entity types the layer tells apart.
const PLAYER: &str = "minecraft:player";
const ORB: &str = "minecraft:experience_orb";

// ---------------------------------------------------------------- state

/// The world as the layer last saw it, between frames.
pub struct World {
    /// The root part's centre above the player's feet this frame.
    pub root_y: f64,
    /// The profile name and UUID the LocalPlayer was last given.
    pub name: String,
    pub uuid: u128,
    /// Where the last move of the root part this frame puts the player's
    /// feet: where the client moves them.
    pub teleport: Option<Vector3>,
    /// The walk and jump scales a script's WalkSpeed and JumpPower ask of
    /// the game, and whether the player passes through blocks now.
    pub speed_request: Option<(f64, f64)>,
    pub noclip: bool,
    /// What scripts asked of the game's own rendering and player, sent to
    /// the game each frame (Studio::roblox_effects).
    pub effects: Effects,
    /// What scripts made the local Humanoid and character do
    /// (hstudio_rbx_body): jumps, sitting, falling over, parts taken off.
    pub body: super::hstudio_rbx_body::Doings,
    /// The game's speeds the Humanoid was last given: a script's own
    /// WalkSpeed stays until the game's changes.
    pub speed: Option<f64>,
    pub jump: Option<f64>,
    /// The local Humanoid's state (a HumanoidStateType's name), whether the
    /// player stood on the ground, and the speed Running last said.
    pub state: &'static str,
    pub on_ground: bool,
    pub running: f64,
    /// Humanoid:MoveTo's walk, while it goes.
    pub walk: Option<Walk>,
    /// The local character's Torso and limbs (LIMBS' order).
    pub limbs: [Id; 5],
    /// The scripts that looked at the world: while one of them runs, the
    /// game's entities are mirrored. The console's look lapses (`lapse`).
    pub lookers: Vec<Owner>,
    /// When the console last looked.
    pub console_looked: f64,
    /// The entities mirrored, by the game's entity id.
    pub mirrors: BTreeMap<u32, Mirror>,
    /// The dimension the mirrored entities are in.
    pub dimension: String,
    /// The other players on the server, in the order they joined.
    pub others: Vec<Other>,
    /// When the player list is read next, and the local player's ping in
    /// milliseconds as it said (negative while unknown), and when it is read
    /// next for the ping alone.
    pub next_list: f64,
    pub ping: i32,
    pub next_ping: f64,
    /// The Workspace's parts as rays and queries meet them (hstudio_rbx_query).
    pub boxes: super::hstudio_rbx_query::Boxes,
    /// The frames the layer has followed the game, and when the Mouse was
    /// last worked out: the frame, whether the pointer was free and where,
    /// and the Mouse's TargetFilter.
    pub frames: u64,
    pub mouse_at: Option<(u64, bool, f64, f64, Option<Id>)>,
    /// The block the Mouse last pointed at, when its Target is the Terrain.
    pub mouse_block: Option<(i32, i32, i32)>,
    /// The player's facing at this frame's sync (degrees), and where their
    /// root part's centre was.
    pub yaw: f32,
    pub pitch: f32,
    pub root_at: Vector3,
    /// The Camera's CFrame as a script last set it, since the view was last
    /// worked out (`camera_frame`), and whether an action took the mouse's
    /// movement then; and the view the game was last given.
    pub camera_set: Option<(CFrame, bool)>,
    pub view: Option<CameraView>,
    /// The camera's distance as Roblox's zoom keeps it, once a script set
    /// the zoom's limits (CameraMinZoomDistance and CameraMaxZoomDistance),
    /// and whether they changed since; whether LockFirstPerson holds the
    /// view in first person.
    pub zoom: Option<f64>,
    pub zoom_changed: bool,
    pub first_locked: bool,
    /// What the game was last told: the keys scripts' actions take and
    /// whether one takes the mouse's movement; whether the character is
    /// anchored.
    pub sunk: (Vec<String>, bool),
    pub frozen: bool,
    /// What scripts took out of the client's world or moved in it, for the
    /// next frame: blocks (the Mouse's, as its Target the Terrain was
    /// destroyed) and where an entity's feet go (a mirror's root part moved).
    pub ghosts: Vec<(i32, i32, i32)>,
    pub entity_moves: Vec<(u32, [f64; 3])>,
}

impl Default for World {
    fn default() -> Self {
        World {
            root_y: TORSO_Y,
            name: NO_NAME.into(),
            uuid: 0,
            teleport: None,
            speed_request: None,
            noclip: false,
            effects: Effects::default(),
            body: Default::default(),
            speed: None,
            jump: None,
            state: "Running",
            on_ground: true,
            running: 0.0,
            walk: None,
            limbs: [0; 5],
            lookers: Vec::new(),
            console_looked: 0.0,
            mirrors: BTreeMap::new(),
            dimension: String::new(),
            others: Vec::new(),
            next_list: 0.0,
            ping: -1,
            next_ping: 0.0,
            boxes: Default::default(),
            frames: 0,
            mouse_at: None,
            mouse_block: None,
            yaw: 0.0,
            pitch: 0.0,
            root_at: Vector3::ZERO,
            camera_set: None,
            view: None,
            zoom: None,
            zoom_changed: false,
            first_locked: false,
            sunk: (Vec::new(), false),
            frozen: false,
            ghosts: Vec::new(),
            entity_moves: Vec::new(),
        }
    }
}

/// Another player on the server: their Player in Players, their entity
/// while the server sends it, their ping, the mirror that is their Character
/// while they are near, and whether that waits a frame (they just joined).
pub struct Other {
    pub uuid: u128,
    pub player: Id,
    pub entity: Option<u32>,
    pub latency: i32,
    pub character: Option<Id>,
    pub later: bool,
}

/// An entity's mirror: its Model (a mob's) or its Part (an item's, an
/// orb's), the parts and Humanoid in it, and what the game last said of it,
/// so that only a change of the game's is passed on.
pub struct Mirror {
    pub kind: String,
    pub model: Id,
    pub root: Id,
    pub head: Option<Id>,
    pub humanoid: Option<Id>,
    /// A player's Torso and limbs.
    pub limbs: Option<[Id; 5]>,
    pub name: String,
    pub health: f32,
    pub max_health: f32,
    pub speed: f64,
    pub state: &'static str,
    pub count: u32,
    /// The width, height and eyes its boxes were made for.
    pub shape: (f64, f64, f64),
    /// How far its root part's centre is over its feet.
    pub lift: f64,
}

/// A walk Humanoid:MoveTo started: its Humanoid, where to and when; whether
/// its goto went to the game, whether another walk's must stop first, and
/// the game's answer to come.
pub struct Walk {
    pub humanoid: Id,
    pub to: Vector3,
    pub since: f64,
    pub sent: bool,
    pub stop: bool,
    pub reply: Option<Receiver<String>>,
}

// ---------------------------------------------------------------- the character

/// A character's instances: its Torso and limbs in LIMBS' order.
#[derive(Clone, Copy, Debug)]
pub struct Rig {
    pub model: Id,
    pub root: Id,
    pub head: Id,
    pub humanoid: Id,
    pub limbs: [Id; 5],
}

/// A player's character, R6 in blocks: a Model named `name` whose
/// PrimaryPart, the HumanoidRootPart, is the torso's box, a Head (the head's
/// 8-pixel cube), a Humanoid, and the Torso, Left Arm, Right Arm, Left Leg
/// and Right Leg. Its parts are invisible (the game draws the player) and
/// touch nothing; as Roblox's, they aren't anchored (anchoring the local
/// one's holds the player still). Not in the Workspace yet.
pub fn build_character(rbx: &mut Rbx, name: &str) -> Rig {
    let model = rbx.make("Model", None);
    rbx.inst_mut(model).name = name.into();
    let root = rig_part(
        rbx,
        model,
        "HumanoidRootPart",
        Vector3::new(8.0 * PX, 12.0 * PX, 4.0 * PX),
    );
    let head = rig_part(rbx, model, "Head", Vector3::new(8.0 * PX, 8.0 * PX, 8.0 * PX));
    let humanoid = rbx.make("Humanoid", None);
    let root_v = rbx.value(root);
    rbx.raw_set(humanoid, "RootPart", root_v.clone());
    rbx.raw_set(humanoid, "DisplayName", Value::str(name));
    rbx.raw_set(humanoid, "RigType", enum_value(enum_item("HumanoidRigType", "R6")));
    // The root part's bottom off the ground: the legs.
    rbx.raw_set(humanoid, "HipHeight", Value::Num(12.0 * PX));
    // A player's jump, until the game says the local player's (`speeds`).
    rbx.raw_set(humanoid, "JumpPower", Value::Num(JUMP * TICKS));
    rbx.raw_set(humanoid, "JumpHeight", Value::Num(jump_height(JUMP)));
    let _ = rbx.set_parent(humanoid, Some(model));
    rbx.raw_set(model, "PrimaryPart", root_v);
    let limbs = LIMBS.map(|(limb, width, _, _)| rig_part(rbx, model, limb, Vector3::new(width, 12.0, 4.0) * PX));
    for part in [root, head].into_iter().chain(limbs) {
        rbx.raw_set(part, "Anchored", Value::Bool(false));
    }
    // Scripts may take the character apart, as in Roblox: what follows the
    // player is whatever the LocalPlayer's Character has (resolve_character).
    let animator = rbx.make("Animator", None);
    rbx.inst_mut(animator).name = "Animator".into();
    let _ = rbx.set_parent(animator, Some(humanoid));
    r6_details(rbx, model, root, head, &limbs);
    let description = rbx.make("HumanoidDescription", None);
    rbx.inst_mut(description).name = "HumanoidDescription".into();
    let _ = rbx.set_parent(description, Some(humanoid));
    rbx.add_animate(model);
    Rig {
        model,
        root,
        head,
        humanoid,
        limbs,
    }
}

/// A character's Torso and limbs in the rest pose, placed with its root part
/// (at `root`, `root_y` above the feet): the torso on it, the arms beside it
/// and the legs under it, standing on the feet however low the pose has the
/// torso. The game's animation of the limbs isn't followed.
fn place_limbs(rbx: &mut Rbx, limbs: &[Id; 5], root: CFrame, root_y: f64) {
    for (&id, &(_, _, right, leg)) in limbs.iter().zip(LIMBS.iter()) {
        let up = if leg { LEG_Y - root_y } else { 0.0 };
        rbx.quiet_set(id, "CFrame", cf(root * CFrame::new(right * PX, up, 0.0)));
    }
}

/// What an R6 character has besides its parts: the head's mesh and face,
/// the torso's decal, and the Motor6D joints that hold the parts together
/// (RootJoint in the root part, Neck, the shoulders and hips in the torso).
fn r6_details(rbx: &mut Rbx, model: Id, root: Id, head: Id, limbs: &[Id; 5]) {
    let _ = model;
    let make = |rbx: &mut Rbx, class: &str, name: &str, parent: Id, extra: &[(&str, Value)]| {
        let i = rbx.make(class, None);
        rbx.inst_mut(i).name = name.into();
        for (k, v) in extra {
            rbx.inst_mut(i).extra.insert((*k).into(), v.clone());
        }
        let _ = rbx.set_parent(i, Some(parent));
        i
    };
    let head_type = enum_value(enum_item("MeshType", "Head"));
    let scale = v3(Vector3::new(1.25, 1.25, 1.25));
    make(
        rbx,
        "SpecialMesh",
        "Mesh",
        head,
        &[("MeshType", head_type), ("Scale", scale)],
    );
    make(
        rbx,
        "Decal",
        "face",
        head,
        &[("Texture", Value::str("rbxasset://textures/face.png"))],
    );
    let torso = limbs[0];
    make(rbx, "Decal", "roblox", torso, &[("Texture", Value::str(""))]);
    let (rv, tv, hv) = (rbx.value(root), rbx.value(torso), rbx.value(head));
    make(
        rbx,
        "Motor6D",
        "RootJoint",
        root,
        &[("Part0", rv), ("Part1", tv.clone())],
    );
    make(rbx, "Motor6D", "Neck", torso, &[("Part0", tv.clone()), ("Part1", hv)]);
    for (name, limb) in [
        ("Left Shoulder", limbs[1]),
        ("Right Shoulder", limbs[2]),
        ("Left Hip", limbs[3]),
        ("Right Hip", limbs[4]),
    ] {
        let lv = rbx.value(limb);
        make(rbx, "Motor6D", name, torso, &[("Part0", tv.clone()), ("Part1", lv)]);
    }
}

fn rig_part(rbx: &mut Rbx, model: Id, name: &str, size: Vector3) -> Id {
    let part = invisible_part(rbx, name, size);
    let _ = rbx.set_parent(part, Some(model));
    part
}

/// A Part for something the game draws itself: invisible, anchored, and
/// touching nothing. Not in the Workspace yet.
fn invisible_part(rbx: &mut Rbx, name: &str, size: Vector3) -> Id {
    let part = rbx.make("Part", None);
    rbx.inst_mut(part).name = name.into();
    rbx.raw_set(part, "Size", v3(size));
    rbx.raw_set(part, "Transparency", Value::Num(1.0));
    rbx.raw_set(part, "Anchored", Value::Bool(true));
    rbx.raw_set(part, "CanCollide", Value::Bool(false));
    rbx.raw_set(part, "CanTouch", Value::Bool(false));
    rbx.raw_set(part, "Massless", Value::Bool(true));
    part
}

// ---------------------------------------------------------------- each frame

/// The world brought up to the game, each frame: the local player, and the
/// entities near them while a script looks.
pub fn sync(rbx: &mut Rbx, game: &mut dyn GameApi) {
    // A new frame: the parts the world moves quietly are elsewhere, and the
    // Mouse points anew.
    rbx.world.frames += 1;
    rbx.world.boxes.fresh = false;
    let player = game.player();
    if let Some(p) = &player {
        let pose = game.pose();
        sync_local(rbx, p, pose.as_ref());
    }
    sync_world(rbx, game, player.as_ref(), false);
}

/// The LocalPlayer is the player, and its character follows them: its root
/// part on the torso, its head at the eyes facing where they look (both as
/// low as the pose has them), its Humanoid's health, speeds, motion and
/// state. Coming back to life, they get a new character.
fn sync_local(rbx: &mut Rbx, p: &PlayerInfo, pose: Option<&PlayerPose>) {
    // Back to life in the game, or a character a script killed respawning.
    let revived = rbx.last_health.is_some_and(|h| h <= 0.0) && p.health > 0.0;
    if revived || rbx.respawn_at.is_some_and(|t| rbx.now >= t) {
        respawn(rbx);
    }
    resolve_character(rbx);
    ensure_camera(rbx);
    // A character whose Humanoid a script took out (or which it destroyed)
    // is dead, as Roblox's server has it: a new one comes RespawnTime later.
    let gone = rbx.character != rbx.spare_model && rbx.humanoid == rbx.spare_humanoid;
    if gone && rbx.respawn_at.is_none() {
        let wait = rbx.raw_get(rbx.players, "RespawnTime").to_num().unwrap_or(5.0);
        rbx.respawn_at = Some(rbx.now + wait);
    }
    identity(rbx, p);
    (rbx.world.yaw, rbx.world.pitch) = (p.yaw, p.pitch);
    let yaw = f64::from(p.yaw).to_radians();
    let pitch = f64::from(p.pitch).to_radians();
    let feet = Vector3::new(p.x, p.y, p.z);
    let flat = Vector3::new(-yaw.sin(), 0.0, yaw.cos());
    let look = Vector3::new(-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
    let (height, eyes) = body(pose);
    rbx.world.body.eyes = eyes;
    rbx.world.root_y = TORSO_Y * height / HEIGHT;
    let root = feet + Vector3::new(0.0, rbx.world.root_y, 0.0);
    let head = feet + Vector3::new(0.0, eyes, 0.0);
    let (rp, hd, hum) = (rbx.root_part, rbx.head, rbx.humanoid);
    // Kept quiet so a script's teleport isn't undone by the player's own place.
    let root_cf = CFrame::look_at(root, root + flat, Vector3::Y_AXIS);
    rbx.world.root_at = root;
    rbx.quiet_set(rp, "CFrame", cf(root_cf));
    rbx.fire(rp, "prop:CFrame", Vec::new());
    let limbs = rbx.world.limbs;
    place_limbs(rbx, &limbs, root_cf, rbx.world.root_y);
    rbx.quiet_set(hd, "CFrame", cf(CFrame::look_at(head, head + look, Vector3::Y_AXIS)));
    rbx.raw_set(hum, "MaxHealth", Value::Num(f64::from(p.max_health) * HEALTH_SCALE));
    let health = f64::from(p.health) * HEALTH_SCALE;
    // A character a script killed stays dead until it respawns.
    if rbx.respawn_at.is_none() && rbx.last_health.is_none_or(|h| h != p.health) {
        rbx.raw_set(hum, "Health", Value::Num(health));
        rbx.fire(hum, "HealthChanged", vec![Value::Num(health)]);
        if p.health <= 0.0 && rbx.last_health.is_some_and(|h| h > 0.0) {
            rbx.fire(hum, "Died", Vec::new());
        }
        rbx.last_health = Some(p.health);
    }
    speeds(rbx, p);
    let speed = motion(rbx, p);
    rbx.world.on_ground = pose.is_none_or(|q| q.on_ground);
    super::hstudio_rbx_body::frame(rbx);
    let state = if rbx.respawn_at.is_some() {
        "Dead"
    } else {
        let computed = next_state(rbx.world.state, p, pose);
        super::hstudio_rbx_body::state(rbx, computed)
    };
    set_state(rbx, state, speed);
}

/// The LocalPlayer takes the player's profile: their name for it, its
/// DisplayName, the character and the Humanoid's DisplayName, and a UserId
/// and a `UUID` attribute from their UUID. Until the game says, it is
/// Player1 with UserId 1.
fn identity(rbx: &mut Rbx, p: &PlayerInfo) {
    if !p.name.is_empty() && p.name != rbx.world.name {
        rbx.world.name = p.name.clone();
        let (player, model, humanoid) = (rbx.local_player, rbx.character, rbx.humanoid);
        rename(rbx, player, &p.name);
        rbx.raw_set(player, "DisplayName", Value::str(&p.name));
        rename(rbx, model, &p.name);
        rbx.raw_set(humanoid, "DisplayName", Value::str(&p.name));
    }
    if p.uuid != 0 && p.uuid != rbx.world.uuid {
        rbx.world.uuid = p.uuid;
        let player = rbx.local_player;
        rbx.raw_set(player, "UserId", Value::Num(user_id(p.uuid)));
        // Whose look the character has: the player's own, as in Roblox.
        rbx.raw_set(player, "CharacterAppearanceId", Value::Num(user_id(p.uuid)));
        set_attribute(rbx, player, "UUID", Value::str(&uuid_text(p.uuid)));
    }
}

/// The player's box height and eyes for a pose, as the game has them.
fn body(pose: Option<&PlayerPose>) -> (f64, f64) {
    match pose.map(|p| p.pose.as_str()) {
        Some("crouching") => (1.5, 1.27),
        Some("swimming" | "gliding" | "spin_attack") => (0.6, 0.4),
        Some("sleeping") => (0.2, 0.2),
        _ => (HEIGHT, EYES_Y),
    }
}

/// The Humanoid's WalkSpeed, JumpPower and JumpHeight from the game's
/// attributes, in blocks (sprinting is in the speed), set when the game's
/// value changes: a script's own value stays until then, and changes
/// nothing in the game.
fn speeds(rbx: &mut Rbx, p: &PlayerInfo) {
    let hum = rbx.humanoid;
    if let Some(speed) = p.speed
        && rbx.world.speed != Some(speed)
    {
        rbx.world.speed = Some(speed);
        rbx.raw_set(hum, "WalkSpeed", Value::Num(speed * SPEED_SCALE));
    }
    if let Some(jump) = p.jump
        && rbx.world.jump != Some(jump)
    {
        rbx.world.jump = Some(jump);
        rbx.raw_set(hum, "JumpPower", Value::Num(jump * TICKS));
        rbx.raw_set(hum, "JumpHeight", Value::Num(jump_height(jump)));
    }
}

/// How high a jump of `jump` blocks a tick goes, as the game moves the
/// player: up by the speed each tick, which gravity then slows by 0.08 and
/// the air by 2%. The game's 0.42 goes 1.2522 blocks.
pub fn jump_height(jump: f64) -> f64 {
    let (mut y, mut v) = (0.0, jump);
    for _ in 0..1000 {
        if v <= 0.0 {
            break;
        }
        y += v;
        v = (v - 0.08) * 0.98;
    }
    y
}

/// How the player moves, as the game has it: the root part's velocity (set
/// quietly, as physics sets it) and the Humanoid's MoveDirection, the way
/// they walk. Returns how fast they go along the ground.
fn motion(rbx: &mut Rbx, p: &PlayerInfo) -> f64 {
    let v = Vector3::new(p.velocity[0], p.velocity[1], p.velocity[2]);
    let rp = rbx.root_part;
    rbx.quiet_set(rp, "AssemblyLinearVelocity", v3(v));
    rbx.quiet_set(rp, "Velocity", v3(v));
    let flat = Vector3::new(v.x, 0.0, v.z);
    let speed = flat.magnitude();
    // The way the movement keys walk, as Roblox's MoveDirection is; else the way they go.
    let walking = if speed >= WALKING { flat / speed } else { Vector3::ZERO };
    let way = super::hstudio_rbx_body::input_direction(rbx, p.yaw).unwrap_or(walking);
    let hum = rbx.humanoid;
    rbx.raw_set(hum, "MoveDirection", v3(way));
    speed
}

/// The local Humanoid's state after `was`, from the player's health, pose
/// and motion: Dead, Swimming, Flying (creative flight, an elytra), Seated
/// (in a bed), Jumping while rising off the ground, Freefall, Landed for a
/// frame back on it, then Running.
fn next_state(was: &str, p: &PlayerInfo, pose: Option<&PlayerPose>) -> &'static str {
    if p.health <= 0.0 {
        return "Dead";
    }
    let Some(pose) = pose else { return "Running" };
    match pose.pose.as_str() {
        "swimming" => return "Swimming",
        "gliding" => return "Flying",
        "sleeping" => return "Seated",
        _ if pose.flying => return "Flying",
        _ => {}
    }
    if !pose.on_ground {
        if p.velocity[1] > 0.0 && was != "Freefall" {
            "Jumping"
        } else {
            "Freefall"
        }
    } else if matches!(was, "Jumping" | "Freefall") {
        "Landed"
    } else {
        "Running"
    }
}

/// Moves the local Humanoid to `state`, with StateChanged(old, new) and
/// Jumping(true) and (false) as it enters and leaves Jumping; and Running
/// with the speed along the ground as it changes while running, 0 when it
/// stops.
pub fn set_state(rbx: &mut Rbx, state: &'static str, speed: f64) {
    let hum = rbx.humanoid;
    let was = rbx.world.state;
    if was != state {
        rbx.world.state = state;
        let item = |s: &str| enum_value(enum_item("HumanoidStateType", s));
        rbx.fire(hum, "StateChanged", vec![item(was), item(state)]);
        if state == "Jumping" || was == "Jumping" {
            rbx.fire(hum, "Jumping", vec![Value::Bool(state == "Jumping")]);
        }
    }
    let running = if state == "Running" && speed >= WALKING {
        speed
    } else {
        0.0
    };
    if (running - rbx.world.running).abs() > 0.1 || (running == 0.0 && rbx.world.running != 0.0) {
        rbx.world.running = running;
        rbx.fire(hum, "Running", vec![Value::Num(running)]);
    }
}

/// The player came back to life: a new character takes the old one's place,
/// as in Roblox. CharacterRemoving fires with the old one, which goes; then
/// CharacterAdded and CharacterAppearanceLoaded with the new one, which the
/// Camera follows and which takes the game's values afresh.
fn respawn(rbx: &mut Rbx) {
    rbx.respawn_at = None;
    // A new character walks and jumps as the game does.
    rbx.world.speed_request = Some((1.0, 1.0));
    rbx.world.body.respawned();
    let player = rbx.local_player;
    // The character the LocalPlayer has goes (none when a script took it).
    let old = Some(rbx.character).filter(|&c| c != rbx.spare_model);
    if let Some(old) = old {
        let old_v = rbx.value(old);
        rbx.fire(player, "CharacterRemoving", vec![old_v]);
    }
    let name = rbx.inst(player).name.clone();
    let rig = build_character(rbx, &name);
    super::hstudio_rbx_body::dress(rbx, rig.model);
    rbx.character = rig.model;
    rbx.root_part = rig.root;
    rbx.head = rig.head;
    rbx.humanoid = rig.humanoid;
    rbx.world.limbs = rig.limbs;
    if let Some(old) = old {
        rbx.destroy(old);
    }
    let cv = rbx.value(rig.model);
    rbx.raw_set(player, "Character", cv.clone());
    let subject = rbx.value(rig.humanoid);
    let camera = rbx.camera;
    rbx.raw_set(camera, "CameraSubject", subject);
    let ws = rbx.workspace;
    let _ = rbx.set_parent(rig.model, Some(ws));
    rbx.world.speed = None;
    rbx.world.jump = None;
    rbx.world.state = "Running";
    rbx.world.running = 0.0;
    rbx.fire(player, "CharacterAdded", vec![cv.clone()]);
    rbx.fire(player, "CharacterAppearanceLoaded", vec![cv]);
}

/// The local character is whatever the LocalPlayer's Character is, as in
/// Roblox, where scripts swap it for a copy (an invisibility script), take
/// its Humanoid out (a god script) or delete its parts: its PrimaryPart or
/// HumanoidRootPart follows the player, its Humanoid has their health and
/// speeds, its limbs their pose. A piece it hasn't (or no character) is a
/// spare kept out of the place, which nothing sees.
pub fn resolve_character(rbx: &mut Rbx) {
    fn live(rbx: &Rbx, id: Id) -> bool {
        rbx.try_inst(id).is_some_and(|i| !i.destroyed)
    }
    let (spare, part, hum) = (rbx.spare_model, rbx.spare_part, rbx.spare_humanoid);
    let player = rbx.local_player;
    let named = rbx.id_of(&rbx.raw_get(player, "Character"));
    let model = named.filter(|&m| live(rbx, m));
    // A destroyed character is still the LocalPlayer's (it has no Humanoid).
    if let Some(dead) = named.filter(|&m| !live(rbx, m)) {
        rbx.character = dead;
        rbx.root_part = part;
        rbx.head = part;
        rbx.humanoid = hum;
        rbx.world.limbs = [part; 5];
        return;
    }
    let Some(model) = model else {
        rbx.character = spare;
        rbx.root_part = part;
        rbx.head = part;
        rbx.humanoid = hum;
        rbx.world.limbs = [part; 5];
        return;
    };
    let child = |rbx: &Rbx, name: &str| rbx.child_named(model, name).filter(|&c| live(rbx, c));
    let primary = rbx
        .id_of(&rbx.raw_get(model, "PrimaryPart"))
        .filter(|&p| live(rbx, p) && rbx.is_descendant_of(p, model));
    rbx.character = model;
    rbx.root_part = child(rbx, "HumanoidRootPart").or(primary).unwrap_or(part);
    rbx.head = child(rbx, "Head").unwrap_or(part);
    rbx.humanoid = rbx
        .inst(model)
        .children
        .iter()
        .copied()
        .find(|&c| live(rbx, c) && rbx.inst(c).class.name == "Humanoid")
        .unwrap_or(hum);
    rbx.world.limbs = LIMBS.map(|(limb, _, _, _)| child(rbx, limb).unwrap_or(part));
}

/// The Workspace always has a CurrentCamera: a script that removes it (or
/// sets it to nil) gets a new one, as in Roblox.
fn ensure_camera(rbx: &mut Rbx) {
    let (cam, ws) = (rbx.camera, rbx.workspace);
    let current = rbx.id_of(&rbx.raw_get(ws, "CurrentCamera"));
    let fine = |id: Id| rbx.try_inst(id).is_some_and(|i| !i.destroyed && i.parent.is_some());
    match current {
        Some(c) if fine(c) => {
            rbx.camera = c;
            return;
        }
        _ if fine(cam) && rbx.inst(cam).parent == Some(ws) => {
            let v = rbx.value(cam);
            rbx.raw_set(ws, "CurrentCamera", v);
            return;
        }
        _ => {}
    }
    let c = rbx.make("Camera", None);
    let _ = rbx.set_parent(c, Some(ws));
    rbx.camera = c;
    let v = rbx.value(c);
    rbx.raw_set(ws, "CurrentCamera", v);
    let subject = rbx.value(rbx.humanoid);
    rbx.raw_set(c, "CameraSubject", subject);
}

/// A script set the local Humanoid's health: at none, it dies as in
/// Roblox (Died, the Dead state) and the LocalPlayer gets a new character
/// Players.RespawnTime later.
pub fn health_set(rbx: &mut Rbx) {
    let hum = rbx.humanoid;
    let health = rbx.raw_get(hum, "Health").to_num().unwrap_or(0.0);
    if health > 0.0 || rbx.respawn_at.is_some() || rbx.world.state == "Dead" {
        return;
    }
    let wait = rbx.raw_get(rbx.players, "RespawnTime").to_num().unwrap_or(5.0);
    rbx.respawn_at = Some(rbx.now + wait);
    set_state(rbx, "Dead", 0.0);
    rbx.fire(hum, "Died", Vec::new());
}

// ---------------------------------------------------------------- effects on the game

/// The game's own lighting, sky, camera and player as Roblox scripts set
/// them. Each request is taken by the frame that sends it to the game.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Effects {
    /// How bright the world is lit regardless of its light, 0 (the game's
    /// own) to 1 (fullbright, as night vision lights it).
    pub light: Option<f32>,
    /// The time of day the client shows (Minecraft's ticks), or None: the
    /// server's.
    pub time: Option<Option<i64>>,
    /// The fog's far end, in blocks (None: the game's).
    pub fog: Option<Option<f32>>,
    /// The player's gravity, as a multiple of the game's.
    pub gravity: Option<f64>,
    /// The field of view, in degrees (None: the player's setting).
    pub fov: Option<Option<f32>>,
    /// Where the player looks (yaw, pitch in degrees), set outright.
    pub look: Option<(f32, f32)>,
    /// How far to turn the player's view (yaw, pitch in degrees).
    pub turn: (f32, f32),
    /// First person (true) or third (LocalPlayer.CameraMode, the zoom).
    pub perspective: Option<bool>,
    /// The third person view's distance in blocks (None: the game's).
    pub distance: Option<Option<f64>>,
    /// The mouse's sensitivity (UserInputService.MouseDeltaSensitivity).
    pub sensitivity: Option<f64>,
    /// The entities' hitboxes shown (RenderSettings.ShowBoundingBoxes).
    pub hitboxes: Option<bool>,
}

/// Roblox's default Workspace.Gravity, in studs per second squared.
pub const ROBLOX_GRAVITY: f64 = 196.2;

/// How brightly a Lighting lights the world past its own light (Effects::
/// light): none while it has Roblox's shadows and default ambient light;
/// full for the fullbright scripts' settings (GlobalShadows off with a
/// light OutdoorAmbient, a light Ambient, or Brightness raised).
pub fn lighting_strength(rbx: &Rbx) -> f32 {
    let Some(l) = rbx.service("Lighting") else { return 0.0 };
    let lum = |key: &str| as_c3(&rbx.raw_get(l, key)).map_or(0.0, |c| (0.299 * c.r + 0.587 * c.g + 0.114 * c.b) as f32);
    let shadows = rbx.raw_get(l, "GlobalShadows").truthy();
    let brightness = rbx.raw_get(l, "Brightness").to_num().unwrap_or(2.0) as f32;
    let exposure = rbx.raw_get(l, "ExposureCompensation").to_num().unwrap_or(0.0) as f32;
    // Roblox's own defaults (70, 70, 70 ambient) light nothing extra.
    let ambient = ((lum("Ambient") - 0.3) / 0.4).clamp(0.0, 1.0);
    let outdoor = if shadows {
        0.0
    } else {
        (lum("OutdoorAmbient") / 0.5).clamp(0.0, 1.0)
    };
    let bright = ((brightness - 2.0) / 3.0).clamp(0.0, 1.0);
    let exposed = (exposure / 2.0).clamp(0.0, 1.0);
    ambient.max(outdoor).max(bright).max(exposed)
}

/// Roblox's ClockTime (hours) as Minecraft's time of day (ticks; 0 is 6:00).
pub fn clock_ticks(hours: f64) -> i64 {
    (((hours - 6.0) * 1000.0).rem_euclid(24000.0)) as i64
}

/// A script set one of the properties that reach the game itself: the
/// request for the next frame.
pub fn effect_set(rbx: &mut Rbx, id: Id, prop: &str) {
    let class = rbx.inst(id).class.name;
    match (class, prop) {
        ("Lighting", "Brightness" | "Ambient" | "OutdoorAmbient" | "GlobalShadows" | "ExposureCompensation") => {
            rbx.world.effects.light = Some(lighting_strength(rbx));
        }
        ("Lighting", "ClockTime") => {
            let h = rbx.raw_get(id, "ClockTime").to_num().unwrap_or(14.0);
            // A time the script read from the world gives the time back to it.
            let own = super::hstudio_rbx_client::clock_restored(rbx, h);
            rbx.world.effects.time = Some((!own).then(|| clock_ticks(h)));
        }
        ("Lighting", "TimeOfDay") => {
            let text = rbx.raw_get(id, "TimeOfDay").as_text().unwrap_or_default();
            let mut parts = text.split(':').map(|p| p.trim().parse::<f64>().unwrap_or(0.0));
            let h = parts.next().unwrap_or(14.0)
                + parts.next().unwrap_or(0.0) / 60.0
                + parts.next().unwrap_or(0.0) / 3600.0;
            rbx.world.effects.time = Some(Some(clock_ticks(h)));
        }
        ("Lighting", "FogEnd") => {
            let end = rbx.raw_get(id, "FogEnd").to_num().unwrap_or(100000.0);
            // Roblox's default (100000) is no fog at all: the game's own.
            rbx.world.effects.fog = Some((end < 10000.0).then_some(end.max(0.0) as f32));
        }
        ("Workspace", "Gravity") => {
            let g = rbx.raw_get(id, "Gravity").to_num().unwrap_or(ROBLOX_GRAVITY);
            rbx.world.effects.gravity = Some((g / ROBLOX_GRAVITY).clamp(0.0, 10.0));
        }
        ("Camera", "FieldOfView") if id == rbx.camera => {
            let fov = rbx.raw_get(id, "FieldOfView").to_num().unwrap_or(70.0);
            rbx.world.effects.fov = Some(Some(fov.clamp(1.0, 120.0) as f32));
        }
        // Where the view goes is worked out once a frame (`camera_frame`).
        ("Camera", "CFrame") if id == rbx.camera => {
            let captured = rbx.world.sunk.1;
            rbx.world.camera_set = as_cf(&rbx.raw_get(id, "CFrame")).map(|c| (c, captured));
        }
        ("Player", "CameraMode") if id == rbx.local_player => {
            let mode = as_item(&rbx.raw_get(id, "CameraMode")).map_or("Classic", |m| m.name);
            let first = mode == "LockFirstPerson";
            rbx.world.first_locked = first;
            rbx.world.effects.perspective = Some(first);
            rbx.world.zoom = Some(if first { MIN_ZOOM } else { GAME_DISTANCE });
        }
        ("Player", "CameraMaxZoomDistance" | "CameraMinZoomDistance") if id == rbx.local_player => {
            rbx.world.zoom_changed = true;
        }
        ("RenderSettings", "ShowBoundingBoxes") => {
            rbx.world.effects.hitboxes = Some(rbx.raw_get(id, "ShowBoundingBoxes").truthy());
        }
        ("UserInputService", "MouseDeltaSensitivity") => {
            let k = rbx.raw_get(id, "MouseDeltaSensitivity").to_num().unwrap_or(1.0);
            rbx.world.effects.sensitivity = Some(k.clamp(0.0, 10.0));
        }
        _ => {}
    }
}

/// Minecraft's yaw and pitch for a way to look (yaw 0 faces +Z, south; 90
/// faces -X; a positive pitch looks down).
pub fn yaw_pitch(look: Vector3) -> (f32, f32) {
    let yaw = (-look.x).atan2(look.z).to_degrees() as f32;
    let pitch = (-look.y).clamp(-1.0, 1.0).asin().to_degrees() as f32 + 0.0;
    (yaw, pitch)
}

// ---------------------------------------------------------------- the view and the input

/// The zoom (Roblox's camera distance, in blocks) under which the view is
/// first person, as Roblox's camera has it.
const FIRST_PERSON_ZOOM: f64 = 1.0;
/// Roblox's least zoom: CameraMinZoomDistance's default, first person.
const MIN_ZOOM: f64 = 0.5;
/// The game's own third person distance, in blocks.
const GAME_DISTANCE: f64 = 4.0;
/// How near where the view is a script may set the Camera's CFrame for it
/// to turn the player rather than move the view, in blocks.
const SAME_PLACE: f64 = 0.25;

/// What the layer asks of the game's view, input and world each frame,
/// before what Effects holds is sent: the perspective and the third person
/// distance (`zoom`), the keys and mouse movement scripts' actions take
/// (`hstudio_rbx_draw::sunk_input`), where the view is (`camera_frame`),
/// whether the anchored character holds still, and the blocks and entities
/// scripts took out of the client's world or moved in it.
pub fn game_effects(rbx: &mut Rbx, game: &mut dyn GameApi) {
    zoom(rbx, game);
    let sunk = super::hstudio_rbx_draw::sunk_input(rbx);
    if sunk != rbx.world.sunk {
        game.sink_input(&sunk.0, sunk.1);
        rbx.world.sunk = sunk;
    }
    camera_frame(rbx, game);
    let frozen = anchored(rbx);
    if frozen != rbx.world.frozen && game.set_player_frozen(frozen) {
        rbx.world.frozen = frozen;
    }
    for (x, y, z) in std::mem::take(&mut rbx.world.ghosts) {
        game.set_client_block(x, y, z, "minecraft:air");
    }
    for (id, feet) in std::mem::take(&mut rbx.world.entity_moves) {
        game.move_entity(id, feet);
    }
    let fx = &mut rbx.world.effects;
    if let Some(first) = fx.perspective.take() {
        game.set_perspective(first);
    }
    if let Some(d) = fx.distance.take() {
        game.set_camera_distance(d);
    }
    if let Some(k) = fx.sensitivity.take() {
        game.set_mouse_sensitivity(k);
    }
    if let Some(on) = fx.hitboxes.take() {
        game.set_hitboxes(on);
    }
}

/// Roblox's zoom as the game's perspective. LockFirstPerson holds the view
/// in first person. Once a script sets the zoom's limits (the LocalPlayer's
/// CameraMinZoomDistance and CameraMaxZoomDistance), the zoom is kept
/// within them, from where the view is (first person is Roblox's least
/// zoom, third the game's distance): under a block, the view is first
/// person; else third person, that far behind. Limits that leave the zoom
/// where it is change nothing (as after camdistance's, the zoom stays).
fn zoom(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if rbx.world.first_locked && game.body_shown() && rbx.world.effects.perspective.is_none() {
        game.set_perspective(true);
    }
    if !std::mem::take(&mut rbx.world.zoom_changed) {
        return;
    }
    let p = rbx.local_player;
    let max = rbx.raw_get(p, "CameraMaxZoomDistance").to_num().unwrap_or(128.0);
    let min = rbx
        .raw_get(p, "CameraMinZoomDistance")
        .to_num()
        .unwrap_or(MIN_ZOOM)
        .min(max);
    let shown = game.body_shown();
    let was = rbx.world.zoom.unwrap_or(if shown { GAME_DISTANCE } else { MIN_ZOOM });
    let z = was.clamp(min, max.max(min));
    if (z - was).abs() < 1e-9 {
        return;
    }
    rbx.world.zoom = Some(z);
    let first = z < FIRST_PERSON_ZOOM || rbx.world.first_locked;
    if first != !shown {
        rbx.world.effects.perspective = Some(first);
    }
    if !first {
        rbx.world.effects.distance = Some(Some(z));
    }
}

/// Where the view is this frame, as Roblox's camera puts it. A Scriptable
/// (or Fixed) Camera stays where scripts put it: its CFrame, place and
/// rotation, is the view, wherever the player goes. Another Camera follows
/// its CameraSubject (`subject_view`), but a CFrame a script set since the
/// last frame (a render step's) is the view for this frame: moved away from
/// where the view was, or while a script's action takes the mouse's
/// movement (a free camera's), the view goes there. Unless an action takes
/// the mouse, the player turns to look that way too, as Roblox's camera
/// keeps its new rotation. The game is told only of a change.
pub fn camera_frame(rbx: &mut Rbx, game: &mut dyn GameApi) {
    let cam = rbx.camera;
    let kind = as_item(&rbx.raw_get(cam, "CameraType")).map_or("Custom", |k| k.name);
    let set = rbx.world.camera_set.take();
    let view = if matches!(kind, "Scriptable" | "Fixed") {
        let frame = set
            .map(|s| s.0)
            .or_else(|| as_cf(&rbx.raw_get(cam, "CFrame")))
            .unwrap_or_default();
        Some(view_at(frame))
    } else if let Some((frame, then)) = set {
        let captured = then || rbx.world.sunk.1;
        let here = game.camera().map(|c| Vector3::new(c.pos[0], c.pos[1], c.pos[2]));
        let near = here.is_some_and(|h| (frame.position() - h).magnitude() <= SAME_PLACE);
        if !captured {
            rbx.world.effects.look = Some(yaw_pitch(frame.look_vector()));
        }
        if captured || !near {
            Some(view_at(frame))
        } else {
            subject_view(rbx)
        }
    } else {
        subject_view(rbx)
    };
    if view != rbx.world.view {
        game.set_camera_view(view);
        rbx.world.view = view;
    }
}

/// The view at a CFrame: its place, looking along its LookVector.
fn view_at(frame: CFrame) -> CameraView {
    let p = frame.position();
    let (yaw, pitch) = yaw_pitch(frame.look_vector());
    CameraView::At {
        pos: [p.x, p.y, p.z],
        yaw,
        pitch,
    }
}

/// Where the Camera's CameraSubject puts the view: the local character (or
/// a part of it) or none, the game's own; another entity's mirror (its
/// Humanoid, its model, a part of it), that entity's view; any other
/// part, model or Humanoid, around its centre (a Humanoid's, its root
/// part's).
fn subject_view(rbx: &Rbx) -> Option<CameraView> {
    let s = rbx.id_of(&rbx.raw_get(rbx.camera, "CameraSubject"))?;
    let inst = rbx.try_inst(s).filter(|i| !i.destroyed)?;
    let ch = rbx.character;
    if s == ch || rbx.is_descendant_of(s, ch) || s == rbx.humanoid {
        return None;
    }
    if let Some((&id, _)) = rbx
        .world
        .mirrors
        .iter()
        .find(|(_, m)| s == m.model || rbx.is_descendant_of(s, m.model))
    {
        return Some(CameraView::Entity(id));
    }
    let target = if inst.class.name == "Humanoid" {
        rbx.id_of(&rbx.raw_get(s, "RootPart")).or(inst.parent)?
    } else {
        s
    };
    let t = rbx.try_inst(target)?;
    // Out of the Workspace (a character parked in Lighting), nothing to see.
    if !super::hstudio_rbx::is_a(t.class, "PVInstance") || !rbx.is_descendant_of(target, rbx.workspace) {
        return None;
    }
    let at = super::hstudio_rbx_methods::pivot(rbx, target).position();
    Some(CameraView::Around([at.x, at.y, at.z]))
}

/// A script's Destroy (or Remove) of the Terrain as the Mouse's Target:
/// the block the Mouse points at goes from the client's world, as Roblox's
/// client deletes a part for itself alone (a ghost block: the server's next
/// word on it puts it back). Whether there was one.
pub fn local_delete(rbx: &mut Rbx, id: Id) -> bool {
    let terrain = rbx.id_of(&rbx.raw_get(rbx.workspace, "Terrain"));
    if Some(id) != terrain {
        return false;
    }
    match rbx.world.mouse_block.take() {
        Some(b) => {
            rbx.world.ghosts.push(b);
            true
        }
        None => false,
    }
}

/// Whether a part of the local character is anchored (anchoring one
/// anchors the whole character in Roblox, its parts joined).
fn anchored(rbx: &Rbx) -> bool {
    let ch = rbx.character;
    ch != rbx.spare_model
        && rbx.descendants(ch).into_iter().any(|d| {
            rbx.try_inst(d)
                .is_some_and(|i| super::hstudio_rbx::is_a(i.class, "BasePart") && i.get("Anchored").truthy())
        })
}

// ---------------------------------------------------------------- the world's entities

/// A script looked at the world: while it runs, the game's entities near the
/// player are mirrored in the Workspace. The first look mirrors them at once,
/// so the script finds them there already, and its ChildAdded handlers fire
/// for those that come after.
pub fn look(env: &mut dyn Env) {
    let (owner, now) = (env.owner(), env.now());
    let (rbx, game) = env.rbx_and_game();
    if owner == Owner::Console {
        rbx.world.console_looked = now;
    }
    if rbx.world.lookers.contains(&owner) {
        return;
    }
    if !rbx.active {
        // The layer's first use (an executor function's can be): it wakes up
        // with the game as it is.
        rbx.active = true;
        rbx.now = now;
        super::hstudio_rbx_draw::sync(rbx, game);
    }
    let first = rbx.world.lookers.is_empty();
    rbx.world.lookers.push(owner);
    if first {
        let player = game.player();
        sync_world(rbx, game, player.as_ref(), true);
        // The Mouse may point at an entity that is there now.
        rbx.world.mouse_at = None;
    }
}

/// A script stopped: it looks no more.
pub fn forget(rbx: &mut Rbx, owner: Owner) {
    rbx.world.lookers.retain(|o| *o != owner);
}

/// The console's look lapses a minute after its last, once none of its
/// handlers, waiting threads or render steps is left on the layer: a line
/// typed to see the world doesn't keep it mirrored for the whole session.
fn lapse(rbx: &mut Rbx) {
    let console = Owner::Console;
    if !rbx.world.lookers.contains(&console) || rbx.now - rbx.world.console_looked < CONSOLE_LOOK {
        return;
    }
    let listens = rbx.insts.values().any(|i| i.conns.iter().any(|c| c.owner == console))
        || rbx.waiters.iter().any(|w| w.owner == console)
        || rbx.render_steps.iter().any(|r| r.2 == console);
    if listens {
        rbx.world.console_looked = rbx.now;
    } else {
        forget(rbx, console);
    }
}

/// The mirrors and the other players brought up to the game while a script
/// looks (`looking`: as it first looks). Out of a world (a disconnect too)
/// there are none, in another dimension the entities are others (the game
/// gives their ids out again), and once no script looks they all go.
fn sync_world(rbx: &mut Rbx, game: &mut dyn GameApi, p: Option<&PlayerInfo>, looking: bool) {
    lapse(rbx);
    let Some(p) = p.filter(|_| !rbx.world.lookers.is_empty()) else {
        clear(rbx, true);
        return;
    };
    if p.dimension != rbx.world.dimension {
        clear(rbx, false);
        rbx.world.dimension = p.dimension.clone();
    }
    sync_players(rbx, game, looking);
    sync_entities(rbx, game);
    link_characters(rbx);
}

/// Every mirror goes, in the order of their entities' ids; and with
/// `players`, every other player, in the order they joined.
fn clear(rbx: &mut Rbx, players: bool) {
    let ids: Vec<u32> = rbx.world.mirrors.keys().copied().collect();
    for id in ids {
        remove_mirror(rbx, id);
    }
    if players {
        let gone: Vec<u128> = rbx.world.others.iter().map(|o| o.uuid).collect();
        for uuid in gone {
            remove_player(rbx, uuid);
        }
        rbx.world.next_list = 0.0;
    }
}

/// The entities as the game lists them now: a mirror for each one chosen
/// (`choose`), made for those new, brought up to the game for the rest, and
/// gone for those no longer chosen. An id that comes back as another type of
/// entity gets a new mirror.
fn sync_entities(rbx: &mut Rbx, game: &mut dyn GameApi) {
    // A mirror something else destroyed (nothing should: they are locked) is
    // let go as it is, and its entity gets a new one.
    let insts = &rbx.insts;
    rbx.world
        .mirrors
        .retain(|_, m| insts.get(&m.model).is_some_and(|i| !i.destroyed));
    let list = game.entities(KEEP_RANK);
    let chosen = choose(&rbx.world.mirrors, &list);
    let keep: BTreeSet<u32> = chosen.iter().map(|e| e.id).collect();
    let gone: Vec<u32> = rbx
        .world
        .mirrors
        .keys()
        .copied()
        .filter(|id| !keep.contains(id))
        .collect();
    for id in gone {
        remove_mirror(rbx, id);
    }
    for e in chosen {
        match rbx.world.mirrors.get(&e.id) {
            Some(m) if m.kind == e.kind => update_mirror(rbx, e, false),
            Some(_) => {
                remove_mirror(rbx, e.id);
                build_mirror(rbx, e);
            }
            None => build_mirror(rbx, e),
        }
    }
}

/// Which of the game's entities (the other players first, then the nearest)
/// to mirror, in its order: those mirrored already while the list has them
/// (it runs to KEEP_RANK), then new ones while fewer than MAX_MIRRORED are.
/// Another player's entity always finds room, the farthest mob's mirror
/// making way. So entities about the cut-off keep their mirrors as they
/// move, rather than coming and going every frame.
fn choose<'a>(mirrors: &BTreeMap<u32, Mirror>, list: &'a [EntityInfo]) -> Vec<&'a EntityInfo> {
    let mut seen = BTreeSet::new();
    let list: Vec<&EntityInfo> = list.iter().filter(|e| seen.insert(e.id)).collect();
    let mut chosen: Vec<usize> = (0..list.len()).filter(|&i| mirrors.contains_key(&list[i].id)).collect();
    for i in (0..list.len()).filter(|&i| !mirrors.contains_key(&list[i].id)) {
        if chosen.len() < MAX_MIRRORED {
            chosen.push(i);
        } else if list[i].kind == PLAYER
            && let Some(at) = chosen.iter().rposition(|&c| list[c].kind != PLAYER)
        {
            chosen.remove(at);
            chosen.push(i);
        }
    }
    chosen.sort_unstable();
    chosen.into_iter().map(|i| list[i]).collect()
}

/// A new entity's mirror, made whole and put in place before it goes into
/// the Workspace, so ChildAdded's handlers see it so. A mob is a Model named
/// after it (its custom name, else its type: "Cave Spider") holding a
/// HumanoidRootPart, a Head and a Humanoid; a dropped item is a Part named
/// after the item with ItemId and Count attributes, and an orb a Part
/// "Experience Orb" with its Value. Each has EntityId and EntityType
/// attributes. Like the character, a mirror is the studio's own and locked:
/// scripts can put things in it, but not take it out of the Workspace (its
/// Destroy takes the entity out of the client's view: hstudio_rbx_tools).
fn build_mirror(rbx: &mut Rbx, e: &EntityInfo) {
    let name = mirror_name(rbx, e);
    let mut limbs = None;
    let (model, root, head, humanoid) = match &e.item {
        Some((item, n)) => {
            let part = invisible_part(rbx, &name, Vector3::ONE);
            if e.kind == ORB {
                set_attribute(rbx, part, "Value", Value::Num(f64::from(*n)));
            } else {
                set_attribute(rbx, part, "ItemId", Value::str(item));
                set_attribute(rbx, part, "Count", Value::Num(f64::from(*n)));
            }
            (part, part, None, None)
        }
        None if e.kind == PLAYER => {
            let rig = build_character(rbx, &name);
            limbs = Some(rig.limbs);
            (rig.model, rig.root, Some(rig.head), Some(rig.humanoid))
        }
        None => {
            let model = rbx.make("Model", None);
            rbx.inst_mut(model).name = name.clone();
            let root = rig_part(rbx, model, "HumanoidRootPart", Vector3::ONE);
            let head = rig_part(rbx, model, "Head", Vector3::ONE);
            let humanoid = rbx.make("Humanoid", None);
            let root_v = rbx.value(root);
            rbx.raw_set(humanoid, "RootPart", root_v.clone());
            rbx.raw_set(humanoid, "DisplayName", Value::str(&name));
            rbx.raw_set(humanoid, "RigType", enum_value(enum_item("HumanoidRigType", "R6")));
            // The root part stands on the ground: no legs under it.
            rbx.raw_set(humanoid, "HipHeight", Value::Num(0.0));
            rbx.raw_set(humanoid, "JumpPower", Value::Num(JUMP * TICKS));
            rbx.raw_set(humanoid, "JumpHeight", Value::Num(jump_height(JUMP)));
            let _ = rbx.set_parent(humanoid, Some(model));
            rbx.raw_set(model, "PrimaryPart", root_v);
            set_attribute(rbx, model, "Baby", Value::Bool(e.baby));
            (model, root, Some(head), Some(humanoid))
        }
    };
    set_attribute(rbx, model, "EntityId", Value::Num(f64::from(e.id)));
    set_attribute(rbx, model, "EntityType", Value::str(&e.kind));
    let mut tree = vec![model];
    tree.extend(rbx.descendants(model));
    for id in tree {
        rbx.inst_mut(id).locked = true;
    }
    rbx.world.mirrors.insert(
        e.id,
        Mirror {
            kind: e.kind.clone(),
            model,
            root,
            head,
            humanoid,
            limbs,
            name,
            health: e.health,
            max_health: e.max_health,
            speed: e.speed,
            state: entity_state(e),
            count: e.item.as_ref().map_or(0, |i| i.1),
            shape: (0.0, 0.0, 0.0),
            lift: 0.0,
        },
    );
    update_mirror(rbx, e, true);
    let ws = rbx.workspace;
    let _ = rbx.set_parent(model, Some(ws));
}

/// A mirror brought up to its entity: its parts where the game draws it
/// (set quietly, as Roblox's physics moves parts, with no Changed) and its
/// velocity; and as the game's own change, its name, its Humanoid's health
/// (HealthChanged, and Died once as it reaches nothing), most health, speed
/// and state (StateChanged), and an item's count or an orb's value. A
/// script's own changes to them stay until the game's change.
fn update_mirror(rbx: &mut Rbx, e: &EntityInfo, fresh: bool) {
    let Some(m) = rbx.world.mirrors.get(&e.id) else {
        return;
    };
    let (model, root, head, humanoid, limbs) = (m.model, m.root, m.head, m.humanoid, m.limbs);
    let (name, health, max_health, speed, state, count, shape) = (
        m.name.clone(),
        m.health,
        m.max_health,
        m.speed,
        m.state,
        m.count,
        m.shape,
    );
    place(rbx, e, root, head, shape != (e.width, e.height, e.eye));
    if let Some(limbs) = limbs {
        let at = as_cf(&rbx.raw_get(root, "CFrame")).unwrap_or_default();
        place_limbs(rbx, &limbs, at, TORSO_Y * e.height / HEIGHT);
    }
    let v = v3(Vector3::new(e.velocity[0], e.velocity[1], e.velocity[2]));
    rbx.quiet_set(root, "AssemblyLinearVelocity", v.clone());
    rbx.quiet_set(root, "Velocity", v);
    let now_name = mirror_name(rbx, e);
    if !fresh && now_name != name {
        rename(rbx, model, &now_name);
        if let Some(h) = humanoid {
            rbx.raw_set(h, "DisplayName", Value::str(&now_name));
        }
    }
    let now_state = entity_state(e);
    if let Some(h) = humanoid {
        if fresh || e.max_health != max_health {
            rbx.raw_set(h, "MaxHealth", Value::Num(f64::from(e.max_health) * HEALTH_SCALE));
        }
        let hp = f64::from(e.health) * HEALTH_SCALE;
        if fresh {
            rbx.raw_set(h, "Health", Value::Num(hp));
        } else if e.health != health {
            rbx.raw_set(h, "Health", Value::Num(hp));
            rbx.fire(h, "HealthChanged", vec![Value::Num(hp)]);
            if e.health <= 0.0 && health > 0.0 {
                rbx.fire(h, "Died", Vec::new());
            }
        }
        if fresh || e.speed != speed {
            rbx.raw_set(h, "WalkSpeed", Value::Num(e.speed * SPEED_SCALE));
        }
        if !fresh && now_state != state {
            let item = |s: &str| enum_value(enum_item("HumanoidStateType", s));
            rbx.fire(h, "StateChanged", vec![item(state), item(now_state)]);
        }
    }
    if let Some((_, n)) = &e.item
        && *n != count
    {
        let key = if e.kind == ORB { "Value" } else { "Count" };
        set_attribute(rbx, model, key, Value::Num(f64::from(*n)));
    }
    if let Some(m) = rbx.world.mirrors.get_mut(&e.id) {
        m.name = now_name;
        m.health = e.health;
        m.max_health = e.max_health;
        m.speed = e.speed;
        m.state = now_state;
        m.count = e.item.as_ref().map_or(0, |i| i.1);
        m.shape = (e.width, e.height, e.eye);
    }
}

/// Where a mirror's parts are this frame, from its entity's feet, box and
/// facing: another player's character as the local one is placed; a mob's
/// root part from its feet up to its head, a cube at its eyes; an item's or
/// an orb's part over its box. Their sizes are set when the box changes.
fn place(rbx: &mut Rbx, e: &EntityInfo, root: Id, head: Option<Id>, resize: bool) {
    let feet = Vector3::new(e.pos[0], e.pos[1], e.pos[2]);
    let body = facing(e.body_yaw, 0.0);
    let look = facing(e.head_yaw, e.pitch);
    let up = |y: f64| feet + Vector3::new(0.0, y, 0.0);
    let (root_at, head_at) = match head {
        _ if e.kind == PLAYER => (up(TORSO_Y * e.height / HEIGHT), up(e.eye)),
        Some(head) => {
            let (r, s) = mob_boxes(e.width, e.height, e.eye);
            if resize {
                rbx.raw_set(root, "Size", v3(Vector3::new(e.width, r, e.width)));
                rbx.raw_set(head, "Size", v3(Vector3::new(s, s, s)));
            }
            (up(r / 2.0), up(r + s / 2.0))
        }
        None => {
            if resize {
                rbx.raw_set(root, "Size", v3(Vector3::new(e.width, e.height, e.width)));
            }
            (up(e.height / 2.0), feet)
        }
    };
    rbx.quiet_set(
        root,
        "CFrame",
        cf(CFrame::look_at(root_at, root_at + body, Vector3::Y_AXIS)),
    );
    if let Some(m) = rbx.world.mirrors.get_mut(&e.id) {
        m.lift = root_at.y - feet.y;
    }
    if let Some(head) = head {
        rbx.quiet_set(
            head,
            "CFrame",
            cf(CFrame::look_at(head_at, head_at + look, Vector3::Y_AXIS)),
        );
    }
}

/// An entity that went: its mirror is destroyed (Destroying, then
/// DescendantRemoving and ChildRemoved; what scripts put in it goes too),
/// and what no script still holds of it is let go at once, rather than at
/// the layer's next sweep.
fn remove_mirror(rbx: &mut Rbx, id: u32) {
    let Some(m) = rbx.world.mirrors.remove(&id) else {
        return;
    };
    // A player's Character: CharacterRemoving first, and it is theirs no more.
    if let Some(o) = rbx.world.others.iter_mut().find(|o| o.character == Some(m.model)) {
        o.character = None;
        let player = o.player;
        let mv = rbx.value(m.model);
        rbx.fire(player, "CharacterRemoving", vec![mv]);
        rbx.raw_set(player, "Character", Value::Nil);
    }
    let mut tree = vec![m.model];
    tree.extend(rbx.descendants(m.model));
    rbx.destroy(m.model);
    loop {
        let free: Vec<Id> = tree
            .iter()
            .copied()
            .filter(|&i| {
                rbx.try_inst(i).is_some_and(|inst| {
                    inst.destroyed && matches!(&inst.value, Value::Userdata(u) if Rc::strong_count(u) == 1)
                })
            })
            .collect();
        if free.is_empty() {
            break;
        }
        for i in free {
            rbx.insts.remove(&i);
        }
    }
}

/// What a mirror is called: a player's name (the game's, else the player
/// list's for that entity) or a mob's custom name, else its type's
/// ("minecraft:cave_spider" is "Cave Spider"); a dropped item after the
/// item, an orb "Experience Orb".
fn mirror_name(rbx: &Rbx, e: &EntityInfo) -> String {
    match &e.item {
        _ if !e.name.is_empty() => e.name.clone(),
        None => rbx
            .world
            .others
            .iter()
            .find(|o| e.kind == PLAYER && o.entity == Some(e.id))
            .and_then(|o| rbx.try_inst(o.player))
            .map_or_else(|| title(&e.kind), |p| p.name.clone()),
        Some(_) if e.kind == ORB => "Experience Orb".into(),
        Some((item, _)) => title(item),
    }
}

/// A mob's Humanoid state, from the game: Dead with no health, Freefall off
/// the ground, else Running.
fn entity_state(e: &EntityInfo) -> &'static str {
    if e.health <= 0.0 {
        "Dead"
    } else if !e.on_ground {
        "Freefall"
    } else {
        "Running"
    }
}

/// A mob's two boxes, in blocks: its root part's height, from its feet to
/// under its head, and the side of its head, a cube on the root part at its
/// eyes. Together they cover its box, and each can be hit on its own.
pub fn mob_boxes(width: f64, height: f64, eye: f64) -> (f64, f64) {
    let side = width.min(0.5).min(height / 2.0).max(0.05);
    let root = (eye - side / 2.0).max(height / 2.0);
    (root, side)
}

// ---------------------------------------------------------------- the other players

/// The server's player list, read every half second while a script looks
/// (the first look reads it at once): a Player in Players for each other
/// player, in the order they joined, with PlayerAdded; PlayerRemoving as one
/// leaves the list. The local player's own entry gives the LocalPlayer the
/// list's display name and its ping. Where the list can't be read now, it is
/// read the next frame. Those there as a script first looks (`looking`) are
/// there before it, with their Characters; one who joins after gets theirs a
/// frame after PlayerAdded, so its handlers can connect CharacterAdded.
fn sync_players(rbx: &mut Rbx, game: &mut dyn GameApi, looking: bool) {
    // A Player something else destroyed (nothing should: they are locked) is
    // let go, and the list, read again at once, gives that player a new one.
    let insts = &rbx.insts;
    let before = rbx.world.others.len();
    rbx.world
        .others
        .retain(|o| insts.get(&o.player).is_some_and(|i| !i.destroyed));
    if rbx.world.others.len() != before {
        rbx.world.next_list = 0.0;
    }
    if rbx.now < rbx.world.next_list {
        return;
    }
    let Some(list) = game.players() else {
        return;
    };
    rbx.world.next_list = rbx.now + PLAYERS_EVERY;
    rbx.world.next_ping = rbx.world.next_list;
    if let Some(me) = list.iter().find(|e| e.local) {
        rbx.world.ping = me.latency;
        if !me.display_name.is_empty() {
            let lp = rbx.local_player;
            rbx.raw_set(lp, "DisplayName", Value::str(&me.display_name));
        }
    }
    let listed: Vec<&PlayerEntry> = list.iter().filter(|e| !e.local).collect();
    let gone: Vec<u128> = rbx
        .world
        .others
        .iter()
        .map(|o| o.uuid)
        .filter(|u| !listed.iter().any(|e| e.uuid == *u))
        .collect();
    for uuid in gone {
        remove_player(rbx, uuid);
    }
    for e in listed {
        let name = if e.name.is_empty() { "Player" } else { e.name.as_str() };
        let shown = if e.display_name.is_empty() {
            name
        } else {
            e.display_name.as_str()
        };
        let Some(o) = rbx.world.others.iter_mut().find(|o| o.uuid == e.uuid) else {
            add_player(rbx, e, name, shown, !looking);
            continue;
        };
        o.entity = e.entity;
        o.latency = e.latency;
        let player = o.player;
        rename(rbx, player, name);
        rbx.raw_set(player, "DisplayName", Value::str(shown));
    }
    count_players(rbx);
}

/// A player who joined: a locked Player of the studio's (scripts can't
/// destroy it), with their name, display name, a UserId from their UUID and
/// the UUID attribute, as the LocalPlayer has them. PlayerAdded fires; with
/// `later`, their Character comes a frame after.
fn add_player(rbx: &mut Rbx, e: &PlayerEntry, name: &str, shown: &str, later: bool) {
    let player = rbx.make("Player", None);
    rbx.inst_mut(player).name = name.to_owned();
    rbx.raw_set(player, "DisplayName", Value::str(shown));
    rbx.raw_set(player, "UserId", Value::Num(user_id(e.uuid)));
    rbx.raw_set(player, "CharacterAppearanceId", Value::Num(user_id(e.uuid)));
    set_attribute(rbx, player, "UUID", Value::str(&uuid_text(e.uuid)));
    rbx.inst_mut(player).locked = true;
    // Their Backpack, which replicates in Roblox (what they carry isn't known here).
    let backpack = rbx.make("Backpack", None);
    rbx.inst_mut(backpack).name = "Backpack".into();
    let _ = rbx.set_parent(backpack, Some(player));
    rbx.inst_mut(backpack).locked = true;
    rbx.world.others.push(Other {
        uuid: e.uuid,
        player,
        entity: e.entity,
        latency: e.latency,
        character: None,
        later,
    });
    let players = rbx.players;
    let _ = rbx.set_parent(player, Some(players));
    let pv = rbx.value(player);
    rbx.fire(players, "PlayerAdded", vec![pv]);
}

/// A player who left: CharacterRemoving if they had a Character, then
/// PlayerRemoving, and their Player is destroyed.
fn remove_player(rbx: &mut Rbx, uuid: u128) {
    let Some(i) = rbx.world.others.iter().position(|o| o.uuid == uuid) else {
        return;
    };
    let o = rbx.world.others.remove(i);
    if let Some(c) = o.character {
        let cv = rbx.value(c);
        rbx.fire(o.player, "CharacterRemoving", vec![cv]);
        rbx.raw_set(o.player, "Character", Value::Nil);
    }
    let (players, pv) = (rbx.players, rbx.value(o.player));
    rbx.fire(players, "PlayerRemoving", vec![pv]);
    rbx.destroy(o.player);
    count_players(rbx);
}

/// The local player's ping, from the server's list as it is now: read for
/// it at most every PLAYERS_EVERY (sync_players reads it while a script
/// looks), so that a UI script's ping counter needs no look at the world.
/// While the list can't be read, the last ping stays.
fn local_ping(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if rbx.now < rbx.world.next_ping {
        return;
    }
    let Some(list) = game.players() else {
        return;
    };
    rbx.world.next_ping = rbx.now + PLAYERS_EVERY;
    if let Some(me) = list.iter().find(|e| e.local) {
        rbx.world.ping = me.latency;
    }
}

/// Players.NumPlayers, the players on the server, and MaxPlayers: at least
/// as many.
fn count_players(rbx: &mut Rbx) {
    let n = 1 + rbx.world.others.len();
    let players = rbx.players;
    rbx.raw_set(players, "MaxPlayers", Value::Num(n as f64));
    rbx.raw_set(players, "NumPlayers", Value::Num(n as f64));
}

/// Each other player's Character is the mirror of their entity while the
/// server sends it (they are near): CharacterAdded and
/// CharacterAppearanceLoaded fire as it comes, and CharacterRemoving as it
/// goes (`remove_mirror`). A player entity no Player is listed for (a
/// server's NPC) stays a character of no one's.
fn link_characters(rbx: &mut Rbx) {
    for i in 0..rbx.world.others.len() {
        if std::mem::take(&mut rbx.world.others[i].later) {
            continue;
        }
        let o = &rbx.world.others[i];
        let model = o
            .entity
            .and_then(|e| rbx.world.mirrors.get(&e))
            .filter(|m| m.kind == PLAYER)
            .map(|m| m.model);
        if model == o.character {
            continue;
        }
        let (player, old) = (o.player, o.character);
        rbx.world.others[i].character = model;
        if let Some(old) = old {
            let ov = rbx.value(old);
            rbx.fire(player, "CharacterRemoving", vec![ov]);
            rbx.raw_set(player, "Character", Value::Nil);
        }
        if let Some(new) = model {
            let cv = rbx.value(new);
            rbx.raw_set(player, "Character", cv.clone());
            rbx.fire(player, "CharacterAdded", vec![cv.clone()]);
            rbx.fire(player, "CharacterAppearanceLoaded", vec![cv]);
        }
    }
}

/// Everyone in Players: the LocalPlayer, then the others in the order they
/// joined.
pub fn players_of(rbx: &Rbx) -> Vec<Id> {
    std::iter::once(rbx.local_player)
        .chain(rbx.world.others.iter().map(|o| o.player))
        .collect()
}

/// Whose Character `model` is, if anyone's.
fn player_of_character(rbx: &Rbx, model: Id) -> Option<Id> {
    if model == rbx.character {
        return Some(rbx.local_player);
    }
    rbx.world
        .others
        .iter()
        .find(|o| o.character == Some(model))
        .map(|o| o.player)
}

/// The Players' and a Player's methods that know who is on the server.
/// Players: GetPlayers (the LocalPlayer first), GetPlayerFromCharacter,
/// GetPlayerByUserId, and GetNameFromUserIdAsync and GetUserIdFromNameAsync
/// among the players here (Roblox's website isn't asked: anyone else is an
/// error). A Player: GetMouse (the LocalPlayer's alone), Kick and
/// LoadCharacter (a LocalScript can't, for another player),
/// DistanceFromCharacter (from their Head) and GetNetworkPing (seconds).
pub fn player_method(env: &mut dyn Env, lua: &mut Lua, id: Id, name: &str, args: &[Value]) -> Res {
    let rbx = env.rbx();
    let local = id == rbx.local_player;
    let by_id = |rbx: &Rbx, n: f64| {
        players_of(rbx)
            .into_iter()
            .find(|&p| rbx.raw_get(p, "UserId").to_num() == Some(n))
    };
    let one = |v: Value| Ok(vec![v]);
    match name {
        "GetPlayers" => one(rbx.list(&players_of(rbx))),
        "GetPlayerFromCharacter" => {
            let found = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .and_then(|m| player_of_character(rbx, m));
            one(found.map_or(Value::Nil, |p| rbx.value(p)))
        }
        "GetPlayerByUserId" => {
            let n = check_num(lua, args, 1, name)?;
            one(by_id(rbx, n).map_or(Value::Nil, |p| rbx.value(p)))
        }
        "GetNameFromUserIdAsync" => {
            let n = check_num(lua, args, 1, name)?;
            match by_id(rbx, n) {
                Some(p) => one(Value::str(&rbx.inst(p).name)),
                None => Err(lua.rt_error(format!(
                    "Players:GetNameFromUserIdAsync() failed: no player here has the UserId {}, and only the players on this server are known",
                    super::hstudio_lua::fmt_num(n)
                ))),
            }
        }
        "GetUserIdFromNameAsync" => {
            let n = check_text(lua, args, 1, name)?;
            let found = players_of(rbx)
                .into_iter()
                .find(|&p| rbx.try_inst(p).is_some_and(|i| i.name.eq_ignore_ascii_case(&n)));
            match found {
                Some(p) => one(rbx.raw_get(p, "UserId")),
                None => Err(lua.rt_error(format!(
                    "Players:GetUserIdFromNameAsync() failed: no player here is named {n}, and only the players on this server are known"
                ))),
            }
        }
        "GetMouse" => one(if local { rbx.value(rbx.mouse) } else { Value::Nil }),
        // The client leaves the server, saying why as a kick's reason does.
        "Kick" if local => {
            let why = args.get(1).and_then(Value::as_text).unwrap_or_default();
            let why = why.trim();
            let reason = if why.is_empty() {
                "You were kicked from this experience".to_owned()
            } else {
                format!("You were kicked from this experience: {why}")
            };
            rbx.client
                .actions
                .push(super::hstudio::ClientAction::Leave(Some(reason)));
            Ok(Vec::new())
        }
        "Kick" => Err(lua.rt_error("Cannot kick a non-local Player from a LocalScript")),
        "LoadCharacter" if local => {
            respawn(rbx);
            Ok(Vec::new())
        }
        "LoadCharacter" => Err(lua.rt_error("LoadCharacter can only be called by the backend server")),
        "DistanceFromCharacter" => {
            let at = args
                .get(1)
                .and_then(super::hstudio_rbx_values::as_v3)
                .ok_or_else(|| bad_arg(lua, 1, name, "Vector3 expected"))?;
            let head = if local {
                Some(rbx.head)
            } else {
                rbx.world
                    .others
                    .iter()
                    .find(|o| o.player == id)
                    .and_then(|o| o.character)
                    .and_then(|c| rbx.child_named(c, "Head"))
            };
            let d = head
                .and_then(|h| as_cf(&rbx.raw_get(h, "CFrame")))
                .map_or(0.0, |c| (at - c.position()).magnitude());
            one(Value::Num(d))
        }
        "GetNetworkPing" if local => {
            let (rbx, game) = env.rbx_and_game();
            local_ping(rbx, game);
            one(Value::Num(f64::from(rbx.world.ping.max(0)) / 1000.0))
        }
        "GetNetworkPing" => {
            let ms = rbx
                .world
                .others
                .iter()
                .find(|o| o.player == id)
                .map_or(0, |o| o.latency);
            one(Value::Num(f64::from(ms.max(0)) / 1000.0))
        }
        _ => Err(lua.rt_error(format!("{name} is not available here"))),
    }
}

// ---------------------------------------------------------------- what scripts read and do

/// Members that are read, not kept as properties: game.LocalPlayer,
/// Horizon's own shortcut for game.Players.LocalPlayer, Roblox's older
/// game.workspace and Players.localPlayer, and the Mouse's UnitRay, Origin
/// and TargetSurface, worked out as they are read. None of them can be set.
pub fn member(rbx: &Rbx, id: Id, key: &str) -> Option<Value> {
    match key {
        "LocalPlayer" if id == rbx.game => Some(rbx.value(rbx.local_player)),
        "workspace" if id == rbx.game => Some(rbx.value(rbx.workspace)),
        "localPlayer" if id == rbx.players => Some(rbx.value(rbx.local_player)),
        "UnitRay" | "Origin" | "TargetSurface" if id == rbx.mouse => Some(rbx.raw_get(id, key)),
        _ => None,
    }
}

/// What a read of `key` on `id` needs from the game first: the Mouse's Hit,
/// Target, TargetSurface, UnitRay and Origin are worked out where it points,
/// once a frame (its Hit, Target and TargetSurface are a look at the world);
/// the local Humanoid's FloorMaterial is the block under the player, looked
/// up only when a script asks; and a read that reaches the world's entities
/// (`reaches_world`) has them mirrored.
pub fn before_read(env: &mut dyn Env, id: Id, key: &str) {
    if id == env.rbx().mouse {
        // Where the Mouse points can be an entity's part.
        if matches!(key, "Hit" | "Target" | "TargetSurface") {
            look(env);
        }
        if matches!(key, "Hit" | "Target" | "TargetSurface" | "UnitRay" | "Origin") {
            mouse_once_a_frame(env);
        }
        return;
    }
    if key == "FloorMaterial" && id == env.rbx().humanoid {
        let (rbx, game) = env.rbx_and_game();
        let m = floor(rbx, game);
        rbx.quiet_set(id, "FloorMaterial", enum_value(enum_item("Material", m)));
    }
    let owner = env.owner();
    if reaches_world(env.rbx(), owner, id, key) {
        look(env);
    }
}

/// Where the Mouse points, worked out once a frame as Roblox does it, and
/// again only as the pointer moves or the Mouse's TargetFilter changes: a
/// script that reads Hit and Target each frame casts one ray.
fn mouse_once_a_frame(env: &mut dyn Env) {
    let rbx = env.rbx();
    let filter = rbx.id_of(&rbx.raw_get(rbx.mouse, "TargetFilter"));
    let now = (rbx.world.frames, rbx.pointer.free, rbx.pointer.x, rbx.pointer.y, filter);
    if rbx.world.mouse_at == Some(now) {
        return;
    }
    super::hstudio_rbx_draw::mouse_refresh(env);
    env.rbx().world.mouse_at = Some(now);
}

/// Whether reading `key` of `id` reaches the world's entities: anything of
/// Players but the LocalPlayer, the Workspace's ChildAdded, ChildRemoved and
/// descendant events and a child by its name, and game's descendant events.
/// The Workspace's CurrentCamera, Terrain, Gravity and the like don't, nor
/// does a child by its name that is the Workspace's own (`workspace.Camera`,
/// the character, a script's part), so a UI script never has the world
/// mirrored. (For a script that looks already, any child is a look: it
/// keeps the console's going, and its children aren't searched twice.)
fn reaches_world(rbx: &Rbx, owner: Owner, id: Id, key: &str) -> bool {
    if id == rbx.players {
        !matches!(key, "LocalPlayer" | "localPlayer")
    } else if id == rbx.game {
        matches!(key, "DescendantAdded" | "DescendantRemoving")
    } else if id == rbx.workspace {
        matches!(
            key,
            "ChildAdded" | "ChildRemoved" | "DescendantAdded" | "DescendantRemoving"
        ) || !super::hstudio_rbx::is_member(rbx.inst(id).class, key)
            && (rbx.world.lookers.contains(&owner) || !own_child(rbx, key))
    } else {
        false
    }
}

/// Whether the Workspace's child named `name` (the first, as a read finds
/// it) is there of its own, not an entity's mirror.
fn own_child(rbx: &Rbx, name: &str) -> bool {
    rbx.child_named(rbx.workspace, name)
        .is_some_and(|c| !rbx.world.mirrors.values().any(|m| m.model == c))
}

/// What a call of `name` on `id` needs first: the world's entities mirrored,
/// when the call reaches them. On the Workspace, the methods that list or
/// find its children and its raycast; on game, those that search all of it.
pub fn before_call(env: &mut dyn Env, id: Id, name: &str, args: &[Value]) {
    let rbx = env.rbx();
    let reaches = if id == rbx.workspace {
        matches!(
            name,
            "GetChildren"
                | "GetDescendants"
                | "FindFirstChild"
                | "FindFirstChildOfClass"
                | "FindFirstChildWhichIsA"
                | "FindFirstDescendant"
                | "WaitForChild"
                | "Raycast"
                | "GetPartBoundsInRadius"
                | "GetPartBoundsInBox"
                | "GetPartsInPart"
                | "FindPartOnRay"
                | "FindPartOnRayWithIgnoreList"
                | "FindPartOnRayWithWhitelist"
        )
    } else if id == rbx.game {
        matches!(name, "GetDescendants" | "FindFirstDescendant")
            || name.starts_with("FindFirstChild") && args.get(2).is_some_and(Value::truthy)
    } else {
        false
    };
    if reaches {
        look(env);
    }
}

/// The Roblox material of what the player stands on: the block under the
/// middle or a corner of their box; Air off the ground, Water swimming.
fn floor(rbx: &Rbx, game: &mut dyn GameApi) -> &'static str {
    use super::hstudio_rbx_draw::{block_material, passable};
    let Some(p) = game.player() else { return "Air" };
    let at = |game: &mut dyn GameApi, x: f64, y: f64, z: f64| {
        game.block(x.floor() as i32, y.floor() as i32, z.floor() as i32)
    };
    if !rbx.world.on_ground {
        let wet = at(game, p.x, p.y, p.z).is_some_and(|b| b.contains("water"));
        return if wet { "Water" } else { "Air" };
    }
    for (dx, dz) in [(0.0, 0.0), (-0.3, -0.3), (0.3, -0.3), (-0.3, 0.3), (0.3, 0.3)] {
        if let Some(b) = at(game, p.x + dx, p.y - 0.05, p.z + dz)
            && !passable(&b)
        {
            return block_material(&b);
        }
    }
    "Air"
}

/// A Humanoid's state: the local one's as the game has the player, a
/// mirrored mob's as the game has it, and another's from its health.
pub fn state_of(rbx: &Rbx, humanoid: Id) -> &'static str {
    if humanoid == rbx.humanoid {
        return rbx.world.state;
    }
    if let Some(m) = rbx.world.mirrors.values().find(|m| m.humanoid == Some(humanoid)) {
        return m.state;
    }
    if rbx.raw_get(humanoid, "Health").to_num().is_some_and(|h| h <= 0.0) {
        "Dead"
    } else {
        "Running"
    }
}

/// The local root part was moved (its CFrame or Position set, PivotTo, a
/// tween): the player goes there, their feet as far under its centre as
/// this frame's pose has them, with a /tp at the end of the frame. A later
/// move takes the earlier's place.
///
/// The player faces the way it faces, as a character turns with its root
/// part (a script turning it to look at someone): a frame with a rotation
/// of its own, that is, not a bare position (`CFrame.new(x, y, z)`), whose
/// yaw isn't the player's. Set where it is (turned on the spot, or put
/// back), it moves no one: as in Roblox, the player keeps their motion.
pub fn root_moved(rbx: &mut Rbx) {
    let frame = as_cf(&rbx.raw_get(rbx.root_part, "CFrame")).unwrap_or_default();
    let at = frame.position();
    let look = frame.look_vector();
    let bare = (look - Vector3::new(0.0, 0.0, -1.0)).magnitude() < 1e-9
        && (frame.up_vector() - Vector3::Y_AXIS).magnitude() < 1e-9;
    if !bare && Vector3::new(look.x, 0.0, look.z).magnitude() > 0.1 {
        let (yaw, _) = yaw_pitch(look);
        let turn = (yaw - rbx.world.yaw + 540.0).rem_euclid(360.0) - 180.0;
        if turn.abs() > 0.5 {
            rbx.world.effects.look = Some((yaw, rbx.world.pitch));
            rbx.world.yaw = yaw;
        }
    }
    let feet = at - Vector3::new(0.0, rbx.world.root_y, 0.0);
    // Out of the world, or where Roblox destroys the character, it doesn't go.
    let goes = super::hstudio_rbx_body::root_moved(rbx, frame, feet);
    if (at - rbx.world.root_at).magnitude() < 1e-6 {
        rbx.world.teleport = None;
    } else if goes {
        rbx.world.teleport = Some(feet);
    }
}

/// A part moved by a script: another entity's mirror's root part moves
/// the entity in the client's world (as Roblox's client moves another
/// player's character, until the server's next word on it).
pub fn mirror_moved(rbx: &mut Rbx, id: Id) {
    let Some((&entity, m)) = rbx.world.mirrors.iter().find(|(_, m)| m.root == id) else {
        return;
    };
    let lift = m.lift;
    let at = as_cf(&rbx.raw_get(id, "CFrame")).unwrap_or_default().position();
    rbx.world.entity_moves.retain(|(e, _)| *e != entity);
    rbx.world.entity_moves.push((entity, [at.x, at.y - lift, at.z]));
}

/// Where Model:MoveTo puts the player's root part, asked for `at`: Roblox
/// moves a model up out of what is in its way, so the player's box (its
/// width and height over their feet, `root_y` under the root part's centre)
/// goes onto the top of the highest solid block it meets, again until it
/// meets none. A point on a block's top puts their feet on the block.
pub fn room_for_player(game: &mut dyn GameApi, root_y: f64, at: Vector3) -> Vector3 {
    let up = Vector3::new(0.0, root_y, 0.0);
    let mut feet = at - up;
    for _ in 0..MAX_RAISE {
        match top_in_the_way(game, feet) {
            Some(top) => feet.y = top,
            None => break,
        }
    }
    feet + up
}

/// The top of the highest solid block the player's box meets, standing at
/// `feet`.
fn top_in_the_way(game: &mut dyn GameApi, feet: Vector3) -> Option<f64> {
    use super::hstudio_rbx_draw::passable;
    // The blocks a span of the box reaches into (one it only touches isn't).
    let blocks = |lo: f64, hi: f64| lo.floor() as i32..=(hi.ceil() as i32).saturating_sub(1);
    let half = WIDTH / 2.0;
    for y in blocks(feet.y, feet.y + HEIGHT).rev() {
        for x in blocks(feet.x - half, feet.x + half) {
            for z in blocks(feet.z - half, feet.z + half) {
                if game.block(x, y, z).is_some_and(|b| !passable(&b)) {
                    return Some(f64::from(y) + 1.0);
                }
            }
        }
    }
    None
}

/// Where the client moves the player's feet (a script moved the root part).
pub fn take_teleport(rbx: &mut Rbx) -> Option<[f64; 3]> {
    let to = rbx.world.teleport.take()?;
    Some([to.x, to.y, to.z])
}

/// A script set the local Humanoid's WalkSpeed, JumpPower or JumpHeight
/// (`prop`): the game's walking or jumping scales to it (16, 50 and 7.2
/// its own; a jump's height goes with the square of its speed), the other
/// as it was asked last.
pub fn speeds_set(rbx: &mut Rbx, prop: &str) {
    let hum = rbx.humanoid;
    let num = |rbx: &Rbx, k: &str, d: f64| rbx.raw_get(hum, k).to_num().unwrap_or(d).max(0.0);
    let (mut walk, mut power) = rbx.world.speed_request.unwrap_or(rbx.world.body.scales);
    match prop {
        "WalkSpeed" => walk = num(rbx, "WalkSpeed", 16.0) / 16.0,
        "JumpPower" => power = num(rbx, "JumpPower", 50.0) / 50.0,
        _ => power = (num(rbx, "JumpHeight", 7.2) / 7.2).sqrt(),
    }
    rbx.world.speed_request = Some((walk, power));
}

/// Whether the character passes through blocks: as a noclip script does it,
/// its Torso's CanCollide off this frame (the HumanoidRootPart's is off
/// anyway, as in Roblox). As Roblox's Humanoid does each step, the Torso
/// takes it back, so a script keeps it off frame after frame, and when it
/// stops the player collides again.
pub fn noclip(rbx: &mut Rbx) -> bool {
    let torso = rbx.world.limbs[0];
    if rbx.try_inst(torso).is_none_or(|i| i.destroyed) {
        return false;
    }
    let off = !rbx.raw_get(torso, "CanCollide").truthy();
    if off {
        rbx.quiet_set(torso, "CanCollide", Value::Bool(true));
    }
    off
}

/// The velocity (studs a second) a mover in the local root part gives the
/// player this frame: a BodyVelocity's (any MaxForce), an enabled
/// LinearVelocity's, or towards a BodyPosition's Position. None when
/// nothing moves it.
pub fn mover_velocity(rbx: &Rbx) -> Option<Vector3> {
    let rp = rbx.root_part;
    let at = as_cf(&rbx.raw_get(rp, "CFrame")).unwrap_or_default().position();
    let kids = rbx.try_inst(rp)?.children.clone();
    let v3 = |id: Id, k: &str| super::hstudio_rbx_values::as_v3(&rbx.raw_get(id, k)).unwrap_or_default();
    for c in kids {
        let Some(i) = rbx.try_inst(c).filter(|i| !i.destroyed) else {
            continue;
        };
        match i.class.name {
            "BodyVelocity" if v3(c, "MaxForce").magnitude() > 0.0 => return Some(v3(c, "Velocity")),
            "LinearVelocity" if rbx.raw_get(c, "Enabled").truthy() => return Some(v3(c, "VectorVelocity")),
            "BodyPosition" if v3(c, "MaxForce").magnitude() > 0.0 => {
                // Most of the way there each second, as its P pulls.
                let to = v3(c, "Position") - at;
                return Some(to * 10.0);
            }
            _ => {}
        }
    }
    None
}

/// Humanoid:MoveTo. The local Humanoid walks the player there with the
/// agent's goto, and MoveToFinished fires with whether they got there:
/// false after 8 seconds (Roblox's timeout), and when another MoveTo takes
/// the walk's place. Another Humanoid has no one to walk: its MoveToFinished
/// fires with false. A MoveTo to the block the walk goes to already (a
/// follow loop's, each frame) keeps the walk going, its timeout anew.
pub fn move_to(rbx: &mut Rbx, humanoid: Id, to: Vector3) {
    if humanoid != rbx.humanoid {
        rbx.fire(humanoid, "MoveToFinished", vec![Value::Bool(false)]);
        return;
    }
    let block = |v: Vector3| (v.x.floor(), v.y.floor(), v.z.floor());
    let now = rbx.now;
    if let Some(w) = rbx.world.walk.as_mut()
        && w.humanoid == humanoid
        && block(w.to) == block(to)
    {
        w.since = now;
        return;
    }
    let stop = match rbx.world.walk.take() {
        Some(w) => {
            rbx.fire(w.humanoid, "MoveToFinished", vec![Value::Bool(false)]);
            w.sent || w.stop
        }
        None => false,
    };
    rbx.world.walk = Some(Walk {
        humanoid,
        to,
        since: rbx.now,
        sent: false,
        stop,
        reply: None,
    });
}

/// Each frame, as the studio does what the layer asked of the game: a new
/// walk's goto goes to the agent (after a stop, when it takes another's
/// place), and its answer, or the timeout, ends it.
pub fn walk(rbx: &mut Rbx, game: &mut dyn GameApi) {
    let now = rbx.now;
    let Some(w) = rbx.world.walk.as_mut() else {
        return;
    };
    let arrived = |v: &serde_json::Value| v.get("arrived").and_then(serde_json::Value::as_bool) == Some(true);
    let mut done = None;
    if !w.sent {
        w.sent = true;
        if w.stop {
            let _ = game.agent("stop");
        }
        let (x, y, z) = (w.to.x.floor(), w.to.y.floor(), w.to.z.floor());
        match game.agent(&format!("goto {x:.0} {y:.0} {z:.0} --near 1 --timeout {MOVE_TIMEOUT}")) {
            Ok(Reply::Later(rx)) => w.reply = Some(rx),
            Ok(Reply::Now(v)) => done = Some(arrived(&v)),
            // No path, or the agent busy with a mod's task: it doesn't walk.
            Err(_) => done = Some(false),
        }
    }
    if done.is_none() {
        done = match w.reply.as_ref().map(Receiver::try_recv) {
            Some(Ok(text)) => Some(serde_json::from_str(&text).is_ok_and(|v| arrived(&v))),
            Some(Err(TryRecvError::Disconnected)) => Some(false),
            _ if now - w.since >= MOVE_TIMEOUT => Some(false),
            _ => None,
        };
    }
    if let Some(reached) = done {
        let humanoid = w.humanoid;
        rbx.world.walk = None;
        rbx.fire(humanoid, "MoveToFinished", vec![Value::Bool(reached)]);
    }
}

// ---------------------------------------------------------------- helpers

/// A Roblox UserId for a profile's UUID: its low 49 bits, a whole number
/// under 10^15, which tostring writes out in full (1 when there is none, as
/// the layer's own).
pub fn user_id(uuid: u128) -> f64 {
    ((uuid & ((1 << 49) - 1)) as f64).max(1.0)
}

/// A UUID as it is written: 8-4-4-4-12 hex digits.
pub fn uuid_text(uuid: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        uuid >> 96,
        (uuid >> 80) & 0xffff,
        (uuid >> 64) & 0xffff,
        (uuid >> 48) & 0xffff,
        uuid & 0xffff_ffff_ffff
    )
}

/// Renames an instance as a script's `inst.Name = name` does: Changed fires,
/// and a WaitForChild waiting for the new name gets it. One that is gone
/// stays gone.
pub fn rename(rbx: &mut Rbx, id: Id, name: &str) {
    let Some(inst) = rbx.insts.get_mut(&id) else {
        return;
    };
    if inst.name == name {
        return;
    }
    inst.name = name.to_owned();
    let parent = inst.parent;
    rbx.changed(id, "Name", &Value::Nil);
    if let Some(p) = parent {
        rbx.child_arrived(p, id);
    }
}

/// The way a yaw and a pitch in degrees look, as the game turns them (yaw 0
/// looks south, +Z; a positive pitch looks down).
fn facing(yaw: f32, pitch: f32) -> Vector3 {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    Vector3::new(-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos())
}

/// A game id as a name: "minecraft:cave_spider" is "Cave Spider".
pub fn title(id: &str) -> String {
    let name = id.rsplit(':').next().unwrap_or(id);
    name.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sets an attribute as SetAttribute does, with its events when it changes,
/// on an instance that is still there.
pub fn set_attribute(rbx: &mut Rbx, id: Id, name: &str, v: Value) {
    let Some(inst) = rbx.insts.get_mut(&id) else {
        return;
    };
    let attrs = &mut inst.attributes;
    match attrs.iter().position(|(k, _)| k == name) {
        Some(i) if super::hstudio_rbx::same_value(&attrs[i].1, &v) => return,
        Some(i) => attrs[i].1 = v,
        None => attrs.push((name.to_owned(), v)),
    }
    rbx.fire(id, "AttributeChanged", vec![Value::str(name)]);
    rbx.fire(id, &format!("attr:{name}"), Vec::new());
}
