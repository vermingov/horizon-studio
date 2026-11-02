// Infinite Yield's server, chat, GUI, settings and voice commands, as the
// Roblox calls they make: what Roblox's own client does for its scripts is
// done by Minecraft's (hstudio_rbx_client). The server is game.JobId, chat
// lines are TextChatService's messages, a kick and a teleport leave and
// rejoin the server, the settings and the HUD are the game's, other
// players' voices are muted in the voice chat.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ClientAction, EntityInfo, ModState, PlayerEntry, ServerInfo, Studio};
use horizon_studio::game::hstudio_rbx_client::{parse_chat, place_id, whisper};

const ALEX: u128 = 0x42;

fn server(address: &str) -> ServerInfo {
    ServerInfo {
        address: address.into(),
        name: address.into(),
        brand: "Paper".into(),
        own_world: false,
    }
}

/// A server with Steve (the player) and Alex near him.
fn world(name: &str) -> FakeGame {
    let mut game = FakeGame::new(name);
    let p = game.player.as_mut().unwrap();
    p.name = "Steve".into();
    p.uuid = 1;
    let listed = |name: &str, uuid: u128, entity: Option<u32>, local: bool| PlayerEntry {
        uuid,
        name: name.into(),
        display_name: name.into(),
        entity,
        local,
        latency: 40,
    };
    game.players = Some(vec![
        listed("Steve", 1, None, true),
        listed("Alex", ALEX, Some(12), false),
    ]);
    game.entities = vec![EntityInfo {
        id: 12,
        kind: "minecraft:player".into(),
        name: "Alex".into(),
        pos: [12.5, 64.0, -3.25],
        width: 0.6,
        height: 1.8,
        eye: 1.62,
        health: 20.0,
        max_health: 20.0,
        speed: 0.1,
        on_ground: true,
        ..EntityInfo::default()
    }];
    game.client.server = Some(server("play.example.net"));
    game
}

fn printed(st: &Studio, id: &str) -> Vec<String> {
    let prefix = format!("[{id}] ");
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix(&prefix).map(|s| s.replace("    ", "\t")))
        .collect()
}

fn load(game: &mut FakeGame, st: &mut Studio, id: &str, src: &str, seconds: f64) {
    game.write_mod(&format!("{id}.lua"), src);
    // The mod is found at the next scan, then runs `seconds`.
    run_for(st, game, 0.6 + seconds);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}\n{:?}", log(st));
    }
}

#[test]
fn the_server_is_the_job_and_its_own_place() {
    let mut game = world("iym-server");
    let mut st = studio(&mut game);
    let src = r#"
        local MarketplaceService = game:GetService("MarketplaceService")
        print(game.JobId, game.PlaceId == game.GameId, game.PlaceId > 0)
        print(MarketplaceService:GetProductInfo(game.PlaceId).Name, game.PlaceId)
    "#;
    load(&mut game, &mut st, "t", src, 0.3);
    let place = place_id(&server("play.example.net"));
    assert_eq!(
        printed(&st, "t"),
        [
            "play.example.net\ttrue\ttrue".to_owned(),
            format!("play.example.net\t{place}")
        ]
    );
    // The same server is the same place each time; another is another.
    assert_eq!(place, place_id(&server("Play.Example.NET")));
    assert_ne!(place, place_id(&server("other.example.net")));
    // Out of the server (kicked), the place stays the last one's.
    game.client.server = None;
    game.client.disconnected = Some("Kicked".into());
    run_for(&mut st, &mut game, 0.2);
    st.run_console(&mut game, "print(game.JobId, game.PlaceId)");
    assert_eq!(log(&st).last().unwrap(), &format!("play.example.net    {place}"));
}

#[test]
fn chat_lines_parse_to_who_said_what() {
    let names = ["Steve".to_owned(), "Alex".to_owned()];
    let said = |line: &str| parse_chat(line, &names);
    let alex = |text: &str, whisper: bool| Some(("Alex".to_owned(), text.to_owned(), whisper));
    assert_eq!(said("<Alex> hello there"), alex("hello there", false));
    assert_eq!(said("<[Admin] alex> hi"), alex("hi", false));
    assert_eq!(said("\u{a7}7[Member] \u{a7}fAlex\u{a7}7: gg"), alex("gg", false));
    assert_eq!(said("Alex \u{bb} anyone?"), alex("anyone?", false));
    assert_eq!(said("Alex whispers to you: psst"), alex("psst", true));
    assert_eq!(said("[Server] Restarting in 5 minutes"), None);
    assert_eq!(said("Bob: not on this server"), None);
    // Roblox's whisper command is Minecraft's private message.
    assert_eq!(
        whisper("/w Alex meet at spawn").as_deref(),
        Some("/msg Alex meet at spawn")
    );
    assert_eq!(whisper("/whisper Alex hi").as_deref(), Some("/msg Alex hi"));
    assert_eq!(whisper("/e dance"), None);
}

#[test]
fn chat_lines_are_text_chat_messages_and_player_chatted() {
    let mut game = world("iym-chat");
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        local TextChatService = game:GetService("TextChatService")
        TextChatService.MessageReceived:Connect(function(m)
            local p = Players:GetPlayerByUserId(m.TextSource.UserId)
            print("message", p and p.Name, m.Text, m.Status == Enum.TextChatMessageStatus.Success, m.TextChannel.Name)
        end)
        Players:WaitForChild("Alex").Chatted:Connect(function(text, to)
            print("chatted", text, to == Players.LocalPlayer)
        end)
        Players.LocalPlayer.Chatted:Connect(function(text) print("me", text) end)
        -- Infinite Yield's whisper and chat go out as Minecraft's.
        TextChatService.TextChannels.RBXGeneral:SendAsync("/w Alex psst")
        TextChatService.TextChannels.RBXGeneral:SendAsync("hello")
    "#;
    load(&mut game, &mut st, "t", src, 0.4);
    for line in [
        "<Alex> hi Steve",
        "Alex whispers to you: secret",
        "<Steve> ;fly",
        "[Server] tick",
    ] {
        st.chat_line(line);
    }
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(game.sent, ["/msg Alex psst", "hello"]);
    assert_eq!(
        printed(&st, "t"),
        [
            "message\tAlex\thi Steve\ttrue\tRBXGeneral",
            "chatted\thi Steve\tfalse",
            "message\tAlex\tsecret\ttrue\tRBXGeneral",
            "chatted\tsecret\ttrue",
            "message\tSteve\t;fly\ttrue\tRBXGeneral",
            "me\t;fly",
        ]
    );
}

#[test]
fn bubble_chat_draws_lines_over_the_speakers_heads() {
    let mut game = world("iym-bubble");
    let mut st = studio(&mut game);
    let src = r#"
        local TextChatService = game:GetService("TextChatService")
        local alex = game.Players:WaitForChild("Alex")
        repeat task.wait() until alex.Character
        TextChatService.BubbleChatConfiguration.Enabled = true
    "#;
    load(&mut game, &mut st, "t", src, 0.5);
    st.chat_line("<Alex> look up here");
    run_for(&mut st, &mut game, 0.2);
    st.run_console(
        &mut game,
        "local b = game.CoreGui.BubbleChat.Alex print(b.ClassName, b.Adornee == game.Players.Alex.Character.Head, b.TextLabel.Text)",
    );
    assert_eq!(log(&st).last().unwrap(), "BillboardGui    true    look up here");
    // It goes after Roblox's 15 seconds; off, no more come.
    run_for(&mut st, &mut game, 15.5);
    st.run_console(&mut game, "print(#game.CoreGui.BubbleChat:GetChildren())");
    assert_eq!(log(&st).last().unwrap(), "0");
    st.run_console(
        &mut game,
        "game:GetService('TextChatService').BubbleChatConfiguration.Enabled = false",
    );
    st.chat_line("<Alex> again");
    run_for(&mut st, &mut game, 0.2);
    st.run_console(&mut game, "print(#game.CoreGui.BubbleChat:GetChildren())");
    assert_eq!(log(&st).last().unwrap(), "0");
}

#[test]
fn a_kick_and_shutdown_leave_and_a_teleport_here_rejoins() {
    let mut game = world("iym-teleport");
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        local TeleportService = game:GetService("TeleportService")
        local GuiService = game:GetService("GuiService")
        TeleportService.TeleportInitFailed:Connect(function(p, result, why) print("failed", p.Name, result.Name) end)
        GuiService.ErrorMessageChanged:Connect(function(why)
            print("error", why, GuiService:GetErrorMessage())
            -- Infinite Yield's rejoin, as its autorejoin runs it.
            Players.LocalPlayer:Kick("\nRejoining...")
            task.wait(0.3)
            TeleportService:TeleportToPlaceInstance(game.PlaceId, game.JobId, Players.LocalPlayer)
        end)
        Players.LocalPlayer:Kick("bye")
        game:Shutdown()
        TeleportService:Teleport(123)
        queue_on_teleport("print('queued ran', game.JobId)")
    "#;
    load(&mut game, &mut st, "t", src, 0.3);
    assert_eq!(
        game.client_done,
        [
            ClientAction::Leave(Some("You were kicked from this experience: bye".into())),
            ClientAction::Leave(None),
        ]
    );
    assert!(
        game.chats
            .iter()
            .any(|c| c.starts_with("[Roblox] Teleport failed: place 123 is no Minecraft server"))
    );
    // The game says it disconnected: the error, and the rejoin.
    game.client_done.clear();
    game.client.server = None;
    game.client.disconnected = Some("Kicked for spamming".into());
    run_for(&mut st, &mut game, 1.2);
    assert_eq!(
        game.client_done,
        [
            ClientAction::Leave(Some("You were kicked from this experience: Rejoining...".into())),
            ClientAction::Rejoin,
        ]
    );
    // Back on the server: the queued script runs there.
    game.client.disconnected = None;
    game.client.server = Some(server("play.example.net"));
    run_for(&mut st, &mut game, 0.2);
    let p = printed(&st, "t");
    assert_eq!(
        p,
        [
            "failed\tSteve\tGameNotFound",
            "error\tKicked for spamming\tKicked for spamming"
        ],
        "{:?}",
        log(&st)
    );
    assert!(
        log(&st).iter().any(|l| l == "queued ran    play.example.net"),
        "{:?}",
        log(&st)
    );
}

#[test]
fn a_teleport_can_be_cancelled_and_another_server_is_refused() {
    let mut game = world("iym-cancel");
    let mut st = studio(&mut game);
    let src = r#"
        local TeleportService = game:GetService("TeleportService")
        game.Players.LocalPlayer.OnTeleport:Connect(function(state) print(state.Name) end)
        TeleportService:Teleport(game.PlaceId)
        TeleportService:TeleportCancel()
        TeleportService:TeleportToPlaceInstance(game.PlaceId, "9b2c1d5e-0000-4000-8000-000000000000")
        print(pcall(function() TeleportService:TeleportAsync(game.PlaceId, {game.Players.LocalPlayer}) end))
    "#;
    load(&mut game, &mut st, "t", src, 1.0);
    assert!(game.client_done.is_empty(), "{:?}", game.client_done);
    let p = printed(&st, "t");
    assert_eq!(p[0], "false\tt.lua:7: TeleportAsync can only be called from the server");
    assert_eq!(&p[1..], ["Started", "Failed", "Failed"]);
    assert!(
        game.chats
            .iter()
            .any(|c| c.contains("there is no other server of this one to join"))
    );
}

#[test]
fn settings_screenshots_and_the_hud_are_the_game_s() {
    let mut game = world("iym-settings");
    let mut st = studio(&mut game);
    let src = r#"
        local StarterGui = game:GetService("StarterGui")
        UserSettings():GetService("UserGameSettings").MasterVolume = 0.3
        settings().Rendering.QualityLevel = 1
        settings().Rendering.QualityLevel = Enum.QualityLevel.Automatic
        game:GetService("GuiService"):ToggleFullscreen()
        game:GetService("RunService"):Set3dRenderingEnabled(false)
        game:GetService("RunService"):Set3dRenderingEnabled(true)
        StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.Chat, false)
        StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.PlayerList, false)
        task.wait(0.2)
        StarterGui:SetCoreGuiEnabled(Enum.CoreGuiType.Chat, true)
        task.wait(0.2)
        game:GetService("TextChatService").ChatWindowConfiguration.Enabled = false
        local CoreGui = game:GetService("CoreGui")
        print(CoreGui:ToggleRecording())
        local saved = CoreGui:TakeScreenshot()
        print(saved:match("^Screenshot saved to .*%.png$") ~= nil)
    "#;
    load(&mut game, &mut st, "t", src, 0.8);
    assert_eq!(
        game.client_done,
        [
            ClientAction::MasterVolume(0.3),
            ClientAction::LowGraphics(true),
            ClientAction::LowGraphics(false),
            ClientAction::ToggleFullscreen,
            ClientAction::WorldRendering(false),
            ClientAction::WorldRendering(true),
            ClientAction::Hud("chat".into(), false),
            ClientAction::Hud("playerlist".into(), false),
            ClientAction::Hud("chat".into(), true),
            ClientAction::Hud("chat".into(), false),
        ]
    );
    assert_eq!(
        printed(&st, "t"),
        [
            "Minecraft has no video recorder (record with your system's screen recorder)",
            "true"
        ]
    );
    let shots = std::fs::read_dir(game.dir.join("screenshots")).unwrap().count();
    assert_eq!(shots, 1);
}

#[test]
fn voices_are_muted_and_heard_from_another_player() {
    let mut game = world("iym-voice");
    let mut st = studio(&mut game);
    let src = r#"
        local VoiceChatInternal = game:GetService("VoiceChatInternal")
        local alex = game.Players:WaitForChild("Alex")
        repeat task.wait() until alex.Character
        VoiceChatInternal:SubscribePause(alex.UserId, true)
        VoiceChatInternal:SubscribePause(alex.UserId, false)
        VoiceChatInternal:SubscribePauseAll(true)
        -- Infinite Yield's listento: voices are heard from Alex's place.
        game:GetService("SoundService"):SetListener(Enum.ListenerType.ObjectPosition, alex.Character.HumanoidRootPart)
        task.wait(0.2)
        game:GetService("SoundService"):SetListener(Enum.ListenerType.Camera)
    "#;
    load(&mut game, &mut st, "t", src, 0.8);
    let heard: Vec<&ClientAction> = game
        .client_done
        .iter()
        .filter(|a| matches!(a, ClientAction::VoiceListener(_)))
        .collect();
    assert!(
        matches!(heard[..], [ClientAction::VoiceListener(Some(at)), ClientAction::VoiceListener(None)] if (at[0] - 12.5).abs() < 0.01),
        "{heard:?}"
    );
    let muted: Vec<&ClientAction> = game
        .client_done
        .iter()
        .filter(|a| matches!(a, ClientAction::MuteVoice(..)))
        .collect();
    assert_eq!(
        muted,
        [
            &ClientAction::MuteVoice(Some(ALEX), true),
            &ClientAction::MuteVoice(Some(ALEX), false),
            &ClientAction::MuteVoice(None, true),
        ]
    );
}

#[test]
fn idled_fires_and_a_virtual_click_is_activity() {
    let mut game = world("iym-idle");
    let mut st = studio(&mut game);
    // Infinite Yield's antiafk.
    let src = r#"
        game.Players.LocalPlayer.Idled:Connect(function(t)
            print("idled", t)
            local vim = Instance.new("VirtualInputManager")
            vim:SendMouseButtonEvent(0, 0, 0, true, game, 0)
            vim:SendMouseButtonEvent(0, 0, 0, false, game, 0)
            vim:Destroy()
        end)
    "#;
    load(&mut game, &mut st, "t", src, 60.0);
    assert!(printed(&st, "t").is_empty());
    run_for(&mut st, &mut game, 61.0);
    assert_eq!(printed(&st, "t"), ["idled\t120"]);
    assert_eq!(game.client_done, [ClientAction::Activity]);
    // Moving is the player's own activity: no Idled for two more minutes.
    game.player.as_mut().unwrap().x += 1.0;
    run_for(&mut st, &mut game, 100.0);
    assert_eq!(printed(&st, "t").len(), 1);
}

#[test]
fn the_developer_console_is_the_studio_s_and_roblox_only_prompts_say_so() {
    let mut game = world("iym-console");
    let mut st = studio(&mut game);
    let src = r#"
        game:GetService("StarterGui"):SetCore("DevConsoleVisible", true)
        local alex = game.Players:WaitForChild("Alex")
        game.Players.LocalPlayer:RequestFriendship(alex)
        game:GetService("GuiService"):InspectPlayerFromUserId(alex.UserId)
        game:GetService("GuiService"):ClearError()
    "#;
    load(&mut game, &mut st, "t", src, 0.5);
    assert!(st.s.ui.open_request || st.s.ui.open);
    assert_eq!(
        game.chats
            .iter()
            .filter(|c| c.starts_with("[Roblox]"))
            .cloned()
            .collect::<Vec<_>>(),
        [
            "[Roblox] Minecraft has no friends list to send a friend request to Alex in",
            "[Roblox] Minecraft has no inspect menu to show Alex's avatar in",
        ]
    );
}
