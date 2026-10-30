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

impl Canvas {
    fn new() -> Canvas {
        let dir = fonts_dir();
        let (w, h) = ((W * DENSITY) as usize, (H * DENSITY) as usize);
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
            fonts: [
                font(&dir, "Geist-Regular.ttf"),
                font(&dir, "Geist-Medium.ttf"),
                font(&dir, "GeistMono-Regular.ttf"),
            ],
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
        (W / 2.0, H / 2.0)
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
        (W, H, 2.0)
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
