// Infinite Yield's character commands in the game (hstudio_rbx_body): each
// test makes the Roblox calls an IY command makes, as a script mod with a
// command per step, and checks what the client did to its own player
// (`game.moves`: a jump, its speeds, a push, its model's look), never with
// a chat command.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, PlayerPose, Studio};

/// A world with the script `src` loaded as mod "t" (its `command`s are the
/// steps), the player standing on the ground facing north.
fn world(name: &str, src: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    game.pose = pose(true);
    let mut st = studio(&mut game);
    let src = format!(
        "local char = game.Players.LocalPlayer.Character\nlocal hum, root = char.Humanoid, char.HumanoidRootPart\n{src}"
    );
    game.write_mod("t.lua", &src);
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "t");
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{msg}\n{trace}\n{:?}", log(&st));
    }
    (game, st)
}

fn pose(on_ground: bool) -> Option<PlayerPose> {
    Some(PlayerPose {
        pose: "standing".into(),
        on_ground,
        ..PlayerPose::default()
    })
}

/// Runs the script's command `cmd` (none for ""), then `secs` of frames:
/// the moves made meanwhile.
fn step(st: &mut Studio, game: &mut FakeGame, cmd: &str, secs: f64) -> Vec<String> {
    let mark = game.moves.len();
    if !cmd.is_empty() {
        assert!(st.command(cmd), "{cmd}");
    }
    run_for(st, game, secs);
    game.moves[mark..].to_vec()
}

/// What the script printed.
fn printed(st: &Studio) -> Vec<String> {
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect()
}

fn count(moves: &[String], what: &str) -> usize {
    moves.iter().filter(|m| *m == what).count()
}

#[test]
fn jump_and_infinite_jump_jump_in_the_game_in_the_air_too() {
    let (mut game, mut st) = world(
        "iyc-jump",
        r#"
        -- jump
        command("jump", function() hum:ChangeState(Enum.HumanoidStateType.Jumping) end)
        -- infjump / flyjump: each jump request jumps
        local conn
        command("infjump", function()
            conn = game:GetService("UserInputService").JumpRequest:Connect(function()
                hum:ChangeState(Enum.HumanoidStateType.Jumping)
            end)
        end)
        command("uninfjump", function() conn:Disconnect() end)
        command("state", function() print(hum:GetState().Name) end)
        "#,
    );
    let m = step(&mut st, &mut game, "jump", 0.1);
    assert_eq!(count(&m, "jump"), 1, "{m:?}");
    step(&mut st, &mut game, "state", 0.05);
    // In the air, the jump key jumps again; held, it keeps jumping.
    step(&mut st, &mut game, "infjump", 0.1);
    game.pose = pose(false);
    st.key("key.keyboard.space", true);
    let m = step(&mut st, &mut game, "", 0.3);
    st.key("key.keyboard.space", false);
    assert!(count(&m, "jump") >= 3, "{m:?}");
    // (The last request's handler runs on the next frame.)
    step(&mut st, &mut game, "", 0.1);
    let m = step(&mut st, &mut game, "", 0.2);
    assert_eq!(count(&m, "jump"), 0, "{m:?}");
    step(&mut st, &mut game, "uninfjump", 0.1);
    st.key("key.keyboard.space", true);
    let m = step(&mut st, &mut game, "", 0.2);
    st.key("key.keyboard.space", false);
    assert_eq!(count(&m, "jump"), 0, "{m:?}");
    assert_eq!(printed(&st), ["Jumping"]);
    assert!(game.sent.is_empty(), "{:?}", game.sent);
}

#[test]
fn humanoid_jump_jumps_once_from_the_ground_as_autojump_has_it() {
    let (mut game, mut st) = world(
        "iyc-autojump",
        r#"
        command("jump", function() hum.Jump = true print(hum.Jump) end)
        command("nojump", function() hum:SetStateEnabled(Enum.HumanoidStateType.Jumping, false) end)
        command("yesjump", function() hum:SetStateEnabled(3, true) end)
        "#,
    );
    let m = step(&mut st, &mut game, "jump", 0.1);
    assert_eq!(m, ["jump"]);
    // Not in the air (Roblox's Humanoid jumps from the ground).
    game.pose = pose(false);
    run_for(&mut st, &mut game, 0.05);
    let m = step(&mut st, &mut game, "jump", 0.1);
    assert!(m.is_empty(), "{m:?}");
    // disablestate Jumping: no jump of the script's, nor the game's own.
    game.pose = pose(true);
    run_for(&mut st, &mut game, 0.05);
    let m = step(&mut st, &mut game, "nojump", 0.1);
    assert_eq!(m, ["speed 1.00 0.00"]);
    let m = step(&mut st, &mut game, "jump", 0.1);
    assert!(m.is_empty(), "{m:?}");
    let m = step(&mut st, &mut game, "yesjump", 0.1);
    assert_eq!(m, ["speed 1.00 1.00"]);
    assert_eq!(printed(&st), ["false", "false", "false"]);
}

#[test]
fn sit_sits_the_model_and_holds_the_player_until_the_jump_key() {
    let (mut game, mut st) = world(
        "iyc-sit",
        r#"
        command("sit", function() hum.Sit = true end)
        command("lay", function()
            hum.Sit = true
            task.wait(0.1)
            root.CFrame = root.CFrame * CFrame.Angles(math.pi * 0.5, 0, 0)
        end)
        command("nosit", function() hum:SetStateEnabled(Enum.HumanoidStateType.Seated, false) end)
        command("state", function() print(hum:GetState().Name, hum.Sit) end)
        "#,
    );
    let m = step(&mut st, &mut game, "sit", 0.1);
    assert_eq!(m, ["speed 0.00 1.00", "body pose=sitting"]);
    step(&mut st, &mut game, "state", 0.05);
    // The jump key gets up (the game's own jump jumps).
    st.key("key.keyboard.space", true);
    let m = step(&mut st, &mut game, "", 0.1);
    st.key("key.keyboard.space", false);
    assert_eq!(m, ["speed 1.00 1.00", "body"]);
    step(&mut st, &mut game, "state", 0.05);
    // lay: seated, laid over, it lies.
    let m = step(&mut st, &mut game, "lay", 0.3);
    assert_eq!(
        m,
        [
            "speed 0.00 1.00",
            "body pose=sitting",
            "move 10.500 64.000 -3.250",
            "body pose=lying"
        ]
    );
    st.key("key.keyboard.space", true);
    let m = step(&mut st, &mut game, "", 0.1);
    st.key("key.keyboard.space", false);
    assert_eq!(m, ["speed 1.00 1.00", "body"]);
    // nosit: Sit does nothing.
    step(&mut st, &mut game, "nosit", 0.05);
    let m = step(&mut st, &mut game, "sit", 0.1);
    assert!(m.is_empty(), "{m:?}");
    step(&mut st, &mut game, "state", 0.05);
    assert_eq!(printed(&st), ["Seated\ttrue", "Running\tfalse", "Running\tfalse"]);
}

#[test]
fn trip_knocks_the_player_over_and_platformstand_holds_them() {
    let (mut game, mut st) = world(
        "iyc-trip",
        r#"
        local states = {}
        hum.StateChanged:Connect(function(_, new) table.insert(states, new.Name) end)
        command("trip", function()
            hum:ChangeState(Enum.HumanoidStateType.FallingDown)
            root.Velocity = root.CFrame.LookVector * 30
        end)
        command("stun", function() hum.PlatformStand = true end)
        command("unstun", function() hum.PlatformStand = false end)
        command("states", function() print(table.concat(states, " ")) end)
        "#,
    );
    // Pushed the way they face (north) and lying, unable to walk or jump.
    let m = step(&mut st, &mut game, "trip", 0.1);
    assert_eq!(m, ["speed 0.00 0.00", "velocity 0.00 0.00 -1.50", "body pose=lying"]);
    // Up again after a while.
    let m = step(&mut st, &mut game, "", 2.0);
    assert_eq!(m, ["body", "speed 1.00 1.00"]);
    step(&mut st, &mut game, "states", 0.05);
    let m = step(&mut st, &mut game, "stun", 0.1);
    assert_eq!(m, ["speed 0.00 0.00"]);
    let m = step(&mut st, &mut game, "unstun", 0.1);
    assert_eq!(m, ["speed 1.00 1.00"]);
    assert_eq!(printed(&st), ["FallingDown GettingUp Running"]);
}

#[test]
fn walk_speed_and_jump_power_scale_the_game_s_each_on_its_own() {
    let (mut game, mut st) = world(
        "iyc-speed",
        r#"
        command("speed", function() hum.WalkSpeed = 50 end)
        command("jp", function() hum.JumpPower = 100 end)
        command("jh", function() hum.UseJumpPower = false hum.JumpHeight = 7.2 * 4 end)
        command("unspeed", function() hum.WalkSpeed = 16 end)
        "#,
    );
    // WalkSpeed alone leaves the jump as it was.
    assert_eq!(step(&mut st, &mut game, "speed", 0.1), ["speed 3.12 1.00"]);
    assert_eq!(step(&mut st, &mut game, "jp", 0.1), ["speed 3.12 2.00"]);
    // A jump four times as high is twice as fast.
    assert_eq!(step(&mut st, &mut game, "jh", 0.1), ["speed 3.12 2.00"]);
    assert_eq!(step(&mut st, &mut game, "unspeed", 0.1), ["speed 1.00 2.00"]);
}

#[test]
fn noarms_nolegs_naked_and_nohats_take_parts_off_the_players_model() {
    let (mut game, mut st) = world(
        "iyc-parts",
        r#"
        local function gone(names)
            for _, v in pairs(char:GetChildren()) do
                if v:IsA("BasePart") and table.find(names, v.Name) then v:Destroy() end
            end
        end
        command("noarms", function() gone({"Right Arm", "Left Arm"}) end)
        command("nolegs", function() gone({"Right Leg", "Left Leg"}) end)
        command("naked", function()
            for _, v in pairs(char:GetDescendants()) do
                if v:IsA("Clothing") or v:IsA("ShirtGraphic") then v:Destroy() end
            end
        end)
        command("nohats", function()
            for _, v in next, char:GetDescendants() do
                if v:IsA("Accessory") then
                    for _, p in next, v:GetDescendants() do if p:IsA("Weld") then p:Destroy() end end
                end
            end
        end)
        command("noface", function()
            for _, v in pairs(char:GetDescendants()) do
                if v:IsA("Decal") and v.Name == "face" then v:Destroy() end
            end
        end)
        command("nohead", function() char.Head:Destroy() end)
        command("notorso", function() char.Torso:Destroy() end)
        "#,
    );
    assert_eq!(
        step(&mut st, &mut game, "noarms", 0.1),
        ["body hidden=left_arm,right_arm,left_sleeve,right_sleeve"]
    );
    assert_eq!(
        step(&mut st, &mut game, "nolegs", 0.1),
        ["body hidden=left_arm,right_arm,left_sleeve,right_sleeve,left_leg,right_leg,left_pants,right_pants"]
    );
    assert_eq!(
        step(&mut st, &mut game, "naked", 0.1),
        ["body hidden=jacket,left_arm,right_arm,left_sleeve,right_sleeve,left_leg,right_leg,left_pants,right_pants"]
    );
    assert_eq!(
        step(&mut st, &mut game, "nohats", 0.1),
        [
            "body hidden=hat,jacket,left_arm,right_arm,left_sleeve,right_sleeve,left_leg,right_leg,left_pants,right_pants"
        ]
    );
    // The face is the skin's: nothing to take off.
    assert!(step(&mut st, &mut game, "noface", 0.1).is_empty());
    assert_eq!(
        step(&mut st, &mut game, "nohead", 0.1),
        [
            "body hidden=head,hat,jacket,left_arm,right_arm,left_sleeve,right_sleeve,left_leg,right_leg,left_pants,right_pants"
        ]
    );
    // With the torso too, nothing of the model is left: none of it is drawn.
    let m = step(&mut st, &mut game, "notorso", 0.1);
    assert_eq!(m.len(), 1);
    assert!(m[0].ends_with(" gone"), "{m:?}");
    assert!(game.body.gone);
}

#[test]
fn creeper_drophats_and_clearcharappearance_go_by_accessories() {
    let (mut game, mut st) = world(
        "iyc-hats",
        r#"
        command("count", function() print(#hum:GetAccessories()) end)
        command("creeper", function()
            char.Head:FindFirstChildOfClass("SpecialMesh"):Destroy()
            char["Left Arm"]:Destroy()
            char["Right Arm"]:Destroy()
            hum:RemoveAccessories()
        end)
        command("clearchar", function() game.Players.LocalPlayer:ClearCharacterAppearance() end)
        "#,
    );
    step(&mut st, &mut game, "count", 0.05);
    assert_eq!(
        step(&mut st, &mut game, "creeper", 0.1),
        ["body hidden=hat,left_arm,right_arm,left_sleeve,right_sleeve"]
    );
    step(&mut st, &mut game, "count", 0.05);
    assert_eq!(
        step(&mut st, &mut game, "clearchar", 0.1),
        ["body hidden=hat,jacket,left_arm,right_arm,left_sleeve,right_sleeve,left_pants,right_pants"]
    );
    assert_eq!(printed(&st), ["1", "0"]);

    // drophats: the hat leaves the character for the Workspace.
    let (mut game, mut st) = world(
        "iyc-drophats",
        r#"
        command("drophats", function()
            for _, v in pairs(hum:GetAccessories()) do v.Parent = workspace end
        end)
        "#,
    );
    assert_eq!(step(&mut st, &mut game, "drophats", 0.1), ["body hidden=hat"]);
}

#[test]
fn invisible_hides_the_players_model_and_never_parks_the_player_out_of_the_world() {
    let (mut game, mut st) = world(
        "iyc-invisible",
        r#"
        local player = game.Players.LocalPlayer
        local real, ghost = char, nil
        -- Infinite Yield's invisible: a see-through copy plays while the
        -- real character is parked far up, out of sight.
        command("invisible", function()
            real.Archivable = true
            ghost = real:Clone()
            for _, v in pairs(ghost:GetDescendants()) do
                if v:IsA("BasePart") then v.Transparency = v.Name == "HumanoidRootPart" and 1 or 0.5 end
            end
            local at = root.CFrame
            real:MoveTo(Vector3.new(0, math.pi * 1000000, 0))
            task.wait(0.2)
            real.Parent = game:GetService("Lighting")
            ghost.Parent = workspace
            ghost.HumanoidRootPart.CFrame = at
            player.Character = ghost
        end)
        command("visible", function()
            ghost:Destroy()
            player.Character = real
            real.Parent = workspace
        end)
        command("nilchar", function() player.Character.Parent = nil end)
        command("unnilchar", function() player.Character.Parent = workspace end)
        "#,
    );
    let m = step(&mut st, &mut game, "invisible", 0.5);
    assert!(m.iter().all(|l| !l.starts_with("move")), "{m:?}");
    assert!(game.body.gone, "{m:?}");
    let m = step(&mut st, &mut game, "visible", 0.1);
    assert_eq!(m, ["body"]);
    assert_eq!(step(&mut st, &mut game, "nilchar", 0.1), ["body gone"]);
    assert_eq!(step(&mut st, &mut game, "unnilchar", 0.1), ["body"]);
    let p = game.player.as_ref().unwrap();
    assert_eq!((p.x, p.y, p.z), (10.5, 64.0, -3.25));
}

#[test]
fn spin_noanim_and_norotate_turn_and_still_the_model() {
    let (mut game, mut st) = world(
        "iyc-spin",
        r#"
        command("spin", function()
            local spin = Instance.new("BodyAngularVelocity")
            spin.Name = "Spinning"
            spin.Parent = root
            spin.MaxTorque = Vector3.new(0, math.huge, 0)
            spin.AngularVelocity = Vector3.new(0, 20, 0)
        end)
        command("unspin", function() root.Spinning:Destroy() end)
        command("noanim", function() char.Animate.Disabled = true end)
        command("reanim", function() char.Animate.Disabled = false end)
        command("norotate", function() hum.AutoRotate = false end)
        command("unnorotate", function() hum.AutoRotate = true end)
        "#,
    );
    // 20 radians a second, the game's yaw the other way round.
    assert_eq!(step(&mut st, &mut game, "spin", 0.1), ["body spin=-1146"]);
    assert_eq!(game.body.spin.round(), -1146.0);
    assert_eq!(step(&mut st, &mut game, "unspin", 0.1), ["body"]);
    assert_eq!(step(&mut st, &mut game, "noanim", 0.1), ["body still"]);
    assert_eq!(step(&mut st, &mut game, "reanim", 0.1), ["body"]);
    assert_eq!(step(&mut st, &mut game, "norotate", 0.1), ["body fixed"]);
    assert_eq!(step(&mut st, &mut game, "unnorotate", 0.1), ["body"]);
}

#[test]
fn reset_and_respawn_kill_the_character_without_moving_the_player() {
    let (mut game, mut st) = world(
        "iyc-reset",
        r#"
        local player = game.Players.LocalPlayer
        game.Players.RespawnTime = 0.5
        hum.Died:Connect(function() print("died") end)
        player.CharacterAdded:Connect(function(c) print("added", c.Humanoid.Health) end)
        command("reset", function() player.Character.Humanoid:ChangeState(Enum.HumanoidStateType.Dead) end)
        -- Infinite Yield's respawn: below FallenPartsDestroyHeight, its check off meanwhile.
        command("respawn", function()
            local c = player.Character
            local archive = workspace.FallenPartsDestroyHeight
            workspace.FallenPartsDestroyHeight = 0/0
            c.Humanoid.RootPart.Position = Vector3.yAxis * archive
            task.wait(0.1)
            workspace.FallenPartsDestroyHeight = archive
            local hum = c.Humanoid
            repeat task.wait() until not c.Parent or not hum.Parent or hum.Health <= 0
            print("respawned")
        end)
        "#,
    );
    let m = step(&mut st, &mut game, "reset", 1.0);
    assert!(m.iter().all(|l| !l.starts_with("move")), "{m:?}");
    let m = step(&mut st, &mut game, "respawn", 1.0);
    assert!(m.iter().all(|l| !l.starts_with("move")), "{m:?}");
    assert_eq!(printed(&st), ["died", "added\t100", "respawned", "added\t100"]);
    assert!(game.sent.is_empty(), "{:?}", game.sent);
}

#[test]
fn edgejump_breakvelocity_tpwalk_and_cframefly_move_the_player() {
    let (mut game, mut st) = world(
        "iyc-velocity",
        r#"
        local RunService = game:GetService("RunService")
        -- edgejump: off an edge (Freefall, not from Jumping), back where it was and up.
        command("edgejump", function()
            local state, laststate, lastcf
            RunService.RenderStepped:Connect(function()
                laststate = state
                state = hum:GetState()
                if laststate ~= state and state == Enum.HumanoidStateType.Freefall and laststate ~= Enum.HumanoidStateType.Jumping then
                    root.CFrame = lastcf
                    root.Velocity = Vector3.new(root.Velocity.X, hum.JumpPower, root.Velocity.Z)
                end
                lastcf = root.CFrame
            end)
        end)
        command("breakvelocity", function()
            for _, v in ipairs(char:GetDescendants()) do
                if v:IsA("BasePart") then v.Velocity, v.RotVelocity = Vector3.zero, Vector3.zero end
            end
        end)
        command("keep", function() root.Velocity = root.Velocity end)
        -- tpwalk: the character goes the way the keys walk it, each frame.
        local walking
        command("tpwalk", function()
            walking = RunService.Heartbeat:Connect(function(dt)
                char:TranslateBy(hum.MoveDirection * 2 * dt * 10)
            end)
        end)
        command("untpwalk", function() walking:Disconnect() end)
        -- cframefly: the anchored head moved, the character with it.
        command("cframefly", function()
            hum.PlatformStand = true
            char.Head.Anchored = true
            char.Head.CFrame = char.Head.CFrame + Vector3.new(0, 3, 0)
        end)
        command("uncframefly", function() hum.PlatformStand = false char.Head.Anchored = false end)
        "#,
    );
    step(&mut st, &mut game, "edgejump", 0.1);
    // Off the edge: falling, not having jumped. JumpPower is the game's jump in
    // blocks a second: a jump's speed.
    game.pose = pose(false);
    game.player.as_mut().unwrap().velocity = [0.0, -1.0, 0.0];
    let m = step(&mut st, &mut game, "", 0.15);
    assert_eq!(m, ["move 10.500 64.000 -3.250", "velocity 0.00 0.42 0.00"]);
    game.pose = pose(true);
    game.player.as_mut().unwrap().velocity = [4.0, 0.0, 0.0];
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        step(&mut st, &mut game, "breakvelocity", 0.1),
        ["velocity 0.00 0.00 0.00"]
    );
    // Setting it to what it is changes nothing.
    assert!(step(&mut st, &mut game, "keep", 0.1).is_empty());
    // Standing still, tpwalk stays; W held (facing north) takes the player north.
    game.player.as_mut().unwrap().velocity = [0.0; 3];
    assert!(step(&mut st, &mut game, "tpwalk", 0.1).is_empty());
    st.key("key.keyboard.w", true);
    let m = step(&mut st, &mut game, "", 0.2);
    st.key("key.keyboard.w", false);
    assert!(
        m.len() >= 2 && m.iter().all(|l| l.starts_with("move 10.500 64.000 -")),
        "{m:?}"
    );
    assert!(game.player.as_ref().unwrap().z < -4.0);
    step(&mut st, &mut game, "untpwalk", 0.1);
    let z = game.player.as_ref().unwrap().z;
    let m = step(&mut st, &mut game, "cframefly", 0.1);
    assert_eq!(m, [format!("move 10.500 67.000 {z:.3}").as_str(), "speed 0.00 0.00"]);
    assert_eq!(step(&mut st, &mut game, "uncframefly", 0.1), ["speed 1.00 1.00"]);
}

#[test]
fn float_holds_the_player_on_its_platform_and_q_and_e_move_it() {
    let (mut game, mut st) = world(
        "iyc-float",
        r#"
        local point, part = -3.1, nil
        command("float", function()
            part = Instance.new("Part")
            part.Name = "FloatPart"
            part.Parent = char
            part.Transparency = 1
            part.Size = Vector3.new(2, 0.2, 1.5)
            part.Anchored = true
            game:GetService("RunService").PreAnimation:Connect(function()
                if part.Parent then part.CFrame = root.CFrame * CFrame.new(0, point, 0) end
            end)
        end)
        command("down", function() point = -3.6 end)
        command("unfloat", function() part:Destroy() end)
        "#,
    );
    step(&mut st, &mut game, "float", 0.1);
    // Falling: held where the platform is.
    game.pose = pose(false);
    game.player.as_mut().unwrap().velocity = [0.0, -3.0, 0.0];
    let m = step(&mut st, &mut game, "", 0.1);
    assert!(!m.is_empty() && m.iter().all(|l| l == "rise 0.00"), "{m:?}");
    // Rising (a jump off it): free.
    game.player.as_mut().unwrap().velocity = [0.0, 6.0, 0.0];
    assert!(step(&mut st, &mut game, "", 0.1).is_empty());
    // Q: the platform half a stud lower, the player follows it down.
    game.player.as_mut().unwrap().velocity = [0.0, -3.0, 0.0];
    let m = step(&mut st, &mut game, "down", 0.15);
    assert_eq!(m.last().map(String::as_str), Some("rise -0.10"), "{m:?}");
    step(&mut st, &mut game, "unfloat", 0.05);
    assert!(step(&mut st, &mut game, "", 0.1).is_empty());
}

#[test]
fn swim_keeps_the_swimming_state_with_the_others_off() {
    let (mut game, mut st) = world(
        "iyc-swim",
        r#"
        command("swim", function()
            workspace.Gravity = 0
            for _, v in pairs(Enum.HumanoidStateType:GetEnumItems()) do
                if v ~= Enum.HumanoidStateType.None then hum:SetStateEnabled(v, false) end
            end
            hum:ChangeState(Enum.HumanoidStateType.Swimming)
        end)
        command("unswim", function()
            workspace.Gravity = 196.2
            for _, v in pairs(Enum.HumanoidStateType:GetEnumItems()) do hum:SetStateEnabled(v, true) end
        end)
        command("state", function() print(hum:GetState().Name) end)
        "#,
    );
    let m = step(&mut st, &mut game, "swim", 0.2);
    // No gravity, no jump (Jumping is off too).
    assert_eq!(m, ["speed 1.00 0.00", "gravity 0.00"]);
    step(&mut st, &mut game, "state", 0.05);
    let m = step(&mut st, &mut game, "unswim", 0.2);
    assert_eq!(m, ["speed 1.00 1.00", "gravity 1.00"]);
    step(&mut st, &mut game, "state", 0.05);
    assert_eq!(printed(&st), ["Swimming", "Running"]);
}

#[test]
fn god_and_the_commands_minecraft_has_no_match_for_leave_the_player_be() {
    let (mut game, mut st) = world(
        "iyc-none",
        r#"
        -- Infinite Yield's god: the Humanoid swapped for a copy, the Character
        -- left nil (its `char` is an unset global).
        command("god", function()
            local player = game.Players.LocalPlayer
            local copy = hum:Clone()
            copy.Parent = char
            player.Character = nil
            copy:SetStateEnabled(15, false)
            copy:SetStateEnabled(1, false)
            copy:SetStateEnabled(0, false)
            hum:Destroy()
            player.Character = nil
            copy.Health = copy.MaxHealth
            print(char:FindFirstChildOfClass("Humanoid") == copy)
        end)
        command("rest", function()
            local h = char:FindFirstChildOfClass("Humanoid")
            h.HipHeight = 5
            h.MaxSlopeAngle = 30
            char.Head:FindFirstChildOfClass("SpecialMesh"):Destroy() -- blockhead
            for _, child in pairs(char:GetDescendants()) do -- weaken
                if child.ClassName == "Part" then child.CustomPhysicalProperties = PhysicalProperties.new(0, 0.3, 0.5) end
            end
            local anim = Instance.new("Animation") -- dance
            anim.AnimationId = "rbxassetid://27789359"
            local track = h:LoadAnimation(anim)
            track.Looped = true
            track:Play()
            track:AdjustSpeed(2)
            track:Stop()
        end)
        -- toolinvisible: off to a box far above the world.
        command("tinvis", function() char:MoveTo(Vector3.new(0, 10000.5, 0)) end)
        "#,
    );
    assert!(step(&mut st, &mut game, "tinvis", 0.2).is_empty());
    // The character left nil, the player's model stays as it was.
    assert!(step(&mut st, &mut game, "god", 0.2).is_empty());
    let m = step(&mut st, &mut game, "rest", 0.2);
    assert!(m.is_empty(), "{m:?}");
    assert_eq!(printed(&st), ["true"]);
    assert!(game.sent.is_empty(), "{:?}", game.sent);
}
