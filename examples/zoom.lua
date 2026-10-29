-- name: Zoom
-- version: 1.0
-- author: Horizon Studio
-- description: Hold C to zoom in; /zoom <2-10> sets how far. Shows key events and view.zoom.

local factor = storage.get("factor") or 4

on("key", function(key, down)
  if key == "key.keyboard.c" and game.screen() == "playing" then
    view.zoom(down and factor or 1)
  end
end)

command("zoom", function(args)
  local n = tonumber(args)
  if not n or n < 2 or n > 10 then
    return "Usage: /zoom <2-10> (now " .. factor .. ")"
  end
  factor = n
  storage.set("factor", n)
  return "Zoom set to " .. n .. "x"
end, "Sets the zoom: /zoom <2-10>")
