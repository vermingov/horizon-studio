//! Where a voice is heard from: SVC plays a player's voice through OpenAL at
//! their eyes, with linear distance (reference 0, the packet's distance the
//! max) on a stereo output. These are OpenAL Soft's gains for that, as
//! Horizon's own audio engine computes them for the game's sounds
//! (`horizon-client-render/src/audio.rs`): the same voice sounds as near,
//! and from the same side, as a sound the game plays there.

/// The listener: the camera's place and the way it looks (degrees; yaw 0 faces south, +z).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Listener {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl Listener {
    /// The camera's forward and up vectors (Minecraft's `Camera`).
    pub fn axes(&self) -> ([f64; 3], [f64; 3]) {
        let (y, p) = (f64::from(self.yaw).to_radians(), f64::from(self.pitch).to_radians());
        let forward = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
        let up = [-y.sin() * p.sin(), p.cos(), y.cos() * p.sin()];
        (forward, up)
    }

    /// A point in listener space: (right, up, forward).
    pub fn local(&self, at: [f64; 3]) -> [f32; 3] {
        let (f, u) = self.axes();
        // OpenAL's right = forward x up.
        let r = [
            f[1] * u[2] - f[2] * u[1],
            f[2] * u[0] - f[0] * u[2],
            f[0] * u[1] - f[1] * u[0],
        ];
        let d = [at[0] - self.pos[0], at[1] - self.pos[1], at[2] - self.pos[2]];
        let dot = |a: [f64; 3]| (a[0] * d[0] + a[1] * d[1] + a[2] * d[2]) as f32;
        [dot(r), dot(u), dot(f)]
    }

    pub fn distance(&self, at: [f64; 3]) -> f64 {
        let d = [at[0] - self.pos[0], at[1] - self.pos[1], at[2] - self.pos[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }
}

/// OpenAL's `AL_LINEAR_DISTANCE` gain with reference distance 0 and rolloff
/// 1: `1 - min(d, max) / max`.
pub fn distance_gain(max: f32, distance: f32) -> f32 {
    if max > 0.0 {
        (1.0 - distance.min(max) / max).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// OpenAL Soft's stereo gains `[left, right]` for a mono source in a
/// direction in listener space (right, up, forward; any length): front
/// azimuths stretched by 1.5, then the first-order stereo decoder. A
/// centred source gets 0.596 on each side; one at the listener is in front.
pub fn stereo_gains(right: f32, up: f32, forward: f32) -> [f32; 2] {
    let length = (right * right + up * up + forward * forward).sqrt();
    if length <= f32::EPSILON {
        return stereo_gains(0.0, 0.0, 1.0);
    }
    let elevation = (up / length).clamp(-1.0, 1.0).asin();
    let mut azimuth = right.atan2(forward);
    if azimuth.abs() < std::f32::consts::FRAC_PI_2 {
        azimuth = (azimuth.abs() * 1.5).min(std::f32::consts::FRAC_PI_2).copysign(azimuth);
    }
    let side = azimuth.sin() * elevation.cos();
    let front = azimuth.cos() * elevation.cos() * 0.095_660_8;
    [0.5 - 0.5 * side + front, 0.5 + 0.5 * side + front]
}

/// A source heard without a place (a group's voice, the camera's own): as OpenAL plays a
/// relative source at the listener, in front.
pub fn centre() -> [f32; 2] {
    stereo_gains(0.0, 0.0, 1.0)
}

/// The gains `[left, right]` of a voice at `at` heard by `listener` with
/// `max` its reach, or None when SVC drops it (farther than `max + 1`).
pub fn positional(listener: &Listener, at: [f64; 3], max: f32) -> Option<[f32; 2]> {
    let d = listener.distance(at);
    if d > f64::from(max) + 1.0 {
        return None;
    }
    let g = distance_gain(max, d as f32);
    let [r, u, f] = listener.local(at);
    let [l, rr] = stereo_gains(r, u, f);
    Some([l * g, rr * g])
}
