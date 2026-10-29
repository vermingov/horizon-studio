# Horizon Studio

Make mods for [Horizon](https://github.com/horizon-rust/horizon) in Lua, inside the game. Horizon Studio is a
native Horizon mod that adds:

- **A code executor.** A console that runs Lua at once in the running game, like a Lua executor: type
  `player.pos()` and see where you are, or `game.run("time set day")`. `/lua <code>` does the same from chat.
- **Script mods with hot reloading.** A script mod is a `.lua` file in your Horizon folder. Save it, from the in-game
  editor or any other editor, and it reloads at once: no rebuild, no restart.
- **An in-game editor.** Lua highlighting, line numbers, undo and redo, the clipboard, comment toggling and
  auto-indent. A syntax or runtime error marks its line.
- **A mod library.** Every script mod with its state, error, commands and cost per call. Turn mods on and off,
  reload them, open them in the editor, start a new one from a template, or install one of the bundled examples.
- **Drag and drop install.** Drop a `.lua` file, a mod folder or a `.zip` on the game window and it runs.
- **Docs.** The whole script API, in the game.
- **3D drawing, sounds, particles and screens.** Scripts draw lines, boxes, beams and labels in the world, play
  sounds, spawn particles, and open screens of their own with buttons, toggles, sliders and text fields.

A mod that fails stops alone, with its error and line in the Library, and the rest of the game carries on.
Each call into a script runs under a step budget, so an endless loop stops the script instead of freezing the game.

> Horizon Studio is not made or endorsed by the Horizon project. Like every native mod, it is compiled into your
> game and runs with your rights. Scripts are sandboxed (see [Safety](#safety)); the studio itself is not.

## Install

**Script mods are drag and drop.** Once Horizon Studio is in your game, drop a `.lua` file, a mod folder (with
`main.lua`) or a `.zip` of one onto the game window: it installs and runs at once, and the chat says so. Dropping a
newer version updates it. Copying files into the mods folder (Library → Open folder) works too. Nothing is built.

**Horizon Studio itself needs one build.** Horizon no longer runs drop-in code mods (its WebAssembly mods were
retired), so a mod with code is a [native mod](https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md)
that the launcher compiles into the game, once. You don't install Rust or type commands: the launcher brings its own
and does it.

On Linux or macOS, one line downloads it and has the launcher build it (close the launcher first; on macOS use
`"/Applications/Horizon.app/Contents/MacOS/horizon-launcher"` for `horizon-launcher`):

```sh
mkdir -p ~/.cache/horizon-studio && curl -fsSL https://github.com/vermingov/horizon-studio/archive/refs/heads/main.tar.gz | tar -xz -C ~/.cache/horizon-studio && horizon-launcher native-build ~/.cache/horizon-studio/horizon-studio-main/mixins/horizon_studio --trust-native-code
```

Then play the **Native mods** instance. Or, by hand:

1. `python3 tools/studio.py package` writes `target/horizon_studio-0.1.0.hnmod`.
2. Install it as a native mod in an instance, then **Build** ("I trust these mods: build"). The first build takes 10
   to 30 minutes; later ones are faster. From the command line:
   `horizon-launcher native-build mixins/horizon_studio --trust-native-code`.
3. Play the instance. Options → Mods lists **Horizon Studio**, with its key (F8) to rebind.

Horizon Studio is pinned to one Horizon Nightly (see [How it hooks into Horizon](#how-it-hooks-into-horizon)). All
the native mods of one build must pin the same commit; the launcher tells you when they don't.

## Use it

In a world, press **F8** (or type `/studio`). Escape or F8 closes it. The world keeps running behind it.

| Tab | What it does | Keys |
|---|---|---|
| **Console** | Runs Lua at once. An expression's value is printed, tables expanded. What you register here (`on`, `command`, `every`) runs until `clear()`. | Enter run · Up/Down history · Tab complete · Page Up/Down scroll · Ctrl+L clear |
| **Editor** | Edits a mod's file (or `scratch.lua`). Saving reloads the mod. | Ctrl+S save · Ctrl+Enter run in the console · Ctrl+Z/Y undo/redo · Ctrl+C/X/V · Ctrl+A · Ctrl+D duplicate line · Ctrl+/ comment · Tab/Shift+Tab indent · mouse click and drag |
| **Library** | Your script mods and the examples; Open folder shows the mods folder. | Up/Down choose · Enter edit · Space on/off · R reload · N new mod |
| **Docs** | The API. | Up/Down, Page Up/Down, wheel |

Ctrl+1 to Ctrl+4 switch tabs. In chat: `/studio` opens it, `/studio reload` reloads every script mod, and
`/lua <code>` runs code and prints the result in chat.

## Write a script mod

Script mods live in `horizon-mods/.horizon-mod-data/horizon_studio/mods/` in your Horizon game folder:

- `name.lua`: a mod in one file, or
- `name/main.lua`: a folder mod, whose other files load with `require("file")` (`require("lib.util")` reads
  `lib/util.lua`).

The file runs when the mod loads, and again each time it changes. Lines at the top name it in the Library:

```lua
-- name: Coordinates
-- version: 1.0
-- author: you
-- description: Your position in the top-left corner. /coords hides it.

local shown = storage.get("shown") ~= false

on("tick", function()
  local x, y, z = player.block()
  if shown and x then
    hud.panel("top-left", { string.format("XYZ %d / %d / %d", x, y, z), "Facing " .. player.facing() })
  else
    hud.panel(nil)
  end
end)

command("coords", function()
  shown = not shown
  storage.set("shown", shown)
  return shown and "Coordinates shown" or "Coordinates hidden"
end, "Shows or hides the coordinates")
```

The Library's **New mod** writes a file like this to start from.

### Events

`on(event, fn)` registers a handler; a mod may have up to 256.

| Event | When | Arguments |
|---|---|---|
| `tick` | each game tick in a world (20 a second) | |
| `frame` | each frame | seconds since the last frame |
| `hud` | each frame the HUD draws (not under F1): where `hud.text` and `hud.rect` draw | GUI width, height |
| `world` | each frame the world draws: the only place `draw.*` draws | |
| `key` | a key or mouse button goes down or up | `"key.keyboard.r"`, `"key.mouse.left"`; `true` when down |
| `chat` | a chat line arrives from the server | its text |
| `unload` | before the mod reloads or turns off | |

### API

| Function | What it does |
|---|---|
| `on(event, fn)` | Runs `fn` on an event (above). |
| `command(name, fn[, help])` | `/name args` calls `fn(args, words)`; a returned string shows in chat. Vanilla's command names are refused. |
| `after(seconds, fn)`, `every(seconds, fn)`, `cancel(id)` | Timers; `after` and `every` return an id. |
| `require(name)` | Loads another file of a folder mod. |
| `color(r, g, b[, a])` | A color number for `hud` functions. Colors are also `0xAARRGGBB` numbers or `"#rrggbb"` strings. |
| `print(...)` | A line in the Studio console, after the mod's id. |
| `chat.show(text)` | A line in your chat that only you see. |
| `chat.say(text)` | Sends a chat message. |
| `game.run(command)` | Runs a command as you: `game.run("time set day")`. `chat.say` and `game.run` are limited to a burst of 8, then 4 a second. |
| `game.fps()` | Frames per second, the slowest frame, the 1% low. |
| `game.screen()` | `"playing"`, `"chat"`, `"inventory"`, `"paused"`... |
| `game.time()`, `game.ticks()` | Seconds since the studio started; game ticks so far. |
| `player.pos()`, `player.block()`, `player.look()` | Position; the block position; yaw and pitch. `nil` outside a world. |
| `player.facing()`, `player.dimension()` | `"north"`...; `"minecraft:overworld"`... |
| `player.health()`, `player.food()`, `player.held()` | Health and max; food and saturation; the held item's id and count. |
| `player.info()` | All of these in a table, with `level`, `mode` and `slot`. |
| `world.block(x, y, z)` | The block's id, `nil` where nothing is loaded. |
| `world.time()` | Time of day (0 to 23999), game time in ticks, the day number. |
| `world.find(block[, radius])` | The nearest loaded blocks of a kind: `{count = n, nearest = {{pos = {x, y, z}, distance = d}, ...}}`. |
| `world.entities([radius[, type]])` | Mobs, items and orbs near you, nearest first. |
| `hud.panel(corner, lines)` | A box of text in `"top-left"`, `"top-right"`, `"bottom-left"` or `"bottom-right"`, stacked with the game's own; `hud.panel(nil)` removes it. |
| `hud.text(x, y, text[, color[, shadow]])`, `hud.rect(x, y, w, h[, color])` | Draw, inside `on("hud")`. |
| `hud.width(text)`, `hud.size()` | A text's width; the GUI's size, in GUI pixels. |
| `keys.down(key)` | Whether a key is held (`"key.keyboard.r"`). |
| `keys.mapping(name)` | A vanilla mapping (`"key.jump"`): held, and its key's label. |
| `view.zoom(factor)` | Zooms the view, 1 (off) to 10. |
| `draw.line(x1, y1, z1, x2, y2, z2[, color[, width[, through]]])` | A line in the world, inside `on("world")`. `width` is in blocks (0.04). Shapes are solid and hidden behind blocks; with `through` = `true` they show through walls. |
| `draw.box(x1, y1, z1, x2, y2, z2[, color[, width[, through]]])`, `draw.block(x, y, z[, color[, width[, through]]])` | A box's outline; a block's outline. |
| `draw.fill(x1, y1, z1, x2, y2, z2[, color])` | A solid box. |
| `draw.beam(x, z[, color[, width[, y1[, y2]]]])` | A vertical beam (from y -64 to 320 unless given), like a waypoint's. |
| `draw.text(x, y, z, text[, color[, distance]])` | A label at a place, seen through walls; `distance` = `true` adds how far it is. |
| `sound.play(id[, volume[, pitch]])`, `sound.at(id, x, y, z[, volume[, pitch]])` | A sound event (`"entity.experience_orb.pickup"`, `"minecraft:block.bell.use"`): for you, or from a place. |
| `world.particle(id, x, y, z[, dx, dy, dz[, count[, color]]])` | Particles (`"flame"`, `"heart"`, `"note"`, `"dust"` with a color...): `dx dy dz` is their speed, `count` up to 64 scattered around the point. Particles that need a block or an item aren't offered. |
| `ui.open(screen)`, `ui.close()`, `ui.is_open()` | A screen of the mod's own: see [Screens](#screens). |
| `agent.query(cmd)` | An instant read from Horizon's agent control: `"state"`, `"inventory"`, `"looking-at"`, `"block X Y Z"`, `"blocks ..."`, `"screen"`, `"messages"`, as a table. |
| `agent.run(cmd[, fn])` | An agent action, `fn(result, err)` when it is done: `"goto X Y Z --near 2"`, `"break X Y Z"`, `"place X Y Z --item stone"`, `"look-at X Y Z"`, `"select 3"`, `"use-item"`, `"attack"`, `"move forward 2"`... One action runs at a time. |
| `agent.stop()` | Cancels the running action and lets go of the keys. |
| `storage.get(key)`, `storage.set(key, value)` | Values the mod keeps between sessions: `nil`, booleans, numbers, strings and tables of them. |
| `mod.info()` | The mod's id, name, version, author and reload count. |

The agent commands are those of Horizon's `horizon-ctl` ([agent control](https://github.com/horizon-rust/horizon/blob/main/docs/agent-control.md)),
limited to reading the world and acting as the player: nothing that saves, quits or changes options.

In the console only: `help([name])`, `clear()`, `reload([id])`, `mods()`, `edit(id)`, `new(name)`.

### Screens

`ui.open(screen)` shows a screen of the mod's own over the game, as the studio's own shows: the pointer is free, the
world keeps running, Escape closes it. `screen` is a table the studio reads again every frame, so changing it
changes the screen:

| Field | What it is |
|---|---|
| `title`, `width` | A panel with this title (220 GUI pixels wide unless given). |
| `widgets` | The panel's rows, top to bottom (below). |
| `draw(w, h)` | Called each frame after the panel: draw anything with `hud.text`, `hud.rect`... A screen with neither `title` nor `widgets` is yours alone to draw. |
| `click(x, y, button)`, `key(key, down)`, `text(t)`, `wheel(steps)` | Input outside the widgets (`button` is `"left"` or `"right"`; `key` is `"enter"`, `"left"`, `"a"`...). |
| `close()` | Called when the screen closes. |

| Widget | Fields | Calls |
|---|---|---|
| `{type = "label", text}` | `color` | |
| `{type = "button", text}` | | `click(widget)` |
| `{type = "toggle", text, value}` | | `change(value, widget)` |
| `{type = "slider", text, min, max, step, value}` | | `change(value, widget)` |
| `{type = "input", text, value, placeholder}` | | `change(value, widget)` as you type, `submit(value, widget)` on Enter |
| `{type = "space", height}` | | |

```lua
command("settings", function()
  ui.open({
    title = "My mod",
    widgets = {
      { type = "toggle", text = "Enabled", value = enabled, change = function(v) enabled = v end },
      { type = "slider", text = "Radius", min = 1, max = 64, step = 1, value = radius, change = function(v) radius = v end },
      { type = "button", text = "Done", click = function() ui.close() end },
    },
  })
end)
```

### The language

Lua 5.3, interpreted by the studio itself (native mods cannot add libraries, so it is written in plain Rust):
`local`, closures and upvalues, varargs and multiple results, every statement but `goto`, metatables
(`__index`, `__newindex`, `__call`, the arithmetic, comparison, `__concat`, `__len`, `__tostring`, `__pairs`),
`pcall`/`xpcall`/`error`, and the standard library: base functions, `string` (with Lua patterns, `format`,
`gsub`, `gmatch`, and the extras `split`, `trim`, `startswith`, `endswith`), `math` (with `clamp`), `table`, and
`os.clock`, `os.time` and `os.date` (UTC). `dump(value)` pretty-prints a value.

Not there: coroutines, `goto`, bitwise operators, integers as a separate type (numbers are 64-bit floats, printed
without a fraction when whole), `io`, the rest of `os`, `string.pack`, `utf8`. Strings are byte strings, as in Lua.

### Examples

The Library installs these; their source is in [`examples/`](examples).

| Example | Shows |
|---|---|
| Hello | a command (installed on the first run) |
| Coordinates | `hud.panel`, `storage`, a toggle command |
| Clock | `world.time`, `os.date` |
| Compass | drawing in `on("hud")` |
| Frame graph | `on("frame")`, `hud.rect` |
| Zoom | `on("key")`, `view.zoom` |
| Notes | `storage` with tables |
| Night watch | `every`, player state |
| Ore finder | `world.find` |
| Miner | `agent.run`: walking to blocks and mining them, `/mine oak_log 3` |
| Waypoints | `draw.beam`, `draw.text` and `draw.block` through walls, and a menu screen (`/wp`) |
| Selection box | `draw.box` and a size label between two corners (`/pos1`, `/pos2`), `sound.play` |
| Playground | a screen with a toggle, a slider, a button and a text field; `world.particle`, `sound.at` |

## Safety

Scripts run in the studio's interpreter and can do only what the API above offers. They have no file access
(besides their own `storage`), no network, no processes and no way into the game's memory. Each call gets a step
budget (10 million to load, 2 million per event, 20 million per console line), nested calls are capped by the stack
they use, strings by 4 MiB and tables by a million entries. When a script goes past one, that script stops.
Commands and chat a script sends are rate limited, and the agent commands exclude anything that saves, quits or
changes options.

That is protection against mistakes and runaway scripts. Treat a script from someone else like any program that can
play the game as you: it can run commands and act as your player.

## How it hooks into Horizon

The bundle is `mixins/horizon_studio/`: `horizon-mixin.toml`, six modules added to the game's `game.rs`, and one
small helper per hook.

| Module | What it is |
|---|---|
| `lua.rs` (`hstudio_lua`) | The Lua interpreter: lexer, parser with scope resolution, evaluator, tables, budgets |
| `lualib.rs` (`hstudio_lualib`) | The standard library, Lua patterns, `string.format` |
| `studio.rs` (`hstudio`) | Script mods, hot reloading, events, timers, the API, the console |
| `ui.rs` (`hstudio_ui`) | The screen: console, editor, library, docs; input |
| `examples.rs` (`hstudio_examples`) | The bundled examples, generated from `examples/` |
| `bridge.rs` (`hstudio_bridge`) | The only module that knows Horizon's `Game`: the hooks and the game's side of the API |

The hooks, all HEAD hooks:

| Hook | Target | For |
|---|---|---|
| frame | `game::control::poll_control` | each frame with the event loop: scripts, input, agent commands |
| window-input | `game::follow_keymap` | each window event: dropped files to install; while the studio is open, keys, clicks and the wheel (the game gets an event it ignores) |
| overlay | `game::remote::remote_hud` | the studio's screen and the scripts' screens, drawn last |
| world | `game::end::end_geometry` | each frame in a world, with its camera: `draw.*` shapes go into the entity geometry, depth tested |
| chat | `game::remote::chat_line` | `on("chat")` |
| hud, key, command, zoom, list, settings, click | Horizon's [native hook points](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/game/native_hooks.rs) | the HUD, keys, commands, `view.zoom`, the Options → Mods row and page |

The first five go beyond the stable hook points, so the bundle pins one Horizon commit (a Nightly's,
`compatibility.json`): the launcher builds it on that commit. Their bodies are free of macros, which the launcher's
horizon-mixins (`0d6b103`) requires of a hooked function. The studio opens over the game's chat screen, which frees
the pointer and keeps the world running while it takes the keyboard.

Moving to a new Nightly: change `horizon_revision` in `horizon-mixin.toml` and `compatibility.json`, then
`python3 tools/studio.py check`.

## Develop

Everything but `bridge.rs` builds and tests without the game: `src/lib.rs` mounts the modules at the paths they
have in the game.

```sh
cargo test                           # the interpreter, the runtime and the screen, on a fake game
python3 tools/gen_examples.py        # after editing examples/*.lua
python3 tools/studio.py check        # apply on the pinned Horizon and cargo check the game
python3 tools/studio.py build        # build the modded game (fast profile); prints its path
python3 tools/studio.py package      # target/horizon_studio-<version>.hnmod
```

`check` and `build` need Horizon's build requirements (on Linux, `libasound2-dev libudev-dev`). They download the
pinned Horizon from GitHub, or export it from a checkout with `--horizon PATH`, and build horizon-mixins once at its
pinned commit, all under `target/`.

The built game needs resources prepared from your own Minecraft 26.3, as Horizon itself does:
`target/game/fast/horizon-client --resources /path/to/prepared/26.3`.

## Status

0.1.0. The interpreter, the runtime and the screen's logic are covered by tests on a fake game, and the bundle
applies to the pinned Horizon and the modded game compiles. It has not been played in a running game yet: the
drawing, layout and input are untested on screen, so expect rough edges there.

License: MIT.
