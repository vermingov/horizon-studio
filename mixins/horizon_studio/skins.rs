// The executor styles: the studio laid out as the script executors of old
// Roblox days were, each in a Windows 10 window (look.rs draws the window):
// Synapse X's editor with its tabs and its script list, ProtoSmasher's
// editor over its output, ProtoSmasher's first look, and RC7's maroon with
// its tabs down the side and big letters. The shapes and colours are
// theirs; the names and the letters are the studio's. Their buttons do the
// studio's work: Execute runs the editor's code, Clear empties it, Execute
// File saves the open file of a mod and reloads the mod, Save File saves it
// (scratch code as a new script, named first), the script list is the
// Editor's (scripts, and modules that fold open to their files), Open File
// is the Library (where the styles with no script list open a module's
// other files, from its "..."), Attach and Inject reload every mod, Options
// is Settings. The other pages show in the editor's place, with the
// studio's tabs above, and a module that failed in another of its files
// says where by the editor, as the studio's own layout does.

use super::hstudio::{Font, Studio};
use super::hstudio_look::{
    P, chevron, draw_code, draw_log, fade, failed_chip, failed_chip_width, hover, id, inside, mix, on, page, pal,
    set_pal, state_dot, top,
};
use super::hstudio_theme::{Layout, Palette, Theme};
use super::hstudio_ui::{Action, ScriptRow, Spot, Tab, open_row, row_label, script_rows, scripts_first};

/// A flat button's look.
#[derive(Clone, Copy)]
struct Btn {
    bg: u32,
    hover: u32,
    fg: u32,
    edge: Option<u32>,
    size: f32,
    upper: bool,
}

fn button(st: &mut Studio, p: &mut P, b: Btn, label: &str, r: [f32; 4], action: Action, dt: f32) {
    let ui = &mut st.s.ui;
    let t = hover(ui, id(label, r[1] as usize), inside(r, ui.mouse), dt);
    p.rect(r, 4.0, mix(b.bg, b.hover, t));
    if let Some(e) = b.edge {
        p.border(r, 4.0, e);
    }
    let text = if b.upper {
        label.to_uppercase()
    } else {
        label.to_owned()
    };
    let text = p.fit(&text, r[2] - 8.0, b.size, Font::Medium);
    let tw = p.measure(&text, b.size, Font::Medium);
    p.text(
        &text,
        (r[0] + (r[2] - tw) / 2.0).floor(),
        top(r[1] + r[3] / 2.0, b.size),
        b.size,
        Font::Medium,
        b.fg,
    );
    ui.hits.push((r, action));
}

/// Buttons sharing `[x, y, w, h]` evenly.
fn row(st: &mut Studio, p: &mut P, b: Btn, items: &[(&str, Action)], [x, y, w, h]: [f32; 4], gap: f32, dt: f32) {
    let n = items.len() as f32;
    let bw = ((w - gap * (n - 1.0)) / n).floor();
    for (i, (label, action)) in items.iter().enumerate() {
        button(st, p, b, label, [x + (bw + gap) * i as f32, y, bw, h], *action, dt);
    }
}

/// Buttons as wide as their labels (`min` at least) from `x`; returns where they end.
#[allow(clippy::too_many_arguments)]
fn strip(st: &mut Studio, p: &mut P, b: Btn, items: &[(&str, Action)], x: f32, y: f32, h: f32, dt: f32) -> f32 {
    let mut bx = x;
    for (label, action) in items {
        let bw = (p.measure(label, b.size, Font::Medium) + 24.0).max(72.0);
        button(st, p, b, label, [bx, y, bw, h], *action, dt);
        bx += bw + 5.0;
    }
    bx - 5.0
}

fn strip_width(p: &mut P, b: Btn, items: &[(&str, Action)]) -> f32 {
    items
        .iter()
        .map(|(l, _)| (p.measure(l, b.size, Font::Medium) + 24.0).max(72.0) + 5.0)
        .sum::<f32>()
        - 5.0
}

/// Execute File: the open file of a mod saved and its mod reloaded (any
/// file of a module, not only main.lua), else the editor's code run.
fn execute_file() -> Action {
    Action::ExecuteFile
}

pub(super) fn draw(st: &mut Studio, p: &mut P, layout: Layout, body: [f32; 4], dt: f32) {
    match layout {
        Layout::Synapse => synapse(st, p, body, dt),
        Layout::ProtoSmasher => protosmasher(st, p, body, dt),
        Layout::ProtoClassic => protoclassic(st, p, body, dt),
        Layout::Rc7 | Layout::Studio => rc7(st, p, body, dt),
    }
}

const PAGES: [(&str, Tab); 5] = [
    ("Editor", Tab::Editor),
    ("Console", Tab::Console),
    ("Library", Tab::Library),
    ("Docs", Tab::Docs),
    ("Options", Tab::Settings),
];

/// Another tab's page in the editor's place, with the tabs above it.
fn tabbed_page(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], b: Btn, dt: f32) {
    let mut bx = x;
    for (label, tab) in PAGES {
        let bw = p.measure(label, b.size, Font::Medium) + 22.0;
        let look = if tab == st.s.ui.tab {
            Btn {
                bg: pal().accent,
                hover: pal().accent_hover,
                fg: on(pal().accent),
                ..b
            }
        } else {
            b
        };
        button(st, p, look, label, [bx, y, bw, 24.0], Action::Tab(tab), dt);
        bx += bw + 4.0;
    }
    sub_page(st, p, [x, y + 30.0, w, h - 30.0], dt);
}

/// A tab's page as the studio draws it, on the editor's colour.
fn sub_page(st: &mut Studio, p: &mut P, area: [f32; 4], dt: f32) {
    let c = pal();
    let sub = Palette {
        page: c.plate_soft,
        surface: c.plate_soft,
        plate_soft: mix(c.plate_soft, c.ink, 0.06),
        plate: mix(c.plate_soft, c.ink, 0.14),
        rule: mix(c.plate_soft, c.ink, 0.2),
        ..c
    };
    set_pal(sub);
    p.rect(area, 0.0, sub.page);
    let pad = 12.0;
    let tab = st.s.ui.tab;
    page(
        st,
        p,
        tab,
        [area[0] + pad, area[1] + pad, area[2] - 2.0 * pad, area[3] - 2.0 * pad],
        dt,
    );
    set_pal(c);
}

// ----------------------------------------------------------------- Synapse X

fn synapse(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let c = pal();
    let pad = 6.0;
    let btn = Btn {
        bg: c.plate,
        hover: c.rule,
        fg: c.ink,
        edge: None,
        size: 12.0,
        upper: false,
    };
    let bar_h = 28.0;
    let list_w = (w * 0.2).clamp(110.0, 210.0);
    let area = [x + pad, y + 4.0, w - 3.0 * pad - list_w, h - 4.0 - bar_h - 2.0 * pad];
    if st.s.ui.tab == Tab::Editor {
        // Its tab: the file open (a dot while unsaved), then + for a new mod.
        let title = st.s.editor.title.clone();
        let label = if st.s.editor.dirty {
            format!("{title}  •")
        } else {
            title
        };
        let tw = (p.measure(&label, 12.0, Font::Regular) + 26.0).min(area[2] - 40.0);
        p.rect([area[0], area[1], tw, 24.0], 3.0, c.plate_soft);
        st.s.ui
            .hits
            .push(([area[0], area[1], tw, 24.0], Action::Spot(Spot::File)));
        let label = p.fit(&label, tw - 20.0, 12.0, Font::Regular);
        p.text(
            &label,
            area[0] + 12.0,
            top(area[1] + 12.0, 12.0),
            12.0,
            Font::Regular,
            c.ink,
        );
        let plus = Btn {
            bg: 0,
            hover: c.plate,
            ..btn
        };
        button(
            st,
            p,
            plus,
            "+",
            [area[0] + tw + 4.0, area[1] + 1.0, 22.0, 22.0],
            Action::NewMod,
            dt,
        );
        // A module that failed in another of its files says where, beside its tab.
        failed_chip(st, p, area[0] + tw + 32.0, area[1], area[0] + area[2], dt);
        draw_code(st, p, [area[0], area[1] + 24.0, area[2], area[3] - 24.0], dt);
    } else {
        tabbed_page(st, p, area, btn, dt);
    }
    script_list(st, p, x + w - pad - list_w, area[1], list_w, area[3], dt);
    // The buttons along the bottom.
    let by = y + h - pad - bar_h;
    let left = [
        ("Execute", Action::Run),
        ("Clear", Action::ClearEditor),
        ("Open File", Action::Tab(Tab::Library)),
        ("Execute File", execute_file()),
        ("Save File", Action::SaveFile),
    ];
    let right = [
        ("Console", Action::Tab(Tab::Console)),
        ("Options", Action::Settings),
        ("Attach", Action::ReloadAll),
    ];
    let rw = strip_width(p, btn, &right);
    let room = w - 2.0 * pad - rw - 10.0;
    let mut shown = left.len();
    while shown > 1 && strip_width(p, btn, &left[..shown]) > room {
        shown -= 1;
    }
    strip(st, p, btn, &left[..shown], x + pad, by, bar_h, dt);
    strip(st, p, btn, &right, x + w - pad - rw, by, bar_h, dt);
}

/// The editor of the styles with no tab for its file: its code in `area`,
/// and over its top right, when a module failed in another of its files,
/// the chip that says where (a click opens it there).
fn code_with_chip(st: &mut Studio, p: &mut P, area: [f32; 4], dt: f32) {
    draw_code(st, p, area, dt);
    let ed = &st.s.editor;
    if ed.conflict || ed.removed {
        // The banner over the code says more just now.
        return;
    }
    if let Some(w) = failed_chip_width(st, p) {
        let (x, y) = (area[0] + area[2] - w - 16.0, area[1] + 8.0);
        if x > area[0] + 60.0 {
            p.rect([x, y, w, 24.0], 12.0, pal().plate_soft);
            failed_chip(st, p, x, y, area[0] + area[2], dt);
        }
    }
}

/// Synapse's script list at the right, its column `[x, y, w, h]`: the
/// Editor's rows (the scratch file, scripts, modules that fold open to
/// their files) in its own flat look. A click opens a file or folds a
/// module, "..." on the row under the pointer (or a right press) opens its
/// menu, and the wheel scrolls it.
fn script_list(st: &mut Studio, p: &mut P, x: f32, y: f32, w: f32, h: f32, dt: f32) {
    let c = pal();
    p.text("Scripts", x + 4.0, top(y + 12.0, 11.0), 11.0, Font::Medium, c.muted);
    let list = [x, y + 24.0, w, h - 24.0];
    p.rect(list, 2.0, c.plate_soft);
    // Its own menu, under its rows'.
    st.s.ui.hits.push(([x, y, w, h], Action::Spot(Spot::Scripts)));
    let row_h = 21.0;
    let rows = (((list[3] - 8.0) / row_h).floor() as usize).max(1);
    let all = script_rows(st);
    let open = open_row(st, &all);
    let first = scripts_first(st, all.len(), rows, open);
    st.s.ui.scripts_at = Some(list);
    for (k, row) in all.iter().enumerate().skip(first).take(rows) {
        let r = [
            list[0] + 3.0,
            list[1] + 4.0 + row_h * (k - first) as f32,
            list[2] - 6.0,
            row_h - 1.0,
        ];
        let label = row_label(st, row);
        let ui = &mut st.s.ui;
        let over = inside(r, ui.mouse);
        let hv = hover(ui, id(&label, k), over, dt);
        let bg = if open == Some(k) {
            fade(c.accent, 0.45)
        } else {
            fade(c.ink, 0.08 * hv)
        };
        p.rect(r, 2.0, bg);
        let cy = r[1] + r[3] / 2.0;
        let m = match row {
            ScriptRow::Scratch => None,
            ScriptRow::Script { i, .. } | ScriptRow::Module { i, .. } | ScriptRow::File { i, .. } => st.s.mods.get(*i),
        };
        let fg = if m.is_none_or(|m| m.enabled) {
            c.ink_soft
        } else {
            c.muted
        };
        let dot = m.map_or(c.muted, |m| state_dot(&m.state));
        let tx = match row {
            ScriptRow::Scratch | ScriptRow::Script { .. } => {
                p.rect([r[0] + 6.0, cy - 3.0, 6.0, 6.0], 3.0, dot);
                r[0] + 18.0
            }
            ScriptRow::Module { open: unfolded, .. } => {
                chevron(p, r[0] + 4.0, cy - 3.5, *unfolded, c.ink_soft);
                p.rect([r[0] + 15.0, cy - 3.0, 6.0, 6.0], 3.0, dot);
                r[0] + 27.0
            }
            ScriptRow::File { .. } => r[0] + 30.0,
        };
        let menu = over && !matches!(row, ScriptRow::Scratch);
        let room = r[2] - (tx - r[0]) - if menu { 24.0 } else { 6.0 };
        let name = p.fit(&label, room.max(12.0), 12.0, Font::Regular);
        p.text(&name, tx, top(cy, 12.0), 12.0, Font::Regular, fg);
        st.s.ui.hits.push((r, Action::ScriptRow(k.min(65535) as u16)));
        st.s.ui.hits.push((r, Action::Spot(Spot::Row(k.min(65535) as u16))));
        if menu {
            let b = [r[0] + r[2] - 20.0, r[1] + 1.0, 18.0, r[3] - 2.0];
            let ui = &mut st.s.ui;
            let bh = hover(ui, id("synapse-row-menu", k), inside(b, ui.mouse), dt);
            p.rect(b, 2.0, fade(c.ink, 0.12 * bh));
            let gw = p.measure("...", 12.0, Font::Medium);
            p.text(
                "...",
                b[0] + (b[2] - gw) / 2.0,
                top(cy, 12.0),
                12.0,
                Font::Medium,
                c.ink_soft,
            );
            ui.hits.push((b, Action::ScriptMenu(k.min(65535) as u16)));
        }
    }
    st.s.ui.script_rows = all;
}

// ----------------------------------------------------------------- ProtoSmasher

fn protosmasher(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let c = pal();
    let pad = 8.0;
    let btn = Btn {
        bg: c.plate,
        hover: c.rule,
        fg: c.ink,
        edge: None,
        size: 11.0,
        upper: true,
    };
    let (row_h, foot_h) = (28.0, 26.0);
    let out_h = (h * 0.24).max(56.0);
    let area = [x + pad, y + pad, w - 2.0 * pad, h - 5.0 * pad - row_h - out_h - foot_h];
    if st.s.ui.tab == Tab::Editor {
        code_with_chip(st, p, area, dt);
    } else {
        tabbed_page(st, p, area, btn, dt);
    }
    let ry = area[1] + area[3] + pad;
    row(
        st,
        p,
        btn,
        &[
            ("Execute", Action::Run),
            ("Open File", Action::Tab(Tab::Library)),
            ("Clear", Action::ClearEditor),
            ("Inject", Action::ReloadAll),
        ],
        [x + pad, ry, w - 2.0 * pad, row_h],
        6.0,
        dt,
    );
    let out = [x + pad, ry + row_h + pad, w - 2.0 * pad, out_h];
    p.rect(out, 0.0, c.plate_soft);
    draw_log(
        st,
        p,
        [out[0] + 8.0, out[1] + 6.0, out[2] - 16.0, out[3] - 12.0],
        false,
        dt,
    );
    let fy = out[1] + out_h + pad;
    let quarter = ((w - 2.0 * pad - 12.0) / 4.0).floor();
    button(
        st,
        p,
        btn,
        "Settings",
        [x + pad, fy, quarter, foot_h],
        Action::Settings,
        dt,
    );
    button(
        st,
        p,
        btn,
        "New Tab",
        [x + pad + quarter + 6.0, fy, quarter, foot_h],
        Action::NewMod,
        dt,
    );
    // The theme, as its dropdown was: a click goes to the next one.
    let name = st.s.ui.look.theme().name.trim_end_matches(" style").to_owned();
    let dx = x + pad + 2.0 * (quarter + 6.0);
    let r = [dx, fy, x + w - pad - dx, foot_h];
    button(st, p, Btn { upper: false, ..btn }, &name, r, Action::NextTheme, dt);
    let (ax, ay) = (r[0] + r[2] - 16.0, r[1] + r[3] / 2.0 - 2.0);
    for i in 0..4 {
        let f = i as f32;
        p.rect([ax + f, ay + f, 7.0 - 2.0 * f, 1.0], 0.0, c.ink_soft);
    }
}

// ----------------------------------------------------------------- ProtoSmasher's first look

fn protoclassic(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let c = pal();
    let pad = 6.0;
    // Windows' own buttons.
    let btn = Btn {
        bg: c.plate,
        hover: 0xFFE5_F1FB,
        fg: 0xFF00_0000,
        edge: Some(c.rule),
        size: 12.0,
        upper: false,
    };
    let (row_h, foot_h) = (26.0, 22.0);
    let out_h = (h * 0.22).max(52.0);
    let area = [x + pad, y + pad, w - 2.0 * pad, h - 5.0 * pad - row_h - out_h - foot_h];
    if st.s.ui.tab == Tab::Editor {
        code_with_chip(st, p, area, dt);
    } else {
        tabbed_page(st, p, area, btn, dt);
    }
    let ry = area[1] + area[3] + pad;
    row(
        st,
        p,
        btn,
        &[
            ("Execute", Action::Run),
            ("Open", Action::Tab(Tab::Library)),
            ("Clear", Action::ClearEditor),
            ("Inject", Action::ReloadAll),
        ],
        [x + pad, ry, w - 2.0 * pad, row_h],
        5.0,
        dt,
    );
    let out = [x + pad, ry + row_h + pad, w - 2.0 * pad, out_h];
    p.rect(out, 0.0, 0xFF3C_3C3C);
    draw_log(
        st,
        p,
        [out[0] + 6.0, out[1] + 4.0, out[2] - 12.0, out[3] - 8.0],
        false,
        dt,
    );
    let fy = out[1] + out_h + pad;
    let running = st.s.mods.iter().filter(|m| m.running()).count();
    let status = format!("{running} of {} mods running", st.s.mods.len());
    p.text(
        &status,
        x + pad + 2.0,
        top(fy + foot_h / 2.0, 11.0),
        11.0,
        Font::Regular,
        c.muted,
    );
    let small = Btn { size: 11.0, ..btn };
    let items = [("Console", Action::Tab(Tab::Console)), ("Options", Action::Settings)];
    let sw = strip_width(p, small, &items);
    strip(st, p, small, &items, x + w - pad - sw, fy, foot_h, dt);
}

// ----------------------------------------------------------------- RC7

fn rc7(st: &mut Studio, p: &mut P, [x, y, w, h]: [f32; 4], dt: f32) {
    let c = pal();
    let pad = 8.0;
    let (side_w, letters_w, bottom_h) = (92.0, 64.0, 30.0);
    let tab_btn = Btn {
        bg: 0xFFFF_FFFF,
        hover: 0xFFEE_E4E4,
        fg: 0xFF1A_1A1A,
        edge: None,
        size: 12.0,
        upper: false,
    };
    let chosen = Btn {
        bg: c.rule,
        hover: c.rule,
        fg: 0xFFFF_FFFF,
        ..tab_btn
    };
    for (i, (label, tab)) in PAGES.into_iter().enumerate() {
        let b = if st.s.ui.tab == tab { chosen } else { tab_btn };
        let r = [x + pad, y + pad + 34.0 * i as f32, side_w, 28.0];
        button(st, p, b, label, r, Action::Tab(tab), dt);
    }
    let area = [
        x + 2.0 * pad + side_w,
        y + pad,
        w - side_w - letters_w - 4.0 * pad,
        h - 3.0 * pad - bottom_h,
    ];
    if st.s.ui.tab == Tab::Editor {
        code_with_chip(st, p, area, dt);
    } else {
        sub_page(st, p, area, dt);
    }
    // Its big letters down the right, the studio's.
    let lx = area[0] + area[2] + pad;
    for (i, l) in ["H", "S"].into_iter().enumerate() {
        let size = 54.0;
        let lw = p.measure(l, size, Font::Medium);
        p.text(
            l,
            (lx + (letters_w - lw) / 2.0).floor(),
            y + pad + 62.0 * i as f32,
            size,
            Font::Medium,
            0xFFFF_FFFF,
        );
    }
    row(
        st,
        p,
        Btn {
            upper: true,
            size: 13.0,
            ..tab_btn
        },
        &[("Execute", Action::Run), ("Clear", Action::ClearEditor)],
        [area[0], y + h - pad - bottom_h, area[2], bottom_h],
        8.0,
        dt,
    );
}

// ----------------------------------------------------------------- pictures for Settings

/// A theme in small, for its card in Settings.
pub(super) fn preview(p: &mut P, t: &Theme, [x, y, w, h]: [f32; 4]) {
    let c = t.palette;
    if t.layout == Layout::Studio {
        p.rect([x, y, w, h], 8.0, c.page);
        p.rect([x + 8.0, y + 7.0, w * 0.42, 9.0], 4.5, c.plate_soft);
        p.rect([x + 9.0, y + 8.0, w * 0.13, 7.0], 3.5, c.ink);
        p.rect([x + w - 8.0 - w * 0.16, y + 7.0, w * 0.16, 9.0], 4.5, c.accent);
        let e = [x + 8.0, y + 22.0, w - 16.0, h - 30.0];
        p.rect(e, 6.0, c.plate_soft);
        code(p, &c, e);
        return;
    }
    // A small window: its title bar, then the style's shape.
    p.rect([x, y, w, h], 0.0, c.page);
    p.rect([x, y, w, 7.0], 0.0, c.surface);
    p.rect([x + w - 6.0, y + 2.0, 3.0, 3.0], 0.0, on(c.surface));
    let b = [x + 4.0, y + 11.0, w - 8.0, h - 15.0];
    match t.layout {
        Layout::Synapse => {
            let ew = (b[2] * 0.74).floor();
            let e = [b[0], b[1], ew, b[3] - 10.0];
            p.rect(e, 0.0, c.plate_soft);
            code(p, &c, e);
            p.rect([b[0] + ew + 4.0, b[1], b[2] - ew - 4.0, b[3] - 10.0], 0.0, c.plate_soft);
            for i in 0..4 {
                let bw = b[2] * 0.15;
                p.rect([b[0] + (bw + 3.0) * i as f32, b[1] + b[3] - 6.0, bw, 6.0], 0.0, c.plate);
            }
        }
        Layout::ProtoSmasher | Layout::ProtoClassic => {
            let eh = (b[3] * 0.5).floor();
            let e = [b[0], b[1], b[2], eh];
            p.rect(e, 0.0, c.plate_soft);
            code(p, &c, e);
            let bw = (b[2] - 9.0) / 4.0;
            for i in 0..4 {
                p.rect([b[0] + (bw + 3.0) * i as f32, b[1] + eh + 3.0, bw, 6.0], 0.0, c.plate);
            }
            let out = if t.layout == Layout::ProtoClassic {
                0xFF3C_3C3C
            } else {
                c.plate_soft
            };
            p.rect([b[0], b[1] + eh + 12.0, b[2], b[3] - eh - 12.0], 0.0, out);
        }
        Layout::Rc7 | Layout::Studio => {
            for i in 0..4 {
                p.rect([b[0], b[1] + 8.0 * i as f32, b[2] * 0.17, 6.0], 0.0, 0xFFFF_FFFF);
            }
            let (ex, ew) = (b[0] + b[2] * 0.21, b[2] * 0.6);
            let e = [ex, b[1], ew, b[3] - 10.0];
            p.rect(e, 0.0, c.plate_soft);
            code(p, &c, e);
            let lw = p.measure("H", 13.0, Font::Medium);
            let lx = ex + ew + (b[0] + b[2] - ex - ew - lw) / 2.0;
            p.text("H", lx, b[1] - 3.0, 13.0, Font::Medium, 0xFFFF_FFFF);
            p.text("S", lx, b[1] + 11.0, 13.0, Font::Medium, 0xFFFF_FFFF);
            p.rect([ex, b[1] + b[3] - 6.0, ew / 2.0 - 2.0, 6.0], 0.0, 0xFFFF_FFFF);
            p.rect(
                [ex + ew / 2.0 + 2.0, b[1] + b[3] - 6.0, ew / 2.0 - 2.0, 6.0],
                0.0,
                0xFFFF_FFFF,
            );
        }
    }
}

/// A few lines of code, as coloured bars.
fn code(p: &mut P, c: &Palette, e: [f32; 4]) {
    let lines: [&[(f32, u32)]; 4] = [
        &[(0.16, c.keyword), (0.3, c.ink_soft)],
        &[(0.08, 0), (0.22, c.global), (0.34, c.string)],
        &[(0.08, 0), (0.3, c.ink_soft), (0.12, c.number)],
        &[(0.2, c.comment)],
    ];
    for (i, line) in lines.iter().enumerate() {
        let ly = e[1] + 5.0 + 6.0 * i as f32;
        if ly + 3.0 > e[1] + e[3] - 3.0 {
            break;
        }
        let mut lx = e[0] + 5.0;
        for &(frac, col) in *line {
            let lw = ((e[2] - 10.0) * frac).floor();
            if col != 0 {
                p.rect([lx, ly, lw, 3.0], 1.5, col);
            }
            lx += lw + 3.0;
        }
    }
}
