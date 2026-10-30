// Horizon Studio's look: Horizon's design system (docs/design-system.md),
// drawn through GameApi's dp calls (Geist and Geist Mono, rounded plates) in
// the launcher's logical pixels. Neutrals do the work and the violet marks
// the main action; selection is ink. Motion follows the tokens: QUICK for
// hovers, MOVE for selection and pages, ENTER (a fade and an 8 dp rise)
// for content arriving, rows 40 ms apart.

use super::hstudio::{API, CONSOLE_API, Font, GameApi, LogKind, ModState, Studio};
use super::hstudio_examples::EXAMPLES;
use super::hstudio_ui::{
    Action, EditorView, LineEdit, Long, Tab, Tint, Ui, complete, field, field_num, field_text, highlight, kind,
    slider_text, widgets,
};

// The dark palette, 0xAARRGGBB.
const PAGE: u32 = 0xFF0D_0E13;
const SURFACE: u32 = 0xFF16_181F;
const PLATE_SOFT: u32 = 0xFF18_1A22;
const PLATE: u32 = 0xFF20_222C;
const RULE: u32 = 0xFF24_262F;
const INK: u32 = 0xFFEC_EDF3;
const INK_SOFT: u32 = 0xFFC5_C7D1;
const MUTED: u32 = 0xFF8B_8F9C;
const ACCENT: u32 = 0xFF9A_86FF;
const ACCENT_HOVER: u32 = 0xFFB0_A0FF;
const DANGER: u32 = 0xFFF0_7A68;
const SUCCESS: u32 = 0xFF45_C27A;
// Code, quiet beside the ink.
const KEYWORD: u32 = 0xFFB4_A7FF;
const STRING: u32 = 0xFF8F_D6A8;
const NUMBER: u32 = 0xFFF2_C48D;
const COMMENT: u32 = 0xFF6E_7280;
const GLOBAL: u32 = 0xFF9F_C2FF;

/// The editor's and console's code: Geist Mono at this size, one line per row.
pub const CODE_SIZE: f32 = 13.0;
pub const CODE_LINE: f32 = 20.0;

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
    bezier(m.curve, ((now - since) / m.secs).clamp(0.0, 1.0) as f32)
}

fn fade(c: u32, a: f32) -> u32 {
    let alpha = ((c >> 24) as f32 * a.clamp(0.0, 1.0)).round() as u32;
    (c & 0x00FF_FFFF) | alpha << 24
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

fn id(s: &str, n: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes().chain(n.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A hover (or other) fade toward `on`, eased.
fn hover(ui: &mut Ui, key: u64, on: bool, dt: f32) -> f32 {
    let v = ui.anim.entry(key).or_insert(0.0);
    let step = dt / QUICK.secs as f32;
    *v = if on { (*v + step).min(1.0) } else { (*v - step).max(0.0) };
    bezier(QUICK.curve, *v)
}

/// A switch's or selection's move toward `on`, eased.
fn glide(ui: &mut Ui, key: u64, on: bool, dt: f32) -> f32 {
    let v = ui.anim.entry(key).or_insert(if on { 1.0 } else { 0.0 });
    let step = dt / MOVE.secs as f32;
    *v = if on { (*v + step).min(1.0) } else { (*v - step).max(0.0) };
    bezier(MOVE.curve, *v)
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// The top of a `size` line box centred on `y`.
fn top(y: f32, size: f32) -> f32 {
    (y - (size * 1.45).round() / 2.0).floor()
}

/// Drawing with a fade and a rise applied, for whatever is arriving.
struct P<'g> {
    g: &'g mut dyn GameApi,
    alpha: f32,
    dy: f32,
}

impl P<'_> {
    fn rect(&mut self, r: [f32; 4], radius: f32, c: u32) {
        let c = fade(c, self.alpha);
        if c >> 24 != 0 && r[2] > 0.0 && r[3] > 0.0 {
            self.g.dp_rect([r[0], r[1] + self.dy, r[2], r[3]], radius, c);
        }
    }
    fn border(&mut self, r: [f32; 4], radius: f32, c: u32) {
        let c = fade(c, self.alpha);
        if c >> 24 != 0 {
            self.g.dp_border([r[0], r[1] + self.dy, r[2], r[3]], radius, c);
        }
    }
    fn text(&mut self, s: &str, x: f32, y: f32, size: f32, font: Font, c: u32) -> f32 {
        if s.is_empty() {
            return 0.0;
        }
        self.g.dp_text(s, [x, y + self.dy], size, font, fade(c, self.alpha))
    }
    fn measure(&mut self, s: &str, size: f32, font: Font) -> f32 {
        self.g.dp_measure(s, size, font)
    }
    /// The longest start of `s` that fits in `max`, with an ellipsis when cut.
    fn fit(&mut self, s: &str, max: f32, size: f32, font: Font) -> String {
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
    fn wrap(&mut self, s: &str, max: f32, size: f32, font: Font) -> Vec<String> {
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
                    lines.push(line.trim_end().to_owned());
                }
                let mut piece = String::new();
                for c in word.chars() {
                    piece.push(c);
                    if self.measure(&piece, size, font) > max {
                        piece.pop();
                        lines.push(std::mem::take(&mut piece));
                        piece.push(c);
                    }
                }
                line = piece;
            }
            lines.push(line.trim_end().to_owned());
        }
        lines
    }
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
        Style::Primary => (mix(ACCENT, ACCENT_HOVER, t), PAGE),
        Style::Secondary => (mix(fade(INK, 0.08), fade(INK, 0.15), t), INK),
        Style::Quiet => (fade(INK, 0.10 * t), mix(INK_SOFT, INK, t)),
    };
    p.rect(r, h / 2.0, bg);
    p.text(label, x + 14.0, top(y + h / 2.0, size), size, Font::Medium, fg);
    ui.hits.push((r, action));
    w
}

/// A key's name on a small key cap, then what it does; returns the width.
fn keycap(p: &mut P, keys: &str, what: &str, x: f32, cy: f32) -> f32 {
    let kw = p.measure(keys, 11.0, Font::Mono) + 12.0;
    p.rect([x, cy - 10.0, kw, 20.0], 5.0, fade(INK, 0.06));
    p.border([x, cy - 10.0, kw, 20.0], 5.0, RULE);
    p.text(keys, x + 6.0, top(cy, 11.0), 11.0, Font::Mono, INK_SOFT);
    let ww = p.text(what, x + kw + 6.0, top(cy, 12.0), 12.0, Font::Regular, MUTED);
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
    *v += (target - *v) * (1.0 - (-dt * 18.0).exp());
    if (target - *v).abs() < 0.01 {
        *v = target;
    }
}

// ----------------------------------------------------------------- the studio

const TABS: [(Tab, &str); 4] = [
    (Tab::Console, "Console"),
    (Tab::Editor, "Editor"),
    (Tab::Library, "Library"),
    (Tab::Docs, "Docs"),
];

pub fn draw(st: &mut Studio, game: &mut dyn GameApi) {
    let now = st.s.now;
    let (w, h, k) = game.dp();
    let dt = {
        let ui = &mut st.s.ui;
        ui.dp = k;
        ui.hits.clear();
        ui.view = None;
        tick(ui, now)
    };
    let (alpha, rise) = presence(&st.s.ui, now);
    let mut p = P {
        g: game,
        alpha,
        dy: 0.0,
    };
    p.rect([0.0, 0.0, w, h], 0.0, fade(PAGE, 0.92));
    p.dy = rise;
    let gutter = if w >= 900.0 { 48.0 } else { 20.0 };
    let cw = (w - 2.0 * gutter).clamp(200.0, 1280.0);
    let x0 = ((w - cw) / 2.0).floor();
    header(st, &mut p, x0, 24.0, cw, dt);
    let tab = st.s.ui.tab;
    let shown = progress(now, st.s.ui.tab_at, MOVE);
    let body = [x0, 80.0, cw, (h - 80.0 - 52.0).max(40.0)];
    p.alpha = alpha * shown;
    p.dy = rise + (1.0 - shown) * RISE;
    match tab {
        Tab::Console => draw_console(st, &mut p, body),
        Tab::Editor => draw_editor(st, &mut p, body, dt),
        Tab::Library => draw_library(st, &mut p, body, dt),
        Tab::Docs => draw_docs(st, &mut p, body, dt),
    }
    p.alpha = alpha;
    p.dy = rise;
    footer(st, &mut p, x0, h - 28.0, cw);
    if let Some(name) = st.s.ui.naming.clone() {
        naming(st, &mut p, &name, w, h);
    }
}

fn header(st: &mut Studio, p: &mut P, x: f32, y: f32, w: f32, dt: f32) {
    let now = st.s.now;
    let ww = p.text("Horizon Studio", x, top(y + 16.0, 16.0), 16.0, Font::Medium, INK);
    let mut sx = x + ww + 28.0;
    let tx = sx;
    let mut segs = Vec::new();
    for (tab, label) in TABS {
        let sw = p.measure(label, 13.0, Font::Medium) + 28.0;
        segs.push((tab, label, [sx + 2.0, y + 2.0, sw, 28.0]));
        sx += sw;
    }
    p.rect([tx, y, sx - tx + 4.0, 32.0], 16.0, PLATE_SOFT);
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
    p.rect([ui.pill[0], y + 2.0, ui.pill[1], 28.0], 14.0, INK);
    for (tab, label, r) in segs {
        let on = tab == ui.tab;
        let hv = hover(ui, id("tab", tab as usize), inside(r, ui.mouse), dt);
        let c = if on { PAGE } else { mix(MUTED, INK, hv) };
        p.text(label, r[0] + 14.0, top(y + 16.0, 13.0), 13.0, Font::Medium, c);
        ui.hits.push((r, Action::Tab(tab)));
    }
    // Close, at the right.
    let r = [x + w - 32.0, y, 32.0, 32.0];
    let hv = hover(ui, id("close", 0), inside(r, ui.mouse), dt);
    p.rect(r, 16.0, fade(INK, 0.10 * hv));
    let cw = p.measure("×", 20.0, Font::Regular);
    p.text(
        "×",
        r[0] + (32.0 - cw) / 2.0,
        top(y + 15.0, 20.0),
        20.0,
        Font::Regular,
        mix(MUTED, INK, hv),
    );
    ui.hits.push((r, Action::Close));
    let esc_w = p.measure("Esc", 11.0, Font::Mono) + 12.0;
    let esc_x = r[0] - esc_w - 10.0;
    keycap(p, "Esc", "", esc_x, y + 16.0);
    // The AI link: the agent connected, else the prompt that sets one up.
    let right = esc_x - 16.0;
    match super::hstudio_link::agents_line() {
        Some(line) => {
            let cw = p.measure(&line, 12.0, Font::Medium) + 36.0;
            let cx = right - cw;
            if cx > sx + 16.0 {
                let pulse = 0.75 + 0.25 * (now as f32 * 2.4).sin();
                p.rect([cx, y + 2.0, cw, 28.0], 14.0, fade(SUCCESS, 0.14));
                p.rect([cx + 13.0, y + 12.0, 8.0, 8.0], 4.0, fade(SUCCESS, pulse));
                p.text(&line, cx + 27.0, top(y + 16.0, 12.0), 12.0, Font::Medium, INK);
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
    let hints: &[(&str, &str)] = match st.s.ui.tab {
        Tab::Console => &[
            ("⏎", "run"),
            ("↑ ↓", "history"),
            ("Tab", "complete"),
            ("Ctrl L", "clear"),
        ],
        Tab::Editor => &[
            ("Ctrl S", "save and reload"),
            ("Ctrl ⏎", "run"),
            ("Ctrl Z", "undo"),
            ("Ctrl /", "comment"),
            ("Ctrl D", "duplicate"),
        ],
        Tab::Library => &[("⏎", "edit"), ("Space", "on or off"), ("R", "reload"), ("N", "new mod")],
        Tab::Docs => &[("↑ ↓", "scroll"), ("PgUp PgDn", "page")],
    };
    let mut hx = x;
    for (keys, what) in hints {
        if hx > x + w - 120.0 {
            break;
        }
        hx += keycap(p, keys, what, hx, cy) + 18.0;
    }
    let tabs = "Ctrl 1-4";
    let tw = p.measure(tabs, 11.0, Font::Mono) + 12.0 + 6.0 + p.measure("tabs", 12.0, Font::Regular);
    if hx + tw < x + w {
        keycap(p, tabs, "tabs", x + w - tw, cy);
    }
}

#[allow(clippy::too_many_arguments)]
fn line_edit(p: &mut P, now: f64, e: &LineEdit, x: f32, y: f32, max: f32, size: f32, font: Font, c: u32) {
    let mut start = 0;
    while start < e.cursor && p.measure(&e.text[start..e.cursor], size, font) > max - 6.0 {
        start += e.text[start..].chars().next().map_or(1, char::len_utf8);
    }
    let shown = p.fit(&e.text[start..], max, size, font);
    p.text(&shown, x, y, size, font, c);
    let blink = 0.55 + 0.45 * (now as f32 * std::f32::consts::PI * 1.6).cos();
    let cx = x + p.measure(&e.text[start..e.cursor], size, font);
    let line = (size * 1.45).round();
    p.rect([cx.floor(), y + 1.0, 2.0, line - 2.0], 1.0, fade(ACCENT, blink));
}

fn log_color(kind: LogKind) -> u32 {
    match kind {
        LogKind::Input => INK,
        LogKind::Output => INK_SOFT,
        LogKind::Error => DANGER,
        LogKind::Trace => MUTED,
        LogKind::Info => mix(INK_SOFT, ACCENT, 0.35),
    }
}

fn draw_console(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4]) {
    let now = st.s.now;
    p.rect([x, y, w, h], 14.0, PLATE_SOFT);
    let pad = 18.0;
    let field_h = 44.0;
    let log_top = y + pad;
    let log_bottom = y + h - pad - field_h - 10.0;
    let rows = ((log_bottom - log_top) / CODE_LINE).max(1.0) as usize;
    st.s.ui.page = rows;
    let need = rows + st.s.ui.log_scroll + 1;
    let text_w = w - 2.0 * pad - 20.0;
    let mut shown: Vec<(String, u32, f64, bool)> = Vec::new();
    for line in st.s.log.iter().rev() {
        let (body, prompt) = match line.text.strip_prefix("> ") {
            Some(rest) if line.kind == LogKind::Input => (rest, true),
            _ => (line.text.as_str(), false),
        };
        let pieces = p.wrap(body, text_w, CODE_SIZE, Font::Mono);
        let n = pieces.len();
        for (i, piece) in pieces.into_iter().enumerate().rev() {
            shown.push((piece, log_color(line.kind), line.at, prompt && i == 0 && n > 0));
        }
        if shown.len() >= need {
            break;
        }
    }
    let max_scroll = shown.len().saturating_sub(rows);
    st.s.ui.log_scroll = st.s.ui.log_scroll.min(max_scroll);
    let skip = st.s.ui.log_scroll;
    let base = p.alpha;
    for (i, (line, c, at, prompt)) in shown.iter().skip(skip).take(rows).enumerate() {
        let ly = log_bottom - CODE_LINE * (i as f32 + 1.0);
        p.alpha = base * progress(now, *at, QUICK);
        let ty = ly + (CODE_LINE - (CODE_SIZE * 1.45).round()) / 2.0;
        if *prompt {
            p.text("›", x + pad, ty, CODE_SIZE, Font::Mono, ACCENT);
        }
        p.text(line, x + pad + 20.0, ty, CODE_SIZE, Font::Mono, *c);
    }
    p.alpha = base;
    if st.s.log.is_empty() {
        p.text(
            "Nothing yet. Try player.pos() or help().",
            x + pad,
            top(log_bottom - 12.0, 13.0),
            13.0,
            Font::Regular,
            MUTED,
        );
    }
    if skip > 0 {
        let more = format!("{skip} below");
        let mw = p.measure(&more, 11.0, Font::Mono) + 16.0;
        p.rect([x + w - pad - mw, log_bottom - 22.0, mw, 20.0], 10.0, fade(INK, 0.10));
        p.text(
            &more,
            x + w - pad - mw + 8.0,
            top(log_bottom - 12.0, 11.0),
            11.0,
            Font::Mono,
            INK_SOFT,
        );
    }
    // The input.
    let f = [x + 10.0, y + h - 10.0 - field_h, w - 20.0, field_h];
    p.rect(f, 12.0, PAGE);
    p.border(f, 12.0, fade(ACCENT, 0.45));
    let cy = f[1] + field_h / 2.0;
    p.text("›", f[0] + 16.0, top(cy, 16.0), 16.0, Font::Mono, ACCENT);
    let input = st.s.ui.input.clone();
    let tx = f[0] + 36.0;
    let ty = top(cy, 14.0);
    if input.text.is_empty() {
        p.text(
            "Type Lua and press Enter. help() lists the API.",
            tx,
            ty,
            14.0,
            Font::Regular,
            MUTED,
        );
    }
    line_edit(p, now, &input, tx, ty, f[2] - 52.0, 14.0, Font::Mono, INK);
    if input.cursor == input.text.len()
        && let Some(rest) = complete(&input.text, input.cursor)
    {
        let cx = tx + p.measure(&input.text, 14.0, Font::Mono) + 2.0;
        if cx + p.measure(&rest, 14.0, Font::Mono) < f[0] + f[2] - 16.0 {
            p.text(&rest, cx, ty, 14.0, Font::Mono, fade(MUTED, 0.8));
        }
    }
}

fn tint(t: Tint) -> u32 {
    match t {
        Tint::Plain => INK,
        Tint::Keyword => KEYWORD,
        Tint::Str => STRING,
        Tint::Number => NUMBER,
        Tint::Comment => COMMENT,
        Tint::Global => GLOBAL,
    }
}

fn draw_editor(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    let title = st.s.editor.title.clone();
    let tw = p.text(&title, x, top(y + 16.0, 15.0), 15.0, Font::Medium, INK);
    if st.s.editor.dirty {
        p.rect([x + tw + 10.0, y + 13.0, 7.0, 7.0], 3.5, ACCENT);
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
    if let Some(problem) = st.s.editor.problem.clone() {
        let px = x + tw + 28.0;
        let msg = p.fit(
            &format!("Could not read the file: {problem}"),
            bx - px - 16.0,
            13.0,
            Font::Regular,
        );
        p.text(&msg, px, top(y + 16.0, 13.0), 13.0, Font::Regular, DANGER);
    }
    let panel = [x, y + 46.0, w, h - 46.0];
    p.rect(panel, 14.0, PLATE_SOFT);
    let top_y = panel[1] + 12.0;
    let rows = ((panel[3] - 24.0) / CODE_LINE).max(1.0) as usize;
    st.s.ui.page = rows;
    let gutter = p.measure("0000", 12.0, Font::Mono) + 30.0;
    let text_x = panel[0] + gutter;
    let text_w = panel[2] - gutter - 18.0;
    let ed = &mut st.s.editor;
    if ed.row < ed.scroll {
        ed.scroll = ed.row;
    }
    if ed.row >= ed.scroll + rows {
        ed.scroll = ed.row + 1 - rows;
    }
    ed.scroll = ed.scroll.min(ed.lines.len().saturating_sub(1));
    let cur_chars = ed.lines[ed.row][..ed.col].chars().count();
    if cur_chars < ed.hscroll {
        ed.hscroll = cur_chars.saturating_sub(4);
    }
    loop {
        let line = &ed.lines[ed.row];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if from > ed.col || p.measure(&line[from..ed.col], CODE_SIZE, Font::Mono) <= text_w - 8.0 {
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
    let ed = &st.s.editor;
    let s = st.s.ui.smooth[1];
    let first = s.floor().max(0.0) as usize;
    let frac = s - first as f32;
    let sel = ed.selection();
    let bottom = panel[1] + panel[3] - 8.0;
    let blink = 0.55 + 0.45 * (now as f32 * std::f32::consts::PI * 1.6).cos();
    for i in 0..=rows {
        let r = first + i;
        if r >= ed.lines.len() {
            break;
        }
        let ly = top_y + (i as f32 - frac) * CODE_LINE;
        if ly < top_y - 0.5 || ly + CODE_LINE > bottom + 0.5 {
            continue;
        }
        let line = &ed.lines[r];
        let from = line.char_indices().nth(ed.hscroll).map_or(line.len(), |(i, _)| i);
        if r == ed.row {
            p.rect([panel[0] + 6.0, ly, panel[2] - 12.0, CODE_LINE], 6.0, fade(INK, 0.045));
        }
        if ed.error_line == Some(r + 1) {
            p.rect(
                [panel[0] + 6.0, ly, panel[2] - 12.0, CODE_LINE],
                6.0,
                fade(DANGER, 0.14),
            );
            p.rect([panel[0] + 12.0, ly + 7.0, 6.0, 6.0], 3.0, DANGER);
        }
        if let Some(((r1, c1), (r2, c2))) = sel
            && r >= r1
            && r <= r2
        {
            let a = if r == r1 { c1 } else { 0 }.max(from);
            let b = if r == r2 { c2 } else { line.len() }.max(a);
            let sx = text_x + p.measure(&line[from..a], CODE_SIZE, Font::Mono);
            let mut sw = p.measure(&line[a..b], CODE_SIZE, Font::Mono);
            if r != r2 {
                sw += 6.0;
            }
            let sw = sw.min(text_x + text_w - sx).max(0.0);
            p.rect([sx, ly + 1.0, sw, CODE_LINE - 2.0], 3.0, fade(ACCENT, 0.30));
        }
        let ty = ly + (CODE_LINE - (CODE_SIZE * 1.45).round()) / 2.0;
        let num = format!("{}", r + 1);
        let nw = p.measure(&num, 12.0, Font::Mono);
        let nc = if r == ed.row { INK_SOFT } else { fade(MUTED, 0.7) };
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
            let pw = p.measure(piece, CODE_SIZE, Font::Mono);
            let shown = if pw > room {
                p.fit(piece, room, CODE_SIZE, Font::Mono)
            } else {
                piece.to_owned()
            };
            p.text(&shown, px, ty, CODE_SIZE, Font::Mono, tint(t));
            px += pw;
        }
        if r == ed.row && ed.col >= from {
            let cx = text_x + p.measure(&line[from..ed.col], CODE_SIZE, Font::Mono);
            if cx <= text_x + text_w {
                p.rect([cx.floor(), ly + 2.0, 2.0, CODE_LINE - 4.0], 1.0, fade(ACCENT, blink));
            }
        }
    }
    if ed.lines.len() > rows {
        let track = panel[3] - 24.0;
        let bar = (track * rows as f32 / ed.lines.len() as f32).max(24.0);
        let span = (ed.lines.len() - rows).max(1) as f32;
        let by = top_y + (track - bar) * (s / span).clamp(0.0, 1.0);
        p.rect([panel[0] + panel[2] - 9.0, by, 4.0, bar], 2.0, fade(INK, 0.18));
    }
    st.s.ui.view = Some(EditorView {
        x: text_x,
        y: top_y,
        w: text_w,
        rows,
    });
}

fn status(state: &ModState, enabled: bool) -> (&'static str, u32) {
    match state {
        ModState::Running => ("Running", SUCCESS),
        ModState::Failed(..) => ("Failed", DANGER),
        ModState::Off if !enabled => ("Off", MUTED),
        ModState::Off => ("Not loaded", MUTED),
    }
}

fn draw_library(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let now = st.s.now;
    let list_w = (w * 0.42).clamp(240.0, 460.0).min(w * 0.55);
    let n = st.s.mods.len();
    if n > 0 {
        st.s.ui.library_sel = st.s.ui.library_sel.min(n - 1);
    }
    let lw = p.text("Script mods", x, top(y + 10.0, 13.0), 13.0, Font::Medium, INK_SOFT);
    let count = format!("{n}");
    let cw = p.measure(&count, 11.0, Font::Mono) + 12.0;
    p.rect([x + lw + 8.0, y + 1.0, cw, 18.0], 9.0, fade(INK, 0.08));
    p.text(&count, x + lw + 14.0, top(y + 10.0, 11.0), 11.0, Font::Mono, MUTED);
    let examples_h = (EXAMPLES.len() as f32 * 34.0 + 34.0).min(h * 0.42);
    let list_top = y + 30.0;
    let list_h = (h - 30.0 - 52.0 - examples_h).max(52.0);
    let row_h = 52.0;
    let max_rows = ((list_h / row_h).floor() as usize).max(1);
    let first = st.s.ui.library_sel.saturating_sub(max_rows - 1);
    let (base_a, base_dy) = (p.alpha, p.dy);
    let mut ly = list_top;
    if n == 0 {
        p.text(
            "None yet. Make one, install an example,",
            x + 4.0,
            top(ly + 14.0, 14.0),
            14.0,
            Font::Regular,
            INK_SOFT,
        );
        p.text(
            "or drop a .lua file on the game window.",
            x + 4.0,
            top(ly + 34.0, 14.0),
            14.0,
            Font::Regular,
            MUTED,
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
        p.rect(r, 12.0, fade(INK, 0.06 * hv + 0.07 * sel));
        let m = &st.s.mods[i];
        let (_, dot) = status(&m.state, m.enabled);
        let cy = ly + (row_h - 4.0) / 2.0;
        p.rect([x + 16.0, cy - 4.0, 8.0, 8.0], 4.0, dot);
        let ver = m.version.clone();
        let vw = if ver.is_empty() {
            0.0
        } else {
            p.measure(&ver, 11.0, Font::Mono)
        };
        let name_w = list_w - 52.0 - vw;
        let name = p.fit(&m.name, name_w, 14.0, Font::Medium);
        let desc = if m.description.is_empty() {
            format!("{}.lua", m.id)
        } else {
            m.description.clone()
        };
        let desc = p.fit(&desc, list_w - 52.0, 12.0, Font::Regular);
        let name_c = if m.enabled { INK } else { MUTED };
        p.text(&name, x + 36.0, top(cy - 8.0, 14.0), 14.0, Font::Medium, name_c);
        p.text(&desc, x + 36.0, top(cy + 10.0, 12.0), 12.0, Font::Regular, MUTED);
        if !ver.is_empty() {
            p.text(
                &ver,
                x + list_w - 14.0 - vw,
                top(cy - 8.0, 11.0),
                11.0,
                Font::Mono,
                MUTED,
            );
        }
        st.s.ui.hits.push((r, Action::Select(i)));
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
    p.text("Examples", x, top(ey + 8.0, 13.0), 13.0, Font::Medium, INK_SOFT);
    ey += 26.0;
    let mut hovered: Option<(&str, &str)> = None;
    for (i, ex) in EXAMPLES.iter().enumerate() {
        if ey + 30.0 > y + h {
            break;
        }
        let r = [x, ey, list_w, 30.0];
        let over = inside(r, st.s.ui.mouse);
        let hv = hover(&mut st.s.ui, id("ex", i), over, dt);
        if inside(r, st.s.ui.mouse) {
            hovered = Some((ex.title, ex.description));
        }
        p.rect(r, 10.0, fade(INK, 0.05 * hv));
        let installed = st.s.mods.iter().any(|m| m.id == ex.id);
        let title = p.fit(ex.title, list_w - 120.0, 13.0, Font::Regular);
        p.text(&title, x + 12.0, top(ey + 15.0, 13.0), 13.0, Font::Regular, INK_SOFT);
        if installed {
            let t = "Installed";
            let tw = p.measure(t, 12.0, Font::Regular);
            p.text(
                t,
                x + list_w - 12.0 - tw,
                top(ey + 15.0, 12.0),
                12.0,
                Font::Regular,
                MUTED,
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
    if dw < 120.0 {
        return;
    }
    if let Some((title, desc)) = hovered {
        p.text(title, dx, top(y + 14.0, 20.0), 20.0, Font::Medium, INK);
        for (j, l) in p.wrap(desc, dw, 14.0, Font::Regular).iter().enumerate() {
            p.text(l, dx, y + 36.0 + 22.0 * j as f32, 14.0, Font::Regular, INK_SOFT);
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
    let file = if m.folder.is_some() {
        format!("{}/main.lua", m.id)
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
    let commands: Vec<(String, String)> = m.commands.iter().map(|c| (c.name.clone(), c.help.clone())).collect();
    let failure = match &m.state {
        ModState::Failed(msg, trace) => Some((msg.clone(), trace.clone())),
        _ => None,
    };
    let mut dy = y;
    let name = p.fit(&name, dw, 24.0, Font::Medium);
    p.text(&name, dx, top(dy + 16.0, 24.0), 24.0, Font::Medium, INK);
    dy += 38.0;
    let meta = p.fit(&meta, dw, 12.0, Font::Regular);
    p.text(&meta, dx, top(dy + 8.0, 12.0), 12.0, Font::Regular, MUTED);
    dy += 26.0;
    let chip_w = p.measure(label, 12.0, Font::Medium) + 30.0;
    p.rect([dx, dy, chip_w, 24.0], 12.0, fade(c, 0.16));
    p.rect([dx + 10.0, dy + 9.0, 6.0, 6.0], 3.0, c);
    p.text(label, dx + 22.0, top(dy + 12.0, 12.0), 12.0, Font::Medium, c);
    let mut chips_w = chip_w;
    if library {
        let lw = p.measure("Library", 12.0, Font::Medium) + 20.0;
        p.rect([dx + chips_w + 8.0, dy, lw, 24.0], 12.0, fade(ACCENT, 0.16));
        p.text(
            "Library",
            dx + chips_w + 18.0,
            top(dy + 12.0, 12.0),
            12.0,
            Font::Medium,
            ACCENT,
        );
        chips_w += lw + 8.0;
    }
    if running {
        let stats = p.fit(&stats, dw - chips_w - 12.0, 11.0, Font::Mono);
        p.text(
            &stats,
            dx + chips_w + 12.0,
            top(dy + 12.0, 11.0),
            11.0,
            Font::Mono,
            MUTED,
        );
    }
    dy += 38.0;
    for l in p.wrap(&description, dw, 14.0, Font::Regular).iter().take(4) {
        p.text(l, dx, dy, 14.0, Font::Regular, INK_SOFT);
        dy += 22.0;
    }
    for (what, ids) in [("Uses", &uses), ("Used by", &used_by)] {
        if !ids.is_empty() {
            let lw = p.text(what, dx, dy, 12.0, Font::Medium, MUTED);
            let ids = p.fit(ids, dw - lw - 8.0, 12.0, Font::Mono);
            p.text(&ids, dx + lw + 8.0, dy, 12.0, Font::Mono, INK_SOFT);
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
    pill(
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
    );
    dy += 50.0;
    if !commands.is_empty() {
        p.text("Commands", dx, top(dy + 8.0, 13.0), 13.0, Font::Medium, INK_SOFT);
        dy += 24.0;
        for (name, help) in commands.iter().take(6) {
            let cmd = format!("/{name}");
            let cw = p.text(&cmd, dx, dy, 13.0, Font::Mono, INK);
            let help = p.fit(help, dw - cw - 12.0, 13.0, Font::Regular);
            p.text(&help, dx + cw + 12.0, dy, 13.0, Font::Regular, MUTED);
            dy += 22.0;
        }
        dy += 10.0;
    }
    if let Some((msg, trace)) = failure {
        let lines = p.wrap(&msg, dw - 28.0, 12.0, Font::Mono);
        let trace = p.wrap(&trace, dw - 28.0, 11.0, Font::Mono);
        let room = ((y + h - dy - 24.0) / 18.0).max(1.0) as usize;
        let total = (lines.len() + trace.len()).min(room);
        let box_h = 20.0 + 18.0 * total as f32;
        p.rect([dx, dy, dw, box_h], 12.0, fade(DANGER, 0.10));
        let mut ty = dy + 10.0;
        for l in lines.iter().chain(trace.iter()).take(total).enumerate() {
            let c = if l.0 < lines.len() { DANGER } else { MUTED };
            p.text(l.1, dx + 14.0, ty, 12.0, Font::Mono, c);
            ty += 18.0;
        }
    } else if dy + 70.0 < y + h {
        drop_hint(p, dx, y + h - 64.0, dw);
    }
}

fn drop_hint(p: &mut P, x: f32, y: f32, w: f32) {
    let r = [x, y, w, 56.0];
    p.border(r, 14.0, RULE);
    let t = "Drop a .lua file, a mod folder or a .zip on the game window to install it.";
    let t = p.fit(t, w - 32.0, 13.0, Font::Regular);
    p.text(&t, x + 16.0, top(y + 28.0, 13.0), 13.0, Font::Regular, MUTED);
}

enum Doc {
    Heading(String),
    Para(String),
    Api(String, String),
}

fn docs() -> Vec<Doc> {
    let mut out = vec![
        Doc::Heading("Script mods".into()),
        Doc::Para("A script mod is a Lua file in horizon-mods/.horizon-mod-data/horizon_studio/mods/: name.lua, or a folder with main.lua and other files to require. The file runs when the mod loads; what it registers with on(), command() and every() then runs in the game. Save it, in the Editor or any other editor, and it reloads at once. A mod that fails stops alone: the Library shows the error and its line.".into()),
        Doc::Para("Drop a .lua file, a mod folder or a .zip on the game window to install it. Lines -- name:, -- version:, -- author: and -- description: at the top name it in the Library.".into()),
        Doc::Heading("Libraries".into()),
        Doc::Para("A mod can be a library for others: it calls export(table), or its main file returns a table, and other mods take it with local kit = import(\"uikit\"). Libraries load first; when one reloads, the mods that use it reload too. A library's functions run as the mod that calls them, so what they draw, register or store belongs to that mod. Mods talk to each other with emit(\"name:event\", ...) and on(\"name:event\", fn), and json.encode and json.decode turn tables into text and back.".into()),
        Doc::Para("The bundled uikit is such a library, in plain Lua on gfx.* and canvas screens: open it in the Editor to see how it is made, or to make your own.".into()),
        Doc::Heading("ViaHorizon".into()),
        Doc::Para("Joins go through ViaProxy, which runs beside the game and translates Horizon's 26.3 to the server's version, as ViaFabricPlus does: Classic to 26.3, and Bedrock (experimental). /via chooses the version (or Auto detect), and so does Options > Mods > Horizon Studio from the title screen. Online-mode servers sign in with your own account. via.* gives scripts the same controls; /vialog prints ViaProxy's last lines.".into()),
        Doc::Heading("Console".into()),
        Doc::Para("Type Lua and press Enter: it runs at once in the game, and an expression's value is shown. /lua <code> does the same from chat. What the console registers (on, command...) lasts until clear().".into()),
        Doc::Heading("The language".into()),
        Doc::Para("Lua 5.3 without coroutines, goto and bitwise operators. Numbers are 64-bit floats. string (with Lua patterns, format, split, trim), math, table and os.clock, os.time and os.date are there; io and the rest of os are not. Each call into a script has a step budget, so an endless loop stops the script, not the game.".into()),
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
    out.push(Doc::Heading("AI agents".into()));
    out.push(Doc::Para("horizon-studio-mcp lets AI agents (Claude Code, omp, Codex, Cursor...) make mods here: they write and test them, headless and in this game, run Lua in the console and read what happened. Copy AI setup prompt, at the top right, copies a prompt for your agent that sets it all up; once one is connected, its name shows there. Options > Mods > Horizon Studio turns the AI link off.".into()));
    out.push(Doc::Heading("Commands in chat".into()));
    out.push(Doc::Para(
        "/studio opens this screen, /studio reload reloads every script mod, /lua <code> runs code.".into(),
    ));
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
    let width = w.min(820.0);
    // Laid out as lines of (text, size, font, colour, height).
    let mut lines: Vec<(String, f32, Font, u32, f32)> = Vec::new();
    for d in docs() {
        match d {
            Doc::Heading(t) => {
                lines.push((String::new(), 0.0, Font::Regular, 0, 10.0));
                lines.push((t, 18.0, Font::Medium, INK, 32.0));
            }
            Doc::Para(t) => {
                for l in p.wrap(&t, width, 14.0, Font::Regular) {
                    lines.push((l, 14.0, Font::Regular, INK_SOFT, 22.0));
                }
                lines.push((String::new(), 0.0, Font::Regular, 0, 8.0));
            }
            Doc::Api(sig, desc) => {
                for l in p.wrap(&sig, width, 13.0, Font::Mono) {
                    lines.push((l, 13.0, Font::Mono, INK, 20.0));
                }
                for l in p.wrap(&desc, width, 13.0, Font::Regular) {
                    lines.push((l, 13.0, Font::Regular, MUTED, 20.0));
                }
                lines.push((String::new(), 0.0, Font::Regular, 0, 10.0));
            }
        }
    }
    let total: f32 = lines.iter().map(|l| l.4).sum();
    let step = 22.0;
    let max = ((total - h) / step).max(0.0).ceil() as usize;
    st.s.ui.page = (h / step) as usize;
    st.s.ui.docs_scroll = st.s.ui.docs_scroll.min(max);
    smooth(&mut st.s.ui.smooth[2], st.s.ui.docs_scroll as f32, dt);
    let mut ly = y - st.s.ui.smooth[2] * step;
    for (t, size, font, c, lh) in lines {
        if ly >= y - 1.0 && ly + lh <= y + h + 1.0 && !t.is_empty() {
            p.text(&t, x, ly + (lh - (size * 1.45).round()) / 2.0, size, font, c);
        }
        ly += lh;
    }
}

fn naming(st: &mut Studio, p: &mut P, name: &LineEdit, w: f32, h: f32) {
    let now = st.s.now;
    let arrive = progress(now, st.s.ui.naming_at, MOVE);
    let (a, dy) = (p.alpha, p.dy);
    p.alpha = a * arrive;
    p.dy = 0.0;
    p.rect([0.0, 0.0, w, h], 0.0, fade(PAGE, 0.55));
    p.dy = (1.0 - arrive) * RISE;
    let cw = 380f32.min(w - 32.0);
    let r = [((w - cw) / 2.0).floor(), (h / 2.0 - 84.0).floor(), cw, 168.0];
    p.rect(r, 16.0, SURFACE);
    p.border(r, 16.0, RULE);
    p.text("New mod", r[0] + 22.0, r[1] + 18.0, 16.0, Font::Medium, INK);
    p.text(
        "What is it called?",
        r[0] + 22.0,
        r[1] + 44.0,
        13.0,
        Font::Regular,
        MUTED,
    );
    let f = [r[0] + 20.0, r[1] + 74.0, cw - 40.0, 40.0];
    p.rect(f, 10.0, PAGE);
    p.border(f, 10.0, fade(ACCENT, 0.6));
    line_edit(
        p,
        now,
        name,
        f[0] + 14.0,
        top(f[1] + 20.0, 14.0),
        f[2] - 28.0,
        14.0,
        Font::Regular,
        INK,
    );
    p.text(
        "Enter to create  ·  Esc to cancel",
        r[0] + 22.0,
        r[1] + 130.0,
        12.0,
        Font::Regular,
        MUTED,
    );
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
    let mut p = P {
        g: game,
        alpha: arrive * leave,
        dy: (1.0 - arrive) * -RISE,
    };
    let tw = p.measure(&text, 13.0, Font::Regular);
    let r = [((w - tw) / 2.0 - 28.0).floor(), 18.0, tw + 46.0, 36.0];
    p.rect(r, 18.0, fade(SURFACE, 0.96));
    p.border(r, 18.0, RULE);
    p.rect([r[0] + 16.0, r[1] + 15.0, 6.0, 6.0], 3.0, ACCENT);
    p.text(&text, r[0] + 30.0, top(r[1] + 18.0, 13.0), 13.0, Font::Regular, INK);
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
        };
        if !matches!(field(&table, "backdrop"), super::hstudio_lua::Value::Bool(false)) {
            p.rect([0.0, 0.0, w, h], 0.0, fade(PAGE, 0.6));
        }
        p.dy = rise;
        if !canvas && (title.is_some() || !ws.is_empty()) {
            let pw = (field_num(&table, "width", 340.0) as f32).clamp(200.0, (w - 32.0).max(200.0));
            let inner = pw - 48.0;
            let heights: Vec<f32> = ws.iter().map(|wt| row_height(&mut p, wt, inner)).collect();
            let head = if title.is_some() { 54.0 } else { 18.0 };
            let ph = head + heights.iter().sum::<f32>() + 18.0;
            let (px, py) = (((w - pw) / 2.0).floor(), ((h - ph) / 2.0).floor().max(16.0));
            p.rect([px, py, pw, ph], 18.0, SURFACE);
            p.border([px, py, pw, ph], 18.0, RULE);
            if let Some(t) = &title {
                let t = p.fit(t, inner, 18.0, Font::Medium);
                p.text(&t, px + 24.0, top(py + 30.0, 18.0), 18.0, Font::Medium, INK);
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
                            (mix(ACCENT, ACCENT_HOVER, t), PAGE)
                        } else {
                            (mix(fade(INK, 0.08), fade(INK, 0.15), t), INK)
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
                            p.rect([r[0] - 10.0, r[1], r[2] + 20.0, r[3]], 10.0, fade(INK, 0.05 * hv));
                        }
                        let cy = r[1] + r[3] / 2.0;
                        let l = p.fit(&label, r[2] - 60.0, 14.0, Font::Regular);
                        p.text(&l, r[0], top(cy, 14.0), 14.0, Font::Regular, INK);
                        let track = [r[0] + r[2] - 40.0, cy - 11.0, 40.0, 22.0];
                        p.rect(track, 11.0, mix(PLATE, ACCENT, t));
                        let kx = track[0] + 3.0 + (track[2] - 22.0) * t;
                        p.rect([kx, cy - 8.0, 16.0, 16.0], 8.0, mix(INK_SOFT, INK, t));
                    }
                    "slider" => {
                        let (min, max) = (field_num(wt, "min", 0.0), field_num(wt, "max", 1.0));
                        let step = field_num(wt, "step", 0.0);
                        let v = field_num(wt, "value", min).clamp(min.min(max), max.max(min));
                        let vs = slider_text(v, step);
                        let vw = p.measure(&vs, 12.0, Font::Mono);
                        let l = p.fit(&label, r[2] - vw - 12.0, 13.0, Font::Regular);
                        p.text(&l, r[0], r[1] + 2.0, 13.0, Font::Regular, INK);
                        p.text(&vs, r[0] + r[2] - vw, r[1] + 3.0, 12.0, Font::Mono, INK_SOFT);
                        let ty = r[1] + 32.0;
                        let frac = if max > min {
                            ((v - min) / (max - min)) as f32
                        } else {
                            0.0
                        };
                        p.rect([r[0], ty - 2.0, r[2], 4.0], 2.0, PLATE);
                        p.rect([r[0], ty - 2.0, r[2] * frac, 4.0], 2.0, ACCENT);
                        let hv = hover(&mut st.s.ui, id("sknob", i), over, dt);
                        let kx = r[0] + r[2] * frac;
                        let kr = 7.0 + 2.0 * hv;
                        if hv > 0.0 {
                            p.rect(
                                [kx - kr - 4.0, ty - kr - 4.0, 2.0 * kr + 8.0, 2.0 * kr + 8.0],
                                kr + 4.0,
                                fade(ACCENT, 0.22 * hv),
                            );
                        }
                        p.rect([kx - kr, ty - kr, 2.0 * kr, 2.0 * kr], kr, INK);
                    }
                    "input" => {
                        let l = p.fit(&label, r[2], 12.0, Font::Regular);
                        p.text(&l, r[0], r[1], 12.0, Font::Regular, MUTED);
                        let f = [r[0], r[1] + 20.0, r[2], 36.0];
                        p.rect(f, 10.0, PAGE);
                        let focused = focus == Some(i);
                        let ring = glide(&mut st.s.ui, id("sfocus", i), focused, dt);
                        p.border(f, 10.0, mix(RULE, ACCENT, ring));
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
                                INK,
                            );
                        } else {
                            let value = field_text(wt, "value").unwrap_or_default();
                            let (shown, c) = if value.is_empty() {
                                (field_text(wt, "placeholder").unwrap_or_default(), MUTED)
                            } else {
                                (value, INK)
                            };
                            let shown = p.fit(&shown, f[2] - 24.0, 14.0, Font::Regular);
                            p.text(&shown, f[0] + 12.0, ty, 14.0, Font::Regular, c);
                        }
                    }
                    "space" => {}
                    _ => {
                        let c = match field(wt, "color") {
                            super::hstudio_lua::Value::Nil => INK_SOFT,
                            v => {
                                let lua = super::hstudio_lua::Lua::new();
                                super::hstudio::color_arg(&lua, &[v], 0, INK_SOFT).unwrap_or(INK_SOFT) | 0xFF00_0000
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
