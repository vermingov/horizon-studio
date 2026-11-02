// What the server sends the client, as Roblox instances
// (hstudio_rbx_server): ReplicatedStorage.Server's resource packs, file by
// file as ModuleScripts whose Source is Lua that gives the file back, the
// edits of them going into the override pack the game reloads with (and a
// bad one refused); the plugins its channels and commands name, with a
// RemoteEvent per channel on the real custom payloads; its command tree and
// registries; all of it made as it is looked at.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_lua::Value;
use horizon_studio::game::hstudio_rbx_remote::Field;
use horizon_studio::game::hstudio_rbx_server::{
    self as server, Kind, Registries, ServerContent, ServerPack, channel_plugin, json_of, kind_of, long_string, lua_of,
    usage,
};
use serde_json::json;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// A tiny PNG of `w` by `h`.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&vec![200; (w * h * 4) as usize]).unwrap();
    }
    out
}

const FSH: &str =
    "#version 330\n\nin vec4 vertexColor;\nout vec4 fragColor;\n\nvoid main() {\n    fragColor = vertexColor;\n}\n";

/// The server's pack, a zip like the ones the game downloads, with `extra`
/// item models more.
fn server_zip(dir: &Path, extra: usize) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("server-pack.zip");
    let f = std::fs::File::create(&path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default();
    let mut put = |name: &str, bytes: &[u8]| {
        z.start_file(name, opts).unwrap();
        z.write_all(bytes).unwrap();
    };
    put(
        "pack.mcmeta",
        br#"{"pack":{"description":"The server's look","pack_format":69}}"#,
    );
    put(
        "assets/minecraft/models/block/stone.json",
        br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"},"elements":[]}"#,
    );
    put(
        "assets/minecraft/blockstates/stone.json",
        br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
    );
    put(
        "assets/minecraft/lang/en_us.json",
        "{\"block.minecraft.stone\":\"Rock\",\"menu.quit\":\"Leave \\\"now\\\"\"}".as_bytes(),
    );
    put(
        "assets/minecraft/sounds.json",
        br#"{"block.stone.break":{"sounds":["custom/rock",{"name":"custom/rock2","volume":0.5}],"replace":false}}"#,
    );
    put("assets/minecraft/textures/block/stone.png", &png(2, 3));
    put("assets/minecraft/shaders/core/rendertype_text.fsh", FSH.as_bytes());
    put("assets/minecraft/sounds/custom/rock.ogg", b"OggS....");
    for i in 0..extra {
        put(
            &format!("assets/minecraft/models/item/thing_{i}.json"),
            format!(
                "{{\"parent\":\"minecraft:item/generated\",\"textures\":{{\"layer0\":\"minecraft:item/thing_{i}\"}}}}"
            )
            .as_bytes(),
        );
    }
    z.finish().unwrap();
    path
}

fn commands() -> serde_json::Value {
    let tp = json!({
        "type": "literal",
        "children": {
            "targets": {"type": "argument", "parser": "minecraft:entity", "properties": "00", "executable": true,
                "children": {"location": {"type": "argument", "parser": "minecraft:vec3", "executable": true}}},
        }
    });
    let wand = json!({"type": "literal", "executable": true});
    let lp = json!({"type": "literal", "executable": true,
        "children": {"user": {"type": "literal", "children": {"name": {"type": "argument", "parser": "brigadier:string", "properties": "00", "executable": true}}}}});
    json!({
        "type": "root",
        "children": {
            "tp": tp.clone(),
            "minecraft:tp": tp,
            "/wand": wand.clone(),
            "worldedit:/wand": wand,
            "lp": lp.clone(),
            "luckperms:lp": lp,
            "teleport": {"type": "literal", "redirect": ["tp"]},
        }
    })
}

fn content(zip: &Path) -> ServerContent {
    ServerContent {
        session: 7,
        versions: [1, 1, 1, 1],
        packs: Some(vec![ServerPack {
            id: "server/0000/abc".into(),
            path: zip.to_path_buf(),
        }]),
        channels: Some(vec![
            "minecraft:register".into(),
            "worldedit:cui".into(),
            "bungeecord:main".into(),
            "BungeeCord".into(),
            "voicechat:request_secret".into(),
        ]),
        commands: Some(commands()),
        registries: Some(Registries {
            entries: vec![(
                "minecraft:worldgen/biome".into(),
                vec![
                    (
                        "minecraft:plains".into(),
                        json!({"temperature": 0.8, "has_precipitation": true}),
                    ),
                    (
                        "minecraft:desert".into(),
                        json!({"temperature": 2.0, "has_precipitation": false}),
                    ),
                ],
            )],
            tags: vec![(
                "minecraft:block".into(),
                vec![(
                    "minecraft:logs".into(),
                    vec!["minecraft:oak_log".into(), "minecraft:birch_log".into()],
                )],
            )],
        }),
    }
}

fn on_server(name: &str, extra: usize) -> (Studio, FakeGame) {
    let mut game = FakeGame::new(name);
    let zip = server_zip(&game.dir, extra);
    game.content = Some(content(&zip));
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.1);
    (st, game)
}

fn said(st: &Studio) -> Vec<String> {
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix("S|").map(|s| s.replace("    ", "\t")))
        .collect()
}

fn run(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    let before = said(st).len();
    st.run_console(game, src);
    run_for(st, game, 0.3);
    said(st)[before..].to_vec()
}

/// The instance at a path of names under ReplicatedStorage, as the studio has it.
fn find(st: &Studio, path: &[&str]) -> Option<u32> {
    let rbx = &st.s.rbx;
    let mut at = rbx.service("ReplicatedStorage")?;
    for name in path {
        at = rbx.child_named(at, name)?;
    }
    Some(at)
}

#[test]
fn the_packs_files_are_modulescripts_that_give_their_data() {
    let (mut st, mut game) = on_server("server-packs", 0);
    let out = run(
        &mut st,
        &mut game,
        r#"
        local server = game:GetService("ReplicatedStorage"):WaitForChild("Server", 5)
        task.wait(0.1)
        server = game:GetService("ReplicatedStorage"):WaitForChild("Server", 5)
        local pack = server.ResourcePacks["Server Pack"]
        print("S|pack", pack:GetAttribute("Id"), #pack:GetChildren())
        local stone = pack.assets.minecraft.models.block["stone.json"]
        print("S|class", stone.ClassName)
        local m = require(stone)
        print("S|model", m.parent, m.textures.all, #m.elements)
        print("S|decompile", decompile(stone) == stone.Source, stone.Source:find("return {", 1, true) ~= nil)
        local lang = require(pack.assets.minecraft.lang["en_us.json"])
        print("S|lang", lang["block.minecraft.stone"], lang["menu.quit"])
        local sounds = require(pack.assets.minecraft["sounds.json"])
        print("S|sounds", sounds["block.stone.break"].sounds[1], sounds["block.stone.break"].sounds[2].volume)
        local glsl = require(pack.assets.minecraft.shaders.core["rendertype_text.fsh"])
        print("S|glsl", glsl == [==[#version 330

in vec4 vertexColor;
out vec4 fragColor;

void main() {
    fragColor = vertexColor;
}
]==])
        local png = require(pack.assets.minecraft.textures.block["stone.png"])
        print("S|png", png.Width, png.Height, png.File, isfile(png.File))
        print("S|ogg", require(pack.assets.minecraft.sounds.custom["rock.ogg"]).Bytes)
        print("S|meta", require(pack["pack.mcmeta"]).pack.description)
        "#,
    );
    assert_eq!(
        out,
        [
            "pack\tserver/0000/abc\t2",
            "class\tModuleScript",
            "model\tminecraft:block/cube_all\tminecraft:block/stone\t0",
            "decompile\ttrue\ttrue",
            "lang\tRock\tLeave \"now\"",
            "sounds\tcustom/rock\t0.5",
            "glsl\ttrue",
            "png\t2\t3\t.server-packs/Server Pack/assets/minecraft/textures/block/stone.png\ttrue",
            "ogg\t8",
            "meta\tThe server's look",
        ],
        "{:?}",
        log(&st)
    );
    // The image's copy in the workspace is the pack's.
    let copy =
        st.s.rbx
            .workspace_dir
            .join(".server-packs/Server Pack/assets/minecraft/textures/block/stone.png");
    assert_eq!(std::fs::read(copy).unwrap(), png(2, 3));
    // The tree sits in ReplicatedStorage, where Dex finds it.
    assert!(find(&st, &["Server", "README"]).is_some());
    assert!(find(&st, &["Server", "Channels"]).is_some());
}

#[test]
fn a_big_pack_is_made_as_it_is_looked_at() {
    let (mut st, mut game) = on_server("server-lazy", 5000);
    let before = st.s.rbx.insts.len();
    // The layer wakes; the tree is made, its packs' folders empty.
    run(&mut st, &mut game, r#"local _ = game:GetService("ReplicatedStorage")"#);
    run_for(&mut st, &mut game, 0.2);
    let pack = find(&st, &["Server", "ResourcePacks", "Server Pack"]).expect("the pack's folder");
    assert!(st.s.rbx.children(pack).is_empty());
    assert!(st.s.rbx.server.is_lazy(pack));
    let made = st.s.rbx.insts.len() - before;
    assert!(made < 60, "joining made {made} instances");
    // game:GetDescendants() (what Dex does first) makes none of them.
    let out = run(
        &mut st,
        &mut game,
        r#"local n = #game:GetDescendants() print("S|all", n > 0)"#,
    );
    assert_eq!(out, ["all\ttrue"]);
    assert!(st.s.rbx.children(pack).is_empty());
    // An explorer showing it (tostring) fills one level.
    let out = run(
        &mut st,
        &mut game,
        r#"local p = game.ReplicatedStorage.Server.ResourcePacks:FindFirstChild("Server Pack") print("S|shown", tostring(p))"#,
    );
    assert_eq!(out, ["shown\tServer Pack"]);
    let kids: Vec<String> =
        st.s.rbx
            .children(pack)
            .iter()
            .map(|&c| st.s.rbx.inst(c).name.clone())
            .collect();
    assert_eq!(kids, ["assets", "pack.mcmeta"]);
    let assets = st.s.rbx.child_named(pack, "assets").unwrap();
    assert!(st.s.rbx.children(assets).is_empty());
    // The folder of 5000 files fills when it is opened, and fast.
    let t = std::time::Instant::now();
    let out = run(
        &mut st,
        &mut game,
        r#"local items = game.ReplicatedStorage.Server.ResourcePacks["Server Pack"].assets.minecraft.models.item
        print("S|items", #items:GetChildren(), require(items["thing_42.json"]).textures.layer0)"#,
    );
    assert_eq!(out, ["items\t5000\tminecraft:item/thing_42"]);
    assert!(t.elapsed().as_secs_f64() < 20.0, "{:?}", t.elapsed());
    // GetDescendants inside the tree is all of it.
    let out = run(
        &mut st,
        &mut game,
        r#"print("S|deep", #game.ReplicatedStorage.Server.ResourcePacks:GetDescendants() > 5000)"#,
    );
    assert_eq!(out, ["deep\ttrue"]);
}

#[test]
fn an_edit_goes_into_the_override_pack_and_a_bad_one_is_refused() {
    let (mut st, mut game) = on_server("server-edit", 0);
    let out = run(
        &mut st,
        &mut game,
        r#"
        task.spawn(function()
            local pack = game:GetService("ReplicatedStorage"):WaitForChild("Server"):WaitForChild("ResourcePacks"):WaitForChild("Server Pack")
            local stone = pack.assets.minecraft.models.block["stone.json"]
            local m = require(stone)
            m.textures.all = "minecraft:block/dirt"
            m.ambientocclusion = false
            -- The data as Lua, back in its Source.
            local src = "return { parent = " .. string.format("%q", m.parent) .. ", textures = { all = 'minecraft:block/dirt' }, elements = {}, ambientocclusion = false }"
            stone.Source = src
            print("S|edited", require(stone).textures.all)
            print("S|bad", pcall(function() stone.Source = "return 5" end))
            print("S|syntax", pcall(function() stone.Source = "return {" end))
            print("S|errs", pcall(function() stone.Source = "error('nope')" end))
            print("S|mixed", pcall(function() stone.Source = "return { 1, 2, x = 3 }" end))
            print("S|kept", require(stone).textures.all)
            local lang = pack.assets.minecraft.lang["en_us.json"]
            print("S|lang", pcall(function() lang.Source = "return { ['block.minecraft.stone'] = 5 }" end))
            local state = pack.assets.minecraft.blockstates["stone.json"]
            print("S|state", pcall(function() state.Source = "return { nothing = true }" end))
            local fsh = pack.assets.minecraft.shaders.core["rendertype_text.fsh"]
            fsh.Source = "return [[#version 330\nout vec4 fragColor;\nvoid main() { fragColor = vec4(1.0); }\n]]"
            print("S|shader", pcall(function() fsh.Source = "return { }" end))
            local png = pack.assets.minecraft.textures.block["stone.png"]
            print("S|png", pcall(function() png.Source = "return {}" end))
            print("S|tp", pcall(function() game.ReplicatedStorage.Server.Commands.tp.Source = "return {}" end))
            print("S|meta", pcall(function() pack["pack.mcmeta"].Source = "return { pack = {} }" end))
            -- A table built by code is data too.
            local generated = "local t = {} for i = 1, 3 do t['k' .. i] = 'v' .. i end return t"
            lang.Source = generated
            print("S|done")
        end)
        "#,
    );
    let name = "ReplicatedStorage.Server.ResourcePacks.Server Pack.assets.minecraft";
    assert_eq!(out.len(), 13, "{out:?}\n{:?}", log(&st));
    assert_eq!(out[0], "edited\tminecraft:block/dirt");
    assert!(
        out[1].starts_with(&format!(
            "bad\tfalse\tconsole:11: {name}.models.block.stone.json: it gives a number: assets/minecraft/models/block/stone.json is a table (JSON); the file is as it was"
        )),
        "{}",
        out[1]
    );
    assert!(
        out[2].starts_with("syntax\tfalse\t") && out[2].ends_with("the file is as it was"),
        "{}",
        out[2]
    );
    assert!(
        out[3].contains("it fails as it runs") && out[3].contains("nope"),
        "{}",
        out[3]
    );
    assert!(
        out[4].contains("a table is a list (keys 1 to n) or has names for keys"),
        "{}",
        out[4]
    );
    assert_eq!(out[5], "kept\tminecraft:block/dirt");
    assert!(out[6].contains("a language's entries are text"), "{}", out[6]);
    assert!(out[7].contains("a blockstate has variants or multipart"), "{}", out[7]);
    assert!(
        out[8].contains("it gives a table: assets/minecraft/shaders/core/rendertype_text.fsh is text"),
        "{}",
        out[8]
    );
    assert!(out[9].contains("isn't data Lua gives"), "{}", out[9]);
    assert!(
        out[10].contains("is what the server sent: it can't be changed here"),
        "{}",
        out[10]
    );
    assert!(
        out[11].starts_with("meta\tfalse") && out[11].contains("pack.mcmeta describes the server's pack"),
        "{}",
        out[11]
    );
    run_for(&mut st, &mut game, 0.2);
    let p = said(&st);
    assert_eq!(p.last().map(String::as_str), Some("done"));
    // The good edits are in the override pack, and the resources reload with it.
    let dir = st.s.dir.join("server-overrides").join("hstudio-overrides");
    let model: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("assets/minecraft/models/block/stone.json")).unwrap()).unwrap();
    assert_eq!(
        model,
        json!({"parent": "minecraft:block/cube_all", "textures": {"all": "minecraft:block/dirt"}, "elements": [], "ambientocclusion": false})
    );
    // The file's own order, an empty list kept a list.
    let text = std::fs::read_to_string(dir.join("assets/minecraft/models/block/stone.json")).unwrap();
    assert!(text.find("parent").unwrap() < text.find("textures").unwrap());
    assert!(text.contains("\"elements\": []"), "{text}");
    assert_eq!(
        std::fs::read_to_string(dir.join("assets/minecraft/shaders/core/rendertype_text.fsh")).unwrap(),
        "#version 330\nout vec4 fragColor;\nvoid main() { fragColor = vec4(1.0); }\n"
    );
    let lang: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("assets/minecraft/lang/en_us.json")).unwrap()).unwrap();
    assert_eq!(lang, json!({"k1": "v1", "k2": "v2", "k3": "v3"}));
    assert!(!dir.join("assets/minecraft/blockstates/stone.json").exists());
    assert!(!dir.join("pack.mcmeta").exists() || std::fs::read(dir.join("pack.mcmeta")).is_ok());
    assert!(!game.overrides.is_empty());
    assert!(game.overrides.iter().all(|o| o.as_deref() == Some(dir.as_path())));
    assert!(
        log(&st)
            .iter()
            .any(|l| l.contains("in the override pack: the resources reload with it")),
        "{:?}",
        log(&st)
    );
}

#[test]
fn plugins_channels_commands_and_registries() {
    let (mut st, mut game) = on_server("server-plugins", 0);
    let out = run(
        &mut st,
        &mut game,
        r#"
        task.spawn(function()
            local server = game:GetService("ReplicatedStorage"):WaitForChild("Server")
            local names = {}
            for _, p in ipairs(server.Plugins:GetChildren()) do table.insert(names, p.Name) end
            table.sort(names)
            print("S|plugins", table.concat(names, " "))
            local cui = server.Plugins.worldedit.cui
            print("S|cui", cui.ClassName, cui:GetAttribute("Channel"))
            print("S|bungee", server.Plugins.bungeecord.main:GetAttribute("Channel"), server.Plugins.bungeecord.BungeeCord:GetAttribute("Channel"))
            cui.OnClientEvent:Connect(function(data) print("S|heard", data) end)
            cui:FireServer("hello")
            print("S|buffer", pcall(function() cui:FireServer(buffer.fromstring("bytes")) end))
            print("S|table", pcall(function() cui:FireServer({}) end))
            print("S|wand", require(server.Plugins.worldedit.Commands)["/wand"].executable)
            print("S|lp", require(server.Plugins.luckperms.Commands).lp.usage[2])
            local tp = require(server.Commands.tp)
            print("S|tp", tp.usage[1], tp.usage[2], tp.children.targets.parser)
            print("S|redirect", require(server.Commands.teleport).usage[1])
            print("S|channels", table.concat(require(server.Channels), " "))
            local biomes = require(server.Registries["minecraft:worldgen/biome"])
            print("S|biome", biomes["minecraft:plains"].temperature, biomes["minecraft:desert"].has_precipitation)
            print("S|tags", table.concat(require(server.Registries.Tags["minecraft:block"])["minecraft:logs"], " "))
            print("S|readme", require(server.README))
        end)
        "#,
    );
    assert_eq!(
        out,
        [
            "plugins\tbungeecord luckperms minecraft voicechat worldedit",
            "cui\tRemoteEvent\tworldedit:cui",
            "bungee\tbungeecord:main\tBungeeCord",
            "buffer\ttrue",
            "table\tfalse\tconsole:13: FireServer: worldedit:cui takes the payload as a string or a buffer, got table",
            "wand\ttrue",
            "lp\t/lp user <name>",
            "tp\t/tp <targets>\t/tp <targets> <location>\tminecraft:entity",
            "redirect\t/teleport -> /tp",
            "channels\tBungeeCord bungeecord:main voicechat:request_secret worldedit:cui",
            "biome\t0.8\tfalse",
            "tags\tminecraft:oak_log minecraft:birch_log",
            "readme\tReplicatedStorage.Server: what the server sent the client",
        ],
        "{:?}",
        log(&st)
    );
    // FireServer sent the custom payload on the channel.
    let sent: Vec<_> = game.packets.iter().filter(|p| p.name == "custom_payload").collect();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0].fields,
        [
            ("channel".to_string(), Field::text("worldedit:cui")),
            ("data".to_string(), Field::Str(b"hello".to_vec())),
        ]
    );
    assert_eq!(sent[1].fields[1].1, Field::Str(b"bytes".to_vec()));
    // The server's payload on it fires OnClientEvent.
    st.server_payload("worldedit:cui", b"from the server");
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(said(&st).last().map(String::as_str), Some("heard\tfrom the server"));
    // A channel first heard in the traffic gets its remote then.
    st.server_payload("axiom:hello", b"x");
    assert!(find(&st, &["Server", "Plugins", "axiom", "hello"]).is_some());
    // The protocol's own channels are no plugin's.
    st.server_payload("minecraft:register", b"a:b");
    assert!(find(&st, &["Server", "Plugins", "minecraft", "register"]).is_none());
}

#[test]
fn a_new_part_or_another_session_makes_it_again() {
    let (mut st, mut game) = on_server("server-session", 0);
    run(&mut st, &mut game, r#"local _ = game.ReplicatedStorage"#);
    run_for(&mut st, &mut game, 0.2);
    let root = find(&st, &["Server"]).unwrap();
    let out = run(
        &mut st,
        &mut game,
        r#"print("S|cmds", #game.ReplicatedStorage.Server.Commands:GetChildren())"#,
    );
    assert_eq!(out, ["cmds\t7"]);
    // The server sends its commands anew (an op given): Commands empties
    // and fills again with the new tree; the rest stays.
    let c = game.content.as_mut().unwrap();
    c.versions[server::COMMANDS] = 2;
    c.commands = Some(json!({"type": "root", "children": {"spawn": {"type": "literal", "executable": true}}}));
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(find(&st, &["Server"]), Some(root));
    let out = run(
        &mut st,
        &mut game,
        r#"local c = game.ReplicatedStorage.Server.Commands:GetChildren() print("S|cmds", #c, c[1].Name)"#,
    );
    assert_eq!(out, ["cmds\t1\tspawn"]);
    // Another connection: all of it anew.
    game.content.as_mut().unwrap().session = 8;
    run_for(&mut st, &mut game, 0.2);
    let again = find(&st, &["Server"]).unwrap();
    assert_ne!(again, root);
    assert!(st.s.rbx.inst(root).destroyed);
    // Off the server it stays as it was (the session's teardown is the layer's).
    game.content = None;
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(find(&st, &["Server"]), Some(again));
}

#[test]
fn json_as_lua_and_back() {
    let v = json!({
        "parent": "block/cube",
        "textures": {"#all": "a\"b\\c\n", "end": "x"},
        "elements": [{"from": [0, 0, 0], "to": [16, 16.5, 16], "faces": {}}],
        "empty": [],
        "big": 1e300,
        "neg": -0.25,
    });
    let lua = lua_of(&v, 0);
    assert!(lua.contains("parent = \"block/cube\""), "{lua}");
    assert!(lua.contains("[\"#all\"] = \"a\\\"b\\\\c\\n\""), "{lua}");
    assert!(lua.contains("[\"end\"] = \"x\""), "{lua}");
    assert!(lua.contains("from = { 0, 0, 0 }"), "{lua}");
    assert!(lua.contains("16.5"), "{lua}");
    // Back through Lua: the same JSON.
    let mut l = horizon_studio::game::hstudio_lua::Lua::new();
    let env = horizon_studio::game::hstudio_lua::Table::new();
    let Value::Table(env) = Value::table(env) else {
        unreachable!()
    };
    let f = l.load(&format!("return {lua}"), "t", &env).unwrap();
    let Ok(got) = l.call(
        &mut horizon_studio::game::hstudio_lua::NoHost::default(),
        &f,
        Vec::new(),
    ) else {
        panic!("the table literal runs")
    };
    let back = json_of(&got[0], Some(&v), &Value::Nil, "", 0).unwrap();
    assert_eq!(back, v);
    assert_eq!(
        serde_json::to_string(&back).unwrap(),
        serde_json::to_string(&v).unwrap()
    );
    // Long strings close past any bracket in the text.
    assert_eq!(long_string("a]]b"), "[=[\na]]b]=]");
    assert_eq!(long_string("a]=]b]]"), "[==[\na]=]b]]]==]");
    assert_eq!(kind_of("assets/x/shaders/core/a.vsh"), Kind::Text);
    assert_eq!(kind_of("pack.mcmeta"), Kind::Json);
    assert_eq!(kind_of("a/b.PNG"), Kind::Image);
    assert_eq!(kind_of("a/b.ogg"), Kind::Binary);
    assert_eq!(channel_plugin("worldedit:cui"), ("worldedit".into(), "cui".into()));
    assert_eq!(channel_plugin("BungeeCord"), ("bungeecord".into(), "BungeeCord".into()));
    assert_eq!(channel_plugin("WECUI"), ("worldedit".into(), "WECUI".into()));
    let u = usage("tp", &commands()["children"]["tp"], 8);
    assert_eq!(u, ["/tp <targets>", "/tp <targets> <location>"]);
}
