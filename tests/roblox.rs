// The Roblox layer on a fake game: Roblox scripts as script mods. Instances,
// properties and their events, services, deferred signals, WaitForChild,
// tweens, ScreenGuis drawn and clicked, UserInputService, Parts in the
// world, raycasts through blocks, and what a mod made going when it stops.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{Camera, ModState, Studio};
use horizon_studio::game::hstudio_rbx_draw::{pointer_button, pointer_moved};

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

fn hud(st: &mut Studio, game: &mut FakeGame) -> Vec<String> {
    game.drawn.clear();
    st.hud(game);
    std::mem::take(&mut game.drawn)
}

#[test]
fn instances_properties_and_the_tree() {
    let mut game = FakeGame::new("rbx-tree");
    let mut st = studio(&mut game);
    let src = r#"
        local folder = Instance.new("Folder")
        folder.Name = "Stuff"
        folder.Parent = workspace
        local part = Instance.new("Part", folder)
        part.Name = "Brick"
        part.Size = Vector3.new(2, 3, 4)
        part.Position = Vector3.new(1, 2, 3)
        part.BrickColor = BrickColor.new("Bright red")
        part.Anchored = true
        print(part.Parent == folder, workspace.Stuff.Brick == part, part:GetFullName())
        print(part.ClassName, part:IsA("BasePart"), part:IsA("GuiObject"), typeof(part), tostring(part))
        print(part.Position, part.CFrame.Position, part.Size, part.Color, part.BrickColor)
        print(#folder:GetChildren(), #workspace:GetDescendants() > 3, folder:FindFirstChild("Brick") == part)
        print(folder:FindFirstChildOfClass("Part") == part, part:FindFirstAncestor("Stuff") == folder)
        local copy = part:Clone()
        print(copy.Parent, copy.Name, copy.Size, copy == part)
        part:SetAttribute("Hp", 10)
        print(part:GetAttribute("Hp"), part:GetAttributes().Hp)
        part:AddTag("Lava")
        print(game:GetService("CollectionService"):HasTag(part, "Lava"), #game:GetService("CollectionService"):GetTagged("Lava"))
        print(pcall(function() part.Size = "big" end))
        print(pcall(function() return part.Nope end))
        print(pcall(function() Instance.new("Nope") end))
        folder:Destroy()
        print(part.Parent, folder.Parent, pcall(function() folder.Parent = workspace end))
        print(game.Players.LocalPlayer.Name, game:GetService("Players").LocalPlayer == game.Players.LocalPlayer)
        print(workspace == game.Workspace, workspace.Parent == game, game:GetService("Workspace") == workspace)
        print(script.Name, script.ClassName, script.Parent.Name)
        print(typeof(game.run), game.ticks() >= 0)
    "#;
    load(&mut game, &mut st, "tree", src, 0.6);
    let p = printed(&st, "tree");
    let want = [
        "true    true    Workspace.Stuff.Brick",
        "Part    true    false    Instance    Brick",
        "1, 2, 3    1, 2, 3    2, 3, 4    0.768627, 0.156863, 0.109804    Bright red",
        "1    true    true",
        "true    true",
        "nil    Brick    2, 3, 4    false",
        "10    10",
        "true    1",
    ];
    for (i, w) in want.iter().enumerate() {
        assert_eq!(p.get(i).map(String::as_str), Some(*w), "line {i}: {p:#?}");
    }
    assert!(
        p[8].starts_with("false    ") && p[8].contains("Unable to assign property Size. Vector3 expected, got string"),
        "{}",
        p[8]
    );
    assert!(
        p[9].contains("Nope is not a valid member of Part \"Workspace.Stuff.Brick\""),
        "{}",
        p[9]
    );
    assert!(
        p[10].contains("Unable to create an Instance of type \"Nope\""),
        "{}",
        p[10]
    );
    assert!(
        p[11].starts_with("nil    nil    false    ") && p[11].contains("locked"),
        "{}",
        p[11]
    );
    assert_eq!(p[12], "Player1    true");
    assert_eq!(p[13], "true    true    true");
    assert_eq!(p[14], "tree    LocalScript    PlayerScripts");
    assert_eq!(p[15], "function    true");
}

#[test]
fn signals_are_deferred_and_wait_works() {
    let mut game = FakeGame::new("rbx-signals");
    let mut st = studio(&mut game);
    let src = r#"
        local order = {}
        local v = Instance.new("NumberValue")
        v.Changed:Connect(function(new) table.insert(order, "changed " .. new) end)
        local conn = v:GetPropertyChangedSignal("Name"):Connect(function() table.insert(order, "name " .. v.Name) end)
        v.Value = 5
        v.Name = "Score"
        table.insert(order, "after set")
        local once = 0
        v.Changed:Once(function() once += 1 end)
        local ev = Instance.new("BindableEvent")
        ev.Event:Connect(function(a, b) table.insert(order, "event " .. a .. b) end)
        ev:Fire("x", "y")
        task.spawn(function()
            local a = ev.Event:Wait()
            table.insert(order, "waited " .. a)
        end)
        task.delay(0.1, function() ev:Fire("late", "!") end)
        local folder = Instance.new("Folder")
        task.spawn(function()
            local c = folder:WaitForChild("Kid")
            table.insert(order, "got " .. c.Name)
            local none = folder:WaitForChild("Never", 0.2)
            table.insert(order, "timeout " .. tostring(none))
        end)
        task.delay(0.15, function()
            local k = Instance.new("Part")
            k.Name = "Kid"
            k.Parent = folder
            v.Value = 6
            v.Value = 7
            conn:Disconnect()
            v.Name = "Ignored"
            print(conn.Connected)
        end)
        command("order", function() return table.concat(order, ", ") end)
        local fn = Instance.new("BindableFunction")
        fn.OnInvoke = function(n) return n * 2 end
        print(fn:Invoke(21))
    "#;
    load(&mut game, &mut st, "sig", src, 1.5);
    assert!(st.command("order"));
    st.frame(&mut game, 0.016);
    assert_eq!(
        game.chats.last().map(String::as_str),
        Some(
            "after set, changed 5, name Score, event xy, waited late, event late!, got Kid, changed 6, changed 7, timeout nil"
        ),
        "{:?}",
        log(&st)
    );
    assert!(printed(&st, "sig").contains(&"42".to_owned()));
    assert!(printed(&st, "sig").contains(&"false".to_owned()));
}

#[test]
fn tweens_move_properties_and_complete() {
    let mut game = FakeGame::new("rbx-tween");
    let mut st = studio(&mut game);
    let src = r#"
        local TweenService = game:GetService("TweenService")
        local f = Instance.new("Frame")
        f.Position = UDim2.new(0, 0, 0, 0)
        f.BackgroundTransparency = 0
        local t = TweenService:Create(f, TweenInfo.new(0.5, Enum.EasingStyle.Linear), {
            Position = UDim2.new(0, 100, 0, 50),
            BackgroundTransparency = 1,
        })
        print(t.PlaybackState, typeof(t), t:IsA("Tween"))
        t.Completed:Connect(function(state) print("done", state, f.Position, f.BackgroundTransparency) end)
        t:Play()
        task.wait(0.25)
        print("half", f.Position.X.Offset > 30 and f.Position.X.Offset < 70)
        print(TweenService:GetValue(0.5, Enum.EasingStyle.Quad, Enum.EasingDirection.In))
        print(pcall(function() TweenService:Create(f, TweenInfo.new(), { Name = "x" }) end))
    "#;
    load(&mut game, &mut st, "tw", src, 1.6);
    let p = printed(&st, "tw");
    assert_eq!(p[0], "Enum.PlaybackState.Begin    Instance    true");
    assert!(p.contains(&"half    true".to_owned()), "{p:?}");
    assert!(p.contains(&"0.25".to_owned()), "{p:?}");
    assert!(
        p.contains(&"done    Enum.PlaybackState.Completed    {0, 100}, {0, 50}    1".to_owned()),
        "{p:?}"
    );
    assert!(
        p.iter()
            .any(|l| l.starts_with("false") && l.contains("cannot be tweened")),
        "{p:?}"
    );
}

const UI: &str = r#"
    local Players = game:GetService("Players")
    local player = Players.LocalPlayer
    local gui = Instance.new("ScreenGui")
    gui.Name = "Shop"
    gui.ResetOnSpawn = false
    gui.Parent = player:WaitForChild("PlayerGui")
    local frame = Instance.new("Frame")
    frame.Size = UDim2.fromOffset(200, 100)
    frame.Position = UDim2.fromScale(0.5, 0.5)
    frame.AnchorPoint = Vector2.new(0.5, 0.5)
    frame.BackgroundColor3 = Color3.fromRGB(30, 30, 30)
    frame.Parent = gui
    Instance.new("UICorner", frame).CornerRadius = UDim.new(0, 8)
    local list = Instance.new("UIListLayout", frame)
    list.Padding = UDim.new(0, 4)
    local title = Instance.new("TextLabel")
    title.Text = "Welcome"
    title.Size = UDim2.new(1, 0, 0, 30)
    title.BackgroundTransparency = 1
    title.TextColor3 = Color3.new(1, 1, 1)
    title.Parent = frame
    local button = Instance.new("TextButton")
    button.Name = "Buy"
    button.Text = "Buy"
    button.Size = UDim2.new(1, 0, 0, 40)
    button.Modal = true
    button.Parent = frame
    local clicks = 0
    button.MouseButton1Click:Connect(function()
        clicks += 1
        title.Text = "Bought " .. clicks
    end)
    button.MouseEnter:Connect(function() print("enter") end)
    frame:GetPropertyChangedSignal("AbsoluteSize"):Connect(function() print("size", frame.AbsoluteSize) end)
"#;

#[test]
fn screen_guis_are_laid_out_drawn_and_clicked() {
    let mut game = FakeGame::new("rbx-gui");
    let mut st = studio(&mut game);
    load(&mut game, &mut st, "shop", UI, 0.6);
    let drawn = hud(&mut st, &mut game);
    // The frame is centred on the 960 x 540 dp screen, the label at its top.
    assert!(
        drawn.contains(&"rect [380.0, 220.0, 200.0, 100.0] r8 ff1e1e1e".to_owned()),
        "{drawn:#?}"
    );
    assert!(
        drawn
            .iter()
            .any(|d| d.starts_with("text Welcome @") && d.ends_with("ffffff")),
        "{drawn:#?}"
    );
    assert!(drawn.iter().any(|d| d.starts_with("text Buy @")), "{drawn:#?}");
    run_for(&mut st, &mut game, 0.1);
    assert!(
        printed(&st, "shop").contains(&"size    200, 100".to_owned()),
        "{:?}",
        log(&st)
    );
    // A Modal button frees the pointer: the GUI's own screen opens.
    assert!(st.accept_screen(), "the Roblox GUI asked for the pointer");
    assert!(st.s.rbx.pointer.free);
    // Hover and click the button (at y 220 + 30 + 4 = 254, 40 tall).
    pointer_moved(&mut st.s.rbx, 480.0, 270.0);
    pointer_button(&mut st.s.rbx, true, true);
    pointer_button(&mut st.s.rbx, true, false);
    run_for(&mut st, &mut game, 0.1);
    assert!(printed(&st, "shop").contains(&"enter".to_owned()));
    let drawn = hud(&mut st, &mut game);
    let _ = drawn;
    game.drawn.clear();
    st.s.screen.as_ref().expect("still open");
    // The screen's draw draws the GUI; the HUD then leaves it alone.
    assert!(hud(&mut st, &mut game).iter().all(|d| !d.contains("Bought")));
    let gui = st.s.rbx.player_gui;
    let shop = st.s.rbx.child_named(gui, "Shop").unwrap();
    let frame = st.s.rbx.children(shop)[0];
    let title = st.s.rbx.children(frame)[2];
    assert_eq!(st.s.rbx.raw_get(title, "Text").plain_text(), "Bought 1");
    // When the mod stops, its GUI goes with it.
    std::fs::remove_file(game.mods_dir().join("shop.lua")).unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.rbx.children(gui).is_empty());
    assert!(st.s.screen.is_none(), "the GUI's screen closed");
}

#[test]
fn text_boxes_take_typing() {
    let mut game = FakeGame::new("rbx-textbox");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local box = Instance.new("TextBox", gui)
        box.Size = UDim2.fromOffset(200, 30)
        box.Text = ""
        box.PlaceholderText = "Your name"
        box.FocusLost:Connect(function(enter) print("lost", enter, box.Text) end)
        box.Focused:Connect(function() print("focused") end)
    "#;
    load(&mut game, &mut st, "tb", src, 0.6);
    let drawn = hud(&mut st, &mut game);
    assert!(drawn.iter().any(|d| d.starts_with("text Your name")), "{drawn:#?}");
    pointer_moved(&mut st.s.rbx, 10.0, 10.0);
    pointer_button(&mut st.s.rbx, true, true);
    horizon_studio::game::hstudio_rbx_draw::type_text(&mut st.s.rbx, "Steve");
    horizon_studio::game::hstudio_rbx_draw::unfocus(&mut st.s.rbx, true);
    run_for(&mut st, &mut game, 0.1);
    let p = printed(&st, "tb");
    assert_eq!(p, ["focused", "lost    true    Steve"]);
}

#[test]
fn user_input_service_and_run_service() {
    let mut game = FakeGame::new("rbx-input");
    let mut st = studio(&mut game);
    let src = r#"
        local UIS = game:GetService("UserInputService")
        local RunService = game:GetService("RunService")
        UIS.InputBegan:Connect(function(input, processed)
            if input.KeyCode == Enum.KeyCode.E then print("E down", processed, UIS:IsKeyDown(Enum.KeyCode.E)) end
            if input.UserInputType == Enum.UserInputType.MouseButton1 then print("click") end
        end)
        UIS.InputEnded:Connect(function(input) if input.KeyCode == Enum.KeyCode.E then print("E up") end end)
        local beats = 0
        RunService.Heartbeat:Connect(function(dt) beats += 1 end)
        RunService:BindToRenderStep("cam", Enum.RenderPriority and 200 or 200, function() end)
        game:GetService("ContextActionService"):BindAction("Sprint", function(name, state)
            print(name, state)
        end, false, Enum.KeyCode.LeftShift)
        task.delay(0.3, function() print("beats", beats > 3) end)
        print(RunService:IsClient(), RunService:IsServer())
    "#;
    load(&mut game, &mut st, "inp", src, 0.6);
    st.key("key.keyboard.e", true);
    st.frame(&mut game, 0.05);
    st.key("key.keyboard.e", false);
    st.key("key.mouse.left", true);
    st.key("key.keyboard.left.shift", true);
    run_for(&mut st, &mut game, 0.4);
    let p = printed(&st, "inp");
    for want in [
        "true    false",
        "E down    false    true",
        "E up",
        "click",
        "Sprint    Enum.UserInputState.Begin",
        "beats    true",
    ] {
        assert!(p.contains(&want.to_owned()), "{want}: {p:#?}");
    }
}

#[test]
fn the_character_camera_and_raycasts_follow_the_game() {
    let mut game = FakeGame::new("rbx-world");
    game.camera = Some(Camera {
        pos: [10.5, 65.62, -3.25],
        yaw: 0.0,
        pitch: 0.0,
        fov: 70.0,
    });
    for z in -10..10 {
        game.blocks.insert((10, 63, z), "minecraft:grass_block".into());
    }
    game.blocks.insert((10, 64, 0), "minecraft:stone".into());
    let mut st = studio(&mut game);
    let src = r#"
        task.wait()
        local player = game.Players.LocalPlayer
        local char = player.Character
        local root = char:WaitForChild("HumanoidRootPart")
        print(math.floor(root.Position.X), math.floor(root.Position.Y), char.Humanoid.Health, char.Humanoid.MaxHealth)
        local down = workspace:Raycast(Vector3.new(10.5, 70, 5.5), Vector3.new(0, -20, 0))
        print(down.Instance == workspace.Terrain, down.Position.Y, down.Normal, down.Material)
        local ahead = workspace:Raycast(Vector3.new(10.5, 64.5, -3.5), Vector3.new(0, 0, 10))
        print(ahead.Position.Z, ahead.Normal, ahead.Block)
        print(workspace:Raycast(Vector3.new(10.5, 70, 5.5), Vector3.new(0, 1, 0)))
        local part = Instance.new("Part")
        part.Size = Vector3.new(2, 2, 2)
        part.Position = Vector3.new(10.5, 66, 3)
        part.Anchored = true
        part.Parent = workspace
        local hit = workspace:Raycast(Vector3.new(10.5, 66, -2), Vector3.new(0, 0, 10))
        print(hit.Instance == part, hit.Position.Z)
        local cam = workspace.CurrentCamera
        local at, on = cam:WorldToViewportPoint(Vector3.new(10.5, 65.62, 5))
        print(math.floor(at.X), math.floor(at.Y), on, cam.ViewportSize)
        print(cam.CFrame.LookVector)
    "#;
    load(&mut game, &mut st, "world", src, 0.7);
    let p = printed(&st, "world");
    assert_eq!(p[0], "10    64    100    100");
    assert_eq!(p[1], "true    64    0, 1, 0    Enum.Material.Grass");
    assert_eq!(p[2], "0    0, 0, -1    minecraft:stone");
    assert_eq!(p[3], "nil");
    assert_eq!(p[4], "true    2");
    assert_eq!(p[5], "480    270    true    960, 540");
    assert_eq!(p[6], "0, 0, 1");
    // The part is drawn in the world: six faces.
    game.world_quads.clear();
    st.world(&mut game);
    // The fake game's lit quads are its plain ones.
    assert_eq!(game.world_quads.len(), 6, "the part's six faces");
}

#[test]
fn a_reload_drops_what_the_mod_made() {
    let mut game = FakeGame::new("rbx-reload");
    let mut st = studio(&mut game);
    let src = "local f = Instance.new('Folder') f.Name = 'Mine' f.Parent = workspace\nworkspace.ChildAdded:Connect(function(c) print('added', c.Name) end)\n";
    load(&mut game, &mut st, "made", src, 0.6);
    let ws = st.s.rbx.workspace;
    assert!(st.s.rbx.child_named(ws, "Mine").is_some());
    game.write_mod("made.lua", "print('second')\n");
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.rbx.child_named(ws, "Mine").is_none());
    load(
        &mut game,
        &mut st,
        "other",
        "local f = Instance.new('Folder') f.Name = 'Theirs' f.Parent = workspace\n",
        0.6,
    );
    assert!(
        !printed(&st, "made").iter().any(|l| l.starts_with("added")),
        "the old connection went"
    );
}

#[test]
fn luau_ui_script_with_types_runs() {
    let mut game = FakeGame::new("rbx-luau");
    let mut st = studio(&mut game);
    let src = r#"
        --!strict
        local Players = game:GetService("Players")
        type Config = { title: string, width: number }
        local config: Config = { title = "Menu", width = 240 }
        local gui = Instance.new("ScreenGui")
        gui.Parent = Players.LocalPlayer:WaitForChild("PlayerGui") :: PlayerGui
        local function label(text: string, order: number): TextLabel
            local l = Instance.new("TextLabel")
            l.Text = `{order}. {text}`
            l.LayoutOrder = order
            l.Size = UDim2.new(1, 0, 0, if order == 1 then 30 else 20)
            return l
        end
        local frame = Instance.new("Frame")
        frame.Size = UDim2.fromOffset(config.width, 200)
        frame.Parent = gui
        Instance.new("UIListLayout").Parent = frame
        for i, name in { "Play", "Settings", "Quit" } do
            if name == "Settings" then continue end
            label(name, i).Parent = frame
        end
        print(#frame:GetChildren())
    "#;
    load(&mut game, &mut st, "luau", src, 0.6);
    assert_eq!(printed(&st, "luau"), ["3"]);
    let drawn = hud(&mut st, &mut game);
    assert!(drawn.iter().any(|d| d.starts_with("text 1. Play")), "{drawn:#?}");
    assert!(drawn.iter().any(|d| d.starts_with("text 3. Quit")), "{drawn:#?}");
}

/// Writes module `id` (its files by their paths in it) and lets it run `seconds`.
fn load_module(game: &mut FakeGame, st: &mut Studio, id: &str, files: &[(&str, &str)], seconds: f64) {
    let dir = game.mods_dir().join(id);
    for (rel, src) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, src).unwrap();
    }
    run_for(st, game, seconds);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}\n{:?}", log(st));
    }
}

#[test]
fn a_modules_files_are_module_scripts_under_script() {
    let mut game = FakeGame::new("rbx-module-scripts");
    let mut st = studio(&mut game);
    load_module(
        &mut game,
        &mut st,
        "kit",
        &[
            (
                "main.lua",
                "print(script.ClassName, script.lib.ClassName, script.lib.util.ClassName, script.shared.ClassName)\n\
                 local a = require(script.lib)\n\
                 print(a == require('lib'), a.shared == require(script.shared), libglobal)\n\
                 print(require(script.lib.util).deep, require('lib.util') == require(script.lib.util))\n",
            ),
            (
                "lib.lua",
                "print('lib runs', script.Name, script.ClassName, script.Parent.Name, script.Parent.ClassName)\n\
                 libglobal = 'set in lib'\n\
                 local shared = require(script.Parent.shared)\n\
                 return { shared = shared }\n",
            ),
            (
                "lib/util.lua",
                "print('util', script.Name, script.Parent.Name)\nreturn { deep = 'deep' }\n",
            ),
            ("shared.lua", "return { n = 1 }\n"),
        ],
        0.6,
    );
    let lines = printed(&st, "kit");
    let has = |l: &str| lines.iter().any(|x| x == l);
    assert!(
        has("LocalScript    ModuleScript    ModuleScript    ModuleScript"),
        "{lines:?}"
    );
    assert!(
        has("lib runs    lib    ModuleScript    kit    LocalScript"),
        "{lines:?}"
    );
    assert_eq!(lines.iter().filter(|l| l.starts_with("lib runs")).count(), 1, "once");
    assert!(has("true    true    set in lib"), "{lines:?}");
    assert!(has("deep    true"), "{lines:?}");
    assert!(
        has("util    util    lib"),
        "a file and a folder of one name nest: {lines:?}"
    );
    // getloadedmodules has them, and after a reload only the live ones.
    let loaded = |st: &Studio| {
        let rbx = &st.s.rbx;
        let live = rbx.loaded_modules.iter().all(|(id, _)| rbx.try_inst(*id).is_some());
        (rbx.loaded_modules.len(), live)
    };
    assert_eq!(loaded(&st), (3, true));
    std::fs::write(
        game.mods_dir().join("kit").join("shared.lua"),
        "return { n = 2, more = true }\n",
    )
    .unwrap();
    run_for(&mut st, &mut game, 0.6);
    assert!(st.s.mods[mod_index(&st, "kit")].running());
    assert_eq!(loaded(&st), (3, true), "no holes after a reload");
    let out: Vec<String> = st
        .run_console(
            &mut game,
            "local n = 0 for _, m in getloadedmodules() do n = n + 1 end return n",
        )
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    assert_eq!(out, ["3"]);
}

#[test]
fn the_script_functions_know_a_modules_files() {
    let mut game = FakeGame::new("rbx-module-senv");
    let mut st = studio(&mut game);
    load_module(
        &mut game,
        &mut st,
        "kit",
        &[
            (
                "main.lua",
                "local lib = require(script.lib)\n\
                 print('main', getsenv(script.lib) == lib.env, getsenv(script.lib).script == script.lib)\n\
                 local ok, why = pcall(getsenv, script.later)\n\
                 print('later', ok, why, getscripthash(script.later))\n\
                 print('failed', pcall(require, script.bad), (pcall(getsenv, script.bad)))\n\
                 local running = {}\n\
                 for _, s in getrunningscripts() do running[s] = true end\n\
                 print('running', running[script] == true, running[script.lib] == true, running[script.later] == true)\n",
            ),
            (
                "lib.lua",
                "print('lib', getsenv(script) == getfenv(), type(getscriptclosure(script)), getscriptclosure(script) ~= getscriptclosure(script))\n\
                 print('code', #getscriptbytecode(script) > 0, #getscripthash(script), getscripthash(script) ~= getscripthash(script.Parent))\n\
                 return { env = getfenv() }\n",
            ),
            ("later.lua", "return 1\n"),
            ("bad.lua", "error('bad file')\n"),
        ],
        0.6,
    );
    let lines = printed(&st, "kit");
    let has = |l: &str| lines.iter().any(|x| x == l);
    // Required, a file's ModuleScript runs: its environment (its own
    // `script`, the mod's globals), its code and its hash.
    assert!(has("lib    true    function    true"), "{lines:?}");
    assert!(has("code    true    96    true"), "{lines:?}");
    assert!(has("main    true    true"), "{lines:?}");
    // Not required yet, it doesn't: getsenv errors, the others give nil.
    assert!(
        has("later    false    kit/main.lua:3: getsenv: the script is not running    nil"),
        "{lines:?}"
    );
    assert!(has("running    true    true    false"), "{lines:?}");
    // Nor does one whose top level failed.
    assert!(has("failed    false    false"), "{lines:?}");
}

#[test]
fn another_mods_module_script_is_not_required() {
    let mut game = FakeGame::new("rbx-module-other");
    let mut st = studio(&mut game);
    load_module(
        &mut game,
        &mut st,
        "a",
        &[("main.lua", "print('a runs')\n"), ("lib.lua", "return 1\n")],
        0.6,
    );
    load(
        &mut game,
        &mut st,
        "b",
        "print(pcall(require, game.Players.LocalPlayer.PlayerScripts.a.lib))\n",
        0.6,
    );
    let lines = printed(&st, "b");
    assert!(lines[0].starts_with("false"), "{lines:?}");
    assert!(
        lines[0].contains("PlayerScripts.a.lib is a file of the mod 'a': use import(\"a\")"),
        "{lines:?}"
    );
}

#[test]
fn the_middle_button_is_mouse_button_3_on_a_gui() {
    use horizon_studio::game::hstudio_ui::{Input, handle_screen};
    let mut game = FakeGame::new("rbx-middle");
    let mut st = studio(&mut game);
    let src = format!(
        "{UI}\nlocal UIS = game:GetService(\"UserInputService\")\nUIS.InputBegan:Connect(function(input) print(\"began \" .. input.UserInputType.Name) end)\nUIS.InputEnded:Connect(function(input) print(\"ended \" .. input.UserInputType.Name) end)\n"
    );
    load(&mut game, &mut st, "shop", &src, 0.6);
    hud(&mut st, &mut game);
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen(), "the GUI's screen");
    handle_screen(&mut st, &mut game, Input::Middle { down: true });
    assert!(st.s.rbx.pointer.buttons[2]);
    handle_screen(&mut st, &mut game, Input::Middle { down: false });
    assert!(!st.s.rbx.pointer.buttons[2]);
    run_for(&mut st, &mut game, 0.1);
    let lines = printed(&st, "shop");
    assert!(lines.contains(&"began MouseButton3".to_owned()), "{lines:?}");
    assert!(lines.contains(&"ended MouseButton3".to_owned()), "{lines:?}");
    assert!(
        !lines.iter().any(|l| l.contains("MouseButton2")),
        "not the right button: {lines:?}"
    );
}

#[test]
fn space_and_the_backquote_are_key_codes_on_a_gui() {
    use horizon_studio::game::hstudio_ui::{Input, Key, Mods, handle_screen};
    let mut game = FakeGame::new("rbx-backquote");
    let mut st = studio(&mut game);
    let src = format!(
        "{UI}\nlocal UIS = game:GetService(\"UserInputService\")\nUIS.InputBegan:Connect(function(input) print(\"began \" .. input.KeyCode.Name) end)\n"
    );
    load(&mut game, &mut st, "shop", &src, 0.6);
    hud(&mut st, &mut game);
    run_for(&mut st, &mut game, 0.1);
    assert!(st.accept_screen(), "the GUI's screen");
    for c in [' ', '`'] {
        let key = Key::Char(c);
        let mods = Mods::default();
        handle_screen(
            &mut st,
            &mut game,
            Input::Key {
                key,
                down: true,
                repeat: false,
                mods,
            },
        );
        handle_screen(
            &mut st,
            &mut game,
            Input::Key {
                key,
                down: false,
                repeat: false,
                mods,
            },
        );
    }
    run_for(&mut st, &mut game, 0.1);
    let lines = printed(&st, "shop");
    assert!(lines.contains(&"began Space".to_owned()), "{lines:?}");
    assert!(lines.contains(&"began Backquote".to_owned()), "{lines:?}");
}
