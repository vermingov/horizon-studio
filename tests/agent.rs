// The in-game agent chat's backend (game::hstudio_agent): Claude Code's and
// omp's JSON streams, the prompt, finding the agents, and running fake ones
// (shell scripts printing recorded lines) to the end or stopping them.

use horizon_studio::game::hstudio_agent::{
    AgentContext, AgentEvent, AgentRun, AgentSpec, expand_args, parse_claude_line, parse_custom, parse_omp_line,
    parse_opencode_line, prompt, result_summary, split_command, tool_name, tool_summary,
};
use serde_json::json;
use std::path::{Path, PathBuf};

use AgentEvent::*;

// The fake agents are shell scripts: those tests are Unix's.
#[cfg(unix)]
use horizon_studio::game::hstudio_agent::detect_in;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hs-agent-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SESSION: &str = "3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10";

// A run of `claude -p ... --output-format stream-json --verbose`, as its
// documentation describes the lines (system/init, assistant and user messages
// with the API's content blocks, the result), trimmed of some usage fields.
const CLAUDE_RUN: &[&str] = &[
    r#"{"type":"system","subtype":"init","cwd":"/home/steve/.horizon/mods","session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","tools":["Task","Bash","Glob","Grep","Read","Edit","Write","mcp__horizon-studio__docs","mcp__horizon-studio__write_mod","mcp__horizon-studio__test_mod","mcp__horizon-studio__run_lua"],"mcp_servers":[{"name":"horizon-studio","status":"connected"}],"model":"claude-sonnet-4-5-20250929","permissionMode":"default","slash_commands":["compact","context","cost","review"],"apiKeySource":"none","claude_code_version":"2.1.314","output_style":"default","agents":["general-purpose"],"skills":[],"plugins":[],"uuid":"a2b0c6f4-1d2e-4f5a-9b8c-7d6e5f4a3b21"}"#,
    r#"{"type":"assistant","message":{"id":"msg_01HxR7a9cY3kPq2vT8mLw4Zn","type":"message","role":"assistant","model":"claude-sonnet-4-5-20250929","content":[{"type":"text","text":"I'll look up how to place blocks first."}],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":4,"cache_creation_input_tokens":2810,"cache_read_input_tokens":14301,"output_tokens":3,"service_tier":"standard"}},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"0d7f1c2e-8a9b-4c3d-b2e1-f0a9b8c7d6e5"}"#,
    r#"{"type":"assistant","message":{"id":"msg_01HxR7a9cY3kPq2vT8mLw4Zn","type":"message","role":"assistant","model":"claude-sonnet-4-5-20250929","content":[{"type":"tool_use","id":"toolu_01VnJ8kq3XQ2mZp5bR7tY9Lc","name":"mcp__horizon-studio__docs","input":{"name":"world.set_block"}}],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":4,"cache_creation_input_tokens":2810,"cache_read_input_tokens":14301,"output_tokens":61,"service_tier":"standard"}},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"6b1e2d3c-4f5a-4b6c-8d7e-9f0a1b2c3d4e"}"#,
    r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01VnJ8kq3XQ2mZp5bR7tY9Lc","type":"tool_result","content":[{"type":"text","text":"world.set_block(x, y, z, id)\nPlaces a block in the world (in single player)."}]}]},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"9c8b7a6f-5e4d-4c3b-a2f1-0e9d8c7b6a5f","tool_use_result":[{"type":"text","text":"world.set_block(x, y, z, id)\nPlaces a block in the world (in single player)."}]}"#,
    r#"{"type":"assistant","message":{"id":"msg_01Qe4Lm8Tn2Wp6Ys9Ka3Bv7D","type":"message","role":"assistant","model":"claude-sonnet-4-5-20250929","content":[{"type":"tool_use","id":"toolu_01Kp2Xz7Mq4Rn9Sw3Tv6Yb8A","name":"mcp__horizon-studio__write_mod","input":{"id":"statue","files":{"main.lua":"command(\"statue\", function()\n  -- ...\nend)\n"}}}],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":2,"cache_creation_input_tokens":512,"cache_read_input_tokens":17111,"output_tokens":240,"service_tier":"standard"}},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d"}"#,
    r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01Kp2Xz7Mq4Rn9Sw3Tv6Yb8A","type":"tool_result","content":[{"type":"text","text":"{\n  \"ok\": true,\n  \"written\": \"/home/steve/.horizon/mods/statue\",\n  \"check\": {\n    \"ok\": true,\n    \"commands\": [\"statue\"]\n  },\n  \"live\": {\n    \"id\": \"statue\",\n    \"state\": \"running\"\n  }\n}"}],"is_error":false}]},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"2b3c4d5e-6f7a-4b8c-9d0e-1f2a3b4c5d6e"}"#,
    r#"{"type":"assistant","message":{"id":"msg_01Zr5Hn3Jc8Vt1Mx6Pq9Wd2F","type":"message","role":"assistant","model":"claude-sonnet-4-5-20250929","content":[{"type":"text","text":"Saved statue: /statue builds a stone statue where you stand."}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":2,"cache_creation_input_tokens":380,"cache_read_input_tokens":17623,"output_tokens":18,"service_tier":"standard"}},"parent_tool_use_id":null,"session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","uuid":"3c4d5e6f-7a8b-4c9d-0e1f-2a3b4c5d6e7f"}"#,
    r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":18234,"duration_api_ms":16120,"num_turns":5,"result":"Saved statue: /statue builds a stone statue where you stand.","stop_reason":"end_turn","session_id":"3f1c9e52-6b7a-4d1e-9a43-0c2b8d5e7f10","total_cost_usd":0.0842,"usage":{"input_tokens":12,"cache_creation_input_tokens":3702,"cache_read_input_tokens":49035,"output_tokens":322,"server_tool_use":{"web_search_requests":0},"service_tier":"standard"},"modelUsage":{},"permission_denials":[],"uuid":"4d5e6f7a-8b9c-4d0e-1f2a-3b4c5d6e7f8a"}"#,
];

fn claude_events() -> Vec<AgentEvent> {
    vec![
        Started {
            session: Some(SESSION.into()),
        },
        Text("I'll look up how to place blocks first.".into()),
        Tool {
            id: Some("toolu_01VnJ8kq3XQ2mZp5bR7tY9Lc".into()),
            name: "docs".into(),
            summary: "docs world.set_block".into(),
        },
        ToolResult {
            id: Some("toolu_01VnJ8kq3XQ2mZp5bR7tY9Lc".into()),
            ok: true,
            summary: "world.set_block(x, y, z, id) Places a block in the world (in single player).".into(),
        },
        Tool {
            id: Some("toolu_01Kp2Xz7Mq4Rn9Sw3Tv6Yb8A".into()),
            name: "write_mod".into(),
            summary: "write_mod statue".into(),
        },
        ToolResult {
            id: Some("toolu_01Kp2Xz7Mq4Rn9Sw3Tv6Yb8A".into()),
            ok: true,
            summary: "ok".into(),
        },
        Text("Saved statue: /statue builds a stone statue where you stand.".into()),
        Done {
            ok: true,
            session: Some(SESSION.into()),
            summary: Some("Saved statue: /statue builds a stone statue where you stand.".into()),
        },
    ]
}

// omp's `--mode json` (as pi's json.md and omp's print-mode.ts have it): the
// session header, then the session's events.
const OMP_RUN: &[&str] = &[
    r#"{"type":"session","version":3,"id":"019a3b7c-5d2e-7f10-8a4b-6c9d0e1f2a3b","timestamp":"2026-10-30T14:00:00.000Z","cwd":"/home/steve/.horizon/mods"}"#,
    r#"{"type":"agent_start"}"#,
    r#"{"type":"turn_start"}"#,
    r#"{"type":"message_start","message":{"role":"user","content":"Make /statue","timestamp":1793368800000}}"#,
    r#"{"type":"message_start","message":{"role":"assistant","content":[],"api":"anthropic-messages","provider":"anthropic","model":"claude-sonnet-4-5","usage":{"input":0,"output":0},"stopReason":"pending","timestamp":1793368801000}}"#,
    r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Let me check"}}"#,
    r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Let me check the docs."},{"type":"toolCall","id":"call_1","name":"mcp__horizon_studio_docs","arguments":{"search":"set block"}}],"api":"anthropic-messages","provider":"anthropic","model":"claude-sonnet-4-5","usage":{"input":1200,"output":40},"stopReason":"toolUse","timestamp":1793368802000}}"#,
    r#"{"type":"tool_execution_start","toolCallId":"call_1","toolName":"mcp__horizon_studio_docs","args":{"search":"set block"}}"#,
    r#"{"type":"tool_execution_end","toolCallId":"call_1","toolName":"mcp__horizon_studio_docs","result":{"content":[{"type":"text","text":"world.set_block(x, y, z, id): places a block"}],"details":{}},"isError":false}"#,
    r#"{"type":"tool_execution_start","toolCallId":"call_2","toolName":"mcp__horizon_studio_run_lua","args":{"code":"print(world.block(0, 64, 0))"}}"#,
    r#"{"type":"tool_execution_end","toolCallId":"call_2","toolName":"mcp__horizon_studio_run_lua","result":{"content":[{"type":"text","text":"not linked: the game isn't running"}],"details":{}},"isError":true}"#,
    r#"{"type":"turn_end","message":{"role":"assistant","content":[],"stopReason":"toolUse"},"toolResults":[]}"#,
    r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Saved statue."}],"api":"anthropic-messages","provider":"anthropic","model":"claude-sonnet-4-5","usage":{"input":1500,"output":12},"stopReason":"stop","timestamp":1793368809000}}"#,
    r#"{"type":"agent_end","messages":[],"willRetry":false}"#,
    r#"{"type":"agent_settled","aborted":false}"#,
];

#[test]
fn claude_stream_json_becomes_events() {
    let events: Vec<AgentEvent> = CLAUDE_RUN.iter().flat_map(|l| parse_claude_line(l)).collect();
    assert_eq!(events, claude_events());
}

#[test]
fn claude_odd_lines_and_failures() {
    // Not JSON: text. Unknown or partial lines: nothing.
    assert_eq!(
        parse_claude_line("Error: not logged in"),
        vec![Text("Error: not logged in".into())]
    );
    assert_eq!(parse_claude_line(""), vec![]);
    assert_eq!(parse_claude_line(r#"{"type":"stream_event","event":{}}"#), vec![]);
    assert_eq!(
        parse_claude_line(r#"{"type":"system","subtype":"hook_started"}"#),
        vec![]
    );
    // The server missing, or failed.
    let init = |servers: serde_json::Value| {
        parse_claude_line(
            &json!({"type":"system","subtype":"init","session_id":"s1","mcp_servers":servers}).to_string(),
        )
    };
    assert_eq!(init(json!([{"name":"horizon-studio","status":"connected"}])).len(), 1);
    let missing = init(json!([{"name":"github","status":"connected"}]));
    assert!(
        matches!(&missing[1], Error(e) if e.contains("horizon-studio-mcp install")),
        "{missing:?}"
    );
    let failed = init(json!([{"name":"horizon-studio","status":"failed"}]));
    assert_eq!(
        failed[1],
        Error("Claude Code's horizon-studio MCP server: failed".into())
    );
    // A denied tool (only horizon-studio's are allowed), with a string as its content.
    let denied = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"Claude requested permissions to use Bash, but you haven't granted it yet.","is_error":true,"tool_use_id":"toolu_01A"}]},"parent_tool_use_id":null,"session_id":"s1"}"#;
    assert_eq!(
        parse_claude_line(denied),
        vec![ToolResult {
            id: Some("toolu_01A".into()),
            ok: false,
            summary: "Claude requested permissions to use Bash, but you haven't granted it yet.".into()
        }]
    );
    // Results that failed.
    let max_turns = r#"{"type":"result","subtype":"error_max_turns","is_error":true,"duration_ms":90210,"duration_api_ms":80100,"num_turns":10,"stop_reason":null,"session_id":"s1","total_cost_usd":0.31,"usage":{},"modelUsage":{},"permission_denials":[],"errors":["Reached maximum number of turns (10)"],"uuid":"u"}"#;
    assert_eq!(
        parse_claude_line(max_turns),
        vec![Done {
            ok: false,
            session: Some("s1".into()),
            summary: Some("Reached maximum number of turns (10)".into())
        }]
    );
    let login = r#"{"type":"result","subtype":"success","is_error":true,"duration_ms":310,"num_turns":1,"result":"Invalid API key · Please run /login","session_id":"s2","total_cost_usd":0}"#;
    assert_eq!(
        parse_claude_line(login),
        vec![Done {
            ok: false,
            session: Some("s2".into()),
            summary: Some("Invalid API key · Please run /login".into())
        }]
    );
    let bare = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"session_id":"s3"}"#;
    assert!(
        matches!(&parse_claude_line(bare)[0], Done { ok: false, summary: Some(s), .. } if s == "error during execution")
    );
    // A retried API request is worth a line; thinking is not shown.
    let retry = r#"{"type":"system","subtype":"api_retry","attempt":2,"max_retries":10,"retry_delay_ms":4000,"error_status":529,"error":"overloaded","uuid":"u","session_id":"s1"}"#;
    assert_eq!(
        parse_claude_line(retry),
        vec![Error("API error (overloaded), retrying (2/10)".into())]
    );
    let thinking = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm","signature":"x"},{"type":"text","text":"  "}]},"session_id":"s1"}"#;
    assert_eq!(parse_claude_line(thinking), vec![]);
}

#[test]
fn omp_json_becomes_events() {
    let events: Vec<AgentEvent> = OMP_RUN.iter().flat_map(|l| parse_omp_line(l)).collect();
    assert_eq!(
        events,
        vec![
            Started {
                session: Some("019a3b7c-5d2e-7f10-8a4b-6c9d0e1f2a3b".into())
            },
            Text("Let me check the docs.".into()),
            Tool {
                id: Some("call_1".into()),
                name: "docs".into(),
                summary: "docs set block".into()
            },
            ToolResult {
                id: Some("call_1".into()),
                ok: true,
                summary: "world.set_block(x, y, z, id): places a block".into()
            },
            Tool {
                id: Some("call_2".into()),
                name: "run_lua".into(),
                summary: "run_lua print(world.block(0, 64, 0))".into()
            },
            ToolResult {
                id: Some("call_2".into()),
                ok: false,
                summary: "not linked: the game isn't running".into()
            },
            Text("Saved statue.".into()),
        ]
    );
    let failed = r#"{"type":"message_end","message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":"429 rate limited"}}"#;
    assert_eq!(parse_omp_line(failed), vec![Error("429 rate limited".into())]);
    assert_eq!(parse_omp_line("plain words"), vec![Text("plain words".into())]);
}

#[test]
fn tools_are_summed_up_in_a_line() {
    assert_eq!(tool_name("mcp__horizon-studio__write_mod"), "write_mod");
    assert_eq!(tool_name("mcp__horizon_studio_write_mod"), "write_mod");
    assert_eq!(tool_name("Bash"), "Bash");
    assert_eq!(tool_name("mcp__github__create_issue"), "mcp__github__create_issue");
    let s = |name: &str, input: serde_json::Value| tool_summary(name, &input);
    assert_eq!(
        s(
            "mcp__horizon-studio__write_mod",
            json!({"id":"statue","files":{"main.lua":"x"}})
        ),
        "write_mod statue"
    );
    assert_eq!(
        s("mcp__horizon-studio__run_lua", json!({"code":"print(1)"})),
        "run_lua print(1)"
    );
    assert_eq!(s("mcp__horizon-studio__game_status", json!({})), "game_status");
    assert_eq!(s("mcp__horizon-studio__docs", json!({"topic":"api"})), "docs api");
    assert_eq!(
        s("Bash", json!({"command":"ls -la","description":"List"})),
        "Bash ls -la"
    );
    // Long code: on one line, cut.
    let code = "for i = 1, 100 do\n  world.set_block(i, 64, 0, 'minecraft:stone')\n  world.set_block(i, 65, 0, 'minecraft:stone')\nend";
    let long = s("mcp__horizon-studio__run_lua", json!({ "code": code }));
    assert!(
        long.starts_with("run_lua for i = 1, 100 do world.set_block(i, 64"),
        "{long}"
    );
    assert!(long.ends_with('…') && long.chars().count() <= 61, "{long}");
    // Results: horizon-studio's JSON by its error or ok, other text by its start.
    let r = |text: &str| result_summary(&json!([{ "type": "text", "text": text }]));
    assert_eq!(
        r("{\"ok\": false, \"check\": {\"ok\": false, \"error\": \"statue/main.lua:3: '=' expected near 'x'\"}}"),
        "statue/main.lua:3: '=' expected near 'x'"
    );
    assert_eq!(r("{\"ok\": true}"), "ok");
    assert_eq!(r("hello\n  world"), "hello world");
    assert_eq!(result_summary(&json!([{ "type": "image", "data": "..." }])), "(image)");
    assert_eq!(result_summary(&json!("plain")), "plain");
}

#[test]
fn the_prompt_has_the_editor_context() {
    let ctx = AgentContext {
        file: Some("statue/main.lua".into()),
        scratch: false,
        mods_dir: PathBuf::from("/home/steve/.horizon/mods"),
        files: Vec::new(),
        selection: Some("world.set_block(x, y, z, 'stone')".into()),
        problems: vec!["statue/main.lua:12: unexpected symbol near 'end'".into()],
        lesson: None,
    };
    let p = prompt("Make the statue taller", &ctx);
    for part in [
        "Horizon Studio",
        "horizon-studio MCP tools",
        "docs",
        "write_mod",
        "test_mod",
        "run_lua",
        "game_status",
        "/home/steve/.horizon/mods",
        "statue/main.lua",
        "mod id \"statue\"",
        "world.set_block(x, y, z, 'stone')",
        "statue/main.lua:12: unexpected symbol near 'end'",
        "plain text",
        "Make the statue taller",
    ] {
        assert!(p.contains(part), "{part:?} missing in:\n{p}");
    }
    assert!(p.trim_end().ends_with("Make the statue taller"));
    assert!(
        !p.starts_with('-') && !p.starts_with('/'),
        "an agent would take it for a flag or command"
    );

    // Nothing open, no selection or problems; a single-file mod's id.
    let bare = prompt(
        "hi",
        &AgentContext {
            mods_dir: PathBuf::from("/m"),
            ..Default::default()
        },
    );
    assert!(bare.contains("No mod file is open"));
    assert!(!bare.contains("selected") && !bare.contains("problems:"));
    // The scratch file open: it says so, and how the editor tools reach it.
    let scratch = prompt(
        "hi",
        &AgentContext {
            scratch: true,
            mods_dir: PathBuf::from("/m"),
            ..Default::default()
        },
    );
    assert!(
        scratch.contains("The editor has its scratch file open") && scratch.contains("scratch: true"),
        "{scratch}"
    );
    assert!(!scratch.contains("No mod file is open"));
    let one = prompt(
        "hi",
        &AgentContext {
            file: Some("hello.lua".into()),
            mods_dir: PathBuf::from("/m"),
            ..Default::default()
        },
    );
    assert!(one.contains("mod id \"hello\""));
    // A huge selection and many problems are cut.
    let big = AgentContext {
        selection: Some("x".repeat(50_000)),
        problems: (1..=40).map(|i| format!("a.lua:{i}: oops")).collect(),
        ..Default::default()
    };
    let p = prompt("fix", &big);
    assert!(p.len() < 8000, "{}", p.len());
    assert!(p.contains("a.lua:20: oops") && !p.contains("a.lua:21: oops") && p.contains("and 20 more"));
}

#[test]
fn the_prompt_names_the_modules_files() {
    let p = prompt(
        "add a file",
        &AgentContext {
            file: Some("m/lib.lua".into()),
            mods_dir: PathBuf::from("/m"),
            files: vec!["main.lua".into(), "lib.lua".into()],
            ..Default::default()
        },
    );
    assert!(
        p.contains("a module (a folder <id>/ whose main.lua requires its other files"),
        "{p}"
    );
    assert!(
        p.contains("m/lib.lua is a file of the module m: main.lua, lib.lua."),
        "{p}"
    );
    // A script's file has no such line.
    let one = prompt(
        "hi",
        &AgentContext {
            file: Some("hello.lua".into()),
            mods_dir: PathBuf::from("/m"),
            ..Default::default()
        },
    );
    assert!(!one.contains("is a file of the module"), "{one}");
}

#[test]
fn the_prompt_teaches_the_live_editor() {
    let p = prompt(
        "fix it",
        &AgentContext {
            file: Some("timer.lua".into()),
            mods_dir: PathBuf::from("/m"),
            ..Default::default()
        },
    );
    for part in [
        "read_script",
        "edit_script",
        "save_script",
        "open_script",
        "one function or block",
        "start_line one past the end appends",
        "Line numbers count from 1",
        "Save before test_mod",
        "To change another script as the player watches, open_script it first",
        "write_mod for a new mod",
        "mark_lines, clear_marks: point at code",
        "When the player asks what is wrong, mark the lines",
        "start_lesson, wait_for_player: teach in the editor",
        "When they want to learn, start_lesson, then wait_for_player",
        "call it again",
    ] {
        assert!(p.contains(part), "{part:?} missing in:\n{p}");
    }
    assert!(!p.contains("Change mods only with write_mod"), "{p}");
    assert!(!p.contains("A lesson is open"), "{p}");
    // A lesson open in the editor: where the player is in it.
    let line = "A lesson is open in the editor: \"Events\", step 3 of 5 (greeter.lua lines 12-18).";
    let p = prompt(
        "is this right?",
        &AgentContext {
            file: Some("greeter.lua".into()),
            mods_dir: PathBuf::from("/m"),
            lesson: Some(line.into()),
            ..Default::default()
        },
    );
    assert!(p.contains(&format!("{line}\n")), "{p}");
    // Everything at once stays a prompt of a reasonable size.
    let big = AgentContext {
        selection: Some("x".repeat(50_000)),
        problems: (1..=40).map(|i| format!("a.lua:{i}: oops")).collect(),
        lesson: Some("y".repeat(5000)),
        ..Default::default()
    };
    let p = prompt("fix", &big);
    assert!(p.len() < 8000, "{}", p.len());
}

#[test]
fn editor_tools_are_summed_up_by_what_they_did() {
    let r = |text: &str| result_summary(&json!([{ "type": "text", "text": text }]));
    assert_eq!(
        r("{\"ok\": true, \"did\": \"Read greeter.lua, lines 1-40 of 120\"}"),
        "Read greeter.lua, lines 1-40 of 120"
    );
    // An error still wins over what it says it did.
    assert_eq!(
        r("{\"ok\": false, \"did\": \"Typing\", \"error\": \"old_text isn't in a.lua\"}"),
        "old_text isn't in a.lua"
    );
    // read_script's two blocks (the state, then the numbered lines): the first.
    let two = json!([
        { "type": "text", "text": "{\"did\": \"Read timer.lua (12 lines)\", \"open\": true}" },
        { "type": "text", "text": "     1\tlocal t = 0" },
    ]);
    assert_eq!(result_summary(&two), "Read timer.lua (12 lines)");
    // A file of a module names the call when there is no id.
    assert_eq!(
        tool_summary(
            "mcp__horizon-studio__edit_script",
            &json!({ "file": "lib/util.lua", "old_text": "x" })
        ),
        "edit_script lib/util.lua"
    );
    assert_eq!(
        tool_summary(
            "mcp__horizon-studio__open_script",
            &json!({ "id": "kit", "file": "lib.lua" })
        ),
        "open_script kit"
    );
}

#[test]
fn arguments_get_the_prompt_and_session() {
    let spec = AgentSpec::claude(PathBuf::from("/usr/local/bin/claude"));
    assert!(spec.json);
    let fresh = expand_args(&spec.args, "Do it", None);
    assert_eq!(
        fresh,
        [
            "-p",
            "Do it",
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-mode",
            "default",
            "--allowedTools",
            "mcp__horizon-studio"
        ]
    );
    let resumed = expand_args(&spec.args, "Again", Some(SESSION));
    assert!(resumed.windows(2).any(|w| w == ["--resume", SESSION]), "{resumed:?}");
    assert_eq!(resumed.last().map(String::as_str), Some("mcp__horizon-studio"));

    let omp = AgentSpec::omp(PathBuf::from("omp"));
    assert_eq!(
        expand_args(&omp.args, "Do it", None),
        ["-p", "--mode", "json", "--approval-mode", "write", "Do it"]
    );
    assert!(
        expand_args(&omp.args, "x", Some("abc"))
            .windows(2)
            .any(|w| w == ["--resume", "abc"])
    );
    // Placeholders inside an argument.
    let args = ["--message={prompt}".to_owned(), "--session={session}".to_owned()];
    assert_eq!(expand_args(&args, "hi", None), ["--message=hi"]);
    assert_eq!(expand_args(&args, "hi", Some("7")), ["--message=hi", "--session=7"]);
}

#[test]
fn command_lines_split_like_a_shell() {
    assert_eq!(
        split_command("aider --message {prompt}"),
        ["aider", "--message", "{prompt}"]
    );
    assert_eq!(
        split_command(r#"  "C:\Program Files\Agent\agent.exe" -m '{prompt}' --x="a b" it\'s "#),
        [r"C:\Program Files\Agent\agent.exe", "-m", "{prompt}", "--x=a b", "it's"]
    );
    assert_eq!(
        split_command(r#"say "a \"quoted\" word" '' end"#),
        ["say", r#"a "quoted" word"#, "", "end"]
    );
    assert_eq!(split_command(r"C:\tools\x.exe a\ b"), [r"C:\tools\x.exe", "a b"]);
    assert!(split_command("   ").is_empty());
}

#[test]
fn agent_txt_gives_a_custom_agent() {
    let spec = parse_custom("# my agent\nname: Aider\n\naider --yes --message {prompt}\n", &[], None).unwrap();
    assert_eq!(spec.id, "custom");
    assert_eq!(spec.name, "Aider");
    assert_eq!(spec.program, PathBuf::from("aider"));
    assert_eq!(spec.args, ["--yes", "--message", "{prompt}"]);
    assert!(!spec.json);
    let unnamed = parse_custom("~/bin/agent run\n", &[], Some(Path::new("/home/steve"))).unwrap();
    assert_eq!(unnamed.name, "Custom agent");
    assert_eq!(unnamed.program, Path::new("/home/steve").join("bin/agent"));
    assert_eq!(unnamed.args, ["run"]);
    assert!(parse_custom("name: Nothing\n# no command\n", &[], None).is_none());
    assert!(parse_custom("", &[], None).is_none());
}

#[cfg(unix)]
fn write_exe(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn agents_are_found_on_the_path_and_in_usual_places() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp("detect");
    let bin = dir.join("bin");
    let home = dir.join("home");
    let studio = dir.join("studio");
    std::fs::create_dir_all(&studio).unwrap();
    write_exe(&bin.join("claude"), "#!/bin/sh\n");
    // Not executable: not an agent.
    std::fs::write(bin.join("omp"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join("omp"), std::fs::Permissions::from_mode(0o644)).unwrap();
    let found = detect_in(&studio, std::slice::from_ref(&bin), Some(&home));
    let claude = found.iter().find(|s| s.id == "claude").expect("claude");
    assert_eq!(claude.program, bin.join("claude"));
    assert_eq!(claude.name, "Claude Code");
    assert!(claude.json && claude.args.iter().any(|a| a == "{prompt}"));
    assert!(found.iter().all(|s| s.program != bin.join("omp")));
    assert!(found.iter().all(|s| s.id != "custom"));

    // omp where bun puts it, though not on the PATH; a custom agent whose program is on it.
    write_exe(&home.join(".bun").join("bin").join("omp"), "#!/bin/sh\n");
    write_exe(&bin.join("aider"), "#!/bin/sh\n");
    std::fs::write(studio.join("agent.txt"), "name: Aider\naider --message {prompt}\n").unwrap();
    let found = detect_in(&studio, std::slice::from_ref(&bin), Some(&home));
    let ids: Vec<&str> = found.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["claude", "omp", "custom"]);
    assert_eq!(found[1].program, home.join(".bun/bin/omp"));
    assert_eq!(found[2].program, bin.join("aider"));
    assert_eq!(found[2].name, "Aider");

    // Claude Code's old local install, with nothing on the PATH.
    write_exe(&home.join(".claude/local/claude"), "#!/bin/sh\n");
    let found = detect_in(&studio, &[], Some(&home));
    assert_eq!(found[0].program, home.join(".claude/local/claude"));
}

// ------------------------------------------------------------------ fake agents

/// Runs `script` (with sh: no exec of a file just written) as the agent `spec`
/// would be run, in `dir`.
#[cfg(unix)]
fn start_fake(dir: &Path, script: &str, mut spec: AgentSpec, resume: Option<&str>) -> AgentRun {
    let path = dir.join("agent.sh");
    std::fs::write(&path, script).unwrap();
    spec.program = PathBuf::from("/bin/sh");
    spec.args.insert(0, path.to_string_lossy().into_owned());
    AgentRun::start(&spec, "Build a statue", dir, resume).unwrap()
}

/// Polls until Done (or `secs` pass).
#[cfg(unix)]
fn to_the_end(run: &mut AgentRun, secs: u64) -> Vec<AgentEvent> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut out = Vec::new();
    while Instant::now() < until {
        let events = run.poll();
        let done = events.iter().any(|e| matches!(e, Done { .. }));
        out.extend(events);
        if done {
            assert!(run.poll().is_empty(), "events after Done");
            assert!(!run.running());
            return out;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no Done in {secs} s: {out:?}");
}

/// A shell script printing these lines.
#[cfg(unix)]
fn printing(lines: &[&str]) -> String {
    let mut s = String::from("printf '%s\\n' \"$@\" > \"$(dirname \"$0\")/args.txt\"\ncat <<'EOF'\n");
    for l in lines {
        s += l;
        s.push('\n');
    }
    s += "EOF\n";
    s
}

#[cfg(unix)]
#[test]
fn a_fake_claude_runs_to_its_result() {
    let dir = temp("claude");
    let spec = AgentSpec::claude(PathBuf::from("claude"));
    let mut run = start_fake(&dir, &printing(CLAUDE_RUN), spec, Some("prev-session"));
    let events = to_the_end(&mut run, 10);
    assert_eq!(events, claude_events());
    assert_eq!(run.session(), Some(SESSION));
    let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert_eq!(&args[..2], ["-p", "Build a statue"]);
    assert!(args.windows(2).any(|w| w == ["--resume", "prev-session"]), "{args:?}");
    assert!(
        args.windows(2).any(|w| w == ["--output-format", "stream-json"]),
        "{args:?}"
    );
    assert_eq!(args.last(), Some(&"mcp__horizon-studio"));
}

#[cfg(unix)]
#[test]
fn a_fake_omp_ends_with_its_exit_status() {
    let dir = temp("omp");
    let mut run = start_fake(&dir, &printing(OMP_RUN), AgentSpec::omp(PathBuf::from("omp")), None);
    let events = to_the_end(&mut run, 10);
    assert_eq!(events.len(), 8, "{events:?}");
    assert_eq!(
        events.last(),
        Some(&Done {
            ok: true,
            session: Some("019a3b7c-5d2e-7f10-8a4b-6c9d0e1f2a3b".into()),
            summary: Some("Saved statue.".into())
        })
    );
    let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
    assert_eq!(args.lines().last(), Some("Build a statue"));
    assert!(!args.contains("--resume"));
}

#[cfg(unix)]
#[test]
fn a_plain_text_agent_prints_text() {
    let dir = temp("text");
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Echo".into(),
        program: PathBuf::new(),
        args: vec!["{prompt}".into()],
        json: false,
        env: Vec::new(),
    };
    let script = "echo \"You asked: $1\"\necho\nprintf '\\033[1mbold\\033[0m done\\n'\n";
    let mut run = start_fake(&dir, script, spec, None);
    let events = to_the_end(&mut run, 10);
    assert_eq!(events[0], Started { session: None });
    let text: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text.join("\n").replace("\n\n", "\n"),
        "You asked: Build a statue\nbold done"
    );
    assert_eq!(
        events.last(),
        Some(&Done {
            ok: true,
            session: None,
            summary: None
        })
    );
}

#[cfg(unix)]
#[test]
fn without_a_prompt_argument_the_prompt_goes_to_stdin() {
    let dir = temp("stdin");
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Cat".into(),
        program: PathBuf::new(),
        args: vec![],
        json: false,
        env: Vec::new(),
    };
    let mut run = start_fake(&dir, "cat\n", spec, None);
    let events = to_the_end(&mut run, 10);
    assert!(events.contains(&Text("Build a statue".into())), "{events:?}");
}

#[cfg(unix)]
#[test]
fn an_agent_failing_without_a_result_is_done_and_not_ok() {
    let dir = temp("fail");
    let script = format!(
        "echo '{}'\necho 'error: unknown option --frobnicate' >&2\nexit 3\n",
        CLAUDE_RUN[0]
    );
    let mut run = start_fake(&dir, &script, AgentSpec::claude(PathBuf::from("claude")), None);
    let events = to_the_end(&mut run, 10);
    assert!(
        events.contains(&Error("error: unknown option --frobnicate".into())),
        "{events:?}"
    );
    match events.last() {
        Some(Done {
            ok: false,
            session,
            summary: Some(s),
        }) => {
            assert_eq!(session.as_deref(), Some(SESSION));
            assert!(s.contains("exit code 3") && s.contains("unknown option"), "{s}");
        }
        other => panic!("{other:?}"),
    }
    // Claude Code exiting 0 without its result line isn't a success either.
    let dir = temp("noresult");
    let mut run = start_fake(&dir, "echo hello\n", AgentSpec::claude(PathBuf::from("claude")), None);
    let events = to_the_end(&mut run, 10);
    assert_eq!(events[0], Text("hello".into()));
    assert!(matches!(&events[1], Done { ok: false, summary: Some(s), .. } if s.contains("without a result")));
}

#[cfg(unix)]
#[test]
fn stderr_is_shown_but_not_flooded() {
    let dir = temp("stderr");
    let script = "i=0\nwhile [ $i -lt 200 ]; do echo \"warning $i\" >&2; i=$((i+1)); done\nexit 1\n";
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Noisy".into(),
        program: PathBuf::new(),
        args: vec!["{prompt}".into()],
        json: false,
        env: Vec::new(),
    };
    let mut run = start_fake(&dir, script, spec, None);
    let events = to_the_end(&mut run, 10);
    let lines: usize = events
        .iter()
        .filter_map(|e| match e {
            Error(t) => Some(t.lines().count()),
            _ => None,
        })
        .sum();
    assert!(lines <= 21, "{lines} lines: {events:?}");
    assert!(events.contains(&Error("(more on stderr, not shown)".into())));
    assert!(matches!(events.last(), Some(Done { ok: false, summary: Some(s), .. }) if s.contains("exit code 1")));
}

/// Whether a process still runs (not gone, nor a zombie).
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let stat = String::from_utf8_lossy(&out.stdout);
    !stat.trim().is_empty() && !stat.trim().starts_with('Z')
}

#[cfg(unix)]
#[test]
fn stop_kills_the_agent_and_what_it_started() {
    let dir = temp("stop");
    // It starts a tool of its own (sleep), says so, and waits.
    let script = "sleep 60 &\necho $! > \"$(dirname \"$0\")/tool.pid\"\necho working\nwait\n";
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Slow".into(),
        program: PathBuf::new(),
        args: vec!["{prompt}".into()],
        json: false,
        env: Vec::new(),
    };
    let mut run = start_fake(&dir, script, spec, None);
    let mut seen = Vec::new();
    let until = Instant::now() + Duration::from_secs(10);
    while !seen.contains(&Text("working".into())) {
        assert!(Instant::now() < until, "{seen:?}");
        seen.extend(run.poll());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(run.running());
    let (agent, tool) = (run.pid(), std::fs::read_to_string(dir.join("tool.pid")).unwrap());
    let tool: u32 = tool.trim().parse().unwrap();
    assert!(alive(agent) && alive(tool));
    let started = Instant::now();
    run.stop();
    assert!(started.elapsed() < Duration::from_millis(500), "stop waited");
    assert!(!run.running());
    assert_eq!(
        run.poll(),
        vec![Done {
            ok: false,
            session: None,
            summary: Some("Stopped.".into())
        }]
    );
    assert!(run.poll().is_empty());
    let until = Instant::now() + Duration::from_secs(5);
    while alive(agent) || alive(tool) {
        assert!(
            Instant::now() < until,
            "still running: agent {} tool {}",
            alive(agent),
            alive(tool)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // Stopping again, or dropping it, does nothing more.
    run.stop();
    assert!(run.poll().is_empty());
}

#[cfg(unix)]
#[test]
fn dropping_a_run_stops_it() {
    let dir = temp("drop");
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Slow".into(),
        program: PathBuf::new(),
        args: vec![],
        json: false,
        env: Vec::new(),
    };
    let run = start_fake(&dir, "sleep 60\n", spec, None);
    let pid = run.pid();
    assert!(alive(pid));
    drop(run);
    let until = Instant::now() + Duration::from_secs(5);
    while alive(pid) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_missing_program_or_folder_is_an_error() {
    let spec = AgentSpec::claude(PathBuf::from("/nonexistent/claude"));
    let err = AgentRun::start(&spec, "hi", &std::env::temp_dir(), None).err().unwrap();
    assert!(err.contains("can't start Claude Code"), "{err}");
    let err = AgentRun::start(&spec, "hi", Path::new("/nonexistent/folder"), None)
        .err()
        .unwrap();
    assert!(err.contains("isn't a folder"), "{err}");
}

#[cfg(unix)]
#[test]
fn an_agent_whose_output_something_else_keeps_open_is_still_done() {
    // It exits, but a process it started holds its stdout for a minute: the
    // run ends within a few seconds of the exit anyway, not when that closes.
    let dir = temp("held");
    let spec = AgentSpec {
        id: "custom".into(),
        name: "Holder".into(),
        program: PathBuf::new(),
        args: vec!["{prompt}".into()],
        json: false,
        env: Vec::new(),
    };
    let script = "echo working on it\n(sleep 60) &\nexit 0\n";
    let started = Instant::now();
    let mut run = start_fake(&dir, script, spec, None);
    let events = to_the_end(&mut run, 15);
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    assert!(events.contains(&Text("working on it".into())), "{events:?}");
    assert_eq!(
        events.last(),
        Some(&Done {
            ok: true,
            session: None,
            summary: None
        })
    );
}

#[test]
fn a_model_is_passed_where_each_agent_takes_it() {
    // Claude Code: right after -p, so --allowedTools (which takes several values) stays last.
    let claude = AgentSpec::claude(PathBuf::from("claude")).with_model("sonnet");
    let args = expand_args(&claude.args, "Build a statue", None);
    let at = args.iter().position(|a| a == "--model").expect("--model");
    assert_eq!(args[at + 1], "sonnet");
    assert_eq!(args[0], "-p");
    assert_eq!(at, 1);
    let tools = args.iter().position(|a| a == "--allowedTools").unwrap();
    assert_eq!(tools, args.len() - 2, "{args:?}");
    assert!(args.contains(&"Build a statue".to_owned()));
    // omp too, with a provider's name.
    let omp = AgentSpec::omp(PathBuf::from("omp")).with_model(" anthropic/claude-sonnet ");
    let args = expand_args(&omp.args, "hi", None);
    assert_eq!(&args[..3], ["-p", "--model", "anthropic/claude-sonnet"]);
    // No model, or an agent of one's own: as they were.
    let plain = AgentSpec::claude(PathBuf::from("claude"));
    assert_eq!(plain.with_model("  "), plain);
    let custom = AgentSpec {
        id: "custom".into(),
        name: "Mine".into(),
        program: PathBuf::from("aider"),
        args: vec!["--message".into(), "{prompt}".into()],
        json: false,
        env: Vec::new(),
    };
    assert_eq!(custom.with_model("opus"), custom);
}

/// An OpenCode run (`opencode run --format json`), as its run command prints it.
const OPENCODE_RUN: &[&str] = &[
    r#"{"type":"step_start","timestamp":1,"sessionID":"ses_abc","part":{"type":"step-start"}}"#,
    r#"{"type":"text","timestamp":2,"sessionID":"ses_abc","part":{"type":"text","text":"Let me check the docs."}}"#,
    r#"{"type":"tool_use","timestamp":3,"sessionID":"ses_abc","part":{"type":"tool","id":"prt_1","callID":"call_1","tool":"horizon-studio_docs","state":{"status":"completed","input":{"query":"set block"},"output":"world.set_block(x, y, z, id): places a block","title":"docs"}}}"#,
    r#"{"type":"tool_use","timestamp":4,"sessionID":"ses_abc","part":{"type":"tool","id":"prt_2","callID":"call_2","tool":"horizon-studio_run_lua","state":{"status":"error","input":{"code":"print(world.block(0, 64, 0))"},"error":"not linked: the game isn't running"}}}"#,
    r#"{"type":"text","timestamp":5,"sessionID":"ses_abc","part":{"type":"text","text":"Saved statue."}}"#,
    r#"{"type":"step_finish","timestamp":6,"sessionID":"ses_abc","part":{"type":"step-finish","reason":"stop"}}"#,
];

#[test]
fn opencode_json_becomes_events() {
    let events: Vec<AgentEvent> = OPENCODE_RUN
        .iter()
        .flat_map(|l| parse_opencode_line(l))
        .filter(|e| !matches!(e, Started { .. }))
        .collect();
    assert_eq!(
        events,
        vec![
            Text("Let me check the docs.".into()),
            Tool {
                id: Some("call_1".into()),
                name: "docs".into(),
                summary: "docs set block".into()
            },
            ToolResult {
                id: Some("call_1".into()),
                ok: true,
                summary: "world.set_block(x, y, z, id): places a block".into()
            },
            Tool {
                id: Some("call_2".into()),
                name: "run_lua".into(),
                summary: "run_lua print(world.block(0, 64, 0))".into()
            },
            ToolResult {
                id: Some("call_2".into()),
                ok: false,
                summary: "not linked: the game isn't running".into()
            },
            Text("Saved statue.".into()),
        ]
    );
    assert_eq!(
        parse_opencode_line(OPENCODE_RUN[0]),
        vec![Started {
            session: Some("ses_abc".into())
        }]
    );
    let failed = r#"{"type":"error","timestamp":7,"sessionID":"ses_abc","error":{"name":"APIError","data":{"message":"429 rate limited"}}}"#;
    assert_eq!(
        parse_opencode_line(failed),
        vec![
            Started {
                session: Some("ses_abc".into())
            },
            Error("429 rate limited".into())
        ]
    );
    assert_eq!(tool_name("horizon-studio_write_mod"), "write_mod");
}

#[test]
fn opencode_runs_without_commands_or_edits_and_takes_a_model() {
    let spec = AgentSpec::opencode(PathBuf::from("opencode"));
    assert_eq!(spec.name, "OpenCode");
    assert_eq!(&spec.args[..3], ["run", "--format", "json"]);
    let (k, v) = &spec.env[0];
    assert_eq!(k, "OPENCODE_CONFIG_CONTENT");
    let config: serde_json::Value = serde_json::from_str(v).unwrap();
    assert_eq!(config["permission"]["bash"], "deny");
    assert_eq!(config["permission"]["edit"], "deny");
    let with = spec.with_model("anthropic/claude-sonnet-4-5");
    assert_eq!(&with.args[..3], ["run", "--model", "anthropic/claude-sonnet-4-5"]);
    let args = expand_args(&spec.args, "Build a statue", None);
    assert_eq!(args, ["run", "--format", "json", "Build a statue"]);
    let args = expand_args(&spec.args, "Build a statue", Some("ses_abc"));
    assert_eq!(
        args,
        ["run", "--format", "json", "--session", "ses_abc", "Build a statue"]
    );
}

#[cfg(unix)]
#[test]
fn a_fake_opencode_runs_to_its_end_with_its_config() {
    let dir = temp("opencode");
    let script = format!(
        "printf '%s' \"$OPENCODE_CONFIG_CONTENT\" > \"$(dirname \"$0\")/config.txt\"\n{}",
        printing(OPENCODE_RUN)
    );
    let mut run = start_fake(
        &dir,
        &script,
        AgentSpec::opencode(PathBuf::from("opencode")),
        Some("ses_abc"),
    );
    let events = to_the_end(&mut run, 10);
    assert_eq!(
        events.first(),
        Some(&Started {
            session: Some("ses_abc".into())
        })
    );
    assert_eq!(
        events.iter().filter(|e| matches!(e, Started { .. })).count(),
        1,
        "{events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&Done {
            ok: true,
            session: Some("ses_abc".into()),
            summary: Some("Saved statue.".into())
        })
    );
    let args = std::fs::read_to_string(dir.join("args.txt")).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert!(args.windows(2).any(|w| w == ["--session", "ses_abc"]), "{args:?}");
    assert_eq!(args.last(), Some(&"Build a statue"));
    let config = std::fs::read_to_string(dir.join("config.txt")).unwrap();
    assert!(config.contains("\"bash\":\"deny\""), "{config}");
}

#[cfg(unix)]
#[test]
fn opencode_is_found_where_its_installer_puts_it() {
    let dir = temp("opencode-find");
    let home = dir.join("home");
    let bin = home.join(".opencode").join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("opencode");
    std::fs::write(&exe, "#!/bin/sh\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let found = detect_in(&dir, &[], Some(&home));
    assert!(
        found.iter().any(|a| a.id == "opencode" && a.program == exe),
        "{found:?}"
    );
}
