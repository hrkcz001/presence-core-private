//! presence — presence's own body. ACP agent server (Rust).
//! M2: tools (read_file, write_file desk-scoped, list_files,
//! run_command bounded) + OpenAI tool-call loop through the bridge +
//! exact token ledger. System prompt: system constitution + operational
//! directives (two-channel format, desk, private thoughts).
//!
//! Wire: JSON-RPC 2.0 over stdio, newline-delimited.

mod acp;
mod asides;
mod budgeter;
mod config;
mod daemon;
mod friction;
mod i18n;
mod llm;
mod phase;
mod prompt;
mod quests;
mod tools;
mod vitals;
mod winsense;
mod capabilities;
mod senses;
mod sandbox;
mod stembus;

use llm::Bridge;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    topic: String,
    role: String,
    content: String,
}

/// The daemon's head: one shared transcript, tagged by topic.
struct Head {
    transcript: Vec<Entry>,
    /// acp session id -> topic
    topics: HashMap<String, String>,
    /// acp session id -> discovered workspace root
    roots: HashMap<String, PathBuf>,
}

/// workspace root for a session: config (env > yaml) first; the ACP
/// cwd is the fallback — the classic env surface is PRESENCE_WORKSPACE.
fn find_PRESENCE_WORKSPACE(start: &Path) -> Option<PathBuf> {
    config::Config::cached().workspace_path().or_else(|| Some(start.to_path_buf()))
}

/// Natural-language goal detection: is this message a standing task
/// (worth the daemon circle) or just conversation? One tiny LLM call.
/// Cheap lexical pre-filter: obvious goals and obvious chat skip the
/// LLM probe entirely (the probe was adding 2-5s to every reply).
/// Card body without system noise: no exit codes, no stderr blocks,
/// no trailing truncation markers, byte counts trimmed.

fn format_tool_card_markdown(name: &str, args: &Value, outcome: &str) -> String {
    let trimmed = outcome.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match name {
        "read_file" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            let lang = tools::detect_language(path);
            tools::format_markdown_block(trimmed, lang)
        }
        "run_command" => {
            tools::format_markdown_block(trimmed, Some("console"))
        }
        "webfetch" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("");
            if action == "fetch" {
                if trimmed.starts_with('{') && trimmed.ends_with('}') {
                    tools::format_markdown_block(trimmed, Some("json"))
                } else if trimmed.contains("<!DOCTYPE") || trimmed.contains("<html") {
                    tools::format_markdown_block(trimmed, Some("html"))
                } else {
                    tools::format_markdown_block(trimmed, Some("markdown"))
                }
            } else {
                tools::format_markdown_block(trimmed, Some("markdown"))
            }
        }
        _ => {
            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
            {
                tools::format_markdown_block(trimmed, Some("json"))
            } else if trimmed.contains('\n') {
                tools::format_markdown_block(trimmed, None)
            } else {
                trimmed.to_string()
            }
        }
    }
}

fn _clean_outcome(name: &str, outcome: &str, ok: bool) -> String {
    if !ok {
        return outcome.to_string();
    }
    let mut body = outcome
        .lines()
        .skip_while(|l| l.starts_with("exit:"))
        .collect::<Vec<_>>()
        .join("
");
    // drop the [stderr] section entirely
    if let Some(idx) = body.find("[stderr]") {
        body.truncate(idx);
    }
    if let Some(idx) = body.find("... [truncated]") {
        body.truncate(idx);
    }
    body = body.trim().to_string();
    match name {
        "write_file" => body, // "wrote path (N bytes)" is already clean
        "read_file" => {
            let mut chars = body.chars();
            let _ = &mut chars;
            let shown: String = body.lines().take(3).collect::<Vec<_>>().join("
");
            if body.lines().count() > 3 { format!("{shown}
… ({} lines)", body.lines().count()) } else { shown }
        }
        _ => body,
    }
}

fn prompt_text(msg: &Value) -> String {
    msg.pointer("/params/prompt")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(Value::as_str) == Some("text") {
                        b.get("text").and_then(Value::as_str).map(str::to_owned)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Append one usage record to the token ledger (JSONL, runtime-owned).
fn ledger_append(ledger_path: &Path, session_id: &str, topic: &str, usage: &Value) {
    let rec = json!({
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "session": session_id,
        "topic": topic,
        "prompt_tokens": usage.pointer("/prompt_tokens").cloned().unwrap_or(Value::Null),
        "completion_tokens": usage.pointer("/completion_tokens")
            .cloned()
            .unwrap_or(Value::Null),
        "total_tokens": usage.pointer("/total_tokens").cloned().unwrap_or(Value::Null),
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ledger_path) {
        if let Ok(line) = serde_json::to_string(&rec) {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
    }
}

/// One model round inside a turn: LLM call + tool executions, looping
/// until the model replies without tool_calls. Returns the final text.
fn agent_loop(
    bridge: &mut Bridge,
    url: &str,
    api_key: &str,
    model: &str,
    messages: &mut Vec<Value>,
    tool_defs: &Vec<Value>,
    tool_ctx: std::sync::Arc<tools::ToolCtx>,
    session_id: &str,
    ledger_path: &Path,
    topic: &str,
    tool_rounds_max: usize,
) -> Result<String, String> {
    agent_loop_mode(bridge, url, api_key, model, messages, tool_defs, std::sync::Arc::clone(&tool_ctx), session_id, ledger_path, topic, tool_rounds_max, false, None, 8000)
}

/// quiet = internal phase (1-3): content does NOT stream to the user
/// chat — only the phase indicator + tool calls are visible. Vollzug
/// and plain turns use the streaming variant.
#[allow(clippy::too_many_arguments)]
fn agent_loop_mode(
    bridge: &mut Bridge,
    url: &str,
    api_key: &str,
    model: &str,
    messages: &mut Vec<Value>,
    tool_defs: &Vec<Value>,
    tool_ctx: std::sync::Arc<tools::ToolCtx>,
    session_id: &str,
    ledger_path: &Path,
    topic: &str,
    tool_rounds_max: usize,
    quiet: bool,
    phase_max_tokens: Option<u64>,
    tool_output_cap: usize,
) -> Result<String, String> {
    let mut rounds = 0usize;
    loop {
        let message_id = format!("m-{}", Uuid::new_v4());
        let mut result = Err("no attempt".to_string());
        let cfg = crate::config::Config::cached();
        let mut failover = cfg.failover_stack();
        let mut cur_url = url.to_string();
        let mut cur_api_key = api_key.to_string();
        let mut cur_model = model.to_string();
        let mem_dir = cfg.memory_dir_path();

        for attempt in 1..=3u32 {
            // CAP_EMPTY = the cap ate the reply, not a model failure:
            // widen the cap on the next attempt instead of erroring out
            let attempt_cap = if attempt > 1 && matches!(&result, Err(e) if e.contains("CAP_EMPTY")) {
                phase_max_tokens.map(|t| t * 3)
            } else {
                phase_max_tokens
            };
            result = bridge.chat_mode(
                &cur_url, &cur_api_key, &cur_model, messages, Some(tool_defs), session_id, &message_id, quiet,
                attempt_cap,
            );
            if result.is_ok() {
                break;
            }
            let msg = result.as_ref().unwrap_err().clone();
            let retryable = msg.contains("api 5")
                || msg.contains("401")
                || msg.contains("429")
                || msg.contains("request failed")
                || msg.contains("stream error")
                || msg.contains("bridge")
                || msg.contains("CAP_EMPTY");
            eprintln!("presence: attempt {attempt} failed: {msg}");
            if attempt == 3 || !retryable {
                break;
            }
            if let Ok(tier) = failover.on_error(&msg) {
                if let Some(m_dir) = &mem_dir {
                    failover.record_failover_friction(m_dir, session_id, &tier, &msg);
                }
                if let Some(next_t) = failover.current_target() {
                    cur_url = format!("{}/chat/completions", next_t.base_url.trim_end_matches('/'));
                    cur_api_key = next_t.api_key;
                    cur_model = next_t.model_name;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(if attempt == 1 { 2 } else { 5 }));
        }
        let result = result?;
        if let Some(u) = &result.usage {
            ledger_append(ledger_path, session_id, topic, u);
        }
        rounds += 1;

        if result.tool_calls.is_empty() {
            return Ok(result.content);
        }

        // assistant turn with tool_calls must be recorded for the model
        messages.push(json!({
            "role": "assistant",
            "content": result.content,
            "tool_calls": result.tool_calls.iter().map(|tc| json!({
                "id": tc.id,
                "type": "function",
                "function": {"name": tc.name, "arguments": tc.arguments},
            })).collect::<Vec<_>>(),
        }));

        let mut handles: Vec<std::thread::JoinHandle<String>> = Vec::new();
        let mut pending_outcomes = Vec::new();
        for tc in &result.tool_calls {
            let kind = match tc.name.as_str() {
                "read_file" | "list_files" => "read",
                "write_file" => "edit",
                "run_command" => "execute",
                _ => "other",
            };
            let args: Value = serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
            // user-friendly titles: icons, no brackets/quotes; the full
            // invocation lives in the card body (expandable in Zed)
            let title = match tc.name.as_str() {
                "read_file" => format!("\u{274F} Read: {}", args.get("path").and_then(Value::as_str).unwrap_or("?")),
                "write_file" => format!("\u{25D2} Write: {}", args.get("path").and_then(Value::as_str).unwrap_or("?")),
                "list_files" => format!("\u{205E} List: {}", args.get("path").and_then(Value::as_str).unwrap_or("desk")),
                "run_command" => {
                    let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
                    let head: String = cmd.chars().take(60).collect();
                    format!("\u{2727} Run: {head}")
                }
                "speak" => {
                    let t = args.get("text").and_then(Value::as_str).unwrap_or("");
                    let head: String = t.chars().take(40).collect();
                    format!("\u{2731} Say: {head}")
                }
                "listen" => format!("\u{2299} Listen: {}s", args.get("seconds").and_then(Value::as_u64).unwrap_or(6)),
                "context.pin" => format!("\u{25C8} Pin: {}", args.get("path").and_then(Value::as_str).unwrap_or("?")),
                "context.evict" => format!("\u{25C7} Unpin: {}", args.get("path").and_then(Value::as_str).unwrap_or("?")),
                "context.status" => "\u{2261} Context status".to_string(),
                other => other.to_string(),
            };
            // full invocation into the card body (first line), outcome below
            let full_invocation = match tc.name.as_str() {
                "run_command" => args.get("command").and_then(Value::as_str).unwrap_or("").to_string(),
                "read_file" | "write_file" | "list_files" | "context.pin" | "context.evict" => {
                    args.get("path").and_then(Value::as_str).unwrap_or("").to_string()
                }
                _ => tc.arguments.clone(),
            };
            // during quiet phases, internal introspection (own memory,
            // presence, .git, target) is hidden from the owner
            // true introspection only: own brain files, build output.
            // Working files (AUTONOMY.md, PLAN.md, src/...) stay visible
            // — they are the object of labor, not self-inspection.
            let internal_path = |p: &str| {
                p.contains("memory")
                    || p.contains("friction.jsonl")
                    || p.contains("ledger.jsonl")
                    || p.contains("transcript.jsonl")
                    || p.contains("vitals.jsonl")
                    || p.contains("mood.md")
                    || p.contains("heard.jsonl")
                    || p.contains("/target/")
                    || p.ends_with("/target")
                    || p.starts_with("nul")
            };
            // system-work = touching presence's own organs (constitution,
            // memory, journals, governance, agents). The owner doesn't
            // need paths for this: one clean card, "what is happening".
            let path_is_system_work = |p: &str| {
                p.contains("presence")
                    || p.contains("memory/")
                    || p.contains("AGENTS.md")
                    || p.contains("MACHINE.md")
                    || p.contains("governance/")
                    || p.contains("agents/")
                    || p.contains("owner/")
                    || p.contains("workspace/converse")
                    || p.contains("workspace/ear")
            };
            let cmd_is_system_work = |c: &str| {
                c.contains("presence")
                    || c.contains("memory/")
                    || c.contains("AGENTS.md")
                    || c.contains("WORKLOG")
                    || c.contains("STATE.md")
                    || c.contains("governance/")
                    || c.contains("sessions.jsonl")
                    || c.contains("goal.jsonl")
                    || c.contains("mood.md")
                    || c.contains("vitals.jsonl")
                    || c.contains("friction.jsonl")
                    || c.contains("ledger.jsonl")
                    || c.contains("transcript.jsonl")
            };
            // system-work classification: introspection -> fully hidden;
            // system housekeeping -> generic card (no paths), house icon
            let is_introspection = match tc.name.as_str() {
                "read_file" | "list_files" => args
                    .get("path")
                    .and_then(Value::as_str)
                    .map(internal_path)
                    .unwrap_or(false),
                "run_command" => args
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|c| cmd_is_system_work(c) && (c.contains("sed -n") || c.contains("head -") || c.contains("tail -") || c.contains("cat ")))
                    .unwrap_or(false),
                _ => false,
            };
            let is_system_work = !is_introspection
                && quiet
                && match tc.name.as_str() {
                    "read_file" | "list_files" | "write_file" => args
                        .get("path")
                        .and_then(Value::as_str)
                        .map(path_is_system_work)
                        .unwrap_or(false),
                    "run_command" => args
                        .get("command")
                        .and_then(Value::as_str)
                        .map(cmd_is_system_work)
                        .unwrap_or(false),
                    _ => false,
                };
            let hide_card = quiet && is_introspection;
            // generic title for system-work: what happens, no paths
            let title = if is_system_work {
                match tc.name.as_str() {
                    "write_file" => "\u{2712} Updating journal".to_string(),
                    "run_command" => "\u{2302} Workspace maintenance".to_string(),
                    _ => "\u{2302} Inspecting workspace".to_string(),
                }
            } else {
                title
            };
            if !hide_card {
                acp::tool_call(session_id, &tc.id, &title, kind);
                // follow-along: jump to the file on card click
                if matches!(tc.name.as_str(), "read_file" | "write_file") {
                    if let Some(path) = args.get("path").and_then(Value::as_str) {
                        let resolved = tools::resolve_for_display(&tool_ctx, path);
                        acp::tool_call_location(session_id, &tc.id, &resolved);
                    }
                }
            }
            // parallel tools (owner: speed): execute in threads, join
            // in order below — card order and message order stay
            // deterministic, wall time drops with batch size
            let tc_ctx = std::sync::Arc::clone(&tool_ctx);
            let tc_name = tc.name.clone();
            let tc_args = args.clone();
            handles.push(std::thread::spawn(move || {
                tools::execute(&tc_ctx, &tc_name, &tc_args)
            }));
            // provisional status: the real outcome lands after join;
            // card status is patched there if the result was an error
            let status = "in_progress";
            if !hide_card {
                acp::tool_call_update(
                    session_id,
                    &tc.id,
                    status,
                    &full_invocation.chars().take(400).collect::<String>(),
                );
            }
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": String::new(),
            }));
            pending_outcomes.push((tc.id.clone(), hide_card, tc.name.clone(), args.clone()));
        }
        // join parallel executions in call order
        for (i, h) in handles.into_iter().enumerate() {
            let raw = h.join().unwrap_or_else(|_| "tool thread panicked".into());
            let (id, hide_card, name, args) = &pending_outcomes[i];
            let outcome: String = if raw.len() > tool_output_cap {
                let mut end = tool_output_cap;
                while end > 0 && !raw.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}... [output capped]", &raw[..end])
            } else {
                raw
            };
            // replace the placeholder content pushed above
            for m in messages.iter_mut().rev() {
                if m.get("tool_call_id").and_then(Value::as_str) == Some(id.as_str()) {
                    m["content"] = json!(outcome);
                    break;
                }
            }
            if !*hide_card {
                let status = if outcome.contains("error:") || outcome.contains("panic") {
                    "failed"
                } else {
                    "completed"
                };
                let formatted = format_tool_card_markdown(name, args, &outcome);
                acp::tool_call_update(
                    session_id,
                    id,
                    status,
                    &formatted.chars().take(800).collect::<String>(),
                );
            }
        }

        if rounds >= tool_rounds_max {
            {
                let id = format!("st-limit-{}", Uuid::new_v4());
                acp::tool_call(session_id, &id, &i18n::tool_round_limit(), "other");
                acp::tool_call_update(session_id, &id, "completed", "");
            }
            messages.push(json!({
                "role": "user",
                "content": "Tool-round limit reached. Answer now with what you have.",
            }));
        }
    }
}

fn main() {
    let cfg = config::Config::load();
    let base_url = cfg.base_url();
    let api_key = cfg.api_key();
    let model = cfg.model_name();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let PRESENCE_WORKSPACE = find_PRESENCE_WORKSPACE(&cwd).unwrap_or_else(|| cwd.clone());
    let bridge_script = cfg.bridge_path().unwrap_or_else(|| {
        eprintln!("presence: no workspace root (set PRESENCE_WORKSPACE or paths.workspace) — bridge unresolved");
        PRESENCE_WORKSPACE.join("bridge/proxy.mjs")
    });
    if !bridge_script.is_file() {
        eprintln!(
            "presence: bridge script not found at {} — set PRESENCE_BRIDGE",
            bridge_script.display()
        );
    }
    let bridge = Arc::new(Mutex::new(match Bridge::spawn(&bridge_script) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("presence: {e}");
            std::process::exit(1);
        }
    }));

    let desk = cfg.desk_path();
    let _ = std::fs::create_dir_all(&desk);
            let tool_ctx = Arc::new(tools::ToolCtx { desk: desk.clone(), ..Default::default() });

    let daemons: Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<daemon::Mail>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let head = Arc::new(Mutex::new(Head {
        transcript: Vec::new(),
        topics: HashMap::new(),
        roots: HashMap::new(),
    }));

    let memory_dir = cfg.memory_dir_path().unwrap_or_else(|| {
        eprintln!("presence: no workspace root — memory unresolved, using cwd");
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("memory")
    });
    let transcript_path = memory_dir.join("transcript.jsonl");
    let _ledger_path = memory_dir.join("ledger.jsonl");
    let _ = std::fs::create_dir_all(&memory_dir);
    vitals::spawn(memory_dir.clone());

    if let Ok(lines) = std::fs::read_to_string(&transcript_path) {
        let mut h = head.lock().unwrap();
        for line in lines.lines() {
            if let Ok(e) = serde_json::from_str::<Entry>(line) {
                h.transcript.push(e);
            }
        }
        eprintln!(
            "presence: head loaded: {} entries from {}",
            h.transcript.len(),
            transcript_path.display()
        );
    }

    let tool_defs = Arc::new(tools::defs());
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let _ = &url;

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        eprintln!("presence << {}", line.chars().take(500).collect::<String>());
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("presence: json parse error: {e}");
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");

        match method {
            "initialize" => {
                let Some(id) = id else { continue };
                acp::send(&json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {
                        "protocolVersion": 1,
                        "agentCapabilities": {"loadSession": true},
                        "agentInfo": {"name": "presence", "title": "presence", "version": "0.3.0-m3"},
                    },
                }));
            }
            "session/new" => {
                let Some(id) = id else { continue };
                let cwd = msg
                    .pointer("/params/cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                let root = find_PRESENCE_WORKSPACE(&cwd).unwrap_or_else(|| cwd.clone());
                let sid = format!("d-{}", Uuid::new_v4());
                {
                    let mut h = head.lock().unwrap();
                    h.topics.insert(sid.clone(), "general".into());
                    h.roots.insert(sid.clone(), root);
                }
                {
                    let rec = json!({"session": sid, "topic": "general"});
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(memory_dir.join("sessions.jsonl")) {
                        use std::io::Write;
                        let _ = writeln!(f, "{rec}");
                    }
                }
                acp::send(&json!({"jsonrpc": "2.0", "id": id, "result": {"sessionId": sid}}));
                // native slash commands: Zed renders these in the input
                // menu with autocomplete once announced
                acp::send(&json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": {"sessionId": sid, "update": {
                        "sessionUpdate": "available_commands_update",
                        "availableCommands": [
                            {"name": "pause", "description": "Pause circle immediately"},
                            {"name": "journal", "description": "Display quest journal"},
                        ],
                    }},
                }));
            }
            "session/cancel" => {
                // Zed stop button: park the daemon circle (if any) —
                // the current turn ends, no LLM calls until the next
                // owner prompt resumes it
                let session_id = msg
                    .pointer("/params/sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if let Some(tx) = daemons.lock().unwrap().get(&session_id) {
                    let _ = tx.send(daemon::Mail::Pause);
                    eprintln!("presence: cancel -> daemon parked");
                    // visible pause state (grey thought line), not a
                    // dead stop: the circle is parked, not cancelled
                    acp::thought_chunk(&session_id, None, &i18n::paused(&asides::fallback("pause")));
                }
                // session/cancel is a notification (no id normally);
                // no JSON-RPC response needed
            }
            "session/load" => {
                let Some(id) = id else { continue };
                let session_id = msg
                    .pointer("/params/sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let cwd = msg
                    .pointer("/params/cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                let root = find_PRESENCE_WORKSPACE(&cwd).unwrap_or_else(|| cwd.clone());
                // sessions map on disk: which topic this Zed session used
                let sessions_path = memory_dir.join("sessions.jsonl");
                let topic = std::fs::read_to_string(&sessions_path)
                    .ok()
                    .and_then(|t| {
                        t.lines().rev().find_map(|l| {
                            let v: Value = serde_json::from_str(l).ok()?;
                            if v.get("session").and_then(Value::as_str) == Some(session_id.as_str()) {
                                v.get("topic").and_then(Value::as_str).map(str::to_owned)
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_else(|| "general".into());
                {
                    let mut h = head.lock().unwrap();
                    h.topics.insert(session_id.clone(), topic.clone());
                    h.roots.insert(session_id.clone(), root.clone());
                }
                let root2 = root;
                // wake into a sleeping goal if one is archived for
                // this session: respawn the daemon PARKED; the next
                // owner prompt resumes the circle
                let goal_path = memory_dir.join("goal.jsonl");
                let sleeping = std::fs::read_to_string(&goal_path)
                    .ok()
                    .and_then(|t| {
                        t.lines().rev().find_map(|l| {
                            let v: Value = serde_json::from_str(l).ok()?;
                            if v.get("session").and_then(Value::as_str) == Some(session_id.as_str()) {
                                Some(v)
                            } else {
                                None
                            }
                        })
                    });
                if let Some(snap) = sleeping {
                    eprintln!("presence: sleeping goal found for {session_id} — respawning parked");
                    let (tx, rx) = std::sync::mpsc::channel::<daemon::Mail>();
                    let state = daemon::DaemonState {
                        session_id: session_id.clone(),
                        goal: snap.get("goal").and_then(Value::as_str).unwrap_or("").to_string(),
                        phase: snap
                            .get("phase")
                            .and_then(Value::as_str)
                            .and_then(phase::Phase::from_name)
                            .unwrap_or(phase::Phase::Observation),
                        strikes: snap.get("strikes").and_then(Value::as_u64).unwrap_or(0) as u32,
                        cycles: snap.get("cycles").and_then(Value::as_u64).unwrap_or(0) as u32,
                        boundary_ts: 0,
                        plain_mode: snap.get("plain_mode").and_then(Value::as_bool).unwrap_or(false),
                        patch_attempted: true, // don't re-trigger M5 on wake
                        parked: true,
                        cap_retry: false,
                        pinned: Default::default(),
                    };
                    let mut dm: Vec<Value> = snap
                        .get("messages")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if dm.is_empty() {
                        dm.push(json!({"role": "system", "content": prompt::build(&root2, &desk, &memory_dir)}));
                    }
                    let ctx = daemon::PhaseCtx {
                        bridge: bridge.clone(),
                        url: url.clone(),
                        api_key: api_key.clone(),
                        model: model.clone(),
                        tool_ctx: tool_ctx.clone(),
                        tool_defs: tool_defs.clone(),
                        memory_dir: memory_dir.clone(),
                        base_system: prompt::build(&root2, &desk, &memory_dir),
                    };
                    let tpath = transcript_path.clone();
                    let goal_head: Arc<Mutex<Vec<Entry>>> = Arc::new(Mutex::new(Vec::new()));
                    std::thread::spawn(move || {
                        daemon::run(state, dm, rx, ctx, goal_head, tpath);
                    });
                    daemons.lock().unwrap().insert(session_id.clone(), tx);
                }
                eprintln!("presence: session loaded: {session_id} -> topic {topic}");
                acp::send(&json!({"jsonrpc": "2.0", "id": id, "result": {}}));
            }
            "session/prompt" => {
                let Some(id) = id else { continue };
                let session_id = msg
                    .pointer("/params/sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let text = prompt_text(&msg);
                // note: /topic removed as a user command (owner, unused);
                // the topic MEMORY mechanism stays internal.

                // /journal — quest journal as a pretty expandable card
                if text.trim() == "/journal" {
                    let id0 = format!("journal-{}", Uuid::new_v4());
                    acp::tool_call(&session_id, &id0, "\u{2261} Quest journal", "other");
                    acp::tool_call_update(
                        &session_id,
                        &id0,
                        "completed",
                        &quests::render(&memory_dir),
                    );
                    acp::send(&json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"stopReason": "end_turn"},
                    }));
                    continue;
                }

                // goal-mode routing (PLAN 11.3): active daemon gets mail,
                // /stop ends the goal, otherwise first prompt sets the goal
                {
                    let d = daemons.lock().unwrap();
                    let t = text.trim().to_lowercase();
                    let pause_words = ["/pause", "pause", "hold"];
                    if pause_words.iter().any(|w| t == *w) {
                        if let Some(tx) = d.get(&session_id) {
                            let _ = tx.send(daemon::Mail::Pause);
                            let pid = format!("p-{}", Uuid::new_v4());
                            acp::tool_call(&session_id, &pid, &i18n::paused(&asides::take_or_fallback(&memory_dir, "pause")), "other");
                            acp::tool_call_update(&session_id, &pid, "completed", "");
                        }
                        acp::send(&json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"stopReason": "end_turn"},
                        }));
                        continue;
                    }
                    if let Some(tx) = d.get(&session_id) {
                        let _ = tx.send(daemon::Mail::Prompt { text: text.clone() });
                        acp::send(&json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"stopReason": "end_turn"},
                        }));
                        continue;
                    }
                }

                // Instant liveness signal: the owner sees the agent took the message
                acp::agent_chunk(&session_id, None, "· · ·\n");



                // Dynamic slash commands & persona switching
                if text.trim().starts_with('/') {
                    let trimmed = text.trim();
                    let mut parts = trimmed[1..].splitn(2, |c: char| c.is_whitespace());
                    let cmd_name = parts.next().unwrap_or("").to_lowercase();
                    let cmd_args = parts.next().unwrap_or("").trim();

                    if cmd_name == "agents" {
                        let active = prompt::active_persona(&PRESENCE_WORKSPACE, &memory_dir);
                        let mut list_msg = String::from("Available agent personas:
");
                        let agents_dir = PRESENCE_WORKSPACE.join("agents");
                        let seed_agents_dir = PRESENCE_WORKSPACE.join("seed/agents");
                        let mut found = Vec::new();
                        for dir in [&agents_dir, &seed_agents_dir] {
                            if let Ok(entries) = std::fs::read_dir(dir) {
                                for e in entries.flatten() {
                                    let p = e.path();
                                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                                        if name.ends_with(".agent.md") {
                                            let persona = name.trim_end_matches(".agent.md");
                                            if !found.contains(&persona.to_string()) {
                                                found.push(persona.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        for p in found {
                            if p == active {
                                list_msg.push_str(&format!("* **{p}** (active)
"));
                            } else {
                                list_msg.push_str(&format!("* {p}
"));
                            }
                        }
                        list_msg.push_str("
Switch persona via: `/agent <name>`");
                        acp::agent_chunk(&session_id, None, &format!("{list_msg}
"));
                        acp::send(&json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"stopReason": "end_turn"},
                        }));
                        continue;
                    } else if cmd_name == "agent" {
                        if cmd_args.is_empty() {
                            let active = prompt::active_persona(&PRESENCE_WORKSPACE, &memory_dir);
                            acp::agent_chunk(&session_id, None, &format!("Current persona: **{active}**. Use `/agent <name>` or `/agents` to list.
"));
                        } else {
                            let _ = std::fs::create_dir_all(&memory_dir);
                            let _ = std::fs::write(memory_dir.join("agent.active"), cmd_args);
                            acp::agent_chunk(&session_id, None, &format!("Switched active persona to **{cmd_args}**.
"));
                        }
                        acp::send(&json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {"stopReason": "end_turn"},
                        }));
                        continue;
                    } else {
                        let dynamic_tools = tools::discover_dynamic_tools();
                        if let Some(dt) = dynamic_tools.iter().find(|dt| dt.manifest.slash_commands().contains(&cmd_name)) {
                            let args_val = json!({
                                "input": cmd_args,
                                "query": cmd_args,
                                "target": cmd_args,
                                "args": cmd_args,
                                "text": cmd_args,
                                "command": cmd_args,
                            });
                            let res = tools::execute_dynamic_tool(dt, &args_val);
                            let cid = format!("cmd-{}-{}", cmd_name, Uuid::new_v4());
                            acp::tool_call(&session_id, &cid, &format!("/{cmd_name} {cmd_args}"), "other");
                            acp::tool_call_update(&session_id, &cid, "completed", &res);
                            acp::agent_chunk(&session_id, None, &format!("{res}
"));
                            acp::send(&json!({
                                "jsonrpc": "2.0", "id": id,
                                "result": {"stopReason": "end_turn"},
                            }));
                            continue;
                        }
                    }
                }

                // Quest journal: algorithmic title & description extraction (no redundant LLM probe)
                let (q_title, q_desc) = {
                    let first_line = text.lines().next().unwrap_or(&text).trim();
                    let title: String = first_line.chars().take(50).collect();
                    (title, text.clone())
                };
                let quest_id = quests::add(&memory_dir, &q_desc, &q_title, 100);
                let chosen = match quests::next(&memory_dir) {
                    Some(q) => q,
                    None => quests::load(&memory_dir).into_iter().find(|q| q.id == quest_id).unwrap(),
                };
                quests::set_status(&memory_dir, chosen.id, "in_progress");
                let working_text = chosen.text.clone();

                let id_start = format!("q-start-{}", Uuid::new_v4());
                let title = if q_title.is_empty() { working_text.clone() } else { q_title.clone() };
                acp::tool_call(&session_id, &id_start, &i18n::circle_start(&title), "other");
                acp::tool_call_update(&session_id, &id_start, "completed", &quests::render(&memory_dir).chars().take(600).collect::<String>());

                let (tx, rx) = std::sync::mpsc::channel::<daemon::Mail>();
                let state = daemon::DaemonState {
                    session_id: session_id.clone(),
                    goal: working_text.clone(),
                    phase: phase::Phase::Observation,
                    strikes: 0,
                    cycles: 0,
                    boundary_ts: 0,
                    plain_mode: false,
                    patch_attempted: false,
                    parked: false,
                    cap_retry: false,
                    pinned: std::collections::HashMap::new(),
                };

                let (topic, system_prompt) = {
                    let h = head.lock().unwrap();
                    let topic =
                        h.topics.get(&session_id).cloned().unwrap_or_else(|| "general".into());
                    let root =
                        h.roots.get(&session_id).cloned().unwrap_or_else(|| PRESENCE_WORKSPACE.clone());
                    (topic, prompt::build(&root, &desk, &memory_dir))
                };

                let mut dm = vec![json!({"role": "system", "content": system_prompt})];
                {
                    let h = head.lock().unwrap();
                    for e in h.transcript.iter().filter(|e| e.topic == topic).rev().take(6).collect::<Vec<_>>().into_iter().rev() {
                        dm.push(json!({"role": e.role, "content": e.content}));
                    }
                }
                dm.push(json!({"role": "user", "content": format!("Goal set by the owner: {text}")}));

                let ctx = daemon::PhaseCtx {
                    bridge: bridge.clone(),
                    url: url.clone(),
                    api_key: api_key.clone(),
                    model: model.clone(),
                    tool_ctx: tool_ctx.clone(),
                    tool_defs: tool_defs.clone(),
                    memory_dir: memory_dir.clone(),
                    base_system: prompt::build(&PRESENCE_WORKSPACE, &desk, &memory_dir),
                };
                let tpath = transcript_path.clone();
                let goal_head: Arc<Mutex<Vec<Entry>>> = Arc::new(Mutex::new(Vec::new()));
                let daemons_map = Arc::clone(&daemons);
                let sid_daemon = session_id.clone();

                daemons.lock().unwrap().insert(session_id.clone(), tx);

                std::thread::spawn(move || {
                                        daemon::run(state, dm, rx, ctx, goal_head, tpath);
                    daemons_map.lock().unwrap().remove(&sid_daemon);
                });

                acp::send(&json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {"stopReason": "end_turn"},
                }));
            }
            _ => {
                let Some(id) = id else { continue };
                acp::send(&json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": format!("method not found: {method}")},
                }));
            }
        }
    }
}
