// presence heartbeat — the standing loop's skeleton (AUTONOMY.md).
// A0: wake/sleep cycle, voice trigger, act/idle decision log.
// A1: alarms.jsonl replaces wake.txt — {id, fire_at, reason, origin},
//     origin: owner | self | vitals | future:economy. A due alarm is
//     an invitation, not a command — the smart-contract seat: the
//     shape a blockchain will later drive (AUTONOMY.md; no chain here).
//
// Triggers:
//   voice — a new ts in heard.jsonl past our cursor (junk lines
//           shorter than 3 chars are consumed silently)
//   alarm — memory/alarms.jsonl holds a due line (fire_at <= now).
//           One due alarm per scan — uncapped ones first, then the
//           earliest fire_at; the rest wait for the next cycle, so
//           storms self-discharge. Max 20 active (AUTONOMY.md Risks):
//           the overflow moves to alarms.dropped.jsonl — read,
//           logged, relocated, never destroyed.
//
// Decision rule (mechanical judgment only — the LLM wake-decision
// arrives with A2):
//   voice         -> act (owner authority, never rate-capped)
//   alarm, owner  -> act, never rate-capped (the owner's will)
//   alarm, other  -> act, unless within the 20-min initiative cap.
//                    A capped alarm is not consumed; attempts count
//                    (debounced to one per cap window), the third
//                    retires it silently — the expired-unacked death
//                    from AUTONOMY.md Risks. A real ack arrives with
//                    A2, when there is someone to ack.
//
// Consumption rewrites alarms.jsonl without the fired id, re-reading
// first so a concurrent hand-edit survives; unparseable lines are
// preserved verbatim (step 19 law: never destroy what you haven't
// read).
//
// Decisions land in memory/heartbeat.log, one jsonl line per wake.
// Test hooks (defaults in parentheses): PRESENCE_HEARTBEAT_INTERVAL_SECS
// (5), PRESENCE_HEARTBEAT_MAXWAKES (0 = run forever),
// PRESENCE_HEARTBEAT_ATTEMPT_DEBOUNCE_SECS (1200).

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BEAT_EVERY: u64 = 15 * 60; // liveness line while quiet

fn cfg() -> &'static presence::config::Config {
    presence::config::Config::cached()
}

#[derive(Clone)]
struct Alarm {
    id: String,
    fire_at: u64,
    reason: String,
    origin: String,
    line: String, // original jsonl, preserved verbatim on rewrite
}

struct Scan {
    alarms: Vec<Alarm>,
    junk: Vec<String>, // unparseable lines, kept verbatim
}

fn main() {
    let mem = memory_dir();
    let heard = mem.join("heard.jsonl");
    let alarms_path = mem.join("alarms.jsonl");
    let log = mem.join("heartbeat.log");
    let interval: u64 = env_num("PRESENCE_HEARTBEAT_INTERVAL_SECS", cfg().heartbeat.interval_secs);
    let max_wakes: u64 = env_num("PRESENCE_HEARTBEAT_MAXWAKES", cfg().heartbeat.max_wakes);
    let debounce: u64 = env_num("PRESENCE_HEARTBEAT_ATTEMPT_DEBOUNCE_SECS", cfg().heartbeat.cap_secs);
    let idle_interval: u64 = (interval * 6).max(30);
    let mut current_interval: u64 = interval;

    let mut stembus = presence::stembus::StemBus::discover();

    let mut voice_cursor = latest_voice(&heard).map(|(ts, _)| ts).unwrap_or(0);
    let mut last_act: u64 = 0;
    let mut last_beat: u64 = now();
    let mut brainless_mode = false;
    let mut attempts: HashMap<String, (u32, u64)> = HashMap::new(); // id -> (tries, last try)
    log_line(&log, "start", &format!("watching {} cursor {voice_cursor} ({} stimuli active)", mem.display(), stembus.stimuli.len()));
    eprintln!("stem: watching {} (voice cursor {voice_cursor}, {} stimuli active)", mem.display(), stembus.stimuli.len());

    let mut wakes: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_secs(current_interval));
        wakes += 1;
        let now = now();

        // --- autonomous vegetative stimuli via StemBus ---
        let stimulus_events = stembus.poll_due(now);
        for ev in stimulus_events {
            if ev.stimulus_name == "user_idle" {
                if ev.triggered {
                    if current_interval != idle_interval {
                        log_line(&log, "modulate_pulse", &format!("user idle threshold reached -> slowing pulse to {idle_interval}s"));
                        current_interval = idle_interval;
                    }
                } else if current_interval != interval {
                    log_line(&log, "modulate_pulse", &format!("user presence restored -> accelerating pulse to {interval}s"));
                    current_interval = interval;
                }
            } else if ev.stimulus_name == "battery_low" && ev.triggered {
                log_line(&log, "stimulus_alert", "battery low (<15%) triggered");
                current_interval = interval;
                let alarm_id = "alarm:stimulus:battery_low".to_string();
                let alert_alarm = serde_json::json!({
                    "id": alarm_id,
                    "fire_at": now,
                    "reason": "Battery critical (<15%), conscious intervention required",
                    "origin": "vitals"
                });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&alarms_path) {
                    let _ = writeln!(f, "{alert_alarm}");
                }
            } else if ev.stimulus_name == "network_state" {
                let online = ev.data.get("online").and_then(|v| v.as_bool()).unwrap_or(true);
                if !online && !brainless_mode {
                    brainless_mode = true;
                    log_line(&log, "network_offline", "internet offline -> entering vegetative brainless mode (LLM disabled)");
                    eprintln!("stem: [VEGETATIVE] Internet disconnected. LLM Cortex disabled. Running in autonomous reflex/stem mode.");
                } else if online && brainless_mode {
                    brainless_mode = false;
                    log_line(&log, "network_online", "internet restored -> conscious Cortex mode re-enabled");
                    eprintln!("stem: [VEGETATIVE] Internet restored. Cortex consciousness enabled.");
                }
            } else if ev.stimulus_name == "display_locked" {
                if ev.triggered && current_interval < idle_interval * 2 {
                    log_line(&log, "modulate_pulse", "display locked -> deep sleep cadence (60s)");
                    current_interval = idle_interval * 2;
                }
            } else if ev.stimulus_name == "high_cpu" && ev.triggered {
                if current_interval < idle_interval {
                    log_line(&log, "high_cpu", "CPU spike detected -> pacing pulse");
                    current_interval = idle_interval;
                }
            } else if ev.stimulus_name == "disk_space_low" && ev.triggered {
                log_line(&log, "stimulus_alert", "disk space critically low (< 5GB)");
                let alarm_id = "alarm:stimulus:disk_space_low".to_string();
                let alert_alarm = serde_json::json!({
                    "id": alarm_id,
                    "fire_at": now,
                    "reason": "Disk space critically low (< 5GB), storage cleanup advised",
                    "origin": "io"
                });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&alarms_path) {
                    let _ = writeln!(f, "{alert_alarm}");
                }
            } else if ev.stimulus_name == "git_dirty_drift" && ev.triggered {
                log_line(&log, "stimulus_alert", "git working tree dirty drift");
                let alarm_id = "alarm:stimulus:git_dirty_drift".to_string();
                let alert_alarm = serde_json::json!({
                    "id": alarm_id,
                    "fire_at": now,
                    "reason": "Uncommitted git drift detected, checkpoint advised",
                    "origin": "git"
                });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&alarms_path) {
                    let _ = writeln!(f, "{alert_alarm}");
                }
            } else if ev.stimulus_name == "stale_goal" && ev.triggered {
                log_line(&log, "stimulus_alert", "stale goals (>12h without update)");
                let alarm_id = "alarm:stimulus:stale_goal".to_string();
                let alert_alarm = serde_json::json!({
                    "id": alarm_id,
                    "fire_at": now,
                    "reason": "Active goals in GOALS.md idle for >12 hours",
                    "origin": "state"
                });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&alarms_path) {
                    let _ = writeln!(f, "{alert_alarm}");
                }
            }
        }

        // --- scan triggers ---
        // (reason, rate-capped?, alarm id to consume on act)
        let mut reason: Option<(String, bool, Option<String>)> = None;
        if let Some((ts, text)) = latest_voice(&heard).filter(|(ts, _)| *ts > voice_cursor) {
            voice_cursor = ts;
            let t = text.trim();
            if t.chars().count() >= 3 {
                reason = Some((format!("voice: {t}"), false, None));
            }
        }

        // alarms: storm cap first, then at most one due alarm per scan
        let scan = enforce_storm_cap(&alarms_path, &log, read_alarms(&alarms_path));
        if reason.is_none() {
            // uncapped due alarms first (owner alarms must not starve
            // behind a capped self-alarm), then earliest fire_at
            let chosen = scan
                .alarms
                .iter()
                .filter(|a| a.fire_at <= now)
                .min_by_key(|a| (is_capped(a, now, last_act), a.fire_at))
                .cloned();
            if let Some(a) = chosen {
                if is_capped(&a, now, last_act) {
                    let e = attempts.entry(a.id.clone()).or_insert((0, 0));
                    if now.saturating_sub(e.1) >= debounce {
                        e.0 += 1;
                        e.1 = now;
                    }
                    if e.0 >= cfg().heartbeat.max_attempts {
                        consume_alarms(&alarms_path, &log, &[a.id.clone()]);
                        attempts.remove(&a.id);
                        log_line(&log, "alarm-dead", &format!(
                            "alarm {} retired: {} capped attempts, unacked",
                            a.id, cfg().heartbeat.max_attempts
                        ));
                        // reason stays None: a death is not a wake
                    } else {
                        reason = Some((
                            format!("alarm {} capped (attempt {}/{}, origin {})",
                                    a.id, e.0, cfg().heartbeat.max_attempts, a.origin),
                            true, None,
                        ));
                    }
                } else {
                    reason = Some((
                        format!("alarm {}: {} [{}]", a.id, a.reason, a.origin),
                        false, Some(a.id.clone()),
                    ));
                }
            }
        }

        if brainless_mode {
            if let Some((ref r, _, _)) = reason {
                log_line(&log, "suppressed_offline", &format!("suppressed wake while offline: {r}"));
            }
            reason = None;
        }

        // --- decide & log ---
        match reason {
            Some((r, capped, consume_id)) => {
                let decision = if capped { "idle" } else { "act" };
                log_line(&log, decision, &r);
                if decision == "act" {
                    last_act = now;
                    if let Some(id) = consume_id {
                        consume_alarms(&alarms_path, &log, &[id.clone()]);
                        attempts.remove(&id);
                    }
                }
            }
            None if now.saturating_sub(last_beat) >= BEAT_EVERY => {
                log_line(&log, "beat", "");
                last_beat = now;
            }
            None => {}
        }

        if max_wakes > 0 && wakes >= max_wakes {
            log_line(&log, "stop", "max wakes reached (test hook)");
            break;
        }
    }
}

fn is_capped(a: &Alarm, now: u64, last_act: u64) -> bool {
    a.origin != "owner" && now.saturating_sub(last_act) < cfg().heartbeat.cap_secs
}

fn read_alarms(path: &Path) -> Scan {
    let mut scan = Scan { alarms: Vec::new(), junk: Vec::new() };
    let Ok(s) = std::fs::read_to_string(path) else { return scan };
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<serde_json::Value>(t)
            .ok()
            .and_then(|v| {
                let id = v.get("id")?.as_str()?.to_string();
                let fire_at = v.get("fire_at")?.as_u64()?;
                let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let origin = v.get("origin").and_then(|x| x.as_str()).unwrap_or("self").to_string();
                Some(Alarm { id, fire_at, reason, origin, line: t.to_string() })
            });
        match parsed {
            Some(a) => scan.alarms.push(a),
            None => scan.junk.push(t.to_string()),
        }
    }
    scan
}

/// Over-cap alarms (>cfg().heartbeat.max_alarms) move to alarms.dropped.jsonl.
fn enforce_storm_cap(path: &Path, log: &Path, mut scan: Scan) -> Scan {
    if scan.alarms.len() <= cfg().heartbeat.max_alarms {
        return scan;
    }
    let mut keep = vec![false; scan.alarms.len()];
    let mut order: Vec<usize> = (0..scan.alarms.len()).collect();
    order.sort_by_key(|&i| scan.alarms[i].fire_at); // stable: file order breaks ties
    for &i in order.iter().take(cfg().heartbeat.max_alarms) {
        keep[i] = true; // the cfg().heartbeat.max_alarms soonest survive
    }
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for (i, a) in scan.alarms.drain(..).enumerate() {
        if keep[i] { kept.push(a) } else { dropped.push(a) }
    }
    let dp = path.with_file_name("alarms.dropped.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&dp) {
        for a in &dropped {
            let _ = writeln!(f, "{}", a.line);
        }
    }
    for a in &dropped {
        log_line(log, "alarm-drop", &format!(
            "alarm {} (origin {}) -> alarms.dropped.jsonl: storm cap {}",
            a.id, a.origin, cfg().heartbeat.max_alarms
        ));
    }
    scan.alarms = kept;
    write_scan(path, log, &scan);
    scan
}

/// Rewrite alarms.jsonl without the given ids (fire = invitation
/// accepted, the id is consumed). Re-reads first: a hand-edit racing
/// the rewrite survives.
fn consume_alarms(path: &Path, log: &Path, ids: &[String]) {
    let fresh = read_alarms(path);
    let next = Scan {
        alarms: fresh.alarms.into_iter().filter(|a| !ids.contains(&a.id)).collect(),
        junk: fresh.junk,
    };
    write_scan(path, log, &next);
}

fn write_scan(path: &Path, log: &Path, scan: &Scan) {
    let mut out = String::new();
    for a in &scan.alarms {
        out.push_str(&a.line);
        out.push('\n');
    }
    for j in &scan.junk {
        out.push_str(j);
        out.push('\n');
    }
    let tmp = path.with_extension("jsonl.tmp");
    if std::fs::write(&tmp, &out).and_then(|_| std::fs::rename(&tmp, path)).is_err() {
        log_line(log, "alarm-write-fail", &format!("could not rewrite {}", path.display()));
    }
}

fn memory_dir() -> PathBuf {
    // env PRESENCE_MEMORY > yaml > derived from PRESENCE_WORKSPACE —
    // classic env surface only, no exe-ancestor heuristics.
    cfg().memory_dir_path().unwrap_or_else(|| PathBuf::from("memory"))
}

/// Newest parseable line of heard.jsonl (reads only the last 4 KiB).
fn latest_voice(path: &Path) -> Option<(u64, String)> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let win = 4096u64.min(len);
    f.seek(SeekFrom::Start(len - win)).ok()?;
    let mut buf = vec![0u8; win as usize];
    f.read_exact(&mut buf).ok()?;
    let s = String::from_utf8_lossy(&buf);
    let mut found: Option<(u64, String)> = None;
    for line in s.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(ts), Some(text)) = (
                v.get("ts").and_then(|t| t.as_u64()),
                v.get("text").and_then(|t| t.as_str()),
            ) {
                found = Some((ts, text.to_string()));
            }
        }
    }
    found
}

fn log_line(log: &Path, decision: &str, reason: &str) {
    let line = if reason.is_empty() {
        serde_json::json!({"ts": now(), "decision": decision}).to_string()
    } else {
        serde_json::json!({"ts": now(), "decision": decision, "reason": reason}).to_string()
    };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
        let _ = f.write_all(line.as_bytes());
        let _ = f.write_all(b"\n");
    }
}

fn env_num(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
