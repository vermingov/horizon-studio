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

/// A studio folder of its own with a mods folder in it, gone at the end.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let dir = std::env::temp_dir().join(format!("hstudio-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
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
