//! Context budgeter (PLAN §3, §11 appetite). Exact accounting from API
//! usage; heuristics only for pre-flight assembly estimates. The
//! budgeter composes the prompt, not the model: immutable constitution
//! first, pinned excerpts, verbatim tail, digested middle, overflow
//! drops oldest digests.

use serde_json::Value;
use std::path::Path;

/// Conservative context limits per model (from agentrouter cards).
pub fn context_limit(model: &str) -> u64 {
    if let Ok(v) = std::env::var("PRESENCE_CTX_LIMIT").or_else(|_| std::env::var("PRESENCE_CTX_LIMIT")) {
        if let Ok(n) = v.parse::<u64>() {
            return n;
        }
    }
    let c = crate::config::Config::cached();
    if c.limits.ctx_limit > 0 {
        return c.limits.ctx_limit;
    }
    for m in c.effective_models() {
        if m.name == model || model.starts_with(&m.name) {
            return m.context_limit;
        }
    }
    if model.contains("deepseek") {
        1_048_576
    } else if model.starts_with("glm-5") {
        128_000
    } else if model.contains("astra") {
        200_000
    } else {
        32_000
    }
}

/// Rough token estimate for pre-flight assembly (chars/4 heuristic).
pub fn estimate(s: &str) -> u64 {
    (s.chars().count() as u64) / 4 + 1
}

/// One entry in the assembled context, with its budget priority.
#[derive(Debug, Clone)]
pub struct Piece {
    pub kind: PieceKind,
    pub text: String,
    /// Higher = dropped last. 0 = never dropped (constitution).
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    Constitution,
    State,
    Pinned,
    Tail,
    Digest,
}

/// Result of an assembly pass.
pub struct Assembled {
    pub text: String,
    pub est_tokens: u64,
    pub dropped_digests: usize,
}

/// Assemble within `budget` tokens: keep constitution, STATE, pinned,
/// and as much tail as fits; drop oldest digests on overflow.
pub fn assemble(pieces: &[Piece], budget: u64) -> Assembled {
    let mut kept: Vec<&Piece> = pieces.iter().collect();
    let mut dropped = 0usize;
    loop {
        let text = kept.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n\n");
        let est = estimate(&text) + 100; // framing overhead
        if est <= budget || kept.len() <= 2 {
            return Assembled { text, est_tokens: est, dropped_digests: dropped };
        }
        // drop the lowest-priority, oldest digest first
        let victim = kept
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == PieceKind::Digest)
            .map(|(i, p)| (p.priority, i))
            .min()
            .map(|(_, i)| i);
        match victim {
            Some(i) => {
                kept.remove(i);
                dropped += 1;
            }
            None => {
                // no digests left: drop the oldest tail entry
                let tail = kept.iter().rposition(|p| p.kind == PieceKind::Tail);
                match tail {
                    Some(i) if kept.len() > 2 => {
                        kept.remove(i);
                        dropped += 1;
                    }
                    _ => return Assembled { text, est_tokens: est, dropped_digests: dropped },
                }
            }
        }
    }
}

/// Compaction prompt (same model, owner decision §3): summarize the
/// middle into a digest preserving decisions, open loops, file paths,
/// phase history.
pub fn compact_prompt(entries: &[Value]) -> String {
    let body = entries
        .iter()
        .map(|e| {
            let role = e.get("role").and_then(Value::as_str).unwrap_or("?");
            let content = e.get("content").and_then(Value::as_str).unwrap_or("");
            format!("{role}: {content}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Summarize the following conversation into a compact digest. \
         Preserve: decisions made, open loops, file paths mentioned, \
         phase history. Plain text, no prose around it.\n\n{body}"
    )
}

/// Status line injected into every user prompt (the model feels the
/// window): [ctx 47% | turn 8 | phase: planning]
pub fn status_line(est_tokens: u64, limit: u64, turn: u32, phase: &str) -> String {
    let pct = if limit == 0 { 0 } else { est_tokens * 100 / limit };
    format!("[ctx {pct}% | turn {turn} | phase: {phase}]")
}

/// Read the last usage from the ledger for exact accounting (most
/// recent line with prompt_tokens + completion_tokens).
pub fn last_exact_usage(ledger: &Path) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string(ledger).ok()?;
    text.lines()
        .rev()
        .find_map(|l| {
            let v = serde_json::from_str::<Value>(l).ok()?;
            let p = v.get("prompt_tokens").and_then(Value::as_u64)?;
            let c = v.get("completion_tokens").and_then(Value::as_u64)?;
            Some((p, c))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(kind: PieceKind, text: &str, priority: u8) -> Piece {
        Piece { kind, text: text.into(), priority }
    }

    #[test]
    fn assemble_fits_without_drops() {
        let pieces = vec![
            piece(PieceKind::Constitution, "constitution", 0),
            piece(PieceKind::State, "state", 1),
            piece(PieceKind::Tail, "recent turn", 2),
        ];
        let a = assemble(&pieces, 10_000);
        assert_eq!(a.dropped_digests, 0);
        assert!(a.text.contains("constitution"));
        assert!(a.text.contains("recent turn"));
    }

    #[test]
    fn overflow_drops_oldest_digest() {
        let big: String = "x".repeat(400);
        let pieces = vec![
            piece(PieceKind::Constitution, "constitution", 0),
            piece(PieceKind::State, "state", 1),
            piece(PieceKind::Digest, &format!("digest old {big}"), 5),
            piece(PieceKind::Digest, &format!("digest new {big}"), 6),
            piece(PieceKind::Tail, "tail", 2),
        ];
        let a = assemble(&pieces, 260);
        assert_eq!(a.dropped_digests, 1);
        assert!(!a.text.contains("digest old"));
        assert!(a.text.contains("digest new"));
    }

    #[test]
    fn constitution_never_dropped() {
        let big: String = "y".repeat(2000);
        let pieces = vec![
            piece(PieceKind::Constitution, &big, 0),
            piece(PieceKind::State, &big.clone(), 1),
            piece(PieceKind::Tail, &big, 2),
        ];
        let a = assemble(&pieces, 300);
        assert!(a.text.contains(&big));
    }

    #[test]
    fn status_line_format() {
        assert_eq!(status_line(600, 1000, 8, "planning"), "[ctx 60% | turn 8 | phase: planning]");
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn deepseek_context_limit_baseline() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("PRESENCE_CTX_LIMIT");
        assert_eq!(context_limit("deepseek-v4-flash"), 1_048_576);
    }

    #[test]
    fn env_override_wins() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("PRESENCE_CTX_LIMIT", "999");
        let limit = context_limit("glm-5.3");
        std::env::remove_var("PRESENCE_CTX_LIMIT");
        assert_eq!(limit, 999);
    }

    #[test]
    fn exact_usage_from_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ledger.jsonl");
        std::fs::write(&p, "{\"prompt_tokens\":10,\"completion_tokens\":5}\n{\"prompt_tokens\":20,\"completion_tokens\":7}\n").unwrap();
        assert_eq!(last_exact_usage(&p), Some((20, 7)));
    }
}
