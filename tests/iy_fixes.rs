// Infinite Yield's commands that didn't carry over, as the Roblox calls they
// make: chatlogs (TextChatService.MessageReceived from servers' real chat
// formats and signed chat), tweengoto and the other moves of the root part
// (in steps a server takes: hstudio_travel), scare, replicationlag
// (backtrack), and the other players' tools (hstudio_rbx_gear: copytools,
// handlekill).

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, ModState, MoveLimits, PlayerEntry, PlayerPose, Studio};
use horizon_studio::game::hstudio_rbx_client::parse_chat;

const ALEX: u128 = 0x42;
/// A vanilla or Paper server's moved-too-quickly budget, as the bridge has it.
const STEP: f64 = 4.9;

/// A server with Steve (the player) at 10.5, 64, -3.25 and Alex near him.
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
    game
}

/// The server checks the player's moves, as a vanilla or Paper server does.
fn checked(game: &mut FakeGame) {
    game.move_limits = Some(MoveLimits {
        per_tick: STEP,
        collide: true,
        riding: false,
    });
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
    run_for(st, game, 0.6 + seconds);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}\n{:?}", log(st));
    }
}

fn feet(game: &FakeGame) -> [f64; 3] {
    let p = game.player.as_ref().unwrap();
    [p.x, p.y, p.z]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Runs `secs` a game tick at a time in `frames` frames each, returning
/// where the player is at each tick's end (where its move packet puts them).
fn ticks(st: &mut Studio, game: &mut FakeGame, secs: f64, frames: usize) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for _ in 0..(secs / 0.05).round() as usize {
        game.ticks += 1;
        for _ in 0..frames {
            st.frame(game, 0.05 / frames as f64);
        }
        out.push(feet(game));
    }
    out
}

/// The longest move from one tick to the next, from `from`.
fn longest(path: &[[f64; 3]], from: [f64; 3]) -> f64 {
    let mut prev = from;
    path.iter().fold(0.0, |most: f64, &p| {
        let d = dist(p, prev);
        prev = p;
        most.max(d)
    })
}

/// Whether the player's box at `at` is out of the stone `game` has.
fn in_the_open(game: &FakeGame, at: [f64; 3]) -> bool {
    let h = 0.299;
    let cells = |lo: f64, hi: f64| lo.floor() as i32..=hi.floor() as i32;
    for x in cells(at[0] - h, at[0] + h) {
        for y in cells(at[1] + 0.001, at[1] + 1.799) {
            for z in cells(at[2] - h, at[2] + h) {
                if game.blocks.contains_key(&(x, y, z)) {
                    return false;
                }
            }
        }
    }
    true
}

// ---------------------------------------------------------------- chatlogs

#[test]
fn servers_chat_formats_parse_to_their_speaker() {
    let names = ["Steve".to_owned(), "Alex".to_owned()];
    let said = |line: &str| parse_chat(line, &names);
    let alex = |text: &str| Some(("Alex".to_owned(), text.to_owned(), false));
    let from = |who: &str, text: &str| Some((who.to_owned(), text.to_owned(), true));
    // Vanilla, Essentials' default and a rank inside the brackets.
    assert_eq!(said("<Alex> hello"), alex("hello"));
    assert_eq!(said("<[Admin] Alex> hi"), alex("hi"));
    assert_eq!(
        said("<Steve> my own line"),
        Some(("Steve".into(), "my own line".into(), false))
    );
    // Hypixel: ranks, guild and party channels, a guild rank after the name.
    assert_eq!(said("[MVP+] Alex: gg"), alex("gg"));
    assert_eq!(said("Guild > [MVP++] Alex [GM]: anyone up?"), alex("anyone up?"));
    assert_eq!(said("Party > [VIP] Alex: warp"), alex("warp"));
    assert_eq!(said("From [MVP+] Alex: psst"), from("Alex", "psst"));
    assert_eq!(said("To [VIP] Alex: hey"), from("Steve", "hey"));
    // Essentials and LuckPerms prefixes and suffixes, with colour codes.
    assert_eq!(
        said("\u{a7}8[\u{a7}bVIP\u{a7}8] \u{a7}7Alex\u{a7}8: \u{a7}fhi all"),
        alex("hi all")
    );
    assert_eq!(said("[Owner] Alex \u{bb} hello"), alex("hello"));
    assert_eq!(said("Owner | Alex: hello"), alex("hello"));
    assert_eq!(said("[Admin] Alex [Lv. 12]: ready"), alex("ready"));
    assert_eq!(said("Alex \u{2726} > ok"), alex("ok"));
    assert_eq!(said("[Alex -> me] secret"), from("Alex", "secret"));
    assert_eq!(said("[me -> Alex] reply"), from("Steve", "reply"));
    // Vanilla's whispers both ways, and Velocity's server tags.
    assert_eq!(said("Alex whispers to you: psst"), from("Alex", "psst"));
    assert_eq!(said("You whisper to Alex: back"), from("Steve", "back"));
    assert_eq!(said("[survival] Alex: lag?"), alex("lag?"));
    assert_eq!(said("[lobby] [Admin] Alex \u{bb} come"), alex("come"));
    // The text keeps its own separators.
    assert_eq!(said("Alex: look: here > there"), alex("look: here > there"));
    // Not a player's message.
    assert_eq!(said("[Server] Restarting in 5 minutes"), None);
    assert_eq!(said("Alex joined the game"), None);
    assert_eq!(said("[Broadcast] Alex reached level 10: gg"), None);
    assert_eq!(said("Bob: not on this server"), None);
}

#[test]
fn infinite_yields_chat_log_gets_every_players_line() {
    let mut game = world("iyf-chatlog");
    game.players.as_mut().unwrap()[1].display_name = "~Bobby".into();
    let mut st = studio(&mut game);
    // IY's chat log: TextChatService's messages, the speaker by UserId, a
    // RichText label sized to its text in its scrolling frame.
    let src = r#"
        local Players = game:GetService("Players")
        local TextChatService = game:GetService("TextChatService")
        Players:WaitForChild("Alex")
        local gui = Instance.new("ScreenGui", Players.LocalPlayer.PlayerGui)
        local scroll = Instance.new("ScrollingFrame", gui)
        scroll.Size = UDim2.new(0, 328, 0, 190)
        TextChatService.MessageReceived:Connect(function(m)
            if m.TextSource and m.Status ~= Enum.TextChatMessageStatus.InvalidTextChannelPermissions then
                local player = Players:GetPlayerByUserId(m.TextSource.UserId)
                if not player then return end
                local tl = Instance.new("TextLabel")
                tl.Name = player.Name
                tl.Parent = scroll
                tl.RichText = true
                tl.TextWrapped = true
                tl.Text = "[" .. player.Name .. "]: " .. m.Text
                tl.Text = tl.ContentText
                tl.Size = UDim2.new(0, 322, 0, tl.TextBounds.Y)
                print(tl.Text, tl.TextBounds.Y > 0)
            end
        end)
    "#;
    load(&mut game, &mut st, "t", src, 0.4);
    for line in [
        "[MVP+] Alex: hi",
        "Guild > Alex [GM]: guild line",
        "[Owner] ~Bobby \u{bb} nicknamed",
        "<Steve> ;fly",
        "Alex has made the advancement [Stone Age]",
    ] {
        st.chat_line(line);
    }
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(
        printed(&st, "t"),
        [
            "[Alex]: hi\ttrue",
            "[Alex]: guild line\ttrue",
            "[Alex]: nicknamed\ttrue",
            "[Steve]: ;fly\ttrue",
        ]
    );
}

#[test]
fn a_signed_message_is_its_senders_whatever_the_line() {
    let mut game = world("iyf-signed");
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        game:GetService("TextChatService").MessageReceived:Connect(function(m)
            local p = Players:GetPlayerByUserId(m.TextSource.UserId)
            print("message", p and p.Name, m.Text)
        end)
        Players:WaitForChild("Alex").Chatted:Connect(function(text, to)
            print("chatted", text, to == Players.LocalPlayer)
        end)
        Players.LocalPlayer.Chatted:Connect(function(text) print("me", text) end)
    "#;
    load(&mut game, &mut st, "t", src, 0.4);
    // A chat plugin's line names no one the parser knows; the server named
    // the sender. Each line comes once, as the hooks bring it.
    let line = "\u{2726} The Great One \u{2726} >> hello there";
    st.player_chat(ALEX, "hello there", line);
    st.chat_line(line);
    // A whisper's line says it is one; the player's own come back too.
    st.player_chat(ALEX, "psst", "Alex whispers to you: psst");
    st.chat_line("Alex whispers to you: psst");
    st.player_chat(1, ";speed 50", "[Member] Steve: ;speed 50");
    st.chat_line("[Member] Steve: ;speed 50");
    // A sender with no Player here (another server of the proxy's) is the line's.
    st.player_chat(0x99, "far away", "<Alex> far away");
    st.chat_line("<Alex> far away");
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(
        printed(&st, "t"),
        [
            "message\tAlex\thello there",
            "chatted\thello there\tfalse",
            "message\tAlex\tpsst",
            "chatted\tpsst\ttrue",
            "message\tSteve\t;speed 50",
            "me\t;speed 50",
            "message\tAlex\tfar away",
            "chatted\tfar away\tfalse",
        ]
    );
}

// ---------------------------------------------------------------- moves

/// The studio with the root part moved by `src` (a console line).
fn moved(name: &str, src: &str, secs: f64, frames: usize, walls: bool) -> (FakeGame, Studio, Vec<[f64; 3]>) {
    let mut game = world(name);
    if walls {
        // A wall across the way east, six blocks high.
        for y in 64..70 {
            for z in -12..6 {
                game.blocks.insert((60, y, z), "minecraft:stone".into());
            }
        }
    }
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    st.run_console(&mut game, src);
    let path = ticks(&mut st, &mut game, secs, frames);
    (game, st, path)
}

#[test]
fn goto_far_away_goes_there_as_fast_as_the_server_lets() {
    // IY's goto: PivotTo in front of the target, 300 blocks east.
    let src = "local c = game.Players.LocalPlayer.Character \
               c:PivotTo(CFrame.new(Vector3.new(310.5, 66, -3.25), Vector3.new(313.5, 66, -3.25)))";
    let (game, _, path) = moved("iyf-goto", src, 4.0, 3, false);
    let start = [10.5, 64.0, -3.25];
    // Never more a tick than the server's check takes, three frames a tick or not.
    assert!(longest(&path, start) <= STEP + 1e-9, "{}", longest(&path, start));
    let end = feet(&game);
    assert!(dist(end, [310.5, 66.0 - 1.0546875, -3.25]) < 1e-6, "{end:?}");
    // As fast as that: 300 blocks in 62 ticks.
    let first = path.iter().position(|&p| dist(p, end) < 1e-6).unwrap();
    assert!(first <= 63, "{first}");
}

#[test]
fn tweengoto_goes_over_a_wall_and_never_into_it() {
    // IY's tweengoto: a tween of the root part's CFrame, a second.
    let src = r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        game:GetService("TweenService"):Create(root, TweenInfo.new(1, Enum.EasingStyle.Linear),
            {CFrame = CFrame.new(110.5, 65, -3.25)}):Play()
    "#;
    let (game, st, path) = moved("iyf-tween", src, 5.0, 1, true);
    let start = [10.5, 64.0, -3.25];
    assert!(longest(&path, start) <= STEP + 1e-9);
    // Each step's box is out of the wall, and so is the way between them.
    let mut prev = start;
    for &p in &path {
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            let at = [0, 1, 2].map(|i| prev[i] + (p[i] - prev[i]) * t);
            assert!(
                in_the_open(&game, at),
                "{at:?} in the wall, on the way {prev:?} to {p:?}"
            );
        }
        prev = p;
    }
    assert!(path.iter().any(|p| p[1] >= 70.0), "it went over");
    let end = feet(&game);
    assert!(dist(end, [110.5, 65.0 - 1.0546875, -3.25]) < 1e-6, "{end:?}");
    assert!(!log(&st).iter().any(|l| l.contains("blocked")));
}

#[test]
fn a_way_that_is_shut_ends_where_it_is_open_and_says_so() {
    let mut game = world("iyf-shut");
    // The goal is inside a closed box of stone.
    for x in 40..=46 {
        for y in 60..=68 {
            for z in -7..=1 {
                let edge = x == 40 || x == 46 || y == 60 || y == 68 || z == -7 || z == 1;
                if edge {
                    game.blocks.insert((x, y, z), "minecraft:stone".into());
                }
            }
        }
    }
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    st.run_console(
        &mut game,
        "game.Players.LocalPlayer.Character.HumanoidRootPart.CFrame = CFrame.new(43.5, 64, -3.25)",
    );
    let path = ticks(&mut st, &mut game, 6.0, 1);
    let end = feet(&game);
    assert!(path.iter().all(|&p| in_the_open(&game, p)));
    assert!(end[0] < 40.0 - 0.29, "stopped at the wall: {end:?}");
    assert!(log(&st).iter().any(|l| l.contains("is blocked")), "{:?}", log(&st));
    // It is over: the player moves no more.
    let still = ticks(&mut st, &mut game, 1.0, 1);
    assert!(still.iter().all(|&p| p == end));
}

#[test]
fn a_goal_in_the_ground_ends_on_top_of_it() {
    let mut game = world("iyf-ground");
    for x in 20..40 {
        for z in -8..2 {
            game.blocks.insert((x, 63, z), "minecraft:grass_block".into());
        }
    }
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    // The feet half a block into the grass: the server refuses a box in a
    // block it wasn't in, so the player stands on it.
    st.run_console(
        &mut game,
        "game.Players.LocalPlayer.Character:PivotTo(CFrame.new(30.5, 63.5 + 1.0546875, -3.25))",
    );
    let path = ticks(&mut st, &mut game, 2.0, 1);
    assert!(path.iter().all(|&p| in_the_open(&game, p)));
    assert!(dist(feet(&game), [30.5, 64.0, -3.25]) < 1e-6, "{:?}", feet(&game));
    // Done: the player's own moves are theirs again.
    game.player.as_mut().unwrap().x = 31.0;
    run_for(&mut st, &mut game, 0.3);
    assert_eq!(feet(&game), [31.0, 64.0, -3.25]);
}

#[test]
fn a_walk_into_a_wall_stops_at_it_quietly() {
    // tpwalk and cframefly move the character a little each frame: into a
    // wall, the player stops at it (no way round, no warning).
    let mut game = world("iyf-tpwalk");
    for y in 63..70 {
        for z in -8..2 {
            game.blocks.insert((12, y, z), "minecraft:stone".into());
        }
    }
    game.blocks.remove(&(12, 63, -4));
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    let src = r#"
        local c = game.Players.LocalPlayer.Character
        game:GetService("RunService").Heartbeat:Connect(function(dt)
            c:TranslateBy(Vector3.new(1, 0, 0) * 8 * dt)
        end)
    "#;
    st.run_console(&mut game, src);
    let path = ticks(&mut st, &mut game, 3.0, 1);
    assert!(path.iter().all(|&p| in_the_open(&game, p)));
    let end = feet(&game);
    assert!((end[0] - (12.0 - 0.3)).abs() < 0.3, "{end:?}");
    assert!(!log(&st).iter().any(|l| l.contains("blocked")));
}

#[test]
fn a_long_way_through_the_air_dips_so_the_server_never_sees_it_hover() {
    let mut game = world("iyf-air");
    game.pose = Some(PlayerPose {
        pose: "standing".into(),
        sprinting: false,
        on_ground: false,
        flying: false,
        camera: "first".into(),
    });
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    st.run_console(
        &mut game,
        "game.Players.LocalPlayer.Character.HumanoidRootPart.CFrame = CFrame.new(810.5, 100, -3.25)",
    );
    let path = ticks(&mut st, &mut game, 10.0, 1);
    assert!(dist(feet(&game), [810.5, 100.0 - 1.0546875, -3.25]) < 1e-6);
    // The server counts the moves that don't go down; at 80 it kicks.
    let mut level = 0;
    let mut prev = [10.5, 64.0, -3.25];
    for &p in &path {
        level = if p[1] - prev[1] < -0.03125 { 0 } else { level + 1 };
        assert!(level < 80, "hovered {level} ticks at {p:?}");
        prev = p;
    }
    assert!(longest(&path, [10.5, 64.0, -3.25]) <= STEP + 1e-9);
}

#[test]
fn a_fast_body_mover_is_kept_to_the_servers_speed() {
    let mut game = world("iyf-fly");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    let src = r#"
        local bv = Instance.new("BodyVelocity")
        bv.MaxForce = Vector3.new(9e9, 9e9, 9e9)
        bv.Velocity = Vector3.new(500, 0, 0)
        bv.Parent = game.Players.LocalPlayer.Character.HumanoidRootPart
    "#;
    let n = game.moves.len();
    st.run_console(&mut game, src);
    run_for(&mut st, &mut game, 0.2);
    assert!(
        game.moves[n..].iter().any(|m| m == "velocity 4.90 0.00 0.00"),
        "{:?}",
        &game.moves[n..]
    );
}

#[test]
fn scare_goes_in_front_of_them_facing_them_then_back() {
    let mut game = world("iyf-scare");
    // Alex 30 blocks east, facing north (yaw 180).
    game.entities[0].pos = [40.5, 64.0, -3.25];
    game.entities[0].body_yaw = 180.0;
    game.entities[0].head_yaw = 180.0;
    let mut st = studio(&mut game);
    // IY's scare, as it is.
    let src = r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        local target = game.Players:WaitForChild("Alex")
        repeat task.wait() until target.Character
        local targetRoot = target.Character.HumanoidRootPart
        local oldpos = root.CFrame
        root.CFrame = targetRoot.CFrame + targetRoot.CFrame.lookVector * 2
        root.CFrame = CFrame.new(root.Position, targetRoot.Position)
        task.wait(0.5)
        root.CFrame = oldpos
    "#;
    load(&mut game, &mut st, "t", "", 0.1);
    checked(&mut game);
    let n = game.moves.len();
    st.run_console(&mut game, src);
    let path = ticks(&mut st, &mut game, 3.0, 1);
    let start = [10.5, 64.0, -3.25];
    assert!(longest(&path, start) <= STEP + 1e-9);
    // In front of Alex (2 blocks north of him), facing him (south), for half a second.
    let front = [40.5, 64.0, -5.25];
    let there: Vec<usize> = (0..path.len()).filter(|&i| dist(path[i], front) < 1e-6).collect();
    assert!(!there.is_empty(), "{path:?}");
    let stayed = (there.last().unwrap() - there[0] + 1) as f64 * 0.05;
    assert!((0.45..=0.6).contains(&stayed), "{stayed}");
    let looks: Vec<&String> = game.moves[n..].iter().filter(|m| m.starts_with("look")).collect();
    let yaw: f64 = looks
        .last()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|y| y.parse().ok())
        .unwrap();
    assert!(yaw.abs() < 0.5, "{looks:?}");
    // And back where they were.
    assert!(dist(feet(&game), start) < 1e-6);
}

// ---------------------------------------------------------------- backtrack

#[test]
fn replicationlag_holds_the_servers_packets_back() {
    let mut game = world("iyf-lag");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    // IY's replicationlag (backtrack) passes its argument as the string typed.
    let set = |st: &mut Studio, game: &mut FakeGame, v: &str| {
        st.run_console(
            game,
            &format!("settings():GetService('NetworkSettings').IncomingReplicationLag = {v}"),
        );
        run_for(st, game, 0.1);
        game.incoming_lag
    };
    assert_eq!(set(&mut st, &mut game, "'1.5'"), 1.5);
    // Past ten seconds the server's keep-alives would time the player out.
    assert_eq!(set(&mut st, &mut game, "1000"), 10.0);
    assert_eq!(set(&mut st, &mut game, "0"), 0.0);
    // Leaving the server ends it, as everything scripts did there.
    assert_eq!(set(&mut st, &mut game, "3"), 3.0);
    st.session_end(&mut game);
    assert_eq!(game.incoming_lag, 0.0);
}

#[test]
fn leaving_the_server_ends_the_moves_under_way() {
    let mut game = world("iyf-leave");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    checked(&mut game);
    st.run_console(
        &mut game,
        "game.Players.LocalPlayer.Character.HumanoidRootPart.CFrame = CFrame.new(500.5, 65, -3.25)",
    );
    ticks(&mut st, &mut game, 0.5, 1);
    let x = feet(&game)[0];
    assert!(x > 20.0 && x < 100.0, "{x}");
    st.session_end(&mut game);
    ticks(&mut st, &mut game, 1.0, 1);
    assert_eq!(feet(&game)[0], x);
}

// ---------------------------------------------------------------- other players' tools

fn gear(v: &[(&str, &str, u32)]) -> Vec<(String, String, u32)> {
    v.iter().map(|(s, i, n)| (s.to_string(), i.to_string(), *n)).collect()
}

#[test]
fn another_players_held_item_is_a_tool_in_their_character() {
    let mut game = world("iyf-gear");
    game.entities[0].equipment = gear(&[
        ("MainHand", "minecraft:diamond_sword", 1),
        ("OffHand", "minecraft:shield", 1),
        ("Head", "minecraft:iron_helmet", 1),
        ("Feet", "minecraft:golden_boots", 1),
    ]);
    let mut st = studio(&mut game);
    let src = r#"
        local Players = game:GetService("Players")
        local alex = Players:WaitForChild("Alex")
        repeat task.wait() until alex.Character
        local function show()
            local held, worn, bag = {}, {}, {}
            for _, c in ipairs(alex.Character:GetChildren()) do
                if c:IsA("Tool") then
                    table.insert(held, c.Name .. "=" .. c:GetAttribute("ItemId") .. "x" .. c:GetAttribute("Count"))
                elseif c:IsA("Accessory") then
                    table.insert(worn, c:GetAttribute("Slot") .. "=" .. c.Name)
                end
            end
            for _, t in ipairs(alex.Backpack:GetChildren()) do
                table.insert(bag, t.Name .. ":" .. type(t:GetAttribute("LastSeen")))
            end
            table.sort(worn)
            print(table.concat(held, ","), table.concat(worn, ","), table.concat(bag, ","))
        end
        show()
        local tool = alex.Character:FindFirstChildOfClass("Tool")
        print(tool.Handle:IsA("BasePart"), (tool.Handle.Position - alex.Character["Right Arm"].Position).Magnitude < 1)
        tool.Unequipped:Connect(function() print("unequipped", tool.Name) end)
        alex.Character.ChildAdded:Connect(function(c)
            if c:IsA("Tool") then print("added", c.Name) end
        end)
        shared.show = show
    "#;
    load(&mut game, &mut st, "t", src, 0.5);
    game.entities[0].equipment = gear(&[("MainHand", "minecraft:bow", 1), ("Feet", "minecraft:golden_boots", 1)]);
    run_for(&mut st, &mut game, 0.2);
    st.run_console(&mut game, "shared.show()");
    // Back to the sword: the one seen comes out of the Backpack.
    game.entities[0].equipment = gear(&[("MainHand", "minecraft:diamond_sword", 1)]);
    run_for(&mut st, &mut game, 0.2);
    st.run_console(&mut game, "shared.show()");
    let out = printed(&st, "t");
    assert_eq!(
        out,
        [
            "Diamond Sword=minecraft:diamond_swordx1\tFeet=Golden Boots,Head=Iron Helmet,OffHand=Shield\t",
            "true\ttrue",
            "unequipped\tDiamond Sword",
            "added\tBow",
            "added\tDiamond Sword",
        ]
    );
    // (What the console's calls of it print.)
    let shown: Vec<String> = log(&st)
        .iter()
        .filter(|l| l.contains("=minecraft:") && !l.starts_with('['))
        .map(|l| l.replace("    ", "\t"))
        .collect();
    assert_eq!(
        shown,
        [
            "Bow=minecraft:bowx1\tFeet=Golden Boots\tDiamond Sword:number",
            "Diamond Sword=minecraft:diamond_swordx1\t\tBow:number",
        ]
    );
    // Out of sight, what they held is in their Backpack.
    game.entities.clear();
    game.players.as_mut().unwrap()[1].entity = None;
    run_for(&mut st, &mut game, 1.2);
    st.run_console(
        &mut game,
        "local n = {} for _, t in ipairs(game.Players.Alex.Backpack:GetChildren()) do table.insert(n, t.Name) end table.sort(n) print(table.concat(n, ','))",
    );
    assert_eq!(log(&st).last().unwrap(), "Bow,Diamond Sword");
}

#[test]
fn copytools_copies_what_they_were_seen_with() {
    let mut game = world("iyf-copytools");
    game.entities[0].equipment = gear(&[("MainHand", "minecraft:fishing_rod", 1)]);
    let mut st = studio(&mut game);
    load(
        &mut game,
        &mut st,
        "t",
        "local a = game.Players:WaitForChild('Alex') repeat task.wait() until a.Character",
        0.4,
    );
    game.entities[0].equipment = gear(&[("MainHand", "minecraft:torch", 16)]);
    run_for(&mut st, &mut game, 0.2);
    // IY's copytools, and its tools(plr) check.
    let src = r#"
        local Players = game:GetService("Players")
        local speaker = Players.LocalPlayer
        local function tools(plr)
            if plr:FindFirstChildOfClass("Backpack"):FindFirstChildOfClass("Tool") or plr.Character:FindFirstChildOfClass("Tool") then
                return true
            end
        end
        print("tools", tools(Players.Alex))
        for _, v in pairs(Players.Alex:FindFirstChildOfClass("Backpack"):GetChildren()) do
            if v:IsA("Tool") or v:IsA("HopperBin") then
                v:Clone().Parent = speaker:FindFirstChildOfClass("Backpack")
            end
        end
        local copy = speaker.Backpack:FindFirstChild("Fishing Rod")
        print("copied", copy ~= nil, copy and copy:GetAttribute("ItemId"))
        print("held", Players.Alex.Character:FindFirstChildOfClass("Tool"):GetAttribute("Count"))
    "#;
    st.run_console(&mut game, src);
    run_for(&mut st, &mut game, 0.1);
    let tail: Vec<String> = log(&st).iter().rev().take(3).rev().cloned().collect();
    assert_eq!(
        tail,
        ["tools    true", "copied    true    minecraft:fishing_rod", "held    16"]
    );
}

#[test]
fn handlekill_hits_whom_the_held_tools_handle_touches() {
    let mut game = world("iyf-handlekill");
    game.inventory = Some(
        (0..36)
            .map(|i| (i == 0).then(|| ("minecraft:diamond_sword".to_owned(), 1)))
            .collect(),
    );
    game.player.as_mut().unwrap().held = Some(("minecraft:diamond_sword".into(), 1));
    let mut st = studio(&mut game);
    load(
        &mut game,
        &mut st,
        "t",
        "local a = game.Players:WaitForChild('Alex') repeat task.wait() until a.Character",
        0.4,
    );
    // IY's handlekill: the held Tool's Handle touched to each target's root, every frame.
    let src = r#"
        local Players = game:GetService("Players")
        local speaker = Players.LocalPlayer
        local tool = speaker.Character:FindFirstChildWhichIsA("Tool")
        local handle = tool and tool:FindFirstChild("Handle")
        print("handle", handle ~= nil)
        task.spawn(function()
            while task.wait() and tool.Parent == speaker.Character do
                local root = Players.Alex.Character and Players.Alex.Character.HumanoidRootPart
                if root then
                    firetouchinterest(handle, root, 1)
                    firetouchinterest(handle, root, 0)
                end
            end
        end)
    "#;
    let n = game.moves.len();
    st.run_console(&mut game, src);
    run_for(&mut st, &mut game, 2.0);
    assert!(log(&st).iter().any(|l| l == "handle    true"));
    let attacks: Vec<&String> = game.moves[n..].iter().filter(|m| m.starts_with("attack")).collect();
    // Once each attack cooldown (0.625 s), at Alex's entity.
    assert!((3..=4).contains(&attacks.len()), "{attacks:?}");
    assert!(attacks.iter().all(|a| *a == "attack 12"));
    // Out of reach, none.
    game.entities[0].pos = [30.5, 64.0, -3.25];
    run_for(&mut st, &mut game, 0.2);
    let n = game.moves.len();
    run_for(&mut st, &mut game, 1.5);
    assert!(!game.moves[n..].iter().any(|m| m.starts_with("attack")));
}
