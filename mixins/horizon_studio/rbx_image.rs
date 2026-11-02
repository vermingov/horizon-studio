// Where Roblox's images come from, and the size scripts measure them in.
//
// `rbxasset://` content is a file of Roblox's client (its content folder):
// the public mirror of it (MaximumADHD's Roblox-Client-Tracker) serves the
// file itself. An asset id's image comes from the thumbnails API, which
// scales every image to fit its box, smaller ones up; scripts crop
// (ImageRectOffset, ImageRectSize) and slice (SliceCenter) in the
// original's pixels, so the original's size is worked out from the
// thumbnails themselves (see `effective_size`) and from how scripts use
// the image (see `native_size`).

use super::hstudio_meshes::Texture;
use std::path::Path;

/// Whether `api` (from `roblox_image`) is the thumbnails API, whose answer
/// names the image, rather than the image itself.
pub fn is_thumbnail(api: &str) -> bool {
    api.starts_with("https://thumbnails.roblox.com/")
}

/// A GET of `url`, if it succeeds.
fn get_ok(url: &str) -> Option<super::hstudio_net::Response> {
    super::hstudio_net::fetch(
        &super::hstudio_net::Request {
            url: url.to_owned(),
            method: "GET".into(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        false,
    )
    .ok()
    .filter(super::hstudio_net::Response::success)
}

/// Writes `bytes` to `path` whole (through a part file): whether it did.
fn write_whole(path: &Path, bytes: &[u8]) -> bool {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = std::path::PathBuf::from(tmp);
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
    std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_ok()
}

/// The address of the image the thumbnails API names for `api`, asking
/// again while the thumbnail is Pending (not made yet).
fn thumbnail_url(api: &str) -> Option<String> {
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let Some(answer) = get_ok(api) else { continue };
        let j = serde_json::from_slice::<serde_json::Value>(&answer.body).ok()?;
        let item = &j["data"][0];
        match (item["state"].as_str(), item["imageUrl"].as_str()) {
            (Some("Completed"), Some(url)) => return Some(url.to_owned()),
            (Some("Pending"), _) => continue,
            _ => return None,
        }
    }
    None
}

/// Fetches a Roblox image (`api` and the file from `roblox_image`: the
/// thumbnail the thumbnails API names, or a file of Roblox's from its
/// mirror) into `path`: whether it is there.
pub fn fetch(api: &str, path: &Path) -> bool {
    let url = if is_thumbnail(api) {
        match thumbnail_url(api) {
            Some(u) => u,
            None => return false,
        }
    } else {
        api.to_owned()
    };
    get_ok(&url).is_some_and(|png| write_whole(path, &png.body))
}

/// The file beside a thumbnail at `path` that keeps its resolution.
pub fn size_file(path: &Path) -> std::path::PathBuf {
    path.with_extension("size")
}

/// The resolution kept for the thumbnail at `path` (`find_resolution`):
/// None when not worked out yet, Some(None) when it couldn't be told.
pub fn kept_resolution(path: &Path) -> Option<Option<u32>> {
    let s = std::fs::read_to_string(size_file(path)).ok()?;
    let n: u32 = s.trim().parse().ok()?;
    Some((n > 0).then_some(n))
}

/// Works out the resolution of an asset's image (`effective_size`) from its
/// 700-pixel thumbnail at `path` and the same thumbnail at 1024 (the
/// thumbnails' server makes it at either size), and keeps it beside the
/// thumbnail (the long side, or 0 when it can't be told). None when the
/// larger thumbnail can't be had (and nothing is kept, so a later session
/// asks again).
pub fn find_resolution(api: &str, path: &Path) -> Option<Option<u32>> {
    let small = super::hstudio_meshes::decode_png_up_to(&std::fs::read(path).ok()?, 2048, 1 << 22).ok()?;
    let url = thumbnail_url(api)?;
    let large = url
        .contains("/700/700/")
        .then(|| url.replacen("/700/700/", "/1024/1024/", 1))
        .and_then(|u| get_ok(&u))
        .and_then(|r| super::hstudio_meshes::decode_png_up_to(&r.body, 2048, 1 << 22).ok())?;
    let n = effective_size(&[&small, &large]);
    write_whole(&size_file(path), n.unwrap_or(0).to_string().as_bytes());
    Some(n)
}

/// The Client Tracker's copy of Roblox's content folder.
const MIRROR: &str = "https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/";

/// Where a `rbxasset://<path>` file is fetched from, and the file it is
/// kept in under the workspace. Paths are Roblox's (`textures/...`,
/// `studio_svg_textures/...`, which the mirror spells
/// `studio-svg-textures`), backslashes read as slashes; None for one that
/// climbs out (`..`) or names nothing. `rbxasset://horizon-studio/` is the
/// workspace's own files (getcustomasset), not Roblox's.
pub fn rbxasset_source(content: &str) -> Option<(String, String)> {
    let rest = content.trim();
    let rest = rest
        .get(..11)
        .filter(|p| p.eq_ignore_ascii_case("rbxasset://"))
        .map(|_| &rest[11..])?;
    let path = rest.replace('\\', "/");
    let path = path.trim_start_matches('/');
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty()
        || parts.iter().any(|p| *p == ".." || p.contains(':'))
        || parts[0].eq_ignore_ascii_case("horizon-studio")
    {
        return None;
    }
    let path = parts.join("/");
    let remote = match parts[0] {
        "studio_svg_textures" => format!("studio-svg-textures{}", &path["studio_svg_textures".len()..]),
        _ => path.clone(),
    };
    let url = format!("{MIRROR}{}", remote.replace(' ', "%20"));
    Some((url, format!(".roblox-assets/rbxasset/{path}")))
}

/// The lowest and highest frequency (cycles a pixel) the resampling's
/// trace is looked for at: below, an image's own shapes drown it; above,
/// it nears the Nyquist limit, where every phase pattern folds.
const LO: f64 = 0.1;
const HI: f64 = 0.47;
/// The sizes (the long side, in pixels) an original is looked for at.
const SMALLEST: u32 = 64;
const LARGEST: u32 = 2048;
/// How far above the spectrum's median the trace must stand to be taken.
const SURE: f64 = 6.0;

/// The image's sharpness along one axis: at each column (or row), the
/// second difference of its premultiplied pixels, summed over the colours
/// and averaged down the other axis.
fn profile(t: &Texture, columns: bool) -> Vec<f64> {
    let (w, h) = (t.w as usize, t.h as usize);
    let (n, across) = if columns { (w, h) } else { (h, w) };
    if n < 3 || across == 0 {
        return Vec::new();
    }
    let px = |x: usize, y: usize| -> [f64; 4] {
        let i = (y * w + x) * 4;
        let a = f64::from(t.rgba[i + 3]) / 255.0;
        [
            f64::from(t.rgba[i]) * a,
            f64::from(t.rgba[i + 1]) * a,
            f64::from(t.rgba[i + 2]) * a,
            a * 255.0,
        ]
    };
    let mut out = vec![0.0; n - 2];
    for j in 0..across {
        let at = |i: usize| if columns { px(i, j) } else { px(j, i) };
        let (mut a, mut b) = (at(0), at(1));
        for (i, o) in out.iter_mut().enumerate() {
            let c = at(i + 2);
            *o += (0..4).map(|k| (a[k] - 2.0 * b[k] + c[k]).abs()).sum::<f64>();
            (a, b) = (b, c);
        }
    }
    out
}

/// A spectrum between LO and HI: (bins per cycle, the first bin's index,
/// the magnitudes).
type Spectrum = (usize, usize, Vec<f64>);

/// The magnitude of a profile's spectrum between LO and HI, four bins to
/// each of its own (zero-padded), over the band's median.
fn spectrum(s: &[f64]) -> Option<Spectrum> {
    let n = s.len();
    if n < 16 {
        return None;
    }
    let mean = s.iter().sum::<f64>() / n as f64;
    let tau = std::f64::consts::TAU;
    let x: Vec<f64> = s
        .iter()
        .enumerate()
        .map(|(i, v)| (v - mean) * (0.5 - 0.5 * (tau * i as f64 / (n - 1) as f64).cos()))
        .collect();
    let m = 4 * n;
    let (cos, sin): (Vec<f64>, Vec<f64>) = (0..m)
        .map(|k| {
            let a = tau * k as f64 / m as f64;
            (a.cos(), a.sin())
        })
        .unzip();
    let first = (LO * m as f64).ceil() as usize;
    let last = (HI * m as f64).floor() as usize;
    let mut mag: Vec<f64> = (first..=last)
        .map(|k| {
            let (mut re, mut im) = (0.0, 0.0);
            let mut at = 0;
            for v in &x {
                re += v * cos[at];
                im -= v * sin[at];
                at = (at + k) % m;
            }
            (re * re + im * im).sqrt()
        })
        .collect();
    let mut sorted = mag.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    if median <= 0.0 {
        return None;
    }
    for v in &mut mag {
        *v /= median;
    }
    Some((m, first, mag))
}

/// A thumbnail's two spectra (columns, rows) with its width and height.
struct Traced {
    sides: [(f64, Option<Spectrum>); 2],
    long: f64,
}

impl Traced {
    fn new(t: &Texture) -> Traced {
        Traced {
            sides: [
                (f64::from(t.w), spectrum(&profile(t, true))),
                (f64::from(t.h), spectrum(&profile(t, false))),
            ],
            long: f64::from(t.w.max(t.h)),
        }
    }
}

/// How strongly the thumbnails show the trace an original `n` pixels on
/// its long side leaves when scaled to them: the spectrum at that
/// resampling's phase frequency (frac(n / side), folded), averaged over
/// each thumbnail's columns and rows. A thumbnail of the original's own
/// size shows none and isn't counted; a frequency outside the band counts
/// as the median (no sign either way).
fn trace(thumbs: &[Traced], n: u32) -> f64 {
    let (mut sum, mut count) = (0.0, 0.0);
    for t in thumbs {
        for (side, spec) in &t.sides {
            let Some((m, first, mag)) = spec else { continue };
            let nn = f64::from(n) * side / t.long;
            if (nn - side).abs() < 1.0 {
                continue;
            }
            let f = (nn / side).fract();
            let f = f.min(1.0 - f);
            let v = if (LO..=HI).contains(&f) {
                let at = (f * *m as f64).round() as usize;
                let lo = at.saturating_sub(2).max(*first) - first;
                let hi = (at + 2).saturating_sub(*first).min(mag.len() - 1);
                mag[lo.min(hi)..=hi].iter().copied().fold(0.0, f64::max)
            } else {
                1.0
            };
            sum += v;
            count += 1.0;
        }
    }
    if count > 0.0 { sum / count } else { 0.0 }
}

/// The resolution an image's thumbnails were made from: the long side, in
/// pixels, of the image whose pixels they hold, when they show it surely.
///
/// A thumbnail is the original resampled, and resampling leaves a trace:
/// each thumbnail pixel falls at a phase between the original's pixels
/// that repeats with a frequency of frac(original / thumbnail) a pixel,
/// and the image's sharpness swings with it. One thumbnail's trace names
/// the original up to its aliases (n, side - n, side + n...); two of
/// different sizes (700 and 1024) agree only on the original. The trace
/// is the pixels' resolution: an image drawn at twice its size with each
/// pixel doubled reads as the smaller, so a script's crops can show the
/// original to be a whole multiple of it (`native_size`). None for
/// images whose pixels show no trace: very smooth ones, tiny ones (under
/// SMALLEST), and ones exactly a thumbnail's size.
pub fn effective_size(thumbs: &[&Texture]) -> Option<u32> {
    let traced: Vec<Traced> = thumbs.iter().map(|t| Traced::new(t)).collect();
    let scores: Vec<f64> = (SMALLEST..=LARGEST).map(|n| trace(&traced, n)).collect();
    let best = scores.iter().copied().fold(0.0, f64::max);
    if best < SURE {
        return None;
    }
    // The best peak; of peaks all but tied with it, the smallest (an alias
    // that an image's own repeating shapes lift can come close).
    let peak = |i: usize| {
        let lo = i.saturating_sub(3);
        let hi = (i + 3).min(scores.len() - 1);
        scores[lo..=hi].iter().all(|&s| s <= scores[i])
    };
    (0..scores.len())
        .find(|&i| scores[i] >= 0.98 * best && peak(i))
        .map(|i| SMALLEST + i as u32)
}

/// How scripts use an image in its pixels: the farthest any crop
/// (ImageRectOffset + ImageRectSize) or slice (SliceCenter) reaches, and
/// for sliced images the size a symmetric SliceCenter implies (its insets
/// alike on both sides).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Uses {
    pub reach: [f64; 2],
    pub cropped: bool,
    pub slice: Option<[f64; 2]>,
}

impl Uses {
    /// A crop `offset` + `size` (pixels).
    pub fn crop(&mut self, offset: [f64; 2], size: [f64; 2]) {
        if size[0] > 0.0 && size[1] > 0.0 {
            self.cropped = true;
            for i in 0..2 {
                self.reach[i] = self.reach[i].max(offset[i].max(0.0) + size[i]);
            }
        }
    }

    /// A SliceCenter from `min` to `max` (pixels).
    pub fn slice(&mut self, min: [f64; 2], max: [f64; 2]) {
        if max[0] > min[0] && max[1] > min[1] {
            let s = self.slice.get_or_insert([0.0; 2]);
            for i in 0..2 {
                self.reach[i] = self.reach[i].max(max[i]);
                s[i] = s[i].max(max[i] + min[i].max(0.0));
            }
        }
    }

    /// These uses and `other`'s together.
    pub fn merge(&mut self, other: &Uses) {
        self.cropped |= other.cropped;
        for i in 0..2 {
            self.reach[i] = self.reach[i].max(other.reach[i]);
        }
        if let Some(o) = other.slice {
            let s = self.slice.get_or_insert([0.0; 2]);
            for i in 0..2 {
                s[i] = s[i].max(o[i]);
            }
        }
    }
}

/// The size (pixels) scripts measure a thumbnail's image in: the
/// original's, as best it can be told. `thumb` is the thumbnail's size,
/// `effective` its resolution (`effective_size`) if known.
///
/// - Cropped, with the resolution known: the smallest whole multiple of it
///   the crops fit in (a sheet of pixel-doubled icons reads as half its
///   size until a crop reaches past that half).
/// - Sliced only: the size its SliceCenter's insets, alike on both sides,
///   imply (slices are mostly small smooth images, which show no trace).
/// - Otherwise the thumbnail's own size, made larger only as far as the
///   crops reach past it.
///
/// Every size keeps the thumbnail's shape, which is the original's.
pub fn native_size(thumb: (u32, u32), effective: Option<u32>, uses: &Uses) -> (f64, f64) {
    let (w, h) = (f64::from(thumb.0.max(1)), f64::from(thumb.1.max(1)));
    let long = w.max(h);
    // The size whose long side is `n`, in the thumbnail's shape.
    let shaped = |n: f64| ((n * w / long).round().max(1.0), (n * h / long).round().max(1.0));
    // The long side that makes the size at least `need` on both axes.
    let reaching = |need: [f64; 2]| (need[0] * long / w).max(need[1] * long / h);
    let fits = |s: (f64, f64)| uses.reach[0] <= s.0 + 0.5 && uses.reach[1] <= s.1 + 0.5;
    if uses.cropped
        && let Some(d) = effective
    {
        let d = f64::from(d);
        let mut k = 1.0;
        while k < 64.0 && !fits(shaped(d * k)) {
            k += 1.0;
        }
        return shaped(d * k);
    }
    if !uses.cropped
        && let Some(s) = uses.slice
    {
        return shaped(reaching(s));
    }
    if fits((w, h)) {
        (w, h)
    } else {
        shaped(reaching(uses.reach))
    }
}
