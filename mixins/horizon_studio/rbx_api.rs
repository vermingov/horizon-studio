// Roblox's whole API, from its API dump (rbx_api_data.rs, which
// tools/gen_api.py makes): every class, property, method, event, callback
// and enum Roblox has.
//
// rbx.rs implements the classes and members scripts use by hand, with
// their behaviour. Everything else a script reaches is here, so that it
// exists as in Roblox: a property has its type and default and keeps what
// a script sets it to, a method returns what its type's empty value is,
// an event can be connected to, and a class Roblox can create can be
// created.

use super::hstudio_lua::{Table, Value};
use super::hstudio_rbx_api_data::API;
use super::hstudio_rbx_types::{
    BrickColor, CFrame, Color3, ColorSequence, Font, NumberRange, NumberSequence, Rect, UDim, UDim2, Vector2, Vector3,
};
use super::hstudio_rbx_values::{Data, c3, cf, enum_def, enum_value, udim, udim2, v2, v3, wrap};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The first id of the API's methods' host functions: the method's index
/// in `Api::funcs` after it.
pub const FIRST_ID: u32 = 100_000;
pub const LAST_ID: u32 = 199_999;

pub enum Member {
    Prop {
        cat: &'static str,
        ty: &'static str,
        default: &'static str,
        read_only: bool,
        /// Not scriptable: only gethiddenproperty reads it.
        hidden: bool,
    },
    /// A method: its index in `Api::funcs`.
    Func(u32),
    Event,
    /// A property a function is set to (OnInvoke, OnClientInvoke).
    Callback,
}

pub struct ApiClass {
    pub name: &'static str,
    pub base: Option<&'static str>,
    pub creatable: bool,
    pub service: bool,
    pub members: Vec<(&'static str, Member)>,
}

pub struct Func {
    pub class: &'static str,
    pub name: &'static str,
    pub cat: &'static str,
    pub ret: &'static str,
}

pub struct Api {
    pub classes: HashMap<&'static str, ApiClass>,
    pub funcs: Vec<Func>,
    pub enums: Vec<(&'static str, Vec<(&'static str, i32)>)>,
}

/// Switches the API dump gives no default (it has defaults only for the
/// classes that declare them as instances), which are on in Roblox: an
/// adornment is Visible, a light or a script Enabled, a part CanCollide.
const TRUE_BY_DEFAULT: &[(&str, &str)] = &[
    ("Instance", "Archivable"),
    ("Instance", "archivable"),
    ("BaseCoreGuiConfiguration", "Enabled"),
    ("Constraint", "Enabled"),
    ("GuiObject", "Visible"),
    ("GuiBase2d", "AutoLocalize"),
    ("LayerCollector", "Enabled"),
    ("GuiBase3d", "Visible"),
    ("JointInstance", "Enabled"),
    ("Light", "Enabled"),
    ("BaseScript", "Enabled"),
    ("BasePart", "CanCollide"),
    ("BasePart", "CanQuery"),
    ("BasePart", "CanTouch"),
    ("PostEffect", "Enabled"),
    ("SoundEffect", "Enabled"),
];

pub fn api() -> &'static Api {
    static API_TABLE: OnceLock<Api> = OnceLock::new();
    API_TABLE.get_or_init(parse)
}

fn parse() -> Api {
    let mut classes: HashMap<&'static str, ApiClass> = HashMap::new();
    let mut funcs = Vec::new();
    let mut enums: Vec<(&'static str, Vec<(&'static str, i32)>)> = Vec::new();
    let mut class: Option<ApiClass> = None;
    for line in API.lines() {
        let f: Vec<&'static str> = line.split('\t').collect();
        let at = |i: usize| f.get(i).copied().unwrap_or("");
        match at(0) {
            "C" => {
                if let Some(c) = class.take() {
                    classes.insert(c.name, c);
                }
                let flags = at(3);
                class = Some(ApiClass {
                    name: at(1),
                    base: Some(at(2)).filter(|b| !b.starts_with('<')),
                    creatable: !flags.contains('n'),
                    service: flags.contains('s'),
                    members: Vec::new(),
                });
            }
            "P" | "F" | "E" | "B" => {
                let Some(c) = class.as_mut() else { continue };
                let m = match at(0) {
                    "P" => Member::Prop {
                        cat: at(2),
                        ty: at(3),
                        default: if at(4).is_empty() && TRUE_BY_DEFAULT.contains(&(c.name, at(1))) {
                            "true"
                        } else {
                            at(4)
                        },
                        read_only: at(5).contains('r'),
                        hidden: at(5).contains('h'),
                    },
                    "F" => {
                        funcs.push(Func {
                            class: c.name,
                            name: at(1),
                            cat: at(2),
                            ret: at(3),
                        });
                        Member::Func(funcs.len() as u32 - 1)
                    }
                    "E" => Member::Event,
                    _ => Member::Callback,
                };
                // The first of overloads with one name is the one.
                if !c.members.iter().any(|(n, _)| *n == at(1)) {
                    c.members.push((at(1), m));
                }
            }
            "N" => enums.push((at(1), Vec::new())),
            "I" => {
                if let Some((_, items)) = enums.last_mut() {
                    items.push((at(1), at(2).parse().unwrap_or(0)));
                }
            }
            _ => {}
        }
    }
    if let Some(c) = class.take() {
        classes.insert(c.name, c);
    }
    Api { classes, funcs, enums }
}

pub fn class(name: &str) -> Option<&'static ApiClass> {
    api().classes.get(name)
}

/// Member `name` of class `class` or a class it inherits, and the class
/// that has it.
pub fn member(class_name: &str, name: &str) -> Option<(&'static str, &'static Member)> {
    let mut at = class(class_name);
    while let Some(c) = at {
        if let Some((_, m)) = c.members.iter().find(|(n, _)| *n == name) {
            return Some((c.name, m));
        }
        at = c.base.and_then(class);
    }
    None
}

pub fn func(id: u32) -> Option<&'static Func> {
    api().funcs.get(id.checked_sub(FIRST_ID)? as usize)
}

fn nums(s: &str) -> Vec<f64> {
    s.split([',', ' '])
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.trim().parse().ok())
        .collect()
}

/// A property's value before a script sets it: Roblox's default, or its
/// type's empty value.
pub fn default_value(cat: &str, ty: &str, default: &str) -> Value {
    let n = nums(default);
    let num = |i: usize, or: f64| n.get(i).copied().unwrap_or(or);
    match (cat, ty) {
        ("Primitive", "bool") => Value::Bool(default == "true"),
        ("Primitive", "int" | "int64" | "float" | "double") => Value::Num(default.trim().parse().unwrap_or(0.0)),
        ("Primitive", _) => Value::str(default),
        ("Class", _) => Value::Nil,
        ("Enum", e) => {
            let Some(def) = enum_def(e) else { return Value::Nil };
            let item = def
                .items
                .iter()
                .find(|i| i.0 == default)
                .or_else(|| def.items.first())
                .copied();
            item.map_or(Value::Nil, |(name, value)| {
                enum_value(super::hstudio_rbx_values::EnumItem {
                    enum_name: def.name,
                    name,
                    value,
                })
            })
        }
        (_, "Vector3") => v3(Vector3::new(num(0, 0.0), num(1, 0.0), num(2, 0.0))),
        (_, "Vector2") => v2(Vector2::new(num(0, 0.0), num(1, 0.0))),
        (_, "CFrame") => {
            if n.len() == 12 {
                let mut c = [0.0; 12];
                c.copy_from_slice(&n);
                cf(CFrame::from_components(c))
            } else {
                cf(CFrame::IDENTITY)
            }
        }
        (_, "Color3") => c3(if n.len() == 3 {
            Color3::new(n[0], n[1], n[2])
        } else {
            Color3::new(1.0, 1.0, 1.0)
        }),
        (_, "UDim") => udim(UDim::new(num(0, 0.0), num(1, 0.0))),
        (_, "UDim2") => udim2(UDim2::new(num(0, 0.0), num(1, 0.0), num(2, 0.0), num(3, 0.0))),
        (_, "BrickColor") => wrap(Data::Brick(BrickColor::new(num(0, 194.0) as i64))),
        (_, "NumberRange") => wrap(Data::Range(
            NumberRange::new(num(0, 0.0), num(1, num(0, 0.0))).unwrap_or(NumberRange::single(num(0, 0.0))),
        )),
        (_, "NumberSequence") => wrap(Data::NumSeq(NumberSequence::constant(num(1, 0.0)))),
        (_, "ColorSequence") => wrap(Data::ColorSeq(ColorSequence::constant(Color3::new(1.0, 1.0, 1.0)))),
        (_, "Rect") => wrap(Data::Rect(Rect::default())),
        (_, "Font") => wrap(Data::Font(Font::default())),
        (_, "Content" | "ContentId" | "string" | "BinaryString" | "ProtectedString" | "SharedString" | "QDir") => {
            Value::str(default)
        }
        _ => Value::Nil,
    }
}

/// What a method of the API that the layer doesn't implement returns: its
/// type's empty value (nothing, false, 0, "", an empty table).
pub fn returns(f: &Func) -> Vec<Value> {
    match (f.cat, f.ret) {
        ("Primitive", "void" | "null") => Vec::new(),
        ("Group", _) | (_, "Array" | "Dictionary" | "Objects" | "Instances" | "Map" | "Tuple") => {
            vec![Value::table(Table::new())]
        }
        ("Class", _) => vec![Value::Nil],
        _ => vec![default_value(f.cat, f.ret, "")],
    }
}
