// The other players' tools and armour (hstudio_rbx_gear). In Roblox another
// player's Backpack replicates, and the Tool they hold is in their
// Character. A Minecraft server tells the client what another player holds
// and wears (SetEquipment: the main hand, the off hand, the armour), never
// the rest of their hotbar or inventory. So the item in their main hand is
// a Tool in their Character, equipped, named after the item, with ItemId
// and Count attributes as the LocalPlayer's are and a Handle in their right
// hand; when they hold another, it goes into their Backpack (Unequipped)
// and the new one comes out of it, or is made (Equipped). Their Backpack
// keeps a Tool for each item seen in their hand this session but the one
// held, with a LastSeen attribute (os.time() when it was put away): what
// Infinite Yield's copytools copies. What is in their off hand and what
// they wear are Accessories in their Character, with ItemId, Count and Slot
// ("OffHand", "Head", "Chest", "Legs", "Feet").
//
// The Handle of the local player's held Tool touched to another entity's
// part (firetouchinterest, as Infinite Yield's handlekill does) attacks
// that entity, as a sword's touch hurts in Roblox: once per attack cooldown
// and within the server's reach.

use super::hstudio::{EntityInfo, GameApi};
use super::hstudio_lua::Value;
use super::hstudio_rbx::{Id, Rbx};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{as_cf, cf, v3};
use super::hstudio_rbx_world::{set_attribute, title};
use std::collections::BTreeMap;

/// The most Tools a Backpack keeps of what was seen (the oldest go first).
const MAX_SEEN: usize = 36;
/// A Handle's size, and the hand under the right arm's centre, in blocks.
const HANDLE: Vector3 = Vector3::new(0.25, 0.25, 0.6);
const HAND_Y: f64 = 0.35;
/// The attack cooldown's wait between two touches' attacks (a sword's), and
/// how far the server lets the player hit (the entity interaction range and
/// the server's three blocks more, from the eyes to the entity's box).
const ATTACK_EVERY: f64 = 0.625;
const REACH: f64 = 6.0;
/// The slots that are Accessories, as EntityInfo names them.
const WORN: [&str; 5] = ["OffHand", "Head", "Chest", "Legs", "Feet"];

/// Each other player's gear, by their UUID; and the attacks touches asked for.
#[derive(Default)]
pub struct Gears {
    pub players: BTreeMap<u128, Gear>,
    pub attacks: Vec<u32>,
    pub attacked_at: Option<f64>,
}

/// One player's: the item in their hand and its Tool, the Tools in their
/// Backpack by item, and their Accessories by slot (with the item).
#[derive(Default)]
pub struct Gear {
    pub held: Option<(String, Id)>,
    pub seen: Vec<(String, Id)>,
    pub worn: Vec<(&'static str, String, Id)>,
}

fn alive(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id).is_some_and(|i| !i.destroyed)
}

fn backpack_of(rbx: &Rbx, player: Id) -> Option<Id> {
    rbx.children(player)
        .iter()
        .copied()
        .find(|&c| rbx.inst(c).class.name == "Backpack" && alive(rbx, c))
}

fn unix_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs() as f64)
}

/// Each frame the world is mirrored: every other player's gear as the game
/// says, from their entity's equipment (`list`, the entities as the game
/// listed them this frame); a player who left takes theirs with them.
pub fn sync(rbx: &mut Rbx, list: &[EntityInfo]) {
    let listed: Vec<u128> = rbx.world.others.iter().map(|o| o.uuid).collect();
    rbx.world.gear.players.retain(|u, _| listed.contains(u));
    for i in 0..rbx.world.others.len() {
        let o = &rbx.world.others[i];
        let (uuid, player, character, entity) = (o.uuid, o.player, o.character, o.entity);
        let mut gear = rbx.world.gear.players.remove(&uuid).unwrap_or_default();
        let e = entity.and_then(|id| list.iter().find(|e| e.id == id));
        follow(rbx, &mut gear, player, character.filter(|_| e.is_some()), e);
        rbx.world.gear.players.insert(uuid, gear);
    }
}

fn follow(rbx: &mut Rbx, gear: &mut Gear, player: Id, character: Option<Id>, e: Option<&EntityInfo>) {
    let Some(bp) = backpack_of(rbx, player) else {
        return;
    };
    gear.seen
        .retain(|(_, t)| alive(rbx, *t) && rbx.inst(*t).parent == Some(bp));
    // The Tool in hand went with a character that went (they went out of
    // sight): the item is in their Backpack, last seen.
    if let Some((item, t)) = gear.held.clone()
        && (!alive(rbx, t) || character.is_none_or(|c| rbx.inst(t).parent != Some(c)))
    {
        gear.held = None;
        if alive(rbx, t) && rbx.inst(t).parent.is_some_and(|p| Some(p) != character) {
            // A script took it out of their hand: theirs to keep.
        } else if !gear.seen.iter().any(|(i, _)| *i == item) {
            let tool = make_tool(rbx, &item, 1);
            put_away(rbx, gear, bp, item, tool);
        }
    }
    let Some((character, e)) = character.zip(e) else {
        return;
    };
    let slot = |name: &str| {
        e.equipment
            .iter()
            .find(|(s, _, _)| s == name)
            .map(|(_, i, n)| (i.clone(), *n))
    };
    let hand = slot("MainHand");
    match (&gear.held, &hand) {
        (Some((was, t)), Some((now, n))) if was == now => {
            set_attribute(rbx, *t, "Count", Value::Num(f64::from(*n)));
        }
        _ => {
            // Another item in hand (or none): the one held goes into the
            // Backpack, the new one comes out of it or is made.
            if let Some((item, t)) = gear.held.take() {
                let _ = rbx.set_parent(t, Some(bp));
                rbx.fire(t, "Unequipped", Vec::new());
                put_away(rbx, gear, bp, item, t);
            }
            if let Some((item, n)) = hand {
                let tool = match gear.seen.iter().position(|(i, _)| *i == item) {
                    Some(k) => gear.seen.remove(k).1,
                    None => make_tool(rbx, &item, n),
                };
                set_attribute(rbx, tool, "Count", Value::Num(f64::from(n)));
                let _ = rbx.set_parent(tool, Some(character));
                rbx.fire(tool, "Equipped", Vec::new());
                gear.held = Some((item, tool));
            }
        }
    }
    // The off hand and the armour, as Accessories.
    for name in WORN {
        let want = slot(name);
        let have = gear.worn.iter().position(|(s, _, _)| *s == name);
        let same = have.is_some_and(|k| {
            let (_, item, a) = &gear.worn[k];
            alive(rbx, *a) && want.as_ref().is_some_and(|(i, _)| i == item)
        });
        if same {
            if let (Some(k), Some((_, n))) = (have, &want) {
                set_attribute(rbx, gear.worn[k].2, "Count", Value::Num(f64::from(*n)));
            }
            continue;
        }
        if let Some(k) = have {
            let (_, _, a) = gear.worn.remove(k);
            if alive(rbx, a) {
                rbx.destroy(a);
            }
        }
        if let Some((item, n)) = want {
            let a = make_accessory(rbx, name, &item, n);
            let _ = rbx.set_parent(a, Some(character));
            gear.worn.push((name, item, a));
        }
    }
}

/// A Tool into the Backpack, last seen now; past MAX_SEEN the oldest goes.
fn put_away(rbx: &mut Rbx, gear: &mut Gear, bp: Id, item: String, tool: Id) {
    set_attribute(rbx, tool, "LastSeen", Value::Num(unix_now()));
    if rbx.inst(tool).parent != Some(bp) {
        let _ = rbx.set_parent(tool, Some(bp));
    }
    gear.seen.retain(|(i, t)| {
        if *i == item && *t != tool {
            rbx.destroy(*t);
            false
        } else {
            true
        }
    });
    gear.seen.push((item, tool));
    while gear.seen.len() > MAX_SEEN {
        let (_, old) = gear.seen.remove(0);
        rbx.destroy(old);
    }
}

/// Another player's item as a Tool: named after it, its ItemId and Count,
/// its ToolTip the id, and a Handle (as the LocalPlayer's stacks are).
fn make_tool(rbx: &mut Rbx, item: &str, count: u32) -> Id {
    let tool = rbx.make("Tool", None);
    rbx.inst_mut(tool).name = title(item);
    rbx.raw_set(tool, "ToolTip", Value::str(item));
    rbx.raw_set(
        tool,
        "TextureId",
        Value::str(&super::hstudio_rbx_tools::item_icon(item)),
    );
    set_attribute(rbx, tool, "ItemId", Value::str(item));
    set_attribute(rbx, tool, "Count", Value::Num(f64::from(count)));
    let handle = handle(rbx);
    let _ = rbx.set_parent(handle, Some(tool));
    tool
}

/// What another player wears or has in their off hand, as an Accessory.
fn make_accessory(rbx: &mut Rbx, slot: &str, item: &str, count: u32) -> Id {
    let a = rbx.make("Accessory", None);
    rbx.inst_mut(a).name = title(item);
    set_attribute(rbx, a, "ItemId", Value::str(item));
    set_attribute(rbx, a, "Count", Value::Num(f64::from(count)));
    set_attribute(rbx, a, "Slot", Value::str(slot));
    let handle = handle(rbx);
    let _ = rbx.set_parent(handle, Some(a));
    a
}

fn handle(rbx: &mut Rbx) -> Id {
    let h = rbx.make("Part", None);
    rbx.inst_mut(h).name = "Handle".into();
    rbx.raw_set(h, "Size", v3(HANDLE));
    rbx.raw_set(h, "Transparency", Value::Num(1.0));
    rbx.raw_set(h, "CanCollide", Value::Bool(false));
    rbx.raw_set(h, "CanTouch", Value::Bool(false));
    rbx.raw_set(h, "Massless", Value::Bool(true));
    h
}

/// The Handles of the Tools other players hold are in their right hands,
/// and their Accessories' on the parts they are worn on (the off hand's in
/// the left hand), as the mirrors are placed each frame.
pub fn place(rbx: &mut Rbx) {
    let mut places = Vec::new();
    for o in &rbx.world.others {
        let (Some(c), Some(e)) = (o.character, o.entity) else {
            continue;
        };
        let Some(limbs) = rbx.world.mirrors.get(&e).and_then(|m| m.limbs) else {
            continue;
        };
        let Some(g) = rbx.world.gear.players.get(&o.uuid) else {
            continue;
        };
        let at = |part: Id| as_cf(&rbx.raw_get(part, "CFrame")).unwrap_or_default();
        let hand =
            |arm: Id| at(arm) * CFrame::new(0.0, -HAND_Y, 0.0) * CFrame::angles(-std::f64::consts::FRAC_PI_2, 0.0, 0.0);
        if let Some((_, t)) = &g.held {
            places.push((*t, hand(limbs[2])));
        }
        let head = rbx.child_named(c, "Head").map_or(at(limbs[0]), at);
        for (slot, _, a) in &g.worn {
            let to = match *slot {
                "OffHand" => hand(limbs[1]),
                "Head" => head,
                "Legs" | "Feet" => at(limbs[3]),
                _ => at(limbs[0]),
            };
            places.push((*a, to));
        }
    }
    for (holder, to) in places {
        if let Some(h) = rbx.child_named(holder, "Handle").filter(|&h| alive(rbx, h)) {
            rbx.quiet_set(h, "CFrame", cf(to));
        }
    }
}

/// firetouchinterest(a, b, touching): the Handle of the local player's held
/// Tool touching a part of another entity's mirror (either way round)
/// attacks that entity.
pub fn touched(rbx: &mut Rbx, a: Id, b: Id, touching: bool) {
    if !touching {
        return;
    }
    let Some(tool) = super::hstudio_rbx_tools::held_tool(rbx) else {
        return;
    };
    let handle = rbx.child_named(tool, "Handle");
    let other = match handle {
        Some(h) if h == a => b,
        Some(h) if h == b => a,
        _ => return,
    };
    let Some(entity) = super::hstudio_rbx_tools::entity_of(rbx, other) else {
        return;
    };
    let living = rbx.world.mirrors.get(&entity).is_some_and(|m| m.humanoid.is_some());
    if living && !rbx.world.gear.attacks.contains(&entity) {
        rbx.world.gear.attacks.push(entity);
    }
}

/// The attacks touches asked for: one each attack cooldown, at an entity
/// alive and in reach (the others wait for the next).
pub fn attack(rbx: &mut Rbx, game: &mut dyn GameApi) {
    let asked = std::mem::take(&mut rbx.world.gear.attacks);
    if asked.is_empty() || rbx.world.gear.attacked_at.is_some_and(|t| rbx.now - t < ATTACK_EVERY) {
        return;
    }
    let Some(p) = game.player() else {
        return;
    };
    let eyes = Vector3::new(p.x, p.y + rbx.world.body.eyes, p.z);
    for id in asked {
        let Some(m) = rbx.world.mirrors.get(&id) else {
            continue;
        };
        let at = as_cf(&rbx.raw_get(m.root, "CFrame")).unwrap_or_default().position();
        if (at - eyes).magnitude() > REACH || m.health <= 0.0 {
            continue;
        }
        if game.attack_entity(id) {
            rbx.world.gear.attacked_at = Some(rbx.now);
            return;
        }
    }
}
