// ViaHorizon on the multiplayer screen: a button in the header's top right
// corner, as vanilla's buttons look, naming the version servers are joined
// as; a click opens the version picker (`hstudio_via_picker`) over the
// screen. Drawn after the title screen, in the game and in the startup menu;
// its input comes first from the window hook, which gives the picker the
// mouse and the keys while it is open.

use super::hstudio_via_picker::{Act, Info, Key, Paint, Picker, Tone};
use horizon_client::hud::{HudAtlas, HudBatch};
use horizon_client::multiplayer::Screen as Multiplayer;
use horizon_client::net::hstudio_via as via;
use horizon_client::title::{Mode, Title};
use horizon_client::ui_theme::hstudio_paint as paint;
use std::cell::RefCell;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The button's width and its place below the header's top, in GUI pixels.
const BUTTON_W: f32 = 112.0;
const BUTTON_TOP: f32 = 6.0;

struct Menu {
    start: Instant,
    /// When the button was last drawn: the multiplayer list shows.
    drawn: Option<Instant>,
    /// Physical pixels and dp per GUI pixel, from the last frame.
    scale: f32,
    dp: f32,
    /// The button, and the pointer, in GUI pixels.
    button: [f32; 4],
    pointer: (f32, f32),
    picker: Picker,
    /// A press the game didn't see: its release isn't the game's either.
    swallow_release: bool,
}

thread_local! {
    static MENU: RefCell<Menu> = RefCell::new(Menu {
        start: Instant::now(),
        drawn: None,
        scale: 1.0,
        dp: 1.0,
        button: [0.0; 4],
        pointer: (-1.0, -1.0),
        picker: Picker::default(),
        swallow_release: false,
    });
}

fn seconds(m: &Menu) -> f64 {
    m.start.elapsed().as_secs_f64()
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn info() -> Info {
    let s = via::settings();
    Info {
        enabled: s.enabled,
        version: s.version,
        status: via::status_text(),
        tone: match via::status() {
            via::Status::Ready(_) => Tone::Ready,
            via::Status::Failed(_) => Tone::Failed,
            via::Status::Off => Tone::Off,
            _ => Tone::Busy,
        },
        versions: via::VERSIONS.iter().map(|v| ((*v).to_owned(), via::label(v))).collect(),
    }
}

/// `0xAARRGGBB` in the HUD's order (`0xAABBGGRR`).
fn hud(argb: u32) -> u32 {
    (argb & 0xFF00_FF00) | ((argb >> 16) & 0xFF) | ((argb & 0xFF) << 16)
}

struct Painter<'a> {
    atlas: &'a HudAtlas,
    batch: &'a mut HudBatch,
}

fn font(medium: bool) -> paint::Font {
    if medium { paint::Font::Medium } else { paint::Font::Regular }
}

impl Paint for Painter<'_> {
    fn rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        paint::rect(self.atlas, self.batch, r, radius, hud(argb));
    }
    fn border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        paint::border(self.atlas, self.batch, r, radius, hud(argb));
    }
    fn text(&mut self, s: &str, x: f32, y: f32, size: f32, medium: bool, argb: u32) -> f32 {
        paint::text(self.atlas, self.batch, font(medium), size, x, y, hud(argb), s)
    }
    fn measure(&mut self, s: &str, size: f32, medium: bool) -> f32 {
        paint::measure(self.atlas, self.batch, font(medium), size, s)
    }
}

/// The button's words: the version (or that ViaHorizon is off).
fn button_label(info: &Info) -> String {
    if !info.enabled {
        return "ViaHorizon: Off".into();
    }
    match info.version.as_str() {
        "" | "auto" => "Via: Auto detect".into(),
        v => format!("Via: {v}"),
    }
}

/// A button as vanilla's look (its sprite's halves), with a state dot.
fn vanilla_button(atlas: &HudAtlas, batch: &mut HudBatch, b: [f32; 4], label: &str, hover: bool, dot: u32) {
    let sprite = if hover {
        "gui/sprites/widget/button_highlighted"
    } else {
        "gui/sprites/widget/button"
    };
    if let Some(r) = atlas.sprites.get(sprite).copied() {
        let half = b[2] / 2.0;
        let mut left = r;
        left.u1 = r.u0 + (r.u1 - r.u0) * (half / r.w as f32);
        batch.rect_uv(b[0], b[1], half, 20.0, left, u32::MAX);
        let mut right = r;
        right.u0 = r.u1 - (r.u1 - r.u0) * (half / r.w as f32);
        batch.rect_uv(b[0] + half, b[1], half, 20.0, right, u32::MAX);
    }
    let mut label = label.to_owned();
    while label.len() > 4 && HudBatch::text_width(atlas, &label) > b[2] - 18.0 {
        label.pop();
    }
    let tw = HudBatch::text_width(atlas, &label).ceil();
    let x = (b[0] + (b[2] - tw - 6.0) / 2.0).floor();
    batch.fill(atlas, x, b[1] + 8.0, 3.0, 3.0, hud(dot));
    batch.text(atlas, &label, x + 6.0, b[1] + 6.0, u32::MAX, true);
}

/// After the title screen draws: on the multiplayer list, the button and,
/// once clicked, the picker.
pub(super) fn draw(atlas: &HudAtlas, batch: &mut HudBatch, title: Option<&Title>, cursor: (f32, f32)) {
    let listing = title.is_some_and(|t| matches!(t.mode, Mode::Multiplayer(Multiplayer::List { .. })));
    MENU.with(|cell| {
        let m = &mut *cell.borrow_mut();
        let now = seconds(m);
        if !listing {
            m.drawn = None;
            m.picker.hide(now);
            return;
        }
        m.drawn = Some(Instant::now());
        m.scale = batch.scale.max(0.01);
        let (dw, dh, dp) = paint::size(atlas, batch);
        m.dp = dp;
        let w = batch.width.ceil();
        let b = [(w - BUTTON_W - 6.0).floor(), BUTTON_TOP, BUTTON_W, 20.0];
        m.button = b;
        let info = info();
        let label = button_label(&info);
        let picker = m.picker.visible(now);
        let hover = !picker && inside(b, cursor);
        horizon_client::widgets::record(&label, b, true);
        horizon_client::widgets::widget_cursor(hover, true);
        let dot = match info.tone {
            Tone::Ready => 0xFF45_C27A,
            Tone::Busy => 0xFFF2_C48D,
            Tone::Failed => 0xFFF0_7A68,
            Tone::Off => 0xFF8B_8F9C,
        };
        vanilla_button(atlas, batch, b, &label, hover, dot);
        if picker {
            let mut p = Painter { atlas, batch };
            m.picker.draw(&mut p, &info, dw, dh, now);
            horizon_client::widgets::widget_cursor(m.picker.pointing(), true);
            paint::flush(atlas, batch);
        }
    });
}

fn apply(act: Act) {
    match act {
        Act::Toggle => via::set_enabled(!via::settings().enabled),
        Act::Pick(id) => {
            let _ = via::set_version(&id);
        }
        Act::None => {}
    }
}

fn function_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::F1
            | KeyCode::F2
            | KeyCode::F3
            | KeyCode::F4
            | KeyCode::F5
            | KeyCode::F6
            | KeyCode::F7
            | KeyCode::F8
            | KeyCode::F9
            | KeyCode::F10
            | KeyCode::F11
            | KeyCode::F12
    )
}

/// A window event, before the game sees it: whether ViaHorizon took it (the
/// game then gets one it ignores). The pointer always goes on to the game.
pub(super) fn input(event: &mut WindowEvent) -> bool {
    let taken = MENU.with(|cell| {
        let m = &mut *cell.borrow_mut();
        let now = seconds(m);
        // Only while the multiplayer list was drawn a moment ago.
        if !m.drawn.is_some_and(|t| t.elapsed().as_millis() < 500) {
            m.picker.hide(now);
            m.swallow_release = false;
            return false;
        }
        let open = m.picker.is_open();
        match &*event {
            WindowEvent::CursorMoved { position, .. } => {
                m.pointer = (position.x as f32 / m.scale, position.y as f32 / m.scale);
                if open {
                    m.picker.point((m.pointer.0 * m.dp, m.pointer.1 * m.dp));
                }
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *state == ElementState::Released {
                    return std::mem::take(&mut m.swallow_release) || open;
                }
                if open {
                    m.swallow_release = true;
                    if *button == MouseButton::Left {
                        apply(m.picker.click(now, &info()));
                    }
                    return true;
                }
                if *button == MouseButton::Left && inside(m.button, m.pointer) {
                    m.swallow_release = true;
                    m.picker.show(now, &info());
                    m.picker.point((m.pointer.0 * m.dp, m.pointer.1 * m.dp));
                    return true;
                }
                false
            }
            WindowEvent::MouseWheel { delta, .. } if open => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 30.0) as f32,
                };
                m.picker.wheel(steps, &info());
                true
            }
            WindowEvent::KeyboardInput { event: k, .. } if open => {
                if let PhysicalKey::Code(code) = k.physical_key
                    && function_key(code)
                {
                    return false;
                }
                if k.state == ElementState::Pressed {
                    let key = match k.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => Some(Key::Escape),
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(Key::Enter),
                        PhysicalKey::Code(KeyCode::ArrowUp) => Some(Key::Up),
                        PhysicalKey::Code(KeyCode::ArrowDown) => Some(Key::Down),
                        PhysicalKey::Code(KeyCode::Backspace) => Some(Key::Backspace),
                        _ => k
                            .text
                            .as_ref()
                            .map(|t| t.chars().filter(|c| !c.is_control()).collect::<String>())
                            .filter(|t| !t.is_empty())
                            .map(Key::Text),
                    };
                    if let Some(key) = key {
                        apply(m.picker.key(now, key, &info()));
                    }
                }
                true
            }
            _ => false,
        }
    });
    if taken {
        *event = WindowEvent::HoveredFileCancelled;
    }
    taken
}
