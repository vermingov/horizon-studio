// Roblox's images in the GUI, beside tests/rbx_gui.rs: `rbxasset://` files
// from the Client Tracker's mirror of Roblox's content folder, thumbnails
// (700 pixels a side) drawn whole, a thumbnail's original size worked out
// from its resampling and from the crops and slices scripts make, so a
// sprite sheet's icons are cut where the script means, and small parts of
// large images (an icon of a sheet, of a strip) put in the atlas on their
// own. Nothing here goes to the network: the images are put where the
// fetch keeps them (the workspace's .roblox-assets).

mod common;

use common::{FakeGame, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_meshes::Texture;
use horizon_studio::game::hstudio_rbx_image::{Uses, effective_size, native_size, rbxasset_source};
use horizon_studio::game::hstudio_sunc::roblox_image;

const MIRROR: &str = "https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/";

#[test]
fn rbxasset_files_come_from_the_client_trackers_mirror() {
    let mirrored =
        |path: &str, remote: &str| Some((format!("{MIRROR}{remote}"), format!(".roblox-assets/rbxasset/{path}")));
    assert_eq!(
        roblox_image("rbxasset://textures/ClassImages.PNG"),
        mirrored("textures/ClassImages.PNG", "textures/ClassImages.PNG")
    );
    assert_eq!(
        roblox_image("rbxasset://textures/ui/Scroll/scroll-middle.png"),
        mirrored(
            "textures/ui/Scroll/scroll-middle.png",
            "textures/ui/Scroll/scroll-middle.png"
        )
    );
    // The mirror spells studio_svg_textures with dashes.
    assert_eq!(
        roblox_image("rbxasset://studio_svg_textures/Shared/InsertableObjects/Dark/Standard/Part.png"),
        mirrored(
            "studio_svg_textures/Shared/InsertableObjects/Dark/Standard/Part.png",
            "studio-svg-textures/Shared/InsertableObjects/Dark/Standard/Part.png"
        )
    );
    // Backslashes are slashes, as on Windows; spaces are escaped.
    assert_eq!(
        rbxasset_source("RBXASSET://textures\\ui\\a b.png"),
        mirrored("textures/ui/a b.png", "textures/ui/a%20b.png")
    );
    // The workspace's own files aren't Roblox's, and nothing climbs out.
    assert_eq!(roblox_image("rbxasset://horizon-studio/x.png"), None);
    assert_eq!(rbxasset_source("rbxasset://textures/../../secret.png"), None);
    assert_eq!(rbxasset_source("rbxasset://"), None);
    assert_eq!(rbxasset_source("rbxasset://c:/x.png"), None);
}

/// A tiny random number generator, for images the same every run.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// A sprite sheet `cells` icons a side, each `cell` pixels with a
/// transparent gutter of 2: random shapes, one colour each.
fn sheet(cells: u32, cell: u32, seed: u64) -> Texture {
    let side = cells * cell;
    let mut rng = Lcg(seed);
    let mut rgba = vec![0u8; (side * side * 4) as usize];
    for gy in 0..cells {
        for gx in 0..cells {
            let c = [rng.next() as u8, rng.next() as u8, rng.next() as u8];
            for y in 2..cell - 2 {
                for x in 2..cell - 2 {
                    if rng.next().is_multiple_of(2) {
                        let i = (((gy * cell + y) * side + gx * cell + x) * 4) as usize;
                        rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                    }
                }
            }
        }
    }
    Texture {
        w: side,
        h: side,
        rgba,
        key: 0,
        version: 0,
    }
}

/// `t` with each pixel made `k` x `k` (an image drawn at `k` times its size).
fn doubled(t: &Texture, k: u32) -> Texture {
    let (w, h) = (t.w * k, t.h * k);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let s = (((y / k) * t.w + x / k) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            rgba[d..d + 4].copy_from_slice(&t.rgba[s..s + 4]);
        }
    }
    Texture {
        w,
        h,
        rgba,
        key: 0,
        version: 0,
    }
}

/// `t` resampled to `w` x `h` (bilinear, pixel centres aligned), as the
/// thumbnails' server scales an image to fit its box.
fn resampled(t: &Texture, w: u32, h: u32) -> Texture {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let px = |x: i64, y: i64, c: usize| -> f64 {
        let x = x.clamp(0, i64::from(t.w) - 1) as u32;
        let y = y.clamp(0, i64::from(t.h) - 1) as u32;
        f64::from(t.rgba[((y * t.w + x) * 4) as usize + c])
    };
    for y in 0..h {
        let sy = (f64::from(y) + 0.5) * f64::from(t.h) / f64::from(h) - 0.5;
        let (y0, fy) = (sy.floor() as i64, sy - sy.floor());
        for x in 0..w {
            let sx = (f64::from(x) + 0.5) * f64::from(t.w) / f64::from(w) - 0.5;
            let (x0, fx) = (sx.floor() as i64, sx - sx.floor());
            for c in 0..4 {
                let top = px(x0, y0, c) * (1.0 - fx) + px(x0 + 1, y0, c) * fx;
                let bottom = px(x0, y0 + 1, c) * (1.0 - fx) + px(x0 + 1, y0 + 1, c) * fx;
                rgba[((y * w + x) * 4) as usize + c] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    }
    Texture {
        w,
        h,
        rgba,
        key: 0,
        version: 0,
    }
}

/// The 700 and 1024 pixel thumbnails of `t` (its long side that long).
fn thumbnails(t: &Texture) -> (Texture, Texture) {
    let long = t.w.max(t.h);
    let fit = |side: u32| {
        let k = f64::from(side) / f64::from(long);
        resampled(
            t,
            (f64::from(t.w) * k).round() as u32,
            (f64::from(t.h) * k).round() as u32,
        )
    };
    (fit(700), fit(1024))
}

#[test]
fn a_thumbnail_shows_the_resolution_it_was_made_from() {
    // A 256-pixel sheet of 16-pixel icons, scaled up to 700 and 1024.
    let s = sheet(16, 16, 1);
    let (a, b) = thumbnails(&s);
    assert_eq!(effective_size(&[&a, &b]), Some(256));
    // Dex++'s NewDark sheet's shape: 18 by 17 icons of 32 pixels (576 x
    // 544), its thumbnails 700 x 661 and 1024 x 967.
    let mut tall = sheet(18, 32, 2);
    tall.rgba.truncate((576 * 544 * 4) as usize);
    tall.h = 544;
    let (a, b) = thumbnails(&tall);
    assert_eq!((a.w, a.h, b.w, b.h), (700, 661, 1024, 967));
    assert_eq!(effective_size(&[&a, &b]), Some(576));
    // A sheet larger than the thumbnails (scaled down to them).
    let big = sheet(25, 32, 3);
    let (a, b) = thumbnails(&big);
    assert_eq!(effective_size(&[&a, &b]), Some(800));
    // Each pixel doubled (Dex++'s Vanilla3 sheet: 16-pixel icons drawn at
    // 32 in an 800-pixel sheet): the pixels are a 400's.
    let (a, b) = thumbnails(&doubled(&sheet(25, 16, 5), 2));
    assert_eq!(effective_size(&[&a, &b]), Some(400));
    // Even a gradient shows it (its rounding repeats with the resampling).
    let mut smooth = sheet(4, 64, 4);
    for (i, p) in smooth.rgba.chunks_mut(4).enumerate() {
        let (x, y) = ((i % 256) as f64, (i / 256) as f64);
        p.copy_from_slice(&[(x / 256.0 * 255.0) as u8, (y / 256.0 * 255.0) as u8, 128, 255]);
    }
    let (a, b) = thumbnails(&smooth);
    assert_eq!(effective_size(&[&a, &b]), Some(256));
    // One colour shows nothing.
    for p in smooth.rgba.chunks_mut(4) {
        p.copy_from_slice(&[40, 90, 200, 255]);
    }
    let (a, b) = thumbnails(&smooth);
    assert_eq!(effective_size(&[&a, &b]), None);
}

#[test]
fn the_size_scripts_measure_a_thumbnail_in() {
    let crops = |reach: [f64; 2]| {
        let mut u = Uses::default();
        u.crop([reach[0] - 16.0, reach[1] - 16.0], [16.0, 16.0]);
        u
    };
    // The resolution, when the crops fit it.
    assert_eq!(
        native_size((700, 661), Some(576), &crops([576.0, 544.0])),
        (576.0, 544.0)
    );
    assert_eq!(
        native_size((700, 700), Some(256), &crops([160.0, 32.0])),
        (256.0, 256.0)
    );
    // A pixel-doubled sheet reads as half its size until a crop reaches
    // past the half: then it is the whole multiple the crops fit in.
    assert_eq!(
        native_size((700, 700), Some(400), &crops([64.0, 416.0])),
        (800.0, 800.0)
    );
    // Not known (no trace, or not worked out yet): the thumbnail's size,
    // larger only as far as the crops reach past it.
    assert_eq!(native_size((700, 700), None, &crops([64.0, 64.0])), (700.0, 700.0));
    assert_eq!(native_size((700, 350), None, &crops([64.0, 400.0])), (800.0, 400.0));
    // A slice: the size its SliceCenter's insets, alike on both sides, make.
    let mut slice = Uses::default();
    slice.slice([6.0, 6.0], [25.0, 25.0]);
    assert_eq!(native_size((700, 700), None, &slice), (31.0, 31.0));
    slice.slice([2.0, 2.0], [254.0, 254.0]);
    assert_eq!(native_size((700, 700), Some(1680), &slice), (256.0, 256.0));
    // No use in pixels: the thumbnail's own size.
    assert_eq!(native_size((700, 661), Some(576), &Uses::default()), (700.0, 661.0));
}

/// A PNG of `t`.
fn png(t: &Texture) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, t.w, t.h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&t.rgba).unwrap();
    out
}

/// Loads `src` as mod `id` and lets it run.
fn load(game: &mut FakeGame, st: &mut Studio, id: &str, src: &str) {
    game.write_mod(&format!("{id}.lua"), src);
    run_for(st, game, 0.6);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}");
    }
}

/// The images the HUD draws: "WxH #firstpixel [where] from [part]".
fn images(st: &mut Studio, game: &mut FakeGame) -> Vec<String> {
    game.drawn.clear();
    st.hud(game);
    game.drawn
        .iter()
        .filter_map(|d| d.strip_prefix("image "))
        .map(|d| d.rsplit_once(' ').map_or(d, |(a, _)| a).to_owned())
        .collect()
}

/// The pixel at `x`, `y` of `t`, as the drawn list shows a first pixel.
fn hex(t: &Texture, x: u32, y: u32) -> String {
    let i = ((y * t.w + x) * 4) as usize;
    t.rgba[i..i + 4].iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn a_sheets_icons_are_cut_in_the_originals_pixels() {
    // A 256-pixel sheet whose thumbnail (700) is kept, its resolution
    // worked out: the script's crops, in the original's pixels, cut the
    // thumbnail where they mean, each icon drawn from a part of its own at
    // the thumbnail's resolution (not the whole sheet scaled to the atlas).
    let mut game = FakeGame::new("rbx-sheet");
    let mut st = studio(&mut game);
    let assets = st.s.rbx.workspace_dir.join(".roblox-assets");
    std::fs::create_dir_all(&assets).unwrap();
    let s = sheet(16, 16, 7);
    let (thumb, _) = thumbnails(&s);
    std::fs::write(assets.join("483448923.png"), png(&thumb)).unwrap();
    std::fs::write(assets.join("483448923.size"), "256").unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        for i, at in ipairs({Vector2.new(0, 0), Vector2.new(32, 16), Vector2.new(240, 240)}) do
            local icon = Instance.new("ImageLabel", gui)
            icon.Size = UDim2.fromOffset(16, 16)
            icon.Position = UDim2.fromOffset(20 * i, 0)
            icon.BackgroundTransparency = 1
            icon.Image = "rbxassetid://483448923"
            icon.ImageRectOffset = at
            icon.ImageRectSize = Vector2.new(16, 16)
        end
    "#;
    load(&mut game, &mut st, "sheet", src);
    let k = 700.0 / 256.0;
    let drawn = images(&mut st, &mut game);
    assert_eq!(drawn.len(), 3, "{drawn:#?}");
    for (n, (x, y)) in [(0.0, 0.0), (32.0, 16.0), (240.0, 240.0)].into_iter().enumerate() {
        // The part taken out: the whole pixels the icon covers.
        let (x0, y0) = ((x * k as f32).floor(), (y * k as f32).floor());
        let x1 = ((x + 16.0) * k as f32).ceil().min(700.0);
        let y1 = ((y + 16.0) * k as f32).ceil().min(700.0);
        let want = format!(
            "{}x{} #{} [{:?}, 0.0, 16.0, 16.0] from {:?}",
            x1 - x0,
            y1 - y0,
            hex(&thumb, x0 as u32, y0 as u32),
            20.0 * (n + 1) as f32,
            [x * k as f32 - x0, y * k as f32 - y0, 16.0 * k as f32, 16.0 * k as f32],
        );
        assert_eq!(drawn[n], want);
    }
}

#[test]
fn a_strip_of_icons_draws_each_whole() {
    // Roblox's ClassImages.PNG, a strip of 16-pixel icons 2352 wide, kept
    // where the fetch puts rbxasset files: an icon cut from it is itself,
    // pixel for pixel, though the strip is larger than the atlas takes.
    let mut game = FakeGame::new("rbx-strip");
    let mut st = studio(&mut game);
    let dir = st.s.rbx.workspace_dir.join(".roblox-assets/rbxasset/textures");
    std::fs::create_dir_all(&dir).unwrap();
    let mut strip = Texture {
        w: 2352,
        h: 16,
        rgba: vec![0; 2352 * 16 * 4],
        key: 0,
        version: 0,
    };
    for (i, p) in strip.rgba.chunks_mut(4).enumerate() {
        let icon = (i % 2352 / 16) as u8;
        p.copy_from_slice(&[icon, 255 - icon, 7, 255]);
    }
    std::fs::write(dir.join("ClassImages.PNG"), png(&strip)).unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local icon = Instance.new("ImageLabel", gui)
        icon.Size = UDim2.fromOffset(16, 16)
        icon.BackgroundTransparency = 1
        icon.Image = "rbxasset://textures/ClassImages.PNG"
        icon.ImageRectOffset = Vector2.new(19 * 16, 0)
        icon.ImageRectSize = Vector2.new(16, 16)
    "#;
    load(&mut game, &mut st, "strip", src);
    assert_eq!(
        images(&mut st, &mut game),
        ["16x16 #13ec07ff [0.0, 0.0, 16.0, 16.0] from [0.0, 0.0, 16.0, 16.0]"]
    );
}

#[test]
fn a_thumbnail_draws_whole() {
    // Thumbnails are 700 pixels a side, past what a mod's texture may be:
    // the GUI draws them all the same (from a copy scaled to the atlas).
    let mut game = FakeGame::new("rbx-thumb");
    let mut st = studio(&mut game);
    let assets = st.s.rbx.workspace_dir.join(".roblox-assets");
    std::fs::create_dir_all(&assets).unwrap();
    let (thumb, _) = thumbnails(&sheet(16, 16, 9));
    std::fs::write(assets.join("5.png"), png(&thumb)).unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local image = Instance.new("ImageLabel", gui)
        image.Size = UDim2.fromOffset(100, 100)
        image.Image = "rbxassetid://5"
        image.Name = "Thumb"
        local other = Instance.new("ImageLabel", gui)
        other.Image = "rbxassetid://6"
        other.Visible = false -- not drawn, so not fetched
    "#;
    load(&mut game, &mut st, "thumb", src);
    let drawn = images(&mut st, &mut game);
    assert_eq!(drawn.len(), 1, "{drawn:#?}");
    assert!(
        drawn[0].starts_with("256x256 #")
            && drawn[0].ends_with("[0.0, 0.0, 100.0, 100.0] from [0.0, 0.0, 256.0, 256.0]"),
        "{drawn:#?}"
    );
    // Loaded once it is kept; the other (never drawn, so never fetched)
    // isn't.
    let out = st.run_console(
        &mut game,
        "local g = game.Players.LocalPlayer.PlayerGui.ScreenGui return g.Thumb.IsLoaded, g.ImageLabel.IsLoaded",
    );
    assert_eq!(out.last().map(|(_, t)| t.as_str()), Some("true, false"));
}
