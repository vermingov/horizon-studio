// Horizon Studio's drawing with Horizon's design system, added to the
// game's `ui_theme` (as `ui_theme::hstudio_paint`) so it reaches the
// toolkit's private font cache: rounded plates, 1 px borders, Geist and
// Geist Mono text at the screen's own resolution, laid out in dp (the
// launcher's logical pixels). Colours are the HUD's (`0xAABBGGRR`).

use super::{Cached, Face, Key, Ui, with_cache};
use crate::hud::{HudAtlas, HudBatch};
use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};

/// Regular, Medium or Mono.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Regular,
    Medium,
    Mono,
}

thread_local! {
    static MONO: FontRef<'static> =
        FontRef::try_from_slice(horizon_theme::fonts::GEIST_MONO).expect("bundled font");
}

/// Mono glyphs live in the toolkit's cache beside its own, under ids it
/// never uses (Geist's glyph ids stay far below this bit).
const MONO_BIT: u16 = 0x8000;

fn ui<'a>(atlas: &'a HudAtlas, batch: &'a mut HudBatch) -> Ui<'a> {
    Ui::new(atlas, batch, (0.0, 0.0))
}

/// The screen in dp, and dp per GUI pixel (for the pointer).
pub fn size(atlas: &HudAtlas, batch: &mut HudBatch) -> (f32, f32, f32) {
    let u = ui(atlas, batch);
    (u.w, u.h, u.scale / u.density)
}

pub fn rect(atlas: &HudAtlas, batch: &mut HudBatch, r: [f32; 4], radius: f32, color: u32) {
    ui(atlas, batch).rounded(r[0], r[1], r[2], r[3], radius, color);
}

pub fn border(atlas: &HudAtlas, batch: &mut HudBatch, r: [f32; 4], radius: f32, color: u32) {
    ui(atlas, batch).border(r[0], r[1], r[2], r[3], radius, color);
}

/// Text whose line box (1.45 × size) has its top at `y`; its width in dp.
pub fn text(atlas: &HudAtlas, batch: &mut HudBatch, font: Font, size: f32, x: f32, y: f32, color: u32, s: &str) -> f32 {
    let mut u = ui(atlas, batch);
    match font {
        Font::Regular => u.text(Face::Regular, size, x, y, color, s),
        Font::Medium => u.text(Face::Medium, size, x, y, color, s),
        Font::Mono => mono_text(&mut u, size, (size * 1.45).round(), x, y, color, s),
    }
}

pub fn measure(atlas: &HudAtlas, batch: &mut HudBatch, font: Font, size: f32, s: &str) -> f32 {
    let u = ui(atlas, batch);
    match font {
        Font::Regular => u.measure(Face::Regular, size, s),
        Font::Medium => u.measure(Face::Medium, size, s),
        Font::Mono => {
            let px = (size * u.density * 4.0).round() / 4.0;
            MONO.with(|f| {
                let scaled = f.as_scaled(mono_scale(f, px));
                s.chars().map(|c| scaled.h_advance(f.glyph_id(c))).sum::<f32>() / u.density
            })
        }
    }
}

/// Hands the glyphs rasterized this frame to the batch's atlas uploads.
pub fn flush(atlas: &HudAtlas, batch: &mut HudBatch) {
    ui(atlas, batch).finish();
}

fn mono_scale(font: &FontRef<'static>, px: f32) -> PxScale {
    let upem = font.units_per_em().unwrap_or(1000.0);
    PxScale::from(px * font.height_unscaled() / upem)
}

/// `Ui::text_line` for Geist Mono, through the same atlas cache.
fn mono_text(u: &mut Ui<'_>, size: f32, line: f32, x: f32, y: f32, color: u32, s: &str) -> f32 {
    let px = (size * u.density * 4.0).round() / 4.0;
    let real = color >> 24 != 0;
    let (x0, y0) = ((x * u.density).round(), (y * u.density).round());
    let box_px = line * u.density;
    let atlas = u.atlas;
    let density = u.density;
    let mut quads = Vec::new();
    let width = MONO.with(|font| {
        let scale = mono_scale(font, px);
        let f = font.as_scaled(scale);
        let baseline = y0 + ((box_px - (f.ascent() - f.descent())) / 2.0 + f.ascent()).round();
        let mut pen = x0;
        for ch in s.chars() {
            let id = font.glyph_id(ch);
            if real && !ch.is_whitespace() && id.0 < MONO_BIT {
                let size4 = (px * 4.0).round() as u16;
                let key = Key::Glyph(Face::Regular, id.0 | MONO_BIT, size4);
                let cached = with_cache(atlas, |c| {
                    if let Some(hit) = c.entries.get(&key) {
                        return *hit;
                    }
                    let glyph = id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
                    let made = match font.outline_glyph(glyph) {
                        Some(outline) => {
                            let b = outline.px_bounds();
                            let (w, h) = (b.width() as u32, b.height() as u32);
                            let mut alpha = vec![0.0; (w * h) as usize];
                            outline.draw(|gx, gy, cov| {
                                if gx < w && gy < h {
                                    alpha[(gy * w + gx) as usize] = cov.powf(0.85);
                                }
                            });
                            Cached {
                                rect: c.upload(atlas, w, h, &alpha),
                                left: b.min.x,
                                top: b.min.y,
                            }
                        }
                        None => Cached {
                            rect: None,
                            left: 0.0,
                            top: 0.0,
                        },
                    };
                    if made.rect.is_some() {
                        c.entries.insert(key, made);
                    }
                    made
                });
                if let Some(rect) = cached.rect {
                    quads.push(((pen + cached.left).round(), baseline + cached.top, rect));
                }
            }
            pen += f.h_advance(id);
        }
        (pen - x0) / density
    });
    let sc = u.scale;
    for (gx, gy, rect) in quads {
        u.batch
            .rect_uv(gx / sc, gy / sc, rect.w as f32 / sc, rect.h as f32 / sc, rect, color);
    }
    width
}
