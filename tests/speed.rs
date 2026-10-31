// Timings of hot loops: `cargo test --release --test speed -- --ignored --nocapture`.
// Each case runs three times; the best time counts.

mod common;

use common::{FakeGame, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, PlayerEntry};
use horizon_studio::game::hstudio_lua::{Lua, NoHost};
use horizon_studio::game::hstudio_lualib::new_env;

const CASES: &[(&str, &str)] = &[
    (
        "fib(27)",
        "local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end return fib(27)",
    ),
    (
        "numeric for 1e7",
        "local s = 0 for i = 1, 10000000 do s = s + i end return s",
    ),
    (
        "loop 1e6",
        "local s = 0 for i = 1, 1000000 do s = s + i % 7 end return s",
    ),
    (
        "while 1e6",
        "local i, s = 0, 0 while i < 1000000 do i = i + 1 s = s + i end return s",
    ),
    (
        "table array 1e6",
        "local t = {} for i = 1, 1000000 do t[i] = i end local s = 0 for i = 1, #t do s = s + t[i] end return s",
    ),
    (
        "table fields 1e6",
        "local p = {x = 0, y = 0} for i = 1, 1000000 do p.x = p.x + 1 p.y = p.x end return p.y",
    ),
    (
        "globals 1e6",
        "count = 0 for i = 1, 1000000 do count = count + 1 end return count",
    ),
    (
        "ipairs 1e5",
        "local t = {} for i = 1, 100000 do t[i] = i * 2 end local s = 0 for _, v in ipairs(t) do s = s + v end return s",
    ),
    (
        "pairs 1e5",
        "local t = {} for i = 1, 100000 do t['k' .. i] = i end local s = 0 for _, v in pairs(t) do s = s + v end return s",
    ),
    (
        "calls 1e6",
        "local function add(a, b) return a + b end local s = 0 for i = 1, 1000000 do s = add(s, i) end return s",
    ),
    (
        "methods 1e6",
        "local C = {} C.__index = C function C:inc() self.n = self.n + 1 end local o = setmetatable({n = 0}, C) for i = 1, 1000000 do o:inc() end return o.n",
    ),
    (
        "closures 1e6",
        "local function counter() local n = 0 return function() n = n + 1 return n end end local c = counter() for i = 1, 1000000 do c() end return c()",
    ),
    (
        "builtin calls 1e6",
        "local s = 0 local floor = math.floor for i = 1, 1000000 do s = s + floor(i / 3) end return s",
    ),
    (
        "concat 1e6",
        "local s for i = 1, 1000000 do s = 'item ' .. i end return s",
    ),
    (
        "concat grow 2e4",
        "local s = '' for i = 1, 20000 do s = s .. 'x' end return #s",
    ),
    (
        "strings 1e4",
        "local parts = {} for i = 1, 10000 do parts[#parts+1] = string.format('%d:%s', i, ('x'):rep(3)) end return #table.concat(parts, ',')",
    ),
];

#[test]
#[ignore]
fn speed() {
    for (name, src) in CASES {
        let mut best = std::time::Duration::MAX;
        let mut steps = 0;
        for _ in 0..3 {
            let mut lua = Lua::new();
            lua.steps = u64::MAX;
            let env = new_env(&mut lua);
            let f = lua.load(src, "s", &env).unwrap();
            let t = std::time::Instant::now();
            assert!(lua.call(&mut NoHost::default(), &f, vec![]).is_ok(), "{name}");
            best = best.min(t.elapsed());
            steps = u64::MAX - lua.steps;
        }
        println!(
            "{name:<18} {:>9.2} ms  ({steps} steps, {:.1} ns/step)",
            best.as_secs_f64() * 1000.0,
            best.as_nanos() as f64 / steps as f64
        );
    }
}

/// What a Roblox script costs the studio's frame with the world mirrored:
/// the most entities mirrored (256), 40 of them other players, and a script
/// doing each case's work every frame.
const WORLD_CASES: &[(&str, &str)] = &[
    ("no script looks", "print(game.Players.LocalPlayer.Name)"),
    ("mirrored", "print(#workspace:GetChildren())"),
    (
        "mirrored, the mouse read twice",
        r#"
        local mouse = game.Players.LocalPlayer:GetMouse()
        print(#workspace:GetChildren())
        game:GetService("RunService").Heartbeat:Connect(function()
            local _ = mouse.Hit, mouse.Target
        end)
        "#,
    ),
    (
        "mirrored, a look at each player",
        r#"
        local Players = game:GetService("Players")
        local me = Players.LocalPlayer
        game:GetService("RunService").Heartbeat:Connect(function()
            local eyes = me.Character.Head.Position
            for _, p in Players:GetPlayers() do
                local c = p ~= me and p.Character
                if c then local _ = c.Head.Position - eyes end
            end
        end)
        "#,
    ),
    (
        "mirrored, a ray to each player",
        r#"
        local Players = game:GetService("Players")
        local me = Players.LocalPlayer
        local params = RaycastParams.new()
        params.FilterDescendantsInstances = { me.Character }
        game:GetService("RunService").Heartbeat:Connect(function()
            local eyes = me.Character.Head.Position
            for _, p in Players:GetPlayers() do
                local c = p ~= me and p.Character
                if c then
                    local hit = workspace:Raycast(eyes, c.Head.Position - eyes, params)
                    local _ = hit and hit.Instance:IsDescendantOf(c)
                end
            end
        end)
        "#,
    ),
    (
        "mirrored, a bounds query",
        r#"
        local root = game.Players.LocalPlayer.Character.HumanoidRootPart
        game:GetService("RunService").Heartbeat:Connect(function()
            local _ = #workspace:GetPartBoundsInRadius(root.Position, 8)
        end)
        "#,
    ),
];

/// The fake game's world for WORLD_CASES: the player at (10.5, 64, -3.25),
/// 40 other players about them and 216 zombies on a grid around.
fn crowd(game: &mut FakeGame) {
    let me = game.player.as_mut().unwrap();
    me.name = "Steve".into();
    me.uuid = 1;
    let mut list = vec![PlayerEntry {
        uuid: 1,
        name: "Steve".into(),
        display_name: "Steve".into(),
        entity: None,
        local: true,
        latency: 20,
    }];
    let mut entities = Vec::new();
    for i in 0..40u32 {
        let a = f64::from(i) / 40.0 * std::f64::consts::TAU;
        let name = format!("Player{}", i + 2);
        list.push(PlayerEntry {
            uuid: u128::from(i) + 2,
            name: name.clone(),
            display_name: name.clone(),
            entity: Some(1000 + i),
            local: false,
            latency: 50,
        });
        entities.push(EntityInfo {
            id: 1000 + i,
            kind: "minecraft:player".into(),
            name,
            pos: [10.5 + 8.0 * a.cos(), 64.0, -3.25 + 8.0 * a.sin()],
            width: 0.6,
            height: 1.8,
            eye: 1.62,
            health: 20.0,
            max_health: 20.0,
            speed: 0.1,
            on_ground: true,
            ..EntityInfo::default()
        });
    }
    for i in 0..216u32 {
        let (x, z) = (f64::from(i % 18) * 3.0 - 27.0, f64::from(i / 18) * 3.0 - 18.0);
        entities.push(EntityInfo {
            id: 2000 + i,
            kind: "minecraft:zombie".into(),
            pos: [10.5 + x, 64.0, -3.25 + z],
            width: 0.6,
            height: 1.95,
            eye: 1.74,
            health: 20.0,
            max_health: 20.0,
            speed: 0.23,
            on_ground: true,
            ..EntityInfo::default()
        });
    }
    game.players = Some(list);
    game.entities = entities;
}

#[test]
#[ignore]
fn world_speed() {
    for (name, src) in WORLD_CASES {
        let mut game = FakeGame::new("speed-world");
        crowd(&mut game);
        let mut st = studio(&mut game);
        game.write_mod("case.lua", src);
        run_for(&mut st, &mut game, 1.0);
        let mut best = std::time::Duration::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            for _ in 0..100 {
                game.ticks += 1;
                st.frame(&mut game, 0.05);
            }
            best = best.min(t.elapsed());
        }
        let ws = st.s.rbx.workspace;
        let mirrored = st.s.rbx.children(ws).len().saturating_sub(3);
        println!(
            "{name:<34} {:>7.3} ms a frame  ({mirrored} mirrored)",
            best.as_secs_f64() * 1000.0 / 100.0
        );
    }
}
