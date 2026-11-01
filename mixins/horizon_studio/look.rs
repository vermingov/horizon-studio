// Horizon Studio's look: Horizon's design system (docs/design-system.md),
// drawn through GameApi's dp calls (Geist and Geist Mono, rounded plates) in
// the launcher's logical pixels. Neutrals do the work and the violet marks
// the main action; selection is ink. Motion follows the tokens: QUICK for
// hovers, MOVE for selection and pages, ENTER (a fade and an 8 dp rise)
// for content arriving, rows 40 ms apart.

use super::hstudio::{API, CONSOLE_API, Font, GameApi, LogKind, ModState, Studio};
use super::hstudio_examples::EXAMPLES;
use super::hstudio_theme::{ACCENTS, Layout, Look, Palette, THEMES};
use super::hstudio_ui::{
    Action, EditorView, LineEdit, Long, Panel, Spot, Tab, Tint, Ui, field, field_num, field_text, highlight, kind,
    slider_text, widgets,
};

// The colours come from the theme (`hstudio_theme`), set as each frame draws.
thread_local! {
    static PAL: std::cell::Cell<Palette> = const { std::cell::Cell::new(super::hstudio_theme::THEMES[0].palette) };
    static CODE: std::cell::Cell<f32> = const { std::cell::Cell::new(13.0) };
    static MOTION: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// The studio's palette easing to a newly chosen one: from, to, since.
    static EASE: std::cell::Cell<Option<(Palette, Palette, f64)>> = const { std::cell::Cell::new(None) };
}

/// This frame's palette.
pub(super) fn pal() -> Palette {
    PAL.with(std::cell::Cell::get)
}

pub(super) fn set_pal(p: Palette) {
    PAL.with(|c| c.set(p));
}

/// Takes up the chosen look: its palette, or with `plain` the studio's own
/// for what the executor styles have no look of (script screens, dialogs,
/// the toast), the code's size and whether things move.
pub(super) fn use_look(look: &Look, plain: bool) {
    let palette = if plain && look.theme().layout != Layout::Studio {
        Look {
            theme: 0,
            ..look.clone()
        }
        .palette()
    } else {
        look.palette()
    };
    set_pal(palette);
    CODE.with(|c| c.set(look.code));
    MOTION.with(|c| c.set(look.motion));
}

fn motion() -> bool {
    MOTION.with(std::cell::Cell::get)
}

fn mix_palette(a: Palette, b: Palette, t: f32) -> Palette {
    Palette {
        page: mix(a.page, b.page, t),
        surface: mix(a.surface, b.surface, t),
        plate_soft: mix(a.plate_soft, b.plate_soft, t),
        plate: mix(a.plate, b.plate, t),
        rule: mix(a.rule, b.rule, t),
        ink: mix(a.ink, b.ink, t),
        ink_soft: mix(a.ink_soft, b.ink_soft, t),
        muted: mix(a.muted, b.muted, t),
        accent: mix(a.accent, b.accent, t),
        accent_hover: mix(a.accent_hover, b.accent_hover, t),
        danger: mix(a.danger, b.danger, t),
        success: mix(a.success, b.success, t),
        keyword: mix(a.keyword, b.keyword, t),
        string: mix(a.string, b.string, t),
        number: mix(a.number, b.number, t),
        comment: mix(a.comment, b.comment, t),
        global: mix(a.global, b.global, t),
        round: a.round + (b.round - a.round) * t.clamp(0.0, 1.0),
    }
}

/// Eases the palette set to a newly chosen one (another theme or accent),
/// from wherever the last change had got to.
fn ease_palette(now: f64) {
    let target = pal();
    let shown = EASE.with(|e| {
        let (from, since) = match e.get() {
            Some((from, to, since)) if to == target => (from, since),
            Some((from, to, since)) => (mix_palette(from, to, progress(now, since, MOVE)), now),
            None => (target, now),
        };
        e.set(Some((from, target, since)));
        mix_palette(from, target, progress(now, since, MOVE))
    });
    set_pal(shown);
}

/// Text that reads on `c`: near black on a light colour, white on a dark one.
pub(super) fn on(c: u32) -> u32 {
    let ch = |s: u32| ((c >> s) & 0xFF) as f32;
    if 0.299 * ch(16) + 0.587 * ch(8) + 0.114 * ch(0) > 140.0 {
        0xFF0D_0E13
    } else {
        0xFFFF_FFFF
    }
}

/// A colour's relative luminance, as WCAG measures it (its alpha left out).
fn luminance(c: u32) -> f32 {
    let lin = |s: u32| {
        let v = ((c >> s) & 0xFF) as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(16) + 0.7152 * lin(8) + 0.0722 * lin(0)
}

/// How well two colours read on each other: WCAG's contrast ratio, 1 to 21.
pub(super) fn contrast(a: u32, b: u32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `c` as text on `bg`: as it is when it reads there at `ratio` (WCAG's
/// 4.5 for text), else moved toward what reads best on `bg` (black or
/// white) until it does.
pub(super) fn legible(c: u32, bg: u32, ratio: f32) -> u32 {
    let best = on(bg);
    (0..=10)
        .map(|k| mix(c, best, k as f32 / 10.0))
        .find(|&t| contrast(t, bg) >= ratio)
        .unwrap_or(best)
}

/// Warnings' amber, the same in every theme (some themes' numbers are
/// green), darker on a light page.
pub(super) fn warn() -> u32 {
    if on(pal().page) == 0xFFFF_FFFF {
        0xFFE8_B04A
    } else {
        0xFFA8_6400
    }
}

/// Text on the accent.
pub(super) fn on_accent() -> u32 {
    on(pal().accent)
}

/// The editor's and console's code: Geist Mono at the chosen size, one line per row.
pub fn code_size() -> f32 {
    CODE.with(std::cell::Cell::get)
}
pub fn code_line() -> f32 {
    (code_size() * 1.54).round()
}

#[derive(Clone, Copy)]
struct Motion {
    secs: f64,
    curve: [f32; 4],
}
const QUICK: Motion = Motion {
    secs: 0.15,
    curve: [0.4, 0.0, 0.2, 1.0],
};
const MOVE: Motion = Motion {
    secs: 0.24,
    curve: [0.23, 1.0, 0.32, 1.0],
};
const ENTER: Motion = Motion {
    secs: 0.6,
    curve: [0.23, 1.0, 0.32, 1.0],
};
const RISE: f32 = 8.0;
const STAGGER: f64 = 0.04;
/// How long the studio and script screens take to go.
pub const CLOSE_SECS: f64 = 0.15;
const TOAST_SECS: f64 = 2.5;

/// A CSS cubic-bezier's y at x = `t`.
fn bezier(c: [f32; 4], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let at = |a: f32, b: f32, u: f32| 3.0 * (1.0 - u) * (1.0 - u) * u * a + 3.0 * (1.0 - u) * u * u * b + u * u * u;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(c[0], c[2], mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    at(c[1], c[3], (lo + hi) / 2.0)
}

fn progress(now: f64, since: f64, m: Motion) -> f32 {
    if !motion() {
        return 1.0;
    }
    bezier(m.curve, ((now - since) / m.secs).clamp(0.0, 1.0) as f32)
}

pub(super) fn fade(c: u32, a: f32) -> u32 {
    let alpha = ((c >> 24) as f32 * a.clamp(0.0, 1.0)).round() as u32;
    (c & 0x00FF_FFFF) | alpha << 24
}

pub(super) fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

pub(super) fn id(s: &str, n: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes().chain(n.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A hover (or other) fade toward `on`, eased.
pub(super) fn hover(ui: &mut Ui, key: u64, on: bool, dt: f32) -> f32 {
    let v = ui.anim.entry(key).or_insert(0.0);
    let step = if motion() { dt / QUICK.secs as f32 } else { 1.0 };
    *v = if on { (*v + step).min(1.0) } else { (*v - step).max(0.0) };
    bezier(QUICK.curve, *v)
}

/// A switch's or selection's move toward `on`, eased.
pub(super) fn glide(ui: &mut Ui, key: u64, on: bool, dt: f32) -> f32 {
    let v = ui.anim.entry(key).or_insert(if on { 1.0 } else { 0.0 });
    let step = if motion() { dt / MOVE.secs as f32 } else { 1.0 };
    *v = if on { (*v + step).min(1.0) } else { (*v - step).max(0.0) };
    bezier(MOVE.curve, *v)
}

pub(super) fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// The top of a `size` line box centred on `y`.
pub(super) fn top(y: f32, size: f32) -> f32 {
    (y - (size * 1.45).round() / 2.0).floor()
}

/// Drawing with a fade and a rise applied, for whatever is arriving, in
/// the studio's units: dp times the size setting, with the theme's corners.
pub(super) struct P<'g> {
    pub(super) g: &'g mut dyn GameApi,
    pub(super) alpha: f32,
    pub(super) dy: f32,
    /// dp per unit.
    pub(super) s: f32,
}

impl P<'_> {
    fn at(&self, r: [f32; 4]) -> [f32; 4] {
        let s = self.s;
        [r[0] * s, (r[1] + self.dy) * s, r[2] * s, r[3] * s]
    }
    pub(super) fn rect(&mut self, r: [f32; 4], radius: f32, c: u32) {
        let c = fade(c, self.alpha);
        if c >> 24 != 0 && r[2] > 0.0 && r[3] > 0.0 {
            let r = self.at(r);
            self.g.dp_rect(r, radius * self.s * pal().round, c);
        }
    }
    pub(super) fn border(&mut self, r: [f32; 4], radius: f32, c: u32) {
        let c = fade(c, self.alpha);
        if c >> 24 != 0 {
            let r = self.at(r);
            self.g.dp_border(r, radius * self.s * pal().round, c);
        }
    }
    pub(super) fn text(&mut self, t: &str, x: f32, y: f32, size: f32, font: Font, c: u32) -> f32 {
        if t.is_empty() {
            return 0.0;
        }
        let s = self.s;
        self.g
            .dp_text(t, [x * s, (y + self.dy) * s], size * s, font, fade(c, self.alpha))
            / s
    }
    pub(super) fn measure(&mut self, t: &str, size: f32, font: Font) -> f32 {
        self.g.dp_measure(t, size * self.s, font) / self.s
    }
    /// The longest start of `s` that fits in `max`, with an ellipsis when cut.
    pub(super) fn fit(&mut self, s: &str, max: f32, size: f32, font: Font) -> String {
        if self.measure(s, size, font) <= max {
            return s.to_owned();
        }
        let mut out = String::new();
        for c in s.chars() {
            out.push(c);
            if self.measure(&out, size, font) > max - self.measure("…", size, font) {
                out.pop();
                out.push('…');
                return out;
            }
        }
        out
    }
    pub(super) fn wrap(&mut self, s: &str, max: f32, size: f32, font: Font) -> Vec<String> {
        self.wrap_sep(s, max, size, font).into_iter().map(|(l, _)| l).collect()
    }
    /// `wrap`, each piece with what came after it in `s`: a line break, a
    /// space (the wrap's), or nothing (a word cut); copying puts it back.
    pub(super) fn wrap_sep(&mut self, s: &str, max: f32, size: f32, font: Font) -> Vec<(String, &'static str)> {
        let mut lines = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split_inclusive(' ') {
                let candidate = format!("{line}{word}");
                if self.measure(candidate.trim_end(), size, font) <= max {
                    line = candidate;
                    continue;
                }
                if !line.is_empty() {
                    lines.push((line.trim_end().to_owned(), " "));
                }
                let mut piece = String::new();
                for c in word.chars() {
                    piece.push(c);
                    if self.measure(&piece, size, font) > max {
                        piece.pop();
                        lines.push((std::mem::take(&mut piece), ""));
                        piece.push(c);
                    }
                }
                line = piece;
            }
            lines.push((line.trim_end().to_owned(), "\n"));
        }
        lines
    }
}

/// A row of selectable text (the console's, the agent's) drawn at `[x, y]`,
/// `h` high: remembered for the pointer, its selected part shaded. Its
/// char edges are measured only while the pointer may need them.
#[allow(clippy::too_many_arguments)]
fn text_row(
    ui: &mut Ui,
    p: &mut P,
    area: u8,
    key: (u64, usize),
    text: &str,
    after: &'static str,
    [x, y, h]: [f32; 3],
    size: f32,
    font: Font,
) {
    use super::hstudio_ui::TextRow;
    if let Some((a, b)) = super::hstudio_ui::selected_cols(ui, area, key, text.len()) {
        let sx = x + p.measure(&text[..a], size, font);
        let mut sw = p.measure(&text[a..b], size, font);
        if b == text.len() && after == "\n" && super::hstudio_ui::selection_goes_on(ui, area, key) {
            sw += 5.0;
        }
        p.rect([sx, y + 1.0, sw, h - 2.0], 3.0, fade(pal().accent, 0.30));
    }
    let want = ui.text_drag || ui.text_areas.iter().any(|(a, r)| *a == area && inside(*r, ui.mouse));
    let edges = if want {
        let mut e = vec![0.0];
        let mut acc = String::new();
        for c in text.chars() {
            acc.push(c);
            e.push(p.measure(&acc, size, font));
        }
        e
    } else {
        Vec::new()
    };
    ui.text_rows.push(TextRow {
        area,
        key,
        x,
        y,
        h,
        text: text.to_owned(),
        after,
        edges,
    });
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Primary,
    Secondary,
    Quiet,
}

/// A pill button; returns its width.
#[allow(clippy::too_many_arguments)]
fn pill(
    ui: &mut Ui,
    p: &mut P,
    key: &str,
    label: &str,
    x: f32,
    y: f32,
    h: f32,
    style: Style,
    action: Action,
    dt: f32,
) -> f32 {
    let size = 13.0;
    let w = p.measure(label, size, Font::Medium) + 28.0;
    let r = [x, y, w, h];
    let t = hover(ui, id(key, 0), inside(r, ui.mouse), dt);
    let (bg, fg) = match style {
        Style::Primary => (mix(pal().accent, pal().accent_hover, t), on_accent()),
        Style::Secondary => (mix(fade(pal().ink, 0.08), fade(pal().ink, 0.15), t), pal().ink),
        Style::Quiet => (fade(pal().ink, 0.10 * t), mix(pal().ink_soft, pal().ink, t)),
    };
    p.rect(r, h / 2.0, bg);
    p.text(label, x + 14.0, top(y + h / 2.0, size), size, Font::Medium, fg);
    ui.hits.push((r, action));
    w
}

/// A key's name on a small key cap, then what it does; returns the width.
fn keycap(p: &mut P, keys: &str, what: &str, x: f32, cy: f32) -> f32 {
    let kw = p.measure(keys, 11.0, Font::Mono) + 12.0;
    p.rect([x, cy - 10.0, kw, 20.0], 5.0, fade(pal().ink, 0.06));
    p.border([x, cy - 10.0, kw, 20.0], 5.0, pal().rule);
    p.text(keys, x + 6.0, top(cy, 11.0), 11.0, Font::Mono, pal().ink_soft);
    let ww = p.text(what, x + kw + 6.0, top(cy, 12.0), 12.0, Font::Regular, pal().muted);
    kw + 6.0 + ww
}

/// How present the screen is: opening and closing, as a fade and a rise.
fn presence(ui: &Ui, now: f64) -> (f32, f32) {
    let open = progress(now, ui.opened_at, MOVE);
    let close = ui.closing_at.map_or(1.0, |t| 1.0 - progress(now, t, QUICK));
    (open * close, (1.0 - open) * RISE + (1.0 - close) * RISE * 0.5)
}

fn tick(ui: &mut Ui, now: f64) -> f32 {
    let dt = ((now - ui.last_draw) as f32).clamp(0.0, 0.1);
    ui.last_draw = now;
    dt
}

fn smooth(v: &mut f32, target: f32, dt: f32) {
    if !motion() {
        *v = target;
        return;
    }
    *v += (target - *v) * (1.0 - (-dt * 18.0).exp());
    if (target - *v).abs() < 0.01 {
        *v = target;
    }
}

// ----------------------------------------------------------------- the studio

const TABS: [(Tab, &str); 5] = [
    (Tab::Console, "Console"),
    (Tab::Editor, "Editor"),
    (Tab::Library, "Library"),
    (Tab::Docs, "Docs"),
    (Tab::Settings, "Settings"),
];

/// A Windows 10 title bar's height, and its buttons' width.
pub(super) const TITLE: f32 = 30.0;
const CAPTION_W: f32 = 46.0;

pub fn draw(st: &mut Studio, game: &mut dyn GameApi) {
    let now = st.s.now;
    if now < st.s.ui.hidden_until {
        return;
    }
    let look = st.s.ui.look.clone();
    use_look(&look, false);
    ease_palette(now);
    let (dw, dh, k) = game.dp();
    let s = look.scale;
    let (w, h) = (dw / s, dh / s);
    // Back after a while (opened again): nothing glides from before.
    let fresh = now - st.s.ui.last_draw > 0.25;
    // Back since the last frame (opened again, or from a script's screen):
    // what the left button held then has ended, its release gone elsewhere.
    if st.s.ui.opened_at > st.s.ui.last_draw {
        super::hstudio_ui::let_go(st);
    }
    let dt = {
        let ui = &mut st.s.ui;
        // A menu left open goes when the studio opens again (or comes back
        // from a script's screen), and one about the code when the code
        // changed under it.
        if ui
            .menu
            .as_ref()
            .is_some_and(|m| m.opened_at < ui.opened_at || m.stale(&st.s.editor))
        {
            ui.menu = None;
        }
        ui.dp = k;
        ui.screen = (w, h);
        ui.hits.clear();
        ui.text_rows.clear();
        ui.text_areas.clear();
        ui.view = None;
        ui.caret_at = None;
        ui.scripts_at = None;
        ui.library_dots = None;
        ui.drawn_files.clear();
        tick(ui, now)
    };
    // The hits name mods by their place in this list.
    st.s.ui.drawn_mods = st.s.mods.iter().map(|m| m.id.clone()).collect();
    // The open file changed on disk (an agent wrote it): read it again,
    // or type the change in (hstudio_coach).
    if now - st.s.ui.disk_at >= 0.5 {
        st.s.ui.disk_at = now;
        if super::hstudio_coach::sync_disk(st) {
            st.s.editor.reloaded_at = Some(now);
        }
    }
    let (alpha, rise) = presence(&st.s.ui, now);
    let mut p = P {
        g: game,
        alpha,
        dy: 0.0,
        s,
    };
    if look.windowed() {
        p.rect([0.0, 0.0, w, h], 0.0, fade(0xFF00_0000, 0.4 * look.opacity));
        p.dy = rise;
        let layout = look.theme().layout;
        let r = glide_window(&mut st.s.ui, look.window_rect(w, h), fresh, dt);
        let c = chrome(layout);
        if let Some((b, shown)) = window(st, &mut p, r, c, dt) {
            p.alpha = alpha * shown;
            match layout {
                Layout::Studio => studio_page(
                    st,
                    &mut p,
                    [b[0] + 16.0, b[1] + 12.0, b[2] - 32.0, b[3] - 26.0],
                    dt,
                    true,
                ),
                _ => super::hstudio_skins::draw(st, &mut p, layout, b, dt),
            }
            p.alpha = alpha;
            if shown >= 1.0 {
                grip(st, &mut p, r, c);
            }
        }
    } else {
        p.rect([0.0, 0.0, w, h], 0.0, fade(pal().page, look.opacity));
        p.dy = rise;
        let gutter = if w >= 900.0 { 48.0 } else { 20.0 };
        let cw = (w - 2.0 * gutter).clamp(200.0, 1280.0);
        let x0 = ((w - cw) / 2.0).floor();
        studio_page(st, &mut p, [x0, 24.0, cw, h - 48.0], dt, false);
    }
    if st.s.ui.menu.is_some() {
        // In the executor styles, in the studio's own colours (as its
        // dialogs are): theirs are a title bar's and a button's.
        if look.theme().layout != Layout::Studio {
            use_look(&look, true);
        }
        p.dy = 0.0;
        draw_menu(st, &mut p, w, h, dt);
    }
    if let Some(name) = st.s.ui.naming.clone() {
        use_look(&look, true);
        p.dy = 0.0;
        naming(st, &mut p, &name, w, h, dt);
    }
    if st.s.ui.notice.is_some() {
        use_look(&look, true);
        p.dy = 0.0;
        notice(st, &mut p, w, h);
    }
}

/// What a menu's item just did ("Copied 42 characters"), at the bottom of
/// the studio a moment: the HUD's toast is not drawn while it is open.
fn notice(st: &mut Studio, p: &mut P, w: f32, h: f32) {
    let now = st.s.now;
    let Some((text, at)) = st.s.ui.notice.clone() else {
        return;
    };
    if now - at > TOAST_SECS || now < at {
        st.s.ui.notice = None;
        return;
    }
    let arrive = progress(now, at, MOVE);
    let leave = 1.0 - progress(now, at + TOAST_SECS - 0.3, QUICK);
    let (a, dy) = (p.alpha, p.dy);
    p.alpha = a * arrive * leave;
    p.dy = (1.0 - arrive) * RISE;
    let text = p.fit(&text, w - 80.0, 13.0, Font::Regular);
    let tw = p.measure(&text, 13.0, Font::Regular);
    let r = [((w - tw) / 2.0 - 28.0).floor(), (h - 92.0).floor(), tw + 46.0, 36.0];
    p.rect(r, 18.0, pal().surface);
    p.border(r, 18.0, pal().rule);
    p.rect([r[0] + 16.0, r[1] + 15.0, 6.0, 6.0], 3.0, pal().accent);
    p.text(
        &text,
        r[0] + 30.0,
        top(r[1] + 18.0, 13.0),
        13.0,
        Font::Regular,
        pal().ink,
    );
    p.alpha = a;
    p.dy = dy;
}

/// The open menu: its items in a plate at its place, kept on the screen
/// (opening upward where there is no room below, or as low as it fits).
/// Its rows shrink to fit the screen, and a menu taller than that still
/// shows what fits from its scroll on, with marks for the rest. One row is
/// lit, the one the keys or the pointer are on; the current choice has a
/// dot, a hint (a shortcut, or "›") sits at the right, and an item that
/// deletes is in the danger colour, each kept readable on its row. The row
/// the menu is for is outlined. Clicks between its rows go nowhere.
fn draw_menu(st: &mut Studio, p: &mut P, w: f32, h: f32, dt: f32) {
    const SEP: f32 = 9.0;
    const PAD: f32 = 5.0;
    const MARK: f32 = 14.0;
    let now = st.s.now;
    let Some(mut menu) = st.s.ui.menu.clone() else { return };
    let n = menu.items.len();
    if n == 0 {
        st.s.ui.menu = None;
        return;
    }
    menu.sel = menu.sel.filter(|&s| s < n);
    let size = 12.5;
    let room = (h - 16.0).max(60.0);
    let seps = menu.items.iter().filter(|i| i.separator).count();
    let rows = (n - seps).max(1);
    let row = ((room - 2.0 * PAD - seps as f32 * SEP) / rows as f32)
        .floor()
        .clamp(20.0, 26.0);
    let item_h = |i: usize| if menu.items[i].separator { SEP } else { row };
    // A label starts 26 in, and a hint ends 10 from the right, 18 after it.
    let hint_w = |p: &mut P, hint: &str| {
        if hint.is_empty() {
            0.0
        } else {
            p.measure(hint, 11.5, Font::Mono) + 18.0
        }
    };
    let mut mw: f32 = 168.0;
    for it in menu.items.iter().filter(|i| !i.separator) {
        let hint = hint_w(p, &it.hint);
        mw = mw.max(p.measure(&it.label, size, Font::Regular) + hint + 2.0 * PAD + 36.0);
    }
    let mw = mw.min(w - 16.0);
    // What shows: everything, or from its scroll as much as fits, the lit
    // item always among it.
    let total = 2.0 * PAD + (0..n).map(item_h).sum::<f32>();
    let (first, end) = if total <= room {
        (0, n)
    } else {
        let end_from = |first: usize| {
            let mut used = 2.0 * PAD + 2.0 * MARK;
            let mut j = first;
            while j < n && used + item_h(j) <= room {
                used += item_h(j);
                j += 1;
            }
            j.max(first + 1)
        };
        let mut first = menu.scroll.min(n - 1);
        if let Some(s) = menu.sel {
            first = first.min(s);
        }
        while menu.sel.is_some_and(|s| s >= end_from(first)) {
            first += 1;
        }
        // Scrolled past the end, it shows as many as fit there.
        while first > 0 && end_from(first - 1) == n {
            first -= 1;
        }
        (first, end_from(first))
    };
    let scrolls = (first, end) != (0, n);
    let mh = 2.0 * PAD + (first..end).map(item_h).sum::<f32>() + if scrolls { 2.0 * MARK } else { 0.0 };
    let mut x = menu.at[0].min(w - mw - 8.0).max(8.0);
    let mut y = menu.at[1];
    if y + mh > h - 8.0 {
        // Above where it was asked for (over what opened it, or the
        // pointer), or else as low as it fits.
        let above = menu.at[1] - mh - 8.0 - menu.anchor_h;
        y = if above >= 8.0 { above } else { (h - 8.0 - mh).max(8.0) };
    }
    x = x.floor();
    y = y.floor();
    let rect = [x, y, mw, mh];
    let arrive = progress(now, menu.opened_at, QUICK);
    let (a, dy) = (p.alpha, p.dy);
    p.alpha = a * arrive;
    p.dy = dy + (1.0 - arrive) * -4.0;
    if let Some(t) = menu.target {
        p.border(t, 8.0, fade(pal().accent, 0.7));
    }
    let plate = pal().surface;
    let lit = mix(plate, pal().accent, 0.18);
    p.rect([x + 1.0, y + 3.0, mw, mh], 10.0, fade(0xFF00_0000, 0.25));
    p.rect(rect, 10.0, plate);
    p.border(rect, 10.0, pal().rule);
    st.s.ui.hits.push((rect, Action::Block));
    let mut ry = y + PAD;
    if scrolls {
        // A mark that there is more above, and below.
        let c = legible(pal().muted, plate, 3.0);
        let cx = x + mw / 2.0;
        for k in 0..4 {
            let f = k as f32;
            if first > 0 {
                p.rect([cx - f, ry + 4.0 + f, 1.0 + 2.0 * f, 1.0], 0.0, c);
            }
            if end < n {
                p.rect(
                    [cx - 3.0 + f, y + mh - PAD - MARK + 6.0 + f, 7.0 - 2.0 * f, 1.0],
                    0.0,
                    c,
                );
            }
        }
        ry += MARK;
    }
    for i in first..end {
        let it = &menu.items[i];
        if it.separator {
            p.rect([x + 10.0, ry + SEP / 2.0, mw - 20.0, 1.0], 0.0, pal().rule);
            ry += SEP;
            continue;
        }
        let r = [x + PAD, ry, mw - 2.0 * PAD, row];
        let ui = &mut st.s.ui;
        let hv = hover(ui, id("menu-row", i), it.enabled && menu.sel == Some(i), dt);
        let bg = if hv > 0.01 {
            let bg = mix(plate, lit, hv);
            p.rect(r, 7.0, bg);
            bg
        } else {
            plate
        };
        if it.checked {
            p.rect([r[0] + 10.0, ry + row / 2.0 - 3.0, 6.0, 6.0], 3.0, pal().accent);
        }
        let c = match (it.enabled, it.danger) {
            (false, _) => legible(pal().muted, bg, 3.0),
            (true, true) => legible(pal().danger, bg, 4.5),
            (true, false) => legible(pal().ink, bg, 4.5),
        };
        let room = r[2] - 36.0 - hint_w(p, &it.hint) + 0.5;
        let label = p.fit(&it.label, room, size, Font::Regular);
        p.text(&label, r[0] + 26.0, top(ry + row / 2.0, size), size, Font::Regular, c);
        if !it.hint.is_empty() {
            let hw = p.measure(&it.hint, 11.5, Font::Mono);
            p.text(
                &it.hint,
                r[0] + r[2] - 10.0 - hw,
                top(ry + row / 2.0, 11.5),
                11.5,
                Font::Mono,
                legible(pal().muted, bg, 3.0),
            );
        }
        if it.enabled {
            ui.hits.push((r, Action::MenuPick(i.min(255) as u8)));
        }
        ry += row;
    }
    p.alpha = a;
    p.dy = dy;
    if let Some(m) = st.s.ui.menu.as_mut() {
        m.rect = Some(rect);
        m.sel = menu.sel;
        m.scroll = first;
        m.shown = (first, end);
    }
}

/// The studio's own layout in `[x, y, w, h]`: the tabs, the page, the keys.
fn studio_page(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32, windowed: bool) {
    let now = st.s.now;
    let (alpha, rise) = (p.alpha, p.dy);
    header(st, p, x, y, w, dt, windowed);
    let tab = st.s.ui.tab;
    let shown = progress(now, st.s.ui.tab_at, MOVE);
    let body = [x, y + 56.0, w, (h - 84.0).max(40.0)];
    p.alpha = alpha * shown;
    p.dy = rise + (1.0 - shown) * RISE;
    page(st, p, tab, body, dt);
    p.alpha = alpha;
    p.dy = rise;
    footer(st, p, x, y + h - 4.0, w);
}

/// A tab's page in `body`.
pub(super) fn page(st: &mut Studio, p: &mut P, tab: Tab, body: [f32; 4], dt: f32) {
    match tab {
        Tab::Console => draw_console(st, p, body, dt),
        Tab::Editor => draw_editor(st, p, body, dt),
        Tab::Library => draw_library(st, p, body, dt),
        Tab::Docs => draw_docs(st, p, body, dt),
        Tab::Settings => draw_settings(st, p, body, dt),
    }
}

/// A window's colours: its title bar and the text on it, its body, its edge.
#[derive(Clone, Copy)]
pub(super) struct Chrome {
    pub(super) bar: u32,
    pub(super) text: u32,
    pub(super) body: u32,
    pub(super) edge: u32,
}

fn chrome(layout: Layout) -> Chrome {
    let c = pal();
    let text = match layout {
        Layout::Studio => c.ink,
        _ => on(c.surface),
    };
    let edge = match layout {
        Layout::Studio | Layout::Synapse => c.rule,
        _ => 0xFF6E_6E6E,
    };
    Chrome {
        bar: c.surface,
        text,
        body: c.page,
        edge,
    }
}

/// The window as drawn, gliding to `to` (maximised, restored, the screen
/// resized); while it is moved or sized it follows the pointer at once.
fn glide_window(ui: &mut Ui, to: [f32; 4], fresh: bool, dt: f32) -> [f32; 4] {
    let shown = match ui.win_shown {
        Some(r) if motion() && !fresh && ui.drag.is_none() => {
            let k = 1.0 - (-dt * 16.0).exp();
            let mut out = [0.0; 4];
            for i in 0..4 {
                out[i] = r[i] + (to[i] - r[i]) * k;
                if (out[i] - to[i]).abs() < 0.5 {
                    out[i] = to[i];
                }
            }
            out
        }
        _ => to,
    };
    ui.win_shown = Some(shown);
    shown
}

/// A Windows 10 window: its shadow, its title bar (the studio's mark, its
/// name, then minimise, maximise and close) and its edge. Returns its body
/// and how much of it shows: rolling up to the title bar, what is in it
/// goes first, then the window folds; unrolling, the reverse.
fn window(st: &mut Studio, p: &mut P, r: [f32; 4], c: Chrome, dt: f32) -> Option<([f32; 4], f32)> {
    let ui = &mut st.s.ui;
    let (rolled, maxed) = (ui.collapsed, ui.look.maximized);
    let [x, y, w, full_h] = r;
    let roll = glide(ui, id("roll", 0), rolled, dt);
    let h = full_h + (TITLE - full_h) * roll;
    let round = pal().round;
    set_pal(Palette { round: 0.0, ..pal() });
    if !maxed {
        for i in 1..=10 {
            let f = i as f32;
            p.rect(
                [x - f, y - f + 4.0, w + 2.0 * f, h + 2.0 * f],
                0.0,
                fade(0xFF00_0000, 0.028),
            );
        }
    }
    p.rect([x, y, w, h], 0.0, c.body);
    p.rect([x, y, w, TITLE], 0.0, c.bar);
    ui.hits.push(([x, y, w, TITLE], Action::WinMove));
    let cy = y + TITLE / 2.0;
    p.rect([x + 9.0, cy - 7.0, 14.0, 14.0], 3.0, pal().accent);
    let hw = p.measure("H", 10.0, Font::Medium);
    p.text("H", x + 16.0 - hw / 2.0, top(cy, 10.0), 10.0, Font::Medium, on_accent());
    let name = p.fit("Horizon Studio", w - 3.0 * CAPTION_W - 44.0, 12.0, Font::Regular);
    p.text(&name, x + 32.0, top(cy, 12.0), 12.0, Font::Regular, c.text);
    for (i, action) in [Action::WinMin, Action::WinMax, Action::Close].into_iter().enumerate() {
        let b = [x + w - CAPTION_W * (3 - i) as f32, y, CAPTION_W, TITLE];
        let t = hover(ui, id("caption", i), inside(b, ui.mouse), dt);
        let close = action == Action::Close;
        let over = if close {
            fade(0xFFE8_1123, t)
        } else {
            fade(c.text, 0.12 * t)
        };
        p.rect(b, 0.0, over);
        let g = if close { mix(c.text, 0xFFFF_FFFF, t) } else { c.text };
        let gx = b[0] + CAPTION_W / 2.0;
        match action {
            Action::WinMin => p.rect([gx - 5.0, cy, 10.0, 1.0], 0.0, g),
            Action::WinMax if maxed => {
                p.border([gx - 3.0, cy - 5.0, 8.0, 8.0], 0.0, g);
                p.rect([gx - 5.0, cy - 3.0, 8.0, 8.0], 0.0, c.bar);
                p.rect([gx - 5.0, cy - 3.0, 8.0, 8.0], 0.0, over);
                p.border([gx - 5.0, cy - 3.0, 8.0, 8.0], 0.0, g);
            }
            Action::WinMax => p.border([gx - 5.0, cy - 5.0, 10.0, 10.0], 0.0, g),
            _ => {
                let xw = p.measure("×", 18.0, Font::Regular);
                p.text("×", gx - xw / 2.0, top(cy - 1.0, 18.0), 18.0, Font::Regular, g);
            }
        }
        ui.hits.push((b, action));
    }
    // A right click on the title bar opens the window's menu, as Windows' does.
    ui.hits.push(([x, y, w, TITLE], Action::Spot(Spot::Window)));
    p.border([x, y, w, h], 0.0, c.edge);
    set_pal(Palette { round, ..pal() });
    // (The eased value is a hair off 0 at rest.)
    let shown = if roll < 0.001 {
        1.0
    } else {
        (1.0 - roll / 0.15).clamp(0.0, 1.0)
    };
    (shown > 0.0).then_some(([x + 1.0, y + TITLE, w - 2.0, full_h - TITLE - 1.0], shown))
}

/// The grip that sizes the window, at its bottom right, over its body.
fn grip(st: &mut Studio, p: &mut P, r: [f32; 4], c: Chrome) {
    if st.s.ui.look.maximized || st.s.ui.collapsed {
        return;
    }
    let (gx, gy) = (r[0] + r[2] - 4.0, r[1] + r[3] - 4.0);
    let dot = fade(on(c.body), 0.4);
    for (a, b) in [(1.0, 1.0), (2.0, 1.0), (1.0, 2.0), (3.0, 1.0), (2.0, 2.0), (1.0, 3.0)] {
        p.g.dp_rect(
            [(gx - 3.0 * a) * p.s, (gy - 3.0 * b + p.dy) * p.s, 2.0 * p.s, 2.0 * p.s],
            0.0,
            fade(dot, p.alpha),
        );
    }
    st.s.ui
        .hits
        .push(([r[0] + r[2] - 16.0, r[1] + r[3] - 16.0, 16.0, 16.0], Action::WinResize));
}

fn header(st: &mut Studio, p: &mut P, x: f32, y: f32, w: f32, dt: f32, windowed: bool) {
    let now = st.s.now;
    // In a window its title bar has the name and the close button.
    let mut sx = if windowed {
        x
    } else {
        x + p.text("Horizon Studio", x, top(y + 16.0, 16.0), 16.0, Font::Medium, pal().ink) + 28.0
    };
    let tx = sx;
    let mut segs = Vec::new();
    for (tab, label) in TABS {
        let sw = p.measure(label, 13.0, Font::Medium) + 28.0;
        segs.push((tab, label, [sx + 2.0, y + 2.0, sw, 28.0]));
        sx += sw;
    }
    p.rect([tx, y, sx - tx + 4.0, 32.0], 16.0, pal().plate_soft);
    let ui = &mut st.s.ui;
    let target = segs
        .iter()
        .find(|s| s.0 == ui.tab)
        .map_or([tx, 0.0], |s| [s.2[0], s.2[2]]);
    if ui.tab_drawn != Some(ui.tab) {
        if ui.tab_drawn.is_some() {
            ui.pill_from = ui.pill;
            ui.tab_at = now;
        } else {
            ui.pill_from = target;
            ui.tab_at = now - 1.0;
        }
        ui.tab_drawn = Some(ui.tab);
    }
    let t = progress(now, ui.tab_at, MOVE);
    ui.pill = [
        ui.pill_from[0] + (target[0] - ui.pill_from[0]) * t,
        ui.pill_from[1] + (target[1] - ui.pill_from[1]) * t,
    ];
    p.rect([ui.pill[0], y + 2.0, ui.pill[1], 28.0], 14.0, pal().ink);
    for (tab, label, r) in segs {
        let on = tab == ui.tab;
        let hv = hover(ui, id("tab", tab as usize), inside(r, ui.mouse), dt);
        let c = if on {
            pal().page
        } else {
            mix(pal().muted, pal().ink, hv)
        };
        p.text(label, r[0] + 14.0, top(y + 16.0, 13.0), 13.0, Font::Medium, c);
        ui.hits.push((r, Action::Tab(tab)));
    }
    // A right click anywhere on the header opens the window's menu.
    ui.hits.push(([x, y, w, 32.0], Action::Spot(Spot::Window)));
    // Close, at the right.
    let mut right = x + w;
    if !windowed {
        let r = [x + w - 32.0, y, 32.0, 32.0];
        let hv = hover(ui, id("close", 0), inside(r, ui.mouse), dt);
        p.rect(r, 16.0, fade(pal().ink, 0.10 * hv));
        let cw = p.measure("×", 20.0, Font::Regular);
        p.text(
            "×",
            r[0] + (32.0 - cw) / 2.0,
            top(y + 15.0, 20.0),
            20.0,
            Font::Regular,
            mix(pal().muted, pal().ink, hv),
        );
        ui.hits.push((r, Action::Close));
        let esc_w = p.measure("Esc", 11.0, Font::Mono) + 12.0;
        let esc_x = r[0] - esc_w - 10.0;
        keycap(p, "Esc", "", esc_x, y + 16.0);
        right = esc_x - 16.0;
    }
    // The AI link: the agent connected, else the prompt that sets one up.
    match super::hstudio_link::agents_line() {
        Some(line) => {
            let cw = p.measure(&line, 12.0, Font::Medium) + 36.0;
            let cx = right - cw;
            if cx > sx + 16.0 {
                let pulse = 0.75 + 0.25 * (now as f32 * 2.4).sin();
                p.rect([cx, y + 2.0, cw, 28.0], 14.0, fade(pal().success, 0.14));
                p.rect([cx + 13.0, y + 12.0, 8.0, 8.0], 4.0, fade(pal().success, pulse));
                p.text(&line, cx + 27.0, top(y + 16.0, 12.0), 12.0, Font::Medium, pal().ink);
            }
        }
        None => {
            let copied = ui.copied_at.is_some_and(|t| now - t < 2.5);
            let label = if copied { "Copied" } else { "Copy AI setup prompt" };
            let pw = p.measure(label, 13.0, Font::Medium) + 28.0;
            if right - pw > sx + 16.0 {
                pill(
                    ui,
                    p,
                    "ai-prompt",
                    label,
                    right - pw,
                    y + 2.0,
                    28.0,
                    Style::Quiet,
                    Action::CopyPrompt,
                    dt,
                );
            }
        }
    }
}

fn footer(st: &mut Studio, p: &mut P, x: f32, cy: f32, w: f32) {
    let menu: &[(&str, &str)] = &[("↑ ↓", "choose"), ("⏎", "do it"), ("Esc", "close")];
    let hints: &[(&str, &str)] = match st.s.ui.tab {
        // An open menu has the keys.
        _ if st.s.ui.menu.is_some() => menu,
        Tab::Console => &[
            ("⏎", "run"),
            ("↑ ↓", "history"),
            ("Tab", "complete"),
            ("Ctrl L", "clear"),
        ],
        Tab::Editor if st.s.ui.panel_focus && !st.s.ui.panel_hidden => match st.s.ui.panel {
            Panel::Console => &[
                ("⏎", "run"),
                ("Tab", "complete"),
                ("↑ ↓", "history"),
                ("Esc", "back to the code"),
            ],
            Panel::Problems => &[("↑ ↓", "choose"), ("⏎", "go to it"), ("Esc", "back to the code")],
            Panel::Agent => &[("⏎", "ask"), ("Esc", "back to the code"), ("Ctrl `", "hide the panel")],
        },
        // An agent's lesson: its keys first, and Esc while its note shows.
        Tab::Editor if st.s.ui.coach.lesson.is_some() && st.s.ui.coach.bubble_at.is_some() => &[
            ("Alt →", "next step"),
            ("Alt ←", "back"),
            ("Esc", "close the note"),
            ("Ctrl S", "save and reload"),
            ("Ctrl Z", "undo"),
        ],
        Tab::Editor if st.s.ui.coach.lesson.is_some() => &[
            ("Alt →", "next step"),
            ("Alt ←", "back"),
            ("Ctrl S", "save and reload"),
            ("Ctrl Z", "undo"),
        ],
        Tab::Editor => &[
            ("Ctrl S", "save and reload"),
            ("Ctrl ⏎", "run"),
            ("Ctrl Space", "suggest"),
            ("Ctrl `", "panel"),
            ("Ctrl Z", "undo"),
            ("Ctrl /", "comment"),
        ],
        Tab::Library => &[
            ("⏎", "edit"),
            ("Space", "on or off"),
            ("R", "reload"),
            ("N", "new mod"),
            (".", "menu"),
        ],
        Tab::Docs => &[("↑ ↓", "scroll"), ("PgUp PgDn", "page")],
        Tab::Settings => &[("↑ ↓", "scroll"), ("Esc", "close")],
    };
    let mut hx = x;
    for (keys, what) in hints {
        if hx > x + w - 120.0 {
            break;
        }
        hx += keycap(p, keys, what, hx, cy) + 18.0;
    }
    let tabs = "Ctrl 1-5";
    let tw = p.measure(tabs, 11.0, Font::Mono) + 12.0 + 6.0 + p.measure("tabs", 12.0, Font::Regular);
    if hx + tw < x + w {
        keycap(p, tabs, "tabs", x + w - tw, cy);
    }
}

#[allow(clippy::too_many_arguments)]
fn line_edit(p: &mut P, now: f64, e: &LineEdit, x: f32, y: f32, max: f32, size: f32, font: Font, c: u32) -> usize {
    edit_line(p, now, e, [x, y, max], size, font, c, true)
}

/// A line being edited in `[x, y, max width]`, scrolled to keep the cursor
/// in view, with the cursor when `caret`. Returns the first byte shown.
#[allow(clippy::too_many_arguments)]
fn edit_line(
    p: &mut P,
    now: f64,
    e: &LineEdit,
    [x, y, max]: [f32; 3],
    size: f32,
    font: Font,
    c: u32,
    caret: bool,
) -> usize {
    let mut start = 0;
    while start < e.cursor && p.measure(&e.text[start..e.cursor], size, font) > max - 6.0 {
        start += e.text[start..].chars().next().map_or(1, char::len_utf8);
    }
    let shown = p.fit(&e.text[start..], max, size, font);
    p.text(&shown, x, y, size, font, c);
    if caret {
        let blink = 0.55 + 0.45 * (now as f32 * std::f32::consts::PI * 1.6).cos();
        let cx = x + p.measure(&e.text[start..e.cursor], size, font);
        let line = (size * 1.45).round();
        p.rect([cx.floor(), y + 1.0, 2.0, line - 2.0], 1.0, fade(pal().accent, blink));
    }
    start
}

fn log_color(kind: LogKind) -> u32 {
    match kind {
        LogKind::Input => pal().ink,
        LogKind::Output => pal().ink_soft,
        LogKind::Error => pal().danger,
        LogKind::Trace => pal().muted,
        LogKind::Info => mix(pal().ink_soft, pal().accent, 0.35),
    }
}

fn draw_console(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    p.rect([x, y, w, h], 14.0, pal().plate_soft);
    let pad = 18.0;
    let field_h = 44.0;
    let log_top = y + pad;
    let log_bottom = y + h - pad - field_h - 10.0;
    draw_log(st, p, [x + pad, log_top, w - 2.0 * pad, log_bottom - log_top], true, dt);
    if st.s.log.is_empty() {
        p.text(
            "Nothing yet. Try player.pos() or help().",
            x + pad,
            top(log_bottom - 12.0, 13.0),
            13.0,
            Font::Regular,
            pal().muted,
        );
    }
    // The input.
    let f = [x + 10.0, y + h - 10.0 - field_h, w - 20.0, field_h];
    p.rect(f, 12.0, pal().page);
    p.border(f, 12.0, fade(pal().accent, 0.45));
    st.s.ui
        .hits
        .push((f, Action::Spot(Spot::Field(super::hstudio_ui::FIELD_CONSOLE))));
    let cy = f[1] + field_h / 2.0;
    p.text("›", f[0] + 16.0, top(cy, 16.0), 16.0, Font::Mono, pal().accent);
    let input = st.s.ui.input.clone();
    let tx = f[0] + 36.0;
    let ty = top(cy, 14.0);
    if input.text.is_empty() {
        p.text(
            "Type Lua and press Enter. Tab completes, help() lists the API.",
            tx,
            ty,
            14.0,
            Font::Regular,
            pal().muted,
        );
    }
    let start = line_edit(p, now, &input, tx, ty, f[2] - 52.0, 14.0, Font::Mono, pal().ink);
    console_suggest(st, p, &input, [tx, f[1], field_h], start, 14.0, dt);
}

/// The console's suggestions, over its input (`[x, top, height]` of the
/// line, `shown` its first byte drawn).
fn console_suggest(
    st: &mut Studio,
    p: &mut P,
    input: &LineEdit,
    [tx, fy, fh]: [f32; 3],
    shown: usize,
    size: f32,
    dt: f32,
) {
    let Some(start) = st.s.ui.suggest.as_ref().filter(|s| s.console).map(|s| s.start) else {
        return;
    };
    let start = start.min(input.cursor);
    let ax = if start >= shown && input.text.is_char_boundary(start) {
        tx + p.measure(&input.text[shown..start], size, Font::Mono)
    } else {
        tx
    };
    let typed = input.text.get(start..input.cursor).unwrap_or("").to_owned();
    draw_suggest(st, p, [ax, fy, fh], &typed, true, dt);
}

/// The log's last lines in `[x, y, w, h]`, the newest at the bottom; with
/// `scrolled` as far back as the console is scrolled (the executors'
/// output shows the newest).
/// A wrapped piece of a console line: its text, colour, when written, whether
/// it starts an input line, its key for selecting and what followed it.
type LogRow = (String, u32, f64, bool, (u64, usize), &'static str);

pub(super) fn draw_log(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], scrolled: bool, dt: f32) {
    let now = st.s.now;
    let log_bottom = y + h;
    if h < code_line() {
        return;
    }
    let rows = (h / code_line()).max(1.0) as usize;
    let scroll = if scrolled { st.s.ui.log_scroll } else { 0 };
    let need = rows + scroll + 2;
    let text_w = w - 20.0;
    st.s.ui.text_areas.push((super::hstudio_ui::AREA_LOG, [x, y, w, h]));
    st.s.ui
        .hits
        .push(([x, y, w, h], Action::Spot(Spot::Text(super::hstudio_ui::AREA_LOG))));
    // Each piece with its line's number (log_seq counts every line written) and place, for selecting.
    let mut shown: Vec<LogRow> = Vec::new();
    let newest = st.s.log_seq;
    for (r, line) in st.s.log.iter().rev().enumerate() {
        let (body, prompt) = match line.text.strip_prefix("> ") {
            Some(rest) if line.kind == LogKind::Input => (rest, true),
            _ => (line.text.as_str(), false),
        };
        let pieces = p.wrap_sep(body, text_w, code_size(), Font::Mono);
        let n = pieces.len();
        let seq = newest.saturating_sub(r as u64);
        for (i, (piece, after)) in pieces.into_iter().enumerate().rev() {
            shown.push((
                piece,
                log_color(line.kind),
                line.at,
                prompt && i == 0 && n > 0,
                (seq, i),
                after,
            ));
        }
        if shown.len() >= need {
            break;
        }
    }
    let skip = scroll.min(shown.len().saturating_sub(rows));
    // Scrolling back glides, a line at a time.
    let back = if scrolled {
        st.s.ui.page = rows;
        st.s.ui.log_scroll = skip;
        smooth(&mut st.s.ui.smooth[0], skip as f32, dt);
        st.s.ui.smooth[0]
    } else {
        0.0
    };
    let first = back.floor().max(0.0) as usize;
    let frac = back - first as f32;
    let (base, dy) = (p.alpha, p.dy);
    for (i, (line, c, at, prompt, key, after)) in shown.iter().skip(first).take(rows + 1).enumerate() {
        let ly = log_bottom - code_line() * (i as f32 + 1.0 - frac);
        if ly < y - 0.5 || ly + code_line() > log_bottom + 0.5 {
            continue;
        }
        let t = progress(now, *at, QUICK);
        p.alpha = base * t;
        p.dy = dy + (1.0 - t) * 4.0;
        let ty = ly + (code_line() - (code_size() * 1.45).round()) / 2.0;
        if *prompt {
            p.text("›", x, ty, code_size(), Font::Mono, pal().accent);
        }
        text_row(
            &mut st.s.ui,
            p,
            super::hstudio_ui::AREA_LOG,
            *key,
            line,
            after,
            [x + 20.0, ly, code_line()],
            code_size(),
            Font::Mono,
        );
        p.text(line, x + 20.0, ty, code_size(), Font::Mono, *c);
    }
    p.alpha = base;
    p.dy = dy;
    if skip > 0 {
        let more = format!("{skip} below");
        let mw = p.measure(&more, 11.0, Font::Mono) + 16.0;
        p.rect([x + w - mw, log_bottom - 22.0, mw, 20.0], 10.0, fade(pal().ink, 0.10));
        p.text(
            &more,
            x + w - mw + 8.0,
            top(log_bottom - 12.0, 11.0),
            11.0,
            Font::Mono,
            pal().ink_soft,
        );
    }
}

pub(super) fn tint(t: Tint) -> u32 {
    match t {
        Tint::Plain => pal().ink,
        Tint::Keyword => pal().keyword,
        Tint::Str => pal().string,
        Tint::Number => pal().number,
        Tint::Comment => pal().comment,
        Tint::Global => pal().global,
    }
}

/// The bottom panel's tab strip, and the bar between it and the code.
const STRIP: f32 = 32.0;
const SPLIT: f32 = 8.0;
/// The least the code keeps of the editor's height with the panel open,
/// and the least the panel takes (room allowing) to show a few lines.
const CODE_MIN: f32 = 100.0;
const PANEL_MIN: f32 = 150.0;

/// The editor's page, as in VS Code: the file's name and its problems, Save
/// and Run, the code, and under it the panel (the console, the problems,
/// the agent), sized by the bar between them.
fn draw_editor(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let title = st.s.editor.title.clone();
    let tmax = (w * 0.45).max(80.0);
    let title = p.fit(&title, tmax, 15.0, Font::Medium);
    let tw = p.text(&title, x, top(y + 16.0, 15.0), 15.0, Font::Medium, pal().ink);
    st.s.ui.hits.push(([x, y, tw + 22.0, 32.0], Action::Spot(Spot::File)));
    let dirty = glide(&mut st.s.ui, id("dirty", 0), st.s.editor.dirty, dt);
    if dirty > 0.01 {
        let d = 7.0 * dirty;
        p.rect(
            [x + tw + 13.5 - d / 2.0, y + 16.5 - d / 2.0, d, d],
            d / 2.0,
            pal().accent,
        );
    }
    let mut bx = x + w;
    {
        let rw = p.measure("Run", 13.0, Font::Medium) + 28.0;
        bx -= rw;
        pill(
            &mut st.s.ui,
            p,
            "run",
            "Run",
            bx,
            y,
            32.0,
            Style::Primary,
            Action::Run,
            dt,
        );
        let sw = p.measure("Save", 13.0, Font::Medium) + 28.0;
        bx -= sw + 8.0;
        pill(
            &mut st.s.ui,
            p,
            "save",
            "Save",
            bx,
            y,
            32.0,
            Style::Secondary,
            Action::Save,
            dt,
        );
    }
    // Its problems at a glance; a click shows them.
    let mut px = x + tw + 28.0;
    let (errors, warnings) = counts(&st.s.editor.problems);
    if errors + warnings > 0 && px + 90.0 < bx - 16.0 {
        let shown: Vec<(String, u32)> = [(errors, pal().danger), (warnings, warn())]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, c)| (n.to_string(), c))
            .collect();
        let cw: f32 = shown
            .iter()
            .map(|(l, _)| 12.0 + p.measure(l, 12.0, Font::Medium) + 10.0)
            .sum();
        let chip = [px - 8.0, y + 4.0, cw + 6.0, 24.0];
        let ui = &mut st.s.ui;
        let hv = hover(ui, id("problem-chip", 0), inside(chip, ui.mouse), dt);
        p.rect(chip, 12.0, fade(pal().ink, 0.04 + 0.06 * hv));
        let mut cx = px;
        for (label, c) in shown {
            p.rect([cx, y + 12.5, 7.0, 7.0], 3.5, c);
            cx +=
                12.0 + p.text(
                    &label,
                    cx + 12.0,
                    top(y + 16.0, 12.0),
                    12.0,
                    Font::Medium,
                    mix(pal().ink_soft, pal().ink, hv),
                ) + 10.0;
        }
        ui.hits.push((chip, Action::PanelTab(Panel::Problems)));
        px += cw + 14.0;
    }
    let chip = failed_chip(st, p, px - 8.0, y + 4.0, bx - 16.0, dt);
    if chip > 0.0 {
        px += chip + 6.0;
    }
    // Agents' notes in the code.
    let chip = coach_chip(st, p, px - 8.0, y + 4.0, bx - 16.0, dt);
    if chip > 0.0 {
        px += chip + 6.0;
    }
    if let Some(problem) = st.s.editor.problem.clone() {
        let msg = p.fit(
            &format!("Could not read the file: {problem}"),
            bx - px - 16.0,
            13.0,
            Font::Regular,
        );
        p.text(&msg, px, top(y + 16.0, 13.0), 13.0, Font::Regular, pal().danger);
    }
    let by = y + 46.0;
    let bh = h - 46.0;
    // The scripts at the left, as VS Code's explorer, when there is room.
    let (x, w) = if w >= 640.0 {
        let sw = (w * 0.18).clamp(150.0, 210.0).floor();
        draw_scripts(st, p, [x, by, sw, bh], dt);
        (x + sw + 10.0, w - sw - 10.0)
    } else {
        (x, w)
    };
    // The panel's height glides to its share of the page (none while
    // hidden); sizing it, and the window, follow the pointer at once.
    let ui = &mut st.s.ui;
    if ui.panel_h <= 0.0 {
        ui.panel_h = 0.4;
    }
    ui.panel_room = bh;
    let room = (bh - CODE_MIN - SPLIT).max(0.0);
    let open = !ui.panel_hidden && room >= STRIP + 60.0;
    let want = if open {
        (bh * ui.panel_h).clamp(PANEL_MIN.min(room), room)
    } else {
        0.0
    };
    if ui.panel_drag.is_some() || ui.drag.is_some() {
        ui.smooth[4] = want;
    } else {
        smooth(&mut ui.smooth[4], want, dt);
    }
    let ph = ui.smooth[4].min(room).round();
    if ph >= 2.0 {
        let gap = SPLIT * (ph / STRIP).min(1.0);
        let code_h = bh - ph - gap;
        let sy = by + code_h;
        let split = [x, sy, w, gap];
        let held = ui.panel_drag.is_some();
        let hv = hover(ui, id("splitter", 0), held || inside(split, ui.mouse), dt);
        let c = if held {
            pal().accent
        } else {
            fade(pal().ink, 0.14 + 0.22 * hv)
        };
        let bw = 36.0 + 24.0 * hv;
        p.rect([x + (w - bw) / 2.0, sy + gap / 2.0 - 1.5, bw, 3.0], 1.5, c);
        ui.hits.push((split, Action::PanelResize));
        draw_panel(st, p, [x, sy + gap, w, ph], dt);
        draw_code(st, p, [x, by, w, code_h], dt);
    } else {
        ui.panel_at = None;
        draw_code(st, p, [x, by, w, bh], dt);
    }
}

/// When the module of the editor's file failed in another of its files:
/// that file's path, and what its chip says ("Failed in lib.lua:5").
fn failed_elsewhere(st: &Studio) -> Option<(std::path::PathBuf, String)> {
    let i = st.s.editor.path.as_deref().and_then(|path| st.mod_of_path(path))?;
    let m = &st.s.mods[i];
    let folder = m.folder.as_deref()?;
    let (file, line) = st.failure_place(i)?;
    if !m.files().contains(&file) || m.chunk_of(&file) == st.s.editor.title {
        return None;
    }
    Some((
        super::hstudio_modfiles::path_in(folder, &file),
        format!("Failed in {file}:{line}"),
    ))
}

/// How wide the Failed in chip is, when there is one to draw.
pub(super) fn failed_chip_width(st: &Studio, p: &mut P) -> Option<f32> {
    let (_, label) = failed_elsewhere(st)?;
    Some(p.measure(&label, 12.0, Font::Medium) + 34.0)
}

/// Where the module of the editor's file failed, when that is another of
/// its files: "Failed in lib.lua:5", a chip 24 high at (x, y) that a click
/// opens there, drawn when it ends before `right`. Returns its width (0
/// when it isn't drawn). Every layout has it by its editor.
pub(super) fn failed_chip(st: &mut Studio, p: &mut P, x: f32, y: f32, right: f32, dt: f32) -> f32 {
    let Some((path, label)) = failed_elsewhere(st) else {
        return 0.0;
    };
    let lw = p.measure(&label, 12.0, Font::Medium);
    let chip = [x, y, lw + 34.0, 24.0];
    if chip[0] + chip[2] >= right {
        return 0.0;
    }
    let ui = &mut st.s.ui;
    let hv = hover(ui, id("failed-chip", 0), inside(chip, ui.mouse), dt);
    p.rect(chip, 12.0, fade(pal().danger, 0.12 + 0.08 * hv));
    p.rect([x + 8.0, y + 8.5, 7.0, 7.0], 3.5, pal().danger);
    p.text(&label, x + 22.0, top(y + 12.0, 12.0), 12.0, Font::Medium, pal().danger);
    edit_file_hit(ui, chip, path);
    chip[2]
}

/// A click in `r` that opens `path`, a file of a mod (`Action::EditFile`).
pub(super) fn edit_file_hit(ui: &mut super::hstudio_ui::Ui, r: [f32; 4], path: std::path::PathBuf) {
    let k = ui.drawn_files.len().min(65535) as u16;
    ui.drawn_files.push(path);
    ui.hits.push((r, Action::EditFile(k)));
}

/// A small filled triangle of 1 dp rects in a 7 x 7 box at (x, y),
/// pointing down for an open folder, right for a folded one.
pub(super) fn chevron(p: &mut P, x: f32, y: f32, open: bool, c: u32) {
    for i in 0..4 {
        let f = i as f32;
        if open {
            p.rect([x + f, y + 1.5 + f, 7.0 - 2.0 * f, 1.0], 0.0, c);
        } else {
            p.rect([x + 1.5 + f, y + f, 1.0, 7.0 - 2.0 * f], 0.0, c);
        }
    }
}

/// A mod's state as its dot's colour.
pub(super) fn state_dot(state: &ModState) -> u32 {
    match state {
        ModState::Running => pal().success,
        ModState::Failed(..) => pal().danger,
        ModState::Off => pal().muted,
    }
}

/// The script list beside the code, as VS Code's explorer: the scratch
/// file, then each mod with its state's dot, a script as one row and a
/// module as a folder that folds, its files under it. A click opens a file
/// or folds a module; the row under the pointer (and the open one) shows
/// "..." for its menu, which a right press opens too, and a module's row
/// has a "+" for a new file of it. The "+" at the top makes a script or a
/// module. The wheel scrolls it, and it brings the open file's row into
/// view.
fn draw_scripts(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    use super::hstudio_ui::{ScriptRow, open_row, row_label, script_rows, scripts_first};
    p.rect([x, y, w, h], 14.0, pal().plate_soft);
    // Its own menu, under its rows' and its buttons.
    st.s.ui.hits.push(([x, y, w, h], Action::Spot(Spot::Scripts)));
    p.text(
        "Scripts",
        x + 14.0,
        top(y + 18.0, 11.0),
        11.0,
        Font::Medium,
        pal().muted,
    );
    let plus = [x + w - 32.0, y + 6.0, 24.0, 24.0];
    {
        let ui = &mut st.s.ui;
        let hv = hover(ui, id("scripts-new", 0), inside(plus, ui.mouse), dt);
        p.rect(plus, 8.0, fade(pal().ink, 0.08 * hv));
        let pw = p.measure("+", 15.0, Font::Medium);
        p.text(
            "+",
            plus[0] + (plus[2] - pw) / 2.0,
            top(y + 18.0, 15.0),
            15.0,
            Font::Medium,
            mix(pal().muted, pal().ink, hv),
        );
        ui.hits.push((plus, Action::NewMenu));
        ui.new_button = Some(plus);
    }
    let row_h = 26.0;
    let list_y = y + 36.0;
    let rows = (((h - 44.0) / row_h).floor() as usize).max(1);
    let all = script_rows(st);
    let open = open_row(st, &all);
    let first = scripts_first(st, all.len(), rows, open);
    st.s.ui.scripts_at = Some([x, list_y, w, row_h * rows as f32]);
    for (k, row) in all.iter().enumerate().skip(first).take(rows) {
        let r = [x + 6.0, list_y + row_h * (k - first) as f32, w - 12.0, row_h - 2.0];
        let label = row_label(st, row);
        let ui = &mut st.s.ui;
        let over = inside(r, ui.mouse);
        let hv = hover(ui, id(&label, k), over, dt);
        let on = open == Some(k);
        let bg = if on {
            fade(pal().accent, 0.16)
        } else {
            fade(pal().ink, 0.06 * hv)
        };
        p.rect(r, 8.0, bg);
        let cy = r[1] + r[3] / 2.0;
        let (enabled, state) = match row {
            ScriptRow::Scratch => (true, None),
            ScriptRow::Script { i, .. } | ScriptRow::Module { i, .. } | ScriptRow::File { i, .. } => {
                st.s.mods
                    .get(*i)
                    .map_or((true, None), |m| (m.enabled, Some(m.state.clone())))
            }
        };
        let fg = if enabled { pal().ink_soft } else { pal().muted };
        let tx = match row {
            ScriptRow::Scratch | ScriptRow::Script { .. } => {
                let dot = state.as_ref().map_or(pal().muted, state_dot);
                p.rect([r[0] + 10.0, cy - 3.0, 6.0, 6.0], 3.0, dot);
                r[0] + 24.0
            }
            ScriptRow::Module { open: unfolded, .. } => {
                chevron(p, r[0] + 7.0, cy - 3.5, *unfolded, mix(pal().muted, pal().ink, hv));
                let dot = state.as_ref().map_or(pal().muted, state_dot);
                p.rect([r[0] + 19.0, cy - 3.0, 6.0, 6.0], 3.0, dot);
                r[0] + 31.0
            }
            // A module's files, under its name.
            ScriptRow::File { .. } => r[0] + 38.0,
        };
        // Its buttons at the right, under the pointer and on the open row.
        let mut bx = r[0] + r[2];
        let mut buttons: Vec<([f32; 4], &str, Action)> = Vec::new();
        if (over || on) && !matches!(row, ScriptRow::Scratch) {
            bx -= 22.0;
            buttons.push((
                [bx, r[1] + 2.0, 20.0, r[3] - 4.0],
                "...",
                Action::ScriptMenu(k.min(65535) as u16),
            ));
            if let ScriptRow::Module { i, .. } = row {
                bx -= 20.0;
                buttons.push(([bx, r[1] + 2.0, 20.0, r[3] - 4.0], "+", Action::NewFileIn(*i)));
            }
        }
        if on && st.s.editor.dirty {
            bx -= 12.0;
            p.rect([bx, cy - 3.0, 6.0, 6.0], 3.0, pal().accent);
        }
        let name = p.fit(&label, (bx - tx - 6.0).max(12.0), 12.5, Font::Regular);
        let fg = if on { pal().ink } else { mix(fg, pal().ink, hv) };
        p.text(&name, tx, top(cy, 12.5), 12.5, Font::Regular, fg);
        let ui = &mut st.s.ui;
        ui.hits.push((r, Action::ScriptRow(k.min(65535) as u16)));
        ui.hits.push((r, Action::Spot(Spot::Row(k.min(65535) as u16))));
        for (b, glyph, action) in buttons {
            let bh = hover(ui, id(glyph, k), inside(b, ui.mouse), dt);
            p.rect(b, 6.0, fade(pal().ink, 0.10 * bh));
            let gw = p.measure(glyph, 12.5, Font::Medium);
            p.text(
                glyph,
                b[0] + (b[2] - gw) / 2.0,
                top(cy, 12.5),
                12.5,
                Font::Medium,
                mix(pal().muted, pal().ink, bh),
            );
            ui.hits.push((b, action));
        }
    }
    st.s.ui.script_rows = all;
}

/// How many of `problems` are errors, and how many warnings.
fn counts(problems: &[super::hstudio_assist::Problem]) -> (usize, usize) {
    let errors = problems.iter().filter(|p| p.error).count();
    (errors, problems.len() - errors)
}

/// The bottom panel in `r`: its tabs, what each tab shows.
fn draw_panel(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let ui = &mut st.s.ui;
    ui.panel_at = Some([x, y, w, h]);
    let focus = glide(ui, id("panel-focus", 0), ui.panel_focus, dt);
    p.rect([x, y, w, h], 14.0, pal().plate_soft);
    p.border([x, y, w, h], 14.0, fade(pal().accent, 0.35 * focus));
    ui.hits.push(([x, y, w, h], Action::FocusPanel));
    // The tab shown's menu, under what its tab's content marks.
    ui.hits.push(([x, y, w, h], Action::Spot(Spot::Panel(ui.panel))));
    let (errors, warnings) = counts(&st.s.editor.problems);
    let busy = super::hstudio_ui::agent_busy(st);
    let ui = &mut st.s.ui;
    let cy = y + STRIP / 2.0 + 1.0;
    let mut tx = x + 8.0;
    let mut active = [tx, 0.0];
    for (panel, label) in [
        (Panel::Console, "Console"),
        (Panel::Problems, "Problems"),
        (Panel::Agent, "Agent"),
    ] {
        let lw = p.measure(label, 12.0, Font::Medium);
        let badge = match panel {
            Panel::Problems if errors + warnings > 0 => {
                let n = (errors + warnings).to_string();
                p.measure(&n, 10.5, Font::Medium) + 18.0
            }
            Panel::Agent if busy => 14.0,
            _ => 0.0,
        };
        let r = [tx, y + 4.0, lw + 24.0 + badge, STRIP - 6.0];
        let on = ui.panel == panel;
        let hv = hover(ui, id("panel-tab", panel as usize), inside(r, ui.mouse), dt);
        p.rect(r, 9.0, fade(pal().ink, 0.07 * hv));
        let c = if on { pal().ink } else { mix(pal().muted, pal().ink, hv) };
        p.text(label, tx + 12.0, top(cy, 12.0), 12.0, Font::Medium, c);
        let bx = tx + 12.0 + lw + 6.0;
        match panel {
            Panel::Problems if errors + warnings > 0 => {
                let n = (errors + warnings).to_string();
                let nw = p.measure(&n, 10.5, Font::Medium) + 10.0;
                let bc = if errors > 0 { pal().danger } else { warn() };
                p.rect([bx, cy - 8.0, nw, 16.0], 8.0, fade(bc, 0.18));
                p.text(&n, bx + 5.0, top(cy, 10.5), 10.5, Font::Medium, bc);
            }
            Panel::Agent if busy => {
                let pulse = 0.45 + 0.55 * (st.s.now as f32 * 4.0).sin().abs();
                p.rect([bx + 1.0, cy - 3.5, 7.0, 7.0], 3.5, fade(pal().accent, pulse));
            }
            _ => {}
        }
        if on {
            active = [r[0] + 8.0, r[2] - 16.0];
        }
        ui.hits.push((r, Action::PanelTab(panel)));
        ui.hits.push((r, Action::Spot(Spot::Panel(panel))));
        tx += r[2] + 2.0;
    }
    // The line under the tab shown glides to it.
    let key = id("panel-tab-line", 0);
    let mut lx = ui.anim.get(&key).copied().unwrap_or(active[0] - x);
    smooth(&mut lx, active[0] - x, dt);
    ui.anim.insert(key, lx);
    let wkey = id("panel-tab-width", 0);
    let mut lw = ui.anim.get(&wkey).copied().unwrap_or(active[1]);
    smooth(&mut lw, active[1], dt);
    ui.anim.insert(wkey, lw);
    let line_c = mix(pal().ink_soft, pal().accent, focus);
    p.rect([x + lx, y + STRIP - 3.0, lw, 2.0], 1.0, line_c);
    p.rect([x + 10.0, y + STRIP - 1.0, w - 20.0, 1.0], 0.0, fade(pal().rule, 0.8));
    // At the right: the tab's own buttons, and hiding the panel.
    let mut rx = x + w - 8.0;
    let hide = [rx - 26.0, y + 4.0, 26.0, STRIP - 6.0];
    let hv = hover(ui, id("panel-hide", 0), inside(hide, ui.mouse), dt);
    p.rect(hide, 9.0, fade(pal().ink, 0.08 * hv));
    let xw = p.measure("×", 16.0, Font::Regular);
    p.text(
        "×",
        hide[0] + (26.0 - xw) / 2.0,
        top(cy - 1.0, 16.0),
        16.0,
        Font::Regular,
        mix(pal().muted, pal().ink, hv),
    );
    ui.hits.push((hide, Action::TogglePanel));
    rx = hide[0] - 4.0;
    let panel = ui.panel;
    let name = super::hstudio_ui::agent_name(st);
    let ui = &mut st.s.ui;
    let button = |ui: &mut Ui, p: &mut P, key: &str, label: &str, action: Action, rx: &mut f32| -> Option<[f32; 4]> {
        let bw = p.measure(label, 12.0, Font::Medium) + 20.0;
        if *rx - bw < tx + 12.0 {
            return None;
        }
        *rx -= bw;
        let r = [*rx, y + 5.0, bw, STRIP - 8.0];
        let hv = hover(ui, id(key, 0), inside(r, ui.mouse), dt);
        let danger = action == Action::AgentStop;
        let c = if danger { pal().danger } else { pal().ink };
        p.rect(r, (STRIP - 8.0) / 2.0, fade(c, 0.07 + 0.09 * hv));
        p.text(
            label,
            *rx + 10.0,
            top(cy, 12.0),
            12.0,
            Font::Medium,
            if danger { c } else { mix(pal().ink_soft, c, hv) },
        );
        ui.hits.push((r, action));
        *rx -= 6.0;
        Some(r)
    };
    match panel {
        Panel::Console => {
            button(ui, p, "panel-clear", "Clear", Action::ClearLog, &mut rx);
        }
        Panel::Agent => {
            if busy {
                button(ui, p, "agent-stop", "Stop", Action::AgentStop, &mut rx);
            }
            let label = name.clone().unwrap_or_else(|| "Look for an agent".into());
            button(ui, p, "agent-next", &label, Action::AgentNext, &mut rx);
            // Its model, for an agent that takes one.
            let id = super::hstudio_ui::agent_id(st);
            let chip = match id.filter(|i| !super::hstudio_ui::model_choices(i).is_empty()) {
                Some(id) => {
                    let model = super::hstudio_ui::agent_model(st);
                    let label = format!("Model: {}", super::hstudio_ui::model_label(&id, model.as_deref()));
                    button(&mut st.s.ui, p, "agent-model", &label, Action::ModelMenu, &mut rx)
                }
                None => None,
            };
            st.s.ui.model_chip = chip;
        }
        Panel::Problems => {}
    }
    // What the tab shows, coming in as the panel opens.
    let body = [x + 14.0, y + STRIP + 5.0, w - 28.0, h - STRIP - 10.0];
    if body[3] < 18.0 {
        return;
    }
    let base = p.alpha;
    let shown = progress(st.s.now, st.s.ui.tab_at, MOVE) * progress(st.s.now, st.s.ui.panel_tab_at, MOVE);
    p.alpha = base * ((h - STRIP) / 60.0).clamp(0.0, 1.0) * shown;
    p.dy += (1.0 - shown) * RISE * 0.5;
    match panel {
        Panel::Console => panel_console(st, p, body, dt),
        Panel::Problems => panel_problems(st, p, body, dt),
        Panel::Agent => panel_agent(st, p, body, name.as_deref(), dt),
    }
    p.alpha = base;
    p.dy -= (1.0 - shown) * RISE * 0.5;
}

/// The console in the panel: the same log and input as the Console tab.
fn panel_console(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    let field_h = 30.0;
    let page = st.s.ui.page;
    draw_log(st, p, [x, y, w, h - field_h - 6.0], true, dt);
    st.s.ui.page = page;
    let focused = st.s.ui.panel_focus && st.s.ui.panel == Panel::Console;
    let f = [x - 4.0, y + h - field_h, w + 8.0, field_h];
    let fo = glide(&mut st.s.ui, id("panel-input", 0), focused, dt);
    p.rect(f, 9.0, pal().page);
    p.border(f, 9.0, mix(pal().rule, fade(pal().accent, 0.55), fo));
    st.s.ui
        .hits
        .push((f, Action::Spot(Spot::Field(super::hstudio_ui::FIELD_CONSOLE))));
    let cy = f[1] + field_h / 2.0;
    let size = code_size();
    p.text("›", f[0] + 10.0, top(cy, size), size, Font::Mono, pal().accent);
    let input = st.s.ui.input.clone();
    let tx = f[0] + 26.0;
    let ty = top(cy, size);
    if input.text.is_empty() {
        let hint = if focused {
            "Lua to run. Tab completes, Esc goes back to the code."
        } else {
            "Click to run Lua here."
        };
        let hint = p.fit(hint, f[2] - 40.0, 12.5, Font::Regular);
        p.text(&hint, tx, top(cy, 12.5), 12.5, Font::Regular, pal().muted);
    }
    let start = edit_line(
        p,
        now,
        &input,
        [tx, ty, f[2] - 36.0],
        size,
        Font::Mono,
        pal().ink,
        focused,
    );
    if focused {
        console_suggest(st, p, &input, [tx, f[1], field_h], start, size, dt);
    }
}

/// The problems the live check found: a row each, a click goes to it.
fn panel_problems(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    let n = st.s.editor.problems.len();
    if n == 0 {
        let checked = st.s.editor.checked == Some(st.s.editor.revision);
        let (label, c) = if checked {
            ("No problems found in this file.", pal().success)
        } else {
            ("Checking…", pal().muted)
        };
        p.rect([x + 2.0, y + 9.0, 8.0, 8.0], 4.0, c);
        p.text(
            label,
            x + 18.0,
            top(y + 13.0, 13.0),
            13.0,
            Font::Regular,
            pal().ink_soft,
        );
        return;
    }
    const ROW: f32 = 26.0;
    let rows = ((h / ROW).floor() as usize).max(1);
    let ui = &mut st.s.ui;
    ui.problem_sel = ui.problem_sel.min(n - 1);
    let first_key = id("problem-first", 0);
    let mut first = ui.anim.get(&first_key).copied().unwrap_or(0.0) as usize;
    if ui.problem_sel < first {
        first = ui.problem_sel;
    } else if ui.problem_sel >= first + rows {
        first = ui.problem_sel + 1 - rows;
    }
    first = first.min(n.saturating_sub(rows));
    ui.anim.insert(first_key, first as f32);
    let focused = ui.panel_focus && ui.panel == Panel::Problems;
    let base = p.alpha;
    let problems = st.s.editor.problems.clone();
    let lines = st.s.editor.lines.clone();
    for (k, pr) in problems.iter().enumerate().skip(first).take(rows) {
        let ry = y + (k - first) as f32 * ROW;
        if ry + ROW > y + h + 0.5 {
            break;
        }
        let r = [x - 6.0, ry, w + 12.0, ROW - 2.0];
        let ui = &mut st.s.ui;
        let at = ui.problem_at.get(&problem_key(pr, &lines)).copied().unwrap_or(0.0);
        p.alpha = base * progress(now, at, MOVE);
        let hv = hover(ui, id("problem-row", k), inside(r, ui.mouse), dt);
        let sel = glide(ui, id("problem-sel", k), k == ui.problem_sel && focused, dt);
        p.rect(r, 8.0, fade(pal().ink, 0.05 * hv + 0.07 * sel));
        let c = if pr.error { pal().danger } else { warn() };
        p.rect([x + 2.0, ry + ROW / 2.0 - 5.0, 8.0, 8.0], 4.0, c);
        let place = format!("line {}", pr.line);
        let lw = p.measure(&place, 11.5, Font::Mono);
        p.text(
            &place,
            x + w - lw,
            top(ry + ROW / 2.0 - 1.0, 11.5),
            11.5,
            Font::Mono,
            pal().muted,
        );
        let msg = p.fit(&pr.message, w - lw - 40.0, 13.0, Font::Regular);
        p.text(
            &msg,
            x + 18.0,
            top(ry + ROW / 2.0 - 1.0, 13.0),
            13.0,
            Font::Regular,
            mix(pal().ink_soft, pal().ink, hv.max(sel)),
        );
        ui.hits.push((r, Action::Problem(k)));
        ui.hits.push((r, Action::Spot(Spot::Problem(k))));
    }
    p.alpha = base;
    if n > rows {
        let more = format!("{} more", n - rows);
        let _ = more;
        let track = h;
        let bar = (track * rows as f32 / n as f32).max(16.0);
        let by = y + (track - bar) * (first as f32 / (n - rows) as f32);
        p.rect([x + w + 6.0, by, 3.0, bar], 1.5, fade(pal().ink, 0.18));
    }
}

/// What tells a problem apart as the code moves: its message and its
/// line's text, not its line's number.
pub(super) fn problem_key(pr: &super::hstudio_assist::Problem, lines: &[String]) -> u64 {
    let line = lines.get(pr.line.wrapping_sub(1)).map_or("", String::as_str);
    id(
        &format!("{}\u{1}{}", pr.message, line.trim()),
        pr.span.map_or(usize::MAX, |s| s.0),
    )
}

/// One line of the agent's conversation as drawn.
struct Said {
    text: String,
    font: Font,
    size: f32,
    c: u32,
    indent: f32,
    mark: Option<u32>,
    bar: bool,
    at: f64,
    /// Its line's number and place, for selecting (None: a gap).
    key: Option<(u64, usize)>,
    after: &'static str,
    /// On the first row of a line about some code (what an agent did in
    /// the editor): the line's number, for its Show link.
    jump: Option<u64>,
}

/// The agent: the conversation (newest at the bottom), and what to ask it.
fn panel_agent(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], name: Option<&str>, dt: f32) {
    use super::hstudio_ui::AgentLineKind as K;
    let now = st.s.now;
    let field_h = 30.0;
    let log_h = h - field_h - 8.0;
    const ROW: f32 = 19.0;
    let busy = super::hstudio_ui::agent_busy(st);
    // An agent waiting for the player (wait_for_player): its turn is theirs.
    let waiting = super::hstudio_coach::waiting_label(st);
    let visible = (log_h / ROW).floor().max(0.0) as usize;
    let want = visible + st.s.ui.agent.scroll.ceil() as usize + 2;
    let mut rows: Vec<Said> = Vec::new();
    let mut whole = true;
    st.s.ui
        .text_areas
        .push((super::hstudio_ui::AREA_AGENT, [x, y, w, log_h]));
    st.s.ui.hits.push((
        [x, y, w, log_h],
        Action::Spot(Spot::Text(super::hstudio_ui::AREA_AGENT)),
    ));
    let newest = st.s.ui.agent.seq;
    for (r, line) in st.s.ui.agent.lines.iter().rev().enumerate() {
        let seq = newest.saturating_sub(r as u64);
        let (font, size, c, indent, mark, bar) = match line.kind {
            K::You => (Font::Regular, 13.0, pal().ink, 12.0, None, true),
            K::Said => (Font::Regular, 13.0, pal().ink, 0.0, None, false),
            K::Tool => (Font::Mono, 12.0, pal().muted, 14.0, Some(pal().muted), false),
            K::ToolOk => (Font::Mono, 12.0, pal().muted, 14.0, Some(pal().success), false),
            K::ToolFailed => (Font::Mono, 12.0, pal().danger, 14.0, Some(pal().danger), false),
            K::Error => (Font::Regular, 13.0, pal().danger, 0.0, None, false),
            K::Done => (Font::Regular, 11.5, pal().muted, 0.0, None, false),
            K::Action => (Font::Regular, 12.5, pal().ink_soft, 14.0, Some(pal().accent), false),
        };
        // A line about some code keeps room for its Show link.
        let show = if line.jump.is_some() {
            p.measure("Show", 12.0, Font::Medium) + 16.0
        } else {
            0.0
        };
        let pieces = p.wrap_sep(&line.text, w - indent - 8.0 - show, size, font);
        let n = pieces.len();
        for (i, (piece, after)) in pieces.into_iter().enumerate().rev() {
            rows.push(Said {
                text: piece,
                font,
                size,
                c,
                indent,
                mark: if i == 0 { mark } else { None },
                bar: bar && n > 0,
                at: line.at,
                key: Some((seq, i)),
                after,
                jump: line.jump.as_ref().filter(|_| i == 0).map(|_| seq),
            });
        }
        if line.kind == K::You || line.kind == K::Done {
            // A turn's end: a breath before the next.
            rows.push(Said {
                text: String::new(),
                font,
                size,
                c,
                indent: 0.0,
                mark: None,
                bar: false,
                at: line.at,
                key: None,
                after: "",
                jump: None,
            });
        }
        if rows.len() >= want {
            whole = false;
            break;
        }
    }
    if whole {
        let max = rows.len().saturating_sub(visible) as f32;
        st.s.ui.agent.scroll = st.s.ui.agent.scroll.min(max);
    }
    let target = st.s.ui.agent.scroll;
    smooth(&mut st.s.ui.smooth[7], target, dt);
    let scroll = st.s.ui.smooth[7];
    let (base, dy) = (p.alpha, p.dy);
    let working = if busy || waiting.is_some() { 1 } else { 0 };
    if rows.is_empty() && !busy && waiting.is_none() {
        let msg = match name {
            Some(n) => format!(
                "Ask {n} to write or change this script. It sees the open file, what you selected and its problems, and works through Horizon Studio's MCP: it types its changes into the editor as you watch, points at code with notes and teaches with lessons, and Show on its lines here goes to the code."
            ),
            None => "No agent found on this computer. Install Claude Code (claude) or omp, set up the MCP (Copy AI setup prompt), then press Look for an agent.".to_owned(),
        };
        for (i, l) in p.wrap(&msg, w - 8.0, 13.0, Font::Regular).iter().enumerate() {
            let ly = y + 4.0 + i as f32 * ROW;
            if ly + ROW > y + log_h {
                break;
            }
            p.text(l, x, ly, 13.0, Font::Regular, pal().muted);
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let ly = y + log_h - ROW * ((i + working) as f32 + 1.0 - scroll);
        if ly < y - 0.5 || ly + ROW > y + log_h + 0.5 {
            continue;
        }
        let t = progress(now, row.at, ENTER);
        p.alpha = base * t;
        p.dy = dy + (1.0 - t) * RISE * 0.5;
        if row.bar {
            p.rect([x, ly + 2.0, 2.0, ROW - 4.0], 1.0, fade(pal().accent, 0.8));
        }
        if let Some(c) = row.mark {
            p.rect([x + 2.0, ly + ROW / 2.0 - 3.0, 6.0, 6.0], 3.0, c);
        }
        if let Some(key) = row.key {
            text_row(
                &mut st.s.ui,
                p,
                super::hstudio_ui::AREA_AGENT,
                key,
                &row.text,
                row.after,
                [x + row.indent, ly, ROW],
                row.size,
                row.font,
            );
        }
        p.text(
            &row.text,
            x + row.indent,
            top(ly + ROW / 2.0, row.size),
            row.size,
            row.font,
            row.c,
        );
        // What an agent did in the editor: Show goes to that code (the
        // line's text stays for selecting).
        if let Some(seq) = row.jump {
            let lw = p.measure("Show", 12.0, Font::Medium);
            let r = [x + w - lw - 8.0, ly + 1.0, lw + 8.0, ROW - 2.0];
            let ui = &mut st.s.ui;
            let hv = hover(ui, id("agent-show", seq as usize), inside(r, ui.mouse), dt);
            let c = mix(pal().accent, pal().accent_hover, hv);
            p.text("Show", r[0] + 4.0, top(ly + ROW / 2.0, 12.0), 12.0, Font::Medium, c);
            p.rect([r[0] + 4.0, ly + ROW - 4.0, lw, 1.0], 0.0, fade(c, hv));
            ui.hits.push((r, Action::AgentJump(seq)));
        }
    }
    p.alpha = base;
    p.dy = dy;
    if busy || waiting.is_some() {
        let ly = y + log_h - ROW * (1.0 - scroll);
        if ly + ROW <= y + log_h + 0.5 {
            let lw = p.text(
                waiting.as_deref().unwrap_or("Working"),
                x,
                top(ly + ROW / 2.0, 12.5),
                12.5,
                Font::Medium,
                pal().ink_soft,
            );
            for k in 0..3 {
                let a = 0.25 + 0.75 * ((now as f32 * 5.0 - k as f32 * 0.7).sin() * 0.5 + 0.5);
                p.rect(
                    [x + lw + 5.0 + k as f32 * 7.0, ly + ROW / 2.0, 3.5, 3.5],
                    1.75,
                    fade(pal().accent, a),
                );
            }
        }
    }
    if scroll > 0.5 {
        let more = "newer below";
        let mw = p.measure(more, 11.0, Font::Mono) + 16.0;
        p.rect([x + w - mw, y + log_h - 22.0, mw, 20.0], 10.0, fade(pal().ink, 0.10));
        p.text(
            more,
            x + w - mw + 8.0,
            top(y + log_h - 12.0, 11.0),
            11.0,
            Font::Mono,
            pal().ink_soft,
        );
    }
    // What to ask.
    let focused = st.s.ui.panel_focus && st.s.ui.panel == Panel::Agent;
    let f = [x - 4.0, y + h - field_h, w + 8.0, field_h];
    let fo = glide(&mut st.s.ui, id("agent-input", 0), focused, dt);
    p.rect(f, 9.0, pal().page);
    p.border(f, 9.0, mix(pal().rule, fade(pal().accent, 0.55), fo));
    st.s.ui
        .hits
        .push((f, Action::Spot(Spot::Field(super::hstudio_ui::FIELD_AGENT))));
    let cy = f[1] + field_h / 2.0;
    p.text("›", f[0] + 10.0, top(cy, 14.0), 14.0, Font::Mono, pal().accent);
    let input = st.s.ui.agent_input.clone();
    let tx = f[0] + 26.0;
    if input.text.is_empty() {
        let hint = match (name, busy) {
            _ if waiting.is_some() => "It's waiting for you: Enter sends it what you write here.".to_owned(),
            (_, true) => "It's working. Stop ends it; send your next request when it's done.".to_owned(),
            (Some(n), _) => format!("Ask {n} about this script, Enter sends."),
            (None, _) => "No agent found.".to_owned(),
        };
        let hint = p.fit(&hint, f[2] - 40.0, 12.5, Font::Regular);
        p.text(&hint, tx, top(cy, 12.5), 12.5, Font::Regular, pal().muted);
    }
    edit_line(
        p,
        now,
        &input,
        [tx, top(cy, 13.0), f[2] - 36.0],
        13.0,
        Font::Regular,
        pal().ink,
        focused,
    );
}

/// The suggestions for the word being typed, under its line (or over it,
/// with `above`, or where there is no room below): `word` is where the
/// word starts, the line's top and its height; `typed` what of it is typed.
fn draw_suggest(st: &mut Studio, p: &mut P, [wx, ly, lh]: [f32; 3], typed: &str, above: bool, dt: f32) {
    use super::hstudio_assist::Kind;
    // (`player:po` becomes `player.pos`: the ':' is part of what is replaced.)
    let typed = typed.trim_start_matches([':', '.']);
    let now = st.s.now;
    let (sw, sh) = st.s.ui.screen;
    let Some(sg) = st.s.ui.suggest.as_mut() else { return };
    let n = sg.items.len();
    if n == 0 {
        return;
    }
    let size = code_size().clamp(11.0, 15.0);
    let row_h = (size * 1.8).round().max(22.0);
    let rows = n.min(8);
    sg.sel = sg.sel.min(n - 1);
    if sg.sel < sg.first {
        sg.first = sg.sel;
    } else if sg.sel >= sg.first + rows {
        sg.first = sg.sel + 1 - rows;
    }
    sg.first = sg.first.min(n - rows);
    let (first, sel, at) = (sg.first, sg.sel, sg.at);
    let items: Vec<_> = sg.items[first..first + rows].to_vec();
    let chosen = sg.items[sel].clone();
    // As wide as what it shows, within the screen.
    let mut label_w: f32 = 0.0;
    let mut detail_w: f32 = 0.0;
    for it in &items {
        label_w = label_w.max(p.measure(&it.label, size, Font::Mono));
        detail_w = detail_w.max(p.measure(&it.detail, 11.0, Font::Mono));
    }
    let pw = (34.0 + label_w + 24.0 + detail_w.min(200.0) + 14.0)
        .clamp(220.0, 480.0)
        .min(sw - 16.0);
    let ph = rows as f32 * row_h + 8.0;
    let px = (wx - 34.0).min(sw - pw - 8.0).max(8.0);
    let below = ly + lh + 4.0;
    let over = ly - ph - 4.0;
    let fits_below = below + ph <= sh - 8.0;
    let fits_over = over >= 8.0;
    let up = (above && fits_over) || (!fits_below && fits_over);
    let py = if up { over } else { below.min(sh - ph - 8.0) };
    let t = progress(now, at, QUICK);
    let (base, dy) = (p.alpha, p.dy);
    p.alpha = base * t;
    p.dy = dy + (1.0 - t) * if up { 6.0 } else { -6.0 };
    for i in 1..=6 {
        let f = i as f32 * 1.5;
        p.rect(
            [px - f, py - f + 3.0, pw + 2.0 * f, ph + 2.0 * f],
            10.0 + f,
            fade(0xFF00_0000, 0.035),
        );
    }
    let r = [px, py, pw, ph];
    p.rect(r, 10.0, pal().surface);
    p.border(r, 10.0, pal().rule);
    // The highlight glides to the one chosen.
    let ui = &mut st.s.ui;
    if (ui.smooth[5] - sel as f32).abs() > rows as f32 || t < 0.05 {
        ui.smooth[5] = sel as f32;
    }
    smooth(&mut ui.smooth[5], sel as f32, dt);
    let hy = (ui.smooth[5] - first as f32).clamp(0.0, (rows - 1) as f32);
    p.rect(
        [px + 4.0, py + 4.0 + hy * row_h, pw - 8.0, row_h],
        7.0,
        fade(pal().accent, 0.16),
    );
    for (k, it) in items.iter().enumerate() {
        let i = first + k;
        let ry = py + 4.0 + k as f32 * row_h;
        let rr = [px + 4.0, ry, pw - 8.0, row_h];
        let hv = hover(ui, id("suggest-row", k), inside(rr, ui.mouse), dt);
        if i != sel {
            p.rect(rr, 7.0, fade(pal().ink, 0.05 * hv));
        }
        let (mark, c) = match it.kind {
            Kind::Function => ("f", pal().global),
            Kind::Namespace => ("m", pal().number),
            Kind::Keyword => ("k", pal().keyword),
            Kind::Event => ("e", pal().string),
            Kind::Word => ("a", pal().muted),
            Kind::Value => ("v", pal().number),
        };
        let cy = ry + row_h / 2.0;
        p.rect([px + 10.0, cy - 9.0, 18.0, 18.0], 5.0, fade(c, 0.18));
        let mw = p.measure(mark, 11.0, Font::Mono);
        p.text(mark, px + 19.0 - mw / 2.0, top(cy, 11.0), 11.0, Font::Mono, c);
        let lx = px + 36.0;
        let lc = if i == sel { pal().ink } else { pal().ink_soft };
        // What is typed of it, in the accent.
        let typed_len = if !typed.is_empty()
            && it.label.to_lowercase().starts_with(&typed.to_lowercase())
            && it.label.is_char_boundary(typed.len())
        {
            typed.len()
        } else {
            0
        };
        let room = pw - 36.0 - 12.0;
        let label = p.fit(&it.label, room, size, Font::Mono);
        let hx = if typed_len > 0 && label.len() >= typed_len && label.is_char_boundary(typed_len) {
            p.text(&label[..typed_len], lx, top(cy, size), size, Font::Mono, pal().accent)
        } else {
            0.0
        };
        let lw = hx
            + p.text(
                label.get(typed_len.min(label.len())..).unwrap_or(""),
                lx + hx,
                top(cy, size),
                size,
                Font::Mono,
                lc,
            );
        let left = px + pw - 12.0 - (lx + lw + 16.0);
        if left > 30.0 && !it.detail.is_empty() {
            let d = p.fit(&it.detail, left, 11.0, Font::Mono);
            let dw = p.measure(&d, 11.0, Font::Mono);
            p.text(&d, px + pw - 12.0 - dw, top(cy, 11.0), 11.0, Font::Mono, pal().muted);
        }
        ui.hits.push((rr, Action::Suggest(i.min(255) as u8)));
    }
    if n > rows {
        let track = ph - 12.0;
        let bar = (track * rows as f32 / n as f32).max(14.0);
        let by = py + 6.0 + (track - bar) * (first as f32 / (n - rows) as f32);
        p.rect([px + pw - 5.0, by, 2.5, bar], 1.25, fade(pal().ink, 0.2));
    }
    // About the one chosen: beside the list, or under (over) it.
    if !chosen.doc.is_empty() || p.measure(&chosen.detail, 11.0, Font::Mono) > 200.0 {
        let dw = 280.0f32.min(sw - 16.0);
        let mut lines: Vec<(String, Font, f32, u32)> = Vec::new();
        for l in p.wrap(&chosen.detail, dw - 24.0, 11.5, Font::Mono).into_iter().take(3) {
            lines.push((l, Font::Mono, 11.5, pal().ink));
        }
        for l in p.wrap(&chosen.doc, dw - 24.0, 12.0, Font::Regular).into_iter().take(6) {
            lines.push((l, Font::Regular, 12.0, pal().ink_soft));
        }
        let dh = 16.0 + lines.len() as f32 * 18.0;
        let (dx, dy0) = if px + pw + 6.0 + dw <= sw - 8.0 {
            (px + pw + 6.0, py)
        } else if px - 6.0 - dw >= 8.0 {
            (px - 6.0 - dw, py)
        } else if up && py - dh - 6.0 >= 8.0 {
            (px, py - dh - 6.0)
        } else if !up && py + ph + 6.0 + dh <= sh - 8.0 {
            (px, py + ph + 6.0)
        } else {
            (f32::NAN, 0.0)
        };
        if dx.is_finite() {
            let dr = [dx, dy0, dw, dh];
            p.rect(dr, 10.0, pal().surface);
            p.border(dr, 10.0, pal().rule);
            for (k, (l, font, size, c)) in lines.iter().enumerate() {
                p.text(l, dx + 12.0, dy0 + 8.0 + k as f32 * 18.0, *size, *font, *c);
            }
        }
    }
    p.alpha = base;
    p.dy = dy;
}

/// How long typing pauses before the live check looks again.
pub const CHECK_PAUSE: f64 = 1.0;

/// A wavy line under `[x, y, w]`, as editors mark problems.
fn squiggle(p: &mut P, x: f32, y: f32, w: f32, c: u32) {
    const LIFT: [f32; 4] = [0.0, 0.9, 1.8, 0.9];
    let mut k = 0;
    let mut sx = x;
    while sx < x + w {
        p.rect([sx, y + LIFT[k % 4], 1.6, 1.3], 0.0, c);
        sx += 1.5;
        k += 1;
    }
}

/// The largest char boundary of `s` at or before `i`.
fn floor_char(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The editor's text on a panel: line numbers, highlighting, the selection,
/// the cursor (it glides), the problems the live check finds (a squiggle,
/// a dot and their message at the line's end), the line that failed, a
/// scrollbar, the suggestions, and the file changing on disk.
pub(super) fn draw_code(st: &mut Studio, p: &mut P, panel: [f32; 4], dt: f32) {
    let now = st.s.now;
    // Agents' marks on their lines as they are now.
    super::hstudio_coach::follow_marks(st);
    st.s.ui.coach.code_at = Some(panel);
    st.s.ui.coach.bubble_at = None;
    // The live check, once typing pauses; a module's file with its siblings
    // (read again, and checked again, when they change).
    let siblings = super::hstudio_ui::editor_siblings(st);
    {
        let s = &mut st.s;
        let ed = &mut s.editor;
        // (A file opened or read again is checked at once; typing, at a pause.)
        let typing = ed.edited_at > 0.0 && now - ed.edited_at < CHECK_PAUSE;
        if ed.checked != Some(ed.revision) && !typing {
            ed.problems = super::hstudio_assist::check(&ed.lines, &ed.title, siblings.as_deref());
            ed.checked = Some(ed.revision);
            let old = std::mem::take(&mut s.ui.problem_at);
            for pr in &ed.problems {
                let k = problem_key(pr, &ed.lines);
                s.ui.problem_at.insert(k, old.get(&k).copied().unwrap_or(now));
            }
        }
    }
    p.rect(panel, 14.0, pal().plate_soft);
    if let Some(at) = st.s.editor.reloaded_at {
        // Read again after a change elsewhere: a flash.
        let k = ((now - at) / 0.9).clamp(0.0, 1.0) as f32;
        if k < 1.0 && motion() {
            p.rect(panel, 14.0, fade(pal().accent, 0.10 * (1.0 - k)));
        }
    }
    let top_y = panel[1] + 12.0;
    if panel[3] < 24.0 + code_line() {
        return;
    }
    // A right click on the code (its line numbers too) opens its menu; the
    // scroll bar, a banner and the suggestions, drawn later, are over it.
    st.s.ui.hits.push((panel, Action::Spot(Spot::Code)));
    let rows = ((panel[3] - 24.0) / code_line()).max(1.0) as usize;
    st.s.ui.page = rows;
    let gutter = p.measure("0000", 12.0, Font::Mono) + 30.0;
    let text_x = panel[0] + gutter;
    let text_w = panel[2] - gutter - 18.0;
    let ed = &mut st.s.editor;
    // The view follows the cursor when it moves or the text changes, not
    // when the wheel scrolls away from it.
    let now_at = (ed.row, ed.col, ed.revision);
    if ed.followed != Some(now_at) {
        ed.followed = Some(now_at);
        if ed.row < ed.scroll {
            ed.scroll = ed.row;
        }
        if ed.row >= ed.scroll + rows {
            ed.scroll = ed.row + 1 - rows;
        }
    }
    ed.scroll = ed.scroll.min(ed.lines.len().saturating_sub(1));
    // The columns shown follow the cursor, or an agent's caret while the view follows that.
    let (hrow, hcol) = super::hstudio_coach::followed_caret(&st.s.ui.coach, ed).unwrap_or((ed.row, ed.col));
    let cur_chars = ed.lines[hrow][..hcol].chars().count();
    if cur_chars < ed.hscroll {
        ed.hscroll = cur_chars.saturating_sub(4);
    }
    loop {
        let line = &ed.lines[hrow];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if from > hcol || p.measure(&line[from..hcol], code_size(), Font::Mono) <= text_w - 8.0 {
            break;
        }
        ed.hscroll += 4;
    }
    let mut state = Long::None;
    let mut states = Vec::with_capacity(ed.lines.len());
    for line in &ed.lines {
        states.push(state);
        state = highlight(line, state).1;
    }
    let target = ed.scroll as f32;
    smooth(&mut st.s.ui.smooth[1], target, dt);
    // The cursor glides to where it is (as far as a screen; farther, it jumps).
    let (row, col) = (st.s.editor.row, st.s.editor.col);
    let to = [
        row as f32,
        p.measure(&st.s.editor.lines[row][..col], code_size(), Font::Mono),
    ];
    {
        let ui = &mut st.s.ui;
        if ui.caret_to != to {
            ui.caret_to = to;
            ui.caret_moved = now;
        }
        let c = match ui.caret {
            Some(c) if motion() && (c[0] - to[0]).abs() <= rows as f32 => {
                let k = 1.0 - (-dt * 30.0).exp();
                let mut c = [c[0] + (to[0] - c[0]) * k, c[1] + (to[1] - c[1]) * k];
                if (c[0] - to[0]).abs() < 0.02 && (c[1] - to[1]).abs() < 0.3 {
                    c = to;
                }
                c
            }
            _ => to,
        };
        ui.caret = Some(c);
    }
    let caret = st.s.ui.caret.unwrap_or(to);
    let ed = &st.s.editor;
    let s = st.s.ui.smooth[1];
    let first = s.floor().max(0.0) as usize;
    let frac = s - first as f32;
    let sel = ed.selection();
    let bottom = panel[1] + panel[3] - 8.0;
    // The line the cursor is on, under it, gliding with it.
    let cly = top_y + (caret[0] - s) * code_line();
    if cly >= top_y - 0.5 && cly + code_line() <= bottom + 0.5 {
        p.rect(
            [panel[0] + 6.0, cly, panel[2] - 12.0, code_line()],
            6.0,
            fade(pal().ink, 0.045),
        );
    }
    let problem_at = &st.s.ui.problem_at;
    let base = p.alpha;
    let mut note_hits = Vec::new();
    for i in 0..=rows {
        let r = first + i;
        if r >= ed.lines.len() {
            break;
        }
        let ly = top_y + (i as f32 - frac) * code_line();
        if ly < top_y - 0.5 || ly + code_line() > bottom + 0.5 {
            continue;
        }
        let line = &ed.lines[r];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if ed.error_line == Some(r + 1) {
            p.rect(
                [panel[0] + 6.0, ly, panel[2] - 12.0, code_line()],
                6.0,
                fade(pal().danger, 0.14),
            );
            p.rect([panel[0] + 12.0, ly + 7.0, 6.0, 6.0], 3.0, pal().danger);
        }
        let step = coach_row(
            p,
            &st.s.ui.coach,
            ed,
            r,
            from,
            [panel[0], panel[2], text_x, text_w, ly],
            now,
        );
        note_hits.extend(step);
        if let Some(((r1, c1), (r2, c2))) = sel
            && r >= r1
            && r <= r2
        {
            let a = if r == r1 { c1 } else { 0 }.max(from);
            let b = if r == r2 { c2 } else { line.len() }.max(a);
            let sx = text_x + p.measure(&line[from..a], code_size(), Font::Mono);
            let mut sw = p.measure(&line[a..b], code_size(), Font::Mono);
            if r != r2 {
                sw += 6.0;
            }
            let sw = sw.min(text_x + text_w - sx).max(0.0);
            p.rect([sx, ly + 1.0, sw, code_line() - 2.0], 3.0, fade(pal().accent, 0.30));
        }
        let ty = ly + (code_line() - (code_size() * 1.45).round()) / 2.0;
        let num = format!("{}", r + 1);
        let nw = p.measure(&num, 12.0, Font::Mono);
        let nc = if r == ed.row {
            pal().ink_soft
        } else {
            fade(pal().muted, 0.7)
        };
        p.text(&num, text_x - 14.0 - nw, ty + 1.0, 12.0, Font::Mono, nc);
        let (pieces, _) = highlight(line, states[r]);
        let mut px = text_x;
        for (a, b, t) in pieces {
            if b <= from {
                continue;
            }
            let piece = &line[a.max(from)..b];
            let room = text_x + text_w - px;
            if room <= 0.0 {
                break;
            }
            let pw = p.measure(piece, code_size(), Font::Mono);
            let shown = if pw > room {
                p.fit(piece, room, code_size(), Font::Mono)
            } else {
                piece.to_owned()
            };
            p.text(&shown, px, ty, code_size(), Font::Mono, tint(t));
            px += pw;
        }
        // An agent's note at the line's end (it says that, not a problem).
        let noted = mark_note(p, &st.s.ui.coach, ed, r, from, [text_x, text_w, ly]);
        let noted = noted.map(|hit| note_hits.push(hit)).is_some();
        // Its problems: the worst marks the gutter and says why at the end.
        let mut said = false;
        for pr in ed.problems.iter().filter(|pr| pr.line == r + 1) {
            let fadein = problem_at
                .get(&problem_key(pr, &ed.lines))
                .map_or(1.0, |at| progress(now, *at, MOVE));
            p.alpha = base * fadein;
            let c = if pr.error { pal().danger } else { warn() };
            let (a, b) = match pr.span {
                Some((a, b)) => (floor_char(line, a), floor_char(line, b)),
                None => (line.len() - line.trim_start().len(), line.len()),
            };
            let a = a.max(from);
            let sx = text_x + p.measure(&line[from..a], code_size(), Font::Mono);
            let mut sw = if b > a {
                p.measure(&line[a..b], code_size(), Font::Mono)
            } else {
                0.0
            };
            if sw < 6.0 {
                sw = p.measure("m", code_size(), Font::Mono).max(6.0);
            }
            let sw = sw.min(text_x + text_w - sx);
            if sw > 0.0 {
                squiggle(p, sx, ly + code_line() - 3.5, sw, c);
            }
            if !said {
                said = true;
                p.rect([panel[0] + 12.0, ly + code_line() / 2.0 - 3.0, 6.0, 6.0], 3.0, c);
                let end = text_x + p.measure(&line[from..], code_size(), Font::Mono) + 20.0;
                let room = text_x + text_w - end;
                if room > 70.0 && !noted {
                    let msg = p.fit(&pr.message, room, 12.0, Font::Regular);
                    p.text(
                        &msg,
                        end,
                        top(ly + code_line() / 2.0, 12.0),
                        12.0,
                        Font::Regular,
                        fade(c, 0.85),
                    );
                }
            }
        }
        p.alpha = base;
    }
    st.s.ui.hits.extend(note_hits);
    // The cursor, solid while it moves and blinking once still.
    let caret_y = top_y + (caret[0] - s) * code_line();
    let line = &ed.lines[ed.row];
    let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
    let cx = text_x + caret[1] - p.measure(&line[..from], code_size(), Font::Mono);
    let shown =
        caret_y >= top_y - 0.5 && caret_y + code_line() <= bottom + 0.5 && cx >= text_x - 1.0 && cx <= text_x + text_w;
    st.s.ui.caret_at = shown.then_some([cx, caret_y + code_line()]);
    if shown {
        let still = (now - st.s.ui.caret_moved - 0.5).max(0.0);
        let blink = 0.55 + 0.45 * (still as f32 * std::f32::consts::PI * 1.6).cos();
        p.rect(
            [cx.floor(), caret_y + 2.0, 2.0, code_line() - 4.0],
            1.0,
            fade(pal().accent, blink),
        );
    }
    agent_caret(p, &st.s.ui.coach, ed, [text_x, text_w, top_y, bottom, s]);
    if ed.lines.len() > rows {
        let track = panel[3] - 24.0;
        let bar = (track * rows as f32 / ed.lines.len() as f32).max(24.0);
        let span = (ed.lines.len() - rows).max(1) as f32;
        let by = top_y + (track - bar) * (s / span).clamp(0.0, 1.0);
        // Dragged or pointed at, it widens; the track takes clicks (a jump there).
        let zone = [panel[0] + panel[2] - 16.0, top_y, 16.0, track];
        let held = st.s.ui.scroll_drag.is_some();
        let over = held || inside(zone, st.s.ui.mouse);
        let hv = hover(&mut st.s.ui, id("editor-scrollbar", 0), over, dt);
        let bw = 4.0 + 3.0 * hv;
        let ed = &st.s.editor;
        p.rect(
            [panel[0] + panel[2] - 7.0 - bw / 2.0, by, bw, bar],
            bw / 2.0,
            fade(pal().ink, 0.18 + 0.17 * hv),
        );
        st.s.ui.scrollbar = Some(super::hstudio_ui::ScrollBar {
            x: zone[0] + zone[2] / 2.0,
            top: top_y,
            track,
            bar,
            span,
            at: by,
        });
        st.s.ui.hits.push((zone, Action::ScrollBar));
        // Where the problems are, on the track.
        for pr in &ed.problems {
            let my = top_y + track * ((pr.line.max(1) - 1) as f32 / ed.lines.len() as f32);
            let c = if pr.error { pal().danger } else { warn() };
            p.rect([panel[0] + panel[2] - 11.0, my, 8.0, 2.0], 1.0, fade(c, 0.85));
        }
        // And agents' marks.
        mark_ticks(p, &st.s.ui.coach, ed, [panel[0] + panel[2] - 11.0, top_y, track]);
    }
    st.s.ui.view = Some(EditorView {
        x: text_x,
        y: top_y,
        w: text_w,
        rows,
    });
    note_bubble(st, p, panel, [text_x, top_y, bottom, s], dt);
    file_changed(st, p, panel, dt);
    agent_notice(st, p, panel, dt);
    // The suggestions, at the word being typed.
    if st.s.ui.suggest.as_ref().is_some_and(|sg| !sg.console) {
        let ed = &st.s.editor;
        let start = st.s.ui.suggest.as_ref().map_or(0, |sg| sg.start).min(ed.col);
        let line = &ed.lines[ed.row];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        let wx = if start >= from && line.is_char_boundary(start) {
            text_x + p.measure(&line[from..start], code_size(), Font::Mono)
        } else {
            text_x
        };
        let typed = line.get(start..ed.col).unwrap_or("").to_owned();
        let ly = top_y + (ed.row as f32 - st.s.ui.smooth[1]) * code_line();
        draw_suggest(st, p, [wx, ly, code_line()], &typed, false, dt);
    }
}

/// What an agent's edit shows on code row `r` (its text drawn from byte
/// `from`; the panel's left and width, the text's left and width, the
/// row's top): the text the edit is about to replace, tinted, and the lines
/// it just put in, lit a moment; and an agent's marks, a band in their
/// colour with a bar at the gutter's left, what they are about underlined,
/// and a lesson's step's number in a circle on its first row, which goes to
/// that step (its hit).
fn coach_row(
    p: &mut P,
    coach: &super::hstudio_coach::Coach,
    ed: &super::hstudio_ui::Editor,
    r: usize,
    from: usize,
    [px, pw, text_x, text_w, ly]: [f32; 5],
    now: f64,
) -> Option<([f32; 4], Action)> {
    use super::hstudio_coach::{FLASH, Phase};
    let mut hit = None;
    for m in coach
        .marks
        .iter()
        .filter(|m| (m.first..=m.last).contains(&r) && ed.path.as_ref() == Some(&m.path))
    {
        let c = tone_color(m.tone);
        let open = coach.open_mark == Some(m.id);
        // A lesson's other steps are fainter than the one it is on.
        let current = coach
            .lesson
            .as_ref()
            .is_some_and(|l| l.steps.get(l.at).is_some_and(|s| s.mark == m.id));
        let k = if m.step.is_some() && !open && !current {
            0.5
        } else {
            1.0
        };
        p.rect(
            [px + 6.0, ly, pw - 12.0, code_line()],
            0.0,
            fade(c, if open { 0.16 } else { 0.10 } * k),
        );
        p.rect([px + 6.0, ly, 3.0, code_line()], 0.0, fade(c, k));
        if let Some(step) = m.step.filter(|_| r == m.first) {
            let d = 15.0;
            let dot = [px + 10.0, ly + (code_line() - d) / 2.0, d, d];
            p.rect(dot, d / 2.0, fade(c, k));
            let label = (step + 1).to_string();
            let lw = p.measure(&label, 10.0, Font::Medium);
            p.text(
                &label,
                dot[0] + (d - lw) / 2.0,
                top(dot[1] + d / 2.0, 10.0),
                10.0,
                Font::Medium,
                on(c),
            );
            hit = Some((dot, Action::MarkOpen(m.id)));
        }
        if let Some((a, b)) = m.span.filter(|_| r == m.first) {
            let line = &ed.lines[r];
            let a = floor_char(line, a).max(from);
            let b = floor_char(line, b).max(a);
            let sx = text_x + p.measure(&line[from..a], code_size(), Font::Mono);
            let sw = p
                .measure(&line[a..b], code_size(), Font::Mono)
                .min(text_x + text_w - sx);
            if sw > 0.0 {
                p.rect([sx, ly + code_line() - 3.0, sw, 2.0], 1.0, c);
            }
        }
    }
    if let Some(f) = &coach.flash
        && ed.path.as_ref() == Some(&f.path)
        && (f.first..=f.last).contains(&r)
        && motion()
    {
        let k = ((now - f.at) / FLASH).clamp(0.0, 1.0) as f32;
        p.rect(
            [px + 6.0, ly, pw - 12.0, code_line()],
            6.0,
            fade(pal().accent, 0.10 * (1.0 - k)),
        );
    }
    let Some(t) = coach.typing.as_ref().filter(|t| {
        t.phase == Phase::ShowOld && ed.path.as_ref() == Some(&t.path) && (t.from.0..=t.old_to.0).contains(&r)
    }) else {
        return hit;
    };
    let line = &ed.lines[r];
    let a = floor_char(line, if r == t.from.0 { t.from.1 } else { 0 }).max(from);
    let b = floor_char(line, if r == t.old_to.0 { t.old_to.1 } else { line.len() }).max(a);
    let sx = text_x + p.measure(&line[from..a], code_size(), Font::Mono);
    let mut sw = p.measure(&line[a..b], code_size(), Font::Mono);
    if r != t.old_to.0 {
        sw += 6.0;
    }
    let sw = sw.min(text_x + text_w - sx).max(0.0);
    p.rect([sx, ly + 1.0, sw, code_line() - 2.0], 3.0, fade(pal().danger, 0.22));
    hit
}

/// An agent's caret where its edit is coming in: solid, in the accent, with
/// the agent's name on a flag above it (below when there is no room), both
/// kept inside the text's width (at its edge when the caret is past it). A
/// change made on disk being typed in has the caret alone.
fn agent_caret(
    p: &mut P,
    coach: &super::hstudio_coach::Coach,
    ed: &super::hstudio_ui::Editor,
    [text_x, text_w, top_y, bottom, s]: [f32; 5],
) {
    let Some(t) = coach
        .typing
        .as_ref()
        .filter(|t| t.phase == super::hstudio_coach::Phase::Typing && ed.path.as_ref() == Some(&t.path))
    else {
        return;
    };
    let (row, col) = t.at;
    let Some(line) = ed.lines.get(row) else {
        return;
    };
    let y = top_y + (row as f32 - s) * code_line();
    if y < top_y - 0.5 || y + code_line() > bottom + 0.5 {
        return;
    }
    let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
    let col = floor_char(line, col);
    let x = if col >= from {
        text_x + p.measure(&line[from..col], code_size(), Font::Mono)
    } else {
        text_x
    };
    let x = x.clamp(text_x, text_x + text_w - 2.0).floor();
    p.rect([x, y + 2.0, 2.0, code_line() - 4.0], 1.0, pal().accent);
    // A file read again is nobody's: no name.
    if t.by.name.is_empty() {
        return;
    }
    let (size, h) = (10.5, 16.0);
    let w = p.measure(&t.by.name, size, Font::Medium) + 12.0;
    let fy = if y - h >= top_y - 10.0 {
        y - h + 2.0
    } else {
        y + code_line() - 2.0
    };
    let fx = x.min(text_x + text_w - w).max(text_x - 4.0);
    p.rect([fx, fy, w, h], 5.0, pal().accent);
    p.text(
        &t.by.name,
        fx + 6.0,
        top(fy + h / 2.0, size),
        size,
        Font::Medium,
        on_accent(),
    );
}

/// At the bottom right of the code, a moment: an agent changed a file the
/// editor doesn't show ("Claude Code changed lib/util.lua"); a click shows it.
fn agent_notice(st: &mut Studio, p: &mut P, panel: [f32; 4], dt: f32) {
    use super::hstudio_coach::NOTICE_SECS;
    let now = st.s.now;
    let Some((text, seq, at)) = st.s.ui.coach.notice.as_ref().map(|n| (n.text.clone(), n.seq, n.at)) else {
        return;
    };
    let label = format!("{text} · Show");
    let lw = p.measure(&label, 12.0, Font::Medium) + 30.0;
    let r = [panel[0] + panel[2] - lw - 14.0, panel[1] + panel[3] - 36.0, lw, 24.0];
    if panel[3] < 72.0 || r[0] < panel[0] + 10.0 {
        return;
    }
    let age = now - at;
    let base = p.alpha;
    p.alpha = base * progress(now, at, QUICK) * (1.0 - ((age - (NOTICE_SECS - 0.6)) / 0.6).clamp(0.0, 1.0) as f32);
    let ui = &mut st.s.ui;
    let hv = hover(ui, id("agent-notice", 0), inside(r, ui.mouse), dt);
    p.rect(r, 12.0, mix(pal().surface, pal().accent, 0.16 + 0.08 * hv));
    p.border(r, 12.0, fade(pal().accent, 0.4));
    p.rect([r[0] + 10.0, r[1] + 8.5, 7.0, 7.0], 3.5, pal().accent);
    p.text(
        &label,
        r[0] + 22.0,
        top(r[1] + 12.0, 12.0),
        12.0,
        Font::Medium,
        pal().ink,
    );
    ui.hits.push((r, Action::AgentJump(seq)));
    p.alpha = base;
}

/// A mark's colour: the accent for a note, danger for an error, the
/// warning amber, success for something right.
pub(super) fn tone_color(t: super::hstudio_coach::Tone) -> u32 {
    use super::hstudio_coach::Tone;
    match t {
        Tone::Note => pal().accent,
        Tone::Error => pal().danger,
        Tone::Warning => warn(),
        Tone::Good => pal().success,
    }
}

/// An agent's note at the end of its mark's first line (row `r`, its text
/// drawn from byte `from`), in its colour, while its bubble isn't the one
/// open: a click opens it. Its hit, when there was room to draw it. (A
/// lesson's steps have their number instead.)
fn mark_note(
    p: &mut P,
    coach: &super::hstudio_coach::Coach,
    ed: &super::hstudio_ui::Editor,
    r: usize,
    from: usize,
    [text_x, text_w, ly]: [f32; 3],
) -> Option<([f32; 4], Action)> {
    let m = coach.marks.iter().rev().find(|m| {
        m.first == r
            && !m.note.is_empty()
            && m.step.is_none()
            && coach.open_mark != Some(m.id)
            && ed.path.as_ref() == Some(&m.path)
    })?;
    let line = &ed.lines[r];
    let end = text_x + p.measure(&line[from..], code_size(), Font::Mono) + 20.0;
    let room = text_x + text_w - end;
    if room <= 70.0 {
        return None;
    }
    let c = tone_color(m.tone);
    let cy = ly + code_line() / 2.0;
    let mut said = m.note.lines().next().unwrap_or_default().to_owned();
    if m.note.contains('\n') {
        said.push_str(" …");
    }
    let said = p.fit(&said, room - 14.0, 12.0, Font::Regular);
    p.rect([end, cy - 3.0, 6.0, 6.0], 3.0, c);
    let w = p.text(&said, end + 12.0, top(cy, 12.0), 12.0, Font::Regular, fade(c, 0.9));
    Some(([end - 4.0, ly, w + 20.0, code_line()], Action::MarkOpen(m.id)))
}

/// Where agents' marks are in the editor's file, on its scroll bar's track
/// (`x`, its top, its height).
fn mark_ticks(
    p: &mut P,
    coach: &super::hstudio_coach::Coach,
    ed: &super::hstudio_ui::Editor,
    [x, top_y, track]: [f32; 3],
) {
    for m in coach.marks.iter().filter(|m| ed.path.as_ref() == Some(&m.path)) {
        let my = top_y + track * (m.first as f32 / ed.lines.len() as f32);
        p.rect([x, my, 8.0, 3.0], 1.0, tone_color(m.tone));
    }
}

/// How many lines of each group of a bubble's words show (`lens`: each
/// group's lines and its share): each group at most its share unless all
/// of it is asked for, and all of them at most `fit` lines, the last groups
/// giving way first (the first keeps a line). And whether a More would show
/// more.
fn bubble_lines(lens: &[(usize, usize)], all: bool, fit: usize) -> (Vec<usize>, bool) {
    let mut keep: Vec<usize> = lens
        .iter()
        .map(|&(n, share)| if all { n } else { n.min(share) })
        .collect();
    let cut = keep.iter().zip(lens).any(|(k, &(n, _))| *k < n);
    if keep.iter().sum::<usize>() + usize::from(cut) <= fit {
        return (keep, cut);
    }
    let mut total: usize = keep.iter().sum();
    for i in (0..keep.len()).rev() {
        let least = usize::from(i == 0);
        while total > fit && keep[i] > least {
            keep[i] -= 1;
            total -= 1;
        }
    }
    (keep, false)
}

/// The open mark's note in a bubble under its lines (over them when there
/// is no room below), from its first line's indent, at most 520 wide. It
/// stays inside the code's panel, never taller than the panel less two rows:
/// its words are cut to fit, and to their share (eight lines of note) until
/// its More is pressed. A lesson's step has what to try, its explanation
/// when the Agent panel doesn't show it (the executor styles have none),
/// and where the lesson is with Back, Check and Next (Done on its last
/// step). Its × clears the mark, a step's ends the lesson; clicks elsewhere
/// on it reach nothing under it. (`text_x`: where the code's text starts;
/// the code's top and bottom; the view's row at its top, as drawn.)
fn note_bubble(st: &mut Studio, p: &mut P, panel: [f32; 4], [text_x, top_y, bottom, s]: [f32; 4], dt: f32) {
    use super::hstudio_ui::Panel;
    const LINE: f32 = 17.0;
    const PAD: f32 = 10.0;
    const FOOT: f32 = 32.0;
    let shown = super::hstudio_coach::shown_mark(&st.s.ui.coach, &st.s.editor);
    let Some(mark) = shown.filter(|m| !m.note.is_empty()).map(|m| m.id) else {
        return;
    };
    let step = super::hstudio_coach::step_view(st, mark);
    let ui = &st.s.ui;
    let explained = ui.look.theme().layout == Layout::Studio
        && !ui.panel_hidden
        && ui.panel == Panel::Agent
        && ui.panel_at.is_some();
    let ed = &st.s.editor;
    let coach = &st.s.ui.coach;
    let Some(m) = coach.marks.iter().find(|m| m.id == mark) else {
        return;
    };
    let lh = code_line();
    let (y0, y1) = (
        top_y + (m.first as f32 - s) * lh,
        top_y + (m.last as f32 + 1.0 - s) * lh,
    );
    // Its lines scrolled away, it waits for them.
    let area = [panel[0] + 8.0, panel[1] + 4.0, panel[2] - 16.0, panel[3] - 8.0];
    let w = area[2].min(520.0);
    let foot = if step.is_some() { FOOT } else { 0.0 };
    if y1 <= top_y || y0 >= bottom || w < 120.0 || area[3] < 2.0 * PAD + LINE + foot {
        return;
    }
    let Some(line) = ed.lines.get(m.first) else {
        return;
    };
    let indent = (line.len() - line.trim_start().len()).saturating_sub(ed.hscroll);
    let ix = p.measure(&" ".repeat(indent), code_size(), Font::Mono);
    let x = (text_x + ix - 14.0).min(area[0] + area[2] - w).max(area[0]);
    let (tone, note, all) = (tone_color(m.tone), m.note.clone(), coach.more);
    // Its words by group (the note, what to try, the explanation), each in
    // its colour with its share of lines.
    let tw = w - 52.0;
    let mut groups: Vec<(Vec<String>, u32, usize)> = vec![(p.wrap(&note, tw, 12.5, Font::Regular), pal().ink, 8)];
    if let Some(v) = &step {
        if let Some(task) = &v.task {
            groups.push((
                p.wrap(&format!("Try: {task}"), tw, 12.5, Font::Regular),
                pal().ink_soft,
                3,
            ));
        }
        if !explained && !v.explain.is_empty() {
            groups.push((p.wrap(&v.explain, tw, 12.5, Font::Regular), pal().muted, 4));
        }
    }
    // The rows it may take: the panel less two rows, so what it is about shows beside it.
    let room = (panel[3] - 2.0 * lh).max(2.0 * PAD + LINE + foot).min(area[3]);
    let fit = ((room - 2.0 * PAD - foot) / LINE).floor().max(1.0) as usize;
    let lens: Vec<(usize, usize)> = groups.iter().map(|(l, _, share)| (l.len(), *share)).collect();
    let (keep, more) = bubble_lines(&lens, all, fit);
    let mut rows: Vec<(String, u32)> = Vec::new();
    for ((lines, c, _), k) in groups.into_iter().zip(keep) {
        let cut = k < lines.len();
        for (i, l) in lines.into_iter().take(k).enumerate() {
            let l = if cut && i + 1 == k {
                p.fit(&format!("{l} …"), tw, 12.5, Font::Regular)
            } else {
                l
            };
            rows.push((l, c));
        }
    }
    let n = rows.len() + usize::from(more);
    let h = 2.0 * PAD + LINE * n as f32 + foot;
    let (below, above, floor) = (y1 + 4.0, y0 - 4.0 - h, area[1] + area[3]);
    let y = if below + h <= floor {
        below
    } else if above >= area[1] {
        above
    } else {
        below.clamp(area[1], floor - h)
    };
    let r = [x, y, w, h];
    let ui = &mut st.s.ui;
    let base = p.alpha;
    p.alpha = base * hover(ui, id("note-bubble", mark as usize), true, dt);
    ui.hits.push((r, Action::Block));
    p.rect(r, 10.0, pal().surface);
    p.border(r, 10.0, fade(tone, 0.55));
    p.rect([x + 8.0, y + 9.0, 3.0, h - 18.0], 1.5, tone);
    for (i, (l, c)) in rows.iter().enumerate() {
        let cy = y + PAD + LINE * (i as f32 + 0.5);
        p.text(l, x + 20.0, top(cy, 12.5), 12.5, Font::Regular, *c);
    }
    if more {
        let cy = y + PAD + LINE * (rows.len() as f32 + 0.5);
        let lw = p.measure("More", 12.0, Font::Medium);
        let mr = [x + 16.0, cy - LINE / 2.0, lw + 8.0, LINE];
        let hv = hover(ui, id("note-more", 0), inside(mr, ui.mouse), dt);
        let c = mix(pal().accent, pal().accent_hover, hv);
        p.text("More", x + 20.0, top(cy, 12.0), 12.0, Font::Medium, c);
        p.rect([x + 20.0, cy + 7.0, lw, 1.0], 0.0, fade(c, hv));
        ui.hits.push((mr, Action::MarkMore));
    }
    if let Some(v) = &step {
        // Where the lesson is, and its buttons at the right: Next (Done on
        // its last step), Check when someone can check, Back after the first.
        let fy = y + h - PAD - 26.0;
        let last = v.at + 1 == v.of;
        let mut buttons = vec![(
            "lesson-next",
            if last { "Done" } else { "Next" },
            Style::Primary,
            Action::LessonNext,
        )];
        if v.check {
            buttons.push(("lesson-check", "Check", Style::Secondary, Action::LessonCheck));
        }
        if v.at > 0 {
            buttons.push(("lesson-back", "Back", Style::Quiet, Action::LessonBack));
        }
        let mut bx = x + w - 10.0;
        for (key, label, style, action) in buttons {
            let bw = p.measure(label, 13.0, Font::Medium) + 28.0;
            if bx - bw < x + 18.0 {
                break;
            }
            bx -= bw;
            pill(ui, p, key, label, bx, fy, 26.0, style, action, dt);
            bx -= 6.0;
        }
        let at = format!("Step {} of {}", v.at + 1, v.of);
        if x + 20.0 + p.measure(&at, 12.0, Font::Medium) < bx - 6.0 {
            p.text(&at, x + 20.0, top(fy + 13.0, 12.0), 12.0, Font::Medium, pal().muted);
        }
    }
    let xr = [x + w - 28.0, y + 6.0, 22.0, 22.0];
    let hv = hover(ui, id("note-close", 0), inside(xr, ui.mouse), dt);
    p.rect(xr, 11.0, fade(pal().ink, 0.10 * hv));
    let xw = p.measure("×", 15.0, Font::Regular);
    p.text(
        "×",
        xr[0] + (22.0 - xw) / 2.0,
        top(xr[1] + 11.0, 15.0),
        15.0,
        Font::Regular,
        mix(pal().muted, pal().ink, hv),
    );
    let close = if step.is_some() {
        Action::LessonEnd
    } else {
        Action::MarkClose(mark)
    };
    ui.hits.push((xr, close));
    ui.coach.bubble_at = Some(r);
    p.alpha = base;
}

/// In the editor's header (at `x`, its top `y`, up to `right`), while
/// agents have marked code: "3 notes" in the newest mark's colour, which
/// goes to the next mark, and a × that clears them; while a lesson is on,
/// "Lesson 2/5" instead, which shows its step again, and a × that ends it.
/// Its width (0: not drawn).
fn coach_chip(st: &mut Studio, p: &mut P, x: f32, y: f32, right: f32, dt: f32) -> f32 {
    let c = &st.s.ui.coach;
    let (label, tone, go, close) = match &c.lesson {
        Some(l) => {
            let step = c.marks.iter().find(|m| m.id == l.steps[l.at].mark);
            (
                format!("Lesson {}/{}", l.at + 1, l.steps.len()),
                step.map_or(pal().accent, |m| tone_color(m.tone)),
                Action::LessonShow,
                Action::LessonEnd,
            )
        }
        None => {
            let Some(newest) = c.marks.last() else {
                return 0.0;
            };
            (
                super::hstudio_coach::notes_text(c.marks.len()),
                tone_color(newest.tone),
                Action::NextMark,
                Action::ClearMarks,
            )
        }
    };
    let lw = p.measure(&label, 12.0, Font::Medium);
    let w = 21.0 + lw + 30.0;
    if x + w > right {
        return 0.0;
    }
    let chip = [x, y, w, 24.0];
    let main = [x, y, w - 24.0, 24.0];
    let cross = [x + w - 24.0, y + 2.0, 20.0, 20.0];
    let ui = &mut st.s.ui;
    let hv = hover(ui, id("notes-chip", 0), inside(main, ui.mouse), dt);
    let hx = hover(ui, id("notes-chip-x", 0), inside(cross, ui.mouse), dt);
    p.rect(chip, 12.0, fade(pal().ink, 0.04 + 0.06 * hv));
    p.rect([x + 8.0, y + 8.5, 7.0, 7.0], 3.5, tone);
    p.text(
        &label,
        x + 21.0,
        top(y + 12.0, 12.0),
        12.0,
        Font::Medium,
        mix(pal().ink_soft, pal().ink, hv),
    );
    p.rect(cross, 10.0, fade(pal().ink, 0.10 * hx));
    let xw = p.measure("×", 14.0, Font::Regular);
    p.text(
        "×",
        cross[0] + (20.0 - xw) / 2.0,
        top(y + 12.0, 14.0),
        14.0,
        Font::Regular,
        mix(pal().muted, pal().ink, hx),
    );
    ui.hits.push((main, go));
    ui.hits.push((cross, close));
    w
}

/// Over the code: the file changed on disk while it had edits here (take
/// that, or keep these), or was removed from disk (let it go, or keep these
/// to save it again), or a note that it was read again.
fn file_changed(st: &mut Studio, p: &mut P, panel: [f32; 4], dt: f32) {
    let now = st.s.now;
    let removed = st.s.editor.removed;
    let c = glide(&mut st.s.ui, id("conflict", 0), st.s.editor.conflict || removed, dt);
    let base = p.alpha;
    if c > 0.01 {
        p.alpha = base * c;
        let r = [
            panel[0] + 10.0,
            panel[1] + 8.0 - (1.0 - c) * 10.0,
            panel[2] - 20.0,
            36.0,
        ];
        p.rect(r, 10.0, mix(pal().surface, warn(), 0.16));
        p.border(r, 10.0, fade(warn(), 0.6));
        st.s.ui.hits.push((r, Action::Block));
        let cy = r[1] + 18.0;
        let mut bx = r[0] + r[2] - 6.0;
        let kw = p.measure("Keep mine", 13.0, Font::Medium) + 28.0;
        bx -= kw;
        pill(
            &mut st.s.ui,
            p,
            "keep-mine",
            "Keep mine",
            bx,
            cy - 13.0,
            26.0,
            Style::Quiet,
            Action::KeepMine,
            dt,
        );
        let (label, action, said) = if removed {
            (
                "Close",
                Action::CloseRemoved,
                "This file was removed from disk while you were editing it.",
            )
        } else {
            (
                "Reload",
                Action::ReloadFile,
                "This file changed on disk while you were editing it.",
            )
        };
        let rw = p.measure(label, 13.0, Font::Medium) + 28.0;
        bx -= rw + 6.0;
        pill(
            &mut st.s.ui,
            p,
            "reload-file",
            label,
            bx,
            cy - 13.0,
            26.0,
            Style::Secondary,
            action,
            dt,
        );
        let msg = p.fit(said, bx - r[0] - 24.0, 13.0, Font::Regular);
        p.text(&msg, r[0] + 14.0, top(cy, 13.0), 13.0, Font::Regular, pal().ink);
        p.alpha = base;
    }
    if let Some(at) = st.s.editor.reloaded_at {
        let age = now - at;
        if age < 2.4 {
            let a = progress(now, at, QUICK) * (1.0 - ((age - 1.8) / 0.6).clamp(0.0, 1.0) as f32);
            p.alpha = base * a;
            let label = "Changed on disk: read again";
            let lw = p.measure(label, 12.0, Font::Medium) + 30.0;
            let r = [panel[0] + panel[2] - lw - 14.0, panel[1] + 10.0, lw, 24.0];
            p.rect(r, 12.0, fade(pal().accent, 0.16));
            p.rect([r[0] + 10.0, r[1] + 8.5, 7.0, 7.0], 3.5, pal().accent);
            p.text(
                label,
                r[0] + 22.0,
                top(r[1] + 12.0, 12.0),
                12.0,
                Font::Medium,
                pal().ink,
            );
            p.alpha = base;
        }
    }
}

/// "1 file", "3 files".
fn files_text(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}

fn status(state: &ModState, enabled: bool) -> (&'static str, u32) {
    match state {
        ModState::Running => ("Running", pal().success),
        ModState::Failed(..) => ("Failed", pal().danger),
        ModState::Off if !enabled => ("Off", pal().muted),
        ModState::Off => ("Not loaded", pal().muted),
    }
}

fn draw_library(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    // Narrow (a small window): the list alone, the chosen mod's buttons on its row.
    let narrow = w < 640.0;
    let pills_w = ["New mod", "Reload all", "Open folder"]
        .iter()
        .map(|l| p.measure(l, 13.0, Font::Medium) + 36.0)
        .sum::<f32>();
    let list_w = if narrow {
        w
    } else {
        (w * 0.42).clamp(240.0, 460.0).min(w * 0.55).max(pills_w)
    };
    let n = st.s.mods.len();
    if n > 0 {
        st.s.ui.library_sel = st.s.ui.library_sel.min(n - 1);
    }
    let lw = p.text(
        "Script mods",
        x,
        top(y + 10.0, 13.0),
        13.0,
        Font::Medium,
        pal().ink_soft,
    );
    let count = format!("{n}");
    let cw = p.measure(&count, 11.0, Font::Mono) + 12.0;
    p.rect([x + lw + 8.0, y + 1.0, cw, 18.0], 9.0, fade(pal().ink, 0.08));
    p.text(
        &count,
        x + lw + 14.0,
        top(y + 10.0, 11.0),
        11.0,
        Font::Mono,
        pal().muted,
    );
    let examples_h = (EXAMPLES.len() as f32 * 34.0 + 34.0).min(h * 0.42);
    let list_top = y + 30.0;
    let list_h = (h - 30.0 - 52.0 - examples_h).max(52.0);
    let row_h = 52.0;
    let max_rows = ((list_h / row_h).floor() as usize).max(1);
    let first = st.s.ui.library_sel.saturating_sub(max_rows - 1);
    // The list's own menu, under its rows'.
    st.s.ui
        .hits
        .push(([x, y, list_w, list_h + 30.0], Action::Spot(Spot::Scripts)));
    let (base_a, base_dy) = (p.alpha, p.dy);
    let mut ly = list_top;
    if n == 0 {
        p.text(
            "None yet. Make one, install an example,",
            x + 4.0,
            top(ly + 14.0, 14.0),
            14.0,
            Font::Regular,
            pal().ink_soft,
        );
        p.text(
            "or drop a .lua file on the game window.",
            x + 4.0,
            top(ly + 34.0, 14.0),
            14.0,
            Font::Regular,
            pal().muted,
        );
        ly += 52.0;
    }
    for i in first..n.min(first + max_rows) {
        let arrive = progress(now, st.s.ui.tab_at + (i - first) as f64 * STAGGER, ENTER);
        p.alpha = base_a * arrive;
        p.dy = base_dy + (1.0 - arrive) * RISE;
        let r = [x, ly, list_w, row_h - 4.0];
        let selected = i == st.s.ui.library_sel;
        let over = inside(r, st.s.ui.mouse);
        let hv = hover(&mut st.s.ui, id("row", i), over, dt);
        let sel = glide(&mut st.s.ui, id("sel", i), selected, dt);
        p.rect(r, 12.0, fade(pal().ink, 0.06 * hv + 0.07 * sel));
        let m = &st.s.mods[i];
        let (_, dot) = status(&m.state, m.enabled);
        let cy = ly + (row_h - 4.0) / 2.0;
        p.rect([x + 16.0, cy - 4.0, 8.0, 8.0], 4.0, dot);
        // A narrow row's buttons take its right side.
        let buttons = narrow && selected;
        let ver = if buttons { String::new() } else { m.version.clone() };
        let vw = if ver.is_empty() {
            0.0
        } else {
            p.measure(&ver, 11.0, Font::Mono)
        };
        let room = if buttons { list_w - 52.0 - 274.0 } else { list_w - 52.0 };
        let name_w = room.max(40.0) - vw;
        let name = p.fit(&m.name, name_w, 14.0, Font::Medium);
        let failed = match &m.state {
            ModState::Failed(msg, _) if narrow => Some(msg.clone()),
            _ => None,
        };
        let desc = if let Some(msg) = &failed {
            msg.clone()
        } else if m.description.is_empty() {
            match &m.folder {
                Some(_) => format!("{}/  ·  {}", m.id, files_text(m.file_count())),
                None => format!("{}.lua", m.id),
            }
        } else {
            m.description.clone()
        };
        let desc = p.fit(&desc, room.max(40.0), 12.0, Font::Regular);
        let name_c = if m.enabled { pal().ink } else { pal().muted };
        p.text(&name, x + 36.0, top(cy - 8.0, 14.0), 14.0, Font::Medium, name_c);
        let desc_c = if failed.is_some() { pal().danger } else { pal().muted };
        p.text(&desc, x + 36.0, top(cy + 10.0, 12.0), 12.0, Font::Regular, desc_c);
        if !ver.is_empty() {
            p.text(
                &ver,
                x + list_w - 14.0 - vw,
                top(cy - 8.0, 11.0),
                11.0,
                Font::Mono,
                pal().muted,
            );
        }
        st.s.ui.hits.push((r, Action::Select(i)));
        st.s.ui.hits.push((r, Action::Spot(Spot::Mod(i))));
        if buttons {
            let enabled = st.s.mods[i].enabled;
            let toggle = if enabled { "Turn off" } else { "Turn on" };
            let mut bx = x + list_w - 8.0;
            // Its "..." has the rest: a module's files, New file, Make it a module, Delete.
            for (key, label, action) in [
                ("n-menu", "...", Action::LibraryMenu(i)),
                ("n-reload", "Reload", Action::Reload(i)),
                ("n-toggle", toggle, Action::Toggle(i)),
                ("n-edit", "Edit", Action::Edit(i)),
            ] {
                bx -= p.measure(label, 13.0, Font::Medium) + 28.0;
                let style = if action == Action::Edit(i) {
                    Style::Primary
                } else {
                    Style::Quiet
                };
                let bw = pill(&mut st.s.ui, p, key, label, bx, cy - 13.0, 26.0, style, action, dt);
                if action == Action::LibraryMenu(i) {
                    st.s.ui.library_dots = Some([bx, cy - 13.0, bw, 26.0]);
                }
                bx -= 4.0;
            }
        }
        ly += row_h;
    }
    p.alpha = base_a;
    p.dy = base_dy;
    let by = (list_top + list_h + 8.0).max(ly + 6.0);
    let mut bx = x;
    bx += pill(
        &mut st.s.ui,
        p,
        "new",
        "New mod",
        bx,
        by,
        32.0,
        Style::Primary,
        Action::NewMod,
        dt,
    ) + 8.0;
    bx += pill(
        &mut st.s.ui,
        p,
        "reload-all",
        "Reload all",
        bx,
        by,
        32.0,
        Style::Secondary,
        Action::ReloadAll,
        dt,
    ) + 8.0;
    pill(
        &mut st.s.ui,
        p,
        "folder",
        "Open folder",
        bx,
        by,
        32.0,
        Style::Secondary,
        Action::OpenFolder,
        dt,
    );
    let mut ey = by + 52.0;
    p.text("Examples", x, top(ey + 8.0, 13.0), 13.0, Font::Medium, pal().ink_soft);
    ey += 26.0;
    let mut hovered: Option<(&str, &str)> = None;
    for (i, ex) in EXAMPLES.iter().enumerate() {
        if ey + 30.0 > y + h {
            break;
        }
        let r = [x, ey, list_w, 30.0];
        st.s.ui.hits.push((r, Action::Spot(Spot::Example(i))));
        let over = inside(r, st.s.ui.mouse);
        let hv = hover(&mut st.s.ui, id("ex", i), over, dt);
        if inside(r, st.s.ui.mouse) {
            hovered = Some((ex.title, ex.description));
        }
        p.rect(r, 10.0, fade(pal().ink, 0.05 * hv));
        let installed = st.s.mods.iter().any(|m| m.id == ex.id);
        let title = p.fit(ex.title, list_w - 120.0, 13.0, Font::Regular);
        p.text(
            &title,
            x + 12.0,
            top(ey + 15.0, 13.0),
            13.0,
            Font::Regular,
            pal().ink_soft,
        );
        if installed {
            let t = "Installed";
            let tw = p.measure(t, 12.0, Font::Regular);
            p.text(
                t,
                x + list_w - 12.0 - tw,
                top(ey + 15.0, 12.0),
                12.0,
                Font::Regular,
                pal().muted,
            );
        } else {
            let label = "Install";
            let bw = p.measure(label, 13.0, Font::Medium) + 28.0;
            pill(
                &mut st.s.ui,
                p,
                &format!("install-{i}"),
                label,
                x + list_w - 4.0 - bw,
                ey + 2.0,
                26.0,
                Style::Quiet,
                Action::Install(i),
                dt,
            );
        }
        ey += 34.0;
    }
    // The detail: the example under the pointer, else the chosen mod.
    let dx = x + list_w + 36.0;
    let dw = w - list_w - 36.0;
    if narrow || dw < 200.0 {
        return;
    }
    let bottom = y + h;
    if let Some((title, desc)) = hovered {
        p.text(title, dx, top(y + 14.0, 20.0), 20.0, Font::Medium, pal().ink);
        for (j, l) in p.wrap(desc, dw, 14.0, Font::Regular).iter().enumerate() {
            p.text(l, dx, y + 36.0 + 22.0 * j as f32, 14.0, Font::Regular, pal().ink_soft);
        }
        return;
    }
    let Some(m) = st.s.mods.get(st.s.ui.library_sel) else {
        drop_hint(p, dx, y, dw);
        return;
    };
    let i = st.s.ui.library_sel;
    let name = m.name.clone();
    let (label, c) = status(&m.state, m.enabled);
    let folder = m.folder.clone();
    let module = folder.is_some();
    let files = m.files();
    let file = if module {
        format!("module  ·  {}", files_text(files.len()))
    } else {
        format!("{}.lua", m.id)
    };
    let meta = [m.version.as_str(), m.author.as_str(), file.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("  ·  ");
    let description = m.description.clone();
    let library = m.exports.is_some();
    let uses = m.imports.join(", ");
    let used_by =
        st.s.mods
            .iter()
            .filter(|o| o.imports.contains(&m.id))
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
    let running = m.running();
    let enabled = m.enabled;
    let stats = format!(
        "{:.2} ms a call  ·  {} handlers  ·  reloaded {}x",
        m.cost_ms,
        m.handlers.count(),
        m.reloads
    );
    // Each command with its usage (`wp [<action>] [<name>]`), then its description.
    let commands: Vec<(String, String)> = m
        .commands
        .iter()
        .map(|c| {
            (
                format!("{} {}", c.name, c.usage_line()).trim_end().to_owned(),
                c.help.clone(),
            )
        })
        .collect();
    let failure = match &m.state {
        ModState::Failed(msg, trace) => Some((msg.clone(), trace.clone())),
        _ => None,
    };
    let mut dy = y;
    let name = p.fit(&name, dw, 24.0, Font::Medium);
    p.text(&name, dx, top(dy + 16.0, 24.0), 24.0, Font::Medium, pal().ink);
    dy += 38.0;
    let meta = p.fit(&meta, dw, 12.0, Font::Regular);
    p.text(&meta, dx, top(dy + 8.0, 12.0), 12.0, Font::Regular, pal().muted);
    dy += 26.0;
    let chip_w = p.measure(label, 12.0, Font::Medium) + 30.0;
    p.rect([dx, dy, chip_w, 24.0], 12.0, fade(c, 0.16));
    p.rect([dx + 10.0, dy + 9.0, 6.0, 6.0], 3.0, c);
    p.text(label, dx + 22.0, top(dy + 12.0, 12.0), 12.0, Font::Medium, c);
    let mut chips_w = chip_w;
    if library {
        let lw = p.measure("Library", 12.0, Font::Medium) + 20.0;
        p.rect([dx + chips_w + 8.0, dy, lw, 24.0], 12.0, fade(pal().accent, 0.16));
        p.text(
            "Library",
            dx + chips_w + 18.0,
            top(dy + 12.0, 12.0),
            12.0,
            Font::Medium,
            pal().accent,
        );
        chips_w += lw + 8.0;
    }
    if module {
        let mw = p.measure("Module", 12.0, Font::Medium) + 20.0;
        p.rect([dx + chips_w + 8.0, dy, mw, 24.0], 12.0, fade(pal().ink, 0.10));
        p.text(
            "Module",
            dx + chips_w + 18.0,
            top(dy + 12.0, 12.0),
            12.0,
            Font::Medium,
            pal().ink_soft,
        );
        chips_w += mw + 8.0;
    }
    if running {
        let stats = p.fit(&stats, dw - chips_w - 12.0, 11.0, Font::Mono);
        p.text(
            &stats,
            dx + chips_w + 12.0,
            top(dy + 12.0, 11.0),
            11.0,
            Font::Mono,
            pal().muted,
        );
    }
    dy += 38.0;
    // As much as fits under it, keeping room for the buttons.
    for l in p.wrap(&description, dw, 14.0, Font::Regular).iter().take(4) {
        if dy + 22.0 + 40.0 > bottom {
            break;
        }
        p.text(l, dx, dy, 14.0, Font::Regular, pal().ink_soft);
        dy += 22.0;
    }
    for (what, ids) in [("Uses", &uses), ("Used by", &used_by)] {
        if !ids.is_empty() && dy + 20.0 + 40.0 <= bottom {
            let lw = p.text(what, dx, dy, 12.0, Font::Medium, pal().muted);
            let ids = p.fit(ids, dw - lw - 8.0, 12.0, Font::Mono);
            p.text(&ids, dx + lw + 8.0, dy, 12.0, Font::Mono, pal().ink_soft);
            dy += 20.0;
        }
    }
    dy += 8.0;
    let mut bx = dx;
    bx += pill(
        &mut st.s.ui,
        p,
        "edit",
        "Edit",
        bx,
        dy,
        32.0,
        Style::Primary,
        Action::Edit(i),
        dt,
    ) + 8.0;
    let toggle = if enabled { "Turn off" } else { "Turn on" };
    bx += pill(
        &mut st.s.ui,
        p,
        "toggle",
        toggle,
        bx,
        dy,
        32.0,
        Style::Secondary,
        Action::Toggle(i),
        dt,
    ) + 8.0;
    bx += pill(
        &mut st.s.ui,
        p,
        "reload",
        "Reload",
        bx,
        dy,
        32.0,
        Style::Secondary,
        Action::Reload(i),
        dt,
    ) + 8.0;
    // A module takes new files; a script can become a module.
    let (key, more, action) = if module {
        ("new-file", "New file", Action::NewFileIn(i))
    } else {
        ("make-module", "Make it a module", Action::MakeModule(i))
    };
    if bx + p.measure(more, 13.0, Font::Medium) + 28.0 > dx + dw {
        dy += 40.0;
        bx = dx;
    }
    bx += pill(&mut st.s.ui, p, key, more, bx, dy, 32.0, Style::Quiet, action, dt) + 8.0;
    // The rest (deleting it, a module's files renamed or deleted) is in its menu.
    let dots_w = p.measure("...", 13.0, Font::Medium) + 28.0;
    if bx + dots_w > dx + dw {
        dy += 40.0;
        bx = dx;
    }
    pill(
        &mut st.s.ui,
        p,
        "library-menu",
        "...",
        bx,
        dy,
        32.0,
        Style::Quiet,
        Action::LibraryMenu(i),
        dt,
    );
    st.s.ui.library_dots = Some([bx, dy, dots_w, 32.0]);
    dy += 50.0;
    // A module's files: a click opens one.
    if module && dy + 46.0 <= bottom {
        p.text("Files", dx, top(dy + 8.0, 13.0), 13.0, Font::Medium, pal().ink_soft);
        dy += 24.0;
        const SHOWN: usize = 8;
        for (k, rel) in files.iter().enumerate().take(SHOWN) {
            if dy + 22.0 > bottom {
                break;
            }
            let r = [dx - 6.0, dy - 3.0, dw.min(340.0), 22.0];
            let ui = &mut st.s.ui;
            let hv = hover(ui, id("library-file", k), inside(r, ui.mouse), dt);
            p.rect(r, 6.0, fade(pal().ink, 0.06 * hv));
            let shown = p.fit(rel, r[2] - 12.0, 13.0, Font::Mono);
            p.text(&shown, dx, dy, 13.0, Font::Mono, mix(pal().ink_soft, pal().ink, hv));
            if let Some(folder) = &folder {
                let path = super::hstudio_modfiles::path_in(folder, rel);
                edit_file_hit(&mut st.s.ui, r, path);
            }
            dy += 22.0;
        }
        if files.len() > SHOWN && dy + 22.0 <= bottom {
            let rest = format!("and {} more in the Editor's script list", files.len() - SHOWN);
            p.text(&rest, dx, dy, 12.0, Font::Regular, pal().muted);
            dy += 22.0;
        }
        dy += 10.0;
    }
    if !commands.is_empty() && dy + 46.0 <= bottom {
        p.text("Commands", dx, top(dy + 8.0, 13.0), 13.0, Font::Medium, pal().ink_soft);
        dy += 24.0;
        for (name, help) in commands.iter().take(6) {
            if dy + 22.0 > bottom {
                break;
            }
            let cmd = p.fit(&format!("/{name}"), dw * 0.6, 13.0, Font::Mono);
            let cw = p.text(&cmd, dx, dy, 13.0, Font::Mono, pal().ink);
            let help = p.fit(help, dw - cw - 12.0, 13.0, Font::Regular);
            p.text(&help, dx + cw + 12.0, dy, 13.0, Font::Regular, pal().muted);
            dy += 22.0;
        }
        dy += 10.0;
    }
    if let Some((msg, trace)) = failure.filter(|_| dy + 38.0 <= bottom) {
        let lines = p.wrap(&msg, dw - 28.0, 12.0, Font::Mono);
        let trace = p.wrap(&trace, dw - 28.0, 11.0, Font::Mono);
        let room = ((y + h - dy - 24.0) / 18.0).max(1.0) as usize;
        let total = (lines.len() + trace.len()).min(room);
        let box_h = 20.0 + 18.0 * total as f32;
        p.rect([dx, dy, dw, box_h], 12.0, fade(pal().danger, 0.10));
        let mut ty = dy + 10.0;
        for l in lines.iter().chain(trace.iter()).take(total).enumerate() {
            let c = if l.0 < lines.len() { pal().danger } else { pal().muted };
            p.text(l.1, dx + 14.0, ty, 12.0, Font::Mono, c);
            ty += 18.0;
        }
    } else if dy + 70.0 < y + h {
        drop_hint(p, dx, y + h - 64.0, dw);
    }
}

fn drop_hint(p: &mut P, x: f32, y: f32, w: f32) {
    let r = [x, y, w, 56.0];
    p.border(r, 14.0, pal().rule);
    let t = "Drop a .lua file, a mod folder or a .zip on the game window to install it.";
    let t = p.fit(t, w - 32.0, 13.0, Font::Regular);
    p.text(&t, x + 16.0, top(y + 28.0, 13.0), 13.0, Font::Regular, pal().muted);
}

enum Doc {
    Heading(String),
    Para(String),
    Api(String, String),
}

fn docs() -> Vec<Doc> {
    let mut out = vec![
        Doc::Heading("Script mods".into()),
        Doc::Para("Script mods live in horizon-mods/.horizon-mod-data/horizon_studio/mods/. A script is one file, name.lua. A module is a folder, name/, whose main.lua loads its other files with require(\"lib\") for lib.lua: each file runs once, however its name is written, and the files share the mod's globals. The New prompt (the Library's New mod, or + in the Editor's script list) makes either; a new module has main.lua, shared.lua and lib.lua. The main file runs when the mod loads; what it registers with on(), command() and every() then runs in the game. Save any of its files, in the Editor or any other editor, and the mod reloads at once. A mod that fails stops alone: the Library shows the error, and the Editor marks its line in the file it is in.".into()),
        Doc::Para("In the Editor's script list a module is a folder that folds open to its files. A row's ... (or a right click) adds, renames and deletes a module's files, deletes a mod and makes a script a module; what is deleted goes to the trash folder beside mods, where an older copy of the same name stays, numbered. In the Library each mod's ... (or the . key) has the same, with a module's files to open. Right-click a script or a module, in the Editor's list or the Library, to rename, duplicate or delete it: a copy starts turned off, and a rename keeps its saved data and its on/off state. Ctrl+Enter on a module's file saves it and reloads the module.".into()),
        Doc::Para("Drop a .lua file, a mod folder or a .zip on the game window to install it. Lines -- name:, -- version:, -- author: and -- description: at the top name it in the Library.".into()),
        Doc::Heading("Libraries".into()),
        Doc::Para("A mod can be a library for others: it calls export(table), or its main file returns a table, and other mods take it with local kit = import(\"uikit\"). Libraries load first; when one reloads, the mods that use it reload too. A library's functions run as the mod that calls them, so what they draw, register or store belongs to that mod. Mods talk to each other with emit(\"name:event\", ...) and on(\"name:event\", fn), and json.encode and json.decode turn tables into text and back.".into()),
        Doc::Para("The bundled uikit is such a library, in plain Lua on gfx.* and canvas screens: open it in the Editor to see how it is made, or to make your own.".into()),
        Doc::Heading("ViaHorizon".into()),
        Doc::Para("Joins go through ViaProxy, which runs beside the game and translates Horizon's 26.3 to the server's version, as ViaFabricPlus does: Classic to 26.3, and Bedrock (experimental). /via chooses the version (or Auto detect), and so does Options > Mods > Horizon Studio from the title screen. Online-mode servers sign in with your own account. via.* gives scripts the same controls; /vialog prints ViaProxy's last lines.".into()),
        Doc::Heading("Console".into()),
        Doc::Para("Type Lua and press Enter: it runs at once in the game, and an expression's value is shown. /lua <code> does the same from chat. What the console registers (on, command...) lasts until clear().".into()),
        Doc::Heading("The language".into()),
        Doc::Para("Lua 5.3 without goto. Numbers are 64-bit floats (bitwise operators work on whole numbers). string (with Lua patterns, format, split, trim), math, table, utf8, coroutine and os.clock, os.time, os.date and os.difftime are there; io and the rest of os are not. Each call into a script has a step budget, so an endless loop stops the script, not the game.".into()),
        Doc::Heading("Roblox scripts".into()),
        Doc::Para("Roblox client scripts run as mods too, in Luau with the Roblox API a LocalScript uses. game.Players.LocalPlayer is you, by your name and a UserId from your UUID; game.LocalPlayer is the studio's own shortcut for it. Its Character follows you, in blocks: its Humanoid has your health, the game's WalkSpeed and JumpPower and your state, moving its HumanoidRootPart (its CFrame, PivotTo, a tween) teleports you with /tp, and Humanoid:MoveTo walks you there. Once a script looks at the world (the Workspace's children, a raycast, the mouse's Target, Players), the mobs, dropped items and orbs near you are in the Workspace and the other players in Players, raycasts and the mouse hit them, and workspace.Terrain reads the map.".into()),
        Doc::Heading("API".into()),
    ];
    let api = API
        .iter()
        .map(|(_, _, _, d)| *d)
        .chain(CONSOLE_API.iter().map(|(_, _, d)| *d));
    for (i, doc) in api.enumerate() {
        if i == API.len() {
            out.push(Doc::Heading("Console only".into()));
        }
        match doc.find("): ") {
            Some(at) => out.push(Doc::Api(doc[..=at].to_owned(), doc[at + 3..].to_owned())),
            None => out.push(Doc::Para(doc.to_owned())),
        }
    }
    out.push(Doc::Heading("Looks".into()));
    out.push(Doc::Para("Settings, the last tab, changes how the studio looks: the theme (Horizon, Horizon Light, Midnight, Nord, Rose, Forest, Solar, or one of four styles after the script executors of old Roblox days), full screen or a window, the accent colour, the size of everything, how much of the game shows behind it, the code's size and whether things move. A window is dragged by its title bar and sized from its bottom right corner; its buttons roll it up, fill the screen and close it. Right-click the title bar for the window and the theme.".into()));
    out.push(Doc::Para("The executor styles are always a window. Their buttons do the studio's work: Execute runs the editor's code, Clear empties it, Execute File saves the file and reloads its mod, Open File shows the Library, Attach and Inject reload every mod, and Options shows Settings, where the console, the Library and the docs are a click away too.".into()));
    out.push(Doc::Heading("AI agents".into()));
    out.push(Doc::Para("horizon-studio-mcp lets AI agents (Claude Code, omp, Codex, Cursor...) make mods here: they write and test them, headless and in this game, run Lua in the console and read what happened. Copy AI setup prompt, at the top right, copies a prompt for your agent that sets it all up; once one is connected, its name shows there. Options > Mods > Horizon Studio turns the AI link off.".into()));
    out.push(Doc::Para("An agent can work in this editor as you watch: it opens any script, reads it as you have it (unsaved edits too), and types its changes in at a caret of its own with its name on it. Your cursor stays where it is, the view follows the agent until you scroll, and any key or click puts the rest of its change in at once. Ctrl+Z takes a whole change back; a change it writes to the file on disk is typed in too. Its changes stay unsaved until it saves them (or you do); nothing you are typing is saved for you, but asking the game's own agent (Enter in the Agent panel, Ask, Explain or Fix in a menu, a lesson's Check) saves the file you have open first, so its turn starts from what you see (the mod reloads with it). A file you don't have open is changed on disk instead, and a note over the code shows it. The Agent panel says what it did, and Show there goes to that code.".into()));
    out.push(Doc::Para("It can point at code too: lines highlighted in a colour (red for an error, amber for a warning, green for something right) with a note. The newest note shows in a bubble under its lines, the others at the end of their line (a click opens them); the bubble's × removes a note and Esc closes it. The editor's header counts the notes: a click goes to the next, its × clears them. Notes stay on their lines as you edit.".into()));
    out.push(Doc::Para("And it can teach: a lesson is numbered steps over your code, each a note with something to try, and an explanation in the Agent panel (in the note, in the executor styles). Next and Back, or Alt+Right and Alt+Left, go through it; Check asks the agent to look at what you did; Done or the × ends it. While the agent waits for you the panel says so, and Enter sends it your answer.".into()));
    out.push(Doc::Heading("Commands in chat".into()));
    out.push(Doc::Para(
        "/studio opens this screen, /studio reload reloads every script mod, /studio help lists the commands the studio takes, /lua <code> runs code.".into(),
    ));
    out.push(Doc::Para("Every mod's command is in the chat's suggestions beside the server's: / lists it, Tab completes it, and the line under the box shows what it takes. command(name, fn, {description = ..., args = {...}}) declares its arguments: a word, text, int, number, bool, player, pos, item or block, with options or a suggest function that the chat completes. A command block's editor and /execute ... run don't offer them: the server runs those.".into()));
    out
}

/// The Docs tab as Markdown, for tools outside the game (the MCP server).
pub fn docs_markdown() -> String {
    let mut out = String::from("# Horizon Studio scripting\n");
    for d in docs() {
        match d {
            Doc::Heading(t) => out.push_str(&format!("\n## {t}\n\n")),
            Doc::Para(t) => out.push_str(&format!("{t}\n\n")),
            Doc::Api(sig, desc) => out.push_str(&format!("- `{sig}`: {desc}\n")),
        }
    }
    out
}

fn draw_docs(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    const LIT_SECS: f64 = 1.5;
    let now = st.s.now;
    st.s.ui.hits.push(([x, y, w, h], Action::Spot(Spot::Docs)));
    let width = w.min(820.0);
    // Laid out as lines of (text, size, font, colour, height), with each
    // function's entry by its name and its lines (Show in Docs goes there).
    let mut lines: Vec<(String, f32, Font, u32, f32)> = Vec::new();
    let mut entries: Vec<(String, usize, usize)> = Vec::new();
    for d in docs() {
        match d {
            Doc::Heading(t) => {
                lines.push((String::new(), 0.0, Font::Regular, 0, 10.0));
                lines.push((t, 18.0, Font::Medium, pal().ink, 32.0));
            }
            Doc::Para(t) => {
                for l in p.wrap(&t, width, 14.0, Font::Regular) {
                    lines.push((l, 14.0, Font::Regular, pal().ink_soft, 22.0));
                }
                lines.push((String::new(), 0.0, Font::Regular, 0, 8.0));
            }
            Doc::Api(sig, desc) => {
                let first = lines.len();
                for l in p.wrap(&sig, width, 13.0, Font::Mono) {
                    lines.push((l, 13.0, Font::Mono, pal().ink, 20.0));
                }
                for l in p.wrap(&desc, width, 13.0, Font::Regular) {
                    lines.push((l, 13.0, Font::Regular, pal().muted, 20.0));
                }
                let name = sig.split('(').next().unwrap_or("").to_owned();
                entries.push((name, first, lines.len()));
                lines.push((String::new(), 0.0, Font::Regular, 0, 10.0));
            }
        }
    }
    let total: f32 = lines.iter().map(|l| l.4).sum();
    let step = 22.0;
    let max = ((total - h) / step).max(0.0).ceil() as usize;
    st.s.ui.page = (h / step) as usize;
    // Show in Docs: the entry asked for at the top, at once, and tinted a moment.
    if let Some(name) = st.s.ui.docs_find.take()
        && let Some((_, first, _)) = entries.iter().find(|(n, ..)| *n == name)
    {
        let above: f32 = lines[..*first].iter().map(|l| l.4).sum();
        st.s.ui.docs_scroll = (above / step).floor() as usize;
        st.s.ui.smooth[2] = st.s.ui.docs_scroll.min(max) as f32;
        st.s.ui.docs_lit = Some((name, now));
    }
    st.s.ui.docs_scroll = st.s.ui.docs_scroll.min(max);
    smooth(&mut st.s.ui.smooth[2], st.s.ui.docs_scroll as f32, dt);
    let lit =
        st.s.ui
            .docs_lit
            .as_ref()
            .filter(|(_, at)| now - at < LIT_SECS)
            .and_then(|(name, at)| {
                let (_, a, b) = entries.iter().find(|(n, ..)| n == name)?;
                Some((*a, *b, (1.0 - (now - at) / LIT_SECS) as f32))
            });
    let mut ly = y - st.s.ui.smooth[2] * step;
    for (k, (t, size, font, c, lh)) in lines.into_iter().enumerate() {
        let shown = ly >= y - 1.0 && ly + lh <= y + h + 1.0;
        if let Some((a, b, k_lit)) = lit
            && (a..b).contains(&k)
            && shown
        {
            p.rect([x - 8.0, ly, width + 16.0, lh], 0.0, fade(pal().accent, 0.16 * k_lit));
        }
        if shown && !t.is_empty() {
            p.text(&t, x, ly + (lh - (size * 1.45).round()) / 2.0, size, font, c);
        }
        ly += lh;
    }
}

// ----------------------------------------------------------------- settings

fn draw_settings(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let look = st.s.ui.look.clone();
    let cw = w.min(780.0);
    let gap = 12.0;
    let cols = (((cw + gap) / (150.0 + gap)).floor() as usize).max(1);
    let card_w = ((cw + gap) / cols as f32 - gap).floor();
    let card_h = 98.0;
    let grid_h = THEMES.len().div_ceil(cols) as f32 * (card_h + gap);
    let row_h = 46.0;
    let total = 30.0 + grid_h + 14.0 + row_h * 7.0;
    let ui = &mut st.s.ui;
    ui.settings_scroll = ui.settings_scroll.clamp(0.0, (total - h).max(0.0));
    ui.page = (h / 40.0) as usize;
    smooth(&mut ui.smooth[3], ui.settings_scroll, dt);
    let shown = |r: [f32; 4]| r[1] >= y - 0.5 && r[1] + r[3] <= y + h + 0.5;
    let mut cy = y - ui.smooth[3];
    if total > h {
        let bar = (h * h / total).max(24.0);
        let by = y + (h - bar) * (ui.smooth[3] / (total - h)).clamp(0.0, 1.0);
        p.rect([x + cw + 8.0, by, 4.0, bar], 2.0, fade(pal().ink, 0.18));
    }
    if shown([x, cy, cw, 20.0]) {
        p.text("Theme", x, top(cy + 10.0, 13.0), 13.0, Font::Medium, pal().ink_soft);
    }
    cy += 30.0;
    for i in 0..THEMES.len() {
        let r = [
            x + (i % cols) as f32 * (card_w + gap),
            cy + (i / cols) as f32 * (card_h + gap),
            card_w,
            card_h,
        ];
        if shown(r) {
            theme_card(st, p, i, r, dt);
        }
    }
    cy += grid_h + 14.0;
    let vx = x + if cw < 460.0 { 110.0 } else { 170.0 };
    let rows: [&str; 7] = ["Window", "Accent", "Size", "Backdrop", "Code size", "Animations", ""];
    for (n, label) in rows.into_iter().enumerate() {
        let r = [x, cy, cw, row_h];
        let mid = cy + row_h / 2.0;
        cy += row_h;
        if !shown(r) {
            continue;
        }
        if n > 0 && n < 6 {
            p.rect([x, r[1], cw, 1.0], 0.0, fade(pal().rule, 0.6));
        }
        p.text(label, x, top(mid, 14.0), 14.0, Font::Regular, pal().ink);
        let ui = &mut st.s.ui;
        match n {
            0 => {
                let mut bx = vx;
                if look.theme().layout == Layout::Studio {
                    bx += segmented(
                        ui,
                        p,
                        bx,
                        mid,
                        &[
                            ("Full screen", !look.window, Action::Windowed(false)),
                            ("Window", look.window, Action::Windowed(true)),
                        ],
                        dt,
                    ) + 10.0;
                } else {
                    let t = p.fit(
                        "Always a window in this style",
                        x + cw - vx - 130.0,
                        13.0,
                        Font::Regular,
                    );
                    bx += p.text(&t, bx, top(mid, 13.0), 13.0, Font::Regular, pal().muted) + 12.0;
                }
                if look.windowed() {
                    pill(
                        ui,
                        p,
                        "reset-window",
                        "Reset window",
                        bx,
                        mid - 14.0,
                        28.0,
                        Style::Quiet,
                        Action::ResetWindow,
                        dt,
                    );
                }
            }
            1 => {
                let own = look.theme().palette.accent;
                for (i, &a) in ACCENTS.iter().enumerate() {
                    let sx = vx + 32.0 * i as f32;
                    if sx + 24.0 > x + cw {
                        break;
                    }
                    let r = [sx, mid - 12.0, 24.0, 24.0];
                    let hv = hover(ui, id("accent", i), inside(r, ui.mouse), dt);
                    let sel = glide(ui, id("accent-sel", i), look.accent == i, dt);
                    p.rect(r, 12.0, if a == 0 { own } else { a });
                    if a == 0 {
                        // The theme's own: a ring.
                        p.rect([sx + 7.0, mid - 5.0, 10.0, 10.0], 5.0, pal().page);
                    }
                    let ring = fade(pal().ink, sel.max(0.35 * hv));
                    p.border([sx - 3.0, mid - 15.0, 30.0, 30.0], 15.0, ring);
                    ui.hits.push((r, Action::Accent(i as u8)));
                }
            }
            2..=4 => {
                let which = (n - 2) as u8;
                let value = match which {
                    0 => format!("{:.0}%", look.scale * 100.0),
                    1 => format!("{:.0}%", look.opacity * 100.0),
                    _ => format!("{:.0}", look.code),
                };
                stepper(ui, p, vx, mid, &value, which, dt);
            }
            5 => {
                let r = [vx, mid - 11.0, 40.0, 22.0];
                let t = glide(ui, id("motion", 0), look.motion, dt);
                p.rect(r, 11.0, mix(pal().plate, pal().accent, t));
                p.rect(
                    [r[0] + 3.0 + 18.0 * t, mid - 8.0, 16.0, 16.0],
                    8.0,
                    mix(pal().ink_soft, pal().ink, t),
                );
                ui.hits.push(([vx - 4.0, mid - 15.0, 48.0, 30.0], Action::Motion));
            }
            _ => {
                let t = p.fit("Kept in look.txt, in the studio's folder.", cw, 12.0, Font::Regular);
                p.text(&t, x, top(mid, 12.0), 12.0, Font::Regular, pal().muted);
            }
        }
    }
}

/// A theme to pick: a small picture of it and its name.
fn theme_card(st: &mut Studio, p: &mut P, i: usize, r: [f32; 4], dt: f32) {
    let t = &THEMES[i];
    let ui = &mut st.s.ui;
    let hv = hover(ui, id("theme", i), inside(r, ui.mouse), dt);
    let sel = glide(ui, id("theme-sel", i), ui.look.theme == i, dt);
    p.rect(r, 12.0, fade(pal().ink, 0.04 + 0.05 * hv));
    super::hstudio_skins::preview(p, t, [r[0] + 8.0, r[1] + 8.0, r[2] - 16.0, r[3] - 40.0]);
    let name = p.fit(t.name, r[2] - 20.0, 13.0, Font::Medium);
    p.text(
        &name,
        r[0] + 10.0,
        top(r[1] + r[3] - 16.0, 13.0),
        13.0,
        Font::Medium,
        pal().ink,
    );
    if sel > 0.0 {
        p.border(r, 12.0, fade(pal().accent, sel));
        p.border(
            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
            11.0,
            fade(pal().accent, sel),
        );
    }
    ui.hits.push((r, Action::Theme(i as u8)));
}

/// Choices side by side, the chosen one in ink; returns the width.
fn segmented(ui: &mut Ui, p: &mut P, x: f32, cy: f32, items: &[(&str, bool, Action)], dt: f32) -> f32 {
    let widths: Vec<f32> = items
        .iter()
        .map(|(l, _, _)| p.measure(l, 13.0, Font::Medium) + 28.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    p.rect([x, cy - 16.0, total, 32.0], 16.0, pal().plate_soft);
    let mut sx = x + 2.0;
    for (i, ((label, on, action), sw)) in items.iter().zip(widths).enumerate() {
        let r = [sx, cy - 14.0, sw, 28.0];
        let t = glide(ui, id(label, i), *on, dt);
        let hv = hover(ui, id("seg", i), inside(r, ui.mouse), dt);
        p.rect(r, 14.0, fade(pal().ink, t));
        let c = mix(mix(pal().muted, pal().ink, hv), pal().page, t);
        p.text(label, sx + 14.0, top(cy, 13.0), 13.0, Font::Medium, c);
        ui.hits.push((r, *action));
        sx += sw;
    }
    total
}

/// A value with a step down and a step up.
fn stepper(ui: &mut Ui, p: &mut P, x: f32, cy: f32, value: &str, which: u8, dt: f32) {
    for (i, (glyph, up)) in [("−", false), ("+", true)].into_iter().enumerate() {
        let r = [x + 92.0 * i as f32, cy - 14.0, 28.0, 28.0];
        let hv = hover(ui, id("step", usize::from(which) * 2 + i), inside(r, ui.mouse), dt);
        p.rect(r, 14.0, mix(fade(pal().ink, 0.08), fade(pal().ink, 0.16), hv));
        let gw = p.measure(glyph, 15.0, Font::Medium);
        p.text(
            glyph,
            r[0] + (28.0 - gw) / 2.0,
            top(cy, 15.0),
            15.0,
            Font::Medium,
            pal().ink,
        );
        ui.hits.push((r, Action::Step(which, up)));
    }
    let vw = p.measure(value, 13.0, Font::Mono);
    p.text(
        value,
        x + 28.0 + (64.0 - vw) / 2.0,
        top(cy, 13.0),
        13.0,
        Font::Mono,
        pal().ink_soft,
    );
}

/// A mod as its file shows it: `<id>.lua` for a script, `<id>/` for a module.
fn mod_file(st: &Studio, id: &str) -> String {
    match st.s.mods.iter().find(|m| m.id == id) {
        Some(m) if m.folder.is_some() => format!("{id}/"),
        _ => format!("{id}.lua"),
    }
}

/// The naming prompt over everything: its heading (with the Script and
/// Module switch for a new mod), what to type, the field (none when it only
/// asks, for a delete), the line saying what Enter will do or why it can't,
/// and its keys.
fn naming(st: &mut Studio, p: &mut P, name: &LineEdit, w: f32, h: f32, dt: f32) {
    use super::hstudio_ui::NamingKind;
    let now = st.s.now;
    let arrive = progress(now, st.s.ui.naming_at, MOVE);
    let (a, dy) = (p.alpha, p.dy);
    p.alpha = a * arrive;
    p.dy = 0.0;
    p.rect([0.0, 0.0, w, h], 0.0, fade(pal().page, 0.55));
    p.dy = (1.0 - arrive) * RISE;
    let cw = 420f32.min(w - 32.0);
    let r = [((w - cw) / 2.0).floor(), (h / 2.0 - 98.0).floor(), cw, 196.0];
    p.rect(r, 16.0, pal().surface);
    p.border(r, 16.0, pal().rule);
    let kind = st.s.ui.naming_kind.clone();
    let shown = |path: &std::path::Path| super::hstudio_ui::prompt_name(st, path);
    let (heading, ask, enter) = match &kind {
        NamingKind::NewScript | NamingKind::NewModule => (
            if kind == NamingKind::NewModule {
                "New module".to_owned()
            } else {
                "New script".to_owned()
            },
            "What is it called?",
            "Enter to create  ·  Tab: Script or Module  ·  Esc to cancel",
        ),
        NamingKind::SaveAs => (
            "Save as a script".to_owned(),
            "What is it called?",
            "Enter to save  ·  Esc to cancel",
        ),
        NamingKind::Model => (
            "The agent's model".to_owned(),
            "Its name, as the agent takes it (empty: its default)",
            "Enter to use it  ·  Esc to cancel",
        ),
        NamingKind::NewFile(folder) => (
            format!("New file in {}", shown(folder)),
            "Its path in the module (lib/util makes a folder)",
            "Enter to create  ·  Esc to cancel",
        ),
        NamingKind::Rename(path) => (
            format!("Rename {}", shown(path)),
            "Its new path in the module",
            "Enter to rename  ·  Esc to cancel",
        ),
        NamingKind::Delete(path) => (
            format!("Delete {}?", shown(path)),
            if st.mod_of_path(path).is_some_and(|i| {
                let m = &st.s.mods[i];
                m.folder.as_deref() == Some(path) || (m.folder.is_none() && m.main == *path)
            }) {
                "The mod stops and leaves the Library, files and all."
            } else {
                "The module reloads without it."
            },
            "Enter to delete  ·  Esc to cancel",
        ),
        NamingKind::RenameMod(id) => (
            format!("Rename {}", mod_file(st, id)),
            "Its new name (its file's)",
            "Enter to rename  ·  Esc to cancel",
        ),
        NamingKind::Duplicate(id) => (
            format!("Duplicate {}", mod_file(st, id)),
            "The copy's name (it starts turned off)",
            "Enter to duplicate  ·  Esc to cancel",
        ),
    };
    let heading = p.fit(&heading, cw - 44.0 - 150.0, 16.0, Font::Medium);
    p.text(&heading, r[0] + 22.0, r[1] + 18.0, 16.0, Font::Medium, pal().ink);
    if matches!(kind, NamingKind::NewScript | NamingKind::NewModule) {
        let module = kind == NamingKind::NewModule;
        let items = [
            ("Script", !module, Action::NewKind(false)),
            ("Module", module, Action::NewKind(true)),
        ];
        let sw: f32 = items
            .iter()
            .map(|(l, _, _)| p.measure(l, 13.0, Font::Medium) + 28.0)
            .sum::<f32>()
            + 4.0;
        segmented(&mut st.s.ui, p, r[0] + cw - 20.0 - sw, r[1] + 30.0, &items, dt);
    }
    p.text(ask, r[0] + 22.0, r[1] + 52.0, 13.0, Font::Regular, pal().muted);
    if !matches!(kind, NamingKind::Delete(_)) {
        let f = [r[0] + 20.0, r[1] + 78.0, cw - 40.0, 40.0];
        p.rect(f, 10.0, pal().page);
        p.border(f, 10.0, fade(pal().accent, 0.6));
        line_edit(
            p,
            now,
            name,
            f[0] + 14.0,
            top(f[1] + 20.0, 14.0),
            f[2] - 28.0,
            14.0,
            Font::Regular,
            pal().ink,
        );
    }
    let (plan, c) = match super::hstudio_ui::naming_plan(st) {
        Ok(line) => (line, pal().muted),
        Err(why) => (why, pal().danger),
    };
    // A line, or two (only the Delete prompt, which has no field, has two).
    let lines: Vec<&str> = plan.lines().collect();
    for (k, line) in lines.iter().enumerate() {
        let line = p.fit(line, cw - 44.0, 12.5, Font::Regular);
        let y = r[1] + 128.0 - 17.0 * (lines.len() - 1 - k) as f32;
        p.text(&line, r[0] + 22.0, y, 12.5, Font::Regular, c);
    }
    p.text(enter, r[0] + 22.0, r[1] + 160.0, 12.0, Font::Regular, pal().muted);
    p.alpha = a;
    p.dy = dy;
}

/// The toast on the HUD: what just happened (a reload, an install).
pub fn draw_toast(st: &mut Studio, game: &mut dyn GameApi) {
    let Some((text, until)) = st.s.toast.clone() else {
        return;
    };
    if st.overlay_open() {
        return;
    }
    let now = st.s.now;
    let start = until - TOAST_SECS;
    let arrive = progress(now, start, MOVE);
    let leave = 1.0 - progress(now, until - 0.3, QUICK);
    let (w, _, _) = game.dp();
    use_look(&st.s.ui.look, true);
    let mut p = P {
        g: game,
        alpha: arrive * leave,
        dy: (1.0 - arrive) * -RISE,
        s: 1.0,
    };
    let tw = p.measure(&text, 13.0, Font::Regular);
    let r = [((w - tw) / 2.0 - 28.0).floor(), 18.0, tw + 46.0, 36.0];
    p.rect(r, 18.0, fade(pal().surface, 0.96));
    p.border(r, 18.0, pal().rule);
    p.rect([r[0] + 16.0, r[1] + 15.0, 6.0, 6.0], 3.0, pal().accent);
    p.text(
        &text,
        r[0] + 30.0,
        top(r[1] + 18.0, 13.0),
        13.0,
        Font::Regular,
        pal().ink,
    );
}

// ----------------------------------------------------------------- script screens

fn row_height(p: &mut P, wt: &super::hstudio_lua::TableRef, inner: f32) -> f32 {
    match kind(wt).as_str() {
        "button" => 44.0,
        "toggle" => 40.0,
        "slider" => 50.0,
        "input" => 62.0,
        "space" => (field_num(wt, "height", 12.0) as f32).clamp(0.0, 300.0),
        _ => {
            21.0 * p
                .wrap(&field_text(wt, "text").unwrap_or_default(), inner, 14.0, Font::Regular)
                .len() as f32
                + 6.0
        }
    }
}

/// A script's screen (`ui.open`): a panel of its widgets when it has a
/// title or widgets, then its own `draw(w, h)` in GUI pixels.
pub fn draw_screen(st: &mut Studio, game: &mut dyn GameApi) {
    let Some(sc) = &st.s.screen else {
        return;
    };
    let table = sc.table.clone();
    let (focus, edit) = (sc.focus, sc.edit.clone());
    let now = st.s.now;
    let (w, h, k) = game.dp();
    use_look(&st.s.ui.look, true);
    st.s.ui.dp = k;
    let dt = tick(&mut st.s.ui, now);
    let (alpha, rise) = presence(&st.s.ui, now);
    let ws = widgets(&table);
    let title = field_text(&table, "title");
    // A canvas draws itself, in dp; the studio adds only the dimmed backdrop.
    let canvas = field(&table, "canvas").truthy();
    let mut rows = Vec::new();
    {
        let mut p = P {
            g: &mut *game,
            alpha,
            dy: 0.0,
            s: 1.0,
        };
        if !matches!(field(&table, "backdrop"), super::hstudio_lua::Value::Bool(false)) {
            p.rect([0.0, 0.0, w, h], 0.0, fade(pal().page, 0.6));
        }
        p.dy = rise;
        if !canvas && (title.is_some() || !ws.is_empty()) {
            let pw = (field_num(&table, "width", 340.0) as f32).clamp(200.0, (w - 32.0).max(200.0));
            let inner = pw - 48.0;
            let heights: Vec<f32> = ws.iter().map(|wt| row_height(&mut p, wt, inner)).collect();
            let head = if title.is_some() { 54.0 } else { 18.0 };
            let ph = head + heights.iter().sum::<f32>() + 18.0;
            let (px, py) = (((w - pw) / 2.0).floor(), ((h - ph) / 2.0).floor().max(16.0));
            p.rect([px, py, pw, ph], 18.0, pal().surface);
            p.border([px, py, pw, ph], 18.0, pal().rule);
            if let Some(t) = &title {
                let t = p.fit(t, inner, 18.0, Font::Medium);
                p.text(&t, px + 24.0, top(py + 30.0, 18.0), 18.0, Font::Medium, pal().ink);
            }
            let mut y = py + head;
            let mouse = st.s.ui.mouse;
            for (i, wt) in ws.iter().enumerate() {
                let r = [px + 24.0, y, inner, heights[i] - 4.0];
                let over = inside(r, mouse);
                let label = field_text(wt, "text").unwrap_or_default();
                match kind(wt).as_str() {
                    "button" => {
                        let t = hover(&mut st.s.ui, id("sbtn", i), over, dt);
                        let primary = field(wt, "primary").truthy();
                        let (bg, fg) = if primary {
                            (mix(pal().accent, pal().accent_hover, t), on_accent())
                        } else {
                            (mix(fade(pal().ink, 0.08), fade(pal().ink, 0.15), t), pal().ink)
                        };
                        let b = [r[0], r[1] + 2.0, r[2], 36.0];
                        p.rect(b, 18.0, bg);
                        let l = p.fit(&label, r[2] - 24.0, 13.0, Font::Medium);
                        let lw = p.measure(&l, 13.0, Font::Medium);
                        p.text(
                            &l,
                            (b[0] + (b[2] - lw) / 2.0).floor(),
                            top(b[1] + 18.0, 13.0),
                            13.0,
                            Font::Medium,
                            fg,
                        );
                    }
                    "toggle" => {
                        let on = field(wt, "value").truthy();
                        let t = glide(&mut st.s.ui, id("switch", i), on, dt);
                        let hv = hover(&mut st.s.ui, id("stog", i), over, dt);
                        if hv > 0.0 {
                            p.rect([r[0] - 10.0, r[1], r[2] + 20.0, r[3]], 10.0, fade(pal().ink, 0.05 * hv));
                        }
                        let cy = r[1] + r[3] / 2.0;
                        let l = p.fit(&label, r[2] - 60.0, 14.0, Font::Regular);
                        p.text(&l, r[0], top(cy, 14.0), 14.0, Font::Regular, pal().ink);
                        let track = [r[0] + r[2] - 40.0, cy - 11.0, 40.0, 22.0];
                        p.rect(track, 11.0, mix(pal().plate, pal().accent, t));
                        let kx = track[0] + 3.0 + (track[2] - 22.0) * t;
                        p.rect([kx, cy - 8.0, 16.0, 16.0], 8.0, mix(pal().ink_soft, pal().ink, t));
                    }
                    "slider" => {
                        let (min, max) = (field_num(wt, "min", 0.0), field_num(wt, "max", 1.0));
                        let step = field_num(wt, "step", 0.0);
                        let v = field_num(wt, "value", min).clamp(min.min(max), max.max(min));
                        let vs = slider_text(v, step);
                        let vw = p.measure(&vs, 12.0, Font::Mono);
                        let l = p.fit(&label, r[2] - vw - 12.0, 13.0, Font::Regular);
                        p.text(&l, r[0], r[1] + 2.0, 13.0, Font::Regular, pal().ink);
                        p.text(&vs, r[0] + r[2] - vw, r[1] + 3.0, 12.0, Font::Mono, pal().ink_soft);
                        let ty = r[1] + 32.0;
                        let frac = if max > min {
                            ((v - min) / (max - min)) as f32
                        } else {
                            0.0
                        };
                        p.rect([r[0], ty - 2.0, r[2], 4.0], 2.0, pal().plate);
                        p.rect([r[0], ty - 2.0, r[2] * frac, 4.0], 2.0, pal().accent);
                        let hv = hover(&mut st.s.ui, id("sknob", i), over, dt);
                        let kx = r[0] + r[2] * frac;
                        let kr = 7.0 + 2.0 * hv;
                        if hv > 0.0 {
                            p.rect(
                                [kx - kr - 4.0, ty - kr - 4.0, 2.0 * kr + 8.0, 2.0 * kr + 8.0],
                                kr + 4.0,
                                fade(pal().accent, 0.22 * hv),
                            );
                        }
                        p.rect([kx - kr, ty - kr, 2.0 * kr, 2.0 * kr], kr, pal().ink);
                    }
                    "input" => {
                        let l = p.fit(&label, r[2], 12.0, Font::Regular);
                        p.text(&l, r[0], r[1], 12.0, Font::Regular, pal().muted);
                        let f = [r[0], r[1] + 20.0, r[2], 36.0];
                        p.rect(f, 10.0, pal().page);
                        let focused = focus == Some(i);
                        let ring = glide(&mut st.s.ui, id("sfocus", i), focused, dt);
                        p.border(f, 10.0, mix(pal().rule, pal().accent, ring));
                        let ty = top(f[1] + 18.0, 14.0);
                        if focused {
                            line_edit(
                                &mut p,
                                now,
                                &edit,
                                f[0] + 12.0,
                                ty,
                                f[2] - 24.0,
                                14.0,
                                Font::Regular,
                                pal().ink,
                            );
                        } else {
                            let value = field_text(wt, "value").unwrap_or_default();
                            let (shown, c) = if value.is_empty() {
                                (field_text(wt, "placeholder").unwrap_or_default(), pal().muted)
                            } else {
                                (value, pal().ink)
                            };
                            let shown = p.fit(&shown, f[2] - 24.0, 14.0, Font::Regular);
                            p.text(&shown, f[0] + 12.0, ty, 14.0, Font::Regular, c);
                        }
                    }
                    "space" => {}
                    _ => {
                        let c = match field(wt, "color") {
                            super::hstudio_lua::Value::Nil => pal().ink_soft,
                            v => {
                                let lua = super::hstudio_lua::Lua::new();
                                super::hstudio::color_arg(&lua, &[v], 0, pal().ink_soft).unwrap_or(pal().ink_soft)
                                    | 0xFF00_0000
                            }
                        };
                        for (j, l) in p.wrap(&label, r[2], 14.0, Font::Regular).iter().enumerate() {
                            p.text(l, r[0], r[1] + 2.0 + 21.0 * j as f32, 14.0, Font::Regular, c);
                        }
                    }
                }
                rows.push((r, i));
                y += heights[i];
            }
        }
    }
    if let Some(sc) = &mut st.s.screen {
        sc.rows = rows;
    }
    let f = field(&table, "draw");
    if f.is_callable() {
        let (gw, gh) = if canvas {
            (w, h)
        } else {
            (w / k.max(0.01), h / k.max(0.01))
        };
        st.screen_call(
            game,
            &f,
            vec![
                super::hstudio_lua::Value::Num(f64::from(gw)),
                super::hstudio_lua::Value::Num(f64::from(gh)),
            ],
            true,
        );
    }
}
