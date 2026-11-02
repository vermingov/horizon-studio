// What the server sends the client, as Roblox instances
// (hstudio_rbx_server): ReplicatedStorage.Server. A Minecraft server runs no
// code on the client; what reaches it is data the client acts on, and that
// is what this mirrors, rebuilt from the game's data each session:
//
// - ResourcePacks.<pack>: the server's resource packs, their file trees as
//   Folders, each file a ModuleScript whose Source is Lua that gives the
//   file back: a JSON file (models, blockstates, lang, sounds.json, fonts,
//   atlases, pack.mcmeta, post effects) the table literal of its data, a
//   shader (GLSL, which runs on the GPU) or another text its text as a long
//   string, an image its size and a workspace copy getcustomasset takes.
//   Changing a file's Source (to Lua that still gives the file's kind of
//   data) writes the file into the studio's override pack, stacked above
//   the server's, and the game's resources reload with it; anything else
//   is refused with an error and nothing changes.
// - Plugins.<namespace>: the plugins the server's channels (its
//   minecraft:register list and the traffic) and commands name: a Commands
//   ModuleScript of theirs and a RemoteEvent per channel, wired to the
//   channel's custom payloads (FireServer sends one, OnClientEvent hears
//   the server's).
// - Commands: the server's command tree, a ModuleScript per command.
// - Registries: the registries and tags the server sent while configuring.
// - Channels and README: every channel, and what can't be here: the
//   plugins' Java code and data pack functions run on the server only.
//
// The tree fills as it is looked at: a folder of a pack (and the Commands
// and Registries folders) makes its children when a script first reads
// them (GetChildren, FindFirstChild, an index by name, GetDescendants
// inside the tree) or an explorer shows it (tostring), so joining a server
// with a pack of thousands of files makes a handful of instances.

use super::hstudio::GameApi;
use super::hstudio_lua::{Lua, Res, Throw, Value};
use super::hstudio_rbx::{Id, Rbx};
use super::hstudio_rbx_remote::{Field, Packet};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The parts of what the server sent, each with a version of its own.
pub const PACKS: usize = 0;
pub const CHANNELS: usize = 1;
pub const COMMANDS: usize = 2;
pub const REGISTRIES: usize = 3;

/// The override pack's folder in the studio's folder, and the pack in it.
pub const OVERRIDES: &str = "server-overrides";
pub const OVERRIDE_PACK: &str = "hstudio-overrides";

/// The most files a pack's index holds (a pack past it shows the first).
const MAX_FILES: usize = 200_000;
/// The largest file a ModuleScript holds the data of.
const MAX_FILE: u64 = 16 << 20;
/// The largest payload FireServer sends (ServerboundCustomPayloadPacket's).
const MAX_PAYLOAD: usize = 32_767;

/// What the server sent that the client acts on, from the game.
#[derive(Clone, Debug, Default)]
pub struct ServerContent {
    /// The connection: another one starts the tree anew.
    pub session: u64,
    /// Each part's version (PACKS, CHANNELS, COMMANDS, REGISTRIES).
    pub versions: [u64; 4],
    /// The parts whose version is new to the caller; None: as it had them.
    pub packs: Option<Vec<ServerPack>>,
    pub channels: Option<Vec<String>>,
    /// The command tree as the data generator's commands.json has it.
    pub commands: Option<serde_json::Value>,
    pub registries: Option<Registries>,
}

/// One of the server's resource packs: the game's id for it and its file
/// (a zip) or folder.
#[derive(Clone, Debug)]
pub struct ServerPack {
    pub id: String,
    pub path: PathBuf,
}

/// The registries the server sent (each entry's data as a data pack's
/// JSON), and their tags (each tag's entries by name).
#[derive(Clone, Debug, Default)]
pub struct Registries {
    pub entries: Vec<(String, Vec<(String, serde_json::Value)>)>,
    pub tags: Vec<(String, Vec<Tag>)>,
}

/// A tag: its name and its entries' names.
pub type Tag = (String, Vec<String>);

/// What a pack's file is, as its ModuleScript gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// JSON (and .mcmeta): a table.
    Json,
    /// Text (shaders, .txt, .properties): a string.
    Text,
    /// A PNG: its size and a copy in the workspace.
    Image,
    /// Anything else (sounds): its size.
    Binary,
}

/// The kind of a pack's file, by its name.
pub fn kind_of(path: &str) -> Kind {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "json" | "mcmeta" => Kind::Json,
        "vsh" | "fsh" | "glsl" | "csh" | "txt" | "properties" | "lang" | "md" | "yml" | "yaml" | "toml" => Kind::Text,
        "png" => Kind::Image,
        _ => Kind::Binary,
    }
}

enum Source {
    Zip(PathBuf),
    Dir(PathBuf),
}

#[derive(Default)]
struct Dir {
    dirs: Vec<String>,
    files: Vec<String>,
}

struct Pack {
    name: String,
    source: Source,
    /// Each folder's folders and files, by its path ("" the root), once a
    /// folder of it was opened.
    index: Option<HashMap<String, Dir>>,
    zip: Option<zip::ZipArchive<std::fs::File>>,
}

impl Pack {
    fn index(&mut self) -> &HashMap<String, Dir> {
        if self.index.is_none() {
            let names = match &self.source {
                Source::Zip(path) => {
                    if self.zip.is_none() {
                        self.zip = std::fs::File::open(path)
                            .ok()
                            .and_then(|f| zip::ZipArchive::new(f).ok());
                    }
                    // By index and raw names: the game's zip and the studio's read alike.
                    let mut names = Vec::new();
                    if let Some(z) = self.zip.as_mut() {
                        for i in 0..z.len().min(MAX_FILES) {
                            if let Ok(f) = z.by_index_raw(i) {
                                let n = String::from_utf8_lossy(f.name_raw()).into_owned();
                                if !n.ends_with('/') {
                                    names.push(n);
                                }
                            }
                        }
                    }
                    names
                }
                Source::Dir(dir) => {
                    let mut out = Vec::new();
                    walk(dir, "", &mut out);
                    out
                }
            };
            self.index = Some(index_of(&names));
        }
        self.index.get_or_insert_with(HashMap::new)
    }

    fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        match &self.source {
            Source::Zip(_) => {
                let z = self.zip.as_mut()?;
                let mut f = z.by_name(path).ok()?;
                if f.size() > MAX_FILE {
                    return None;
                }
                let mut out = Vec::with_capacity(f.size() as usize);
                f.read_to_end(&mut out).ok()?;
                Some(out)
            }
            Source::Dir(dir) => {
                let p = dir.join(path);
                if std::fs::metadata(&p).ok()?.len() > MAX_FILE {
                    return None;
                }
                std::fs::read(p).ok()
            }
        }
    }
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&e.path(), &format!("{path}/"), out),
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

/// A pack's files as folders: each folder's folders and files, sorted.
fn index_of(names: &[String]) -> HashMap<String, Dir> {
    let mut index: HashMap<String, Dir> = HashMap::new();
    index.entry(String::new()).or_default();
    for name in names {
        // Not a path that leaves the pack, or an empty part.
        if name.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
            continue;
        }
        let mut dir = String::new();
        let parts: Vec<&str> = name.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == parts.len();
            let entry = index.entry(dir.clone()).or_default();
            if last {
                entry.files.push((*part).to_owned());
            } else {
                if !entry.dirs.iter().any(|d| d == part) {
                    entry.dirs.push((*part).to_owned());
                }
                dir = if dir.is_empty() {
                    (*part).to_owned()
                } else {
                    format!("{dir}/{part}")
                };
            }
        }
    }
    for d in index.values_mut() {
        d.dirs.sort();
        d.files.sort();
        d.files.dedup();
    }
    index
}

/// What a folder fills with, when it does.
enum Lazy {
    PackDir { pack: usize, dir: String },
    Commands,
    Registries,
    Tags,
}

/// A pack file's ModuleScript.
struct File {
    pack: usize,
    path: String,
    kind: Kind,
}

/// ReplicatedStorage.Server and what it was made of.
#[derive(Default)]
pub struct Server {
    root: Option<Id>,
    session: Option<u64>,
    versions: [u64; 4],
    packs: Vec<Pack>,
    resource_packs: Option<Id>,
    plugins: Option<Id>,
    commands_folder: Option<Id>,
    registries_folder: Option<Id>,
    channels_module: Option<Id>,
    lazy: HashMap<Id, Lazy>,
    files: HashMap<Id, File>,
    /// The ModuleScripts that are what the server sent (commands,
    /// registries, the lists): their Source can't be changed.
    readonly: HashSet<Id>,
    /// Each plugin's folder, and its Commands ModuleScript.
    plugin_folders: BTreeMap<String, (Id, Option<Id>)>,
    /// The channels' RemoteEvents.
    channels: HashMap<Id, String>,
    by_channel: BTreeMap<String, Id>,
    commands: Option<serde_json::Value>,
    registries: Option<Registries>,
    /// Files edited, to write into the override pack.
    edits: Vec<(String, Vec<u8>)>,
    /// The override pack was emptied for this session.
    started_override: bool,
    /// What the console is to say.
    pub notes: Vec<(bool, String)>,
}

impl Server {
    /// Whether `id` is a folder that fills when it is looked in.
    pub fn is_lazy(&self, id: Id) -> bool {
        self.lazy.contains_key(&id)
    }
    /// The channel a RemoteEvent of a plugin's is wired to.
    pub fn channel_of(&self, id: Id) -> Option<&str> {
        self.channels.get(&id).map(String::as_str)
    }
    /// Whether `id` is ReplicatedStorage.Server or in it.
    pub fn holds(&self, rbx: &Rbx, id: Id) -> bool {
        self.root.is_some_and(|r| r == id || rbx.is_descendant_of(id, r))
    }
    /// Whether `id` is one of the tree's ModuleScripts whose Source is
    /// looked after here (a pack's file, or what reads only).
    pub fn owns_source(&self, id: Id) -> bool {
        self.files.contains_key(&id) || self.readonly.contains(&id)
    }
}

// ---------------------------------------------------------------- JSON as Lua

const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while", "continue",
];

fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
        && !KEYWORDS.contains(&s)
}

/// A Lua string literal of `s`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => out.push_str(&format!("\\{}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Lua long string of `text`: `[==[`...`]==]` at a level the text has no
/// closing bracket of; the newline after the opening one is Lua's to skip.
pub fn long_string(text: &str) -> String {
    let mut level = 0;
    while text.contains(&format!("]{}]", "=".repeat(level))) {
        level += 1;
    }
    let eq = "=".repeat(level);
    format!("[{eq}[\n{text}]{eq}]")
}

fn number(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(0.0);
    let s = format!("{f}");
    if s.contains(['.', 'e', 'E', 'i', 'N']) {
        s
    } else {
        format!("{s}.0")
    }
}

fn scalar(v: &serde_json::Value) -> bool {
    !matches!(v, serde_json::Value::Array(_) | serde_json::Value::Object(_))
}

/// JSON as a Lua table literal (`{ key = value, ... }`, lists as `{ a, b }`),
/// `depth` tabs in.
pub fn lua_of(v: &serde_json::Value, depth: usize) -> String {
    use serde_json::Value as J;
    match v {
        // A JSON null has no Lua form in a table: json_null stands for it.
        J::Null => "json_null".into(),
        J::Bool(b) => b.to_string(),
        J::Number(n) => number(n),
        J::String(s) => quote(s),
        J::Array(a) if a.is_empty() => "{}".into(),
        J::Array(a) => {
            // A short list of plain values on one line ({ 0, 0, 16 }).
            if a.iter().all(scalar) {
                let line = format!(
                    "{{ {} }}",
                    a.iter().map(|x| lua_of(x, 0)).collect::<Vec<_>>().join(", ")
                );
                if line.len() <= 96 {
                    return line;
                }
            }
            let pad = "\t".repeat(depth + 1);
            let mut out = String::from("{\n");
            for x in a {
                out.push_str(&format!("{pad}{},\n", lua_of(x, depth + 1)));
            }
            out.push_str(&"\t".repeat(depth));
            out.push('}');
            out
        }
        J::Object(o) if o.is_empty() => "{}".into(),
        J::Object(o) => {
            let pad = "\t".repeat(depth + 1);
            let mut out = String::from("{\n");
            for (k, x) in o {
                let key = if is_name(k) {
                    k.clone()
                } else {
                    format!("[{}]", quote(k))
                };
                out.push_str(&format!("{pad}{key} = {},\n", lua_of(x, depth + 1)));
            }
            out.push_str(&"\t".repeat(depth));
            out.push('}');
            out
        }
    }
}

/// Lua's value as JSON, shaped like `shape` (the file as it was) where it
/// says nothing itself: an empty table is an array where the file had one,
/// and an object's keys keep the file's order, new ones after.
pub fn json_of(
    v: &Value,
    shape: Option<&serde_json::Value>,
    null: &Value,
    at: &str,
    depth: usize,
) -> Result<serde_json::Value, String> {
    use serde_json::Value as J;
    if depth > 128 {
        return Err("tables nested too deep (or a table inside itself)".into());
    }
    let place = || {
        if at.is_empty() {
            "the value".to_owned()
        } else {
            at.to_owned()
        }
    };
    if !null.is_nil() && same(v, null) {
        return Ok(J::Null);
    }
    Ok(match v {
        Value::Nil => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Num(n) if !n.is_finite() => return Err(format!("{}: {n} isn't a number JSON has", place())),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => J::from(*n as i64),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(J::Null, J::Number),
        Value::Str(s) => match std::str::from_utf8(s) {
            Ok(s) => J::String(s.to_owned()),
            Err(_) => return Err(format!("{}: the text isn't UTF-8", place())),
        },
        Value::Table(t) => {
            let t = t.borrow();
            let entries = t.entries();
            let n = t.len();
            let shape_list = matches!(shape, Some(J::Array(_)));
            if entries.is_empty() {
                return Ok(if shape_list {
                    J::Array(Vec::new())
                } else {
                    J::Object(serde_json::Map::new())
                });
            }
            if n > 0 && entries.len() == n {
                let items = t.list();
                let mut out = Vec::with_capacity(items.len());
                for (i, x) in items.iter().enumerate() {
                    let s = match shape {
                        Some(J::Array(a)) => a.get(i).or(a.first()),
                        _ => None,
                    };
                    out.push(json_of(x, s, null, &format!("{}[{}]", at, i + 1), depth + 1)?);
                }
                J::Array(out)
            } else {
                let mut keyed: Vec<(String, Value)> = Vec::with_capacity(entries.len());
                for (k, x) in entries {
                    match &k {
                        Value::Str(s) => match std::str::from_utf8(s) {
                            Ok(s) => keyed.push((s.to_owned(), x)),
                            Err(_) => return Err(format!("{}: a key isn't UTF-8", place())),
                        },
                        other => {
                            return Err(format!(
                                "{}: a table is a list (keys 1 to n) or has names for keys, not a {} key ({})",
                                place(),
                                other.type_name(),
                                other.plain_text()
                            ));
                        }
                    }
                }
                let order: Vec<&String> = match shape {
                    Some(J::Object(o)) => o.keys().collect(),
                    _ => Vec::new(),
                };
                keyed.sort_by_key(|(k, _)| order.iter().position(|o| *o == k).unwrap_or(usize::MAX));
                let mut out = serde_json::Map::new();
                for (k, x) in keyed {
                    let s = match shape {
                        Some(J::Object(o)) => o.get(&k),
                        _ => None,
                    };
                    let path = if at.is_empty() {
                        k.clone()
                    } else if is_name(&k) {
                        format!("{at}.{k}")
                    } else {
                        format!("{at}[{}]", quote(&k))
                    };
                    out.insert(k, json_of(&x, s, null, &path, depth + 1)?);
                }
                J::Object(out)
            }
        }
        other => return Err(format!("{}: a {} has no JSON form", place(), other.type_name())),
    })
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Table(x), Value::Table(y)) => std::rc::Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// A JSON file as Minecraft reads it (leniently: a byte order mark, what
/// follows the value): none for what isn't JSON.
pub fn parse_json(bytes: &[u8]) -> Option<serde_json::Value> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    serde_json::Deserializer::from_slice(bytes)
        .into_iter::<serde_json::Value>()
        .next()?
        .ok()
}

/// A PNG's width and height, from its header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// A pack file's Source: Lua that gives its data back.
pub fn file_source(pack: &str, path: &str, bytes: &[u8], workspace_copy: Option<&str>) -> String {
    let head = format!("-- {path}, from the server's resource pack \"{pack}\".\n");
    match kind_of(path) {
        Kind::Json => match parse_json(bytes) {
            Some(v) => {
                let null = if lua_of(&v, 0).contains("json_null") {
                    "local json_null = json_null or {} -- what stands for JSON's null\n"
                } else {
                    ""
                };
                format!(
                    "{head}-- require() gives its data. Change the table and set Source to it: the file goes into the\n\
                     -- studio's override pack, above the server's, and the game's resources reload.\n{null}return {}\n",
                    lua_of(&v, 0)
                )
            }
            None => format!(
                "{head}-- It isn't JSON as it is (Minecraft reads it leniently): its text as it is.\nreturn {}\n",
                long_string(&String::from_utf8_lossy(bytes))
            ),
        },
        Kind::Text => {
            let what = if path.ends_with(".vsh")
                || path.ends_with(".fsh")
                || path.ends_with(".glsl")
                || path.ends_with(".csh")
            {
                "GLSL the game compiles for the GPU (Horizon translates it with naga)"
            } else {
                "its text"
            };
            format!(
                "{head}-- require() gives {what}. Change the text and set Source to it: the file goes into the\n\
                 -- studio's override pack, above the server's, and the game's resources reload.\nreturn {}\n",
                long_string(&String::from_utf8_lossy(bytes))
            )
        }
        Kind::Image => {
            let (w, h) = png_size(bytes).unwrap_or((0, 0));
            let mut out = format!(
                "{head}-- A {w}x{h} PNG. Image is what an ImageLabel's Image takes (getcustomasset of the copy\n\
                 -- in the workspace). Its pixels aren't Lua: to change it, write a PNG there and use it.\n"
            );
            match workspace_copy {
                Some(file) => out.push_str(&format!(
                    "local file = {}\nreturn {{\n\tPath = {},\n\tWidth = {w},\n\tHeight = {h},\n\tBytes = {},\n\tFile = file,\n\tImage = getcustomasset and getcustomasset(file) or file,\n}}\n",
                    quote(file),
                    quote(path),
                    bytes.len()
                )),
                None => out.push_str(&format!(
                    "return {{\n\tPath = {},\n\tWidth = {w},\n\tHeight = {h},\n\tBytes = {},\n}}\n",
                    quote(path),
                    bytes.len()
                )),
            }
            out
        }
        Kind::Binary => format!(
            "{head}-- Not text (a sound, a font's glyphs...): what it is.\nreturn {{\n\tPath = {},\n\tBytes = {},\n}}\n",
            quote(path),
            bytes.len()
        ),
    }
}

// ---------------------------------------------------------------- building

fn module(rbx: &mut Rbx, name: &str, source: &str, parent: Id) -> Id {
    let m = rbx.make("ModuleScript", None);
    rbx.inst_mut(m).name = name.to_owned();
    rbx.raw_set(m, "Source", Value::str(source));
    let _ = rbx.set_parent(m, Some(parent));
    rbx.inst_mut(m).locked = true;
    m
}

fn folder(rbx: &mut Rbx, name: &str, parent: Id) -> Id {
    let f = rbx.make("Folder", None);
    rbx.inst_mut(f).name = name.to_owned();
    let _ = rbx.set_parent(f, Some(parent));
    rbx.inst_mut(f).locked = true;
    f
}

fn attribute(rbx: &mut Rbx, id: Id, name: &str, v: Value) {
    let a = &mut rbx.inst_mut(id).attributes;
    a.retain(|(k, _)| k != name);
    a.push((name.to_owned(), v));
}

const README: &str = r#"-- ReplicatedStorage.Server: what this Minecraft server sent the client, as it acts on it.
-- A Minecraft server runs no code on the client. Its plugins (Java) and its data packs'
-- functions run on the server and never reach the client, so they can't be here: only
-- what they send can.
--   ResourcePacks  its resource packs, file by file: models, blockstates, textures, sounds,
--                  lang, fonts, atlases, and core and post shaders (GLSL that runs on the GPU).
--                  require() gives a file's data; set a file's Source to change it (the
--                  studio's override pack, above the server's, and a resource reload).
--   Plugins        each plugin its channels and commands name: a RemoteEvent per channel
--                  (FireServer sends a custom payload on it, OnClientEvent hears the server's)
--                  and its Commands.
--   Commands       the command tree the server sent, a ModuleScript per command.
--   Registries     the registries and tags the server sent while the client configured.
--   Channels       every plugin channel the server registered or used.
return "ReplicatedStorage.Server: what the server sent the client"
"#;

/// The tree follows the game: built when the client is on a server, each
/// part made again when the server sends it anew, all of it anew for
/// another connection. Then the edits go into the override pack.
pub fn step(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if !rbx.active {
        return;
    }
    let have = if rbx.server.root.is_some() {
        (rbx.server.session, rbx.server.versions)
    } else {
        (None, [0; 4])
    };
    if let Some(content) = game.server_content(have.0, have.1) {
        apply(rbx, content);
    }
    write_edits(rbx, game);
}

fn apply(rbx: &mut Rbx, c: ServerContent) {
    if rbx.server.session != Some(c.session)
        || rbx
            .server
            .root
            .is_none_or(|r| rbx.try_inst(r).is_none_or(|i| i.destroyed))
    {
        reset(rbx);
        rbx.server.session = Some(c.session);
    }
    let Some(rs) = rbx.service("ReplicatedStorage") else {
        return;
    };
    if rbx.server.root.is_none() {
        {
            // Made whole, then put in: one DescendantAdded each.
            let r = rbx.make("Folder", None);
            rbx.inst_mut(r).name = "Server".into();
            module(rbx, "README", README, r);
            let packs = folder(rbx, "ResourcePacks", r);
            let plugins = folder(rbx, "Plugins", r);
            let commands = folder(rbx, "Commands", r);
            let registries = folder(rbx, "Registries", r);
            let channels = module(rbx, "Channels", "return {}\n", r);
            for id in [r, packs, plugins, commands, registries] {
                rbx.inst_mut(id).locked = true;
            }
            let s = &mut rbx.server;
            s.root = Some(r);
            s.resource_packs = Some(packs);
            s.plugins = Some(plugins);
            s.commands_folder = Some(commands);
            s.registries_folder = Some(registries);
            s.channels_module = Some(channels);
            s.readonly.insert(channels);
            s.lazy.insert(commands, Lazy::Commands);
            s.lazy.insert(registries, Lazy::Registries);
            let _ = rbx.set_parent(r, Some(rs));
            rbx.inst_mut(r).locked = true;
        }
    }
    if let Some(packs) = c.packs {
        set_packs(rbx, &packs);
    }
    if let Some(channels) = c.channels {
        for ch in channels {
            ensure_channel(rbx, &ch);
        }
    }
    if let Some(commands) = c.commands {
        rbx.server.commands = Some(commands);
        refill(rbx, rbx.server.commands_folder, Lazy::Commands);
        plugin_commands(rbx);
    }
    if let Some(r) = c.registries {
        rbx.server.registries = Some(r);
        refill(rbx, rbx.server.registries_folder, Lazy::Registries);
    }
    rbx.server.versions = c.versions;
}

/// Everything of the last connection's goes.
fn reset(rbx: &mut Rbx) {
    if let Some(r) = rbx.server.root.take() {
        rbx.destroy(r);
    }
    let loaded: HashSet<Id> = rbx
        .server
        .files
        .keys()
        .chain(rbx.server.readonly.iter())
        .copied()
        .collect();
    rbx.loaded_modules.retain(|(id, _)| !loaded.contains(id));
    rbx.server = Server::default();
}

/// A lazy folder emptied, to fill again when it is next looked in.
fn refill(rbx: &mut Rbx, folder: Option<Id>, lazy: Lazy) {
    let Some(f) = folder else { return };
    for c in rbx.children(f).to_vec() {
        forget(rbx, c);
        rbx.destroy(c);
    }
    rbx.server.lazy.insert(f, lazy);
}

fn forget(rbx: &mut Rbx, id: Id) {
    let mut all = vec![id];
    all.extend(rbx.descendants(id));
    for d in &all {
        let s = &mut rbx.server;
        s.lazy.remove(d);
        s.files.remove(d);
        s.readonly.remove(d);
    }
    rbx.loaded_modules.retain(|(i, _)| !all.contains(i));
}

fn set_packs(rbx: &mut Rbx, packs: &[ServerPack]) {
    let Some(holder) = rbx.server.resource_packs else {
        return;
    };
    for c in rbx.children(holder).to_vec() {
        forget(rbx, c);
        rbx.destroy(c);
    }
    rbx.server.packs.clear();
    for (i, p) in packs.iter().enumerate() {
        let name = if i == 0 {
            "Server Pack".to_owned()
        } else {
            format!("Server Pack {}", i + 1)
        };
        let source = if p.path.is_dir() {
            Source::Dir(p.path.clone())
        } else {
            Source::Zip(p.path.clone())
        };
        rbx.server.packs.push(Pack {
            name: name.clone(),
            source,
            index: None,
            zip: None,
        });
        let f = folder(rbx, &name, holder);
        attribute(rbx, f, "Id", Value::str(&p.id));
        attribute(rbx, f, "File", Value::str(&p.path.to_string_lossy()));
        rbx.server.lazy.insert(
            f,
            Lazy::PackDir {
                pack: i,
                dir: String::new(),
            },
        );
    }
}

// ---------------------------------------------------------------- filling

/// The folder's children made, if it is one that waits for that.
pub fn fill(rbx: &mut Rbx, id: Id) {
    let Some(lazy) = rbx.server.lazy.remove(&id) else {
        return;
    };
    match lazy {
        Lazy::PackDir { pack, dir } => fill_pack_dir(rbx, id, pack, &dir),
        Lazy::Commands => fill_commands(rbx, id),
        Lazy::Registries => fill_registries(rbx, id),
        Lazy::Tags => fill_tags(rbx, id),
    }
}

/// Everything under `id` made.
pub fn fill_all(rbx: &mut Rbx, id: Id) {
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        fill(rbx, n);
        stack.extend(rbx.children(n).iter().copied());
    }
}

fn fill_pack_dir(rbx: &mut Rbx, id: Id, pack: usize, dir: &str) {
    let Some(p) = rbx.server.packs.get_mut(pack) else {
        return;
    };
    let (dirs, files) = match p.index().get(dir) {
        Some(d) => (d.dirs.clone(), d.files.clone()),
        None => return,
    };
    let pack_name = p.name.clone();
    let copies = rbx
        .workspace_dir
        .join(".server-packs")
        .join(super::hstudio_dump::sanitize(&pack_name));
    for d in dirs {
        let f = folder(rbx, &d, id);
        let sub = if dir.is_empty() {
            d.clone()
        } else {
            format!("{dir}/{d}")
        };
        rbx.server.lazy.insert(f, Lazy::PackDir { pack, dir: sub });
    }
    for name in files {
        let path = if dir.is_empty() {
            name.clone()
        } else {
            format!("{dir}/{name}")
        };
        let kind = kind_of(&path);
        let bytes = rbx.server.packs[pack].read(&path).unwrap_or_default();
        // An image's copy in the workspace, for getcustomasset.
        let copy = (kind == Kind::Image && !bytes.is_empty())
            .then(|| {
                let file = copies.join(&path);
                let fresh = std::fs::metadata(&file).is_ok_and(|m| m.len() == bytes.len() as u64);
                let ok = fresh
                    || file.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok())
                        && std::fs::write(&file, &bytes).is_ok();
                ok.then(|| format!(".server-packs/{}/{path}", super::hstudio_dump::sanitize(&pack_name)))
            })
            .flatten();
        let source = file_source(&pack_name, &path, &bytes, copy.as_deref());
        let m = module(rbx, &name, &source, id);
        rbx.server.files.insert(m, File { pack, path, kind });
    }
}

/// A command's lines of use: `/tp <targets> <location>`, as the client's
/// own help would list them (at most `max`).
pub fn usage(name: &str, node: &serde_json::Value, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    fn go(node: &serde_json::Value, line: String, depth: usize, out: &mut Vec<String>, max: usize) {
        if out.len() >= max {
            return;
        }
        if node["executable"].as_bool() == Some(true) {
            out.push(line.clone());
        }
        if let Some(r) = node["redirect"].as_array() {
            let to: Vec<&str> = r.iter().filter_map(|p| p.as_str()).collect();
            out.push(format!("{line} -> /{}", to.join(" ")));
            return;
        }
        let Some(children) = node["children"].as_object() else {
            return;
        };
        if depth > 12 {
            return;
        }
        for (k, c) in children {
            let part = if c["type"] == "argument" {
                format!("<{k}>")
            } else {
                k.clone()
            };
            go(c, format!("{line} {part}"), depth + 1, out, max);
        }
    }
    go(node, format!("/{name}"), 0, &mut out, max);
    if out.len() >= max {
        out.push("...".into());
    }
    out
}

fn command_source(name: &str, node: &serde_json::Value) -> String {
    let lines = usage(name, node, 64);
    let mut node = node.clone();
    if let Some(o) = node.as_object_mut() {
        o.insert(
            "usage".into(),
            serde_json::Value::Array(lines.into_iter().map(serde_json::Value::String).collect()),
        );
    }
    format!(
        "-- /{name}: a command the server sent in its command tree. What it does runs on the server;\n\
         -- this is its shape: its arguments' parsers (brigadier's), whether it runs there, its usage.\nreturn {}\n",
        lua_of(&node, 0)
    )
}

fn fill_commands(rbx: &mut Rbx, id: Id) {
    let Some(tree) = rbx.server.commands.clone() else {
        return;
    };
    let Some(children) = tree["children"].as_object() else {
        return;
    };
    for (name, node) in children {
        let m = module(rbx, name, &command_source(name, node), id);
        rbx.server.readonly.insert(m);
    }
}

fn registry_source(name: &str, entries: &[(String, serde_json::Value)]) -> String {
    let map: serde_json::Map<String, serde_json::Value> = entries.iter().cloned().collect();
    format!(
        "-- {name}: the registry as the server sent it while the client configured ({} entries).\nreturn {}\n",
        entries.len(),
        lua_of(&serde_json::Value::Object(map), 0)
    )
}

fn fill_registries(rbx: &mut Rbx, id: Id) {
    let Some(r) = rbx.server.registries.clone() else { return };
    for (name, entries) in &r.entries {
        let m = module(rbx, name, &registry_source(name, entries), id);
        rbx.server.readonly.insert(m);
    }
    if !r.tags.is_empty() {
        let f = folder(rbx, "Tags", id);
        rbx.server.lazy.insert(f, Lazy::Tags);
    }
}

fn fill_tags(rbx: &mut Rbx, id: Id) {
    let Some(r) = rbx.server.registries.clone() else { return };
    for (registry, tags) in &r.tags {
        let map: serde_json::Map<String, serde_json::Value> = tags
            .iter()
            .map(|(t, ids)| (t.clone(), serde_json::Value::from(ids.clone())))
            .collect();
        let src = format!(
            "-- {registry}'s tags as the server sent them: each tag's entries.\nreturn {}\n",
            lua_of(&serde_json::Value::Object(map), 0)
        );
        let m = module(rbx, registry, &src, id);
        rbx.server.readonly.insert(m);
    }
}

// ---------------------------------------------------------------- plugins

/// The channels that are the protocol's own, not a plugin's.
fn protocol_channel(ch: &str) -> bool {
    matches!(
        ch,
        "minecraft:register" | "minecraft:unregister" | "minecraft:brand" | "REGISTER" | "UNREGISTER" | "MC|Brand"
    )
}

/// The plugin a channel is of, and its remote's name.
pub fn channel_plugin(ch: &str) -> (String, String) {
    match ch.split_once(':') {
        Some((ns, path)) if !ns.is_empty() && !path.is_empty() => (ns.to_ascii_lowercase(), path.to_owned()),
        // Channels of before 1.13 have no namespace: BungeeCord's, WorldEdit's CUI.
        _ => {
            let ns = match ch {
                "BungeeCord" => "bungeecord".to_owned(),
                "WECUI" => "worldedit".to_owned(),
                other => other.trim_start_matches("MC|").to_ascii_lowercase(),
            };
            (ns, ch.to_owned())
        }
    }
}

/// A plugin's folder (made the first time it is named).
fn plugin_folder(rbx: &mut Rbx, ns: &str) -> Option<Id> {
    if let Some((f, _)) = rbx.server.plugin_folders.get(ns) {
        return Some(*f);
    }
    let holder = rbx.server.plugins?;
    let f = folder(rbx, ns, holder);
    rbx.server.plugin_folders.insert(ns.to_owned(), (f, None));
    Some(f)
}

/// A channel's RemoteEvent, made the first time the channel is named.
pub fn ensure_channel(rbx: &mut Rbx, ch: &str) -> Option<Id> {
    if let Some(id) = rbx.server.by_channel.get(ch) {
        return Some(*id);
    }
    if ch.is_empty() || protocol_channel(ch) || rbx.server.root.is_none() {
        return None;
    }
    let (ns, name) = channel_plugin(ch);
    let f = plugin_folder(rbx, &ns)?;
    let r = rbx.make("RemoteEvent", None);
    rbx.inst_mut(r).name = name;
    attribute(rbx, r, "Channel", Value::str(ch));
    let _ = rbx.set_parent(r, Some(f));
    rbx.inst_mut(r).locked = true;
    rbx.server.channels.insert(r, ch.to_owned());
    rbx.server.by_channel.insert(ch.to_owned(), r);
    let list: Vec<serde_json::Value> = rbx
        .server
        .by_channel
        .keys()
        .cloned()
        .map(serde_json::Value::String)
        .collect();
    if let Some(m) = rbx.server.channels_module {
        let src = format!(
            "-- The plugin channels the server registered (minecraft:register) or sent on.\nreturn {}\n",
            lua_of(&serde_json::Value::Array(list), 0)
        );
        rbx.raw_set(m, "Source", Value::str(&src));
        rbx.loaded_modules.retain(|(i, _)| *i != m);
    }
    Some(r)
}

/// Each plugin's commands: the ones the server lists under its namespace
/// (`worldedit://wand`, `luckperms:lp`), by their names without it.
fn plugin_commands(rbx: &mut Rbx) {
    let Some(tree) = rbx.server.commands.clone() else {
        return;
    };
    let Some(children) = tree["children"].as_object() else {
        return;
    };
    let mut by_ns: BTreeMap<String, serde_json::Map<String, serde_json::Value>> = BTreeMap::new();
    for (name, node) in children {
        let Some((ns, cmd)) = name.split_once(':') else {
            continue;
        };
        if ns.is_empty() || cmd.is_empty() {
            continue;
        }
        let node = children.get(cmd).unwrap_or(node);
        let lines = usage(cmd, node, 16);
        let mut node = node.clone();
        if let Some(o) = node.as_object_mut() {
            o.insert(
                "usage".into(),
                serde_json::Value::Array(lines.into_iter().map(serde_json::Value::String).collect()),
            );
        }
        by_ns
            .entry(ns.to_ascii_lowercase())
            .or_default()
            .insert(cmd.to_owned(), node);
    }
    for (ns, cmds) in by_ns {
        let Some(f) = plugin_folder(rbx, &ns) else { continue };
        let src = format!(
            "-- The commands the server lists as {ns}'s ({} of them), by name. Their code runs on the server.\nreturn {}\n",
            cmds.len(),
            lua_of(&serde_json::Value::Object(cmds), 0)
        );
        let existing = rbx.server.plugin_folders.get(&ns).and_then(|(_, m)| *m);
        match existing.filter(|m| rbx.try_inst(*m).is_some_and(|i| !i.destroyed)) {
            Some(m) => {
                rbx.raw_set(m, "Source", Value::str(&src));
                rbx.loaded_modules.retain(|(i, _)| *i != m);
            }
            None => {
                let m = module(rbx, "Commands", &src, f);
                rbx.server.readonly.insert(m);
                if let Some(e) = rbx.server.plugin_folders.get_mut(&ns) {
                    e.1 = Some(m);
                }
            }
        }
    }
}

/// A custom payload from the server: its channel's RemoteEvent fires
/// OnClientEvent with the bytes (a string).
pub fn payload(rbx: &mut Rbx, channel: &str, data: &[u8]) {
    if !rbx.active || rbx.server.root.is_none() {
        return;
    }
    if let Some(r) = ensure_channel(rbx, channel) {
        rbx.fire(r, "OnClientEvent", vec![Value::bytes(data)]);
    }
}

/// `remote:FireServer(data)` on a plugin's channel: a custom payload on it.
pub fn fire(rbx: &mut Rbx, game: &mut dyn GameApi, now: f64, lua: &Lua, id: Id, args: &[Value]) -> Res {
    let Some(channel) = rbx.server.channel_of(id).map(str::to_owned) else {
        return Ok(Vec::new());
    };
    let data = match args.first() {
        Some(Value::Str(s)) => s.to_vec(),
        Some(v) => match buffer_bytes(v) {
            Some(b) => b,
            None => {
                return Err(lua.rt_error(format!(
                    "FireServer: {channel} takes the payload as a string or a buffer, got {}",
                    lua.type_of(v)
                )));
            }
        },
        None => Vec::new(),
    };
    if data.len() > MAX_PAYLOAD {
        return Err(lua.rt_error(format!(
            "FireServer: a payload is at most {MAX_PAYLOAD} bytes ({channel}'s is {})",
            data.len()
        )));
    }
    let r = &mut rbx.remotes;
    if r.frame_at != now {
        r.frame_at = now;
        r.sent = 0;
    }
    if r.sent >= super::hstudio_rbx_remote::MAX_PER_FRAME {
        return Err(lua.rt_error(format!(
            "FireServer: too many packets this frame (at most {})",
            super::hstudio_rbx_remote::MAX_PER_FRAME
        )));
    }
    r.sent += 1;
    let id = game
        .packet_names()
        .0
        .iter()
        .position(|n| *n == "custom_payload")
        .ok_or_else(|| lua.rt_error("FireServer: not connected to a server"))?;
    let p = Packet {
        id: id as i32,
        name: "custom_payload".into(),
        fields: vec![
            ("channel".into(), Field::text(&channel)),
            ("data".into(), Field::Str(data)),
        ],
        mapped: true,
        raw: None,
    };
    game.send_packet(&p)
        .map_err(|e| lua.rt_error(format!("FireServer: {channel}: {e}")))?;
    Ok(Vec::new())
}

// ---------------------------------------------------------------- edits

/// `module.Source = text` on one of the tree's ModuleScripts. A pack's
/// file takes it when the Lua runs and gives that file's kind of data (a
/// table for JSON, a string for a shader or text): the file goes into the
/// override pack. Anything else is an error, and the Source stays.
pub fn set_source(rbx: &mut Rbx, lua: &mut Lua, id: Id, text: &Value) -> Result<(), Throw> {
    let name = rbx.full_name(id);
    if rbx.server.readonly.contains(&id) {
        return Err(lua.rt_error(format!(
            "{name} is what the server sent: it can't be changed here (only ReplicatedStorage.Server.ResourcePacks' files can)"
        )));
    }
    let Some(f) = rbx.server.files.get(&id) else {
        return Ok(());
    };
    let (pack, path, kind) = (f.pack, f.path.clone(), f.kind);
    let src = text.as_bytes().map(<[u8]>::to_vec).unwrap_or_default();
    match edited(rbx, lua, pack, &path, kind, &src) {
        Ok(bytes) => {
            rbx.raw_set(id, "Source", Value::bytes(&src));
            rbx.loaded_modules.retain(|(i, _)| *i != id);
            rbx.server.edits.retain(|(p, _)| *p != path);
            rbx.server.edits.push((path, bytes));
            Ok(())
        }
        Err(Ok(why)) => Err(lua.rt_error(format!("{name}: {why}; the file is as it was"))),
        Err(Err(fatal)) => Err(fatal),
    }
}

/// The file an edit's Lua gives, or why there is none (Err(Err) when the
/// Lua ran out of time).
fn edited(
    rbx: &mut Rbx,
    lua: &mut Lua,
    pack: usize,
    path: &str,
    kind: Kind,
    src: &[u8],
) -> Result<Vec<u8>, Result<String, Throw>> {
    let no = |why: String| Err(Ok(why));
    if path == "pack.mcmeta" {
        return no("pack.mcmeta describes the server's pack; the override pack has its own".into());
    }
    if matches!(kind, Kind::Image | Kind::Binary) {
        return no(format!("{path} isn't data Lua gives (its pixels or sound aren't Lua)"));
    }
    // It runs as require runs it, in the caller's globals, with json_null
    // (what stands for JSON's null) one of its own.
    let Some(caller) = super::hstudio_exec::caller_env(lua) else {
        return no("no environment to run it in".into());
    };
    let null = Value::table(super::hstudio_lua::Table::new());
    let mut meta = super::hstudio_lua::Table::new();
    meta.set_str("__index", Value::Table(caller));
    let mut env = super::hstudio_lua::Table::new();
    env.set_str("json_null", null.clone());
    if let Value::Table(m) = Value::table(meta) {
        env.meta = Some(m);
    }
    let Value::Table(env) = Value::table(env) else {
        return no("no environment to run it in".into());
    };
    let f = match lua.load_bytes(src, "=Source", &env) {
        Ok(f) => f,
        Err(e) => return no(e),
    };
    let mut host = super::hstudio_lua::NoHost::default();
    let got = match lua.call(&mut host, &f, Vec::new()) {
        Ok(r) => r.into_iter().next().unwrap_or_default(),
        Err(Throw::Fatal(m)) => return Err(Err(Throw::Fatal(m))),
        Err(e) => return no(format!("it fails as it runs: {}", e.message())),
    };
    let original = rbx
        .server
        .packs
        .get_mut(pack)
        .and_then(|p| p.read(path))
        .unwrap_or_default();
    if kind != Kind::Json {
        return match &got {
            Value::Str(s) => Ok(s.to_vec()),
            other => no(format!("it gives a {}: {path} is text (a string)", other.type_name())),
        };
    }
    let shape = parse_json(&original);
    let json = match &got {
        Value::Str(s) => match parse_json(s) {
            Some(j) => j,
            None => return no("it gives text that isn't JSON".into()),
        },
        Value::Table(_) => match json_of(&got, shape.as_ref(), &null, "", 0) {
            Ok(j) => j,
            Err(e) => return no(e),
        },
        other => return no(format!("it gives a {}: {path} is a table (JSON)", other.type_name())),
    };
    if !json.is_object() {
        return no(format!("{path} is a JSON object: a table with names for keys"));
    }
    if let Err(e) = check_json(path, &json) {
        return no(e);
    }
    let mut out = serde_json::to_vec_pretty(&json).map_err(|e| Ok(e.to_string()))?;
    out.push(b'\n');
    Ok(out)
}

/// What Minecraft needs of a file of the pack, past being JSON.
fn check_json(path: &str, v: &serde_json::Value) -> Result<(), String> {
    let parts: Vec<&str> = path.split('/').collect();
    let dir = parts.get(2).copied().unwrap_or("");
    let bad = |what: &str| Err(format!("{path}: {what}"));
    if parts.first() != Some(&"assets") {
        return Ok(());
    }
    match dir {
        "lang"
            if !v
                .as_object()
                .is_some_and(|o| o.values().all(serde_json::Value::is_string)) =>
        {
            bad("a language's entries are text (key = \"text\")")
        }
        "blockstates" if v.get("variants").is_none() && v.get("multipart").is_none() => {
            bad("a blockstate has variants or multipart")
        }
        "font" if !v.get("providers").is_some_and(serde_json::Value::is_array) => bad("a font has a providers list"),
        "atlases" if !v.get("sources").is_some_and(serde_json::Value::is_array) => bad("an atlas has a sources list"),
        "models" if v.get("elements").is_some_and(|e| !e.is_array()) => bad("a model's elements are a list"),
        _ => Ok(()),
    }
}

/// The override pack's folder: `server-overrides/hstudio-overrides` in the
/// studio's folder.
pub fn override_dir(rbx: &Rbx) -> PathBuf {
    rbx.workspace_dir
        .parent()
        .unwrap_or(&rbx.workspace_dir)
        .join(OVERRIDES)
        .join(OVERRIDE_PACK)
}

/// The edits written into the override pack (emptied the first time this
/// session), and the game's resources reloaded with it.
fn write_edits(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if rbx.server.edits.is_empty() {
        return;
    }
    let dir = override_dir(rbx);
    if !rbx.server.started_override {
        rbx.server.started_override = true;
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut written = Vec::new();
    for (path, bytes) in std::mem::take(&mut rbx.server.edits) {
        let file = dir.join(&path);
        let ok =
            file.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&file, &bytes).is_ok();
        if ok {
            written.push(path);
        } else {
            rbx.server
                .notes
                .push((true, format!("{path} couldn't be written into {}", dir.display())));
        }
    }
    if written.is_empty() {
        return;
    }
    match game.resource_override(Some(&dir)) {
        Ok(()) => rbx.server.notes.push((
            false,
            format!(
                "{} in the override pack: the resources reload with it",
                written.join(", ")
            ),
        )),
        Err(e) => rbx.server.notes.push((
            true,
            format!("{}: the resources didn't reload: {e}", written.join(", ")),
        )),
    }
}

fn buffer_bytes(v: &Value) -> Option<Vec<u8>> {
    match v {
        Value::Userdata(u) => u
            .data
            .borrow()
            .downcast_ref::<super::hstudio_lua::LuaBuffer>()
            .map(|b| b.0.clone()),
        _ => None,
    }
}
