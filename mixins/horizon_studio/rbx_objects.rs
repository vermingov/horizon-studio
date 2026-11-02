// Model files as Instances (hstudio_rbx_objects): `game:GetObjects(url)`
// and `InsertService:LoadLocalAsset(path)`. Where a model comes from (a
// Roblox asset kept in the workspace, a web address, a workspace file), and
// a model file's items (hstudio_rbx_file reads them) as the plain Lua the
// executor's prelude makes Instances of: each item's class, its properties
// as Roblox's values, the items its Ref properties point at, and its parent.

use super::hstudio_lua::{Table, Value};
use super::hstudio_rbx_file::{Content, PropValue, RbxFile};
use super::hstudio_rbx_types as t;
use super::hstudio_rbx_values::{Data, c3, cf, udim, udim2, v2, v3, wrap};

/// The folder in the workspace a Roblox asset is kept in (`<id>.rbxm` or
/// `<id>.rbxmx` for a model, as images are `<id>.png`).
pub const ASSETS: &str = ".roblox-assets";

/// Where a model is.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A Roblox asset by its id (`rbxassetid://123`, a roblox.com asset
    /// address, or the number).
    Asset(u64),
    /// A web address.
    Url(String),
    /// A file in the workspace.
    File(String),
}

/// What `content` names: a Roblox asset, a web address or a workspace file.
pub fn source(content: &str) -> Source {
    let s = content.trim();
    let lower = s.to_ascii_lowercase();
    let digits = |t: &str| -> Option<u64> {
        let d: String = t.chars().take_while(char::is_ascii_digit).collect();
        d.parse().ok()
    };
    if let Some(id) = lower.strip_prefix("rbxassetid://").and_then(digits) {
        return Source::Asset(id);
    }
    if lower.contains("roblox.com/asset")
        && let Some(id) = lower.find("id=").and_then(|at| digits(&lower[at + 3..]))
    {
        return Source::Asset(id);
    }
    if !s.is_empty()
        && s.chars().all(|c| c.is_ascii_digit())
        && let Some(id) = digits(s)
    {
        return Source::Asset(id);
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Source::Url(s.to_owned());
    }
    Source::File(s.to_owned())
}

/// The workspace files a Roblox asset may be kept in.
pub fn asset_files(id: u64) -> [String; 2] {
    [format!("{ASSETS}/{id}.rbxm"), format!("{ASSETS}/{id}.rbxmx")]
}

/// Why a Roblox asset download can't go ahead here, with the account
/// connected (`have_account`: the call isn't in a thread that can wait) or
/// not (connect one in Settings, or save the file by hand).
pub fn asset_blocked(id: u64, have_account: bool) -> String {
    if have_account {
        format!(
            "GetObjects: rbxassetid://{id} has to be downloaded, which this code can't wait for: run it in a thread \
             that can wait (task.spawn), or save it as workspace/{ASSETS}/{id}.rbxm"
        )
    } else {
        format!(
            "GetObjects: rbxassetid://{id} isn't in the workspace and no Roblox account is connected: connect one in \
             the studio's Settings > Roblox account, or save the model by hand as workspace/{ASSETS}/{id}.rbxm or \
             .rbxmx (Roblox Studio: insert it, right-click, Save to File)"
        )
    }
}

/// Where a downloaded asset is cached, and the write of it.
pub fn cache_asset(workspace: &std::path::Path, id: u64, bytes: &[u8]) -> std::io::Result<std::path::PathBuf> {
    let dir = workspace.join(ASSETS);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.rbxm"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Properties a file keeps that scripts don't set (or can't).
const SKIPPED: &[&str] = &[
    "Name",
    "AttributesSerialize",
    "Attributes",
    "Tags",
    "SourceAssetId",
    "HistoryId",
    "UniqueId",
    "Capabilities",
    "DefinesCapabilities",
    "ScriptGuid",
    "LinkedSource",
];

/// A file's name for a property, as scripts name it.
fn prop_name(name: &str) -> &str {
    match name {
        "size" => "Size",
        "Color3uint8" => "Color",
        "shape" => "Shape",
        "formFactorRaw" => "FormFactor",
        other => other,
    }
}

/// A property's value as Lua has it; None for what has no Lua value here.
fn value(v: &PropValue) -> Option<Value> {
    let p3 = |p: &super::hstudio_rbx_file::Vector3| t::Vector3::new(f64::from(p.x), f64::from(p.y), f64::from(p.z));
    let p2 = |p: &super::hstudio_rbx_file::Vector2| t::Vector2::new(f64::from(p.x), f64::from(p.y));
    let col = |c: &super::hstudio_rbx_file::Color3| t::Color3::new(f64::from(c.r), f64::from(c.g), f64::from(c.b));
    let ud = |u: &super::hstudio_rbx_file::UDim| t::UDim::new(f64::from(u.scale), f64::from(u.offset));
    let frame = |c: &super::hstudio_rbx_file::CFrame| {
        let r = c.rotation.map(|row| row.map(f64::from));
        let pos = p3(&c.position);
        t::CFrame::from_components([
            pos.x, pos.y, pos.z, r[0][0], r[0][1], r[0][2], r[1][0], r[1][1], r[1][2], r[2][0], r[2][1], r[2][2],
        ])
    };
    Some(match v {
        PropValue::String(s) => Value::bytes(s),
        PropValue::Bool(b) => Value::Bool(*b),
        PropValue::Int32(n) => Value::Num(f64::from(*n)),
        PropValue::Int64(n) => Value::Num(*n as f64),
        PropValue::Float32(n) => Value::Num(f64::from(*n)),
        PropValue::Float64(n) => Value::Num(*n),
        PropValue::Enum(n) => Value::Num(f64::from(*n)),
        PropValue::UDim(u) => udim(ud(u)),
        PropValue::UDim2(u) => udim2(t::UDim2 {
            x: ud(&u.x),
            y: ud(&u.y),
        }),
        PropValue::Vector2(p) => v2(p2(p)),
        PropValue::Vector3(p) => v3(p3(p)),
        PropValue::Vector2int16([x, y]) => v2(t::Vector2::new(f64::from(*x), f64::from(*y))),
        PropValue::Vector3int16([x, y, z]) => v3(t::Vector3::new(f64::from(*x), f64::from(*y), f64::from(*z))),
        PropValue::Color3(c) => c3(col(c)),
        PropValue::Color3uint8([r, g, b]) => c3(t::Color3::new(
            f64::from(*r) / 255.0,
            f64::from(*g) / 255.0,
            f64::from(*b) / 255.0,
        )),
        PropValue::CFrame(c) | PropValue::OptionalCFrame(Some(c)) => cf(frame(c)),
        PropValue::BrickColor(n) => wrap(Data::Brick(t::BrickColor::new(i64::from(*n)))),
        PropValue::Rect(r) => wrap(Data::Rect(t::Rect::new(p2(&r.min), p2(&r.max)))),
        PropValue::NumberRange(r) => wrap(Data::Range(
            t::NumberRange::new(f64::from(r.min), f64::from(r.max)).ok()?,
        )),
        PropValue::NumberSequence(keys) => wrap(Data::NumSeq(
            t::NumberSequence::new(
                keys.iter()
                    .map(|k| {
                        t::NumberSequenceKeypoint::new(f64::from(k.time), f64::from(k.value), f64::from(k.envelope))
                    })
                    .collect(),
            )
            .ok()?,
        )),
        PropValue::ColorSequence(keys) => wrap(Data::ColorSeq(
            t::ColorSequence::new(
                keys.iter()
                    .map(|k| t::ColorSequenceKeypoint::new(f64::from(k.time), col(&k.color)))
                    .collect(),
            )
            .ok()?,
        )),
        PropValue::Font(f) => wrap(Data::Font(t::Font::new(
            &f.family,
            t::FontWeight::from_value(i32::from(f.weight)).unwrap_or(t::FontWeight::Regular),
            t::FontStyle::from_value(i32::from(f.style)).unwrap_or(t::FontStyle::Normal),
        ))),
        PropValue::Content(Content::Uri(s)) => Value::str(s),
        PropValue::Content(Content::None) => Value::str(""),
        PropValue::SharedString(s) => Value::bytes(s),
        _ => return None,
    })
}

/// A model file's items as the prelude builds them: `{ items = { { class,
/// name, props = { [name] = value }, refs = { [name] = item }, parent }
/// ... }, roots = { item ... } }`, items counted from 1.
pub fn describe(file: &RbxFile) -> Value {
    let mut items = Table::new();
    for it in &file.items {
        let mut e = Table::new();
        e.set_str("class", Value::str(&it.class));
        e.set_str("name", Value::str(&it.name()));
        let mut props = Table::new();
        let mut refs = Table::new();
        for (k, v) in &it.properties {
            if SKIPPED.contains(&k.as_str()) {
                continue;
            }
            let k = prop_name(k);
            match v {
                PropValue::Ref(Some(i)) | PropValue::Content(Content::Object(Some(i))) => {
                    refs.set_str(k, Value::Num((*i + 1) as f64));
                }
                other => {
                    if let Some(v) = value(other) {
                        props.set_str(k, v);
                    }
                }
            }
        }
        if let PropValue::String(tags) = it.get("Tags").unwrap_or(&PropValue::Bool(false)) {
            let list = tags
                .split(|&b| b == 0)
                .filter(|t| !t.is_empty())
                .map(Value::bytes)
                .collect();
            e.set_str("tags", Value::table(Table::from_list(list)));
        }
        e.set_str("props", Value::table(props));
        e.set_str("refs", Value::table(refs));
        if let Some(p) = it.parent {
            e.set_str("parent", Value::Num((p + 1) as f64));
        }
        items.push(Value::table(e));
    }
    let roots = file.roots.iter().map(|r| Value::Num((*r + 1) as f64)).collect();
    let mut out = Table::new();
    out.set_str("items", Value::table(items));
    out.set_str("roots", Value::table(Table::from_list(roots)));
    Value::table(out)
}

/// The Lua that makes a described model's Instances (the executor's
/// prelude runs it): each item as its class (a Folder for one the layer
/// hasn't), its properties (those it can't take are left as they are), its
/// Ref properties, then its parent; the roots back, unparented.
pub const BUILD: &str = r#"
local function build(desc)
    local made = {}
    for i, it in ipairs(desc.items) do
        local ok, inst = pcall(Instance.new, it.class)
        if not ok then
            inst = Instance.new("Folder")
        end
        inst.Name = it.name
        made[i] = inst
    end
    for i, it in ipairs(desc.items) do
        local inst = made[i]
        for k, v in pairs(it.props) do
            pcall(function() inst[k] = v end)
        end
        for k, r in pairs(it.refs) do
            pcall(function() inst[k] = made[r] end)
        end
        if it.tags then
            for _, tag in ipairs(it.tags) do pcall(function() inst:AddTag(tag) end) end
        end
    end
    for i, it in ipairs(desc.items) do
        if it.parent and made[it.parent] then
            made[i].Parent = made[it.parent]
        end
    end
    local roots = {}
    for _, r in ipairs(desc.roots) do
        table.insert(roots, made[r])
    end
    return roots
end
"#;
