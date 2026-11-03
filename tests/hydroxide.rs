// What Hydroxide (github.com/Upbolt/Hydroxide) needs of the executor and the
// Roblox layer, each as the calls it makes: its loader's methods, its
// RemoteSpy's hooks on the game's packets, its ClosureSpy's hookfunction,
// the Upvalue and Constant Scanners' getgc and debug library, the Script and
// Module Scanners' script functions, and its UI, a model game:GetObjects
// loads. Hydroxide itself isn't here; the snippets are its calls.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_rbx_remote::{Field, Packet, Verdict};

fn setup(name: &str) -> (Studio, FakeGame) {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    (st, game)
}

/// The lines printed starting with `tag`, tabs as tabs.
fn tagged(st: &Studio, tag: &str) -> Vec<String> {
    log(st)
        .iter()
        .filter_map(|l| {
            let l = l.strip_prefix("[game] ").unwrap_or(l);
            l.starts_with(tag).then(|| l.replace("    ", "\t"))
        })
        .collect()
}

fn run(st: &mut Studio, game: &mut FakeGame, src: &str) {
    st.run_console(game, src);
    run_for(st, game, 0.2);
}

/// A mod standing in for a game's LocalScript: an upvalue, constants, a
/// remote it fires, a ModuleScript it requires.
const GAME_SCRIPT: &str = r#"
local coins = 1337
local remote = Instance.new("RemoteEvent")
remote.Name = "BuyItem"
remote.Parent = game:GetService("ReplicatedStorage")
local function buyItem(item, amount)
    coins = coins - amount
    remote:FireServer(item, amount)
    return "bought " .. item
end
local function getCoins() return coins end
getgenv().shop = { buy = buyItem, coins = getCoins }
local m = Instance.new("ModuleScript")
m.Name = "ShopConfig"
m.Source = "return { price = 25 }"
m.Parent = script
getgenv().config = require(m)
"#;

fn with_game_script(name: &str) -> (Studio, FakeGame) {
    let (mut st, mut game) = setup(name);
    game.write_mod("game.lua", GAME_SCRIPT);
    run_for(&mut st, &mut game, 0.5);
    (st, game)
}

fn move_pos() -> Packet {
    Packet {
        id: 6,
        name: "move_player_pos".into(),
        fields: vec![
            ("x".into(), Field::Num(1.0)),
            ("y".into(), Field::Num(64.0)),
            ("z".into(), Field::Num(2.0)),
            ("flags".into(), Field::Int(1)),
        ],
        mapped: true,
        raw: None,
    }
}

// ------------------------------------------------------------ the loader

#[test]
fn the_loader_finds_every_method_it_asks_for() {
    // init.lua's globalMethods, by the first name it tries; a missing one
    // is what makes a section say "Your exploit does not support this".
    let (mut st, mut game) = setup("hx-methods");
    run(
        &mut st,
        &mut game,
        r#"
        local m = {
          checkCaller = checkcaller, newCClosure = newcclosure, hookFunction = hookfunction,
          getGc = getgc, getInfo = debug.getinfo, getSenv = getsenv, getMenv = getmenv or getsenv,
          getContext = getthreadcontext, getConnections = getconnections, getScriptClosure = getscriptclosure,
          getNamecallMethod = getnamecallmethod, getCallingScript = getcallingscript,
          getLoadedModules = getloadedmodules, getConstants = debug.getconstants, getUpvalues = debug.getupvalues,
          getProtos = debug.getprotos, getStack = debug.getstack, getConstant = debug.getconstant,
          getUpvalue = debug.getupvalue, getProto = debug.getproto, getMetatable = getrawmetatable,
          getHui = gethui, setClipboard = setclipboard, setConstant = debug.setconstant,
          setContext = setthreadcontext, setUpvalue = debug.setupvalue, setStack = debug.setstack,
          setReadOnly = setreadonly, isLClosure = islclosure, isReadOnly = isreadonly, isXClosure = checkclosure,
          hookMetaMethod = hookmetamethod, readFile = readfile, writeFile = writefile, makeFolder = makefolder,
          isFolder = isfolder, isFile = isfile,
        }
        local missing = {}
        for _, k in ipairs({"checkCaller", "newCClosure", "hookFunction", "getGc", "getInfo", "getSenv", "getMenv",
          "getContext", "getConnections", "getScriptClosure", "getNamecallMethod", "getCallingScript",
          "getLoadedModules", "getConstants", "getUpvalues", "getProtos", "getStack", "getConstant", "getUpvalue",
          "getProto", "getMetatable", "getHui", "setClipboard", "setConstant", "setContext", "setUpvalue",
          "setStack", "setReadOnly", "isLClosure", "isReadOnly", "isXClosure", "hookMetaMethod", "readFile",
          "writeFile", "makeFolder", "isFolder", "isFile"}) do
          if not m[k] then table.insert(missing, k) end
        end
        print("MISSING", #missing, table.concat(missing, ","))
        -- It silences ScriptContext.Error's connections the old way.
        for _, c in pairs(getconnections(game:GetService("ScriptContext").Error)) do
          local mt = getrawmetatable(c)
          setreadonly(mt, false)
          c:Disable()
        end
        -- Its loader: the releases' JSON, its files under hydroxide/.
        local rel = game:GetService("HttpService"):JSONDecode('[{"tag_name":"v1"}]')[1]
        makefolder("hydroxide") makefolder("hydroxide/user")
        writefile("__oh_version.txt", rel.tag_name)
        print("FILES", isfolder("hydroxide/user"), isfile("__oh_version.txt"), readfile("__oh_version.txt"))
        -- methods/environment.lua: secureCall swaps its own environment.
        local env = getfenv(1)
        setfenv(1, setmetatable({ script = script }, { __index = getrenv() }))
        local inner = getfenv(1)
        setfenv(1, env)
        print("FENV", inner ~= env, getfenv(1) == env, typeof(gethui()))
        "#,
    );
    assert_eq!(tagged(&st, "MISSING"), ["MISSING\t0\t"]);
    assert_eq!(tagged(&st, "FILES"), ["FILES\ttrue\ttrue\tv1"]);
    assert_eq!(tagged(&st, "FENV"), ["FENV\ttrue\ttrue\tInstance"]);
}

// -------------------------------------------------------------- RemoteSpy

/// RemoteSpy's hook: a plain Lua function given to hookmetamethod, asking
/// who called through `getInfo(3).func`; blocked remotes return nothing.
const REMOTE_SPY: &str = r#"
getgenv().blocked = {}
local nmcTrampoline
nmcTrampoline = hookmetamethod(game, "__namecall", function(...)
    local instance = ...
    if typeof(instance) ~= "Instance" then return nmcTrampoline(...) end
    local method = getnamecallmethod()
    if instance.ClassName == "RemoteEvent" and method == "FireServer" then
        local call = { script = getcallingscript(), args = {select(2, ...)}, func = debug.getinfo(3).func }
        print("RSPY", instance.Name, call.script and call.script.Name, typeof(call.func), debug.info(call.func, "n"),
            #call.args)
        if blocked[instance.Name] then return end
    end
    return nmcTrampoline(...)
end)
"#;

#[test]
fn remote_spy_logs_the_game_s_packets_and_a_script_s_with_who_called() {
    let (mut st, mut game) = with_game_script("hx-remotespy");
    run(&mut st, &mut game, REMOTE_SPY);
    // The game's packet: its caller is the game, a C function, and no script.
    assert_eq!(st.game_packet(&mut game, &move_pos()), Verdict::Send);
    run(&mut st, &mut game, "getgenv().shop.buy('Sword', 1)");
    assert_eq!(
        tagged(&st, "RSPY"),
        [
            "RSPY\tMovePlayerPos\tnil\tfunction\tsend\t1",
            "RSPY\tBuyItem\tgame\tfunction\tbuyItem\t2"
        ]
    );
    assert!(!log(&st).iter().any(|l| l.contains("hook failed")), "{:?}", log(&st));
    // Blocking: the hook returns without the original.
    run(&mut st, &mut game, "blocked.MovePlayerPos = true");
    assert_eq!(st.game_packet(&mut game, &move_pos()), Verdict::Block);
}

#[test]
fn a_lua_hook_of_hookmetamethod_is_a_c_closure_level() {
    // As executors wrap it: level 2 in the hook is C, level 3 the code
    // that made the call; under the game's call, the game and no more.
    let (mut st, mut game) = setup("hx-levels");
    run(
        &mut st,
        &mut game,
        r#"
        local old
        old = hookmetamethod(game, "__namecall", function(self, ...)
          if getnamecallmethod() == "FireServer" then
            print("LV", debug.getinfo(2).what, debug.getinfo(3).what, debug.getinfo(3).name, debug.getinfo(4) == nil)
          end
          return old(self, ...)
        end)
        local function caller()
          game.ReplicatedStorage.Minecraft.MovePlayerPos:FireServer({x = 0, y = 64, z = 0, flags = 0})
        end
        caller()
        "#,
    );
    st.game_packet(&mut game, &move_pos());
    let lv = tagged(&st, "LV");
    assert_eq!(lv[0], "LV\tC\tLua\tcaller\tfalse");
    assert_eq!(lv[1], "LV\tC\tC\tsend\ttrue");
}

#[test]
fn a_script_path_names_services_and_children_as_remote_spies_write_it() {
    // getInstancePath tries each class as a service: a class that isn't
    // one is nil, so `.BuyItem` comes after ReplicatedStorage.
    let (mut st, mut game) = setup("hx-paths");
    run(
        &mut st,
        &mut game,
        r#"
        print("SVC", select(2, pcall(game.GetService, game, "RemoteEvent")), game:GetService("ReplicatedStorage").Name,
          pcall(game.GetService, game, "NotAClass"))
        "#,
    );
    let svc = tagged(&st, "SVC");
    assert!(svc[0].starts_with("SVC\tnil\tReplicatedStorage\tfalse\t"), "{svc:?}");
    assert!(svc[0].contains("is not a valid Service name"), "{svc:?}");
}

// ------------------------------------------------------------- ClosureSpy

#[test]
fn closure_spy_hooks_a_script_s_local_function() {
    let (mut st, mut game) = with_game_script("hx-closurespy");
    run(
        &mut st,
        &mut game,
        r#"
        local data = getgenv().shop.buy
        print("CS info", debug.getinfo(data).nups, debug.getinfo(data).name, islclosure(data))
        local hookCache = {}
        hookCache[data] = hookfunction(data, function(...)
          print("CS call", ...)
          return hookCache[data](...)
        end)
        print("CS ret", data("Shield", 2))
        "#,
    );
    assert_eq!(tagged(&st, "CS info"), ["CS info\t2\tbuyItem\ttrue"]);
    assert_eq!(tagged(&st, "CS call"), ["CS call\tShield\t2"]);
    assert_eq!(tagged(&st, "CS ret"), ["CS ret\tbought Shield"]);
}

// ------------------------------------------------- Upvalue / Constant Scanner

#[test]
fn scanners_find_a_script_s_functions_in_getgc_but_not_the_executor_s() {
    // Hydroxide skips checkclosure's functions: the executor's (the console's,
    // loadstring's). A mod's own code is the game's scripts here, and getgc
    // has its functions though it was made before anything asked for them.
    let (mut st, mut game) = with_game_script("hx-gc");
    run(
        &mut st,
        &mut game,
        r#"
        local mine = function() return "console" end
        local loaded = loadstring("return function() return 1 end")()
        print("XC", checkclosure(getgenv().shop.buy), checkclosure(mine), checkclosure(loaded), checkclosure(print))
        local names = {}
        for _, f in pairs(getgc()) do
          if type(f) == "function" and not checkclosure(f) and islclosure(f) then
            local n = debug.getinfo(f).name
            if n == "buyItem" or n == "getCoins" then names[n] = true end
          end
        end
        print("GC", names.buyItem, names.getCoins)
        print("FG", filtergc("function", { Name = "getCoins" }, true) == getgenv().shop.coins)
        "#,
    );
    assert_eq!(tagged(&st, "XC"), ["XC\tfalse\ttrue\ttrue\tfalse"]);
    assert_eq!(tagged(&st, "GC"), ["GC\ttrue\ttrue"]);
    assert_eq!(tagged(&st, "FG"), ["FG\ttrue"]);
}

#[test]
fn upvalue_scanner_reads_and_changes_an_upvalue() {
    let (mut st, mut game) = with_game_script("hx-upvalues");
    run(
        &mut st,
        &mut game,
        r#"
        local f = getgenv().shop.coins
        for i, v in pairs(debug.getupvalues(f)) do print("UP", i, v) end
        debug.setupvalue(f, 1, 99999)
        print("UP now", debug.getupvalue(f, 1), getgenv().shop.coins())
        "#,
    );
    assert_eq!(tagged(&st, "UP"), ["UP\t1\t1337", "UP now\t99999\t99999"]);
}

#[test]
fn constant_scanner_reads_and_changes_a_constant() {
    let (mut st, mut game) = with_game_script("hx-constants");
    run(
        &mut st,
        &mut game,
        r#"
        local f = getgenv().shop.buy
        local at
        for i, k in pairs(debug.getconstants(f)) do
          if k == "bought " then at = i end
        end
        print("K", at ~= nil, debug.getconstant(f, at))
        debug.setconstant(f, at, "got ")
        print("K now", f("Bow", 0))
        "#,
    );
    assert_eq!(tagged(&st, "K"), ["K\ttrue\tbought ", "K now\tgot Bow"]);
}

// ------------------------------------------------- Script / Module Scanner

#[test]
fn script_scanner_finds_a_script_its_closure_environment_and_source() {
    let (mut st, mut game) = with_game_script("hx-scripts");
    run(
        &mut st,
        &mut game,
        r#"
        local script
        for _, f in pairs(getgc()) do
          if type(f) == "function" and not checkclosure(f) then
            local s = rawget(getfenv(f), "script")
            if typeof(s) == "Instance" and s.Name == "game" then script = s end
          end
        end
        local closure = getscriptclosure(script)
        print("SS", script.ClassName, script:IsA("LocalScript"), typeof(getsenv(script)), #debug.getprotos(closure),
          table.find(getscripts(), script) ~= nil)
        print("SS src", decompile(script):find("local function buyItem", 1, true) ~= nil)
        "#,
    );
    assert_eq!(
        tagged(&st, "SS"),
        ["SS\tLocalScript\ttrue\ttable\t2\ttrue", "SS src\ttrue"]
    );
}

#[test]
fn module_scanner_lists_the_loaded_modules() {
    let (mut st, mut game) = with_game_script("hx-modules");
    run(
        &mut st,
        &mut game,
        r#"
        local found
        for _, m in pairs(getloadedmodules()) do
          if m.Name == "ShopConfig" then found = m end
        end
        print("MS", found ~= nil, found and #debug.getconstants(getscriptclosure(found)) >= 1, require(found).price)
        "#,
    );
    assert_eq!(tagged(&st, "MS"), ["MS\ttrue\ttrue\t25"]);
}

// ------------------------------------------------------------- the UI model

/// A model file as Roblox Studio saves one: a ScreenGui, a Frame in it with
/// a TextLabel, and an ObjectValue pointing at the label.
const MODEL: &str = r#"<roblox xmlns:xmime="http://www.w3.org/2005/05/xmlmime" version="4">
  <Item class="ScreenGui" referent="RBX0">
    <Properties><string name="Name">Interface</string><token name="ZIndexBehavior">1</token></Properties>
    <Item class="Frame" referent="RBX1">
      <Properties>
        <string name="Name">Base</string>
        <UDim2 name="Size"><XS>0</XS><XO>650</XO><YS>0</YS><YO>350</YO></UDim2>
        <Color3 name="BackgroundColor3"><R>0.125</R><G>0.25</G><B>0.5</B></Color3>
        <bool name="Visible">false</bool>
        <BinaryString name="AttributesSerialize"></BinaryString>
      </Properties>
      <Item class="TextLabel" referent="RBX2">
        <Properties>
          <string name="Name">Status</string>
          <string name="Text">• Status: Home</string>
          <token name="TextXAlignment">0</token>
          <float name="TextSize">18</float>
        </Properties>
      </Item>
    </Item>
    <Item class="ObjectValue" referent="RBX3">
      <Properties><string name="Name">Pointer</string><Ref name="Value">RBX2</Ref></Properties>
    </Item>
  </Item>
</roblox>
"#;

#[test]
fn get_objects_loads_a_model_kept_in_the_workspace() {
    // Hydroxide's UI is two Roblox models (`game:GetObjects("rbxassetid://...")`);
    // Roblox serves them only to a signed-in account, so they are files
    // in the workspace's .roblox-assets.
    let (mut st, mut game) = setup("hx-getobjects");
    let assets = st.s.dir.join("workspace").join(".roblox-assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("11389137937.rbxmx"), MODEL).unwrap();
    std::fs::write(st.s.dir.join("workspace").join("ui.rbxmx"), MODEL).unwrap();
    run(
        &mut st,
        &mut game,
        r#"
        task.spawn(function()
          local objects = game:GetObjects("rbxassetid://11389137937")
          local ui = objects[1]
          local base = ui.Base
          print("GO", #objects, ui.ClassName, ui.Parent, ui.ZIndexBehavior.Name, base.ClassName, base.Size, base.Visible)
          print("GO", base.BackgroundColor3, base.Status.Text, base.Status.TextXAlignment.Name, base.Status.TextSize,
            ui.Pointer.Value == base.Status)
          print("GO file", game:GetObjects("ui.rbxmx")[1].Name, game:GetService("InsertService"):LoadLocalAsset("ui.rbxmx").Name)
          -- No account connected and not cached: the call waits for the
          -- in-game prompt (so it must not have returned and printed).
          game:GetObjects("rbxassetid://5042114982")
          print("GO reached-after-missing")
        end)
        "#,
    );
    let go = tagged(&st, "GO");
    assert_eq!(
        go[0],
        "GO\t1\tScreenGui\tnil\tSibling\tFrame\t{0, 650}, {0, 350}\tfalse"
    );
    assert_eq!(go[1], "GO\t0.125, 0.25, 0.5\t• Status: Home\tLeft\t18\ttrue");
    assert_eq!(go[2], "GO file\tInterface\tInterface");
    // The missing asset with no account connected yields, waiting for the
    // prompt; it never returned, so nothing printed past it.
    assert!(!go.iter().any(|l| l.contains("reached-after-missing")), "{go:?}");
    assert!(
        st.s.rbx.assets.iter().any(|a| a.id == 5042114982 && a.job.is_none()),
        "a thread should be waiting for the Roblox-account prompt"
    );
}

#[test]
fn a_gzipped_model_from_roblox_s_cdn_loads() {
    // Roblox's asset CDN sends models gzip-compressed (asked for it or not),
    // and that is what the download keeps: the reader unpacks it.
    use std::io::Write;
    let (mut st, mut game) = setup("hx-gzip");
    let assets = st.s.dir.join("workspace").join(".roblox-assets");
    std::fs::create_dir_all(&assets).unwrap();
    let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(MODEL.as_bytes()).unwrap();
    std::fs::write(assets.join("4743303040.rbxm"), z.finish().unwrap()).unwrap();
    run(
        &mut st,
        &mut game,
        r#"
        task.spawn(function()
          local ok, objects = pcall(game.GetObjects, game, "rbxassetid://4743303040")
          print("GZ", ok, ok and objects[1].ClassName or objects, ok and objects[1].Base.ClassName)
        end)
        "#,
    );
    assert_eq!(tagged(&st, "GZ"), ["GZ\ttrue\tScreenGui\tFrame"]);
}
