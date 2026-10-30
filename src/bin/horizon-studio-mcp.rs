// horizon-studio-mcp: Horizon Studio's MCP server for AI agents (stdio), and
// its installer: `horizon-studio-mcp install` registers it with the agents on
// this machine (Claude Code, omp, Codex, Cursor, Gemini CLI, Claude Desktop).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => horizon_studio::mcp::serve(),
        Some("install") => std::process::exit(horizon_studio::mcp_install::install(&args[1..])),
        Some("uninstall") => std::process::exit(horizon_studio::mcp_install::uninstall(&args[1..])),
        Some("--version" | "-V" | "version") => {
            println!("horizon-studio-mcp {}", horizon_studio::game::hstudio::VERSION)
        }
        Some(_) => {
            println!(
                "horizon-studio-mcp {}: Horizon Studio for AI agents (MCP over stdio)\n\n\
                 Usage:\n  horizon-studio-mcp            serve MCP on stdin/stdout (what agents run)\n  \
                 horizon-studio-mcp install    register it with Claude Code, omp, Codex, Cursor, Gemini CLI, Claude Desktop\n  \
                 horizon-studio-mcp uninstall  remove it from them\n  horizon-studio-mcp --version",
                horizon_studio::game::hstudio::VERSION
            );
        }
    }
}
