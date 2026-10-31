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
