//! Network leg: dumb pipe to the TLS bridge child process (PLAN §6, §11.6).
//! The bridge is a tiny Node process (proxy.mjs) that speaks the
//! agentrouter-compatible TLS fingerprint. Rust owns the session,
//! the memory, the tools; Node only owns the bytes on the wire.

use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use serde_json::{json, Value};
use crate::acp;

pub struct Bridge {
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Value>,
}

impl Bridge {
    pub fn spawn(bridge_script: &std::path::Path) -> std::io::Result<Self> {
        let mut child = Command::new("node")
            .arg(bridge_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().expect("bridge stdin");
        let stdout = BufReader::new(child.stdout.take().expect("bridge stdout"));
        Ok(Self { child, stdin, stdout })
    }

    /// POST + stream SSE; returns final content + tool calls + usage.
    /// Reasoning goes to stderr only (step 16: private thoughts).
    pub fn chat(
        &mut self,
        url: &str,
        api_key: &str,
        model: &str,
        messages: &[Value],
        tools: Option<&Vec<Value>>,
        session_id: &str,
        message_id: &str,
    ) -> Result<ChatResult, String> {
        self.chat_mode(url, api_key, model, messages, tools, session_id, message_id, false, None)
    }

    /// quiet = internal phase (1-3): content is collected but NOT
    /// streamed to the user chat (owner: no text wall from phases;
    /// only the phase indicator + Vollzug speak).
    /// Maximum silence between bridge lines before the call is
    /// considered hung (hung must never mean died).
    const STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(150);

    pub fn chat_mode(
        &mut self,
        url: &str,
        api_key: &str,
        model: &str,
        messages: &[Value],
        tools: Option<&Vec<Value>>,
        session_id: &str,
        message_id: &str,
        quiet: bool,
        max_tokens: Option<u64>,
    ) -> Result<ChatResult, String> {
        let cfg = crate::config::Config::cached();
        let caps = cfg
            .effective_models()
            .into_iter()
            .find(|m| m.name == model || model.starts_with(&m.name))
            .map(|m| m.capabilities)
            .unwrap_or_default();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "stream_options": {"include_usage": true},
        });
        if let Some(t) = tools {
            body["tools"] = json!(t);
            if caps.parallel_tool_calls {
                body["parallel_tool_calls"] = json!(true);
            }
        }
        if caps.prompt_cache_key {
            body["prompt_cache_key"] = json!(session_id);
        }
        if let Some(mt) = max_tokens {
            body["max_tokens"] = json!(mt);
            // reasoning budget (owner: precise short thinking, not walls):
            // glm-5.3 always thinks; budget_tokens caps HOW MUCH.
            // DeepSeek OpenAI-compatible schema rejects "thinking" object.
            if model.starts_with("glm-5") || std::env::var("PRESENCE_THINKING_BUDGET").or_else(|_| std::env::var("PRESENCE_THINKING_BUDGET")).is_ok() {
                let budget = std::env::var("PRESENCE_THINKING_BUDGET").or_else(|_| std::env::var("PRESENCE_THINKING_BUDGET"))
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or_else(|| {
                        if mt <= 800 {
                            256  // schema phases: brief, structured
                        } else if mt <= 2000 {
                            384  // chat: light reasoning
                        } else {
                            512  // reprompts and retries: room to fix itself
                        }
                    });
                body["thinking"] = json!({"type": "enabled", "budget_tokens": budget});
            }
        }
        let mut headers = json!({
            "Authorization": format!("Bearer {api_key}"),
            "Content-Type": "application/json",
        });
        if caps.prompt_cache_key {
            headers["X-Prompt-Cache-Key"] = json!(session_id);
        }
        let req = json!({
            "id": message_id,
            "url": url,
            "headers": headers,
            "body": body.to_string(),
        });
        let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        line.push(10 as char); // newline for bridge readline
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("bridge write failed: {e}"))?;

        let mut buf = String::new();
        let mut out_buffer = String::new();
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut usage = None;

        loop {
            let mut envelope = String::new();
            // stream watchdog: a line must arrive within STREAM_TIMEOUT,
            // else the call is hung (LLM stall) — cut it, don't hang forever
            let n = match wait_line_with_timeout(&mut self.stdout, &mut envelope, Self::STREAM_TIMEOUT) {
                Ok(n) => n,
                Err(_) => {
                    return Err("STREAM_HUNG: no data from the bridge for 150s".into());
                }
            };
            if n == 0 {
                return Err("bridge closed unexpectedly".into());
            }
            let Ok(ev) = serde_json::from_str::<Value>(&envelope) else { continue };
            match ev.get("type").and_then(Value::as_str) {
                Some("chunk") => {
                    let data = ev.get("data").and_then(Value::as_str).unwrap_or("");
                    buf.push_str(data);
                    let mut lines: Vec<String> = buf.split('\n').map(str::to_owned).collect();
                    buf = lines.pop().unwrap_or_default();
                    for l in &lines {
                        let l = l.trim();
                        if !l.starts_with("data:") {
                            continue;
                        }
                        let d = l[5..].trim();
                        if d == "[DONE]" {
                            continue;
                        }
                        let Ok(payload) = serde_json::from_str::<Value>(d) else { continue };
                        if let Some(u) = payload.pointer("/usage").filter(|u| !u.is_null()) {
                            usage = Some(u.clone());
                        }
                        let Some(delta) = payload
                            .pointer("/choices/0/delta")
                            .and_then(|x| x.as_object().cloned())
                        else {
                            continue;
                        };
                        if let Some(r) = delta.get("reasoning_content").and_then(Value::as_str) {
                            // thoughts stay private (stderr) — the owner wants
                            // the PHASE indicator, not the thought wall
                            eprintln!("presence [thought] {r}");
                        }
                        if let Some(c) = delta.get("content").and_then(Value::as_str) {
                            if quiet {
                                eprintln!("presence [phase-internal] {c}");
                            } else {
                                // goal_done lines are machine-only; chunks can
                                // split mid-marker, so emit line-wise via buffer
                                out_buffer.push_str(c);
                                if let Some(nl) = out_buffer.rfind('\n') {
                                    let (emit, rest) = out_buffer.split_at(nl + 1);
                                    let shown: String = emit
                                        .lines()
                                        .filter(|l| !l.trim().starts_with("goal_done:"))
                                        .map(|l| format!("{l}\n"))
                                        .collect();
                                    if !shown.is_empty() {
                                        acp::agent_chunk(session_id, Some(message_id), &shown);
                                    }
                                    out_buffer = rest.to_string();
                                }
                            }
                            content.push_str(c);
                        }
                        if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
                            for tc in tcs {
                                let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                                if tool_calls.len() <= idx {
                                    tool_calls.resize(idx + 1, ToolCall::default());
                                }
                                let slot = &mut tool_calls[idx];
                                if let Some(i) = tc.pointer("/id").and_then(Value::as_str) {
                                    slot.id = i.to_string();
                                }
                                if let Some(n) =
                                    tc.pointer("/function/name").and_then(Value::as_str)
                                {
                                    slot.name.push_str(n);
                                }
                                if let Some(a) =
                                    tc.pointer("/function/arguments").and_then(Value::as_str)
                                {
                                    slot.arguments.push_str(a);
                                }
                            }
                        }
                    }
                }
                Some("end") => break,
                Some("error") => {
                    return Err(
                        ev.get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown bridge error")
                            .to_string(),
                    );
                }
                _ => {} // "status": ok unless followed by error
            }
        }
        if !out_buffer.is_empty() {
            let shown: String = out_buffer
                .lines()
                .filter(|l| !l.trim().starts_with("goal_done:"))
                .map(|l| format!("{l}\n"))
                .collect();
            if !shown.is_empty() {
                acp::agent_chunk(session_id, Some(message_id), &shown);
            }
        }
        if content.is_empty() && tool_calls.is_empty() {
            // reasoning consumed the whole generation window: this is a
            // CAP problem, not a model failure — signal it so the caller
            // can retry with more rope instead of erroring to the owner
            return Err("CAP_EMPTY: reasoning used the entire budget".to_string());
        }
        Ok(ChatResult { content, tool_calls, usage })
    }
}

/// Blocking read_line with a hard silence timeout (watchdog).
fn wait_line_with_timeout(
    r: &mut impl std::io::BufRead,
    buf: &mut String,
    timeout: std::time::Duration,
) -> std::io::Result<usize> {
    use std::time::Instant;
    let deadline = Instant::now() + timeout;
    loop {
        let consumed = {
            let available = r.fill_buf()?;
            if available.is_empty() {
                return Ok(0); // EOF
            }
            if let Some(pos) = available.iter().position(|&b| b == b'\n') {
                let take = pos + 1;
                buf.push_str(&String::from_utf8_lossy(&available[..take]));
                take
            } else {
                buf.push_str(&String::from_utf8_lossy(available));
                available.len()
            }
        };
        r.consume(consumed);
        if buf.ends_with('\n') {
            return Ok(buf.len());
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "stream timeout",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
