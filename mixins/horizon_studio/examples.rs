// The bundled example mods, from examples/*.lua by tools/gen_examples.py:
// edit those files and run it again (tests/examples.rs checks this file).

pub struct Example {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub source: &'static str,
}

/// Installed on a first run.
pub const WELCOME: &Example = &EXAMPLES[0];

pub const EXAMPLES: &[Example] = &[
    Example {
        id: r#"hello"#,
        title: r#"Hello"#,
        description: r#"Your first script mod. Type /hello in chat, then open it in the Editor (F8, Library), change the text and press Ctrl+S: it reloads at once."#,
        source: r#"-- name: Hello
-- version: 1.0
-- author: Horizon Studio
-- description: Your first script mod. Type /hello in chat, then open it in the Editor (F8, Library), change the text and press Ctrl+S: it reloads at once.

print("hello loaded")

command("hello", function(args)
  local name = args ~= "" and args or "world"
  return "Hello, " .. name .. "!"
end, "Says hello: /hello [name]")
"#,
    },
    Example {
        id: r#"coords"#,
        title: r#"Coordinates"#,
        description: r#"Your block position and the way you face, in the top-left corner. /coords hides or shows it."#,
        source: r#"-- name: Coordinates
-- version: 1.0
-- author: Horizon Studio
-- description: Your block position and the way you face, in the top-left corner. /coords hides or shows it.

local shown = storage.get("shown") ~= false

local function update()
  if not shown then
    hud.panel(nil)
    return
  end
  local x, y, z = player.block()
  if not x then
    hud.panel(nil)
    return
  end
  local yaw = player.look()
  hud.panel("top-left", {
    string.format("XYZ %d / %d / %d", x, y, z),
    string.format("Facing %s (%.0f)", player.facing(), (yaw % 360 + 360) % 360),
  })
end

on("tick", update)

command("coords", function()
  shown = not shown
  storage.set("shown", shown)
  update()
  return shown and "Coordinates shown" or "Coordinates hidden"
end, "Shows or hides the coordinates")
"#,
    },
    Example {
        id: r#"clock"#,
        title: r#"Clock"#,
        description: r#"The time of day in the world, the day number, and your real clock, in the top-right corner."#,
        source: r#"-- name: Clock
-- version: 1.0
-- author: Horizon Studio
-- description: The time of day in the world, the day number, and your real clock, in the top-right corner.

on("tick", function()
  local time, _, day = world.time()
  if not time then
    hud.panel(nil)
    return
  end
  -- Day time 0 is 6:00 in the morning.
  local minutes = math.floor((time / 1000 + 6) % 24 * 60)
  local h, m = minutes // 60, minutes % 60
  local night = time >= 13000 and time < 23000
  hud.panel("top-right", {
    string.format("Day %d, %02d:%02d %s", day, h, m, night and "(night)" or ""),
    os.date("Real time %H:%M (UTC)"),
  })
end)
"#,
    },
    Example {
        id: r#"compass"#,
        title: r#"Compass"#,
        description: r#"A compass strip at the top of the screen, drawn with hud.rect and hud.text in an on("hud") handler."#,
        source: r##"-- name: Compass
-- version: 1.0
-- author: Horizon Studio
-- description: A compass strip at the top of the screen, drawn with hud.rect and hud.text in an on("hud") handler.

local marks = { [0] = "S", [45] = "SW", [90] = "W", [135] = "NW", [180] = "N", [225] = "NE", [270] = "E", [315] = "SE" }

on("hud", function(w, h)
  local yaw = player.look()
  if not yaw then return end
  local width, cx, y = 180, w / 2, 22
  hud.rect(cx - width / 2, y - 2, width, 12, "#80000000")
  -- Every 15 degrees within 90 of where you look.
  local base = math.floor(yaw / 15) * 15
  for d = base - 90, base + 90, 15 do
    local offset = (d - yaw) * width / 180
    if math.abs(offset) < width / 2 - 6 then
      local label = marks[d % 360]
      local x = cx + offset
      if label then
        hud.text(x - hud.width(label) / 2, y, label, label == "N" and "#ff7a68" or "#ffffff")
      else
        hud.rect(x, y + 3, 1, 3, "#a0ffffff")
      end
    end
  end
  hud.rect(cx, y - 3, 1, 14, "#ff9a86ff")
end)
"##,
    },
    Example {
        id: r#"frame_graph"#,
        title: r#"Frame graph"#,
        description: r#"The last 100 frame times as a bar graph in the bottom-left corner, with the FPS."#,
        source: r##"-- name: Frame graph
-- version: 1.0
-- author: Horizon Studio
-- description: The last 100 frame times as a bar graph in the bottom-left corner, with the FPS.

local times, size = {}, 100

on("frame", function(dt)
  table.insert(times, dt)
  if #times > size then table.remove(times, 1) end
end)

on("hud", function(w, h)
  local x0, y0, scale = 4, h - 50, 600  -- 1/60 s is 10 pixels high
  hud.rect(x0 - 2, y0 - 34, size + 4, 36, "#78000000")
  for i, dt in ipairs(times) do
    local bar = math.min(32, dt * scale)
    local slow = dt > 1 / 30
    hud.rect(x0 + i - 1, y0 - bar, 1, bar, slow and "#f07a68" or "#45c27a")
  end
  local fps = game.fps()
  hud.text(x0, y0 - 44, fps .. " fps", "#ffffff")
end)
"##,
    },
    Example {
        id: r#"zoom"#,
        title: r#"Zoom"#,
        description: r#"Hold C to zoom in; /zoom <2-10> sets how far. Shows key events and view.zoom."#,
        source: r#"-- name: Zoom
-- version: 1.0
-- author: Horizon Studio
-- description: Hold C to zoom in; /zoom <2-10> sets how far. Shows key events and view.zoom.

local factor = storage.get("factor") or 4

on("key", function(key, down)
  if key == "key.keyboard.c" and game.screen() == "playing" then
    view.zoom(down and factor or 1)
  end
end)

command("zoom", function(args)
  local n = tonumber(args)
  if not n or n < 2 or n > 10 then
    return "Usage: /zoom <2-10> (now " .. factor .. ")"
  end
  factor = n
  storage.set("factor", n)
  return "Zoom set to " .. n .. "x"
end, "Sets the zoom: /zoom <2-10>")
"#,
    },
    Example {
        id: r#"notes"#,
        title: r#"Notes"#,
        description: r#"Notes that stay between sessions, with their coordinates. /note <text> adds one, /notes lists them, /notes clear empties the list."#,
        source: r#"-- name: Notes
-- version: 1.0
-- author: Horizon Studio
-- description: Notes that stay between sessions, with their coordinates. /note <text> adds one, /notes lists them, /notes clear empties the list.

local notes = storage.get("notes") or {}

command("note", function(args)
  if args == "" then return "Usage: /note <text>" end
  local x, y, z = player.block()
  notes[#notes + 1] = { text = args, at = x and string.format("%d %d %d", x, y, z) or "?", day = select(3, world.time()) }
  storage.set("notes", notes)
  return "Noted (" .. #notes .. ")"
end, "Adds a note: /note <text>")

command("notes", function(args)
  if args == "clear" then
    notes = {}
    storage.set("notes", nil)
    return "Notes cleared"
  end
  if #notes == 0 then return "No notes yet: /note <text>" end
  for i, n in ipairs(notes) do
    chat.show(string.format("%d. %s  (at %s, day %s)", i, n.text, n.at, tostring(n.day or "?")))
  end
end, "Lists notes; /notes clear removes them")
"#,
    },
    Example {
        id: r#"night_watch"#,
        title: r#"Night watch"#,
        description: r#"Tells you when the sun sets and when your health or food runs low, once each time."#,
        source: r#"-- name: Night watch
-- version: 1.0
-- author: Horizon Studio
-- description: Tells you when the sun sets and when your health or food runs low, once each time.

local warned = {}

local function once(key, cond, text)
  if cond and not warned[key] then
    chat.show(text)
  end
  warned[key] = cond
end

every(1, function()
  local time = world.time()
  if not time then return end
  once("dusk", time >= 12000 and time < 13000, "The sun is setting: monsters soon.")
  local health, max = player.health()
  once("health", health and health <= max * 0.3, "Health low: " .. tostring(health))
  local food = player.food()
  once("food", food and food <= 6, "You are hungry: food " .. tostring(food))
end)
"#,
    },
    Example {
        id: r#"ore_finder"#,
        title: r#"Ore finder"#,
        description: r#"/ores [radius] lists the nearest diamond, iron and gold ores in the loaded world around you, with their direction."#,
        source: r#"-- name: Ore finder
-- version: 1.0
-- author: Horizon Studio
-- description: /ores [radius] lists the nearest diamond, iron and gold ores in the loaded world around you, with their direction.

local ores = {
  "minecraft:diamond_ore", "minecraft:deepslate_diamond_ore",
  "minecraft:iron_ore", "minecraft:deepslate_iron_ore",
  "minecraft:gold_ore", "minecraft:deepslate_gold_ore",
}

local function direction(dx, dz)
  local dirs = { "south", "south-west", "west", "north-west", "north", "north-east", "east", "south-east" }
  local yaw = math.deg(math.atan(-dx, dz))
  return dirs[math.floor((yaw % 360) / 45 + 0.5) % 8 + 1]
end

command("ores", function(args)
  local radius = tonumber(args) or 24
  local px, py, pz = player.pos()
  if not px then return "Not in a world" end
  local lines = {}
  for _, ore in ipairs(ores) do
    local found = world.find(ore, radius)
    local near = found and found.nearest and found.nearest[1]
    if near then
      local p = near.pos
      lines[#lines + 1] = string.format("%s: %d blocks %s, y %d (%d nearby)",
        ore:gsub("minecraft:", ""):gsub("_", " "), math.floor(near.distance), direction(p[1] - px, p[3] - pz), p[2], found.count)
    end
  end
  if #lines == 0 then return "No ores within " .. radius .. " blocks" end
  for _, line in ipairs(lines) do chat.show(line) end
end, "Finds ores near you: /ores [radius]")
"#,
    },
    Example {
        id: r#"miner"#,
        title: r#"Miner"#,
        description: r#"/mine <block> [count] walks to the nearest blocks of a kind and mines them, using Horizon's agent. /mine stop cancels. Try /mine oak_log 3."#,
        source: r#"-- name: Miner
-- version: 1.0
-- author: Horizon Studio
-- description: /mine <block> [count] walks to the nearest blocks of a kind and mines them, using Horizon's agent. /mine stop cancels. Try /mine oak_log 3.

local job = nil

local function finish(text)
  job = nil
  chat.show(text)
end

local function next_block()
  if not job then return end
  if job.done >= job.count then return finish("Mined " .. job.done .. " " .. job.block) end
  local found = world.find(job.block, 32)
  local near = found and found.nearest and found.nearest[1]
  if not near then return finish("No more " .. job.block .. " within 32 blocks") end
  local x, y, z = near.pos[1], near.pos[2], near.pos[3]
  agent.run(string.format("goto %d %d %d --near 3 --timeout 30", x, y, z), function(_, err)
    if not job then return end
    if err then return finish("Could not get there: " .. err) end
    agent.run(string.format("break %d %d %d --timeout 15", x, y, z), function(result, err2)
      if not job then return end
      if err2 then return finish("Could not mine it: " .. err2) end
      job.done = job.done + 1
      next_block()
    end)
  end)
end

command("mine", function(args, words)
  if words[1] == "stop" then
    agent.stop()
    job = nil
    return "Stopped"
  end
  local block = words[1]
  if not block then return "Usage: /mine <block> [count]" end
  if not block:find(":") then block = "minecraft:" .. block end
  job = { block = block, count = tonumber(words[2]) or 1, done = 0 }
  after(0, next_block)
  return "Mining " .. job.count .. " " .. block
end, "Mines blocks for you: /mine <block> [count], /mine stop")

on("unload", function() if job then agent.stop() end end)
"#,
    },
    Example {
        id: r#"waypoints"#,
        title: r#"Waypoints"#,
        description: r#"Beams and labels you can see through walls. /wp add <name>, /wp remove <name>, /wp clear, and /wp for a menu."#,
        source: r##"-- name: Waypoints
-- version: 1.0
-- author: Horizon Studio
-- description: Beams and labels you can see through walls. /wp add <name>, /wp remove <name>, /wp clear, and /wp for a menu.

local colors = { "#9a86ff", "#45c27a", "#f2c48d", "#9fc2ff", "#f07a68" }
local points = storage.get("points") or {}

local function save() storage.set("points", points) end

local function here()
  local x, y, z = player.block()
  return x and { x = x + 0.5, y = y, z = z + 0.5, dim = player.dimension() }
end

local function names()
  local list = {}
  for name in pairs(points) do list[#list + 1] = name end
  table.sort(list)
  return list
end

on("world", function()
  local dim = player.dimension()
  local i = 0
  for _, name in ipairs(names()) do
    local p = points[name]
    i = i + 1
    if p.dim == dim then
      local c = colors[(i - 1) % #colors + 1]
      draw.beam(p.x, p.z, c, 0.2)
      draw.block(p.x, p.y, p.z, c, 0.03, true)
      draw.text(p.x, p.y + 2.5, p.z, name, c, true)
    end
  end
end)

local function menu()
  local widgets = { { type = "label", text = #names() == 0 and "No waypoints yet: /wp add <name>" or "Click one to remove it." } }
  for _, name in ipairs(names()) do
    local p = points[name]
    widgets[#widgets + 1] = {
      type = "button",
      text = string.format("%s  (%d, %d, %d)", name, math.floor(p.x), p.y, math.floor(p.z)),
      click = function()
        points[name] = nil
        save()
        sound.play("ui.button.click")
        menu()
      end,
    }
  end
  widgets[#widgets + 1] = { type = "space" }
  widgets[#widgets + 1] = {
    type = "input", text = "Add one here", placeholder = "a name, then Enter",
    submit = function(name)
      if name ~= "" then points[name] = here(); save(); menu() end
    end,
  }
  ui.open({ title = "Waypoints", width = 240, widgets = widgets })
end

command("wp", function(args, words)
  local what, name = words[1], table.concat(words, " ", 2)
  if not what then menu() return end
  if what == "add" and name ~= "" then
    points[name] = here()
    save()
    return "Waypoint " .. name .. " added"
  elseif what == "remove" and points[name] then
    points[name] = nil
    save()
    return "Waypoint " .. name .. " removed"
  elseif what == "clear" then
    points = {}
    save()
    return "Waypoints cleared"
  end
  return "Usage: /wp, /wp add <name>, /wp remove <name>, /wp clear"
end, "Waypoints: /wp add <name>, /wp remove <name>, /wp clear, /wp for a menu")
"##,
    },
    Example {
        id: r#"selection"#,
        title: r#"Selection box"#,
        description: r#"A building helper. /pos1 and /pos2 mark two corners where you stand; a box shows between them with its size. /sel clear removes it."#,
        source: r##"-- name: Selection box
-- version: 1.0
-- author: Horizon Studio
-- description: A building helper. /pos1 and /pos2 mark two corners where you stand; a box shows between them with its size. /sel clear removes it.

local a, b = nil, nil

local function corner()
  local x, y, z = player.block()
  return x and { x, y, z }
end

on("world", function()
  for _, p in ipairs({ a, b }) do
    if p then draw.block(p[1], p[2], p[3], "#f2c48d", 0.04) end
  end
  if a and b then
    local lo = { math.min(a[1], b[1]), math.min(a[2], b[2]), math.min(a[3], b[3]) }
    local hi = { math.max(a[1], b[1]) + 1, math.max(a[2], b[2]) + 1, math.max(a[3], b[3]) + 1 }
    draw.box(lo[1], lo[2], lo[3], hi[1], hi[2], hi[3], "#9a86ff", 0.05)
    local size = string.format("%d x %d x %d", hi[1] - lo[1], hi[2] - lo[2], hi[3] - lo[3])
    draw.text((lo[1] + hi[1]) / 2, hi[2] + 0.5, (lo[3] + hi[3]) / 2, size, "#ffffff")
  end
end)

local function mark(which)
  local p = corner()
  if not p then return "Not in a world" end
  if which == 1 then a = p else b = p end
  sound.play("block.note_block.hat", 0.6, which == 1 and 1.2 or 1.5)
  return string.format("Corner %d at %d, %d, %d", which, p[1], p[2], p[3])
end

command("pos1", function() return mark(1) end, "Marks the first corner where you stand")
command("pos2", function() return mark(2) end, "Marks the second corner where you stand")
command("sel", function(args)
  if args == "clear" then a, b = nil, nil return "Selection cleared" end
  return "Usage: /pos1, /pos2, /sel clear"
end, "/sel clear removes the selection")
"##,
    },
    Example {
        id: r#"playground"#,
        title: r#"Playground"#,
        description: r#"A screen of widgets for particles and sounds. /playground opens it: a flame trail, a slider, a sound button and a text field."#,
        source: r##"-- name: Playground
-- version: 1.0
-- author: Horizon Studio
-- description: A screen of widgets for particles and sounds. /playground opens it: a flame trail, a slider, a sound button and a text field.

local trail, density, color = false, 3, "#9a86ff"

on("tick", function()
  if not trail then return end
  local x, y, z = player.pos()
  if x then
    world.particle("dust", x, y + 0.1, z, 0, 0.02, 0, density, color)
  end
end)

local function open()
  ui.open({
    title = "Playground",
    width = 220,
    widgets = {
      { type = "label", text = "Things scripts can do now." },
      { type = "toggle", text = "Particle trail", value = trail, change = function(v) trail = v end },
      { type = "slider", text = "Trail density", min = 1, max = 10, step = 1, value = density, change = function(v) density = v end },
      { type = "button", text = "Ring the bell", click = function()
          local x, y, z = player.pos()
          sound.at("block.bell.use", x, y, z)
          for i = 1, 16 do
            local a = i / 16 * math.pi * 2
            world.particle("note", x + math.cos(a) * 2, y + 1, z + math.sin(a) * 2)
          end
        end },
      { type = "input", text = "Trail colour", value = color, placeholder = "#rrggbb",
        submit = function(v) if v:match("^#%x%x%x%x%x%x$") then color = v end end },
      { type = "space" },
      { type = "button", text = "Close", click = function() ui.close() end },
    },
    close = function() sound.play("ui.button.click", 0.5) end,
  })
end

command("playground", open, "Opens the playground screen")
"##,
    },
    Example {
        id: r#"uikit"#,
        title: r#"UI Kit"#,
        description: r#"A library for modern, animated screens, written in plain Lua on gfx.*: windows, buttons, toggles, sliders, text fields, tabs, lists and HUD cards. Other mods use it with local kit = import("uikit"). Open it to see how it works, or copy it to make your own."#,
        source: r##"-- name: UI Kit
-- version: 1.0
-- author: Horizon Studio
-- description: A library for modern, animated screens, written in plain Lua on gfx.*: windows, buttons, toggles, sliders, text fields, tabs, lists and HUD cards. Other mods use it with local kit = import("uikit"). Open it to see how it works, or copy it to make your own.

-- Usage, from another mod:
--
--   local kit = import("uikit")
--   local loud = false
--   local win = kit.window({ title = "My tool" }, function(ui)
--     loud = ui.toggle("Loud", loud)
--     if ui.button("Hello", { primary = true }) then chat.show("hi") end
--   end)
--   command("tool", function() win:open() end)
--
-- The build function runs every frame the window shows (immediate mode):
-- each widget draws itself and returns its new value, so state lives in
-- your own variables. Everything here runs as the mod that calls it.

local kit = { version = "1.0" }

-- ------------------------------------------------------------ colours

-- Colours are {r, g, b}; paint() turns one into the number gfx.* takes.
local function rgb(hex)
  return { tonumber(hex:sub(2, 3), 16), tonumber(hex:sub(4, 5), 16), tonumber(hex:sub(6, 7), 16) }
end

local function mix(a, b, t)
  return { a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t }
end

local function paint(c, alpha)
  return color(c[1], c[2], c[3], math.floor(255 * math.clamp(alpha or 1, 0, 1) + 0.5))
end

-- The studio's own palette. Change it here and every screen made with
-- the kit changes too (the mods that use it reload).
local theme = {
  page = rgb("#0d0e13"),
  surface = rgb("#16181f"),
  plate = rgb("#20222c"),
  raised = rgb("#2b2e3a"),
  rule = rgb("#2a2c36"),
  ink = rgb("#ecedf3"),
  soft = rgb("#c5c7d1"),
  muted = rgb("#8b8f9c"),
  accent = rgb("#9a86ff"),
  accent_hover = rgb("#b0a0ff"),
  danger = rgb("#f07a68"),
  success = rgb("#45c27a"),
  black = rgb("#000000"),
}

kit.theme, kit.rgb, kit.mix, kit.paint = theme, rgb, mix, paint

-- ------------------------------------------------------------ motion and text

local LINE = 1.45 -- a line of text is 1.45 x its size tall
local GAP = 10
local PAD = 22

-- 1 - (1 - t)^3: fast, then settling, like the studio's own motion.
local function ease(t)
  t = math.clamp(t, 0, 1)
  return 1 - (1 - t) ^ 3
end

-- Moves v toward target, a fraction each frame (frame rate independent).
local function approach(v, target, dt, speed)
  return target + (v - target) * math.exp(-dt * (speed or 20))
end

kit.ease, kit.approach = ease, approach

-- The top of a line of text centred on cy.
local function top(cy, size)
  return math.floor(cy - size * LINE / 2 + 0.5)
end

local function continuation(s, i)
  local b = s:byte(i)
  return b ~= nil and b >= 128 and b < 192
end

-- s shortened with "…" to fit w.
local function fit(s, w, size, font)
  s = tostring(s)
  if gfx.measure(s, size, font) <= w then
    return s
  end
  local lo, hi = 0, #s
  while lo < hi do
    local mid = math.floor((lo + hi + 1) / 2)
    if gfx.measure(s:sub(1, mid) .. "…", size, font) <= w then
      lo = mid
    else
      hi = mid - 1
    end
  end
  while lo > 0 and continuation(s, lo + 1) do
    lo = lo - 1
  end
  return s:sub(1, lo) .. "…"
end

-- s broken into lines no wider than w.
local function wrap(s, w, size, font)
  local lines = {}
  for para in (tostring(s) .. "\n"):gmatch("(.-)\n") do
    local line = ""
    for word in para:gmatch("%S+") do
      local try = line == "" and word or line .. " " .. word
      if line ~= "" and gfx.measure(try, size, font) > w then
        lines[#lines + 1] = line
        line = word
      else
        line = try
      end
    end
    lines[#lines + 1] = line
  end
  return lines
end

kit.fit, kit.wrap = fit, wrap

-- How many decimals a slider shows: as many as its step has (2 without one).
local function decimals(step)
  if not step or step <= 0 then
    return 2
  end
  local frac = string.format("%g", step):match("%.(%d+)$")
  return frac and math.min(#frac, 4) or 0
end

-- Byte offsets of the characters around offset i (text fields).
local function before(s, i)
  i = i - 1
  while i > 0 and continuation(s, i + 1) do
    i = i - 1
  end
  return math.max(i, 0)
end

local function after(s, i)
  i = i + 1
  while i < #s and continuation(s, i + 1) do
    i = i + 1
  end
  return math.min(i, #s)
end

-- ------------------------------------------------------------ HUD cards

-- A card of text lines for on("hud"): kit.card(x, y, lines[, opts]).
-- Negative x or y count from the right or bottom edge. opts: title,
-- width, size, font, alpha. Returns the card's width and height.
function kit.card(x, y, lines, opts)
  opts = opts or {}
  local size = opts.size or 13
  local lh = math.floor(size * LINE + 0.5) + 2
  local pad = 12
  local widest = 0
  for _, l in ipairs(lines) do
    widest = math.max(widest, gfx.measure(tostring(l), size, opts.font))
  end
  if opts.title then
    widest = math.max(widest, gfx.measure(opts.title:upper(), 10, "medium"))
  end
  local w = math.max(opts.width or 0, math.ceil(widest) + pad * 2)
  local h = pad * 2 + #lines * lh - 2 + (opts.title and 20 or 0)
  local sw, sh = gfx.size()
  if x < 0 then
    x = sw + x - w
  end
  if y < 0 then
    y = sh + y - h
  end
  local a = opts.alpha or 1
  gfx.rect(x, y, w, h, paint(theme.page, 0.84 * a), 12)
  gfx.border(x, y, w, h, paint(theme.rule, a), 12)
  local ty = y + pad
  if opts.title then
    gfx.text(x + pad, ty - 1, opts.title:upper(), paint(theme.accent, a), 10, "medium")
    ty = ty + 20
  end
  for _, l in ipairs(lines) do
    gfx.text(x + pad, ty, tostring(l), paint(theme.ink, a), size, opts.font)
    ty = ty + lh
  end
  return w, h
end

-- ------------------------------------------------------------ windows

local Window = {}
Window.__index = Window

-- Where the next widget goes: x, y, width, height.
local function place(w, h)
  local r = w.row
  if r then
    local cw = (w.cw - (r.n - 1) * GAP) / r.n
    local x = w.cx + r.i * (cw + GAP)
    r.i = r.i + 1
    r.h = math.max(r.h, h)
    if r.i >= r.n then
      w.cy = r.y + r.h + GAP
      w.row = nil
    end
    return x, r.y, cw, h
  end
  local y = w.cy
  w.cy = w.cy + h + GAP
  return w.cx, y, w.cw, h
end

-- The width the next widget gets.
local function cell(w)
  local r = w.row
  if r then
    return (w.cw - (r.n - 1) * GAP) / r.n
  end
  return w.cw
end

local function hovering(w, x, y, bw, bh)
  return w.live and w.drag == nil and w.mx >= x and w.mx < x + bw and w.my >= y and w.my < y + bh
end

-- A widget's id: its label and its place in the window.
local function next_id(w, label)
  w.n = w.n + 1
  return tostring(label) .. "#" .. w.n
end

-- A value animated toward target, kept per widget.
local function anim(w, key, target, speed)
  local v = w.anim[key]
  if v == nil then
    v = target
  else
    v = approach(v, target, w.dt, speed)
  end
  w.anim[key] = v
  return v
end

-- Pressed on this widget and released on it: a click.
local function clicked(w, id, hot)
  if hot and w.pressed then
    w.active = id
  end
  if w.released and w.active == id then
    if hot and w.spec.sounds ~= false then
      sound.play("minecraft:ui.button.click", 0.25, 1.4)
    end
    return hot
  end
  return false
end

local function label_text(w, x, y, s, size, c, font, width)
  gfx.text(x, y, fit(s, width or w.cw, size, font), paint(c, w.alpha), size, font)
end

-- The functions a window's build function gets, as ui.name(...).
local function widgets(w)
  local ui = {}

  function ui.heading(s)
    local x, y, cw = place(w, 24)
    label_text(w, x, y, s, 16, theme.ink, "medium", cw)
  end

  function ui.text(s, opts)
    opts = opts or {}
    local size = opts.size or 14
    local lh = math.floor(size * LINE + 0.5) + 1
    local lines = wrap(s, cell(w), size, opts.font)
    local x, y = place(w, #lines * lh - 1)
    local c = opts.color and (type(opts.color) == "string" and rgb(opts.color) or opts.color) or theme.soft
    for i, l in ipairs(lines) do
      gfx.text(x, y + (i - 1) * lh, l, paint(c, w.alpha), size, opts.font)
    end
  end

  function ui.muted(s)
    ui.text(s, { size = 12, color = theme.muted })
  end

  function ui.space(h)
    w.cy = w.cy + (h or 8)
  end

  function ui.separator()
    local x, y, cw = place(w, 1)
    gfx.rect(x, y, cw, 1, paint(theme.rule, w.alpha))
  end

  -- The next n widgets share one row.
  function ui.row(n)
    w.row = { n = math.max(1, math.floor(n or 2)), i = 0, y = w.cy, h = 0 }
  end

  function ui.button(label, opts)
    opts = opts or {}
    local x, y, bw, bh = place(w, opts.height or 36)
    local id = next_id(w, label)
    local hot = hovering(w, x, y, bw, bh)
    local hit = clicked(w, id, hot)
    local t = anim(w, id .. ":h", hot and 1 or 0)
    local down = anim(w, id .. ":d", (hot and w.down and w.active == id) and 1 or 0, 30)
    local bg, fg
    if opts.primary then
      bg, fg = mix(theme.accent, theme.accent_hover, t), theme.page
    elseif opts.danger then
      bg, fg = mix(mix(theme.plate, theme.danger, 0.25), mix(theme.plate, theme.danger, 0.4), t), theme.danger
    else
      bg, fg = mix(theme.plate, theme.raised, t), theme.ink
    end
    local inset = down * 1.5
    gfx.rect(x + inset, y + inset, bw - inset * 2, bh - inset * 2, paint(bg, w.alpha), bh / 2)
    local l = fit(label, bw - 24, 13, "medium")
    local lw = gfx.measure(l, 13, "medium")
    gfx.text(math.floor(x + (bw - lw) / 2), top(y + bh / 2, 13), l, paint(fg, w.alpha), 13, "medium")
    return hit
  end

  function ui.toggle(label, value, opts)
    local x, y, cw, h = place(w, 36)
    local id = next_id(w, label)
    local hot = hovering(w, x - 10, y, cw + 20, h)
    if clicked(w, id, hot) then
      value = not value
    end
    local hv = anim(w, id .. ":h", hot and 1 or 0)
    local t = anim(w, id .. ":v", value and 1 or 0, 16)
    if hv > 0.01 then
      gfx.rect(x - 10, y, cw + 20, h, paint(theme.ink, 0.05 * hv * w.alpha), 10)
    end
    label_text(w, x, top(y + h / 2, 14), label, 14, theme.ink, nil, cw - 60)
    local tx, ty = x + cw - 40, y + h / 2 - 11
    gfx.rect(tx, ty, 40, 22, paint(mix(theme.raised, theme.accent, t), w.alpha), 11)
    gfx.rect(tx + 3 + 18 * t, ty + 3, 16, 16, paint(mix(theme.soft, theme.ink, t), w.alpha), 8)
    return value
  end

  function ui.slider(label, value, min, max, opts)
    opts = opts or {}
    min, max = min or 0, max or 1
    local step = opts.step
    local x, y, cw, h = place(w, 46)
    local id = next_id(w, label)
    local hot = hovering(w, x - 6, y, cw + 12, h)
    if hot and w.pressed then
      w.active = id
    end
    local function snap(v)
      if step and step > 0 then
        v = min + math.floor((v - min) / step + 0.5) * step
      end
      return math.clamp(v, math.min(min, max), math.max(min, max))
    end
    if w.active == id and w.down and max ~= min then
      value = snap(min + (max - min) * math.clamp((w.mx - x) / cw, 0, 1))
    elseif hot and w.wheel ~= 0 then
      value = snap(value + (step or (max - min) / 20) * (w.wheel > 0 and 1 or -1))
    end
    value = snap(value or min)
    local shown = string.format(opts.format or ("%." .. decimals(step) .. "f"), value)
    local vw = gfx.measure(shown, 12, "mono")
    label_text(w, x, y, label, 13, theme.ink, nil, cw - vw - 12)
    gfx.text(x + cw - vw, y + 1, shown, paint(theme.muted, w.alpha), 12, "mono")
    local frac = max ~= min and (value - min) / (max - min) or 0
    local f = anim(w, id .. ":f", frac, 24)
    local ty = y + 34
    gfx.rect(x, ty - 2, cw, 4, paint(theme.raised, w.alpha), 2)
    gfx.rect(x, ty - 2, cw * f, 4, paint(theme.accent, w.alpha), 2)
    local hv = anim(w, id .. ":h", (hot or w.active == id) and 1 or 0)
    local kr = 7 + 2 * hv
    local kx = x + cw * f
    if hv > 0.01 then
      gfx.rect(kx - kr - 4, ty - kr - 4, kr * 2 + 8, kr * 2 + 8, paint(theme.accent, 0.22 * hv * w.alpha), kr + 4)
    end
    gfx.rect(kx - kr, ty - kr, kr * 2, kr * 2, paint(theme.ink, w.alpha), kr)
    return value
  end

  -- A text field: returns its text, and true when Enter was pressed.
  function ui.input(label, value, opts)
    opts = opts or {}
    value = tostring(value or "")
    local x, y, cw = place(w, 58)
    local id = next_id(w, label)
    local fy = y + 22
    local hot = hovering(w, x, fy, cw, 36)
    local submitted = false
    if clicked(w, id, hot) and w.focus ~= id then
      w.focus, w.caret = id, #value
    elseif w.pressed and not hot and w.focus == id then
      w.focus = nil
    end
    if w.focus == id then
      local caret = math.clamp(w.caret or #value, 0, #value)
      for _, t in ipairs(w.typed) do
        if #value + #t <= (opts.max or 256) then
          value = value:sub(1, caret) .. t .. value:sub(caret + 1)
          caret = caret + #t
        end
      end
      for _, k in ipairs(w.keys) do
        if k == "backspace" and caret > 0 then
          local b = before(value, caret)
          value, caret = value:sub(1, b) .. value:sub(caret + 1), b
        elseif k == "delete" and caret < #value then
          value = value:sub(1, caret) .. value:sub(after(value, caret) + 1)
        elseif k == "left" then
          caret = before(value, caret)
        elseif k == "right" then
          caret = after(value, caret)
        elseif k == "home" or k == "up" then
          caret = 0
        elseif k == "end" or k == "down" then
          caret = #value
        elseif k == "enter" then
          submitted, w.focus = true, nil
        end
      end
      w.caret = caret
      w.typed, w.keys = {}, {}
    end
    label_text(w, x, y, label, 12, theme.muted, nil, cw)
    local ring = anim(w, id .. ":f", w.focus == id and 1 or 0, 18)
    gfx.rect(x, fy, cw, 36, paint(theme.page, w.alpha), 10)
    gfx.border(x, fy, cw, 36, paint(mix(theme.rule, theme.accent, ring), w.alpha), 10)
    local ty = top(fy + 18, 14)
    if value == "" and w.focus ~= id then
      label_text(w, x + 12, ty, opts.placeholder or "", 14, theme.muted, nil, cw - 24)
    elseif w.focus == id then
      -- Scrolled so the caret shows.
      local cx = gfx.measure(value:sub(1, w.caret), 14)
      local shift = math.max(0, cx - (cw - 26))
      local shown = value
      while shift > 0 and #shown > 0 do
        local n = after(shown, 0)
        shift = shift - gfx.measure(shown:sub(1, n), 14)
        cx = cx - gfx.measure(shown:sub(1, n), 14)
        shown = shown:sub(n + 1)
      end
      gfx.text(x + 12, ty, fit(shown, cw - 24, 14), paint(theme.ink, w.alpha), 14)
      if (w.now * 2) % 2 < 1.3 then
        gfx.rect(x + 12 + cx, fy + 9, 1.5, 18, paint(theme.accent, w.alpha))
      end
    else
      label_text(w, x + 12, ty, value, 14, theme.ink, nil, cw - 24)
    end
    return value, submitted
  end

  -- A segmented control with a sliding highlight; returns the chosen index.
  function ui.tabs(items, current)
    current = math.clamp(math.floor(current or 1), 1, math.max(1, #items))
    local x, y, cw, h = place(w, 36)
    local id = next_id(w, "tabs")
    local seg = cw / math.max(1, #items)
    gfx.rect(x, y, cw, h, paint(theme.page, w.alpha), h / 2)
    for i = 1, #items do
      if clicked(w, id .. ":" .. i, hovering(w, x + (i - 1) * seg, y, seg, h)) then
        current = i
      end
    end
    local px = anim(w, id .. ":x", x + (current - 1) * seg, 18)
    gfx.rect(px + 3, y + 3, seg - 6, h - 6, paint(theme.raised, w.alpha), (h - 6) / 2)
    for i, name in ipairs(items) do
      local sx = x + (i - 1) * seg
      local on = anim(w, id .. ":c" .. i, i == current and 1 or 0)
      local hv = anim(w, id .. ":h" .. i, hovering(w, sx, y, seg, h) and 1 or 0)
      local l = fit(name, seg - 16, 13, "medium")
      local lw = gfx.measure(l, 13, "medium")
      local c = mix(theme.muted, theme.ink, math.max(on, hv * 0.6))
      gfx.text(math.floor(sx + (seg - lw) / 2), top(y + h / 2, 13), l, paint(c, w.alpha), 13, "medium")
    end
    return current
  end

  -- A scrolling list (mouse wheel); returns the selected index.
  function ui.list(items, selected, opts)
    opts = opts or {}
    local rows = math.max(1, opts.rows or 5)
    local rh = 34
    local x, y, cw, h = place(w, rows * rh + 8)
    local id = next_id(w, "list")
    local hot = hovering(w, x, y, cw, h)
    local most = math.max(0, #items - rows)
    local scroll = math.clamp(w.anim[id .. ":s"] or 0, 0, most)
    if hot and w.wheel ~= 0 then
      scroll = math.clamp(scroll - (w.wheel > 0 and 1 or -1), 0, most)
    end
    w.anim[id .. ":s"] = scroll
    gfx.rect(x, y, cw, h, paint(theme.page, w.alpha), 12)
    gfx.border(x, y, cw, h, paint(theme.rule, w.alpha), 12)
    local sel_y = anim(w, id .. ":y", y + 4 + ((selected or 1) - 1 - scroll) * rh, 22)
    if selected and selected > scroll and selected <= scroll + rows then
      gfx.rect(x + 4, sel_y, cw - 8, rh, paint(mix(theme.page, theme.accent, 0.18), w.alpha), 8)
    end
    for i = scroll + 1, math.min(#items, scroll + rows) do
      local ry = y + 4 + (i - 1 - scroll) * rh
      local rhot = hovering(w, x + 4, ry, cw - 8, rh)
      if clicked(w, id .. ":" .. i, rhot) then
        selected = i
      end
      local hv = anim(w, id .. ":h" .. i, rhot and 1 or 0)
      if hv > 0.01 and i ~= selected then
        gfx.rect(x + 4, ry, cw - 8, rh, paint(theme.ink, 0.05 * hv * w.alpha), 8)
      end
      local c = i == selected and theme.accent_hover or theme.ink
      label_text(w, x + 14, top(ry + rh / 2, 14), items[i], 14, c, nil, cw - 36)
    end
    if most > 0 then
      local track = h - 16
      local th = math.max(24, track * rows / #items)
      local ty = y + 8 + (track - th) * scroll / most
      gfx.rect(x + cw - 8, ty, 3, th, paint(theme.muted, 0.5 * w.alpha), 1.5)
    end
    return selected
  end

  -- A bar for 0..1.
  function ui.progress(value, opts)
    opts = opts or {}
    local x, y, cw = place(w, 6)
    local id = next_id(w, "progress")
    local f = anim(w, id, math.clamp(value or 0, 0, 1), 10)
    local c = opts.color and rgb(opts.color) or theme.accent
    gfx.rect(x, y, cw, 6, paint(theme.raised, w.alpha), 3)
    if f > 0.005 then
      gfx.rect(x, y, math.max(6, cw * f), 6, paint(c, w.alpha), 3)
    end
  end

  function ui.close()
    w:close()
  end

  -- The content width, the seconds since the last frame, and the window.
  function ui.width()
    return cell(w)
  end
  function ui.dt()
    return w.dt
  end
  ui.window = w
  return ui
end

-- kit.window(spec, build): a window that build(ui) fills each frame.
-- spec: title, subtitle, width, accent ("#rrggbb"), sounds (false: quiet),
-- close() when it closes. Returns it: win:open(), win:close(),
-- win:toggle(), win:is_open().
function kit.window(spec, build)
  local w = setmetatable({ spec = spec or {}, build = build, anim = {}, shown = false }, Window)
  w.ui = widgets(w)
  return w
end

function Window:open()
  local w = self
  w.opened, w.closing, w.last = game.time(), nil, nil
  w.focus, w.active, w.drag, w.row = nil, nil, nil, nil
  w.mx, w.my, w.down, w.pressed, w.released, w.wheel = -1, -1, false, false, false, 0
  w.typed, w.keys = {}, {}
  ui.open({
    canvas = true,
    draw = function(sw, sh)
      w:frame(sw, sh)
    end,
    move = function(x, y)
      w.mx, w.my = x, y
    end,
    click = function(x, y, button)
      w.mx, w.my = x, y
      if button == "left" then
        w.down, w.pressed = true, true
      end
    end,
    release = function(x, y, button)
      if button == "left" then
        w.down, w.released = false, true
      end
    end,
    wheel = function(steps)
      w.wheel = w.wheel + steps
    end,
    text = function(t)
      w.typed[#w.typed + 1] = t
    end,
    key = function(k, down)
      if down then
        w.keys[#w.keys + 1] = k
      end
    end,
    close = function()
      w.shown = false
      if w.spec.close then
        w.spec.close()
      end
    end,
  })
  w.shown = true
end

function Window:close()
  if self.shown and not self.closing then
    self.closing = game.time()
  end
end

function Window:toggle()
  if self.shown and not self.closing then
    self:close()
  else
    self:open()
  end
end

function Window:is_open()
  return self.shown
end

function Window:frame(sw, sh)
  local w, spec = self, self.spec
  local now = game.time()
  w.dt = w.last and math.clamp(now - w.last, 0, 0.1) or 0
  w.last, w.now = now, now
  -- In over 240 ms, out over 150 ms.
  local a
  if w.closing then
    a = 1 - math.clamp((now - w.closing) / 0.15, 0, 1)
    if a <= 0 then
      w.shown = false
      ui.close()
      if spec.close then
        spec.close()
      end
      return
    end
  else
    a = ease((now - w.opened) / 0.24)
  end
  w.alpha = a
  w.live = not w.closing and a > 0.5
  local saved = theme.accent
  if spec.accent then
    theme.accent = rgb(spec.accent)
  end

  local width = math.min(spec.width or 380, sw - 32)
  local target = w.content_h or spec.height or 200
  w.height = w.height and approach(w.height, target, w.dt, 22) or target
  local height = math.min(w.height, sh - 32)
  -- Dragged by its header.
  local head = spec.title and (spec.subtitle and 74 or 58) or 18
  if w.drag and w.down then
    w.x, w.y = w.mx - w.drag[1], w.my - w.drag[2]
  elseif w.drag then
    w.drag = nil
  end
  local px = math.floor(math.clamp(w.x or (sw - width) / 2, 8, math.max(8, sw - width - 8)))
  local py = math.floor(math.clamp(w.y or (sh - height) / 2, 8, math.max(8, sh - height - 8)))
  if w.x then
    w.x, w.y = px, py
  end
  py = py + math.floor((1 - a) * 8 + 0.5)

  -- A soft shadow: a few wider, fainter layers.
  for i = 1, 4 do
    gfx.rect(px - i * 2, py + i * 3, width + i * 4, height + i * 2, paint(theme.black, 0.07 * a), 18 + i * 2)
  end
  gfx.rect(px, py, width, height, paint(theme.surface, a), 18)
  gfx.border(px, py, width, height, paint(theme.rule, a), 18)

  -- The close button, then the header (whose press starts a drag).
  local cx, cy = px + width - 22 - 14, py + 16
  local chot = w.live and w.mx >= cx and w.mx < cx + 28 and w.my >= cy and w.my < cy + 28
  local ch = anim(w, ":close", chot and 1 or 0)
  if ch > 0.01 then
    gfx.rect(cx, cy, 28, 28, paint(theme.ink, 0.08 * ch * a), 14)
  end
  local xw = gfx.measure("×", 18, "regular")
  gfx.text(cx + (28 - xw) / 2, top(cy + 14, 18), "×", paint(mix(theme.muted, theme.ink, ch), a), 18)
  if chot and w.pressed then
    w.active = ":close"
  end
  if w.released and w.active == ":close" and chot then
    w:close()
  end
  if spec.title then
    local tw = width - PAD * 2 - 36
    gfx.text(px + PAD, py + 18, fit(spec.title, tw, 18, "medium"), paint(theme.ink, a), 18, "medium")
    if spec.subtitle then
      gfx.text(px + PAD, py + 44, fit(spec.subtitle, tw, 12), paint(theme.muted, a), 12)
    end
  end
  local over_head = w.mx >= px and w.mx < px + width and w.my >= py and w.my < py + head
  if w.live and w.pressed and over_head and not chot then
    w.drag = { w.mx - px, w.my - py }
  end

  -- The content.
  w.cx, w.cy, w.cw, w.n, w.row = px + PAD, py + head, width - PAD * 2, 0, nil
  local ok, err = pcall(w.build, w.ui)
  w.row = nil
  theme.accent = saved
  w.content_h = w.cy - GAP + PAD - py
  if w.released then
    w.active = nil
  end
  w.pressed, w.released, w.wheel, w.typed, w.keys = false, false, 0, {}, {}
  if not ok then
    error(err, 0)
  end
end

return kit
"##,
    },
    Example {
        id: r#"uikit_demo"#,
        title: r#"UI Kit Demo"#,
        description: r#"A settings screen built with the UI Kit library: tabs, toggles, a slider, a text field and a list. /kitdemo opens it. It can show a HUD card and outline the nearest block you pick."#,
        source: r##"-- name: UI Kit Demo
-- version: 1.0
-- author: Horizon Studio
-- description: A settings screen built with the UI Kit library: tabs, toggles, a slider, a text field and a list. /kitdemo opens it. It can show a HUD card and outline the nearest block you pick.

local kit = import("uikit")

local saved = storage.get("settings") or {}
local settings = {
  card = saved.card == true,
  fps = saved.fps ~= false,
  size = saved.size or 13,
  title = saved.title or "",
  pick = saved.pick or 1,
  outline = saved.outline == true,
}

local blocks = {
  { "Diamond ore", "minecraft:diamond_ore" },
  { "Deepslate diamond ore", "minecraft:deepslate_diamond_ore" },
  { "Ancient debris", "minecraft:ancient_debris" },
  { "Emerald ore", "minecraft:emerald_ore" },
  { "Gold ore", "minecraft:gold_ore" },
  { "Iron ore", "minecraft:iron_ore" },
  { "Redstone ore", "minecraft:redstone_ore" },
  { "Lapis lazuli ore", "minecraft:lapis_ore" },
  { "Chest", "minecraft:chest" },
  { "Spawner", "minecraft:spawner" },
}
local names = {}
for i, b in ipairs(blocks) do
  names[i] = b[1]
end

local tab = 1
local found = nil

local win = kit.window({
  title = "UI Kit demo",
  subtitle = "A screen made in Lua with import(\"uikit\")",
  width = 440,
}, function(ui)
  tab = ui.tabs({ "HUD card", "Finder", "About" }, tab)
  ui.space(4)
  if tab == 1 then
    settings.card = ui.toggle("Show the card", settings.card)
    settings.fps = ui.toggle("With frames per second", settings.fps)
    settings.size = ui.slider("Text size", settings.size, 11, 18, { step = 1 })
    settings.title = ui.input("Title", settings.title, { placeholder = "Coordinates" })
  elseif tab == 2 then
    ui.muted("Pick a block: the nearest one within 32 blocks gets an outline.")
    settings.pick = ui.list(names, settings.pick, { rows = 5 })
    settings.outline = ui.toggle("Outline it", settings.outline)
    if found then
      ui.muted(string.format("Nearest at %d %d %d, %.1f blocks away", found[1], found[2], found[3], found.d))
    end
  else
    ui.heading("How this works")
    ui.text("uikit is a library mod: plain Lua that draws with gfx.* and exports its functions. This mod imports it, and builds this window every frame from the values it keeps.")
    ui.text("Open uikit in the Editor and change a colour in its theme: this screen reloads with it.")
  end
  ui.space(6)
  ui.row(2)
  if ui.button("Save", { primary = true }) then
    storage.set("settings", settings)
    ui.close()
  end
  if ui.button("Close") then
    ui.close()
  end
end)

command("kitdemo", function()
  win:open()
end, "Opens the UI Kit demo screen")

every(1, function()
  found = nil
  if not settings.outline then
    return
  end
  local hits = world.find(blocks[settings.pick][2], 32)
  local near = hits and hits.nearest and hits.nearest[1]
  if near then
    found = { near.pos[1], near.pos[2], near.pos[3], d = near.distance }
  end
end)

on("world", function()
  if found then
    draw.block(found[1], found[2], found[3], "#9a86ff", 0.04, true)
  end
end)

on("hud", function()
  if not settings.card then
    return
  end
  local x, y, z = player.block()
  if not x then
    return
  end
  local lines = { string.format("%d  %d  %d", x, y, z), "Facing " .. player.facing() }
  if settings.fps then
    lines[#lines + 1] = string.format("%d fps", game.fps())
  end
  local title = settings.title ~= "" and settings.title or "Coordinates"
  kit.card(-16, 96, lines, { title = title, size = settings.size, width = 150 })
end)
"##,
    },
    Example {
        id: r#"viahorizon"#,
        title: r#"ViaHorizon"#,
        description: r#"Join servers of any version, as ViaFabricPlus does: every join goes through ViaProxy, which runs beside the game and translates. /via picks the version, or Auto detect. From the title screen: Options, Mods, Horizon Studio."#,
        source: r#"-- name: ViaHorizon
-- version: 1.0
-- author: Horizon Studio
-- description: Join servers of any version, as ViaFabricPlus does: every join goes through ViaProxy, which runs beside the game and translates. /via picks the version, or Auto detect. From the title screen: Options, Mods, Horizon Studio.

local kit = import("uikit")

local filter = ""
local shown = {}

local win = kit.window({
  title = "ViaHorizon",
  subtitle = "Join servers of any version through ViaProxy",
  width = 460,
}, function(ui)
  local st = via.status()
  local on = ui.toggle("Join servers through ViaProxy", st.enabled)
  if on ~= st.enabled then
    via.enable(on)
  end
  ui.muted(st.text)
  ui.space(4)

  filter = ui.input("Server version: " .. st.label, filter, { placeholder = "Search: 1.8, 1.12.2, beta, bedrock..." })
  local needle = filter:lower():trim()
  local ids, labels, selected = {}, {}, nil
  for _, v in ipairs(via.versions()) do
    if needle == "" or v.label:lower():find(needle, 1, true) then
      ids[#ids + 1] = v.id
      labels[#labels + 1] = v.label
      if v.id == st.version then
        selected = #ids
      end
    end
  end
  if #ids == 0 then
    ui.muted("No version matches that.")
  else
    local pick = ui.list(labels, selected, { rows = 6 })
    if pick and pick ~= selected and ids[pick] then
      via.version(ids[pick])
      chat.show("ViaHorizon joins as " .. labels[pick] .. " from the next join")
    end
  end
  ui.muted("It applies the next time you join a server. Auto detect asks the server its version.")
  ui.space(4)
  ui.row(2)
  if ui.button("Auto detect") then
    via.version("auto")
  end
  if ui.button("Done", { primary = true }) then
    ui.close()
  end
end)

command("via", function(args)
  if args ~= "" then
    -- /via 1.8.x: choose without the screen.
    local ok, err = pcall(via.version, args)
    if not ok then
      return tostring(err):gsub("^.-: ", "")
    end
    return "ViaHorizon joins as " .. via.status().label .. " from the next join"
  end
  win:open()
end, "Choose the version ViaHorizon joins servers as: /via, or /via 1.8.x")

command("vialog", function()
  for _, line in ipairs(via.log()) do
    print(line)
  end
  return "ViaProxy's last lines are in the Studio console (F8)"
end, "ViaProxy's last lines, in the Studio console")
"#,
    },
];
