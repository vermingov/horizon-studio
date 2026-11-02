// Natrium's switches (as `horizon_core::natrium`), in the crate every part
// it speeds up can read: the renderer's culling and the server's ticks. The
// game sets them from Natrium's settings; each optimization checks its own.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

static PARTICLES: AtomicBool = AtomicBool::new(false);
static FOG: AtomicBool = AtomicBool::new(false);
static TICKS: AtomicBool = AtomicBool::new(false);

/// This frame's fog end past which terrain is all fog colour on a fog
/// coloured background (infinity: none).
static FOG_CUT: AtomicU32 = AtomicU32::new(0x7F80_0000);

/// What Video Settings shows of Natrium (`set_video`): 0 while it is off in
/// Options > Mods.
static VIDEO: AtomicU64 = AtomicU64::new(0);

/// How much each optimization skipped, for Natrium's page (counts since start).
static PARTICLES_SKIPPED: AtomicU64 = AtomicU64::new(0);
static SECTIONS_SKIPPED: AtomicU64 = AtomicU64::new(0);
static TICK_WORK_SAVED: AtomicU64 = AtomicU64::new(0);

/// Natrium's switches, all at once.
pub fn set(particles: bool, fog: bool, ticks: bool) {
    PARTICLES.store(particles, Ordering::Relaxed);
    FOG.store(fog, Ordering::Relaxed);
    TICKS.store(ticks, Ordering::Relaxed);
}

/// Particles outside the view aren't drawn.
pub fn particles() -> bool {
    PARTICLES.load(Ordering::Relaxed)
}

/// Chunk sections wholly inside the fog aren't drawn.
pub fn fog() -> bool {
    FOG.load(Ordering::Relaxed)
}

/// The server's ticks take the faster paths (same results).
pub fn ticks() -> bool {
    TICKS.load(Ordering::Relaxed)
}

/// Sets this frame's fog end for culling (`None`: nothing to cut).
pub fn set_fog_cut(end: Option<f32>) {
    let end = end.filter(|e| e.is_finite() && *e > 0.0).unwrap_or(f32::INFINITY);
    FOG_CUT.store(end.to_bits(), Ordering::Relaxed);
}

/// The world's terrain isn't drawn at all (a Roblox script's
/// RunService:Set3dRenderingEnabled(false), through Horizon Studio): the
/// same culling, of every section but the camera's.
static WORLD_HIDDEN: AtomicBool = AtomicBool::new(false);

pub fn set_world_hidden(hidden: bool) {
    WORLD_HIDDEN.store(hidden, Ordering::Relaxed);
}

pub fn world_hidden() -> bool {
    WORLD_HIDDEN.load(Ordering::Relaxed)
}

/// The fog end to cull sections past, when fog culling applies this frame.
pub fn fog_cut() -> Option<f32> {
    let end = f32::from_bits(FOG_CUT.load(Ordering::Relaxed));
    (fog() && end.is_finite()).then_some(end)
}

/// Whether a camera-relative 16-block box `min..min+16` is wholly past `end`.
pub fn past(min: [f32; 3], end: f32) -> bool {
    past_box(min, min.map(|m| m + 16.0), end)
}

/// Whether the camera-relative box `min..max` is wholly past `end`: its
/// nearest point to the camera is.
pub fn past_box(min: [f32; 3], max: [f32; 3], end: f32) -> bool {
    let near: [f32; 3] = std::array::from_fn(|i| {
        if min[i] > 0.0 {
            min[i]
        } else if max[i] < 0.0 {
            max[i]
        } else {
            0.0
        }
    });
    // A block's margin: fog is linear up to `end`, so only past it is it whole.
    length_squared(near) >= (end + 1.0) * (end + 1.0)
}

/// Whether some of the camera-relative box `min..max` may be past `end`: its
/// farthest point from the camera is. A box that isn't holds no section
/// `past` would cut.
pub fn reaches(min: [f32; 3], max: [f32; 3], end: f32) -> bool {
    let far: [f32; 3] = std::array::from_fn(|i| if min[i].abs() > max[i].abs() { min[i] } else { max[i] });
    length_squared(far) >= (end + 1.0) * (end + 1.0)
}

/// How many sections a box `min..max` stands for: its height in sections (a
/// section's own box, even a tight one, is one; a chunk column's, its height).
pub fn sections_in(min: [f32; 3], max: [f32; 3]) -> u64 {
    ((max[1] - min[1]) / 16.0).round().max(1.0) as u64
}

fn length_squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// Natrium's switches as its settings have them (Natrium, particle culling,
/// fog culling, faster ticks, block face culling), for Video Settings.
pub type Switches = [bool; 5];

/// Video Settings' Natrium section: Natrium's place in Options > Mods (its
/// buttons are that page's) and its switches; `None` hides it.
pub fn set_video(row: Option<(usize, Switches)>) {
    let bits = row.map_or(0, |(index, switches)| {
        let on = switches
            .iter()
            .enumerate()
            .fold(0u64, |bits, (i, &on)| bits | (u64::from(on) << i));
        1 << 63 | (index as u64 & 0xFFFF_FFFF) << 8 | on
    });
    VIDEO.store(bits, Ordering::Relaxed);
}

pub fn video() -> Option<(usize, Switches)> {
    let bits = VIDEO.load(Ordering::Relaxed);
    (bits >> 63 == 1).then(|| {
        let index = (bits >> 8 & 0xFFFF_FFFF) as usize;
        (index, std::array::from_fn(|i| bits >> i & 1 == 1))
    })
}

pub fn skipped_particles(n: u64) {
    PARTICLES_SKIPPED.fetch_add(n, Ordering::Relaxed);
}

pub fn skipped_sections(n: u64) {
    SECTIONS_SKIPPED.fetch_add(n, Ordering::Relaxed);
}

pub fn saved_tick_work(n: u64) {
    TICK_WORK_SAVED.fetch_add(n, Ordering::Relaxed);
}

/// The switches' captions, in their buttons' order on Natrium's page and in
/// Video Settings.
pub const CAPTIONS: [&str; 5] = [
    "Natrium",
    "Particle Culling",
    "Fog Culling",
    "Faster Ticks",
    "Block Face Culling",
];
/// The preset's button, after the switches.
pub const PRESET: usize = 5;
pub const PRESET_LABEL: &str = "Apply Natrium Preset";

/// What Natrium has skipped since the game started, in a line.
pub fn skipped() -> String {
    let (p, s, t) = counts();
    format!(
        "Skipped so far: {} particles, {} sections, {} tick lookups",
        short(p),
        short(s),
        short(t)
    )
}

/// A count in 4 characters or so: 999, 1.2k, 3.4M, 5.6G.
pub fn short(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.1}G", n as f64 / 1e9),
    }
}

/// (particles, sections, tick work) skipped so far.
pub fn counts() -> (u64, u64, u64) {
    (
        PARTICLES_SKIPPED.load(Ordering::Relaxed),
        SECTIONS_SKIPPED.load(Ordering::Relaxed),
        TICK_WORK_SAVED.load(Ordering::Relaxed),
    )
}
