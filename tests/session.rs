// A session is the time on one server or world, as a Roblox game is: leaving
// it (a disconnect, a kick, a rejoin or a teleport, quitting to the title)
// ends everything the scripts did there. What they changed of the game is
// undone, the DataModel, the interpreter and the executor's globals start
// afresh, the user's mods start again, and only queue_on_teleport's code
// (with the teleport's data) goes on to the next server.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ClientAction, LogKind, ModState, ServerInfo, Studio};

fn server(address: &str) -> ServerInfo {
    ServerInfo {
        address: address.into(),
        name: address.into(),
        brand: "Paper".into(),
        own_world: false,
    }
}

/// The player on play.example.net, the studio started (on the server).
fn on_server(name: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    game.client.server = Some(server("play.example.net"));
    game.give(&[(0, "minecraft:diamond_pickaxe", 1)]);
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    (game, st)
}

/// Runs console code, then lets a few frames pass: the lines it logged.
fn lua(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    let mark = st.s.log_seq;
    st.run_console(game, src);
    run_for(st, game, 0.2);
    let n = (st.s.log_seq - mark) as usize;
    let start = st.s.log.len().saturating_sub(n);
    st.s.log
        .iter()
        .skip(start)
        .filter(|l| l.kind != LogKind::Input)
        .map(|l| l.text.clone())
        .collect()
}

/// The last move of a kind the game was asked for.
fn last<'a>(game: &'a FakeGame, prefix: &str) -> Option<&'a str> {
    game.moves
        .iter()
        .rev()
        .find(|m| m.starts_with(prefix))
        .map(String::as_str)
}

/// The moves since the session's end.
fn since_end(game: &FakeGame) -> Vec<String> {
    let at = game
        .moves
        .iter()
        .rposition(|m| m == "session end")
        .expect("no session end");
    game.moves[at + 1..].to_vec()
}

// What a session of Infinite Yield and the like does, run from the
// executor: the character's arms off and the rest invisible, fullbright,
// the field of view, gravity, a free camera, the chat hidden, the graphics
// lowered, the volume down, tools in extra hotbar slots, a Drawing, hooks,
// the executor's globals, a thread that loops and a GUI in CoreGui.
const SESSION: &str = r#"
    local Players, Lighting = game:GetService("Players"), game:GetService("Lighting")
    local lp = Players.LocalPlayer
    local char = lp.Character
    char["Right Arm"]:Destroy()
    char["Left Arm"]:Destroy()
    for _, p in ipairs(char:GetDescendants()) do
        if p:IsA("BasePart") then p.Transparency = 1 end
    end
    Lighting.Brightness = 2
    Lighting.ClockTime = 14
    Lighting.FogEnd = 100000
    Lighting.GlobalShadows = false
    Lighting.OutdoorAmbient = Color3.fromRGB(128, 128, 128)
    workspace.CurrentCamera.FieldOfView = 100
    workspace.Gravity = 50
    workspace.CurrentCamera.CameraType = Enum.CameraType.Scriptable
    workspace.CurrentCamera.CFrame = CFrame.new(0, 80, 0)
    game:GetService("StarterGui"):SetCoreGuiEnabled(Enum.CoreGuiType.Chat, false)
    settings().Rendering.QualityLevel = 1
    UserSettings():GetService("UserGameSettings").MasterVolume = 0.2
    for _, n in ipairs({"Sword", "Bomb"}) do
        local t = Instance.new("Tool") t.Name = n t.Parent = lp.Backpack
    end
    local d = Drawing.new("Square") d.Visible = true d.Size = Vector2.new(50, 50)
    hookmetamethod(Vector3.new(), "__add", function() return "hooked" end)
    local old
    old = hookmetamethod(game, "__namecall", function(self, ...) return old(self, ...) end)
    hookfunction(warn, function() print("warn hooked") end)
    getgenv().IY_LOADED = true
    local gui = Instance.new("ScreenGui") gui.Name = "IY" gui.Parent = game:GetService("CoreGui")
    task.spawn(function() while true do task.wait(0.1) print("still running") end end)
"#;

/// What the session sees of the game, as the next session's scripts read it.
const LOOK: &str = r#"
    local lp = game.Players.LocalPlayer
    local char = lp.Character
    local cam = workspace.CurrentCamera
    print("arms", char:FindFirstChild("Right Arm") ~= nil, char:FindFirstChild("Left Arm") ~= nil, char.Head.Transparency)
    print("view", cam.FieldOfView, workspace.Gravity, cam.CameraType.Name, game.Lighting.GlobalShadows)
    print("gui", game.CoreGui:FindFirstChild("IY") ~= nil, #lp.Backpack:GetChildren(), lp.Backpack:FindFirstChild("Sword") ~= nil)
    print("genv", getgenv().IY_LOADED, typeof(Vector3.new(1, 0, 0) + Vector3.new(0, 1, 0)))
    print("chat", game:GetService("StarterGui"):GetCoreGuiEnabled(Enum.CoreGuiType.Chat), UserSettings():GetService("UserGameSettings").MasterVolume)
"#;

#[test]
fn leaving_and_rejoining_undoes_everything_the_scripts_did() {
    let (mut game, mut st) = on_server("session-all");
    let before = lua(&mut st, &mut game, LOOK);
    assert!(log(&st).iter().all(|l| !l.contains("error")), "{:?}", log(&st));
    lua(&mut st, &mut game, SESSION);
    assert!(
        !log(&st).iter().any(|l| l.contains("rror")),
        "the session's code failed: {:?}",
        log(&st)
    );
    run_for(&mut st, &mut game, 0.3);
    // It all reached the game.
    assert!(
        last(&game, "body").is_some_and(|b| b.contains("left_arm")),
        "{:?}",
        game.moves
    );
    assert_eq!(last(&game, "fov"), Some("fov Some(100.0)"));
    assert_eq!(last(&game, "gravity"), Some("gravity 0.25"));
    assert!(
        last(&game, "view").is_some_and(|v| v.starts_with("view at")),
        "{:?}",
        game.moves
    );
    assert!(last(&game, "fullbright").is_some_and(|f| f != "fullbright 0.00"));
    assert!(game.client_done.contains(&ClientAction::Hud("chat".into(), false)));
    assert!(game.client_done.contains(&ClientAction::LowGraphics(true)));
    assert_eq!(game.client.master_volume, 0.2);
    assert_eq!(st.s.rbx.drawings.len(), 1);
    assert_eq!(st.s.rbx.tools.order.len(), 2);
    assert!(log(&st).iter().any(|l| l == "still running"));

    // The player leaves for the title screen: the session ends, once.
    game.client_done.clear();
    game.leave();
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(game.session_ends, 1);
    assert_eq!(st.s.session.ends, 1);
    // What the scripts asked of the game, undone.
    assert_eq!(last(&game, "body"), Some("body"));
    assert_eq!(last(&game, "fullbright"), Some("fullbright 0.00"));
    assert_eq!(last(&game, "time"), Some("time None"));
    assert_eq!(last(&game, "fog"), Some("fog None"));
    assert_eq!(last(&game, "fov"), Some("fov None"));
    assert_eq!(last(&game, "gravity"), Some("gravity 1.00"));
    assert_eq!(last(&game, "view"), Some("view game"));
    assert!(
        game.client_done.contains(&ClientAction::Hud("chat".into(), true)),
        "{:?}",
        game.client_done
    );
    assert!(game.client_done.contains(&ClientAction::LowGraphics(false)));
    assert_eq!(game.client.master_volume, 1.0);
    // Nothing of the layer's session is left: no Drawing, no tool, no thread.
    assert!(st.s.rbx.drawings.is_empty());
    assert!(st.s.rbx.tools.order.is_empty());
    assert!(st.s.console_mod.tasks.is_empty());
    let ran = log(&st).iter().filter(|l| *l == "still running").count();
    run_for(&mut st, &mut game, 0.5);
    assert_eq!(log(&st).iter().filter(|l| *l == "still running").count(), ran);

    // Back on the server: as it was before the scripts ran, and none of it
    // comes back by itself.
    game.join(server("play.example.net"));
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(game.session_ends, 1, "joining is no end");
    assert!(
        since_end(&game).iter().all(|m| !m.starts_with("body hidden")),
        "{:?}",
        since_end(&game)
    );
    assert_eq!(lua(&mut st, &mut game, LOOK), before);
    assert_eq!(before[0], "arms    true    true    1");
    assert_eq!(before[1], "view    70    196.2    Custom    true");
    assert_eq!(before[2], "gui    false    0    false");
    assert_eq!(before[3], "genv    nil    Vector3");
    assert_eq!(before[4], "chat    true    0");
    // The hooks are gone: warn is Roblox's own again.
    lua(&mut st, &mut game, "warn('plain')");
    assert!(!log(&st).iter().any(|l| l == "warn hooked"));
    assert!(st.s.rbx.drawings.is_empty());
}

#[test]
fn queued_code_and_teleport_data_reach_the_next_session() {
    let (mut game, mut st) = on_server("session-teleport");
    lua(
        &mut st,
        &mut game,
        r#"
        getgenv().IY_LOADED = true
        queue_on_teleport("print('queued ran', game.JobId, getgenv().IY_LOADED, game:GetService('TeleportService'):GetLocalPlayerTeleportData().note)")
        local TeleportService = game:GetService("TeleportService")
        TeleportService:Teleport(game.PlaceId, game.Players.LocalPlayer, {note = "hello", f = function() end})
        "#,
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(
        game.client_done.contains(&ClientAction::Rejoin),
        "{:?}",
        game.client_done
    );
    // The game leaves, waits for the server to let go, and joins again.
    game.leave();
    run_for(&mut st, &mut game, 0.5);
    assert_eq!(game.session_ends, 1);
    assert!(!log(&st).iter().any(|l| l.starts_with("queued ran")));
    game.join(server("play.example.net"));
    run_for(&mut st, &mut game, 0.3);
    assert!(
        log(&st)
            .iter()
            .any(|l| l == "queued ran    play.example.net    nil    hello"),
        "{:?}",
        log(&st)
    );
    // Once: another rejoin runs nothing more.
    game.leave();
    run_for(&mut st, &mut game, 0.2);
    game.join(server("play.example.net"));
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(log(&st).iter().filter(|l| l.starts_with("queued ran")).count(), 1);
    assert_eq!(game.session_ends, 2);
}

#[test]
fn a_kick_s_screen_keeps_the_session_until_the_player_rejoins() {
    let (mut game, mut st) = on_server("session-kick");
    // Infinite Yield's autorejoin, cut down: on the kick's error, a rejoin.
    lua(
        &mut st,
        &mut game,
        r#"
        game:GetService("GuiService").ErrorMessageChanged:Connect(function(why)
            print("kicked:", why)
            task.wait(0.2)
            game:GetService("TeleportService"):TeleportToPlaceInstance(game.PlaceId, game.JobId, game.Players.LocalPlayer)
        end)
        game.Players.LocalPlayer.Character["Right Arm"]:Destroy()
        "#,
    );
    game.kicked("Kicked for spamming");
    run_for(&mut st, &mut game, 1.0);
    // The scripts still ran over the Disconnected screen, as over Roblox's
    // kick prompt: the error, and the rejoin.
    assert!(
        log(&st).iter().any(|l| l == "kicked:    Kicked for spamming"),
        "{:?}",
        log(&st)
    );
    assert!(game.client_done.contains(&ClientAction::Rejoin));
    assert_eq!(game.session_ends, 0);
    // The rejoin: a new session, the old one ended once.
    game.join(server("play.example.net"));
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(game.session_ends, 1);
    assert_eq!(last(&game, "body"), Some("body"));
    assert_eq!(
        lua(
            &mut st,
            &mut game,
            "print(game.Players.LocalPlayer.Character:FindFirstChild('Right Arm') ~= nil)"
        ),
        ["true"]
    );
    // A kick then the menus ends it there.
    game.kicked("Server closed");
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(game.session_ends, 1);
    game.leave();
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(game.session_ends, 2);
    // In the menus, with nothing to leave, nothing more ends.
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(game.session_ends, 2);
}

#[test]
fn the_mods_start_again_fresh_and_run_in_the_menus() {
    // The studio starts on the title screen, as the game does.
    let mut game = FakeGame::new("session-mods");
    game.leave();
    let mut st = studio(&mut game);
    game.write_mod(
        "m.lua",
        r#"
        count = (count or 0) + 1
        getgenv().runs = (getgenv().runs or 0) + 1
        print("started", count, getgenv().runs, game.JobId)
        on("unload", function() print("unloaded") end)
        command("ping", function() return "pong" end)
        "#,
    );
    run_for(&mut st, &mut game, 0.7);
    let printed = |st: &Studio| -> Vec<String> {
        log(st)
            .iter()
            .filter_map(|l| l.strip_prefix("[m] ").map(|s| s.replace("    ", "\t")))
            .collect()
    };
    assert_eq!(printed(&st), ["started\t1\t1\t"]);
    // Joining from the menus: the mod goes on as it is.
    game.join(server("play.example.net"));
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(printed(&st), ["started\t1\t1\t"]);
    assert_eq!(game.session_ends, 0);
    // Leaving: it stops (its unload handler runs) and starts again, fresh,
    // in the menus, where its command works.
    game.leave();
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(printed(&st), ["started\t1\t1\t", "unloaded", "started\t1\t1\t"]);
    let i = mod_index(&st, "m");
    assert_eq!(st.s.mods[i].state, ModState::Running);
    assert!(st.command("ping"));
    // A mod turned off stays off.
    st.set_enabled(&mut game, i, false);
    game.join(server("other.example.net"));
    run_for(&mut st, &mut game, 0.2);
    game.leave();
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(st.s.mods[i].state, ModState::Off);
}

#[test]
fn a_world_of_the_player_s_own_is_a_session_too() {
    let mut game = FakeGame::new("session-world");
    game.client.server = Some(ServerInfo {
        address: String::new(),
        name: "New World".into(),
        brand: String::new(),
        own_world: true,
    });
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    lua(
        &mut st,
        &mut game,
        "workspace.CurrentCamera.FieldOfView = 30 getgenv().x = 1",
    );
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(last(&game, "fov"), Some("fov Some(30.0)"));
    game.leave();
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(game.session_ends, 1);
    assert_eq!(last(&game, "fov"), Some("fov None"));
    assert_eq!(lua(&mut st, &mut game, "print(getgenv().x)"), ["nil"]);
    // session_end is safe to call again: nothing is left to undo.
    st.session_end(&mut game);
    assert_eq!(
        lua(&mut st, &mut game, "print(workspace.CurrentCamera.FieldOfView)"),
        ["70"]
    );
}
