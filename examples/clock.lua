-- name: Clock
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
