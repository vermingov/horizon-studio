// The game's packets as Roblox remotes (hstudio_rbx_remote): the remotes in
// ReplicatedStorage.Minecraft, the game's own packets reaching a
// `__namecall` hook as FireServer (seen, stopped or changed), a script's
// FireServer sending one, the server's packets firing OnClientEvent, and the
// fields read from a packet's Debug form.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_rbx_remote::{Field, Packet, Verdict, debug_fields, remote_name};

fn setup(name: &str) -> (Studio, FakeGame) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    (st, game)
}

/// Runs console code; the lines it printed starting with `tag`, tabs as tabs.
fn tagged(st: &Studio, tag: &str) -> Vec<String> {
    log(st)
        .iter()
        .filter(|l| l.starts_with(tag))
        .map(|l| l.replace("    ", "\t"))
        .collect()
}

fn run(st: &mut Studio, game: &mut FakeGame, src: &str) {
    st.run_console(game, src);
    run_for(st, game, 0.1);
}

fn move_pos(x: f64, y: f64, z: f64) -> Packet {
    Packet {
        id: 6,
        name: "move_player_pos".into(),
        fields: vec![
            ("x".into(), Field::Num(x)),
            ("y".into(), Field::Num(y)),
            ("z".into(), Field::Num(z)),
            ("flags".into(), Field::Int(1)),
        ],
        mapped: true,
        raw: None,
    }
}

#[test]
fn remote_names_are_vanillas_in_pascal_case() {
    assert_eq!(remote_name("move_player_status_only"), "MovePlayerStatusOnly");
    assert_eq!(remote_name("debug/block_value"), "DebugBlockValue");
    assert_eq!(remote_name("chat"), "Chat");
}

#[test]
fn each_packet_has_a_remote_in_replicated_storage() {
    let (mut st, mut game) = setup("remotes-made");
    run(
        &mut st,
        &mut game,
        r#"
        local mc = game:GetService("ReplicatedStorage"):WaitForChild("Minecraft")
        print("R", mc.ClassName, #mc:GetChildren())
        print("R", mc.MovePlayerPos.ClassName, mc.CommandSuggestion.ClassName, mc.CustomPayload.ClassName)
        print("R", mc.SystemChat.ClassName, typeof(mc.SystemChat.OnClientEvent))
        "#,
    );
    // Nine serverbound and five clientbound, two names shared.
    assert_eq!(
        tagged(&st, "R"),
        [
            "R\tFolder\t12",
            "R\tRemoteEvent\tRemoteFunction\tRemoteEvent",
            "R\tRemoteEvent\tRBXScriptSignal"
        ]
    );
}

#[test]
fn without_a_hook_the_game_s_packets_skip_lua() {
    let (mut st, mut game) = setup("remotes-unspied");
    assert!(!st.packets_spied());
    assert_eq!(st.game_packet(&mut game, &move_pos(1.0, 2.0, 3.0)), Verdict::Send);
    // A hook restored (hookmetamethod with the original) costs nothing again.
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(...) return old(...) end)
        hookmetamethod(game, "__namecall", old)
        "#,
    );
    assert!(!st.packets_spied());
}

#[test]
fn a_namecall_hook_sees_the_game_s_own_packets_as_fire_server() {
    let (mut st, mut game) = setup("remotes-spy");
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", newcclosure(function(self, ...)
          local method = getnamecallmethod()
          if method == "FireServer" or method == "InvokeServer" then
            local a = ...
            -- A method call in the hook names its own method, as in Roblox:
            -- the original is told again which one this call is.
            print("SPY", method, self:GetFullName(), a.x, a.y, a.flags, checkcaller(), select('#', ...))
            setnamecallmethod(method)
          end
          return old(self, ...)
        end))
        "#,
    );
    assert!(st.packets_spied());
    assert_eq!(st.game_packet(&mut game, &move_pos(10.5, 64.0, -3.25)), Verdict::Send);
    assert_eq!(
        tagged(&st, "SPY"),
        ["SPY\tFireServer\tReplicatedStorage.Minecraft.MovePlayerPos\t10.5\t64\t1\tfalse\t1"]
    );
    // A packet whose fields are read from the game's form of it comes with its payload.
    let chat = Packet {
        id: 1,
        name: "chat".into(),
        fields: vec![("message".into(), Field::text("hi"))],
        mapped: false,
        raw: Some(b"\x02hi".to_vec()),
    };
    run(
        &mut st,
        &mut game,
        r#"
        local old2
        old2 = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" and self.Name == "Chat" then
            local fields, raw = ...
            print("CHAT", fields.message, #raw, raw:byte(1))
          end
          return old2(self, ...)
        end)
        "#,
    );
    assert_eq!(st.game_packet(&mut game, &chat), Verdict::Send);
    assert_eq!(tagged(&st, "CHAT"), ["CHAT\thi\t3\t2"]);
    // A script's own call is the executor's.
    run(
        &mut st,
        &mut game,
        "game.ReplicatedStorage.Minecraft.MovePlayerPos:FireServer({x = 1, y = 2, z = 3, flags = 0})",
    );
    assert_eq!(
        tagged(&st, "SPY").last().unwrap(),
        "SPY\tFireServer\tReplicatedStorage.Minecraft.MovePlayerPos\t1\t2\t0\ttrue\t1"
    );
}

#[test]
fn a_hook_that_returns_without_the_original_blocks_the_packet() {
    let (mut st, mut game) = setup("remotes-block");
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" and self.Name == "MovePlayerPos" then
            local a = ...
            if a.y < 0 then return end
          end
          return old(self, ...)
        end)
        "#,
    );
    assert_eq!(st.game_packet(&mut game, &move_pos(0.0, -5.0, 0.0)), Verdict::Block);
    assert_eq!(st.game_packet(&mut game, &move_pos(0.0, 70.0, 0.0)), Verdict::Send);
    // Nothing reached the game itself: the bridge sends (or not) the game's packet.
    assert!(game.packets.is_empty());
}

#[test]
fn a_hook_that_changes_the_arguments_sends_the_changed_packet() {
    let (mut st, mut game) = setup("remotes-change");
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" and self.Name == "MovePlayerPos" then
            local a = table.clone((...))
            a.y = a.y + 1
            return old(self, a)
          end
          return old(self, ...)
        end)
        "#,
    );
    match st.game_packet(&mut game, &move_pos(1.0, 64.0, 2.0)) {
        Verdict::Replace(p) => {
            assert_eq!(p.name, "move_player_pos");
            assert!(p.mapped);
            assert!(p.fields.contains(&("y".into(), Field::Num(65.0))), "{:?}", p.fields);
            assert!(p.fields.contains(&("x".into(), Field::Num(1.0))), "{:?}", p.fields);
        }
        other => panic!("{other:?}"),
    }
    // A chat message changed goes out through the chat again (signed by the game).
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" and self.Name == "Chat" then
            local fields, raw = ...
            return old(self, { message = fields.message .. "!" }, raw)
          end
          return old(self, ...)
        end)
        "#,
    );
    let chat = Packet {
        id: 1,
        name: "chat".into(),
        fields: vec![("message".into(), Field::text("hi"))],
        mapped: false,
        raw: Some(b"\x02hi".to_vec()),
    };
    assert_eq!(st.game_packet(&mut game, &chat), Verdict::Block);
    assert_eq!(game.sent.last().map(String::as_str), Some("hi!"));
}

#[test]
fn a_failing_hook_lets_the_packet_through() {
    let (mut st, mut game) = setup("remotes-fail");
    run(
        &mut st,
        &mut game,
        r#"hookmetamethod(game, "__namecall", function() error("broken spy") end)"#,
    );
    assert_eq!(st.game_packet(&mut game, &move_pos(0.0, 64.0, 0.0)), Verdict::Send);
    assert!(log(&st).iter().any(|l| l.contains("broken spy")), "{:?}", log(&st));
}

#[test]
fn hookfunction_of_fire_server_sees_the_game_s_packets_too() {
    // SimpleSpy's other way: hookfunction(remote.FireServer, ...). With a
    // namecall hook as well, each call is seen once.
    let (mut st, mut game) = setup("remotes-hookfunction");
    run(
        &mut st,
        &mut game,
        r#"
        local probe = Instance.new("RemoteEvent")
        local seen = 0
        local original
        original = hookfunction(probe.FireServer, function(self, ...)
          print("HF", self.Name, (...).x)
          return original(self, ...)
        end)
        "#,
    );
    assert!(st.packets_spied());
    assert_eq!(st.game_packet(&mut game, &move_pos(4.0, 64.0, 0.0)), Verdict::Send);
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" then print("NC", self.Name) end
          return old(self, ...)
        end)
        game.ReplicatedStorage.Minecraft.MovePlayerPos:FireServer({x = 5, y = 64, z = 0, flags = 0})
        "#,
    );
    assert_eq!(tagged(&st, "HF"), ["HF\tMovePlayerPos\t4"]);
    assert_eq!(tagged(&st, "NC"), ["NC\tMovePlayerPos"]);
    assert_eq!(game.packets.len(), 1);
}

#[test]
fn fire_server_sends_the_packet_with_its_fields() {
    let (mut st, mut game) = setup("remotes-fire");
    run(
        &mut st,
        &mut game,
        r#"
        local mc = game.ReplicatedStorage.Minecraft
        mc.MovePlayerPos:FireServer({ x = 10.5, y = 70, z = -3, flags = 1 })
        mc.UseItemOn:FireServer({ hand = 0, pos = Vector3.new(1, 64, 2), face = 1, cursor = Vector3.new(0.5, 1, 0.5),
          inside = false, world_border = false, sequence = 3 })
        mc.CustomPayload:FireServer({}, buffer.fromstring("\0\1hello"))
        mc.CustomPayload:FireServer("raw only")
        mc.Chat:FireServer({ message = "hello there" })
        mc.ChatCommand:FireServer({ command = "say hi" })
        "#,
    );
    assert_eq!(game.packets.len(), 4, "{:?}", log(&st));
    let p = &game.packets[0];
    assert_eq!(
        (p.id, p.name.as_str(), p.mapped, p.raw.is_none()),
        (6, "move_player_pos", true, true)
    );
    assert_eq!(
        p.fields,
        [
            ("flags".into(), Field::Num(1.0)),
            ("x".into(), Field::Num(10.5)),
            ("y".into(), Field::Num(70.0)),
            ("z".into(), Field::Num(-3.0)),
        ]
    );
    assert!(
        game.packets[1]
            .fields
            .contains(&("pos".into(), Field::Vec3([1.0, 64.0, 2.0])))
    );
    assert_eq!(game.packets[2].raw.as_deref(), Some(&b"\0\x01hello"[..]));
    assert!(!game.packets[2].mapped);
    assert_eq!(game.packets[3].raw.as_deref(), Some(&b"raw only"[..]));
    // Chat goes through the game's chat, which signs it.
    assert_eq!(game.sent, ["hello there", "/say hi"]);
}

#[test]
fn bad_fire_server_calls_are_clear_errors_and_send_nothing() {
    let (mut st, mut game) = setup("remotes-bad");
    run(
        &mut st,
        &mut game,
        r#"
        local mc = game.ReplicatedStorage.Minecraft
        local function try(f) local ok, e = pcall(f) print("E", ok, e) end
        try(function() mc.MovePlayerPos:FireServer(42) end)
        try(function() mc.MovePlayerPos:FireServer({ x = workspace }) end)
        try(function() mc.SystemChat:FireServer({}) end)
        try(function() mc.Chat:FireServer({ message = "/op me" }) end)
        try(function() mc.Chat:FireServer({}) end)
        local t = {} t.self = t
        try(function() mc.MovePlayerPos:FireServer(t) end)
        "#,
    );
    let errs = tagged(&st, "E");
    assert_eq!(errs.len(), 6, "{errs:?}");
    assert!(errs.iter().all(|e| e.starts_with("E\tfalse\t")), "{errs:?}");
    assert!(errs[0].contains("not a number"), "{}", errs[0]);
    assert!(errs[1].contains("Instance can't be sent"), "{}", errs[1]);
    assert!(errs[2].contains("the client doesn't send it"), "{}", errs[2]);
    assert!(errs[3].contains("fire ChatCommand"), "{}", errs[3]);
    assert!(errs[4].contains("message is missing"), "{}", errs[4]);
    assert!(errs[5].contains("too deep"), "{}", errs[5]);
    assert!(game.packets.is_empty() && game.sent.is_empty());
    // The game's own refusal reaches the script.
    game.packet_error = Some("flags: 300 is out of range (0 to 255)".into());
    run(
        &mut st,
        &mut game,
        r#"print("G", pcall(function() game.ReplicatedStorage.Minecraft.MovePlayerPos:FireServer({ x = 0, y = 0, z = 0, flags = 300 }) end))"#,
    );
    let g = tagged(&st, "G");
    assert!(g[0].contains("MovePlayerPos: flags: 300 is out of range"), "{g:?}");
}

#[test]
fn a_loop_can_t_flood_the_server() {
    let (mut st, mut game) = setup("remotes-flood");
    run(
        &mut st,
        &mut game,
        r#"
        local r = game.ReplicatedStorage.Minecraft.CustomPayload
        print("F", pcall(function() for i = 1, 1000 do r:FireServer("x") end end))
        "#,
    );
    assert_eq!(game.packets.len(), 256);
    assert!(tagged(&st, "F")[0].contains("too many packets this frame"));
}

#[test]
fn the_server_s_packets_fire_on_client_event() {
    let (mut st, mut game) = setup("remotes-incoming");
    let packet = Packet {
        id: 3,
        name: "system_chat".into(),
        fields: vec![
            ("content".into(), Field::text("Welcome")),
            ("overlay".into(), Field::Bool(false)),
        ],
        mapped: false,
        raw: Some(b"\x07Welcome\0".to_vec()),
    };
    assert!(!st.packet_heard("system_chat"));
    run(
        &mut st,
        &mut game,
        r#"
        game.ReplicatedStorage.Minecraft.SystemChat.OnClientEvent:Connect(function(fields, raw)
          print("IN", fields.content, fields.overlay, #raw)
        end)
        "#,
    );
    assert!(st.packet_heard("system_chat"));
    assert!(!st.packet_heard("set_time"));
    st.server_packet(&packet);
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(tagged(&st, "IN"), ["IN\tWelcome\tfalse\t9"]);
}

#[test]
fn invoke_server_waits_for_the_server_s_answer() {
    let (mut st, mut game) = setup("remotes-invoke");
    run(
        &mut st,
        &mut game,
        r#"
        task.spawn(function()
          local r = game.ReplicatedStorage.Minecraft.CommandSuggestion:InvokeServer({ id = 7, command = "/gamem" })
          print("ANSWER", r.id, r.suggestions[1].text)
        end)
        "#,
    );
    assert_eq!(game.packets.len(), 1);
    assert_eq!(game.packets[0].name, "command_suggestion");
    assert!(st.packet_heard("command_suggestions"));
    let answer = |id| Packet {
        id: 0,
        name: "command_suggestions".into(),
        fields: vec![
            ("id".into(), Field::Int(id)),
            ("start".into(), Field::Int(1)),
            ("length".into(), Field::Int(5)),
            (
                "suggestions".into(),
                Field::List(vec![Field::Map(vec![("text".into(), Field::text("gamemode"))])]),
            ),
        ],
        mapped: false,
        raw: Some(Vec::new()),
    };
    // Another request's answer doesn't wake it.
    st.server_packet(&answer(8));
    run_for(&mut st, &mut game, 0.1);
    assert!(tagged(&st, "ANSWER").is_empty());
    st.server_packet(&answer(7));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(tagged(&st, "ANSWER"), ["ANSWER\t7\tgamemode"]);
}

#[test]
fn debug_forms_read_as_fields() {
    let f = debug_fields("MovePlayerPos(MovePlayerPos { x: 1.5, y: 64.0, z: -3.0, flags: 1 })").unwrap();
    assert_eq!(
        f,
        [
            ("x".into(), Field::Num(1.5)),
            ("y".into(), Field::Num(64.0)),
            ("z".into(), Field::Num(-3.0)),
            ("flags".into(), Field::Int(1)),
        ]
    );
    // Options, unit variants, nested structs, lists, escapes and big numbers.
    let f = debug_fields(
        r#"Chat(Chat { message: "a \"q\"\n\u{e9}", timestamp: 1730000000000, salt: -9007199254740993, signature: None, last_seen: LastSeenUpdate { offset: 0, acknowledged: [0, 0, 0] }, hand: Main, pos: Some([1, 2, 3]) })"#,
    )
    .unwrap();
    assert_eq!(
        f[0],
        ("message".into(), Field::Str("a \"q\"\n\u{e9}".as_bytes().to_vec()))
    );
    assert_eq!(f[1], ("timestamp".into(), Field::Int(1_730_000_000_000)));
    assert_eq!(f[2], ("salt".into(), Field::Int(-9_007_199_254_740_993)));
    assert_eq!(
        f[3],
        (
            "last_seen".into(),
            Field::Map(vec![
                ("type".into(), Field::text("LastSeenUpdate")),
                ("offset".into(), Field::Int(0)),
                (
                    "acknowledged".into(),
                    Field::List(vec![Field::Int(0), Field::Int(0), Field::Int(0)])
                ),
            ])
        )
    );
    assert_eq!(f[4], ("hand".into(), Field::text("Main")));
    // Three numbers under a position's name are a Vector3.
    assert_eq!(f[5], ("pos".into(), Field::Vec3([1.0, 2.0, 3.0])));
    assert_eq!(f.len(), 6);
    // A tuple variant names its type; an empty packet has no fields.
    let f = debug_fields("Waypoint(W { id: Uuid(12), target: Azimuth(-1.25) })").unwrap();
    assert_eq!(
        f[1],
        (
            "target".into(),
            Field::Map(vec![
                ("type".into(), Field::text("Azimuth")),
                ("1".into(), Field::Num(-1.25))
            ])
        )
    );
    assert_eq!(debug_fields("ClientTickEnd(Empty)").unwrap(), []);
    // Packets Horizon doesn't model, and text that isn't Debug's, have none.
    assert_eq!(debug_fields("Unknown(12, [1, 2, 3])"), None);
    assert_eq!(debug_fields("Foo { x: }"), None);
    assert_eq!(debug_fields("Foo { x: 1 } trailing"), None);
}

#[test]
fn fields_reach_lua_as_values() {
    // A whole number beyond a double's exact range is its digits.
    let (mut st, mut game) = setup("remotes-values");
    run(
        &mut st,
        &mut game,
        r#"
        game.ReplicatedStorage.Minecraft.KeepAlive.OnClientEvent:Connect(function(f)
          print("KA", type(f.id), f.id, typeof(f.pos), f.pos, f.list[2], f.list[3] == nil, f.opt)
        end)
        "#,
    );
    st.server_packet(&Packet {
        id: 2,
        name: "keep_alive".into(),
        fields: vec![
            ("id".into(), Field::Int(-9_007_199_254_740_993)),
            ("pos".into(), Field::Vec3([1.0, 2.5, -3.0])),
            ("list".into(), Field::List(vec![Field::Int(1), Field::Bool(true)])),
            ("opt".into(), Field::Nil),
        ],
        mapped: false,
        raw: None,
    });
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        tagged(&st, "KA"),
        ["KA\tstring\t-9007199254740993\tVector3\t1, 2.5, -3\ttrue\ttrue\tnil"]
    );
}

#[test]
fn the_packets_only_the_game_sends_can_t_be_stopped_or_fired() {
    let (mut st, mut game) = setup("remotes-game-only");
    run(
        &mut st,
        &mut game,
        r#"
        -- Stops every packet of the game's (checkcaller is false for them).
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if not checkcaller() then return end
          return old(self, ...)
        end)
        "#,
    );
    let keep_alive = Packet {
        id: 5,
        name: "keep_alive".into(),
        fields: vec![("id".into(), Field::Int(123_456_789_012))],
        mapped: true,
        raw: None,
    };
    // Without it the server disconnects the player: it goes as it was.
    assert_eq!(st.game_packet(&mut game, &keep_alive), Verdict::Send);
    assert!(
        log(&st).iter().any(|l| l.contains("KeepAlive can't be stopped")),
        "{:?}",
        log(&st)
    );
    assert_eq!(st.game_packet(&mut game, &move_pos(0.0, 64.0, 0.0)), Verdict::Block);
    run(
        &mut st,
        &mut game,
        r#"print("FIRED", pcall(function() game.ReplicatedStorage.Minecraft.KeepAlive:FireServer({ id = 1 }) end))"#,
    );
    assert!(
        tagged(&st, "FIRED")[0].contains("only the game sends it"),
        "{:?}",
        tagged(&st, "FIRED")
    );
    assert!(game.packets.is_empty());
}

#[test]
fn a_broken_hook_is_reported_once() {
    let (mut st, mut game) = setup("remotes-once");
    run(
        &mut st,
        &mut game,
        r#"hookmetamethod(game, "__namecall", function() error("nope") end)"#,
    );
    for _ in 0..5 {
        assert_eq!(st.game_packet(&mut game, &move_pos(0.0, 64.0, 0.0)), Verdict::Send);
    }
    assert_eq!(log(&st).iter().filter(|l| l.contains("hook failed")).count(), 1);
}

// ------------------------------------------------- what remote spies use

#[test]
fn spies_find_namecall_in_the_metatable_and_hook_it_the_old_way() {
    // SimpleSpy checks getrawmetatable(game).__namecall before it starts;
    // older spies replace it in the metatable instead of hookmetamethod.
    let (mut st, mut game) = setup("remotes-oldway");
    run(
        &mut st,
        &mut game,
        r#"
        local mt = getrawmetatable(game)
        print("M", type(mt.__namecall), iscclosure(mt.__namecall))
        local old = mt.__namecall
        setreadonly(mt, false)
        mt.__namecall = newcclosure(function(self, ...)
          if getnamecallmethod() == "FireServer" then print("OLD", self.Name) end
          return old(self, ...)
        end)
        setreadonly(mt, true)
        print("M", workspace:FindFirstChild("Terrain") ~= nil)
        "#,
    );
    assert_eq!(tagged(&st, "M"), ["M\tfunction\ttrue", "M\ttrue"]);
    assert!(st.packets_spied());
    assert_eq!(st.game_packet(&mut game, &move_pos(0.0, 64.0, 0.0)), Verdict::Send);
    assert_eq!(tagged(&st, "OLD"), ["OLD\tMovePlayerPos"]);
}

#[test]
fn hooks_and_debug_info_as_remote_spies_use_them() {
    let (mut st, mut game) = setup("remotes-spy-tools");
    game.write_mod(
        "firer.lua",
        r#"
        local r = Instance.new("RemoteEvent")
        r.Name = "Probe"
        r.Parent = game.ReplicatedStorage
        getgenv().fire = function() r:FireServer(1) end
        "#,
    );
    run_for(&mut st, &mut game, 1.5);
    run(
        &mut st,
        &mut game,
        r#"
        -- A newcclosure is a C level of its own, as in Luau: level 3 from
        -- inside it is whoever called it.
        local function inner() local i = debug.getinfo(2) print("L", i.what, debug.getinfo(3).name) end
        local w = newcclosure(inner)
        local function outer() w() end
        outer()
        print("L", debug.info(1, "s") ~= nil)

        local old
        old = hookmetamethod(game, "__namecall", newcclosure(function(self, ...)
          if getnamecallmethod() == "FireServer" and self.Name == "Probe" then
            local s = getcallingscript()
            print("CS", s and s:GetFullName(), debug.getinfo(3).short_src)
            -- Another method call names its own method: tell the original again.
            local _ = self:GetFullName()
            setnamecallmethod("FireServer")
          end
          return old(self, ...)
        end))
        getgenv().fire()
        -- Hooking a function with itself leaves it as it is (SimpleSpy's
        -- shutdown restores FireServer that way).
        local probe = Instance.new("RemoteEvent")
        print("SELFHOOK", pcall(hookfunction, probe.FireServer, probe.FireServer))
        "#,
    );
    assert_eq!(tagged(&st, "L"), ["L\tC\touter", "L\ttrue"]);
    assert_eq!(
        tagged(&st, "CS"),
        ["CS\tPlayers.Player1.PlayerScripts.firer\tfirer.lua"]
    );
    assert!(
        tagged(&st, "SELFHOOK")[0].starts_with("SELFHOOK\ttrue\t"),
        "{:?}",
        tagged(&st, "SELFHOOK")
    );
    // The game's own packets have no calling script.
    run(
        &mut st,
        &mut game,
        r#"
        hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" then print("GS", getcallingscript()) end
          return ...
        end)
        "#,
    );
    st.game_packet(&mut game, &move_pos(0.0, 64.0, 0.0));
    assert_eq!(tagged(&st, "GS"), ["GS\tnil"]);
}

#[test]
fn a_screen_gui_not_shown_yet_is_laid_out_on_the_screen() {
    // SimpleSpy sizes its window from AbsoluteSize before it parents it.
    let (mut st, mut game) = setup("remotes-unparented");
    run(
        &mut st,
        &mut game,
        r#"
        local g = Instance.new("ScreenGui")
        local f = Instance.new("Frame")
        f.Position = UDim2.new(0, 500, 0, 200)
        f.Size = UDim2.new(0, 450, 0, 268)
        f.Parent = g
        print("A", f.AbsolutePosition, f.AbsoluteSize)
        "#,
    );
    assert_eq!(tagged(&st, "A"), ["A\t500, 200\t450, 268"]);
}
