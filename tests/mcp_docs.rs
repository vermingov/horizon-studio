// The MCP server's reference (src/mcp_docs.rs) against the runtime: an entry
// for every API function and for every standard function a mod can reach,
// none for what is gone, Lua examples that compile, and lookups that work.

use horizon_studio::game::hstudio::{API, CONSOLE_API, LogKind};
use horizon_studio::game::hstudio_lua::Lua;
use horizon_studio::game::hstudio_lualib::new_env;
use horizon_studio::headless::{Run, STEPS_HELP, Source};
use horizon_studio::mcp_docs::{ENTRIES, Entry, TOPICS, entry, index, search, topic};
use std::collections::BTreeSet;

/// Every API function by the name scripts call it, console ones too.
fn api_names() -> BTreeSet<String> {
    API.iter()
        .map(|(table, name, _, _)| {
            if table.is_empty() {
                (*name).to_owned()
            } else {
                format!("{table}.{name}")
            }
        })
        .chain(CONSOLE_API.iter().map(|(name, _, _)| (*name).to_owned()))
        .collect()
}

/// An entry named like an API function: not an event, a widget, the
/// standard library or Roblox's data types.
fn api_like(e: &Entry) -> bool {
    !["stdlib", "roblox", "executor"].contains(&e.topic) && !e.name.contains(':')
}

/// A mod run headless; what it printed, without the `[m] ` in front.
fn printed(src: &str) -> Vec<String> {
    let source = Source::Files {
        id: "m".into(),
        files: vec![("m.lua".into(), src.into())],
        libraries: None,
        remove: Vec::new(),
    };
    let mut run = Run::new(&source).unwrap();
    run.frames(1);
    let lines: Vec<(LogKind, String)> = run.studio.log_since(0).map(|(_, l)| (l.kind, l.text.clone())).collect();
    assert!(
        lines.iter().all(|(k, _)| *k != LogKind::Error),
        "the mod failed: {lines:?}"
    );
    lines
        .into_iter()
        .filter_map(|(_, t)| t.strip_prefix("[m] ").map(str::to_owned))
        .collect()
}

/// The ```lang blocks of a Markdown text.
fn blocks<'a>(text: &'a str, lang: &str) -> Vec<&'a str> {
    let open = format!("```{lang}\n");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(&open) {
        let body = &rest[at + open.len()..];
        let end = body.find("\n```").expect("a code block is closed");
        out.push(&body[..end]);
        rest = &body[end + 4..];
    }
    out
}

#[test]
fn every_api_function_has_an_entry_and_no_entry_is_stale() {
    let api = api_names();
    let names: BTreeSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
    assert_eq!(names.len(), ENTRIES.len(), "two entries have the same name");
    let missing: Vec<&String> = api.iter().filter(|n| !names.contains(n.as_str())).collect();
    assert!(missing.is_empty(), "API functions without an entry: {missing:?}");
    let stale: Vec<&str> = ENTRIES
        .iter()
        .filter(|e| api_like(e) && !api.contains(e.name))
        .map(|e| e.name)
        .collect();
    assert!(
        stale.is_empty(),
        "entries for functions the API doesn't have: {stale:?}"
    );
    for e in ENTRIES {
        assert!(
            TOPICS.iter().any(|t| t.id == e.topic),
            "{} is in topic '{}', which doesn't exist",
            e.name,
            e.topic
        );
        // Each starts with its signature.
        let first = e.doc.trim().lines().next().unwrap_or("");
        if api_like(e) {
            assert!(
                first.starts_with(&format!("`{}(", e.name)),
                "{}'s entry starts with '{first}', not its signature",
                e.name
            );
        }
    }
}

#[test]
fn the_events_documented_are_the_events_on_takes() {
    // on() names the events it knows when given another.
    let lines = printed(r#"print(select(2, pcall(on, "no_such_event", function() end)))"#);
    let msg = lines.first().expect("the error was printed");
    let list = msg
        .split("unknown event 'no_such_event' (")
        .nth(1)
        .and_then(|s| s.split(", or one of").next())
        .unwrap_or_else(|| panic!("on's error changed: {msg}"));
    let known: BTreeSet<String> = list.split(", ").map(|s| format!("event:{s}")).collect();
    let documented: BTreeSet<String> = ENTRIES
        .iter()
        .filter(|e| e.name.starts_with("event:") && e.name != "event:custom")
        .map(|e| e.name.to_owned())
        .collect();
    assert_eq!(known, documented);
    assert!(entry("event:custom").is_some(), "mod-to-mod events have an entry");
    // And each is accepted.
    let src: String = documented
        .iter()
        .map(|e| format!("on(\"{}\", function() end)\n", &e["event:".len()..]))
        .collect();
    printed(&format!("{src}on(\"m:custom\", function() end)\nprint(\"ok\")"));
}

#[test]
fn every_function_a_mod_reaches_has_an_entry_and_every_stdlib_entry_exists() {
    // The mod's globals and the tables in them, as names: f:string.format, v:math.pi.
    let lines = printed(
        r#"
        local names, seen = {}, {}
        local all = {}
        for _, env in { getrenv(), getgenv(), _G } do
          for k, v in pairs(env) do all[k] = v end
        end
        for k, v in pairs(all) do
          if type(v) == "function" then
            names[#names + 1] = "f:" .. k
          elseif type(v) == "table" and k ~= "_G" then
            names[#names + 1] = "v:" .. k
            for kk, vv in pairs(v) do
              names[#names + 1] = (type(vv) == "function" and "f:" or "v:") .. k .. "." .. kk
            end
          else
            names[#names + 1] = "v:" .. k
          end
        end
        table.sort(names)
        print(table.concat(names, " "))
        "#,
    );
    let all: BTreeSet<&str> = lines.first().expect("the names were printed").split(' ').collect();
    let functions: BTreeSet<&str> = all.iter().filter_map(|n| n.strip_prefix("f:")).collect();
    let present: BTreeSet<&str> = all.iter().map(|n| &n[2..]).collect();
    let names: BTreeSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
    let missing: Vec<&&str> = functions.iter().filter(|f| !names.contains(**f)).collect();
    assert!(
        missing.is_empty(),
        "functions a mod can call without an entry: {missing:?}"
    );
    // The console's own functions are not in a mod's globals.
    for (name, _, _) in CONSOLE_API {
        assert!(!functions.contains(name), "{name} reaches mods");
    }
    let gone: Vec<&str> = ENTRIES
        .iter()
        .filter(|e| ["stdlib", "roblox", "executor"].contains(&e.topic) && !present.contains(e.name) && e.name != "_G")
        .map(|e| e.name)
        .collect();
    assert!(gone.is_empty(), "stdlib entries for what mods don't have: {gone:?}");
    let api = api_names();
    let misfiled: Vec<&str> = ENTRIES
        .iter()
        .filter(|e| e.topic == "stdlib" && api.contains(e.name))
        .map(|e| e.name)
        .collect();
    assert!(misfiled.is_empty(), "API functions filed under stdlib: {misfiled:?}");
}

#[test]
fn every_lua_example_compiles_and_every_json_example_parses() {
    let mut lua = Lua::new();
    let env = new_env(&mut lua);
    let mut texts: Vec<(String, String)> = TOPICS
        .iter()
        .map(|t| (format!("topic:{}", t.id), topic(t.id).unwrap()))
        .collect();
    texts.extend(ENTRIES.iter().map(|e| (e.name.to_owned(), e.doc.to_owned())));
    let mut failures = Vec::new();
    let mut count = 0;
    for (name, text) in &texts {
        for block in blocks(text, "lua") {
            count += 1;
            // Compiled, not run.
            if let Err(e) = lua.load(block, "doc", &env) {
                failures.push(format!("{name}: {e}\n{block}"));
            }
        }
        for block in blocks(text, "json") {
            if let Err(e) = serde_json::from_str::<serde_json::Value>(block) {
                failures.push(format!("{name}: not JSON: {e}\n{block}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    assert!(count > ENTRIES.len(), "every entry has an example ({count} blocks)");
    for e in ENTRIES {
        assert!(!blocks(e.doc, "lua").is_empty(), "{} has no Lua example", e.name);
    }
}

#[test]
fn lookups_find_what_they_should() {
    for name in [
        "player.pos",
        "player.pos()",
        " player.pos(...) ",
        "event:tick",
        "ui.open",
        "math.rad",
    ] {
        assert!(entry(name).is_some(), "entry({name:?})");
    }
    assert_eq!(entry("player.pos()").unwrap().name, "player.pos");
    assert_eq!(entry("event:waypoints:changed").unwrap().name, "event:custom");
    assert!(entry("player.teleport").is_none());
    let hits = search("radians", 5);
    assert!(hits.iter().any(|(n, _)| n == "math.rad"), "{hits:?}");
    assert!(search("draw through walls", 10).iter().any(|(n, _)| n == "draw.line"));
    assert!(search("RATE limit", 10).iter().any(|(n, _)| n.starts_with("topic:")));
    assert!(search("zzzz qqqq", 10).is_empty());
    assert!(search("", 10).is_empty());
    assert_eq!(search("the", 3).len(), 3);
    let idx = index();
    for t in TOPICS {
        let text = topic(t.id).unwrap_or_else(|| panic!("topic {}", t.id));
        assert!(text.len() > 400, "topic {} is nearly empty", t.id);
        assert!(idx.contains(&format!("`{}`", t.id)), "the index lacks {}", t.id);
    }
    assert!(topic("nope").is_none());
    assert!(topic("tests").unwrap().contains(STEPS_HELP));
    // A topic holds its entries in full.
    assert!(
        topic("events")
            .unwrap()
            .contains(entry("event:hud").unwrap().doc.trim())
    );
}

#[test]
fn every_mcp_tool_is_in_the_mcp_topic() {
    let text = topic("mcp").unwrap();
    let tools = horizon_studio::mcp::tools();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.len() >= 14);
    for name in names {
        assert!(text.contains(&format!("`{name}`")), "topic mcp lacks the tool {name}");
    }
}

/// docs/reference.json is what the MCP server fetches: it must say what
/// this code says. `HORIZON_STUDIO_WRITE_DOCS=1 cargo test --test mcp_docs`
/// (or `horizon-studio-mcp docs-json > docs/reference.json`) writes it.
#[test]
fn the_published_reference_is_current() {
    use horizon_studio::mcp_docs::Reference;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/reference.json");
    let now = Reference::built_in().to_text();
    if std::env::var_os("HORIZON_STUDIO_WRITE_DOCS").is_some() {
        std::fs::write(&path, &now).unwrap();
    }
    let published = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        published == now,
        "docs/reference.json is behind the code: run HORIZON_STUDIO_WRITE_DOCS=1 cargo test --test mcp_docs"
    );
    // It reads back as the same reference.
    let back = Reference::from_text(&published, "test").unwrap();
    assert_eq!(
        (back.topics, back.entries),
        (Reference::built_in().topics, Reference::built_in().entries)
    );
}

#[test]
fn the_server_answers_with_a_published_reference() {
    use horizon_studio::mcp::Server;
    use horizon_studio::mcp_docs::{Reference, TopicDoc};
    let dir = std::env::temp_dir().join(format!("hs-docs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Reference::built_in();
    r.topics.push(TopicDoc {
        id: "news".into(),
        title: "What's new".into(),
        summary: "Published after the release.".into(),
        text: "A topic the release did not have.".into(),
    });
    let file = dir.join("reference.json");
    std::fs::write(&file, r.to_text()).unwrap();
    let mut s = Server::new();
    assert!(s.reference().source.starts_with("built into"));
    s.load_docs(file.to_str().unwrap()).unwrap();
    let answer = s.call_tool("docs", &serde_json::json!({ "topic": "news" }));
    let text = answer["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("A topic the release did not have."), "{answer}");
    assert!(
        s.call_tool("docs", &serde_json::json!({}))["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("This copy: the published reference")
    );
    // One for an older studio, or of another layout, is not taken.
    let mut old = Reference::built_in();
    old.version = "0.0.1".into();
    std::fs::write(&file, old.to_text()).unwrap();
    assert!(
        s.load_docs(file.to_str().unwrap())
            .unwrap_err()
            .contains("older than this server's")
    );
    std::fs::write(&file, "{\"schema\": 99}").unwrap();
    assert!(
        s.load_docs(file.to_str().unwrap())
            .unwrap_err()
            .contains("another layout")
    );
    assert!(
        s.reference().topics.iter().any(|t| t.id == "news"),
        "the last good one stays"
    );
    let _ = std::fs::remove_dir_all(dir);
}
