// What scripts ask of the world (hstudio_rbx_query): rays through the
// world's blocks and the Workspace's parts, the mirrored entities' among them
// (workspace:Raycast, Roblox's older FindPartOnRay and its kin, the Mouse's
// Hit and Target), and the bounds queries (GetPartBoundsInRadius,
// GetPartBoundsInBox, GetPartsInPart). A stud is a block. The local
// character is left out of rays, which start at its eyes, unless an Include
// filter names it.

use super::hstudio_lua::{Lua, Res, Table, TableRef, Value, bad_arg, check_num};
use super::hstudio_rbx::{Env, Id, Rbx, is_a};
use super::hstudio_rbx_draw::{block_material, first_block};
use super::hstudio_rbx_types::{CFrame, Vector3};
use super::hstudio_rbx_values::{Data, as_cf, as_item, as_v3, enum_item, enum_value, get, v3};

/// The farthest a ray goes, in blocks.
const MAX_RAY: f64 = 5000.0;

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

/// The Workspace's parts a ray (`ray`) or a query can meet: BaseParts but the
/// Terrain, with CanQuery, that the filter lets through. A ray leaves out the
/// local character unless an Include filter names it.
fn parts(rbx: &Rbx, filter: &Filter, ray: bool) -> Vec<Id> {
    rbx.descendants(rbx.workspace)
        .into_iter()
        .filter(|&id| {
            let inst = rbx.inst(id);
            is_a(inst.class, "BasePart")
                && inst.class.name != "Terrain"
                && inst.get("CanQuery").truthy()
                && filter.passes(rbx, id)
                && !(ray && rbx.is_descendant_of(id, rbx.character) && !filter.include)
        })
        .collect()
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
    let mut best: Option<(f64, Vector3, Vector3, Id)> = None;
    for id in parts(rbx, filter, true) {
        if let Some((t, face, normal)) = ray_box(rbx, id, origin, unit, length)
            && best.is_none_or(|b| t < b.0)
        {
            best = Some((t, face, normal, id));
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

/// Where a ray (unit `unit`, `length` long) meets a part's box: how far
/// along, and the face's normal in the part's space and in the world's.
fn ray_box(rbx: &Rbx, id: Id, origin: Vector3, unit: Vector3, length: f64) -> Option<(f64, Vector3, Vector3)> {
    let inst = rbx.inst(id);
    let frame = as_cf(&inst.get("CFrame")).unwrap_or_default();
    let half = as_v3(&inst.get("Size")).unwrap_or(Vector3::ONE) / 2.0;
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
    match cast(env, origin, dir * 500.0, &filter) {
        Some(hit) => {
            let (target, _) = what_was_hit(env.rbx(), &hit);
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
    let filter = Filter::of(rbx, params.as_ref());
    let most = params
        .as_ref()
        .and_then(|p| p.borrow().get_str("MaxParts").to_num())
        .filter(|n| *n >= 1.0)
        .map_or(usize::MAX, |n| n as usize);
    let found: Vec<Id> = parts(rbx, &filter, false)
        .into_iter()
        .filter(|&id| Some(id) != not)
        .filter(|&id| {
            let (lo, hi) = bounds(rbx, id);
            meets(lo, hi)
        })
        .take(most)
        .collect();
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
