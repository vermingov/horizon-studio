// horizon-studio-mcp and modules: write_mod writing some files of a module
// and removing others, turning a script into a module, and list_mods and
// check_mod saying each mod's kind, its files and the file an error is in.
// The tools try the game's AI link: these tests point it at a file that
// doesn't exist (once, before any of them reads the environment), so no
// running game is reached, and they have this binary to themselves so no
// other test changes that variable meanwhile.

use horizon_studio::mcp::Server;
use serde_json::{Value as Json, json};
use std::path::PathBuf;

fn server() -> Server {
    static ONCE: std::sync::Once = std::sync::Once::new();
    // SAFETY: set once, before any test here reads the environment: each
    // calls this first, and call_once holds the others until it is done.
    ONCE.call_once(|| unsafe {
        let nowhere = std::env::temp_dir().join(format!("hstudio-no-link-{}", std::process::id()));
        std::env::set_var("HORIZON_STUDIO_LINK", nowhere.join("link.json"));
    });
    Server::new()
}

/// A tool's result as JSON (its text parsed), and whether it was an error.
fn call(server: &mut Server, name: &str, args: Json) -> (Json, bool) {
    let r = server.call_tool(name, &args);
    let text = r["content"][0]["text"].as_str().unwrap_or("").to_owned();
    let error = r["isError"] == true;
    (serde_json::from_str(&text).unwrap_or(Json::String(text)), error)
}

/// A studio folder of its own with a mods folder in it (and the studio.txt
/// the game writes there), gone at the end.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let dir = std::env::temp_dir().join(format!("hstudio-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::write(dir.join("studio.txt"), "seen:uikit\n").unwrap();
        Folder(dir)
    }
    fn mods(&self) -> PathBuf {
        self.0.join("mods")
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn write_mod_changes_some_files_of_a_module_and_removes_others() {
    let mut s = server();
    let studio = Folder::new("partial");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    let main = "local util = require('util')\ncommand('kit', function() return util.two() end)\n";
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": {
            "main.lua": main,
            "util.lua": "return { two = function() return 2 end }\n",
            "old.lua": "return 'old'\n",
        } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["kind"], "module");
    // Only util.lua: main.lua stays, and the check sees it require util.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "util.lua": "return { two = function() return 'two' end }\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["state"], "running", "{r:#}");
    assert_eq!(r["check"]["mod"]["files"], json!(["main.lua", "old.lua", "util.lua"]));
    assert_eq!(std::fs::read_to_string(mods.join("kit/main.lua")).unwrap(), main);
    assert!(
        std::fs::read_to_string(mods.join("kit/util.lua"))
            .unwrap()
            .contains("'two'")
    );
    // remove deletes files, checked without them first.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "remove": ["old.lua"] }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["removed"], json!(["old.lua"]));
    assert_eq!(r["check"]["mod"]["files"], json!(["main.lua", "util.lua"]));
    assert!(!mods.join("kit/old.lua").exists());
    for (bad, why) in [
        ("main.lua", "main.lua starts the module"),
        ("nope.lua", "no file nope.lua"),
    ] {
        let (r, error) = call(
            &mut s,
            "write_mod",
            json!({ "id": "kit", "mods_dir": d, "remove": [bad] }),
        );
        assert!(error, "{bad}");
        assert!(r.as_str().unwrap_or("").contains(why), "{bad}: {r}");
    }
    assert!(mods.join("kit/main.lua").is_file());
    // A module's own file name as a whole script, and nothing at all: errors.
    let (_, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "kit.lua": "print(1)" } }),
    );
    assert!(error);
    let (_, error) = call(&mut s, "write_mod", json!({ "id": "kit", "mods_dir": d }));
    assert!(error);
    // main.lua for a script makes it a module; the old file goes to the trash.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "solo.lua": "print('one file')\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["kind"], "script");
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "main.lua": "print('a module now')\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["kind"], "module");
    assert!(mods.join("solo/main.lua").is_file());
    assert!(!mods.join("solo.lua").exists());
    assert!(studio.0.join("trash/solo.lua").is_file());
    assert_eq!(r["trashed"], json!(studio.0.join("trash/solo.lua")), "{r:#}");
    // A script can't lose files: remove is for modules.
    let (_, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "plain", "mods_dir": d, "files": { "plain.lua": "print(1)\n" } }),
    );
    assert!(!error);
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "plain", "mods_dir": d, "remove": ["plain.lua"] }),
    );
    assert!(error && r.as_str().unwrap_or("").contains("not a module"), "{r}");
}

#[test]
fn list_mods_and_check_mod_say_kind_files_and_the_error_file() {
    let mut s = server();
    let studio = Folder::new("kinds");
    let mods = studio.mods();
    std::fs::create_dir_all(mods.join("kit/lib")).unwrap();
    std::fs::write(mods.join("kit/main.lua"), "require('lib.util')\n").unwrap();
    std::fs::write(mods.join("kit/lib/util.lua"), "return 1\n").unwrap();
    std::fs::write(mods.join("plain.lua"), "print(1)\n").unwrap();
    let d = mods.to_str().unwrap();
    let (r, error) = call(&mut s, "list_mods", json!({ "mods_dir": d }));
    assert!(!error, "{r:#}");
    let list = r["mods"].as_array().unwrap();
    let kit = list.iter().find(|m| m["id"] == "kit").unwrap();
    assert_eq!(kit["kind"], "module");
    assert_eq!(kit["files"], json!(["main.lua", "lib/util.lua"]));
    let plain = list.iter().find(|m| m["id"] == "plain").unwrap();
    assert_eq!((&plain["kind"], &plain["files"]), (&json!("script"), &json!([])));
    // A module failing in lib.lua's line 2: that file and line, not main.lua's.
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "broken", "mods_dir": d, "files": {
            "main.lua": "local lib = require('lib')\n",
            "lib.lua": "local x = 1\nerror('lib fails')\n",
        } }),
    );
    assert!(error);
    assert_eq!(r["mod"]["state"], "failed");
    assert_eq!(r["mod"]["kind"], "module");
    assert_eq!(r["mod"]["files"], json!(["main.lua", "lib.lua"]));
    assert_eq!(r["mod"]["file"], "broken/lib.lua", "{r:#}");
    assert_eq!(r["mod"]["line"], 2, "{r:#}");
    // A syntax error there: the same, though the message starts at main.lua's require.
    let (r, _) = call(
        &mut s,
        "check_mod",
        json!({ "id": "broken", "mods_dir": d, "files": {
            "main.lua": "\nlocal lib = require('lib')\n",
            "lib.lua": "local x = = 1\n",
        } }),
    );
    assert!(
        r["mod"]["error"]
            .as_str()
            .unwrap()
            .starts_with("broken/main.lua:2: broken/lib.lua:1:"),
        "{r:#}"
    );
    assert_eq!(
        (&r["mod"]["file"], &r["mod"]["line"]),
        (&json!("broken/lib.lua"), &json!(1))
    );
    // check_mod with some files of a module on disk checks them over its others.
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "lib/util.lua": "return 2\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["files"], json!(["main.lua", "lib/util.lua"]));
}

#[test]
fn a_path_refused_changes_nothing_on_disk() {
    let mut s = server();
    let studio = Folder::new("refused");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    let (_, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "solo.lua": "print('one file')\n" } }),
    );
    assert!(!error);
    // A script made a module with a path that isn't one inside it, before
    // main.lua or after it: refused before anything moves.
    for files in [
        json!({ "../evil.lua": "return 1\n", "main.lua": "print('module')\n" }),
        json!({ "main.lua": "print('module')\n", "./lib.lua": "return 1\n" }),
        json!({ "main.lua": "print('module')\n", "a/b/c/d/e.lua": "return 1\n" }),
    ] {
        let (r, error) = call(
            &mut s,
            "write_mod",
            json!({ "id": "solo", "mods_dir": d, "files": files }),
        );
        assert!(error, "{files}: {r}");
        assert!(r.as_str().unwrap_or("").contains("is not a path inside the mod"), "{r}");
        assert_eq!(
            std::fs::read_to_string(mods.join("solo.lua")).unwrap(),
            "print('one file')\n"
        );
        assert!(!mods.join("solo").exists(), "{files}: no module, not even half of one");
        assert!(!studio.0.join("trash").exists(), "{files}: nothing in the trash");
    }
    let (r, _) = call(&mut s, "list_mods", json!({ "mods_dir": d }));
    assert_eq!(r["mods"][0]["id"], "solo", "{r:#}");
    // An existing module: a bad path among good ones writes none of them.
    let main = "print('kit')\n";
    let (_, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "main.lua": main, "lib.lua": "return 1\n" } }),
    );
    assert!(!error);
    std::fs::create_dir_all(mods.join("kit/data")).unwrap();
    for (bad, why) in [
        ("../x.lua", "is not a path inside the mod"),
        ("data", "is a folder in the mod"),
        ("lib.lua/x.lua", "would be inside lib.lua"),
    ] {
        let (r, error) = call(
            &mut s,
            "write_mod",
            json!({ "id": "kit", "mods_dir": d, "files": {
                "main.lua": "print('changed')\n",
                bad: "return 2\n",
                "new.lua": "return 3\n",
            } }),
        );
        assert!(error && r.as_str().unwrap_or("").contains(why), "{bad}: {r}");
        assert_eq!(
            std::fs::read_to_string(mods.join("kit/main.lua")).unwrap(),
            main,
            "{bad}"
        );
        assert!(!mods.join("kit/new.lua").exists(), "{bad}");
    }
    // Written, a module leaves no hidden folder behind.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "main.lua": "print('module')\n", "lib/util.lua": "return 1\n" } }),
    );
    assert!(!error, "{r:#}");
    assert!(mods.join("solo/lib/util.lua").is_file() && !mods.join("solo.lua").exists());
    assert!(studio.0.join("trash/solo.lua").is_file());
    let hidden: Vec<String> = std::fs::read_dir(&mods)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(hidden.is_empty(), "{hidden:?}");
}

#[test]
fn removing_a_file_the_module_loads_fails_the_check() {
    let mut s = server();
    let studio = Folder::new("removed");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    let kit = mods.join("kit");
    std::fs::create_dir_all(kit.join("models")).unwrap();
    std::fs::write(
        kit.join("main.lua"),
        "local lib = require('lib')\nlocal box = mesh.load('models/box.obj')\n",
    )
    .unwrap();
    std::fs::write(kit.join("lib.lua"), "return {}\n").unwrap();
    std::fs::write(kit.join("models/box.obj"), "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "kit", "mods_dir": d }));
    assert!(!error, "{r:#}");
    // An asset main.lua loads: the check runs without it, and fails as the game will.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "remove": ["models/box.obj"] }),
    );
    assert!(error, "{r:#}");
    assert_eq!(r["removed"], json!(["models/box.obj"]));
    let why = r["check"]["mod"]["error"].as_str().unwrap_or("");
    assert!(why.contains("mesh.load: no file 'models/box.obj'"), "{r:#}");
    assert!(!kit.join("models").exists(), "removed, and its emptied folder");
    // A file it requires, removed beside a main.lua given: the same.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "main.lua": "local lib = require('lib')\n" }, "remove": ["lib.lua"] }),
    );
    assert!(error, "{r:#}");
    let why = r["check"]["mod"]["error"].as_str().unwrap_or("");
    assert!(why.contains("lib.lua") && why.contains("not found"), "{r:#}");
}

#[test]
fn files_given_are_checked_over_the_others_as_they_are() {
    let mut s = server();
    let studio = Folder::new("as-they-are");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    let kit = mods.join("kit");
    std::fs::create_dir_all(&kit).unwrap();
    std::fs::write(
        kit.join("main.lua"),
        "local lib = require('lib')\nlocal data = require('data')\nprint(lib.name, #data)\n",
    )
    .unwrap();
    // A file saved in Latin-1 (not UTF-8), and one over 1 MiB: the game runs both.
    std::fs::write(kit.join("lib.lua"), b"-- caf\xe9 au lait\nreturn { name = 'lib' }\n").unwrap();
    let big = format!("-- {}\nreturn {{ 1, 2, 3 }}\n", "x".repeat(1_200_000));
    std::fs::write(kit.join("data.lua"), big).unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "kit", "mods_dir": d }));
    assert!(!error, "as it is on disk: {r:#}");
    let main = "local lib = require('lib')\nlocal data = require('data')\ncommand('kit', function() return 'changed ' .. lib.name .. ' ' .. #data end)\n";
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "main.lua": main } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["state"], "running", "{r:#}");
    assert_eq!(r["mod"]["files"], json!(["main.lua", "data.lua", "lib.lua"]));
    let (r, error) = call(
        &mut s,
        "test_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "main.lua": main },
                "steps": [{ "command": "/kit", "expect_chat": "changed lib 3" }] }),
    );
    assert!(!error && r["ok"] == true, "{r:#}");
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "main.lua": main } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["check"]["mod"]["state"], "running", "{r:#}");
    assert_eq!(std::fs::read(kit.join("lib.lua")).unwrap()[..8], *b"-- caf\xe9 ");
}

#[test]
fn a_mod_on_disk_is_run_as_it_is_whatever_its_encoding_or_size() {
    let mut s = server();
    let studio = Folder::new("on-disk");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    // A module whose main.lua is saved in Latin-1 (not UTF-8): the game runs it.
    let kit = mods.join("kit");
    std::fs::create_dir_all(&kit).unwrap();
    std::fs::write(
        kit.join("main.lua"),
        b"-- caf\xe9\nlocal lib = require('lib')\ncommand('kit', function() return lib.word end)\n",
    )
    .unwrap();
    std::fs::write(kit.join("lib.lua"), "return { word = 'kit runs' }\n").unwrap();
    std::fs::write(kit.join("shared.lua"), "return {}\n").unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "kit", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["state"], "running", "{r:#}");
    let steps = json!([{ "command": "/kit", "expect_chat": "kit runs" }]);
    let (r, error) = call(
        &mut s,
        "test_mod",
        json!({ "id": "kit", "mods_dir": d, "steps": steps }),
    );
    assert!(!error && r["ok"] == true, "{r:#}");
    // With only another file of its: two files besides main.lua, then one.
    std::fs::remove_file(kit.join("shared.lua")).unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "kit", "mods_dir": d }));
    assert!(!error, "{r:#}");
    // A script in Latin-1: listed, checked and loaded with run_lua.
    std::fs::write(
        mods.join("lamp.lua"),
        b"-- caf\xe9\ncommand('lamp', function() return 'on' end)\n",
    )
    .unwrap();
    let (r, _) = call(&mut s, "list_mods", json!({ "mods_dir": d }));
    assert!(r["mods"].as_array().unwrap().iter().any(|m| m["id"] == "lamp"), "{r:#}");
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "lamp", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["commands"], json!(["lamp"]), "{r:#}");
    let (r, error) = call(
        &mut s,
        "run_lua",
        json!({ "code": "print('with lamp')", "with_mod": "lamp", "live": false, "mods_dir": d }),
    );
    assert!(!error, "{r:#}");
    // read_mod gives text: it says why it can't, not that there is no mod.
    let (r, error) = call(&mut s, "read_mod", json!({ "id": "lamp", "mods_dir": d }));
    assert!(
        error && r.as_str().unwrap_or("").contains("lamp.lua is not UTF-8 text"),
        "{r}"
    );
    // A main.lua over 1 MiB.
    let big = mods.join("big");
    std::fs::create_dir_all(&big).unwrap();
    let main = format!("-- {}\nlocal lib = require('lib')\n", "x".repeat(1_200_000));
    std::fs::write(big.join("main.lua"), main).unwrap();
    std::fs::write(big.join("lib.lua"), "return {}\n").unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "big", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["state"], "running", "{r:#}");
}

#[test]
fn a_check_runs_the_mod_the_studio_lists() {
    let mut s = server();
    let studio = Folder::new("listed");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    // "Night Light.lua" holds night_light; a night_light/ module beside it
    // is passed over, by the studio and so by the check.
    std::fs::write(mods.join("Night Light.lua"), "print('the script the game runs')\n").unwrap();
    std::fs::create_dir_all(mods.join("night_light")).unwrap();
    std::fs::write(mods.join("night_light/main.lua"), "print('the module passed over')\n").unwrap();
    std::fs::write(mods.join("night_light/old.lua"), "return 'passed over too'\n").unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "night_light", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["kind"], "script", "{r:#}");
    let log = r["log"].to_string();
    assert!(
        log.contains("the script the game runs") && !log.contains("passed over"),
        "{r:#}"
    );
    // Its files given are checked alone, as a script's: not over that module's.
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "night_light", "mods_dir": d, "files": { "main.lua": "print('a module now')\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["files"], json!(["main.lua"]), "{r:#}");
    // A library in a folder named otherwise ("UI Bits/" is ui_bits) comes
    // with its assets, as the game has it.
    let lib = mods.join("UI Bits");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("main.lua"),
        "local box = mesh.load('box.obj')\nexport({ size = function() return 3 end })\n",
    )
    .unwrap();
    let obj = format!("# {}\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", "x".repeat(1_100_000));
    std::fs::write(lib.join("box.obj"), obj).unwrap();
    std::fs::write(
        mods.join("user.lua"),
        "local ui = import('ui_bits')\ncommand('size', function() return 'size ' .. ui.size() end)\n",
    )
    .unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "user", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["libraries"], json!(["ui_bits"]), "{r:#}");
    let steps = json!([{ "command": "/size", "expect_chat": "size 3" }]);
    let (r, error) = call(
        &mut s,
        "test_mod",
        json!({ "id": "user", "mods_dir": d, "steps": steps }),
    );
    assert!(!error && r["ok"] == true, "{r:#}");
}

#[test]
fn only_main_lua_for_a_script_moves_a_file_to_the_trash() {
    let mut s = server();
    let studio = Folder::new("stray");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    // A module with a file of its name beside it, which the module shadows.
    std::fs::create_dir_all(mods.join("kit")).unwrap();
    std::fs::write(mods.join("kit/main.lua"), "print('kit')\n").unwrap();
    std::fs::write(mods.join("kit.lua"), "-- an older copy, kept by hand\n").unwrap();
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "lib.lua": "return 1\n" } }),
    );
    assert!(!error, "{r:#}");
    assert!(mods.join("kit/lib.lua").is_file());
    assert!(
        mods.join("kit.lua").is_file(),
        "a write of lib.lua leaves kit.lua alone"
    );
    assert!(!studio.0.join("trash").exists());
    // `remove` beside an empty `files`: as `remove` alone.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": {}, "remove": ["lib.lua"] }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["removed"], json!(["lib.lua"]));
    assert!(!mods.join("kit/lib.lua").exists());
    let (r, error) = call(&mut s, "write_mod", json!({ "id": "kit", "mods_dir": d, "files": {} }));
    assert!(
        error && r.as_str().unwrap_or("").contains("give 'files' to write"),
        "{r}"
    );
    // check_mod still wants files to check, when it is given them.
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "kit", "mods_dir": d, "files": {} }));
    assert!(error && r.as_str().unwrap_or("").contains("files is empty"), "{r}");
}

#[test]
fn an_id_another_file_of_the_mods_folder_holds_is_refused() {
    let mut s = server();
    let studio = Folder::new("held");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    // "Night Light.lua" is the mod night_light, as the studio lists it.
    std::fs::write(mods.join("Night Light.lua"), "print('the one the game runs')\n").unwrap();
    let (r, error) = call(&mut s, "list_mods", json!({ "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mods"].as_array().map(Vec::len), Some(1), "{r:#}");
    assert_eq!(
        (&r["mods"][0]["id"], &r["mods"][0]["kind"]),
        (&json!("night_light"), &json!("script"))
    );
    assert!(
        r["mods"][0]["main"].as_str().unwrap_or("").ends_with("Night Light.lua"),
        "{r:#}"
    );
    let (r, error) = call(&mut s, "read_mod", json!({ "id": "night_light", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["files"]["night_light.lua"], "print('the one the game runs')\n");
    // A module or a script written under that id would never load: refused,
    // naming the file that has it, and nothing is written.
    for files in [
        json!({ "main.lua": "print('a module')\n" }),
        json!({ "night_light.lua": "print('a script')\n" }),
    ] {
        let (r, error) = call(
            &mut s,
            "write_mod",
            json!({ "id": "night_light", "mods_dir": d, "files": files }),
        );
        assert!(error, "{files}: {r:#}");
        assert!(
            r.as_str()
                .unwrap_or("")
                .contains("night_light is \"Night Light.lua\" in the mods folder"),
            "{r}"
        );
        assert!(!mods.join("night_light").exists() && !mods.join("night_light.lua").exists());
    }
    // A module whose folder's name gives its id: checked by that id, with
    // its assets (one over 1 MiB, which isn't read as text).
    let folder = mods.join("Desk Lamp");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("main.lua"),
        "local lib = require('lib')\nlocal box = mesh.load('box.obj')\ncommand('lamp', function() return lib.word end)\n",
    )
    .unwrap();
    std::fs::write(folder.join("lib.lua"), "return { word = 'on' }\n").unwrap();
    let obj = format!("# {}\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", "x".repeat(1_100_000));
    std::fs::write(folder.join("box.obj"), obj).unwrap();
    let (r, error) = call(&mut s, "check_mod", json!({ "id": "desk_lamp", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["state"], "running", "{r:#}");
    // Some of its files, checked and tested over its others in that folder.
    let main = "local lib = require('lib')\nlocal box = mesh.load('box.obj')\ncommand('lamp', function() return 'lamp ' .. lib.word end)\n";
    let (r, error) = call(
        &mut s,
        "check_mod",
        json!({ "id": "desk_lamp", "mods_dir": d, "files": { "main.lua": main } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(r["mod"]["files"], json!(["main.lua", "lib.lua"]), "{r:#}");
    let (r, error) = call(
        &mut s,
        "test_mod",
        json!({ "id": "desk_lamp", "mods_dir": d, "files": { "main.lua": main },
                "steps": [{ "command": "/lamp", "expect_chat": "lamp on" }] }),
    );
    assert!(!error && r["ok"] == true, "{r:#}");
    // `remove` is refused there as writing its files is: neither changes a
    // module whose folder is named otherwise.
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "desk_lamp", "mods_dir": d, "remove": ["lib.lua"] }),
    );
    assert!(error, "{r:#}");
    assert!(
        r.as_str()
            .unwrap_or("")
            .contains("desk_lamp is \"Desk Lamp\" in the mods folder"),
        "{r}"
    );
    assert!(folder.join("lib.lua").is_file());
    // A mod's own file is written as before.
    for text in ["print('lamp')\n", "print('lamp, again')\n"] {
        let (r, error) = call(
            &mut s,
            "write_mod",
            json!({ "id": "lamp", "mods_dir": d, "files": { "lamp.lua": text } }),
        );
        assert!(!error, "{r:#}");
    }
}

#[test]
fn delete_mod_deletes_what_list_mods_lists_under_the_id() {
    let mut s = server();
    let studio = Folder::new("delete");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    let night = mods.join("Night Light.lua");
    std::fs::write(&night, "print('night')\n").unwrap();
    let lamp = mods.join("Desk Lamp");
    std::fs::create_dir_all(&lamp).unwrap();
    std::fs::write(lamp.join("main.lua"), "print('lamp')\n").unwrap();
    // The ids list_mods gives, whatever the names on disk.
    for (id, path) in [("desk_lamp", &lamp), ("night_light", &night)] {
        let (r, error) = call(&mut s, "delete_mod", json!({ "id": id, "mods_dir": d }));
        assert!(!error, "{id}: {r:#}");
        assert_eq!((&r["deleted"], &r["path"]), (&json!(id), &json!(path)), "{r:#}");
        assert!(r.get("now_listed").is_none(), "{r:#}");
        assert!(!path.exists(), "{id}");
    }
    let (r, _) = call(&mut s, "list_mods", json!({ "mods_dir": d }));
    assert_eq!(r["mods"], json!([]), "{r:#}");
    // Of two names with one id, the one listed goes, not the one passed
    // over, and the reply says the other is listed now.
    std::fs::write(&night, "print('night')\n").unwrap();
    std::fs::create_dir_all(mods.join("night_light")).unwrap();
    std::fs::write(mods.join("night_light/main.lua"), "print('passed over')\n").unwrap();
    let (r, error) = call(&mut s, "delete_mod", json!({ "id": "night_light", "mods_dir": d }));
    assert!(!error, "{r:#}");
    assert_eq!(r["path"], json!(night), "{r:#}");
    assert!(!night.exists() && mods.join("night_light/main.lua").is_file());
    assert_eq!(r["now_listed"], json!(mods.join("night_light")), "{r:#}");
    let (r, error) = call(&mut s, "delete_mod", json!({ "id": "nope", "mods_dir": d }));
    assert!(error && r.as_str().unwrap_or("").contains("no mod 'nope'"), "{r}");
}

#[test]
fn where_case_is_ignored_a_mods_own_name_is_its_own() {
    let mut s = server();
    let studio = Folder::new("case");
    let mods = studio.mods();
    let d = mods.to_str().unwrap();
    // "Lamp.lua" is lamp. Where the file system ignores case (macOS's,
    // Windows') lamp.lua is that very file; a hard link, a second name for
    // one file, is the same on any other.
    std::fs::write(mods.join("Lamp.lua"), "print('lamp')\n").unwrap();
    if !mods.join("lamp.lua").exists() {
        std::fs::hard_link(mods.join("Lamp.lua"), mods.join("lamp.lua")).unwrap();
    }
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "lamp", "mods_dir": d, "files": { "lamp.lua": "print('lamp, again')\n" } }),
    );
    assert!(!error, "{r:#}");
    assert_eq!(
        std::fs::read_to_string(mods.join("Lamp.lua")).unwrap(),
        "print('lamp, again')\n"
    );
    // A module's folder the same way: "Kit" is kit/.
    std::fs::create_dir_all(mods.join("Kit")).unwrap();
    std::fs::write(mods.join("Kit/main.lua"), "print('kit')\n").unwrap();
    #[cfg(unix)]
    if !mods.join("kit").exists() {
        std::os::unix::fs::symlink("Kit", mods.join("kit")).unwrap();
    }
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "files": { "lib.lua": "return 1\n" } }),
    );
    assert!(!error, "{r:#}");
    assert!(mods.join("Kit/lib.lua").is_file());
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "kit", "mods_dir": d, "remove": ["lib.lua"] }),
    );
    assert!(!error, "{r:#}");
    assert!(!mods.join("Kit/lib.lua").exists());
}

#[test]
fn a_script_made_a_module_outside_a_studio_folder_is_deleted() {
    let mut s = server();
    // A mods folder in a project: its parent is no studio's folder.
    let project = std::env::temp_dir().join(format!("hstudio-mcp-project-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    let mods = project.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    let d = mods.to_str().unwrap();
    let (_, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "solo.lua": "print('one file')\n" } }),
    );
    assert!(!error);
    let (r, error) = call(
        &mut s,
        "write_mod",
        json!({ "id": "solo", "mods_dir": d, "files": { "main.lua": "print('a module now')\n" } }),
    );
    assert!(!error, "{r:#}");
    assert!(mods.join("solo/main.lua").is_file() && !mods.join("solo.lua").exists());
    // Deleted for good, as delete_mod and remove delete, and the reply says
    // so: no trash folder appears beside the project's mods folder.
    assert!(!project.join("trash").exists());
    assert_eq!(r["deleted"], "solo.lua", "{r:#}");
    assert!(r.get("trashed").is_none(), "{r:#}");
    let _ = std::fs::remove_dir_all(&project);
}
