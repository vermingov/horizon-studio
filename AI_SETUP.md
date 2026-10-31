# Setting up Horizon Studio for an AI agent

These are instructions for an AI coding agent (Claude Code, omp, Codex, Cursor, Gemini CLI, OpenCode...) whose user
wants it to make Horizon mods. Follow them in order, run the commands yourself, and tell the user plainly what you did
and what they need to do. Ask before anything that takes long or changes their game.

Horizon is Minecraft Java 26.3 rewritten in Rust. Horizon Studio is a native mod for it that runs mods written in Lua,
hot reloaded in the running game. Its MCP server, `horizon-studio-mcp`, lets you write those mods, test them headless
(no game needed) and live in the game, run Lua there, and see what happens.

## 1. Install the MCP server

On Linux or macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/vermingov/horizon-studio/main/mcp/install.sh | sh
```

On Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/vermingov/horizon-studio/main/mcp/install.ps1 | iex
```

It downloads the program for this machine (checksum verified), puts it in `~/.local/bin` (Windows:
`%LOCALAPPDATA%\horizon-studio\bin`) and registers it, as `horizon-studio`, with every agent it finds: Claude Code (user
scope, through `claude mcp add`), omp (`~/.omp/agent/mcp.json`), Codex, Cursor, Gemini CLI, Claude Desktop and OpenCode.
Read its output: it lists where it registered.

If your agent isn't in that list, register it by hand as a stdio MCP server whose command is the program's full path
with no arguments (`horizon-studio-mcp install --print` prints the JSON most agents take). For Claude Code alone:

```sh
claude mcp add --scope user horizon-studio -- ~/.local/bin/horizon-studio-mcp
```

## 2. Make sure Horizon Studio is in the game

The MCP server works without the game (headless checks and tests), but live testing needs Horizon Studio installed in
Horizon. Ask the user whether it is: in Horizon, Options → Mods lists **Horizon Studio**, and F8 opens it in a world.

If it isn't, it is installed into Horizon's launcher with one line, on Linux or macOS. On Linux (x86_64) and macOS
(Apple silicon) it takes seconds, from the game the release carries already built; elsewhere the launcher compiles
it into the game once (10 to 30 minutes; the launcher brings its own Rust). Ask the user before starting it, and ask
them to close the launcher first:

```sh
curl -fsSL https://raw.githubusercontent.com/vermingov/horizon-studio/main/install.sh | sh
```

Then the user presses **Play** in the launcher (older launchers: on the **Native mods** instance).

## 3. Load the MCP server

Agents load MCP servers when a session starts. Tell the user to restart you (Claude Code: run `/mcp` or start a new
session; omp: `/mcp reload`), and to tell you when they have. Stop here until they do.

## 4. Check the link to the game

Once the `horizon-studio` tools are available:

1. Call `game_status`. `linked: true` means the game is running with Horizon Studio: you can test live. If it isn't
   linked, ask the user to start Horizon (with the instance the installer selected) and open a world; Horizon Studio's AI link is on by
   default (Options → Mods → Horizon Studio → AI link (MCP)). Everything but live testing works without it.
2. Call `docs` with no arguments: it lists the reference's topics. Read `start`, then the topics your mod needs
   (`events`, `screens`, `world`, `models`, `recipes`...); `docs` with `name` gives one function, event or widget in
   full, and `search` finds anything by words. Everything about the API, the Lua it runs (Lua 5.3 without
   goto), libraries, limits, errors and the test step format is there: nothing needs looking up elsewhere.
3. Make a small mod to prove it works, and tell the user what you did. For example, a `/hello` command:

   - `write_mod` with `{"id": "hello", "files": {"hello.lua": "command('hello', function(rest) return 'Hello, ' .. (rest ~= '' and rest or 'world') .. '!' end, 'says hello')"}}`
   - `test_mod` with `{"id": "hello", "steps": [{"command": "/hello Alex", "expect_chat": "Hello, Alex!"}]}` (headless)
   - and, when linked, the same with `"live": true`. In the game, the user can now type `/hello`.

## 5. How to work from here

- Write mods with `write_mod`: a script is one `<id>.lua`; a module is a folder whose `main.lua` `require`s its other
  files. For a module that exists, give only the files that change (the others stay), and `remove` deletes files. It
  checks the mod headless first, as it will be on disk, and reports errors with their file and line, then writes it
  where the game reads it; the game reloads it at once.
- Shared code goes in a library: `export{...}` in one mod, `import("id")` in others. The bundled UI Kit (`uikit`) is
  one; `examples` lists the bundled mods and gives their source.
- Test with `test_mod`: headless first (commands, chat, keys, Lua assertions, the player and blocks, script screens and
  clicks, sounds, particles, text drawn), then live. Fix what fails and test again. Its `files` test text not yet
  written: for a module, only the files that change, over its other files on disk.
- In the running game: `run_lua` runs Lua in the studio's console, `send_chat` sends chat or commands and returns what
  followed, `game_log` reads the console and chat, `reload_mod` reloads.
- With Horizon started with `--control` (or `horizon-ctl launch --world NAME --wait`), `horizon_ctl` plays the game
  and `horizon_ctl` with `screenshot` shows you the screen.
- Mods are sandboxed: no files, no network, a step budget per call. A mod that fails stops alone.
