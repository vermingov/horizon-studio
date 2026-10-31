// hstudio_quads, the geometry behind GameApi's dp_clip, dp_rotate and
// dp_shade in the game: quads cut to a rectangle with their texture and
// colours, turned, and coloured by gradients of any number of stops, which
// cut them into bands where the stops fall.

use horizon_studio::game::hstudio_quads::{Corner, Quad, Shade, blend, clip, multiply, rotate, shade, turned};

/// A rectangle from (x0, y0) to (x1, y1) in one colour, its texture from
/// (0, 0) to (1, 1).
fn rect(x0: f32, y0: f32, x1: f32, y1: f32, argb: u32) -> Quad {
    let k = |x: f32, y: f32, u: f32, v: f32| Corner {
        at: [x, y],
        uv: [u, v],
        argb,
    };
    [
        k(x0, y0, 0.0, 0.0),
        k(x1, y0, 1.0, 0.0),
        k(x1, y1, 1.0, 1.0),
        k(x0, y1, 0.0, 1.0),
    ]
}

/// Twice the area the quads cover (each is drawn as triangles 0 1 2 and 0 2 3).
fn area(quads: &[Quad]) -> f32 {
    let tri =
        |a: [f32; 2], b: [f32; 2], c: [f32; 2]| ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs();
    quads
        .iter()
        .map(|q| tri(q[0].at, q[1].at, q[2].at) + tri(q[0].at, q[2].at, q[3].at))
        .sum()
}

/// Every corner of the quads.
fn corners(quads: &[Quad]) -> Vec<Corner> {
    quads.iter().flatten().copied().collect()
}

#[track_caller]
fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} is not {b}");
}

#[test]
fn colours_blend_and_multiply_channel_by_channel() {
    assert_eq!(blend(0xFF00_0000, 0xFFFF_FFFF, 0.5), 0xFF80_8080);
    assert_eq!(blend(0x0000_00FF, 0xFF00_0000, 0.0), 0x0000_00FF);
    assert_eq!(multiply(0xFFFF_FFFF, 0x80FF_8000), 0x80FF_8000);
    assert_eq!(multiply(0xFF80_8080, 0xFF80_0000), 0xFF40_0000);
}

#[test]
fn clipping_keeps_what_is_inside_and_cuts_the_rest() {
    // Wholly inside: as it was. Wholly outside: gone.
    let inside = rect(10.0, 10.0, 20.0, 20.0, 0xFFFF_0000);
    assert_eq!(clip(&[inside], [0.0, 0.0, 100.0, 100.0]), vec![inside]);
    assert!(clip(&[rect(200.0, 0.0, 300.0, 10.0, 0xFF00_0000)], [0.0, 0.0, 100.0, 100.0]).is_empty());
    // Half out on the right: cut at x = 50, its texture with it.
    let cut = clip(&[rect(0.0, 0.0, 100.0, 10.0, 0xFFFF_FFFF)], [0.0, 0.0, 50.0, 10.0]);
    near(area(&cut), 2.0 * 50.0 * 10.0);
    for k in corners(&cut) {
        assert!(k.at[0] <= 50.0 + 1e-4);
        near(k.uv[0], k.at[0] / 100.0);
        near(k.uv[1], k.at[1] / 10.0);
        assert_eq!(k.argb, 0xFFFF_FFFF);
    }
    // A diamond (a square turned 45 degrees) cut by a box round its middle
    // keeps an octagon: its corners lie on the box or on the diamond.
    let d = |x: f32, y: f32| Corner {
        at: [x, y],
        uv: [x / 20.0, y / 20.0],
        argb: 0xFF00_FF00,
    };
    let diamond = [d(10.0, 0.0), d(20.0, 10.0), d(10.0, 20.0), d(0.0, 10.0)];
    let cut = clip(&[diamond], [4.0, 4.0, 16.0, 16.0]);
    // The box (144) less four corner triangles of 2 x 2 / 2 each.
    near(area(&cut), 2.0 * (144.0 - 4.0 * 2.0));
    for k in corners(&cut) {
        let on_box = [4.0, 16.0]
            .iter()
            .any(|e| (k.at[0] - e).abs() < 1e-3 || (k.at[1] - e).abs() < 1e-3);
        assert!(on_box, "{k:?}");
        near(k.uv[0], k.at[0] / 20.0);
    }
}

#[test]
fn colours_are_cut_with_the_quad() {
    // Red on the left, blue on the right; cut in the middle, the new right
    // edge is half way between.
    let mut q = rect(0.0, 0.0, 10.0, 10.0, 0);
    for (i, c) in [0xFFFF_0000, 0xFF00_00FF, 0xFF00_00FF, 0xFFFF_0000]
        .into_iter()
        .enumerate()
    {
        q[i].argb = c;
    }
    let cut = clip(&[q], [0.0, 0.0, 5.0, 10.0]);
    let right: Vec<u32> = corners(&cut)
        .iter()
        .filter(|k| k.at[0] == 5.0)
        .map(|k| k.argb)
        .collect();
    assert!(!right.is_empty());
    assert!(right.iter().all(|&c| c == 0xFF80_0080), "{right:x?}");
}

#[test]
fn turning_goes_clockwise_on_the_screen() {
    // y goes down, so a quarter turn takes a point on the right to below.
    let p = turned([11.0, 5.0], [10.0, 5.0], 90f32.to_radians().sin_cos());
    near(p[0], 10.0);
    near(p[1], 6.0);
    let mut q = [rect(0.0, 0.0, 2.0, 2.0, 0xFFFF_FFFF)];
    rotate(&mut q, [1.0, 1.0], 180.0);
    near(q[0][0].at[0], 2.0);
    near(q[0][0].at[1], 2.0);
    assert_eq!(q[0][0].uv, [0.0, 0.0], "the texture turns with it");
}

/// Red, green in the middle, blue: along x over 0 to 100.
fn rainbow() -> Shade {
    Shade {
        axis: [0.01, 0.0],
        base: 0.0,
        stops: vec![(0.0, 0xFFFF_0000), (0.5, 0xFF00_FF00), (1.0, 0xFF00_00FF)],
    }
}

#[test]
fn a_gradient_blends_between_its_stops_and_holds_past_them() {
    let s = rainbow();
    assert_eq!(s.at([0.0, 7.0]), 0xFFFF_0000);
    assert_eq!(s.at([50.0, 0.0]), 0xFF00_FF00);
    assert_eq!(s.at([25.0, 0.0]), 0xFF80_8000);
    assert_eq!(s.at([100.0, 0.0]), 0xFF00_00FF);
    assert_eq!(s.at([-40.0, 0.0]), 0xFFFF_0000);
    assert_eq!(s.at([140.0, 0.0]), 0xFF00_00FF);
    // In units twice as large, the same place is half the number.
    assert_eq!(s.in_units(2.0).at([25.0, 0.0]), 0xFF00_FF00);
    let none = Shade {
        axis: [1.0, 0.0],
        base: 0.0,
        stops: Vec::new(),
    };
    assert_eq!(none.at([3.0, 3.0]), 0xFFFF_FFFF);
}

#[test]
fn a_gradient_of_three_stops_shows_its_middle_colour() {
    // One quad across the whole gradient: a quad blends only between its
    // corners, so it is cut at the middle stop, and the cut is green.
    let shaded = shade(&[rect(0.0, 0.0, 100.0, 20.0, 0xFFFF_FFFF)], &rainbow());
    near(area(&shaded), 2.0 * 100.0 * 20.0);
    let all = corners(&shaded);
    let at = |x: f32| -> Vec<u32> {
        all.iter()
            .filter(|k| (k.at[0] - x).abs() < 1e-3)
            .map(|k| k.argb)
            .collect()
    };
    assert!(at(0.0).iter().all(|&c| c == 0xFFFF_0000), "{:x?}", at(0.0));
    assert!(
        !at(50.0).is_empty() && at(50.0).iter().all(|&c| c == 0xFF00_FF00),
        "{:x?}",
        at(50.0)
    );
    assert!(at(100.0).iter().all(|&c| c == 0xFF00_00FF), "{:x?}", at(100.0));
    // No corner of a band lies past a stop it doesn't start or end at.
    for q in &shaded {
        let xs: Vec<f32> = q.iter().map(|k| k.at[0]).collect();
        let (lo, hi) = (
            xs.iter().copied().fold(f32::MAX, f32::min),
            xs.iter().copied().fold(f32::MIN, f32::max),
        );
        assert!(
            hi <= 50.0 + 1e-3 || lo >= 50.0 - 1e-3,
            "a band across the middle stop: {xs:?}"
        );
    }
    // The gradient multiplies what is drawn: half-grey stays half as bright.
    let grey = shade(&[rect(0.0, 0.0, 100.0, 20.0, 0xFF80_8080)], &rainbow());
    assert!(
        corners(&grey)
            .iter()
            .filter(|k| k.at[0] == 50.0)
            .all(|k| k.argb == 0xFF00_8000)
    );
}

#[test]
fn two_stops_at_one_place_make_a_hard_edge() {
    let s = Shade {
        axis: [0.1, 0.0],
        base: 0.0,
        stops: vec![
            (0.0, 0xFFFF_0000),
            (0.5, 0xFFFF_0000),
            (0.5, 0xFF00_00FF),
            (1.0, 0xFF00_00FF),
        ],
    };
    let shaded = shade(&[rect(0.0, 0.0, 10.0, 4.0, 0xFFFF_FFFF)], &s);
    for q in &shaded {
        let left = q.iter().all(|k| k.at[0] <= 5.0 + 1e-4);
        let want = if left { 0xFFFF_0000 } else { 0xFF00_00FF };
        assert!(q.iter().all(|k| k.argb == want), "{q:?}");
    }
    // A quad that starts on the edge is wholly the colour after it.
    let after = shade(&[rect(5.0, 0.0, 8.0, 4.0, 0xFFFF_FFFF)], &s);
    assert!(corners(&after).iter().all(|k| k.argb == 0xFF00_00FF));
}

#[test]
fn a_turned_gradient_cuts_across() {
    // Down the screen: t is y / 10, so the cut at 0.5 is the line y = 5.
    let s = Shade {
        axis: [0.0, 0.1],
        base: 0.0,
        stops: vec![(0.0, 0xFF00_0000), (0.5, 0xFFFF_FFFF), (1.0, 0xFF00_0000)],
    };
    let shaded = shade(&[rect(0.0, 0.0, 30.0, 10.0, 0xFFFF_FFFF)], &s);
    let middle: Vec<u32> = corners(&shaded)
        .iter()
        .filter(|k| k.at[1] == 5.0)
        .map(|k| k.argb)
        .collect();
    assert!(!middle.is_empty() && middle.iter().all(|&c| c == 0xFFFF_FFFF));
    assert!(
        corners(&shaded)
            .iter()
            .filter(|k| k.at[1] == 0.0)
            .all(|k| k.argb == 0xFF00_0000)
    );
    near(area(&shaded), 2.0 * 300.0);
}
