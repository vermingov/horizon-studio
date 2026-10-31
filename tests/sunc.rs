// The executor library against sUNC's documented behaviour: each test runs
// the docs' examples (docs.sunc.io) as a mod on the fake game and checks
// what they print.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};

/// Runs `src` as a mod for `seconds`; what it printed.
fn run_mod(src: &str, seconds: f64) -> (Studio, FakeGame, Vec<String>) {
    let mut game = FakeGame::new("sunc");
    let mut st = studio(&mut game);
    game.write_mod("t.lua", src);
    run_for(&mut st, &mut game, seconds);
    let i = mod_index(&st, "t");
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("the mod failed: {msg}\n{trace}\n{:?}", log(&st));
    }
    let p = log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect();
    (st, game, p)
}

fn printed(src: &str) -> Vec<String> {
    let (st, _, p) = run_mod(src, 0.6);
    if p.is_empty() {
        eprintln!(
            "{:#?} rbx active {} http {}",
            log(&st),
            st.s.rbx.active,
            st.s.rbx.http.len()
        );
    }
    p
}

#[test]
fn closures() {
    let p = printed(
        r##"
        local function dummy_func() return "not hooked" end
        local function dummy_hook() return "hooked" end
        print(dummy_func())
        local old = hookfunction(dummy_func, dummy_hook)
        print(dummy_func(), old())
        restorefunction(dummy_func)
        print(dummy_func(), pcall(restorefunction, dummy_func))

        -- Hooking a native with a Lua function and back.
        local real_tostring = tostring
        local old_ts
        old_ts = hookfunction(tostring, function(v) return "hooked " .. old_ts(v) end)
        print(tostring(5))
        restorefunction(tostring)
        print(tostring(5), real_tostring == tostring)

        local wrapped = newcclosure(function(...) return ... end)
        print(iscclosure(wrapped), islclosure(wrapped), wrapped("Hello"))
        print(iscclosure(print), iscclosure(getgc), iscclosure(dummy_func), islclosure(dummy_func))
        print(isexecutorclosure(dummy_func), isexecutorclosure(wrapped), isexecutorclosure(getgc), isexecutorclosure(print))

        local function dummy_function() print("Hello") end
        local cloned = clonefunction(dummy_function)
        print(debug.info(cloned, "l") > 0, debug.info(cloned, "n"), cloned == dummy_function, getfenv(cloned) == getfenv(dummy_function))
        hookfunction(dummy_function, function() return "x" end)
        print(cloned == dummy_function, select("#", cloned()))

        local function is_sha384_hex(h) return #h == 96 and h:match("^[0-9a-fA-F]+$") ~= nil end
        local f0 = function() end
        local f1 = function(...) end
        local f2 = function() end
        local f3 = function() return "Constant" end
        local f4 = function() return "Constant2" end
        print(is_sha384_hex(getfunctionhash(f0)), getfunctionhash(f0) == getfunctionhash(f1), getfunctionhash(f0) == getfunctionhash(f2), getfunctionhash(f3) == getfunctionhash(f4))
        print(pcall(getfunctionhash, print))

        loadstring([[ placeholder = {"Example"} ]])()
        print(placeholder[1])
        local func, err = loadstring("Example = ", "CustomChunk")
        print(func, err:sub(1, 22))
        print(checkcaller())

        -- newcclosure's functions can wait.
        task.spawn(newcclosure(function()
            print("Before")
            task.wait(0.1)
            print("After")
        end))
    "##,
    );
    let want = [
        "not hooked",
        "hooked\tnot hooked",
        "not hooked\tfalse\tt.lua:8: restorefunction: function is not hooked",
        "hooked 5",
        "5\ttrue",
        "true\tfalse\tHello",
        "true\ttrue\tfalse\ttrue",
        "true\ttrue\ttrue\tfalse",
        "true\tdummy_function\tfalse\ttrue",
        "Hello",
        "false\t0",
        "true\tfalse\ttrue\tfalse",
    ];
    for (i, w) in want.iter().enumerate() {
        assert_eq!(p.get(i).map(String::as_str), Some(*w), "line {i}: {p:#?}");
    }
    assert!(
        p[12].starts_with("false\t") && p[12].contains("C closures"),
        "{}",
        p[12]
    );
    assert_eq!(p[13], "Example");
    assert_eq!(p[14], "nil\t[string \"CustomChunk\"]");
    assert_eq!(p[15], "true");
    assert_eq!(&p[16..], ["Before", "After"]);
}

#[test]
fn metamethods_and_namecall() {
    let p = printed(
        r##"
        local mt = getrawmetatable(game)
        print(type(mt), mt.__index(game, "Workspace") == workspace, isreadonly(mt), isreadonly({}))
        print(getrawmetatable(workspace) == mt, getmetatable(game))
        print(pcall(function() mt.Example = "Hello" end))
        setreadonly(mt, false)
        mt.Example = "Hello"
        print(mt.Example)
        mt.Example = nil
        setreadonly(mt, true)

        local keys = {}
        local original; original = hookmetamethod(game, "__index", function(...)
            local key = select(2, ...)
            keys[#keys + 1] = key
            return original(...)
        end)
        local _ = game.PlaceId
        hookmetamethod(game, "__index", original)
        print(keys[1])

        local methods = {}
        local nc; nc = hookmetamethod(game, "__namecall", function(...)
            local self = ...
            local method = getnamecallmethod()
            if self == game and method == "service" then error("Using game:service() is not allowed.") end
            methods[#methods + 1] = method
            return nc(...)
        end)
        print(game:GetService("Players") == game.Players, pcall(function() return game:service("x") end))
        print(workspace:FindFirstChild("Terrain") ~= nil)
        hookmetamethod(game, "__namecall", nc)
        print(methods[1], methods[2])
        print(getrawmetatable(newproxy(false)), typeof(newproxy(true)))
        local t = setrawmetatable({}, { __index = { x = 1 } })
        print(t.x)
    "##,
    );
    assert_eq!(p[0], "table\ttrue\ttrue\tfalse");
    assert_eq!(p[1], "true\tThe metatable is locked");
    assert!(p[2].starts_with("false\t") && p[2].contains("readonly"), "{}", p[2]);
    assert_eq!(p[3], "Hello");
    assert_eq!(p[4], "PlaceId");
    assert!(
        p[5].starts_with("true\tfalse\t") && p[5].contains("game:service() is not allowed"),
        "{}",
        p[5]
    );
    assert_eq!(p[6], "true");
    assert_eq!(p[7], "GetService\tFindFirstChild");
    assert_eq!(p[8], "nil\tuserdata");
    assert_eq!(p[9], "1");
}

#[test]
fn debug_library() {
    let p = printed(
        r##"
        local function dummy_function()
            local dummy_string = "foo bar"
            string.split(dummy_string, " ")
        end
        local has = {}
        for _, c in debug.getconstants(dummy_function) do has[c] = true end
        print(has.split, has["foo bar"], pcall(debug.getconstants, print))

        local function name_only() print(game.Name) end
        local idx
        for i, c in debug.getconstants(name_only) do if c == "Name" then idx = i end end
        debug.setconstant(name_only, idx, "Players")
        local seen
        local old = hookfunction(print, function(v) seen = tostring(v) end)
        name_only()
        restorefunction(print)
        print(seen)

        local function outer()
            local function proto_1() return "one" end
            local function proto_2() return "two" end
        end
        local names = {}
        for i, proto in debug.getprotos(outer) do names[i] = debug.info(proto, "n") end
        print(names[1], names[2], pcall(debug.getproto(outer, 1)))

        local function maker()
            local function dummy_proto() return "hi" end
            return dummy_proto
        end
        local real = maker()
        local live = debug.getproto(maker, 1, true)[1]
        print(real == live, live())

        local UpFunction = function() return "Hello from up" end
        local function Dummy() return UpFunction() end
        print(debug.getupvalue(Dummy, 1)(), pcall(debug.getupvalue, Dummy, 0), pcall(debug.getupvalue, print, 1))
        local var1, var2 = false, "Hi"
        local function ups() var1 = true; var2 ..= ", hello" end
        local u = debug.getupvalues(ups)
        print(u[1], u[2], next(debug.getupvalues(function() return 123 end)))
        local upvalue = 90
        local function bump() upvalue += 1; return upvalue end
        print(bump())
        debug.setupvalue(bump, 1, 99)
        print(bump())

        local value = 5
        value += 1
        local function inner()
            print(debug.getstack(2, 1) ~= nil, debug.getstack(1, 1))
        end
        local function stacked(a) inner() return a end
        stacked("arg")
        local function scope()
            local outer_value = 10
            local function inner_function()
                outer_value += 9
                debug.setstack(2, 1, 100)
            end
            inner_function()
            return outer_value
        end
        print(scope())
        print(debug.info(1, "l") > 0, debug.info(print, "s"), select("#", debug.info(stacked, "a")), debug.info(stacked, "a"))
        print(type(debug.traceback("msg")), debug.traceback("msg"):sub(1, 3))
    "##,
    );
    assert!(p[0].starts_with("true\ttrue\tfalse\t"), "{}", p[0]);
    assert_eq!(p[1], "Players");
    assert!(
        p[2].starts_with("proto_1\tproto_2\tfalse\t") && p[2].contains("inactive proto"),
        "{}",
        p[2]
    );
    assert_eq!(p[3], "true\thi");
    // The middle pcall keeps only its first result (false) in the list.
    assert!(
        p[4].starts_with("Hello from up\tfalse\tfalse\t") && p[4].contains("C closures"),
        "{}",
        p[4]
    );
    assert_eq!(p[5], "false\tHi\tnil");
    assert_eq!(p[6], "91");
    assert_eq!(p[7], "100");
    // A function without locals has the function it is calling in slot 1, as Luau's does.
    assert!(p[8].starts_with("true\tfunction"), "{}", p[8]);
    assert_eq!(p[9], "100");
    assert_eq!(p[10], "true\t[C]\t2\t1\tfalse");
    assert_eq!(p[11], "string\tmsg");
}

#[test]
fn environments() {
    let p = printed(
        r##"
        getgenv().dummy_val = "value"
        getfenv().dummy_val_2 = 1
        print(dummy_val, getgenv().dummy_val_2)
        getgenv().dummy_val = "value2"
        dummy_val = nil
        print(dummy_val)
        local saved = getrenv().warn
        getrenv().warn = "Hello!"
        print(type(warn))
        getrenv().warn = saved
        print(type(warn), getrenv().game == game, getgenv().getgenv == getgenv)
        local dummy_table = {}
        local function dummy_function() end
        local found_f, found_t = false, false
        for _, v in getgc() do
            if v == dummy_function then found_f = true end
            if v == dummy_table then found_t = true end
        end
        print(found_f, found_t)
        for _, v in getgc(true) do if v == dummy_table then found_t = true end end
        print(found_t)
        local retrieved = filtergc("function", { Name = "dummy_function", IgnoreExecutor = false })
        print(typeof(retrieved), retrieved[1] == dummy_function)
        print(filtergc("function", { Name = "dummy_function", IgnoreExecutor = false }, true) == dummy_function)
        local up = 5
        local function target() up += 1; print(game.Players.LocalPlayer) end
        print(filtergc("function", { Constants = { "game", "Players", "LocalPlayer" }, Upvalues = { 5 }, IgnoreExecutor = false }, true) == target)
        print(filtergc("function", { Hash = getfunctionhash(target), IgnoreExecutor = false }, true) == target)
        local kt = { dummy_key = "dummy_value" }
        print(filtergc("table", { Keys = { "dummy_key" } }, true) == kt, filtergc("table", { KeyValuePairs = { dummy_key = "dummy_value" } }, true) == kt)
        local mt_t = setmetatable({}, { __index = getgenv() })
        print(filtergc("table", { Metatable = getmetatable(mt_t) }, true) == mt_t)
        local loop = task.spawn(function() while task.wait(1) do end end)
        local in_reg = false
        for _, v in getreg() do if v == loop then in_reg = true end end
        print(in_reg)
        print(identifyexecutor())
    "##,
    );
    assert_eq!(p[0], "value\tnil");
    assert_eq!(p[1], "value2");
    assert_eq!(p[2], "string");
    assert_eq!(p[3], "function\ttrue\ttrue");
    assert_eq!(p[4], "true\tfalse");
    assert_eq!(p[5], "true");
    assert_eq!(p[6], "table\ttrue");
    assert_eq!(p[7], "true");
    assert_eq!(p[8], "true");
    assert_eq!(p[9], "true");
    assert_eq!(p[10], "true\ttrue");
    assert_eq!(p[11], "true");
    assert_eq!(p[12], "true");
    assert_eq!(p[13], "Horizon Studio\t0.4.0");
}

#[test]
fn encoding() {
    let p = printed(
        r##"
        print(base64encode("DummyString\0\2"), base64decode("RHVtbXlTdHJpbmcAAg==") == "DummyString\0\2")
        local text = "Hello, world! Hello, world! Goodbye, world!"
        local c = lz4compress(text)
        print(#c < #text, lz4decompress(c) == text, lz4decompress(c, #text) == text)
        print(crypt)
    "##,
    );
    assert_eq!(p, ["RHVtbXlTdHJpbmcAAg==\ttrue", "true\ttrue\ttrue", "nil"]);
}

#[test]
fn filesystem() {
    let (st, _game, p) = run_mod(
        r##"
        writefile("file.txt", "Hello world")
        print(readfile("file.txt"))
        writefile("file4.txt", "print(")
        appendfile("file4.txt", "'Hello')")
        print(readfile("file4.txt"))
        print(isfile("nonexistent.txt"), isfile("file.txt"), isfolder("file.txt"))
        makefolder("folder2")
        print(isfolder("folder2"))
        writefile("folder2/inner.lua", "return 10 + ...")
        print(loadfile("folder2/inner.lua")(1), dofile and true)
        local seen = {}
        for _, f in listfiles("") do seen[f] = true end
        print(seen["file.txt"], seen["folder2"], listfiles("folder2")[1])
        delfile("file.txt")
        delfolder("folder2")
        print(isfile("file.txt"), isfolder("folder2"))
        print(pcall(readfile, "missing.txt"))
        print(pcall(writefile, "evil.exe", "x"))
        print(pcall(writefile, "../escape.txt", "x"))
        print(pcall(readfile, "/etc/passwd"))
        writefile("broken.lua", "retrn 10")
        print(pcall(loadfile, "broken.lua"))
        writefile("pic.png", "x")
        print(getcustomasset("pic.png"))
    "##,
        0.6,
    );
    assert_eq!(p[0], "Hello world");
    assert_eq!(p[1], "print('Hello')");
    assert_eq!(p[2], "false\ttrue\tfalse");
    assert_eq!(p[3], "true");
    assert_eq!(p[4], "11\ttrue");
    assert_eq!(p[5], "true\ttrue\tfolder2/inner.lua");
    assert_eq!(p[6], "false\tfalse");
    assert!(
        p[7].starts_with("false\t") && p[7].contains("does not exist"),
        "{}",
        p[7]
    );
    assert!(
        p[8].starts_with("false\t") && p[8].contains(".exe files can't be written"),
        "{}",
        p[8]
    );
    assert!(
        p[9].starts_with("false\t") && p[9].contains("outside the workspace"),
        "{}",
        p[9]
    );
    assert!(
        p[10].starts_with("false\t") && p[10].contains("outside the workspace"),
        "{}",
        p[10]
    );
    assert!(p[11].starts_with("false\t"), "{}", p[11]);
    assert_eq!(p[12], "rbxasset://horizon-studio/pic.png");
    // Everything stayed in the studio's workspace folder.
    assert!(st.s.dir.join("workspace").join("file4.txt").is_file());
    assert!(!st.s.dir.join("escape.txt").exists());
}

#[test]
fn instances_and_signals() {
    let p = printed(
        r##"
        local original = game.Players.LocalPlayer
        local clone = cloneref(original)
        print(original == clone, clone.Name, compareinstances(original, clone), compareinstances(game, workspace))
        print(compareinstances(game, cloneref(game)), game == cloneref(game))

        local hui = gethui()
        local gui = Instance.new("ScreenGui")
        gui.Name = "GUI"
        gui.Parent = hui
        print(hui:FindFirstChild("GUI") == gui, game:FindFirstChild("HiddenUI", true))

        local part = Instance.new("Part")
        local found_all, found_nil = false, false
        for _, i in getinstances() do if i == part then found_all = true end end
        for _, i in getnilinstances() do if i == part then found_nil = true end end
        print(found_all, found_nil)

        local cd = Instance.new("ClickDetector")
        cd.MouseClick:Connect(function(player) print(player.Name .. " Fired M1") end)
        cd.RightMouseClick:Connect(function(player) print(player.Name .. " Fired M2") end)
        fireclickdetector(cd, 0, "MouseClick")
        fireclickdetector(cd, 0, "RightMouseClick")
        local prompt = Instance.new("ProximityPrompt", part)
        prompt.Triggered:Connect(function(player) print(player.Name .. " triggered the prompt") end)
        fireproximityprompt(prompt)
        part.Touched:Connect(function(other) print(other.Name .. " touched") end)
        firetouchinterest(game.Players.LocalPlayer.Character.Head, part, true)
        task.wait()

        local bindable = Instance.new("BindableFunction")
        bindable.OnInvoke = function() return "from callback" end
        print(getcallbackvalue(bindable, "OnInvoke")(), getcallbackvalue(Instance.new("RemoteFunction"), "OnClientInvoke"))

        local folder = Instance.new("Folder")
        folder.ChildAdded:Connect(function(arg) print("Triggered", typeof(arg)) end)
        local c = getconnections(folder.ChildAdded)[1]
        print(c.Enabled, c.ForeignState, c.LuaConnection, typeof(c.Function), typeof(c.Thread))
        c.Function()
        c:Fire(workspace)
        firesignal(folder.ChildAdded)
        firesignal(folder.ChildAdded, workspace)
        c:Disable()
        print(c.Enabled)
        firesignal(folder.ChildAdded)
        c:Enable()
        c:Disconnect()
        print(#getconnections(folder.ChildAdded), pcall(replicatesignal, folder.ChildAdded))
    "##,
    );
    let want = [
        "false\tPlayer1\ttrue\tfalse",
        "true\tfalse",
        "true\tnil",
        "true\ttrue",
        "Player1 Fired M1",
        "Player1 Fired M2",
        "Player1 triggered the prompt",
        "Head touched",
        "from callback\tnil",
        "true\tfalse\ttrue\tfunction\tthread",
        "Triggered\tnil",
        "Triggered\tInstance",
        "Triggered\tnil",
        "Triggered\tInstance",
        "false",
    ];
    for (i, w) in want.iter().enumerate() {
        assert_eq!(p.get(i).map(String::as_str), Some(*w), "line {i}: {p:#?}");
    }
    assert!(
        p[15].starts_with("0\tfalse\t") && p[15].contains("no server"),
        "{}",
        p[15]
    );
}

#[test]
fn scripts_and_reflection() {
    let p = printed(
        r##"
        local found = false
        for _, s in getscripts() do if s == script then found = true end end
        local running = false
        for _, s in getrunningscripts() do if s == script then running = true end end
        local idle = Instance.new("LocalScript")
        for _, s in getrunningscripts() do if s == idle then running = "idle too!" end end
        print(found, running, getcallingscript(), getscriptfromthread(coroutine.running()))
        mine = "global of mine"
        print(getsenv(script).mine, pcall(getsenv, idle))
        print(typeof(getscriptclosure(script)), getscriptclosure(idle), type(getscriptbytecode(script)), getscriptbytecode(idle))
        print(#getscripthash(script), getscripthash(idle))

        local module = Instance.new("ModuleScript")
        sethiddenproperty(module, "Source", "return { answer = 42 }")
        local unloaded = Instance.new("ModuleScript")
        print(require(module).answer, require(module) == require(module))
        local loaded, wrong = false, false
        for _, m in getloadedmodules() do
            if m == module then loaded = true elseif m == unloaded then wrong = true end
        end
        print(loaded, wrong)

        local part = Instance.new("Part")
        print(gethiddenproperty(part, "Name"))
        print(gethiddenproperty(part, "NetworkOwnerV3"))
        print(sethiddenproperty(part, "DefinesCapabilities", true), gethiddenproperty(part, "DefinesCapabilities"))
        setscriptable(part, "Name", false)
        print(isscriptable(part, "Name"), pcall(function() return part.Name end))
        setscriptable(part, "Name", true)
        print(isscriptable(part, "Name"), part.Name, isscriptable(part, "Nope"))
        setscriptable(part, "DefinesCapabilities", true)
        print(part.DefinesCapabilities)

        task.defer(function()
            setthreadidentity(2)
            print("deferred", getthreadidentity(), game:GetService("CoreGui"))
        end)
        setthreadidentity(3)
        print(getthreadidentity(), game:GetService("CoreGui") ~= nil)
    "##,
    );
    assert_eq!(p[0], "true\ttrue\tnil\tnil");
    assert!(
        p[1].starts_with("global of mine\tfalse\t") && p[1].contains("not running"),
        "{}",
        p[1]
    );
    assert_eq!(p[2], "function\tnil\tstring\tnil");
    assert_eq!(p[3], "96\tnil");
    assert_eq!(p[4], "42\ttrue");
    assert_eq!(p[5], "true\tfalse");
    assert_eq!(p[6], "Part\tfalse");
    assert_eq!(p[7], "nil\ttrue");
    assert_eq!(p[8], "true\ttrue\ttrue");
    assert!(p[9].starts_with("false\tfalse\t"), "{}", p[9]);
    assert_eq!(p[10], "true\tPart\tnil");
    assert_eq!(p[11], "true");
    assert_eq!(p[12], "3\ttrue");
    assert_eq!(p[13], "deferred\t2\tnil");
}

#[test]
fn drawing() {
    let (mut st, mut game, p) = run_mod(
        r##"
        local viewport = workspace.CurrentCamera.ViewportSize
        local circle = Drawing.new("Circle")
        circle.Radius = 50
        circle.Color = Color3.fromRGB(255, 0, 0)
        circle.Filled = true
        circle.NumSides = 8
        circle.Position = Vector2.new(viewport.X / 2, viewport.Y / 2)
        circle.Visible = true
        print(getrenderproperty(circle, "Radius"), circle.Visible, isrenderobj(circle), isrenderobj(workspace), isrenderobj("x"))
        setrenderproperty(circle, "Radius", 40)
        print(circle.Radius, circle.__OBJECT_EXISTS, circle.Transparency)
        local text = Drawing.new("Text")
        text.Text = "Hello"
        text.Size = 20
        text.Position = Vector2.new(10, 10)
        text.Outline = true
        text.Visible = true
        print(text.TextBounds.X > 0, pcall(function() text.Size = "big" end))
        local line = Drawing.new("Line")
        line.From = Vector2.new(0, 0)
        line.To = Vector2.new(100, 0)
        line.Visible = true
        local square = Drawing.new("Square")
        square.Size = Vector2.new(20, 20)
        square.Filled = true
        square.Visible = true
        local gone = Drawing.new("Triangle")
        gone:Destroy()
        print(gone.__OBJECT_EXISTS, pcall(Drawing.new, "Hexagon"))
        command("clearall", function() task.defer(cleardrawcache) end)
        _G.circle = circle
    "##,
        0.6,
    );
    assert_eq!(p[0], "50\ttrue\ttrue\tfalse\tfalse");
    assert_eq!(p[1], "40\ttrue\t0");
    assert!(p[2].starts_with("true\tfalse\t"), "{}", p[2]);
    assert!(p[3].starts_with("false\tfalse\t"), "{}", p[3]);
    game.drawn.clear();
    game.quads.clear();
    st.hud(&mut game);
    // The filled circle is a fan of 6 triangles (8 sides), the line one quad.
    assert!(game.quads.len() >= 7, "{}", game.quads.len());
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text Hello")),
        "{:?}",
        game.drawn
    );
    assert!(
        game.drawn.iter().any(|d| d.starts_with("rect [0.0, 0.0, 20.0, 20.0]")),
        "{:?}",
        game.drawn
    );
    assert!(st.command("clearall"));
    run_for(&mut st, &mut game, 0.1);
    game.quads.clear();
    st.hud(&mut game);
    assert!(game.quads.is_empty());
    assert!(st.s.rbx.drawings.is_empty());
}

#[test]
fn network_stays_on_the_internet() {
    let mut game = FakeGame::new("sunc-net");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r##"
        local r = request({ Url = "http://127.0.0.1:9/", Method = "GET" })
        print(r.Success, r.StatusCode, #r.StatusMessage > 0, typeof(r.Headers), r.Body)
        print(pcall(function() return game:HttpGet("http://127.0.0.1:9/") end))
        print(pcall(request, { Url = "http://example.com", Method = "BREW" }))
        print(pcall(WebSocket.connect, "ws://127.0.0.1:9/"))
        print(type(http.request), type(http_request), type(WebSocket.connect))
    "##,
    );
    // Requests answer on threads of their own: real time has to pass.
    let mut p: Vec<String> = Vec::new();
    for _ in 0..400 {
        run_for(&mut st, &mut game, 0.05);
        std::thread::sleep(std::time::Duration::from_millis(5));
        p = log(&st)
            .iter()
            .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
            .collect();
        if p.len() >= 5 {
            break;
        }
    }
    assert!(!p.is_empty(), "nothing printed");
    assert!(p[0].starts_with("false\t0\ttrue\ttable\t"), "{}", p[0]);
    assert!(p[1].starts_with("false\t"), "{}", p[1]);
    assert!(
        p[2].starts_with("false\t") && p[2].contains("unknown method"),
        "{}",
        p[2]
    );
    assert!(p[3].starts_with("false\t"), "{}", p[3]);
    assert_eq!(p[4], "function\tfunction\tfunction");
}
