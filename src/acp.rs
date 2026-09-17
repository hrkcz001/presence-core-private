//! ACP wire helpers: JSON-RPC send, session updates, message chunks,
//! tool_call lifecycle emissions.

use serde_json::{json, Value};
use std::io::{self, Write};

pub fn send(v: &Value) {
    let mut h = io::stdout().lock();
    let _ = serde_json::to_writer(&mut h, v);
    let _ = h.write_all(b"\n");
    let _ = h.flush();
}

pub fn session_update(session_id: &str, update: Value) {
    send(&json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"sessionId": session_id, "update": update},
    }));
}

pub fn agent_chunk(session_id: &str, message_id: Option<&str>, text: &str) {
    let mut update = json!({
        "sessionUpdate": "agent_message_chunk",
        "content": {"type": "text", "text": text},
    });
    if let Some(id) = message_id {
        update["messageId"] = json!(id);
    }
    session_update(session_id, update);
}


/// Status strings.
pub fn status(key: &str) -> String {
    match key {
        "circle_start" => format!("{} Circle started", '\u{25D0}'),
        "goal_done" => format!("{} Goal completed", '\u{2713}'),
        "goal_stopped" => format!("{} Stopped by owner", '\u{2713}'),
        "phase_prefix" => "Phase".into(),
        _ => key.to_string(),
    }
}

/// Stream a reasoning fragment — renders as the grey "thinking" UI in
/// Zed. Not part of the message text; pure liveness/phase signal.
pub fn thought_chunk(session_id: &str, message_id: Option<&str>, text: &str) {
    let mut update = json!({
        "sessionUpdate": "agent_thought_chunk",
        "content": {"type": "text", "text": text},
    });
    if let Some(id) = message_id {
        update["messageId"] = json!(id);
    }
    session_update(session_id, update);
}

/// Emit the start of a tool call (renders as in_progress in the client).
pub fn tool_call(session_id: &str, tool_call_id: &str, title: &str, kind: &str) {
    session_update(session_id, json!({
        "sessionUpdate": "tool_call",
        "toolCallId": tool_call_id,
        "title": title,
        "kind": kind,
        "status": "in_progress",
        "content": [],
    }));
}

/// Patch a tool call: final status + a text content item.
pub fn tool_call_update(session_id: &str, tool_call_id: &str, status: &str, text: &str) {
    session_update(session_id, json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": tool_call_id,
        "status": status,
        "content": [{"type": "content", "content": {"type": "text", "text": text}}],
    }));
}

/// Native plan update: Zed renders a live checklist from these.
/// entries: (content, status) where status is pending/in_progress/completed.
pub fn plan(session_id: &str, entries: &[(String, &str)]) {
    session_update(session_id, json!({
        "sessionUpdate": "plan",
        "entries": entries.iter().map(|(c, st)| json!({
            "content": c,
            "status": st,
        })).collect::<Vec<_>>(),
    }));
}

/// Error as a red card: short cause in the title, raw dump in the
/// expandable body.
pub fn error_card(session_id: &str, e: &str) {
    let id = format!("err-{}", uuid::Uuid::new_v4());
    tool_call(session_id, &id, &crate::i18n::error_title(e), "other");
    tool_call_update(session_id, &id, "failed", e);
}

/// Attach a file location to a tool call (follow-along in Zed: clicking
/// the card opens the file).
pub fn tool_call_location(session_id: &str, tool_call_id: &str, path: &str) {
    send(&json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"sessionId": session_id, "update": {
            "sessionUpdate": "tool_call_update",
            "toolCallId": tool_call_id,
            "locations": [{"path": path}],
        }},
    }));
}
