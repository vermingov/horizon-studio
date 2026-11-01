// Saving Instances as a Roblox XML model or place (.rbxmx, .rbxlx), the
// format Studio opens: what saveinstance writes. Each property is written
// as Roblox's API types it (rbx_api.rs), from the value the instance has;
// references inside the saved tree are kept, others are null, and scripts'
// sources are ProtectedStrings. The layer's reader (rbx_file.rs) reads
// these back.

use super::hstudio_lua::Value;
use super::hstudio_rbx::{Id, Rbx};
use super::hstudio_rbx_values::{Data, get};
use std::collections::HashMap;
use std::fmt::Write;

/// The XML of `roots` and everything under them (none destroyed), as a
/// model (or a place when the roots are services).
pub fn rbxmx(rbx: &Rbx, roots: &[Id]) -> String {
    let mut refs: HashMap<Id, usize> = HashMap::new();
    let mut order = Vec::new();
    fn walk(rbx: &Rbx, id: Id, refs: &mut HashMap<Id, usize>, order: &mut Vec<Id>) {
        let Some(i) = rbx.try_inst(id).filter(|i| !i.destroyed) else {
            return;
        };
        refs.insert(id, refs.len());
        order.push(id);
        for &c in &i.children {
            walk(rbx, c, refs, order);
        }
    }
    for &r in roots {
        walk(rbx, r, &mut refs, &mut order);
    }
    let mut out = String::from(
        "<roblox xmlns:xmime=\"http://www.w3.org/2005/05/xmlmime\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"http://www.roblox.com/roblox.xsd\" version=\"4\">\n\t<Meta name=\"ExplicitAutoJoints\">true</Meta>\n",
    );
    for &r in roots {
        if refs.contains_key(&r) {
            item(rbx, r, &refs, &mut out, 1);
        }
    }
    out.push_str("</roblox>\n");
    out
}

fn item(rbx: &Rbx, id: Id, refs: &HashMap<Id, usize>, out: &mut String, depth: usize) {
    let Some(inst) = rbx.try_inst(id).filter(|i| !i.destroyed) else {
        return;
    };
    let tab = "\t".repeat(depth);
    let _ = writeln!(
        out,
        "{tab}<Item class=\"{}\" referent=\"RBX{}\">",
        esc(inst.class.name),
        refs[&id]
    );
    let _ = writeln!(out, "{tab}\t<Properties>");
    let _ = writeln!(out, "{tab}\t\t<string name=\"Name\">{}</string>", esc(&inst.name));
    let mut props: Vec<(String, Value)> = inst
        .props
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .chain(inst.extra.iter().map(|(k, v)| (k.clone(), v.clone())))
        .filter(|(k, _)| !matches!(k.as_str(), "Name" | "Parent" | "ClassName"))
        .collect();
    props.sort_by(|a, b| a.0.cmp(&b.0));
    props.dedup_by(|a, b| a.0 == b.0);
    for (name, v) in &props {
        let (cat, ty) = match super::hstudio_rbx_api::member(inst.class.name, name) {
            Some((_, super::hstudio_rbx_api::Member::Prop { cat, ty, .. })) => (*cat, *ty),
            Some(_) => continue,
            None => ("", ""),
        };
        prop(rbx, name, v, cat, ty, refs, out, &format!("{tab}\t\t"));
    }
    let _ = writeln!(out, "{tab}\t</Properties>");
    for &c in &inst.children {
        if refs.contains_key(&c) {
            item(rbx, c, refs, out, depth + 1);
        }
    }
    let _ = writeln!(out, "{tab}</Item>");
}

#[allow(clippy::too_many_arguments)]
fn prop(rbx: &Rbx, name: &str, v: &Value, cat: &str, ty: &str, refs: &HashMap<Id, usize>, out: &mut String, tab: &str) {
    let n = esc(name);
    let line = match v {
        Value::Bool(b) => format!("<bool name=\"{n}\">{b}</bool>"),
        Value::Num(x) => match ty {
            "int" | "BrickColor" => format!("<int name=\"{n}\">{}</int>", *x as i64),
            "int64" => format!("<int64 name=\"{n}\">{}</int64>", *x as i64),
            "float" => format!("<float name=\"{n}\">{x}</float>"),
            _ if cat == "Enum" => format!("<token name=\"{n}\">{}</token>", *x as i64),
            _ => format!("<double name=\"{n}\">{x}</double>"),
        },
        Value::Str(_) => {
            let text = v.as_text().unwrap_or_default();
            if name == "Source" {
                format!(
                    "<ProtectedString name=\"{n}\"><![CDATA[{}]]></ProtectedString>",
                    text.replace("]]>", "]]]]><![CDATA[>")
                )
            } else if matches!(ty, "Content" | "ContentId") {
                if text.is_empty() {
                    format!("<Content name=\"{n}\"><null></null></Content>")
                } else {
                    format!("<Content name=\"{n}\"><url>{}</url></Content>", esc(&text))
                }
            } else {
                format!("<string name=\"{n}\">{}</string>", esc(&text))
            }
        }
        Value::Nil if cat == "Class" => format!("<Ref name=\"{n}\">null</Ref>"),
        Value::Userdata(_) => {
            if let Some(target) = rbx.id_of(v) {
                match refs.get(&target) {
                    Some(r) => format!("<Ref name=\"{n}\">RBX{r}</Ref>"),
                    None => format!("<Ref name=\"{n}\">null</Ref>"),
                }
            } else {
                match get(v) {
                    Some(d) => match data(&n, &d) {
                        Some(s) => s,
                        None => return,
                    },
                    None => return,
                }
            }
        }
        _ => return,
    };
    let _ = writeln!(out, "{tab}{line}");
}

fn data(n: &str, d: &Data) -> Option<String> {
    let f = |x: f64| format!("{x}");
    Some(match d {
        Data::V3(v) => format!(
            "<Vector3 name=\"{n}\"><X>{}</X><Y>{}</Y><Z>{}</Z></Vector3>",
            f(v.x),
            f(v.y),
            f(v.z)
        ),
        Data::V2(v) => format!("<Vector2 name=\"{n}\"><X>{}</X><Y>{}</Y></Vector2>", f(v.x), f(v.y)),
        Data::CF(c) => {
            let k = c.components();
            let tags = [
                "X", "Y", "Z", "R00", "R01", "R02", "R10", "R11", "R12", "R20", "R21", "R22",
            ];
            let body: String = tags.iter().zip(k).map(|(t, x)| format!("<{t}>{x}</{t}>")).collect();
            format!("<CoordinateFrame name=\"{n}\">{body}</CoordinateFrame>")
        }
        Data::C3(c) => format!(
            "<Color3 name=\"{n}\"><R>{}</R><G>{}</G><B>{}</B></Color3>",
            f(c.r),
            f(c.g),
            f(c.b)
        ),
        Data::UD(u) => format!("<UDim name=\"{n}\"><S>{}</S><O>{}</O></UDim>", f(u.scale), u.offset),
        Data::UD2(u) => format!(
            "<UDim2 name=\"{n}\"><XS>{}</XS><XO>{}</XO><YS>{}</YS><YO>{}</YO></UDim2>",
            f(u.x.scale),
            u.x.offset,
            f(u.y.scale),
            u.y.offset
        ),
        Data::Rect(r) => format!(
            "<Rect2D name=\"{n}\"><min><X>{}</X><Y>{}</Y></min><max><X>{}</X><Y>{}</Y></max></Rect2D>",
            f(r.min.x),
            f(r.min.y),
            f(r.max.x),
            f(r.max.y)
        ),
        Data::Range(r) => format!("<NumberRange name=\"{n}\">{} {} </NumberRange>", f(r.min), f(r.max)),
        Data::NumSeq(s) => {
            let body: String = s
                .keypoints()
                .iter()
                .map(|k| format!("{} {} {} ", k.time, k.value, k.envelope))
                .collect();
            format!("<NumberSequence name=\"{n}\">{body}</NumberSequence>")
        }
        Data::ColorSeq(s) => {
            let body: String = s
                .keypoints()
                .iter()
                .map(|k| format!("{} {} {} {} 0 ", k.time, k.value.r, k.value.g, k.value.b))
                .collect();
            format!("<ColorSequence name=\"{n}\">{body}</ColorSequence>")
        }
        Data::Brick(b) => format!("<int name=\"{n}\">{}</int>", b.number()),
        Data::Item(it) => format!("<token name=\"{n}\">{}</token>", it.value),
        Data::Font(font) => format!(
            "<Font name=\"{n}\"><Family><url>{}</url></Family><Weight>{}</Weight><Style>{}</Style></Font>",
            esc(&font.family),
            font.weight.value(),
            font.style.name()
        ),
        Data::Phys(p) => format!(
            "<PhysicalProperties name=\"{n}\"><CustomPhysics>true</CustomPhysics><Density>{}</Density><Friction>{}</Friction><Elasticity>{}</Elasticity><FrictionWeight>{}</FrictionWeight><ElasticityWeight>{}</ElasticityWeight></PhysicalProperties>",
            p[0], p[1], p[2], p[3], p[4]
        ),
        _ => return None,
    })
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t' | '\r') => {
                let _ = write!(out, "&#{};", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}
