//! Phase machine (PLAN §2, §11.2). Schemas, per-phase prompt suffixes,
//! tool subsets, and strict-JSON parsing with rejection. One phase =
//! one agent_loop call; this module only shapes the request and judges
//! the reply.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Observation,
    Planning,
    Verification,
    Execution,
}

#[allow(non_upper_case_globals, dead_code)]
impl Phase {
    // Legacy Heideggerian aliases for backwards compatibility
    pub const Geworfenheit: Phase = Phase::Observation;
    pub const Entwurf: Phase = Phase::Planning;
    pub const Vorlaufen: Phase = Phase::Verification;
    pub const Vollzug: Phase = Phase::Execution;

    pub fn name(&self) -> &'static str {
        match self {
            Phase::Observation => "Observation",
            Phase::Planning => "Planning",
            Phase::Verification => "Verification",
            Phase::Execution => "Execution",
        }
    }

    pub fn from_name(name: &str) -> Option<Phase> {
        match name {
            "Observation" | "Geworfenheit" => Some(Phase::Observation),
            "Planning" | "Entwurf" => Some(Phase::Planning),
            "Verification" | "Vorlaufen" => Some(Phase::Verification),
            "Execution" | "Vollzug" => Some(Phase::Execution),
            _ => None,
        }
    }

    pub fn next(&self) -> Option<Phase> {
        match self {
            Phase::Observation => Some(Phase::Planning),
            Phase::Planning => Some(Phase::Verification),
            Phase::Verification => Some(Phase::Execution),
            Phase::Execution => None, // world changed -> Observation (caller)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Situation {
    pub givens: Vec<String>,
    #[serde(default)]
    pub anomalies: Vec<String>,
    #[serde(default)]
    pub unknowns: Vec<String>,
    #[serde(default)]
    pub mood: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub options: Vec<String>,
    pub chosen: String,
    pub why: String,
    pub first_step: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anticipation {
    pub failure_modes: Vec<String>,
    pub cheap_tests: Vec<String>,
    pub revised_plan: String,
}

/// The schema'd output of phases 1-3 (Execution needs none).
#[derive(Debug, Clone)]
pub enum PhaseOutput {
    Situation(Situation),
    Plan(Plan),
    Anticipation(Anticipation),
}

/// Algorithmic assembly of an observation Situation without invoking an LLM.
/// Pulls environment facts from winsense, friction mood, and the input request.
pub fn assemble_deterministic_observation(
    memory_dir: &std::path::Path,
    prompt: &str,
    winsense_overview: &crate::winsense::Overview,
) -> Situation {
    let mut givens = Vec::new();
    let mut anomalies = Vec::new();
    let unknowns = Vec::new();

    givens.push(format!(
        "Active window: \"{}\" (pid {})",
        winsense_overview.foreground.title, winsense_overview.foreground.pid
    ));
    givens.push(format!(
        "User idle seconds: {:.1}",
        winsense_overview.user_idle_seconds
    ));
    givens.push(format!(
        "Power status: {} (battery {:?}%)",
        winsense_overview.power.power_source, winsense_overview.power.battery_percent
    ));

    let mood = crate::friction::read_mood(memory_dir);
    if mood == "heavy" {
        anomalies.push("High friction detected in recent cycles".to_string());
    }

    if !prompt.trim().is_empty() {
        givens.push(format!("User directive: {}", prompt.trim()));
    }

    Situation {
        givens,
        anomalies,
        unknowns,
        mood,
    }
}

/// System-prompt suffix for a phase; appended after the base prompt.
/// Zero philosophical terms reach the model: pure engineering taxonomy.
pub fn phase_prompt(phase: Phase) -> String {
    match phase {
        Phase::Observation => format!(
            "\n\n--- phase: observation ---\n\
             Assemble the current state: STATE, journal tail, git status, task \
             queue, file reality, vitals tail, mood. No plan allowed \
             here — pure disclosure.\n\
             HARD CONTRACT: reply FIRST character is {{, LAST is }}. No prose \
             before, after or around; no markdown fences. Caught yourself \
             writing text - stop, output only JSON. Keep each field to \
             3-5 short items; a Situation is ~10 lines total.\n\
             {}\n\
             Tools: read-only, bounded.",
            r#"{"givens": ["..."], "anomalies": ["..."], "unknowns": ["..."], "mood": "one word from STATE vocabulary"}"#
        ),
        Phase::Planning => format!(
            "\n\n--- phase: planning ---\n\
             Given the Situation, generate 2-4 candidate courses (each \
             with what would have to be true), choose one course, \
             commit. Planning phase — no action, no tools.\n\
             Reply with ONLY this JSON object (no prose, no fences):\n\
             {}",
            r#"{"options": ["course (what would have to be true)"], "chosen": "one of the options", "why": "...", "first_step": "one bounded step"}"#
        ),
        Phase::Verification => format!(
            "\n\n--- phase: verification ---\n\
             For the chosen plan: failure modes, cheap validation \
             tests (run the read-only ones now), risk-ordered steps.\n\
             Reply with ONLY this JSON object (no prose, no fences):\n\
             {}\n\
             Tools: read-only execution of cheap tests.",
            r#"{"failure_modes": ["..."], "cheap_tests": ["..."], "revised_plan": "..."}"#
        ),
        Phase::Execution => "\n\n--- phase: execution ---\n\
             Execute ONE bounded step of the revised plan. Full tool \
             set. operating rules: bounded output, one turn one topic.\n\
             End the reply with a line:\n\
             goal_done: true|false"
            .to_string(),
    }
}

/// Tool subset for a phase (indices into tools::defs() order).
pub fn phase_tools(phase: Phase, all: &[Value]) -> Vec<Value> {
    let allowed: &[&str] = match phase {
        Phase::Observation => &["read_file", "list_files", "tune_senses", "manage_package", "switch_agent"],
        Phase::Planning => &["switch_agent"],
        Phase::Verification => &["read_file", "list_files", "run_command", "tune_senses", "manage_package", "switch_agent"],
        Phase::Execution => &["read_file", "list_files", "write_file", "run_command", "tune_senses", "manage_package", "switch_agent"],
    };
    all.iter()
        .filter(|d| {
            let name = d.pointer("/function/name").and_then(Value::as_str);
            name.is_some_and(|n| allowed.contains(&n))
        })
        .cloned()
        .collect()
}

/// Extract a JSON object from a reply that may carry stray prose or
/// markdown fences around it.
fn extract_json(reply: &str) -> Option<&str> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    (end > start).then(|| &reply[start..=end])
}

/// Parse a phase reply. Err carries the parse error for the reprompt.
pub fn parse_phase(phase: Phase, reply: &str) -> Result<PhaseOutput, String> {
    let raw = extract_json(reply).ok_or_else(|| "no JSON object found in reply".to_string())?;
    match phase {
        Phase::Observation => serde_json::from_str::<Situation>(raw)
            .map(PhaseOutput::Situation)
            .map_err(|e| format!("Situation schema: {e}")),
        Phase::Planning => serde_json::from_str::<Plan>(raw)
            .map(PhaseOutput::Plan)
            .map_err(|e| format!("Plan schema: {e}")),
        Phase::Verification => serde_json::from_str::<Anticipation>(raw)
            .map(PhaseOutput::Anticipation)
            .map_err(|e| format!("Anticipation schema: {e}")),
        Phase::Execution => Err("Execution has no schema".into()),
    }
}

/// The local reprompt appended when a phase reply doesn't parse
/// (out-of-phase behavior rejected — no API call wasted beyond one).
pub fn rejection_note(phase: Phase, err: &str) -> String {
    format!(
        "[runtime] Phase {}: your reply was rejected — {err}. \
         Reply again with ONLY the required JSON object.",
        phase.name()
    )
}

/// A phase reply as a compact context block for the next phase.
pub fn as_context(out: &PhaseOutput) -> Value {
    match out {
        PhaseOutput::Situation(s) => json!({"role": "assistant", "content": serde_json::to_string(s).unwrap_or_default()}),
        PhaseOutput::Plan(p) => json!({"role": "assistant", "content": serde_json::to_string(p).unwrap_or_default()}),
        PhaseOutput::Anticipation(a) => json!({"role": "assistant", "content": serde_json::to_string(a).unwrap_or_default()}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_cycle_order() {
        assert_eq!(Phase::Observation.next(), Some(Phase::Planning));
        assert_eq!(Phase::Planning.next(), Some(Phase::Verification));
        assert_eq!(Phase::Verification.next(), Some(Phase::Execution));
        assert_eq!(Phase::Execution.next(), None);
    }

    #[test]
    fn test_assemble_deterministic_observation() {
        let temp = tempfile::tempdir().unwrap();
        let overview = crate::winsense::Overview {
            foreground: crate::winsense::ForegroundInfo {
                title: "Editor".into(),
                pid: 1234,
            },
            user_idle_seconds: 12.5,
            power: crate::winsense::PowerInfo {
                power_source: "battery".into(),
                battery_percent: Some(85),
            },
            ..Default::default()
        };
        let sit = assemble_deterministic_observation(temp.path(), "run tests", &overview);
        assert!(sit.givens.iter().any(|g| g.contains("Editor")));
        assert!(sit.givens.iter().any(|g| g.contains("run tests")));
        assert!(!sit.mood.is_empty());
    }


    #[test]
    fn parse_situation_strict() {
        let out =
            parse_phase(Phase::Observation, r#"{"givens":["a"],"anomalies":[],"unknowns":[],"mood":"steady"}"#)
                .expect("parses");
        assert!(matches!(out, PhaseOutput::Situation(_)));
    }

    #[test]
    fn parse_situation_defaults() {
        let out = parse_phase(Phase::Observation, r#"{"givens":["a"]}"#).expect("parses");
        let PhaseOutput::Situation(s) = out else { panic!() };
        assert_eq!(s.mood, "");
    }

    #[test]
    fn parse_plan_behind_fences() {
        let reply = "Here is my plan:\n```json\n{\"options\":[\"x\"],\"chosen\":\"x\",\"why\":\"y\",\"first_step\":\"z\"}\n```";
        let out = parse_phase(Phase::Planning, reply).expect("parses");
        assert!(matches!(out, PhaseOutput::Plan(_)));
    }

    #[test]
    fn parse_rejects_prose() {
        assert!(parse_phase(Phase::Planning, "I will do the thing.").is_err());
    }

    #[test]
    fn parse_rejects_wrong_schema() {
        // Plan-shaped reply in Observation -> wrong fields -> error
        assert!(parse_phase(
            Phase::Observation,
            r#"{"options":["x"],"chosen":"x","why":"y","first_step":"z"}"#
        )
        .is_err());
    }

    #[test]
    fn tool_subsets() {
        let all = crate::tools::defs();
        assert_eq!(phase_tools(Phase::Planning, &all).len(), 1);
        assert_eq!(phase_tools(Phase::Observation, &all).len(), 5);
        assert_eq!(phase_tools(Phase::Execution, &all).len(), 7);
        let vor = phase_tools(Phase::Verification, &all);
        assert!(vor.iter().any(|d| d.pointer("/function/name") == Some(&json!("run_command"))));
        assert!(vor.iter().all(|d| d.pointer("/function/name") != Some(&json!("write_file"))));
    }

    #[test]
    fn prompts_are_sanitized_of_heideggerian_terms() {
        let forbidden = [
            "geworfenheit",
            "entwurf",
            "vorlaufen",
            "vollzug",
            "thrownness",
            "projection",
            "running-ahead",
        ];
        for ph in [
            Phase::Observation,
            Phase::Planning,
            Phase::Verification,
            Phase::Execution,
        ] {
            let prompt = phase_prompt(ph).to_lowercase();
            for term in forbidden {
                assert!(
                    !prompt.contains(term),
                    "Phase {:?} prompt contains forbidden philosophical term '{term}': {prompt}",
                    ph
                );
            }
        }
    }
}
