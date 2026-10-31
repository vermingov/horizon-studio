// Roblox model and place files, binary (.rbxm, .rbxl) and XML (.rbxmx,
// .rbxlx), read into a plain tree: items with a class, the file's referent,
// typed properties and children, for the Roblox layer to make Instances from
// (game:GetObjects loads a model this way). The formats are as rbx-dom
// documents them (docs/binary.md and docs/xml.md in rojo-rbx/rbx-dom), and
// where those are unclear, as its readers do. Files come from the internet,
// so nothing here trusts them: every count is checked against the bytes
// left, decompression and the tree have size limits (`Limits`), and a bad
// file is a plain-English error, never a panic. Binary chunks may be LZ4 or
// ZSTD compressed; both decoders are pure Rust.

use super::hstudio_crypt::{base64_decode, lz4_decompress};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

// ---------------------------------------------------------------- the tree

/// A model or place file's items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RbxFile {
    pub items: Vec<Item>,
    /// The items without a parent, in the file's order.
    pub roots: Vec<usize>,
    /// The file's metadata (META in binary, `<Meta>` in XML).
    pub meta: Vec<(String, String)>,
    /// What was skipped and why: properties of unknown types, unknown chunks
    /// and the like (at most MAX_NOTES, and one more saying there were more).
    pub notes: Vec<String>,
}

/// One Instance in a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub class: String,
    /// The file's name for it: a number in binary files, the `referent`
    /// attribute in XML ones. Ref properties are resolved to item indices.
    pub referent: String,
    /// Marked as a service (binary places mark them).
    pub service: bool,
    pub properties: Vec<(String, PropValue)>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

impl Item {
    /// A property's value (the first, if the file gives it twice).
    pub fn get(&self, name: &str) -> Option<&PropValue> {
        self.properties.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// Its Name property as text ("" without one).
    pub fn name(&self) -> Cow<'_, str> {
        match self.get("Name") {
            Some(PropValue::String(s)) => String::from_utf8_lossy(s),
            _ => Cow::Borrowed(""),
        }
    }
}

/// A property's value. Without Roblox's class reflection a file can't say
/// which of a few types that share a format it means, so those come as one:
/// String is also BinaryString, ProtectedString, ContentId (the old Content),
/// Tags and attributes; SharedString is also NetAssetRef. BrickColor
/// properties are Int32 in XML files.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    /// Bytes, usually UTF-8 text.
    String(Vec<u8>),
    Bool(bool),
    Int32(i32),
    Float32(f32),
    Float64(f64),
    UDim(UDim),
    UDim2(UDim2),
    Ray(Ray),
    /// Bits 0 to 5: Right, Top, Back, Left, Bottom and Front (NormalId's order).
    Faces(u8),
    /// Bits 0 to 2: X, Y and Z.
    Axes(u8),
    /// A BrickColor's number.
    BrickColor(u32),
    Color3(Color3),
    Vector2(Vector2),
    Vector3(Vector3),
    CFrame(CFrame),
    /// An enum item's value.
    Enum(u32),
    /// Another item, by index; None for an empty Ref or one to an Instance
    /// outside the file.
    Ref(Option<usize>),
    Vector2int16([i16; 2]),
    Vector3int16([i16; 3]),
    NumberSequence(Vec<NumberKeypoint>),
    ColorSequence(Vec<ColorKeypoint>),
    NumberRange(NumberRange),
    Rect(Rect),
    /// None for the material's own physics.
    PhysicalProperties(Option<CustomPhysics>),
    /// Red, green and blue, 0 to 255.
    Color3uint8([u8; 3]),
    Int64(i64),
    /// A string the file keeps once for all the properties that use it.
    SharedString(Arc<[u8]>),
    /// Compiled Luau, which Roblox loads only when signed: never run it.
    Bytecode(Vec<u8>),
    OptionalCFrame(Option<CFrame>),
    UniqueId(UniqueId),
    Font(Font),
    SecurityCapabilities(u64),
    /// The newer Content type (old content IDs are String).
    Content(Content),
}

impl PropValue {
    /// The type's name, as the variant has it ("Vector3").
    pub fn kind(&self) -> &'static str {
        use PropValue as V;
        match self {
            V::String(_) => "String",
            V::Bool(_) => "Bool",
            V::Int32(_) => "Int32",
            V::Float32(_) => "Float32",
            V::Float64(_) => "Float64",
            V::UDim(_) => "UDim",
            V::UDim2(_) => "UDim2",
            V::Ray(_) => "Ray",
            V::Faces(_) => "Faces",
            V::Axes(_) => "Axes",
            V::BrickColor(_) => "BrickColor",
            V::Color3(_) => "Color3",
            V::Vector2(_) => "Vector2",
            V::Vector3(_) => "Vector3",
            V::CFrame(_) => "CFrame",
            V::Enum(_) => "Enum",
            V::Ref(_) => "Ref",
            V::Vector2int16(_) => "Vector2int16",
            V::Vector3int16(_) => "Vector3int16",
            V::NumberSequence(_) => "NumberSequence",
            V::ColorSequence(_) => "ColorSequence",
            V::NumberRange(_) => "NumberRange",
            V::Rect(_) => "Rect",
            V::PhysicalProperties(_) => "PhysicalProperties",
            V::Color3uint8(_) => "Color3uint8",
            V::Int64(_) => "Int64",
            V::SharedString(_) => "SharedString",
            V::Bytecode(_) => "Bytecode",
            V::OptionalCFrame(_) => "OptionalCFrame",
            V::UniqueId(_) => "UniqueId",
            V::Font(_) => "Font",
            V::SecurityCapabilities(_) => "SecurityCapabilities",
            V::Content(_) => "Content",
        }
    }

    /// A String or SharedString as text, if it is UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropValue::String(s) => std::str::from_utf8(s).ok(),
            PropValue::SharedString(s) => std::str::from_utf8(s).ok(),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UDim {
    pub scale: f32,
    pub offset: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UDim2 {
    pub x: UDim,
    pub y: UDim,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ray {
    pub origin: Vector3,
    pub direction: Vector3,
}

/// Red, green and blue, usually 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color3 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// A position and a rotation matrix, row by row: [[R00, R01, R02], [R10,
/// R11, R12], [R20, R21, R22]], so its columns are the right, up and back
/// vectors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CFrame {
    pub position: Vector3,
    pub rotation: [[f32; 3]; 3],
}

impl CFrame {
    pub const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
}

impl Default for CFrame {
    fn default() -> CFrame {
        CFrame {
            position: Vector3::default(),
            rotation: CFrame::IDENTITY,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vector2,
    pub max: Vector2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NumberRange {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NumberKeypoint {
    pub time: f32,
    pub value: f32,
    pub envelope: f32,
}

/// A ColorSequence keypoint; files store an envelope that Roblox doesn't use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorKeypoint {
    pub time: f32,
    pub color: Color3,
    pub envelope: f32,
}

/// Custom physical properties (AcousticAbsorption is 1 in older files).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CustomPhysics {
    pub density: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub friction_weight: f32,
    pub elasticity_weight: f32,
    pub acoustic_absorption: f32,
}

/// A UniqueId, with `random` as XML files write it (binary files rotate it a
/// bit to the left, which reading undoes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UniqueId {
    pub index: u32,
    pub time: u32,
    pub random: i64,
}

/// A font: its family's content URI, its weight (100 to 900), its style (0
/// normal, 1 italic) and the cached face's URI ("" when there is none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    pub family: String,
    pub weight: u16,
    pub style: u8,
    pub cached_face_id: String,
}

impl Default for Font {
    /// Roblox's default, Source Sans Pro regular.
    fn default() -> Font {
        Font {
            family: "rbxasset://fonts/families/SourceSansPro.json".into(),
            weight: 400,
            style: 0,
            cached_face_id: String::new(),
        }
    }
}

/// A Content value: nothing, a URI, or an object in the file (by item index;
/// None when it is outside the file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Content {
    #[default]
    None,
    Uri(String),
    Object(Option<usize>),
}

// ---------------------------------------------------------------- reading

/// How much a file may decode to, so a forged one can't take all the memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Bytes of chunk data after decompression (binary files), or of text
    /// (XML files).
    pub data: usize,
    /// The tree's size in memory, roughly.
    pub tree: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            data: 128 << 20,
            tree: 512 << 20,
        }
    }
}

/// At most this many notes are kept.
pub const MAX_NOTES: usize = 64;

/// A binary or XML model or place file, with the default limits.
pub fn read(bytes: &[u8]) -> Result<RbxFile, String> {
    read_with(bytes, Limits::default())
}

/// A binary or XML model or place file: binary when it starts `<roblox!`.
pub fn read_with(bytes: &[u8], limits: Limits) -> Result<RbxFile, String> {
    if bytes.starts_with(MAGIC) {
        read_binary(bytes, limits)
    } else {
        read_xml(bytes, limits)
    }
}

/// What is left of a limit.
struct Budget {
    left: usize,
    limit: usize,
    what: &'static str,
}

impl Budget {
    fn new(limit: usize, what: &'static str) -> Budget {
        Budget {
            left: limit,
            limit,
            what,
        }
    }

    fn spend(&mut self, n: usize) -> Result<(), String> {
        self.left = self.left.checked_sub(n).ok_or_else(|| {
            let limit = match self.limit {
                l if l.is_multiple_of(1 << 20) => format!("{} MiB", l >> 20),
                l => format!("{l} bytes"),
            };
            format!("the file is too big: its {} more than {limit}", self.what)
        })?;
        Ok(())
    }
}

const TREE: &str = "tree would take";
/// What a property costs besides its value's own heap (which is no bigger
/// than the file's bytes it comes from).
const PROP_COST: usize = size_of::<(String, PropValue)>();
/// What an item costs besides its strings: itself, and its place in a map.
const ITEM_COST: usize = size_of::<Item>() + 32;

fn note(notes: &mut Vec<String>, text: impl FnOnce() -> String) {
    match notes.len() {
        n if n < MAX_NOTES => notes.push(text()),
        MAX_NOTES => notes.push("(more notes left out)".into()),
        _ => {}
    }
}

/// A name from the file, for a message: cut at 60 characters.
fn clip(s: &str) -> Cow<'_, str> {
    match s.char_indices().nth(60) {
        Some((i, _)) => Cow::Owned(format!("{}...", &s[..i])),
        None => Cow::Borrowed(s),
    }
}

/// A value from the file, for a message: quoted, and cut at 40 characters.
fn shown(s: &str) -> String {
    match s.char_indices().nth(40) {
        Some((i, _)) => format!("{:?}...", &s[..i]),
        None => format!("{s:?}"),
    }
}

/// Checks that every item is reached once from the roots (no loops).
fn check_tree(file: &RbxFile) -> Result<(), String> {
    let mut seen = vec![false; file.items.len()];
    let mut stack = file.roots.clone();
    let mut count = 0;
    while let Some(i) = stack.pop() {
        let Some(s) = seen.get_mut(i) else {
            return Err(format!("the tree has an item {i} that doesn't exist"));
        };
        if std::mem::replace(s, true) {
            return Err("an instance is in the tree twice".into());
        }
        count += 1;
        stack.extend_from_slice(&file.items[i].children);
    }
    if count < file.items.len() {
        return Err("the file parents instances in a loop".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- binary

const MAGIC: &[u8; 8] = b"<roblox!";
const SIGNATURE: [u8; 6] = [0x89, 0xff, 0x0d, 0x0a, 0x1a, 0x0a];
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// The rotation matrix a CFrame's rotation ID stands for, if it is one of
/// the 24. ID - 1 is 6a + b, where a and b give the directions of the first
/// two columns (0 to 5: +X, +Y, +Z, -X, -Y, -Z) and the third is their cross
/// product.
pub fn rotation_from_id(id: u8) -> Option<[[f32; 3]; 3]> {
    if id == 0 || id > 36 {
        return None;
    }
    let (a, b) = ((id - 1) / 6, (id - 1) % 6);
    if a % 3 == b % 3 {
        return None;
    }
    let axis = |n: u8| {
        let mut v = [0.0f32; 3];
        v[usize::from(n % 3)] = if n < 3 { 1.0 } else { -1.0 };
        v
    };
    let (x, y) = (axis(a), axis(b));
    // Adding 0 turns the cross product's -0s into 0s.
    let z = [
        x[1] * y[2] - x[2] * y[1] + 0.0,
        x[2] * y[0] - x[0] * y[2] + 0.0,
        x[0] * y[1] - x[1] * y[0] + 0.0,
    ];
    Some([[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]])
}

/// The name of a binary property type, if it is one this reader knows.
fn type_name(ty: u8) -> Option<&'static str> {
    Some(match ty {
        0x01 => "String",
        0x02 => "Bool",
        0x03 => "Int32",
        0x04 => "Float32",
        0x05 => "Float64",
        0x06 => "UDim",
        0x07 => "UDim2",
        0x08 => "Ray",
        0x09 => "Faces",
        0x0a => "Axes",
        0x0b => "BrickColor",
        0x0c => "Color3",
        0x0d => "Vector2",
        0x0e => "Vector3",
        0x10 => "CFrame",
        0x12 => "Enum",
        0x13 => "Ref",
        0x14 => "Vector3int16",
        0x15 => "NumberSequence",
        0x16 => "ColorSequence",
        0x17 => "NumberRange",
        0x18 => "Rect",
        0x19 => "PhysicalProperties",
        0x1a => "Color3uint8",
        0x1b => "Int64",
        0x1c => "SharedString",
        0x1d => "Bytecode",
        0x1e => "OptionalCFrame",
        0x1f => "UniqueId",
        0x20 => "Font",
        0x21 => "SecurityCapabilities",
        0x22 => "Content",
        _ => return None,
    })
}

/// Bytes read from the front, each read checked against what is left.
struct Cursor<'a> {
    b: &'a [u8],
}

fn cut() -> String {
    "the data ends early".into()
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// A transformed integer back: 0, -1, 1, -2... are stored as 0, 1, 2, 3...
fn untransform32(x: u32) -> i32 {
    ((x >> 1) as i32) ^ -((x & 1) as i32)
}

fn untransform64(x: u64) -> i64 {
    ((x >> 1) as i64) ^ -((x & 1) as i64)
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Cursor<'a> {
        Cursor { b }
    }

    fn left(&self) -> usize {
        self.b.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if n > self.b.len() {
            return Err(cut());
        }
        let (a, rest) = self.b.split_at(n);
        self.b = rest;
        Ok(a)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        self.array().map(u16::from_le_bytes)
    }

    fn i16(&mut self) -> Result<i16, String> {
        self.array().map(i16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.array().map(f32::from_le_bytes)
    }

    fn f64(&mut self) -> Result<f64, String> {
        self.array().map(f64::from_le_bytes)
    }

    fn vector3(&mut self) -> Result<Vector3, String> {
        Ok(Vector3 {
            x: self.f32()?,
            y: self.f32()?,
            z: self.f32()?,
        })
    }

    /// A string: its length (u32), then its bytes.
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    fn text(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.bytes()?).into_owned())
    }

    /// A count (u32) of things at least `size` bytes each, checked against
    /// the bytes left.
    fn count(&mut self, size: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n.saturating_mul(size) > self.left() {
            return Err(format!("it claims {n} entries with only {} bytes left", self.left()));
        }
        Ok(n)
    }

    /// `n` values of N bytes stored byte column by byte column: every
    /// value's first byte, then every second byte, and so on.
    fn interleaved<const N: usize>(&mut self, n: usize) -> Result<Vec<[u8; N]>, String> {
        let data = self.take(n.checked_mul(N).ok_or_else(cut)?)?;
        Ok((0..n).map(|i| std::array::from_fn(|b| data[i + n * b])).collect())
    }

    fn u32s(&mut self, n: usize) -> Result<Vec<u32>, String> {
        Ok(self.interleaved(n)?.into_iter().map(u32::from_be_bytes).collect())
    }

    fn i32s(&mut self, n: usize) -> Result<Vec<i32>, String> {
        Ok(self.u32s(n)?.into_iter().map(untransform32).collect())
    }

    fn i64s(&mut self, n: usize) -> Result<Vec<i64>, String> {
        let v = self.interleaved(n)?;
        Ok(v.into_iter().map(|b| untransform64(u64::from_be_bytes(b))).collect())
    }

    /// Roblox's floats: big-endian, with the sign moved to the lowest bit.
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, String> {
        Ok(self
            .u32s(n)?
            .into_iter()
            .map(|x| f32::from_bits(x.rotate_right(1)))
            .collect())
    }

    /// Referents: transformed integers, each the difference from the one before.
    fn refs(&mut self, n: usize) -> Result<Vec<i32>, String> {
        let mut last = 0i32;
        Ok(self
            .i32s(n)?
            .into_iter()
            .map(|d| {
                last = last.wrapping_add(d);
                last
            })
            .collect())
    }
}

struct Binary {
    file: RbxFile,
    data: Budget,
    tree: Budget,
    shared: Vec<Arc<[u8]>>,
    /// Class IDs, with their names and items.
    classes: HashMap<u32, (String, Range<usize>)>,
    by_ref: HashMap<i32, usize>,
    /// Items a PRNT chunk placed.
    placed: Vec<bool>,
}

/// A binary model or place file.
pub fn read_binary(bytes: &[u8], limits: Limits) -> Result<RbxFile, String> {
    let mut c = Cursor::new(bytes);
    let head = c
        .take(32)
        .map_err(|_| "not a Roblox binary file: it is shorter than the 32-byte header".to_string())?;
    if !head.starts_with(MAGIC) {
        return Err("not a Roblox binary file: it doesn't start with <roblox!".into());
    }
    if head[8..14] != SIGNATURE {
        return Err("the file's binary signature is damaged (was it saved or downloaded as text?)".into());
    }
    let version = u16::from_le_bytes([head[14], head[15]]);
    if version != 0 {
        return Err(format!("binary format version {version} isn't known (only 0 is)"));
    }
    let mut bin = Binary {
        file: RbxFile::default(),
        data: Budget::new(limits.data, "chunks decompress to"),
        tree: Budget::new(limits.tree, TREE),
        shared: Vec::new(),
        classes: HashMap::new(),
        by_ref: HashMap::new(),
        placed: Vec::new(),
    };
    loop {
        let h = c
            .take(16)
            .map_err(|_| "the file is cut short: it ends before its END chunk".to_string())?;
        let tag = [h[0], h[1], h[2], h[3]];
        let name = chunk_name(&tag);
        let packed = le32(&h[4..8]) as usize;
        let size = le32(&h[8..12]) as usize;
        let short = |_| format!("the file is cut short inside its {name} chunk");
        let data: Cow<[u8]> = if packed == 0 {
            Cow::Borrowed(c.take(size).map_err(short)?)
        } else {
            let body = c.take(packed).map_err(short)?;
            Cow::Owned(unpack(&name, body, size, &mut bin.data)?)
        };
        let done = match &tag {
            b"META" => bin.meta(&data),
            b"SSTR" => bin.sstr(&data),
            b"INST" => bin.inst(&data),
            b"PROP" => bin.prop(&data),
            b"PRNT" => bin.prnt(&data),
            b"END\0" => break,
            // Signatures, which only Roblox can check.
            b"SIGN" => Ok(()),
            _ => {
                note(&mut bin.file.notes, || format!("skipped an unknown {name} chunk"));
                Ok(())
            }
        };
        done.map_err(|e| format!("bad {name} chunk: {e}"))?;
    }
    bin.finish()
}

/// A chunk's name for a message ("INST"), its padding left off.
fn chunk_name(tag: &[u8; 4]) -> String {
    let end = tag.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    tag[..end].escape_ascii().to_string()
}

/// A compressed chunk's data: ZSTD when it starts with ZSTD's magic number,
/// otherwise an LZ4 block.
fn unpack(name: &str, body: &[u8], size: usize, budget: &mut Budget) -> Result<Vec<u8>, String> {
    budget.spend(size)?;
    let out = if body.starts_with(&ZSTD_MAGIC) {
        zstd_decompress(body, size)
    } else {
        lz4_decompress(body, size)
    }
    .map_err(|e| format!("the {name} chunk doesn't decompress: {e}"))?;
    if out.len() != size {
        return Err(format!(
            "the {name} chunk decompresses to {} bytes, not the {size} it claims",
            out.len()
        ));
    }
    Ok(out)
}

impl Binary {
    fn meta(&mut self, data: &[u8]) -> Result<(), String> {
        let mut c = Cursor::new(data);
        let n = c.count(8)?;
        for _ in 0..n {
            let (k, v) = (c.text()?, c.text()?);
            self.tree.spend(48 + k.len() + v.len())?;
            self.file.meta.push((k, v));
        }
        Ok(())
    }

    fn sstr(&mut self, data: &[u8]) -> Result<(), String> {
        let mut c = Cursor::new(data);
        let version = c.u32()?;
        if version != 0 {
            return Err(format!("it is version {version} (only 0 is known)"));
        }
        let n = c.count(20)?;
        for _ in 0..n {
            // An MD5 hash, which nothing checks.
            c.take(16)?;
            let s = c.bytes()?;
            self.tree.spend(32 + s.len())?;
            self.shared.push(Arc::from(s));
        }
        Ok(())
    }

    fn inst(&mut self, data: &[u8]) -> Result<(), String> {
        let mut c = Cursor::new(data);
        let id = c.u32()?;
        let class = c.text()?;
        let service = c.u8()? == 1;
        let n = c.count(4)?;
        let refs = c.refs(n)?;
        // Services' markers (a 1 for each) follow; nothing needs them.
        if self.classes.contains_key(&id) {
            return Err(format!("class ID {id} has two INST chunks"));
        }
        self.tree.spend(n.saturating_mul(ITEM_COST + class.len() + 12))?;
        let start = self.file.items.len();
        self.file.items.reserve(n);
        for (k, r) in refs.into_iter().enumerate() {
            if self.by_ref.insert(r, start + k).is_some() {
                return Err(format!("referent {r} is used twice"));
            }
            self.file.items.push(Item {
                class: class.clone(),
                referent: r.to_string(),
                service,
                ..Item::default()
            });
        }
        self.classes.insert(id, (class, start..start + n));
        Ok(())
    }

    fn prop(&mut self, data: &[u8]) -> Result<(), String> {
        let mut c = Cursor::new(data);
        let id = c.u32()?;
        let name = c.text()?;
        let Some((class, range)) = self.classes.get(&id) else {
            return Err(format!(
                "{} is for class ID {id}, which no INST chunk defines",
                clip(&name)
            ));
        };
        let range = range.clone();
        // A chunk without a type is ignored, as Roblox ignores it.
        let Ok(ty) = c.u8() else {
            return Ok(());
        };
        let Some(ty_name) = type_name(ty) else {
            let what = format!("{}.{}", clip(class), clip(&name));
            note(&mut self.file.notes, || {
                format!("skipped {what}: its type ({ty:#04x}) isn't known")
            });
            return Ok(());
        };
        let what = || format!("{}.{} ({ty_name})", clip(class), clip(&name));
        // Every value takes a byte at least.
        let n = range.len();
        if n > c.left() {
            return Err(format!("{} is cut short", what()));
        }
        self.tree.spend(n.saturating_mul(PROP_COST + name.len()))?;
        let values = decode(&mut c, ty, n, &self.shared, &self.by_ref).map_err(|e| format!("{}: {e}", what()))?;
        for (item, v) in self.file.items[range].iter_mut().zip(values) {
            item.properties.push((name.clone(), v));
        }
        Ok(())
    }

    fn prnt(&mut self, data: &[u8]) -> Result<(), String> {
        let mut c = Cursor::new(data);
        let version = c.u8()?;
        if version != 0 {
            return Err(format!("it is version {version} (only 0 is known)"));
        }
        let n = c.count(8)?;
        let children = c.refs(n)?;
        let parents = c.refs(n)?;
        self.placed.resize(self.file.items.len(), false);
        let find = |r: i32| {
            self.by_ref
                .get(&r)
                .copied()
                .ok_or_else(|| format!("it names referent {r}, which no INST chunk defines"))
        };
        for (child, parent) in children.into_iter().zip(parents) {
            let k = find(child)?;
            if std::mem::replace(&mut self.placed[k], true) {
                return Err(format!("it places referent {child} twice"));
            }
            if parent == -1 {
                self.file.roots.push(k);
            } else {
                let p = find(parent)?;
                self.file.items[k].parent = Some(p);
                self.file.items[p].children.push(k);
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<RbxFile, String> {
        // Items that no PRNT chunk placed are roots too.
        self.placed.resize(self.file.items.len(), false);
        let unplaced = self.placed.iter().enumerate().filter(|(_, p)| !**p).map(|(i, _)| i);
        self.file.roots.extend(unplaced);
        check_tree(&self.file)?;
        Ok(self.file)
    }
}

/// A PROP chunk's `n` values of type `ty` (one `type_name` knows).
fn decode(
    c: &mut Cursor,
    ty: u8,
    n: usize,
    shared: &[Arc<[u8]>],
    by_ref: &HashMap<i32, usize>,
) -> Result<Vec<PropValue>, String> {
    use PropValue as V;
    let resolve = |r: i32| by_ref.get(&r).copied();
    Ok(match ty {
        0x01 => each(c, n, |c| Ok(V::String(c.bytes()?.to_vec())))?,
        0x02 => c.take(n)?.iter().map(|&b| V::Bool(b != 0)).collect(),
        0x03 => c.i32s(n)?.into_iter().map(V::Int32).collect(),
        0x04 => c.f32s(n)?.into_iter().map(V::Float32).collect(),
        0x05 => each(c, n, |c| Ok(V::Float64(c.f64()?)))?,
        0x06 => {
            let (scale, offset) = (c.f32s(n)?, c.i32s(n)?);
            let udims = scale.into_iter().zip(offset);
            udims.map(|(scale, offset)| V::UDim(UDim { scale, offset })).collect()
        }
        0x07 => {
            let (xs, ys, xo, yo) = (c.f32s(n)?, c.f32s(n)?, c.i32s(n)?, c.i32s(n)?);
            let parts = xs.into_iter().zip(ys).zip(xo.into_iter().zip(yo));
            let udim2 = |((xs, ys), (xo, yo))| {
                V::UDim2(UDim2 {
                    x: UDim { scale: xs, offset: xo },
                    y: UDim { scale: ys, offset: yo },
                })
            };
            parts.map(udim2).collect()
        }
        0x08 => each(c, n, |c| {
            Ok(V::Ray(Ray {
                origin: c.vector3()?,
                direction: c.vector3()?,
            }))
        })?,
        0x09 => c.take(n)?.iter().map(|&b| V::Faces(b & 63)).collect(),
        0x0a => c.take(n)?.iter().map(|&b| V::Axes(b & 7)).collect(),
        0x0b => c.u32s(n)?.into_iter().map(V::BrickColor).collect(),
        0x0c => {
            let (r, g, b) = (c.f32s(n)?, c.f32s(n)?, c.f32s(n)?);
            let rgb = r.into_iter().zip(g).zip(b);
            rgb.map(|((r, g), b)| V::Color3(Color3 { r, g, b })).collect()
        }
        0x0d => {
            let (x, y) = (c.f32s(n)?, c.f32s(n)?);
            x.into_iter()
                .zip(y)
                .map(|(x, y)| V::Vector2(Vector2 { x, y }))
                .collect()
        }
        0x0e => {
            let (x, y, z) = (c.f32s(n)?, c.f32s(n)?, c.f32s(n)?);
            let xyz = x.into_iter().zip(y).zip(z);
            xyz.map(|((x, y), z)| V::Vector3(Vector3 { x, y, z })).collect()
        }
        0x10 => cframes(c, n)?.into_iter().map(V::CFrame).collect(),
        0x12 => c.u32s(n)?.into_iter().map(V::Enum).collect(),
        0x13 => c.refs(n)?.into_iter().map(|r| V::Ref(resolve(r))).collect(),
        0x14 => each(c, n, |c| Ok(V::Vector3int16([c.i16()?, c.i16()?, c.i16()?])))?,
        0x15 => each(c, n, |c| {
            let k = c.count(12)?;
            let mut points = Vec::with_capacity(k);
            for _ in 0..k {
                let (time, value, envelope) = (c.f32()?, c.f32()?, c.f32()?);
                points.push(NumberKeypoint { time, value, envelope });
            }
            Ok(V::NumberSequence(points))
        })?,
        0x16 => each(c, n, |c| {
            let k = c.count(20)?;
            let mut points = Vec::with_capacity(k);
            for _ in 0..k {
                let time = c.f32()?;
                let color = Color3 {
                    r: c.f32()?,
                    g: c.f32()?,
                    b: c.f32()?,
                };
                points.push(ColorKeypoint {
                    time,
                    color,
                    envelope: c.f32()?,
                });
            }
            Ok(V::ColorSequence(points))
        })?,
        0x17 => each(c, n, |c| {
            let (min, max) = (c.f32()?, c.f32()?);
            Ok(V::NumberRange(NumberRange { min, max }))
        })?,
        0x18 => {
            let (x0, y0, x1, y1) = (c.f32s(n)?, c.f32s(n)?, c.f32s(n)?, c.f32s(n)?);
            let corners = x0.into_iter().zip(y0).zip(x1.into_iter().zip(y1));
            let rect = |((x0, y0), (x1, y1))| {
                V::Rect(Rect {
                    min: Vector2 { x: x0, y: y0 },
                    max: Vector2 { x: x1, y: y1 },
                })
            };
            corners.map(rect).collect()
        }
        0x19 => each(c, n, |c| {
            // Bit 0: custom; bit 1: AcousticAbsorption is there.
            let flags = c.u8()?;
            if flags > 3 {
                return Err(format!("{flags} isn't a PhysicalProperties flag byte"));
            }
            if flags & 1 == 0 {
                return Ok(V::PhysicalProperties(None));
            }
            Ok(V::PhysicalProperties(Some(CustomPhysics {
                density: c.f32()?,
                friction: c.f32()?,
                elasticity: c.f32()?,
                friction_weight: c.f32()?,
                elasticity_weight: c.f32()?,
                acoustic_absorption: if flags & 2 != 0 { c.f32()? } else { 1.0 },
            })))
        })?,
        0x1a => {
            let (r, g, b) = (c.take(n)?, c.take(n)?, c.take(n)?);
            let rgb = r.iter().zip(g).zip(b);
            rgb.map(|((&r, &g), &b)| V::Color3uint8([r, g, b])).collect()
        }
        0x1b => c.i64s(n)?.into_iter().map(V::Int64).collect(),
        0x1c => {
            let strings = c.u32s(n)?.into_iter().map(|i| {
                let s = shared
                    .get(i as usize)
                    .ok_or_else(|| format!("it uses shared string {i}, but the SSTR chunk has {}", shared.len()))?;
                Ok(V::SharedString(s.clone()))
            });
            strings.collect::<Result<Vec<_>, String>>()?
        }
        0x1d => each(c, n, |c| Ok(V::Bytecode(c.bytes()?.to_vec())))?,
        0x1e => {
            let t = c.u8()?;
            if t != 0x10 {
                return Err(format!("its CFrames' type is {t:#04x}, not 0x10"));
            }
            let frames = cframes(c, n)?;
            let t = c.u8()?;
            if t != 0x02 {
                return Err(format!("its flags' type is {t:#04x}, not 0x02"));
            }
            let set = c.take(n)?;
            let frames = frames.into_iter().zip(set);
            frames.map(|(f, &s)| V::OptionalCFrame((s != 0).then_some(f))).collect()
        }
        0x1f => {
            let ids = c.interleaved::<16>(n)?.into_iter().map(|b| UniqueId {
                index: be32(&b[0..4]),
                time: be32(&b[4..8]),
                random: ((u64::from(be32(&b[8..12])) << 32) | u64::from(be32(&b[12..16]))).rotate_right(1) as i64,
            });
            ids.map(V::UniqueId).collect()
        }
        0x20 => each(c, n, |c| {
            Ok(V::Font(Font {
                family: c.text()?,
                weight: c.u16()?,
                style: c.u8()?,
                cached_face_id: c.text()?,
            }))
        })?,
        0x21 => c
            .i64s(n)?
            .into_iter()
            .map(|v| V::SecurityCapabilities(v as u64))
            .collect(),
        0x22 => {
            // Each value's source (0 none, 1 URI, 2 object), then the URIs
            // and the objects in order, then objects outside the file.
            let sources = c.i32s(n)?;
            let k = c.count(4)?;
            let mut uris = Vec::with_capacity(k);
            for _ in 0..k {
                uris.push(c.text()?);
            }
            let k = c.count(4)?;
            let objects = c.refs(k)?;
            let k = c.count(4)?;
            c.take(k * 4)?;
            let (mut uris, mut objects) = (uris.into_iter(), objects.into_iter());
            let mut v = Vec::with_capacity(n);
            for source in sources {
                v.push(V::Content(match source {
                    0 => Content::None,
                    1 => Content::Uri(uris.next().ok_or("it has fewer URIs than values that use them")?),
                    2 => Content::Object(resolve(
                        objects.next().ok_or("it has fewer objects than values that use them")?,
                    )),
                    s => return Err(format!("{s} isn't a Content source")),
                }));
            }
            v
        }
        _ => return Err(format!("type {ty:#04x} isn't known")),
    })
}

/// `n` values, each read on its own.
fn each(
    c: &mut Cursor,
    n: usize,
    mut f: impl FnMut(&mut Cursor) -> Result<PropValue, String>,
) -> Result<Vec<PropValue>, String> {
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(f(c)?);
    }
    Ok(v)
}

/// `n` CFrames: each one's rotation ID and, for ID 0, its matrix (9 floats);
/// then the positions, as Vector3s.
fn cframes(c: &mut Cursor, n: usize) -> Result<Vec<CFrame>, String> {
    let mut rotations = Vec::with_capacity(n);
    for _ in 0..n {
        let id = c.u8()?;
        rotations.push(if id == 0 {
            let mut m = [[0.0; 3]; 3];
            for v in m.iter_mut().flatten() {
                *v = c.f32()?;
            }
            m
        } else {
            rotation_from_id(id).ok_or_else(|| format!("{id:#04x} isn't a CFrame rotation ID"))?
        });
    }
    let (x, y, z) = (c.f32s(n)?, c.f32s(n)?, c.f32s(n)?);
    let parts = rotations.into_iter().zip(x).zip(y).zip(z);
    Ok(parts
        .map(|(((rotation, x), y), z)| CFrame {
            position: Vector3 { x, y, z },
            rotation,
        })
        .collect())
}

// ---------------------------------------------------------------- ZSTD

// ZSTD frames as RFC 8878 has them: raw, RLE and compressed blocks, Huffman
// literals, FSE sequences and the content checksum (XXH64). Dictionaries
// aren't supported (Roblox doesn't use them).

/// The most a block holds or makes.
const BLOCK_MAX: usize = 128 << 10;

fn zstd_err(what: &str) -> String {
    format!("the ZSTD data is corrupt ({what})")
}

/// ZSTD frames' contents, `size` bytes at most (skippable frames are skipped).
pub fn zstd_decompress(data: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let mut c = Cursor::new(data);
    let mut out = Vec::with_capacity(size.min(data.len().saturating_mul(8)));
    while c.left() > 0 {
        let magic = c.u32()?;
        if magic & 0xffff_fff0 == 0x184d_2a50 {
            let n = c.u32()? as usize;
            c.take(n)?;
        } else if magic == 0xfd2f_b528 {
            zstd_frame(&mut c, &mut out, size)?;
        } else {
            return Err("not ZSTD data: a frame has the wrong magic number".into());
        }
    }
    Ok(out)
}

/// What a frame's blocks share.
struct Zstd {
    /// The last three offsets.
    rep: [usize; 3],
    huffman: Option<Huffman>,
    /// The literal length, offset and match length tables.
    tables: [Option<Fse>; 3],
    /// The frame's first byte in the output.
    start: usize,
    limit: usize,
}

fn zstd_frame(c: &mut Cursor, out: &mut Vec<u8>, limit: usize) -> Result<(), String> {
    let flags = c.u8()?;
    if flags & 8 != 0 {
        return Err(zstd_err("a reserved bit is set"));
    }
    let single = flags & 0x20 != 0;
    if !single {
        // The window's size: everything is in memory anyway.
        c.u8()?;
    }
    let little = |b: &[u8]| b.iter().rev().fold(0u64, |v, &x| (v << 8) | u64::from(x));
    let dictionary = little(c.take([0, 1, 2, 4][usize::from(flags & 3)])?);
    if dictionary != 0 {
        return Err("the ZSTD data needs a dictionary".into());
    }
    let content = match (flags >> 6, single) {
        (0, false) => None,
        (0, true) => Some(little(c.take(1)?)),
        (1, _) => Some(little(c.take(2)?) + 256),
        (2, _) => Some(little(c.take(4)?)),
        _ => Some(little(c.take(8)?)),
    };
    if content.is_some_and(|n| n > (limit - out.len()) as u64) {
        return Err("the ZSTD data decompresses to more than the chunk says".into());
    }
    let mut z = Zstd {
        rep: [1, 4, 8],
        huffman: None,
        tables: [None, None, None],
        start: out.len(),
        limit,
    };
    loop {
        let h = c.take(3)?;
        let h = u32::from(h[0]) | u32::from(h[1]) << 8 | u32::from(h[2]) << 16;
        let n = (h >> 3) as usize;
        if n > BLOCK_MAX {
            return Err(zstd_err("a block is too big"));
        }
        match (h >> 1) & 3 {
            0 => z.push(out, c.take(n)?)?,
            1 => {
                let b = c.u8()?;
                z.room(out, n)?;
                out.resize(out.len() + n, b);
            }
            2 => z.block(c.take(n)?, out)?,
            _ => return Err(zstd_err("a block has the reserved type")),
        }
        if h & 1 != 0 {
            break;
        }
    }
    let made = &out[z.start..];
    if content.is_some_and(|n| n != made.len() as u64) {
        return Err(zstd_err("it isn't the size its frame says"));
    }
    if flags & 4 != 0 && c.u32()? != xxh64(made) as u32 {
        return Err(zstd_err("its checksum doesn't match"));
    }
    Ok(())
}

/// Bits read backwards, from the highest: a ZSTD bitstream ends with a 1 bit
/// above its last bit. Bits below its start read as 0, as the format wants.
struct Back<'a> {
    b: &'a [u8],
    /// Bits [0, pos) are still to read; it goes below 0 when reading past the start.
    pos: isize,
}

impl<'a> Back<'a> {
    fn new(b: &'a [u8]) -> Result<Back<'a>, String> {
        match b.last() {
            Some(&last) if last != 0 => Ok(Back {
                b,
                pos: (b.len() as isize - 1) * 8 + 7 - last.leading_zeros() as isize,
            }),
            _ => Err(zstd_err("a bitstream has no end mark")),
        }
    }

    /// The next `n` bits (at most 56) without reading them.
    fn peek(&self, n: u32) -> u64 {
        if n == 0 || self.pos <= 0 {
            return 0;
        }
        let end = self.pos as usize;
        let start = end.saturating_sub(n as usize);
        let mut v = 0u64;
        for (k, &x) in self.b[start / 8..=(end - 1) / 8].iter().enumerate() {
            v |= u64::from(x) << (8 * k);
        }
        let width = end - start;
        ((v >> (start % 8)) & ((1u64 << width) - 1)) << (n as usize - width)
    }

    fn read(&mut self, n: u32) -> u64 {
        let v = self.peek(n);
        self.pos -= n as isize;
        v
    }

    fn overflowed(&self) -> bool {
        self.pos < 0
    }
}

/// Bits read forwards, from the lowest of the first byte; bits past the end
/// can be looked at (as 0) but not read.
struct Fwd<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Fwd<'_> {
    fn peek(&self, n: u32) -> u32 {
        let first = self.pos / 8;
        let v = (0..5).fold(0u64, |v, k| {
            v | u64::from(self.b.get(first + k).copied().unwrap_or(0)) << (8 * k)
        });
        ((v >> (self.pos % 8)) & ((1 << n) - 1)) as u32
    }

    fn skip(&mut self, n: u32) -> Result<(), String> {
        self.pos += n as usize;
        if self.pos > self.b.len() * 8 {
            return Err(zstd_err("an FSE table is cut short"));
        }
        Ok(())
    }

    fn read(&mut self, n: u32) -> Result<u32, String> {
        let v = self.peek(n);
        self.skip(n)?;
        Ok(v)
    }
}

/// An FSE decoding table.
struct Fse {
    log: u32,
    /// Each state's symbol, how many bits to read for the next, and what to add to them.
    cells: Vec<(u8, u8, u16)>,
}

impl Fse {
    /// The table for symbols' counts out of 1 << log (-1 for "less than 1").
    fn new(counts: &[i32], log: u32) -> Result<Fse, String> {
        let size = 1usize << log;
        let total: i64 = counts.iter().map(|&c| i64::from(if c == -1 { 1 } else { c })).sum();
        if total != size as i64 || counts.iter().any(|&c| c < -1) {
            return Err(zstd_err("an FSE table's counts don't add up"));
        }
        let mut cells = vec![(0u8, 0u8, 0u16); size];
        let mut next = vec![0u32; counts.len()];
        // Symbols of "less than 1" go at the end.
        let mut high = size as isize - 1;
        for (s, &n) in counts.iter().enumerate() {
            if n == -1 {
                cells[high as usize].0 = s as u8;
                high -= 1;
                next[s] = 1;
            } else {
                next[s] = n as u32;
            }
        }
        // The others are spread through the rest (the step is odd, so it reaches every cell).
        let step = (size >> 1) + (size >> 3) + 3;
        let mut pos = 0;
        for (s, &n) in counts.iter().enumerate() {
            for _ in 0..n.max(0) {
                cells[pos].0 = s as u8;
                pos = (pos + step) & (size - 1);
                while pos as isize > high {
                    pos = (pos + step) & (size - 1);
                }
            }
        }
        for cell in &mut cells {
            let x = &mut next[usize::from(cell.0)];
            let bits = log - (31 - x.leading_zeros());
            cell.1 = bits as u8;
            cell.2 = ((*x << bits) as usize - size) as u16;
            *x += 1;
        }
        Ok(Fse { log, cells })
    }

    /// A table description: its accuracy log (at most `max_log`) and the
    /// counts of symbols 0 to `max_symbol` at most. Returns the table and
    /// the bytes it took.
    fn read(b: &[u8], max_log: u32, max_symbol: usize) -> Result<(Fse, usize), String> {
        let mut r = Fwd { b, pos: 0 };
        let log = r.read(4)? + 5;
        if log > max_log {
            return Err(zstd_err("an FSE table is too precise"));
        }
        let mut remaining = (1i32 << log) + 1;
        let mut threshold = 1i32 << log;
        let mut bits = log + 1;
        let mut counts = Vec::new();
        let mut zero = false;
        while remaining > 1 {
            if zero {
                // A count of 0 is followed by how many more there are, 2 bits at a time.
                loop {
                    let more = r.read(2)?;
                    counts.extend(std::iter::repeat_n(0, more as usize));
                    if counts.len() > max_symbol {
                        return Err(zstd_err("an FSE table has too many symbols"));
                    }
                    if more != 3 {
                        break;
                    }
                }
            }
            if counts.len() > max_symbol {
                return Err(zstd_err("an FSE table has too many symbols"));
            }
            let max = 2 * threshold - 1 - remaining;
            let low = r.peek(bits - 1) as i32;
            let mut count = if low < max {
                r.skip(bits - 1)?;
                low
            } else {
                let v = r.read(bits)? as i32;
                if v >= threshold { v - max } else { v }
            };
            count -= 1;
            remaining -= count.abs();
            counts.push(count);
            zero = count == 0;
            if remaining < 1 {
                return Err(zstd_err("an FSE table's counts add up to too much"));
            }
            while remaining < threshold {
                bits -= 1;
                threshold >>= 1;
            }
        }
        Ok((Fse::new(&counts, log)?, r.pos.div_ceil(8)))
    }

    /// A table of one symbol, which reads no bits.
    fn rle(symbol: u8) -> Fse {
        Fse {
            log: 0,
            cells: vec![(symbol, 0, 0)],
        }
    }

    fn symbol(&self, state: usize) -> u8 {
        self.cells[state].0
    }

    fn next(&self, state: usize, bits: &mut Back) -> usize {
        let (_, n, base) = self.cells[state];
        usize::from(base) + bits.read(u32::from(n)) as usize
    }
}

/// A Huffman decoding table: for every `bits`-bit value, the symbol it
/// starts with and that symbol's length in bits.
struct Huffman {
    bits: u32,
    table: Vec<(u8, u8)>,
}

impl Huffman {
    /// The table for symbols' weights (the last one's is implied).
    fn new(weights: &[u8]) -> Result<Huffman, String> {
        let bad = || zstd_err("a Huffman table is invalid");
        if weights.len() > 255 || weights.iter().any(|&w| w > 11) {
            return Err(bad());
        }
        let total: u32 = weights.iter().filter(|&&w| w > 0).map(|&w| 1 << (w - 1)).sum();
        if total == 0 {
            return Err(bad());
        }
        let bits = 32 - total.leading_zeros();
        let rest = (1 << bits) - total;
        if bits > 11 || !rest.is_power_of_two() {
            return Err(bad());
        }
        let mut weights = weights.to_vec();
        weights.push(rest.trailing_zeros() as u8 + 1);
        // The longest codes come in pairs.
        let ones = weights.iter().filter(|&&w| w == 1).count();
        if ones < 2 || ones % 2 == 1 {
            return Err(bad());
        }
        let mut table = Vec::with_capacity(1 << bits);
        for w in 1..=bits {
            for (s, _) in weights.iter().enumerate().filter(|&(_, &x)| u32::from(x) == w) {
                table.extend(std::iter::repeat_n((s as u8, (bits + 1 - w) as u8), 1 << (w - 1)));
            }
        }
        if table.len() != 1 << bits {
            return Err(bad());
        }
        Ok(Huffman { bits, table })
    }

    /// A table description: the weights, either 4 bits each or FSE
    /// compressed. Returns the table and the bytes it took.
    fn read(b: &[u8]) -> Result<(Huffman, usize), String> {
        let head = usize::from(*b.first().ok_or_else(cut)?);
        if head >= 128 {
            let n = head - 127;
            let data = b.get(1..1 + n.div_ceil(2)).ok_or_else(cut)?;
            let weights: Vec<u8> = (0..n).map(|i| data[i / 2] >> (4 * (1 - i % 2)) & 15).collect();
            return Ok((Huffman::new(&weights)?, 1 + n.div_ceil(2)));
        }
        let data = b.get(1..1 + head).ok_or_else(cut)?;
        let (fse, used) = Fse::read(data, 6, 255)?;
        // Two states take turns until the bits run out.
        let mut bits = Back::new(&data[used..])?;
        let mut states = [bits.read(fse.log) as usize, bits.read(fse.log) as usize];
        let mut weights = Vec::new();
        for k in (0..2).cycle() {
            if weights.len() + 2 > 255 {
                return Err(zstd_err("a Huffman table has too many weights"));
            }
            weights.push(fse.symbol(states[k]));
            states[k] = fse.next(states[k], &mut bits);
            if bits.overflowed() {
                weights.push(fse.symbol(states[1 - k]));
                break;
            }
        }
        Ok((Huffman::new(&weights)?, 1 + head))
    }

    /// `n` symbols from one stream, which they must use up exactly.
    fn decode(&self, stream: &[u8], n: usize, out: &mut Vec<u8>) -> Result<(), String> {
        let mut bits = Back::new(stream)?;
        for _ in 0..n {
            let (symbol, len) = self.table[bits.peek(self.bits) as usize];
            bits.pos -= isize::from(len);
            out.push(symbol);
        }
        if bits.pos != 0 {
            return Err(zstd_err("a Huffman stream isn't the length it should be"));
        }
        Ok(())
    }
}

/// Literal length codes' baselines and extra bits.
const LL_BASE: [u32; 36] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32, 40, 48, 64, 128, 256, 512, 1024,
    2048, 4096, 8192, 16384, 32768, 65536,
];
const LL_BITS: [u32; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];
/// Match length codes' baselines and extra bits.
const ML_BASE: [u32; 53] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    33, 34, 35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 131, 259, 515, 1027, 2051, 4099, 8195, 16387, 32771, 65539,
];
const ML_BITS: [u32; 53] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2,
    3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];
/// The predefined counts for literal lengths, offsets and match lengths, and their accuracy logs.
const LL_COUNTS: [i32; 36] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1, -1, -1, -1, -1,
];
const OF_COUNTS: [i32; 29] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];
const ML_COUNTS: [i32; 53] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];

impl Zstd {
    /// Checks that `n` more bytes fit in the output.
    fn room(&self, out: &[u8], n: usize) -> Result<(), String> {
        if n > self.limit - out.len() {
            return Err("the ZSTD data decompresses to more than the chunk says".into());
        }
        Ok(())
    }

    fn push(&self, out: &mut Vec<u8>, b: &[u8]) -> Result<(), String> {
        self.room(out, b.len())?;
        out.extend_from_slice(b);
        Ok(())
    }

    /// A compressed block: its literals, then the sequences that copy them
    /// and earlier output.
    fn block(&mut self, b: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
        let (literals, used) = self.literals(b)?;
        let b = &b[used..];
        let first = usize::from(*b.first().ok_or_else(cut)?);
        let (count, mut at) = match first {
            0 => {
                if b.len() != 1 {
                    return Err(zstd_err("a block goes on after it ends"));
                }
                return self.push(out, &literals);
            }
            1..128 => (first, 1),
            128..255 => (((first - 128) << 8) + usize::from(*b.get(1).ok_or_else(cut)?), 2),
            _ => {
                let (lo, hi) = (b.get(1).ok_or_else(cut)?, b.get(2).ok_or_else(cut)?);
                (usize::from(*lo) + (usize::from(*hi) << 8) + 0x7f00, 3)
            }
        };
        let modes = *b.get(at).ok_or_else(cut)?;
        at += 1;
        if modes & 3 != 0 {
            return Err(zstd_err("a block's reserved bits are set"));
        }
        // Literal lengths, offsets, then match lengths.
        let kinds: [(u8, &[i32], u32, u32, usize); 3] = [
            (modes >> 6, &LL_COUNTS, 6, 9, 35),
            (modes >> 4 & 3, &OF_COUNTS, 5, 8, 31),
            (modes >> 2 & 3, &ML_COUNTS, 6, 9, 52),
        ];
        for (k, (mode, counts, log, max_log, max_symbol)) in kinds.into_iter().enumerate() {
            let rest = b.get(at..).ok_or_else(cut)?;
            self.tables[k] = Some(match mode {
                0 => Fse::new(counts, log)?,
                1 => {
                    let s = *rest.first().ok_or_else(cut)?;
                    if usize::from(s) > max_symbol {
                        return Err(zstd_err("a sequence code is out of range"));
                    }
                    at += 1;
                    Fse::rle(s)
                }
                2 => {
                    let (fse, used) = Fse::read(rest, max_log, max_symbol)?;
                    at += used;
                    fse
                }
                _ => self.tables[k]
                    .take()
                    .ok_or_else(|| zstd_err("a block repeats tables it hasn't had"))?,
            });
        }
        let [Some(ll), Some(of), Some(ml)] = &self.tables else {
            return Err(zstd_err("a block has no tables"));
        };
        let mut bits = Back::new(b.get(at..).ok_or_else(cut)?)?;
        let mut states = [
            bits.read(ll.log) as usize,
            bits.read(of.log) as usize,
            bits.read(ml.log) as usize,
        ];
        let mut lit = 0;
        for i in 0..count {
            let (ll_code, of_code, ml_code) = (
                usize::from(ll.symbol(states[0])),
                u32::from(of.symbol(states[1])),
                usize::from(ml.symbol(states[2])),
            );
            let value = (1usize << of_code) + bits.read(of_code) as usize;
            let matched = (ML_BASE[ml_code] + bits.read(ML_BITS[ml_code]) as u32) as usize;
            let length = (LL_BASE[ll_code] + bits.read(LL_BITS[ll_code]) as u32) as usize;
            let rep = &mut self.rep;
            let offset = if value > 3 {
                *rep = [value - 3, rep[0], rep[1]];
                rep[0]
            } else {
                // A repeated offset, one further back when there are no literals.
                let r = value - 1 + usize::from(length == 0);
                let offset = if r == 3 { rep[0].wrapping_sub(1) } else { rep[r] };
                if offset == 0 {
                    return Err(zstd_err("an offset is 0"));
                }
                match r {
                    0 => {}
                    1 => *rep = [offset, rep[0], rep[2]],
                    _ => *rep = [offset, rep[0], rep[1]],
                }
                offset
            };
            if i + 1 < count {
                states[0] = ll.next(states[0], &mut bits);
                states[2] = ml.next(states[2], &mut bits);
                states[1] = of.next(states[1], &mut bits);
            }
            if bits.overflowed() {
                return Err(zstd_err("the sequences run out of bits"));
            }
            let lits = literals
                .get(lit..lit + length)
                .ok_or_else(|| zstd_err("a sequence has too few literals"))?;
            lit += length;
            self.push(out, lits)?;
            self.room(out, matched)?;
            let from = out
                .len()
                .checked_sub(offset)
                .filter(|&f| f >= self.start)
                .ok_or_else(|| zstd_err("an offset goes back before the frame"))?;
            if offset >= matched {
                out.extend_from_within(from..from + matched);
            } else {
                // The copy overlaps what it makes.
                for _ in 0..matched {
                    out.push(out[out.len() - offset]);
                }
            }
        }
        if bits.pos != 0 {
            return Err(zstd_err("the sequences don't use up their bits"));
        }
        self.push(out, &literals[lit..])
    }

    /// A block's literals section: the literals, and the bytes it took.
    fn literals(&mut self, b: &[u8]) -> Result<(Vec<u8>, usize), String> {
        let byte = |i: usize| b.get(i).map(|&x| usize::from(x)).ok_or_else(cut);
        let b0 = byte(0)?;
        let (kind, format) = (b0 & 3, b0 >> 2 & 3);
        if kind < 2 {
            // Raw or RLE: a size of 5, 12 or 20 bits.
            let (n, head) = match format {
                0 | 2 => (b0 >> 3, 1),
                1 => ((b0 >> 4) + (byte(1)? << 4), 2),
                _ => ((b0 >> 4) + (byte(1)? << 4) + (byte(2)? << 12), 3),
            };
            if n > BLOCK_MAX {
                return Err(zstd_err("a block has too many literals"));
            }
            return Ok(if kind == 0 {
                (b.get(head..head + n).ok_or_else(cut)?.to_vec(), head + n)
            } else {
                (vec![byte(head)? as u8; n], head + 1)
            });
        }
        // Huffman coded, with a new table or the last one: sizes of 10, 14 or 18 bits.
        let (streams, head, bits) = match format {
            0 => (1, 3, 10),
            1 => (4, 3, 10),
            2 => (4, 4, 14),
            _ => (4, 5, 18),
        };
        let h = (0..head).try_fold(0usize, |h, i| Ok::<_, String>(h | byte(i)? << (8 * i)))?;
        let mask = (1 << bits) - 1;
        let (n, size) = (h >> 4 & mask, h >> (4 + bits) & mask);
        if n > BLOCK_MAX {
            return Err(zstd_err("a block has too many literals"));
        }
        let mut data = b.get(head..head + size).ok_or_else(cut)?;
        if kind == 2 {
            let (huffman, used) = Huffman::read(data)?;
            self.huffman = Some(huffman);
            data = &data[used..];
        }
        let huffman = self
            .huffman
            .as_ref()
            .ok_or_else(|| zstd_err("literals reuse a Huffman table it hasn't had"))?;
        let mut literals = Vec::with_capacity(n);
        if streams == 1 {
            huffman.decode(data, n, &mut literals)?;
        } else {
            // A jump table gives the first three streams' sizes; each makes a quarter.
            let jump = data.get(..6).ok_or_else(cut)?;
            let sizes: Vec<usize> = (0..3)
                .map(|i| usize::from(jump[2 * i]) | usize::from(jump[2 * i + 1]) << 8)
                .collect();
            let quarter = n.div_ceil(4);
            let last = n
                .checked_sub(3 * quarter)
                .ok_or_else(|| zstd_err("too few literals for 4 streams"))?;
            let mut rest = &data[6..];
            for (i, want) in [quarter, quarter, quarter, last].into_iter().enumerate() {
                let len = sizes.get(i).copied().unwrap_or(rest.len());
                let stream = rest
                    .get(..len)
                    .ok_or_else(|| zstd_err("a Huffman stream is cut short"))?;
                huffman.decode(stream, want, &mut literals)?;
                rest = &rest[len..];
            }
        }
        Ok((literals, head + size))
    }
}

/// XXH64 with seed 0, which ZSTD's checksums use (their low 32 bits).
pub fn xxh64(data: &[u8]) -> u64 {
    const P1: u64 = 0x9e37_79b1_85eb_ca87;
    const P2: u64 = 0xc2b2_ae3d_27d4_eb4f;
    const P3: u64 = 0x1656_67b1_9e37_79f9;
    const P4: u64 = 0x85eb_ca77_c2b2_ae63;
    const P5: u64 = 0x27d4_eb2f_1656_67c5;
    let round = |acc: u64, v: u64| acc.wrapping_add(v.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1);
    let merge = |acc: u64, v: u64| (acc ^ round(0, v)).wrapping_mul(P1).wrapping_add(P4);
    let u64_at = |b: &[u8]| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
    let (stripes, tail) = data.split_at(data.len() / 32 * 32);
    let mut h = if data.len() >= 32 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        for stripe in stripes.as_chunks::<32>().0 {
            for (k, lane) in v.iter_mut().enumerate() {
                *lane = round(*lane, u64_at(&stripe[8 * k..]));
            }
        }
        let h = v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7));
        let h = h.wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18));
        v.iter().fold(h, |h, &lane| merge(h, lane))
    } else {
        P5
    };
    h = h.wrapping_add(data.len() as u64);
    let (words, mut rest) = tail.as_chunks::<8>();
    for w in words {
        h = (h ^ round(0, u64::from_le_bytes(*w)))
            .rotate_left(27)
            .wrapping_mul(P1)
            .wrapping_add(P4);
    }
    if rest.len() >= 4 {
        h ^= u64::from(le32(rest)).wrapping_mul(P1);
        h = h.rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rest = &rest[4..];
    }
    for &b in rest {
        h ^= u64::from(b).wrapping_mul(P5);
        h = h.rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

// ---------------------------------------------------------------- XML

/// An XML element; the document is element 0, its children the top-level
/// elements. Elements are kept in a list, not nested, so a deep file can't
/// overflow the stack.
#[derive(Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    /// Its text and CDATA, entities decoded.
    text: String,
    kids: Vec<usize>,
}

const NODE_COST: usize = size_of::<Node>() + 16;

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

/// Where the next `c` is from `i` on, or the end.
fn find_byte(b: &[u8], i: usize, c: u8) -> usize {
    b[i..].iter().position(|&x| x == c).map_or(b.len(), |k| i + k)
}

/// Text with its entities decoded and its line breaks made "\n", as XML
/// reads it; an `&` that starts no entity is kept as it is.
fn unescape_into(out: &mut String, s: &str) {
    let mut rest = s;
    while let Some(k) = rest.find(['&', '\r']) {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        if let Some(r) = rest.strip_prefix('\r') {
            out.push('\n');
            rest = r.strip_prefix('\n').unwrap_or(r);
            continue;
        }
        // An entity is short: look for its ';' close by.
        let semi = rest.as_bytes().iter().take(12).position(|&c| c == b';');
        if let Some((k, e)) = semi.and_then(|k| Some((k, entity(&rest[1..k])?))) {
            out.push(e);
            rest = &rest[k + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    unescape_into(&mut out, s);
    out
}

/// The character an entity's name (between `&` and `;`) stands for.
fn entity(name: &str) -> Option<char> {
    Some(match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

/// An XML document's elements. It is read tolerantly: comments, processing
/// instructions and DOCTYPEs are skipped, unknown entities and stray `<`s
/// are kept as text, attributes may be unquoted, and a closing tag that
/// matches no open element is ignored. A file that ends inside a tag or an
/// open element is an error, since it was cut short.
fn parse_xml(src: &str, budget: &mut Budget) -> Result<Vec<Node>, String> {
    let b = src.as_bytes();
    let mut nodes = vec![Node::default()];
    let mut open = vec![0usize];
    let ends = |what: &str| format!("the XML ends inside {what}");
    let mut i = 0;
    while i < b.len() {
        let top = open.last().copied().unwrap_or(0);
        if b[i] != b'<' {
            let end = find_byte(b, i, b'<');
            // Text outside the elements doesn't matter.
            if top != 0 {
                budget.spend(end - i)?;
                unescape_into(&mut nodes[top].text, &src[i..end]);
            }
            i = end;
            continue;
        }
        let rest = &src[i..];
        if let Some(r) = rest.strip_prefix("<!--") {
            i += 4 + r.find("-->").ok_or_else(|| ends("a comment"))? + 3;
        } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let k = r.find("]]>").ok_or_else(|| ends("a CDATA section"))?;
            if top != 0 {
                budget.spend(k)?;
                let text = &mut nodes[top].text;
                text.reserve(k);
                let mut lines = r[..k].split('\r');
                text.push_str(lines.next().unwrap_or(""));
                for line in lines {
                    text.push('\n');
                    text.push_str(line.strip_prefix('\n').unwrap_or(line));
                }
            }
            i += 9 + k + 3;
        } else if let Some(r) = rest.strip_prefix("<?") {
            i += 2 + r.find("?>").ok_or_else(|| ends("a processing instruction"))? + 2;
        } else if rest.starts_with("<!") {
            i = skip_declaration(b, i).ok_or_else(|| ends("a declaration"))?;
        } else if let Some(r) = rest.strip_prefix("</") {
            let k = r.find('>').ok_or_else(|| ends("a closing tag"))?;
            // It closes the innermost open element of its name, if that is
            // close by (looking further would make bad files slow).
            let name = r[..k].trim();
            let near = open.len().saturating_sub(64).max(1);
            if let Some(at) = open[near..].iter().rposition(|&n| nodes[n].name == name) {
                open.truncate(near + at);
            }
            i += 2 + k + 1;
        } else if b
            .get(i + 1)
            .is_some_and(|&c| c.is_ascii_alphabetic() || c == b'_' || c == b':' || c >= 0x80)
        {
            i = open_tag(src, i, &mut nodes, &mut open, budget)?;
        } else {
            // A '<' that starts no tag: text.
            if top != 0 {
                nodes[top].text.push('<');
            }
            i += 1;
        }
    }
    if let Some(&top) = open.last()
        && top != 0
    {
        return Err(ends(&format!("<{}>", clip(&nodes[top].name))));
    }
    Ok(nodes)
}

/// Past a `<!DOCTYPE ...>` (or another `<!` declaration) starting at `i`,
/// with any `[...]` and quoted parts in it.
fn skip_declaration(b: &[u8], mut i: usize) -> Option<usize> {
    let (mut depth, mut quote) = (0usize, None);
    i += 2;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'[' => depth += 1,
                b']' => depth = depth.saturating_sub(1),
                b'>' if depth == 0 => return Some(i + 1),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// An opening tag at `start` (`<name attr="value" ...>` or `.../>`): adds
/// its element and returns where the tag ends.
fn open_tag(
    src: &str,
    start: usize,
    nodes: &mut Vec<Node>,
    open: &mut Vec<usize>,
    budget: &mut Budget,
) -> Result<usize, String> {
    let b = src.as_bytes();
    let mut i = start + 1;
    while i < b.len() && !is_space(b[i]) && b[i] != b'/' && b[i] != b'>' {
        i += 1;
    }
    let name = &src[start + 1..i];
    let ends = || format!("the XML ends inside the tag <{}", clip(name));
    let mut attrs = Vec::new();
    let mut cost = NODE_COST + name.len();
    let closed = loop {
        while i < b.len() && is_space(b[i]) {
            i += 1;
        }
        match b.get(i) {
            None => return Err(ends()),
            Some(b'>') => {
                i += 1;
                break false;
            }
            Some(b'/') => {
                i += 1;
                if b.get(i) == Some(&b'>') {
                    i += 1;
                    break true;
                }
            }
            Some(_) => {
                let k = i;
                while i < b.len() && !is_space(b[i]) && !matches!(b[i], b'=' | b'>' | b'/') {
                    i += 1;
                }
                let key = &src[k..i];
                while i < b.len() && is_space(b[i]) {
                    i += 1;
                }
                let mut value = "";
                if b.get(i) == Some(&b'=') {
                    i += 1;
                    while i < b.len() && is_space(b[i]) {
                        i += 1;
                    }
                    match b.get(i) {
                        Some(&q @ (b'"' | b'\'')) => {
                            let end = find_byte(b, i + 1, q);
                            if end == b.len() {
                                return Err(ends());
                            }
                            value = &src[i + 1..end];
                            i = end + 1;
                        }
                        _ => {
                            let v = i;
                            while i < b.len() && !is_space(b[i]) && b[i] != b'>' {
                                i += 1;
                            }
                            value = &src[v..i];
                        }
                    }
                }
                cost += 48 + key.len() + value.len();
                attrs.push((key.to_string(), unescape(value)));
            }
        }
    };
    budget.spend(cost)?;
    let id = nodes.len();
    nodes.push(Node {
        name: name.to_string(),
        attrs,
        ..Node::default()
    });
    let parent = open.last().copied().unwrap_or(0);
    nodes[parent].kids.push(id);
    if !closed {
        open.push(id);
    }
    Ok(i)
}

/// An XML file's elements, read for its items.
struct Xml {
    nodes: Vec<Node>,
    file: RbxFile,
    tree: Budget,
    shared: HashMap<String, Arc<[u8]>>,
    /// Ref values to resolve once every item is known: the item, the
    /// property's index and the referent.
    refs: Vec<(usize, usize, String)>,
}

/// An XML model or place file (its text should be UTF-8; anything else is
/// replaced).
pub fn read_xml(bytes: &[u8], limits: Limits) -> Result<RbxFile, String> {
    if bytes.len() > limits.data {
        return Err(format!("the file is too big: it is more than {} bytes", limits.data));
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut tree = Budget::new(limits.tree, TREE);
    let nodes = parse_xml(text, &mut tree)?;
    let mut x = Xml {
        nodes,
        file: RbxFile::default(),
        tree,
        shared: HashMap::new(),
        refs: Vec::new(),
    };
    let root = x
        .kid(0, "roblox")
        .ok_or("not a Roblox model file: it has no <roblox> element")?;
    x.read(root)?;
    Ok(x.file)
}

impl Xml {
    /// An element's first child with that name.
    fn kid(&self, n: usize, name: &str) -> Option<usize> {
        self.nodes[n].kids.iter().copied().find(|&k| self.nodes[k].name == name)
    }

    fn kids<'a>(&'a self, n: usize, name: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.nodes[n]
            .kids
            .iter()
            .copied()
            .filter(move |&k| self.nodes[k].name == name)
    }

    fn attr(&self, n: usize, name: &str) -> Option<&str> {
        self.nodes[n]
            .attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The text of an element's child, which must be there.
    fn field(&self, n: usize, name: &str) -> Result<&str, String> {
        match self.kid(n, name) {
            Some(k) => Ok(&self.nodes[k].text),
            None => Err(format!("it has no <{name}>")),
        }
    }

    fn f32_field(&self, n: usize, name: &str) -> Result<f32, String> {
        float(self.field(n, name)?)
    }

    fn vector2(&self, n: usize) -> Result<Vector2, String> {
        Ok(Vector2 {
            x: self.f32_field(n, "X")?,
            y: self.f32_field(n, "Y")?,
        })
    }

    fn vector3(&self, n: usize) -> Result<Vector3, String> {
        Ok(Vector3 {
            x: self.f32_field(n, "X")?,
            y: self.f32_field(n, "Y")?,
            z: self.f32_field(n, "Z")?,
        })
    }

    /// A child that holds a Vector2 or Vector3 (`<origin>` in a Ray).
    fn part(&self, n: usize, name: &str) -> Result<usize, String> {
        self.kid(n, name).ok_or_else(|| format!("it has no <{name}>"))
    }

    fn cframe(&self, n: usize) -> Result<CFrame, String> {
        const R: [[&str; 3]; 3] = [["R00", "R01", "R02"], ["R10", "R11", "R12"], ["R20", "R21", "R22"]];
        let mut rotation = [[0.0; 3]; 3];
        for (row, names) in rotation.iter_mut().zip(R) {
            for (v, name) in row.iter_mut().zip(names) {
                *v = self.f32_field(n, name)?;
            }
        }
        Ok(CFrame {
            position: self.vector3(n)?,
            rotation,
        })
    }

    /// A content URI (`<url>`, or `<null>` for none) inside a child (a Font's).
    fn content_uri(&self, n: usize, name: &str) -> Result<String, String> {
        let k = self.part(n, name)?;
        Ok(self
            .kid(k, "url")
            .map(|u| self.nodes[u].text.clone())
            .unwrap_or_default())
    }

    /// The file's shared strings, items and metadata, under `<roblox>`.
    fn read(&mut self, root: usize) -> Result<(), String> {
        let kids = self.nodes[root].kids.clone();
        for &k in &kids {
            match self.nodes[k].name.as_str() {
                "Meta" => {
                    let key = self.attr(k, "name").unwrap_or_default().to_string();
                    let value = self.nodes[k].text.clone();
                    self.tree.spend(48 + key.len() + value.len())?;
                    self.file.meta.push((key, value));
                }
                "SharedStrings" => {
                    for s in self.kids(k, "SharedString").collect::<Vec<_>>() {
                        let key = self.attr(s, "md5").unwrap_or_default().to_string();
                        match base64_decode(self.nodes[s].text.trim()) {
                            Ok(data) => {
                                self.tree.spend(64 + key.len() + data.len())?;
                                self.shared.insert(key, Arc::from(data));
                            }
                            Err(e) => note(&mut self.file.notes, || {
                                format!("skipped shared string {}: {e}", shown(&key))
                            }),
                        }
                    }
                }
                _ => {}
            }
        }
        // Items depth first, in the file's order, without recursion.
        let mut stack: Vec<(usize, Option<usize>)> = kids
            .iter()
            .rev()
            .filter(|&&k| self.nodes[k].name == "Item")
            .map(|&k| (k, None))
            .collect();
        let mut by_ref = HashMap::new();
        while let Some((node, parent)) = stack.pop() {
            let Some(class) = self.attr(node, "class").map(str::to_string) else {
                return Err("an <Item> has no class".into());
            };
            let referent = self.attr(node, "referent").unwrap_or_default().to_string();
            self.tree.spend(ITEM_COST + class.len() + referent.len())?;
            let index = self.file.items.len();
            if !referent.is_empty() && referent != "null" {
                match by_ref.entry(referent.clone()) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(index);
                    }
                    std::collections::hash_map::Entry::Occupied(_) => note(&mut self.file.notes, || {
                        format!("referent {} is used twice; Refs go to the first", shown(&referent))
                    }),
                }
            }
            self.file.items.push(Item {
                class,
                referent,
                parent,
                ..Item::default()
            });
            match parent {
                Some(p) => self.file.items[p].children.push(index),
                None => self.file.roots.push(index),
            }
            for props in self.kids(node, "Properties").collect::<Vec<_>>() {
                for p in self.nodes[props].kids.clone() {
                    self.property(index, p)?;
                }
            }
            let items: Vec<usize> = self.kids(node, "Item").collect();
            stack.extend(items.into_iter().rev().map(|k| (k, Some(index))));
        }
        for (item, prop, key) in std::mem::take(&mut self.refs) {
            let target = by_ref.get(&key).copied();
            if let Some((_, value)) = self.file.items[item].properties.get_mut(prop) {
                match value {
                    PropValue::Ref(r) => *r = target,
                    PropValue::Content(Content::Object(r)) => *r = target,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// One property element of an item's `<Properties>`; one that can't be
    /// read is skipped with a note.
    fn property(&mut self, item: usize, p: usize) -> Result<(), String> {
        let ty = self.nodes[p].name.clone();
        let Some(name) = self.attr(p, "name").map(str::to_string) else {
            let class = clip(&self.file.items[item].class).into_owned();
            note(&mut self.file.notes, || {
                format!("skipped a <{}> of {class} without a name", clip(&ty))
            });
            return Ok(());
        };
        let what = || {
            format!(
                "{}.{} (<{}>)",
                clip(&self.file.items[item].class),
                clip(&name),
                clip(&ty)
            )
        };
        let value = match self.value(p, &ty) {
            Ok(Some(v)) => v,
            Ok(None) => {
                let what = what();
                note(&mut self.file.notes, || format!("skipped {what}: its type isn't known"));
                return Ok(());
            }
            Err(e) => {
                let what = what();
                note(&mut self.file.notes, || format!("skipped {what}: {e}"));
                return Ok(());
            }
        };
        let key = match (&ty[..], &value) {
            ("Ref", _) | ("Content", PropValue::Content(Content::Object(_))) => {
                let text = self.ref_text(p, &ty);
                (text != "null").then(|| text.to_string())
            }
            _ => None,
        };
        self.tree.spend(PROP_COST + name.len())?;
        let props = &mut self.file.items[item].properties;
        if let Some(key) = key {
            self.refs.push((item, props.len(), key));
        }
        props.push((name, value));
        Ok(())
    }

    /// The referent a Ref (or a Content's `<Ref>`) names.
    fn ref_text(&self, p: usize, ty: &str) -> &str {
        let n = if ty == "Ref" { Some(p) } else { self.kid(p, "Ref") };
        n.map_or("null", |n| self.nodes[n].text.trim())
    }

    /// The value of a property element of type `ty`; None for a type this
    /// reader doesn't know. Ref values are resolved later.
    fn value(&self, p: usize, ty: &str) -> Result<Option<PropValue>, String> {
        use PropValue as V;
        let text = self.nodes[p].text.as_str();
        Ok(Some(match ty {
            "string" | "ProtectedString" => V::String(text.as_bytes().to_vec()),
            "BinaryString" => V::String(base64_decode(text.trim())?),
            "bool" => V::Bool(boolean(text)?),
            "int" => V::Int32(int(text)?),
            "int64" => V::Int64(int(text)?),
            "float" => V::Float32(float(text)?),
            "double" => V::Float64(float(text)?),
            "token" => V::Enum(int(text)?),
            "BrickColor" => V::BrickColor(int(text)?),
            "SecurityCapabilities" => V::SecurityCapabilities(int(text)?),
            "Vector2" => V::Vector2(self.vector2(p)?),
            "Vector3" => V::Vector3(self.vector3(p)?),
            "Vector2int16" => V::Vector2int16([int(self.field(p, "X")?)?, int(self.field(p, "Y")?)?]),
            "Vector3int16" => V::Vector3int16([
                int(self.field(p, "X")?)?,
                int(self.field(p, "Y")?)?,
                int(self.field(p, "Z")?)?,
            ]),
            "Color3" => match self.kid(p, "R") {
                Some(_) => V::Color3(Color3 {
                    r: self.f32_field(p, "R")?,
                    g: self.f32_field(p, "G")?,
                    b: self.f32_field(p, "B")?,
                }),
                // Older files pack it into a number, as Color3uint8.
                None => {
                    let [r, g, b] = packed_color(text)?.map(|c| f32::from(c) / 255.0);
                    V::Color3(Color3 { r, g, b })
                }
            },
            "Color3uint8" => V::Color3uint8(packed_color(text)?),
            "CoordinateFrame" => V::CFrame(self.cframe(p)?),
            "OptionalCoordinateFrame" => V::OptionalCFrame(match self.kid(p, "CFrame") {
                Some(k) => Some(self.cframe(k)?),
                None => None,
            }),
            "UDim" => V::UDim(UDim {
                scale: self.f32_field(p, "S")?,
                offset: int(self.field(p, "O")?)?,
            }),
            "UDim2" => V::UDim2(UDim2 {
                x: UDim {
                    scale: self.f32_field(p, "XS")?,
                    offset: int(self.field(p, "XO")?)?,
                },
                y: UDim {
                    scale: self.f32_field(p, "YS")?,
                    offset: int(self.field(p, "YO")?)?,
                },
            }),
            "Ray" => V::Ray(Ray {
                origin: self.vector3(self.part(p, "origin")?)?,
                direction: self.vector3(self.part(p, "direction")?)?,
            }),
            "Rect2D" => V::Rect(Rect {
                min: self.vector2(self.part(p, "min")?)?,
                max: self.vector2(self.part(p, "max")?)?,
            }),
            "Faces" => V::Faces(int::<u8>(self.field(p, "faces")?)? & 63),
            "Axes" => V::Axes(int::<u8>(self.field(p, "axes")?)? & 7),
            "NumberRange" => {
                let v = floats(text, 2)?;
                let (min, max) = match v[..] {
                    [min, max, ..] => (min, max),
                    _ => return Err(format!("{} isn't two numbers", shown(text))),
                };
                V::NumberRange(NumberRange { min, max })
            }
            "NumberSequence" => V::NumberSequence(
                floats(text, 3)?
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|k| NumberKeypoint {
                        time: k[0],
                        value: k[1],
                        envelope: k[2],
                    })
                    .collect(),
            ),
            "ColorSequence" => V::ColorSequence(
                floats(text, 5)?
                    .as_chunks::<5>()
                    .0
                    .iter()
                    .map(|k| ColorKeypoint {
                        time: k[0],
                        color: Color3 {
                            r: k[1],
                            g: k[2],
                            b: k[3],
                        },
                        envelope: k[4],
                    })
                    .collect(),
            ),
            "PhysicalProperties" => {
                if !boolean(self.field(p, "CustomPhysics")?)? {
                    V::PhysicalProperties(None)
                } else {
                    V::PhysicalProperties(Some(CustomPhysics {
                        density: self.f32_field(p, "Density")?,
                        friction: self.f32_field(p, "Friction")?,
                        elasticity: self.f32_field(p, "Elasticity")?,
                        friction_weight: self.f32_field(p, "FrictionWeight")?,
                        elasticity_weight: self.f32_field(p, "ElasticityWeight")?,
                        acoustic_absorption: match self.kid(p, "AcousticAbsorption") {
                            Some(k) => float(&self.nodes[k].text)?,
                            None => 1.0,
                        },
                    }))
                }
            }
            // The old content type, a URI.
            "ContentId" => {
                let url = self.kid(p, "url").map(|u| self.nodes[u].text.clone());
                V::String(url.unwrap_or_default().into_bytes())
            }
            "Content" => {
                let first = self.nodes[p].kids.first().map(|&k| (k, self.nodes[k].name.as_str()));
                V::Content(match first {
                    Some((k, "uri" | "url")) => Content::Uri(self.nodes[k].text.clone()),
                    Some((_, "Ref")) => Content::Object(None),
                    // `null`, and the old `binary` and `hash`, which hold nothing usable.
                    _ => Content::None,
                })
            }
            "Font" => {
                // Some older files have empty Font elements.
                if self.nodes[p].kids.is_empty() {
                    V::Font(Font::default())
                } else {
                    V::Font(Font {
                        family: self.content_uri(p, "Family")?,
                        weight: match self.kid(p, "Weight") {
                            Some(k) => int(&self.nodes[k].text)?,
                            None => 400,
                        },
                        style: match self.kid(p, "Style") {
                            Some(k) if self.nodes[k].text.trim() == "Italic" => 1,
                            _ => 0,
                        },
                        cached_face_id: match self.kid(p, "CachedFaceId") {
                            Some(_) => self.content_uri(p, "CachedFaceId")?,
                            None => String::new(),
                        },
                    })
                }
            }
            "UniqueId" => V::UniqueId(unique_id(text)?),
            "Ref" => V::Ref(None),
            "SharedString" | "NetAssetRef" => {
                let key = text.trim();
                match self.shared.get(key) {
                    Some(s) => V::SharedString(s.clone()),
                    None => return Err(format!("its shared string {} isn't in the file", shown(key))),
                }
            }
            _ => return Ok(None),
        }))
    }
}

fn boolean(s: &str) -> Result<bool, String> {
    match s.trim() {
        t if t.eq_ignore_ascii_case("true") => Ok(true),
        t if t.eq_ignore_ascii_case("false") => Ok(false),
        t => Err(format!("{} isn't true or false", shown(t))),
    }
}

fn int<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    let t = s.trim();
    t.parse()
        .map_err(|_| format!("{} isn't a whole number in range", shown(t)))
}

/// A float as XML files write them, INF and NAN too.
fn float<T: std::str::FromStr + From<f32>>(s: &str) -> Result<T, String> {
    match s.trim() {
        "INF" | "+INF" => Ok(f32::INFINITY.into()),
        "-INF" => Ok(f32::NEG_INFINITY.into()),
        "NAN" | "-NAN" => Ok(f32::NAN.into()),
        t => t.parse().map_err(|_| format!("{} isn't a number", shown(t))),
    }
}

/// Numbers separated by spaces, a multiple of `group` of them.
fn floats(s: &str, group: usize) -> Result<Vec<f32>, String> {
    let v = s
        .split_ascii_whitespace()
        .map(float)
        .collect::<Result<Vec<f32>, String>>()?;
    if !v.len().is_multiple_of(group) {
        return Err(format!("it has {} numbers, not a multiple of {group}", v.len()));
    }
    Ok(v)
}

/// Red, green and blue packed in a number (0xFFRRGGBB).
fn packed_color(s: &str) -> Result<[u8; 3], String> {
    let [_, r, g, b] = int::<u32>(s)?.to_be_bytes();
    Ok([r, g, b])
}

/// 32 hex digits: the random part, the time and the index.
fn unique_id(s: &str) -> Result<UniqueId, String> {
    let t = s.trim();
    if t.len() != 32 || !t.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{} isn't 32 hex digits", shown(t)));
    }
    let hex = |r: Range<usize>| u64::from_str_radix(&t[r], 16).unwrap_or(0);
    Ok(UniqueId {
        random: hex(0..16) as i64,
        time: hex(16..24) as u32,
        index: hex(24..32) as u32,
    })
}
