// The Roblox layer's building handles (hstudio_rbx_handles): Handles,
// ArcHandles and the Dragger, as building tools (F3X) use them to move,
// resize and rotate parts.
//
// Handles with an Adornee are drawn over everything, as Roblox draws them:
// a knob off each face of the part (Right, Top, Back, Left, Bottom, Front),
// in its Color3. ArcHandles draw a ring about each of the part's axes, with
// two knobs on it, red, green and blue. A press of the left button on a knob
// (the crosshair's, or the free pointer's) fires MouseButton1Down(face) or
// (axis); while it is held, each frame the pointer moved fires MouseDrag:
// a Handles' with how far along the face's normal the pointer has gone
// since the press (the nearest point of its ray to the line through the
// part's centre), an ArcHandles' with the angle it has turned about the
// axis (in the plane through the centre) and how much farther from the
// centre it is; MouseButton1Up as it comes up. The line, the plane and the
// centre are where they were at the press, so a tool moving the part from
// where it was keeps the drag steady.
//
// A Dragger (Roblox's old one, which F3X's move tool drags with) moves the
// parts it was given at MouseDown(part, pointOnPart, parts) so that the
// point held follows the ray MouseMove(ray) is given, on what it meets
// (leaving the parts out): the parts rest on the surface the ray meets, a
// whole stud along it from where they were. AxisRotate(axis) turns them a
// quarter about the point held; MouseUp lets them go.

use super::hstudio::GameApi;
use super::hstudio_lua::{Lua, Res, Value};
use super::hstudio_rbx::{Env, Id, Rbx, is_a};
use super::hstudio_rbx_draw::adorn_prop;
use super::hstudio_rbx_tools::{Drag, HandleDrag};
use super::hstudio_rbx_types::{CFrame, Color3, Vector3};
use super::hstudio_rbx_values::{Data, as_c3, as_cf, as_item, as_v3, cf, enum_item, enum_value, get};

/// How far a Handles' knob is off its face, in studs.
const GAP: f64 = 1.0;
/// How near the pointer must be to a knob to take it, in dp.
const REACH: f32 = 12.0;
/// A knob's size on the screen, in dp.
const KNOB: f32 = 10.0;
/// The dots a ring is drawn with.
const RING_DOTS: usize = 32;
/// The faces, as NormalIds, and their normals in the part's own space.
const FACES: [(&str, [f64; 3]); 6] = [
    ("Right", [1.0, 0.0, 0.0]),
    ("Top", [0.0, 1.0, 0.0]),
    ("Back", [0.0, 0.0, 1.0]),
    ("Left", [-1.0, 0.0, 0.0]),
    ("Bottom", [0.0, -1.0, 0.0]),
    ("Front", [0.0, 0.0, -1.0]),
];
/// The axes, the one their ring's knobs are along, and their colours.
const AXES: [(&str, [f64; 3], [f64; 3], u32); 3] = [
    ("X", [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0xFFE5_3935),
    ("Y", [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 0xFF43_A047),
    ("Z", [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], 0xFF1E_88E5),
];

fn v(a: [f64; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn alive(rbx: &Rbx, id: Id) -> bool {
    rbx.try_inst(id).is_some_and(|i| !i.destroyed)
}

/// The Handles and ArcHandles shown: (them, whether they are ArcHandles,
/// their adornee's frame and size).
fn shown(rbx: &Rbx) -> Vec<(Id, bool, CFrame, Vector3)> {
    let mut out = Vec::new();
    for inst in rbx.insts.values() {
        let arc = match inst.class.name {
            "Handles" => false,
            "ArcHandles" => true,
            _ => continue,
        };
        let id = inst.id;
        if inst.destroyed || !(rbx.is_descendant_of(id, rbx.game) || rbx.is_descendant_of(id, rbx.hui)) {
            continue;
        }
        if !adorn_prop(rbx, id, "Visible").truthy() {
            continue;
        }
        let Some(part) = rbx
            .id_of(&adorn_prop(rbx, id, "Adornee"))
            .filter(|&p| alive(rbx, p) && is_a(rbx.inst(p).class, "BasePart"))
        else {
            continue;
        };
        let frame = as_cf(&rbx.raw_get(part, "CFrame")).unwrap_or_default();
        let size = as_v3(&rbx.raw_get(part, "Size")).unwrap_or(Vector3::ONE);
        out.push((id, arc, frame, size));
    }
    out.sort_by_key(|h| h.0);
    out
}

/// A Handles' or an ArcHandles' knobs: (its face or axis, where the knob
/// is, the face's normal or the axis, in the world).
fn knobs(arc: bool, frame: CFrame, size: Vector3) -> Vec<(&'static str, Vector3, Vector3)> {
    let at = frame.position();
    let half = size * 0.5;
    if arc {
        let r = ring_radius(size);
        AXES.iter()
            .flat_map(|&(name, axis, along, _)| {
                let a = frame.vector_to_world(v(axis));
                let b = frame.vector_to_world(v(along));
                [(name, at + b * r, a), (name, at - b * r, a)]
            })
            .collect()
    } else {
        FACES
            .iter()
            .map(|&(name, n)| {
                let reach = (v(n) * half).magnitude() + GAP;
                let dir = frame.vector_to_world(v(n));
                (name, at + dir * reach, dir)
            })
            .collect()
    }
}

/// An ArcHandles' rings' radius about a part of `size`.
fn ring_radius(size: Vector3) -> f64 {
    (size * 0.5).magnitude() + 0.5
}

/// Where the pointer is (dp): the free pointer's place, else the crosshair.
fn pointer(env: &mut dyn Env) -> (f64, f64) {
    let p = &env.rbx().pointer;
    let (free, x, y) = (p.free || p.chat_open, p.x, p.y);
    if free {
        return (x, y);
    }
    let (w, h, _) = env.game().dp();
    (f64::from(w) / 2.0, f64::from(h) / 2.0)
}

/// The nearest point to the ray `o` + `d`s on the line `c` + `a`t, as its t
/// (None when they are parallel).
fn along_line(c: Vector3, a: Vector3, o: Vector3, d: Vector3) -> Option<f64> {
    let w = c - o;
    let b = a.dot(d);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-6 {
        return None;
    }
    Some((b * d.dot(w) - a.dot(w)) / denom)
}

/// Where the ray `o` + `d`s meets the plane through `c` across `n`.
fn on_plane(c: Vector3, n: Vector3, o: Vector3, d: Vector3) -> Option<Vector3> {
    let denom = n.dot(d);
    if denom.abs() < 1e-6 {
        return None;
    }
    let s = n.dot(c - o) / denom;
    (s >= 0.0).then(|| o + d * s)
}

/// A press of the left button: on a knob of Handles or ArcHandles shown,
/// it takes hold of them (the nearest knob under the pointer) and their
/// MouseButton1Down fires. Whether it did.
pub fn press(env: &mut dyn Env) -> bool {
    let list = shown(env.rbx());
    if list.is_empty() {
        return false;
    }
    let (px, py) = pointer(env);
    let mut best: Option<(f64, Id, bool, &'static str, CFrame, Vector3)> = None;
    for &(id, arc, frame, size) in &list {
        for (which, at, dir) in knobs(arc, frame, size) {
            let (s, _) = super::hstudio_rbx_draw::to_screen(env, at);
            let near = (f64::from(REACH)).powi(2) >= (s.x - px).powi(2) + (s.y - py).powi(2);
            if s.z > 0.0 && near && best.as_ref().is_none_or(|b| s.z < b.0) {
                best = Some((s.z, id, arc, which, frame, dir));
            }
        }
    }
    let Some((_, id, arc, which, frame, dir)) = best else {
        return false;
    };
    let (o, d) = super::hstudio_rbx_draw::screen_ray(env, px, py);
    let centre = frame.position();
    let (start, from) = if arc {
        let q = on_plane(centre, dir, o, d).unwrap_or(centre + dir.cross(Vector3::Y_AXIS));
        (0.0, q - centre)
    } else {
        (along_line(centre, dir, o, d).unwrap_or(0.0), Vector3::ZERO)
    };
    let rbx = env.rbx();
    rbx.tools.handle = Some(HandleDrag {
        handles: id,
        arc,
        which,
        centre,
        dir,
        start,
        from,
        last: (0.0, 0.0),
    });
    rbx.fire(id, "MouseButton1Down", vec![which_value(arc, which)]);
    true
}

fn which_value(arc: bool, which: &'static str) -> Value {
    enum_value(enum_item(if arc { "Axis" } else { "NormalId" }, which))
}

/// The left button came up: the Handles held let go (MouseButton1Up).
/// Whether there were any.
pub fn release(rbx: &mut Rbx) -> bool {
    let Some(h) = rbx.tools.handle.take() else {
        return false;
    };
    rbx.fire(h.handles, "MouseButton1Up", vec![which_value(h.arc, h.which)]);
    true
}

/// Each frame Handles are held: MouseDrag, when the pointer's ray says
/// something new.
pub fn drag(env: &mut dyn Env) {
    let Some(h) = env.rbx().tools.handle.clone() else {
        return;
    };
    if !alive(env.rbx(), h.handles) {
        env.rbx().tools.handle = None;
        return;
    }
    let (px, py) = pointer(env);
    let (o, d) = super::hstudio_rbx_draw::screen_ray(env, px, py);
    let now = if h.arc {
        let Some(q) = on_plane(h.centre, h.dir, o, d) else {
            return;
        };
        let to = q - h.centre;
        let angle = h.from.cross(to).dot(h.dir).atan2(h.from.dot(to));
        (angle, to.magnitude() - h.from.magnitude())
    } else {
        let Some(t) = along_line(h.centre, h.dir, o, d) else {
            return;
        };
        (t - h.start, 0.0)
    };
    if (now.0 - h.last.0).abs() < 1e-6 && (now.1 - h.last.1).abs() < 1e-6 {
        return;
    }
    let rbx = env.rbx();
    if let Some(held) = rbx.tools.handle.as_mut() {
        held.last = now;
    }
    let mut args = vec![which_value(h.arc, h.which), Value::Num(now.0)];
    if h.arc {
        args.push(Value::Num(now.1));
    }
    rbx.fire(h.handles, "MouseDrag", args);
}

fn argb(c: Color3, alpha: f64) -> u32 {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(alpha) << 24) | (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
}

/// The Handles and ArcHandles shown, over everything (on the HUD, in dp).
pub fn draw(rbx: &Rbx, game: &mut dyn GameApi) {
    if !rbx.active {
        return;
    }
    let list = shown(rbx);
    if list.is_empty() {
        return;
    }
    let (w, h, _) = game.dp();
    let Some(cam) = game.camera() else { return };
    let dot = |game: &mut dyn GameApi, p: Vector3, size: f32, c: u32| {
        if let Some((x, y, _)) = cam.project([p.x, p.y, p.z], w, h) {
            game.dp_rect([x - size / 2.0, y - size / 2.0, size, size], size / 2.0, c);
        }
    };
    for (id, arc, frame, size) in list {
        let alpha = 1.0 - adorn_prop(rbx, id, "Transparency").to_num().unwrap_or(0.0);
        if alpha <= 0.0 {
            continue;
        }
        if arc {
            let r = ring_radius(size);
            for &(_, axis, along, c) in &AXES {
                let a = frame.vector_to_world(v(axis));
                let b = frame.vector_to_world(v(along));
                let third = a.cross(b);
                for i in 0..RING_DOTS {
                    let t = i as f64 / RING_DOTS as f64 * std::f64::consts::TAU;
                    let p = frame.position() + (b * t.cos() + third * t.sin()) * r;
                    dot(game, p, 3.0, c);
                }
            }
            for (which, at, _) in knobs(true, frame, size) {
                let c = AXES.iter().find(|a| a.0 == which).map_or(0xFFFF_FFFF, |a| a.3);
                dot(game, at, KNOB, c);
            }
        } else {
            let c = as_c3(&adorn_prop(rbx, id, "Color3")).unwrap_or(Color3::new(0.05, 0.41, 0.67));
            for (_, at, _) in knobs(false, frame, size) {
                dot(game, at, KNOB, argb(c, alpha));
            }
        }
    }
}

// ---------------------------------------------------------------- the Dragger

/// Dragger:MouseDown(part, pointOnPart, parts), MouseMove(ray), MouseUp()
/// and AxisRotate(axis).
pub fn dragger(env: &mut dyn Env, lua: &mut Lua, id: Id, name: &str, args: &[Value]) -> Res {
    let rbx = env.rbx();
    match name {
        "MouseDown" => {
            let part = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .ok_or_else(|| lua.rt_error("Dragger:MouseDown(part, pointOnPart, parts): part expected"))?;
            let point = args.get(2).and_then(as_v3).unwrap_or_default();
            let mut parts: Vec<Id> = match args.get(3) {
                Some(Value::Table(t)) => t.borrow().list().iter().filter_map(|v| rbx.id_of(v)).collect(),
                _ => Vec::new(),
            };
            parts.retain(|&p| is_a(rbx.inst(p).class, "BasePart"));
            if !parts.contains(&part) && is_a(rbx.inst(part).class, "BasePart") {
                parts.push(part);
            }
            rbx.tools.draggers.retain(|d| d.0 != id);
            rbx.tools.draggers.push((id, Drag { parts, part, point }));
        }
        "MouseUp" => rbx.tools.draggers.retain(|d| d.0 != id),
        "MouseMove" => {
            let Some(Data::Ray(o, d)) = args.get(1).and_then(get) else {
                return Err(lua.rt_error("Dragger:MouseMove(ray): Ray expected"));
            };
            if let Some(drag) = rbx.tools.draggers.iter().find(|d| d.0 == id).map(|d| d.1.clone()) {
                move_along(env, &drag, o, d);
            }
        }
        "AxisRotate" => {
            let axis = args.get(1).and_then(as_item).map_or("Y", |i| i.name);
            if let Some(drag) = rbx.tools.draggers.iter().find(|d| d.0 == id).map(|d| d.1.clone()) {
                turn(rbx, &drag, axis);
            }
        }
        _ => return Err(lua.rt_error(format!("{name} is not available here"))),
    }
    Ok(Vec::new())
}

/// The parts dragged moved so that the point held is where the ray meets
/// what isn't them, resting on it, a whole stud along it from where they
/// were.
fn move_along(env: &mut dyn Env, drag: &Drag, o: Vector3, d: Vector3) {
    let rbx = env.rbx();
    let parts: Vec<Id> = drag.parts.iter().copied().filter(|&p| alive(rbx, p)).collect();
    if parts.is_empty() || !alive(rbx, drag.part) {
        return;
    }
    let filter = super::hstudio_rbx_query::Filter {
        ids: parts.clone(),
        include: false,
        ignore_water: true,
    };
    let unit = if d.magnitude() > 0.0 { d.unit() } else { return };
    let Some(hit) = super::hstudio_rbx_query::cast(env, o, unit * 1000.0, &filter) else {
        return;
    };
    let rbx = env.rbx();
    let frame_of = |p: Id| as_cf(&rbx.raw_get(p, "CFrame")).unwrap_or_default();
    let held = frame_of(drag.part) * drag.point;
    let n = hit.normal;
    // How far the parts reach behind the point held, against the normal.
    let mut behind: f64 = 0.0;
    for &p in &parts {
        let f = frame_of(p);
        let half = as_v3(&rbx.raw_get(p, "Size")).unwrap_or(Vector3::ONE) * 0.5;
        for i in 0..8 {
            let corner = Vector3::new(
                if i & 1 == 0 { -half.x } else { half.x },
                if i & 2 == 0 { -half.y } else { half.y },
                if i & 4 == 0 { -half.z } else { half.z },
            );
            behind = behind.max((held - f * corner).dot(n));
        }
    }
    let mut delta = hit.at + n * behind - held;
    // A whole stud along the surface.
    let snap = |x: f64, axis: f64| if axis.abs() > 0.9 { x } else { x.round() };
    delta = Vector3::new(snap(delta.x, n.x), snap(delta.y, n.y), snap(delta.z, n.z));
    if delta.magnitude() < 1e-9 {
        return;
    }
    let moved: Vec<(Id, CFrame)> = parts
        .iter()
        .map(|&p| (p, CFrame::from_position(delta) * frame_of(p)))
        .collect();
    for (p, f) in moved {
        rbx.raw_set(p, "CFrame", cf(f));
    }
}

/// AxisRotate: the parts dragged turned a quarter about the point held.
fn turn(rbx: &mut Rbx, drag: &Drag, axis: &str) {
    let Some(f) = as_cf(&rbx.raw_get(drag.part, "CFrame")) else {
        return;
    };
    let about = f * drag.point;
    let q = std::f64::consts::FRAC_PI_2;
    let r = match axis {
        "X" => CFrame::angles(q, 0.0, 0.0),
        "Z" => CFrame::angles(0.0, 0.0, q),
        _ => CFrame::angles(0.0, q, 0.0),
    };
    let around = CFrame::from_position(about) * r * CFrame::from_position(about * -1.0);
    for &p in &drag.parts {
        if let Some(pf) = as_cf(&rbx.raw_get(p, "CFrame")) {
            rbx.raw_set(p, "CFrame", cf(around * pf));
        }
    }
}
