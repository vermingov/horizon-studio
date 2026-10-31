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
    Siblings {
        file: file.to_owned(),
        others: others
            .iter()
            .map(|(f, src)| ((*f).to_owned(), src.lines().map(str::to_owned).collect()))
            .collect(),
    }
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
fn a_required_files_members_are_suggested_and_checked() {
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
    let src = "local lib = require('lib')\nprint(lib.gret('x'), lib.stop(), lib.whatever)\n";
    assert_eq!(
        problems_in(src, Some(&m)),
        vec![(
            2,
            Some((10, 14)),
            "lib.lua has no 'gret' (did you mean greet?)".to_owned(),
            false
        )]
    );
    // What the file itself adds to the table is known.
    let adds = "local lib = require('lib')\nfunction lib.gree() end\nlib.gree()\n";
    assert!(problems_in(adds, Some(&m)).is_empty());
    // A table returned as a constructor.
    let m = module(
        "main.lua",
        &[("cfg.lua", "return {\n  speed = 1,\n  go = function() end,\n}\n")],
    );
    let p = problems_in("local cfg = require('cfg')\nprint(cfg.sped)\n", Some(&m));
    assert!(p[0].2.contains("cfg.lua has no 'sped' (did you mean speed?)"), "{p:?}");
    // One made with setmetatable or filled by key can't be read: nothing is said.
    let dynamic = "local lib = setmetatable({}, {})\nfunction lib.greet() end\nreturn lib\n";
    let m = module("main.lua", &[("lib.lua", dynamic)]);
    assert!(problems_in("local lib = require('lib')\nlib.gret()\n", Some(&m)).is_empty());
}
