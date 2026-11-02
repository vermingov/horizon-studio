// The local character's doings in the game (hstudio_rbx_body): what Roblox
// scripts tell its Humanoid (jump, sit, fall over, stand still, a state of
// their own), what they take off the character (its arms, legs, head, hats
// and clothes), and what turns, pushes or holds up its root part. The game
// gets a real jump, the player's own model posed, spun or with parts hidden
// (hstudio::Body, which only this client sees), and the player held still
// while the Humanoid can't walk. hstudio_rbx_world calls in each frame.

use super::hstudio::{Body, GameApi};
use super::hstudio_lua::Value;
use super::hstudio_rbx::{Id, Rbx, is_a};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, as_v3, enum_def};

/// How long FallingDown keeps the Humanoid down, then getting up, in seconds.
const FALLEN: f64 = 1.5;
const GETTING_UP: f64 = 0.3;
/// How long a jump keeps the Humanoid Jumping while the game catches up.
const JUMP_HOLD: f64 = 0.25;
/// Roblox's FallenPartsDestroyHeight until a script sets one.
const DESTROY: f64 = -500.0;
/// Higher than this (above any world's build limit), or out past the world
/// border, is out of the world: a script that moves the character there
/// (an invisibility script parks the real one) doesn't move the player.
const SKY: f64 = 4096.0;
const BORDER: f64 = 30_000_000.0;
/// The root part's centre above what an R6 character stands on, in studs:
/// its HipHeight (2) and half its root part (1).
const STAND: f64 = 3.0;
/// How fast the player follows a platform that moves under them, in blocks
/// a tick for each stud it is off.
const FOLLOW: f64 = 0.2;
/// The most the body turns in a tick (degrees): faster spins alias.
const MAX_TURN: f32 = 170.0;
/// The rig's own parts, which never hold the player up.
const RIG: [&str; 7] = [
    "HumanoidRootPart",
    "Head",
    "Torso",
    "Left Arm",
    "Right Arm",
    "Left Leg",
    "Right Leg",
];

/// What the local Humanoid is doing beyond what the game says.
pub struct Doings {
    /// A jump to ask of the game this frame, and when the last was asked.
    pub jump: bool,
    pub jumped_at: Option<f64>,
    /// A state a script put the Humanoid in, until when (None: until
    /// another).
    pub forced: Option<(&'static str, Option<f64>)>,
    /// A script laid the seated character over (its root part turned on
    /// its side): it lies.
    pub lying: bool,
    /// The velocity a script gave the root part (studs a second), for the
    /// game this frame; and the player's own, as the root part had it.
    pub velocity: Option<Vector3>,
    pub measured: Vector3,
    /// The jump key held last frame (JumpRequest fires each frame it is).
    pub space: bool,
    /// The walk and jump scales WalkSpeed and JumpPower (or JumpHeight) ask,
    /// and the scales last sent with the holds on them.
    pub scales: (f64, f64),
    pub scales_sent: Option<(f64, f64)>,
    /// The player's model as the game last had it.
    pub sent: Option<Body>,
    /// The last FallenPartsDestroyHeight a script left a number (a script
    /// sets it to NaN while it moves the character below).
    pub destroy: f64,
    /// The player's eyes above their feet this frame.
    pub eyes: f64,
}

impl Default for Doings {
    fn default() -> Self {
        Doings {
            jump: false,
            jumped_at: None,
            forced: None,
            lying: false,
            velocity: None,
            measured: Vector3::ZERO,
            space: false,
            scales: (1.0, 1.0),
            scales_sent: None,
            sent: None,
            destroy: DESTROY,
            eyes: 1.62,
        }
    }
}

impl Doings {
    /// A new character: what the old one was doing goes; the game's state
    /// (what was sent) stays.
    pub fn respawned(&mut self) {
        self.jump = false;
        self.jumped_at = None;
        self.forced = None;
        self.lying = false;
        self.velocity = None;
        self.scales = (1.0, 1.0);
    }
}

fn live(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id).is_some_and(|i| !i.destroyed)
}

/// What the local character wears beyond its parts, as a Roblox avatar
/// does: a Shirt and Pants (the skin's jacket, sleeves and trousers) and a
/// hat (its hat layer), an Accessory whose Handle is welded to the Head.
/// Taking them off takes those layers off the player's model.
pub fn dress(rbx: &mut Rbx, model: Id) {
    for class in ["Shirt", "Pants"] {
        let c = rbx.make(class, None);
        rbx.inst_mut(c).name = class.into();
        let _ = rbx.set_parent(c, Some(model));
    }
    let head = rbx.child_named(model, "Head");
    let hat = rbx.make("Accessory", None);
    rbx.inst_mut(hat).name = "Hat".into();
    let handle = rbx.make("Part", None);
    rbx.inst_mut(handle).name = "Handle".into();
    rbx.raw_set(
        handle,
        "Size",
        super::hstudio_rbx_values::v3(Vector3::new(0.55, 0.55, 0.55)),
    );
    rbx.raw_set(handle, "Transparency", Value::Num(1.0));
    rbx.raw_set(handle, "CanCollide", Value::Bool(false));
    rbx.raw_set(handle, "CanTouch", Value::Bool(false));
    rbx.raw_set(handle, "Massless", Value::Bool(true));
    let _ = rbx.set_parent(handle, Some(hat));
    let weld = rbx.make("Weld", None);
    rbx.inst_mut(weld).name = "AccessoryWeld".into();
    let hv = rbx.value(handle);
    rbx.inst_mut(weld).extra.insert("Part0".into(), hv);
    if let Some(h) = head {
        let v = rbx.value(h);
        rbx.inst_mut(weld).extra.insert("Part1".into(), v);
    }
    let _ = rbx.set_parent(weld, Some(handle));
    let _ = rbx.set_parent(hat, Some(model));
}

/// The hats (Accessories, older Hats) of a Humanoid's character.
pub fn accessories(rbx: &Rbx, humanoid: Id) -> Vec<Id> {
    let Some(model) = rbx.try_inst(humanoid).and_then(|i| i.parent) else {
        return Vec::new();
    };
    rbx.inst(model)
        .children
        .iter()
        .copied()
        .filter(|&c| live(rbx, c) && is_a(rbx.inst(c).class, "Accoutrement"))
        .collect()
}

/// Player:ClearCharacterAppearance: the character's hats, clothes, body
/// colours and meshes go.
pub fn clear_appearance(rbx: &mut Rbx, model: Id) {
    let Some(kids) = rbx.try_inst(model).map(|i| i.children.clone()) else {
        return;
    };
    for c in kids {
        let class = rbx.inst(c).class;
        let worn = [
            "Accoutrement",
            "Clothing",
            "ShirtGraphic",
            "BodyColors",
            "CharacterMesh",
        ];
        if live(rbx, c) && worn.iter().any(|w| is_a(class, w)) {
            rbx.destroy(c);
        }
    }
}

// ---------------------------------------------------------------- states

/// A HumanoidStateType's name as the layer keeps it.
pub fn state_named(name: &str) -> Option<&'static str> {
    enum_def("HumanoidStateType")?
        .items
        .iter()
        .find(|i| i.0 == name)
        .map(|i| i.0)
}

/// Whether the Humanoid may enter `state` (SetStateEnabled).
pub fn enabled(rbx: &Rbx, humanoid: Id, state: &str) -> bool {
    rbx.try_inst(humanoid)
        .and_then(|i| i.extra.get(&format!("StateEnabled:{state}")))
        .is_none_or(Value::truthy)
}

/// The local Humanoid's state this frame, from the one the game's player
/// gives (`computed`) and what scripts made it do: the game's death first,
/// then a state a script put it in (FallingDown gets up after a while),
/// Seated while it sits, PlatformStanding while it stands on its platform,
/// Jumping while a jump it was told to make is on its way; and a state it
/// may not enter (SetStateEnabled) keeps it where it is.
pub fn state(rbx: &mut Rbx, computed: &'static str) -> &'static str {
    let hum = rbx.humanoid;
    let now = rbx.now;
    if computed == "Dead" {
        rbx.world.body.forced = None;
        return computed;
    }
    if let Some((s, until)) = rbx.world.body.forced {
        match until {
            Some(t) if now >= t => {
                if s == "FallingDown" {
                    rbx.world.body.forced = Some(("GettingUp", Some(now + GETTING_UP)));
                    return "GettingUp";
                }
                rbx.world.body.forced = None;
            }
            _ => return s,
        }
    }
    if rbx.raw_get(hum, "Sit").truthy() {
        return "Seated";
    }
    rbx.world.body.lying = false;
    if rbx.raw_get(hum, "PlatformStand").truthy() {
        return "PlatformStanding";
    }
    if let Some(t) = rbx.world.body.jumped_at {
        if now - t < JUMP_HOLD && matches!(computed, "Running" | "Landed") {
            return "Jumping";
        }
        rbx.world.body.jumped_at = None;
    }
    if !enabled(rbx, hum, computed) {
        return rbx.world.state;
    }
    computed
}

/// Humanoid:ChangeState on the local Humanoid (another has no one to move):
/// Jumping jumps (in the air too, as infinite jump scripts have it) unless
/// it may not jump, Dead kills the character, FallingDown knocks it over
/// for a while, Ragdoll, Physics and PlatformStanding hold it until another
/// state, Seated sits. As in Roblox, it goes into a state it may not enter
/// on its own (a swim script turns them all off, then swims).
pub fn change_state(rbx: &mut Rbx, humanoid: Id, state: &str) {
    let Some(state) = state_named(state) else { return };
    if humanoid != rbx.humanoid || rbx.world.state == "Dead" {
        return;
    }
    if state == "Jumping" && !enabled(rbx, humanoid, state) {
        return;
    }
    let now = rbx.now;
    match state {
        "Jumping" => {
            stand_up(rbx);
            jump(rbx);
            rbx.world.body.forced = None;
        }
        "Dead" => {
            kill(rbx);
            return;
        }
        "FallingDown" => rbx.world.body.forced = Some((state, Some(now + FALLEN))),
        "GettingUp" => rbx.world.body.forced = Some((state, Some(now + GETTING_UP))),
        "Ragdoll" | "Physics" | "PlatformStanding" => rbx.world.body.forced = Some((state, None)),
        "Seated" => {
            rbx.world.body.forced = None;
            rbx.raw_set(humanoid, "Sit", Value::Bool(true));
        }
        _ => rbx.world.body.forced = None,
    }
    super::hstudio_rbx_world::set_state(rbx, state, 0.0);
}

/// A jump for the game this frame, the Humanoid Jumping meanwhile.
fn jump(rbx: &mut Rbx) {
    rbx.world.body.jump = true;
    rbx.world.body.jumped_at = Some(rbx.now);
}

/// The character gets off its seat (and up, if it lay).
fn stand_up(rbx: &mut Rbx) {
    let hum = rbx.humanoid;
    if rbx.raw_get(hum, "Sit").truthy() {
        rbx.raw_set(hum, "Sit", Value::Bool(false));
    }
    rbx.world.body.lying = false;
}

/// The local character dies, as Roblox's does when its Humanoid's health
/// goes (Died, the Dead state, a new character Players.RespawnTime later).
/// The player in the game lives on: only the server can kill them.
pub fn kill(rbx: &mut Rbx) {
    let hum = rbx.humanoid;
    if rbx.world.state == "Dead" || rbx.respawn_at.is_some() {
        return;
    }
    rbx.raw_set(hum, "Health", Value::Num(0.0));
    rbx.fire(hum, "HealthChanged", vec![Value::Num(0.0)]);
    super::hstudio_rbx_world::health_set(rbx);
}

/// A script set the local Humanoid's Jump, Sit or PlatformStand. Jump
/// jumps once, as Roblox's Humanoid does on its next step, from the ground
/// (a seated one gets up and jumps), then reads false again; Sit sits,
/// unless the Humanoid may not be Seated.
pub fn humanoid_set(rbx: &mut Rbx, prop: &str) {
    let hum = rbx.humanoid;
    let on = rbx.raw_get(hum, prop).truthy();
    match prop {
        "Jump" if on => {
            rbx.quiet_set(hum, "Jump", Value::Bool(false));
            let seated = rbx.raw_get(hum, "Sit").truthy();
            stand_up(rbx);
            let free = rbx.world.body.forced.is_none() && !rbx.raw_get(hum, "PlatformStand").truthy();
            if free && (seated || rbx.world.on_ground) && enabled(rbx, hum, "Jumping") {
                jump(rbx);
            }
        }
        "Sit" if on && !enabled(rbx, hum, "Seated") => rbx.quiet_set(hum, "Sit", Value::Bool(false)),
        "Sit" if !on => rbx.world.body.lying = false,
        _ => {}
    }
}

/// A script set the local root part's Velocity (or AssemblyLinearVelocity):
/// the player goes that fast this frame. Setting it to what it reads (a
/// script keeping it) changes nothing.
pub fn root_velocity(rbx: &mut Rbx, prop: &str) {
    let Some(v) = as_v3(&rbx.raw_get(rbx.root_part, prop)) else {
        return;
    };
    if (v - rbx.world.body.measured).magnitude() < 0.05 {
        return;
    }
    rbx.world.body.velocity = Some(v);
}

/// A script moved the local root part to `to`, the player's feet to
/// `feet`: whether the player goes there. Below Workspace's
/// FallenPartsDestroyHeight Roblox destroys the character (it dies, as a
/// respawn script means it to; the player stays); out of the world (far
/// up, past the border) the player isn't moved. Turned on its side while
/// seated, the character lies down.
pub fn root_moved(rbx: &mut Rbx, to: CFrame, feet: Vector3) -> bool {
    let set = rbx.raw_get(rbx.workspace, "FallenPartsDestroyHeight").to_num();
    if let Some(h) = set.filter(|h| h.is_finite()) {
        rbx.world.body.destroy = h;
    }
    if !feet.x.is_finite() || !feet.y.is_finite() || !feet.z.is_finite() {
        return false;
    }
    if feet.y < rbx.world.body.destroy {
        kill(rbx);
        return false;
    }
    if feet.y > SKY || feet.x.abs() > BORDER || feet.z.abs() > BORDER {
        return false;
    }
    if to.up_vector().y.abs() < 0.5 && rbx.raw_get(rbx.humanoid, "Sit").truthy() {
        rbx.world.body.lying = true;
    }
    true
}

/// A script moved the local Head (a cframe fly script flies it): the whole
/// character goes with it, as Roblox moves a part's assembly (the Neck
/// holds it to the torso), the player's eyes where the head is.
pub fn head_moved(rbx: &mut Rbx) {
    if rbx.head == rbx.spare_part {
        return;
    }
    let Some(to) = as_cf(&rbx.raw_get(rbx.head, "CFrame")) else {
        return;
    };
    let feet = to.position() - Vector3::new(0.0, rbx.world.body.eyes, 0.0);
    if root_moved(rbx, CFrame::from_position(feet), feet) {
        rbx.world.teleport = Some(feet);
    }
}

/// The way the movement keys held walk the player (W, A, S and D, as the
/// game's own are), flat and a unit long, as Roblox's MoveDirection is: None
/// while none is held (or a TextBox takes the keys).
pub fn input_direction(rbx: &Rbx, yaw: f32) -> Option<Vector3> {
    let held = |k: &str| f64::from(u8::from(rbx.pointer.held.contains(k)));
    let (ahead, right) = (held("W") - held("S"), held("D") - held("A"));
    if (ahead == 0.0 && right == 0.0) || rbx.pointer.focus.is_some() {
        return None;
    }
    let yaw = f64::from(yaw).to_radians();
    let forward = Vector3::new(-yaw.sin(), 0.0, yaw.cos());
    let to_right = Vector3::new(-yaw.cos(), 0.0, -yaw.sin());
    let way = forward * ahead + to_right * right;
    Some(way / way.magnitude())
}

// ---------------------------------------------------------------- each frame

/// Each frame, before the state: the player's velocity as the root part
/// has it, and the jump key, which (as Roblox's controls do while it is
/// held) gets a seated character up and fires JumpRequest again each frame.
pub fn frame(rbx: &mut Rbx) {
    rbx.world.body.measured = as_v3(&rbx.raw_get(rbx.root_part, "Velocity")).unwrap_or_default();
    let held = rbx.pointer.held.contains("Space") && rbx.pointer.focus.is_none();
    if held {
        stand_up(rbx);
        if rbx.world.body.space {
            let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
            rbx.fire(uis, "JumpRequest", Vec::new());
        }
    }
    rbx.world.body.space = held;
}

/// What the Humanoid's state holds back: walking (seated, standing on its
/// platform, fallen over) and jumping (all but seated, and while it may
/// not jump).
fn holds(rbx: &Rbx) -> (bool, bool) {
    let (walk, jump) = match rbx.world.state {
        "Seated" => (true, false),
        "PlatformStanding" | "FallingDown" | "GettingUp" | "Ragdoll" | "Physics" => (true, true),
        _ => (false, false),
    };
    (walk, jump || !enabled(rbx, rbx.humanoid, "Jumping"))
}

/// What the character asks of the game this frame (Studio::roblox_effects):
/// the speeds WalkSpeed and JumpPower ask with what the state holds back, a
/// jump, a velocity given to the root part (unless a body mover moves it),
/// a platform holding the player up, and how the player's model looks.
pub fn effects(rbx: &mut Rbx, game: &mut dyn GameApi) {
    let asked = rbx.world.speed_request.take();
    if let Some(s) = asked {
        rbx.world.body.scales = s;
    }
    let (walk_held, jump_held) = holds(rbx);
    let (walk, jump) = rbx.world.body.scales;
    let want = (if walk_held { 0.0 } else { walk }, if jump_held { 0.0 } else { jump });
    let send = match rbx.world.body.scales_sent {
        Some(s) => s != want || asked.is_some(),
        None => asked.is_some() || want != (1.0, 1.0),
    };
    if send {
        rbx.world.body.scales_sent = Some(want);
        game.set_player_speed(want.0, want.1);
    }
    if std::mem::take(&mut rbx.world.body.jump) {
        game.jump_player();
    }
    if let Some(v) = rbx.world.body.velocity.take()
        && super::hstudio_rbx_world::mover_velocity(rbx).is_none()
    {
        // Studs a second are blocks a second here; the game counts ticks.
        game.set_player_velocity([v.x / 20.0, v.y / 20.0, v.z / 20.0]);
    }
    if let Some(vy) = platform(rbx) {
        game.set_player_rise(vy);
    }
    if rbx.active {
        let body = look(rbx);
        let fresh = match &rbx.world.body.sent {
            Some(b) => *b != body,
            None => body != Body::default(),
        };
        if fresh {
            game.set_body(&body);
            rbx.world.body.sent = Some(body);
        }
    }
}

/// How fast the player rises standing on a platform a script keeps under
/// the character (a float script's anchored part in it, under the root
/// part): held where the platform has them, following it down or up as it
/// moves (blocks a tick), free to jump off it. None without one, or while
/// the player rises faster.
fn platform(rbx: &Rbx) -> Option<f64> {
    let model = rbx.character;
    let root = as_cf(&rbx.raw_get(rbx.root_part, "CFrame"))?.position();
    let kids = rbx.try_inst(model)?.children.clone();
    for c in kids {
        let Some(i) = rbx.try_inst(c).filter(|i| !i.destroyed) else {
            continue;
        };
        if !is_a(i.class, "BasePart") || RIG.contains(&i.name.as_str()) {
            continue;
        }
        if !i.get("Anchored").truthy() || !i.get("CanCollide").truthy() {
            continue;
        }
        let (Some(at), Some(size)) = (as_cf(&i.get("CFrame")), as_v3(&i.get("Size"))) else {
            continue;
        };
        let p = at.position();
        let top = p.y + size.y / 2.0;
        let under = (p.x - root.x).abs() <= size.x / 2.0 + 1.0 && (p.z - root.z).abs() <= size.z / 2.0 + 1.0;
        if !under || !(1.0..=6.0).contains(&(root.y - top)) {
            continue;
        }
        let want = ((top - (root.y - STAND)) * FOLLOW).clamp(-1.0, 1.0);
        let now = rbx.world.body.measured.y / 20.0;
        return (now < want - 1e-3).then_some(want);
    }
    None
}

/// The player's model as the local character has it: a part the game
/// draws while the character has it and leaves it invisible (a part a
/// script makes visible is drawn as the Roblox part it is, a ghost of the
/// character in its place); the hat layer while a hat is welded on, the
/// jacket and sleeves with a Shirt, the trousers with Pants. Out of the
/// Workspace, or with none of its parts, nothing of it is drawn. Seated it
/// sits (lies, laid over), fallen over it lies; a BodyAngularVelocity in
/// the root part spins it; with its Animate script disabled the limbs keep
/// still, and with AutoRotate off the body doesn't turn to the walk.
pub fn look(rbx: &Rbx) -> Body {
    let model = rbx.character;
    let mut body = Body::default();
    // With no Character (a god script leaves it nil), the old one stays as it was.
    if model == rbx.spare_model {
        return body;
    }
    let placed = live(rbx, model) && rbx.is_descendant_of(model, rbx.workspace);
    if !placed {
        body.gone = true;
        return body;
    }
    let kids: Vec<Id> = rbx
        .inst(model)
        .children
        .iter()
        .copied()
        .filter(|&c| live(rbx, c))
        .collect();
    let part = |name: &str| {
        kids.iter()
            .any(|&c| rbx.inst(c).name == name && is_a(rbx.inst(c).class, "BasePart") && drawn_by_game(rbx, c))
    };
    let wears = |class: &str| kids.iter().any(|&c| is_a(rbx.inst(c).class, class));
    let head = part("Head");
    let torso = part("Torso");
    let arms = [part("Left Arm"), part("Right Arm")];
    let legs = [part("Left Leg"), part("Right Leg")];
    let hat = head && kids.iter().any(|&c| welded_hat(rbx, c));
    let shirt = wears("Shirt");
    let pants = wears("Pants");
    let shown = [
        ("head", head),
        ("hat", hat),
        ("body", torso),
        ("jacket", torso && shirt),
        ("left_arm", arms[0]),
        ("right_arm", arms[1]),
        ("left_sleeve", arms[0] && shirt),
        ("right_sleeve", arms[1] && shirt),
        ("left_leg", legs[0]),
        ("right_leg", legs[1]),
        ("left_pants", legs[0] && pants),
        ("right_pants", legs[1] && pants),
    ];
    body.hidden = shown.iter().filter(|s| !s.1).map(|s| s.0).collect();
    if !(head || torso || arms.contains(&true) || legs.contains(&true)) {
        body.gone = true;
    }
    body.pose = match rbx.world.state {
        "FallingDown" | "Ragdoll" | "Physics" => "lying",
        "Seated" if rbx.world.body.lying => "lying",
        "Seated" => "sitting",
        _ => "",
    };
    body.spin = spin(rbx);
    body.still = kids.iter().any(|&c| {
        let i = rbx.inst(c);
        i.name == "Animate" && is_a(i.class, "BaseScript") && (i.get("Disabled").truthy() || !i.get("Enabled").truthy())
    });
    body.fixed = !rbx.raw_get(rbx.humanoid, "AutoRotate").truthy() && rbx.humanoid != rbx.spare_humanoid;
    body
}

/// Whether the game draws a part of the character: invisible as the layer
/// made it, the game's model in its place.
fn drawn_by_game(rbx: &Rbx, part: Id) -> bool {
    rbx.raw_get(part, "Transparency").to_num().unwrap_or(0.0) >= 0.999
}

/// Whether `id` is a hat still on: an Accessory (or an older Hat) whose
/// Handle is welded to the character.
fn welded_hat(rbx: &Rbx, id: Id) -> bool {
    if !is_a(rbx.inst(id).class, "Accoutrement") {
        return false;
    }
    let Some(handle) = rbx.child_named(id, "Handle").filter(|&h| live(rbx, h)) else {
        return false;
    };
    rbx.inst(handle).children.iter().any(|&j| {
        live(rbx, j) && {
            let c = rbx.inst(j).class;
            is_a(c, "JointInstance") || is_a(c, "WeldConstraint")
        }
    })
}

/// How fast a BodyAngularVelocity (or an enabled AngularVelocity) in the
/// local root part turns the body, in the game's degrees a second (its yaw
/// grows the other way round to Roblox's Y axis), as fast as a tick allows.
fn spin(rbx: &Rbx) -> f32 {
    let Some(root) = rbx.try_inst(rbx.root_part) else {
        return 0.0;
    };
    for &c in &root.children {
        let Some(i) = rbx.try_inst(c).filter(|i| !i.destroyed) else {
            continue;
        };
        let v3 = |k: &str| as_v3(&i.get(k)).unwrap_or_default();
        let turning = match i.class.name {
            "BodyAngularVelocity" if v3("MaxTorque").y > 0.0 => v3("AngularVelocity").y,
            "AngularVelocity" if i.get("Enabled").truthy() => v3("AngularVelocity").y,
            _ => continue,
        };
        let limit = MAX_TURN * super::hstudio_rbx_world::TICKS as f32;
        return (-(turning.to_degrees()) as f32).clamp(-limit, limit);
    }
    0.0
}
