// The player's own settings while Roblox scripts have changed some of them
// for the session (the master volume, the graphics they lower, the chat's
// background, full screen: hstudio_bridge's client actions). options.txt
// is written with the player's values, never with the scripts' (the
// settings-save hook), and the session's end gives them back. A setting the
// player changed since a script did is theirs: it stays.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A setting as the player had it, and as a script set it.
#[derive(Clone, Debug)]
pub struct Kept<T> {
    pub own: T,
    pub set: T,
}

/// The settings scripts changed: each as the player had it.
#[derive(Clone, Debug, Default)]
pub struct Own {
    pub master_volume: Option<Kept<f32>>,
    pub text_background_opacity: Option<Kept<f32>>,
    pub fullscreen: Option<Kept<bool>>,
    /// The graphics lowered (Roblox's lowest quality levels), and each of
    /// the settings that lowers.
    pub lowered: bool,
    pub render_distance: Option<Kept<i32>>,
    pub particles: Option<Kept<u8>>,
    pub clouds: Option<Kept<String>>,
    pub entity_shadows: Option<Kept<bool>>,
    pub entity_distance: Option<Kept<f32>>,
    pub biome_blend: Option<Kept<i32>>,
    pub weather_radius: Option<Kept<i32>>,
}

static OWN: Mutex<Own> = Mutex::new(Own {
    master_volume: None,
    text_background_opacity: None,
    fullscreen: None,
    lowered: false,
    render_distance: None,
    particles: None,
    clouds: None,
    entity_shadows: None,
    entity_distance: None,
    biome_blend: None,
    weather_radius: None,
});
/// While the player's own settings are saved (through Settings::save again).
static SAVING: AtomicBool = AtomicBool::new(false);

fn lock() -> std::sync::MutexGuard<'static, Own> {
    OWN.lock().unwrap_or_else(|e| e.into_inner())
}

/// What is kept now, to change it.
pub fn with<R>(f: impl FnOnce(&mut Own) -> R) -> R {
    f(&mut lock())
}

/// Takes everything kept: the session's end gives it back (`Own::give_back`).
pub fn take() -> Own {
    std::mem::take(&mut *lock())
}

/// A script sets `now` to `to`: the player's value is kept (the first time,
/// or again once they changed it themselves).
pub fn change<T: PartialEq + Clone>(slot: &mut Option<Kept<T>>, now: &mut T, to: T) {
    let own = match slot.take() {
        Some(k) if *now == k.set => k.own,
        _ => now.clone(),
    };
    *now = to.clone();
    *slot = Some(Kept { own, set: to });
}

/// The player's value back where it is still the script's, forgotten.
pub fn back<T: PartialEq + Clone>(slot: &mut Option<Kept<T>>, now: &mut T) {
    if let Some(k) = slot.take()
        && *now == k.set
    {
        *now = k.own;
    }
}

fn put<T: PartialEq + Clone>(slot: &Option<Kept<T>>, now: &mut T) {
    if let Some(k) = slot
        && *now == k.set
    {
        *now = k.own.clone();
    }
}

impl Own {
    fn is_empty(&self) -> bool {
        self.master_volume.is_none()
            && self.text_background_opacity.is_none()
            && self.fullscreen.is_none()
            && !self.lowered
            && self.render_distance.is_none()
    }
    /// `s` with the player's own values (what is kept stays kept).
    pub fn apply(&self, s: &mut super::Settings) {
        put(&self.master_volume, &mut s.master_volume);
        put(&self.text_background_opacity, &mut s.text_background_opacity);
        put(&self.fullscreen, &mut s.fullscreen);
        put(&self.render_distance, &mut s.render_distance);
        put(&self.particles, &mut s.particles);
        put(&self.clouds, &mut s.clouds);
        put(&self.entity_shadows, &mut s.entity_shadows);
        put(&self.entity_distance, &mut s.entity_distance);
        put(&self.biome_blend, &mut s.biome_blend);
        put(&self.weather_radius, &mut s.weather_radius);
    }
    /// The graphics as the player had them, forgotten (but full screen,
    /// which the game toggles itself).
    pub fn give_back(&mut self, s: &mut super::Settings) {
        back(&mut self.master_volume, &mut s.master_volume);
        back(&mut self.text_background_opacity, &mut s.text_background_opacity);
        self.give_back_graphics(s);
    }
    /// The graphics lowered, as the player had them again.
    pub fn give_back_graphics(&mut self, s: &mut super::Settings) {
        self.lowered = false;
        back(&mut self.render_distance, &mut s.render_distance);
        back(&mut self.particles, &mut s.particles);
        back(&mut self.clouds, &mut s.clouds);
        back(&mut self.entity_shadows, &mut s.entity_shadows);
        back(&mut self.entity_distance, &mut s.entity_distance);
        back(&mut self.biome_blend, &mut s.biome_blend);
        back(&mut self.weather_radius, &mut s.weather_radius);
    }
}

/// Settings::save, as the settings-save hook has it: while scripts have
/// changed some settings, the player's own are written instead (a copy,
/// saved through Settings::save as itself). Whether it saved.
pub fn save_own(s: &super::Settings, path: &Path) -> bool {
    if SAVING.load(Ordering::Relaxed) {
        return false;
    }
    let mut own = s.clone();
    {
        let kept = lock();
        if kept.is_empty() {
            return false;
        }
        kept.apply(&mut own);
    }
    SAVING.store(true, Ordering::Relaxed);
    own.save(path);
    SAVING.store(false, Ordering::Relaxed);
    true
}
