-- name: Coordinates
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
