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
- **ViaHorizon: join servers of any version.** Like ViaFabricPlus for Fabric: every join goes through
  [ViaProxy](https://github.com/ViaVersion/ViaProxy), which ViaHorizon downloads and runs beside the game, so you can
  play on 1.8, 1.12.2, 1.20 or Beta servers with Horizon's 26.3. You type the server's real address; pick the version
  in Options → Mods → Horizon Studio (or `/via` in game), or leave it on Auto detect.
- **Libraries and modules.** A mod can export functions for other mods to `import`: a UI library, a pathfinder, a
  shared config. Libraries load first, and mods using one reload with it. Mods send each other events, and
  `gfx.*` draws in Horizon's own style (rounded shapes, the Geist fonts) so a UI library can be written in Lua. The
  bundled **UI Kit** is one, installed with the studio.
- **Natrium: Sodium's and Lithium's ideas for Horizon.** A second native mod in the same bundle: particles off
  screen and terrain hidden in thick fog aren't drawn, the server's ticks skip work that changes nothing, and a
  graphics preset sets Horizon's own options for frame time. It draws and ticks the same as without it.

![The console](docs/screenshots/console.png)

| | |
|---|---|
| ![The editor](docs/screenshots/editor.png) | ![The library](docs/screenshots/library.png) |
| ![A mod's error](docs/screenshots/library-error.png) | ![A script's screen](docs/screenshots/screen.png) |
| ![A screen made with the UI Kit library](docs/screenshots/kit.png) | ![ViaHorizon's version picker](docs/screenshots/via-picker.png) |

The studio draws with Horizon's own design system: Geist and Geist Mono, rounded plates, the dark palette, and its
motion (screens fade and rise in, the tab indicator slides, rows arrive one after another, hovers fade). The pictures
are rendered by `cargo test --test preview -- --ignored` with the game's fonts.

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

On Linux or macOS, one line downloads it and has the launcher build it, and the same line updates it (close the
launcher first; on macOS use `"/Applications/Horizon.app/Contents/MacOS/horizon-launcher"` for `horizon-launcher`):

```sh
rm -rf ~/.cache/horizon-studio/horizon-studio-main && mkdir -p ~/.cache/horizon-studio && curl -fsSL https://github.com/vermingov/horizon-studio/archive/refs/heads/main.tar.gz | tar -xz -C ~/.cache/horizon-studio && horizon-launcher native-build ~/.cache/horizon-studio/horizon-studio-main/mixins/horizon_studio --trust-native-code
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

## ViaHorizon

ViaHorizon joins servers of other versions as ViaFabricPlus does, with
[ViaProxy](https://github.com/ViaVersion/ViaProxy) doing the translating: Classic, Alpha, Beta, every release from 1.0
to 26.3, and Bedrock (experimental, as in ViaProxy). It is on from the start, set to **Auto detect** (ViaProxy asks
the server its version).

- **Choose the version** with the button in the top right corner of the multiplayer screen (*Via: 1.8.x*, with a dot
  that is green when ViaProxy is ready): it opens the picker, with ViaHorizon's switch, its state, a search field
  (type `1.12`, `beta`, `bedrock`) and every version; click one, or use the arrow keys and Enter. In a world, `/via`
  opens the same list, and `/via 1.8.x` picks one directly. Options → Mods → Horizon Studio has the switch and
  the version too. The choice applies from the next join.
- **Then join as usual**, from the server list or Direct Connection, with the server's own address. The server list
  pings through ViaProxy too when a version is chosen, so it shows those servers as joinable.
- **Online-mode servers** sign in with the account you play with: ViaProxy asks the game (with the
  [OpenAuthMod](https://github.com/ViaVersionAddons/ViaProxyOpenAuthMod) protocol) and the game joins the server's
  session itself, as for any server. Your account's token never leaves the game, and nothing is set up in ViaProxy.
- **The first time**, ViaHorizon downloads ViaProxy 3.4.14 (49 MB, from ViaVersion's Maven repository) and its
  OpenAuthMod plugin (from GitHub), both checked against pinned SHA-256 checksums. ViaProxy needs Java 17 or newer:
  ViaHorizon uses `JAVA_HOME`, the `java` on your PATH, the runtime the Minecraft launcher installed, or a JDK in the
  usual places; with none of these, it downloads Eclipse Temurin 21 (about 50 MB, from Adoptium, checksum verified).
  Everything goes in `horizon-mods/.horizon-mod-data/horizon_studio/viahorizon/`.
- **ViaProxy listens on 127.0.0.1 only**, on a free port, and stops with the game: a small watcher (a shell loop, or
  PowerShell on Windows) ends it when the game's process is gone, even after a crash. It starts when the game does,
  about 3 seconds after Java is ready, so the first join doesn't wait.
- **When it can't start** (no Java and no download), joins with Auto detect go straight to the server; with a
  version chosen, the join says why. The state line on the settings page says what it is doing, and a click retries.
  `/vialog` prints ViaProxy's last lines to the Studio console.

What ViaProxy translates is the protocol: blocks, items, entities, chat, the world. It does not turn Horizon's 26.3
movement and combat into an old version's, as ViaFabricPlus's emulation does inside a Fabric client, so on a 1.8
PvP server you move and hit as in 26.3, which some anticheats notice. Joining as 26.3 skips ViaProxy.

## Natrium

Natrium does for Horizon what Sodium and Lithium do for Java Minecraft: the same picture and the same game, with
less work a frame and a tick. It comes with Horizon Studio, has its own row in Options → Mods, and is on from the
start. Its page has a switch for each part, the preset, and what it has skipped so far.

- **Particle culling.** Horizon builds every particle's quad each frame, behind the camera too (up to 16,384).
  Natrium tests each one against the frame's view first: those outside it cost nothing. Those in view are built
  exactly as Horizon builds them.
- **Fog culling.** Where the fog ends in its own colour, as in the Nether, under water or lava, in powder snow, with
  blindness or darkness, a chunk section wholly past the fog's end is invisible, so it isn't drawn. It never applies
  where the sky shows through (the Overworld's clear air, the End) or with a shader pack.
- **Faster ticks**, as Lithium's: the same work in the same order, with less of it. Item merging looks each
  stack's size up once instead of up to four times per neighbour, and keeps its list of neighbours between items
  (moving drops try to merge every other tick). Villagers looking for items to pick up ask an item where it is before
  what it is, so a village near an item farm no longer costs a tag lookup per villager and item each tick. Swimming
  mobs (fish, squid, drowned) search for paths without a new hash map for every node.
- **The preset** (a button on its page) sets Horizon's own options as their descriptions advise for frame time:
  Entity Culling on (it is off by default), Cave Culling on, and at most 16 chunk updates and 4 MiB of uploads a
  frame, which keeps frames even while chunks load. It changes nothing else and saves with your options.
- **Block face culling** (Sodium's skipping of faces turned away from the camera) is already in Horizon and on by
  default, so Natrium doesn't add it twice; its page shows its state.

Each part checks its own switch at the moment it runs, so turning one off in Options → Mods takes effect on the next
frame or tick. To see what it gains on your machine, compare the FPS counter (F3) with Natrium on and off.

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
| `"name:event"` | another mod (or this one) called `emit("name:event", ...)`, the next frame | what it passed |

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
| `export(value)` | What the mod offers other mods. A main file that `return`s a table exports it too. See [Libraries](#libraries). |
| `import(id)` | Another mod's exports. It loads before this mod, and this mod reloads when it does. |
| `emit(event, ...)` | Calls every mod's `on(event, fn)` handlers next frame, with these values; `event` has a `:` (`"mymod:changed"`). |
| `json.encode(value[, pretty])`, `json.decode(text)` | Tables to JSON text and back; `decode` returns `nil` and the error for bad JSON. |
| `gfx.rect(x, y, w, h[, color[, radius]])`, `gfx.border(x, y, w, h[, color[, radius]])` | A rounded rectangle; a 1 dp rounded outline. In dp, Horizon's logical pixels, inside `on("hud")` or a screen's `draw`. |
| `gfx.text(x, y, text[, color[, size[, font]]])` | Geist text, `font` `"regular"`, `"medium"` or `"mono"`, `size` in dp (14); `y` is the top of its line, 1.45 × `size` tall. Returns its width. |
| `gfx.measure(text[, size[, font]])`, `gfx.size()` | A text's width in dp; the screen's size in dp. |
| `via.status()`, `via.versions()` | ViaHorizon: `{enabled, version, label, text, ready}`; the versions it joins as, newest first, `{id, label}`. |
| `via.enable(on)`, `via.version(id)`, `via.log()` | Join through ViaProxy or straight; the version from the next join (`"1.8.x"`, `"auto"`...); ViaProxy's last lines. |
| `gfx.mouse()` | While the mod's screen shows: the pointer's x and y in dp, whether the left and right buttons are held. |

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
| `canvas` | `true`: the screen is yours to draw, in dp with `gfx.*`. `draw(w, h)` gets the size in dp, and the pointer calls come in dp too, with `move(x, y)` and `release(x, y, button)` added. `backdrop = false` drops the dimmed backdrop. |

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

### Libraries

A library is a mod like any other that exports something. Other mods `import` it by id:

```lua
-- mods/greeter.lua
local M = {}
function M.greet(name) return "Hello, " .. name end
return M            -- or export(M)
```

```lua
-- mods/my_mod.lua
local greeter = import("greeter")
command("hi", function(args) return greeter.greet(args) end)
```

- The studio reads which mods a file imports and loads libraries first. When a library reloads, fails or turns
  off, the mods that import it reload with it: they never hold stale functions, and a broken library stops them
  with its reason. Fix it and they start again.
- A library's functions run as the mod that calls them: what they draw, register with `on` or `command`, keep in
  `storage` or show on a screen belongs to that mod. A library keeps its own state between calls (its locals), so
  it can be a shared service: a registry, an event bus, a cache.
- Mods talk without importing each other with `emit("name:event", ...)` and `on("name:event", fn)`.
- Folder mods can still split themselves into files with `require`; `import` is between mods.

#### The UI Kit

`uikit` comes installed: a library, in plain Lua, of animated screens in the studio's style, made with `gfx.*`
and a canvas screen. Open it in the Editor to see how it works, change its `theme`, or copy it to start your own.

```lua
local kit = import("uikit")
local loud, volume, name = false, 5, ""

local win = kit.window({ title = "My tool", subtitle = "Made with the UI Kit", width = 400 }, function(ui)
  loud = ui.toggle("Loud", loud)
  volume = ui.slider("Volume", volume, 0, 10, { step = 1 })
  name = ui.input("Name", name, { placeholder = "Steve" })
  ui.row(2)
  if ui.button("Say it", { primary = true }) then chat.show("Hello, " .. name) end
  if ui.button("Close") then ui.close() end
end)

command("tool", function() win:open() end)
```

The build function runs every frame the window shows. Each widget draws itself and returns its new value, so the
state stays in your own variables.

| Function | What it does |
|---|---|
| `kit.window(spec, build)` | A window: `spec` has `title`, `subtitle`, `width`, `accent` (`"#rrggbb"`), `sounds` (`false` for quiet) and `close()`. Returns `win` with `win:open()`, `win:close()`, `win:toggle()`, `win:is_open()`. Its header drags it. |
| `ui.heading(text)`, `ui.text(text[, opts])`, `ui.muted(text)` | Text, wrapped to the width; `opts` has `size`, `color`, `font`. |
| `ui.button(label[, opts])` | `true` when clicked; `opts.primary`, `opts.danger`, `opts.height`. |
| `ui.toggle(label, value)` | A switch; returns the value. |
| `ui.slider(label, value, min, max[, opts])` | Drag or scroll; `opts.step`, `opts.format`. Returns the value. |
| `ui.input(label, value[, opts])` | A text field; returns the text and `true` when Enter was pressed. `opts.placeholder`, `opts.max`. |
| `ui.tabs(names, current)`, `ui.list(items, selected[, opts])` | A segmented control; a scrolling list (`opts.rows`). Both return the chosen index. |
| `ui.progress(fraction)`, `ui.separator()`, `ui.space([h])`, `ui.row(n)` | A bar; a line; a gap; the next `n` widgets side by side. |
| `ui.close()`, `ui.width()`, `ui.dt()` | Close with the exit animation; the content width; seconds since the last frame. |
| `kit.card(x, y, lines[, opts])` | A HUD card, inside `on("hud")`; negative `x` or `y` count from the right or bottom. `opts`: `title`, `width`, `size`. |
| `kit.theme`, `kit.rgb`, `kit.mix`, `kit.paint`, `kit.fit`, `kit.wrap`, `kit.ease` | Its palette and helpers, for widgets of your own. |

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
| UI Kit | a library mod: `gfx.*`, a canvas screen, `export` (installed with the studio) |
| ViaHorizon | the ViaHorizon screen: `via.*`, a searchable list made with the UI Kit (`/via`, installed with the studio) |
| UI Kit demo | `import("uikit")`: tabs, toggles, a slider, a text field, a list and a HUD card (`/kitdemo`) |

## Safety

Scripts run in the studio's interpreter and can do only what the API above offers. They have no file access
(besides their own `storage`), no network, no processes and no way into the game's memory. Each call gets a step
budget (10 million to load, 2 million per event, 20 million per console line), nested calls are capped by the stack
they use, strings by 4 MiB and tables by a million entries. When a script goes past one, that script stops.
Commands and chat a script sends are rate limited, and the agent commands exclude anything that saves, quits or
changes options.

That is protection against mistakes and runaway scripts. Treat a script from someone else like any program that can
play the game as you: it can run commands and act as your player.

ViaHorizon, part of the studio's native code rather than a script, downloads and runs programs: ViaProxy (GPL-3.0,
by the ViaVersion team) and its OpenAuthMod plugin, at pinned versions with pinned checksums, and a Java runtime
from Adoptium when there is none (checked against Adoptium's checksum). They are downloaded from their publishers,
not shipped here. ViaProxy runs as you, listening only on the loopback. Turn ViaHorizon off in Options → Mods →
Horizon Studio to join servers directly and keep ViaProxy from starting.

## How it hooks into Horizon

The bundle is `mixins/horizon_studio/`: `horizon-mixin.toml`, the studio's modules added to the game's `game.rs`,
ViaHorizon's to its network code, and one small helper per hook.

| Module | What it is |
|---|---|
| `lua.rs` (`hstudio_lua`) | The Lua interpreter: lexer, parser with scope resolution, evaluator, tables, budgets |
| `lualib.rs` (`hstudio_lualib`) | The standard library, Lua patterns, `string.format` |
| `studio.rs` (`hstudio`) | Script mods, hot reloading, events, timers, the API, the console |
| `ui.rs` (`hstudio_ui`) | The screen: console, editor, library, docs; input |
| `examples.rs` (`hstudio_examples`) | The bundled examples, generated from `examples/` |
| `bridge.rs` (`hstudio_bridge`) | The only module that knows Horizon's `Game`: the hooks and the game's side of the API |
| `paint.rs` (`ui_theme::hstudio_paint`) | In the game's library: Horizon's design system's shapes and fonts, for the studio and `gfx.*` |
| `via.rs` (`net::hstudio_via`) | In the game's library: ViaHorizon's settings, Java, the downloads, ViaProxy's process, which joins it routes |
| `via_auth.rs` (`net::hstudio_via_auth`) | ViaHorizon's sign-in with the account a join is made as |
| `via_picker.rs`, `via_menu.rs` (`hstudio_via_picker`, `hstudio_via_menu`) | The multiplayer screen's button and the version picker (drawn with the design system's shapes and fonts) |
| `via_net.rs` (`horizon_protocol::hstudio_via_net`) | In the protocol crate: ViaProxy's internal host, the redirect, OpenAuthMod's answers |
| `natrium.rs` (`hstudio_natrium`) | Natrium's settings, its Options → Mods row and page, the preset |
| `natrium_flags.rs` (`horizon_core::natrium`) | In the core crate, which the renderer and the server both read: Natrium's switches and counts |
| `natrium_particles.rs` (`particles::natrium_particles`) | In the renderer: Horizon's particle loop, with the view test first |
| `natrium_items.rs`, `natrium_villagers.rs`, `natrium_paths.rs` | Next to the code they speed up: item merging, villagers' item pickup, swimmers' path search |

The hooks, HEAD hooks but where it says otherwise (via-address, via-srv, via-recv and Natrium's particle, merge,
pickup and swim hooks can answer in their function's place):

| Hook | Target | For |
|---|---|---|
| frame | `game::control::poll_control` | each frame with the event loop: scripts, input, agent commands |
| window-input | `game::follow_keymap` | each window event: dropped files to install; while the studio is open, keys, clicks and the wheel (the game gets an event it ignores) |
| overlay | `game::remote::remote_hud` | the studio's screen and the scripts' screens, drawn last |
| world | `game::end::end_geometry` | each frame in a world, with its camera: `draw.*` shapes go into the entity geometry, depth tested |
| chat | `game::remote::chat_line` | `on("chat")` |
| via-address | `horizon_protocol::servers::parse_address` | on the threads that join and ping servers, an address ViaHorizon routes parses as ViaProxy's internal host (the server and version, for ViaProxy to read from the handshake) |
| via-srv | `horizon_protocol::resolver::srv_redirect` | that host's SRV lookup answers with the local ViaProxy, so the connection goes there |
| via-recv | `horizon_protocol::connection::Connection::recv` | during login, ViaProxy's OpenAuthMod sign-in requests are answered |
| via-menu-draw | `game::chat::Game::draw_link_prompt` | after the title screen draws: ViaHorizon's button on the multiplayer list, and its picker |
| via-startup | `game::startup_menu::Loading::draw_menu` (a RETURN hook) | the same on the startup menu's title screen, before the game has loaded |
| via-session, via-transfer | `net::Session::connect`, `connect_transferred` | the account the join is made as, for that sign-in |
| natrium-view | `horizon_client_render::gfx::Renderer::camera_frustum` (RETURN) | the frame's view, for particle culling |
| natrium-particles-render | `horizon_client_render::particles::ParticleEngine::render` | Natrium's particle loop in its place, while particle culling is on |
| natrium-frame | `horizon_client_render::gfx::Renderer::render` | this frame's fog, and whether it hides what is past it |
| natrium-fog | `horizon_client_render::gfx::resident::in_frustum` (RETURN) | a section wholly past that fog counts as out of view |
| natrium-merge, natrium-pickups, natrium-swim | `entities::merge_with_neighbours`, `game::villagers::Game::villager_pickups`, `mobs::path::Finder::swim_neighbors` | Natrium's versions in their place, while faster ticks are on |
| hud, key, command, zoom, list, settings, click | Horizon's [native hook points](https://github.com/horizon-rust/horizon/blob/main/crates/horizon-client/src/game/native_hooks.rs) | the HUD, keys, commands, `view.zoom`, the Options → Mods row and page |

The first five, ViaHorizon's and Natrium's go beyond the stable hook points, so the bundle pins one Horizon commit (a Nightly's,
`compatibility.json`): the launcher builds it on that commit. Their bodies are free of macros, which the launcher's
horizon-mixins (`0d6b103`) requires of a hooked function. The studio opens over the game's chat screen, which frees
the pointer and keeps the world running while it takes the keyboard.

Moving to a new Nightly: change `horizon_revision` in `horizon-mixin.toml` and `compatibility.json`, then
`python3 tools/studio.py check`.

## Develop

Everything but `bridge.rs` and `via_auth.rs` builds and tests without the game: `src/lib.rs` mounts the modules at
the paths they have in the game (`tests/shim/horizon-protocol` stands in for the protocol crate ViaHorizon adds to).

```sh
cargo test                           # the interpreter, the runtime and the screen, on a fake game
python3 tools/gen_examples.py        # after editing examples/*.lua
python3 tools/studio.py check        # apply on the pinned Horizon and cargo check the game
python3 tools/studio.py build        # build the modded game (fast profile); prints its path
python3 tools/studio.py package      # target/horizon_studio-<version>.hnmod
python3 tools/via_e2e.py             # ViaHorizon end to end (after check): see below
```

`tools/via_e2e.py` builds `tools/via_probe.rs` inside the staged game and joins two fake 1.8 servers
(`tools/fake_server_1_8.py`, offline and online mode) through the real ViaProxy, as 1.8.x and with Auto detect,
using the game's own network code: the server list's ping, the join up to the configuration's end, the online
server's encryption with ViaProxy's sign-in request answered, and the refusal without an account. It needs Java
17+ (or downloads it) and Python's `cryptography`.

`check` and `build` need Horizon's build requirements (on Linux, `libasound2-dev libudev-dev`). They download the
pinned Horizon from GitHub, or export it from a checkout with `--horizon PATH`, and build horizon-mixins once at its
pinned commit, all under `target/`.

The built game needs resources prepared from your own Minecraft 26.3, as Horizon itself does:
`target/game/fast/horizon-client --resources /path/to/prepared/26.3`.

## Status

0.1.0. The interpreter, the runtime and the screen's logic are covered by tests on a fake game, and the bundle
applies to the pinned Horizon and the modded game compiles. ViaHorizon's joins run end to end, through the game's
network code and the real ViaProxy, against fake 1.8 servers. It has not been played in a running game yet: the
drawing, layout and input are untested on screen, and ViaHorizon hasn't met a real server, so expect rough edges.
Natrium's particle loop and item merging are checked against Horizon's own (the same vertices, the same items, tick
after tick, in the staged game); what it gains in frames and ticks has not been measured on a real machine yet.

License: MIT.
