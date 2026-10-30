-- name: ViaHorizon
-- version: 1.0
-- author: Horizon Studio
-- description: Join servers of any version, as ViaFabricPlus does: every join goes through ViaProxy, which runs beside the game and translates. /via picks the version, or Auto detect. From the title screen: Options, Mods, Horizon Studio.

local kit = import("uikit")

local filter = ""
local shown = {}

local win = kit.window({
  title = "ViaHorizon",
  subtitle = "Join servers of any version through ViaProxy",
  width = 460,
}, function(ui)
  local st = via.status()
  local on = ui.toggle("Join servers through ViaProxy", st.enabled)
  if on ~= st.enabled then
    via.enable(on)
  end
  ui.muted(st.text)
  ui.space(4)

  filter = ui.input("Server version: " .. st.label, filter, { placeholder = "Search: 1.8, 1.12.2, beta, bedrock..." })
  local needle = filter:lower():trim()
  local ids, labels, selected = {}, {}, nil
  for _, v in ipairs(via.versions()) do
    if needle == "" or v.label:lower():find(needle, 1, true) then
      ids[#ids + 1] = v.id
      labels[#labels + 1] = v.label
      if v.id == st.version then
        selected = #ids
      end
    end
  end
  if #ids == 0 then
    ui.muted("No version matches that.")
  else
    local pick = ui.list(labels, selected, { rows = 6 })
    if pick and pick ~= selected and ids[pick] then
      via.version(ids[pick])
      chat.show("ViaHorizon joins as " .. labels[pick] .. " from the next join")
    end
  end
  ui.muted("It applies the next time you join a server. Auto detect asks the server its version.")
  ui.space(4)
  ui.row(2)
  if ui.button("Auto detect") then
    via.version("auto")
  end
  if ui.button("Done", { primary = true }) then
    ui.close()
  end
end)

command("via", function(args)
  if args ~= "" then
    -- /via 1.8.x: choose without the screen.
    local ok, err = pcall(via.version, args)
    if not ok then
      return tostring(err):gsub("^.-: ", "")
    end
    return "ViaHorizon joins as " .. via.status().label .. " from the next join"
  end
  win:open()
end, "Choose the version ViaHorizon joins servers as: /via, or /via 1.8.x")

command("vialog", function()
  for _, line in ipairs(via.log()) do
    print(line)
  end
  return "ViaProxy's last lines are in the Studio console (F8)"
end, "ViaProxy's last lines, in the Studio console")
