-- name: Notes
-- version: 1.0
-- author: Horizon Studio
-- description: Notes that stay between sessions, with their coordinates. /note <text> adds one, /notes lists them, /notes clear empties the list.

local notes = storage.get("notes") or {}

command("note", function(args)
  if args == "" then return "Usage: /note <text>" end
  local x, y, z = player.block()
  notes[#notes + 1] = { text = args, at = x and string.format("%d %d %d", x, y, z) or "?", day = select(3, world.time()) }
  storage.set("notes", notes)
  return "Noted (" .. #notes .. ")"
end, "Adds a note: /note <text>")

command("notes", function(args)
  if args == "clear" then
    notes = {}
    storage.set("notes", nil)
    return "Notes cleared"
  end
  if #notes == 0 then return "No notes yet: /note <text>" end
  for i, n in ipairs(notes) do
    chat.show(string.format("%d. %s  (at %s, day %s)", i, n.text, n.at, tostring(n.day or "?")))
  end
end, "Lists notes; /notes clear removes them")
