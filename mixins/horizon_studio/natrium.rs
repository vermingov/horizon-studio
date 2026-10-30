// Natrium: Sodium's and Lithium's ideas for Horizon. Its own row in Options >
// Mods, with a page of switches; each frame the game's settings go to the
// switches the patched code reads (`horizon_core::natrium`), so a change
// applies at once, on the server thread too. Horizon already culls block
// faces turned away from the camera (Sodium's block face culling): the page
// says so rather than adding it twice.

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
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            enabled: true,
            particles: true,
            fog: true,
            ticks: true,
        }
    }
}

const KEYS: [&str; 4] = ["enabled", "particles", "fog", "ticks"];

impl Settings {
    fn get(&self, i: usize) -> bool {
        [self.enabled, self.particles, self.fog, self.ticks][i]
    }
    fn flip(&mut self, i: usize) {
        match i {
            0 => self.enabled = !self.enabled,
            1 => self.particles = !self.particles,
            2 => self.fog = !self.fog,
            3 => self.ticks = !self.ticks,
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

/// Each frame: the switches follow the settings (and Options > Mods).
pub(super) fn sync(game: &Game<'_>) {
    let s = settings(game);
    let on = s.enabled && game.native_on(ID);
    flags::set(on && s.particles, on && s.fog, on && s.ticks);
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
    buttons.push(format!("Natrium: {}", on_off(s.enabled)));
    buttons.push(format!("Particle culling: {}", on_off(s.particles)));
    buttons.push(format!("Fog culling: {}", on_off(s.fog)));
    buttons.push(format!("Faster ticks: {}", on_off(s.ticks)));
    buttons.push("Apply Natrium's graphics preset".into());
    let faces = game.renderer.as_ref().is_some_and(|r| r.section_sides());
    buttons.push(format!("Block face culling: built into Horizon ({})", on_off(faces)));
    let (p, sec, t) = flags::counts();
    buttons.push(format!(
        "Skipped so far: {} particles, {} sections, {} tick lookups",
        short(p),
        short(sec),
        short(t)
    ));
}

pub fn short(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.1}G", n as f64 / 1e9),
    }
}

pub(super) fn click(game: &mut Game<'_>, button: usize) {
    let mut s = settings(game);
    match button {
        0..=3 => {
            s.flip(button);
            save(game, &s);
            CACHE.with(|c| c.set(Some(s)));
            sync(game);
        }
        4 => preset(game),
        _ => {}
    }
}

/// Horizon's own settings, set as its descriptions advise for frame time:
/// entity culling on (off by default), cave culling on, and at most 16
/// sections and 4 MiB of mesh data uploaded a frame ("lower is smoother
/// while loading"). Saved with the game's settings.
fn preset(game: &mut Game<'_>) {
    let g = &mut game.settings.horizon;
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
