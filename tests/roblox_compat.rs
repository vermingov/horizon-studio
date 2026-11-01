// What scripts made for Roblox executors (Infinite Yield, Dex) need at
// their top and in their commands: services, enums, older members, and
// Roblox's own behaviour where it differs from a plain call.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::ModState;

fn run(name: &str, src: &str, secs: f64) -> Vec<String> {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    game.write_mod("t.lua", src);
    run_for(&mut st, &mut game, secs);
    let i = mod_index(&st, "t");
    if let ModState::Failed(m, t) = &st.s.mods[i].state {
        panic!("{m}\n{t}\n{:?}", log(&st));
    }
    log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect()
}

#[test]
fn services_enums_and_members_executor_scripts_read_at_their_top() {
    let p = run(
        "compat-top",
        r#"
        for _, s in ipairs({"ExperienceService", "AvatarEditorService", "CaptureService", "ContentProvider",
            "VoiceChatService", "NetworkSettings", "RenderSettings", "UserGameSettings"}) do
            assert(game:GetService(s), s)
        end
        local cs = game:GetService("CaptureService")
        print(typeof(cs.CaptureBegan), typeof(cs.CaptureEnded))
        print(game:GetService("ContentProvider").BaseUrl)
        game:GetService("ContentProvider"):PreloadAsync({})
        print(Enum.ChatVersion.LegacyChatService.Name, Enum.Platform.Windows.Name, Enum.FontSize.Size14.Name)
        local lp = game.Players.LocalPlayer
        print(typeof(lp.OnTeleport), lp.CameraMode.Name, lp.Guest)
        local b = Instance.new("TextButton")
        b.FontSize = Enum.FontSize.Size14
        b.TextWrap = true
        print(b.FontSize.Name, b.TextWrap)
        print(typeof(lp:GetMouse().KeyDown))
        "#,
        0.6,
    );
    assert_eq!(
        p,
        [
            "RBXScriptSignal\tRBXScriptSignal",
            "https://www.roblox.com/",
            "LegacyChatService\tWindows\tSize14",
            "RBXScriptSignal\tClassic\tfalse",
            "Size14\ttrue",
            "RBXScriptSignal",
        ]
    );
}

#[test]
fn invoke_before_on_invoke_is_set_waits_for_it() {
    let p = run(
        "compat-invoke",
        r#"
        local f = Instance.new("BindableFunction")
        task.spawn(function() print("got", f:Invoke(2, 3)) end)
        task.wait(0.2)
        f.OnInvoke = function(a, b) return a + b end
        print("set")
        "#,
        1.5,
    );
    assert_eq!(p, ["set", "got\t5"]);
}

#[test]
fn a_spawned_threads_error_is_printed_and_the_script_goes_on() {
    let p = run(
        "compat-spawn-error",
        r#"
        task.spawn(function() error("in a thread") end)
        print("goes on")
        task.wait(0.1)
        print("still")
        "#,
        1.2,
    );
    assert_eq!(p.first().map(String::as_str), Some("goes on"));
    assert!(p.iter().any(|l| l.contains("in a thread")), "{p:?}");
    assert_eq!(p.last().map(String::as_str), Some("still"));
}

#[test]
fn keypress_reaches_input_began_and_the_mouses_key_down() {
    let p = run(
        "compat-keypress",
        r#"
        local UIS = game:GetService("UserInputService")
        UIS.InputBegan:Connect(function(i) print("began", i.KeyCode.Name) end)
        game.Players.LocalPlayer:GetMouse().KeyDown:Connect(function(k) print("keydown", k) end)
        keypress(0x45) keyrelease(0x45)
        setfpscap(144) print(getfpscap())
        print(typeof(syn.request), getsynasset == getcustomasset, openfile == readfile)
        "#,
        0.6,
    );
    // Handlers run deferred, after the code that fired them.
    assert_eq!(p, ["144", "function\ttrue\ttrue", "began\tE", "keydown\te"]);
}

#[test]
fn roblox_image_ids_ask_the_thumbnails_api() {
    use horizon_studio::game::hstudio_sunc::roblox_image;
    let asset = |id: &str| {
        Some((
            format!("https://thumbnails.roblox.com/v1/assets?assetIds={id}&size=700x700&format=Png"),
            format!(".roblox-assets/{id}.png"),
        ))
    };
    assert_eq!(roblox_image("rbxassetid://5147695474"), asset("5147695474"));
    assert_eq!(roblox_image("http://www.roblox.com/asset/?id=123"), asset("123"));
    assert_eq!(roblox_image("https://www.roblox.com/asset?id=77&x=1"), asset("77"));
    assert_eq!(roblox_image("42"), asset("42"));
    assert_eq!(
        roblox_image("rbxthumb://type=AvatarHeadShot&id=1&w=150&h=150"),
        Some((
            "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds=1&size=150x150&format=Png".into(),
            ".roblox-assets/AvatarHeadShot-1-150.png".into()
        ))
    );
    assert_eq!(roblox_image("rbxthumb://type=Asset&id=9&w=420&h=420"), asset("9"));
    assert_eq!(roblox_image("rbxasset://textures/ui/x.png"), None);
    assert_eq!(roblox_image(""), None);
}

#[test]
fn fly_noclip_and_speed_move_the_player_in_the_client_with_no_command() {
    let mut game = FakeGame::new("compat-move");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local char = game.Players.LocalPlayer.Character
        local root, hum = char.HumanoidRootPart, char.Humanoid
        command("fly", function()
            -- Infinite Yield's way, Roblox's older names and all.
            local bg = Instance.new("BodyGyro", root)
            bg.P = 9e4 bg.maxTorque = Vector3.new(9e9, 9e9, 9e9) bg.cframe = root.CFrame
            local bv = Instance.new("BodyVelocity", root)
            bv.velocity = Vector3.new(0, 20, 0) bv.maxForce = Vector3.new(9e9, 9e9, 9e9)
        end)
        command("land", function() root.BodyVelocity:Destroy() root.BodyGyro:Destroy() end)
        local stepped
        command("noclip", function()
            stepped = game:GetService("RunService").Stepped:Connect(function()
                for _, p in ipairs(char:GetDescendants()) do
                    if p:IsA("BasePart") and p.CanCollide then p.CanCollide = false end
                end
            end)
        end)
        command("clip", function() stepped:Disconnect() end)
        command("speed", function() hum.WalkSpeed = 50 hum.JumpPower = 100 end)
        "#,
    );
    run_for(&mut st, &mut game, 0.7);
    let run = |st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, cmd: &str| {
        let mark = game.moves.len();
        assert!(st.command(cmd), "{cmd}");
        run_for(st, game, 0.3);
        game.moves[mark..].to_vec()
    };
    // Flying: the BodyVelocity's 20 studs a second up, a block a tick, every frame.
    let m = run(&mut st, &mut game, "fly");
    assert!(
        m.len() >= 4 && m.iter().all(|l| l == "velocity 0.00 1.00 0.00"),
        "{m:?}"
    );
    let m = run(&mut st, &mut game, "land");
    assert!(m.iter().all(|l| !l.starts_with("velocity")), "{m:?}");
    // Noclip while the script keeps the parts' CanCollide off; collisions back once it stops.
    let m = run(&mut st, &mut game, "noclip");
    assert_eq!(m, ["noclip true"]);
    let m = run(&mut st, &mut game, "clip");
    assert_eq!(m, ["noclip false"]);
    // WalkSpeed and JumpPower scale the game's own (16 and 50 are its).
    let m = run(&mut st, &mut game, "speed");
    assert_eq!(m, ["speed 3.12 2.00"]);
    // Never a command or a chat line.
    assert!(game.sent.is_empty(), "{:?}", game.sent);
}
