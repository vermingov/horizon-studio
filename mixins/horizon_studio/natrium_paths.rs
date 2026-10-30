// Natrium's swimming neighbours (in the game's mobs/path.rs): Horizon's
// `swim_neighbors`, which keeps the six sides' nodes in a new hash map for
// every node the search expands, with them in an array instead. The same
// nodes are asked for and given back in the same order; fish, squid, drowned
// and the other swimmers find their paths with less work.

use super::*;

/// Direction.values(): down, up, north, south, west, east.
const SIDES: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];

/// The side (dx, 0, dz)'s place in `SIDES`.
fn side(dx: i32, dz: i32) -> usize {
    match (dx, dz) {
        (0, -1) => 2,
        (0, 1) => 3,
        (-1, 0) => 4,
        _ => 5,
    }
}

pub(super) fn swim_neighbors(f: &mut Finder<'_, '_>, pos: u32, out: &mut Vec<u32>) {
    let (px, py, pz) = {
        let n = &f.nodes[pos as usize];
        (n.x, n.y, n.z)
    };
    let open = |s: &Finder<'_, '_>, n: Option<u32>| n.is_some_and(|n| !s.nodes[n as usize].closed);
    let malus = |s: &Finder<'_, '_>, n: Option<u32>| n.is_some_and(|n| s.nodes[n as usize].cost_malus >= 0.0);
    let mut by_side = [None; 6];
    for (k, (dx, dy, dz)) in SIDES.into_iter().enumerate() {
        let n = f.swim_accepted(px + dx, py + dy, pz + dz);
        by_side[k] = n;
        if open(f, n) {
            out.push(n.unwrap_or_default());
        }
    }
    for ((dx, dz), (cx, cz)) in [
        ((0, -1), (1, 0)),
        ((1, 0), (0, 1)),
        ((0, 1), (-1, 0)),
        ((-1, 0), (0, -1)),
    ] {
        if malus(f, by_side[side(dx, dz)]) && malus(f, by_side[side(cx, cz)]) {
            let d = f.swim_accepted(px + dx + cx, py, pz + dz + cz);
            if open(f, d) {
                out.push(d.unwrap_or_default());
            }
        }
    }
    horizon_core::natrium::saved_tick_work(1);
}
