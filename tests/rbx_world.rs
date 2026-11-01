// The Roblox layer's world on a fake game (hstudio_rbx_world): the
// LocalPlayer is the player playing, and its Character an R6 rig that
// follows them; the entities near them are mirrored in the Workspace once a
// script looks.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, ModState, PlayerEntry, PlayerPose, Studio};
use horizon_studio::headless::{Run, Source, run_steps};
use serde_json::json;

fn printed(st: &Studio, id: &str) -> Vec<String> {
    let prefix = format!("[{id}] ");
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

/// Loads `src` as mod `id` and lets it run `seconds`.
fn load(game: &mut FakeGame, st: &mut Studio, id: &str, src: &str, seconds: f64) {
    game.write_mod(&format!("{id}.lua"), src);
    run_for(st, game, seconds);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}\n{:?}", log(st));
    }
}

/// `v`, which the scripts below print vectors with: three decimals, and no
/// sign on a zero.
const FMT: &str = r#"
    local function n(x) return (string.format("%.3f", x):gsub("^%-(0%.000)$", "%1")) end
    local function v(x) return n(x.X) .. " " .. n(x.Y) .. " " .. n(x.Z) end
"#;

#[test]
fn the_character_is_an_r6_rig_on_the_player() {
    let mut game = FakeGame::new("rbxw-rig");
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local char = game.Players.LocalPlayer.Character
        local root, head, hum = char.HumanoidRootPart, char.Head, char.Humanoid
        print(char.PrimaryPart == root, hum.RootPart == root, hum.RigType, hum.HipHeight)
        print(v(root.Size), v(head.Size))
        print(v(root.Position), v(root.CFrame.LookVector))
        print(v(head.Position), v(head.CFrame.LookVector))
        print(root.Transparency, root.Anchored, root.CanCollide, root.CanTouch, root.Massless, head.CanCollide)
        print(pcall(function() char.Parent = nil end))
        print(pcall(function() root:Destroy() end))
        local tag = Instance.new("Folder")
        tag.Parent = char
        print(tag.Parent == char)
    "#
    );
    load(&mut game, &mut st, "rig", &src, 0.7);
    let p = printed(&st, "rig");
    assert_eq!(p[0], "true    true    Enum.HumanoidRigType.R6    0.703125");
    // The torso's box (8 x 12 x 4 pixels) and the head's cube, 15/16 of a block-sized model.
    assert_eq!(p[1], "0.469 0.703 0.234    0.469 0.469 0.469");
    // At the torso's centre over the feet (10.5, 64, -3.25), facing north (yaw 180).
    assert_eq!(p[2], "10.500 65.055 -3.250    0.000 0.000 -1.000");
    assert_eq!(p[3], "10.500 65.620 -3.250    0.000 0.000 -1.000");
    assert_eq!(p[4], "1    true    false    false    true    false");
    assert!(p[5].starts_with("false") && p[5].contains("locked"), "{}", p[5]);
    assert!(p[6].starts_with("false") && p[6].contains("locked"), "{}", p[6]);
    assert_eq!(p[7], "true");
}

#[test]
fn the_local_player_takes_the_players_name_and_uuid() {
    let mut game = FakeGame::new("rbxw-name");
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        local me = Players.LocalPlayer
        print(me.Name, me.UserId, me:GetAttribute("UUID"), me.Character.Name)
        local renamed = 0
        me:GetPropertyChangedSignal("Name"):Connect(function() renamed += 1 end)
        task.spawn(function()
            local char = workspace:WaitForChild("Steve")
            print("found", char == me.Character, char.Humanoid.DisplayName)
        end)
        command("who", function()
            print(me.Name, me.DisplayName, me.Character.Name, me.Character.Humanoid.DisplayName)
            print(me.UserId, me:GetAttribute("UUID"), renamed, Players:FindFirstChild("Steve") == me)
        end)
    "#;
    load(&mut game, &mut st, "who", src, 0.7);
    // Until the game says who the player is, the LocalPlayer is Player1.
    let p = game.player.as_mut().unwrap();
    p.name = "Steve".into();
    p.uuid = 0x8667ba71_b85a_4004_af54_457a9734eed7;
    run_for(&mut st, &mut game, 0.2);
    assert!(st.command("who"));
    run_for(&mut st, &mut game, 0.1);
    // A name the game no longer says keeps the last one.
    game.player.as_mut().unwrap().name.clear();
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("who"));
    run_for(&mut st, &mut game, 0.1);
    let p = printed(&st, "who");
    assert_eq!(p[0], "Player1    1    nil    Player1");
    assert_eq!(p[1], "found    true    Steve");
    assert_eq!(p[2], "Steve    Steve    Steve    Steve");
    // The UUID's low 49 bits, printed in full.
    assert_eq!(
        p[3],
        "76392825155287    8667ba71-b85a-4004-af54-457a9734eed7    1    true"
    );
    assert_eq!(p[4..], ["Steve    Steve    Steve    Steve", p[3].as_str()]);
}

#[test]
fn game_local_player_and_workspace_are_aliases() {
    let mut game = FakeGame::new("rbxw-alias");
    let mut st = studio(&mut game);
    let src = r#"
        print(game.LocalPlayer == game.Players.LocalPlayer, game.workspace == workspace)
        print(game.Players.localPlayer == game.Players.LocalPlayer, game.LocalPlayer.Name)
        print(pcall(function() game.LocalPlayer = nil end))
        print(pcall(function() game.workspace = nil end))
        print(pcall(function() game.Players.localPlayer = nil end))
        print(pcall(function() return workspace.LocalPlayer end))
    "#;
    load(&mut game, &mut st, "alias", src, 0.7);
    let p = printed(&st, "alias");
    assert_eq!(p[0], "true    true");
    assert_eq!(p[1], "true    Player1");
    for line in &p[2..5] {
        assert!(
            line.starts_with("false") && line.contains("Property is read only"),
            "{line}"
        );
    }
    // Only game has them.
    assert!(
        p[5].starts_with("false") && p[5].contains("LocalPlayer is not a valid member of Workspace"),
        "{}",
        p[5]
    );
}

#[test]
fn a_headless_test_names_the_player() {
    let source = Source::Files {
        id: "hello".into(),
        files: vec![("hello.lua".into(), "local me = game.Players.LocalPlayer\n".into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let lua = "print(game.Players.LocalPlayer.Name, game.LocalPlayer.UserId, game.LocalPlayer:GetAttribute('UUID'))";
    let report = run_steps(
        &mut run,
        &[
            json!({ "lua": lua, "expect_output": "Player1    1    nil" }),
            json!({ "player": { "name": "Steve", "uuid": "8667ba71-b85a-4004-af54-457a9734eed7" } }),
            json!({ "frames": 1 }),
            json!({ "lua": lua, "expect_output": "Steve    76392825155287    8667ba71-b85a-4004-af54-457a9734eed7" }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
    // How the player moves, which the Humanoid follows.
    let lua = "local h = game.LocalPlayer.Character.Humanoid print(h:GetState().Name, h.MoveDirection.X)";
    let report = run_steps(
        &mut run,
        &[
            json!({ "lua": lua, "expect_output": "Running    0" }),
            json!({ "player": { "velocity": [3, 6, 0], "on_ground": false } }),
            json!({ "lua": lua, "expect_output": "Jumping    1" }),
            json!({ "player": { "velocity": [0, 0, 0], "on_ground": true } }),
            json!({ "frames": 2 }),
            json!({ "lua": lua, "expect_output": "Running    0" }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
}

/// Runs a mod's command and a few frames after (as the sends refill): what
/// the game was sent.
fn sent_by(st: &mut Studio, game: &mut FakeGame, cmd: &str) -> Vec<String> {
    let mark = game.sent.len();
    assert!(st.command(cmd), "no command {cmd}");
    run_for(st, game, 0.3);
    game.sent[mark..].to_vec()
}

#[test]
fn moving_the_root_part_teleports_with_tp() {
    let mut game = FakeGame::new("rbxw-tp");
    let mut st = studio(&mut game);
    let src = r#"
        local char = game.Players.LocalPlayer.Character
        local root = char.HumanoidRootPart
        command("set", function() root.CFrame = CFrame.new(1, 70, 2) end)
        command("pos", function() root.Position = Vector3.new(3, 71, 4) end)
        command("pivot", function() char:PivotTo(CFrame.new(5, 72, 6)) end)
        command("primary", function() char:SetPrimaryPartCFrame(CFrame.new(7, 73, 8)) end)
        command("moveto", function() char:MoveTo(Vector3.new(9, 74, 10)) end)
        command("onto", function() char:MoveTo(Vector3.new(9.5, 64, 10.5)) end)
        command("pillar", function() char:MoveTo(Vector3.new(20.5, 64.5, 20.5)) end)
        command("into", function() char:PivotTo(CFrame.new(9.5, 64, 10.5)) end)
        command("partpivot", function() root:PivotTo(CFrame.new(11, 75, 12)) end)
        command("twice", function()
            root.CFrame = CFrame.new(0, 80, 0)
            root.CFrame = CFrame.new(1, 81, 1)
        end)
        command("stay", function() char:PivotTo(char:GetPivot()) end)
        command("where", function() print(string.format("%.3f", root.Position.Y)) end)
        command("others", function()
            local m = Instance.new("Model", workspace)
            local p = Instance.new("Part", m)
            m.PrimaryPart = p
            m:PivotTo(CFrame.new(50, 50, 50))
            p.CFrame = CFrame.new(1, 2, 3)
            char.Head.CFrame = CFrame.new(1, 2, 3)
        end)
    "#;
    load(&mut game, &mut st, "tp", src, 0.7);
    // The feet go where the root part's centre is, less the 1.05 blocks it stands above them.
    assert_eq!(sent_by(&mut st, &mut game, "set"), ["/tp @s 1.000 68.945 2.000"]);
    assert_eq!(sent_by(&mut st, &mut game, "pos"), ["/tp @s 3.000 69.945 4.000"]);
    assert_eq!(sent_by(&mut st, &mut game, "pivot"), ["/tp @s 5.000 70.945 6.000"]);
    assert_eq!(sent_by(&mut st, &mut game, "primary"), ["/tp @s 7.000 71.945 8.000"]);
    assert_eq!(sent_by(&mut st, &mut game, "moveto"), ["/tp @s 9.000 72.945 10.000"]);
    // MoveTo moves the character up out of the blocks in its way, as
    // Roblox's does: a point on a stone block's top (the mouse's Hit there)
    // puts the feet on the block, and one by a pillar on its top. PivotTo
    // puts it exactly where it is asked, in the block.
    game.blocks.insert((9, 63, 10), "minecraft:stone".into());
    game.blocks.insert((20, 64, 20), "minecraft:stone".into());
    game.blocks.insert((20, 65, 20), "minecraft:stone".into());
    assert_eq!(sent_by(&mut st, &mut game, "onto"), ["/tp @s 9.500 64.000 10.500"]);
    assert_eq!(sent_by(&mut st, &mut game, "pillar"), ["/tp @s 20.500 66.000 20.500"]);
    assert_eq!(sent_by(&mut st, &mut game, "into"), ["/tp @s 9.500 62.945 10.500"]);
    assert_eq!(
        sent_by(&mut st, &mut game, "partpivot"),
        ["/tp @s 11.000 73.945 12.000"]
    );
    // One teleport a frame: the last move's.
    assert_eq!(sent_by(&mut st, &mut game, "twice"), ["/tp @s 1.000 79.945 1.000"]);
    // Moving the character where it is sends the player where they are.
    assert_eq!(sent_by(&mut st, &mut game, "stay"), ["/tp @s 10.500 64.000 -3.250"]);
    // Crouching, the root part is lower, and the teleport still lands the feet where they are.
    game.pose = Some(PlayerPose {
        pose: "crouching".into(),
        on_ground: true,
        ..PlayerPose::default()
    });
    run_for(&mut st, &mut game, 0.05);
    assert!(sent_by(&mut st, &mut game, "where").is_empty());
    assert_eq!(sent_by(&mut st, &mut game, "stay"), ["/tp @s 10.500 64.000 -3.250"]);
    // A script's own parts, and the head, move nothing.
    assert!(sent_by(&mut st, &mut game, "others").is_empty());
    assert_eq!(printed(&st, "tp"), ["64.879"]);
    // Never a chat line.
    assert!(game.sent.iter().all(|l| l.starts_with("/tp @s ")), "{:?}", game.sent);
}

#[test]
fn a_tween_of_the_root_part_teleports_and_a_teleport_waits_for_a_send() {
    let mut game = FakeGame::new("rbxw-tween");
    let mut st = studio(&mut game);
    let src = r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        local TweenService = game:GetService("TweenService")
        command("glide", function()
            TweenService:Create(root, TweenInfo.new(0.5, Enum.EasingStyle.Linear), { CFrame = CFrame.new(20, 70, 20) }):Play()
        end)
        command("busy", function()
            for i = 1, 10 do pcall(chat.say, "hi " .. i) end
            root.CFrame = CFrame.new(1, 70, 2)
        end)
        command("again", function() root.CFrame = CFrame.new(3, 70, 4) end)
    "#;
    load(&mut game, &mut st, "tween", src, 0.7);
    let mark = game.sent.len();
    assert!(st.command("glide"));
    run_for(&mut st, &mut game, 1.5);
    let tps = &game.sent[mark..];
    assert!(tps.len() >= 2, "{tps:?}");
    assert!(tps.iter().all(|l| l.starts_with("/tp @s ")), "{tps:?}");
    assert_eq!(tps.last().unwrap(), "/tp @s 20.000 68.945 20.000");
    // A teleport takes only what chat.say and game.run leave: after a while
    // with nothing sent, one goes though chat used every send of its burst,
    run_for(&mut st, &mut game, 3.0);
    let mark = game.sent.len();
    assert!(st.command("busy"));
    run_for(&mut st, &mut game, 0.3);
    let sent = &game.sent[mark..];
    assert_eq!(sent.iter().filter(|l| l.starts_with("hi ")).count(), 8, "{sent:?}");
    assert_eq!(sent.last().unwrap(), "/tp @s 1.000 68.945 2.000");
    // and the next waits until that budget is whole again (two seconds).
    let mark = game.sent.len();
    assert!(st.command("again"));
    run_for(&mut st, &mut game, 1.5);
    assert!(game.sent[mark..].is_empty(), "{:?}", &game.sent[mark..]);
    run_for(&mut st, &mut game, 1.0);
    assert_eq!(game.sent[mark..], ["/tp @s 3.000 68.945 4.000"]);
}

#[test]
fn a_script_teleporting_every_frame_leaves_the_mods_their_burst() {
    // A fly script moves the root part every frame, so a teleport is always
    // waiting, while another mod's timer says four lines at a time and its
    // command a whole burst. Every line goes (the mod would stop at one that
    // couldn't), and the teleports go on with what the mods leave.
    let mut game = FakeGame::new("rbxw-tp-budget");
    let mut st = studio(&mut game);
    let fly = r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        game:GetService("RunService").RenderStepped:Connect(function(dt)
            root.CFrame = root.CFrame + Vector3.new(0, 0, dt)
        end)
    "#;
    load(&mut game, &mut st, "fly", fly, 0.7);
    let talk = r#"
        command("talk", function() for i = 1, 8 do chat.say("line " .. i) end end)
        every(5, function() for i = 1, 4 do chat.say("tick " .. i) end end)
    "#;
    load(&mut game, &mut st, "talk", talk, 0.6);
    // The timer's first four, then the burst two seconds after them, then
    // the timer's next four.
    let mark = game.sent.len();
    run_for(&mut st, &mut game, 7.0);
    let i = mod_index(&st, "talk");
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    assert!(st.command("talk"));
    run_for(&mut st, &mut game, 3.2);
    assert!(st.s.mods[i].running(), "{:?}", st.s.mods[i].state);
    let sent = &game.sent[mark..];
    let count = |what: &str| sent.iter().filter(|l| l.starts_with(what)).count();
    assert_eq!((count("line "), count("tick ")), (8, 8), "{sent:?}");
    assert!(count("/tp @s ") >= 20, "{sent:?}");
}

/// The player's pose on the fake game: standing, on the ground or not.
fn pose(on_ground: bool) -> Option<PlayerPose> {
    Some(PlayerPose {
        pose: "standing".into(),
        on_ground,
        ..PlayerPose::default()
    })
}

#[test]
fn the_humanoid_follows_the_game() {
    let mut game = FakeGame::new("rbxw-humanoid");
    game.blocks.insert((10, 63, -4), "minecraft:grass_block".into());
    game.pose = pose(true);
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local char = game.Players.LocalPlayer.Character
        local hum, root = char.Humanoid, char.HumanoidRootPart
        local function r(x) return string.format("%.4f", x) end
        local states, jumps, runs, turns = {}, {}, {}, 0
        hum.StateChanged:Connect(function(old, new) table.insert(states, old.Name .. ">" .. new.Name) end)
        hum.Jumping:Connect(function(on) table.insert(jumps, tostring(on)) end)
        hum.Running:Connect(function(speed) table.insert(runs, r(speed)) end)
        hum:GetPropertyChangedSignal("MoveDirection"):Connect(function() turns += 1 end)
        command("speeds", function()
            print(r(hum.WalkSpeed), r(hum.JumpPower), r(hum.JumpHeight), workspace.Gravity, hum.MaxHealth)
        end)
        command("moving", function()
            print(v(hum.MoveDirection), v(root.AssemblyLinearVelocity), hum:GetState().Name, hum.FloorMaterial.Name)
        end)
        command("events", function()
            print(table.concat(states, " "), table.concat(jumps, " "), table.concat(runs, " "), turns)
        end)
        command("fast", function() hum.WalkSpeed = 50 end)
        command("state", function() print(hum:GetState().Name) end)
    "#
    );
    load(&mut game, &mut st, "hum", &src, 0.7);
    let say = |st: &mut Studio, game: &mut FakeGame, cmd: &str| {
        assert!(st.command(cmd));
        run_for(st, game, 0.05);
    };
    // The game's walking speed and jump, in blocks.
    say(&mut st, &mut game, "speeds");
    say(&mut st, &mut game, "moving");
    // A script's own WalkSpeed stays until the game's speed changes (it sprints).
    say(&mut st, &mut game, "fast");
    say(&mut st, &mut game, "speeds");
    game.player.as_mut().unwrap().speed = Some(0.13);
    run_for(&mut st, &mut game, 0.05);
    say(&mut st, &mut game, "speeds");
    // A speed the game can't read now leaves it alone.
    game.player.as_mut().unwrap().speed = None;
    run_for(&mut st, &mut game, 0.05);
    say(&mut st, &mut game, "speeds");
    // Walking east, then stopping.
    game.player.as_mut().unwrap().velocity = [4.3, 0.0, 0.0];
    run_for(&mut st, &mut game, 0.1);
    say(&mut st, &mut game, "moving");
    game.player.as_mut().unwrap().velocity = [0.0; 3];
    run_for(&mut st, &mut game, 0.1);
    // A jump: rising, falling, landing.
    game.pose = pose(false);
    game.player.as_mut().unwrap().velocity = [0.0, 8.0, 0.0];
    run_for(&mut st, &mut game, 0.1);
    say(&mut st, &mut game, "moving");
    game.player.as_mut().unwrap().velocity = [0.0, -5.0, 0.0];
    run_for(&mut st, &mut game, 0.1);
    game.pose = pose(true);
    game.player.as_mut().unwrap().velocity = [0.0; 3];
    run_for(&mut st, &mut game, 0.1);
    say(&mut st, &mut game, "moving");
    say(&mut st, &mut game, "events");
    // Swimming, flying (creative flight, then an elytra), asleep in a bed, and
    // back on the ground.
    for (pose, flying) in [
        ("swimming", false),
        ("standing", true),
        ("gliding", false),
        ("sleeping", false),
    ] {
        game.pose = Some(PlayerPose {
            pose: pose.into(),
            flying,
            ..PlayerPose::default()
        });
        run_for(&mut st, &mut game, 0.1);
        say(&mut st, &mut game, "state");
    }
    game.pose = pose(true);
    run_for(&mut st, &mut game, 0.1);
    say(&mut st, &mut game, "events");
    let p = printed(&st, "hum");
    assert_eq!(p[0], "4.3178    8.4000    1.2522    32    100");
    assert_eq!(p[1], "0.000 0.000 0.000    0.000 0.000 0.000    Running    Grass");
    assert_eq!(p[2], "50.0000    8.4000    1.2522    32    100");
    assert_eq!(p[3], "5.6131    8.4000    1.2522    32    100");
    assert_eq!(p[4], p[3]);
    assert_eq!(p[5], "1.000 0.000 0.000    4.300 0.000 0.000    Running    Grass");
    assert_eq!(p[6], "0.000 0.000 0.000    0.000 8.000 0.000    Jumping    Air");
    assert_eq!(p[7], "0.000 0.000 0.000    0.000 0.000 0.000    Running    Grass");
    assert_eq!(
        p[8],
        "Running>Jumping Jumping>Freefall Freefall>Landed Landed>Running    true false    4.3000 0.0000    2"
    );
    assert_eq!(p[9..13], ["Swimming", "Flying", "Flying", "Seated"]);
    assert_eq!(
        p[13],
        "Running>Jumping Jumping>Freefall Freefall>Landed Landed>Running Running>Swimming Swimming>Flying Flying>Seated Seated>Running    true false    4.3000 0.0000    2"
    );
}

#[test]
fn respawning_makes_a_new_character() {
    let mut game = FakeGame::new("rbxw-respawn");
    let mut st = studio(&mut game);
    let src = r#"
        local me = game.Players.LocalPlayer
        local first = me.Character
        local log = {}
        first.Humanoid.Died:Connect(function() table.insert(log, "died") end)
        me.CharacterRemoving:Connect(function(c) table.insert(log, "removing " .. tostring(c == first)) end)
        me.CharacterAdded:Connect(function(c)
            table.insert(log, "added " .. tostring(c == me.Character) .. " " .. tostring(c.Parent == workspace))
        end)
        me.CharacterAppearanceLoaded:Connect(function(c) table.insert(log, "loaded " .. c.Name) end)
        command("state", function() print(me.Character.Humanoid:GetState().Name, me.Character == first) end)
        command("check", function()
            local now = me.Character
            print(table.concat(log, ", "))
            print(now ~= first, first.Parent, now.Parent == workspace, workspace.CurrentCamera.CameraSubject == now.Humanoid)
            print(now.Humanoid.Health, now.Humanoid.WalkSpeed, now.Humanoid:GetState().Name, now.PrimaryPart.Name)
        end)
    "#;
    load(&mut game, &mut st, "life", src, 0.7);
    // Dying: Died fires once, however long the player stays dead.
    game.player.as_mut().unwrap().health = 0.0;
    run_for(&mut st, &mut game, 0.3);
    assert!(st.command("state"));
    run_for(&mut st, &mut game, 0.05);
    // Back to life, a new character.
    game.player.as_mut().unwrap().health = 20.0;
    run_for(&mut st, &mut game, 0.2);
    // Out of a world, the character stays as it is.
    let saved = game.player.take();
    run_for(&mut st, &mut game, 0.2);
    game.player = saved;
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "life");
    assert_eq!(p[0], "Dead    true");
    assert_eq!(p[1], "died, removing true, added true true, loaded Player1");
    assert_eq!(p[2], "true    nil    true    true");
    assert_eq!(p[3], "100    4.3178    Running    HumanoidRootPart");
}

#[test]
fn humanoid_move_to_walks_only_you_and_says_when_it_is_done() {
    let mut game = FakeGame::new("rbxw-moveto");
    let mut st = studio(&mut game);
    let src = r#"
        local hum = game.Players.LocalPlayer.Character.Humanoid
        local npc = Instance.new("Humanoid")
        local log = {}
        hum.MoveToFinished:Connect(function(reached) table.insert(log, "me " .. tostring(reached)) end)
        npc.MoveToFinished:Connect(function(reached) table.insert(log, "npc " .. tostring(reached)) end)
        command("walk", function() hum:MoveTo(Vector3.new(12.5, 64, -3.5)) end)
        command("npc", function() npc:MoveTo(Vector3.new(1, 2, 3)) end)
        command("log", function() print(table.concat(log, ", ")) end)
    "#;
    load(&mut game, &mut st, "walker", src, 0.7);
    let goto = "goto 12 64 -4 --near 1 --timeout 8";
    // A Humanoid of the script's own has no one to walk.
    assert!(st.command("npc"));
    run_for(&mut st, &mut game, 0.1);
    assert!(game.agent_calls.is_empty(), "{:?}", game.agent_calls);
    // The game answers at once: there.
    game.agent_now
        .insert("goto".into(), serde_json::json!({ "ok": true, "arrived": true }));
    assert!(st.command("walk"));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(game.agent_calls, [goto]);
    // The game answers later, when the player gets there.
    game.agent_now.clear();
    assert!(st.command("walk"));
    run_for(&mut st, &mut game, 0.1);
    let (_, tx) = game.agent_later.pop().unwrap();
    tx.send(r#"{"ok": true, "arrived": true}"#.into()).unwrap();
    run_for(&mut st, &mut game, 0.1);
    // Another MoveTo takes the walk's place: the first stops, unfinished.
    assert!(st.command("walk"));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("walk"));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(game.agent_calls[2..], [goto, "stop", goto]);
    // No answer: it gives up after 8 seconds, as Roblox's does.
    run_for(&mut st, &mut game, 7.5);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    run_for(&mut st, &mut game, 1.0);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "walker");
    assert_eq!(p[0], "npc false, me true, me true, me false");
    assert_eq!(p[1], "npc false, me true, me true, me false, me false");
}

#[test]
fn the_mouse_has_its_unit_ray_and_origin() {
    let mut game = FakeGame::new("rbxw-mouse");
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local mouse = game.Players.LocalPlayer:GetMouse()
        local ray = mouse.UnitRay
        print(typeof(ray), v(ray.Origin), v(ray.Direction))
        print(typeof(mouse.Origin), v(mouse.Origin.Position), v(mouse.Origin.LookVector))
        print(pcall(function() mouse.UnitRay = ray end))
    "#
    );
    load(&mut game, &mut st, "mouse", &src, 0.7);
    let p = printed(&st, "mouse");
    // From the camera at the eyes, through the crosshair: north (yaw 180).
    assert_eq!(p[0], "Ray    10.500 65.620 -3.250    0.000 0.000 -1.000");
    assert_eq!(p[1], "CFrame    10.500 65.620 -3.250    0.000 0.000 -1.000");
    assert!(
        p[2].starts_with("false") && p[2].contains("Property is read only"),
        "{}",
        p[2]
    );
}

/// A zombie of the fake game's at (x, 64, z), facing south, with its health.
fn zombie(id: u32, x: f64, z: f64) -> EntityInfo {
    EntityInfo {
        id,
        kind: "minecraft:zombie".into(),
        pos: [x, 64.0, z],
        width: 0.6,
        height: 1.95,
        eye: 1.74,
        health: 20.0,
        max_health: 20.0,
        speed: 0.23,
        on_ground: true,
        ..EntityInfo::default()
    }
}

/// A dropped stack of `count` of `item` at (x, 64, z); with `item` empty,
/// an experience orb worth `count`.
fn dropped(id: u32, item: &str, count: u32, x: f64, z: f64) -> EntityInfo {
    let orb = item.is_empty();
    let size = if orb { 0.5 } else { 0.25 };
    EntityInfo {
        id,
        kind: if orb {
            "minecraft:experience_orb"
        } else {
            "minecraft:item"
        }
        .into(),
        pos: [x, 64.0, z],
        width: size,
        height: size,
        eye: size * 0.85,
        on_ground: true,
        item: Some((item.into(), count)),
        ..EntityInfo::default()
    }
}

#[test]
fn mobs_items_and_orbs_are_in_the_workspace() {
    let mut game = FakeGame::new("rbxw-mobs");
    let mut bob = zombie(6, 8.5, -6.0);
    bob.name = "Bob".into();
    bob.baby = true;
    game.entities = vec![
        zombie(5, 12.5, -3.25),
        bob,
        dropped(7, "minecraft:diamond_sword", 3, 9.5, -1.5),
        dropped(8, "", 7, 11.5, -1.5),
    ];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        command("look", function()
            local names = {}
            for _, c in workspace:GetChildren() do table.insert(names, c.Name .. ":" .. c.ClassName) end
            print(table.concat(names, " "))
            local z = workspace.Zombie
            local root, head, hum = z.HumanoidRootPart, z.Head, z.Humanoid
            print(z.PrimaryPart == root, hum.RootPart == root, hum.Health, hum.MaxHealth, hum.RigType, hum.HipHeight, string.format("%.4f", hum.WalkSpeed))
            print(v(root.Size), v(root.Position), v(head.Size), v(head.Position))
            print(z:GetAttribute("EntityId"), z:GetAttribute("EntityType"), z:GetAttribute("Baby"), workspace.Bob:GetAttribute("Baby"))
            local item = workspace["Diamond Sword"]
            print(item.ClassName, item:GetAttribute("ItemId"), item:GetAttribute("Count"), v(item.Size), v(item.Position))
            local orb = workspace["Experience Orb"]
            print(orb:GetAttribute("Value"), orb:GetAttribute("EntityType"), root.Transparency, root.CanCollide, root.CanTouch, root.CanQuery, root.Anchored)
        end)
    "#
    );
    load(&mut game, &mut st, "mobs", &src, 0.6);
    // Nothing looked at the world yet: the game wasn't asked.
    assert_eq!(game.entity_asks, 0);
    assert!(st.command("look"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "mobs");
    assert_eq!(
        p[0],
        "Camera:Camera Terrain:Terrain Player1:Model Zombie:Model Bob:Model Diamond Sword:Part Experience Orb:Part"
    );
    assert_eq!(
        p[1],
        "true    true    100    100    Enum.HumanoidRigType.R6    0    9.9309"
    );
    // The root part from the feet to under the head, a half-block cube at the eyes (1.74).
    assert_eq!(
        p[2],
        "0.600 1.490 0.600    12.500 64.745 -3.250    0.500 0.500 0.500    12.500 65.740 -3.250"
    );
    assert_eq!(p[3], "5    minecraft:zombie    false    true");
    assert_eq!(
        p[4],
        "Part    minecraft:diamond_sword    3    0.250 0.250 0.250    9.500 64.125 -1.500"
    );
    assert_eq!(
        p[5],
        "7    minecraft:experience_orb    1    false    false    true    true"
    );
    // The game draws them, so the layer draws nothing for them.
    game.world_quads.clear();
    st.world(&mut game);
    assert!(game.world_quads.is_empty(), "{} quads", game.world_quads.len());
}

#[test]
fn entities_move_quietly() {
    let mut game = FakeGame::new("rbxw-move");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local root = workspace:WaitForChild("Zombie").HumanoidRootPart
        local changes = 0
        root.Changed:Connect(function() changes += 1 end)
        root:GetPropertyChangedSignal("CFrame"):Connect(function() changes += 1 end)
        command("where", function() print(v(root.Position), v(root.AssemblyLinearVelocity), changes) end)
    "#
    );
    load(&mut game, &mut st, "move", &src, 0.6);
    game.entities[0].pos = [14.5, 64.0, -3.25];
    game.entities[0].velocity = [4.0, 0.0, 0.0];
    run_for(&mut st, &mut game, 0.05);
    assert!(st.command("where"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(printed(&st, "move"), ["14.500 64.745 -3.250    4.000 0.000 0.000    0"]);
}

#[test]
fn entities_coming_and_going_fire_events() {
    let mut game = FakeGame::new("rbxw-events");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local log, descendants = {}, 0
        workspace.ChildAdded:Connect(function(c)
            table.insert(log, "added " .. c.Name .. " " .. tostring(c:FindFirstChildOfClass("Humanoid") ~= nil))
            c.Destroying:Connect(function() table.insert(log, "destroying " .. c.Name) end)
        end)
        workspace.ChildRemoved:Connect(function(c) table.insert(log, "removed " .. c.Name .. " " .. tostring(c.Parent)) end)
        game.DescendantAdded:Connect(function() descendants += 1 end)
        command("log", function() print(table.concat(log, ", "), descendants) end)
    "#;
    load(&mut game, &mut st, "events", src, 0.6);
    // The zombie was there when the script looked: no ChildAdded for it. A
    // skeleton comes, whole, and goes.
    let mut skeleton = zombie(9, 8.5, -6.0);
    skeleton.kind = "minecraft:skeleton".into();
    game.entities.push(skeleton);
    run_for(&mut st, &mut game, 0.1);
    game.entities.remove(1);
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(
        printed(&st, "events"),
        ["added Skeleton true, destroying Skeleton, removed Skeleton nil    4"]
    );
}

#[test]
fn mob_health_fires_health_changed_and_died_before_it_goes() {
    let mut game = FakeGame::new("rbxw-health");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local z = workspace:WaitForChild("Zombie")
        local hum = z.Humanoid
        local log = {}
        hum.HealthChanged:Connect(function(h) table.insert(log, "health " .. h) end)
        hum.Died:Connect(function() table.insert(log, "died " .. hum:GetState().Name) end)
        z.Destroying:Connect(function() table.insert(log, "destroying") end)
        command("log", function() print(table.concat(log, ", ")) end)
    "#;
    load(&mut game, &mut st, "health", src, 0.6);
    game.entities[0].health = 10.0;
    run_for(&mut st, &mut game, 0.1);
    // Dying, it stays as long as the game keeps it, and Died fires once.
    game.entities[0].health = 0.0;
    run_for(&mut st, &mut game, 0.3);
    game.entities.clear();
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(printed(&st, "health"), ["health 50, health 0, died Dead, destroying"]);
}

#[test]
fn a_changed_kind_rebuilds_the_mirror() {
    let mut game = FakeGame::new("rbxw-kind");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local first = workspace:WaitForChild("Zombie")
        command("check", function()
            local now = workspace:FindFirstChild("Skeleton")
            print(first.Parent, workspace:FindFirstChild("Zombie"), now ~= nil, now and now:GetAttribute("EntityId"))
        end)
    "#;
    load(&mut game, &mut st, "kind", src, 0.6);
    // The same id, another type of entity (the game gave it out again).
    game.entities[0].kind = "minecraft:skeleton".into();
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(printed(&st, "kind"), ["nil    nil    true    5"]);
}

#[test]
fn mirrors_are_locked_but_take_children() {
    let mut game = FakeGame::new("rbxw-locked");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local z = workspace:WaitForChild("Zombie")
        local root = z.HumanoidRootPart
        print(pcall(function() z:Destroy() end))
        print(pcall(function() z.Parent = nil end))
        print(pcall(function() root.Parent = workspace end))
        local tag = Instance.new("Folder", z)
        root.CFrame = CFrame.new(0, 100, 0)
        print(tag.Parent == z, v(root.Position))
        command("check", function() print(v(root.Position), tag.Parent) end)
    "#
    );
    load(&mut game, &mut st, "locked", &src, 0.6);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    game.entities.clear();
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "locked");
    for line in &p[0..3] {
        assert!(line.starts_with("false") && line.contains("locked"), "{line}");
    }
    assert_eq!(p[3], "true    0.000 100.000 0.000");
    // The game's place again on the next frame.
    assert_eq!(p[4], "12.500 64.745 -3.250    Zombie");
    // The script's folder went with the zombie.
    assert_eq!(p[5], "12.500 64.745 -3.250    nil");
}

#[test]
fn the_world_is_mirrored_once_looked_at() {
    let mut game = FakeGame::new("rbxw-lazy");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    // A UI script: its GUI, the camera's size, the player and their character.
    let ui = r#"
        local Players = game:GetService("Players")
        local gui = Instance.new("ScreenGui")
        gui.Parent = Players.LocalPlayer:WaitForChild("PlayerGui")
        local size = workspace.CurrentCamera.ViewportSize
        local char = Players.LocalPlayer.Character
        print(size.X, char.Humanoid.Health, workspace.Gravity, workspace.Terrain.Name)
        -- The Workspace's own children by name, and what is out of the tree.
        print(workspace.Camera.ViewportSize.X, workspace[char.Name] == char, #getnilinstances() > 0)
    "#;
    load(&mut game, &mut st, "ui", ui, 0.6);
    assert_eq!(
        printed(&st, "ui"),
        ["960    100    32    Terrain", "960    true    true"]
    );
    assert_eq!(game.entity_asks, 0, "a UI script doesn't have the world mirrored");
    // A look: the zombie is there at once, and ChildAdded doesn't fire for it.
    let src = r#"
        workspace.ChildAdded:Connect(function(c) print("added", c.Name) end)
        print(workspace:FindFirstChild("Zombie") ~= nil)
    "#;
    load(&mut game, &mut st, "look", src, 0.6);
    let asks = game.entity_asks;
    assert!(asks > 0);
    run_for(&mut st, &mut game, 0.5);
    assert_eq!(game.entity_asks, asks + 10, "asked once a frame");
    assert_eq!(printed(&st, "look"), ["true"]);
    // Once the script that looked stops, the game isn't asked, and the mirror goes.
    let i = mod_index(&st, "look");
    st.unload(&mut game, i);
    run_for(&mut st, &mut game, 0.1);
    let asks = game.entity_asks;
    run_for(&mut st, &mut game, 0.5);
    assert_eq!(game.entity_asks, asks);
    let ws = st.s.rbx.workspace;
    assert!(st.s.rbx.child_named(ws, "Zombie").is_none());
}

#[test]
fn game_descendants_and_getinstances_see_the_world() {
    let find = r#"
        local function has(list) for _, i in list do if i.Name == "Zombie" then return true end end return false end
    "#;
    let checks = [
        "print(has(game:GetDescendants()))",
        "print(game:FindFirstChild('Zombie', true) ~= nil)",
        "print(has(getinstances()))",
    ];
    for (n, check) in checks.iter().enumerate() {
        let mut game = FakeGame::new(&format!("rbxw-desc{n}"));
        game.entities = vec![zombie(5, 12.5, -3.25)];
        let mut st = studio(&mut game);
        load(&mut game, &mut st, "desc", &format!("{find}{check}"), 0.6);
        assert_eq!(printed(&st, "desc"), ["true"], "{check}");
    }
}

#[test]
fn a_console_look_lapses_once_nothing_listens() {
    let mut game = FakeGame::new("rbxw-console");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let lines = st.run_chunk(&mut game, "print(#workspace:GetChildren())", false);
    assert_eq!(lines[0].1, "4");
    let asks_after = |st: &mut Studio, game: &mut FakeGame, seconds: f64| {
        run_for(st, game, seconds);
        let asks = game.entity_asks;
        run_for(st, game, 1.0);
        game.entity_asks - asks
    };
    assert!(asks_after(&mut st, &mut game, 30.0) > 0, "mirrored half a minute on");
    // A minute after the line looked, its look lapses.
    assert_eq!(asks_after(&mut st, &mut game, 30.0), 0);
    // A handler of the console's keeps it while it listens.
    st.run_chunk(&mut game, "workspace.ChildAdded:Connect(function() end)", false);
    assert!(asks_after(&mut st, &mut game, 70.0) > 0);
}

#[test]
fn mirrors_outlive_a_mod_reload() {
    let mut game = FakeGame::new("rbxw-reload");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        workspace.ChildAdded:Connect(function(c) print("added", c.Name) end)
        print(workspace:WaitForChild("Zombie"):GetDebugId())
    "#;
    load(&mut game, &mut st, "keep", src, 0.6);
    let i = mod_index(&st, "keep");
    st.load_and_dependents(&mut game, i);
    run_for(&mut st, &mut game, 0.3);
    let p = printed(&st, "keep");
    assert_eq!(p.len(), 2, "{p:?}");
    assert_eq!(p[0], p[1], "the same zombie, and no ChildAdded for it");
}

#[test]
fn entities_about_the_cut_off_keep_their_mirrors() {
    let mut game = FakeGame::new("rbxw-cap");
    game.entities = (0..300)
        .map(|i| zombie(100 + i, 10.5 + f64::from(i) * 0.1, 0.0))
        .collect();
    let mut st = studio(&mut game);
    let src = r#"
        local added = 0
        workspace.ChildAdded:Connect(function() added += 1 end)
        command("count", function() print(#workspace:GetChildren(), added) end)
    "#;
    load(&mut game, &mut st, "cap", src, 0.6);
    // As they move, neighbours in the game's list swap places, back and
    // forth: the 256th and the 257th among them.
    for _ in 0..100 {
        for i in (1..299).step_by(2) {
            game.entities.swap(i, i + 1);
        }
        game.ticks += 1;
        st.frame(&mut game, 0.05);
    }
    assert!(st.command("count"));
    run_for(&mut st, &mut game, 0.05);
    // The 256 nearest, the camera, the terrain and the character: none came or went.
    assert_eq!(printed(&st, "cap"), ["259    0"]);
}

#[test]
fn another_dimension_or_leaving_the_world_clears_the_mirrors() {
    let mut game = FakeGame::new("rbxw-dim");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local log = {}
        local first = workspace:WaitForChild("Zombie")
        workspace.ChildAdded:Connect(function(c) table.insert(log, "added " .. c.Name) end)
        workspace.ChildRemoved:Connect(function(c) table.insert(log, "removed " .. c.Name) end)
        command("log", function()
            print(table.concat(log, ", "), first.Parent, workspace:FindFirstChild("Zombie") ~= first)
        end)
    "#;
    load(&mut game, &mut st, "dim", src, 0.6);
    // Another dimension: the same id is another zombie.
    game.player.as_mut().unwrap().dimension = "minecraft:the_nether".into();
    run_for(&mut st, &mut game, 0.1);
    // Out of the world (a disconnect), no entities; back, they come again.
    let saved = game.player.take();
    run_for(&mut st, &mut game, 0.1);
    game.player = saved;
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(
        printed(&st, "dim"),
        ["removed Zombie, added Zombie, removed Zombie, added Zombie    nil    true"]
    );
}

/// Another player's entity at (x, 64, z), standing.
fn player_entity(id: u32, name: &str, x: f64, z: f64) -> EntityInfo {
    EntityInfo {
        id,
        kind: "minecraft:player".into(),
        name: name.into(),
        pos: [x, 64.0, z],
        width: 0.6,
        height: 1.8,
        eye: 1.62,
        health: 20.0,
        max_health: 20.0,
        speed: 0.1,
        on_ground: true,
        ..EntityInfo::default()
    }
}

/// A player in the server's list.
fn listed(name: &str, uuid: u128, entity: Option<u32>, latency: i32, local: bool) -> PlayerEntry {
    PlayerEntry {
        uuid,
        name: name.into(),
        display_name: name.into(),
        entity,
        local,
        latency,
    }
}

#[test]
fn players_join_leave_and_show_their_characters() {
    let mut game = FakeGame::new("rbxw-players");
    let steve = 0x8667ba71_b85a_4004_af54_457a9734eed7;
    let p = game.player.as_mut().unwrap();
    p.name = "Steve".into();
    p.uuid = steve;
    game.players = Some(vec![
        listed("Steve", steve, None, 30, true),
        listed("Alex", 0x42, Some(12), 120, false),
    ]);
    game.entities = vec![player_entity(12, "Alex", 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        local log = {}
        local function names(list)
            local out = {}
            for _, p in list do table.insert(out, p.Name) end
            return table.concat(out, " ")
        end
        Players.PlayerAdded:Connect(function(p)
            table.insert(log, "joined " .. p.Name)
            p.CharacterAdded:Connect(function(c)
                table.insert(log, p.Name .. " near " .. tostring(c == p.Character and c.Parent == workspace))
            end)
            p.CharacterRemoving:Connect(function(c) table.insert(log, p.Name .. " away " .. c.Name .. " " .. tostring(p.Character)) end)
        end)
        Players.PlayerRemoving:Connect(function(p) table.insert(log, "left " .. p.Name) end)
        local alex = Players:FindFirstChild("Alex")
        print(names(Players:GetPlayers()), alex.Character == workspace.Alex, Players:GetPlayerFromCharacter(workspace.Alex) == alex)
        print(Players:GetPlayerByUserId(alex.UserId) == alex, Players:GetNameFromUserIdAsync(alex.UserId), Players:GetUserIdFromNameAsync("alex") == alex.UserId, alex:GetAttribute("UUID"))
        print(pcall(function() return Players:GetNameFromUserIdAsync(5) end))
        print(pcall(function() return Players:GetUserIdFromNameAsync("Notch") end))
        local h = alex.Character.Humanoid
        print(alex:GetNetworkPing(), Players.LocalPlayer:GetNetworkPing(), Players.MaxPlayers, h.Health, string.format("%.4f %.4f", h.JumpPower, h.JumpHeight))
        command("log", function() print(table.concat(log, ", "), "|", names(Players:GetPlayers()), Players.MaxPlayers) end)
    "#;
    load(&mut game, &mut st, "players", src, 0.6);
    // Kim joins far away, comes near, goes out of range again, and leaves.
    fn list(game: &mut FakeGame) -> &mut Vec<PlayerEntry> {
        game.players.as_mut().unwrap()
    }
    list(&mut game).push(listed("Kim", 0x77, None, 50, false));
    run_for(&mut st, &mut game, 0.6);
    list(&mut game)[2].entity = Some(13);
    game.entities.push(player_entity(13, "Kim", 8.5, -6.0));
    run_for(&mut st, &mut game, 0.6);
    game.entities.pop();
    list(&mut game)[2].entity = None;
    run_for(&mut st, &mut game, 0.6);
    list(&mut game).pop();
    run_for(&mut st, &mut game, 0.6);
    // Bo joins near: his Character comes a frame after PlayerAdded.
    list(&mut game).push(listed("Bo", 0x99, Some(14), 0, false));
    game.entities.push(player_entity(14, "Bo", 9.5, -6.0));
    run_for(&mut st, &mut game, 0.6);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    // Out of the world (a disconnect), the others go.
    let saved = game.player.take();
    run_for(&mut st, &mut game, 0.1);
    game.player = saved;
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "players");
    // Alex was there before the script looked: no PlayerAdded for him.
    assert_eq!(p[0], "Steve Alex    true    true");
    assert_eq!(p[1], "true    Alex    true    00000000-0000-0000-0000-000000000042");
    assert!(p[2].starts_with("false") && p[2].contains("UserId 5"), "{}", p[2]);
    assert!(p[3].starts_with("false") && p[3].contains("Notch"), "{}", p[3]);
    assert_eq!(p[4], "0.12    0.03    2    100    8.4000 1.2522");
    assert_eq!(
        p[5],
        "joined Kim, Kim near true, Kim away Kim nil, left Kim, joined Bo, Bo near true    |    Steve Alex Bo    3"
    );
    assert!(
        p[6].starts_with("joined Kim, Kim near true, Kim away Kim nil, left Kim, joined Bo, Bo near true, Bo away Bo nil, left Alex, left Bo"),
        "{}",
        p[6]
    );
    assert!(p[6].ends_with("|    Steve    1"), "{}", p[6]);
}

#[test]
fn an_unlisted_player_entity_is_an_npc() {
    let mut game = FakeGame::new("rbxw-npc");
    game.players = Some(Vec::new());
    game.entities = vec![player_entity(20, "Guide", 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local npc = workspace:WaitForChild("Guide")
        print(npc.ClassName, npc.Humanoid.Health, game.Players:GetPlayerFromCharacter(npc), #game.Players:GetPlayers())
    "#;
    load(&mut game, &mut st, "npc", src, 0.6);
    assert_eq!(printed(&st, "npc"), ["Model    100    nil    1"]);
}

#[test]
fn another_player_is_not_yours_to_kick() {
    let mut game = FakeGame::new("rbxw-other");
    game.players = Some(vec![listed("Alex", 0x42, Some(12), 0, false)]);
    game.entities = vec![player_entity(12, "Alex", 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local alex = game.Players:WaitForChild("Alex")
        print(pcall(function() alex:Kick("bye") end))
        print(alex:GetMouse(), pcall(function() alex:LoadCharacter() end))
        print(string.format("%.3f", alex:DistanceFromCharacter(Vector3.new(12.5, 65.62, 0))), game.Players.LocalPlayer:GetMouse() ~= nil)
        print(pcall(function() alex:Destroy() end))
    "#;
    load(&mut game, &mut st, "other", src, 0.6);
    let p = printed(&st, "other");
    assert!(
        p[0].starts_with("false") && p[0].contains("Cannot kick a non-local Player"),
        "{}",
        p[0]
    );
    assert!(
        p[1].starts_with("nil    false") && p[1].contains("backend server"),
        "{}",
        p[1]
    );
    // From the head, at the eyes.
    assert_eq!(p[2], "3.250    true");
    assert!(p[3].starts_with("false") && p[3].contains("locked"), "{}", p[3]);
}

#[test]
fn another_players_character_finds_room_at_the_cap() {
    let mut game = FakeGame::new("rbxw-room");
    game.entities = (0..256)
        .map(|i| zombie(100 + i, 10.5 + f64::from(i) * 0.1, 0.0))
        .collect();
    let mut st = studio(&mut game);
    let src = r#"
        local log = {}
        workspace.ChildAdded:Connect(function(c) table.insert(log, "added " .. c.Name) end)
        workspace.ChildRemoved:Connect(function(c) table.insert(log, "removed " .. c:GetAttribute("EntityId")) end)
        command("log", function() print(table.concat(log, ", "), #workspace:GetChildren()) end)
    "#;
    load(&mut game, &mut st, "room", src, 0.6);
    // The game lists the other players first: the farthest zombie makes way.
    game.entities.insert(0, player_entity(12, "Alex", 12.5, -3.25));
    run_for(&mut st, &mut game, 0.1);
    assert!(st.command("log"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(printed(&st, "room"), ["removed 355, added Alex    259"]);
}

#[test]
fn raycasts_and_the_mouse_hit_mobs_not_yourself() {
    let mut game = FakeGame::new("rbxw-rays");
    // Five blocks north of the player, facing them.
    game.entities = vec![zombie(5, 10.5, -8.25)];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local z = workspace:WaitForChild("Zombie")
        local eyes = Vector3.new(10.5, 65.62, -3.25)
        local function hit(r) return r and (r.Instance.Name .. " " .. string.format("%.3f", r.Distance)) end
        local mouse = game.Players.LocalPlayer:GetMouse()
        command("rays", function()
            -- At its eyes, its head; at its knees, its root part.
            print(hit(workspace:Raycast(eyes, Vector3.new(0, 0, -10))))
            print(hit(workspace:Raycast(Vector3.new(10.5, 64.5, -3.25), Vector3.new(0, 0, -10))))
            -- One's own character only when an Include filter names it.
            print(workspace:Raycast(Vector3.new(10.5, 70, -3.25), Vector3.new(0, -10, 0)))
            local only = RaycastParams.new()
            only.FilterType = Enum.RaycastFilterType.Include
            only.FilterDescendantsInstances = { game.Players.LocalPlayer.Character }
            print(hit(workspace:Raycast(Vector3.new(10.5, 70, -3.25), Vector3.new(0, -10, 0), only)))
            print(mouse.Target.Name, mouse.Target.Parent == z, mouse.TargetSurface, v(mouse.Hit.Position))
            mouse.TargetFilter = z
            print(mouse.Target)
            mouse.TargetFilter = nil
        end)
        command("walled", function()
            local only = RaycastParams.new()
            only.FilterType = Enum.RaycastFilterType.Include
            only.FilterDescendantsInstances = { z }
            print(hit(workspace:Raycast(eyes, Vector3.new(0, 0, -10))), hit(workspace:Raycast(eyes, Vector3.new(0, 0, -10), only)))
            print(mouse.Target.Name, mouse.TargetSurface)
        end)
    "#
    );
    load(&mut game, &mut st, "rays", &src, 0.6);
    assert!(st.command("rays"));
    run_for(&mut st, &mut game, 0.05);
    // A wall between: an Include filter that doesn't name the Terrain goes through it.
    game.blocks.insert((10, 65, -6), "minecraft:stone".into());
    assert!(st.command("walled"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "rays");
    assert_eq!(p[0], "Head 4.750");
    assert_eq!(p[1], "HumanoidRootPart 4.700");
    assert_eq!(p[2], "nil");
    assert_eq!(p[3], "Head 4.146");
    assert_eq!(p[4], "Head    true    Enum.NormalId.Front    10.500 65.620 -8.000");
    assert_eq!(p[5], "nil");
    assert_eq!(p[6], "Terrain 1.750    Head 4.750");
    assert_eq!(p[7], "Terrain    Enum.NormalId.Back");
}

#[test]
fn the_mouse_target_looks_at_the_world() {
    let mut game = FakeGame::new("rbxw-target");
    game.entities = vec![zombie(5, 10.5, -8.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local mouse = game.Players.LocalPlayer:GetMouse()
        print(mouse.Target and mouse.Target.Parent.Name)
    "#;
    load(&mut game, &mut st, "target", src, 0.6);
    assert_eq!(printed(&st, "target"), ["Zombie"]);
}

#[test]
fn bounds_queries_find_entities() {
    let mut game = FakeGame::new("rbxw-bounds");
    game.entities = vec![zombie(5, 10.5, -8.25), dropped(7, "minecraft:diamond", 1, 20.5, 10.5)];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local z = workspace:WaitForChild("Zombie")
        local function names(list)
            local out = {}
            for _, p in list do table.insert(out, p.Parent.Name .. "." .. p.Name) end
            table.sort(out)
            return table.concat(out, " ")
        end
        local centre = Vector3.new(10.5, 65, -8.25)
        print(names(workspace:GetPartBoundsInRadius(centre, 1)))
        local params = OverlapParams.new()
        params.FilterDescendantsInstances = { z }
        print(#workspace:GetPartBoundsInRadius(centre, 1, params))
        print(names(workspace:GetPartBoundsInBox(CFrame.new(centre), Vector3.new(2, 2, 2))))
        local ray = Ray.new(Vector3.new(10.5, 65.62, -3.25), Vector3.new(0, 0, -10))
        local part, at, normal, material = workspace:FindPartOnRay(ray)
        print(part.Name, v(at), v(normal), material)
        print(workspace:FindPartOnRayWithIgnoreList(ray, { z }))
        local probe = Instance.new("Part")
        probe.Size = Vector3.new(2, 2, 2)
        probe.CFrame = CFrame.new(centre)
        probe.Parent = workspace
        print(names(workspace:GetPartsInPart(probe)))
        params.FilterDescendantsInstances = {}
        params.MaxParts = 1
        print(#workspace:GetPartBoundsInBox(CFrame.new(centre), Vector3.new(2, 2, 2), params))
        print(workspace:FindPartOnRayWithWhitelist(ray, { probe }) == probe, workspace:FindPartOnRay(ray, probe).Name)
        print(pcall(function() workspace:FindPartOnRay(Vector3.zero) end))
        print(pcall(function() workspace:GetPartsInPart(z) end))
    "#
    );
    load(&mut game, &mut st, "bounds", &src, 0.6);
    let p = printed(&st, "bounds");
    // Near the zombie: its two parts, not the item far off nor the player.
    assert_eq!(p[0], "Zombie.Head Zombie.HumanoidRootPart");
    assert_eq!(p[1], "0");
    assert_eq!(p[2], p[0]);
    assert_eq!(
        p[3],
        "Head    10.500 65.620 -8.000    0.000 0.000 1.000    Enum.Material.Plastic"
    );
    // Vector3s print as Roblox has them, in single precision.
    assert_eq!(p[4], "nil    10.5, 65.6200027, -13.25    0, 0, 0    Enum.Material.Air");
    assert_eq!(p[5], p[0]);
    assert_eq!(p[6], "1");
    assert_eq!(p[7], "true    Head");
    assert!(p[8].starts_with("false") && p[8].contains("Ray expected"), "{}", p[8]);
    assert!(
        p[9].starts_with("false") && p[9].contains("BasePart expected"),
        "{}",
        p[9]
    );
}

#[test]
fn terrain_reads_the_map() {
    let mut game = FakeGame::new("rbxw-terrain");
    game.blocks.insert((10, 63, -5), "minecraft:grass_block".into());
    game.blocks.insert((10, 62, -5), "minecraft:stone".into());
    game.blocks.insert((11, 62, -5), "minecraft:stone".into());
    let mut st = studio(&mut game);
    let src = r#"
        local terrain = workspace.Terrain
        local m, o = terrain:ReadVoxels(Region3.new(Vector3.new(10, 62, -5), Vector3.new(11, 65, -4)), 1)
        print(m.Size, m[1][1][1], m[1][2][1], m[1][3][1], o[1][1][1], o[1][2][1], o[1][3][1], o.Size)
        -- Roblox's cells of 4: one, around these blocks.
        local m4, o4 = terrain:ReadVoxels(Region3.new(Vector3.new(9, 61, -6), Vector3.new(10, 62, -5)), 4)
        print(m4.Size, m4[1][1][1], o4[1][1][1])
        print(terrain:GetBlock(Vector3.new(10.5, 62.5, -4.5)), terrain:GetBlock(Vector3.new(10.5, 70, -4.5)))
        print(terrain:WorldToCell(Vector3.new(1.5, 64.2, -0.5)) == Vector3.new(1, 64, -1), terrain:CellCenterToWorld(1, 64, -1), terrain:CellCornerToWorld(1, 64, -1))
        print(pcall(function() terrain:FillBlock(CFrame.new(), Vector3.one, Enum.Material.Grass) end))
        print(pcall(function() terrain:ReadVoxels(Region3.new(Vector3.zero, Vector3.new(100, 100, 100)), 1) end))
        print(pcall(function() terrain:ReadVoxels(Region3.new(Vector3.new(3e9, 0, 0), Vector3.new(3e9 + 8, 8, 8)), 4) end))
        print(pcall(function() terrain:ReadVoxels(Region3.new(Vector3.zero, Vector3.one), 0) end))
        print(pcall(function() terrain:ReadVoxels(Vector3.zero, 4) end))
    "#;
    load(&mut game, &mut st, "terrain", src, 0.6);
    let p = printed(&st, "terrain");
    assert_eq!(
        p[0],
        "1, 3, 1    Enum.Material.Slate    Enum.Material.Grass    Enum.Material.Air    1    1    0    1, 3, 1"
    );
    // Two stone blocks and a grass one of the cell's 64.
    assert_eq!(p[1], "1, 1, 1    Enum.Material.Slate    0.046875");
    assert_eq!(p[2], "minecraft:stone    minecraft:air");
    assert_eq!(p[3], "true    1.5, 64.5, -0.5    1, 64, -1");
    assert!(
        p[4].starts_with("false") && p[4].contains("can't change the terrain"),
        "{}",
        p[4]
    );
    assert!(
        p[5].starts_with("false") && p[5].contains("at most 4096 cells"),
        "{}",
        p[5]
    );
    // Past the world's edge is an error, not a read that wraps around.
    assert!(
        p[6].starts_with("false") && p[6].contains("30000000 blocks out"),
        "{}",
        p[6]
    );
    assert!(p[7].starts_with("false") && p[7].contains("1 to 64 blocks"), "{}", p[7]);
    assert!(
        p[8].starts_with("false") && p[8].contains("Region3 expected"),
        "{}",
        p[8]
    );
    // Reading the map doesn't mirror the world's entities.
    assert_eq!(game.entity_asks, 0);
}

#[test]
fn characters_have_r6_limbs_in_a_rest_pose() {
    let mut game = FakeGame::new("rbxw-limbs");
    // Another player five blocks north, facing south.
    game.players = Some(vec![listed("Alex", 0x42, Some(12), 0, false)]);
    game.entities = vec![player_entity(12, "Alex", 12.5, -8.25)];
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local char = game.Players.LocalPlayer.Character
        local names = {}
        for _, c in char:GetChildren() do table.insert(names, c.Name) end
        print(table.concat(names, ", "))
        print(v(char.Torso.Size), v(char["Right Arm"].Size), v(char["Left Leg"].Size))
        -- Facing north: the right arm is to the east.
        print(v(char.Torso.Position), v(char["Right Arm"].Position), v(char["Left Arm"].Position))
        print(v(char["Left Leg"].Position), v(char["Right Leg"].Position))
        local unit = true
        for _, c in char:GetChildren() do
            if c:IsA("BasePart") and math.abs(c.CFrame.LookVector.Magnitude - 1) > 1e-9 then unit = false end
        end
        print(unit)
        -- Another player's limbs can be hit: facing south, his right arm is to the west.
        local alex = workspace:WaitForChild("Alex")
        local arm = alex["Right Arm"]
        local hit = workspace:Raycast(arm.Position - Vector3.new(2, 0, 0), Vector3.new(4, 0, 0))
        print(hit.Instance == arm, #alex:GetChildren(), v(arm.Position))
        command("crouched", function() print(v(char["Left Leg"].Position), v(char.Torso.Position)) end)
    "#
    );
    load(&mut game, &mut st, "limbs", &src, 0.6);
    // Crouching, the torso is lower and the legs still stand on the feet.
    game.pose = Some(PlayerPose {
        pose: "crouching".into(),
        on_ground: true,
        ..PlayerPose::default()
    });
    run_for(&mut st, &mut game, 0.05);
    assert!(st.command("crouched"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "limbs");
    assert_eq!(
        p[0],
        "HumanoidRootPart, Head, Humanoid, Torso, Left Arm, Right Arm, Left Leg, Right Leg"
    );
    assert_eq!(p[1], "0.469 0.703 0.234    0.234 0.703 0.234    0.234 0.703 0.234");
    assert_eq!(
        p[2],
        "10.500 65.055 -3.250    10.852 65.055 -3.250    10.148 65.055 -3.250"
    );
    assert_eq!(p[3], "10.389 64.352 -3.250    10.611 64.352 -3.250");
    assert_eq!(p[4], "true");
    assert_eq!(p[5], "true    8    12.148 65.055 -8.250");
    assert_eq!(p[6], "10.389 64.352 -3.250    10.500 64.879 -3.250");
}

#[test]
fn the_limbs_are_where_the_games_model_has_them() {
    // The headless game poses the player's model standing: each limb's centre
    // is its frame's origin moved as the model's boxes are (an arm 4 pixels
    // down and 1 out, a leg 6 down), whichever way they face.
    let source = Source::Files {
        id: "limbs".into(),
        files: vec![("limbs.lua".into(), "local me = game.Players.LocalPlayer\n".into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let lua = r#"
        local char = game.Players.LocalPlayer.Character
        local function frame(name)
            local f = player.part(name)
            return Vector3.new(f.x, f.y, f.z), Vector3.new(f.up.x, f.up.y, f.up.z), Vector3.new(f.left.x, f.left.y, f.left.z)
        end
        local o, up, left = frame("right_arm")
        local arm = o - up * (4 / 16) - left * (1 / 16)
        local lo, lup = frame("left_leg")
        local leg = lo - lup * (6 / 16)
        print((char["Right Arm"].Position - arm).Magnitude < 1e-6, (char["Left Leg"].Position - leg).Magnitude < 1e-6)
    "#;
    let report = run_steps(
        &mut run,
        &[
            json!({ "lua": lua, "expect_output": "true    true" }),
            json!({ "player": { "yaw": 90 } }),
            json!({ "frames": 1 }),
            json!({ "lua": lua, "expect_output": "true    true" }),
        ],
    );
    assert_eq!(report["ok"], true, "{report:#}");
}

#[test]
fn a_headless_test_puts_mobs_and_players_near() {
    let source = Source::Files {
        id: "near".into(),
        files: vec![("near.lua".into(), "local me = game.Players.LocalPlayer\n".into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    let lua = "local z = workspace:FindFirstChild('Zombie') print(z and z.Humanoid.Health, #game.Players:GetPlayers(), workspace:FindFirstChild('Diamond') ~= nil)";
    let report = run_steps(
        &mut run,
        &[
            json!({ "lua": lua, "expect_output": "nil    1    false" }),
            json!({ "entities": [
                { "kind": "zombie", "pos": [3, 64, 0], "health": 10 },
                { "kind": "player", "name": "Alex", "id": 12, "pos": [5, 64, 0] },
                { "kind": "item", "item": "minecraft:diamond", "count": 3 },
            ] }),
            json!({ "players": [{ "name": "Alex", "entity": 12, "latency": 40 }] }),
            json!({ "frames": 10 }),
            json!({ "lua": lua, "expect_output": "50    2    true" }),
            json!({
                "lua": "local alex = game.Players.Alex print(alex.Character == workspace.Alex, alex:GetNetworkPing(), workspace.Diamond:GetAttribute('Count'), workspace.Zombie:GetAttribute('EntityId'))",
                "expect_output": "true    0.04    3    100",
            }),
            json!({ "entities": [] }),
            json!({ "frames": 2 }),
            json!({ "lua": lua, "expect_output": "nil    2    false" }),
            json!({ "entity": [] }),
        ],
    );
    let steps = report["steps"].as_array().unwrap();
    assert!(steps[..9].iter().all(|s| s["ok"] == true), "{report:#}");
    // An unknown step says which there are.
    assert!(
        steps[9]["failures"][0].as_str().unwrap().contains("entities, players"),
        "{report:#}"
    );
}

#[test]
fn a_mobs_humanoid_walks_no_one() {
    let mut game = FakeGame::new("rbxw-mobwalk");
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    let src = r#"
        local hum = workspace:WaitForChild("Zombie").Humanoid
        hum.MoveToFinished:Connect(function(reached) print("finished", reached) end)
        hum:MoveTo(Vector3.new(0, 64, 0))
        print(hum:GetState().Name, hum.WalkSpeed > 9)
    "#;
    load(&mut game, &mut st, "mobwalk", src, 0.6);
    // The game moves it: its MoveToFinished fires with false, and the agent isn't asked.
    assert!(game.agent_calls.is_empty(), "{:?}", game.agent_calls);
    assert_eq!(printed(&st, "mobwalk"), ["Running    true", "finished    false"]);
}

#[test]
fn debris_leaves_the_worlds_instances_alone() {
    let mut game = FakeGame::new("rbxw-debris");
    game.players = Some(vec![listed("Alex", 0x42, Some(12), 0, false)]);
    game.entities = vec![
        zombie(5, 12.5, -3.25),
        player_entity(12, "Alex", 8.5, -6.0),
        dropped(7, "minecraft:diamond", 3, 9.5, -1.5),
    ];
    let mut st = studio(&mut game);
    let src = r#"
        local Debris = game:GetService("Debris")
        local first = workspace:WaitForChild("Zombie"):GetDebugId()
        -- Every one of them as Debris gets it from a script: through a lookup, none kept.
        Debris:AddItem(workspace.Zombie, 0)
        Debris:AddItem(workspace.Zombie.HumanoidRootPart, 0)
        Debris:AddItem(workspace.Diamond, 0)
        Debris:AddItem(game.Players:FindFirstChild("Alex"), 0)
        Debris:AddItem(workspace.Alex, 0)
        Debris:AddItem(game.Players.LocalPlayer.Character, 0)
        local mine = Instance.new("Part", workspace)
        mine.Name = "Mine"
        Debris:AddItem(mine, 0)
        mine = nil
        command("check", function()
            local z = workspace:FindFirstChild("Zombie")
            print(z and z:GetDebugId() == first, z and z:FindFirstChild("HumanoidRootPart") ~= nil, workspace:FindFirstChild("Mine"))
            local alex = game.Players:FindFirstChild("Alex")
            print(workspace.Diamond:GetAttribute("Count"), alex ~= nil and alex.Character == workspace.Alex, #game.Players:GetPlayers(), workspace:FindFirstChild("Bob") ~= nil)
        end)
    "#;
    load(&mut game, &mut st, "debris", src, 0.6);
    // Past the layer's sweep of what is destroyed (every 120 frames), with
    // the list read every half second.
    run_for(&mut st, &mut game, 7.0);
    // The game changes them: a stack grows, the zombie gets a name.
    game.entities[2].item = Some(("minecraft:diamond".into(), 5));
    game.entities[0].name = "Bob".into();
    run_for(&mut st, &mut game, 0.6);
    game.entities[0].name.clear();
    run_for(&mut st, &mut game, 0.6);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "debris");
    assert_eq!(p[0], "true    true    nil");
    assert_eq!(p[1], "5    true    2    false");
    // Debris says why they stay, as Destroy does; the script's own part went.
    let warnings: Vec<String> = log(&st)
        .into_iter()
        .filter(|l| l.contains("Debris can't destroy"))
        .collect();
    assert_eq!(warnings.len(), 6, "{warnings:?}");
    assert!(
        warnings[0].contains("Zombie: its Parent property is locked"),
        "{}",
        warnings[0]
    );
}

#[test]
fn the_world_mends_what_something_else_destroyed() {
    // Nothing a script does destroys a mirror or another player's Player
    // (they are locked), but should something, the world lets it go: the
    // entity gets a new mirror, the player a new Player.
    let mut game = FakeGame::new("rbxw-mend");
    game.players = Some(vec![listed("Alex", 0x42, Some(12), 0, false)]);
    game.entities = vec![
        zombie(5, 12.5, -3.25),
        player_entity(12, "Alex", 8.5, -6.0),
        dropped(7, "minecraft:diamond", 3, 9.5, -1.5),
    ];
    let mut st = studio(&mut game);
    let src = r#"
        local log = {}
        workspace.ChildAdded:Connect(function(c) table.insert(log, "added " .. c.Name) end)
        game.Players.PlayerAdded:Connect(function(p) table.insert(log, "joined " .. p.Name) end)
        command("check", function()
            local alex = game.Players:FindFirstChild("Alex")
            print(table.concat(log, ", "))
            print(workspace.Zombie.Humanoid.Health, workspace.Diamond:GetAttribute("Count"), alex.Character == workspace.Alex, #game.Players:GetPlayers())
        end)
    "#;
    load(&mut game, &mut st, "mend", src, 0.6);
    let rbx = &mut st.s.rbx;
    let (ws, players) = (rbx.workspace, rbx.players);
    let (zombie, diamond, alex) = (
        rbx.child_named(ws, "Zombie").unwrap(),
        rbx.child_named(ws, "Diamond").unwrap(),
        rbx.child_named(players, "Alex").unwrap(),
    );
    for id in [zombie, diamond, alex] {
        rbx.destroy(id);
    }
    // The zombie's model and Alex's Player are let go of at once, as the
    // layer's sweep lets go of what no script holds; the diamond's is kept.
    rbx.insts.remove(&zombie);
    rbx.insts.remove(&alex);
    game.entities[0].health = 10.0;
    game.entities[2].item = Some(("minecraft:diamond".into(), 5));
    game.players.as_mut().unwrap()[0].latency = 30;
    run_for(&mut st, &mut game, 1.0);
    assert!(st.command("check"));
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "mend");
    assert_eq!(p[0], "joined Alex, added Zombie, added Diamond");
    assert_eq!(p[1], "50    5    true    2");
}

#[test]
fn removing_a_mod_leaves_the_later_ones_looks_and_handlers_theirs() {
    let mut game = FakeGame::new("rbxw-renumber");
    let mut st = studio(&mut game);
    load(&mut game, &mut st, "a", "print('a')", 0.6);
    let src = r#"
        workspace.ChildAdded:Connect(function(c) print("added", c.Name) end)
        print(#workspace:GetChildren())
    "#;
    load(&mut game, &mut st, "b", src, 0.6);
    assert!(mod_index(&st, "a") < mod_index(&st, "b"));
    // a's file goes: b moves up the studio's list, and still looks and listens.
    std::fs::remove_file(game.mods_dir().join("a.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    game.entities = vec![zombie(5, 12.5, -3.25)];
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(printed(&st, "b"), ["3", "added    Zombie"]);
    // b's goes too: no script looks, and the game is asked no more.
    std::fs::remove_file(game.mods_dir().join("b.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    let asks = game.entity_asks;
    run_for(&mut st, &mut game, 1.0);
    assert_eq!(game.entity_asks, asks, "the world stays mirrored for no one");
    let ws = st.s.rbx.workspace;
    assert!(st.s.rbx.child_named(ws, "Zombie").is_none());
}

#[test]
fn a_script_moving_the_player_every_frame_leaves_the_mods_their_sends() {
    let mut game = FakeGame::new("rbxw-mover");
    let mut st = studio(&mut game);
    // A fly script's way: the root part a little higher each frame.
    let mover = r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        game:GetService("RunService").Heartbeat:Connect(function()
            root.CFrame = root.CFrame + Vector3.new(0, 0.05, 0)
        end)
    "#;
    load(&mut game, &mut st, "mover", mover, 1.0);
    let sender = r#"
        command("send", function()
            local ok = 0
            for i = 1, 5 do if pcall(game.run, "time set day") then ok += 1 end end
            print(ok)
        end)
    "#;
    load(&mut game, &mut st, "sender", sender, 2.0);
    // The player moves a few times a second: never more than the sends refill.
    let mark = game.sent.len();
    run_for(&mut st, &mut game, 3.0);
    let tps = game.sent[mark..].iter().filter(|l| l.starts_with("/tp @s ")).count();
    assert!((8..=13).contains(&tps), "{tps} teleports in 3 seconds");
    // And another mod's game.run keeps working, again and again.
    for _ in 0..3 {
        assert!(st.command("send"));
        run_for(&mut st, &mut game, 2.0);
    }
    let p = printed(&st, "sender");
    assert_eq!(p.len(), 3, "{p:?}");
    assert!(p.iter().all(|n| n.parse::<u32>().unwrap() >= 3), "{p:?}");
}

#[test]
fn rays_see_what_a_script_just_moved_and_the_mouse_points_once_a_frame() {
    let mut game = FakeGame::new("rbxw-fresh");
    let mut st = studio(&mut game);
    let src = r#"
        local mouse = game.Players.LocalPlayer:GetMouse()
        local eyes = Vector3.new(10.5, 65.62, -3.25)
        local ahead = Vector3.new(0, 0, -10)
        local function ray() return workspace:Raycast(eyes, ahead) end
        local function far(r) return r and string.format("%.2f", r.Distance) end
        local wall, box
        command("wall", function()
            print(mouse.Target)
            wall = Instance.new("Part")
            wall.Name = "Wall"
            wall.Size = Vector3.new(4, 4, 1)
            wall.CFrame = CFrame.new(10.5, 65.62, -6.25)
            wall.Parent = workspace
            -- A ray meets it at once; the Mouse, worked out once a frame, the next frame.
            print(ray().Instance.Name, mouse.Target)
            wall.Position = Vector3.new(10.5, 65.62, -8.25)
            print(far(ray()))
            wall.CanQuery = false
            print(ray())
            wall.CanQuery = true
            wall.Size = Vector3.new(4, 4, 3)
            print(far(ray()), #workspace:GetPartBoundsInRadius(Vector3.new(10.5, 65.62, -8.25), 1))
            box = Instance.new("Model", workspace)
            wall.Parent = box
            print(ray().Instance.Name)
            box.Parent = nil
            print(ray())
            box.Parent = workspace
        end)
        command("later", function() print(mouse.Target.Name, far(ray())) end)
    "#;
    load(&mut game, &mut st, "fresh", src, 0.6);
    assert!(st.command("wall"));
    run_for(&mut st, &mut game, 0.05);
    assert!(st.command("later"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(
        printed(&st, "fresh"),
        [
            "nil",
            "Wall    nil",
            "4.50",
            "nil",
            "3.50    1",
            "Wall",
            "nil",
            "Wall    3.50"
        ]
    );
}

#[test]
fn the_local_ping_needs_no_look_at_the_world() {
    let mut game = FakeGame::new("rbxw-ping");
    game.players = Some(vec![listed("Steve", 1, None, 80, true)]);
    game.entities = vec![zombie(5, 12.5, -3.25)];
    let mut st = studio(&mut game);
    // A UI script's ping counter: the list is read for it, the world isn't.
    let src = r#"
        local me = game.Players.LocalPlayer
        command("ping", function() print(me:GetNetworkPing()) end)
    "#;
    load(&mut game, &mut st, "ping", src, 0.6);
    let ping = |st: &mut Studio, game: &mut FakeGame| {
        assert!(st.command("ping"));
        run_for(st, game, 0.05);
    };
    ping(&mut st, &mut game);
    // Read again at most every half second.
    game.players.as_mut().unwrap()[0].latency = 120;
    ping(&mut st, &mut game);
    run_for(&mut st, &mut game, 0.6);
    ping(&mut st, &mut game);
    // While the list can't be read, the last ping stays.
    game.players = None;
    run_for(&mut st, &mut game, 0.6);
    ping(&mut st, &mut game);
    assert_eq!(printed(&st, "ping"), ["0.08", "0.08", "0.12", "0.12"]);
    assert_eq!(game.entity_asks, 0);
}
