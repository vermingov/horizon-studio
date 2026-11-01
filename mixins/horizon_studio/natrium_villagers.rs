// Natrium's villager item pickup (in the game's villagers.rs): Horizon's
// `villager_pickups`, with one change. For every villager it asks each dropped
// item what it is (a hash lookup in an item tag) before whether it is within a
// block; Natrium asks where it is first. Both tests only read, so the same
// items are taken in the same order, and a village near an item farm no longer
// costs a lookup per villager and item each tick.

use super::*;

pub(in crate::game) fn villager_pickups(game: &mut Game<'_>) {
    if game.dropped.is_empty() || !game.ui.rules.bool("mob_griefing") {
        return;
    }
    let picks_up = game.res.items().item_tags.get("minecraft:villager_picks_up").cloned();
    let mut skipped = 0u64;
    for i in 0..game.mobs.len() {
        let m = &game.mobs[i];
        if m.species() != Species::Villager || !m.alive() {
            continue;
        }
        let (pos, half) = (m.position, f64::from(m.width()) / 2.0 + 1.0);
        let top = f64::from(m.height());
        let requested = villager::requested_items(m);
        let mut j = 0;
        while j < game.dropped.len() {
            let e = &game.dropped[j];
            let near = (e.position.x - pos.x).abs() <= half
                && (e.position.z - pos.z).abs() <= half
                && e.position.y >= pos.y - 0.25
                && e.position.y <= pos.y + top;
            // Natrium: the item's place first, then what it is (a tag
            // lookup by name): the same items, without looking up those
            // too far to take.
            if !near || e.pickup_delay > 0 {
                skipped += 1;
                j += 1;
                continue;
            }
            let item = e.stack.item.clone();
            let wants = villager::food_points(&item) > 0
                || picks_up.as_ref().is_some_and(|t| t.contains(&*item))
                || requested.contains(&&*item);
            if !wants {
                j += 1;
                continue;
            }
            let max = game.item_max_stack(&item);
            let stack = game.dropped[j].stack.clone();
            if !villager::ext_mut(&mut game.mobs[i]).can_add(&stack, max) {
                j += 1;
                continue;
            }
            let before = game.dropped[j].clone();
            let rest = villager::ext_mut(&mut game.mobs[i]).add_item(stack, max);
            if rest.as_ref().is_none_or(|r| r.count < before.stack.count) {
                let id = game.mobs[i].id;
                game.add_pickup(
                    crate::game::pickups::Taken::Item(Box::new(before)),
                    crate::game::pickups::Collector::Mob(id),
                );
            }
            match rest {
                None => {
                    game.remove_dropped(j);
                }
                Some(rest) => {
                    game.dropped[j].stack.count = rest.count;
                    j += 1;
                }
            }
        }
    }
    horizon_core::natrium::saved_tick_work(skipped);
}
