//! Mood + friction ledger (PLAN §11.5, owner directive step 20).
//! friction.jsonl: runtime-owned, append-only, never wiped — one line
//! per LLM retry, failed tool outcome, forced final, phase-schema
//! strike. mood.md: rewritten at each phase boundary from mechanical
//! signals (friction counts since last boundary + cycle count) into
//! the STATE vocabulary. No model calls: mood is felt, not narrated.

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrictionKind {
    LlmRetry,
    ToolFailed,
    ForcedFinal,
    PhaseStrike,
    RitualSkip,
    RulePatch,
    Failover,
}

impl FrictionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrictionKind::LlmRetry => "llm_retry",
            FrictionKind::ToolFailed => "tool_failed",
            FrictionKind::ForcedFinal => "forced_final",
            FrictionKind::PhaseStrike => "phase_strike",
            FrictionKind::RitualSkip => "ritual_skip",
            FrictionKind::RulePatch => "rule_patch",
            FrictionKind::Failover => "failover",
        }
    }
}

/// Append one friction record. Detail is bounded to ~200 chars.
pub fn record(dir: &Path, session: &str, kind: FrictionKind, detail: &str) {
    let bounded: String = detail.chars().take(200).collect();
    let rec = serde_json::json!({
        "ts": now_ts(),
        "session": session,
        "kind": kind.as_str(),
        "detail": bounded,
    });
    let path = dir.join("friction.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{rec}");
    }
}

/// Count friction records newer than `since` (ts field). Broken lines
/// are skipped, not fatal.
fn count_since(dir: &Path, since: u64) -> usize {
    let path = dir.join("friction.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else { return 0 };
    text.lines()
        .filter(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v.get("ts").and_then(|t| t.as_u64()))
                .is_some_and(|ts| ts >= since)
        })
        .count()
}

/// Count records of one class string in the whole ledger (runtime
/// guard for self-repair: a patch needs its class to have recurred).
pub fn count_class(dir: &Path, class: &str) -> usize {
    let path = dir.join("friction.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else { return 0 };
    text.lines()
        .filter(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(|k| k == class))
                .unwrap_or(false)
        })
        .count()
}

/// Derive mood from mechanical signals: friction since the last
/// boundary + cycle count. STATE vocabulary: weight light/steady/heavy,
/// posture steady/restless/agitated.
///
/// Thresholds are deliberately coarse — this is a dial, not a diagnosis.
pub fn derive(dir: &Path, boundary_ts: u64, cycles: u32) -> (String, String) {
    let n = count_since(dir, boundary_ts) as u32;
    // cycles without friction still count: long work steadies, grind wears
    let load = n.saturating_mul(2) + cycles.min(4);
    let weight = match load {
        0..=2 => "light",
        3..=6 => "steady",
        _ => "heavy",
    };
    let posture = match n {
        0 => if cycles >= 3 { "steady" } else { "fresh" },
        1..=2 => "steady",
        3..=5 => "restless",
        _ => "agitated",
    };
    (weight.to_string(), posture.to_string())
}

/// Rewrite mood.md at a phase boundary (runtime-owned).
pub fn write_mood(dir: &Path, session: &str, phase: &str, boundary_ts: u64, cycles: u32) {
    let (weight, posture) = derive(dir, boundary_ts, cycles);
    let body = format!(
        "# mood\n\nweight: {weight}\nposture: {posture}\nfriction_since_last_boundary: {}\ncycles: {cycles}\nphase: {phase}\nupdated_ts: {}\n",
        count_since(dir, boundary_ts),
        now_ts()
    );
    let _ = std::fs::write(dir.join("mood.md"), body);
    let _ = session; // session kept for future per-session moods
}

/// The mood file as the Observation prompt sees it.
pub fn read_mood(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("mood.md")).unwrap_or_else(|_| "(no mood yet)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn record_and_count() {
        let d = dir();
        record(d.path(), "s1", FrictionKind::ToolFailed, "read_file error: nope");
        record(d.path(), "s1", FrictionKind::LlmRetry, "api 500");
        assert_eq!(count_since(d.path(), 0), 2);
    }

    #[test]
    fn count_respects_since() {
        let d = dir();
        record(d.path(), "s1", FrictionKind::LlmRetry, "x");
        // ts of the just-written record is "now" — anything after counts
        let now = now_ts();
        assert_eq!(count_since(d.path(), now + 100), 0);
    }

    #[test]
    fn derive_light_when_quiet() {
        let d = dir();
        let (w, p) = derive(d.path(), 0, 0);
        assert_eq!((w.as_str(), p.as_str()), ("light", "fresh"));
    }

    #[test]
    fn derive_heavy_under_friction() {
        let d = dir();
        for i in 0..4 {
            record(d.path(), "s1", FrictionKind::PhaseStrike, &format!("strike {i}"));
        }
        let (w, p) = derive(d.path(), 0, 0);
        assert_eq!((w.as_str(), p.as_str()), ("heavy", "restless"));
    }

    #[test]
    fn count_class_finds_recurring() {
        let d = dir();
        record(d.path(), "s1", FrictionKind::PhaseStrike, "a");
        record(d.path(), "s1", FrictionKind::PhaseStrike, "b");
        record(d.path(), "s1", FrictionKind::ToolFailed, "c");
        assert_eq!(count_class(d.path(), "phase_strike"), 2);
        assert_eq!(count_class(d.path(), "tool_failed"), 1);
        assert_eq!(count_class(d.path(), "llm_retry"), 0);
        assert_eq!(count_class(d.path(), "failover"), 0);
    }

    #[test]
    fn mood_file_roundtrip() {
        let d = dir();
        write_mood(d.path(), "s1", "Planning", 0, 2);
        let text = read_mood(d.path());
        assert!(text.contains("weight:"), "{text}");
        assert!(text.contains("phase: Planning"), "{text}");
    }
}
