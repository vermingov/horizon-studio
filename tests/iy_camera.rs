// Infinite Yield's camera, mouse, teleport and visuals commands, as the
// Roblox calls they make: what each asks of the game (the view, the
// perspective, the input it takes, the player and the client's world).

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, PlayerEntry, Studio};

/// A world with another player, Alex (entity 12), ten blocks north, a stone
/// wall at z = -10 and grass underfoot.
fn world(name: &str) -> FakeGame {
    let mut game = FakeGame::new(name);
    let entry = |name: &str, uuid: u128, entity: Option<u32>, local: bool| PlayerEntry {
        uuid,
        name: name.into(),
        display_name: name.into(),
        entity,
        local,
        latency: 50,
    };
    game.players = Some(vec![
        entry("Steve", 1, None, true),
        entry("Alex", 0x42, Some(12), false),
    ]);
    game.entities = vec![EntityInfo {
        id: 12,
        kind: "minecraft:player".into(),
        name: "Alex".into(),
        pos: [10.5, 64.0, -13.25],
        width: 0.6,
        height: 1.8,
        eye: 1.62,
        health: 20.0,
        max_health: 20.0,
        speed: 0.1,
        on_ground: true,
        head_yaw: 0.0,
        ..EntityInfo::default()
    }];
    for x in 0..20 {
        for y in 60..70 {
            game.blocks.insert((x, y, -20), "minecraft:stone".into());
        }
        for z in -25..5 {
            game.blocks.insert((x, 63, z), "minecraft:grass_block".into());
        }
    }
    game.blocks.insert((10, 65, -8), "minecraft:stone".into());
    game
}

/// The studio with the console's script looking at the world.
fn start(game: &mut FakeGame) -> Studio {
    let mut st = studio(game);
    run_for(&mut st, game, 0.1);
    lua(&mut st, game, "local _ = game.Players:GetPlayers()");
    st
}

/// Runs console Lua for a few frames: what it asked of the game, and what
/// it printed.
fn lua(st: &mut Studio, game: &mut FakeGame, src: &str) -> (Vec<String>, Vec<String>) {
    let (m0, l0) = (game.moves.len(), log(st).len());
    st.run_console(game, src);
    run_for(st, game, 0.2);
    let printed = log(st)[l0..].iter().skip(1).cloned().collect();
    (game.moves[m0..].to_vec(), printed)
}

/// Lets frames pass: what was asked of the game meanwhile.
fn frames(st: &mut Studio, game: &mut FakeGame, secs: f64) -> Vec<String> {
    let m0 = game.moves.len();
    run_for(st, game, secs);
    game.moves[m0..].to_vec()
}

fn has(moves: &[String], prefix: &str) -> bool {
    moves.iter().any(|m| m.starts_with(prefix))
}

// Infinite Yield's freecam, cut down: its keys and the mouse's movement
// taken by ContextActionService (sunk), and a render step that flies the
// camera with them.
const FREECAM: &str = r#"
    local CAS, RS = game:GetService("ContextActionService"), game:GetService("RunService")
    local cam = workspace.CurrentCamera
    local keys, pan = {W = 0, S = 0}, Vector2.new()
    local pos, rot = cam.CFrame.Position, Vector2.new()
    cam.CameraType = Enum.CameraType.Custom
    CAS:BindActionAtPriority("FreecamKeyboard", function(_, state, input)
        keys[input.KeyCode.Name] = state == Enum.UserInputState.Begin and 1 or 0
        return Enum.ContextActionResult.Sink
    end, false, Enum.ContextActionPriority.High.Value, Enum.KeyCode.W, Enum.KeyCode.S, Enum.KeyCode.E, Enum.KeyCode.Q)
    CAS:BindActionAtPriority("FreecamMousePan", function(_, _, input)
        pan = Vector2.new(-input.Delta.y, -input.Delta.x)
        return Enum.ContextActionResult.Sink
    end, false, Enum.ContextActionPriority.High.Value, Enum.UserInputType.MouseMovement)
    RS:BindToRenderStep("Freecam", Enum.RenderPriority.Camera.Value, function(dt)
        rot = rot + pan * (math.pi / 64)
        pan = Vector2.new()
        local frame = CFrame.new(pos) * CFrame.fromOrientation(rot.x, rot.y, 0) * CFrame.new(0, 0, (keys.S - keys.W) * 20 * dt)
        pos = frame.Position
        cam.CFrame = frame
    end)
    function stopFreecam()
        CAS:UnbindAction("FreecamKeyboard") CAS:UnbindAction("FreecamMousePan")
        RS:UnbindFromRenderStep("Freecam")
    end
"#;

#[test]
fn freecam_flies_the_view_with_the_keys_and_mouse_it_takes_while_the_player_stays() {
    let mut game = world("iyc-freecam");
    let mut st = start(&mut game);
    let (m, _) = lua(&mut st, &mut game, FREECAM);
    // The keys and the mouse's movement are the script's: the game gets none.
    assert!(
        m.contains(&"sink key.keyboard.e,key.keyboard.q,key.keyboard.s,key.keyboard.w look true".into()),
        "{m:?}"
    );
    // The view leaves the player, where the camera was, looking north.
    assert!(m.contains(&"view at 10.50 65.62 -3.25 180.0 0.0".into()), "{m:?}");
    st.key("key.keyboard.w", true);
    frames(&mut st, &mut game, 0.5);
    st.key("key.keyboard.w", false);
    let cam = game.camera.unwrap();
    assert!(cam.pos[2] < -10.0 && (cam.pos[0] - 10.5).abs() < 1e-6, "{cam:?}");
    // The mouse turns the view, not the player.
    game.mouse = [32.0, 0.0];
    frames(&mut st, &mut game, 0.2);
    let cam = game.camera.unwrap();
    assert!((cam.yaw + 90.0).abs() < 0.5, "{cam:?}");
    let p = game.player.clone().unwrap();
    assert_eq!((p.x, p.y, p.z, p.yaw), (10.5, 64.0, -3.25, 180.0));
    assert!(
        !game
            .moves
            .iter()
            .any(|m| m.starts_with("move") || m.starts_with("look")),
        "{:?}",
        game.moves
    );
    // notifyfreecamposition reads the camera's place.
    let (_, printed) = lua(
        &mut st,
        &mut game,
        "local p = workspace.CurrentCamera.CFrame.Position print(math.round(p.Z))",
    );
    assert_eq!(printed, [format!("{}", cam.pos[2].round())]);
    // unfreecam: the input and the view are the game's again.
    let (m, _) = lua(&mut st, &mut game, "stopFreecam()");
    assert_eq!(m, ["sink  look false", "view game"]);
}

#[test]
fn a_scriptable_camera_stays_where_it_is_put_and_custom_comes_back() {
    let mut game = world("iyc-scriptable");
    let mut st = start(&mut game);
    let (m, _) = lua(
        &mut st,
        &mut game,
        "local cam = workspace.CurrentCamera cam.CameraType = 'Scriptable' cam.CFrame = CFrame.new(0, 80, 0)",
    );
    assert_eq!(m, ["view at 0.00 80.00 0.00 180.0 0.0"]);
    // The player moves; the view stays.
    game.player.as_mut().unwrap().x = 30.0;
    assert!(!has(&frames(&mut st, &mut game, 0.2), "view"));
    let (_, p) = lua(&mut st, &mut game, "print(workspace.CurrentCamera.CFrame.Y)");
    assert_eq!(p, ["80"]);
    let (m, _) = lua(
        &mut st,
        &mut game,
        "workspace.CurrentCamera.CameraType = Enum.CameraType.Custom",
    );
    assert_eq!(m, ["view game"]);
}

#[test]
fn lookat_turns_the_player_where_the_camera_is_set_to_look() {
    let mut game = world("iyc-lookat");
    let mut st = start(&mut game);
    // Infinite Yield's lookat: the camera where it is, looking at Alex's head.
    let (m, _) = lua(
        &mut st,
        &mut game,
        "local cam = workspace.CurrentCamera cam.CFrame = CFrame.new(cam.CFrame.p, workspace.Alex.Head.CFrame.p)",
    );
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].starts_with("look -180.0 0.") || m[0].starts_with("look 180.0 0."),
        "{m:?}"
    );
}

#[test]
fn spectating_follows_another_player_and_viewpart_a_part() {
    let mut game = world("iyc-view");
    let mut st = start(&mut game);
    // view Alex: the camera's subject is their character.
    let (m, _) = lua(
        &mut st,
        &mut game,
        "workspace.CurrentCamera.CameraSubject = workspace.Alex",
    );
    assert_eq!(m, ["view entity 12"]);
    let cam = game.camera.unwrap();
    assert_eq!(cam.pos, [10.5, 65.62, -13.25]);
    // unview: back to the player's Humanoid.
    let (m, _) = lua(
        &mut st,
        &mut game,
        "workspace.CurrentCamera.CameraSubject = game.Players.LocalPlayer.Character:FindFirstChildWhichIsA('Humanoid')",
    );
    assert_eq!(m, ["view game"]);
    // viewpart: around a part of the place.
    let src = "local p = Instance.new('Part') p.Name = 'Spot' p.Anchored = true p.CFrame = CFrame.new(3, 70, 4) \
               p.Parent = workspace workspace.CurrentCamera.CameraSubject = p";
    let (m, _) = lua(&mut st, &mut game, src);
    assert_eq!(m, ["view around 3.00 70.00 4.00"]);
    // fixcam: the camera removed, a new one on the player.
    let (m, _) = lua(&mut st, &mut game, "workspace.CurrentCamera:remove()");
    assert_eq!(m, ["view game"]);
}

#[test]
fn camera_mode_and_zoom_set_the_perspective_and_the_distance() {
    let mut game = world("iyc-zoom");
    let mut st = start(&mut game);
    let lp = "local lp = game.Players.LocalPlayer ";
    // firstp, thirdp.
    let (m, _) = lua(&mut st, &mut game, &format!("{lp} lp.CameraMode = 'LockFirstPerson'"));
    assert_eq!(m, ["perspective first"]);
    // Locked: F5's third person goes back to first.
    game.third_person = true;
    assert_eq!(frames(&mut st, &mut game, 0.05), ["perspective first"]);
    let (m, _) = lua(&mut st, &mut game, &format!("{lp} lp.CameraMode = 'Classic'"));
    assert_eq!(m, ["perspective third"]);
    // minzoom 10: the view backs off to 10 blocks; maxzoom 2 brings it to 2.
    let (m, _) = lua(&mut st, &mut game, &format!("{lp} lp.CameraMinZoomDistance = '10'"));
    assert_eq!(m, ["distance Some(10.0)"]);
    let (m, _) = lua(&mut st, &mut game, &format!("{lp} lp.CameraMaxZoomDistance = 2"));
    assert_eq!(m, ["distance Some(2.0)"]);
    // camdistance 7: the limits on 7, then back; the zoom stays at 7.
    let src = format!(
        "{lp} lp.CameraMaxZoomDistance = 7 lp.CameraMinZoomDistance = 7 task.wait() \
         lp.CameraMaxZoomDistance = 400 lp.CameraMinZoomDistance = 0.5"
    );
    let (m, _) = lua(&mut st, &mut game, &src);
    assert_eq!(m, ["distance Some(7.0)"]);
    // lookat's 0.5: first person.
    let (m, _) = lua(&mut st, &mut game, &format!("{lp} lp.CameraMaxZoomDistance = 0.5"));
    assert_eq!(m, ["perspective first"]);
}

// Infinite Yield's click teleport and click delete, bound to Left Control
// and Left Alt held as the left button is pressed.
const CLICKS: &str = r#"
    local UIS = game:GetService("UserInputService")
    local mouse = game.Players.LocalPlayer:GetMouse()
    mouse.Button1Down:Connect(function()
        if UIS:IsKeyDown(Enum.KeyCode.LeftControl) then
            local root = game.Players.LocalPlayer.Character.HumanoidRootPart
            local hit = mouse.Hit.Position
            root.CFrame = CFrame.new(hit, Vector3.new(root.Position.X, hit.Y, root.Position.Z))
                * CFrame.Angles(0, math.pi, 0) + Vector3.new(0, 4, 0)
        elseif UIS:IsKeyDown(Enum.KeyCode.LeftAlt) then
            pcall(function() mouse.Target:Destroy() end)
        end
    end)
"#;

fn click(st: &mut Studio, game: &mut FakeGame, held: &str) -> Vec<String> {
    let m0 = game.moves.len();
    st.key(held, true);
    run_for(st, game, 0.05);
    st.key("key.mouse.left", true);
    run_for(st, game, 0.1);
    st.key("key.mouse.left", false);
    st.key(held, false);
    run_for(st, game, 0.1);
    game.moves[m0..].to_vec()
}

#[test]
fn ctrl_click_teleports_to_the_mouse_and_alt_click_deletes_its_block() {
    let mut game = world("iyc-click");
    let mut st = start(&mut game);
    lua(&mut st, &mut game, CLICKS);
    // Looking down onto the grass: the click lands on its top.
    game.player.as_mut().unwrap().pitch = 45.0;
    game.camera.as_mut().unwrap().pitch = 45.0;
    let m = click(&mut st, &mut game, "key.keyboard.left.control");
    let mv: Vec<&String> = m.iter().filter(|m| m.starts_with("move")).collect();
    assert_eq!(mv.len(), 1, "{m:?}");
    // 1.62 blocks ahead on the grass's top (y 64), the root part 4 above it.
    assert_eq!(
        mv[0],
        &format!("move 10.500 {:.3} -4.870", 64.0 + 4.0 - 18.0 * 0.9375 / 16.0)
    );
    assert!(game.sent.is_empty(), "no command: {:?}", game.sent);
    // Alt-click: the stone in front goes from the client's world.
    game.player.as_mut().unwrap().pitch = 0.0;
    let cam = game.camera.as_mut().unwrap();
    (cam.pos, cam.pitch) = ([10.5, 65.62, -3.25], 0.0);
    let m = click(&mut st, &mut game, "key.keyboard.left.alt");
    assert!(m.contains(&"block 10 65 -8 minecraft:air".into()), "{m:?}");
    assert_eq!(
        game.blocks.get(&(10, 65, -8)).map(String::as_str),
        Some("minecraft:air")
    );
}

#[test]
fn deleting_another_players_part_takes_them_out_of_the_client_s_world() {
    let mut game = world("iyc-delete");
    let mut st = start(&mut game);
    let (m, p) = lua(
        &mut st,
        &mut game,
        "print(pcall(function() workspace.Alex.Head:Destroy() end))",
    );
    assert_eq!(p, ["true"]);
    assert!(m.contains(&"hide 12".into()), "{m:?}");
    frames(&mut st, &mut game, 0.1);
    let (_, p) = lua(&mut st, &mut game, "print(workspace:FindFirstChild('Alex'))");
    assert_eq!(p, ["nil"]);
    // The Terrain itself, with no block under the mouse, can't go.
    game.camera.as_mut().unwrap().pitch = -90.0;
    let src = "local m = game.Players.LocalPlayer:GetMouse() local _ = m.Target \
               print(pcall(function() workspace.Terrain:Destroy() end))";
    let (_, p) = lua(&mut st, &mut game, src);
    assert!(p[0].starts_with("false"), "{p:?}");
}

#[test]
fn freeze_and_anchor_hold_the_player_still() {
    let mut game = world("iyc-anchor");
    let mut st = start(&mut game);
    let root = "game.Players.LocalPlayer.Character.HumanoidRootPart";
    let (m, _) = lua(&mut st, &mut game, &format!("{root}.Anchored = true"));
    assert_eq!(m, ["frozen true"]);
    let (m, _) = lua(&mut st, &mut game, &format!("{root}.Anchored = false"));
    assert_eq!(m, ["frozen false"]);
    // freeze: every part not anchored yet; thaw: all of them back.
    let each = |v: bool| {
        format!(
            "for _, x in game.Players.LocalPlayer.Character:GetDescendants() do \
             if x:IsA('BasePart') then x.Anchored = {v} end end"
        )
    };
    assert_eq!(lua(&mut st, &mut game, &each(true)).0, ["frozen true"]);
    assert_eq!(lua(&mut st, &mut game, &each(false)).0, ["frozen false"]);
}

#[test]
fn the_mouse_s_movement_reaches_scripts_at_their_sensitivity() {
    let mut game = world("iyc-mouse");
    let mut st = start(&mut game);
    let src = r#"
        local UIS = game:GetService("UserInputService")
        UIS.InputChanged:Connect(function(i)
            if i.UserInputType == Enum.UserInputType.MouseMovement then print("delta", i.Delta.X, i.Delta.Y) end
        end)
        game.Players.LocalPlayer:GetMouse().Move:Connect(function() print("move") end)
        UIS.MouseDeltaSensitivity = 2
    "#;
    let (m, _) = lua(&mut st, &mut game, src);
    assert_eq!(m, ["sensitivity 2.00"]);
    game.mouse = [3.0, -1.0];
    let l0 = log(&st).len();
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(log(&st)[l0..], ["delta\t6\t-2".replace('\t', "    "), "move".into()]);
}

#[test]
fn turning_the_character_turns_the_player_and_a_bare_position_does_not() {
    let mut game = world("iyc-turn");
    let mut st = start(&mut game);
    // stareat: the character turned on the spot to face east (+x).
    let src = "local c = game.Players.LocalPlayer.Character local p = c.PrimaryPart.Position \
               c:SetPrimaryPartCFrame(CFrame.new(p, p + Vector3.new(1, 0, 0)))";
    assert_eq!(lua(&mut st, &mut game, src).0, ["look -90.0 0.0"]);
    // tpposition: a place with no rotation moves the player, turning no one.
    let src = "game.Players.LocalPlayer.Character.HumanoidRootPart.CFrame = CFrame.new(1, 70, 1)";
    let m = lua(&mut st, &mut game, src).0;
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("move 1.000"), "{m:?}");
}

#[test]
fn moving_another_player_s_root_part_moves_them_in_the_client_s_world() {
    let mut game = world("iyc-bring");
    let mut st = start(&mut game);
    // clientbring: their root part three blocks east of ours, one up.
    let src = "local me = game.Players.LocalPlayer.Character.HumanoidRootPart \
               workspace.Alex.HumanoidRootPart.CFrame = me.CFrame + Vector3.new(3, 1, 0)";
    let (m, _) = lua(&mut st, &mut game, src);
    assert_eq!(m, ["entity 12 13.500 65.000 -3.250"]);
    assert_eq!(game.entities[0].pos, [13.5, 65.0, -3.25]);
}

#[test]
fn an_enlarged_see_through_hitbox_is_drawn_around_its_player() {
    let mut game = world("iyc-hitbox");
    let mut st = start(&mut game);
    lua(
        &mut st,
        &mut game,
        "local r = workspace.Alex.HumanoidRootPart r.Size = Vector3.new(5, 5, 5) r.Transparency = 0.4",
    );
    st.world(&mut game);
    let near = |p: &[f64; 3]| (p[0] - 10.5).abs() <= 2.5 + 1e-6 && (p[2] + 13.25).abs() <= 2.5 + 1e-6;
    let boxes = game.world_quads.iter().filter(|(c, _)| c.iter().all(near)).count();
    assert_eq!(boxes, 6, "{:?}", game.world_quads);
}

#[test]
fn walking_to_a_point_goes_with_the_agent_and_a_follow_loop_keeps_one_walk() {
    let mut game = world("iyc-walk");
    let mut st = start(&mut game);
    // walktoposition: WalkToPoint walks there, as MoveTo does.
    let hum = "game.Players.LocalPlayer.Character.Humanoid";
    lua(
        &mut st,
        &mut game,
        &format!("{hum}.WalkToPoint = Vector3.new(12, 64, -3)"),
    );
    assert_eq!(game.agent_calls, ["goto 12 64 -3 --near 1 --timeout 8"]);
    // walkto: MoveTo to the same place each frame is one walk, not a new one each time.
    let src = format!("for i = 1, 5 do {hum}:MoveTo(Vector3.new(12.5, 64, -2.5)) task.wait() end");
    lua(&mut st, &mut game, &src);
    assert_eq!(game.agent_calls, ["goto 12 64 -3 --near 1 --timeout 8"]);
}

#[test]
fn autoclick_and_autokeypress_press_the_game_s_own_button_and_key() {
    let mut game = world("iyc-press");
    let mut st = start(&mut game);
    let (m, _) = lua(
        &mut st,
        &mut game,
        "mouse1press() mouse1release() keypress(0x45) keyrelease(0x45)",
    );
    assert_eq!(
        m,
        [
            "press key.mouse.left true",
            "press key.mouse.left false",
            "press key.keyboard.e true",
            "press key.keyboard.e false"
        ]
    );
}

#[test]
fn xray_s_local_transparency_shows_through_parts() {
    let mut game = world("iyc-xray");
    let mut st = start(&mut game);
    let src = "local p = Instance.new('Part') p.Anchored = true p.CFrame = CFrame.new(10.5, 66, -6) p.Parent = workspace \
               p.LocalTransparencyModifier = 0.5";
    lua(&mut st, &mut game, src);
    st.world(&mut game);
    assert!(!game.world_quads.is_empty());
    assert!(
        game.world_quads.iter().all(|(_, argb)| argb >> 24 == 0x80),
        "{:?}",
        game.world_quads
    );
}

#[test]
fn hitboxes_shows_the_game_s_entity_hitboxes() {
    let mut game = world("iyc-hitboxes");
    let mut st = start(&mut game);
    let rs = "settings():GetService('RenderSettings').ShowBoundingBoxes";
    assert_eq!(lua(&mut st, &mut game, &format!("{rs} = true")).0, ["hitboxes true"]);
    assert_eq!(lua(&mut st, &mut game, &format!("{rs} = false")).0, ["hitboxes false"]);
}
