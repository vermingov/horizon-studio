// Luau on the studio's interpreter: its syntax (types, compound assignment,
// continue, interpolated strings, if-expressions, number literals) and its
// standard library additions, script by script.

use horizon_studio::game::hstudio_lua::{Lua, NoHost};
use horizon_studio::game::hstudio_lualib::new_env;

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

/// What `src` printed, one line each.
fn run(src: &str) -> Vec<String> {
    try_run(src).unwrap_or_else(|e| panic!("script failed: {e}\n--- script ---\n{src}"))
}

/// What `src` printed, joined with spaces.
fn out(src: &str) -> String {
    run(src).join(" ")
}

fn error_of(src: &str) -> String {
    try_run(src).expect_err("the script should fail")
}

// ----------------------------------------------------------------- types

#[test]
fn type_annotations_are_read_and_dropped() {
    let src = r##"
        type Point = { x: number, y: number }
        export type Pair<T> = { first: T, second: T? }
        type Callback = (number, string?) -> ()
        type Union = "a" | "b" | nil
        type Map<K, V> = { [K]: V }
        type Nested = Map<string, Map<string, {number}>>
        type Fn = <T>(T, ...any) -> (T, ...number)
        type Packs<T...> = (T...) -> T...
        type Props = { read x: number, write y: string }

        local p: Point = { x = 1, y = 2 }
        local a: number, b: string = 3, "four"
        local list: {number} = {1, 2, 3}
        local nested: Map<string, Map<string, number>>= {}
        local function add<T>(x: number, y: number): number
            return x + y
        end
        local function many(...: number): (number, number)
            return select("#", ...), (...)
        end
        local obj = {}
        function obj.method(self: any, n: number?): string
            return tostring(n)
        end
        local f = function(x: number): number return x * 2 end
        for i: number = 1, 2 do a += i end
        for k: string, v: number in pairs({ q = 1 }) do b ..= k .. v end
        local cast = (p :: any).x :: number
        print(p.x + p.y, a, b, #list, add(2, 3), many(7, 8), obj:method(5), f(4), cast)
        print(type(nested), typeof(1))
    "##;
    assert_eq!(
        run(src),
        ["3 6 fourq1 3 5 2 5 8 1", "table number"].map(|l| l.replace(' ', "\t"))
    );
}

#[test]
fn type_is_still_a_function_name() {
    assert_eq!(
        out("local type = type\nprint(type(1))\nprint(type 'x')"),
        "number string"
    );
}

// ----------------------------------------------------------------- compound assignment

#[test]
fn compound_assignment() {
    let src = r#"
        local n = 10
        n += 5; n -= 3; n *= 2; n /= 4; n //= 1; n %= 4; n ^= 3
        local s = "a"; s ..= "b"; s ..= 1
        g = 1; g += 1
        local t = { x = 1, list = { 5 } }
        t.x += 10; t.list[1] *= 3
        local up = 0
        local function bump() up += 1 end
        bump(); bump()
        print(n, s, g, t.x, t.list[1], up)
    "#;
    assert_eq!(out(src), "8\tab1\t2\t11\t15\t2");
}

#[test]
fn compound_assignment_evaluates_its_target_once() {
    let src = r#"
        local calls = 0
        local t = { 1, 2, 3 }
        local function key() calls += 1; return 2 end
        local function tab() calls += 10; return t end
        tab()[key()] += 100
        print(t[2], calls)
    "#;
    assert_eq!(out(src), "102\t11");
}

#[test]
fn compound_assignment_uses_metamethods() {
    let src = r#"
        local v = setmetatable({ n = 1 }, { __add = function(a, b) return setmetatable({ n = a.n + b }, getmetatable(a)) end })
        v += 41
        print(v.n)
    "#;
    assert_eq!(out(src), "42");
}

// ----------------------------------------------------------------- continue

#[test]
fn continue_in_every_loop() {
    let src = r#"
        local r = {}
        for i = 1, 6 do
            if i % 2 == 0 then continue end
            table.insert(r, i)
        end
        for _, v in ipairs({ "a", "skip", "b" }) do
            if v == "skip" then continue end
            table.insert(r, v)
        end
        local i = 0
        while i < 5 do
            i += 1
            if i == 3 then continue end
            table.insert(r, "w" .. i)
        end
        local j = 0
        repeat
            j += 1
            if j == 2 then continue end
            table.insert(r, "r" .. j)
        until j >= 3
        print(table.concat(r, ","))
    "#;
    assert_eq!(out(src), "1,3,5,a,b,w1,w2,w4,w5,r1,r3");
}

#[test]
fn continue_is_still_a_name() {
    let src =
        "local continue = 5\ncontinue = continue + 1\nprint(continue)\nlocal t = { continue = 2 }\nprint(t.continue)";
    assert_eq!(out(src), "6 2");
    assert!(error_of("continue").contains("continue outside a loop"));
}

#[test]
fn continue_with_closures_gives_each_turn_its_own_local() {
    let src = r#"
        local fs = {}
        for i = 1, 4 do
            local x = i * 10
            fs[#fs + 1] = function() return x end
            if i == 2 then continue end
        end
        print(fs[1](), fs[2](), fs[3](), fs[4]())
    "#;
    assert_eq!(out(src), "10\t20\t30\t40");
}

// ----------------------------------------------------------------- strings

#[test]
fn interpolated_strings() {
    let src = r#"
        local name, n = "Steve", 3
        print(`hello {name}, you have {n * 2} items`)
        print(`{n}`, `plain`, `{1}{2}`, `a{ ({1, 2})[2] }b`)
        local v = setmetatable({}, { __tostring = function() return "V!" end })
        print(`{v} and {nil} and {true}`)
        print(`nested {`inner {name}`} done`)
        print(`escapes \{ \` \n{"x"}`)
        print(`{(function() return 1, 2 end)()}`)
        print((`x`):upper())
    "#;
    assert_eq!(
        run(src),
        [
            "hello Steve, you have 6 items",
            "3\tplain\t12\ta2b",
            "V! and nil and true",
            "nested inner Steve done",
            "escapes { ` \nx",
            "1",
            "X",
        ]
    );
}

#[test]
fn interpolated_string_errors() {
    assert!(error_of("print(`a {1}").contains("unfinished"));
    assert!(error_of("print(`a {{1}}`)").contains("double braces"));
}

// ----------------------------------------------------------------- if-expressions

#[test]
fn if_expressions() {
    let src = r#"
        local function grade(n)
            return if n >= 90 then "A" elseif n >= 80 then "B" else "C"
        end
        local x = if false then nil else false
        local t = { v = if 1 > 2 then "no" else "yes" }
        local calls = 0
        local function side() calls += 1; return calls end
        local y = if true then side() else side()
        print(grade(95), grade(85), grade(10), x, t.v, y, calls, (if nil then 1 else 2) + 1)
    "#;
    assert_eq!(out(src), "A\tB\tC\tfalse\tyes\t1\t1\t3");
    assert!(error_of("local x = if true then 1").contains("else"));
}

// ----------------------------------------------------------------- numbers and attributes

#[test]
fn number_literals() {
    assert_eq!(
        out("print(1_000_000, 0b1010, 0B11, 0xFF_FF, 1_0.5_0)"),
        "1000000\t10\t3\t65535\t10.5"
    );
    assert!(error_of("print(0b102)").contains("malformed number"));
}

#[test]
fn attributes_are_accepted() {
    assert_eq!(out("@native\nlocal function f() return 1 end\nprint(f())"), "1");
}

// ----------------------------------------------------------------- the standard library

#[test]
fn luau_table_functions() {
    let src = r#"
        local t = { "a", "b", "c", "b" }
        print(table.find(t, "b"), table.find(t, "b", 3), table.find(t, "z"))
        local c = table.create(3, 0)
        print(#c, c[1], c[3], #table.create(2))
        local mt = { __index = function() return "dflt" end }
        local src = setmetatable({ 1, 2, k = "v" }, mt)
        local copy = table.clone(src)
        copy[1] = 9
        print(src[1], copy[1], copy.k, copy.missing, getmetatable(copy) == mt)
        table.clear(copy)
        print(#copy, copy.k, getmetatable(copy) == mt)
        local f = table.freeze({ x = 1 })
        print(table.isfrozen(f), table.isfrozen({}), pcall(function() f.x = 2 end))
        print(table.maxn({ [1] = 1, [7] = 2, x = 3 }), table.getn({ 1, 2 }))
    "#;
    assert_eq!(
        run(src),
        [
            "2\t4\tnil",
            "3\t0\t0\t0",
            "1\t9\tv\tdflt\ttrue",
            "0\tdflt\ttrue",
            "true\tfalse\tfalse\ttest.lua:14: attempt to modify a readonly table",
            "7\t2",
        ]
    );
}

#[test]
fn luau_math_functions() {
    let src = r#"
        print(math.sign(-3), math.sign(0), math.sign(2), math.round(2.5), math.round(-2.5), math.round(1.4))
        print(math.lerp(0, 10, 0.25), math.map(5, 0, 10, 0, 100), math.clamp(12, 0, 10))
        print(math.noise(1, 2, 3), math.noise(0.5, 0.3) ~= 0, math.abs(math.noise(0.3, 0.7)) <= 1)
        print(math.isnan(0/0), math.isinf(math.huge), math.isfinite(1))
    "#;
    assert_eq!(
        run(src),
        ["-1\t0\t1\t3\t-3\t1", "2.5\t50\t10", "0\ttrue\ttrue", "true\ttrue\ttrue"]
    );
}

#[test]
fn bit32() {
    let src = r#"
        print(bit32.band(0xFF, 0x0F), bit32.bor(1, 2, 4), bit32.bxor(5, 1), bit32.bnot(0))
        print(bit32.lshift(1, 4), bit32.rshift(256, 4), bit32.arshift(-16, 2), bit32.lshift(1, 32))
        print(bit32.lrotate(0x80000000, 1), bit32.rrotate(1, 1), bit32.btest(2, 3), bit32.btest(4, 3))
        print(bit32.extract(0xABCD, 4, 8), bit32.replace(0, 0xF, 8, 4), bit32.countlz(1), bit32.countrz(8))
        print(bit32.byteswap(0x01020304), bit32.band(-1))
    "#;
    assert_eq!(
        run(src),
        [
            "15\t7\t4\t4294967295",
            "16\t16\t4294967292\t0",
            "1\t2147483648\ttrue\tfalse",
            "188\t3840\t31\t3",
            "67305985\t4294967295",
        ]
    );
}

#[test]
fn typeof_names_types() {
    assert_eq!(
        out("print(typeof(nil), typeof(1), typeof('s'), typeof({}), typeof(print), typeof(coroutine.create(print)))"),
        "nil\tnumber\tstring\ttable\tfunction\tthread"
    );
}
