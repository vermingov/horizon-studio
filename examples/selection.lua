-- name: Selection box
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
