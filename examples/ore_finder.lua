-- name: Ore finder
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
