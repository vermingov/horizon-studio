use horizon_studio::game::hstudio_lua::{Lua, NoHost};
use horizon_studio::game::hstudio_lualib::new_env;
#[test]
#[ignore]
fn speed() {
    for (name, src) in [
        (
            "fib(25)",
            "local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end return fib(25)",
        ),
        (
            "loop 1e6",
            "local s = 0 for i = 1, 1000000 do s = s + i % 7 end return s",
        ),
        (
            "table 1e5",
            "local t = {} for i = 1, 100000 do t[i] = i * 2 end local s = 0 for _, v in ipairs(t) do s = s + v end return s",
        ),
        (
            "strings 1e4",
            "local parts = {} for i = 1, 10000 do parts[#parts+1] = string.format('%d:%s', i, ('x'):rep(3)) end return #table.concat(parts, ',')",
        ),
    ] {
        let mut lua = Lua::new();
        lua.steps = u64::MAX;
        let env = new_env(&mut lua);
        let f = lua.load(src, "s", &env).unwrap();
        let t = std::time::Instant::now();
        let steps0 = lua.steps;
        assert!(lua.call(&mut NoHost::default(), &f, vec![]).is_ok());
        let steps = steps0 - lua.steps;
        println!(
            "{name}: {:?} ({steps} steps, {:.0} ns/step)",
            t.elapsed(),
            t.elapsed().as_nanos() as f64 / steps as f64
        );
    }
}
