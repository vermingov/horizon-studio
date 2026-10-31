// The player's own AI coding agent, chatted with from inside the game: the
// editor runs the agent's CLI (Claude Code's `claude`, or `omp`) as a child
// process with a prompt, reads what it streams and shows it in a panel. The
// agent does its work through the horizon-studio MCP server, which
// `horizon-studio-mcp install` registered with it. This is the backend only
// (no UI), compiled into the game too: the standard library and serde_json,
// threads instead of async, on Linux, macOS and Windows.
//
// Claude Code, as its documentation has it (checked 2026-10-30):
// code.claude.com/docs/en/headless (`claude -p`, stream-json, --resume),
// code.claude.com/docs/en/agent-sdk/typescript (the message types),
// code.claude.com/docs/en/permissions (MCP rules), and `claude --help` 2.1.314.
// - `claude -p <prompt> --output-format stream-json --verbose` prints one
//   JSON object per line (stream-json needs --verbose with -p). Without
//   --include-partial-messages there are no token deltas: whole messages.
// - `{"type":"system","subtype":"init","session_id","cwd","model","tools",
//   "mcp_servers":[{"name","status"}],"permissionMode",...}` comes first
//   (after hook events, if hooks run). `{"type":"system","subtype":
//   "api_retry","attempt","max_retries","error","error_status",...}` before a
//   retried API request; other system subtypes (permission_denied, hooks...)
//   are skipped here.
// - `{"type":"assistant","message":{<an API message>: "content":[{"type":
//   "text","text"},{"type":"tool_use","id","name","input"},{"type":
//   "thinking",...}]},"parent_tool_use_id","session_id","uuid"}`.
// - `{"type":"user","message":{"role":"user","content":[{"type":
//   "tool_result","tool_use_id","content": a string or [{"type":"text",
//   "text"},...],"is_error"?}]},...}`: the tools' results.
// - The last line: `{"type":"result","subtype":"success" | "error_max_turns"
//   | "error_during_execution" | "error_max_budget_usd" | ...,"is_error",
//   "result" (success only: the final answer, or the failure's text such as
//   "Invalid API key"),"errors" (error subtypes),"session_id","num_turns",
//   "duration_ms","total_cost_usd","usage","permission_denials",...}`.
// - The exit status is 0 on success; a bad flag is reported on stderr before
//   the run starts. SIGTERM ends a run with status 143, its turn unfinished.
// - `--resume <session id>` continues a conversation (from any directory).
// - `--allowedTools mcp__horizon-studio` approves every tool of that MCP
//   server ("mcp__puppeteer matches any tool provided by the puppeteer
//   server"); its tools then appear as `mcp__horizon-studio__<tool>`. The flag
//   takes several values (`<tools...>`), so it goes last, after the prompt.
//   New tools of the server (the player's editor: open_script, read_script,
//   edit_script, save_script, mark_lines, clear_marks) are approved with the
//   rest, nothing to add.
// - `--permission-mode default` (Manual mode) is passed too: in it a `-p` run
//   denies every call that needs approval (Bash, Edit, Write...) since nobody
//   can answer, whatever the user's settings make their default mode; reads
//   inside the working directory need no approval. So the agent can use
//   Horizon Studio's tools and read the mods folder, but not run commands or
//   edit files itself.
// - `--bare` is not used: it skips the user's MCP servers, horizon-studio too.
//
// omp (oh-my-pi, can1357/oh-my-pi, npm @oh-my-pi/pi-coding-agent 18.14.0: its
// README, src/modes/print-mode.ts, src/cli/flag-tables.ts,
// src/session/session-entries.ts, src/mcp/tool-bridge.ts, src/tools/
// settings.ts and src/extensibility/extensions/wrapper.ts; and the upstream pi
// documentation, pi.dev/docs/latest/cli-integration, json.md, message-types.md):
// - `omp -p <prompt>` answers one prompt and exits, printing the final text;
//   `--mode json` prints every event instead, one JSON object per line on
//   stdout (diagnostics go to stderr): first the session header
//   `{"type":"session","version","id","timestamp","cwd"}`, then events:
//   agent_start, turn_start, message_start, `message_update` (deltas only:
//   `"assistantMessageEvent":{"type":"text_delta","delta",...}`),
//   `{"type":"message_end","message":{"role":"assistant","content":[{"type":
//   "text","text"},{"type":"thinking",...},{"type":"toolCall","id","name",
//   "arguments"}],"stopReason":"stop"|"length"|"toolUse"|"error"|"aborted",
//   "errorMessage"?}}` (the authoritative message), `{"type":
//   "tool_execution_start","toolCallId","toolName","args"}`, `{"type":
//   "tool_execution_end","toolCallId","toolName","result":{"content":[...],
//   "details"},"isError"}`, turn_end, agent_end, agent_settled; and
//   `{"type":"auto_retry_start","attempt","maxAttempts","errorMessage"}`.
// - There is no result line: the exit status says how it went (0, 1 for a
//   turn that ended in an error or abort, 3 for the cost cap).
// - `--resume <session id or file>` continues a session.
// - MCP tools are named `mcp__<server>_<tool>` with the server's name
//   sanitized to [a-z0-9_]: `mcp__horizon_studio_write_mod`.
// - Tool approval: omp's default `tools.approvalMode` is "yolo" (every tool
//   runs). `--approval-mode write` auto-approves read and write tools (MCP
//   tools are write tier) and asks for exec tools (bash, eval, browser,
//   task), which with no UI fails ("requires approval but no interactive UI
//   available"): the agent can't run commands, as with Claude Code.
//
// A custom agent: `agent.txt` in the studio's folder, its first line
// optionally `name: My agent`, then a command line such as
// `aider --message {prompt}` (split like a shell's, with quotes); its stdout
// is shown as text. Without `{prompt}` the prompt goes to its stdin.
//
// Windows: an npm-installed agent is a `claude.cmd` shim, which only cmd.exe
// runs. std's Command does that itself for a program path ending in .cmd or
// .bat (`cmd.exe /e:ON /v:OFF /d /c`, quoting each argument against cmd's
// own parsing since Rust 1.77.2: running `cmd /C` by hand would let a quote
// in the prompt end the quoting and `&` start another command). cmd.exe can't
// take a newline in an argument, nor more than 8191 characters, so for a
// batch file the prompt goes to Claude Code's stdin (`claude -p` reads its
// prompt there) and other agents get it on one line.

use serde_json::Value as Json;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// The MCP server's name, as the agents have it registered.
pub const SERVER: &str = "horizon-studio";

/// An agent the game can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSpec {
    /// "claude", "omp" or "custom".
    pub id: String,
    /// Its name for the player: "Claude Code".
    pub name: String,
    pub program: PathBuf,
    /// The arguments: `{prompt}` is replaced by the prompt (without it, the
    /// prompt goes to stdin), `{session}` by the session resumed; without
    /// one, an argument with `{session}` is left out, and the flag before a
    /// bare `{session}` with it.
    pub args: Vec<String>,
    /// Whether it prints a JSON event stream (omp's `--mode json` when the id
    /// is "omp", else Claude Code's stream-json), else plain text.
    pub json: bool,
}

impl AgentSpec {
    /// The same agent asked to use `model` (`--model`, which Claude Code and
    /// omp both take: an alias such as `sonnet` or a full name); others, and
    /// an empty model, as they are.
    pub fn with_model(&self, model: &str) -> AgentSpec {
        let model = model.trim();
        if model.is_empty() || !matches!(self.id.as_str(), "claude" | "omp") {
            return self.clone();
        }
        let mut spec = self.clone();
        // Right after -p: Claude Code's --allowedTools must stay last.
        let at = spec.args.iter().position(|a| a == "-p").map_or(0, |i| i + 1);
        spec.args.insert(at, "--model".into());
        spec.args.insert(at + 1, model.into());
        spec
    }

    /// Claude Code at `program`.
    pub fn claude(program: PathBuf) -> AgentSpec {
        let mut args = vec!["-p".to_owned()];
        // A .cmd shim (npm on Windows) runs through cmd.exe: the prompt goes to stdin.
        if !(cfg!(windows) && is_batch(&program)) {
            args.push("{prompt}".into());
        }
        args.extend(
            [
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-mode",
                "default",
                "--resume",
                "{session}",
                "--allowedTools",
            ]
            .map(str::to_owned),
        );
        args.push(format!("mcp__{SERVER}"));
        AgentSpec {
            id: "claude".into(),
            name: "Claude Code".into(),
            program,
            args,
            json: true,
        }
    }

    /// omp (oh-my-pi) at `program`.
    pub fn omp(program: PathBuf) -> AgentSpec {
        let args = [
            "-p",
            "--mode",
            "json",
            "--approval-mode",
            "write",
            "--resume",
            "{session}",
            "{prompt}",
        ];
        AgentSpec {
            id: "omp".into(),
            name: "omp".into(),
            program,
            args: args.map(str::to_owned).to_vec(),
            json: true,
        }
    }
}

// ------------------------------------------------------------------ finding them

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Where agents are installed besides the PATH: games started from a desktop
/// launcher often have a minimal one.
pub fn usual_places(home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(h) = home {
        // The native installers (claude, also on Windows), Claude Code's old
        // local install, npm's user prefix, bun (omp's install), Volta.
        for dir in [
            &[".local", "bin"][..],
            &[".claude", "local"],
            &[".npm-global", "bin"],
            &[".bun", "bin"],
            &[".volta", "bin"],
        ] {
            out.push(dir.iter().fold(h.to_path_buf(), |p, d| p.join(d)));
        }
        // nvm's Node versions, the newest first.
        if let Ok(entries) = std::fs::read_dir(h.join(".nvm").join("versions").join("node")) {
            let mut versions: Vec<(Vec<u64>, PathBuf)> = entries
                .flatten()
                .map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let number = name
                        .trim_start_matches('v')
                        .split('.')
                        .map_while(|n| n.parse().ok())
                        .collect();
                    (number, e.path().join("bin"))
                })
                .collect();
            versions.sort_by(|a, b| b.0.cmp(&a.0));
            out.extend(versions.into_iter().map(|(_, p)| p));
        }
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA").filter(|a| !a.is_empty()) {
            out.push(PathBuf::from(appdata).join("npm"));
        }
    } else {
        out.push(PathBuf::from("/usr/local/bin"));
        out.push(PathBuf::from("/opt/homebrew/bin"));
    }
    out
}

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// A program in the first of `dirs` that has it (on Windows `name`.exe, .cmd or .bat).
pub fn find_program(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "bat"].iter().map(|e| format!("{name}.{e}")).collect()
    } else {
        vec![name.to_owned()]
    };
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| executable(p))
}

/// The agents installed here: Claude Code and omp found on the PATH or in their
/// usual places, and a custom one from `studio_dir`/agent.txt if there is one.
pub fn detect(studio_dir: &Path) -> Vec<AgentSpec> {
    let path: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    detect_in(studio_dir, &path, home().as_deref())
}

/// [`detect`] with the PATH's folders and the home folder given.
pub fn detect_in(studio_dir: &Path, path: &[PathBuf], home: Option<&Path>) -> Vec<AgentSpec> {
    let mut dirs: Vec<PathBuf> = path.iter().filter(|d| !d.as_os_str().is_empty()).cloned().collect();
    dirs.extend(usual_places(home));
    let mut out = Vec::new();
    if let Some(p) = find_program("claude", &dirs) {
        out.push(AgentSpec::claude(p));
    }
    if let Some(p) = find_program("omp", &dirs) {
        out.push(AgentSpec::omp(p));
    }
    if let Some(custom) = std::fs::read_to_string(studio_dir.join("agent.txt"))
        .ok()
        .and_then(|text| parse_custom(&text, &dirs, home))
    {
        out.push(custom);
    }
    out
}

/// The custom agent of an agent.txt: an optional `name: ...` line, then its
/// command line (blank lines and `#` comments skipped). Its program is looked
/// for in `dirs` when it is a bare name.
pub fn parse_custom(text: &str, dirs: &[PathBuf], home: Option<&Path>) -> Option<AgentSpec> {
    let mut name = None;
    let mut command = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line
            .get(..5)
            .filter(|h| h.eq_ignore_ascii_case("name:"))
            .map(|_| &line[5..])
        {
            name = Some(rest.trim().to_owned()).filter(|n| !n.is_empty());
        } else if command.is_none() {
            command = Some(split_command(line));
        }
    }
    let mut words = command?.into_iter();
    let program = words.next()?;
    let program = match (program.strip_prefix("~/").or_else(|| program.strip_prefix("~\\")), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ if program.contains(['/', '\\']) => PathBuf::from(&program),
        _ => find_program(&program, dirs).unwrap_or_else(|| PathBuf::from(&program)),
    };
    Some(AgentSpec {
        id: "custom".into(),
        name: name.unwrap_or_else(|| "Custom agent".into()),
        program,
        args: words.collect(),
        json: false,
    })
}

/// A command line split into words as a shell would: whitespace between them,
/// 'single' and "double" quotes, and a backslash before a quote or a space
/// (other backslashes stay, for Windows paths).
pub fn split_command(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' && chars.peek() == Some(&'"') => word.push(chars.next().unwrap_or('"')),
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c == '\\'
                && chars
                    .peek()
                    .is_some_and(|n| matches!(n, '"' | '\'') || n.is_whitespace()) =>
            {
                word.extend(chars.next());
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        out.push(word);
    }
    out
}

// ------------------------------------------------------------------ the prompt

/// What the editor knows, for the prompt.
#[derive(Clone, Debug, Default)]
pub struct AgentContext {
    /// The mod file open in the editor, relative to the mods folder:
    /// "hello.lua" or "statue/main.lua".
    pub file: Option<String>,
    pub mods_dir: PathBuf,
    /// The files of the module the open file is in, by their paths in it
    /// (main.lua first); empty for a script.
    pub files: Vec<String>,
    /// The text selected in the editor.
    pub selection: Option<String>,
    /// The editor's problems: "hello.lua:12: unexpected symbol near 'end'".
    pub problems: Vec<String>,
}

/// The mod a file of the mods folder belongs to: "statue/main.lua" -> "statue".
fn mod_of(file: &str) -> String {
    let file = file.replace('\\', "/");
    match file.split_once('/') {
        Some((dir, _)) => dir.to_owned(),
        None => file.strip_suffix(".lua").unwrap_or(&file).to_owned(),
    }
}

/// `s` cut to `max` characters, with "…" if it was longer.
fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", s[..at].trim_end()),
        None => s.to_owned(),
    }
}

/// `s` on one line, its whitespace runs as single spaces.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const SELECTION_CHARS: usize = 4000;
const PROBLEMS: usize = 20;

/// The full prompt for the agent: what it is there for, the editor's context,
/// and the player's request.
pub fn prompt(request: &str, ctx: &AgentContext) -> String {
    let mut p = String::from(
        "You are a coding agent running inside Minecraft: the player is chatting with you from Horizon \
         Studio's in-game mod editor, in the Horizon client. You make and fix Lua script mods for Horizon \
         with the horizon-studio MCP tools:\n\
         - docs: the complete reference, every API function, event and screen widget with examples. Call \
         it first when you are unsure; don't guess function names.\n\
         - list_mods, read_mod: the mods there are, a mod's files.\n\
         - write_mod: writes a mod and checks it; while the game runs, the game reloads it and the editor \
         shows the new text.\n\
         - test_mod: tests a mod with steps (headless, or live: true in the running game).\n\
         - run_lua: runs Lua in the running game's console.\n\
         - read_script, edit_script, save_script: the script in the player's editor, live. read_script gives its \
         text as they have it (unsaved edits too); edit_script types a change into it while they watch \
         (old_text and new_text, one function or block per call; start_line one past the end appends); \
         save_script saves it and the game reloads the mod.\n\
         - open_script: opens any script (any mod, any file of a module) in the editor.\n\
         - mark_lines, clear_marks: point at code in the editor: lines highlighted with a short note beside \
         them, such as where a bug is and why (tone error, warning, good or note).\n\
         - game_status, game_log: the game, the mods' states and errors, the console.\n\
         - screenshot: a picture of the game as the player sees it (studio: true shows the editor too); \
         look with it to check what a mod draws.\n\n",
    );
    p += &format!(
        "The mods folder is {}. A mod is a script (one file, <id>.lua) or a module (a folder <id>/ whose \
         main.lua requires its other files: write_mod with some of a module's files changes only those, and its \
         remove deletes files).\n",
        ctx.mods_dir.display()
    );
    match ctx.file.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        Some(file) => {
            let id = mod_of(file);
            p += &format!(
                "The editor has {file} open ({}): mod id \"{id}\".\n",
                ctx.mods_dir.join(file).display(),
            );
            if !ctx.files.is_empty() {
                p += &format!("{file} is a file of the module {id}: {}.\n", ctx.files.join(", "));
            }
        }
        None => p += "No mod file is open in the editor.\n",
    }
    if let Some(sel) = ctx.selection.as_deref().filter(|s| !s.trim().is_empty()) {
        p += &format!(
            "The player selected this text in it:\n```lua\n{}\n```\n",
            cut(sel, SELECTION_CHARS)
        );
    }
    let problems: Vec<&String> = ctx.problems.iter().filter(|l| !l.trim().is_empty()).collect();
    if !problems.is_empty() {
        p += "The editor shows these problems:\n";
        for l in problems.iter().take(PROBLEMS) {
            p += &format!("- {}\n", cut(&one_line(l), 300));
        }
        if problems.len() > PROBLEMS {
            p += &format!("- and {} more\n", problems.len() - PROBLEMS);
        }
    }
    p += "\nHow to work:\n\
          - For the script open in the editor: read_script, then edit_script, then save_script when the change \
          is complete (edits you leave unsaved are saved when you finish, if nobody changed the file since). \
          Line numbers count from 1 in read_script's text. Save before test_mod or reload_mod: they use the \
          file on disk. Use write_mod for a new mod or files that aren't open, after reading them with \
          read_mod.\n\
          - When the player asks what is wrong, mark the lines with mark_lines and a note of one or two \
          sentences, and explain in your answer.\n\
          - Answer briefly, in plain text without Markdown: your answer is shown in a small panel in the game.\n\n\
          The player's request:\n";
    p += request.trim();
    p.push('\n');
    p
}

// ------------------------------------------------------------------ the events

/// What the agent did, as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// The agent started; its session, to resume the conversation with.
    Started { session: Option<String> },
    /// What it says (a text block, or lines of a plain-text agent).
    Text(String),
    /// A tool it calls: the call's id (which its result names), its short
    /// name ("write_mod") and a line for the panel ("write_mod statue").
    Tool {
        id: Option<String>,
        name: String,
        summary: String,
    },
    /// A tool's result, in a line, with the id of the call it answers.
    ToolResult {
        id: Option<String>,
        ok: bool,
        summary: String,
    },
    /// The run is over (always the last event). `summary`: when ok, the final
    /// answer (Claude Code's `result`, which repeats the last Text; omp's last
    /// text; none for plain-text agents); when not, why. `session` resumes it.
    Done {
        ok: bool,
        session: Option<String>,
        summary: Option<String>,
    },
    /// Something went wrong or is worth a warning (stderr lines, retries, a
    /// missing MCP server); the run goes on until Done.
    Error(String),
}

/// The horizon-studio tools that act in the player's editor: each says what
/// it did there as a line of its own in the Agent panel (one that jumps to
/// the code), so their calls and results show no line besides, unless one
/// fails.
pub const EDITOR_TOOLS: &[&str] = &["open_script", "edit_script", "save_script", "mark_lines", "clear_marks"];

/// A tool's name without the horizon-studio MCP prefix (Claude Code's
/// `mcp__horizon-studio__`, omp's `mcp__horizon_studio_`).
pub fn tool_name(full: &str) -> &str {
    ["mcp__horizon-studio__", "mcp__horizon_studio__", "mcp__horizon_studio_"]
        .iter()
        .find_map(|p| full.strip_prefix(p))
        .filter(|s| !s.is_empty())
        .unwrap_or(full)
}

/// The arguments that say what a call is about, by preference.
const KEY_ARGS: &[&str] = &[
    "id",
    "file",
    "code",
    "command",
    "line",
    "name",
    "topic",
    "search",
    "file_path",
    "path",
    "pattern",
    "query",
    "url",
    "description",
    "prompt",
];

/// A tool call on one line: its short name and its key argument, cut to ~60
/// characters ("write_mod statue", "run_lua print(1)").
pub fn tool_summary(name: &str, input: &Json) -> String {
    let short = tool_name(name);
    let value = |v: &Json| match v {
        Json::String(s) if !s.trim().is_empty() => Some(one_line(s)),
        Json::Number(n) => Some(n.to_string()),
        _ => None,
    };
    let arg = input.as_object().and_then(|o| {
        KEY_ARGS
            .iter()
            .find_map(|k| o.get(*k).and_then(value))
            .or_else(|| o.values().find_map(value))
    });
    match arg {
        Some(a) => cut(&format!("{short} {a}"), 60),
        None => short.to_owned(),
    }
}

/// The text of a tool result's content: a string, or blocks of text (and images).
fn content_text(content: &Json) -> String {
    match content {
        Json::String(s) => s.clone(),
        Json::Array(blocks) => {
            let texts: Vec<&str> = blocks.iter().filter_map(|b| b["text"].as_str()).collect();
            if texts.is_empty() && blocks.iter().any(|b| b["type"] == "image") {
                "(image)".into()
            } else {
                texts.join("\n")
            }
        }
        Json::Object(o) => o.get("content").map(content_text).unwrap_or_default(),
        _ => String::new(),
    }
}

/// A tool result on one line: for horizon-studio's JSON results, their error,
/// what they say they did (`did`), or "ok"; else the start of the text. A
/// result of several text blocks (read_script's state, then its lines) is
/// summed up by its first.
pub fn result_summary(content: &Json) -> String {
    let text = content_text(content);
    let first = content
        .as_array()
        .and_then(|blocks| blocks.iter().find_map(|b| b["text"].as_str()))
        .unwrap_or(&text);
    let json = serde_json::from_str::<Json>(text.trim()).or_else(|_| serde_json::from_str::<Json>(first.trim()));
    if let Ok(Json::Object(o)) = json {
        let error =
            |o: &serde_json::Map<String, Json>| ["error", "why"].iter().find_map(|k| o.get(*k)?.as_str().map(one_line));
        let nested = o.values().filter_map(Json::as_object).find_map(error);
        if let Some(e) = error(&o).or(nested) {
            return cut(&e, 80);
        }
        if let Some(did) = o.get("did").and_then(Json::as_str) {
            return cut(&one_line(did), 80);
        }
        if o.get("ok") == Some(&Json::Bool(true)) {
            return "ok".into();
        }
    }
    cut(&one_line(&text), 80)
}

fn parse_json(line: &str) -> Result<Json, Vec<AgentEvent>> {
    let line = line.trim();
    if line.is_empty() {
        return Err(Vec::new());
    }
    match serde_json::from_str::<Json>(line) {
        Ok(j) if j.is_object() => Ok(j),
        _ => Err(vec![AgentEvent::Text(line.to_owned())]),
    }
}

fn some_text(v: &Json) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// The events in one line of Claude Code's stream-json (see the top of this
/// file); a line that isn't JSON is text, an unknown one nothing.
pub fn parse_claude_line(line: &str) -> Vec<AgentEvent> {
    let j = match parse_json(line) {
        Ok(j) => j,
        Err(events) => return events,
    };
    let session = some_text(&j["session_id"]);
    let blocks = || j["message"]["content"].as_array().into_iter().flatten();
    match j["type"].as_str().unwrap_or("") {
        "system" => match j["subtype"].as_str().unwrap_or("") {
            "init" => {
                let mut out = vec![AgentEvent::Started { session }];
                if let Some(servers) = j["mcp_servers"].as_array() {
                    match servers.iter().find(|s| s["name"] == SERVER) {
                        None => out.push(AgentEvent::Error(format!(
                            "Claude Code has no {SERVER} MCP server: run `horizon-studio-mcp install`"
                        ))),
                        Some(s) => {
                            let status = s["status"].as_str().unwrap_or("");
                            if !matches!(status, "" | "connected" | "pending") {
                                out.push(AgentEvent::Error(format!(
                                    "Claude Code's {SERVER} MCP server: {status}"
                                )));
                            }
                        }
                    }
                }
                out
            }
            "api_retry" => vec![AgentEvent::Error(format!(
                "API error ({}), retrying ({}/{})",
                j["error"].as_str().unwrap_or("unknown"),
                j["attempt"].as_u64().unwrap_or(0),
                j["max_retries"].as_u64().unwrap_or(0)
            ))],
            _ => Vec::new(),
        },
        "assistant" => blocks()
            .filter_map(|b| match b["type"].as_str()? {
                "text" => some_text(&b["text"]).map(AgentEvent::Text),
                "tool_use" => {
                    let name = b["name"].as_str().unwrap_or("tool");
                    Some(AgentEvent::Tool {
                        id: some_text(&b["id"]),
                        name: tool_name(name).to_owned(),
                        summary: tool_summary(name, &b["input"]),
                    })
                }
                _ => None,
            })
            .collect(),
        "user" => blocks()
            .filter(|b| b["type"] == "tool_result")
            .map(|b| AgentEvent::ToolResult {
                id: some_text(&b["tool_use_id"]),
                ok: b["is_error"] != true,
                summary: result_summary(&b["content"]),
            })
            .collect(),
        "result" => {
            let subtype = j["subtype"].as_str().unwrap_or("");
            let ok = subtype == "success" && j["is_error"] != true;
            let errors: Vec<&str> = j["errors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_str)
                .collect();
            let summary = some_text(&j["result"])
                .or_else(|| (!errors.is_empty()).then(|| errors.join("; ")))
                .or_else(|| (!ok).then(|| subtype.replace('_', " ")).filter(|s| !s.is_empty()));
            vec![AgentEvent::Done { ok, session, summary }]
        }
        _ => Vec::new(),
    }
}

/// The events in one line of omp's `--mode json` stream (see the top of this
/// file); a line that isn't JSON is text, an unknown one nothing. Its Done
/// comes from its exit status.
pub fn parse_omp_line(line: &str) -> Vec<AgentEvent> {
    let j = match parse_json(line) {
        Ok(j) => j,
        Err(events) => return events,
    };
    match j["type"].as_str().unwrap_or("") {
        "session" => vec![AgentEvent::Started {
            session: some_text(&j["id"]),
        }],
        "message_end" if j["message"]["role"] == "assistant" => {
            let m = &j["message"];
            let mut out: Vec<AgentEvent> = m["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| some_text(&b["text"]).map(AgentEvent::Text))
                .collect();
            if let Some(reason @ ("error" | "aborted")) = m["stopReason"].as_str() {
                out.push(AgentEvent::Error(
                    some_text(&m["errorMessage"]).unwrap_or_else(|| format!("the answer ended: {reason}")),
                ));
            }
            out
        }
        "tool_execution_start" => {
            let name = j["toolName"].as_str().unwrap_or("tool");
            vec![AgentEvent::Tool {
                id: some_text(&j["toolCallId"]),
                name: tool_name(name).to_owned(),
                summary: tool_summary(name, &j["args"]),
            }]
        }
        "tool_execution_end" => vec![AgentEvent::ToolResult {
            id: some_text(&j["toolCallId"]),
            ok: j["isError"] != true,
            summary: result_summary(&j["result"]),
        }],
        "auto_retry_start" => vec![AgentEvent::Error(format!(
            "{}, retrying ({}/{})",
            some_text(&j["errorMessage"]).unwrap_or_else(|| "API error".into()),
            j["attempt"].as_u64().unwrap_or(0),
            j["maxAttempts"].as_u64().unwrap_or(0)
        ))],
        _ => Vec::new(),
    }
}

// ------------------------------------------------------------------ running one

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Claude,
    Omp,
    Text,
}

impl Format {
    fn of(spec: &AgentSpec) -> Format {
        match (spec.json, spec.id.as_str()) {
            (false, _) => Format::Text,
            (true, "omp") => Format::Omp,
            (true, _) => Format::Claude,
        }
    }
}

/// From the reader threads to the run.
enum Msg {
    Event(AgentEvent),
    /// A plain-text agent's line (consecutive ones become one Text).
    Line(String),
    /// A line on stderr (consecutive ones become one Error).
    Stderr(String),
    /// The agent exited, and its output stayed open a while after: something it started holds
    /// it (an MCP server, a background tool), so the reader would wait for ever.
    Exited(Option<ExitStatus>),
}

/// How long after the agent exits its output may stay open before the run ends without it.
const EXIT_GRACE: Duration = Duration::from_secs(3);

/// Whether a program is a batch file, which Windows runs through cmd.exe.
fn is_batch(program: &Path) -> bool {
    program
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// The arguments with `{prompt}` and `{session}` put in (see [`AgentSpec::args`]).
pub fn expand_args(args: &[String], prompt: &str, session: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a.contains("{session}") {
            match session.filter(|s| !s.is_empty()) {
                Some(s) => out.push(a.replace("{session}", s)),
                None => {
                    let flag = i.checked_sub(1).map(|j| &args[j]);
                    if a == "{session}" && flag.is_some_and(|f| f.starts_with('-') && !f.contains('{')) {
                        out.pop();
                    }
                }
            }
        } else {
            out.push(a.replace("{prompt}", prompt));
        }
    }
    out
}

/// The child's PATH: the program's folder first (an npm or bun install's
/// `#!/usr/bin/env node` finds its node there), then the game's PATH and the
/// usual places.
fn child_path(program: &Path) -> Option<OsString> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = program.parent().filter(|d| !d.as_os_str().is_empty()) {
        dirs.push(dir.to_path_buf());
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.extend(usual_places(home().as_deref()));
    let mut unique: Vec<PathBuf> = Vec::new();
    for d in dirs {
        if !d.as_os_str().is_empty() && !unique.contains(&d) {
            unique.push(d);
        }
    }
    std::env::join_paths(unique).ok()
}

/// A line without terminal colours and other control characters.
fn clean(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI sequences end with a letter; others are two characters.
            if chars.next() == Some('[') {
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() || d == '~' {
                        break;
                    }
                }
            }
        } else if c == '\t' {
            out.push_str("    ");
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim_end().to_owned()
}

fn lock(child: &Mutex<Child>) -> MutexGuard<'_, Child> {
    child.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Lines of a pipe, invalid UTF-8 replaced, until it closes.
fn each_line(pipe: impl Read, mut f: impl FnMut(&str) -> bool) {
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&buf);
        if !f(line.trim_end_matches(['\n', '\r'])) {
            return;
        }
    }
}

fn exit_text(status: Option<ExitStatus>) -> String {
    match status {
        Some(s) => match s.code() {
            Some(c) => format!("exit code {c}"),
            None => s.to_string(),
        },
        None => "its exit status unknown".into(),
    }
}

/// The stderr lines shown at most in a run, and their length.
const STDERR_LINES: usize = 20;
const STDERR_CHARS: usize = 400;

/// What the stdout reader keeps track of.
#[derive(Default)]
struct Seen {
    session: Option<String>,
    last_text: Option<String>,
    last_error: Option<String>,
    done: bool,
}

/// A running agent: its process, a thread reading its stdout (and one its
/// stderr), and the channel they send on.
pub struct AgentRun {
    child: Arc<Mutex<Child>>,
    pid: u32,
    rx: Receiver<Msg>,
    /// Set by the reader once it has sent Done (read by `running`, which
    /// the game doesn't call: it waits for Done).
    #[allow(dead_code)]
    over: Arc<AtomicBool>,
    /// Whether Done was handed out (or made by stop): nothing comes after it.
    done: bool,
    /// Events for the next poll (stop's).
    pending: Vec<AgentEvent>,
    session: Option<String>,
    stderr_shown: usize,
    killing: bool,
    /// The agent's name, and whether it ends with a result line (Claude Code), for a Done made
    /// when it exits without one.
    name: String,
    has_result: bool,
}

impl AgentRun {
    /// Starts the agent in `cwd` with the prompt (resuming a session if given).
    /// Its first event takes a while: Claude Code's Started comes once it has
    /// connected its MCP servers (a few seconds), its first text after the
    /// model's first answer.
    pub fn start(spec: &AgentSpec, prompt: &str, cwd: &Path, resume: Option<&str>) -> Result<AgentRun, String> {
        if !cwd.is_dir() {
            return Err(format!("{} isn't a folder", cwd.display()));
        }
        let format = Format::of(spec);
        let batch = cfg!(windows) && is_batch(&spec.program);
        let to_stdin = !spec.args.iter().any(|a| a.contains("{prompt}"));
        let in_args = if batch { one_line(prompt) } else { prompt.to_owned() };
        let args = expand_args(&spec.args, &in_args, resume);

        let mut cmd = Command::new(&spec.program);
        cmd.args(&args)
            .current_dir(cwd)
            .stdin(if to_stdin { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = child_path(&spec.program) {
            cmd.env("PATH", path);
        }
        if format == Format::Text {
            cmd.env("NO_COLOR", "1");
        }
        #[cfg(unix)]
        {
            // Its own process group, to stop it with what it started (MCP servers, tools).
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("can't start {} ({}): {e}", spec.name, spec.program.display()))?;
        let pid = child.id();
        let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let child = Arc::new(Mutex::new(child));
        let (tx, rx) = channel();
        let over = Arc::new(AtomicBool::new(false));

        if let Some(mut stdin) = stdin {
            let text = prompt.to_owned();
            thread::spawn(move || {
                let _ = stdin.write_all(text.as_bytes());
                // Dropped: closed, so the agent sees the end of its prompt.
            });
        }

        let last_stderr = Arc::new(Mutex::new(None::<String>));
        let stderr_over = Arc::new(AtomicBool::new(stderr.is_none()));
        if let Some(stderr) = stderr {
            let (tx, last, over) = (tx.clone(), last_stderr.clone(), stderr_over.clone());
            thread::spawn(move || {
                each_line(stderr, |line| {
                    let line = cut(&clean(line), STDERR_CHARS);
                    if !line.trim().is_empty() {
                        *last.lock().unwrap_or_else(PoisonError::into_inner) = Some(line.clone());
                        let _ = tx.send(Msg::Stderr(line));
                    }
                    true
                });
                over.store(true, Ordering::Release);
            });
        }

        // The watchdog: once the agent has exited, the run ends within EXIT_GRACE even if its
        // output never closes (the reader below waits for it to close).
        {
            let (tx, child, over) = (tx.clone(), child.clone(), over.clone());
            thread::spawn(move || {
                let status = loop {
                    if over.load(Ordering::Acquire) {
                        return;
                    }
                    match lock(&child).try_wait() {
                        Ok(Some(s)) => break Some(s),
                        Ok(None) => {}
                        Err(_) => break None,
                    }
                    thread::sleep(Duration::from_millis(200));
                };
                let until = Instant::now() + EXIT_GRACE;
                while Instant::now() < until {
                    if over.load(Ordering::Acquire) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                let _ = tx.send(Msg::Exited(status));
            });
        }

        let name = spec.name.clone();
        let (child2, over2) = (child.clone(), over.clone());
        thread::spawn(move || {
            let mut seen = Seen::default();
            let send = |seen: &mut Seen, e: AgentEvent| {
                match &e {
                    AgentEvent::Started { session: Some(s) } => seen.session = Some(s.clone()),
                    AgentEvent::Text(t) => seen.last_text = Some(t.clone()),
                    AgentEvent::Error(m) => seen.last_error = Some(m.clone()),
                    AgentEvent::Done { session, .. } => {
                        seen.done = true;
                        if session.is_some() {
                            seen.session.clone_from(session);
                        }
                    }
                    _ => {}
                }
                let done = seen.done;
                let _ = tx.send(Msg::Event(e));
                if done {
                    over2.store(true, Ordering::Release);
                }
            };
            if format == Format::Text {
                send(&mut seen, AgentEvent::Started { session: None });
            }
            if let Some(stdout) = stdout {
                let mut gap = false;
                each_line(stdout, |line| {
                    match format {
                        Format::Claude | Format::Omp if !seen.done => {
                            let events = if format == Format::Claude {
                                parse_claude_line(line)
                            } else {
                                parse_omp_line(line)
                            };
                            for e in events {
                                send(&mut seen, e);
                            }
                        }
                        Format::Text => {
                            let line = clean(line);
                            if line.trim().is_empty() {
                                gap = true;
                            } else {
                                let _ = tx.send(Msg::Line(if std::mem::take(&mut gap) {
                                    format!("\n{line}")
                                } else {
                                    line
                                }));
                            }
                        }
                        // After Claude Code's result: read on until it closes.
                        _ => {}
                    }
                    true
                });
            }
            // Its output ended: wait for it to exit, then for the rest of its stderr.
            let status = loop {
                match lock(&child2).try_wait() {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => {}
                    Err(_) => break None,
                }
                thread::sleep(Duration::from_millis(20));
            };
            let until = Instant::now() + Duration::from_secs(1);
            while !stderr_over.load(Ordering::Acquire) && Instant::now() < until {
                thread::sleep(Duration::from_millis(10));
            }
            if !seen.done {
                let success = status.is_some_and(|s| s.success());
                let ok = success && format != Format::Claude;
                let summary = if ok {
                    seen.last_text.take().filter(|_| format == Format::Omp)
                } else {
                    let mut why = if success {
                        format!("{name} ended without a result")
                    } else {
                        format!("{name} stopped ({})", exit_text(status))
                    };
                    let last = last_stderr.lock().unwrap_or_else(PoisonError::into_inner).take();
                    if let Some(detail) = seen.last_error.take().or(last) {
                        why = format!("{why}: {detail}");
                    }
                    Some(why)
                };
                let session = seen.session.clone();
                send(&mut seen, AgentEvent::Done { ok, session, summary });
            }
            over2.store(true, Ordering::Release);
        });

        Ok(AgentRun {
            child,
            pid,
            rx,
            over,
            done: false,
            pending: Vec::new(),
            session: None,
            stderr_shown: 0,
            killing: false,
            name: spec.name.clone(),
            has_result: format == Format::Claude,
        })
    }

    /// What arrived since the last call, without waiting; Done is the last
    /// event, and nothing comes after it.
    pub fn poll(&mut self) -> Vec<AgentEvent> {
        let mut out = std::mem::take(&mut self.pending);
        self.drain(&mut out);
        out
    }

    fn drain(&mut self, out: &mut Vec<AgentEvent>) {
        // Whether the last event out was made of lines (Text) or stderr (Error), to join the next to.
        #[derive(PartialEq)]
        enum Join {
            None,
            Lines,
            Stderr,
        }
        let mut join = Join::None;
        while !self.done {
            let Ok(msg) = self.rx.try_recv() else { break };
            match msg {
                Msg::Line(line) => {
                    if join == Join::Lines
                        && let Some(AgentEvent::Text(t)) = out.last_mut()
                    {
                        t.push('\n');
                        t.push_str(&line);
                        continue;
                    }
                    out.push(AgentEvent::Text(line.trim_start_matches('\n').to_owned()));
                    join = Join::Lines;
                }
                Msg::Stderr(line) => {
                    self.stderr_shown += 1;
                    if self.stderr_shown > STDERR_LINES {
                        if self.stderr_shown == STDERR_LINES + 1 {
                            out.push(AgentEvent::Error("(more on stderr, not shown)".into()));
                            join = Join::None;
                        }
                        continue;
                    }
                    if join == Join::Stderr
                        && let Some(AgentEvent::Error(t)) = out.last_mut()
                    {
                        t.push('\n');
                        t.push_str(&line);
                        continue;
                    }
                    out.push(AgentEvent::Error(line));
                    join = Join::Stderr;
                }
                Msg::Exited(status) => {
                    join = Join::None;
                    let success = status.is_some_and(|s| s.success());
                    let ok = success && !self.has_result;
                    let why = if success {
                        format!("{} ended without a result", self.name)
                    } else {
                        format!("{} stopped ({})", self.name, exit_text(status))
                    };
                    self.done = true;
                    out.push(AgentEvent::Done {
                        ok,
                        session: self.session.clone(),
                        summary: (!ok).then_some(why),
                    });
                }
                Msg::Event(e) => {
                    join = Join::None;
                    match &e {
                        AgentEvent::Started { session: Some(s) } | AgentEvent::Done { session: Some(s), .. } => {
                            self.session = Some(s.clone());
                        }
                        _ => {}
                    }
                    self.done = matches!(e, AgentEvent::Done { .. });
                    out.push(e);
                }
            }
        }
    }

    /// Stops the agent: kills its process (on Unix its process group: what it
    /// started too; on Windows its process tree), without waiting for it. The
    /// next poll gives what had arrived and, unless the run was over already,
    /// Done { ok: false, summary: "Stopped." } with the session to resume.
    pub fn stop(&mut self) {
        let mut out = std::mem::take(&mut self.pending);
        self.drain(&mut out);
        if !self.done {
            self.done = true;
            out.push(AgentEvent::Done {
                ok: false,
                session: self.session.clone(),
                summary: Some("Stopped.".into()),
            });
        }
        self.pending = out;
        if !self.killing {
            self.killing = true;
            let (child, pid) = (self.child.clone(), self.pid);
            thread::spawn(move || {
                // Asked to end (Claude Code then ends its tools and MCP servers), then made to.
                if !signal(&child, pid, false) {
                    return;
                }
                let until = Instant::now() + Duration::from_millis(1500);
                while Instant::now() < until {
                    thread::sleep(Duration::from_millis(50));
                    if !matches!(lock(&child).try_wait(), Ok(None)) {
                        return;
                    }
                }
                signal(&child, pid, true);
            });
        }
    }

    /// Whether it is still going: false once Done has arrived (or stop was
    /// called); a last poll then gives what is left.
    #[allow(dead_code)]
    pub fn running(&self) -> bool {
        !self.done && !self.over.load(Ordering::Acquire)
    }

    /// The session to resume the conversation with, as far as it is known.
    #[allow(dead_code)]
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// The agent's process id (on Unix also its process group's).
    #[allow(dead_code)]
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for AgentRun {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Signals the agent's process and what it started, if it still runs (the
/// lock held, so its pid can't have been reused): TERM, or KILL when `hard`.
/// Whether it still ran.
fn signal(child: &Mutex<Child>, pid: u32, hard: bool) -> bool {
    let mut c = lock(child);
    if !matches!(c.try_wait(), Ok(None)) {
        return false;
    }
    #[cfg(unix)]
    {
        // The process group (its id is the agent's pid), with the shell's POSIX kill.
        let sig = if hard { "KILL" } else { "TERM" };
        let _ = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("kill -s {sig} -- -{pid}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if hard {
            let _ = c.kill();
        }
    }
    #[cfg(windows)]
    {
        // The process tree (cmd.exe, node and the MCP servers), forced at once:
        // taskkill before the process itself goes, as it finds the tree from it.
        use std::os::windows::process::CommandExt;
        let _ = hard;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .status();
        let _ = c.kill();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (pid, hard);
        let _ = c.kill();
    }
    true
}
