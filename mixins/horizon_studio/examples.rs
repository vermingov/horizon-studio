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
];
