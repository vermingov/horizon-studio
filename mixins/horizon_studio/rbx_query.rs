// What scripts ask of the world (hstudio_rbx_query): rays through the
// world's blocks and the Workspace's parts, the mirrored entities' among them
// (workspace:Raycast, Roblox's older FindPartOnRay and its kin, the Mouse's
// Hit and Target), and the bounds queries (GetPartBoundsInRadius,
// GetPartBoundsInBox, GetPartsInPart). A stud is a block. The local
// character is left out of rays, which start at its eyes, unless an Include
// filter names it. The parts are boxed once while nothing in the Workspace
// moves, so that a frame's many rays and queries don't each walk it. And the
// map, read through workspace.Terrain: a cell is a block (GetBlock,
// ReadVoxels).

use super::hstudio_lua::{Lua, Res, Table, TableRef, Value, bad_arg, check_num};
use super::hstudio_rbx::{C, Env, Id, Rbx, is_a};
use super::hstudio_rbx_draw::{block_material, first_block, passable};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{Data, as_cf, as_item, as_v3, enum_item, enum_value, get, v3};

/// The farthest a ray goes, in blocks.
const MAX_RAY: f64 = 5000.0;
/// The most cells, and blocks in them, one Terrain:ReadVoxels reads.
pub const MAX_CELLS: usize = 4096;
pub const MAX_BLOCKS: usize = 65536;
/// How far out from 0, 0 the world's blocks go, each way.
const WORLD_EDGE: f64 = 30_000_000.0;

// ---------------------------------------------------------------- filters

/// Which parts a ray or a query meets: with `include`, only `ids` and what is
/// in them; else all but those. A ray meets the world's blocks when the
/// filter lets the Terrain through. `ignore_water`: water doesn't stop a ray.
#[derive(Default)]
pub struct Filter {
    pub ids: Vec<Id>,
    pub include: bool,
    pub ignore_water: bool,
}

impl Filter {
    /// RaycastParams' or OverlapParams' filter (a table of their fields).
    pub fn of(rbx: &Rbx, params: Option<&TableRef>) -> Filter {
        let Some(p) = params else {
            return Filter::default();
        };
        let p = p.borrow();
        let ids = match p.get_str("FilterDescendantsInstances") {
            Value::Table(t) => t.borrow().list().iter().filter_map(|v| rbx.id_of(v)).collect(),
            _ => Vec::new(),
        };
        Filter {
            ids,
            include: as_item(&p.get_str("FilterType")).is_some_and(|i| i.value == 1),
            ignore_water: p.get_str("IgnoreWater").truthy(),
        }
    }

    /// Whether it names `id`, or something `id` is in.
    fn names(&self, rbx: &Rbx, id: Id) -> bool {
        self.ids.iter().any(|&f| f == id || rbx.is_descendant_of(id, f))
    }

    /// Whether it lets `id` through.
    fn passes(&self, rbx: &Rbx, id: Id) -> bool {
        self.names(rbx, id) == self.include
    }
}

// ---------------------------------------------------------------- the parts, boxed

/// The Workspace's parts a ray or a query can meet (BaseParts but the
/// Terrain, with CanQuery), worked out once and kept while nothing in the
/// Workspace moves: the world's entities and the character move as each
/// frame begins, and what a script changes goes through Rbx::changed
/// (`changed`). Each part has its place, half its size and the box it fills
/// in the world. The parts under each of the Workspace's children (a
/// model's, a mob's) are a group with a box around them all, so that a ray
/// or a query that misses a group's box passes all its parts by.
#[derive(Default)]
pub struct Boxes {
    /// Whether they are as the Workspace is.
    pub fresh: bool,
    parts: Vec<Boxed>,
    groups: Vec<Group>,
    /// The walk's stack: instances, and whether they are in the character.
    stack: Vec<(Id, bool)>,
    /// The classes met, and whether their instances are parts rays meet.
    kinds: Vec<(&'static C, bool)>,
}

/// A part as rays and queries meet it, and whether it is in the local
/// character.
struct Boxed {
    id: Id,
    frame: CFrame,
    half: Vector3,
    lo: Vector3,
    hi: Vector3,
    mine: bool,
}

/// The parts under one of the Workspace's children (`parts[start..end]`),
/// and the box around them.
struct Group {
    lo: Vector3,
    hi: Vector3,
    start: usize,
    end: usize,
}

impl Boxes {
    /// The Workspace's parts as they are now, in its order.
    fn build(&mut self, rbx: &Rbx) {
        self.parts.clear();
        self.groups.clear();
        for &top in rbx.children(rbx.workspace) {
            let start = self.parts.len();
            self.stack.clear();
            self.stack.push((top, top == rbx.character));
            while let Some((id, mine)) = self.stack.pop() {
                let Some(inst) = rbx.try_inst(id) else {
                    continue;
                };
                let part = match self.kinds.iter().find(|k| std::ptr::eq(k.0, inst.class)) {
                    Some(k) => k.1,
                    None => {
                        let part = inst.class.name != "Terrain" && is_a(inst.class, "BasePart");
                        self.kinds.push((inst.class, part));
                        part
                    }
                };
                if part && inst.props.get("CanQuery").is_some_and(Value::truthy) {
                    let frame = inst.props.get("CFrame").and_then(as_cf).unwrap_or_default();
                    let half = inst.props.get("Size").and_then(as_v3).unwrap_or(Vector3::ONE) / 2.0;
                    let (lo, hi) = oriented_bounds(frame, half.abs());
                    self.parts.push(Boxed {
                        id,
                        frame,
                        half,
                        lo,
                        hi,
                        mine,
                    });
                }
                let character = rbx.character;
                self.stack
                    .extend(inst.children.iter().rev().map(|&c| (c, mine || c == character)));
            }
            if let Some(first) = self.parts.get(start) {
                let (lo, hi) = self.parts[start..]
                    .iter()
                    .fold((first.lo, first.hi), |(lo, hi), b| (lo.min(b.lo), hi.max(b.hi)));
                self.groups.push(Group {
                    lo,
                    hi,
                    start,
                    end: self.parts.len(),
                });
            }
        }
        self.fresh = true;
    }
}

/// The Workspace's parts, boxed, worked out again if something moved.
fn ready(rbx: &mut Rbx) {
    if !rbx.world.boxes.fresh {
        let mut boxes = std::mem::take(&mut rbx.world.boxes);
        boxes.build(rbx);
        rbx.world.boxes = boxes;
    }
}

/// A property of `id` changed (Rbx::changed): the boxes are stale when the
/// change can move what rays and queries meet, a part's place, size or
/// CanQuery, or where anything but a GUI object is.
pub fn changed(rbx: &mut Rbx, id: Id, name: &str) {
    if !rbx.world.boxes.fresh {
        return;
    }
    let class = rbx.try_inst(id).map(|i| i.class);
    let moves = match name {
        "Parent" => class.is_none_or(|c| !is_a(c, "GuiBase2d")),
        "CFrame" | "Size" | "CanQuery" => class.is_some_and(|c| is_a(c, "BasePart")),
        _ => false,
    };
    if moves {
        rbx.world.boxes.fresh = false;
    }
}

// ---------------------------------------------------------------- rays

/// Where a ray met something: the part (None for a block of the world's,
/// `block`), the point, the face's normal in the world and in the part's own
/// space (a block's are the same), and how far along the ray.
pub struct Hit {
    pub part: Option<Id>,
    pub block: Option<String>,
    pub at: Vector3,
    pub normal: Vector3,
    pub face: Vector3,
    pub distance: f64,
}

/// Where a ray from `origin` along `dir` (as long as it goes, up to 5000
/// blocks) first meets a part or a solid block that the filter lets through.
pub fn cast(env: &mut dyn Env, origin: Vector3, dir: Vector3, filter: &Filter) -> Option<Hit> {
    let length = dir.magnitude().min(MAX_RAY);
    if length <= 0.0 || !length.is_finite() {
        return None;
    }
    let unit = dir.unit();
    let rbx = env.rbx();
    ready(rbx);
    let rbx: &Rbx = rbx;
    let boxes = &rbx.world.boxes;
    let inv = Vector3::new(1.0 / unit.x, 1.0 / unit.y, 1.0 / unit.z);
    let mut best: Option<(f64, Vector3, Vector3, Id)> = None;
    for g in &boxes.groups {
        // A group farther than the nearest part met yet has none nearer.
        if !ray_meets(origin, unit, inv, best.map_or(length, |b| b.0), g.lo, g.hi) {
            continue;
        }
        for b in &boxes.parts[g.start..g.end] {
            // The local character is left out unless an Include filter names it.
            if b.mine && !filter.include || !filter.passes(rbx, b.id) {
                continue;
            }
            if let Some((t, face, normal)) = ray_box(b.frame, b.half, origin, unit, length)
                && best.is_none_or(|x| t < x.0)
            {
                best = Some((t, face, normal, b.id));
            }
        }
    }
    let terrain = rbx.id_of(&rbx.raw_get(rbx.workspace, "Terrain"));
    let blocks = terrain.is_none_or(|t| filter.passes(rbx, t));
    let limit = best.map_or(length, |b| b.0);
    let block = if blocks {
        first_block(env.game(), origin, unit, limit, filter.ignore_water)
    } else {
        None
    };
    match (block, best) {
        (Some((t, n, b)), _) if t <= limit => Some(Hit {
            part: None,
            block: Some(b),
            at: origin + unit * t,
            normal: n,
            face: n,
            distance: t,
        }),
        (_, Some((t, face, normal, id))) => Some(Hit {
            part: Some(id),
            block: None,
            at: origin + unit * t,
            normal,
            face,
            distance: t,
        }),
        _ => None,
    }
}

/// Whether a ray (`unit` and its inverse `inv`, as far as `reach`) meets a
/// box in the world (its least and most corners), with a hair to spare.
fn ray_meets(origin: Vector3, unit: Vector3, inv: Vector3, reach: f64, lo: Vector3, hi: Vector3) -> bool {
    const HAIR: f64 = 1e-6;
    let (mut t0, mut t1) = (0.0f64, reach + HAIR);
    for (o, d, i, l, h) in [
        (origin.x, unit.x, inv.x, lo.x, hi.x),
        (origin.y, unit.y, inv.y, lo.y, hi.y),
        (origin.z, unit.z, inv.z, lo.z, hi.z),
    ] {
        let (l, h) = (l - HAIR, h + HAIR);
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return false;
            }
            continue;
        }
        let (a, b) = ((l - o) * i, (h - o) * i);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Where a ray (unit `unit`, `length` long) meets a part's box (at `frame`,
/// `half` its size): how far along, and the face's normal in the part's
/// space and in the world's.
fn ray_box(
    frame: CFrame,
    half: Vector3,
    origin: Vector3,
    unit: Vector3,
    length: f64,
) -> Option<(f64, Vector3, Vector3)> {
    let o = frame.point_to_object(origin);
    let d = frame.vector_to_object(unit);
    let (mut t0, mut t1) = (0.0f64, length);
    let mut normal = Vector3::ZERO;
    for (axis, (oc, dc, hc)) in [(o.x, d.x, half.x), (o.y, d.y, half.y), (o.z, d.z, half.z)]
        .into_iter()
        .enumerate()
    {
        if dc.abs() < 1e-12 {
            if oc.abs() > hc {
                return None;
            }
            continue;
        }
        let (mut a, mut b) = ((-hc - oc) / dc, (hc - oc) / dc);
        let mut sign = -1.0;
        if a > b {
            std::mem::swap(&mut a, &mut b);
            sign = 1.0;
        }
        if a > t0 {
            t0 = a;
            let mut n = [0.0; 3];
            n[axis] = sign;
            normal = Vector3::new(n[0], n[1], n[2]);
        }
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, normal, frame.vector_to_world(normal)))
}

/// workspace:Raycast: a RaycastResult table (Instance, Position, Normal,
/// Distance, Material, and Horizon's Block for a block of the world's), or
/// nil when the ray meets nothing.
pub fn raycast(env: &mut dyn Env, origin: Vector3, dir: Vector3, params: Option<&TableRef>) -> Value {
    let filter = Filter::of(env.rbx(), params);
    let Some(hit) = cast(env, origin, dir, &filter) else {
        return Value::Nil;
    };
    let rbx = env.rbx();
    let (instance, material) = what_was_hit(rbx, &hit);
    let mut t = Table::new();
    t.set_str("Position", v3(hit.at));
    t.set_str("Normal", v3(hit.normal));
    t.set_str("Distance", Value::Num(hit.distance));
    t.set_str("Instance", instance);
    t.set_str("Material", material);
    if let Some(b) = &hit.block {
        t.set_str("Block", Value::str(b));
    }
    Value::table(t)
}

/// The instance a ray met (the Terrain for a block) and its material.
fn what_was_hit(rbx: &Rbx, hit: &Hit) -> (Value, Value) {
    match (&hit.block, hit.part) {
        (Some(b), _) => (
            rbx.raw_get(rbx.workspace, "Terrain"),
            enum_value(enum_item("Material", block_material(b))),
        ),
        (None, Some(p)) => (rbx.value(p), rbx.raw_get(p, "Material")),
        (None, None) => (Value::Nil, enum_value(enum_item("Material", "Air"))),
    }
}

/// The Mouse's Hit, Target and TargetSurface along a ray from the camera
/// (`dir` a unit): where it meets a part or a block within 500 blocks (else
/// that far along), the part or the Terrain, and the face it meets. Its
/// TargetFilter, and what is in it, is left out.
pub fn mouse_hit(env: &mut dyn Env, origin: Vector3, dir: Vector3) -> (Vector3, Value, &'static str) {
    let rbx = env.rbx();
    let ignored = rbx.id_of(&rbx.raw_get(rbx.mouse, "TargetFilter"));
    let filter = Filter {
        ids: ignored.into_iter().collect(),
        include: false,
        ignore_water: true,
    };
    let hit = cast(env, origin, dir * 500.0, &filter);
    // The block met, a hair past where the ray enters it.
    let rbx = env.rbx();
    rbx.world.mouse_block = hit.as_ref().filter(|h| h.block.is_some()).map(|h| {
        let inside = h.at + dir * 1e-4;
        (
            inside.x.floor() as i32,
            inside.y.floor() as i32,
            inside.z.floor() as i32,
        )
    });
    match hit {
        Some(hit) => {
            let (target, _) = what_was_hit(rbx, &hit);
            (hit.at, target, surface(hit.face))
        }
        None => (origin + dir * 500.0, Value::Nil, "Top"),
    }
}

/// The NormalId of a face's normal, in its part's space.
fn surface(n: Vector3) -> &'static str {
    let a = n.abs();
    if a.x >= a.y && a.x >= a.z && a.x > 0.0 {
        if n.x > 0.0 { "Right" } else { "Left" }
    } else if a.y >= a.z && a.y > 0.0 {
        if n.y > 0.0 { "Top" } else { "Bottom" }
    } else if a.z > 0.0 {
        if n.z > 0.0 { "Back" } else { "Front" }
    } else {
        "Top"
    }
}

// ---------------------------------------------------------------- the Workspace's queries

/// The Workspace's bounds queries and its older rays. GetPartBoundsInRadius
/// (pos, radius), GetPartBoundsInBox(cframe, size) and GetPartsInPart(part)
/// give the parts whose boxes, as the world sees them, are within the radius
/// or overlap the box (`query`). FindPartOnRay(ray, ignore),
/// FindPartOnRayWithIgnoreList(ray, list) and FindPartOnRayWithWhitelist(ray,
/// list) give the part a ray meets (the Terrain for a block), where, the
/// face's normal and the material; nil and the ray's end when it meets
/// nothing.
pub fn workspace_method(env: &mut dyn Env, lua: &mut Lua, name: &str, args: &[Value]) -> Res {
    match name {
        "GetPartBoundsInRadius" => {
            let at = vector_arg(lua, args, 1, name)?;
            let r = check_num(lua, args, 2, name)?;
            query(env, args.get(3), None, |lo, hi| nearest(at, lo, hi).magnitude() <= r)
        }
        "GetPartBoundsInBox" => {
            let frame = args
                .get(1)
                .and_then(as_cf)
                .ok_or_else(|| bad_arg(lua, 1, name, "CFrame expected"))?;
            let size = vector_arg(lua, args, 2, name)?;
            let (a, b) = oriented_bounds(frame, size.abs() / 2.0);
            query(env, args.get(3), None, |lo, hi| overlap(a, b, lo, hi))
        }
        "GetPartsInPart" => {
            let rbx = env.rbx();
            let part = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .filter(|&p| is_a(rbx.inst(p).class, "BasePart"))
                .ok_or_else(|| bad_arg(lua, 1, name, "BasePart expected"))?;
            let (a, b) = bounds(rbx, part);
            query(env, args.get(2), Some(part), |lo, hi| overlap(a, b, lo, hi))
        }
        _ => find_part_on_ray(env, lua, name, args),
    }
}

/// The parts a bounds query finds: those whose boxes `meets`, but `not`, as
/// OverlapParams lets them (its filter, and MaxParts at most), in the
/// Workspace's order.
fn query(env: &mut dyn Env, params: Option<&Value>, not: Option<Id>, meets: impl Fn(Vector3, Vector3) -> bool) -> Res {
    let params = match params {
        Some(Value::Table(t)) => Some(t.clone()),
        _ => None,
    };
    let rbx = env.rbx();
    ready(rbx);
    let rbx: &Rbx = rbx;
    let filter = Filter::of(rbx, params.as_ref());
    let most = params
        .as_ref()
        .and_then(|p| p.borrow().get_str("MaxParts").to_num())
        .filter(|n| *n >= 1.0)
        .map_or(usize::MAX, |n| n as usize);
    let boxes = &rbx.world.boxes;
    let mut found: Vec<Id> = Vec::new();
    // A group's box holds its parts': one it doesn't meet has none that do.
    'groups: for g in boxes.groups.iter().filter(|g| meets(g.lo, g.hi)) {
        for b in &boxes.parts[g.start..g.end] {
            if Some(b.id) != not && filter.passes(rbx, b.id) && meets(b.lo, b.hi) {
                found.push(b.id);
                if found.len() >= most {
                    break 'groups;
                }
            }
        }
    }
    Ok(vec![rbx.list(&found)])
}

/// FindPartOnRay(ray[, ignore[, cubes[, ignoreWater]]]), its IgnoreList's
/// list of what to leave out, and its Whitelist's (ray, list[,
/// ignoreWater]) of what alone to meet.
fn find_part_on_ray(env: &mut dyn Env, lua: &mut Lua, name: &str, args: &[Value]) -> Res {
    let Some(Data::Ray(origin, dir)) = args.get(1).and_then(get) else {
        return Err(bad_arg(lua, 1, name, "Ray expected"));
    };
    let rbx = env.rbx();
    let listed = |v: Option<&Value>| -> Vec<Id> {
        match v {
            Some(Value::Table(t)) => t.borrow().list().iter().filter_map(|x| rbx.id_of(x)).collect(),
            _ => Vec::new(),
        }
    };
    let (ids, include, water) = match name {
        "FindPartOnRay" => (args.get(2).and_then(|v| rbx.id_of(v)).into_iter().collect(), false, 4),
        "FindPartOnRayWithIgnoreList" => (listed(args.get(2)), false, 4),
        _ => (listed(args.get(2)), true, 3),
    };
    let filter = Filter {
        ids,
        include,
        ignore_water: args.get(water).is_some_and(Value::truthy),
    };
    match cast(env, origin, dir, &filter) {
        Some(hit) => {
            let (part, material) = what_was_hit(env.rbx(), &hit);
            Ok(vec![part, v3(hit.at), v3(hit.normal), material])
        }
        None => Ok(vec![
            Value::Nil,
            v3(origin + dir),
            v3(Vector3::ZERO),
            enum_value(enum_item("Material", "Air")),
        ]),
    }
}

/// A Vector3 argument.
fn vector_arg(lua: &Lua, args: &[Value], i: usize, name: &str) -> Result<Vector3, super::hstudio_lua::Throw> {
    args.get(i)
        .and_then(as_v3)
        .ok_or_else(|| bad_arg(lua, i, name, "Vector3 expected"))
}

/// A part's box as the world sees it: its least and most corners.
fn bounds(rbx: &Rbx, id: Id) -> (Vector3, Vector3) {
    let inst = rbx.inst(id);
    let frame = as_cf(&inst.get("CFrame")).unwrap_or_default();
    let half = as_v3(&inst.get("Size")).unwrap_or(Vector3::ONE).abs() / 2.0;
    oriented_bounds(frame, half)
}

/// The box around a turned box of half-size `half` at `frame`.
fn oriented_bounds(frame: CFrame, half: Vector3) -> (Vector3, Vector3) {
    let r = frame.rot;
    let reach = |row: [f64; 3]| row[0].abs() * half.x + row[1].abs() * half.y + row[2].abs() * half.z;
    let ext = Vector3::new(reach(r[0]), reach(r[1]), reach(r[2]));
    let c = frame.position();
    (c - ext, c + ext)
}

/// Whether two boxes (least and most corners) overlap.
fn overlap(a: Vector3, b: Vector3, lo: Vector3, hi: Vector3) -> bool {
    a.x <= hi.x && lo.x <= b.x && a.y <= hi.y && lo.y <= b.y && a.z <= hi.z && lo.z <= b.z
}

/// From `p` to the nearest point of a box.
fn nearest(p: Vector3, lo: Vector3, hi: Vector3) -> Vector3 {
    p.max(lo).min(hi) - p
}

// ---------------------------------------------------------------- the Terrain

/// workspace.Terrain's methods: the terrain is the map, Minecraft's blocks,
/// and a cell is a block. GetBlock(pos) is Horizon's own: the id of the
/// block there, or nil where nothing is loaded. ReadVoxels(region,
/// resolution) reads its cells (`read_voxels`). WorldToCell (and its
/// PreferSolid and PreferEmpty) give a position's cell, CellCenterToWorld
/// and CellCornerToWorld a cell's centre and least corner. The rest would
/// change the world, which a command does here: they raise an error saying
/// so.
pub fn terrain_method(env: &mut dyn Env, lua: &mut Lua, name: &str, args: &[Value]) -> Res {
    match name {
        "GetBlock" => {
            let p = vector_arg(lua, args, 1, name)?.floor();
            let b = env.game().block(p.x as i32, p.y as i32, p.z as i32);
            Ok(vec![b.map_or(Value::Nil, |b| Value::str(&b))])
        }
        "ReadVoxels" => read_voxels(env, lua, args),
        "WorldToCell" | "WorldToCellPreferSolid" | "WorldToCellPreferEmpty" => {
            Ok(vec![v3(vector_arg(lua, args, 1, name)?.floor())])
        }
        "CellCenterToWorld" | "CellCornerToWorld" => {
            let n = |i| check_num(lua, args, i, name).map(f64::floor);
            let half = if name == "CellCenterToWorld" { 0.5 } else { 0.0 };
            Ok(vec![v3(Vector3::new(n(1)? + half, n(2)? + half, n(3)? + half))])
        }
        _ => Err(lua.rt_error(format!(
            "Terrain:{name} can't change the terrain here: it is Minecraft's blocks, which a command changes (game.run(\"fill ...\"))"
        ))),
    }
}

/// Terrain:ReadVoxels(region, resolution): the materials and occupancies of
/// the region's cells, `resolution` blocks a side (Roblox's are 4; 1 reads
/// block by block), as [x][y][z] arrays with a Size field (the number of
/// cells each way). The region is first grown out to the grid of the cells.
/// A cell's material is its most common solid block's (Air when it has
/// none; water is Water) and its occupancy how much of it is solid. At most
/// 4096 cells and 65536 blocks are read at once, charged to the script, and
/// none past the world's edge.
fn read_voxels(env: &mut dyn Env, lua: &mut Lua, args: &[Value]) -> Res {
    let name = "ReadVoxels";
    let Some(Data::Region3(region)) = args.get(1).and_then(get) else {
        return Err(bad_arg(lua, 1, name, "Region3 expected"));
    };
    let res = check_num(lua, args, 2, name)?.floor();
    if !(1.0..=64.0).contains(&res) {
        return Err(bad_arg(lua, 2, name, "a resolution of 1 to 64 blocks expected"));
    }
    let r = region.expand_to_grid(res);
    let inside = |v: Vector3| [v.x, v.y, v.z].iter().all(|c| c.abs() <= WORLD_EDGE);
    if !(inside(r.min) && inside(r.max)) {
        return Err(lua.rt_error(format!(
            "Terrain:ReadVoxels reads the world's blocks, which go {WORLD_EDGE:.0} blocks out from 0, 0 each way: this region goes past them"
        )));
    }
    let count = |a: f64, b: f64| ((b - a) / res).round().max(0.0) as usize;
    let (nx, ny, nz) = (
        count(r.min.x, r.max.x),
        count(r.min.y, r.max.y),
        count(r.min.z, r.max.z),
    );
    let cells = nx.saturating_mul(ny).saturating_mul(nz);
    let side = res as usize;
    let blocks = cells.saturating_mul(side * side * side);
    if cells > MAX_CELLS || blocks > MAX_BLOCKS {
        return Err(lua.rt_error(format!(
            "Terrain:ReadVoxels reads at most {MAX_CELLS} cells and {MAX_BLOCKS} blocks at once: this region has {cells} cells of {blocks} blocks"
        )));
    }
    lua.charge(blocks * 16)?;
    let game = env.game();
    let mut materials = Vec::with_capacity(nx);
    let mut occupancies = Vec::with_capacity(nx);
    for cx in 0..nx {
        let (mut mys, mut oys) = (Vec::with_capacity(ny), Vec::with_capacity(ny));
        for cy in 0..ny {
            let (mut mzs, mut ozs) = (Vec::with_capacity(nz), Vec::with_capacity(nz));
            for cz in 0..nz {
                let corner = r.min + Vector3::new(cx as f64, cy as f64, cz as f64) * res;
                let (material, solid) = cell(game, corner, side);
                mzs.push(enum_value(enum_item("Material", material)));
                ozs.push(Value::Num(solid as f64 / blocks_per(side)));
            }
            mys.push(Value::table(Table::from_list(mzs)));
            oys.push(Value::table(Table::from_list(ozs)));
        }
        materials.push(Value::table(Table::from_list(mys)));
        occupancies.push(Value::table(Table::from_list(oys)));
    }
    let size = v3(Vector3::new(nx as f64, ny as f64, nz as f64));
    let mut m = Table::from_list(materials);
    m.set_str("Size", size.clone());
    let mut o = Table::from_list(occupancies);
    o.set_str("Size", size);
    Ok(vec![Value::table(m), Value::table(o)])
}

/// A cell `side` blocks a side from `corner`: its most common solid block's
/// material (the first met of those as common), and how many are solid.
fn cell(game: &mut dyn super::hstudio::GameApi, corner: Vector3, side: usize) -> (&'static str, usize) {
    let mut tally: Vec<(&'static str, usize)> = Vec::new();
    let mut solid = 0;
    let (x0, y0, z0) = (corner.x as i32, corner.y as i32, corner.z as i32);
    for dx in 0..side as i32 {
        for dy in 0..side as i32 {
            for dz in 0..side as i32 {
                let Some(b) = game.block(x0 + dx, y0 + dy, z0 + dz) else {
                    continue;
                };
                if passable(&b) {
                    continue;
                }
                solid += 1;
                let m = block_material(&b);
                match tally.iter_mut().find(|t| t.0 == m) {
                    Some(t) => t.1 += 1,
                    None => tally.push((m, 1)),
                }
            }
        }
    }
    let most = tally
        .iter()
        .fold(None, |best: Option<(&'static str, usize)>, &t| match best {
            Some(b) if b.1 >= t.1 => Some(b),
            _ => Some(t),
        });
    (most.map_or("Air", |m| m.0), solid)
}

fn blocks_per(side: usize) -> f64 {
    (side * side * side) as f64
}
