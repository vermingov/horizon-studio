// Timings of hot loops: `cargo test --release --test speed -- --ignored --nocapture`.
// Each case runs three times; the best time counts.

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
