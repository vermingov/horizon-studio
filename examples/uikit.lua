-- name: UI Kit
-- version: 1.0
-- author: Horizon Studio
-- description: A library for modern, animated screens, written in plain Lua on gfx.*: windows, buttons, toggles, sliders, text fields, tabs, lists and HUD cards. Other mods use it with local kit = import("uikit"). Open it to see how it works, or copy it to make your own.

-- Usage, from another mod:
--
--   local kit = import("uikit")
--   local loud = false
--   local win = kit.window({ title = "My tool" }, function(ui)
--     loud = ui.toggle("Loud", loud)
--     if ui.button("Hello", { primary = true }) then chat.show("hi") end
--   end)
--   command("tool", function() win:open() end)
--
-- The build function runs every frame the window shows (immediate mode):
-- each widget draws itself and returns its new value, so state lives in
-- your own variables. Everything here runs as the mod that calls it.

local kit = { version = "1.0" }

-- ------------------------------------------------------------ colours

-- Colours are {r, g, b}; paint() turns one into the number gfx.* takes.
local function rgb(hex)
  return { tonumber(hex:sub(2, 3), 16), tonumber(hex:sub(4, 5), 16), tonumber(hex:sub(6, 7), 16) }
end

local function mix(a, b, t)
  return { a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t }
end

local function paint(c, alpha)
  return color(c[1], c[2], c[3], math.floor(255 * math.clamp(alpha or 1, 0, 1) + 0.5))
end

-- The studio's own palette. Change it here and every screen made with
-- the kit changes too (the mods that use it reload).
local theme = {
  page = rgb("#0d0e13"),
  surface = rgb("#16181f"),
  plate = rgb("#20222c"),
  raised = rgb("#2b2e3a"),
  rule = rgb("#2a2c36"),
  ink = rgb("#ecedf3"),
  soft = rgb("#c5c7d1"),
  muted = rgb("#8b8f9c"),
  accent = rgb("#9a86ff"),
  accent_hover = rgb("#b0a0ff"),
  danger = rgb("#f07a68"),
  success = rgb("#45c27a"),
  black = rgb("#000000"),
}

kit.theme, kit.rgb, kit.mix, kit.paint = theme, rgb, mix, paint

-- ------------------------------------------------------------ motion and text

local LINE = 1.45 -- a line of text is 1.45 x its size tall
local GAP = 10
local PAD = 22

-- 1 - (1 - t)^3: fast, then settling, like the studio's own motion.
local function ease(t)
  t = math.clamp(t, 0, 1)
  return 1 - (1 - t) ^ 3
end

-- Moves v toward target, a fraction each frame (frame rate independent).
local function approach(v, target, dt, speed)
  return target + (v - target) * math.exp(-dt * (speed or 20))
end

kit.ease, kit.approach = ease, approach

-- The top of a line of text centred on cy.
local function top(cy, size)
  return math.floor(cy - size * LINE / 2 + 0.5)
end

local function continuation(s, i)
  local b = s:byte(i)
  return b ~= nil and b >= 128 and b < 192
end

-- s shortened with "…" to fit w.
local function fit(s, w, size, font)
  s = tostring(s)
  if gfx.measure(s, size, font) <= w then
    return s
  end
  local lo, hi = 0, #s
  while lo < hi do
    local mid = math.floor((lo + hi + 1) / 2)
    if gfx.measure(s:sub(1, mid) .. "…", size, font) <= w then
      lo = mid
    else
      hi = mid - 1
    end
  end
  while lo > 0 and continuation(s, lo + 1) do
    lo = lo - 1
  end
  return s:sub(1, lo) .. "…"
end

-- s broken into lines no wider than w.
local function wrap(s, w, size, font)
  local lines = {}
  for para in (tostring(s) .. "\n"):gmatch("(.-)\n") do
    local line = ""
    for word in para:gmatch("%S+") do
      local try = line == "" and word or line .. " " .. word
      if line ~= "" and gfx.measure(try, size, font) > w then
        lines[#lines + 1] = line
        line = word
      else
        line = try
      end
    end
    lines[#lines + 1] = line
  end
  return lines
end

kit.fit, kit.wrap = fit, wrap

-- How many decimals a slider shows: as many as its step has (2 without one).
local function decimals(step)
  if not step or step <= 0 then
    return 2
  end
  local frac = string.format("%g", step):match("%.(%d+)$")
  return frac and math.min(#frac, 4) or 0
end

-- Byte offsets of the characters around offset i (text fields).
local function before(s, i)
  i = i - 1
  while i > 0 and continuation(s, i + 1) do
    i = i - 1
  end
  return math.max(i, 0)
end

local function after(s, i)
  i = i + 1
  while i < #s and continuation(s, i + 1) do
    i = i + 1
  end
  return math.min(i, #s)
end

-- ------------------------------------------------------------ HUD cards

-- A card of text lines for on("hud"): kit.card(x, y, lines[, opts]).
-- Negative x or y count from the right or bottom edge. opts: title,
-- width, size, font, alpha. Returns the card's width and height.
function kit.card(x, y, lines, opts)
  opts = opts or {}
  local size = opts.size or 13
  local lh = math.floor(size * LINE + 0.5) + 2
  local pad = 12
  local widest = 0
  for _, l in ipairs(lines) do
    widest = math.max(widest, gfx.measure(tostring(l), size, opts.font))
  end
  if opts.title then
    widest = math.max(widest, gfx.measure(opts.title:upper(), 10, "medium"))
  end
  local w = math.max(opts.width or 0, math.ceil(widest) + pad * 2)
  local h = pad * 2 + #lines * lh - 2 + (opts.title and 20 or 0)
  local sw, sh = gfx.size()
  if x < 0 then
    x = sw + x - w
  end
  if y < 0 then
    y = sh + y - h
  end
  local a = opts.alpha or 1
  gfx.rect(x, y, w, h, paint(theme.page, 0.84 * a), 12)
  gfx.border(x, y, w, h, paint(theme.rule, a), 12)
  local ty = y + pad
  if opts.title then
    gfx.text(x + pad, ty - 1, opts.title:upper(), paint(theme.accent, a), 10, "medium")
    ty = ty + 20
  end
  for _, l in ipairs(lines) do
    gfx.text(x + pad, ty, tostring(l), paint(theme.ink, a), size, opts.font)
    ty = ty + lh
  end
  return w, h
end

-- ------------------------------------------------------------ windows

local Window = {}
Window.__index = Window

-- Where the next widget goes: x, y, width, height.
local function place(w, h)
  local r = w.row
  if r then
    local cw = (w.cw - (r.n - 1) * GAP) / r.n
    local x = w.cx + r.i * (cw + GAP)
    r.i = r.i + 1
    r.h = math.max(r.h, h)
    if r.i >= r.n then
      w.cy = r.y + r.h + GAP
      w.row = nil
    end
    return x, r.y, cw, h
  end
  local y = w.cy
  w.cy = w.cy + h + GAP
  return w.cx, y, w.cw, h
end

-- The width the next widget gets.
local function cell(w)
  local r = w.row
  if r then
    return (w.cw - (r.n - 1) * GAP) / r.n
  end
  return w.cw
end

local function hovering(w, x, y, bw, bh)
  return w.live and w.drag == nil and w.mx >= x and w.mx < x + bw and w.my >= y and w.my < y + bh
end

-- A widget's id: its label and its place in the window.
local function next_id(w, label)
  w.n = w.n + 1
  return tostring(label) .. "#" .. w.n
end

-- A value animated toward target, kept per widget.
local function anim(w, key, target, speed)
  local v = w.anim[key]
  if v == nil then
    v = target
  else
    v = approach(v, target, w.dt, speed)
  end
  w.anim[key] = v
  return v
end

-- Pressed on this widget and released on it: a click.
local function clicked(w, id, hot)
  if hot and w.pressed then
    w.active = id
  end
  if w.released and w.active == id then
    if hot and w.spec.sounds ~= false then
      sound.play("minecraft:ui.button.click", 0.25, 1.4)
    end
    return hot
  end
  return false
end

local function label_text(w, x, y, s, size, c, font, width)
  gfx.text(x, y, fit(s, width or w.cw, size, font), paint(c, w.alpha), size, font)
end

-- The functions a window's build function gets, as ui.name(...).
local function widgets(w)
  local ui = {}

  function ui.heading(s)
    local x, y, cw = place(w, 24)
    label_text(w, x, y, s, 16, theme.ink, "medium", cw)
  end

  function ui.text(s, opts)
    opts = opts or {}
    local size = opts.size or 14
    local lh = math.floor(size * LINE + 0.5) + 1
    local lines = wrap(s, cell(w), size, opts.font)
    local x, y = place(w, #lines * lh - 1)
    local c = opts.color and (type(opts.color) == "string" and rgb(opts.color) or opts.color) or theme.soft
    for i, l in ipairs(lines) do
      gfx.text(x, y + (i - 1) * lh, l, paint(c, w.alpha), size, opts.font)
    end
  end

  function ui.muted(s)
    ui.text(s, { size = 12, color = theme.muted })
  end

  function ui.space(h)
    w.cy = w.cy + (h or 8)
  end

  function ui.separator()
    local x, y, cw = place(w, 1)
    gfx.rect(x, y, cw, 1, paint(theme.rule, w.alpha))
  end

  -- The next n widgets share one row.
  function ui.row(n)
    w.row = { n = math.max(1, math.floor(n or 2)), i = 0, y = w.cy, h = 0 }
  end

  function ui.button(label, opts)
    opts = opts or {}
    local x, y, bw, bh = place(w, opts.height or 36)
    local id = next_id(w, label)
    local hot = hovering(w, x, y, bw, bh)
    local hit = clicked(w, id, hot)
    local t = anim(w, id .. ":h", hot and 1 or 0)
    local down = anim(w, id .. ":d", (hot and w.down and w.active == id) and 1 or 0, 30)
    local bg, fg
    if opts.primary then
      bg, fg = mix(theme.accent, theme.accent_hover, t), theme.page
    elseif opts.danger then
      bg, fg = mix(mix(theme.plate, theme.danger, 0.25), mix(theme.plate, theme.danger, 0.4), t), theme.danger
    else
      bg, fg = mix(theme.plate, theme.raised, t), theme.ink
    end
    local inset = down * 1.5
    gfx.rect(x + inset, y + inset, bw - inset * 2, bh - inset * 2, paint(bg, w.alpha), bh / 2)
    local l = fit(label, bw - 24, 13, "medium")
    local lw = gfx.measure(l, 13, "medium")
    gfx.text(math.floor(x + (bw - lw) / 2), top(y + bh / 2, 13), l, paint(fg, w.alpha), 13, "medium")
    return hit
  end

  function ui.toggle(label, value, opts)
    local x, y, cw, h = place(w, 36)
    local id = next_id(w, label)
    local hot = hovering(w, x - 10, y, cw + 20, h)
    if clicked(w, id, hot) then
      value = not value
    end
    local hv = anim(w, id .. ":h", hot and 1 or 0)
    local t = anim(w, id .. ":v", value and 1 or 0, 16)
    if hv > 0.01 then
      gfx.rect(x - 10, y, cw + 20, h, paint(theme.ink, 0.05 * hv * w.alpha), 10)
    end
    label_text(w, x, top(y + h / 2, 14), label, 14, theme.ink, nil, cw - 60)
    local tx, ty = x + cw - 40, y + h / 2 - 11
    gfx.rect(tx, ty, 40, 22, paint(mix(theme.raised, theme.accent, t), w.alpha), 11)
    gfx.rect(tx + 3 + 18 * t, ty + 3, 16, 16, paint(mix(theme.soft, theme.ink, t), w.alpha), 8)
    return value
  end

  function ui.slider(label, value, min, max, opts)
    opts = opts or {}
    min, max = min or 0, max or 1
    local step = opts.step
    local x, y, cw, h = place(w, 46)
    local id = next_id(w, label)
    local hot = hovering(w, x - 6, y, cw + 12, h)
    if hot and w.pressed then
      w.active = id
    end
    local function snap(v)
      if step and step > 0 then
        v = min + math.floor((v - min) / step + 0.5) * step
      end
      return math.clamp(v, math.min(min, max), math.max(min, max))
    end
    if w.active == id and w.down and max ~= min then
      value = snap(min + (max - min) * math.clamp((w.mx - x) / cw, 0, 1))
    elseif hot and w.wheel ~= 0 then
      value = snap(value + (step or (max - min) / 20) * (w.wheel > 0 and 1 or -1))
    end
    value = snap(value or min)
    local shown = string.format(opts.format or ("%." .. decimals(step) .. "f"), value)
    local vw = gfx.measure(shown, 12, "mono")
    label_text(w, x, y, label, 13, theme.ink, nil, cw - vw - 12)
    gfx.text(x + cw - vw, y + 1, shown, paint(theme.muted, w.alpha), 12, "mono")
    local frac = max ~= min and (value - min) / (max - min) or 0
    local f = anim(w, id .. ":f", frac, 24)
    local ty = y + 34
    gfx.rect(x, ty - 2, cw, 4, paint(theme.raised, w.alpha), 2)
    gfx.rect(x, ty - 2, cw * f, 4, paint(theme.accent, w.alpha), 2)
    local hv = anim(w, id .. ":h", (hot or w.active == id) and 1 or 0)
    local kr = 7 + 2 * hv
    local kx = x + cw * f
    if hv > 0.01 then
      gfx.rect(kx - kr - 4, ty - kr - 4, kr * 2 + 8, kr * 2 + 8, paint(theme.accent, 0.22 * hv * w.alpha), kr + 4)
    end
    gfx.rect(kx - kr, ty - kr, kr * 2, kr * 2, paint(theme.ink, w.alpha), kr)
    return value
  end

  -- A text field: returns its text, and true when Enter was pressed.
  function ui.input(label, value, opts)
    opts = opts or {}
    value = tostring(value or "")
    local x, y, cw = place(w, 58)
    local id = next_id(w, label)
    local fy = y + 22
    local hot = hovering(w, x, fy, cw, 36)
    local submitted = false
    if clicked(w, id, hot) and w.focus ~= id then
      w.focus, w.caret = id, #value
    elseif w.pressed and not hot and w.focus == id then
      w.focus = nil
    end
    if w.focus == id then
      local caret = math.clamp(w.caret or #value, 0, #value)
      for _, t in ipairs(w.typed) do
        if #value + #t <= (opts.max or 256) then
          value = value:sub(1, caret) .. t .. value:sub(caret + 1)
          caret = caret + #t
        end
      end
      for _, k in ipairs(w.keys) do
        if k == "backspace" and caret > 0 then
          local b = before(value, caret)
          value, caret = value:sub(1, b) .. value:sub(caret + 1), b
        elseif k == "delete" and caret < #value then
          value = value:sub(1, caret) .. value:sub(after(value, caret) + 1)
        elseif k == "left" then
          caret = before(value, caret)
        elseif k == "right" then
          caret = after(value, caret)
        elseif k == "home" or k == "up" then
          caret = 0
        elseif k == "end" or k == "down" then
          caret = #value
        elseif k == "enter" then
          submitted, w.focus = true, nil
        end
      end
      w.caret = caret
      w.typed, w.keys = {}, {}
    end
    label_text(w, x, y, label, 12, theme.muted, nil, cw)
    local ring = anim(w, id .. ":f", w.focus == id and 1 or 0, 18)
    gfx.rect(x, fy, cw, 36, paint(theme.page, w.alpha), 10)
    gfx.border(x, fy, cw, 36, paint(mix(theme.rule, theme.accent, ring), w.alpha), 10)
    local ty = top(fy + 18, 14)
    if value == "" and w.focus ~= id then
      label_text(w, x + 12, ty, opts.placeholder or "", 14, theme.muted, nil, cw - 24)
    elseif w.focus == id then
      -- Scrolled so the caret shows.
      local cx = gfx.measure(value:sub(1, w.caret), 14)
      local shift = math.max(0, cx - (cw - 26))
      local shown = value
      while shift > 0 and #shown > 0 do
        local n = after(shown, 0)
        shift = shift - gfx.measure(shown:sub(1, n), 14)
        cx = cx - gfx.measure(shown:sub(1, n), 14)
        shown = shown:sub(n + 1)
      end
      gfx.text(x + 12, ty, fit(shown, cw - 24, 14), paint(theme.ink, w.alpha), 14)
      if (w.now * 2) % 2 < 1.3 then
        gfx.rect(x + 12 + cx, fy + 9, 1.5, 18, paint(theme.accent, w.alpha))
      end
    else
      label_text(w, x + 12, ty, value, 14, theme.ink, nil, cw - 24)
    end
    return value, submitted
  end

  -- A segmented control with a sliding highlight; returns the chosen index.
  function ui.tabs(items, current)
    current = math.clamp(math.floor(current or 1), 1, math.max(1, #items))
    local x, y, cw, h = place(w, 36)
    local id = next_id(w, "tabs")
    local seg = cw / math.max(1, #items)
    gfx.rect(x, y, cw, h, paint(theme.page, w.alpha), h / 2)
    for i = 1, #items do
      if clicked(w, id .. ":" .. i, hovering(w, x + (i - 1) * seg, y, seg, h)) then
        current = i
      end
    end
    local px = anim(w, id .. ":x", x + (current - 1) * seg, 18)
    gfx.rect(px + 3, y + 3, seg - 6, h - 6, paint(theme.raised, w.alpha), (h - 6) / 2)
    for i, name in ipairs(items) do
      local sx = x + (i - 1) * seg
      local on = anim(w, id .. ":c" .. i, i == current and 1 or 0)
      local hv = anim(w, id .. ":h" .. i, hovering(w, sx, y, seg, h) and 1 or 0)
      local l = fit(name, seg - 16, 13, "medium")
      local lw = gfx.measure(l, 13, "medium")
      local c = mix(theme.muted, theme.ink, math.max(on, hv * 0.6))
      gfx.text(math.floor(sx + (seg - lw) / 2), top(y + h / 2, 13), l, paint(c, w.alpha), 13, "medium")
    end
    return current
  end

  -- A scrolling list (mouse wheel); returns the selected index.
  function ui.list(items, selected, opts)
    opts = opts or {}
    local rows = math.max(1, opts.rows or 5)
    local rh = 34
    local x, y, cw, h = place(w, rows * rh + 8)
    local id = next_id(w, "list")
    local hot = hovering(w, x, y, cw, h)
    local most = math.max(0, #items - rows)
    local scroll = math.clamp(w.anim[id .. ":s"] or 0, 0, most)
    if hot and w.wheel ~= 0 then
      scroll = math.clamp(scroll - (w.wheel > 0 and 1 or -1), 0, most)
    end
    w.anim[id .. ":s"] = scroll
    gfx.rect(x, y, cw, h, paint(theme.page, w.alpha), 12)
    gfx.border(x, y, cw, h, paint(theme.rule, w.alpha), 12)
    local sel_y = anim(w, id .. ":y", y + 4 + ((selected or 1) - 1 - scroll) * rh, 22)
    if selected and selected > scroll and selected <= scroll + rows then
      gfx.rect(x + 4, sel_y, cw - 8, rh, paint(mix(theme.page, theme.accent, 0.18), w.alpha), 8)
    end
    for i = scroll + 1, math.min(#items, scroll + rows) do
      local ry = y + 4 + (i - 1 - scroll) * rh
      local rhot = hovering(w, x + 4, ry, cw - 8, rh)
      if clicked(w, id .. ":" .. i, rhot) then
        selected = i
      end
      local hv = anim(w, id .. ":h" .. i, rhot and 1 or 0)
      if hv > 0.01 and i ~= selected then
        gfx.rect(x + 4, ry, cw - 8, rh, paint(theme.ink, 0.05 * hv * w.alpha), 8)
      end
      local c = i == selected and theme.accent_hover or theme.ink
      label_text(w, x + 14, top(ry + rh / 2, 14), items[i], 14, c, nil, cw - 36)
    end
    if most > 0 then
      local track = h - 16
      local th = math.max(24, track * rows / #items)
      local ty = y + 8 + (track - th) * scroll / most
      gfx.rect(x + cw - 8, ty, 3, th, paint(theme.muted, 0.5 * w.alpha), 1.5)
    end
    return selected
  end

  -- A bar for 0..1.
  function ui.progress(value, opts)
    opts = opts or {}
    local x, y, cw = place(w, 6)
    local id = next_id(w, "progress")
    local f = anim(w, id, math.clamp(value or 0, 0, 1), 10)
    local c = opts.color and rgb(opts.color) or theme.accent
    gfx.rect(x, y, cw, 6, paint(theme.raised, w.alpha), 3)
    if f > 0.005 then
      gfx.rect(x, y, math.max(6, cw * f), 6, paint(c, w.alpha), 3)
    end
  end

  function ui.close()
    w:close()
  end

  -- The content width, the seconds since the last frame, and the window.
  function ui.width()
    return cell(w)
  end
  function ui.dt()
    return w.dt
  end
  ui.window = w
  return ui
end

-- kit.window(spec, build): a window that build(ui) fills each frame.
-- spec: title, subtitle, width, accent ("#rrggbb"), sounds (false: quiet),
-- close() when it closes. Returns it: win:open(), win:close(),
-- win:toggle(), win:is_open().
function kit.window(spec, build)
  local w = setmetatable({ spec = spec or {}, build = build, anim = {}, shown = false }, Window)
  w.ui = widgets(w)
  return w
end

function Window:open()
  local w = self
  w.opened, w.closing, w.last = game.time(), nil, nil
  w.focus, w.active, w.drag, w.row = nil, nil, nil, nil
  w.mx, w.my, w.down, w.pressed, w.released, w.wheel = -1, -1, false, false, false, 0
  w.typed, w.keys = {}, {}
  ui.open({
    canvas = true,
    draw = function(sw, sh)
      w:frame(sw, sh)
    end,
    move = function(x, y)
      w.mx, w.my = x, y
    end,
    click = function(x, y, button)
      w.mx, w.my = x, y
      if button == "left" then
        w.down, w.pressed = true, true
      end
    end,
    release = function(x, y, button)
      if button == "left" then
        w.down, w.released = false, true
      end
    end,
    wheel = function(steps)
      w.wheel = w.wheel + steps
    end,
    text = function(t)
      w.typed[#w.typed + 1] = t
    end,
    key = function(k, down)
      if down then
        w.keys[#w.keys + 1] = k
      end
    end,
    close = function()
      w.shown = false
      if w.spec.close then
        w.spec.close()
      end
    end,
  })
  w.shown = true
end

function Window:close()
  if self.shown and not self.closing then
    self.closing = game.time()
  end
end

function Window:toggle()
  if self.shown and not self.closing then
    self:close()
  else
    self:open()
  end
end

function Window:is_open()
  return self.shown
end

function Window:frame(sw, sh)
  local w, spec = self, self.spec
  local now = game.time()
  w.dt = w.last and math.clamp(now - w.last, 0, 0.1) or 0
  w.last, w.now = now, now
  -- In over 240 ms, out over 150 ms.
  local a
  if w.closing then
    a = 1 - math.clamp((now - w.closing) / 0.15, 0, 1)
    if a <= 0 then
      w.shown = false
      ui.close()
      if spec.close then
        spec.close()
      end
      return
    end
  else
    a = ease((now - w.opened) / 0.24)
  end
  w.alpha = a
  w.live = not w.closing and a > 0.5
  local saved = theme.accent
  if spec.accent then
    theme.accent = rgb(spec.accent)
  end

  local width = math.min(spec.width or 380, sw - 32)
  local target = w.content_h or spec.height or 200
  w.height = w.height and approach(w.height, target, w.dt, 22) or target
  local height = math.min(w.height, sh - 32)
  -- Dragged by its header.
  local head = spec.title and (spec.subtitle and 74 or 58) or 18
  if w.drag and w.down then
    w.x, w.y = w.mx - w.drag[1], w.my - w.drag[2]
  elseif w.drag then
    w.drag = nil
  end
  local px = math.floor(math.clamp(w.x or (sw - width) / 2, 8, math.max(8, sw - width - 8)))
  local py = math.floor(math.clamp(w.y or (sh - height) / 2, 8, math.max(8, sh - height - 8)))
  if w.x then
    w.x, w.y = px, py
  end
  py = py + math.floor((1 - a) * 8 + 0.5)

  -- A soft shadow: a few wider, fainter layers.
  for i = 1, 4 do
    gfx.rect(px - i * 2, py + i * 3, width + i * 4, height + i * 2, paint(theme.black, 0.07 * a), 18 + i * 2)
  end
  gfx.rect(px, py, width, height, paint(theme.surface, a), 18)
  gfx.border(px, py, width, height, paint(theme.rule, a), 18)

  -- The close button, then the header (whose press starts a drag).
  local cx, cy = px + width - 22 - 14, py + 16
  local chot = w.live and w.mx >= cx and w.mx < cx + 28 and w.my >= cy and w.my < cy + 28
  local ch = anim(w, ":close", chot and 1 or 0)
  if ch > 0.01 then
    gfx.rect(cx, cy, 28, 28, paint(theme.ink, 0.08 * ch * a), 14)
  end
  local xw = gfx.measure("×", 18, "regular")
  gfx.text(cx + (28 - xw) / 2, top(cy + 14, 18), "×", paint(mix(theme.muted, theme.ink, ch), a), 18)
  if chot and w.pressed then
    w.active = ":close"
  end
  if w.released and w.active == ":close" and chot then
    w:close()
  end
  if spec.title then
    local tw = width - PAD * 2 - 36
    gfx.text(px + PAD, py + 18, fit(spec.title, tw, 18, "medium"), paint(theme.ink, a), 18, "medium")
    if spec.subtitle then
      gfx.text(px + PAD, py + 44, fit(spec.subtitle, tw, 12), paint(theme.muted, a), 12)
    end
  end
  local over_head = w.mx >= px and w.mx < px + width and w.my >= py and w.my < py + head
  if w.live and w.pressed and over_head and not chot then
    w.drag = { w.mx - px, w.my - py }
  end

  -- The content.
  w.cx, w.cy, w.cw, w.n, w.row = px + PAD, py + head, width - PAD * 2, 0, nil
  local ok, err = pcall(w.build, w.ui)
  w.row = nil
  theme.accent = saved
  w.content_h = w.cy - GAP + PAD - py
  if w.released then
    w.active = nil
  end
  w.pressed, w.released, w.wheel, w.typed, w.keys = false, false, 0, {}, {}
  if not ok then
    error(err, 0)
  end
end

return kit
