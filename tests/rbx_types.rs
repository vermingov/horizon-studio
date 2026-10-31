// hstudio_rbx_types against values worked out by hand and from Roblox's own
// docs and API dump: vector math, CFrame compositions, inverses, Euler and
// axis-angle round trips, lookAt, slerped lerps, Color3's HSV and hex, the
// BrickColor palette, Font's enum mapping, sequences and their checks,
// TweenService's easing curves, Random, and how each type prints.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use horizon_studio::game::hstudio_rbx_types::{
    Axis, BrickColor, CFrame, Color3, ColorSequence, ColorSequenceKeypoint, EasingDirection, EasingStyle, Font,
    FontEnum, FontStyle, FontWeight, MAX_KEYPOINTS, NormalId, NumberRange, NumberSequence, NumberSequenceKeypoint,
    Random, Rect, Region3, RotationOrder, TweenInfo, UDim, UDim2, Vector2, Vector3, ease, float32,
};

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

#[track_caller]
fn assert_close(a: f64, b: f64) {
    assert!(close(a, b), "{a} is not {b}");
}

#[track_caller]
fn assert_v3(v: Vector3, x: f64, y: f64, z: f64) {
    assert!(
        close(v.x, x) && close(v.y, y) && close(v.z, z),
        "{v:?} is not ({x}, {y}, {z})"
    );
}

#[track_caller]
fn assert_cf(a: CFrame, b: CFrame) {
    let (ca, cb) = (a.get_components(), b.get_components());
    assert!(
        ca.iter().zip(cb).all(|(x, y)| (x - y).abs() <= 1e-9),
        "{a}\n  is not\n{b}"
    );
}

fn v3(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

// ---------------------------------------------------------------- Vector2, Vector3

#[test]
fn vector_constants() {
    assert_v3(Vector3::ZERO, 0.0, 0.0, 0.0);
    assert_v3(Vector3::ONE, 1.0, 1.0, 1.0);
    assert_v3(Vector3::X_AXIS, 1.0, 0.0, 0.0);
    assert_v3(Vector3::Y_AXIS, 0.0, 1.0, 0.0);
    assert_v3(Vector3::Z_AXIS, 0.0, 0.0, 1.0);
    assert_eq!(Vector2::ZERO, Vector2::new(0.0, 0.0));
    assert_eq!(Vector2::ONE, Vector2::new(1.0, 1.0));
    assert_eq!(Vector2::X_AXIS, Vector2::new(1.0, 0.0));
    assert_eq!(Vector2::Y_AXIS, Vector2::new(0.0, 1.0));
    assert_eq!(Vector3::default(), Vector3::ZERO);
}

#[test]
fn vector3_magnitude_and_unit() {
    let v = v3(3.0, 4.0, 0.0);
    assert_close(v.magnitude(), 5.0);
    assert_v3(v.unit(), 0.6, 0.8, 0.0);
    assert_close(v3(1.0, 2.0, 2.0).magnitude(), 3.0);
    assert_close(v3(-7.0, 0.5, 11.0).unit().magnitude(), 1.0);
    // The zero vector's Unit is NaN, as the docs say.
    let u = Vector3::ZERO.unit();
    assert!(u.x.is_nan() && u.y.is_nan() && u.z.is_nan());
    assert!(Vector2::ZERO.unit().x.is_nan());
    assert_close(Vector2::new(3.0, 4.0).magnitude(), 5.0);
}

#[test]
fn vector3_arithmetic() {
    let (a, b) = (v3(1.0, 2.0, 3.0), v3(4.0, -5.0, 6.0));
    assert_eq!(a + b, v3(5.0, -3.0, 9.0));
    assert_eq!(a - b, v3(-3.0, 7.0, -3.0));
    assert_eq!(-a, v3(-1.0, -2.0, -3.0));
    assert_eq!(a * b, v3(4.0, -10.0, 18.0));
    assert_eq!(a * 2.0, v3(2.0, 4.0, 6.0));
    assert_eq!(2.0 * a, v3(2.0, 4.0, 6.0));
    assert_eq!(b / a, v3(4.0, -2.5, 2.0));
    assert_eq!(a / 2.0, v3(0.5, 1.0, 1.5));
    assert_eq!(6.0 / a, v3(6.0, 3.0, 2.0));
    assert_eq!(v3(7.0, -7.0, 7.5).floor_div(v3(2.0, 2.0, 2.5)), v3(3.0, -4.0, 3.0));
    assert_close(a.dot(b), 4.0 - 10.0 + 18.0);
    assert_eq!(Vector3::X_AXIS.cross(Vector3::Y_AXIS), Vector3::Z_AXIS);
    assert_eq!(Vector3::Y_AXIS.cross(Vector3::X_AXIS), -Vector3::Z_AXIS);
    assert_eq!(
        a.cross(b),
        v3(2.0 * 6.0 - 3.0 * -5.0, 3.0 * 4.0 - 1.0 * 6.0, 1.0 * -5.0 - 2.0 * 4.0)
    );
}

#[test]
fn vector3_componentwise_helpers() {
    let v = v3(-2.6, 5.1, 0.0);
    assert_eq!(v.sign(), v3(-1.0, 1.0, 0.0));
    assert_eq!(v.abs(), v3(2.6, 5.1, 0.0));
    assert_eq!(v.floor(), v3(-3.0, 5.0, 0.0));
    assert_eq!(v.ceil(), v3(-2.0, 6.0, 0.0));
    let (a, b) = (v3(1.0, 2.0, 1.0), v3(2.0, 1.0, 2.0));
    assert_eq!(a.max(b), v3(2.0, 2.0, 2.0));
    assert_eq!(a.min(b), v3(1.0, 1.0, 1.0));
    // Lerp's ends and middle; alpha isn't clamped.
    let (from, to) = (v3(0.0, 10.0, -4.0), v3(10.0, 20.0, 4.0));
    assert_eq!(from.lerp(to, 0.0), from);
    assert_eq!(from.lerp(to, 1.0), to);
    assert_eq!(from.lerp(to, 0.5), v3(5.0, 15.0, 0.0));
    assert_eq!(from.lerp(to, 2.0), v3(20.0, 30.0, 12.0));
}

#[test]
fn vector3_from_normal_id_and_axis() {
    assert_eq!(Vector3::from_normal_id(NormalId::Front), v3(0.0, 0.0, -1.0));
    assert_eq!(Vector3::from_normal_id(NormalId::Back), v3(0.0, 0.0, 1.0));
    assert_eq!(Vector3::from_normal_id(NormalId::Top), v3(0.0, 1.0, 0.0));
    assert_eq!(Vector3::from_normal_id(NormalId::Bottom), v3(0.0, -1.0, 0.0));
    assert_eq!(Vector3::from_normal_id(NormalId::Right), v3(1.0, 0.0, 0.0));
    assert_eq!(Vector3::from_normal_id(NormalId::Left), v3(-1.0, 0.0, 0.0));
    assert_eq!(Vector3::from_axis(Axis::Y), Vector3::Y_AXIS);
}

#[test]
fn vector_angles() {
    let (a, b) = (Vector3::X_AXIS, Vector3::Y_AXIS);
    assert_close(a.angle(b, None), FRAC_PI_2);
    assert_close(a.angle(b, Some(Vector3::Z_AXIS)), FRAC_PI_2);
    assert_close(b.angle(a, Some(Vector3::Z_AXIS)), -FRAC_PI_2);
    assert_close(a.angle(-a, None), PI);
    assert_close(v3(2.0, 0.0, 0.0).angle(v3(5.0, 5.0, 0.0), None), FRAC_PI_4);
    let (x, y) = (Vector2::X_AXIS, Vector2::Y_AXIS);
    assert_close(x.angle(y, false), FRAC_PI_2);
    assert_close(y.angle(x, false), FRAC_PI_2);
    assert_close(y.angle(x, true), -FRAC_PI_2);
    assert_close(x.cross(y), 1.0);
    assert_close(y.cross(x), -1.0);
}

#[test]
fn vector_fuzzy_eq() {
    let a = v3(1.0, 2.0, 3.0);
    let b = v3(1.000001, 2.0, 3.0);
    assert!(a.fuzzy_eq(b, 1e-5));
    assert!(!a.fuzzy_eq(b, 1e-8));
    // Relative for big components.
    assert!(v3(1e6, 0.0, 0.0).fuzzy_eq(v3(1e6 + 5.0, 0.0, 0.0), 1e-5));
    assert!(!v3(1e6, 0.0, 0.0).fuzzy_eq(v3(1e6 + 50.0, 0.0, 0.0), 1e-5));
    // Vector2's is absolute.
    assert!(Vector2::new(1.0, 1.0).fuzzy_eq(Vector2::new(1.000009, 0.999991), 1e-5));
    assert!(!Vector2::new(1e6, 0.0).fuzzy_eq(Vector2::new(1e6 + 1.0, 0.0), 1e-5));
}

#[test]
fn vector2_arithmetic() {
    let (a, b) = (Vector2::new(1.0, 2.0), Vector2::new(3.0, -4.0));
    assert_eq!(a + b, Vector2::new(4.0, -2.0));
    assert_eq!(a - b, Vector2::new(-2.0, 6.0));
    assert_eq!(a * b, Vector2::new(3.0, -8.0));
    assert_eq!(b / a, Vector2::new(3.0, -2.0));
    assert_eq!(a * 3.0, Vector2::new(3.0, 6.0));
    assert_eq!(3.0 * a, Vector2::new(3.0, 6.0));
    assert_eq!(a / 2.0, Vector2::new(0.5, 1.0));
    assert_eq!(-a, Vector2::new(-1.0, -2.0));
    assert_close(a.dot(b), -5.0);
    assert_eq!(a.lerp(b, 0.5), Vector2::new(2.0, -1.0));
    assert_eq!(a.max(b), Vector2::new(3.0, 2.0));
    assert_eq!(a.min(b), Vector2::new(1.0, -4.0));
    assert_eq!(Vector2::new(-1.5, 2.5).sign(), Vector2::new(-1.0, 1.0));
}

#[test]
fn vectors_compare_as_float32() {
    // 0.1 + 0.2 is 0.30000000000000004 in f64, the same float32 as 0.3.
    assert_eq!(v3(0.1 + 0.2, 0.0, 0.0), v3(0.3, 0.0, 0.0));
    assert_ne!(v3(0.3, 0.0, 0.0), v3(0.3001, 0.0, 0.0));
    assert_eq!(float32(0.1), 0.10000000149011612);
    assert!(v3(f64::NAN, 0.0, 0.0) != v3(f64::NAN, 0.0, 0.0));
}

#[test]
fn vectors_print_like_roblox() {
    assert_eq!(v3(1.0, 2.0, 3.0).to_string(), "1, 2, 3");
    assert_eq!(v3(0.1, 0.2, 0.3).to_string(), "0.100000001, 0.200000003, 0.300000012");
    assert_eq!(v3(3.0, 4.0, 0.0).unit().to_string(), "0.600000024, 0.800000012, 0");
    assert_eq!(v3(-0.0, 0.0, -1.0).to_string(), "-0, 0, -1");
    assert_eq!(
        v3(1e20, -2.5e-7, 123456789.0).to_string(),
        "1.00000002e+20, -2.49999999e-07, 123456792"
    );
    assert_eq!(Vector3::ZERO.unit().to_string(), "nan, nan, nan");
    assert_eq!(Vector2::new(0.5, 0.0).to_string(), "0.5, 0");
    // As the API dump prints UISizeConstraint.MaxSize.
    assert_eq!(Vector2::new(f64::INFINITY, f64::INFINITY).to_string(), "INF, INF");
    assert_eq!(Vector2::new(f64::NEG_INFINITY, 800.0).to_string(), "-INF, 800");
}

// ---------------------------------------------------------------- CFrame

#[test]
fn cframe_constructors() {
    assert_cf(CFrame::from_numbers(&[]).unwrap(), CFrame::IDENTITY);
    assert_eq!(CFrame::default(), CFrame::IDENTITY);
    let t = CFrame::from_numbers(&[1.0, 2.0, 3.0]).unwrap();
    assert_cf(t, CFrame::new(1.0, 2.0, 3.0));
    assert_cf(t, CFrame::from_position(v3(1.0, 2.0, 3.0)));
    assert_eq!(t.position(), v3(1.0, 2.0, 3.0));
    assert_eq!((t.x(), t.y(), t.z()), (1.0, 2.0, 3.0));
    assert_cf(t.rotation(), CFrame::IDENTITY);

    // Twelve numbers are the position and the matrix by rows, as GetComponents gives them back.
    let c = [1.0, 2.0, 3.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let m = CFrame::from_numbers(&c).unwrap();
    assert_eq!(m.get_components(), c);
    assert_eq!(m.components(), c);
    assert_eq!(m.rot[0], [0.0, -1.0, 0.0]);

    // Seven: a quaternion, normalized first. Half of y and w turns 90 degrees about Y.
    let q = CFrame::from_numbers(&[0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 2.0]).unwrap();
    assert_cf(q, CFrame::angles(0.0, FRAC_PI_2, 0.0));
    assert_cf(
        CFrame::from_quaternion(Vector3::ZERO, 0.0, 0.0, 0.0, 0.0),
        CFrame::IDENTITY,
    );

    // Six: a position and a point to look at.
    let six = CFrame::from_numbers(&[0.0, 20.0, 20.0, 0.0, 0.0, 0.0]).unwrap();
    assert_cf(six, CFrame::new_look(v3(0.0, 20.0, 20.0), Vector3::ZERO));
    assert!(CFrame::from_numbers(&[1.0, 2.0]).is_err());
    assert!(CFrame::from_numbers(&[0.0; 13]).is_err());
}

#[test]
fn cframe_single_axis_rotations_follow_the_right_hand_rule() {
    // A quarter turn about Y takes +X to -Z, about X takes +Y to +Z, about Z takes +X to +Y.
    assert_v3(CFrame::angles(0.0, FRAC_PI_2, 0.0) * Vector3::X_AXIS, 0.0, 0.0, -1.0);
    assert_v3(CFrame::angles(FRAC_PI_2, 0.0, 0.0) * Vector3::Y_AXIS, 0.0, 0.0, 1.0);
    assert_v3(CFrame::angles(0.0, 0.0, FRAC_PI_2) * Vector3::X_AXIS, 0.0, 1.0, 0.0);
    // Angles is Rx * Ry * Rz; fromOrientation is Ry * Rx * Rz.
    let (rx, ry, rz) = (0.3, -1.1, 2.0);
    let x = CFrame::angles(rx, 0.0, 0.0);
    let y = CFrame::angles(0.0, ry, 0.0);
    let z = CFrame::angles(0.0, 0.0, rz);
    assert_cf(CFrame::angles(rx, ry, rz), x * y * z);
    assert_cf(CFrame::from_euler_angles_xyz(rx, ry, rz), x * y * z);
    assert_cf(CFrame::from_orientation(rx, ry, rz), y * x * z);
    assert_cf(CFrame::from_euler_angles_yxz(rx, ry, rz), y * x * z);
    assert_cf(CFrame::from_euler_angles(rx, ry, rz, RotationOrder::ZYX), z * y * x);
    assert_cf(CFrame::from_euler_angles(rx, ry, rz, RotationOrder::XZY), x * z * y);
}

#[test]
fn cframe_composition_and_points() {
    let a = CFrame::new(1.0, 2.0, 3.0) * CFrame::angles(0.0, FRAC_PI_2, 0.0);
    assert_v3(a.position(), 1.0, 2.0, 3.0);
    assert_v3(a * Vector3::X_AXIS, 1.0, 2.0, 2.0);
    assert_v3(a.vector_to_world(Vector3::X_AXIS), 0.0, 0.0, -1.0);
    assert_v3(a.right_vector(), 0.0, 0.0, -1.0);
    assert_v3(a.look_vector(), -1.0, 0.0, 0.0);
    // Order matters: rotating first then moving moves along the rotated axes.
    let b = CFrame::angles(0.0, FRAC_PI_2, 0.0) * CFrame::new(1.0, 0.0, 0.0);
    assert_v3(b.position(), 0.0, 0.0, -1.0);
    // + and - move in world space and keep the rotation.
    let moved = a + v3(10.0, 0.0, 0.0);
    assert_v3(moved.position(), 11.0, 2.0, 3.0);
    assert_eq!(moved.rot, a.rot);
    assert_v3((a - v3(1.0, 2.0, 3.0)).position(), 0.0, 0.0, 0.0);
    // The docs' examples.
    let cf = CFrame::new(5.0, 10.0, 0.0);
    assert_eq!(
        cf.to_world_space(&[CFrame::new(0.0, 2.0, 0.0)])[0].to_string(),
        "5, 12, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1"
    );
    assert_eq!(cf.point_to_world_space(&[]), vec![v3(5.0, 10.0, 0.0)]);
    assert_eq!(cf.point_to_world_space(&[v3(0.0, 4.0, 0.0)]), vec![v3(5.0, 14.0, 0.0)]);
}

#[test]
fn cframe_inverse() {
    let a = CFrame::new(1.0, 2.0, 3.0) * CFrame::angles(0.0, FRAC_PI_2, 0.0);
    let inv = a.inverse();
    assert_v3(inv.position(), 3.0, -2.0, -1.0);
    assert_cf(a * inv, CFrame::IDENTITY);
    assert_cf(inv * a, CFrame::IDENTITY);
    let c = CFrame::new(-4.0, 9.5, 0.25) * CFrame::angles(0.7, -0.2, 1.9);
    assert_cf(c * c.inverse(), CFrame::IDENTITY);
    assert_cf(c.inverse().inverse(), c);
    let p = v3(3.0, -1.0, 8.0);
    assert_v3(c.point_to_object(c * p), p.x, p.y, p.z);
    assert_v3(
        c.inverse() * p,
        c.point_to_object(p).x,
        c.point_to_object(p).y,
        c.point_to_object(p).z,
    );
    let d = v3(0.0, 1.0, 1.0);
    let back = c.vector_to_object(c.vector_to_world(d));
    assert_v3(back, d.x, d.y, d.z);
}

#[test]
fn cframe_world_and_object_space_are_variadic() {
    let c = CFrame::new(1.0, 0.0, 0.0) * CFrame::angles(0.0, 0.0, FRAC_PI_2);
    let parts = [CFrame::new(0.0, 1.0, 0.0), CFrame::angles(0.4, 0.0, 0.0)];
    let world = c.to_world_space(&parts);
    assert_eq!(world.len(), 2);
    assert_cf(world[0], c * parts[0]);
    assert_cf(world[1], c * parts[1]);
    let object = c.to_object_space(&world);
    assert_cf(object[0], parts[0]);
    assert_cf(object[1], parts[1]);
    assert_cf(c.to_world_space(&[])[0], c);
    assert_cf(c.to_object_space(&[])[0], c.inverse());
    let pts = c.point_to_world_space(&[v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 5.0)]);
    assert_v3(pts[0], 1.0, 1.0, 0.0);
    assert_v3(pts[1], 1.0, 0.0, 5.0);
    let local = c.point_to_object_space(&pts);
    assert_v3(local[0], 1.0, 0.0, 0.0);
    assert_v3(local[1], 0.0, 0.0, 5.0);
    let dirs = c.vector_to_world_space(&[Vector3::X_AXIS, Vector3::Y_AXIS]);
    assert_v3(dirs[0], 0.0, 1.0, 0.0);
    assert_v3(dirs[1], -1.0, 0.0, 0.0);
    assert_v3(c.vector_to_object_space(&dirs)[1], 0.0, 1.0, 0.0);
    assert_eq!(c.vector_to_world_space(&[]), vec![Vector3::ZERO]);
}

#[test]
fn cframe_euler_round_trips() {
    // Angles in range (the middle axis within ±90 degrees) come back as they went in.
    let samples = [
        (0.1, 0.2, 0.3),
        (-1.2, 0.9, 2.8),
        (1.4, -1.4, -3.0),
        (0.0, 0.0, 0.0),
        (1.0, 1.5, -0.5),
    ];
    for &(rx, ry, rz) in &samples {
        let (x, y, z) = CFrame::angles(rx, ry, rz).to_euler_angles_xyz();
        assert!(
            close(x, rx) && close(y, ry) && close(z, rz),
            "XYZ {rx} {ry} {rz} gave {x} {y} {z}"
        );
        let (x, y, z) = CFrame::from_orientation(rx, ry, rz).to_orientation();
        assert!(
            close(x, rx) && close(y, ry) && close(z, rz),
            "YXZ {rx} {ry} {rz} gave {x} {y} {z}"
        );
        let (x, y, z) = CFrame::from_euler_angles_yxz(rx, ry, rz).to_euler_angles_yxz();
        assert!(close(x, rx) && close(y, ry) && close(z, rz));
    }
    // Every order rebuilds the same rotation from its angles, in range or not.
    let wide = [(2.5, -1.4, -3.0), (-3.0, 2.0, 2.2), (0.4, -2.9, 1.7)];
    for &order in RotationOrder::ALL {
        for &(rx, ry, rz) in samples.iter().chain(&wide) {
            let cf = CFrame::from_euler_angles(rx, ry, rz, order);
            let (x, y, z) = cf.to_euler_angles(order);
            assert_cf(CFrame::from_euler_angles(x, y, z, order), cf);
        }
    }
    // The docs' example: 30, 45 and 60 degrees come back.
    let (x, y, z) = CFrame::angles(30f64.to_radians(), 45f64.to_radians(), 60f64.to_radians()).to_euler_angles_xyz();
    assert_eq!(
        format!("{:.2} {:.2} {:.2}", x.to_degrees(), y.to_degrees(), z.to_degrees()),
        "30.00 45.00 60.00"
    );
}

#[test]
fn cframe_euler_at_gimbal_lock() {
    // With the middle angle at ±90 degrees only the sum (or difference) of the others counts;
    // the angles that come back still rebuild the rotation.
    for &order in RotationOrder::ALL {
        for &middle in &[FRAC_PI_2, -FRAC_PI_2] {
            let mut angles = [0.4, 0.4, 0.4];
            let axes = match order {
                RotationOrder::XYZ | RotationOrder::ZYX => 1,
                RotationOrder::XZY | RotationOrder::YZX => 2,
                RotationOrder::YXZ | RotationOrder::ZXY => 0,
            };
            angles[axes] = middle;
            let cf = CFrame::from_euler_angles(angles[0], angles[1], angles[2], order);
            // Snap the near-zero entries to exact zeros, as an exact matrix has them.
            let snapped = CFrame::from_components(cf.get_components().map(|c| if c.abs() < 1e-12 { 0.0 } else { c }));
            let (x, y, z) = snapped.to_euler_angles(order);
            assert_cf(CFrame::from_euler_angles(x, y, z, order), snapped);
        }
    }
}

#[test]
fn cframe_look_at() {
    // Looking down -Z is no rotation.
    assert_cf(
        CFrame::look_at(Vector3::ZERO, v3(0.0, 0.0, -10.0), Vector3::Y_AXIS),
        CFrame::IDENTITY,
    );
    let cf = CFrame::look_at(v3(1.0, 1.0, 1.0), v3(11.0, 1.0, 1.0), Vector3::Y_AXIS);
    assert_v3(cf.position(), 1.0, 1.0, 1.0);
    assert_v3(cf.look_vector(), 1.0, 0.0, 0.0);
    assert_v3(cf.right_vector(), 0.0, 0.0, 1.0);
    assert_v3(cf.up_vector(), 0.0, 1.0, 0.0);
    // Workspace.CurrentCamera's default: at (0, 20, 20) looking at the origin, which is Rx(-45°).
    let camera = CFrame::new_look(v3(0.0, 20.0, 20.0), Vector3::ZERO);
    let h = FRAC_PI_4.cos();
    assert_cf(
        camera,
        CFrame::from_components([0.0, 20.0, 20.0, 1.0, 0.0, 0.0, 0.0, h, h, 0.0, -h, h]),
    );
    assert_cf(
        camera,
        CFrame::new(0.0, 20.0, 20.0) * CFrame::angles(-FRAC_PI_4, 0.0, 0.0),
    );
    // Straight up (along `up`): the X axis stands in, so UpVector becomes X.
    let up = CFrame::look_at(Vector3::ZERO, v3(0.0, 10.0, 0.0), Vector3::Y_AXIS);
    assert_v3(up.look_vector(), 0.0, 1.0, 0.0);
    assert_v3(up.up_vector(), 1.0, 0.0, 0.0);
    let down = CFrame::look_at(Vector3::ZERO, v3(0.0, -3.0, 0.0), Vector3::Y_AXIS);
    assert_v3(down.look_vector(), 0.0, -1.0, 0.0);
    assert_v3(down.up_vector(), 1.0, 0.0, 0.0);
    // Another up vector rolls it.
    let rolled = CFrame::look_at(Vector3::ZERO, v3(0.0, 0.0, -1.0), Vector3::X_AXIS);
    assert_v3(rolled.up_vector(), 1.0, 0.0, 0.0);
    assert_v3(rolled.look_vector(), 0.0, 0.0, -1.0);
    // lookAlong is lookAt toward at + direction; looking at itself is no rotation.
    assert_cf(
        CFrame::look_along(v3(2.0, 0.0, 0.0), v3(0.0, 0.0, 5.0), Vector3::Y_AXIS),
        CFrame::look_at(v3(2.0, 0.0, 0.0), v3(2.0, 0.0, 5.0), Vector3::Y_AXIS),
    );
    assert_cf(
        CFrame::look_at(v3(1.0, 2.0, 3.0), v3(1.0, 2.0, 3.0), Vector3::Y_AXIS),
        CFrame::new(1.0, 2.0, 3.0),
    );
}

#[test]
fn cframe_vectors_and_identity_look() {
    let i = CFrame::IDENTITY;
    // The docs print (-0, -0, -1): LookVector is the negated third column.
    assert_eq!(i.look_vector().to_string(), "-0, -0, -1");
    assert_eq!(i.right_vector(), Vector3::X_AXIS);
    assert_eq!(i.x_vector(), Vector3::X_AXIS);
    assert_eq!(i.up_vector(), Vector3::Y_AXIS);
    assert_eq!(i.y_vector(), Vector3::Y_AXIS);
    assert_eq!(i.z_vector(), Vector3::Z_AXIS);
    let cf = CFrame::from_components([0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    assert_eq!(cf.x_vector(), v3(1.0, 4.0, 7.0));
    assert_eq!(cf.y_vector(), v3(2.0, 5.0, 8.0));
    assert_eq!(cf.z_vector(), v3(3.0, 6.0, 9.0));
    assert_eq!(cf.look_vector(), v3(-3.0, -6.0, -9.0));
}

#[test]
fn cframe_from_matrix_and_axis_angle() {
    let cf = CFrame::from_matrix(v3(1.0, 2.0, 3.0), v3(0.0, 0.0, -1.0), Vector3::Y_AXIS, None);
    assert_v3(cf.z_vector(), 1.0, 0.0, 0.0);
    assert_cf(cf, CFrame::new(1.0, 2.0, 3.0) * CFrame::angles(0.0, FRAC_PI_2, 0.0));
    let explicit = CFrame::from_matrix(Vector3::ZERO, Vector3::X_AXIS, Vector3::Y_AXIS, Some(-Vector3::Z_AXIS));
    assert_eq!(explicit.z_vector(), -Vector3::Z_AXIS);

    assert_cf(
        CFrame::from_axis_angle(Vector3::Y_AXIS, FRAC_PI_2),
        CFrame::angles(0.0, FRAC_PI_2, 0.0),
    );
    assert_cf(
        CFrame::from_axis_angle(v3(0.0, 0.0, 7.0), 1.0),
        CFrame::angles(0.0, 0.0, 1.0),
    );
    assert_cf(
        CFrame::from_axis_angle(-Vector3::X_AXIS, 0.5),
        CFrame::angles(-0.5, 0.0, 0.0),
    );
    assert_cf(CFrame::from_axis_angle(Vector3::ZERO, 1.0), CFrame::IDENTITY);
    // A third of a turn about (1, 1, 1) cycles the axes.
    let cycle = CFrame::from_axis_angle(Vector3::ONE, 2.0 * PI / 3.0);
    assert_v3(cycle * Vector3::X_AXIS, 0.0, 1.0, 0.0);
    assert_v3(cycle * Vector3::Y_AXIS, 0.0, 0.0, 1.0);
}

#[test]
fn cframe_to_axis_angle_round_trips() {
    let axis = v3(1.0, 1.0, 0.0).unit();
    let (a, angle) = CFrame::from_axis_angle(v3(1.0, 1.0, 0.0), 1.2).to_axis_angle();
    assert_v3(a, axis.x, axis.y, axis.z);
    assert_close(angle, 1.2);
    // A negative angle comes back as the positive one about the opposite axis.
    let (a, angle) = CFrame::from_axis_angle(Vector3::Z_AXIS, -0.75).to_axis_angle();
    assert_v3(a, 0.0, 0.0, -1.0);
    assert_close(angle, 0.75);
    let (a, angle) = CFrame::angles(PI, 0.0, 0.0).to_axis_angle();
    assert_close(angle, PI);
    assert_close(a.x.abs(), 1.0);
    let (a, angle) = CFrame::IDENTITY.to_axis_angle();
    assert_eq!((a, angle), (Vector3::X_AXIS, 0.0));
    for &(rx, ry, rz) in &[(0.3, 0.2, 0.1), (-2.0, 1.0, 0.5), (3.0, -0.1, -2.9)] {
        let cf = CFrame::angles(rx, ry, rz);
        let (axis, angle) = cf.to_axis_angle();
        assert_cf(CFrame::from_axis_angle(axis, angle), cf);
        assert!((0.0..=PI).contains(&angle));
    }
}

#[test]
fn cframe_lerp() {
    let a = CFrame::new(0.0, 0.0, 0.0);
    let b = CFrame::new(10.0, 0.0, -4.0) * CFrame::angles(0.0, FRAC_PI_2, 0.0);
    // The ends exactly.
    assert_eq!(a.lerp(b, 0.0).get_components(), a.get_components());
    assert_eq!(a.lerp(b, 1.0).get_components(), b.get_components());
    // The middle: half the move, half the turn.
    let mid = a.lerp(b, 0.5);
    assert_v3(mid.position(), 5.0, 0.0, -2.0);
    assert_cf(mid.rotation(), CFrame::angles(0.0, FRAC_PI_4, 0.0));
    let quarter = a.lerp(b, 0.25);
    assert_cf(quarter.rotation(), CFrame::angles(0.0, FRAC_PI_2 / 4.0, 0.0));
    // Constant speed: equal steps turn equal angles.
    let r = CFrame::IDENTITY;
    let goal = CFrame::from_axis_angle(v3(1.0, 2.0, 3.0), 2.0);
    for i in 1..10 {
        let t = f64::from(i) / 10.0;
        assert_close(r.angle_between(r.lerp(goal, t)), 2.0 * t);
    }
    // The shortest arc: 170 to -170 degrees about Y passes 180, not 0.
    let from = CFrame::angles(0.0, 170f64.to_radians(), 0.0);
    let to = CFrame::angles(0.0, (-170f64).to_radians(), 0.0);
    let halfway = from.lerp(to, 0.5);
    assert_close(halfway.angle_between(CFrame::angles(0.0, PI, 0.0)), 0.0);
    // Nearly the same rotation still interpolates cleanly.
    let near = CFrame::angles(0.0, 1e-7, 0.0);
    assert_cf(CFrame::IDENTITY.lerp(near, 0.5), CFrame::angles(0.0, 0.5e-7, 0.0));
}

#[test]
fn cframe_orthonormalize() {
    // A matrix drifted off orthonormal comes back to the nearest frame, X kept.
    let skew = CFrame::from_components([1.0, 2.0, 3.0, 2.0, 0.1, 0.0, 0.0, 0.9, 0.2, 0.0, 0.0, 1.1]);
    let o = skew.orthonormalize();
    assert_v3(o.position(), 1.0, 2.0, 3.0);
    assert_v3(o.x_vector(), 1.0, 0.0, 0.0);
    let (x, y, z) = (o.x_vector(), o.y_vector(), o.z_vector());
    for (a, b) in [(x, y), (y, z), (x, z)] {
        assert_close(a.dot(b), 0.0);
    }
    for v in [x, y, z] {
        assert_close(v.magnitude(), 1.0);
    }
    assert_v3(x.cross(y), z.x, z.y, z.z);
    let r = CFrame::angles(0.4, 0.5, 0.6);
    assert_cf(r.orthonormalize(), r);
}

#[test]
fn cframe_fuzzy_eq_and_angle_between() {
    let a = CFrame::new(1.0, 2.0, 3.0) * CFrame::angles(0.1, 0.2, 0.3);
    assert!(a.fuzzy_eq(a + v3(1e-7, 0.0, 0.0), 1e-5));
    assert!(!a.fuzzy_eq(a + v3(1e-3, 0.0, 0.0), 1e-5));
    assert!(a.fuzzy_eq(a * CFrame::angles(1e-7, 0.0, 0.0), 1e-5));
    assert!(!a.fuzzy_eq(a * CFrame::angles(1e-3, 0.0, 0.0), 1e-5));
    assert_close(CFrame::IDENTITY.angle_between(CFrame::angles(0.0, 0.0, 1.0)), 1.0);
    assert_close(
        CFrame::angles(0.0, 0.5, 0.0).angle_between(CFrame::angles(0.0, -0.5, 0.0)),
        1.0,
    );
    assert_close(a.angle_between(a + v3(9.0, 9.0, 9.0)), 0.0);
}

#[test]
fn cframe_rotation_between_vectors() {
    let cf = CFrame::from_rotation_between_vectors(Vector3::Y_AXIS, Vector3::X_AXIS);
    assert_v3(cf * Vector3::Y_AXIS, 1.0, 0.0, 0.0);
    assert_eq!(cf.position(), Vector3::ZERO);
    let (from, to) = (v3(1.0, 2.0, 3.0), v3(-4.0, 0.5, 2.0));
    let turned = CFrame::from_rotation_between_vectors(from, to) * from.unit();
    let want = to.unit();
    assert_v3(turned, want.x, want.y, want.z);
    // Opposite directions: half a turn.
    let flip = CFrame::from_rotation_between_vectors(Vector3::X_AXIS, -Vector3::X_AXIS);
    assert_v3(flip * Vector3::X_AXIS, -1.0, 0.0, 0.0);
    assert_close(CFrame::IDENTITY.angle_between(flip), PI);
    assert_cf(
        CFrame::from_rotation_between_vectors(Vector3::Z_AXIS, Vector3::Z_AXIS * 3.0),
        CFrame::IDENTITY,
    );
}

#[test]
fn cframe_quaternion_round_trip() {
    for &(rx, ry, rz) in &[
        (0.0, 0.0, 0.0),
        (0.3, -0.2, 1.0),
        (3.1, 0.1, -3.1),
        (PI, 0.0, 0.0),
        (0.0, PI, 0.0),
    ] {
        let cf = CFrame::new(1.0, -2.0, 0.5) * CFrame::angles(rx, ry, rz);
        let [x, y, z, w] = cf.quaternion();
        assert_close(x * x + y * y + z * z + w * w, 1.0);
        assert_cf(CFrame::from_quaternion(cf.position(), x, y, z, w), cf);
    }
}

#[test]
fn cframes_compare_and_print() {
    assert_eq!(
        CFrame::new(5.0, 12.0, 0.0).to_string(),
        "5, 12, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1"
    );
    assert_eq!(
        CFrame::new(0.0, 0.0, -5.0).to_string(),
        "0, 0, -5, 1, 0, 0, 0, 1, 0, 0, 0, 1"
    );
    let half = FRAC_PI_4.cos();
    let cf = CFrame::from_components([0.0, 0.0, 0.0, half, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(cf.to_string(), "0, 0, 0, 0.707106769, 0, 0, 0, 1, 0, 0, 0, 1");
    // Equal as float32.
    assert_eq!(CFrame::new(0.1 + 0.2, 0.0, 0.0), CFrame::new(0.3, 0.0, 0.0));
    assert_ne!(CFrame::new(0.0, 0.0, 0.0), CFrame::angles(0.0, 0.001, 0.0));
}

// ---------------------------------------------------------------- Color3

#[test]
fn color3_constructors() {
    assert_eq!(Color3::from_rgb(255.0, 0.0, 0.0), Color3::new(1.0, 0.0, 0.0));
    assert_close(Color3::from_rgb(51.0, 102.0, 204.0).g, 0.4);
    assert_eq!(Color3::default(), Color3::new(0.0, 0.0, 0.0));
    // Not clamped.
    assert_close(Color3::from_rgb(510.0, -255.0, 0.0).r, 2.0);
    assert_close(Color3::from_rgb(510.0, -255.0, 0.0).g, -1.0);
}

#[test]
fn color3_hsv() {
    assert_eq!(Color3::from_hsv(0.0, 1.0, 1.0), Color3::new(1.0, 0.0, 0.0));
    // The hue wraps: 1 is red again.
    assert_eq!(Color3::from_hsv(1.0, 1.0, 1.0), Color3::new(1.0, 0.0, 0.0));
    assert_eq!(Color3::from_hsv(1.0 / 3.0, 1.0, 1.0), Color3::new(0.0, 1.0, 0.0));
    assert_eq!(Color3::from_hsv(2.0 / 3.0, 1.0, 1.0), Color3::new(0.0, 0.0, 1.0));
    assert_eq!(Color3::from_hsv(0.0, 0.0, 1.0), Color3::new(1.0, 1.0, 1.0));
    assert_eq!(Color3::from_hsv(0.5, 0.5, 0.5), Color3::new(0.25, 0.5, 0.5));
    assert_eq!(Color3::from_hsv(1.25, 1.0, 1.0), Color3::from_hsv(0.25, 1.0, 1.0));

    let (h, s, v) = Color3::from_rgb(255.0, 0.0, 0.0).to_hsv();
    assert_eq!((h, s, v), (0.0, 1.0, 1.0));
    let (h, s, v) = Color3::from_rgb(0.0, 255.0, 0.0).to_hsv();
    assert_close(h, 1.0 / 3.0);
    assert_eq!((s, v), (1.0, 1.0));
    let (h, _, _) = Color3::new(1.0, 0.0, 1.0).to_hsv();
    assert_close(h, 5.0 / 6.0);
    // Greys have no hue or saturation.
    assert_eq!(Color3::new(0.5, 0.5, 0.5).to_hsv(), (0.0, 0.0, 0.5));
    assert_eq!(Color3::new(0.0, 0.0, 0.0).to_hsv(), (0.0, 0.0, 0.0));
    // Round trips, the hue staying under 1.
    for i in 0..=40 {
        let c = Color3::from_rgb(f64::from(i * 6), f64::from(255 - i * 5), f64::from((i * 37) % 256));
        let (h, s, v) = c.to_hsv();
        assert!((0.0..1.0).contains(&h), "{h}");
        let back = Color3::from_hsv(h, s, v);
        assert!(
            close(back.r, c.r) && close(back.g, c.g) && close(back.b, c.b),
            "{c} came back {back}"
        );
    }
}

#[test]
fn color3_hex() {
    assert_eq!(Color3::from_hex("FF0000").unwrap(), Color3::new(1.0, 0.0, 0.0));
    assert_eq!(Color3::from_hex("ec008c").unwrap(), Color3::from_rgb(236.0, 0.0, 140.0));
    assert_eq!(Color3::from_hex("000").unwrap(), Color3::new(0.0, 0.0, 0.0));
    assert_eq!(Color3::from_hex("#FFF").unwrap(), Color3::new(1.0, 1.0, 1.0));
    assert_eq!(Color3::from_hex("#FA0").unwrap(), Color3::from_hex("#FFAA00").unwrap());
    assert_eq!(
        Color3::from_hex("#aBcDeF").unwrap(),
        Color3::from_rgb(171.0, 205.0, 239.0)
    );
    for bad in ["", "#", "12345", "1234567", "GGGGGG", "#12", "ff 000", "+fff", "ffé"] {
        assert!(Color3::from_hex(bad).is_err(), "{bad:?}");
    }
    assert_eq!(Color3::from_rgb(255.0, 0.0, 0.0).to_hex(), "ff0000");
    assert_eq!(Color3::from_rgb(236.0, 0.0, 140.0).to_hex(), "ec008c");
    // Rounded, and clamped to a byte.
    assert_eq!(Color3::new(0.5, 0.501, 2.0).to_hex(), "8080ff");
    assert_eq!(Color3::new(-1.0, 0.0, 0.0).to_hex(), "000000");
    for rgb in [(0, 0, 0), (255, 255, 255), (27, 42, 53), (163, 162, 165), (1, 254, 128)] {
        let c = Color3::from_rgb(rgb.0.into(), rgb.1.into(), rgb.2.into());
        assert_eq!(Color3::from_hex(&c.to_hex()).unwrap(), c);
    }
}

#[test]
fn color3_lerp_and_print() {
    let (white, black) = (Color3::new(1.0, 1.0, 1.0), Color3::new(0.0, 0.0, 0.0));
    // The docs' examples.
    assert_eq!(white.lerp(black, 0.1).to_string(), "0.9, 0.9, 0.9");
    assert_eq!(white.lerp(black, 0.5).to_string(), "0.5, 0.5, 0.5");
    assert_eq!(white.lerp(black, 0.85).to_string(), "0.15, 0.15, 0.15");
    assert_eq!(white.lerp(black, 0.0), white);
    assert_eq!(white.lerp(black, 1.0), black);
    // As the API dump prints BorderColor3's default.
    assert_eq!(
        Color3::from_rgb(27.0, 42.0, 53.0).to_string(),
        "0.105882, 0.164706, 0.207843"
    );
    assert_eq!(Color3::new(1.0, 0.0, 0.0).to_string(), "1, 0, 0");
}

// ---------------------------------------------------------------- UDim, UDim2, Rect

#[test]
fn udim_offsets_are_int32() {
    let u = UDim::new(0.5, 10.7);
    assert_eq!((u.scale, u.offset), (0.5, 10));
    assert_eq!(UDim::new(0.0, -10.7).offset, -10);
    assert_eq!(UDim::new(0.0, 1e12).offset, i32::MAX);
    assert_close(UDim::new(0.5, 10.0).resolve(200.0), 110.0);
    assert_eq!(UDim::new(0.25, 3.0) + UDim::new(0.5, 4.0), UDim::new(0.75, 7.0));
    assert_eq!(UDim::new(0.25, 3.0) - UDim::new(0.5, 4.0), UDim::new(-0.25, -1.0));
    assert_eq!(UDim::new(0.5, 8.0).to_string(), "0.5, 8");
}

#[test]
fn udim2_constructors_and_math() {
    let a = UDim2::new(0.5, 20.0, 1.0, 0.0);
    assert_eq!(a.x, UDim::new(0.5, 20.0));
    assert_eq!(a.width(), a.x);
    assert_eq!(a.height(), a.y);
    assert_eq!(UDim2::from_scale(0.5, 0.25), UDim2::new(0.5, 0.0, 0.25, 0.0));
    assert_eq!(UDim2::from_offset(100.0, 50.0), UDim2::new(0.0, 100.0, 0.0, 50.0));
    assert_eq!(
        UDim2::from_udims(UDim::new(0.5, 10.0), UDim::new(1.0, 0.0)),
        UDim2::new(0.5, 10.0, 1.0, 0.0)
    );
    assert_eq!(a + UDim2::new(0.1, 5.0, -1.0, 7.0), UDim2::new(0.6, 25.0, 0.0, 7.0));
    assert_eq!(a - UDim2::new(0.5, 20.0, 1.0, 0.0), UDim2::default());
    assert_eq!(a.resolve(Vector2::new(300.0, 80.0)), Vector2::new(170.0, 80.0));
    assert_ne!(UDim2::new(0.0, 1.0, 0.0, 1.0), UDim2::new(0.0, 1.0, 0.0, 2.0));
}

#[test]
fn udim2_lerp() {
    let (a, b) = (UDim2::new(0.0, 0.0, 1.0, 10.0), UDim2::new(1.0, 5.0, 0.0, -10.0));
    assert_eq!(a.lerp(b, 0.0), a);
    assert_eq!(a.lerp(b, 1.0), b);
    let mid = a.lerp(b, 0.5);
    assert_close(mid.x.scale, 0.5);
    assert_close(mid.y.scale, 0.5);
    // Offsets are whole pixels: 2.5 rounds to 3, 0 stays 0.
    assert_eq!((mid.x.offset, mid.y.offset), (3, 0));
    assert_eq!(a.lerp(b, 0.3).x.offset, 2);
}

#[test]
fn udim2_and_rect_print() {
    assert_eq!(UDim2::new(0.0, 300.0, 1.0, 0.0).to_string(), "{0, 300}, {1, 0}");
    assert_eq!(UDim2::new(0.0, 0.0, 2.0, 0.0).to_string(), "{0, 0}, {2, 0}");
    assert_eq!(UDim2::new(0.5, -4.0, 0.25, 8.0).to_string(), "{0.5, -4}, {0.25, 8}");
    let r = Rect::from_coords(1.0, 2.0, 11.0, 7.0);
    assert_eq!(r, Rect::new(Vector2::new(1.0, 2.0), Vector2::new(11.0, 7.0)));
    assert_eq!((r.width(), r.height()), (10.0, 5.0));
    assert_eq!(r.to_string(), "1, 2, 11, 7");
    assert_eq!(Rect::default().to_string(), "0, 0, 0, 0");
}

// ---------------------------------------------------------------- ranges and sequences

#[test]
fn number_range() {
    let r = NumberRange::new(0.0, 60000.0).unwrap();
    assert_eq!((r.min, r.max), (0.0, 60000.0));
    assert_eq!(r.to_string(), "0 60000 ");
    assert_eq!(NumberRange::new(0.9, 1.05).unwrap().to_string(), "0.9 1.05 ");
    assert_eq!(NumberRange::new(0.0, f64::INFINITY).unwrap().to_string(), "0 inf ");
    assert_eq!(NumberRange::single(5.5), NumberRange::new(5.5, 5.5).unwrap());
    assert!(NumberRange::new(2.0, 1.0).is_err());
}

fn nk(time: f64, value: f64) -> NumberSequenceKeypoint {
    NumberSequenceKeypoint::new(time, value, 0.0)
}

#[test]
fn number_sequence_checks_its_keypoints() {
    assert!(NumberSequence::new(vec![nk(0.0, 1.0), nk(1.0, 2.0)]).is_ok());
    let err = |v: Vec<NumberSequenceKeypoint>| NumberSequence::new(v).unwrap_err();
    assert!(err(vec![]).contains("at least 2"));
    assert!(err(vec![nk(0.0, 1.0)]).contains("at least 2"));
    assert!(err(vec![nk(0.1, 1.0), nk(1.0, 1.0)]).contains("time 0"));
    assert!(err(vec![nk(0.0, 1.0), nk(0.9, 1.0)]).contains("time 1"));
    assert!(err(vec![nk(0.0, 1.0), nk(0.6, 1.0), nk(0.4, 1.0), nk(1.0, 1.0)]).contains("ordered"));
    // 20 keypoints at most.
    let many = |n: usize| (0..n).map(|i| nk(i as f64 / (n - 1) as f64, 0.0)).collect::<Vec<_>>();
    assert!(NumberSequence::new(many(MAX_KEYPOINTS)).is_ok());
    assert!(err(many(MAX_KEYPOINTS + 1)).contains("at most 20"));
    // Equal times are allowed, and times are float32: 0.99999999 is 1.
    assert!(NumberSequence::new(vec![nk(0.0, 0.0), nk(0.5, 0.0), nk(0.5, 1.0), nk(1.0, 1.0)]).is_ok());
    assert!(NumberSequence::new(vec![nk(0.0, 0.0), nk(0.99999999, 1.0)]).is_ok());
    assert!(NumberSequence::new(vec![nk(1e-12, 0.0), nk(1.0, 1.0)]).is_err());
}

#[test]
fn number_sequence_evaluates() {
    // The docs' example: 0.65 is 0.7.
    let seq = NumberSequence::new(vec![nk(0.0, 0.0), nk(0.5, 1.0), nk(1.0, 0.0)]).unwrap();
    assert_close(seq.value_at(0.65), 0.7);
    assert_close(seq.value_at(0.0), 0.0);
    assert_close(seq.value_at(0.25), 0.5);
    assert_close(seq.value_at(0.5), 1.0);
    assert_close(seq.value_at(1.0), 0.0);
    // Out of range is clamped.
    assert_close(seq.value_at(-3.0), 0.0);
    assert_close(seq.value_at(9.0), 0.0);
    assert_eq!(seq.keypoints().len(), 3);
    // A step where two keypoints share a time: the later one from that time on.
    let step = NumberSequence::new(vec![nk(0.0, 0.0), nk(0.5, 0.0), nk(0.5, 1.0), nk(1.0, 1.0)]).unwrap();
    assert_close(step.value_at(0.49), 0.0);
    assert_close(step.value_at(0.5), 1.0);
    assert_close(NumberSequence::constant(4.0).value_at(0.3), 4.0);
    assert_close(NumberSequence::between(2.0, 4.0).value_at(0.75), 3.5);
    // Envelopes interpolate too.
    let env = NumberSequence::new(vec![
        NumberSequenceKeypoint::new(0.0, 1.0, 0.0),
        NumberSequenceKeypoint::new(1.0, 1.0, 0.5),
    ])
    .unwrap();
    assert_close(env.envelope_at(0.5), 0.25);
}

#[test]
fn color_sequence() {
    let red = Color3::from_rgb(255.0, 0.0, 0.0);
    let cyan = Color3::from_rgb(0.0, 190.0, 200.0);
    let purple = Color3::from_rgb(190.0, 0.0, 255.0);
    let seq = ColorSequence::new(vec![
        ColorSequenceKeypoint::new(0.0, red),
        ColorSequenceKeypoint::new(0.5, cyan),
        ColorSequenceKeypoint::new(1.0, purple),
    ])
    .unwrap();
    // The docs' example prints 0.372549, 0.372549, 0.892157.
    assert_eq!(seq.value_at(0.75).to_string(), "0.372549, 0.372549, 0.892157");
    assert_eq!(seq.value_at(0.0), red);
    assert_eq!(seq.value_at(0.5), cyan);
    assert_eq!(seq.value_at(1.0), purple);
    assert_eq!(seq.value_at(0.25), red.lerp(cyan, 0.5));
    let bad = ColorSequence::new(vec![ColorSequenceKeypoint::new(0.0, red)]);
    assert!(bad.unwrap_err().starts_with("ColorSequence"));
    assert!(
        ColorSequence::new(vec![
            ColorSequenceKeypoint::new(0.0, red),
            ColorSequenceKeypoint::new(0.5, red)
        ])
        .is_err()
    );
    assert_eq!(ColorSequence::constant(cyan).value_at(0.3), cyan);
    assert_eq!(ColorSequence::between(red, purple).value_at(0.5), red.lerp(purple, 0.5));
}

#[test]
fn sequences_print_like_the_api_dump() {
    let white = Color3::new(1.0, 1.0, 1.0);
    assert_eq!(ColorSequence::constant(white).to_string(), "0 1 1 1 0 1 1 1 1 0 ");
    assert_eq!(NumberSequence::constant(0.5).to_string(), "0 0.5 0 1 0.5 0 ");
    assert_eq!(NumberSequence::constant(32.0).to_string(), "0 32 0 1 32 0 ");
    let env = NumberSequence::new(vec![NumberSequenceKeypoint::new(0.0, 1.0, 0.25), nk(1.0, 0.0)]).unwrap();
    assert_eq!(env.to_string(), "0 1 0.25 1 0 0 ");
}

// ---------------------------------------------------------------- easing and TweenInfo

#[test]
fn easing_ends_exactly() {
    for &style in EasingStyle::ALL {
        for &dir in EasingDirection::ALL {
            assert_eq!(ease(style, dir, 0.0), 0.0, "{style:?} {dir:?} at 0");
            assert_eq!(ease(style, dir, 1.0), 1.0, "{style:?} {dir:?} at 1");
            // Alpha is clamped, as GetValue clamps it.
            assert_eq!(ease(style, dir, -0.5), 0.0);
            assert_eq!(ease(style, dir, 7.0), 1.0);
            assert_eq!(ease(style, dir, f64::NAN), 0.0);
            // InOut is halfway at the middle.
            assert_close(ease(style, EasingDirection::InOut, 0.5), 0.5);
        }
    }
}

#[test]
fn easing_is_symmetric() {
    for &style in EasingStyle::ALL {
        for i in 0..=20 {
            let t = f64::from(i) / 20.0;
            let (i, o, io) = (EasingDirection::In, EasingDirection::Out, EasingDirection::InOut);
            assert_close(ease(style, o, t), 1.0 - ease(style, i, 1.0 - t));
            assert_close(ease(style, io, t) + ease(style, io, 1.0 - t), 1.0);
            if t < 0.5 {
                assert_close(ease(style, io, t), ease(style, i, 2.0 * t) / 2.0);
            }
        }
    }
}

#[test]
fn easing_known_values() {
    use EasingDirection::{In, InOut, Out};
    use EasingStyle::*;
    for i in 0..=10 {
        let t = f64::from(i) / 10.0;
        assert_close(ease(Linear, In, t), t);
        assert_close(ease(Linear, Out, t), t);
        assert_close(ease(Linear, InOut, t), t);
    }
    assert_close(ease(Quad, In, 0.5), 0.25);
    assert_close(ease(Quad, Out, 0.5), 0.75);
    assert_close(ease(Quad, InOut, 0.25), 0.125);
    assert_close(ease(Quad, InOut, 0.75), 0.875);
    assert_close(ease(Cubic, In, 0.5), 0.125);
    assert_close(ease(Quart, In, 0.5), 0.0625);
    assert_close(ease(Quint, In, 0.5), 0.03125);
    assert_close(ease(Quint, Out, 0.5), 0.96875);
    assert_close(ease(Sine, In, 0.5), 1.0 - FRAC_PI_4.cos());
    assert_close(ease(Sine, Out, 0.5), FRAC_PI_4.sin());
    assert_close(ease(Circular, In, 0.5), 1.0 - 0.75f64.sqrt());
    assert_close(ease(Exponential, In, 0.5), 0.03125);
    assert_close(ease(Exponential, Out, 0.5), 0.96875);
    assert_close(ease(Back, In, 0.5), -0.0876975);
    assert_close(ease(Back, Out, 0.5), 1.0876975);
    assert_close(ease(Bounce, Out, 0.5), 0.765625);
    assert_close(ease(Bounce, In, 0.5), 0.234375);
    assert_close(ease(Bounce, Out, 0.2), 0.3025);
    assert_close(ease(Bounce, Out, 0.9), 0.988125);
    assert_close(ease(Elastic, In, 0.5), -0.015625);
    assert_close(ease(Elastic, Out, 0.5), 1.015625);
    assert!((ease(Elastic, Out, 0.25) - 0.9116116523516815).abs() < 1e-12);
}

#[test]
fn easing_shapes() {
    use EasingDirection::{In, Out};
    use EasingStyle::*;
    // The plain curves only climb.
    for style in [Linear, Sine, Quad, Cubic, Quart, Quint, Exponential, Circular] {
        let mut last = 0.0;
        for i in 1..=100 {
            let v = ease(style, In, f64::from(i) / 100.0);
            assert!(v >= last, "{style:?} fell at {i}");
            last = v;
        }
    }
    // Back pulls back first and overshoots after; Elastic overshoots too; Bounce stays inside.
    assert!(ease(Back, In, 0.2) < 0.0);
    assert!(ease(Back, Out, 0.8) > 1.0);
    assert!((1..100).any(|i| ease(Elastic, Out, f64::from(i) / 100.0) > 1.0));
    assert!((1..100).all(|i| (0.0..=1.0).contains(&ease(Bounce, Out, f64::from(i) / 100.0))));
    // Faster styles are further along at the same point of an Out curve.
    let at = |s| ease(s, Out, 0.3);
    assert!(at(Linear) < at(Sine) && at(Sine) < at(Quad) && at(Quad) < at(Cubic));
    assert!(at(Cubic) < at(Quart) && at(Quart) < at(Quint) && at(Quint) < at(Exponential));
}

#[test]
fn easing_enums() {
    assert_eq!(EasingStyle::ALL.len(), 11);
    assert_eq!(EasingStyle::from_name("Exponential"), Some(EasingStyle::Exponential));
    assert_eq!(EasingStyle::Cubic.value(), 10);
    assert_eq!(EasingStyle::from_value(2), Some(EasingStyle::Back));
    // Bezier and Spring aren't styles TweenService runs.
    assert_eq!(EasingStyle::from_name("Bezier"), None);
    assert_eq!(EasingStyle::from_value(12), None);
    assert_eq!(EasingDirection::from_name("InOut").map(EasingDirection::value), Some(2));
}

#[test]
fn tween_info() {
    let info = TweenInfo::default();
    assert_eq!(info.time, 1.0);
    assert_eq!(info.easing_style, EasingStyle::Quad);
    assert_eq!(info.easing_direction, EasingDirection::Out);
    assert_eq!((info.repeat_count, info.reverses, info.delay_time), (0, false, 0.0));
    assert_eq!(
        info.to_string(),
        "Time:1 DelayTime:0 RepeatCount:0 Reverses:False EasingDirection:Out EasingStyle:Quad"
    );
    let custom = TweenInfo::new(0.5, EasingStyle::Back, EasingDirection::InOut, -1, true, 2.0);
    assert_eq!(
        custom.to_string(),
        "Time:0.5 DelayTime:2 RepeatCount:-1 Reverses:True EasingDirection:InOut EasingStyle:Back"
    );
    assert_close(info.value(0.5), 0.75);
}

// ---------------------------------------------------------------- BrickColor

#[test]
fn brick_color_by_number_and_name() {
    let white = BrickColor::new(1);
    assert_eq!(white.name(), "White");
    assert_eq!(white.number(), 1);
    assert_eq!(white.color(), Color3::from_rgb(242.0, 243.0, 243.0));
    assert_close(white.r(), 242.0 / 255.0);
    assert_close(white.b(), 243.0 / 255.0);
    assert_eq!(BrickColor::new(194).name(), "Medium stone grey");
    assert_eq!(BrickColor::new(1032).name(), "Hot pink");
    assert_eq!(BrickColor::new(1032).color(), Color3::from_rgb(255.0, 0.0, 191.0));
    assert_eq!(BrickColor::from_name("Pastel Blue").number(), 11);
    assert_eq!(BrickColor::from_name("Really black").number(), 1003);
    assert_eq!(BrickColor::from_name("Light green (Mint)").number(), 6);
    // Unknown names and numbers give the default.
    assert_eq!(BrickColor::new(4).number(), 194);
    assert_eq!(BrickColor::new(-1).number(), 194);
    assert_eq!(BrickColor::from_name("pastel blue").number(), 194);
    assert_eq!(BrickColor::default().number(), 194);
    // Names two colors share go to the lower number.
    assert_eq!(BrickColor::from_name("Rust").number(), 216);
    assert_eq!(BrickColor::from_name("Gold").number(), 127);
    assert_eq!(BrickColor::from_name("Deep orange").number(), 1005);
    assert_eq!(BrickColor::new(23).to_string(), "Bright blue");
    assert_eq!(BrickColor::new(21), BrickColor::from_name("Bright red"));
}

#[test]
fn brick_color_shortcuts() {
    let named = [
        (BrickColor::white(), 1, "White"),
        (BrickColor::gray(), 194, "Medium stone grey"),
        (BrickColor::dark_gray(), 199, "Dark stone grey"),
        (BrickColor::black(), 26, "Black"),
        (BrickColor::red(), 21, "Bright red"),
        (BrickColor::yellow(), 24, "Bright yellow"),
        (BrickColor::green(), 28, "Dark green"),
        (BrickColor::blue(), 23, "Bright blue"),
    ];
    for (color, number, name) in named {
        assert_eq!((color.number(), color.name()), (number, name));
    }
}

#[test]
fn brick_color_palette() {
    assert_eq!(BrickColor::all().count(), 208);
    assert_eq!(BrickColor::palette(0).unwrap().name(), "Earth green");
    assert_eq!(BrickColor::palette(21).unwrap().name(), "Bright blue");
    assert_eq!(BrickColor::palette(127).unwrap().name(), "Really black");
    assert!(BrickColor::palette(128).is_err());
    assert!(BrickColor::palette(-1).is_err());
    // The 128 palette indices each name one color.
    let mut seen = [false; 128];
    for c in BrickColor::all() {
        if let Some(i) = c.palette_index() {
            assert!(!seen[usize::from(i)]);
            seen[usize::from(i)] = true;
            assert_eq!(BrickColor::palette(i.into()).unwrap(), c);
        }
    }
    assert!(seen.iter().all(|&s| s));
    assert_eq!(BrickColor::new(2).palette_index(), None);
    let mut rng = Random::new(7.0);
    for _ in 0..200 {
        assert!(BrickColor::random(&mut rng).palette_index().is_some());
    }
}

#[test]
fn brick_color_nearest() {
    assert_eq!(BrickColor::from_rgb(1.0, 0.0, 0.0).name(), "Really red");
    assert_eq!(BrickColor::from_rgb(0.0, 0.0, 1.0).name(), "Really blue");
    assert_eq!(BrickColor::from_rgb(0.99, 0.02, 0.01).name(), "Really red");
    assert_eq!(
        BrickColor::from_color3(Color3::from_rgb(242.0, 243.0, 242.0)).name(),
        "White"
    );
    assert_eq!(
        BrickColor::from_color3(Color3::new(0.0, 0.0, 0.0)).name(),
        "Really black"
    );
    // Every color finds a color of exactly its own RGB.
    for c in BrickColor::all() {
        assert_eq!(BrickColor::from_color3(c.color()).color(), c.color(), "{c}");
    }
}

// ---------------------------------------------------------------- Font

#[test]
fn font_from_enum() {
    let bold = Font::from_enum(FontEnum::SourceSansBold).unwrap();
    assert_eq!(bold.family, "rbxasset://fonts/families/SourceSansPro.json");
    assert_eq!(bold.weight, FontWeight::Bold);
    assert_eq!(bold.style, FontStyle::Normal);
    let italic = Font::from_enum(FontEnum::SourceSansItalic).unwrap();
    assert_eq!((italic.weight, italic.style), (FontWeight::Regular, FontStyle::Italic));
    assert_eq!(
        Font::from_enum(FontEnum::Arcade).unwrap().family,
        "rbxasset://fonts/families/PressStart2P.json"
    );
    assert_eq!(
        Font::from_enum(FontEnum::Code).unwrap().family,
        "rbxasset://fonts/families/Inconsolata.json"
    );
    assert_eq!(
        Font::from_enum(FontEnum::Antique).unwrap().family,
        "rbxasset://fonts/families/RomanAntique.json"
    );
    assert_eq!(
        Font::from_enum(FontEnum::Ubuntu).unwrap().family,
        "rbxasset://fonts/families/Ubuntu.json"
    );
    let black = Font::from_enum(FontEnum::GothamBlack).unwrap();
    assert_eq!(
        (black.family.as_str(), black.weight),
        ("rbxasset://fonts/families/Montserrat.json", FontWeight::Heavy)
    );
    assert!(Font::from_enum(FontEnum::Unknown).is_err());
}

#[test]
fn font_enum_round_trip_and_aliases() {
    for &e in FontEnum::ALL {
        if e == FontEnum::Unknown {
            continue;
        }
        let font = Font::from_enum(e).unwrap();
        // Arial and Legacy are the same face; the first of them reads back.
        let want = if e == FontEnum::Arial { FontEnum::Legacy } else { e };
        assert_eq!(font.to_enum(), want, "{e:?}");
    }
    assert_eq!(
        Font::from_name("Roboto", FontWeight::Thin, FontStyle::Normal).to_enum(),
        FontEnum::Unknown
    );
    assert_eq!(
        FontEnum::from_name_or_alias("GothamSemibold"),
        Some(FontEnum::GothamMedium)
    );
    assert_eq!(FontEnum::from_name_or_alias("Montserrat"), Some(FontEnum::Gotham));
    assert_eq!(
        FontEnum::from_name_or_alias("MontserratBlack"),
        Some(FontEnum::GothamBlack)
    );
    assert_eq!(FontEnum::from_name_or_alias("SourceSans"), Some(FontEnum::SourceSans));
    assert_eq!(FontEnum::from_name("GothamSemibold"), None);
    assert_eq!(FontEnum::Unknown.value(), 100);
    assert_eq!(FontEnum::from_value(19), Some(FontEnum::GothamBold));
}

#[test]
fn font_constructors_and_print() {
    let default = Font::default();
    assert_eq!(default.family, "rbxasset://fonts/families/LegacyArial.json");
    assert_eq!(default.to_enum(), FontEnum::Legacy);
    assert_eq!(
        Font::from_id(12187365364, FontWeight::Bold, FontStyle::Italic).family,
        "rbxassetid://12187365364"
    );
    let mut f = Font::new(
        "rbxasset://fonts/families/Nunito.json",
        FontWeight::Regular,
        FontStyle::Normal,
    );
    assert_eq!(f.to_enum(), FontEnum::Nunito);
    assert!(!f.bold());
    f.set_bold(true);
    assert_eq!(f.weight, FontWeight::Bold);
    assert!(f.bold());
    assert!(Font::from_name("Nunito", FontWeight::Heavy, FontStyle::Normal).bold());
    assert!(!Font::from_name("Nunito", FontWeight::SemiBold, FontStyle::Normal).bold());
    f.set_bold(false);
    assert_eq!(f.weight, FontWeight::Regular);
    assert_eq!(
        Font::from_enum(FontEnum::SourceSans).unwrap().to_string(),
        "Font { Family = rbxasset://fonts/families/SourceSansPro.json, Weight = Regular, Style = Normal }"
    );
    assert_eq!(FontWeight::from_value(600), Some(FontWeight::SemiBold));
}

// ---------------------------------------------------------------- Random

#[test]
fn random_is_pcg32() {
    // The reference implementation's demo: pcg32_srandom_r(42, 54).
    let mut rng = Random::pcg32(42, 54);
    let got: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
    assert_eq!(
        got,
        [0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e]
    );
}

#[test]
fn random_seeds() {
    let draw = |mut r: Random| (0..8).map(|_| r.next_u32()).collect::<Vec<_>>();
    assert_eq!(draw(Random::new(5.0)), draw(Random::new(5.0)));
    assert_ne!(draw(Random::new(5.0)), draw(Random::new(6.0)));
    // Seeds are floored; 0 and 0.99 are the same generator, -0.5 is -1.
    assert_eq!(draw(Random::new(0.0)), draw(Random::new(0.99)));
    assert_eq!(draw(Random::new(-0.5)), draw(Random::new(-1.0)));
    // Outside ±(2^53 - 1), and NaN, are 0.
    assert_eq!(draw(Random::new(1e300)), draw(Random::new(0.0)));
    assert_eq!(draw(Random::new(f64::NAN)), draw(Random::new(0.0)));
    assert_ne!(draw(Random::new(9007199254740991.0)), draw(Random::new(0.0)));
    assert_eq!(draw(Random::new(42.0)), draw(Random::pcg32(42, 105)));
    // Unseeded generators differ.
    assert_ne!(draw(Random::from_entropy()), draw(Random::from_entropy()));
}

#[test]
fn random_next_integer() {
    let mut rng = Random::new(1.0);
    let mut counts = [0u32; 6];
    for _ in 0..12000 {
        let n = rng.next_integer(1.0, 6.0);
        assert!((1..=6).contains(&n));
        counts[(n - 1) as usize] += 1;
    }
    // Both ends come up, about evenly.
    assert!(counts.iter().all(|&c| (1800..2200).contains(&c)), "{counts:?}");
    // Swapped bounds, truncated arguments, a single value.
    for _ in 0..200 {
        let n = rng.next_integer(10.0, -10.0);
        assert!((-10..=10).contains(&n));
        let t = rng.next_integer(1.9, 3.9);
        assert!((1..=3).contains(&t));
        assert_eq!(rng.next_integer(-2.7, -2.2), -2);
    }
    assert_eq!(rng.next_integer(5.0, 5.0), 5);
    // Ranges past 32 bits, and the widest.
    for _ in 0..200 {
        let big = rng.next_integer(0.0, 1e15);
        assert!((0..=1_000_000_000_000_000).contains(&big));
        rng.next_integer(-9.2e18, 9.2e18);
    }
    let spread: Vec<i64> = (0..64).map(|_| rng.next_integer(0.0, 1e15)).collect();
    assert!(spread.iter().any(|&n| n > 500_000_000_000_000));
}

#[test]
fn random_next_number() {
    let mut rng = Random::new(99.0);
    let mut sum = 0.0;
    for _ in 0..10000 {
        let x = rng.next_number();
        assert!((0.0..=1.0).contains(&x));
        sum += x;
        let y = rng.next_number_between(-5.0, 5.0);
        assert!((-5.0..=5.0).contains(&y));
    }
    assert!((sum / 10000.0 - 0.5).abs() < 0.02);
    let mut total = Vector3::ZERO;
    for _ in 0..4000 {
        let v = rng.next_unit_vector();
        assert_close(v.magnitude(), 1.0);
        total = total + v;
    }
    assert!((total / 4000.0).magnitude() < 0.06);
}

#[test]
fn random_shuffle_and_clone() {
    let mut rng = Random::new(3.0);
    let mut items: Vec<u32> = (0..50).collect();
    rng.shuffle(&mut items);
    assert_ne!(items, (0..50).collect::<Vec<_>>());
    let mut sorted = items.clone();
    sorted.sort();
    assert_eq!(sorted, (0..50).collect::<Vec<_>>());
    // The same seed shuffles the same way.
    let mut again: Vec<u32> = (0..50).collect();
    Random::new(3.0).shuffle(&mut again);
    assert_eq!(again, items);
    let mut empty: [u8; 0] = [];
    rng.shuffle(&mut empty);
    let mut one = [7];
    rng.shuffle(&mut one);
    assert_eq!(one, [7]);
    // A clone continues the same sequence, independently.
    let mut a = Random::new(11.0);
    a.next_u32();
    let mut b = a.clone();
    let from_a: Vec<i64> = (0..5).map(|_| a.next_integer(0.0, 1000.0)).collect();
    let from_b: Vec<i64> = (0..5).map(|_| b.next_integer(0.0, 1000.0)).collect();
    assert_eq!(from_a, from_b);
    a.next_u32();
    assert_ne!(a.next_u32(), b.next_u32());
}

#[test]
fn region3s_by_their_corners() {
    let r = Region3::new(Vector3::new(4.0, 64.0, 3.0), Vector3::new(0.0, 60.5, 0.0));
    assert_v3(r.min, 0.0, 60.5, 0.0);
    assert_v3(r.max, 4.0, 64.0, 3.0);
    assert_v3(r.size(), 4.0, 3.5, 3.0);
    assert_cf(r.cframe(), CFrame::new(2.0, 62.25, 1.5));
    // Out to the grid, never in.
    let g = r.expand_to_grid(4.0);
    assert_v3(g.min, 0.0, 60.0, 0.0);
    assert_v3(g.max, 4.0, 64.0, 4.0);
    assert_eq!(
        r.expand_to_grid(0.5),
        Region3::new(Vector3::new(0.0, 60.5, 0.0), Vector3::new(4.0, 64.0, 3.0))
    );
    assert_eq!(
        Region3::new(Vector3::ZERO, Vector3::ONE * 2.0).to_string(),
        "1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1; 2, 2, 2"
    );
}
