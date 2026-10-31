// Roblox's data types for scripts (hstudio_rbx_values): Vector2, Vector3,
// CFrame, Color3, UDim, UDim2, Rect, NumberRange, NumberSequence,
// ColorSequence and their keypoints, TweenInfo, BrickColor, Font, Random,
// Ray, and the Enum tree, as userdata over hstudio_rbx_types. Each has
// Roblox's constructors (`Vector3.new`), properties (`v.Magnitude`), methods
// (`a:Lerp(b, t)`), operators and `tostring`, and `typeof` names it. Like
// Roblox's they can't be changed: a new value comes back instead.

use super::hstudio_lua::{
    Host, Lua, Native, Res, Table, TableRef, Throw, Userdata, Value, bad_arg, check_int, check_num, check_text, opt_num,
};
use super::hstudio_rbx_types::{
    Axis, BrickColor, CFrame, Color3, ColorSequence, ColorSequenceKeypoint, EasingDirection, EasingStyle, Font,
    FontEnum, FontStyle, FontWeight, NormalId, NumberRange, NumberSequence, NumberSequenceKeypoint, Random, Rect,
    RotationOrder, TweenInfo, UDim, UDim2, Vector2, Vector3,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ---------------------------------------------------------------- the values

/// An Enum item: `Enum.KeyCode.E`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnumItem {
    pub enum_name: &'static str,
    pub name: &'static str,
    pub value: i32,
}

/// What a data type userdata holds.
#[derive(Clone, Debug)]
pub enum Data {
    V2(Vector2),
    V3(Vector3),
    CF(CFrame),
    C3(Color3),
    UD(UDim),
    UD2(UDim2),
    Rect(Rect),
    Range(NumberRange),
    NumSeq(NumberSequence),
    NumKey(NumberSequenceKeypoint),
    ColorSeq(ColorSequence),
    ColorKey(ColorSequenceKeypoint),
    Tween(TweenInfo),
    Brick(BrickColor),
    Font(Font),
    Rng(Random),
    Ray(Vector3, Vector3),
    Item(EnumItem),
    /// `Enum.KeyCode`.
    EnumType(&'static str),
    /// `Enum`.
    Enums,
}

impl Data {
    /// Its `typeof`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Data::V2(_) => "Vector2",
            Data::V3(_) => "Vector3",
            Data::CF(_) => "CFrame",
            Data::C3(_) => "Color3",
            Data::UD(_) => "UDim",
            Data::UD2(_) => "UDim2",
            Data::Rect(_) => "Rect",
            Data::Range(_) => "NumberRange",
            Data::NumSeq(_) => "NumberSequence",
            Data::NumKey(_) => "NumberSequenceKeypoint",
            Data::ColorSeq(_) => "ColorSequence",
            Data::ColorKey(_) => "ColorSequenceKeypoint",
            Data::Tween(_) => "TweenInfo",
            Data::Brick(_) => "BrickColor",
            Data::Font(_) => "Font",
            Data::Rng(_) => "Random",
            Data::Ray(..) => "Ray",
            Data::Item(_) => "EnumItem",
            Data::EnumType(_) => "Enum",
            Data::Enums => "Enums",
        }
    }
}

thread_local! {
    /// One metatable per type, made once.
    static METAS: RefCell<HashMap<&'static str, TableRef>> = RefCell::new(HashMap::new());
    /// Enum items, made once each, so `==` and table keys work on them.
    static ITEMS: RefCell<HashMap<(&'static str, &'static str), Value>> = RefCell::new(HashMap::new());
}

fn meta(name: &'static str) -> TableRef {
    METAS.with(|m| {
        m.borrow_mut()
            .entry(name)
            .or_insert_with(|| {
                let mut t = Table::new();
                t.set_type_name(name);
                for (k, f) in [
                    ("__index", index as super::hstudio_lua::Builtin),
                    ("__newindex", newindex),
                    ("__tostring", tostring),
                    ("__eq", eq),
                    ("__add", add),
                    ("__sub", sub),
                    ("__mul", mul),
                    ("__div", div),
                    ("__idiv", idiv),
                    ("__unm", unm),
                    ("__lt", lt),
                    ("__le", le),
                ] {
                    t.set_str(k, Native::builtin(k, f));
                }
                t.set_str("__metatable", Value::str("The metatable is locked"));
                Rc::new(RefCell::new(t))
            })
            .clone()
    })
}

/// A userdata holding `d`.
pub fn wrap(d: Data) -> Value {
    if let Data::Item(item) = d {
        return enum_value(item);
    }
    let m = meta(d.type_name());
    Userdata::value(d, Some(m))
}

/// The one value of an Enum item.
pub fn enum_value(item: EnumItem) -> Value {
    ITEMS.with(|items| {
        items
            .borrow_mut()
            .entry((item.enum_name, item.name))
            .or_insert_with(|| Userdata::value(Data::Item(item), Some(meta("EnumItem"))))
            .clone()
    })
}

/// What a value holds, if it is one of these.
pub fn get(v: &Value) -> Option<Data> {
    match v {
        Value::Userdata(u) => u.data.borrow().downcast_ref::<Data>().cloned(),
        _ => None,
    }
}

pub fn v2(v: Vector2) -> Value {
    wrap(Data::V2(v))
}
pub fn v3(v: Vector3) -> Value {
    wrap(Data::V3(v))
}
pub fn cf(c: CFrame) -> Value {
    wrap(Data::CF(c))
}
pub fn c3(c: Color3) -> Value {
    wrap(Data::C3(c))
}
pub fn udim(u: UDim) -> Value {
    wrap(Data::UD(u))
}
pub fn udim2(u: UDim2) -> Value {
    wrap(Data::UD2(u))
}

pub fn as_v2(v: &Value) -> Option<Vector2> {
    match get(v)? {
        Data::V2(x) => Some(x),
        _ => None,
    }
}
pub fn as_v3(v: &Value) -> Option<Vector3> {
    match get(v)? {
        Data::V3(x) => Some(x),
        _ => None,
    }
}
pub fn as_cf(v: &Value) -> Option<CFrame> {
    match get(v)? {
        Data::CF(x) => Some(x),
        _ => None,
    }
}
pub fn as_c3(v: &Value) -> Option<Color3> {
    match get(v)? {
        Data::C3(x) => Some(x),
        _ => None,
    }
}
pub fn as_udim(v: &Value) -> Option<UDim> {
    match get(v)? {
        Data::UD(x) => Some(x),
        _ => None,
    }
}
pub fn as_udim2(v: &Value) -> Option<UDim2> {
    match get(v)? {
        Data::UD2(x) => Some(x),
        _ => None,
    }
}
pub fn as_item(v: &Value) -> Option<EnumItem> {
    match get(v)? {
        Data::Item(x) => Some(x),
        _ => None,
    }
}
pub fn as_font(v: &Value) -> Option<Font> {
    match get(v)? {
        Data::Font(x) => Some(x),
        _ => None,
    }
}
pub fn as_tween(v: &Value) -> Option<TweenInfo> {
    match get(v)? {
        Data::Tween(x) => Some(x),
        _ => None,
    }
}

/// "Vector3 expected, got string" for argument `i`.
fn want(lua: &Lua, args: &[Value], i: usize, fname: &str, what: &str) -> Throw {
    let got = args.get(i).map_or("no value".to_owned(), |v| lua.type_of(v));
    bad_arg(lua, i, fname, &format!("{what} expected, got {got}"))
}

fn arg_v3(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Vector3, Throw> {
    args.get(i)
        .and_then(as_v3)
        .ok_or_else(|| want(lua, args, i, fname, "Vector3"))
}
fn arg_v2(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Vector2, Throw> {
    args.get(i)
        .and_then(as_v2)
        .ok_or_else(|| want(lua, args, i, fname, "Vector2"))
}
fn arg_cf(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<CFrame, Throw> {
    args.get(i)
        .and_then(as_cf)
        .ok_or_else(|| want(lua, args, i, fname, "CFrame"))
}
fn arg_c3(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Color3, Throw> {
    args.get(i)
        .and_then(as_c3)
        .ok_or_else(|| want(lua, args, i, fname, "Color3"))
}
fn num_or(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<f64, Throw> {
    opt_num(lua, args, i, fname, 0.0)
}

/// An enum argument: the item, its name or its number.
pub fn arg_enum(lua: &Lua, args: &[Value], i: usize, fname: &str, enum_name: &str) -> Result<EnumItem, Throw> {
    args.get(i)
        .and_then(|v| coerce_enum(enum_name, v))
        .ok_or_else(|| want(lua, args, i, fname, &format!("Enum.{enum_name}")))
}

/// `v` as an item of `enum_name`: one of its items, a name or a number.
pub fn coerce_enum(enum_name: &str, v: &Value) -> Option<EnumItem> {
    let def = enum_def(enum_name)?;
    match v {
        Value::Str(s) => {
            let s = std::str::from_utf8(s).ok()?;
            def.items.iter().find(|i| i.0 == s).map(|i| item(def, i))
        }
        Value::Num(n) => def.items.iter().find(|i| f64::from(i.1) == *n).map(|i| item(def, i)),
        other => as_item(other).filter(|it| it.enum_name == def.name),
    }
}

fn item(def: &EnumDef, i: &(&'static str, i32)) -> EnumItem {
    EnumItem {
        enum_name: def.name,
        name: i.0,
        value: i.1,
    }
}

/// `Enum.<enum_name>.<name>`; panics on a name not in the tables (a bug).
pub fn enum_item(enum_name: &str, name: &str) -> EnumItem {
    let def = enum_def(enum_name).unwrap_or_else(|| panic!("no Enum.{enum_name}"));
    let i = def
        .items
        .iter()
        .find(|i| i.0 == name)
        .unwrap_or_else(|| panic!("no Enum.{enum_name}.{name}"));
    item(def, i)
}

// ---------------------------------------------------------------- constructors

/// The data types' global tables: `Vector3`, `CFrame`, ... and `Enum`.
pub fn globals() -> Vec<(&'static str, Value)> {
    let lib = |entries: &[(&'static str, super::hstudio_lua::Builtin)], values: Vec<(&'static str, Value)>| {
        let mut t = Table::new();
        for (name, f) in entries {
            t.set_str(name, Native::builtin(name, *f));
        }
        for (name, v) in values {
            t.set_str(name, v);
        }
        t.set_readonly(true);
        Value::table(t)
    };
    vec![
        (
            "Vector3",
            lib(
                &[
                    ("new", vector3_new),
                    ("FromNormalId", vector3_from_normal_id),
                    ("FromAxis", vector3_from_axis),
                ],
                vec![
                    ("zero", v3(Vector3::ZERO)),
                    ("one", v3(Vector3::ONE)),
                    ("xAxis", v3(Vector3::X_AXIS)),
                    ("yAxis", v3(Vector3::Y_AXIS)),
                    ("zAxis", v3(Vector3::Z_AXIS)),
                ],
            ),
        ),
        (
            "Vector2",
            lib(
                &[("new", vector2_new)],
                vec![
                    ("zero", v2(Vector2::ZERO)),
                    ("one", v2(Vector2::ONE)),
                    ("xAxis", v2(Vector2::X_AXIS)),
                    ("yAxis", v2(Vector2::Y_AXIS)),
                ],
            ),
        ),
        (
            "CFrame",
            lib(
                &[
                    ("new", cframe_new),
                    ("lookAt", cframe_look_at),
                    ("lookAlong", cframe_look_along),
                    ("Angles", cframe_angles),
                    ("fromEulerAnglesXYZ", cframe_angles),
                    ("fromEulerAnglesYXZ", cframe_yxz),
                    ("fromOrientation", cframe_orientation),
                    ("fromEulerAngles", cframe_euler),
                    ("fromAxisAngle", cframe_axis_angle),
                    ("fromMatrix", cframe_matrix),
                    ("fromRotationBetweenVectors", cframe_between),
                ],
                vec![("identity", cf(CFrame::IDENTITY))],
            ),
        ),
        (
            "Color3",
            lib(
                &[
                    ("new", color3_new),
                    ("fromRGB", color3_rgb),
                    ("fromHSV", color3_hsv),
                    ("fromHex", color3_hex),
                    ("toHSV", color3_to_hsv),
                ],
                vec![],
            ),
        ),
        ("UDim", lib(&[("new", udim_new)], vec![])),
        (
            "UDim2",
            lib(
                &[
                    ("new", udim2_new),
                    ("fromScale", udim2_scale),
                    ("fromOffset", udim2_offset),
                ],
                vec![],
            ),
        ),
        ("Rect", lib(&[("new", rect_new)], vec![])),
        ("NumberRange", lib(&[("new", range_new)], vec![])),
        ("NumberSequence", lib(&[("new", numseq_new)], vec![])),
        ("NumberSequenceKeypoint", lib(&[("new", numkey_new)], vec![])),
        ("ColorSequence", lib(&[("new", colorseq_new)], vec![])),
        ("ColorSequenceKeypoint", lib(&[("new", colorkey_new)], vec![])),
        ("TweenInfo", lib(&[("new", tween_new)], vec![])),
        (
            "BrickColor",
            lib(
                &[
                    ("new", brick_new),
                    ("palette", brick_palette),
                    ("random", brick_random),
                    ("White", brick_named),
                    ("Gray", brick_named),
                    ("DarkGray", brick_named),
                    ("Black", brick_named),
                    ("Red", brick_named),
                    ("Yellow", brick_named),
                    ("Green", brick_named),
                    ("Blue", brick_named),
                ],
                vec![],
            ),
        ),
        (
            "Font",
            lib(
                &[
                    ("new", font_new),
                    ("fromEnum", font_from_enum),
                    ("fromName", font_from_name),
                    ("fromId", font_from_id),
                ],
                vec![],
            ),
        ),
        ("Random", lib(&[("new", random_new)], vec![])),
        ("Ray", lib(&[("new", ray_new)], vec![])),
        ("RaycastParams", lib(&[("new", raycast_params_new)], vec![])),
        ("OverlapParams", lib(&[("new", raycast_params_new)], vec![])),
        ("Enum", wrap(Data::Enums)),
    ]
}

fn one(v: Value) -> Res {
    Ok(vec![v])
}

fn vector3_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = |i| num_or(lua, &args, i, "new");
    one(v3(Vector3::new(n(0)?, n(1)?, n(2)?)))
}

fn vector3_from_normal_id(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let face = arg_enum(lua, &args, 0, "FromNormalId", "NormalId")?;
    let face = NormalId::from_name(face.name).unwrap_or(NormalId::Front);
    one(v3(Vector3::from_normal_id(face)))
}

fn vector3_from_axis(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let axis = arg_enum(lua, &args, 0, "FromAxis", "Axis")?;
    let axis = Axis::from_name(axis.name).unwrap_or(Axis::X);
    one(v3(Vector3::from_axis(axis)))
}

fn vector2_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(v2(Vector2::new(
        num_or(lua, &args, 0, "new")?,
        num_or(lua, &args, 1, "new")?,
    )))
}

fn cframe_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if let Some(pos) = args.first().and_then(as_v3) {
        return one(cf(match args.get(1).and_then(as_v3) {
            Some(at) => CFrame::new_look(pos, at),
            None => CFrame::from_position(pos),
        }));
    }
    let mut nums = Vec::with_capacity(args.len());
    for i in 0..args.len() {
        nums.push(check_num(lua, &args, i, "new")?);
    }
    CFrame::from_numbers(&nums)
        .map(cf)
        .map(|v| vec![v])
        .map_err(|e| lua.rt_error(e))
}

fn cframe_look_at(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let at = arg_v3(lua, &args, 0, "lookAt")?;
    let target = arg_v3(lua, &args, 1, "lookAt")?;
    let up = args.get(2).and_then(as_v3).unwrap_or(Vector3::Y_AXIS);
    one(cf(CFrame::look_at(at, target, up)))
}

fn cframe_look_along(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let at = arg_v3(lua, &args, 0, "lookAlong")?;
    let dir = arg_v3(lua, &args, 1, "lookAlong")?;
    let up = args.get(2).and_then(as_v3).unwrap_or(Vector3::Y_AXIS);
    one(cf(CFrame::look_along(at, dir, up)))
}

fn three(lua: &Lua, args: &[Value], fname: &str) -> Result<(f64, f64, f64), Throw> {
    Ok((
        num_or(lua, args, 0, fname)?,
        num_or(lua, args, 1, fname)?,
        num_or(lua, args, 2, fname)?,
    ))
}

fn cframe_angles(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, y, z) = three(lua, &args, "Angles")?;
    one(cf(CFrame::angles(x, y, z)))
}

fn cframe_yxz(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, y, z) = three(lua, &args, "fromEulerAnglesYXZ")?;
    one(cf(CFrame::from_euler_angles_yxz(x, y, z)))
}

fn cframe_orientation(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, y, z) = three(lua, &args, "fromOrientation")?;
    one(cf(CFrame::from_orientation(x, y, z)))
}

fn cframe_euler(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, y, z) = three(lua, &args, "fromEulerAngles")?;
    let order = match args.get(3) {
        None | Some(Value::Nil) => RotationOrder::XYZ,
        Some(_) => {
            let it = arg_enum(lua, &args, 3, "fromEulerAngles", "RotationOrder")?;
            RotationOrder::from_name(it.name).unwrap_or(RotationOrder::XYZ)
        }
    };
    one(cf(CFrame::from_euler_angles(x, y, z, order)))
}

fn cframe_axis_angle(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let axis = arg_v3(lua, &args, 0, "fromAxisAngle")?;
    let angle = check_num(lua, &args, 1, "fromAxisAngle")?;
    one(cf(CFrame::from_axis_angle(axis, angle)))
}

fn cframe_matrix(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let pos = arg_v3(lua, &args, 0, "fromMatrix")?;
    let vx = arg_v3(lua, &args, 1, "fromMatrix")?;
    let vy = arg_v3(lua, &args, 2, "fromMatrix")?;
    let vz = args.get(3).and_then(as_v3);
    one(cf(CFrame::from_matrix(pos, vx, vy, vz)))
}

fn cframe_between(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let a = arg_v3(lua, &args, 0, "fromRotationBetweenVectors")?;
    let b = arg_v3(lua, &args, 1, "fromRotationBetweenVectors")?;
    one(cf(CFrame::from_rotation_between_vectors(a, b)))
}

fn color3_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (r, g, b) = three(lua, &args, "new")?;
    one(c3(Color3::new(r, g, b)))
}

fn color3_rgb(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (r, g, b) = three(lua, &args, "fromRGB")?;
    one(c3(Color3::from_rgb(r, g, b)))
}

fn color3_hsv(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (h, s, v) = three(lua, &args, "fromHSV")?;
    one(c3(Color3::from_hsv(h, s, v)))
}

fn color3_hex(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let hex = check_text(lua, &args, 0, "fromHex")?;
    Color3::from_hex(&hex).map(|c| vec![c3(c)]).map_err(|e| lua.rt_error(e))
}

fn color3_to_hsv(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (h, s, v) = arg_c3(lua, &args, 0, "toHSV")?.to_hsv();
    Ok(vec![Value::Num(h), Value::Num(s), Value::Num(v)])
}

fn udim_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(udim(UDim::new(
        num_or(lua, &args, 0, "new")?,
        num_or(lua, &args, 1, "new")?,
    )))
}

fn udim2_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if let (Some(x), Some(y)) = (args.first().and_then(as_udim), args.get(1).and_then(as_udim)) {
        return one(udim2(UDim2::from_udims(x, y)));
    }
    let n = |i| num_or(lua, &args, i, "new");
    one(udim2(UDim2::new(n(0)?, n(1)?, n(2)?, n(3)?)))
}

fn udim2_scale(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(udim2(UDim2::from_scale(
        num_or(lua, &args, 0, "fromScale")?,
        num_or(lua, &args, 1, "fromScale")?,
    )))
}

fn udim2_offset(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(udim2(UDim2::from_offset(
        num_or(lua, &args, 0, "fromOffset")?,
        num_or(lua, &args, 1, "fromOffset")?,
    )))
}

fn rect_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if let (Some(a), Some(b)) = (args.first().and_then(as_v2), args.get(1).and_then(as_v2)) {
        return one(wrap(Data::Rect(Rect::new(a, b))));
    }
    let n = |i| num_or(lua, &args, i, "new");
    one(wrap(Data::Rect(Rect::from_coords(n(0)?, n(1)?, n(2)?, n(3)?))))
}

fn range_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let min = check_num(lua, &args, 0, "new")?;
    let r = match args.get(1) {
        None | Some(Value::Nil) => NumberRange::single(min),
        Some(_) => NumberRange::new(min, check_num(lua, &args, 1, "new")?).map_err(|e| lua.rt_error(e))?,
    };
    one(wrap(Data::Range(r)))
}

/// A table of keypoints, of the kind `pick` takes.
fn keypoints<T>(lua: &Lua, v: &Value, fname: &str, pick: fn(Data) -> Option<T>) -> Result<Vec<T>, Throw> {
    let Value::Table(t) = v else {
        return Err(lua.rt_error(format!("{fname}: a table of keypoints expected")));
    };
    t.borrow()
        .list()
        .iter()
        .map(|k| {
            get(k)
                .and_then(pick)
                .ok_or_else(|| lua.rt_error(format!("{fname}: every item must be a keypoint")))
        })
        .collect()
}

fn numseq_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let seq = match args.first() {
        Some(t @ Value::Table(_)) => {
            let keys = keypoints(lua, t, "NumberSequence.new", |d| match d {
                Data::NumKey(k) => Some(k),
                _ => None,
            })?;
            NumberSequence::new(keys).map_err(|e| lua.rt_error(e))?
        }
        _ => {
            let a = check_num(lua, &args, 0, "new")?;
            match args.get(1) {
                None | Some(Value::Nil) => NumberSequence::constant(a),
                Some(_) => NumberSequence::between(a, check_num(lua, &args, 1, "new")?),
            }
        }
    };
    one(wrap(Data::NumSeq(seq)))
}

fn numkey_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_num(lua, &args, 0, "new")?;
    let v = check_num(lua, &args, 1, "new")?;
    let e = num_or(lua, &args, 2, "new")?;
    one(wrap(Data::NumKey(NumberSequenceKeypoint::new(t, v, e))))
}

fn colorseq_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let seq = match args.first() {
        Some(t @ Value::Table(_)) => {
            let keys = keypoints(lua, t, "ColorSequence.new", |d| match d {
                Data::ColorKey(k) => Some(k),
                _ => None,
            })?;
            ColorSequence::new(keys).map_err(|e| lua.rt_error(e))?
        }
        _ => {
            let a = arg_c3(lua, &args, 0, "new")?;
            match args.get(1).and_then(as_c3) {
                None => ColorSequence::constant(a),
                Some(b) => ColorSequence::between(a, b),
            }
        }
    };
    one(wrap(Data::ColorSeq(seq)))
}

fn colorkey_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_num(lua, &args, 0, "new")?;
    let c = arg_c3(lua, &args, 1, "new")?;
    one(wrap(Data::ColorKey(ColorSequenceKeypoint::new(t, c))))
}

fn tween_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let d = TweenInfo::default();
    let time = opt_num(lua, &args, 0, "new", d.time)?;
    let style = match args.get(1) {
        None | Some(Value::Nil) => d.easing_style,
        Some(_) => {
            let it = arg_enum(lua, &args, 1, "new", "EasingStyle")?;
            EasingStyle::from_name(it.name)
                .ok_or_else(|| lua.rt_error(format!("EasingStyle {} is not supported", it.name)))?
        }
    };
    let dir = match args.get(2) {
        None | Some(Value::Nil) => d.easing_direction,
        Some(_) => {
            let it = arg_enum(lua, &args, 2, "new", "EasingDirection")?;
            EasingDirection::from_name(it.name).unwrap_or(EasingDirection::Out)
        }
    };
    let repeat = opt_num(lua, &args, 3, "new", 0.0)? as i32;
    let reverses = args.get(4).is_some_and(Value::truthy);
    let delay = opt_num(lua, &args, 5, "new", 0.0)?;
    one(wrap(Data::Tween(TweenInfo::new(
        time, style, dir, repeat, reverses, delay,
    ))))
}

fn brick_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let b = match args.first() {
        Some(Value::Str(s)) => BrickColor::from_name(&String::from_utf8_lossy(s)),
        Some(Value::Num(_)) if args.len() >= 3 => {
            let (r, g, b) = three(lua, &args, "new")?;
            BrickColor::from_rgb(r, g, b)
        }
        Some(Value::Num(n)) => BrickColor::new(*n as i64),
        Some(v) => match as_c3(v) {
            Some(c) => BrickColor::from_color3(c),
            None => return Err(want(lua, &args, 0, "new", "number, string or Color3")),
        },
        None => BrickColor::default(),
    };
    one(wrap(Data::Brick(b)))
}

fn brick_palette(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let i = check_int(lua, &args, 0, "palette")?;
    BrickColor::palette(i)
        .map(|b| vec![wrap(Data::Brick(b))])
        .map_err(|e| lua.rt_error(e))
}

fn brick_random(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    let mut rng = Random::from_entropy();
    one(wrap(Data::Brick(BrickColor::random(&mut rng))))
}

fn brick_named(_: &mut Lua, _: &mut dyn Host, me: &Native, _: Vec<Value>) -> Res {
    let b = match &*me.name {
        "White" => BrickColor::white(),
        "Gray" => BrickColor::gray(),
        "DarkGray" => BrickColor::dark_gray(),
        "Black" => BrickColor::black(),
        "Red" => BrickColor::red(),
        "Yellow" => BrickColor::yellow(),
        "Green" => BrickColor::green(),
        _ => BrickColor::blue(),
    };
    one(wrap(Data::Brick(b)))
}

fn weight_style(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<(FontWeight, FontStyle), Throw> {
    let weight = match args.get(i) {
        None | Some(Value::Nil) => FontWeight::Regular,
        Some(_) => {
            FontWeight::from_name(arg_enum(lua, args, i, fname, "FontWeight")?.name).unwrap_or(FontWeight::Regular)
        }
    };
    let style = match args.get(i + 1) {
        None | Some(Value::Nil) => FontStyle::Normal,
        Some(_) => {
            FontStyle::from_name(arg_enum(lua, args, i + 1, fname, "FontStyle")?.name).unwrap_or(FontStyle::Normal)
        }
    };
    Ok((weight, style))
}

fn font_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let family = check_text(lua, &args, 0, "new")?;
    let (w, s) = weight_style(lua, &args, 1, "new")?;
    one(wrap(Data::Font(Font::new(&family, w, s))))
}

fn font_from_name(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let name = check_text(lua, &args, 0, "fromName")?;
    let (w, s) = weight_style(lua, &args, 1, "fromName")?;
    one(wrap(Data::Font(Font::from_name(&name, w, s))))
}

fn font_from_id(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let id = check_num(lua, &args, 0, "fromId")?;
    let (w, s) = weight_style(lua, &args, 1, "fromId")?;
    one(wrap(Data::Font(Font::from_id(id.max(0.0) as u64, w, s))))
}

fn font_from_enum(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let it = arg_enum(lua, &args, 0, "fromEnum", "Font")?;
    let e = FontEnum::from_name(it.name).unwrap_or(FontEnum::Unknown);
    Font::from_enum(e)
        .map(|f| vec![wrap(Data::Font(f))])
        .map_err(|e| lua.rt_error(e))
}

fn random_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let r = match args.first() {
        None | Some(Value::Nil) => Random::from_entropy(),
        Some(_) => Random::new(check_num(lua, &args, 0, "new")?),
    };
    one(wrap(Data::Rng(r)))
}

fn ray_new(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let o = args.first().and_then(as_v3).unwrap_or(Vector3::ZERO);
    let d = args.get(1).and_then(as_v3).unwrap_or(Vector3::ZERO);
    let _ = lua;
    one(wrap(Data::Ray(o, d)))
}

/// RaycastParams.new(): a table of its fields, which scripts set.
fn raycast_params_new(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    let mut t = Table::new();
    t.set_str("FilterType", enum_value(enum_item("RaycastFilterType", "Exclude")));
    t.set_str("FilterDescendantsInstances", Value::table(Table::new()));
    t.set_str("IgnoreWater", Value::Bool(false));
    t.set_str("CollisionGroup", Value::str("Default"));
    t.set_str("RespectCanCollide", Value::Bool(false));
    one(Value::table(t))
}

// ---------------------------------------------------------------- properties and methods

/// A method of a type: a builtin named as the method, made once.
fn method(type_name: &'static str, name: &str) -> Option<Value> {
    thread_local! {
        static METHODS: RefCell<HashMap<(&'static str, String), Value>> = RefCell::new(HashMap::new());
    }
    let f: super::hstudio_lua::Builtin = match type_name {
        "Vector3" | "Vector2" => vector_method,
        "CFrame" => cframe_method,
        "Color3" => color_method,
        "UDim2" => udim2_method,
        "Random" => random_method,
        "Ray" => ray_method,
        "EnumItem" => item_method,
        "Enum" => enum_method,
        "Enums" => enums_method,
        "Font"
        | "TweenInfo"
        | "BrickColor"
        | "UDim"
        | "Rect"
        | "NumberRange"
        | "NumberSequence"
        | "NumberSequenceKeypoint"
        | "ColorSequence"
        | "ColorSequenceKeypoint" => return None,
        _ => return None,
    };
    let known: &[&str] = match type_name {
        "Vector3" => &[
            "Lerp", "Dot", "Cross", "FuzzyEq", "Abs", "Ceil", "Floor", "Sign", "Min", "Max", "Angle",
        ],
        "Vector2" => &[
            "Lerp", "Dot", "Cross", "FuzzyEq", "Abs", "Ceil", "Floor", "Sign", "Min", "Max", "Angle",
        ],
        "CFrame" => &[
            "Inverse",
            "Lerp",
            "Orthonormalize",
            "ToWorldSpace",
            "ToObjectSpace",
            "PointToWorldSpace",
            "PointToObjectSpace",
            "VectorToWorldSpace",
            "VectorToObjectSpace",
            "GetComponents",
            "components",
            "ToEulerAnglesXYZ",
            "ToEulerAnglesYXZ",
            "ToOrientation",
            "toEulerAnglesXYZ",
            "toEulerAnglesYXZ",
            "toOrientation",
            "ToEulerAngles",
            "ToAxisAngle",
            "toAxisAngle",
            "FuzzyEq",
            "AngleBetween",
        ],
        "Color3" => &["Lerp", "ToHSV", "ToHex"],
        "UDim2" => &["Lerp"],
        "Random" => &["NextInteger", "NextNumber", "NextUnitVector", "Shuffle", "Clone"],
        "Ray" => &["ClosestPoint", "Distance"],
        "EnumItem" => &["IsA"],
        "Enum" => &["GetEnumItems", "FromName", "FromValue"],
        "Enums" => &["GetEnums"],
        _ => &[],
    };
    let name = known.iter().find(|k| **k == name)?;
    Some(METHODS.with(|m| {
        m.borrow_mut()
            .entry((type_name, (*name).to_owned()))
            .or_insert_with(|| Native::builtin(name, f))
            .clone()
    }))
}

fn missing(lua: &Lua, d: &Data, key: &Value) -> Throw {
    lua.rt_error(format!(
        "{} is not a valid member of {}",
        key.plain_text(),
        d.type_name()
    ))
}

fn index(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let d = args.first().and_then(get).ok_or_else(|| lua.rt_error("bad self"))?;
    let key = args.get(1).cloned().unwrap_or_default();
    let k = key.as_text().unwrap_or_default();
    let num = |n: f64| Ok(vec![Value::Num(n)]);
    let found: Option<Value> = match (&d, k.as_str()) {
        (Data::V3(v), "X" | "x") => return num(v.x),
        (Data::V3(v), "Y" | "y") => return num(v.y),
        (Data::V3(v), "Z" | "z") => return num(v.z),
        (Data::V3(v), "Magnitude" | "magnitude") => return num(v.magnitude()),
        (Data::V3(v), "Unit" | "unit") => Some(v3(v.unit())),
        (Data::V2(v), "X" | "x") => return num(v.x),
        (Data::V2(v), "Y" | "y") => return num(v.y),
        (Data::V2(v), "Magnitude" | "magnitude") => return num(v.magnitude()),
        (Data::V2(v), "Unit" | "unit") => Some(v2(v.unit())),
        (Data::CF(c), "Position" | "p") => Some(v3(c.position())),
        (Data::CF(c), "Rotation") => Some(cf(c.rotation())),
        (Data::CF(c), "X" | "x") => return num(c.x()),
        (Data::CF(c), "Y" | "y") => return num(c.y()),
        (Data::CF(c), "Z" | "z") => return num(c.z()),
        (Data::CF(c), "LookVector" | "lookVector") => Some(v3(c.look_vector())),
        (Data::CF(c), "RightVector" | "rightVector") => Some(v3(c.right_vector())),
        (Data::CF(c), "UpVector" | "upVector") => Some(v3(c.up_vector())),
        (Data::CF(c), "XVector") => Some(v3(c.x_vector())),
        (Data::CF(c), "YVector") => Some(v3(c.y_vector())),
        (Data::CF(c), "ZVector") => Some(v3(c.z_vector())),
        (Data::C3(c), "R" | "r") => return num(c.r),
        (Data::C3(c), "G" | "g") => return num(c.g),
        (Data::C3(c), "B" | "b") => return num(c.b),
        (Data::UD(u), "Scale") => return num(u.scale),
        (Data::UD(u), "Offset") => return num(f64::from(u.offset)),
        (Data::UD2(u), "X" | "Width") => Some(udim(u.x)),
        (Data::UD2(u), "Y" | "Height") => Some(udim(u.y)),
        (Data::Rect(r), "Min") => Some(v2(r.min)),
        (Data::Rect(r), "Max") => Some(v2(r.max)),
        (Data::Rect(r), "Width") => return num(r.width()),
        (Data::Rect(r), "Height") => return num(r.height()),
        (Data::Range(r), "Min") => return num(r.min),
        (Data::Range(r), "Max") => return num(r.max),
        (Data::NumSeq(s), "Keypoints") => Some(Value::table(Table::from_list(
            s.keypoints().iter().map(|k| wrap(Data::NumKey(*k))).collect(),
        ))),
        (Data::ColorSeq(s), "Keypoints") => Some(Value::table(Table::from_list(
            s.keypoints().iter().map(|k| wrap(Data::ColorKey(*k))).collect(),
        ))),
        (Data::NumKey(k), "Time") => return num(k.time),
        (Data::NumKey(k), "Value") => return num(k.value),
        (Data::NumKey(k), "Envelope") => return num(k.envelope),
        (Data::ColorKey(k), "Time") => return num(k.time),
        (Data::ColorKey(k), "Value") => Some(c3(k.value)),
        (Data::Tween(t), "Time") => return num(t.time),
        (Data::Tween(t), "DelayTime") => return num(t.delay_time),
        (Data::Tween(t), "RepeatCount") => return num(f64::from(t.repeat_count)),
        (Data::Tween(t), "Reverses") => Some(Value::Bool(t.reverses)),
        (Data::Tween(t), "EasingStyle") => Some(enum_value(enum_item("EasingStyle", t.easing_style.name()))),
        (Data::Tween(t), "EasingDirection") => {
            Some(enum_value(enum_item("EasingDirection", t.easing_direction.name())))
        }
        (Data::Brick(b), "Name") => Some(Value::str(b.name())),
        (Data::Brick(b), "Number") => return num(f64::from(b.number())),
        (Data::Brick(b), "Color") => Some(c3(b.color())),
        (Data::Brick(b), "r") => return num(b.r()),
        (Data::Brick(b), "g") => return num(b.g()),
        (Data::Brick(b), "b") => return num(b.b()),
        (Data::Font(f), "Family") => Some(Value::str(&f.family)),
        (Data::Font(f), "Weight") => Some(enum_value(enum_item("FontWeight", f.weight.name()))),
        (Data::Font(f), "Style") => Some(enum_value(enum_item("FontStyle", f.style.name()))),
        (Data::Font(f), "Bold") => Some(Value::Bool(f.bold())),
        (Data::Ray(o, _), "Origin") => Some(v3(*o)),
        (Data::Ray(_, d), "Direction") => Some(v3(*d)),
        (Data::Ray(o, d), "Unit") => Some(wrap(Data::Ray(*o, d.unit()))),
        (Data::Item(i), "Name") => Some(Value::str(i.name)),
        (Data::Item(i), "Value") => return num(f64::from(i.value)),
        (Data::Item(i), "EnumType") => Some(wrap(Data::EnumType(i.enum_name))),
        (Data::EnumType(e), name) => {
            enum_def(e).and_then(|def| def.items.iter().find(|i| i.0 == name).map(|i| enum_value(item(def, i))))
        }
        (Data::Enums, name) => enum_def(name).map(|def| wrap(Data::EnumType(def.name))),
        _ => None,
    };
    if let Some(v) = found {
        return Ok(vec![v]);
    }
    if let Some(m) = method(d.type_name(), &k) {
        return Ok(vec![m]);
    }
    if let Data::EnumType(e) = d {
        return Err(lua.rt_error(format!("{k} is not a valid member of \"Enum.{e}\"")));
    }
    if let Data::Enums = d {
        return Err(lua.rt_error(format!("{k} is not a valid EnumItem")));
    }
    Err(missing(lua, &d, &key))
}

fn newindex(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let d = args.first().and_then(get);
    let what = d.as_ref().map_or("value", Data::type_name);
    let key = args.get(1).map(Value::plain_text).unwrap_or_default();
    Err(lua.rt_error(format!("{key} cannot be assigned to ({what} values can't be changed)")))
}

/// How `tostring` writes one.
pub fn text(d: &Data) -> String {
    match d {
        Data::V2(v) => v.to_string(),
        Data::V3(v) => v.to_string(),
        Data::CF(c) => c.to_string(),
        Data::C3(c) => c.to_string(),
        Data::UD(u) => u.to_string(),
        Data::UD2(u) => u.to_string(),
        Data::Rect(r) => r.to_string(),
        Data::Range(r) => r.to_string(),
        Data::NumSeq(s) => s.to_string(),
        Data::NumKey(k) => format!("{} {} {}", k.time, k.value, k.envelope),
        Data::ColorSeq(s) => s.to_string(),
        Data::ColorKey(k) => format!("{} {}", k.time, k.value),
        Data::Tween(t) => t.to_string(),
        Data::Brick(b) => b.to_string(),
        Data::Font(f) => f.to_string(),
        Data::Rng(_) => "Random".into(),
        Data::Ray(o, d) => format!("{{{o}}}, {{{d}}}"),
        Data::Item(i) => format!("Enum.{}.{}", i.enum_name, i.name),
        Data::EnumType(e) => (*e).to_owned(),
        Data::Enums => "Enums".into(),
    }
}

fn tostring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let d = args.first().and_then(get).ok_or_else(|| lua.rt_error("bad self"))?;
    one(Value::str(&text(&d)))
}

/// Whether two values are equal as Roblox compares them.
pub fn same(a: &Data, b: &Data) -> bool {
    match (a, b) {
        (Data::V2(x), Data::V2(y)) => x == y,
        (Data::V3(x), Data::V3(y)) => x == y,
        (Data::CF(x), Data::CF(y)) => x == y,
        (Data::C3(x), Data::C3(y)) => x == y,
        (Data::UD(x), Data::UD(y)) => x == y,
        (Data::UD2(x), Data::UD2(y)) => x == y,
        (Data::Rect(x), Data::Rect(y)) => x == y,
        (Data::Range(x), Data::Range(y)) => x == y,
        (Data::NumSeq(x), Data::NumSeq(y)) => x == y,
        (Data::ColorSeq(x), Data::ColorSeq(y)) => x == y,
        (Data::Tween(x), Data::Tween(y)) => x == y,
        (Data::Brick(x), Data::Brick(y)) => x == y,
        (Data::Font(x), Data::Font(y)) => x == y,
        (Data::Item(x), Data::Item(y)) => x == y,
        (Data::EnumType(x), Data::EnumType(y)) => x == y,
        (Data::Ray(a, b), Data::Ray(c, d)) => a == c && b == d,
        (Data::Enums, Data::Enums) => true,
        _ => false,
    }
}

fn eq(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let r = match (args.first().and_then(get), args.get(1).and_then(get)) {
        (Some(a), Some(b)) => same(&a, &b),
        _ => false,
    };
    one(Value::Bool(r))
}

/// The two operands of an operator, as data or numbers.
enum Operand {
    D(Data),
    N(f64),
}

fn operand(v: &Value) -> Option<Operand> {
    match v {
        Value::Num(n) => Some(Operand::N(*n)),
        other => get(other).map(Operand::D),
    }
}

fn op_error(lua: &Lua, op: &str, args: &[Value]) -> Throw {
    let a = args.first().map_or("nil".to_owned(), |v| lua.type_of(v));
    let b = args.get(1).map_or("nil".to_owned(), |v| lua.type_of(v));
    lua.rt_error(format!("attempt to perform arithmetic ({op}) on {a} and {b}"))
}

fn binary(lua: &Lua, args: &[Value], op: &str) -> Result<Value, Throw> {
    use Operand::{D, N};
    let (Some(a), Some(b)) = (args.first().and_then(operand), args.get(1).and_then(operand)) else {
        return Err(op_error(lua, op, args));
    };
    let r = match (op, a, b) {
        ("add", D(Data::V3(x)), D(Data::V3(y))) => v3(x + y),
        ("add", D(Data::V2(x)), D(Data::V2(y))) => v2(x + y),
        ("add", D(Data::CF(c)), D(Data::V3(v))) => cf(c + v),
        ("add", D(Data::UD(x)), D(Data::UD(y))) => udim(x + y),
        ("add", D(Data::UD2(x)), D(Data::UD2(y))) => udim2(x + y),
        ("sub", D(Data::V3(x)), D(Data::V3(y))) => v3(x - y),
        ("sub", D(Data::V2(x)), D(Data::V2(y))) => v2(x - y),
        ("sub", D(Data::CF(c)), D(Data::V3(v))) => cf(c - v),
        ("sub", D(Data::UD(x)), D(Data::UD(y))) => udim(x - y),
        ("sub", D(Data::UD2(x)), D(Data::UD2(y))) => udim2(x - y),
        ("mul", D(Data::V3(x)), D(Data::V3(y))) => v3(x * y),
        ("mul", D(Data::V3(x)), N(n)) | ("mul", N(n), D(Data::V3(x))) => v3(x * n),
        ("mul", D(Data::V2(x)), D(Data::V2(y))) => v2(x * y),
        ("mul", D(Data::V2(x)), N(n)) | ("mul", N(n), D(Data::V2(x))) => v2(x * n),
        ("mul", D(Data::CF(a)), D(Data::CF(b))) => cf(a * b),
        ("mul", D(Data::CF(c)), D(Data::V3(v))) => v3(c * v),
        ("div", D(Data::V3(x)), D(Data::V3(y))) => v3(x / y),
        ("div", D(Data::V3(x)), N(n)) => v3(x / n),
        ("div", N(n), D(Data::V3(x))) => v3(Vector3::new(n, n, n) / x),
        ("div", D(Data::V2(x)), D(Data::V2(y))) => v2(x / y),
        ("div", D(Data::V2(x)), N(n)) => v2(x / n),
        ("div", N(n), D(Data::V2(x))) => v2(Vector2::new(n, n) / x),
        ("idiv", D(Data::V3(x)), D(Data::V3(y))) => v3(x.floor_div(y)),
        ("idiv", D(Data::V3(x)), N(n)) => v3(x.floor_div(Vector3::new(n, n, n))),
        ("idiv", D(Data::V2(x)), D(Data::V2(y))) => v2(x.floor_div(y)),
        ("idiv", D(Data::V2(x)), N(n)) => v2(x.floor_div(Vector2::new(n, n))),
        _ => return Err(op_error(lua, op, args)),
    };
    Ok(r)
}

fn add(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(binary(lua, &args, "add")?)
}
fn sub(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(binary(lua, &args, "sub")?)
}
fn mul(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(binary(lua, &args, "mul")?)
}
fn div(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(binary(lua, &args, "div")?)
}
fn idiv(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(binary(lua, &args, "idiv")?)
}

fn unm(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    match args.first().and_then(get) {
        Some(Data::V3(v)) => one(v3(-v)),
        Some(Data::V2(v)) => one(v2(-v)),
        _ => Err(op_error(lua, "unm", &args)),
    }
}

/// Enum items order by their values; nothing else orders.
fn lt(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    match (
        args.first().and_then(as_item_value),
        args.get(1).and_then(as_item_value),
    ) {
        (Some(a), Some(b)) => one(Value::Bool(a < b)),
        _ => Err(lua.rt_error("attempt to compare two values that can't be ordered")),
    }
}
fn le(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    match (
        args.first().and_then(as_item_value),
        args.get(1).and_then(as_item_value),
    ) {
        (Some(a), Some(b)) => one(Value::Bool(a <= b)),
        _ => Err(lua.rt_error("attempt to compare two values that can't be ordered")),
    }
}
fn as_item_value(v: &Value) -> Option<i32> {
    as_item(v).map(|i| i.value)
}

fn vector_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let name = &*me.name;
    match args.first().and_then(get) {
        Some(Data::V3(a)) => {
            let other = |i| arg_v3(lua, &args, i, name);
            one(match name {
                "Lerp" => v3(a.lerp(other(1)?, check_num(lua, &args, 2, name)?)),
                "Dot" => Value::Num(a.dot(other(1)?)),
                "Cross" => v3(a.cross(other(1)?)),
                "FuzzyEq" => Value::Bool(a.fuzzy_eq(other(1)?, opt_num(lua, &args, 2, name, 1e-5)?)),
                "Abs" => v3(a.abs()),
                "Ceil" => v3(a.ceil()),
                "Floor" => v3(a.floor()),
                "Sign" => v3(a.sign()),
                "Min" => v3(a.min(other(1)?)),
                "Max" => v3(a.max(other(1)?)),
                _ => Value::Num(a.angle(other(1)?, args.get(2).and_then(as_v3))),
            })
        }
        Some(Data::V2(a)) => {
            let other = |i| arg_v2(lua, &args, i, name);
            one(match name {
                "Lerp" => v2(a.lerp(other(1)?, check_num(lua, &args, 2, name)?)),
                "Dot" => Value::Num(a.dot(other(1)?)),
                "Cross" => Value::Num(a.cross(other(1)?)),
                "FuzzyEq" => Value::Bool(a.fuzzy_eq(other(1)?, opt_num(lua, &args, 2, name, 1e-5)?)),
                "Abs" => v2(a.abs()),
                "Ceil" => v2(a.ceil()),
                "Floor" => v2(a.floor()),
                "Sign" => v2(a.sign()),
                "Min" => v2(a.min(other(1)?)),
                "Max" => v2(a.max(other(1)?)),
                _ => Value::Num(a.angle(other(1)?, args.get(2).is_some_and(Value::truthy))),
            })
        }
        _ => Err(want(lua, &args, 0, name, "a vector (call it with ':')")),
    }
}

fn nums(v: &[f64]) -> Vec<Value> {
    v.iter().map(|n| Value::Num(*n)).collect()
}

fn cframe_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let name = &*me.name;
    let c = arg_cf(lua, &args, 0, name)?;
    // The *Space methods take any number of values and return as many.
    let each_cf = |lua: &Lua, f: &dyn Fn(CFrame) -> CFrame| -> Res {
        (1..args.len().max(2))
            .map(|i| arg_cf(lua, &args, i, name).map(|x| cf(f(x))))
            .collect()
    };
    let each_v3 = |lua: &Lua, f: &dyn Fn(Vector3) -> Vector3| -> Res {
        (1..args.len().max(2))
            .map(|i| arg_v3(lua, &args, i, name).map(|x| v3(f(x))))
            .collect()
    };
    match name {
        "Inverse" => one(cf(c.inverse())),
        "Lerp" => one(cf(c.lerp(arg_cf(lua, &args, 1, name)?, check_num(lua, &args, 2, name)?))),
        "Orthonormalize" => one(cf(c.orthonormalize())),
        "ToWorldSpace" => each_cf(lua, &|x| c.to_world(x)),
        "ToObjectSpace" => each_cf(lua, &|x| c.to_object(x)),
        "PointToWorldSpace" => each_v3(lua, &|x| c.point_to_world(x)),
        "PointToObjectSpace" => each_v3(lua, &|x| c.point_to_object(x)),
        "VectorToWorldSpace" => each_v3(lua, &|x| c.vector_to_world(x)),
        "VectorToObjectSpace" => each_v3(lua, &|x| c.vector_to_object(x)),
        "GetComponents" | "components" => Ok(nums(&c.get_components())),
        "ToEulerAnglesXYZ" | "toEulerAnglesXYZ" => {
            let (x, y, z) = c.to_euler_angles_xyz();
            Ok(nums(&[x, y, z]))
        }
        "ToEulerAnglesYXZ" | "toEulerAnglesYXZ" => {
            let (x, y, z) = c.to_euler_angles_yxz();
            Ok(nums(&[x, y, z]))
        }
        "ToOrientation" | "toOrientation" => {
            let (x, y, z) = c.to_orientation();
            Ok(nums(&[x, y, z]))
        }
        "ToEulerAngles" => {
            let order = match args.get(1) {
                None | Some(Value::Nil) => RotationOrder::XYZ,
                Some(_) => RotationOrder::from_name(arg_enum(lua, &args, 1, name, "RotationOrder")?.name)
                    .unwrap_or(RotationOrder::XYZ),
            };
            let (x, y, z) = c.to_euler_angles(order);
            Ok(nums(&[x, y, z]))
        }
        "ToAxisAngle" | "toAxisAngle" => {
            let (axis, angle) = c.to_axis_angle();
            Ok(vec![v3(axis), Value::Num(angle)])
        }
        "FuzzyEq" => one(Value::Bool(
            c.fuzzy_eq(arg_cf(lua, &args, 1, name)?, opt_num(lua, &args, 2, name, 1e-5)?),
        )),
        _ => one(Value::Num(c.angle_between(arg_cf(lua, &args, 1, name)?))),
    }
}

fn color_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let name = &*me.name;
    let c = arg_c3(lua, &args, 0, name)?;
    match name {
        "Lerp" => one(c3(c.lerp(arg_c3(lua, &args, 1, name)?, check_num(lua, &args, 2, name)?))),
        "ToHSV" => {
            let (h, s, v) = c.to_hsv();
            Ok(nums(&[h, s, v]))
        }
        _ => one(Value::str(&c.to_hex())),
    }
}

fn udim2_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let a = args
        .first()
        .and_then(as_udim2)
        .ok_or_else(|| want(lua, &args, 0, &me.name, "UDim2"))?;
    let b = args
        .get(1)
        .and_then(as_udim2)
        .ok_or_else(|| want(lua, &args, 1, &me.name, "UDim2"))?;
    one(udim2(a.lerp(b, check_num(lua, &args, 2, &me.name)?)))
}

fn random_method(lua: &mut Lua, h: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let name = &*me.name;
    let Some(Value::Userdata(u)) = args.first() else {
        return Err(want(lua, &args, 0, name, "Random"));
    };
    let lo = opt_num(lua, &args, 1, name, 0.0)?;
    let hi = opt_num(lua, &args, 2, name, 1.0)?;
    let shuffle = if name == "Shuffle" {
        let Some(Value::Table(t)) = args.get(1) else {
            return Err(want(lua, &args, 1, name, "table"));
        };
        Some(t.clone())
    } else {
        None
    };
    let mut data = u.data.borrow_mut();
    let Some(Data::Rng(rng)) = data.downcast_mut::<Data>() else {
        return Err(want(lua, &args, 0, name, "Random"));
    };
    match name {
        "NextInteger" => {
            if args.len() < 3 {
                return Err(bad_arg(lua, 2, name, "number expected"));
            }
            if lo > hi {
                return Err(bad_arg(lua, 2, name, "max must be greater than or equal to min"));
            }
            one(Value::Num(rng.next_integer(lo, hi) as f64))
        }
        "NextNumber" => one(Value::Num(if args.len() >= 3 {
            rng.next_number_between(lo, hi)
        } else {
            rng.next_number()
        })),
        "NextUnitVector" => one(v3(rng.next_unit_vector())),
        "Clone" => {
            let copy = rng.clone();
            drop(data);
            one(wrap(Data::Rng(copy)))
        }
        _ => {
            let t = shuffle.expect("checked above");
            let mut items = t.borrow().list();
            rng.shuffle(&mut items);
            drop(data);
            let _ = h;
            t.borrow_mut().set_list(items);
            Ok(Vec::new())
        }
    }
}

fn ray_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let Some(Data::Ray(o, d)) = args.first().and_then(get) else {
        return Err(want(lua, &args, 0, &me.name, "Ray"));
    };
    let p = arg_v3(lua, &args, 1, &me.name)?;
    let unit = d.unit();
    let t = (p - o).dot(unit).max(0.0);
    let closest = o + unit * t;
    if &*me.name == "ClosestPoint" {
        one(v3(closest))
    } else {
        one(Value::Num((p - closest).magnitude()))
    }
}

fn item_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let it = args
        .first()
        .and_then(as_item)
        .ok_or_else(|| want(lua, &args, 0, &me.name, "EnumItem"))?;
    let name = check_text(lua, &args, 1, &me.name)?;
    one(Value::Bool(it.enum_name == name))
}

fn enum_method(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let Some(Data::EnumType(e)) = args.first().and_then(get) else {
        return Err(want(lua, &args, 0, &me.name, "Enum"));
    };
    let def = enum_def(e).ok_or_else(|| lua.rt_error("unknown Enum"))?;
    match &*me.name {
        "GetEnumItems" => one(Value::table(Table::from_list(
            def.items.iter().map(|i| enum_value(item(def, i))).collect(),
        ))),
        "FromName" => one(args
            .get(1)
            .and_then(|v| match v {
                Value::Str(_) => coerce_enum(e, v),
                _ => None,
            })
            .map_or(Value::Nil, enum_value)),
        _ => one(args
            .get(1)
            .and_then(|v| match v {
                Value::Num(_) => coerce_enum(e, v),
                _ => None,
            })
            .map_or(Value::Nil, enum_value)),
    }
}

fn enums_method(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::table(Table::from_list(
        enum_defs().iter().map(|d| wrap(Data::EnumType(d.name))).collect(),
    )))
}

// ---------------------------------------------------------------- the enums

/// An Enum: its name and its items with their values.
pub struct EnumDef {
    pub name: &'static str,
    pub items: Vec<(&'static str, i32)>,
}

/// Enums the studio's classes and services use, beyond those hstudio_rbx_types has.
const ENUMS: &[(&str, &[(&str, i32)])] = &[
    (
        "UserInputType",
        &[
            ("MouseButton1", 0),
            ("MouseButton2", 1),
            ("MouseButton3", 2),
            ("MouseWheel", 3),
            ("MouseMovement", 4),
            ("Touch", 7),
            ("Keyboard", 8),
            ("Focus", 9),
            ("Accelerometer", 10),
            ("Gyro", 11),
            ("Gamepad1", 12),
            ("TextInput", 20),
            ("InputMethod", 21),
            ("None", 22),
        ],
    ),
    (
        "UserInputState",
        &[("Begin", 0), ("Change", 1), ("End", 2), ("Cancel", 3), ("None", 4)],
    ),
    ("FillDirection", &[("Horizontal", 0), ("Vertical", 1)]),
    ("HorizontalAlignment", &[("Center", 0), ("Left", 1), ("Right", 2)]),
    ("VerticalAlignment", &[("Center", 0), ("Top", 1), ("Bottom", 2)]),
    ("SortOrder", &[("Name", 0), ("Custom", 1), ("LayoutOrder", 2)]),
    ("TextXAlignment", &[("Left", 0), ("Right", 1), ("Center", 2)]),
    ("TextYAlignment", &[("Top", 0), ("Center", 1), ("Bottom", 2)]),
    ("AutomaticSize", &[("None", 0), ("X", 1), ("Y", 2), ("XY", 3)]),
    (
        "SizeConstraint",
        &[("RelativeXY", 0), ("RelativeXX", 1), ("RelativeYY", 2)],
    ),
    ("ZIndexBehavior", &[("Global", 0), ("Sibling", 1)]),
    ("AspectType", &[("FitWithinMaxSize", 0), ("ScaleWithParentSize", 1)]),
    ("DominantAxis", &[("Width", 0), ("Height", 1)]),
    (
        "StartCorner",
        &[("TopLeft", 0), ("TopRight", 1), ("BottomLeft", 2), ("BottomRight", 3)],
    ),
    ("ScrollingDirection", &[("X", 1), ("Y", 2), ("XY", 4)]),
    ("ScrollBarInset", &[("None", 0), ("ScrollBar", 1), ("Always", 2)]),
    ("VerticalScrollBarPosition", &[("Right", 0), ("Left", 1)]),
    ("ElasticBehavior", &[("WhenScrollable", 0), ("Always", 1), ("Never", 2)]),
    (
        "GuiState",
        &[("Idle", 0), ("Hover", 1), ("Press", 2), ("NonInteractable", 3)],
    ),
    ("ApplyStrokeMode", &[("Contextual", 0), ("Border", 1)]),
    ("LineJoinMode", &[("Round", 0), ("Bevel", 1), ("Miter", 2)]),
    ("TextTruncate", &[("None", 0), ("AtEnd", 1), ("SplitWord", 2)]),
    ("BorderMode", &[("Outline", 0), ("Middle", 1), ("Inset", 2)]),
    (
        "ScaleType",
        &[("Stretch", 0), ("Slice", 1), ("Tile", 2), ("Fit", 3), ("Crop", 4)],
    ),
    ("ResamplerMode", &[("Default", 0), ("Pixelated", 1)]),
    (
        "UIFlexMode",
        &[("None", 0), ("Grow", 1), ("Shrink", 2), ("Fill", 3), ("Custom", 4)],
    ),
    (
        "UIFlexAlignment",
        &[
            ("None", 0),
            ("Fill", 1),
            ("SpaceAround", 2),
            ("SpaceBetween", 3),
            ("SpaceEvenly", 4),
        ],
    ),
    (
        "ItemLineAlignment",
        &[
            ("Automatic", 0),
            ("Start", 1),
            ("Center", 2),
            ("End", 3),
            ("Stretch", 4),
        ],
    ),
    ("TableMajorAxis", &[("RowMajor", 0), ("ColumnMajor", 1)]),
    ("SafeAreaCompatibility", &[("None", 0), ("FullscreenExtension", 1)]),
    (
        "ScreenInsets",
        &[
            ("None", 0),
            ("DeviceSafeInsets", 1),
            ("CoreUISafeInsets", 2),
            ("TopbarSafeInsets", 3),
        ],
    ),
    (
        "MouseBehavior",
        &[("Default", 0), ("LockCenter", 1), ("LockCurrentPosition", 2)],
    ),
    ("TweenStatus", &[("Canceled", 0), ("Completed", 1)]),
    (
        "PlaybackState",
        &[
            ("Begin", 0),
            ("Delayed", 1),
            ("Playing", 2),
            ("Paused", 3),
            ("Completed", 4),
            ("Cancelled", 5),
        ],
    ),
    (
        "PartType",
        &[
            ("Ball", 0),
            ("Block", 1),
            ("Cylinder", 2),
            ("Wedge", 3),
            ("CornerWedge", 4),
        ],
    ),
    (
        "Material",
        &[
            ("Plastic", 256),
            ("SmoothPlastic", 272),
            ("Neon", 288),
            ("Wood", 512),
            ("WoodPlanks", 528),
            ("Marble", 784),
            ("Basalt", 788),
            ("Slate", 800),
            ("CrackedLava", 804),
            ("Concrete", 816),
            ("Limestone", 820),
            ("Granite", 832),
            ("Pavement", 836),
            ("Brick", 848),
            ("Pebble", 864),
            ("Cobblestone", 880),
            ("Rock", 896),
            ("Sandstone", 912),
            ("CorrodedMetal", 1040),
            ("DiamondPlate", 1056),
            ("Foil", 1072),
            ("Metal", 1088),
            ("Grass", 1280),
            ("LeafyGrass", 1284),
            ("Sand", 1296),
            ("Fabric", 1312),
            ("Snow", 1328),
            ("Mud", 1344),
            ("Ground", 1376),
            ("Asphalt", 1424),
            ("Salt", 1392),
            ("Ice", 1536),
            ("Glacier", 1552),
            ("Glass", 1568),
            ("ForceField", 1584),
            ("Air", 1792),
            ("Water", 2048),
        ],
    ),
    (
        "CameraType",
        &[
            ("Fixed", 0),
            ("Attach", 1),
            ("Watch", 2),
            ("Track", 3),
            ("Follow", 4),
            ("Custom", 5),
            ("Scriptable", 6),
            ("Orbital", 7),
        ],
    ),
    (
        "HumanoidStateType",
        &[
            ("FallingDown", 0),
            ("Ragdoll", 1),
            ("GettingUp", 2),
            ("Jumping", 3),
            ("Swimming", 4),
            ("Freefall", 5),
            ("Flying", 6),
            ("Landed", 7),
            ("Running", 8),
            ("RunningNoPhysics", 10),
            ("StrafingNoPhysics", 11),
            ("Climbing", 12),
            ("Seated", 13),
            ("PlatformStanding", 14),
            ("Dead", 15),
            ("Physics", 16),
            ("None", 18),
        ],
    ),
    ("HumanoidRigType", &[("R6", 0), ("R15", 1)]),
    (
        "CoreGuiType",
        &[
            ("PlayerList", 0),
            ("Health", 1),
            ("Backpack", 2),
            ("Chat", 3),
            ("All", 4),
            ("EmotesMenu", 5),
            ("SelfView", 6),
            ("Captures", 7),
        ],
    ),
    (
        "RaycastFilterType",
        &[("Exclude", 0), ("Include", 1), ("Blacklist", 0), ("Whitelist", 1)],
    ),
    ("ContextActionResult", &[("Sink", 0), ("Pass", 1)]),
    (
        "RenderPriority",
        &[
            ("First", 0),
            ("Input", 100),
            ("Camera", 200),
            ("Character", 300),
            ("Last", 2000),
        ],
    ),
    (
        "MessageType",
        &[
            ("MessageOutput", 0),
            ("MessageInfo", 1),
            ("MessageWarning", 2),
            ("MessageError", 3),
        ],
    ),
    (
        "KeyCode",
        &[
            ("Unknown", 0),
            ("Backspace", 8),
            ("Tab", 9),
            ("Clear", 12),
            ("Return", 13),
            ("Pause", 19),
            ("Escape", 27),
            ("Space", 32),
            ("QuotedDouble", 34),
            ("Hash", 35),
            ("Dollar", 36),
            ("Percent", 37),
            ("Ampersand", 38),
            ("Quote", 39),
            ("LeftParenthesis", 40),
            ("RightParenthesis", 41),
            ("Asterisk", 42),
            ("Plus", 43),
            ("Comma", 44),
            ("Minus", 45),
            ("Period", 46),
            ("Slash", 47),
            ("Zero", 48),
            ("One", 49),
            ("Two", 50),
            ("Three", 51),
            ("Four", 52),
            ("Five", 53),
            ("Six", 54),
            ("Seven", 55),
            ("Eight", 56),
            ("Nine", 57),
            ("Colon", 58),
            ("Semicolon", 59),
            ("LessThan", 60),
            ("Equals", 61),
            ("GreaterThan", 62),
            ("Question", 63),
            ("At", 64),
            ("LeftBracket", 91),
            ("BackSlash", 92),
            ("RightBracket", 93),
            ("Caret", 94),
            ("Underscore", 95),
            ("Backquote", 96),
            ("A", 97),
            ("B", 98),
            ("C", 99),
            ("D", 100),
            ("E", 101),
            ("F", 102),
            ("G", 103),
            ("H", 104),
            ("I", 105),
            ("J", 106),
            ("K", 107),
            ("L", 108),
            ("M", 109),
            ("N", 110),
            ("O", 111),
            ("P", 112),
            ("Q", 113),
            ("R", 114),
            ("S", 115),
            ("T", 116),
            ("U", 117),
            ("V", 118),
            ("W", 119),
            ("X", 120),
            ("Y", 121),
            ("Z", 122),
            ("LeftCurly", 123),
            ("Pipe", 124),
            ("RightCurly", 125),
            ("Tilde", 126),
            ("Delete", 127),
            ("KeypadZero", 256),
            ("KeypadOne", 257),
            ("KeypadTwo", 258),
            ("KeypadThree", 259),
            ("KeypadFour", 260),
            ("KeypadFive", 261),
            ("KeypadSix", 262),
            ("KeypadSeven", 263),
            ("KeypadEight", 264),
            ("KeypadNine", 265),
            ("KeypadPeriod", 266),
            ("KeypadDivide", 267),
            ("KeypadMultiply", 268),
            ("KeypadMinus", 269),
            ("KeypadPlus", 270),
            ("KeypadEnter", 271),
            ("KeypadEquals", 272),
            ("Up", 273),
            ("Down", 274),
            ("Right", 275),
            ("Left", 276),
            ("Insert", 277),
            ("Home", 278),
            ("End", 279),
            ("PageUp", 280),
            ("PageDown", 281),
            ("F1", 282),
            ("F2", 283),
            ("F3", 284),
            ("F4", 285),
            ("F5", 286),
            ("F6", 287),
            ("F7", 288),
            ("F8", 289),
            ("F9", 290),
            ("F10", 291),
            ("F11", 292),
            ("F12", 293),
            ("F13", 294),
            ("F14", 295),
            ("F15", 296),
            ("NumLock", 300),
            ("CapsLock", 301),
            ("ScrollLock", 302),
            ("RightShift", 303),
            ("LeftShift", 304),
            ("RightControl", 305),
            ("LeftControl", 306),
            ("RightAlt", 307),
            ("LeftAlt", 308),
            ("RightMeta", 309),
            ("LeftMeta", 310),
            ("LeftSuper", 311),
            ("RightSuper", 312),
            ("Print", 316),
            ("Menu", 319),
        ],
    ),
];

/// Every Enum, made once.
pub fn enum_defs() -> &'static [EnumDef] {
    thread_local! {
        static DEFS: &'static [EnumDef] = Box::leak(build_enums().into_boxed_slice());
    }
    DEFS.with(|d| *d)
}

fn build_enums() -> Vec<EnumDef> {
    fn from<T: Copy>(name: &'static str, all: &'static [T], f: fn(T) -> (&'static str, i32)) -> EnumDef {
        EnumDef {
            name,
            items: all.iter().map(|v| f(*v)).collect(),
        }
    }
    let mut defs = vec![
        from("EasingStyle", EasingStyle::ALL, |v| (v.name(), v.value())),
        from("EasingDirection", EasingDirection::ALL, |v| (v.name(), v.value())),
        from("RotationOrder", RotationOrder::ALL, |v| (v.name(), v.value())),
        from("NormalId", NormalId::ALL, |v| (v.name(), v.value())),
        from("Axis", Axis::ALL, |v| (v.name(), v.value())),
        from("FontWeight", FontWeight::ALL, |v| (v.name(), v.value())),
        from("FontStyle", FontStyle::ALL, |v| (v.name(), v.value())),
        from("Font", FontEnum::ALL, |v| (v.name(), v.value())),
    ];
    for (name, items) in ENUMS {
        defs.push(EnumDef {
            name,
            items: items.to_vec(),
        });
    }
    defs.sort_by_key(|d| d.name);
    defs
}

pub fn enum_def(name: &str) -> Option<&'static EnumDef> {
    let defs = enum_defs();
    defs.binary_search_by_key(&name, |d| d.name).ok().map(|i| &defs[i])
}
