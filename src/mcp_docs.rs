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
        title: "Script mods: files, loading, hot reload, errors",
        summary: "What a mod is, its files and header, how it loads, reloads and stops, and where the mods folder is.",
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
        summary: "Lua 5.3 without coroutines and goto: numbers (all floats), integer and float behaviour, bitwise operators, strings, tables, metatables, varargs, and what is missing.",
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
        summary: "export, import, emit and require; load order and reloads of users; the bundled uikit library's whole API.",
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
        "input" => INPUT,
        "storage" => STORAGE,
        "libraries" => LIBRARIES,
        "console" => CONSOLE,
        "agent" => AGENT,
        "via" => VIA,
        "tests" => TESTS,
        "mcp" => MCP,
        "examples" => EXAMPLES_TEXT,
        "recipes" => RECIPES,
        "limits" => LIMITS,
        "errors" => ERRORS,
        _ => "",
    }
}

/// A topic's prose, with the test step format filled in from the headless runner.
fn prose(id: &str) -> String {
    topic_text(id).replace("{STEPS_HELP}", crate::headless::STEPS_HELP)
}

/// What the reference holds and how to look things up.
pub fn index() -> String {
    let mut out = String::from(
        "# Horizon Studio reference\n\n\
         Everything needed to write, check and test script mods, in topics (prose) and entries (one function, \
         event or widget each). Look things up three ways:\n\n\
         - a topic by its id, e.g. `start`, `events`, `screens`: its prose, then the full entry of everything in it;\n\
         - an entry by its name: an API function as scripts call it (`player.pos`, `on`, `ui.open`), a standard \
         function (`string.format`, `math.rad`, `pairs`), an event (`event:tick`, `event:hud`, `event:custom` for \
         mod-to-mod events), a widget (`widget:button`); a trailing `()` is fine;\n\
         - a search: words, all of which must appear (case does not matter), best matches first.\n\n\
         New here: read `start`, `api`, `events` and `language`, then `recipes`; before testing, `tests`.\n\n\
         ## Topics\n\n",
    );
    for t in TOPICS {
        out.push_str(&format!("- `{}`: {}. {}\n", t.id, t.title, t.summary));
    }
    out.push_str("\n## Entries by topic\n\n");
    for t in TOPICS {
        let names: Vec<&str> = ENTRIES.iter().filter(|e| e.topic == t.id).map(|e| e.name).collect();
        if !names.is_empty() {
            out.push_str(&format!("- `{}`: {}\n", t.id, names.join(", ")));
        }
    }
    out
}

/// A topic's whole text: its prose, then the full entry of each thing in it.
pub fn topic(id: &str) -> Option<String> {
    let id = id.trim();
    let t = TOPICS.iter().find(|t| t.id == id)?;
    let mut out = format!("# {}\n\n{}\n", t.title, prose(t.id).trim());
    let entries: Vec<&Entry> = ENTRIES.iter().filter(|e| e.topic == t.id).collect();
    if !entries.is_empty() {
        out.push_str("\n## Reference\n");
        for e in entries {
            out.push_str(&format!("\n### {}\n\n{}\n", e.name, e.doc.trim()));
        }
    }
    Some(out)
}

/// An entry by its exact name; `player.pos()`, `player.pos(...)` and spaces
/// around it are fine, and any `event:mymod:name` is `event:custom`.
pub fn entry(name: &str) -> Option<&'static Entry> {
    let mut n = name.trim();
    if n.ends_with(')')
        && let Some(open) = n.find('(')
    {
        n = n[..open].trim_end();
    }
    if let Some(e) = ENTRIES.iter().find(|e| e.name == n) {
        return Some(e);
    }
    if let Some(e) = ENTRIES.iter().find(|e| e.name.eq_ignore_ascii_case(n)) {
        return Some(e);
    }
    match n.strip_prefix("event:") {
        Some(rest) if rest.contains(':') => ENTRIES.iter().find(|e| e.name == "event:custom"),
        _ => None,
    }
}

/// Entries and topics with every word of `query` in them (any case), best
/// first: (an entry's name or `topic:<id>`, a short snippet).
pub fn search(query: &str, limit: usize) -> Vec<(String, String)> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits: Vec<(u32, String, String)> = Vec::new();
    for e in ENTRIES {
        if let Some(s) = score(&words, e.name, e.doc) {
            hits.push((s + 5, e.name.to_owned(), snippet(e.doc, &words)));
        }
    }
    for t in TOPICS {
        let text = format!("{}\n{}\n{}", t.title, t.summary, prose(t.id));
        if let Some(s) = score(&words, t.id, &text) {
            hits.push((s, format!("topic:{}", t.id), format!("{}: {}", t.title, t.summary)));
        }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    hits.into_iter().take(limit).map(|(_, n, s)| (n, s)).collect()
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
A script mod is Lua that Horizon Studio runs inside Horizon (Minecraft Java 26.3 in Rust). It is either one file, `<id>.lua`, or a folder `<id>/` with `main.lua` and other `.lua` files that `main.lua` loads with `require`. Both live in the mods folder:

`<game folder>/horizon-mods/.horizon-mod-data/horizon_studio/mods/`

On Linux, Horizon's default game folder is `~/.local/share/horizon` (`$HORIZON_HOME` changes it). The MCP server finds the folder by itself (see `mcp`). Beside `mods/`, the studio keeps `storage/` (what `storage.set` writes), `studio.txt` (mods turned off in the Library), `history.txt` and `scratch.lua` (the console's).

## Ids and files

- The id comes from the file or folder name: lowercased, every character but `a-z`, `0-9`, `_` and `-` becomes `_`, at most 48 characters (`My Mod.lua` is `my_mod`).
- Names that start with `.` are skipped, and so is a folder without `main.lua`. When a folder and a file give the same id, the first in sorted order wins (`x` sorts before `x.lua`).
- Errors in a single-file mod name the file `<id>.lua`; in a folder mod `<id>/main.lua` and `<id>/lib/util.lua`.

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
3. **Hot reload.** Every 0.5 s the studio looks at the mods folder. When a mod's files change (the size or modification time of a single-file mod, or of any `.lua` file up to 3 folders deep in a folder mod), it reloads that mod: its `unload` handlers run, everything it registered is dropped (handlers, commands, timers, its HUD panel and zoom, its models, its screen, its `require`d modules), and its main file runs again in a fresh environment. Then the mods that `import` it reload too. New mods load; mods whose files are deleted unload. Saving in the in-game Editor, in any editor, or with the MCP tool `write_mod` all work the same way.
4. **Unload.** `on("unload", fn)` runs before a reload, when the mod is turned off in the Library, and when its file is deleted. It does not run when the mod fails.

Each mod has its own global environment (`_G`), with the standard library and the API in it. A mod's globals are invisible to other mods and to the console: share values with `export` and `import`, or `emit` events.

## Errors stop only that mod

An error while loading, or in an event handler, a timer, an agent callback or a screen callback stops that mod (its state becomes "failed"): the studio logs `<id> stopped: <message>` and a traceback, shows the line in chat (when the studio's screen is closed), and drops what the mod registered. Other mods keep running. Saving the file again, `reload("<id>")` in the console, or the MCP tool `reload_mod` retries.

Messages carry the file and line: `waypoints.lua:12: attempt to index a nil value (local 'p')`. In a handler they start with the event: `tick: waypoints.lua:12: ...`; in a timer `timer: `, in a screen callback `screen: `, in an agent callback `agent callback: `.

An error in a command's function does not stop the mod: the player sees `/name: <message>` in chat and the console logs it.

Use `pcall` around code that may fail. Running out of steps (an endless loop), a string over 4 MiB and a table over 1,048,576 entries cannot be caught by `pcall`: they always stop the mod.

## Making and installing mods

- In a world, F8 (rebindable in Options > Mods) opens the studio: Console, Editor, Library, Docs and Settings. The Library's New button makes `<id>.lua` from a template; in the Editor, Ctrl+S saves (the mod reloads) and Ctrl+Enter runs the file in the console.
- Dropping a `.lua` file, a mod folder or a `.zip` of either on the game window installs it; a mod of the same id and kind is replaced (at most 256 files and 8 MiB).
- On its first start the studio writes the welcome mod `hello.lua`, and it installs the bundled libraries `uikit` and `viahorizon` once (deleting one keeps it deleted).
- In chat, `/studio` opens the studio, `/studio reload` reloads every mod, `/lua <code>` runs Lua in the console.
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
| normal | loading, `tick`, `frame`, `key`, `chat`, `unload`, mod-to-mod events, timers, commands, agent callbacks, screen callbacks other than `draw`, the console | | `hud.text`, `hud.rect`, `gfx.rect`, `gfx.border`, `gfx.text`, `draw.*` |
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
10. Drawing: `world` handlers and attached models as the world draws; then the HUD (through-wall lines and labels from `world` handlers, `hud` handlers, `hud.panel` boxes); then a script screen (its widgets and `draw`).

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
Horizon Studio runs its own Lua interpreter, written in Rust, with the syntax and semantics of Lua 5.3 except where this page says otherwise. Mods are sandboxed: no files, no network, no other processes, only the functions the studio gives them.

## Statements

Everything in Lua 5.3 but `goto` and labels: `local`, assignment (several targets at once), `function` and `local function` (and methods with `:`), `if`/`elseif`/`else`, `while`, `repeat ... until` (the condition sees the loop body's locals), numeric `for`, generic `for ... in`, `do ... end`, `break` and `return` (the last statement of its block). A `#!` first line is skipped. Comments: `--` to the end of the line, and long comments `--[[ ... ]]` / `--[==[ ... ]==]`.

- `goto` and `::label::` are a syntax error: "goto and labels are not supported".
- `local x <const> = 1` is accepted, but not enforced (`x` can still be assigned). `<close>` is a syntax error ("unknown attribute 'close'").
- There is no `_ENV`. The globals table is `_G` (the mod's own).
- Nesting of blocks and expressions is limited to 80 levels ("chunk has too many syntax levels").

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

Closures with upvalues, varargs (`...`, `select("#", ...)`, `{...}`, `table.pack(...)`), multiple results, methods (`obj:name(...)` passes `obj` as `self`). Tail calls are ordinary calls: they count toward the limit of 200 nested calls (deep recursion raises "stack overflow"; release builds also stop at about 120 nested Lua calls to keep the game's stack safe). A chunk's own `...` are the values it was called with: a mod's main file gets none; a file loaded by `require` gets its module name.

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

## Errors

`error(value[, level])` raises any value; a string gets `file:line: ` in front (level 1, the default, is where `error` was called; 2 its caller; 0 adds nothing). `pcall(f, ...)` returns `true, results...` or `false, error`; `xpcall(f, handler, ...)` calls `handler(error)` and returns `false` and its results. Built-in errors are strings like `mymod.lua:3: attempt to call a nil value (global 'foo')`. An uncaught table error shows as "(error object is a table value)". Running out of steps, a too long string and a too large table cannot be caught.

## The step budget

Every statement, loop round and call costs one step; string and table functions that loop cost one step for each 16 bytes or elements. A mod's main file gets 10,000,000 steps, each handler, timer and callback 2,000,000, a console line 20,000,000. Out of steps, the script stops with "script ran too long and was stopped (an endless loop?)", which `pcall` cannot catch.

## Not there

No coroutines (no `coroutine` table), no `goto`, no `<close>`, no `_ENV`, no `io`, no `debug`, no `os` beyond `clock`, `time`, `date` and `difftime`, no `string.pack`/`unpack`/`packsize`/`dump`, no `dofile`, `loadfile` or `loadstring` (`load` takes text only), no binary chunks, no weak tables or finalizers; `collectgarbage` does nothing. `require` is the studio's own: it loads files of a folder mod (see `require`).
"##;

const STDLIB: &str = r##"
Every standard function a mod (and the console) can call, as Lua 5.3 has it unless its entry says otherwise. All are in each mod's own environment.

- Base: `print`, `type`, `tostring`, `tonumber`, `pairs`, `ipairs`, `next`, `select`, `rawget`, `rawset`, `rawequal`, `rawlen`, `setmetatable`, `getmetatable`, `error`, `assert`, `pcall`, `xpcall`, `load`, `unpack` (as Lua 5.1, also `table.unpack`), `collectgarbage` (does nothing), and the studio's `dump` and `inspect` (a value as readable text). Values: `_G`, `_VERSION`.
- `string`: `len`, `sub`, `upper`, `lower`, `rep`, `reverse`, `byte`, `char`, `format`, `find`, `match`, `gmatch`, `gsub` with Lua patterns, and the studio's `split`, `trim`, `startswith`, `endswith`. Strings have them as methods (`s:upper()`).
- `math`: `abs`, `ceil`, `floor`, `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `exp`, `log`, `fmod`, `modf`, `max`, `min`, `random`, `randomseed`, `tointeger`, `type`, `rad`, `deg`, `ult`, the studio's `clamp`, and from Lua 5.1/5.2 `atan2`, `pow`, `log10`, `sinh`, `cosh`, `tanh`, `frexp`, `ldexp`. Values: `pi`, `huge`, `maxinteger`, `mininteger`.
- `table`: `insert`, `remove`, `concat`, `sort`, `unpack`, `pack`, `move`.
- `utf8`: `char`, `codepoint`, `len`, `offset`, `codes`, and `charpattern`.
- `os`: `clock`, `time`, `date`, `difftime` (UTC only; no other `os` function).
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
| `click(x, y, button)` | function | a press that is not on a widget (and right presses on widgets); `button` is `"left"` or `"right"` |
| `release(x, y, button)` | function | a mouse button released, anywhere |
| `move(x, y)` | function | the pointer moved |
| `wheel(steps)` | function | the wheel turned (positive is up), unless over a slider |
| `key(key, down)` | function | a key went down or up while no text field has focus (names below) |
| `text(t)` | function | text typed while no text field has focus |
| `close()` | function | the player closed the screen (Escape, or the studio's key) |

`click`, `release` and `move` get GUI pixels on a panel screen and dp on a canvas. All of these callbacks are optional, on any screen. Their return values are ignored.

## Keys on a screen

`key(key, down)` gets short names, not the `key.keyboard.*` names of `on("key")`: `"enter"`, `"backspace"`, `"delete"`, `"tab"`, `"left"`, `"right"`, `"up"`, `"down"`, `"home"`, `"end"`, `"pageup"`, `"pagedown"`, a lowercase letter `"a"`..`"z"` or digit `"0"`..`"9"`, `"/"`, and `"other"` for the rest. Escape and the studio's key (F8) close the screen when pressed, so `key` only sees them released (`"escape"`, `"toggle"`). `text(t)` gets typed characters (not while Control is held). Function keys stay with the game.

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
`model.attach(part, spec)` puts an item, or coloured boxes, on a body part of the player (or of another entity) so that it follows the part as it moves and animates, every frame, until removed. `draw.item` draws an item model at a fixed place for one frame.

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

64 attachments per mod, 256 boxes each; boxes count toward the frame's 16384 world quads (6 each). Ids come from one counter for the whole studio: an id is never reused in a session.

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
- Not while the studio's screen or a script screen is open: those take the keyboard and mouse (a screen gets its own `key(key, down)`, see `screens`).
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

## require: files of a folder mod

A folder mod (`<id>/main.lua`) loads its other files with `require("lib.util")` (`lib/util.lua`): the file runs in the mod's own globals with the module name as `...`, and its first result (or `true`) is kept, so the next `require` of the same name returns it without running the file again. Any change to a `.lua` file of the folder reloads the whole mod.

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
- The Editor tab's Ctrl+Enter runs the open file in the console.

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

const TESTS: &str = r##"
`test_mod` runs a list of steps against a mod and reports what each step caused. Headless (the default) it runs on a simulated game with no Horizon needed; with `live: true` it runs the same steps in the running game through the AI link.

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
| `{"player": {...}}` | changes the player: `x`, `y`, `z`, `yaw`, `pitch`, `health`, `food`, `level`, `mode`, `dimension`, `held` (an item id, or null for an empty hand); `"player": null` leaves the world (no ticks, player functions return nil). Then one frame. Headless only |
| `{"block": [x, y, z], "id": "minecraft:stone"}` | sets a block (default air); runs no frame. Headless only |
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
- `expect_output`: a `lua` step's printed lines and returned values, as the console shows them (strings in quotes: returning `"hi"` gives `"hi"` with the quotes).
- `expect_log`: console lines (prints of mods as `[id] text`, errors, tracebacks, info).
- `expect_sent`: what was sent as the player (`chat.say` text, `game.run` commands with their `/`).
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
- No agent: `agent.*`, `world.find` and `world.entities` raise errors. No entities, no ViaHorizon.
- Sounds, particles and items are recorded, not played; ids are checked for form only.
- A command a mod sends with `game.run` reaches mods' commands as in the game; other commands are noted in chat as `(the game would run /...)`.
- The mod's libraries come from the mods folder (when the tool has one), else the bundled examples (`uikit`, `viahorizon`...).

## Live tests

`live: true` writes the files (when given), reloads the mod in the game and runs the steps there. `player`, `block`, `time`, `click`, `type`, `camera` and `pose` steps are headless only: live, use a command step (`/tp`, `/setblock`, `/time set`) or the `horizon_ctl` tool. `expect_drawn`, `expect_sound`, `expect_particle` and `expect_screen` are only noted live (look with `horizon_ctl screenshot`). Live, a `command` step waits 10 frames, a `chat` step 2, a `key` step a frame between press and release and 2 after.

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
3. `write_mod` with the files; read its `check`: whether the mod loads, its error and line. Fix and write again.
4. `test_mod` with steps, headless first. Then `game_status`; when it says `linked: true`, `test_mod` with `live: true`.
5. In the running game: `run_lua`, `send_chat`, `game_log`, `reload_mod`; with Horizon started with `--control`, `horizon_ctl` (`screenshot` returns the image).

## The tools

- `docs` (`topic`, `name`, `search`, all optional): this reference. No arguments: the index (every topic and entry name). `topic`: a topic's prose and the full entry of everything in it (`"all"`: every topic). `name`: one entry, exactly (`"player.pos"`, `"string.format"`, `"event:tick"`, `"widget:button"`). `search`: words, all of which must appear; the best matches with a line of each. The topics are also MCP resources, `horizon-studio://docs/<topic>`. Headless, no game needed.
- `examples` (`id` optional): without an id, the list of bundled examples (`id`, `title`, `description`); with one, its source.
- `game_status` (no arguments): `{"linked": true, "game": {studio, protocol, frame, screen, in_world, ticks, fps, player, mods_dir, log_seq, chat_seq, agents, mods}}`, or `{"linked": false, "why", "mods_dir", "horizon_ctl"}`.
- `list_mods` (`mods_dir` optional): `{"mods_dir", "mods"}`: every mod in the folder, with its live state when the game is linked.
- `read_mod` (`id`, `mods_dir`): `{"id", "files": {path: text}}`.
- `write_mod` (`id`, `files`, `mods_dir`): creates or replaces a mod. `files` is `{"<id>.lua": "..."}` for one file, or `{"main.lua": "...", "lib/util.lua": "..."}` for a folder (a folder mod's other files are kept). It is checked headless first, then written; while the game is linked it reloads there. Returns `{"ok", "written", "check", "live"}` (`check` as `check_mod`; `live` the mod's state in the game, or "not linked: ...").
- `delete_mod` (`id`, `mods_dir`): deletes the file or folder; `{"ok": true, "deleted"}`.
- `check_mod` (`id`, `files` optional, `mods_dir`): loads the mod headless with the libraries it imports and runs 2 frames: `{"ok", "mod", "libraries", "log", "chat"}`; `mod` has `state`, `error`, `line`, `traceback`, `commands`, `handlers`, `timers`, `library`, `imports`... Give `files` to check text not yet written.
- `test_mod` (`id`, `steps`, `files` optional, `live` default false, `mods_dir`): runs test steps (see `tests`) and returns the report.
- `run_lua` (`code`, `live`, `with_mod`, `mods_dir`): runs Lua in the console: in the game while it is linked (default), else headless (with the mod `with_mod` loaded first). Returns `{"ok", "where", "lines": [{"kind", "text"}]}` (headless also `chat`).
- `send_chat` (`line`, `wait_frames` default 20, 1 to 1200): sends a chat line or command in the game as the player, waits, and returns `{"chat", "log"}` that followed. Live only.
- `game_log` (`since_log`, `since_chat`): the console's log and the chat from the game; give the last numbers seen to get only newer lines. Live only.
- `reload_mod` (`id` optional): reloads a mod in the game (and the mods using it), or all; returns the mods' states. Live only.
- `horizon_ctl` (`command`): one `horizon-ctl` command to Horizon itself (`state`, `ui`, `click ID`, `screenshot`, `goto X Y Z`, `inventory`, `command /...`, `help`...); `screenshot` returns the PNG. Needs a game started with `--control` (or `horizon-ctl launch --world NAME --wait`).

A mod's state, as `list_mods`, `game_status` and `check_mod` give it: `id`, `name`, `version`, `author`, `description`, `enabled`, `state` (`running`, `failed`, `off`), `error`, `line`, `traceback`, `main`, `folder`, `library` (whether it exports), `imports`, `commands`, `handlers`, `timers`, `cost_ms`, `reloads`.

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
| Command name | 1 to 32 letters, digits, `_`, `-` |

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
| `no mod 'x' to import` | no such mod in the mods folder | check the id (the file or folder name) |
| `'x' exports nothing: it needs export(...) or to return a table` | the library neither called `export` nor returned a table | add `export({...})` or `return M` |
| `library 'x' failed: ...` / `library 'x' is turned off` | the library is broken or off | fix or turn on the library; this mod reloads with it |
| `library 'x' isn't loaded yet (do they import each other?)` | an import cycle, or an import the load-order scan missed | break the cycle; use `import("x")` with a literal id |
| `require works in folder mods (a folder with main.lua)` | `require` in a single-file mod | make the mod a folder with `main.lua` |
| `module 'lib.x' not found (lib/x.lua)` / `bad module name '../x'` | no such file in the mod's folder / `..` or other characters | the path inside the folder, with `.` or `/` |
| `storage keys are 1 to 64 letters, digits, '_', '-' or '.'` | a key with spaces or symbols | use a simple key |
| `functions cannot be stored` / `value too large to store` / `tables nested too deep to store (or a cycle)` | storing what storage cannot keep | store plain data |
| `this mod stores too many keys (256)` | over 256 keys | keep one table under one key |
| `too many handlers (256)` / `too many timers (256)` | registering in a loop or on every tick | register once at load; cancel timers |
| `a mod attaches at most 64 models` | `model.attach` in a loop | reuse ids with `model.set` |
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
`command(name, fn[, help])`

Adds the chat command `/name`. When the player types `/name ...` in chat, the studio takes it before the server and `fn(rest, words)` runs on the next frame. If `fn` returns a value other than nil, the player's chat shows it as plain text (numbers formatted, tables as `table: 0x...`; no prefix). An error in `fn` does not stop the mod: the chat shows `/name: <message>` and the console logs it with a traceback.

- `name` (string): 1 to 32 letters, digits, `_` or `-`; a leading `/` is dropped. Registering the same name again in the same mod replaces it.
- `fn(rest, words)`: `rest` is the text after the name, trimmed (`""` when there is none); `words` is `rest` split on whitespace, a list (`words[1]` is the first word, nil when none).
- `help` (string, optional): shown in the Library next to the command.
- Returns: nothing.
- Where: anywhere. The command lasts until the mod reloads or stops.
- Errors: `bad argument #1 to 'command' (a name of letters, digits, '_' or '-' expected)`; `/name is the game's own command` (names of vanilla commands and the studio's: `studio`, `lua`, `advancement`, `attribute`, `ban`, `ban-ip`, `banlist`, `bossbar`, `clear`, `clone`, `damage`, `data`, `datapack`, `debug`, `defaultgamemode`, `deop`, `dialog`, `difficulty`, `effect`, `enchant`, `execute`, `experience`, `xp`, `fetchprofile`, `fill`, `fillbiome`, `forceload`, `function`, `gamemode`, `gamerule`, `give`, `help`, `item`, `jfr`, `kick`, `kill`, `list`, `locate`, `loot`, `me`, `msg`, `tell`, `w`, `op`, `pardon`, `pardon-ip`, `particle`, `perf`, `place`, `playsound`, `publish`, `random`, `recipe`, `reload`, `return`, `ride`, `rotate`, `save-all`, `save-off`, `save-on`, `say`, `schedule`, `scoreboard`, `seed`, `setblock`, `setidletimeout`, `setworldspawn`, `spawnpoint`, `spectate`, `spreadplayers`, `stop`, `stopsound`, `stopwatch`, `summon`, `tag`, `team`, `teammsg`, `tm`, `teleport`, `tp`, `tellraw`, `test`, `tick`, `time`, `title`, `transfer`, `trigger`, `version`, `waypoint`, `weather`, `whitelist`, `worldborder`); `/name is already used by <mod id>` (another running mod has it).

`game.run("name args")` runs a mod's command too. In tests: `{"command": "/name args", "expect_chat": "..."}`.

```lua
command("greet", function(rest, words)
  if #words == 0 then return "Usage: /greet <name> [times]" end
  local times = tonumber(words[2]) or 1
  for i = 1, math.min(times, 5) do chat.show("Hello, " .. words[1] .. "!") end
end, "Greets someone: /greet <name> [times]")
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
`require(name)`

Loads another file of a folder mod (`<id>/main.lua` and its other files) and returns its result. The file runs in the mod's own globals with `name` as its `...`; its first result is kept (`true` when it returns nothing), and the next `require` of the same name returns the kept value without running the file again. The cache key is the name as written: `require("lib.util")` and `require("lib/util")` load the same file twice.

- `name` (string): a path inside the mod's folder, with `.` or `/` between folders, without or with `.lua`: `"util"` is `util.lua`, `"lib.util"` and `"lib/util"` are `lib/util.lua`. Letters, digits, `_`, `-`, `.` and `/`; no `..`.
- Returns: the module's value.
- Where: anywhere in a folder mod (usually at load).
- Errors: `require works in folder mods (a folder with main.lua)`; `bad module name '../x'`; `module 'lib.x' not found (lib/x.lua)`; the file's own syntax or runtime errors (`mymod/lib/util.lua:3: ...`).

A change to any `.lua` file of the folder reloads the whole mod. For code shared between mods, use a library (`export`/`import`).

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

In tests: `expect_sent` sees `/time set day`; headless, a command no mod owns is noted in chat as `(the game would run /time set day)`.

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
- `click(x, y, button)`: a press not on a widget (and any right press); `button` is `"left"` or `"right"`.
- `release(x, y, button)`: a button released.
- `move(x, y)`: the pointer moved.
- `wheel(steps)`: the wheel turned (positive up), unless over a slider.
- `key(key, down)`: a key, while no input has focus: `"enter"`, `"backspace"`, `"delete"`, `"tab"`, `"left"`, `"right"`, `"up"`, `"down"`, `"home"`, `"end"`, `"pageup"`, `"pagedown"`, `"a"`..`"z"`, `"0"`..`"9"`, `"/"`, `"other"`; `"escape"` and `"toggle"` only on release (pressing them closes the screen).
- `text(t)`: typed text, while no input has focus.
- `close()`: the player closed the screen (Escape or F8); not called by `ui.close()`.

`click`, `release` and `move` get GUI pixels on a panel screen and dp on a canvas. Return values of callbacks are ignored. An error in any callback stops the mod and closes the screen (`screen: ...`).

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
- `spec` (table), at least one of `item` and `boxes`:
  - `item` (string): an item or block id (`"minecraft:carved_pumpkin"`; `minecraft:` added when missing).
  - `boxes` (list, at most 256): `{from = {x, y, z}, to = {x, y, z}, color = ...}` in model pixels from the part's pivot, x to its right, y up, z forward (`color` default opaque white).
  - `display`: how the item shows: `"head"`, `"hand"`, `"left_hand"`, `"fixed"`, `"ground"`, `"gui"` (default: `"head"` on head and hat, `"hand"` on the right arm, `"left_hand"` on the left arm, else `"fixed"`).
  - `offset = {right, up, forward}` in pixels; `rotation = {pitch, yaw, roll}` in degrees; `scale` a number or `{x, y, z}` (each number at most 4096 either way; lists or `{x =, y =, z =}`).
  - `glow = true`: full bright instead of lit by the world.
  - `first_person = true`: also drawn in first person (the player's own body is not).
  - `visible = false`: kept but not drawn.
  - `target`: `"player"` (default) or an entity id from `world.entities`.
- Returns: the model's id (a number), for `model.set` and `model.remove`.
- Where: anywhere.
- Errors: `bad argument #1 to 'attach' (a body part expected: root, head, body, ...)`; `model.attach: give an item or boxes`; `model.attach: item is an item id such as "minecraft:diamond_sword"`; `model.attach: display is one of head, hand, left_hand, fixed, ground, gui`; `model.attach: boxes[1] needs from and to, each {x, y, z} in pixels`; `model.attach: at most 256 boxes`; `model.attach: offset is {x, y, z}`; `model.attach: scale is out of range`; `model.attach: glow is true or false`; `model.attach: target is "player" or an entity id (world.entities gives them)`; `a mod attaches at most 64 models`.

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
`edit(id)`

Console only. Opens a mod's main file in the studio's Editor tab (after the console line).

- `id` (string).
- Returns: nothing.
- Where: the console only.
- Errors: `bad argument #1 to 'edit' (a mod id expected)`; `no script mod 'x'`.

```lua
edit("waypoints")
```
"##
    ),
    e!(
        "new",
        "console",
        r##"
`new(name)`

Console only. Makes a new single-file mod from the template (`<id>.lua`, the id made from the name) and opens it in the Editor. If the mod exists, the console logs `<id> already exists`.

- `name` (string): the mod's name; the id is made from it.
- Returns: nothing.
- Where: the console only.

```lua
new("My Tool")
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
];
