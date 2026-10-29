// The interpreter against Lua 5.3's behaviour, script by script.

use horizon_studio::game::hstudio_lua::{Lua, NoHost, Throw, Value};
use horizon_studio::game::hstudio_lualib::{inspect, new_env};

/// Runs `src` and returns what it printed, one line each.
fn run(src: &str) -> Vec<String> {
    match try_run(src) {
        Ok(lines) => lines,
        Err(e) => panic!("script failed: {e}\n--- script ---\n{src}"),
    }
}

fn try_run(src: &str) -> Result<Vec<String>, String> {
    let mut lua = Lua::new();
    lua.steps = 5_000_000;
    let env = new_env(&mut lua);
    let f = lua.load(src, "test.lua", &env)?;
    let mut host = NoHost::default();
    match lua.call(&mut host, &f, Vec::new()) {
        Ok(_) => Ok(host.printed),
        Err(e) => Err(e.message()),
    }
}

/// The values the script returns, as `inspect` writes them.
fn eval(src: &str) -> String {
    let mut lua = Lua::new();
    let env = new_env(&mut lua);
    let f = lua.load(src, "test.lua", &env).unwrap_or_else(|e| panic!("{e}"));
    let mut host = NoHost::default();
    let r = lua
        .call(&mut host, &f, Vec::new())
        .unwrap_or_else(|e| panic!("{}", e.message()));
    r.iter().map(|v| inspect(v, 3)).collect::<Vec<_>>().join(", ")
}

fn error_of(src: &str) -> String {
    try_run(src).expect_err("the script should fail")
}

#[test]
fn arithmetic_and_number_printing() {
    assert_eq!(
        eval("return 1 + 2, 7 / 2, 7 // 2, -7 // 2, 7 % 3, -7 % 3, 2 ^ 10"),
        "3, 3.5, 3, -4, 1, 2, 1024"
    );
    assert_eq!(eval("return 10 / 3"), "3.3333333333333");
    assert_eq!(
        eval("return 1e100, 1/0, -1/0, 2^53"),
        "1e+100, inf, -inf, 9.007199254741e+15"
    );
    assert_eq!(eval("return '10' + 5, '0x10' * 1, 3 .. ''"), "15, 16, \"3\"");
    assert_eq!(eval("return -2 ^ 2, (-2) ^ 2, 2 ^ 3 ^ 2"), "-4, 4, 512");
    assert_eq!(eval("return 5.5 % 2, -5.5 % 2, 5 % -3"), "1.5, 0.5, -1");
    assert_eq!(eval("return 0x1F, 1e3, .5, 3."), "31, 1000, 0.5, 3");
}

#[test]
fn strings_and_escapes() {
    assert_eq!(run(r#"print("a\tb\\n\65\x41\u{48}")"#), vec!["a\tb\\nAAH"]);
    assert_eq!(run("print([[\nline1\nline2]])"), vec!["line1\nline2"]);
    assert_eq!(run("print([==[a]]b]==])"), vec!["a]]b"]);
    assert!(try_run("print('it''s')").unwrap_err().contains("')' expected"));
    assert_eq!(eval("return #'hello', 'ab' < 'b', 'a' .. 1 .. 2"), "5, true, \"a12\"");
}

#[test]
fn comments_are_skipped() {
    assert_eq!(
        run("-- one\nprint(1) --[[ block\n comment ]] print(2)\n--[==[ x ]==] print(3)"),
        vec!["1", "2", "3"]
    );
}

#[test]
fn locals_scopes_and_shadowing() {
    let out = run("
        local x = 1
        do local x = 2; print(x) end
        print(x)
        local a, b, c = 1, 2
        print(a, b, c)
        local x = x + 10
        print(x)
    ");
    assert_eq!(out, vec!["2", "1", "1\t2\tnil", "11"]);
}

#[test]
fn closures_capture_per_iteration() {
    let out = run("
        local fns = {}
        for i = 1, 3 do fns[i] = function() return i end end
        print(fns[1](), fns[2](), fns[3]())
        local function counter()
            local n = 0
            return function() n = n + 1; return n end
        end
        local c1, c2 = counter(), counter()
        c1(); c1()
        print(c1(), c2())
    ");
    assert_eq!(out, vec!["1\t2\t3", "3\t1"]);
}

#[test]
fn shared_upvalues_between_closures() {
    let out = run("
        local function pair()
            local v = 0
            local function get() return v end
            local function set(x) v = x end
            return get, set
        end
        local get, set = pair()
        set(42)
        print(get())
        local outer = 1
        local function a() local function b() outer = outer + 1 end b() end
        a(); a()
        print(outer)
    ");
    assert_eq!(out, vec!["42", "3"]);
}

#[test]
fn recursion_and_local_function() {
    assert_eq!(
        eval("local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end return fib(20)"),
        "6765"
    );
    assert_eq!(
        eval("function fact(n) return n <= 1 and 1 or n * fact(n - 1) end return fact(10)"),
        "3628800"
    );
}

#[test]
fn control_flow() {
    let out = run("
        local i = 0
        while true do i = i + 1; if i >= 5 then break end end
        print(i)
        repeat local j = i; i = i - 1 until j <= 2
        print(i)
        for k = 10, 1, -3 do io_free = (io_free or '') .. k .. ' ' end
        print(io_free)
        for k = 1, 0 do print('never') end
        if i == 1 then print('one') elseif i == 2 then print('two') else print('other') end
        for k = 1, 2, 0.5 do io2 = (io2 or 0) + 1 end
        print(io2)
    ");
    assert_eq!(out, vec!["5", "1", "10 7 4 1 ", "one", "3"]);
}

#[test]
fn varargs_and_multiple_results() {
    let out = run("
        local function f(...) return select('#', ...), ... end
        print(f(1, nil, 3))
        local function g() return 1, 2, 3 end
        local t = {g(), g()}
        print(#t)
        print((g()))
        print(g(), 'x')
        local function sum(...) local s = 0 for _, v in ipairs({...}) do s = s + v end return s end
        print(sum(1, 2, 3, 4))
        print(select(2, 'a', 'b', 'c'))
        print(select(-1, 'a', 'b', 'c'))
        local a, b = (function() return 1, 2 end)()
        print(a, b)
    ");
    assert_eq!(out, vec!["3\t1\tnil\t3", "4", "1", "1\tx", "10", "b\tc", "c", "1\t2"]);
}

#[test]
fn tables_length_and_pairs_order() {
    let out = run("
        local t = {10, 20, 30, x = 1, y = 2, [100] = 'h'}
        print(#t, t[2], t.x, t[100])
        local keys = {}
        for k, v in pairs(t) do keys[#keys + 1] = tostring(k) end
        print(table.concat(keys, ','))
        t[#t + 1] = 40
        t[2] = nil
        print(#t, t[4])
        local u = {}
        u[3] = 'c'; u[2] = 'b'; u[1] = 'a'
        print(#u, table.concat(u))
        local n = 0
        for k in pairs(u) do u[k] = nil; n = n + 1 end
        print(n, next(u))
    ");
    assert_eq!(out, vec!["3\t20\t1\th", "1,2,3,x,y,100", "4\t40", "3\tabc", "3\tnil"]);
}

#[test]
fn table_keys_normalise_numbers() {
    assert_eq!(
        eval("local t = {} t[1.0] = 'a' t[2] = 'b' return t[1], #t, t['1']"),
        "\"a\", 2, nil"
    );
    assert!(error_of("local t = {} t[nil] = 1").contains("index is nil"));
    assert!(error_of("local t = {} t[0/0] = 1").contains("index is NaN"));
}

#[test]
fn methods_and_metatables() {
    let out = run("
        local Vec = {}
        Vec.__index = Vec
        function Vec.new(x, y) return setmetatable({x = x, y = y}, Vec) end
        function Vec:len() return math.sqrt(self.x ^ 2 + self.y ^ 2) end
        Vec.__add = function(a, b) return Vec.new(a.x + b.x, a.y + b.y) end
        Vec.__eq = function(a, b) return a.x == b.x and a.y == b.y end
        Vec.__tostring = function(v) return '(' .. v.x .. ', ' .. v.y .. ')' end
        Vec.__lt = function(a, b) return a:len() < b:len() end
        Vec.__le = function(a, b) return a:len() <= b:len() end
        Vec.__call = function(self, k) return self[k] end
        Vec.__len = function() return 2 end
        Vec.__concat = function(a, b) return tostring(a) .. tostring(b) end
        Vec.__unm = function(a) return Vec.new(-a.x, -a.y) end
        local a, b = Vec.new(3, 4), Vec.new(1, 1)
        print(a:len(), tostring(a + b), a == Vec.new(3, 4), b < a, a <= b, a('x'), #a)
        print(a .. b, tostring(-b))
        local defaults = setmetatable({}, {__index = function(t, k) return k .. '!' end})
        print(defaults.hi, rawget(defaults, 'hi'))
        local log = {}
        local proxy = setmetatable({}, {__newindex = function(t, k, v) rawset(t, k, v * 2) end})
        proxy.a = 5
        print(proxy.a)
        print(getmetatable(a) == Vec)
    ");
    assert_eq!(
        out,
        vec![
            "5\t(4, 5)\ttrue\ttrue\tfalse\t3\t2",
            "(3, 4)(1, 1)\t(-1, -1)",
            "hi!\tnil",
            "10",
            "true"
        ]
    );
}

#[test]
fn inheritance_chain() {
    let out = run("
        local Animal = {} Animal.__index = Animal
        function Animal.new(name) return setmetatable({name = name}, Animal) end
        function Animal:speak() return self.name .. ' makes a sound' end
        local Dog = setmetatable({}, {__index = Animal}) Dog.__index = Dog
        function Dog.new(name) local d = Animal.new(name) return setmetatable(d, Dog) end
        function Dog:speak() return self.name .. ' barks' end
        function Dog:base() return Animal.speak(self) end
        local d = Dog.new('Rex')
        print(d:speak(), d:base())
    ");
    assert_eq!(out, vec!["Rex barks\tRex makes a sound"]);
}

#[test]
fn errors_name_the_culprit() {
    assert_eq!(
        error_of("local x\nprint(x.y)"),
        "test.lua:2: attempt to index a nil value (local 'x')"
    );
    assert_eq!(
        error_of("foo()"),
        "test.lua:1: attempt to call a nil value (global 'foo')"
    );
    assert_eq!(
        error_of("local t = {}\n\nt.a.b = 1"),
        "test.lua:3: attempt to index a nil value (field 'a')"
    );
    assert_eq!(
        error_of("return 1 + {}"),
        "test.lua:1: attempt to perform arithmetic on a table value"
    );
    assert_eq!(
        error_of("local s = 'x' return s + 1"),
        "test.lua:1: attempt to perform arithmetic on a string value (local 's') (not a number)"
    );
    assert_eq!(
        error_of("return {} < {}"),
        "test.lua:1: attempt to compare two table values"
    );
    assert_eq!(
        error_of("return 1 < 'x'"),
        "test.lua:1: attempt to compare number with string"
    );
    assert_eq!(
        error_of("local t = {} t:nope()"),
        "test.lua:1: attempt to call a nil value (method 'nope')"
    );
    assert_eq!(
        error_of("return #nil"),
        "test.lua:1: attempt to get length of a nil value"
    );
    assert_eq!(
        error_of("return 'a' .. {}"),
        "test.lua:1: attempt to concatenate a table value"
    );
    assert_eq!(error_of("error('boom')"), "test.lua:1: boom");
    assert_eq!(error_of("error({code = 1})"), "(error object is a table value)");
    assert_eq!(error_of("error('plain', 0)"), "plain");
    assert_eq!(
        error_of("local function check(x) if not x then error('x is required', 2) end end\n\ncheck(nil)"),
        "test.lua:3: x is required"
    );
}

#[test]
fn syntax_errors_say_where() {
    assert_eq!(
        try_run("x = = 1").unwrap_err(),
        "test.lua:1: unexpected symbol near '='"
    );
    assert_eq!(
        try_run("if x then\n  print(1)\n").unwrap_err(),
        "test.lua:3: 'end' expected (to close 'if' at line 1) near '<eof>'"
    );
    assert_eq!(try_run("print('abc)").unwrap_err(), "test.lua:1: unfinished string");
    assert_eq!(try_run("break").unwrap_err(), "test.lua:1: break outside a loop");
    assert_eq!(
        try_run("x = 1 +").unwrap_err(),
        "test.lua:1: unexpected symbol near '<eof>'"
    );
    assert!(try_run("goto done").unwrap_err().contains("not supported"));
    assert!(try_run("x = 1 & 2").unwrap_err().contains("bitwise"));
    assert_eq!(
        try_run("f() = 1").unwrap_err(),
        "test.lua:1: syntax error (cannot assign to this)"
    );
    assert_eq!(
        try_run("x").unwrap_err(),
        "test.lua:1: syntax error (expected a call or an assignment) near '<eof>'"
    );
    let deep = format!("return {}1{}", "(".repeat(200), ")".repeat(200));
    assert!(try_run(&deep).unwrap_err().contains("too many syntax levels"));
}

#[test]
fn pcall_and_error_values() {
    let out = run("
        local ok, err = pcall(function() error('bad') end)
        print(ok, err)
        local ok2, v = pcall(function() return 7 end)
        print(ok2, v)
        local ok3, obj = pcall(error, {code = 42})
        print(ok3, obj.code)
        print(select('#', pcall(error)))
        print(xpcall(function() error('deep') end, function(m) return 'handled: ' .. m end))
        print(pcall(nil))
        local ok4, msg = pcall(function() local t = nil; return t.x end)
        print(msg)
    ");
    assert_eq!(
        out,
        vec![
            "false\ttest.lua:2: bad",
            "true\t7",
            "false\t42",
            "2",
            "false\thandled: test.lua:9: deep",
            "false\ttest.lua:10: attempt to call a nil value",
            "test.lua:11: attempt to index a nil value (local 't')",
        ]
    );
}

#[test]
fn budget_stops_endless_loops_even_inside_pcall() {
    let mut lua = Lua::new();
    lua.steps = 10_000;
    let env = new_env(&mut lua);
    let f = lua
        .load(
            "pcall(function() while true do end end) print('escaped')",
            "spin.lua",
            &env,
        )
        .unwrap();
    let mut host = NoHost::default();
    let r = lua.call(&mut host, &f, Vec::new());
    match r {
        Err(Throw::Fatal(m)) => assert!(m.contains("ran too long"), "{m}"),
        _ => panic!("the loop should have been stopped"),
    }
    assert!(host.printed.is_empty());
}

#[test]
fn stack_overflow_is_an_error_not_a_crash() {
    let e = error_of("local function f() return 1 + f() end f()");
    assert!(e.contains("stack overflow"), "{e}");
    assert!(run("print(pcall(function() local function f() f() end f() end))")[0].starts_with("false"));
}

#[test]
fn memory_limits() {
    let e = error_of("local s = 'x' while true do s = s .. s end");
    assert!(e.contains("too long"), "{e}");
    let e = error_of("return string.rep('x', 1e9)");
    assert!(e.contains("too large"), "{e}");
}

#[test]
fn string_library() {
    assert_eq!(
        eval(
            "return ('hello'):upper(), string.sub('hello', 2, -2), ('abc'):rep(3, '-'), ('abc'):reverse(), ('abc'):len()"
        ),
        "\"HELLO\", \"ell\", \"abc-abc-abc\", \"cba\", 3"
    );
    assert_eq!(
        eval(
            "return string.sub('hello', -3), string.sub('hello', 0), string.sub('hello', 4, 100), string.sub('hi', 3)"
        ),
        "\"llo\", \"hello\", \"lo\", \"\""
    );
    assert_eq!(eval("return string.byte('ABC', 1, -1)"), "65, 66, 67");
    assert_eq!(eval("return string.char(72, 105)"), "\"Hi\"");
    assert_eq!(eval("local s = 'x' return s:byte()"), "120");
    assert_eq!(
        eval("return ('  pad  '):trim(), ('a.b'):startswith('a.'), ('a.b'):endswith('x')"),
        "\"pad\", true, false"
    );
    assert_eq!(
        eval("return string.split('give @p diamond 64')"),
        "{ \"give\", \"@p\", \"diamond\", \"64\" }"
    );
    assert_eq!(
        eval("return string.split('a,b,,c', ',')"),
        "{ \"a\", \"b\", \"\", \"c\" }"
    );
    assert_eq!(
        eval("return string.split('a1b22c', '%d+', false)"),
        "{ \"a\", \"b\", \"c\" }"
    );
}

#[test]
fn string_format() {
    assert_eq!(eval("return string.format('%d items', 3)"), "\"3 items\"");
    assert_eq!(
        eval("return string.format('%5d|%-5d|%05d', 42, 42, -42)"),
        "\"   42|42   |-0042\""
    );
    assert_eq!(
        eval("return string.format('%.2f %.0f %10.3f', 3.14159, 2.5, -1.5)"),
        "\"3.14 2     -1.500\""
    );
    assert_eq!(
        eval("return string.format('%x %X %#x %o', 255, 255, 255, 8)"),
        "\"ff FF 0xff 10\""
    );
    assert_eq!(eval("return string.format('%e', 1234.5)"), "\"1.234500e+03\"");
    assert_eq!(
        eval("return string.format('%g %g %g', 0.0001, 1e20, 100)"),
        "\"0.0001 1e+20 100\""
    );
    assert_eq!(eval("return string.format('%s %s %s', 'a', 1, nil)"), "\"a 1 nil\"");
    assert_eq!(
        eval("return string.format('%q', 'say \"hi\"\\n')"),
        "\"\\\"say \\\\\\\"hi\\\\\\\"\\\\n\\\"\""
    );
    assert_eq!(eval("return string.format('%5.1s|', 'abc')"), "\"    a|\"");
    assert_eq!(eval("return string.format('%c%c', 72, 105)"), "\"Hi\"");
    assert_eq!(eval("return string.format('100%%')"), "\"100%\"");
    assert_eq!(eval("return string.format('%+d % d', 5, 5)"), "\"+5  5\"");
    assert!(error_of("return string.format('%d', 1.5)").contains("no integer representation"));
    assert!(error_of("return string.format('%d')").contains("no value"));
}

#[test]
fn patterns_find_and_match() {
    assert_eq!(eval("return string.find('hello world', 'o w')"), "5, 7");
    assert_eq!(eval("return string.find('hello', 'l+')"), "3, 4");
    assert_eq!(eval("return string.find('a.b', '.', 1, true)"), "2, 2");
    assert_eq!(eval("return string.find('abc', 'x')"), "nil");
    assert_eq!(
        eval("return string.match('key=value', '(%w+)=(%w+)')"),
        "\"key\", \"value\""
    );
    assert_eq!(
        eval("return string.match('  trim me  ', '^%s*(.-)%s*$')"),
        "\"trim me\""
    );
    assert_eq!(
        eval("return string.match('2024-01-15', '(%d+)-(%d+)-(%d+)')"),
        "\"2024\", \"01\", \"15\""
    );
    assert_eq!(eval("return string.match('hello', '()ll()')"), "3, 5");
    assert_eq!(eval("return string.match('[[nested]] x', '%b[]')"), "\"[[nested]]\"");
    assert_eq!(eval("return string.match('THE (quick) fox', '%f[%a]%a+')"), "\"THE\"");
    assert_eq!(eval("return string.match('abcabc', '(abc)%1')"), "\"abc\"");
    assert_eq!(
        eval("return string.match('x = 10', '^(%a+)%s*=%s*(%d+)$')"),
        "\"x\", \"10\""
    );
    assert_eq!(eval("return string.match('hello', '[^aeiou]+')"), "\"h\"");
    assert_eq!(eval("return string.match('f(a,b)', '%((.-)%)')"), "\"a,b\"");
    assert_eq!(eval("return string.find('abc', '', 10)"), "nil");
    assert_eq!(eval("return string.match('abc', 'b', -1)"), "nil");
    assert!(error_of("return string.match('x', '[a')").contains("missing ']'"));
    assert!(error_of("return string.match('x', '%')").contains("ends with '%'"));
}

#[test]
fn patterns_gmatch_and_gsub() {
    let out = run("
        local words = {}
        for w in string.gmatch('one two  three', '%a+') do words[#words + 1] = w end
        print(table.concat(words, '|'))
        for k, v in string.gmatch('a=1, b=2', '(%w+)=(%w+)') do print(k, v) end
        print(string.gsub('hello world', 'o', '0'))
        print(string.gsub('hello world', '(%w+)', '<%1>'))
        print(string.gsub('abc', '%w', '%0%0', 2))
        print(string.gsub('$name is $age', '%$(%w+)', {name = 'Ann', age = 30}))
        print(string.gsub('1 2 3', '%d', function(d) return d * 2 end))
        print(string.gsub('abc', '', '-'))
        print(('x'):gsub('x', '%%'))
    ");
    assert_eq!(
        out,
        vec![
            "one|two|three",
            "a\t1",
            "b\t2",
            "hell0 w0rld\t2",
            "<hello> <world>\t2",
            "aabbc\t2",
            "Ann is 30\t2",
            "2 4 6\t3",
            "-a-b-c-\t4",
            "%\t1",
        ]
    );
}

#[test]
fn math_library() {
    assert_eq!(
        eval("return math.floor(3.7), math.ceil(3.2), math.abs(-2), math.max(1, 5, 3), math.min(4, 2)"),
        "3, 4, 2, 5, 2"
    );
    assert_eq!(
        eval("return math.sqrt(16), math.pi > 3.14, math.huge, math.fmod(7, 3), math.fmod(-7, 3)"),
        "4, true, inf, 1, -1"
    );
    assert_eq!(eval("return math.modf(3.7)"), "3, 0.7");
    assert_eq!(
        eval("return math.tointeger(3.0), math.tointeger(3.5), math.type(1), math.type(1.5), math.type('1')"),
        "3, nil, \"integer\", \"float\", nil"
    );
    assert_eq!(
        eval("return math.log(8, 2), math.log(100, 10), math.clamp(15, 0, 10)"),
        "3, 2, 10"
    );
    let out = run("
        math.randomseed(42)
        local ok = true
        for i = 1, 1000 do
            local r = math.random(1, 6)
            if r < 1 or r > 6 or r % 1 ~= 0 then ok = false end
            local f = math.random()
            if f < 0 or f >= 1 then ok = false end
        end
        print(ok)
        math.randomseed(7) local a = math.random(1000)
        math.randomseed(7) local b = math.random(1000)
        print(a == b)
    ");
    assert_eq!(out, vec!["true", "true"]);
    assert!(error_of("return math.random(5, 1)").contains("interval is empty"));
}

#[test]
fn table_library() {
    let out = run("
        local t = {3, 1, 2}
        table.sort(t)
        print(table.concat(t, ','))
        table.sort(t, function(a, b) return a > b end)
        print(table.concat(t, ','))
        table.insert(t, 4)
        table.insert(t, 1, 0)
        print(table.concat(t, ','))
        print(table.remove(t), table.remove(t, 1), table.concat(t, ','))
        local words = {'pear', 'apple', 'fig'}
        table.sort(words)
        print(table.concat(words, ' '))
        local packed = table.pack(1, nil, 3)
        print(packed.n, table.unpack({1, 2, 3}))
        local people = {{n='b', a=2}, {n='a', a=2}, {n='c', a=1}}
        table.sort(people, function(x, y) return x.a < y.a end)
        print(people[1].n, people[2].n, people[3].n)
        print(table.concat({}, ','), table.concat({1, 2, 3}, ', ', 2, 3))
        local e = {}
        print(table.remove(e), #e)
    ");
    assert_eq!(
        out,
        vec![
            "1,2,3",
            "3,2,1",
            "0,3,2,1,4",
            "4\t0\t3,2,1",
            "apple fig pear",
            "3\t1\t2\t3",
            "c\tb\ta",
            "\t2, 3",
            "nil\t0"
        ]
    );
    assert!(error_of("string.gsub('hello', 'l', nil)").contains("string/function/table expected"));
    assert!(error_of("table.sort({1, 'a', 2})").contains("attempt to compare"));
    assert!(error_of("table.concat({1, {}, 3})").contains("invalid value (at index 2)"));
    // An inconsistent comparator must not crash.
    run("local t = {} for i = 1, 100 do t[i] = i end table.sort(t, function() return true end)");
}

#[test]
fn base_library() {
    assert_eq!(
        eval("return type(nil), type(1), type('a'), type({}), type(print), type(true)"),
        "\"nil\", \"number\", \"string\", \"table\", \"function\", \"boolean\""
    );
    assert_eq!(
        eval(
            "return tonumber('42'), tonumber(' 0x1F '), tonumber('1e2'), tonumber('abc'), tonumber('ff', 16), tonumber('z', 36), tonumber('8', 8)"
        ),
        "42, 31, 100, nil, 255, 35, nil"
    );
    assert_eq!(
        eval("return tostring(12), tostring(nil), tostring(1.5)"),
        "\"12\", \"nil\", \"1.5\""
    );
    assert_eq!(
        eval("return rawequal({}, {}), rawlen({1, 2}), rawlen('abc')"),
        "false, 2, 3"
    );
    assert_eq!(
        eval("local t = setmetatable({}, {__metatable = 'locked'}) return getmetatable(t)"),
        "\"locked\""
    );
    assert!(error_of("local t = setmetatable({}, {__metatable = 1}) setmetatable(t, {})").contains("protected"));
    assert_eq!(eval("local f = load('return 1 + ...') return f(41)"), "42");
    assert_eq!(eval("x = 5 local f = load('return x') return f()"), "5");
    assert_eq!(
        eval("local f, err = load('return +') return f, err"),
        "nil, \"load:1: unexpected symbol near '+'\""
    );
    assert_eq!(
        eval("local t = {} for i, v in ipairs({'a', 'b', nil, 'd'}) do t[#t+1] = i end return #t"),
        "2"
    );
    assert_eq!(eval("return assert(1, 'm')"), "1, \"m\"");
    assert_eq!(error_of("assert(false, 'nope')"), "nope");
    assert_eq!(error_of("assert(nil)"), "test.lua:1: assertion failed!");
    assert_eq!(eval("return _G.print == print, _VERSION ~= nil"), "true, true");
    assert!(error_of("next({}, 'missing')").contains("invalid key to 'next'"));
}

#[test]
fn inspect_pretty_prints() {
    assert_eq!(eval("return {1, 2, 3}"), "{ 1, 2, 3 }");
    assert_eq!(eval("return {a = 1, ['b c'] = true}"), "{ a = 1, [\"b c\"] = true }");
    assert_eq!(eval("return {}"), "{}");
    assert_eq!(eval("local t = {} t.self = t return t"), "{\n  self = <cycle>,\n}");
    assert_eq!(eval("return {pos = {x = 1, y = 2}}"), "{\n  pos = { x = 1, y = 2 },\n}");
}

#[test]
fn os_library() {
    assert_eq!(
        eval("return os.date('!%Y-%m-%d %H:%M:%S', 0)"),
        "\"1970-01-01 00:00:00\""
    );
    assert_eq!(
        eval("return os.date('%A %B %j', 1700000000)"),
        "\"Tuesday November 318\""
    );
    assert_eq!(
        eval("local t = os.date('*t', 86400 * 365) return t.year, t.month, t.day, t.wday"),
        "1971, 1, 1, 6"
    );
    assert_eq!(
        eval("return os.time({year = 2000, month = 1, day = 1, hour = 0})"),
        "946684800"
    );
    assert_eq!(eval("return os.clock() >= 0, os.time() > 1700000000"), "true, true");
}

#[test]
fn globals_are_per_environment() {
    let mut lua = Lua::new();
    let a = new_env(&mut lua);
    let b = new_env(&mut lua);
    let mut host = NoHost::default();
    let f = lua.load("shared = 'from a'", "a.lua", &a).unwrap();
    lua.call(&mut host, &f, Vec::new()).ok().unwrap();
    let g = lua.load("return shared", "b.lua", &b).unwrap();
    let r = lua.call(&mut host, &g, Vec::new()).ok().unwrap();
    assert!(r[0].is_nil());
    // A function made in `a` keeps `a`'s globals when `b` calls it.
    let f = lua.load("return function() return shared end", "a.lua", &a).unwrap();
    let getter = lua.call(&mut host, &f, Vec::new()).ok().unwrap().remove(0);
    let r = lua.call(&mut host, &getter, Vec::new()).ok().unwrap();
    assert_eq!(r[0].as_text().as_deref(), Some("from a"));
}

#[test]
fn traceback_lists_the_calls() {
    let mut lua = Lua::new();
    let env = new_env(&mut lua);
    let src = "local function inner() error('deep') end\nlocal function outer() inner() end\nouter()";
    let f = lua.load(src, "t.lua", &env).unwrap();
    let mut host = NoHost::default();
    let Err(e) = lua.call(&mut host, &f, Vec::new()) else {
        panic!("should fail");
    };
    assert_eq!(e.message(), "t.lua:1: deep");
    assert_eq!(
        e.traceback(),
        "t.lua:1: in function 'inner'\nt.lua:2: in function 'outer'\nt.lua:3: in main chunk"
    );
}

#[test]
fn a_longer_program() {
    let out = run(r#"
        -- A tiny inventory with JSON-ish output.
        local Inventory = {}
        Inventory.__index = Inventory
        function Inventory.new() return setmetatable({items = {}, count = 0}, Inventory) end
        function Inventory:add(name, n)
            self.items[name] = (self.items[name] or 0) + (n or 1)
            self.count = self.count + (n or 1)
            return self
        end
        function Inventory:sorted()
            local names = {}
            for name in pairs(self.items) do names[#names + 1] = name end
            table.sort(names)
            local parts = {}
            for _, name in ipairs(names) do
                parts[#parts + 1] = string.format("%s=%d", name, self.items[name])
            end
            return "{" .. table.concat(parts, ", ") .. "}"
        end
        local inv = Inventory.new():add("stone", 32):add("dirt"):add("stone", 32)
        print(inv:sorted(), inv.count)
        local primes, n = {}, 2
        while #primes < 10 do
            local prime = true
            for _, p in ipairs(primes) do if n % p == 0 then prime = false break end end
            if prime then primes[#primes + 1] = n end
            n = n + 1
        end
        print(table.concat(primes, " "))
    "#);
    assert_eq!(out, vec!["{dirt=1, stone=64}\t65", "2 3 5 7 11 13 17 19 23 29"]);
}

#[test]
fn values_round_trip_through_tables() {
    assert_eq!(
        eval("local t = {true, false, 'x', 1.5} return t[1], t[2], t[3], t[4]"),
        "true, false, \"x\", 1.5"
    );
    let mut lua = Lua::new();
    let env = new_env(&mut lua);
    env.borrow_mut().set_str("answer", Value::Num(42.0));
    let f = lua.load("return answer * 2", "x.lua", &env).unwrap();
    let r = lua.call(&mut NoHost::default(), &f, Vec::new()).ok().unwrap();
    assert_eq!(r[0].as_num(), Some(84.0));
}
