// The HUD's quads cut, turned and coloured (hstudio_quads), for GameApi's
// dp_clip, dp_rotate and dp_shade: the bridge hands this module what the
// game drew since a mark and draws what comes back in its place. It is plain
// geometry, apart from any game, so it is tested on its own. A quad's
// texture coordinates and colours are cut with it, and a gradient is cut
// into bands where its stops fall, since a quad only blends between its
// corners.

/// A quad's corner: where it is, its place in the texture, and its colour
/// (0xAARRGGBB), which multiplies the texture's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corner {
    pub at: [f32; 2],
    pub uv: [f32; 2],
    pub argb: u32,
}

/// Four corners in order round a convex shape, drawn as the triangles
/// 0 1 2 and 0 2 3 (a triangle repeats its last corner).
pub type Quad = [Corner; 4];

/// A gradient over a drawing, as a UIGradient colours its object: at a
/// point p its colour is that of `stops` at t = p · axis + base, a blend of
/// the two stops around t, and the first or the last stop's colour beyond
/// them. Each stop is (t, 0xAARRGGBB), in order of t; two stops at one t
/// make a hard edge. Its colour multiplies what is drawn there.
#[derive(Clone, Debug, PartialEq)]
pub struct Shade {
    pub axis: [f32; 2],
    pub base: f32,
    pub stops: Vec<(f32, u32)>,
}

impl Shade {
    /// The gradient's t at `p`.
    pub fn t(&self, p: [f32; 2]) -> f32 {
        p[0] * self.axis[0] + p[1] * self.axis[1] + self.base
    }

    /// Its colour at `p` (at a hard edge, the colour before it); white
    /// without stops.
    pub fn at(&self, p: [f32; 2]) -> u32 {
        let t = self.t(p);
        let (Some(first), Some(last)) = (self.stops.first(), self.stops.last()) else {
            return 0xFFFF_FFFF;
        };
        if t <= first.0 {
            return first.1;
        }
        self.stops
            .windows(2)
            .find(|w| t <= w[1].0)
            .map_or(last.1, |w| between(w[0], w[1], t))
    }

    /// The same gradient for points given in units of `k`: where a point
    /// was `p`, it is now `p / k` (the bridge's GUI pixels, `k` dp each).
    pub fn in_units(&self, k: f32) -> Shade {
        Shade {
            axis: [self.axis[0] * k, self.axis[1] * k],
            base: self.base,
            stops: self.stops.clone(),
        }
    }

    /// Its colour at t inside a band that starts at `from` (no stop lies
    /// between them): the stops on either side of the band blended, so a
    /// hard edge at the band's start gives the colour after it.
    fn in_band(&self, from: f32, t: f32) -> u32 {
        let (Some(first), Some(last)) = (self.stops.first(), self.stops.last()) else {
            return 0xFFFF_FFFF;
        };
        if from < first.0 {
            return first.1;
        }
        // The last stop at or before the band's start, and the one after it.
        let i = self.stops.iter().rposition(|s| s.0 <= from).unwrap_or(0);
        match self.stops.get(i + 1) {
            Some(&next) => between(self.stops[i], next, t),
            None => last.1,
        }
    }
}

/// The colour `t` of the way between two stops.
fn between(a: (f32, u32), b: (f32, u32), t: f32) -> u32 {
    let f = if b.0 > a.0 {
        ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
    } else {
        1.0
    };
    blend(a.1, b.1, f)
}

/// `a` blended `f` of the way to `b`, channel by channel.
pub fn blend(a: u32, b: u32, f: f32) -> u32 {
    let (a, b) = (a.to_be_bytes(), b.to_be_bytes());
    let mix = |i: usize| (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * f).round() as u8;
    u32::from_be_bytes([mix(0), mix(1), mix(2), mix(3)])
}

/// Two colours multiplied, channel by channel (white changes nothing).
pub fn multiply(a: u32, b: u32) -> u32 {
    let (a, b) = (a.to_be_bytes(), b.to_be_bytes());
    let mul = |i: usize| ((u32::from(a[i]) * u32::from(b[i]) + 127) / 255) as u8;
    u32::from_be_bytes([mul(0), mul(1), mul(2), mul(3)])
}

/// A corner `f` of the way from `a` to `b`: its place, texture and colour.
fn lerp(a: &Corner, b: &Corner, f: f32) -> Corner {
    let mix = |x: f32, y: f32| x + (y - x) * f;
    Corner {
        at: [mix(a.at[0], b.at[0]), mix(a.at[1], b.at[1])],
        uv: [mix(a.uv[0], b.uv[0]), mix(a.uv[1], b.uv[1])],
        argb: blend(a.argb, b.argb, f),
    }
}

/// The part of a convex shape where `normal · p + c >= 0` (Sutherland and
/// Hodgman's cut against one edge).
fn cut(shape: &[Corner], normal: [f32; 2], c: f32) -> Vec<Corner> {
    let side = |k: &Corner| normal[0] * k.at[0] + normal[1] * k.at[1] + c;
    let mut out: Vec<Corner> = Vec::with_capacity(shape.len() + 2);
    let keep = |k: Corner, out: &mut Vec<Corner>| {
        if out.last().is_none_or(|l| l.at != k.at) {
            out.push(k);
        }
    };
    for (i, a) in shape.iter().enumerate() {
        let b = &shape[(i + 1) % shape.len()];
        let (da, db) = (side(a), side(b));
        if da >= 0.0 {
            keep(*a, &mut out);
        }
        if (da >= 0.0) != (db >= 0.0) {
            keep(lerp(a, b, da / (da - db)), &mut out);
        }
    }
    if out.len() > 1 && out.first().map(|k| k.at) == out.last().map(|k| k.at) {
        out.pop();
    }
    out
}

/// A convex shape as quads, a fan of triangles from its first corner two
/// at a time; fewer than three corners make nothing.
fn fan(shape: &[Corner], out: &mut Vec<Quad>) {
    let mut i = 1;
    while i + 1 < shape.len() {
        let last = shape[(i + 2).min(shape.len() - 1)];
        out.push([shape[0], shape[i], shape[i + 1], last]);
        i += 2;
    }
}

/// The quads cut to the rectangle from (x0, y0) to (x1, y1): what is
/// outside it goes, and what crosses its edges is cut there, with its
/// texture and colours.
pub fn clip(quads: &[Quad], [x0, y0, x1, y1]: [f32; 4]) -> Vec<Quad> {
    let edges = [
        ([1.0, 0.0], -x0),
        ([-1.0, 0.0], x1),
        ([0.0, 1.0], -y0),
        ([0.0, -1.0], y1),
    ];
    let mut out = Vec::with_capacity(quads.len());
    for q in quads {
        let inside = q
            .iter()
            .all(|k| k.at[0] >= x0 && k.at[0] <= x1 && k.at[1] >= y0 && k.at[1] <= y1);
        if inside {
            out.push(*q);
            continue;
        }
        let mut shape = q.to_vec();
        for (normal, c) in edges {
            shape = cut(&shape, normal, c);
        }
        fan(&shape, &mut out);
    }
    out
}

/// `at` turned about `centre` by the angle whose sine and cosine are
/// given: clockwise on a screen whose y goes down, for a positive angle.
pub fn turned(at: [f32; 2], centre: [f32; 2], (sin, cos): (f32, f32)) -> [f32; 2] {
    let (dx, dy) = (at[0] - centre[0], at[1] - centre[1]);
    [centre[0] + dx * cos - dy * sin, centre[1] + dx * sin + dy * cos]
}

/// The quads turned by `degrees` clockwise about `centre`.
pub fn rotate(quads: &mut [Quad], centre: [f32; 2], degrees: f32) {
    let sc = degrees.to_radians().sin_cos();
    for k in quads.iter_mut().flatten() {
        k.at = turned(k.at, centre, sc);
    }
}

/// The quads coloured by `shade`. Each is first cut into bands where the
/// gradient's stops fall across it, so each band blends between two of
/// them as its corners do, then each corner's colour is multiplied by the
/// gradient's there.
pub fn shade(quads: &[Quad], shade: &Shade) -> Vec<Quad> {
    let mut out = Vec::with_capacity(quads.len());
    let [ax, ay] = shade.axis;
    for q in quads {
        let ts = q.map(|k| shade.t(k.at));
        let lo = ts.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = ts.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut bounds = vec![f32::NEG_INFINITY];
        for &(t, _) in &shade.stops {
            if t > lo && t < hi && bounds.last().is_none_or(|&b| t > b) {
                bounds.push(t);
            }
        }
        bounds.push(f32::INFINITY);
        for band in bounds.windows(2) {
            let (from, to) = (band[0], band[1]);
            let mut shape = q.to_vec();
            if from.is_finite() {
                shape = cut(&shape, [ax, ay], shade.base - from);
            }
            if to.is_finite() {
                shape = cut(&shape, [-ax, -ay], to - shade.base);
            }
            // Where the band starts in this quad, which says which stops it
            // lies between (a corner on its edge may round past it).
            let start = if from.is_finite() { from } else { lo };
            for k in &mut shape {
                k.argb = multiply(k.argb, shade.in_band(start, shade.t(k.at)));
            }
            fan(&shape, &mut out);
        }
    }
    out
}
