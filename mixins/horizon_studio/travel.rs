// The player's moves a Roblox script makes (hstudio_travel), kept to what a
// server takes. A script moving the root part (its CFrame set, PivotTo, a
// tween, TranslateBy) moves the player at once in Roblox; a Minecraft
// server checks each move packet (ServerGamePacketListenerImpl's
// handleMovePlayer) and sends the player back for one that goes too far
// from where the tick began ("moved too quickly": 10 blocks for one packet,
// sqrt(100 n) for n in a tick on vanilla, 10 n on Paper) or through blocks
// ("moved wrongly", not in creative), and kicks a player who hovers over
// air for four seconds without falling. So the client goes there as fast as
// that allows: a step of at most MoveLimits::per_tick each game tick (the
// one move packet the client sends a tick), in a line where the way is
// open, round the blocks in the way (up over them, or along a path through
// the air the search finds) where it isn't, and as far as it can get where
// there is no way, with a warning. A long way through the air dips a little
// every two seconds, so the server never counts it as hovering.
//
// Moves a script makes one after another without waiting (a tween, a loop)
// change where the player is going; one it makes after holding the last
// for a while (IY's scare: in front of someone, half a second, back) is a
// place of its own the player goes to and stays at for as long as the
// script held it, before the next. Where the server checks nothing (the
// client's own world), a move goes at once, as before.

use super::hstudio::{GameApi, MoveLimits};
use super::hstudio_rbx_types::Vector3;
use std::collections::{BinaryHeap, HashMap, VecDeque};

/// The player's box: its width, and a standing player's height.
const WIDTH: f64 = 0.6;
const HEIGHT: f64 = 1.8;
/// A move held this long (seconds) before the next is a place of its own;
/// a place got to last is stayed at for as long as the script held it, up
/// to MAX_HOLD.
const HOLD: f64 = 0.12;
const MAX_HOLD: f64 = 5.0;
/// The places waiting, at most.
const MAX_GOALS: usize = 8;
/// A goal this near is only gone to in a line (a step, a walk, a fly): no
/// way is searched round what is in it.
const NEAR: f64 = 3.0;
/// Close enough to be there.
const THERE: f64 = 1e-3;
/// How far apart the boxes a move is checked with are, along it.
const SAMPLE: f64 = 0.25;
/// Searches for a way at most this often (seconds), and the cells one may try.
const PLAN_EVERY: f64 = 0.25;
const MAX_NODES: usize = 2500;
/// How far past the start and the goal a search may go, in blocks.
const MARGIN: i32 = 24;
/// The heights tried to go over what is in the way, above the higher end.
const OVER: [f64; 6] = [2.0, 4.0, 8.0, 16.0, 32.0, 64.0];
/// Ticks of moves through the air that don't fall before one dips, and by how much.
const AIR_TICKS: u32 = 40;
const DIP: f64 = 0.0625;
/// A game tick, in seconds.
const TICK: f64 = 0.05;
/// How long a goal waits for a ride to end, and for a blocked way to clear.
const WAIT: f64 = 2.0;

/// A place a script moved the player to (their feet), and when it last did.
#[derive(Clone, Copy, Debug)]
pub struct Goal {
    pub feet: Vector3,
    pub since: f64,
}

/// The moves under way, between frames.
#[derive(Default)]
pub struct Travel {
    pub goals: VecDeque<Goal>,
    /// The way to the first goal: the points to pass, the last the goal
    /// (as far as it can be reached), and the goal it was found for.
    route: Vec<Vector3>,
    route_for: Option<Vector3>,
    /// The goal can't be reached past the route's end.
    stuck: bool,
    planned_at: f64,
    searched_at: f64,
    /// When the player got to the first goal (it waits there), and when
    /// the goal started waiting for a ride to end or a way to open.
    arrived: Option<f64>,
    waiting: Option<f64>,
    /// The last goal got to (when the script set it, when the player got
    /// there), and when the next may start.
    done: Option<(f64, f64)>,
    hold: Option<f64>,
    /// The game tick the moves are counted in, and where the player was as
    /// it began (where the last move packet put them).
    tick: Option<u64>,
    anchor: Vector3,
    /// Ticks the moves went through the air without falling, and the
    /// frame's time.
    air: u32,
    last: f64,
}

/// A script moved the player's feet to `feet` (the last of the frame's moves).
pub fn want(t: &mut Travel, now: f64, feet: Vector3) {
    if ![feet.x, feet.y, feet.z].iter().all(|v| v.is_finite()) {
        return;
    }
    // The last place was got to later than the script put the player there:
    // the script's next move waits as long after it as the script waited.
    if t.goals.is_empty()
        && let Some((since, got)) = t.done.take()
        && now - since < MAX_HOLD
    {
        t.hold = Some(got + (now - since));
    }
    match t.goals.back_mut() {
        Some(last) if now - last.since < HOLD || (last.feet - feet).magnitude() < 0.5 => {
            last.feet = feet;
            last.since = now;
        }
        _ => {
            if t.goals.len() >= MAX_GOALS {
                // The oldest after the one under way give way.
                t.goals.remove(1);
            }
            t.goals.push_back(Goal { feet, since: now });
        }
    }
}

/// Drops the moves under way.
pub fn stop(t: &mut Travel) {
    t.goals.clear();
    t.route.clear();
    t.route_for = None;
    t.arrived = None;
    t.waiting = None;
    t.stuck = false;
}

/// Each frame: the player moves on toward the first goal, as far as the
/// server's limits let this frame. A warning for the console, when a goal
/// can't be reached.
pub fn step(t: &mut Travel, game: &mut dyn GameApi, now: f64) -> Option<String> {
    let dt = (now - t.last).clamp(0.0, 1.0);
    t.last = now;
    if t.goals.is_empty() {
        t.tick = None;
        return None;
    }
    let Some(p) = game.player() else {
        stop(t);
        return None;
    };
    let here = Vector3::new(p.x, p.y, p.z);
    let Some(lim) = game.move_limits() else {
        // Nothing checks: at once, to the last place asked.
        let goal = t.goals.back().map(|g| g.feet);
        stop(t);
        if let Some(g) = goal {
            game.move_player([g.x, g.y, g.z]);
        }
        return None;
    };
    let ticks = game.ticks();
    if t.tick != Some(ticks) {
        // A new tick: its budget runs from where the last packet put them.
        let falls = here.y - t.anchor.y < -0.03125;
        let ground = game.pose().is_some_and(|q| q.on_ground);
        t.air = if t.tick.is_some() && !falls && !ground {
            t.air + 1
        } else {
            0
        };
        t.tick = Some(ticks);
        t.anchor = here;
    }
    if t.hold.is_some_and(|h| now < h) {
        return None;
    }
    t.hold = None;
    if lim.riding {
        // A passenger's moves don't move them: the goal waits for a while.
        let since = *t.waiting.get_or_insert(now);
        if now - since > WAIT {
            stop(t);
        }
        return None;
    }
    let goal = t.goals[0].feet;
    if (goal - here).magnitude() < THERE {
        arrive(t, now);
        return None;
    }
    match t.route_for {
        Some(r) if (r - goal).magnitude() <= 1e-9 => {}
        // A goal that moved on a little (a tween, a fly) along a line that
        // was open: the line goes on to it, each step checked as it goes.
        Some(r) if t.route.len() == 1 && !t.stuck && (r - goal).magnitude() < 2.0 => {
            t.route_for = Some(goal);
            t.route[0] = if lim.collide {
                room_at(game, goal).unwrap_or(goal)
            } else {
                goal
            };
        }
        _ => route_to(t, game, &lim, here, goal, now),
    }
    // This frame's share of the tick's move, and what is left of it.
    let budget = lim.per_tick - (here - t.anchor).magnitude();
    let share = lim.per_tick * (dt / TICK).max(0.25);
    let mut allow = budget.min(share);
    let mut at = here;
    let mut moved = false;
    let mut warning = None;
    while allow > 1e-9 {
        let Some(&next) = t.route.first() else { break };
        let way = next - at;
        let dist = way.magnitude();
        if dist < THERE {
            t.route.remove(0);
            continue;
        }
        let mut to = if dist <= allow { next } else { at + way * (allow / dist) };
        // Through the air too long without falling: this step dips.
        if lim.collide && t.air >= AIR_TICKS && to.y - here.y >= -0.03125 {
            let dipped = Vector3::new(to.x, here.y - DIP, to.z);
            if (dipped - here).magnitude() <= budget && clear_way(game, at, dipped) {
                to = dipped;
                t.air = 0;
            }
        }
        if lim.collide && !clear_way(game, at, to) {
            // Blocked: as far as it is open, and a new way next time.
            let open = open_part(game, at, to);
            if (open - at).magnitude() > THERE {
                at = open;
                moved = true;
            }
            if now - t.planned_at >= PLAN_EVERY {
                t.route_for = None;
            }
            let since = *t.waiting.get_or_insert(now);
            if now - since > WAIT {
                // A walk or a fly into a wall just stops; a long way says why.
                if (goal - at).magnitude() > NEAR {
                    warning = Some(blocked(goal, at));
                }
                stop(t);
            }
            break;
        }
        allow -= (to - at).magnitude();
        at = to;
        moved = true;
        t.waiting = None;
        if (to - next).magnitude() < THERE {
            t.route.remove(0);
        }
    }
    if moved {
        game.move_player([at.x, at.y, at.z]);
    }
    if t.route.is_empty() && !t.goals.is_empty() {
        if t.stuck {
            // As near as it gets.
            if (goal - at).magnitude() > NEAR {
                warning = Some(blocked(goal, at));
            }
            t.goals.pop_front();
            t.route_for = None;
            t.stuck = false;
        } else {
            // At the route's end: the goal, or the room nearest it.
            arrive(t, now);
        }
    }
    warning
}

fn blocked(goal: Vector3, at: Vector3) -> String {
    format!(
        "the way to {:.1}, {:.1}, {:.1} is blocked: the player stopped at {:.1}, {:.1}, {:.1}, as near as the server lets them",
        goal.x, goal.y, goal.z, at.x, at.y, at.z
    )
}

/// At the first goal: the last one ends the moves; another is stayed at
/// for as long as the script held it, then the next one is gone to.
fn arrive(t: &mut Travel, now: f64) {
    t.route.clear();
    t.route_for = None;
    t.waiting = None;
    let since = *t.arrived.get_or_insert(now);
    if t.goals.len() <= 1 {
        // Kept, in case the script moves them on later (`want`).
        t.done = t.goals.front().map(|g| (g.since, since));
        stop(t);
        return;
    }
    let held = t.goals[1].since - t.goals[0].since;
    if now - since >= held {
        t.goals.pop_front();
        t.arrived = None;
    }
}

/// The way from `here` to `goal`: a line where it is open (or the server
/// lets the player through blocks, or the goal is near), else up over what
/// is in the way, else a path the search finds, else as far as it is open.
fn route_to(t: &mut Travel, game: &mut dyn GameApi, lim: &MoveLimits, here: Vector3, goal: Vector3, now: f64) {
    t.route_for = Some(goal);
    t.planned_at = now;
    t.stuck = false;
    let goal = if lim.collide { room_at(game, goal) } else { Some(goal) };
    let Some(goal) = goal else {
        // Nowhere to stand there: as near as the line gets.
        t.route = vec![open_part(game, here, t.goals[0].feet)];
        t.stuck = true;
        return;
    };
    if !lim.collide || (goal - here).magnitude() <= NEAR || clear_way(game, here, goal) {
        t.route = vec![goal];
        return;
    }
    let top = here.y.max(goal.y);
    for up in OVER {
        let a = Vector3::new(here.x, top + up, here.z);
        let b = Vector3::new(goal.x, top + up, goal.z);
        if clear_way(game, here, a) && clear_way(game, a, b) && clear_way(game, b, goal) {
            t.route = vec![a, b, goal];
            return;
        }
    }
    if now - t.searched_at >= PLAN_EVERY || t.searched_at == 0.0 {
        t.searched_at = now;
        if let Some(path) = search(game, here, goal) {
            t.route = path;
            return;
        }
    }
    t.route = vec![open_part(game, here, goal)];
    t.stuck = true;
}

/// Where the player can stand nearest `feet`: there, or up to three blocks
/// higher (out of what it is in), or a block to a side.
fn room_at(game: &mut dyn GameApi, feet: Vector3) -> Option<Vector3> {
    if clear_at(game, feet) {
        return Some(feet);
    }
    for k in 1..=6 {
        let up = Vector3::new(feet.x, (feet.y + 0.5 * f64::from(k)).floor(), feet.z);
        if clear_at(game, up) {
            return Some(up);
        }
    }
    for (dx, dz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let side = Vector3::new(feet.x + dx, feet.y, feet.z + dz);
        if clear_at(game, side) {
            return Some(side);
        }
    }
    None
}

/// Whether the player's box standing at `feet` is clear of blocks (or not
/// known: the server will say).
pub fn clear_at(game: &mut dyn GameApi, feet: Vector3) -> bool {
    // A hair smaller, so a box standing on a block or by a wall isn't in it.
    let h = WIDTH / 2.0 - 1e-3;
    let min = [feet.x - h, feet.y + 1e-3, feet.z - h];
    let max = [feet.x + h, feet.y + HEIGHT - 1e-3, feet.z + h];
    game.box_clear(min, max) != Some(false)
}

/// Whether the player's box goes from `a` to `b` without meeting a block
/// (one it is in at `a` aside: the server lets a player out of one).
pub fn clear_way(game: &mut dyn GameApi, a: Vector3, b: Vector3) -> bool {
    let len = (b - a).magnitude();
    let n = (len / SAMPLE).ceil().max(1.0) as usize;
    let mut out = clear_at(game, a);
    for i in 1..=n {
        let at = a.lerp(b, i as f64 / n as f64);
        let ok = clear_at(game, at);
        if !ok && out {
            return false;
        }
        out |= ok;
    }
    true
}

/// How far along the line from `a` to `b` the box goes before it meets a block.
fn open_part(game: &mut dyn GameApi, a: Vector3, b: Vector3) -> Vector3 {
    let len = (b - a).magnitude();
    let n = (len / SAMPLE).ceil().max(1.0) as usize;
    let mut last = a;
    let mut out = clear_at(game, a);
    for i in 1..=n {
        let at = a.lerp(b, i as f64 / n as f64);
        let ok = clear_at(game, at);
        if !ok && out {
            return last;
        }
        out |= ok;
        last = at;
    }
    b
}

// ---------------------------------------------------------------- the search

type Cell = (i32, i32, i32);

/// A cell's feet: the middle of its floor.
fn feet_of(c: Cell) -> Vector3 {
    Vector3::new(f64::from(c.0) + 0.5, f64::from(c.1), f64::from(c.2) + 0.5)
}

fn cell_of(v: Vector3) -> Cell {
    (v.x.floor() as i32, (v.y + 1e-3).floor() as i32, v.z.floor() as i32)
}

/// A* over the cells the player's box fits in (face neighbours, and
/// diagonals in the level whose sides are open), from the cell at `here`
/// to the one at `goal`, at most MAX_NODES cells; then the path pulled
/// straight where the line between its points is open.
fn search(game: &mut dyn GameApi, here: Vector3, goal: Vector3) -> Option<Vec<Vector3>> {
    #[derive(PartialEq)]
    struct Open(f64, Cell);
    impl Eq for Open {}
    impl PartialOrd for Open {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Open {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0)
        }
    }
    let mut fits: HashMap<Cell, bool> = HashMap::new();
    let mut fit = |game: &mut dyn GameApi, c: Cell| *fits.entry(c).or_insert_with(|| clear_at(game, feet_of(c)));
    let start = [cell_of(here), {
        let c = cell_of(here);
        (c.0, c.1 + 1, c.2)
    }]
    .into_iter()
    .find(|&c| fit(game, c) && clear_way(game, here, feet_of(c)))?;
    let end = cell_of(goal);
    if !fit(game, end) || !clear_way(game, feet_of(end), goal) {
        return None;
    }
    let lo = (
        start.0.min(end.0) - MARGIN,
        start.1.min(end.1) - MARGIN,
        start.2.min(end.2) - MARGIN,
    );
    let hi = (
        start.0.max(end.0) + MARGIN,
        start.1.max(end.1) + MARGIN,
        start.2.max(end.2) + MARGIN,
    );
    let inside = |c: Cell| (lo.0..=hi.0).contains(&c.0) && (lo.1..=hi.1).contains(&c.1) && (lo.2..=hi.2).contains(&c.2);
    let h = |c: Cell| (feet_of(c) - feet_of(end)).magnitude();
    let mut open = BinaryHeap::new();
    let mut came: HashMap<Cell, Cell> = HashMap::new();
    let mut cost: HashMap<Cell, f64> = HashMap::new();
    cost.insert(start, 0.0);
    open.push(Open(h(start), start));
    let mut tried = 0;
    while let Some(Open(_, c)) = open.pop() {
        if c == end {
            let mut cells = vec![c];
            let mut at = c;
            while let Some(&prev) = came.get(&at) {
                cells.push(prev);
                at = prev;
            }
            cells.reverse();
            let mut points: Vec<Vector3> = cells.into_iter().map(feet_of).collect();
            points.push(goal);
            return Some(pull(game, here, &points));
        }
        tried += 1;
        if tried > MAX_NODES {
            return None;
        }
        let g = cost[&c];
        let mut next = Vec::with_capacity(10);
        for d in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0)] {
            next.push(((c.0 + d.0, c.1 + d.1, c.2 + d.2), 1.0));
        }
        for (dx, dz) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            if fit(game, (c.0 + dx, c.1, c.2)) && fit(game, (c.0, c.1, c.2 + dz)) {
                next.push(((c.0 + dx, c.1, c.2 + dz), std::f64::consts::SQRT_2));
            }
        }
        for (n, step) in next {
            if !inside(n) || !fit(game, n) {
                continue;
            }
            let ng = g + step;
            if cost.get(&n).is_none_or(|&old| ng < old) {
                cost.insert(n, ng);
                came.insert(n, c);
                open.push(Open(ng + h(n), n));
            }
        }
    }
    None
}

/// A path's points with those the line can skip left out: from each point
/// kept, on to the farthest one an open line reaches.
fn pull(game: &mut dyn GameApi, here: Vector3, points: &[Vector3]) -> Vec<Vector3> {
    let mut out = Vec::new();
    let mut from = here;
    let mut i = 0;
    while i < points.len() {
        let mut j = i;
        while j + 1 < points.len() && clear_way(game, from, points[j + 1]) {
            j += 1;
        }
        out.push(points[j]);
        from = points[j];
        i = j + 1;
    }
    out
}

/// The blocks the player walks through (no collision shape), by their id:
/// for a game that says no more of a block than its id.
pub fn walk_through(id: &str) -> bool {
    let b = id.trim_start_matches("minecraft:");
    const ENDS: &[&str] = &[
        "air",
        "grass",
        "fern",
        "torch",
        "sign",
        "button",
        "sapling",
        "flower",
        "tulip",
        "pressure_plate",
        "rail",
        "banner",
        "vine",
        "vines",
        "coral",
        "coral_fan",
        "mushroom",
        "carpet",
    ];
    const IS: &[&str] = &[
        "water",
        "lava",
        "fire",
        "soul_fire",
        "cobweb",
        "dandelion",
        "poppy",
        "blue_orchid",
        "allium",
        "azure_bluet",
        "oxeye_daisy",
        "cornflower",
        "lily_of_the_valley",
        "wither_rose",
        "sunflower",
        "lilac",
        "rose_bush",
        "peony",
        "dead_bush",
        "seagrass",
        "tall_seagrass",
        "kelp",
        "kelp_plant",
        "sugar_cane",
        "wheat",
        "carrots",
        "potatoes",
        "beetroots",
        "nether_wart",
        "redstone_wire",
        "tripwire",
        "lever",
        "hanging_roots",
        "crimson_roots",
        "warped_roots",
        "light",
        "structure_void",
        "bubble_column",
    ];
    IS.contains(&b) || (ENDS.iter().any(|e| b.ends_with(e)) && !b.ends_with("_block") && b != "grass_block")
}
