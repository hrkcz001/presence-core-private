//! Presence event bus and decoupled client emissions.
//! Core engine emits events without knowing whether the client
//! is Zed (ACP), Neovim, CLI stdout, or a websocket.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
pub enum EventPayload {
    MessageChunk {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    ThoughtChunk {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    ToolCallStart {
        id: String,
        title: String,
        kind: String,
    },
    ToolCallUpdate {
        id: String,
        status: String,
        text: String,
    },
    ToolCallLocation {
        id: String,
        path: String,
    },
    PlanUpdate {
        entries: Vec<(String, String)>,
    },
    ErrorCard {
        id: String,
        title: String,
        detail: String,
    },
    TurnFinished {
        stop_reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresenceEvent {
    pub session_id: String,
    #[serde(flatten)]
    pub payload: EventPayload,
}

impl PresenceEvent {
    pub fn message_chunk(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::MessageChunk {
                text: text.into(),
                message_id: None,
            },
        }
    }

    pub fn thought_chunk(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::ThoughtChunk {
                text: text.into(),
                message_id: None,
            },
        }
    }

    pub fn tool_start(session_id: impl Into<String>, id: impl Into<String>, title: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::ToolCallStart {
                id: id.into(),
                title: title.into(),
                kind: kind.into(),
            },
        }
    }

    pub fn tool_update(session_id: impl Into<String>, id: impl Into<String>, status: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::ToolCallUpdate {
                id: id.into(),
                status: status.into(),
                text: text.into(),
            },
        }
    }

    pub fn tool_location(session_id: impl Into<String>, id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::ToolCallLocation {
                id: id.into(),
                path: path.into(),
            },
        }
    }

    pub fn plan(session_id: impl Into<String>, entries: Vec<(String, String)>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::PlanUpdate { entries },
        }
    }

    pub fn error(session_id: impl Into<String>, id: impl Into<String>, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::ErrorCard {
                id: id.into(),
                title: title.into(),
                detail: detail.into(),
            },
        }
    }

    pub fn turn_finished(session_id: impl Into<String>, stop_reason: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            payload: EventPayload::TurnFinished {
                stop_reason: stop_reason.into(),
            },
        }
    }
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: PresenceEvent);
}

/// A sink that collects events into an in-memory vector (useful for testing and IPC).
#[derive(Default)]
pub struct MemorySink {
    events: Mutex<Vec<PresenceEvent>>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_events(&self) -> Vec<PresenceEvent> {
        let mut guard = self.events.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn events(&self) -> Vec<PresenceEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl EventSink for MemorySink {
    fn emit(&self, event: PresenceEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// Standard ACP / Zed event sink: transforms PresenceEvent into Zed's JSON-RPC wire protocol.
pub struct AcpSink;

impl EventSink for AcpSink {
    fn emit(&self, event: PresenceEvent) {
        match event.payload {
            EventPayload::MessageChunk { text, message_id } => {
                crate::acp::agent_chunk(&event.session_id, message_id.as_deref(), &text);
            }
            EventPayload::ThoughtChunk { text, message_id } => {
                crate::acp::thought_chunk(&event.session_id, message_id.as_deref(), &text);
            }
            EventPayload::ToolCallStart { id, title, kind } => {
                crate::acp::tool_call(&event.session_id, &id, &title, &kind);
            }
            EventPayload::ToolCallUpdate { id, status, text } => {
                crate::acp::tool_call_update(&event.session_id, &id, &status, &text);
            }
            EventPayload::ToolCallLocation { id, path } => {
                crate::acp::tool_call_location(&event.session_id, &id, &path);
            }
            EventPayload::PlanUpdate { entries } => {
                let refs: Vec<(String, &str)> = entries.iter().map(|(c, st)| (c.clone(), st.as_str())).collect();
                crate::acp::plan(&event.session_id, &refs);
            }
            EventPayload::ErrorCard { detail, .. } => {
                crate::acp::error_card(&event.session_id, &detail);
            }
            EventPayload::TurnFinished { stop_reason } => {
                crate::acp::send(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": {
                        "sessionId": event.session_id,
                        "update": {
                            "sessionUpdate": "turn_finished",
                            "stopReason": stop_reason,
                        }
                    }
                }));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_sink_emission() {
        let sink = MemorySink::new();
        sink.emit(PresenceEvent::thought_chunk("s1", "reasoning"));
        sink.emit(PresenceEvent::tool_start("s1", "tc1", "read_file", "other"));
        sink.emit(PresenceEvent::tool_update("s1", "tc1", "completed", "file contents"));
        sink.emit(PresenceEvent::turn_finished("s1", "end_turn"));

        let events = sink.events();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].session_id, "s1");
        match &events[1].payload {
            EventPayload::ToolCallStart { id, title, .. } => {
                assert_eq!(id, "tc1");
                assert_eq!(title, "read_file");
            }
            other => panic!("expected ToolCallStart, got: {:?}", other),
        }
    }
}