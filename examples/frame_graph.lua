-- name: Frame graph
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
