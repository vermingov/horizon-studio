// ESP and aim hubs (Unnamed ESP, OwlHub's universal aimbot and ESP), as
// the Roblox and executor calls they make: Drawing boxes, tracers and tags
// placed from the Camera's WorldToViewportPoint, a menu drawn with the
// Drawing library and clicked through UserInputService, team checks on the
// scoreboard's teams, mousemoverel aiming and Mouse hooks.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, PlayerEntry, Studio};
use horizon_studio::game::hstudio_ui::{Input, handle_screen};

/// Steve (playing, looking north) with Alex ten blocks ahead and Bob
/// farther, to the right.
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
        entry("Bob", 0x43, Some(14), false),
    ]);
    let body = |id: u32, name: &str, x: f64, z: f64, health: f32| EntityInfo {
        id,
        kind: "minecraft:player".into(),
        name: name.into(),
        pos: [x, 64.0, z],
        width: 0.6,
        height: 1.8,
        eye: 1.62,
        health,
        max_health: 20.0,
        speed: 0.1,
        on_ground: true,
        ..EntityInfo::default()
    };
    game.entities = vec![body(12, "Alex", 10.5, -13.25, 14.0), body(14, "Bob", 16.0, -25.0, 20.0)];
    let p = game.player.as_mut().unwrap();
    p.name = "Steve".into();
    p.uuid = 1;
    game
}

fn start(game: &mut FakeGame) -> Studio {
    let mut st = studio(game);
    run_for(&mut st, game, 0.1);
    lua(&mut st, game, "local _ = game.Players:GetPlayers()");
    run_for(&mut st, game, 0.6);
    st
}

/// Runs console code: what it printed.
fn lua(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    let before = log(st).len();
    st.run_console(game, src);
    run_for(st, game, 0.1);
    log(st)[before..]
        .iter()
        .filter(|l| !l.starts_with("> ") && !src.lines().any(|s| s.trim() == l.trim()))
        .cloned()
        .collect()
}

#[test]
fn a_drawing_s_transparency_is_its_opacity() {
    // Unnamed ESP: "Transparency is really Opacity with drawing api (1
    // being visible, 0 being invisible)"; its box outline is 0.75.
    let mut game = world("esp-opacity");
    let mut st = start(&mut game);
    lua(
        &mut st,
        &mut game,
        r#"
        local function line(y, t)
            local l = Drawing.new("Line")
            l.From, l.To = Vector2.new(0, y), Vector2.new(100, y)
            l.Color = Color3.new(1, 0, 0)
            if t then l.Transparency = t end
            l.Visible = true
            print(l.Transparency)
        end
        line(10) line(20, 0.75) line(30, 0)
        "#,
    );
    game.quads.clear();
    st.hud(&mut game);
    let alphas: Vec<u32> = game.quads.iter().map(|q| q.1 >> 24).collect();
    assert_eq!(alphas, [0xFF, 0xBF], "the invisible one isn't drawn");
}

#[test]
fn distance_tags_format_fractions_as_luau_does() {
    let mut game = world("esp-format");
    let mut st = start(&mut game);
    let p = lua(
        &mut st,
        &mut game,
        "print(string.format('[%d] [%d/%d] [%s%%]', 21.75, 70, 100, math.floor(70 / 100 * 100)))",
    );
    assert_eq!(p, ["[21] [70/100] [70%]"]);
}

#[test]
fn boxes_and_name_tags_land_on_the_player() {
    // Unnamed ESP's box corners and name tag: the root part's CFrame (and
    // GetRenderCFrame) offset in its own space, through a WorldToViewportPoint
    // taken off the Camera; a tracer from the bottom middle.
    let mut game = world("esp-box");
    let mut st = start(&mut game);
    let p = lua(
        &mut st,
        &mut game,
        r#"
        local Camera = workspace.CurrentCamera
        local WTVP = Camera.WorldToViewportPoint
        local c = game.Players.Alex.Character
        local root, head = c.HumanoidRootPart, c.Head
        local function at(cf) local v, on = WTVP(Camera, cf.Position) return v, on end
        print(root:GetRenderCFrame() == root.CFrame)
        local tag, on = at(root:GetRenderCFrame() * CFrame.new(0, head.Size.Y + root.Size.Y, 0))
        local h = WTVP(Camera, head.Position)
        local size = Vector3.new(2, 3, 1) * head.Size.Y
        local tl = at(root.CFrame * CFrame.new(size.X, size.Y, 0))
        local br = at(root.CFrame * CFrame.new(-size.X, -size.Y, 0))
        print(math.round(tag.X), on, tag.Y < h.Y, math.round(h.X), math.round(h.Y), math.round(h.Z))
        print(math.round(tl.X + br.X), tl.Y < h.Y, br.Y > h.Y)
        "#,
    );
    assert_eq!(p[0], "true");
    // Ten blocks ahead at eye level: the middle of the 960 x 540 dp view.
    assert_eq!(p[1], "480    true    true    480    270    10");
    assert_eq!(p[2], "960    true    true", "the box is centred on the player");
}

#[test]
fn a_drawing_menu_takes_the_pointer_and_its_clicks() {
    // A menu of Drawings, clicked as Unnamed ESP's is: InputEnded's
    // MouseButton1 over a Square, by GetMouseLocation.
    let mut game = world("esp-menu");
    let mut st = start(&mut game);
    lua(
        &mut st,
        &mut game,
        r#"
        local UIS = game:GetService("UserInputService")
        local button = Drawing.new("Square")
        button.Position, button.Size = Vector2.new(100, 100), Vector2.new(200, 40)
        button.Filled, button.Visible = true, true
        _G.on = false
        UIS.InputEnded:Connect(function(input)
            if input.UserInputType.Name ~= "MouseButton1" then return end
            local m = UIS:GetMouseLocation()
            local p, s = button.Position, button.Size
            if m.X >= p.X and m.Y >= p.Y and m.X <= p.X + s.X and m.Y <= p.Y + s.Y then
                _G.on = not _G.on
            end
        end)
        "#,
    );
    // Playing, the pointer is the game's: nothing asks for it.
    assert!(st.s.screen_request.is_none());
    st.key("key.keyboard.left.alt", true);
    run_for(&mut st, &mut game, 0.05);
    st.key("key.keyboard.left.alt", false);
    run_for(&mut st, &mut game, 0.1);
    assert!(
        st.s.screen_request.is_some(),
        "Left Alt frees the pointer over Drawings"
    );
    assert!(st.accept_screen());
    for (x, y) in [(150.0, 120.0), (500.0, 300.0)] {
        handle_screen(&mut st, &mut game, Input::Mouse(x, y));
        handle_screen(&mut st, &mut game, Input::Button { left: true, down: true });
        handle_screen(
            &mut st,
            &mut game,
            Input::Button {
                left: true,
                down: false,
            },
        );
        run_for(&mut st, &mut game, 0.1);
    }
    let p = lua(
        &mut st,
        &mut game,
        "print(_G.on, game:GetService('UserInputService'):GetMouseLocation())",
    );
    assert_eq!(
        p,
        ["true    500, 300"],
        "the click on it toggled, the one beside it didn't"
    );
}

#[test]
fn scoreboard_teams_are_roblox_teams() {
    let mut game = world("esp-teams");
    game.teams.insert("Steve".into(), ("red".into(), Some(0xFF5555)));
    game.teams.insert("Alex".into(), ("red".into(), Some(0xFF5555)));
    game.teams.insert("Bob".into(), ("blue".into(), Some(0x5555FF)));
    let mut st = start(&mut game);
    let src = r#"
        local P = game:GetService("Players")
        local me = P.LocalPlayer
        for _, p in ipairs(P:GetPlayers()) do
            print(p.Name, p.Team and p.Team.Name, p.Neutral, p.TeamColor == me.TeamColor, p.TeamColor.Name)
        end
        for _, t in ipairs(game:GetService("Teams"):GetTeams()) do
            print(t.Name, t.TeamColor.Name, #t:GetPlayers())
        end
    "#;
    let p = lua(&mut st, &mut game, src);
    assert_eq!(
        p,
        [
            "Steve    red    false    true    Persimmon",
            "Alex    red    false    true    Persimmon",
            "Bob    blue    false    false    Royal purple",
            "red    Persimmon    2",
            "blue    Royal purple    1",
        ]
    );
    // Bob leaves his team: he is neutral, and the team no one is on goes.
    game.teams.remove("Bob");
    run_for(&mut st, &mut game, 0.6);
    let p = lua(&mut st, &mut game, src);
    assert_eq!(p[2], "Bob    nil    true    false    Royal purple");
    assert_eq!(p.len(), 4, "{p:?}");
}

#[test]
fn mousemoverel_brings_a_screen_point_to_the_crosshair() {
    // An aimbot's step: mousemoverel by its target's screen point less the
    // middle turns the view so the target comes to the crosshair.
    let mut game = world("esp-aim");
    for (yaw, pitch) in [(176.0, 3.0), (186.0, -8.0)] {
        game.player.as_mut().unwrap().yaw = yaw;
        game.player.as_mut().unwrap().pitch = pitch;
        let cam = game.camera.as_mut().unwrap();
        // A wide view (Quake Pro): a pixel is more than the 70 degrees' 0.15.
        (cam.yaw, cam.pitch, cam.fov) = (yaw, pitch, 110.0);
        let mut st = start(&mut game);
        let m0 = game.moves.len();
        lua(
            &mut st,
            &mut game,
            r#"
            local cam = workspace.CurrentCamera
            local v = cam:WorldToViewportPoint(game.Players.Alex.Character.Head.Position)
            mousemoverel(v.X - cam.ViewportSize.X / 2, v.Y - cam.ViewportSize.Y / 2)
            "#,
        );
        let turns: Vec<(f32, f32)> = game.moves[m0..]
            .iter()
            .filter_map(|m| m.strip_prefix("turn "))
            .map(|t| {
                let v: Vec<f32> = t.split(' ').map(|n| n.parse().unwrap()).collect();
                (v[0], v[1])
            })
            .collect();
        assert_eq!(turns.len(), 1, "{:?}", &game.moves[m0..]);
        // Alex's eyes are level with Steve's, due north (yaw 180).
        let (dyaw, dpitch) = turns[0];
        assert!((yaw + dyaw - 180.0).abs() < 0.2, "{yaw} + {dyaw}");
        assert!((pitch + dpitch).abs() < 0.2, "{pitch} + {dpitch}");
    }
}

#[test]
fn a_silent_aim_s_mouse_hooks_and_a_camera_aim() {
    // Silent aim: an __index hook on the Mouse's Hit and Target that scripts
    // reading them see; camera aim: the Camera's CFrame turned to a head
    // turns the player.
    let mut game = world("esp-silent");
    let mut st = start(&mut game);
    let p = lua(
        &mut st,
        &mut game,
        r#"
        local mouse = game.Players.LocalPlayer:GetMouse()
        local bob = game.Players.Bob.Character.Head
        local old
        old = hookmetamethod(game, "__index", function(self, k)
            if self == mouse and (k == "Hit" or k == "Target") then
                return k == "Hit" and bob.CFrame or bob
            end
            return old(self, k)
        end)
        print(mouse.Target == bob, mouse.Hit.Position == bob.Position, mouse.X)
        "#,
    );
    assert_eq!(p, ["true    true    480"]);
    let m0 = game.moves.len();
    lua(
        &mut st,
        &mut game,
        r#"
        local cam = workspace.CurrentCamera
        cam.CFrame = CFrame.lookAt(cam.CFrame.Position, game.Players.Bob.Character.Head.Position)
        "#,
    );
    let look = game.moves[m0..].iter().find(|m| m.starts_with("look ")).cloned();
    // Bob is 5.5 east and 21.75 north: Minecraft's yaw 180 + 14.2, as -165.8.
    assert_eq!(look.as_deref(), Some("look -165.8 0.0"), "{:?}", &game.moves[m0..]);
}

#[test]
fn get_objects_says_why_it_can_t_load_a_roblox_asset() {
    // OwlHub's UI library is Roblox model assets, which need a Roblox
    // account: one clear error, not a nil to index.
    let mut game = world("esp-objects");
    let mut st = start(&mut game);
    let p = lua(
        &mut st,
        &mut game,
        "print(pcall(function() return game:GetObjects('rbxassetid://4743303040')[1] end))",
    );
    assert_eq!(p.len(), 1);
    assert!(p[0].starts_with("false    "), "{p:?}");
    assert!(p[0].contains("signed-in Roblox account"), "{p:?}");
}
