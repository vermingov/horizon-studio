// Models attached to bodies (`model.*`): items and coloured boxes that follow a
// body part of the player (or of another player or a mob) as it moves and
// animates, drawn with the world each frame.
//
// A part's frame is where the game says the part is this frame
// (`GameApi::part_frame`): its origin (the part's pivot) and its left, up and
// forward axes, a block long (a model pixel is 1/16 of an axis). What a mod
// gives is in pixels: `offset` and box corners as {right, up, forward},
// `rotation` as {pitch, yaw, roll} in degrees (pitch tips the top forward, yaw
// turns it to its left, roll tilts it to its right), `scale` a number or one
// per axis. An item worn on the head or held in a hand starts where the game
// puts a helmet item or a held item (the "head" and "hand" displays); the
// mod's offset, rotation and scale apply from there.

use super::hstudio::{GameApi, PartFrame, add, color_arg, scale as scaled};
use super::hstudio_lua::{Lua, Table, Throw, Value, bad_arg};
use super::hstudio_meshes::Store;

/// Attachments one mod may have, and boxes in one.
pub const MAX_MODELS: usize = 64;
pub const MAX_BOXES: usize = 256;

/// The player's body parts (other models name theirs: a mob's own part names
/// work for it).
pub const PLAYER_PARTS: &[&str] = &[
    "root",
    "head",
    "body",
    "right_arm",
    "left_arm",
    "right_leg",
    "left_leg",
    "hat",
    "jacket",
    "right_sleeve",
    "left_sleeve",
    "right_pants",
    "left_pants",
];

/// How an item is shown (the game's item display contexts).
pub const DISPLAYS: &[&str] = &["head", "hand", "left_hand", "fixed", "ground", "gui"];

#[derive(Clone, Debug, PartialEq)]
pub struct Cuboid {
    /// Opposite corners, in pixels {right, up, forward} from the part's pivot.
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub argb: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub id: u32,
    /// None: the player; else an entity's id.
    pub target: Option<u32>,
    pub part: String,
    pub item: Option<String>,
    /// The item's display; None: the part's own ("head" on the head, "hand"
    /// on an arm, else "fixed").
    pub display: Option<String>,
    pub boxes: Vec<Cuboid>,
    /// A mesh of its mod's (`mesh.*`), its x, y and z the part's left, up
    /// and forward in pixels.
    pub mesh: Option<u32>,
    pub offset: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
    /// Shown in first person too (the player's own body isn't).
    pub first_person: bool,
    /// Full bright, not lit by the world.
    pub glow: bool,
    pub visible: bool,
}

impl Attachment {
    pub fn new(id: u32, part: &str) -> Attachment {
        Attachment {
            id,
            target: None,
            part: part.to_owned(),
            item: None,
            display: None,
            boxes: Vec::new(),
            mesh: None,
            offset: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
            first_person: false,
            glow: false,
            visible: true,
        }
    }

    /// The item's display, from the part when none is given.
    pub fn display(&self) -> &str {
        match self.display.as_deref() {
            Some(d) => d,
            None => match self.part.as_str() {
                "head" | "hat" => "head",
                "right_arm" | "right_sleeve" => "hand",
                "left_arm" | "left_sleeve" => "left_hand",
                _ => "fixed",
            },
        }
    }
}

/// A part name: letters, digits and `_`.
pub fn check_part(lua: &Lua, part: &str, target: Option<u32>, fname: &str) -> Result<(), Throw> {
    let ok = !part.is_empty() && part.len() <= 40 && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ok || (target.is_none() && !PLAYER_PARTS.contains(&part)) {
        return Err(bad_arg(
            lua,
            0,
            fname,
            &format!(
                "a body part expected: {} (a mob: its model's part names)",
                PLAYER_PARTS.join(", ")
            ),
        ));
    }
    Ok(())
}

/// A vector from {x, y, z} or {1, 2, 3}, or a number for all three.
fn vec3(v: &Value, all: bool) -> Option<[f64; 3]> {
    match v {
        Value::Num(n) if all => Some([*n; 3]),
        Value::Table(t) => {
            let t = t.borrow();
            let get = |k: &str, i: i64| {
                let named = t.get_str(k);
                if named.is_nil() {
                    t.get_int(i).to_num()
                } else {
                    named.to_num()
                }
            };
            Some([get("x", 1)?, get("y", 2)?, get("z", 3)?])
        }
        _ => None,
    }
}

/// The target a spec names: "player" (or nil) or an entity id.
pub fn target_of(lua: &Lua, v: &Value, fname: &str) -> Result<Option<Option<u32>>, Throw> {
    Ok(match v {
        Value::Nil => None,
        Value::Str(s) if &**s == b"player" => Some(None),
        Value::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= f64::from(u32::MAX) => Some(Some(*n as u32)),
        _ => {
            return Err(lua.rt_error(format!(
                "{fname}: target is \"player\" or an entity id (world.entities gives them)"
            )));
        }
    })
}

/// Applies the fields a spec table has to an attachment (others stay).
pub fn apply_spec(lua: &Lua, a: &mut Attachment, spec: &Table, fname: &str) -> Result<(), Throw> {
    let field = |k: &str| spec.get_str(k);
    let err = |what: &str| lua.rt_error(format!("{fname}: {what}"));
    match field("item") {
        Value::Nil => {}
        Value::Bool(false) => a.item = None,
        Value::Str(s) => {
            let id = String::from_utf8_lossy(&s).into_owned();
            if id.is_empty() || id.len() > 96 || !id.chars().all(|c| c.is_ascii_alphanumeric() || "_:./-".contains(c)) {
                return Err(err("item is an item id such as \"minecraft:diamond_sword\""));
            }
            a.item = Some(if id.contains(':') {
                id
            } else {
                format!("minecraft:{id}")
            });
        }
        _ => {
            return Err(err(
                "item is an item id such as \"minecraft:diamond_sword\" (or false for none)",
            ));
        }
    }
    match field("display") {
        Value::Nil => {}
        Value::Str(s) => {
            let d = String::from_utf8_lossy(&s).into_owned();
            if !DISPLAYS.contains(&d.as_str()) {
                return Err(err(&format!("display is one of {}", DISPLAYS.join(", "))));
            }
            a.display = Some(d);
        }
        _ => return Err(err(&format!("display is one of {}", DISPLAYS.join(", ")))),
    }
    let boxes = field("boxes");
    if let Value::Table(list) = &boxes {
        let items = list.borrow().list();
        if items.len() > MAX_BOXES {
            return Err(err(&format!("at most {MAX_BOXES} boxes")));
        }
        let mut out = Vec::with_capacity(items.len());
        for (i, b) in items.iter().enumerate() {
            let Value::Table(bt) = b else {
                return Err(err(&format!(
                    "boxes[{}] is {{from = {{x, y, z}}, to = {{x, y, z}}, color = ...}}",
                    i + 1
                )));
            };
            let bt = bt.borrow();
            let (Some(from), Some(to)) = (vec3(&bt.get_str("from"), false), vec3(&bt.get_str("to"), false)) else {
                return Err(err(&format!(
                    "boxes[{}] needs from and to, each {{x, y, z}} in pixels",
                    i + 1
                )));
            };
            let color = bt.get_str("color");
            let argb = if color.is_nil() {
                0xFFFF_FFFF
            } else {
                color_arg(lua, &[color], 0, 0xFFFF_FFFF)?
            };
            out.push(Cuboid { from, to, argb });
        }
        a.boxes = out;
    } else if !boxes.is_nil() && !matches!(boxes, Value::Bool(false)) {
        return Err(err(
            "boxes is a list of {from = {x, y, z}, to = {x, y, z}, color = ...}",
        ));
    } else if matches!(boxes, Value::Bool(false)) {
        a.boxes.clear();
    }
    match field("mesh") {
        Value::Nil => {}
        Value::Bool(false) => a.mesh = None,
        v => match v.to_num() {
            Some(n) if n >= 1.0 && n.fract() == 0.0 => a.mesh = Some(n as u32),
            _ => return Err(err("mesh is a mesh id (mesh.new, mesh.load...) or false for none")),
        },
    }
    for (k, slot, all) in [("offset", 0, false), ("rotation", 1, false), ("scale", 2, true)] {
        let v = field(k);
        if v.is_nil() {
            continue;
        }
        let Some(x) = vec3(&v, all) else {
            return Err(err(&format!(
                "{k} is {{x, y, z}}{}",
                if all { " or a number" } else { "" }
            )));
        };
        if x.iter().any(|n| !n.is_finite() || n.abs() > 4096.0) {
            return Err(err(&format!("{k} is out of range")));
        }
        match slot {
            0 => a.offset = x,
            1 => a.rotation = x,
            _ => a.scale = x,
        }
    }
    for (k, slot) in [("first_person", 0), ("glow", 1), ("visible", 2)] {
        match field(k) {
            Value::Nil => {}
            Value::Bool(b) => match slot {
                0 => a.first_person = b,
                1 => a.glow = b,
                _ => a.visible = b,
            },
            _ => return Err(err(&format!("{k} is true or false"))),
        }
    }
    if let Some(t) = target_of(lua, &field("target"), fname)? {
        a.target = t;
    }
    Ok(())
}

/// A 3x3 matrix, columns.
type M3 = [[f64; 3]; 3];

fn mul(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn mul_m(a: &M3, b: &M3) -> M3 {
    [mul(a, b[0]), mul(a, b[1]), mul(a, b[2])]
}

fn rx(t: f64) -> M3 {
    let (s, c) = t.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]
}
fn ry(t: f64) -> M3 {
    let (s, c) = t.sin_cos();
    [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]
}
fn rz(t: f64) -> M3 {
    let (s, c) = t.sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// {pitch, yaw, roll} in degrees as a rotation in the part's {left, up,
/// forward} space: yaw outermost, then pitch, then roll.
pub fn rotation(r: [f64; 3]) -> M3 {
    mul_m(
        &mul_m(&ry(r[1].to_radians()), &rx(r[0].to_radians())),
        &rz(r[2].to_radians()),
    )
}

/// Pixels {right, up, forward} as the part's {left, up, forward}.
fn luf(p: [f64; 3]) -> [f64; 3] {
    [-p[0], p[1], p[2]]
}

/// Where the game places an item for a display, in the part's {left, up,
/// forward} pixels and turn: vanilla's CustomHeadLayer (head) and
/// ItemInHandLayer (hands).
pub fn preset(display: &str) -> ([f64; 3], M3) {
    let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    match display {
        "head" => {
            let s = 0.625;
            (
                [0.0, 4.0, 0.0],
                mul_m(
                    &ry(std::f64::consts::PI),
                    &[[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]],
                ),
            )
        }
        "hand" | "left_hand" => {
            let x = if display == "hand" { -1.0 } else { 1.0 };
            (
                [x, -10.0, 2.0],
                mul_m(&rx(std::f64::consts::FRAC_PI_2), &ry(std::f64::consts::PI)),
            )
        }
        _ => ([0.0; 3], id),
    }
}

/// The part's axes as a matrix (columns left, up, forward).
fn axes(f: &PartFrame) -> M3 {
    [f.left, f.up, f.forward]
}

/// Draws one attachment at its part's frame, its mesh from its mod's
/// `meshes`; `budget` is the world quads left.
pub fn draw(game: &mut dyn GameApi, f: &PartFrame, a: &Attachment, meshes: &Store, budget: &mut usize) {
    let r = rotation(a.rotation);
    let world = |v_luf_px: [f64; 3]| add(f.origin, scaled(mul(&axes(f), v_luf_px), 1.0 / 16.0));
    let light = (!a.glow).then_some(f.origin);
    let off = luf(a.offset);
    if let Some(mesh) = a.mesh.and_then(|id| meshes.meshes.get(&id)) {
        // A mesh's x, y and z are already the part's left, up and forward.
        let place = |p: [f32; 3]| {
            let v = [
                f64::from(p[0]) * a.scale[0],
                f64::from(p[1]) * a.scale[1],
                f64::from(p[2]) * a.scale[2],
            ];
            world(add(mul(&r, v), off))
        };
        super::hstudio_meshes::draw(game, meshes, mesh, place, 0xFFFF_FFFF, None, light, budget);
    }
    for b in &a.boxes {
        if *budget < 6 {
            return;
        }
        *budget -= 6;
        let corner = |i: usize| {
            let p = [
                if i & 1 == 0 { b.from[0] } else { b.to[0] },
                if i & 2 == 0 { b.from[1] } else { b.to[1] },
                if i & 4 == 0 { b.from[2] } else { b.to[2] },
            ];
            let v = luf([p[0] * a.scale[0], p[1] * a.scale[1], p[2] * a.scale[2]]);
            world(add(mul(&r, v), off))
        };
        let c: Vec<[f64; 3]> = (0..8).map(corner).collect();
        let centre = scaled(c.iter().fold([0.0; 3], |s, p| add(s, *p)), 1.0 / 8.0);
        // The six faces, each with its outward normal.
        for face in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            let q = [c[face[0]], c[face[1]], c[face[2]], c[face[3]]];
            let mid = scaled(add(add(q[0], q[1]), add(q[2], q[3])), 0.25);
            let out = add(mid, scaled(centre, -1.0));
            game.world_quad_lit(q, b.argb, out, light);
        }
    }
    if let Some(item) = &a.item {
        let display = a.display();
        let (t, base) = preset(display);
        let s = [[a.scale[0], 0.0, 0.0], [0.0, a.scale[1], 0.0], [0.0, 0.0, a.scale[2]]];
        let m = mul_m(&mul_m(&mul_m(&axes(f), &r), &s), &base);
        let origin = world(add(t, off));
        let _ = game.world_item(item, [m[0], m[1], m[2], origin], display, light);
    }
}

/// A part frame as a table: x, y, z and its left, up and forward axes.
pub fn frame_table(f: &PartFrame) -> Value {
    let v = |p: [f64; 3]| {
        let mut t = Table::new();
        t.set_str("x", Value::Num(p[0]));
        t.set_str("y", Value::Num(p[1]));
        t.set_str("z", Value::Num(p[2]));
        Value::table(t)
    };
    let mut t = Table::new();
    t.set_str("x", Value::Num(f.origin[0]));
    t.set_str("y", Value::Num(f.origin[1]));
    t.set_str("z", Value::Num(f.origin[2]));
    t.set_str("left", v(f.left));
    t.set_str("up", v(f.up));
    t.set_str("forward", v(f.forward));
    Value::table(t)
}
