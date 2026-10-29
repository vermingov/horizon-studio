-- name: Night watch
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
