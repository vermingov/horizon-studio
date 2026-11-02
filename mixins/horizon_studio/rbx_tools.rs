// The Roblox layer's tools (hstudio_rbx_tools): the LocalPlayer's Backpack
// is the player's inventory. Each stack in it (the hotbar's 9 slots and the
// 27 above them) is a Tool named after its item, with ItemId, Count and Slot
// (1 to 36, 1 to 9 the hotbar) attributes and a Handle; the selected hotbar
// slot's Tool is in the character, equipped, as Roblox has a tool in hand.
// Each frame the Tools follow the game, as the rest of the world does: a
// stack that comes, goes or changes, and the player choosing another slot,
// which moves the Tools (Unequipped, then Equipped). Scripts move them the
// other way: a Tool put in the character (`tool.Parent = character`,
// Humanoid:EquipTool) selects its slot; the one held taken out of it
// (Humanoid:UnequipTools) selects an empty slot, or holds nothing as far as
// the layer goes when there is none; one put in the Workspace is dropped
// (the whole stack, as Ctrl+Q); and Tool:Activate uses the item, as a right
// click does. The player using the item held (the right button, the game's
// use) fires its Tool's Activated, and Deactivated as they let go. A Tool of
// a stack a script destroys leaves the Backpack until the stack changes: the
// item stays in the game (the client can't take it away).
//
// The Tools and HopperBins scripts make (BTools' bins, F3X, IY's teleport
// tool) aren't items: they are the studio's own slots after the hotbar's,
// drawn beside it. 0 equips the first one (and the next, again), and the
// wheel goes on past the hotbar's ends into them; 1 to 9 go back to the
// game's slots. While one is held the mouse's buttons are its, not the
// game's (the bridge keeps them from the game): its Activated and
// Deactivated, the Mouse's Button1Down and Button1Up. The HopperBins of
// BinType Hammer, Clone and Grab do as Roblox's did: the Hammer deletes the
// part clicked, the Clone copies it on top of itself, the Grab drags it
// along the Mouse. On the world's blocks (the Terrain) they change the
// client's copy of a server's world only, as a Roblox client's own changes
// are: the Hammer breaks the block, the Clone puts a copy of it on the face
// clicked, the Grab lifts it and puts it down where the button is let go.
// An entity's mirror the Hammer hits leaves the client's view of the world.

use super::hstudio::{GameApi, Owner};
use super::hstudio_lua::{Lua, Res, Value};
use super::hstudio_rbx::{Env, Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, as_item, as_v3, cf, v3};
use super::hstudio_rbx_world::{set_attribute, title};
use std::collections::VecDeque;

/// The inventory's slots, and the hotbar's (the first ones).
pub const SLOTS: usize = 36;
pub const HOTBAR: usize = 9;
/// A stack's Tool's Handle, in blocks.
const HANDLE: Vector3 = Vector3::new(0.25, 0.25, 0.6);
/// The hand under the right arm's centre (half the arm's 12 pixels).
const HAND_Y: f64 = 0.35;
/// The most asks waiting (a loop equipping tools can't pile them up).
const MAX_ASKS: usize = 64;

/// The Backpack and the tools, between frames.
#[derive(Default)]
pub struct Tools {
    /// Each slot's stack as the layer last saw it, and its Tool (empty
    /// until the game says what the inventory holds).
    pub stacks: Vec<Option<Stack>>,
    /// The game's selected hotbar slot as the layer last left it: another
    /// is the player's choice.
    pub selected: Option<usize>,
    pub held: Held,
    /// What scripts asked of the hotbar, done in order: a slot is selected
    /// at once; a use or a drop waits for a game tick after its slot is
    /// selected (`waiting`), so a server knows which slot it is.
    pub asks: VecDeque<Ask>,
    pub waiting: Option<u64>,
    /// While the layer moves tools itself: their events fire, but nothing
    /// is asked of the game.
    pub quiet: bool,
    /// The scripts' own tools, in the order they came: their slots.
    pub order: Vec<Id>,
    /// The left button's presses and releases since the last step: for the
    /// Handles they are on, else the HopperBin held.
    pub presses: Vec<bool>,
    pub grab: Option<Grab>,
    /// Handles or ArcHandles being dragged.
    pub handle: Option<HandleDrag>,
    /// Draggers between MouseDown and MouseUp, and what they drag.
    pub draggers: Vec<(Id, Drag)>,
    /// Where the camera was and looked at the last step: as it turns with
    /// a script's tool in hand, the Mouse moves (its Move fires).
    pub view: Option<(Vector3, Vector3)>,
    /// The right button's presses since the last step (ClickDetectors'
    /// RightMouseClick).
    pub rights: Vec<bool>,
    /// The ClickDetectors, ProximityPrompts and body movers put in the
    /// place (hstudio_rbx_parts).
    pub watched: Vec<Id>,
    /// The ProximityPrompt shown, and its key's hold.
    pub prompt: super::hstudio_rbx_parts::Prompt,
    /// When the step last ran, for the body movers.
    pub last_step: Option<f64>,
    /// The parts with a TouchInterest the character is in.
    pub touching: Vec<Id>,
}

/// The classes `watched` keeps.
const WATCHED: &[&str] = &[
    "ClickDetector",
    "ProximityPrompt",
    "BodyPosition",
    "BodyGyro",
    "BodyVelocity",
    "TouchTransmitter",
];

/// An instance put somewhere (Rbx::set_parent): a ClickDetector, a
/// ProximityPrompt or a body mover is watched from then on.
pub fn watch(rbx: &mut Rbx, id: Id) {
    let class = rbx.inst(id).class.name;
    if WATCHED.contains(&class) && !rbx.tools.watched.contains(&id) {
        rbx.tools.watched.push(id);
    }
}

/// A press on a Handles' face (`Handles`, its NormalId) or an ArcHandles'
/// ring (its Axis): where the pointer was along it as it went down, and
/// what MouseDrag last said.
#[derive(Clone, Debug)]
pub struct HandleDrag {
    pub handles: Id,
    pub arc: bool,
    /// The NormalId or the Axis.
    pub which: &'static str,
    /// The adornee's centre and the direction (the face's normal, the
    /// ring's axis), in the world.
    pub centre: Vector3,
    pub dir: Vector3,
    /// Along the face's line: where it was grabbed. On the ring: the
    /// vector from the centre to where it was grabbed.
    pub start: f64,
    pub from: Vector3,
    pub last: (f64, f64),
}

/// What a Dragger drags: the parts, the part held and the point on it held
/// (in its own space).
#[derive(Clone, Debug)]
pub struct Drag {
    pub parts: Vec<Id>,
    pub part: Id,
    pub point: Vector3,
}

/// A stack and the Tool that is it.
pub struct Stack {
    pub tool: Id,
    pub item: String,
    pub count: u32,
}

/// What the character holds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Held {
    /// The selected hotbar slot's stack (if any).
    #[default]
    Slot,
    /// A script's tool.
    Script(Id),
    /// Nothing: a script unequipped the stack held and no slot is empty.
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    Select(usize),
    Use(usize),
    Drop(usize),
    Unequip,
}

/// What a Grab bin holds: a part, or a block of the world lifted from
/// where it was.
#[derive(Clone, Debug, PartialEq)]
pub enum Grab {
    Part(Id),
    Block(String, [i32; 3]),
}

/// Whether `id` is a BackpackItem: a Tool, a HopperBin or a Flag.
pub fn is_tool(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id)
        .is_some_and(|i| matches!(i.class.name, "Tool" | "HopperBin" | "Flag"))
}

fn alive(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id).is_some_and(|i| !i.destroyed)
}

/// The LocalPlayer's Backpack.
pub fn backpack(rbx: &Rbx) -> Option<Id> {
    rbx.children(rbx.local_player)
        .iter()
        .copied()
        .find(|&c| rbx.inst(c).class.name == "Backpack")
}

/// The slot whose stack `tool` is.
pub fn slot_of(rbx: &Rbx, tool: Id) -> Option<usize> {
    rbx.tools
        .stacks
        .iter()
        .position(|s| s.as_ref().is_some_and(|s| s.tool == tool))
}

fn in_character(rbx: &Rbx, id: Id) -> bool {
    alive(rbx, id) && rbx.inst(id).parent == Some(rbx.character)
}

/// The scripts' own tools in the Backpack or the character, in their slots'
/// order.
pub fn own_tools(rbx: &Rbx) -> Vec<Id> {
    rbx.tools
        .order
        .iter()
        .copied()
        .filter(|&t| own_tool_here(rbx, t))
        .collect()
}

fn own_tool_here(rbx: &Rbx, t: Id) -> bool {
    let bp = backpack(rbx);
    alive(rbx, t)
        && is_tool(rbx, t)
        && slot_of(rbx, t).is_none()
        && rbx.inst(t).parent.is_some_and(|p| Some(p) == bp || p == rbx.character)
}

/// The scripts' tool or the stack's held, if it is in the character.
pub fn held_tool(rbx: &Rbx) -> Option<Id> {
    let t = match rbx.tools.held {
        Held::Script(t) => t,
        Held::Slot => rbx
            .tools
            .selected
            .and_then(|s| rbx.tools.stacks.get(s))
            .and_then(|s| s.as_ref())
            .map(|s| s.tool)?,
        Held::Nothing => return None,
    };
    in_character(rbx, t).then_some(t)
}

/// Whether the mouse's buttons are a script's tool's now, not the game's.
pub fn takes_clicks(rbx: &Rbx) -> bool {
    rbx.active && matches!(rbx.tools.held, Held::Script(t) if in_character(rbx, t))
}

fn ask(rbx: &mut Rbx, a: Ask) {
    if rbx.tools.asks.len() < MAX_ASKS {
        rbx.tools.asks.push_back(a);
    }
}

// ---------------------------------------------------------------- moving tools

/// A tool moved (Rbx::set_parent): into the character it is equipped
/// (Equipped with the Mouse, a HopperBin's Selected and Active), out of it
/// unequipped. A script's move is passed on to the game: a stack's Tool put
/// in the character selects its slot, the one held taken out unequips it,
/// and one put in the Workspace is dropped.
pub fn moved(rbx: &mut Rbx, id: Id, old: Option<Id>, new: Option<Id>) {
    let ch = rbx.character;
    let (was, now) = (old == Some(ch), new == Some(ch));
    let bin = rbx.inst(id).class.name == "HopperBin";
    if was && !now {
        if bin {
            rbx.raw_set(id, "Active", Value::Bool(false));
            rbx.fire(id, "Deselected", Vec::new());
        } else {
            rbx.fire(id, "Unequipped", Vec::new());
        }
        if rbx.tools.held == Held::Script(id) {
            put_back_grab(rbx);
        }
    }
    if now && !was {
        let mouse = rbx.value(rbx.mouse);
        if bin {
            rbx.raw_set(id, "Active", Value::Bool(true));
            rbx.fire(id, "Selected", vec![mouse]);
        } else {
            rbx.fire(id, "Equipped", vec![mouse]);
        }
    }
    if rbx.tools.quiet {
        return;
    }
    let slot = slot_of(rbx, id);
    if now {
        match slot {
            Some(s) if s < HOTBAR => {
                ask(rbx, Ask::Select(s));
                rbx.tools.held = Held::Slot;
            }
            // A stack above the hotbar can't be in hand.
            Some(_) => {}
            None => rbx.tools.held = Held::Script(id),
        }
        return;
    }
    let ws = rbx.workspace;
    let into_world = new.is_some_and(|p| p == ws || rbx.is_descendant_of(p, ws) && !rbx.is_descendant_of(p, ch));
    if let Some(s) = slot.filter(|&s| s < HOTBAR && into_world) {
        ask(rbx, Ask::Drop(s));
        rbx.tools.held = Held::Slot;
        return;
    }
    if !was {
        return;
    }
    let holding = match rbx.tools.held {
        Held::Script(t) => t == id,
        Held::Slot => slot.is_some() && slot == rbx.tools.selected,
        Held::Nothing => false,
    };
    if holding {
        if slot.is_some() {
            ask(rbx, Ask::Unequip);
        } else {
            rbx.tools.held = Held::Nothing;
        }
    }
}

/// Moves `id` under `parent` as the layer's own move: its events fire, but
/// nothing is asked of the game.
fn quiet_parent(rbx: &mut Rbx, id: Id, parent: Option<Id>) {
    let was = std::mem::replace(&mut rbx.tools.quiet, true);
    let _ = rbx.set_parent(id, parent);
    rbx.tools.quiet = was;
}

/// Every tool in the character back into the Backpack, as Roblox's
/// Humanoid does before it equips another.
fn unequip_all(rbx: &mut Rbx) {
    let Some(bp) = backpack(rbx) else { return };
    let ch = rbx.character;
    let held: Vec<Id> = rbx.children(ch).iter().copied().filter(|&c| is_tool(rbx, c)).collect();
    for t in held {
        quiet_parent(rbx, t, Some(bp));
    }
}

/// The player chose hotbar slot `slot`: its stack's Tool is in hand, and
/// nothing else.
fn hold_slot(rbx: &mut Rbx, slot: usize) {
    rbx.tools.selected = Some(slot);
    rbx.tools.held = Held::Slot;
    unequip_all(rbx);
    let bp = backpack(rbx);
    let tool = rbx.tools.stacks.get(slot).and_then(|s| s.as_ref()).map(|s| s.tool);
    if let Some(t) = tool.filter(|&t| alive(rbx, t) && rbx.inst(t).parent == bp) {
        let ch = rbx.character;
        quiet_parent(rbx, t, Some(ch));
    }
}

/// The player chose a script's tool: it is in hand, and nothing else.
fn hold_own(rbx: &mut Rbx, tool: Id) {
    unequip_all(rbx);
    let ch = rbx.character;
    quiet_parent(rbx, tool, Some(ch));
    rbx.tools.held = Held::Script(tool);
}

/// 0: the first of the scripts' tools, or the next after the one held.
fn next_own(rbx: &mut Rbx) {
    let own = own_tools(rbx);
    if own.is_empty() {
        return;
    }
    let next = match rbx.tools.held {
        Held::Script(t) => own.iter().position(|&o| o == t).map_or(0, |i| (i + 1) % own.len()),
        _ => 0,
    };
    hold_own(rbx, own[next]);
}

/// A wheel step while playing (`steps` > 0 goes left, as the game's
/// hotbar does), the game's selected slot `slot`: whether the scripts'
/// tools take it. Past the hotbar's ends it goes into their slots, and
/// through them; out of them it goes back to the hotbar: Some(Some(slot))
/// asks the game to select that slot. A step within the hotbar is the
/// game's.
pub fn wheel(rbx: &mut Rbx, steps: f64, slot: usize) -> Option<Option<usize>> {
    if !rbx.active || steps == 0.0 {
        return None;
    }
    let own = own_tools(rbx);
    if own.is_empty() {
        return None;
    }
    let n = (HOTBAR + own.len()) as isize;
    let at = match rbx.tools.held {
        Held::Script(t) => own.iter().position(|&o| o == t).map_or(slot, |i| HOTBAR + i),
        _ => slot.min(HOTBAR - 1),
    } as isize;
    let next = at + if steps > 0.0 { -1 } else { 1 };
    let to = next.rem_euclid(n) as usize;
    if at < HOTBAR as isize && to < HOTBAR && next == to as isize {
        return None;
    }
    if to >= HOTBAR {
        hold_own(rbx, own[to - HOTBAR]);
        // The game's slot stays as it is: not a choice of the player's.
        rbx.tools.selected = Some(slot.min(HOTBAR - 1));
        return Some(None);
    }
    // Back to the game's slot: the next frame finds it the player's choice.
    rbx.tools.selected = None;
    Some(Some(to))
}

/// The left button, as the Mouse has it (the crosshair's, or the free
/// pointer's; `processed`: over the GUI, or the game's own): a press is
/// for the Handles it is on, else the HopperBin held (`step` sees to both);
/// a script's Tool held is Activated, and Deactivated as it comes up.
pub fn button1(rbx: &mut Rbx, down: bool, processed: bool) {
    if !rbx.active || (processed && down) {
        return;
    }
    if rbx.tools.presses.len() < 16 {
        rbx.tools.presses.push(down);
    }
    let Held::Script(t) = rbx.tools.held else { return };
    if !in_character(rbx, t) || rbx.inst(t).class.name != "Tool" {
        return;
    }
    if !down {
        rbx.fire(t, "Deactivated", Vec::new());
    } else if rbx.raw_get(t, "Enabled").truthy() && !rbx.raw_get(t, "ManualActivationOnly").truthy() {
        rbx.fire(t, "Activated", Vec::new());
    }
}

/// A key or button as the game reports it while playing (`processed`: the
/// game or the studio had it). The right button is the stack's use: its
/// Activated as it goes down, Deactivated as it comes up. 0 equips the
/// scripts' tools in turn; 1 to 9, the game's own slots, give the hotbar
/// back.
pub fn input(rbx: &mut Rbx, key: &str, down: bool, processed: bool) {
    if !rbx.active || (processed && down) {
        return;
    }
    if super::hstudio_rbx_parts::prompt_key(rbx).as_deref() == Some(key) {
        super::hstudio_rbx_parts::prompt_input(rbx, down);
        return;
    }
    match key {
        "key.mouse.right" => {
            if down && rbx.tools.rights.len() < 16 {
                rbx.tools.rights.push(true);
            }
            if rbx.tools.held != Held::Slot {
                return;
            }
            let Some(t) = held_tool(rbx) else { return };
            if !down {
                rbx.fire(t, "Deactivated", Vec::new());
            } else if rbx.raw_get(t, "Enabled").truthy() {
                rbx.fire(t, "Activated", Vec::new());
            }
        }
        "key.keyboard.0" if down => next_own(rbx),
        k if down && hotbar_key(k) => rbx.tools.selected = None,
        _ => {}
    }
}

fn hotbar_key(k: &str) -> bool {
    k.strip_prefix("key.keyboard.")
        .is_some_and(|d| d.len() == 1 && ('1'..='9').contains(&d.chars().next().unwrap_or('0')))
}

// ---------------------------------------------------------------- methods

/// Humanoid:EquipTool and UnequipTools, Tool:Activate and Deactivate,
/// HopperBin:ToggleSelect and Disable.
pub fn method(env: &mut dyn Env, lua: &mut Lua, id: Id, name: &str, args: &[Value]) -> Res {
    let rbx = env.rbx();
    match name {
        "EquipTool" => {
            let tool = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .filter(|&t| is_tool(rbx, t))
                .ok_or_else(|| lua.rt_error("Unable to cast value to Object"))?;
            equip(rbx, id, tool).map_err(|e| lua.rt_error(e))?;
        }
        "UnequipTools" => {
            let model = rbx.inst(id).parent;
            if id == rbx.humanoid && model == Some(rbx.character) {
                let ch = rbx.character;
                let bp = backpack(rbx);
                let held: Vec<Id> = rbx.children(ch).iter().copied().filter(|&c| is_tool(rbx, c)).collect();
                for t in held {
                    let _ = rbx.set_parent(t, bp);
                }
            }
        }
        "Activate" => activate(rbx, id),
        "Deactivate" => {
            if rbx.inst(id).parent.is_some() {
                rbx.fire(id, "Deactivated", Vec::new());
            }
        }
        "ToggleSelect" => {
            if in_character(rbx, id) {
                let bp = backpack(rbx);
                let _ = rbx.set_parent(id, bp);
            } else {
                let hum = rbx.humanoid;
                equip(rbx, hum, id).map_err(|e| lua.rt_error(e))?;
            }
        }
        "Disable" => {
            if in_character(rbx, id) {
                let bp = backpack(rbx);
                let _ = rbx.set_parent(id, bp);
            }
        }
        _ => return Err(lua.rt_error(format!("{name} is not available here"))),
    }
    Ok(Vec::new())
}

/// Humanoid:EquipTool: the tools in the Humanoid's character go back to the
/// Backpack, and `tool` goes in (the local character: its slot is selected,
/// or a script's tool is held). Another Humanoid's model just takes it.
fn equip(rbx: &mut Rbx, humanoid: Id, tool: Id) -> Result<(), String> {
    let Some(model) = rbx.inst(humanoid).parent else {
        return Ok(());
    };
    if humanoid == rbx.humanoid && model == rbx.character {
        if rbx.inst(tool).parent == Some(model) {
            return Ok(());
        }
        unequip_all(rbx);
    }
    rbx.set_parent(tool, Some(model))
}

/// Tool:Activate: as a click of the tool held, Activated (not while it is
/// disabled or in no character); a stack's Tool uses its item, as a right
/// click does.
fn activate(rbx: &mut Rbx, tool: Id) {
    let in_a_character = rbx
        .inst(tool)
        .parent
        .is_some_and(|p| rbx.children(p).iter().any(|&c| rbx.inst(c).class.name == "Humanoid"));
    if !in_a_character || !rbx.raw_get(tool, "Enabled").truthy() {
        return;
    }
    rbx.fire(tool, "Activated", Vec::new());
    if let Some(s) = slot_of(rbx, tool).filter(|&s| s < HOTBAR) {
        ask(rbx, Ask::Use(s));
    }
}

// ---------------------------------------------------------------- each frame

/// The tools brought up to the game, each frame the layer runs: the
/// Backpack's stacks follow the inventory, the player's choice of slot
/// moves the Tools, the scripts' asks reach the game, the HopperBins act,
/// and the Handles of the tools in hand are at the right hand.
pub fn step(env: &mut dyn Env) {
    let (rbx, game) = env.rbx_and_game();
    if !rbx.active {
        return;
    }
    if let Some(p) = game.player()
        && let Some(inv) = game.inventory()
    {
        mirror(rbx, &inv, p.slot.min(HOTBAR - 1));
        do_asks(rbx, game);
    }
    refresh_order(rbx);
    if let Held::Script(t) = rbx.tools.held
        && !in_character(rbx, t)
    {
        rbx.tools.held = Held::Nothing;
    }
    place_handles(rbx);
    mouse_moves(rbx);
    bins(env);
    super::hstudio_rbx_parts::step(env);
}

/// With a script's tool in hand, the Mouse is the crosshair: as the camera
/// turns or moves, what it points at changes, and its Move fires (Roblox's
/// does as a locked mouse turns the camera).
fn mouse_moves(rbx: &mut Rbx) {
    let cam = as_cf(&rbx.raw_get(rbx.camera, "CFrame")).unwrap_or_default();
    let view = (cam.position(), cam.look_vector());
    let before = rbx.tools.view.replace(view);
    let turned =
        before.is_some_and(|(at, look)| (at - view.0).magnitude() > 1e-4 || (look - view.1).magnitude() > 1e-5);
    // Handles being dragged have the mouse: they fire MouseDrag instead.
    if turned && takes_clicks(rbx) && !rbx.pointer.free && rbx.tools.handle.is_none() {
        let mouse = rbx.mouse;
        rbx.fire(mouse, "Move", Vec::new());
    }
}

/// The Backpack's stacks as the inventory has them: a Tool for each new
/// one (in the character if it is the stack in hand), its Count as it
/// changes, gone with its stack; and the player's choice of another slot.
fn mirror(rbx: &mut Rbx, inv: &[Option<(String, u32)>], slot: usize) {
    if rbx.tools.stacks.len() != SLOTS {
        rbx.tools.stacks = (0..SLOTS).map(|_| None).collect();
    }
    let was = std::mem::replace(&mut rbx.tools.quiet, true);
    let chosen = rbx.tools.selected != Some(slot);
    for i in 0..SLOTS {
        let want = inv.get(i).cloned().flatten().filter(|(_, n)| *n > 0);
        mirror_slot(rbx, i, want, slot);
    }
    if chosen {
        hold_slot(rbx, slot);
    }
    rbx.tools.quiet = was;
}

fn mirror_slot(rbx: &mut Rbx, i: usize, want: Option<(String, u32)>, slot: usize) {
    let have = rbx.tools.stacks[i].as_ref().map(|s| (s.tool, s.item.clone(), s.count));
    match (have, want) {
        (None, None) => {}
        (Some((t, _, _)), None) => {
            rbx.tools.stacks[i] = None;
            rbx.destroy(t);
        }
        (Some((t, item, count)), Some((now, n))) if item == now && (count == n || alive(rbx, t)) => {
            if count != n {
                set_attribute(rbx, t, "Count", Value::Num(f64::from(n)));
                if let Some(s) = rbx.tools.stacks[i].as_mut() {
                    s.count = n;
                }
            }
        }
        (have, Some((item, n))) => {
            if let Some((t, _, _)) = have {
                rbx.destroy(t);
            }
            let tool = make_tool(rbx, i, &item, n);
            rbx.tools.stacks[i] = Some(Stack { tool, item, count: n });
            let in_hand = i == slot && rbx.tools.selected == Some(slot) && rbx.tools.held == Held::Slot;
            let parent = if in_hand { Some(rbx.character) } else { backpack(rbx) };
            let _ = rbx.set_parent(tool, parent);
        }
    }
}

/// A stack's Tool: named after its item, its ItemId, Count and Slot as
/// attributes, its ToolTip the item's id, and an invisible Handle.
fn make_tool(rbx: &mut Rbx, slot: usize, item: &str, count: u32) -> Id {
    let tool = rbx.make("Tool", None);
    rbx.inst_mut(tool).name = title(item);
    rbx.raw_set(tool, "ToolTip", Value::str(item));
    rbx.raw_set(tool, "TextureId", Value::str(&item_icon(item)));
    set_attribute(rbx, tool, "ItemId", Value::str(item));
    set_attribute(rbx, tool, "Count", Value::Num(f64::from(count)));
    set_attribute(rbx, tool, "Slot", Value::Num((slot + 1) as f64));
    let handle = rbx.make("Part", None);
    rbx.inst_mut(handle).name = "Handle".into();
    rbx.raw_set(handle, "Size", v3(HANDLE));
    rbx.raw_set(handle, "Transparency", Value::Num(1.0));
    rbx.raw_set(handle, "CanCollide", Value::Bool(false));
    rbx.raw_set(handle, "CanTouch", Value::Bool(false));
    rbx.raw_set(handle, "Massless", Value::Bool(true));
    let _ = rbx.set_parent(handle, Some(tool));
    tool
}

/// An item's icon, as its TextureId: Minecraft's own texture of it, named
/// as Roblox names a texture of the client's (the game draws the item,
/// not the layer).
pub fn item_icon(item: &str) -> String {
    let (space, name) = item.split_once(':').unwrap_or(("minecraft", item));
    format!("rbxasset://{space}/textures/item/{name}.png")
}

/// The scripts' asks, in order: a slot selected at once; a use or a drop
/// once a game tick has passed with its slot selected; an unequip selects
/// the empty hotbar slot nearest the one held, else holds nothing.
fn do_asks(rbx: &mut Rbx, game: &mut dyn GameApi) {
    while let Some(&a) = rbx.tools.asks.front() {
        match a {
            Ask::Select(s) => {
                if rbx.tools.selected != Some(s) && game.select_slot(s) {
                    rbx.tools.selected = Some(s);
                }
            }
            Ask::Unequip => {
                let at = rbx.tools.selected.unwrap_or(0);
                let empty = (0..HOTBAR)
                    .filter(|&s| rbx.tools.stacks.get(s).is_some_and(Option::is_none))
                    .min_by_key(|&s| (s.abs_diff(at), s));
                match empty {
                    Some(s) if s == at || game.select_slot(s) => {
                        rbx.tools.selected = Some(s);
                        rbx.tools.held = Held::Slot;
                    }
                    _ => rbx.tools.held = Held::Nothing,
                }
            }
            Ask::Use(s) | Ask::Drop(s) => {
                let ticks = game.ticks();
                if rbx.tools.selected != Some(s) {
                    if !game.select_slot(s) {
                        rbx.tools.asks.pop_front();
                        continue;
                    }
                    rbx.tools.selected = Some(s);
                    rbx.tools.waiting = Some(ticks);
                    return;
                }
                if rbx.tools.waiting.is_some_and(|t| ticks <= t) {
                    return;
                }
                rbx.tools.waiting = None;
                match a {
                    Ask::Use(_) => game.click_item(false),
                    _ => game.drop_held(true),
                };
                rbx.tools.asks.pop_front();
                // One use or drop a tick.
                rbx.tools.waiting = Some(ticks);
                if rbx
                    .tools
                    .asks
                    .front()
                    .is_some_and(|a| matches!(a, Ask::Use(_) | Ask::Drop(_)))
                {
                    return;
                }
                continue;
            }
        }
        rbx.tools.asks.pop_front();
    }
}

/// The scripts' tools keep the slots they came to: those gone (destroyed,
/// out of the Backpack and the character) leave theirs, new ones take the
/// next.
fn refresh_order(rbx: &mut Rbx) {
    let mut order = std::mem::take(&mut rbx.tools.order);
    order.retain(|&t| own_tool_here(rbx, t));
    let holders = [backpack(rbx), Some(rbx.character)];
    for h in holders.into_iter().flatten() {
        for &c in rbx.children(h) {
            if own_tool_here(rbx, c) && !order.contains(&c) {
                order.push(c);
            }
        }
    }
    rbx.tools.order = order;
}

/// The Handles of the tools in the local character are in its right hand:
/// at the hand, pointing ahead, by the tool's Grip.
fn place_handles(rbx: &mut Rbx) {
    let ch = rbx.character;
    let arm = rbx.world.limbs[2];
    let arm_at = as_cf(&rbx.raw_get(arm, "CFrame"))
        .or_else(|| as_cf(&rbx.raw_get(rbx.root_part, "CFrame")))
        .unwrap_or_default();
    let hand = arm_at * CFrame::new(0.0, -HAND_Y, 0.0) * CFrame::angles(-std::f64::consts::FRAC_PI_2, 0.0, 0.0);
    let tools: Vec<Id> = rbx.children(ch).iter().copied().filter(|&c| is_tool(rbx, c)).collect();
    for t in tools {
        let Some(h) = rbx.child_named(t, "Handle").filter(|&h| alive(rbx, h)) else {
            continue;
        };
        let grip = as_cf(&rbx.raw_get(t, "Grip")).unwrap_or_default();
        let pos = as_v3(&rbx.raw_get(t, "GripPos")).unwrap_or_default();
        let at = hand * (CFrame::from_position(pos) * grip.rotation()).inverse();
        rbx.quiet_set(h, "CFrame", cf(at));
    }
}

// ---------------------------------------------------------------- BTools' bins

/// The left button's presses since the last step: one on Handles is
/// theirs (and so is its release); the others are the held HopperBin's
/// clicks: the Hammer, the Clone and the Grab (a held Grab drags its part
/// each frame).
fn bins(env: &mut dyn Env) {
    let presses = std::mem::take(&mut env.rbx().tools.presses);
    let mut clicks = Vec::new();
    for down in presses {
        let theirs = if down {
            super::hstudio_rbx_handles::press(env)
        } else {
            super::hstudio_rbx_handles::release(env.rbx())
        };
        if !theirs {
            if down {
                super::hstudio_rbx_parts::click(env, false);
            }
            clicks.push(down);
        }
    }
    for _ in std::mem::take(&mut env.rbx().tools.rights) {
        super::hstudio_rbx_parts::click(env, true);
    }
    super::hstudio_rbx_handles::drag(env);
    let rbx = env.rbx();
    let bin = match rbx.tools.held {
        Held::Script(t) if in_character(rbx, t) && rbx.inst(t).class.name == "HopperBin" => {
            as_item(&rbx.raw_get(t, "BinType")).map_or("", |i| i.name)
        }
        _ => "",
    };
    if bin != "Grab" && rbx.tools.grab.is_some() {
        put_back_grab(rbx);
    }
    for down in clicks {
        match (bin, down) {
            ("Hammer", true) => hammer(env),
            ("Clone", true) => clone(env),
            ("Grab", true) => grab(env),
            ("Grab", false) => let_go(env),
            _ => {}
        }
    }
    if let Some(Grab::Part(p)) = env.rbx().tools.grab.clone() {
        drag(env, p);
    }
}

/// Where the Mouse points (the pointer, or the crosshair while the game has
/// it), leaving out `ignore` and the Mouse's TargetFilter.
pub fn aim(env: &mut dyn Env, ignore: &[Id]) -> Option<super::hstudio_rbx_query::Hit> {
    super::hstudio_rbx_world::look(env);
    let (free, x, y) = {
        let p = &env.rbx().pointer;
        (p.free || p.chat_open, p.x, p.y)
    };
    let (w, h, _) = env.game().dp();
    let (mx, my) = if free {
        (x, y)
    } else {
        (f64::from(w) / 2.0, f64::from(h) / 2.0)
    };
    let (o, d) = super::hstudio_rbx_draw::screen_ray(env, mx, my);
    let rbx = env.rbx();
    let mut ids = ignore.to_vec();
    ids.extend(rbx.id_of(&rbx.raw_get(rbx.mouse, "TargetFilter")));
    let filter = super::hstudio_rbx_query::Filter {
        ids,
        include: false,
        ignore_water: true,
    };
    super::hstudio_rbx_query::cast(env, o, d * 500.0, &filter)
}

/// The block a ray met: the one behind the face it hit.
fn cell(hit: &super::hstudio_rbx_query::Hit) -> [i32; 3] {
    let p = hit.at - hit.normal * 0.01;
    [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
}

/// The block beside `c` across the face whose normal is `n`.
fn beside(c: [i32; 3], n: Vector3) -> [i32; 3] {
    [
        c[0] + n.x.round() as i32,
        c[1] + n.y.round() as i32,
        c[2] + n.z.round() as i32,
    ]
}

/// The entity whose mirror `part` is in, by its id.
pub fn entity_of(rbx: &Rbx, part: Id) -> Option<u32> {
    rbx.world
        .mirrors
        .iter()
        .find(|(_, m)| m.model == part || rbx.is_descendant_of(part, m.model))
        .map(|(&id, _)| id)
}

/// Whether a bin may change `part`: a script's own part, not Locked (as
/// Roblox's bins left locked parts be), not the character's or a mirror's.
fn free_part(rbx: &Rbx, part: Id) -> bool {
    let i = rbx.inst(part);
    let terrain = rbx.id_of(&rbx.raw_get(rbx.workspace, "Terrain"));
    !i.locked
        && !i.destroyed
        && !rbx.raw_get(part, "Locked").truthy()
        && Some(part) != terrain
        && !rbx.is_descendant_of(part, rbx.character)
        && entity_of(rbx, part).is_none()
}

/// The Hammer: the part clicked is destroyed, with a blast's sound; a block
/// clicked breaks in the client's copy of the world; an entity's mirror
/// takes the entity out of the client's view.
fn hammer(env: &mut dyn Env) {
    let Some(hit) = aim(env, &[]) else { return };
    if hit.block.is_some() {
        let [x, y, z] = cell(&hit);
        env.game().set_client_block(x, y, z, "minecraft:air");
        return;
    }
    let Some(part) = hit.part else { return };
    let rbx = env.rbx();
    if let Some(e) = entity_of(rbx, part) {
        env.game().hide_entity(e);
        return;
    }
    if free_part(rbx, part) {
        let at = hit.at;
        rbx.sounds.push((
            "minecraft:entity.generic.explode".into(),
            Some([at.x, at.y, at.z]),
            0.3,
            1.4,
        ));
        rbx.destroy(part);
    }
}

/// The Clone: a copy of the part clicked, on top of it; a block clicked is
/// copied onto the face clicked, in the client's copy of the world.
fn clone(env: &mut dyn Env) {
    let Some(hit) = aim(env, &[]) else { return };
    if let Some(block) = &hit.block {
        let [x, y, z] = beside(cell(&hit), hit.normal);
        env.game().set_client_block(x, y, z, block);
        return;
    }
    let Some(part) = hit.part else { return };
    let rbx = env.rbx();
    if !free_part(rbx, part) || !rbx.raw_get(part, "Archivable").truthy() {
        return;
    }
    let parent = rbx.inst(part).parent;
    let copy = rbx.clone_tree(part, Owner::Console, true);
    let at = as_cf(&rbx.raw_get(part, "CFrame")).unwrap_or_default();
    let size = as_v3(&rbx.raw_get(part, "Size")).unwrap_or_default();
    rbx.raw_set(
        copy,
        "CFrame",
        cf(CFrame::from_position(Vector3::new(0.0, size.y, 0.0)) * at),
    );
    let _ = rbx.set_parent(copy, parent);
}

/// The Grab takes hold of the part clicked, or lifts the block clicked out
/// of the client's copy of the world.
fn grab(env: &mut dyn Env) {
    put_back_grab(env.rbx());
    let Some(hit) = aim(env, &[]) else { return };
    if let Some(block) = hit.block.clone() {
        let c = cell(&hit);
        if env.game().set_client_block(c[0], c[1], c[2], "minecraft:air") {
            env.rbx().tools.grab = Some(Grab::Block(block, c));
        }
        return;
    }
    let rbx = env.rbx();
    if let Some(part) = hit.part.filter(|&p| free_part(rbx, p)) {
        rbx.tools.grab = Some(Grab::Part(part));
    }
}

/// The part held follows the Mouse: on what it points at, its bottom there.
fn drag(env: &mut dyn Env, part: Id) {
    if !alive(env.rbx(), part) {
        env.rbx().tools.grab = None;
        return;
    }
    let Some(hit) = aim(env, &[part]) else { return };
    let rbx = env.rbx();
    let at = as_cf(&rbx.raw_get(part, "CFrame")).unwrap_or_default();
    let size = as_v3(&rbx.raw_get(part, "Size")).unwrap_or_default();
    let to = hit.at + hit.normal * (size.y / 2.0);
    if (to - at.position()).magnitude() > 1e-3 {
        rbx.raw_set(part, "CFrame", cf(CFrame::from_position(to) * at.rotation()));
    }
}

/// The Grab let go: a part stays where it was dragged; a block goes down
/// on the face the Mouse points at (back where it was when it points at
/// nothing).
fn let_go(env: &mut dyn Env) {
    let Some(g) = env.rbx().tools.grab.take() else { return };
    let Grab::Block(block, from) = g else { return };
    let to = match aim(env, &[]) {
        Some(hit) if hit.block.is_some() => beside(cell(&hit), hit.normal),
        _ => from,
    };
    env.game().set_client_block(to[0], to[1], to[2], &block);
}

/// A Grab put away while it lifts a block: the block, left in the air, is
/// forgotten (the next frame's step can't reach the game from here); a
/// part held is let go.
fn put_back_grab(rbx: &mut Rbx) {
    rbx.tools.grab = None;
}

// ---------------------------------------------------------------- the hotbar's extra slots

/// The scripts' tools as slots after the hotbar's, on the HUD: each a box
/// with its name's first letters (a HopperBin's BinType), the one held
/// framed in white with its name above.
pub fn draw(rbx: &Rbx, game: &mut dyn GameApi) {
    super::hstudio_rbx_handles::draw(rbx, game);
    super::hstudio_rbx_parts::draw(rbx, game);
    if !rbx.active || game.screen() != "playing" {
        return;
    }
    let own = own_tools(rbx);
    if own.is_empty() {
        return;
    }
    let (w, h) = game.gui_size();
    // The hotbar is 182 wide, centred on the bottom edge.
    let x0 = (w / 2.0 + 91.0 + 4.0).floor();
    let y0 = h - 22.0;
    let held = match rbx.tools.held {
        Held::Script(t) if in_character(rbx, t) => Some(t),
        _ => None,
    };
    for (i, &t) in own.iter().enumerate() {
        let x = x0 + i as f32 * 22.0;
        let name = tool_label(rbx, t);
        let on = held == Some(t);
        game.fill([x, y0, 22.0, 22.0], 0x9000_0000);
        let edge = if on { 0xFFFF_FFFF } else { 0xFF8B_8B8B };
        let t2 = if on { 2.0 } else { 1.0 };
        game.fill([x, y0, 22.0, t2], edge);
        game.fill([x, y0 + 22.0 - t2, 22.0, t2], edge);
        game.fill([x, y0, t2, 22.0], edge);
        game.fill([x + 22.0 - t2, y0, t2, 22.0], edge);
        let short: String = name.chars().filter(|c| !c.is_whitespace()).take(3).collect();
        let tw = game.text_width(&short);
        game.text(&short, [x + 11.0 - tw / 2.0, y0 + 7.0], 0xFF_FFFF, true);
        if i == 0 {
            game.text("0", [x + 1.0, y0 - 9.0], 0xA0_A0A0, true);
        }
        if on {
            let tw = game.text_width(&name);
            let nx = (x + 11.0 - tw / 2.0).max(x0);
            game.text(&name, [nx, y0 - 10.0], 0xFF_FFFF, true);
        }
    }
}

/// What a script's tool's slot is called: a HopperBin's BinType (BTools'
/// bins have names nobody reads), a Tool's name.
pub fn tool_label(rbx: &Rbx, t: Id) -> String {
    if rbx.inst(t).class.name == "HopperBin" {
        return as_item(&rbx.raw_get(t, "BinType")).map_or("Bin", |i| i.name).to_owned();
    }
    rbx.inst(t).name.clone()
}
