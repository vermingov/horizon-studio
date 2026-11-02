// saveinstance as a world downloader: the world the client has goes into
// the studio's workspace folder as a Minecraft world (the game writes it:
// the fake game here records the dump), which Singleplayer lists beside the
// saves (hstudio_worlds); the calling thread waits until it is written (code
// that can't wait gets the folder at once), and a part's or model's blocks
// become a vanilla structure file. Nothing of Roblox's file formats is written.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::{ClientInfo, ServerInfo};
use horizon_studio::game::hstudio_dump::{
    self as dump, MARKER, Region, RegionBlock, block_box, date_of, folder_name, sanitize, structure_nbt,
};
use horizon_studio::title::hstudio_worlds as worlds;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

fn on_server(game: &mut FakeGame, address: &str) {
    game.client = ClientInfo {
        server: Some(ServerInfo {
            address: address.into(),
            name: address.into(),
            brand: "Paper".into(),
            own_world: false,
        }),
        ..ClientInfo::default()
    };
    let saves = game.dir.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    game.saves = Some(saves);
}

fn printed(st: &horizon_studio::game::hstudio::Studio) -> Vec<String> {
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(|s| s.replace("    ", "\t")))
        .collect()
}

#[test]
fn saveinstance_dumps_the_world_into_the_workspace_and_waits_until_it_is_written() {
    let mut game = FakeGame::new("save-world");
    on_server(&mut game, "play.example.net:25565");
    game.dump_polls = 10;
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        task.spawn(function()
            local a = saveinstance()
            print("first", a)
            local b = saveinstance(game, { FilePath = "My Dump", Decompile = true, NilInstances = true, mode = "full" })
            print("second", b)
            local c = saveinstance({ FileName = "other.rbxl", ShowStatus = false })
            print("third", c)
            print("again", saveinstance() == a, SaveInstance(game, "My Dump") == b)
        end)
        print("went on", iscclosure(saveinstance))
        "#,
    );
    for _ in 0..100 {
        run_for(&mut st, &mut game, 0.05);
        if !printed(&st).is_empty() {
            break;
        }
    }
    // The thread is waiting for the first dump; the mod went on.
    run_for(&mut st, &mut game, 0.2);
    assert_eq!(printed(&st), ["went on\ttrue"], "{:?}", log(&st));
    assert_eq!(game.dumps.len(), 1);
    run_for(&mut st, &mut game, 3.0);
    let date = dump::today();
    let ws = st.s.rbx.workspace_dir.clone();
    assert_eq!(ws, st.s.dir.join("workspace"));
    let first = ws.join(format!("play.example.net 25565 {date}"));
    let mine = ws.join("My Dump");
    let other = ws.join("other");
    assert_eq!(
        printed(&st),
        [
            "went on\ttrue".to_string(),
            format!("first\t{}", first.display()),
            format!("second\t{}", mine.display()),
            format!("third\t{}", other.display()),
            "again\ttrue\ttrue".to_string(),
        ],
        "{:?}",
        log(&st)
    );
    assert_eq!(
        game.dumps,
        [first.clone(), mine.clone(), other.clone(), first.clone(), mine.clone()]
    );
    // Each world has the marker that lets saveinstance add to it again.
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(first.join(MARKER)).unwrap()).unwrap();
    assert_eq!(marker["source"], "play.example.net:25565");
    assert!(marker["first_saved"].as_u64().unwrap() <= marker["last_saved"].as_u64().unwrap());
    assert!(other.join("level.dat").is_file());
    // Nothing went into the saves, and no Roblox file anywhere.
    assert!(walk(&game.dir.join("saves")).is_empty());
    let rbx_files: Vec<_> = walk(&ws)
        .into_iter()
        .filter(|p| p.to_string_lossy().contains(".rbx"))
        .collect();
    assert!(rbx_files.is_empty(), "{rbx_files:?}");
    // Singleplayer lists them, by their path from the saves.
    let saves = game.dir.join("saves");
    for w in [&first, &mine, &other] {
        let name = w.file_name().unwrap().to_str().unwrap();
        let folder = worlds::listed(&saves, &ws, name).expect("a dump world is listed");
        assert!(folder.starts_with("../"), "{folder}");
        assert_eq!(worlds::resolve_in(&saves, &ws, &folder).as_deref(), Some(w.as_path()));
    }
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            }
            out.push(p);
        }
    }
    out
}

#[test]
fn saveinstance_never_writes_into_a_folder_it_did_not_make() {
    let mut game = FakeGame::new("save-refuse");
    on_server(&mut game, "mc.example.org");
    let mut st = studio(&mut game);
    let ws = st.s.dir.join("workspace");
    std::fs::create_dir_all(ws.join("Survival")).unwrap();
    std::fs::write(ws.join("Survival").join("level.dat"), b"the player's own").unwrap();
    std::fs::write(ws.join("a file"), b"not a world").unwrap();
    game.write_mod(
        "t.lua",
        r#"
        task.spawn(function()
            print(pcall(saveinstance, { FilePath = "Survival" }))
            print(pcall(saveinstance, { FilePath = "a file" }))
            print(pcall(saveinstance, { FilePath = "../../escape" }))
        end)
        "#,
    );
    run_for(&mut st, &mut game, 1.0);
    let p = printed(&st);
    assert_eq!(p.len(), 3, "{:?}", log(&st));
    assert!(
        p[0].starts_with(
            "false\tsaveinstance: the workspace folder already has a folder 'Survival' that saveinstance didn't make"
        ),
        "{p:?}"
    );
    assert!(
        p[1].starts_with("false\tsaveinstance: 'a file' in the workspace folder is a file"),
        "{p:?}"
    );
    // A path is one folder's name in the workspace, never a way out of it.
    let escaped = ws.join("_.._escape");
    assert_eq!(p[2], format!("true\t{}", escaped.display()), "{p:?}");
    assert_eq!(game.dumps, [escaped]);
    assert_eq!(
        std::fs::read(ws.join("Survival").join("level.dat")).unwrap(),
        b"the player's own"
    );
    assert!(!ws.join("Survival").join(MARKER).exists());
    assert!(!game.dir.join("escape").exists());
    // The world list shows none but saveinstance's.
    let saves = game.dir.join("saves");
    assert_eq!(worlds::listed(&saves, &ws, "Survival"), None);
    assert_eq!(worlds::listed(&saves, &ws, "a file"), None);
}

#[test]
fn a_dump_that_fails_is_a_lua_error() {
    let mut game = FakeGame::new("save-fail");
    on_server(&mut game, "mc.example.org");
    game.dump_result = Err("the disk is full".into());
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        task.spawn(function()
            print(pcall(saveinstance))
        end)
        "#,
    );
    run_for(&mut st, &mut game, 1.0);
    assert_eq!(
        printed(&st),
        ["false\tsaveinstance: the disk is full"],
        "{:?}",
        log(&st)
    );
}

#[test]
fn code_that_cant_wait_still_saves_the_world() {
    // Inside gsub's callback no thread can wait: the folder comes back at
    // once, the dump goes on, and the chat says when it is written.
    let mut game = FakeGame::new("save-nowait");
    on_server(&mut game, "mc.example.org");
    game.dump_polls = 5;
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local got
        string.gsub("x", "x", function() got = saveinstance({ FilePath = "Clicked" }) end)
        print("got", got)
        "#,
    );
    for _ in 0..100 {
        run_for(&mut st, &mut game, 0.05);
        if !printed(&st).is_empty() {
            break;
        }
    }
    let world = st.s.dir.join("workspace").join("Clicked");
    assert_eq!(printed(&st), [format!("got\t{}", world.display())], "{:?}", log(&st));
    assert_eq!(game.dumps, std::slice::from_ref(&world));
    run_for(&mut st, &mut game, 1.0);
    assert!(world.join("level.dat").is_file());
    let said = format!("saveinstance: the world is saved in {}", world.display());
    assert!(game.chats.iter().any(|c| c == &said), "{:?}", game.chats);
}

#[test]
fn the_world_list_finds_workspace_worlds_from_the_saves() {
    let root = std::env::temp_dir().join(format!("hstudio-worlds-{}", std::process::id()));
    let saves = root.join("saves");
    let ws = root.join("horizon-mods/.horizon-mod-data/horizon_studio/workspace");
    let world = ws.join("play.example.net 2026-11-02");
    std::fs::create_dir_all(&world).unwrap();
    std::fs::create_dir_all(&saves).unwrap();
    // The marker's name is saveinstance's.
    assert_eq!(worlds::MARKER, MARKER);
    let folder = worlds::folder_for(&saves, &world);
    assert_eq!(
        folder,
        "../horizon-mods/.horizon-mod-data/horizon_studio/workspace/play.example.net 2026-11-02"
    );
    // Opened as the game opens a world: saves/<folder>.
    assert_eq!(
        std::fs::canonicalize(saves.join(&folder)).unwrap(),
        std::fs::canonicalize(&world).unwrap()
    );
    // Without its marker and level.dat it's no world of saveinstance's.
    assert_eq!(worlds::listed(&saves, &ws, "play.example.net 2026-11-02"), None);
    std::fs::write(world.join(MARKER), b"{}").unwrap();
    assert_eq!(worlds::listed(&saves, &ws, "play.example.net 2026-11-02"), None);
    std::fs::write(world.join("level.dat"), b"x").unwrap();
    assert_eq!(
        worlds::listed(&saves, &ws, "play.example.net 2026-11-02").as_deref(),
        Some(folder.as_str())
    );
    // The world list's own worlds and other paths are left to the game.
    assert_eq!(worlds::resolve_in(&saves, &ws, "Survival"), None);
    assert_eq!(worlds::resolve_in(&saves, &ws, "../elsewhere/x"), None);
    assert_eq!(worlds::resolve_in(&saves, &ws, &folder), Some(world.clone()));
    // Without a workspace named, it is the one beside the saves.
    assert_eq!(worlds::workspace_beside(&saves), ws);
    // A world list read inside the hook is the game's own.
    let outer = worlds::listing().expect("not listing yet");
    assert!(worlds::listing().is_none());
    drop(outer);
    assert!(worlds::listing().is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn folder_names_are_one_plain_folder() {
    let server = |address: &str, name: &str| ClientInfo {
        server: Some(ServerInfo {
            address: address.into(),
            name: name.into(),
            brand: String::new(),
            own_world: address.is_empty(),
        }),
        ..ClientInfo::default()
    };
    assert_eq!(
        folder_name(None, &server("hypixel.net", "hypixel.net"), "2026-11-02"),
        "hypixel.net 2026-11-02"
    );
    assert_eq!(
        folder_name(None, &server("10.0.0.2:25570", "x"), "2026-11-02"),
        "10.0.0.2 25570 2026-11-02"
    );
    assert_eq!(
        folder_name(None, &server("", "New World"), "2026-11-02"),
        "New World 2026-11-02"
    );
    assert_eq!(
        folder_name(None, &ClientInfo::default(), "2026-11-02"),
        "world 2026-11-02"
    );
    assert_eq!(folder_name(Some("dex.txt"), &ClientInfo::default(), ""), "dex");
    assert_eq!(folder_name(Some("Place.rbxlx"), &ClientInfo::default(), ""), "Place");
    assert_eq!(folder_name(Some("  "), &server("a.b", "a.b"), "d"), "a.b d");
    assert_eq!(sanitize("../x/y\\z:*?"), "_x_y_z___");
    assert_eq!(sanitize("..."), "world");
    assert_eq!(sanitize("con"), "_con");
    assert_eq!(sanitize("COM1.txt"), "_COM1.txt");
    assert_eq!(sanitize(&"a".repeat(200)).len(), 80);
    assert_eq!(date_of(0), "1970-01-01");
    assert_eq!(date_of(20_759), "2026-11-02");
    assert_eq!(date_of(11_016), "2000-02-29");
}

#[test]
fn block_boxes_cover_the_blocks_a_part_fills() {
    assert_eq!(
        block_box([10.0, 64.0, -3.0], [12.0, 65.0, -2.0]),
        ([10, 64, -3], [11, 64, -3])
    );
    // A part a hair over a block's edge doesn't take in the next block.
    assert_eq!(
        block_box([0.00001, 0.0, 0.0], [0.99999, 1.0, 1.0]),
        ([0, 0, 0], [0, 0, 0])
    );
    // A part smaller than a block is the block it is in.
    assert_eq!(block_box([5.2, 5.2, 5.2], [5.4, 5.4, 5.4]), ([5, 5, 5], [5, 5, 5]));
}

// ---------------------------------------------------------------- a small NBT reader, for the files

#[derive(Debug, PartialEq, Clone)]
enum Nbt {
    Byte(i8),
    Int(i32),
    Str(String),
    List(Vec<Nbt>),
    Compound(BTreeMap<String, Nbt>),
}

impl Nbt {
    fn get(&self, k: &str) -> &Nbt {
        match self {
            Nbt::Compound(m) => m.get(k).unwrap_or_else(|| panic!("no {k} in {m:?}")),
            _ => panic!("not a compound"),
        }
    }
    fn list(&self) -> &[Nbt] {
        match self {
            Nbt::List(l) => l,
            other => panic!("not a list: {other:?}"),
        }
    }
    fn int(&self) -> i32 {
        match self {
            Nbt::Int(i) => *i,
            other => panic!("not an int: {other:?}"),
        }
    }
    fn str(&self) -> &str {
        match self {
            Nbt::Str(s) => s,
            other => panic!("not a string: {other:?}"),
        }
    }
    fn ints(&self) -> Vec<i32> {
        self.list().iter().map(Nbt::int).collect()
    }
}

fn read_nbt(bytes: &[u8]) -> (String, Nbt) {
    fn s(b: &[u8], i: &mut usize) -> String {
        let n = u16::from_be_bytes([b[*i], b[*i + 1]]) as usize;
        *i += 2;
        let t = String::from_utf8(b[*i..*i + n].to_vec()).unwrap();
        *i += n;
        t
    }
    fn payload(kind: u8, b: &[u8], i: &mut usize) -> Nbt {
        match kind {
            1 => {
                *i += 1;
                Nbt::Byte(b[*i - 1] as i8)
            }
            3 => {
                *i += 4;
                Nbt::Int(i32::from_be_bytes(b[*i - 4..*i].try_into().unwrap()))
            }
            8 => Nbt::Str(s(b, i)),
            9 => {
                let k = b[*i];
                *i += 1;
                let n = i32::from_be_bytes(b[*i..*i + 4].try_into().unwrap());
                *i += 4;
                Nbt::List((0..n).map(|_| payload(k, b, i)).collect())
            }
            10 => {
                let mut m = BTreeMap::new();
                loop {
                    let k = b[*i];
                    *i += 1;
                    if k == 0 {
                        break;
                    }
                    let name = s(b, i);
                    m.insert(name, payload(k, b, i));
                }
                Nbt::Compound(m)
            }
            k => panic!("tag {k} not read here"),
        }
    }
    let mut i = 1;
    assert_eq!(bytes[0], 10, "a compound at the root");
    let name = s(bytes, &mut i);
    let root = payload(10, bytes, &mut i);
    assert_eq!(i, bytes.len(), "nothing after the root");
    (name, root)
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}

#[test]
fn a_structure_s_bytes_are_vanilla_s() {
    let region = Region {
        data_version: 5023,
        blocks: vec![RegionBlock {
            pos: [0, 0, 0],
            name: "minecraft:stone".into(),
            properties: Vec::new(),
            nbt: None,
        }],
    };
    let b = structure_nbt([1, 1, 1], &region);
    let mut want: Vec<u8> = vec![10, 0, 0];
    want.extend([3, 0, 11]);
    want.extend(b"DataVersion");
    want.extend(5023i32.to_be_bytes());
    want.extend([9, 0, 4]);
    want.extend(b"size");
    want.extend([3, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1]);
    want.extend([9, 0, 7]);
    want.extend(b"palette");
    want.extend([10, 0, 0, 0, 1]);
    want.extend([8, 0, 4]);
    want.extend(b"Name");
    want.extend([0, 15]);
    want.extend(b"minecraft:stone");
    want.push(0);
    want.extend([9, 0, 6]);
    want.extend(b"blocks");
    want.extend([10, 0, 0, 0, 1]);
    want.extend([9, 0, 3]);
    want.extend(b"pos");
    want.extend([3, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    want.extend([3, 0, 5]);
    want.extend(b"state");
    want.extend(0i32.to_be_bytes());
    want.push(0);
    want.extend([9, 0, 8]);
    want.extend(b"entities");
    want.extend([0, 0, 0, 0, 0]);
    want.push(0);
    assert_eq!(b, want);

    // Properties, a block entity's own compound, and a shared palette.
    let mut sign = Vec::new();
    sign.extend([8, 0, 2]);
    sign.extend(b"id");
    sign.extend([0, 14]);
    sign.extend(b"minecraft:sign");
    sign.push(0);
    let facing = vec![("facing".to_string(), "north".to_string())];
    let region = Region {
        data_version: 1,
        blocks: vec![
            RegionBlock {
                pos: [0, 0, 0],
                name: "minecraft:oak_sign".into(),
                properties: facing.clone(),
                nbt: Some(sign),
            },
            RegionBlock {
                pos: [1, 0, 0],
                name: "minecraft:oak_sign".into(),
                properties: facing,
                nbt: None,
            },
            RegionBlock {
                pos: [2, 0, 0],
                name: "minecraft:structure_void".into(),
                properties: Vec::new(),
                nbt: None,
            },
        ],
    };
    let (_, root) = read_nbt(&structure_nbt([3, 1, 1], &region));
    let palette = root.get("palette").list();
    assert_eq!(palette.len(), 1);
    assert_eq!(palette[0].get("Properties").get("facing").str(), "north");
    let blocks = root.get("blocks").list();
    assert_eq!(blocks.len(), 2, "the structure void left out");
    assert_eq!(blocks[0].get("nbt").get("id").str(), "minecraft:sign");
    assert_eq!(blocks[1].get("state").int(), 0);
}

#[test]
fn saveinstance_of_a_part_or_a_model_writes_their_blocks_as_a_structure() {
    let mut game = FakeGame::new("save-structure");
    game.blocks.insert((10, 64, -3), "minecraft:stone".into());
    game.blocks.insert((11, 64, -3), "minecraft:oak_planks".into());
    game.blocks.insert((20, 70, 5), "minecraft:glass".into());
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local part = Instance.new("Part")
        part.Name = "Floor"
        part.Size = Vector3.new(2, 1, 1)
        part.Position = Vector3.new(11, 64.5, -2.5)
        part.Anchored = true
        part.Parent = workspace
        local model = Instance.new("Model")
        local a = Instance.new("Part")
        a.Size = Vector3.new(1, 1, 1)
        a.Position = Vector3.new(20.5, 70.5, 5.5)
        a.Anchored = true
        a.Parent = model
        local b = a:Clone()
        b.Position = Vector3.new(21.5, 71.5, 5.5)
        b.Parent = model
        model.Name = "House"
        model.Parent = workspace
        print(saveinstance(part), saveinstance(part, "dex.txt"), SaveInstance(model, "house"))
        print(saveinstance({ Object = model, FileName = "opts" }))
        print(pcall(saveinstance, Instance.new("Folder")))
        "#,
    );
    run_for(&mut st, &mut game, 0.5);
    let p = printed(&st);
    assert_eq!(p[0], "Floor.nbt\tdex.nbt\thouse.nbt", "{:?}", log(&st));
    assert_eq!(p[1], "opts.nbt");
    assert!(
        p[2].starts_with("false\tsaveinstance: Folder (Folder) has no parts"),
        "{p:?}"
    );
    assert!(game.dumps.is_empty(), "a part's save is no world dump");
    let ws = st.s.rbx.workspace_dir.clone();
    let (name, floor) = read_nbt(&gunzip(&std::fs::read(ws.join("dex.nbt")).unwrap()));
    assert_eq!(name, "");
    assert_eq!(floor.get("DataVersion").int(), dump::DATA_VERSION);
    assert_eq!(floor.get("size").ints(), [2, 1, 1]);
    let names: Vec<&str> = floor
        .get("palette")
        .list()
        .iter()
        .map(|p| p.get("Name").str())
        .collect();
    assert_eq!(names, ["minecraft:stone", "minecraft:oak_planks"]);
    let blocks: Vec<(Vec<i32>, i32)> = floor
        .get("blocks")
        .list()
        .iter()
        .map(|b| (b.get("pos").ints(), b.get("state").int()))
        .collect();
    assert_eq!(blocks, [(vec![0, 0, 0], 0), (vec![1, 0, 0], 1)]);
    let (_, house) = read_nbt(&gunzip(&std::fs::read(ws.join("house.nbt")).unwrap()));
    assert_eq!(house.get("size").ints(), [2, 2, 1]);
    let glass = house
        .get("palette")
        .list()
        .iter()
        .position(|p| p.get("Name").str() == "minecraft:glass")
        .unwrap() as i32;
    let at: Vec<Vec<i32>> = house
        .get("blocks")
        .list()
        .iter()
        .filter(|b| b.get("state").int() == glass)
        .map(|b| b.get("pos").ints())
        .collect();
    assert_eq!(at, [vec![0, 0, 0]]);
    assert!(!ws.join("dex.txt").exists() && !ws.join("dex.nbt.txt").exists());
}
