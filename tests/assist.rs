// Suggestions and the live check (hstudio_assist).

use horizon_studio::game::hstudio_assist::{Kind, STDLIB, Siblings, check, suggest};

fn labels(before: &str, lines: &[&str], console: bool) -> Vec<String> {
    let lines: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
    suggest(before, &lines, console, None)
        .map(|(_, s)| s.into_iter().map(|x| x.label).collect())
        .unwrap_or_default()
}

#[test]
fn suggestions_fit_what_is_typed() {
    assert_eq!(labels("play", &[], false)[0], "player");
    let members = labels("player.po", &[], false);
    assert_eq!(&members[..2], ["pos", "pose"], "{members:?}");
    let (start, s) = suggest("x = player.po", &[], false, None).unwrap();
    assert_eq!(start, "x = player.".len());
    assert!(s[0].detail.starts_with("player.pos("), "{:?}", s[0]);
    assert!(!s[0].doc.is_empty());
    assert_eq!(s[0].kind, Kind::Function);
    assert_eq!(labels("name:up", &[], false)[0], "upper");
    assert_eq!(labels("math.ra", &[], false)[..2], ["rad", "random"]);
    // Events inside on("...").
    let (start, s) = suggest("on(\"ti", &[], false, None).unwrap();
    assert_eq!((start, s[0].label.as_str(), s[0].kind), (4, "tick", Kind::Event));
    assert_eq!(labels("on('w", &[], false), ["world"]);
    // The file's own names; keywords in the editor, the console's functions in the console.
    assert!(labels("myV", &["local myVariable = 1"], false).contains(&"myVariable".to_owned()));
    assert!(labels("fun", &[], false).contains(&"function".to_owned()));
    assert!(labels("hel", &[], true).contains(&"help".to_owned()));
    assert!(!labels("hel", &[], false).contains(&"help".to_owned()));
    // Letters in order, when nothing starts so.
    assert!(labels("plpos", &[], false).is_empty() || !labels("tostr", &[], false).is_empty());
    // Nothing in comments and strings, or after a number.
    assert!(labels("-- play", &[], false).is_empty());
    assert!(labels("print(\"play", &[], false).is_empty());
    assert!(labels("x = 12", &[], false).is_empty());
}

#[test]
fn a_letter_of_more_than_one_byte_is_never_cut_in_two() {
    // Before the word typed, or before what a '.' follows: the word, and
    // its owner, start after the whole letter.
    for console in [false, true] {
        for before in [
            "local nú",
            "local número",
            "grö",
            "größe",
            "x = é.a",
            "é.",
            "ñ:up",
            "a.ü.b",
        ] {
            let found = suggest(before, &[], console, None);
            if let Some((start, _)) = found {
                assert!(before.is_char_boundary(start), "{before:?}: {start}");
            }
        }
    }
    assert_eq!(labels("épla", &[], false)[0], "player");
    let (start, s) = suggest("ö = player.po", &[], false, None).unwrap();
    assert_eq!((start, s[0].label.as_str()), ("ö = player.".len(), "pos"));
}

/// A problem as (line, span, message, error).
type Found = (usize, Option<(usize, usize)>, String, bool);

fn problems(src: &str) -> Vec<Found> {
    problems_in(src, None)
}

/// The problems of `src` as a file of a module (`module`), or of a script.
fn problems_in(src: &str, module: Option<&Siblings>) -> Vec<Found> {
    let lines: Vec<String> = src.lines().map(str::to_owned).collect();
    check(&lines, "m.lua", module)
        .into_iter()
        .map(|p| (p.line, p.span, p.message, p.error))
        .collect()
}

/// A module whose open file is `file`, with these other files.
fn module(file: &str, others: &[(&str, &str)]) -> Siblings {
    Siblings::new(
        file.to_owned(),
        others
            .iter()
            .map(|(f, src)| ((*f).to_owned(), src.lines().map(str::to_owned).collect()))
            .collect(),
    )
}

#[test]
fn the_live_check_finds_syntax_errors_and_typos() {
    assert!(problems("local x = 1\nprint(x)\n").is_empty());
    let p = problems("if x then\n  print(1)\n");
    assert_eq!(p.len(), 1);
    assert!(p[0].3 && p[0].2.contains("'end' expected"), "{p:?}");
    let p = problems("local x = = 2\n");
    assert_eq!((p[0].0, p[0].1), (1, Some((10, 11))), "{p:?}");
    // A member a library doesn't have, under its name.
    let p = problems("local x = player.poss()\n");
    assert_eq!(p, vec![(1, Some((17, 21)), "player has no 'poss'".to_owned(), false)]);
    // A misspelt library.
    let p = problems("plyer.pos()\n");
    assert_eq!(p[0].2, "nothing defines 'plyer' (did you mean player?)");
    // Nothing for what the file defines, for strings and comments, or real names.
    assert!(problems("local player = {}\nplayer.foo = 1\n").is_empty());
    assert!(problems("print(\"player.poss\") -- player.poss\n").is_empty());
    assert!(problems("local t = {}\nt.x = string.format('%d', math.rad(1))\nprint(t.x .. texture.size)\n").is_empty());
    assert!(problems("local function f(plyer) return plyer.x end\n").is_empty());
}

#[test]
fn the_standard_library_matches_the_reference() {
    let ours: Vec<&str> = STDLIB.iter().map(|(n, ..)| *n).collect();
    let documented: Vec<&str> = horizon_studio::mcp_docs::ENTRIES
        .iter()
        .filter(|e| e.topic == "stdlib")
        .map(|e| e.name)
        .collect();
    assert_eq!(ours, documented);
}

#[test]
fn game_has_its_roblox_members_too() {
    // The DataModel's services, LocalPlayer and the rest are no unknown names.
    let src = "local lp = game.LocalPlayer\nlocal ws = game.workspace\nlocal p = game.Players\nlocal w = game.Workspace\n\
               print(game.Name, game.PlaceId, game.Lighting.ClockTime)\ngame.DescendantAdded:Connect(print)\n";
    assert!(problems(src).is_empty(), "{:?}", problems(src));
    assert_eq!(problems("game.tiks()\n")[0].2, "game has no 'tiks'");
    assert_eq!(problems("local p = game.Plyers\n")[0].2, "game has no 'Plyers'");
    // After `game.`, with the studio's own functions.
    let got = labels("game.", &[], false);
    for want in [
        "LocalPlayer",
        "Players",
        "Workspace",
        "ReplicatedStorage",
        "run",
        "ticks",
    ] {
        assert!(got.contains(&want.to_owned()), "{want} in {got:?}");
    }
    assert!(!got.contains(&"GetService".to_owned()), "methods take ':': {got:?}");
    // The services that do nothing here come after the others, and still
    // complete when typed.
    let at = |n: &str| got.iter().position(|g| g == n);
    assert!(
        at("VirtualUser").is_none_or(|v| at("Workspace").is_some_and(|w| w < v)),
        "{got:?}"
    );
    assert_eq!(labels("game.VirtualU", &[], false)[0], "VirtualUser");
    assert_eq!(labels("game.LocalP", &[], false), ["LocalPlayer"]);
    assert!(labels("game.wor", &[], false).contains(&"workspace".to_owned()));
    let (start, s) = suggest("local me = game.Loc", &[], false, None).unwrap();
    assert_eq!(start, "local me = game.".len());
    assert_eq!(
        (s[0].insert.as_str(), s[0].detail.as_str()),
        ("LocalPlayer", "game.LocalPlayer")
    );
    assert!(s[0].doc.contains("game.Players.LocalPlayer"), "{:?}", s[0]);
    // Its methods after `game:`, from the ':'.
    let (start, s) = suggest("local rs = game:GetS", &[], false, None).unwrap();
    assert_eq!(start, "local rs = game".len());
    let get = s.iter().find(|x| x.label == "GetService").expect("GetService");
    assert_eq!((get.insert.as_str(), get.kind), (":GetService", Kind::Function));
}

#[test]
fn after_a_colon_the_receivers_own_methods() {
    // A library's functions: the ':' becomes the '.' they are called with.
    let (start, s) = suggest("x = player:po", &[], false, None).unwrap();
    assert_eq!(start, "x = player".len(), "from the ':'");
    assert_eq!(s[0].label, "pos");
    assert_eq!(s[0].insert, ".pos");
    assert!(!s.iter().any(|x| x.label == "upper"), "no string methods");
    // A table of the file's: its methods, not a string's.
    let file = [
        "local Car = {}",
        "function Car:drive(speed) end",
        "function Car.new() return setmetatable({}, Car) end",
        "Car.wheels = 4",
    ];
    let got = labels("Car:", &file, false);
    assert!(
        got.contains(&"drive".to_owned()) && got.contains(&"new".to_owned()),
        "{got:?}"
    );
    assert!(
        !got.contains(&"upper".to_owned()) && !got.contains(&"wheels".to_owned()),
        "{got:?}"
    );
    assert_eq!(labels("Car.wh", &file, false), ["wheels"]);
    // A constructor's fields, and methods called on it elsewhere.
    let file = [
        "local cfg = {",
        "  speed = 1, name = \"x\",",
        "  go = function(self) end,",
        "  nested = { inner = 2 },",
        "}",
        "cfg:jump()",
    ];
    let got = labels("cfg.", &file, false);
    for want in ["speed", "name", "go", "nested", "jump"] {
        assert!(got.contains(&want.to_owned()), "{want} in {got:?}");
    }
    assert!(!got.contains(&"inner".to_owned()), "{got:?}");
    assert_eq!(labels("cfg:j", &file, false), ["jump"]);
    // A string, by its literal or what the file sets the name to.
    assert_eq!(labels("(\"hi\"):up", &[], false)[0], "upper");
    let got = labels("s:", &["local s = \"hello\""], false);
    assert!(
        got.contains(&"upper".to_owned()) && got.contains(&"sub".to_owned()),
        "{got:?}"
    );
    // A name the file says nothing about may well be a string.
    assert_eq!(labels("name:up", &[], false)[0], "upper");
}

#[test]
fn names_from_sibling_files_are_not_unknown() {
    let main = "players.add('x')\nchat.shout('hi')\n";
    let warned = |p: &[Found]| p.iter().any(|f| f.2.contains("did you mean player"));
    assert!(warned(&problems_in(main, None)), "a script has no siblings");
    let m = module(
        "main.lua",
        &[("players.lua", "players = {}\nfunction chat.shout(s) end\nreturn true\n")],
    );
    let p = problems_in(main, Some(&m));
    assert!(!warned(&p), "{p:?}");
    assert!(!p.iter().any(|f| f.2.contains("chat has no")), "{p:?}");
    // Set through _G or the executor's getgenv(), in a function a table
    // holds, or after a long comment with a brace in it: the module's too.
    for lib in [
        "_G.players = {}\n",
        "getgenv().players = {}\n",
        "local M = {\n  start = function()\n    players = {}\n  end,\n}\nreturn M\n",
        "--[[ call lib.setup({\n  before anything ]]\nplayers = {}\n",
    ] {
        let p = problems_in(main, Some(&module("main.lua", &[("lib.lua", lib)])));
        assert!(!warned(&p), "{lib}: {p:?}");
    }
    // A sibling's locals are its own.
    let m = module("main.lua", &[("players.lua", "local players = {}\nreturn players\n")]);
    assert!(warned(&problems_in(main, Some(&m))));
    // Its names are suggested, saying where they come from.
    let m = module("main.lua", &[("shared.lua", "settings = { loud = true }\n")]);
    let lines: Vec<String> = vec!["sett".into()];
    let (_, s) = suggest("sett", &lines, false, Some(&m)).unwrap();
    let hit = s.iter().find(|x| x.label == "settings").expect("settings suggested");
    assert_eq!(hit.doc, "in shared.lua");
}

#[test]
fn a_siblings_members_and_parameters_hide_no_typos() {
    let main = "chat.sya('hi')\nplayer.poss()\nchat.shout('hi')\n";
    let warned = |p: &[Found], m: &str| p.iter().any(|f| f.2 == m);
    // A sibling's `function chat.shout`: chat.shout is known, chat.sya still a typo.
    let m = module(
        "main.lua",
        &[("lib.lua", "function chat.shout(s) chat.say(s) end\nreturn true\n")],
    );
    let p = problems_in(main, Some(&m));
    assert!(warned(&p, "chat has no 'sya'"), "{p:?}");
    assert!(!p.iter().any(|f| f.2.contains("'shout'")), "{p:?}");
    assert!(warned(&problems_in(main, None), "chat has no 'shout'"));
    // Its parameters and loop names are its own, though named like a
    // library, and a table's fields and a call's arguments are no globals,
    // one a line too; nor is what a long string or comment holds.
    for lib in [
        "local M = {}\nfunction M.describe(player) return player.name end\nreturn M\n",
        "M = {}\nM.find = function(chat, player) return chat end\n",
        "for chat, player in pairs({}) do end\nreturn true\n",
        "local cfg = {\n  player = nil,\n  chat = true,\n}\nreturn cfg\n",
        "return {\n  chat = 1,\n  player = nil,\n}\n",
        "local function f()\n  ui.button({\n    chat = 1,\n    player = 2,\n  })\nend\n",
        "local t = {\n  f = function() local x = if a then 1 else 2 end,\n  chat = 1, player = 2,\n}\n",
        "local doc = [[\nplayer = nil\nchat = nil\n]]\n",
        "--[==[\nplayer = 1\nchat = 2\n]==]\n",
    ] {
        let p = problems_in(main, Some(&module("main.lua", &[("lib.lua", lib)])));
        assert!(warned(&p, "player has no 'poss'"), "{lib}: {p:?}");
        assert!(warned(&p, "chat has no 'sya'"), "{lib}: {p:?}");
    }
    // `chat.x = ...` and `obj:m` defined there are known as those members.
    let m = module(
        "main.lua",
        &[(
            "lib.lua",
            "chat.loud = function(s) end\nfunction tbl:go() end\nif x then chat.n = 1 end\n",
        )],
    );
    let p = problems_in("chat.loud('x')\nprint(chat.n, tbl.go)\nchat.lod('x')\n", Some(&m));
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].2, "chat has no 'lod'");
    // A sibling that sets the library's name to a table of its own: the
    // file's uses of it are that table's, as when the file sets it.
    let m = module("main.lua", &[("lib.lua", "chat = { sya = print }\n")]);
    assert!(problems_in(main, Some(&m)).iter().all(|f| !f.2.starts_with("chat")));
}

#[test]
fn requires_of_missing_files_are_problems() {
    let m = module(
        "main.lua",
        &[("lib.lua", "return {}\n"), ("lib/util.lua", "return {}\n")],
    );
    let p = problems_in("local x = require('nope')\n", Some(&m));
    assert_eq!(
        p,
        vec![(1, Some((18, 24)), "no nope.lua in this module".to_owned(), false)]
    );
    assert!(problems_in("local lib = require('lib')\nlocal u = require \"lib.util\"\n", Some(&m)).is_empty());
    let p = problems_in("require('main')\n", Some(&m));
    assert!(p[0].2.contains("a file can't require itself"), "{p:?}");
    let lib = module("lib.lua", &[("main.lua", "")]);
    let p = problems_in("require('main')\n", Some(&lib));
    assert!(p[0].2.contains("main.lua starts the module"), "{p:?}");
    let p = problems_in("require('../up')\n", Some(&m));
    assert!(p[0].2.contains("bad module name"), "{p:?}");
    let p = problems_in("local lib = require('lib')\n", None);
    assert!(p[0].2.contains("require loads a module's files"), "{p:?}");
    // Only literal names, outside strings and comments; Roblox's form is fine.
    let quiet = "require(script.lib)\n-- require('nope')\nprint(\"require('nope')\")\n";
    assert!(problems_in(quiet, Some(&m)).is_empty());
    assert!(problems_in("require(script.lib)\n", None).is_empty());
}

#[test]
fn require_suggests_the_modules_files() {
    let m = module(
        "main.lua",
        &[
            ("shared.lua", ""),
            ("lib.lua", ""),
            ("lib/util.lua", ""),
            ("main.lua", ""),
        ],
    );
    let before = "local s = require(\"sh";
    let (start, s) = suggest(before, &[], false, Some(&m)).unwrap();
    assert_eq!(start, "local s = require(\"".len());
    assert_eq!(s[0].label, "shared");
    assert_eq!(s[0].kind, Kind::Value);
    assert_eq!(s[0].detail, "require(\"shared\")");
    assert_eq!(s[0].doc, "shared.lua in this module");
    let (_, s) = suggest("require('lib.u", &[], false, Some(&m)).unwrap();
    assert_eq!(s[0].label, "lib.util");
    let all: Vec<String> = suggest("require('", &[], false, Some(&m))
        .unwrap()
        .1
        .into_iter()
        .map(|x| x.label)
        .collect();
    assert!(!all.contains(&"main".to_owned()), "{all:?}");
    // Not in a script, nor in another call's string.
    assert!(suggest(before, &[], false, None).is_none());
    assert!(suggest("print(\"sh", &[], false, Some(&m)).is_none());
}

#[test]
fn a_required_files_members_are_suggested() {
    let lib = "local lib = {}\nfunction lib.greet(name) return name end\nfunction lib.stop() end\nlib.version = 2\nreturn lib\n";
    let m = module("main.lua", &[("lib.lua", lib)]);
    let file = ["local lib = require('lib')".to_owned(), "lib.".to_owned()];
    let (_, s) = suggest("lib.", &file, false, Some(&m)).unwrap();
    let names: Vec<&str> = s.iter().map(|x| x.label.as_str()).collect();
    for want in ["greet", "stop", "version"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    let greet = s.iter().find(|x| x.label == "greet").unwrap();
    assert_eq!((greet.kind, greet.doc.as_str()), (Kind::Function, "in lib.lua"));
    // A table returned as a constructor.
    let m = module(
        "main.lua",
        &[("cfg.lua", "return {\n  speed = 1,\n  go = function() end,\n}\n")],
    );
    let file = ["local cfg = require('cfg')".to_owned(), "cfg.".to_owned()];
    let (_, s) = suggest("cfg.sp", &file, false, Some(&m)).unwrap();
    assert_eq!(s[0].label, "speed");
}

#[test]
fn members_another_file_adds_to_a_shared_table_are_no_problem() {
    // The template's way: shared.lua holds a table, main.lua adds to it at
    // start, lib.lua reads what it added. That runs: nothing is marked, a
    // name one letter from a member's (players, player) included.
    let shared = "local shared = {\n  greeting = 'Hello',\n  ticks = 0,\n  player = nil,\n}\nreturn shared\n";
    let main = "local shared = require('shared')\nshared.players = {}\nlocal lib = require('lib')\n";
    let m = module("lib.lua", &[("main.lua", main), ("shared.lua", shared)]);
    let lib = "local shared = require('shared')\nlocal lib = {}\nfunction lib.count()\n  return #shared.players\nend\nreturn lib\n";
    assert_eq!(problems_in(lib, Some(&m)), vec![]);
    // Nor is a member of a required file's table one letter from another:
    // members come from anywhere (a set by key, another file, a function).
    let m = module(
        "main.lua",
        &[("lib.lua", "local lib = {}\nfunction lib.greet() end\nreturn lib\n")],
    );
    assert_eq!(
        problems_in("local lib = require('lib')\nlib.gret()\n", Some(&m)),
        vec![]
    );
}

#[test]
fn names_and_modules_under_the_cursor() {
    use horizon_studio::game::hstudio_assist::{module_at, name_at};
    let line = "local x = player.pos() + obj:m(1.5)";
    let name = |col: usize| name_at(line, col).map(|(_, _, n)| n);
    // As far as the part clicked: the table, or its member.
    assert_eq!(name(line.find("player").unwrap() + 2).as_deref(), Some("player"));
    assert_eq!(name(line.find("pos").unwrap() + 1).as_deref(), Some("player.pos"));
    assert_eq!(
        name(line.find("pos").unwrap() + 3).as_deref(),
        Some("player.pos"),
        "just after it"
    );
    assert_eq!(
        name(line.find(":m").unwrap() + 1).as_deref(),
        Some("obj.m"),
        "':' read as '.'"
    );
    assert_eq!(name(line.find('x').unwrap()).as_deref(), Some("x"));
    let (start, end, _) = name_at(line, line.find("pos").unwrap()).unwrap();
    assert_eq!(&line[start..end], "player.pos");
    // Not in strings or comments, nor on numbers or keywords.
    assert_eq!(name(line.find("1.5").unwrap()), None);
    assert_eq!(name(1), None, "local is a keyword");
    let quoted = "print('player.pos') -- player.pos";
    assert_eq!(name_at(quoted, 9), None);
    assert_eq!(name_at(quoted, quoted.len() - 3), None);
    // The string of an import or a require.
    let imports = "local kit = import(\"uikit\")  local u = require('lib.util')";
    assert_eq!(
        module_at(imports, imports.find("uikit").unwrap()),
        Some((true, "uikit".into()))
    );
    assert_eq!(
        module_at(imports, imports.find("lib.util").unwrap() + 4),
        Some((false, "lib.util".into()))
    );
    assert_eq!(module_at(imports, 2), None);
    assert_eq!(module_at("print(\"uikit\")", 8), None);
}

#[test]
fn definitions_are_found_in_the_file() {
    use horizon_studio::game::hstudio_assist::{definition, documented};
    let src = [
        "local function greet(name, times)",
        "  return name",
        "end",
        "function kit.shout(s) end",
        "function obj:m() end",
        "local a, b = 1, 2",
        "for i, v in ipairs({}) do end",
        "count = 0",
        "kit.size = 3",
        "if count == 1 then count = 2 end",
        "local greet = 5",
    ];
    let lines: Vec<String> = src.iter().map(|s| (*s).to_owned()).collect();
    let at = |name: &str, row: usize| definition(&lines, name, row, false).map(|(r, a, b)| (r, &lines[r][a..b]));
    assert_eq!(at("greet", 3), Some((0, "greet")), "local function");
    assert_eq!(at("times", 1), Some((0, "times")), "a parameter");
    assert_eq!(at("kit.shout", 9), Some((3, "kit.shout")), "function t.f");
    assert_eq!(at("obj.m", 9), Some((4, "obj:m")), "function t:m");
    assert_eq!(at("b", 9), Some((5, "b")), "local a, b");
    assert_eq!(at("v", 9), Some((6, "v")), "a for name");
    assert_eq!(at("count", 9), Some((7, "count")), "a global assignment");
    assert_eq!(at("kit.size", 9), Some((8, "kit.size")));
    // The nearest above wins; with none above, the first below.
    assert_eq!(at("greet", 10), Some((10, "greet")));
    assert_eq!(at("count", 0), Some((7, "count")));
    assert_eq!(at("nothing", 5), None);
    // Another file of a module: only what it defines for all of it.
    assert_eq!(definition(&lines, "a", 0, true), None, "a local");
    assert_eq!(definition(&lines, "count", 0, true).map(|d| d.0), Some(7));
    // The Docs have the API's functions and the console's.
    assert!(documented("player.pos") && documented("help") && documented("on"));
    assert!(!documented("player") && !documented("string.format") && !documented("greet"));
}
