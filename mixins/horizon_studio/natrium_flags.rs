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

/// The fog end to cull sections past, when fog culling applies this frame.
pub fn fog_cut() -> Option<f32> {
    let end = f32::from_bits(FOG_CUT.load(Ordering::Relaxed));
    (fog() && end.is_finite()).then_some(end)
}

/// Whether a camera-relative 16-block box `min..min+16` is wholly past `end`.
pub fn past(min: [f32; 3], end: f32) -> bool {
    let near = min.map(|m| {
        if m > 0.0 {
            m
        } else if m + 16.0 < 0.0 {
            m + 16.0
        } else {
            0.0
        }
    });
    let d2 = near[0] * near[0] + near[1] * near[1] + near[2] * near[2];
    // A block's margin: fog is linear up to `end`, so only past it is it whole.
    d2 >= (end + 1.0) * (end + 1.0)
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

/// (particles, sections, tick work) skipped so far.
pub fn counts() -> (u64, u64, u64) {
    (
        PARTICLES_SKIPPED.load(Ordering::Relaxed),
        SECTIONS_SKIPPED.load(Ordering::Relaxed),
        TICK_WORK_SAVED.load(Ordering::Relaxed),
    )
}
