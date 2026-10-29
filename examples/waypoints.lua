-- name: Waypoints
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
