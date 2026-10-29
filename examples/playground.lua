-- name: Playground
-- version: 1.0
-- author: Horizon Studio
-- description: A screen of widgets for particles and sounds. /playground opens it: a flame trail, a slider, a sound button and a text field.

local trail, density, color = false, 3, "#9a86ff"

on("tick", function()
  if not trail then return end
  local x, y, z = player.pos()
  if x then
    world.particle("dust", x, y + 0.1, z, 0, 0.02, 0, density, color)
  end
end)

local function open()
  ui.open({
    title = "Playground",
    width = 220,
    widgets = {
      { type = "label", text = "Things scripts can do now." },
      { type = "toggle", text = "Particle trail", value = trail, change = function(v) trail = v end },
      { type = "slider", text = "Trail density", min = 1, max = 10, step = 1, value = density, change = function(v) density = v end },
      { type = "button", text = "Ring the bell", click = function()
          local x, y, z = player.pos()
          sound.at("block.bell.use", x, y, z)
          for i = 1, 16 do
            local a = i / 16 * math.pi * 2
            world.particle("note", x + math.cos(a) * 2, y + 1, z + math.sin(a) * 2)
          end
        end },
      { type = "input", text = "Trail colour", value = color, placeholder = "#rrggbb",
        submit = function(v) if v:match("^#%x%x%x%x%x%x$") then color = v end end },
      { type = "space" },
      { type = "button", text = "Close", click = function() ui.close() end },
    },
    close = function() sound.play("ui.button.click", 0.5) end,
  })
end

command("playground", open, "Opens the playground screen")
