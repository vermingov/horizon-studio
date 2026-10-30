// Textures and meshes (`texture.*`, `mesh.*`, `draw.mesh`, meshes on
// `model.attach`) on the headless game, whose player stands at
// (0.5, 64, 0.5) facing south.

use horizon_studio::game::hstudio_meshes::{base64, decode_png, parse_bbmodel, parse_obj};
use horizon_studio::headless::{Run, Source, run_steps};
use serde_json::json;

fn run(src: &str) -> Run {
    let source = Source::Files {
        id: "m".into(),
        files: vec![("m.lua".into(), src.into())],
        libraries: None,
    };
    let mut run = Run::new(&source).unwrap();
    run.frames(2);
    assert_eq!(run.mods()[0]["state"], "running", "{:#}", run.mods());
    run
}

/// What a mod command returned (it is shown in chat).
fn say(r: &mut Run, command: &str) -> String {
    assert!(r.studio.command(command), "/{command}");
    r.frame();
    r.game.chats.last().cloned().unwrap_or_default()
}

fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
    out
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            s.push(if k <= c.len() {
                A[(n >> (18 - 6 * k) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    s
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn base64_and_png_decode() {
    let red_dot = png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]);
    assert_eq!(base64(&b64(&red_dot)).unwrap(), red_dot);
    assert_eq!(
        base64(&format!("data:image/png;base64,{}", b64(&red_dot))).unwrap(),
        red_dot
    );
    let t = decode_png(&red_dot).unwrap();
    assert_eq!((t.w, t.h), (2, 1));
    assert_eq!(t.get(0, 0), Some(0xFFFF_0000));
    assert_eq!(t.get(1, 0), Some(0x8000_00FF));
    assert!(base64("not*base64").is_err());
    assert!(decode_png(b"nope").unwrap_err().contains("not a PNG"));
    let big = png(600, 1, &vec![0; 600 * 4]);
    assert!(decode_png(&big).unwrap_err().contains("1 to 512 pixels"));
}

#[test]
fn textures_are_made_painted_and_read() {
    let dot = b64(&png(1, 1, &[10, 20, 30, 255]));
    let mut r = run(&format!(
        r##"local t = texture.create(4, 2, "#ff0000")
           command("px", function()
             texture.set(t, 1, 0, "#00ff00")
             texture.fill(t, 2, 0, 2, 2, 0x800000ff)
             texture.paint(t, {{"ab", "b."}}, {{a = "#ffffff", b = "#000000"}}, 0, 0)
             local w, h = texture.size(t)
             return string.format("%dx%d %x %x %x %x %s", w, h, texture.get(t, 0, 0), texture.get(t, 1, 0),
               texture.get(t, 3, 1), texture.get(t, 1, 1), tostring(texture.get(t, 9, 9)))
           end)
           command("decode", function()
             local id, w, h = texture.decode("{dot}")
             return string.format("%d %dx%d %x", id, w, h, texture.get(id, 0, 0))
           end)
           command("free", function() return tostring(texture.free(t)) .. tostring(texture.free(t)) end)"##
    ));
    // The palette paints "a" white and "b" black, and leaves "." as it was.
    assert_eq!(say(&mut r, "px"), "4x2 ffffffff ff000000 800000ff ffff0000 nil");
    assert_eq!(say(&mut r, "decode"), "2 1x1 ff0a141e");
    assert_eq!(say(&mut r, "free"), "truefalse");
}

#[test]
fn meshes_from_faces_and_boxes() {
    let mut r = run(r##"local skin = texture.create(64, 64)
           tri = mesh.new({ vertices = {{0, 0, 0}, {16, 0, 0}, {0, 16, 0}, {16, 16, 0}},
                            uvs = {{0, 1}, {1, 1}, {0, 0}, {1, 0}},
                            faces = {{1, 2, 4, 3}}, color = "#c0392b" })
           head = mesh.new({ texture = skin, boxes = {{ from = {-4, 0, -4}, to = {4, 8, 4}, uv = {0, 0} }} })
           command("info", function()
             local a, b = mesh.info(tri), mesh.info(head)
             return a.triangles .. " " .. b.triangles .. " " .. b.min[1] .. "," .. b.max[2] .. " " .. #b.textures
           end)"##);
    assert_eq!(say(&mut r, "info"), "2 12 -4,8 1");
    // A skin's head: its front (+z) face takes the layout's face slot, 8..16 by 8..16.
    let head = r.studio.s.mods[0].meshes.meshes.values().nth(1).unwrap().clone();
    let front: Vec<_> = head.tris.iter().filter(|t| t.p.iter().all(|p| p[2] == 4.0)).collect();
    assert_eq!(front.len(), 2);
    let us: Vec<f32> = front.iter().flat_map(|t| t.uv.map(|uv| uv[0] * 64.0)).collect();
    let vs: Vec<f32> = front.iter().flat_map(|t| t.uv.map(|uv| uv[1] * 64.0)).collect();
    assert_eq!(us.iter().cloned().fold(f32::MAX, f32::min), 8.0);
    assert_eq!(us.iter().cloned().fold(f32::MIN, f32::max), 16.0);
    assert_eq!(vs.iter().cloned().fold(f32::MAX, f32::min), 8.0);
}

const CUBE_OBJ: &str = "# a unit cube
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1
vt 0 0
vt 1 0
vt 1 1
vt 0 1
f 1/1 2/2 3/3 4/4
f 5/1 6/2 7/3 8/4
f 1 2 6 5
f -5 -6 -2 -1
f 1 4 8 5
f 2 3 7 6
";

#[test]
fn obj_files() {
    let m = parse_obj(CUBE_OBJ, 16.0, 0xFFFF_FFFF, None).unwrap();
    assert_eq!(m.tris.len(), 12);
    assert_eq!(m.bounds(), ([0.0; 3], [16.0; 3]));
    // .obj's v counts up from the bottom.
    assert_eq!(m.tris[0].uv[0], [0.0, 1.0]);
    assert!(
        parse_obj("v 0 0 0\nf 1 2 3\n", 16.0, 0, None)
            .unwrap_err()
            .starts_with("line 2:")
    );
    assert!(
        parse_obj("v 0 0\n", 16.0, 0, None)
            .unwrap_err()
            .contains("v needs x y z")
    );
    assert!(
        parse_obj("# nothing\n", 16.0, 0, None)
            .unwrap_err()
            .contains("no faces")
    );
}

fn bbmodel(texture: &str) -> String {
    json!({
        "meta": { "format_version": "4.10", "model_format": "free" },
        "resolution": { "width": 16, "height": 16 },
        "elements": [
            { "type": "cube", "uuid": "c1", "from": [-2, 0, -2], "to": [2, 4, 2], "origin": [0, 0, 0],
              "faces": {
                  "north": { "uv": [0, 0, 4, 4], "texture": 0 },
                  "south": { "uv": [4, 0, 8, 4], "texture": 0 },
                  "east": { "uv": [0, 4, 4, 8], "texture": 0 },
                  "west": { "uv": [4, 4, 8, 8], "texture": 0 },
                  "up": { "uv": [8, 0, 12, 4], "texture": 0 },
                  "down": { "uv": [8, 4, 12, 8], "texture": null } } },
            { "type": "mesh", "uuid": "m1", "origin": [0, 8, 0],
              "vertices": { "a": [0, 0, 0], "b": [2, 0, 0], "c": [0, 2, 0] },
              "faces": { "f": { "vertices": ["a", "b", "c"], "uv": { "a": [0, 0], "b": [16, 0], "c": [0, 16] }, "texture": 0 } } }
        ],
        "outliner": [ { "name": "body", "origin": [0, 0, 0], "rotation": [0, 90, 0], "children": ["c1"] }, "m1" ],
        "textures": [ { "name": "skin", "source": format!("data:image/png;base64,{texture}") } ]
    })
    .to_string()
}

#[test]
fn blockbench_models() {
    let tex = b64(&png(16, 16, &[200; 16 * 16 * 4]));
    let m = parse_bbmodel(&bbmodel(&tex), false).unwrap();
    // Five faces of the cube (down has no texture) and one triangle.
    assert_eq!(m.mesh.tris.len(), 11);
    assert_eq!(m.textures.len(), 1);
    assert_eq!(m.textures[0].as_ref().map(|t| (t.w, t.h)), Some((16, 16)));
    // The mesh's corners are from its origin, 8 up.
    let tri = m.mesh.tris.last().unwrap();
    assert_eq!(tri.p[2], [0.0, 10.0, 0.0]);
    assert_eq!(tri.uv[1], [1.0, 0.0]);
    // Flipped half round, Blockbench's north-facing front faces +z.
    let flipped = parse_bbmodel(&bbmodel(&tex), true).unwrap();
    assert_eq!(flipped.mesh.tris.last().unwrap().p[1], [-2.0, 8.0, 0.0]);
    assert!(parse_bbmodel("{", true).unwrap_err().contains("not a .bbmodel"));
    assert!(
        parse_bbmodel("{\"elements\": []}", true)
            .unwrap_err()
            .contains("no faces")
    );
}

#[test]
fn drawn_in_the_world_and_on_the_body() {
    let tex = b64(&png(16, 16, &[200; 16 * 16 * 4]));
    let mut r = run(&format!(
        r##"local statue = mesh.parse([[{}]], "bbmodel")
           local cube = mesh.parse([[{CUBE_OBJ}]], "obj")
           local arrow = mesh.new({{ vertices = {{{{0, 0, 0}}, {{1, 0, 0}}, {{0, 0, 16}}}}, faces = {{{{1, 2, 3}}}} }})
           on("world", function()
             draw.mesh(cube, 10, 64, 10, {{ scale = 0.5 }})
             draw.mesh(arrow, 0, 70, 0)
           end)
           command("hat", function() return model.attach("head", {{ mesh = statue }}) end)
           command("bad", function() return model.attach("head", {{ mesh = 99 }}) end)"##,
        bbmodel(&tex)
    ));
    r.game.meshes.clear();
    r.frame();
    let drawn = r.game.meshes.clone();
    assert_eq!(drawn.len(), 2, "{drawn:?}");
    assert_eq!((drawn[0].0, drawn[0].1), (12, None));
    // Half scale: the cube's corner at the origin, its far one half a block away.
    assert!(near(drawn[0].2[0], 10.0) && near(drawn[0].2[1], 64.0), "{drawn:?}");
    assert_eq!(r.game.meshes[1].0, 1);
    say(&mut r, "hat");
    r.game.meshes.clear();
    r.frame();
    let on_head = r.game.meshes.iter().find(|m| m.1 == Some((16, 16))).cloned();
    assert!(on_head.is_some_and(|m| m.0 == 11), "{:?}", r.game.meshes);
    assert!(say(&mut r, "bad").contains("this mod has no mesh 99"));
}

#[test]
fn yaw_turns_a_mesh_as_the_player_turns() {
    let mut r = run(
        r#"local arrow = mesh.new({ vertices = {{0, 0, 0}, {0, 0, 0}, {0, 0, 16}}, faces = {{3, 1, 2}} })
           on("world", function() draw.mesh(arrow, 0, 70, 0, { yaw = 90 }) end)"#,
    );
    r.game.meshes.clear();
    r.frame();
    // The first corner is the tip: a block ahead, which at yaw 90 is west.
    let tip = r.game.meshes[0].2;
    assert!(near(tip[0], -1.0) && near(tip[1], 70.0) && near(tip[2], 0.0), "{tip:?}");
}

#[test]
fn limits_and_mistakes() {
    let mut r = run("");
    for (code, want) in [
        ("texture.load('skin.png')", "reads files of a folder mod"),
        ("mesh.load('m.obj')", "mesh.parse or mesh.new"),
        ("texture.create(0, 4)", "1 to 512 pixels"),
        ("texture.set(42, 0, 0, 0)", "has no texture 42"),
        ("mesh.parse('v 0 0 0', 'stl')", "'stl' is not a format it reads"),
        ("mesh.parse('v 0 0 0', 'obj')", "no faces"),
        ("mesh.new({ faces = {{1, 2, 3}} })", "faces[1]"),
        ("mesh.new({})", "give faces (with vertices) or boxes"),
        ("draw.mesh(1, 0, 0, 0)", "draws only inside on(\"world\""),
        (
            "draw.triangle(0, 0, 0, 1, 0, 0, 0, 1, 0)",
            "draws only inside on(\"world\"",
        ),
        ("texture.decode('not png')", "not a PNG"),
        ("texture.decode('*')", "not base64"),
    ] {
        let out = r.studio.run_chunk(&mut r.game, code, false);
        assert!(out.iter().any(|(_, t)| t.contains(want)), "{code}: {out:?}");
    }
    // Every mod's textures together are at most a 1024x1024's pixels.
    let out = r.studio.run_chunk(
        &mut r.game,
        "for i = 1, 4 do texture.create(512, 512) end return texture.create(1, 1)",
        false,
    );
    assert!(out.iter().any(|(_, t)| t.contains("at most 1048576 pixels")), "{out:?}");
}

#[test]
fn folder_mods_load_their_files() {
    let dir = std::env::temp_dir().join(format!("hs-meshes-{}", std::process::id()));
    let folder = dir.join("statue");
    std::fs::create_dir_all(folder.join("models")).unwrap();
    std::fs::write(folder.join("skin.png"), png(4, 4, &[90; 64])).unwrap();
    std::fs::write(folder.join("models/cube.obj"), CUBE_OBJ).unwrap();
    std::fs::write(
        folder.join("main.lua"),
        r#"local skin, w, h = texture.load("skin.png")
           cube = mesh.load("models/cube.obj", { texture = skin, scale = 8 })
           command("info", function() local i = mesh.info(cube) return w .. "x" .. h .. " " .. i.triangles .. " " .. i.max[1] end)
           command("escape", function() return texture.load("../secret.png") end)
           command("missing", function() return mesh.load("nope.obj") end)"#,
    )
    .unwrap();
    let mut r = Run::new(&Source::Installed {
        mods_dir: dir.clone(),
        id: "statue".into(),
    })
    .unwrap();
    r.frames(2);
    assert_eq!(r.mods()[0]["state"], "running", "{:#}", r.mods());
    assert_eq!(say(&mut r, "info"), "4x4 12 8");
    assert!(say(&mut r, "escape").contains("not a path inside the mod's folder"));
    assert!(say(&mut r, "missing").contains("no file 'nope.obj'"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_steps_see_meshes() {
    let mut r = run(
        r##"local m = mesh.new({ boxes = {{ from = {0, 0, 0}, to = {4, 4, 4}, color = "#ffd700" }} })
           local shown = false
           command("show", function() shown = true end)
           on("world", function() if shown then draw.mesh(m, 0, 70, 0) end end)"##,
    );
    let report = run_steps(
        &mut r,
        &[
            json!({ "frames": 2, "expect_mesh": false }),
            json!({ "command": "/show", "expect_mesh": 12 }),
            json!({ "frames": 1, "expect_mesh": 13 }),
        ],
    );
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps[0]["ok"], true, "{report:#}");
    assert_eq!(steps[1]["ok"], true, "{report:#}");
    assert_eq!(steps[1]["meshes"]["triangles"], 12);
    assert_eq!(steps[2]["ok"], false, "only 12 triangles a frame");
}
