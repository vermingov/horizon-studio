// The Roblox layer's world on a fake game (hstudio_rbx_world): the
// LocalPlayer is the player playing, and its Character an R6 rig that
// follows them.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, PlayerPose, Studio};
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

/// `v` printed with three decimals, as the scripts below print vectors.
const FMT: &str = r#"
    local function v(x) return string.format("%.3f %.3f %.3f", x.X, x.Y, x.Z) end
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
}

/// Runs a mod's command and a frame or two after: what the game was sent.
fn sent_by(st: &mut Studio, game: &mut FakeGame, cmd: &str) -> Vec<String> {
    let mark = game.sent.len();
    assert!(st.command(cmd), "no command {cmd}");
    run_for(st, game, 0.1);
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
    "#;
    load(&mut game, &mut st, "tween", src, 0.7);
    let mark = game.sent.len();
    assert!(st.command("glide"));
    run_for(&mut st, &mut game, 1.5);
    let tps = &game.sent[mark..];
    assert!(tps.len() >= 2, "{tps:?}");
    assert!(tps.iter().all(|l| l.starts_with("/tp @s ")), "{tps:?}");
    assert_eq!(tps.last().unwrap(), "/tp @s 20.000 68.945 20.000");
    // With every send used by chat, the teleport waits for the next one.
    run_for(&mut st, &mut game, 3.0);
    let mark = game.sent.len();
    assert!(st.command("busy"));
    run_for(&mut st, &mut game, 0.05);
    let now: Vec<&String> = game.sent[mark..].iter().filter(|l| l.starts_with('/')).collect();
    assert!(now.is_empty(), "{now:?}");
    run_for(&mut st, &mut game, 0.5);
    let later: Vec<&String> = game.sent[mark..].iter().filter(|l| l.starts_with('/')).collect();
    assert_eq!(later, ["/tp @s 1.000 68.945 2.000"]);
}
