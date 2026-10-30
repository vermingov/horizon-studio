-- name: UI Kit Demo
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
