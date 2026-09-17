//! Status line and card text formatting for Presence.

pub fn circle_start(goal: &str) -> String {
    let head: String = goal.chars().take(60).collect();
    format!("\u{25CF} New goal: {head}")
}

pub fn goal_done() -> String {
    "\u{2713} Goal completed".into()
}

pub fn goal_stopped() -> String {
    "\u{2713} Stopped by owner".into()
}

pub fn phase_strike(n: u32, _err: &str) -> String {
    format!("\u{21BB} Retrying {n}/3")
}

pub fn plain_mode() -> String {
    "Three strikes \u{2014} plain mode for the rest of the goal".into()
}

pub fn tool_round_limit() -> String {
    "\u{21BB} Tool round limit \u{2014} forcing a final answer".into()
}

pub fn cap_retry() -> String {
    "\u{21BB} Thinking timeout \u{2014} retrying with direct response".into()
}

pub fn plain_fallback() -> String {
    "Switching to direct execution".into()
}

pub fn journal_empty() -> String {
    "(journal empty)".into()
}

/// Error card title: concise category by error signature.
pub fn error_title(e: &str) -> String {
    let warn = "\u{26A0} ";
    if e.contains("content-blocked") {
        format!("{warn}Content blocked by provider policy")
    } else if e.contains("api 402") || e.contains("quota") {
        format!("{warn}API quota exhausted")
    } else if e.contains("api 401") {
        format!("{warn}API key rejected (401)")
    } else if e.contains("api 429") {
        format!("{warn}Rate limit exceeded (429)")
    } else if e.contains("api 400") {
        format!("{warn}Bad request to model API (400)")
    } else if e.contains("api 5") {
        format!("{warn}Provider server error \u{2014} retrying")
    } else if e.contains("bridge") || e.contains("request failed") {
        format!("{warn}LLM bridge communication error \u{2014} retrying")
    } else if e.contains("CAP_EMPTY") {
        "\u{25D1} Response delayed \u{2014} simplifying mode".into()
    } else if e.contains("empty completion") {
        format!("{warn}Empty completion from model")
    } else if e.contains("timeout") {
        format!("{warn}Command execution timed out")
    } else {
        format!("{warn}Error: {}", e.chars().take(60).collect::<String>())
    }
}

pub fn paused(aside: &str) -> String {
    format!("\u{258D}\u{258D} Paused \u{2014} {aside}")
}

pub fn resumed(aside: &str) -> String {
    format!("\u{25B6} Resuming \u{2014} {aside}")
}

pub fn hung() -> String {
    "...thinking".into()
}
