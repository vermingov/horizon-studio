// Meshes and textures for script mods. A texture is RGBA pixels: read from a
// PNG file in a folder mod, decoded from base64, or made in Lua and painted
// pixel by pixel. A mesh is triangles with texture coordinates, in pixels
// (16 to a block): built from vertices and faces, or from boxes with
// Minecraft's box UV (a skin's layout), or read from a Wavefront .obj or a
// Blockbench .bbmodel (whose textures come with it). Meshes face +z: drawn
// with draw.mesh at a yaw they face that way, and attached to a body part
// their x, y and z are its left, up and forward. GameApi::world_mesh draws
// the triangles, textured, lit by the world where they are.

use super::hstudio::{GameApi, TexRef, WorldTri};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// A texture's longest side.
pub const MAX_SIDE: u32 = 512;
/// Every mod's textures together, in pixels (the game keeps them in its
/// atlas, beside the interface's glyphs).
pub const MAX_TEXELS: usize = 1 << 20;
/// A mesh's triangles.
pub const MAX_TRIANGLES: usize = 65_536;
/// A mod's textures and meshes.
pub const MAX_TEXTURES: usize = 256;
pub const MAX_MESHES: usize = 256;

static KEYS: AtomicU32 = AtomicU32::new(1);

/// A new texture's name in the game's atlas.
fn key() -> u32 {
    KEYS.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub w: u32,
    pub h: u32,
    /// Row by row from the top left, 4 bytes a pixel.
    pub rgba: Vec<u8>,
    /// Its name in the game's atlas, and how many times its pixels changed.
    pub key: u32,
    pub version: u32,
}

impl Texture {
    /// `w` x `h` pixels of `argb` (0xAARRGGBB).
    pub fn new(w: u32, h: u32, argb: u32) -> Result<Texture, String> {
        check_size(w, h)?;
        let [a, r, g, b] = argb.to_be_bytes();
        Ok(Texture {
            w,
            h,
            rgba: [r, g, b, a].repeat((w * h) as usize),
            key: key(),
            version: 0,
        })
    }
    pub fn get(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.w || y >= self.h {
            return None;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let p = &self.rgba[i..i + 4];
        Some(u32::from_be_bytes([p[3], p[0], p[1], p[2]]))
    }
    /// Sets a pixel (false outside the texture). Call `touched` after a change.
    pub fn set(&mut self, x: u32, y: u32, argb: u32) -> bool {
        if x >= self.w || y >= self.h {
            return false;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let [a, r, g, b] = argb.to_be_bytes();
        self.rgba[i..i + 4].copy_from_slice(&[r, g, b, a]);
        true
    }
    /// Its pixels changed: the game uploads them again.
    pub fn touched(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
    pub fn texels(&self) -> usize {
        (self.w * self.h) as usize
    }
}

fn check_size(w: u32, h: u32) -> Result<(), String> {
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("a texture is 1 to {MAX_SIDE} pixels a side (this one {w}x{h})"));
    }
    Ok(())
}

/// A PNG file's pixels, as a texture (at most MAX_SIDE a side).
pub fn decode_png(bytes: &[u8]) -> Result<Texture, String> {
    decode_png_within(bytes, check_size)
}

/// A PNG file's pixels, at most `most` pixels a side and `texels` in all:
/// GUI images (a sprite sheet, a strip of icons), which the image cache
/// scales down for the game's atlas itself.
pub fn decode_png_up_to(bytes: &[u8], most: u32, texels: usize) -> Result<Texture, String> {
    decode_png_within(bytes, |w, h| {
        if w == 0 || h == 0 || w > most || h > most || (w as usize) * (h as usize) > texels {
            return Err(format!(
                "an image is 1 to {most} pixels a side and {texels} in all (this one {w}x{h})"
            ));
        }
        Ok(())
    })
}

/// A PNG file's pixels, if `check` takes its size.
fn decode_png_within(bytes: &[u8], check: impl Fn(u32, u32) -> Result<(), String>) -> Result<Texture, String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = d.read_info().map_err(|e| format!("not a PNG image ({e})"))?;
    let (w, h) = (reader.info().width, reader.info().height);
    check(w, h)?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG image too large")?];
    let out = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("bad PNG image ({e})"))?;
    let px = &buf[..out.buffer_size()];
    let rgba: Vec<u8> = match out.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => px
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&p| [p, p, p, 255]).collect(),
        png::ColorType::Indexed => return Err("unsupported PNG colours".into()),
    };
    if rgba.len() != (w * h * 4) as usize {
        return Err("unsupported PNG layout".into());
    }
    Ok(Texture {
        w,
        h,
        rgba,
        key: key(),
        version: 0,
    })
}

/// Base64 (standard or URL-safe, padded or not, with or without a
/// `data:...;base64,` prefix) as bytes.
pub fn base64(text: &str) -> Result<Vec<u8>, String> {
    let s = text.trim();
    let s = s.find("base64,").map_or(s, |i| &s[i + 7..]);
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return Err(format!("not base64 (it has '{}')", c as char)),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// A texture from bytes: PNG data, or base64 of it.
pub fn texture_from(bytes: &[u8]) -> Result<Texture, String> {
    if bytes.starts_with(b"\x89PNG") {
        return decode_png(bytes);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "neither PNG data nor base64".to_owned())?;
    decode_png(&base64(text)?)
}

// ------------------------------------------------------------------ meshes

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tri {
    /// Corners, in pixels.
    pub p: [[f32; 3]; 3],
    /// Texture coordinates, 0 to 1 across the texture from its top left.
    pub uv: [[f32; 2]; 3],
    /// Its colour, which tints the texture (0xAARRGGBB).
    pub argb: u32,
    /// A texture of the mod's, by id (None: plain colour).
    pub tex: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub tris: Vec<Tri>,
}

impl Mesh {
    /// Its lowest and highest corner, in pixels.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in self.tris.iter().flat_map(|t| t.p) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        if self.tris.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        (lo, hi)
    }
    fn push(&mut self, t: Tri) -> Result<(), String> {
        if self.tris.len() >= MAX_TRIANGLES {
            return Err(format!("a mesh has at most {MAX_TRIANGLES} triangles"));
        }
        self.tris.push(t);
        Ok(())
    }
    /// A four-cornered face as two triangles.
    fn quad(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], argb: u32, tex: Option<u32>) -> Result<(), String> {
        self.push(Tri {
            p: [p[0], p[1], p[2]],
            uv: [uv[0], uv[1], uv[2]],
            argb,
            tex,
        })?;
        self.push(Tri {
            p: [p[0], p[2], p[3]],
            uv: [uv[0], uv[2], uv[3]],
            argb,
            tex,
        })
    }
}

/// A mod's textures and meshes, by id.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub textures: BTreeMap<u32, Texture>,
    pub meshes: BTreeMap<u32, Mesh>,
    next: u32,
}

impl Store {
    pub fn add_texture(&mut self, t: Texture) -> Result<u32, String> {
        if self.textures.len() >= MAX_TEXTURES {
            return Err(format!(
                "a mod has at most {MAX_TEXTURES} textures (texture.free drops one)"
            ));
        }
        self.next += 1;
        self.textures.insert(self.next, t);
        Ok(self.next)
    }
    pub fn add_mesh(&mut self, m: Mesh) -> Result<u32, String> {
        if self.meshes.len() >= MAX_MESHES {
            return Err(format!("a mod has at most {MAX_MESHES} meshes (mesh.free drops one)"));
        }
        self.next += 1;
        self.meshes.insert(self.next, m);
        Ok(self.next)
    }
    pub fn texels(&self) -> usize {
        self.textures.values().map(Texture::texels).sum()
    }
    pub fn clear(&mut self) {
        self.textures.clear();
        self.meshes.clear();
    }
}

// ------------------------------------------------------------------ faces and boxes

/// A box face's corners, top left first then clockwise as seen from
/// outside: Minecraft's block model faces (north is -z, up is +y).
pub fn face_corners(dir: &str, lo: [f32; 3], hi: [f32; 3]) -> Option<[[f32; 3]; 4]> {
    let ([x0, y0, z0], [x1, y1, z1]) = (lo, hi);
    Some(match dir {
        "north" => [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]],
        "south" => [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]],
        "east" => [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]],
        "west" => [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]],
        "up" => [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
        "down" => [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
        _ => return None,
    })
}

pub const FACES: [&str; 6] = ["north", "south", "east", "west", "up", "down"];

/// A face's texture coordinates from a rectangle `[u1, v1, u2, v2]` in
/// texture pixels of a `size` texture, turned `rotation` degrees.
fn face_uv(rect: [f32; 4], size: [f32; 2], rotation: i32) -> [[f32; 2]; 4] {
    let [u1, v1, u2, v2] = rect;
    let s = |u: f32, v: f32| [u / size[0].max(1.0), v / size[1].max(1.0)];
    let base = [s(u1, v1), s(u2, v1), s(u2, v2), s(u1, v2)];
    let turn = (rotation.rem_euclid(360) / 90) as usize;
    std::array::from_fn(|k| base[(k + 4 - turn) % 4])
}

/// Minecraft's box UV (a skin's layout) for a box `size` pixels big at
/// `(u, v)`, as the faces of a box facing `front` (south, +z, for meshes;
/// north for Blockbench, whose models face -z).
pub fn box_uv(dir: &str, u: f32, v: f32, size: [f32; 3], front_south: bool) -> [f32; 4] {
    let (w, h, d) = (size[0], size[1], size[2]);
    // The layout's slots: right side, front, left side, back; top, bottom.
    let slot = |x: f32, y: f32, sw: f32, sh: f32| [u + x, v + y, u + x + sw, v + y + sh];
    let right = slot(0.0, d, d, h);
    let front = slot(d, d, w, h);
    let left = slot(d + w, d, d, h);
    let back = slot(2.0 * d + w, d, w, h);
    let top = slot(d, 0.0, w, d);
    let bottom = slot(d + w, 0.0, w, d);
    match (dir, front_south) {
        ("south", true) | ("north", false) => front,
        ("north", true) | ("south", false) => back,
        // Facing south, the right side is west; facing north, east.
        ("west", true) | ("east", false) => right,
        ("east", true) | ("west", false) => left,
        ("up", _) => top,
        _ => bottom,
    }
}

/// A box of `lo` to `hi` pixels, every face given its rectangle by `uv`
/// (None: the face is left out).
pub fn add_box(
    mesh: &mut Mesh,
    lo: [f32; 3],
    hi: [f32; 3],
    argb: u32,
    tex: Option<u32>,
    size: [f32; 2],
    mut uv: impl FnMut(&str) -> Option<([f32; 4], i32)>,
) -> Result<(), String> {
    for dir in FACES {
        let Some((rect, rotation)) = uv(dir) else { continue };
        let Some(p) = face_corners(dir, lo, hi) else { continue };
        mesh.quad(p, face_uv(rect, size, rotation), argb, tex)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ .obj

/// A Wavefront .obj: its `v`, `vt` and `f` lines (polygons as fans of
/// triangles), positions times `scale` (16: the file is in blocks).
pub fn parse_obj(text: &str, scale: f32, argb: u32, tex: Option<u32>) -> Result<Mesh, String> {
    let mut v: Vec<[f32; 3]> = Vec::new();
    let mut vt: Vec<[f32; 2]> = Vec::new();
    let mut mesh = Mesh::default();
    for (n, line) in text.lines().enumerate() {
        let err = |what: &str| format!("line {}: {what}", n + 1);
        let mut w = line.split_whitespace();
        let nums = |w: std::str::SplitWhitespace<'_>| -> Result<Vec<f32>, String> {
            w.map(|s| s.parse::<f32>().map_err(|_| err(&format!("'{s}' is not a number"))))
                .collect()
        };
        match w.next() {
            Some("v") => {
                let p = nums(w)?;
                if p.len() < 3 {
                    return Err(err("v needs x y z"));
                }
                v.push([p[0] * scale, p[1] * scale, p[2] * scale]);
            }
            Some("vt") => {
                let t = nums(w)?;
                let Some(&u) = t.first() else {
                    return Err(err("vt needs u v"));
                };
                // .obj counts v up from the bottom; textures here, down from the top.
                vt.push([u, 1.0 - t.get(1).copied().unwrap_or(0.0)]);
            }
            Some("f") => {
                let index = |s: &str, len: usize| -> Result<usize, String> {
                    let i: i64 = s.parse().map_err(|_| err(&format!("'{s}' is not an index")))?;
                    let at = if i < 0 { len as i64 + i } else { i - 1 };
                    if at < 0 || at >= len as i64 {
                        return Err(err(&format!("index {i} is not one of the {len} so far")));
                    }
                    Ok(at as usize)
                };
                let mut corners = Vec::new();
                for c in w {
                    let mut parts = c.split('/');
                    let p = index(parts.next().unwrap_or(""), v.len())?;
                    let t = match parts.next() {
                        Some(s) if !s.is_empty() => Some(index(s, vt.len())?),
                        _ => None,
                    };
                    corners.push((p, t));
                }
                if corners.len() < 3 {
                    return Err(err("a face has 3 corners or more"));
                }
                let uv = |c: (usize, Option<usize>)| c.1.map_or([0.0, 0.0], |t| vt[t]);
                for k in 1..corners.len() - 1 {
                    let (a, b, c) = (corners[0], corners[k], corners[k + 1]);
                    mesh.push(Tri {
                        p: [v[a.0], v[b.0], v[c.0]],
                        uv: [uv(a), uv(b), uv(c)],
                        argb,
                        tex,
                    })
                    .map_err(|e| err(&e))?;
                }
            }
            _ => {}
        }
    }
    if mesh.tris.is_empty() {
        return Err("the .obj has no faces (f lines)".into());
    }
    Ok(mesh)
}

// ------------------------------------------------------------------ .bbmodel

/// A Blockbench model: its mesh, whose triangles' `tex` index `textures`.
#[derive(Debug)]
pub struct Imported {
    pub mesh: Mesh,
    pub textures: Vec<Option<Texture>>,
}

type M3 = [[f32; 3]; 3];

fn rot_x(t: f32) -> M3 {
    let (s, c) = t.to_radians().sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}
fn rot_y(t: f32) -> M3 {
    let (s, c) = t.to_radians().sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}
fn rot_z(t: f32) -> M3 {
    let (s, c) = t.to_radians().sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}
/// Rows times a vector.
fn apply(m: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}
fn times(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
/// Blockbench's euler angles (degrees; three.js's ZYX order: x turns first).
fn euler(r: [f32; 3]) -> M3 {
    times(&times(&rot_z(r[2]), &rot_y(r[1])), &rot_x(r[0]))
}

/// A turn of `r` degrees about `origin`.
#[derive(Clone, Copy)]
struct Pivot {
    origin: [f32; 3],
    m: M3,
}

impl Pivot {
    fn turn(&self, p: [f32; 3]) -> [f32; 3] {
        let d = [p[0] - self.origin[0], p[1] - self.origin[1], p[2] - self.origin[2]];
        let r = apply(&self.m, d);
        [r[0] + self.origin[0], r[1] + self.origin[1], r[2] + self.origin[2]]
    }
}

fn vec3(v: &Json) -> Option<[f32; 3]> {
    let a = v.as_array()?;
    Some([
        a.first()?.as_f64()? as f32,
        a.get(1)?.as_f64()? as f32,
        a.get(2)?.as_f64()? as f32,
    ])
}

fn pivot(v: &Json) -> Pivot {
    Pivot {
        origin: vec3(&v["origin"]).unwrap_or([0.0; 3]),
        m: euler(vec3(&v["rotation"]).unwrap_or([0.0; 3])),
    }
}

/// The groups each element is in (innermost first), from the outliner.
fn groups(items: &Json, chain: &[Pivot], out: &mut BTreeMap<String, Vec<Pivot>>) {
    let Some(items) = items.as_array() else { return };
    for item in items {
        match item {
            Json::String(uuid) => {
                out.insert(uuid.clone(), chain.iter().rev().copied().collect());
            }
            Json::Object(_) => {
                let mut inner = chain.to_vec();
                inner.push(pivot(item));
                groups(&item["children"], &inner, out);
            }
            _ => {}
        }
    }
}

/// A Blockbench .bbmodel (its cubes, meshes, groups and embedded
/// textures). `flip` turns it half round so that its front, which faces
/// north (-z) in Blockbench, faces +z as meshes do here.
pub fn parse_bbmodel(text: &str, flip: bool) -> Result<Imported, String> {
    let j: Json = serde_json::from_str(text).map_err(|e| format!("not a .bbmodel (JSON: {e})"))?;
    let res = [
        j["resolution"]["width"].as_f64().unwrap_or(16.0) as f32,
        j["resolution"]["height"].as_f64().unwrap_or(16.0) as f32,
    ];
    let mut textures = Vec::new();
    let mut sizes = Vec::new();
    for (i, t) in j["textures"].as_array().into_iter().flatten().enumerate() {
        let tex = match t["source"].as_str() {
            Some(src) if src.contains("base64,") => {
                Some(decode_png(&base64(src)?).map_err(|e| format!("textures[{}]: {e}", i + 1))?)
            }
            _ => None,
        };
        sizes.push([
            t["uv_width"].as_f64().map_or(res[0], |v| v as f32),
            t["uv_height"].as_f64().map_or(res[1], |v| v as f32),
        ]);
        textures.push(tex);
    }
    let mut chains = BTreeMap::new();
    groups(&j["outliner"], &[], &mut chains);
    let mut mesh = Mesh::default();
    for (n, e) in j["elements"].as_array().into_iter().flatten().enumerate() {
        if e["visibility"] == Json::Bool(false) {
            continue;
        }
        let err = |what: &str| format!("elements[{}]: {what}", n + 1);
        let own = pivot(e);
        let chain = e["uuid"]
            .as_str()
            .and_then(|u| chains.get(u))
            .cloned()
            .unwrap_or_default();
        let place = |p: [f32; 3]| {
            let p = chain.iter().fold(own.turn(p), |p, g| g.turn(p));
            if flip { [-p[0], p[1], -p[2]] } else { p }
        };
        let tex_of = |face: &Json| -> Option<Option<u32>> {
            match &face["texture"] {
                Json::Null if face.get("texture").is_some() => None,
                Json::Number(i) => Some(i.as_u64().map(|i| i as u32)),
                _ => Some((!textures.is_empty()).then_some(0)),
            }
        };
        let size_of = |tex: Option<u32>| tex.and_then(|i| sizes.get(i as usize).copied()).unwrap_or(res);
        match e["type"].as_str().unwrap_or("cube") {
            "cube" => {
                let (Some(from), Some(to)) = (vec3(&e["from"]), vec3(&e["to"])) else {
                    return Err(err("a cube needs from and to"));
                };
                let g = e["inflate"].as_f64().unwrap_or(0.0) as f32;
                let lo = std::array::from_fn(|i| from[i].min(to[i]) - g);
                let hi = std::array::from_fn(|i| from[i].max(to[i]) + g);
                let boxed = e["box_uv"].as_bool().unwrap_or(false);
                let offset = e["uv_offset"].as_array().map_or([0.0, 0.0], |a| {
                    [
                        a.first().and_then(Json::as_f64).unwrap_or(0.0) as f32,
                        a.get(1).and_then(Json::as_f64).unwrap_or(0.0) as f32,
                    ]
                });
                let dims = [to[0] - from[0], to[1] - from[1], to[2] - from[2]].map(f32::abs);
                for dir in FACES {
                    let face = &e["faces"][dir];
                    if face.is_null() {
                        continue;
                    }
                    let Some(tex) = tex_of(face) else { continue };
                    let rect = match face["uv"].as_array() {
                        Some(a) if a.len() == 4 => std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32),
                        _ if boxed => box_uv(dir, offset[0], offset[1], dims, false),
                        _ => [0.0, 0.0, 0.0, 0.0],
                    };
                    let Some(p) = face_corners(dir, lo, hi) else { continue };
                    let rotation = face["rotation"].as_i64().unwrap_or(0) as i32;
                    let uv = face_uv(rect, size_of(tex), rotation);
                    mesh.quad(p.map(place), uv, 0xFFFF_FFFF, tex).map_err(|x| err(&x))?;
                }
            }
            "mesh" => {
                let Some(verts) = e["vertices"].as_object() else {
                    return Err(err("a mesh needs vertices"));
                };
                for face in e["faces"].as_object().into_iter().flat_map(|f| f.values()) {
                    let Some(tex) = tex_of(face) else { continue };
                    let size = size_of(tex);
                    let keys: Vec<&str> = face["vertices"]
                        .as_array()
                        .map(|a| a.iter().filter_map(Json::as_str).collect())
                        .unwrap_or_default();
                    let corner = |k: &str| -> Option<([f32; 3], [f32; 2])> {
                        let p = vec3(verts.get(k)?)?;
                        let uv = face["uv"][k].as_array().map_or([0.0, 0.0], |a| {
                            [
                                a.first().and_then(Json::as_f64).unwrap_or(0.0) as f32 / size[0],
                                a.get(1).and_then(Json::as_f64).unwrap_or(0.0) as f32 / size[1],
                            ]
                        });
                        // A mesh's vertices are relative to its origin.
                        let o = own.origin;
                        Some((place(own.turn([p[0] + o[0], p[1] + o[1], p[2] + o[2]])), uv))
                    };
                    let cs: Vec<_> = keys.iter().filter_map(|k| corner(k)).collect();
                    match cs.len() {
                        3 => mesh
                            .push(Tri {
                                p: [cs[0].0, cs[1].0, cs[2].0],
                                uv: [cs[0].1, cs[1].1, cs[2].1],
                                argb: 0xFFFF_FFFF,
                                tex,
                            })
                            .map_err(|x| err(&x))?,
                        4 => {
                            let o = quad_order([cs[0].0, cs[1].0, cs[2].0, cs[3].0]);
                            let q = o.map(|i| cs[i]);
                            mesh.quad(q.map(|c| c.0), q.map(|c| c.1), 0xFFFF_FFFF, tex)
                                .map_err(|x| err(&x))?;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if mesh.tris.is_empty() {
        return Err("the .bbmodel has no faces to draw".into());
    }
    Ok(Imported { mesh, textures })
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// An order of four corners that goes round them (Blockbench keeps a
/// quad's corners in no particular order).
fn quad_order(p: [[f32; 3]; 4]) -> [usize; 4] {
    for o in [[0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3]] {
        let n1 = cross(sub(p[o[1]], p[o[0]]), sub(p[o[2]], p[o[0]]));
        let n2 = cross(sub(p[o[2]], p[o[0]]), sub(p[o[3]], p[o[0]]));
        if dot(n1, n2) > 0.0 {
            return o;
        }
    }
    [0, 1, 2, 3]
}

// ------------------------------------------------------------------ drawing

/// Multiplies two colours channel by channel (a tint).
pub fn tint(a: u32, b: u32) -> u32 {
    let ch = |s: u32| ((((a >> s) & 0xFF) * ((b >> s) & 0xFF) + 127) / 255) << s;
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// Draws `mesh`, its pixels placed in the world by `place`, its textures
/// from `store` (or `texture` for every triangle), tinted, lit at
/// `light_at` (None: full bright). `budget` is the world's quads left this
/// frame, one a triangle; false once it ran out.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    game: &mut dyn GameApi,
    store: &Store,
    mesh: &Mesh,
    place: impl Fn([f32; 3]) -> [f64; 3],
    argb: u32,
    texture: Option<Option<u32>>,
    light_at: Option<[f64; 3]>,
    budget: &mut usize,
) -> bool {
    let mut groups: BTreeMap<Option<u32>, Vec<WorldTri>> = BTreeMap::new();
    let mut whole = true;
    for t in &mesh.tris {
        if *budget == 0 {
            whole = false;
            break;
        }
        *budget -= 1;
        let p = t.p.map(&place);
        let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let tex = texture.unwrap_or(t.tex).filter(|id| store.textures.contains_key(id));
        groups.entry(tex).or_default().push(WorldTri {
            p,
            uv: t.uv,
            argb: tint(t.argb, argb),
            normal,
        });
    }
    for (tex, tris) in &groups {
        let texture = tex.and_then(|id| store.textures.get(&id)).map(|t| TexRef {
            key: t.key,
            version: t.version,
            w: t.w,
            h: t.h,
            rgba: &t.rgba,
        });
        game.world_mesh(tris, texture, light_at);
    }
    whole
}
