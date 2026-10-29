-- name: Hello
-- version: 1.0
-- author: Horizon Studio
-- description: Your first script mod. Type /hello in chat, then open it in the Editor (F8, Library), change the text and press Ctrl+S: it reloads at once.

print("hello loaded")

command("hello", function(args)
  local name = args ~= "" and args or "world"
  return "Hello, " .. name .. "!"
end, "Says hello: /hello [name]")
