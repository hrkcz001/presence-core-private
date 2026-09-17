//! Reflex spine: the daemon (PLAN §9 arch B, §11.3). When a goal is
//! active, `session/prompt` replies `end_turn` immediately and drops
//! the message into a mailbox; the circle keeps running in this thread,
//! streaming via out-of-turn session/update (S1-proven). Mailbox is
//! consumed at phase boundaries only — never mid-phase.

use crate::asides;
use crate::acp;
use crate::i18n;
use crate::friction::{self, FrictionKind};
use crate::llm::Bridge;
use crate::phase::{self, Phase};
use crate::tools;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DaemonState {
    pub session_id: String,
    pub goal: String,
    pub phase: Phase,
    pub strikes: u32,
    pub cycles: u32,
    pub boundary_ts: u64,
    pub plain_mode: bool,
    /// One self-repair attempt per goal (M5).
    pub patch_attempted: bool,
    /// Zed stop button parked the circle; next owner prompt resumes.
    pub parked: bool,
    /// One CAP_EMPTY retry per phase with a bigger token cap.
    pub cap_retry: bool,
    /// Pinned file excerpts (context.pin): path -> excerpt, kept in the
    /// context at assembly priority 2, never digested away.
    pub pinned: std::collections::HashMap<String, String>,
}

fn now_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Owner input arriving between turns (session/prompt payloads).
pub enum Mail {
    Prompt { text: String },
    Stop,
    /// Zed stop button: park the circle (resume on next prompt).
    Pause,
}

/// One phase's agent_loop call shaped by phase.rs. Returns the parsed
/// output (phases 1-3) or the raw final text (Vollzug, with goal_done).
pub struct PhaseCtx {
    pub bridge: Arc<Mutex<Bridge>>,
    pub url: String,
    pub api_key: String,
    pub model: String,
    pub tool_ctx: Arc<tools::ToolCtx>,
    pub tool_defs: Arc<Vec<Value>>,
    pub memory_dir: PathBuf,
    pub base_system: String,
}

/// Run the full circle until the goal is done or the owner stops.
/// The receiver carries owner mail; every message lands at the next
/// phase boundary.
pub fn run(
    mut state: DaemonState,
    mut messages: Vec<Value>,
    mail: Receiver<Mail>,
    ctx: PhaseCtx,
    transcript: Arc<Mutex<Vec<crate::Entry>>>,
    transcript_path: PathBuf,
) {
    let sid = state.session_id.clone();
    let tools_all = (*ctx.tool_defs).clone();

    // aside pool: one cheap batch per goal; refill only when dry
    {
        let pause_dry = asides::take(&ctx.memory_dir, "pause").is_none();
        // put it back — take() rotated it away; simpler: check file
        let has_any = std::fs::read_to_string(ctx.memory_dir.join("asides.jsonl"))
            .map(|t| t.lines().count() >= 4)
            .unwrap_or(false);
        if !has_any {
            if let Some(batch) = generate_asides(&ctx) {
                let mut out = String::new();
                for (kind, list) in [("pause", &batch.0), ("resume", &batch.1)] {
                    for t in list {
                        out.push_str(&serde_json::to_string(&serde_json::json!({"kind": kind, "text": t})).unwrap_or_default());
                        out.push('\n');
                    }
                }
                let _ = std::fs::write(ctx.memory_dir.join("asides.jsonl"), out);
            }
        }
        let _ = pause_dry;
    }
    loop {
        // parked (Zed stop): wait for the next mail, no LLM calls
        // drain mailbox at the boundary
        // drain mailbox at the boundary
        let mut owner_input: Option<String> = None;
        loop {
            match mail.try_recv() {
                Ok(Mail::Pause) => {
                    state.parked = true;
                    {
                        let id = format!("st-pause-{}", uuid_counter());
                        acp::tool_call(&sid, &id, &crate::i18n::paused(&crate::asides::take_or_fallback(&ctx.memory_dir, "pause")), "other");
                        acp::tool_call_update(&sid, &id, "completed", "");
                    }
                }
                Ok(Mail::Stop) => {
                    {
                let id = format!("st-stop-{}", state.cycles);
                acp::tool_call(&sid, &id, &i18n::goal_stopped(), "other");
                acp::tool_call_update(&sid, &id, "completed", "");
            }
                    return;
                }
                Ok(Mail::Prompt { text }) => owner_input = Some(text),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        }

        if state.parked {
            match mail.recv() {
                Ok(Mail::Prompt { text }) => {
                    state.parked = false;
                    {
                        let id = format!("st-resume-{}", uuid_counter());
                        acp::tool_call(&sid, &id, &crate::i18n::resumed(&crate::asides::take_or_fallback(&ctx.memory_dir, "resume")), "other");
                        acp::tool_call_update(&sid, &id, "completed", "");
                    }
                    owner_input = Some(text);
                }
                Ok(Mail::Pause) => {}
                Ok(Mail::Stop) | Err(_) => return,
            }
        }

        // owner words are a world-change, not another input: the
        // circle re-throws into Geworfenheit with them as the first
        // given — never pasted into a mid-flight Entwurf/Vorlaufen
        let phase_now = if owner_input.is_some() {
            state.phase = Phase::Observation;
            state.strikes = 0; // fresh disclosure, no stale strikes
            Phase::Observation
        } else {
            state.phase
        };
        if let Some(text) = &owner_input {
            let _ = std::fs::write(ctx.memory_dir.join("mail.latest.txt"), text);
            messages.push(json!({
                "role": "user",
                "content": format!(
                    "[runtime] The owner spoke while the circle was running — sensory mail received: \"{text}\". Re-read the situation with this as a primary given.",
                ),
            }));
        } else {
            let _ = std::fs::remove_file(ctx.memory_dir.join("mail.latest.txt"));
        }

        

        let phase_tools = phase::phase_tools(phase_now, &tools_all);
        let mut phase_messages = messages.clone();
        // phase status as a live tool-call card (compact, spinner,
        // replaces the "Thinking" wall): open at phase start, close at end
        let phase_call_id = format!("phase-{}-{}", state.cycles, phase_now.name());
        let icon = match phase_now {
                Phase::Observation => "\u{25D1}",
                Phase::Planning => "\u{25CE}",
                Phase::Verification => "\u{25D3}",
                Phase::Execution => "\u{25B2}",
            };
            acp::tool_call(&sid, &phase_call_id, &format!("{icon} {}", phase_now.name()), "other");

        // pinned excerpts (context.pin) ride at high priority
        let _pinned_block = {
            let pinned = ctx.tool_ctx.pinned.lock().map(|p| p.clone()).unwrap_or_default();
            if pinned.is_empty() {
                String::new()
            } else {
                let body = pinned
                    .iter()
                    .map(|(k, v)| format!("--- pinned: {k} ---
{v}"))
                    .collect::<Vec<_>>()
                    .join("
");
                format!("

--- pinned context ---
{body}")
            }
        };

        // Observation wakes into the unified sensory grounding
        let grounding = if phase_now == Phase::Observation {
            let engine = crate::senses::SensesEngine::new();
            let sense_ctx = crate::senses::SenseCtx {
                workspace: &ctx.tool_ctx.desk,
                memory_dir: &ctx.memory_dir,
            };
            let mask_guard = ctx.tool_ctx.senses_mask.lock().ok();
            engine.assemble_grounding(
                &crate::config::Config::cached().senses,
                mask_guard.as_deref(),
                &sense_ctx,
            )
        } else {
            String::new()
        };
        let limit = crate::budgeter::context_limit(&ctx.model);
        let est = phase_messages
            .iter()
            .map(|m| crate::budgeter::estimate(m.get("content").and_then(|c| c.as_str()).unwrap_or("")))
            .sum::<u64>();
        let status = crate::budgeter::status_line(est, limit, state.cycles, phase_now.name().to_lowercase().as_str());
        phase_messages[0] = json!({
            "role": "system",
            "content": format!("{}{}{}

{}", ctx.base_system, grounding, phase::phase_prompt(phase_now), status),
        });
        // pre-flight (PLAN s3): refuse to assemble over budget
        if est > limit {
            acp::agent_chunk(&sid, None, &format!("[presence] context over budget ({est} > {limit}) — compacting
"));
            let compact_req = crate::budgeter::compact_prompt(&phase_messages);
            let mut compact_msgs = vec![
                json!({"role": "system", "content": "You are a compactor."}),
                json!({"role": "user", "content": compact_req}),
            ];
            let mut guard = ctx.bridge.lock().unwrap();
            let digest = crate::agent_loop(
                &mut guard, &ctx.url, &ctx.api_key, &ctx.model,
                &mut compact_msgs, &vec![], std::sync::Arc::clone(&ctx.tool_ctx), &sid,
                &ctx.memory_dir.join("ledger.jsonl"), "compact", 2,
            );
            drop(guard);
            if let Ok(d) = digest {
                phase_messages.truncate(1);
                phase_messages.push(json!({"role": "user", "content": format!("[digest of prior context]
{d}")}));
            } else {
                acp::agent_chunk(&sid, None, "[presence] compaction failed — surfacing to owner, stopping circle
");
                return;
            }
        }

        // one bounded call per phase; on schema strike: reprompt once, then plain
        let reply: Result<String, String> = loop {
            let mut guard = ctx.bridge.lock().unwrap();
            // Vollzug body prints once at the end (marker stripped);
            // phases 1-3 and plain-mode internals stay silent
            let quiet = !state.plain_mode;
            // after a strike give the model more rope to obey the schema
            let cap = if state.cap_retry { 2400 }
                else if state.strikes > 0 { 1600 }
                else if phase_now == Phase::Execution { 1500 }
                else { 800 };
            let r = crate::agent_loop_mode(
                &mut guard,
                &ctx.url,
                &ctx.api_key,
                &ctx.model,
                &mut phase_messages,
                &phase_tools,
                std::sync::Arc::clone(&ctx.tool_ctx),
                &sid,
                &ctx.memory_dir.join("ledger.jsonl"),
                "goal",
                8,
                quiet,
                // lightning circle (owner): schema phases are tiny JSON;
                // Vollzug gets room to speak. Kills the 6k-token
                // reasoning storms on reconnaissance calls.
                Some(cap),
                // tool results re-entering context are capped tighter
                // than the global 8000: reconnaissance reads are glimpses
                if phase_now == Phase::Execution { 4000 } else { 1500 },
            );
            let r = match r {
                Err(e) if e.contains("CAP_EMPTY") && !state.cap_retry => {
                    // not the model's fault: reasoning ate the window.
                    // One retry with more rope, no strike, no friction.
                    state.cap_retry = true;
                    {
                        let id = format!("st-cap-{}", uuid_counter());
                        acp::tool_call(&sid, &id, &crate::i18n::cap_retry(), "other");
                        acp::tool_call_update(&sid, &id, "completed", "");
                    }
                    let mut guard = ctx.bridge.lock().unwrap();
                    crate::agent_loop_mode(
                        &mut guard,
                        &ctx.url,
                        &ctx.api_key,
                        &ctx.model,
                        &mut phase_messages,
                        &phase_tools,
                        std::sync::Arc::clone(&ctx.tool_ctx),
                        &sid,
                        &ctx.memory_dir.join("ledger.jsonl"),
                        "goal",
                        8,
                        quiet,
                        Some(2400),
                        if phase_now == Phase::Execution { 4000 } else { 1500 },
                    )
                }
                other => other,
            };
            state.cap_retry = false;
            match r {
                Ok(text) => {
                    if state.plain_mode || phase_now == Phase::Execution {
                        break Ok(text);
                    }
                    match phase::parse_phase(phase_now, &text) {
                        Ok(_) => break Ok(text),
                        Err(e) => {
                            state.strikes += 1;
                            friction::record(
                                &ctx.memory_dir,
                                &sid,
                                FrictionKind::PhaseStrike,
                                &e,
                            );
                            // M5 self-repair: recurring schema misses ->
                            // draft a prompt.md amendment (runtime-guarded:
                            // only on the 2nd strike of this goal, once per
                            // goal; the amendment lands in a proposal file,
                            // applied only by the owner)
                            if state.strikes == 2 && !state.patch_attempted {
                                state.patch_attempted = true;
                                if let Some(amendment) = draft_rule_patch(&ctx, &phase_messages) {
                                    let proposal = ctx.memory_dir.join("prompt-amendment.md");
                                    let _ = std::fs::write(&proposal, amendment);
                                    friction::record(
                                        &ctx.memory_dir,
                                        &sid,
                                        FrictionKind::RulePatch,
                                        "schema-strike amendment drafted -> prompt-amendment.md",
                                    );
                                    acp::agent_chunk(
                                        &sid,
                                        None,
                                        "[presence] self-repair: prompt amendment drafted (memory/prompt-amendment.md)
",
                                    );
                                }
                            }
                            {
                                let id = format!("st-strike-{}-{}", state.cycles, state.strikes);
                                acp::tool_call(&sid, &id, &i18n::phase_strike(state.strikes, &e), "other");
                                acp::tool_call_update(&sid, &id, "completed", "");
                            }
                            if state.strikes >= 3 {
                                state.plain_mode = true;
                                acp::thought_chunk(
                                    &sid,
                                    None,
                                    &i18n::plain_mode(),
                                );
                                break Ok(text);
                            }
                            phase_messages.push(json!({"role": "assistant", "content": text}));
                            phase_messages.push(json!({
                                "role": "user",
                                "content": phase::rejection_note(phase_now, &e),
                            }));
                        }
                    }
                }
                Err(e) => {
                    friction::record(&ctx.memory_dir, &sid, FrictionKind::LlmRetry, &e);
                    if e.contains("CAP_EMPTY") {
                        // even the retry got eaten: degrade to plain mode
                        // (no JSON demands) instead of dying — the circle
                        // continues, the owner loses nothing
                        state.plain_mode = true;
                        let id = format!("st-plain-{}", uuid_counter());
                        acp::tool_call(&sid, &id, &crate::i18n::plain_fallback(), "other");
                        acp::tool_call_update(&sid, &id, "completed", "");
                        continue;
                    }
                    // provider flakiness (content-blocked etc. after all
                    // retries) must NOT kill the circle: park it instead —
                    // the next owner prompt or a later wake resumes.
                    acp::error_card(&sid, &e);
                    state.parked = true;
                    break Err(e);
                }
            }
        };

        let reply = match reply {
            Ok(t) => t,
            // parked, not dead: the circle waits for the next owner
            // prompt (mailbox unparks it); no panic, no process exit
            Err(_) => continue,
        };

        // extract structured output for context; keep raw for Vollzug
        let parsed = if state.plain_mode || phase_now == Phase::Execution {
            None
        } else {
            phase::parse_phase(phase_now, &reply).ok()
        };
        if let Some(out) = &parsed {
            messages.push(phase::as_context(out));
        } else {
            messages.push(json!({"role": "assistant", "content": reply}));
        }

        // phase card body: the structured thought (Situation/Plan/
        // Anticipation JSON) — collapsed in Zed, one click to expand
        let phase_body: String = match &parsed {
            Some(out) => match out {
                phase::PhaseOutput::Situation(s) => {
                    format!("givens:
{}

anomalies:
{}

unknowns:
{}

mood: {}",
                        s.givens.iter().map(|g| format!("- {g}")).collect::<Vec<_>>().join("
"),
                        s.anomalies.iter().map(|g| format!("- {g}")).collect::<Vec<_>>().join("
"),
                        s.unknowns.iter().map(|g| format!("- {g}")).collect::<Vec<_>>().join("
"),
                        s.mood,
                    )
                }
                phase::PhaseOutput::Plan(p) => {
                    format!("options:
{}

chosen: {}

why: {}

first step: {}",
                        p.options.iter().map(|o| format!("- {o}")).collect::<Vec<_>>().join("
"),
                        p.chosen, p.why, p.first_step,
                    )
                }
                phase::PhaseOutput::Anticipation(a) => {
                    format!("failure modes:
{}

cheap tests:
{}

revised plan: {}",
                        a.failure_modes.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("
"),
                        a.cheap_tests.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("
"),
                        a.revised_plan,
                    )
                }
            },
            None => String::new(),
        };
        acp::tool_call_update(&sid, &phase_call_id, "completed", &phase_body.chars().take(1200).collect::<String>());
        // native plan checklist: the four phases of this cycle
        {
            let phases = [
                (Phase::Observation, "pending"),
                (Phase::Planning, "pending"),
                (Phase::Verification, "pending"),
                (Phase::Execution, "pending"),
            ];
            let entries: Vec<(String, &str)> = phases
                .iter()
                .map(|(ph, mut st)| {
                    if *ph == phase_now {
                        st = "completed";
                    } else if ph.next() == Some(phase_now) {
                        st = "in_progress";
                    }
                    (ph.name().to_string(), st)
                })
                .collect();
            acp::plan(&sid, &entries);
        }

        // sleep-state snapshot: the goal survives process death
        {
            let snap = json!({
                "session": sid,
                "goal": state.goal,
                "phase": phase_now.name(),
                "strikes": state.strikes,
                "cycles": state.cycles,
                "plain_mode": state.plain_mode,
                "messages": messages,
            });
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(ctx.memory_dir.join("goal.jsonl"))
            {
                use std::io::Write;
                let _ = writeln!(f, "{snap}");
            }
        }

        // mood + goal mirror at the boundary        // mood + goal mirror at the boundary
        friction::write_mood(&ctx.memory_dir, &sid, phase_now.name(), state.boundary_ts, state.cycles);
        state.boundary_ts = now_ts();

        if phase_now == Phase::Execution {
            let done = reply
                .lines()
                .rev()
                .find_map(|l| l.trim().strip_prefix("goal_done:"))
                .map(|v| v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            // human-facing Vollzug output: body without the marker line
            let body: Vec<&str> = reply
                .lines()
                .filter(|l| !l.trim().starts_with("goal_done:"))
                .collect();
            let body = body.join("
").trim().to_string();
            if !body.is_empty() {
                acp::agent_chunk(&sid, None, &format!("{body}

"));
            }
            if done {
                {
                let id = format!("st-done-{}", state.cycles);
                acp::tool_call(&sid, &id, &i18n::goal_done(), "other");
                acp::tool_call_update(&sid, &id, "completed", "");
            }
                if let Ok(mut t) = transcript.lock() {
                    t.push(crate::Entry {
                        topic: "goal".into(),
                        role: "assistant".into(),
                        content: reply.clone(),
                    });
                    append_line(&transcript_path, &crate::Entry {
                        topic: "goal".into(),
                        role: "assistant".into(),
                        content: reply.clone(),
                    });
                }
                // quest journal: mark done, pull the next quest; if the
                // ranking is ambiguous, ASK THE OWNER instead of guessing
                // (owner rule). The circle continues seamlessly.
                {
                    let qpath = &ctx.memory_dir;
                    // mark current done by text match (id not tracked in state)
                    let cur = state.goal.clone();
                    for mut q in crate::quests::load(qpath) {
                        if q.text == cur {
                            q.status = "done".into();
                            let mut all = crate::quests::load(qpath);
                            for x in all.iter_mut() {
                                if x.text == cur { x.status = "done".into(); }
                            }
                            crate::quests::save(qpath, &all);
                        }
                    }
                    match crate::quests::next(qpath) {
                        Some(nq) => {
                            crate::quests::set_status(qpath, nq.id, "in_progress");
                            state.goal = nq.text.clone();
                            state.phase = Phase::Observation;
                            state.cycles = 0;
                            state.strikes = 0;
                            state.plain_mode = false;
                            messages.push(json!({
                                "role": "user",
                                "content": format!("[runtime] Quest done. Next quest from the journal: \"{}\". Begin its circle.", nq.text),
                            }));
                            let id = format!("q-next-{}", state.cycles);
                            acp::tool_call(&sid, &id, &crate::i18n::circle_start(&nq.text), "other");
                            acp::tool_call_update(&sid, &id, "completed", "");
                        }
                        None => {
                            acp::thought_chunk(&sid, None, &crate::i18n::journal_empty());
                            return;
                        }
                    }
                }
                return;
            }
            // world changed -> re-throw (Geworfenheit-lite: tool outcomes
            // are already in context; a fresh Situation follows)
            state.phase = Phase::Observation;
            state.cycles += 1;
            messages.push(json!({
                "role": "user",
                "content": "[runtime] The world changed after execution. Re-read the situation (diff-scan, not full re-read).",
            }));
        } else {
            state.phase = phase_now.next().unwrap_or(Phase::Observation);
        }

    }
}

fn append_line(path: &Path, e: &crate::Entry) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_string(e) {
            let _ = writeln!(f, "{line}");
        }
    }
}


/// M5: ask the model to draft an amendment to its own operational
/// prompt (prompt.md) — a separate small call; the result is a
/// proposal, applied only by the owner (git-versioned).
fn draft_rule_patch(ctx: &PhaseCtx, phase_messages: &[Value]) -> Option<String> {
    let recent: Vec<String> = phase_messages
        .iter()
        .rev()
        .take(4)
        .rev()
        .filter_map(|m| {
            let role = m.get("role").and_then(|r| r.as_str())?;
            let content = m.get("content").and_then(|c| c.as_str())?;
            Some(format!("{role}: {content}"))
        })
        .collect();
    let req = format!(
        "You are Presence in self-repair mode. The phase machine rejected your replies twice — the JSON schema instruction is not landing. Draft an amendment to the operational prompt: a short section that makes the phase output contract unambiguous. Output ONLY the amendment text, ready to append.

Recent context:
{}",
        recent.join("
")
    );
    let mut msgs = vec![
        json!({"role": "system", "content": "You write terse prompt amendments."}),
        json!({"role": "user", "content": req}),
    ];
    let mut guard = ctx.bridge.lock().unwrap();
    let out = crate::agent_loop(
        &mut guard, &ctx.url, &ctx.api_key, &ctx.model,
        &mut msgs, &vec![], std::sync::Arc::clone(&ctx.tool_ctx), "goal",
        &ctx.memory_dir.join("ledger.jsonl"), "self-repair", 2,
    );
    out.ok().filter(|t| !t.trim().is_empty()).map(|t| t.trim().to_string())
}

fn uuid_counter() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static C: AtomicU32 = AtomicU32::new(0);
    C.fetch_add(1, Ordering::Relaxed)
}

/// One cheap call: the model writes its own status asides.
fn generate_asides(ctx: &PhaseCtx) -> Option<(Vec<String>, Vec<String>)> {
    let mut msgs = vec![
        json!({"role": "system", "content": "You write witty micro-statuses. Output ONLY the JSON asked for."}),
        json!({"role": "user", "content": asides::batch_prompt()}),
    ];
    let mut guard = ctx.bridge.lock().unwrap();
    let out = crate::agent_loop(
        &mut guard, &ctx.url, &ctx.api_key, &ctx.model,
        &mut msgs, &vec![], std::sync::Arc::clone(&ctx.tool_ctx), "asides",
        &ctx.memory_dir.join("ledger.jsonl"), "asides", 1,
    );
    let text = out.ok()?;
    // extract JSON
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    let collect = |key: &str| -> Vec<String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    };
    let p = collect("pause");
    let r = collect("resume");
    if p.is_empty() && r.is_empty() { None } else { Some((p, r)) }
}
