//! Horizon Studio's reference for AI agents, as the MCP server gives it: what
//! a script mod is, every API function, event, widget and standard library
//! function, the Lua the studio runs, the test steps and the MCP tools. Topics
//! are prose; entries are the reference, each in one topic. Every fact here
//! comes from the runtime's source (studio.rs, lua.rs, lualib.rs, ui.rs,
//! look.rs, models.rs, bridge.rs, headless.rs, mcp.rs); tests/mcp_docs.rs
//! keeps it in step with the API tables and the runtime.

/// A topic: prose about one part of the studio.
pub struct Topic {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
}

/// One function, event or widget, in Markdown.
pub struct Entry {
    pub name: &'static str,
    pub topic: &'static str,
    pub doc: &'static str,
}

pub const TOPICS: &[Topic] = &[
    Topic {
        id: "start",
        title: "Script mods: scripts and modules, loading, hot reload, errors",
        summary: "What a mod is (a script, or a module of files), its files and header, how it loads, reloads and stops, and where the mods folder is.",
    },
    Topic {
        id: "api",
        title: "The scripting API at a glance",
        summary: "Every namespace and function, coordinates and angles, and which calls work where.",
    },
    Topic {
        id: "events",
        title: "Events",
        summary: "Every on(...) event: when it fires, its arguments, the order in a frame, which calls work in it, and mod-to-mod events.",
    },
    Topic {
        id: "language",
        title: "The Lua the studio runs",
        summary: "Lua 5.3 without goto: numbers (all floats), integer and float behaviour, bitwise operators, strings, tables, metatables, varargs, coroutines, and what is missing.",
    },
    Topic {
        id: "stdlib",
        title: "Standard library",
        summary: "Every standard function a mod can call (base, string, math, table, utf8, os), with each difference from real Lua.",
    },
    Topic {
        id: "screens",
        title: "Screens and widgets (ui.open)",
        summary: "Screen fields and callbacks, every widget kind and its fields, canvas screens, focus, keys and what returns do.",
    },
    Topic {
        id: "gfx",
        title: "Drawing on the HUD and on screens",
        summary: "hud.* in GUI pixels and gfx.* in dp: units, fonts, text sizes, and every color form.",
    },
    Topic {
        id: "world",
        title: "The world: reading it, drawing in it, particles and sounds",
        summary: "world.* reads, draw.* inside on(\"world\"), through-wall lines and labels, particles and sounds.",
    },
    Topic {
        id: "models",
        title: "Models on bodies and items in the world",
        summary: "model.*, player.part, player.pose, player.camera and draw.item: body parts, their axes, pixels, display contexts and first person.",
    },
    Topic {
        id: "meshes",
        title: "Textures, meshes and model files",
        summary: "Real 3D models: textures (PNG files, base64, painted pixels), meshes (triangles, boxes with skin UV, .obj and Blockbench .bbmodel files), draw.mesh, meshes on body parts.",
    },
    Topic {
        id: "input",
        title: "Keys and the mouse",
        summary: "Key and mouse button names, on(\"key\"), keys.down, keys.mapping and its mapping names, and keys on script screens.",
    },
    Topic {
        id: "storage",
        title: "Storage",
        summary: "storage.get and storage.set: what can be kept, where it goes, and the limits.",
    },
    Topic {
        id: "libraries",
        title: "Libraries and mods talking to each other",
        summary: "export, import and emit between mods; modules, whose files require each other; load order and reloads of users; the bundled uikit library's whole API.",
    },
    Topic {
        id: "console",
        title: "The console",
        summary: "The studio's console, its own environment, the console-only functions, /lua and run_lua.",
    },
    Topic {
        id: "agent",
        title: "Horizon's agent from scripts",
        summary: "agent.query and agent.run: the commands scripts may use, what they answer, and where they work.",
    },
    Topic {
        id: "via",
        title: "ViaHorizon",
        summary: "via.*: joining servers of other versions through ViaProxy.",
    },
    Topic {
        id: "voice",
        title: "Voice chat (Simple Voice Chat servers)",
        summary: "voice.*: talking to and hearing the players of a server with Simple Voice Chat: its state, who is talking, mute, deafen, the volume, the microphone and groups.",
    },
    Topic {
        id: "tests",
        title: "Testing mods (test_mod steps)",
        summary: "Every step and expectation of test_mod with examples, the headless game it runs on, and live tests.",
    },
    Topic {
        id: "mcp",
        title: "The MCP server's tools",
        summary: "Every tool of horizon-studio-mcp: arguments, results, live or headless, and the workflow.",
    },
    Topic {
        id: "examples",
        title: "Bundled examples",
        summary: "The example mods that come with the studio and what each one shows.",
    },
    Topic {
        id: "recipes",
        title: "Recipes",
        summary: "Short complete mods for common asks: HUD panel, commands, key toggle, timers, beams, screens, canvas, libraries, settings, models, particles, chat.",
    },
    Topic {
        id: "limits",
        title: "Limits",
        summary: "Step budgets, the chat rate limit and every cap, with the real numbers.",
    },
    Topic {
        id: "roblox",
        title: "Roblox scripts: game, Instances, GUIs, input and data types",
        summary: "The Roblox API on Horizon: game and its services, Instance.new and the Instance tree, signals, ScreenGuis drawn in game, the mouse, UserInputService, the LocalPlayer and its character, the world's mobs, items and other players in the Workspace and Players, the Camera, Parts, Raycast and the bounds queries, the Terrain, TweenService, Enum and every data type.",
    },
    Topic {
        id: "executor",
        title: "Executor functions (sUNC): hooks, debug, files, HTTP, Drawing",
        summary: "The sUNC standard's functions: hookfunction, newcclosure, hookmetamethod, the debug library, getrawmetatable, getgenv and getrenv, getgc and filtergc, Base64 and LZ4, the workspace file system, request and WebSocket, Drawing, cloneref, getconnections, getsenv and hidden properties, and how they differ from a Roblox executor's.",
    },
    Topic {
        id: "errors",
        title: "Error messages",
        summary: "Common errors a script gets, what each means and the fix.",
    },
];

/// The topic's prose (before its entries).
fn topic_text(id: &str) -> &'static str {
    match id {
        "start" => START,
        "api" => API_TEXT,
        "events" => EVENTS,
        "language" => LANGUAGE,
        "stdlib" => STDLIB,
        "screens" => SCREENS,
        "gfx" => GFX,
        "world" => WORLD,
        "models" => MODELS,
        "meshes" => MESHES,
        "input" => INPUT,
        "storage" => STORAGE,
        "libraries" => LIBRARIES,
        "console" => CONSOLE,
        "agent" => AGENT,
        "via" => VIA,
        "voice" => VOICE,
        "tests" => TESTS,
        "mcp" => MCP,
        "examples" => EXAMPLES_TEXT,
        "recipes" => RECIPES,
        "limits" => LIMITS,
        "errors" => ERRORS,
        "roblox" => ROBLOX,
        "executor" => EXECUTOR,
        _ => "",
    }
}

/// A topic's prose, with the test step format filled in from the headless runner.
fn prose(id: &str) -> String {
    topic_text(id).replace("{STEPS_HELP}", crate::headless::STEPS_HELP)
}

// ------------------------------------------------------------------ the reference as data

/// A topic: its prose, filled in.
#[derive(Clone, Debug, PartialEq)]
pub struct TopicDoc {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub text: String,
}

/// An entry: one function, event or widget.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryDoc {
    pub name: String,
    pub topic: String,
    pub doc: String,
}

/// The reference the MCP server answers with: the one built into it, or
/// the one published on the repository's main branch
/// (`docs/reference.json`), which it fetches, so the docs can change
/// without a new release.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    /// Where it came from, for the index.
    pub source: String,
    /// The studio's version it describes.
    pub version: String,
    pub topics: Vec<TopicDoc>,
    pub entries: Vec<EntryDoc>,
}

/// The JSON layout of `docs/reference.json`; a server that reads another
/// keeps its own reference.
pub const SCHEMA: u64 = 1;

/// Where the published reference is.
pub const REFERENCE_URL: &str = "https://raw.githubusercontent.com/vermingov/horizon-studio/main/docs/reference.json";

impl Reference {
    /// The reference this program was built with.
    pub fn built_in() -> Reference {
        Reference {
            source: format!("built into horizon-studio-mcp {}", crate::game::hstudio::VERSION),
            version: crate::game::hstudio::VERSION.to_owned(),
            topics: TOPICS
                .iter()
                .map(|t| TopicDoc {
                    id: t.id.to_owned(),
                    title: t.title.to_owned(),
                    summary: t.summary.to_owned(),
                    text: prose(t.id).trim().to_owned(),
                })
                .collect(),
            entries: ENTRIES
                .iter()
                .map(|e| EntryDoc {
                    name: e.name.to_owned(),
                    topic: e.topic.to_owned(),
                    doc: e.doc.trim().to_owned(),
                })
                .collect(),
        }
    }

    /// As `docs/reference.json` has it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": SCHEMA,
            "version": self.version,
            "topics": self.topics.iter().map(|t| serde_json::json!({
                "id": t.id, "title": t.title, "summary": t.summary, "text": t.text,
            })).collect::<Vec<_>>(),
            "entries": self.entries.iter().map(|e| serde_json::json!({
                "name": e.name, "topic": e.topic, "doc": e.doc,
            })).collect::<Vec<_>>(),
        })
    }

    /// The text of `docs/reference.json`.
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.to_json()).unwrap_or_default();
        text.push('\n');
        text
    }

    /// A published reference (`docs/reference.json`'s text), from `source`.
    pub fn from_text(text: &str, source: &str) -> Result<Reference, String> {
        let j: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not the reference's JSON: {e}"))?;
        if j["schema"].as_u64() != Some(SCHEMA) {
            return Err(format!(
                "a reference of another layout (schema {}, this server reads {SCHEMA})",
                j["schema"]
            ));
        }
        let field = |v: &serde_json::Value, k: &str| -> Result<String, String> {
            v[k].as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("the reference lacks {k}"))
        };
        let list = |k: &str| {
            j[k].as_array()
                .cloned()
                .ok_or_else(|| format!("the reference lacks {k}"))
        };
        let topics = list("topics")?
            .iter()
            .map(|t| {
                Ok(TopicDoc {
                    id: field(t, "id")?,
                    title: field(t, "title")?,
                    summary: field(t, "summary")?,
                    text: field(t, "text")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let entries = list("entries")?
            .iter()
            .map(|e| {
                Ok(EntryDoc {
                    name: field(e, "name")?,
                    topic: field(e, "topic")?,
                    doc: field(e, "doc")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if topics.is_empty() || entries.is_empty() {
            return Err("an empty reference".into());
        }
        Ok(Reference {
            source: source.to_owned(),
            version: field(&j, "version").unwrap_or_default(),
            topics,
            entries,
        })
    }

    /// What the reference holds and how to look things up.
    pub fn index(&self) -> String {
        let mut out = format!(
            "# Horizon Studio reference\n\n\
             This copy: {} (Horizon Studio {}).\n\n\
             Everything needed to write, check and test script mods, in topics (prose) and entries (one function, \
             event or widget each). Look things up three ways:\n\n\
             - a topic by its id, e.g. `start`, `events`, `screens`: its prose, then the full entry of everything in it;\n\
             - an entry by its name: an API function as scripts call it (`player.pos`, `on`, `ui.open`), a standard \
             function (`string.format`, `math.rad`, `pairs`), an event (`event:tick`, `event:hud`, `event:custom` for \
             mod-to-mod events), a widget (`widget:button`); a trailing `()` is fine;\n\
             - a search: words, all of which must appear (case does not matter), best matches first.\n\n\
             New here: read `start`, `api`, `events` and `language`, then `recipes`; before testing, `tests`.\n\n\
             ## Topics\n\n",
            self.source, self.version
        );
        for t in &self.topics {
            out.push_str(&format!("- `{}`: {}. {}\n", t.id, t.title, t.summary));
        }
        out.push_str("\n## Entries by topic\n\n");
        for t in &self.topics {
            let names: Vec<&str> = self
                .entries
                .iter()
                .filter(|e| e.topic == t.id)
                .map(|e| e.name.as_str())
                .collect();
            if !names.is_empty() {
                out.push_str(&format!("- `{}`: {}\n", t.id, names.join(", ")));
            }
        }
        out
    }

    /// A topic's whole text: its prose, then the full entry of each thing in it.
    pub fn topic(&self, id: &str) -> Option<String> {
        let id = id.trim();
        let t = self.topics.iter().find(|t| t.id == id)?;
        let mut out = format!("# {}\n\n{}\n", t.title, t.text.trim());
        let entries: Vec<&EntryDoc> = self.entries.iter().filter(|e| e.topic == t.id).collect();
        if !entries.is_empty() {
            out.push_str("\n## Reference\n");
            for e in entries {
                out.push_str(&format!("\n### {}\n\n{}\n", e.name, e.doc.trim()));
            }
        }
        Some(out)
    }

    /// An entry by its exact name; `player.pos()`, `player.pos(...)` and
    /// spaces around it are fine, and any `event:mymod:name` is `event:custom`.
    pub fn entry(&self, name: &str) -> Option<&EntryDoc> {
        let mut n = name.trim();
        if n.ends_with(')')
            && let Some(open) = n.find('(')
        {
            n = n[..open].trim_end();
        }
        if let Some(e) = self.entries.iter().find(|e| e.name == n) {
            return Some(e);
        }
        if let Some(e) = self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(n)) {
            return Some(e);
        }
        match n.strip_prefix("event:") {
            Some(rest) if rest.contains(':') => self.entries.iter().find(|e| e.name == "event:custom"),
            _ => None,
        }
    }

    /// Entries and topics with every word of `query` in them (any case),
    /// best first: (an entry's name or `topic:<id>`, a short snippet).
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String)> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits: Vec<(u32, String, String)> = Vec::new();
        for e in &self.entries {
            if let Some(s) = score(&words, &e.name, &e.doc) {
                hits.push((s + 5, e.name.clone(), snippet(&e.doc, &words)));
            }
        }
        for t in &self.topics {
            let text = format!("{}\n{}\n{}", t.title, t.summary, t.text);
            if let Some(s) = score(&words, &t.id, &text) {
                hits.push((s, format!("topic:{}", t.id), format!("{}: {}", t.title, t.summary)));
            }
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        hits.into_iter().take(limit).map(|(_, n, s)| (n, s)).collect()
    }
}

/// The reference built into this program.
pub fn built_in() -> &'static Reference {
    static BUILT_IN: std::sync::OnceLock<Reference> = std::sync::OnceLock::new();
    BUILT_IN.get_or_init(Reference::built_in)
}

/// The built-in reference's index.
pub fn index() -> String {
    built_in().index()
}

/// A topic of the built-in reference.
pub fn topic(id: &str) -> Option<String> {
    built_in().topic(id)
}

/// An entry of the built-in reference.
pub fn entry(name: &str) -> Option<&'static EntryDoc> {
    built_in().entry(name)
}

/// A search of the built-in reference.
pub fn search(query: &str, limit: usize) -> Vec<(String, String)> {
    built_in().search(query, limit)
}

/// How well `name` and `text` match every word, or None when one is missing.
fn score(words: &[String], name: &str, text: &str) -> Option<u32> {
    let name = name.to_lowercase();
    let lower = text.to_lowercase();
    let first = lower.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    // The name's parts, for a word that starts with one ("radians" and math.rad).
    let parts: Vec<&str> = name.split(['.', ':', '_']).filter(|p| p.len() >= 3).collect();
    let mut total = 0;
    for w in words {
        let in_name = name.contains(w.as_str());
        let count = lower.matches(w.as_str()).count() as u32;
        if !in_name && count == 0 {
            return None;
        }
        if name == *w || name.rsplit(['.', ':']).next() == Some(w.as_str()) {
            total += 100;
        } else if in_name {
            total += 40;
        }
        if parts.iter().any(|p| w.starts_with(p)) {
            total += 25;
        }
        if first.contains(w.as_str()) {
            total += 10;
        }
        total += count.min(10) * 2;
    }
    Some(total)
}

/// The entry's first line, and the first line with a word of the query
/// when that is another one.
fn snippet(doc: &str, words: &[String]) -> String {
    let mut lines = doc.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or("").to_owned();
    let has = |l: &str| {
        let l = l.to_lowercase();
        words.iter().any(|w| l.contains(w.as_str()))
    };
    let mut out = first.clone();
    if !has(&first)
        && let Some(l) = lines.find(|l| has(l))
    {
        out.push_str(" ... ");
        out.push_str(l);
    }
    if out.len() > 240 {
        let cut = (0..=237).rev().find(|&i| out.is_char_boundary(i)).unwrap_or(0);
        out.truncate(cut);
        out.push_str("...");
    }
    out
}

macro_rules! e {
    ($name:expr, $topic:expr, $doc:expr) => {
        Entry {
            name: $name,
            topic: $topic,
            doc: $doc,
        }
    };
}

// ------------------------------------------------------------------ topics

const START: &str = r##"
A script mod is Lua that Horizon Studio runs inside Horizon (Minecraft Java 26.3 in Rust). It is either a **script**, one file `<id>.lua`, or a **module** (a folder mod), a folder `<id>/` whose `main.lua` loads its other `.lua` files with `require` (see `libraries`). Both live in the mods folder:

`<game folder>/horizon-mods/.horizon-mod-data/horizon_studio/mods/`

On Linux, Horizon's default game folder is `~/.local/share/horizon` (`$HORIZON_HOME` changes it). The MCP server finds the folder by itself (see `mcp`). Beside `mods/`, the studio keeps `storage/` (what `storage.set` writes), `studio.txt` (mods turned off in the Library; a mod deleted in the studio leaves it, and a mod made new there starts on), `history.txt` and `scratch.lua` (the console's), and `trash/`, where the studio's Delete moves a file or a whole mod (as `trash/<its path in mods/>`: the newest copy of each file at its name, an older one beside it under a number, `test (2).lua`, then `test (3).lua`; a module deleted whole joins the files deleted from it before; nothing in the trash is deleted, and the studio never empties it).

## Ids and files

- The id comes from the file or folder name: lowercased, every character but `a-z`, `0-9`, `_` and `-` becomes `_`, at most 48 characters (`My Mod.lua` is `my_mod`).
- Names that start with `.` are skipped, and so is a folder without `main.lua`. When a folder and a file give the same id, the first in sorted order wins (`x` sorts before `x.lua`).
- A module's files are its `.lua` files up to three folders deep (`lib/util.lua`), names that start with `.` left out; other files (textures, models) are its assets.
- Errors in a script name the file `<id>.lua`; in a module, the id and the file's path in it: `<id>/main.lua`, `<id>/lib/util.lua`. The editor titles files the same way.

## Header

Comment lines at the top of the main file name the mod in the Library. Blank lines are skipped; the first line that is not a `--` comment ends the header. Keys are `name`, `version`, `author` and `description` (any case); other comments are ignored. Without `name`, the name is the id. `chat.show` prefixes its lines with the name.

```lua
-- name: Waypoints
-- version: 1.0
-- author: Alex
-- description: Beams and labels you can see through walls.
```

## Lifecycle

1. **Load.** The main file runs once, top to bottom, with a budget of 10,000,000 steps. This is where a mod registers what it does: `on(event, fn)`, `command(name, fn)`, `after` and `every`, `hud.panel`, `model.attach`, `export`. A main file that returns a table exports it, unless it called `export`.
2. **Running.** The game then calls what the mod registered, each call with a budget of 2,000,000 steps: ticks (20 a second in a world), frames, the HUD, the world as it draws, keys, chat lines, commands, timers, agent replies, screen callbacks.
3. **Hot reload.** Every 0.5 s the studio looks at the mods folder. When a mod's files change (the size or modification time of a script, or of any `.lua` file up to 3 folders deep in a module, names that start with `.` left out), it reloads that mod, a module as a whole whichever of its files changed: its `unload` handlers run, everything it registered is dropped (handlers, commands, timers, its HUD panel and zoom, its models, its screen, its `require`d modules), and its main file runs again in a fresh environment. Then the mods that `import` it reload too. New mods load; mods whose files are deleted unload. Saving in the in-game Editor, in any editor, or with the MCP tool `write_mod` all work the same way.
4. **Unload.** `on("unload", fn)` runs before a reload, when the mod is turned off in the Library, and when its file is deleted. It does not run when the mod fails.

Each mod has its own global environment (`_G`), with the standard library and the API in it. A mod's globals are invisible to other mods and to the console: share values with `export` and `import`, or `emit` events.

## Errors stop only that mod

An error while loading, or in an event handler, a timer, an agent callback or a screen callback stops that mod (its state becomes "failed"): the studio logs `<id> stopped: <message>` and a traceback, shows the line in chat (when the studio's screen is closed), and drops what the mod registered. Other mods keep running. Saving the file again, `reload("<id>")` in the console, or the MCP tool `reload_mod` retries.

Messages carry the file and line: `waypoints.lua:12: attempt to index a nil value (local 'p')`. In a handler they start with the event: `tick: waypoints.lua:12: ...`; in a timer `timer: `, in a screen callback `screen: `, in an agent callback `agent callback: `.

An error in a command's function does not stop the mod: the player sees `/name: <message>` in chat and the console logs it.

Use `pcall` around code that may fail. Running out of steps (an endless loop), a string over 4 MiB and a table over 1,048,576 entries cannot be caught by `pcall`: they always stop the mod.

## Making and installing mods

- In a world, F8 (rebindable in Options > Mods) opens the studio: Console, Editor, Library, Docs and Settings. The Library's New button (or N) and the + at the top of the Editor's script list open the New prompt: a name, and a Script | Module switch (Tab turns it). A script is `<id>.lua` from a template; a module is `<id>/` with `main.lua`, `shared.lua` and `lib.lua`, wired together with `require`. In the Editor, Ctrl+S saves (the mod reloads) and Ctrl+Enter runs the file in the console (on a module's file, it saves it and reloads the module).
- The Editor's script list shows a module as a folder that folds open to its files. The menus of its rows (their "...", or a right click) add, rename and delete a module's files, delete a mod, and make a script a module (its file becomes `<id>/main.lua`, keeping its id, storage and on/off state). A delete asks first and moves what it deletes to `trash/`. A mod's menu also renames it (its file or folder takes the new id in place, with its on/off state and its saved data in `storage/<id>`; its `-- name:` follows when it was the old id's, mods that import the old id fail until they import the new one, and mods that were waiting for the new id start; an id another mod has is refused, whatever its file is called) and duplicates it (the copy starts turned off, with its own `-- name:`). In the Library each mod has the same menu (its "...", a right click on its row, the . key or the Menu key), with a module's files to open, which is how the executor styles, which have no script list, reach them; F there makes a new file of a module and M makes a script a module. When the open script is made a module elsewhere (an agent's `write_mod`), the editor follows it to its main.lua, unsaved edits and all; when the open file is removed, its module's main.lua opens, or with unsaved edits the file stays open, marked as removed (a save writes it back, Close lets it go).
- Dropping a `.lua` file, a mod folder or a `.zip` of either on the game window installs it; a mod of the same id and kind is replaced where it is, whatever its file or folder is called (a dropped `night_light.lua` updates `Night Light.lua`; a script and a module of one id don't replace each other) (at most 256 files and 8 MiB).
- On its first start the studio writes the welcome mod `hello.lua`, and it installs the bundled libraries `uikit` and `viahorizon` once (deleting one keeps it deleted).
- In chat, `/studio` opens the studio, `/studio reload` reloads every mod, `/studio help` lists the commands the studio takes (with what they take and do), `/lua <code>` runs Lua in the console. These and every mod's `command` are in the chat's suggestions, beside the server's commands.
- An AI agent writes mods with the MCP tool `write_mod`, which checks the mod headless first (see `mcp`).

## A whole mod

```lua
-- name: Hello
-- description: /hello [name] says hello.

print("hello loaded") -- in the studio's console, as "[hello] hello loaded"

command("hello", function(rest)
  local name = rest ~= "" and rest or "world"
  return "Hello, " .. name .. "!" -- shown in the player's chat
end, "Says hello: /hello [name]")

on("tick", function()
  -- 20 times a second, in a world
end)
```
"##;

const API_TEXT: &str = r##"
Every function a mod sees, by namespace. Each has an entry with its full documentation (look it up by name, e.g. `player.pos`).

| Namespace | Functions | For |
| --- | --- | --- |
| globals | `on`, `command`, `after`, `every`, `cancel`, `color`, `export`, `import`, `emit`, `require` | events, chat commands, timers, colors, libraries |
| `chat` | `show`, `say` | a line only the player sees; a message to the server |
| `game` | `run`, `fps`, `screen`, `time`, `ticks` | commands as the player, frame rate, the open screen, clocks |
| `player` | `pos`, `block`, `look`, `facing`, `dimension`, `health`, `food`, `held`, `info`, `pose`, `part`, `camera` | the player (nil outside a world) |
| `world` | `block`, `time`, `find`, `entities`, `particle` | blocks, the day, nearby blocks and entities, particles |
| `hud` | `panel`, `text`, `rect`, `width`, `size` | the HUD in GUI pixels |
| `gfx` | `size`, `rect`, `border`, `text`, `measure`, `mouse` | rounded shapes and Geist text in dp |
| `draw` | `line`, `box`, `block`, `fill`, `beam`, `text`, `item` | shapes in the world, inside `on("world")` |
| `sound` | `play`, `at` | sounds |
| `ui` | `open`, `close`, `is_open` | screens with widgets, or canvases |
| `model` | `attach`, `set`, `remove`, `clear`, `list` | items and boxes that follow body parts |
| `keys` | `down`, `mapping` | held keys, vanilla key mappings |
| `view` | `zoom` | zooming the view |
| `agent` | `query`, `run`, `stop` | Horizon's agent: reads and actions as the player |
| `storage` | `get`, `set` | values kept on disk per mod |
| `mod` | `info` | this mod's id, name, version |
| `json` | `encode`, `decode` | JSON text and tables |
| `via` | `status`, `versions`, `enable`, `version`, `log` | ViaHorizon, joining servers of other versions |
| console only | `help`, `clear`, `reload`, `mods`, `edit`, `new` | the console (see `console`) |

The standard library (`print`, `pairs`, `string`, `math`, `table`, `utf8`, `os`...) is in `stdlib`.

## Coordinates and angles

- World positions are in blocks: x grows east, y up, z south. A block's position is its lowest corner; its centre is `x + 0.5, y + 0.5, z + 0.5`. Functions that take block positions floor them.
- Yaw is in degrees: 0 looks south (+z), 90 west (-x), 180 north, 270 (or -90) east. It is not kept within 0..360: use `yaw % 360`. Pitch is -90 straight up, 90 straight down.
- The HUD and screens are measured from the top left, y down: `hud.*` in GUI pixels, `gfx.*` in dp (see `gfx`).

## Which calls work where

The studio runs a call in one of four phases. Most functions work in all of them; these are the exceptions.

| Phase | What runs in it | Only here | Not here |
| --- | --- | --- | --- |
| normal | loading, `tick`, `frame`, `key`, `chat`, `unload`, mod-to-mod events, timers, commands and their `suggest` functions, agent callbacks, screen callbacks other than `draw`, the console | | `hud.text`, `hud.rect`, `gfx.rect`, `gfx.border`, `gfx.text`, `draw.*` |
| HUD | `on("hud")` | `hud.text`, `hud.rect`, `gfx.rect`, `gfx.border`, `gfx.text` (also in a screen's draw) | `agent.*`, `world.find`, `world.entities`, `world.particle`, `sound.*`, `draw.*` |
| screen draw | a screen's `draw(w, h)` | the HUD's drawing functions | `world.particle`, `draw.*` |
| world | `on("world")` | `draw.*` | `hud.text`, `hud.rect`, `gfx.rect`, `gfx.border`, `gfx.text`, `world.particle`, `sound.*` |

In the game, the world and a script screen are drawn (`on("world")` and a screen's `draw`) while the game is only read: what they change in it (`chat.show`, `chat.say`, `game.run`, `sound.*` in a screen's draw, copying to the clipboard) happens at the start of the next frame, a frame later. Agent commands (`agent.*`, `world.find`, `world.entities`) need the game itself and fail there with "the agent is not available here": call them from `tick`, `frame`, commands and click/key callbacks. Headless tests do not show this.

## Arguments

- A wrong argument raises `bad argument #N to 'name' (number expected, got nil)`; N counts from 1.
- Numbers given as strings are converted where a number is expected (`"5"` works), and numbers work where a string is expected.
- Optional arguments may be left out or given as nil.
- Every color argument takes the forms in `color` (`"#rrggbb"`, `"#aarrggbb"`, a 0xAARRGGBB number).
- Functions that read the player or the world return nil outside a world.
"##;

const EVENTS: &str = r##"
`on(event, fn)` registers a handler. These are the events (each has an entry, `event:<name>`):

| Event | When | Arguments |
| --- | --- | --- |
| `tick` | 20 times a second, only in a world (game ticks; after a slow frame up to 10 run at once) | none |
| `frame` | every frame, in a world or not (the title screen too) | `dt`: seconds since the last frame (0 to 1) |
| `hud` | every frame the HUD is drawn (in a world, not while F1 hides the HUD) | `w`, `h`: the GUI's size in GUI pixels |
| `world` | every frame as the world is drawn | none |
| `key` | a key or mouse button went down or up (on the next frame) | `key` (`"key.keyboard.r"`, `"key.mouse.left"`), `down` (boolean) |
| `chat` | a chat line came from the server (on the next frame) | `text`: its plain text |
| `unload` | before the mod reloads, is turned off or its file is deleted | none |
| `<mod>:<name>` | another mod (or this one) called `emit("<mod>:<name>", ...)`, on the next frame | the values given to `emit` |

## What runs in a frame, in order

1. Every 0.5 s: the mods folder is scanned and changed mods reload.
2. Keys that went down or up since the last frame: `key` handlers.
3. Events mods emitted: their handlers.
4. Chat lines that arrived: `chat` handlers.
5. Chat commands typed for mods: their functions.
6. Ticks the game advanced (in a world): `tick` handlers, at most 10 times.
7. Timers that are due (`after`, `every`).
8. Agent replies that arrived: `agent.run` callbacks.
9. `frame` handlers.
10. In the game, the completions the chat asked for (a command's `suggest` functions), and the chat's command tree catching up with the commands registered or dropped.
11. Drawing: `world` handlers and attached models as the world draws; then the HUD (through-wall lines and labels from `world` handlers, `hud` handlers, `hud.panel` boxes); then a script screen (its widgets and `draw`).

Handlers of one event run mod by mod (in the order the mods were found: sorted file names, newer mods after), then the console's; within a mod, in the order they were registered. A handler may register more handlers; they run from the next time the event fires. There is no way to remove a handler: keep a flag and return early, or reload the mod.

## Which calls work in which event

- `tick`, `frame`, `key`, `chat`, `unload`, mod-to-mod events: everything but drawing (`hud.text`, `hud.rect`, `gfx.rect`, `gfx.border`, `gfx.text`, `draw.*`).
- `hud`: `hud.text`, `hud.rect` and `gfx.*` drawing work only here (and in a screen's draw). Agent functions, `world.find`, `world.entities`, `world.particle`, `sound.*` and `draw.*` raise errors.
- `world`: `draw.*` work only here. `hud.text`/`hud.rect`/`gfx.*` drawing, `world.particle` and `sound.*` raise errors. In the game the agent is not available here either, and `chat.show`, `chat.say` and `game.run` happen a frame later.
- Errors in any handler stop the mod, with the event's name in front: `hud: mymod.lua:8: ...`.

## Mod-to-mod events

A name with a `:` in it (at most 64 characters, e.g. `"waypoints:changed"`) is a custom event. `emit(name, ...)` queues it with up to 16 values (tables are passed as they are, not copied); on the next frame every running mod's handlers for that name run, the emitting mod's own included, then the console's. By convention the part before `:` is the id of the mod that emits it.

```lua
-- in mod "waypoints"
emit("waypoints:changed", "home", { x = 10, y = 64, z = -3 })

-- in any mod
on("waypoints:changed", function(name, p)
  chat.show("Waypoint " .. name .. " is now at " .. p.x .. " " .. p.y .. " " .. p.z)
end)
```
"##;

const LANGUAGE: &str = r##"
Horizon Studio runs its own Lua interpreter, written in Rust, with the syntax and semantics of Lua 5.3 except where this page says otherwise. Mods are sandboxed: no other processes and only the functions the studio gives them; their files are their storage and the executor functions' `workspace` folder, and their network the internet only (see the `executor` topic).

## Statements

Everything in Lua 5.3 but `goto` and labels: `local`, assignment (several targets at once), `function` and `local function` (and methods with `:`), `if`/`elseif`/`else`, `while`, `repeat ... until` (the condition sees the loop body's locals), numeric `for`, generic `for ... in`, `do ... end`, `break` and `return` (the last statement of its block). A `#!` first line is skipped. Comments: `--` to the end of the line, and long comments `--[[ ... ]]` / `--[==[ ... ]==]`.

- `goto` and `::label::` are a syntax error: "goto and labels are not supported".
- `local x <const> = 1` is accepted, but not enforced (`x` can still be assigned). `<close>` is a syntax error ("unknown attribute 'close'").
- There is no `_ENV`. The globals table is `_G` (the mod's own).
- Nesting of blocks and expressions is limited to 80 levels ("chunk has too many syntax levels").

## Luau

Luau's syntax works too, so Roblox scripts run as they are:

- **Types** are read and dropped (they change nothing at run time): `local x: number = 1`, parameters and returns (`function f(a: string, ...: number): (boolean, string?)`), generics (`function f<T>(x: T): T`), `type Name<T> = ...` and `export type`, casts (`value :: any`), table, function, union, intersection, optional, literal and `typeof(...)` types. `--!strict` and other `--!` lines are comments.
- **Compound assignment**: `x += 1`, `-=`, `*=`, `/=`, `//=`, `%=`, `^=`, `..=`. The target's table and key are worked out once (`t[f()] += 1` calls `f` once), and metamethods apply.
- **`continue`** in `for`, `while` and `repeat` loops goes to the next round. It is still a name elsewhere (`local continue = 1`).
- **Interpolated strings**: `` `hello {name}, {n * 2} items` `` (each `{expression}` through `tostring`, so `__tostring` applies); `\{` and `` \` `` escape.
- **If-expressions**: `local kind = if n > 0 then "positive" elseif n < 0 then "negative" else "zero"` (the `else` is required).
- **Generalized iteration**: `for k, v in t do` goes through a table without `pairs`, or through its `__iter` metamethod.
- **Numbers**: `0b1010` binary literals and `1_000_000` digit separators.
- **Attributes** (`@native`, `@checked`) are accepted and change nothing.
- Each mod's top level, its commands, `task.spawn`/`task.defer`/`task.delay` threads and Roblox signal handlers run in coroutines of their own, so `task.wait`, `wait`, `WaitForChild` and `signal:Wait()` can wait there. `on(...)` handlers and timers run on the main thread, where waiting errors.

```lua
type Item = { name: string, price: number }
local items: { Item } = { { name = "Sword", price = 10 }, { name = "Shield", price = 15 } }
local total = 0
for _, item in items do
  if item.price > 12 then continue end
  total += item.price
end
print(`total: {total}, {if total > 5 then "pricey" else "cheap"}`)
```

## Numbers: all floats

Every number is a 64-bit float; there is no separate integer subtype.

- `3` and `3.0` are the same value. Whole numbers print without a decimal point: `3.0` prints `3`, `10 / 2` prints `5`, `7 // 2` prints `3` (real Lua 5.3 prints `3.0` for float results).
- Other numbers print with 14 significant digits: `0.1 + 0.2` prints `0.3`, `1/3` prints `0.33333333333333`, `math.pi` prints `3.1415926535898`. Whole numbers of 1e15 and more print in exponent form: `2^53` prints `9.007199254741e+15`. Use `string.format` for other forms.
- Whole numbers are exact up to 2^53. `math.maxinteger` is 2^53 - 1 (9007199254740991) and `math.mininteger` its negative, not 2^63 - 1.
- `math.type(x)` is `"integer"` for any whole number (`3.0` too) and `"float"` otherwise. `math.tointeger(x)` returns `x` if it is whole, else nil.
- `/` divides. `//` is floor division: `7 // 2` is 3, `-7 // 2` is -4, `1 // 0` is `inf` (no error, unlike integer division in Lua 5.3). `%` takes the divisor's sign: `-7 % 3` is 2, `5.5 % 2` is 1.5, `5 % 0` is `nan` (no error). `^` is power and always a float.
- Hexadecimal literals are whole numbers (`0xff` is 255); there are no hexadecimal floats (`0x1p4`). A hexadecimal literal above 2^53 loses precision and does not wrap: `0xffffffffffffffff` is 1.8446744073709552e19, not -1.
- `nan` prints as `nan` or `-nan`; infinities as `inf` and `-inf`.
- Arithmetic converts strings to numbers (`"10" + 5` is 15, `"3" * "4"` is 12); a string that is not a number raises "attempt to perform arithmetic on a string value (not a number)". `..` converts numbers to strings (`10 .. 20` is `"1020"`). Comparisons do not convert: `1 < "2"` raises "attempt to compare number with string". `1 == "1"` is false.

## Bitwise operators

`&`, `|`, `~` (xor), `<<`, `>>` and unary `~` work as in Lua 5.3 on 64-bit integers: each operand must be a whole number (or a string of one) within ±2^63; the result is turned back into a float. Shifts are logical, a negative shift goes the other way, and shifting by 64 or more gives 0. Results above 2^53 lose precision (`1 << 62` prints `4.6116860184274e+18`). A number with a fraction raises "number has no integer representation"; a table without a metamethod raises "attempt to perform bitwise operation on a table value".

```lua
local flags = 5 & 3        -- 1
local both = 5 | 3         -- 7
local x = 5 ~ 3            -- 6
local half = 256 >> 1      -- 128
local argb = (0xff << 24) | (0x40 << 16) | (0x80 << 8) | 0xc0
```

## Strings

Strings are byte strings (UTF-8 text is bytes; `#s` counts bytes; `utf8.*` counts characters). Quotes `"..."` and `'...'`, and long strings `[[...]]` / `[==[...]==]` (a newline right after the opening bracket is dropped).

Escapes: `\n \t \r \a \b \f \v \\ \" \'`, a backslash before a newline (a newline), `\xXX` (two hex digits), `\ddd` (up to three decimal digits, at most 255), `\z` (skips the following whitespace and newlines) and `\u{XXX}` (a code point as UTF-8). Any other escape is a syntax error ("invalid escape sequence"); a string that runs to the end of its line is "unfinished string".

Strings have the `string` functions as methods: `("abc"):upper()`, `s:format(...)`, `s:split(",")`. Indexing a string with any other key gives nil. A string is at most 4 MiB; making a longer one stops the mod ("string is too long").

## Tables

- Tables keep an array part for keys 1..n and the other keys in insertion order: `pairs` visits 1..n first, then the other keys in the order they were added (real Lua's order is undefined; here it is stable).
- `#t` is the length of the array part: the count of keys 1, 2, 3... up to the first nil. A table constructor with a nil hole stops there: `#{1, 2, nil, 4}` is 2. Setting `t[#t] = nil` shortens it.
- Whole float keys are the same as integer keys (`t[1.0]` is `t[1]`). A nil or NaN key raises "index is nil" / "index is NaN".
- A table holds at most 1,048,576 entries; more stops the mod ("table is too large").

## Functions

Closures with upvalues, varargs (`...`, `select("#", ...)`, `{...}`, `table.pack(...)`), multiple results, methods (`obj:name(...)` passes `obj` as `self`). Tail calls are ordinary calls: they count toward the limit of 200 nested Lua calls (deep recursion raises "stack overflow"). Calls back into Lua from a built-in (a metamethod, `table.sort`'s comparator, `string.gsub`'s function) also stop at about 100 deep, to keep the game's stack safe. A chunk's own `...` are the values it was called with: a mod's main file gets none; a file loaded by `require` gets its module name.

## Metatables

`setmetatable` and `getmetatable` work on tables (`getmetatable("")` is nil: strings have no metatable to change). Metamethods:

- `__index` (a table or a function(t, k)), `__newindex` (a table or a function(t, k, v)); chains of tables are followed up to 64 deep.
- `__call` (a function; called with the table first).
- Arithmetic: `__add`, `__sub`, `__mul`, `__div`, `__mod`, `__pow`, `__idiv`, `__unm`; bitwise: `__band`, `__bor`, `__bxor`, `__shl`, `__shr`, `__bnot`; `__concat` (when either side is not a string or number); `__len` (`#t`).
- Comparison: `__eq` (only when both values are tables), `__lt`, `__le`. `__le` does not fall back to `__lt`: define both for `<=` and `>=`. `a > b` uses `__lt(b, a)`.
- `__tostring` (used by `tostring`, `print`, `chat.show`...; must return a string or a number), `__pairs` (used by `pairs`), `__metatable` (`getmetatable` returns it, and `setmetatable` refuses: "cannot change a protected metatable").
- Not supported: `__gc`, `__mode` (no weak tables), `__close`, `__name`.

```lua
local Vec = {}
Vec.__index = Vec
Vec.__add = function(a, b) return Vec.new(a.x + b.x, a.y + b.y) end
Vec.__tostring = function(v) return "(" .. v.x .. ", " .. v.y .. ")" end
function Vec.new(x, y) return setmetatable({ x = x, y = y }, Vec) end
function Vec:length() return math.sqrt(self.x ^ 2 + self.y ^ 2) end

local v = Vec.new(3, 4) + Vec.new(0, 0)
print(tostring(v), v:length()) -- (3, 4)  5
```

## Coroutines

`coroutine.create`, `resume`, `yield`, `status`, `wrap`, `isyieldable`, `running` and `close`, as in Lua 5.3 (`close` as in 5.4). A coroutine can yield anywhere in its own Lua code, inside `pcall` and `xpcall` too, but not from inside a function a built-in calls back (a metamethod, `table.sort`'s comparator): that raises "attempt to yield across a C-call boundary". An error inside a `coroutine.wrap` function comes out of the call with where it was called from in front.

```lua
local gen = coroutine.wrap(function()
  for i = 1, 3 do coroutine.yield(i) end
end)
print(gen(), gen(), gen()) -- 1 2 3
```

## Errors

`error(value[, level])` raises any value; a string gets `file:line: ` in front (level 1, the default, is where `error` was called; 2 its caller; 0 adds nothing). `pcall(f, ...)` returns `true, results...` or `false, error`; `xpcall(f, handler, ...)` calls `handler(error)` and returns `false` and its results. Built-in errors are strings like `mymod.lua:3: attempt to call a nil value (global 'foo')`. An uncaught table error shows as "(error object is a table value)". Running out of steps, a too long string and a too large table cannot be caught.

## The step budget

Every statement, loop round and call costs one step; string and table functions that loop cost one step for each 16 bytes or elements. A mod's main file gets 10,000,000 steps, each handler, timer and callback 2,000,000, a console line 20,000,000. Out of steps, the script stops with "script ran too long and was stopped (an endless loop?)", which `pcall` cannot catch.

## Not there

No `goto`, no `<close>`, no `_ENV`, no `io`, no `debug`, no `os` beyond `clock`, `time`, `date` and `difftime`, no `string.pack`/`unpack`/`packsize`/`dump`, no `dofile`, `loadfile` or `loadstring` (`load` takes text only), no binary chunks, no weak tables or finalizers; `collectgarbage` does nothing. `require` is the studio's own: it loads the files of a module, each once (see `require`).
"##;

const STDLIB: &str = r##"
Every standard function a mod (and the console) can call, as Lua 5.3 has it unless its entry says otherwise. All are in each mod's own environment.

- Base: `print`, `type`, `tostring`, `tonumber`, `pairs`, `ipairs`, `next`, `select`, `rawget`, `rawset`, `rawequal`, `rawlen`, `setmetatable`, `getmetatable`, `error`, `assert`, `pcall`, `xpcall`, `load`, `unpack` (as Lua 5.1, also `table.unpack`), `collectgarbage` (does nothing), and the studio's `dump` and `inspect` (a value as readable text). Values: `_G`, `_VERSION`.
- `string`: `len`, `sub`, `upper`, `lower`, `rep`, `reverse`, `byte`, `char`, `format`, `find`, `match`, `gmatch`, `gsub` with Lua patterns, and the studio's `split`, `trim`, `startswith`, `endswith`. Strings have them as methods (`s:upper()`).
- `math`: `abs`, `ceil`, `floor`, `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `exp`, `log`, `fmod`, `modf`, `max`, `min`, `random`, `randomseed`, `tointeger`, `type`, `rad`, `deg`, `ult`, the studio's `clamp`, and from Lua 5.1/5.2 `atan2`, `pow`, `log10`, `sinh`, `cosh`, `tanh`, `frexp`, `ldexp`. Values: `pi`, `huge`, `maxinteger`, `mininteger`. Luau's `sign`, `round`, `noise`, `lerp`, `map`, `isnan`, `isinf`, `isfinite`.
- Luau's: `typeof`, `table.find`, `table.create`, `table.clear`, `table.clone`, `table.freeze`, `table.isfrozen`, `table.maxn`, `table.getn`, `table.foreach`, `table.foreachi`, and `bit32` (`band`, `bor`, `bxor`, `btest`, `bnot`, `lshift`, `rshift`, `arshift`, `lrotate`, `rrotate`, `extract`, `replace`, `countlz`, `countrz`, `byteswap`). Roblox's data types (`Vector3`, `CFrame`, `UDim2`, `Color3`...) and `Enum` are in the `roblox` topic.
- `table`: `insert`, `remove`, `concat`, `sort`, `unpack`, `pack`, `move`.
- `utf8`: `char`, `codepoint`, `len`, `offset`, `codes`, and `charpattern`.
- `os`: `clock`, `time`, `date`, `difftime` (UTC only; no other `os` function).
- `coroutine`: `create`, `resume`, `yield`, `status`, `wrap`, `isyieldable`, `running`, `close`.
- `json` (`encode`, `decode`) is part of the API (see `json.encode`).

Differences from Lua 5.3 that matter most:

- All numbers are floats (see `language`): whole floats print without `.0`, `math.type(3.0)` is `"integer"`, `math.maxinteger` is 2^53 - 1.
- `print` writes to the studio's console log, not to the chat; a mod's lines start with `[<id>] `.
- `pairs` visits keys in a stable order: 1..n, then insertion order.
- `table.sort` is a stable merge sort; an inconsistent comparator cannot crash it.
- `string.format` has no `%a`; width and precision are at most 99.
- `math.random(0)` raises "interval is empty" (Lua 5.3 returns a random integer).
- `os.clock()` is wall-clock seconds since the studio's interpreter started, not CPU time; `os.date` and `os.time` work in UTC.
- `load` takes a string only (no function, no binary chunks); its environment is the mod's globals unless given.
"##;

const SCREENS: &str = r##"
`ui.open(spec)` shows a screen over the game: a centred panel of widgets, or a canvas the mod draws itself. While it shows, the keyboard and mouse go to the screen (no `on("key")` events reach mods). One script screen shows at a time.

```lua
local volume, name = 5, ""
command("settings", function()
  ui.open({
    title = "Settings",
    width = 280,
    widgets = {
      { type = "label", text = "Changes apply at once." },
      { type = "slider", text = "Volume", min = 0, max = 10, step = 1, value = volume,
        change = function(v) volume = v end },
      { type = "input", text = "Name", value = name, placeholder = "your name",
        submit = function(v) name = v; chat.show("Hello, " .. v) end },
      { type = "space" },
      { type = "button", text = "Done", primary = true, click = function() ui.close() end },
    },
  })
end)
```

## The spec table

The studio reads the table again every frame: change a field (or a widget's `text`, `value`...) and the screen shows the change. Fields:

| Field | Type | What |
| --- | --- | --- |
| `title` | string | the panel's heading |
| `width` | number | the panel's width in dp (default 340, kept within 200 and the screen's width - 32) |
| `widgets` | list of tables | the widgets, top to bottom (at most 64; see the `widget:*` entries) |
| `canvas` | boolean | true: no panel and no widgets; `draw` draws everything with `gfx.*`, and every coordinate is in dp |
| `backdrop` | boolean | false: no dimmed backdrop over the game |
| `draw(w, h)` | function | every frame, after the widgets: `w`, `h` are the screen's size in GUI pixels (dp on a canvas); `hud.*` and `gfx.*` draw here |
| `click(x, y, button)` | function | a press that is not on a widget (and right and middle presses on widgets); `button` is `"left"`, `"right"` or `"middle"` |
| `release(x, y, button)` | function | a mouse button released, anywhere |
| `move(x, y)` | function | the pointer moved |
| `wheel(steps)` | function | the wheel turned (positive is up), unless over a slider |
| `key(key, down, mods)` | function | a key went down or up while no text field has focus (names below); `mods` is `{ctrl, shift, alt}`, the modifier keys held |
| `text(t)` | function | text typed while no text field has focus |
| `escape()` | function | Escape was pressed while no text field has focus: return true to keep the screen open (a text field of the screen's own lets go of its focus this way); otherwise the screen closes |
| `close()` | function | the player closed the screen (Escape, or the studio's key) |

`click`, `release` and `move` get GUI pixels on a panel screen and dp on a canvas. All of these callbacks are optional, on any screen. Their return values are ignored, but for `escape`'s.

## Keys on a screen

`key(key, down, mods)` gets short names, not the `key.keyboard.*` names of `on("key")`: `"enter"`, `"backspace"`, `"delete"`, `"tab"`, `"left"`, `"right"`, `"up"`, `"down"`, `"home"`, `"end"`, `"pageup"`, `"pagedown"`, a lowercase letter `"a"`..`"z"` or digit `"0"`..`"9"`, `"/"`, `" "` (Space), `` "`" `` (the backquote), and `"other"` for the rest (the Menu key too). Escape and the studio's key (F8) close the screen when pressed (unless `escape()` keeps it open), so `key` only sees them released (`"escape"`, `"toggle"`). `mods.ctrl`, `mods.shift` and `mods.alt` say which modifier keys are held (Control+V is `"v"` with `mods.ctrl`). `text(t)` gets typed characters (not while Control is held). Function keys stay with the game, but for Shift+F10, which comes as the Menu key does (`"other"`).

## Focus

Clicking an `input` widget focuses it: typed text, Backspace, Delete, arrows, Home and End edit it, Control+V pastes, Tab moves to the next input, Enter calls its `submit` and drops the focus, Escape drops the focus (a second Escape closes the screen). Clicking anything else drops the focus. While an input has focus, the screen's `key` and `text` get nothing.

## Opening and closing

- `ui.open` takes effect at the end of the frame: `ui.is_open()` is false right after it, true from the next frame. In the game the screen opens over the studio's screen if that is open, else it takes the pointer through the game's chat screen, which only works while playing in a world (otherwise the request is dropped). While it shows, `game.screen()` is `"chat"` in the game (`"script"` headless).
- Calling `ui.open` while a screen shows replaces it (the old one's `close` is not called).
- `ui.close()` closes this mod's screen at once without calling `close`; the player closing it calls `close`.
- An error in any callback stops the mod and closes its screen (`screen: <message>`). When the mod stops or reloads, its screen goes.
- `gfx.mouse()` gives the pointer in dp and the buttons while the mod's screen shows.

## The panel's layout

The panel is centred. Its rows, in dp: a title 54 (18 without one), `label` 21 per wrapped line + 6, `button` 44, `toggle` 40, `slider` 50, `input` 62, `space` its `height` (default 12), and 18 at the bottom. Widgets are as wide as the panel less 48.

## Canvas screens

With `canvas = true` the mod draws the whole screen in `draw(w, h)` with `gfx.*` (dp) and handles the mouse and keys itself. The bundled `uikit` library builds windows, buttons, toggles, sliders, text fields, tabs and lists this way (see `libraries`).

```lua
local px, py = 100, 100
command("canvas", function()
  ui.open({
    canvas = true,
    draw = function(w, h)
      gfx.rect(px - 20, py - 20, 40, 40, "#9a86ff", 20)
      gfx.text(16, 16, "Click to move the dot, Escape to close", "#ffffff", 16, "medium")
    end,
    click = function(x, y, button)
      if button == "left" then px, py = x, y end
    end,
  })
end)
```

Sounds: a screen's `draw` may play sounds (UI libraries click as they draw); in the game they play at the start of the next frame, as the screen is drawn while the game is only read.
"##;

const GFX: &str = r##"
Two families draw on the screen, both only inside `on("hud", fn)` or a screen's `draw(w, h)`.

## hud.*: GUI pixels and the game's font

- Units are GUI pixels: the game's GUI scale applies. `hud.size()` and the `hud` handler's `w, h` give the GUI's size; a panel screen's `draw(w, h)` gets it too.
- `hud.text(x, y, text[, color[, shadow]])` writes with the game's own font (lines are about 9 pixels tall; `hud.panel` puts them 10 apart); `y` is the top of the text; `hud.width(text)` measures it. Its color's alpha is ignored; a shadow is drawn unless `shadow` is `false`.
- `hud.rect(x, y, w, h[, color])` fills a rectangle (default `0x78000000`, translucent black).
- `hud.panel(corner, lines)` is different: it works anywhere, and the studio draws the box every frame until it is cleared.

## gfx.*: dp and Horizon's design fonts

- Units are dp, Horizon's logical pixels: `gfx.size()` gives the screen in dp. There are a few dp per GUI pixel (headless tests use 2: a 480 x 270 GUI is 960 x 540 dp). Canvas screens use dp for everything.
- `gfx.text(x, y, text[, color[, size[, font]]])` writes in Geist (`"regular"`, `"medium"`; `"bold"` is medium) or Geist Mono (`"mono"`), `size` 6 to 96 dp (default 14). The line box is 1.45 x size tall and `y` is its top: to centre text on `cy`, use `y = cy - size * 1.45 / 2`. It returns the width drawn; `gfx.measure(text[, size[, font]])` gives the width without drawing. Text is cut at 1024 bytes.
- `gfx.rect(x, y, w, h[, color[, radius]])` and `gfx.border(...)` draw rounded rectangles and 1 dp outlines; the radius is kept within half the smaller side; nothing is drawn when `w` or `h` is 0 or less. Default color white.
- `gfx.mouse()` gives the pointer in dp while this mod's screen shows.

## Colors

Every color argument in the API (and a widget's `color`, a model box's `color`) takes:

| Form | Means |
| --- | --- |
| `"#rrggbb"` or `"rrggbb"` | opaque |
| `"#aarrggbb"` or `"aarrggbb"` | with alpha: `00` transparent, `ff` opaque (`"#80000000"` is half-transparent black) |
| a number | `0xAARRGGBB` exactly as given: `0xffff0000` is opaque red, but `0xff0000` has alpha 0 and draws nothing |
| `color(r, g, b[, a])` | builds that number from 0..255 values (alpha defaults to 255) |

`"#rgb"` is short for `"#rrggbb"` (`"#fff"` is white). `hud.text` and `draw.text` ignore alpha. Anything else raises `bad argument #N to 'color' ('#rgb', '#rrggbb' or '#aarrggbb' expected)` or `(number or '#rrggbb' expected)`.

```lua
on("hud", function(w, h)
  local x, y = 8, h - 40
  hud.rect(x, y, 104, 14, "#80000000")
  local health, max = player.health()
  if health then
    hud.rect(x + 2, y + 2, 100 * health / max, 10, color(220, 60, 60))
    hud.text(x + 4, y + 3, string.format("%d / %d", health, max), "#ffffff")
  end
  local sw = gfx.size()
  local label = "Horizon"
  local tw = gfx.measure(label, 18, "medium")
  gfx.rect(sw - tw - 36, 12, tw + 24, 36, "#cc16181f", 12)
  gfx.text(sw - tw - 24, 30 - 18 * 1.45 / 2, label, "#ecedf3", 18, "medium")
end)
```
"##;

const WORLD: &str = r##"
## Reading the world

- `world.block(x, y, z)`: the block id at a position (`"minecraft:stone"`).
- `world.time()`: the day time (0 to 23999; 0 is sunrise, 6000 noon, 12000 sunset, 18000 midnight), the game time in ticks, and the day number.
- `world.find(block[, radius])`: the nearest blocks of a kind, and how many there are.
- `world.entities([radius[, type]])`: mobs, dropped items and experience orbs near the player.
- `player.*` reads the player (see `api`).

`world.find` and `world.entities` ask Horizon's agent, so they work only where the agent does: not in `on("hud")`, and in the game not in `on("world")` or a screen's draw; headless tests cannot run them at all (see `agent`).

## Drawing in the world: draw.*

`draw.*` work only inside `on("world", fn)`, which runs every frame as the world is drawn. What a handler draws lasts that frame: draw it again each frame.

- Solid shapes (`draw.line`, `draw.box`, `draw.block`, `draw.fill`, `draw.beam`) are quads in the world: hidden behind blocks, seen from both sides, full bright (not lit by the world). A line is a flat ribbon `width` blocks wide that faces the camera.
- `through = true` on `draw.line`, `draw.box` and `draw.block` draws the lines on the HUD instead, over everything (seen through walls), `width x 40` GUI pixels wide (1 to 8). `draw.text` labels are always drawn that way: centred on the point, with a translucent black background, and `distance = true` adds `(12 m)`.
- `draw.item` draws an item or block model (see `models`).
- Per frame, all mods together may draw 16384 quads (a line is 1, a box outline 12, a filled box 6, a beam 4, an item 1, each model box 6) and 4096 through-wall lines and labels; past that, `draw.*` raise "too much drawn in the world this frame" / "too much drawn through walls this frame".
- Colors are as in `color`; the default is opaque white. A number color without alpha (`0xff0000`) is invisible.

```lua
local target = nil
command("mark", function()
  local x, y, z = player.block()
  if not x then return "Not in a world" end
  target = { x = x, y = y, z = z }
  return "Marked " .. x .. " " .. y .. " " .. z
end)

on("world", function()
  if not target then return end
  local t = target
  draw.beam(t.x + 0.5, t.z + 0.5, "#809a86ff", 0.3)
  draw.block(t.x, t.y, t.z, "#9a86ff", 0.03, true)
  draw.text(t.x + 0.5, t.y + 2, t.z + 0.5, "Mark", "#ffffff", true)
end)
```

## Particles

`world.particle(id, x, y, z[, dx, dy, dz[, count[, color]]])` spawns particles for the player only (`"flame"`, `"heart"`, `"note"`, `"dust"` with a color...). It does not work while drawing (`hud`, `world`, a screen's draw): call it from `tick`, `frame`, a command or a callback. At most 512 particles a frame for all mods. Particles that need a block or an item (`block`, `block_crumble`, `falling_dust`, `dust_pillar`, `block_marker`, `item`, `vibration`, `trail`) are refused in the game.

## Sounds

`sound.play(id[, volume[, pitch]])` plays a sound event for the player; `sound.at(id, x, y, z[, volume[, pitch]])` from a place. Ids are sound events such as `"minecraft:block.note_block.bell"` or `"entity.experience_orb.pickup"` (`minecraft:` is added when there is no `:`). Volume 0 to 4, pitch 0.5 to 2. Not in `on("hud")` or `on("world")`; at most 32 sounds a frame.
"##;

const MODELS: &str = r##"
`model.attach(part, spec)` puts an item, coloured boxes or a textured mesh (topic `meshes`) on a body part of the player (or of another entity) so that it follows the part as it moves and animates, every frame, until removed. `draw.item` draws an item model at a fixed place for one frame.

## Body parts

The player's parts: `root` (the whole model), `head`, `body`, `right_arm`, `left_arm`, `right_leg`, `left_leg`, and the outer layers `hat`, `jacket`, `right_sleeve`, `left_sleeve`, `right_pants`, `left_pants` (which move with the part under them). For another entity (`target` = an entity id from `world.entities`), use its own model's part names; a name it does not have draws nothing.

## A part's frame and the units

A part has a frame each frame: its origin is the part's pivot, and it has three axes, `left`, `up` and `forward`, each one block long in a block-sized model (a model pixel is 1/16 of an axis; the player's model is 15/16 the size of a block-sized one, so a player pixel is 0.9375/16 blocks). `player.part(name[, target])` returns that frame: `{x, y, z, left = {x, y, z}, up = {...}, forward = {...}}`.

Pivots of the player's parts: `head`, `hat`, `body` and `jacket` turn at the neck (24 pixels above the feet; the head is 8 pixels tall above it, the body 12 pixels below it); the arms at the shoulders (5 pixels to the side, 2 below the neck); the legs at the hips (1.9 pixels to the side, 12 above the feet).

What a mod gives is in pixels and degrees, from the pivot:

- `offset = {right, up, forward}` moves the model.
- A box is `{from = {x, y, z}, to = {x, y, z}, color = ...}`: two opposite corners in pixels, x to the part's right, y up, z forward. On the head, `{-4, 0, -4}` to `{4, 8, 4}` is the head itself; on the body, `{-4, -12, -2}` to `{4, 0, 2}` is the torso.
- `rotation = {pitch, yaw, roll}` in degrees: pitch tips the top forward, yaw turns it to its left, roll tilts it to its right (yaw outermost, then pitch, then roll).
- `scale` is a number, or `{x, y, z}` per axis.
- Vectors may be lists `{1, 2, 3}` or tables `{x = 1, y = 2, z = 3}`; each number at most 4096 either way.

## Items and their display context

An item is shown as the game shows it in one of its display contexts: `"head"` (worn as the game places a helmet item: 4 pixels above the pivot, at 0.625 size), `"hand"` and `"left_hand"` (held in a right or left hand: 10 pixels down the arm and 2 forward), `"fixed"` (as in an item frame), `"ground"` (as a dropped item), `"gui"` (as in the inventory). Without `display`, an item on `head` or `hat` uses `"head"`, on `right_arm`/`right_sleeve` `"hand"`, on `left_arm`/`left_sleeve` `"left_hand"`, and anywhere else `"fixed"`. The mod's offset, rotation and scale apply from there. Items are lit by the world's light where the part is, unless `glow = true`. Item ids without a namespace get `minecraft:`; blocks work too (`"minecraft:carved_pumpkin"`).

## First person

In first person the game does not draw the player's own body, so the player's attachments are hidden then, unless `first_person = true`: then they are drawn where the part is, around the camera (good for halos and orbiting things, not for hats, which would cover the view). Other entities' attachments always show. `player.camera()` says which view is on (`"first"`, `"third_back"`, `"third_front"`; F5 changes it), and `player.pose()` the pose.

## Limits

64 attachments per mod, 256 boxes each; boxes count toward the frame's 16384 world quads (6 each), a mesh's triangles 1 each. Ids come from one counter for the whole studio: an id is never reused in a session.

```lua
-- A pumpkin hat, a sword in the right hand and a red cape.
local hat = model.attach("head", { item = "minecraft:carved_pumpkin" })
local sword = model.attach("right_arm", { item = "minecraft:diamond_sword" })
local cape = model.attach("body", {
  boxes = { { from = { -5, -16, -3 }, to = { 5, 0, -2 }, color = "#9b1b30" } },
  rotation = { -8, 0, 0 },
})
command("hat", function()
  local on = model.remove(hat)
  if not on then hat = model.attach("head", { item = "minecraft:carved_pumpkin" }) end
  return on and "Hat off" or "Hat on"
end)
```
"##;

const MESHES: &str = r##"
Real 3D models: triangles with a texture, drawn in the world (`draw.mesh`) or on a body part (`model.attach{mesh = id}`), lit by the world where they are. Every mod keeps its own textures and meshes, by id, until it reloads (or `texture.free`, `mesh.free`).

## Textures

A texture is RGBA pixels, at most 512 a side, `x` and `y` from 0 at the top left. Make one:

- `texture.load("skin.png")`: a PNG file in this folder mod (`mods/<id>/skin.png`; any folder under it).
- `texture.decode(data)`: a PNG as base64 text (what an agent writes into a Lua file), a `data:image/png;base64,...` URL, or the raw bytes.
- `texture.create(w, h[, color])`, then `texture.set`, `texture.fill` and `texture.paint` (rows of characters and a palette: pixel art as text).

Colours are what `hud.rect` takes: `"#rgb"`, `"#rrggbb"`, `"#aarrggbb"`, `color(r, g, b[, a])`, or a number `0xAARRGGBB` (a number without its alpha byte, `0xff0000`, is transparent: write `0xffff0000`).

The game keeps textures in its interface atlas: a texture shows from the frame after it is first drawn, and after a change (`set`, `fill`, `paint`) the next frame still shows the old pixels. All mods' textures together are at most 1048576 pixels.

## Meshes

A mesh is triangles in pixels, 16 to a block, each corner with texture coordinates `u`, `v` from 0 to 1 across the texture from its top left. Its origin is where `draw.mesh` puts it or the body part's pivot. Meshes face `+z`: `draw.mesh` at a yaw (as `player.look()` gives it) faces that way, and on a body part a mesh's x, y and z are the part's left, up and forward. At yaw 0, then, x is east, y is up and z is south.

Make one:

- `mesh.new{vertices = ..., faces = ...}`: corners and faces (triangles, or polygons split into triangles), `uvs` for texture coordinates.
- `mesh.new{boxes = ...}`: boxes, each textured with Minecraft's box UV (`uv = {u, v}`: the layout of a skin's head, body and limbs, front facing +z) or with a rectangle per face (`faces = {north = {u1, v1, u2, v2}, ...}` in texture pixels; north is -z, south +z, east +x, west -x, up, down).
- `mesh.load(file)`, `mesh.parse(text, format)`: a Wavefront `.obj` (in blocks: times 16 unless `scale` says otherwise; `v`, `vt` and `f` lines) or a Blockbench `.bbmodel` (cubes, meshes, groups with their pivots and turns, and its embedded textures, which become this mod's). Blockbench models face north; they are turned half round to face +z unless `flip = false`.

Each triangle drawn counts 1 toward the frame's 16384 world shapes, shared by all mods. A mesh has at most 65536 triangles; a mod, 256 meshes and 256 textures.

## Drawing

```lua
local skin = texture.create(64, 64, "#3b7dd8")
texture.fill(skin, 8, 8, 8, 8, "#f1c27d")          -- the head's front
local head = mesh.new({ texture = skin, boxes = {
  { from = { -4, 0, -4 }, to = { 4, 8, 4 }, uv = { 0, 0 } },
} })
on("world", function()
  local x, y, z = player.pos()
  local yaw = player.look()
  if x then draw.mesh(head, x + 2, y, z, { yaw = yaw }) end
end)
command("hat", function() model.attach("head", { mesh = head, offset = { 0, 8, 0 } }) end)
```

Tests: `expect_mesh` (at least N triangles drawn in a step; `false` for none).
"##;

const INPUT: &str = r##"
## Key names

`on("key", fn(key, down))` and `keys.down(key)` use the game's vanilla names:

- Letters and digits: `key.keyboard.a` ... `key.keyboard.z`, `key.keyboard.0` ... `key.keyboard.9`.
- `key.keyboard.space`, `enter`, `escape`, `backspace`, `tab`, `minus`, `equal`, `left.bracket`, `right.bracket`, `backslash`, `semicolon`, `apostrophe`, `grave.accent`, `comma`, `period`, `slash`, `caps.lock` (each after `key.keyboard.`).
- `key.keyboard.f1` ... `key.keyboard.f24`, `print.screen`, `scroll.lock`, `pause`, `insert`, `home`, `page.up`, `delete`, `end`, `page.down`, `right`, `left`, `down`, `up`, `num.lock`, `menu`.
- Keypad: `key.keyboard.keypad.0` ... `keypad.9`, `keypad.divide`, `keypad.multiply`, `keypad.subtract`, `keypad.add`, `keypad.enter`, `keypad.period`, `keypad.equal`, `keypad.comma`.
- Modifiers: `key.keyboard.left.shift`, `right.shift`, `left.control`, `right.control`, `left.alt`, `right.alt`, `left.win`, `right.win`.
- Mouse buttons: `key.mouse.left`, `key.mouse.right`, `key.mouse.middle`, `key.mouse.4` (back), `key.mouse.5` (forward), and `key.mouse.6`... for others.

These are physical keys: `key.keyboard.w` is the W key whatever the player bound to it. For what the player bound (jump, attack...), use `keys.mapping`.

## When on("key") fires

- Keys go down and up on any screen of the game, including while the player types in the chat box (check `game.screen() == "playing"` to react only while playing). Mouse buttons only while playing. Held keys do not repeat.
- Not while the studio's screen or a script screen is open: those take the keyboard and mouse (a screen gets its own `key(key, down, mods)`, see `screens`).
- Handlers run on the next frame.

## keys.down and keys.mapping

- `keys.down(key)` is true while a key is held, from the key events the studio saw (so a key pressed while a screen was open is not seen until it is pressed again).
- `keys.mapping(name)` returns whether a vanilla mapping is held and the label of its key as Controls shows it. Names: `key.forward`, `key.left`, `key.back`, `key.right`, `key.jump`, `key.sneak`, `key.sprint`, `key.attack`, `key.use`. In Horizon, the held state of the movement mappings follows their default keys (W, A, S, D, Space, Left Shift, Left Control) and attack and use follow the mouse buttons.

## Testing keys

The test step `{"key": "key.keyboard.r"}` presses and releases that key: `on("key")` handlers get exactly the name given, so test with the names the game sends (`key.keyboard.r`). `{"key": "key.jump"}` makes `keys.mapping("key.jump")` held in headless tests, but `on("key")` then gets `"key.jump"`, a name the game never sends.

```lua
local zoomed = false
on("key", function(key, down)
  if key == "key.keyboard.z" and down and game.screen() == "playing" then
    zoomed = not zoomed
    view.zoom(zoomed and 4 or 1)
  end
end)
```
"##;

const STORAGE: &str = r##"
`storage.set(key, value)` keeps a value on disk for this mod; `storage.get(key)` reads it back, after reloads and game restarts.

- What can be stored: nil (deletes the key), booleans, numbers (also `inf` and `nan`), strings, and tables of these (keys and values), nested at most 16 deep. Functions cannot ("functions cannot be stored"); a table that refers to itself fails ("tables nested too deep to store (or a cycle)"); a table that writes out to more than 256 KiB fails ("value too large to store").
- `storage.get` returns a new copy each time: changing it changes nothing on disk until `storage.set`. Tables that were the same table twice come back as two copies. It reads the file on every call: read once at load into a local, and write when something changes.
- Keys: 1 to 64 characters of letters, digits, `_`, `-` and `.`, not starting with `.`. A mod keeps at most 256 keys.
- Where: `<studio folder>/storage/<mod id>/<key>.lua`, each value written as a Lua literal (the console's under `storage/console/`). A library's functions run as the mod that calls them, so what a library stores goes to the caller's storage.
- Headless tests start with empty storage, in a folder of their own.

```lua
local settings = storage.get("settings") or { enabled = true, size = 13 }

local function save() storage.set("settings", settings) end

command("toggle", function()
  settings.enabled = not settings.enabled
  save()
  return settings.enabled and "On" or "Off"
end)
```
"##;

const LIBRARIES: &str = r##"
## export and import

A library is a mod that offers values to others: it calls `export(table)`, or its main file returns a table. Another mod gets it with `local kit = import("uikit")`.

- `import` returns the library's exports themselves (the same table each time, not a copy).
- A library's functions run as the mod that calls them: what they register (`on`, `command`, timers, models, screens), draw, store or print belongs to the caller, and `mod.info()` inside them describes the caller.
- Libraries load first: before loading mods, the studio scans each mod's files for `import("id")` or `import "id"` with a literal id, and loads those libraries before the mods that use them. An `import` with a computed id, or `pcall(import, "id")`, is not seen by that scan.
- When a library reloads, every mod that imported it reloads too (they hold its old exports), and theirs in turn. When a library appears, mods that failed for want of it reload. When a library is turned off or deleted, its users reload (and then fail on `import`).
- `import` records the library even when it fails, so a mod retries by itself when the library comes.
- Two mods that import each other cannot both load: one gets "library 'x' isn't loaded yet (do they import each other?)".

```lua
-- mods/greetings.lua: a library
local count = 0
export({
  greet = function(name)
    count = count + 1
    return "Hello, " .. name .. " (" .. count .. ")"
  end,
})
```

```lua
-- mods/greeter.lua: a mod that uses it
local greetings = import("greetings")
command("greet", function(rest)
  return greetings.greet(rest ~= "" and rest or "you")
end)
```

## emit

`emit("mymod:event", ...)` calls every mod's `on("mymod:event", fn)` handlers on the next frame (see `events`). Use it to tell other mods something happened without importing them.

## Modules: files that require each other

A module (also called a folder mod) is a folder `<id>/` whose `main.lua` loads the module's other files with `require("lib")` (`lib.lua`) or `require("lib.util")` (`lib/util.lua`). The New prompt's Module makes one with three files, main.lua, shared.lua and lib.lua, as below.

- Each file runs once, the first time it is required, however the name is written (`"lib.util"`, `"lib/util"` and `"lib/util.lua"` are one file), with the name as its `...`; every later `require` gets the value it returned (`true` when it returned nothing). Return a table of functions from a file, and keep state several files share in one file's table (shared.lua below): every file that requires it gets the same table.
- The files share the mod's globals: a global one file sets, the others see. Their locals stay their own.
- Two files can't require each other while they load: `require: 'lib' is already loading (main.lua -> lib.lua -> shared.lua -> lib.lua): ...`. Move what both need into a third file, or `require` inside a function that runs later (a handler or a command).
- A required file's top level runs inside the `require` call, so it can't wait (`task.wait`): wait in main.lua or in a handler.
- In Roblox's style each file is also a ModuleScript under `script` (see `roblox`): `require(script.lib)` in main.lua and `require(script.Parent.shared)` in lib.lua are `require("lib")` and `require("shared")`.
- Any change to a `.lua` file of the module (up to three folders deep) reloads the whole module, main.lua first. For code shared between mods, use a library (`export` and `import`).

```lua
-- mymod/main.lua: starts the module
local shared = require("shared")
local lib = require("lib")
command("mymod", function(args) return lib.greet(args ~= "" and args or "you") end)
on("tick", function() shared.ticks = shared.ticks + 1 end)
```

```lua
-- mymod/shared.lua: one table every file gets
return { greeting = "Hello", ticks = 0 }
```

```lua
-- mymod/lib.lua: functions for the other files
local shared = require("shared")
local lib = {}
function lib.greet(name)
  return shared.greeting .. ", " .. name .. "! (" .. shared.ticks .. " ticks so far)"
end
return lib
```

## The bundled uikit library

`uikit` (installed with the studio, and available to headless tests) draws modern animated screens in plain Lua on `gfx.*` and canvas screens. `local kit = import("uikit")`. Its whole API:

- `kit.window(spec, build)` returns a window. `spec`: `title`, `subtitle`, `width` (dp, default 380), `height` (starting height), `accent` (`"#rrggbb"`), `sounds = false` (no click sounds), `close()` (called when it closes). `build(ui)` runs every frame the window shows (immediate mode): each `ui.*` call draws a widget and returns its new value, so state lives in the mod's own variables.
- Window methods: `win:open()`, `win:close()` (fades out, then calls `spec.close`), `win:toggle()`, `win:is_open()`.
- In `build(ui)`, the `ui` argument (not the global `ui`) has:
  - `ui.heading(text)`, `ui.text(text[, opts])` (`opts.size`, `opts.font`, `opts.color` as `"#rrggbb"` or `{r, g, b}`), `ui.muted(text)`, `ui.space([h])` (default 8), `ui.separator()`, `ui.row(n)` (the next n widgets share a row);
  - `ui.button(label[, opts])` returns true when clicked (`opts.primary`, `opts.danger`, `opts.height`);
  - `ui.toggle(label, value)` returns the new value;
  - `ui.slider(label, value, min, max[, opts])` returns the new value (`opts.step`, `opts.format` for `string.format`);
  - `ui.input(label, value[, opts])` returns the text and true when Enter was pressed (`opts.placeholder`, `opts.max` length, default 256);
  - `ui.tabs(items, current)` returns the chosen index; `ui.list(items, selected[, opts])` returns the selected index (`opts.rows`, default 5; scrolls with the wheel); `ui.progress(value[, opts])` draws a 0..1 bar (`opts.color`);
  - `ui.close()`, `ui.width()` (the content width), `ui.dt()` (seconds since the last frame), `ui.window` (the window).
- `kit.card(x, y, lines[, opts])` draws a card of text lines inside `on("hud")` (negative x or y count from the right or bottom edge; `opts.title`, `width`, `size`, `font`, `alpha`); it returns its width and height.
- Helpers: `kit.theme` (its palette of `{r, g, b}` colours), `kit.rgb("#rrggbb")`, `kit.mix(a, b, t)`, `kit.paint(c[, alpha])` (a color number), `kit.ease(t)`, `kit.approach(v, target, dt[, speed])`, `kit.fit(text, w, size[, font])`, `kit.wrap(text, w, size[, font])`, `kit.version`.

```lua
local kit = import("uikit")
local loud, level, name = false, 5, ""
local win = kit.window({ title = "My tool", subtitle = "Made with uikit", width = 360 }, function(ui)
  loud = ui.toggle("Loud", loud)
  level = ui.slider("Level", level, 0, 10, { step = 1 })
  local submitted
  name, submitted = ui.input("Name", name, { placeholder = "type, then Enter" })
  if submitted then chat.show("Hi " .. name) end
  ui.row(2)
  if ui.button("Save", { primary = true }) then storage.set("level", level) end
  if ui.button("Close") then ui.close() end
end)
command("tool", function() win:open() end)
```

uikit plays a click sound from inside the screen's draw; see the note at the end of `screens` (pass `sounds = false` to turn it off).
"##;

const CONSOLE: &str = r##"
The studio's Console tab runs Lua at once in the running game, like a Lua executor: type a line and press Enter. An expression's values are shown (`player.pos()` shows `12.5, 64, -3.2`); statements run. `/lua <code>` in chat does the same and answers in chat. The MCP tool `run_lua` runs code there too (live), or in a headless console.

- The console has its own environment: its globals stay between lines, and it does not see mods' globals. Get at a mod through its exports: `import("mymod")`.
- A console line has 20,000,000 steps.
- What the console registers (`on`, `command`, `every`, `hud.panel`, `model.attach`...) lasts until `clear()`. A console handler that fails is removed; the console stays.
- The console's `print` lines have no `[id]` prefix, and its `chat.show` lines no `[name]` prefix.
- Only the console has the functions below; in a mod they are nil.
- The Editor tab's Ctrl+Enter runs the open file in the console. On a file of a module, whose `require` the console can't do, it saves the file and reloads the module instead.

```lua
help("player")          -- every function with "player" in its name
player.info()           -- an expression: its value is shown, expanded
mods()                  -- every mod and its state
reload("waypoints")     -- reload one mod
```
"##;

const AGENT: &str = r##"
Horizon has an agent that can read the world and act as the player (the same commands as `horizon-ctl`). Scripts reach part of it: reading the world and acting as the player, nothing that saves, quits or changes options.

- `agent.query(cmd)` runs a read and returns its JSON reply as a table at once (objects and arrays become tables, null becomes nil). An error reply raises an error.
- `agent.run(cmd[, fn])` starts a command; when it finishes, `fn(result)` runs, or `fn(nil, error)` (on a later frame; for instant commands, the next frame). At most 64 waiting per mod.
- `agent.stop()` cancels the running action and releases every key.
- One action (`move`, `jump`, `goto`, `break`, `place`, `interact`, `use-item`, `attack`, `wait`) runs at a time: starting another while one runs raises "busy with goto; wait for it or send stop".

Where the agent works: from `tick`, `frame`, timers, commands, `key`, `chat`, mod-to-mod events, agent callbacks and screen callbacks (not `draw`), and the console. Not in `on("hud")` ("agent functions are not available while drawing the HUD"); in the game not in `on("world")` or a screen's draw either ("the agent is not available here"). Headless tests cannot run agent commands: every one raises "Horizon's agent command '<name>' needs the game: test this part live (the game running with the studio)"; `world.find` and `world.entities` too.

## The commands scripts may use

Any other command raises "'<name>' is not an agent command scripts can use".

| Command | Kind | What it does |
| --- | --- | --- |
| `state` | read | player (position, block_pos, eye, yaw, pitch, velocity, on_ground, flying, health, food, xp, effects...), screen, world, dimension, gamemode |
| `looking-at`, `target` | read | the block or entity under the crosshair: `{kind = "block"|"entity"|"none", block, entity}` |
| `entities [RADIUS] [TYPE]` | read | mobs, dropped items and orbs near the player, nearest first (default radius 32): `{entities = {...}}` |
| `block X Y Z` | read | the block state at a position |
| `blocks X1 Y1 Z1 X2 Y2 Z2` | read | the non-air blocks of a box (at most 32768 blocks), with counts per block |
| `light X1 Y1 Z1 X2 Y2 Z2` | read | the stored sky and block light of a box |
| `find-block BLOCK [RADIUS]` | read | the nearest loaded blocks of a kind (radius 1 to 64, default 32): `{block, count, nearest = {{pos, distance}...}}` |
| `inventory` | read | hotbar, main inventory, armor, offhand, held stack and item counts |
| `screen` | read | the open screen or title menu, its buttons and slots |
| `messages [N]` | read | recent chat and system messages, the title and the action bar |
| `move forward|back|left|right SECONDS [--sprint] [--sneak] [--jump]` | action | walk relative to the view |
| `jump` | action | jump once, answering when landed |
| `goto X Y Z [--near R] [--sprint] [--direct] [--timeout S]` | action | walk to a position along a path (default within 0.6 blocks, 60 s) |
| `mouse-move DX DY` | | turn the view like mouse motion |
| `look-at X Y Z`, `look-at block X Y Z [FACE]`, `look-at entity ID` `[--over S]` | | face a point, a block's face or an entity |
| `turn DYAW DPITCH [--over S]` | | rotate the view |
| `attack [ID]` | action | swing at the crosshair's target (after facing entity ID) |
| `break X Y Z [--timeout S]` | action | mine a block (default 30 s) |
| `place X Y Z [--item ITEM]` | action | place the held item (or ITEM from the hotbar) |
| `interact X Y Z [FACE]` | action | right-click a block (open a chest, press a button) |
| `use-item [--hold SECONDS]` | action | right-click with the held item (eat, throw, draw a bow) |
| `select SLOT|ITEM` | | select hotbar slot 1-9, or the slot holding ITEM |
| `drop [all]` | | drop one of the held item, or the stack |
| `open-inventory` | | open the inventory screen |
| `click-button LABEL` | | click a button of the open screen by its label |
| `click-at X Y [right]`, `drag-at [right] X Y X Y...`, `scroll-at X Y STEPS` | | mouse actions at GUI pixels |
| `command TEXT` | | run a command (the leading / is optional) |
| `pause` | | open the pause menu |
| `wait TICKS` | action | answer after that many game ticks |
| `stop` | | cancel the running action and release every key |

```lua
command("gohome", function()
  agent.run("goto 0 64 0 --near 2 --timeout 60", function(result, err)
    if err then chat.show("Could not get there: " .. err) else chat.show("Home") end
  end)
  return "Walking home"
end)
```
"##;

const VIA: &str = r##"
ViaHorizon lets the game join servers of other Minecraft versions: every join goes through ViaProxy, which runs beside the game and translates Horizon's 26.3 to the server's version (Classic to 26.3, and Bedrock, experimental). The bundled `viahorizon` mod gives it a screen (`/via`) and `/vialog`; Options > Mods > Horizon Studio has it on the title screen. Changes apply from the next join.

- `via.status()`: `{enabled, version, label, text, ready}`.
- `via.versions()`: every version it can join as, newest first: `{{id = "1.8.x", label = "1.8.x"}, ...}`; `"auto"` asks the server.
- `via.enable(on)`: join through ViaProxy (true) or straight (false).
- `via.version(id)`: the version joins go as.
- `via.log()`: ViaProxy's last lines (up to 40), for when a join fails.

Headless tests have no ViaHorizon: `via.status()` says "Not in a headless test", `via.versions()` and `via.log()` are empty, and `via.enable`/`via.version` raise "ViaHorizon isn't available here".
"##;

const VOICE: &str = r##"
Horizon Studio is a client for Simple Voice Chat (henkelmax's mod for Fabric and NeoForge, and its Paper plugin): on a server that runs it, the player talks to and hears the other players, their voices from where they stand, louder the nearer. Both of its live versions are spoken (2.6.x, compatibility 20, and 2.7.x, compatibility 21); the version a server answered in is remembered for it.

- Push to talk (Caps Lock) is the default; voice activation sends while the voice is louder than its threshold. Mute (M) and deafen (N, SVC's "disable voice chat": nothing heard, nothing sent) are keys too. All three keys are chosen in Options > Mods > Horizon Studio, and its page has the settings: the microphone's mode, mute, deafen, the voices' volume, the microphone's gain and the threshold. They are kept in `voice.txt` in the studio's folder.
- The HUD shows the microphone while it sends, "Muted", "Deafened" or "Voice chat unavailable" (the server has voice chat, but it can't connect), and the names of those talking.
- Groups: their voices are heard (without a place); `voice.join_group` joins one.

A mod reads and changes it with `voice.*`:

- `voice.status()`: `{state, connected, unavailable, talking, muted, deafened, activation, ptt_key, volume, gain, threshold, level, players, compat, error, mic}`; `state` is `"off"` (not on a server), `"asking"`, `"none"` (the server has no voice chat), `"incompatible"`, `"connecting"`, `"connected"`, `"lost"` or `"failed"`.
- `voice.connected()`, `voice.talking()`, `voice.speakers()`.
- `voice.mute([on])`, `voice.deafen([on])`, `voice.volume([x])`, `voice.gain([db])`, `voice.threshold([db])`, `voice.activation([mode])`: each sets it when given a value, and returns it.
- `voice.groups()`, `voice.join_group(id[, password])`, `voice.leave_group()`.

Headless tests have no voice chat: `voice.status().state` is `"off"`, the reads are false or empty, and the changes raise "voice chat isn't available here".
"##;

const TESTS: &str = r##"
`test_mod` runs a list of steps against a mod and reports what each step caused. Headless (the default) it runs on a simulated game with no Horizon needed; with `live: true` it runs the same steps in the running game through the AI link.

Without `files` it tests the mod as it is in the mods folder. Give `files` to test text not yet written: a script's file, or for a module that exists only the files that change, laid over its other files as they are on disk (with `live: true` they are written first, the others kept, as `write_mod` writes them).

The step format, as the MCP server describes it:

> {STEPS_HELP}

## Steps (one action each)

| Step | What it does |
| --- | --- |
| `{"frames": N}` | runs N frames (headless: one frame is one tick, 50 ms) |
| `{"seconds": S}` | runs S seconds (20 frames a second headless; live, the game's frame rate) |
| `{"command": "/name args"}` | runs a chat command (a mod's own, or `/studio`, `/lua`), then one frame; fails when no mod has it |
| `{"chat": "<Alex> hi"}` | a chat line arrives from the server (`on("chat")` handlers), then one frame |
| `{"key": "key.keyboard.r"}` | presses the key for a frame and releases it for a frame; with `"down": true` or `false` only presses or releases. `on("key")` gets exactly this name, so use the game's names (`key.keyboard.r`); a mapping name such as `key.jump` makes `keys.mapping("key.jump")` held headless |
| `{"lua": "code"}` | runs Lua in the console: its prints and returned values are the step's output; a failed `assert` fails the step. Mods' globals are not visible there: use `import("id")` on a mod that exports |
| `{"suggest": "/name words"}` | the completions the chat would show for that line, with the cursor at its end (`"/wp "` completes the first argument, `"/wp re"` the word `re`): each is a line of the step's output, its text and then its tooltip in brackets (`remove (Forgets one)`); runs no frame. It fails when no command the studio takes has that name, or the line has no space after the name. Headless only |
| `{"player": {...}}` | changes the player: `x`, `y`, `z`, `yaw`, `pitch`, `health`, `food`, `level`, `mode`, `dimension`, `held` (an item id, or null for an empty hand), `name` and `uuid` (their profile's: a Roblox script's LocalPlayer Name and UserId), `velocity` (blocks a second) and `on_ground` (how they move, which the Roblox Humanoid's MoveDirection and state follow); `"player": null` leaves the world (no ticks, player functions return nil). Then one frame. Headless only |
| `{"block": [x, y, z], "id": "minecraft:stone"}` | sets a block (default air); runs no frame. Headless only |
| `{"entities": [{"kind": "zombie", "pos": [3, 64, 0]}, ...]}` | puts these entities near the player in place of the last ones, which Roblox scripts see in the Workspace: each a `kind` (`zombie`, `player`, `item`, `experience_orb`...), an `id` (100 on in their order unless given), `pos`, and `name`, `yaw`, `velocity`, `on_ground`, `baby`, `health`, `max_health`, `speed`, `width`, `height` and `eye` (the rest as the game has the kind), a dropped item's `item` and `count`, an orb's `value`. Then one frame. Headless only |
| `{"players": [{"name": "Alex", "entity": 12, "latency": 40}, ...]}` | the server's player list (`uuid`, `display_name` and `local` too): the other players Roblox scripts see in Players, each with the entity of its `entity` id as its Character. Then one frame. Headless only |
| `{"time": 13000}` | sets the day time; runs no frame. Headless only |
| `{"click": "label"}` | clicks text drawn on the open script screen: the first text equal to the label, else containing it (a button, toggle, slider or input row, or the screen's `click`), then one frame. Headless only |
| `{"type": "text"}` | types into the focused input (or the screen's `text`), then one frame. Headless only |
| `{"reload": "id"}` or `{"reload": true}` | reloads one mod (and its users), or all, then one frame |
| `{"camera": "first"}` | sets the view: `first`, `third_back` or `third_front`, then one frame. Headless only |
| `{"pose": "crouching"}` | sets the player's pose (`standing`, `crouching`, `swimming`...), then one frame. Headless only |
| a step with only `expect_*` keys | checks the state as it is |

## Expectations (on any step)

Strings, or lists of strings, matched as substrings of what the step caused:

- `expect_chat` / `expect_no_chat`: lines shown in the player's chat (`chat.show`, command results, the studio's messages).
- `expect_output`: a `lua` step's printed lines and returned values, as the console shows them (strings in quotes: returning `"hi"` gives `"hi"` with the quotes), or a `suggest` step's completions.
- `expect_log`: console lines (prints of mods as `[id] text`, errors, tracebacks, info).
- `expect_sent`: what was sent as the player (`chat.say` text, `game.run` commands with their `/`). A command the studio takes (a mod's, `/studio`, `/lua`) isn't sent: it runs in the studio, as in the game.
- `expect_drawn`: text drawn on the HUD or a screen during the step (`hud.text`, `gfx.text`, panels, widget labels, `draw.text` labels). Headless only.
- `expect_sound`, `expect_particle`: ids played or spawned (`minecraft:` added). Headless only.
- `expect_item`: item models drawn (`model.attach` items, `draw.item`). Headless only.
- `expect_state`: `{"id": "running" | "failed" | "off"}` for mods.
- `expect_screen`: true or false: whether a script screen is open. Headless only.
- `expect_error`: true, or a substring: the step must log an error. Without it, any error logged during a step fails the step.

## What each step reports

`{"step", "do", "ok", "failures", "output", "chat", "sent", "log", "drawn", "sounds", "particles", "items"}` (empty fields are left out). The whole report: `{"ok", "summary": "5/5 steps passed", "steps", "mods", "frames", "where": "headless"}`. A test runs at most 12000 frames (10 minutes of game time).

## The headless game

- The player stands at x 0.5, y 64, z 0.5, facing south (yaw 0, pitch 0), in `minecraft:overworld`, survival, health 20 of 20, food 20, saturation 5, level 0, slot 1, empty hand, standing, camera `third_back`.
- Every block is `minecraft:air` unless a `block` step set it. The day time starts at 1000 and every frame adds one tick.
- The GUI is 480 x 270 GUI pixels; dp are twice that (960 x 540). Text is 6 GUI pixels a character (`hud.width`); `gfx.measure` is characters x size x 0.55.
- `game.fps()` is 20, 20, 20; `game.screen()` is `"playing"`, or `"script"` while a script screen shows.
- No agent: `agent.*`, `world.find` and `world.entities` raise errors (they ask the game's agent). No entities and no other players until an `entities` or `players` step puts them there, for the Roblox layer (the Workspace, Players). No ViaHorizon.
- Sounds, particles and items are recorded, not played; ids are checked for form only.
- A command a mod sends with `game.run` that the studio takes (a mod's, `/studio`, `/lua`) runs in it, as in the game; the others would go to the server, and are noted in chat as `(the game would run /...)`.
- The mod's libraries come from the mods folder (when the tool has one), else the bundled examples (`uikit`, `viahorizon`...).

## Live tests

`live: true` writes the files (when given), reloads the mod in the game and runs the steps there. `player`, `block`, `entities`, `players`, `time`, `click`, `type`, `camera`, `pose` and `suggest` steps are headless only: live, use a command step (`/tp`, `/setblock`, `/summon`, `/time set`) or the `horizon_ctl` tool. `expect_drawn`, `expect_sound`, `expect_particle` and `expect_screen` are only noted live (look with `horizon_ctl screenshot`). Live, a `command` step waits 10 frames, a `chat` step 2, a `key` step a frame between press and release and 2 after.

## Examples

```json
[
  {"frames": 5, "expect_state": {"greeter": "running"}},
  {"command": "/greet Alex", "expect_chat": "Hello, Alex"},
  {"lua": "local g = import('greeter'); assert(g.count() == 1, 'counted once')"},
  {"key": "key.keyboard.g", "expect_chat": "pressed G"},
  {"chat": "<Steve> hi greeter", "expect_chat": "Hi Steve"},
  {"player": {"x": 100, "z": -20, "held": "minecraft:diamond_sword"}},
  {"lua": "print(player.held())", "expect_output": "minecraft:diamond_sword"},
  {"block": [0, 63, 0], "id": "minecraft:stone"},
  {"lua": "return world.block(0, 63, 0)", "expect_output": "minecraft:stone"},
  {"command": "/greetscreen", "expect_screen": true, "expect_drawn": "Greet"},
  {"click": "Greet", "expect_chat": "Hello"},
  {"command": "/broken", "expect_error": "boom"},
  {"seconds": 2, "expect_no_chat": "error"}
]
```
"##;

const MCP: &str = r##"
`horizon-studio-mcp` is an MCP server on stdio: it writes and reads mods in the game's mods folder, checks and tests them headless (the studio itself on a simulated game), and, while the game runs with Horizon Studio, works live through the studio's AI link (Lua in the console, chat and commands, reloads, the console's log). For a game started with `--control` it also reaches Horizon's own control socket (screenshots, playing).

## Workflow

1. `docs`: this reference. With no arguments, its index; then the topics the mod needs (`start`, `api`, `events`, `language`, `recipes`...), `name` for one function, event or widget, `search` for anything else.
2. `examples` for idioms (`examples` with an id gives the source).
3. `write_mod` with the files; read its `check`: whether the mod loads, its error and the file and line it is in. Fix and write again (for a module, only the files that change).
4. `test_mod` with steps, headless first. Then `game_status`; when it says `linked: true`, `test_mod` with `live: true`.
5. In the running game: `run_lua`, `send_chat`, `game_log`, `reload_mod`, and `screenshot` to see what the mod draws; with Horizon started with `--control`, `horizon_ctl`.
6. In the player's editor, while the game runs: `read_script`, then `edit_script` (it types the change in as they watch), then `save_script`; `open_script` opens another script to type into, `mark_lines` points at code with a note, and `start_lesson` with `wait_for_player` teaches (see "The player's editor (live)" below).

## The tools

- `docs` (`topic`, `name`, `search`, all optional): this reference, the newest published one: the server fetches `docs/reference.json` from the repository's main branch as it starts (kept in `~/.horizon-studio/reference.json` for when it is offline; `$HORIZON_STUDIO_DOCS_URL` names another URL or a file, `off` keeps the one built in), so the docs change without a new release. The index says which copy answers. No arguments: the index (every topic and entry name). `topic`: a topic's prose and the full entry of everything in it (`"all"`: every topic). `name`: one entry, exactly (`"player.pos"`, `"string.format"`, `"event:tick"`, `"widget:button"`). `search`: words, all of which must appear; the best matches with a line of each. The topics are also MCP resources, `horizon-studio://docs/<topic>`. Headless, no game needed.
- `examples` (`id` optional): without an id, the list of bundled examples (`id`, `title`, `description`); with one, its source.
- `game_status` (no arguments): `{"linked": true, "game": {studio, protocol, frame, screen, in_world, ticks, fps, player, mods_dir, log_seq, chat_seq, agents, mods, editor}}`, or `{"linked": false, "why", "mods_dir", "horizon_ctl"}`. `editor` is the player's editor: `{"file", "path", "mod", "dirty", "line_count", "typing", "unsaved", "marks", "lesson"}` (`typing`: an edit being typed in, `{"id", "by", "file"}`; `unsaved`: an agent's edits nobody has saved, `{"file", "by", "conn"}`, `conn` being the link connection they came over; `marks`: how many marks agents have in the editor's files; `lesson`: the lesson on, `{"title", "step", "of", "file", "lines", "by"}`, or null).
- `list_mods` (`mods_dir` optional): `{"mods_dir", "mods"}`: every mod in the folder, by its id as the studio reads it from the file's or folder's name (`Night Light.lua` is `night_light`; of two names with one id, the first), with its `kind` (`"script"` or `"module"`), `files` (a module's `.lua` files, main.lua first; `[]` for a script) and `main` (its main file), and its live state when the game is linked.
- `read_mod` (`id`, `mods_dir`): `{"id", "files": {path: text}}` (a script's one file as `<id>.lua`, whatever its name on disk). It gives text: a module's files that are not UTF-8 or are over 1 MiB are left out, and a script that is not UTF-8 is an error that says so (`check_mod` and `test_mod` still run them, as the game does). While the player's editor has unsaved edits to one of the mod's files, `editor_unsaved` says which (`read_script` shows them; `check_mod` without files and live `test_mod` say so too, since they use the files on disk).
- `write_mod` (`id`, `files`, `remove`, `mods_dir`; `files` or `remove` or both): creates or changes a mod. `files` is `{"<id>.lua": "..."}` for a script, or a module's files, `{"main.lua": "...", "lib/util.lua": "..."}`. For a module that exists, only the files given are written and the others stay as they are, and `remove` (paths in the module, `["old.lua"]`; not main.lua) deletes files for good. `main.lua` for an existing script makes it a module: once the module's files are written, the script's file goes to the studio's trash (`trash/<id>.lua`, beside the mods folder) when the mods folder is the studio's (its folder has the game's `studio.txt`), and is deleted for good, as `delete_mod` deletes, when it is any other folder. Every path is checked before anything on disk changes, so one refused (outside the mod, more than three folders deep, where a folder is) leaves the mod as it was. An id that another file of the folder has is refused, for `files` and `remove` alike, naming the file (`night_light` while `Night Light.lua` is there: the studio would keep running that one); where the file system ignores case, as macOS's and Windows' do, `Lamp.lua` is `lamp.lua` itself, and is written. The mod is checked headless first, as it will be on disk (for a module, over its other files as they are there, every byte, its assets too, and without the files `remove` deletes), then written and the files removed; while the game is linked it reloads there. Returns `{"ok", "written", "removed", "check", "live"}` (`check` as `check_mod`; `live` the mod's state in the game, or "not linked: ..."), and for a script made a module `trashed` (where its file is in the trash) or `deleted` (`"<id>.lua"`).
- `delete_mod` (`id`, `mods_dir`): deletes the mod for good: the file or folder `list_mods` lists under that id, whatever it is called (`Night Light.lua` for `night_light`). Returns `{"ok": true, "deleted", "path"}` (`path`: what was deleted), and `now_listed` when another file of the folder had that id too: the studio lists that one now.
- `check_mod` (`id`, `files` optional, `mods_dir`): loads the mod headless with the libraries it imports and runs 2 frames: `{"ok", "mod", "libraries", "log", "chat"}`; `mod` has `state`, `error`, `file` and `line` (where the error is: in a module, the file `require` loaded when that is where it failed), `traceback`, `commands`, `handlers`, `timers`, `library`, `imports`... The mod and its libraries are the ones `list_mods` lists under their ids (`desk_lamp` is the folder `Desk Lamp`), run as the game reads them: every file as it is on disk, whatever its encoding or size. Give `files` to check text not yet written (for a module that exists, they are checked over its other files).
- `test_mod` (`id`, `steps`, `files` optional, `live` default false, `mods_dir`): runs test steps (see `tests`) and returns the report. `files` are tested as `check_mod` checks them (for a module that exists, over its other files); with `live` they are written first.
- `run_lua` (`code`, `live`, `with_mod`, `mods_dir`): runs Lua in the console: in the game while it is linked (default), else headless (with the mod `with_mod` loaded first). Returns `{"ok", "where", "lines": [{"kind", "text"}]}` (headless also `chat`).
- `send_chat` (`line`, `wait_frames` default 20, 1 to 1200): sends a chat line or command in the game as the player, waits, and returns `{"chat", "log"}` that followed. A command the studio takes (a mod's, `/studio`, `/lua`) runs in it, as one the player types; the rest goes to the server. Live only.
- `game_log` (`since_log`, `since_chat`): the console's log and the chat from the game; give the last numbers seen to get only newer lines. Live only.
- `reload_mod` (`id` optional): reloads a mod in the game (and the mods using it), or all; returns the mods' states. Live only.
- `update_server` (no arguments): updates this server to the newest release of `horizon-studio-mcp`, in place (`{"updated", "from", "to"}` or `{"updated": false, "version"}`); it switches to the new binary at the next call, keeping the connection: no restart, no reinstall. The game's Options > Mods > Horizon Studio has the same as a button, and `horizon-studio-mcp update` from a shell. Whenever the binary is replaced (those, or the install line run again), a running server hands over to the new one before its next answer and sends `notifications/tools/list_changed`, so call `tools/list` again then.
- `screenshot` (`studio` optional, default false): a picture of the running game's window, as the player sees it, returned as a PNG image. It goes through Horizon Studio's AI link, so it needs nothing set up (no `--control`). The studio's window is hidden for that frame unless `studio` is true. Live only; with the window minimised no frame is drawn, and it says so after 8 s.
- `horizon_ctl` (`command`): one `horizon-ctl` command to Horizon itself (`state`, `ui`, `click ID`, `screenshot`, `goto X Y Z`, `inventory`, `command /...`, `help`...); `screenshot` returns the PNG. Needs a game started with `--control` (or `horizon-ctl launch --world NAME --wait`).
- `open_script` (`id`, `file`, `scratch`, `line`, `create`; all optional): opens a script in the player's editor, as its script list does, and brings the Editor tab forward. It waits (up to 10 s) while the player is using the studio (a key, a click or the wheel in the last 2 seconds, a drag still held, a pane being sized from the keys, a prompt or a menu up), and refuses while the file open has unsaved edits; the file open already only comes forward then too. `create: true` opens a file that isn't there yet, empty: a new script (`<id>.lua`), a new module's `main.lua` (`file: "main.lua"`), or a new file of a module; it is written when saved. `line` shows that line, unless the player has changed the code in the last 2 s, is clicking on the code, holds a drag or sizes a pane from the keys: the view stays theirs, and `did` says the line wasn't shown. Returns `{"ok", "did", "file", "path", "line_count", "dirty", "created"}`. Live only.
- `read_script` (`id`, `file`, `scratch`, `from`, `to`; all optional): a script's text as the player's editor has it, unsaved edits included (a file it doesn't have open: as on disk), 400 lines at a time. Two text blocks: the state as JSON, `{"did", "file", "path", "open", "dirty", "line_count", "from", "to", "more", "marks", "lesson", "cursor", "selection", "typing"}` (`marks` the file's marks, `[{"mark", "lines", "tone", "note", "by", "step"}]`, `step` a lesson's step's number; `lesson` as in `game_status`; for the file open only: `cursor` is the player's, `{"line", "column"}`, `selection` theirs, `{"from", "to", "text"}` or null, and `typing` an edit being typed in, `{"id", "by", "file"}` or null), then the lines numbered from 1 as `cat -n` numbers them (`     12<TAB>code`). A file the editor has open but couldn't read (one not in UTF-8) is an error that says so: the editor has none of its text. Live only.
- `edit_script` (`id`, `file`, `scratch`, then one of: `old_text` with `new_text`, `start_line` with `new_text` (and `end_line`), or `text`; `typing` default true, `save`): changes a script in the player's editor as they watch, and answers once the change is in. `old_text` is the code to replace exactly as `read_script` shows it, there once (tabs and Windows line ends are taken as the editor has them); `start_line`..`end_line` are whole lines from 1 (`end_line` defaults to `start_line`; `end_line = start_line - 1` puts `new_text` in before `start_line`; `start_line` one past the last line adds it at the end; `new_text: ""` removes the lines; a line break at the end of `new_text` is the last line's); `text` is the whole file. Returns `{"ok", "did", "file", "lines", "line_count", "dirty", "animated", "saved", "problems"}`: `lines` are the new text's lines (from 1) once it is in, `removed` the lines taken out instead, `problems` the editor's live check (`[{"line", "message", "error"}]`); `mod` (`{"id", "state", "error", "file", "line"}`) once saved, and `player_edits: true` when the player's own unsaved edits in the file were saved with it; `why` when it wasn't saved as asked, or by default with the studio closed; `cut_short` when the player's key or click put the rest in at once. An edit that changes nothing answers `{"ok": true, "changed": false, "did", "file", "lines", "line_count", "dirty"}` (`did`: the file "has that already"), and one still typing in after 15 s `{"ok": true, "still_typing": true, "typing", "file", "did"}` (`read_script` gives the text as it will be). A file the editor doesn't have open is written to disk at once instead (`written`, `saved: true`, `mod`): only what the edit replaces changes, and the rest of the file keeps its tabs and line ends; to type into it as the player watches, `open_script` it first. Live only.
- `save_script` (`id`, `file`, `scratch`; the file the editor has open): saves it, as Ctrl+S does, and the game reloads its mod. Returns `{"ok", "did", "file", "saved", "mod", "problems"}` (`did`: "Saved greeter.lua: running", or where it failed), with `player_edits: true` when the player's own unsaved edits in the file were saved too. It waits (up to 8 s) while the player is typing in it, and refuses when the file changed on disk since the editor read it. Live only.
- `mark_lines` (`id`, `file`, `scratch`, `start_line`, `end_line`, `note`, `tone`, `columns`, `reveal` default true, `replace`; `start_line` and `note` required): points at code in the player's editor: lines `start_line`..`end_line` (from 1) highlighted in the colour of `tone` (`note`, the default, `error`, `warning` or `good`), with `note` (500 characters at most) beside them in a bubble; `columns` (`[first, last]`, characters from 1) underlines part of `start_line`. Returns `{"ok", "mark", "did", "file", "lines", "open"}`: `mark` is its number. Live only.
- `clear_marks` (`mark`, `id`, `file`, `scratch`; all optional): clears one mark by its number, a file's marks, or with none of these every mark, which also ends the lesson. A lesson's steps go with it only: one by its number is an error, and a file's leaves them. Returns `{"ok", "cleared", "lesson_ended", "did"}`. Live only.
- `start_lesson` (`title`, `steps`, `id`, `file`, `scratch`; `title` and `steps` required): a lesson in the player's editor. `steps` (1 to 20) are `{"start_line", "end_line", "note", "explain", "task", "tone", "id", "file", "scratch"}` (`start_line` and `note` required): each step's lines are marked with its note, `explain` says more in the Agent pane, and `task` is something to try ("change 20 to 40"). A step's file is the lesson's (`id`, `file`, `scratch`, else the file open) unless it names its own. The first step shows at once (the view goes to its lines unless the player has changed the code in the last 2 s, holds a drag or sizes a pane from the keys); its file opens only while the player isn't using the studio (it waits up to 10 s) and the file open has no unsaved edits. A new lesson replaces the one shown. Returns `{"ok", "did", "steps", "marks", "seq"}` (`marks` the steps' mark numbers). Live only.
- `wait_for_player` (`timeout_s`, default 30, 5 to 50): waits for the player and returns the next thing they did: `{"event", "seq", "age_s", "did", ...}`, `event` being `next` or `back` (`step`, `of`, `file`, `lines`: the step they went to), `check` (`step`, `of`, `task`, `file`, `lines`, `text`: the step's lines as they are now, `problems`, `dirty`), `done` (they finished the lesson), `closed` (`step`: they closed it there), `dismissed` (`marks`: notes they closed), `saved` (`file`, `mod`), `message` (`text`: what they wrote to you in the Agent pane), or `timeout` (`waited_s`: nothing yet; call it again to go on waiting). Live only.

A mod's state, as `list_mods`, `game_status` and `check_mod` give it: `id`, `name`, `version`, `author`, `description`, `kind` (`script` or `module`), `files` (a module's, by their paths in it), `enabled`, `state` (`running`, `failed`, `off`), `error`, `file` and `line` (the file and line of the error: `mymod/lib.lua` and its line when a module failed in lib.lua), `traceback`, `main`, `folder`, `library` (whether it exports), `imports`, `commands`, `handlers`, `timers`, `cost_ms`, `reloads`.

## The player's editor (live)

While the game runs with Horizon Studio, `open_script`, `read_script`, `edit_script`, `save_script`, `mark_lines`, `clear_marks`, `start_lesson` and `wait_for_player` work in the player's own editor, the one in the studio (F8), through the AI link. They are live only: without the game they say so, and a game whose Horizon Studio is older than this server says to update it.

- Which file: `scratch: true` is the editor's scratch file; `id` is a mod, and `file` a file of it by its path in it (`main.lua`, the default, `lib.lua`, `lib/util.lua`; a script's one file is `<id>.lua`); `file` alone is a file of the mod the editor has open; with neither, the file open. A module's file follows the studio's rules for its names (letters, digits, `_` and `-`, at most three folders deep), so nothing outside the mods folder can be named.
- Line numbers count from 1, as `read_script` numbers them. The empty line after a file's last line break isn't one: `line_count` leaves it out.
- Typing: an edit to the file open shows the text it replaces, tinted, for 0.3 s, then the new text types in at about 70 characters a second (2 s at most; spaces and indents come at once) at a caret of your own with your name on it. The view follows it until the player scrolls; their cursor and selection keep their place in the text. It lands at once with Motion off in the studio's Settings, with `typing: false`, while nobody has the editor in view (the studio closed, another tab), and while the player has changed the code in the last 2 s, holds a drag (selecting, the scroll bar, a bar between panes, a pane's tab) or sizes a pane from the keys: their view and cursor stay. Any key or click of theirs, a save, a run or another file opening puts the rest in at once (the result says so in `cut_short`), so their key lands at their own cursor. However long it takes, it is one undo step: Ctrl+Z takes it back, Ctrl+Y puts it back.
- Reading while an edit types in gives the text as it will be. Every request that changes something puts an edit still typing in at once first.
- The file the editor has open, with no unsaved edits, changed on disk (`write_mod`, another program): a change of up to 60 lines is typed in the same way, with no name on the caret, and stays one undo step; a bigger one is read in at once, as with Motion off, and with unsaved edits the editor asks the player (Reload, or Keep mine).
- A file the editor doesn't have open is changed on disk, and its mod reloads: an agent never switches the player's file or tab to edit it. Only `open_script` switches files: open a script first to type into it as the player watches.
- A file the editor has open but couldn't read (one not in UTF-8, say) shows none of its text: `read_script`, `edit_script`, `save_script`, `mark_lines` and `start_lesson` refuse it and say why, and nothing is saved over it.
- Saving: an edit stays unsaved (the player sees the dot by the file's name, and Ctrl+S saves it) until `save_script`, or `save: true`; with the studio closed it is saved unless `save: false`, or unless the file has unsaved edits of the player's (nothing of theirs is saved unasked; `why` says so). `save_script` and `save: true` save the file as it is, the player's unsaved edits too (`player_edits: true` says so). Nothing is saved while the player is typing in the file (`save_script` waits for them), over a change made to the file on disk since the editor read it, or with the rest of an edit the player cut short. The game's own agent (the one the player asks in the studio's Agent pane) has its unsaved edits saved when its turn ends well, unless the file changed since (the player's edits, or on disk); otherwise the Agent pane says they aren't saved. Tools that read the files on disk (`read_mod`, `check_mod`, live `test_mod`) say when the editor has unsaved edits (`editor_unsaved`): save first to test them.
- The Agent pane shows what you did ("Claude Code: Typed lines 12-19 into timer.lua"; an agent outside the game is named), and its Show goes to that code; a change written to a file the editor doesn't show puts a note over the code that shows it too.
- Marks (`mark_lines`): lines highlighted in a colour (a band, and a bar in the gutter) with a note. The newest mark's note, or the one the player clicks, shows in a bubble under the lines, with a × that clears it; the others show their note at the end of their first line, and a click opens it. Esc closes the bubble. The code's tab strip, by the file's name, says how many notes there are: a click goes to the next, its × clears them all. With `reveal` (the default) the view goes to the lines, unless the player is using the studio; their cursor stays. A file the editor doesn't have open keeps its marks until the player opens it, and a note over the code says so (the Agent pane's Show opens it there): marking never switches their file or tab. Marks stay on their lines as the code changes: lines put in or taken out above move them, and inside them they grow or shrink; a file opened again has its marks found where their lines are, and a file that moves (a mod or a module's file renamed, a script made a module) takes its marks and a lesson's steps along. At most 20 marks besides a lesson's steps (the oldest goes), and `replace: true` clears yours first. Marks and lessons last for the session.
- Lessons (`start_lesson`): numbered steps, each a mark with a number in the gutter. The step showing has its note in a bubble with what to try, "Step 2 of 5", and Back, Check and Next (Done on the last step; Check on a step with a task, when an agent waits for the player or the game's own agent can be asked, which is only in the studio's own layout: the executor styles have no Agent pane for its answer); its explanation goes to the Agent pane the first time the step shows, or into the bubble where the Agent pane isn't shown (the executor styles have none). The player steps with the buttons, Alt+Right and Alt+Left (while the keys go to the code), or a click on a step's number; a step in another file opens it. The code's tab strip, by the file's name, shows "Lesson 2/5" (a click shows the step again, its × ends the lesson), and the bubble's × ends it too. Check goes to an agent waiting in `wait_for_player`; with none waiting, the game's own agent is asked to check the step, in a new turn of its conversation. A new lesson replaces the one shown, and `clear_marks` with no arguments ends it.
- Waiting for the player (`wait_for_player`): what they do (a lesson's steps, Check, the end of a lesson, notes they close, saves, what they write) is kept for an agent, at most 16 things for 5 minutes (the steps they go to, 30 s while nobody waits), and `wait_for_player` gives the next one it hasn't had, or after `timeout_s` a `timeout`: call it again to go on waiting (it never holds the server longer than 50 s). While an agent waits, the Agent pane says "Waiting for you" (with its name, for an agent outside the game), and Enter there sends what the player writes to it instead of asking the game's own agent. A new turn of the game's own agent starts afresh.
- A request that changes something carries a key: sent again after a dropped link, it is answered as it was, not done twice.

## Where things are

- The mods folder: the `mods_dir` argument, else `$HORIZON_STUDIO_MODS`, else the one the game last wrote in its link file, else `$HORIZON_HOME` (or `~/.local/share/horizon`) + `/horizon-mods/.horizon-mod-data/horizon_studio/mods`.
- The link file: `~/.horizon-studio/link.json` (`$HORIZON_STUDIO_LINK`), written by the game; the AI link is on unless turned off in Options > Mods > Horizon Studio.
- Horizon's control socket: `$HORIZON_CONTROL`, else `horizon-control.sock` in `$XDG_RUNTIME_DIR` (Windows: `%TEMP%\horizon-control.json`).
- `horizon-studio-mcp install` registers the server with the agents on the machine (`--print` prints the config), `uninstall` removes it.
"##;

const EXAMPLES_TEXT: &str = r##"
The studio comes with these example mods. The MCP tool `examples` lists them and gives each one's source; in the game they are in the Library. `uikit` and `viahorizon` are installed on the first start; `hello` is written on the first run.

| Id | Title | What it shows |
| --- | --- | --- |
| `hello` | Hello | The smallest mod: a `print` at load and `command` returning a string. |
| `coords` | Coordinates | `hud.panel` updated in `on("tick")`, `player.block`, `player.look`, `player.facing`, `string.format`, a toggle kept with `storage`. |
| `clock` | Clock | `world.time()` to hours and minutes, `os.date` (UTC), a panel in the top right. |
| `compass` | Compass | Drawing in `on("hud", fn(w, h))` with `hud.rect`, `hud.text`, `hud.width` and colors with alpha. |
| `frame_graph` | Frame graph | `on("frame", fn(dt))` collecting frame times, `table.insert`/`table.remove`, bars with `hud.rect`, `game.fps`. |
| `zoom` | Zoom | `on("key")` with `key.keyboard.c` and `game.screen()`, `view.zoom`, a `/zoom <2-10>` command, `storage`. |
| `notes` | Notes | Commands with arguments, a list kept with `storage`, `chat.show` for several lines, `select`. |
| `night_watch` | Night watch | `every(1, ...)`, `world.time`, `player.health`, `player.food`, warning once per condition. |
| `ore_finder` | Ore finder | `world.find` (Horizon's agent), directions with `math.atan`, `gsub`, a command with an optional radius. |
| `miner` | Miner | `agent.run` with callbacks chained (`goto`, then `break`), `agent.stop`, `after(0, ...)`, `on("unload")`. |
| `waypoints` | Waypoints | `on("world")` with `draw.beam`, `draw.block` and `draw.text` through walls, a `ui.open` menu of buttons and an input rebuilt on change, `storage`, `sound.play`. |
| `selection` | Selection box | `draw.block` and `draw.box` outlines, a size label with `draw.text`, `sound.play` with pitch. |
| `playground` | Playground | A widget screen (`label`, `toggle`, `slider`, `button`, `input`, `space`), `world.particle` (dust with a color, notes), `sound.at`, the screen's `close`. |
| `outfit` | Outfit | `model.attach` with items and boxes on the head, body and arm, `rotation`, `glow`, `first_person`, `model.set` every tick for an orbiting item, `model.remove`, `model.clear`, `player.camera`. |
| `uikit` | UI Kit | A library (`return kit`) of animated canvas windows on `gfx.*`: see `libraries` for its API. |
| `uikit_demo` | UI Kit Demo | `import("uikit")`: tabs, toggles, slider, input, list, row of buttons, a HUD card, `world.find` every second, an outline in `on("world")`, settings in `storage`. |
| `viahorizon` | ViaHorizon | `via.*` behind a uikit window with a filtered list, `/via <version>` with `pcall`, `/vialog` printing to the console. |
"##;

const RECIPES: &str = r##"
Short complete mods for common asks. Each is one file (`<id>.lua`) unless it says otherwise; write it with `write_mod` and test it with `test_mod`.

## HUD panel with live info

```lua
-- name: Status panel
on("tick", function()
  local x, y, z = player.block()
  if not x then hud.panel(nil) return end
  local health, max = player.health()
  local time = world.time()
  hud.panel("top-left", {
    string.format("XYZ %d %d %d", x, y, z),
    string.format("Health %.0f/%.0f  Food %d", health, max, (player.food())),
    string.format("Day time %d", time),
  })
end)
```

## Chat command with arguments

```lua
-- name: Calculator
-- description: /calc 2 + 3
command("calc", function(rest, words)
  local a, op, b = tonumber(words[1]), words[2], tonumber(words[3])
  if not (a and op and b) then return "Usage: /calc <a> <+|-|*|/> <b>" end
  local ops = {
    ["+"] = function() return a + b end,
    ["-"] = function() return a - b end,
    ["*"] = function() return a * b end,
    ["/"] = function() return a / b end,
  }
  if not ops[op] then return "Unknown operator " .. op end
  return string.format("%s %s %s = %s", words[1], op, words[3], tostring(ops[op]()))
end, "Calculates: /calc <a> <op> <b>")
```

## Chat command with completions

Declared arguments show in the chat's usage line (`/home <action> <name>`), and the chat completes them: the options (with their tooltips) for the first, the saved names for the second.

```lua
-- name: Homes
-- description: /home add|remove|show <name>: places to come back to
local homes = storage.get("homes") or {}

local function names()
  local list = {}
  for name in pairs(homes) do list[#list + 1] = name end
  table.sort(list)
  return list
end

command("home", function(rest, words)
  local action, name = words[1], words[2]
  if action == "add" and name then
    local x, y, z = player.block()
    if not x then return "Not in a world" end
    homes[name] = { x = x, y = y, z = z }
    storage.set("homes", homes)
    return "Saved " .. name
  elseif action == "remove" and homes[name] then
    homes[name] = nil
    storage.set("homes", homes)
    return "Removed " .. name
  elseif action == "show" and homes[name] then
    local h = homes[name]
    return string.format("%s: %d %d %d", name, h.x, h.y, h.z)
  end
  return "Usage: /home add|remove|show <name>"
end, {
  description = "Places to come back to",
  args = {
    { name = "action", options = {
      { text = "add", tooltip = "Saves where you stand" },
      { text = "remove", tooltip = "Forgets a place" },
      { text = "show", tooltip = "Shows where a place is" },
    } },
    { name = "name", suggest = function(rest, words)
      -- After add a new name is typed; remove and show take a saved one.
      if words[1] == "add" then return {} end
      return names()
    end },
  },
})

-- Typed arguments complete as the game's own commands do: players online,
-- and a number the chat shows in red outside 1 to 64.
command("tip", function(rest, words)
  return "Tipped " .. words[1] .. " " .. words[2] .. " emeralds"
end, { args = { { name = "who", kind = "player" }, { name = "amount", kind = "int", min = 1, max = 64 } } })
```

Test the completions headless with `{"suggest": "/home re", "expect_output": "remove (Forgets a place)"}`.

## Toggle with a key

```lua
-- name: Fullbright note
local on_now = false
on("key", function(key, down)
  if key ~= "key.keyboard.g" or not down then return end
  if game.screen() ~= "playing" then return end -- not while typing in chat
  on_now = not on_now
  chat.show(on_now and "G: on" or "G: off")
  sound.play("ui.button.click", 0.5, on_now and 1.4 or 0.8)
end)
```

## Timers: a repeating reminder

```lua
-- name: Reminder
local timer = nil
command("remind", function(rest, words)
  if words[1] == "stop" then
    if timer then cancel(timer) timer = nil end
    return "Reminder stopped"
  end
  local minutes = tonumber(words[1])
  if not minutes or minutes <= 0 then return "Usage: /remind <minutes> [text], /remind stop" end
  local text = table.concat(words, " ", 2)
  if text == "" then text = "Take a break" end
  if timer then cancel(timer) end
  timer = every(minutes * 60, function() chat.show(text) sound.play("block.note_block.bell") end)
  after(1, function() chat.show("Reminder set: every " .. minutes .. " min") end)
end, "/remind <minutes> [text], /remind stop")
```

## 3D waypoint beam and label

```lua
-- name: Beacon
local point = storage.get("point")

command("beacon", function(rest)
  if rest == "clear" then point = nil storage.set("point", nil) return "Beacon cleared" end
  local x, y, z = player.block()
  if not x then return "Not in a world" end
  point = { x = x + 0.5, y = y, z = z + 0.5, dim = player.dimension() }
  storage.set("point", point)
  return "Beacon set"
end, "/beacon sets it where you stand, /beacon clear removes it")

on("world", function()
  if not point or point.dim ~= player.dimension() then return end
  draw.beam(point.x, point.z, "#a09a86ff", 0.3)
  draw.text(point.x, point.y + 2, point.z, "Beacon", "#ffffff", true)
end)
```

## Screen with widgets

```lua
-- name: Options screen
local opts = storage.get("opts") or { trail = false, density = 3, name = "" }

local function open()
  ui.open({
    title = "Options",
    width = 260,
    widgets = {
      { type = "toggle", text = "Particle trail", value = opts.trail,
        change = function(v) opts.trail = v end },
      { type = "slider", text = "Density", min = 1, max = 10, step = 1, value = opts.density,
        change = function(v) opts.density = v end },
      { type = "input", text = "Nickname", value = opts.name, placeholder = "Enter to save",
        submit = function(v) opts.name = v end },
      { type = "space" },
      { type = "button", text = "Save", primary = true,
        click = function() storage.set("opts", opts) ui.close() chat.show("Saved") end },
    },
    close = function() storage.set("opts", opts) end,
  })
end

command("options", open, "Opens the options screen")
```

## Canvas screen with gfx

```lua
-- name: Sketch
local points = {}
local drawing = false

command("sketch", function()
  ui.open({
    canvas = true,
    draw = function(w, h)
      gfx.text(24, 20, "Hold the left button and move. Right click clears. Escape closes.", "#ecedf3", 15)
      for _, p in ipairs(points) do gfx.rect(p[1] - 3, p[2] - 3, 6, 6, "#9a86ff", 3) end
      local mx, my = gfx.mouse()
      if mx then gfx.border(mx - 8, my - 8, 16, 16, "#80ffffff", 8) end
    end,
    click = function(x, y, button)
      if button == "right" then points = {} else drawing = true points[#points + 1] = { x, y } end
    end,
    move = function(x, y)
      if drawing and #points < 2000 then points[#points + 1] = { x, y } end
    end,
    release = function() drawing = false end,
  })
end)
```

## Library and a mod that uses it

Two mods. `mods/mathx.lua`:

```lua
-- name: mathx
local M = {}
function M.round(x, digits)
  local k = 10 ^ (digits or 0)
  return math.floor(x * k + 0.5) / k
end
function M.distance(a, b)
  return math.sqrt((a.x - b.x) ^ 2 + (a.y - b.y) ^ 2 + (a.z - b.z) ^ 2)
end
return M -- a main file that returns a table exports it
```

`mods/spawn_distance.lua`:

```lua
-- name: Spawn distance
local mathx = import("mathx")
command("spawn", function()
  local x, y, z = player.pos()
  if not x then return "Not in a world" end
  return "Spawn is " .. mathx.round(mathx.distance({ x = x, y = y, z = z }, { x = 0, y = 64, z = 0 }), 1) .. " blocks away"
end)
```

## Settings kept in storage

```lua
-- name: Greeting settings
local defaults = { greeting = "Hello", times = 1 }
local settings = storage.get("settings") or {}
for k, v in pairs(defaults) do
  if settings[k] == nil then settings[k] = v end
end

command("greeting", function(rest, words)
  if words[1] == "set" and words[2] then
    settings.greeting = table.concat(words, " ", 2)
    storage.set("settings", settings)
    return "Greeting is now: " .. settings.greeting
  end
  return settings.greeting .. ", " .. (rest ~= "" and rest or "world") .. "!"
end, "/greeting [name], /greeting set <text>")
```

## A hat and a sword on the player

```lua
-- name: Gear
local worn = {}
local gear = {
  hat = { "head", { item = "minecraft:golden_helmet" } },
  sword = { "right_arm", { item = "minecraft:netherite_sword" } },
  halo = { "head", { boxes = { { from = { -4, 10, -4 }, to = { 4, 10.5, 4 }, color = "#fff2a8" } },
                     glow = true, first_person = true } },
}
command("gear", function(rest)
  local g = gear[rest]
  if not g then return "Try /gear hat, sword or halo" end
  if worn[rest] then
    model.remove(worn[rest])
    worn[rest] = nil
    return rest .. " off"
  end
  worn[rest] = model.attach(g[1], g[2])
  return rest .. " on" .. (player.camera() == "first" and " (F5 to see it)" or "")
end)
```

## Particle trail

```lua
-- name: Trail
local on_now = false
command("trail", function() on_now = not on_now return on_now and "Trail on" or "Trail off" end)
on("tick", function()
  if not on_now then return end
  local x, y, z = player.pos()
  if x then world.particle("dust", x, y + 0.1, z, 0, 0.02, 0, 2, "#9a86ff") end
end)
```

## Reacting to chat

```lua
-- name: Mention alert
local me = "Alex" -- the name to listen for
on("chat", function(text)
  local who, said = text:match("^<([^>]+)> (.*)$")
  if who and who ~= me and said:lower():find(me:lower(), 1, true) then
    sound.play("entity.experience_orb.pickup")
    chat.show(who .. " mentioned you")
  end
end)
```

## Mods talking with events

```lua
-- name: Counter
local n = 0
command("bump", function()
  n = n + 1
  emit("counter:changed", n)
end)
on("counter:changed", function(value)
  hud.panel("bottom-right", { "Count: " .. value })
end)
```

## A HUD health bar

```lua
-- name: Health bar
on("hud", function(w, h)
  local health, max = player.health()
  if not health then return end
  local bw = 120
  local x, y = (w - bw) / 2, h - 60
  hud.rect(x - 1, y - 1, bw + 2, 7, "#a0000000")
  hud.rect(x, y, bw * math.clamp(health / max, 0, 1), 5, health < max * 0.3 and "#f07a68" or "#45c27a")
end)
```
"##;

const LIMITS: &str = r##"
Every number below is in the runtime's source.

## Steps and calls

| What | Limit |
| --- | --- |
| Steps for loading a mod (its main file, `LOAD_STEPS`) | 10,000,000 |
| Steps for one handler, timer, command or callback (`EVENT_STEPS`) | 2,000,000 |
| Steps for one console line (`CONSOLE_STEPS`) | 20,000,000 |
| A step | one statement, loop round or call; string and table functions cost 1 per 16 bytes or elements |
| Nested calls | 200 (and about 120 nested Lua calls in a release build, from the stack guard) |
| Nested blocks and expressions in source | 80 |
| Pattern captures / pattern recursion depth | 32 / 200 |
| Longest string | 4 MiB (4,194,304 bytes) |
| Most entries in a table | 1,048,576 |
| `unpack` results | 1,000,000 |
| `table.move` elements | 16,777,216 |
| `string.format` width and precision | 99 |

## Chat and commands

| What | Limit |
| --- | --- |
| `chat.say` and `game.run` together (`SEND_BURST`, `SEND_RATE`) | a burst of 8, then 4 a second |
| A Roblox script's teleports (`/tp`), out of what those sends leave | one at a time, as their budget's refill overflows (4 a second while no mod sends) |
| Command name, argument name | 1 to 32 letters, digits, `_`, `-` |
| Commands one mod (or the console) has (`MAX_COMMANDS`) | 256 |
| Arguments one command declares (`MAX_ARGS`) | 16 |
| Options of one argument / completions in one answer (`MAX_COMPLETIONS`) | 256 / 256 |
| A completion's text / tooltip (`MAX_COMPLETION_BYTES`) | 256 bytes (a longer text is left out, a tooltip is cut) |
| Entries of a `suggest` function's list looked at | 4096 |
| `/studio help` lines for the mods' commands | 20 (typing / in the chat lists them all; the Library shows a mod's first 6) |

## Per mod

| What | Limit |
| --- | --- |
| Handlers (all events) | 256 |
| Timers | 256 (`every` at least 0.01 s apart) |
| Agent actions waiting | 64 |
| Libraries recorded by `import` (for reloads) | 64 |
| Attached models (`MAX_MODELS`) / boxes in one | 64 / 256 |
| Storage keys / key length / value size (`MAX_STORED`) / nesting | 256 / 64 / 256 KiB / 16 |
| HUD panel lines / bytes per line | 16 / 256 |
| Mod id length | 48 |

## Per frame, all mods together

| What | Limit |
| --- | --- |
| World quads (`MAX_WORLD_QUADS`) | 16384 |
| A texture's side (`hstudio_meshes::MAX_SIDE`) | 512 pixels |
| Every mod's textures together (`MAX_TEXELS`) | 1048576 pixels (a 1024x1024) |
| A mod's textures / meshes (`MAX_TEXTURES`, `MAX_MESHES`) | 256 / 256 |
| A mesh's triangles (`MAX_TRIANGLES`) | 65536 |
| A model file read by `texture.load` / `mesh.load` | 16 MB |
| Through-wall lines and labels (`MAX_OVERLAY`) | 4096 |
| Particles (`MAX_PARTICLES`) | 512 (1 to 64 per `world.particle`) |
| Sounds (`MAX_SOUNDS`) | 32 |
| Ticks caught up in one frame | 10 |
| Keys / chat lines / commands queued for the next frame | 256 / 64 / 64 |
| Events waiting (`emit`), values per event, name length | 1024, 16, 64 |

## Sizes and ranges

| What | Range |
| --- | --- |
| `draw.*` width | 0.002 to 4 blocks (through walls: 1 to 8 GUI pixels) |
| `draw.text` text | 128 characters |
| `gfx.text` text | 1024 bytes; size 6 to 96 dp |
| Screen widgets / panel width / `space` height | 64 / 200 to screen width - 32 / 0 to 300 |
| `world.find` radius / results | 1 to 64 (default 32) / 16 nearest |
| `world.entities` radius | 1 to 128 (default 16) |
| `sound` volume / pitch | 0 to 4 / 0.5 to 2 |
| `view.zoom` | 1 (off) to 10 |
| `color` components | 0 to 255 |
| Model offset, rotation, scale numbers | at most 4096 either way |
| Item id / part name | 96 / 40 characters |
| JSON nesting | 64 |

## The studio

| What | Limit |
| --- | --- |
| Console log kept (`MAX_LOG`) | 1000 lines |
| Console history kept | 200 lines |
| Mods folder scan | every 0.5 s |
| A dropped mod | 256 files, 8 MiB |
| Headless test | 12000 frames (10 minutes of game time) |
| `send_chat` wait / link `wait` | 1 to 1200 frames |
| A chat line sent through the link | 1 to 256 characters |
"##;

const ROBLOX: &str = r##"
Horizon Studio runs Roblox client scripts: Luau (see the `language` topic) and the Roblox API a LocalScript uses, put onto what the studio already does. A Roblox UI script, or a general Luau script, runs as a script mod as it is.

```lua
local Players = game:GetService("Players")
local TweenService = game:GetService("TweenService")

local gui = Instance.new("ScreenGui")
gui.Parent = Players.LocalPlayer:WaitForChild("PlayerGui")

local button = Instance.new("TextButton")
button.Size = UDim2.fromOffset(160, 40)
button.Position = UDim2.new(0.5, -80, 0.8, 0)
button.Text = "Click me"
button.Modal = true -- frees the mouse while the button shows
button.Parent = gui
Instance.new("UICorner", button)

button.MouseButton1Click:Connect(function()
  TweenService:Create(button, TweenInfo.new(0.3), { BackgroundColor3 = Color3.fromRGB(0, 170, 255) }):Play()
end)
```

**How it maps.**

- `game` is the DataModel, shared by every mod as one game is by every script in Roblox. It keeps the studio's own `game.run`, `game.fps`, `game.screen`, `game.time` and `game.ticks`. `game.Players`, `game:GetService("X")`, `workspace` (also `Workspace`, and Roblox's older `game.workspace`) and `shared` (one table for every mod) work as in Roblox. `game.LocalPlayer` is Horizon's own shortcut for `game.Players.LocalPlayer`: Roblox has no such member, so write `game.Players.LocalPlayer` in code meant for Roblox (its older `Players.localPlayer` works too). `script` is a LocalScript named after the mod, in `Players.LocalPlayer.PlayerScripts`.
- What a mod makes (Instances, connections, tweens, waiting threads, render steps, actions) goes when it stops or reloads.
- **Signals** (`inst.Changed`, `button.MouseButton1Click`, `RunService.Heartbeat`...) have `Connect`, `Once`, `Wait` and `ConnectParallel` (as Connect); connections have `Disconnect` and `Connected`. They are deferred, Roblox's default: a handler runs in a thread of its own once the code that fired it is done (in the same frame), so it can `task.wait`. `GetPropertyChangedSignal(name)` and `GetAttributeChangedSignal(name)` give a property's own signal.
- **ScreenGuis** under `Players.LocalPlayer.PlayerGui` (or `game.CoreGui`) are laid out as Roblox lays them out (Size, Position, AnchorPoint, SizeConstraint, AutomaticSize, UIListLayout, UIGridLayout, UIPageLayout and UITableLayout (see Layouts), UIPadding, UIScale, UIAspectRatioConstraint, UISizeConstraint, UITextSizeConstraint, ScrollingFrame canvases, ClipsDescendants, ZIndex with Global or Sibling, DisplayOrder) and drawn on the HUD with the design system's shapes and Geist fonts: backgrounds and transparency, UICorner, UIStroke (on the edge, or round text), UIGradient, borders, text with its alignment, wrapping, scaling and stroke, `TextTruncate` (AtEnd cuts a line that doesn't fit where it and "..." still do, SplitWord after its last whole word, and wrapped text cut off below ends its last line so), `MaxVisibleGraphemes` (only the first characters drawn, each where the whole text puts it, for a typewriter effect), TextBox placeholders, the caret and the selection, ScrollingFrame bars, AutoButtonColor. A GUI pixel is a dp (Horizon's logical pixel); there is no top bar, so `GuiService:GetGuiInset()` is zero. An image from `getcustomasset` (a PNG in the workspace folder) is drawn as it is, placed as its `ScaleType` says (Stretch, Fit, Crop, Tile at `TileSize`, or Slice by `SliceCenter` and `SliceScale`), the part of it `ImageRectOffset` and `ImageRectSize` name, tinted by `ImageColor3` and faded by `ImageTransparency`. The game keeps images beside the interface's text, so one larger than 256 pixels a side is drawn from a copy scaled down to that, and once a frame's images take 256K of those pixels, the others are drawn in blocks of colour. A file is read and decoded once while it is drawn: the 256 images drawn last are kept, 8M pixels in all (32 of 512 a side), and a GUI that draws more keeps the first it draws decoded, frame after frame, and decodes only the rest again. An image keeps its place beside the text however many are drawn, one drawn from a scaled copy too. Roblox's images load too: an asset id (`rbxassetid://`, `http://www.roblox.com/asset/?id=`, a bare number) or a `rbxthumb://` comes from Roblox's thumbnails (its own asset downloads need a signed-in account), and a `rbxasset://` path (`textures/...`, `studio_svg_textures/...`: Roblox's content folder) from the Client Tracker's mirror of that folder, each fetched once in the background and kept in the workspace's `.roblox-assets` folder; until it has come an image shows only a faint tint. A thumbnail is the image scaled to fit 700 pixels, a smaller one scaled up, while scripts crop (`ImageRectOffset`, `ImageRectSize`) and slice (`SliceCenter`) in the original's pixels, so the original's size is worked out: for a cropped image, from the trace resampling leaves in its 700- and 1024-pixel thumbnails (kept beside it as a `.size` file), times the smallest whole number that makes every crop drawn fit (a sheet of doubled pixels reads as half its size until a crop reaches past that); for a sliced one, from its SliceCenter, its insets taken as alike on both sides; for one that shows no trace, the thumbnail's own size, made larger only as far as its crops reach. A small part of a large image (an icon of a sprite sheet or of a strip such as `ClassImages.PNG`) goes in the game's atlas on its own, at the image's resolution.
- **Drawing.** `ClipsDescendants` cuts what an object holds at its edges exactly: shapes, text and images (as in Roblox, an object that is turned, or in a turned one, doesn't clip). `Rotation` turns an object and what it holds about its centre, and the pointer finds it where it is drawn; its AbsolutePosition and AbsoluteSize stay upright, as in Roblox. A UIGradient colours its object's background, border, image and text (multiplying their colours) with every keypoint of its `Color` and `Transparency`, from left to right turned clockwise by its `Rotation` across the object's box (so 90 runs top to bottom), and moved by its `Offset` (a fraction of the object's size). It leaves the object's UIStroke alone: a UIGradient in the UIStroke colours that. A CanvasGroup's UIGradient colours all the group holds, and its `GroupColor3` tints and its `GroupTransparency` fades it all. Where the game can't change what it has drawn (a headless test), the GUI is drawn upright, cut to whole shapes and text lines, and a gradient gives its middle colour.
- **BillboardGuis** in the Workspace (in a part, a model or an attachment, which they float over) or in the PlayerGui with an `Adornee` are drawn under the ScreenGuis, the farthest first, at the adornee's place on the screen: moved by `StudsOffset` along the camera's right, up and back, `StudsOffsetWorldSpace` along the adornee's own axes, and `ExtentsOffset` and `ExtentsOffsetWorldSpace` the same ways in halves of the adornee's size (a model's box is round all its parts). A `Size` in scale is in studs, so it shrinks as the camera moves away (between `DistanceLowerLimit` and `DistanceUpperLimit`, in steps of `DistanceStep`), and in offset in pixels; `SizeOffset` moves it by fractions of its size, up for y. It hides behind the camera, beyond `MaxDistance`, behind a block in the way unless `AlwaysOnTop`, and from `PlayerToHideFrom`. Only an `Active` one takes the pointer, and its `ClipsDescendants` cuts what it holds to its box. Inside one, `AbsolutePosition` is from its top left corner. `LightInfluence` and `Brightness` are kept: it is drawn as bright as the HUD. Name tags and health bars over the Workspace's parts work this way.
- **Layouts.** UIListLayout's `HorizontalFlex` and `VerticalFlex` share the room left along an axis: `Fill` grows (or shrinks) the items along their line, and the lines across when it `Wraps`; `SpaceBetween`, `SpaceAround` and `SpaceEvenly` make gaps of it. Its `ItemLineAlignment` places the items across their line (`Stretch` makes them as thick as it). A UIFlexItem in an item says how it grows and shrinks (`FlexMode` Grow, Shrink, Fill, or Custom by `GrowRatio` and `ShrinkRatio`; it shrinks by its ratio times its size, as CSS's flexbox does) and has an `ItemLineAlignment` of its own. A UIPageLayout lays its container's pages one after another along its `FillDirection` (with `Padding` between), its `CurrentPage` lined up as its alignments say (give the container `ClipsDescendants` to show one page at a time). `Next()`, `Previous()`, `JumpTo(page)` and `JumpToIndex(i)` (counting from 0) turn it to a page over `TweenTime` with its `EasingStyle` and `EasingDirection` when `Animated` (else at once), round from one end to the other with `Circular`; PageLeave and PageEnter fire as it starts, and Stopped once it is there. The wheel over it turns pages (`ScrollWheelInputEnabled`). A UITableLayout makes its container's objects rows (`MajorAxis` RowMajor) or columns (ColumnMajor), and their objects the cells: each column is as wide as its widest cell and each row as tall as its tallest, with `Padding` (a UDim2: across, then down) between; `FillEmptySpaceColumns` and `FillEmptySpaceRows` share the room left among them, every cell takes its cell's size, and the table aligns in the container. Its `FillDirection` is kept.
- **RichText.** With `RichText = true` a text object's markup is drawn: `<b>`, `<i>`, `<u>`, `<s>`, `<br />`, `<font color="#FF7800" size="20" face="GothamBold" family="rbxasset://fonts/families/Ubuntu.json" weight="bold" transparency="0.5">` (a colour is `#RRGGBB`, `#RGB` or `rgb(r, g, b)`), `<stroke color thickness transparency>`, `<mark color transparency>`, `<uppercase>` (`<uc>`), `<smallcaps>` (`<sc>`), `<!-- comments -->`, and `&lt;` `&gt;` `&quot;` `&apos;` `&amp;` for those characters. Each run is measured and wrapped in its own size and font, a line is as tall as its largest run, and TextTruncate's "..." takes the style of what it follows. A tag it doesn't know is dropped, and a `<` that starts no tag (`1 < 2`) is text. Every font is drawn in Geist (Regular, Medium for medium weights and bolder, so bold shows) or Geist Mono for monospace families; italic letters (`<i>`, or a FontFace whose Style is Italic) are drawn leaning, as there is no italic Geist.
- **GUI state.** What the layout reports (`AbsolutePosition`, `AbsoluteSize`, `AbsoluteRotation`, `TextBounds`, `TextFits`, a layout's `AbsoluteContentSize`, a ScrollingFrame's `AbsoluteCanvasSize` and `AbsoluteWindowSize`) is right as soon as it is read, as in Roblox: a read after a change lays out again what that value needs (the way down from its ScreenGui, and the sizes of what a layout on the way arranges, whose places are then fresh too), and a read with no change since costs nothing. A colour, a transparency, an image or a TextBox's caret changing is no change to them. Only ScreenGuis and BillboardGuis that draw are laid out, so an object outside them keeps the values it had. `ContentText` is a text object's text without rich text's tags, as drawn (`<br />` a new line, `<uppercase>` in capitals). `GuiState` is `Enum.GuiState.Idle`, `Hover` (the pointer is over it), `Press` (a button held down) or `NonInteractable` (`Interactable` is false), and `GetPropertyChangedSignal("GuiState")` fires as it changes. Both are read-only. A GuiObject's deprecated `Transparency` is its `BackgroundTransparency`, read, set or tweened; a Part's stays its own. An ImageButton shows its `HoverImage` while the pointer is over it and its `PressedImage` while it is held, when they are set. An image's `IsLoaded` is true when it draws (a `getcustomasset` PNG). `ScrollingFrame.ElasticBehavior` takes `Enum.ElasticBehavior`, though nothing scrolls past the canvas's ends.
- **TextBoxes.** A focused TextBox edits as a text field does: typing goes in at the caret, over the selection; Left and Right (a word with Control), Home and End move the caret, and with Shift they select; Backspace and Delete take a character (a word with Control) or the selection; Control+A selects everything, and Control+C, X and V copy, cut and paste. A press in the box puts the caret there and a drag selects. In `MultiLine` text Enter starts a new line and Up and Down move a line as the text is drawn; otherwise Enter ends the editing with `FocusLost(true)`. Escape lets go of the box first (`FocusLost(false)`), and only a second Escape closes a GUI that still wants the pointer. A box destroyed while it has the focus (or what holds it: its ScreenGui, a mob's mirror as the mob goes) loses it as it goes, with `FocusLost(false)`, and `CaptureFocus` on a box that went does nothing, as in Roblox, so a FocusLost handler that takes the focus back lets it go then. `CursorPosition` and `SelectionStart` follow the caret and the selection's other end as 1-based byte offsets (CursorPosition is 1 before the box was ever edited and -1 once its editing ends, and SelectionStart is -1 with nothing selected), and setting them moves the caret and selects. While a box has the focus, keys still fire UserInputService's InputBegan and InputEnded, with `gameProcessedEvent` true. A box being edited shows RichText's tags, since they are what is edited; one that isn't `TextEditable` still moves its caret, selects and copies.
- **ScrollingFrames.** The wheel scrolls the topmost ScrollingFrame under the pointer the way its `ScrollingDirection` says: X across, Y down, and XY down while it can, else across. Its bars show while the canvas is longer than the window that way: the vertical one on the side `VerticalScrollBarPosition` names, the horizontal one at the bottom, both over what the frame holds and leaving each other the corner. Dragging a bar's thumb scrolls the canvas, and a press on the track beside the thumb brings the thumb there. A ScrollingFrame clips what it holds (`ClipsDescendants` is true for it, as in Roblox).
- **The mouse.** In Minecraft the pointer belongs to the game. The GUI takes it while a visible GuiButton has `Modal = true` or a TextBox has focus (`CaptureFocus`), or after **Left Alt** over a GUI with buttons; Escape gives it back. Then MouseEnter/MouseLeave/MouseMoved, InputBegan/InputEnded, MouseButton1Down/Up/Click, MouseButton2*, Activated, MouseWheel*, TextBox focus and typing (FocusLost(enterPressed)), ScrollingFrame wheel scrolling and `Draggable` work.
- **Input.** Keys and mouse buttons fire UserInputService's InputBegan/InputEnded (`input.KeyCode`, `input.UserInputType`, `input.UserInputState`, `input.Position`, and `gameProcessedEvent`, true while chat, a menu or the studio is open), `UIS:IsKeyDown`, `IsMouseButtonPressed`, `GetMouseLocation`, ContextActionService's BindAction (keys and mouse buttons) and the Mouse's (`Player:GetMouse()`) Button1Down, Button1Up, Move, X, Y, Hit (where the crosshair points: on a part, an entity's too, or a block), Target (the part there, or `workspace.Terrain` for a block; its `TargetFilter`, and what is in it, is left out), TargetSurface (the face, a NormalId in the part's own space), UnitRay (from the camera through the pointer, a block long) and Origin (the camera, looking along it). As in Roblox, they are worked out once a frame (and again as the pointer moves or TargetFilter changes): a part a script moves shows in them the next frame.
- **The player.** `Players.LocalPlayer` is the player playing: its Name and DisplayName are their profile's name, its UserId comes from their UUID (its low 49 bits; the whole UUID is its `UUID` attribute), and until the game says who they are (a headless test) it is Player1 with UserId 1. Its Character is an R6 rig that follows them each frame, in blocks (a stud is a block): the HumanoidRootPart is the torso's box (0.47 x 0.70 x 0.23, its centre 1.05 blocks above the feet standing, lower crouching, swimming, gliding or asleep), the Head the head's cube at the eyes, facing where they look, the Torso, Left Arm, Right Arm, Left Leg and Right Leg are placed with the root part as the player stands (the game's animation of them isn't followed), and the Humanoid is theirs. Health and MaxHealth are the player's times 5 (HealthChanged and Died fire); WalkSpeed and JumpPower are the game's, in blocks a second (4.3178 walking, 5.6131 sprinting; 8.4); JumpHeight is how high a jump goes (1.2522), HipHeight the legs, RigType R6; MoveDirection is the way the movement keys walk them while one is held (else the way they go) and the root part's AssemblyLinearVelocity how they move; FloorMaterial is the material of the block under them; `workspace.Gravity` is Minecraft's 32. `Humanoid:GetState()` and StateChanged follow the game: Running, Jumping, Freefall, Landed, Swimming, Flying (creative flight, an elytra), Seated (in a bed) and Dead, with Jumping(active) and Running(speed) too. A script's own WalkSpeed or JumpPower stays until the game's value changes; setting one scales the game's own walking or jumping (below). Coming back to life, they get a new Character, as in Roblox: CharacterRemoving fires with the old one, then CharacterAdded and CharacterAppearanceLoaded with the new one.
- **Moving the player.** Setting the HumanoidRootPart's CFrame or Position, `character:PivotTo(cf)`, `SetPrimaryPartCFrame`, `Model:MoveTo(pos)` and a tween of the root part's CFrame teleport the player with the game's `/tp` command (on a server, the player needs the right to run it): their feet go to the root part's centre less its height above them, so moving the character where it is leaves them there. `Model:MoveTo(pos)` moves the character up out of the blocks in its way, as Roblox moves a model up out of what is in its way: a point on a block's top (the Mouse's Hit on the ground) puts the feet on the block; `PivotTo` and `SetPrimaryPartCFrame` put it exactly where they are asked. One teleport goes at a time, the last move's, and only out of what the chat rate limit's sends leave (the refill of a burst that is whole): a script that moves the player every frame (a fly script) moves them 4 times a second while no mod sends, less while they do, and the mods' `chat.say` and `game.run` keep their whole burst. One that must wait isn't lost. `Humanoid:MoveTo(pos)` walks them there with the agent's `goto`; MoveToFinished fires with true when they get there, and with false when they can't, after 8 seconds (as in Roblox) or when another MoveTo takes its place. Another Humanoid can't walk: its MoveToFinished fires with false.
- **The character's doings.** What a script tells the local Humanoid, or takes off the character, reaches the game, client-side: the player's moves go to the server as their own, and their own model's changes only this client sees. `Humanoid:ChangeState(Enum.HumanoidStateType.Jumping)` makes the player jump the game's own jump (its strength, Jump Boost, a sprint's push), in the air too, as infinite jump scripts have it; `Humanoid.Jump = true` jumps once from the ground (seated, it gets up and jumps) and reads false again; UserInputService.JumpRequest fires each frame the jump key is held. `Humanoid.Sit = true` (or `ChangeState(Seated)`) sits the model as a rider sits (state Seated) and holds the player still until the jump key or `Sit = false`; a root part turned on its side while seated lies the model down. `PlatformStand = true` holds them still (PlatformStanding), `ChangeState(FallingDown)` lays the model down and holds them for 1.5 seconds, then GettingUp; Ragdoll and Physics hold them until another state. A state the Humanoid may not enter (`SetStateEnabled(state, false)`) it doesn't enter on its own, though ChangeState puts it there (but Jumping); with Jumping off the player can't jump at all. `WalkSpeed`, `JumpPower` and (with UseJumpPower off) `JumpHeight` each scale the game's own (16, 50 and 7.2 are its; a jump's height goes with the square of its speed). Setting the root part's `Velocity` or `AssemblyLinearVelocity` (studs, here blocks, a second) sets the player's, unless a body mover moves them; setting it to what it reads changes nothing. An anchored, collidable part a script keeps in the character under the root part (a float script's) holds the player up: they don't fall below where it holds them (its top 3 studs under the root part, an R6 character's stand), follow it as it moves down or up, and can jump off it. `Model:TranslateBy` moves the character by an offset, and setting the Head's CFrame moves the character with it, as Roblox moves a part's assembly (the player's eyes where it is: a cframe fly). The character's parts are the player's model: a Head, Torso, Left Arm, Right Arm, Left Leg or Right Leg it no longer has (destroyed, moved out) is no longer drawn, and with no arm the first-person hand isn't either; the character wears a Shirt and Pants (the skin's jacket and sleeves, and trousers) and an Accessory named Hat whose Handle is welded to the Head (the hat layer; `Humanoid:GetAccessories`, `RemoveAccessories`, `Player:ClearCharacterAppearance` and destroying its weld take it off). A part a script makes visible (Transparency under 1) is drawn as the Roblox part it is, in place of the model's. A character out of the Workspace, or with none of its parts, isn't drawn at all, armour, held items and cape too. A BodyAngularVelocity (or enabled AngularVelocity) in the root part spins the model at its AngularVelocity.Y (the view stays); the character's Animate script disabled keeps the model's limbs still; `AutoRotate = false` keeps the body from turning to the walk. `ChangeState(Dead)`, `character:BreakJoints()` and moving the root part below `workspace.FallenPartsDestroyHeight` (the last number it had, while a script has it at NaN) kill the character as the server would (Died, the Dead state, a new character `Players.RespawnTime` later); the player in the game lives on and isn't moved, nor out of the world (above y 4096, past the world border). Roblox's animations by id, HipHeight, MaxSlopeAngle and physical properties (density) have no match in the game.
- **The world.** While a script looks at the world (below), the game's entities near the player are in the Workspace, the studio's own and locked as the character is (scripts can put things in them, not take them out; `Destroy()` on one takes its entity out of the client's view instead; Debris leaves them, with a warning). A mob is a Model named after it (its custom name, else its type: `minecraft:cave_spider` is "Cave Spider") with a HumanoidRootPart from its feet to under its head, a Head (a cube at its eyes, so a ray at the eyes hits the Head) and a Humanoid (RigType R6, HipHeight 0) with its health times 5, a WalkSpeed that is its movement_speed attribute on the player's scale (times 43.178, so a zombie's is 9.93: not how fast it walks) and its state (Running, Freefall or Dead). Another player's entity is an R6 character as the LocalPlayer's is, named after them. A dropped item is a Part named after the item ("Diamond Sword") with `ItemId` and `Count` attributes, and an experience orb a Part "Experience Orb" with a `Value` attribute. Each has `EntityId` and `EntityType` attributes (`minecraft:zombie`), and a mob a `Baby` one. They are invisible, since the game draws them (a part a script makes less than fully transparent shows its box). Each frame they follow the game: their parts move quietly, with no Changed, as Roblox's physics moves parts, and the root part's AssemblyLinearVelocity is the entity's; HealthChanged, Died (once), StateChanged and a new name or count fire as the game's own values change. ChildAdded fires for one that comes, whole, and Destroying then ChildRemoved for one that goes. What a script sets on them changes nothing in the game: a part it moves is back where the game has it the next frame, and a Humanoid's values stay until the game's change. At most 256 are mirrored, the other players first and then the nearest; one mirrored stays until it is 288th or farther, so those about the cut-off don't come and go as they move. Another dimension, or leaving the world, clears them.
- **Looking at the world.** Mirroring costs the game, so it starts once a script reaches for the entities: the Workspace's `GetChildren`, `GetDescendants`, `FindFirstChild` and the other finds, `WaitForChild`, a child by its name, its ChildAdded, ChildRemoved and descendant events, `Raycast` and the queries below; game's `GetDescendants`, descendant events and recursive finds; anything of Players but the LocalPlayer; the Mouse's Hit, Target and TargetSurface; and `getinstances`. Reading `workspace.CurrentCamera`, `workspace.Terrain`, a child of the Workspace's own by its name (`workspace.Camera`, the character, a script's part), `Players.LocalPlayer` or `getnilinstances()` doesn't start it, so a UI script costs the game nothing. The first look mirrors what is there at once, with no ChildAdded for it (as for what a Roblox place held before a script ran). It lasts while a script that looked runs (a mod that reloads and looks again keeps the same mirrors); a console line's lasts a minute after its last look, and as long as a handler of the console's listens.
- **The Backpack is the inventory.** `Players.LocalPlayer.Backpack` holds a Tool for each stack of the player's inventory (the hotbar's 9 slots and the 27 above), named after its item (`Diamond Pickaxe`), with `ItemId`, `Count` and `Slot` (1 to 36, 1 to 9 the hotbar) attributes, the item's id as its ToolTip, Minecraft's texture of it as its TextureId and an invisible `Handle` in the right hand; the selected hotbar slot's Tool is in the character, equipped. Each frame they follow the game: a stack that comes, goes or changes, and the player choosing another slot (Unequipped, then Equipped with the Mouse). Scripts move them the other way: a Tool put in the character (`tool.Parent = character`, `Humanoid:EquipTool(tool)`) selects its slot; `Humanoid:UnequipTools()` selects the empty hotbar slot nearest, or holds nothing as far as the layer goes when there is none (the number keys give the slot back); a Tool put in the Workspace drops its stack, as Ctrl+Q (one at a time, a game tick after its slot is selected, so a server knows it); `tool:Activate()` uses the item, as a right click does. Activated fires as the player uses the item held (the right button, Minecraft's use) and Deactivated as they let go. A stack above the hotbar can't be equipped or dropped from here, and a stack's Tool a script destroys leaves the Backpack until the stack changes (the client can't take an item away).
- **Scripts' tools.** The Tools and HopperBins scripts make (BTools' bins, F3X, a teleport tool) are slots of the studio's own after the hotbar's, drawn beside it with their names' first letters (a HopperBin's BinType). `0` equips the first, and the next on each press; the wheel goes on past the hotbar's ends into them and through them; 1 to 9 go back to the game's slots. While one is held the mouse's buttons are its, not the game's: its Activated and Deactivated (not with `ManualActivationOnly`), the Mouse's Button1Down and Button1Up, with the Mouse at the crosshair (or the pointer, while the GUI has it). A HopperBin fires Selected(mouse) and Deselected, its `Active` following. The HopperBins of BinType Hammer, Clone and Grab do as Roblox's did: the Hammer destroys the part clicked, the Clone copies it on top of itself, the Grab (and the GameTool, Roblox's old tool for moving bricks) drags it along the Mouse while the button is held (not a Locked part, the character or a mirror). On Minecraft's blocks they change the client's copy of a server's world only, as a Roblox client's own changes are, until the server updates the block: the Hammer breaks it, the Clone puts a copy of it on the face clicked, the Grab lifts it and puts it down on the face the Mouse points at as the button comes up. An entity's mirror the Hammer hits leaves the client's view of the world until the server sends it again.
- **Building handles.** `Handles` and `ArcHandles` with an `Adornee` (in the PlayerGui, the CoreGui or gethui) are drawn over everything: a knob a stud off each face of the part in the Handles' `Color3` (or `Color`), and a ring about each of the part's axes with two knobs on it, red, green and blue. A left-button press on a knob (at the crosshair, or the free pointer) fires `MouseButton1Down(face)` (a NormalId) or `(axis)`; while it is held, `MouseDrag` fires as the pointer's ray changes: a Handles' with how far along the face's normal it has gone since the press (the ray's nearest point to the line through the part's centre), an ArcHandles' with the angle turned about the axis (radians, in the plane through the centre) and how much farther from the centre it is; `MouseButton1Up` as it comes up. The line and the plane stay where they were at the press, and the Mouse doesn't Move meanwhile. A `Dragger` (`Instance.new("Dragger")`) drags the parts given to `MouseDown(part, pointOnPart, parts)`: `MouseMove(ray)` puts the point held where the ray meets what isn't them, the parts resting on that surface, a whole stud along it from where they were; `AxisRotate(axis)` turns them a quarter about it; `MouseUp()` lets go. Building Tools by F3X runs on them: select by clicking a part (Shift adds), move, resize and rotate with its handles or by dragging, paint, add (NewPart) and delete parts; its dock and panels take the pointer with Left Alt.
- **Interactive parts.** A click in the world (either button) on a part whose `ClickDetector` is in it or in a model it is in, within the detector's `MaxActivationDistance` of the character's head, fires `MouseClick` (or `RightMouseClick`) with the LocalPlayer; one in the Terrain fires for a click on a block, one a script put in an entity's mirror for a click on the entity. The nearest enabled `ProximityPrompt` within its `MaxActivationDistance` of the character is shown over the hotbar (`[E] Open Door`), with `PromptShown` and `PromptHidden` (its own and ProximityPromptService's) as it comes and goes; its `KeyboardKeyCode` is then the prompt's, not the game's: `Triggered` (and ProximityPromptService's `PromptTriggered`), or `PromptButtonHoldBegan` and `Triggered` a `HoldDuration` later, and `TriggerEnded` as it comes up. A part a script connects `Touched` or `TouchEnded` to has a `TouchInterest` (a TouchTransmitter), and the player's box coming into it fires `Touched` with the root part (`TouchEnded` as it leaves). `BodyPosition`, `BodyVelocity` and `BodyGyro` in a script's unanchored part move it, as the client's physics would. `Model:GetBoundingBox()`, `GetModelCFrame()`, `GetModelSize()` and `GetExtentsSize()` give the box its parts fill. `Destroy()` on an entity's mirror takes the entity out of the client's view of a server's world (it comes back when the server sends it again). Another player's Player has a `Backpack`, empty: the game doesn't say what they carry.
- **Other players.** While a script looks, Players has a Player for each other player on the server (a LAN host's guests too), in the order they joined, with their Name, DisplayName, a UserId from their UUID and the `UUID` attribute, as the LocalPlayer has them. PlayerAdded and PlayerRemoving fire as they join and leave; those there already when a script first looks fire nothing, as in Roblox. A player's Character is the mirror of their entity while the server sends it (they are near), and nil while they are far: CharacterAdded and CharacterAppearanceLoaded fire as it comes (a frame after PlayerAdded for one who joins near, so its handlers see it), and CharacterRemoving as it goes. A player entity the list doesn't name (a server's NPC) is a character of no one's. `Players:GetPlayers()` (the LocalPlayer first), `GetPlayerFromCharacter`, `GetPlayerByUserId`, and `GetNameFromUserIdAsync` and `GetUserIdFromNameAsync` among the players on the server (anyone else is an error: Roblox's website isn't asked). `Player:GetNetworkPing()` is their ping in seconds (the LocalPlayer's needs no look: the list is read for it, at most every half second) and `DistanceFromCharacter` is from their Head; another player's `GetMouse()` is nil, and their `Kick` and `LoadCharacter` give Roblox's errors for a LocalScript. `Players.MaxPlayers` is at least how many are there.
- **The Camera.** `workspace.CurrentCamera` follows the game's: CFrame, FieldOfView, ViewportSize (dp); `WorldToViewportPoint`, `WorldToScreenPoint`, `ViewportPointToRay`, `ScreenPointToRay`.
- **Parts.** BaseParts in the Workspace (Part with Block, Ball and Cylinder shapes, WedgePart, MeshPart and the rest as boxes) are drawn in the world each frame with their CFrame, Size, Color and Transparency, lit by the world (Neon is not). They don't collide or fall: Anchored or not, they stay where they are put.
- **`workspace:Raycast(origin, direction[, params])`** goes through the world's blocks (`result.Instance` is `workspace.Terrain`, `result.Material` the closest Roblox material, `result.Block` the block's id) and the Workspace's parts, the entities' among them, with RaycastParams' filter: an Include list that doesn't name the Terrain goes through the blocks. The local character is left out, since a ray from the eyes starts in its head, unless an Include filter names it. A mob's Head and root part, and another player's limbs, are hit on their own. Roblox's older `FindPartOnRay(ray[, ignore])`, `FindPartOnRayWithIgnoreList(ray, list)` and `FindPartOnRayWithWhitelist(ray, list)` give the part (or the Terrain), the point, the normal and the material, and nil and the ray's end when it meets nothing.
- **Bounds queries.** `workspace:GetPartBoundsInRadius(position, radius[, params])`, `GetPartBoundsInBox(cframe, size[, params])` and `GetPartsInPart(part[, params])` give the Workspace's parts whose boxes, as the world sees them, are within the radius or overlap the box, with OverlapParams' filter and `MaxParts`. Unlike rays, they find the local character too, as Roblox's do.
- **Terrain.** `workspace.Terrain` is the map, Minecraft's blocks, and a cell is a block. `Terrain:GetBlock(position)`, Horizon's own, gives the id of the block there (nil where nothing is loaded). `Terrain:ReadVoxels(region, resolution)` gives the materials and occupancies of a `Region3`'s cells `resolution` blocks a side (Roblox's are 4; 1 reads block by block), as [x][y][z] arrays with a `Size`: a cell's material is its most common solid block's (Air when it has none, Water for water) and its occupancy how much of it is solid. One call reads at most 4096 cells and 65536 blocks, and none past the world's edge, 30,000,000 blocks out from 0, 0 (an error). `WorldToCell` (and its `PreferSolid` and `PreferEmpty`), `CellCenterToWorld` and `CellCornerToWorld` turn positions into cells and back. What would change the terrain (`FillBlock`, `FillBall`, `WriteVoxels`, `Clear` and the rest) is an error: a command changes the world here (`game.run("fill ...")`). Reading the map doesn't start mirroring the entities.
- **RunService** fires PreRender, RenderStepped, Heartbeat, Stepped(time, dt) and the rest once a frame; `BindToRenderStep` works.
- **Tweens.** `TweenService:Create(inst, info, goals)` tweens numbers, booleans, Vector2, Vector3, CFrame, Color3, UDim, UDim2; `Play`, `Pause`, `Cancel`, `Completed`, `PlaybackState`; `GetValue`. GuiObject's TweenPosition, TweenSize and TweenSizeAndPosition too.
- **The client.** `game.JobId` is the server's address and `game.PlaceId` (and `GameId`) a number of its own, the same each time (a world of the player's own has an empty JobId and a number of its name), kept after a disconnect; `MarketplaceService:GetProductInfo(game.PlaceId).Name` is the server's address or the world's name. A player's chat line (vanilla's `<Name> text`, a whisper, and the servers' `[Rank] Name: text` and `Name » text`, from a player on the server) fires TextChatService's `MessageReceived` (and RBXGeneral's) with a TextChatMessage (`Text`, `TextSource.UserId`, `Status`, `TextChannel`) and the Player's `Chatted`; `/w Name text` sent to the chat is Minecraft's `/msg`. With `BubbleChatConfiguration.Enabled` set true, each line shows over the speaker's head for 15 seconds. `LocalPlayer:Kick(why)` leaves the server with that reason, and `game:Shutdown()` as Disconnect does; `TeleportService:Teleport(game.PlaceId)` and `TeleportToPlaceInstance(game.PlaceId, game.JobId)` rejoin it half a second later (`TeleportCancel` stops that, `GetLocalPlayerTeleportData` gives the data it carried there, and `queue_on_teleport`'s code runs there); another place fails with TeleportInitFailed and a line in chat. GuiService's `ErrorMessageChanged` and `GetErrorMessage` are the Disconnected screen's reason, and its `ToggleFullscreen` toggles full screen. `UserSettings():GetService("UserGameSettings").MasterVolume` is the game's master volume; `settings().Rendering.QualityLevel` 1 to 3 lowers the graphics (render distance, particles, clouds, entity shadows) until it is set back; `RunService:Set3dRenderingEnabled(false)` stops drawing the terrain. `StarterGui:SetCoreGuiEnabled` hides the game's chat (Chat, and TextChatService's `ChatWindowConfiguration.Enabled`), its player list (PlayerList) or its HUD (All), and `ChatWindowConfiguration.BackgroundTransparency` is how see-through the chat's background is; `SetCore("DevConsoleVisible", true)` opens the studio's console. `CoreGui:TakeScreenshot()` saves one in the studio's `screenshots` folder and returns where. VoiceChatInternal's `SubscribePause(userId, paused)` and `SubscribePauseAll(paused)` mute other players' voices in the voice chat (for this player alone), and `SoundService:SetListener(Enum.ListenerType.ObjectPosition, part)` hears voices from that part. The LocalPlayer's `Idled` fires after two minutes without input, and a VirtualInputManager or VirtualUser input is activity the server sees (no idle kick). The recorder, the inspect menu and friend requests have no Minecraft counterpart: they say so in chat. A PointLight, SpotLight or SurfaceLight on a part in the Workspace lights the view around it (the nearest 8, with their `Color`, `Range` up to 16 blocks and `Brightness`), and `Camera:PanUnits(n)` turns the view 45 degrees a unit. Setting `Lighting.ClockTime` back to a value read from it while it was the world's own time (as a script restores the lighting it kept) gives the time back to the world.
- **Other services:** Debris (`AddItem`), HttpService (`JSONEncode`, `JSONDecode`, `GenerateGUID`, `UrlEncode`; its requests aren't here: use `http.*`), TextService (`GetTextSize`, and `GetTextBoundsAsync(params)` with a GetTextBoundsParams's `Text`, `Font`, `Size`, `Width`, where 0 doesn't wrap, and `RichText`, measured run by run), StarterGui (`SetCore("SendNotification", {Title, Text})` shows it in chat, `SetCoreGuiEnabled`; a ScreenGui or BillboardGui put in it is copied into the PlayerGui at the next frame, once and with what it holds then, as a player's spawn copies it in Roblox, and changes to the one in StarterGui don't show; the copy is another instance, so connect to it (`PlayerGui:WaitForChild(name)`): what is connected on the one in StarterGui stays with that one), CollectionService (tags), Lighting (`ClockTime` and `TimeOfDay` follow the world's time), ReplicatedStorage and the other containers, the services scripts get at their top though they have nothing to do here (VirtualUser, VirtualInputManager, Stats, PathfindingService, PhysicsService, SocialService, BadgeService, GroupService, LocalizationService, PolicyService, ProximityPromptService, VRService, HapticService, GamepadService, AssetService, InsertService, MaterialService: they take any property, and a member they haven't is nil), and Sound (`Play` plays a sound event's name in `SoundId`, such as `"entity.experience_orb.pickup"`; asset ids are silent).

**Instances.** Every Instance has Name, ClassName, Parent, Archivable; `FindFirstChild(name[, recursive])`, `FindFirstChildOfClass`, `FindFirstChildWhichIsA`, `FindFirstAncestor`, `FindFirstAncestorOfClass`, `FindFirstAncestorWhichIsA`, `FindFirstDescendant`, `GetChildren`, `GetDescendants`, `IsA`, `IsDescendantOf`, `IsAncestorOf`, `GetFullName`, `Destroy`, `ClearAllChildren`, `Clone`, `WaitForChild(name[, timeout])` (warns after 5 seconds, as Roblox does), attributes (`SetAttribute`, `GetAttribute`, `GetAttributes`) and tags (`AddTag`, `HasTag`, `RemoveTag`, `GetTags`); events Changed, ChildAdded, ChildRemoved, DescendantAdded, DescendantRemoving, AncestryChanged, Destroying, AttributeChanged. A child is reached by its name (`gui.Frame.Title`) after the properties, events and methods. Properties are checked as Roblox checks them (`Unable to assign property Size. UDim2 expected, got string`); enums take an EnumItem, its name or its number.

**Classes Instance.new makes:** Folder, Configuration, Model, Part, MeshPart, WedgePart, CornerWedgePart, TrussPart, SpawnLocation, Seat, Camera, Humanoid, StringValue, NumberValue, IntValue, BoolValue, ObjectValue, Vector3Value, CFrameValue, Color3Value, BrickColorValue, BindableEvent, BindableFunction, Sound, LocalScript, Script, ModuleScript, ScreenGui, Frame, CanvasGroup, TextLabel, TextButton, TextBox, ImageLabel, ImageButton, ScrollingFrame, ViewportFrame and VideoFrame (frames: their 3D view and their video aren't drawn, and `VideoFrame:Play()` only fires Played), UICorner, UIStroke, UIGradient, UIPadding, UIScale, UIAspectRatioConstraint, UISizeConstraint, UITextSizeConstraint, UIListLayout, UIGridLayout, UIFlexItem, UIPageLayout, UITableLayout, BillboardGui, GetTextBoundsParams; and, kept with whatever properties they get but not drawn, SurfaceGui, UIDragDetector (nothing drags by it, and its events never fire), Decal, Texture, lights, ParticleEmitter, Beam, Trail, Attachment, welds, Highlight, ProximityPrompt, ClickDetector, Animation, Tool, RemoteEvent, RemoteFunction and post effects.

**`Enum`** has every enum these classes use (KeyCode, UserInputType, UserInputState, EasingStyle, EasingDirection, Font, FontWeight, Material, PartType, the GUI ones, CameraType, HumanoidStateType, CoreGuiType, RaycastFilterType, RenderPriority...): `Enum.KeyCode.E` with `Name`, `Value`, `EnumType`, `IsA`; `Enum.KeyCode:GetEnumItems()`, `FromName`, `FromValue`.

**Modules and ModuleScripts.** In a module (a folder whose `main.lua` requires its other files), `script` in main.lua is the mod's LocalScript, and every other file is a ModuleScript under it, named after the file: `lib.lua` is `script.lib`, and `lib/util.lua` is `script.lib.util` (a folder in a file's path is a Folder, or the ModuleScript of a file of the same name, as Rojo nests them). Inside a file, `script` is that file's own ModuleScript, so `require(script.Parent.shared)` works there as in Roblox, and `require(script.lib)` gives the same value as `require("lib")`: each file runs once. The files still share the mod's globals (a file's `getfenv()` is a table of its own that only holds `script`). A file named after an Instance member (`Name.lua`, `Parent.lua`) is reached with `script:FindFirstChild("Name")`, and `init.lua` is not special. Another mod's file ModuleScripts can't be required: share values with `export` and `import`.

```lua
-- mymod/main.lua
local lib = require(script.lib)   -- mymod/lib.lua, the same as require("lib")
print(lib.greet("you"))

-- mymod/lib.lua would be:
-- local shared = require(script.Parent.shared)
-- return { greet = function(name) return shared.greeting .. ", " .. name end }
```

**Remotes are Minecraft's packets.** `ReplicatedStorage.Minecraft` holds a RemoteEvent for each play packet, named after vanilla's name for it in PascalCase (`move_player_pos` is `MovePlayerPos`, `custom_payload` `CustomPayload`, `use_item_on` `UseItemOn`; a name both directions use is one remote), and a RemoteFunction for each request the server answers (`CommandSuggestion`, `PingRequest`, `BlockEntityTagQuery`, `EntityTagQuery`).

- **Fields.** A packet is one table of its fields, by the names Horizon gives them: `MovePlayerPos` is `{ x = 10.5, y = 64, z = -3.25, flags = 1 }` (flags: 1 on the ground, 2 against a wall). A block position (`pos`) or a vector is a Vector3, text and bytes are strings (a buffer is taken too), a UUID is its text, a whole number past 2^53 its digits as a string, an absent optional field nil. The packets whose fields are plain values map both ways: movement (`MovePlayerPos`, `MovePlayerPosRot`, `MovePlayerRot`, `MovePlayerStatusOnly`, `MoveVehicle`, `AcceptTeleportation`), `Interact`, `Attack`, `UseItem`, `UseItemOn`, `PlayerAction`, `PlayerCommand`, `PlayerInput`, `SetCarriedItem`, `CustomPayload`, `ChatCommand`, `CommandSuggestion`, `ClientCommand`, containers' clicks on buttons and closes, `SignUpdate`, `EditBook`, `RenameItem`, `SelectTrade`, `SetBeacon`, `PickItemFromBlock`/`Entity`, `PaddleBoat`, `ClientTickEnd` and the like. Any other (the signed `Chat`, `ContainerClick`, `SetCreativeModeSlot`, every clientbound packet) is read from the game's own form of it, to look at (a nested value is a table naming its `type`; an enum's unit value is its name), and comes with its payload (the bytes after the id) as a string after the table.
- **The game's packets reach `__namecall` hooks.** While a script has hooked `__namecall` (with `hookmetamethod`, or by setting it in `getrawmetatable(game)`), or hooked `FireServer` with `hookfunction`, each packet the game sends goes through it as `remote:FireServer(fields[, payload])` (`InvokeServer` for a RemoteFunction's), with `checkcaller()` false and `getcallingscript()` nil, so a remote spy shows the client's real traffic. A hook that returns without calling the original stops the packet; one that calls it with other arguments sends those instead (a chat message changed goes out again through the chat, which signs it). A hook that errors lets the packet through (the error is printed once). The packets only the game may send go through as they were whatever the hook does: `KeepAlive`, `ChatAck`, `ChatSessionUpdate` and `ConfigurationAcknowledged` (the server disconnects without them). Without a hook the packets don't touch Lua at all; a hook restored with `hookmetamethod(game, "__namecall", old)` costs nothing again.
- **`FireServer` sends one**: its fields, or the payload as a string or buffer (`FireServer(payload)` or `FireServer(fields, payload)`). It is checked as the server will read it: it must decode as that packet, whole; positions must be finite and inside the world, angles finite, hands, faces, directions and actions ones vanilla knows. A bad one is an error naming the field, and nothing is sent: a script can't get the player kicked by a malformed packet. `Chat` (`{ message = "hi" }`) and `ChatCommand` (`{ command = "say hi" }`) go through the game's chat, which signs them; the packets only the game sends can't be fired, and a packet Horizon doesn't model is never sent from bytes it can't check. At most 256 a frame. Moving with `MovePlayerPos` tells the server where you are, as the client would; your own player stays where it is (the server may pull you back).
- **`InvokeServer`** sends the request and waits for the server's answer, returning its fields: `CommandSuggestion:InvokeServer({ id = 7, command = "/gamem" })` returns `{ id = 7, start = 1, length = 5, suggestions = { { text = "gamemode" } } }`. The answer is matched by its id (`transaction` for the tag queries, `time` for a ping).
- **`OnClientEvent`** fires for each clientbound packet with its fields and payload, deferred like every signal, while a script listens to that remote (`Connect`, `Once` or `Wait`): only then does the game read them.

```lua
local mc = game:GetService("ReplicatedStorage").Minecraft
local old
old = hookmetamethod(game, "__namecall", function(self, ...)
  local method = getnamecallmethod()
  if method == "FireServer" and self.Parent == mc and not checkcaller() then
    local fields = ...
    if self.Name == "UseItemOn" then print("used on", fields.pos) end
  end
  return old(self, ...)
end)
mc.SystemChat.OnClientEvent:Connect(function(fields) print("server says", fields.overlay) end)
mc.CustomPayload:FireServer({ channel = "minecraft:brand", data = "\7vanilla" })
```

[SimpleSpy](https://github.com/exxtremestuffs/SimpleSpySource) runs as it is: its window shows the game's packets and scripts' calls, the code to fire each again, its calling function and script.

**Not here:** a Roblox server (a script's own RemoteEvents go nowhere), physics, animations, Roblox assets (images, meshes, sounds by id), SurfaceGuis, dragging by UIDragDetectors, and what ViewportFrames and VideoFrames show.
"##;

const EXECUTOR: &str = r##"
Horizon Studio has the executor functions of the [sUNC](https://docs.sunc.io) standard, so scripts written for Roblox executors run as mods: closures and hooks, the debug library, metatables, Base64 and LZ4, the collector, environments, a file system, HTTP and WebSockets, the Drawing library, Instances, signals, scripts and hidden properties. Each has an entry here.

```lua
local old
old = hookmetamethod(game, "__namecall", function(self, ...)
  if getnamecallmethod() == "Destroy" and self == workspace then
    return warn("not destroying the workspace")
  end
  return old(self, ...)
end)

writefile("hello.txt", "Hello from " .. identifyexecutor())
local box = Drawing.new("Square")
box.Size = Vector2.new(120, 40)
box.Position = Vector2.new(20, 20)
box.Filled = true
box.Visible = true
```

**Environments.** A mod's globals are its own table; names it doesn't have come from `getgenv()` (the executor's: the studio's API and this library, shared by every mod), and from there from `getrenv()` (Roblox's: the standard library, `game`, `workspace`, `Instance`, the data types, `task`). `getgenv().x = 1` gives every mod an `x`; `getrenv().game = nil` takes `game` from them all.

**What is different from a Roblox executor**, because the scripts are mods and the game is Minecraft:

- Every Lua function and thread is the executor's: there are no game scripts. `checkcaller()` is true but while the game's own packets go through a `__namecall` hook (see `roblox`: remotes are Minecraft's packets), `getcallingscript()` is the calling mod's `script` (nil for the console and the game's packets), `getscriptfromthread()` is nil, and `filtergc`'s `IgnoreExecutor` (true by default) skips every Lua function: pass `IgnoreExecutor = false`.
- `getgc`, `filtergc`, `getreg` and live protos see what was made after the studio began tracking objects: from the load of the first mod whose source names one of them (or their first call), as tracking costs a little on every function and table made.
- Hooks are global: `hookfunction(print, ...)` changes `print` for every mod. They stay until `restorefunction` (or the game restarts), even after the mod that made them stops.
- `getscriptbytecode` is the studio's own dump of a mod's compiled main file, not Luau bytecode; `getscripthash` hashes that. A ModuleScript runs under `require` from its hidden `Source` (set with `sethiddenproperty`). A module's files are ModuleScripts too (see `roblox`), but their `Source` is nil: `require` runs the file itself, and `getloadedmodules` lists them once required. Once `require` has run one (from its top level on), it is a running script: `getrunningscripts` lists it, `getsenv` gives the file's own environment (`getfenv()` in the file: its `script` is that ModuleScript, everything else the mod's globals), and `getscriptclosure`, `getscriptbytecode` and `getscripthash` give the file's code. Before that, or when its top level failed, `getsenv` errors and the others give nil.
- As in Roblox, a method call goes to `__namecall` alone: `hookfunction(remote.FireServer, ...)` sees `remote.FireServer(remote, ...)` calls, not `remote:FireServer(...)` (remote spies hook both). A method call inside a `__namecall` hook names its own method, so read `getnamecallmethod()` first (or `setnamecallmethod` it back) before calling the original. A `newcclosure` is a C level of its own to `debug.info` and `debug.getinfo`, as in Luau.
- `replicatesignal` errors: there is no server. `fireclickdetector`, `fireproximityprompt` and `firetouchinterest` fire the events here, deferred like every signal.
- Files live in the studio's `workspace` folder (`horizon-mods/.horizon-mod-data/horizon_studio/workspace/`), which scripts can't leave. `request`, `game:HttpGet` and WebSockets reach the internet only, never this computer or its network.
- Drawings are in dp, Horizon's logical pixels (the Camera's ViewportSize), over the HUD and the mods' GUIs. Images (Drawing's `Data` and ImageLabels with a `getcustomasset` id) are PNGs, drawn as the GUI's images are.
- `base64encode`, `base64decode`, `lz4compress` and `lz4decompress` are globals. For the older [UNC](https://github.com/unified-naming-convention/NamingStandard) standard the `crypt` library is here too (`crypt.base64encode`, `crypt.generatekey`, `crypt.hash`, `crypt.encrypt`/`decrypt` over AES), with `base64` and `cache` tables; sUNC deprecates `crypt` (its test warns one exists), UNC needs it.
- UNC's console (`rconsoleprint`, `rconsoleclear`...) prints to the Studio console; its mouse input (`mouse1click`, `mousemoveabs`, `mousescroll`...) drives the Roblox GUI's pointer; `setclipboard` uses the system clipboard; `setfpscap`, `queue_on_teleport` and `messagebox` have nothing to act on here. UNC tests that read Roblox game content a mod can't have (CoreGui's modules, a character's Animate script) can't pass on Minecraft.

These functions widen what a script can do (see the `limits` topic): run a script from someone you trust.
"##;

const ERRORS: &str = r##"
Errors have the form `<file>:<line>: <message>`; in a handler the event's name comes first (`tick: mymod.lua:12: ...`), and the mod stops (`mymod stopped: ...`). The common ones:

| Message | Means | Fix |
| --- | --- | --- |
| `attempt to index a nil value (local 'p')` / `(global 'x')` / `(field 'y')` | indexing nil: a value is missing (often a player function outside a world, or a misspelt name) | check for nil first: `local x = player.pos() if not x then return end` |
| `attempt to call a nil value (global 'foo')` | no such function in this mod's globals (console-only functions, a typo, another mod's global) | spell it as in the API; share functions with `export`/`import` |
| `attempt to call a nil value (method 'x')` | `obj:x()` where `obj` has no `x` | check the object's type and fields |
| `attempt to perform arithmetic on a nil value` / `on a string value (not a number)` | maths on a missing value or text | `tonumber(...)` and nil checks |
| `attempt to concatenate a nil value` | `..` with nil (or a table, boolean) | `tostring(v)` |
| `attempt to compare number with string` / `two table values` | `<` between different types, or tables without `__lt` | convert first; give a comparator to `table.sort` |
| `bad argument #2 to 'on' (function expected, got nil)` | a wrong or missing argument; #N counts from 1 | pass the documented type |
| `unknown event 'x' (tick, frame, ...)` | `on` with an event that does not exist | one of the events, or a name with `:` |
| `hud.text draws only inside on("hud", function(w, h) ... end) or a screen's draw` | HUD drawing (`hud.*` or `gfx.*`, the message names it) outside `on("hud")` or a screen's draw | draw from `on("hud")`, or use `hud.panel` |
| `draw.line draws only inside on("world", function() ... end)` | `draw.*` elsewhere | keep the shapes in a variable and draw them in `on("world")` |
| `world.particle doesn't work while drawing: call it from tick, frame, a command or another event` | particles or sounds from `hud`/`world` handlers | spawn them in `on("tick")` |
| `agent functions are not available while drawing the HUD` | `agent.*`, `world.find` or `world.entities` in `on("hud")` | call them from a timer or tick, keep the result in a variable |
| `the agent is not available here` | the same, in the game, from `on("world")` or a screen's draw | as above |
| `Horizon's agent command 'find-block' needs the game: test this part live (the game running with the studio)` | agent commands in a headless test | test that part live, or keep it out of the headless steps |
| `'save' is not an agent command scripts can use` | a command outside the allowed list | see `agent` for the list |
| `busy with goto; wait for it or send stop` | an agent action while another runs | wait for the callback, or `agent.stop()` |
| `sending too fast: chat.say and game.run are limited to a few a second` | more than 8 at once, or over 4 a second | space them out with `after`/`every` |
| `/time is the game's own command` | `command` with a vanilla or studio name | pick another name |
| `/x is already used by othermod` | another running mod has that command | pick another name |
| `bad argument #1 to 'command' (a name of letters, digits, '_' or '-' expected)` | spaces or symbols in a command name | letters, digits, `_` and `-`, up to 32 |
| `too many commands (256)` | registering commands in a loop | fewer commands, with arguments (`args`) to choose what they do |
| `bad argument #3 to 'command' (args[1].kind: unknown kind 'x' (word, text, ...))` | a kind that does not exist | `word`, `text`, `int`, `number`, `bool`, `player`, `pos`, `item` or `block` |
| `bad argument #3 to 'command' (args[2]: a required argument can't follow an optional one)` / `(args[1]: only the last argument can be text)` | arguments in an order the chat can't read | optional arguments last, a `text` one at the very end |
| `bad argument #3 to 'command' (suggest: with args, each argument takes its own suggest)` | a command-level `suggest` beside `args` | put `suggest` in the argument it completes |
| `/home completions: homes.lua:9: ...` (in the console) | the command's `suggest` function failed; it is logged once and the mod runs on | fix the function; it is called again at the next keystroke |
| `no mod 'x' to import` | no such mod in the mods folder | check the id (the file or folder name) |
| `'x' exports nothing: it needs export(...) or to return a table` | the library neither called `export` nor returned a table | add `export({...})` or `return M` |
| `library 'x' failed: ...` / `library 'x' is turned off` | the library is broken or off | fix or turn on the library; this mod reloads with it |
| `library 'x' isn't loaded yet (do they import each other?)` | an import cycle, or an import the load-order scan missed | break the cycle; use `import("x")` with a literal id |
| `require works in modules (a folder with main.lua): a one-file script can't load other files` | `require` of a file in a one-file script, or in the console | make the mod a module: a folder with `main.lua` |
| `require('lib.x'): lib/x.lua not found in this module (its files: main.lua, util.lua)` / `bad module name '../x': ...` | no such file in the module / a name with `..`, a leading `/` or `.`, or other characters | a file of the module, with `.` or `/` between folders |
| `require: 'lib' is already loading (main.lua -> lib.lua -> shared.lua -> lib.lua): ...` | two files require each other while they load | move what both need to a third file, or `require` inside a function that runs later |
| `require: main.lua starts the module and runs once by itself; ...` | a file requires `main` | move what the other files need from main.lua into a file they require |
| `require: Players.LocalPlayer.PlayerScripts.a.lib is a file of the mod 'a': use import("a") for another mod's values` | `require` of another mod's file ModuleScript | have that mod `export` what it shares, and `import` it |
| `storage keys are 1 to 64 letters, digits, '_', '-' or '.'` | a key with spaces or symbols | use a simple key |
| `functions cannot be stored` / `value too large to store` / `tables nested too deep to store (or a cycle)` | storing what storage cannot keep | store plain data |
| `this mod stores too many keys (256)` | over 256 keys | keep one table under one key |
| `too many handlers (256)` / `too many timers (256)` | registering in a loop or on every tick | register once at load; cancel timers |
| `a mod attaches at most 64 models` | `model.attach` in a loop | reuse ids with `model.set` |
| `texture.load reads files of a folder mod (mods/<id>/main.lua); a one-file mod can use texture.decode or texture.create` | `texture.load`/`mesh.load` in a one-file mod | make it a folder mod, or use `texture.decode`, `texture.create` + `texture.paint`, `mesh.parse`, `mesh.new` |
| `texture.load: no file 'x.png' in the mod's folder` / `is not a path inside the mod's folder` | a wrong or escaping path | paths are relative to the mod's folder, no `..` |
| `a texture is 1 to 512 pixels a side (this one 600x600)` | a big image | scale it down |
| `the mods' textures are at most 1048576 pixels in all (a 1024x1024)` | many or big textures, often made again on every load | `texture.free` the ones not used; make them once |
| `draw.mesh: this mod has no mesh 3` | an id from another mod, a freed mesh, or a typo | meshes belong to the mod that made them |
| `mesh.parse: line 12: index 9 is not one of the 8 so far` | a broken .obj | fix the file's f line |
| `model.attach: give an item or boxes` | a spec with neither | add `item` or `boxes` |
| `bad argument #1 to 'attach' (a body part expected: root, head, body, ...)` | an unknown part | a player part name |
| `too much drawn in the world this frame` / `too much drawn through walls this frame` | over 16384 quads / 4096 overlay items | draw less, or only what is near |
| `too many particles this frame (512)` / `too many sounds this frame (32)` | over the cap | spawn fewer, or every few ticks |
| `script ran too long and was stopped (an endless loop?)` | out of steps (cannot be caught) | end the loop; spread work over ticks |
| `stack overflow` | recursion over 200 calls deep | use a loop |
| `table is too large` / `string is too long` | over 1,048,576 entries / 4 MiB (cannot be caught) | keep data smaller |
| `goto and labels are not supported` | `goto` | use loops, `break` and functions |
| `'end' expected (to close 'function' at line 3) near '<eof>'` | a missing `end` | close every `function`, `if`, `for`, `while`, `do` |
| `unexpected symbol near '='` / `'then' expected near 'x'` | a syntax error at that line | fix the line |
| `invalid escape sequence` / `unfinished string` | a bad `\` escape / a quote not closed on its line | `\\` for a backslash; `[[...]]` for text over several lines |
| `number has no integer representation` | a fraction where a whole number is needed (bitwise, `%d`, `string.rep`...) | `math.floor(x)` |
| `'%a' is not supported by 'format'` | `%a` in `string.format` | another conversion |
| `bad argument #1 to 'random' (interval is empty)` | `math.random(m, n)` with m > n (or `math.random(0)`) | swap them |
| `index is nil` / `index is NaN` | `t[nil] = v` or `t[0/0] = v` | check the key |
| `cannot change a protected metatable` | `setmetatable` on a table whose metatable has `__metatable` | use another table |
| `ViaHorizon isn't available here` | `via.enable`/`via.version` headless | test it live |
"##;

// ------------------------------------------------------------------ entries

pub const ENTRIES: &[Entry] = &[
    // ------------------------------------------------------------- globals
    e!(
        "on",
        "events",
        r##"
`on(event, fn)`

Registers `fn` to run on an event. Several handlers of one event run in the order they were registered. There is no way to remove a handler (no `off`): keep a flag and return early, or reload the mod. Registering works anywhere, also inside a handler (the new one runs from the next time the event fires).

- `event` (string): `"tick"`, `"frame"`, `"hud"`, `"world"`, `"key"`, `"chat"`, `"unload"`, or a mod-to-mod event: any name with a `:` in it, at most 64 characters (`"waypoints:changed"`).
- `fn` (function): gets the event's arguments: `tick()`, `frame(dt)`, `hud(w, h)`, `world()`, `key(key, down)`, `chat(text)`, `unload()`, and for a mod-to-mod event the values given to `emit`.
- Returns: nothing.
- Where: anywhere.
- Errors: `bad argument #1 to 'on' (unknown event 'x' (tick, frame, hud, world, key, chat, unload, or one of "mymod:name" for emit))`; `bad argument #2 to 'on' (function expected, got nil)`; `too many handlers (256)` (all of a mod's events together).

An error inside `fn` stops the mod (`tick: mymod.lua:4: ...`). See the `event:*` entries for each event.

```lua
on("tick", function()
  local x = player.pos()
  if not x then return end -- not in a world
end)
on("key", function(key, down)
  if key == "key.keyboard.r" and down then chat.show("R pressed") end
end)
```
"##
    ),
    e!(
        "command",
        "api",
        r##"
`command(name, fn[, help | options])`

Adds the chat command `/name`. When the player types `/name ...` in chat, the studio takes it before the server and `fn(rest, words)` runs on the next frame. If `fn` returns a value other than nil, the player's chat shows it as plain text (numbers formatted, tables as `table: 0x...`; no prefix). An error in `fn` does not stop the mod: the chat shows `/name: <message>` and the console logs it with a traceback.

In the game the command is in the chat's suggestions too, beside the server's commands: typing `/` lists it, Tab completes its name, the grey line under the box shows what it takes, and it is not red as an unknown command. An options table declares its arguments and their completions.

- `name` (string): 1 to 32 letters, digits, `_` or `-`; a leading `/` is dropped. Registering the same name again in the same mod replaces it.
- `fn(rest, words)`: `rest` is the text after the name, trimmed (`""` when there is none); `words` is `rest` split on whitespace, a list (`words[1]` is the first word, nil when none). It gets the line as typed whatever the arguments declare: their kinds give the chat its colours, completions and usage line, and check nothing.
- `help` (string, optional): the description, shown in the Library and by `/studio help`.
- `options` (table, optional), in place of `help`:
  - `description` (string): the same as `help`.
  - `args` (a list of at most 16): the arguments, in order. Each is a name (a required word), or a table:
    - `name` (string): 1 to 32 letters, digits, `_` or `-`; the usage line shows `<name>`.
    - `kind` (string, default `"word"`): what it takes; see the table below.
    - `optional` (boolean): the usage line shows `[<name>]`. A required argument can't follow an optional one, and only the last argument can be `text`.
    - `min`, `max` (numbers): the bounds of an `int` (whole numbers from -2147483648 to 2147483647) or a `number` (finite); the chat shows the game's red range error outside them. Other kinds ignore them.
    - `options` (a list of at most 256): fixed completions, each a string, a number, or `{text = "clear", tooltip = "Removes every waypoint"}` (the chat shows the tooltip over it).
    - `suggest` (function): more completions, `suggest(rest, words, name)` (see below).
  - `suggest` (function): completes a command that declares no `args` (its free text), at the word typed. With `args`, each argument takes its own.
  - Other keys are ignored.
- Returns: nothing.
- Where: anywhere. The command lasts until the mod reloads or stops. A mod (and the console) has at most 256 commands.

| Kind | Takes | In the chat |
| --- | --- | --- |
| `word` | one word: anything but a space | no completions of its own; a word that starts with `@` reads as a player selector |
| `text` | the rest of the line (the last argument only) | no completions of its own |
| `int` | a whole number | red when it is not one, or outside `min` and `max` |
| `number` | a number | the same, for any number |
| `bool` | `true` or `false` | completes them |
| `player` | a player's name, or a selector (`@p`, `@s`) | completes the players online and the selectors |
| `pos` | a block position: three numbers or `~` (`~ ~1 ~`), or `^ ^ ^` | completes `~ ~ ~` and the block looked at |
| `item` | an item id (`minecraft:diamond_sword`) | completes item ids |
| `block` | a block id, with states (`minecraft:oak_stairs[facing=east]`) | completes block ids and states |

Without `args` a command takes any text after its name (the usage line shows `[<args>]`); `args = {}` takes nothing.

Completions: when the chat comes to an argument that has `options` or a `suggest` function, it asks the studio for its completions, as it asks a server for a server's, and the list shows a frame later; an argument with neither keeps the game's own completions for its kind (the table's), and one with either has the studio's in their place. What is completed is the argument at the cursor: a one-word argument at its word, a `text` argument as a whole (the option `"the base"` completes `/wp remove the b`), a `pos` as a whole from its first coordinate (the option `"~ ~ ~"` completes `/mark ~ `), and a command without `args` at the word typed.

- `suggest(rest, words, name)` gets the text after the command's name up to the cursor, its words, and the argument's name (nil for a command without `args`). Unlike `fn`'s, `rest` keeps its trailing spaces and `words` always ends with the word being typed: `""` after a space (`"/wp remove "` gives `"remove "` and `{"remove", ""}`).
- It returns a list of strings, numbers or `{text = , tooltip = }` tables (other entries are skipped), or nil for none.
- The studio keeps what matches the text typed, as the game's own completions match (at the start, or after a `.`, `_` or `/`, in any case: `ho` matches `go_home`; for an `item` or a `block`, as the game matches ids, also after the id's `:` when no `:` is typed, so `dia` matches `minecraft:diamond`, and an id without a namespace is `minecraft`'s), and leaves out the text typed itself, empty texts, texts over 256 bytes and, but for `text` and `pos` arguments and free text, texts with a space. Tooltips are cut to 256 bytes, and at most 256 completions go to the chat, which sorts them.
- It runs in the normal phase, with a handler's budget, on every keystroke at that argument: keep it quick. It can't wait.
- An error in it does not stop the mod: the console logs `/name completions: <message>` once, until the message changes or the function works again, and the `options` are still offered. One that runs out of steps is not called again until the mod reloads (for a command of the console's, until `command` registers it again).

Where the commands show: on servers (vanilla, Paper, proxies, older versions through ViaHorizon) and in single player and LAN, where the game joins its own server. A script's command hides the server's command of the same name, which the studio takes first anyway. They do not show in a command block's editor (command blocks run on the server; until you type there, its first list may still have them), after `/execute ... run` (the server runs what follows), before a server has sent its commands, or in the deprecated in-frame single player: there they run but are not suggested. Nothing typed after their names reaches the server, completions included.

- Errors: `bad argument #1 to 'command' (a name of letters, digits, '_' or '-' expected)`; `/name is the game's own command` (names of vanilla commands and the studio's: `studio`, `lua`, `advancement`, `attribute`, `ban`, `ban-ip`, `banlist`, `bossbar`, `clear`, `clone`, `damage`, `data`, `datapack`, `debug`, `defaultgamemode`, `deop`, `dialog`, `difficulty`, `effect`, `enchant`, `execute`, `experience`, `xp`, `fetchprofile`, `fill`, `fillbiome`, `forceload`, `function`, `gamemode`, `gamerule`, `give`, `help`, `item`, `jfr`, `kick`, `kill`, `list`, `locate`, `loot`, `me`, `msg`, `tell`, `w`, `op`, `pardon`, `pardon-ip`, `particle`, `perf`, `place`, `playsound`, `publish`, `random`, `recipe`, `reload`, `return`, `ride`, `rotate`, `save-all`, `save-off`, `save-on`, `say`, `schedule`, `scoreboard`, `seed`, `setblock`, `setidletimeout`, `setworldspawn`, `spawnpoint`, `spectate`, `spreadplayers`, `stop`, `stopsound`, `stopwatch`, `summon`, `tag`, `team`, `teammsg`, `tm`, `teleport`, `tp`, `tellraw`, `test`, `tick`, `time`, `title`, `transfer`, `trigger`, `version`, `waypoint`, `weather`, `whitelist`, `worldborder`); `/name is already used by <mod id>` (another running mod has it); `too many commands (256)`.
- Errors in the options table are `bad argument #3 to 'command' (...)`, saying what is wrong: `(string or table expected, got boolean)`; `(description: string expected)`; `(suggest: function expected)`; `(suggest: with args, each argument takes its own suggest)`; `(args: a list of at most 16 arguments expected)`; `(args[2]: a name or a table expected)`; `(args[2]: a name of letters, digits, '_' or '-' expected)` (a name alone) or `(args[2].name: ...)`; `(args[2].kind: unknown kind 'x' (word, text, int, number, bool, player, pos, item or block))`; `(args[1].optional: true or false expected)`; `(args[1].min: a whole number from -2147483648 to 2147483647 expected)` (an `int`), `(args[1].min: a finite number expected)` (a `number`), `(args[1].min is more than its max)`; `(args[1].options: a list of at most 256 strings or {text, tooltip} tables expected)`; `(args[1].suggest: function expected)`; `(args[2]: only the last argument can be text)`; `(args[2]: a required argument can't follow an optional one)`.

`game.run("name args")` runs a mod's command too. In tests: `{"command": "/name args", "expect_chat": "..."}`, and `{"suggest": "/name words", "expect_output": "..."}` for its completions (headless).

```lua
command("greet", function(rest, words)
  if #words == 0 then return "Usage: /greet <name> [times]" end
  local times = tonumber(words[2]) or 1
  for i = 1, math.min(times, 5) do chat.show("Hello, " .. words[1] .. "!") end
end, "Greets someone: /greet <name> [times]")

-- The same with its arguments declared: the chat shows /hi <who> [<times>],
-- completes the players online, and shows a times over 5 in red.
command("hi", function(rest, words)
  local times = tonumber(words[2]) or 1
  for i = 1, math.min(times, 5) do chat.show("Hello, " .. words[1] .. "!") end
end, {
  description = "Greets someone",
  args = {
    { name = "who", kind = "player" },
    { name = "times", kind = "int", min = 1, max = 5, optional = true },
  },
})
```
"##
    ),
    e!(
        "after",
        "api",
        r##"
`after(seconds, fn)`

Calls `fn()` once, `seconds` from now on the studio's clock (`game.time()`, which runs in a world or not). Timers are checked once a frame, so they fire on the first frame at or after the time; `after(0, fn)` runs on the next frame's timers.

- `seconds` (number): 0 or more.
- `fn` (function): called with no arguments.
- Returns: the timer's id (a number) for `cancel`. Ids are unique across all mods.
- Where: anywhere.
- Errors: `bad argument #1 to 'after' (a positive number of seconds expected)` (negative, `inf` or `nan`); `too many timers (256)` (per mod, `after` and `every` together). An error inside `fn` stops the mod (`timer: ...`).

```lua
command("countdown", function()
  for i = 3, 1, -1 do
    after(4 - i, function() chat.show(tostring(i)) end)
  end
  after(3, function() chat.show("Go!") end)
end)
```
"##
    ),
    e!(
        "every",
        "api",
        r##"
`every(seconds, fn)`

Calls `fn()` every `seconds` on the studio's clock, first `seconds` from now, until cancelled or the mod reloads. It runs at most once a frame: when a frame is late, the next call is due `seconds` after the one that was due (never in the past).

- `seconds` (number): at least 0.01.
- `fn` (function): called with no arguments.
- Returns: the timer's id for `cancel`.
- Where: anywhere.
- Errors: `bad argument #1 to 'every' (a positive number of seconds expected)` (below 0.01, `inf` or `nan`); `too many timers (256)`. An error inside `fn` stops the mod (`timer: ...`).

```lua
local ticks = 0
local id = every(1, function()
  ticks = ticks + 1
  hud.panel("top-right", { "Seconds: " .. ticks })
end)
command("stopclock", function() cancel(id) return "Stopped" end)
```
"##
    ),
    e!(
        "cancel",
        "api",
        r##"
`cancel(id)`

Stops a timer this mod made with `after` or `every`. An id that is not one of this mod's timers (already run, cancelled, or another mod's) is ignored.

- `id` (number): from `after` or `every`.
- Returns: nothing.
- Where: anywhere, also inside the timer's own function.
- Errors: `bad argument #1 to 'cancel' (number expected, got nil)`.

```lua
local blink = every(0.5, function() sound.play("block.note_block.hat", 0.3) end)
after(5, function() cancel(blink) end)
```
"##
    ),
    e!(
        "require",
        "libraries",
        r##"
`require(name)` or `require(moduleScript)`

Loads another file of a module (a folder `<id>/` that starts at `main.lua`) and returns its result. The file runs once, in the mod's own globals, with `name` as its `...`; its first result is kept (`true` when it returns nothing), and every later `require` of that file returns the kept value without running it again, however the name is written: `require("lib.util")`, `require("lib/util")` and `require("lib/util.lua")` are the same file and give the same value.

Each file of a module is also a ModuleScript under `script` (see `roblox`), and requiring that is requiring the file: `require(script.lib)` in main.lua, or `require(script.Parent.shared)` in lib.lua, gives the same value as `require("lib")` or `require("shared")`. Another mod's file ModuleScripts can't be required (use `import`). A ModuleScript made with `Instance.new` runs its hidden `Source` instead (see `executor`).

- `name` (string): a file of the module, with `.` or `/` between folders, without or with `.lua`: `"util"` is `util.lua`, `"lib.util"` and `"lib/util"` are `lib/util.lua`. Each part is letters, digits, `_` and `-`, at most three folders deep, so a name never reaches outside the module's folder.
- Returns: the file's value.
- Where: anywhere in a module (usually at the top of a file). A required file's top level runs inside the `require` call, so it can't wait (`task.wait`).
- Errors: `require works in modules (a folder with main.lua): a one-file script can't load other files`; `bad module name '../x': name a file of the module, as "lib" or "lib.util"`; `require('lib.x'): lib/x.lua not found in this module (its files: main.lua, shared.lua)`; `require: 'lib' is already loading (main.lua -> lib.lua -> shared.lua -> lib.lua): ...` when two files require each other while they load; `require: main.lua starts the module and runs once by itself; ...`; `require: Players.LocalPlayer.PlayerScripts.a.lib is a file of the mod 'a': use import("a") for another mod's values`; the file's own syntax or runtime errors (`mymod/lib/util.lua:3: ...`).

A change to any `.lua` file of the module reloads the whole module. For code shared between mods, use a library (`export`/`import`).

```lua
-- mymod/main.lua
local util = require("lib.util")   -- mymod/lib/util.lua
print(util.twice(21))

-- mymod/lib/util.lua would be:
-- local M = {}
-- function M.twice(x) return x * 2 end
-- return M
```
"##
    ),
    e!(
        "color",
        "gfx",
        r##"
`color(r, g, b[, a])`

Builds a color number `0xAARRGGBB` from components 0 to 255 (kept within 0..255, fractions dropped). Every color argument of the API takes it, as well as strings.

- `r`, `g`, `b` (numbers, default 0), `a` (number, default 255: opaque).
- Returns: a number (`color(255, 0, 0)` is 4294901760, `0xffff0000`).
- Where: anywhere.

Every color argument (in `hud.*`, `gfx.*`, `draw.*`, `world.particle`, a label widget's `color`, a model box's `color`) takes:

- `"#rrggbb"` or `"rrggbb"`: opaque;
- `"#aarrggbb"` or `"aarrggbb"`: with alpha (`"#80000000"` is half-transparent black);
- a number: `0xAARRGGBB` exactly; `0xff0000` has alpha 0 and draws nothing (use `0xffff0000`). `hud.text` and `draw.text` ignore alpha.

`"#rgb"` is short for `"#rrggbb"` (`"#fff"` is white). Bad colors raise `bad argument #N to 'color' ('#rgb', '#rrggbb' or '#aarrggbb' expected)`.

```lua
local red = color(255, 0, 0)
local glass = color(255, 255, 255, 64)
on("hud", function(w, h)
  hud.rect(10, 10, 50, 10, red)
  hud.rect(10, 22, 50, 10, glass)
  hud.rect(10, 34, 50, 10, "#8000ff00")
end)
```
"##
    ),
    e!(
        "export",
        "libraries",
        r##"
`export(value)`

Sets what other mods get with `import("<this mod's id>")`: usually a table of functions. The last call wins; `export(nil)` offers nothing again. A main file that returns a table exports it, unless it called `export`.

- `value` (any value).
- Returns: nothing.
- Where: anywhere (usually at load).

When this mod reloads, the mods that import it reload too, so they get the new exports. Exported functions run as the mod that calls them.

```lua
-- mods/colors.lua
local palette = { accent = "#9a86ff", warn = "#f07a68" }
export({
  get = function(name) return palette[name] end,
  names = function()
    local out = {}
    for k in pairs(palette) do out[#out + 1] = k end
    return out
  end,
})
```
"##
    ),
    e!(
        "import",
        "libraries",
        r##"
`import(id)`

Returns another mod's exports (see `export`): the same value each time, not a copy. The studio loads libraries before the mods that import them (it scans files for `import("id")` with a literal id), and reloads a mod when a library it imported reloads, appears, or goes.

- `id` (string): the library's mod id (its file or folder name).
- Returns: the exports.
- Where: anywhere; usually at the top of the main file. The console can import too (handy in tests).
- Errors: `no mod 'x' to import`; `a mod cannot import itself`; `library 'x' failed: <its error>`; `library 'x' is turned off`; `library 'x' isn't loaded yet (do they import each other?)`; `'x' exports nothing: it needs export(...) or to return a table`.

The dependency is recorded even when `import` fails, so the mod reloads by itself once the library is there.

```lua
local kit = import("uikit")
local win = kit.window({ title = "Hello" }, function(ui)
  ui.text("A window from the uikit library.")
  if ui.button("Close") then ui.close() end
end)
command("hello", function() win:open() end)
```
"##
    ),
    e!(
        "emit",
        "libraries",
        r##"
`emit(event, ...)`

Sends a mod-to-mod event: on the next frame, the handlers every running mod (this one included) and the console registered with `on(event, fn)` run with the values.

- `event` (string): a name with a `:` in it, at most 64 characters; by convention `"<your mod id>:<what>"`.
- `...`: up to 16 values (more are dropped); tables are passed as they are, not copied.
- Returns: nothing.
- Where: anywhere.
- Errors: `bad argument #1 to 'emit' (an event name with a ':' expected ("mymod:changed"))`; `too many events waiting (1024)`.

```lua
command("score", function(rest)
  emit("scores:changed", tonumber(rest) or 0)
end)
on("scores:changed", function(score)
  hud.panel("top-right", { "Score: " .. score })
end)
```
"##
    ),
    // ------------------------------------------------------------- chat
    e!(
        "chat.show",
        "api",
        r##"
`chat.show(text)`

Shows a line in the player's own chat that only they see; nothing is sent to the server, and `on("chat")` handlers do not get it. A mod's lines start with `[<mod name>] ` (the header's name, else the id); the console's have no prefix.

- `text` (any value): turned into text with `tostring` (so `__tostring` works).
- Returns: nothing.
- Where: anywhere; in the game, inside `on("world")` or a screen's draw it shows a frame later (the game is only read then).

In tests: `expect_chat`.

```lua
command("where", function()
  local x, y, z = player.block()
  if x then chat.show(("You are at %d %d %d"):format(x, y, z)) end
end)
```
"##
    ),
    e!(
        "chat.say",
        "api",
        r##"
`chat.say(text)`

Sends a chat message to the server as the player (everyone on the server sees it). Leading `/` are removed, so it cannot run commands: use `game.run` for those.

- `text` (string or number).
- Returns: nothing.
- Where: anywhere; in the game, inside `on("world")` or a screen's draw it is sent a frame later.
- Errors: `sending too fast: chat.say and game.run are limited to a few a second` (the two share a budget: a burst of 8, then 4 a second); `bad argument #1 to 'say' (string expected, got nil)`.

In tests: `expect_sent`.

```lua
command("gg", function() chat.say("gg") end)
```
"##
    ),
    // ------------------------------------------------------------- game
    e!(
        "game.run",
        "api",
        r##"
`game.run(command)`

Runs a command as the player, as if typed in chat: the server's (`"time set day"`, if the player may run it) or a mod's own command (on the next frame).

- `command` (string): with or without the leading `/`; spaces around it are trimmed.
- Returns: nothing (the server answers in chat).
- Where: anywhere; in the game, inside `on("world")` or a screen's draw it runs a frame later.
- Errors: `sending too fast: chat.say and game.run are limited to a few a second` (shared with `chat.say`: a burst of 8, then 4 a second).

In tests: `expect_sent` sees `/time set day` (a mod's own command runs in the studio and isn't sent); headless, a command no mod owns is noted in chat as `(the game would run /time set day)`.

```lua
command("day", function() game.run("time set day") end)
```
"##
    ),
    e!(
        "game.fps",
        "api",
        r##"
`game.fps()`

The frame rate: frames in the last second, the slowest frame and the 1% low of the last 5 seconds (both as frames per second).

- Returns: three numbers. Headless: 20, 20, 20.
- Where: anywhere.

```lua
on("hud", function(w, h)
  local fps, slowest, low = game.fps()
  hud.text(4, h - 12, fps .. " fps (low " .. low .. ")", "#ffffff")
end)
```
"##
    ),
    e!(
        "game.screen",
        "api",
        r##"
`game.screen()`

The screen the game shows: `"playing"` (no screen), `"chat"`, `"inventory"`, `"container"`, `"station"`, `"creative"`, `"paused"`, `"options"`, `"title"`, `"dead"`, `"book"`, `"sign"`, `"dialog"`, `"stats"`, `"advancements"`, `"gamemode-switcher"`, `"world-options"`, `"friends"`, `"server-links"`, `"confirm-link"`, `"dialog-warning"`, `"command-block"` or `"win"`.

While the studio or a script screen is open, the game's chat screen is under it, so this is `"chat"`. Headless it is `"playing"`, and `"script"` while a script screen shows.

- Returns: a string.
- Where: anywhere.

```lua
on("key", function(key, down)
  -- on("key") also fires while the player types in chat
  if game.screen() ~= "playing" then return end
  if key == "key.keyboard.h" and down then chat.show("Hi") end
end)
```
"##
    ),
    e!(
        "game.time",
        "api",
        r##"
`game.time()`

Seconds on the studio's clock: since the studio started, adding each frame's time (a frame counts at most 1 second). Timers run on this clock. It is not the world's time (`world.time`) nor the real clock (`os.time`, `os.clock`).

- Returns: a number. Headless it grows by 0.05 a frame.
- Where: anywhere.

```lua
local started = game.time()
command("uptime", function()
  return string.format("Loaded %.1f s ago", game.time() - started)
end)
```
"##
    ),
    e!(
        "game.ticks",
        "api",
        r##"
`game.ticks()`

The game's tick counter: 20 ticks a second, and it moves only in a world.

- Returns: a number. Headless: one more every frame.
- Where: anywhere.

```lua
command("ticks", function() return "Tick " .. game.ticks() end)
```
"##
    ),
    // ------------------------------------------------------------- player
    e!(
        "player.pos",
        "api",
        r##"
`player.pos()`

The player's position in blocks (their feet), as numbers with fractions.

- Returns: `x, y, z`, or a single nil outside a world.
- Where: anywhere.

```lua
local x, y, z = player.pos()
if x then
  print(string.format("At %.2f %.2f %.2f", x, y, z))
end
```
"##
    ),
    e!(
        "player.block",
        "api",
        r##"
`player.block()`

The block position the player's feet are in: `player.pos()` floored.

- Returns: `x, y, z` (whole numbers), or nil outside a world.
- Where: anywhere.

```lua
local x, y, z = player.block()
if x then chat.show("Standing on " .. tostring(world.block(x, y - 1, z))) end
```
"##
    ),
    e!(
        "player.look",
        "api",
        r##"
`player.look()`

Where the player looks, in degrees. Yaw: 0 is south (+z), 90 west (-x), 180 north, 270 or -90 east; it is not kept within 0..360 (use `yaw % 360`). Pitch: -90 straight up, 0 level, 90 straight down.

- Returns: `yaw, pitch`, or nil outside a world.
- Where: anywhere.

```lua
local yaw, pitch = player.look()
if yaw then
  local heading = (yaw % 360 + 360) % 360
  print(string.format("Heading %.0f, pitch %.0f", heading, pitch))
end
```
"##
    ),
    e!(
        "player.facing",
        "api",
        r##"
`player.facing()`

The compass direction the player faces, from the yaw rounded to the nearest quarter (as the game's `Direction.fromYRot`).

- Returns: `"north"`, `"south"`, `"east"` or `"west"`; nil outside a world.
- Where: anywhere.

```lua
command("facing", function() return "You face " .. (player.facing() or "nothing") end)
```
"##
    ),
    e!(
        "player.dimension",
        "api",
        r##"
`player.dimension()`

The dimension the player is in: `"minecraft:overworld"`, `"minecraft:the_nether"`, `"minecraft:the_end"`, or a server's own id.

- Returns: a string, or nil outside a world.
- Where: anywhere.

```lua
if player.dimension() == "minecraft:the_nether" then chat.show("Hot in here") end
```
"##
    ),
    e!(
        "player.health",
        "api",
        r##"
`player.health()`

The player's health and maximum health (20 is ten hearts; the maximum comes from the max health attribute). Health can have a fraction.

- Returns: `health, max_health`, or nil outside a world.
- Where: anywhere.

```lua
every(1, function()
  local h, max = player.health()
  if h and h < max / 4 then chat.show("Low health: " .. string.format("%.1f", h)) end
end)
```
"##
    ),
    e!(
        "player.food",
        "api",
        r##"
`player.food()`

The hunger bar: food level (0 to 20) and saturation.

- Returns: `food, saturation`, or nil outside a world.
- Where: anywhere.

```lua
local food, saturation = player.food()
if food and food <= 6 then chat.show("Eat something") end
```
"##
    ),
    e!(
        "player.held",
        "api",
        r##"
`player.held()`

The stack in the selected hotbar slot.

- Returns: `item_id, count` (`"minecraft:diamond_sword", 1`), or a single nil for an empty hand (and outside a world).
- Where: anywhere.

```lua
local item, count = player.held()
if item then chat.show("Holding " .. count .. " x " .. item) else chat.show("Empty hand") end
```
"##
    ),
    e!(
        "player.info",
        "api",
        r##"
`player.info()`

All of the player at once, as a table: `x`, `y`, `z`, `yaw`, `pitch`, `facing`, `dimension`, `health`, `max_health`, `food`, `saturation`, `level` (experience level), `mode` (`"survival"`, `"creative"`, `"adventure"`, `"spectator"`), `slot` (the selected hotbar slot, 1 to 9), and when holding something `held` (item id) and `held_count`.

- Returns: a table, or nil outside a world.
- Where: anywhere.

```lua
local p = player.info()
if p and p.mode == "creative" then chat.show("Creative, slot " .. p.slot) end
```
"##
    ),
    e!(
        "player.pose",
        "models",
        r##"
`player.pose()`

The player's pose and movement: `{pose, crouching, swimming, gliding, sleeping, sprinting, on_ground, flying, camera}`. `pose` is `"standing"`, `"crouching"`, `"swimming"`, `"gliding"` (elytra), `"spin_attack"` (riptide) or `"sleeping"`; the next four are booleans for those poses; `sprinting`, `on_ground` and `flying` are booleans; `camera` is as `player.camera()`.

- Returns: a table, or nil outside a world.
- Where: anywhere. In tests: `{"pose": "crouching"}` (headless).

```lua
on("tick", function()
  local p = player.pose()
  if p and p.gliding then hud.panel("top-left", { "Gliding" }) else hud.panel(nil) end
end)
```
"##
    ),
    e!(
        "player.part",
        "models",
        r##"
`player.part(name[, target])`

Where a body part is this frame: its pivot and its axes. Use it to place things yourself (`draw.*`, particles) relative to a part.

- `name` (string): a player part (`root`, `head`, `body`, `right_arm`, `left_arm`, `right_leg`, `left_leg`, `hat`, `jacket`, `right_sleeve`, `left_sleeve`, `right_pants`, `left_pants`), or for another entity a part name of its model.
- `target` (optional): `"player"` (or nil) or an entity id from `world.entities`.
- Returns: `{x, y, z, left = {x, y, z}, up = {x, y, z}, forward = {x, y, z}}`: the pivot in world blocks and the part's left, up and forward axes, each one block long in a block-sized model (the player's are 0.9375 long; a model pixel is 1/16 of an axis). nil when there is no such part or no world.
- Where: anywhere.
- Errors: `bad argument #1 to 'part' (a body part expected: root, head, body, ...)` (an unknown player part, or a name that is not letters, digits and `_`); `player.part: target is "player" or an entity id (world.entities gives them)`.

```lua
on("world", function()
  local h = player.part("head")
  if not h then return end
  -- a point 10 model pixels above the neck pivot
  local px = 10 / 16
  local x, y, z = h.x + h.up.x * px, h.y + h.up.y * px, h.z + h.up.z * px
  draw.line(x, y, z, x, y + 1, z, "#ffd700", 0.03)
end)
```
"##
    ),
    e!(
        "player.camera",
        "models",
        r##"
`player.camera()`

The camera view (F5 changes it): `"first"`, `"third_back"` or `"third_front"`. In first person the player's own body (and the attachments without `first_person`) is not drawn.

- Returns: a string, or nil outside a world.
- Where: anywhere. In tests: `{"camera": "first"}` (headless).

```lua
command("hat", function()
  model.attach("head", { item = "minecraft:carved_pumpkin" })
  return player.camera() == "first" and "Hat on (F5 to see it)" or "Hat on"
end)
```
"##
    ),
    // ------------------------------------------------------------- world
    e!(
        "world.block",
        "world",
        r##"
`world.block(x, y, z)`

The id of the block at a position (`"minecraft:stone"`, `"minecraft:air"`), without its state properties. Coordinates are floored. In the game, places outside the loaded chunks read as air.

- `x`, `y`, `z` (numbers).
- Returns: a string, or nil outside a world.
- Where: anywhere.
- Errors: `bad argument #1 to 'block' (number expected, got nil)`.

Headless every block is `"minecraft:air"` unless a test step set it (`{"block": [x, y, z], "id": "minecraft:stone"}`).

```lua
command("below", function()
  local x, y, z = player.block()
  if not x then return "Not in a world" end
  return "Below you: " .. world.block(x, y - 1, z)
end)
```
"##
    ),
    e!(
        "world.time",
        "world",
        r##"
`world.time()`

The world's clock: the time of day (0 to 23999: 0 is sunrise at 6:00, 6000 noon, 12000 sunset, 13000 night, 18000 midnight), the game time in ticks, and the day number (1 on the first day).

- Returns: `time, game_time, day`, or nil outside a world.
- Where: anywhere. In tests: `{"time": 13000}` sets it (headless).

```lua
local time, _, day = world.time()
if time then
  local minutes = math.floor((time / 1000 + 6) % 24 * 60)
  chat.show(string.format("Day %d, %02d:%02d", day, minutes // 60, minutes % 60))
end
```
"##
    ),
    e!(
        "world.find",
        "world",
        r##"
`world.find(block[, radius])`

The nearest loaded blocks of a kind around the player's eyes, through Horizon's agent (`find-block`): it scans a cube of `radius` blocks each way, between y -64 and 319.

- `block` (string): a block id, `"minecraft:diamond_ore"` or `"diamond_ore"` (letters, digits, `_ : . / -`).
- `radius` (number, default 32): kept within 1 and 64.
- Returns: `{block = id, count = n, nearest = {{pos = {x, y, z}, distance = d}, ...}}`: `count` blocks found, the 16 nearest first (`pos[1]`, `pos[2]`, `pos[3]`).
- Where: not in `on("hud")`; in the game not in `on("world")` or a screen's draw. Scanning 129 x 129 x 129 blocks is slow: call it from a command or a timer (once a second), not every frame.
- Errors: `bad argument #1 to 'find' (a block id expected)`; `agent functions are not available while drawing the HUD`; `the agent is not available here`; headless always `Horizon's agent command 'find-block' needs the game: test this part live (the game running with the studio)`.

```lua
command("diamonds", function()
  local found = world.find("minecraft:diamond_ore", 24)
  local near = found.nearest[1]
  if not near then return "No diamonds within 24 blocks" end
  local p = near.pos
  return string.format("%d found, nearest at %d %d %d (%.0f blocks)", found.count, p[1], p[2], p[3], near.distance)
end)
```
"##
    ),
    e!(
        "world.entities",
        "world",
        r##"
`world.entities([radius[, type]])`

Mobs, dropped items and experience orbs near the player, nearest first, through Horizon's agent (`entities`).

- `radius` (number, default 16): kept within 1 and 128.
- `type` (string, optional): an entity type, `"zombie"` or `"minecraft:zombie"` (letters, digits, `_ : -`).
- Returns: `{entities = {...}}` (the list is the `entities` field). Each has `id` (for `model.attach`'s `target`, `player.part`), `type` (`"minecraft:zombie"`, `"minecraft:item"`, `"minecraft:experience_orb"`), `kind` (`"mob"`, `"item"`, `"orb"`), `pos = {x, y, z}` and `distance`; mobs also `health`, `baby`, `hurt_time` (villagers `villager = {type, profession, level}`), items `item = {item, count, ...}`, orbs `value`.
- Where: as `world.find` (not in `on("hud")`; in the game not in `on("world")` or a screen's draw; never headless).
- Errors: `bad argument #2 to 'entities' (an entity type expected)`; the agent errors of `world.find`.

```lua
command("mobs", function()
  local list = world.entities(32).entities
  local zombies = 0
  for _, e in ipairs(list) do
    if e.type == "minecraft:zombie" then zombies = zombies + 1 end
  end
  return #list .. " entities, " .. zombies .. " zombies"
end)
```
"##
    ),
    e!(
        "world.particle",
        "world",
        r##"
`world.particle(id, x, y, z[, dx, dy, dz[, count[, color]]])`

Spawns particles that only this player sees.

- `id` (string): a particle type, `"flame"`, `"minecraft:heart"`, `"note"`, `"smoke"`, `"happy_villager"`, `"end_rod"`, `"dust"` (takes a color)...; types that need a block or an item (`block`, `block_crumble`, `falling_dust`, `dust_pillar`, `block_marker`, `item`, `vibration`, `trail`) are refused in the game.
- `x`, `y`, `z` (numbers): where, in blocks.
- `dx`, `dy`, `dz` (numbers, default 0): the particles' speed.
- `count` (number, default 1): kept within 1 and 64; with more than one, each is placed up to 0.25 blocks from the point at random.
- `color` (a color, optional): for particles that take one (`dust`); alpha too.
- Returns: nothing.
- Where: not while drawing (`on("hud")`, `on("world")`, a screen's draw).
- Errors: `world.particle doesn't work while drawing: call it from tick, frame, a command or another event`; `too many particles this frame (512)` (all mods together); `unknown particle <id>` (game); headless `'<id>' is not a particle id (they look like minecraft:flame)` for malformed ids.

In tests: `expect_particle`.

```lua
on("tick", function()
  local x, y, z = player.pos()
  if x then world.particle("dust", x, y + 2.2, z, 0, 0, 0, 3, "#ff4040") end
end)
```
"##
    ),
    // ------------------------------------------------------------- hud
    e!(
        "hud.panel",
        "gfx",
        r##"
`hud.panel(corner, lines)`

A box of text lines in a corner of the HUD, stacked with the game's own boxes there. The studio draws it every frame until it is changed, cleared or the mod stops, so it can be set from anywhere (a tick, a command), not only in `on("hud")`. A mod has one panel; calling again replaces it. White text with a shadow on translucent black, lines 10 GUI pixels apart.

- `corner` (string): `"top-left"` (`"topleft"`, `"tl"`), `"top-right"` (`"tr"`), `"bottom-left"` (`"bl"`), `"bottom-right"` (`"br"`); nil clears the panel (`hud.panel(nil)` or `hud.panel()`).
- `lines`: a list of values (the first 16 show, each turned into text with `tostring` and cut to 256 bytes), or one value for a single line.
- Returns: nothing.
- Where: anywhere.
- Errors: `bad argument #1 to 'panel' ("top-left", "top-right", "bottom-left" or "bottom-right" expected)`.

In tests: the lines are in `expect_drawn`.

```lua
on("tick", function()
  local x, y, z = player.block()
  if not x then hud.panel(nil) return end
  hud.panel("top-left", { ("XYZ %d %d %d"):format(x, y, z), "Facing " .. player.facing() })
end)
```
"##
    ),
    e!(
        "hud.text",
        "gfx",
        r##"
`hud.text(x, y, text[, color[, shadow]])`

Draws text with the game's own font, in GUI pixels; `(x, y)` is its top left.

- `x`, `y` (numbers): GUI pixels from the top left.
- `text` (any value): turned into text with `tostring`.
- `color` (a color, default white): the alpha is ignored.
- `shadow` (boolean, default true): `false` draws no shadow.
- Returns: nothing.
- Where: only inside `on("hud")` or a screen's `draw`.
- Errors: `hud.text draws only inside on("hud", function(w, h) ... end) or a screen's draw`.

```lua
on("hud", function(w, h)
  local label = "Hello"
  hud.text((w - hud.width(label)) / 2, 8, label, "#ffd700")
end)
```
"##
    ),
    e!(
        "hud.rect",
        "gfx",
        r##"
`hud.rect(x, y, w, h[, color])`

Fills a rectangle in GUI pixels.

- `x`, `y`, `w`, `h` (numbers): GUI pixels.
- `color` (a color, default `0x78000000`, translucent black). A number without alpha (`0xff0000`) is invisible.
- Returns: nothing.
- Where: only inside `on("hud")` or a screen's `draw`.
- Errors: `hud.rect draws only inside on("hud", function(w, h) ... end) or a screen's draw`.

```lua
on("hud", function(w, h)
  hud.rect(w / 2 - 40, h - 70, 80, 6, "#a0000000")
  hud.rect(w / 2 - 40, h - 70, 80 * 0.6, 6, "#45c27a")
end)
```
"##
    ),
    e!(
        "hud.width",
        "gfx",
        r##"
`hud.width(text)`

The width of text in GUI pixels with the game's font (as `hud.text` draws it).

- `text` (any value): turned into text with `tostring`.
- Returns: a number. Headless: 6 per character.
- Where: anywhere.

```lua
on("hud", function(w, h)
  local t = "Right aligned"
  hud.text(w - hud.width(t) - 4, 4, t)
end)
```
"##
    ),
    e!(
        "hud.size",
        "gfx",
        r##"
`hud.size()`

The GUI's size in GUI pixels (the `hud` handler gets the same as `w, h`).

- Returns: `w, h`. Headless: 480, 270.
- Where: anywhere.

```lua
local w, h = hud.size()
print("GUI is " .. w .. " x " .. h)
```
"##
    ),
    // ------------------------------------------------------------- gfx
    e!(
        "gfx.size",
        "gfx",
        r##"
`gfx.size()`

The screen's size in dp, Horizon's logical pixels, which `gfx.*` draws in.

- Returns: `w, h`. Headless: 960, 540 (2 dp per GUI pixel).
- Where: anywhere.

```lua
on("hud", function()
  local w, h = gfx.size()
  gfx.text(w - 120, h - 30, "Bottom right", "#ffffff", 13)
end)
```
"##
    ),
    e!(
        "gfx.rect",
        "gfx",
        r##"
`gfx.rect(x, y, w, h[, color[, radius]])`

A filled rectangle with rounded corners, in dp.

- `x`, `y`, `w`, `h` (numbers): dp; nothing is drawn when `w` or `h` is 0 or less (or not finite).
- `color` (a color, default opaque white).
- `radius` (number, default 0): corner radius in dp, kept within half the smaller side (`radius = h / 2` makes a pill).
- Returns: nothing.
- Where: only inside `on("hud")` or a screen's `draw`.
- Errors: `gfx.rect draws only inside on("hud", function(w, h) ... end) or a screen's draw`.

```lua
on("hud", function()
  gfx.rect(20, 20, 160, 44, "#e016181f", 14)
  gfx.text(36, 32, "Rounded", "#ecedf3", 15, "medium")
end)
```
"##
    ),
    e!(
        "gfx.border",
        "gfx",
        r##"
`gfx.border(x, y, w, h[, color[, radius]])`

A 1 dp outline of a rounded rectangle, in dp. Arguments as `gfx.rect`.

- Returns: nothing.
- Where: only inside `on("hud")` or a screen's `draw`.
- Errors: `gfx.border draws only inside on("hud", function(w, h) ... end) or a screen's draw`.

```lua
on("hud", function()
  gfx.rect(20, 80, 120, 36, "#c0000000", 10)
  gfx.border(20, 80, 120, 36, "#9a86ff", 10)
end)
```
"##
    ),
    e!(
        "gfx.text",
        "gfx",
        r##"
`gfx.text(x, y, text[, color[, size[, font]]])`

Draws text in Horizon's fonts, in dp. Its line box is 1.45 x `size` tall and `y` is the top of it.

- `x`, `y` (numbers): dp.
- `text` (any value): turned into text with `tostring`, cut at 1024 bytes.
- `color` (a color, default opaque white).
- `size` (number, default 14): dp, kept within 6 and 96.
- `font` (string, default `"regular"`): `"regular"` (Geist), `"medium"` (Geist Medium; `"bold"` is the same), `"mono"` (Geist Mono).
- Returns: the width drawn, in dp.
- Where: only inside `on("hud")` or a screen's `draw`.
- Errors: `gfx.text draws only inside on("hud", function(w, h) ... end) or a screen's draw`; `bad argument #6 to 'text' ("regular", "medium" or "mono" expected)`.

```lua
on("hud", function()
  local size, cy = 16, 40
  local w = gfx.text(20, cy - size * 1.45 / 2, "Centred on y 40", "#ffffff", size, "medium")
  gfx.rect(20, cy + 14, w, 2, "#9a86ff")
end)
```
"##
    ),
    e!(
        "gfx.measure",
        "gfx",
        r##"
`gfx.measure(text[, size[, font]])`

The width `gfx.text` would draw, in dp, without drawing. A line is 1.45 x `size` tall.

- `text` (any value), `size` (default 14, 6 to 96), `font` (`"regular"`, `"medium"`/`"bold"`, `"mono"`).
- Returns: a number. Headless: characters x size x 0.55.
- Where: anywhere.
- Errors: `bad argument #3 to 'measure' ("regular", "medium" or "mono" expected)`.

```lua
on("hud", function()
  local sw = gfx.size()
  local t = "Centred"
  gfx.text((sw - gfx.measure(t, 20, "medium")) / 2, 12, t, "#ffffff", 20, "medium")
end)
```
"##
    ),
    e!(
        "gfx.mouse",
        "gfx",
        r##"
`gfx.mouse()`

The pointer while this mod's screen shows (`ui.open`).

- Returns: `x, y` in dp, `left` held, `right` held (booleans); nil when this mod's screen is not showing.
- Where: anywhere (most useful in a screen's `draw`).

```lua
command("pointer", function()
  ui.open({ canvas = true, draw = function()
    local x, y, left = gfx.mouse()
    if x then gfx.rect(x - 6, y - 6, 12, 12, left and "#f07a68" or "#ffffff", 6) end
  end })
end)
```
"##
    ),
    // ------------------------------------------------------------- draw
    e!(
        "draw.line",
        "world",
        r##"
`draw.line(x1, y1, z1, x2, y2, z2[, color[, width[, through]]])`

A line in the world for this frame: a flat ribbon that faces the camera, hidden behind blocks; or, with `through`, a line drawn over everything on the HUD.

- `x1, y1, z1`, `x2, y2, z2` (numbers): its ends, in blocks.
- `color` (a color, default opaque white).
- `width` (number, default 0.04): in blocks, kept within 0.002 and 4; through walls it is `width x 40` GUI pixels, 1 to 8.
- `through` (boolean, default false): true draws it through walls.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.line draws only inside on("world", function() ... end)`; `too much drawn in the world this frame` (16384 quads); `too much drawn through walls this frame` (4096).

```lua
on("world", function()
  local x, y, z = player.pos()
  if x then draw.line(x, y + 0.1, z, 0.5, 64, 0.5, "#9a86ff", 0.05, true) end -- a line to 0, 64, 0
end)
```
"##
    ),
    e!(
        "draw.box",
        "world",
        r##"
`draw.box(x1, y1, z1, x2, y2, z2[, color[, width[, through]]])`

The 12 edges of a box between two corners (in any order), for this frame.

- Corners in blocks; `color` (default opaque white); `width` (default 0.03 blocks, 0.002 to 4); `through` (default false) as `draw.line`.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: as `draw.line` (a box is 12 lines).

```lua
on("world", function()
  draw.box(0, 64, 0, 3, 67, 3, "#f2c48d", 0.05) -- a 3 x 3 x 3 area
end)
```
"##
    ),
    e!(
        "draw.block",
        "world",
        r##"
`draw.block(x, y, z[, color[, width[, through]]])`

The outline of one block (the position is floored), slightly larger than the block so it is not hidden by it.

- `x`, `y`, `z` (numbers); `color` (default opaque white); `width` (default 0.03 blocks); `through` (default false) as `draw.line`.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: as `draw.line`.

```lua
on("world", function()
  local x, y, z = player.block()
  if x then draw.block(x, y - 1, z, "#9a86ff", 0.04) end -- the block under the player
end)
```
"##
    ),
    e!(
        "draw.fill",
        "world",
        r##"
`draw.fill(x1, y1, z1, x2, y2, z2[, color])`

A solid box between two corners (6 faces, seen from both sides), for this frame. Use alpha for glass-like boxes.

- Corners in blocks; `color` (default opaque white).
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.fill draws only inside on("world", function() ... end)`; `too much drawn in the world this frame`.

```lua
on("world", function()
  draw.fill(0, 64, 0, 1, 65, 1, "#4045c27a")
end)
```
"##
    ),
    e!(
        "draw.beam",
        "world",
        r##"
`draw.beam(x, z[, color[, width[, y1[, y2]]]])`

A vertical beam like a beacon's or a waypoint's: four sides of a tall box, open at the ends, centred on `x, z`.

- `x`, `z` (numbers): the centre, in blocks (use `x + 0.5` for a block's centre).
- `color` (a color, default opaque white; alpha works).
- `width` (number, default 0.25 blocks, kept within 0.002 and 4).
- `y1`, `y2` (numbers, default -64 and 320): where it starts and ends.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: as `draw.fill`.

```lua
on("world", function()
  draw.beam(100.5, -40.5, "#a0ff8040", 0.4)
end)
```
"##
    ),
    e!(
        "draw.text",
        "world",
        r##"
`draw.text(x, y, z, text[, color[, distance]])`

A label at a place in the world, drawn on the HUD over everything (seen through walls), centred on the point, on a translucent black background, with the game's font.

- `x`, `y`, `z` (numbers): the point, in blocks.
- `text` (string or number): at most 128 characters are kept.
- `color` (a color, default white): the alpha is ignored.
- `distance` (boolean, default false): true adds how far the point is from the camera, `" (12 m)"`.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.text draws only inside on("world", function() ... end)`; `too much drawn through walls this frame` (4096).

In tests: the label is in `expect_drawn` when the point is in view.

```lua
on("world", function()
  draw.text(0.5, 70, 0.5, "Spawn", "#ffd700", true)
end)
```
"##
    ),
    e!(
        "draw.item",
        "models",
        r##"
`draw.item(id, x, y, z[, opts])`

An item (or block) as its 3D model at a place, for this frame, as the game draws it in a display context.

- `id` (string): an item id; give the namespace, `"minecraft:diamond_sword"`.
- `x`, `y`, `z` (numbers): where its model's origin goes, in blocks.
- `opts` (table, optional): `yaw`, `pitch`, `roll` (degrees, default 0), `scale` (a number, default 1), `display` (`"fixed"`, the default, `"ground"`, `"gui"`, `"head"`, `"hand"` or `"left_hand"`), `glow = true` (full bright; otherwise lit by the light at the point).
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.item draws only inside on("world", function() ... end)`; `bad argument #5 to 'item' (display is fixed, ground, gui, head, hand or left_hand)`; `draw.item: no item '<id>'` (game); `too much drawn in the world this frame` (an item counts as 1).

In tests: `expect_item`.

```lua
local spin = 0
on("frame", function(dt) spin = (spin + dt * 90) % 360 end)
on("world", function()
  draw.item("minecraft:diamond", 0.5, 65.5, 0.5, { yaw = spin, scale = 1.5, glow = true })
end)
```
"##
    ),
    // ------------------------------------------------------------- meshes
    e!(
        "draw.mesh",
        "meshes",
        r##"
`draw.mesh(id, x, y, z[, opts])`

Draws a mesh of this mod's at a place, for this frame, textured and lit by the world there. Its origin goes to `x`, `y`, `z` and it faces +z at yaw 0 (see the topic `meshes`).

- `id` (number): a mesh of this mod's.
- `x`, `y`, `z` (numbers): where its origin goes, in blocks.
- `opts` (table, optional):
  - `yaw`, `pitch`, `roll` (degrees, default 0): `yaw` as `player.look()` gives it (90 faces west, as the player does), positive `pitch` tilts its front down.
  - `scale`: a number or `{x, y, z}` (default 1; the mesh is in pixels, 16 to a block).
  - `color`: tints every triangle (multiplied with its colour and texture; default white).
  - `texture` (number): a texture of this mod's for every triangle, instead of the mesh's own.
  - `glow = true`: full bright instead of lit by the world's light at `x`, `y`, `z`.
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.mesh draws only inside on("world", function() ... end)`; `draw.mesh: this mod has no mesh 3`; `draw.mesh: this mod has no texture 5`; `bad argument #5 to 'mesh' (scale is a number or {x, y, z})`; `too much drawn in the world this frame` (each triangle counts 1 of 16384).

In tests: `expect_mesh`.

```lua
local cube = mesh.new({ boxes = { { from = { -8, 0, -8 }, to = { 8, 16, 8 }, color = "#c0392b" } } })
local spin = 0
on("frame", function(dt) spin = (spin + dt * 45) % 360 end)
on("world", function()
  draw.mesh(cube, 0.5, 70, 0.5, { yaw = spin, scale = 0.5, glow = true })
end)
```
"##
    ),
    e!(
        "draw.triangle",
        "meshes",
        r##"
`draw.triangle(x1, y1, z1, x2, y2, z2, x3, y3, z3[, color])`

A filled triangle in the world for this frame, full bright and seen from both sides.

- The nine numbers: its corners, in blocks.
- `color` (default opaque white): any colour form (`"#rrggbb"`, `0xAARRGGBB`...).
- Returns: nothing.
- Where: only inside `on("world")`.
- Errors: `draw.triangle draws only inside on("world", function() ... end)`; `too much drawn in the world this frame` (it counts 1 of 16384).

```lua
on("world", function()
  draw.triangle(0, 70, 0, 2, 70, 0, 1, 72, 0, "#80ffd700")
end)
```
"##
    ),
    e!(
        "texture.load",
        "meshes",
        r##"
`texture.load(file)`

Reads a PNG file of this folder mod as a texture.

- `file` (string): a path inside the mod's folder, `"skin.png"`, `"textures/cape.png"` (no `..`, at most 16 MB).
- Returns: the texture's id, its width and height.
- Where: anywhere; only in a folder mod (`mods/<id>/main.lua`).
- Errors: `texture.load reads files of a folder mod (mods/<id>/main.lua); a one-file mod can use texture.decode or texture.create`; `texture.load: 'x' is not a path inside the mod's folder`; `texture.load: no file 'x.png' in the mod's folder`; `texture.load: 'x.png': not a PNG image (...)`; `texture.load: a texture is 1 to 512 pixels a side (this one 600x600)`; `texture.load: the mods' textures are at most 1048576 pixels in all (a 1024x1024); texture.free drops one`; `texture.load: a mod has at most 256 textures (texture.free drops one)`.

```lua
local cape, w, h = texture.load("textures/cape.png")
print("cape " .. w .. "x" .. h)
```
"##
    ),
    e!(
        "texture.decode",
        "meshes",
        r##"
`texture.decode(data)`

A texture from a PNG given in the code: base64 text (standard or URL-safe, padding and line breaks optional), a `data:image/png;base64,...` URL, or the PNG's raw bytes. How an agent ships an image: it writes the base64 into the Lua file.

- `data` (string).
- Returns: the texture's id, its width and height.
- Where: anywhere.
- Errors: `texture.decode: not base64 (it has '*')`; `texture.decode: not a PNG image (...)`; the size and count limits of `texture.load`.

```lua
-- a 1x1 white PNG
local white = texture.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGP4DwQACfsD/fteaysAAAAASUVORK5CYII=")
```
"##
    ),
    e!(
        "texture.create",
        "meshes",
        r##"
`texture.create(w, h[, color])`

A new texture of `w` x `h` pixels, every one `color` (transparent without one), to paint with `texture.set`, `texture.fill` and `texture.paint`.

- `w`, `h` (numbers): 1 to 512.
- `color`: any colour form.
- Returns: the texture's id.
- Where: anywhere.
- Errors: `texture.create: a texture is 1 to 512 pixels a side (this one 0x4)`; the count and total limits of `texture.load`.

```lua
local t = texture.create(16, 16, "#2ecc71")
texture.fill(t, 0, 0, 16, 4, "#27ae60")
```
"##
    ),
    e!(
        "texture.set",
        "meshes",
        r##"
`texture.set(id, x, y, color)`

Sets one pixel. Outside the texture nothing happens.

- `id` (number): a texture of this mod's.
- `x`, `y` (numbers): from 0 at the top left (rounded down).
- `color`: any colour form; `"#00000000"` is transparent.
- Returns: nothing.
- Where: anywhere. A changed texture is uploaded again: the next frame still shows the old pixels.
- Errors: `texture.set: this mod has no texture 4`.

```lua
local t = texture.create(8, 8)
for i = 0, 7 do texture.set(t, i, i, "#ffffff") end
```
"##
    ),
    e!(
        "texture.get",
        "meshes",
        r##"
`texture.get(id, x, y)`

A pixel's colour.

- `id` (number), `x`, `y` (numbers from 0).
- Returns: `0xAARRGGBB` as a number, or nil outside the texture.
- Where: anywhere.
- Errors: `texture.get: this mod has no texture 4`.

```lua
local t = texture.create(2, 2, "#ff0000")
print(string.format("%x", texture.get(t, 1, 1)))   -- ffff0000
```
"##
    ),
    e!(
        "texture.fill",
        "meshes",
        r##"
`texture.fill(id, x, y, w, h, color)`

Fills a rectangle of pixels (the part inside the texture).

- `id` (number), `x`, `y` (its top left, from 0), `w`, `h` (its size), `color`.
- Returns: nothing.
- Where: anywhere.
- Errors: `texture.fill: this mod has no texture 4`.

```lua
local flag = texture.create(9, 6, "#ffffff")
texture.fill(flag, 0, 0, 9, 3, "#0055a4")
```
"##
    ),
    e!(
        "texture.paint",
        "meshes",
        r##"
`texture.paint(id, rows, palette[, x, y])`

Pixel art as text: each character of each row is looked up in `palette` and that pixel set to its colour. A character the palette lacks (or maps to `false`) leaves its pixel as it was.

- `id` (number).
- `rows` (list of strings): the top row first.
- `palette` (table): one-character keys to colours, `{x = "#c0392b", o = "#ffffff", ["."] = false}`.
- `x`, `y` (numbers, default 0): where the first row's first character goes.
- Returns: nothing.
- Where: anywhere.
- Errors: `texture.paint: this mod has no texture 4`; `bad argument #3 to 'color' (...)` for a palette colour it can't read.

```lua
local heart = texture.create(7, 6)
texture.paint(heart, {
  ".xx.xx.",
  "xxxxxxx",
  "xxxxxxx",
  ".xxxxx.",
  "..xxx..",
  "...x...",
}, { x = "#e0115f" })
```
"##
    ),
    e!(
        "texture.size",
        "meshes",
        r##"
`texture.size(id)`

- Returns: the texture's width and height.
- Where: anywhere.
- Errors: `texture.size: this mod has no texture 4`.

```lua
local t = texture.create(32, 16)
local w, h = texture.size(t)
```
"##
    ),
    e!(
        "texture.free",
        "meshes",
        r##"
`texture.free(id)`

Drops a texture (and its pixels' share of the 1048576 all mods may keep). Meshes that used it draw those triangles in plain colour.

- Returns: true if there was one.
- Where: anywhere.

```lua
local t = texture.create(4, 4)
print(texture.free(t), texture.free(t))   -- true false
```
"##
    ),
    e!(
        "mesh.load",
        "meshes",
        r##"
`mesh.load(file[, opts])`

Reads a model file of this folder mod as a mesh, by its extension: a Wavefront `.obj` or a Blockbench `.bbmodel`.

- `file` (string): a path inside the mod's folder (no `..`, at most 16 MB).
- `opts` (table, optional):
  - `scale` (.obj, default 16): pixels per unit of the file (16: the file is in blocks).
  - `texture` (number): a texture of this mod's for the .obj's triangles, and for .bbmodel faces whose texture is not embedded.
  - `color`: tints every triangle.
  - `flip = false` (.bbmodel): keep Blockbench's north-facing front instead of turning it to face +z.
- .obj: `v`, `vt` (texture coordinates, counted up from the bottom as the format does) and `f` lines (polygons, negative indices too); other lines are skipped.
- .bbmodel: cubes (with inflate, rotation about their origin, per-face UV or box UV, face rotation), mesh elements, outliner groups (their pivots and turns), hidden elements skipped; textures embedded in the file become this mod's (see `mesh.info`). A face with no texture is left out.
- Returns: the mesh's id.
- Where: anywhere; only in a folder mod.
- Errors: `mesh.load reads files of a folder mod (mods/<id>/main.lua); a one-file mod can use mesh.parse or mesh.new`; `mesh.load: no file 'm.obj' in the mod's folder`; `mesh.load: 'stl' is not a format it reads: obj or bbmodel`; `mesh.load: line 12: index 9 is not one of the 8 so far`; `mesh.load: the .obj has no faces (f lines)`; `mesh.load: not a .bbmodel (JSON: ...)`; `mesh.load: elements[3]: a cube needs from and to`; `mesh.load: a mesh has at most 65536 triangles`; `mesh.load: a mod has at most 256 meshes (mesh.free drops one)`.

```lua
local statue = mesh.load("models/statue.bbmodel")
local rock = mesh.load("models/rock.obj", { texture = texture.load("textures/rock.png") })
```
"##
    ),
    e!(
        "mesh.parse",
        "meshes",
        r##"
`mesh.parse(text, format[, opts])`

A mesh from a model file's text, for a one-file mod or a model an agent writes into the code: `format` is `"obj"` or `"bbmodel"`, `opts` as `mesh.load`'s.

- Returns: the mesh's id.
- Where: anywhere.
- Errors: as `mesh.load`'s, named `mesh.parse`.

```lua
local pyramid = mesh.parse([[
v -1 0 -1
v 1 0 -1
v 1 0 1
v -1 0 1
v 0 1.5 0
f 1 2 5
f 2 3 5
f 3 4 5
f 4 1 5
f 4 3 2 1
]], "obj", { color = "#f4c430" })
```
"##
    ),
    e!(
        "mesh.new",
        "meshes",
        r##"
`mesh.new(spec)`

A mesh built in Lua, in pixels (16 to a block), from corners and faces, from boxes, or both.

- `spec` (table):
  - `vertices`: a list of `{x, y, z}`.
  - `uvs`: a list of `{u, v}` (0 to 1 from the texture's top left); a face corner without its own takes its vertex's.
  - `faces`: a list of faces, each `{a, b, c, ...}` (vertex numbers from 1; more than three corners are split into triangles around the first) or `{v = {a, b, c, d}, uv = {1, 2, 3, 4}, color = ...}`.
  - `boxes`: a list of `{from = {x, y, z}, to = {x, y, z}, color = ..., uv = {u, v}}` textured with Minecraft's box UV (a skin's layout at `u`, `v` in texture pixels: for a box `w` x `h` x `d` the right side, front, left side and back side by side below the top and bottom; the front faces +z), or `faces = {north = {u1, v1, u2, v2}, south = ..., east = ..., west = ..., up = ..., down = ...}` (texture pixels; faces left out are not drawn). Without either, a box is plain colour.
  - `texture` (number): a texture of this mod's for all of it.
  - `color`: the colour of faces and boxes without their own (default white; it tints the texture).
- Returns: the mesh's id.
- Where: anywhere.
- Errors: `mesh.new: give faces (with vertices) or boxes`; `mesh.new: vertices[2] is {x, y, z}`; `mesh.new: faces[1] is {a, b, c} or {v = {...}, uv = {...}} (vertex numbers from 1)`; `mesh.new: boxes[1]: needs from and to, each {x, y, z} in pixels`; `mesh.new: this mod has no texture 3`; `mesh.new: a mod has at most 256 meshes (mesh.free drops one)`.

```lua
local skin = texture.create(64, 64, "#888888")
local steve_ish = mesh.new({
  texture = skin,
  boxes = {
    { from = { -4, 24, -4 }, to = { 4, 32, 4 }, uv = { 0, 0 } },      -- head
    { from = { -4, 12, -2 }, to = { 4, 24, 2 }, uv = { 16, 16 } },    -- body
  },
})
local sail = mesh.new({
  vertices = { { 0, 0, 0 }, { 0, 32, 0 }, { 0, 0, 24 } },
  faces = { { 1, 2, 3 } },
  color = "#ecf0f1",
})
```
"##
    ),
    e!(
        "mesh.info",
        "meshes",
        r##"
`mesh.info(id)`

- Returns: `{triangles, min = {x, y, z}, max = {x, y, z}, textures = {ids}}` (its size in pixels, and the textures it uses: for a .bbmodel, the ones that came with it), or nil when this mod has no such mesh.
- Where: anywhere.

```lua
local m = mesh.new({ boxes = { { from = { 0, 0, 0 }, to = { 4, 8, 2 } } } })
local i = mesh.info(m)
print(i.triangles, i.max[2])   -- 12 8
```
"##
    ),
    e!(
        "mesh.free",
        "meshes",
        r##"
`mesh.free(id)`

Drops a mesh; attachments that used it draw nothing more of it.

- Returns: true if there was one.
- Where: anywhere.

```lua
local m = mesh.new({ boxes = { { from = { 0, 0, 0 }, to = { 1, 1, 1 } } } })
mesh.free(m)
```
"##
    ),
    // ------------------------------------------------------------- sound
    e!(
        "sound.play",
        "world",
        r##"
`sound.play(id[, volume[, pitch]])`

Plays a sound for the player (not from a place).

- `id` (string): a sound event, `"minecraft:entity.experience_orb.pickup"`, `"block.note_block.bell"`, `"ui.button.click"`; `minecraft:` is added when there is no `:`.
- `volume` (number, default 1): kept within 0 and 4.
- `pitch` (number, default 1): kept within 0.5 and 2.
- Returns: nothing.
- Where: not in `on("hud")` or `on("world")`. In a screen's `draw` it is allowed; in the game it plays at the start of the next frame.
- Errors: `sound.play doesn't work while drawing: call it from tick, frame, a command or another event`; `too many sounds this frame (32)`; headless `'<id>' is not a sound id (they look like minecraft:block.note_block.bell)` for malformed ids.

In tests: `expect_sound`.

```lua
command("ding", function()
  sound.play("block.note_block.bell", 1, 1.5)
end)
```
"##
    ),
    e!(
        "sound.at",
        "world",
        r##"
`sound.at(id, x, y, z[, volume[, pitch]])`

Plays a sound from a place in the world, so it sounds from there (only this player hears it).

- `id`, `volume`, `pitch` as `sound.play`; `x`, `y`, `z` (numbers): where, in blocks.
- Returns: nothing.
- Where and errors: as `sound.play`.

```lua
command("bell", function()
  local x, y, z = player.pos()
  if x then sound.at("block.bell.use", x + 5, y, z) end
end)
```
"##
    ),
    // ------------------------------------------------------------- ui
    e!(
        "ui.open",
        "screens",
        r##"
`ui.open(spec)`

Shows a screen: a centred panel of widgets, or a canvas the mod draws itself. It replaces a script screen already showing, and takes effect at the end of the frame (`ui.is_open()` is true from the next frame). See the topic `screens`.

`spec` fields (the table is read again every frame, so changing it changes the screen):

- `title` (string): the panel's heading.
- `width` (number, default 340): the panel's width in dp (200 to the screen's width - 32).
- `widgets` (list): widget tables, top to bottom, at most 64: `{type = "label"|"button"|"toggle"|"slider"|"input"|"space", ...}` (see `widget:label` and the others).
- `canvas` (boolean): true draws no panel and no widgets; `draw` and every coordinate are in dp.
- `backdrop` (boolean): false draws no dimmed backdrop over the game.
- `draw(w, h)`: every frame, after the widgets; `w`, `h` in GUI pixels (dp on a canvas). `hud.*` and `gfx.*` draw here; `world.particle` and `draw.*` do not.
- `click(x, y, button)`: a press not on a widget (and any right or middle press); `button` is `"left"`, `"right"` or `"middle"`.
- `release(x, y, button)`: a button released.
- `move(x, y)`: the pointer moved.
- `wheel(steps)`: the wheel turned (positive up), unless over a slider.
- `key(key, down, mods)`: a key, while no input has focus (`mods` is `{ctrl, shift, alt}`): `"enter"`, `"backspace"`, `"delete"`, `"tab"`, `"left"`, `"right"`, `"up"`, `"down"`, `"home"`, `"end"`, `"pageup"`, `"pagedown"`, `"a"`..`"z"`, `"0"`..`"9"`, `"/"`, `" "`, `` "`" ``, `"other"`; `"escape"` and `"toggle"` only on release (pressing them closes the screen).
- `escape()`: Escape pressed while no input has focus; returning true keeps the screen open, else it closes.
- `text(t)`: typed text, while no input has focus.
- `close()`: the player closed the screen (Escape or F8); not called by `ui.close()`.

`click`, `release` and `move` get GUI pixels on a panel screen and dp on a canvas. Return values of callbacks are ignored, but for `escape`'s. An error in any callback stops the mod and closes the screen (`screen: ...`).

- Returns: nothing.
- Where: anywhere (usually a command or a key handler). In the game it needs a world: it opens over the studio, or through the game's chat screen while playing; otherwise the request is dropped.
- Errors: `bad argument #1 to 'open' (table expected, got nil)`.

In tests: `{"click": "label"}`, `{"type": "text"}`, `expect_screen`, `expect_drawn` (headless).

```lua
local count = 0
local screen
screen = {
  title = "Counter",
  widgets = {
    { type = "label", text = "Clicked 0 times" },
    { type = "button", text = "Click me", primary = true, click = function()
        count = count + 1
        screen.widgets[1].text = "Clicked " .. count .. " times"
      end },
    { type = "button", text = "Close", click = function() ui.close() end },
  },
  close = function() chat.show("Closed at " .. count) end,
}
command("counter", function() ui.open(screen) end)
```
"##
    ),
    e!(
        "ui.close",
        "screens",
        r##"
`ui.close()`

Closes this mod's screen at once (or cancels one asked for this frame). The screen's `close` callback is not called; call it yourself if it saves something. Another mod's screen is left alone.

- Returns: nothing.
- Where: anywhere, also inside the screen's own callbacks.

```lua
local done = { type = "button", text = "Done", click = function() ui.close() end }
command("quick", function() ui.open({ title = "Quick", widgets = { done } }) end)
```
"##
    ),
    e!(
        "ui.is_open",
        "screens",
        r##"
`ui.is_open()`

Whether this mod's screen is showing. False on the frame `ui.open` was called (the screen shows from the next frame), and false when another mod's screen shows.

- Returns: a boolean.
- Where: anywhere.

```lua
on("key", function(key, down)
  if key == "key.keyboard.m" and down and not ui.is_open() then
    ui.open({ title = "Menu", widgets = { { type = "label", text = "Hi" } } })
  end
end)
```
"##
    ),
    // ------------------------------------------------------------- model
    e!(
        "model.attach",
        "models",
        r##"
`model.attach(part, spec)`

Attaches a model to a body part of the player (or of another entity) that follows it as it moves and animates, drawn with the world every frame until removed or the mod reloads. See the topic `models` for parts, axes and units.

- `part` (string): `root`, `head`, `body`, `right_arm`, `left_arm`, `right_leg`, `left_leg`, `hat`, `jacket`, `right_sleeve`, `left_sleeve`, `right_pants`, `left_pants`; for another entity, a part name of its model.
- `spec` (table), at least one of `item`, `boxes` and `mesh`:
  - `item` (string): an item or block id (`"minecraft:carved_pumpkin"`; `minecraft:` added when missing).
  - `boxes` (list, at most 256): `{from = {x, y, z}, to = {x, y, z}, color = ...}` in model pixels from the part's pivot, x to its right, y up, z forward (`color` default opaque white).
  - `mesh` (number): a mesh of this mod's (`mesh.new`, `mesh.load`, `mesh.parse`), textured; its x, y and z are the part's left, up and forward in pixels (see the topic `meshes`); `false` removes it.
  - `display`: how the item shows: `"head"`, `"hand"`, `"left_hand"`, `"fixed"`, `"ground"`, `"gui"` (default: `"head"` on head and hat, `"hand"` on the right arm, `"left_hand"` on the left arm, else `"fixed"`).
  - `offset = {right, up, forward}` in pixels; `rotation = {pitch, yaw, roll}` in degrees; `scale` a number or `{x, y, z}` (each number at most 4096 either way; lists or `{x =, y =, z =}`).
  - `glow = true`: full bright instead of lit by the world.
  - `first_person = true`: also drawn in first person (the player's own body is not).
  - `visible = false`: kept but not drawn.
  - `target`: `"player"` (default) or an entity id from `world.entities`.
- Returns: the model's id (a number), for `model.set` and `model.remove`.
- Where: anywhere.
- Errors: `bad argument #1 to 'attach' (a body part expected: root, head, body, ...)`; `model.attach: give an item, boxes or a mesh`; `model.attach: this mod has no mesh 7`; `model.attach: mesh is a mesh id (mesh.new, mesh.load...) or false for none`; `model.attach: item is an item id such as "minecraft:diamond_sword"`; `model.attach: display is one of head, hand, left_hand, fixed, ground, gui`; `model.attach: boxes[1] needs from and to, each {x, y, z} in pixels`; `model.attach: at most 256 boxes`; `model.attach: offset is {x, y, z}`; `model.attach: scale is out of range`; `model.attach: glow is true or false`; `model.attach: target is "player" or an entity id (world.entities gives them)`; `a mod attaches at most 64 models`.

In tests: `expect_item` (items drawn).

```lua
local crown = model.attach("head", {
  boxes = {
    { from = { -4.5, 8, -4.5 }, to = { 4.5, 9.5, 4.5 }, color = "#f4c430" },
    { from = { -0.75, 9.5, -4.6 }, to = { 0.75, 11.5, -4.4 }, color = "#e0115f" },
  },
})
local sword = model.attach("right_arm", { item = "minecraft:diamond_sword" })
local shield = model.attach("left_arm", { item = "minecraft:shield", rotation = { 0, 0, 10 } })
```
"##
    ),
    e!(
        "model.set",
        "models",
        r##"
`model.set(id, spec)`

Changes one of this mod's attached models: only the fields `spec` gives change (the fields of `model.attach`, but not the part). `item = false` and `boxes = false` remove the item or the boxes. Cheap enough to call every tick for animation.

- `id` (number): from `model.attach`.
- `spec` (table).
- Returns: nothing.
- Where: anywhere.
- Errors: `model.set: this mod has no model 7`; the field errors of `model.attach` (with `model.set:` in front).

```lua
local orb = model.attach("head", { item = "minecraft:amethyst_shard", display = "fixed", scale = 0.5, glow = true })
local angle = 0
on("tick", function()
  angle = angle + 9
  local a = math.rad(angle)
  model.set(orb, { offset = { math.cos(a) * 9, 6, math.sin(a) * 9 }, rotation = { 0, angle * 2, 0 } })
end)
```
"##
    ),
    e!(
        "model.remove",
        "models",
        r##"
`model.remove(id)`

Removes one of this mod's attached models.

- `id` (number).
- Returns: true if this mod had it, false otherwise.
- Where: anywhere.

```lua
local hat = model.attach("head", { item = "minecraft:carved_pumpkin" })
command("hatoff", function() return model.remove(hat) and "Hat off" or "No hat" end)
```
"##
    ),
    e!(
        "model.clear",
        "models",
        r##"
`model.clear()`

Removes every model this mod attached. (A reload does it too.)

- Returns: nothing.
- Where: anywhere.

```lua
command("undress", function() model.clear() return "All off" end)
```
"##
    ),
    e!(
        "model.list",
        "models",
        r##"
`model.list()`

This mod's attached models: a list of `{id, part, item, boxes, target}`, where `item` is the item id (nil when none), `boxes` the number of boxes, and `target` `"player"` or an entity id.

- Returns: a table (a list).
- Where: anywhere.

```lua
for _, m in ipairs(model.list()) do
  print(m.id, m.part, m.item or "-", m.boxes, m.target)
end
```
"##
    ),
    // ------------------------------------------------------------- keys, view
    e!(
        "keys.down",
        "input",
        r##"
`keys.down(key)`

Whether a key or mouse button is held now, from the key events the studio has seen (those `on("key")` gets).

- `key` (string): a key name, `"key.keyboard.r"`, `"key.keyboard.left.shift"`, `"key.mouse.left"` (see the topic `input`).
- Returns: a boolean.
- Where: anywhere.

```lua
on("tick", function()
  if keys.down("key.keyboard.left.alt") then hud.panel("top-left", { "Alt held" }) else hud.panel(nil) end
end)
```
"##
    ),
    e!(
        "keys.mapping",
        "input",
        r##"
`keys.mapping(name)`

A vanilla key mapping: whether it is held, and the label of the key it is bound to, as Controls shows it. In Horizon, the movement mappings' held state follows their default keys (W, A, S, D, Space, Left Shift, Left Control), and `key.attack` and `key.use` the mouse buttons.

- `name` (string): `"key.forward"`, `"key.left"`, `"key.back"`, `"key.right"`, `"key.jump"`, `"key.sneak"`, `"key.sprint"`, `"key.attack"`, `"key.use"`.
- Returns: `held` (boolean), `label` (string).
- Where: anywhere.
- Errors: `bad argument #1 to 'mapping' (one of key.forward, key.left, key.back, key.right, key.jump, key.sneak, key.sprint, key.attack, key.use)`.

Headless any `key.*` name works: held after a test step `{"key": "key.jump", "down": true}`, its label the name without `key.`.

```lua
command("jumpkey", function()
  local held, label = keys.mapping("key.jump")
  return "Jump is on " .. label .. (held and " (held)" or "")
end)
```
"##
    ),
    e!(
        "view.zoom",
        "api",
        r##"
`view.zoom(factor)`

Zooms the view while playing, until changed or the mod stops. When several mods zoom, the strongest wins; the spyglass's own view wins over all.

- `factor` (number, optional): more than 1 zooms in (at most 10; more is 10); 1, less, or nil turns this mod's zoom off.
- Returns: nothing.
- Where: anywhere.

```lua
on("key", function(key, down)
  if key == "key.keyboard.c" and game.screen() == "playing" then
    view.zoom(down and 4 or 1)
  end
end)
```
"##
    ),
    // ------------------------------------------------------------- agent
    e!(
        "agent.query",
        "agent",
        r##"
`agent.query(cmd)`

Runs one of Horizon's agent reads and returns its reply at once, as a table (JSON objects and arrays become tables; null becomes nil). Commands: see the topic `agent` (`state`, `looking-at`, `inventory`, `block X Y Z`, `blocks ...`, `light ...`, `find-block BLOCK [R]`, `entities [R] [TYPE]`, `screen`, `messages [N]`).

- `cmd` (string): the command line.
- Returns: the reply table. An action given to `query` still starts, but returns nil and `"this agent command is an action: use agent.run"`.
- Where: not in `on("hud")`; in the game not in `on("world")` or a screen's draw. Never headless.
- Errors: the reply's error (e.g. a usage error); `'<name>' is not an agent command scripts can use`; `agent functions are not available while drawing the HUD`; `the agent is not available here`; headless `Horizon's agent command '<name>' needs the game: test this part live (the game running with the studio)`.

```lua
command("look", function()
  local t = agent.query("looking-at")
  if t.kind == "block" then return "Looking at " .. t.block.block end
  if t.kind == "entity" then return "Looking at " .. t.entity.type end
  return "Looking at nothing"
end)
```
"##
    ),
    e!(
        "agent.run",
        "agent",
        r##"
`agent.run(cmd[, fn])`

Starts one of Horizon's agent commands, usually an action (`goto X Y Z`, `break X Y Z`, `place X Y Z`, `look-at X Y Z`, `select 3`, `jump`, `move forward 2`...; see the topic `agent`). When it finishes, `fn(result)` runs, or `fn(nil, error)` (a later frame; for instant commands the next frame). One action runs at a time.

- `cmd` (string): the command line.
- `fn` (function, optional): the callback; an error in it stops the mod (`agent callback: ...`).
- Returns: nothing.
- Where: as `agent.query`. At most 64 waiting per mod.
- Errors (raised at once): `'<name>' is not an agent command scripts can use`; `busy with goto; wait for it or send stop`; `too many agent actions waiting (64)`; `bad argument #2 to 'run' (function expected, got number)`; the agent errors of `agent.query`.

```lua
command("mineit", function()
  local t = agent.query("looking-at")
  if t.kind ~= "block" then return "Look at a block" end
  local p = t.block.pos
  agent.run(("break %d %d %d"):format(p[1], p[2], p[3]), function(result, err)
    chat.show(err and ("Failed: " .. err) or "Mined it")
  end)
end)
```
"##
    ),
    e!(
        "agent.stop",
        "agent",
        r##"
`agent.stop()`

Cancels the running agent action (`goto`, `break`...) and releases every key (Horizon's `stop` command).

- Returns: nothing.
- Where: as `agent.query`.

```lua
on("unload", function() agent.stop() end)
```
"##
    ),
    // ------------------------------------------------------------- storage, mod, json
    e!(
        "storage.get",
        "storage",
        r##"
`storage.get(key)`

A value this mod stored with `storage.set`, read from disk (a fresh copy each call).

- `key` (string): 1 to 64 letters, digits, `_`, `-` or `.`, not starting with `.`.
- Returns: the value, or nil when nothing is stored.
- Where: anywhere.
- Errors: `storage keys are 1 to 64 letters, digits, '_', '-' or '.'`; `stored value 'k' is damaged: ...` (the file was edited by hand into something that is not a Lua value).

```lua
local count = storage.get("count") or 0
count = count + 1
storage.set("count", count)
print("Loaded " .. count .. " times")
```
"##
    ),
    e!(
        "storage.set",
        "storage",
        r##"
`storage.set(key, value)`

Keeps a value on disk for this mod, across reloads and game restarts (`<studio folder>/storage/<mod id>/<key>.lua`). nil deletes the key.

- `key` (string): as `storage.get`.
- `value`: nil, a boolean, a number, a string, or a table of these (keys and values), nested at most 16 deep.
- Returns: nothing.
- Where: anywhere. Called from a library's function, it stores under the calling mod.
- Errors: `storage keys are 1 to 64 letters, digits, '_', '-' or '.'`; `functions cannot be stored`; `tables nested too deep to store (or a cycle)`; `value too large to store` (a table over 256 KiB); `this mod stores too many keys (256)`; `could not store 'k': <reason>`.

```lua
local notes = storage.get("notes") or {}
command("note", function(rest)
  notes[#notes + 1] = { text = rest, at = os.time() }
  storage.set("notes", notes)
  return "Noted (" .. #notes .. ")"
end)
```
"##
    ),
    e!(
        "mod.info",
        "api",
        r##"
`mod.info()`

This mod as the Library shows it: `{id, name, version, author, reloads}` (`name`, `version` and `author` from the header; `reloads` how many times it reloaded after a change). Inside a library's function it describes the calling mod; in the console `id` is `"console"`.

- Returns: a table.
- Where: anywhere.

```lua
local me = mod.info()
print(me.name .. " " .. me.version .. " by " .. me.author .. " (" .. me.id .. ")")
```
"##
    ),
    e!(
        "json.encode",
        "api",
        r##"
`json.encode(value[, pretty])`

A value as JSON text. Tables whose keys are exactly 1..n (n at least 1) become arrays; any other table, the empty table included, becomes an object with its keys as strings (`{1, nil, 3}` is `{"1":1,"3":3}`). Whole numbers below 9e15 are written without a fraction; `nan` and infinities become `null`; nil is `null`.

- `value`: nil, booleans, numbers, strings and tables of them (nested at most 64 deep).
- `pretty` (boolean, default false): true indents it.
- Returns: a string.
- Where: anywhere.
- Errors: `functions have no JSON form`; `tables nested too deep for JSON (or a cycle)`.

```lua
local text = json.encode({ name = "Alex", scores = { 3, 5, 8 } })
print(text) -- {"name":"Alex","scores":[3,5,8]}
```
"##
    ),
    e!(
        "json.decode",
        "api",
        r##"
`json.decode(text)`

JSON text as a value: objects and arrays become tables (arrays as lists from 1), `null` becomes nil (so an object's null fields are absent), numbers become numbers.

- `text` (string).
- Returns: the value; or nil and an error message (such as `"expected ident at line 1 column 2"`) when the text is not JSON. It does not raise for bad JSON.
- Where: anywhere.

```lua
local t, err = json.decode('{"a": [1, 2, 3], "ok": true}')
if t then print(#t.a, t.ok) else print("bad JSON: " .. err) end
```
"##
    ),
    // ------------------------------------------------------------- via
    e!(
        "via.status",
        "via",
        r##"
`via.status()`

ViaHorizon's state: `{enabled, version, label, text, ready}`: whether joins go through ViaProxy, the version id they go as (`"1.8.x"`, `"auto"`), its label, what ViaProxy is doing (`"Ready (ViaProxy 3.4.14, Java 21)"`), and whether it is ready.

- Returns: a table. Headless: disabled, `text = "Not in a headless test"`.
- Where: anywhere.

```lua
local st = via.status()
chat.show((st.enabled and "ViaHorizon on: " or "ViaHorizon off: ") .. st.label .. " (" .. st.text .. ")")
```
"##
    ),
    e!(
        "via.versions",
        "via",
        r##"
`via.versions()`

Every version ViaHorizon can join as, newest first: `{{id = "1.8.x", label = "1.8.x"}, ...}`. The id `"auto"` asks the server its version.

- Returns: a list. Headless: empty.
- Where: anywhere.

```lua
for i, v in ipairs(via.versions()) do
  if i <= 5 then print(v.id, v.label) end
end
```
"##
    ),
    e!(
        "via.enable",
        "via",
        r##"
`via.enable(on)`

Joins servers through ViaProxy (true) or straight (false), from the next join.

- `on` (any value; truthy is on).
- Returns: nothing.
- Where: anywhere.
- Errors: headless `ViaHorizon isn't available here`.

```lua
command("viaon", function() via.enable(true) return "Joins go through ViaProxy" end)
```
"##
    ),
    e!(
        "via.version",
        "via",
        r##"
`via.version(id)`

Chooses the version joins go as, from the next join (`"1.8.x"`, `"1.12.2"`, `"auto"`...; ids from `via.versions()`).

- `id` (string).
- Returns: nothing.
- Where: anywhere.
- Errors: an unknown version; headless `ViaHorizon isn't available here`.

```lua
command("old", function()
  local ok, err = pcall(via.version, "1.8.x")
  return ok and "Joining as 1.8.x" or tostring(err)
end)
```
"##
    ),
    e!(
        "via.log",
        "via",
        r##"
`via.log()`

ViaProxy's last lines (up to 40), oldest first, for when a join fails.

- Returns: a list of strings. Headless: empty.
- Where: anywhere.

```lua
command("vialines", function()
  for _, line in ipairs(via.log()) do print(line) end
  return "In the console (F8)"
end)
```
"##
    ),
    // ------------------------------------------------------------- voice
    e!(
        "voice.status",
        "voice",
        r##"
`voice.status()`

The voice chat's state on a Simple Voice Chat server: `{state, connected, unavailable, talking, muted, deafened, activation, ptt_key, volume, gain, threshold, level, players, compat, error, mic}`. `state` is `"off"` (not on a server), `"asking"`, `"none"` (the server has no voice chat), `"incompatible"`, `"connecting"`, `"connected"`, `"lost"` (the voice server stopped answering; it reconnects) or `"failed"`; `unavailable` is true when the server has voice chat but it isn't working. `level` is the microphone's level in dBFS, `compat` the version the server speaks (20 or 21), `mic` the microphone's device (or why there is none).

- Returns: a table. Headless: `state = "off"`, the settings' defaults.
- Where: anywhere.

```lua
on("tick", function()
  local v = voice.status()
  if v.state ~= "connected" then hud.panel(nil) return end
  hud.panel("top-right", {"Voice: " .. v.players .. " players", string.format("Mic: %.0f dB", v.level)})
end)
```
"##
    ),
    e!(
        "voice.connected",
        "voice",
        r##"
`voice.connected()`

Whether this server's voice chat is connected (voices come and the microphone can go).

- Returns: a boolean. Headless: false.
- Where: anywhere.

```lua
every(5, function()
  if voice.connected() then print("voice chat is up") end
end)
```
"##
    ),
    e!(
        "voice.talking",
        "voice",
        r##"
`voice.talking()`

Whether the microphone is sending now (push to talk held, or the voice above the threshold; not while muted).

- Returns: a boolean. Headless: false.
- Where: anywhere.

```lua
on("tick", function()
  if voice.talking() then hud.panel("bottom-right", {"On air"}) else hud.panel(nil) end
end)
```
"##
    ),
    e!(
        "voice.speakers",
        "voice",
        r##"
`voice.speakers()`

Who is heard now: `{{name = "Alex", whispering = false}, ...}`, by name. Someone counts for a quarter of a second after their last sound, and only while heard (near enough, or in the group).

- Returns: a list. Headless: empty.
- Where: anywhere.

```lua
on("tick", function()
  local names = {}
  for _, s in ipairs(voice.speakers()) do names[#names + 1] = s.name end
  if #names == 0 then hud.panel(nil) return end
  hud.panel("top-right", {"Talking: " .. table.concat(names, ", ")})
end)
```
"##
    ),
    e!(
        "voice.mute",
        "voice",
        r##"
`voice.mute([on])`

Mutes the microphone (true) or unmutes it (false); without a value only reads it. The server is told, as SVC's mute does. Kept in `voice.txt`.

- `on` (any value, optional; truthy mutes).
- Returns: whether it is muted.
- Where: anywhere.
- Errors: headless `voice chat isn't available here` (when a value is given).

```lua
command("mute", function()
  local now = voice.mute(not voice.mute())
  return now and "Muted" or "Unmuted"
end)
```
"##
    ),
    e!(
        "voice.deafen",
        "voice",
        r##"
`voice.deafen([on])`

Deafens (true): nothing is heard and nothing sent, SVC's "disable voice chat" (the server stops sending voices). False hears again; without a value only reads it.

- `on` (any value, optional; truthy deafens).
- Returns: whether it is deafened.
- Where: anywhere.
- Errors: headless `voice chat isn't available here` (when a value is given).

```lua
command("deafen", function()
  return voice.deafen(not voice.deafen()) and "Deafened" or "Hearing again"
end)
```
"##
    ),
    e!(
        "voice.volume",
        "voice",
        r##"
`voice.volume([x])`

The voices' volume, 0 to 3 (1 as they come; outside that range it is clamped); without a value only reads it.

- `x` (number, optional).
- Returns: the volume.
- Where: anywhere.
- Errors: a value that isn't a finite number; headless `voice chat isn't available here` (when a value is given).

```lua
command("loud", function()
  return "Voice volume " .. voice.volume(voice.volume() + 0.25)
end)
```
"##
    ),
    e!(
        "voice.gain",
        "voice",
        r##"
`voice.gain([db])`

The microphone's gain in dB, -40 to 24 (0 as it comes; -40 is silence), as SVC's microphone gain; a limiter keeps it from clipping. Without a value only reads it.

- `db` (number, optional).
- Returns: the gain in dB.
- Where: anywhere.
- Errors: a value that isn't a finite number; headless `voice chat isn't available here` (when a value is given).

```lua
command("micboost", function()
  return "Microphone gain " .. voice.gain(6) .. " dB"
end)
```
"##
    ),
    e!(
        "voice.threshold",
        "voice",
        r##"
`voice.threshold([db])`

Voice activation's threshold, in dBFS from -127 to 0 (-50 by default, as SVC's): the microphone sends while a frame's peak is above it, and half a second after. Without a value only reads it.

- `db` (number, optional).
- Returns: the threshold.
- Where: anywhere.
- Errors: a value that isn't a finite number; headless `voice chat isn't available here` (when a value is given).

```lua
command("noisy", function()
  voice.activation("voice")
  return "Voice activation above " .. voice.threshold(-35) .. " dB"
end)
```
"##
    ),
    e!(
        "voice.activation",
        "voice",
        r##"
`voice.activation([mode])`

How the microphone is turned on: `"ptt"` (push to talk, while its key is held: Caps Lock unless chosen otherwise in Options > Mods) or `"voice"` (voice activation, above `voice.threshold()`). Without a value only reads it.

- `mode` (string, optional): `"ptt"` or `"voice"`.
- Returns: the mode.
- Where: anywhere.
- Errors: another mode; headless `voice chat isn't available here` (when a value is given).

```lua
command("ptt", function()
  local m = voice.activation(voice.activation() == "ptt" and "voice" or "ptt")
  return "Microphone: " .. m
end)
```
"##
    ),
    e!(
        "voice.groups",
        "voice",
        r##"
`voice.groups()`

The server's voice groups (when it has them on): `{{id = "<uuid>", name = "Friends", password = false, joined = true}, ...}`, by name. A group's voices are heard by its members without a place.

- Returns: a list. Headless: empty.
- Where: anywhere.

```lua
command("groups", function()
  for _, g in ipairs(voice.groups()) do
    print(g.name .. (g.joined and " (joined)" or "") .. (g.password and " (password)" or ""))
  end
  return "In the console (F8)"
end)
```
"##
    ),
    e!(
        "voice.join_group",
        "voice",
        r##"
`voice.join_group(id[, password])`

Joins a voice group, by its id from `voice.groups()` (and its password if it has one). The server answers; `voice.groups()` then marks it joined.

- `id` (string), `password` (string, optional).
- Returns: nothing.
- Where: anywhere.
- Errors: `no such group`; `this server's voice chat has no groups`; headless `voice chat isn't available here`.

```lua
command("join", function(args)
  for _, g in ipairs(voice.groups()) do
    if g.name == args[1] then voice.join_group(g.id, args[2]) return "Joining " .. g.name end
  end
  return "No group " .. tostring(args[1])
end)
```
"##
    ),
    e!(
        "voice.leave_group",
        "voice",
        r##"
`voice.leave_group()`

Leaves the voice group joined (nothing when none is).

- Returns: nothing.
- Where: anywhere.
- Errors: headless `voice chat isn't available here`.

```lua
command("leave", function() voice.leave_group() return "Left the group" end)
```
"##
    ),
    // ------------------------------------------------------------- console only
    e!(
        "help",
        "console",
        r##"
`help([name])`

Console only. Lists the API in the console: every function's one-line summary, or with a name, that function's line (an exact full name such as `"player.pos"`), or every function whose name contains the text (`help("draw")`).

- `name` (string, optional).
- Returns: nothing (it logs the lines; `"nothing by that name"` when none match).
- Where: the console only (in a mod it is nil).

```lua
help("player.pos")
help("model")
```
"##
    ),
    e!(
        "clear",
        "console",
        r##"
`clear()`

Console only. Empties the console's log and drops everything the console registered: its handlers, commands, timers, HUD panel, zoom and models. The console's globals stay.

- Returns: nothing.
- Where: the console only.

```lua
clear()
```
"##
    ),
    e!(
        "stop",
        "console",
        r##"
`stop()`

Console only. Stops everything the console ran: code typed there, run from an executor style's Execute, or loaded by such code (`loadstring(game:HttpGet(url))()`). Its threads, timers, handlers and Roblox events stop, and the Roblox instances it made (its GUIs) go. The executor's shared globals (`getgenv()`) start afresh, so a script that marks itself loaded there can run again. The console's log stays. The Console pane's **Stop scripts** does the same.

- Returns: nothing; the console says whether anything was running.
- Where: the console only.

```lua
stop()
```
"##
    ),
    e!(
        "reload",
        "console",
        r##"
`reload([id])`

Console only. Reloads one script mod (turning it on if it was off in the Library), or every mod without an id, once the console line has finished. Mods that import it are not reloaded by this call; saving the file or the MCP tool `reload_mod` reloads them too.

- `id` (string, optional): a mod id.
- Returns: nothing.
- Where: the console only.
- Errors: `no script mod 'x'`.

```lua
reload("waypoints")
```
"##
    ),
    e!(
        "mods",
        "console",
        r##"
`mods()`

Console only. Every script mod and its state: `{waypoints = "running", broken = "failed: broken.lua:3: ...", old = "off"}`.

- Returns: a table.
- Where: the console only.

```lua
for id, state in pairs(mods()) do print(id, state) end
```
"##
    ),
    e!(
        "edit",
        "console",
        r##"
`edit(id[, file])`

Console only. Opens a mod in the studio's Editor tab (after the console line): its main file, or with `file` another file of a module, by its path in the module (`"lib.lua"`, `"lib/util"`; the `.lua` may be left out). Unsaved edits in the file open before are saved first; the file open already stays as it is, unsaved edits and all. Run by an agent (`run_lua`), it opens no other file while the player is using the studio or has a prompt or a menu up, nor over unsaved edits in the file open: an error line says why, as `open_script` would wait or refuse. The file open already, as with `open_script`, is never refused: it comes forward unless the player is using the studio or has a prompt or a menu up.

- `id` (string): a mod id.
- `file` (string, optional): a file of the module.
- Returns: nothing.
- Where: the console only.
- Errors: `bad argument #1 to 'edit' (a mod id expected)`; `no script mod 'x'`; `'hello' is a script, one file: edit("hello") opens it`; `no nope.lua in tool (it has main.lua, lib.lua, shared.lua)`.

```lua
edit("waypoints")
edit("tool", "lib.lua")
```
"##
    ),
    e!(
        "new",
        "console",
        r##"
`new(name[, kind])`

Console only. Makes a new mod and opens it in the Editor: a script, `<id>.lua` from the template (the id made from the name), or with `kind` `"module"` a module, the folder `<id>/` with `main.lua`, `shared.lua` and `lib.lua` (main.lua requires the other two, and every file shares the table shared.lua returns). If a mod has that id, the console logs `<id> already exists`. Unsaved edits in the file open are saved first. Run by an agent (`run_lua`), it makes and opens nothing while the player is using the studio or has a prompt or a menu up, nor over unsaved edits in the file open: an error line says why, as `open_script` would wait or refuse.

- `name` (string): the mod's name; the id is made from it.
- `kind` (string, optional): `"script"` (the default) or `"module"`.
- Returns: nothing.
- Where: the console only.
- Errors: `bad argument #2 to 'new' ("script" or "module" expected)`.

```lua
new("My Tool")
new("Night Light", "module")
```
"##
    ),
    // ------------------------------------------------------------- events
    e!(
        "event:tick",
        "events",
        r##"
`on("tick", function() ... end)`

Runs 20 times a second (each game tick) while the player is in a world; not on the title screen or while loading. After a slow frame the studio runs up to 10 missed ticks at once. No arguments.

- Works: everything but drawing (`hud.text`, `hud.rect`, `gfx.rect`/`border`/`text`, `draw.*`). Good for game logic, `hud.panel`, particles, sounds, `model.set`.
- An error stops the mod: `tick: <message>`.

In tests: each headless frame is one tick (`{"frames": 20}` is a second).

```lua
local n = 0
on("tick", function()
  n = n + 1
  if n % 20 == 0 then hud.panel("top-right", { "Seconds: " .. n // 20 }) end
end)
```
"##
    ),
    e!(
        "event:frame",
        "events",
        r##"
`on("frame", function(dt) ... end)`

Runs every frame, in a world or not (the title screen too), after ticks, timers and agent replies.

- `dt` (number): seconds since the last frame, 0 to 1 (headless 0.05).
- Works: as `tick`; use it for smooth animation (multiply speeds by `dt`).
- An error stops the mod: `frame: <message>`.

```lua
local angle = 0
on("frame", function(dt)
  angle = (angle + 90 * dt) % 360 -- a quarter turn a second
end)
on("world", function()
  draw.item("minecraft:compass", 0.5, 65, 0.5, { yaw = angle })
end)
```
"##
    ),
    e!(
        "event:hud",
        "events",
        r##"
`on("hud", function(w, h) ... end)`

Runs every frame the HUD is drawn: in a world, not while F1 hides the HUD. Draw with `hud.text`, `hud.rect` and `gfx.*` here.

- `w`, `h` (numbers): the GUI's size in GUI pixels (headless 480, 270). `gfx.*` uses dp: `gfx.size()`.
- Works: drawing, reading (`player.*`, `world.block`, `world.time`, `keys.*`, `storage.get`...).
- Not here: `agent.*`, `world.find`, `world.entities` (`agent functions are not available while drawing the HUD`), `world.particle` and `sound.*` (`... doesn't work while drawing`), `draw.*`.
- An error stops the mod: `hud: <message>`.

In tests: text drawn shows in a step's `drawn` (`expect_drawn`).

```lua
on("hud", function(w, h)
  local x, y, z = player.block()
  if not x then return end
  local text = ("%d %d %d"):format(x, y, z)
  hud.rect(w - hud.width(text) - 8, 4, hud.width(text) + 4, 12, "#80000000")
  hud.text(w - hud.width(text) - 6, 6, text, "#ffffff")
end)
```
"##
    ),
    e!(
        "event:world",
        "events",
        r##"
`on("world", function() ... end)`

Runs every frame as the world is drawn, with the frame's camera. Draw with `draw.*` here (shapes in the world, through-wall lines and labels, `draw.item`); what is drawn lasts this frame only. No arguments.

- Works: `draw.*`, reading (`player.*`, `world.block`, `world.time`, `keys.*`).
- Not here: `hud.text`, `hud.rect`, `gfx.*` drawing, `world.particle`, `sound.*` (errors); in the game also the agent (`the agent is not available here`); `chat.show`, `chat.say` and `game.run` happen a frame later.
- An error stops the mod: `world: <message>`.

```lua
on("world", function()
  local x, y, z = player.block()
  if not x then return end
  draw.block(x, y - 1, z, "#809a86ff", 0.03)
end)
```
"##
    ),
    e!(
        "event:key",
        "events",
        r##"
`on("key", function(key, down) ... end)`

Runs when a key or mouse button goes down or up, on the next frame. Keys come on any game screen (also while the player types in chat: check `game.screen() == "playing"`), mouse buttons only while playing; held keys do not repeat. Nothing comes while the studio's screen or a script screen is open (they take the input).

- `key` (string): the game's name, `"key.keyboard.r"`, `"key.keyboard.space"`, `"key.keyboard.left.shift"`, `"key.mouse.left"`, `"key.mouse.right"` (see the topic `input`).
- `down` (boolean): true when pressed, false when released.
- Works: everything but drawing.
- An error stops the mod: `key: <message>`.

In tests: `{"key": "key.keyboard.r"}` presses and releases it.

```lua
on("key", function(key, down)
  if key == "key.keyboard.j" and down and game.screen() == "playing" then
    chat.show("J pressed")
  end
end)
```
"##
    ),
    e!(
        "event:chat",
        "events",
        r##"
`on("chat", function(text) ... end)`

Runs for each chat line that arrives from the server (players' messages, system and command messages), on the next frame. Lines the client shows itself (`chat.show`, the studio's own) do not come here. Up to 64 lines are queued a frame.

- `text` (string): the line's plain text, without colors (`"<Alex> hello"` for a vanilla player message).
- Works: everything but drawing. Answering with `chat.say` in a loop can spam: the rate limit applies.
- An error stops the mod: `chat: <message>`.

In tests: `{"chat": "<Alex> hello"}`.

```lua
on("chat", function(text)
  local who, msg = text:match("^<(.-)> (.*)$")
  if msg and msg:lower():find("ping", 1, true) then chat.show(who .. " said ping") end
end)
```
"##
    ),
    e!(
        "event:unload",
        "events",
        r##"
`on("unload", function() ... end)`

Runs before the mod reloads (its files changed, `reload`, a library it imports reloaded), when it is turned off in the Library, and when its file is deleted. Not when it fails, and not when the game quits. The mod's registrations are dropped right after, so there is no need to clear them here; use it to stop agent actions or save state.

- No arguments. An error here is logged (`<id>: unload: <message>`) and the unload goes on.

```lua
local seen = storage.get("seen") or 0
on("tick", function() seen = seen + 1 end)
on("unload", function() storage.set("seen", seen) end)
```
"##
    ),
    e!(
        "event:custom",
        "events",
        r##"
`on("<mod>:<name>", function(...) ... end)`

A mod-to-mod event: any event name with a `:` in it (at most 64 characters). It runs on the frame after a mod calls `emit` with that name, with the values given to `emit` (up to 16; tables are shared, not copied). Every running mod's handlers for the name run, the emitting mod's own included, then the console's. Use `"<your mod id>:<what>"` names.

- Works: everything but drawing.
- An error stops the handling mod: `<name>: <message>`.

```lua
-- one mod
command("ping", function() emit("pinger:ping", game.ticks()) end)
-- any mod
on("pinger:ping", function(tick) chat.show("ping at tick " .. tick) end)
```
"##
    ),
    // ------------------------------------------------------------- widgets
    e!(
        "widget:label",
        "screens",
        r##"
`{type = "label", text = "...", color = ...}`

Text in a screen's panel, wrapped to its width (14 dp Geist, 21 dp a line). A widget without `type`, or with a type the studio does not know, is a label too. Clicking it does nothing.

- `text` (any value): shown with `tostring`; change `widget.text` and the next frame shows it.
- `color` (a color, optional): the text's color, always opaque; default the theme's soft ink.

```lua
local status = { type = "label", text = "Ready", color = "#45c27a" }
command("status", function()
  ui.open({ title = "Status", widgets = { status } })
  after(2, function() status.text = "Still here" end)
end)
```
"##
    ),
    e!(
        "widget:button",
        "screens",
        r##"
`{type = "button", text = "...", primary = false, click = function(widget) ... end}`

A pill-shaped button across the panel (44 dp tall).

- `text` (string): its label.
- `primary` (boolean): true draws it in the accent color (for the main action).
- `click(widget)`: called with the widget's own table when it is left-clicked. A right press goes to the screen's `click` instead.

In tests: `{"click": "<its text>"}`.

```lua
command("ask", function()
  ui.open({ title = "Sure?", widgets = {
    { type = "button", text = "Yes", primary = true, click = function() chat.show("Yes") ui.close() end },
    { type = "button", text = "No", click = function() ui.close() end },
  } })
end)
```
"##
    ),
    e!(
        "widget:toggle",
        "screens",
        r##"
`{type = "toggle", text = "...", value = false, change = function(value, widget) ... end}`

A switch with a label (40 dp tall). A left click flips `value`: the studio sets `widget.value` first, then calls `change`.

- `text` (string): its label.
- `value` (boolean): on or off; set it to change what it shows.
- `change(value, widget)`: called with the new value.

```lua
local loud = false
command("loud", function()
  ui.open({ title = "Sound", widgets = {
    { type = "toggle", text = "Loud", value = loud, change = function(v) loud = v end },
  } })
end)
```
"##
    ),
    e!(
        "widget:slider",
        "screens",
        r##"
`{type = "slider", text = "...", min = 0, max = 1, step = 0, value = 0, change = function(value, widget) ... end}`

A slider with its label and value shown (50 dp tall). Pressing on it sets the value where the pointer is, dragging moves it, and the wheel over it moves it a step. The studio sets `widget.value`, then calls `change` (pressing and dragging only when the value changed).

- `text` (string): its label.
- `min`, `max` (numbers, default 0 and 1).
- `step` (number, default 0): the value snaps to `min + k * step`; 0 means no snapping (the wheel then moves it a twentieth of the range). With a step of 1 or more and a whole value, it shows as a whole number, otherwise with 2 decimals.
- `value` (number, default `min`).
- `change(value, widget)`.

```lua
local size = 5
command("size", function()
  ui.open({ title = "Size", widgets = {
    { type = "slider", text = "Size", min = 1, max = 10, step = 1, value = size,
      change = function(v) size = v end },
  } })
end)
```
"##
    ),
    e!(
        "widget:input",
        "screens",
        r##"
`{type = "input", text = "label", value = "", placeholder = "...", change = function(value, widget) ... end, submit = function(value, widget) ... end}`

A one-line text field with a label above it (62 dp tall). Clicking it gives it focus; then typing, Backspace, Delete, the arrows, Home, End and Control+V edit it, Tab goes to the next input, Enter calls `submit` and drops the focus, Escape drops the focus.

- `text` (string): the label above the field.
- `value` (string): the text in it; the studio keeps it up to date as the player types.
- `placeholder` (string): shown greyed while it is empty and not focused.
- `change(value, widget)`: after every edit, with the whole text.
- `submit(value, widget)`: on Enter.

In tests: `{"click": "<its label>"}` focuses it, `{"type": "text"}` types into it (`change` runs; `submit` cannot be pressed headless, so test `change` or the effect of `value`).

```lua
local name = ""
command("name", function()
  ui.open({ title = "Name", widgets = {
    { type = "input", text = "Your name", value = name, placeholder = "type, then Enter",
      change = function(v) name = v end,
      submit = function(v) chat.show("Hello, " .. v) ui.close() end },
  } })
end)
```
"##
    ),
    e!(
        "widget:space",
        "screens",
        r##"
`{type = "space", height = 12}`

Empty space between widgets.

- `height` (number, default 12): in dp, 0 to 300.

```lua
local widgets = {
  { type = "label", text = "Above" },
  { type = "space", height = 24 },
  { type = "label", text = "Below" },
}
command("gap", function() ui.open({ title = "Gap", widgets = widgets }) end)
```
"##
    ),
    // ------------------------------------------------------------- base functions
    e!(
        "print",
        "stdlib",
        r##"
`print(...)`

Writes its arguments, each through `tostring` and separated by tabs, as one line in the studio's console log (not in the chat; use `chat.show` for that). A mod's lines start with `[<id>] `. In tests the lines are in a step's `log` (`expect_log`), and a `lua` step's prints in `output`.

```lua
print("loaded", 42, nil, true) -- [mymod] loaded	42	nil	true
```
"##
    ),
    e!(
        "type",
        "stdlib",
        r##"
`type(v)`

The type's name: `"nil"`, `"boolean"`, `"number"`, `"string"`, `"table"` or `"function"` (built-in functions too). There are no userdata or threads. `type()` with no argument raises "bad argument #1 to 'type' (value expected)".

```lua
assert(type(print) == "function" and type(1.5) == "number")
```
"##
    ),
    e!(
        "tostring",
        "stdlib",
        r##"
`tostring(v)`

A value as text, with `__tostring` when its metatable has one. Whole numbers have no fraction (`tostring(3.0)` is `"3"`), others 14 significant digits; tables and functions show as `table: 0x...` and `function: 0x...` (built-ins as `function: builtin 'print': 0x...`).

```lua
local s = tostring(10 / 4) .. " " .. tostring(nil) -- "2.5 nil"
```
"##
    ),
    e!(
        "tonumber",
        "stdlib",
        r##"
`tonumber(v[, base])`

A number from a number or a string: decimal (`"1e3"`, `".5"`, `"  5  "`) or hexadecimal integers (`"0x10"`); nil when it is not one (`"5x"`, `"inf"`, hexadecimal floats). With a `base` (2 to 36), the string is read in that base (`tonumber("ff", 16)` is 255, `tonumber("z", 36)` is 35). Other types give nil.

```lua
local n = tonumber("42") or 0
local bits = tonumber("1011", 2) -- 11
```
"##
    ),
    e!(
        "pairs",
        "stdlib",
        r##"
`pairs(t)`

An iterator over every key and value of a table: keys 1..n first, then the other keys in the order they were added (a stable order, unlike real Lua). With a `__pairs` metamethod, its three results are used instead. Setting existing keys to nil while iterating is fine.

```lua
for k, v in pairs({ a = 1, b = 2, 10, 20 }) do print(k, v) end -- 1 10, 2 20, a 1, b 2
```
"##
    ),
    e!(
        "ipairs",
        "stdlib",
        r##"
`ipairs(t)`

An iterator over `t[1], t[2], ...` up to the first nil. A table with a metatable is read through `__index`. `ipairs(nil)` raises "bad argument #1 to 'ipairs' (table expected, got nil)".

```lua
for i, name in ipairs({ "a", "b", "c" }) do print(i, name) end
```
"##
    ),
    e!(
        "next",
        "stdlib",
        r##"
`next(t[, key])`

The key and value after `key` in the order `pairs` uses (nil: the first), or nil at the end. `next(t) == nil` tests for an empty table. A key the table does not hold raises "invalid key to 'next'".

```lua
local function is_empty(t) return next(t) == nil end
```
"##
    ),
    e!(
        "select",
        "stdlib",
        r##"
`select(n, ...)`

`select("#", ...)` is the number of values (nils included); `select(n, ...)` returns the values from the nth on, and a negative `n` counts from the end. `n` of 0, or before the first, raises "bad argument #1 to 'select' (index out of range)".

```lua
local function count(...) return select("#", ...) end
local third = select(3, "a", "b", "c", "d") -- "c" (and "d")
```
"##
    ),
    e!(
        "rawget",
        "stdlib",
        r##"
`rawget(t, k)`

`t[k]` without `__index`.

```lua
local t = setmetatable({}, { __index = function() return "default" end })
assert(rawget(t, "x") == nil and t.x == "default")
```
"##
    ),
    e!(
        "rawset",
        "stdlib",
        r##"
`rawset(t, k, v)`

`t[k] = v` without `__newindex`; returns `t`. A nil or NaN key raises "index is nil" / "index is NaN".

```lua
local t = setmetatable({}, { __newindex = function() error("read only") end })
rawset(t, "x", 1)
```
"##
    ),
    e!(
        "rawequal",
        "stdlib",
        r##"
`rawequal(a, b)`

Whether two values are equal without `__eq` (tables and functions: the same one).

```lua
assert(not rawequal({}, {}))
```
"##
    ),
    e!(
        "rawlen",
        "stdlib",
        r##"
`rawlen(v)`

The length of a table (without `__len`) or a string, in bytes. Anything else raises "bad argument #1 to 'rawlen' (table or string expected)".

```lua
assert(rawlen({ 1, 2, 3 }) == 3 and rawlen("abc") == 3)
```
"##
    ),
    e!(
        "setmetatable",
        "stdlib",
        r##"
`setmetatable(t, mt)`

Sets (or with nil removes) a table's metatable and returns `t`. Only tables have metatables. A metatable with a `__metatable` field cannot be changed: "cannot change a protected metatable". The metamethods supported are in the topic `language`.

```lua
local Point = {}
Point.__index = Point
function Point:len() return math.sqrt(self.x ^ 2 + self.y ^ 2) end
local p = setmetatable({ x = 3, y = 4 }, Point)
print(p:len()) -- 5
```
"##
    ),
    e!(
        "getmetatable",
        "stdlib",
        r##"
`getmetatable(v)`

A table's metatable (or its `__metatable` field when it has one), or nil. Strings and other values have none here: `getmetatable("")` is nil.

```lua
local mt = {}
assert(getmetatable(setmetatable({}, mt)) == mt)
```
"##
    ),
    e!(
        "error",
        "stdlib",
        r##"
`error(value[, level])`

Raises an error with any value. A string gets the place in front: level 1 (the default) where `error` was called, 2 the caller of that function, 0 nothing. Uncaught, it stops the mod with that message (a table shows as "(error object is a table value)").

```lua
local function need(n)
  if type(n) ~= "number" then error("need a number, got " .. type(n), 2) end
  return n
end
print(pcall(need, "x")) -- false   mymod.lua:5: need a number, got string
```
"##
    ),
    e!(
        "assert",
        "stdlib",
        r##"
`assert(v[, message, ...])`

Returns all its arguments when `v` is neither nil nor false; otherwise raises `message` as it is (any value), or "assertion failed!" with the place. In a test's `lua` step, a failing `assert` fails the step.

```lua
local x, y, z = player.pos()
assert(x, "not in a world")
```
"##
    ),
    e!(
        "pcall",
        "stdlib",
        r##"
`pcall(f, ...)`

Calls `f(...)` and catches errors: returns `true` and `f`'s results, or `false` and the error value. Running out of steps, a string over 4 MiB and a table over 1,048,576 entries are not caught: they stop the mod.

```lua
local ok, err = pcall(function() return world.find("diamond_ore") end)
if not ok then print("find failed: " .. tostring(err)) end
```
"##
    ),
    e!(
        "xpcall",
        "stdlib",
        r##"
`xpcall(f, handler, ...)`

Like `pcall`, but on an error calls `handler(error)` and returns `false` and the handler's results. With fewer than two arguments it raises "bad argument #2 to 'xpcall' (value expected)".

```lua
local ok, msg = xpcall(function() error("boom") end, function(e) return "caught: " .. e end)
```
"##
    ),
    e!(
        "load",
        "stdlib",
        r##"
`load(text[, name[, mode[, env]]])`

Compiles Lua text into a function (it does not run it). Returns the function, or nil and the syntax error. Text only: no reader functions and no binary chunks (`mode` is ignored). `name` (default `"load"`, a leading `=` or `@` dropped) names the chunk in errors. Its globals are `env` when given, else the mod's own globals. Running it is subject to the same step budget.

```lua
local f, err = load("return 1 + ...")
if f then print(f(41)) else print(err) end -- 42
local sandboxed = load("x = 1", "cfg", "t", {})
```
"##
    ),
    e!(
        "unpack",
        "stdlib",
        r##"
`unpack(t[, i[, j]])`

`t[i], ..., t[j]` as separate values (`i` defaults to 1, `j` to `#t`); the same as `table.unpack`, kept as a global as in Lua 5.1. More than 1,000,000 values raise "too many results to unpack".

```lua
local x, y, z = unpack({ 1, 2, 3 })
print(math.max(unpack({ 4, 9, 2 }))) -- 9
```
"##
    ),
    e!(
        "dump",
        "stdlib",
        r##"
`dump(v[, depth])`

The studio's own: a value as readable text, as the console shows values: strings quoted, tables expanded `depth` levels (default 3, at most 8; 30 entries each), cycles marked `<cycle>`. Handy with `print`.

```lua
print(dump({ name = "Alex", pos = { 1, 2, 3 } })) -- {\n  name = "Alex",\n  pos = { 1, 2, 3 },\n}
```
"##
    ),
    e!(
        "inspect",
        "stdlib",
        r##"
`inspect(v[, depth])`

The same function as `dump`: a value as readable text.

```lua
print(inspect(player.info(), 1))
```
"##
    ),
    e!(
        "collectgarbage",
        "stdlib",
        r##"
`collectgarbage([opt])`

Does nothing and returns 0 (memory is managed by the studio; `collectgarbage("count")` is 0).

```lua
collectgarbage()
```
"##
    ),
    e!(
        "_G",
        "stdlib",
        r##"
`_G`

The mod's own globals table (each mod and the console has its own). `_G.name` is the global `name`; `_G[name]` reads a global by a computed name. There is no `_ENV`.

```lua
_G.counter = (_G.counter or 0) + 1
for name, v in pairs(_G) do if type(v) == "table" and name == "draw" then print("draw is there") end end
```
"##
    ),
    e!(
        "_VERSION",
        "stdlib",
        r##"
`_VERSION`

The string `"Lua 5.3 (Horizon Studio)"`.

```lua
print(_VERSION)
```
"##
    ),
    // ------------------------------------------------------------- string
    e!(
        "string.len",
        "stdlib",
        r##"
`string.len(s)` / `s:len()`

The length in bytes (`#s` too). For characters of UTF-8 text use `utf8.len`.

```lua
assert(("héllo"):len() == 6 and utf8.len("héllo") == 5)
```
"##
    ),
    e!(
        "string.sub",
        "stdlib",
        r##"
`string.sub(s[, i[, j]])` / `s:sub(i, j)`

The bytes from `i` to `j` (both included; default 1 and -1). Negative positions count from the end (-1 is the last byte); positions outside the string are clamped.

```lua
local s = "hello"
print(s:sub(2, 4), s:sub(-3), s:sub(2)) -- ell llo ello
```
"##
    ),
    e!(
        "string.upper",
        "stdlib",
        r##"
`string.upper(s)` / `s:upper()`

ASCII letters in upper case (other bytes as they are).

```lua
print(("Hello"):upper()) -- HELLO
```
"##
    ),
    e!(
        "string.lower",
        "stdlib",
        r##"
`string.lower(s)` / `s:lower()`

ASCII letters in lower case.

```lua
if ("YES"):lower() == "yes" then print("same") end
```
"##
    ),
    e!(
        "string.rep",
        "stdlib",
        r##"
`string.rep(s, n[, sep])` / `s:rep(n, sep)`

`s` repeated `n` times with `sep` between; `n` of 0 or less gives `""`. A result over 4 MiB stops the mod ("string.rep: resulting string is too large").

```lua
print(("ab"):rep(3, "-")) -- ab-ab-ab
local bar = ("|"):rep(10)
```
"##
    ),
    e!(
        "string.reverse",
        "stdlib",
        r##"
`string.reverse(s)` / `s:reverse()`

The bytes in reverse order.

```lua
print(("abc"):reverse()) -- cba
```
"##
    ),
    e!(
        "string.byte",
        "stdlib",
        r##"
`string.byte(s[, i[, j]])` / `s:byte(i, j)`

The byte values of `s[i..j]` (default `i` 1, `j` = `i`), negative positions from the end; nothing when out of range.

```lua
print(("A"):byte(), ("abc"):byte(1, -1)) -- 65 97 98 99
```
"##
    ),
    e!(
        "string.char",
        "stdlib",
        r##"
`string.char(...)`

A string of the given byte values (0 to 255; others raise "value out of range"). For code points above 127 use `utf8.char`.

```lua
print(string.char(72, 105)) -- Hi
```
"##
    ),
    e!(
        "string.format",
        "stdlib",
        r##"
`string.format(fmt, ...)` / `fmt:format(...)`

Formats values as C's `printf`: `%d` and `%i` (whole numbers only: a fraction raises "number has no integer representation"), `%u`, `%c`, `%x`, `%X`, `%o`, `%f`/`%F`, `%e`/`%E`, `%g`/`%G`, `%q` (a quoted Lua string), `%s` (any value through `tostring`), `%%`. Flags `-`, `+`, space, `#`, `0`, a width and a precision (each at most 99). `%a` is not supported. A missing argument raises "bad argument #N to 'format' (no value)".

```lua
local x, y, z = 10.5, 64, -3.25
print(string.format("%.1f / %d / %.2f", x, y, z))   -- 10.5 / 64 / -3.25
print(("%5s|%-5s|%05d|%x"):format("ab", "ab", 42, 255)) -- "   ab|ab   |00042|ff"
```
"##
    ),
    e!(
        "string.find",
        "stdlib",
        r##"
`string.find(s, pattern[, init[, plain]])` / `s:find(pattern, init, plain)`

The start and end of the first match of a Lua pattern (from byte `init`, default 1; negative counts from the end), and the captures; nil when none. With `plain` true (or a pattern without special characters), a plain substring search.

Patterns: `.` any byte, `%a` letters, `%d` digits, `%l` lower, `%u` upper, `%s` spaces, `%w` letters and digits, `%x` hex digits, `%p` punctuation, `%c` control, `%g` printable but space (upper case: the opposite), `%.` a literal `.`, `[set]`, `[^set]`, ranges `[a-z]`; `*`, `+`, `-` (lazy), `?`; `^` and `$` anchors; captures `( )` and `()` (a position); `%1`..`%9` back references; `%b()` balanced; `%f[set]` frontier. At most 32 captures.

```lua
print(("hello world"):find("o w"))       -- 5 7
print(("key=val"):find("(%w+)=(%w+)"))   -- 1 7 key val
print(("a.b"):find(".", 1, true))        -- 2 2
```
"##
    ),
    e!(
        "string.match",
        "stdlib",
        r##"
`string.match(s, pattern[, init])` / `s:match(pattern, init)`

The captures of the first match (or the whole match when the pattern has none), or nil. Patterns as `string.find`.

```lua
local name, value = ("volume=7"):match("^(%w+)=(%d+)$")
local who = ("<Alex> hi"):match("^<(.-)>")
```
"##
    ),
    e!(
        "string.gmatch",
        "stdlib",
        r##"
`string.gmatch(s, pattern)` / `s:gmatch(pattern)`

An iterator over every match: each round gives the captures (or the whole match).

```lua
local words = {}
for w in ("one two three"):gmatch("%a+") do words[#words + 1] = w end
for k, v in ("a=1, b=2"):gmatch("(%w+)=(%w+)") do print(k, v) end
```
"##
    ),
    e!(
        "string.gsub",
        "stdlib",
        r##"
`string.gsub(s, pattern, repl[, n])` / `s:gsub(pattern, repl, n)`

`s` with matches replaced (at most `n` of them), and the number of matches. `repl` is a string (`%0` the whole match, `%1`..`%9` captures, `%%` a percent), a table (looked up with the first capture), or a function (called with the captures; its result replaces the match). A false or nil result keeps the match. A result over 4 MiB stops the mod.

```lua
local s, n = ("hello world"):gsub("o", "0")             -- "hell0 w0rld", 2
local t = ("$name is $age"):gsub("%$(%w+)", { name = "Alex", age = 30 })
local caps = ("abc"):gsub("%w", function(c) return c:upper() end)
```
"##
    ),
    e!(
        "string.split",
        "stdlib",
        r##"
`string.split(s[, sep[, plain]])` / `s:split(sep, plain)`

The studio's own: the pieces of `s` between separators, as a list.

- Without `sep`: split on runs of whitespace, empty pieces dropped (`("  a b ")` gives `{"a", "b"}`).
- `sep = ""`: every byte apart.
- Otherwise `sep` is plain text (`plain` defaults to true) and empty pieces are kept (`"a,,b"` gives `{"a", "", "b"}`); with `plain = false`, `sep` is a Lua pattern.

```lua
local parts = ("x,y,,z"):split(",")      -- { "x", "y", "", "z" }
local words = ("  go   home "):split()   -- { "go", "home" }
local nums = ("a1b22c"):split("%d+", false)
```
"##
    ),
    e!(
        "string.trim",
        "stdlib",
        r##"
`string.trim(s)` / `s:trim()`

The studio's own: `s` without ASCII whitespace at both ends.

```lua
print("[" .. ("  hi  "):trim() .. "]") -- [hi]
```
"##
    ),
    e!(
        "string.startswith",
        "stdlib",
        r##"
`string.startswith(s, prefix)` / `s:startswith(prefix)`

The studio's own: whether `s` starts with `prefix` (plain text).

```lua
if ("minecraft:stone"):startswith("minecraft:") then print("vanilla") end
```
"##
    ),
    e!(
        "string.endswith",
        "stdlib",
        r##"
`string.endswith(s, suffix)` / `s:endswith(suffix)`

The studio's own: whether `s` ends with `suffix` (plain text).

```lua
if ("deepslate_diamond_ore"):endswith("_ore") then print("an ore") end
```
"##
    ),
    // ------------------------------------------------------------- math
    e!(
        "math.abs",
        "stdlib",
        r##"
`math.abs(x)`

The absolute value.

```lua
print(math.abs(-3.5)) -- 3.5
```
"##
    ),
    e!(
        "math.ceil",
        "stdlib",
        r##"
`math.ceil(x)`

The smallest whole number at or above `x` (a float, printed without a fraction).

```lua
print(math.ceil(2.1), math.ceil(-2.1)) -- 3 -2
```
"##
    ),
    e!(
        "math.floor",
        "stdlib",
        r##"
`math.floor(x)`

The largest whole number at or below `x`. `math.floor(x + 0.5)` rounds.

```lua
print(math.floor(2.7), math.floor(-2.1), math.floor(2.5 + 0.5)) -- 2 -3 3
```
"##
    ),
    e!(
        "math.sqrt",
        "stdlib",
        r##"
`math.sqrt(x)`

The square root (`nan` for negative numbers).

```lua
local function dist(a, b) return math.sqrt((a.x - b.x) ^ 2 + (a.z - b.z) ^ 2) end
```
"##
    ),
    e!(
        "math.sin",
        "stdlib",
        r##"
`math.sin(x)`

The sine of `x` in radians (`math.rad` turns degrees into radians).

```lua
local y = math.sin(math.rad(30)) -- 0.5
```
"##
    ),
    e!(
        "math.cos",
        "stdlib",
        r##"
`math.cos(x)`

The cosine of `x` in radians.

```lua
local a = game.time() * 2
local ox, oz = math.cos(a) * 3, math.sin(a) * 3 -- a circle of radius 3
```
"##
    ),
    e!(
        "math.tan",
        "stdlib",
        r##"
`math.tan(x)`

The tangent of `x` in radians.

```lua
print(math.tan(math.pi / 4)) -- 1
```
"##
    ),
    e!(
        "math.asin",
        "stdlib",
        r##"
`math.asin(x)`

The arc sine, in radians (`nan` outside -1..1).

```lua
print(math.deg(math.asin(0.5))) -- 30
```
"##
    ),
    e!(
        "math.acos",
        "stdlib",
        r##"
`math.acos(x)`

The arc cosine, in radians.

```lua
print(math.deg(math.acos(0))) -- 90
```
"##
    ),
    e!(
        "math.atan",
        "stdlib",
        r##"
`math.atan(y[, x])`

The arc tangent of `y / x` in radians, using both signs for the quadrant (`x` defaults to 1). The yaw that faces from the player towards a point (yaw 0 is south): `math.deg(math.atan(-dx, dz))`.

```lua
local px, _, pz = player.pos()
if px then
  local dx, dz = 100 - px, 50 - pz
  local yaw = math.deg(math.atan(-dx, dz))
  print("Turn to yaw " .. math.floor(yaw))
end
```
"##
    ),
    e!(
        "math.exp",
        "stdlib",
        r##"
`math.exp(x)`

e to the power `x`. Frame-rate independent easing: `v = target + (v - target) * math.exp(-dt * speed)`.

```lua
local v, target = 0, 10
on("frame", function(dt) v = target + (v - target) * math.exp(-dt * 8) end)
```
"##
    ),
    e!(
        "math.log",
        "stdlib",
        r##"
`math.log(x[, base])`

The natural logarithm, or in a `base` (2 and 10 exactly).

```lua
print(math.log(8, 2), math.log(100, 10), math.log(math.exp(1))) -- 3 2 1
```
"##
    ),
    e!(
        "math.fmod",
        "stdlib",
        r##"
`math.fmod(a, b)`

The remainder of `a / b` with the sign of `a` (`%` has the sign of `b`).

```lua
print(math.fmod(-7, 3), -7 % 3) -- -1 2
```
"##
    ),
    e!(
        "math.modf",
        "stdlib",
        r##"
`math.modf(x)`

The whole part (towards zero) and the fraction.

```lua
local whole, frac = math.modf(3.7) -- 3, 0.7
```
"##
    ),
    e!(
        "math.max",
        "stdlib",
        r##"
`math.max(x, ...)`

The largest argument (at least one).

```lua
print(math.max(1, 5, 3)) -- 5
```
"##
    ),
    e!(
        "math.min",
        "stdlib",
        r##"
`math.min(x, ...)`

The smallest argument (at least one).

```lua
print(math.min(4, 2, 8)) -- 2
```
"##
    ),
    e!(
        "math.random",
        "stdlib",
        r##"
`math.random([m[, n]])`

A pseudo-random number: with no argument a float in [0, 1); `math.random(n)` a whole number in 1..n; `math.random(m, n)` in m..n. An empty interval raises "bad argument #N to 'random' (interval is empty)", also for `math.random(0)` (Lua 5.3 gives any integer there). The generator is shared by every mod and starts the same each time the game starts: call `math.randomseed` for other numbers.

```lua
local roll = math.random(6)
local pick = ({ "red", "green", "blue" })[math.random(3)]
```
"##
    ),
    e!(
        "math.randomseed",
        "stdlib",
        r##"
`math.randomseed([x])`

Restarts the random generator from a seed (default 0): the same seed gives the same numbers.

```lua
math.randomseed(os.time())
```
"##
    ),
    e!(
        "math.tointeger",
        "stdlib",
        r##"
`math.tointeger(x)`

`x` when it is a whole number, else nil (strings give nil too). All numbers are floats here, so it only tests wholeness.

```lua
assert(math.tointeger(3.0) == 3 and math.tointeger(3.5) == nil)
```
"##
    ),
    e!(
        "math.type",
        "stdlib",
        r##"
`math.type(x)`

`"integer"` for a whole number (`3.0` too, since every number is a float), `"float"` otherwise, nil for anything that is not a number.

```lua
print(math.type(3), math.type(3.0), math.type(3.5), math.type("3")) -- integer integer float nil
```
"##
    ),
    e!(
        "math.clamp",
        "stdlib",
        r##"
`math.clamp(x, lo, hi)`

The studio's own: `x` kept within `lo` and `hi`.

```lua
local alpha = math.clamp(1.4, 0, 1) -- 1
```
"##
    ),
    e!(
        "math.rad",
        "stdlib",
        r##"
`math.rad(x)`

Degrees to radians: `x * pi / 180` (`math.rad(180)` is 3.1415926535898). Yaw and pitch are in degrees; `math.sin` and `math.cos` take radians.

```lua
local yaw = player.look()
if yaw then
  local a = math.rad(yaw)
  local dx, dz = -math.sin(a), math.cos(a) -- the way the player faces
end
```
"##
    ),
    e!(
        "math.deg",
        "stdlib",
        r##"
`math.deg(x)`

Radians to degrees.

```lua
print(math.deg(math.pi / 2)) -- 90
```
"##
    ),
    e!(
        "math.ult",
        "stdlib",
        r##"
`math.ult(m, n)`

Whether `m < n` when both are read as unsigned 64-bit integers (whole numbers only).

```lua
print(math.ult(1, -1)) -- true
```
"##
    ),
    e!(
        "math.atan2",
        "stdlib",
        r##"
`math.atan2(y, x)`

From Lua 5.1: the same function as `math.atan(y, x)`.

```lua
local angle = math.deg(math.atan2(1, 1)) -- 45
```
"##
    ),
    e!(
        "math.pow",
        "stdlib",
        r##"
`math.pow(x, y)`

From Lua 5.1: `x ^ y`.

```lua
print(math.pow(2, 10)) -- 1024
```
"##
    ),
    e!(
        "math.log10",
        "stdlib",
        r##"
`math.log10(x)`

From Lua 5.1: the base 10 logarithm.

```lua
print(math.log10(1000)) -- 3
```
"##
    ),
    e!(
        "math.sinh",
        "stdlib",
        r##"
`math.sinh(x)`

From Lua 5.1: the hyperbolic sine.

```lua
print(math.sinh(0)) -- 0
```
"##
    ),
    e!(
        "math.cosh",
        "stdlib",
        r##"
`math.cosh(x)`

From Lua 5.1: the hyperbolic cosine.

```lua
print(math.cosh(0)) -- 1
```
"##
    ),
    e!(
        "math.tanh",
        "stdlib",
        r##"
`math.tanh(x)`

From Lua 5.1: the hyperbolic tangent (a smooth step from -1 to 1).

```lua
local s = math.tanh(2)
```
"##
    ),
    e!(
        "math.frexp",
        "stdlib",
        r##"
`math.frexp(x)`

From Lua 5.1: `m, e` with `x = m * 2^e` and 0.5 <= |m| < 1 (0 and non-finite numbers give `x, 0`).

```lua
local m, e = math.frexp(8) -- 0.5, 4
```
"##
    ),
    e!(
        "math.ldexp",
        "stdlib",
        r##"
`math.ldexp(m, e)`

From Lua 5.1: `m * 2^e`.

```lua
print(math.ldexp(0.5, 4)) -- 8
```
"##
    ),
    e!(
        "math.pi",
        "stdlib",
        r##"
`math.pi`

The number pi (prints as 3.1415926535898).

```lua
local full_turn = 2 * math.pi
```
"##
    ),
    e!(
        "math.huge",
        "stdlib",
        r##"
`math.huge`

Infinity (prints as `inf`); `-math.huge` is minus infinity.

```lua
local best = math.huge
for _, d in ipairs({ 5, 2, 9 }) do best = math.min(best, d) end
```
"##
    ),
    e!(
        "math.maxinteger",
        "stdlib",
        r##"
`math.maxinteger`

The largest whole number numbers hold exactly here: 2^53 - 1 = 9007199254740991 (not 2^63 - 1 as in Lua 5.3, since every number is a float).

```lua
print(math.maxinteger == 2 ^ 53 - 1) -- true
```
"##
    ),
    e!(
        "math.mininteger",
        "stdlib",
        r##"
`math.mininteger`

-(2^53 - 1) = -9007199254740991.

```lua
local lowest = math.mininteger
```
"##
    ),
    // ------------------------------------------------------------- table
    e!(
        "table.insert",
        "stdlib",
        r##"
`table.insert(t, [pos,] value)`

Appends `value` at `#t + 1`, or inserts it at `pos` (1 to `#t + 1`), shifting the rest up. A bad position raises "bad argument #2 to 'insert' (position out of bounds)"; other argument counts "wrong number of arguments to 'insert'".

```lua
local list = { "b" }
table.insert(list, "c")
table.insert(list, 1, "a") -- { "a", "b", "c" }
```
"##
    ),
    e!(
        "table.remove",
        "stdlib",
        r##"
`table.remove(t[, pos])`

Removes and returns `t[pos]` (default the last), shifting the rest down. `pos` may be 1 to `#t` (or `#t + 1`); others raise "bad argument #2 to 'remove' (position out of bounds)".

```lua
local queue = { 1, 2, 3 }
local first = table.remove(queue, 1) -- 1; queue is { 2, 3 }
local last = table.remove(queue)     -- 3
```
"##
    ),
    e!(
        "table.concat",
        "stdlib",
        r##"
`table.concat(t[, sep[, i[, j]]])`

The strings and numbers `t[i]..t[j]` (default 1 and `#t`) joined with `sep` (default `""`). Another value raises "invalid value (at index 2) in table for 'concat' (a table value)".

```lua
print(table.concat({ "a", "b", 3 }, ", ")) -- a, b, 3
```
"##
    ),
    e!(
        "table.sort",
        "stdlib",
        r##"
`table.sort(t[, less])`

Sorts `t[1..#t]` in place: with `<`, or with `less(a, b)` returning true when `a` goes first. It is a stable merge sort (equal elements keep their order); a comparator that is not consistent gives some order instead of an error. Values `<` cannot compare raise "attempt to compare ...".

```lua
local players = { { name = "b", score = 3 }, { name = "a", score = 5 } }
table.sort(players, function(x, y) return x.score > y.score end)
```
"##
    ),
    e!(
        "table.unpack",
        "stdlib",
        r##"
`table.unpack(t[, i[, j]])`

`t[i], ..., t[j]` as separate values (default 1 and `#t`). Give `j` to keep nils: `table.unpack(t, 1, t.n)`.

```lua
local x, y, z = table.unpack({ 10, 64, -3 })
```
"##
    ),
    e!(
        "table.pack",
        "stdlib",
        r##"
`table.pack(...)`

A list of the arguments with `n`, their count (nils included).

```lua
local t = table.pack(1, nil, 3)
print(t.n, t[1], t[3]) -- 3 1 3
```
"##
    ),
    e!(
        "table.move",
        "stdlib",
        r##"
`table.move(a1, f, e, t[, a2])`

Copies `a1[f..e]` into `a2` (default `a1`) from position `t`, correctly when the ranges overlap; returns `a2`. At most 16,777,216 elements.

```lua
local copy = table.move({ 1, 2, 3 }, 1, 3, 1, {})
local shifted = table.move({ 1, 2, 3 }, 1, 3, 2) -- { 1, 1, 2, 3 }
```
"##
    ),
    // ------------------------------------------------------------- utf8
    e!(
        "utf8.char",
        "stdlib",
        r##"
`utf8.char(...)`

A string of the given code points as UTF-8. A value that is not a valid code point raises "bad argument #N to 'char' (value out of range)".

```lua
print(utf8.char(72, 228, 8364)) -- Hä€
```
"##
    ),
    e!(
        "utf8.codepoint",
        "stdlib",
        r##"
`utf8.codepoint(s[, i[, j]])`

The code points of the characters that start between bytes `i` and `j` (default `i` 1, `j` = `i`). Positions out of the string raise "bad argument #2 to 'codepoint' (out of range)", bad UTF-8 "invalid UTF-8 code".

```lua
print(utf8.codepoint("ä")) -- 228
```
"##
    ),
    e!(
        "utf8.len",
        "stdlib",
        r##"
`utf8.len(s[, i[, j]])`

The number of UTF-8 characters between bytes `i` and `j` (default 1 and -1); for invalid UTF-8, nil and the position of the first bad byte.

```lua
print(#"héllo", utf8.len("héllo")) -- 6 5
```
"##
    ),
    e!(
        "utf8.offset",
        "stdlib",
        r##"
`utf8.offset(s, n[, i])`

The byte position where the nth character (counting from byte `i`) starts; negative `n` counts back; nil past the end. `n` = 0 finds the start of the character at `i`.

```lua
local s = "häh"
print(s:sub(utf8.offset(s, 3))) -- h (the third character)
```
"##
    ),
    e!(
        "utf8.codes",
        "stdlib",
        r##"
`utf8.codes(s)`

An iterator over the characters: each round gives the byte position and the code point.

```lua
for pos, code in utf8.codes("aä") do print(pos, code) end -- 1 97, 2 228
```
"##
    ),
    e!(
        "utf8.charpattern",
        "stdlib",
        r##"
`utf8.charpattern`

A pattern that matches one UTF-8 character: `"[\0-\x7F\xC2-\xFD][\x80-\xBF]*"`.

```lua
local n = 0
for ch in ("añb"):gmatch(utf8.charpattern) do n = n + 1 end -- 3
```
"##
    ),
    // ------------------------------------------------------------- coroutine
    e!(
        "coroutine.create",
        "stdlib",
        r##"
`coroutine.create(f)`

A new coroutine that will run `f`, suspended until the first `coroutine.resume`. Its type is `"thread"`.

```lua
local co = coroutine.create(function(a) local b = coroutine.yield(a + 1) return b * 2 end)
print(coroutine.resume(co, 1)) -- true 2
print(coroutine.resume(co, 10)) -- true 20
```
"##
    ),
    e!(
        "coroutine.resume",
        "stdlib",
        r##"
`coroutine.resume(co, ...)`

Runs `co` until it yields or ends: returns `true` and the values it yielded or returned, or `false` and the error if it failed (then it is dead). The values given are its function's arguments the first time, then what its `coroutine.yield` returns. A dead or running coroutine gives `false, "cannot resume dead coroutine"` (or `"non-suspended"`).

```lua
local co = coroutine.create(function() error("boom") end)
print(coroutine.resume(co)) -- false  m.lua:1: boom
```
"##
    ),
    e!(
        "coroutine.yield",
        "stdlib",
        r##"
`coroutine.yield(...)`

Suspends the running coroutine: its `coroutine.resume` returns `true` and these values, and the next resume's values are what `yield` returns. Works inside `pcall`; outside a coroutine, or inside a function a built-in calls back (a metamethod, `table.sort`'s comparator), it raises an error.

```lua
local co = coroutine.wrap(function() while true do print("got", coroutine.yield()) end end)
co() co("a") co("b") -- got a, got b
```
"##
    ),
    e!(
        "coroutine.status",
        "stdlib",
        r##"
`coroutine.status(co)`

`"suspended"` (not started, or yielded), `"running"` (it is the one running), `"normal"` (it resumed another, which runs) or `"dead"` (it ended or failed).

```lua
local co = coroutine.create(function() end)
print(coroutine.status(co)) -- suspended
coroutine.resume(co)
print(coroutine.status(co)) -- dead
```
"##
    ),
    e!(
        "coroutine.wrap",
        "stdlib",
        r##"
`coroutine.wrap(f)`

A function that resumes a new coroutine running `f` each time it is called, returning what it yields or returns. An error in it comes out of the call, with where the call was in front. Handy as an iterator.

```lua
local function range(n)
  return coroutine.wrap(function() for i = 1, n do coroutine.yield(i) end end)
end
for i in range(3) do print(i) end
```
"##
    ),
    e!(
        "coroutine.isyieldable",
        "stdlib",
        r##"
`coroutine.isyieldable()`

Whether the running code can yield: inside a coroutine, and not inside a function a built-in called back.

```lua
print(coroutine.isyieldable()) -- false in a mod's main file
```
"##
    ),
    e!(
        "coroutine.running",
        "stdlib",
        r##"
`coroutine.running()`

The running coroutine, and `true` when it is the main one (outside any coroutine).

```lua
local co, main = coroutine.running()
print(type(co), main) -- thread true
```
"##
    ),
    e!(
        "coroutine.close",
        "stdlib",
        r##"
`coroutine.close(co)`

Ends a suspended or dead coroutine for good (Lua 5.4's): returns `true`, or `false` and the error when it had died of one. A running or normal coroutine cannot be closed.

```lua
local co = coroutine.create(function() coroutine.yield() end)
coroutine.resume(co)
print(coroutine.close(co), coroutine.status(co)) -- true dead
```
"##
    ),
    // ------------------------------------------------------------- os
    e!(
        "os.clock",
        "stdlib",
        r##"
`os.clock()`

Seconds since the studio's interpreter started, by the wall clock (not CPU time as in real Lua). Good for measuring how long code takes.

```lua
local t = os.clock()
for i = 1, 100000 do local _ = i * i end
print(("took %.1f ms"):format((os.clock() - t) * 1000))
```
"##
    ),
    e!(
        "os.time",
        "stdlib",
        r##"
`os.time([t])`

The current time in whole seconds since 1970 (UTC). With a table, the time it describes, read as UTC: `year`, `month` and `day` are needed ("field 'day' missing in date table"), `hour` defaults to 12, `min` and `sec` to 0.

```lua
local now = os.time()
local new_year = os.time({ year = 2027, month = 1, day = 1, hour = 0 })
print(("%d days to go"):format((new_year - now) // 86400))
```
"##
    ),
    e!(
        "os.date",
        "stdlib",
        r##"
`os.date([format[, time]])`

A time (default now) as text, always in UTC (a leading `!` is accepted and changes nothing). `"*t"` gives a table `{year, month, day, hour, min, sec, wday (1 is Sunday), yday, isdst = false}`. Formats: `%Y %y %m %d %H %I %M %S %p %A %a %B %b %j %c %x %X %%`; any other raises "bad argument #1 to 'date' (invalid conversion specifier '%Q')". Default format `%c`.

```lua
print(os.date("%Y-%m-%d %H:%M UTC"))
local t = os.date("*t")
if t.hour >= 22 then chat.show("It is late (UTC)") end
```
"##
    ),
    e!(
        "os.difftime",
        "stdlib",
        r##"
`os.difftime(t2[, t1])`

`t2 - t1` in seconds (`t1` defaults to 0).

```lua
local start = os.time()
local elapsed = os.difftime(os.time(), start)
```
"##
    ),
    e!(
        "task.wait",
        "api",
        r##"
`task.wait([seconds])`

Waits without stopping the game: the running thread yields and the scheduler resumes it once `seconds` have passed on the studio's clock (by default, on the next frame). Works at a mod's top level, in a command, in `task.spawn`/`task.defer`/`task.delay` threads and in Roblox signal handlers, which all run in threads of their own.

- `seconds` (number, optional): 0 or more; `nil` waits one frame.
- Returns: the seconds actually waited (a number).
- Where: in a thread that can wait. `on(...)` handlers and timers (`after`, `every`) run on the main thread: there it errors `task.wait can't wait here: ... (run it with task.spawn)`.

```lua
print("start")
task.wait(2)
print("two seconds later")
```
"##
    ),
    e!(
        "task.spawn",
        "api",
        r##"
`task.spawn(fn_or_thread, ...)`

Runs `fn` (or resumes a suspended coroutine) at once, in a thread of its own, with the arguments; the caller carries on when it first waits or ends. An error in it stops the mod, as any error does.

- Returns: the thread.

```lua
task.spawn(function(name)
  for i = 1, 3 do
    print(name, i)
    task.wait(1)
  end
end, "counter")
```
"##
    ),
    e!(
        "task.defer",
        "api",
        r##"
`task.defer(fn_or_thread, ...)`

Like `task.spawn`, but runs once the current code is done (in the same frame), as Roblox's deferred events do.

- Returns: the thread.

```lua
task.defer(print, "after")
print("before")
```
"##
    ),
    e!(
        "task.delay",
        "api",
        r##"
`task.delay(seconds, fn_or_thread, ...)`

Runs `fn` with the arguments in a thread of its own, `seconds` from now.

- Returns: the thread, which `task.cancel` stops.

```lua
local t = task.delay(5, function() chat.show("five seconds") end)
```
"##
    ),
    e!(
        "task.cancel",
        "api",
        r##"
`task.cancel(thread)`

Stops a thread that waits (a `task.delay` not started yet, or one inside `task.wait`). A thread that is running can't be cancelled.

```lua
local t = task.delay(10, print, "never")
task.cancel(t)
```
"##
    ),
    e!(
        "task.synchronize",
        "api",
        r##"
`task.synchronize()`

Does nothing: there is no parallel Luau here, so every script is already synchronized. Kept so Roblox code runs.

```lua
task.synchronize()
```
"##
    ),
    e!(
        "task.desynchronize",
        "api",
        r##"
`task.desynchronize()`

Does nothing (no parallel Luau); kept so Roblox code runs.

```lua
task.desynchronize()
```
"##
    ),
    e!(
        "wait",
        "api",
        r##"
`wait([seconds])`

Roblox's older `task.wait`: waits at least 1/30 of a second and returns the time waited and the studio's clock.

```lua
local waited, now = wait(0.5)
```
"##
    ),
    e!(
        "spawn",
        "api",
        r##"
`spawn(fn)`

Roblox's older `task.defer`: runs `fn(0, clock)` in a thread of its own on the next frame.

```lua
spawn(function() print("later") end)
```
"##
    ),
    e!(
        "delay",
        "api",
        r##"
`delay(seconds, fn)`

Roblox's older `task.delay`: calls `fn(waited, clock)` in a thread of its own after `seconds`.

```lua
delay(1, function(waited) print("waited", waited) end)
```
"##
    ),
    e!(
        "tick",
        "api",
        r##"
`tick()`

Seconds since 1970 (UTC), with a fraction, as Roblox's `tick`.

```lua
local started = tick()
```
"##
    ),
    e!(
        "time",
        "api",
        r##"
`time()`

Seconds since the studio started: the same clock as `game.time()` and `task.wait`.

```lua
print(time())
```
"##
    ),
    e!(
        "warn",
        "api",
        r##"
`warn(...)`

Prints its arguments, joined with spaces, as a warning (in red) in the Studio console, after the mod's id.

```lua
warn("low health:", 3)
```
"##
    ),
    e!(
        "typeof",
        "stdlib",
        r##"
`typeof(value)`

Luau's `typeof`: `type`, but a Roblox value's own type: `Vector3`, `CFrame`, `Instance`, `EnumItem`, `RBXScriptSignal`...

```lua
print(typeof(Vector3.new()), typeof(workspace), typeof(1))
```
"##
    ),
    e!(
        "table.find",
        "stdlib",
        r##"
`table.find(t, value[, init])`

the first index from `init` (1) whose item is `value` (raw equality), or nil.

```lua
print(table.find({"a", "b"}, "b")) -- 2
```
"##
    ),
    e!(
        "table.create",
        "stdlib",
        r##"
`table.create(n[, value])`

a list of `n` items, each `value` (empty when nil).

```lua
local zeros = table.create(10, 0)
```
"##
    ),
    e!(
        "table.clear",
        "stdlib",
        r##"
`table.clear(t)`

removes every key, keeping the metatable.

```lua
local t = {1, 2}
table.clear(t)
```
"##
    ),
    e!(
        "table.clone",
        "stdlib",
        r##"
`table.clone(t)`

a shallow copy with the same metatable (not frozen).

```lua
local copy = table.clone({x = 1})
```
"##
    ),
    e!(
        "table.freeze",
        "stdlib",
        r##"
`table.freeze(t)`

makes `t` read only (writes error `attempt to modify a readonly table`); returns it.

```lua
local CONFIG = table.freeze({speed = 2})
```
"##
    ),
    e!(
        "table.isfrozen",
        "stdlib",
        r##"
`table.isfrozen(t)`

whether `table.freeze` froze it.

```lua
print(table.isfrozen({}))
```
"##
    ),
    e!(
        "table.maxn",
        "stdlib",
        r##"
`table.maxn(t)`

the largest number key (0 without one).

```lua
print(table.maxn({[3] = 1}))
```
"##
    ),
    e!(
        "table.getn",
        "stdlib",
        r##"
`table.getn(t)`

Lua 5.0's `#t`.

```lua
print(table.getn({1, 2}))
```
"##
    ),
    e!(
        "table.foreach",
        "stdlib",
        r##"
`table.foreach(t, fn)`

calls `fn(k, v)` for each pair until one returns a value, which it returns.

```lua
table.foreach({a = 1}, print)
```
"##
    ),
    e!(
        "table.foreachi",
        "stdlib",
        r##"
`table.foreachi(t, fn)`

calls `fn(i, v)` for each list item until one returns a value.

```lua
table.foreachi({"x"}, print)
```
"##
    ),
    e!(
        "math.sign",
        "stdlib",
        r##"
`math.sign(x)`

-1, 0 or 1.

```lua
print(math.sign(-4))
```
"##
    ),
    e!(
        "math.round",
        "stdlib",
        r##"
`math.round(x)`

the nearest whole number, halves away from zero.

```lua
print(math.round(2.5)) -- 3
```
"##
    ),
    e!(
        "math.noise",
        "stdlib",
        r##"
`math.noise(x[, y[, z]])`

Perlin noise, about -1 to 1, 0 at whole numbers.

```lua
local h = math.noise(x or 0.5, 0.3) * 10
```
"##
    ),
    e!(
        "math.lerp",
        "stdlib",
        r##"
`math.lerp(a, b, t)`

`a + (b - a) * t`, exactly `b` at `t` 1.

```lua
print(math.lerp(0, 10, 0.5))
```
"##
    ),
    e!(
        "math.map",
        "stdlib",
        r##"
`math.map(x, inmin, inmax, outmin, outmax)`

`x` moved from one range to another.

```lua
print(math.map(5, 0, 10, 0, 100)) -- 50
```
"##
    ),
    e!(
        "math.isnan",
        "stdlib",
        r##"
`math.isnan(x)`

whether `x` is NaN.

```lua
print(math.isnan(0/0))
```
"##
    ),
    e!(
        "math.isinf",
        "stdlib",
        r##"
`math.isinf(x)`

whether `x` is infinite.

```lua
print(math.isinf(math.huge))
```
"##
    ),
    e!(
        "math.isfinite",
        "stdlib",
        r##"
`math.isfinite(x)`

neither NaN nor infinite.

```lua
print(math.isfinite(1))
```
"##
    ),
    e!(
        "buffer.create",
        "stdlib",
        r##"
`buffer.create(size)`

Luau's buffer: a new buffer of `size` bytes (at most 1 GiB), all zero. `type` and `typeof` of one say `"buffer"`; offsets count from 0, and reading or writing past its end errors `buffer access out of bounds`.

```lua
local b = buffer.create(8)
```
"##
    ),
    e!(
        "buffer.fromstring",
        "stdlib",
        r##"
`buffer.fromstring(s)`

A buffer holding the bytes of `s`.

```lua
local b = buffer.fromstring("hi")
```
"##
    ),
    e!(
        "buffer.tostring",
        "stdlib",
        r##"
`buffer.tostring(b)`

The buffer's bytes as a string.

```lua
print(buffer.tostring(buffer.fromstring("hi")))
```
"##
    ),
    e!(
        "buffer.len",
        "stdlib",
        r##"
`buffer.len(b)`

The buffer's size in bytes.

```lua
print(buffer.len(buffer.create(4)))
```
"##
    ),
    e!(
        "buffer.readi8",
        "stdlib",
        r##"
`buffer.readi8(b, offset)`

Reads a signed byte (little-endian) at byte `offset`.

```lua
print(buffer.readi8(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writei8",
        "stdlib",
        r##"
`buffer.writei8(b, offset, value)`

Writes `value` as a signed byte (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writei8(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readu8",
        "stdlib",
        r##"
`buffer.readu8(b, offset)`

Reads an unsigned byte (little-endian) at byte `offset`.

```lua
print(buffer.readu8(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writeu8",
        "stdlib",
        r##"
`buffer.writeu8(b, offset, value)`

Writes `value` as an unsigned byte (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writeu8(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readi16",
        "stdlib",
        r##"
`buffer.readi16(b, offset)`

Reads a signed 16-bit integer (little-endian) at byte `offset`.

```lua
print(buffer.readi16(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writei16",
        "stdlib",
        r##"
`buffer.writei16(b, offset, value)`

Writes `value` as a signed 16-bit integer (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writei16(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readu16",
        "stdlib",
        r##"
`buffer.readu16(b, offset)`

Reads an unsigned 16-bit integer (little-endian) at byte `offset`.

```lua
print(buffer.readu16(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writeu16",
        "stdlib",
        r##"
`buffer.writeu16(b, offset, value)`

Writes `value` as an unsigned 16-bit integer (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writeu16(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readi32",
        "stdlib",
        r##"
`buffer.readi32(b, offset)`

Reads a signed 32-bit integer (little-endian) at byte `offset`.

```lua
print(buffer.readi32(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writei32",
        "stdlib",
        r##"
`buffer.writei32(b, offset, value)`

Writes `value` as a signed 32-bit integer (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writei32(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readu32",
        "stdlib",
        r##"
`buffer.readu32(b, offset)`

Reads an unsigned 32-bit integer (little-endian) at byte `offset`.

```lua
print(buffer.readu32(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writeu32",
        "stdlib",
        r##"
`buffer.writeu32(b, offset, value)`

Writes `value` as an unsigned 32-bit integer (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writeu32(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readf32",
        "stdlib",
        r##"
`buffer.readf32(b, offset)`

Reads a 32-bit float (little-endian) at byte `offset`.

```lua
print(buffer.readf32(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writef32",
        "stdlib",
        r##"
`buffer.writef32(b, offset, value)`

Writes `value` as a 32-bit float (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writef32(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readf64",
        "stdlib",
        r##"
`buffer.readf64(b, offset)`

Reads a 64-bit float (little-endian) at byte `offset`.

```lua
print(buffer.readf64(buffer.create(8), 0))
```
"##
    ),
    e!(
        "buffer.writef64",
        "stdlib",
        r##"
`buffer.writef64(b, offset, value)`

Writes `value` as a 64-bit float (little-endian) at byte `offset`; whole numbers wrap to its range.

```lua
local b = buffer.create(8) buffer.writef64(b, 0, 7)
```
"##
    ),
    e!(
        "buffer.readstring",
        "stdlib",
        r##"
`buffer.readstring(b, offset, count)`

`count` bytes at `offset`, as a string.

```lua
print(buffer.readstring(buffer.fromstring("hello"), 1, 3))
```
"##
    ),
    e!(
        "buffer.writestring",
        "stdlib",
        r##"
`buffer.writestring(b, offset, s[, count])`

Writes `s` (its first `count` bytes) at `offset`.

```lua
local b = buffer.create(5) buffer.writestring(b, 0, "hello")
```
"##
    ),
    e!(
        "buffer.copy",
        "stdlib",
        r##"
`buffer.copy(target, targetOffset, source[, sourceOffset[, count]])`

Copies `count` bytes of `source` from `sourceOffset` (0) into `target` at `targetOffset`; the two may be the same buffer.

```lua
buffer.copy(a, 0, b)
```
"##
    ),
    e!(
        "buffer.fill",
        "stdlib",
        r##"
`buffer.fill(b, offset, value[, count])`

Sets `count` bytes from `offset` (to the end) to the byte `value`.

```lua
buffer.fill(b, 0, 255)
```
"##
    ),
    e!(
        "buffer.readbits",
        "stdlib",
        r##"
`buffer.readbits(b, bitOffset, bitCount)`

Reads `bitCount` bits (0 to 32) from bit `bitOffset`, the lowest bit first, as a number.

```lua
print(buffer.readbits(b, 3, 5))
```
"##
    ),
    e!(
        "buffer.writebits",
        "stdlib",
        r##"
`buffer.writebits(b, bitOffset, bitCount, value)`

Writes the low `bitCount` bits (0 to 32) of `value` from bit `bitOffset`.

```lua
buffer.writebits(b, 3, 5, 17)
```
"##
    ),
    e!(
        "string.pack",
        "stdlib",
        r##"
`string.pack(fmt, ...)`

Packs values into a binary string as Lua 5.3's format describes: `<` `>` `=` byte order, `b` `B` `h` `H` `i[n]` `I[n]` `l` `L` `j` `J` `T` integers, `f` `d` `n` floats, `s[n]` length-prefixed strings, `c<n>` fixed strings, `z` zero-ended strings, `x` a zero byte, `X` alignment.

```lua
local data = string.pack("<i4f", 7, 1.5)
```
"##
    ),
    e!(
        "string.unpack",
        "stdlib",
        r##"
`string.unpack(fmt, s[, pos])`

The values `string.pack` would have packed into `s` (from byte `pos`, 1), then the position after them.

```lua
print(string.unpack("<i4", string.pack("<i4", 7)))
```
"##
    ),
    e!(
        "string.packsize",
        "stdlib",
        r##"
`string.packsize(fmt)`

The size of what `fmt` packs (it must have no variable-size items).

```lua
print(string.packsize("i4i4"))
```
"##
    ),
    e!(
        "gcinfo",
        "stdlib",
        r##"
`gcinfo()`

Luau's memory in use, in kilobytes: here an estimate.

```lua
print(gcinfo())
```
"##
    ),
    e!(
        "utf8.graphemes",
        "stdlib",
        r##"
`utf8.graphemes(s[, i[, j]])`

Roblox's: an iterator over the grapheme clusters of `s` (a character with the accents, variation selectors and zero-width joins after it), giving each one's first and last byte.

```lua
for first, last in utf8.graphemes("hé") do print(first, last) end
```
"##
    ),
    e!(
        "utf8.nfcnormalize",
        "stdlib",
        r##"
`utf8.nfcnormalize(s)`

Roblox's Unicode composition: here the text as it is.

```lua
print(utf8.nfcnormalize("é"))
```
"##
    ),
    e!(
        "utf8.nfdnormalize",
        "stdlib",
        r##"
`utf8.nfdnormalize(s)`

Roblox's Unicode decomposition: here the text as it is.

```lua
print(utf8.nfdnormalize("é"))
```
"##
    ),
    e!(
        "vector.create",
        "stdlib",
        r##"
`vector.create(x, y[, z])`

Luau's vector library; in Roblox a vector is a Vector3: this is `Vector3.new(x, y, z)`.

```lua
local v = vector.create(1, 2, 3)
```
"##
    ),
    e!(
        "vector.magnitude",
        "stdlib",
        r##"
`vector.magnitude(v)`

The vector's length.

```lua
print(vector.magnitude(vector.create(3, 4, 0)))
```
"##
    ),
    e!(
        "vector.normalize",
        "stdlib",
        r##"
`vector.normalize(v)`

The vector at length 1 (its Unit).

```lua
print(vector.normalize(vector.create(0, 5, 0)))
```
"##
    ),
    e!(
        "vector.dot",
        "stdlib",
        r##"
`vector.dot(a, b)`

The dot product.

```lua
print(vector.dot(vector.one, vector.one))
```
"##
    ),
    e!(
        "vector.cross",
        "stdlib",
        r##"
`vector.cross(a, b)`

The cross product.

```lua
print(vector.cross(Vector3.xAxis, Vector3.yAxis))
```
"##
    ),
    e!(
        "vector.angle",
        "stdlib",
        r##"
`vector.angle(a, b[, axis])`

The angle between two vectors in radians, negative when `axis` says it turns the other way.

```lua
print(vector.angle(Vector3.xAxis, Vector3.yAxis))
```
"##
    ),
    e!(
        "vector.floor",
        "stdlib",
        r##"
`vector.floor(v)`

Each component rounded down.

```lua
print(vector.floor(vector.create(1.5, 2.5, -0.5)))
```
"##
    ),
    e!(
        "vector.ceil",
        "stdlib",
        r##"
`vector.ceil(v)`

Each component rounded up.

```lua
print(vector.ceil(vector.create(1.5, 2.5, -0.5)))
```
"##
    ),
    e!(
        "vector.abs",
        "stdlib",
        r##"
`vector.abs(v)`

Each component's absolute value.

```lua
print(vector.abs(vector.create(-1, 2, -3)))
```
"##
    ),
    e!(
        "vector.sign",
        "stdlib",
        r##"
`vector.sign(v)`

Each component's sign (-1, 0 or 1).

```lua
print(vector.sign(vector.create(-4, 0, 9)))
```
"##
    ),
    e!(
        "vector.clamp",
        "stdlib",
        r##"
`vector.clamp(v, min, max)`

Each component kept between those of `min` and `max`.

```lua
print(vector.clamp(v, vector.zero, vector.one))
```
"##
    ),
    e!(
        "vector.max",
        "stdlib",
        r##"
`vector.max(v, ...)`

The largest of each component.

```lua
print(vector.max(a, b))
```
"##
    ),
    e!(
        "vector.min",
        "stdlib",
        r##"
`vector.min(v, ...)`

The smallest of each component.

```lua
print(vector.min(a, b))
```
"##
    ),
    e!(
        "bit32.band",
        "stdlib",
        r##"
`bit32.band(...)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the bits set in all of them (all ones with none).

```lua
print(bit32.band(0xF0, 0x3C))
```
"##
    ),
    e!(
        "bit32.bor",
        "stdlib",
        r##"
`bit32.bor(...)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the bits set in any of them.

```lua
print(bit32.bor(0xF0, 0x3C))
```
"##
    ),
    e!(
        "bit32.bxor",
        "stdlib",
        r##"
`bit32.bxor(...)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the bits set in an odd number of them.

```lua
print(bit32.bxor(0xF0, 0x3C))
```
"##
    ),
    e!(
        "bit32.btest",
        "stdlib",
        r##"
`bit32.btest(...)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): whether `band` of them is not 0.

```lua
print(bit32.btest(0xF0, 0x3C))
```
"##
    ),
    e!(
        "bit32.bnot",
        "stdlib",
        r##"
`bit32.bnot(x)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): every bit flipped.

```lua
print(bit32.bnot(6))
```
"##
    ),
    e!(
        "bit32.lshift",
        "stdlib",
        r##"
`bit32.lshift(x, n)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` shifted left by `n` (right when negative); 32 or more gives 0.

```lua
print(bit32.lshift(6, 1))
```
"##
    ),
    e!(
        "bit32.rshift",
        "stdlib",
        r##"
`bit32.rshift(x, n)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` shifted right by `n`, zeros coming in.

```lua
print(bit32.rshift(6, 1))
```
"##
    ),
    e!(
        "bit32.arshift",
        "stdlib",
        r##"
`bit32.arshift(x, n)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` shifted right by `n`, the sign bit coming in.

```lua
print(bit32.arshift(6, 1))
```
"##
    ),
    e!(
        "bit32.lrotate",
        "stdlib",
        r##"
`bit32.lrotate(x, n)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` rotated left by `n`.

```lua
print(bit32.lrotate(6, 1))
```
"##
    ),
    e!(
        "bit32.rrotate",
        "stdlib",
        r##"
`bit32.rrotate(x, n)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` rotated right by `n`.

```lua
print(bit32.rrotate(6, 1))
```
"##
    ),
    e!(
        "bit32.extract",
        "stdlib",
        r##"
`bit32.extract(x, field[, width])`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `width` (1) bits of `x` from bit `field`.

```lua
print(bit32.extract(0xABCD, 4, 8))
```
"##
    ),
    e!(
        "bit32.replace",
        "stdlib",
        r##"
`bit32.replace(x, v, field[, width])`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): `x` with `width` (1) bits from `field` set to `v`'s.

```lua
print(bit32.replace(0, 0xF, 8, 4))
```
"##
    ),
    e!(
        "bit32.countlz",
        "stdlib",
        r##"
`bit32.countlz(x)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the zero bits above the highest one (32 for 0).

```lua
print(bit32.countlz(6))
```
"##
    ),
    e!(
        "bit32.countrz",
        "stdlib",
        r##"
`bit32.countrz(x)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the zero bits below the lowest one (32 for 0).

```lua
print(bit32.countrz(6))
```
"##
    ),
    e!(
        "bit32.byteswap",
        "stdlib",
        r##"
`bit32.byteswap(x)`

Luau's bit32, on 32-bit unsigned whole numbers (arguments are taken modulo 2^32): the four bytes in reverse order.

```lua
print(bit32.byteswap(6))
```
"##
    ),
    e!(
        "Vector3.new",
        "roblox",
        r##"
`Vector3.new([x, y, z])`

A 3D vector (missing parts are 0). Fields `X`, `Y`, `Z` (also lowercase), `Magnitude`, `Unit`; methods `Lerp`, `Dot`, `Cross`, `FuzzyEq`, `Abs`, `Ceil`, `Floor`, `Sign`, `Min`, `Max`, `Angle`; `+ - * / //` with vectors and numbers, unary `-`, `==`. `Vector3.zero`, `one`, `xAxis`, `yAxis`, `zAxis`. A stud is a block.

```lua
local up = Vector3.new(0, 1, 0)
print((up * 2 + Vector3.one).Magnitude)
```
"##
    ),
    e!(
        "Vector3.FromNormalId",
        "roblox",
        r##"
`Vector3.FromNormalId(normal)`

The unit vector of an `Enum.NormalId` face.

```lua
print(Vector3.FromNormalId(Enum.NormalId.Top))
```
"##
    ),
    e!(
        "Vector3.FromAxis",
        "roblox",
        r##"
`Vector3.FromAxis(axis)`

The unit vector of an `Enum.Axis`.

```lua
print(Vector3.FromAxis(Enum.Axis.X))
```
"##
    ),
    e!(
        "Vector2.new",
        "roblox",
        r##"
`Vector2.new([x, y])`

A 2D vector: `X`, `Y`, `Magnitude`, `Unit`, the same methods and operators as Vector3 (`Cross` gives a number). `Vector2.zero`, `one`, `xAxis`, `yAxis`. GUI positions are Vector2s in dp.

```lua
local center = Vector2.new(480, 270)
```
"##
    ),
    e!(
        "CFrame.new",
        "roblox",
        r##"
`CFrame.new([x, y, z] | pos[, lookAt] | x, y, z, qx, qy, qz, qw | 12 numbers)`

A position and rotation. Fields `Position`, `X`, `Y`, `Z`, `LookVector`, `RightVector`, `UpVector`, `XVector`, `YVector`, `ZVector`, `Rotation`; methods `Inverse`, `Lerp`, `ToWorldSpace`, `ToObjectSpace`, `PointToWorldSpace`, `PointToObjectSpace`, `VectorToWorldSpace`, `VectorToObjectSpace`, `GetComponents`, `ToEulerAnglesXYZ`, `ToEulerAnglesYXZ`, `ToOrientation`, `ToAxisAngle`, `Orthonormalize`, `FuzzyEq`. `cf * cf`, `cf * vector`, `cf + vector`, `cf - vector`. `CFrame.identity`.

```lua
local cf = CFrame.new(0, 5, 0) * CFrame.Angles(0, math.rad(90), 0)
print(cf.LookVector)
```
"##
    ),
    e!(
        "CFrame.lookAt",
        "roblox",
        r##"
`CFrame.lookAt(at, target[, up])`

A CFrame at `at` looking at `target` (-Z forward, as in Roblox).

```lua
local cam = CFrame.lookAt(Vector3.new(0, 10, 10), Vector3.zero)
```
"##
    ),
    e!(
        "CFrame.lookAlong",
        "roblox",
        r##"
`CFrame.lookAlong(at, direction[, up])`

A CFrame at `at` looking along `direction`.

```lua
local cf = CFrame.lookAlong(Vector3.zero, Vector3.xAxis)
```
"##
    ),
    e!(
        "CFrame.Angles",
        "roblox",
        r##"
`CFrame.Angles(rx, ry, rz)`

A rotation by radians about X, then Y, then Z (as `fromEulerAnglesXYZ`).

```lua
local turn = CFrame.Angles(0, math.pi, 0)
```
"##
    ),
    e!(
        "CFrame.fromEulerAnglesXYZ",
        "roblox",
        r##"
`CFrame.fromEulerAnglesXYZ(rx, ry, rz)`

The same as `CFrame.Angles`.

```lua
local r = CFrame.fromEulerAnglesXYZ(0, 1, 0)
```
"##
    ),
    e!(
        "CFrame.fromEulerAnglesYXZ",
        "roblox",
        r##"
`CFrame.fromEulerAnglesYXZ(rx, ry, rz)`

A rotation in Y, X, Z order.

```lua
local r = CFrame.fromEulerAnglesYXZ(0.2, 1, 0)
```
"##
    ),
    e!(
        "CFrame.fromOrientation",
        "roblox",
        r##"
`CFrame.fromOrientation(rx, ry, rz)`

A rotation as a part's Orientation gives it (YXZ order), in radians.

```lua
local r = CFrame.fromOrientation(0, math.rad(45), 0)
```
"##
    ),
    e!(
        "CFrame.fromEulerAngles",
        "roblox",
        r##"
`CFrame.fromEulerAngles(rx, ry, rz[, order])`

A rotation in an `Enum.RotationOrder` (XYZ by default).

```lua
local r = CFrame.fromEulerAngles(0, 1, 0, Enum.RotationOrder.YXZ)
```
"##
    ),
    e!(
        "CFrame.fromAxisAngle",
        "roblox",
        r##"
`CFrame.fromAxisAngle(axis, angle)`

A rotation of `angle` radians about `axis`.

```lua
local r = CFrame.fromAxisAngle(Vector3.yAxis, math.pi / 2)
```
"##
    ),
    e!(
        "CFrame.fromMatrix",
        "roblox",
        r##"
`CFrame.fromMatrix(pos, vx, vy[, vz])`

A CFrame from its position and its right, up (and back) vectors.

```lua
local cf = CFrame.fromMatrix(Vector3.zero, Vector3.xAxis, Vector3.yAxis)
```
"##
    ),
    e!(
        "CFrame.fromRotationBetweenVectors",
        "roblox",
        r##"
`CFrame.fromRotationBetweenVectors(from, to)`

The smallest rotation turning `from` into `to`.

```lua
local r = CFrame.fromRotationBetweenVectors(Vector3.xAxis, Vector3.yAxis)
```
"##
    ),
    e!(
        "Color3.new",
        "roblox",
        r##"
`Color3.new([r, g, b])`

A colour, each part 0 to 1. Fields `R`, `G`, `B`; methods `Lerp`, `ToHSV`, `ToHex`.

```lua
local red = Color3.new(1, 0, 0)
```
"##
    ),
    e!(
        "Color3.fromRGB",
        "roblox",
        r##"
`Color3.fromRGB(r, g, b)`

A colour from 0 to 255 parts.

```lua
local dark = Color3.fromRGB(30, 30, 30)
```
"##
    ),
    e!(
        "Color3.fromHSV",
        "roblox",
        r##"
`Color3.fromHSV(h, s, v)`

A colour from hue, saturation and value, each 0 to 1.

```lua
local c = Color3.fromHSV(0.6, 0.8, 1)
```
"##
    ),
    e!(
        "Color3.fromHex",
        "roblox",
        r##"
`Color3.fromHex(hex)`

A colour from `"#rrggbb"` or `"rgb"` text.

```lua
local c = Color3.fromHex("#ff8800")
```
"##
    ),
    e!(
        "Color3.toHSV",
        "roblox",
        r##"
`Color3.toHSV(color)`

Hue, saturation and value of a colour.

```lua
local h, s, v = Color3.toHSV(Color3.new(1, 0, 0))
```
"##
    ),
    e!(
        "UDim.new",
        "roblox",
        r##"
`UDim.new([scale, offset])`

One GUI length: a fraction of the parent plus pixels (dp). Fields `Scale`, `Offset`; `+` and `-`.

```lua
local radius = UDim.new(0, 8)
```
"##
    ),
    e!(
        "UDim2.new",
        "roblox",
        r##"
`UDim2.new([xScale, xOffset, yScale, yOffset] | x, y)`

A GUI position or size: `X` and `Y` UDims (also `Width`, `Height`); `Lerp`; `+` and `-`.

```lua
local size = UDim2.new(0.5, 0, 0, 40)
```
"##
    ),
    e!(
        "UDim2.fromScale",
        "roblox",
        r##"
`UDim2.fromScale(x, y)`

A UDim2 of fractions only.

```lua
local full = UDim2.fromScale(1, 1)
```
"##
    ),
    e!(
        "UDim2.fromOffset",
        "roblox",
        r##"
`UDim2.fromOffset(x, y)`

A UDim2 of pixels (dp) only.

```lua
local box = UDim2.fromOffset(200, 100)
```
"##
    ),
    e!(
        "Rect.new",
        "roblox",
        r##"
`Rect.new(min, max | minX, minY, maxX, maxY)`

A rectangle: `Min`, `Max`, `Width`, `Height`.

```lua
local r = Rect.new(0, 0, 10, 10)
```
"##
    ),
    e!(
        "NumberRange.new",
        "roblox",
        r##"
`NumberRange.new(min[, max])`

A range: `Min`, `Max`.

```lua
local r = NumberRange.new(1, 5)
```
"##
    ),
    e!(
        "saveinstance",
        "executor",
        r##"
`saveinstance([object | file, object | options])`

Saves an Instance and everything in it as a Roblox XML model (`.rbxmx`)
in the workspace folder, or the whole place (`.rbxlx`: its services with
what is in them) when given none or `game`, and returns the file's name.
Studio opens both. Options: `FileName`, `Object`.

```lua
saveinstance(workspace.Model)                 -- Model.rbxmx
saveinstance({ FileName = "my place" })       -- my place.rbxlx
```
"##
    ),
    e!(
        "SaveInstance",
        "executor",
        r##"
`SaveInstance(object, file)`

Elysian's name for `saveinstance(file, object)`: the object as an XML
model in the workspace folder.

```lua
SaveInstance(workspace.Model, "model")       -- model.rbxmx
```
"##
    ),
    e!(
        "decompile",
        "executor",
        r##"
`decompile(script | function | bytecode)`

The code, as text. Every function compiled here keeps its source, so
nothing is guessed from bytecode:
- a script: its Source, the file of a mod it runs, or the code a running
  script runs;
- a function (also one wrapped by `newcclosure`, or from `loadstring`,
  `getscriptclosure`, `getgc`): its own text, from `function` (or
  `local function`) through its `end`; a built-in gives a comment saying so;
- what `getscriptbytecode` gave: the source those bytes carry (bytes without
  one give their listing as comments).

An Instance that isn't a script gives a comment saying it has no code.

```lua
local function add(a, b) return a + b end
print(decompile(add))         --> local function add(a, b) return a + b end
print(decompile(game.Players.LocalPlayer.Character.Animate))
print(decompile(getscriptbytecode(script)) == decompile(script)) --> true
```
"##
    ),
    e!(
        "PhysicalProperties.new",
        "roblox",
        r##"
`PhysicalProperties.new(density, friction, elasticity[, frictionWeight, elasticityWeight] | material)`

A part's physics (`CustomPhysicalProperties`): `Density`, `Friction`,
`Elasticity`, `FrictionWeight`, `ElasticityWeight`. Given an
`Enum.Material`, that material's own.

```lua
part.CustomPhysicalProperties = PhysicalProperties.new(0.7, 0.3, 0.5)
local wood = PhysicalProperties.new(Enum.Material.Wood)
```
"##
    ),
    e!(
        "ypcall",
        "roblox",
        r##"
`ypcall(f, ...)`

Roblox's older name for `pcall` (which can yield): the same function.

```lua
local ok, err = ypcall(function() error("no") end)
```
"##
    ),
    e!(
        "settings",
        "roblox",
        r##"
`settings()`

Roblox's GlobalSettings: `Rendering`, `Physics`, `Network`, `Diagnostics`,
`Game`, `Lua` and `TaskScheduler`, each a settings object scripts read and
set (`settings():GetService("RenderSettings")` too).

```lua
settings().Rendering.QualityLevel = Enum.QualityLevel.Level01
```
"##
    ),
    e!(
        "UserSettings",
        "roblox",
        r##"
`UserSettings()`

Roblox's UserSettings: its `UserGameSettings` (`MasterVolume`,
`MouseSensitivity`, `SavedQualityLevel` and the rest).

```lua
local game_settings = UserSettings():GetService("UserGameSettings")
print(game_settings.MasterVolume)
```
"##
    ),
    e!(
        "NumberSequence.new",
        "roblox",
        r##"
`NumberSequence.new(n | n0, n1 | keypoints)`

Numbers over time 0 to 1: `Keypoints`.

```lua
local fade = NumberSequence.new(0, 1)
```
"##
    ),
    e!(
        "NumberSequenceKeypoint.new",
        "roblox",
        r##"
`NumberSequenceKeypoint.new(time, value[, envelope])`

One keypoint: `Time`, `Value`, `Envelope`.

```lua
local k = NumberSequenceKeypoint.new(0.5, 1)
```
"##
    ),
    e!(
        "ColorSequence.new",
        "roblox",
        r##"
`ColorSequence.new(color | c0, c1 | keypoints)`

Colours over time 0 to 1 (UIGradient.Color): `Keypoints`.

```lua
local g = ColorSequence.new(Color3.new(1, 0, 0), Color3.new(0, 0, 1))
```
"##
    ),
    e!(
        "ColorSequenceKeypoint.new",
        "roblox",
        r##"
`ColorSequenceKeypoint.new(time, color)`

One keypoint: `Time`, `Value`.

```lua
local k = ColorSequenceKeypoint.new(0, Color3.new(1, 1, 1))
```
"##
    ),
    e!(
        "TweenInfo.new",
        "roblox",
        r##"
`TweenInfo.new([time, style, direction, repeatCount, reverses, delayTime])`

How a tween runs: 1 second of Quad Out by default. Fields of the same names. `repeatCount` -1 repeats for ever.

```lua
local info = TweenInfo.new(0.5, Enum.EasingStyle.Sine, Enum.EasingDirection.Out)
```
"##
    ),
    e!(
        "BrickColor.new",
        "roblox",
        r##"
`BrickColor.new(name | number | r, g, b | color)`

One of Roblox's named colours: `Name`, `Number`, `Color`, `r`, `g`, `b`. A part's `BrickColor` sets its `Color`.

```lua
local red = BrickColor.new("Bright red")
```
"##
    ),
    e!(
        "BrickColor.palette",
        "roblox",
        r##"
`BrickColor.palette(i)`

The palette's colour number `i` (0 to 127).

```lua
local c = BrickColor.palette(5)
```
"##
    ),
    e!(
        "BrickColor.random",
        "roblox",
        r##"
`BrickColor.random()`

A random named colour.

```lua
local c = BrickColor.random()
```
"##
    ),
    e!(
        "BrickColor.White",
        "roblox",
        r##"
`BrickColor.White()`

The named colour White.

```lua
local c = BrickColor.White()
```
"##
    ),
    e!(
        "BrickColor.Gray",
        "roblox",
        r##"
`BrickColor.Gray()`

The named colour Gray.

```lua
local c = BrickColor.Gray()
```
"##
    ),
    e!(
        "BrickColor.DarkGray",
        "roblox",
        r##"
`BrickColor.DarkGray()`

The named colour DarkGray.

```lua
local c = BrickColor.DarkGray()
```
"##
    ),
    e!(
        "BrickColor.Black",
        "roblox",
        r##"
`BrickColor.Black()`

The named colour Black.

```lua
local c = BrickColor.Black()
```
"##
    ),
    e!(
        "BrickColor.Red",
        "roblox",
        r##"
`BrickColor.Red()`

The named colour Red.

```lua
local c = BrickColor.Red()
```
"##
    ),
    e!(
        "BrickColor.Yellow",
        "roblox",
        r##"
`BrickColor.Yellow()`

The named colour Yellow.

```lua
local c = BrickColor.Yellow()
```
"##
    ),
    e!(
        "BrickColor.Green",
        "roblox",
        r##"
`BrickColor.Green()`

The named colour Green.

```lua
local c = BrickColor.Green()
```
"##
    ),
    e!(
        "BrickColor.Blue",
        "roblox",
        r##"
`BrickColor.Blue()`

The named colour Blue.

```lua
local c = BrickColor.Blue()
```
"##
    ),
    e!(
        "Font.new",
        "roblox",
        r##"
`Font.new(family[, weight, style])`

A font face (TextLabel.FontFace): `Family`, `Weight`, `Style`, `Bold`. Drawn with Geist: a mono family as Geist Mono, Medium weight or heavier as Geist Medium.

```lua
local f = Font.new("rbxasset://fonts/families/GothamSSm.json", Enum.FontWeight.Bold)
```
"##
    ),
    e!(
        "Font.fromEnum",
        "roblox",
        r##"
`Font.fromEnum(font)`

The face of an `Enum.Font`.

```lua
local f = Font.fromEnum(Enum.Font.GothamBold)
```
"##
    ),
    e!(
        "Font.fromName",
        "roblox",
        r##"
`Font.fromName(name[, weight, style])`

A face of a family by its name.

```lua
local f = Font.fromName("RobotoMono")
```
"##
    ),
    e!(
        "Font.fromId",
        "roblox",
        r##"
`Font.fromId(assetId[, weight, style])`

A face of a family asset (kept; drawn with Geist).

```lua
local f = Font.fromId(12187365364)
```
"##
    ),
    e!(
        "Random.new",
        "roblox",
        r##"
`Random.new([seed])`

A random generator (Luau's PCG32): `NextInteger(min, max)`, `NextNumber([min, max])`, `NextUnitVector()`, `Shuffle(t)`, `Clone()`.

```lua
local rng = Random.new(42)
print(rng:NextInteger(1, 6))
```
"##
    ),
    e!(
        "Ray.new",
        "roblox",
        r##"
`Ray.new(origin, direction)`

A ray: `Origin`, `Direction`, `Unit`, `ClosestPoint(p)`, `Distance(p)`.

```lua
local r = Ray.new(Vector3.zero, Vector3.new(0, 0, -10))
```
"##
    ),
    e!(
        "Region3.new",
        "roblox",
        r##"
`Region3.new(min, max)`

A box in the world by its corners, not turned (the least and the most of the two given): `CFrame` (its centre), `Size`, and `ExpandToGrid(resolution)`, the box grown out to a grid of cells that many blocks a side. `workspace.Terrain:ReadVoxels(region, resolution)` reads one.

```lua
local region = Region3.new(Vector3.new(0, 60, 0), Vector3.new(4, 64, 4))
local materials, occupancies = workspace.Terrain:ReadVoxels(region, 1)
print(materials.Size, materials[1][4][1], occupancies[1][4][1])
```
"##
    ),
    e!(
        "RaycastParams.new",
        "roblox",
        r##"
`RaycastParams.new()`

A table of `workspace:Raycast`'s options to set: `FilterType` (`Enum.RaycastFilterType.Exclude` or `Include`), `FilterDescendantsInstances` (a list), `IgnoreWater`.

```lua
local params = RaycastParams.new()
params.FilterDescendantsInstances = { game.Players.LocalPlayer.Character }
```
"##
    ),
    e!(
        "OverlapParams.new",
        "roblox",
        r##"
`OverlapParams.new()`

The same fields as RaycastParams, for code that makes one.

```lua
local params = OverlapParams.new()
```
"##
    ),
    e!(
        "Instance.new",
        "roblox",
        r##"
`Instance.new(className[, parent])`

A new Instance of a class (see the topic for the classes), owned by the mod: it is destroyed when the mod stops or reloads. Errors `Unable to create an Instance of type "X"` for others.

```lua
local part = Instance.new("Part")
part.Parent = workspace
```
"##
    ),
    e!(
        "Instance.fromExisting",
        "roblox",
        r##"
`Instance.fromExisting(instance)`

A copy of an Instance's properties, without its children.

```lua
local copy = Instance.fromExisting(workspace.CurrentCamera)
```
"##
    ),
    e!(
        "hookfunction",
        "executor",
        r##"
`hookfunction(target, hook)`

Calls of `target` (a Lua function, a `newcclosure` or a built-in such as `print`) run `hook` from now on, everywhere it is used. Returns a function that runs `target` as it was, for the hook to call.

```lua
local old
old = hookfunction(print, function(...) return old("[hooked]", ...) end)
```
"##
    ),
    e!(
        "replaceclosure",
        "executor",
        r##"
`replaceclosure(target, hook)`

The same function as `hookfunction`, under the name other executors use.

```lua
print(replaceclosure == hookfunction)
```
"##
    ),
    e!(
        "restorefunction",
        "executor",
        r##"
`restorefunction(f)`

Undoes every `hookfunction` of `f`. Errors `restorefunction: function is not hooked` for one that isn't.

```lua
local function f() end
hookfunction(f, function() end)
restorefunction(f)
```
"##
    ),
    e!(
        "hookmetamethod",
        "executor",
        r##"
`hookmetamethod(object, name, hook)`

Hooks the metamethod `name` (`"__index"`, `"__namecall"`, `"__newindex"`...) of the object's raw metatable and returns the original. Instances and `game` share one metatable, as in Roblox, so hooking `game`'s hooks every Instance's. A `__namecall` hook gets every method call, and every packet the game sends as `remote:FireServer(fields)` on its remote in `ReplicatedStorage.Minecraft` (see `roblox`); `getnamecallmethod()` names the method.

```lua
local old
old = hookmetamethod(game, "__namecall", function(self, ...)
  if getnamecallmethod() == "Kick" then return end
  return old(self, ...)
end)
```
"##
    ),
    e!(
        "newcclosure",
        "executor",
        r##"
`newcclosure(f)`

`f` wrapped as a C function (`iscclosure` is true, it has no upvalues). It can still wait (`task.wait`) and returns what `f` returns.

```lua
local wrapped = newcclosure(function(x) return x * 2 end)
print(wrapped(21), iscclosure(wrapped))
```
"##
    ),
    e!(
        "clonefunction",
        "executor",
        r##"
`clonefunction(f)`

A new function that behaves as `f` does, with the same globals; hooking `f` later leaves the clone as it was.

```lua
local copy = clonefunction(print)
```
"##
    ),
    e!(
        "getfunctionhash",
        "executor",
        r##"
`getfunctionhash(f)`

The SHA-384, in hex (96 characters), of a Lua function's constants and code: equal for functions with the same code. Errors on C closures.

```lua
print(getfunctionhash(function() end))
```
"##
    ),
    e!(
        "checkcaller",
        "executor",
        r##"
`checkcaller()`

Whether the code calling is the executor's: `true`, since every script is a mod, but inside a `__namecall` hook that the game's own packet reached (`remote:FireServer` from the game, see `roblox`).

```lua
print(checkcaller())
```
"##
    ),
    e!(
        "iscclosure",
        "executor",
        r##"
`iscclosure(f)`

Whether `f` is a C function: a built-in, a studio function or a `newcclosure`.

```lua
print(iscclosure(print))
```
"##
    ),
    e!(
        "islclosure",
        "executor",
        r##"
`islclosure(f)`

Whether `f` is a Lua function.

```lua
print(islclosure(function() end))
```
"##
    ),
    e!(
        "isexecutorclosure",
        "executor",
        r##"
`isexecutorclosure(f)`

Whether `f` is the executor's: any Lua function (every script is a mod's), the executor library's and the studio's functions, and `newcclosure`s; not Roblox's (`print`, `game`'s methods).

```lua
print(isexecutorclosure(getgenv), isexecutorclosure(print))
```
"##
    ),
    e!(
        "checkclosure",
        "executor",
        r##"
`checkclosure(f)`

The same function as `isexecutorclosure`, under the name other executors use.

```lua
print(checkclosure == isexecutorclosure)
```
"##
    ),
    e!(
        "isourclosure",
        "executor",
        r##"
`isourclosure(f)`

The same function as `isexecutorclosure`, under the name other executors use.

```lua
print(isourclosure == isexecutorclosure)
```
"##
    ),
    e!(
        "loadstring",
        "executor",
        r##"
`loadstring(source[, chunkname])`

Compiles Luau into a function with the caller's globals. On a syntax error it returns `nil` and the message (`[string "chunkname"]:1: ...`) instead of erroring.

```lua
local f, err = loadstring("return 1 + 1")
print(f())
```
"##
    ),
    e!(
        "debug.info",
        "executor",
        r##"
`debug.info(f_or_level, options)`

Luau's: for each letter of `options`, `s` the source (`[C]` for a C function), `l` the line (where it was defined, or the level's current line), `n` the name, `a` the parameter count and whether it is vararg, `f` the function.

```lua
print(debug.info(print, "s"), debug.info(1, "l"))
```
"##
    ),
    e!(
        "debug.traceback",
        "executor",
        r##"
`debug.traceback([message])`

The message and the calls in progress, one a line.

```lua
print(debug.traceback("here"))
```
"##
    ),
    e!(
        "debug.getinfo",
        "executor",
        r##"
`debug.getinfo(f_or_level)`

A table: `source`, `short_src`, `currentline`, `linedefined`, `name`, `what` (`"Lua"` or `"C"`), `nups`, `numparams`, `is_vararg`, `func`.

```lua
print(debug.getinfo(print).what)
```
"##
    ),
    e!(
        "debug.getconstants",
        "executor",
        r##"
`debug.getconstants(f_or_level)`

A Lua function's constants as Luau numbers them (strings, numbers, booleans; nil where Luau keeps an import or a closure). Errors on C closures.

```lua
for i, c in debug.getconstants(function() print("hi") end) do print(i, c) end
```
"##
    ),
    e!(
        "debug.getconstant",
        "executor",
        r##"
`debug.getconstant(f_or_level, index)`

One constant (from 1), or nil.

```lua
print(debug.getconstant(function() print("hi") end, 1))
```
"##
    ),
    e!(
        "debug.setconstant",
        "executor",
        r##"
`debug.setconstant(f_or_level, index, value)`

Replaces a constant (a number, string, boolean or nil) in that function: `game.Name` with "Name" set to "Players" reads `game.Players`.

```lua
local function f() return "a" end
local i = table.find(debug.getconstants(f), "a")
if i then debug.setconstant(f, i, "b") end
```
"##
    ),
    e!(
        "debug.getprotos",
        "executor",
        r##"
`debug.getprotos(f_or_level)`

The functions defined inside a Lua function, as closures that can't be called (inactive protos: for `debug.info` and the like).

```lua
print(#debug.getprotos(function() local function inner() end end))
```
"##
    ),
    e!(
        "debug.getproto",
        "executor",
        r##"
`debug.getproto(f_or_level, index[, activated])`

One nested function by index (from 1); with `activated` = true, a list of its live closures instead (those `getgc` sees), which can be called.

```lua
local function outer() return function() return 1 end end
local live = debug.getproto(outer, 1, true)
```
"##
    ),
    e!(
        "debug.getstack",
        "executor",
        r##"
`debug.getstack(level[, index])`

A register of the function `level` calls up (1: the caller), from 1, or all of them as a list. Errors for a level that isn't a Lua function.

```lua
local a = 5
print(debug.getstack(1, 1))
```
"##
    ),
    e!(
        "debug.setstack",
        "executor",
        r##"
`debug.setstack(level, index, value)`

Sets that register (a captured local's cell too).

```lua
local x = 1
debug.setstack(1, 1, 2)
```
"##
    ),
    e!(
        "debug.getupvalues",
        "executor",
        r##"
`debug.getupvalues(f_or_level)`

A Lua function's upvalues, in order. A local that is never assigned after a constant value is folded into its uses and isn't an upvalue here.

```lua
local n = 0
local function f() n += 1 end
print(debug.getupvalues(f)[1])
```
"##
    ),
    e!(
        "debug.getupvalue",
        "executor",
        r##"
`debug.getupvalue(f_or_level, index)`

One upvalue (from 1); errors when out of range or on a C closure.

```lua
local n = 0
local function f() n += 1 end
print(debug.getupvalue(f, 1))
```
"##
    ),
    e!(
        "debug.setupvalue",
        "executor",
        r##"
`debug.setupvalue(f_or_level, index, value)`

Replaces an upvalue: what the function and every other closure sharing that local see.

```lua
local n = 0
local function f() n += 1 return n end
debug.setupvalue(f, 1, 99)
```
"##
    ),
    e!(
        "debug.getmetatable",
        "executor",
        r##"
`debug.getmetatable(value)`

The raw metatable, as `getrawmetatable`.

```lua
print(debug.getmetatable(game) ~= nil)
```
"##
    ),
    e!(
        "debug.setmetatable",
        "executor",
        r##"
`debug.setmetatable(value, mt)`

As `setrawmetatable`.

```lua
debug.setmetatable({}, {})
```
"##
    ),
    e!(
        "debug.getregistry",
        "executor",
        r##"
`debug.getregistry()`

As `getreg`.

```lua
print(type(debug.getregistry()))
```
"##
    ),
    e!(
        "debug.profilebegin",
        "executor",
        r##"
`debug.profilebegin(label)`

Does nothing (Roblox's profiler isn't here).

```lua
debug.profilebegin("work")
```
"##
    ),
    e!(
        "debug.profileend",
        "executor",
        r##"
`debug.profileend()`

Does nothing.

```lua
debug.profileend()
```
"##
    ),
    e!(
        "debug.setmemorycategory",
        "executor",
        r##"
`debug.setmemorycategory(name)`

Does nothing.

```lua
debug.setmemorycategory("ui")
```
"##
    ),
    e!(
        "debug.resetmemorycategory",
        "executor",
        r##"
`debug.resetmemorycategory()`

Does nothing.

```lua
debug.resetmemorycategory()
```
"##
    ),
    e!(
        "getrawmetatable",
        "executor",
        r##"
`getrawmetatable(value)`

A table's or userdata's metatable, even when `__metatable` hides it (Instances' is read only).

```lua
local mt = getrawmetatable(game)
```
"##
    ),
    e!(
        "setrawmetatable",
        "executor",
        r##"
`setrawmetatable(value, mt)`

Sets a table's or userdata's metatable past `__metatable`; returns the value.

```lua
local t = setrawmetatable({}, { __index = { x = 1 } })
```
"##
    ),
    e!(
        "setreadonly",
        "executor",
        r##"
`setreadonly(t, on)`

Makes a table read only (writes error `attempt to modify a readonly table`) or writable again.

```lua
local mt = getrawmetatable(game)
setreadonly(mt, false)
setreadonly(mt, true)
```
"##
    ),
    e!(
        "isreadonly",
        "executor",
        r##"
`isreadonly(t)`

Whether a table is read only (`table.freeze` or `setreadonly`).

```lua
print(isreadonly(getrawmetatable(game)))
```
"##
    ),
    e!(
        "getnamecallmethod",
        "executor",
        r##"
`getnamecallmethod()`

The method name of the `obj:Method(...)` call in progress, inside a `__namecall` hook.

```lua
print(getnamecallmethod())
```
"##
    ),
    e!(
        "setnamecallmethod",
        "executor",
        r##"
`setnamecallmethod(name)`

Sets the method name `getnamecallmethod` gives and the original `__namecall` calls: a method call inside a hook names its own method, so a hook sets it back before calling the original.

```lua
local old
old = hookmetamethod(game, "__namecall", function(self, ...)
  local method = getnamecallmethod()
  local name = self:GetFullName() -- another method call
  setnamecallmethod(method)
  return old(self, ...)
end)
```
"##
    ),
    e!(
        "getgenv",
        "executor",
        r##"
`getgenv()`

The executor's globals, shared by every mod: the studio's API and the executor library, and what scripts put there (`getgenv().x = 1` makes `x` a global of every mod that hasn't its own). Each mod's own globals fall back to it.

```lua
getgenv().shared_setting = true
```
"##
    ),
    e!(
        "getrenv",
        "executor",
        r##"
`getrenv()`

Roblox's globals: the standard library, `game`, `workspace`, `Instance`, the data types, `task`. `getgenv()` falls back to it, so changing it changes what every mod sees.

```lua
print(getrenv().game == game)
```
"##
    ),
    e!(
        "getfenv",
        "executor",
        r##"
`getfenv([f_or_level])`

A Lua function's globals (by default the caller's).

```lua
print(getfenv() == _G)
```
"##
    ),
    e!(
        "setfenv",
        "executor",
        r##"
`setfenv(f_or_level, globals)`

Gives a Lua function other globals; returns it.

```lua
local f = function() return x end
setfenv(f, { x = 1 })
```
"##
    ),
    e!(
        "getgc",
        "executor",
        r##"
`getgc([includeTables])`

The live functions (and tables, with `true`) made since the studio started tracking them: from the load of the first mod whose source names `getgc`, `filtergc`, `getreg` or `getproto` (or the first call to one).

```lua
for _, f in getgc() do end
```
"##
    ),
    e!(
        "filtergc",
        "executor",
        r##"
`filtergc(kind, options[, returnOne])`

`getgc` filtered: `kind` `"function"` with `Name`, `Hash` (getfunctionhash), `Constants` and `Upvalues` (every item must be there) and `IgnoreExecutor` (true by default, which skips every Lua function here: they are all mods'), or `"table"` with `Keys`, `Values`, `KeyValuePairs` and `Metatable`. A list, or the first match with `returnOne`.

```lua
local f = filtergc("function", { Name = "update", IgnoreExecutor = false }, true)
```
"##
    ),
    e!(
        "getreg",
        "executor",
        r##"
`getreg()`

The live threads and functions the studio tracks (see `getgc`), as a list.

```lua
for _, v in getreg() do end
```
"##
    ),
    e!(
        "newproxy",
        "executor",
        r##"
`newproxy([withMetatable])`

An empty userdata, with an empty metatable of its own when asked.

```lua
local p = newproxy(true)
```
"##
    ),
    e!(
        "identifyexecutor",
        "executor",
        r##"
`identifyexecutor()`

`"Horizon Studio"` and the studio's version.

```lua
print(identifyexecutor())
```
"##
    ),
    e!(
        "getexecutorname",
        "executor",
        r##"
`getexecutorname()`

The same function as `identifyexecutor`, under the name other executors use.

```lua
print(getexecutorname == identifyexecutor)
```
"##
    ),
    e!(
        "getthreadidentity",
        "executor",
        r##"
`getthreadidentity()`

The running thread's identity: 8 unless `setthreadidentity` set another.

```lua
print(getthreadidentity())
```
"##
    ),
    e!(
        "setthreadidentity",
        "executor",
        r##"
`setthreadidentity(level)`

Sets the running thread's identity (0 to 8). Below 3, `game:GetService("CoreGui")` gives nil.

```lua
setthreadidentity(8)
```
"##
    ),
    e!(
        "getidentity",
        "executor",
        r##"
`getidentity()`

The same function as `getthreadidentity`, under the name other executors use.

```lua
print(getidentity == getthreadidentity)
```
"##
    ),
    e!(
        "setidentity",
        "executor",
        r##"
`setidentity(level)`

The same function as `setthreadidentity`, under the name other executors use.

```lua
print(setidentity == setthreadidentity)
```
"##
    ),
    e!(
        "base64encode",
        "executor",
        r##"
`base64encode(data)`

`data` (any bytes) as Base64 text.

```lua
print(base64encode("hi")) -- aGk=
```
"##
    ),
    e!(
        "base64decode",
        "executor",
        r##"
`base64decode(text)`

Base64 text back to its bytes; errors on bad Base64.

```lua
print(base64decode("aGk="))
```
"##
    ),
    e!(
        "lz4compress",
        "executor",
        r##"
`lz4compress(data)`

An LZ4 block of `data`.

```lua
local packed = lz4compress(string.rep("ab", 100))
```
"##
    ),
    e!(
        "lz4decompress",
        "executor",
        r##"
`lz4decompress(block[, size])`

An LZ4 block's bytes (`size`, if given, is the most it may make).

```lua
print(lz4decompress(lz4compress("hello")))
```
"##
    ),
    e!(
        "writefile",
        "executor",
        r##"
`writefile(path, data)`

Writes a file (and the folders on its way), replacing it. Paths are relative to the studio's `workspace` folder (`horizon-mods/.horizon-mod-data/horizon_studio/workspace/`), with `/` between folders; absolute paths and `..` are refused (`... is outside the workspace folder`). Files are 16 MiB at most, and executable or archive extensions (`.exe`, `.bat`, `.js`, `.zip`, ...) are refused.

```lua
writefile("notes.txt", "hello")
```
"##
    ),
    e!(
        "readfile",
        "executor",
        r##"
`readfile(path)`

A file's bytes; errors when there is none. Paths are relative to the studio's `workspace` folder (`horizon-mods/.horizon-mod-data/horizon_studio/workspace/`), with `/` between folders; absolute paths and `..` are refused (`... is outside the workspace folder`).

```lua
local text = readfile("notes.txt")
```
"##
    ),
    e!(
        "appendfile",
        "executor",
        r##"
`appendfile(path, data)`

Adds to the end of a file, making it if needed. Paths are relative to the studio's `workspace` folder (`horizon-mods/.horizon-mod-data/horizon_studio/workspace/`), with `/` between folders; absolute paths and `..` are refused (`... is outside the workspace folder`).

```lua
appendfile("log.txt", "line\n")
```
"##
    ),
    e!(
        "isfile",
        "executor",
        r##"
`isfile(path)`

Whether a file is there.

```lua
print(isfile("notes.txt"))
```
"##
    ),
    e!(
        "isfolder",
        "executor",
        r##"
`isfolder(path)`

Whether a folder is there.

```lua
print(isfolder("saves"))
```
"##
    ),
    e!(
        "makefolder",
        "executor",
        r##"
`makefolder(path)`

Makes a folder (and those on its way).

```lua
makefolder("saves")
```
"##
    ),
    e!(
        "delfile",
        "executor",
        r##"
`delfile(path)`

Deletes a file, if there is one.

```lua
delfile("notes.txt")
```
"##
    ),
    e!(
        "delfolder",
        "executor",
        r##"
`delfolder(path)`

Deletes a folder and what is in it (not the workspace folder itself).

```lua
delfolder("saves")
```
"##
    ),
    e!(
        "listfiles",
        "executor",
        r##"
`listfiles([path])`

The files and folders in a folder, as paths from the workspace folder (`"saves/a.txt"`), sorted.

```lua
for _, p in listfiles("") do print(p) end
```
"##
    ),
    e!(
        "loadfile",
        "executor",
        r##"
`loadfile(path)`

Compiles a file of Luau into a function with the caller's globals; a syntax error errors.

```lua
writefile("m.lua", "return 1")
print(loadfile("m.lua")())
```
"##
    ),
    e!(
        "dofile",
        "executor",
        r##"
`dofile(path)`

Runs a file of Luau and returns what it returns.

```lua
writefile("m.lua", "return 1")
print(dofile("m.lua"))
```
"##
    ),
    e!(
        "getcustomasset",
        "executor",
        r##"
`getcustomasset(path)`

An asset id (`rbxasset://horizon-studio/<path>`) for a workspace file. An ImageLabel or ImageButton with it as its `Image` draws the PNG.

```lua
local img = Instance.new("ImageLabel")
img.Image = getcustomasset("icon.png")
```
"##
    ),
    e!(
        "request",
        "executor",
        r##"
`request(options)`

An HTTP request, waiting (the thread yields) for the answer: `options` has `Url`, `Method` (`GET`, `HEAD`, `POST`, `PUT`, `DELETE`, `OPTIONS`, `PATCH`), and optional `Body`, `Headers` and `Cookies`. Returns `{Success, StatusCode, StatusMessage, Headers, Body}`; a request that got no answer has `StatusCode` 0 and the reason in `StatusMessage`. It reaches the internet only: addresses on this computer or its network are refused. It sends `User-Agent: Horizon Studio` and the `Horizon-Studio-User-Identifier` and `Horizon-Studio-Fingerprint` headers (a value made from the studio's folder, not the machine). `game:HttpGet(url)`, `game:HttpPost(url, data[, contentType])` and HttpService's `GetAsync`, `PostAsync` and `RequestAsync` use it and error when there is no answer.

```lua
local r = request({ Url = "https://httpbin.org/get", Method = "GET" })
print(r.StatusCode)
```
"##
    ),
    e!(
        "http_request",
        "executor",
        r##"
`http_request(options)`

The same function as `request`, under the name other executors use.

```lua
print(http_request == request)
```
"##
    ),
    e!(
        "http.request",
        "executor",
        r##"
`http.request(options)`

The same function as `request`.

```lua
print(http.request == request)
```
"##
    ),
    e!(
        "WebSocket.connect",
        "executor",
        r##"
`WebSocket.connect(url)`

Opens a `ws://` or `wss://` WebSocket, waiting until it is open (errors if it can't). The object has `Send(message)`, `Close()`, and the events `OnMessage(message)` and `OnClose()`. The internet only, as `request`.

```lua
local ws = WebSocket.connect("wss://ws.postman-echo.com/raw")
ws.OnMessage:Connect(print)
ws:Send("Hello")
```
"##
    ),
    e!(
        "WebSocket.Connect",
        "executor",
        r##"
`WebSocket.Connect(url)`

The same function as `WebSocket.connect`.

```lua
print(WebSocket.Connect == WebSocket.connect)
```
"##
    ),
    e!(
        "Drawing.new",
        "executor",
        r##"
`Drawing.new(kind)`

A drawing over the HUD, in screen dp (the Camera's ViewportSize): `Line` (`From`, `To`, `Thickness`), `Text` (`Text`, `Size`, `Position`, `Center`, `Outline`, `OutlineColor`, `Font` from `Drawing.Fonts`, read-only `TextBounds`), `Image` (`Data`: a PNG's bytes, `Size`, `Position`), `Circle` (`Position` its centre, `Radius`, `NumSides`, `Thickness`, `Filled`), `Square` (`Position`, `Size`, `Thickness`, `Filled`), `Quad` (`PointA` to `PointD`) and `Triangle` (`PointA` to `PointC`), each with `Thickness` and `Filled`. All have `Visible` (false at first), `ZIndex`, `Color`, `Transparency` (0 opaque, 1 invisible), `__OBJECT_EXISTS` and `Destroy()` (or `Remove()`). They go when their mod stops.

```lua
local c = Drawing.new("Circle")
c.Radius = 30
c.Position = workspace.CurrentCamera.ViewportSize / 2
c.Visible = true
```
"##
    ),
    e!(
        "Drawing.clear",
        "executor",
        r##"
`Drawing.clear()`

The same function as `cleardrawcache`.

```lua
Drawing.clear()
```
"##
    ),
    e!(
        "cleardrawcache",
        "executor",
        r##"
`cleardrawcache()`

Destroys every Drawing.

```lua
cleardrawcache()
```
"##
    ),
    e!(
        "getrenderproperty",
        "executor",
        r##"
`getrenderproperty(drawing, property)`

`drawing[property]`.

```lua
local c = Drawing.new("Circle")
print(getrenderproperty(c, "Radius"))
```
"##
    ),
    e!(
        "setrenderproperty",
        "executor",
        r##"
`setrenderproperty(drawing, property, value)`

`drawing[property] = value`.

```lua
local c = Drawing.new("Circle")
setrenderproperty(c, "Radius", 10)
```
"##
    ),
    e!(
        "isrenderobj",
        "executor",
        r##"
`isrenderobj(value)`

Whether the value is a Drawing.

```lua
print(isrenderobj(Drawing.new("Line")))
```
"##
    ),
    e!(
        "cloneref",
        "executor",
        r##"
`cloneref(instance)`

Another reference to the Instance: it works the same but isn't `==` to the first (compare with `compareinstances`).

```lua
local p = cloneref(game.Players.LocalPlayer)
```
"##
    ),
    e!(
        "compareinstances",
        "executor",
        r##"
`compareinstances(a, b)`

Whether two values are the same Instance, clonerefs included.

```lua
print(compareinstances(game, cloneref(game)))
```
"##
    ),
    e!(
        "gethui",
        "executor",
        r##"
`gethui()`

A Folder outside the DataModel for GUIs: ScreenGuis parented there draw like the PlayerGui's, and nothing under `game` finds them.

```lua
local gui = Instance.new("ScreenGui")
gui.Parent = gethui()
```
"##
    ),
    e!(
        "getinstances",
        "executor",
        r##"
`getinstances()`

Every Instance that exists (destroyed ones aside), in or out of the DataModel. The world's entities are among them: calling it is a look at the world, which mirrors them (see the `roblox` topic).

```lua
print(#getinstances())
```
"##
    ),
    e!(
        "getnilinstances",
        "executor",
        r##"
`getnilinstances()`

The Instances with no parent (besides `game` and `gethui()`).

```lua
print(#getnilinstances())
```
"##
    ),
    e!(
        "fireclickdetector",
        "executor",
        r##"
`fireclickdetector(detector[, distance[, event]])`

Fires a ClickDetector's `MouseClick` (or `RightMouseClick`, `MouseHoverEnter`, `MouseHoverLeave`) with the LocalPlayer.

```lua
local cd = Instance.new("ClickDetector")
fireclickdetector(cd)
```
"##
    ),
    e!(
        "fireproximityprompt",
        "executor",
        r##"
`fireproximityprompt(prompt)`

Triggers a ProximityPrompt at once: PromptButtonHoldBegan, Triggered, PromptButtonHoldEnded and TriggerEnded, with the LocalPlayer.

```lua
local p = Instance.new("ProximityPrompt")
fireproximityprompt(p)
```
"##
    ),
    e!(
        "firetouchinterest",
        "executor",
        r##"
`firetouchinterest(part1, part2, toggle)`

Fires each part's `Touched` with the other (`toggle` true or 0), or `TouchEnded` (false or 1).

```lua
local a, b = Instance.new("Part"), Instance.new("Part")
firetouchinterest(a, b, true)
```
"##
    ),
    e!(
        "getcallbackvalue",
        "executor",
        r##"
`getcallbackvalue(instance, property)`

A callback property's function (`OnInvoke`, `OnClientInvoke`...), or nil.

```lua
local bf = Instance.new("BindableFunction")
print(getcallbackvalue(bf, "OnInvoke"))
```
"##
    ),
    e!(
        "getconnections",
        "executor",
        r##"
`getconnections(signal)`

A signal's connections, each a table: `Enabled`, `ForeignState` (false), `LuaConnection` (true), `Function`, `Thread` (that connected it), and `Fire(...)`, `Defer(...)`, `Disconnect()`, `Disable()`, `Enable()`.

```lua
for _, c in getconnections(workspace.ChildAdded) do c:Disable() end
```
"##
    ),
    e!(
        "firesignal",
        "executor",
        r##"
`firesignal(signal, ...)`

Runs every enabled connection of the signal now, each in a thread of its own, with these arguments.

```lua
firesignal(workspace.ChildAdded, workspace)
```
"##
    ),
    e!(
        "replicatesignal",
        "executor",
        r##"
`replicatesignal(signal, ...)`

Errors: there is no Roblox server here to replicate to.

```lua
print(pcall(replicatesignal, workspace.ChildAdded))
```
"##
    ),
    e!(
        "getscripts",
        "executor",
        r##"
`getscripts()`

Every LocalScript, Script and ModuleScript Instance, including each mod's `script`.

```lua
print(#getscripts())
```
"##
    ),
    e!(
        "getrunningscripts",
        "executor",
        r##"
`getrunningscripts()`

The `script` of every running mod, and the ModuleScripts of a running module's files that `require` has run.

```lua
print(#getrunningscripts())
```
"##
    ),
    e!(
        "getloadedmodules",
        "executor",
        r##"
`getloadedmodules()`

The ModuleScripts `require` has run.

```lua
print(#getloadedmodules())
```
"##
    ),
    e!(
        "getcallingscript",
        "executor",
        r##"
`getcallingscript()`

The script of the code that called the running function: a mod's LocalScript (or a module file's ModuleScript), its `script`. nil for the console's code, and inside a `__namecall` hook for the game's own packets.

```lua
print(getcallingscript())
```
"##
    ),
    e!(
        "getscriptfromthread",
        "executor",
        r##"
`getscriptfromthread(thread)`

nil, as every thread is the executor's here.

```lua
print(getscriptfromthread(coroutine.running()))
```
"##
    ),
    e!(
        "getsenv",
        "executor",
        r##"
`getsenv(script)`

A running mod's own globals, by its `script`; for the ModuleScript of a module's file that `require` has run, the file's own environment (as `getfenv()` there). Errors for a script that isn't running (a file not required yet, or whose top level failed).

```lua
print(getsenv(script) == getfenv())
```
"##
    ),
    e!(
        "getscriptclosure",
        "executor",
        r##"
`getscriptclosure(script)`

A new function of a running mod's main file (not the one running), or of a module's file that `require` has run (by its ModuleScript); else nil.

```lua
local main = getscriptclosure(script)
```
"##
    ),
    e!(
        "getscriptfunction",
        "executor",
        r##"
`getscriptfunction(script)`

The same function as `getscriptclosure`, under the name other executors use.

```lua
print(getscriptfunction == getscriptclosure)
```
"##
    ),
    e!(
        "getscriptbytecode",
        "executor",
        r##"
`getscriptbytecode(script)`

A running mod's compiled main file, or a module's file that `require` has run (by its ModuleScript), as the studio's own text dump of its bytecode (it is not Luau bytecode), or nil.

```lua
print(#getscriptbytecode(script))
```
"##
    ),
    e!(
        "dumpstring",
        "executor",
        r##"
`dumpstring(script)`

The same function as `getscriptbytecode`, under the name other executors use.

```lua
print(dumpstring == getscriptbytecode)
```
"##
    ),
    e!(
        "getscripthash",
        "executor",
        r##"
`getscripthash(script)`

The SHA-384, in hex, of `getscriptbytecode(script)`, or nil.

```lua
print(getscripthash(script))
```
"##
    ),
    e!(
        "gethiddenproperty",
        "executor",
        r##"
`gethiddenproperty(instance, property)`

A property's value and whether it is hidden: Roblox's properties here give `false`; others are hidden ones, kept per Instance (nil until set). A ModuleScript's hidden `Source` is what `require` runs.

```lua
print(gethiddenproperty(Instance.new("Part"), "Name"))
```
"##
    ),
    e!(
        "sethiddenproperty",
        "executor",
        r##"
`sethiddenproperty(instance, property, value)`

Sets a property, hidden or not; returns whether it was hidden.

```lua
local m = Instance.new("ModuleScript")
sethiddenproperty(m, "Source", "return 1")
print(require(m))
```
"##
    ),
    e!(
        "isscriptable",
        "executor",
        r##"
`isscriptable(instance, property)`

Whether scripts can read and write a property (nil when there is no such property).

```lua
print(isscriptable(Instance.new("Part"), "Name"))
```
"##
    ),
    e!(
        "setscriptable",
        "executor",
        r##"
`setscriptable(instance, property, on)`

Makes a property unreadable to scripts (false), or a hidden one readable (true); returns whether it was scriptable before.

```lua
local p = Instance.new("Part")
setscriptable(p, "Name", false)
```
"##
    ),
    e!(
        "crypt.base64encode",
        "executor",
        r##"
`crypt.base64encode(data)`

`data` as Base64 text (also `base64encode`, `base64.encode`, `crypt.base64.encode`).

```lua
print(crypt.base64encode("test")) -- dGVzdA==
```
"##
    ),
    e!(
        "crypt.base64decode",
        "executor",
        r##"
`crypt.base64decode(text)`

Base64 text back to its bytes.

```lua
print(crypt.base64decode("dGVzdA==")) -- test
```
"##
    ),
    e!(
        "crypt.base64_encode",
        "executor",
        r##"
`crypt.base64_encode(data)`

The same function as `crypt.base64encode`, under the name UNC uses.

```lua
print(crypt.base64_encode ~= nil)
```
"##
    ),
    e!(
        "crypt.base64_decode",
        "executor",
        r##"
`crypt.base64_decode(text)`

The same function as `crypt.base64decode`, under the name UNC uses.

```lua
print(crypt.base64_decode ~= nil)
```
"##
    ),
    e!(
        "crypt.generatebytes",
        "executor",
        r##"
`crypt.generatebytes(size)`

`size` random bytes, as Base64 (decode it to get the raw bytes).

```lua
local b = crypt.generatebytes(16)
```
"##
    ),
    e!(
        "crypt.generatekey",
        "executor",
        r##"
`crypt.generatekey()`

A random 32-byte AES key, as Base64.

```lua
local key = crypt.generatekey()
```
"##
    ),
    e!(
        "crypt.hash",
        "executor",
        r##"
`crypt.hash(data, algorithm)`

The digest of `data` in hex. Algorithms: `md5`, `sha1`, `sha224`/`256`/`384`/`512`, `sha3-224`/`256`/`384`/`512`.

```lua
print(crypt.hash("test", "sha256"))
```
"##
    ),
    e!(
        "crypt.encrypt",
        "executor",
        r##"
`crypt.encrypt(data, key, iv?, mode?)`

AES. `key` and `iv` are Base64; a random IV is made when none is given; `mode` is `CBC` (default), `ECB`, `CFB`, `OFB`, `CTR` or `GCM`. Returns the Base64 ciphertext and the Base64 IV.

```lua
local key = crypt.generatekey()
local data, iv = crypt.encrypt("hi", key, nil, "CBC")
```
"##
    ),
    e!(
        "crypt.decrypt",
        "executor",
        r##"
`crypt.decrypt(data, key, iv, mode?)`

The plaintext from the Base64 `data`, `key` and `iv`.

```lua
print(crypt.decrypt(data, key, iv, "CBC"))
```
"##
    ),
    e!(
        "base64.encode",
        "executor",
        r##"
`base64.encode(data)`

`data` as Base64 (also `base64encode`, `base64_encode`).

```lua
print(base64.encode("test"))
```
"##
    ),
    e!(
        "base64.decode",
        "executor",
        r##"
`base64.decode(text)`

Base64 text back to its bytes.

```lua
print(base64.decode("dGVzdA=="))
```
"##
    ),
    e!(
        "base64_encode",
        "executor",
        r##"
`base64_encode(data)`

The same function as `base64encode`, under the name UNC uses.

```lua
print(base64_encode ~= nil)
```
"##
    ),
    e!(
        "base64_decode",
        "executor",
        r##"
`base64_decode(text)`

The same function as `base64decode`, under the name UNC uses.

```lua
print(base64_decode ~= nil)
```
"##
    ),
    e!(
        "cache.invalidate",
        "executor",
        r##"
`cache.invalidate(instance)`

Drops the Instance's cached reference, so the next time it is read a new, non-equal reference is made.

```lua
cache.invalidate(workspace:FindFirstChildOfClass("Camera"))
```
"##
    ),
    e!(
        "cache.iscached",
        "executor",
        r##"
`cache.iscached(instance)`

Whether this reference is the Instance's current cached one.

```lua
print(cache.iscached(workspace))
```
"##
    ),
    e!(
        "cache.replace",
        "executor",
        r##"
`cache.replace(instance, other)`

Makes references to `instance` resolve to `other` from now on.

```lua
cache.replace(a, b)
```
"##
    ),
    e!(
        "rconsoleprint",
        "executor",
        r##"
`rconsoleprint(text)`

Prints a line in the Studio console.

```lua
rconsoleprint("hello")
```
"##
    ),
    e!(
        "consoleprint",
        "executor",
        r##"
`consoleprint(text)`

The same function as `rconsoleprint`, under the name UNC uses.

```lua
print(consoleprint ~= nil)
```
"##
    ),
    e!(
        "rconsoleclear",
        "executor",
        r##"
`rconsoleclear()`

Empties the Studio console.

```lua
rconsoleclear()
```
"##
    ),
    e!(
        "consoleclear",
        "executor",
        r##"
`consoleclear()`

The same function as `rconsoleclear`, under the name UNC uses.

```lua
print(consoleclear ~= nil)
```
"##
    ),
    e!(
        "rconsolecreate",
        "executor",
        r##"
`rconsolecreate()`

Does nothing (the Studio console is always open).

```lua
rconsolecreate()
```
"##
    ),
    e!(
        "consolecreate",
        "executor",
        r##"
`consolecreate()`

The same function as `rconsolecreate`, under the name UNC uses.

```lua
print(consolecreate ~= nil)
```
"##
    ),
    e!(
        "rconsoledestroy",
        "executor",
        r##"
`rconsoledestroy()`

Does nothing.

```lua
rconsoledestroy()
```
"##
    ),
    e!(
        "consoledestroy",
        "executor",
        r##"
`consoledestroy()`

The same function as `rconsoledestroy`, under the name UNC uses.

```lua
print(consoledestroy ~= nil)
```
"##
    ),
    e!(
        "rconsoleinput",
        "executor",
        r##"
`rconsoleinput()`

Returns `""` (the Studio console has no blocking input).

```lua
local line = rconsoleinput()
```
"##
    ),
    e!(
        "consoleinput",
        "executor",
        r##"
`consoleinput()`

The same function as `rconsoleinput`, under the name UNC uses.

```lua
print(consoleinput ~= nil)
```
"##
    ),
    e!(
        "rconsolesettitle",
        "executor",
        r##"
`rconsolesettitle(title)`

Does nothing (the console has no title bar of its own).

```lua
rconsolesettitle("My tool")
```
"##
    ),
    e!(
        "rconsolename",
        "executor",
        r##"
`rconsolename(title)`

The same function as `rconsolesettitle`, under the name UNC uses.

```lua
print(rconsolename ~= nil)
```
"##
    ),
    e!(
        "consolesettitle",
        "executor",
        r##"
`consolesettitle(title)`

The same function as `rconsolesettitle`, under the name UNC uses.

```lua
print(consolesettitle ~= nil)
```
"##
    ),
    e!(
        "mouse1click",
        "executor",
        r##"
`mouse1click()`

A left press and release, onto the Roblox GUI's pointer (its screen must have the mouse; see the `roblox` topic).

```lua
mouse1click()
```
"##
    ),
    e!(
        "mouse1press",
        "executor",
        r##"
`mouse1press()`

Holds the left button down.

```lua
mouse1press()
```
"##
    ),
    e!(
        "mouse1release",
        "executor",
        r##"
`mouse1release()`

Lets the left button up.

```lua
mouse1release()
```
"##
    ),
    e!(
        "mouse2click",
        "executor",
        r##"
`mouse2click()`

A right press and release.

```lua
mouse2click()
```
"##
    ),
    e!(
        "mouse2press",
        "executor",
        r##"
`mouse2press()`

Holds the right button down.

```lua
mouse2press()
```
"##
    ),
    e!(
        "mouse2release",
        "executor",
        r##"
`mouse2release()`

Lets the right button up.

```lua
mouse2release()
```
"##
    ),
    e!(
        "mousemoveabs",
        "executor",
        r##"
`mousemoveabs(x, y)`

Moves the pointer to `x, y` in dp.

```lua
mousemoveabs(100, 100)
```
"##
    ),
    e!(
        "mousemoverel",
        "executor",
        r##"
`mousemoverel(dx, dy)`

Moves the pointer by `dx, dy` dp.

```lua
mousemoverel(5, 0)
```
"##
    ),
    e!(
        "mousescroll",
        "executor",
        r##"
`mousescroll(amount)`

Scrolls the wheel (positive up).

```lua
mousescroll(1)
```
"##
    ),
    e!(
        "isrbxactive",
        "executor",
        r##"
`isrbxactive()`

Whether the game window is active (playing, not paused).

```lua
print(isrbxactive())
```
"##
    ),
    e!(
        "isgameactive",
        "executor",
        r##"
`isgameactive()`

The same function as `isrbxactive`, under the name UNC uses.

```lua
print(isgameactive ~= nil)
```
"##
    ),
    e!(
        "setclipboard",
        "executor",
        r##"
`setclipboard(text)`

Puts `text` on the system clipboard.

```lua
setclipboard("copied")
```
"##
    ),
    e!(
        "toclipboard",
        "executor",
        r##"
`toclipboard(text)`

The same function as `setclipboard`, under the name UNC uses.

```lua
print(toclipboard ~= nil)
```
"##
    ),
    e!(
        "messagebox",
        "executor",
        r##"
`messagebox(text[, title[, flags]])`

Shows `text` in the Studio console and returns 1 (the OK button); there is no modal dialog here.

```lua
messagebox("done", "Info", 0)
```
"##
    ),
    e!(
        "setfpscap",
        "executor",
        r##"
`setfpscap(cap)`

Keeps the cap for `getfpscap`; Horizon's frame rate stays the game's.

```lua
setfpscap(60)
```
"##
    ),
    e!(
        "getfpscap",
        "executor",
        r##"
`getfpscap()`

The cap `setfpscap` was given, or 60 when none was.

```lua
print(getfpscap())
```
"##
    ),
    e!(
        "keypress",
        "executor",
        r##"
`keypress(keycode)`

Presses a key, by its Windows virtual-key code (`0x41` is A, `0x20` Space, `0x0D` Enter), as the game's own key would: UserInputService's InputBegan, the mouse's KeyDown and ContextActionService get it. `keyrelease` lets it go, `keyclick` does both.

```lua
keypress(0x57) task.wait(1) keyrelease(0x57)
```
"##
    ),
    e!(
        "keyrelease",
        "executor",
        r##"
`keyrelease(keycode)`

Lets go of a key `keypress` pressed (its Windows virtual-key code).

```lua
keyrelease(0x57)
```
"##
    ),
    e!(
        "keyclick",
        "executor",
        r##"
`keyclick(keycode)`

Presses and lets go of a key, by its Windows virtual-key code.

```lua
keyclick(0x45)
```
"##
    ),
    e!(
        "getclipboard",
        "executor",
        r##"
`getclipboard()`

The text on the clipboard.

```lua
print(getclipboard())
```
"##
    ),
    e!(
        "getsynasset",
        "executor",
        r##"
`getsynasset(path)`

Synapse X's name for `getcustomasset`.

```lua
image.Image = getsynasset("logo.png")
```
"##
    ),
    e!(
        "iswindowactive",
        "executor",
        r##"
`iswindowactive()`

Another name for `isrbxactive`: whether the game has the keys (no screen of its own up).

```lua
print(iswindowactive())
```
"##
    ),
    e!(
        "printconsole",
        "executor",
        r##"
`printconsole(...)`

Another name for `rconsoleprint`.

```lua
printconsole("hi")
```
"##
    ),
    e!(
        "openfile",
        "executor",
        r##"
`openfile(path)`

Another name for `readfile`: a file's text in the workspace folder.

```lua
print(openfile("notes.txt"))
```
"##
    ),
    e!(
        "syn.request",
        "executor",
        r##"
`syn.request(options)`

Synapse X's name for `request`.

```lua
print(syn.request({ Url = "https://example.com" }).StatusCode)
```
"##
    ),
    e!(
        "syn.queue_on_teleport",
        "executor",
        r##"
`syn.queue_on_teleport(code)`

Synapse X's name for `queue_on_teleport`: nothing teleports here.

```lua
syn.queue_on_teleport("print('back')")
```
"##
    ),
    e!(
        "syn.write_clipboard",
        "executor",
        r##"
`syn.write_clipboard(text)`

Synapse X's name for `setclipboard`.

```lua
syn.write_clipboard("hi")
```
"##
    ),
    e!(
        "syn.set_thread_identity",
        "executor",
        r##"
`syn.set_thread_identity(n)`

Synapse X's name for `setthreadidentity`.

```lua
syn.set_thread_identity(8)
```
"##
    ),
    e!(
        "syn.get_thread_identity",
        "executor",
        r##"
`syn.get_thread_identity()`

Synapse X's name for `getthreadidentity`.

```lua
print(syn.get_thread_identity())
```
"##
    ),
    e!(
        "syn.protect_gui",
        "executor",
        r##"
`syn.protect_gui(gui)`

Hides a ScreenGui from the game's own scripts in Synapse X; here no game script looks, so it does nothing (parent it to `gethui()` to draw it over everything).

```lua
syn.protect_gui(gui) gui.Parent = game:GetService("CoreGui")
```
"##
    ),
    e!(
        "syn.unprotect_gui",
        "executor",
        r##"
`syn.unprotect_gui(gui)`

The reverse of `syn.protect_gui`: does nothing here.

```lua
syn.unprotect_gui(gui)
```
"##
    ),
    e!(
        "queue_on_teleport",
        "executor",
        r##"
`queue_on_teleport(code)`

Keeps `code` to run once the player is on the next server they join: after a rejoin (`TeleportService:Teleport(game.PlaceId)`, Infinite Yield's `rejoin`) or a server joined from the list. It runs as console code does.

```lua
queue_on_teleport("print('back on ' .. game.JobId)")
```
"##
    ),
    e!(
        "queueonteleport",
        "executor",
        r##"
`queueonteleport(code)`

The same function as `queue_on_teleport`, under the name UNC uses.

```lua
print(queueonteleport ~= nil)
```
"##
    ),
    e!(
        "setrbxclipboard",
        "executor",
        r##"
`setrbxclipboard(data)`

Does nothing (no Roblox Studio clipboard here); returns true.

```lua
print(setrbxclipboard(""))
```
"##
    ),
    e!(
        "getthreadcontext",
        "executor",
        r##"
`getthreadcontext()`

The same function as `getthreadidentity`, under the name UNC uses.

```lua
print(getthreadcontext ~= nil)
```
"##
    ),
    e!(
        "setthreadcontext",
        "executor",
        r##"
`setthreadcontext(level)`

The same function as `setthreadidentity`, under the name UNC uses.

```lua
print(setthreadcontext ~= nil)
```
"##
    ),
    e!(
        "version",
        "roblox",
        r##"
`version()`

The Roblox client's version, as `"0.697.0.6970574"`.

```lua
print(version())
```
"##
    ),
    e!(
        "Version",
        "roblox",
        r##"
`Version()`

Roblox's older name for `version`.

```lua
print(Version())
```
"##
    ),
    e!(
        "Wait",
        "roblox",
        r##"
`Wait([seconds])`

Roblox's older name for `wait`.

```lua
Wait(0.1)
```
"##
    ),
    e!(
        "Spawn",
        "roblox",
        r##"
`Spawn(f)`

Roblox's older name for `spawn`.

```lua
Spawn(function() print("later") end)
```
"##
    ),
    e!(
        "Delay",
        "roblox",
        r##"
`Delay(seconds, f)`

Roblox's older name for `delay`.

```lua
Delay(1, function() print("a second later") end)
```
"##
    ),
    e!(
        "elapsedTime",
        "roblox",
        r##"
`elapsedTime()`

Seconds since the studio's Roblox layer started.

```lua
print(elapsedTime())
```
"##
    ),
    e!(
        "ElapsedTime",
        "roblox",
        r##"
`ElapsedTime()`

Roblox's older name for `elapsedTime`.

```lua
print(ElapsedTime())
```
"##
    ),
    e!(
        "stats",
        "roblox",
        r##"
`stats()`

The Stats service, as `game:GetService("Stats")`.

```lua
print(stats())
```
"##
    ),
    e!(
        "Stats",
        "roblox",
        r##"
`Stats()`

Roblox's older name for `stats`.

```lua
print(Stats())
```
"##
    ),
    e!(
        "printidentity",
        "roblox",
        r##"
`printidentity([prefix])`

Prints the thread's identity (8, an executor's): `Current identity is 8`.

```lua
printidentity()
```
"##
    ),
    e!(
        "Game.run",
        "roblox",
        r##"
`Game.run(command)`

`Game` is Roblox's older name for `game`: `Game.run` is `game.run`.

```lua
Game.run("time set day")
```
"##
    ),
    e!(
        "Game.fps",
        "roblox",
        r##"
`Game.fps()`

`game.fps`, by Roblox's older name `Game`.

```lua
print(Game.fps())
```
"##
    ),
    e!(
        "Game.screen",
        "roblox",
        r##"
`Game.screen()`

`game.screen`, by Roblox's older name `Game`.

```lua
print(Game.screen())
```
"##
    ),
    e!(
        "Game.ticks",
        "roblox",
        r##"
`Game.ticks()`

`game.ticks`, by Roblox's older name `Game`.

```lua
print(Game.ticks())
```
"##
    ),
    e!(
        "Game.time",
        "roblox",
        r##"
`Game.time()`

`game.time`, by Roblox's older name `Game`.

```lua
print(Game.time())
```
"##
    ),
    e!(
        "Vector3int16.new",
        "roblox",
        r##"
`Vector3int16.new([x, y, z])`

A Vector3int16: X, Y and Z as 16-bit integers (fractions cut, values wrapping as Roblox's do), with `+`, `-`, `*`, `/` and `==`.

```lua
print(Vector3int16.new(1, 2, 3) + Vector3int16.new(1, 1, 1))
```
"##
    ),
    e!(
        "Vector2int16.new",
        "roblox",
        r##"
`Vector2int16.new([x, y])`

A Vector2int16: X and Y as 16-bit integers, with `+`, `-`, `*`, `/` and `==`.

```lua
print(Vector2int16.new(3, 4) * 2)
```
"##
    ),
    e!(
        "Region3int16.new",
        "roblox",
        r##"
`Region3int16.new([min, max])`

A Region3int16 from two Vector3int16s, its `Min` and `Max`.

```lua
print(Region3int16.new(Vector3int16.new(0, 0, 0), Vector3int16.new(4, 4, 4)).Max)
```
"##
    ),
    e!(
        "Faces.new",
        "roblox",
        r##"
`Faces.new(...)`

Faces from NormalIds: `Top`, `Bottom`, `Left`, `Right`, `Back` and `Front` are true or false.

```lua
print(Faces.new(Enum.NormalId.Top).Top)
```
"##
    ),
    e!(
        "Axes.new",
        "roblox",
        r##"
`Axes.new(...)`

Axes from Axis items or NormalIds: `X`, `Y` and `Z` (and the faces on them) are true or false.

```lua
print(Axes.new(Enum.Axis.X).X)
```
"##
    ),
    e!(
        "PathWaypoint.new",
        "roblox",
        r##"
`PathWaypoint.new([position, action, label])`

A PathWaypoint: its `Position`, `Action` (Walk unless given) and `Label`.

```lua
print(PathWaypoint.new(Vector3.new(1, 2, 3)).Position)
```
"##
    ),
    e!(
        "DateTime.now",
        "roblox",
        r##"
`DateTime.now()`

The DateTime now: `UnixTimestamp`, `UnixTimestampMillis`, and `ToUniversalTime()`, `ToLocalTime()`, `ToIsoDate()`, `FormatUniversalTime(format)` and `FormatLocalTime(format)` (Roblox's tokens: YYYY, MMMM, MM, D, dddd, HH, h, mm, ss, A...). Local time is UTC.

```lua
print(DateTime.now():ToIsoDate())
```
"##
    ),
    e!(
        "DateTime.fromUnixTimestamp",
        "roblox",
        r##"
`DateTime.fromUnixTimestamp(seconds)`

The DateTime that many seconds after 1970 began (UTC).

```lua
print(DateTime.fromUnixTimestamp(0))
```
"##
    ),
    e!(
        "DateTime.fromUnixTimestampMillis",
        "roblox",
        r##"
`DateTime.fromUnixTimestampMillis(ms)`

The DateTime that many milliseconds after 1970 began (UTC).

```lua
print(DateTime.fromUnixTimestampMillis(1500))
```
"##
    ),
    e!(
        "DateTime.fromUniversalTime",
        "roblox",
        r##"
`DateTime.fromUniversalTime([year, month, day, hour, minute, second, millisecond])`

The DateTime at that UTC date and time.

```lua
print(DateTime.fromUniversalTime(2024, 2, 29, 13, 5):FormatUniversalTime("dddd, MMMM D YYYY"))
```
"##
    ),
    e!(
        "DateTime.fromLocalTime",
        "roblox",
        r##"
`DateTime.fromLocalTime([year, month, day, hour, minute, second, millisecond])`

As `fromUniversalTime`: local time is UTC here.

```lua
print(DateTime.fromLocalTime(2024, 1, 1))
```
"##
    ),
    e!(
        "DateTime.fromIsoDate",
        "roblox",
        r##"
`DateTime.fromIsoDate(text)`

The DateTime an ISO 8601 date names (`2024-02-29T13:05:09.250Z`), or nil.

```lua
print(DateTime.fromIsoDate("2024-02-29T13:05:09Z").UnixTimestamp)
```
"##
    ),
    e!(
        "SharedTable.new",
        "roblox",
        r##"
`SharedTable.new([table])`

A SharedTable: indexed, set and iterated as a table (`for k, v in st do`).

```lua
local st = SharedTable.new({ a = 1 })
st.b = 2
print(st.a, st.b)
```
"##
    ),
    e!(
        "SharedTable.clear",
        "roblox",
        r##"
`SharedTable.clear(st)`

Empties a SharedTable.

```lua
local st = SharedTable.new({ a = 1 })
SharedTable.clear(st)
```
"##
    ),
    e!(
        "SharedTable.clone",
        "roblox",
        r##"
`SharedTable.clone(st)`

A copy of a SharedTable.

```lua
print(SharedTable.clone(SharedTable.new({ a = 1 })).a)
```
"##
    ),
    e!(
        "SharedTable.cloneAndFreeze",
        "roblox",
        r##"
`SharedTable.cloneAndFreeze(st)`

A frozen copy of a SharedTable: setting it errors.

```lua
print(SharedTable.isFrozen(SharedTable.cloneAndFreeze(SharedTable.new())))
```
"##
    ),
    e!(
        "SharedTable.increment",
        "roblox",
        r##"
`SharedTable.increment(st, key[, by])`

Adds `by` (1) to a number in a SharedTable and returns what it was.

```lua
local st = SharedTable.new({ n = 1 })
print(SharedTable.increment(st, "n"), st.n)
```
"##
    ),
    e!(
        "SharedTable.isFrozen",
        "roblox",
        r##"
`SharedTable.isFrozen(st)`

Whether a SharedTable is frozen.

```lua
print(SharedTable.isFrozen(SharedTable.new()))
```
"##
    ),
    e!(
        "SharedTable.size",
        "roblox",
        r##"
`SharedTable.size(st)`

How many keys a SharedTable has.

```lua
print(SharedTable.size(SharedTable.new({ a = 1, b = 2 })))
```
"##
    ),
    e!(
        "SharedTable.update",
        "roblox",
        r##"
`SharedTable.update(st, key, f)`

Sets a key of a SharedTable to `f(old value)`.

```lua
local st = SharedTable.new({ n = 1 })
SharedTable.update(st, "n", function(n) return n * 10 end)
print(st.n)
```
"##
    ),
    e!(
        "CatalogSearchParams.new",
        "roblox",
        r##"
`CatalogSearchParams.new()`

A CatalogSearchParams with Roblox's defaults, its fields set as in Roblox (searches find nothing here).

```lua
local p = CatalogSearchParams.new()
p.SearchKeyword = "hat"
```
"##
    ),
    e!(
        "DockWidgetPluginGuiInfo.new",
        "roblox",
        r##"
`DockWidgetPluginGuiInfo.new([state, enabled, override, floatX, floatY, minWidth, minHeight])`

A DockWidgetPluginGuiInfo holding what it is given (no plugins here).

```lua
print(DockWidgetPluginGuiInfo.new().MinWidth)
```
"##
    ),
    e!(
        "Content.fromUri",
        "roblox",
        r##"
`Content.fromUri(uri)`

A Content naming an asset: `SourceType` Uri, `Uri`.

```lua
print(Content.fromUri("rbxassetid://1").Uri)
```
"##
    ),
    e!(
        "Content.fromObject",
        "roblox",
        r##"
`Content.fromObject(object)`

A Content holding an object: `SourceType` Object, `Object`.

```lua
print(typeof(Content.fromObject(workspace)))
```
"##
    ),
    e!(
        "SecurityCapabilities.new",
        "roblox",
        r##"
`SecurityCapabilities.new(...)`

A set of SecurityCapability items: `Contains`, `Add` and `Remove`.

```lua
print(SecurityCapabilities.new())
```
"##
    ),
    e!(
        "SecurityCapabilities.fromCurrent",
        "roblox",
        r##"
`SecurityCapabilities.fromCurrent()`

The running script's capabilities: none are kept here.

```lua
print(SecurityCapabilities.fromCurrent())
```
"##
    ),
    e!(
        "FloatCurveKey.new",
        "roblox",
        r##"
`FloatCurveKey.new(time, value[, interpolation])`

A FloatCurveKey: `Time`, `Value`, `Interpolation`, and `LeftTangent` and `RightTangent` to set.

```lua
print(FloatCurveKey.new(0, 1).Value)
```
"##
    ),
    e!(
        "ValueCurveKey.new",
        "roblox",
        r##"
`ValueCurveKey.new(time, value[, interpolation])`

A ValueCurveKey, as FloatCurveKey with any value.

```lua
print(ValueCurveKey.new(0, Vector3.new()).Time)
```
"##
    ),
    e!(
        "RotationCurveKey.new",
        "roblox",
        r##"
`RotationCurveKey.new(time, cframe[, interpolation])`

A RotationCurveKey, as FloatCurveKey with a CFrame.

```lua
print(RotationCurveKey.new(0, CFrame.new()).Time)
```
"##
    ),
    e!(
        "Path2DControlPoint.new",
        "roblox",
        r##"
`Path2DControlPoint.new([position, leftTangent, rightTangent])`

A Path2DControlPoint of UDim2s: `Position`, `LeftTangent` and `RightTangent`.

```lua
print(Path2DControlPoint.new(UDim2.fromOffset(4, 4)).Position)
```
"##
    ),
];
