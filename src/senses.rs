//! Unified Sensory Organ Architecture (PLAN §2.5).
//! All senses (proprioception, windows, time, hearing, vision) implement
//! the common `Sense` trait and register into `SensesEngine`.

use crate::config::SensesCfg;
use std::path::Path;

pub struct SenseCtx<'a> {
    pub workspace: &'a Path,
    pub memory_dir: &'a Path,
}

#[derive(Debug, Clone)]
pub struct SenseTrigger {
    pub source: &'static str,
    pub description: String,
}

pub trait Sense: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_enabled(&self, cfg: &SensesCfg) -> bool;
    /// Formats a markdown grounding block for the observation phase.
    fn snapshot(&self, ctx: &SenseCtx) -> Option<String>;
    /// Check for trigger condition (for heartbeat / wake-up).
    fn poll_trigger(&mut self, _ctx: &SenseCtx) -> Option<SenseTrigger> {
        None
    }
}

pub struct ProprioceptionSense;

impl Sense for ProprioceptionSense {
    fn name(&self) -> &'static str {
        "proprioception"
    }

    fn is_enabled(&self, cfg: &SensesCfg) -> bool {
        cfg.proprioception
    }

    fn snapshot(&self, ctx: &SenseCtx) -> Option<String> {
        let vitals = crate::vitals::tail(ctx.memory_dir, 5);
        let mood = crate::friction::read_mood(ctx.memory_dir);
        let mut out = String::new();
        if !vitals.is_empty() {
            out.push_str(&format!("--- vitals tail ---\n{vitals}\n\n"));
        }
        out.push_str(&format!("--- mood (felt, not narrated) ---\n{mood}"));
        Some(out)
    }
}

pub struct WindowsSense;

impl Sense for WindowsSense {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn is_enabled(&self, cfg: &SensesCfg) -> bool {
        cfg.windows
    }

    fn snapshot(&self, _ctx: &SenseCtx) -> Option<String> {
        let snap = crate::winsense::query("overview");
        Some(format!("--- windows environment sense ---\n{snap}"))
    }
}

pub struct TimeSense;

impl Sense for TimeSense {
    fn name(&self) -> &'static str {
        "time"
    }

    fn is_enabled(&self, cfg: &SensesCfg) -> bool {
        cfg.time
    }

    fn snapshot(&self, _ctx: &SenseCtx) -> Option<String> {
        let now = std::time::SystemTime::now();
        let ts = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        Some(format!("--- time sense ---\nunix_timestamp: {ts}"))
    }
}

pub struct HearingSense;

impl Sense for HearingSense {
    fn name(&self) -> &'static str {
        "hearing"
    }

    fn is_enabled(&self, cfg: &SensesCfg) -> bool {
        cfg.hearing
    }

    fn snapshot(&self, ctx: &SenseCtx) -> Option<String> {
        let heard_path = ctx.workspace.join("heard.jsonl");
        if heard_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&heard_path) {
                let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
                if !lines.is_empty() {
                    let last_lines: Vec<&str> = lines.into_iter().rev().take(3).collect();
                    let rev = last_lines.into_iter().rev().collect::<Vec<_>>().join("\n");
                    return Some(format!("--- hearing sense (recent utterances) ---\n{rev}"));
                }
            }
        }
        None
    }
}

pub struct VisionSense;

impl Sense for VisionSense {
    fn name(&self) -> &'static str {
        "vision"
    }

    fn is_enabled(&self, cfg: &SensesCfg) -> bool {
        cfg.vision
    }

    fn snapshot(&self, ctx: &SenseCtx) -> Option<String> {
        let out = std::process::Command::new("git")
            .args(["status", "--porcelain", "-b"])
            .current_dir(ctx.workspace)
            .output()
            .ok()?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(format!("--- vision sense (workspace git) ---\n{s}"));
            }
        }
        None
    }
}

pub struct MailSense;

impl Sense for MailSense {
    fn name(&self) -> &'static str {
        "mail"
    }

    fn is_enabled(&self, _cfg: &SensesCfg) -> bool {
        true
    }

    fn snapshot(&self, ctx: &SenseCtx) -> Option<String> {
        let mail_file = ctx.memory_dir.join("mail.latest.txt");
        if mail_file.is_file() {
            if let Ok(c) = std::fs::read_to_string(&mail_file) {
                let trimmed = c.trim();
                if !trimmed.is_empty() {
                    return Some(format!("--- mail sense (user async speech) ---\n{trimmed}"));
                }
            }
        }
        None
    }
}

pub struct SensesEngine {
    pub senses: Vec<Box<dyn Sense>>,
}

impl Default for SensesEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SensesEngine {
    pub fn new() -> Self {
        Self {
            senses: vec![
                Box::new(ProprioceptionSense),
                Box::new(WindowsSense),
                Box::new(TimeSense),
                Box::new(HearingSense),
                Box::new(VisionSense),
                Box::new(MailSense),
            ],
        }
    }

    pub fn assemble_grounding(
        &self,
        cfg: &SensesCfg,
        mask: Option<&std::collections::HashMap<String, bool>>,
        ctx: &SenseCtx,
    ) -> String {
        let mut sections = Vec::new();
        for s in &self.senses {
            let is_on = mask
                .and_then(|m| m.get(s.name()).copied())
                .unwrap_or_else(|| s.is_enabled(cfg));
            if is_on {
                if let Some(snap) = s.snapshot(ctx) {
                    sections.push(snap);
                }
            }
        }
        if sections.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", sections.join("\n\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_senses_engine_assembly() {
        let temp = tempfile::tempdir().unwrap();
        let engine = SensesEngine::new();
        let cfg = SensesCfg {
            time: true,
            hearing: false,
            vision: false,
            proprioception: true,
            windows: true,
        };
        let ctx = SenseCtx {
            workspace: temp.path(),
            memory_dir: temp.path(),
        };
        let grounding = engine.assemble_grounding(&cfg, None, &ctx);
        assert!(grounding.contains("--- mood (felt, not narrated) ---"));
        assert!(grounding.contains("--- windows environment sense ---"));
        assert!(grounding.contains("--- time sense ---"));
        assert!(!grounding.contains("--- hearing sense ---"));

        // Dynamic mask: mute windows, enable hearing
        let mut mask = std::collections::HashMap::new();
        mask.insert("windows".to_string(), false);
        mask.insert("hearing".to_string(), true);
        let masked = engine.assemble_grounding(&cfg, Some(&mask), &ctx);
        assert!(!masked.contains("--- windows environment sense ---"));
        assert!(grounding.contains("--- mood (felt, not narrated) ---"));
        assert!(grounding.contains("--- windows environment sense ---"));
        assert!(grounding.contains("--- time sense ---"));
        assert!(!grounding.contains("--- hearing sense ---"));
    }
}
