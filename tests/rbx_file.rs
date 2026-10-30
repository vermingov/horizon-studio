// hstudio_rbx_file on binary files built here by a small encoder that
// follows rbx-dom's docs/binary.md, checked with the document's own example
// bytes for each type where it gives them; on XML files, with entities,
// CDATA and every type element; on the same model in both formats; and on
// broken input of every kind (truncations, forged counts and lengths, bit
// flips, random bytes), which must fail cleanly or at least not panic.

use horizon_studio::game::hstudio_crypt::{from_hex, lz4_compress};
use horizon_studio::game::hstudio_rbx_file::{
    CFrame, Color3, ColorKeypoint, Content, CustomPhysics, Font, Limits, NumberKeypoint, NumberRange, PropValue, Ray,
    RbxFile, Rect, UDim, UDim2, UniqueId, Vector2, Vector3, read, read_binary, read_with, read_xml, rotation_from_id,
    xxh64, zstd_decompress,
};

/// Hex as bytes; spaces are skipped.
fn h(s: &str) -> Vec<u8> {
    from_hex(s).unwrap()
}

fn near(a: f32, b: f32) -> bool {
    a == b || (a - b).abs() <= 1e-5 * b.abs().max(1.0)
}

fn near3(a: Vector3, b: [f32; 3]) -> bool {
    near(a.x, b[0]) && near(a.y, b[1]) && near(a.z, b[2])
}

fn near_m(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> bool {
    a.iter().flatten().zip(b.iter().flatten()).all(|(&x, &y)| near(x, y))
}

// ---------------------------------------------------------------- the encoder

/// The integer transformation: 0, -1, 1, -2... become 0, 1, 2, 3...
fn transform(x: i32) -> u32 {
    ((x << 1) ^ (x >> 31)) as u32
}

fn transform64(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}

/// Values of N bytes, interleaved: every value's first byte, then every
/// second byte, and so on.
fn interleave<const N: usize>(values: &[[u8; N]]) -> Vec<u8> {
    let n = values.len();
    let mut out = vec![0; n * N];
    for (i, v) in values.iter().enumerate() {
        for (b, &byte) in v.iter().enumerate() {
            out[b * n + i] = byte;
        }
    }
    out
}

fn ints(v: &[i32]) -> Vec<u8> {
    interleave(&v.iter().map(|&x| transform(x).to_be_bytes()).collect::<Vec<_>>())
}

fn uints(v: &[u32]) -> Vec<u8> {
    interleave(&v.iter().map(|x| x.to_be_bytes()).collect::<Vec<_>>())
}

fn longs(v: &[i64]) -> Vec<u8> {
    interleave(&v.iter().map(|&x| transform64(x).to_be_bytes()).collect::<Vec<_>>())
}

/// Roblox floats: the sign moved to the lowest bit, big-endian, interleaved.
fn floats(v: &[f32]) -> Vec<u8> {
    interleave(
        &v.iter()
            .map(|x| x.to_bits().rotate_left(1).to_be_bytes())
            .collect::<Vec<_>>(),
    )
}

/// Referents: each the difference from the one before, as ints.
fn refs(v: &[i32]) -> Vec<u8> {
    let mut last = 0i32;
    let deltas: Vec<i32> = v
        .iter()
        .map(|&r| {
            let d = r.wrapping_sub(last);
            last = r;
            d
        })
        .collect();
    ints(&deltas)
}

fn string(s: &[u8]) -> Vec<u8> {
    [&(s.len() as u32).to_le_bytes()[..], s].concat()
}

fn chunk(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
    [
        &name[..],
        &0u32.to_le_bytes(),
        &(data.len() as u32).to_le_bytes(),
        &[0; 4],
        data,
    ]
    .concat()
}

fn lz4_chunk(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let packed = lz4_compress(data);
    [
        &name[..],
        &(packed.len() as u32).to_le_bytes(),
        &(data.len() as u32).to_le_bytes(),
        &[0; 4],
        &packed,
    ]
    .concat()
}

fn inst_data(id: u32, class: &str, referents: &[i32], service: bool) -> Vec<u8> {
    let mut d = id.to_le_bytes().to_vec();
    d.extend(string(class.as_bytes()));
    d.push(u8::from(service));
    d.extend((referents.len() as u32).to_le_bytes());
    d.extend(refs(referents));
    if service {
        d.extend(vec![1; referents.len()]);
    }
    d
}

fn inst(id: u32, class: &str, referents: &[i32]) -> Vec<u8> {
    chunk(b"INST", &inst_data(id, class, referents, false))
}

fn prop_data(id: u32, name: &str, ty: u8, values: &[u8]) -> Vec<u8> {
    let mut d = id.to_le_bytes().to_vec();
    d.extend(string(name.as_bytes()));
    d.push(ty);
    d.extend(values);
    d
}

fn prop(id: u32, name: &str, ty: u8, values: &[u8]) -> Vec<u8> {
    chunk(b"PROP", &prop_data(id, name, ty, values))
}

/// A PROP chunk of strings.
fn strings(id: u32, name: &str, values: &[&str]) -> Vec<u8> {
    prop(
        id,
        name,
        0x01,
        &values.iter().flat_map(|s| string(s.as_bytes())).collect::<Vec<_>>(),
    )
}

fn prnt_data(pairs: &[(i32, i32)]) -> Vec<u8> {
    let mut d = vec![0];
    d.extend((pairs.len() as u32).to_le_bytes());
    d.extend(refs(&pairs.iter().map(|p| p.0).collect::<Vec<_>>()));
    d.extend(refs(&pairs.iter().map(|p| p.1).collect::<Vec<_>>()));
    d
}

/// Children and their parents (-1 for none).
fn prnt(pairs: &[(i32, i32)]) -> Vec<u8> {
    chunk(b"PRNT", &prnt_data(pairs))
}

fn meta(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut d = (pairs.len() as u32).to_le_bytes().to_vec();
    for (k, v) in pairs {
        d.extend(string(k.as_bytes()));
        d.extend(string(v.as_bytes()));
    }
    chunk(b"META", &d)
}

fn sstr(values: &[&[u8]]) -> Vec<u8> {
    let mut d = 0u32.to_le_bytes().to_vec();
    d.extend((values.len() as u32).to_le_bytes());
    for v in values {
        d.extend([0; 16]);
        d.extend(string(v));
    }
    chunk(b"SSTR", &d)
}

fn header() -> Vec<u8> {
    [
        &b"<roblox!"[..],
        &[0x89, 0xff, 0x0d, 0x0a, 0x1a, 0x0a],
        &[0, 0],
        &[0; 4],
        &[0; 4],
        &[0; 8],
    ]
    .concat()
}

/// A whole file: the header, the chunks and END.
fn file(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut f = header();
    for c in chunks {
        f.extend(c);
    }
    f.extend(chunk(b"END\0", b"</roblox>"));
    f
}

/// One property of `n` Things (referents 0 to n - 1), decoded.
fn values(ty: u8, n: usize, data: &[u8]) -> Vec<PropValue> {
    let referents: Vec<i32> = (0..n as i32).collect();
    let f = read(&file(&[inst(0, "Thing", &referents), prop(0, "P", ty, data)])).unwrap();
    assert!(f.notes.is_empty(), "{:?}", f.notes);
    f.items.iter().map(|i| i.get("P").unwrap().clone()).collect()
}

fn floats_of(v: &[PropValue]) -> Vec<f32> {
    v.iter()
        .map(|v| match v {
            PropValue::Float32(f) => *f,
            v => panic!("{v:?}"),
        })
        .collect()
}

// ---------------------------------------------------------------- the file and its tree

#[test]
fn header_chunks_and_tree() {
    // Folder 10 { Part 20 { Model 40 }, Part 30 }, written as Studio does:
    // parenting in post-order, descendants first.
    let f = file(&[
        meta(&[("ExplicitAutoJoints", "true")]),
        inst(0, "Folder", &[10]),
        inst(1, "Part", &[20, 30]),
        inst(2, "Model", &[40]),
        strings(0, "Name", &["Root"]),
        strings(1, "Name", &["A", "B"]),
        strings(2, "Name", &["M"]),
        prnt(&[(40, 20), (20, 10), (30, 10), (10, -1)]),
    ]);
    let m = read(&f).unwrap();
    assert_eq!(m.meta, vec![("ExplicitAutoJoints".to_string(), "true".to_string())]);
    assert!(m.notes.is_empty());
    let classes: Vec<_> = m
        .items
        .iter()
        .map(|i| (i.class.as_str(), i.referent.as_str()))
        .collect();
    assert_eq!(
        classes,
        [("Folder", "10"), ("Part", "20"), ("Part", "30"), ("Model", "40")]
    );
    let names: Vec<String> = m.items.iter().map(|i| i.name().into_owned()).collect();
    assert_eq!(names, ["Root", "A", "B", "M"]);
    assert_eq!(m.roots, [0]);
    assert_eq!(m.items[0].children, [1, 2]);
    assert_eq!(m.items[1].children, [3]);
    assert_eq!(m.items[3].parent, Some(1));
    assert_eq!(m.items[0].parent, None);
    assert!(!m.items[0].service);
}

#[test]
fn several_roots_and_unparented_items() {
    // 1 and 2 are roots in PRNT; 3 isn't in PRNT at all, so it is one too.
    let f = file(&[inst(0, "Folder", &[1, 2, 3]), prnt(&[(2, -1), (1, -1)])]);
    let m = read(&f).unwrap();
    assert_eq!(m.roots, [1, 0, 2]);
    // No PRNT chunk: every item is a root.
    let m = read(&file(&[inst(0, "Folder", &[5, 6])])).unwrap();
    assert_eq!(m.roots, [0, 1]);
}

#[test]
fn services_are_marked() {
    let f = file(&[
        chunk(b"INST", &inst_data(0, "Workspace", &[0], true)),
        inst(1, "Part", &[1]),
        prnt(&[(1, 0), (0, -1)]),
    ]);
    let m = read(&f).unwrap();
    assert!(m.items[0].service);
    assert!(!m.items[1].service);
}

#[test]
fn referents_accumulate() {
    // The document's example: stored 1619, 1, 4, 2, 3, 5 are 1619, 1620, 1624, 1626, 1629, 1634.
    let mut d = 0u32.to_le_bytes().to_vec();
    d.extend(string(b"Folder"));
    d.push(0);
    d.extend(6u32.to_le_bytes());
    d.extend(ints(&[1619, 1, 4, 2, 3, 5]));
    let m = read(&file(&[chunk(b"INST", &d)])).unwrap();
    let r: Vec<_> = m.items.iter().map(|i| i.referent.as_str()).collect();
    assert_eq!(r, ["1619", "1620", "1624", "1626", "1629", "1634"]);
    // Negative steps, and the ends of the range.
    let all = [i32::MAX, i32::MIN, -1, 0, 7, -7];
    let m = read(&file(&[inst(0, "Folder", &all)])).unwrap();
    let r: Vec<_> = m.items.iter().map(|i| i.referent.parse::<i32>().unwrap()).collect();
    assert_eq!(r, all);
}

#[test]
fn unknown_chunks_and_signatures_are_skipped() {
    let f = file(&[
        inst(0, "Folder", &[0]),
        chunk(b"SIGN", b"whatever"),
        chunk(b"ABCD", b"x"),
    ]);
    let m = read(&f).unwrap();
    assert_eq!(m.items.len(), 1);
    assert_eq!(m.notes, ["skipped an unknown ABCD chunk"]);
}

#[test]
fn lz4_chunks_decode_like_plain_ones() {
    let plain = [
        meta(&[("ExplicitAutoJoints", "true")]),
        inst(0, "Frame", &[0, 1, 2]),
        strings(0, "Name", &["One", "Two", "Three"]),
        prop(0, "Size", 0x0e, &floats(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0])),
        prnt(&[(1, 0), (2, 0), (0, -1)]),
    ];
    let packed: Vec<Vec<u8>> = plain
        .iter()
        .map(|c| {
            let name: [u8; 4] = c[..4].try_into().unwrap();
            lz4_chunk(&name, &c[16..])
        })
        .collect();
    assert_ne!(file(&plain), file(&packed));
    let (a, b) = (read(&file(&plain)).unwrap(), read(&file(&packed)).unwrap());
    assert_eq!(a, b);
    assert_eq!(a.items[2].name(), "Three");
    assert_eq!(a.items[0].children, [1, 2]);
    // A long, repetitive chunk compresses well and still decodes.
    let names: Vec<String> = (0..500).map(|i| format!("Part{}", i % 7)).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let referents: Vec<i32> = (0..500).collect();
    let body = &strings(0, "Name", &names)[16..];
    let m = read(&file(&[inst(0, "Part", &referents), lz4_chunk(b"PROP", body)])).unwrap();
    assert_eq!(m.items[499].name(), "Part2");
}

// ---------------------------------------------------------------- property types

#[test]
fn string_bool_and_bytecode() {
    let v = values(0x01, 3, &[string(b"hi"), string(b""), string(&[0xff, 0, 1])].concat());
    assert_eq!(v[0], PropValue::String(b"hi".to_vec()));
    assert_eq!(v[0].as_str(), Some("hi"));
    assert_eq!(v[1], PropValue::String(vec![]));
    assert_eq!(v[2], PropValue::String(vec![0xff, 0, 1]));
    assert_eq!(v[2].as_str(), None);
    let v = values(0x02, 3, &[1, 0, 1]);
    assert_eq!(
        v,
        [PropValue::Bool(true), PropValue::Bool(false), PropValue::Bool(true)]
    );
    let v = values(0x1d, 1, &string(&[0x1b, 0x4c, 0x75, 0x61]));
    assert_eq!(v, [PropValue::Bytecode(vec![0x1b, 0x4c, 0x75, 0x61])]);
}

#[test]
fn int32_is_transformed_and_interleaved() {
    // 1 and -2 transform to 2 and 3: big-endian, interleaved.
    assert_eq!(
        values(0x03, 2, &h("00 00 00 00 00 00 02 03")),
        [PropValue::Int32(1), PropValue::Int32(-2)]
    );
    let all = [0, -1, 1, i32::MAX, i32::MIN, 123_456_789, -987_654_321];
    let v = values(0x03, all.len(), &ints(&all));
    assert_eq!(v, all.map(PropValue::Int32));
}

#[test]
fn int64_is_transformed_and_interleaved() {
    // 1 and -1 transform to 2 and 1.
    let v = values(0x1b, 2, &h("0000 0000 0000 0000 0000 0000 0000 0201"));
    assert_eq!(v, [PropValue::Int64(1), PropValue::Int64(-1)]);
    let all = [0, i64::MAX, i64::MIN, -5_000_000_000, 5_000_000_000];
    assert_eq!(values(0x1b, all.len(), &longs(&all)), all.map(PropValue::Int64));
}

#[test]
fn float32_moves_its_sign() {
    // The document's -0.15625 is 7c 40 00 01; 1.0 is 7f 00 00 00.
    let v = floats_of(&values(0x04, 2, &h("7c7f 4000 0000 0100")));
    assert_eq!(v, [-0.15625, 1.0]);
    let all = [
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        3.4e38,
        -1e-30,
    ];
    let v = floats_of(&values(0x04, all.len(), &floats(&all)));
    assert_eq!(v.iter().map(|f| f.to_bits()).collect::<Vec<_>>(), all.map(f32::to_bits));
    assert!(floats_of(&values(0x04, 1, &floats(&[f32::NAN])))[0].is_nan());
}

#[test]
fn float64_is_plain() {
    let d = [0.1f64.to_le_bytes(), (-2.5e300f64).to_le_bytes()].concat();
    assert_eq!(
        values(0x05, 2, &d),
        [PropValue::Float64(0.1), PropValue::Float64(-2.5e300)]
    );
}

#[test]
fn udim_and_udim2() {
    // The document's {1, 2} and {3, 4}.
    let v = values(0x06, 2, &h("7f 80 00 80 00 00 00 00 00 00 00 00 00 00 04 08"));
    assert_eq!(
        v,
        [
            PropValue::UDim(UDim { scale: 1.0, offset: 2 }),
            PropValue::UDim(UDim { scale: 3.0, offset: 4 })
        ]
    );
    // The document's {0.75, -30, -1.5, 60}: X.Scale, Y.Scale, X.Offset, Y.Offset.
    let v = values(0x07, 1, &h("7e 80 00 00 7f 80 00 01 00 00 00 3b 00 00 00 78"));
    let want = UDim2 {
        x: UDim {
            scale: 0.75,
            offset: -30,
        },
        y: UDim {
            scale: -1.5,
            offset: 60,
        },
    };
    assert_eq!(v, [PropValue::UDim2(want)]);
}

#[test]
fn ray_faces_axes() {
    let d: Vec<u8> = [1.0f32, 2.0, 3.0, -1.0, 0.0, 0.5]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let want = Ray {
        origin: Vector3 { x: 1.0, y: 2.0, z: 3.0 },
        direction: Vector3 {
            x: -1.0,
            y: 0.0,
            z: 0.5,
        },
    };
    assert_eq!(values(0x08, 1, &d), [PropValue::Ray(want)]);
    // Bytes as they are, without the meaningless high bits.
    assert_eq!(
        values(0x09, 4, &h("01 18 26 ff")),
        [1, 0x18, 0x26, 63].map(PropValue::Faces)
    );
    assert_eq!(values(0x0a, 4, &h("01 03 05 ff")), [1, 3, 5, 7].map(PropValue::Axes));
}

#[test]
fn brickcolor_and_enum() {
    // The document's Really red (1004), Bright green (37) and Really blue (1010).
    let v = values(0x0b, 3, &h("00 00 00 00 00 00 03 00 03 EC 25 F2"));
    assert_eq!(v, [1004, 37, 1010].map(PropValue::BrickColor));
    // Enums aren't transformed.
    let v = values(0x12, 3, &uints(&[0, 3, u32::MAX]));
    assert_eq!(v, [0, 3, u32::MAX].map(PropValue::Enum));
    assert_eq!(values(0x12, 1, &h("00 00 00 05")), [PropValue::Enum(5)]);
}

#[test]
fn color3_vector2_vector3() {
    // The document's 255, 180, 20.
    let v = values(0x0c, 1, &h("7f 00 00 00 7e 69 69 6a 7b 41 41 42"));
    let PropValue::Color3(c) = v[0] else { panic!("{v:?}") };
    assert!(
        near(c.r, 1.0) && near(c.g, 180.0 / 255.0) && near(c.b, 20.0 / 255.0),
        "{c:?}"
    );
    // The document's (-100.80, 200.55) and (200.55, -100.80).
    let v = values(0x0d, 2, &h("85 86 93 91 33 19 35 9a 86 85 91 93 19 33 9a 35"));
    assert_eq!(
        v,
        [
            PropValue::Vector2(Vector2 { x: -100.80, y: 200.55 }),
            PropValue::Vector2(Vector2 { x: 200.55, y: -100.80 })
        ]
    );
    // The document's (1, 2, 3) and (-1, -2, -3).
    let v = values(
        0x0e,
        2,
        &h("7F 7F 00 00 00 00 00 01 80 80 00 00 00 00 00 01 80 80 80 80 00 00 00 01"),
    );
    assert_eq!(
        v,
        [
            PropValue::Vector3(Vector3 { x: 1.0, y: 2.0, z: 3.0 }),
            PropValue::Vector3(Vector3 {
                x: -1.0,
                y: -2.0,
                z: -3.0
            })
        ]
    );
}

/// rbx-dom's matrices for the 24 rotation IDs, row by row.
const ROTATIONS: [(u8, [[f32; 3]; 3]); 24] = [
    (0x02, [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]),
    (0x03, [[1., 0., 0.], [0., 0., -1.], [0., 1., 0.]]),
    (0x05, [[1., 0., 0.], [0., -1., 0.], [0., 0., -1.]]),
    (0x06, [[1., 0., 0.], [0., 0., 1.], [0., -1., 0.]]),
    (0x07, [[0., 1., 0.], [1., 0., 0.], [0., 0., -1.]]),
    (0x09, [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]]),
    (0x0a, [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]]),
    (0x0c, [[0., 0., -1.], [1., 0., 0.], [0., -1., 0.]]),
    (0x0d, [[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]]),
    (0x0e, [[0., 0., -1.], [0., 1., 0.], [1., 0., 0.]]),
    (0x10, [[0., -1., 0.], [0., 0., -1.], [1., 0., 0.]]),
    (0x11, [[0., 0., 1.], [0., -1., 0.], [1., 0., 0.]]),
    (0x14, [[-1., 0., 0.], [0., 1., 0.], [0., 0., -1.]]),
    (0x15, [[-1., 0., 0.], [0., 0., 1.], [0., 1., 0.]]),
    (0x17, [[-1., 0., 0.], [0., -1., 0.], [0., 0., 1.]]),
    (0x18, [[-1., 0., 0.], [0., 0., -1.], [0., -1., 0.]]),
    (0x19, [[0., 1., 0.], [-1., 0., 0.], [0., 0., 1.]]),
    (0x1b, [[0., 0., -1.], [-1., 0., 0.], [0., 1., 0.]]),
    (0x1c, [[0., -1., 0.], [-1., 0., 0.], [0., 0., -1.]]),
    (0x1e, [[0., 0., 1.], [-1., 0., 0.], [0., -1., 0.]]),
    (0x1f, [[0., 1., 0.], [0., 0., -1.], [-1., 0., 0.]]),
    (0x20, [[0., 0., 1.], [0., 1., 0.], [-1., 0., 0.]]),
    (0x22, [[0., -1., 0.], [0., 0., 1.], [-1., 0., 0.]]),
    (0x23, [[0., 0., -1.], [0., -1., 0.], [-1., 0., 0.]]),
];

/// The document's table: each ID's rotation in degrees, applied Y, then X, then Z.
const ANGLES: [(u8, [f64; 3]); 24] = [
    (0x02, [0., 0., 0.]),
    (0x03, [90., 0., 0.]),
    (0x05, [0., 180., 180.]),
    (0x06, [-90., 0., 0.]),
    (0x07, [0., 180., 90.]),
    (0x09, [0., 90., 90.]),
    (0x0a, [0., 0., 90.]),
    (0x0c, [0., -90., 90.]),
    (0x0d, [-90., -90., 0.]),
    (0x0e, [0., -90., 0.]),
    (0x10, [90., -90., 0.]),
    (0x11, [0., 90., 180.]),
    (0x14, [0., 180., 0.]),
    (0x15, [-90., -180., 0.]),
    (0x17, [0., 0., 180.]),
    (0x18, [90., 180., 0.]),
    (0x19, [0., 0., -90.]),
    (0x1b, [0., -90., -90.]),
    (0x1c, [0., -180., -90.]),
    (0x1e, [0., 90., -90.]),
    (0x1f, [90., 90., 0.]),
    (0x20, [0., 90., 0.]),
    (0x22, [-90., 90., 0.]),
    (0x23, [0., -90., 180.]),
];

fn mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

fn rx(t: f64) -> [[f64; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, t.cos(), -t.sin()], [0.0, t.sin(), t.cos()]]
}

fn ry(t: f64) -> [[f64; 3]; 3] {
    [[t.cos(), 0.0, t.sin()], [0.0, 1.0, 0.0], [-t.sin(), 0.0, t.cos()]]
}

fn rz(t: f64) -> [[f64; 3]; 3] {
    [[t.cos(), -t.sin(), 0.0], [t.sin(), t.cos(), 0.0], [0.0, 0.0, 1.0]]
}

fn to_f32(m: [[f64; 3]; 3]) -> [[f32; 3]; 3] {
    m.map(|r| r.map(|v| v as f32))
}

#[test]
fn cframe_rotation_ids_are_rbx_doms_and_the_documents() {
    for (id, want) in ROTATIONS {
        assert_eq!(rotation_from_id(id), Some(want), "ID {id:#04x}");
    }
    for (id, [x, y, z]) in ANGLES {
        let m = mul(mul(ry(y.to_radians()), rx(x.to_radians())), rz(z.to_radians()));
        assert!(near_m(rotation_from_id(id).unwrap(), to_f32(m)), "ID {id:#04x}");
    }
    let known: Vec<u8> = ROTATIONS.iter().map(|r| r.0).collect();
    for id in 0..=255u8 {
        assert_eq!(rotation_from_id(id).is_some(), known.contains(&id), "ID {id:#04x}");
    }
    // Every ID in a file, with positions.
    let n = ROTATIONS.len();
    let pos: Vec<f32> = (0..n).map(|i| i as f32).collect();
    let mut d: Vec<u8> = ROTATIONS.iter().map(|r| r.0).collect();
    d.extend(floats(&pos));
    d.extend(floats(&pos.iter().map(|p| -p).collect::<Vec<_>>()));
    d.extend(floats(&pos.iter().map(|p| p * 0.5).collect::<Vec<_>>()));
    for (i, v) in values(0x10, n, &d).into_iter().enumerate() {
        let p = i as f32;
        let want = CFrame {
            position: Vector3 {
                x: p,
                y: -p,
                z: p * 0.5,
            },
            rotation: ROTATIONS[i].1,
        };
        assert_eq!(v, PropValue::CFrame(want));
    }
}

#[test]
fn cframe_with_a_full_matrix() {
    // The document's example: CFrame.new(1, 2, 3) (ID 2) and
    // CFrame.new(4, 5, 6) * CFrame.Angles(7, 8, 9) (ID 0 and nine floats).
    let mut d = h(
        "02 00 4B C0 07 3E 08 9C 75 3D 95 46 7D 3F 1D 25 90 BE 58 6C 74 BF 84 C5 C3 3D 1E 4A 73 3F \
                   6F 19 95 BE 9F A6 E0 BD",
    );
    // Its positions, though, give a second y of 1.136, not 5; these are written here.
    d.extend(floats(&[1.0, 4.0]));
    d.extend(floats(&[2.0, 5.0]));
    d.extend(floats(&[3.0, 6.0]));
    let v = values(0x10, 2, &d);
    assert_eq!(
        v[0],
        PropValue::CFrame(CFrame {
            position: Vector3 { x: 1.0, y: 2.0, z: 3.0 },
            rotation: CFrame::IDENTITY
        })
    );
    let PropValue::CFrame(c) = v[1] else { panic!("{v:?}") };
    assert!(near3(c.position, [4.0, 5.0, 6.0]));
    // CFrame.Angles(x, y, z) rotates about X, then Y, then Z: Rx * Ry * Rz.
    let angles = mul(mul(rx(7.0), ry(8.0)), rz(9.0));
    assert!(near_m(c.rotation, to_f32(angles)), "{:?}", c.rotation);
    // The document's positions decode as it wrote them.
    let mut d = h("02 02");
    d.extend(h(
        "7F 81 00 00 00 00 00 00 80 7F 00 22 00 D4 00 B2 80 81 80 80 00 00 00 00",
    ));
    let v = values(0x10, 2, &d);
    let PropValue::CFrame(c) = v[1] else { panic!("{v:?}") };
    assert!(near3(c.position, [4.0, 1.136_059_2, 6.0]), "{c:?}");
}

#[test]
fn optional_cframe() {
    // The document's example: CFrame.new(0, 0, 1, 0, -1, 0, 1, 0, 0, 0, 0, 1), then none.
    let v = values(
        0x1e,
        2,
        &h("10 0a 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 7f 00 00 00 00 00 00 00 02 01 00"),
    );
    let want = CFrame {
        position: Vector3 { x: 0.0, y: 0.0, z: 1.0 },
        rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
    };
    assert_eq!(
        v,
        [PropValue::OptionalCFrame(Some(want)), PropValue::OptionalCFrame(None)]
    );
}

#[test]
fn referent_properties_resolve_to_items() {
    // ObjectValue.Value: 0 -> 2, 1 -> nothing, 2 -> 0, 3 -> an Instance outside the file.
    let f = file(&[
        inst(0, "ObjectValue", &[100, 101, 102, 103]),
        prop(0, "Value", 0x13, &refs(&[102, -1, 100, 999])),
    ]);
    let m = read(&f).unwrap();
    let v: Vec<_> = m.items.iter().map(|i| i.get("Value").unwrap().clone()).collect();
    assert_eq!(
        v,
        [
            PropValue::Ref(Some(2)),
            PropValue::Ref(None),
            PropValue::Ref(Some(0)),
            PropValue::Ref(None)
        ]
    );
}

#[test]
fn vector3int16_and_ranges() {
    let d: Vec<u8> = [1i16, 2, 3, -1, -2, i16::MIN]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        values(0x14, 2, &d),
        [
            PropValue::Vector3int16([1, 2, 3]),
            PropValue::Vector3int16([-1, -2, i16::MIN])
        ]
    );
    // The document's NumberRange.new(0, 0.5) and NumberRange.new(0.5, 1).
    let v = values(0x17, 2, &h("00 00 00 00 00 00 00 3f 00 00 00 3f 00 00 80 3f"));
    assert_eq!(
        v,
        [
            PropValue::NumberRange(NumberRange { min: 0.0, max: 0.5 }),
            PropValue::NumberRange(NumberRange { min: 0.5, max: 1.0 })
        ]
    );
    // The document's Rect.new(-1, -10, 8, 9) and Rect.new(0, 1, 5, 6).
    let v = values(
        0x18,
        2,
        &h("7f 00 00 00 00 00 01 00 82 7f 40 00 00 00 01 00 82 81 00 40 00 00 00 00 82 81 20 80 00 00 00 00"),
    );
    let rect = |a, b, c, d| {
        PropValue::Rect(Rect {
            min: Vector2 { x: a, y: b },
            max: Vector2 { x: c, y: d },
        })
    };
    assert_eq!(v, [rect(-1.0, -10.0, 8.0, 9.0), rect(0.0, 1.0, 5.0, 6.0)]);
}

#[test]
fn sequences() {
    // The document's two NumberSequences.
    let v = values(
        0x15,
        2,
        &h(
            "03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 3f 00 00 80 3f 00 00 00 00 00 00 80 3f \
            00 00 80 3f 00 00 00 3f 03 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 3f 00 00 00 3f \
            00 00 00 3f 00 00 80 3f 00 00 00 3f 00 00 00 00",
        ),
    );
    let k = |time, value, envelope| NumberKeypoint { time, value, envelope };
    assert_eq!(
        v,
        [
            PropValue::NumberSequence(vec![k(0.0, 0.0, 0.0), k(0.5, 1.0, 0.0), k(1.0, 1.0, 0.5)]),
            PropValue::NumberSequence(vec![k(0.0, 1.0, 0.0), k(0.5, 0.5, 0.5), k(1.0, 0.5, 0.0)]),
        ]
    );
    // The document's two ColorSequences.
    let v = values(
        0x16,
        2,
        &h(
            "03 00 00 00 00 00 00 00 00 00 80 3f 00 00 80 3f 00 00 80 3f 00 00 00 00 00 00 00 3f 00 00 00 00 \
            00 00 00 00 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 80 3f 00 00 80 3f 00 00 80 3f 00 00 00 00 \
            03 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 3f 00 00 00 00 \
            00 00 80 3f 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00 00 00 00 00 00 00 80 3f 00 00 00 00",
        ),
    );
    let k = |time, r, g, b| ColorKeypoint {
        time,
        color: Color3 { r, g, b },
        envelope: 0.0,
    };
    assert_eq!(
        v,
        [
            PropValue::ColorSequence(vec![
                k(0.0, 1.0, 1.0, 1.0),
                k(0.5, 0.0, 0.0, 0.0),
                k(1.0, 1.0, 1.0, 1.0)
            ]),
            PropValue::ColorSequence(vec![
                k(0.0, 1.0, 0.0, 0.0),
                k(0.5, 0.0, 1.0, 0.0),
                k(1.0, 0.0, 0.0, 1.0)
            ]),
        ]
    );
    // An empty sequence.
    assert_eq!(values(0x15, 1, &[0; 4]), [PropValue::NumberSequence(vec![])]);
}

#[test]
fn physical_properties() {
    // The document's four: default; custom without AcousticAbsorption;
    // default with its bit set; custom with it.
    let v = values(
        0x19,
        4,
        &h(
            "00 01 33 33 33 3f 9a 99 99 3e 00 00 00 3f 00 00 80 3f 00 00 80 3f 02 03 00 00 80 3e 00 00 00 3f \
            00 00 00 3e 00 00 80 3f 00 00 80 3e 00 00 00 3f",
        ),
    );
    let p = |density, friction, elasticity, friction_weight, elasticity_weight, acoustic_absorption| {
        PropValue::PhysicalProperties(Some(CustomPhysics {
            density,
            friction,
            elasticity,
            friction_weight,
            elasticity_weight,
            acoustic_absorption,
        }))
    };
    assert_eq!(
        v,
        [
            PropValue::PhysicalProperties(None),
            p(0.7, 0.3, 0.5, 1.0, 1.0, 1.0),
            PropValue::PhysicalProperties(None),
            p(0.25, 0.5, 0.125, 1.0, 0.25, 0.5),
        ]
    );
}

#[test]
fn color3uint8() {
    // The document's (0, 255, 255) and (63, 0, 127): all reds, greens, then blues.
    let v = values(0x1a, 2, &h("00 3f ff 00 ff 7f"));
    assert_eq!(
        v,
        [
            PropValue::Color3uint8([0, 255, 255]),
            PropValue::Color3uint8([63, 0, 127])
        ]
    );
}

#[test]
fn shared_strings() {
    let f = file(&[
        sstr(&[b"mesh data", b"\x00\x01"]),
        inst(0, "MeshPart", &[0, 1, 2]),
        prop(0, "MeshData", 0x1c, &uints(&[1, 0, 1])),
    ]);
    let m = read(&f).unwrap();
    let v: Vec<_> = m.items.iter().map(|i| i.get("MeshData").unwrap().clone()).collect();
    assert_eq!(v[0], PropValue::SharedString(b"\x00\x01"[..].into()));
    assert_eq!(v[1].as_str(), Some("mesh data"));
    assert_eq!(v[2], v[0]);
    // One the SSTR chunk doesn't have.
    let f = file(&[
        sstr(&[b"x"]),
        inst(0, "MeshPart", &[0]),
        prop(0, "MeshData", 0x1c, &uints(&[1])),
    ]);
    let e = read(&f).unwrap_err();
    assert!(e.contains("shared string 1"), "{e}");
}

#[test]
fn unique_ids() {
    // Index, time and random, each big-endian, the 16 bytes interleaved; the
    // random part is stored rotated a bit to the left.
    let ids = [
        UniqueId {
            index: 0xdead_beef,
            time: 0x0013_3700,
            random: 0x0bad_f00d_c0ff_ee42,
        },
        UniqueId {
            index: 1,
            time: 2,
            random: -3,
        },
    ];
    let rows: Vec<[u8; 16]> = ids
        .iter()
        .map(|u| {
            let mut b = [0; 16];
            b[..4].copy_from_slice(&u.index.to_be_bytes());
            b[4..8].copy_from_slice(&u.time.to_be_bytes());
            b[8..].copy_from_slice(&(u.random as u64).rotate_left(1).to_be_bytes());
            b
        })
        .collect();
    assert_eq!(values(0x1f, 2, &interleave(&rows)), ids.map(PropValue::UniqueId));
}

#[test]
fn fonts() {
    let mut d = string(b"rbxasset://fonts/families/GothamSSm.json");
    d.extend(700u16.to_le_bytes());
    d.push(1);
    d.extend(string(b"rbxasset://fonts/GothamSSm-Bold.otf"));
    d.extend(string(b"rbxasset://fonts/families/Arial.json"));
    d.extend(400u16.to_le_bytes());
    d.push(0);
    d.extend(string(b""));
    let v = values(0x20, 2, &d);
    assert_eq!(
        v,
        [
            PropValue::Font(Font {
                family: "rbxasset://fonts/families/GothamSSm.json".into(),
                weight: 700,
                style: 1,
                cached_face_id: "rbxasset://fonts/GothamSSm-Bold.otf".into(),
            }),
            PropValue::Font(Font {
                family: "rbxasset://fonts/families/Arial.json".into(),
                weight: 400,
                style: 0,
                cached_face_id: String::new(),
            }),
        ]
    );
}

#[test]
fn security_capabilities() {
    let v = values(0x21, 3, &longs(&[0, 0x3f, -1]));
    assert_eq!(v, [0, 0x3f, u64::MAX].map(PropValue::SecurityCapabilities));
}

#[test]
fn content() {
    // Sources none, URI, object, URI, object (one outside the file).
    let mut d = ints(&[0, 1, 2, 1, 2]);
    d.extend(2u32.to_le_bytes());
    d.extend(string(b"rbxassetid://1"));
    d.extend(string(b"rbxassetid://2"));
    d.extend(2u32.to_le_bytes());
    d.extend(refs(&[3, 77]));
    d.extend(1u32.to_le_bytes());
    d.extend(refs(&[5]));
    let v = values(0x22, 5, &d);
    assert_eq!(
        v,
        [
            PropValue::Content(Content::None),
            PropValue::Content(Content::Uri("rbxassetid://1".into())),
            PropValue::Content(Content::Object(Some(3))),
            PropValue::Content(Content::Uri("rbxassetid://2".into())),
            PropValue::Content(Content::Object(None)),
        ]
    );
    // More values that use URIs than URIs.
    let mut d = ints(&[1, 1]);
    d.extend(1u32.to_le_bytes());
    d.extend(string(b"x"));
    d.extend([0; 8]);
    let referents: Vec<i32> = (0..2).collect();
    let e = read(&file(&[inst(0, "Thing", &referents), prop(0, "P", 0x22, &d)])).unwrap_err();
    assert!(e.contains("fewer URIs"), "{e}");
}

#[test]
fn unknown_types_are_skipped_with_a_note() {
    let f = file(&[
        inst(0, "Part", &[0, 1]),
        prop(0, "Mystery", 0x11, b"anything at all"),
        prop(0, "Later", 0x77, b""),
        // A PROP chunk without a type, which Roblox ignores.
        chunk(b"PROP", &[&0u32.to_le_bytes()[..], &string(b"Empty")].concat()),
        prop(0, "Anchored", 0x02, &[1, 0]),
    ]);
    let m = read(&f).unwrap();
    assert_eq!(m.notes.len(), 2);
    assert!(
        m.notes[0].contains("Part.Mystery") && m.notes[0].contains("0x11"),
        "{:?}",
        m.notes
    );
    assert_eq!(m.items[0].properties, [("Anchored".to_string(), PropValue::Bool(true))]);
    // Notes stop piling up.
    let mut chunks = vec![inst(0, "Part", &[0])];
    chunks.extend((0..200).map(|i| prop(0, &format!("P{i}"), 0x99, b"")));
    let m = read(&file(&chunks)).unwrap();
    assert_eq!(m.notes.len(), 65);
}

#[test]
fn every_type_in_one_lz4_file() {
    let f = rich(true);
    let m = read(&f).unwrap();
    assert!(m.notes.is_empty(), "{:?}", m.notes);
    assert_eq!(m, read(&rich(false)).unwrap());
    let part = &m.items[1];
    assert_eq!(part.properties.len(), 32);
    assert_eq!(part.get("Anchored"), Some(&PropValue::Bool(true)));
    assert_eq!(part.get("Peer"), Some(&PropValue::Ref(Some(0))));
    assert_eq!(m.items[0].get("Peer"), Some(&PropValue::Ref(Some(1))));
    assert_eq!(m.items[0].children, [1, 2]);
}

/// A model with a property of every type, for the tests of broken files.
fn rich(lz4: bool) -> Vec<u8> {
    let c = |name: &[u8; 4], data: Vec<u8>| {
        if lz4 {
            lz4_chunk(name, &data)
        } else {
            chunk(name, &data)
        }
    };
    let mut chunks = vec![
        c(b"META", meta(&[("ExplicitAutoJoints", "true")])[16..].to_vec()),
        c(b"SSTR", sstr(&[b"shared", b"other"])[16..].to_vec()),
        c(b"INST", inst_data(0, "Folder", &[0], false)),
        c(b"INST", inst_data(1, "Part", &[1, 2], false)),
    ];
    let mut content = ints(&[1, 2]);
    content.extend(1u32.to_le_bytes());
    content.extend(string(b"rbxassetid://5"));
    content.extend(1u32.to_le_bytes());
    content.extend(refs(&[0]));
    content.extend(0u32.to_le_bytes());
    let props: Vec<(&str, u8, Vec<u8>)> = vec![
        ("Name", 0x01, [string(b"A"), string(b"B")].concat()),
        ("Anchored", 0x02, vec![1, 0]),
        ("Int", 0x03, ints(&[5, -5])),
        ("Float", 0x04, floats(&[0.5, -2.0])),
        ("Double", 0x05, [1.5f64.to_le_bytes(), 2.5f64.to_le_bytes()].concat()),
        ("UDim", 0x06, h("7f 80 00 80 00 00 00 00 00 00 00 00 00 00 04 08")),
        (
            "UDim2",
            0x07,
            [
                floats(&[0.5, 1.0]),
                floats(&[0.0, 0.25]),
                ints(&[10, -10]),
                ints(&[3, 4]),
            ]
            .concat(),
        ),
        ("Ray", 0x08, vec![0; 48]),
        ("Faces", 0x09, vec![1, 63]),
        ("Axes", 0x0a, vec![1, 7]),
        ("BrickColor", 0x0b, uints(&[194, 1004])),
        (
            "Color",
            0x0c,
            h("7f 00 7e 00 7e 00 00 00 7b 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00"),
        ),
        ("V2", 0x0d, h("85 86 93 91 33 19 35 9a 86 85 91 93 19 33 9a 35")),
        (
            "V3",
            0x0e,
            h("7F 7F 00 00 00 00 00 01 80 80 00 00 00 00 00 01 80 80 80 80 00 00 00 01"),
        ),
        (
            "CFrame",
            0x10,
            [
                vec![0x02, 0x0a],
                floats(&[1.0, 2.0]),
                floats(&[3.0, 4.0]),
                floats(&[5.0, 6.0]),
            ]
            .concat(),
        ),
        ("Material", 0x12, uints(&[256, 512])),
        ("Peer", 0x13, refs(&[0, 2])),
        (
            "V3i16",
            0x14,
            vec![1, 0, 2, 0, 3, 0, 0xff, 0xff, 0xfe, 0xff, 0xfd, 0xff],
        ),
        ("NS", 0x15, h("01000000 00000000 0000803f 00000000 00000000")),
        (
            "CS",
            0x16,
            h("01000000 00000000 0000803f 00000000 00000000 00000000 00000000"),
        ),
        ("Range", 0x17, h("00 00 00 00 00 00 00 3f 00 00 00 3f 00 00 80 3f")),
        ("Rect", 0x18, vec![0; 32]),
        (
            "Physics",
            0x19,
            h("00 01 33 33 33 3f 9a 99 99 3e 00 00 00 3f 00 00 80 3f 00 00 80 3f"),
        ),
        ("C3u8", 0x1a, h("00 3f ff 00 ff 7f")),
        ("Long", 0x1b, longs(&[-1, 1 << 40])),
        ("Mesh", 0x1c, uints(&[0, 1])),
        ("Code", 0x1d, [string(b"\x1bLua"), string(b"")].concat()),
        (
            "Pivot",
            0x1e,
            h("10 0a 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 7f 00 00 00 00 00 00 00 02 01 00"),
        ),
        ("Id", 0x1f, vec![7; 32]),
        (
            "Font",
            0x20,
            [
                string(b"f"),
                vec![0x90, 1, 0],
                string(b""),
                string(b"g"),
                vec![0x2c, 1, 1],
                string(b""),
            ]
            .concat(),
        ),
        ("Caps", 0x21, longs(&[3, 0])),
        ("Image", 0x22, content),
    ];
    // The Parts get every property, and the Folder a Ref too.
    chunks.push(c(b"PROP", prop_data(0, "Peer", 0x13, &refs(&[1]))));
    for (name, ty, data) in props {
        chunks.push(c(b"PROP", prop_data(1, name, ty, &data)));
    }
    chunks.push(c(b"PRNT", prnt_data(&[(1, 0), (2, 0), (0, -1)])));
    file(&chunks)
}

// ---------------------------------------------------------------- broken binary files

fn err(f: &[u8]) -> String {
    match read(f) {
        Ok(m) => panic!("read a broken file: {m:?}"),
        Err(e) => e,
    }
}

#[test]
fn bad_headers() {
    assert!(err(b"").contains("no <roblox>"));
    assert!(err(b"<roblox!").contains("shorter than"));
    let mut f = file(&[]);
    f[9] = b'\r';
    assert!(err(&f).contains("signature"));
    let mut f = file(&[]);
    f[14] = 1;
    assert!(err(&f).contains("version 1"));
    let f = header();
    assert!(err(&f).contains("END"));
    // The header's counts aren't trusted.
    let mut f = file(&[inst(0, "Folder", &[0])]);
    f[16..24].fill(0xff);
    assert_eq!(read(&f).unwrap().items.len(), 1);
}

#[test]
fn bad_structure() {
    let e = err(&file(&[inst(0, "Folder", &[1, 1])]));
    assert!(e.contains("referent 1 is used twice"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), inst(0, "Part", &[2])]));
    assert!(e.contains("two INST chunks"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), strings(3, "Name", &["x"])]));
    assert!(e.contains("class ID 3"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), prnt(&[(1, 9)])]));
    assert!(e.contains("referent 9"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), prnt(&[(9, -1)])]));
    assert!(e.contains("referent 9"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), prnt(&[(1, -1), (1, -1)])]));
    assert!(e.contains("twice"), "{e}");
    // Parents in a loop, and one its own parent.
    let e = err(&file(&[
        inst(0, "Folder", &[1, 2, 3]),
        prnt(&[(1, -1), (2, 3), (3, 2)]),
    ]));
    assert!(e.contains("loop"), "{e}");
    let e = err(&file(&[inst(0, "Folder", &[1]), prnt(&[(1, 1)])]));
    assert!(e.contains("loop"), "{e}");
    let mut bad = prnt_data(&[(1, -1)]);
    bad[0] = 1;
    let e = err(&file(&[inst(0, "Folder", &[1]), chunk(b"PRNT", &bad)]));
    assert!(e.contains("version 1"), "{e}");
}

#[test]
fn bad_values() {
    let bad = |ty: u8, n: usize, data: &[u8]| {
        let referents: Vec<i32> = (0..n as i32).collect();
        err(&file(&[inst(0, "Thing", &referents), prop(0, "P", ty, data)]))
    };
    let e = bad(0x10, 1, &[0x04; 13]);
    assert!(
        e.contains("0x04 isn't a CFrame rotation ID") && e.contains("Thing.P (CFrame)"),
        "{e}"
    );
    assert!(bad(0x19, 1, &[4]).contains("flag byte"));
    assert!(bad(0x1e, 1, &[0x11; 30]).contains("not 0x10"));
    assert!(bad(0x1e, 1, &h("10 02 00000000 00000000 00000000 03 01")).contains("not 0x02"));
    assert!(bad(0x22, 1, &[&ints(&[3])[..], &[0; 12]].concat()).contains("Content source"));
    // Too few bytes for the values.
    assert!(bad(0x03, 2, &[0; 7]).contains("ends early"));
    assert!(bad(0x02, 3, &[1, 1]).contains("cut short"));
    assert!(bad(0x01, 1, &[5, 0, 0, 0, b'a']).contains("ends early"));
}

#[test]
fn forged_counts_and_lengths_fail_fast() {
    // Each claims far more than the chunk holds: none may allocate for it.
    let mut d = 0u32.to_le_bytes().to_vec();
    d.extend(string(b"Folder"));
    d.push(0);
    d.extend(u32::MAX.to_le_bytes());
    assert!(err(&file(&[chunk(b"INST", &d)])).contains("claims 4294967295 entries"));
    let e = err(&file(&[chunk(b"META", &u32::MAX.to_le_bytes())]));
    assert!(e.contains("claims"), "{e}");
    let e = err(&file(&[chunk(b"SSTR", &[0, 0, 0, 0, 0xff, 0xff, 0xff, 0x7f])]));
    assert!(e.contains("claims"), "{e}");
    let e = err(&file(&[chunk(b"PRNT", &[0, 0xff, 0xff, 0xff, 0xff])]));
    assert!(e.contains("claims"), "{e}");
    let mut d = 0u32.to_le_bytes().to_vec();
    d.extend(u32::MAX.to_le_bytes());
    d.extend(b"Folder");
    assert!(err(&file(&[chunk(b"INST", &d)])).contains("ends early"));
    // A keypoint count, and a string's length.
    let e = err(&file(&[
        inst(0, "Thing", &[0]),
        prop(0, "P", 0x15, &u32::MAX.to_le_bytes()),
    ]));
    assert!(e.contains("claims"), "{e}");
    let e = err(&file(&[
        inst(0, "Thing", &[0]),
        prop(0, "P", 0x01, &[0xff, 0xff, 0xff, 0xff, 0]),
    ]));
    assert!(e.contains("ends early"), "{e}");
    // A chunk longer than the file.
    let mut f = file(&[inst(0, "Folder", &[0])]);
    f[32 + 8..32 + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(err(&f).contains("cut short inside its INST chunk"));
    let mut f = file(&[lz4_chunk(b"INST", &inst_data(0, "Folder", &[0], false))]);
    f[32 + 4..32 + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(err(&f).contains("cut short inside its INST chunk"));
}

#[test]
fn decompression_has_limits() {
    // A chunk that claims 4 GiB.
    let mut f = file(&[lz4_chunk(b"INST", &inst_data(0, "Folder", &[0], false))]);
    f[32 + 8..32 + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    let e = err(&f);
    assert!(e.contains("too big") && e.contains("128 MiB"), "{e}");
    // One that decompresses to more or less than it claims.
    let data = inst_data(0, "Folder", &[0], false);
    for size in [data.len() - 1, data.len() + 1] {
        let mut f = file(&[lz4_chunk(b"INST", &data)]);
        f[32 + 8..32 + 12].copy_from_slice(&(size as u32).to_le_bytes());
        assert!(err(&f).contains("decompress"), "{}", err(&f));
    }
    // A megabyte of zeros packs into a few kilobytes; small limits refuse it.
    let mut zeros = 0u32.to_le_bytes().to_vec();
    zeros.extend(string(&vec![0; 1 << 20]));
    let f = file(&[lz4_chunk(b"META", &zeros)]);
    assert!(f.len() < 8000);
    let small = Limits {
        data: 1 << 19,
        ..Limits::default()
    };
    assert!(
        read_with(&f, small)
            .unwrap_err()
            .contains("chunks decompress to more than")
    );
    // The tree's limit: many items, then many properties with a long name.
    let referents: Vec<i32> = (0..10_000).collect();
    let items = file(&[inst(0, "Part", &referents)]);
    let name = "N".repeat(1000);
    let f = file(&[inst(0, "Part", &referents), prop(0, &name, 0x02, &[1; 10_000])]);
    assert!(read(&f).is_ok());
    let small = Limits {
        tree: 100_000,
        ..Limits::default()
    };
    assert!(
        read_with(&f, small)
            .unwrap_err()
            .contains("tree would take more than 100000 bytes")
    );
    let small = Limits {
        tree: 5_000_000,
        ..Limits::default()
    };
    assert!(read_with(&items, small).is_ok());
    assert!(read_with(&f, small).unwrap_err().contains("tree would take"));
}

#[test]
fn truncations_fail_cleanly() {
    for lz4 in [false, true] {
        let f = rich(lz4);
        assert!(read(&f).is_ok());
        for n in 0..f.len() {
            assert!(read(&f[..n]).is_err(), "{n} of {} bytes read", f.len());
        }
    }
}

/// A small fast generator for the random tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn bit_flips_never_panic() {
    for lz4 in [false, true] {
        let f = rich(lz4);
        let mut ok = 0;
        for i in 0..f.len() * 8 {
            let mut g = f.clone();
            g[i / 8] ^= 1 << (i % 8);
            ok += usize::from(read(&g).is_ok());
        }
        // Flips in values still read; flips in the structure don't.
        assert!(ok > 0 && ok < f.len() * 8, "{ok}");
    }
}

#[test]
fn random_damage_never_panics() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let files = [rich(false), rich(true)];
    for round in 0..4000 {
        let mut g = files[round % 2].clone();
        for _ in 0..1 + rng.below(8) {
            let i = rng.below(g.len());
            match rng.below(4) {
                0 => g[i] = rng.next() as u8,
                1 => g[i] = [0, 0xff, 0x7f, 0x80][rng.below(4)],
                2 => {
                    g.remove(i);
                }
                _ => g.insert(i, rng.next() as u8),
            }
        }
        let _ = read(&g);
    }
    // Random bytes after a good header.
    for _ in 0..2000 {
        let mut g = header();
        g.extend((0..rng.below(300)).map(|_| rng.next() as u8));
        let _ = read(&g);
    }
}

#[test]
fn deep_trees_are_fine() {
    // A chain of 100,000 Folders, each the parent of the next.
    let n = 100_000;
    let referents: Vec<i32> = (0..n).collect();
    let pairs: Vec<(i32, i32)> = (0..n).map(|i| (i, i - 1)).collect();
    let m = read(&file(&[inst(0, "Folder", &referents), prnt(&pairs)])).unwrap();
    assert_eq!(m.roots, [0]);
    assert_eq!(m.items[99_998].children, [99_999]);
}

// ---------------------------------------------------------------- ZSTD

/// The data the reference frames hold: letters and spaces from an LCG.
fn letters(n: usize) -> Vec<u8> {
    let mut x: u64 = 1;
    (0..n)
        .map(|_| {
            x = (x * 1_103_515_245 + 12_345) % (1 << 31);
            match (x >> 16) % 27 {
                26 => b' ',
                k => b'a' + k as u8,
            }
        })
        .collect()
}

fn parts(n: usize) -> Vec<u8> {
    (0..n)
        .flat_map(|i| format!("Part{} Anchored={} Size={}\n", i % 13, i % 3, i * 7 % 100).into_bytes())
        .collect()
}

// Frames made by the reference implementation (python-zstandard 0.25, from
// the data above), with Huffman-coded literals, FSE-coded sequences and
// XXH64 checksums.
/// 1200 letters and spaces (`letters`), at level 19 with a checksum.
const ZSTD_LETTERS: &str = "\
    28b52ffd64b0036517000a4b9c0b11b03db04936b049a596c0b6a42449985c05b500b400b300cdb5a3246c6eba084b948be29aa0\
    81057e18c6c9df5af569ed299ce7431050958c72c33c4c834a111f7aa5fd3849d46af86592cba720ced1884e8567822a618e95d2\
    53fa52ff8d01a5a704d29f9d214b0949d430bf3f9e0be5449a0b0ed984962cc3659a42fe2d6b769f7acd50656b8961f2d76ba214\
    8d732c1c906d0c01a151afa9b1e393f44a9d41a2e4ec4895518ab7d929fa52980851289bce822c342fa83d5cc5671c810a873430\
    03974b539e140ddac7f8032302b87e4156c9d9a256798244c65a3e75ba0da24b6b15ae3c05960b3d548c66c5ca347d5b2ae54758\
    b9ab32616113cfeb98db46684e240ad43a7bf4cfc1c0409c112112e99a98aed55537797490565732e355c8b5df4d5a86a5a63fa7\
    58407419f11958802e43a0ad525c29b924bb87f810e78edafcd02db1660c8b84757ca26c69d501411a43033407b93a4113928bc5\
    510ae664e12a731289cf016c27e41b2838d0e210317ac69b4ee8dbd22ca282ab0ac86625ad3db411a0c271a23fb2367b19c112d7\
    e501ef1d25f694e77669bf3e04a83ff1794712d069e92c72c6d5969a8c7789324f0fa5fd41ad76c24fef25c3f4c7a01dc22d707a\
    a8633ea3f6a925139d6aa612db087e24f03eba8c7bc61538a96bd4eccb00cc65ab575b3a4ae232c4ccd8b55be5c1e1c44dbe5589\
    aee53c65623cc86487d893498b1cd542b9e08d9625f044c948fd37f18e7d5372213011acab1ea1ce9ea150edc5125ac38c5b46bc\
    651e079d2303b3313cca772e64e6e52f4521a85f5f5bf80897a0a9c7320e6c294abe823131816bc78ad15c9d10df13502a5aa889\
    409a1c5f08787400c8b074ed14cbcf619e1ba43831612876b99ea941c341a0911cfedd9807f7779ea206e18bd2a44ca4fb3af0e4\
    f57c91a6db2b4f776e73d03cabfcce7cee231c1d3bcc0bca98fd58d089db72aaf464f107aaa1ee1cd8a213ed8cc66f1cd182b7e0\
    75732cea331ee03115ef09048bd541de79269a1140f6501054afa96b0100f18edc0f";
/// 300 lines (`parts`), at level 3 with a checksum and no content size.
const ZSTD_PARTS: &str = "\
    28b52ffd04187d1800f6e84e1980b7a0391c69e586f823b9973cd88a26222244ba740c48ea045100450042006d3bcdeb3368572e\
    c6aa0eba449a9ebc705a7b525ae371ec95ec2b5e1dda86ac5c25dd7961471ff378eda5388a7a63ebb0484af75ccadcda232c1288\
    020f8a848200030f038ac5018180018081088b0422b3b3b3343b3b712ab3775ab90cd675d83552f5e8674c28d2ba34bee3d7b1ad\
    c8aad0351cd2ca54defd69a35c8e65a35a4437cf889595eb31d851bacfb8756a23922a730947b58d557a3895cbf6d9f1d16a17a9\
    f4502a97778ede476b5da4ce42699f7d70f436566be27416b67d56c78357db58a58953b96cffeef878b58b547a2895cbcb859d0d\
    0519bd8dd69a489dd9d907afb7b15a13a7b2b07d763c78b58b557a3895cbf6393a3e5aed22951e4a7b79e7e87db4d644ea2c94f6\
    791fbcdec66a4d9ccec2f6d9f1e0058275a81279fbfe77020f450229c907126890438480e21022c470abdc3a99847c5924a69ab5\
    af2280153ba857750a8024cca65836aac194ee891c53a4eb3f69d40233f91310cf98bd4d6756b88a37815779634eb9629ac90a56\
    f326b0fd531eb331c64cb29809cce49e88f707d998761c0ff25f3a378b797b31de66dec493ca648bc9c6c4bc5b314f4a9ec53b59\
    4c631a33d925b5a47b681236b6c863fe318b848262360f5938558c427b89c65ca012ea29399d9bc0aa7f6c4c6e24bd6bec9d4a52\
    8d42f339c48f3919a995ce66972414ea9c7ca24f27991543184a3a6c80783a9cdd863a93ef235b4650a9619df866f27d64bcac4e\
    844a6246a4e944b47a38404a070d279a0e075953e72696c7ea35bfccae01682f40e75f1d6178f414eadc1c0fd04a4a31d08234e3\
    105312efad0e3c58eb9d9657afd07ef462bd427db8b0e42c96ea05aaf46cea11d7555929a9b4ed98699915e1cfbda53a3b5685d2\
    949a3468d15ecb5a6f7d947a5cff6f4b4dba123397240f2d7c333e5cdeee3e9e75862401a76a498d6990da195da72f55a14fb518\
    a4a4b32ec98c348a6144b944d506a852ef577fa1cdd56fe8ab15c01e7fca6bf38fb69f47f2d7a033403320c88f6612cd4001909f\
    4b4f4c2828e38d6e5a46bf156adb81c3";
/// An INST chunk of 50 TextButtons, at level 3 with a checksum.
const ZSTD_INST: &str = "28b52ffd24dffd0000b0000000000a00000054657874427574746f6e0032000202008148b450800905e2773f";
/// Their names, Button0 to Button49, likewise.
const ZSTD_PROP: &str = "\
    28b52ffd645b011d0400e2840f14a0356d48ff39119c56c4ddddddbdf9bf4536a2a5ab8967451f5b4d3c2bfad46ae259d1875613\
    cf8a3e2f5613cf8a3e01412903c336212400e4b03dd8763120e0d746cc1966c49c6146cc35b8991173861931679811730ddacc88\
    39c38c9833cc88b906373362ce3023e60c33a2cdbdc10537dbb26fb32dfb36db5be02d8d0189fa42e5";
/// A chunk of ZSTD data.
fn zstd_chunk(name: &[u8; 4], frame: &[u8], size: usize) -> Vec<u8> {
    [
        &name[..],
        &(frame.len() as u32).to_le_bytes(),
        &(size as u32).to_le_bytes(),
        &[0; 4],
        frame,
    ]
    .concat()
}

#[test]
fn zstd_reference_frames() {
    let (want, frame) = (letters(1200), h(ZSTD_LETTERS));
    assert_eq!(zstd_decompress(&frame, want.len()).unwrap(), want);
    assert!(
        zstd_decompress(&frame, want.len() - 1)
            .unwrap_err()
            .contains("more than")
    );
    let (want, frame) = (parts(300), h(ZSTD_PARTS));
    assert_eq!(zstd_decompress(&frame, want.len()).unwrap(), want);
    assert!(
        zstd_decompress(&frame, want.len() - 1)
            .unwrap_err()
            .contains("more than")
    );
    // Two frames with a skippable one between them.
    let skip = [&0x184d_2a5au32.to_le_bytes()[..], &3u32.to_le_bytes(), b"abc"].concat();
    let both = [h(ZSTD_LETTERS), skip, h(ZSTD_PARTS)].concat();
    assert_eq!(
        zstd_decompress(&both, 1 << 20).unwrap(),
        [letters(1200), parts(300)].concat()
    );
    // A damaged checksum.
    let mut bad = h(ZSTD_LETTERS);
    *bad.last_mut().unwrap() ^= 1;
    assert!(zstd_decompress(&bad, 1200).unwrap_err().contains("checksum"));
}

#[test]
fn zstd_chunks_in_a_file() {
    let f = file(&[
        zstd_chunk(b"INST", &h(ZSTD_INST), 223),
        zstd_chunk(b"PROP", &h(ZSTD_PROP), 603),
    ]);
    let m = read(&f).unwrap();
    assert_eq!(m.items.len(), 50);
    assert!(m.items.iter().all(|i| i.class == "TextButton"));
    assert_eq!(m.items[49].name(), "Button49");
    assert_eq!(m.roots.len(), 50);
}

#[test]
fn zstd_blocks_by_hand() {
    let frame = |flags: u8, rest: &[u8]| [&[0x28, 0xb5, 0x2f, 0xfd, flags][..], rest].concat();
    // "ab" in a raw block, then five c's in an RLE block, the last.
    let raw = (2u32 << 3).to_le_bytes();
    let rle = ((5u32 << 3) | 2 | 1).to_le_bytes();
    let blocks = [&raw[..3], b"ab", &rle[..3], b"c"].concat();
    // One segment, with a 1-byte content size.
    let f = frame(0x20, &[&[7], &blocks[..]].concat());
    assert_eq!(zstd_decompress(&f, 7).unwrap(), b"abccccc");
    // A window descriptor, and no content size.
    let f = frame(0x00, &[&[0], &blocks[..]].concat());
    assert_eq!(zstd_decompress(&f, 7).unwrap(), b"abccccc");
    assert!(zstd_decompress(&f, 6).unwrap_err().contains("more than"));
    // A 2-byte content size (less 256): 300 c's. Then one that is wrong.
    let rle300 = ((300u32 << 3) | 2 | 1).to_le_bytes();
    let f = frame(0x60, &[&[44, 0], &rle300[..3], b"c"].concat());
    assert_eq!(zstd_decompress(&f, 300).unwrap(), vec![b'c'; 300]);
    assert!(zstd_decompress(&f, 299).unwrap_err().contains("more than"));
    let f = frame(0x20, &[&[8], &blocks[..]].concat());
    assert!(zstd_decompress(&f, 8).unwrap_err().contains("size its frame says"));
    // A reserved bit, a dictionary, a reserved block type, a block too big, a wrong magic number.
    assert!(
        zstd_decompress(&frame(0x28, &[&[7], &blocks[..]].concat()), 7)
            .unwrap_err()
            .contains("reserved bit")
    );
    assert!(
        zstd_decompress(&frame(0x21, &[&[1, 7], &blocks[..]].concat()), 7)
            .unwrap_err()
            .contains("dictionary")
    );
    let reserved = ((1u32 << 3) | 6 | 1).to_le_bytes();
    let f = frame(0x00, &[&[0], &reserved[..3], b"x"].concat());
    assert!(zstd_decompress(&f, 9).unwrap_err().contains("reserved type"));
    let huge = ((((128u32 << 10) + 1) << 3) | 1).to_le_bytes();
    let f = frame(0x00, &[&[0], &huge[..3]].concat());
    assert!(zstd_decompress(&f, 1 << 20).unwrap_err().contains("too big"));
    assert!(
        zstd_decompress(b"\x28\xb5\x2f\xfe\x00", 9)
            .unwrap_err()
            .contains("magic")
    );
    // As a chunk in a file.
    let data = inst_data(0, "Folder", &[0], false);
    let mut body = vec![data.len() as u8];
    body.extend(&(((data.len() as u32) << 3) | 1).to_le_bytes()[..3]);
    body.extend(&data);
    let m = read(&file(&[zstd_chunk(b"INST", &frame(0x20, &body), data.len())])).unwrap();
    assert_eq!(m.items[0].class, "Folder");
}

#[test]
fn xxh64_vectors() {
    // From the reference implementation (the xxhash package).
    for (data, want) in [
        (b"".to_vec(), 0xef46_db37_51d8_e999),
        (b"a".to_vec(), 0xd24e_c4f1_a98c_6e5b),
        (b"abc".to_vec(), 0x44bc_2cf5_ad77_0999),
        (letters(31), 0xfa14_5e6d_7842_6f8b),
        (letters(32), 0x9057_fb1c_ecfd_58da),
        (letters(100), 0x858a_16e5_7a33_18f4),
        (letters(1000), 0xb068_bcd1_8d6e_9a28),
    ] {
        assert_eq!(xxh64(&data), want, "{} bytes", data.len());
    }
}

#[test]
fn zstd_damage_never_panics() {
    let (want, frame) = (letters(1200), h(ZSTD_LETTERS));
    for n in 1..frame.len() {
        assert!(zstd_decompress(&frame[..n], 1200).is_err(), "{n} bytes");
    }
    // With a checksum, damage that still decodes gives the same data.
    for i in 0..frame.len() * 8 {
        let mut g = frame.clone();
        g[i / 8] ^= 1 << (i % 8);
        if let Ok(out) = zstd_decompress(&g, 1200) {
            assert_eq!(out, want, "bit {i}");
        }
    }
    // Without one, damage can go unnoticed, but never panics.
    let mut plain = h(ZSTD_PARTS);
    plain[4] &= !4;
    plain.truncate(plain.len() - 4);
    assert_eq!(zstd_decompress(&plain, 1 << 20).unwrap(), parts(300));
    let mut rng = Rng(7);
    for _ in 0..5000 {
        let mut g = plain.clone();
        for _ in 0..1 + rng.below(4) {
            let i = rng.below(g.len());
            g[i] ^= 1 << rng.below(8);
        }
        let _ = zstd_decompress(&g, 1 << 16);
    }
}

// ---------------------------------------------------------------- XML

const MODEL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Made by hand, as Studio would. -->
<!DOCTYPE roblox [ <!ENTITY x "not expanded"> ]>
<roblox xmlns:xmime="http://www.w3.org/2005/05/xmlmime" version="4">
	<Meta name="ExplicitAutoJoints">true</Meta>
	<External>null</External>
	<External>nil</External>
	<Item class="ScreenGui" referent="RBX0">
		<Properties>
			<string name="Name">Rayfield</string>
			<bool name="Enabled">TRUE</bool>
			<token name="ZIndexBehavior">1</token>
			<Ref name="Self">RBX0</Ref>
		</Properties>
		<Item class="Frame" referent="RBX1">
			<Properties>
				<string name="Name">Main &amp; &lt;Top&gt; &#x41;&#66; &quot;q&quot; &apos;a&apos; &bogus; &amp x</string>
				<UDim2 name="Size">
					<XS>0.5</XS>
					<XO>-30</XO>
					<YS>0</YS>
					<YO>475</YO>
				</UDim2>
				<UDim name="Padding"><S>0.15625</S><O>1337</O></UDim>
				<Color3 name="BackgroundColor3"><R>0.25</R><G>1</G><B>INF</B></Color3>
				<Color3 name="Packed">4284497952</Color3>
				<Color3uint8 name="BorderColor3">4284497952</Color3uint8>
				<float name="BackgroundTransparency">0.15625</float>
				<double name="D">-INF</double>
				<int name="BorderSizePixel">-7</int>
				<int64 name="Big">-9223372036854775808</int64>
				<BrickColor name="BrickColor">194</BrickColor>
				<Ref name="NextSelectionUp">RBX2</Ref>
				<Ref name="NextSelectionDown">null</Ref>
				<Ref name="Dangling">RBXnothere</Ref>
				<Vector2 name="AnchorPoint"><X>0.5</X><Y>-1e3</Y></Vector2>
				<Vector3 name="V3"><X>1</X><Y>2</Y><Z>3</Z></Vector3>
				<Vector2int16 name="V2i"><X>-32768</X><Y>32767</Y></Vector2int16>
				<Vector3int16 name="V3i"><X>1337</X><Y>0</Y><Z>-1337</Z></Vector3int16>
				<CoordinateFrame name="CFrame">
					<X>1</X><Y>2</Y><Z>3</Z>
					<R00>0</R00><R01>-1</R01><R02>0</R02>
					<R10>1</R10><R11>0</R11><R12>0</R12>
					<R20>0</R20><R21>0</R21><R22>1</R22>
				</CoordinateFrame>
				<OptionalCoordinateFrame name="Pivot"><CFrame><X>0</X><Y>0</Y><Z>0</Z><R00>1</R00><R01>0</R01><R02>0</R02><R10>0</R10><R11>1</R11><R12>0</R12><R20>0</R20><R21>0</R21><R22>1</R22></CFrame></OptionalCoordinateFrame>
				<OptionalCoordinateFrame name="NoPivot"></OptionalCoordinateFrame>
				<Ray name="Ray"><origin><X>1</X><Y>2</Y><Z>3</Z></origin><direction><X>-1</X><Y>-2</Y><Z>-3</Z></direction></Ray>
				<Rect2D name="SliceCenter"><min><X>1</X><Y>2</Y></min><max><X>3</X><Y>4</Y></max></Rect2D>
				<Faces name="Faces"><faces>42</faces></Faces>
				<Axes name="Axes"><axes>5</axes></Axes>
				<NumberRange name="Range">0.15625 1337 </NumberRange>
				<NumberSequence name="Transparency">0 6 3 1 4 2 </NumberSequence>
				<ColorSequence name="Gradient">0 0.376471 0.25098 0.12549 0 1 0.0196078 0.0392157 0.0588235 0 </ColorSequence>
				<PhysicalProperties name="Default"><CustomPhysics>false</CustomPhysics></PhysicalProperties>
				<PhysicalProperties name="Custom">
					<CustomPhysics>true</CustomPhysics>
					<Density>1</Density>
					<Friction>2</Friction>
					<Elasticity>1</Elasticity>
					<FrictionWeight>0.15625</FrictionWeight>
					<ElasticityWeight>1.25</ElasticityWeight>
				</PhysicalProperties>
				<BinaryString name="Tags">Um9qbyBpcyBjb29sIQ==</BinaryString>
				<BinaryString name="AttributesSerialize"></BinaryString>
				<ContentId name="Image"><url>rbxassetid://123</url></ContentId>
				<ContentId name="NoImage"><null></null></ContentId>
				<Content name="Content"><uri>rbxassetid://456</uri></Content>
				<Content name="OldContent"><url>rbxasset://textures/face.png</url></Content>
				<Content name="NoContent"><null></null></Content>
				<Content name="Hash"><hash>abc</hash></Content>
				<Content name="Object"><Ref>RBX2</Ref></Content>
				<Font name="FontFace">
					<Family><url>rbxasset://fonts/families/Arial.json</url></Family>
					<Weight>700</Weight>
					<Style>Italic</Style>
					<CachedFaceId><url>rbxasset://fonts/arialbd.ttf</url></CachedFaceId>
				</Font>
				<Font name="Empty"/>
				<UniqueId name="UniqueId">0badf00dc0ffee4200133700deadbeef</UniqueId>
				<SecurityCapabilities name="Capabilities">18446744073709551615</SecurityCapabilities>
				<SharedString name="MeshData">aGFzaA==</SharedString>
				<NetAssetRef name="Asset">aGFzaA==</NetAssetRef>
				<Mystery name="Unknown"><a>1</a></Mystery>
				<int name="Broken">twelve</int>
				<int>no name</int>
			</Properties>
			<Item class="Script" referent="RBX2">
				<Properties>
					<string name="Name">Loader</string>
					<ProtectedString name="Source"><![CDATA[local t = {a = 1} -- a < b and c > d
print("]]]]><![CDATA[>", t["a"])	-- tabs kept
]]></ProtectedString>
				</Properties>
			</Item>
		</Item>
		<Item class="Folder" referent="RBX3"><Properties><string name="Name">  spaced  </string></Properties></Item>
	</Item>
	<SharedStrings>
		<SharedString md5="aGFzaA==">bWVzaCBk
YXRh</SharedString>
	</SharedStrings>
</roblox>
"#;

#[test]
fn xml_tree() {
    let m = read(MODEL.as_bytes()).unwrap();
    assert_eq!(m.meta, vec![("ExplicitAutoJoints".to_string(), "true".to_string())]);
    let classes: Vec<_> = m
        .items
        .iter()
        .map(|i| (i.class.as_str(), i.referent.as_str()))
        .collect();
    assert_eq!(
        classes,
        [
            ("ScreenGui", "RBX0"),
            ("Frame", "RBX1"),
            ("Script", "RBX2"),
            ("Folder", "RBX3")
        ]
    );
    assert_eq!(m.roots, [0]);
    assert_eq!(m.items[0].children, [1, 3]);
    assert_eq!(m.items[1].children, [2]);
    assert_eq!(m.items[2].parent, Some(1));
    assert_eq!(m.items[0].name(), "Rayfield");
    assert_eq!(m.items[3].name(), "  spaced  ");
    assert_eq!(m.items[0].get("Enabled"), Some(&PropValue::Bool(true)));
    assert_eq!(m.items[0].get("ZIndexBehavior"), Some(&PropValue::Enum(1)));
    assert_eq!(m.items[0].get("Self"), Some(&PropValue::Ref(Some(0))));
}

#[test]
fn xml_text_entities_and_cdata() {
    let m = read(MODEL.as_bytes()).unwrap();
    assert_eq!(m.items[1].name(), r#"Main & <Top> AB "q" 'a' &bogus; &amp x"#);
    let source = m.items[2].get("Source").unwrap().as_str().unwrap();
    assert_eq!(
        source,
        "local t = {a = 1} -- a < b and c > d\nprint(\"]]>\", t[\"a\"])\t-- tabs kept\n"
    );
    // Line breaks become "\n", but a written &#13; stays.
    let m = read(
        b"<roblox><Item class='S'><Properties><string name='A'>a\r\nb\rc&#13;d</string>\
                   <ProtectedString name='B'><![CDATA[x\r\ny]]></ProtectedString></Properties></Item></roblox>",
    )
    .unwrap();
    assert_eq!(m.items[0].get("A").unwrap().as_str(), Some("a\nb\nc\rd"));
    assert_eq!(m.items[0].get("B").unwrap().as_str(), Some("x\ny"));
}

#[test]
fn xml_every_type() {
    let m = read(MODEL.as_bytes()).unwrap();
    let f = &m.items[1];
    let get = |n: &str| f.get(n).unwrap_or_else(|| panic!("no {n}")).clone();
    use PropValue as V;
    let udim2 = UDim2 {
        x: UDim {
            scale: 0.5,
            offset: -30,
        },
        y: UDim {
            scale: 0.0,
            offset: 475,
        },
    };
    assert_eq!(get("Size"), V::UDim2(udim2));
    assert_eq!(
        get("Padding"),
        V::UDim(UDim {
            scale: 0.15625,
            offset: 1337
        })
    );
    assert_eq!(
        get("BackgroundColor3"),
        V::Color3(Color3 {
            r: 0.25,
            g: 1.0,
            b: f32::INFINITY
        })
    );
    assert_eq!(
        get("Packed"),
        V::Color3(Color3 {
            r: 96.0 / 255.0,
            g: 64.0 / 255.0,
            b: 32.0 / 255.0
        })
    );
    assert_eq!(get("BorderColor3"), V::Color3uint8([96, 64, 32]));
    assert_eq!(get("BackgroundTransparency"), V::Float32(0.15625));
    assert_eq!(get("D"), V::Float64(f64::NEG_INFINITY));
    assert_eq!(get("BorderSizePixel"), V::Int32(-7));
    assert_eq!(get("Big"), V::Int64(i64::MIN));
    assert_eq!(get("BrickColor"), V::BrickColor(194));
    assert_eq!(get("NextSelectionUp"), V::Ref(Some(2)));
    assert_eq!(get("NextSelectionDown"), V::Ref(None));
    assert_eq!(get("Dangling"), V::Ref(None));
    assert_eq!(get("AnchorPoint"), V::Vector2(Vector2 { x: 0.5, y: -1000.0 }));
    assert_eq!(get("V3"), V::Vector3(Vector3 { x: 1.0, y: 2.0, z: 3.0 }));
    assert_eq!(get("V2i"), V::Vector2int16([-32768, 32767]));
    assert_eq!(get("V3i"), V::Vector3int16([1337, 0, -1337]));
    let cf = CFrame {
        position: Vector3 { x: 1.0, y: 2.0, z: 3.0 },
        rotation: rotation_from_id(0x0a).unwrap(),
    };
    assert_eq!(get("CFrame"), V::CFrame(cf));
    assert_eq!(get("Pivot"), V::OptionalCFrame(Some(CFrame::default())));
    assert_eq!(get("NoPivot"), V::OptionalCFrame(None));
    let ray = Ray {
        origin: Vector3 { x: 1.0, y: 2.0, z: 3.0 },
        direction: Vector3 {
            x: -1.0,
            y: -2.0,
            z: -3.0,
        },
    };
    assert_eq!(get("Ray"), V::Ray(ray));
    let rect = Rect {
        min: Vector2 { x: 1.0, y: 2.0 },
        max: Vector2 { x: 3.0, y: 4.0 },
    };
    assert_eq!(get("SliceCenter"), V::Rect(rect));
    assert_eq!(get("Faces"), V::Faces(42));
    assert_eq!(get("Axes"), V::Axes(5));
    assert_eq!(
        get("Range"),
        V::NumberRange(NumberRange {
            min: 0.15625,
            max: 1337.0
        })
    );
    let k = |time, value, envelope| NumberKeypoint { time, value, envelope };
    assert_eq!(
        get("Transparency"),
        V::NumberSequence(vec![k(0.0, 6.0, 3.0), k(1.0, 4.0, 2.0)])
    );
    let V::ColorSequence(g) = get("Gradient") else { panic!() };
    assert_eq!(g.len(), 2);
    assert!(near(g[0].color.r, 96.0 / 255.0) && near(g[1].color.b, 15.0 / 255.0) && g[1].time == 1.0);
    assert_eq!(get("Default"), V::PhysicalProperties(None));
    let physics = CustomPhysics {
        density: 1.0,
        friction: 2.0,
        elasticity: 1.0,
        friction_weight: 0.15625,
        elasticity_weight: 1.25,
        acoustic_absorption: 1.0,
    };
    assert_eq!(get("Custom"), V::PhysicalProperties(Some(physics)));
    assert_eq!(get("Tags"), V::String(b"Rojo is cool!".to_vec()));
    assert_eq!(get("AttributesSerialize"), V::String(vec![]));
    assert_eq!(get("Image"), V::String(b"rbxassetid://123".to_vec()));
    assert_eq!(get("NoImage"), V::String(vec![]));
    assert_eq!(get("Content"), V::Content(Content::Uri("rbxassetid://456".into())));
    assert_eq!(
        get("OldContent"),
        V::Content(Content::Uri("rbxasset://textures/face.png".into()))
    );
    assert_eq!(get("NoContent"), V::Content(Content::None));
    assert_eq!(get("Hash"), V::Content(Content::None));
    assert_eq!(get("Object"), V::Content(Content::Object(Some(2))));
    let font = Font {
        family: "rbxasset://fonts/families/Arial.json".into(),
        weight: 700,
        style: 1,
        cached_face_id: "rbxasset://fonts/arialbd.ttf".into(),
    };
    assert_eq!(get("FontFace"), V::Font(font));
    assert_eq!(get("Empty"), V::Font(Font::default()));
    let id = UniqueId {
        index: 0xdead_beef,
        time: 0x0013_3700,
        random: 0x0bad_f00d_c0ff_ee42,
    };
    assert_eq!(get("UniqueId"), V::UniqueId(id));
    assert_eq!(get("Capabilities"), V::SecurityCapabilities(u64::MAX));
    assert_eq!(get("MeshData"), V::SharedString(b"mesh data"[..].into()));
    assert_eq!(get("Asset"), get("MeshData"));
    // What couldn't be read was skipped, with notes.
    assert!(f.get("Unknown").is_none() && f.get("Broken").is_none());
    assert_eq!(m.notes.len(), 3, "{:?}", m.notes);
    assert!(m.notes[0].contains("Frame.Unknown (<Mystery>)") && m.notes[0].contains("isn't known"));
    assert!(m.notes[1].contains("Frame.Broken (<int>)") && m.notes[1].contains("\"twelve\""));
    assert!(m.notes[2].contains("without a name"));
}

#[test]
fn xml_tolerance() {
    // No declaration, unquoted and single-quoted attributes, a stray '<',
    // closing tags that match nothing, comments inside elements.
    let m = read(
        b"\xef\xbb\xbf  <roblox version=4><Item class=Part referent='a'><!-- c --><Properties>\
          <string name=\"Name\">1 < 2</string></bogus><bool name='Anchored'> false </bool>\
          </Properties></Item></roblox>",
    )
    .unwrap();
    assert_eq!(m.items[0].class, "Part");
    assert_eq!(m.items[0].name(), "1 < 2");
    assert_eq!(m.items[0].get("Anchored"), Some(&PropValue::Bool(false)));
    // A closing tag can close several open ones.
    let m = read(b"<roblox><Item class='A'><Properties><string name='Name'>x</Properties></Item></roblox>").unwrap();
    assert_eq!(m.items[0].name(), "x");
    // Items without a referent, a duplicated referent (Refs go to the first).
    let m = read(
        b"<roblox><Item class='A' referent='r'><Properties><Ref name='R'>r</Ref></Properties></Item>\
          <Item class='B' referent='r'/><Item class='C'/></roblox>",
    )
    .unwrap();
    assert_eq!(m.roots, [0, 1, 2]);
    assert_eq!(m.items[0].get("R"), Some(&PropValue::Ref(Some(0))));
    assert_eq!(m.notes.len(), 1);
    // Text outside elements, and empty Properties.
    let m = read(b"junk <roblox>more junk<Item class='A'><Properties/></Item></roblox> trailing").unwrap();
    assert_eq!(m.items.len(), 1);
}

#[test]
fn xml_errors() {
    for (xml, want) in [
        ("", "no <roblox>"),
        ("<html></html>", "no <roblox>"),
        ("<roblox><Item class='A'>", "ends inside <Item>"),
        ("<roblox><Item class='A'></Item>", "ends inside <roblox>"),
        ("<roblox><!-- open", "inside a comment"),
        (
            "<roblox><Item class='A'><Properties><string name='x'><![CDATA[abc",
            "inside a CDATA",
        ),
        ("<roblox><Item class='A' referent=\"abc", "inside the tag <Item"),
        ("<roblox><Item class='A'", "inside the tag <Item"),
        ("<roblox></Item", "inside a closing tag"),
        ("<?xml version='1.0'", "inside a processing instruction"),
        ("<!DOCTYPE roblox [ <!ENTITY a '>'> ", "inside a declaration"),
        ("<roblox><Item referent='a'/></roblox>", "has no class"),
    ] {
        let e = read(xml.as_bytes()).unwrap_err();
        assert!(e.contains(want), "{xml}: {e}");
    }
    // A shared string that is missing, or isn't base64.
    let m = read(
        b"<roblox><Item class='A'><Properties><SharedString name='S'>nope</SharedString></Properties></Item>\
          <SharedStrings><SharedString md5='bad'>!!!</SharedString></SharedStrings></roblox>",
    )
    .unwrap();
    assert!(m.items[0].properties.is_empty());
    assert_eq!(m.notes.len(), 2, "{:?}", m.notes);
}

#[test]
fn xml_damage_never_panics() {
    let x = MODEL.as_bytes();
    // Every cut before </roblox> leaves it open.
    let end = MODEL.find("</roblox>").unwrap() + "</roblox>".len();
    for n in 0..x.len() {
        let r = read_xml(&x[..n], Limits::default());
        assert_eq!(r.is_ok(), n >= end, "{n}: {r:?}");
    }
    let mut rng = Rng(42);
    for _ in 0..3000 {
        let mut g = x.to_vec();
        for _ in 0..1 + rng.below(6) {
            let i = rng.below(g.len());
            match rng.below(3) {
                0 => g[i] = b"<>&;/='\"[]!?-x \xc3\xff"[rng.below(17)],
                1 => {
                    g.remove(i);
                }
                _ => g.insert(i, rng.next() as u8),
            }
        }
        let _ = read(&g);
    }
}

#[test]
fn xml_deep_nesting_is_fine() {
    let n = 100_000;
    let mut x = "<roblox>".to_string();
    x.push_str(&"<Item class='Folder'>".repeat(n));
    x.push_str(&"</Item>".repeat(n));
    x.push_str("</roblox>");
    let m = read(x.as_bytes()).unwrap();
    assert_eq!(m.items.len(), n);
    assert_eq!(m.items[n - 1].parent, Some(n - 2));
    // Elements that aren't Items, nested as deep.
    let x = format!("<roblox>{}</roblox>", "<a>".repeat(n) + &"</a>".repeat(n));
    assert!(read(x.as_bytes()).unwrap().items.is_empty());
}

#[test]
fn xml_limits() {
    let x = MODEL.as_bytes();
    assert!(
        read_with(
            x,
            Limits {
                data: 100,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .contains("too big")
    );
    assert!(
        read_with(
            x,
            Limits {
                tree: 2000,
                ..Limits::default()
            }
        )
        .unwrap_err()
        .contains("tree would take")
    );
    // Many tiny elements cost far more than their bytes.
    let x = format!("<roblox>{}</roblox>", "<a/>".repeat(100_000));
    let e = read_with(
        x.as_bytes(),
        Limits {
            tree: 1 << 20,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert!(e.contains("tree would take"), "{e}");
}

// ---------------------------------------------------------------- both formats

/// A small UI, as Rayfield's model has them, in binary.
fn ui_binary() -> Vec<u8> {
    let mut font = string(b"rbxasset://fonts/families/GothamSSm.json");
    font.extend(500u16.to_le_bytes());
    font.push(0);
    font.extend(string(b""));
    let id = UniqueId {
        index: 7,
        time: 99,
        random: 0x1234_5678_9abc_def0,
    };
    let mut row = [0u8; 16];
    row[..4].copy_from_slice(&id.index.to_be_bytes());
    row[4..8].copy_from_slice(&id.time.to_be_bytes());
    row[8..].copy_from_slice(&(id.random as u64).rotate_left(1).to_be_bytes());
    file(&[
        inst(0, "ScreenGui", &[0]),
        inst(1, "Frame", &[1]),
        inst(2, "TextLabel", &[2]),
        strings(0, "Name", &["Rayfield"]),
        strings(1, "Name", &["Main"]),
        strings(2, "Name", &["Title"]),
        strings(2, "Text", &["Rayfield Interface Suite"]),
        prop(
            1,
            "Size",
            0x07,
            &[floats(&[0.5]), floats(&[0.0]), ints(&[-30]), ints(&[475])].concat(),
        ),
        prop(
            1,
            "BackgroundColor3",
            0x0c,
            &[floats(&[0.1]), floats(&[0.2]), floats(&[0.3])].concat(),
        ),
        prop(1, "Visible", 0x02, &[1]),
        prop(2, "FontFace", 0x20, &font),
        prop(2, "TextSize", 0x04, &floats(&[14.0])),
        prop(2, "LayoutOrder", 0x03, &ints(&[-2])),
        prop(2, "UniqueId", 0x1f, &interleave(&[row])),
        prop(
            2,
            "Pivot",
            0x10,
            &[vec![0x0a], floats(&[1.0]), floats(&[2.0]), floats(&[3.0])].concat(),
        ),
        prnt(&[(2, 1), (1, 0), (0, -1)]),
    ])
}

const UI_XML: &str = r#"<roblox version="4">
  <Item class="ScreenGui" referent="RBXa"><Properties><string name="Name">Rayfield</string></Properties>
    <Item class="Frame" referent="RBXb">
      <Properties>
        <string name="Name">Main</string>
        <UDim2 name="Size"><XS>0.5</XS><XO>-30</XO><YS>0</YS><YO>475</YO></UDim2>
        <Color3 name="BackgroundColor3"><R>0.1</R><G>0.2</G><B>0.3</B></Color3>
        <bool name="Visible">true</bool>
      </Properties>
      <Item class="TextLabel" referent="RBXc">
        <Properties>
          <string name="Name">Title</string>
          <string name="Text">Rayfield Interface Suite</string>
          <Font name="FontFace"><Family><url>rbxasset://fonts/families/GothamSSm.json</url></Family><Weight>500</Weight><Style>Normal</Style></Font>
          <float name="TextSize">14</float>
          <int name="LayoutOrder">-2</int>
          <UniqueId name="UniqueId">123456789abcdef00000006300000007</UniqueId>
          <CoordinateFrame name="Pivot"><X>1</X><Y>2</Y><Z>3</Z><R00>0</R00><R01>-1</R01><R02>0</R02><R10>1</R10><R11>0</R11><R12>0</R12><R20>0</R20><R21>0</R21><R22>1</R22></CoordinateFrame>
        </Properties>
      </Item>
    </Item>
  </Item>
</roblox>"#;

/// A file without its referents, which differ between the formats.
fn shape(mut m: RbxFile) -> RbxFile {
    for i in &mut m.items {
        i.referent.clear();
    }
    m
}

#[test]
fn both_formats_read_the_same() {
    let b = read_binary(&ui_binary(), Limits::default()).unwrap();
    let x = read_xml(UI_XML.as_bytes(), Limits::default()).unwrap();
    assert!(b.notes.is_empty() && x.notes.is_empty());
    assert_eq!(shape(b.clone()), shape(x));
    assert_eq!(
        b.items[2].get("Text").unwrap().as_str(),
        Some("Rayfield Interface Suite")
    );
    assert_eq!(b.items[2].parent, Some(1));
}
