-- name: Compass
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
