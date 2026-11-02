// The Roblox layer's interactive parts (hstudio_rbx_parts): ClickDetectors,
// ProximityPrompts and body movers on the parts scripts make.
//
// A click in the world (the left button, or the right) on a part whose
// ClickDetector is in it or in a model it is in, within the detector's
// MaxActivationDistance of the character's head, fires MouseClick (or
// RightMouseClick) with the LocalPlayer, as Roblox's do; one in the Terrain
// fires for a click on a block, and one a script put in an entity's mirror
// for a click on the entity. (fireclickdetector fires them too.)
//
// The nearest enabled ProximityPrompt within its MaxActivationDistance of
// the character is shown on the HUD ("[E] ActionText"), with PromptShown
// and PromptHidden (the prompt's and ProximityPromptService's) as it comes
// and goes. Its KeyboardKeyCode is then the prompt's, not the game's (the
// bridge keeps it from the game while a prompt is shown): pressed, it fires
// Triggered, or PromptButtonHoldBegan and Triggered HoldDuration later;
// let go, PromptButtonHoldEnded or TriggerEnded.
//
// BodyPositions, BodyGyros and BodyVelocities in an unanchored part a
// script made (not the character's, whose root part moves the player, nor
// a mirror's) move it as Roblox's physics would on the client that owns it:
// most of the way to the BodyPosition's Position each second, along the
// BodyVelocity's Velocity, turned to the BodyGyro's CFrame. So IY's
// tpunanchored brings parts to a player and freezeunanchored holds them.

use super::hstudio::GameApi;
use super::hstudio_rbx::{Env, Id, Rbx, is_a};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, as_item, as_v3, cf, enum_item, enum_value};

/// How often the prompts in range are looked for, in seconds.
const PROMPTS_EVERY: f64 = 0.2;

/// The ProximityPrompt shown, and its key's hold.
#[derive(Default)]
pub struct Prompt {
    pub shown: Option<Id>,
    /// When its key went down (a prompt with a HoldDuration), and whether
    /// it triggered since.
    pub held: Option<f64>,
    pub triggered: bool,
    pub next_look: f64,
}

fn alive(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id).is_some_and(|i| !i.destroyed)
}

fn in_place(rbx: &Rbx, id: Id) -> bool {
    alive(rbx, id) && rbx.is_descendant_of(id, rbx.workspace)
}

/// The watched instances of `class` in the Workspace.
fn watched(rbx: &Rbx, class: &str) -> Vec<Id> {
    rbx.tools
        .watched
        .iter()
        .copied()
        .filter(|&i| in_place(rbx, i) && rbx.inst(i).class.name == class)
        .collect()
}

/// Each frame: what is watched still there, the prompts in range, a held
/// prompt's time, and the body movers.
pub fn step(env: &mut dyn Env) {
    let rbx = env.rbx();
    let now = rbx.now;
    let dt = rbx
        .tools
        .last_step
        .replace(now)
        .map_or(0.0, |t| (now - t).clamp(0.0, 0.25));
    let insts = &rbx.insts;
    rbx.tools.watched.retain(|i| insts.get(i).is_some_and(|i| !i.destroyed));
    if rbx.tools.watched.is_empty() {
        hide_prompt(rbx);
        return;
    }
    prompts(rbx);
    prompt_hold(rbx);
    touches(rbx);
    movers(rbx, dt);
}

// ---------------------------------------------------------------- ClickDetectors

/// A click of the left button (or the right): the ClickDetector of what the
/// Mouse points at, within its reach, fires.
pub fn click(env: &mut dyn Env, right: bool) {
    let rbx = env.rbx();
    if !rbx
        .tools
        .watched
        .iter()
        .any(|&i| in_place(rbx, i) && rbx.inst(i).class.name == "ClickDetector")
    {
        return;
    }
    let Some(hit) = super::hstudio_rbx_tools::aim(env, &[]) else {
        return;
    };
    let rbx = env.rbx();
    let target = match hit.part {
        Some(p) => Some(p),
        None => rbx.id_of(&rbx.raw_get(rbx.workspace, "Terrain")),
    };
    let Some(target) = target else { return };
    // The detector in the part, else in the nearest model it is in.
    let mut at = Some(target);
    let mut found = None;
    while let Some(a) = at.filter(|&a| a != rbx.workspace) {
        found = rbx
            .children(a)
            .iter()
            .copied()
            .find(|&c| alive(rbx, c) && rbx.inst(c).class.name == "ClickDetector");
        if found.is_some() {
            break;
        }
        at = rbx.inst(a).parent;
    }
    let Some(cd) = found else { return };
    let head = as_cf(&rbx.raw_get(rbx.head, "CFrame")).unwrap_or_default().position();
    let reach = rbx.raw_get(cd, "MaxActivationDistance").to_num().unwrap_or(32.0);
    if (hit.at - head).magnitude() > reach {
        return;
    }
    let p = rbx.value(rbx.local_player);
    rbx.fire(cd, if right { "RightMouseClick" } else { "MouseClick" }, vec![p]);
}

// ---------------------------------------------------------------- ProximityPrompts

/// Where a prompt is: its part's, its attachment's, or its model's.
fn prompt_at(rbx: &Rbx, prompt: Id) -> Option<Vector3> {
    let parent = rbx.inst(prompt).parent?;
    let class = rbx.inst(parent).class;
    if is_a(class, "BasePart") {
        return as_cf(&rbx.raw_get(parent, "CFrame")).map(CFrame::position);
    }
    if class.name == "Attachment" {
        let part = rbx.inst(parent).parent?;
        let frame = as_cf(&rbx.raw_get(part, "CFrame"))?;
        let local = rbx
            .inst(parent)
            .extra
            .get("Position")
            .and_then(as_v3)
            .unwrap_or_default();
        return Some(frame * local);
    }
    if is_a(class, "Model") {
        let primary = rbx.id_of(&rbx.raw_get(parent, "PrimaryPart"))?;
        return as_cf(&rbx.raw_get(primary, "CFrame")).map(CFrame::position);
    }
    None
}

/// The nearest enabled prompt within its reach of the character, shown;
/// the one shown before hidden.
fn prompts(rbx: &mut Rbx) {
    if rbx.now < rbx.tools.prompt.next_look {
        return;
    }
    rbx.tools.prompt.next_look = rbx.now + PROMPTS_EVERY;
    let me = as_cf(&rbx.raw_get(rbx.root_part, "CFrame"))
        .unwrap_or_default()
        .position();
    let mut best: Option<(f64, Id)> = None;
    for p in watched(rbx, "ProximityPrompt") {
        if !rbx.raw_get(p, "Enabled").truthy() {
            continue;
        }
        let Some(at) = prompt_at(rbx, p) else { continue };
        let d = (at - me).magnitude();
        let reach = rbx.raw_get(p, "MaxActivationDistance").to_num().unwrap_or(10.0);
        if d <= reach && best.is_none_or(|b| d < b.0) {
            best = Some((d, p));
        }
    }
    let now = best.map(|b| b.1);
    if now == rbx.tools.prompt.shown {
        return;
    }
    hide_prompt(rbx);
    let Some(p) = now else { return };
    rbx.tools.prompt.shown = Some(p);
    let keyboard = enum_value(enum_item("ProximityPromptInputType", "Keyboard"));
    rbx.fire(p, "PromptShown", vec![keyboard.clone()]);
    if let Some(s) = rbx.service("ProximityPromptService") {
        let pv = rbx.value(p);
        rbx.fire(s, "PromptShown", vec![pv, keyboard]);
    }
}

fn hide_prompt(rbx: &mut Rbx) {
    let Some(old) = rbx.tools.prompt.shown.take() else {
        return;
    };
    prompt_input(rbx, false);
    rbx.tools.prompt.held = None;
    rbx.tools.prompt.triggered = false;
    if alive(rbx, old) {
        rbx.fire(old, "PromptHidden", Vec::new());
        if let Some(s) = rbx.service("ProximityPromptService") {
            let pv = rbx.value(old);
            rbx.fire(s, "PromptHidden", vec![pv]);
        }
    }
}

/// The game's name for the shown prompt's key (`key.keyboard.e`).
pub fn prompt_key(rbx: &Rbx) -> Option<String> {
    let p = rbx.tools.prompt.shown.filter(|&p| alive(rbx, p))?;
    let code = as_item(&rbx.raw_get(p, "KeyboardKeyCode")).map_or("E", |i| i.name);
    let digits = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
    ];
    let key = match digits.iter().position(|d| *d == code) {
        Some(n) => n.to_string(),
        None if code.len() == 1 => code.to_ascii_lowercase(),
        None => return None,
    };
    Some(format!("key.keyboard.{key}"))
}

/// Fires a prompt's event and ProximityPromptService's (named `service`,
/// with the prompt first), each with the LocalPlayer.
fn prompt_event(rbx: &mut Rbx, p: Id, event: &str, service: &str) {
    let player = rbx.value(rbx.local_player);
    rbx.fire(p, event, vec![player.clone()]);
    if let Some(s) = rbx.service("ProximityPromptService") {
        let pv = rbx.value(p);
        rbx.fire(s, service, vec![pv, player]);
    }
}

/// The shown prompt's key went down or up.
pub fn prompt_input(rbx: &mut Rbx, down: bool) {
    let Some(p) = rbx.tools.prompt.shown.filter(|&p| alive(rbx, p)) else {
        return;
    };
    let hold = rbx.raw_get(p, "HoldDuration").to_num().unwrap_or(0.0);
    if down {
        if rbx.tools.prompt.held.is_some() || rbx.tools.prompt.triggered {
            return;
        }
        if hold > 0.0 {
            rbx.tools.prompt.held = Some(rbx.now);
            prompt_event(rbx, p, "PromptButtonHoldBegan", "PromptButtonHoldBegan");
        } else {
            rbx.tools.prompt.triggered = true;
            prompt_event(rbx, p, "Triggered", "PromptTriggered");
        }
        return;
    }
    if rbx.tools.prompt.held.take().is_some() {
        prompt_event(rbx, p, "PromptButtonHoldEnded", "PromptButtonHoldEnded");
    }
    if std::mem::take(&mut rbx.tools.prompt.triggered) {
        prompt_event(rbx, p, "TriggerEnded", "PromptTriggerEnded");
    }
}

/// A held prompt whose HoldDuration has passed triggers.
fn prompt_hold(rbx: &mut Rbx) {
    let (Some(p), Some(since)) = (rbx.tools.prompt.shown, rbx.tools.prompt.held) else {
        return;
    };
    let hold = rbx.raw_get(p, "HoldDuration").to_num().unwrap_or(0.0);
    if rbx.now - since >= hold {
        rbx.tools.prompt.held = None;
        rbx.tools.prompt.triggered = true;
        prompt_event(rbx, p, "PromptButtonHoldEnded", "PromptButtonHoldEnded");
        prompt_event(rbx, p, "Triggered", "PromptTriggered");
    }
}

/// The prompt shown, on the HUD over the hotbar: its key, ActionText and
/// ObjectText, and how far a hold has gone.
pub fn draw(rbx: &Rbx, game: &mut dyn GameApi) {
    let Some(p) = rbx.tools.prompt.shown.filter(|&p| alive(rbx, p)) else {
        return;
    };
    if !rbx.active || game.screen() != "playing" {
        return;
    }
    let key = as_item(&rbx.raw_get(p, "KeyboardKeyCode")).map_or("E", |i| i.name);
    let action = rbx.raw_get(p, "ActionText").as_text().unwrap_or_default();
    let object = rbx.raw_get(p, "ObjectText").as_text().unwrap_or_default();
    let line = if object.is_empty() {
        format!("[{key}] {action}")
    } else {
        format!("[{key}] {action} {object}")
    };
    let (w, h) = game.gui_size();
    let tw = game.text_width(&line);
    let (x, y) = ((w - tw) / 2.0, h - 70.0);
    game.fill([x - 3.0, y - 2.0, tw + 6.0, 12.0], 0x9000_0000);
    if let Some(since) = rbx.tools.prompt.held {
        let hold = rbx.raw_get(p, "HoldDuration").to_num().unwrap_or(1.0).max(0.01);
        let done = ((rbx.now - since) / hold).clamp(0.0, 1.0) as f32;
        game.fill([x - 3.0, y + 9.0, (tw + 6.0) * done, 1.0], 0xFFFF_FFFF);
    }
    game.text(&line, [x, y], 0xFF_FFFF, true);
}

// ---------------------------------------------------------------- touches

/// A script connected to `part`'s Touched or TouchEnded: it has a
/// TouchTransmitter called TouchInterest, as Roblox gives it.
pub fn touch_interest(rbx: &mut Rbx, part: Id) {
    let Some(inst) = rbx.try_inst(part) else { return };
    if inst.destroyed || !is_a(inst.class, "BasePart") || inst.locked {
        return;
    }
    if rbx
        .children(part)
        .iter()
        .any(|&c| rbx.inst(c).class.name == "TouchTransmitter")
    {
        return;
    }
    let t = rbx.make("TouchTransmitter", None);
    rbx.inst_mut(t).name = "TouchInterest".into();
    let _ = rbx.set_parent(t, Some(part));
}

/// A part's box in the world, as its corners' least and most.
fn bounds(rbx: &Rbx, part: Id) -> Option<(Vector3, Vector3)> {
    let f = as_cf(&rbx.raw_get(part, "CFrame"))?;
    let half = as_v3(&rbx.raw_get(part, "Size")).unwrap_or(Vector3::ONE) * 0.5;
    let mut lo = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = Vector3::new(f64::MIN, f64::MIN, f64::MIN);
    for i in 0..8 {
        let c = f * Vector3::new(
            if i & 1 == 0 { -half.x } else { half.x },
            if i & 2 == 0 { -half.y } else { half.y },
            if i & 4 == 0 { -half.z } else { half.z },
        );
        lo = Vector3::new(lo.x.min(c.x), lo.y.min(c.y), lo.z.min(c.z));
        hi = Vector3::new(hi.x.max(c.x), hi.y.max(c.y), hi.z.max(c.z));
    }
    Some((lo, hi))
}

/// The parts with a TouchInterest the player's box meets fire Touched (with
/// the character's root part) as it comes into them, and TouchEnded as it
/// leaves; the root part's own Touched and TouchEnded fire with them.
fn touches(rbx: &mut Rbx) {
    let root = rbx.root_part;
    let at = as_cf(&rbx.raw_get(root, "CFrame")).unwrap_or_default().position();
    let feet = at.y - rbx.world.root_y;
    let half = super::hstudio_rbx_world::HEIGHT / 6.0;
    let (me_lo, me_hi) = (
        Vector3::new(at.x - half, feet, at.z - half),
        Vector3::new(at.x + half, feet + super::hstudio_rbx_world::HEIGHT, at.z + half),
    );
    let now: Vec<Id> = watched(rbx, "TouchTransmitter")
        .into_iter()
        .filter_map(|t| rbx.inst(t).parent)
        .filter(|&p| {
            is_a(rbx.inst(p).class, "BasePart")
                && rbx.raw_get(p, "CanTouch").truthy()
                && !rbx.is_descendant_of(p, rbx.character)
                && bounds(rbx, p).is_some_and(|(lo, hi)| {
                    lo.x <= me_hi.x
                        && hi.x >= me_lo.x
                        && lo.y <= me_hi.y
                        && hi.y >= me_lo.y
                        && lo.z <= me_hi.z
                        && hi.z >= me_lo.z
                })
        })
        .collect();
    let before = std::mem::take(&mut rbx.tools.touching);
    let rv = rbx.value(root);
    for &p in now.iter().filter(|p| !before.contains(p)) {
        let pv = rbx.value(p);
        rbx.fire(p, "Touched", vec![rv.clone()]);
        rbx.fire(root, "Touched", vec![pv]);
    }
    let left: Vec<Id> = before
        .iter()
        .copied()
        .filter(|p| !now.contains(p) && alive(rbx, *p))
        .collect();
    for p in left {
        let pv = rbx.value(p);
        rbx.fire(p, "TouchEnded", vec![rv.clone()]);
        rbx.fire(root, "TouchEnded", vec![pv]);
    }
    rbx.tools.touching = now;
}

// ---------------------------------------------------------------- models

/// A model's box: GetBoundingBox (its CFrame and size), the older
/// GetModelCFrame and GetModelSize, and GetExtentsSize. The box is the one
/// its parts fill, along the world's axes, at their middle.
pub fn model_box(rbx: &Rbx, model: Id, name: &str) -> Vec<super::hstudio_lua::Value> {
    let mut lo = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut hi = Vector3::new(f64::MIN, f64::MIN, f64::MIN);
    for d in rbx.descendants(model) {
        if !alive(rbx, d) || !is_a(rbx.inst(d).class, "BasePart") {
            continue;
        }
        if let Some((a, b)) = bounds(rbx, d) {
            lo = Vector3::new(lo.x.min(a.x), lo.y.min(a.y), lo.z.min(a.z));
            hi = Vector3::new(hi.x.max(b.x), hi.y.max(b.y), hi.z.max(b.z));
        }
    }
    let (centre, size) = if lo.x <= hi.x {
        ((lo + hi) * 0.5, hi - lo)
    } else {
        let pivot = as_cf(&rbx.raw_get(model, "WorldPivot")).unwrap_or_default().position();
        (pivot, Vector3::ZERO)
    };
    let frame = cf(CFrame::from_position(centre));
    let size = super::hstudio_rbx_values::v3(size);
    match name {
        "GetBoundingBox" => vec![frame, size],
        "GetModelCFrame" => vec![frame],
        _ => vec![size],
    }
}

// ---------------------------------------------------------------- body movers

/// The body movers in the scripts' unanchored parts move them.
fn movers(rbx: &mut Rbx, dt: f64) {
    if dt <= 0.0 {
        return;
    }
    let ch = rbx.character;
    let list: Vec<Id> = rbx
        .tools
        .watched
        .iter()
        .copied()
        .filter(|&m| matches!(rbx.inst(m).class.name, "BodyPosition" | "BodyGyro" | "BodyVelocity"))
        .collect();
    for m in list {
        let Some(part) = rbx.inst(m).parent else { continue };
        let movable = in_place(rbx, part)
            && is_a(rbx.inst(part).class, "BasePart")
            && !rbx.inst(part).locked
            && !rbx.raw_get(part, "Anchored").truthy()
            && part != rbx.root_part
            && !rbx.is_descendant_of(part, ch);
        if !movable {
            continue;
        }
        let Some(frame) = as_cf(&rbx.raw_get(part, "CFrame")) else {
            continue;
        };
        let v3 = |k: &str| as_v3(&rbx.raw_get(m, k)).unwrap_or_default();
        let next = match rbx.inst(m).class.name {
            "BodyPosition" if v3("MaxForce").magnitude() > 0.0 => {
                let to = v3("Position");
                let at = frame.position();
                let t = (dt * 10.0).min(1.0);
                CFrame::from_position(at + (to - at) * t) * frame.rotation()
            }
            "BodyVelocity" if v3("MaxForce").magnitude() > 0.0 => {
                CFrame::from_position(frame.position() + v3("Velocity") * dt) * frame.rotation()
            }
            "BodyGyro" if v3("MaxTorque").magnitude() > 0.0 => {
                let goal = as_cf(&rbx.raw_get(m, "CFrame")).unwrap_or_default();
                CFrame::from_position(frame.position()) * goal.rotation()
            }
            _ => continue,
        };
        let moved = (next.position() - frame.position()).magnitude() > 1e-6
            || (next.look_vector() - frame.look_vector()).magnitude() > 1e-6
            || (next.up_vector() - frame.up_vector()).magnitude() > 1e-6;
        if moved {
            rbx.raw_set(part, "CFrame", cf(next));
        }
    }
}
