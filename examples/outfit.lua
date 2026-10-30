-- name: Outfit
-- version: 1.0
-- author: Horizon Studio
-- description: Models that follow your body as you move: /outfit hat, crown, cape, sword, halo or orbit, again to take it off, /outfit off for all. F5 shows them.

-- model.attach(part, spec) puts an item or coloured boxes on a body part.
-- Boxes are in pixels from the part's pivot: x to its right, y up, z forward.
-- The head's pivot is the neck (the head is 8 pixels tall above it); the
-- body's is the neck too (12 pixels down to the waist).

local outfits = {
  hat = function()
    return model.attach("head", { item = "minecraft:carved_pumpkin" })
  end,
  crown = function()
    local gold = "#f4c430"
    return model.attach("head", { boxes = {
      { from = {-4.5, 8, -4.5}, to = {4.5, 9.5, 4.5}, color = gold },
      { from = {-4.5, 9.5, -4.5}, to = {-3, 11, -3}, color = gold },
      { from = {3, 9.5, -4.5}, to = {4.5, 11, -3}, color = gold },
      { from = {-4.5, 9.5, 3}, to = {-3, 11, 4.5}, color = gold },
      { from = {3, 9.5, 3}, to = {4.5, 11, 4.5}, color = gold },
      { from = {-0.75, 9.5, -4.6}, to = {0.75, 11.5, -4.4}, color = "#e0115f" },
    } })
  end,
  cape = function()
    return model.attach("body", {
      boxes = { { from = {-5, -16, -3}, to = {5, 0, -2}, color = "#9b1b30" } },
      rotation = { -8, 0, 0 },
    })
  end,
  sword = function()
    return model.attach("right_arm", { item = "minecraft:diamond_sword" })
  end,
  halo = function()
    return model.attach("head", {
      boxes = {
        { from = {-4, 11, -4}, to = {4, 11.5, -3}, color = "#fff2a8" },
        { from = {-4, 11, 3}, to = {4, 11.5, 4}, color = "#fff2a8" },
        { from = {-4, 11, -3}, to = {-3, 11.5, 3}, color = "#fff2a8" },
        { from = {3, 11, -3}, to = {4, 11.5, 3}, color = "#fff2a8" },
      },
      glow = true,
      first_person = true,
    })
  end,
  orbit = function()
    return model.attach("head", { item = "minecraft:amethyst_shard", display = "fixed", scale = 0.5, glow = true, offset = {8, 6, 0} })
  end,
}

local worn = {}
local angle = 0

-- The orbiting shard moves every tick: model.set changes only what it is given.
on("tick", function()
  if not worn.orbit then return end
  angle = angle + 9
  local a = math.rad(angle)
  model.set(worn.orbit, { offset = { math.cos(a) * 9, 6 + math.sin(a * 2), math.sin(a) * 9 }, rotation = { 0, angle * 2, 0 } })
end)

local names = {}
for name in pairs(outfits) do names[#names + 1] = name end
table.sort(names)

command("outfit", function(rest, args)
  local what = args[1]
  if what == nil or what == "off" then
    model.clear()
    worn = {}
    return "Outfit off"
  end
  if not outfits[what] then
    return "Try /outfit " .. table.concat(names, ", ") .. " or off"
  end
  if worn[what] then
    model.remove(worn[what])
    worn[what] = nil
    return what .. " off"
  end
  worn[what] = outfits[what]()
  local view = player.camera() == "first" and " (F5 to see it)" or ""
  return what .. " on" .. view
end, "Wear models: /outfit hat, crown, cape, sword, halo, orbit, off")
