-- name: Miner
-- version: 1.0
-- author: Horizon Studio
-- description: /mine <block> [count] walks to the nearest blocks of a kind and mines them, using Horizon's agent. /mine stop cancels. Try /mine oak_log 3.

local job = nil

local function finish(text)
  job = nil
  chat.show(text)
end

local function next_block()
  if not job then return end
  if job.done >= job.count then return finish("Mined " .. job.done .. " " .. job.block) end
  local found = world.find(job.block, 32)
  local near = found and found.nearest and found.nearest[1]
  if not near then return finish("No more " .. job.block .. " within 32 blocks") end
  local x, y, z = near.pos[1], near.pos[2], near.pos[3]
  agent.run(string.format("goto %d %d %d --near 3 --timeout 30", x, y, z), function(_, err)
    if not job then return end
    if err then return finish("Could not get there: " .. err) end
    agent.run(string.format("break %d %d %d --timeout 15", x, y, z), function(result, err2)
      if not job then return end
      if err2 then return finish("Could not mine it: " .. err2) end
      job.done = job.done + 1
      next_block()
    end)
  end)
end

command("mine", function(args, words)
  if words[1] == "stop" then
    agent.stop()
    job = nil
    return "Stopped"
  end
  local block = words[1]
  if not block then return "Usage: /mine <block> [count]" end
  if not block:find(":") then block = "minecraft:" .. block end
  job = { block = block, count = tonumber(words[2]) or 1, done = 0 }
  after(0, next_block)
  return "Mining " .. job.count .. " " .. block
end, "Mines blocks for you: /mine <block> [count], /mine stop")

on("unload", function() if job then agent.stop() end end)
