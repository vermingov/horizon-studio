// Natrium's item merging (in the game's entities.rs): `merge_with_neighbours`
// as Horizon has it, item for item and in the same order, with each item's
// stack size looked up once (Horizon asks up to four times per neighbour, a
// hash lookup by item id each time) and the neighbour list kept between calls.
// A merge changes counts only, never an item or its components, so a stack's
// size is the same all through the call.

use super::*;

thread_local! {
    static NEAR: std::cell::Cell<Vec<u32>> = const { std::cell::Cell::new(Vec::new()) };
}

pub fn merge_with_neighbours(
    items: &mut [ItemEntity],
    removed: &mut [bool],
    cells: &mut ItemCells,
    i: usize,
    max_stack: &dyn Fn(&ItemStack) -> u32,
) {
    let age = |items: &[ItemEntity], j: usize| {
        if j == i { items[j].age - 1 } else { items[j].age }
    };
    if removed[i] {
        return;
    }
    let max_i = max_stack(&items[i].stack);
    if !items[i].mergeable(age(items, i), max_i) {
        return;
    }
    let area = items[i].bounds().inflate(Vec3::new(0.5, 0.0, 0.5));
    let mut near = NEAR.take();
    cells.near(area, &mut near);
    let mut saved = 0u64;
    for &j in &near {
        let j = j as usize;
        if j == i || removed[j] || !items[j].bounds().intersects(area) {
            continue;
        }
        let max_j = max_stack(&items[j].stack);
        if !items[j].mergeable(age(items, j), max_j) {
            continue;
        }
        // ItemEntity.areMergable: both fit one stack of the same item and components.
        let (this, other) = (&items[i].stack, &items[j].stack);
        if items[i].target != items[j].target {
            continue;
        }
        saved += 1;
        if this.count + other.count > max_j || !this.same_item_same_components(other) {
            continue;
        }
        let (to, from) = if other.count < this.count { (i, j) } else { (j, i) };
        // ItemEntity.merge(toStack, fromStack, 64).
        let max_to = if to == i { max_i } else { max_j };
        saved += 1;
        let room = max_to.min(64) - items[to].stack.count;
        let moved = room.min(items[from].stack.count);
        items[to].stack.count += moved;
        items[from].stack.count -= moved;
        let (delay, from_age, to_age) = (items[from].pickup_delay, age(items, from), age(items, to));
        let to_item = &mut items[to];
        to_item.pickup_delay = to_item.pickup_delay.max(delay);
        // This item's age goes up once its tick ends.
        to_item.age = to_age.min(from_age) + u32::from(to == i);
        if items[from].stack.count == 0 {
            removed[from] = true;
            cells.remove(from, items[from].position);
        }
        if removed[i] {
            break;
        }
    }
    NEAR.set(near);
    horizon_core::natrium::saved_tick_work(saved);
}
