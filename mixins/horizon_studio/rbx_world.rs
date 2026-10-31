// The Roblox layer's world (hstudio_rbx_world): the Workspace as the
// Minecraft world. Players.LocalPlayer is the player playing, and its
// Character follows them each frame: an R6 rig as Roblox's, in blocks (a
// stud is a block), whose HumanoidRootPart is the torso's box, whose Head
// is at the eyes and whose Humanoid is the player's. Moving the root part
// teleports the player with /tp. hstudio_rbx_draw's sync hands the player
// over to it every frame.

use super::hstudio::{GameApi, PlayerInfo, PlayerPose};
use super::hstudio_lua::Value;
use super::hstudio_rbx::{Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, cf, enum_item, enum_value, v3};

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
/// The LocalPlayer's name until the game says the player's.
pub const NO_NAME: &str = "Player1";

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
}

impl Default for World {
    fn default() -> Self {
        World {
            root_y: TORSO_Y,
            name: NO_NAME.into(),
            uuid: 0,
            teleport: None,
        }
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

/// Members Roblox and Horizon have under other names: game.LocalPlayer,
/// Horizon's own shortcut for game.Players.LocalPlayer, and Roblox's older
/// game.workspace and Players.localPlayer. They can't be set.
pub fn alias(rbx: &Rbx, id: Id, key: &str) -> Option<Value> {
    match key {
        "LocalPlayer" if id == rbx.game => Some(rbx.value(rbx.local_player)),
        "workspace" if id == rbx.game => Some(rbx.value(rbx.workspace)),
        "localPlayer" if id == rbx.players => Some(rbx.value(rbx.local_player)),
        _ => None,
    }
}

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

/// The world brought up to the game, each frame: the local player.
pub fn sync(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if let Some(p) = game.player() {
        let pose = game.pose();
        sync_local(rbx, &p, pose.as_ref());
    }
}

/// The LocalPlayer is the player, and its character follows them: its root
/// part on the torso, its head at the eyes facing where they look (both as
/// low as the pose has them), its Humanoid's health.
fn sync_local(rbx: &mut Rbx, p: &PlayerInfo, pose: Option<&PlayerPose>) {
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
}
