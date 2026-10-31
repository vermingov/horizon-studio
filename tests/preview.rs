// Renders the studio's screens to PNG files, with Horizon's own fonts, as
// the game would draw them: a way to see the design without the game.
//
//     cargo test --test preview -- --ignored
//
// It writes target/preview/*.png. The fonts come from Horizon's source
// (tools/studio.py check puts it in target/horizon/<rev>/), or HORIZON_FONTS.

mod common;

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
use common::{FakeGame, run_for, studio};
use horizon_studio::game::hstudio::{Camera, Corner, Font, GameApi, PlayerInfo, Reply, Studio};
use horizon_studio::game::hstudio_ui::{Input, LineEdit, Tab, draw, draw_screen};
use std::path::{Path, PathBuf};

const DENSITY: f32 = 1.5;
const W: f32 = 1240.0;
const H: f32 = 720.0;

thread_local! {
    /// The screen drawn on, in dp: W x H unless a test sets another.
    static SIZE: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((W, H)) };
}

fn size() -> (f32, f32) {
    SIZE.with(std::cell::Cell::get)
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
    fonts: [FontRef<'static>; 3],
}

fn fonts_dir() -> PathBuf {
    if let Ok(d) = std::env::var("HORIZON_FONTS") {
        return PathBuf::from(d);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    for base in [root.join("stage"), root.join("horizon")] {
        if base.join("crates/horizon-theme/fonts").is_dir() {
            return base.join("crates/horizon-theme/fonts");
        }
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                let d = e.path().join("crates/horizon-theme/fonts");
                if d.is_dir() {
                    return d;
                }
            }
        }
    }
    panic!("no Horizon fonts: run python3 tools/studio.py check first, or set HORIZON_FONTS");
}

fn font(dir: &Path, name: &str) -> FontRef<'static> {
    let bytes: &'static [u8] = Box::leak(std::fs::read(dir.join(name)).unwrap().into_boxed_slice());
    FontRef::try_from_slice(bytes).unwrap()
}

thread_local! {
    static FONTS: std::cell::OnceCell<[FontRef<'static>; 3]> = const { std::cell::OnceCell::new() };
}

/// Geist, Geist Medium and Geist Mono, read once.
fn fonts() -> [FontRef<'static>; 3] {
    FONTS.with(|f| {
        f.get_or_init(|| {
            let dir = fonts_dir();
            [
                font(&dir, "Geist-Regular.ttf"),
                font(&dir, "Geist-Medium.ttf"),
                font(&dir, "GeistMono-Regular.ttf"),
            ]
        })
        .clone()
    })
}

impl Canvas {
    /// One that only measures, for frames drawn to let motion settle.
    fn blind() -> Canvas {
        Canvas {
            w: 0,
            h: 0,
            px: Vec::new(),
            fonts: fonts(),
        }
    }
    fn new() -> Canvas {
        let (sw, sh) = size();
        let (w, h) = ((sw * DENSITY) as usize, (sh * DENSITY) as usize);
        // A stand-in for the world behind the veil: sky over grass.
        let mut px = vec![[0.0; 3]; w * h];
        for y in 0..h {
            let t = y as f32 / h as f32;
            let c = if t < 0.62 {
                [0.45 + 0.2 * t, 0.62 + 0.2 * t, 0.92]
            } else {
                [0.30, 0.52, 0.24]
            };
            for x in 0..w {
                px[y * w + x] = c;
            }
        }
        Canvas {
            w,
            h,
            px,
            fonts: fonts(),
        }
    }
    fn blend(&mut self, x: usize, y: usize, argb: u32, cover: f32) {
        if x >= self.w || y >= self.h {
            return;
        }
        let a = ((argb >> 24) as f32 / 255.0) * cover.clamp(0.0, 1.0);
        let c = [
            ((argb >> 16) & 255) as f32 / 255.0,
            ((argb >> 8) & 255) as f32 / 255.0,
            (argb & 255) as f32 / 255.0,
        ];
        let p = &mut self.px[y * self.w + x];
        for i in 0..3 {
            p[i] = p[i] * (1.0 - a) + c[i] * a;
        }
    }
    /// A rounded rectangle's coverage at a pixel centre, by its distance field.
    fn rounded(&mut self, r: [f32; 4], radius: f32, argb: u32, ring: bool) {
        if self.w == 0 {
            return;
        }
        let d = DENSITY;
        let (x0, y0, x1, y1) = (
            (r[0] * d).round(),
            (r[1] * d).round(),
            ((r[0] + r[2]) * d).round(),
            ((r[1] + r[3]) * d).round(),
        );
        let rad = (radius * d)
            .round()
            .min(((x1 - x0) / 2.0).floor())
            .min(((y1 - y0) / 2.0).floor())
            .max(0.0);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (hx, hy) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
        for y in y0.max(0.0) as usize..y1.max(0.0) as usize {
            for x in x0.max(0.0) as usize..x1.max(0.0) as usize {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let qx = px.abs() - (hx - rad);
                let qy = py.abs() - (hy - rad);
                let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
                let mut cover = (0.5 - outside).clamp(0.0, 1.0);
                if ring {
                    cover *= (outside + 1.5).clamp(0.0, 1.0);
                }
                if cover > 0.0 {
                    self.blend(x, y, argb, cover);
                }
            }
        }
    }
    fn face(&self, f: Font) -> &FontRef<'static> {
        &self.fonts[match f {
            Font::Regular => 0,
            Font::Medium => 1,
            Font::Mono => 2,
        }]
    }
    fn scale(&self, f: Font, px: f32) -> PxScale {
        let font = self.face(f);
        PxScale::from(px * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0))
    }
    fn measure(&self, s: &str, size: f32, f: Font) -> f32 {
        let font = self.face(f);
        let scaled = font.as_scaled(self.scale(f, size * DENSITY));
        let mut w = 0.0;
        let mut prev = None;
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                w += scaled.kern(p, id);
            }
            w += scaled.h_advance(id);
            prev = Some(id);
        }
        w / DENSITY
    }
    fn text(&mut self, s: &str, at: [f32; 2], size: f32, f: Font, argb: u32) -> f32 {
        if self.w == 0 {
            return self.measure(s, size, f);
        }
        let px = size * DENSITY;
        let scale = self.scale(f, px);
        let font = self.face(f).clone();
        let scaled = font.as_scaled(scale);
        let line = (size * 1.45).round() * DENSITY;
        let (x0, y0) = ((at[0] * DENSITY).round(), (at[1] * DENSITY).round());
        let baseline = y0 + ((line - (scaled.ascent() - scaled.descent())) / 2.0 + scaled.ascent()).round();
        let mut pen = x0;
        let mut prev = None;
        let mut hits = Vec::new();
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(p) = prev {
                pen += scaled.kern(p, id);
            }
            let g = id.with_scale_and_position(scale, ab_glyph::point(pen, baseline));
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| {
                    hits.push((b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, cov.powf(0.85)))
                });
            }
            pen += scaled.h_advance(id);
            prev = Some(id);
        }
        for (x, y, cov) in hits {
            if x >= 0 && y >= 0 {
                self.blend(x as usize, y as usize, argb, cov);
            }
        }
        (pen - x0) / DENSITY
    }
    fn save(&self, path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        let data: Vec<u8> = self
            .px
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        writer.write_image_data(&data).unwrap();
    }
}

/// The fake game, with a canvas for the studio's design-system drawing.
struct Preview {
    game: FakeGame,
    canvas: Canvas,
}

impl GameApi for Preview {
    fn chat(&mut self, text: &str) {
        self.game.chat(text)
    }
    fn send(&mut self, line: &str) {
        self.game.send(line)
    }
    fn player(&mut self) -> Option<PlayerInfo> {
        self.game.player()
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        self.game.block(x, y, z)
    }
    fn clock(&self) -> Option<(i64, i64)> {
        self.game.clock()
    }
    fn ticks(&self) -> u64 {
        self.game.ticks()
    }
    fn fps(&self) -> [u32; 3] {
        self.game.fps()
    }
    fn mapping(&mut self, name: &str) -> Option<(bool, String)> {
        self.game.mapping(name)
    }
    fn screen(&self) -> String {
        self.game.screen()
    }
    fn agent(&mut self, line: &str) -> Result<Reply, String> {
        self.game.agent(line)
    }
    fn data_dir(&self) -> PathBuf {
        self.game.data_dir()
    }
    fn clipboard(&mut self) -> String {
        String::new()
    }
    fn set_clipboard(&mut self, _: &str) {}
    fn gui_size(&self) -> (f32, f32) {
        (size().0 / 2.0, size().1 / 2.0)
    }
    fn text_width(&self, text: &str) -> f32 {
        self.canvas.measure(text, 8.0, Font::Regular)
    }
    fn fill(&mut self, r: [f32; 4], argb: u32) {
        self.canvas
            .rounded([r[0] * 2.0, r[1] * 2.0, r[2] * 2.0, r[3] * 2.0], 0.0, argb, false);
    }
    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32, _: bool) {
        self.canvas
            .text(text, [at[0] * 2.0, at[1] * 2.0], 16.0, Font::Regular, 0xFF00_0000 | rgb);
    }
    fn place(&mut self, _: Corner, _: f32, _: f32) -> (f32, f32) {
        (4.0, 4.0)
    }
    fn open_folder(&mut self, _: &Path) {}
    fn camera(&self) -> Option<Camera> {
        None
    }
    fn world_quad(&mut self, _: [[f64; 3]; 4], _: u32) {}
    fn quad(&mut self, _: [[f32; 2]; 4], _: u32) {}
    fn sound(&mut self, _: &str, _: Option<[f64; 3]>, _: f32, _: f32) -> Result<(), String> {
        Ok(())
    }
    fn particle(&mut self, _: &str, _: [f64; 3], _: [f64; 3], _: Option<[f32; 4]>) -> Result<(), String> {
        Ok(())
    }
    fn dp(&mut self) -> (f32, f32, f32) {
        (size().0, size().1, 2.0)
    }
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.canvas.rounded(r, radius, argb, false);
    }
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.canvas.rounded(r, radius, argb, true);
    }
    fn dp_text(&mut self, text: &str, at: [f32; 2], size: f32, font: Font, argb: u32) -> f32 {
        self.canvas.text(text, at, size, font, argb)
    }
    fn dp_measure(&mut self, text: &str, size: f32, font: Font) -> f32 {
        self.canvas.measure(text, size, font)
    }
}

fn draw_settle(st: &mut Studio, p: &mut Preview, f: &impl Fn(&mut Studio, &mut Preview)) {
    f(st, p);
}

fn shot(st: &mut Studio, name: &str, f: impl Fn(&mut Studio, &mut Preview)) {
    let mut p = Preview {
        game: FakeGame::new(name),
        canvas: Canvas::new(),
    };
    p.game.dir = st.s.dir.clone();
    // Draw once (a tab change starts its transition), let every animation
    // finish, then draw what shows.
    st.s.now += 5.0;
    let mut first = Preview {
        game: FakeGame::new(&format!("{name}-warm")),
        canvas: Canvas::new(),
    };
    first.game.dir = st.s.dir.clone();
    draw_settle(st, &mut first, &f);
    std::mem::forget(first.game);
    st.s.now += 5.0;
    f(st, &mut p);
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/preview");
    std::fs::create_dir_all(&out).unwrap();
    p.canvas.save(&out.join(format!("{name}.png")));
    // The fake game's folder is the studio's: keep it.
    std::mem::forget(p.game);
}

#[test]
#[ignore]
fn render_the_screens() {
    let mut game = FakeGame::new("preview");
    let mut st = studio(&mut game);
    for (i, ex) in horizon_studio::game::hstudio_examples::EXAMPLES.iter().enumerate() {
        if ["coords", "waypoints", "compass", "playground", "miner"].contains(&ex.id) {
            st.install_example(&mut game, i).unwrap();
        }
    }
    game.write_mod("broken.lua", "-- name: Broken demo\n-- version: 0.2\n-- description: Fails on purpose, to show an error.\nlocal t = nil\nprint(t.x)\n");
    run_for(&mut st, &mut game, 0.6);
    for line in [
        "player.pos()",
        "x = 21 * 2",
        "x",
        "{ name = 'Steve', items = { 'pickaxe', 'torch' } }",
        "nope()",
        "help('world.find')",
    ] {
        st.run_console(&mut game, line);
    }
    st.s.ui.open = true;
    st.s.ui.input.set("world.block(");
    shot(&mut st, "console", |st, p| draw(st, p));
    st.s.ui.input = LineEdit::default();
    let i = st.s.mods.iter().position(|m| m.id == "waypoints").unwrap();
    st.edit_mod(i);
    st.s.ui.tab = Tab::Editor;
    st.s.editor.click(8, 4, false);
    st.s.editor.move_v(2, true);
    st.s.editor.error_line = Some(19);
    shot(&mut st, "editor", |st, p| draw(st, p));
    st.s.ui.tab = Tab::Library;
    st.s.ui.library_sel = st.s.mods.iter().position(|m| m.id == "waypoints").unwrap();
    shot(&mut st, "library", |st, p| draw(st, p));
    st.s.ui.library_sel = st.s.mods.iter().position(|m| m.id == "broken").unwrap();
    shot(&mut st, "library-error", |st, p| draw(st, p));
    st.s.ui.tab = Tab::Docs;
    shot(&mut st, "docs", |st, p| draw(st, p));
    st.s.ui.tab = Tab::Library;
    st.s.ui.naming = Some(LineEdit::default());
    st.s.ui.naming.as_mut().unwrap().set("Night Vision");
    shot(&mut st, "new-mod", |st, p| draw(st, p));
    st.s.ui.naming = None;
    assert!(st.command("playground"));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen());
    shot(&mut st, "screen", |st, p| {
        draw_screen(st, p);
        let _ = Input::Wheel(0.0);
    });
    st.close_screen(&mut game);
    // The UI Kit draws its own animations: let them run their course.
    let i = horizon_studio::game::hstudio_examples::EXAMPLES
        .iter()
        .position(|e| e.id == "uikit_demo")
        .unwrap();
    st.install_example(&mut game, i).unwrap();
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("kitdemo"));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen());
    st.s.ui.library_sel = st.s.mods.iter().position(|m| m.id == "uikit").unwrap();
    shot(&mut st, "kit", |st, p| {
        for _ in 0..40 {
            st.s.now += 0.05;
            let mut warm = Preview {
                game: FakeGame::new("kit-warm"),
                canvas: Canvas::new(),
            };
            warm.game.dir = st.s.dir.clone();
            draw_screen(st, &mut warm);
            std::mem::forget(warm.game);
        }
        draw_screen(st, p);
    });
    st.close_screen(&mut game);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Library;
    shot(&mut st, "library-kit", |st, p| draw(st, p));
}

/// ViaHorizon's picker over a stand-in for the multiplayer screen: its
/// header, the server list's first row and the button (in GUI pixels at a
/// GUI scale of 2, so 4/3 dp each).
#[test]
#[ignore]
fn render_the_via_picker() {
    use horizon_studio::game::hstudio_via_picker::{Info, Key, Paint, Picker, Tone};
    struct On<'a>(&'a mut Canvas);
    impl Paint for On<'_> {
        fn rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
            self.0.rounded(r, radius, argb, false);
        }
        fn border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
            self.0.rounded(r, radius, argb, true);
        }
        fn text(&mut self, s: &str, x: f32, y: f32, size: f32, medium: bool, argb: u32) -> f32 {
            self.0
                .text(s, [x, y], size, if medium { Font::Medium } else { Font::Regular }, argb)
        }
        fn measure(&mut self, s: &str, size: f32, medium: bool) -> f32 {
            self.0
                .measure(s, size, if medium { Font::Medium } else { Font::Regular })
        }
    }
    let ids = [
        "auto",
        "26.3",
        "26.2",
        "26.1-26.1.2",
        "1.21.11",
        "1.21.4",
        "1.20.5-1.20.6",
        "1.20.3-1.20.4",
        "1.19.4",
        "1.18.2",
        "1.16.4-1.16.5",
        "1.12.2",
        "1.12.1",
        "1.8.x",
        "1.7.6-1.7.10",
        "b1.7-b1.7.3",
        "c0.30 CPE",
        "Bedrock 1.26.51",
    ];
    let info = Info {
        enabled: true,
        version: "1.8.x".into(),
        status: "Ready (ViaProxy 3.4.14, Java 21)".into(),
        tone: Tone::Ready,
        versions: ids
            .iter()
            .map(|v| {
                (
                    (*v).into(),
                    if *v == "auto" {
                        "Auto detect".into()
                    } else {
                        (*v).into()
                    },
                )
            })
            .collect(),
    };
    let k = 4.0 / 3.0;
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/preview");
    std::fs::create_dir_all(&out).unwrap();
    let screen = |c: &mut Canvas| {
        c.rounded([0.0, 0.0, W, H], 0.0, 0x8C00_0000, false);
        c.rounded([0.0, 0.0, W, 33.0 * k], 0.0, 0x6600_0000, false);
        let tw = c.measure("Play Multiplayer", 12.0, Font::Regular);
        c.text(
            "Play Multiplayer",
            [(W - tw) / 2.0, 7.0 * k],
            12.0,
            Font::Regular,
            0xFFFF_FFFF,
        );
        let b = [W - (112.0 + 6.0) * k, 6.0 * k, 112.0 * k, 20.0 * k];
        c.rounded(b, 0.0, 0xFF6F_6F6F, false);
        c.rounded(b, 0.0, 0xFF00_0000, true);
        c.rounded([b[0] + 22.0, b[1] + 11.0, 4.0, 4.0], 0.0, 0xFF45_C27A, false);
        c.text(
            "Via: 1.8.x",
            [b[0] + 30.0, b[1] + 4.0],
            12.0,
            Font::Regular,
            0xFFFF_FFFF,
        );
        c.text(
            "Horizon Official",
            [W / 2.0 - 140.0, 45.0 * k],
            12.0,
            Font::Regular,
            0xFFFF_FFFF,
        );
    };
    let mut button = Canvas::new();
    screen(&mut button);
    button.save(&out.join("via-button.png"));
    let mut c = Canvas::new();
    screen(&mut c);
    let mut picker = Picker::default();
    picker.show(0.0, &info);
    picker.point((W / 2.0, 330.0));
    for i in 0..40 {
        picker.draw(&mut On(&mut Canvas::new()), &info, W, H, f64::from(i) * 0.05);
    }
    picker.draw(&mut On(&mut c), &info, W, H, 2.5);
    c.save(&out.join("via-picker.png"));
    let mut c = Canvas::new();
    screen(&mut c);
    picker.key(3.0, Key::Text("1.1".into()), &info);
    for i in 0..10 {
        picker.draw(&mut On(&mut Canvas::new()), &info, W, H, 3.0 + f64::from(i) * 0.05);
    }
    picker.draw(&mut On(&mut c), &info, W, H, 3.6);
    c.save(&out.join("via-picker-search.png"));
}

/// Each theme on the editor, the Settings page, and the studio in a window.
#[test]
#[ignore]
fn render_the_themes() {
    use horizon_studio::game::hstudio_theme::{Layout, Look, THEMES};
    let mut game = FakeGame::new("preview-themes");
    let mut st = studio(&mut game);
    for (i, ex) in horizon_studio::game::hstudio_examples::EXAMPLES.iter().enumerate() {
        if ["coords", "waypoints", "compass", "outfit"].contains(&ex.id) {
            st.install_example(&mut game, i).unwrap();
        }
    }
    run_for(&mut st, &mut game, 0.6);
    for line in ["player.pos()", "x = 21 * 2", "nope()"] {
        st.run_console(&mut game, line);
    }
    st.s.ui.open = true;
    let i = st.s.mods.iter().position(|m| m.id == "waypoints").unwrap();
    st.edit_mod(i);
    for (n, t) in THEMES.iter().enumerate() {
        st.s.ui.look = Look {
            theme: n,
            motion: false,
            ..Look::default()
        };
        st.s.ui.tab = Tab::Editor;
        shot(&mut st, &format!("theme-{}", t.id), |st, p| draw(st, p));
        if t.layout != Layout::Studio {
            st.s.ui.tab = Tab::Library;
            shot(&mut st, &format!("theme-{}-library", t.id), |st, p| draw(st, p));
        }
    }
    st.s.ui.look = Look::default();
    st.s.ui.tab = Tab::Settings;
    shot(&mut st, "settings", |st, p| draw(st, p));
    st.s.ui.look = Look {
        theme: 1,
        window: true,
        motion: false,
        ..Look::default()
    };
    st.s.ui.tab = Tab::Settings;
    shot(&mut st, "settings-light-window", |st, p| draw(st, p));
    st.s.ui.look = Look {
        window: true,
        scale: 0.85,
        motion: false,
        ..Look::default()
    };
    st.s.ui.tab = Tab::Console;
    shot(&mut st, "window-small", |st, p| draw(st, p));
}

/// The Statue example's meshes, drawn by a small rasterizer (nearest
/// texels, a depth buffer, an orthographic camera turned `yaw` degrees
/// round them): textures and box UV as the game would map them.
#[test]
#[ignore]
fn render_the_statue_meshes() {
    use horizon_studio::game::hstudio_meshes::{Mesh, Store};
    use horizon_studio::headless::{Run, Source};
    fn render(store: &Store, mesh: &Mesh, yaw: f32, name: &str) {
        const S: usize = 320;
        let (lo, hi) = mesh.bounds();
        let size = (0..3).map(|i| hi[i] - lo[i]).fold(0.0f32, f32::max) * 1.25;
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let (sn, cs) = yaw.to_radians().sin_cos();
        // Looking from +z toward -z, turned about y: screen x is east at yaw 0.
        let project = |p: [f32; 3]| {
            let (x, z) = (p[0] - c[0], p[2] - c[2]);
            let (rx, rz) = (x * cs - z * sn, x * sn + z * cs);
            [
                (rx / size + 0.5) * S as f32,
                ((c[1] - p[1]) / size + 0.5) * S as f32,
                rz,
            ]
        };
        let mut px = vec![[40u8, 44, 52]; S * S];
        let mut depth = vec![f32::MIN; S * S];
        for t in &mesh.tris {
            let q = t.p.map(project);
            let tex = t.tex.and_then(|id| store.textures.get(&id));
            let (x0, x1) = (
                q.iter().map(|v| v[0]).fold(f32::MAX, f32::min),
                q.iter().map(|v| v[0]).fold(f32::MIN, f32::max),
            );
            let (y0, y1) = (
                q.iter().map(|v| v[1]).fold(f32::MAX, f32::min),
                q.iter().map(|v| v[1]).fold(f32::MIN, f32::max),
            );
            let area = (q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1]);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in (y0.max(0.0) as usize)..(y1.ceil().min(S as f32) as usize) {
                for x in (x0.max(0.0) as usize)..(x1.ceil().min(S as f32) as usize) {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w1 = ((fx - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (fy - q[0][1])) / area;
                    let w2 = ((q[1][0] - q[0][0]) * (fy - q[0][1]) - (fx - q[0][0]) * (q[1][1] - q[0][1])) / area;
                    let w0 = 1.0 - w1 - w2;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * q[0][2] + w1 * q[1][2] + w2 * q[2][2];
                    if z <= depth[y * S + x] {
                        continue;
                    }
                    let u = w0 * t.uv[0][0] + w1 * t.uv[1][0] + w2 * t.uv[2][0];
                    let v = w0 * t.uv[0][1] + w1 * t.uv[1][1] + w2 * t.uv[2][1];
                    let mut rgba = [
                        (t.argb >> 16) as u8,
                        (t.argb >> 8) as u8,
                        t.argb as u8,
                        (t.argb >> 24) as u8,
                    ];
                    if let Some(tx) = tex {
                        let (ix, iy) = (
                            ((u * tx.w as f32) as u32).min(tx.w - 1),
                            ((v * tx.h as f32) as u32).min(tx.h - 1),
                        );
                        let i = ((iy * tx.w + ix) * 4) as usize;
                        let s = &tx.rgba[i..i + 4];
                        rgba = [
                            (u32::from(s[0]) * u32::from(rgba[0]) / 255) as u8,
                            (u32::from(s[1]) * u32::from(rgba[1]) / 255) as u8,
                            (u32::from(s[2]) * u32::from(rgba[2]) / 255) as u8,
                            s[3],
                        ];
                    }
                    if rgba[3] < 128 {
                        continue;
                    }
                    depth[y * S + x] = z;
                    px[y * S + x] = [rgba[0], rgba[1], rgba[2]];
                }
            }
        }
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/preview");
        std::fs::create_dir_all(&out).unwrap();
        let file = std::fs::File::create(out.join(format!("{name}.png"))).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), S as u32, S as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px.concat()).unwrap();
    }
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/statue.lua")).unwrap();
    let mut r = Run::new(&Source::Files {
        id: "statue".into(),
        files: vec![("statue.lua".into(), src)],
        libraries: None,
    })
    .unwrap();
    r.frames(2);
    let store = r.studio.s.mods[0].meshes.clone();
    let meshes: Vec<Mesh> = store.meshes.values().cloned().collect();
    render(&store, &meshes[0], 0.0, "statue-front");
    render(&store, &meshes[0], -35.0, "statue-turned");
    render(&store, &meshes[1], -30.0, "statue-crystal");
}

/// The editor as in VS Code (its problems marked, the suggestions open, the
/// panel under it on each of its tabs) at the sizes a game window comes
/// in, full screen and windowed: nothing should overlap or run off.
#[test]
#[ignore]
fn render_the_editor_at_every_size() {
    use horizon_studio::game::hstudio_theme::Look;
    use horizon_studio::game::hstudio_ui::{AgentLine, AgentLineKind, Panel, refresh_suggest};
    let mut game = FakeGame::new("preview-sizes");
    let mut st = studio(&mut game);
    for (i, ex) in horizon_studio::game::hstudio_examples::EXAMPLES.iter().enumerate() {
        if ex.id == "waypoints" {
            st.install_example(&mut game, i).unwrap();
        }
    }
    run_for(&mut st, &mut game, 0.6);
    for line in ["player.pos()", "x = 21 * 2", "nope()"] {
        st.run_console(&mut game, line);
    }
    st.s.ui.open = true;
    let i = st.s.mods.iter().position(|m| m.id == "waypoints").unwrap();
    st.edit_mod(i);
    st.s.ui.tab = Tab::Editor;
    // A typo and a broken line, for the live check.
    let mut text = st.s.editor.text();
    text = text.replacen("local", "local plyer = player.poss()\nlocal", 1);
    st.s.editor.set_text(&text);
    st.s.editor.click(0, 0, false);
    let now = st.s.now;
    st.s.ui.agent.lines = vec![
        AgentLine {
            kind: AgentLineKind::You,
            text: "Make the waypoints glow at night".into(),
            at: now,
            jump: None,
        },
        AgentLine {
            kind: AgentLineKind::Said,
            text: "I'll read the script first, then add a glow that turns on after dusk.".into(),
            at: now,
            jump: None,
        },
        AgentLine {
            kind: AgentLineKind::ToolOk,
            text: "read_mod waypoints".into(),
            at: now,
            jump: None,
        },
        AgentLine {
            kind: AgentLineKind::Tool,
            text: "write_mod waypoints".into(),
            at: now,
            jump: None,
        },
    ];
    let settle = |st: &mut Studio, p: &mut Preview| {
        // Frames a 60th of a second apart, so everything settles.
        for _ in 0..60 {
            st.s.now += 1.0 / 60.0;
            let mut warm = Preview {
                game: FakeGame::new("sizes-warm"),
                canvas: Canvas::blind(),
            };
            warm.game.dir = st.s.dir.clone();
            draw(st, &mut warm);
            std::mem::forget(warm.game);
        }
        draw(st, p);
    };
    for (w, h) in [(640.0, 400.0), (960.0, 540.0), (1240.0, 720.0), (1920.0, 1080.0)] {
        SIZE.with(|c| c.set((w, h)));
        for windowed in [false, true] {
            st.s.ui.look = Look {
                window: windowed,
                ..Look::default()
            };
            for panel in [Panel::Problems, Panel::Console, Panel::Agent] {
                st.s.ui.panel = panel;
                st.s.ui.panel_hidden = false;
                st.s.ui.panel_focus = panel == Panel::Agent;
                // The suggestions for what is typed on the line with the typo.
                st.s.ui.panel_focus = false;
                let row = st.s.editor.lines.iter().position(|l| l.contains("plyer")).unwrap();
                st.s.editor.click(row, "local plyer = player.po".len(), false);
                refresh_suggest(&mut st);
                st.s.ui.panel_focus = panel == Panel::Agent;
                let name = format!(
                    "sizes-{}x{}{}-{:?}",
                    w as u32,
                    h as u32,
                    if windowed { "-window" } else { "" },
                    panel
                )
                .to_lowercase();
                shot(&mut st, &name, settle);
            }
        }
    }
    // A syntax error where it shows.
    SIZE.with(|c| c.set((1240.0, 720.0)));
    st.s.ui.look = Look::default();
    st.s.ui.suggest = None;
    let broken = text.replacen("local colors = {", "local colors = {{", 1);
    st.s.editor.set_text(&broken);
    st.s.ui.panel = Panel::Problems;
    st.s.ui.panel_focus = false;
    shot(&mut st, "sizes-syntax-error", settle);
    st.s.editor.set_text(&text);
    // The other tabs, and the executor styles, at the smallest and largest.
    for (w, h) in [(640.0, 400.0), (1920.0, 1080.0)] {
        SIZE.with(|c| c.set((w, h)));
        for tab in [Tab::Console, Tab::Library, Tab::Docs, Tab::Settings] {
            st.s.ui.look = Look::default();
            st.s.ui.tab = tab;
            shot(
                &mut st,
                &format!("sizes-{}x{}-{:?}", w as u32, h as u32, tab).to_lowercase(),
                settle,
            );
        }
        for (n, t) in horizon_studio::game::hstudio_theme::THEMES.iter().enumerate() {
            if t.layout == horizon_studio::game::hstudio_theme::Layout::Studio {
                continue;
            }
            st.s.ui.look = Look {
                theme: n,
                ..Look::default()
            };
            st.s.ui.tab = Tab::Editor;
            shot(&mut st, &format!("sizes-{}x{}-{}", w as u32, h as u32, t.id), settle);
        }
    }
    SIZE.with(|c| c.set((W, H)));
}

#[test]
#[ignore]
fn render_the_agent_in_the_editor() {
    use horizon_studio::game::hstudio_coach::{self as coach, By};
    use horizon_studio::game::hstudio_theme::{Look, THEMES};
    use horizon_studio::game::hstudio_ui::Panel;
    use serde_json::json;
    let mut game = FakeGame::new("preview-agent");
    let mut st = studio(&mut game);
    for (i, ex) in horizon_studio::game::hstudio_examples::EXAMPLES.iter().enumerate() {
        if ex.id == "waypoints" {
            st.install_example(&mut game, i).unwrap();
        }
    }
    run_for(&mut st, &mut game, 0.6);
    st.s.ui.open = true;
    let i = st.s.mods.iter().position(|m| m.id == "waypoints").unwrap();
    st.edit_mod(i);
    st.s.ui.tab = Tab::Editor;
    st.s.ui.panel = Panel::Agent;
    st.s.ui.panel_hidden = false;
    let by = By {
        name: "Claude Code".into(),
        conn: 1,
    };
    let settle = |st: &mut Studio, p: &mut Preview| {
        for _ in 0..60 {
            st.s.now += 1.0 / 60.0;
            let mut warm = Preview {
                game: FakeGame::new("agent-warm"),
                canvas: Canvas::blind(),
            };
            warm.game.dir = st.s.dir.clone();
            draw(st, &mut warm);
            std::mem::forget(warm.game);
        }
        draw(st, p);
    };
    // Pointing at code: a bug's lines in red with its note, and two more notes.
    for (line, tone, note) in [
        (6, "good", "Each waypoint keeps its name and place."),
        (12, "warning", "This runs every tick: keep it cheap."),
        (
            4,
            "error",
            "This loop never ends: i never changes, so i < 10 stays true. Add i = i + 1 at the end of the loop.",
        ),
    ] {
        let mark = json!({ "start_line": line, "end_line": line + 1, "tone": tone, "note": note });
        coach::call(&mut st, &mut game, &by, "mark", &mark).unwrap();
    }
    shot(&mut st, "agent-marks", settle);
    // A lesson, on its second step, its explanation in the Agent panel.
    let lesson = json!({ "title": "Waypoints", "steps": [
        { "start_line": 1, "note": "The mod's name and what it does.", "explain": "A header comment names the mod." },
        { "start_line": 6, "end_line": 8, "note": "A table holds the waypoints.", "task": "Add a waypoint called home.", "explain": "Tables hold lists and records: this one is a list of records." },
        { "start_line": 12, "note": "Its command adds one where you stand." },
    ] });
    coach::call(&mut st, &mut game, &by, "unmark", &json!({})).unwrap();
    coach::call(&mut st, &mut game, &by, "lesson", &lesson).unwrap();
    st.s.ui.agent.agents = Some(Vec::new());
    // On to step 2, as Alt+Right goes.
    let alt = horizon_studio::game::hstudio_ui::Mods {
        ctrl: false,
        shift: false,
        alt: true,
    };
    let right = Input::Key {
        key: horizon_studio::game::hstudio_ui::Key::Right,
        down: true,
        repeat: false,
        mods: alt,
    };
    horizon_studio::game::hstudio_ui::handle(&mut st, &mut game, right);
    // An agent waits for the player: the panel says so, and Check shows.
    let waiting = |st: &mut Studio, p: &mut Preview| {
        coach::player_event(st, &by, &json!({ "after": 1000 })).unwrap();
        settle(st, p);
    };
    shot(&mut st, "agent-lesson", waiting);
    // The same in the Synapse X style, where the explanation is in the note.
    st.s.ui.look = Look {
        theme: THEMES.iter().position(|t| t.id == "synapse").unwrap(),
        ..Look::default()
    };
    shot(&mut st, "agent-lesson-synapse", waiting);
    // The agent typing a change in, with its own caret and name.
    st.s.ui.look = Look::default();
    coach::call(&mut st, &mut game, &by, "unmark", &json!({})).unwrap();
    st.s.ui.coach.input_at = f64::NEG_INFINITY;
    st.s.ui.coach.player_at = f64::NEG_INFINITY;
    let edit = json!({ "start_line": 3, "end_line": 2, "new_text": "local glow = true -- the waypoints glow at night\nlocal glow_from = 13000" });
    coach::call(&mut st, &mut game, &by, "edit", &edit).unwrap();
    run_for(&mut st, &mut game, 0.6);
    shot(&mut st, "agent-typing", settle);
}
