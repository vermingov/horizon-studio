//! `horizon-studio-mcp install`: registers the server with the AI agents on
//! this machine, as a stdio server running this program. Claude Code through
//! its own `claude mcp add` (user scope); omp, Cursor, Gemini CLI, Claude
//! Desktop and OpenCode in their JSON configuration, Codex in its TOML; each
//! only when it is installed. Other entries in those files are kept.

use serde_json::{Value as Json, json};
use std::path::{Path, PathBuf};
use std::process::Command;

const NAME: &str = crate::mcp::SERVER_NAME;

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Whether a program is on the PATH.
fn on_path(name: &str) -> bool {
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| exts.iter().any(|e| dir.join(format!("{name}{e}")).is_file()))
    })
}

/// Runs a program (through `cmd /c` on Windows, for .cmd shims).
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = if cfg!(windows) {
        Command::new("cmd").arg("/c").arg(program).args(args).output()
    } else {
        Command::new(program).args(args).output()
    }
    .map_err(|e| e.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(text)
    } else {
        Err(text.trim().to_owned())
    }
}

/// Adds or removes `<key>.horizon-studio` in a JSON file (made if missing).
fn edit_json(path: &Path, key: &str, entry: Option<Json>) -> Result<(), String> {
    let mut root: Json = match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .map_err(|e| format!("{} isn't plain JSON ({e}): add it by hand", path.display()))?,
        _ => json!({}),
    };
    let obj = root
        .as_object_mut()
        .ok_or_else(|| format!("{} isn't a JSON object", path.display()))?;
    let servers = obj.entry(key).or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| format!("{key} in {} isn't an object", path.display()))?;
    match entry {
        Some(e) => {
            servers.insert(NAME.into(), e);
        }
        None => {
            if servers.remove(NAME).is_none() {
                return Ok(());
            }
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// Adds or removes `[mcp_servers.horizon-studio]` in Codex's config.toml.
fn edit_codex(path: &Path, exe: Option<&str>) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let header = format!("[mcp_servers.{NAME}]");
    let mut out = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        let t = line.trim();
        if t == header || t.starts_with(&format!("[mcp_servers.{NAME}.")) {
            skipping = true;
            continue;
        }
        if skipping && t.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            out.push(line.to_owned());
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    if let Some(exe) = exe {
        if !out.is_empty() {
            out.push(String::new());
        }
        let quoted = serde_json::to_string(exe).map_err(|e| e.to_string())?;
        out.push(header);
        out.push(format!("command = {quoted}"));
        out.push("args = []".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, out.join("\n") + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// Registers this program (`Some(path)`) or removes it (`None`); says where.
type Apply = Box<dyn Fn(Option<&str>) -> Result<String, String>>;

struct Client {
    name: &'static str,
    /// Whether it is installed here.
    present: bool,
    apply: Apply,
}

fn clients() -> Vec<Client> {
    let home = home().unwrap_or_default();
    let mut out = Vec::new();
    // Claude Code.
    let claude_json = home.join(".claude.json");
    out.push(Client {
        name: "Claude Code",
        present: on_path("claude") || claude_json.is_file(),
        apply: Box::new(move |exe| {
            if on_path("claude") {
                let _ = run("claude", &["mcp", "remove", "--scope", "user", NAME]);
                if let Some(exe) = exe {
                    run("claude", &["mcp", "add", "--scope", "user", NAME, "--", exe])?;
                }
                return Ok("claude mcp add --scope user".into());
            }
            let entry = exe.map(|e| json!({ "type": "stdio", "command": e, "args": [], "env": {} }));
            edit_json(&claude_json, "mcpServers", entry).map(|()| claude_json.display().to_string())
        }),
    });
    // omp (oh-my-pi).
    let omp_dir = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".omp").join("agent"));
    let omp = omp_dir.join("mcp.json");
    out.push(Client {
        name: "omp",
        present: on_path("omp") || home.join(".omp").is_dir(),
        apply: Box::new(move |exe| {
            let entry = exe.map(|e| json!({ "type": "stdio", "command": e, "args": [] }));
            edit_json(&omp, "mcpServers", entry).map(|()| omp.display().to_string())
        }),
    });
    // Codex.
    let codex = home.join(".codex").join("config.toml");
    out.push(Client {
        name: "Codex",
        present: on_path("codex") || home.join(".codex").is_dir(),
        apply: Box::new(move |exe| edit_codex(&codex, exe).map(|()| codex.display().to_string())),
    });
    // Cursor and Gemini CLI: mcpServers in their JSON.
    for (name, file, dir) in [
        ("Cursor", home.join(".cursor").join("mcp.json"), home.join(".cursor")),
        (
            "Gemini CLI",
            home.join(".gemini").join("settings.json"),
            home.join(".gemini"),
        ),
    ] {
        out.push(Client {
            name,
            present: dir.is_dir(),
            apply: Box::new(move |exe| {
                let entry = exe.map(|e| json!({ "command": e, "args": [] }));
                edit_json(&file, "mcpServers", entry).map(|()| file.display().to_string())
            }),
        });
    }
    // Claude Desktop.
    let desktop_dir = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("Claude")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Claude")
    } else {
        home.join(".config").join("Claude")
    };
    let desktop = desktop_dir.join("claude_desktop_config.json");
    out.push(Client {
        name: "Claude Desktop",
        present: desktop_dir.is_dir(),
        apply: Box::new(move |exe| {
            let entry = exe.map(|e| json!({ "command": e, "args": [] }));
            edit_json(&desktop, "mcpServers", entry).map(|()| desktop.display().to_string())
        }),
    });
    // OpenCode.
    let opencode_dir = home.join(".config").join("opencode");
    let opencode = opencode_dir.join("opencode.json");
    out.push(Client {
        name: "OpenCode",
        present: on_path("opencode") || opencode_dir.is_dir(),
        apply: Box::new(move |exe| {
            let entry = exe.map(|e| json!({ "type": "local", "command": [e], "enabled": true }));
            edit_json(&opencode, "mcp", entry).map(|()| opencode.display().to_string())
        }),
    });
    out
}

/// `--only claude,omp`: the clients to touch (by name, any case, a prefix).
fn wanted(args: &[String]) -> Option<Vec<String>> {
    let i = args.iter().position(|a| a == "--only")?;
    Some(
        args.get(i + 1)?
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

fn picked(client: &Client, only: &Option<Vec<String>>) -> bool {
    match only {
        None => client.present,
        Some(names) => {
            let n = client.name.to_lowercase();
            names
                .iter()
                .any(|w| n.starts_with(w.as_str()) || n.replace(' ', "-") == *w)
        }
    }
}

fn snippet(exe: &str) -> String {
    let entry = json!({ "mcpServers": { NAME: { "command": exe, "args": [] } } });
    serde_json::to_string_pretty(&entry).unwrap_or_default()
}

pub fn install(args: &[String]) -> i32 {
    let exe = match std::env::current_exe().and_then(|p| p.canonicalize().or(Ok(p))) {
        Ok(p) => p.display().to_string(),
        Err(e) => {
            eprintln!("Can't tell where this program is: {e}");
            return 1;
        }
    };
    if args.iter().any(|a| a == "--print") {
        println!("{}", snippet(&exe));
        return 0;
    }
    let only = wanted(args);
    let mut done = 0;
    let mut failed = 0;
    println!("Registering {NAME} ({exe}):");
    for c in clients() {
        if !picked(&c, &only) {
            continue;
        }
        match (c.apply)(Some(&exe)) {
            Ok(where_) => {
                done += 1;
                println!("  {:<15} added ({where_})", c.name);
            }
            Err(e) => {
                failed += 1;
                println!("  {:<15} not added: {e}", c.name);
            }
        }
    }
    if done == 0 && failed == 0 {
        println!("  No agent found here (Claude Code, omp, Codex, Cursor, Gemini CLI, Claude Desktop, OpenCode).");
    }
    println!(
        "\nAny other MCP client: a stdio server with the command {exe:?} and no arguments:\n{}\n\n\
         Restart your agent (or reload its MCP servers), then ask it to call `game_status`.",
        snippet(&exe)
    );
    i32::from(failed > 0 && done == 0)
}

pub fn uninstall(args: &[String]) -> i32 {
    let only = wanted(args);
    for c in clients() {
        if !picked(&c, &only) {
            continue;
        }
        match (c.apply)(None) {
            Ok(_) => println!("  {:<15} removed", c.name),
            Err(e) => println!("  {:<15} not changed: {e}", c.name),
        }
    }
    0
}
