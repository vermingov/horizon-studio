// The files of saveinstance's world besides its chunks and entities
// (hstudio_world_files): level.dat and the player (Horizon's own `Level`),
// a void generator beside what was saved, the maps, the scoreboard, the
// server's registries and tags as a data pack, its command tree, icon and
// resource pack. hstudio_world_dump copies what they hold on the frame and
// writes them with this on a thread of its own. It needs no `Game`, so
// tools/dump_probe.rs builds it too, to read the world it writes back.

use horizon_protocol::packets::configuration::RegistryTags;
use horizon_protocol::packets::play::Commands;
use horizon_protocol::registry::{Builtin, Dynamic};
use horizon_world::nbt::Tag;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the world's files other than its chunks hold, copied on the frame.
pub struct Meta {
    pub world: PathBuf,
    pub level: horizon_client::save::Level,
    pub maps: Vec<(i32, Tag)>,
    pub scoreboard: Tag,
    pub border: Tag,
    pub registries: Option<(Arc<Builtin>, Arc<Dynamic>, String)>,
    pub tags: Vec<RegistryTags>,
    pub commands: Option<Commands>,
    pub icon: Option<Vec<u8>>,
    pub packs: Vec<PathBuf>,
}

fn dat(path: PathBuf, data: Tag) -> Result<(), String> {
    horizon_client::save::write_dat(
        &path,
        Tag::compound([("data", data), ("DataVersion", Tag::Int(horizon_world::DATA_VERSION))]),
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

impl Meta {
    pub fn write(mut self) -> Result<(), String> {
        let world = self.world.clone();
        let data = world.join("data").join("minecraft");
        // A void world beside what was saved: no terrain of a generator's
        // own next to the server's (a world downloader's usual choice).
        if !data.join("world_gen_settings.dat").is_file() {
            dat(data.join("world_gen_settings.dat"), void_world_gen(self.level.seed))?;
        }
        let datapack = match &self.registries {
            Some((builtin, dynamic, dimension)) => {
                write_datapack(&world, builtin, dynamic, &self.tags, dimension, &self.level.dimension)?
            }
            None => false,
        };
        if datapack {
            self.level.data_packs = Some((
                vec!["vanilla".into(), format!("file/{DATAPACK}")],
                ["minecart_improvements", "redstone_experiments", "trade_rebalance"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ));
        }
        self.level.write(&world).map_err(|e| format!("level.dat: {e}"))?;
        let mut last = None;
        for (id, tag) in std::mem::take(&mut self.maps) {
            horizon_client::save::write_dat(&data.join("maps").join(format!("{id}.dat")), tag)
                .map_err(|e| format!("map {id}: {e}"))?;
            last = last.max(Some(id));
        }
        if let Some(last) = last {
            dat(
                data.join("maps").join("last_id.dat"),
                Tag::compound([("map", Tag::Int(last))]),
            )?;
        }
        dat(data.join("scoreboard.dat"), self.scoreboard.clone())?;
        dat(
            horizon_client::save::dimension_dir(&world, "minecraft:overworld").join("data/minecraft/world_border.dat"),
            self.border.clone(),
        )?;
        if let Some(c) = &self.commands {
            let text = serde_json::to_string_pretty(&command_tree(c)).unwrap_or_default();
            std::fs::write(world.join("server_commands.json"), text).map_err(|e| format!("the command tree: {e}"))?;
        }
        if let Some(icon) = self.icon.as_ref().filter(|i| i.starts_with(b"\x89PNG")) {
            std::fs::write(world.join("icon.png"), icon).map_err(|e| format!("icon.png: {e}"))?;
        }
        // The server's pack on top: the world's own (`resources.zip`), which
        // opening it offers; the ones under it beside it.
        if let Some((top, rest)) = self.packs.split_last() {
            let dir = world.join("resourcepacks");
            std::fs::create_dir_all(&dir).map_err(|e| format!("resourcepacks: {e}"))?;
            std::fs::copy(top, dir.join("resources.zip")).map_err(|e| format!("the server's resource pack: {e}"))?;
            for (i, p) in rest.iter().enumerate() {
                let _ = std::fs::copy(p, dir.join(format!("server_{i}.zip")));
            }
        }
        Ok(())
    }
}

/// `world_gen_settings`: every dimension a flat world of air over the void
/// biome, without structures.
pub fn void_world_gen(seed: i64) -> Tag {
    let s = |v: &str| Tag::String(v.into());
    let void = |kind: &str| {
        Tag::compound([
            ("type", s(kind)),
            (
                "generator",
                Tag::compound([
                    ("type", s("minecraft:flat")),
                    (
                        "settings",
                        Tag::compound([
                            ("biome", s("minecraft:the_void")),
                            ("features", Tag::Byte(0)),
                            ("lakes", Tag::Byte(0)),
                            (
                                "layers",
                                Tag::List(vec![Tag::compound([
                                    ("block", s("minecraft:air")),
                                    ("height", Tag::Int(1)),
                                ])]),
                            ),
                            ("structure_overrides", Tag::List(Vec::new())),
                        ]),
                    ),
                ]),
            ),
        ])
    };
    Tag::compound([
        ("seed", Tag::Long(seed)),
        ("generate_structures", Tag::Byte(0)),
        ("bonus_chest", Tag::Byte(0)),
        (
            "dimensions",
            Tag::compound([
                ("minecraft:overworld", void("minecraft:overworld")),
                ("minecraft:the_nether", void("minecraft:the_nether")),
                ("minecraft:the_end", void("minecraft:the_end")),
            ]),
        ),
    ])
}

// ---------------------------------------------------------------- the server's registries

/// The data pack's folder in `datapacks/`.
pub const DATAPACK: &str = "server";

/// One part of a resource path, as a file name: what an identifier allows.
fn plain_part(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
}

/// `namespace:a/b` as its parts, if each is a plain one.
fn id_parts(id: &str) -> Option<(String, Vec<String>)> {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let parts: Vec<String> = path.split('/').map(str::to_owned).collect();
    (plain_part(ns) && parts.iter().all(|p| plain_part(p))).then(|| (ns.to_owned(), parts))
}

/// A registry's folder in a data pack: its path, under its namespace when
/// that isn't minecraft's (`Registries.elementsDirPath`).
fn registry_dir(registry: &str) -> Option<Vec<String>> {
    let (ns, mut parts) = id_parts(registry)?;
    if ns != "minecraft" {
        parts.insert(0, ns);
    }
    Some(parts)
}

fn json_file(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap_or_default())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The registries the server sent in the configuration (biomes, dimension
/// types, enchantments, damage types, painting variants...) and its tags,
/// as the data pack `datapacks/server`. The dimension the dump is of gets
/// the server's dimension type of it (its height). Whether there was one.
pub fn write_datapack(
    world: &Path,
    builtin: &Builtin,
    dynamic: &Dynamic,
    tags: &[RegistryTags],
    dimension_type: &str,
    dimension: &str,
) -> Result<bool, String> {
    if dynamic.is_empty() {
        return Ok(false);
    }
    let pack = world.join("datapacks").join(DATAPACK);
    // The pack is the dump's own: written anew each time.
    let _ = std::fs::remove_dir_all(&pack);
    let format = horizon_core::version::DATA_PACK_FORMAT.0;
    json_file(
        &pack.join("pack.mcmeta"),
        &serde_json::json!({
            "pack": {
                "description": "The server's registries and tags, saved by Horizon Studio's saveinstance",
                "min_format": format,
                "max_format": format,
            }
        }),
    )?;
    let data = pack.join("data");
    let file = |ns: &str, dir: &[String], path: &[String]| {
        let mut p = data.join(ns);
        for d in dir.iter().chain(path) {
            p.push(d);
        }
        p.set_extension("json");
        p
    };
    for (registry, entries) in &dynamic.registries {
        let Some(dir) = registry_dir(registry) else { continue };
        for (name, tag) in entries {
            let (Some((ns, path)), Some(tag)) = (id_parts(name), tag) else {
                continue;
            };
            json_file(&file(&ns, &dir, &path), &nbt_json(tag))?;
        }
    }
    // The dimension saved is the server's dimension type, whatever its name.
    if let Some(tag) = dynamic
        .registries
        .get("minecraft:dimension_type")
        .and_then(|l| l.iter().find(|(n, _)| n == dimension_type))
        .and_then(|(_, t)| t.as_ref())
        && let Some((_, path)) = id_parts(dimension)
    {
        json_file(&file("minecraft", &["dimension_type".into()], &path), &nbt_json(tag))?;
    }
    for r in tags {
        let Some(mut dir) = registry_dir(&r.registry) else {
            continue;
        };
        dir.insert(0, "tags".into());
        for t in &r.tags {
            let Some((ns, path)) = id_parts(&t.name) else { continue };
            let values: Vec<String> = t
                .ids
                .iter()
                .filter_map(|&id| {
                    builtin
                        .get(&r.registry)
                        .and_then(|ids| ids.name(id))
                        .or_else(|| dynamic.name(&r.registry, id))
                        .map(str::to_owned)
                })
                .collect();
            json_file(
                &file(&ns, &dir, &path),
                &serde_json::json!({ "replace": false, "values": values }),
            )?;
        }
    }
    Ok(true)
}

/// A registry entry's NBT as the JSON a data pack holds: a byte of 0 or 1
/// is a boolean (as NbtOps writes them), 26.3's mixed lists unwrapped.
pub fn nbt_json(t: &Tag) -> serde_json::Value {
    use serde_json::Value as J;
    let float = |f: f64| serde_json::Number::from_f64(f).map_or(J::Null, J::Number);
    match t {
        Tag::End => J::Null,
        Tag::Byte(b) if *b == 0 || *b == 1 => J::Bool(*b == 1),
        Tag::Byte(b) => J::from(*b),
        Tag::Short(v) => J::from(*v),
        Tag::Int(v) => J::from(*v),
        Tag::Long(v) => J::from(*v),
        // The float's own shortest digits, not its double's.
        Tag::Float(bits) => float(f32::from_bits(*bits).to_string().parse().unwrap_or(0.0)),
        Tag::Double(bits) => float(f64::from_bits(*bits)),
        Tag::String(s) => J::String(s.to_string_lossy()),
        Tag::List(l) => J::Array(
            l.iter()
                .map(|e| match e {
                    Tag::Compound(m) if m.len() == 1 && m.keys().next().is_some_and(|k| k.0.is_empty()) => {
                        nbt_json(m.values().next().unwrap_or(&Tag::End))
                    }
                    other => nbt_json(other),
                })
                .collect(),
        ),
        Tag::Compound(m) => J::Object(m.iter().map(|(k, v)| (k.to_string_lossy(), nbt_json(v))).collect()),
        Tag::ByteArray(v) => J::Array(v.iter().map(|&b| J::from(b as i8)).collect()),
        Tag::IntArray(v) => J::Array(v.iter().map(|&i| J::from(i)).collect()),
        Tag::LongArray(v) => J::Array(v.iter().map(|&i| J::from(i)).collect()),
    }
}

/// The server's command tree as the data generator's `commands.json`
/// report writes one: each node's type, whether it runs, its argument's
/// parser and its children by name; a redirect as the path to its node.
pub fn command_tree(c: &Commands) -> serde_json::Value {
    use serde_json::{Map, Value as J};
    // Each node's path from the root, for redirects.
    let mut paths: HashMap<i32, Vec<String>> = HashMap::new();
    let mut queue = VecDeque::from([(c.root, Vec::new())]);
    while let Some((i, path)) = queue.pop_front() {
        if paths.contains_key(&i) {
            continue;
        }
        let Some(n) = usize::try_from(i).ok().and_then(|i| c.nodes.get(i)) else {
            continue;
        };
        for &child in &n.children {
            if let Some(cn) = usize::try_from(child).ok().and_then(|i| c.nodes.get(i)) {
                let mut p = path.clone();
                p.push(cn.name.clone());
                queue.push_back((child, p));
            }
        }
        paths.insert(i, path);
    }
    fn node(c: &Commands, i: i32, paths: &HashMap<i32, Vec<String>>, depth: usize) -> J {
        let mut o = Map::new();
        let Some(n) = usize::try_from(i).ok().and_then(|i| c.nodes.get(i)) else {
            return J::Object(o);
        };
        let kind = match n.kind {
            0 => "root",
            1 => "literal",
            _ => "argument",
        };
        o.insert("type".into(), J::from(kind));
        if let Some((id, props)) = &n.parser {
            let parser = usize::try_from(*id)
                .ok()
                .and_then(|i| horizon_protocol::packets::play::ARGUMENT_TYPES.get(i))
                .map_or_else(|| J::from(*id), |p| J::from(*p));
            o.insert("parser".into(), parser);
            if !props.is_empty() {
                let hex: String = props.iter().map(|b| format!("{b:02x}")).collect();
                o.insert("properties".into(), J::from(hex));
            }
        }
        if let Some(s) = &n.suggestions {
            o.insert("suggestions".into(), J::from(s.as_str()));
        }
        if n.executable {
            o.insert("executable".into(), J::Bool(true));
        }
        if let Some(r) = n.redirect {
            let path = paths.get(&r).cloned().unwrap_or_default();
            o.insert("redirect".into(), J::from(path));
        }
        if !n.children.is_empty() && depth < 64 {
            let mut children = Map::new();
            for &child in &n.children {
                if let Some(cn) = usize::try_from(child).ok().and_then(|i| c.nodes.get(i)) {
                    children.insert(cn.name.clone(), node(c, child, paths, depth + 1));
                }
            }
            o.insert("children".into(), J::Object(children));
        }
        J::Object(o)
    }
    node(c, c.root, &paths, 0)
}
