//! S1 spike (PLAN.md §9): does Zed render `session/update` notifications
//! sent with no pending `session/prompt` request? Decides goal-mode
//! architecture B (daemon) vs C (checkpoint cadence).
//!
//! VERDICT (12.09.2026, owner-confirmed): B is GO — out-of-turn updates
//! render in Zed; messages sent during ticks are delivered immediately.
//!
//! Every prompt gets a normal echo turn. The first prompt also starts a
//! daemon thread ticking `S1_TICKS` agent_message_chunk updates every
//! `S1_INTERVAL_SECS` seconds (defaults 10 / 3). All incoming JSON-RPC
//! lines are echoed to stderr for diagnostics (Zed's agent-server log).

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

fn send(v: &Value) {
    let mut h = io::stdout().lock();
    let _ = serde_json::to_writer(&mut h, v);
    let _ = h.write_all(b"\n");
    let _ = h.flush();
}

fn agent_chunk(session_id: &str, message_id: Option<&str>, text: String) -> Value {
    let mut update = json!({
        "sessionUpdate": "agent_message_chunk",
        "content": {"type": "text", "text": text},
    });
    if let Some(id) = message_id {
        update["messageId"] = json!(id);
    }
    json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"sessionId": session_id, "update": update},
    })
}

/// Echo the prompt as one agent chunk; returns (sessionId, turn result).
fn handle_prompt(msg: &Value) -> (String, Value) {
    let session_id = msg
        .pointer("/params/sessionId")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let text: String = msg
        .pointer("/params/prompt")
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
        .unwrap_or_default();

    send(&agent_chunk(
        &session_id,
        None,
        format!("[s1 echo] {text}\n"),
    ));
    (session_id, json!({"stopReason": "end_turn"}))
}

fn main() {
    let stdin = io::stdin();
    let daemon_started = Arc::new(AtomicBool::new(false));
    let interval = Duration::from_secs(
        std::env::var("S1_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3),
    );
    let ticks: u64 = std::env::var("S1_TICKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        eprintln!("s1 << {line}");
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("s1: json parse error: {e}");
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");

        let result = match method {
            "initialize" => json!({
                "protocolVersion": 1,
                "agentCapabilities": {},
                "agentInfo": {"name": "presence-s1", "version": "0.1.0-s1"},
            }),
            "session/new" => json!({"sessionId": "s1-1"}),
            "session/prompt" => {
                let Some(id) = id else { continue };
                let (session_id, result) = handle_prompt(&msg);
                send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
                if !daemon_started.swap(true, Ordering::SeqCst) {
                    let sid = session_id.clone();
                    thread::spawn(move || {
                        let message_id = "s1-daemon";
                        for i in 1..=ticks {
                            thread::sleep(interval);
                            send(&agent_chunk(
                                &sid,
                                Some(message_id),
                                format!("[s1 daemon tick {i}/{ticks}] (out-of-turn)\n"),
                            ));
                        }
                        send(&agent_chunk(
                            &sid,
                            Some(message_id),
                            "[s1 daemon] done ticking\n".to_string(),
                        ));
                    });
                }
                continue;
            }
            _ => {
                let Some(id) = id else { continue };
                send(
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32601, "message": format!("method not found: {method}")},
                    }),
                );
                continue;
            }
        };

        if let Some(id) = id {
            send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
        }
    }
}
