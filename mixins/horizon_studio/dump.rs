// saveinstance as a world downloader (hstudio_dump): what the studio itself
// decides about a dump, with no game in it. The world's folder in the
// game's saves (its name, kept to one plain folder there), the marker that
// says a folder is a dump saveinstance may add to, and the vanilla structure
// file (`.nbt`, the structure block's format) that saveinstance(part)
// writes of the blocks in a part's or a model's box. The world itself (its
// chunks, entities, level.dat, player, maps, the server's registries) is
// the game's to write: hstudio_bridge does it with Horizon's save code.

use super::hstudio::ClientInfo;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The file in a world saveinstance made: it adds to a world only with it.
pub const MARKER: &str = "horizon-studio-dump.json";

/// The most blocks a structure file holds (128 x 128 x 128).
pub const MAX_STRUCTURE: i64 = 128 * 128 * 128;

/// Minecraft 26.3's data version, for structures a game without its own
/// says nothing of (the tests' fake game).
pub const DATA_VERSION: i32 = 5023;

/// A block in a box, as a structure file keeps it: where it is in the box,
/// its block, its block state's properties and its block entity (the
/// payload of an NBT compound, its entries and their end, without its x, y
/// and z), if it has one the client knows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionBlock {
    pub pos: [i32; 3],
    pub name: String,
    pub properties: Vec<(String, String)>,
    pub nbt: Option<Vec<u8>>,
}

/// The blocks of a box, and the data version they are of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub data_version: i32,
    pub blocks: Vec<RegionBlock>,
}

/// Today's date (UTC) as `2026-11-02`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date_of(secs as i64 / 86_400)
}

/// The date `days` after 1970-01-01 (Howard Hinnant's civil_from_days).
pub fn date_of(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A name as one plain folder: letters, digits, spaces and `-_.()+,` kept,
/// anything else (a slash, a colon) an underscore; no leading dots or
/// trailing dots and spaces (Windows drops them), not Windows' device
/// names, at most 80 characters. Empty is "world".
pub fn sanitize(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '+' | ',') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    let trimmed = out
        .trim_start_matches(['.', ' '])
        .trim_end_matches(['.', ' '])
        .to_string();
    out = trimmed;
    if out.is_empty() {
        out = "world".into();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if device {
        out.insert(0, '_');
    }
    out
}

/// The dump's folder name: the one asked for (a Roblox file's extension,
/// and Dex's `.txt`, taken off), else the server's address (a world of the
/// player's own: its name) and today's date.
pub fn folder_name(asked: Option<&str>, client: &ClientInfo, date: &str) -> String {
    if let Some(asked) = asked.map(str::trim).filter(|a| !a.is_empty()) {
        let mut name = asked.to_string();
        for ext in [".txt", ".rbxlx", ".rbxmx", ".rbxl", ".rbxm"] {
            if name.len() > ext.len() && name.to_ascii_lowercase().ends_with(ext) {
                name.truncate(name.len() - ext.len());
            }
        }
        return sanitize(&name);
    }
    let source = match &client.server {
        Some(s) if !s.address.is_empty() => s.address.clone(),
        Some(s) if !s.name.is_empty() => s.name.clone(),
        _ => "world".into(),
    };
    sanitize(&format!("{} {date}", source.replace(':', " ")))
}

/// The world folder `folder` in `saves`, ready for a dump: made with its
/// marker, or one saveinstance made before (its marker there), which the
/// dump adds to. Any other folder (a world of the player's own) is refused
/// and left as it is.
pub fn prepare(saves: &Path, folder: &str, source: &str) -> Result<PathBuf, String> {
    let name = sanitize(folder);
    let world = saves.join(&name);
    // One folder, right in the saves: nothing else is ever written.
    if world.parent() != Some(saves) || world.file_name().and_then(|n| n.to_str()) != Some(name.as_str()) {
        return Err(format!("'{folder}' is not a folder name"));
    }
    let marker = world.join(MARKER);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut first = now;
    if world.exists() {
        if !world.is_dir() {
            return Err(format!("'{name}' in the saves folder is a file, not a world"));
        }
        if !marker.is_file() {
            return Err(format!(
                "the saves folder already has a world '{name}' that saveinstance didn't make: it is left as it is \
                 (give saveinstance another FilePath)"
            ));
        }
        if let Some(t) = std::fs::read(&marker)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v["first_saved"].as_u64())
        {
            first = t;
        }
    }
    std::fs::create_dir_all(&world).map_err(|e| format!("can't make the world's folder: {e}"))?;
    let text = serde_json::json!({
        "made_by": "Horizon Studio saveinstance",
        "source": source,
        "first_saved": first,
        "last_saved": now,
    });
    std::fs::write(&marker, serde_json::to_string_pretty(&text).unwrap_or_default())
        .map_err(|e| format!("can't write {MARKER}: {e}"))?;
    Ok(world)
}

/// The blocks a box of world coordinates covers: the least and the most
/// corner (inclusive) of the blocks it fills more than a hair of.
pub fn block_box(lo: [f64; 3], hi: [f64; 3]) -> ([i32; 3], [i32; 3]) {
    const HAIR: f64 = 1e-4;
    let a = lo.map(|v| (v + HAIR).floor() as i32);
    let b = hi.map(|v| (v - HAIR).ceil() as i32 - 1);
    ([a[0], a[1], a[2]], [b[0].max(a[0]), b[1].max(a[1]), b[2].max(a[2])])
}

// ---------------------------------------------------------------- NBT

struct Nbt(Vec<u8>);

impl Nbt {
    fn name(&mut self, s: &str) {
        let b = java_utf8(s);
        self.0.extend_from_slice(&(b.len() as u16).to_be_bytes());
        self.0.extend_from_slice(&b);
    }
    fn head(&mut self, kind: u8, name: &str) {
        self.0.push(kind);
        self.name(name);
    }
    fn int(&mut self, name: &str, v: i32) {
        self.head(3, name);
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn string(&mut self, name: &str, v: &str) {
        self.head(8, name);
        self.name(v);
    }
    fn list(&mut self, name: &str, kind: u8, len: usize) {
        self.head(9, name);
        self.0.push(if len == 0 { 0 } else { kind });
        self.0.extend_from_slice(&(len as i32).to_be_bytes());
    }
    fn ints(&mut self, name: &str, v: [i32; 3]) {
        self.list(name, 3, 3);
        for i in v {
            self.0.extend_from_slice(&i.to_be_bytes());
        }
    }
    fn end(&mut self) {
        self.0.push(0);
    }
}

/// Java's modified UTF-8 (`DataOutput.writeUTF`): NUL and the characters
/// past the Basic Multilingual Plane as Java's UTF-16 halves.
fn java_utf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        match unit {
            1..=0x7F => out.push(unit as u8),
            0 | 0x80..=0x7FF => {
                out.push(0xC0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    out
}

/// `StructureTemplate.save`: a structure of `size` with these blocks
/// (structure voids left out, as the structure block leaves them), as NBT
/// (uncompressed: `.nbt` files are this, gzipped).
pub fn structure_nbt(size: [i32; 3], region: &Region) -> Vec<u8> {
    let mut palette: Vec<(&str, &[(String, String)])> = Vec::new();
    let mut states = Vec::with_capacity(region.blocks.len());
    let blocks: Vec<&RegionBlock> = region
        .blocks
        .iter()
        .filter(|b| b.name != "minecraft:structure_void")
        .collect();
    for b in &blocks {
        let key = (b.name.as_str(), b.properties.as_slice());
        let i = match palette.iter().position(|p| *p == key) {
            Some(i) => i,
            None => {
                palette.push(key);
                palette.len() - 1
            }
        };
        states.push(i as i32);
    }
    let mut w = Nbt(Vec::with_capacity(blocks.len() * 24 + 256));
    w.head(10, "");
    w.int("DataVersion", region.data_version);
    w.ints("size", size);
    w.list("palette", 10, palette.len());
    for (name, props) in &palette {
        w.string("Name", name);
        if !props.is_empty() {
            w.head(10, "Properties");
            for (k, v) in *props {
                w.string(k, v);
            }
            w.end();
        }
        w.end();
    }
    w.list("blocks", 10, blocks.len());
    for (b, state) in blocks.iter().zip(&states) {
        w.ints("pos", b.pos);
        w.int("state", *state);
        if let Some(nbt) = &b.nbt {
            w.head(10, "nbt");
            w.0.extend_from_slice(nbt);
        }
        w.end();
    }
    w.list("entities", 10, 0);
    w.end();
    w.0
}

/// Bytes gzipped, as `NbtIo.writeCompressed` writes a structure.
pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = z.write_all(bytes);
    z.finish().unwrap_or_default()
}
