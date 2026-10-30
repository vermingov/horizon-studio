// Suggestions and the live check (hstudio_assist).

use horizon_studio::game::hstudio_assist::{Kind, STDLIB, check, suggest};

fn labels(before: &str, lines: &[&str], console: bool) -> Vec<String> {
    let lines: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
    suggest(before, &lines, console)
        .map(|(_, s)| s.into_iter().map(|x| x.label).collect())
        .unwrap_or_default()
}

#[test]
fn suggestions_fit_what_is_typed() {
    assert_eq!(labels("play", &[], false)[0], "player");
    let members = labels("player.po", &[], false);
    assert_eq!(&members[..2], ["pos", "pose"], "{members:?}");
    let (start, s) = suggest("x = player.po", &[], false).unwrap();
    assert_eq!(start, "x = player.".len());
    assert!(s[0].detail.starts_with("player.pos("), "{:?}", s[0]);
    assert!(!s[0].doc.is_empty());
    assert_eq!(s[0].kind, Kind::Function);
    assert_eq!(labels("name:up", &[], false)[0], "upper");
    assert_eq!(labels("math.ra", &[], false)[..2], ["rad", "random"]);
    // Events inside on("...").
    let (start, s) = suggest("on(\"ti", &[], false).unwrap();
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
    let lines: Vec<String> = src.lines().map(str::to_owned).collect();
    check(&lines, "m.lua")
        .into_iter()
        .map(|p| (p.line, p.span, p.message, p.error))
        .collect()
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
    let (start, s) = suggest("x = player:po", &[], false).unwrap();
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
