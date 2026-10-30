// Models attached to the player (`model.*`), `player.part`, `player.pose`,
// `player.camera` and `draw.item`, on the headless game: its player stands at
// (0.5, 64, 0.5) facing south, its model 15/16 of a block-sized one.

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

/// The items drawn by the last frame.
fn items(run: &mut Run) -> Vec<(String, [f64; 3], String)> {
    run.game.items.clear();
    run.frame();
    run.game.items.clone()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.02
}

const PX: f64 = 0.9375 / 16.0;

#[test]
fn a_hat_sits_on_the_head_and_a_sword_in_the_right_hand() {
    let mut r = run(r#"hat = model.attach("head", { item = "minecraft:carved_pumpkin" })
           sword = model.attach("right_arm", { item = "diamond_sword" })"#);
    let drawn = items(&mut r);
    assert_eq!(drawn.len(), 2, "{drawn:?}");
    let (id, at, display) = &drawn[0];
    assert_eq!((id.as_str(), display.as_str()), ("minecraft:carved_pumpkin", "head"));
    // The neck is 24 pixels up; the helmet item sits 4 above it.
    assert!(
        near(at[0], 0.5) && near(at[1], 64.0 + 28.0 * PX) && near(at[2], 0.5),
        "{at:?}"
    );
    let (id, at, display) = &drawn[1];
    assert_eq!((id.as_str(), display.as_str()), ("minecraft:diamond_sword", "hand"));
    // Facing south, its right is west: the shoulder 5 pixels and the hand 1 more,
    // 10 below the shoulder (22 up), 2 forward.
    assert!(
        near(at[0], 0.5 - 6.0 * PX) && near(at[1], 64.0 + 12.0 * PX) && near(at[2], 0.5 + 2.0 * PX),
        "{at:?}"
    );
}

#[test]
fn offsets_follow_the_body_as_it_turns() {
    let mut r = run(r#"model.attach("body", { item = "minecraft:stick", offset = { 2, 0, 3 } })"#);
    let (_, at, display) = items(&mut r)[0].clone();
    assert_eq!(display, "fixed");
    // Facing south: right is -x, forward is +z.
    assert!(near(at[0], 0.5 - 2.0 * PX) && near(at[2], 0.5 + 3.0 * PX), "{at:?}");
    // Facing west (yaw 90): right is -z, forward is -x.
    r.game.player.as_mut().unwrap().yaw = 90.0;
    let (_, at, _) = items(&mut r)[0].clone();
    assert!(near(at[0], 0.5 - 3.0 * PX) && near(at[2], 0.5 - 2.0 * PX), "{at:?}");
}

#[test]
fn boxes_hide_in_first_person_unless_asked_and_change_with_set() {
    let mut r = run(
        r##"cape = model.attach("body", { boxes = { { from = {-5, 1, -3}, to = {5, 16, -2}, color = "#c0392b" } } })
           halo = model.attach("head", { boxes = { { from = {-4, 10, -4}, to = {4, 11, 4}, color = "#ffd700" } }, glow = true, first_person = true })
           command("change", function()
             model.set(halo, { visible = false })
             model.set(cape, { item = "minecraft:elytra", boxes = false })
             return #model.list() .. " " .. model.list()[1].item
           end)
           command("drop", function()
             return tostring(model.remove(cape)) .. " " .. tostring(model.remove(cape)) .. " " .. #model.list()
           end)"##,
    );
    r.game.world_quads = 0;
    r.frame();
    assert_eq!(r.game.world_quads, 12, "two boxes, six faces each");
    r.game.camera_mode = "first".into();
    r.game.world_quads = 0;
    r.frame();
    assert_eq!(r.game.world_quads, 6, "only the halo shows in first person");
    // model.set changes what is given; item = false and visible = false work too.
    assert!(r.studio.command("change"));
    r.frame();
    assert_eq!(r.game.chats.last().unwrap(), "2 minecraft:elytra");
    r.game.camera_mode = "third_back".into();
    r.game.world_quads = 0;
    let drawn = items(&mut r);
    assert_eq!(r.game.world_quads, 0);
    assert_eq!(
        drawn.iter().map(|d| d.0.as_str()).collect::<Vec<_>>(),
        ["minecraft:elytra"]
    );
    assert!(r.studio.command("drop"));
    r.frame();
    assert_eq!(r.game.chats.last().unwrap(), "true false 1");
}

#[test]
fn reloading_the_mod_takes_its_models_away() {
    let mut r = run(r#"model.attach("head", { item = "minecraft:carved_pumpkin" })"#);
    assert_eq!(items(&mut r).len(), 1);
    let i = r.studio.s.mods.iter().position(|m| m.id == "m").unwrap();
    r.studio.unload(&mut r.game, i);
    assert!(items(&mut r).is_empty());
}

#[test]
fn bad_specs_say_what_is_wrong() {
    let mut r = run("");
    for (code, want) in [
        (
            "model.attach('tail', { item = 'stick' })",
            "a body part expected: root, head",
        ),
        ("model.attach('head', {})", "give an item or boxes"),
        (
            "model.attach('head', { item = 'stick', display = 'hat' })",
            "display is one of head, hand",
        ),
        (
            "model.attach('head', { boxes = { { from = {0, 0} , to = {1, 1, 1} } } })",
            "needs from and to",
        ),
        (
            "model.attach('head', { item = 'stick', target = 'bob' })",
            "target is \"player\" or an entity id",
        ),
        ("model.set(99, { item = 'stick' })", "this mod has no model 99"),
        ("draw.item('stick', 0, 64, 0)", "draws only inside on(\"world\""),
    ] {
        let out = r.studio.run_chunk(&mut r.game, code, false);
        assert!(out.iter().any(|(_, t)| t.contains(want)), "{code}: {out:?}");
    }
}

#[test]
fn parts_pose_camera_and_draw_item() {
    let mut r = run(r#"on("world", function()
             local h = player.part("head")
             draw.item("minecraft:diamond", h.x, h.y + 1, h.z, { yaw = 45, scale = 0.5, glow = true })
           end)"#);
    let out = r.studio.run_chunk(
        &mut r.game,
        "local h = player.part('head') local a = player.part('right_arm') \
         return h.y, a.x, h.up.y, player.pose().pose, player.pose().crouching, player.camera()",
        false,
    );
    let text = &out.last().unwrap().1;
    let parts: Vec<&str> = text.split(", ").collect();
    assert!(near(parts[0].parse().unwrap(), 64.0 + 24.0 * PX), "{text}");
    assert!(near(parts[1].parse().unwrap(), 0.5 - 5.0 * PX), "{text}");
    assert!(near(parts[2].parse().unwrap(), 0.9375), "{text}");
    assert_eq!(&parts[3..], ["\"standing\"", "false", "\"third_back\""]);
    let drawn = items(&mut r);
    assert_eq!(drawn[0].0, "minecraft:diamond");
    assert!(near(drawn[0].1[1], 65.0 + 24.0 * PX), "{drawn:?}");
    r.game.pose = "crouching".into();
    let out = r.studio.run_chunk(&mut r.game, "return player.pose().crouching", false);
    assert_eq!(out.last().unwrap().1, "true");
}

#[test]
fn test_steps_see_the_models() {
    let mut r = run(r#"command("hat", function() model.attach("head", { item = "minecraft:carved_pumpkin" }) end)"#);
    let report = run_steps(
        &mut r,
        &[
            json!({ "command": "/hat", "expect_item": "carved_pumpkin" }),
            json!({ "camera": "first" }),
            json!({ "frames": 2, "expect_item": "carved_pumpkin" }),
        ],
    );
    let steps = report["steps"].as_array().unwrap();
    assert_eq!(steps[0]["ok"], true, "{report:#}");
    assert_eq!(steps[2]["ok"], false, "hidden in first person");
}
