// The Roblox layer's world (hstudio_rbx_world): the Workspace as the
// Minecraft world. Players.LocalPlayer is the player playing, and its
// Character follows them each frame: an R6 rig as Roblox's, in blocks (a
// stud is a block), whose HumanoidRootPart is the torso's box, whose Head
// is at the eyes and whose Humanoid is the player's. hstudio_rbx_draw's
// sync hands the player over to it every frame.

use super::hstudio::{GameApi, PlayerInfo};
use super::hstudio_lua::Value;
use super::hstudio_rbx::{Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{cf, enum_item, enum_value, v3};

/// A pixel of the player's model: the model is 15/16 the size of a
/// block-sized one, whose pixel is 1/16 of a block.
const PX: f64 = 0.9375 / 16.0;
/// The torso's centre above the feet of a player standing: the legs' 12
/// pixels and half the body's.
pub const TORSO_Y: f64 = 18.0 * PX;
/// The eyes above the feet of a player standing.
pub const EYES_Y: f64 = 1.62;
/// Health in the layer is the game's times this (20 hearts' worth is 100).
pub const HEALTH_SCALE: f64 = 5.0;

/// The world as the layer last saw it, between frames.
pub struct World {
    /// The root part's centre above the player's feet this frame.
    pub root_y: f64,
}

impl Default for World {
    fn default() -> Self {
        World { root_y: TORSO_Y }
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
        sync_local(rbx, &p);
    }
}

/// The LocalPlayer's character follows the player: its root part on the
/// torso, its head at the eyes facing where they look, its Humanoid's health.
fn sync_local(rbx: &mut Rbx, p: &PlayerInfo) {
    let yaw = f64::from(p.yaw).to_radians();
    let pitch = f64::from(p.pitch).to_radians();
    let feet = Vector3::new(p.x, p.y, p.z);
    let flat = Vector3::new(-yaw.sin(), 0.0, yaw.cos());
    let look = Vector3::new(-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
    rbx.world.root_y = TORSO_Y;
    let root = feet + Vector3::new(0.0, TORSO_Y, 0.0);
    let head = feet + Vector3::new(0.0, EYES_Y, 0.0);
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
