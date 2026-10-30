// The studio's looks: themes (palettes, and for the executor styles a layout
// of their own), and what the player chose in Settings: the theme, full
// screen or a window (moved and sized by hand), the accent, the size of
// everything, the backdrop, the code's size and the motion. Kept in
// `look.txt` in the studio's folder.

use std::path::Path;

/// Colours, 0xAARRGGBB, and how round corners are (1 the design system's).
/// The executor styles use them their own way: page is the window's body,
/// surface its title bar, plate_soft the editor, plate and rule a button
/// and its hover (or edge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub page: u32,
    pub surface: u32,
    pub plate_soft: u32,
    pub plate: u32,
    pub rule: u32,
    pub ink: u32,
    pub ink_soft: u32,
    pub muted: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub danger: u32,
    pub success: u32,
    pub keyword: u32,
    pub string: u32,
    pub number: u32,
    pub comment: u32,
    pub global: u32,
    pub round: f32,
}

/// How a theme lays the studio out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// The studio's own: tabs, full screen or in a window.
    Studio,
    /// After the executors of old Roblox days: a window of their shape.
    Synapse,
    ProtoSmasher,
    ProtoClassic,
    Rc7,
}

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub layout: Layout,
    pub palette: Palette,
}

#[allow(clippy::too_many_arguments)]
const fn studio(
    page: u32,
    surface: u32,
    plate_soft: u32,
    plate: u32,
    rule: u32,
    ink: u32,
    ink_soft: u32,
    muted: u32,
    accent: u32,
    accent_hover: u32,
) -> Palette {
    Palette {
        page,
        surface,
        plate_soft,
        plate,
        rule,
        ink,
        ink_soft,
        muted,
        accent,
        accent_hover,
        danger: 0xFFF0_7A68,
        success: 0xFF45_C27A,
        keyword: 0xFFB4_A7FF,
        string: 0xFF8F_D6A8,
        number: 0xFFF2_C48D,
        comment: 0xFF6E_7280,
        global: 0xFF9F_C2FF,
        round: 1.0,
    }
}

/// Code colours on a light editor.
const fn light_code(mut p: Palette) -> Palette {
    p.keyword = 0xFF6D_4AE0;
    p.string = 0xFF1A_7F4B;
    p.number = 0xFFB1_5C00;
    p.comment = 0xFF8A_8F99;
    p.global = 0xFF1F_6FC9;
    p.danger = 0xFFD0_3B2A;
    p.success = 0xFF1E_8E4E;
    p
}

/// Visual Studio Code's dark colours, as the executors' editors had.
const fn vs_code(mut p: Palette) -> Palette {
    p.keyword = 0xFF56_9CD6;
    p.string = 0xFFCE_9178;
    p.number = 0xFFB5_CEA8;
    p.comment = 0xFF6A_9955;
    p.global = 0xFF4E_C9B0;
    p
}

const fn square(mut p: Palette, round: f32) -> Palette {
    p.round = round;
    p
}

pub const THEMES: &[Theme] = &[
    Theme {
        id: "horizon",
        name: "Horizon",
        layout: Layout::Studio,
        palette: studio(
            0xFF0D_0E13,
            0xFF16_181F,
            0xFF18_1A22,
            0xFF20_222C,
            0xFF24_262F,
            0xFFEC_EDF3,
            0xFFC5_C7D1,
            0xFF8B_8F9C,
            0xFF9A_86FF,
            0xFFB0_A0FF,
        ),
    },
    Theme {
        id: "light",
        name: "Horizon Light",
        layout: Layout::Studio,
        palette: light_code(studio(
            0xFFF6_F7FA,
            0xFFFF_FFFF,
            0xFFEE_F0F4,
            0xFFE4_E7ED,
            0xFFD9_DCE3,
            0xFF15_171D,
            0xFF3A_3E4A,
            0xFF6C_7180,
            0xFF6D_4AE0,
            0xFF83_66F0,
        )),
    },
    Theme {
        id: "midnight",
        name: "Midnight",
        layout: Layout::Studio,
        palette: studio(
            0xFF00_0000,
            0xFF0A_0A0C,
            0xFF0E_0F12,
            0xFF16_171B,
            0xFF1C_1E24,
            0xFFF2_F3F7,
            0xFFC0_C3CC,
            0xFF7D_818C,
            0xFF4D_8DFF,
            0xFF6E_A2FF,
        ),
    },
    Theme {
        id: "nord",
        name: "Nord",
        layout: Layout::Studio,
        palette: studio(
            0xFF24_2933,
            0xFF2B_303B,
            0xFF2E_3440,
            0xFF3B_4252,
            0xFF43_4C5E,
            0xFFEC_EFF4,
            0xFFD8_DEE9,
            0xFF8F_9BB0,
            0xFF88_C0D0,
            0xFF8F_BCBB,
        ),
    },
    Theme {
        id: "rose",
        name: "Rose",
        layout: Layout::Studio,
        palette: studio(
            0xFF19_1418,
            0xFF22_1A20,
            0xFF26_1D24,
            0xFF31_252E,
            0xFF3A_2C36,
            0xFFF6_EAF0,
            0xFFD9_C6CF,
            0xFF9E_8A94,
            0xFFF2_7FAE,
            0xFFF7_9CC0,
        ),
    },
    Theme {
        id: "forest",
        name: "Forest",
        layout: Layout::Studio,
        palette: studio(
            0xFF10_1612,
            0xFF16_1E19,
            0xFF19_221C,
            0xFF20_2B24,
            0xFF27_342C,
            0xFFE9_F2EC,
            0xFFC4_D3C9,
            0xFF86_9A8D,
            0xFF5F_D38D,
            0xFF7F_E0A4,
        ),
    },
    Theme {
        id: "solar",
        name: "Solar",
        layout: Layout::Studio,
        palette: studio(
            0xFF00_202A,
            0xFF03_2A36,
            0xFF07_303D,
            0xFF0B_3946,
            0xFF12_4352,
            0xFFEE_E8D5,
            0xFFC9_C3AE,
            0xFF83_9496,
            0xFFE0_A526,
            0xFFEC_BB4B,
        ),
    },
    Theme {
        id: "synapse",
        name: "Synapse X style",
        layout: Layout::Synapse,
        palette: square(
            vs_code(studio(
                0xFF2E_2E2E,
                0xFF20_2020,
                0xFF1E_1E1E,
                0xFF3C_3C3C,
                0xFF50_5050,
                0xFFFF_FFFF,
                0xFFDA_DADA,
                0xFF85_8585,
                0xFF6A_3FE0,
                0xFF83_5CF0,
            )),
            0.15,
        ),
    },
    Theme {
        id: "protosmasher",
        name: "ProtoSmasher style",
        layout: Layout::ProtoSmasher,
        palette: {
            let mut p = square(
                studio(
                    0xFF2B_2B2B,
                    0xFFFF_FFFF,
                    0xFF1E_1E1E,
                    0xFF3A_3A3A,
                    0xFF4F_4F4F,
                    0xFFF2_F2F2,
                    0xFFCC_CCCC,
                    0xFF96_9696,
                    0xFF6A_8CFF,
                    0xFF8A_A6FF,
                ),
                0.0,
            );
            p.keyword = 0xFFF0_7178;
            p.string = 0xFFC3_E88D;
            p.number = 0xFFF7_8C6C;
            p.comment = 0xFF9E_7C7C;
            p.global = 0xFF82_AAFF;
            p.success = 0xFF6B_CF6B;
            p
        },
    },
    Theme {
        id: "protoclassic",
        name: "ProtoSmasher classic",
        layout: Layout::ProtoClassic,
        palette: square(
            vs_code(studio(
                0xFF26_2626,
                0xFFFF_FFFF,
                0xFF1A_1A1A,
                0xFFE1_E1E1,
                0xFFAD_ADAD,
                0xFFFF_FFFF,
                0xFFDD_DDDD,
                0xFF9A_9A9A,
                0xFF4F_7BE0,
                0xFF6A_90F0,
            )),
            0.0,
        ),
    },
    Theme {
        id: "rc7",
        name: "RC7 style",
        layout: Layout::Rc7,
        palette: square(
            light_code(studio(
                0xFF6B_2324,
                0xFFFF_FFFF,
                0xFFFF_FFFF,
                0xFF8C_2A2C,
                0xFF4A_1415,
                0xFF00_0000,
                0xFF22_2222,
                0xFF6E_6E6E,
                0xFF3A_8FD8,
                0xFF5A_A3E0,
            )),
            0.0,
        ),
    },
];

/// Accents to pick instead of the theme's (0 is the theme's own).
pub const ACCENTS: &[u32] = &[
    0,
    0xFF9A_86FF,
    0xFF4D_8DFF,
    0xFF2E_C4B6,
    0xFF45_C27A,
    0xFFE0_A526,
    0xFFF0_7A68,
    0xFFF2_7FAE,
    0xFFEC_EDF3,
];

/// The settings, as kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub theme: usize,
    /// In a window (the studio's layout; the executor styles always are).
    pub window: bool,
    /// An index into `ACCENTS` (0: the theme's).
    pub accent: usize,
    /// Everything's size, 0.75 to 1.5.
    pub scale: f32,
    /// How much the backdrop hides the game (full screen), 0.3 to 1.
    pub opacity: f32,
    /// The code's size in the editor and console, 11 to 20.
    pub code: f32,
    pub motion: bool,
    /// The window, in the studio's units (dp at scale 1), once moved.
    pub rect: Option<[f32; 4]>,
    pub maximized: bool,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            theme: 0,
            window: false,
            accent: 0,
            scale: 1.0,
            opacity: 0.92,
            code: 13.0,
            motion: true,
            rect: None,
            maximized: false,
        }
    }
}

/// A window `r` kept on a `w` x `h` screen: no bigger than it (and no
/// smaller than a window can be, room allowing), and all of it on it, so
/// a smaller screen (the game's window sized down) brings it along.
pub fn fit(r: [f32; 4], w: f32, h: f32) -> [f32; 4] {
    let rw = r[2].max(320.0).min(w.max(1.0));
    let rh = r[3].max(220.0).min(h.max(1.0));
    [
        r[0].clamp(0.0, (w - rw).max(0.0)),
        r[1].clamp(0.0, (h - rh).max(0.0)),
        rw,
        rh,
    ]
}

impl Look {
    pub fn theme(&self) -> &'static Theme {
        &THEMES[self.theme.min(THEMES.len() - 1)]
    }

    /// The theme's palette with the chosen accent.
    pub fn palette(&self) -> Palette {
        let mut p = self.theme().palette;
        if let Some(&a) = ACCENTS.get(self.accent)
            && a != 0
        {
            p.accent = a;
            p.accent_hover = mix(a, 0xFFFF_FFFF, 0.18);
        }
        p
    }

    /// Whether the studio shows in a window.
    pub fn windowed(&self) -> bool {
        self.window || self.theme().layout != Layout::Studio
    }

    /// The window in a screen `w` x `h` (studio units): kept on screen, or a
    /// theme's own size in the middle the first time.
    pub fn window_rect(&self, w: f32, h: f32) -> [f32; 4] {
        if self.maximized {
            return [0.0, 0.0, w, h];
        }
        let (dw, dh) = match self.theme().layout {
            Layout::Studio => ((w * 0.78).clamp(560.0, 1180.0), (h * 0.8).clamp(380.0, 760.0)),
            Layout::Synapse => (820.0, 360.0),
            Layout::ProtoSmasher => (470.0, 560.0),
            Layout::ProtoClassic => (400.0, 530.0),
            Layout::Rc7 => (520.0, 470.0),
        };
        let r = self.rect.unwrap_or([(w - dw) / 2.0, (h - dh) / 2.0, dw, dh]);
        fit(r, w, h)
    }

    pub fn load(dir: &Path) -> Look {
        let mut look = Look::default();
        let Ok(text) = std::fs::read_to_string(dir.join("look.txt")) else {
            return look;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let num = |d: f32| v.parse::<f32>().ok().filter(|n| n.is_finite()).unwrap_or(d);
            match k {
                "theme" => look.theme = THEMES.iter().position(|t| t.id == v).unwrap_or(0),
                "window" => look.window = v == "true",
                "accent" => look.accent = v.parse().unwrap_or(0).min(ACCENTS.len() - 1),
                "scale" => look.scale = num(1.0).clamp(0.75, 1.5),
                "opacity" => look.opacity = num(0.92).clamp(0.3, 1.0),
                "code" => look.code = num(13.0).clamp(11.0, 20.0),
                "motion" => look.motion = v != "false",
                "maximized" => look.maximized = v == "true",
                "rect" => {
                    let n: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    if let [x, y, w, h] = n[..] {
                        look.rect = Some([x, y, w, h]);
                    }
                }
                _ => {}
            }
        }
        look
    }

    pub fn save(&self, dir: &Path) {
        let mut text = format!(
            "theme={}\nwindow={}\naccent={}\nscale={}\nopacity={}\ncode={}\nmotion={}\nmaximized={}\n",
            self.theme().id,
            self.window,
            self.accent,
            self.scale,
            self.opacity,
            self.code,
            self.motion,
            self.maximized
        );
        if let Some([x, y, w, h]) = self.rect {
            text.push_str(&format!("rect={x:.0},{y:.0},{w:.0},{h:.0}\n"));
        }
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join("look.txt"), text);
    }
}

pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}
