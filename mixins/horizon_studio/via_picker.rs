// ViaHorizon's version picker, over the multiplayer screen: a card in the
// studio's style with ViaHorizon's switch, its state, a search field and the
// versions to join servers as. Laid out and drawn in dp through `Paint`, so
// the game draws it with Horizon's design system and the tests draw it on
// anything; the game feeds it the pointer, clicks, the wheel and keys.

/// What the picker draws with (colours `0xAARRGGBB`; `y` is the top of a
/// line 1.45 x `size` tall, as Horizon's toolkit lays text out).
pub trait Paint {
    fn rect(&mut self, r: [f32; 4], radius: f32, argb: u32);
    fn border(&mut self, r: [f32; 4], radius: f32, argb: u32);
    fn text(&mut self, s: &str, x: f32, y: f32, size: f32, medium: bool, argb: u32) -> f32;
    fn measure(&mut self, s: &str, size: f32, medium: bool) -> f32;
}

/// ViaHorizon as the picker shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub enabled: bool,
    /// The chosen version's id ("1.8.x", "auto").
    pub version: String,
    pub status: String,
    pub tone: Tone,
    /// (id, label), newest first.
    pub versions: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Off,
    Busy,
    Ready,
    Failed,
}

/// What a click or a key asks of ViaHorizon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    None,
    /// Turn ViaHorizon on or off.
    Toggle,
    /// Join as this version from now on.
    Pick(String),
}

/// The keys the picker takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Enter,
    Up,
    Down,
    Backspace,
    Text(String),
}

const PAGE: u32 = 0xFF0D_0E13;
const SURFACE: u32 = 0xFF16_181F;
const RAISED: u32 = 0xFF2B_2E3A;
const RULE: u32 = 0xFF2A_2C36;
const INK: u32 = 0xFFEC_EDF3;
const INK_SOFT: u32 = 0xFFC5_C7D1;
const MUTED: u32 = 0xFF8B_8F9C;
const ACCENT: u32 = 0xFF9A_86FF;
const SUCCESS: u32 = 0xFF45_C27A;
const AMBER: u32 = 0xFFF2_C48D;
const DANGER: u32 = 0xFFF0_7A68;

const ROW: f32 = 36.0;
const OPEN_SECS: f64 = 0.24;
const CLOSE_SECS: f64 = 0.15;

fn fade(c: u32, a: f32) -> u32 {
    let alpha = ((c >> 24) as f32 * a.clamp(0.0, 1.0)).round() as u32;
    (alpha << 24) | (c & 0xFF_FFFF)
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn approach(v: f32, target: f32, dt: f32, speed: f32) -> f32 {
    target + (v - target) * (-dt * speed).exp()
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

/// The top of a line of `size` text centred on `cy`.
fn top(cy: f32, size: f32) -> f32 {
    (cy - size * 1.45 / 2.0).round()
}

/// A few words on a version beside its name.
pub fn tag(id: &str) -> &'static str {
    match id {
        "auto" => "asks the server",
        "26.3" => "Horizon's own",
        v if v.starts_with("Bedrock") => "experimental",
        v if v.starts_with('c') => "Classic",
        v if v.starts_with('a') => "Alpha",
        v if v.starts_with('b') => "Beta",
        _ => "",
    }
}

/// The versions whose name holds `filter` (any case), as indices.
pub fn filtered(versions: &[(String, String)], filter: &str) -> Vec<usize> {
    let f = filter.trim().to_lowercase();
    versions
        .iter()
        .enumerate()
        .filter(|(_, (id, label))| {
            f.is_empty()
                || label.to_lowercase().contains(&f)
                || id.to_lowercase().contains(&f)
                || tag(id).to_lowercase().contains(&f)
        })
        .map(|(i, _)| i)
        .collect()
}

#[derive(Clone, Debug, Default)]
struct Layout {
    card: [f32; 4],
    close: [f32; 4],
    toggle: [f32; 4],
    list: [f32; 4],
    /// Each visible row and its index in `filtered`.
    rows: Vec<([f32; 4], usize)>,
}

#[derive(Default)]
pub struct Picker {
    open: bool,
    opened_at: f64,
    closing_at: Option<f64>,
    pub filter: String,
    /// The first row shown, and the row the keys are on (in the filtered list).
    scroll: usize,
    focus: usize,
    /// The pointer, in dp.
    pub mouse: (f32, f32),
    layout: Layout,
    last: Option<f64>,
    switch: Option<f32>,
    sel_y: Option<f32>,
    hover: Vec<(usize, f32)>,
    close_hover: f32,
    /// The list moved by keys last (not by the pointer).
    keyed: bool,
}

impl Picker {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open or closing (its exit still drawing).
    pub fn visible(&self, now: f64) -> bool {
        self.open || self.closing_at.is_some_and(|t| now - t < CLOSE_SECS)
    }

    /// Opens on the chosen version.
    pub fn show(&mut self, now: f64, info: &Info) {
        *self = Picker {
            open: true,
            opened_at: now,
            mouse: self.mouse,
            ..Picker::default()
        };
        let list = filtered(&info.versions, "");
        if let Some(i) = list.iter().position(|&i| info.versions[i].0 == info.version) {
            self.focus = i;
            self.scroll = i.saturating_sub(2);
        }
    }

    pub fn hide(&mut self, now: f64) {
        if self.open {
            self.open = false;
            self.closing_at = Some(now);
        }
    }

    pub fn wheel(&mut self, steps: f32, info: &Info) {
        let n = filtered(&info.versions, &self.filter).len();
        let rows = self.visible_rows();
        let most = n.saturating_sub(rows);
        let by = steps.round() as isize;
        let by = if by == 0 { steps.signum() as isize } else { by };
        self.scroll = (self.scroll as isize - by).clamp(0, most as isize) as usize;
    }

    fn visible_rows(&self) -> usize {
        ((self.layout.list[3] / ROW).floor() as usize).max(1)
    }

    /// A press of the left button at the pointer.
    pub fn click(&mut self, now: f64, info: &Info) -> Act {
        if !self.open {
            return Act::None;
        }
        let at = self.mouse;
        if !inside(self.layout.card, at) || inside(self.layout.close, at) {
            self.hide(now);
            return Act::None;
        }
        if inside(self.layout.toggle, at) {
            return Act::Toggle;
        }
        let list = filtered(&info.versions, &self.filter);
        if let Some(&(_, i)) = self.layout.rows.iter().find(|(r, _)| inside(*r, at))
            && let Some(&v) = list.get(i)
        {
            self.focus = i;
            self.hide(now);
            return Act::Pick(info.versions[v].0.clone());
        }
        Act::None
    }

    pub fn key(&mut self, now: f64, key: Key, info: &Info) -> Act {
        if !self.open {
            return Act::None;
        }
        let list = filtered(&info.versions, &self.filter);
        match key {
            Key::Escape => self.hide(now),
            Key::Enter => {
                if let Some(&v) = list.get(self.focus) {
                    self.hide(now);
                    return Act::Pick(info.versions[v].0.clone());
                }
            }
            Key::Up | Key::Down => {
                if !list.is_empty() {
                    self.focus = if key == Key::Up {
                        self.focus.saturating_sub(1)
                    } else {
                        (self.focus + 1).min(list.len() - 1)
                    };
                    let rows = self.visible_rows();
                    if self.focus < self.scroll {
                        self.scroll = self.focus;
                    } else if self.focus >= self.scroll + rows {
                        self.scroll = self.focus + 1 - rows;
                    }
                    self.keyed = true;
                }
            }
            Key::Backspace => {
                self.filter.pop();
                self.focus = 0;
                self.scroll = 0;
            }
            Key::Text(t) => {
                if self.filter.chars().count() < 32 {
                    self.filter.extend(t.chars().filter(|c| !c.is_control()));
                    self.focus = 0;
                    self.scroll = 0;
                }
            }
        }
        Act::None
    }

    /// The pointer moved (in dp): the row under it takes the focus.
    pub fn point(&mut self, at: (f32, f32)) {
        if (at.0 - self.mouse.0).abs() + (at.1 - self.mouse.1).abs() < 0.5 {
            return;
        }
        self.mouse = at;
        if let Some(&(_, i)) = self.layout.rows.iter().find(|(r, _)| inside(*r, at)) {
            self.focus = i;
            self.keyed = false;
        }
    }

    /// Whether the pointer is over something to click.
    pub fn pointing(&self) -> bool {
        self.open
            && (inside(self.layout.close, self.mouse)
                || inside(self.layout.toggle, self.mouse)
                || self.layout.rows.iter().any(|(r, _)| inside(*r, self.mouse)))
    }

    /// Draws it over a screen `w` x `h` dp.
    pub fn draw(&mut self, p: &mut dyn Paint, info: &Info, w: f32, h: f32, now: f64) {
        let dt = self.last.map_or(0.0, |l| (now - l).clamp(0.0, 0.1) as f32);
        self.last = Some(now);
        let a = if self.open {
            ease(((now - self.opened_at) / OPEN_SECS) as f32)
        } else {
            match self.closing_at {
                Some(t) => 1.0 - ((now - t) / CLOSE_SECS) as f32,
                None => 0.0,
            }
        };
        if a <= 0.0 {
            return;
        }
        let a = a.clamp(0.0, 1.0);
        p.rect([0.0, 0.0, w, h], 0.0, fade(PAGE, 0.72 * a));
        let cw = (w - 32.0).clamp(240.0, 460.0);
        let ch = (h - 48.0).clamp(320.0, 620.0);
        let rise = ((1.0 - a) * 8.0).round();
        let (cx, cy) = (((w - cw) / 2.0).floor(), ((h - ch) / 2.0).floor() + rise);
        let pad = 24.0;
        let inner = cw - pad * 2.0;
        // A soft shadow, then the card.
        for i in 1..=4 {
            let i = i as f32;
            p.rect(
                [cx - i * 2.0, cy + i * 3.0, cw + i * 4.0, ch + i * 2.0],
                18.0 + i * 2.0,
                fade(0xFF00_0000, 0.07 * a),
            );
        }
        p.rect([cx, cy, cw, ch], 18.0, fade(SURFACE, a));
        p.border([cx, cy, cw, ch], 18.0, fade(RULE, a));
        let mut layout = Layout {
            card: [cx, cy, cw, ch],
            ..Layout::default()
        };

        // Header and close.
        p.text("ViaHorizon", cx + pad, cy + 20.0, 20.0, true, fade(INK, a));
        p.text(
            "The version servers are joined as",
            cx + pad,
            cy + 50.0,
            13.0,
            false,
            fade(MUTED, a),
        );
        let close = [cx + cw - 18.0 - 28.0, cy + 16.0, 28.0, 28.0];
        let over_close = self.open && inside(close, self.mouse);
        self.close_hover = approach(self.close_hover, f32::from(u8::from(over_close)), dt, 20.0);
        if self.close_hover > 0.01 {
            p.rect(close, 14.0, fade(INK, 0.08 * self.close_hover * a));
        }
        let xw = p.measure("×", 18.0, false);
        p.text(
            "×",
            close[0] + (28.0 - xw) / 2.0,
            top(close[1] + 14.0, 18.0),
            18.0,
            false,
            fade(mix(MUTED, INK, self.close_hover), a),
        );
        layout.close = close;

        // The switch, then the state.
        let row = [cx + pad - 10.0, cy + 80.0, inner + 20.0, 40.0];
        let over = self.open && inside(row, self.mouse);
        if over {
            p.rect(row, 10.0, fade(INK, 0.05 * a));
        }
        p.text(
            "Join servers through ViaProxy",
            cx + pad,
            top(row[1] + 20.0, 14.0),
            14.0,
            false,
            fade(INK, a),
        );
        let on = f32::from(u8::from(info.enabled));
        let t = approach(self.switch.unwrap_or(on), on, dt, 16.0);
        self.switch = Some(t);
        let track = [cx + cw - pad - 40.0, row[1] + 9.0, 40.0, 22.0];
        p.rect(track, 11.0, fade(mix(RAISED, ACCENT, t), a));
        p.rect(
            [track[0] + 3.0 + 18.0 * t, track[1] + 3.0, 16.0, 16.0],
            8.0,
            fade(mix(INK_SOFT, INK, t), a),
        );
        layout.toggle = row;
        let dot = match info.tone {
            Tone::Ready => SUCCESS,
            Tone::Busy => AMBER,
            Tone::Failed => DANGER,
            Tone::Off => MUTED,
        };
        let sy = cy + 128.0;
        p.rect([cx + pad, sy + 5.0, 8.0, 8.0], 4.0, fade(dot, a));
        let status = fit(p, &info.status, inner - 16.0, 12.0, false);
        p.text(
            &status,
            cx + pad + 16.0,
            top(sy + 9.0, 12.0),
            12.0,
            false,
            fade(INK_SOFT, a),
        );

        // The search field: typing goes there.
        let field = [cx + pad, cy + 152.0, inner, 38.0];
        p.rect(field, 10.0, fade(PAGE, a));
        p.border(field, 10.0, fade(mix(RULE, ACCENT, 0.6), a));
        let ty = top(field[1] + 19.0, 14.0);
        if self.filter.is_empty() {
            p.text(
                "Search: 1.8, 1.12.2, beta, bedrock...",
                field[0] + 14.0,
                ty,
                14.0,
                false,
                fade(MUTED, a),
            );
        } else {
            let shown = fit_end(p, &self.filter, inner - 32.0, 14.0);
            let tw = p.text(&shown, field[0] + 14.0, ty, 14.0, false, fade(INK, a));
            if (now * 2.0) % 2.0 < 1.3 {
                p.rect([field[0] + 15.0 + tw, field[1] + 10.0, 1.5, 18.0], 0.0, fade(ACCENT, a));
            }
        }

        // The versions.
        let list_top = cy + 202.0;
        let list_bottom = cy + ch - 46.0;
        layout.list = [
            cx + pad - 6.0,
            list_top,
            inner + 12.0,
            (list_bottom - list_top).max(ROW),
        ];
        self.layout.list = layout.list;
        let rows = self.visible_rows();
        let list = filtered(&info.versions, &self.filter);
        self.focus = self.focus.min(list.len().saturating_sub(1));
        self.scroll = self.scroll.min(list.len().saturating_sub(rows));
        if list.is_empty() {
            let msg = fit(
                p,
                &format!("No version matches \"{}\"", self.filter),
                inner,
                14.0,
                false,
            );
            p.text(&msg, cx + pad, list_top + 8.0, 14.0, false, fade(MUTED, a));
        }
        let chosen = list.iter().position(|&v| info.versions[v].0 == info.version);
        let target = chosen
            .filter(|c| (self.scroll..self.scroll + rows).contains(c))
            .map(|c| list_top + (c - self.scroll) as f32 * ROW);
        self.sel_y = match (self.sel_y, target) {
            (Some(y), Some(t)) => Some(approach(y, t, dt, 22.0)),
            (_, t) => t,
        };
        if let Some(y) = self.sel_y {
            p.rect(
                [cx + pad - 6.0, y + 2.0, inner + 12.0, ROW - 4.0],
                10.0,
                fade(mix(SURFACE, ACCENT, 0.2), a),
            );
        }
        let mut hover = Vec::new();
        for (k, &v) in list.iter().enumerate().skip(self.scroll).take(rows) {
            let y = list_top + (k - self.scroll) as f32 * ROW;
            let r = [cx + pad - 6.0, y + 2.0, inner + 12.0, ROW - 4.0];
            let focused = self.open && k == self.focus && (self.keyed || inside(r, self.mouse));
            let was = self.hover.iter().find(|(i, _)| *i == k).map_or(0.0, |h| h.1);
            let hv = approach(was, f32::from(u8::from(focused)), dt, 20.0);
            if hv > 0.01 {
                hover.push((k, hv));
                if Some(k) != chosen {
                    p.rect(r, 10.0, fade(INK, 0.06 * hv * a));
                }
            }
            let (id, label) = &info.versions[v];
            let is_chosen = Some(k) == chosen;
            let c = if is_chosen { 0xFFB0_A0FF } else { INK };
            let label = fit(p, label, inner - 130.0, 14.0, is_chosen);
            p.text(
                &label,
                cx + pad + 8.0,
                top(y + ROW / 2.0, 14.0),
                14.0,
                is_chosen,
                fade(c, a),
            );
            let tag = tag(id);
            if !tag.is_empty() {
                let tw = p.measure(tag, 12.0, false);
                p.text(
                    tag,
                    cx + pad + inner - 6.0 - tw,
                    top(y + ROW / 2.0, 12.0),
                    12.0,
                    false,
                    fade(MUTED, a),
                );
            }
            layout.rows.push((r, k));
        }
        self.hover = hover;
        if list.len() > rows {
            let track = layout.list[3] - 8.0;
            let th = (track * rows as f32 / list.len() as f32).max(24.0);
            let most = (list.len() - rows) as f32;
            let ty = list_top + 4.0 + (track - th) * self.scroll as f32 / most;
            p.rect([cx + cw - pad + 10.0, ty, 3.0, th], 1.5, fade(MUTED, 0.5 * a));
        }

        // The footer.
        let hint = fit(
            p,
            "Esc closes. It applies the next time you join a server.",
            inner,
            12.0,
            false,
        );
        p.text(&hint, cx + pad, cy + ch - 34.0, 12.0, false, fade(MUTED, a));
        self.layout = layout;
    }
}

/// `s` shortened with "…" to fit `w`.
fn fit(p: &mut dyn Paint, s: &str, w: f32, size: f32, medium: bool) -> String {
    if p.measure(s, size, medium) <= w {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while !out.is_empty() {
        out.pop();
        let t = format!("{}…", out.trim_end());
        if p.measure(&t, size, medium) <= w {
            return t;
        }
    }
    "…".into()
}

/// The end of `s` that fits `w` (a search longer than its field).
fn fit_end(p: &mut dyn Paint, s: &str, w: f32, size: f32) -> String {
    let mut out: String = s.to_owned();
    while !out.is_empty() && p.measure(&out, size, false) > w {
        out.remove(0);
    }
    out
}
