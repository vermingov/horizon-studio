// 3D drawing, sounds, particles and script screens, on the fake game.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{Camera, ModState, Overlay, Studio, box_edges, box_faces};
use horizon_studio::game::hstudio_ui::{Input, Key, Mods, UiResult, draw_screen, handle_screen};

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

#[test]
fn the_camera_projects_like_vanillas() {
    let cam = Camera {
        pos: [0.0, 0.0, 0.0],
        yaw: 0.0,
        pitch: 0.0,
        fov: 90.0,
    };
    // Yaw 0 looks south (+Z): straight ahead lands in the middle.
    let (x, y, d) = cam.project([0.0, 0.0, 10.0], 400.0, 200.0).unwrap();
    assert!((x - 200.0).abs() < 1e-3 && (y - 100.0).abs() < 1e-3 && (d - 10.0).abs() < 1e-9);
    // West (-X) is on the right when facing south; up is up.
    let (x, y, _) = cam.project([-5.0, 5.0, 10.0], 400.0, 200.0).unwrap();
    assert!(x > 200.0 && y < 100.0, "{x} {y}");
    // With a 90 degree FOV, 45 degrees up reaches the top edge.
    let (_, y, _) = cam.project([0.0, 10.0, 10.0], 400.0, 200.0).unwrap();
    assert!(y.abs() < 1e-3, "{y}");
    assert!(cam.project([0.0, 0.0, -10.0], 400.0, 200.0).is_none(), "behind");
    // Looking north (yaw 180) flips it.
    let north = Camera { yaw: 180.0, ..cam };
    assert!(north.project([0.0, 0.0, -10.0], 400.0, 200.0).is_some());
    let up = Camera { pitch: -90.0, ..cam };
    assert!(
        up.project([0.0, 10.0, 0.0], 400.0, 200.0).is_some(),
        "pitch -90 looks up"
    );
}

#[test]
fn box_geometry() {
    let edges = box_edges([0.0; 3], [1.0, 2.0, 3.0]);
    assert_eq!(edges.len(), 12);
    let total: f64 = edges
        .iter()
        .map(|(a, b)| (0..3).map(|i| (b[i] - a[i]).abs()).sum::<f64>())
        .sum();
    assert_eq!(total, 4.0 * (1.0 + 2.0 + 3.0));
    let faces = box_faces([0.0; 3], [1.0; 3]);
    assert_eq!(faces.len(), 6);
}

#[test]
fn world_handlers_draw_shapes() {
    let mut game = FakeGame::new("world-draw");
    let mut st = studio(&mut game);
    game.write_mod(
        "shapes.lua",
        "on('world', function()\n  draw.line(0, 64, 0, 0, 70, 0, '#ff0000')\n  draw.box(0, 64, 0, 2, 66, 2, '#00ff00')\n  draw.block(5, 64, 5)\n  draw.fill(0, 0, 0, 1, 1, 1, 0xff0000ff)\n  draw.beam(10.5, -20.5, '#9a86ff')\n  draw.line(0, 64, -10, 1, 64, -10, nil, nil, true)\n  draw.text(10.5, 66, -10, 'Home', '#ffff00', true)\nend)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.mods[mod_index(&st, "shapes")].running());
    game.world_quads.clear();
    st.world(&mut game);
    // 1 line + 12 box edges + 12 block edges + 6 faces + 4 beam sides.
    assert_eq!(game.world_quads.len(), 1 + 12 + 12 + 6 + 4);
    assert_eq!(game.world_quads[0].1, 0xFFFF_0000);
    assert_eq!(game.world_quads[13].1, 0xFFFF_FFFF, "block outlines default to white");
    assert_eq!(st.s.overlay.len(), 2);
    assert!(matches!(&st.s.overlay[1], Overlay::Text { text, distance: true, .. } if text == "Home"));
    // The beam stands from -64 to 320.
    let beam = &game.world_quads[31..];
    let ys: Vec<f64> = beam.iter().flat_map(|(c, _)| c.iter().map(|p| p[1])).collect();
    assert_eq!(ys.iter().cloned().fold(f64::MAX, f64::min), -64.0);
    assert_eq!(ys.iter().cloned().fold(f64::MIN, f64::max), 320.0);
    // A line's ribbon faces the camera: its width is across the view.
    let (c, _) = &game.world_quads[0];
    let width: f64 = (0..3).map(|i| (c[1][i] - c[0][i]).powi(2)).sum::<f64>().sqrt();
    assert!((width - 0.04).abs() < 1e-9, "{width}");
    // The HUD shows what goes through walls: the line as a GUI quad, the label as text.
    game.quads.clear();
    game.drawn.clear();
    st.hud(&mut game);
    assert_eq!(game.quads.len(), 1, "the through-wall line");
    let label = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text Home ("))
        .expect("label drawn");
    assert!(label.contains("(7 m)"), "{label}");
    // Labels behind the camera don't show.
    game.camera = Some(Camera {
        yaw: 0.0,
        ..game.camera.unwrap()
    });
    game.drawn.clear();
    st.hud(&mut game);
    assert!(!game.drawn.iter().any(|d| d.starts_with("text Home")));
}

#[test]
fn draw_needs_the_world_phase() {
    let mut game = FakeGame::new("world-phase");
    let mut st = studio(&mut game);
    let out = console(&mut st, &mut game, "draw.line(0, 0, 0, 1, 1, 1)");
    assert!(out[0].contains("draw.line draws only inside on(\"world\""), "{out:?}");
    console(
        &mut st,
        &mut game,
        "on('world', function() sound.play('block.note_block.bell') end)",
    );
    st.world(&mut game);
    assert!(
        log(&st)
            .iter()
            .any(|l| l.contains("sound.play doesn't work while drawing")),
        "{:?}",
        log(&st)
    );
    game.write_mod(
        "greedy.lua",
        "on('world', function() for i = 1, 20000 do draw.fill(0,0,0,1,1,1) end end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    st.world(&mut game);
    match &st.s.mods[mod_index(&st, "greedy")].state {
        ModState::Failed(msg, _) => assert!(msg.contains("too much drawn"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn sounds_and_particles() {
    let mut game = FakeGame::new("sounds");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "sound.play('entity.experience_orb.pickup') sound.at('minecraft:block.bell.use', 1, 2, 3, 0.5, 9)",
    );
    assert_eq!(
        game.sounds[0],
        ("minecraft:entity.experience_orb.pickup".to_owned(), None, 1.0, 1.0)
    );
    assert_eq!(
        game.sounds[1],
        ("minecraft:block.bell.use".to_owned(), Some([1.0, 2.0, 3.0]), 0.5, 2.0)
    );
    console(&mut st, &mut game, "world.particle('flame', 1, 2, 3, 0, 0.1, 0)");
    assert_eq!(
        game.particles[0],
        ("flame".to_owned(), [1.0, 2.0, 3.0], [0.0, 0.1, 0.0], None)
    );
    console(
        &mut st,
        &mut game,
        "world.particle('dust', 0, 64, 0, 0, 0, 0, 10, '#ff0000')",
    );
    assert_eq!(game.particles.len(), 11);
    let (_, at, _, color) = &game.particles[5];
    assert!((at[1] - 64.0).abs() <= 0.25);
    assert_eq!(*color, Some([1.0, 0.0, 0.0, 1.0]));
    let out = console(&mut st, &mut game, "world.particle('nope', 0, 0, 0)");
    assert!(out[0].contains("unknown particle nope"), "{out:?}");
    let out = console(&mut st, &mut game, "for i = 1, 40 do sound.play('ui.button.click') end");
    assert!(out[0].contains("too many sounds this frame"), "{out:?}");
}

fn screen_input(st: &mut Studio, game: &mut FakeGame, input: Input) -> UiResult {
    handle_screen(st, game, input)
}

/// Clicks the middle of the screen row whose drawn text starts with `label`.
fn click_label(st: &mut Studio, game: &mut FakeGame, label: &str) {
    game.drawn.clear();
    draw_screen(st, game);
    let line = game
        .drawn
        .iter()
        .find(|d| d.starts_with(&format!("text {label} @")))
        .cloned()
        .unwrap_or_else(|| panic!("{label} not drawn: {:?}", game.drawn));
    let at = line
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap()
        .to_owned();
    let xy: Vec<f32> = at.split(", ").map(|v| v.parse().unwrap()).collect();
    screen_input(st, game, Input::Mouse(xy[0] + 1.0, xy[1] + 1.0));
    screen_input(st, game, Input::Button { left: true, down: true });
    screen_input(
        st,
        game,
        Input::Button {
            left: true,
            down: false,
        },
    );
}

const SCREEN: &str = r#"
clicks, toggled, radius, name, submitted, closed = 0, nil, nil, nil, nil, false
function show()
  ui.open({
    title = "Settings",
    widgets = {
      { type = "label", text = "Change things" },
      { type = "button", text = "Press me", click = function() clicks = clicks + 1 end },
      { type = "toggle", text = "Glow", value = false, change = function(v) toggled = v end },
      { type = "slider", text = "Radius", min = 0, max = 10, step = 1, value = 5, change = function(v) radius = v end },
      { type = "input", text = "Name", value = "", placeholder = "type here", change = function(v) name = v end, submit = function(v) submitted = v end },
    },
    close = function() closed = true end,
  })
end
command("settings", show)
"#;

#[test]
fn a_script_screen_with_widgets() {
    let mut game = FakeGame::new("screen");
    let mut st = studio(&mut game);
    game.write_mod("settings.lua", SCREEN);
    run_for(&mut st, &mut game, 0.6);
    assert!(st.command("settings"));
    st.frame(&mut game, 0.016);
    assert!(st.s.screen_request.is_some());
    assert!(st.accept_screen());
    assert!(st.overlay_open());
    assert_eq!(
        console(&mut st, &mut game, "ui.is_open()"),
        vec!["false"],
        "the console doesn't own it"
    );
    game.drawn.clear();
    draw_screen(&mut st, &mut game);
    for t in [
        "Settings",
        "Change things",
        "Press me",
        "Glow",
        "Radius",
        "5",
        "Name",
        "type here",
    ] {
        assert!(
            game.drawn.iter().any(|d| d.starts_with(&format!("text {t} @"))),
            "{t} not drawn: {:?}",
            game.drawn
        );
    }
    click_label(&mut st, &mut game, "Press me");
    click_label(&mut st, &mut game, "Press me");
    click_label(&mut st, &mut game, "Glow");
    let i = mod_index(&st, "settings");
    let get = |st: &mut Studio, game: &mut FakeGame, name: &str| {
        // The mod's own globals, read in its environment.
        let env = st.s.mods[i].env.clone().unwrap();
        let f = st.lua.load(&format!("return {name}"), "t", &env).unwrap();
        let _ = game;
        let r = st
            .lua
            .call(&mut horizon_studio::game::hstudio_lua::NoHost::default(), &f, vec![])
            .ok()
            .unwrap();
        horizon_studio::game::hstudio_lualib::inspect(&r[0], 1)
    };
    assert_eq!(get(&mut st, &mut game, "clicks"), "2");
    assert_eq!(get(&mut st, &mut game, "toggled"), "true");
    // The slider: drag to its far right.
    game.drawn.clear();
    draw_screen(&mut st, &mut game);
    let radius = game
        .drawn
        .iter()
        .find(|d| d.starts_with("text Radius @"))
        .cloned()
        .unwrap();
    let y: f32 = radius
        .split(", ")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let x: f32 = radius
        .split('@')
        .nth(1)
        .unwrap()
        .trim_start_matches('[')
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    screen_input(&mut st, &mut game, Input::Mouse(x + 10.0, y + 14.0));
    screen_input(&mut st, &mut game, Input::Button { left: true, down: true });
    screen_input(&mut st, &mut game, Input::Mouse(x + 900.0, y + 14.0));
    screen_input(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(get(&mut st, &mut game, "radius"), "10");
    // The text field: click, type, Enter.
    click_label(&mut st, &mut game, "type here");
    screen_input(&mut st, &mut game, Input::Text("Ste".into()));
    screen_input(&mut st, &mut game, Input::Text("ve".into()));
    screen_input(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Backspace,
            down: true,
            repeat: false,
            mods: Mods::default(),
        },
    );
    assert_eq!(get(&mut st, &mut game, "name"), "\"Stev\"");
    screen_input(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Enter,
            down: true,
            repeat: false,
            mods: Mods::default(),
        },
    );
    assert_eq!(get(&mut st, &mut game, "submitted"), "\"Stev\"");
    // Escape closes, and the close handler runs.
    let r = screen_input(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Escape,
            down: true,
            repeat: false,
            mods: Mods::default(),
        },
    );
    assert_eq!(r, UiResult::Close);
    st.close_screen(&mut game);
    assert!(!st.overlay_open());
    assert_eq!(get(&mut st, &mut game, "closed"), "true");
}

#[test]
fn a_custom_drawn_screen() {
    let mut game = FakeGame::new("screen-custom");
    let mut st = studio(&mut game);
    console(
        &mut st,
        &mut game,
        "last = nil keys = {} ui.open({ draw = function(w, h) hud.rect(10, 10, 50, 20, '#ff0000') hud.text(12, 15, 'Custom ' .. w) end, click = function(x, y, b) last = x .. ',' .. y .. ',' .. b end, key = function(k, down) if down then keys[#keys + 1] = k end end, text = function(t) keys[#keys + 1] = 'text:' .. t end })",
    );
    assert!(st.accept_screen());
    game.drawn.clear();
    draw_screen(&mut st, &mut game);
    assert!(
        game.drawn.contains(&"text Custom 960 @[12.0, 15.0] ffffff".to_owned()),
        "{:?}",
        game.drawn
    );
    screen_input(&mut st, &mut game, Input::Mouse(33.0, 44.0));
    screen_input(
        &mut st,
        &mut game,
        Input::Button {
            left: false,
            down: true,
        },
    );
    screen_input(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Char('w'),
            down: true,
            repeat: false,
            mods: Mods::default(),
        },
    );
    screen_input(&mut st, &mut game, Input::Text("w".into()));
    assert_eq!(console(&mut st, &mut game, "last"), vec!["\"33,44,right\""]);
    assert_eq!(
        console(&mut st, &mut game, "table.concat(keys, ' ')"),
        vec!["\"w text:w\""]
    );
    assert_eq!(console(&mut st, &mut game, "ui.is_open()"), vec!["true"]);
    console(&mut st, &mut game, "ui.close()");
    assert!(!st.overlay_open());
}

#[test]
fn a_failing_screen_closes_with_its_mod() {
    let mut game = FakeGame::new("screen-fail");
    let mut st = studio(&mut game);
    game.write_mod(
        "bad_screen.lua",
        "ui.open({ title = 'Bad', draw = function() error('broken draw') end })\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(st.accept_screen());
    draw_screen(&mut st, &mut game);
    assert!(st.s.screen.is_none());
    match &st.s.mods[mod_index(&st, "bad_screen")].state {
        ModState::Failed(msg, _) => assert!(msg.contains("broken draw"), "{msg}"),
        other => panic!("{other:?}"),
    }
    // A mod reloaded takes its screen away.
    game.write_mod("other_screen.lua", "ui.open({ title = 'Other' })\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(st.accept_screen());
    let i = mod_index(&st, "other_screen");
    st.set_enabled(&mut game, i, false);
    assert!(st.s.screen.is_none());
}

#[test]
fn the_new_examples_draw_and_open_screens() {
    use horizon_studio::game::hstudio_examples::EXAMPLES;
    let mut game = FakeGame::new("examples-3d");
    let mut st = studio(&mut game);
    for id in ["waypoints", "selection", "playground"] {
        let i = EXAMPLES.iter().position(|e| e.id == id).unwrap();
        st.install_example(&mut game, i).unwrap();
    }
    run_for(&mut st, &mut game, 0.1);
    // Waypoints: a beam, a block outline and a label through walls.
    assert!(st.command("wp add home"));
    run_for(&mut st, &mut game, 0.1);
    assert!(
        game.chats.iter().any(|c| c == "Waypoint home added"),
        "{:?}",
        game.chats
    );
    game.world_quads.clear();
    st.world(&mut game);
    assert_eq!(game.world_quads.len(), 4, "the beam's sides");
    assert!(
        st.s.overlay
            .iter()
            .any(|o| matches!(o, Overlay::Text { text, .. } if text == "home"))
    );
    // Its menu lists it; clicking removes it.
    assert!(st.command("wp"));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen());
    click_label(&mut st, &mut game, "home  (10, 64, -4)");
    assert!(game.sounds.iter().any(|s| s.0 == "minecraft:ui.button.click"));
    game.world_quads.clear();
    st.world(&mut game);
    assert!(game.world_quads.is_empty(), "removed");
    handle_screen(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Escape,
            down: true,
            repeat: false,
            mods: Mods::default(),
        },
    );
    st.close_screen(&mut game);
    // Selection: two corners, a box and its size.
    assert!(st.command("pos1"));
    run_for(&mut st, &mut game, 0.1);
    game.player.as_mut().unwrap().x += 3.0;
    assert!(st.command("pos2"));
    run_for(&mut st, &mut game, 0.1);
    st.world(&mut game);
    assert!(
        st.s.overlay
            .iter()
            .any(|o| matches!(o, Overlay::Text { text, .. } if text == "4 x 1 x 1")),
        "{:?}",
        st.s.overlay
    );
    // Playground: the trail's toggle makes particles; the bell rings.
    assert!(st.command("playground"));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen());
    click_label(&mut st, &mut game, "Particle trail");
    let before = game.particles.len();
    run_for(&mut st, &mut game, 0.5);
    assert!(
        game.particles.len() >= before + 9 * 3,
        "{} -> {}",
        before,
        game.particles.len()
    );
    assert!(game.particles.iter().all(|p| p.0 == "dust"));
    click_label(&mut st, &mut game, "Ring the bell");
    assert!(
        game.sounds
            .iter()
            .any(|s| s.0 == "minecraft:block.bell.use" && s.1.is_some())
    );
    assert_eq!(game.particles.iter().filter(|p| p.0 == "note").count(), 16);
    click_label(&mut st, &mut game, "Close");
    assert!(st.s.screen.is_none());
    for m in &st.s.mods {
        assert_eq!(m.state, ModState::Running, "{}", m.id);
    }
}
