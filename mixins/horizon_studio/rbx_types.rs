// Roblox's data types for the Roblox emulation layer (hstudio_rbx_types), in
// plain Rust: Vector2, Vector3, CFrame, Color3, UDim and UDim2, Rect,
// NumberRange, NumberSequence and ColorSequence, TweenInfo, BrickColor (the
// whole palette), Font, Random, and TweenService's easing curves. The Lua
// bindings wrap these later; nothing here knows about the VM.
//
// Roblox keeps every component as a float32. These keep f64, so chains of
// math (CFrame products, Euler round trips) lose less, and apply float32
// where it shows: `==` compares the float32 values (0.1 + 0.2 equals 0.3, as
// in Roblox), Display prints them as `tostring` does (0.1 prints
// 0.100000001), sequence keypoint times are checked as float32, and
// UDim.Offset is an int32. Still different: reading a component gives the
// f64 (Roblox gives 0.10000000149011612 for 0.1; `float32` rounds one for a
// binding that wants that), and results float32 math changes, such as
// cos(pi/2) in a CFrame (6.1e-17 here, -4.37e-08 in Roblox).

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::fmt;
use std::hash::{BuildHasher, RandomState};
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------- numbers

/// The float32 Roblox stores for `x`, back as an f64.
pub fn float32(x: f64) -> f64 {
    x as f32 as f64
}

/// Whether two numbers are the same float32.
fn same32(a: f64, b: f64) -> bool {
    float32(a) == float32(b)
}

/// `x` as C's `printf("%.<digits>g")` writes it: "0.100000001", "1e+20",
/// "-4.37113883e-08"; "-0" keeps its sign, and "nan", "inf" and "-inf".
fn format_g(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let p = digits.max(1);
    let sci = format!("{:.*e}", p - 1, x);
    let Some((mantissa, exp)) = sci.split_once('e') else {
        return sci;
    };
    let exp: i32 = exp.parse().unwrap_or(0);
    if exp < -4 || exp >= p as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim_zeros(mantissa), exp.unsigned_abs())
    } else {
        trim_zeros(&format!("{:.*}", (p as i32 - 1 - exp) as usize, x))
    }
}

/// A decimal without its trailing zeros (and point): "1.500" is "1.5".
fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// A component as most types print it: its float32, to 9 significant digits.
fn num9(x: f64) -> String {
    format_g(float32(x), 9)
}

/// A component as Color3, NumberRange and the sequences print it: 6 digits.
fn num6(x: f64) -> String {
    format_g(float32(x), 6)
}

/// The sign of `x` as Roblox's Sign gives it: -1, 0 or 1 (0 for NaN).
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Close enough for FuzzyEq: within `eps`, scaled up for large numbers.
fn fuzzy(a: f64, b: f64, eps: f64) -> bool {
    a == b || (a - b).abs() <= eps * 1f64.max(a.abs()).max(b.abs())
}

// ---------------------------------------------------------------- enums

/// An enum with Roblox's item names and values.
macro_rules! rbx_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every item, in order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Its Name.
            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => stringify!($variant)),+
                }
            }

            /// Its Value.
            pub fn value(self) -> i32 {
                match self {
                    $($name::$variant => $value),+
                }
            }

            /// The item with exactly this name.
            pub fn from_name(name: &str) -> Option<$name> {
                Self::ALL.iter().copied().find(|v| v.name() == name)
            }

            /// The item with this value.
            pub fn from_value(value: i32) -> Option<$name> {
                Self::ALL.iter().copied().find(|v| v.value() == value)
            }
        }
    };
}

rbx_enum! {
    /// Enum.EasingStyle, without Bezier (11) and Spring (12): TweenInfo.new
    /// and TweenService:GetValue refuse those, so a binding errors on them.
    EasingStyle {
        Linear = 0, Sine = 1, Back = 2, Quad = 3, Quart = 4, Quint = 5, Bounce = 6, Elastic = 7,
        Exponential = 8, Circular = 9, Cubic = 10,
    }
}

rbx_enum! {
    /// Enum.EasingDirection.
    EasingDirection { In = 0, Out = 1, InOut = 2 }
}

rbx_enum! {
    /// Enum.RotationOrder: the order CFrame.fromEulerAngles applies its axes in.
    #[allow(clippy::upper_case_acronyms)]
    RotationOrder { XYZ = 0, XZY = 1, YZX = 2, YXZ = 3, ZXY = 4, ZYX = 5 }
}

rbx_enum! {
    /// Enum.NormalId: the faces of a part.
    NormalId { Right = 0, Top = 1, Back = 2, Left = 3, Bottom = 4, Front = 5 }
}

rbx_enum! {
    /// Enum.Axis.
    Axis { X = 0, Y = 1, Z = 2 }
}

rbx_enum! {
    /// Enum.FontWeight.
    FontWeight {
        Thin = 100, ExtraLight = 200, Light = 300, Regular = 400, Medium = 500, SemiBold = 600, Bold = 700,
        ExtraBold = 800, Heavy = 900,
    }
}

rbx_enum! {
    /// Enum.FontStyle.
    FontStyle { Normal = 0, Italic = 1 }
}

rbx_enum! {
    /// Enum.Font, the older way to name a font; `Font::from_enum` maps it.
    FontEnum {
        Legacy = 0, Arial = 1, ArialBold = 2, SourceSans = 3, SourceSansBold = 4, SourceSansLight = 5,
        SourceSansItalic = 6, Bodoni = 7, Garamond = 8, Cartoon = 9, Code = 10, Highway = 11, SciFi = 12,
        Arcade = 13, Fantasy = 14, Antique = 15, SourceSansSemibold = 16, Gotham = 17, GothamMedium = 18,
        GothamBold = 19, GothamBlack = 20, AmaticSC = 21, Bangers = 22, Creepster = 23, DenkOne = 24,
        Fondamento = 25, FredokaOne = 26, GrenzeGotisch = 27, IndieFlower = 28, JosefinSans = 29, Jura = 30,
        Kalam = 31, LuckiestGuy = 32, Merriweather = 33, Michroma = 34, Nunito = 35, Oswald = 36,
        PatrickHand = 37, PermanentMarker = 38, Roboto = 39, RobotoCondensed = 40, RobotoMono = 41,
        Sarpanch = 42, SpecialElite = 43, TitilliumWeb = 44, Ubuntu = 45, BuilderSans = 46,
        BuilderSansMedium = 47, BuilderSansBold = 48, BuilderSansExtraBold = 49, Arimo = 50, ArimoBold = 51,
        Unknown = 100,
    }
}

impl FontEnum {
    /// By name, taking the legacy names too: GothamSemibold is GothamMedium,
    /// and the Montserrat names are the Gotham ones.
    pub fn from_name_or_alias(name: &str) -> Option<FontEnum> {
        let name = match name {
            "Montserrat" => "Gotham",
            "GothamSemibold" | "MontserratMedium" => "GothamMedium",
            "MontserratBold" => "GothamBold",
            "MontserratBlack" => "GothamBlack",
            n => n,
        };
        FontEnum::from_name(name)
    }

    /// The face Font.fromEnum gives it: a family file, weight and style.
    fn face(self) -> Option<(&'static str, FontWeight, FontStyle)> {
        use FontEnum::*;
        use FontWeight::{Bold, ExtraBold, Heavy, Light, Medium, Regular, SemiBold};
        let n = FontStyle::Normal;
        Some(match self {
            Legacy | Arial => ("LegacyArial", Regular, n),
            ArialBold => ("LegacyArial", Bold, n),
            SourceSans => ("SourceSansPro", Regular, n),
            SourceSansBold => ("SourceSansPro", Bold, n),
            SourceSansLight => ("SourceSansPro", Light, n),
            SourceSansItalic => ("SourceSansPro", Regular, FontStyle::Italic),
            SourceSansSemibold => ("SourceSansPro", SemiBold, n),
            Bodoni => ("AccanthisADFStd", Regular, n),
            Garamond => ("Guru", Regular, n),
            Cartoon => ("ComicNeueAngular", Regular, n),
            Code => ("Inconsolata", Regular, n),
            Highway => ("HighwayGothic", Regular, n),
            SciFi => ("Zekton", Regular, n),
            Arcade => ("PressStart2P", Regular, n),
            Fantasy => ("Balthazar", Regular, n),
            Antique => ("RomanAntique", Regular, n),
            Gotham => ("Montserrat", Regular, n),
            GothamMedium => ("Montserrat", Medium, n),
            GothamBold => ("Montserrat", Bold, n),
            GothamBlack => ("Montserrat", Heavy, n),
            BuilderSans => ("BuilderSans", Regular, n),
            BuilderSansMedium => ("BuilderSans", Medium, n),
            BuilderSansBold => ("BuilderSans", Bold, n),
            BuilderSansExtraBold => ("BuilderSans", ExtraBold, n),
            Arimo => ("Arimo", Regular, n),
            ArimoBold => ("Arimo", Bold, n),
            Unknown => return None,
            // The rest are families of their own name.
            other => (other.name(), Regular, n),
        })
    }
}

// ---------------------------------------------------------------- vectors

/// The arithmetic Vector2 and Vector3 share.
macro_rules! vector_common {
    ($t:ident { $($f:ident),+ }) => {
        impl Add for $t {
            type Output = $t;
            fn add(self, o: $t) -> $t {
                $t { $($f: self.$f + o.$f),+ }
            }
        }

        impl Sub for $t {
            type Output = $t;
            fn sub(self, o: $t) -> $t {
                $t { $($f: self.$f - o.$f),+ }
            }
        }

        impl Neg for $t {
            type Output = $t;
            fn neg(self) -> $t {
                $t { $($f: -self.$f),+ }
            }
        }

        /// Component by component.
        impl Mul for $t {
            type Output = $t;
            fn mul(self, o: $t) -> $t {
                $t { $($f: self.$f * o.$f),+ }
            }
        }

        impl Mul<f64> for $t {
            type Output = $t;
            fn mul(self, n: f64) -> $t {
                $t { $($f: self.$f * n),+ }
            }
        }

        impl Mul<$t> for f64 {
            type Output = $t;
            fn mul(self, v: $t) -> $t {
                $t { $($f: self * v.$f),+ }
            }
        }

        /// Component by component.
        impl Div for $t {
            type Output = $t;
            fn div(self, o: $t) -> $t {
                $t { $($f: self.$f / o.$f),+ }
            }
        }

        impl Div<f64> for $t {
            type Output = $t;
            fn div(self, n: f64) -> $t {
                $t { $($f: self.$f / n),+ }
            }
        }

        /// Equal as float32, as Roblox compares them.
        impl PartialEq for $t {
            fn eq(&self, o: &$t) -> bool {
                $(same32(self.$f, o.$f))&&+
            }
        }

        impl $t {
            /// Its length.
            pub fn magnitude(self) -> f64 {
                self.dot(self).sqrt()
            }

            /// It scaled to length 1; the zero vector gives NaN components.
            pub fn unit(self) -> $t {
                self / self.magnitude()
            }

            /// The dot product.
            pub fn dot(self, o: $t) -> f64 {
                0.0 $(+ self.$f * o.$f)+
            }

            /// `self + (goal - self) * alpha`; alpha isn't clamped.
            pub fn lerp(self, goal: $t, alpha: f64) -> $t {
                $t { $($f: self.$f + (goal.$f - self.$f) * alpha),+ }
            }

            /// The smaller of each component.
            pub fn min(self, o: $t) -> $t {
                $t { $($f: self.$f.min(o.$f)),+ }
            }

            /// The larger of each component.
            pub fn max(self, o: $t) -> $t {
                $t { $($f: self.$f.max(o.$f)),+ }
            }

            pub fn abs(self) -> $t {
                $t { $($f: self.$f.abs()),+ }
            }

            pub fn floor(self) -> $t {
                $t { $($f: self.$f.floor()),+ }
            }

            pub fn ceil(self) -> $t {
                $t { $($f: self.$f.ceil()),+ }
            }

            /// Each component's sign: -1, 0 or 1.
            pub fn sign(self) -> $t {
                $t { $($f: sign(self.$f)),+ }
            }

            /// `self // o`: floored division, component by component (for
            /// `v // n`, divide by a vector of n's).
            pub fn floor_div(self, o: $t) -> $t {
                $t { $($f: (self.$f / o.$f).floor()),+ }
            }
        }
    };
}

/// A 2D vector (float32 x2 in Roblox); screen pixels with Y down in GUI code.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

vector_common!(Vector2 { x, y });

impl Vector2 {
    pub const ZERO: Vector2 = Vector2 { x: 0.0, y: 0.0 };
    pub const ONE: Vector2 = Vector2 { x: 1.0, y: 1.0 };
    pub const X_AXIS: Vector2 = Vector2 { x: 1.0, y: 0.0 };
    pub const Y_AXIS: Vector2 = Vector2 { x: 0.0, y: 1.0 };

    pub const fn new(x: f64, y: f64) -> Vector2 {
        Vector2 { x, y }
    }

    /// The 2D cross product, a number: `x1*y2 - y1*x2`.
    pub fn cross(self, o: Vector2) -> f64 {
        self.x * o.y - self.y * o.x
    }

    /// The angle to `o` in radians: 0 to pi, or -pi to pi when `signed`
    /// (negative going clockwise, counting Y up).
    pub fn angle(self, o: Vector2, signed: bool) -> f64 {
        let a = self.cross(o).atan2(self.dot(o));
        if signed { a } else { a.abs() }
    }

    /// Whether each component is within `eps` (Roblox's default is 1e-5).
    pub fn fuzzy_eq(self, o: Vector2, eps: f64) -> bool {
        (self.x - o.x).abs() <= eps && (self.y - o.y).abs() <= eps
    }
}

/// "x, y"; an infinite component prints INF, as Roblox prints a Vector2.
impl fmt::Display for Vector2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let part = |x: f64| match x {
            x if x == f64::INFINITY => "INF".to_string(),
            x if x == f64::NEG_INFINITY => "-INF".to_string(),
            x => num9(x),
        };
        write!(f, "{}, {}", part(self.x), part(self.y))
    }
}

/// A 3D vector (Luau's native float32 x3 vector in Roblox): Y up, -Z forward.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

vector_common!(Vector3 { x, y, z });

/// `n / v`, component by component.
impl Div<Vector3> for f64 {
    type Output = Vector3;
    fn div(self, v: Vector3) -> Vector3 {
        Vector3::new(self / v.x, self / v.y, self / v.z)
    }
}

impl Vector3 {
    pub const ZERO: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vector3 = Vector3 { x: 1.0, y: 1.0, z: 1.0 };
    pub const X_AXIS: Vector3 = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y_AXIS: Vector3 = Vector3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z_AXIS: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3 { x, y, z }
    }

    /// Vector3.FromNormalId: the face's outward direction (Front is -Z).
    pub fn from_normal_id(face: NormalId) -> Vector3 {
        match face {
            NormalId::Right => Vector3::X_AXIS,
            NormalId::Top => Vector3::Y_AXIS,
            NormalId::Back => Vector3::Z_AXIS,
            NormalId::Left => -Vector3::X_AXIS,
            NormalId::Bottom => -Vector3::Y_AXIS,
            NormalId::Front => -Vector3::Z_AXIS,
        }
    }

    /// Vector3.FromAxis.
    pub fn from_axis(axis: Axis) -> Vector3 {
        match axis {
            Axis::X => Vector3::X_AXIS,
            Axis::Y => Vector3::Y_AXIS,
            Axis::Z => Vector3::Z_AXIS,
        }
    }

    pub fn cross(self, o: Vector3) -> Vector3 {
        Vector3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// The angle to `o` in radians, 0 to pi; with an axis it is negative when
    /// the turn from self to o goes clockwise about the axis.
    pub fn angle(self, o: Vector3, axis: Option<Vector3>) -> f64 {
        let c = self.cross(o);
        let a = c.magnitude().atan2(self.dot(o));
        match axis {
            Some(axis) if c.dot(axis) < 0.0 => -a,
            _ => a,
        }
    }

    /// Whether each component is within `eps` (default 1e-5), scaled by the
    /// component's size when that is over 1.
    pub fn fuzzy_eq(self, o: Vector3, eps: f64) -> bool {
        fuzzy(self.x, o.x, eps) && fuzzy(self.y, o.y, eps) && fuzzy(self.z, o.z, eps)
    }
}

/// "x, y, z", each component to 9 significant digits: "1, 2, 3".
impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, {}, {}", num9(self.x), num9(self.y), num9(self.z))
    }
}

// ---------------------------------------------------------------- CFrame

type Matrix = [[f64; 3]; 3];

const IDENTITY: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mat_mul(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]))
}

fn mat_vec(m: &Matrix, v: Vector3) -> Vector3 {
    let row = |r: &[f64; 3]| r[0] * v.x + r[1] * v.y + r[2] * v.z;
    Vector3::new(row(&m[0]), row(&m[1]), row(&m[2]))
}

fn transpose(m: &Matrix) -> Matrix {
    std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]))
}

/// The matrix with these columns.
fn from_columns(x: Vector3, y: Vector3, z: Vector3) -> Matrix {
    [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]]
}

fn column(m: &Matrix, j: usize) -> Vector3 {
    Vector3::new(m[0][j], m[1][j], m[2][j])
}

/// The rotation by `angle` about the unit vector `k` (right-hand rule).
fn axis_angle_matrix(k: Vector3, angle: f64) -> Matrix {
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [c + k.x * k.x * t, k.x * k.y * t - k.z * s, k.x * k.z * t + k.y * s],
        [k.y * k.x * t + k.z * s, c + k.y * k.y * t, k.y * k.z * t - k.x * s],
        [k.z * k.x * t - k.y * s, k.z * k.y * t + k.x * s, c + k.z * k.z * t],
    ]
}

/// The rotation about one axis (0 X, 1 Y, 2 Z).
fn axis_matrix(axis: usize, angle: f64) -> Matrix {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// A rotation order's axes, first applied (leftmost) first.
fn order_axes(order: RotationOrder) -> [usize; 3] {
    match order {
        RotationOrder::XYZ => [0, 1, 2],
        RotationOrder::XZY => [0, 2, 1],
        RotationOrder::YZX => [1, 2, 0],
        RotationOrder::YXZ => [1, 0, 2],
        RotationOrder::ZXY => [2, 0, 1],
        RotationOrder::ZYX => [2, 1, 0],
    }
}

/// A unit quaternion (x, y, z, w) for a rotation matrix (Shepperd's method).
fn matrix_quaternion(m: &Matrix) -> [f64; 4] {
    let trace = m[0][0] + m[1][1] + m[2][2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
            0.25 * s,
        ]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [
            0.25 * s,
            (m[0][1] + m[1][0]) / s,
            (m[0][2] + m[2][0]) / s,
            (m[2][1] - m[1][2]) / s,
        ]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [
            (m[0][1] + m[1][0]) / s,
            0.25 * s,
            (m[1][2] + m[2][1]) / s,
            (m[0][2] - m[2][0]) / s,
        ]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [
            (m[0][2] + m[2][0]) / s,
            (m[1][2] + m[2][1]) / s,
            0.25 * s,
            (m[1][0] - m[0][1]) / s,
        ]
    };
    normalize4(q).unwrap_or([0.0, 0.0, 0.0, 1.0])
}

/// The quaternion at length 1; None for the zero (or a NaN) quaternion.
fn normalize4(q: [f64; 4]) -> Option<[f64; 4]> {
    let len = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    (len > 0.0 && len.is_finite()).then(|| q.map(|c| c / len))
}

fn quaternion_matrix([x, y, z, w]: [f64; 4]) -> Matrix {
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

/// The angle a unit quaternion turns by, 0 to pi.
fn quaternion_angle([x, y, z, w]: [f64; 4]) -> f64 {
    2.0 * (x * x + y * y + z * z).sqrt().atan2(w.abs())
}

/// A position and a 3x3 rotation matrix: where a part is and which way it
/// faces. The matrix's columns are RightVector, UpVector and ZVector
/// (LookVector is -ZVector).
#[derive(Clone, Copy, Debug)]
pub struct CFrame {
    pub pos: Vector3,
    /// The rotation by rows: `[[R00, R01, R02], [R10, R11, R12], [R20, R21, R22]]`.
    pub rot: [[f64; 3]; 3],
}

impl Default for CFrame {
    fn default() -> CFrame {
        CFrame::IDENTITY
    }
}

impl CFrame {
    /// No translation, no rotation.
    pub const IDENTITY: CFrame = CFrame {
        pos: Vector3::ZERO,
        rot: IDENTITY,
    };

    /// `CFrame.new(x, y, z)`: a translation.
    pub fn new(x: f64, y: f64, z: f64) -> CFrame {
        CFrame::from_position(Vector3::new(x, y, z))
    }

    /// `CFrame.new(pos)`: a translation.
    pub fn from_position(pos: Vector3) -> CFrame {
        CFrame { pos, rot: IDENTITY }
    }

    /// `CFrame.new(x, y, z, qX, qY, qZ, qW)`: the quaternion is normalized
    /// first (a zero one gives no rotation).
    pub fn from_quaternion(pos: Vector3, qx: f64, qy: f64, qz: f64, qw: f64) -> CFrame {
        let rot = normalize4([qx, qy, qz, qw]).map_or(IDENTITY, quaternion_matrix);
        CFrame { pos, rot }
    }

    /// `CFrame.new(x, y, z, R00, R01, R02, R10, R11, R12, R20, R21, R22)`,
    /// as GetComponents returns them; the matrix is taken as it is.
    pub fn from_components(c: [f64; 12]) -> CFrame {
        CFrame {
            pos: Vector3::new(c[0], c[1], c[2]),
            rot: [[c[3], c[4], c[5]], [c[6], c[7], c[8]], [c[9], c[10], c[11]]],
        }
    }

    /// `CFrame.new(...)` with numbers: none (identity), 3 (a position), 6 (a
    /// position and a point to look at), 7 (and a quaternion) or 12 (and a
    /// matrix).
    pub fn from_numbers(n: &[f64]) -> Result<CFrame, String> {
        match *n {
            [] => Ok(CFrame::IDENTITY),
            [x, y, z] => Ok(CFrame::new(x, y, z)),
            [x, y, z, lx, ly, lz] => Ok(CFrame::new_look(Vector3::new(x, y, z), Vector3::new(lx, ly, lz))),
            [x, y, z, qx, qy, qz, qw] => Ok(CFrame::from_quaternion(Vector3::new(x, y, z), qx, qy, qz, qw)),
            [..] if n.len() == 12 => Ok(CFrame::from_components(std::array::from_fn(|i| n[i]))),
            _ => Err(format!("CFrame.new takes 0, 3, 6, 7 or 12 numbers, not {}", n.len())),
        }
    }

    /// `CFrame.new(pos, lookAt)`, the legacy `lookAt` with Y up.
    pub fn new_look(pos: Vector3, look_at: Vector3) -> CFrame {
        CFrame::look_at(pos, look_at, Vector3::Y_AXIS)
    }

    /// `CFrame.lookAt(at, target, up)`: at `at`, its LookVector toward
    /// `target`. Looking straight along `up`, the X axis stands in for it;
    /// with `target` at `at`, there's no rotation.
    pub fn look_at(at: Vector3, target: Vector3, up: Vector3) -> CFrame {
        let delta = target - at;
        let length = delta.magnitude();
        if length == 0.0 || !length.is_finite() {
            return CFrame::from_position(at);
        }
        let forward = delta / length;
        let mut right = forward.cross(up);
        if right.magnitude() <= 1e-9 * up.magnitude() {
            right = forward.cross(Vector3::X_AXIS);
        }
        let right = right.unit();
        let upward = right.cross(forward);
        CFrame {
            pos: at,
            rot: from_columns(right, upward, -forward),
        }
    }

    /// `CFrame.lookAlong(at, direction, up)`: `lookAt(at, at + direction, up)`.
    pub fn look_along(at: Vector3, direction: Vector3, up: Vector3) -> CFrame {
        CFrame::look_at(at, at + direction, up)
    }

    /// `CFrame.fromMatrix(pos, vX, vY, vZ)`: the columns; vZ defaults to
    /// `vX:Cross(vY).Unit`.
    pub fn from_matrix(pos: Vector3, vx: Vector3, vy: Vector3, vz: Option<Vector3>) -> CFrame {
        let vz = vz.unwrap_or_else(|| vx.cross(vy).unit());
        CFrame {
            pos,
            rot: from_columns(vx, vy, vz),
        }
    }

    /// `CFrame.fromAxisAngle(axis, angle)`: turns `angle` radians about the
    /// axis (normalized first; a zero one gives no rotation).
    pub fn from_axis_angle(axis: Vector3, angle: f64) -> CFrame {
        let length = axis.magnitude();
        if length == 0.0 || !length.is_finite() {
            return CFrame::IDENTITY;
        }
        CFrame {
            pos: Vector3::ZERO,
            rot: axis_angle_matrix(axis / length, angle),
        }
    }

    /// `CFrame.Angles` = `fromEulerAnglesXYZ`: `Rx(rx) * Ry(ry) * Rz(rz)`.
    pub fn angles(rx: f64, ry: f64, rz: f64) -> CFrame {
        CFrame::from_euler_angles(rx, ry, rz, RotationOrder::XYZ)
    }

    /// `CFrame.fromEulerAnglesXYZ`, the same as `angles`.
    pub fn from_euler_angles_xyz(rx: f64, ry: f64, rz: f64) -> CFrame {
        CFrame::angles(rx, ry, rz)
    }

    /// `CFrame.fromEulerAnglesYXZ` = `fromOrientation`: `Ry(ry) * Rx(rx) * Rz(rz)`.
    pub fn from_euler_angles_yxz(rx: f64, ry: f64, rz: f64) -> CFrame {
        CFrame::from_euler_angles(rx, ry, rz, RotationOrder::YXZ)
    }

    /// `CFrame.fromOrientation`, the same as `from_euler_angles_yxz`.
    pub fn from_orientation(rx: f64, ry: f64, rz: f64) -> CFrame {
        CFrame::from_euler_angles_yxz(rx, ry, rz)
    }

    /// `CFrame.fromEulerAngles(rx, ry, rz, order)`: the three single-axis
    /// rotations multiplied in the order's order.
    pub fn from_euler_angles(rx: f64, ry: f64, rz: f64, order: RotationOrder) -> CFrame {
        let angles = [rx, ry, rz];
        let [a, b, c] = order_axes(order);
        let rot = mat_mul(
            &mat_mul(&axis_matrix(a, angles[a]), &axis_matrix(b, angles[b])),
            &axis_matrix(c, angles[c]),
        );
        CFrame {
            pos: Vector3::ZERO,
            rot,
        }
    }

    /// `CFrame.fromRotationBetweenVectors(from, to)`: the shortest turn that
    /// takes `from`'s direction to `to`'s.
    pub fn from_rotation_between_vectors(from: Vector3, to: Vector3) -> CFrame {
        let (a, b) = (from.unit(), to.unit());
        let axis = a.cross(b);
        let dot = a.dot(b);
        if axis.magnitude() > 1e-12 || dot > 0.0 {
            let q = [axis.x, axis.y, axis.z, 1.0 + dot];
            return CFrame::from_quaternion(Vector3::ZERO, q[0], q[1], q[2], q[3]);
        }
        // Opposite: half a turn about any axis at right angles to them.
        let mut side = a.cross(Vector3::X_AXIS);
        if side.magnitude() < 1e-6 {
            side = a.cross(Vector3::Y_AXIS);
        }
        CFrame::from_axis_angle(side, PI)
    }

    // ------------------------------------------------ properties

    /// Position.
    pub fn position(self) -> Vector3 {
        self.pos
    }

    /// Rotation: the CFrame without its position.
    pub fn rotation(self) -> CFrame {
        CFrame {
            pos: Vector3::ZERO,
            rot: self.rot,
        }
    }

    pub fn x(self) -> f64 {
        self.pos.x
    }

    pub fn y(self) -> f64 {
        self.pos.y
    }

    pub fn z(self) -> f64 {
        self.pos.z
    }

    /// LookVector: forward, the negated third column.
    pub fn look_vector(self) -> Vector3 {
        -column(&self.rot, 2)
    }

    /// RightVector (= XVector): the first column.
    pub fn right_vector(self) -> Vector3 {
        column(&self.rot, 0)
    }

    /// UpVector (= YVector): the second column.
    pub fn up_vector(self) -> Vector3 {
        column(&self.rot, 1)
    }

    pub fn x_vector(self) -> Vector3 {
        column(&self.rot, 0)
    }

    pub fn y_vector(self) -> Vector3 {
        column(&self.rot, 1)
    }

    /// ZVector: the third column (backward).
    pub fn z_vector(self) -> Vector3 {
        column(&self.rot, 2)
    }

    // ------------------------------------------------ methods

    /// The CFrame that undoes this one: `(Rᵀ, -Rᵀ·p)`.
    pub fn inverse(self) -> CFrame {
        let rot = transpose(&self.rot);
        CFrame {
            pos: -mat_vec(&rot, self.pos),
            rot,
        }
    }

    /// The position linearly, the rotation along the shortest arc (slerp);
    /// 0 gives self and 1 gives goal exactly.
    pub fn lerp(self, goal: CFrame, alpha: f64) -> CFrame {
        if alpha == 0.0 {
            return self;
        }
        if alpha == 1.0 {
            return goal;
        }
        let a = matrix_quaternion(&self.rot);
        let mut b = matrix_quaternion(&goal.rot);
        let mut dot: f64 = (0..4).map(|i| a[i] * b[i]).sum();
        if dot < 0.0 {
            b = b.map(|c| -c);
            dot = -dot;
        }
        let q = if dot > 0.9995 {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * alpha)
        } else {
            let theta = dot.min(1.0).acos();
            let (wa, wb) = (((1.0 - alpha) * theta).sin(), (alpha * theta).sin());
            let s = theta.sin();
            std::array::from_fn(|i| (a[i] * wa + b[i] * wb) / s)
        };
        let rot = normalize4(q).map_or(self.rot, quaternion_matrix);
        CFrame {
            pos: self.pos.lerp(goal.pos, alpha),
            rot,
        }
    }

    /// One CFrame from object space to world space: `self * cf`.
    pub fn to_world(self, cf: CFrame) -> CFrame {
        self * cf
    }

    /// One CFrame from world space to object space: `self:Inverse() * cf`.
    pub fn to_object(self, cf: CFrame) -> CFrame {
        self.inverse() * cf
    }

    /// A point from object space to world space: `R·v + p`.
    pub fn point_to_world(self, v: Vector3) -> Vector3 {
        mat_vec(&self.rot, v) + self.pos
    }

    /// A point from world space to object space: `Rᵀ·(v - p)`.
    pub fn point_to_object(self, v: Vector3) -> Vector3 {
        mat_vec(&transpose(&self.rot), v - self.pos)
    }

    /// A direction from object space to world space: `R·v`.
    pub fn vector_to_world(self, v: Vector3) -> Vector3 {
        mat_vec(&self.rot, v)
    }

    /// A direction from world space to object space: `Rᵀ·v`.
    pub fn vector_to_object(self, v: Vector3) -> Vector3 {
        mat_vec(&transpose(&self.rot), v)
    }

    /// ToWorldSpace(...): each one through `to_world`; none is one identity.
    pub fn to_world_space(self, cfs: &[CFrame]) -> Vec<CFrame> {
        each_or(cfs, CFrame::IDENTITY, |cf| self.to_world(cf))
    }

    /// ToObjectSpace(...): each one through `to_object`; none is one identity.
    pub fn to_object_space(self, cfs: &[CFrame]) -> Vec<CFrame> {
        each_or(cfs, CFrame::IDENTITY, |cf| self.to_object(cf))
    }

    /// PointToWorldSpace(...); none is the origin, which gives the position.
    pub fn point_to_world_space(self, vs: &[Vector3]) -> Vec<Vector3> {
        each_or(vs, Vector3::ZERO, |v| self.point_to_world(v))
    }

    /// PointToObjectSpace(...); none is the origin.
    pub fn point_to_object_space(self, vs: &[Vector3]) -> Vec<Vector3> {
        each_or(vs, Vector3::ZERO, |v| self.point_to_object(v))
    }

    /// VectorToWorldSpace(...); none is the zero vector.
    pub fn vector_to_world_space(self, vs: &[Vector3]) -> Vec<Vector3> {
        each_or(vs, Vector3::ZERO, |v| self.vector_to_world(v))
    }

    /// VectorToObjectSpace(...); none is the zero vector.
    pub fn vector_to_object_space(self, vs: &[Vector3]) -> Vec<Vector3> {
        each_or(vs, Vector3::ZERO, |v| self.vector_to_object(v))
    }

    /// GetComponents: x, y, z, then the matrix by rows.
    pub fn get_components(self) -> [f64; 12] {
        let (p, r) = (self.pos, self.rot);
        [
            p.x, p.y, p.z, r[0][0], r[0][1], r[0][2], r[1][0], r[1][1], r[1][2], r[2][0], r[2][1], r[2][2],
        ]
    }

    /// `components()`, the same as `get_components`.
    pub fn components(self) -> [f64; 12] {
        self.get_components()
    }

    /// The rotation as a unit quaternion (x, y, z, w).
    pub fn quaternion(self) -> [f64; 4] {
        matrix_quaternion(&self.rot)
    }

    /// ToEulerAnglesXYZ: `(rx, ry, rz)` for which `angles` gives this rotation.
    pub fn to_euler_angles_xyz(self) -> (f64, f64, f64) {
        self.to_euler_angles(RotationOrder::XYZ)
    }

    /// ToEulerAnglesYXZ: `(rx, ry, rz)` for `from_euler_angles_yxz`.
    pub fn to_euler_angles_yxz(self) -> (f64, f64, f64) {
        self.to_euler_angles(RotationOrder::YXZ)
    }

    /// ToOrientation, the same as `to_euler_angles_yxz`.
    pub fn to_orientation(self) -> (f64, f64, f64) {
        self.to_euler_angles_yxz()
    }

    /// ToEulerAngles(order): `(rx, ry, rz)` for which `from_euler_angles`
    /// with that order gives this rotation. For XYZ that is `ry = asin(R02)`,
    /// `rx = atan2(-R12, R22)`, `rz = atan2(-R01, R00)`; at gimbal lock the
    /// last axis gets 0.
    pub fn to_euler_angles(self, order: RotationOrder) -> (f64, f64, f64) {
        let [i, j, k] = order_axes(order);
        let m = &self.rot;
        // +1 for the cyclic orders (XYZ, YZX, ZXY), -1 for the others.
        let s = if (j + 3 - i) % 3 == 1 { 1.0 } else { -1.0 };
        let middle = (s * m[i][k]).clamp(-1.0, 1.0).asin();
        let (first, last) = if m[j][k] == 0.0 && m[k][k] == 0.0 {
            ((s * m[k][j]).atan2(m[j][j]), 0.0)
        } else {
            ((-s * m[j][k]).atan2(m[k][k]), (-s * m[i][j]).atan2(m[i][i]))
        };
        let mut out = [0.0; 3];
        out[i] = first;
        out[j] = middle;
        out[k] = last;
        (out[0], out[1], out[2])
    }

    /// ToAxisAngle: a unit axis and an angle from 0 to pi, as
    /// `from_axis_angle` takes them; no rotation gives the X axis and 0.
    pub fn to_axis_angle(self) -> (Vector3, f64) {
        let q = matrix_quaternion(&self.rot);
        let [x, y, z, w] = if q[3] < 0.0 { q.map(|c| -c) } else { q };
        let v = Vector3::new(x, y, z);
        let length = v.magnitude();
        if length == 0.0 {
            return (Vector3::X_AXIS, 0.0);
        }
        (v / length, 2.0 * length.atan2(w))
    }

    /// Orthonormalize: Gram-Schmidt on the columns, X first.
    pub fn orthonormalize(self) -> CFrame {
        let (a, b, c) = (column(&self.rot, 0), column(&self.rot, 1), column(&self.rot, 2));
        let x = a.unit();
        let y = (b - x * x.dot(b)).unit();
        let z = (c - x * x.dot(c) - y * y.dot(c)).unit();
        CFrame {
            pos: self.pos,
            rot: from_columns(x, y, z),
        }
    }

    /// FuzzyEq: the positions within `eps` component by component (as
    /// Vector3's), and the rotations within `eps` radians of each other.
    pub fn fuzzy_eq(self, o: CFrame, eps: f64) -> bool {
        self.pos.fuzzy_eq(o.pos, eps) && self.angle_between(o) <= eps
    }

    /// AngleBetween: how far the rotation of `self:Inverse() * o` turns, 0 to pi.
    pub fn angle_between(self, o: CFrame) -> f64 {
        quaternion_angle(matrix_quaternion(&mat_mul(&transpose(&self.rot), &o.rot)))
    }
}

/// Each item through `f`, or just `default` through it when there are none.
fn each_or<T: Copy, U>(items: &[T], default: T, f: impl Fn(T) -> U) -> Vec<U> {
    if items.is_empty() {
        vec![f(default)]
    } else {
        items.iter().map(|&t| f(t)).collect()
    }
}

/// Composition: `(Ra·Rb, Ra·pb + pa)`, b applied in a's space.
impl Mul for CFrame {
    type Output = CFrame;
    fn mul(self, o: CFrame) -> CFrame {
        CFrame {
            pos: self.point_to_world(o.pos),
            rot: mat_mul(&self.rot, &o.rot),
        }
    }
}

/// A point to world space: `R·v + p`.
impl Mul<Vector3> for CFrame {
    type Output = Vector3;
    fn mul(self, v: Vector3) -> Vector3 {
        self.point_to_world(v)
    }
}

/// Moved in world space; the rotation stays.
impl Add<Vector3> for CFrame {
    type Output = CFrame;
    fn add(self, v: Vector3) -> CFrame {
        CFrame {
            pos: self.pos.add(v),
            rot: self.rot,
        }
    }
}

impl Sub<Vector3> for CFrame {
    type Output = CFrame;
    fn sub(self, v: Vector3) -> CFrame {
        CFrame {
            pos: self.pos.sub(v),
            rot: self.rot,
        }
    }
}

/// Equal as float32, all 12 components.
impl PartialEq for CFrame {
    fn eq(&self, o: &CFrame) -> bool {
        self.get_components()
            .iter()
            .zip(o.get_components())
            .all(|(&a, b)| same32(a, b))
    }
}

/// The 12 components: "5, 12, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1".
impl fmt::Display for CFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let parts: Vec<String> = self.get_components().iter().map(|&c| num9(c)).collect();
        f.write_str(&parts.join(", "))
    }
}

// ---------------------------------------------------------------- Color3

/// A color, each channel nominally 0 to 1 but not clamped.
#[derive(Clone, Copy, Debug, Default)]
pub struct Color3 {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color3 {
    pub const fn new(r: f64, g: f64, b: f64) -> Color3 {
        Color3 { r, g, b }
    }

    /// Color3.fromRGB: channels 0 to 255.
    pub fn from_rgb(r: f64, g: f64, b: f64) -> Color3 {
        Color3::new(r / 255.0, g / 255.0, b / 255.0)
    }

    /// Color3.fromHSV: hue, saturation and value 0 to 1; the hue wraps, so 1 is red.
    pub fn from_hsv(h: f64, s: f64, v: f64) -> Color3 {
        let h = (h - h.floor()) * 6.0;
        let sector = h.floor();
        let f = h - sector;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
        match sector as i32 {
            0 => Color3::new(v, t, p),
            1 => Color3::new(q, v, p),
            2 => Color3::new(p, v, t),
            3 => Color3::new(p, q, v),
            4 => Color3::new(t, p, v),
            _ => Color3::new(v, p, q),
        }
    }

    /// Color3.fromHex: "RRGGBB" or "RGB", in either case, with or without "#".
    pub fn from_hex(hex: &str) -> Result<Color3, String> {
        let digits = hex.strip_prefix('#').unwrap_or(hex);
        let value = |s: &str| u8::from_str_radix(s, 16).ok();
        let channels = match digits.len() {
            _ if !digits.bytes().all(|c| c.is_ascii_hexdigit()) => None,
            6 => (0..3)
                .map(|i| value(&digits[i * 2..i * 2 + 2]))
                .collect::<Option<Vec<u8>>>(),
            3 => (0..3).map(|i| value(&digits[i..i + 1]).map(|v| v * 17)).collect(),
            _ => None,
        };
        match channels.as_deref() {
            Some(&[r, g, b]) => Ok(Color3::from_rgb(r.into(), g.into(), b.into())),
            _ => Err(format!("Color3.fromHex: {hex:?} is not a hex color (RRGGBB or RGB)")),
        }
    }

    /// ToHSV: hue (0 to under 1, red is 0), saturation and value.
    pub fn to_hsv(self) -> (f64, f64, f64) {
        let (r, g, b) = (self.r, self.g, self.b);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let s = if max == 0.0 { 0.0 } else { delta / max };
        if delta == 0.0 {
            return (0.0, s, max);
        }
        let sixths = if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        let h = sixths / 6.0;
        (if h >= 1.0 { 0.0 } else { h }, s, max)
    }

    /// ToHex: "rrggbb" in lowercase, without "#"; each channel ×255, rounded.
    pub fn to_hex(self) -> String {
        let byte = |c: f64| (c * 255.0).round().clamp(0.0, 255.0) as u8;
        format!("{:02x}{:02x}{:02x}", byte(self.r), byte(self.g), byte(self.b))
    }

    /// Each channel linearly toward `goal`.
    pub fn lerp(self, goal: Color3, alpha: f64) -> Color3 {
        let mix = |a: f64, b: f64| a + (b - a) * alpha;
        Color3::new(mix(self.r, goal.r), mix(self.g, goal.g), mix(self.b, goal.b))
    }
}

impl PartialEq for Color3 {
    fn eq(&self, o: &Color3) -> bool {
        same32(self.r, o.r) && same32(self.g, o.g) && same32(self.b, o.b)
    }
}

/// "r, g, b", 6 significant digits: "0.105882, 0.164706, 0.207843".
impl fmt::Display for Color3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, {}, {}", num6(self.r), num6(self.g), num6(self.b))
    }
}

// ---------------------------------------------------------------- UDim, UDim2, Rect

/// One GUI dimension: `Scale * reference + Offset` pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct UDim {
    pub scale: f64,
    /// An int32 in Roblox too.
    pub offset: i32,
}

impl UDim {
    /// UDim.new: the offset is truncated toward zero, as an int32 is.
    pub fn new(scale: f64, offset: f64) -> UDim {
        UDim {
            scale,
            offset: offset as i32,
        }
    }

    /// Its pixels against a reference length.
    pub fn resolve(self, reference: f64) -> f64 {
        self.scale * reference + f64::from(self.offset)
    }

    /// For tweens: the scale linearly, the offset rounded.
    pub fn lerp(self, goal: UDim, alpha: f64) -> UDim {
        let offset = f64::from(self.offset) + f64::from(goal.offset - self.offset) * alpha;
        UDim {
            scale: self.scale + (goal.scale - self.scale) * alpha,
            offset: offset.round() as i32,
        }
    }
}

impl Add for UDim {
    type Output = UDim;
    fn add(self, o: UDim) -> UDim {
        UDim {
            scale: self.scale + o.scale,
            offset: self.offset.wrapping_add(o.offset),
        }
    }
}

impl Sub for UDim {
    type Output = UDim;
    fn sub(self, o: UDim) -> UDim {
        UDim {
            scale: self.scale - o.scale,
            offset: self.offset.wrapping_sub(o.offset),
        }
    }
}

impl PartialEq for UDim {
    fn eq(&self, o: &UDim) -> bool {
        same32(self.scale, o.scale) && self.offset == o.offset
    }
}

/// "scale, offset".
impl fmt::Display for UDim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}, {}", num9(self.scale), self.offset)
    }
}

/// A GUI position or size: an X and a Y UDim.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UDim2 {
    pub x: UDim,
    pub y: UDim,
}

impl UDim2 {
    /// UDim2.new(xScale, xOffset, yScale, yOffset).
    pub fn new(x_scale: f64, x_offset: f64, y_scale: f64, y_offset: f64) -> UDim2 {
        UDim2 {
            x: UDim::new(x_scale, x_offset),
            y: UDim::new(y_scale, y_offset),
        }
    }

    /// UDim2.new(x, y) from two UDims.
    pub fn from_udims(x: UDim, y: UDim) -> UDim2 {
        UDim2 { x, y }
    }

    /// UDim2.fromScale: no offsets.
    pub fn from_scale(x: f64, y: f64) -> UDim2 {
        UDim2::new(x, 0.0, y, 0.0)
    }

    /// UDim2.fromOffset: no scales.
    pub fn from_offset(x: f64, y: f64) -> UDim2 {
        UDim2::new(0.0, x, 0.0, y)
    }

    /// Width, the same as X.
    pub fn width(self) -> UDim {
        self.x
    }

    /// Height, the same as Y.
    pub fn height(self) -> UDim {
        self.y
    }

    /// Each axis toward `goal`: the scales linearly, the offsets rounded.
    pub fn lerp(self, goal: UDim2, alpha: f64) -> UDim2 {
        UDim2 {
            x: self.x.lerp(goal.x, alpha),
            y: self.y.lerp(goal.y, alpha),
        }
    }

    /// Its pixels against a reference size.
    pub fn resolve(self, reference: Vector2) -> Vector2 {
        Vector2::new(self.x.resolve(reference.x), self.y.resolve(reference.y))
    }
}

impl Add for UDim2 {
    type Output = UDim2;
    fn add(self, o: UDim2) -> UDim2 {
        UDim2 {
            x: self.x.add(o.x),
            y: self.y.add(o.y),
        }
    }
}

impl Sub for UDim2 {
    type Output = UDim2;
    fn sub(self, o: UDim2) -> UDim2 {
        UDim2 {
            x: self.x.sub(o.x),
            y: self.y.sub(o.y),
        }
    }
}

/// "{xs, xo}, {ys, yo}": "{0, 300}, {1, 0}".
impl fmt::Display for UDim2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{{}}}, {{{}}}", self.x, self.y)
    }
}

/// A rectangle by its corners (ImageLabel.SliceCenter, GuiService.TopbarInset).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vector2,
    pub max: Vector2,
}

impl Rect {
    pub fn new(min: Vector2, max: Vector2) -> Rect {
        Rect { min, max }
    }

    /// Rect.new(minX, minY, maxX, maxY).
    pub fn from_coords(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Rect {
        Rect::new(Vector2::new(min_x, min_y), Vector2::new(max_x, max_y))
    }

    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }
}

/// "minX, minY, maxX, maxY".
impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (a, b) = (self.min, self.max);
        write!(f, "{}, {}, {}, {}", num9(a.x), num9(a.y), num9(b.x), num9(b.y))
    }
}

/// A box in the world by its corners (Terrain:ReadVoxels' region), not
/// turned.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Region3 {
    pub min: Vector3,
    pub max: Vector3,
}

impl Region3 {
    /// Region3.new(min, max): the least and the most of the corners given.
    pub fn new(a: Vector3, b: Vector3) -> Region3 {
        Region3 {
            min: a.min(b),
            max: a.max(b),
        }
    }

    /// Its centre, as a CFrame with no turn.
    pub fn cframe(self) -> CFrame {
        CFrame::from_position((self.min + self.max) / 2.0)
    }

    pub fn size(self) -> Vector3 {
        self.max - self.min
    }

    /// Region3:ExpandToGrid(resolution): its corners out to the grid of
    /// cells `resolution` a side.
    pub fn expand_to_grid(self, resolution: f64) -> Region3 {
        let r = resolution;
        let out = |v: Vector3, f: fn(f64) -> f64| Vector3::new(f(v.x / r) * r, f(v.y / r) * r, f(v.z / r) * r);
        Region3 {
            min: out(self.min, f64::floor),
            max: out(self.max, f64::ceil),
        }
    }
}

/// "CFrame; Size", as Roblox writes it.
impl fmt::Display for Region3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}; {}", self.cframe(), self.size())
    }
}

// ---------------------------------------------------------------- ranges and sequences

/// A range of numbers, Min to Max.
#[derive(Clone, Copy, Debug, Default)]
pub struct NumberRange {
    pub min: f64,
    pub max: f64,
}

impl NumberRange {
    /// NumberRange.new(min, max); min may not be over max.
    pub fn new(min: f64, max: f64) -> Result<NumberRange, String> {
        if float32(min) > float32(max) {
            return Err("NumberRange: the minimum is greater than the maximum".into());
        }
        Ok(NumberRange { min, max })
    }

    /// NumberRange.new(value): Min and Max both `value`.
    pub fn single(value: f64) -> NumberRange {
        NumberRange { min: value, max: value }
    }
}

impl PartialEq for NumberRange {
    fn eq(&self, o: &NumberRange) -> bool {
        same32(self.min, o.min) && same32(self.max, o.max)
    }
}

/// "min max " with a space after each: "0 60000 ".
impl fmt::Display for NumberRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} ", num6(self.min), num6(self.max))
    }
}

/// The longest sequence Roblox takes.
pub const MAX_KEYPOINTS: usize = 20;

/// Roblox's checks on a sequence's keypoint times: 2 to 20 of them, in
/// time order, from exactly 0 to exactly 1 (as float32).
fn check_times(kind: &str, times: &[f64]) -> Result<(), String> {
    if times.len() < 2 {
        return Err(format!("{kind}: it needs at least 2 keypoints"));
    }
    if times.len() > MAX_KEYPOINTS {
        return Err(format!("{kind}: it takes at most {MAX_KEYPOINTS} keypoints"));
    }
    if times.windows(2).any(|w| float32(w[1]) < float32(w[0]) || w[1].is_nan()) {
        return Err(format!("{kind}: all keypoints must be ordered by time"));
    }
    if float32(times[0]) != 0.0 || float32(times[times.len() - 1]) != 1.0 {
        return Err(format!(
            "{kind}: the first keypoint must be at time 0 and the last at time 1"
        ));
    }
    Ok(())
}

/// Where `t` falls: the keypoint pair around it and how far between them,
/// as the docs' evaluation does (t at 0 or 1 is the first or last keypoint).
fn locate(times: &[f64], t: f64) -> (usize, usize, f64) {
    let last = times.len() - 1;
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    if t <= 0.0 {
        return (0, 0, 0.0);
    }
    for i in 0..last {
        if t >= times[i] && t < times[i + 1] {
            return (i, i + 1, (t - times[i]) / (times[i + 1] - times[i]));
        }
    }
    (last, last, 0.0)
}

/// A NumberSequence keypoint: a value (± a random envelope) at a time.
#[derive(Clone, Copy, Debug, Default)]
pub struct NumberSequenceKeypoint {
    pub time: f64,
    pub value: f64,
    pub envelope: f64,
}

impl NumberSequenceKeypoint {
    /// NumberSequenceKeypoint.new(time, value, envelope = 0).
    pub fn new(time: f64, value: f64, envelope: f64) -> NumberSequenceKeypoint {
        NumberSequenceKeypoint { time, value, envelope }
    }
}

impl PartialEq for NumberSequenceKeypoint {
    fn eq(&self, o: &NumberSequenceKeypoint) -> bool {
        same32(self.time, o.time) && same32(self.value, o.value) && same32(self.envelope, o.envelope)
    }
}

/// Numbers over a time from 0 to 1 (ParticleEmitter.Size, UIGradient.Transparency).
#[derive(Clone, Debug, PartialEq)]
pub struct NumberSequence {
    keypoints: Vec<NumberSequenceKeypoint>,
}

impl NumberSequence {
    /// NumberSequence.new({keypoints}), checked as Roblox checks it.
    pub fn new(keypoints: Vec<NumberSequenceKeypoint>) -> Result<NumberSequence, String> {
        let times: Vec<f64> = keypoints.iter().map(|k| k.time).collect();
        check_times("NumberSequence", &times)?;
        Ok(NumberSequence { keypoints })
    }

    /// NumberSequence.new(n): `n` throughout.
    pub fn constant(n: f64) -> NumberSequence {
        NumberSequence::between(n, n)
    }

    /// NumberSequence.new(n0, n1): `n0` at time 0 to `n1` at time 1.
    pub fn between(n0: f64, n1: f64) -> NumberSequence {
        let k = |time, value| NumberSequenceKeypoint::new(time, value, 0.0);
        NumberSequence {
            keypoints: vec![k(0.0, n0), k(1.0, n1)],
        }
    }

    /// Keypoints, in time order.
    pub fn keypoints(&self) -> &[NumberSequenceKeypoint] {
        &self.keypoints
    }

    fn times(&self) -> Vec<f64> {
        self.keypoints.iter().map(|k| k.time).collect()
    }

    /// The value at `t` (clamped to 0..1): linear between the keypoints
    /// around it, the envelopes left out.
    pub fn value_at(&self, t: f64) -> f64 {
        let (a, b, alpha) = locate(&self.times(), t);
        let (a, b) = (self.keypoints[a].value, self.keypoints[b].value);
        a + (b - a) * alpha
    }

    /// The envelope at `t`, linear between keypoints the same way.
    pub fn envelope_at(&self, t: f64) -> f64 {
        let (a, b, alpha) = locate(&self.times(), t);
        let (a, b) = (self.keypoints[a].envelope, self.keypoints[b].envelope);
        a + (b - a) * alpha
    }
}

/// "time value envelope " for each keypoint: "0 0.5 0 1 0.5 0 ".
impl fmt::Display for NumberSequence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for k in &self.keypoints {
            write!(f, "{} {} {} ", num6(k.time), num6(k.value), num6(k.envelope))?;
        }
        Ok(())
    }
}

/// A ColorSequence keypoint: a color at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ColorSequenceKeypoint {
    pub time: f64,
    pub value: Color3,
}

impl ColorSequenceKeypoint {
    pub fn new(time: f64, value: Color3) -> ColorSequenceKeypoint {
        ColorSequenceKeypoint { time, value }
    }
}

/// Colors over a time from 0 to 1 (UIGradient.Color, Beam.Color).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSequence {
    keypoints: Vec<ColorSequenceKeypoint>,
}

impl ColorSequence {
    /// ColorSequence.new({keypoints}), checked as Roblox checks it.
    pub fn new(keypoints: Vec<ColorSequenceKeypoint>) -> Result<ColorSequence, String> {
        let times: Vec<f64> = keypoints.iter().map(|k| k.time).collect();
        check_times("ColorSequence", &times)?;
        Ok(ColorSequence { keypoints })
    }

    /// ColorSequence.new(c): `c` throughout.
    pub fn constant(c: Color3) -> ColorSequence {
        ColorSequence::between(c, c)
    }

    /// ColorSequence.new(c0, c1).
    pub fn between(c0: Color3, c1: Color3) -> ColorSequence {
        let keypoints = vec![ColorSequenceKeypoint::new(0.0, c0), ColorSequenceKeypoint::new(1.0, c1)];
        ColorSequence { keypoints }
    }

    pub fn keypoints(&self) -> &[ColorSequenceKeypoint] {
        &self.keypoints
    }

    /// The color at `t` (clamped to 0..1), each channel linear between the
    /// keypoints around it, as UIGradient draws it.
    pub fn value_at(&self, t: f64) -> Color3 {
        let times: Vec<f64> = self.keypoints.iter().map(|k| k.time).collect();
        let (a, b, alpha) = locate(&times, t);
        self.keypoints[a].value.lerp(self.keypoints[b].value, alpha)
    }
}

/// "time r g b 0 " for each keypoint: "0 1 1 1 0 1 1 1 1 0 ".
impl fmt::Display for ColorSequence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for k in &self.keypoints {
            let c = k.value;
            write!(f, "{} {} {} {} 0 ", num6(k.time), num6(c.r), num6(c.g), num6(c.b))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- tweens

/// TweenInfo: how a tween runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TweenInfo {
    /// Seconds.
    pub time: f64,
    pub easing_style: EasingStyle,
    pub easing_direction: EasingDirection,
    /// Extra cycles; -1 repeats forever.
    pub repeat_count: i32,
    /// Each cycle plays back to the start after reaching the goal.
    pub reverses: bool,
    /// Seconds before it starts.
    pub delay_time: f64,
}

impl Default for TweenInfo {
    /// TweenInfo.new(): 1 second of Quad Out.
    fn default() -> TweenInfo {
        TweenInfo::new(1.0, EasingStyle::Quad, EasingDirection::Out, 0, false, 0.0)
    }
}

impl TweenInfo {
    pub fn new(
        time: f64,
        easing_style: EasingStyle,
        easing_direction: EasingDirection,
        repeat_count: i32,
        reverses: bool,
        delay_time: f64,
    ) -> TweenInfo {
        TweenInfo {
            time,
            easing_style,
            easing_direction,
            repeat_count,
            reverses,
            delay_time,
        }
    }

    /// The eased progress at `alpha` (0 to 1) of a cycle.
    pub fn value(&self, alpha: f64) -> f64 {
        ease(self.easing_style, self.easing_direction, alpha)
    }
}

/// "Time:1 DelayTime:0 RepeatCount:0 Reverses:False EasingDirection:Out EasingStyle:Quad".
impl fmt::Display for TweenInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Time:{} DelayTime:{} RepeatCount:{} Reverses:{} EasingDirection:{} EasingStyle:{}",
            num6(self.time),
            num6(self.delay_time),
            self.repeat_count,
            if self.reverses { "True" } else { "False" },
            self.easing_direction.name(),
            self.easing_style.name()
        )
    }
}

/// TweenService:GetValue: the eased value of `t`, which is clamped to 0..1
/// (so 0 gives exactly 0 and 1 exactly 1). Each style's In curve is a
/// Robert Penner form; `Out(t) = 1 - In(1 - t)`, and InOut runs In to the
/// middle and Out after it.
pub fn ease(style: EasingStyle, direction: EasingDirection, t: f64) -> f64 {
    if t.is_nan() || t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    match direction {
        EasingDirection::In => ease_in(style, t),
        EasingDirection::Out => 1.0 - ease_in(style, 1.0 - t),
        EasingDirection::InOut if t < 0.5 => ease_in(style, 2.0 * t) / 2.0,
        EasingDirection::InOut => 1.0 - ease_in(style, 2.0 - 2.0 * t) / 2.0,
    }
}

/// A style's In curve, from 0 at t = 0 to 1 at t = 1.
fn ease_in(style: EasingStyle, t: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    match style {
        EasingStyle::Linear => t,
        EasingStyle::Sine => 1.0 - (t * FRAC_PI_2).cos(),
        EasingStyle::Quad => t * t,
        EasingStyle::Cubic => t * t * t,
        EasingStyle::Quart => t.powi(4),
        EasingStyle::Quint => t.powi(5),
        EasingStyle::Exponential => 2f64.powf(10.0 * t - 10.0),
        EasingStyle::Circular => 1.0 - (1.0 - t * t).sqrt(),
        EasingStyle::Back => {
            const S: f64 = 1.70158;
            t * t * ((S + 1.0) * t - S)
        }
        EasingStyle::Elastic => -(2f64.powf(10.0 * t - 10.0)) * ((10.0 * t - 10.75) * TAU / 3.0).sin(),
        EasingStyle::Bounce => 1.0 - bounce_out(1.0 - t),
    }
}

/// Bounce's Out curve: four falling parabolas.
fn bounce_out(t: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let t = t - 1.5 / D;
        N * t * t + 0.75
    } else if t < 2.5 / D {
        let t = t - 2.25 / D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625 / D;
        N * t * t + 0.984375
    }
}

// ---------------------------------------------------------------- BrickColor

/// Every BrickColor: number, name, palette index (-1 when it isn't in the
/// 128-color palette) and RGB, by number.
const BRICK_COLORS: &[(u16, &str, i8, [u8; 3])] = &[
    (1, "White", 87, [242, 243, 243]),
    (2, "Grey", -1, [161, 165, 162]),
    (3, "Light yellow", -1, [249, 233, 153]),
    (5, "Brick yellow", 109, [215, 197, 154]),
    (6, "Light green (Mint)", -1, [194, 218, 184]),
    (9, "Light reddish violet", 88, [232, 186, 200]),
    (11, "Pastel Blue", 51, [128, 187, 219]),
    (12, "Light orange brown", -1, [203, 132, 66]),
    (18, "Nougat", 110, [204, 142, 105]),
    (21, "Bright red", 68, [196, 40, 28]),
    (22, "Med. reddish violet", -1, [196, 112, 160]),
    (23, "Bright blue", 21, [13, 105, 172]),
    (24, "Bright yellow", 71, [245, 205, 48]),
    (25, "Earth orange", -1, [98, 71, 50]),
    (26, "Black", 3, [27, 42, 53]),
    (27, "Dark grey", -1, [109, 110, 108]),
    (28, "Dark green", 8, [40, 127, 71]),
    (29, "Medium green", 59, [161, 196, 140]),
    (36, "Lig. Yellowich orange", -1, [243, 207, 155]),
    (37, "Bright green", 25, [75, 151, 75]),
    (38, "Dark orange", 119, [160, 95, 53]),
    (39, "Light bluish violet", -1, [193, 202, 222]),
    (40, "Transparent", -1, [236, 236, 236]),
    (41, "Tr. Red", -1, [205, 84, 75]),
    (42, "Tr. Lg blue", -1, [193, 223, 240]),
    (43, "Tr. Blue", -1, [123, 182, 232]),
    (44, "Tr. Yellow", -1, [247, 241, 141]),
    (45, "Light blue", 63, [180, 210, 228]),
    (47, "Tr. Flu. Reddish orange", -1, [217, 133, 108]),
    (48, "Tr. Green", -1, [132, 182, 141]),
    (49, "Tr. Flu. Green", -1, [248, 241, 132]),
    (50, "Phosph. White", -1, [236, 232, 222]),
    (100, "Light red", -1, [238, 196, 182]),
    (101, "Medium red", 101, [218, 134, 122]),
    (102, "Medium blue", 20, [110, 153, 202]),
    (103, "Light grey", -1, [199, 193, 183]),
    (104, "Bright violet", 44, [107, 50, 124]),
    (105, "Br. yellowish orange", 94, [226, 155, 64]),
    (106, "Bright orange", 83, [218, 133, 65]),
    (107, "Bright bluish green", 2, [0, 143, 156]),
    (108, "Earth yellow", -1, [104, 92, 67]),
    (110, "Bright bluish violet", -1, [67, 84, 147]),
    (111, "Tr. Brown", -1, [191, 183, 177]),
    (112, "Medium bluish violet", -1, [104, 116, 172]),
    (113, "Tr. Medi. reddish violet", -1, [229, 173, 200]),
    (115, "Med. yellowish green", -1, [199, 210, 60]),
    (116, "Med. bluish green", -1, [85, 165, 175]),
    (118, "Light bluish green", -1, [183, 215, 213]),
    (119, "Br. yellowish green", 46, [164, 189, 71]),
    (120, "Lig. yellowish green", -1, [217, 228, 167]),
    (121, "Med. yellowish orange", -1, [231, 172, 88]),
    (123, "Br. reddish orange", -1, [211, 111, 76]),
    (124, "Bright reddish violet", -1, [146, 57, 120]),
    (125, "Light orange", 100, [234, 184, 146]),
    (126, "Tr. Bright bluish violet", -1, [165, 165, 203]),
    (127, "Gold", -1, [220, 188, 129]),
    (128, "Dark nougat", -1, [174, 122, 89]),
    (131, "Silver", -1, [156, 163, 168]),
    (133, "Neon orange", 82, [213, 115, 61]),
    (134, "Neon green", -1, [216, 221, 86]),
    (135, "Sand blue", 19, [116, 134, 157]),
    (136, "Sand violet", -1, [135, 124, 144]),
    (137, "Medium orange", -1, [224, 152, 100]),
    (138, "Sand yellow", -1, [149, 138, 115]),
    (140, "Earth blue", -1, [32, 58, 86]),
    (141, "Earth green", 0, [39, 70, 45]),
    (143, "Tr. Flu. Blue", -1, [207, 226, 247]),
    (145, "Sand blue metallic", -1, [121, 136, 161]),
    (146, "Sand violet metallic", -1, [149, 142, 163]),
    (147, "Sand yellow metallic", -1, [147, 135, 103]),
    (148, "Dark grey metallic", -1, [87, 88, 87]),
    (149, "Black metallic", -1, [22, 29, 50]),
    (150, "Light grey metallic", -1, [171, 173, 172]),
    (151, "Sand green", 34, [120, 144, 130]),
    (153, "Sand red", 114, [149, 121, 119]),
    (154, "Dark red", -1, [123, 46, 47]),
    (157, "Tr. Flu. Yellow", -1, [255, 246, 123]),
    (158, "Tr. Flu. Red", -1, [225, 164, 194]),
    (168, "Gun metallic", -1, [117, 108, 98]),
    (176, "Red flip/flop", -1, [151, 105, 91]),
    (178, "Yellow flip/flop", -1, [180, 132, 85]),
    (179, "Silver flip/flop", -1, [137, 135, 136]),
    (180, "Curry", -1, [215, 169, 75]),
    (190, "Fire Yellow", -1, [249, 214, 46]),
    (191, "Flame yellowish orange", -1, [232, 171, 45]),
    (192, "Reddish brown", 103, [105, 64, 40]),
    (193, "Flame reddish orange", -1, [207, 96, 36]),
    (194, "Medium stone grey", 123, [163, 162, 165]),
    (195, "Royal blue", -1, [70, 103, 164]),
    (196, "Dark Royal blue", -1, [35, 71, 139]),
    (198, "Bright reddish lilac", -1, [142, 66, 133]),
    (199, "Dark stone grey", 122, [99, 95, 98]),
    (200, "Lemon metalic", -1, [130, 138, 93]),
    (208, "Light stone grey", 62, [229, 228, 223]),
    (209, "Dark Curry", -1, [176, 142, 68]),
    (210, "Faded green", -1, [112, 149, 120]),
    (211, "Turquoise", -1, [121, 181, 181]),
    (212, "Light Royal blue", -1, [159, 195, 233]),
    (213, "Medium Royal blue", -1, [108, 129, 183]),
    (216, "Rust", -1, [144, 76, 42]),
    (217, "Brown", 111, [124, 92, 70]),
    (218, "Reddish lilac", -1, [150, 112, 159]),
    (219, "Lilac", -1, [107, 98, 155]),
    (220, "Light lilac", -1, [167, 169, 206]),
    (221, "Bright purple", -1, [205, 98, 152]),
    (222, "Light purple", -1, [228, 173, 200]),
    (223, "Light pink", -1, [220, 144, 149]),
    (224, "Light brick yellow", -1, [240, 213, 160]),
    (225, "Warm yellowish orange", -1, [235, 184, 127]),
    (226, "Cool yellow", 73, [253, 234, 141]),
    (232, "Dove blue", -1, [125, 187, 221]),
    (268, "Medium lilac", -1, [52, 43, 117]),
    (301, "Slime green", 1, [80, 109, 84]),
    (302, "Smoky grey", 10, [91, 93, 105]),
    (303, "Dark blue", 5, [0, 16, 176]),
    (304, "Parsley green", 7, [44, 101, 29]),
    (305, "Steel blue", 11, [82, 124, 174]),
    (306, "Storm blue", 12, [51, 88, 130]),
    (307, "Lapis", 13, [16, 42, 220]),
    (308, "Dark indigo", 14, [61, 21, 133]),
    (309, "Sea green", 16, [52, 142, 64]),
    (310, "Shamrock", 17, [91, 154, 76]),
    (311, "Fossil", 29, [159, 161, 172]),
    (312, "Mulberry", 23, [89, 34, 89]),
    (313, "Forest green", 24, [31, 128, 29]),
    (314, "Cadet blue", 39, [159, 173, 192]),
    (315, "Electric blue", 30, [9, 137, 207]),
    (316, "Eggplant", 33, [123, 0, 123]),
    (317, "Moss", 35, [124, 156, 107]),
    (318, "Artichoke", 36, [138, 171, 133]),
    (319, "Sage green", 37, [185, 196, 177]),
    (320, "Ghost grey", 50, [202, 203, 209]),
    (321, "Lilac", 42, [167, 94, 155]),
    (322, "Plum", 43, [123, 47, 123]),
    (323, "Olivine", 47, [148, 190, 129]),
    (324, "Laurel green", 48, [168, 189, 153]),
    (325, "Quill grey", 49, [223, 223, 222]),
    (327, "Crimson", 56, [151, 0, 0]),
    (328, "Mint", 60, [177, 229, 166]),
    (329, "Baby blue", 64, [152, 194, 219]),
    (330, "Carnation pink", 65, [255, 152, 220]),
    (331, "Persimmon", 66, [255, 89, 89]),
    (332, "Maroon", 69, [117, 0, 0]),
    (333, "Gold", 70, [239, 184, 56]),
    (334, "Daisy orange", 72, [248, 217, 109]),
    (335, "Pearl", 75, [231, 231, 236]),
    (336, "Fog", 76, [199, 212, 228]),
    (337, "Salmon", 90, [255, 148, 148]),
    (338, "Terra Cotta", 79, [190, 104, 98]),
    (339, "Cocoa", 81, [86, 36, 36]),
    (340, "Wheat", 84, [241, 231, 199]),
    (341, "Buttermilk", 85, [254, 243, 187]),
    (342, "Mauve", 77, [224, 178, 208]),
    (343, "Sunrise", 78, [212, 144, 189]),
    (344, "Tawny", 91, [150, 85, 85]),
    (345, "Rust", 92, [143, 76, 42]),
    (346, "Cashmere", 95, [211, 190, 150]),
    (347, "Khaki", 96, [226, 220, 188]),
    (348, "Lily white", 97, [237, 234, 234]),
    (349, "Seashell", 98, [233, 218, 218]),
    (350, "Burgundy", 102, [136, 62, 62]),
    (351, "Cork", 104, [188, 155, 93]),
    (352, "Burlap", 105, [199, 172, 120]),
    (353, "Beige", 106, [202, 191, 163]),
    (354, "Oyster", 107, [187, 179, 178]),
    (355, "Pine Cone", 112, [108, 88, 75]),
    (356, "Fawn brown", 113, [160, 132, 79]),
    (357, "Hurricane grey", 115, [149, 137, 136]),
    (358, "Cloudy grey", 116, [171, 168, 158]),
    (359, "Linen", 117, [175, 148, 131]),
    (360, "Copper", 118, [150, 103, 102]),
    (361, "Medium brown", 120, [86, 66, 54]),
    (362, "Bronze", 121, [126, 104, 63]),
    (363, "Flint", 124, [105, 102, 92]),
    (364, "Dark taupe", 125, [90, 76, 66]),
    (365, "Burnt Sienna", 126, [106, 57, 9]),
    (1001, "Institutional white", 86, [248, 248, 248]),
    (1002, "Mid gray", 108, [205, 205, 205]),
    (1003, "Really black", 127, [17, 17, 17]),
    (1004, "Really red", 67, [255, 0, 0]),
    (1005, "Deep orange", 57, [255, 176, 0]),
    (1006, "Alder", 41, [180, 128, 255]),
    (1007, "Dusty Rose", 80, [163, 75, 75]),
    (1008, "Olive", 45, [193, 190, 66]),
    (1009, "New Yeller", 58, [255, 255, 0]),
    (1010, "Really blue", 22, [0, 0, 255]),
    (1011, "Navy blue", 6, [0, 32, 96]),
    (1012, "Deep blue", 4, [33, 84, 185]),
    (1013, "Cyan", 40, [4, 175, 236]),
    (1014, "CGA brown", 93, [170, 85, 0]),
    (1015, "Magenta", 55, [170, 0, 170]),
    (1016, "Pink", 53, [255, 102, 204]),
    (1017, "Deep orange", -1, [255, 175, 0]),
    (1018, "Teal", 9, [18, 238, 212]),
    (1019, "Toothpaste", 18, [0, 255, 255]),
    (1020, "Lime green", 27, [0, 255, 0]),
    (1021, "Camo", 15, [58, 125, 21]),
    (1022, "Grime", 26, [127, 142, 100]),
    (1023, "Lavender", 31, [140, 91, 159]),
    (1024, "Pastel light blue", 38, [175, 221, 255]),
    (1025, "Pastel orange", 89, [255, 201, 201]),
    (1026, "Pastel violet", 52, [177, 167, 255]),
    (1027, "Pastel blue-green", 28, [159, 243, 233]),
    (1028, "Pastel green", 61, [204, 255, 204]),
    (1029, "Pastel yellow", 74, [255, 255, 204]),
    (1030, "Pastel brown", 99, [255, 204, 153]),
    (1031, "Royal purple", 32, [98, 37, 209]),
    (1032, "Hot pink", 54, [255, 0, 191]),
];

/// The default, for an unknown name or number: 194, "Medium stone grey".
const DEFAULT_BRICK: u16 = 194;

/// A named color from Roblox's legacy palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrickColor {
    /// Its row in BRICK_COLORS.
    index: usize,
}

impl Default for BrickColor {
    fn default() -> BrickColor {
        BrickColor::new(DEFAULT_BRICK.into())
    }
}

impl BrickColor {
    /// BrickColor.new(number); an unknown number gives Medium stone grey.
    pub fn new(number: i64) -> BrickColor {
        let find = |n: i64| BRICK_COLORS.binary_search_by_key(&n, |row| row.0.into()).ok();
        let index = find(number).or_else(|| find(DEFAULT_BRICK.into())).unwrap_or(0);
        BrickColor { index }
    }

    /// BrickColor.new(name): the exact name (the lowest number when two share
    /// it); an unknown name gives Medium stone grey.
    pub fn from_name(name: &str) -> BrickColor {
        match BRICK_COLORS.iter().position(|row| row.1 == name) {
            Some(index) => BrickColor { index },
            None => BrickColor::default(),
        }
    }

    /// BrickColor.new(r, g, b) with channels 0 to 1: the color nearest it, by
    /// the sum of the channels' differences (ties to the lower number).
    pub fn from_rgb(r: f64, g: f64, b: f64) -> BrickColor {
        let distance = |rgb: &[u8; 3]| {
            (f64::from(rgb[0]) / 255.0 - r).abs()
                + (f64::from(rgb[1]) / 255.0 - g).abs()
                + (f64::from(rgb[2]) / 255.0 - b).abs()
        };
        let mut best = (0, f64::INFINITY);
        for (index, row) in BRICK_COLORS.iter().enumerate() {
            let d = distance(&row.3);
            if d < best.1 {
                best = (index, d);
            }
        }
        BrickColor { index: best.0 }
    }

    /// BrickColor.new(color): the nearest, as `from_rgb`.
    pub fn from_color3(c: Color3) -> BrickColor {
        BrickColor::from_rgb(c.r, c.g, c.b)
    }

    /// BrickColor.palette(i): the i-th of the 128-color palette, from 0.
    pub fn palette(i: i64) -> Result<BrickColor, String> {
        BRICK_COLORS
            .iter()
            .position(|row| i64::from(row.2) == i && row.2 >= 0)
            .map(|index| BrickColor { index })
            .ok_or_else(|| format!("BrickColor.palette: {i} is not a palette index (0 to 127)"))
    }

    /// BrickColor.random(): one of the 128-color palette.
    pub fn random(rng: &mut Random) -> BrickColor {
        let i = rng.next_integer(0.0, 127.0);
        BrickColor::palette(i).unwrap_or_default()
    }

    /// Every BrickColor, by number.
    pub fn all() -> impl Iterator<Item = BrickColor> {
        (0..BRICK_COLORS.len()).map(|index| BrickColor { index })
    }

    /// BrickColor.White(): 1, "White".
    pub fn white() -> BrickColor {
        BrickColor::new(1)
    }

    /// BrickColor.Gray(): 194, "Medium stone grey".
    pub fn gray() -> BrickColor {
        BrickColor::new(194)
    }

    /// BrickColor.DarkGray(): 199, "Dark stone grey".
    pub fn dark_gray() -> BrickColor {
        BrickColor::new(199)
    }

    /// BrickColor.Black(): 26, "Black".
    pub fn black() -> BrickColor {
        BrickColor::new(26)
    }

    /// BrickColor.Red(): 21, "Bright red".
    pub fn red() -> BrickColor {
        BrickColor::new(21)
    }

    /// BrickColor.Yellow(): 24, "Bright yellow".
    pub fn yellow() -> BrickColor {
        BrickColor::new(24)
    }

    /// BrickColor.Green(): 28, "Dark green".
    pub fn green() -> BrickColor {
        BrickColor::new(28)
    }

    /// BrickColor.Blue(): 23, "Bright blue".
    pub fn blue() -> BrickColor {
        BrickColor::new(23)
    }

    /// Number.
    pub fn number(self) -> u16 {
        BRICK_COLORS[self.index].0
    }

    /// Name.
    pub fn name(self) -> &'static str {
        BRICK_COLORS[self.index].1
    }

    /// Its index in BrickColor.palette, if it is one of those 128.
    pub fn palette_index(self) -> Option<u8> {
        u8::try_from(BRICK_COLORS[self.index].2).ok()
    }

    /// Color.
    pub fn color(self) -> Color3 {
        let [r, g, b] = BRICK_COLORS[self.index].3;
        Color3::from_rgb(r.into(), g.into(), b.into())
    }

    /// r, g and b: the Color's channels.
    pub fn r(self) -> f64 {
        self.color().r
    }

    pub fn g(self) -> f64 {
        self.color().g
    }

    pub fn b(self) -> f64 {
        self.color().b
    }
}

/// Its name: "Pastel Blue".
impl fmt::Display for BrickColor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------- Font

/// A font face: a family asset, a weight and a style (TextLabel.FontFace).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Font {
    /// The family's asset id: "rbxasset://fonts/families/SourceSansPro.json".
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
}

impl Default for Font {
    /// A text object's FontFace from Instance.new: Legacy Arial.
    fn default() -> Font {
        Font::from_name("LegacyArial", FontWeight::Regular, FontStyle::Normal)
    }
}

impl Font {
    /// Font.new(family, weight, style).
    pub fn new(family: &str, weight: FontWeight, style: FontStyle) -> Font {
        Font {
            family: family.to_string(),
            weight,
            style,
        }
    }

    /// Font.fromName(name): `rbxasset://fonts/families/<name>.json`.
    pub fn from_name(name: &str, weight: FontWeight, style: FontStyle) -> Font {
        Font::new(&format!("rbxasset://fonts/families/{name}.json"), weight, style)
    }

    /// Font.fromId(assetId): `rbxassetid://<id>`.
    pub fn from_id(id: u64, weight: FontWeight, style: FontStyle) -> Font {
        Font::new(&format!("rbxassetid://{id}"), weight, style)
    }

    /// Font.fromEnum: the face an Enum.Font stands for; Unknown has none.
    pub fn from_enum(font: FontEnum) -> Result<Font, String> {
        let (family, weight, style) = font
            .face()
            .ok_or_else(|| "Font.fromEnum: Enum.Font.Unknown has no font".to_string())?;
        Ok(Font::from_name(family, weight, style))
    }

    /// The Enum.Font a text object's Font property reads for this face:
    /// the first whose face it is, or Unknown.
    pub fn to_enum(&self) -> FontEnum {
        let same = |e: &FontEnum| Font::from_enum(*e).is_ok_and(|f| f == *self);
        FontEnum::ALL.iter().copied().find(same).unwrap_or(FontEnum::Unknown)
    }

    /// Bold: whether the weight is Bold or heavier.
    pub fn bold(&self) -> bool {
        self.weight.value() >= FontWeight::Bold.value()
    }

    /// Setting Bold: the weight becomes Bold, or Regular.
    pub fn set_bold(&mut self, bold: bool) {
        self.weight = if bold { FontWeight::Bold } else { FontWeight::Regular };
    }
}

/// "Font { Family = rbxasset://fonts/families/X.json, Weight = Regular, Style = Normal }".
impl fmt::Display for Font {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (weight, style) = (self.weight.name(), self.style.name());
        write!(
            f,
            "Font {{ Family = {}, Weight = {weight}, Style = {style} }}",
            self.family
        )
    }
}

// ---------------------------------------------------------------- Random

/// PCG32's multiplier, and the stream Luau's math.random uses (its
/// increment is `(105 << 1) | 1`).
const PCG_MULTIPLIER: u64 = 6364136223846793005;
const LUAU_STREAM: u64 = 105;

/// A seed from the clock, a counter and std's random hash keys.
fn entropy() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    RandomState::new().hash_one((count, nanos))
}

/// Random: a pseudorandom generator. It is PCG32 (O'Neill's XSH-RR, the
/// generator Luau's math.random uses), seeded as Luau seeds it; Roblox
/// doesn't document its own, so its sequences aren't reproduced. Clone
/// gives an independent copy that continues the same sequence.
#[derive(Clone, Debug)]
pub struct Random {
    state: u64,
    increment: u64,
}

impl Random {
    /// Random.new(seed): the seed is floored; outside ±(2^53 - 1) it is 0.
    pub fn new(seed: f64) -> Random {
        let limit = 9007199254740991.0;
        let seed = if seed.abs() <= limit { seed.floor() as i64 } else { 0 };
        Random::pcg32(seed as u64, LUAU_STREAM)
    }

    /// Random.new() without a seed: one from an entropy source.
    pub fn from_entropy() -> Random {
        Random::pcg32(entropy(), LUAU_STREAM)
    }

    /// PCG32 seeded as the reference `pcg32_srandom_r(initstate, initseq)`
    /// seeds it; `new` uses Luau's stream, 105.
    pub fn pcg32(init_state: u64, init_seq: u64) -> Random {
        let mut rng = Random {
            state: 0,
            increment: (init_seq << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(init_state);
        rng.next_u32();
        rng
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULTIPLIER).wrapping_add(self.increment);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    fn next_u64(&mut self) -> u64 {
        (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
    }

    /// Uniform in 0..n, unbiased (Lemire's method); n is over 0.
    fn below(&mut self, n: u64) -> u64 {
        if n <= 1 << 32 {
            let mut m = u64::from(self.next_u32()) * n;
            if (m as u32 as u64) < n {
                let threshold = (1u64 << 32) % n;
                while (m as u32 as u64) < threshold {
                    m = u64::from(self.next_u32()) * n;
                }
            }
            return m >> 32;
        }
        let mut m = u128::from(self.next_u64()) * u128::from(n);
        if (m as u64) < n {
            let threshold = n.wrapping_neg() % n;
            while (m as u64) < threshold {
                m = u128::from(self.next_u64()) * u128::from(n);
            }
        }
        (m >> 64) as u64
    }

    /// NextInteger(min, max): uniform over min..=max, both truncated toward
    /// zero first and swapped if min is over max.
    pub fn next_integer(&mut self, min: f64, max: f64) -> i64 {
        let (mut lo, mut hi) = (min.trunc() as i64, max.trunc() as i64);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        let span = (hi as u64).wrapping_sub(lo as u64).wrapping_add(1);
        if span == 0 {
            return self.next_u64() as i64;
        }
        (lo as u64).wrapping_add(self.below(span)) as i64
    }

    /// NextNumber(): uniform over 0 to 1, both included.
    pub fn next_number(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) - 1) as f64
    }

    /// NextNumber(min, max): uniform over min to max.
    pub fn next_number_between(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.next_number()
    }

    /// NextUnitVector: a direction uniform over the sphere.
    pub fn next_unit_vector(&mut self) -> Vector3 {
        let z = self.next_number_between(-1.0, 1.0);
        let angle = self.next_number() * TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vector3::new(r * angle.cos(), r * angle.sin(), z)
    }

    /// Shuffle: Fisher-Yates in place, one NextInteger per item after the first.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.next_integer(0.0, i as f64) as usize;
            items.swap(i, j);
        }
    }
}
