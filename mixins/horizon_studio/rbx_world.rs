// The Roblox layer's world (hstudio_rbx_world): the Workspace as the
// Minecraft world. Players.LocalPlayer is the player playing, and its
// Character follows them each frame: an R6 rig as Roblox's, in blocks (a
// stud is a block), whose HumanoidRootPart is the torso's box, whose Head
// is at the eyes and whose Humanoid is the player's: their health, the
// game's speeds and how they move. Moving the root part teleports the player
// with /tp, and Humanoid:MoveTo walks them with the agent's goto.
// hstudio_rbx_draw's sync hands the player over to it every frame.

use super::hstudio::{GameApi, PlayerInfo, PlayerPose, Reply};
use super::hstudio_lua::Value;
use super::hstudio_rbx::{Env, Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, cf, enum_item, enum_value, v3};
use std::sync::mpsc::{Receiver, TryRecvError};

/// A pixel of the player's model: the model is 15/16 the size of a
/// block-sized one, whose pixel is 1/16 of a block.
const PX: f64 = 0.9375 / 16.0;
/// The torso's centre above the feet of a player standing: the legs' 12
/// pixels and half the body's.
pub const TORSO_Y: f64 = 18.0 * PX;
/// The eyes above the feet of a player standing.
pub const EYES_Y: f64 = 1.62;
/// The player's box, standing.
pub const HEIGHT: f64 = 1.8;
/// Health in the layer is the game's times this (the game's 20 is 100).
pub const HEALTH_SCALE: f64 = 5.0;
/// Blocks a second for a movement_speed of 1: the game's 0.1 walks 4.3178
/// blocks a second, sprinting's 0.13 runs 5.6131.
pub const SPEED_SCALE: f64 = 43.178;
/// Ticks a second: jump_strength is blocks a tick.
pub const TICKS: f64 = 20.0;
/// Moving slower than this, in blocks a second, a Humanoid isn't walking.
const WALKING: f64 = 0.5;
/// How long Humanoid:MoveTo walks before it gives up, in seconds (Roblox's).
pub const MOVE_TIMEOUT: f64 = 8.0;
/// The LocalPlayer's name until the game says the player's.
pub const NO_NAME: &str = "Player1";

// ---------------------------------------------------------------- state

/// The world as the layer last saw it, between frames.
pub struct World {
    /// The root part's centre above the player's feet this frame.
    pub root_y: f64,
    /// The profile name and UUID the LocalPlayer was last given.
    pub name: String,
    pub uuid: u128,
    /// Where the last move of the root part this frame puts the player's
    /// feet: the /tp to send.
    pub teleport: Option<Vector3>,
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
}

impl Default for World {
    fn default() -> Self {
        World {
            root_y: TORSO_Y,
            name: NO_NAME.into(),
            uuid: 0,
            teleport: None,
            speed: None,
            jump: None,
            state: "Running",
            on_ground: true,
            running: 0.0,
            walk: None,
        }
    }
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

/// A character's instances.
#[derive(Clone, Copy, Debug)]
pub struct Rig {
    pub model: Id,
    pub root: Id,
    pub head: Id,
    pub humanoid: Id,
}

/// A player's character, R6 in blocks: a Model named `name` whose
/// PrimaryPart, the HumanoidRootPart, is the torso's box, a Head (the head's
/// 8-pixel cube) and a Humanoid. Its parts are invisible (the game draws the
/// player), anchored, and touch nothing; like the services it is locked, so
/// scripts can add to it but not take it apart. Not in the Workspace yet.
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
    let _ = rbx.set_parent(humanoid, Some(model));
    rbx.raw_set(model, "PrimaryPart", root_v);
    for id in [model, root, head, humanoid] {
        rbx.inst_mut(id).locked = true;
    }
    Rig {
        model,
        root,
        head,
        humanoid,
    }
}

fn rig_part(rbx: &mut Rbx, model: Id, name: &str, size: Vector3) -> Id {
    let part = rbx.make("Part", None);
    rbx.inst_mut(part).name = name.into();
    rbx.raw_set(part, "Size", v3(size));
    rbx.raw_set(part, "Transparency", Value::Num(1.0));
    rbx.raw_set(part, "Anchored", Value::Bool(true));
    rbx.raw_set(part, "CanCollide", Value::Bool(false));
    rbx.raw_set(part, "CanTouch", Value::Bool(false));
    rbx.raw_set(part, "Massless", Value::Bool(true));
    let _ = rbx.set_parent(part, Some(model));
    part
}

// ---------------------------------------------------------------- each frame

/// The world brought up to the game, each frame: the local player.
pub fn sync(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if let Some(p) = game.player() {
        let pose = game.pose();
        sync_local(rbx, &p, pose.as_ref());
    }
}

/// The LocalPlayer is the player, and its character follows them: its root
/// part on the torso, its head at the eyes facing where they look (both as
/// low as the pose has them), its Humanoid's health, speeds, motion and
/// state. Coming back to life, they get a new character.
fn sync_local(rbx: &mut Rbx, p: &PlayerInfo, pose: Option<&PlayerPose>) {
    if rbx.last_health.is_some_and(|h| h <= 0.0) && p.health > 0.0 {
        respawn(rbx);
    }
    identity(rbx, p);
    let yaw = f64::from(p.yaw).to_radians();
    let pitch = f64::from(p.pitch).to_radians();
    let feet = Vector3::new(p.x, p.y, p.z);
    let flat = Vector3::new(-yaw.sin(), 0.0, yaw.cos());
    let look = Vector3::new(-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
    let (height, eyes) = body(pose);
    rbx.world.root_y = TORSO_Y * height / HEIGHT;
    let root = feet + Vector3::new(0.0, rbx.world.root_y, 0.0);
    let head = feet + Vector3::new(0.0, eyes, 0.0);
    let (rp, hd, hum) = (rbx.root_part, rbx.head, rbx.humanoid);
    // Kept quiet so a script's teleport isn't undone by the player's own place.
    rbx.quiet_set(rp, "CFrame", cf(CFrame::look_at(root, root + flat, Vector3::Y_AXIS)));
    rbx.fire(rp, "prop:CFrame", Vec::new());
    rbx.quiet_set(hd, "CFrame", cf(CFrame::look_at(head, head + look, Vector3::Y_AXIS)));
    rbx.raw_set(hum, "MaxHealth", Value::Num(f64::from(p.max_health) * HEALTH_SCALE));
    let health = f64::from(p.health) * HEALTH_SCALE;
    if rbx.last_health.is_none_or(|h| h != p.health) {
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
    let state = next_state(rbx.world.state, p, pose);
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
    let way = if speed >= WALKING { flat / speed } else { Vector3::ZERO };
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
fn set_state(rbx: &mut Rbx, state: &'static str, speed: f64) {
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
    let player = rbx.local_player;
    let old = rbx.character;
    let old_v = rbx.value(old);
    rbx.fire(player, "CharacterRemoving", vec![old_v]);
    let name = rbx.inst(player).name.clone();
    let rig = build_character(rbx, &name);
    rbx.character = rig.model;
    rbx.root_part = rig.root;
    rbx.head = rig.head;
    rbx.humanoid = rig.humanoid;
    rbx.destroy(old);
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

// ---------------------------------------------------------------- what scripts read and do

/// Members that are read, not kept as properties: game.LocalPlayer,
/// Horizon's own shortcut for game.Players.LocalPlayer, Roblox's older
/// game.workspace and Players.localPlayer, and the Mouse's UnitRay and
/// Origin, worked out as they are read. None of them can be set.
pub fn member(rbx: &Rbx, id: Id, key: &str) -> Option<Value> {
    match key {
        "LocalPlayer" if id == rbx.game => Some(rbx.value(rbx.local_player)),
        "workspace" if id == rbx.game => Some(rbx.value(rbx.workspace)),
        "localPlayer" if id == rbx.players => Some(rbx.value(rbx.local_player)),
        "UnitRay" | "Origin" if id == rbx.mouse => Some(rbx.raw_get(id, key)),
        _ => None,
    }
}

/// What a read of `key` on `id` needs from the game first: the local
/// Humanoid's FloorMaterial is the block under the player, looked up only
/// when a script asks.
pub fn before_read(env: &mut dyn Env, id: Id, key: &str) {
    if key == "FloorMaterial" && id == env.rbx().humanoid {
        let (rbx, game) = env.rbx_and_game();
        let m = floor(rbx, game);
        rbx.quiet_set(id, "FloorMaterial", enum_value(enum_item("Material", m)));
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

/// A Humanoid's state: the local one's as the game has the player, and
/// another's from its health.
pub fn state_of(rbx: &Rbx, humanoid: Id) -> &'static str {
    if humanoid == rbx.humanoid {
        return rbx.world.state;
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
pub fn root_moved(rbx: &mut Rbx) {
    let at = as_cf(&rbx.raw_get(rbx.root_part, "CFrame"))
        .unwrap_or_default()
        .position();
    rbx.world.teleport = Some(at - Vector3::new(0.0, rbx.world.root_y, 0.0));
}

/// The teleport to send, as the game's command: never as a chat line.
pub fn take_teleport(rbx: &mut Rbx) -> Option<String> {
    let to = rbx.world.teleport.take()?;
    Some(format!("/tp @s {:.3} {:.3} {:.3}", to.x, to.y, to.z))
}

/// Humanoid:MoveTo. The local Humanoid walks the player there with the
/// agent's goto, and MoveToFinished fires with whether they got there:
/// false after 8 seconds (Roblox's timeout), and when another MoveTo takes
/// the walk's place. Another Humanoid has no one to walk: its MoveToFinished
/// fires with false.
pub fn move_to(rbx: &mut Rbx, humanoid: Id, to: Vector3) {
    if humanoid != rbx.humanoid {
        rbx.fire(humanoid, "MoveToFinished", vec![Value::Bool(false)]);
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
/// and a WaitForChild waiting for the new name gets it.
pub fn rename(rbx: &mut Rbx, id: Id, name: &str) {
    if rbx.inst(id).name == name {
        return;
    }
    rbx.inst_mut(id).name = name.to_owned();
    rbx.changed(id, "Name", &Value::Nil);
    if let Some(p) = rbx.inst(id).parent {
        rbx.child_arrived(p, id);
    }
}

/// Sets an attribute as SetAttribute does, with its events when it changes.
pub fn set_attribute(rbx: &mut Rbx, id: Id, name: &str, v: Value) {
    let attrs = &mut rbx.inst_mut(id).attributes;
    match attrs.iter().position(|(k, _)| k == name) {
        Some(i) if super::hstudio_rbx::same_value(&attrs[i].1, &v) => return,
        Some(i) => attrs[i].1 = v,
        None => attrs.push((name.to_owned(), v)),
    }
    rbx.fire(id, "AttributeChanged", vec![Value::str(name)]);
    rbx.fire(id, &format!("attr:{name}"), Vec::new());
}
