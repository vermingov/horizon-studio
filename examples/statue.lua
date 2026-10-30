-- name: Statue
-- version: 1.0
-- author: Horizon Studio
-- description: Real 3D models: a textured figure made from a painted skin, a crystal from .obj text and a spinning one beside you. /statue places the figure where you look, /statue crystal swaps it, /statue wear puts the crystal over your head, /statue off clears them.

-- texture.create and texture.paint make a texture from text; mesh.new makes
-- a mesh from boxes, textured with Minecraft's box UV (a skin's layout);
-- mesh.parse reads a model file's text (.obj here; .bbmodel works too, and
-- mesh.load reads either from a folder mod's files). draw.mesh draws a mesh
-- in the world, and model.attach puts one on a body part.

-- A 64x64 skin: a face, a shirt and trousers, painted from rows of letters.
local skin = texture.create(64, 64, "#6b4423")
local palette = {
  s = "#f1c27d", -- skin
  e = "#ffffff", -- eye white
  p = "#3b5dc9", -- pupil
  m = "#9c5b3c", -- mouth
  h = "#4a2f1b", -- hair
}
-- The head's front (8x8 at 8, 8 in the skin's layout).
texture.paint(skin, {
  "hhhhhhhh",
  "hssssssh",
  "ssssssss",
  "sepssepe",
  "ssssssss",
  "sssmmsss",
  "ssssssss",
  "ssssssss",
}, palette, 8, 8)
texture.fill(skin, 0, 8, 8, 8, "#e0ac69")   -- the head's sides and back
texture.fill(skin, 16, 8, 16, 8, "#e0ac69")
texture.fill(skin, 8, 0, 16, 8, "#4a2f1b")  -- the top of the head: hair
texture.fill(skin, 16, 16, 24, 16, "#2e86de") -- the body: a blue shirt
texture.fill(skin, 0, 16, 16, 16, "#34495e")  -- the legs: dark trousers
texture.fill(skin, 40, 16, 16, 16, "#2e86de") -- the arms: sleeves

-- The figure, in pixels, standing on its origin and facing +z (like a player).
local figure = mesh.new({
  texture = skin,
  boxes = {
    { from = { -4, 24, -4 }, to = { 4, 32, 4 }, uv = { 0, 0 } },    -- head
    { from = { -4, 12, -2 }, to = { 4, 24, 2 }, uv = { 16, 16 } },  -- body
    { from = { -8, 12, -2 }, to = { -4, 24, 2 }, uv = { 40, 16 } }, -- right arm
    { from = { 4, 12, -2 }, to = { 8, 24, 2 }, uv = { 40, 16 } },   -- left arm
    { from = { -4, 0, -2 }, to = { 0, 12, 2 }, uv = { 0, 16 } },    -- right leg
    { from = { 0, 0, -2 }, to = { 4, 12, 2 }, uv = { 0, 16 } },     -- left leg
  },
})

-- A crystal as .obj text (in blocks), tinted.
local crystal = mesh.parse([[
v 0 0 0
v 0.25 0.5 0
v 0 0.5 0.25
v -0.25 0.5 0
v 0 0.5 -0.25
v 0 1 0
f 1 2 3
f 1 3 4
f 1 4 5
f 1 5 2
f 6 3 2
f 6 4 3
f 6 5 4
f 6 2 5
]], "obj", { color = "#8e44ad" })

local placed = nil  -- {mesh, x, y, z, yaw}
local worn = nil
local spin = 0

on("frame", function(dt)
  spin = (spin + dt * 60) % 360
end)

on("world", function()
  if placed then
    draw.mesh(placed.mesh, placed.x, placed.y, placed.z, { yaw = placed.yaw })
  end
  local x, y, z = player.pos()
  if x then
    -- A small crystal turning over your shoulder, full bright.
    draw.mesh(crystal, x + 0.8, y + 2.2, z, { yaw = spin, scale = 0.5, glow = true })
  end
end)

local function place(m)
  local x, y, z = player.pos()
  if not x then
    return "Join a world first."
  end
  local yaw = player.look()
  local r = math.rad(yaw)
  -- Two blocks ahead, turned to face you.
  placed = { mesh = m, x = x - math.sin(r) * 2, y = y, z = z + math.cos(r) * 2, yaw = yaw + 180 }
  return "Placed. /statue off clears it."
end

command("statue", function(rest)
  if rest == "off" then
    placed = nil
    if worn then
      model.remove(worn)
      worn = nil
    end
    return "Cleared."
  elseif rest == "crystal" then
    return place(crystal)
  elseif rest == "wear" then
    if worn then
      model.remove(worn)
      worn = nil
      return "Taken off."
    end
    worn = model.attach("head", { mesh = crystal, offset = { 0, 12, 0 }, scale = 0.6, glow = true })
    return "A crystal over your head. F5 to see it."
  end
  return place(figure)
end, "Places a textured figure (/statue crystal, wear, off)")
