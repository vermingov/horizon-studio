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
    // Roblox's own files aren't asset ids (tests/rbx_images.rs has them).
    assert!(roblox_image("rbxasset://textures/ui/x.png").is_some_and(|(url, _)| !url.contains("thumbnails")));
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

#[test]
fn roblox_s_own_scripts_are_there_for_the_script_functions() {
    let p = run(
        "compat-core",
        r#"
        local module = game:GetService("CoreGui").RobloxGui.Modules.Common.Constants
        local constants = getrenv().require(module)
        local generated = getscriptclosure(module)()
        print(constants ~= generated, constants.BUTTON_HEIGHT == generated.BUTTON_HEIGHT)
        local loaded = false
        for _, m in getloadedmodules() do if m == module then loaded = true end end
        print(loaded)
        local animate = game.Players.LocalPlayer.Character.Animate
        print(animate.ClassName, getsenv(animate).script == animate, type(getscriptbytecode(animate)))
        print(type(loadstring(getscriptbytecode(animate))))
        local copy = animate:Clone()
        local hash = getscripthash(copy)
        copy.Source = "print('Hello, world!')"
        print(hash ~= getscripthash(copy), getscripthash(copy) == getscripthash(copy))
        "#,
        0.8,
    );
    assert_eq!(
        p,
        ["true\ttrue", "true", "LocalScript\ttrue\tstring", "nil", "true\ttrue"]
    );
}

#[test]
fn roblox_s_whole_api_backs_what_the_layer_does_not_implement() {
    let p = run(
        "compat-api",
        r#"
        -- A member of Roblox's API the layer has no code for: its type and default, kept when set.
        local h = game.Players.LocalPlayer.Character.Humanoid
        print(h.SeatPart, h.Sit, h.BreakJointsOnDeath, typeof(h.Seated))
        h.BreakJointsOnDeath = false
        print(h.BreakJointsOnDeath, pcall(function() h.BreakJointsOnDeath = nil end))
        print(pcall(function() h.SeatPart = workspace end))
        -- A class only the API has, an API service, and every enum.
        local p = Instance.new("ProximityPrompt")
        print(p.ActionText, p.HoldDuration, p:IsA("Instance"), typeof(p.Triggered))
        print(game:GetService("GroupService").ClassName, Enum.CreatorType.Group.Value)
        -- Humanoid states, settings, product info, physics.
        print(h:GetStateEnabled(Enum.HumanoidStateType.Dead), h:SetStateEnabled(15, false), h:GetStateEnabled(15))
        print(settings().Rendering.ClassName, UserSettings():GetService("UserGameSettings").ClassName)
        print(game:GetService("MarketplaceService"):GetProductInfo(game.PlaceId).Name)
        print(PhysicalProperties.new(Enum.Material.Wood).Density, typeof(PhysicalProperties.new(1, 0.3, 0.5)))
        -- Animations, and the R6 rig's details.
        local anim = Instance.new("Animation")
        local track = h:LoadAnimation(anim)
        track:Play()
        print(track.IsPlaying, #h:GetPlayingAnimationTracks(), #h.Animator:GetPlayingAnimationTracks())
        track:Stop()
        local char = h.Parent
        print(char.Head.Mesh.ClassName, char.Torso.Neck.ClassName, char.Animate.sit.SitAnim.ClassName)
        -- Chat goes to the game's chat.
        game:GetService("TextChatService").TextChannels.RBXGeneral:SendAsync("hello from roblox")
        -- next goes on past a key the loop removed.
        local t = {1, 2, 3, x = 1}
        local n = 0
        for k in pairs(t) do n = n + 1 if k == 3 then table.remove(t, 3) end end
        print(n)
        "#,
        0.8,
    );
    assert_eq!(
        p,
        [
            "nil\tfalse\ttrue\tRBXScriptSignal",
            "false\tfalse\tt.lua:6: Unable to assign property BreakJointsOnDeath. bool expected, got nil",
            "false\tt.lua:7: Unable to assign property SeatPart. Property is read only",
            "Interact\t0\ttrue\tRBXScriptSignal",
            "GroupService\t1",
            "true\tnil\tfalse",
            "RenderSettings\tUserGameSettings",
            "Minecraft",
            "0.35\tPhysicalProperties",
            "true\t1\t1",
            "SpecialMesh\tMotor6D\tAnimation",
            "4",
        ]
    );
}

#[test]
fn a_script_takes_the_character_apart_as_in_roblox() {
    let mut game = FakeGame::new("compat-swap");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local lp = game.Players.LocalPlayer
        local char = lp.Character
        char.Archivable = true
        local copy = char:Clone()
        copy.Name = "Copy"
        copy.Parent = workspace
        lp.Character = copy
        task.wait(0.1)
        print(copy.Humanoid.RootPart == copy.HumanoidRootPart, workspace.CurrentCamera ~= nil)
        workspace.CurrentCamera:Destroy()
        task.wait(0.1)
        print(workspace.CurrentCamera ~= nil)
        -- Its Humanoid gone, the character is dead: a new one comes RespawnTime later.
        game.Players.RespawnTime = 0.2
        copy.Humanoid:Destroy()
        local new = lp.CharacterAdded:Wait()
        print(new ~= copy, new:FindFirstChildOfClass("Humanoid") ~= nil)
        "#,
    );
    run_for(&mut st, &mut game, 1.5);
    let p: Vec<String> = log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect();
    assert_eq!(p, ["true\ttrue", "true", "true\ttrue"], "{:?}", log(&st));
    assert!(game.sent.is_empty());
}

#[test]
fn a_roblox_script_s_chat_message_goes_to_the_game_s_chat() {
    let mut game = FakeGame::new("compat-chat");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        game:GetService("TextChatService").TextChannels.RBXGeneral:SendAsync("hello from roblox")
        game.ReplicatedStorage.DefaultChatSystemChatEvents.SayMessageRequest:FireServer("and the old chat", "All")
        game:GetService("TextChatService").TextChannels.RBXGeneral:SendAsync("/e dance")
        "#,
    );
    run_for(&mut st, &mut game, 0.8);
    assert_eq!(game.sent, ["hello from roblox", "and the old chat"]);
}

#[test]
fn saveinstance_writes_a_model_studio_and_the_reader_open() {
    let mut game = FakeGame::new("compat-save");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local char = game.Players.LocalPlayer.Character
        local part = Instance.new("Part")
        part.Name = "Saved <&> part"
        part.Size = Vector3.new(2, 3, 4)
        part.Color = Color3.fromRGB(255, 0, 0)
        part.Parent = char
        print(saveinstance(char), SaveInstance(part, "one"), saveinstance(part, "dex.txt"), ypcall == pcall)
        print(decompile(char.Animate):sub(1, 21))
        "#,
    );
    run_for(&mut st, &mut game, 0.8);
    let p: Vec<String> = log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect();
    assert_eq!(
        p,
        ["Player1.rbxmx\tone.rbxmx\tdex.rbxmx\ttrue", "local Character = scr"],
        "{:?}",
        log(&st)
    );
    let dir = st.s.rbx.workspace_dir.clone();
    let bytes = std::fs::read(dir.join("Player1.rbxmx")).unwrap();
    let file = horizon_studio::game::hstudio_rbx_file::read(&bytes).unwrap();
    let names: Vec<&str> = file.roots.iter().map(|&r| file.items[r].class.as_str()).collect();
    assert_eq!(names, ["Model"]);
    let part = file
        .items
        .iter()
        .find(|i| i.properties.iter().any(|(k, v)| k == "Name" && matches!(v, horizon_studio::game::hstudio_rbx_file::PropValue::String(s) if s == b"Saved <&> part")))
        .expect("the part, its name escaped and read back");
    assert_eq!(part.class, "Part");
    assert!(
        part.properties.iter().any(|(k, _)| k == "Size"),
        "{:?}",
        part.properties
    );
    let animate = file.items.iter().find(|i| i.class == "LocalScript").unwrap();
    assert!(animate.properties.iter().any(|(k, v)| k == "Source"
        && matches!(v, horizon_studio::game::hstudio_rbx_file::PropValue::String(s) if s.starts_with(b"local Character"))));
}

#[test]
fn lighting_workspace_and_camera_reach_the_game() {
    let mut game = FakeGame::new("compat-effects");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local Lighting = game:GetService("Lighting")
        command("fullbright", function()
            -- Infinite Yield's fullbright.
            Lighting.Brightness = 2
            Lighting.ClockTime = 14
            Lighting.FogEnd = 100000
            Lighting.GlobalShadows = false
            Lighting.OutdoorAmbient = Color3.fromRGB(128, 128, 128)
        end)
        command("dark", function()
            Lighting.Brightness = 1 Lighting.GlobalShadows = true
            Lighting.OutdoorAmbient = Color3.fromRGB(70, 70, 70)
            Lighting.FogEnd = 64
        end)
        command("gravity", function() workspace.Gravity = 0 end)
        command("fov", function() workspace.CurrentCamera.FieldOfView = 100 end)
        command("look", function()
            local cam = workspace.CurrentCamera
            cam.CFrame = CFrame.lookAt(Vector3.new(0, 0, 0), Vector3.new(1, 0, 0))
        end)
        command("turn", function() mousemoverel(100, 0) end)
        "#,
    );
    run_for(&mut st, &mut game, 0.7);
    let run = |st: &mut horizon_studio::game::hstudio::Studio, game: &mut FakeGame, cmd: &str| {
        let mark = game.moves.len();
        assert!(st.command(cmd), "{cmd}");
        run_for(st, game, 0.2);
        game.moves[mark..].to_vec()
    };
    let m = run(&mut st, &mut game, "fullbright");
    assert!(m.contains(&"fullbright 1.00".to_owned()), "{m:?}");
    assert!(m.contains(&"time Some(8000)".to_owned()), "{m:?}");
    assert!(m.contains(&"fog None".to_owned()), "{m:?}");
    let m = run(&mut st, &mut game, "dark");
    assert!(m.contains(&"fullbright 0.00".to_owned()), "{m:?}");
    assert!(m.contains(&"fog Some(64.0)".to_owned()), "{m:?}");
    assert_eq!(run(&mut st, &mut game, "gravity"), ["gravity 0.00"]);
    assert_eq!(run(&mut st, &mut game, "fov"), ["fov Some(100.0)"]);
    // Looking along +x: Minecraft's yaw -90; the view is at the origin for
    // that frame (as Roblox's camera, which then follows the player again).
    assert_eq!(
        run(&mut st, &mut game, "look"),
        ["view at 0.00 0.00 0.00 -90.0 0.0", "look -90.0 0.0", "view game"]
    );
    let m = run(&mut st, &mut game, "turn");
    assert_eq!(m, ["turn 15.0 0.0"]);
    assert!(game.sent.is_empty(), "{:?}", game.sent);
}

#[test]
fn a_highlight_is_drawn_over_its_part() {
    let mut game = FakeGame::new("compat-highlight");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local p = Instance.new("Part")
        p.Anchored = true
        p.Size = Vector3.new(2, 2, 2)
        p.CFrame = CFrame.new(10.5, 65.62, -13.25)
        p.Parent = workspace
        local h = Instance.new("Highlight")
        h.FillColor = Color3.new(1, 0, 0)
        h.OutlineColor = Color3.new(0, 1, 0)
        h.Adornee = p
        h.Parent = p
        "#,
    );
    run_for(&mut st, &mut game, 0.7);
    st.hud(&mut game);
    let red = game
        .drawn
        .iter()
        .filter(|l| l.starts_with("rect") && l.contains("ff0000"))
        .count();
    let green = game
        .drawn
        .iter()
        .filter(|l| l.starts_with("rect") && l.contains("ff00"))
        .count();
    assert!(red >= 1 && green >= 4, "{:?}", game.drawn);
}

#[test]
fn roblox_s_older_globals_and_rarer_data_types_are_there() {
    let p = run(
        "compat-types",
        r#"
        print(version(), Version == version, Wait == wait, Spawn == spawn, Delay == delay, Game == game)
        print(typeof(ElapsedTime()), stats() == game:GetService("Stats"))
        local v = Vector3int16.new(1.7, -2.9, 40000)
        print(typeof(v), v, v + Vector3int16.new(1, 1, 1), v == Vector3int16.new(1, -2, -25536))
        print(Vector2int16.new(3, 4) * 2, typeof(Region3int16.new()))
        print(Faces.new(Enum.NormalId.Top, Enum.NormalId.Front), Faces.new(Enum.NormalId.Top).Top, Axes.new(Enum.Axis.X, Enum.NormalId.Top))
        local d = DateTime.fromUniversalTime(2024, 2, 29, 13, 5, 9, 250)
        print(typeof(d), d, d.UnixTimestamp, d:FormatUniversalTime("dddd, MMMM D YYYY h:mm A"))
        print(DateTime.fromIsoDate("2024-02-29T13:05:09.250Z") == d, d:ToUniversalTime().Month, DateTime.now().UnixTimestamp > 1.7e9)
        local s = SharedTable.new({ a = 1 }) s.b = 2
        print(typeof(s), s.a, s.b, SharedTable.size(s))
        print(typeof(Content.fromUri("rbxassetid://1")), typeof(PathWaypoint.new(Vector3.new())), typeof(FloatCurveKey.new(0, 1)))
        print(pcall(function() return v.W end))
        "#,
        0.6,
    );
    assert_eq!(
        p,
        [
            "0.697.0.6970574\ttrue\ttrue\ttrue\ttrue\ttrue",
            "number\ttrue",
            "Vector3int16\t1, -2, -25536\t2, -1, -25535\ttrue",
            "6, 8\tRegion3int16",
            "Top, Front\ttrue\tX, Y",
            "DateTime\t2024-02-29T13:05:09.250Z\t1709211909\tThursday, February 29 2024 1:05 PM",
            "true\t2\ttrue",
            "SharedTable\t1\t2\t2",
            "Content\tPathWaypoint\tFloatCurveKey",
            "false\tt.lua:14: W is not a valid member of Vector3int16",
        ]
    );
}

#[test]
fn json_encode_writes_null_for_what_json_cannot_hold_and_methods_take_lowercase_names() {
    let p = run(
        "compat-json",
        r#"
        local HttpService = game:GetService("HttpService")
        print(HttpService:JSONEncode({ v = Vector3.new(1, 2, 3) }), HttpService:JSONEncode({ print, 1, workspace }))
        print(Color3.new(0, 0, 0):lerp(Color3.new(1, 1, 1), 0.5).R, CFrame.new(1, 2, 3):inverse().X)
        "#,
        0.6,
    );
    assert_eq!(p, ["{\"v\":null}\t[null,1,null]", "0.5\t-1"]);
}

#[test]
fn esp_in_a_folder_in_core_gui_draws_its_boxes_and_name_tags() {
    // Infinite Yield's ESP: a Folder in CoreGui with a BoxHandleAdornment per
    // part (coloured by the older BrickColor Color) and a BillboardGui tag.
    let mut game = FakeGame::new("compat-esp");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local p = Instance.new("Part")
        p.Anchored = true
        p.Size = Vector3.new(2, 2, 2)
        p.CFrame = CFrame.new(10.5, 65.62, -13.25)
        p.Parent = workspace
        local holder = Instance.new("Folder")
        holder.Name = "Alex_ESP"
        holder.Parent = game:GetService("CoreGui")
        local box = Instance.new("BoxHandleAdornment")
        box.Adornee = p
        box.AlwaysOnTop = true
        box.Size = p.Size
        box.Transparency = 0.3
        box.Color = BrickColor.new("Bright red")
        box.Parent = holder
        local tag = Instance.new("BillboardGui")
        tag.Adornee = p
        tag.Size = UDim2.new(0, 100, 0, 150)
        tag.AlwaysOnTop = true
        tag.Parent = holder
        local label = Instance.new("TextLabel")
        label.BackgroundTransparency = 1
        label.Size = UDim2.new(0, 100, 0, 100)
        label.Text = "Name: Alex"
        label.Parent = tag
        print(box.Visible)
        "#,
    );
    run_for(&mut st, &mut game, 0.7);
    st.hud(&mut game);
    let box_red = game
        .drawn
        .iter()
        .any(|d| d.starts_with("rect") && d.ends_with(&format!("b3{:06x}", 0xc4281c)));
    let tag = game.drawn.iter().any(|d| d.starts_with("text Name: Alex"));
    assert!(box_red && tag, "{:?}", game.drawn);
    assert_eq!(
        log(&st).iter().filter(|l| l.as_str() == "[t] true").count(),
        1,
        "adornments are Visible"
    );
}

#[test]
fn an_unturned_part_reads_no_negative_zero() {
    // Dex++'s Properties shows Rotation as Roblox gives it: 0, 0, 0.
    let p = run(
        "compat-zero",
        r#"
        local part = Instance.new("Part")
        print(part.Rotation, part.Orientation)
        part.CFrame = CFrame.Angles(0, math.rad(90), 0)
        print(math.round(part.Orientation.Y))
        "#,
        0.6,
    );
    assert_eq!(p, ["0, 0, 0\t0, 0, 0", "90"]);
}
