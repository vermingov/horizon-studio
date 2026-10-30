// Natrium: Sodium's and Lithium's ideas for Horizon. Its own row in Options >
// Mods, with a page of switches, and the same switches in a section of Video
// Settings (`natrium_video`, whose buttons are this page's); each frame the
// game's settings go to the switches the patched code reads
// (`horizon_core::natrium`), so a change applies at once, on the server thread
// too. Horizon already culls block faces turned away from the camera (Sodium's
// block face culling): Natrium's switch is Horizon's own.

use super::Game;
use super::native_hooks::NativeMod;
use horizon_core::natrium as flags;

pub const ID: &str = "natrium";
pub const NAME: &str = "Natrium";
pub const VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub particles: bool,
    pub fog: bool,
    pub ticks: bool,
    pub faces: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            enabled: true,
            particles: true,
            fog: true,
            ticks: true,
            faces: true,
        }
    }
}

const KEYS: [&str; 5] = ["enabled", "particles", "fog", "ticks", "faces"];

impl Settings {
    fn get(&self, i: usize) -> bool {
        [self.enabled, self.particles, self.fog, self.ticks, self.faces][i]
    }
    fn flip(&mut self, i: usize) {
        match i {
            0 => self.enabled = !self.enabled,
            1 => self.particles = !self.particles,
            2 => self.fog = !self.fog,
            3 => self.ticks = !self.ticks,
            4 => self.faces = !self.faces,
            _ => {}
        }
    }
}

fn load(game: &Game<'_>) -> Settings {
    let mut s = Settings::default();
    for (i, key) in KEYS.iter().enumerate() {
        if let Some(v) = game.native_setting(ID, key)
            && s.get(i) != (v == "true")
        {
            s.flip(i);
        }
    }
    s
}

fn save(game: &Game<'_>, s: &Settings) {
    for (i, key) in KEYS.iter().enumerate() {
        game.native_set_setting(ID, key, if s.get(i) { "true" } else { "false" });
    }
}

thread_local! {
    static CACHE: std::cell::Cell<Option<Settings>> = const { std::cell::Cell::new(None) };
}

fn settings(game: &Game<'_>) -> Settings {
    CACHE.with(|c| {
        c.get().unwrap_or_else(|| {
            let s = load(game);
            c.set(Some(s));
            s
        })
    })
}

thread_local! {
    /// The block face culling Natrium last asked of the renderer.
    static FACES: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Each frame: the switches follow the settings (and Options > Mods), and
/// Video Settings shows them.
pub(super) fn sync(game: &mut Game<'_>) {
    let s = settings(game);
    let listed = game.native_on(ID);
    let on = s.enabled && listed;
    flags::set(on && s.particles, on && s.fog, on && s.ticks);
    // Horizon's own block face culling (on in Horizon), off only when asked.
    let faces = !on || s.faces;
    let changed = FACES.replace(Some(faces)) != Some(faces);
    if let Some(r) = game.renderer.as_mut()
        && (changed || (!faces && r.section_sides()))
    {
        r.set_section_sides(faces);
    }
    let index = game
        .settings
        .mods
        .installed
        .iter()
        .position(|m| m.id == ID && m.native.is_some());
    let switches = [s.enabled, s.particles, s.fog, s.ticks, s.faces];
    flags::set_video(index.filter(|_| listed).map(|i| (i, switches)));
}

pub(super) fn listed() -> NativeMod {
    NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: "Sodium's and Lithium's ideas for Horizon: draws less of what you can't see (particles off \
                      screen, terrain lost in thick fog) and ticks the server faster, with the same results."
            .into(),
        keys: Vec::new(),
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

/// Natrium's page: its switches, the preset, and what it skipped.
pub(super) fn page(game: &Game<'_>, buttons: &mut Vec<String>) {
    let s = settings(game);
    for (i, caption) in flags::CAPTIONS.iter().enumerate() {
        buttons.push(format!("{caption}: {}", on_off(s.get(i))));
    }
    buttons.push(flags::PRESET_LABEL.into());
    buttons.push(flags::skipped());
}

pub(super) fn click(game: &mut Game<'_>, button: usize) {
    let mut s = settings(game);
    match button {
        0..=4 => {
            s.flip(button);
            save(game, &s);
            CACHE.with(|c| c.set(Some(s)));
            sync(game);
        }
        flags::PRESET => preset(game),
        _ => {}
    }
}

/// Horizon's own settings, set as its descriptions advise for frame time:
/// entity culling on (off by default), cave culling on, and at most 16
/// sections and 4 MiB of mesh data uploaded a frame ("lower is smoother
/// while loading"). Saved with the game's settings; the graphics preset
/// reads Custom after it, as after any such change.
fn preset(game: &mut Game<'_>) {
    let g = &mut game.settings.horizon;
    g.preset = horizon_client::graphics::Preset::Custom;
    g.entity_culling = true;
    g.cave_culling = true;
    if g.chunk_updates == 0 || g.chunk_updates > 16 {
        g.chunk_updates = 16;
    }
    if g.upload_budget_kib == 0 || g.upload_budget_kib > 4096 {
        g.upload_budget_kib = 4096;
    }
    game.settings.save(&Game::options_path());
}
