// The bytecode VM: coroutines (the library and the host's API), yields
// across pcall and the C-call boundary, frame limits, closures shared
// between coroutines and swapped in place, userdata, readonly tables, the
// object registry, and the compiler's Luau-like constants and registers.

use horizon_studio::game::hstudio_lua::{
    Closure, Host, Lua, Native, NoHost, Res, Resume, Table, TableRef, Throw, Userdata, Value,
};
use horizon_studio::game::hstudio_lualib::{inspect, new_env};
use std::cell::RefCell;
use std::rc::Rc;

/// A fresh interpreter with the standard library and the test's natives.
fn lua() -> (Lua, TableRef) {
    let mut lua = Lua::new();
    lua.steps = 5_000_000;
    let env = new_env(&mut lua);
    {
        let mut g = env.borrow_mut();
        g.set_str("make", Native::builtin("make", make));
        g.set_str("typeof", Native::builtin("typeof", typeof_));
        g.set_str("getnamecallmethod", Native::builtin("getnamecallmethod", namecall));
        g.set_str("getstack", Native::builtin("getstack", getstack));
        g.set_str("setstack", Native::builtin("setstack", setstack));
        g.set_str("snap", Native::builtin("snap", snap));
    }
    (lua, env)
}

/// `make(mt)`: a userdata with that metatable.
fn make(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mt = match args.first() {
        Some(Value::Table(t)) => Some(t.clone()),
        _ => None,
    };
    Ok(vec![Userdata::value(7u32, mt)])
}

fn typeof_(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = lua.type_of(&args.first().cloned().unwrap_or_default());
    Ok(vec![Value::str(&t)])
}

fn namecall(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    Ok(vec![lua.namecall_method().map_or(Value::Nil, Value::Str)])
}

/// `getstack(level, i)`: register i (from 1) of the frame `level` up.
fn getstack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let level = args[0].as_num().unwrap_or(1.0) as usize;
    let i = args[1].as_num().unwrap_or(1.0) as usize;
    Ok(vec![lua.get_stack(level, i - 1).unwrap_or_default()])
}

fn setstack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let level = args[0].as_num().unwrap_or(1.0) as usize;
    let i = args[1].as_num().unwrap_or(1.0) as usize;
    lua.set_stack(level, i - 1, args[2].clone());
    Ok(Vec::new())
}

thread_local! {
    static SNAP: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

/// `snap()`: keeps the caller's registers for the test to look at.
fn snap(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    let regs = lua.stack_values(1).unwrap_or_default();
    SNAP.with(|s| *s.borrow_mut() = regs);
    Ok(Vec::new())
}

fn try_eval(src: &str) -> Result<String, String> {
    let (mut lua, env) = lua();
    let f = lua.load(src, "test.lua", &env)?;
    let r = lua
        .call(&mut NoHost::default(), &f, Vec::new())
        .map_err(|e| e.message())?;
    Ok(r.iter().map(|v| inspect(v, 3)).collect::<Vec<_>>().join(", "))
}

/// What `src` returns, as `inspect` writes the values.
fn eval(src: &str) -> String {
    try_eval(src).unwrap_or_else(|e| panic!("{e}\n--- script ---\n{src}"))
}

fn error_of(src: &str) -> String {
    try_eval(src).expect_err("the script should fail")
}

/// Runs a chunk that returns a function; that function.
fn function(src: &str) -> (Lua, Rc<Closure>) {
    let (mut lua, env) = lua();
    let f = lua.load(src, "test.lua", &env).unwrap();
    let r = lua.call(&mut NoHost::default(), &f, Vec::new()).ok().unwrap();
    match r.into_iter().next() {
        Some(Value::Func(c)) => (lua, c),
        _ => panic!("the chunk should return a Lua function"),
    }
}

// ----------------------------------------------------------------- coroutines

#[test]
fn create_resume_yield_and_values_both_ways() {
    assert_eq!(
        eval(
            "local co = coroutine.create(function(a, b)
               local c = coroutine.yield(a + b)
               local d, e = coroutine.yield(c * 2)
               return d .. e, 'end'
             end)
             local r1 = {coroutine.resume(co, 1, 2)}
             local r2 = {coroutine.resume(co, 10)}
             local r3 = {coroutine.resume(co, 'x', 'y')}
             return r1, r2, r3, type(co)"
        ),
        "{ true, 3 }, { true, 20 }, { true, \"xy\", \"end\" }, \"thread\""
    );
}

#[test]
fn status_running_and_normal() {
    assert_eq!(
        eval(
            "local co
             co = coroutine.create(function()
               local inner = coroutine.create(function()
                 return coroutine.status(co), coroutine.status(coroutine.running())
               end)
               local _, a, b = coroutine.resume(inner)
               coroutine.yield(coroutine.status(co), a, b)
             end)
             local s0 = coroutine.status(co)
             local _, s1, a, b = coroutine.resume(co)
             local s2 = coroutine.status(co)
             coroutine.resume(co)
             local s3 = coroutine.status(co)
             return s0, s1, a, b, s2, s3, coroutine.resume(co)"
        ),
        "\"suspended\", \"running\", \"normal\", \"running\", \"suspended\", \"dead\", false, \"cannot resume dead coroutine\""
    );
    assert_eq!(
        eval(
            "local co = coroutine.create(function() return coroutine.resume(coroutine.running()) end)
             return coroutine.resume(co)"
        ),
        "true, false, \"cannot resume non-suspended coroutine\""
    );
}

#[test]
fn running_and_isyieldable() {
    assert_eq!(
        eval(
            "local main, ismain = coroutine.running()
             local co = coroutine.create(function()
               local c, m = coroutine.running()
               local sorted = {}
               table.sort({2, 1}, function(a, b) sorted[#sorted + 1] = coroutine.isyieldable() return a < b end)
               return c, m, coroutine.isyieldable(), sorted[1]
             end)
             local _, c, m, y, in_sort = coroutine.resume(co)
             return type(main), ismain, c == co, m, y, in_sort, coroutine.isyieldable()"
        ),
        "\"thread\", true, true, false, true, false, false"
    );
}

#[test]
fn wrap_generators_and_errors_with_position() {
    assert_eq!(
        eval(
            "local function range(n)
               return coroutine.wrap(function() for i = 1, n do coroutine.yield(i) end end)
             end
             local out = {}
             for i in range(4) do out[#out + 1] = i end
             return table.concat(out, ' ')"
        ),
        "\"1 2 3 4\""
    );
    // Generators inside generators.
    assert_eq!(
        eval(
            "local function walk(t)
               return coroutine.wrap(function()
                 for _, v in ipairs(t) do
                   if type(v) == 'table' then
                     for x in walk(v) do coroutine.yield(x) end
                   else
                     coroutine.yield(v)
                   end
                 end
               end)
             end
             local out = {}
             for x in walk({1, {2, {3, 4}}, 5}) do out[#out + 1] = x end
             return table.concat(out, ',')"
        ),
        "\"1,2,3,4,5\""
    );
    assert_eq!(
        error_of("local f = coroutine.wrap(function()\n  error('boom')\nend)\nf()"),
        "test.lua:4: test.lua:2: boom"
    );
    assert_eq!(
        error_of("local f = coroutine.wrap(function() end)\nf()\nf()"),
        "test.lua:3: cannot resume dead coroutine"
    );
    assert_eq!(
        error_of("coroutine.wrap(42)"),
        "test.lua:1: bad argument #1 to 'wrap' (function expected, got number)"
    );
}

#[test]
fn errors_inside_coroutines() {
    assert_eq!(
        eval(
            "local co = coroutine.create(function()\n  local t = nil\n  return t.x\nend)
             local ok, err = coroutine.resume(co)
             return ok, err, coroutine.status(co)"
        ),
        "false, \"test.lua:3: attempt to index a nil value (local 't')\", \"dead\""
    );
    // Error values that are not strings come through unchanged.
    assert_eq!(
        eval(
            "local co = coroutine.create(function() error({code = 7}) end) local _, e = coroutine.resume(co) return e.code"
        ),
        "7"
    );
    assert_eq!(
        error_of("coroutine.resume(5)"),
        "test.lua:1: bad argument #1 to 'resume' (coroutine expected)"
    );
}

#[test]
fn close() {
    assert_eq!(
        eval(
            "local co = coroutine.create(function() coroutine.yield() end)
             coroutine.resume(co)
             local a = coroutine.close(co)
             local s = coroutine.status(co)
             local bad = coroutine.create(function()\n error('oops')\n end)
             coroutine.resume(bad)
             local ok, err = coroutine.close(bad)
             local ok2, err2 = pcall(coroutine.close, coroutine.running())
             return a, s, ok, err, coroutine.close(bad), ok2, err2"
        ),
        "true, \"dead\", false, \"test.lua:6: oops\", true, false, \"test.lua:10: cannot close a running coroutine\""
    );
}

#[test]
fn yields_pass_through_pcall_and_xpcall() {
    assert_eq!(
        eval(
            "local co = coroutine.create(function()
               local ok, err = pcall(function()
                 local v = coroutine.yield('first')
                 error('got ' .. v, 0)
               end)
               local ok2, v2 = pcall(coroutine.yield, 'second')
               local r = {xpcall(function() return coroutine.yield('third') + 1 end, function(m) return 'handled' end)}
               local h = {xpcall(function() coroutine.yield('fourth') error('e', 0) end, function(m) return 'h:' .. m end)}
               return ok, err, ok2, v2, r[1], r[2], h[1], h[2]
             end)
             local _, a = coroutine.resume(co)
             local _, b = coroutine.resume(co, 'x')
             local _, c = coroutine.resume(co, 'y')
             local _, d = coroutine.resume(co, 41)
             return a, b, c, d, coroutine.resume(co)"
        ),
        "\"first\", \"second\", \"third\", \"fourth\", true, false, \"got x\", true, \"y\", true, 42, false, \"h:e\""
    );
    // pcall inside pcall, with a yield in the middle.
    assert_eq!(
        eval(
            "local co = coroutine.wrap(function()
               return pcall(pcall, function() coroutine.yield(1) error('deep', 0) end)
             end)
             co()
             return co()"
        ),
        "true, false, \"deep\""
    );
    // Lua iterators may yield too.
    assert_eq!(
        eval(
            "local co = coroutine.wrap(function()
               local function iter(_, i) i = i + 1 if i > 3 then return nil end coroutine.yield('tick ' .. i) return i end
               local n = 0
               for i in iter, nil, 0 do n = n + i end
               return 'sum ' .. n
             end)
             return co(), co(), co(), co()"
        ),
        "\"tick 1\", \"tick 2\", \"tick 3\", \"sum 6\""
    );
}

#[test]
fn a_native_can_be_the_coroutine() {
    assert_eq!(
        eval(
            "local co = coroutine.create(coroutine.yield)
             local a = {coroutine.resume(co, 1, 2)}
             local b = {coroutine.resume(co, 3)}
             return a, b, coroutine.status(co)"
        ),
        "{ true, 1, 2 }, { true, 3 }, \"dead\""
    );
}

#[test]
fn yielding_across_a_c_call_boundary_is_an_error() {
    assert_eq!(
        eval(
            "local co = coroutine.create(function()
               table.sort({3, 2, 1}, function(a, b) coroutine.yield() return a < b end)
             end)
             return coroutine.resume(co)"
        ),
        "false, \"test.lua:2: attempt to yield across a C-call boundary\""
    );
    assert_eq!(
        eval(
            "local t = setmetatable({}, {__index = function(t, k) return coroutine.yield(k) end})
             local co = coroutine.create(function() return t.x end)
             return coroutine.resume(co)"
        ),
        "false, \"test.lua:1: attempt to yield across a C-call boundary\""
    );
    assert_eq!(
        error_of("coroutine.yield(1)"),
        "test.lua:1: attempt to yield from outside a coroutine"
    );
}

#[test]
fn the_budget_stops_a_coroutine_pcall_or_not() {
    let (mut lua, env) = lua();
    lua.steps = 10_000;
    let f = lua
        .load(
            "local co = coroutine.wrap(function() while true do end end) print(pcall(co))",
            "spin.lua",
            &env,
        )
        .unwrap();
    let mut host = NoHost::default();
    match lua.call(&mut host, &f, Vec::new()) {
        Err(Throw::Fatal(m)) => assert!(m.contains("ran too long"), "{m}"),
        _ => panic!("the loop should have been stopped"),
    }
    assert!(host.printed.is_empty());
}

// ----------------------------------------------------------------- the host's API

/// A host whose `wait` yields what it was given, as a scheduler's would.
struct Scheduler;

impl Host for Scheduler {
    fn call(&mut self, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res {
        match id {
            1 => Err(Throw::Yield(args)),
            _ => Err(lua.rt_error("no such function")),
        }
    }
    fn print(&mut self, _: &str) {}
}

fn nums(v: &[Value]) -> Vec<f64> {
    v.iter().map(|v| v.as_num().unwrap_or(f64::NAN)).collect()
}

#[test]
fn the_host_resumes_threads() {
    let (mut lua, env) = lua();
    env.borrow_mut().set_str("wait", Native::host("wait", 1));
    let f = lua
        .load(
            "local total = 0
             for i = 1, 3 do
               local ok, got = pcall(wait, i)
               total = total + got
             end
             return total, 'done'",
            "task.lua",
            &env,
        )
        .unwrap();
    let mut h = Scheduler;
    let co = lua.new_thread(f);
    assert!(co.is_suspended());
    let mut sent = Vec::new();
    for give in [None, Some(10.0), Some(20.0)] {
        let args = give.map(Value::Num).into_iter().collect();
        match lua.resume(&mut h, &co, args) {
            Resume::Yielded(v) => sent.extend(nums(&v)),
            _ => panic!("it should yield"),
        }
        assert!(co.is_suspended());
    }
    assert_eq!(sent, vec![1.0, 2.0, 3.0]);
    match lua.resume(&mut h, &co, vec![Value::Num(30.0)]) {
        Resume::Done(v) => {
            assert_eq!(v[0].as_num(), Some(60.0));
            assert_eq!(v[1].as_text().as_deref(), Some("done"));
        }
        _ => panic!("it should finish"),
    }
    assert!(co.is_dead());
    assert!(matches!(lua.resume(&mut h, &co, vec![]), Resume::Error(_)));
    // A thread that fails: its error, and it is dead.
    let f = lua.load("wait()\nerror('boom')", "task.lua", &env).unwrap();
    let co = lua.new_thread(f);
    assert!(matches!(lua.resume(&mut h, &co, vec![]), Resume::Yielded(_)));
    match lua.resume(&mut h, &co, vec![]) {
        Resume::Error(e) => assert_eq!(e.message(), "task.lua:2: boom"),
        _ => panic!("it should fail"),
    }
    assert!(co.is_dead());
    // Outside a thread, the native's yield is an error.
    let f = lua.load("wait(1)", "main.lua", &env).unwrap();
    match lua.call(&mut h, &f, vec![]) {
        Err(e) => assert_eq!(e.message(), "main.lua:1: attempt to yield from outside a coroutine"),
        Ok(_) => panic!("it should fail"),
    }
    // The interpreter is still sound after all that.
    let f = lua.load("return 1 + 1", "x.lua", &env).unwrap();
    assert_eq!(lua.call(&mut h, &f, vec![]).ok().unwrap()[0].as_num(), Some(2.0));
}

// ----------------------------------------------------------------- frames

#[test]
fn deep_recursion_is_a_stack_overflow_not_a_crash() {
    assert!(error_of("local function f(n) return f(n + 1) end f(1)").contains("stack overflow"));
    assert_eq!(
        eval("local depth = 0 local function f() depth = depth + 1 f() end pcall(f) return depth"),
        "199"
    );
    // A coroutine has frames of its own, as many (its function is one).
    assert_eq!(
        eval(
            "local depth = 0
             local co = coroutine.create(function() local function f() depth = depth + 1 return f() + 1 end return f() end)
             local ok, err = coroutine.resume(co)
             return ok, depth, err:find('stack overflow') ~= nil"
        ),
        "false, 199, true"
    );
    // Recursion through natives and resumes stops too.
    assert!(
        error_of("local t = setmetatable({}, {__index = function(t, k) return t[k] end}) return t.x")
            .contains("stack overflow")
    );
    assert!(
        error_of(
            "local function nest() local co = coroutine.create(nest) local ok, e = coroutine.resume(co) if not ok then error(e, 0) end end nest()"
        )
        .contains("stack overflow")
    );
    assert!(error_of("local function f() return table.sort({1, 2}, f) end f()").contains("stack overflow"));
}

#[test]
fn closures_share_upvalues_across_coroutines() {
    assert_eq!(
        eval(
            "local count = 0
             local function inc() count = count + 1 return count end
             local co = coroutine.wrap(function()
               while true do inc() coroutine.yield(count) end
             end)
             local a = co()
             inc()
             local b = co()
             return a, b, count, inc()"
        ),
        "1, 3, 3, 4"
    );
    assert_eq!(
        eval(
            "local get, set
             local co = coroutine.create(function()
               local x = 1
               get = function() return x end
               set = function(v) x = v end
               coroutine.yield()
               return x
             end)
             coroutine.resume(co)
             set(42)
             local _, r = coroutine.resume(co)
             set(7)
             return get(), r, coroutine.status(co)"
        ),
        "7, 42, \"dead\""
    );
}

#[test]
fn a_closure_can_run_another_body_in_place() {
    let (mut lua, old) = function("return function() return 'old' end");
    let (_, hook) = function("local tag = 'new' return function() return tag end");
    let f = Value::Func(old.clone());
    let before = f.clone();
    let original = old.replace_body(hook.body());
    let mut h = NoHost::default();
    assert_eq!(
        lua.call1(&mut h, &f, vec![]).ok().unwrap().as_text().as_deref(),
        Some("new")
    );
    assert!(f.raw_eq(&before), "the same function, by identity");
    let orig = Value::Func(Rc::new(Closure::from_body(original)));
    assert_eq!(
        lua.call1(&mut h, &orig, vec![]).ok().unwrap().as_text().as_deref(),
        Some("old")
    );
    assert!(!orig.raw_eq(&f));
}

#[test]
fn frames_and_upvalues_are_open_to_inspection() {
    // debug.getstack and setstack, as sUNC tests them.
    assert_eq!(eval("local _ = 'a' .. 'b' return getstack(1, 1)"), "\"ab\"");
    assert_eq!(
        eval("local function test() return 'fail', setstack(1, 1, 'success') end return test()"),
        "\"success\""
    );
    // getupvalue and setupvalue.
    let (mut lua, test) = function(
        "local upvalue = function() return 'fail' end
         local function test() return upvalue() end
         return test",
    );
    assert!(matches!(test.upvalue(0), Some(Value::Func(_))));
    let (_, success) = function("return function() return 'success' end");
    assert!(test.set_upvalue(0, Value::Func(success)));
    let r = lua.call1(&mut NoHost::default(), &Value::Func(test.clone()), vec![]);
    assert_eq!(r.ok().unwrap().as_text().as_deref(), Some("success"));
    // What debug.getinfo reads.
    let (_, f) = function("local function named(a, b, ...) print(a) end return named");
    let p = f.proto();
    assert_eq!(
        (&*p.chunk, p.line, p.nparams, p.vararg, p.upvals.len()),
        ("test.lua", 1, 2, true, 0)
    );
    assert_eq!(p.debug_name.as_deref(), Some("named"));
}

// ----------------------------------------------------------------- Luau's layout

fn slots(c: &Closure) -> Vec<Option<String>> {
    c.proto()
        .constant_slots()
        .into_iter()
        .map(|v| v.map(|v| v.plain_text()))
        .collect()
}

fn some(v: &[&str]) -> Vec<Option<String>> {
    v.iter().map(|s| Some((*s).to_owned())).collect()
}

#[test]
fn constants_are_numbered_as_luau_numbers_them() {
    let (_, f) = function("local function f()\n  local s = \"foo bar\"\n  string.split(s, \" \")\nend\nreturn f");
    assert_eq!(
        slots(&f),
        vec![
            Some("string".into()),
            Some("split".into()),
            None,
            Some("foo bar".into()),
            Some(" ".into())
        ]
    );
    // debug.getconstants' own test.
    let (_, f) = function("local function test()\n  print(\"Hello, world!\")\nend\nreturn test");
    assert_eq!(
        slots(&f),
        vec![Some("print".into()), None, Some("Hello, world!".into())]
    );
    // A global the chunk assigns to is read by name, not imported.
    let (_, f) = function("helper = 1\nreturn function() return helper.x end");
    assert_eq!(slots(&f), some(&["helper", "x"]));
    // Small whole numbers need no constant; others do, in order.
    let (_, f) = function("return function(x) return x + 1, 2, 1.5, 100000 end");
    assert_eq!(slots(&f), some(&["1", "1.5", "100000"]));
    // A constant local is folded into a closure too: no upvalue.
    let (_, f) = function("local x = 'shared'\nreturn function() return x end");
    assert!(f.proto().upvals.is_empty());
    assert_eq!(slots(&f), some(&["shared"]));
    // A table of records gets a template after its keys.
    let (_, f) = function("return function() return {x = 1, y = 'z'} end");
    assert_eq!(
        slots(&f),
        vec![Some("x".into()), Some("y".into()), None, Some("z".into())]
    );
}

#[test]
fn registers_are_parameters_then_locals_then_temporaries() {
    let (mut lua, env) = lua();
    let src = "local a = 29 local b = true local c = 'Example'
               a = a + 1 b = false c = c .. 's'
               print(snap())";
    let f = lua.load(src, "test.lua", &env).unwrap();
    lua.call(&mut NoHost::default(), &f, vec![]).ok().unwrap();
    let regs = SNAP.with(|s| s.borrow().clone());
    assert_eq!(regs[0].as_num(), Some(30.0));
    assert!(matches!(regs[1], Value::Bool(false)));
    assert_eq!(regs[2].as_text().as_deref(), Some("Examples"));
    let g = env.borrow();
    assert!(regs[3].raw_eq(&g.get_str("print")), "then the call's function");
    assert!(regs[4].raw_eq(&g.get_str("snap")), "then its arguments");
    // Parameters come first, and a constant local takes no register.
    let (mut lua, f) = function("local k = 5 return function(p, q) local r = p + k snap() end");
    lua.call(
        &mut NoHost::default(),
        &Value::Func(f),
        vec![Value::Num(1.0), Value::Num(2.0)],
    )
    .ok()
    .unwrap();
    let regs = SNAP.with(|s| s.borrow().clone());
    assert_eq!(nums(&regs[..3]), vec![1.0, 2.0, 6.0]);
}

#[test]
fn child_functions_in_order_with_their_names() {
    let (mut lua, env) = lua();
    let f = lua
        .load(
            "local function a() end
             function b() end
             local t = {}
             function t.c() end
             local d = function() end
             function t:e() local function inner() end end",
            "test.lua",
            &env,
        )
        .unwrap();
    let Value::Func(c) = f else { panic!() };
    let p = c.proto();
    let names: Vec<Option<&str>> = p.protos.iter().map(|p| p.debug_name.as_deref()).collect();
    assert_eq!(names, vec![Some("a"), Some("b"), Some("c"), None, Some("e")]);
    assert_eq!(p.protos[4].protos[0].debug_name.as_deref(), Some("inner"));
    // Tracebacks keep their own names.
    assert_eq!(&*p.protos[2].name, "function 't.c'");
}

// ----------------------------------------------------------------- userdata

#[test]
fn userdata_with_metamethods() {
    assert_eq!(
        eval(
            "local mt = {
               __index = function(u, k) return 'get ' .. k end,
               __newindex = function(u, k, v) last = k .. '=' .. tostring(v) end,
               __call = function(u, a) return 'called ' .. a end,
               __add = function(a, b) return 'add' end,
               __eq = function(a, b) return true end,
               __lt = function(a, b) return true end,
               __le = function(a, b) return false end,
               __len = function(u) return 42 end,
               __concat = function(a, b) return 'cat' end,
               __unm = function(u) return 'neg' end,
               __tostring = function(u) return 'UD' end,
               __type = 'Thing',
             }
             local u, v = make(mt), make(mt)
             u.x = 5
             return type(u), typeof(u), u.foo, last, u(1), u + 1, u == v, u == u, u < v, u <= v, #u, u .. 's', -u, tostring(u), rawequal(u, v)"
        ),
        "\"userdata\", \"Thing\", \"get foo\", \"x=5\", \"called 1\", \"add\", true, true, true, false, 42, \"cat\", \"neg\", \"UD\", false"
    );
    assert_eq!(
        error_of("local u = make()\nreturn u.x"),
        "test.lua:2: attempt to index a userdata value (local 'u')"
    );
    assert_eq!(
        eval("return typeof(make()), typeof(1), make() == make()"),
        "\"userdata\", \"number\", false"
    );
    // __eq is for two userdata only.
    assert_eq!(
        eval("local u = make({__eq = function() return true end}) return u == 1"),
        "false"
    );
}

#[test]
fn namecall_takes_method_calls_on_userdata() {
    assert_eq!(
        eval(
            "local u = make({ __namecall = function(self, ...) return getnamecallmethod(), select('#', ...), ... end })
             return u:Foo(1, 2)"
        ),
        "\"Foo\", 2, 1, 2"
    );
    assert_eq!(
        eval("local u = make({ __index = { Bar = function(self, x) return x * 2 end } }) return u:Bar(21)"),
        "42"
    );
    assert_eq!(
        error_of("local u = make({})\nu:Nope()"),
        "test.lua:2: attempt to index a userdata value"
    );
}

// ----------------------------------------------------------------- tables

#[test]
fn readonly_tables_refuse_writes() {
    let (mut lua, env) = lua();
    let mut t = Table::from_list(vec![Value::Num(1.0), Value::Num(2.0)]);
    t.set_str("x", Value::Num(1.0));
    t.set_readonly(true);
    assert!(t.is_readonly());
    env.borrow_mut().set_str("frozen", Value::table(t));
    let mut h = NoHost::default();
    for src in [
        "frozen.x = 2",
        "frozen.new = 1",
        "frozen[3] = 3",
        "rawset(frozen, 'y', 1)",
        "table.insert(frozen, 3)",
        "table.remove(frozen)",
        "table.sort(frozen)",
        "setmetatable(frozen, {})",
    ] {
        let f = lua.load(src, "test.lua", &env).unwrap();
        match lua.call(&mut h, &f, vec![]) {
            Err(e) => assert_eq!(e.message(), "test.lua:1: attempt to modify a readonly table", "{src}"),
            Ok(_) => panic!("{src} should fail"),
        }
    }
    let f = lua
        .load(
            "local n = 0 for k, v in pairs(frozen) do n = n + 1 end return frozen.x, #frozen, n, pcall(function() frozen.x = 1 end)",
            "test.lua",
            &env,
        )
        .unwrap();
    let r = lua.call(&mut h, &f, vec![]).ok().unwrap();
    assert_eq!(
        r.iter().map(|v| inspect(v, 1)).collect::<Vec<_>>().join(", "),
        "1, 2, 3, false, \"test.lua:1: attempt to modify a readonly table\""
    );
}

#[test]
fn the_registry_finds_live_closures_and_tables() {
    let (mut lua, env) = lua();
    let mut h = NoHost::default();
    assert!(lua.live_closures().is_empty(), "off by default");
    lua.track_objects(true);
    let f = lua
        .load(
            "keep = {} for i = 1, 3 do keep[i] = function() return i end end local gone = {}",
            "test.lua",
            &env,
        )
        .unwrap();
    lua.call(&mut h, &f, vec![]).ok().unwrap();
    // The chunk and the three closures it kept.
    assert_eq!(lua.live_closures().len(), 4);
    assert_eq!(lua.live_tables().len(), 1);
    let f = lua.load("keep = nil", "test.lua", &env).unwrap();
    lua.call(&mut h, &f, vec![]).ok().unwrap();
    assert_eq!(lua.live_closures().len(), 2, "the two chunks");
    assert!(lua.live_tables().is_empty());
}

#[test]
fn the_stack_and_frames_after_errors_stay_sound() {
    let (mut lua, env) = lua();
    let mut h = NoHost::default();
    for _ in 0..3 {
        let f = lua
            .load(
                "local co = coroutine.wrap(function() error('x') end) pcall(co) error('y')",
                "t.lua",
                &env,
            )
            .unwrap();
        assert!(lua.call(&mut h, &f, vec![]).is_err());
    }
    let f = lua.load("return select('#', 1, 2, 3)", "t.lua", &env).unwrap();
    assert_eq!(lua.call(&mut h, &f, vec![]).ok().unwrap()[0].as_num(), Some(3.0));
    assert!(lua.frame_info(1).is_none(), "no frames left behind");
}
